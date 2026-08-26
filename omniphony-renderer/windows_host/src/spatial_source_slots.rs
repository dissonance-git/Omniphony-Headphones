//! Stable renderer-lane allocation for Windows Spatial Audio objects.
//!
//! `SourceFrameRenderer` intentionally treats its channel topology as runtime
//! state. A channel-count change resets that state, which is correct for fixed
//! source integrations but not for Windows dynamic objects: object activation
//! and end-of-stream are ordinary stream events. The Windows host therefore
//! reserves the stream's known static roles plus `MaxDynamicObjectCount` once,
//! then keeps that width and lane ordering stable for the life of the stream.
//!
//! Inactive slots carry zero PCM and identity zero. Reusing a freed slot for a
//! new object is therefore an explicit identity replacement rather than a pose
//! interpolation from the outgoing object. No inactive slot becomes an audible
//! inferred source.

use bridge_api::RChannelLabel;
use renderer::source_scene::{SourceLaneKind, SourceSceneEvidence};
use renderer::stable_source_slots::{StableSourceSlotError, StableSourceSlots};

use crate::spatial_ingress::{
    WindowsSpatialIngressError, WindowsSpatialIngressQuantum, WindowsStaticObjectRole,
    windows_static_label,
};
use crate::spatial_source_frame::{
    WindowsSpatialSourceKey, build_windows_spatial_source_frame,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowsSpatialSourceSlotError {
    Ingress(WindowsSpatialIngressError),
    DuplicateConfiguredStaticRole(WindowsStaticObjectRole),
    UnconfiguredStaticRole(RChannelLabel),
    ReservedDynamicIdZero,
    DynamicCapacityExceeded { capacity: usize, active: usize },
}

impl From<WindowsSpatialIngressError> for WindowsSpatialSourceSlotError {
    fn from(value: WindowsSpatialIngressError) -> Self {
        Self::Ingress(value)
    }
}

/// Fixed-width source frame ready for `SourceFrameRenderer`.
///
/// Static entries in `slot_keys` remain reserved for the stream lifetime.
/// Dynamic entries are `Some` only while that object is active. Every vector
/// has the same stable lane width even when active object count changes.
#[derive(Debug)]
pub struct WindowsSpatialSlottedSourceFrame<'a> {
    pub frame_count: usize,
    pub slot_keys: Vec<Option<WindowsSpatialSourceKey>>,
    pub sources: Vec<SourceSceneEvidence>,
    pub interleaved_pcm: Vec<f32>,
    pub lfe_mono: Option<&'a [f32]>,
}

impl WindowsSpatialSlottedSourceFrame<'_> {
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }
}

/// Stateful stream-local allocator for dynamic Windows object lanes.
///
/// `static_roles` must be the role set activated for the stream. LFE is tracked
/// separately and therefore does not consume a directional renderer slot.
/// `max_dynamic_objects` should be the stream's negotiated maximum dynamic
/// object count, so ordinary object spawn/despawn never changes renderer width.
pub struct WindowsSpatialSourceSlots {
    static_labels: Vec<RChannelLabel>,
    dynamic_slots: StableSourceSlots,
}

impl WindowsSpatialSourceSlots {
    pub fn new(
        static_roles: &[WindowsStaticObjectRole],
        max_dynamic_objects: usize,
    ) -> Result<Self, WindowsSpatialSourceSlotError> {
        let mut seen = [false; 17];
        let mut configured: [Option<WindowsStaticObjectRole>; 17] = [None; 17];
        for role in static_roles.iter().copied() {
            let index = role.canonical_scene_index();
            if seen[index] {
                return Err(WindowsSpatialSourceSlotError::DuplicateConfiguredStaticRole(role));
            }
            seen[index] = true;
            configured[index] = Some(role);
        }

        let mut static_labels = Vec::with_capacity(static_roles.len().min(16));
        for role in crate::spatial_ingress::WINDOWS_STATIC_OBJECT_ROLES_8_1_4_4 {
            if configured[role.canonical_scene_index()].is_some()
                && role != WindowsStaticObjectRole::LowFrequency
            {
                static_labels.push(windows_static_label(role));
            }
        }

        Ok(Self {
            static_labels,
            dynamic_slots: StableSourceSlots::new(max_dynamic_objects),
        })
    }

    pub fn source_count(&self) -> usize {
        self.static_labels.len() + self.dynamic_slots.capacity()
    }

    pub fn max_dynamic_objects(&self) -> usize {
        self.dynamic_slots.capacity()
    }

    /// Lower one active-object quantum into the stream's fixed renderer slots.
    pub fn slot_quantum<'a>(
        &mut self,
        quantum: &WindowsSpatialIngressQuantum<'a>,
    ) -> Result<WindowsSpatialSlottedSourceFrame<'a>, WindowsSpatialSourceSlotError> {
        let active = build_windows_spatial_source_frame(quantum)?;

        for key in &active.source_keys {
            match *key {
                WindowsSpatialSourceKey::Static(label) => {
                    if !self.static_labels.contains(&label) {
                        return Err(WindowsSpatialSourceSlotError::UnconfiguredStaticRole(label));
                    }
                }
                WindowsSpatialSourceKey::Dynamic(0) => {
                    // SourceFrameRenderer reserves identity zero to mean "no
                    // presentation identity". The Windows host owns these IDs,
                    // so reserving zero here keeps dynamic continuity explicit.
                    return Err(WindowsSpatialSourceSlotError::ReservedDynamicIdZero);
                }
                WindowsSpatialSourceKey::Dynamic(_) => {}
            }
        }

        let active_dynamic_count = active
            .source_keys
            .iter()
            .filter(|key| matches!(key, WindowsSpatialSourceKey::Dynamic(_)))
            .count();
        if active_dynamic_count > self.dynamic_slots.capacity() {
            return Err(WindowsSpatialSourceSlotError::DynamicCapacityExceeded {
                capacity: self.dynamic_slots.capacity(),
                active: active_dynamic_count,
            });
        }

        let active_dynamic_ids = active
            .source_keys
            .iter()
            .filter_map(|key| match *key {
                WindowsSpatialSourceKey::Dynamic(id) => Some(id),
                WindowsSpatialSourceKey::Static(_) => None,
            })
            .collect::<Vec<_>>();
        self.dynamic_slots
            .reconcile(&active_dynamic_ids)
            .map_err(|error| match error {
                StableSourceSlotError::ReservedIdZero => {
                    WindowsSpatialSourceSlotError::ReservedDynamicIdZero
                }
                StableSourceSlotError::DuplicateId(id) => {
                    WindowsSpatialSourceSlotError::Ingress(
                        WindowsSpatialIngressError::DuplicateDynamicId(id),
                    )
                }
                StableSourceSlotError::CapacityExceeded { capacity, active } => {
                    WindowsSpatialSourceSlotError::DynamicCapacityExceeded {
                        capacity,
                        active,
                    }
                }
            })?;

        let source_count = self.source_count();
        let mut slot_keys = Vec::with_capacity(source_count);
        slot_keys.extend(
            self.static_labels
                .iter()
                .copied()
                .map(|label| Some(WindowsSpatialSourceKey::Static(label))),
        );
        slot_keys.extend(
            self.dynamic_slots
                .slots()
                .iter()
                .copied()
                .map(|id| id.map(WindowsSpatialSourceKey::Dynamic)),
        );

        let inactive = SourceSceneEvidence {
            lane_kind: SourceLaneKind::DrySource,
            source_id: 0,
            persistent_part_id: None,
            confidence: 0.0,
            ..SourceSceneEvidence::default()
        };
        let mut sources = vec![inactive; source_count];
        let mut active_channel_for_slot = vec![None; source_count];

        for (channel_index, key) in active.source_keys.iter().copied().enumerate() {
            let slot_index = match key {
                WindowsSpatialSourceKey::Static(label) => self
                    .static_labels
                    .iter()
                    .position(|configured| *configured == label)
                    .ok_or(WindowsSpatialSourceSlotError::UnconfiguredStaticRole(label))?,
                WindowsSpatialSourceKey::Dynamic(id) => {
                    let dynamic_index = self
                        .dynamic_slots
                        .slot_for(id)
                        .ok_or(WindowsSpatialSourceSlotError::DynamicCapacityExceeded {
                            capacity: self.dynamic_slots.capacity(),
                            active: active_dynamic_count,
                        })?;
                    self.static_labels.len() + dynamic_index
                }
            };
            sources[slot_index] = active.sources[channel_index];
            active_channel_for_slot[slot_index] = Some(channel_index);
        }

        let mut interleaved_pcm = Vec::with_capacity(
            active
                .frame_count
                .checked_mul(source_count)
                .unwrap_or(usize::MAX),
        );
        for frame_index in 0..active.frame_count {
            for channel_index in &active_channel_for_slot {
                let sample = channel_index
                    .map(|channel| {
                        active.interleaved_pcm[frame_index * active.source_count() + channel]
                    })
                    .unwrap_or(0.0);
                interleaved_pcm.push(sample);
            }
        }

        Ok(WindowsSpatialSlottedSourceFrame {
            frame_count: active.frame_count,
            slot_keys,
            sources,
            interleaved_pcm,
            lfe_mono: active.lfe_mono,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spatial_ingress::{
        WindowsDynamicObject, WindowsSpatialPosition, WindowsStaticObject,
        build_windows_spatial_ingress_quantum,
    };

    fn dynamic<'a>(id: u64, x: f32, pcm: &'a [f32]) -> WindowsDynamicObject<'a> {
        WindowsDynamicObject {
            stable_id: id,
            windows_position: WindowsSpatialPosition::new(x, 0.25, -1.0),
            mono_pcm: pcm,
        }
    }

    #[test]
    fn spawn_despawn_and_provider_reorder_keep_renderer_width_and_slots_stable() {
        let front = [0.1, 0.2];
        let first = [0.3, 0.4];
        let second = [0.5, 0.6];
        let third = [0.7, 0.8];
        let static_objects = [WindowsStaticObject {
            role: WindowsStaticObjectRole::FrontLeft,
            windows_position: None,
            mono_pcm: &front,
        }];
        let mut slots = WindowsSpatialSourceSlots::new(
            &[WindowsStaticObjectRole::FrontLeft, WindowsStaticObjectRole::LowFrequency],
            2,
        )
        .unwrap();
        assert_eq!(slots.source_count(), 3);

        let q1 = build_windows_spatial_ingress_quantum(
            &static_objects,
            &[dynamic(10, -0.5, &first), dynamic(20, 0.5, &second)],
        )
        .unwrap();
        let f1 = slots.slot_quantum(&q1).unwrap();
        assert_eq!(f1.source_count(), 3);
        assert_eq!(
            f1.slot_keys,
            vec![
                Some(WindowsSpatialSourceKey::Static(RChannelLabel::L)),
                Some(WindowsSpatialSourceKey::Dynamic(10)),
                Some(WindowsSpatialSourceKey::Dynamic(20)),
            ]
        );
        assert_eq!(f1.interleaved_pcm, vec![0.1, 0.3, 0.5, 0.2, 0.4, 0.6]);

        // Provider enumeration order changes, but identities keep their slots.
        let q2 = build_windows_spatial_ingress_quantum(
            &static_objects,
            &[dynamic(20, 0.6, &second), dynamic(10, -0.4, &first)],
        )
        .unwrap();
        let f2 = slots.slot_quantum(&q2).unwrap();
        assert_eq!(f2.slot_keys, f1.slot_keys);
        assert_eq!(f2.interleaved_pcm, f1.interleaved_pcm);

        // 10 ends and 30 starts in the same update. 30 reuses the freed slot,
        // while 20 remains on its original lane and total width never changes.
        let q3 = build_windows_spatial_ingress_quantum(
            &static_objects,
            &[dynamic(20, 0.7, &second), dynamic(30, -0.2, &third)],
        )
        .unwrap();
        let f3 = slots.slot_quantum(&q3).unwrap();
        assert_eq!(f3.source_count(), 3);
        assert_eq!(
            f3.slot_keys,
            vec![
                Some(WindowsSpatialSourceKey::Static(RChannelLabel::L)),
                Some(WindowsSpatialSourceKey::Dynamic(30)),
                Some(WindowsSpatialSourceKey::Dynamic(20)),
            ]
        );
        assert_eq!(f3.sources[1].source_id, 30);
        assert_eq!(f3.sources[2].source_id, 20);
    }

    #[test]
    fn inactive_reserved_dynamic_lane_is_silent_and_has_no_identity() {
        let front = [1.0, -1.0];
        let static_objects = [WindowsStaticObject {
            role: WindowsStaticObjectRole::FrontLeft,
            windows_position: None,
            mono_pcm: &front,
        }];
        let ingress = build_windows_spatial_ingress_quantum(&static_objects, &[]).unwrap();
        let mut slots = WindowsSpatialSourceSlots::new(&[WindowsStaticObjectRole::FrontLeft], 1).unwrap();
        let frame = slots.slot_quantum(&ingress).unwrap();

        assert_eq!(frame.source_count(), 2);
        assert_eq!(frame.slot_keys[1], None);
        assert_eq!(frame.sources[1].source_id, 0);
        assert_eq!(frame.sources[1].persistent_part_id, None);
        assert_eq!(frame.sources[1].confidence, 0.0);
        assert_eq!(frame.interleaved_pcm, vec![1.0, 0.0, -1.0, 0.0]);
    }

    #[test]
    fn dynamic_capacity_and_identity_zero_are_explicit_host_contract_errors() {
        let pcm = [0.25];
        let mut zero_slots = WindowsSpatialSourceSlots::new(&[], 1).unwrap();
        let zero = build_windows_spatial_ingress_quantum(&[], &[dynamic(0, 0.0, &pcm)]).unwrap();
        assert_eq!(
            zero_slots.slot_quantum(&zero).unwrap_err(),
            WindowsSpatialSourceSlotError::ReservedDynamicIdZero
        );

        let mut one_slot = WindowsSpatialSourceSlots::new(&[], 1).unwrap();
        let overflow = build_windows_spatial_ingress_quantum(
            &[],
            &[dynamic(1, -0.5, &pcm), dynamic(2, 0.5, &pcm)],
        )
        .unwrap();
        assert_eq!(
            one_slot.slot_quantum(&overflow).unwrap_err(),
            WindowsSpatialSourceSlotError::DynamicCapacityExceeded {
                capacity: 1,
                active: 2,
            }
        );
    }

    #[test]
    fn lfe_stays_outside_fixed_directional_slot_width() {
        let lfe = [0.4, -0.4];
        let ingress = build_windows_spatial_ingress_quantum(
            &[WindowsStaticObject {
                role: WindowsStaticObjectRole::LowFrequency,
                windows_position: None,
                mono_pcm: &lfe,
            }],
            &[],
        )
        .unwrap();
        let mut slots = WindowsSpatialSourceSlots::new(&[WindowsStaticObjectRole::LowFrequency], 1).unwrap();
        let frame = slots.slot_quantum(&ingress).unwrap();
        assert_eq!(frame.source_count(), 1);
        assert_eq!(frame.slot_keys, vec![None]);
        assert_eq!(frame.interleaved_pcm, vec![0.0, 0.0]);
        assert_eq!(frame.lfe_mono, Some(lfe.as_slice()));
    }
}

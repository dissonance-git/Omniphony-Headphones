//! Windows Spatial Audio object ingress, before any rendering decision.
//!
//! The accepted Omniphony channel/object contract has two distinct source
//! classes: fixed channels are identified by `RChannelLabel`, while dynamic
//! objects keep an opaque stable object identity and authored motion. Windows
//! Spatial Audio already exposes exactly those two classes, so this adapter
//! preserves them instead of collapsing the stream into WAVEFORMATEXTENSIBLE
//! or inventing one shared numeric ID space.
//!
//! This module deliberately stops before COM/provider activation. It is the
//! lossless packet boundary that a proven Windows Spatial Sound provider must
//! feed once the system-instantiation experiment succeeds.

use bridge_api::RChannelLabel;
use renderer::authored_scene::MetricPosition;

// Keep the already-landed Windows semantic contract as the single source of
// truth while provider activation is still experimental. If that experiment
// succeeds, this contract can move to a shared platform-contract crate without
// changing the ingress semantics below.
#[path = "../../realtime_ffi/src/windows_spatial_contract.rs"]
mod windows_spatial_contract;

pub use windows_spatial_contract::{
    WINDOWS_STATIC_OBJECT_ROLES_8_1_4_4, WindowsDynamicObject, WindowsSpatialPosition,
    WindowsStaticObject, WindowsStaticObjectRole,
};

#[derive(Clone, Copy, Debug)]
pub struct WindowsSpatialStaticLane<'a> {
    /// Exact Windows static role received/activated by the host.
    pub role: WindowsStaticObjectRole,
    /// Existing Omniphony fixed-channel vocabulary. This is the rendering
    /// identity; the endpoint position below is retained as source truth rather
    /// than converting this fixed channel into a fake dynamic object.
    pub label: RChannelLabel,
    /// Endpoint-reported static geometry transformed into Omniphony axes when
    /// available. LFE is always `None` because it is non-directional.
    pub endpoint_position: Option<MetricPosition>,
    pub mono_pcm: &'a [f32],
}

#[derive(Clone, Copy, Debug)]
pub struct WindowsSpatialDynamicLane<'a> {
    /// Opaque stable identity owned by the Windows-facing host. It remains u64
    /// here so no bridge/API width conversion can silently truncate it.
    pub stable_id: u64,
    /// Exact authored dynamic position transformed only between coordinate
    /// conventions. No bed quantization or inferred presentation is applied.
    pub authored_position: MetricPosition,
    pub mono_pcm: &'a [f32],
}

#[derive(Debug)]
pub struct WindowsSpatialIngressQuantum<'a> {
    pub frame_count: usize,
    /// Canonical 8.1.4.4 order, containing only the static roles active in this
    /// quantum. A provider may legitimately expose a subset of the full bed.
    pub static_lanes: Vec<WindowsSpatialStaticLane<'a>>,
    /// Host order is retained so a later provider can keep its stable object
    /// slots. Identity is never derived from the lane index.
    pub dynamic_lanes: Vec<WindowsSpatialDynamicLane<'a>>,
}

impl WindowsSpatialIngressQuantum<'_> {
    pub fn has_complete_static_8_1_4_4(&self) -> bool {
        self.static_lanes.len() == WINDOWS_STATIC_OBJECT_ROLES_8_1_4_4.len()
            && WINDOWS_STATIC_OBJECT_ROLES_8_1_4_4
                .into_iter()
                .zip(self.static_lanes.iter())
                .all(|(role, lane)| role == lane.role)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowsSpatialIngressError {
    DuplicateStaticRole(WindowsStaticObjectRole),
    DuplicateDynamicId(u64),
    FrameCountMismatch { expected: usize, actual: usize },
}

/// Map the complete Windows 8.1.4.4 static vocabulary into Omniphony's
/// authoritative fixed-channel vocabulary.
pub const fn windows_static_label(role: WindowsStaticObjectRole) -> RChannelLabel {
    match role {
        WindowsStaticObjectRole::FrontLeft => RChannelLabel::L,
        WindowsStaticObjectRole::FrontRight => RChannelLabel::R,
        WindowsStaticObjectRole::FrontCenter => RChannelLabel::C,
        WindowsStaticObjectRole::LowFrequency => RChannelLabel::LFE,
        WindowsStaticObjectRole::SideLeft => RChannelLabel::Ls,
        WindowsStaticObjectRole::SideRight => RChannelLabel::Rs,
        WindowsStaticObjectRole::BackLeft => RChannelLabel::Lb,
        WindowsStaticObjectRole::BackRight => RChannelLabel::Rb,
        WindowsStaticObjectRole::BackCenter => RChannelLabel::Cb,
        WindowsStaticObjectRole::TopFrontLeft => RChannelLabel::Tfl,
        WindowsStaticObjectRole::TopFrontRight => RChannelLabel::Tfr,
        WindowsStaticObjectRole::TopBackLeft => RChannelLabel::Tbl,
        WindowsStaticObjectRole::TopBackRight => RChannelLabel::Tbr,
        WindowsStaticObjectRole::BottomFrontLeft => RChannelLabel::Bfl,
        WindowsStaticObjectRole::BottomFrontRight => RChannelLabel::Bfr,
        WindowsStaticObjectRole::BottomBackLeft => RChannelLabel::Bbl,
        WindowsStaticObjectRole::BottomBackRight => RChannelLabel::Bbr,
    }
}

fn establish_frame_count(
    expected: &mut Option<usize>,
    actual: usize,
) -> Result<(), WindowsSpatialIngressError> {
    match *expected {
        Some(value) if value != actual => Err(WindowsSpatialIngressError::FrameCountMismatch {
            expected: value,
            actual,
        }),
        Some(_) => Ok(()),
        None => {
            *expected = Some(actual);
            Ok(())
        }
    }
}

/// Assemble one object-update quantum without reducing source semantics.
///
/// Static input enumeration order is normalized to the canonical 8.1.4.4
/// order. Dynamic input order, IDs, positions, and mono buffers pass through.
/// Every active object buffer must describe the same frame count, matching the
/// Windows Spatial Audio update-quantum contract.
pub fn build_windows_spatial_ingress_quantum<'a>(
    static_objects: &[WindowsStaticObject<'a>],
    dynamic_objects: &[WindowsDynamicObject<'a>],
) -> Result<WindowsSpatialIngressQuantum<'a>, WindowsSpatialIngressError> {
    let mut frame_count = None;
    let mut static_slots: [Option<WindowsStaticObject<'a>>; 17] = [None; 17];

    for object in static_objects.iter().copied() {
        establish_frame_count(&mut frame_count, object.mono_pcm.len())?;
        let index = object.role.canonical_scene_index();
        if static_slots[index].is_some() {
            return Err(WindowsSpatialIngressError::DuplicateStaticRole(object.role));
        }
        static_slots[index] = Some(object);
    }

    for (index, object) in dynamic_objects.iter().copied().enumerate() {
        establish_frame_count(&mut frame_count, object.mono_pcm.len())?;
        if dynamic_objects[..index]
            .iter()
            .any(|previous| previous.stable_id == object.stable_id)
        {
            return Err(WindowsSpatialIngressError::DuplicateDynamicId(
                object.stable_id,
            ));
        }
    }

    let mut static_lanes = Vec::with_capacity(static_objects.len());
    for role in WINDOWS_STATIC_OBJECT_ROLES_8_1_4_4 {
        if let Some(object) = static_slots[role.canonical_scene_index()] {
            static_lanes.push(WindowsSpatialStaticLane {
                role,
                label: windows_static_label(role),
                endpoint_position: object.omniphony_metric_position(),
                mono_pcm: object.mono_pcm,
            });
        }
    }

    let dynamic_lanes = dynamic_objects
        .iter()
        .copied()
        .map(|object| WindowsSpatialDynamicLane {
            stable_id: object.stable_id,
            authored_position: object.omniphony_metric_position(),
            mono_pcm: object.mono_pcm,
        })
        .collect();

    Ok(WindowsSpatialIngressQuantum {
        frame_count: frame_count.unwrap_or(0),
        static_lanes,
        dynamic_lanes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED_8_1_4_4_LABELS: [RChannelLabel; 17] = [
        RChannelLabel::L,
        RChannelLabel::R,
        RChannelLabel::C,
        RChannelLabel::LFE,
        RChannelLabel::Ls,
        RChannelLabel::Rs,
        RChannelLabel::Lb,
        RChannelLabel::Rb,
        RChannelLabel::Cb,
        RChannelLabel::Tfl,
        RChannelLabel::Tfr,
        RChannelLabel::Tbl,
        RChannelLabel::Tbr,
        RChannelLabel::Bfl,
        RChannelLabel::Bfr,
        RChannelLabel::Bbl,
        RChannelLabel::Bbr,
    ];

    #[test]
    fn full_windows_8_1_4_4_maps_to_existing_fixed_labels_without_mask_loss() {
        let pcm: Vec<[f32; 2]> = (0..17)
            .map(|index| [index as f32 + 0.25, -(index as f32) - 0.5])
            .collect();
        let static_objects: Vec<_> = WINDOWS_STATIC_OBJECT_ROLES_8_1_4_4
            .into_iter()
            .rev()
            .map(|role| {
                let index = role.canonical_scene_index();
                WindowsStaticObject {
                    role,
                    windows_position: Some(WindowsSpatialPosition::new(
                        index as f32 * 0.125,
                        if role.is_upper() {
                            1.0
                        } else if role.is_lower() {
                            -1.0
                        } else {
                            0.0
                        },
                        -1.0,
                    )),
                    mono_pcm: &pcm[index],
                }
            })
            .collect();

        let quantum = build_windows_spatial_ingress_quantum(&static_objects, &[]).unwrap();
        assert_eq!(quantum.frame_count, 2);
        assert!(quantum.has_complete_static_8_1_4_4());
        assert_eq!(quantum.static_lanes.len(), 17);

        for (index, lane) in quantum.static_lanes.iter().enumerate() {
            assert_eq!(lane.role, WINDOWS_STATIC_OBJECT_ROLES_8_1_4_4[index]);
            assert_eq!(lane.label, EXPECTED_8_1_4_4_LABELS[index]);
            assert_eq!(lane.mono_pcm, &pcm[index]);
        }

        assert_eq!(quantum.static_lanes[13].label, RChannelLabel::Bfl);
        assert_eq!(quantum.static_lanes[14].label, RChannelLabel::Bfr);
        assert_eq!(quantum.static_lanes[15].label, RChannelLabel::Bbl);
        assert_eq!(quantum.static_lanes[16].label, RChannelLabel::Bbr);
    }

    #[test]
    fn dynamic_ids_positions_order_and_pcm_are_lossless() {
        let first_pcm = [0.125, -0.25, 0.5];
        let second_pcm = [-0.75, 0.25, 1.0];
        let dynamic_objects = [
            WindowsDynamicObject {
                stable_id: u64::MAX,
                windows_position: WindowsSpatialPosition::new(-0.75, 0.25, -1.5),
                mono_pcm: &first_pcm,
            },
            WindowsDynamicObject {
                stable_id: 42,
                windows_position: WindowsSpatialPosition::new(0.5, -0.5, 0.25),
                mono_pcm: &second_pcm,
            },
        ];

        let quantum = build_windows_spatial_ingress_quantum(&[], &dynamic_objects).unwrap();
        assert_eq!(quantum.frame_count, 3);
        assert_eq!(quantum.dynamic_lanes.len(), 2);
        assert_eq!(quantum.dynamic_lanes[0].stable_id, u64::MAX);
        assert_eq!(quantum.dynamic_lanes[0].authored_position, [-0.75, 1.5, 0.25]);
        assert_eq!(quantum.dynamic_lanes[0].mono_pcm, first_pcm);
        assert_eq!(quantum.dynamic_lanes[1].stable_id, 42);
        assert_eq!(quantum.dynamic_lanes[1].authored_position, [0.5, -0.25, -0.5]);
        assert_eq!(quantum.dynamic_lanes[1].mono_pcm, second_pcm);
    }

    #[test]
    fn lfe_stays_fixed_and_non_directional_even_with_reported_geometry() {
        let pcm = [1.0, 0.5];
        let object = WindowsStaticObject {
            role: WindowsStaticObjectRole::LowFrequency,
            windows_position: Some(WindowsSpatialPosition::new(9.0, 9.0, 9.0)),
            mono_pcm: &pcm,
        };
        let quantum = build_windows_spatial_ingress_quantum(&[object], &[]).unwrap();
        assert_eq!(quantum.static_lanes[0].label, RChannelLabel::LFE);
        assert_eq!(quantum.static_lanes[0].endpoint_position, None);
        assert_eq!(quantum.static_lanes[0].mono_pcm, pcm);
    }

    #[test]
    fn partial_native_static_sets_are_valid_without_fabricating_missing_roles() {
        let pcm = [0.25];
        let objects = [WindowsStaticObject {
            role: WindowsStaticObjectRole::BottomBackRight,
            windows_position: Some(WindowsSpatialPosition::new(0.5, -0.5, 1.0)),
            mono_pcm: &pcm,
        }];
        let quantum = build_windows_spatial_ingress_quantum(&objects, &[]).unwrap();
        assert!(!quantum.has_complete_static_8_1_4_4());
        assert_eq!(quantum.static_lanes.len(), 1);
        assert_eq!(quantum.static_lanes[0].label, RChannelLabel::Bbr);
        assert_eq!(quantum.static_lanes[0].endpoint_position, Some([0.5, -1.0, -0.5]));
    }

    #[test]
    fn duplicate_source_declarations_are_rejected_not_merged() {
        let pcm = [0.0];
        let static_object = WindowsStaticObject {
            role: WindowsStaticObjectRole::FrontLeft,
            windows_position: None,
            mono_pcm: &pcm,
        };
        assert_eq!(
            build_windows_spatial_ingress_quantum(&[static_object, static_object], &[])
                .unwrap_err(),
            WindowsSpatialIngressError::DuplicateStaticRole(WindowsStaticObjectRole::FrontLeft)
        );

        let dynamic_object = WindowsDynamicObject {
            stable_id: 7,
            windows_position: WindowsSpatialPosition::new(0.0, 0.0, -1.0),
            mono_pcm: &pcm,
        };
        assert_eq!(
            build_windows_spatial_ingress_quantum(&[], &[dynamic_object, dynamic_object])
                .unwrap_err(),
            WindowsSpatialIngressError::DuplicateDynamicId(7)
        );
    }

    #[test]
    fn every_object_in_a_windows_update_quantum_must_share_frame_count() {
        let static_pcm = [0.0, 0.0];
        let dynamic_pcm = [0.0];
        let static_object = WindowsStaticObject {
            role: WindowsStaticObjectRole::FrontCenter,
            windows_position: None,
            mono_pcm: &static_pcm,
        };
        let dynamic_object = WindowsDynamicObject {
            stable_id: 9,
            windows_position: WindowsSpatialPosition::new(0.0, 0.0, -1.0),
            mono_pcm: &dynamic_pcm,
        };
        assert_eq!(
            build_windows_spatial_ingress_quantum(&[static_object], &[dynamic_object]).unwrap_err(),
            WindowsSpatialIngressError::FrameCountMismatch {
                expected: 2,
                actual: 1,
            }
        );
    }
}

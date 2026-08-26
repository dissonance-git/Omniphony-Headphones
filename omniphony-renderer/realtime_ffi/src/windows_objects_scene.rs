//! Worker-side renderer for a complete Windows Spatial Audio object quantum.
//!
//! Static roles and dynamic objects enter one authored source scene and therefore
//! one HRTF/rendering pass. Dynamic object IDs remain stable across quanta while
//! their listener-relative Cartesian position may move continuously. No dynamic
//! position is quantized to the 17-role static bed.

use crate::StereoLookaheadPeakGuard;
use crate::noire_x_profile::NoireXPersonalEq;
use crate::windows_spatial_contract::{
    WindowsDynamicObject, WindowsStaticObject, WindowsStaticObjectRole,
};
use orender_engine::{
    SourceRendererOptions, SourceSpatialMode, build_source_frame_renderer,
};
use renderer::source_frame::SourceFrameRenderer;
use renderer::source_scene::{SourceLaneKind, SourceSceneEvidence};
use scene_contract::stable_source_slots::StableSourceSlots;
use std::f32::consts::PI;

const OBJECT_OUTPUT_GAIN: f32 = 0.90;
const LFE_CUTOFF_HZ: f32 = 120.0;
const STATIC_SOURCE_NAMESPACE: u64 = 0x5354_4154_4943_0000;

fn static_source_id(role: WindowsStaticObjectRole) -> u64 {
    STATIC_SOURCE_NAMESPACE | role.canonical_scene_index() as u64
}

#[derive(Clone, Copy, Debug)]
struct OnePoleLowPass {
    alpha: f32,
    state: f32,
}

impl OnePoleLowPass {
    fn new(sample_rate_hz: u32, cutoff_hz: f32) -> Self {
        let rate = sample_rate_hz.max(1) as f32;
        let alpha = 1.0 - (-2.0 * PI * cutoff_hz.max(1.0) / rate).exp();
        Self { alpha, state: 0.0 }
    }

    fn process(&mut self, sample: f32) -> f32 {
        let input = if sample.is_finite() { sample } else { 0.0 };
        self.state += self.alpha * (input - self.state);
        self.state
    }
}

struct LfeLowPass {
    first: OnePoleLowPass,
    second: OnePoleLowPass,
}

impl LfeLowPass {
    fn new(sample_rate_hz: u32) -> Self {
        Self {
            first: OnePoleLowPass::new(sample_rate_hz, LFE_CUTOFF_HZ),
            second: OnePoleLowPass::new(sample_rate_hz, LFE_CUTOFF_HZ),
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        self.second.process(self.first.process(sample))
    }
}

/// One renderer instance for one Windows Spatial Audio stream. The static role
/// set is fixed by stream activation; the dynamic set may change every quantum.
pub(crate) struct WindowsSpatialObjectPipeline {
    renderer: SourceFrameRenderer,
    sources: Vec<SourceSceneEvidence>,
    interleaved: Vec<f32>,
    render_buf: Vec<f32>,
    lfe: LfeLowPass,
    headphone_eq: NoireXPersonalEq,
    peak_guard: StereoLookaheadPeakGuard,
    sample_pos: u64,
    dynamic_slots: StableSourceSlots,
    previous_dynamic_slots: Vec<Option<u64>>,
    dynamic_ids: Vec<u64>,
    dynamic_slot_to_input: Vec<Option<usize>>,
    active_lanes: Vec<bool>,
    presentation_ramp_frames: Vec<u32>,
}

impl WindowsSpatialObjectPipeline {
    pub(crate) fn new(sample_rate_hz: u32, max_dynamic_objects: usize) -> Result<Self, String> {
        let renderer = build_source_frame_renderer(
            sample_rate_hz,
            None,
            SourceRendererOptions {
                mode: SourceSpatialMode::FullSphere,
                externalization: false,
                authored_metric_objects: true,
                ..SourceRendererOptions::default()
            },
        )
        .map_err(|error| error.to_string())?;

        Ok(Self {
            renderer,
            sources: Vec::new(),
            interleaved: Vec::new(),
            render_buf: Vec::new(),
            lfe: LfeLowPass::new(sample_rate_hz),
            headphone_eq: NoireXPersonalEq::new(sample_rate_hz),
            peak_guard: StereoLookaheadPeakGuard::new(sample_rate_hz),
            sample_pos: 0,
            dynamic_slots: StableSourceSlots::new(max_dynamic_objects),
            previous_dynamic_slots: vec![None; max_dynamic_objects],
            dynamic_ids: Vec::with_capacity(max_dynamic_objects),
            dynamic_slot_to_input: vec![None; max_dynamic_objects],
            active_lanes: Vec::with_capacity(16 + max_dynamic_objects),
            presentation_ramp_frames: Vec::with_capacity(16 + max_dynamic_objects),
        })
    }

    /// Advance stream time through a quantum in which a dynamic-only stream has
    /// no currently active objects. No source is fabricated merely to keep the
    /// clock moving.
    pub(crate) fn process_silence(&mut self, frames: usize) -> Result<Vec<f32>, String> {
        if frames == 0 {
            return Err("spatial object silence quantum has zero frames".to_string());
        }
        self.previous_dynamic_slots
            .copy_from_slice(self.dynamic_slots.slots());
        self.dynamic_slots
            .reconcile(&[])
            .map_err(|error| format!("dynamic slot release failed: {error:?}"))?;
        for (slot, previous) in self.previous_dynamic_slots.iter().copied().enumerate() {
            if previous.is_some() {
                // process_silence is used only when this stream has no static
                // objects, so dynamic slot zero is renderer lane zero.
                self.renderer.reset_channel_runtime_state(slot);
            }
        }
        self.sample_pos = self.sample_pos.saturating_add(frames as u64);
        Ok(self.peak_guard.process_interleaved(&vec![0.0f32; frames * 2]))
    }

    fn validate_quantum(
        static_objects: &[WindowsStaticObject<'_>],
        dynamic_objects: &[WindowsDynamicObject<'_>],
    ) -> Result<usize, String> {
        if static_objects.is_empty() && dynamic_objects.is_empty() {
            return Err("spatial object quantum is empty".to_string());
        }

        let frames = static_objects
            .first()
            .map(|object| object.mono_pcm.len())
            .or_else(|| dynamic_objects.first().map(|object| object.mono_pcm.len()))
            .unwrap_or(0);
        if frames == 0 {
            return Err("spatial object quantum has zero frames".to_string());
        }
        if static_objects.iter().any(|object| object.mono_pcm.len() != frames)
            || dynamic_objects.iter().any(|object| object.mono_pcm.len() != frames)
        {
            return Err("spatial object PCM frame counts differ within one quantum".to_string());
        }

        let mut seen_static = [false; 17];
        for object in static_objects {
            let index = object.role.canonical_scene_index();
            if seen_static[index] {
                return Err(format!("duplicate static object role {:?}", object.role));
            }
            seen_static[index] = true;
            if object.role.is_directional() && object.windows_position.is_none() {
                return Err(format!(
                    "directional static object {:?} has no authored position",
                    object.role
                ));
            }
        }

        for (index, object) in dynamic_objects.iter().enumerate() {
            if dynamic_objects[..index]
                .iter()
                .any(|previous| previous.stable_id == object.stable_id)
            {
                return Err(format!("duplicate dynamic object id {}", object.stable_id));
            }
            let position = object.windows_position;
            if !position.x_right_m.is_finite()
                || !position.y_up_m.is_finite()
                || !position.z_back_m.is_finite()
            {
                return Err(format!(
                    "dynamic object {} has non-finite position",
                    object.stable_id
                ));
            }
            if static_objects.iter().any(|static_object| {
                static_source_id(static_object.role) == object.stable_id
            }) {
                return Err(format!(
                    "dynamic object id {} collides with a static source id",
                    object.stable_id
                ));
            }
        }

        Ok(frames)
    }

    /// Render one complete authored object quantum. Directional static objects
    /// retain their fixed role geometry, dynamic objects retain stable identity
    /// and continuous XYZ, and LFE remains non-directional.
    pub(crate) fn process(
        &mut self,
        static_objects: &[WindowsStaticObject<'_>],
        dynamic_objects: &[WindowsDynamicObject<'_>],
    ) -> Result<Vec<f32>, String> {
        let frames = Self::validate_quantum(static_objects, dynamic_objects)?;

        let mut directional_static = Vec::with_capacity(static_objects.len());
        let mut lfe = None;
        for object in static_objects {
            if object.role == WindowsStaticObjectRole::LowFrequency {
                lfe = Some(object);
            } else {
                directional_static.push(object);
            }
        }
        directional_static.sort_by_key(|object| object.role.canonical_scene_index());

        self.previous_dynamic_slots
            .copy_from_slice(self.dynamic_slots.slots());
        self.dynamic_ids.clear();
        self.dynamic_ids
            .extend(dynamic_objects.iter().map(|object| object.stable_id));
        self.dynamic_slots
            .reconcile(&self.dynamic_ids)
            .map_err(|error| format!("dynamic slot reconciliation failed: {error:?}"))?;
        let dynamic_span = self.dynamic_slots.active_span_len();
        self.dynamic_slot_to_input[..dynamic_span].fill(None);
        for (input_index, object) in dynamic_objects.iter().enumerate() {
            let slot = self
                .dynamic_slots
                .slot_for(object.stable_id)
                .ok_or_else(|| format!("dynamic object {} lost stable slot", object.stable_id))?;
            self.dynamic_slot_to_input[slot] = Some(input_index);
        }

        self.sources.clear();
        self.sources.reserve(directional_static.len() + dynamic_span);
        self.active_lanes.clear();
        self.active_lanes
            .reserve(directional_static.len() + dynamic_span);
        self.presentation_ramp_frames.clear();
        self.presentation_ramp_frames
            .reserve(directional_static.len() + dynamic_span);

        for object in &directional_static {
            let position = object
                .omniphony_metric_position()
                .ok_or_else(|| format!("static object {:?} lost position", object.role))?;
            let source_id = static_source_id(object.role);
            self.sources.push(SourceSceneEvidence {
                lane_kind: SourceLaneKind::DrySource,
                source_id,
                persistent_part_id: Some(source_id),
                authored_position: Some(position),
                confidence: 1.0,
                ..SourceSceneEvidence::default()
            });
            // Static role geometry is already fixed for the stream.
            self.active_lanes.push(true);
            self.presentation_ramp_frames.push(0);
        }

        let static_count = directional_static.len();
        for slot in 0..dynamic_span {
            let slot_id = self.dynamic_slots.slots()[slot];
            let Some(id) = slot_id else {
                self.sources.push(SourceSceneEvidence {
                    lane_kind: SourceLaneKind::DrySource,
                    source_id: 0,
                    persistent_part_id: None,
                    confidence: 0.0,
                    ..SourceSceneEvidence::default()
                });
                self.active_lanes.push(false);
                self.presentation_ramp_frames.push(0);
                continue;
            };

            let input_index = self.dynamic_slot_to_input[slot]
                .ok_or_else(|| format!("active dynamic slot {slot} has no input object"))?;
            let object = &dynamic_objects[input_index];

            // Reusing a physical slot for a different identity must clear only
            // that lane's convolution/ramp history. Surviving objects retain
            // their state even when another object ends or provider order moves.
            if self.previous_dynamic_slots[slot] != Some(id) {
                self.renderer
                    .reset_channel_runtime_state(static_count + slot);
            }

            self.sources.push(SourceSceneEvidence {
                lane_kind: SourceLaneKind::DrySource,
                source_id: id,
                persistent_part_id: Some(id),
                authored_position: Some(object.omniphony_metric_position()),
                confidence: 1.0,
                ..SourceSceneEvidence::default()
            });
            self.active_lanes.push(true);

            let ramp = if self.previous_dynamic_slots[slot] == Some(id) {
                frames.min(u32::MAX as usize) as u32
            } else {
                0
            };
            self.presentation_ramp_frames.push(ramp);
        }

        let source_count = directional_static.len() + dynamic_span;
        self.interleaved.clear();
        self.interleaved.reserve(frames.saturating_mul(source_count));
        for frame_index in 0..frames {
            for object in &directional_static {
                let sample = object.mono_pcm[frame_index];
                self.interleaved
                    .push(if sample.is_finite() { sample } else { 0.0 });
            }
            for slot in 0..dynamic_span {
                let sample = self.dynamic_slot_to_input[slot]
                    .map(|input_index| dynamic_objects[input_index].mono_pcm[frame_index])
                    .unwrap_or(0.0);
                self.interleaved
                    .push(if sample.is_finite() { sample } else { 0.0 });
            }
        }

        let mut mixed = if source_count == 0 {
            vec![0.0f32; frames * 2]
        } else {
            self.renderer
                .render_source_frame_with_lane_activity(
                    &self.interleaved,
                    &self.sources,
                    None,
                    None,
                    Some(&self.presentation_ramp_frames),
                    Some(&self.active_lanes),
                    self.sample_pos,
                    0,
                    std::mem::take(&mut self.render_buf),
                    false,
                )
                .map_err(|error| error.to_string())?
                .samples
        };

        if mixed.len() != frames * 2 {
            return Err(format!(
                "spatial object renderer returned {} samples for {frames} frames",
                mixed.len()
            ));
        }

        if let Some(lfe) = lfe {
            for (frame_index, &sample) in lfe.mono_pcm.iter().enumerate() {
                let low = self.lfe.process(sample);
                mixed[frame_index * 2] += low;
                mixed[frame_index * 2 + 1] += low;
            }
        }

        for sample in &mut mixed {
            *sample = if sample.is_finite() {
                *sample * OBJECT_OUTPUT_GAIN
            } else {
                0.0
            };
        }

        self.headphone_eq.process_interleaved(&mut mixed);
        self.sample_pos = self.sample_pos.saturating_add(frames as u64);
        Ok(self.peak_guard.process_interleaved(&mixed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows_spatial_contract::WindowsSpatialPosition;

    #[test]
    fn moving_dynamic_object_keeps_identity_without_bed_quantization() {
        let pcm = [0.2f32; 32];
        let first = WindowsDynamicObject {
            stable_id: 0x4459_4e41_0000_0041,
            windows_position: WindowsSpatialPosition::new(-0.75, 0.25, -1.0),
            mono_pcm: &pcm,
        };
        let second = WindowsDynamicObject {
            stable_id: first.stable_id,
            windows_position: WindowsSpatialPosition::new(0.9, -0.2, 0.4),
            mono_pcm: &pcm,
        };
        assert_eq!(first.stable_id, second.stable_id);
        assert_ne!(first.omniphony_metric_position(), second.omniphony_metric_position());
    }

    #[test]
    fn dynamic_slots_preserve_survivors_and_reuse_only_freed_lanes() {
        let mut slots = StableSourceSlots::new(3);
        slots.reconcile(&[9, 3]).unwrap();
        assert_eq!(slots.slot_for(9), Some(0));
        assert_eq!(slots.slot_for(3), Some(1));

        let before = slots.slots().to_vec();
        slots.reconcile(&[3, 9]).unwrap();
        assert_eq!(slots.slots(), before.as_slice());

        slots.reconcile(&[3, 11]).unwrap();
        assert_eq!(slots.slot_for(3), Some(1));
        assert_eq!(slots.slot_for(11), Some(0));
        assert_eq!(slots.active_span_len(), 2);
    }

    #[test]
    fn dynamic_ids_must_be_unique_inside_one_quantum() {
        let pcm = [0.0f32; 8];
        let object = WindowsDynamicObject {
            stable_id: 7,
            windows_position: WindowsSpatialPosition::new(0.0, 0.0, -1.0),
            mono_pcm: &pcm,
        };
        assert!(WindowsSpatialObjectPipeline::validate_quantum(&[], &[object, object]).is_err());
    }

    #[test]
    fn static_and_dynamic_sources_share_one_render_pass() {
        let static_pcm = [0.1f32; 64];
        let dynamic_pcm = [0.15f32; 64];
        let static_objects = [WindowsStaticObject {
            role: WindowsStaticObjectRole::TopFrontLeft,
            windows_position: Some(WindowsSpatialPosition::new(-0.5, 0.7, -0.8)),
            mono_pcm: &static_pcm,
        }];
        let dynamic_objects = [WindowsDynamicObject {
            stable_id: 0x4459_4e41_0000_0001,
            windows_position: WindowsSpatialPosition::new(0.6, -0.1, -1.2),
            mono_pcm: &dynamic_pcm,
        }];
        assert_eq!(
            WindowsSpatialObjectPipeline::validate_quantum(&static_objects, &dynamic_objects)
                .unwrap(),
            64
        );
    }
}

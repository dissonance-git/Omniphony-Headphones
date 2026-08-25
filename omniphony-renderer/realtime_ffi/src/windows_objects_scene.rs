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
}

impl WindowsSpatialObjectPipeline {
    pub(crate) fn new(sample_rate_hz: u32) -> Result<Self, String> {
        let renderer = build_source_frame_renderer(
            sample_rate_hz,
            None,
            SourceRendererOptions {
                mode: SourceSpatialMode::FullSphere,
                externalization: false,
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
        })
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

        self.sources.clear();
        self.sources.reserve(static_objects.len() + dynamic_objects.len());

        let mut directional_static = Vec::with_capacity(static_objects.len());
        let mut lfe = None;
        for object in static_objects {
            if object.role == WindowsStaticObjectRole::LowFrequency {
                lfe = Some(object);
                continue;
            }
            let position = object
                .omniphony_position()
                .ok_or_else(|| format!("static object {:?} lost position", object.role))?;
            let source_id = static_source_id(object.role);
            self.sources.push(SourceSceneEvidence {
                lane_kind: SourceLaneKind::DrySource,
                source_id,
                persistent_part_id: Some(source_id),
                authored_position: Some([
                    position[0] as f64,
                    position[1] as f64,
                    position[2] as f64,
                ]),
                confidence: 1.0,
                ..SourceSceneEvidence::default()
            });
            directional_static.push(object);
        }

        for object in dynamic_objects {
            let position = object.omniphony_position();
            self.sources.push(SourceSceneEvidence {
                lane_kind: SourceLaneKind::DrySource,
                source_id: object.stable_id,
                persistent_part_id: Some(object.stable_id),
                authored_position: Some([
                    position[0] as f64,
                    position[1] as f64,
                    position[2] as f64,
                ]),
                confidence: 1.0,
                ..SourceSceneEvidence::default()
            });
        }

        let source_count = directional_static.len() + dynamic_objects.len();
        self.interleaved.clear();
        self.interleaved.reserve(frames.saturating_mul(source_count));
        for frame_index in 0..frames {
            for object in &directional_static {
                let sample = object.mono_pcm[frame_index];
                self.interleaved
                    .push(if sample.is_finite() { sample } else { 0.0 });
            }
            for object in dynamic_objects {
                let sample = object.mono_pcm[frame_index];
                self.interleaved
                    .push(if sample.is_finite() { sample } else { 0.0 });
            }
        }

        let mut mixed = if source_count == 0 {
            vec![0.0f32; frames * 2]
        } else {
            self.renderer
                .render_source_frame_with_gain_policy(
                    &self.interleaved,
                    &self.sources,
                    None,
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
        assert_ne!(first.omniphony_position(), second.omniphony_position());
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

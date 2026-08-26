//! Construction policy for already-separated causal source lanes.
//!
//! This is intentionally host-agnostic. A foobar component, a native Windows
//! host, or a fixture can all build the same source renderer and therefore use
//! the same Omniphony binaural semantics.
//!
//! Retro VGM Compiler owns source truth. This module chooses the listening
//! presentation. FullSphere is deliberately an immersive remix mode, not a
//! claim that the historical source authored modern rear/height coordinates.

use anyhow::Result;
use bridge_api::{RVbapCartesianDefaults, RVbapTableMode};
use renderer::binaural::HrirSource;
use renderer::config::RenderConfig;
use renderer::live_params::{BinauralMode, OutputMode, RampMode};
use renderer::source_frame::SourceFrameRenderer;
use renderer::source_scene::{SharedWetPresentationPolicy, SourcePresentationPolicy};
use renderer::speaker_layout::SpeakerLayout;

use crate::renderer_build::{EvalMode, SpatialRendererParams, build_spatial_renderer};

const SOURCE_SHELL_LAYOUT: &str =
    include_str!("../../../layouts/system-h-derived-22.0-upper60-grid10.yaml");
/// Five precomputed size states (0, .25, .5, .75, 1) are enough for smooth
/// per-object extent while keeping the tiny 4x4x4 source grid cheap to build.
const SOURCE_SIZE_INTERVALS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceSpatialMode {
    /// Preserve native laterality and stable source identity, but do not add
    /// creative rear/height/depth/extent. This is a source-aware control above
    /// the protected historical/reference mix, not a second renderer topology.
    NativeRouting,
    /// Mix recovered real sources into Omniphony's full immersive field. Native
    /// route and authored geometry remain constraints; otherwise width, depth,
    /// height and extent are explicitly DERIVED production decisions.
    FullSphere,
}

impl SourceSpatialMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NativeRouting => "native_routing",
            Self::FullSphere => "full_sphere",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceRendererOptions {
    pub mode: SourceSpatialMode,
    /// Listening-room early reflections are an externalization control, not
    /// part of source geometry. Keeping this independent lets a listener test
    /// full sphere dry versus the same exact scene with room cues.
    pub externalization: bool,
    /// Measured SAF/KEMAR is the default and can later be replaced by a
    /// listener-specific SOFA set without changing the source-scene contract.
    pub hrir_source: HrirSource,
    /// Metres represented by one ADM unit for binaural distance cues.
    pub unit_scale_m: f32,
    /// Preserve host-supplied metric object radius by rendering object lanes
    /// directly to the ears. False keeps the accepted cascaded source path.
    pub authored_metric_objects: bool,
    /// Early-reflection return level when externalization is enabled.
    pub reflection_level: f32,
    /// Small listening-room dimensions for externalization, not source reverb.
    pub reflection_room_size_m: [f32; 3],
}

impl Default for SourceRendererOptions {
    fn default() -> Self {
        Self {
            mode: SourceSpatialMode::FullSphere,
            externalization: false,
            hrir_source: HrirSource::SafKemar,
            unit_scale_m: 1.0,
            authored_metric_objects: false,
            reflection_level: 0.22,
            reflection_room_size_m: [4.0, 5.0, 2.7],
        }
    }
}

pub fn source_presentation_policy(mode: SourceSpatialMode) -> SourcePresentationPolicy {
    match mode {
        SourceSpatialMode::NativeRouting => SourcePresentationPolicy {
            sphere_strength: 0.0,
            max_rear_azimuth_deg: 100.0,
            max_elevation_deg: 0.0,
            max_distance: 1.0,
            // The historical wet field still exists in the source mix, but the
            // control mode adds no modern field scale, height, depth, or extent.
            shared_wet: SharedWetPresentationPolicy {
                strength: 0.0,
                rear_azimuth_deg: 100.0,
                elevation_deg: 0.0,
                distance: 1.0,
                extent: [0.0, 0.0, 0.0],
            },
        },
        SourceSpatialMode::FullSphere => SourcePresentationPolicy {
            // FullSphere intentionally opens the source-native remix rather than
            // waiting for historical proof of a speaker coordinate that the old
            // format could never encode.
            sphere_strength: 1.0,
            // Dynamic source objects may live well behind the listener while
            // leaving a margin around the exact rear singularity.
            max_rear_azimuth_deg: 150.0,
            // Strong enough to create an unmistakable upper hemisphere. Musical
            // role and native routing still shape where each source actually goes.
            max_elevation_deg: 60.0,
            max_distance: 1.75,
            // Historical shared effects, especially S-DSP echo, form their own
            // environmental layer. Keep it wide and rearward but slightly below
            // the dry-object maximums so the direct musical scene remains legible.
            shared_wet: SharedWetPresentationPolicy {
                strength: 1.0,
                rear_azimuth_deg: 140.0,
                elevation_deg: 38.0,
                distance: 1.60,
                extent: [1.0, 0.95, 0.85],
            },
        },
    }
}

/// Both source-aware modes intentionally share one physical renderer topology.
/// This keeps the ABI mode switch a policy change rather than a hidden renderer
/// swap, and makes NativeRouting ↔ FullSphere comparisons isolate geometry and
/// extent rather than direct-vs-cascaded binaural differences.
fn source_layout() -> Result<SpeakerLayout> {
    SpeakerLayout::from_yaml_str(SOURCE_SHELL_LAYOUT)
}

fn binaural_mode(options: &SourceRendererOptions) -> BinauralMode {
    if options.authored_metric_objects {
        BinauralMode::Direct
    } else {
        BinauralMode::Cascaded
    }
}

fn source_render_config(render_cfg: Option<&RenderConfig>) -> Option<RenderConfig> {
    let mut cfg = render_cfg.cloned().unwrap_or_default();
    // The shared source topology must be ready for a runtime switch from
    // NativeRouting to FullSphere without rebuilding the processor. Precomputed
    // evaluators otherwise freeze event_size at the build-time zero-sized
    // request, so both modes carry the same extent-capable tables.
    cfg.render_evaluation_mode = Some("precomputed_cartesian".to_string());
    cfg.evaluation_object_size_intervals = Some(
        cfg.evaluation_object_size_intervals
            .unwrap_or(SOURCE_SIZE_INTERVALS)
            .max(SOURCE_SIZE_INTERVALS),
    );
    Some(cfg)
}

/// Build the source-aware Omniphony renderer used by game-music integrations.
///
/// Both listening modes use the same embedded 22-direction System-H-derived
/// shell and cascaded binaural stage. `NativeRouting` closes creative geometry
/// through `SourcePresentationPolicy`; `FullSphere` opens the same renderer's
/// width/depth/height/extent budget. This makes runtime mode changes coherent
/// and avoids treating a renderer swap as part of the spatial effect.
pub fn build_source_frame_renderer(
    sample_rate: u32,
    render_cfg: Option<&RenderConfig>,
    options: SourceRendererOptions,
) -> Result<SourceFrameRenderer> {
    let source_cfg = source_render_config(render_cfg);
    let effective_render_cfg = source_cfg.as_ref();
    let mut params = SpatialRendererParams::from_render_config(effective_render_cfg);

    // The shared source stage needs a closed 3-D panning field so FullSphere can
    // spend object size over the shell. NativeRouting uses the same tables with
    // creative extent set to zero.
    params.render_evaluation_mode = Some(EvalMode::Cartesian);
    params.evaluation_mode_explicit = true;
    params.evaluation_cartesian_x_size = Some(4);
    params.evaluation_cartesian_y_size = Some(4);
    params.evaluation_cartesian_z_size = Some(4);
    params.evaluation_cartesian_z_neg_size = Some(4);
    params.vbap_allow_negative_z = true;
    params.no_vbap_allow_negative_z = false;

    let defaults = RVbapCartesianDefaults {
        x_size: 4,
        y_size: 4,
        z_size: 4,
        allow_negative_z: true,
    };
    let layout = source_layout()?;
    let mut renderer = build_spatial_renderer(
        &params,
        layout,
        sample_rate,
        defaults,
        RVbapTableMode::Cartesian,
        effective_render_cfg,
    )?;

    // The shared shell uses the same bounded partial inverse of common
    // SAF/KEMAR diffuse colour as Current support. Do not apply a SAF-specific
    // correction to synthetic or future listener HRIRs.
    renderer.set_cascade_spectral_compensation(matches!(
        &options.hrir_source,
        HrirSource::SafKemar
    ));

    {
        let control = renderer.renderer_control();
        let mut live = control.live.write();
        live.binaural.output_mode = OutputMode::Binaural;
        live.binaural.mode = binaural_mode(&options);
        live.binaural.hrir_source = options.hrir_source;
        live.binaural.unit_scale_m = options.unit_scale_m.clamp(0.25, 4.0);
        live.binaural.air_absorption = true;
        live.binaural.near_field_parallax = options.authored_metric_objects;
        live.binaural.reverb.enabled = false;
        live.binaural.reflections.enabled = options.externalization;
        live.binaural.reflections.level = options.reflection_level.clamp(0.0, 1.0);
        live.binaural.reflections.room_size_m = [
            options.reflection_room_size_m[0].max(1.0),
            options.reflection_room_size_m[1].max(1.0),
            options.reflection_room_size_m[2].max(1.0),
        ];
        // Source timing remains host-owned. The object positions and sizes ramp
        // at frame granularity before the speaker stage spends them over the
        // shell, preventing callback boundaries from becoming audible geometry.
        live.ramp_mode = RampMode::Frame;
    }

    Ok(SourceFrameRenderer::new(
        renderer,
        source_presentation_policy(options.mode),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use renderer::source_scene::SourceSceneEvidence;

    #[test]
    fn native_mode_disables_creative_depth_height_and_wet_scale() {
        let policy = source_presentation_policy(SourceSpatialMode::NativeRouting);
        assert_eq!(policy.sphere_strength, 0.0);
        assert_eq!(policy.max_elevation_deg, 0.0);
        assert_eq!(policy.max_distance, 1.0);
        assert_eq!(policy.shared_wet.strength, 0.0);
        assert_eq!(policy.shared_wet.extent, [0.0, 0.0, 0.0]);
        assert_eq!(binaural_mode(&SourceRendererOptions::default()), BinauralMode::Cascaded);
    }

    #[test]
    fn full_sphere_opens_immersive_rear_height_depth_and_wet_layer() {
        let policy = source_presentation_policy(SourceSpatialMode::FullSphere);
        assert_eq!(policy.sphere_strength, 1.0);
        assert!(policy.max_rear_azimuth_deg > 135.0);
        assert!(policy.max_elevation_deg >= 55.0);
        assert!(policy.max_distance > 1.5);
        assert!(policy.shared_wet.strength > 0.9);
        assert!(policy.shared_wet.rear_azimuth_deg > 120.0);
        assert!(policy.shared_wet.elevation_deg > 25.0);
        assert!(policy.shared_wet.distance > 1.4);
        assert!(policy.shared_wet.extent[0] > policy.shared_wet.extent[2]);
        assert_eq!(binaural_mode(&SourceRendererOptions::default()), BinauralMode::Cascaded);
    }

    #[test]
    fn authored_metric_objects_use_direct_binaural_geometry() {
        let options = SourceRendererOptions {
            authored_metric_objects: true,
            ..SourceRendererOptions::default()
        };
        assert_eq!(binaural_mode(&options), BinauralMode::Direct);
        assert!(options.authored_metric_objects);
    }

    #[test]
    fn both_modes_share_the_current_22_direction_shell() {
        let layout = source_layout().expect("embedded shell");
        assert_eq!(layout.num_speakers(), 22);
        assert!(layout.speakers.iter().all(|speaker| speaker.spatialize));
        assert!(layout.speakers.iter().any(|speaker| speaker.name == "TpC"));
        assert!(layout.speakers.iter().any(|speaker| speaker.name == "BC"));
        assert!(layout.speakers.iter().any(|speaker| speaker.name == "BtFC"));
    }

    #[test]
    fn shared_topology_precomputes_dynamic_extent_states() {
        let cfg = source_render_config(None).expect("source path owns an internal render config");
        assert_eq!(cfg.render_evaluation_mode.as_deref(), Some("precomputed_cartesian"));
        assert_eq!(cfg.evaluation_object_size_intervals, Some(SOURCE_SIZE_INTERVALS));

        let mut supplied = RenderConfig::default();
        supplied.evaluation_object_size_intervals = Some(8);
        let kept = source_render_config(Some(&supplied)).expect("source config");
        assert_eq!(kept.evaluation_object_size_intervals, Some(8));
    }

    #[test]
    fn full_sphere_extent_changes_headphone_audio_without_moving_source_center() {
        const SAMPLE_RATE: u32 = 48_000;
        const FRAMES: usize = 2_048;
        let mut renderer = build_source_frame_renderer(
            SAMPLE_RATE,
            None,
            SourceRendererOptions {
                hrir_source: HrirSource::Synthetic,
                ..SourceRendererOptions::default()
            },
        )
        .expect("FullSphere renderer");

        let input: Vec<f32> = (0..FRAMES)
            .map(|i| {
                let t = i as f32 / SAMPLE_RATE as f32;
                0.07 * (std::f32::consts::TAU * 997.0 * t).sin()
                    + 0.03 * (std::f32::consts::TAU * 2_113.0 * t).sin()
            })
            .collect();
        let center = [0.45, 0.85, 0.25];
        let point = SourceSceneEvidence {
            source_id: 77,
            authored_position: Some(center),
            width: 0.0,
            diffuse: 0.0,
            confidence: 1.0,
            ..SourceSceneEvidence::default()
        };
        let wide = SourceSceneEvidence {
            width: 1.0,
            diffuse: 1.0,
            ..point
        };

        let point_out = renderer
            .render_source_frame(&input, &[point], 0, 0, Vec::new(), false)
            .expect("point render")
            .samples;
        renderer.reset_runtime_state();
        let wide_out = renderer
            .render_source_frame(&input, &[wide], 0, 0, Vec::new(), false)
            .expect("wide render")
            .samples;

        assert_eq!(point_out.len(), FRAMES * 2);
        assert_eq!(wide_out.len(), point_out.len());
        assert!(point_out.iter().chain(&wide_out).all(|sample| sample.is_finite()));
        let delta_rms = (point_out
            .iter()
            .zip(&wide_out)
            .map(|(point, wide)| (point - wide) * (point - wide))
            .sum::<f32>()
            / point_out.len() as f32)
            .sqrt();
        assert!(
            delta_rms > 1.0e-5,
            "object extent must alter cascaded headphone audio; delta_rms={delta_rms}"
        );
    }

    #[test]
    fn externalization_defaults_off_so_geometry_can_be_tested_alone() {
        let options = SourceRendererOptions::default();
        assert_eq!(options.mode, SourceSpatialMode::FullSphere);
        assert!(!options.externalization);
    }
}

from pathlib import Path

engine = Path('omniphony-renderer/orender_engine/src/engine.rs')
s = engine.read_text()
s = s.replace('    bridge: LoadedBridge,\n', '    bridge: Option<LoadedBridge>,\n', 1)

old = '''    pub fn new(bridge: LoadedBridge, renderer: SpatialRenderer, sample_rate: u32) -> Self {
        let coordinate_format = bridge.bridge.coordinate_format();
        let engine = Self {
            bridge,
            renderer,
            sample_rate,
            coordinate_format,
            fixed_planner: virtual_bed::FixedChannelPlanner::new(),
            object_channels: Vec::new(),
            has_objects: false,
            loudness_applied: false,
            decoded_samples: 0,
            last_object_count: 0,
            last_dialnorm: None,
            last_bed_labels: Vec::new(),
            drc_gain: 1.0,
            drc_target_gain: 1.0,
            drc_ramp_samples_remaining: 0,
            applied_drc_mode: String::new(),
            frame_events: Vec::new(),
            pcm_f32_buf: Vec::new(),
            osc: None,
            audio_meter: None,
            object_names: HashMap::new(),
            perf: std::env::var_os("ORENDER_PERF_LOG")
                .is_some()
                .then(PerfLog::new),
            object_gen: object_gen::ObjectGenStage::new(),
            phantom: phantom_extract::PhantomExtractStage::new(),
            fixed_processing_sig: None,
        };
        engine
            .renderer
            .renderer_control()
            .set_fixed_channel_catalog(virtual_bed::fixed_channel_catalog_json());
        engine
    }
'''
new = '''    pub fn new(bridge: LoadedBridge, renderer: SpatialRenderer, sample_rate: u32) -> Self {
        let coordinate_format = bridge.bridge.coordinate_format();
        Self::new_inner(Some(bridge), renderer, sample_rate, coordinate_format)
    }

    /// Build the render half of the engine for callers that already own decoded
    /// semantic frames. Current uses this crate-private seam so in-process PCM
    /// does not masquerade as a decoder plugin. Compressed-media hosts continue
    /// to use [`Engine::new`] / [`Engine::process`] unchanged.
    pub(crate) fn new_decoded(
        renderer: SpatialRenderer,
        sample_rate: u32,
        coordinate_format: RCoordinateFormat,
    ) -> Self {
        Self::new_inner(None, renderer, sample_rate, coordinate_format)
    }

    fn new_inner(
        bridge: Option<LoadedBridge>,
        renderer: SpatialRenderer,
        sample_rate: u32,
        coordinate_format: RCoordinateFormat,
    ) -> Self {
        let engine = Self {
            bridge,
            renderer,
            sample_rate,
            coordinate_format,
            fixed_planner: virtual_bed::FixedChannelPlanner::new(),
            object_channels: Vec::new(),
            has_objects: false,
            loudness_applied: false,
            decoded_samples: 0,
            last_object_count: 0,
            last_dialnorm: None,
            last_bed_labels: Vec::new(),
            drc_gain: 1.0,
            drc_target_gain: 1.0,
            drc_ramp_samples_remaining: 0,
            applied_drc_mode: String::new(),
            frame_events: Vec::new(),
            pcm_f32_buf: Vec::new(),
            osc: None,
            audio_meter: None,
            object_names: HashMap::new(),
            perf: std::env::var_os("ORENDER_PERF_LOG")
                .is_some()
                .then(PerfLog::new),
            object_gen: object_gen::ObjectGenStage::new(),
            phantom: phantom_extract::PhantomExtractStage::new(),
            fixed_processing_sig: None,
        };
        engine
            .renderer
            .renderer_control()
            .set_fixed_channel_catalog(virtual_bed::fixed_channel_catalog_json());
        engine
    }
'''
assert old in s, 'Engine::new baseline drifted'
s = s.replace(old, new, 1)

s = s.replace(
    '        self.bridge.bridge.has_objects()\n',
    '        self.bridge\n            .as_ref()\n            .is_some_and(|bridge| bridge.bridge.has_objects())\n            || self.has_objects\n',
    1,
)
s = s.replace(
    '        self.bridge.bridge.reset();\n',
    '        if let Some(bridge) = self.bridge.as_mut() {\n            bridge.bridge.reset();\n        }\n',
    1,
)
old = '''        self.bridge.bridge.set_drc_mode(live_mode.as_str().into());
        self.applied_drc_mode = live_mode;
'''
new = '''        if let Some(bridge) = self.bridge.as_mut() {
            bridge.bridge.set_drc_mode(live_mode.as_str().into());
            self.applied_drc_mode = live_mode;
        }
'''
assert old in s, 'sync_drc_mode baseline drifted'
s = s.replace(old, new, 1)

old = '''        let result = self
            .bridge
            .bridge
            .push_packet(data.into(), transport, data_type);
'''
new = '''        let result = self
            .bridge
            .as_mut()
            .ok_or_else(|| anyhow!("decoded-frame engine has no packet decoder"))?
            .bridge
            .push_packet(data.into(), transport, data_type);
'''
assert old in s, 'Engine::process bridge call baseline drifted'
s = s.replace(old, new, 1)

marker = '''    /// Convenience wrapper for hosts that always feed raw access units.
    pub fn process_raw(&mut self, data: &[u8]) -> Result<Vec<RenderedAudio>> {
        self.process(data, RInputTransport::Raw, 0)
    }

'''
insert = marker + '''    /// Render semantic decoder output without routing it back through a decoder
    /// bridge. Crate-private by design: public compressed-media hosts still use
    /// `process`, while Current can enter at the representation it already owns.
    pub(crate) fn process_decoded_frames(
        &mut self,
        frames: &[RDecodedFrame],
    ) -> Result<Vec<RenderedAudio>> {
        let mut out = Vec::with_capacity(frames.len());
        for frame in frames {
            if let Some(chunk) = self.render_frame(frame, 0.0)? {
                out.push(chunk);
            }
        }
        Ok(out)
    }

'''
assert marker in s, 'process_raw baseline drifted'
s = s.replace(marker, insert, 1)
engine.write_text(s)

music = Path('omniphony-renderer/orender_engine/src/music_support.rs')
m = music.read_text()
enum_at = m.index('/// The normal Windows host has one listening model.')
imports = '''#[cfg(test)]
use crate::bridge_loader::LoadedBridge;
use crate::music_early_reflections::HrtfEarlyReflectionField;
use crate::renderer_build::{SpatialRendererParams, build_spatial_renderer};
use crate::{Engine, RenderedAudio};
use abi_stable::std_types::{ROption, RVec};
use anyhow::{Context, bail};
use bridge_api::{
    RChannelLabel, RCoordinateFormat, RDecodedFrame, RVbapCartesianDefaults, RVbapTableMode,
};
#[cfg(test)]
use bridge_api::RInputTransport;
use renderer::config::Config;
use renderer::music_field::MUSIC_FIELD_CHANNELS;
use renderer::speaker_layout::SpeakerLayout;
use std::path::PathBuf;

const CURRENT_PCM_BLOCK_FRAMES: usize = 2048;
const CURRENT_PCM_FULL_SCALE: f32 = 8_388_607.0;
const CURRENT_CHANNEL_LABELS: [RChannelLabel; MUSIC_FIELD_CHANNELS] = [
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

'''
m = imports + m[enum_at:]

adapter_start = m.index('/// Narrow in-process PCM adapter')
renderer_start = m.index('pub(crate) struct MusicSupportRenderer')
frame_helpers = '''#[inline]
fn quantize_current_pcm(sample: f32) -> i32 {
    if sample.is_finite() {
        (sample.clamp(-1.0, 1.0) * CURRENT_PCM_FULL_SCALE) as i32
    } else {
        0
    }
}

fn current_decoded_frames(field_input: &[f32], sample_rate_hz: u32) -> Vec<RDecodedFrame> {
    let total_frames = field_input.len() / MUSIC_FIELD_CHANNELS;
    let mut frames = Vec::with_capacity(total_frames.div_ceil(CURRENT_PCM_BLOCK_FRAMES));
    for frame_start in (0..total_frames).step_by(CURRENT_PCM_BLOCK_FRAMES) {
        let n_frames = (total_frames - frame_start).min(CURRENT_PCM_BLOCK_FRAMES);
        let sample_start = frame_start * MUSIC_FIELD_CHANNELS;
        let sample_end = sample_start + n_frames * MUSIC_FIELD_CHANNELS;
        let mut pcm = RVec::with_capacity(n_frames * MUSIC_FIELD_CHANNELS);
        pcm.extend(
            field_input[sample_start..sample_end]
                .iter()
                .copied()
                .map(quantize_current_pcm),
        );
        frames.push(RDecodedFrame {
            sampling_frequency: sample_rate_hz,
            sample_count: n_frames as u32,
            channel_count: MUSIC_FIELD_CHANNELS as u32,
            pcm,
            channel_labels: RVec::from(CURRENT_CHANNEL_LABELS.to_vec()),
            metadata: RVec::new(),
            drc_gain: 1.0,
            drc_ramp_duration: 0,
            dialogue_level: ROption::RNone,
            is_new_segment: false,
        });
    }
    frames
}

'''
m = m[:adapter_start] + frame_helpers + m[renderer_start:]

old = '''        let primary = build_embedded_engine_with_bridge(
            &current_config,
            GRID_LAYOUT,
            sample_rate_hz,
            "Current model support",
            current_pcm_bridge(sample_rate_hz),
        )?;
'''
new = '''        let primary = build_embedded_decoded_engine(
            &current_config,
            GRID_LAYOUT,
            sample_rate_hz,
            "Current model support",
        )?;
'''
assert old in m, 'production builder baseline drifted'
m = m.replace(old, new, 1)

old = '''        let primary = self
            .primary
            .process(f32_as_native_bytes(field_input), RInputTransport::Raw, 0)
            .context("Current model music support render failed")?;
'''
new = '''        let frames = current_decoded_frames(field_input, self.primary.sample_rate());
        let primary = self
            .primary
            .process_decoded_frames(&frames)
            .context("Current model music support render failed")?;
'''
assert old in m, 'production process baseline drifted'
m = m.replace(old, new, 1)

old = '''    build_embedded_engine_with_bridge(config_yaml, layout_yaml, sample_rate_hz, label, bridge)
}

/// Build the embedded Current renderer around a caller-supplied decoded-frame
/// ingress. The acoustic renderer and all Current presentation laws live below
/// this seam; changing ingress must therefore null before the oracle is retired.
fn build_embedded_engine_with_bridge(
    config_yaml: &str,
    layout_yaml: &str,
    sample_rate_hz: u32,
    label: &str,
    bridge: LoadedBridge,
) -> anyhow::Result<Engine> {
'''
new = '''    build_embedded_engine_with_ingress(
        config_yaml,
        layout_yaml,
        sample_rate_hz,
        label,
        Some(bridge),
    )
}

fn build_embedded_decoded_engine(
    config_yaml: &str,
    layout_yaml: &str,
    sample_rate_hz: u32,
    label: &str,
) -> anyhow::Result<Engine> {
    build_embedded_engine_with_ingress(config_yaml, layout_yaml, sample_rate_hz, label, None)
}

fn build_embedded_engine_with_ingress(
    config_yaml: &str,
    layout_yaml: &str,
    sample_rate_hz: u32,
    label: &str,
    bridge: Option<LoadedBridge>,
) -> anyhow::Result<Engine> {
'''
assert old in m, 'embedded builder baseline drifted'
m = m.replace(old, new, 1)

old = '''    let vbap_defaults = bridge.vbap_cartesian_defaults();
    let preferred = bridge.preferred_vbap_table_mode();
'''
new = '''    let (vbap_defaults, preferred) = if let Some(bridge) = bridge.as_ref() {
        (bridge.vbap_cartesian_defaults(), bridge.preferred_vbap_table_mode())
    } else {
        (
            RVbapCartesianDefaults {
                x_size: 62,
                y_size: 62,
                z_size: 15,
                allow_negative_z: false,
            },
            RVbapTableMode::Cartesian,
        )
    };
'''
assert old in m, 'VBAP baseline drifted'
m = m.replace(old, new, 1)

old = '''    let supported_drc: Vec<String> = bridge
        .bridge
        .supported_drc_modes()
        .iter()
        .map(|mode| mode.as_str().to_string())
        .collect();
'''
new = '''    let supported_drc: Vec<String> = bridge
        .as_ref()
        .map(|bridge| {
            bridge
                .bridge
                .supported_drc_modes()
                .iter()
                .map(|mode| mode.as_str().to_string())
                .collect()
        })
        .unwrap_or_default();
'''
assert old in m, 'DRC baseline drifted'
m = m.replace(old, new, 1)

old = '    let engine = Engine::new(bridge, spatial_renderer, sample_rate_hz);\n'
new = '''    let engine = if let Some(bridge) = bridge {
        Engine::new(bridge, spatial_renderer, sample_rate_hz)
    } else {
        Engine::new_decoded(spatial_renderer, sample_rate_hz, RCoordinateFormat::Cartesian)
    };
'''
assert old in m, 'engine construction baseline drifted'
m = m.replace(old, new, 1)

old = '''        let mut direct = build_embedded_engine_with_bridge(
            &current_config,
            GRID_LAYOUT,
            SAMPLE_RATE,
            "direct Current ingress",
            current_pcm_bridge(SAMPLE_RATE),
        )
        .unwrap();
'''
new = '''        let mut direct = build_embedded_decoded_engine(
            &current_config,
            GRID_LAYOUT,
            SAMPLE_RATE,
            "decoded-frame Current ingress",
        )
        .unwrap();
'''
assert old in m, 'test challenger baseline drifted'
m = m.replace(old, new, 1)

old = '''        let direct_out = direct
            .process(f32_as_native_bytes(&input), RInputTransport::Raw, 0)
            .unwrap();
'''
new = '''        let frames = current_decoded_frames(&input, SAMPLE_RATE);
        let direct_out = direct.process_decoded_frames(&frames).unwrap();
'''
assert old in m, 'test process baseline drifted'
m = m.replace(old, new, 1)
music.write_text(m)

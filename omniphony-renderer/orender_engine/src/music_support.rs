use crate::bridge_loader::LoadedBridge;
use crate::music_early_reflections::HrtfEarlyReflectionField;
use crate::renderer_build::{SpatialRendererParams, build_spatial_renderer};
use crate::{Engine, RenderedAudio};
use abi_stable::{
    sabi_trait::prelude::TD_Opaque,
    std_types::{ROption, RSlice, RStr, RString, RVec},
};
use anyhow::{Context, bail};
use bridge_api::{
    FormatBridge, FormatBridgeBox, FormatBridge_TO, RChannelLabel, RCoordinateFormat,
    RDecodedFrame, RInputTransport, RPushResult, RVbapCartesianDefaults, RVbapTableMode,
};
use renderer::config::Config;
use renderer::music_field::MUSIC_FIELD_CHANNELS;
use renderer::speaker_layout::SpeakerLayout;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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

/// The normal Windows host has one listening model.
///
/// Historical profile experiments are preserved in git history and
/// `docs/listening-history.md`, but they are not runtime product modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpatialProfile {
    Current,
}

pub(crate) fn current_model_config(base: &str) -> String {
    let mut cfg = base.to_string();
    cfg = cfg.replace("      level: 0.32", "      level: 0.36");
    // Keep the transient-aware measured-HRTF early field intact, but reduce the
    // low-level late closure after listening found the center slightly too wet.
    // Spatial scale should come from geometry and early directional evidence,
    // leaving centered vocals anchored in the protected master.
    cfg = cfg.replace("      level: 0.028", "      level: 0.016");
    cfg = cfg.replace("      rt60_s: 0.16", "      rt60_s: 0.12");

    // The Current model owns first-order reflections in the fixed-cost six-bus
    // measured-HRTF field below, so disable the inherited analytic reflection
    // bank to prevent duplicate early energy.
    cfg.replace(
        "    reflections:\n      enabled: true",
        "    reflections:\n      enabled: false",
    )
}

/// Shared-memory ingress used only inside the retained Current worker.
///
/// The old path converted every 17-lane f32 block to little-endian bytes, pushed
/// those bytes through a streaming RIFF/WAVE parser, quantized them into the
/// bridge's 24-bit-in-i32 PCM convention, then converted them back to f32 in the
/// engine. This ingress presents the same decoded frames directly while keeping
/// the established 24-bit clamp/quantization law and 2048-frame boundaries.
///
/// The mutex is not on the Windows audio callback: Current already runs on its
/// dedicated worker. It is a narrow ownership adapter around Engine's bridge
/// abstraction and can disappear with the later Engine→SpatialRenderer
/// contraction.
#[derive(Clone, Default)]
struct CurrentPcmIngress {
    pending: Arc<Mutex<Vec<f32>>>,
}

impl CurrentPcmIngress {
    fn queue(&self, samples: &[f32]) -> anyhow::Result<()> {
        if !samples.len().is_multiple_of(MUSIC_FIELD_CHANNELS) {
            bail!(
                "Current model PCM width mismatch: {} samples are not divisible by {} lanes",
                samples.len(),
                MUSIC_FIELD_CHANNELS
            );
        }
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if !pending.is_empty() {
            bail!("previous Current model PCM block was not consumed");
        }
        pending.extend_from_slice(samples);
        Ok(())
    }
}

struct CurrentPcmBridge {
    ingress: CurrentPcmIngress,
    sample_rate_hz: u32,
    scratch: Vec<f32>,
    frames_emitted: u64,
}

impl CurrentPcmBridge {
    fn new(ingress: CurrentPcmIngress, sample_rate_hz: u32) -> Self {
        Self {
            ingress,
            sample_rate_hz,
            scratch: Vec::new(),
            frames_emitted: 0,
        }
    }

    #[inline]
    fn quantize(sample: f32) -> i32 {
        // Match reference_bridge::wav::SampleFormat::F32 exactly. Native-float
        // Current is a separate fidelity experiment, not part of this refactor.
        if sample.is_finite() {
            (sample.clamp(-1.0, 1.0) * CURRENT_PCM_FULL_SCALE) as i32
        } else {
            0
        }
    }
}

impl FormatBridge for CurrentPcmBridge {
    fn push_packet(
        &mut self,
        _data: RSlice<'_, u8>,
        _transport: RInputTransport,
        _data_type: u8,
    ) -> RPushResult {
        {
            let mut pending = self
                .ingress
                .pending
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            std::mem::swap(&mut self.scratch, &mut *pending);
        }

        let mut result = RPushResult {
            frames: RVec::new(),
            error_message: RString::new(),
            did_reset: false,
        };
        if self.scratch.is_empty() {
            return result;
        }
        if !self.scratch.len().is_multiple_of(MUSIC_FIELD_CHANNELS) {
            result.error_message = RString::from("direct Current PCM width mismatch");
            self.scratch.clear();
            return result;
        }

        let total_frames = self.scratch.len() / MUSIC_FIELD_CHANNELS;
        for frame_start in (0..total_frames).step_by(CURRENT_PCM_BLOCK_FRAMES) {
            let n_frames = (total_frames - frame_start).min(CURRENT_PCM_BLOCK_FRAMES);
            let sample_start = frame_start * MUSIC_FIELD_CHANNELS;
            let sample_end = sample_start + n_frames * MUSIC_FIELD_CHANNELS;
            let mut pcm = RVec::with_capacity(sample_end - sample_start);
            for &sample in &self.scratch[sample_start..sample_end] {
                pcm.push(Self::quantize(sample));
            }
            result.frames.push(RDecodedFrame {
                sampling_frequency: self.sample_rate_hz,
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
        self.frames_emitted += total_frames as u64;
        self.scratch.clear();
        result
    }

    fn reset(&mut self) {
        self.scratch.clear();
        self.ingress
            .pending
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clear();
        self.frames_emitted = 0;
    }

    fn is_ready(&self) -> bool {
        self.frames_emitted > 0
    }

    fn has_objects(&self) -> bool {
        false
    }

    fn configure(&mut self, key: RStr<'_>, _value: RStr<'_>) -> bool {
        key.as_str() == "presentation"
    }

    fn coordinate_format(&self) -> RCoordinateFormat {
        RCoordinateFormat::Cartesian
    }

    fn vbap_cartesian_defaults(&self) -> RVbapCartesianDefaults {
        // Exact reference-bridge defaults. Keeping these stable makes renderer
        // construction independent of which ingress carries the same PCM.
        RVbapCartesianDefaults {
            x_size: 62,
            y_size: 62,
            z_size: 15,
            allow_negative_z: false,
        }
    }

    fn preferred_vbap_table_mode(&self) -> RVbapTableMode {
        RVbapTableMode::Cartesian
    }

    fn supported_drc_modes(&self) -> RVec<RString> {
        RVec::new()
    }

    fn set_drc_mode(&mut self, _mode: RStr<'_>) -> bool {
        false
    }
}

fn current_pcm_bridge(ingress: CurrentPcmIngress, sample_rate_hz: u32) -> LoadedBridge {
    // Keep a valid ABI root-module owner alongside the in-process bridge object;
    // no reference-bridge decoder instance is created or used for Current PCM.
    let lib = reference_bridge::linked_library();
    let bridge: FormatBridgeBox =
        FormatBridge_TO::from_value(CurrentPcmBridge::new(ingress, sample_rate_hz), TD_Opaque);
    LoadedBridge { lib, bridge }
}

pub(crate) struct MusicSupportRenderer {
    primary: Engine,
    early_reflections: HrtfEarlyReflectionField,
    pcm_ingress: CurrentPcmIngress,
}

impl MusicSupportRenderer {
    pub(crate) fn new(_profile: SpatialProfile, sample_rate_hz: u32) -> anyhow::Result<Self> {
        const FIELD_CONFIG: &str =
            include_str!("../../assets/binaural-baselines/stereo-field-prototype.yaml");
        const GRID_LAYOUT: &str =
            include_str!("../../../layouts/system-h-derived-22.0-upper60-grid10.yaml");

        // Current is fully embedded. Do not materialize YAML into LOCALAPPDATA:
        // this constructor is also used by the native endpoint realtime worker,
        // which must not depend on a writable interactive-user profile.
        let current_config = current_model_config(FIELD_CONFIG);
        let pcm_ingress = CurrentPcmIngress::default();
        let primary = build_embedded_engine_with_bridge(
            &current_config,
            GRID_LAYOUT,
            sample_rate_hz,
            "Current model support",
            current_pcm_bridge(pcm_ingress.clone(), sample_rate_hz),
        )?;
        let early_reflections = HrtfEarlyReflectionField::new(sample_rate_hz);

        Ok(Self {
            primary,
            early_reflections,
            pcm_ingress,
        })
    }

    pub(crate) fn process(&mut self, field_input: &[f32]) -> anyhow::Result<Vec<RenderedAudio>> {
        self.pcm_ingress.queue(field_input)?;
        let primary = self
            .primary
            .process(&[], RInputTransport::Raw, 0)
            .context("Current model music support render failed")?;
        let early = self.early_reflections.process(field_input)?;
        add_stereo_support(primary, &early)
    }
}

/// Construct the legacy Current support engine around the linked reference WAV
/// bridge. Retained as the differential oracle while the direct ingress ships.
pub(crate) fn build_embedded_engine(
    config_yaml: &str,
    layout_yaml: &str,
    sample_rate_hz: u32,
    label: &str,
) -> anyhow::Result<Engine> {
    let mut bridge = LoadedBridge::from_library(reference_bridge::linked_library());
    bridge.configure("presentation", "best");
    build_embedded_engine_with_bridge(config_yaml, layout_yaml, sample_rate_hz, label, bridge)
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
    let mut config: Config = serde_yaml_ng::from_str(config_yaml)
        .with_context(|| format!("failed to parse embedded Omniphony {label} config"))?;
    if let Some(render) = config.render.as_mut() {
        render.normalize_room_meters();
    }
    let render_cfg = config.render;
    let layout = SpeakerLayout::from_yaml_str(layout_yaml)
        .with_context(|| format!("failed to parse embedded Omniphony {label} layout"))?;

    let vbap_defaults = bridge.vbap_cartesian_defaults();
    let preferred = bridge.preferred_vbap_table_mode();

    let params = SpatialRendererParams::from_render_config(render_cfg.as_ref());
    let mut spatial_renderer = build_spatial_renderer(
        &params,
        layout,
        sample_rate_hz,
        vbap_defaults,
        preferred,
        render_cfg.as_ref(),
    )
    .with_context(|| format!("failed to construct Omniphony {label} renderer"))?;

    let cascade_spectral_compensation = render_cfg
        .as_ref()
        .and_then(|render| render.binaural.as_ref())
        .and_then(|binaural| binaural.spectral_compensation.as_deref())
        .is_some_and(|mode| mode.eq_ignore_ascii_case("saf_partial"));
    spatial_renderer.set_cascade_spectral_compensation(cascade_spectral_compensation);

    let control = spatial_renderer.renderer_control();
    control.set_bridge_path(Some(PathBuf::from("<embedded-current-ingress>")));
    control.set_meter_rate_hz(
        render_cfg
            .as_ref()
            .and_then(|c| c.meter_rate)
            .unwrap_or(10.0),
    );
    control.set_diag_rate_hz(
        render_cfg
            .as_ref()
            .and_then(|c| c.diag_rate)
            .unwrap_or(10.0),
    );

    let ramp_mode = render_cfg
        .as_ref()
        .and_then(renderer::config_fields::ramp_mode::get)
        .as_deref()
        .and_then(renderer::live_params::RampMode::from_str)
        .unwrap_or(renderer::live_params::RampMode::Frame);
    control.set_requested_ramp_mode(ramp_mode);
    control.live.write().ramp_mode = ramp_mode;

    if let Some(render) = render_cfg.as_ref() {
        renderer::options::seed_live_from_config(&mut control.live.write(), render);
    }

    let supported_drc: Vec<String> = bridge
        .bridge
        .supported_drc_modes()
        .iter()
        .map(|mode| mode.as_str().to_string())
        .collect();
    control.set_bridge_supported_drc_modes(supported_drc);
    {
        let mut live = control.live.write();
        live.drc_mode = render_cfg
            .as_ref()
            .and_then(|c| c.drc_mode.clone())
            .unwrap_or_else(|| "Off".to_string());
        live.drc_weight = render_cfg
            .as_ref()
            .and_then(|c| c.drc_weight)
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);
    }

    let engine = Engine::new(bridge, spatial_renderer, sample_rate_hz);
    engine.set_channel_render_mode_code(1);
    if engine.channel_count() != 2 {
        bail!(
            "{label} configuration expected 2 output channels but engine reports {}",
            engine.channel_count()
        );
    }
    Ok(engine)
}

pub(crate) fn seed_engine(engine: &mut Engine, header: &[u8], label: &str) -> anyhow::Result<()> {
    let output = engine
        .process(header, RInputTransport::Raw, 0)
        .with_context(|| {
            format!(
                "failed to seed {label} canonical {}-lane PCM bridge",
                MUSIC_FIELD_CHANNELS
            )
        })?;
    if !output.is_empty() {
        bail!("{label} streaming WAV header unexpectedly produced audio");
    }
    Ok(())
}

fn add_stereo_support(
    mut primary: Vec<RenderedAudio>,
    added: &[f32],
) -> anyhow::Result<Vec<RenderedAudio>> {
    let total: usize = primary.iter().map(|block| block.samples.len()).sum();
    if total != added.len() {
        bail!(
            "Current model HRTF early-reflection support length mismatch: renderer={} reflection_field={}",
            total,
            added.len()
        );
    }
    let mut cursor = 0usize;
    for block in &mut primary {
        if block.n_channels != 2 {
            bail!(
                "Current model HRTF early-reflection field expected stereo primary output, got {} channels",
                block.n_channels
            );
        }
        let end = cursor + block.samples.len();
        for (dst, src) in block.samples.iter_mut().zip(&added[cursor..end]) {
            *dst += *src;
        }
        cursor = end;
    }
    Ok(primary)
}

pub(crate) fn streaming_f32_wav_header(channels: u16, sample_rate_hz: u32) -> Vec<u8> {
    let block_align = channels.saturating_mul(4);
    let byte_rate = sample_rate_hz.saturating_mul(u32::from(block_align));
    let mut wav = Vec::with_capacity(44);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&u32::MAX.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&3u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate_hz.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&32u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&u32::MAX.to_le_bytes());
    wav
}

pub(crate) fn f32_as_le_bytes(samples: &[f32], out: &mut Vec<u8>) {
    out.clear();
    out.reserve(samples.len() * 4);
    for &sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current_test_input(frames: usize) -> Vec<f32> {
        let mut out = Vec::with_capacity(frames * MUSIC_FIELD_CHANNELS);
        for frame in 0..frames {
            for channel in 0..MUSIC_FIELD_CHANNELS {
                let phase = frame as f32 * 0.013 + channel as f32 * 0.17;
                let carrier = phase.sin() * 0.55;
                let texture = (phase * 0.37 + channel as f32 * 0.11).cos() * 0.18;
                out.push(carrier + texture);
            }
        }
        // Exercise the reference bridge's exact F32 compatibility law too:
        // finite values clamp to ±1 before 24-bit scaling; non-finite values
        // become zero. The direct challenger must reproduce that separately
        // from any future native-float fidelity experiment.
        out[3] = 1.25;
        out[29] = -1.25;
        out[61] = f32::NAN;
        out[97] = f32::INFINITY;
        out
    }

    #[test]
    fn direct_current_ingress_matches_streaming_reference_bridge() {
        const FIELD_CONFIG: &str =
            include_str!("../../assets/binaural-baselines/stereo-field-prototype.yaml");
        const GRID_LAYOUT: &str =
            include_str!("../../../layouts/system-h-derived-22.0-upper60-grid10.yaml");
        const SAMPLE_RATE: u32 = 48_000;

        let current_config = current_model_config(FIELD_CONFIG);
        let mut legacy = build_embedded_engine(
            &current_config,
            GRID_LAYOUT,
            SAMPLE_RATE,
            "legacy Current ingress",
        )
        .unwrap();
        let header = streaming_f32_wav_header(MUSIC_FIELD_CHANNELS as u16, SAMPLE_RATE);
        seed_engine(&mut legacy, &header, "legacy Current ingress").unwrap();

        let ingress = CurrentPcmIngress::default();
        let mut direct = build_embedded_engine_with_bridge(
            &current_config,
            GRID_LAYOUT,
            SAMPLE_RATE,
            "direct Current ingress",
            current_pcm_bridge(ingress.clone(), SAMPLE_RATE),
        )
        .unwrap();

        let input = current_test_input(CURRENT_PCM_BLOCK_FRAMES + 73);
        let mut bytes = Vec::new();
        f32_as_le_bytes(&input, &mut bytes);
        let legacy_out = legacy.process(&bytes, RInputTransport::Raw, 0).unwrap();
        ingress.queue(&input).unwrap();
        let direct_out = direct.process(&[], RInputTransport::Raw, 0).unwrap();

        assert_eq!(legacy_out.len(), direct_out.len(), "decoded block count changed");
        let mut sum_sq = 0.0f64;
        let mut max_abs = 0.0f32;
        let mut samples = 0usize;
        for (old, new) in legacy_out.iter().zip(&direct_out) {
            assert_eq!(old.n_channels, new.n_channels);
            assert_eq!(old.n_frames, new.n_frames);
            assert_eq!(old.sample_pos, new.sample_pos);
            assert_eq!(old.samples.len(), new.samples.len());
            for (&a, &b) in old.samples.iter().zip(&new.samples) {
                let d = (a - b).abs();
                max_abs = max_abs.max(d);
                sum_sq += (d as f64) * (d as f64);
                samples += 1;
            }
        }
        let rms = (sum_sq / samples.max(1) as f64).sqrt() as f32;
        eprintln!("Current ingress residual: max_abs={max_abs:e} rms={rms:e}");
        assert!(
            max_abs <= 1.0e-6 && rms <= 1.0e-7,
            "direct Current ingress changed the primary render: max_abs={max_abs:e} rms={rms:e}"
        );
    }
}

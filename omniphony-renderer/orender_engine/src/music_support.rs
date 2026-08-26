use crate::bridge_loader::LoadedBridge;
use crate::music_early_reflections::HrtfEarlyReflectionField;
use crate::music_late_enclosure::HrtfLateEnclosure;
use crate::renderer_build::{SpatialRendererParams, build_spatial_renderer};
use crate::{Engine, RenderedAudio};
use anyhow::{Context, bail};
use bridge_api::RInputTransport;
use renderer::config::Config;
use renderer::music_field::MUSIC_FIELD_CHANNELS;
use renderer::speaker_layout::SpeakerLayout;
use std::path::PathBuf;

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

    // The Current model owns first-order reflections in the fixed-cost six-bus
    // measured-HRTF field below, so disable the inherited analytic reflection
    // bank to prevent duplicate early energy.
    cfg = cfg.replace(
        "    reflections:\n      enabled: true",
        "    reflections:\n      enabled: false",
    );

    // The inherited FDN collapses to two decorrelated ear returns. Current now
    // keeps the same tiny 0.016 / 0.12 s / 32 ms closure in
    // `HrtfLateEnclosure`, where the upper tail remains six-axis directional
    // through measured HRTFs and the low tail stays interaurally coherent.
    cfg.replace(
        "    reverb:\n      enabled: true",
        "    reverb:\n      enabled: false",
    )
}

pub(crate) struct MusicSupportRenderer {
    primary: Engine,
    early_reflections: HrtfEarlyReflectionField,
    late_enclosure: HrtfLateEnclosure,
    primary_pcm: Vec<u8>,
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
        let mut primary = build_embedded_engine(
            &current_config,
            GRID_LAYOUT,
            sample_rate_hz,
            "Current model support",
        )?;
        let early_reflections = HrtfEarlyReflectionField::new(sample_rate_hz);
        let late_enclosure = HrtfLateEnclosure::new(sample_rate_hz);

        let header = streaming_f32_wav_header(MUSIC_FIELD_CHANNELS as u16, sample_rate_hz);
        seed_engine(&mut primary, &header, "Current model support")?;

        Ok(Self {
            primary,
            early_reflections,
            late_enclosure,
            primary_pcm: Vec::new(),
        })
    }

    pub(crate) fn process(&mut self, field_input: &[f32]) -> anyhow::Result<Vec<RenderedAudio>> {
        f32_as_le_bytes(field_input, &mut self.primary_pcm);
        let primary = self
            .primary
            .process(&self.primary_pcm, RInputTransport::Raw, 0)
            .context("Current model music support render failed")?;
        let early = self.early_reflections.process(field_input)?;
        let late = self.late_enclosure.process(field_input)?;
        let primary = add_stereo_support(primary, &early)?;
        add_stereo_support(primary, &late)
    }
}

/// Construct the exact Current support engine directly from embedded YAML.
///
/// This intentionally mirrors the sound-affecting parts of `Engine::from_paths`
/// while omitting config persistence, overlay preferences and bridge discovery.
/// The reference PCM bridge is linked directly per instance, so Current can be
/// destroyed/recreated in one process without a one-shot global registration.
pub(crate) fn build_embedded_engine(
    config_yaml: &str,
    layout_yaml: &str,
    sample_rate_hz: u32,
    label: &str,
) -> anyhow::Result<Engine> {
    let mut config: Config = serde_yaml_ng::from_str(config_yaml)
        .with_context(|| format!("failed to parse embedded Omniphony {label} config"))?;
    if let Some(render) = config.render.as_mut() {
        render.normalize_room_meters();
    }
    let render_cfg = config.render;
    let layout = SpeakerLayout::from_yaml_str(layout_yaml)
        .with_context(|| format!("failed to parse embedded Omniphony {label} layout"))?;

    let mut bridge = LoadedBridge::from_library(reference_bridge::linked_library());
    bridge.configure("presentation", "best");
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
    control.set_bridge_path(Some(PathBuf::from("<linked-reference-bridge>")));
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
            "Current model HRTF support length mismatch: renderer={} support={}",
            total,
            added.len()
        );
    }
    let mut cursor = 0usize;
    for block in &mut primary {
        if block.n_channels != 2 {
            bail!(
                "Current model HRTF support expected stereo primary output, got {} channels",
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

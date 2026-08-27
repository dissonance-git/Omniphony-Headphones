//! Listening-candidate Current support wrapper.
//!
//! The protected Current support renderer remains the normal build. This swaps
//! only the early-reflection field in the dedicated physical-listening EXE.

use crate::front_directional_early_candidate::HrtfEarlyReflectionField;
use crate::music_late_enclosure::HrtfLateEnclosure;
use crate::music_support::{SpatialProfile, build_embedded_engine, current_model_config, f32_as_le_bytes, seed_engine, streaming_f32_wav_header};
use crate::{Engine, RenderedAudio};
use anyhow::{Context, bail};
use bridge_api::RInputTransport;
use renderer::music_field::MUSIC_FIELD_CHANNELS;

pub(crate) struct FrontDirectionalEarlyMusicSupportRenderer {
    primary: Engine,
    early_reflections: HrtfEarlyReflectionField,
    late_enclosure: HrtfLateEnclosure,
    primary_pcm: Vec<u8>,
}

impl FrontDirectionalEarlyMusicSupportRenderer {
    pub(crate) fn new(_profile: SpatialProfile, sample_rate_hz: u32) -> anyhow::Result<Self> {
        const FIELD_CONFIG: &str = include_str!("../../assets/binaural-baselines/stereo-field-prototype.yaml");
        const GRID_LAYOUT: &str = include_str!("../../../layouts/system-h-derived-22.0-upper60-grid10.yaml");
        let current_config = current_model_config(FIELD_CONFIG);
        let mut primary = build_embedded_engine(&current_config, GRID_LAYOUT, sample_rate_hz, "front-directional early listening candidate")?;
        let early_reflections = HrtfEarlyReflectionField::new(sample_rate_hz);
        let late_enclosure = HrtfLateEnclosure::new(sample_rate_hz);
        let header = streaming_f32_wav_header(MUSIC_FIELD_CHANNELS as u16, sample_rate_hz);
        seed_engine(&mut primary, &header, "front-directional early listening candidate")?;
        Ok(Self { primary, early_reflections, late_enclosure, primary_pcm: Vec::new() })
    }

    pub(crate) fn process(&mut self, field_input: &[f32]) -> anyhow::Result<Vec<RenderedAudio>> {
        f32_as_le_bytes(field_input, &mut self.primary_pcm);
        let primary = self.primary.process(&self.primary_pcm, RInputTransport::Raw, 0).context("front-directional early primary support render failed")?;
        let early = self.early_reflections.process(field_input)?;
        let late = self.late_enclosure.process(field_input)?;
        let primary = add_stereo_support(primary, &early)?;
        add_stereo_support(primary, &late)
    }
}

fn add_stereo_support(mut primary: Vec<RenderedAudio>, added: &[f32]) -> anyhow::Result<Vec<RenderedAudio>> {
    let total: usize = primary.iter().map(|block| block.samples.len()).sum();
    if total != added.len() { bail!("front-directional early support length mismatch: renderer={} support={}", total, added.len()); }
    let mut cursor = 0usize;
    for block in &mut primary {
        if block.n_channels != 2 { bail!("front-directional early support expected stereo primary output, got {} channels", block.n_channels); }
        let end = cursor + block.samples.len();
        for (dst, src) in block.samples.iter_mut().zip(&added[cursor..end]) { *dst += *src; }
        cursor = end;
    }
    Ok(primary)
}

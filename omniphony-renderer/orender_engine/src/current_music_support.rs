//! Host-agnostic entry to the retained Omniphony Current-model support renderer.
//!
//! The implementation is the same support/early-field code used by the Windows
//! listening worker. Native hosts call this wrapper instead of cloning the DSP.

use crate::{RenderedAudio, music_support::{MusicSupportRenderer, SpatialProfile}};

pub struct CurrentMusicSupportRenderer {
    inner: MusicSupportRenderer,
}

impl CurrentMusicSupportRenderer {
    pub fn new(sample_rate_hz: u32) -> anyhow::Result<Self> {
        Ok(Self {
            inner: MusicSupportRenderer::new(SpatialProfile::Current, sample_rate_hz)?,
        })
    }

    pub fn process(&mut self, field_input: &[f32]) -> anyhow::Result<Vec<RenderedAudio>> {
        self.inner.process(field_input)
    }
}

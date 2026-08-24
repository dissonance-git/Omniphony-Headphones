//! Compatibility adapter for the retired process-loopback Windows app.
//!
//! Current support DSP is owned by `orender_engine`. Keep only the tiny host
//! shape needed by `music_worker_evidence` so the historical Windows app can
//! still compile without carrying a second copy of the renderer or early field.

use orender_engine::RenderedAudio;
use orender_engine::current_music_support::CurrentMusicSupportRenderer;

/// The historical Windows app has one retained listening model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpatialProfile {
    Current,
}

impl SpatialProfile {
    pub(crate) fn from_env() -> anyhow::Result<Self> {
        // Historical OMNIPHONY_PROFILE values are deliberately ignored. Current
        // is the only retained product model.
        Ok(Self::Current)
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
        }
    }
}

pub(crate) struct MusicSupportRenderer {
    inner: CurrentMusicSupportRenderer,
}

impl MusicSupportRenderer {
    pub(crate) fn new(_profile: SpatialProfile, sample_rate_hz: u32) -> anyhow::Result<Self> {
        Ok(Self {
            inner: CurrentMusicSupportRenderer::new(sample_rate_hz)?,
        })
    }

    /// Retained for the historical diagnostic print path. Current no longer has
    /// the retired hybrid-height branch.
    pub(crate) fn is_hybrid(&self) -> bool {
        false
    }

    pub(crate) fn process(&mut self, field_input: &[f32]) -> anyhow::Result<Vec<RenderedAudio>> {
        self.inner.process(field_input)
    }
}

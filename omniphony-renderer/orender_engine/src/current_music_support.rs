//! Host-agnostic entry to the retained Omniphony Current-model support renderer.
//!
//! Normal builds use protected Current. The candidate feature swaps only the
//! support renderer for a bounded physical-listening build.

use crate::RenderedAudio;
use crate::music_support::SpatialProfile;

#[cfg(feature = "front-directional-early-candidate")]
use crate::front_directional_early_music_support::FrontDirectionalEarlyMusicSupportRenderer as SelectedMusicSupportRenderer;
#[cfg(not(feature = "front-directional-early-candidate"))]
use crate::music_support::MusicSupportRenderer as SelectedMusicSupportRenderer;

pub struct CurrentMusicSupportRenderer { inner: SelectedMusicSupportRenderer }

impl CurrentMusicSupportRenderer {
    pub fn new(sample_rate_hz: u32) -> anyhow::Result<Self> {
        Ok(Self { inner: SelectedMusicSupportRenderer::new(SpatialProfile::Current, sample_rate_hz)? })
    }
    pub fn process(&mut self, field_input: &[f32]) -> anyhow::Result<Vec<RenderedAudio>> { self.inner.process(field_input) }
}

//! Authored fixed-bed entry into the retained Current renderer.
//!
//! This is intentionally not an upmixer. 5.1/7.1/7.1.4/7.1.4.4/8.1.4.4
//! channels are mapped into Omniphony's canonical 17-anchor static frame with
//! their source authority intact, then rendered through the same 22-direction
//! Current HRTF/room geometry used by the Windows listening path.
//!
//! LFE remains semantically distinct. The 22-direction headphone shell has no
//! physical subwoofer route, so LFE is removed from the positional bed and
//! folded equally into the final headphone pair at -6 dB per ear.

use crate::music_support::MusicSupportRenderer;
use anyhow::{bail, ensure};
use renderer::music_field::MUSIC_FIELD_CHANNELS;

const LFE_FOLD_GAIN: f32 = 0.501_187_2; // -6 dB amplitude per ear

pub struct CurrentAuthoredBedRenderer {
    input_channels: usize,
    support: MusicSupportRenderer,
    canonical: Vec<f32>,
    lfe: Vec<f32>,
}

impl CurrentAuthoredBedRenderer {
    pub fn supports_channels(channels: usize) -> bool {
        matches!(channels, 6 | 8 | 12 | 16 | 17)
    }

    pub fn new(sample_rate_hz: u32, input_channels: usize) -> anyhow::Result<Self> {
        ensure!(
            Self::supports_channels(input_channels),
            "Current authored bed supports 5.1/7.1/7.1.4/7.1.4.4/8.1.4.4, got {input_channels} channels"
        );
        Ok(Self {
            input_channels,
            support: MusicSupportRenderer::new(sample_rate_hz)?,
            canonical: Vec::new(),
            lfe: Vec::new(),
        })
    }

    /// Render interleaved authored PCM to binaural stereo.
    pub fn process(&mut self, input: &[f32]) -> anyhow::Result<Vec<f32>> {
        ensure!(
            !input.is_empty() && input.len() % self.input_channels == 0,
            "authored bed input width mismatch"
        );
        let frames = input.len() / self.input_channels;
        self.canonical.clear();
        self.canonical.resize(frames * MUSIC_FIELD_CHANNELS, 0.0);
        self.lfe.clear();
        self.lfe.resize(frames, 0.0);

        for (frame_index, source) in input.chunks_exact(self.input_channels).enumerate() {
            let target = &mut self.canonical
                [frame_index * MUSIC_FIELD_CHANNELS..(frame_index + 1) * MUSIC_FIELD_CHANNELS];
            map_frame_to_canonical(source, target)?;
            self.lfe[frame_index] = target[3];
            // Preserve LFE separately rather than pretending it has a point pose.
            target[3] = 0.0;
        }

        let rendered = self.support.process(&self.canonical)?;
        let total_frames: usize = rendered.iter().map(|block| block.n_frames).sum();
        ensure!(
            total_frames == frames,
            "Current authored bed changed frame count: input={frames} output={total_frames}"
        );

        let mut out = Vec::with_capacity(frames * 2);
        let mut frame_cursor = 0usize;
        for block in rendered {
            if block.n_channels != 2 || block.samples.len() != block.n_frames * 2 {
                bail!("Current authored bed renderer returned malformed stereo block");
            }
            for stereo in block.samples.chunks_exact(2) {
                let lfe = self.lfe[frame_cursor] * LFE_FOLD_GAIN;
                out.push(finite_or_zero(stereo[0]) + lfe);
                out.push(finite_or_zero(stereo[1]) + lfe);
                frame_cursor += 1;
            }
        }
        Ok(out)
    }
}

fn map_frame_to_canonical(source: &[f32], target: &mut [f32]) -> anyhow::Result<()> {
    ensure!(target.len() == MUSIC_FIELD_CHANNELS, "canonical scene width mismatch");
    target.fill(0.0);

    match source.len() {
        // 5.1: L R C LFE Ls Rs
        6 => target[..6].copy_from_slice(source),
        // 7.1: L R C LFE Ls Rs Lb Rb
        8 => target[..8].copy_from_slice(source),
        // 7.1.4: horizontal 7.1 + Tfl Tfr Tbl Tbr. Canonical index 8 is Cb.
        12 => {
            target[..8].copy_from_slice(&source[..8]);
            target[9..13].copy_from_slice(&source[8..12]);
        }
        // 7.1.4.4: horizontal 7.1 + upper four + lower four.
        16 => {
            target[..8].copy_from_slice(&source[..8]);
            target[9..13].copy_from_slice(&source[8..12]);
            target[13..17].copy_from_slice(&source[12..16]);
        }
        // 8.1.4.4 is already the canonical 17-anchor order.
        17 => target.copy_from_slice(source),
        other => bail!("unsupported authored bed width {other}"),
    }
    Ok(())
}

#[inline]
fn finite_or_zero(sample: f32) -> f32 {
    if sample.is_finite() { sample } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_layouts_map_without_inventing_missing_anchors() {
        let mut canonical = [0.0f32; MUSIC_FIELD_CHANNELS];
        let seven_one: Vec<f32> = (1..=8).map(|x| x as f32).collect();
        map_frame_to_canonical(&seven_one, &mut canonical).unwrap();
        assert_eq!(&canonical[..8], seven_one.as_slice());
        assert_eq!(canonical[8], 0.0); // BC EMPTY
        assert!(canonical[9..].iter().all(|x| *x == 0.0));

        let seven_one_four: Vec<f32> = (1..=12).map(|x| x as f32).collect();
        map_frame_to_canonical(&seven_one_four, &mut canonical).unwrap();
        assert_eq!(&canonical[..8], &seven_one_four[..8]);
        assert_eq!(canonical[8], 0.0);
        assert_eq!(&canonical[9..13], &seven_one_four[8..12]);
        assert!(canonical[13..].iter().all(|x| *x == 0.0));
    }

    #[test]
    fn full_8_1_4_4_is_identity_mapping() {
        let source: Vec<f32> = (1..=17).map(|x| x as f32).collect();
        let mut canonical = [0.0f32; MUSIC_FIELD_CHANNELS];
        map_frame_to_canonical(&source, &mut canonical).unwrap();
        assert_eq!(canonical.as_slice(), source.as_slice());
    }

    #[test]
    fn supported_widths_are_explicit() {
        for channels in [6usize, 8, 12, 16, 17] {
            assert!(CurrentAuthoredBedRenderer::supports_channels(channels));
        }
        for channels in [1usize, 2, 4, 10, 18] {
            assert!(!CurrentAuthoredBedRenderer::supports_channels(channels));
        }
    }

    #[test]
    fn lfe_fold_is_symmetric_and_bounded() {
        assert!(LFE_FOLD_GAIN > 0.49 && LFE_FOLD_GAIN < 0.51);
    }
}

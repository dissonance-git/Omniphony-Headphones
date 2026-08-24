//! Frequency-aware stereo music support-field extraction.
//!
//! The finished stereo master remains authoritative. This module analyzes real
//! L/R magnitude and phase relationships in the frequency domain, reuses the
//! portable stereo/scene evidence laws, and turns that evidence into causal
//! support lanes for the inherited binaural renderer.
//!
//! The FFT is analysis-only. Audible support is extracted with a causal
//! multiband filter bank, so this stage does not introduce STFT reconstruction
//! latency or replace the protected direct master.

use crate::scene_inference::{SceneEvidenceInput, SceneEvidenceKind, infer_scene_evidence};
use crate::stereo_inference::{
    StereoBinEvidence, StereoEvidenceTracker, StereoInferenceParams, estimate_bin,
};
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::f32::consts::PI;
use std::sync::Arc;

/// Canonical Omniphony 8.1.4.4 static-scene order:
/// L R C LFE Ls Rs Lb Rb Cb Tfl Tfr Tbl Tbr Bfl Bfr Bbl Bbr.
///
/// A stereo-derived field does not imply that all seventeen anchors contain
/// trustworthy content. C/LFE/Cb and all four lower anchors remain EMPTY here;
/// richer authored ingress may fill those positions without changing the scene
/// vocabulary or the downstream 22-direction rendering geometry.
pub const MUSIC_FIELD_CHANNELS: usize = 17;
const FFT_SIZE: usize = 1024;
const TRACK_TIME_CONSTANT_MS: f32 = 140.0;
/// Keep kick/snare body and low-frequency pressure in the protected master.
/// Spatial support starts above 320 Hz because physical listening showed the
/// spatial path losing some of the clean bass/drum authority below that point.
const CROSSOVER_HZ: [f32; 3] = [320.0, 1_200.0, 5_000.0];
const HEIGHT_PRIOR: [f32; 3] = [0.26, 0.60, 0.82];
/// Static top-band support trim. A fixed scale cannot pump; slower scene
/// controls own all audible movement.
const HIGH_BAND_SUPPORT_SCALE: f32 = 0.48;
/// The first audible support band overlaps the musical body region. Keep it
/// present for continuity, but let the protected master/foundation dominate.
const LOW_MID_SUPPORT_SCALE: f32 = 0.82;
/// Cascaded virtual-speaker -> HRTF rendering adds a second spectral-spatial
/// shaping stage. Keep the 1.2-5 kHz presence band slightly direct-dominant so
/// bright partials do not become hard-edged while the master retains attack.
const PRESENCE_SUPPORT_SCALE: f32 = 0.83;
/// Coherent height transfer. A fraction of an already-existing horizontal
/// support waveform is moved, sample-for-sample, into its elevated counterpart.
/// This is not an extra wet copy: horizontal + elevated lane amplitude is
/// algebraically unchanged before binaural rendering.
///
/// Physical listening found that the previous top-forward weighting could leave
/// the lower front hemisphere hollow while the rear/top shell was convincing.
/// Keep useful height, but reclaim enough presence/high-band support for the
/// horizontal front to remain a load-bearing part of the sphere.
const FRONT_COHERENT_HEIGHT_TRANSFER: [f32; 3] = [0.18, 0.44, 0.36];
const REAR_COHERENT_HEIGHT_TRANSFER: [f32; 3] = [0.12, 0.32, 0.28];

/// Front weighting is a transfer, not a gain or copied wet path. Existing rear
/// support is moved sample-for-sample into the matching front lane, preserving
/// the algebraic front+rear sum before binaural rendering. The fixed base makes
/// the front stronger for all earned shell material. Stable frontal-anchor
/// evidence adds a bounded extra bias so primary musical structure receives a
/// firmer front frame without inventing an authored center channel.
const REAR_TO_FRONT_BASE_TRANSFER: [f32; 3] = [0.14, 0.22, 0.16];
const REAR_TO_FRONT_ANCHOR_BONUS: [f32; 3] = [0.04, 0.06, 0.04];
const TOP_REAR_TO_TOP_FRONT_BASE_TRANSFER: [f32; 3] = [0.08, 0.14, 0.10];
const TOP_REAR_TO_TOP_FRONT_ANCHOR_BONUS: [f32; 3] = [0.02, 0.03, 0.02];

#[derive(Debug, Clone, Copy, Default)]
pub struct MusicFieldSnapshot {
    pub anchor: f32,
    pub broad: f32,
    pub lateral: f32,
    pub diffuse: f32,
    pub height: f32,
    pub lateral_pan: f32,
    pub side_fraction: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct BandAccum {
    weight: f32,
    anchor: f32,
    broad: f32,
    lateral: f32,
    diffuse: f32,
    pan_num: f32,
    pan_weight: f32,
    side_fraction: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct BandControl {
    anchor: f32,
    broad: f32,
    lateral: f32,
    diffuse: f32,
    height: f32,
    pan: f32,
    side_fraction: f32,
}

impl BandControl {
    fn approach(&mut self, target: Self, high_band: bool) {
        // >5 kHz carries the strongest height prior, so opening/closing it too
        // quickly can sound like treble gain breathing. Move that band slowly;
        // height comes from geometry and HRTF evidence, not a fast envelope.
        let (rise, fall, pan_rise, pan_fall) = if high_band {
            (0.10, 0.040, 0.085, 0.040)
        } else {
            (0.32, 0.12, 0.30, 0.12)
        };
        self.anchor = slew_with_rates(self.anchor, target.anchor, rise, fall);
        self.broad = slew_with_rates(self.broad, target.broad, rise, fall);
        self.lateral = slew_with_rates(self.lateral, target.lateral, rise, fall);
        self.diffuse = slew_with_rates(self.diffuse, target.diffuse, rise, fall);
        self.height = slew_with_rates(self.height, target.height, rise, fall);
        self.pan = slew_signed_with_rates(self.pan, target.pan, pan_rise, pan_fall);
        self.side_fraction = slew_with_rates(self.side_fraction, target.side_fraction, rise, fall);
    }
}

fn slew_with_rates(current: f32, target: f32, rise: f32, fall: f32) -> f32 {
    let coefficient = if target > current { rise } else { fall };
    (current + coefficient * (target - current)).clamp(0.0, 1.0)
}

fn slew_signed_with_rates(current: f32, target: f32, rise: f32, fall: f32) -> f32 {
    let coefficient = if target.abs() > current.abs() { rise } else { fall };
    (current + coefficient * (target - current)).clamp(-1.0, 1.0)
}

#[inline]
fn transfer_to_elevation(horizontal: &mut f32, elevated: &mut f32, fraction: f32) {
    let transfer = *horizontal * fraction.clamp(0.0, 0.60);
    *horizontal -= transfer;
    *elevated += transfer;
}

#[inline]
fn transfer_forward(rear: &mut f32, front: &mut f32, fraction: f32) {
    let transfer = *rear * fraction.clamp(0.0, 0.40);
    *rear -= transfer;
    *front += transfer;
}

#[derive(Debug, Clone, Copy)]
struct OnePoleLowPass {
    alpha: f32,
    state: f32,
}

impl OnePoleLowPass {
    fn new(sample_rate_hz: u32, cutoff_hz: f32) -> Self {
        let dt = 1.0 / sample_rate_hz.max(1) as f32;
        let rc = 1.0 / (2.0 * PI * cutoff_hz.max(1.0));
        Self { alpha: dt / (rc + dt), state: 0.0 }
    }

    fn process(&mut self, sample: f32) -> f32 {
        self.state += self.alpha * (sample - self.state);
        self.state
    }
}

struct ChannelBandSplit {
    low_320: OnePoleLowPass,
    low_1200: OnePoleLowPass,
    low_5000: OnePoleLowPass,
}

impl ChannelBandSplit {
    fn new(sample_rate_hz: u32) -> Self {
        Self {
            low_320: OnePoleLowPass::new(sample_rate_hz, CROSSOVER_HZ[0]),
            low_1200: OnePoleLowPass::new(sample_rate_hz, CROSSOVER_HZ[1]),
            low_5000: OnePoleLowPass::new(sample_rate_hz, CROSSOVER_HZ[2]),
        }
    }

    /// The four outputs sum algebraically to `sample`: adjacent bands are
    /// differences of parallel low-pass states rather than independent filters.
    fn split(&mut self, sample: f32) -> [f32; 4] {
        let a = self.low_320.process(sample);
        let b = self.low_1200.process(sample);
        let c = self.low_5000.process(sample);
        [a, b - a, c - b, sample - c]
    }
}

/// Portable music-field extractor.
///
/// Output order is canonical 8.1.4.4:
/// `L R C LFE Ls Rs Lb Rb Cb Tfl Tfr Tbl Tbr Bfl Bfr Bbl Bbr`.
///
/// Stereo evidence fills only the positions it can justify. C/LFE/Cb and the
/// lower layer are zero here, preserving the difference between DERIVED and
/// EMPTY rather than treating a coordinate frame as evidence of authorship.
pub struct MusicFieldProcessor {
    sample_rate_hz: u32,
    fft: Arc<dyn Fft<f32>>,
    left_fft: Vec<Complex<f32>>,
    right_fft: Vec<Complex<f32>>,
    trackers: Vec<StereoEvidenceTracker>,
    controls: [BandControl; 3],
    left_split: ChannelBandSplit,
    right_split: ChannelBandSplit,
    snapshot: MusicFieldSnapshot,
}

impl MusicFieldProcessor {
    pub fn new(sample_rate_hz: u32) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        Self {
            sample_rate_hz,
            fft,
            left_fft: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            right_fft: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            trackers: vec![StereoEvidenceTracker::default(); FFT_SIZE / 2 + 1],
            controls: [BandControl::default(); 3],
            left_split: ChannelBandSplit::new(sample_rate_hz),
            right_split: ChannelBandSplit::new(sample_rate_hz),
            snapshot: MusicFieldSnapshot::default(),
        }
    }

    pub fn snapshot(&self) -> MusicFieldSnapshot { self.snapshot }

    pub fn process_interleaved_stereo(&mut self, input: &[f32]) -> Vec<f32> {
        if input.len() < 2 || input.len() % 2 != 0 {
            return Vec::new();
        }

        self.analyze(input);

        let frames = input.len() / 2;
        let mut out = Vec::with_capacity(frames * MUSIC_FIELD_CHANNELS);
        for frame in input.chunks_exact(2) {
            let left_bands = self.left_split.split(frame[0]);
            let right_bands = self.right_split.split(frame[1]);

            let mut front_l = 0.0;
            let mut front_r = 0.0;
            let mut side_l = 0.0;
            let mut side_r = 0.0;
            let mut rear_l = 0.0;
            let mut rear_r = 0.0;
            let mut top_front_l = 0.0;
            let mut top_front_r = 0.0;
            let mut top_rear_l = 0.0;
            let mut top_rear_r = 0.0;

            // Band 0 (<320 Hz) remains entirely in the protected master.
            for band in 1..4 {
                let control = self.controls[band - 1];
                let left = left_bands[band];
                let right = right_bands[band];
                let mid = 0.5 * (left + right);
                let side = 0.5 * (left - right);

                let relational_l = left - 0.78 * mid;
                let relational_r = right - 0.78 * mid;

                let steer_l = (0.5 - 0.5 * control.pan).clamp(0.0, 1.0);
                let steer_r = (0.5 + 0.5 * control.pan).clamp(0.0, 1.0);

                let broad_l = relational_l * control.broad;
                let broad_r = relational_r * control.broad;
                let lateral_l =
                    (0.70 * relational_l + 0.30 * side) * control.lateral * (0.62 + 0.38 * steer_l);
                let lateral_r =
                    (0.70 * relational_r - 0.30 * side) * control.lateral * (0.62 + 0.38 * steer_r);
                let diffuse_l = side * control.diffuse;
                let diffuse_r = -side * control.diffuse;

                let mut band_front_l = 0.98 * broad_l + 0.16 * lateral_l;
                let mut band_front_r = 0.98 * broad_r + 0.16 * lateral_r;
                let mut band_side_l = 0.28 * broad_l + 0.90 * lateral_l + 0.06 * diffuse_l;
                let mut band_side_r = 0.28 * broad_r + 0.90 * lateral_r + 0.06 * diffuse_r;
                let mut band_rear_l = 0.14 * lateral_l + 0.28 * diffuse_l;
                let mut band_rear_r = 0.14 * lateral_r + 0.28 * diffuse_r;

                let height = control.height;
                // The protected master already carries the coherent stereo mid.
                // Duplicating bright correlated mid directly into top-front can
                // comb against that master after the cascaded room. Above 5 kHz
                // require height to come from relational/lateral/diffuse evidence.
                let front_height_mid = if band == 3 { 0.0 } else { mid * control.broad * 0.08 };
                let mut band_top_front_l = height
                    * (0.62 * broad_l + 0.22 * lateral_l + 0.08 * diffuse_l + front_height_mid);
                let mut band_top_front_r = height
                    * (0.62 * broad_r + 0.22 * lateral_r + 0.08 * diffuse_r + front_height_mid);
                let mut band_top_rear_l =
                    height * (0.06 * broad_l + 0.10 * lateral_l + 0.19 * diffuse_l);
                let mut band_top_rear_r =
                    height * (0.06 * broad_r + 0.10 * lateral_r + 0.19 * diffuse_r);

                let static_band_scale = if band == 1 {
                    LOW_MID_SUPPORT_SCALE
                } else if band == 2 {
                    PRESENCE_SUPPORT_SCALE
                } else if band == 3 {
                    HIGH_BAND_SUPPORT_SCALE
                } else {
                    1.0
                };
                band_front_l *= static_band_scale;
                band_front_r *= static_band_scale;
                band_side_l *= static_band_scale;
                band_side_r *= static_band_scale;
                band_rear_l *= static_band_scale;
                band_rear_r *= static_band_scale;
                band_top_front_l *= static_band_scale;
                band_top_front_r *= static_band_scale;
                band_top_rear_l *= static_band_scale;
                band_top_rear_r *= static_band_scale;

                // Move exact existing support upward. No delay, decorrelation or
                // second copy is created. The 22-direction HRTF renderer receives
                // genuine elevated excitation while horizontal front remains
                // perceptually load-bearing.
                let front_transfer =
                    (height * FRONT_COHERENT_HEIGHT_TRANSFER[band - 1]).clamp(0.0, 0.60);
                let rear_transfer =
                    (height * REAR_COHERENT_HEIGHT_TRANSFER[band - 1]).clamp(0.0, 0.60);
                transfer_to_elevation(&mut band_front_l, &mut band_top_front_l, front_transfer);
                transfer_to_elevation(&mut band_front_r, &mut band_top_front_r, front_transfer);
                transfer_to_elevation(&mut band_rear_l, &mut band_top_rear_l, rear_transfer);
                transfer_to_elevation(&mut band_rear_r, &mut band_top_rear_r, rear_transfer);

                // Counter the strong rear hemisphere with a stronger front shell.
                // Anchor confidence is already smoothed/persistent evidence, so it
                // can add a small front bias without reacting to a single frame.
                let anchor = control.anchor.clamp(0.0, 1.0);
                let horizontal_forward = (REAR_TO_FRONT_BASE_TRANSFER[band - 1]
                    + anchor * REAR_TO_FRONT_ANCHOR_BONUS[band - 1])
                    .clamp(0.0, 0.40);
                let elevated_forward = (TOP_REAR_TO_TOP_FRONT_BASE_TRANSFER[band - 1]
                    + anchor * TOP_REAR_TO_TOP_FRONT_ANCHOR_BONUS[band - 1])
                    .clamp(0.0, 0.40);
                transfer_forward(&mut band_rear_l, &mut band_front_l, horizontal_forward);
                transfer_forward(&mut band_rear_r, &mut band_front_r, horizontal_forward);
                transfer_forward(&mut band_top_rear_l, &mut band_top_front_l, elevated_forward);
                transfer_forward(&mut band_top_rear_r, &mut band_top_front_r, elevated_forward);

                front_l += band_front_l;
                front_r += band_front_r;
                side_l += band_side_l;
                side_r += band_side_r;
                rear_l += band_rear_l;
                rear_r += band_rear_r;
                top_front_l += band_top_front_l;
                top_front_r += band_top_front_r;
                top_rear_l += band_top_rear_l;
                top_rear_r += band_top_rear_r;
            }

            // Canonical 8.1.4.4. Stereo inference earns horizontal wrap and
            // upper support only; center/LFE/back-center/lower stay EMPTY.
            out.extend_from_slice(&[
                front_l, front_r, 0.0, 0.0,
                side_l, side_r, rear_l, rear_r, 0.0,
                top_front_l, top_front_r, top_rear_l, top_rear_r,
                0.0, 0.0, 0.0, 0.0,
            ]);
        }
        out
    }

    fn analyze(&mut self, input: &[f32]) {
        self.left_fft.fill(Complex::new(0.0, 0.0));
        self.right_fft.fill(Complex::new(0.0, 0.0));

        let frames = input.len() / 2;
        let usable = frames.min(FFT_SIZE);
        let start_frame = frames.saturating_sub(usable);
        for i in 0..usable {
            let source = (start_frame + i) * 2;
            let window = if usable > 1 {
                0.5 - 0.5 * (2.0 * PI * i as f32 / (usable - 1) as f32).cos()
            } else {
                1.0
            };
            self.left_fft[i] = Complex::new(input[source] * window, 0.0);
            self.right_fft[i] = Complex::new(input[source + 1] * window, 0.0);
        }

        self.fft.process(&mut self.left_fft);
        self.fft.process(&mut self.right_fft);

        let mut reference_magnitude = 1.0e-9_f32;
        for bin in 1..=FFT_SIZE / 2 {
            let l = self.left_fft[bin].norm();
            let r = self.right_fft[bin].norm();
            reference_magnitude = reference_magnitude.max(l.hypot(r));
        }

        let elapsed_ms = frames as f32 * 1000.0 / self.sample_rate_hz.max(1) as f32;
        let mut accum = [BandAccum::default(); 3];
        let params = StereoInferenceParams { focus: 0.05, object_separation: 0.15 };

        for bin in 1..=FFT_SIZE / 2 {
            let frequency_hz = bin as f32 * self.sample_rate_hz as f32 / FFT_SIZE as f32;
            if frequency_hz < CROSSOVER_HZ[0] { continue; }
            let band = if frequency_hz < CROSSOVER_HZ[1] {
                0
            } else if frequency_hz < CROSSOVER_HZ[2] {
                1
            } else {
                2
            };

            let left = self.left_fft[bin];
            let right = self.right_fft[bin];
            let estimate = estimate_bin(
                StereoBinEvidence {
                    left_magnitude: left.norm(),
                    right_magnitude: right.norm(),
                    left_phase: left.im.atan2(left.re),
                    right_phase: right.im.atan2(right.re),
                },
                params,
            );
            let tracked = self.trackers[bin].update(estimate, elapsed_ms, TRACK_TIME_CONSTANT_MS);
            let candidate = infer_scene_evidence(SceneEvidenceInput {
                frequency_hz,
                estimate,
                tracked,
                magnitude: estimate.total_magnitude,
                reference_magnitude,
            });

            let weight = estimate.total_magnitude.max(1.0e-9);
            let anchor = if matches!(candidate.kind, SceneEvidenceKind::FrontalAnchor) {
                candidate.foundation_support.max(0.72)
            } else {
                candidate.foundation_support
            }
            .clamp(0.0, 1.0);
            let movable = (1.0 - 0.92 * anchor).clamp(0.0, 1.0);

            let lateral = if matches!(candidate.kind, SceneEvidenceKind::LateralObjectCandidate) {
                candidate.reassignment_safety.max(0.62 * candidate.object_support)
            } else {
                0.28 * candidate.reassignment_safety
            } * movable;

            let broad = match candidate.kind {
                SceneEvidenceKind::BroadSource => 0.40 + 0.60 * candidate.side_fraction,
                SceneEvidenceKind::LateralObjectCandidate => 0.22 + 0.38 * candidate.side_fraction,
                SceneEvidenceKind::DiffuseField => 0.16 + 0.24 * candidate.side_fraction,
                SceneEvidenceKind::FrontalAnchor => 0.0,
            } * movable;

            let diffuse = if matches!(candidate.kind, SceneEvidenceKind::DiffuseField) {
                candidate.field_support
            } else {
                0.22 * candidate.field_support
            } * movable;

            let a = &mut accum[band];
            a.weight += weight;
            a.anchor += weight * anchor;
            a.broad += weight * broad;
            a.lateral += weight * lateral;
            a.diffuse += weight * diffuse;
            a.side_fraction += weight * candidate.side_fraction;
            let pan_weight = weight * candidate.object_support.max(0.05);
            a.pan_num += pan_weight * candidate.pan;
            a.pan_weight += pan_weight;
        }

        let mut snapshot_weight = 0.0;
        let mut snapshot = MusicFieldSnapshot::default();
        for (index, a) in accum.into_iter().enumerate() {
            let target = if a.weight > 1.0e-9 {
                let broad = ((a.broad / a.weight) * 1.65).clamp(0.0, 1.0);
                let lateral = ((a.lateral / a.weight) * 2.00).clamp(0.0, 1.0);
                let diffuse = ((a.diffuse / a.weight) * 1.70).clamp(0.0, 1.0);
                let shell_evidence = (0.45 * broad + 0.25 * lateral + 0.65 * diffuse).clamp(0.0, 1.0);
                let height = (HEIGHT_PRIOR[index] * (0.35 + 0.65 * shell_evidence)).clamp(0.0, 1.0);
                BandControl {
                    anchor: (a.anchor / a.weight).clamp(0.0, 1.0),
                    broad,
                    lateral,
                    diffuse,
                    height,
                    pan: if a.pan_weight > 1.0e-9 {
                        (a.pan_num / a.pan_weight).clamp(-1.0, 1.0)
                    } else { 0.0 },
                    side_fraction: (a.side_fraction / a.weight).clamp(0.0, 1.0),
                }
            } else {
                BandControl::default()
            };
            self.controls[index].approach(target, index == 2);

            let w = a.weight.max(1.0e-9);
            snapshot_weight += w;
            snapshot.anchor += w * self.controls[index].anchor;
            snapshot.broad += w * self.controls[index].broad;
            snapshot.lateral += w * self.controls[index].lateral;
            snapshot.diffuse += w * self.controls[index].diffuse;
            snapshot.height += w * self.controls[index].height;
            snapshot.lateral_pan += w * self.controls[index].pan;
            snapshot.side_fraction += w * self.controls[index].side_fraction;
        }

        if snapshot_weight > 1.0e-9 {
            snapshot.anchor /= snapshot_weight;
            snapshot.broad /= snapshot_weight;
            snapshot.lateral /= snapshot_weight;
            snapshot.diffuse /= snapshot_weight;
            snapshot.height /= snapshot_weight;
            snapshot.lateral_pan /= snapshot_weight;
            snapshot.side_fraction /= snapshot_weight;
        }
        self.snapshot = snapshot;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_field_is_8_1_4_4_wide() {
        assert_eq!(MUSIC_FIELD_CHANNELS, 17);
    }

    #[test]
    fn elevation_transfer_moves_signal_without_adding_a_copy() {
        for horizontal in [0.75_f32, -0.75, 0.125, -0.125] {
            let mut h = horizontal;
            let mut e = 0.20_f32;
            let before = h + e;
            transfer_to_elevation(&mut h, &mut e, 0.35);
            assert!((h + e - before).abs() < 1.0e-6);
            assert!(h.abs() < horizontal.abs());
        }
    }

    #[test]
    fn forward_transfer_moves_existing_support_without_adding_a_copy() {
        for rear in [0.75_f32, -0.75, 0.125, -0.125] {
            let mut r = rear;
            let mut f = 0.20_f32;
            let before = r + f;
            transfer_forward(&mut r, &mut f, 0.28);
            assert!((r + f - before).abs() < 1.0e-6);
            assert!(r.abs() < rear.abs());
        }
    }

    #[test]
    fn front_weighting_keeps_horizontal_front_load_bearing() {
        assert_eq!(FRONT_COHERENT_HEIGHT_TRANSFER[0], 0.18);
        assert!(FRONT_COHERENT_HEIGHT_TRANSFER[1] <= 0.44);
        assert!(FRONT_COHERENT_HEIGHT_TRANSFER[2] <= 0.36);
        assert!(REAR_COHERENT_HEIGHT_TRANSFER[1] < FRONT_COHERENT_HEIGHT_TRANSFER[1]);
        assert!(REAR_COHERENT_HEIGHT_TRANSFER[2] < FRONT_COHERENT_HEIGHT_TRANSFER[2]);
    }

    #[test]
    fn stable_anchor_can_strengthen_front_beyond_the_previous_fixed_candidate() {
        let band = 1usize;
        let anchored = REAR_TO_FRONT_BASE_TRANSFER[band] + REAR_TO_FRONT_ANCHOR_BONUS[band];
        assert!(REAR_TO_FRONT_BASE_TRANSFER[band] > 0.16);
        assert!(anchored > 0.24);
        assert!(anchored <= 0.40);
        assert!(TOP_REAR_TO_TOP_FRONT_BASE_TRANSFER[band] > 0.10);
    }

    #[test]
    fn mono_center_does_not_become_a_large_support_field() {
        let mut processor = MusicFieldProcessor::new(48_000);
        let mut input = Vec::new();
        for i in 0..1024 {
            let x = (2.0 * PI * 1000.0 * i as f32 / 48_000.0).sin() * 0.5;
            input.extend_from_slice(&[x, x]);
        }
        let out = processor.process_interleaved_stereo(&input);
        assert_eq!(out.len(), 1024 * MUSIC_FIELD_CHANNELS);
        let energy: f32 = out.iter().map(|x| x * x).sum::<f32>() / out.len() as f32;
        assert!(energy < 0.012);
    }

    #[test]
    fn stereo_derivation_leaves_unearned_canonical_anchors_empty() {
        let mut processor = MusicFieldProcessor::new(48_000);
        let mut input = Vec::new();
        for i in 0..4096 {
            let l = (2.0 * PI * 1800.0 * i as f32 / 48_000.0).sin() * 0.5;
            let r = (2.0 * PI * 2500.0 * i as f32 / 48_000.0).sin() * 0.2;
            input.extend_from_slice(&[l, r]);
        }
        let out = processor.process_interleaved_stereo(&input);
        for frame in out.chunks_exact(MUSIC_FIELD_CHANNELS) {
            for index in [2usize, 3, 8, 13, 14, 15, 16] {
                assert_eq!(frame[index], 0.0, "canonical anchor {index} was invented");
            }
        }
    }

    #[test]
    fn hard_left_material_produces_anterior_height_support_without_lfe() {
        let mut processor = MusicFieldProcessor::new(48_000);
        let mut input = Vec::new();
        for i in 0..4096 {
            let x = (2.0 * PI * 1800.0 * i as f32 / 48_000.0).sin() * 0.5;
            input.extend_from_slice(&[x, 0.0]);
        }
        let mut front_energy = 0.0;
        let mut lateral_energy = 0.0;
        let mut rear_energy = 0.0;
        let mut height_energy = 0.0;
        let mut top_front_energy = 0.0;
        let mut top_rear_energy = 0.0;
        let mut lfe_energy = 0.0;
        for chunk in input.chunks(2048) {
            let out = processor.process_interleaved_stereo(chunk);
            for frame in out.chunks_exact(MUSIC_FIELD_CHANNELS) {
                front_energy += frame[0] * frame[0] + frame[1] * frame[1];
                lateral_energy += frame[4] * frame[4] + frame[5] * frame[5];
                rear_energy += frame[6] * frame[6] + frame[7] * frame[7];
                top_front_energy += frame[9] * frame[9] + frame[10] * frame[10];
                top_rear_energy += frame[11] * frame[11] + frame[12] * frame[12];
                height_energy += frame[9..13].iter().map(|x| x * x).sum::<f32>();
                lfe_energy += frame[3] * frame[3];
            }
        }
        assert!(front_energy > rear_energy);
        assert!(lateral_energy > 0.0);
        assert!(height_energy > 0.0);
        assert!(top_front_energy > top_rear_energy);
        assert_eq!(lfe_energy, 0.0);
    }

    #[test]
    fn high_band_trim_is_static() {
        assert!(HIGH_BAND_SUPPORT_SCALE > 0.0);
        assert!(HIGH_BAND_SUPPORT_SCALE <= 1.0);
        assert!(LOW_MID_SUPPORT_SCALE > 0.0);
        assert!(LOW_MID_SUPPORT_SCALE <= 1.0);
    }

    #[test]
    fn bass_foundation_stays_well_below_direct_energy() {
        let mut processor = MusicFieldProcessor::new(48_000);
        let mut input = Vec::new();
        for i in 0..4096 {
            let x = (2.0 * PI * 60.0 * i as f32 / 48_000.0).sin() * 0.5;
            input.extend_from_slice(&[x, x * 0.8]);
        }
        let mut support_energy = 0.0;
        let mut direct_energy = 0.0;
        let mut lfe_energy = 0.0;
        for chunk in input.chunks(2048) {
            let out = processor.process_interleaved_stereo(chunk);
            for (frame, direct) in out.chunks_exact(MUSIC_FIELD_CHANNELS).zip(chunk.chunks_exact(2)) {
                support_energy += frame.iter().map(|x| x * x).sum::<f32>();
                direct_energy += direct[0] * direct[0] + direct[1] * direct[1];
                lfe_energy += frame[3] * frame[3];
            }
        }
        let leakage_ratio = support_energy / direct_energy.max(1.0e-12);
        assert!(
            leakage_ratio < 0.002,
            "60 Hz support leakage ratio {leakage_ratio:.6} exceeds the -27 dB energy guard"
        );
        assert_eq!(lfe_energy, 0.0);
    }
}

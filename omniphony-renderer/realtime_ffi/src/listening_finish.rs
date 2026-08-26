//! Noire X-specific post-EQ transient finishing for the Windows Current path.
//!
//! This processor is deliberately distinct from the static headphone EQ, but it
//! is a permanent part of the Noire X profile. It runs after tonal correction and
//! before the existing lookahead peak guard. The audible signal is never split
//! into frequency bands: a low-frequency sidechain only decides whether a linked
//! broadband gain is allowed to move.
//!
//! The qualification rule is intentionally strict. Only LF-dominant motion below
//! the kick/bass region can open the enhancement. Midrange, treble, broadband
//! impulses, and crossover leakage have no independent gain authority. Qualifying
//! attacks receive a short positive lift and then return to unity; this processor
//! never ducks sustain or falls below unity. Both ears receive exactly the same
//! gain so existing ILD/IPD/HRTF relationships are preserved.

use std::f32::consts::PI;

const FAST_ENVELOPE_MS: f32 = 2.0;
const SLOW_ENVELOPE_MS: f32 = 48.0;
const MAX_ATTACK_LIFT_DB: f32 = 2.8;
const TRANSIENT_FULL_SCALE: f32 = 1.35;

const DETECTOR_LOW_CUTOFF_HZ: f32 = 180.0;
const LF_DOMINANCE_FLOOR: f32 = 0.30;
const LF_DOMINANCE_FULL: f32 = 0.65;

pub(crate) struct ListeningFinish {
    total_fast_energy: f32,
    total_slow_energy: f32,
    low_fast_energy: f32,
    low_slow_energy: f32,
    detector_low_left_1: f32,
    detector_low_left_2: f32,
    detector_low_right_1: f32,
    detector_low_right_2: f32,
    fast_alpha: f32,
    slow_alpha: f32,
    detector_low_alpha: f32,
}

impl ListeningFinish {
    pub(crate) fn new(sample_rate_hz: u32) -> Self {
        let sample_rate = sample_rate_hz.max(1) as f32;
        Self {
            total_fast_energy: 0.0,
            total_slow_energy: 0.0,
            low_fast_energy: 0.0,
            low_slow_energy: 0.0,
            detector_low_left_1: 0.0,
            detector_low_left_2: 0.0,
            detector_low_right_1: 0.0,
            detector_low_right_2: 0.0,
            fast_alpha: one_pole_alpha(FAST_ENVELOPE_MS, sample_rate),
            slow_alpha: one_pole_alpha(SLOW_ENVELOPE_MS, sample_rate),
            detector_low_alpha: one_pole_cutoff_alpha(DETECTOR_LOW_CUTOFF_HZ, sample_rate),
        }
    }

    pub(crate) fn process_interleaved(&mut self, samples: &mut [f32]) {
        for frame in samples.chunks_exact_mut(2) {
            let mut left = finite_or_zero(frame[0]);
            let mut right = finite_or_zero(frame[1]);

            let linked_energy = 0.5 * (left * left + right * right);
            self.total_fast_energy +=
                self.fast_alpha * (linked_energy - self.total_fast_energy);
            self.total_slow_energy +=
                self.slow_alpha * (linked_energy - self.total_slow_energy);

            let low_left = cascaded_low_pass(
                left,
                &mut self.detector_low_left_1,
                &mut self.detector_low_left_2,
                self.detector_low_alpha,
            );
            let low_right = cascaded_low_pass(
                right,
                &mut self.detector_low_right_1,
                &mut self.detector_low_right_2,
                self.detector_low_alpha,
            );
            let low_energy = 0.5 * (low_left * low_left + low_right * low_right);
            self.low_fast_energy += self.fast_alpha * (low_energy - self.low_fast_energy);
            self.low_slow_energy += self.slow_alpha * (low_energy - self.low_slow_energy);

            let attack_dominance = dominance_gate(
                self.low_fast_energy / (self.total_fast_energy + 1.0e-12),
            );
            let transient = normalized_rise(
                self.low_fast_energy,
                self.low_slow_energy,
                TRANSIENT_FULL_SCALE,
            ) * attack_dominance;

            let linked_gain = db_to_gain(MAX_ATTACK_LIFT_DB * transient);
            left *= linked_gain;
            right *= linked_gain;

            frame[0] = finite_or_zero(left);
            frame[1] = finite_or_zero(right);
        }
    }
}

fn cascaded_low_pass(sample: f32, state_1: &mut f32, state_2: &mut f32, alpha: f32) -> f32 {
    *state_1 += alpha * (sample - *state_1);
    *state_2 += alpha * (*state_1 - *state_2);
    *state_2
}

fn dominance_gate(share: f32) -> f32 {
    ((share - LF_DOMINANCE_FLOOR) / (LF_DOMINANCE_FULL - LF_DOMINANCE_FLOOR)).clamp(0.0, 1.0)
}

fn normalized_rise(fast: f32, slow: f32, full_scale: f32) -> f32 {
    let positive_rise = (fast - slow).max(0.0);
    let relative_rise = positive_rise / (slow + 1.0e-7);
    (relative_rise / full_scale).clamp(0.0, 1.0)
}

fn one_pole_alpha(time_ms: f32, sample_rate_hz: f32) -> f32 {
    1.0 - (-1.0 / (0.001 * time_ms.max(0.01) * sample_rate_hz.max(1.0))).exp()
}

fn one_pole_cutoff_alpha(cutoff_hz: f32, sample_rate_hz: f32) -> f32 {
    let sample_rate = sample_rate_hz.max(1.0);
    let cutoff = cutoff_hz.clamp(1.0, sample_rate * 0.45);
    1.0 - (-2.0 * PI * cutoff / sample_rate).exp()
}

fn db_to_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

fn finite_or_zero(sample: f32) -> f32 {
    if sample.is_finite() { sample } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_burst_peak_gain(frequency_hz: f32) -> f32 {
        let sample_rate = 48_000.0_f32;
        let amplitude = 0.4_f32;
        let warmup_frames = 512usize;
        let burst_frames = 4096usize;
        let mut samples = vec![0.0_f32; (warmup_frames + burst_frames) * 2];
        for frame in 0..burst_frames {
            let phase = 2.0 * PI * frequency_hz * frame as f32 / sample_rate;
            let sample = amplitude * phase.sin();
            let index = (warmup_frames + frame) * 2;
            samples[index] = sample;
            samples[index + 1] = sample;
        }

        let mut finish = ListeningFinish::new(48_000);
        finish.process_interleaved(&mut samples);
        samples
            .iter()
            .fold(0.0_f32, |peak, &sample| peak.max(sample.abs()))
            / amplitude
    }

    #[test]
    fn enhancement_is_strictly_kick_and_bass_qualified() {
        let kick_gain = sine_burst_peak_gain(80.0);
        let mid_gain = sine_burst_peak_gain(1_000.0);
        let high_gain = sine_burst_peak_gain(10_000.0);
        assert!(kick_gain > 1.10, "kick_gain={kick_gain}");
        assert!(mid_gain <= 1.005, "mid_gain={mid_gain}");
        assert!(high_gain <= 1.005, "high_gain={high_gain}");
    }

    #[test]
    fn broadband_impulse_does_not_open_the_enhancement() {
        let mut finish = ListeningFinish::new(48_000);
        let mut samples = vec![0.0_f32; 2048 * 2];
        samples[1024 * 2] = 1.0;
        samples[1024 * 2 + 1] = -0.5;
        finish.process_interleaved(&mut samples);
        assert!((samples[1024 * 2] - 1.0).abs() < 1.0e-6);
        assert!((samples[1024 * 2 + 1] + 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn low_frequency_enhancement_uses_one_linked_gain_for_both_ears() {
        let sample_rate = 48_000.0_f32;
        let mut samples = vec![0.0_f32; 4096 * 2];
        for frame in 0..4096 {
            let phase = 2.0 * PI * 80.0 * frame as f32 / sample_rate;
            samples[frame * 2] = 0.4 * phase.sin();
            samples[frame * 2 + 1] = -0.2 * phase.sin();
        }
        let before = samples.clone();
        let mut finish = ListeningFinish::new(48_000);
        finish.process_interleaved(&mut samples);

        let mut observed_boost = false;
        for frame in 0..4096 {
            let l0 = before[frame * 2];
            let r0 = before[frame * 2 + 1];
            if l0.abs() < 1.0e-4 || r0.abs() < 1.0e-4 {
                continue;
            }
            let left_gain = samples[frame * 2] / l0;
            let right_gain = samples[frame * 2 + 1] / r0;
            assert!((left_gain - right_gain).abs() < 1.0e-5);
            observed_boost |= left_gain > 1.02;
        }
        assert!(observed_boost);
    }

    #[test]
    fn enhancement_never_reduces_program_level() {
        let sample_rate = 48_000.0_f32;
        let frames = 8192usize;
        let mut samples = vec![0.0_f32; frames * 2];
        for frame in 0..frames {
            let amplitude = if frame < 4096 { 0.5 } else { 0.15 };
            let phase = 2.0 * PI * 80.0 * frame as f32 / sample_rate;
            let sample = amplitude * phase.sin();
            samples[frame * 2] = sample;
            samples[frame * 2 + 1] = sample;
        }
        let before = samples.clone();
        let mut finish = ListeningFinish::new(48_000);
        finish.process_interleaved(&mut samples);
        for (before, after) in before.iter().zip(samples.iter()) {
            if before.abs() > 1.0e-6 {
                assert!(after.abs() + 1.0e-7 >= before.abs());
            }
        }
    }

    #[test]
    fn attack_gain_is_bounded() {
        assert!((2.5..=3.0).contains(&MAX_ATTACK_LIFT_DB));
        assert!(db_to_gain(MAX_ATTACK_LIFT_DB) < 1.40);
    }

    #[test]
    fn silence_and_non_finite_input_remain_finite() {
        let mut finish = ListeningFinish::new(48_000);
        let mut samples = [0.0_f32, 0.0, f32::NAN, f32::INFINITY];
        finish.process_interleaved(&mut samples);
        assert!(samples.iter().all(|sample| sample.is_finite()));
    }
}

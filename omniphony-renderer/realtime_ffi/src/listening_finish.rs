//! Listener-facing post-render finishing controls for the Windows Current path.
//!
//! This stage deliberately runs after the headphone EQ and before the existing
//! lookahead peak guard. It is not a second spatializer: both ears receive the
//! same time-varying enhancement gain, so ILD/IPD/HRTF relationships are not
//! widened, decorrelated, delayed, or otherwise re-authored here.
//!
//! The single `Noire X Enhancement` switch is intentionally conservative. It
//! uses linked fast/slow envelopes as a production-style transient designer:
//! short attacks earn a little extra crest while falling/sustain regions receive
//! a much smaller relief. A lightweight three-band sidechain makes the detector
//! frequency-aware without filtering the audible output: low/mid musical attack
//! energy keeps strong authority while high-frequency-only energy receives less
//! influence. The resulting gain is still one broadband linked gain for both
//! ears, preserving the finished binaural relationship.
//! Output trim is a separate user preference and is still bounded by the final
//! peak guard owned by `lib.rs`.

use std::env;
use std::f32::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const SETTING_POLL_MS: u64 = 500;
const ENHANCEMENT_FILE_NAME: &str = "noire-x-enhancement.txt";
const OUTPUT_TRIM_FILE_NAME: &str = "output-trim.txt";

const FAST_ENVELOPE_MS: f32 = 2.5;
const SLOW_ENVELOPE_MS: f32 = 42.0;
const MAX_ATTACK_LIFT_DB: f32 = 1.15;
const MAX_SUSTAIN_RELIEF_DB: f32 = 0.22;
const TRANSIENT_FULL_SCALE: f32 = 1.35;
const SUSTAIN_FULL_SCALE: f32 = 0.65;
const DEFAULT_OUTPUT_TRIM_DB: f32 = 1.5;

const DETECTOR_BANDS: usize = 3;
const DETECTOR_LOW_CUTOFF_HZ: f32 = 280.0;
const DETECTOR_HIGH_CUTOFF_HZ: f32 = 4_000.0;
const DETECTOR_ATTACK_BAND_WEIGHTS: [f32; DETECTOR_BANDS] = [0.95, 1.15, 0.15];
const DETECTOR_SUSTAIN_BAND_WEIGHTS: [f32; DETECTOR_BANDS] = [0.95, 1.05, 0.55];
const BROADBAND_ATTACK_BLEND: f32 = 0.20;
const SUBBAND_ATTACK_BLEND: f32 = 0.80;
const BROADBAND_SUSTAIN_BLEND: f32 = 0.50;
const SUBBAND_SUSTAIN_BLEND: f32 = 0.50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EnhancementPreset {
    Off,
    On,
}

impl EnhancementPreset {
    fn from_text(text: &str) -> Self {
        match text.trim().to_ascii_lowercase().as_str() {
            "0" | "off" | "false" | "disabled" | "none" => Self::Off,
            _ => Self::On,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum OutputTrim {
    Flat,
    Plus1_5Db,
}

impl OutputTrim {
    fn from_text(text: &str) -> Self {
        match text.trim().to_ascii_lowercase().as_str() {
            "0" | "0db" | "flat" | "off" | "false" | "disabled" => Self::Flat,
            _ => Self::Plus1_5Db,
        }
    }

    fn gain(self) -> f32 {
        match self {
            Self::Flat => 1.0,
            Self::Plus1_5Db => db_to_gain(DEFAULT_OUTPUT_TRIM_DB),
        }
    }
}

pub(crate) struct ListeningFinish {
    enhancement: EnhancementPreset,
    output_trim: OutputTrim,
    enhancement_path: PathBuf,
    output_trim_path: PathBuf,
    last_setting_check: Instant,
    fast_energy: f32,
    slow_energy: f32,
    band_fast_energy: [f32; DETECTOR_BANDS],
    band_slow_energy: [f32; DETECTOR_BANDS],
    detector_low_left: f32,
    detector_upper_left: f32,
    detector_low_right: f32,
    detector_upper_right: f32,
    fast_alpha: f32,
    slow_alpha: f32,
    detector_low_alpha: f32,
    detector_upper_alpha: f32,
}

impl ListeningFinish {
    pub(crate) fn new(sample_rate_hz: u32) -> Self {
        let root = omniphony_state_root();
        let enhancement_path = root.join(ENHANCEMENT_FILE_NAME);
        let output_trim_path = root.join(OUTPUT_TRIM_FILE_NAME);
        let sample_rate = sample_rate_hz.max(1) as f32;
        Self {
            enhancement: read_enhancement(&enhancement_path),
            output_trim: read_output_trim(&output_trim_path),
            enhancement_path,
            output_trim_path,
            last_setting_check: Instant::now(),
            fast_energy: 0.0,
            slow_energy: 0.0,
            band_fast_energy: [0.0; DETECTOR_BANDS],
            band_slow_energy: [0.0; DETECTOR_BANDS],
            detector_low_left: 0.0,
            detector_upper_left: 0.0,
            detector_low_right: 0.0,
            detector_upper_right: 0.0,
            fast_alpha: one_pole_alpha(FAST_ENVELOPE_MS, sample_rate),
            slow_alpha: one_pole_alpha(SLOW_ENVELOPE_MS, sample_rate),
            detector_low_alpha: one_pole_cutoff_alpha(DETECTOR_LOW_CUTOFF_HZ, sample_rate),
            detector_upper_alpha: one_pole_cutoff_alpha(DETECTOR_HIGH_CUTOFF_HZ, sample_rate),
        }
    }

    fn refresh_settings(&mut self) {
        if self.last_setting_check.elapsed() < Duration::from_millis(SETTING_POLL_MS) {
            return;
        }
        self.last_setting_check = Instant::now();
        self.enhancement = read_enhancement(&self.enhancement_path);
        self.output_trim = read_output_trim(&self.output_trim_path);
    }

    pub(crate) fn process_interleaved(&mut self, samples: &mut [f32]) {
        self.refresh_settings();
        let trim = self.output_trim.gain();

        for frame in samples.chunks_exact_mut(2) {
            let mut left = finite_or_zero(frame[0]);
            let mut right = finite_or_zero(frame[1]);

            if self.enhancement == EnhancementPreset::On {
                let linked_energy = 0.5 * (left * left + right * right);
                self.fast_energy += self.fast_alpha * (linked_energy - self.fast_energy);
                self.slow_energy += self.slow_alpha * (linked_energy - self.slow_energy);

                let base_transient = normalized_rise(
                    self.fast_energy,
                    self.slow_energy,
                    TRANSIENT_FULL_SCALE,
                );
                let base_sustain = normalized_fall(
                    self.fast_energy,
                    self.slow_energy,
                    SUSTAIN_FULL_SCALE,
                );

                // The sidechain only is split into broad low / mid / high
                // regions. The audible binaural signal never passes through
                // these filters. Per-band envelope motion is normalized by the
                // energy share of that band so tiny crossover leakage cannot
                // cast a full-strength transient vote.
                let left_bands = split_detector_bands(
                    left,
                    &mut self.detector_low_left,
                    &mut self.detector_upper_left,
                    self.detector_low_alpha,
                    self.detector_upper_alpha,
                );
                let right_bands = split_detector_bands(
                    right,
                    &mut self.detector_low_right,
                    &mut self.detector_upper_right,
                    self.detector_low_alpha,
                    self.detector_upper_alpha,
                );

                let mut band_transients = [0.0_f32; DETECTOR_BANDS];
                let mut band_sustains = [0.0_f32; DETECTOR_BANDS];
                for band in 0..DETECTOR_BANDS {
                    let band_energy = 0.5
                        * (left_bands[band] * left_bands[band]
                            + right_bands[band] * right_bands[band]);
                    self.band_fast_energy[band] +=
                        self.fast_alpha * (band_energy - self.band_fast_energy[band]);
                    self.band_slow_energy[band] +=
                        self.slow_alpha * (band_energy - self.band_slow_energy[band]);
                    band_transients[band] = normalized_rise(
                        self.band_fast_energy[band],
                        self.band_slow_energy[band],
                        TRANSIENT_FULL_SCALE,
                    );
                    band_sustains[band] = normalized_fall(
                        self.band_fast_energy[band],
                        self.band_slow_energy[band],
                        SUSTAIN_FULL_SCALE,
                    );
                }

                let fast_sum = self.band_fast_energy.iter().sum::<f32>() + 1.0e-12;
                let slow_sum = self.band_slow_energy.iter().sum::<f32>() + 1.0e-12;
                let mut subband_transient = 0.0_f32;
                let mut subband_sustain = 0.0_f32;
                for band in 0..DETECTOR_BANDS {
                    let attack_share = self.band_fast_energy[band] / fast_sum;
                    let sustain_share = self.band_slow_energy[band] / slow_sum;
                    subband_transient += attack_share
                        * DETECTOR_ATTACK_BAND_WEIGHTS[band]
                        * band_transients[band];
                    subband_sustain += sustain_share
                        * DETECTOR_SUSTAIN_BAND_WEIGHTS[band]
                        * band_sustains[band];
                }

                // Keep some broadband authority so the accepted detector does
                // not disappear inside the new sidechain. The sub-band term is
                // dominant enough to stop high-frequency-only material from
                // driving the enhancer as strongly as low/mid musical attacks.
                let transient = (BROADBAND_ATTACK_BLEND * base_transient
                    + SUBBAND_ATTACK_BLEND * subband_transient)
                    .clamp(0.0, 1.0);
                let sustain = (BROADBAND_SUSTAIN_BLEND * base_sustain
                    + SUBBAND_SUSTAIN_BLEND * subband_sustain)
                    .clamp(0.0, 1.0);

                let gain_db = MAX_ATTACK_LIFT_DB * transient - MAX_SUSTAIN_RELIEF_DB * sustain;
                let linked_gain = db_to_gain(gain_db);
                left *= linked_gain;
                right *= linked_gain;
            } else {
                // Keep detector history quiet while bypassed so enabling the
                // effect does not release a stale envelope into the first block.
                self.reset_detector_history();
            }

            frame[0] = finite_or_zero(left * trim);
            frame[1] = finite_or_zero(right * trim);
        }
    }

    fn reset_detector_history(&mut self) {
        self.fast_energy = 0.0;
        self.slow_energy = 0.0;
        self.band_fast_energy = [0.0; DETECTOR_BANDS];
        self.band_slow_energy = [0.0; DETECTOR_BANDS];
        self.detector_low_left = 0.0;
        self.detector_upper_left = 0.0;
        self.detector_low_right = 0.0;
        self.detector_upper_right = 0.0;
    }
}

fn split_detector_bands(
    sample: f32,
    low_state: &mut f32,
    upper_state: &mut f32,
    low_alpha: f32,
    upper_alpha: f32,
) -> [f32; DETECTOR_BANDS] {
    *low_state += low_alpha * (sample - *low_state);
    *upper_state += upper_alpha * (sample - *upper_state);
    let low = *low_state;
    let mid = *upper_state - low;
    let high = sample - *upper_state;
    [low, mid, high]
}

fn normalized_rise(fast: f32, slow: f32, full_scale: f32) -> f32 {
    let positive_rise = (fast - slow).max(0.0);
    let relative_rise = positive_rise / (slow + 1.0e-7);
    (relative_rise / full_scale).clamp(0.0, 1.0)
}

fn normalized_fall(fast: f32, slow: f32, full_scale: f32) -> f32 {
    let positive_fall = (slow - fast).max(0.0);
    let relative_fall = positive_fall / (slow + 1.0e-7);
    (relative_fall / full_scale).clamp(0.0, 1.0)
}

fn one_pole_alpha(time_ms: f32, sample_rate_hz: f32) -> f32 {
    1.0 - (-1.0 / (0.001 * time_ms.max(0.01) * sample_rate_hz.max(1.0))).exp()
}

fn one_pole_cutoff_alpha(cutoff_hz: f32, sample_rate_hz: f32) -> f32 {
    let sample_rate = sample_rate_hz.max(1.0);
    let cutoff = cutoff_hz.clamp(1.0, sample_rate * 0.45);
    1.0 - (-2.0 * PI * cutoff / sample_rate).exp()
}

fn read_enhancement(path: &Path) -> EnhancementPreset {
    fs::read_to_string(path)
        .map(|text| EnhancementPreset::from_text(&text))
        .unwrap_or(EnhancementPreset::On)
}

fn read_output_trim(path: &Path) -> OutputTrim {
    fs::read_to_string(path)
        .map(|text| OutputTrim::from_text(&text))
        .unwrap_or(OutputTrim::Plus1_5Db)
}

fn omniphony_state_root() -> PathBuf {
    env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
        .join("Omniphony")
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

    fn test_finish(enhancement: EnhancementPreset, trim: OutputTrim) -> ListeningFinish {
        let mut finish = ListeningFinish::new(48_000);
        finish.enhancement = enhancement;
        finish.output_trim = trim;
        finish.last_setting_check = Instant::now();
        finish
    }

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

        let mut finish = test_finish(EnhancementPreset::On, OutputTrim::Flat);
        finish.process_interleaved(&mut samples);
        samples
            .iter()
            .fold(0.0_f32, |peak, &sample| peak.max(sample.abs()))
            / amplitude
    }

    #[test]
    fn bypass_with_flat_trim_is_bit_exact_for_finite_audio() {
        let mut finish = test_finish(EnhancementPreset::Off, OutputTrim::Flat);
        let mut samples = [0.125_f32, -0.25, 0.5, -0.75, 0.0, -0.0];
        let before = samples.map(f32::to_bits);
        finish.process_interleaved(&mut samples);
        assert_eq!(before, samples.map(f32::to_bits));
    }

    #[test]
    fn output_trim_is_the_requested_one_point_five_db() {
        let gain = OutputTrim::Plus1_5Db.gain();
        let expected = 10.0_f32.powf(1.5 / 20.0);
        assert!((gain - expected).abs() < 1.0e-7);
    }

    #[test]
    fn enhancement_uses_one_linked_gain_for_both_ears() {
        let mut finish = test_finish(EnhancementPreset::On, OutputTrim::Flat);
        let mut samples = vec![0.0_f32; 256 * 2];
        samples[120] = 0.4;
        samples[121] = -0.2;
        finish.process_interleaved(&mut samples);
        let left = samples[120];
        let right = samples[121];
        assert!(left > 0.4);
        assert!(right < -0.2);
        assert!((left / right - (0.4 / -0.2)).abs() < 1.0e-5);
    }

    #[test]
    fn attack_lift_and_sustain_relief_remain_conservative() {
        assert!(MAX_ATTACK_LIFT_DB > 1.0);
        assert!(MAX_ATTACK_LIFT_DB <= 1.2);
        assert!(MAX_SUSTAIN_RELIEF_DB > 0.18);
        assert!(MAX_SUSTAIN_RELIEF_DB <= 0.25);
        assert!(MAX_SUSTAIN_RELIEF_DB < MAX_ATTACK_LIFT_DB);
    }

    #[test]
    fn high_frequency_only_attack_has_less_detector_authority() {
        let mid_gain = sine_burst_peak_gain(1_000.0);
        let high_gain = sine_burst_peak_gain(10_000.0);
        assert!(mid_gain > high_gain + 0.02);
        assert!(high_gain > 1.0);
    }

    #[test]
    fn enhancement_gain_is_bounded() {
        let mut finish = test_finish(EnhancementPreset::On, OutputTrim::Flat);
        let mut samples = vec![0.0_f32; 1024 * 2];
        for frame in (0..1024).step_by(64) {
            samples[frame * 2] = 1.0;
            samples[frame * 2 + 1] = -1.0;
        }
        finish.process_interleaved(&mut samples);
        let max = samples.iter().fold(0.0_f32, |a, &b| a.max(b.abs()));
        assert!(max <= db_to_gain(MAX_ATTACK_LIFT_DB) + 1.0e-5);
    }

    #[test]
    fn falling_envelope_gets_small_sustain_relief() {
        let mut finish = test_finish(EnhancementPreset::On, OutputTrim::Flat);
        let steady_frames = 4096usize;
        let tail_frames = 1024usize;
        let mut samples = vec![0.0_f32; (steady_frames + tail_frames) * 2];

        for frame in 0..steady_frames {
            samples[frame * 2] = 0.5;
            samples[frame * 2 + 1] = -0.25;
        }
        for frame in steady_frames..(steady_frames + tail_frames) {
            samples[frame * 2] = 0.15;
            samples[frame * 2 + 1] = -0.075;
        }

        finish.process_interleaved(&mut samples);

        let mut min_gain = 1.0_f32;
        for frame in steady_frames..(steady_frames + tail_frames) {
            let gain = samples[frame * 2] / 0.15;
            min_gain = min_gain.min(gain);
            let right_gain = samples[frame * 2 + 1] / -0.075;
            assert!((gain - right_gain).abs() < 1.0e-5);
        }

        assert!(min_gain < 1.0);
        assert!(min_gain >= db_to_gain(-MAX_SUSTAIN_RELIEF_DB) - 1.0e-5);
    }

    #[test]
    fn bypass_clears_all_frequency_detector_history() {
        let mut finish = test_finish(EnhancementPreset::On, OutputTrim::Flat);
        let mut samples = vec![0.25_f32; 512 * 2];
        finish.process_interleaved(&mut samples);
        assert!(finish.band_fast_energy.iter().any(|&energy| energy > 0.0));

        finish.enhancement = EnhancementPreset::Off;
        finish.last_setting_check = Instant::now();
        let mut silence = [0.0_f32, 0.0];
        finish.process_interleaved(&mut silence);

        assert_eq!(finish.fast_energy, 0.0);
        assert_eq!(finish.slow_energy, 0.0);
        assert_eq!(finish.band_fast_energy, [0.0; DETECTOR_BANDS]);
        assert_eq!(finish.band_slow_energy, [0.0; DETECTOR_BANDS]);
        assert_eq!(finish.detector_low_left, 0.0);
        assert_eq!(finish.detector_upper_left, 0.0);
        assert_eq!(finish.detector_low_right, 0.0);
        assert_eq!(finish.detector_upper_right, 0.0);
    }

    #[test]
    fn silence_and_non_finite_input_remain_finite() {
        let mut finish = test_finish(EnhancementPreset::On, OutputTrim::Plus1_5Db);
        let mut samples = [0.0_f32, 0.0, f32::NAN, f32::INFINITY];
        finish.process_interleaved(&mut samples);
        assert!(samples.iter().all(|sample| sample.is_finite()));
    }
}

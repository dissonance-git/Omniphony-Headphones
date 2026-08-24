//! Dan Clark Noire X / primary-listener output calibration for the Windows path.
//!
//! The shared Omniphony headphone / renderer EQ is exposed simply as On/Off.
//! Listener-specific right-ear compensation is part of the fixed personal
//! calibration and remains active regardless of the shared EQ setting.
//!
//! The retired DTS-era curve remains only as listening-history evidence. Its
//! broad upper-mid / treble suppression taught an important constraint: reduce
//! glare without dimming the open top end that makes Current feel clear.
//!
//! The active curve therefore spends most of its extra energy below the kick
//! region, preserves a small 3.5-5 kHz relaxation, restores the upper treble,
//! and keeps enough preamp headroom for the downstream lookahead peak guard.

use std::env;
use std::f64::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const TUNED_GLOBAL_PREAMP_DB: f64 = -4.0;
const RIGHT_PREAMP_DB: f64 = -0.4;
const RIGHT_DELAY_MS: f64 = 0.02;
const SETTING_POLL_MS: u64 = 500;
const EQ_PRESET_FILE_NAME: &str = "eq-preset.txt";
const LEGACY_EQ_FILE_NAME: &str = "personal-eq.txt";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EqPreset {
    Off,
    On,
}

impl EqPreset {
    fn from_text(text: &str) -> Self {
        match text.trim().to_ascii_lowercase().as_str() {
            "0" | "off" | "false" | "disabled" | "none" => Self::Off,
            // Every historical enabled spelling migrates to the one supported
            // tuned curve. The DTS-era curve is no longer a runtime mode.
            _ => Self::On,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FilterKind {
    HighPass,
    Peaking,
    LowShelf,
    HighShelf,
}

#[derive(Clone, Copy, Debug)]
struct FilterSpec {
    kind: FilterKind,
    frequency_hz: f64,
    gain_db: f64,
    q: f64,
}

impl FilterSpec {
    const fn high_pass(frequency_hz: f64, q: f64) -> Self {
        Self {
            kind: FilterKind::HighPass,
            frequency_hz,
            gain_db: 0.0,
            q,
        }
    }

    const fn peaking(frequency_hz: f64, gain_db: f64, q: f64) -> Self {
        Self {
            kind: FilterKind::Peaking,
            frequency_hz,
            gain_db,
            q,
        }
    }

    const fn low_shelf(frequency_hz: f64, gain_db: f64, q: f64) -> Self {
        Self {
            kind: FilterKind::LowShelf,
            frequency_hz,
            gain_db,
            q,
        }
    }

    const fn high_shelf(frequency_hz: f64, gain_db: f64, q: f64) -> Self {
        Self {
            kind: FilterKind::HighShelf,
            frequency_hz,
            gain_db,
            q,
        }
    }
}

// Working Omniphony listening baseline. The extra weight is intentionally spent
// below the obvious kick/body region: the 32 Hz shelf extends the floor while
// the 60 Hz term is modest and 150-260 Hz stays controlled. The former DTS-era
// curve established that large 4-8 kHz cuts destroy openness, so the two glare
// notches are now shallow and the top octave is allowed to breathe.
const TUNED_SHARED_FILTERS: [FilterSpec; 12] = [
    FilterSpec::high_pass(11.0, 0.65),
    FilterSpec::low_shelf(32.0, 5.5, 0.55),
    FilterSpec::peaking(60.0, 1.0, 0.70),
    FilterSpec::peaking(150.0, 0.2, 0.80),
    FilterSpec::peaking(260.0, -0.4, 0.85),
    FilterSpec::peaking(520.0, 0.5, 0.75),
    FilterSpec::peaking(1_100.0, 0.6, 0.75),
    FilterSpec::peaking(1_900.0, 0.35, 0.90),
    FilterSpec::peaking(3_000.0, -0.35, 0.80),
    FilterSpec::peaking(3_900.0, -0.8, 0.90),
    FilterSpec::peaking(5_000.0, -0.6, 1.10),
    FilterSpec::high_shelf(10_000.0, 0.50, 0.70),
];

// Listener-specific right-ear compensation. This is intentionally permanent:
// EQ Off is a tonal A/B control, not a way to erase the listener calibration.
const RIGHT_FILTERS: [FilterSpec; 3] = [
    FilterSpec::peaking(180.0, -0.3, 0.9),
    FilterSpec::peaking(3_000.0, -1.1, 1.0),
    FilterSpec::high_shelf(6_200.0, -0.3, 0.7),
];

#[derive(Clone, Copy, Debug)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Biquad {
    fn new(spec: FilterSpec, sample_rate_hz: u32) -> Self {
        let sample_rate = sample_rate_hz.max(1) as f64;
        let a = 10.0f64.powf(spec.gain_db / 40.0);
        let mut frequency_hz = spec.frequency_hz;

        // Equalizer APO treats ordinary LS/HS Fc as a corner frequency. When
        // Q is supplied, it first derives S only for the corner->center
        // frequency conversion, then still uses Q in the RBJ alpha term.
        if matches!(spec.kind, FilterKind::LowShelf | FilterKind::HighShelf) {
            let q = spec.q.max(f64::EPSILON);
            let s = 1.0 / (((1.0 / (q * q) - 2.0) / (a + 1.0 / a)) + 1.0);
            let center_factor = 10.0f64.powf(spec.gain_db.abs() / 80.0 / s);
            match spec.kind {
                FilterKind::LowShelf => frequency_hz *= center_factor,
                FilterKind::HighShelf => frequency_hz /= center_factor,
                _ => {}
            }
        }

        frequency_hz = frequency_hz.clamp(1.0e-6, sample_rate * 0.499_999);
        let omega = 2.0 * PI * frequency_hz / sample_rate;
        let sn = omega.sin();
        let cs = omega.cos();
        let alpha = sn / (2.0 * spec.q.max(f64::EPSILON));
        let beta = 2.0 * a.sqrt() * alpha;

        let (b0, b1, b2, a0, a1, a2) = match spec.kind {
            FilterKind::HighPass => (
                (1.0 + cs) / 2.0,
                -(1.0 + cs),
                (1.0 + cs) / 2.0,
                1.0 + alpha,
                -2.0 * cs,
                1.0 - alpha,
            ),
            FilterKind::Peaking => (
                1.0 + alpha * a,
                -2.0 * cs,
                1.0 - alpha * a,
                1.0 + alpha / a,
                -2.0 * cs,
                1.0 - alpha / a,
            ),
            FilterKind::LowShelf => (
                a * ((a + 1.0) - (a - 1.0) * cs + beta),
                2.0 * a * ((a - 1.0) - (a + 1.0) * cs),
                a * ((a + 1.0) - (a - 1.0) * cs - beta),
                (a + 1.0) + (a - 1.0) * cs + beta,
                -2.0 * ((a - 1.0) + (a + 1.0) * cs),
                (a + 1.0) + (a - 1.0) * cs - beta,
            ),
            FilterKind::HighShelf => (
                a * ((a + 1.0) + (a - 1.0) * cs + beta),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * cs),
                a * ((a + 1.0) + (a - 1.0) * cs - beta),
                (a + 1.0) - (a - 1.0) * cs + beta,
                2.0 * ((a - 1.0) - (a + 1.0) * cs),
                (a + 1.0) - (a - 1.0) * cs - beta,
            ),
        };

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let x = if input.is_finite() { input as f64 } else { 0.0 };
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = if y.abs() < 1.0e-30 { 0.0 } else { y };
        self.y1 as f32
    }
}

struct SampleDelay {
    samples: Vec<f32>,
    offset: usize,
}

impl SampleDelay {
    fn new(sample_rate_hz: u32, delay_ms: f64) -> Self {
        let count = ((sample_rate_hz as f64 * delay_ms / 1000.0) + 0.5).floor() as usize;
        Self {
            samples: vec![0.0; count],
            offset: 0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        if self.samples.is_empty() {
            return input;
        }
        let output = self.samples[self.offset];
        self.samples[self.offset] = input;
        self.offset += 1;
        if self.offset == self.samples.len() {
            self.offset = 0;
        }
        output
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.samples.len()
    }
}

pub(crate) struct NoireXPersonalEq {
    sample_rate_hz: u32,
    preset: EqPreset,
    eq_setting_path: PathBuf,
    legacy_eq_setting_path: PathBuf,
    last_setting_check: Instant,
    global_gain: f32,
    shared: Vec<[Biquad; 2]>,
    right_gain: f32,
    right_only: Vec<Biquad>,
    right_delay: SampleDelay,
}

impl NoireXPersonalEq {
    pub(crate) fn new(sample_rate_hz: u32) -> Self {
        let root = omniphony_state_root();
        let eq_setting_path = root.join(EQ_PRESET_FILE_NAME);
        let legacy_eq_setting_path = root.join(LEGACY_EQ_FILE_NAME);
        let preset = read_eq_preset(&eq_setting_path, &legacy_eq_setting_path);
        let (global_gain, shared) = build_eq_preset(preset, sample_rate_hz);
        Self {
            sample_rate_hz,
            preset,
            eq_setting_path,
            legacy_eq_setting_path,
            last_setting_check: Instant::now(),
            global_gain,
            shared,
            right_gain: db_to_gain(RIGHT_PREAMP_DB),
            right_only: build_right_filters(sample_rate_hz),
            right_delay: SampleDelay::new(sample_rate_hz, RIGHT_DELAY_MS),
        }
    }

    fn refresh_settings(&mut self) {
        if self.last_setting_check.elapsed() < Duration::from_millis(SETTING_POLL_MS) {
            return;
        }
        self.last_setting_check = Instant::now();

        let preset = read_eq_preset(&self.eq_setting_path, &self.legacy_eq_setting_path);
        if preset == self.preset {
            return;
        }

        self.preset = preset;
        let (global_gain, shared) = build_eq_preset(self.preset, self.sample_rate_hz);
        self.global_gain = global_gain;
        self.shared = shared;
    }

    pub(crate) fn process_interleaved(&mut self, samples: &mut [f32]) {
        self.refresh_settings();

        for frame in samples.chunks_exact_mut(2) {
            let mut left = finite_or_zero(frame[0]);
            let mut right = finite_or_zero(frame[1]);

            if self.preset == EqPreset::On {
                left *= self.global_gain;
                right *= self.global_gain;
                for pair in &mut self.shared {
                    left = pair[0].process(left);
                    right = pair[1].process(right);
                }
            }

            right *= self.right_gain;
            for filter in &mut self.right_only {
                right = filter.process(right);
            }
            right = self.right_delay.process(right);

            frame[0] = finite_or_zero(left);
            frame[1] = finite_or_zero(right);
        }
    }
}

fn build_eq_preset(preset: EqPreset, sample_rate_hz: u32) -> (f32, Vec<[Biquad; 2]>) {
    let (preamp_db, specs): (f64, &[FilterSpec]) = match preset {
        EqPreset::Off => (0.0, &[]),
        EqPreset::On => (TUNED_GLOBAL_PREAMP_DB, &TUNED_SHARED_FILTERS),
    };
    let filters = specs
        .iter()
        .map(|&spec| {
            [
                Biquad::new(spec, sample_rate_hz),
                Biquad::new(spec, sample_rate_hz),
            ]
        })
        .collect();
    (db_to_gain(preamp_db), filters)
}

fn build_right_filters(sample_rate_hz: u32) -> Vec<Biquad> {
    RIGHT_FILTERS
        .iter()
        .map(|&spec| Biquad::new(spec, sample_rate_hz))
        .collect()
}

fn omniphony_state_root() -> PathBuf {
    env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
        .join("Omniphony")
}

fn read_eq_preset(path: &Path, legacy_path: &Path) -> EqPreset {
    if let Ok(text) = fs::read_to_string(path) {
        return EqPreset::from_text(&text);
    }
    if let Ok(text) = fs::read_to_string(legacy_path) {
        return EqPreset::from_text(&text);
    }
    EqPreset::On
}

fn db_to_gain(db: f64) -> f32 {
    10.0f64.powf(db / 20.0) as f32
}

fn finite_or_zero(sample: f32) -> f32 {
    if sample.is_finite() { sample } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_parser_collapses_all_enabled_history_to_one_curve() {
        assert_eq!(EqPreset::from_text("0"), EqPreset::Off);
        assert_eq!(EqPreset::from_text("off"), EqPreset::Off);
        assert_eq!(EqPreset::from_text("1"), EqPreset::On);
        assert_eq!(EqPreset::from_text("legacy"), EqPreset::On);
        assert_eq!(EqPreset::from_text("omniphony"), EqPreset::On);
        assert_eq!(EqPreset::from_text("native"), EqPreset::On);
        assert_eq!(EqPreset::from_text("on"), EqPreset::On);
    }

    #[test]
    fn tuned_profile_spends_gain_in_deep_sub_and_preserves_air() {
        assert!(TUNED_SHARED_FILTERS.iter().any(|spec| {
            spec.kind == FilterKind::LowShelf
                && spec.frequency_hz <= 35.0
                && spec.gain_db >= 3.5
        }));
        assert!(TUNED_SHARED_FILTERS.iter().any(|spec| {
            spec.kind == FilterKind::HighShelf
                && spec.frequency_hz >= 9_000.0
                && spec.gain_db > 0.0
        }));
        assert!(TUNED_SHARED_FILTERS
            .iter()
            .filter(|spec| {
                (3_500.0..=8_000.0).contains(&spec.frequency_hz) && spec.gain_db < 0.0
            })
            .all(|spec| spec.gain_db >= -1.0));
    }

    #[test]
    fn tuned_preamp_reserves_headroom_for_the_deep_sub_shelf() {
        assert!(TUNED_GLOBAL_PREAMP_DB <= -3.5);
        assert!(TUNED_GLOBAL_PREAMP_DB >= -4.5);
    }

    #[test]
    fn tuned_profile_prefers_deep_sub_over_midbass_and_midrange() {
        fn left_rms_at(frequency_hz: f64) -> f64 {
            let sample_rate = 48_000u32;
            let frames = sample_rate as usize;
            let mut samples = Vec::with_capacity(frames * 2);
            for frame in 0..frames {
                let sample =
                    (2.0 * PI * frequency_hz * frame as f64 / sample_rate as f64).sin() as f32
                        * 0.1;
                samples.extend_from_slice(&[sample, sample]);
            }

            let mut profile = NoireXPersonalEq::new(sample_rate);
            profile.preset = EqPreset::On;
            let (gain, shared) = build_eq_preset(EqPreset::On, sample_rate);
            profile.global_gain = gain;
            profile.shared = shared;
            profile.process_interleaved(&mut samples);

            let start = frames / 2;
            let energy: f64 = samples[(start * 2)..]
                .chunks_exact(2)
                .map(|frame| (frame[0] as f64).powi(2))
                .sum();
            (energy / (frames - start) as f64).sqrt()
        }

        let deep = left_rms_at(25.0);
        let midbass = left_rms_at(60.0);
        let midrange = left_rms_at(1_000.0);
        assert!(deep > midbass * 1.10, "deep={deep} midbass={midbass}");
        assert!(deep > midrange * 1.40, "deep={deep} midrange={midrange}");
    }

    #[test]
    fn right_compensation_delay_matches_equalizer_apo_rounding_at_48k() {
        let delay = SampleDelay::new(48_000, RIGHT_DELAY_MS);
        assert_eq!(delay.len(), 1);
    }

    #[test]
    fn right_compensation_remains_active_when_headphone_eq_is_off() {
        let mut profile = NoireXPersonalEq::new(48_000);
        profile.preset = EqPreset::Off;
        profile.global_gain = 1.0;
        profile.shared.clear();
        profile.right_only = build_right_filters(48_000);
        profile.right_delay = SampleDelay::new(48_000, RIGHT_DELAY_MS);
        let mut impulse = vec![0.0f32; 16];
        impulse[0] = 1.0;
        impulse[1] = 1.0;
        profile.process_interleaved(&mut impulse);
        assert_eq!(impulse[0], 1.0);
        assert_eq!(impulse[1], 0.0);
        assert!(impulse[3].abs() > 1.0e-6);
    }

    #[test]
    fn peaking_filter_hits_requested_center_gain() {
        let sample_rate = 48_000u32;
        let frequency = 1_000.0;
        let mut filter = Biquad::new(FilterSpec::peaking(frequency, 6.0, 1.0), sample_rate);
        let mut in_energy = 0.0f64;
        let mut out_energy = 0.0f64;
        let frames = sample_rate as usize;
        for frame in 0..frames {
            let sample =
                (2.0 * PI * frequency * frame as f64 / sample_rate as f64).sin() as f32 * 0.1;
            let output = filter.process(sample);
            if frame >= frames / 2 {
                in_energy += (sample as f64).powi(2);
                out_energy += (output as f64).powi(2);
            }
        }
        let gain = (out_energy / in_energy).sqrt();
        let expected = 10.0f64.powf(6.0 / 20.0);
        assert!((gain - expected).abs() < 0.02, "gain={gain} expected={expected}");
    }

    #[test]
    fn hot_tuned_profile_processing_remains_finite() {
        let mut profile = NoireXPersonalEq::new(48_000);
        profile.preset = EqPreset::On;
        let (gain, shared) = build_eq_preset(EqPreset::On, 48_000);
        profile.global_gain = gain;
        profile.shared = shared;
        profile.right_only = build_right_filters(48_000);
        profile.right_delay = SampleDelay::new(48_000, RIGHT_DELAY_MS);
        let mut samples = vec![4.0f32; 48_000 * 2 / 10];
        profile.process_interleaved(&mut samples);
        assert!(samples.iter().all(|sample| sample.is_finite()));
    }
}

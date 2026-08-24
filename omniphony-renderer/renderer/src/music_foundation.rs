//! Small, source-coherent foundation/body enhancement for stereo music.
//!
//! This is deliberately not a second headphone EQ and not a spatial bass path.
//! The finished stereo master remains explicit. The processor emits only the
//! additive delta needed to give the protected master more pressure, body and
//! density before it is linearly combined with Omniphony's spatial support.
//!
//! Design constraints:
//! - no compression, limiting, saturation or dynamics-dependent gain;
//! - no fake LFE and no HRTF rendering of the low-frequency foundation;
//! - identical filter topology in left and right channels so stereo relations
//!   remain intact;
//! - minimum-phase IIR shaping only, with downstream headroom owned by the host.

use std::f32::consts::PI;

const FOUNDATION_LOW_SHELF_HZ: f32 = 60.0;

#[derive(Debug, Clone, Copy)]
pub struct MusicFoundationTuning {
    /// Broad low-frequency pressure / mass.
    pub low_shelf_db: f32,
    /// Coherent kick / upper-bass impact around 110 Hz.
    pub punch_db: f32,
    /// Upper-bass / lower-mid body.
    pub body_db: f32,
    /// Small midrange density correction.
    pub density_db: f32,
    /// Gentle upper-presence relaxation; negative values reduce emphasis.
    pub presence_shelf_db: f32,
}

impl Default for MusicFoundationTuning {
    fn default() -> Self {
        // Physical listening established a stronger invariant than the first
        // conservative pass: Omniphony ON must never feel weaker than OFF in
        // bass pressure, kick weight or drum body. Keep this coherent and
        // non-spatial rather than trying to recover impact with fake LFE or
        // extra room energy. The stronger 110 Hz term is deliberately narrow
        // enough to add kick impact without turning the whole bass range up.
        // Keep the pressure shelf below that term so deeper extension does not
        // become an 80-150 Hz cloud around the protected master.
        Self {
            low_shelf_db: 3.40,
            punch_db: 1.60,
            body_db: 1.20,
            density_db: 0.50,
            presence_shelf_db: -0.35,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn identity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn from_coefficients(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Self {
        let inv_a0 = if a0.abs() > 1.0e-12 { 1.0 / a0 } else { 1.0 };
        Self {
            b0: b0 * inv_a0,
            b1: b1 * inv_a0,
            b2: b2 * inv_a0,
            a1: a1 * inv_a0,
            a2: a2 * inv_a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    fn peaking(sample_rate_hz: u32, frequency_hz: f32, q: f32, gain_db: f32) -> Self {
        if gain_db.abs() < 1.0e-6 {
            return Self::identity();
        }
        let fs = sample_rate_hz.max(1) as f32;
        let f = frequency_hz.clamp(1.0, 0.49 * fs);
        let w0 = 2.0 * PI * f / fs;
        let cos_w0 = w0.cos();
        let alpha = w0.sin() / (2.0 * q.max(0.05));
        let a = 10.0_f32.powf(gain_db / 40.0);
        Self::from_coefficients(
            1.0 + alpha * a,
            -2.0 * cos_w0,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos_w0,
            1.0 - alpha / a,
        )
    }

    fn low_shelf(sample_rate_hz: u32, frequency_hz: f32, gain_db: f32) -> Self {
        if gain_db.abs() < 1.0e-6 {
            return Self::identity();
        }
        let fs = sample_rate_hz.max(1) as f32;
        let f = frequency_hz.clamp(1.0, 0.49 * fs);
        let w0 = 2.0 * PI * f / fs;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let a = 10.0_f32.powf(gain_db / 40.0);
        let alpha = 0.5 * sin_w0 * (2.0_f32).sqrt();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
        Self::from_coefficients(
            a * ((a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0),
            a * ((a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha),
            (a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha,
            -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0),
            (a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha,
        )
    }

    fn high_shelf(sample_rate_hz: u32, frequency_hz: f32, gain_db: f32) -> Self {
        if gain_db.abs() < 1.0e-6 {
            return Self::identity();
        }
        let fs = sample_rate_hz.max(1) as f32;
        let f = frequency_hz.clamp(1.0, 0.49 * fs);
        let w0 = 2.0 * PI * f / fs;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let a = 10.0_f32.powf(gain_db / 40.0);
        let alpha = 0.5 * sin_w0 * (2.0_f32).sqrt();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
        Self::from_coefficients(
            a * ((a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0),
            a * ((a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha),
            (a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha,
            2.0 * ((a - 1.0) - (a + 1.0) * cos_w0),
            (a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha,
        )
    }

    #[inline]
    fn process(&mut self, sample: f32) -> f32 {
        let out = self.b0 * sample + self.z1;
        self.z1 = self.b1 * sample - self.a1 * out + self.z2;
        self.z2 = self.b2 * sample - self.a2 * out;
        out
    }
}

struct ChannelFoundation {
    pressure: Biquad,
    punch: Biquad,
    body: Biquad,
    density: Biquad,
    presence: Biquad,
}

impl ChannelFoundation {
    fn new(sample_rate_hz: u32, tuning: MusicFoundationTuning) -> Self {
        Self {
            pressure: Biquad::low_shelf(
                sample_rate_hz,
                FOUNDATION_LOW_SHELF_HZ,
                tuning.low_shelf_db,
            ),
            punch: Biquad::peaking(sample_rate_hz, 110.0, 0.80, tuning.punch_db),
            body: Biquad::peaking(sample_rate_hz, 240.0, 0.80, tuning.body_db),
            density: Biquad::peaking(sample_rate_hz, 800.0, 0.70, tuning.density_db),
            presence: Biquad::high_shelf(sample_rate_hz, 4_500.0, tuning.presence_shelf_db),
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        let x = self.pressure.process(sample);
        let x = self.punch.process(x);
        let x = self.body.process(x);
        let x = self.density.process(x);
        self.presence.process(x)
    }
}

/// Emits only the additive stereo delta. The authoritative master remains a
/// separate path and is summed with this delta later in the host.
pub struct MusicFoundationProcessor {
    left: ChannelFoundation,
    right: ChannelFoundation,
}

impl MusicFoundationProcessor {
    pub fn new(sample_rate_hz: u32) -> Self {
        Self::with_tuning(sample_rate_hz, MusicFoundationTuning::default())
    }

    pub fn with_tuning(sample_rate_hz: u32, tuning: MusicFoundationTuning) -> Self {
        Self {
            left: ChannelFoundation::new(sample_rate_hz, tuning),
            right: ChannelFoundation::new(sample_rate_hz, tuning),
        }
    }

    pub fn process_interleaved_delta(&mut self, input: &[f32]) -> Vec<f32> {
        if input.len() < 2 || input.len() % 2 != 0 {
            return Vec::new();
        }
        let mut delta = Vec::with_capacity(input.len());
        for frame in input.chunks_exact(2) {
            let left = if frame[0].is_finite() { frame[0] } else { 0.0 };
            let right = if frame[1].is_finite() { frame[1] } else { 0.0 };
            delta.push(self.left.process(left) - left);
            delta.push(self.right.process(right) - right);
        }
        delta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(frequency_hz: f32, frames: usize) -> Vec<f32> {
        let mut out = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let x = (2.0 * PI * frequency_hz * i as f32 / 48_000.0).sin() * 0.25;
            out.extend_from_slice(&[x, x]);
        }
        out
    }

    fn rms(samples: &[f32]) -> f32 {
        let sum = samples.iter().map(|x| x * x).sum::<f32>();
        (sum / samples.len().max(1) as f32).sqrt()
    }

    #[test]
    fn default_foundation_adds_low_frequency_mass_without_channel_skew() {
        let input = sine(60.0, 16_384);
        let mut p = MusicFoundationProcessor::new(48_000);
        let delta = p.process_interleaved_delta(&input);
        let shaped: Vec<f32> = input.iter().zip(delta.iter()).map(|(a, b)| a + b).collect();
        let start = 4_096 * 2;
        assert!(rms(&shaped[start..]) > rms(&input[start..]));
        for frame in delta[start..].chunks_exact(2) {
            assert!((frame[0] - frame[1]).abs() < 1.0e-6);
        }
    }

    #[test]
    fn default_foundation_adds_coherent_kick_punch_at_110_hz() {
        let input = sine(110.0, 16_384);
        let mut p = MusicFoundationProcessor::new(48_000);
        let delta = p.process_interleaved_delta(&input);
        let shaped: Vec<f32> = input.iter().zip(delta.iter()).map(|(a, b)| a + b).collect();
        let start = 4_096 * 2;
        assert!(rms(&shaped[start..]) > rms(&input[start..]) * 1.20);
        for frame in delta[start..].chunks_exact(2) {
            assert!((frame[0] - frame[1]).abs() < 1.0e-6);
        }
    }

    #[test]
    fn default_foundation_favors_deep_pressure_over_midbass_fog() {
        let deep = sine(25.0, 48_000);
        let upper_bass = sine(90.0, 48_000);
        let mut deep_processor = MusicFoundationProcessor::new(48_000);
        let mut upper_processor = MusicFoundationProcessor::new(48_000);
        let deep_delta = deep_processor.process_interleaved_delta(&deep);
        let upper_delta = upper_processor.process_interleaved_delta(&upper_bass);
        let deep_shaped: Vec<f32> = deep
            .iter()
            .zip(deep_delta.iter())
            .map(|(source, delta)| source + delta)
            .collect();
        let upper_shaped: Vec<f32> = upper_bass
            .iter()
            .zip(upper_delta.iter())
            .map(|(source, delta)| source + delta)
            .collect();
        let start = 8_192 * 2;
        let deep_gain = rms(&deep_shaped[start..]) / rms(&deep[start..]);
        let upper_gain = rms(&upper_shaped[start..]) / rms(&upper_bass[start..]);
        assert!(deep_gain > upper_gain * 1.10, "deep={deep_gain} upper={upper_gain}");
    }

    #[test]
    fn default_foundation_adds_body_at_240_hz() {
        let input = sine(240.0, 16_384);
        let mut p = MusicFoundationProcessor::new(48_000);
        let delta = p.process_interleaved_delta(&input);
        let shaped: Vec<f32> = input.iter().zip(delta.iter()).map(|(a, b)| a + b).collect();
        let start = 4_096 * 2;
        assert!(rms(&shaped[start..]) > rms(&input[start..]) * 1.08);
    }

    #[test]
    fn foundation_preserves_left_to_right_body_motion() {
        let frames_per_side = 8_192;
        let mut input = Vec::with_capacity(frames_per_side * 4);
        for i in 0..frames_per_side {
            let x = (2.0 * PI * 180.0 * i as f32 / 48_000.0).sin() * 0.25;
            input.extend_from_slice(&[x, 0.20 * x]);
        }
        for i in 0..frames_per_side {
            let x = (2.0 * PI * 180.0 * i as f32 / 48_000.0).sin() * 0.25;
            input.extend_from_slice(&[0.20 * x, x]);
        }

        let mut p = MusicFoundationProcessor::new(48_000);
        let delta = p.process_interleaved_delta(&input);
        let shaped: Vec<f32> = input.iter().zip(delta.iter()).map(|(a, b)| a + b).collect();

        // Ignore filter settling after each pan change. The dominant side must
        // remain dominant by a wide margin in both directions: foundation adds
        // weight, but it may not freeze or mono-ize authored stereo motion.
        let settle = 2_048;
        let mut first_l = 0.0;
        let mut first_r = 0.0;
        for frame in shaped[(settle * 2)..(frames_per_side * 2)].chunks_exact(2) {
            first_l += frame[0] * frame[0];
            first_r += frame[1] * frame[1];
        }
        let second_start = (frames_per_side + settle) * 2;
        let mut second_l = 0.0;
        let mut second_r = 0.0;
        for frame in shaped[second_start..].chunks_exact(2) {
            second_l += frame[0] * frame[0];
            second_r += frame[1] * frame[1];
        }
        assert!(first_l > first_r * 10.0);
        assert!(second_r > second_l * 10.0);
    }

    #[test]
    fn default_foundation_relaxes_upper_presence_slightly() {
        let input = sine(10_000.0, 16_384);
        let mut p = MusicFoundationProcessor::new(48_000);
        let delta = p.process_interleaved_delta(&input);
        let shaped: Vec<f32> = input.iter().zip(delta.iter()).map(|(a, b)| a + b).collect();
        let start = 4_096 * 2;
        assert!(rms(&shaped[start..]) < rms(&input[start..]));
    }

    #[test]
    fn zero_tuning_is_effectively_transparent() {
        let tuning = MusicFoundationTuning {
            low_shelf_db: 0.0,
            punch_db: 0.0,
            body_db: 0.0,
            density_db: 0.0,
            presence_shelf_db: 0.0,
        };
        let input = sine(997.0, 4_096);
        let mut p = MusicFoundationProcessor::with_tuning(48_000, tuning);
        let delta = p.process_interleaved_delta(&input);
        assert!(delta.iter().all(|x| x.abs() < 1.0e-7));
    }
}

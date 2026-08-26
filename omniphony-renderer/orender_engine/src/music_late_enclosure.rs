//! First-order spherical measured-HRTF late enclosure for the retained Current music path.
//!
//! The protected stereo master never enters this module. It receives only the
//! derived full-sphere support field, exactly like `music_early_reflections`.
//!
//! The inherited binaural FDN ends as two decorrelated ear returns. That is a
//! useful distance/closure cue, but after the network has collapsed to L/R it
//! can no longer say "this late energy arrived from above / in front / behind".
//! This candidate keeps the same bounded 8-line Householder network and short
//! RT60, but reads four mutually-orthogonal late buses as a first-order spherical
//! field W/X/Y/Z. The field is decoded energy-neutrally to six virtual directions
//! ±X / ±Y / ±Z, then rendered through the same embedded SAF/KEMAR HRTF family
//! used elsewhere in the Current support renderer.
//!
//! This is deliberately different from six independent directional buses. A
//! first-order spherical field is closed under arbitrary rotation: W is invariant
//! and X/Y/Z rotate as a vector. That makes the late representation compatible
//! with future head tracking and matches the low-order Ambisonic strategy used
//! by established binaural renderers, while retaining exactly the same six HRTF
//! virtual directions for a causal listening comparison.
//!
//! Below 300 Hz the late field is deliberately coherent at the ears: one fifth
//! orthogonal FDN output is low-passed and shared by L/R. Above 300 Hz the four
//! W/X/Y/Z readouts are high-passed before virtual-speaker decoding. Because
//! these are orthogonal stochastic FDN readouts rather than two coherent speaker
//! bands, the crossover is a single 2nd-order Butterworth pair: its LP/HP squared
//! magnitudes sum to unity, preserving late-field power through the transition.
//!
//! This is intentionally a tiny closure layer, not a new reverb effect. Its
//! level, RT60 and predelay match the retained Current model's already-reduced
//! late field. Scale/front/height are still led by direct geometry and the much
//! stronger measured-HRTF early field.

use anyhow::bail;
use renderer::binaural::convolver::EarConvolver;
use renderer::binaural::hrir::{HRIR_LEN, HrirPair, HrirSet};
use renderer::binaural::itd;
use renderer::binaural::measured::MeasuredHrirData;
use renderer::crossover::filter::{
    BiquadCoeffs, BiquadState, biquad, butterworth2_hp, butterworth2_lp,
};
use renderer::delay_line::DelayLine;
use renderer::music_field::MUSIC_FIELD_CHANNELS;

const N: usize = 8;
const AXES: usize = 6;
const FIELD_CHANNELS: usize = 4;
const LENGTHS_48K: [usize; N] = [1031, 1327, 1523, 1801, 2053, 2311, 2617, 2903];
const DAMPING: f32 = 0.35;
const MOD_DEPTH_48K: f32 = 24.0;
const MOD_RATES_HZ: [f32; N] = [0.31, 0.41, 0.53, 0.67, 0.79, 0.97, 1.13, 1.31];
const MOD_UPDATE: usize = 128;
const ITD_MAX_S: f32 = 0.003;
const COHERENCE_XOVER_HZ: f32 = 300.0;

/// Retained Current late-room settings after the listening reduction in
/// `music_support::current_model_config`.
const CURRENT_LATE_LEVEL: f32 = 0.016;
const CURRENT_LATE_RT60_S: f32 = 0.12;
const CURRENT_LATE_PREDELAY_MS: f32 = 32.0;

/// Householder input injection and the independent shared low-frequency readout.
const COHERENT_SIGNS: [f32; N] = [1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0];

/// Four zero-sum, mutually orthogonal H8 rows, all orthogonal to
/// `COHERENT_SIGNS`. They are interpreted as W/X/Y/Z stochastic field
/// coefficients rather than physical directions. Equal row norms give equal
/// coefficient variance before the explicit field normalization below.
const FIELD_SIGNS: [[f32; N]; FIELD_CHANNELS] = [
    [1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0],
    [1.0, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0],
];

/// Axis order matches the renderer's shoebox wall convention: +X, -X, +Y,
/// -Y, +Z, -Z. +Y is front; +Z is ceiling.
const AXIS_DIRECTIONS: [[f32; 3]; AXES] = [
    [1.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, -1.0, 0.0],
    [0.0, 0.0, 1.0],
    [0.0, 0.0, -1.0],
];

/// Energy-normalized first-order field -> six cardinal virtual speakers.
/// Rows are speaker feeds; columns are [W, X, Y, Z]. R^T R = I exactly in
/// real arithmetic, so decoding does not change total field energy.
const FOA_TO_AXES: [[f32; FIELD_CHANNELS]; AXES] = [
    [0.408_248_3, 0.707_106_77, 0.0, 0.0],
    [0.408_248_3, -0.707_106_77, 0.0, 0.0],
    [0.408_248_3, 0.0, 0.707_106_77, 0.0],
    [0.408_248_3, 0.0, -0.707_106_77, 0.0],
    [0.408_248_3, 0.0, 0.0, 0.707_106_77],
    [0.408_248_3, 0.0, 0.0, -0.707_106_77],
];

#[inline]
fn dot_signs(signs: &[f32; N], values: &[f32; N]) -> f32 {
    let mut sum = 0.0f32;
    for i in 0..N {
        sum += signs[i] * values[i];
    }
    sum / (N as f32).sqrt()
}

#[inline]
fn decode_foa_to_axes(field: [f32; FIELD_CHANNELS]) -> [f32; AXES] {
    std::array::from_fn(|axis| {
        FOA_TO_AXES[axis]
            .iter()
            .zip(field.iter())
            .map(|(weight, sample)| weight * sample)
            .sum()
    })
}

/// The same compact late-network topology as the generic binaural FDN, but its
/// readout remains in a rotation-complete first-order field basis instead of
/// collapsing to two ears inside the network.
struct LateSphereFdn {
    lines: Vec<Vec<f32>>,
    base_len: [f32; N],
    pos: [usize; N],
    damp_state: [f32; N],
    fb_gain: [f32; N],
    mod_phase: [f32; N],
    cur_delay: [f32; N],
    mod_step: [f32; N],
    mod_samples_left: usize,
    predelay: Vec<f32>,
    pre_pos: usize,
    pre_len: usize,
    sample_rate: u32,
}

impl LateSphereFdn {
    fn new(sample_rate: u32) -> Self {
        let scale = sample_rate as f32 / 48_000.0;
        let margin = (MOD_DEPTH_48K * scale).ceil() as usize + 2;
        let mut base_len = [0.0f32; N];
        let lines: Vec<Vec<f32>> = LENGTHS_48K
            .iter()
            .enumerate()
            .map(|(i, &len)| {
                let base = ((len as f32 * scale) as usize).max(16);
                base_len[i] = base as f32;
                vec![0.0f32; base + margin]
            })
            .collect();
        let mut fb_gain = [0.0f32; N];
        for i in 0..N {
            let exp = -3.0 * base_len[i] / (CURRENT_LATE_RT60_S * sample_rate as f32);
            fb_gain[i] = 10.0f32.powf(exp);
        }
        let mut mod_phase = [0.0f32; N];
        for (i, phase) in mod_phase.iter_mut().enumerate() {
            *phase = i as f32 * 2.4;
        }
        let pre_cap = (sample_rate as usize * 120 / 1000).max(16);
        let pre_len = (CURRENT_LATE_PREDELAY_MS * sample_rate as f32 / 1000.0) as usize;
        Self {
            lines,
            base_len,
            pos: [0; N],
            damp_state: [0.0; N],
            fb_gain,
            mod_phase,
            cur_delay: base_len,
            mod_step: [0.0; N],
            mod_samples_left: 0,
            predelay: vec![0.0; pre_cap],
            pre_pos: 0,
            pre_len: pre_len.min(pre_cap - 1),
            sample_rate,
        }
    }

    #[inline]
    fn begin_modulation_segment(&mut self) {
        let sr = self.sample_rate as f32;
        let depth = MOD_DEPTH_48K * sr / 48_000.0;
        for i in 0..N {
            self.mod_phase[i] = (self.mod_phase[i]
                + std::f32::consts::TAU * MOD_RATES_HZ[i] * MOD_UPDATE as f32 / sr)
                % std::f32::consts::TAU;
            let target = self.base_len[i] + depth * self.mod_phase[i].sin();
            self.mod_step[i] = (target - self.cur_delay[i]) / MOD_UPDATE as f32;
        }
        self.mod_samples_left = MOD_UPDATE;
    }

    #[inline]
    fn process(&mut self, input: f32) -> ([f32; FIELD_CHANNELS], f32) {
        if self.mod_samples_left == 0 {
            self.begin_modulation_segment();
        }

        let read = (self.pre_pos + self.predelay.len() - self.pre_len) % self.predelay.len();
        let x = if self.pre_len == 0 { input } else { self.predelay[read] };
        self.predelay[self.pre_pos] = input;
        self.pre_pos = (self.pre_pos + 1) % self.predelay.len();

        let mut line_out = [0.0f32; N];
        let mut sum = 0.0f32;
        for i in 0..N {
            self.cur_delay[i] += self.mod_step[i];
            let delay = self.cur_delay[i];
            let cap = self.lines[i].len();
            let whole = delay as usize;
            let frac = delay - whole as f32;
            let r0 = (self.pos[i] + cap - whole) % cap;
            let r1 = if r0 == 0 { cap - 1 } else { r0 - 1 };
            let line = &self.lines[i];
            line_out[i] = line[r0] * (1.0 - frac) + line[r1] * frac;
            sum += line_out[i];
        }
        self.mod_samples_left -= 1;

        let field = std::array::from_fn(|channel| dot_signs(&FIELD_SIGNS[channel], &line_out));
        let coherent = dot_signs(&COHERENT_SIGNS, &line_out);

        // Householder feedback H·o = o - (2/N)Σo, with the same darkened loop
        // and alternating input injection as the inherited binaural FDN.
        let householder = 2.0 / N as f32 * sum;
        for i in 0..N {
            let fb = line_out[i] - householder;
            self.damp_state[i] += (fb - self.damp_state[i]) * (1.0 - DAMPING);
            let inject = COHERENT_SIGNS[i] * x;
            let cap = self.lines[i].len();
            self.lines[i][self.pos[i]] = self.damp_state[i] * self.fb_gain[i] + inject;
            self.pos[i] = (self.pos[i] + 1) % cap;
        }

        (field, coherent)
    }
}

struct AxisHrtfBus {
    delay_l: DelayLine,
    delay_r: DelayLine,
    conv_l: EarConvolver,
    conv_r: EarConvolver,
}

impl AxisHrtfBus {
    fn new(sample_rate: u32, hrir: &HrirSet, direction: [f32; 3]) -> Self {
        let az = direction[0].atan2(direction[1]);
        let horiz = (direction[0] * direction[0] + direction[1] * direction[1]).sqrt();
        let el = direction[2].atan2(horiz);
        let mut pair = HrirPair {
            left: [0.0; HRIR_LEN],
            right: [0.0; HRIR_LEN],
        };
        hrir.at(az.to_degrees(), el.to_degrees(), &mut pair);

        let max_itd = (ITD_MAX_S * sample_rate as f32).ceil() as usize;
        let mut delay_l = DelayLine::new(max_itd);
        let mut delay_r = DelayLine::new(max_itd);
        let (itd_l, itd_r) = itd::ear_delays_seconds(az, el, itd::DEFAULT_HEAD_RADIUS_M);
        delay_l.set_target_ms(itd_l * 1_000.0, sample_rate);
        delay_r.set_target_ms(itd_r * 1_000.0, sample_rate);

        let mut conv_l = EarConvolver::new();
        let mut conv_r = EarConvolver::new();
        conv_l.set_coeffs(&pair.left);
        conv_r.set_coeffs(&pair.right);
        Self {
            delay_l,
            delay_r,
            conv_l,
            conv_r,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> (f32, f32) {
        (
            self.conv_l.process(self.delay_l.process(input)),
            self.conv_r.process(self.delay_r.process(input)),
        )
    }
}

pub(crate) struct HrtfLateEnclosure {
    fdn: LateSphereFdn,
    axes: [AxisHrtfBus; AXES],
    xover_lp: BiquadCoeffs,
    xover_hp: BiquadCoeffs,
    low_state: BiquadState,
    high_state: [BiquadState; FIELD_CHANNELS],
}

impl HrtfLateEnclosure {
    pub(crate) fn new(sample_rate: u32) -> Self {
        let measured = MeasuredHrirData::saf_kemar().resampled_to(sample_rate);
        let hrir = HrirSet::new(&measured, sample_rate);
        Self {
            fdn: LateSphereFdn::new(sample_rate),
            axes: std::array::from_fn(|axis| {
                AxisHrtfBus::new(sample_rate, &hrir, AXIS_DIRECTIONS[axis])
            }),
            xover_lp: butterworth2_lp(COHERENCE_XOVER_HZ, sample_rate),
            xover_hp: butterworth2_hp(COHERENCE_XOVER_HZ, sample_rate),
            low_state: Default::default(),
            high_state: std::array::from_fn(|_| Default::default()),
        }
    }

    pub(crate) fn process(&mut self, field_input: &[f32]) -> anyhow::Result<Vec<f32>> {
        if field_input.len() % MUSIC_FIELD_CHANNELS != 0 {
            bail!(
                "HRTF late enclosure expected {}-channel interleaved support, got {} samples",
                MUSIC_FIELD_CHANNELS,
                field_input.len()
            );
        }

        let frames = field_input.len() / MUSIC_FIELD_CHANNELS;
        let mut out = vec![0.0f32; frames * 2];
        let field_gain = CURRENT_LATE_LEVEL / (FIELD_CHANNELS as f32).sqrt();
        let (lp, hp) = (self.xover_lp, self.xover_hp);

        for frame in 0..frames {
            let base = frame * MUSIC_FIELD_CHANNELS;
            // Match the inherited per-channel reverb-send topology: support
            // channels sum into one late network. Current support lanes are all
            // at the same metric radius, so no per-lane distance remapping is
            // needed here.
            let input = field_input[base..base + MUSIC_FIELD_CHANNELS]
                .iter()
                .copied()
                .sum::<f32>();
            let (field_raw, coherent_raw) = self.fdn.process(input);

            // Shared low-frequency field: BW2 low-pass, identical at both ears.
            let low = biquad(coherent_raw, lp, &mut self.low_state) * CURRENT_LATE_LEVEL;
            let o = frame * 2;
            out[o] += low;
            out[o + 1] += low;

            // Directional upper late field: filter the four independent W/X/Y/Z
            // coordinates, normalize their total variance, then decode to the
            // same six HRTF virtual directions used by the axis candidate.
            let mut field = [0.0f32; FIELD_CHANNELS];
            for channel in 0..FIELD_CHANNELS {
                field[channel] =
                    biquad(field_raw[channel], hp, &mut self.high_state[channel]) * field_gain;
            }
            let axis_input = decode_foa_to_axes(field);
            for axis in 0..AXES {
                let (l, r) = self.axes[axis].process(axis_input[axis]);
                out[o] += l;
                out[o + 1] += r;
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn impulse_field(frames: usize, channel: usize) -> Vec<f32> {
        let mut field = vec![0.0f32; frames * MUSIC_FIELD_CHANNELS];
        field[channel] = 1.0;
        field
    }

    #[test]
    fn field_basis_is_balanced_and_orthogonal() {
        let dot = |a: &[f32; N], b: &[f32; N]| -> f32 {
            a.iter().zip(b).map(|(x, y)| x * y).sum()
        };
        for row in &FIELD_SIGNS {
            assert_eq!(row.iter().sum::<f32>(), 0.0);
            assert_eq!(dot(row, &COHERENT_SIGNS), 0.0);
        }
        assert_eq!(COHERENT_SIGNS.iter().sum::<f32>(), 0.0);
        for i in 0..FIELD_CHANNELS {
            for j in i + 1..FIELD_CHANNELS {
                assert_eq!(dot(&FIELD_SIGNS[i], &FIELD_SIGNS[j]), 0.0);
            }
        }
    }

    #[test]
    fn foa_virtual_speaker_decode_preserves_energy() {
        for field in [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
            [0.4, -0.7, 0.2, 1.1],
        ] {
            let axes = decode_foa_to_axes(field);
            let field_energy: f32 = field.iter().map(|x| x * x).sum();
            let axis_energy: f32 = axes.iter().map(|x| x * x).sum();
            assert!(
                (field_energy - axis_energy).abs() < 2.0e-6,
                "FOA decode changed energy: field={field_energy}, axes={axis_energy}"
            );
        }
    }

    #[test]
    fn coherence_crossover_is_power_complementary() {
        let sample_rate = 48_000u32;
        let lp = butterworth2_lp(COHERENCE_XOVER_HZ, sample_rate);
        let hp = butterworth2_hp(COHERENCE_XOVER_HZ, sample_rate);

        for freq in [100.0f32, 300.0, 1_000.0] {
            let mut lp_state = BiquadState::default();
            let mut hp_state = BiquadState::default();
            let mut input_power = 0.0f64;
            let mut split_power = 0.0f64;
            for i in 0..48_000usize {
                let x = (std::f32::consts::TAU * freq * i as f32 / sample_rate as f32).sin();
                let low = biquad(x, lp, &mut lp_state);
                let high = biquad(x, hp, &mut hp_state);
                if i >= 24_000 {
                    input_power += (x as f64) * (x as f64);
                    split_power += (low as f64) * (low as f64) + (high as f64) * (high as f64);
                }
            }
            let ratio = split_power / input_power;
            assert!(
                (0.995..=1.005).contains(&ratio),
                "incoherent crossover power changed at {freq} Hz: {ratio}"
            );
        }
    }

    #[test]
    fn late_enclosure_is_delayed_not_an_extra_direct_copy() {
        let mut enclosure = HrtfLateEnclosure::new(48_000);
        let out = enclosure.process(&impulse_field(8_000, 0)).unwrap();
        // 32 ms predelay + shortest ~21 ms FDN line keeps the first 50 ms dry.
        let first_50ms: f32 = out[..2_400 * 2].iter().map(|x| x * x).sum();
        let later: f32 = out[2_400 * 2..].iter().map(|x| x * x).sum();
        assert!(first_50ms < 1.0e-12, "late field arrived early: {first_50ms}");
        assert!(later > 1.0e-10, "late enclosure produced no delayed energy");
    }

    #[test]
    fn processing_is_block_boundary_invariant() {
        let input = impulse_field(12_000, 9);
        let mut whole = HrtfLateEnclosure::new(48_000);
        let expected = whole.process(&input).unwrap();

        let split_frames = 3_137usize;
        let split = split_frames * MUSIC_FIELD_CHANNELS;
        let mut partitioned = HrtfLateEnclosure::new(48_000);
        let mut actual = partitioned.process(&input[..split]).unwrap();
        actual.extend(partitioned.process(&input[split..]).unwrap());

        assert_eq!(expected.len(), actual.len());
        let max_error = expected
            .iter()
            .zip(&actual)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(max_error < 1.0e-6, "callback boundary changed late enclosure: {max_error}");
    }

    #[test]
    fn low_frequency_late_field_is_more_coherent_than_upper_field() {
        fn render_tone(freq: f32) -> (f32, f32) {
            let frames = 48_000usize;
            let mut input = vec![0.0f32; frames * MUSIC_FIELD_CHANNELS];
            for frame in 0..frames {
                input[frame * MUSIC_FIELD_CHANNELS] =
                    0.1 * (std::f32::consts::TAU * freq * frame as f32 / 48_000.0).sin();
            }
            let mut enclosure = HrtfLateEnclosure::new(48_000);
            let out = enclosure.process(&input).unwrap();
            let start = 24_000usize;
            let mut same = 0.0f32;
            let mut diff = 0.0f32;
            for frame in start..frames {
                let l = out[frame * 2];
                let r = out[frame * 2 + 1];
                same += (l + r) * (l + r);
                diff += (l - r) * (l - r);
            }
            (same, diff)
        }

        let (low_same, low_diff) = render_tone(120.0);
        let (high_same, high_diff) = render_tone(4_000.0);
        assert!(low_same > low_diff * 6.0, "120 Hz late field lost ear coherence");
        assert!(
            high_diff > low_diff * 4.0 || high_diff > high_same * 0.02,
            "upper late field did not develop binaural difference"
        );
    }
}

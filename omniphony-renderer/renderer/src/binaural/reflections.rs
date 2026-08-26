//! First-order shoebox early reflections for the binaural stage.
//!
//! Externalization aid: an anechoic HRTF render often remains perceptually
//! close to the head because it lacks a coherent environment. This module adds
//! the six first-order image sources of a shoebox room (listener at the room
//! centre, world-fixed walls): each reflection is a delayed, attenuated copy of
//! the channel signal with independent left/right arrival times and gains.
//!
//! The early room is intentionally hybrid. Side/rear reflections stay on the
//! cheap directional path (ITD + broadband ILD), while the front wall, ceiling
//! and floor receive short per-ear HRIR filters. Those three surfaces are the
//! minimum useful shell for frontal externalization and vertical enclosure, and
//! keeping their filters to the perceptually critical early 64 taps bounds the
//! realtime cost instead of multiplying the full 128-tap direct HRTF by six.
//!
//! All reflections retain propagation delay plus broad frequency-dependent wall
//! / extra-path HF loss. The direct object still keeps the full measured HRTF.
//!
//! The direct path keeps zero common propagation delay (A/V sync unchanged);
//! reflection delays are *relative* to the direct path
//! (`(d_image − d_direct) / c ≥ 0`) plus each reflection direction's per-ear
//! ITD. The direct/reflected timing and binaural structure therefore change with
//! source and image direction without turning the room into generic reverb.

use super::hrir::HRIR_LEN;

/// Speed of sound (m/s), matching `itd.rs`.
const SPEED_OF_SOUND: f32 = 343.0;

/// Ring capacity in seconds. Bounds the relative reflection delay; with room
/// dimensions clamped to [`MAX_ROOM_M`] the longest first-order detour stays
/// well below this.
const RING_CAPACITY_S: f32 = 0.25;

/// Per-axis room size clamp (m). The music frontier intentionally uses very
/// large virtual rooms; 32 m still fits comfortably inside the delay ring.
pub const MIN_ROOM_M: f32 = 1.0;
pub const MAX_ROOM_M: f32 = 32.0;

/// Margin (m) keeping the (clamped) source strictly inside the room so an
/// image can never coincide with the listener.
const WALL_MARGIN_M: f32 = 0.05;

/// Delay ramp speed in delay-samples per output sample (same policy as
/// [`crate::delay_line::DelayLine`]): no discontinuities, a full-scale change
/// completes in at most the delay span itself.
const DELAY_RAMP_RATE: f32 = 1.0;

/// One-pole smoothing coefficient for tap gains (~1.5 ms at 48 kHz).
const GAIN_SMOOTH: f32 = 0.015;

/// Broad split used only on reflected paths. This is intentionally not a narrow
/// corrective EQ: it separates the upper spectrum so real-wall / extra-path HF
/// loss can be represented without weakening the reflection's low/mid timing.
const REFLECTION_TONE_SPLIT_HZ: f32 = 4_000.0;

/// Steam Audio's generic material model absorbs more energy at high frequencies
/// than low frequencies. 0.84 is approximately sqrt(1 - 0.30), i.e. the
/// amplitude retention corresponding to 30% high-band energy absorption.
const GENERIC_WALL_HF_AMPLITUDE: f32 = 0.84;

/// Additional upper-band amplitude decay per metre of reflection-only detour.
/// The direct-path air filter already models source distance; this term covers
/// only the extra image-source path that a reflection travels.
const EXTRA_PATH_HF_DECAY_PER_M: f32 = 0.020;

/// Number of first-order images of a shoebox (one per wall).
pub const NUM_REFLECTIONS: usize = 6;

/// Full direct HRIRs are 128 taps. Reflections only need the early directional
/// structure, so the HRTF shell keeps the first 64 taps and energy-normalizes the
/// truncation. At 48 kHz that is ~1.33 ms, enough to carry pinna/head spectral
/// structure while halving the per-reflection FIR work.
const REFLECTION_HRIR_LEN: usize = 64;
const REFLECTION_ACC_LANES: usize = 8;

/// Image indices are produced as ±X, ±Y, ±Z. +Y is the room's front wall,
/// +Z the ceiling and -Z the floor. These three surfaces form the bounded HRTF
/// shell; side and rear reflections retain the existing cheap ITD/ILD path.
const HRTF_REFLECTION_INDICES: [usize; 3] = [2, 4, 5];
const HRTF_REFLECTION_COUNT: usize = HRTF_REFLECTION_INDICES.len();

const _: () = assert!(REFLECTION_HRIR_LEN <= HRIR_LEN);
const _: () = assert!(REFLECTION_HRIR_LEN.is_multiple_of(REFLECTION_ACC_LANES));

/// Mirror `src` (listener-relative metres, listener at the room centre)
/// across each of the six walls of a `room` (full extents, metres).
///
/// Sources outside the room are first clamped just inside the walls — the
/// geometry stays valid for any `unit_scale_m`.
pub fn first_order_images(src_m: [f32; 3], room_m: [f32; 3]) -> [[f32; 3]; NUM_REFLECTIONS] {
    let mut half = [0.0f32; 3];
    let mut s = src_m;
    for a in 0..3 {
        half[a] = (room_m[a].clamp(MIN_ROOM_M, MAX_ROOM_M)) * 0.5;
        s[a] = s[a].clamp(-(half[a] - WALL_MARGIN_M), half[a] - WALL_MARGIN_M);
    }
    let mut out = [[0.0f32; 3]; NUM_REFLECTIONS];
    for a in 0..3 {
        let mut pos = s;
        pos[a] = 2.0 * half[a] - s[a];
        out[a * 2] = pos;
        let mut neg = s;
        neg[a] = -2.0 * half[a] - s[a];
        out[a * 2 + 1] = neg;
    }
    out
}

/// One smoothed fractional read tap (delay + level + reflection spectral state).
#[derive(Clone, Copy, Default)]
struct Tap {
    delay: f32,
    delay_target: f32,
    gain: f32,
    gain_target: f32,
    /// One-pole low component used to form a broad low/high spectral split.
    tone_state: f32,
    /// Current / target retention for the upper side of that split.
    hf_gain: f32,
    hf_gain_target: f32,
}

impl Tap {
    #[inline]
    fn step(&mut self) {
        let d = self.delay_target - self.delay;
        if d.abs() <= DELAY_RAMP_RATE {
            self.delay = self.delay_target;
        } else {
            self.delay += DELAY_RAMP_RATE * d.signum();
        }
        self.gain += (self.gain_target - self.gain) * GAIN_SMOOTH;
        self.hf_gain += (self.hf_gain_target - self.hf_gain) * GAIN_SMOOTH;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ReflectionHrtfTag {
    generation: u32,
    az_bits: u32,
    el_bits: u32,
}

impl ReflectionHrtfTag {
    #[inline]
    fn new(generation: u32, az_rad: f32, el_rad: f32) -> Self {
        Self {
            generation,
            az_bits: az_rad.to_bits(),
            el_bits: el_rad.to_bits(),
        }
    }
}

/// Compact direct-form FIR used only for the selected early-room surfaces.
/// It mirrors the direct convolver's continuity rules but keeps half the taps.
struct ReflectionConvolver {
    hist: [f32; 2 * REFLECTION_HRIR_LEN],
    pos: usize,
    rcoeffs: [f32; REFLECTION_HRIR_LEN],
    initialized: bool,
    prev_rcoeffs: [f32; REFLECTION_HRIR_LEN],
    fade_pos: u32,
    fade_len: u32,
}

impl ReflectionConvolver {
    fn new() -> Self {
        Self {
            hist: [0.0; 2 * REFLECTION_HRIR_LEN],
            pos: 0,
            rcoeffs: [0.0; REFLECTION_HRIR_LEN],
            initialized: false,
            prev_rcoeffs: [0.0; REFLECTION_HRIR_LEN],
            fade_pos: 0,
            fade_len: 0,
        }
    }

    #[inline]
    fn kernel_is(&self, coeffs: &[f32; REFLECTION_HRIR_LEN]) -> bool {
        self.rcoeffs.iter().eq(coeffs.iter().rev())
    }

    fn set_coeffs_smooth(&mut self, coeffs: &[f32; REFLECTION_HRIR_LEN], fade_len: usize) {
        if !self.initialized || fade_len == 0 {
            for (dst, &c) in self.rcoeffs.iter_mut().zip(coeffs.iter().rev()) {
                *dst = c;
            }
            self.initialized = true;
            self.fade_pos = 0;
            self.fade_len = 0;
            return;
        }
        if self.kernel_is(coeffs) {
            return;
        }
        if self.fade_pos < self.fade_len {
            let w = self.fade_pos as f32 / self.fade_len as f32;
            for i in 0..REFLECTION_HRIR_LEN {
                self.prev_rcoeffs[i] += (self.rcoeffs[i] - self.prev_rcoeffs[i]) * w;
            }
        } else {
            self.prev_rcoeffs.copy_from_slice(&self.rcoeffs);
        }
        for (dst, &c) in self.rcoeffs.iter_mut().zip(coeffs.iter().rev()) {
            *dst = c;
        }
        self.fade_pos = 0;
        self.fade_len = fade_len.min(REFLECTION_HRIR_LEN) as u32;
    }

    fn reset_runtime_state(&mut self) {
        self.hist.fill(0.0);
        self.pos = 0;
        self.rcoeffs.fill(0.0);
        self.prev_rcoeffs.fill(0.0);
        self.initialized = false;
        self.fade_pos = 0;
        self.fade_len = 0;
    }

    #[inline(always)]
    fn dot(coeffs: &[f32; REFLECTION_HRIR_LEN], win: &[f32]) -> f32 {
        let mut acc = [0.0f32; REFLECTION_ACC_LANES];
        for (c, h) in coeffs
            .chunks_exact(REFLECTION_ACC_LANES)
            .zip(win.chunks_exact(REFLECTION_ACC_LANES))
        {
            for lane in 0..REFLECTION_ACC_LANES {
                acc[lane] += c[lane] * h[lane];
            }
        }
        acc.iter().sum()
    }

    #[inline(always)]
    fn dot2(
        new_c: &[f32; REFLECTION_HRIR_LEN],
        old_c: &[f32; REFLECTION_HRIR_LEN],
        win: &[f32],
    ) -> (f32, f32) {
        let mut acc_new = [0.0f32; REFLECTION_ACC_LANES];
        let mut acc_old = [0.0f32; REFLECTION_ACC_LANES];
        for ((cn, co), h) in new_c
            .chunks_exact(REFLECTION_ACC_LANES)
            .zip(old_c.chunks_exact(REFLECTION_ACC_LANES))
            .zip(win.chunks_exact(REFLECTION_ACC_LANES))
        {
            for lane in 0..REFLECTION_ACC_LANES {
                let hv = h[lane];
                acc_new[lane] += cn[lane] * hv;
                acc_old[lane] += co[lane] * hv;
            }
        }
        (acc_new.iter().sum(), acc_old.iter().sum())
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        if !self.initialized {
            return x;
        }
        self.pos = if self.pos + 1 == REFLECTION_HRIR_LEN {
            0
        } else {
            self.pos + 1
        };
        self.hist[self.pos] = x;
        self.hist[self.pos + REFLECTION_HRIR_LEN] = x;
        let win = &self.hist[self.pos + 1..self.pos + 1 + REFLECTION_HRIR_LEN];

        if self.fade_pos < self.fade_len {
            self.fade_pos += 1;
            let w = self.fade_pos as f32 / self.fade_len as f32;
            let (new_y, old_y) = Self::dot2(&self.rcoeffs, &self.prev_rcoeffs, win);
            old_y + (new_y - old_y) * w
        } else {
            Self::dot(&self.rcoeffs, win)
        }
    }
}

/// Keep the early 64 taps but preserve the full HRIR's per-ear energy so the
/// CPU-bound truncation does not accidentally flatten HRTF ILD. The scale cap
/// prevents pathological files with almost all energy in the discarded tail
/// from turning a reflection into an unstable gain boost.
fn compact_hrir(src: &[f32; HRIR_LEN]) -> [f32; REFLECTION_HRIR_LEN] {
    let full_energy = src.iter().map(|x| x * x).sum::<f32>();
    let short_energy = src[..REFLECTION_HRIR_LEN]
        .iter()
        .map(|x| x * x)
        .sum::<f32>();
    let scale = if full_energy > 1e-12 && short_energy > 1e-12 {
        (full_energy / short_energy).sqrt().clamp(0.5, 2.0)
    } else {
        1.0
    };
    let mut out = [0.0f32; REFLECTION_HRIR_LEN];
    for (dst, &x) in out.iter_mut().zip(src.iter()) {
        *dst = x * scale;
    }
    out
}

#[inline]
fn hrtf_slot(idx: usize) -> Option<usize> {
    HRTF_REFLECTION_INDICES.iter().position(|&candidate| candidate == idx)
}

/// Per-channel reflection bank: one shared ring buffer written once per
/// sample, read by `NUM_REFLECTIONS × 2` smoothed taps (left/right ear).
pub struct ReflectionBank {
    ring: Vec<f32>,
    write_pos: usize,
    taps_l: [Tap; NUM_REFLECTIONS],
    taps_r: [Tap; NUM_REFLECTIONS],
    sample_rate: u32,
    tone_alpha: f32,
    hrtf_l: [ReflectionConvolver; HRTF_REFLECTION_COUNT],
    hrtf_r: [ReflectionConvolver; HRTF_REFLECTION_COUNT],
    hrtf_tags: [Option<ReflectionHrtfTag>; HRTF_REFLECTION_COUNT],
}

impl ReflectionBank {
    pub fn new(sample_rate: u32) -> Self {
        let cap = (RING_CAPACITY_S * sample_rate as f32).ceil() as usize + 2;
        Self {
            ring: vec![0.0; cap],
            write_pos: 0,
            taps_l: Default::default(),
            taps_r: Default::default(),
            sample_rate,
            tone_alpha: 1.0
                - (-std::f32::consts::TAU * REFLECTION_TONE_SPLIT_HZ / sample_rate as f32).exp(),
            hrtf_l: std::array::from_fn(|_| ReflectionConvolver::new()),
            hrtf_r: std::array::from_fn(|_| ReflectionConvolver::new()),
            hrtf_tags: [None; HRTF_REFLECTION_COUNT],
        }
    }

    /// Whether this wall belongs to the bounded HRTF shell.
    #[inline]
    pub fn uses_hrtf(idx: usize) -> bool {
        hrtf_slot(idx).is_some()
    }

    /// True when a selected reflection needs a new HRIR kernel for this source
    /// direction or active HRTF generation. Non-HRTF walls always return false.
    #[inline]
    pub fn hrtf_needs_update(
        &self,
        idx: usize,
        generation: u32,
        az_rad: f32,
        el_rad: f32,
    ) -> bool {
        let Some(slot) = hrtf_slot(idx) else {
            return false;
        };
        self.hrtf_tags[slot] != Some(ReflectionHrtfTag::new(generation, az_rad, el_rad))
    }

    /// Install/crossfade the short reflection HRIR for one selected wall. The
    /// caller owns HRTF interpolation and supplies the same active grid used by
    /// the direct path; this bank only compacts and convolves it.
    pub fn set_hrtf(
        &mut self,
        idx: usize,
        generation: u32,
        az_rad: f32,
        el_rad: f32,
        left: &[f32; HRIR_LEN],
        right: &[f32; HRIR_LEN],
        fade_len: usize,
    ) {
        let Some(slot) = hrtf_slot(idx) else {
            return;
        };
        let left = compact_hrir(left);
        let right = compact_hrir(right);
        self.hrtf_l[slot].set_coeffs_smooth(&left, fade_len);
        self.hrtf_r[slot].set_coeffs_smooth(&right, fade_len);
        self.hrtf_tags[slot] = Some(ReflectionHrtfTag::new(generation, az_rad, el_rad));
    }

    /// Backward-compatible full-band target update with the same delay at both
    /// ears. Tests and callers that deliberately want the historical broadband
    /// tap can continue to use this method.
    pub fn set_targets(&mut self, idx: usize, delay_s: f32, gain_l: f32, gain_r: f32) {
        self.set_targets_binaural_toned(idx, delay_s, delay_s, gain_l, gain_r, 1.0);
    }

    /// Update one physical reflection's per-ear targets.
    ///
    /// `delay_l_s` and `delay_r_s` include the common image-source propagation
    /// detour plus the reflection direction's interaural delay. Keeping those
    /// delays separate lets the cheap tap bank carry a real binaural timing cue
    /// rather than only an ILD pan.
    ///
    /// Unlike the legacy broadband tap, production reflections also receive a
    /// broad high-frequency loss derived from a generic wall absorption term and
    /// the reflection-only propagation detour. This keeps large virtual rooms
    /// from behaving like six perfectly shiny broadband mirrors.
    pub fn set_targets_binaural(
        &mut self,
        idx: usize,
        delay_l_s: f32,
        delay_r_s: f32,
        gain_l: f32,
        gain_r: f32,
    ) {
        let extra_path_s = (0.5 * (delay_l_s + delay_r_s)).max(0.0);
        let extra_path_m = extra_path_s * SPEED_OF_SOUND;
        let extra_air_hf = (-EXTRA_PATH_HF_DECAY_PER_M * extra_path_m).exp();
        let hf_gain = (GENERIC_WALL_HF_AMPLITUDE * extra_air_hf).clamp(0.45, 0.90);
        self.set_targets_binaural_toned(idx, delay_l_s, delay_r_s, gain_l, gain_r, hf_gain);
    }

    /// Explicit spectral variant. `hf_gain=1` reproduces the historical
    /// broadband reflection; smaller values attenuate only the upper side of the
    /// broad reflection split while preserving low/mid timing and level.
    pub fn set_targets_binaural_toned(
        &mut self,
        idx: usize,
        delay_l_s: f32,
        delay_r_s: f32,
        gain_l: f32,
        gain_r: f32,
        hf_gain: f32,
    ) {
        let max = (self.ring.len() - 2) as f32;
        let d_l = (delay_l_s * self.sample_rate as f32).clamp(0.0, max);
        let d_r = (delay_r_s * self.sample_rate as f32).clamp(0.0, max);
        let hf_gain = hf_gain.clamp(0.0, 1.0);
        for (tap, delay, gain) in [
            (&mut self.taps_l[idx], d_l, gain_l),
            (&mut self.taps_r[idx], d_r, gain_r),
        ] {
            tap.delay_target = delay;
            tap.gain_target = gain;
            tap.hf_gain_target = hf_gain;
            // While the tap is (near) silent a delay / spectral jump is
            // inaudible. Snap instead of sweeping a fresh tap through the live
            // signal on its way to the target.
            if tap.gain.abs() < 1e-4 {
                tap.delay = delay;
                tap.hf_gain = hf_gain;
            }
        }
    }

    /// Fade every tap out (e.g. when the channel goes silent) so re-enabling
    /// does not click.
    pub fn mute_targets(&mut self) {
        for t in self.taps_l.iter_mut().chain(self.taps_r.iter_mut()) {
            t.gain_target = 0.0;
        }
    }

    /// Reset one logical stream while retaining the preallocated reflection ring.
    pub fn reset_runtime_state(&mut self) {
        self.ring.fill(0.0);
        self.write_pos = 0;
        self.taps_l = Default::default();
        self.taps_r = Default::default();
        for conv in self.hrtf_l.iter_mut().chain(self.hrtf_r.iter_mut()) {
            conv.reset_runtime_state();
        }
        self.hrtf_tags = [None; HRTF_REFLECTION_COUNT];
    }

    /// Write one input sample and return the summed (left, right) reflection
    /// contribution.
    #[inline]
    pub fn process(&mut self, input: f32) -> (f32, f32) {
        let cap = self.ring.len();
        self.ring[self.write_pos] = input;

        let mut l = 0.0f32;
        let mut r = 0.0f32;
        for i in 0..NUM_REFLECTIONS {
            let tl = &mut self.taps_l[i];
            tl.step();
            let xl = read_frac(&self.ring, cap, self.write_pos, tl.delay);
            tl.tone_state += (xl - tl.tone_state) * self.tone_alpha;
            let yl = tl.gain * (tl.tone_state + tl.hf_gain * (xl - tl.tone_state));

            let tr = &mut self.taps_r[i];
            tr.step();
            let xr = read_frac(&self.ring, cap, self.write_pos, tr.delay);
            tr.tone_state += (xr - tr.tone_state) * self.tone_alpha;
            let yr = tr.gain * (tr.tone_state + tr.hf_gain * (xr - tr.tone_state));

            if let Some(slot) = hrtf_slot(i) {
                l += self.hrtf_l[slot].process(yl);
                r += self.hrtf_r[slot].process(yr);
            } else {
                l += yl;
                r += yr;
            }
        }

        self.write_pos += 1;
        if self.write_pos >= cap {
            self.write_pos = 0;
        }
        (l, r)
    }
}

/// Linear-interpolated read at `delay` samples behind `write_pos` (which still
/// points at the sample just written).
#[inline]
fn read_frac(ring: &[f32], cap: usize, write_pos: usize, delay: f32) -> f32 {
    let lo = delay.floor();
    let frac = delay - lo;
    let lo = lo as usize;
    let idx0 = (write_pos + cap - lo % cap) % cap;
    let idx1 = (idx0 + cap - 1) % cap;
    ring[idx0] * (1.0 - frac) + ring[idx1] * frac
}

/// Speed of sound accessor so callers share one constant.
#[inline]
pub fn speed_of_sound() -> f32 {
    SPEED_OF_SOUND
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_source_images_sit_one_room_dimension_away() {
        let room = [4.0, 6.0, 3.0];
        let images = first_order_images([0.0, 0.0, 0.0], room);
        assert_eq!(images[0][0], 4.0);
        assert_eq!(images[1][0], -4.0);
        assert_eq!(images[2][1], 6.0);
        assert_eq!(images[3][1], -6.0);
        assert_eq!(images[4][2], 3.0);
        assert_eq!(images[5][2], -3.0);
    }

    #[test]
    fn outside_source_is_clamped_inside() {
        let room = [4.0, 4.0, 4.0];
        let images = first_order_images([100.0, 0.0, 0.0], room);
        assert!(images[0][0] > 2.0 && images[0][0] < 2.1);
        for img in images {
            assert!(img.iter().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn hrtf_shell_is_front_and_vertical_only() {
        assert!(!ReflectionBank::uses_hrtf(0));
        assert!(!ReflectionBank::uses_hrtf(1));
        assert!(ReflectionBank::uses_hrtf(2));
        assert!(!ReflectionBank::uses_hrtf(3));
        assert!(ReflectionBank::uses_hrtf(4));
        assert!(ReflectionBank::uses_hrtf(5));
    }

    #[test]
    fn bank_delays_and_attenuates() {
        let mut bank = ReflectionBank::new(48_000);
        bank.set_targets(0, 10.0 / 48_000.0, 0.5, 0.0);
        for _ in 0..4_000 {
            bank.process(0.0);
        }
        let mut outs = Vec::new();
        outs.push(bank.process(1.0));
        for _ in 0..20 {
            outs.push(bank.process(0.0));
        }
        let (l10, r10) = outs[10];
        assert!((l10 - 0.5).abs() < 1e-3, "left tap at 10 smp: {l10}");
        assert!(r10.abs() < 1e-6, "right must stay silent: {r10}");
        for (i, &(l, _)) in outs.iter().enumerate() {
            if i != 10 {
                assert!(l.abs() < 1e-3, "leak at {i}: {l}");
            }
        }
    }

    #[test]
    fn selected_reflection_hrtf_keeps_ear_specific_filter_timing() {
        let mut bank = ReflectionBank::new(48_000);
        bank.set_targets_binaural_toned(2, 0.0, 0.0, 1.0, 1.0, 1.0);
        let mut left = [0.0f32; HRIR_LEN];
        let mut right = [0.0f32; HRIR_LEN];
        left[0] = 1.0;
        right[3] = 1.0;
        bank.set_hrtf(2, 1, 0.0, 0.0, &left, &right, 0);
        for _ in 0..4_000 {
            bank.process(0.0);
        }

        let mut outs = Vec::new();
        outs.push(bank.process(1.0));
        for _ in 0..8 {
            outs.push(bank.process(0.0));
        }
        assert!((outs[0].0 - 1.0).abs() < 1e-3, "left={:?}", outs[0]);
        assert!(outs[0].1.abs() < 1e-3, "right arrived early={:?}", outs[0]);
        assert!((outs[3].1 - 1.0).abs() < 1e-3, "right={:?}", outs[3]);
    }

    #[test]
    fn binaural_targets_can_arrive_at_different_ear_times() {
        let mut bank = ReflectionBank::new(48_000);
        bank.set_targets_binaural_toned(0, 10.0 / 48_000.0, 14.0 / 48_000.0, 1.0, 1.0, 1.0);
        for _ in 0..4_000 {
            bank.process(0.0);
        }

        let mut outs = Vec::new();
        outs.push(bank.process(1.0));
        for _ in 0..20 {
            outs.push(bank.process(0.0));
        }
        assert!(
            (outs[10].0 - 1.0).abs() < 1e-3,
            "left arrival={:?}",
            outs[10]
        );
        assert!(
            outs[10].1.abs() < 1e-3,
            "right arrived too early={:?}",
            outs[10]
        );
        assert!(
            (outs[14].1 - 1.0).abs() < 1e-3,
            "right arrival={:?}",
            outs[14]
        );
        assert!(
            outs[14].0.abs() < 1e-3,
            "left leaked at right arrival={:?}",
            outs[14]
        );
    }

    #[test]
    fn physical_reflection_loses_more_hf_as_detour_grows() {
        let mut near = ReflectionBank::new(48_000);
        near.set_targets_binaural(0, 0.001, 0.001, 1.0, 1.0);
        let near_hf = near.taps_l[0].hf_gain_target;

        let mut far = ReflectionBank::new(48_000);
        far.set_targets_binaural(0, 0.050, 0.050, 1.0, 1.0);
        let far_hf = far.taps_l[0].hf_gain_target;

        assert!(near_hf < 1.0);
        assert!(far_hf < near_hf, "near={near_hf} far={far_hf}");
        assert!(far_hf >= 0.45);
    }

    #[test]
    fn toned_reflection_reduces_high_frequency_more_than_low_frequency() {
        fn rms_for(freq: f32) -> f32 {
            let mut bank = ReflectionBank::new(48_000);
            bank.set_targets_binaural_toned(0, 0.0, 0.0, 1.0, 0.0, 0.35);
            let mut sum = 0.0f32;
            let mut count = 0usize;
            for i in 0..8_000 {
                let x = (std::f32::consts::TAU * freq * i as f32 / 48_000.0).sin();
                let (y, _) = bank.process(x);
                if i > 2_000 {
                    sum += y * y;
                    count += 1;
                }
            }
            (sum / count as f32).sqrt()
        }

        let low = rms_for(500.0);
        let high = rms_for(8_000.0);
        assert!(low > high * 1.35, "low={low} high={high}");
    }

    #[test]
    fn gain_changes_are_smoothed() {
        let mut bank = ReflectionBank::new(48_000);
        bank.set_targets(0, 0.0, 1.0, 1.0);
        for _ in 0..4_000 {
            bank.process(1.0);
        }
        let (settled, _) = bank.process(1.0);
        assert!((settled - 1.0).abs() < 1e-2);
        bank.set_targets(0, 0.0, 0.0, 0.0);
        let (next, _) = bank.process(1.0);
        assert!(next > 0.9, "gain jumped instead of smoothing: {next}");
    }
}

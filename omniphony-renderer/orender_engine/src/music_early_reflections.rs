//! Bounded measured-HRTF early-reflection field for the music path.
//!
//! A literal measured-HRTF convolution for every image of every virtual support
//! source would multiply the FIR count by the speaker count. This module keeps
//! image-source timing and wall tone per support lane, groups contributions by
//! final-arrival wall, then applies exactly six measured SAF/KEMAR HRTFs.
//!
//! The retained field is first-order for the full support shell. Front and
//! top-front lanes additionally redistribute a bounded share of that same early
//! reflection power into physically derived second-order image timing. This is
//! a depth/externalization cue, not an added wet layer: the front-lane tap-power
//! budget is preserved before the shared measured-HRTF wall buses.
//!
//! The transient-aware excitation in this file is intentionally narrower than
//! transient separation or transient reshaping. Each support lane compares a
//! fast energy envelope with a slow energy envelope. A sharp positive rise may
//! briefly increase only that lane's signal entering the early-reflection delay
//! bank. The protected stereo master, coherent foundation, primary support
//! render and late room field are not modified here.

use anyhow::bail;
use renderer::binaural::convolver::EarConvolver;
use renderer::binaural::hrir::{HRIR_LEN, HrirPair, HrirSet};
use renderer::binaural::itd;
use renderer::binaural::measured::MeasuredHrirData;
use renderer::binaural::reflections::{self, NUM_REFLECTIONS};
use renderer::delay_line::DelayLine;
use renderer::music_field::MUSIC_FIELD_CHANNELS;

const CURRENT_UNIT_SCALE_M: f32 = 9.25;
const CURRENT_ROOM_M: [f32; 3] = [23.0, 32.0, 21.0];
const CURRENT_REFLECTION_LEVEL: f32 = 0.36;
const REF_DISTANCE_M: f32 = 1.0;
const MAX_DISTANCE_GAIN: f32 = 4.0;
const MIN_DISTANCE_M: f32 = 0.25;
const RING_CAPACITY_S: f32 = 0.25;
const TONE_SPLIT_HZ: f32 = 4_000.0;
const GENERIC_WALL_HF_AMPLITUDE: f32 = 0.84;
const EXTRA_PATH_HF_DECAY_PER_M: f32 = 0.020;
const ITD_MAX_S: f32 = 0.003;

// Frontal externalization candidate. Pyroomacoustics' shoebox ISM enumerates
// the discrete L1 image lattice; in 3-D there are exactly 18 order-2 images.
// Only images that stay inside the perceptually useful early window are kept.
// Rather than adding their energy on top of the accepted room, 20% of each
// front lane's existing first-order tap power is redistributed into them.
const SECOND_ORDER_IMAGE_COUNT: usize = 18;
const FRONT_SECOND_ORDER_POWER_FRACTION: f32 = 0.20;
const SECOND_ORDER_MAX_DELAY_S: f32 = 0.100;
const SECOND_ORDER_WALL_MARGIN_M: f32 = 0.05;

// Listening-candidate transient law. Fast/slow energy comparison follows the
// established onset-detection idea that a transient is a positive change in
// short-time energy, not simply a loud sample. Values are deliberately bounded
// and local to each existing spatial-support lane so a drum event cannot turn
// the whole mixture's room up at once.
const TRANSIENT_FAST_MS: f32 = 3.0;
const TRANSIENT_SLOW_MS: f32 = 45.0;
const TRANSIENT_RELEASE_MS: f32 = 20.0;
const TRANSIENT_MIN_RMS: f32 = 0.0015;
const TRANSIENT_RISE_THRESHOLD: f32 = 0.75;
const TRANSIENT_FULL_RISE: f32 = 3.0;
const TRANSIENT_MAX_GAIN_DB: f32 = 2.5;

// The legacy analytic reflection panner has total L+R power 4/3 for a unit
// reflection gain (`SHADOW=0.5`, denominator 1.5). A diffuse-normalized HRIR
// pair is approximately 2.0 total-ear power. Scale by sqrt((4/3)/2) so the
// measured-HRTF field primarily changes directional spectral information rather
// than simply making the early field louder.
const HRTF_POWER_MATCH: f32 = 0.816_496_6;

/// Canonical 8.1.4.4 lane directions in `MUSIC_FIELD_CHANNELS` order:
/// L R C LFE Ls Rs Lb Rb Cb Tfl Tfr Tbl Tbr Bfl Bfr Bbl Bbr.
/// Positive azimuth is right, positive elevation is up. The stereo Current
/// extractor leaves C/LFE/Cb/lower lanes EMPTY, but the geometry is complete so
/// authored richer ingress can use the same portable frame later.
const LANE_DIRECTIONS_DEG: [(f32, f32); MUSIC_FIELD_CHANNELS] = [
    (-30.0, 0.0),
    (30.0, 0.0),
    (0.0, 0.0),
    (0.0, 0.0),
    (-90.0, 0.0),
    (90.0, 0.0),
    (-140.0, 0.0),
    (140.0, 0.0),
    (180.0, 0.0),
    (-40.0, 60.0),
    (40.0, 60.0),
    (-140.0, 60.0),
    (140.0, 60.0),
    (-40.0, -60.0),
    (40.0, -60.0),
    (-140.0, -60.0),
    (140.0, -60.0),
];

#[inline]
fn is_front_externalization_lane(channel: usize) -> bool {
    matches!(channel, 0 | 1 | 9 | 10)
}

#[derive(Clone, Copy, Default)]
struct PathTap {
    delay_samples: f32,
    gain: f32,
    hf_gain: f32,
    tone_state: f32,
}

#[derive(Clone, Copy, Default)]
struct SecondOrderPathTap {
    tap: PathTap,
    wall: usize,
}

#[derive(Clone, Copy)]
struct TransientReflectionExciter {
    fast_energy: f32,
    slow_energy: f32,
    envelope: f32,
    fast_alpha: f32,
    slow_alpha: f32,
    release_coeff: f32,
    max_delta: f32,
}

impl TransientReflectionExciter {
    fn new(sample_rate_hz: u32) -> Self {
        let sample_rate_hz = sample_rate_hz.max(1) as f32;
        let one_pole_alpha = |time_ms: f32| {
            1.0 - (-1.0 / (0.001 * time_ms.max(0.01) * sample_rate_hz)).exp()
        };
        Self {
            fast_energy: 0.0,
            slow_energy: 0.0,
            envelope: 0.0,
            fast_alpha: one_pole_alpha(TRANSIENT_FAST_MS),
            slow_alpha: one_pole_alpha(TRANSIENT_SLOW_MS),
            release_coeff: (-1.0
                / (0.001 * TRANSIENT_RELEASE_MS.max(0.01) * sample_rate_hz))
                .exp(),
            max_delta: 10.0_f32.powf(TRANSIENT_MAX_GAIN_DB / 20.0) - 1.0,
        }
    }

    #[inline]
    fn gain(&mut self, input: f32) -> f32 {
        let energy = input * input;
        self.fast_energy += self.fast_alpha * (energy - self.fast_energy);
        self.slow_energy += self.slow_alpha * (energy - self.slow_energy);

        let target = if self.fast_energy > TRANSIENT_MIN_RMS * TRANSIENT_MIN_RMS {
            let positive_rise = (self.fast_energy - self.slow_energy).max(0.0);
            let relative_rise = positive_rise / (self.slow_energy + 1.0e-9);
            ((relative_rise - TRANSIENT_RISE_THRESHOLD)
                / (TRANSIENT_FULL_RISE - TRANSIENT_RISE_THRESHOLD))
                .clamp(0.0, 1.0)
        } else {
            0.0
        };

        if target > self.envelope {
            self.envelope = target;
        } else {
            self.envelope *= self.release_coeff;
        }

        1.0 + self.max_delta * self.envelope
    }
}

struct SourceReflectionBank {
    ring: Vec<f32>,
    write_pos: usize,
    taps: [PathTap; NUM_REFLECTIONS],
    second_order_taps: Vec<SecondOrderPathTap>,
    tone_alpha: f32,
    air_state: f32,
    air_coeff: f32,
    transient: TransientReflectionExciter,
}

impl SourceReflectionBank {
    fn new(
        sample_rate_hz: u32,
        source_m: [f32; 3],
        enable_second_order: bool,
    ) -> (Self, [[f32; 3]; NUM_REFLECTIONS], [f32; NUM_REFLECTIONS]) {
        let cap = (RING_CAPACITY_S * sample_rate_hz as f32).ceil() as usize + 2;
        let direct_distance = norm(source_m).max(MIN_DISTANCE_M);
        let images = reflections::first_order_images(source_m, CURRENT_ROOM_M);
        let mut taps = [PathTap::default(); NUM_REFLECTIONS];
        let mut directions = [[0.0f32; 3]; NUM_REFLECTIONS];
        let mut direction_weights = [0.0f32; NUM_REFLECTIONS];

        for (i, image) in images.iter().copied().enumerate() {
            let image_distance = norm(image).max(MIN_DISTANCE_M);
            let relative_path_m = (image_distance - direct_distance).max(0.0);
            let delay_s = relative_path_m / reflections::speed_of_sound();
            let distance_gain = (REF_DISTANCE_M / image_distance).clamp(0.0, MAX_DISTANCE_GAIN);
            let hf_gain = (GENERIC_WALL_HF_AMPLITUDE
                * (-EXTRA_PATH_HF_DECAY_PER_M * relative_path_m).exp())
            .clamp(0.45, 0.90);
            let gain = CURRENT_REFLECTION_LEVEL * distance_gain;
            taps[i] = PathTap {
                delay_samples: (delay_s * sample_rate_hz as f32).clamp(0.0, (cap - 2) as f32),
                gain,
                hf_gain,
                tone_state: 0.0,
            };
            directions[i] = normalized(image);
            // Preserve the accepted wall-bus HRTF geometry even when a front
            // lane redistributes some first-order energy into later arrivals.
            direction_weights[i] = gain;
        }

        let mut second_order_taps = Vec::new();
        if enable_second_order {
            for image in second_order_images(source_m, CURRENT_ROOM_M) {
                let image_distance = norm(image).max(MIN_DISTANCE_M);
                let relative_path_m = (image_distance - direct_distance).max(0.0);
                let delay_s = relative_path_m / reflections::speed_of_sound();
                if delay_s > SECOND_ORDER_MAX_DELAY_S {
                    continue;
                }

                let distance_gain = (REF_DISTANCE_M / image_distance).clamp(0.0, MAX_DISTANCE_GAIN);
                let hf_gain = (GENERIC_WALL_HF_AMPLITUDE
                    * GENERIC_WALL_HF_AMPLITUDE
                    * (-EXTRA_PATH_HF_DECAY_PER_M * relative_path_m).exp())
                .clamp(0.30, 0.78);
                second_order_taps.push(SecondOrderPathTap {
                    tap: PathTap {
                        delay_samples: (delay_s * sample_rate_hz as f32)
                            .clamp(0.0, (cap - 2) as f32),
                        gain: CURRENT_REFLECTION_LEVEL
                            * CURRENT_REFLECTION_LEVEL
                            * distance_gain,
                        hf_gain,
                        tone_state: 0.0,
                    },
                    wall: final_wall_for_image(image, CURRENT_ROOM_M),
                });
            }

            let first_power: f32 = taps.iter().map(|tap| tap.gain * tap.gain).sum();
            let raw_second_power: f32 = second_order_taps
                .iter()
                .map(|path| path.tap.gain * path.tap.gain)
                .sum();
            if first_power > 1.0e-12 && raw_second_power > 1.0e-12 {
                let first_scale = (1.0 - FRONT_SECOND_ORDER_POWER_FRACTION).sqrt();
                let target_second_power = first_power * FRONT_SECOND_ORDER_POWER_FRACTION;
                let second_scale = (target_second_power / raw_second_power).sqrt();
                for tap in &mut taps {
                    tap.gain *= first_scale;
                }
                for path in &mut second_order_taps {
                    path.tap.gain *= second_scale;
                }
            }
        }

        let air_coeff = if direct_distance > 3.0 {
            let fc = (20_000.0 * (-0.05 * (direct_distance - 3.0)).exp()).max(2_000.0);
            (-std::f32::consts::TAU * fc / sample_rate_hz as f32).exp()
        } else {
            0.0
        };

        (
            Self {
                ring: vec![0.0; cap],
                write_pos: 0,
                taps,
                second_order_taps,
                tone_alpha: 1.0
                    - (-std::f32::consts::TAU * TONE_SPLIT_HZ / sample_rate_hz as f32).exp(),
                air_state: 0.0,
                air_coeff,
                transient: TransientReflectionExciter::new(sample_rate_hz),
            },
            directions,
            direction_weights,
        )
    }

    #[inline]
    fn process(&mut self, mut input: f32) -> [f32; NUM_REFLECTIONS] {
        // Only the signal entering the early-reflection delay bank receives the
        // transient-dependent gain. The direct master and primary spatial field
        // are outside this module and therefore cannot be reshaped by it.
        input *= self.transient.gain(input);

        if self.air_coeff > 0.0 {
            self.air_state += (input - self.air_state) * (1.0 - self.air_coeff);
            input = self.air_state;
        }

        let cap = self.ring.len();
        self.ring[self.write_pos] = input;
        let mut out = [0.0f32; NUM_REFLECTIONS];
        for (i, tap) in self.taps.iter_mut().enumerate() {
            let delayed = read_frac(&self.ring, cap, self.write_pos, tap.delay_samples);
            tap.tone_state += (delayed - tap.tone_state) * self.tone_alpha;
            let toned = tap.tone_state + tap.hf_gain * (delayed - tap.tone_state);
            out[i] = tap.gain * toned;
        }
        for path in &mut self.second_order_taps {
            let tap = &mut path.tap;
            let delayed = read_frac(&self.ring, cap, self.write_pos, tap.delay_samples);
            tap.tone_state += (delayed - tap.tone_state) * self.tone_alpha;
            let toned = tap.tone_state + tap.hf_gain * (delayed - tap.tone_state);
            out[path.wall] += tap.gain * toned;
        }
        self.write_pos += 1;
        if self.write_pos >= cap {
            self.write_pos = 0;
        }
        out
    }
}

struct HrtfWallBus {
    delay_l: DelayLine,
    delay_r: DelayLine,
    conv_l: EarConvolver,
    conv_r: EarConvolver,
}

impl HrtfWallBus {
    fn new(sample_rate_hz: u32, hrir: &HrirSet, direction: [f32; 3]) -> Self {
        let az = direction[0].atan2(direction[1]);
        let horiz = (direction[0] * direction[0] + direction[1] * direction[1]).sqrt();
        let el = direction[2].atan2(horiz);
        let mut pair = HrirPair {
            left: [0.0; HRIR_LEN],
            right: [0.0; HRIR_LEN],
        };
        hrir.at(az.to_degrees(), el.to_degrees(), &mut pair);

        let max_itd = (ITD_MAX_S * sample_rate_hz as f32).ceil() as usize;
        let mut delay_l = DelayLine::new(max_itd);
        let mut delay_r = DelayLine::new(max_itd);
        let (itd_l, itd_r) = itd::ear_delays_seconds(az, el, itd::DEFAULT_HEAD_RADIUS_M);
        delay_l.set_target_ms(itd_l * 1_000.0, sample_rate_hz);
        delay_r.set_target_ms(itd_r * 1_000.0, sample_rate_hz);

        let mut conv_l = EarConvolver::new();
        let mut conv_r = EarConvolver::new();
        conv_l.set_coeffs(&pair.left);
        conv_r.set_coeffs(&pair.right);
        Self { delay_l, delay_r, conv_l, conv_r }
    }

    #[inline]
    fn process(&mut self, input: f32) -> (f32, f32) {
        let x = input * HRTF_POWER_MATCH;
        (
            self.conv_l.process(self.delay_l.process(x)),
            self.conv_r.process(self.delay_r.process(x)),
        )
    }
}

pub(crate) struct HrtfEarlyReflectionField {
    sources: Vec<Option<SourceReflectionBank>>,
    buses: [HrtfWallBus; NUM_REFLECTIONS],
}

impl HrtfEarlyReflectionField {
    pub(crate) fn new(sample_rate_hz: u32) -> Self {
        Self::new_with_front_second_order(sample_rate_hz, true)
    }

    fn new_with_front_second_order(sample_rate_hz: u32, enable_front_second_order: bool) -> Self {
        let mut sources = Vec::with_capacity(MUSIC_FIELD_CHANNELS);
        let mut direction_sum = [[0.0f32; 3]; NUM_REFLECTIONS];
        let mut direction_weight = [0.0f32; NUM_REFLECTIONS];

        for channel in 0..MUSIC_FIELD_CHANNELS {
            if matches!(channel, 2 | 3) {
                sources.push(None);
                continue;
            }
            let (az, el) = LANE_DIRECTIONS_DEG[channel];
            let source_m = spherical_position(az, el, CURRENT_UNIT_SCALE_M);
            let enable_second_order =
                enable_front_second_order && is_front_externalization_lane(channel);
            let (bank, directions, baseline_weights) =
                SourceReflectionBank::new(sample_rate_hz, source_m, enable_second_order);
            for wall in 0..NUM_REFLECTIONS {
                let w = baseline_weights[wall].max(0.0);
                for axis in 0..3 {
                    direction_sum[wall][axis] += directions[wall][axis] * w;
                }
                direction_weight[wall] += w;
            }
            sources.push(Some(bank));
        }

        let measured = MeasuredHrirData::saf_kemar().resampled_to(sample_rate_hz);
        let hrir = HrirSet::new(&measured, sample_rate_hz);
        let fallback: [[f32; 3]; NUM_REFLECTIONS] = [
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ];
        let directions: [[f32; 3]; NUM_REFLECTIONS] = std::array::from_fn(|wall| {
            if direction_weight[wall] > 1e-9 {
                normalized(direction_sum[wall])
            } else {
                fallback[wall]
            }
        });
        let buses: [HrtfWallBus; NUM_REFLECTIONS] = std::array::from_fn(|wall| {
            HrtfWallBus::new(sample_rate_hz, &hrir, directions[wall])
        });
        Self { sources, buses }
    }

    pub(crate) fn process(&mut self, field_input: &[f32]) -> anyhow::Result<Vec<f32>> {
        if field_input.len() % MUSIC_FIELD_CHANNELS != 0 {
            bail!(
                "HRTF early-reflection field expected {}-channel interleaved support, got {} samples",
                MUSIC_FIELD_CHANNELS,
                field_input.len()
            );
        }
        let frames = field_input.len() / MUSIC_FIELD_CHANNELS;
        let mut out = vec![0.0f32; frames * 2];
        for frame in 0..frames {
            let mut wall_bus = [0.0f32; NUM_REFLECTIONS];
            let base = frame * MUSIC_FIELD_CHANNELS;
            for channel in 0..MUSIC_FIELD_CHANNELS {
                let Some(source) = self.sources[channel].as_mut() else { continue; };
                let paths = source.process(field_input[base + channel]);
                for wall in 0..NUM_REFLECTIONS {
                    wall_bus[wall] += paths[wall];
                }
            }
            let o = frame * 2;
            for (wall, bus) in self.buses.iter_mut().enumerate() {
                let (l, r) = bus.process(wall_bus[wall]);
                out[o] += l;
                out[o + 1] += r;
            }
        }
        Ok(out)
    }
}

fn spherical_position(az_deg: f32, el_deg: f32, distance_m: f32) -> [f32; 3] {
    let az = az_deg.to_radians();
    let el = el_deg.to_radians();
    let ce = el.cos();
    [
        distance_m * ce * az.sin(),
        distance_m * ce * az.cos(),
        distance_m * el.sin(),
    ]
}

/// Exact order-2 shoebox image positions using the same discrete L1 lattice as
/// mature image-source implementations. The room is listener-centred here, so
/// the source is temporarily translated into a [0, L] frame for the lattice
/// formula and translated back afterwards.
fn second_order_images(src_m: [f32; 3], room_m: [f32; 3]) -> Vec<[f32; 3]> {
    let mut room = [0.0f32; 3];
    let mut half = [0.0f32; 3];
    let mut source_from_min = [0.0f32; 3];
    for axis in 0..3 {
        room[axis] = room_m[axis].clamp(reflections::MIN_ROOM_M, reflections::MAX_ROOM_M);
        half[axis] = 0.5 * room[axis];
        let centred = src_m[axis].clamp(
            -(half[axis] - SECOND_ORDER_WALL_MARGIN_M),
            half[axis] - SECOND_ORDER_WALL_MARGIN_M,
        );
        source_from_min[axis] = centred + half[axis];
    }

    let mut images = Vec::with_capacity(SECOND_ORDER_IMAGE_COUNT);
    for z in -2_i32..=2 {
        for y in -2_i32..=2 {
            for x in -2_i32..=2 {
                let lattice = [x, y, z];
                if lattice.iter().map(|index| index.abs()).sum::<i32>() != 2 {
                    continue;
                }
                let mut image = [0.0f32; 3];
                for axis in 0..3 {
                    let index = lattice[axis];
                    let step = if index.abs() % 2 == 1 {
                        room[axis] - source_from_min[axis]
                    } else {
                        source_from_min[axis]
                    };
                    image[axis] = index as f32 * room[axis] + step - half[axis];
                }
                images.push(image);
            }
        }
    }
    debug_assert_eq!(images.len(), SECOND_ORDER_IMAGE_COUNT);
    images
}

/// Trace from the listener at the room centre toward an image source. The first
/// room boundary crossed is the physical wall of the final reflection before
/// the ray reaches the listener. Wall ordering matches `first_order_images`:
/// +X, -X, +Y, -Y, +Z, -Z.
fn final_wall_for_image(image_m: [f32; 3], room_m: [f32; 3]) -> usize {
    let mut best_wall = 0usize;
    let mut best_fraction = f32::INFINITY;
    for axis in 0..3 {
        let half = 0.5 * room_m[axis].clamp(reflections::MIN_ROOM_M, reflections::MAX_ROOM_M);
        let magnitude = image_m[axis].abs();
        if magnitude <= 1.0e-9 {
            continue;
        }
        let fraction = half / magnitude;
        if fraction < best_fraction {
            best_fraction = fraction;
            best_wall = axis * 2 + usize::from(image_m[axis] < 0.0);
        }
    }
    best_wall
}

#[inline]
fn norm(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[inline]
fn normalized(v: [f32; 3]) -> [f32; 3] {
    let n = norm(v).max(1e-9);
    [v[0] / n, v[1] / n, v[2] / n]
}

#[inline]
fn read_frac(ring: &[f32], cap: usize, write_pos: usize, delay: f32) -> f32 {
    let lo = delay.floor();
    let frac = delay - lo;
    let lo = lo as usize;
    let idx0 = (write_pos + cap - lo % cap) % cap;
    let idx1 = (idx0 + cap - 1) % cap;
    ring[idx0] * (1.0 - frac) + ring[idx1] * frac
}

#[cfg(test)]
mod tests {
    use super::*;

    fn impulse_field(frames: usize, channel: usize) -> Vec<f32> {
        let mut field = vec![0.0f32; frames * MUSIC_FIELD_CHANNELS];
        field[channel] = 1.0;
        field
    }

    fn tap_power(bank: &SourceReflectionBank) -> (f32, f32) {
        let first: f32 = bank.taps.iter().map(|tap| tap.gain * tap.gain).sum();
        let second: f32 = bank
            .second_order_taps
            .iter()
            .map(|path| path.tap.gain * path.tap.gain)
            .sum();
        (first, second)
    }

    #[test]
    fn lane_direction_table_matches_canonical_width() {
        assert_eq!(LANE_DIRECTIONS_DEG.len(), 17);
        assert_eq!(LANE_DIRECTIONS_DEG[8], (180.0, 0.0));
        assert!(LANE_DIRECTIONS_DEG[13].1 < 0.0);
        assert!(LANE_DIRECTIONS_DEG[16].1 < 0.0);
    }

    #[test]
    fn second_order_shoebox_has_eighteen_unique_images() {
        let images = second_order_images([1.0, 2.0, 0.5], [6.0, 8.0, 5.0]);
        assert_eq!(images.len(), SECOND_ORDER_IMAGE_COUNT);
        for i in 0..images.len() {
            for j in i + 1..images.len() {
                let delta = [
                    images[i][0] - images[j][0],
                    images[i][1] - images[j][1],
                    images[i][2] - images[j][2],
                ];
                assert!(norm(delta) > 1.0e-5, "duplicate order-2 image {i}/{j}");
            }
        }
    }

    #[test]
    fn front_second_order_redistributes_instead_of_adding_tap_power() {
        let source = spherical_position(-30.0, 0.0, CURRENT_UNIT_SCALE_M);
        let (baseline, _, _) = SourceReflectionBank::new(48_000, source, false);
        let (candidate, _, _) = SourceReflectionBank::new(48_000, source, true);
        let (baseline_first, baseline_second) = tap_power(&baseline);
        let (candidate_first, candidate_second) = tap_power(&candidate);
        assert_eq!(baseline_second, 0.0);
        assert!(!candidate.second_order_taps.is_empty());

        let baseline_total = baseline_first + baseline_second;
        let candidate_total = candidate_first + candidate_second;
        let relative_error = (candidate_total - baseline_total).abs() / baseline_total.max(1.0e-12);
        assert!(relative_error < 1.0e-5, "early-field tap power changed by {relative_error}");

        let fraction = candidate_second / candidate_total.max(1.0e-12);
        assert!(
            (fraction - FRONT_SECOND_ORDER_POWER_FRACTION).abs() < 1.0e-5,
            "second-order power fraction {fraction}"
        );
    }

    #[test]
    fn front_second_order_stays_inside_the_early_window() {
        let source = spherical_position(-30.0, 0.0, CURRENT_UNIT_SCALE_M);
        let (candidate, _, _) = SourceReflectionBank::new(48_000, source, true);
        assert!(!candidate.second_order_taps.is_empty());
        assert!(candidate.second_order_taps.len() < SECOND_ORDER_IMAGE_COUNT);
        let max_delay = SECOND_ORDER_MAX_DELAY_S * 48_000.0 + 1.0;
        assert!(candidate.second_order_taps.iter().all(|path| {
            path.wall < NUM_REFLECTIONS
                && path.tap.delay_samples > 0.0
                && path.tap.delay_samples <= max_delay
        }));
        let first_min = candidate
            .taps
            .iter()
            .map(|tap| tap.delay_samples)
            .fold(f32::INFINITY, f32::min);
        let second_min = candidate
            .second_order_taps
            .iter()
            .map(|path| path.tap.delay_samples)
            .fold(f32::INFINITY, f32::min);
        assert!(second_min > first_min, "second order arrived before the first early field");
    }

    #[test]
    fn nonfront_input_is_bit_exact_with_front_depth_candidate_disabled() {
        let input = impulse_field(8_000, 4);
        let mut baseline = HrtfEarlyReflectionField::new_with_front_second_order(48_000, false);
        let mut candidate = HrtfEarlyReflectionField::new_with_front_second_order(48_000, true);
        let expected = baseline.process(&input).unwrap();
        let actual = candidate.process(&input).unwrap();
        assert_eq!(expected.len(), actual.len());
        assert!(
            expected
                .iter()
                .zip(&actual)
                .all(|(a, b)| a.to_bits() == b.to_bits()),
            "front-only depth candidate changed a side-only reflection render"
        );
    }

    #[test]
    fn transient_exciter_is_bounded_and_returns_to_unity() {
        let mut exciter = TransientReflectionExciter::new(48_000);
        for _ in 0..1_024 {
            assert!((exciter.gain(0.0) - 1.0).abs() < 1.0e-7);
        }

        let peak = exciter.gain(0.5);
        let maximum = 10.0_f32.powf(TRANSIENT_MAX_GAIN_DB / 20.0);
        assert!(peak > 1.25, "impulse did not excite early room enough: {peak}");
        assert!(peak <= maximum + 1.0e-6, "transient gain exceeded bound: {peak}");

        let mut settled = peak;
        for _ in 0..4_800 {
            settled = exciter.gain(0.0);
        }
        assert!(settled < 1.01, "transient room gain did not decay: {settled}");
    }

    #[test]
    fn steady_tone_does_not_sustain_transient_excitation() {
        let mut exciter = TransientReflectionExciter::new(48_000);
        let mut max_tail = 1.0f32;
        for sample in 0..48_000 {
            let x = (std::f32::consts::TAU * 1_000.0 * sample as f32 / 48_000.0).sin() * 0.2;
            let gain = exciter.gain(x);
            if sample > 9_600 { max_tail = max_tail.max(gain); }
        }
        assert!(max_tail < 1.005, "steady tone kept transient room excitation alive: {max_tail}");
    }

    #[test]
    fn sub_threshold_impulse_does_not_excitate_room() {
        let mut exciter = TransientReflectionExciter::new(48_000);
        let gain = exciter.gain(0.0001);
        assert!((gain - 1.0).abs() < 1.0e-7);
    }

    #[test]
    fn center_and_lfe_never_enter_reflection_support() {
        for channel in [2usize, 3usize] {
            let mut field = HrtfEarlyReflectionField::new(48_000);
            let out = field.process(&impulse_field(4_096, channel)).unwrap();
            assert!(out.iter().all(|x| x.abs() < 1e-10));
        }
    }

    #[test]
    fn early_reflection_field_is_delayed_not_a_second_direct_copy() {
        let mut field = HrtfEarlyReflectionField::new(48_000);
        let out = field.process(&impulse_field(6_000, 0)).unwrap();
        let early_energy: f32 = out[..960].iter().map(|x| x * x).sum();
        let tail_energy: f32 = out[960..].iter().map(|x| x * x).sum();
        assert!(early_energy < 1e-10, "early reflection arrived too soon: {early_energy}");
        assert!(tail_energy > 1e-8, "HRTF reflection field produced no delayed energy");
    }

    #[test]
    fn processing_is_block_boundary_invariant() {
        let input = impulse_field(8_000, 4);
        let mut whole = HrtfEarlyReflectionField::new(48_000);
        let expected = whole.process(&input).unwrap();

        let split_at_frames = 2_137usize;
        let split_at = split_at_frames * MUSIC_FIELD_CHANNELS;
        let mut split = HrtfEarlyReflectionField::new(48_000);
        let mut actual = split.process(&input[..split_at]).unwrap();
        actual.extend(split.process(&input[split_at..]).unwrap());

        assert_eq!(expected.len(), actual.len());
        let max_error = expected.iter().zip(&actual).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
        assert!(max_error < 1e-6, "callback boundary changed reflection field: {max_error}");
    }

    #[test]
    fn measured_hrtf_wall_bus_has_lateral_asymmetry() {
        let measured = MeasuredHrirData::saf_kemar().resampled_to(48_000);
        let hrir = HrirSet::new(&measured, 48_000);
        let mut right = HrtfWallBus::new(48_000, &hrir, [1.0, 0.0, 0.0]);
        let mut left = HrtfWallBus::new(48_000, &hrir, [-1.0, 0.0, 0.0]);
        let mut right_energy = [0.0f32; 2];
        let mut left_energy = [0.0f32; 2];
        for i in 0..512 {
            let x = if i == 0 { 1.0 } else { 0.0 };
            let (rl, rr) = right.process(x);
            let (ll, lr) = left.process(x);
            right_energy[0] += rl * rl;
            right_energy[1] += rr * rr;
            left_energy[0] += ll * ll;
            left_energy[1] += lr * lr;
        }
        assert!(right_energy[1] > right_energy[0], "right wall lacks right-ear dominance: {right_energy:?}");
        assert!(left_energy[0] > left_energy[1], "left wall lacks left-ear dominance: {left_energy:?}");
    }
}

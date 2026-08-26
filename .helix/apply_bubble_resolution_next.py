from pathlib import Path

root = Path.cwd()
hrir_path = root / 'omniphony-renderer/renderer/src/binaural/hrir.rs'
measured_path = root / 'omniphony-renderer/renderer/src/binaural/measured.rs'
late_path = root / 'omniphony-renderer/orender_engine/src/music_late_enclosure.rs'


def require_once(text: str, needle: str, label: str) -> None:
    count = text.count(needle)
    if count != 1:
        raise RuntimeError(f'{label}: expected one anchor, found {count}')


# ---------------------------------------------------------------------------
# Upstream correctness ports that do not affect Current's SAF/KEMAR sound.
# ---------------------------------------------------------------------------
hrir = hrir_path.read_text(encoding='utf-8').replace('\r\n', '\n')
old_front = '        let front = (0.5 * az).cos().clamp(0.0, 1.0);'
require_once(hrir, old_front, 'pinna front/back factor')
hrir = hrir.replace(
    old_front,
    '''        // Brown & Duda assumes signed azimuth in [-180, 180], while the\n        // grid feeds [0, 360). The absolute form is the same even periodic\n        // factor under both conventions and preserves left/right symmetry.\n        let front = (0.5 * az).cos().abs();''',
    1,
)

hrir_end = '''        for n in 0..HRIR_LEN {\n            out.left[n] =\n                w00 * p00.left[n] + w10 * p10.left[n] + w01 * p01.left[n] + w11 * p11.left[n];\n            out.right[n] =\n                w00 * p00.right[n] + w10 * p10.right[n] + w01 * p01.right[n] + w11 * p11.right[n];\n        }\n    }\n}\n\n#[cfg(test)]'''
require_once(hrir, hrir_end, 'HrirSet impl tail')
hrir = hrir.replace(
    hrir_end,
    '''        for n in 0..HRIR_LEN {\n            out.left[n] =\n                w00 * p00.left[n] + w10 * p10.left[n] + w01 * p01.left[n] + w11 * p11.left[n];\n            out.right[n] =\n                w00 * p00.right[n] + w10 * p10.right[n] + w01 * p01.right[n] + w11 * p11.right[n];\n        }\n    }\n\n    /// Largest absolute tap in the grid. Build-time validation only.\n    pub fn peak(&self) -> f32 {\n        self.grid\n            .iter()\n            .flat_map(|p| p.left.iter().chain(p.right.iter()))\n            .fold(0.0f32, |m, &x| m.max(x.abs()))\n    }\n\n    /// Whether every grid node holds the same left/right kernel.\n    pub fn is_direction_invariant(&self) -> bool {\n        let Some((first, rest)) = self.grid.split_first() else {\n            return true;\n        };\n        rest.iter()\n            .all(|p| p.left == first.left && p.right == first.right)\n    }\n}\n\n#[cfg(test)]''',
    1,
)

# Add a compact regression for the upstream pinna symmetry bug.
insert_at = hrir.rfind('\n}')
if insert_at < 0:
    raise RuntimeError('hrir tests: final module brace not found')
hrir_test = r'''

    #[test]
    fn pinna_provider_is_symmetric_across_grid_azimuth_wrap() {
        let p = ParametricPinnaHrir {
            d: ParametricPinnaHrir::D_PB_NH,
            depth: 1.0,
        };
        for el in [-30.0f32, 0.0, 30.0] {
            for az in [10.0f32, 30.0, 90.0, 150.0] {
                let a = p.render(az, el, 48_000);
                let b = p.render(360.0 - az, el, 48_000);
                let lr: f32 = a.left.iter().zip(&b.right).map(|(x, y)| (x - y) * (x - y)).sum();
                let rl: f32 = a.right.iter().zip(&b.left).map(|(x, y)| (x - y) * (x - y)).sum();
                assert!(lr < 1.0e-8 && rl < 1.0e-8, "az={az} el={el} lr={lr:e} rl={rl:e}");
            }
        }
    }
'''
hrir = hrir[:insert_at] + hrir_test + hrir[insert_at:]
hrir_path.write_text(hrir, encoding='utf-8', newline='\n')

measured = measured_path.read_text(encoding='utf-8').replace('\r\n', '\n')
sofa_old = '''    let filter_len = sofa.filter_len();\n    let provider = SofaProvider { sofa, filter_len };\n    Ok(super::hrir::HrirSet::new(&provider, sample_rate))\n}\n'''
require_once(measured, sofa_old, 'SOFA build return')
sofa_new = r'''    let filter_len = sofa.filter_len();
    let provider = SofaProvider { sofa, filter_len };
    let set = super::hrir::HrirSet::new(&provider, sample_rate);
    check_loaded_set(&set, path, filter_len)?;
    Ok(set)
}

const SILENT_SOFA_PEAK: f32 = 1.0e-9;

fn check_loaded_set(
    set: &super::hrir::HrirSet,
    path: &str,
    filter_len: usize,
) -> anyhow::Result<()> {
    if set.peak() <= SILENT_SOFA_PEAK {
        anyhow::bail!(
            "SOFA '{path}' builds a silent HRIR set (filter length {filter_len}); room impulse-response conventions are not supported by this free-field binaural loader"
        );
    }
    if set.is_direction_invariant() {
        log::warn!(
            "SOFA '{path}' resolves every direction to the same impulse response; binaural direction will not move"
        );
    }
    Ok(())
}
'''
measured = measured.replace(sofa_old, sofa_new, 1)

# Guard the checker without needing a real SOFA file.
insert_at = measured.rfind('\n}')
if insert_at < 0:
    raise RuntimeError('measured tests: final module brace not found')
measured_test = r'''

    struct SilentProvider;
    impl HrirProvider for SilentProvider {
        fn render(&self, _az: f32, _el: f32, _fs: u32) -> HrirPair {
            HrirPair { left: [0.0; HRIR_LEN], right: [0.0; HRIR_LEN] }
        }
    }

    #[test]
    fn silent_hrir_grid_is_detectable() {
        let set = HrirSet::new(&SilentProvider, 48_000);
        assert_eq!(set.peak(), 0.0);
        assert!(check_loaded_set(&set, "silent.sofa", 7).is_err());
    }
'''
measured = measured[:insert_at] + measured_test + measured[insert_at:]
measured_path.write_text(measured, encoding='utf-8', newline='\n')

# ---------------------------------------------------------------------------
# Current late field: dense first-order HRTF projection.
# ---------------------------------------------------------------------------
late = late_path.read_text(encoding='utf-8').replace('\r\n', '\n')

# Keep the old six-axis implementation only as a regression oracle in tests.
late = late.replace(
    'use renderer::binaural::convolver::EarConvolver;\n',
    '#[cfg(test)]\nuse renderer::binaural::convolver::EarConvolver;\n',
    1,
)
late = late.replace(
    'use renderer::delay_line::DelayLine;\n',
    '#[cfg(test)]\nuse renderer::delay_line::DelayLine;\n',
    1,
)

# Refresh the module description so it says what production actually does.
late = late.replace(
    '//! decoded energy-neutrally to six virtual directions ±X / ±Y / ±Z, then rendered through the same embedded SAF/KEMAR HRTF family\n//! used elsewhere in the Current support renderer.\n',
    '//! projected once at construction into four first-order binaural filters W/X/Y/Z\n//! from a dense equal-area SAF/KEMAR sphere. Runtime therefore keeps a continuous\n//! first-order field without a six-cardinal-speaker lattice.\n',
)

start = late.find('struct AxisHrtfBus {')
end = late.find('pub(crate) struct HrtfLateEnclosure')
if start < 0 or end < 0 or end <= start:
    raise RuntimeError('late HRTF axis block markers not found')

new_block = r'''const SH_AZIMUTH_SAMPLES: usize = 24;
const SH_Z_RINGS: usize = 12;
const PROJECTED_FIR_LANES: usize = 8;

/// Fixed stereo FIR for one FOA coefficient. Both ears share one input history.
/// Coefficients are built once from the measured sphere; the realtime path only
/// performs two contiguous dot products with no allocation or coefficient swap.
struct StereoProjectedFir {
    hist: Vec<f32>,
    pos: usize,
    taps: usize,
    left_rev: Vec<f32>,
    right_rev: Vec<f32>,
}

impl StereoProjectedFir {
    fn new(left: Vec<f32>, right: Vec<f32>) -> Self {
        assert_eq!(left.len(), right.len());
        assert!(left.len().is_multiple_of(PROJECTED_FIR_LANES));
        let taps = left.len();
        Self {
            hist: vec![0.0; 2 * taps],
            pos: 0,
            taps,
            left_rev: left.into_iter().rev().collect(),
            right_rev: right.into_iter().rev().collect(),
        }
    }

    #[inline(always)]
    fn process(&mut self, input: f32) -> (f32, f32) {
        self.pos = if self.pos + 1 == self.taps { 0 } else { self.pos + 1 };
        self.hist[self.pos] = input;
        self.hist[self.pos + self.taps] = input;
        let win = &self.hist[self.pos + 1..self.pos + 1 + self.taps];
        let mut acc_l = [0.0f32; PROJECTED_FIR_LANES];
        let mut acc_r = [0.0f32; PROJECTED_FIR_LANES];
        for ((cl, cr), h) in self
            .left_rev
            .chunks_exact(PROJECTED_FIR_LANES)
            .zip(self.right_rev.chunks_exact(PROJECTED_FIR_LANES))
            .zip(win.chunks_exact(PROJECTED_FIR_LANES))
        {
            for lane in 0..PROJECTED_FIR_LANES {
                let x = h[lane];
                acc_l[lane] += cl[lane] * x;
                acc_r[lane] += cr[lane] * x;
            }
        }
        (acc_l.iter().sum(), acc_r.iter().sum())
    }
}

#[inline]
fn effective_filter_len(sample_rate: u32) -> usize {
    let (max_itd, _) = itd::ear_delays_seconds(
        std::f32::consts::FRAC_PI_2,
        0.0,
        itd::DEFAULT_HEAD_RADIUS_M,
    );
    let needed = HRIR_LEN + (max_itd * sample_rate as f32).ceil() as usize;
    needed.next_multiple_of(PROJECTED_FIR_LANES)
}

fn bake_fractional_delay(
    ir: &[f32; HRIR_LEN],
    delay_samples: f32,
    filter_len: usize,
) -> Vec<f32> {
    let delay = delay_samples.max(0.0);
    let whole = delay.floor() as usize;
    let frac = delay - whole as f32;
    let mut out = vec![0.0f32; filter_len];
    for (tap, &sample) in ir.iter().enumerate() {
        let i0 = tap + whole;
        if i0 < filter_len {
            out[i0] += sample * (1.0 - frac);
        }
        let i1 = i0 + 1;
        if frac != 0.0 && i1 < filter_len {
            out[i1] += sample * frac;
        }
    }
    out
}

fn direction_pair(
    sample_rate: u32,
    hrir: &HrirSet,
    direction: [f32; 3],
    filter_len: usize,
) -> (Vec<f32>, Vec<f32>) {
    let az = direction[0].atan2(direction[1]);
    let horiz = (direction[0] * direction[0] + direction[1] * direction[1]).sqrt();
    let el = direction[2].atan2(horiz);
    let mut pair = HrirPair {
        left: [0.0; HRIR_LEN],
        right: [0.0; HRIR_LEN],
    };
    hrir.at(az.to_degrees(), el.to_degrees(), &mut pair);
    let (itd_l, itd_r) = itd::ear_delays_seconds(az, el, itd::DEFAULT_HEAD_RADIUS_M);
    (
        bake_fractional_delay(&pair.left, itd_l * sample_rate as f32, filter_len),
        bake_fractional_delay(&pair.right, itd_r * sample_rate as f32, filter_len),
    )
}

/// Project a set of equal-weight directions onto the fork's orthonormal FOA
/// coefficient convention. A least-squares first-order fit is used per FIR tap.
/// The final scaling is anchored so the six cardinal directions reproduce the
/// previous FOA_TO_AXES decoder exactly for any transfer function that is first
/// order over the sphere: W = sqrt(6) * monopole, XYZ = sqrt(2) * dipoles.
fn project_directions_to_foa(
    sample_rate: u32,
    hrir: &HrirSet,
    directions: &[[f32; 3]],
) -> [StereoProjectedFir; FIELD_CHANNELS] {
    assert!(!directions.is_empty());
    let filter_len = effective_filter_len(sample_rate);
    let inv_n = 1.0 / directions.len() as f32;
    let mut gram = [0.0f32; FIELD_CHANNELS];
    let mut left: [Vec<f32>; FIELD_CHANNELS] =
        std::array::from_fn(|_| vec![0.0; filter_len]);
    let mut right: [Vec<f32>; FIELD_CHANNELS] =
        std::array::from_fn(|_| vec![0.0; filter_len]);

    for &direction in directions {
        let basis = [1.0, direction[0], direction[1], direction[2]];
        for c in 0..FIELD_CHANNELS {
            gram[c] += inv_n * basis[c] * basis[c];
        }
        let (eff_l, eff_r) = direction_pair(sample_rate, hrir, direction, filter_len);
        for c in 0..FIELD_CHANNELS {
            let w = inv_n * basis[c];
            for tap in 0..filter_len {
                left[c][tap] += w * eff_l[tap];
                right[c][tap] += w * eff_r[tap];
            }
        }
    }

    let target_scale = [6.0f32.sqrt(), 2.0f32.sqrt(), 2.0f32.sqrt(), 2.0f32.sqrt()];
    for c in 0..FIELD_CHANNELS {
        let scale = target_scale[c] / gram[c].max(1.0e-9);
        for tap in 0..filter_len {
            left[c][tap] *= scale;
            right[c][tap] *= scale;
        }
    }

    let mut buses: Vec<StereoProjectedFir> = (0..FIELD_CHANNELS)
        .map(|c| StereoProjectedFir::new(std::mem::take(&mut left[c]), std::mem::take(&mut right[c])))
        .collect();
    std::array::from_fn(|_| buses.remove(0))
}

fn dense_projection_directions() -> Vec<[f32; 3]> {
    let mut directions = Vec::with_capacity(SH_AZIMUTH_SAMPLES * SH_Z_RINGS);
    for zi in 0..SH_Z_RINGS {
        // Equal-area midpoint rings: z is uniform, while azimuth is uniform on
        // every ring. Opposite rings and the complete azimuth cycles make all
        // first-order cross terms cancel to floating-point noise.
        let z = -1.0 + 2.0 * (zi as f32 + 0.5) / SH_Z_RINGS as f32;
        let radius = (1.0 - z * z).max(0.0).sqrt();
        for ai in 0..SH_AZIMUTH_SAMPLES {
            let az = std::f32::consts::TAU * ai as f32 / SH_AZIMUTH_SAMPLES as f32;
            directions.push([radius * az.sin(), radius * az.cos(), z]);
        }
    }
    directions
}

fn build_dense_foa_hrtf(
    sample_rate: u32,
    hrir: &HrirSet,
) -> [StereoProjectedFir; FIELD_CHANNELS] {
    let directions = dense_projection_directions();
    project_directions_to_foa(sample_rate, hrir, &directions)
}

#[cfg(test)]
struct AxisHrtfBus {
    delay_l: DelayLine,
    delay_r: DelayLine,
    conv_l: EarConvolver,
    conv_r: EarConvolver,
}

#[cfg(test)]
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
        Self { delay_l, delay_r, conv_l, conv_r }
    }

    #[inline]
    fn process(&mut self, input: f32) -> (f32, f32) {
        (
            self.conv_l.process(self.delay_l.process(input)),
            self.conv_r.process(self.delay_r.process(input)),
        )
    }
}

'''
late = late[:start] + new_block + late[end:]

require_once(late, '    axes: [AxisHrtfBus; AXES],', 'late struct axes')
late = late.replace(
    '    axes: [AxisHrtfBus; AXES],',
    '    foa_hrtf: [StereoProjectedFir; FIELD_CHANNELS],',
    1,
)
old_init = '''            axes: std::array::from_fn(|axis| {\n                AxisHrtfBus::new(sample_rate, &hrir, AXIS_DIRECTIONS[axis])\n            }),'''
require_once(late, old_init, 'late axis init')
late = late.replace(old_init, '            foa_hrtf: build_dense_foa_hrtf(sample_rate, &hrir),', 1)

old_runtime = '''            let axis_input = decode_foa_to_axes(field);\n            for axis in 0..AXES {\n                let (l, r) = self.axes[axis].process(axis_input[axis]);\n                out[o] += l;\n                out[o + 1] += r;\n            }'''
require_once(late, old_runtime, 'late runtime axis decode')
late = late.replace(
    old_runtime,
    '''            for (channel, &sample) in field.iter().enumerate() {\n                let (l, r) = self.foa_hrtf[channel].process(sample);\n                out[o] += l;\n                out[o + 1] += r;\n            }''',
    1,
)

# Add objective projection tests before the tests module's closing brace.
insert_at = late.rfind('\n}')
if insert_at < 0:
    raise RuntimeError('late tests: final module brace not found')
late_tests = r'''

    #[test]
    fn dense_projection_grid_has_first_order_symmetry() {
        let directions = dense_projection_directions();
        let n = directions.len() as f32;
        let mean = [
            directions.iter().map(|d| d[0]).sum::<f32>() / n,
            directions.iter().map(|d| d[1]).sum::<f32>() / n,
            directions.iter().map(|d| d[2]).sum::<f32>() / n,
        ];
        for value in mean {
            assert!(value.abs() < 1.0e-6, "nonzero first moment {value:e}");
        }
        let xy = directions.iter().map(|d| d[0] * d[1]).sum::<f32>() / n;
        let xz = directions.iter().map(|d| d[0] * d[2]).sum::<f32>() / n;
        let yz = directions.iter().map(|d| d[1] * d[2]).sum::<f32>() / n;
        assert!(xy.abs() < 1.0e-6 && xz.abs() < 1.0e-6 && yz.abs() < 1.0e-6);
    }

    #[test]
    fn six_axis_filter_projection_matches_the_old_runtime_after_itd_settles() {
        let sample_rate = 48_000u32;
        let measured = MeasuredHrirData::saf_kemar().resampled_to(sample_rate);
        let hrir = HrirSet::new(&measured, sample_rate);
        let mut old_axes: [AxisHrtfBus; AXES] =
            std::array::from_fn(|axis| AxisHrtfBus::new(sample_rate, &hrir, AXIS_DIRECTIONS[axis]));
        let mut projected = project_directions_to_foa(sample_rate, &hrir, &AXIS_DIRECTIONS);

        // The historical DelayLine ramps to its static ITD target at one sample
        // per sample. Late energy itself has a 32 ms predelay, so this warm-up
        // always completes before audible late output in production.
        for _ in 0..256 {
            for axis in &mut old_axes {
                axis.process(0.0);
            }
            for bus in &mut projected {
                bus.process(0.0);
            }
        }

        let mut max_error = 0.0f32;
        for i in 0..768usize {
            let t = i as f32;
            let field = [
                (0.071 * t).sin() * 0.3,
                (0.113 * t + 0.2).sin() * 0.2,
                (0.047 * t + 0.7).cos() * 0.25,
                (0.089 * t + 1.1).sin() * 0.15,
            ];
            let axis_input = decode_foa_to_axes(field);
            let mut old = [0.0f32; 2];
            for axis in 0..AXES {
                let (l, r) = old_axes[axis].process(axis_input[axis]);
                old[0] += l;
                old[1] += r;
            }
            let mut new = [0.0f32; 2];
            for channel in 0..FIELD_CHANNELS {
                let (l, r) = projected[channel].process(field[channel]);
                new[0] += l;
                new[1] += r;
            }
            max_error = max_error.max((old[0] - new[0]).abs());
            max_error = max_error.max((old[1] - new[1]).abs());
        }
        assert!(max_error < 3.0e-5, "projected six-axis transfer drifted by {max_error:e}");
    }

    #[test]
    fn projected_fir_is_smaller_than_six_axis_runtime_at_48k() {
        let taps = effective_filter_len(48_000);
        assert_eq!(taps, 160);
        assert!(FIELD_CHANNELS * 2 * taps < AXES * 2 * HRIR_LEN);
    }
'''
late = late[:insert_at] + late_tests + late[insert_at:]
late_path.write_text(late, encoding='utf-8', newline='\n')

print('BUBBLE_RESOLUTION_PATCH_OK 1')

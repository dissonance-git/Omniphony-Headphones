from pathlib import Path

EARLY_PATH = Path("omniphony-renderer/orender_engine/src/music_early_reflections.rs")
SUPPORT_PATH = Path("omniphony-renderer/orender_engine/src/music_support.rs")

text = EARLY_PATH.read_text(encoding="utf-8")

old_doc = '''//! A literal measured-HRTF convolution for every image of every virtual support
//! source would multiply the FIR count by the speaker count. This module keeps
//! image-source timing and wall tone per support lane, groups contributions by
//! final-arrival wall, then applies exactly six measured SAF/KEMAR HRTFs.
'''
new_doc = '''//! A literal measured-HRTF convolution for every image of every virtual support
//! source would multiply the FIR count by the speaker count. This module keeps
//! image-source timing and wall tone per support lane, then routes each source-wall
//! contribution to a bounded set of direction clusters before measured SAF/KEMAR
//! HRTF rendering. The clustering is built once from the static image geometry;
//! no clustering or HRTF lookup occurs in the realtime sample loop.
'''
if old_doc not in text:
    raise SystemExit("early reflection module doc anchor not found")
text = text.replace(old_doc, new_doc, 1)

power_anchor = "const HRTF_POWER_MATCH: f32 = 0.816_496_6;\n"
cluster_constants = '''const HRTF_POWER_MATCH: f32 = 0.816_496_6;

// Ten clusters are the bounded Pareto point for the current 90 source-wall
// directions: materially lower directional quantization error than six global
// wall averages without paying for two additional HRTF buses that remain
// effectively redundant on this geometry.
const EARLY_HRTF_BUSES: usize = 10;
const EARLY_CLUSTER_ITERS: usize = 8;
const FIBONACCI_TURN: f32 = 0.618_033_95;
'''
if power_anchor not in text:
    raise SystemExit("HRTF power anchor not found")
text = text.replace(power_anchor, cluster_constants, 1)

text = text.replace("HrtfWallBus", "HrtfDirectionBus")

start = text.find("pub(crate) struct HrtfEarlyReflectionField")
end = text.find("\nfn spherical_position", start)
if start < 0 or end < 0:
    raise SystemExit("early field implementation anchors not found")

replacement = r'''#[derive(Clone, Copy)]
struct SourceWallGeometry {
    directions: [[f32; 3]; NUM_REFLECTIONS],
    weights: [f32; NUM_REFLECTIONS],
}

#[inline]
fn nearest_direction_cluster(
    direction: [f32; 3],
    centers: &[[f32; 3]; EARLY_HRTF_BUSES],
) -> usize {
    let mut best = 0usize;
    let mut best_dot = -f32::INFINITY;
    for (index, center) in centers.iter().enumerate() {
        let dot =
            direction[0] * center[0] + direction[1] * center[1] + direction[2] * center[2];
        if dot > best_dot {
            best_dot = dot;
            best = index;
        }
    }
    best
}

fn initial_direction_clusters() -> [[f32; 3]; EARLY_HRTF_BUSES] {
    std::array::from_fn(|index| {
        let i = index as f32;
        let z = 1.0 - 2.0 * (i + 0.5) / EARLY_HRTF_BUSES as f32;
        let radius = (1.0 - z * z).max(0.0).sqrt();
        let az = std::f32::consts::TAU * (i * FIBONACCI_TURN).fract();
        [radius * az.sin(), radius * az.cos(), z]
    })
}

/// Weighted spherical k-means over the static first-order source-wall image
/// directions. Initialization and the iteration count are fixed, so identical
/// geometry produces identical HRTF buses. Baseline reflection gain is used as
/// the weight, matching the old wall-direction averaging convention.
///
/// Only direction assignment changes. Delay, tap gain, wall tone, transient
/// excitation, second-order power redistribution and HRTF power matching remain
/// owned by the existing reflection banks.
fn cluster_early_directions(
    geometry: &[Option<SourceWallGeometry>],
) -> [[f32; 3]; EARLY_HRTF_BUSES] {
    let mut centers = initial_direction_clusters();

    for _ in 0..EARLY_CLUSTER_ITERS {
        let mut sums = [[0.0f32; 3]; EARLY_HRTF_BUSES];
        let mut weights = [0.0f32; EARLY_HRTF_BUSES];

        for source in geometry.iter().flatten() {
            for wall in 0..NUM_REFLECTIONS {
                let weight = source.weights[wall].max(0.0);
                if weight <= 1.0e-12 {
                    continue;
                }
                let direction = source.directions[wall];
                let cluster = nearest_direction_cluster(direction, &centers);
                for axis in 0..3 {
                    sums[cluster][axis] += weight * direction[axis];
                }
                weights[cluster] += weight;
            }
        }

        for cluster in 0..EARLY_HRTF_BUSES {
            if weights[cluster] > 1.0e-9 {
                centers[cluster] = normalized(sums[cluster]);
            }
        }
    }

    centers
}

fn build_cluster_routes(
    geometry: &[Option<SourceWallGeometry>],
    centers: &[[f32; 3]; EARLY_HRTF_BUSES],
) -> Vec<Option<[usize; NUM_REFLECTIONS]>> {
    geometry
        .iter()
        .map(|source| {
            source.map(|source| {
                std::array::from_fn(|wall| {
                    nearest_direction_cluster(source.directions[wall], centers)
                })
            })
        })
        .collect()
}

#[cfg(test)]
fn wall_average_directions(
    geometry: &[Option<SourceWallGeometry>],
) -> [[f32; 3]; NUM_REFLECTIONS] {
    let fallback: [[f32; 3]; NUM_REFLECTIONS] = [
        [1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
    ];
    std::array::from_fn(|wall| {
        let mut sum = [0.0f32; 3];
        let mut total = 0.0f32;
        for source in geometry.iter().flatten() {
            let weight = source.weights[wall].max(0.0);
            for axis in 0..3 {
                sum[axis] += weight * source.directions[wall][axis];
            }
            total += weight;
        }
        if total > 1.0e-9 {
            normalized(sum)
        } else {
            fallback[wall]
        }
    })
}

#[cfg(test)]
fn direction_quantization_error(
    geometry: &[Option<SourceWallGeometry>],
    directions: &[[f32; 3]],
    routes: &[Option<[usize; NUM_REFLECTIONS]>],
) -> f32 {
    let mut error = 0.0f32;
    for (channel, source) in geometry.iter().enumerate() {
        let Some(source) = source else {
            continue;
        };
        let Some(route) = routes[channel] else {
            continue;
        };
        for wall in 0..NUM_REFLECTIONS {
            let target = source.directions[wall];
            let rendered = directions[route[wall]];
            let dot = (target[0] * rendered[0]
                + target[1] * rendered[1]
                + target[2] * rendered[2])
                .clamp(-1.0, 1.0);
            error += source.weights[wall].max(0.0) * (1.0 - dot);
        }
    }
    error
}

pub(crate) struct HrtfEarlyReflectionField {
    sources: Vec<Option<SourceReflectionBank>>,
    routes: Vec<Option<[usize; NUM_REFLECTIONS]>>,
    buses: [HrtfDirectionBus; EARLY_HRTF_BUSES],
}

impl HrtfEarlyReflectionField {
    pub(crate) fn new(sample_rate_hz: u32) -> Self {
        Self::new_with_front_second_order(sample_rate_hz, true)
    }

    fn new_with_front_second_order(sample_rate_hz: u32, enable_front_second_order: bool) -> Self {
        let mut sources = Vec::with_capacity(MUSIC_FIELD_CHANNELS);
        let mut geometry = Vec::with_capacity(MUSIC_FIELD_CHANNELS);

        for channel in 0..MUSIC_FIELD_CHANNELS {
            if matches!(channel, 2 | 3) {
                sources.push(None);
                geometry.push(None);
                continue;
            }
            let (az, el) = LANE_DIRECTIONS_DEG[channel];
            let source_m = spherical_position(az, el, CURRENT_UNIT_SCALE_M);
            let enable_second_order =
                enable_front_second_order && is_front_externalization_lane(channel);
            let (bank, directions, weights) =
                SourceReflectionBank::new(sample_rate_hz, source_m, enable_second_order);
            sources.push(Some(bank));
            geometry.push(Some(SourceWallGeometry {
                directions,
                weights,
            }));
        }

        let directions = cluster_early_directions(&geometry);
        let routes = build_cluster_routes(&geometry, &directions);
        let measured = MeasuredHrirData::saf_kemar().resampled_to(sample_rate_hz);
        let hrir = HrirSet::new(&measured, sample_rate_hz);
        let buses: [HrtfDirectionBus; EARLY_HRTF_BUSES] = std::array::from_fn(|cluster| {
            HrtfDirectionBus::new(sample_rate_hz, &hrir, directions[cluster])
        });

        Self {
            sources,
            routes,
            buses,
        }
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
            let mut direction_bus = [0.0f32; EARLY_HRTF_BUSES];
            let base = frame * MUSIC_FIELD_CHANNELS;
            for channel in 0..MUSIC_FIELD_CHANNELS {
                let Some(route) = self.routes[channel] else {
                    continue;
                };
                let Some(source) = self.sources[channel].as_mut() else {
                    continue;
                };
                let paths = source.process(field_input[base + channel]);
                for wall in 0..NUM_REFLECTIONS {
                    direction_bus[route[wall]] += paths[wall];
                }
            }
            let o = frame * 2;
            for (cluster, bus) in self.buses.iter_mut().enumerate() {
                let (l, r) = bus.process(direction_bus[cluster]);
                out[o] += l;
                out[o + 1] += r;
            }
        }
        Ok(out)
    }
}
'''
text = text[:start] + replacement + text[end:]

test_anchor = "\n    #[test]\n    fn measured_hrtf_wall_bus_has_lateral_asymmetry() {"
if test_anchor not in text:
    raise SystemExit("test insertion anchor not found")

new_tests = r'''
    #[test]
    fn clustered_directions_reduce_wall_average_quantization_error() {
        let mut geometry = Vec::with_capacity(MUSIC_FIELD_CHANNELS);
        for channel in 0..MUSIC_FIELD_CHANNELS {
            if matches!(channel, 2 | 3) {
                geometry.push(None);
                continue;
            }
            let (az, el) = LANE_DIRECTIONS_DEG[channel];
            let source_m = spherical_position(az, el, CURRENT_UNIT_SCALE_M);
            let (_, directions, weights) = SourceReflectionBank::new(
                48_000,
                source_m,
                is_front_externalization_lane(channel),
            );
            geometry.push(Some(SourceWallGeometry {
                directions,
                weights,
            }));
        }

        let clustered = cluster_early_directions(&geometry);
        let routes = build_cluster_routes(&geometry, &clustered);
        let wall_average = wall_average_directions(&geometry);
        let wall_routes: Vec<Option<[usize; NUM_REFLECTIONS]>> = geometry
            .iter()
            .map(|source| source.map(|_| std::array::from_fn(|wall| wall)))
            .collect();

        let baseline_error =
            direction_quantization_error(&geometry, &wall_average, &wall_routes);
        let clustered_error = direction_quantization_error(&geometry, &clustered, &routes);
        assert!(
            clustered_error < baseline_error * 0.80,
            "direction clustering did not materially beat wall averaging: clustered={clustered_error:e} wall={baseline_error:e}"
        );

        let mut used = [false; EARLY_HRTF_BUSES];
        for route in routes.iter().flatten() {
            for &cluster in route {
                assert!(cluster < EARLY_HRTF_BUSES);
                used[cluster] = true;
            }
        }
        assert_eq!(
            used.iter().filter(|&&value| value).count(),
            EARLY_HRTF_BUSES,
            "bounded HRTF budget contains an unused direction cluster"
        );
    }

'''
text = text.replace(test_anchor, new_tests + test_anchor, 1)

EARLY_PATH.write_text(text, encoding="utf-8")

support = SUPPORT_PATH.read_text(encoding="utf-8")
old_support = '''    // The Current model owns first-order reflections in the fixed-cost six-bus
    // measured-HRTF field below, so disable the inherited analytic reflection
    // bank to prevent duplicate early energy.
'''
new_support = '''    // The Current model owns first-order reflections in the bounded clustered
    // measured-HRTF early field below, so disable the inherited analytic
    // reflection bank to prevent duplicate early energy.
'''
if old_support not in support:
    raise SystemExit("music support early-field comment anchor not found")
SUPPORT_PATH.write_text(support.replace(old_support, new_support, 1), encoding="utf-8")

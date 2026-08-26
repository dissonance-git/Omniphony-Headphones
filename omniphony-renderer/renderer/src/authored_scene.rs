//! Portable authored-object geometry and sample-time semantics.
//!
//! This module is intentionally small. It is the common vocabulary for richer
//! source adapters (Windows Spatial Audio, future ADM/BW64 ingestion, tests)
//! without importing platform APIs into the renderer core.
//!
//! Positions are listener-relative metres in Omniphony axes:
//! +X right, +Y forward, +Z up.

pub type MetricPosition = [f64; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleSpan {
    pub start_sample: u64,
    pub frame_count: u32,
}

impl SampleSpan {
    pub const fn new(start_sample: u64, frame_count: u32) -> Self {
        Self {
            start_sample,
            frame_count,
        }
    }

    pub fn end_sample_exclusive(self) -> u64 {
        self.start_sample.saturating_add(self.frame_count as u64)
    }

    pub fn contains(self, sample: u64) -> bool {
        sample >= self.start_sample && sample < self.end_sample_exclusive()
    }

    pub fn progress(self, sample: u64) -> f64 {
        if self.frame_count <= 1 {
            return if sample <= self.start_sample { 0.0 } else { 1.0 };
        }
        let last = self.end_sample_exclusive().saturating_sub(1);
        let clamped = sample.clamp(self.start_sample, last);
        (clamped - self.start_sample) as f64 / (self.frame_count - 1) as f64
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AuthoredObjectMotion {
    pub stable_id: u64,
    pub span: SampleSpan,
    pub start_position_m: MetricPosition,
    pub end_position_m: MetricPosition,
}

impl AuthoredObjectMotion {
    pub fn new(
        stable_id: u64,
        span: SampleSpan,
        start_position_m: MetricPosition,
        end_position_m: MetricPosition,
    ) -> Result<Self, &'static str> {
        if span.frame_count == 0 {
            return Err("authored object span must contain at least one frame");
        }
        if !finite_position(start_position_m) || !finite_position(end_position_m) {
            return Err("authored object position must be finite");
        }
        Ok(Self {
            stable_id,
            span,
            start_position_m,
            end_position_m,
        })
    }

    pub fn position_at(self, sample: u64) -> MetricPosition {
        let t = self.span.progress(sample);
        [
            lerp(self.start_position_m[0], self.end_position_m[0], t),
            lerp(self.start_position_m[1], self.end_position_m[1], t),
            lerp(self.start_position_m[2], self.end_position_m[2], t),
        ]
    }

    pub fn start_distance_m(self) -> f64 {
        radial_distance_m(self.start_position_m)
    }

    pub fn end_distance_m(self) -> f64 {
        radial_distance_m(self.end_position_m)
    }
}

pub fn finite_position(position_m: MetricPosition) -> bool {
    position_m.iter().all(|value| value.is_finite())
}

pub fn radial_distance_m(position_m: MetricPosition) -> f64 {
    (position_m[0] * position_m[0]
        + position_m[1] * position_m[1]
        + position_m[2] * position_m[2])
        .sqrt()
}

pub fn ear_positions_m(head_radius_m: f64) -> (MetricPosition, MetricPosition) {
    let radius = if head_radius_m.is_finite() {
        head_radius_m.max(0.0)
    } else {
        0.0
    };
    ([-radius, 0.0, 0.0], [radius, 0.0, 0.0])
}

pub fn ear_distances_m(position_m: MetricPosition, head_radius_m: f64) -> (f64, f64) {
    let (left, right) = ear_positions_m(head_radius_m);
    (
        distance_between(position_m, left),
        distance_between(position_m, right),
    )
}

pub fn ear_relative_directions(
    position_m: MetricPosition,
    head_radius_m: f64,
) -> (MetricPosition, MetricPosition) {
    let (left, right) = ear_positions_m(head_radius_m);
    (
        normalized(subtract(position_m, left)),
        normalized(subtract(position_m, right)),
    )
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn subtract(a: MetricPosition, b: MetricPosition) -> MetricPosition {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn distance_between(a: MetricPosition, b: MetricPosition) -> f64 {
    radial_distance_m(subtract(a, b))
}

fn normalized(v: MetricPosition) -> MetricPosition {
    let n = radial_distance_m(v);
    if !n.is_finite() || n <= 1.0e-12 {
        [0.0, 1.0, 0.0]
    } else {
        [v[0] / n, v[1] / n, v[2] / n]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_span_is_half_open_and_progress_is_callback_independent() {
        let span = SampleSpan::new(1_000, 5);
        assert!(span.contains(1_000));
        assert!(span.contains(1_004));
        assert!(!span.contains(1_005));
        assert_eq!(span.end_sample_exclusive(), 1_005);
        assert_eq!(span.progress(1_000), 0.0);
        assert_eq!(span.progress(1_002), 0.5);
        assert_eq!(span.progress(1_004), 1.0);
    }

    #[test]
    fn authored_motion_preserves_identity_and_metric_radius() {
        let motion = AuthoredObjectMotion::new(
            41,
            SampleSpan::new(10, 3),
            [0.0, 1.0, 0.0],
            [0.0, 3.0, 0.0],
        )
        .unwrap();
        assert_eq!(motion.stable_id, 41);
        assert_eq!(motion.position_at(10), [0.0, 1.0, 0.0]);
        assert_eq!(motion.position_at(11), [0.0, 2.0, 0.0]);
        assert_eq!(motion.position_at(12), [0.0, 3.0, 0.0]);
        assert_eq!(motion.start_distance_m(), 1.0);
        assert_eq!(motion.end_distance_m(), 3.0);
    }

    #[test]
    fn invalid_positions_and_empty_spans_are_rejected() {
        assert!(
            AuthoredObjectMotion::new(
                1,
                SampleSpan::new(0, 0),
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            )
            .is_err()
        );
        assert!(
            AuthoredObjectMotion::new(
                1,
                SampleSpan::new(0, 16),
                [f64::NAN, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            )
            .is_err()
        );
    }

    #[test]
    fn near_source_has_real_acoustic_parallax_between_ears() {
        let source = [0.15, 0.0, 0.0];
        let (left_distance, right_distance) = ear_distances_m(source, 0.0875);
        assert!(left_distance > right_distance * 3.0);

        let (left_dir, right_dir) = ear_relative_directions(source, 0.0875);
        assert!(left_dir[0] > 0.99);
        assert!(right_dir[0] > 0.99);
        assert_ne!(left_distance, right_distance);
    }

    #[test]
    fn far_source_converges_toward_common_listener_direction() {
        let source = [1.0, 20.0, 0.5];
        let (left, right) = ear_relative_directions(source, 0.0875);
        let delta = ((left[0] - right[0]).powi(2)
            + (left[1] - right[1]).powi(2)
            + (left[2] - right[2]).powi(2))
        .sqrt();
        assert!(delta < 0.01, "far-field ear-direction delta={delta}");
    }
}

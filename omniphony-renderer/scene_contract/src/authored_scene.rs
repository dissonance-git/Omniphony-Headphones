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

/// One authored metadata block after a source adapter has converted its timing
/// to integer samples.
///
/// `interpolation_frames` is the prefix of this block over which an adjacent
/// previous block may interpolate into `position_m`. A regular continuous ADM
/// object block uses the whole block duration; jump-style metadata can use zero
/// or a shorter prefix. Adjacency and stable identity are still checked here so
/// callback boundaries cannot manufacture continuity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AuthoredObjectBlock {
    pub stable_id: u64,
    pub span: SampleSpan,
    pub position_m: MetricPosition,
    pub interpolation_frames: u32,
}

impl AuthoredObjectBlock {
    pub fn new(
        stable_id: u64,
        span: SampleSpan,
        position_m: MetricPosition,
        interpolation_frames: u32,
    ) -> Result<Self, &'static str> {
        if span.frame_count == 0 {
            return Err("authored object block must contain at least one frame");
        }
        if interpolation_frames > span.frame_count {
            return Err("authored object interpolation exceeds block duration");
        }
        if !finite_position(position_m) {
            return Err("authored object block position must be finite");
        }
        Ok(Self {
            stable_id,
            span,
            position_m,
            interpolation_frames,
        })
    }

    /// Plan this block from source timing alone.
    ///
    /// Interpolation is legal only when the previous block belongs to the same
    /// stable object and ends exactly where this block starts. A gap, identity
    /// replacement, first block, or zero interpolation length produces a fixed
    /// block immediately.
    pub fn plan_from(
        self,
        previous: Option<Self>,
    ) -> AuthoredObjectBlockPlan {
        let previous = previous.filter(|previous| {
            previous.stable_id == self.stable_id
                && previous.span.end_sample_exclusive() == self.span.start_sample
        });

        let interpolation = previous.and_then(|previous| {
            (self.interpolation_frames > 0).then_some(AuthoredObjectInterpolation {
                stable_id: self.stable_id,
                span: SampleSpan::new(self.span.start_sample, self.interpolation_frames),
                start_position_m: previous.position_m,
                end_position_m: self.position_m,
            })
        });

        let fixed_frames = if interpolation.is_some() {
            self.span.frame_count - self.interpolation_frames
        } else {
            self.span.frame_count
        };
        let fixed_span = (fixed_frames > 0).then(|| {
            let offset = if interpolation.is_some() {
                self.interpolation_frames
            } else {
                0
            };
            SampleSpan::new(
                self.span.start_sample.saturating_add(offset as u64),
                fixed_frames,
            )
        });

        AuthoredObjectBlockPlan {
            stable_id: self.stable_id,
            interpolation,
            fixed_span,
            fixed_position_m: self.position_m,
        }
    }
}

/// Half-open block interpolation.
///
/// Unlike `AuthoredObjectMotion`, which is endpoint-inclusive over a finite
/// sample set, this type models metadata processors whose target is exact at the
/// *end boundary* of `[start, end)`. Therefore the last affected sample is
/// still one sample-step short of the target when the span has integer bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AuthoredObjectInterpolation {
    pub stable_id: u64,
    pub span: SampleSpan,
    pub start_position_m: MetricPosition,
    pub end_position_m: MetricPosition,
}

impl AuthoredObjectInterpolation {
    pub fn position_at_sample(self, sample: u64) -> MetricPosition {
        let last = self.span.end_sample_exclusive().saturating_sub(1);
        self.position_at_boundary(sample.clamp(self.span.start_sample, last))
    }

    pub fn position_at_boundary(self, sample_boundary: u64) -> MetricPosition {
        let end = self.span.end_sample_exclusive();
        let clamped = sample_boundary.clamp(self.span.start_sample, end);
        let t = if self.span.frame_count == 0 {
            1.0
        } else {
            (clamped - self.span.start_sample) as f64 / self.span.frame_count as f64
        };
        [
            lerp(self.start_position_m[0], self.end_position_m[0], t),
            lerp(self.start_position_m[1], self.end_position_m[1], t),
            lerp(self.start_position_m[2], self.end_position_m[2], t),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AuthoredObjectBlockPlan {
    pub stable_id: u64,
    pub interpolation: Option<AuthoredObjectInterpolation>,
    pub fixed_span: Option<SampleSpan>,
    pub fixed_position_m: MetricPosition,
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
    fn adjacent_authored_blocks_follow_half_open_reference_timing() {
        let first = AuthoredObjectBlock::new(
            7,
            SampleSpan::new(0, 48_000),
            [0.0, 1.0, 0.0],
            48_000,
        )
        .unwrap();
        let first_plan = first.plan_from(None);
        assert!(first_plan.interpolation.is_none());
        assert_eq!(first_plan.fixed_span, Some(first.span));

        let second = AuthoredObjectBlock::new(
            7,
            SampleSpan::new(48_000, 48_000),
            [0.0, 2.0, 0.0],
            48_000,
        )
        .unwrap();
        let second_plan = second.plan_from(Some(first));
        let interpolation = second_plan.interpolation.expect("adjacent block ramp");
        assert_eq!(interpolation.span, second.span);
        assert!(second_plan.fixed_span.is_none());
        assert_eq!(
            interpolation.position_at_sample(48_000),
            first.position_m
        );

        let last = interpolation.position_at_sample(95_999);
        assert!(last[1] < 2.0);
        assert!((last[1] - (1.0 + 47_999.0 / 48_000.0)).abs() < 1.0e-12);
        assert_eq!(
            interpolation.position_at_boundary(96_000),
            second.position_m
        );
    }

    #[test]
    fn jump_prefix_then_hold_matches_reference_block_semantics() {
        let previous = AuthoredObjectBlock::new(
            11,
            SampleSpan::new(96_000, 48_000),
            [0.0, 3.0, 0.0],
            0,
        )
        .unwrap();
        let block = AuthoredObjectBlock::new(
            11,
            SampleSpan::new(144_000, 48_000),
            [0.0, 4.0, 0.0],
            24_000,
        )
        .unwrap();

        let plan = block.plan_from(Some(previous));
        let interpolation = plan.interpolation.expect("jump interpolation prefix");
        assert_eq!(interpolation.span, SampleSpan::new(144_000, 24_000));
        assert_eq!(
            interpolation.position_at_boundary(168_000),
            block.position_m
        );
        assert_eq!(plan.fixed_span, Some(SampleSpan::new(168_000, 24_000)));
        assert_eq!(plan.fixed_position_m, block.position_m);
    }

    #[test]
    fn gaps_identity_changes_and_zero_length_jumps_do_not_interpolate() {
        let previous = AuthoredObjectBlock::new(
            21,
            SampleSpan::new(0, 48_000),
            [0.0, 1.0, 0.0],
            48_000,
        )
        .unwrap();

        let gap = AuthoredObjectBlock::new(
            21,
            SampleSpan::new(96_000, 48_000),
            [0.0, 2.0, 0.0],
            48_000,
        )
        .unwrap();
        assert!(gap.plan_from(Some(previous)).interpolation.is_none());

        let replacement = AuthoredObjectBlock::new(
            22,
            SampleSpan::new(48_000, 48_000),
            [0.0, 2.0, 0.0],
            48_000,
        )
        .unwrap();
        assert!(replacement.plan_from(Some(previous)).interpolation.is_none());

        let jump = AuthoredObjectBlock::new(
            21,
            SampleSpan::new(48_000, 48_000),
            [0.0, 3.0, 0.0],
            0,
        )
        .unwrap();
        let jump_plan = jump.plan_from(Some(previous));
        assert!(jump_plan.interpolation.is_none());
        assert_eq!(jump_plan.fixed_span, Some(jump.span));
    }

    #[test]
    fn authored_block_rejects_interpolation_beyond_duration() {
        assert!(
            AuthoredObjectBlock::new(
                1,
                SampleSpan::new(0, 16),
                [0.0, 1.0, 0.0],
                17,
            )
            .is_err()
        );
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

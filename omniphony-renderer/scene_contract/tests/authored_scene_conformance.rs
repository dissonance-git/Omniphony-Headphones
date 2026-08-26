use scene_contract::authored_scene::{
    AuthoredObjectBlock, MetricPosition, SampleSpan, ear_distances_m, radial_distance_m,
};

fn approx(left: f64, right: f64) {
    assert!((left - right).abs() < 1.0e-10, "{left} != {right}");
}

fn approx_position(left: MetricPosition, right: MetricPosition) {
    for axis in 0..3 {
        approx(left[axis], right[axis]);
    }
}

#[test]
fn continuous_object_motion_is_invariant_to_callback_partitioning() {
    let first = AuthoredObjectBlock::new(
        0xA11CE,
        SampleSpan::new(0, 48_000),
        [-1.0, 1.0, 0.25],
        48_000,
    )
    .unwrap();
    let second = AuthoredObjectBlock::new(
        0xA11CE,
        SampleSpan::new(48_000, 48_000),
        [1.0, 3.0, -0.25],
        48_000,
    )
    .unwrap();
    let interpolation = second
        .plan_from(Some(first))
        .interpolation
        .expect("continuous adjacent block");

    // Host callback cuts are deliberately irregular. The canonical authored
    // position at each boundary must depend only on sample time, never on the
    // callback sizes used to reach that boundary.
    let callback_boundaries = [48_000_u64, 48_127, 48_640, 50_113, 57_777, 72_001, 95_999, 96_000];
    for boundary in callback_boundaries {
        let t = (boundary - 48_000) as f64 / 48_000.0;
        let expected = [
            -1.0 + 2.0 * t,
            1.0 + 2.0 * t,
            0.25 - 0.5 * t,
        ];
        approx_position(interpolation.position_at_boundary(boundary), expected);
    }
}

#[test]
fn discontinuity_gap_and_identity_replacement_never_invent_motion() {
    let previous = AuthoredObjectBlock::new(
        7,
        SampleSpan::new(0, 480),
        [0.0, 1.0, 0.0],
        480,
    )
    .unwrap();

    let jump = AuthoredObjectBlock::new(
        7,
        SampleSpan::new(480, 480),
        [1.0, 1.0, 0.0],
        0,
    )
    .unwrap();
    assert!(jump.plan_from(Some(previous)).interpolation.is_none());

    let gap = AuthoredObjectBlock::new(
        7,
        SampleSpan::new(1_440, 480),
        [2.0, 1.0, 0.0],
        480,
    )
    .unwrap();
    assert!(gap.plan_from(Some(previous)).interpolation.is_none());

    let replacement = AuthoredObjectBlock::new(
        8,
        SampleSpan::new(480, 480),
        [3.0, 1.0, 0.0],
        480,
    )
    .unwrap();
    assert!(replacement.plan_from(Some(previous)).interpolation.is_none());
}

#[test]
fn authored_radius_is_geometry_not_a_direction_normalization_artifact() {
    let near = [0.3, 0.4, 0.0];
    let far = [3.0, 4.0, 0.0];

    approx(radial_distance_m(near), 0.5);
    approx(radial_distance_m(far), 5.0);

    let near_direction = [near[0] / 0.5, near[1] / 0.5, 0.0];
    let far_direction = [far[0] / 5.0, far[1] / 5.0, 0.0];
    approx_position(near_direction, far_direction);

    // Same direction, ten-times different authored radius. Any adapter that
    // normalizes both positions onto one shell has destroyed source truth.
    assert!(radial_distance_m(far) > radial_distance_m(near) * 9.9);
}

#[test]
fn near_field_geometry_converges_to_far_field_without_a_mode_boundary() {
    let head_radius_m = 0.0875;

    let near = [0.15, 0.25, 0.0];
    let (near_left, near_right) = ear_distances_m(near, head_radius_m);
    assert!((near_left - near_right).abs() > 0.05);

    let far = [3.0, 20.0, 0.0];
    let (far_left, far_right) = ear_distances_m(far, head_radius_m);
    let relative_far_difference = (far_left - far_right).abs() / ((far_left + far_right) * 0.5);
    assert!(relative_far_difference < 0.002);
}

#[test]
fn jump_interpolation_prefix_is_half_open_then_holds_exact_target() {
    let previous = AuthoredObjectBlock::new(
        99,
        SampleSpan::new(0, 1_000),
        [0.0, 1.0, 0.0],
        0,
    )
    .unwrap();
    let current = AuthoredObjectBlock::new(
        99,
        SampleSpan::new(1_000, 1_000),
        [0.0, 2.0, 0.0],
        250,
    )
    .unwrap();

    let plan = current.plan_from(Some(previous));
    let interpolation = plan.interpolation.expect("250-frame interpolation prefix");
    assert_eq!(interpolation.span, SampleSpan::new(1_000, 250));
    assert_eq!(plan.fixed_span, Some(SampleSpan::new(1_250, 750)));

    let last_interpolated_sample = interpolation.position_at_sample(1_249);
    assert!(last_interpolated_sample[1] < 2.0);
    approx_position(interpolation.position_at_boundary(1_250), current.position_m);
    approx_position(plan.fixed_position_m, current.position_m);
}

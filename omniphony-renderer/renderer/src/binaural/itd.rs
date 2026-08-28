//! Interaural time difference (ITD) from source direction.
//!
//! Uses Woodworth's spherical-head approximation. The broadband ITD is applied
//! as a pure per-ear delay (via [`crate::delay_line::DelayLine`]) rather than
//! baked into the HRIR phase, so head-tracking can move it smoothly without
//! re-deriving filters.

/// Default effective head radius (m) — KEMAR-ish. Live-tunable via
/// `BinauralLiveParams::head_radius_m` for per-listener ITD fit.
pub const DEFAULT_HEAD_RADIUS_M: f32 = 0.0875;
/// Speed of sound (m/s).
const SPEED_OF_SOUND: f32 = 343.0;

/// Ray-traced path length from one finite point source to one ear on a rigid
/// spherical head.
///
/// The source is listener-relative metres in Omniphony axes (+X right, +Y
/// forward, +Z up). `ear_x_sign` is -1 for the left ear and +1 for the right.
/// A visible ear receives the straight source-to-ear path. An occluded ear uses
/// the tangent from the source to the sphere plus the shortest great-circle arc
/// from tangency to the ear, matching the finite-distance Woodworth geometry.
fn finite_source_path_length_m(
    source_m: [f64; 3],
    head_radius_m: f64,
    ear_x_sign: f64,
) -> Option<f64> {
    if !head_radius_m.is_finite()
        || head_radius_m <= 0.0
        || !source_m.iter().all(|value| value.is_finite())
    {
        return None;
    }
    let source_radius_m =
        (source_m[0] * source_m[0] + source_m[1] * source_m[1] + source_m[2] * source_m[2])
            .sqrt();
    if !source_radius_m.is_finite() || source_radius_m < head_radius_m {
        return None;
    }

    let cos_delta = (source_m[0] * ear_x_sign / source_radius_m).clamp(-1.0, 1.0);
    let delta = cos_delta.acos();
    let tangent_angle = (head_radius_m / source_radius_m).clamp(0.0, 1.0).acos();

    if delta <= tangent_angle {
        Some(
            (source_radius_m * source_radius_m
                + head_radius_m * head_radius_m
                - 2.0 * source_radius_m * head_radius_m * cos_delta)
                .max(0.0)
                .sqrt(),
        )
    } else {
        let tangent_m =
            (source_radius_m * source_radius_m - head_radius_m * head_radius_m)
                .max(0.0)
                .sqrt();
        Some(tangent_m + head_radius_m * (delta - tangent_angle))
    }
}

/// Per-ear relative delays for a finite point source around a rigid spherical
/// head, using the finite-distance Woodworth ray geometry.
///
/// Unlike `ear_delays_seconds`, this consumes actual listener-relative metric
/// XYZ. It therefore captures source-distance dependence of the path to each
/// ear. Only the interaural path difference is returned: the nearer ear remains
/// at zero, so source distance does not add blanket transport latency.
///
/// Returns `None` for invalid geometry or a source inside the spherical head;
/// callers can then retain the ordinary direction-only model.
pub fn ear_delays_seconds_finite_source(
    source_m: [f64; 3],
    head_radius_m: f32,
) -> Option<(f32, f32)> {
    let radius = head_radius_m as f64;
    let left_path = finite_source_path_length_m(source_m, radius, -1.0)?;
    let right_path = finite_source_path_length_m(source_m, radius, 1.0)?;
    let delta_s = (left_path - right_path) / SPEED_OF_SOUND as f64;
    if !delta_s.is_finite() {
        return None;
    }
    if delta_s >= 0.0 {
        Some((delta_s as f32, 0.0))
    } else {
        Some((0.0, (-delta_s) as f32))
    }
}

/// Per-ear delays in seconds for a source at the given azimuth/elevation.
///
/// `azimuth_rad`: 0 = front, positive = source to the **right**.
/// `elevation_rad`: 0 = horizontal; the ITD shrinks toward the poles by
/// `cos(elevation)`.
///
/// Returns `(left_delay_s, right_delay_s)`, both ≥ 0: the ear nearer the source
/// gets 0, the far (contralateral) ear gets the positive ITD. A source on the
/// right therefore delays the **left** ear.
pub fn ear_delays_seconds(azimuth_rad: f32, elevation_rad: f32, head_radius_m: f32) -> (f32, f32) {
    // Woodworth: Δt = (r/c)(θ + sinθ) for the far ear, with θ measured from the
    // median plane and clamped to ±90° (rear hemisphere mirrors the front).
    let mut theta = azimuth_rad.rem_euclid(std::f32::consts::TAU);
    if theta > std::f32::consts::PI {
        theta -= std::f32::consts::TAU;
    }
    // Fold the rear hemisphere onto [-90°, 90°] (front/back ITD is ~symmetric).
    let folded = if theta > std::f32::consts::FRAC_PI_2 {
        std::f32::consts::PI - theta
    } else if theta < -std::f32::consts::FRAC_PI_2 {
        -std::f32::consts::PI - theta
    } else {
        theta
    };
    let mag = (head_radius_m / SPEED_OF_SOUND)
        * (folded.abs() + folded.abs().sin())
        * elevation_rad.cos().abs();
    if folded >= 0.0 {
        // Source on the right → left ear is the far ear.
        (mag, 0.0)
    } else {
        (0.0, mag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_source_has_zero_itd() {
        let (l, r) = ear_delays_seconds(0.0, 0.0, DEFAULT_HEAD_RADIUS_M);
        assert!(l.abs() < 1e-9 && r.abs() < 1e-9);
    }

    #[test]
    fn right_source_delays_left_ear() {
        let (l, r) = ear_delays_seconds(std::f32::consts::FRAC_PI_2, 0.0, DEFAULT_HEAD_RADIUS_M);
        assert!(l > r);
        assert!(r.abs() < 1e-9);
        // Max ITD for a 0.0875 m head ≈ 0.66 ms.
        assert!((l - 0.00066).abs() < 0.0002, "itd={l}");
    }

    #[test]
    fn left_source_delays_right_ear() {
        let (l, r) = ear_delays_seconds(-std::f32::consts::FRAC_PI_2, 0.0, DEFAULT_HEAD_RADIUS_M);
        assert!(r > l);
        assert!(l.abs() < 1e-9);
    }

    #[test]
    fn elevated_source_has_smaller_itd_than_horizontal() {
        let (l_horiz, _) =
            ear_delays_seconds(std::f32::consts::FRAC_PI_2, 0.0, DEFAULT_HEAD_RADIUS_M);
        let (l_high, _) = ear_delays_seconds(
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::FRAC_PI_4,
            DEFAULT_HEAD_RADIUS_M,
        );
        assert!(l_high < l_horiz && l_high > 0.0);
    }

    #[test]
    fn finite_source_converges_to_plane_wave_woodworth_at_long_range() {
        let radius_m = 1_000.0f64;
        for az_deg in [0.0f32, 30.0, 60.0, 90.0, 120.0, 150.0, 180.0] {
            let az = az_deg.to_radians();
            let source = [
                radius_m * az.sin() as f64,
                radius_m * az.cos() as f64,
                0.0,
            ];
            let finite =
                ear_delays_seconds_finite_source(source, DEFAULT_HEAD_RADIUS_M).unwrap();
            let plane = ear_delays_seconds(az, 0.0, DEFAULT_HEAD_RADIUS_M);
            assert!(
                (finite.0 - plane.0).abs() < 2.0e-8
                    && (finite.1 - plane.1).abs() < 2.0e-8,
                "finite source failed to converge at az={az_deg}: finite={finite:?} plane={plane:?}"
            );
        }
    }

    #[test]
    fn finite_source_increases_lateral_itd_within_reach() {
        let finite =
            ear_delays_seconds_finite_source([0.15, 0.0, 0.0], DEFAULT_HEAD_RADIUS_M).unwrap();
        let plane = ear_delays_seconds(
            std::f32::consts::FRAC_PI_2,
            0.0,
            DEFAULT_HEAD_RADIUS_M,
        );
        assert_eq!(
            finite.1, 0.0,
            "right-near source must leave the right ear undelayed"
        );
        assert!(
            finite.0 > plane.0 + 50.0e-6,
            "finite-distance lateral ITD should exceed its plane-wave limit nearby: finite={finite:?} plane={plane:?}"
        );
    }

    #[test]
    fn finite_source_keeps_the_median_plane_symmetric_in_3d() {
        let (left, right) =
            ear_delays_seconds_finite_source([0.0, 0.15, 0.20], DEFAULT_HEAD_RADIUS_M).unwrap();
        assert!(left.abs() < 1.0e-9 && right.abs() < 1.0e-9);
    }

    #[test]
    fn finite_source_rejects_positions_inside_the_head() {
        assert!(
            ear_delays_seconds_finite_source([0.01, 0.0, 0.0], DEFAULT_HEAD_RADIUS_M).is_none()
        );
    }
}

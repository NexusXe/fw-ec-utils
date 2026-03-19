use super::curve_lut_gen::generate_fan_curve_lut_dyn;
use super::*;

fn const_lut_as_vec<const N: usize, const LUT_SIZE: usize>(points: &[(u8, u8); N]) -> Vec<u8> {
    generate_fan_curve_lut::<N, LUT_SIZE>(points).to_vec()
}

macro_rules! parity_test {
    ($test_name:ident, $profile:expr, $points:expr) => {
        #[test]
        fn $test_name() {
            let runtime = generate_fan_curve_lut_dyn(&$points).expect(&format!(
                "Failed to generate rumtime LUT for profile '{}'",
                $profile.name
            ));
            assert_eq!(
                $profile.lut.as_ref(),
                runtime.as_slice(),
                "Parity failure for profile '{}'",
                $profile.name
            );
        }
    };
}

parity_test!(
    parity_default,
    DEFAULT_PROFILE,
    [(25, 10), (30, 15), (45, 30), (60, 50), (75, 80), (85, 100)]
);
parity_test!(
    parity_quiet,
    QUIET_PROFILE,
    [
        (30, 10),
        (35, 15),
        (50, 25),
        (65, 40),
        (80, 60),
        (85, 80),
        (90, 90),
        (92, 100)
    ]
);
parity_test!(
    parity_performance,
    PERFORMANCE_PROFILE,
    [(25, 15), (35, 30), (50, 50), (65, 75), (80, 100)]
);
parity_test!(
    parity_turbo,
    TURBO_PROFILE,
    [(25, 25), (30, 35), (45, 50), (60, 75), (70, 100)]
);
parity_test!(
    parity_deaf,
    DEAF_PROFILE,
    [(25, 35), (30, 40), (45, 50), (60, 75), (65, 100)]
);

/// Helper to generate a LUT at runtime or panic if it fails
#[allow(clippy::expect_used)]
#[inline]
fn runtime_lut_or_panic(points: &[(u8, u8)]) -> Vec<u8> {
    generate_fan_curve_lut_dyn(points).expect("Failed to generate rumtime LUT for test")
}

#[test]
fn output_values_in_range() {
    for profile in BUILTIN_PROFILES {
        for &v in profile.lut.iter() {
            assert!(
                v <= 100,
                "Profile '{}' has out-of-range value {v}",
                profile.name
            );
        }
    }
    // Also check the runtime path independently on a fresh curve
    let lut = runtime_lut_or_panic(&[(10, 0), (50, 100)]);
    assert!(lut.iter().all(|&v| v <= 100));
}

#[test]
fn knot_values_are_exact() {
    // Raw points; the EC offset shifts X but not the LUT index origin.
    // LUT index 0 = first knot, index 20 = second knot, index 40 = third.
    let points: &[(u8, u8)] = &[(10, 0), (30, 50), (50, 100)];
    let lut = runtime_lut_or_panic(points);

    assert_eq!(lut[0], 0, "First knot Y should be 0");
    assert_eq!(lut[20], 50, "Middle knot Y should be 50");
    assert_eq!(lut[40], 100, "Last knot Y should be 100");
}

#[test]
fn two_point_linear_matches_const() {
    const POINTS: [(u8, u8); 2] = [(0, 0), (100, 100)];
    const LUT_SIZE: usize = 101;

    let const_lut = const_lut_as_vec::<2, LUT_SIZE>(&POINTS);
    let runtime_lut = runtime_lut_or_panic(&POINTS);

    assert_eq!(const_lut, runtime_lut, "Two-point linear parity failed");

    // Verify the line is exact: value at index i should equal i
    for (i, &v) in runtime_lut.iter().enumerate() {
        assert_eq!(
            v, i as u8,
            "Two-point linear: index {i} should equal {i}, got {v}"
        );
    }
}

#[test]
fn flat_plateau_stays_flat() {
    let points: &[(u8, u8)] = &[(0, 20), (20, 50), (40, 50), (60, 100)];
    let lut = runtime_lut_or_panic(points);

    // Indices 20..=40 correspond to raw-X 20..=40, which is the flat region
    for (idx, &v) in lut[20..=40].iter().enumerate() {
        assert_eq!(
            v,
            50,
            "Flat plateau broken at index {}: expected 50, got {v}",
            idx + 20
        );
    }
    // The full curve must be non-decreasing
    for w in lut.windows(2) {
        assert!(
            w[1] >= w[0],
            "LUT not non-decreasing: {:?} → {:?}",
            w[0],
            w[1]
        );
    }
}

#[test]
fn non_uniform_spacing_matches_const() {
    const POINTS: [(u8, u8); 3] = [(0, 0), (10, 80), (90, 100)];
    const LUT_SIZE: usize = 91;

    let const_lut = const_lut_as_vec::<3, LUT_SIZE>(&POINTS);
    let runtime_lut = runtime_lut_or_panic(&POINTS);

    assert_eq!(const_lut, runtime_lut, "Non-uniform spacing parity failed");
}

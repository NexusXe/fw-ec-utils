#![allow(clippy::cast_sign_loss)]
#![allow(clippy::too_many_lines)]

use crate::temp::{CelsiusTemp, EC_TEMP_SENSOR_OFFSET_CELSIUS, EcTemp};

use std::borrow::Cow;
use std::convert::Into;

const fn get_pt<const N: usize>(points: &[(u8, u8); N], i: usize) -> (u8, u8) {
    let mut pt = points[i];
    pt.0 = pt.0.saturating_add(EC_TEMP_SENSOR_OFFSET_CELSIUS as u8);
    pt
}

pub type FanCurveFloat = f64;

const fn slope(p1: (u8, u8), p2: (u8, u8)) -> FanCurveFloat {
    if p2.0 == p1.0 {
        return 0.0; // Prevent divide by zero (should be blocked by validation)
    }
    let dy = FanCurveFloat::from(p2.1) - FanCurveFloat::from(p1.1);
    let dx = FanCurveFloat::from(p2.0) - FanCurveFloat::from(p1.0);
    dy / dx
}

/// Generates a perfectly smoothed, overshoot-free spline fan curve lookup table.
///
/// # Arguments
/// * `points` - An array of (`temp`, `fan_speed`) points defining the curve, in strictly
///   increasing temperature order. The first and last points are the saturation limits.
pub const fn generate_fan_curve_lut<const N: usize, const LUT_SIZE: usize>(
    points: &[(u8, u8); N],
) -> [u8; LUT_SIZE]
where
    [(); N]:,
{
    assert!(N >= 2, "At least two points (start and end) are required");

    // 1. Validate Inputs (using shifted values)
    let shifted_start = get_pt(points, 0);
    let shifted_end = get_pt(points, N - 1);

    assert!(shifted_start.1 <= 100, "Start Y must be <= 100");
    assert!(shifted_end.1 <= 100, "End Y must be <= 100");

    let mut i = 1;
    let mut last_x = shifted_start.0;
    while i < N {
        let pt = get_pt(points, i);
        assert!(pt.1 <= 100, "Intermediate Y must be <= 100");
        assert!(
            pt.0 > last_x,
            "Curve X coordinates must be strictly increasing"
        );
        last_x = pt.0;
        i += 1;
    }

    // 2. Compute Initial Tangents for Cubic Spline
    let mut tangents: [FanCurveFloat; N] = [0.0; N];

    tangents[0] = slope(get_pt(points, 0), get_pt(points, 1));
    tangents[N - 1] = slope(get_pt(points, N - 2), get_pt(points, N - 1));

    let mut i = 1;
    while i < N - 1 {
        let s1 = slope(get_pt(points, i - 1), get_pt(points, i));
        let s2 = slope(get_pt(points, i), get_pt(points, i + 1));
        tangents[i] = (s1 + s2) * 0.5; // Average contiguous slopes
        i += 1;
    }

    // 3. Apply Fritsch-Carlson Monotonicity Constraints
    let mut i = 0;
    while i < N - 1 {
        let p_i = get_pt(points, i);
        let p_next = get_pt(points, i + 1);
        let s = slope(p_i, p_next);
        let max_t = 3.0 * s.abs();

        if s == 0.0 {
            tangents[i] = 0.0;
            tangents[i + 1] = 0.0;
        } else {
            // Constrain left tangent
            if tangents[i].signum() != s.signum() {
                tangents[i] = 0.0;
            } else if tangents[i].abs() > max_t {
                tangents[i] = s.signum() * max_t;
            }
            // Constrain right tangent
            if tangents[i + 1].signum() != s.signum() {
                tangents[i + 1] = 0.0;
            } else if tangents[i + 1].abs() > max_t {
                tangents[i + 1] = s.signum() * max_t;
            }
        }
        i += 1;
    }

    assert!(
        LUT_SIZE == (points[N - 1].0 - points[0].0 + 1) as usize,
        "LUT_SIZE must match the X range"
    );
    let mut lut = [0u8; LUT_SIZE];
    let mut lut_idx = 0usize;

    let mut x_int = shifted_start.0 as usize;

    while x_int <= shifted_end.0 as usize {
        let x = x_int as u8;

        // Find the segment enclosing `x`
        let mut seg = 0;
        while seg < N - 1 {
            let p_seg = get_pt(points, seg);
            let p_next = get_pt(points, seg + 1);
            if x >= p_seg.0 && x <= p_next.0 {
                break;
            }
            seg += 1;
        }

        let p0 = get_pt(points, seg);
        let p1 = get_pt(points, seg + 1);

        if x == p0.0 {
            lut[lut_idx] = p0.1;
        } else if x == p1.0 {
            lut[lut_idx] = p1.1;
        } else {
            let m0 = tangents[seg];
            let m1 = tangents[seg + 1];
            let dx = FanCurveFloat::from(p1.0 - p0.0);

            // Relative position t in [0, 1]
            let t = (FanCurveFloat::from(x) - FanCurveFloat::from(p0.0)) / dx;
            let t2 = t * t;
            let t3 = t2 * t;

            // Cubic Hermite basis functions
            let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
            let h10 = t3 - 2.0 * t2 + t;
            let h01 = -2.0 * t3 + 3.0 * t2;
            let h11 = t3 - t2;

            let y0 = FanCurveFloat::from(p0.1);
            let y1 = FanCurveFloat::from(p1.1);

            // Evaluate the polynomial (tangents are scaled by dx to match Hermite convention)
            let y_f = h00 * y0 + h01 * y1 + h10 * m0 * dx + h11 * m1 * dx;

            // Round to nearest integer and clamp to 0..=100
            let y = y_f.round().clamp(0.0, 100.0) as u8;
            lut[lut_idx] = y;
        }

        x_int += 1;
        lut_idx += 1;
    }

    lut
}

/// Runtime version of [`generate_fan_curve_lut`] that accepts a dynamically-sized slice
/// and returns a heap-allocated `Vec<u8>`. Uses the same Fritsch-Carlson monotone cubic
/// Hermite spline algorithm.
///
/// # Panics
/// Panics under the same conditions as the `const fn` version (fewer than 2 points,
/// non-monotone X coordinates, Y values > 100).
pub fn generate_fan_curve_lut_dyn(points: &[(u8, u8)]) -> Vec<u8> {
    assert!(
        points.len() >= 2,
        "At least two points (start and end) are required"
    );

    // Shift all X values by the EC sensor offset up-front.
    let shifted: Vec<(u8, u8)> = points
        .iter()
        .map(|&(x, y)| (x.saturating_add(EC_TEMP_SENSOR_OFFSET_CELSIUS as u8), y))
        .collect();

    // Validate
    assert!(shifted.first().unwrap().1 <= 100, "Start Y must be <= 100");
    assert!(shifted.last().unwrap().1 <= 100, "End Y must be <= 100");
    assert!(
        shifted.windows(2).all(|w| w[1].0 > w[0].0),
        "Curve X coordinates must be strictly increasing"
    );
    assert!(
        shifted.iter().all(|p| p.1 <= 100),
        "Intermediate Y must be <= 100"
    );

    let n = shifted.len();

    // Compute initial tangents (Catmull-Rom style averages at interior points)
    let seg_slopes: Vec<FanCurveFloat> = shifted.windows(2).map(|w| slope(w[0], w[1])).collect();

    let mut tangents: Vec<FanCurveFloat> = (0..n)
        .map(|i| match i {
            0 => seg_slopes[0],
            k if k == n - 1 => seg_slopes[n - 2],
            k => (seg_slopes[k - 1] + seg_slopes[k]) * 0.5,
        })
        .collect();

    // Apply Fritsch-Carlson monotonicity constraints
    for (i, (&s, w)) in seg_slopes.iter().zip(shifted.windows(2)).enumerate() {
        let _ = w; // windows used only to pair; slopes were pre-computed
        let max_t = 3.0 * s.abs();
        if s == 0.0 {
            tangents[i] = 0.0;
            tangents[i + 1] = 0.0;
        } else {
            let clamp_t = |t: FanCurveFloat| {
                if t.signum() != s.signum() {
                    0.0
                } else if t.abs() > max_t {
                    s.signum() * max_t
                } else {
                    t
                }
            };
            tangents[i] = clamp_t(tangents[i]);
            tangents[i + 1] = clamp_t(tangents[i + 1]);
        }
    }

    // Build the LUT by evaluating the spline at every integer X in the range
    let x_start = shifted.first().unwrap().0;
    let x_end = shifted.last().unwrap().0;

    (x_start..=x_end)
        .map(|x| {
            // Find the segment enclosing `x`
            let seg = shifted
                .windows(2)
                .position(|w| x >= w[0].0 && x <= w[1].0)
                .unwrap_or(n - 2);

            let p0 = shifted[seg];
            let p1 = shifted[seg + 1];

            if x == p0.0 {
                p0.1
            } else if x == p1.0 {
                p1.1
            } else {
                let m0 = tangents[seg];
                let m1 = tangents[seg + 1];
                let dx = FanCurveFloat::from(p1.0 - p0.0);
                let t = (FanCurveFloat::from(x) - FanCurveFloat::from(p0.0)) / dx;
                let t2 = t * t;
                let t3 = t2 * t;
                let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
                let h10 = t3 - 2.0 * t2 + t;
                let h01 = -2.0 * t3 + 3.0 * t2;
                let h11 = t3 - t2;
                let y0 = FanCurveFloat::from(p0.1);
                let y1 = FanCurveFloat::from(p1.1);
                let y_f = h00 * y0 + h01 * y1 + h10 * m0 * dx + h11 * m1 * dx;
                y_f.round().clamp(0.0, 100.0) as u8
            }
        })
        .collect()
}

pub struct FanProfile {
    /// Human-readable curve name
    pub name: Cow<'static, str>,
    /// Start temperature (in EC temp units). Values below this will be clamped to the start value.
    pub start: u8,
    /// End temperature (in EC temp units). Values above this will be clamped to the end value.
    pub end: u8,
    /// Fan speed lookup table. The index is `temp - start`.
    pub lut: Cow<'static, [u8]>,
    /// XXH3 signature of defined points
    pub signature: u64,
}

impl FanProfile {
    pub fn get_fan_speed<T: Into<EcTemp>>(&self, temp: T) -> u8 {
        let temp: EcTemp = temp.into();
        let start: u8 = self.start;
        let end: u8 = self.end;
        let index = (temp.0.clamp(start, end) - start) as usize;
        self.lut[index]
    }
}

impl std::fmt::Display for FanProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Fan Curve: {}", self.name)?;
        writeln!(
            f,
            "Defined between {:+}°C and {:+}°C",
            Result::<CelsiusTemp, _>::from(EcTemp(self.start))
                .expect("Invalid start temperature")
                .0,
            Result::<CelsiusTemp, _>::from(EcTemp(self.end))
                .expect("Invalid end temperature")
                .0
        )?;
        writeln!(f, "Signature: 0x{:x}", self.signature)?;
        Ok(())
    }
}

macro_rules! define_profile {
    ($prof_ident:ident, $name_str:literal, $points:expr) => {
        pub const $prof_ident: FanProfile = FanProfile {
            name: Cow::Borrowed($name_str),
            start: $points[0].0 + EC_TEMP_SENSOR_OFFSET_CELSIUS as u8,
            end: $points[$points.len() - 1].0 + EC_TEMP_SENSOR_OFFSET_CELSIUS as u8,
            lut: Cow::Borrowed(&{
                const PTS: &[(u8, u8)] = &$points;
                const N: usize = PTS.len();
                const LUT_SIZE: usize = (PTS[N - 1].0 - PTS[0].0 + 1) as usize;
                const LUT: [u8; LUT_SIZE] = generate_fan_curve_lut(&{
                    // Re-express as a fixed-size array for the const fn
                    const fn to_array<const M: usize>(s: &[(u8, u8)]) -> [(u8, u8); M] {
                        let mut arr = [(0u8, 0u8); M];
                        let mut i = 0;
                        while i < M {
                            arr[i] = s[i];
                            i += 1;
                        }
                        arr
                    }
                    to_array::<N>(PTS)
                });
                LUT
            }),
            signature: {
                const PTS: &[(u8, u8)] = &$points;
                const LEN: usize = PTS.len() * 2;
                let ptr = PTS.as_ptr().cast::<u8>();
                let flat_pts: &[u8] = unsafe { std::slice::from_raw_parts(ptr, LEN) };
                xxhash_rust::const_xxh3::xxh3_64(flat_pts)
            },
        };
    };
}

define_profile!(
    FW_LAZIEST,
    "fw-laziest",
    [(45, 0), (65, 25), (70, 35), (75, 50), (85, 100)]
);

define_profile!(
    FW_LAZY,
    "fw-lazy",
    [(50, 15), (65, 25), (70, 35), (75, 50), (85, 100)]
);

define_profile!(
    FW_MEDIUM,
    "fw-medium",
    [(40, 15), (60, 30), (70, 40), (75, 80), (85, 100)]
);

define_profile!(FW_DEAF, "fw-deaf", [(0, 20), (40, 30), (50, 50), (60, 100)]);

define_profile!(FW_AEOLUS, "fw-aeolus", [(0, 20), (40, 50), (60, 100)]);

define_profile!(
    DEFAULT_PROFILE,
    "default",
    [(25, 10), (30, 15), (45, 30), (60, 50), (75, 80), (85, 100)]
);

define_profile!(
    QUIET_PROFILE,
    "quiet",
    [
        (30, 10),
        (35, 15),
        (50, 25),
        (65, 40),
        (80, 60),
        (85, 80),
        (90, 90),
        (92, 100),
    ]
);

define_profile!(
    PERFORMANCE_PROFILE,
    "performance",
    [(25, 15), (35, 30), (50, 50), (65, 75), (80, 100)]
);

define_profile!(
    TURBO_PROFILE,
    "turbo",
    [(25, 25), (30, 35), (45, 50), (60, 75), (70, 100)]
);

define_profile!(
    DEAF_PROFILE,
    "deaf",
    [(25, 35), (30, 40), (45, 50), (60, 75), (65, 100)]
);

pub const BUILTIN_PROFILES: &[FanProfile] = &[
    FW_LAZIEST,
    FW_LAZY,
    FW_MEDIUM,
    FW_DEAF,
    FW_AEOLUS,
    DEFAULT_PROFILE,
    QUIET_PROFILE,
    PERFORMANCE_PROFILE,
    TURBO_PROFILE,
    DEAF_PROFILE,
];

pub(crate) fn new_profile(name: &str, points: &[(u8, u8)]) -> FanProfile {
    let ptr = points.as_ptr().cast::<u8>();
    let flat_pts: &[u8] = unsafe { std::slice::from_raw_parts(ptr, points.len() * 2) };
    let signature = xxhash_rust::xxh3::xxh3_64(flat_pts);
    // Re-use the existing static LUT if this matches a built-in profile; otherwise generate one.
    let lut: Cow<'static, [u8]> = BUILTIN_PROFILES
        .iter()
        .find(|p| p.signature == signature)
        .map_or_else(
            || Cow::Owned(generate_fan_curve_lut_dyn(points)),
            |p| Cow::Borrowed(p.lut.as_ref()),
        );
    FanProfile {
        name: Cow::Owned(name.to_owned()),
        start: points[0].0 + EC_TEMP_SENSOR_OFFSET_CELSIUS as u8,
        end: points[points.len() - 1].0 + EC_TEMP_SENSOR_OFFSET_CELSIUS as u8,
        lut,
        signature,
    }
}

pub fn get_profile_by_name<'a>(
    name: &str,
    external_profiles: Option<&'a [FanProfile]>,
) -> Option<&'a FanProfile> {
    if let Some(profile) = BUILTIN_PROFILES.iter().find(|p| p.name == name) {
        Some(profile)
    } else {
        println!("[INFO]: Profile \"{name}\" not in built-in profiles.");
        if let Some(external_profiles) = external_profiles {
            println!("[INFO]: Checking external profiles for \"{name}\"...");
            if let Some(profile) = external_profiles.iter().find(|p| p.name == name) {
                Some(profile)
            } else {
                eprintln!("[WARN]: Profile \"{name}\" not in external profiles.");
                None
            }
        } else {
            eprintln!("[WARN]: No external profiles provided to check.");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn const_lut_as_vec<const N: usize, const LUT_SIZE: usize>(points: &[(u8, u8); N]) -> Vec<u8> {
        generate_fan_curve_lut::<N, LUT_SIZE>(points).to_vec()
    }

    macro_rules! parity_test {
        ($test_name:ident, $profile:expr, $points:expr) => {
            #[test]
            fn $test_name() {
                let runtime = generate_fan_curve_lut_dyn(&$points);
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
        parity_fw_laziest,
        FW_LAZIEST,
        [(45, 0), (65, 25), (70, 35), (75, 50), (85, 100)]
    );
    parity_test!(
        parity_fw_lazy,
        FW_LAZY,
        [(50, 15), (65, 25), (70, 35), (75, 50), (85, 100)]
    );
    parity_test!(
        parity_fw_medium,
        FW_MEDIUM,
        [(40, 15), (60, 30), (70, 40), (75, 80), (85, 100)]
    );
    parity_test!(
        parity_fw_deaf,
        FW_DEAF,
        [(0, 20), (40, 30), (50, 50), (60, 100)]
    );
    parity_test!(parity_fw_aeolus, FW_AEOLUS, [(0, 20), (40, 50), (60, 100)]);
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
        let lut = generate_fan_curve_lut_dyn(&[(10, 0), (50, 100)]);
        assert!(lut.iter().all(|&v| v <= 100));
    }

    #[test]
    fn knot_values_are_exact() {
        // Raw points; the EC offset shifts X but not the LUT index origin.
        // LUT index 0 = first knot, index 20 = second knot, index 40 = third.
        let points: &[(u8, u8)] = &[(10, 0), (30, 50), (50, 100)];
        let lut = generate_fan_curve_lut_dyn(points);

        assert_eq!(lut[0], 0, "First knot Y should be 0");
        assert_eq!(lut[20], 50, "Middle knot Y should be 50");
        assert_eq!(lut[40], 100, "Last knot Y should be 100");
    }

    #[test]
    fn two_point_linear_matches_const() {
        const POINTS: [(u8, u8); 2] = [(0, 0), (100, 100)];
        const LUT_SIZE: usize = 101;

        let const_lut = const_lut_as_vec::<2, LUT_SIZE>(&POINTS);
        let runtime_lut = generate_fan_curve_lut_dyn(&POINTS);

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
        let lut = generate_fan_curve_lut_dyn(points);

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
        let runtime_lut = generate_fan_curve_lut_dyn(&POINTS);

        assert_eq!(const_lut, runtime_lut, "Non-uniform spacing parity failed");
    }
}

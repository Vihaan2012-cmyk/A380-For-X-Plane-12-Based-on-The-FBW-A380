//! Roll control power: X-Plane's blade elements give the ailerons about a quarter of the rolling moment FlyByWire's lateral law inverts.

const WING_AREA_M2: f64 = 845.0;
const SEMISPAN_M: f64 = 39.9;
/// The law's plant (A380LateralNormalLaw.cpp `L_xi`): rolling moment q S s per radian of roll order.
const LAW_CL_PER_RAD: f64 = 1.0;
/// Span-weighted mean aileron per roll order below 220 kt: inner and mid 1.25, outer 0.5 (A380PrimComputerFctl.cpp).
const AILERON_PER_ORDER: f64 = 0.92;
/// X-Plane's own rolling moment per radian of mean aileron: the 1.0.3 step recordings and strip theory on the .acf ailerons.
const XPLANE_CL_PER_RAD: f64 = 0.25;
pub const EXTRA_CL_PER_RAD: f64 = LAW_CL_PER_RAD / AILERON_PER_ORDER - XPLANE_CL_PER_RAD;

/// Span-weighted mean of (left - right) / 2, trailing edge down degrees, over X-Plane's aileron sets.
pub fn mean_differential_deg(ailerons_deg: &[[f64; 2]; 2], spans: &[f64; 2]) -> f64 {
    let total: f64 = spans.iter().sum();
    (0..2).map(|k| spans[k] / total * (ailerons_deg[0][k] - ailerons_deg[1][k]) / 2.).sum()
}

/// The rolling moment to add, N m, positive right wing down.
pub fn extra_moment_nm(q_pa: f64, mean_differential_deg: f64) -> f64 {
    if !q_pa.is_finite() || !mean_differential_deg.is_finite() || disabled() {
        return 0.;
    }
    EXTRA_CL_PER_RAD * q_pa.max(0.) * WING_AREA_M2 * SEMISPAN_M * mean_differential_deg.to_radians()
}

fn disabled() -> bool {
    static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OFF.get_or_init(|| std::env::var("FBW_ROLL_POWER").is_ok_and(|v| v.trim() == "0"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_x_planes_own_share_the_roll_power_is_the_laws() {
        let q = 0.6125 * (160. * 0.5144f64).powi(2);
        let order_deg = 5.;
        let mean = AILERON_PER_ORDER * order_deg;
        let native = XPLANE_CL_PER_RAD * q * WING_AREA_M2 * SEMISPAN_M * mean.to_radians();
        let law = LAW_CL_PER_RAD * q * WING_AREA_M2 * SEMISPAN_M * order_deg.to_radians();
        let total = native + extra_moment_nm(q, mean);
        assert!((total / law - 1.).abs() < 1e-9, "{total} vs {law}");
        let accel = law / 9.0e7;
        let fbw_l_xi = q * 845. * 39.9 / 9.0e7 * order_deg.to_radians();
        assert!((accel - fbw_l_xi).abs() < 1e-12);
    }

    #[test]
    fn left_trailing_edge_down_rolls_right() {
        let a = [[4., 6.], [-4., -6.]];
        let d = mean_differential_deg(&a, &[1., 1.]);
        assert!((d - 5.).abs() < 1e-12);
        assert!(extra_moment_nm(4000., d) > 0.);
        assert_eq!(mean_differential_deg(&[[3., 3.], [3., 3.]], &[1., 2.]), 0., "symmetric droop adds nothing");
    }

    #[test]
    fn the_total_is_over_four_times_x_planes_own() {
        let ratio = (EXTRA_CL_PER_RAD + XPLANE_CL_PER_RAD) / XPLANE_CL_PER_RAD;
        assert!(ratio > 4.0 && ratio < 5.0, "{ratio}");
        assert_eq!(extra_moment_nm(f64::NAN, 3.), 0.);
        assert_eq!(extra_moment_nm(-5., 3.), 0.);
    }
}

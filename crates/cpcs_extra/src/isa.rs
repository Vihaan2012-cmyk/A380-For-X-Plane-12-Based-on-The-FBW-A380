pub const P0_HPA: f64 = 1013.25;
const T0_K: f64 = 288.15;
const LAPSE_K_PER_M: f64 = 0.0065;
const EXPONENT: f64 = 0.190263;
const M_TO_FT: f64 = 3.280_839_9;

pub fn altitude_ft(pressure_hpa: f64, reference_hpa: f64) -> f64 {
    if pressure_hpa <= 0.0 || reference_hpa <= 0.0 {
        return 0.0;
    }
    T0_K / LAPSE_K_PER_M * (1.0 - (pressure_hpa / reference_hpa).powf(EXPONENT)) * M_TO_FT
}

pub fn pressure_hpa(altitude_ft: f64, reference_hpa: f64) -> f64 {
    let h_m = altitude_ft / M_TO_FT;
    reference_hpa * (1.0 - LAPSE_K_PER_M * h_m / T0_K).max(0.0).powf(1.0 / EXPONENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sea_level_is_zero() {
        assert!(altitude_ft(P0_HPA, P0_HPA).abs() < 1e-9);
    }

    #[test]
    fn eight_thousand_feet_matches_the_standard_atmosphere() {
        let p = pressure_hpa(8000.0, P0_HPA);
        assert!((p - 752.6).abs() < 1.0, "{p}");
        assert!((altitude_ft(p, P0_HPA) - 8000.0).abs() < 0.01);
    }

    #[test]
    fn a_lower_reference_reads_a_higher_altitude_difference() {
        let high = altitude_ft(900.0, 1013.25);
        let relative = altitude_ft(900.0, 1000.0);
        assert!(relative < high);
    }

    #[test]
    fn invalid_pressures_read_zero() {
        assert_eq!(altitude_ft(0.0, P0_HPA), 0.0);
        assert_eq!(altitude_ft(900.0, -1.0), 0.0);
    }
}

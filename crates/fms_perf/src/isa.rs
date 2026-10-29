pub const ISA_SEA_LEVEL_HPA: f64 = 1013.25;
const FT_TO_M: f64 = 0.3048;

pub fn isa_pressure_hpa(alt_ft: f64) -> f64 {
    let h = alt_ft * FT_TO_M;
    if h <= 11_000.0 {
        ISA_SEA_LEVEL_HPA * (1.0 - 2.255_77e-5 * h).powf(5.255_88)
    } else {
        let p11 = ISA_SEA_LEVEL_HPA * (1.0 - 2.255_77e-5 * 11_000.0_f64).powf(5.255_88);
        p11 * (-(h - 11_000.0) / 6_341.62).exp()
    }
}

pub fn mach_to_kcas(mach: f64, static_pressure_hpa: f64) -> f64 {
    let qc = static_pressure_hpa * ((1.0 + 0.2 * mach * mach).powf(3.5) - 1.0);
    cas_from_impact(qc)
}

pub fn cas_from_impact(qc_hpa: f64) -> f64 {
    661.478_8 * (5.0 * ((qc_hpa / ISA_SEA_LEVEL_HPA + 1.0).powf(2.0 / 7.0) - 1.0)).max(0.0).sqrt()
}

pub fn kcas_to_mach(kcas: f64, static_pressure_hpa: f64) -> f64 {
    let qc = ISA_SEA_LEVEL_HPA * ((1.0 + 0.2 * (kcas / 661.478_8).powi(2)).powf(3.5) - 1.0);
    (5.0 * ((qc / static_pressure_hpa + 1.0).powf(2.0 / 7.0) - 1.0)).max(0.0).sqrt()
}

pub fn mach_to_kcas_at_altitude(mach: f64, alt_ft: f64) -> f64 {
    mach_to_kcas(mach, isa_pressure_hpa(alt_ft))
}

pub fn isa_temp_c(alt_ft: f64) -> f64 {
    alt_ft.min(36_089.0) * -0.001_981_2 + 15.0
}

pub fn isa_deviation_c(alt_ft: f64, sat_c: f64) -> f64 {
    sat_c - isa_temp_c(alt_ft)
}

pub fn pressure_alt_at_elevation(elev_ft: f64, qnh: f64) -> f64 {
    let p0 = if qnh < 500.0 { 29.92 } else { 1013.2 };
    elev_ft + 145_442.15 * (1.0 - (qnh / p0).powf(0.190_263))
}

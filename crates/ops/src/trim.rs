//! Take-off THS for a CG, from FlyByWire's
//! fbw-a380x/src/systems/shared/src/PitchTrimUtils.ts (`cgToPitchTrim`,
//! `pitchTrimToCg`, `pitchTrimOutOfRange`, `pitchTrimInCyanBand`).
//! Degrees, nose up positive.

pub const THS_MAX_DEG: f64 = 5.8;
pub const THS_MIN_DEG: f64 = -0.18;

pub fn ths_for_cg(cg_pct_mac: f64) -> f64 {
    if cg_pct_mac < 29.0 {
        THS_MAX_DEG
    } else if cg_pct_mac <= 34.5 {
        5.8 - ((cg_pct_mac - 29.0) * 56.0) / 275.0
    } else if cg_pct_mac <= 43.0 {
        4.68 - ((cg_pct_mac - 34.5) * 243.0) / 425.0
    } else {
        THS_MIN_DEG
    }
}

pub fn cg_for_ths(ths_deg: f64) -> f64 {
    if ths_deg > 4.68 {
        34.5 - ((ths_deg - 4.68) * 275.0) / 56.0
    } else {
        43.0 - ((ths_deg + 0.18) * 425.0) / 243.0
    }
}

pub fn ths_out_of_range(ths_deg: f64) -> bool {
    !(THS_MIN_DEG..=THS_MAX_DEG).contains(&ths_deg)
}

pub fn ths_in_cyan_band(cg_pct_mac: f64, ths_deg: f64) -> bool {
    (ths_for_cg(cg_pct_mac) - ths_deg).abs() < 1.5 && !ths_out_of_range(ths_deg)
}

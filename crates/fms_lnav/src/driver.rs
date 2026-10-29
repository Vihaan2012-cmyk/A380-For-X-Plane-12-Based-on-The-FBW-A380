use crate::common::{max_bank, G};

pub const TAU: f64 = 3.;
pub const ZETA: f64 = 0.8;
pub const G_KTS_H: f64 = G * 6997.84;
pub const T_HOURS: f64 = TAU / 3600.;
pub const K1: f64 = 180. / 4. / (std::f64::consts::PI * std::f64::consts::PI) / ZETA / T_HOURS;
pub const K2: f64 = ZETA / std::f64::consts::PI / G_KTS_H / T_HOURS;
pub const INTERCEPT_ANGLE: f64 = 45.;
pub const NAV_ACTIVE_CAPTURE_ZONE: f64 = 30.;
pub const MINIMUM_TRACK_ANGLE_ERROR: f64 = 1.;
pub const FORCED_TURN_TKAE_THRESHOLD: f64 = 150.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnState {
    Normal,
    ForceLeft,
    ForceRight,
}

pub fn within_sequencing_area(cross_track_error: f64, track_angle_error: f64) -> bool {
    cross_track_error.abs() < 7. && track_angle_error.abs() < 90.
}

pub fn forced_turn_command(state: TurnState, track_angle_error: f64, tas: f64) -> (TurnState, Option<f64>) {
    if state == TurnState::Normal {
        return (state, None);
    }
    let next = if track_angle_error.abs() < FORCED_TURN_TKAE_THRESHOLD { TurnState::Normal } else { state };
    let phi = if next == TurnState::ForceLeft { -max_bank(tas, true) } else { max_bank(tas, true) };
    (next, Some(phi))
}

pub struct CaptureInputs {
    pub tas: f64,
    pub leg_gs: f64,
    pub nominal_roll: f64,
    pub last_xte: f64,
    pub last_tae: f64,
    pub nav_engaged: bool,
}

pub fn nav_capture_condition(i: &CaptureInputs) -> bool {
    if i.nav_engaged {
        return i.last_xte.abs() < NAV_ACTIVE_CAPTURE_ZONE;
    }
    let bank = max_bank(i.tas, true);
    let turn_radius = i.tas.powi(2) / bank.to_radians().tan() / G_KTS_H;
    if i.last_tae * i.last_xte > 0. {
        return i.last_xte.abs() < (2. * turn_radius).abs();
    }
    let unsat = i.last_tae.clamp(-INTERCEPT_ANGLE, INTERCEPT_ANGLE);
    let saturated = i.last_tae.signum() * turn_radius * (unsat.to_radians().cos() - i.last_tae.to_radians().cos());
    let unsaturated = (i.nominal_roll / K2 - unsat * i.leg_gs) / K1;
    let optimal = (saturated - unsaturated).abs();
    let minimum = MINIMUM_TRACK_ANGLE_ERROR * i.leg_gs / K1;
    i.last_xte.abs() < minimum.max(optimal)
}

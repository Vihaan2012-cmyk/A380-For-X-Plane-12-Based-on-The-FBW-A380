use crate::atmosphere::Atmosphere;
use crate::profile::{AltitudeConstraint, AltitudeDescriptor, ProfilePhase, Profile};

#[derive(Clone, Copy, Debug)]
pub struct WaypointLeg {
    pub index: usize,
    pub cumulative_distance_with_transitions: f64,
    pub altitude_constraint: Option<AltitudeConstraint>,
    pub speed_constraint: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
pub struct WaypointPrediction {
    pub leg: usize,
    pub distance_from_start: f64,
    pub seconds_from_present: f64,
    pub altitude: f64,
    /// Knots, or Mach when the CAS is above the Mach's CAS (`casOrMach`).
    pub speed: f64,
    pub estimated_fuel_on_board: f64,
    pub distance_from_aircraft: f64,
    pub profile_phase: ProfilePhase,
    pub is_speed_constraint_met: bool,
    pub alt_error: f64,
}

/// `fix_between`: FBW measures a BETWEEN constraint undershoot against altitude 1 (the upper bound).
pub fn compute_alt_error(altitude: f64, constraint: Option<&AltitudeConstraint>, fix_between: bool) -> f64 {
    let Some(c) = constraint else { return 0.0 };
    use AltitudeDescriptor::*;
    match c.descriptor {
        AtAlt1 | AtAlt1GsIntcptAlt2 | AtAlt1AngleAlt2 => altitude - c.altitude1,
        AtOrAboveAlt1 | AtOrAboveAlt1GsIntcptAlt2 | AtOrAboveAlt1AngleAlt2 => (altitude - c.altitude1).min(0.0),
        AtOrBelowAlt1 | AtOrBelowAlt1AngleAlt2 => (altitude - c.altitude1).max(0.0),
        BetweenAlt1Alt2 => {
            if altitude >= c.altitude1 {
                altitude - c.altitude1
            } else if altitude <= c.altitude2 {
                altitude - if fix_between { c.altitude2 } else { c.altitude1 }
            } else {
                0.0
            }
        }
        AtOrAboveAlt2 => (altitude - c.altitude2).min(0.0),
        _ => 0.0,
    }
}

/// `NavGeometryProfile.computePredictionsAtWaypoints`; `legs` are the plan legs from `activeLegIndex - 1`
/// up to the first missed approach leg, discontinuities left out.
pub fn compute_predictions_at_waypoints(
    profile: &Profile,
    atm: &Atmosphere,
    legs: &[WaypointLeg],
    distance_to_present_position: f64,
    fix_between: bool,
) -> Vec<WaypointPrediction> {
    if profile.checkpoints.is_empty() {
        return Vec::new();
    }
    legs.iter()
        .map(|leg| {
            let d = leg.cumulative_distance_with_transitions;
            let p = profile.interpolate_everything_from_start(d, true);
            let speed = if atm.cas_or_mach_is_mach(p.speed, p.mach, p.altitude) { p.mach } else { p.speed };
            WaypointPrediction {
                leg: leg.index,
                distance_from_start: d,
                seconds_from_present: p.seconds_from_present,
                altitude: p.altitude,
                speed,
                estimated_fuel_on_board: p.remaining_fuel_on_board,
                distance_from_aircraft: d - distance_to_present_position,
                profile_phase: p.profile_phase,
                is_speed_constraint_met: leg.speed_constraint.is_none_or(|s| p.speed - s < 5.0),
                alt_error: compute_alt_error(p.altitude, leg.altitude_constraint.as_ref(), fix_between),
            }
        })
        .collect()
}

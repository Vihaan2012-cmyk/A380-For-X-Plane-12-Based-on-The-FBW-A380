// Translated from FlyByWire's fmgc guidance/vnav/VerticalProfileManager.ts (GPL-3.0-or-later): computeTacticalNdProfile,
// computeVerticalProfileForExpediteClimb, interceptNdWithGuidanceProfile, insertLevelSegmentPwp, insertNextDescentPwp,
// the strategy pickers for the active FCU modes and shouldShowTooSteepPathAhead.

use crate::climb::ClimbPathBuilder;
use crate::config::AircraftConfig;
use crate::constraints::ConstraintReader;
use crate::coordinator::calculate_intercept;
use crate::descent::{DescentStrategy, IdleDescentStrategy};
use crate::manager::{
    add_present_position_checkpoint, deceleration_schedule_from_descent_path, des_mode_strategy, interpolate_distance_at_altitude_backwards,
    managed_mach_target, tactical_to_guidance_offset, DescentProfileRelation, PresentState,
};
use crate::modes::{armed_vertical, is_armed, vertical, FcuModes};
use crate::profile::{Checkpoint, ComputationParameters, FlightPhase, Profile, Reason, Winds};
use crate::speed_profile::{ExpediteSpeedProfile, McduSpeedProfile, NdSpeedProfile, SpeedProfile};
use crate::strategy::{ClimbStrategy, ClimbThrustClimbStrategy, FlightPathAngleStrategy, VerticalSpeedStrategy};
use crate::tactical::{AsDescent, TacticalDescentPathBuilder};

pub struct NdInputs<'b, 'a> {
    pub modes: FcuModes,
    pub present: PresentState,
    pub relation: DescentProfileRelation,
    pub descent_profile: Option<&'b Profile<'a>>,
}

pub fn climb_strategy_for_vertical_mode<'a>(p: &'a ComputationParameters, config: &'a AircraftConfig, modes: &FcuModes) -> Box<dyn ClimbStrategy + 'a> {
    let atm = p.atmosphere();
    match modes.vertical {
        vertical::VS => Box::new(VerticalSpeedStrategy { params: p, atm, vertical_speed: modes.vertical_speed.max(0.0), config }),
        vertical::FPA => Box::new(FlightPathAngleStrategy { params: p, atm, flight_path_angle_deg: modes.flight_path_angle.max(0.0), config }),
        _ => Box::new(ClimbThrustClimbStrategy { params: p, atm, config }),
    }
}

pub fn descent_strategy_for_vertical_mode<'a>(
    p: &'a ComputationParameters,
    config: &'a AircraftConfig,
    modes: &FcuModes,
    relation: &DescentProfileRelation,
) -> Box<dyn DescentStrategy + 'a> {
    let atm = p.atmosphere();
    match modes.vertical {
        vertical::VS => Box::new(AsDescent(VerticalSpeedStrategy { params: p, atm, vertical_speed: modes.vertical_speed.min(0.0), config })),
        vertical::FPA => Box::new(AsDescent(FlightPathAngleStrategy { params: p, atm, flight_path_angle_deg: modes.flight_path_angle.min(0.0), config })),
        vertical::OP_DES => Box::new(IdleDescentStrategy::new(p, config)),
        _ if !relation.is_valid => Box::new(IdleDescentStrategy::new(p, config)),
        _ => Box::new(des_mode_strategy(p, config, relation)),
    }
}

/// The ND profile: present position to the FCU altitude in the active modes. Pass the flight plan winds when the lateral
/// mode is managed and FBW's `ConstantWindProfile` equivalent otherwise (a `SelectedGeometryProfile` has no constraints).
pub fn compute_tactical_nd_profile<'a>(
    p: &ComputationParameters,
    config: &'a AircraftConfig,
    reader: &ConstraintReader,
    winds: Box<dyn Winds + 'a>,
    i: &NdInputs,
) -> Profile<'a> {
    let modes = &i.modes;
    let nav = modes.is_lat_auto_control_active();
    let mut nd = Profile::new(config, winds);
    if nav {
        reader.apply_to_profile(&mut nd);
    }
    let distance = reader.distance_to_present_position();
    let mcdu;
    let ndsp;
    let expedite;
    let sp: &dyn SpeedProfile = if modes.is_expedite_mode_active() {
        expedite = ExpediteSpeedProfile { green_dot_speed: p.clean_speed };
        &expedite
    } else if modes.is_speed_auto_control_active() {
        mcdu = McduSpeedProfile::new(p, distance, reader.climb_speed_constraints.clone(), reader.descent_speed_constraints.clone());
        &mcdu
    } else {
        ndsp = NdSpeedProfile {
            params: p,
            aircraft_distance_along_track: distance,
            max_speed_constraints: reader.climb_speed_constraints.clone(),
            descent_speed_constraints: reader.descent_speed_constraints.clone(),
        };
        &ndsp
    };
    if modes.is_in_selected_vertical_mode() {
        nd.max_altitude_constraints.clear();
        nd.descent_altitude_constraints.clear();
    }
    add_present_position_checkpoint(&mut nd, p, managed_mach_target(p), i.present);

    if modes.is_in_takeoff_mode() || modes.is_in_climbing_mode() {
        let strategy = climb_strategy_for_vertical_mode(p, config, modes);
        ClimbPathBuilder::new(p).compute_climb_path(&mut nd, strategy.as_ref(), sp, p.fcu_altitude, modes.climb_hint());
    } else if modes.is_in_descent_mode() {
        let strategy = descent_strategy_for_vertical_mode(p, config, modes, &i.relation);
        let offset = tactical_to_guidance_offset(i.descent_profile, &i.relation, distance);
        let schedule = deceleration_schedule_from_descent_path(i.descent_profile, offset);
        TacticalDescentPathBuilder::new(p, config).build_tactical_descent_path_to_altitude(
            &mut nd,
            strategy.as_ref(),
            sp.should_take_descent_speed_limit(),
            p.fcu_altitude,
            &schedule,
        );
        intercept_nd_with_guidance_profile(&mut nd, p, modes, i.descent_profile, offset);
        insert_level_segment_pwp(&mut nd, p, modes, i.descent_profile.is_some());
        insert_next_descent_pwp(&mut nd, p, modes);
    }
    nd.finalize_profile();
    nd
}

fn in_descent_or_approach(p: &ComputationParameters) -> bool {
    p.flight_phase == FlightPhase::Descent || p.flight_phase == FlightPhase::Approach
}

fn guidance_checkpoint_for_nd(c: &Checkpoint, offset: f64) -> Checkpoint {
    Checkpoint {
        reason: if c.reason != Reason::Decel { c.reason } else { Reason::AtmosphericConditions },
        distance_from_start: c.distance_from_start + offset,
        ..*c
    }
}

pub fn intercept_nd_with_guidance_profile(nd: &mut Profile, p: &ComputationParameters, modes: &FcuModes, descent_profile: Option<&Profile>, offset: f64) {
    let Some(dp) = descent_profile else { return };
    if !modes.is_lat_auto_control_active() || !in_descent_or_approach(p) {
        return;
    }
    let intercept = calculate_intercept(&nd.checkpoints, &dp.checkpoints, offset);
    let des_active = modes.vertical == vertical::DES;
    if let Some((_, distance)) = intercept {
        let reason = if des_active { Reason::InterceptDescentProfileManaged } else { Reason::InterceptDescentProfileSelected };
        let k = nd.add_interpolated_checkpoint(distance, reason);
        if (p.present_altitude - nd.checkpoints[k].altitude).abs() < 100.0 {
            nd.checkpoints[k].reason = Reason::AtmosphericConditions;
        }
    }
    let des_armed = is_armed(modes.armed_vertical, armed_vertical::DES);
    match intercept {
        Some((index, distance)) if des_active || des_armed => {
            let tail: Vec<Checkpoint> = dp
                .checkpoints
                .iter()
                .map(|c| guidance_checkpoint_for_nd(c, offset))
                .filter(|c| c.distance_from_start > distance)
                .collect();
            nd.checkpoints.truncate((index + 2).min(nd.checkpoints.len()));
            nd.checkpoints.extend(tail);
            let level_off = interpolate_distance_at_altitude_backwards(&nd.checkpoints, p.fcu_altitude, true);
            nd.add_interpolated_checkpoint(level_off, Reason::CrossingFcuAltitudeDescent);
        }
        _ => {
            let last = nd.last().distance_from_start;
            nd.checkpoints.extend(dp.checkpoints.iter().map(|c| guidance_checkpoint_for_nd(c, offset)).filter(|c| c.distance_from_start > last));
        }
    }
}

pub fn insert_level_segment_pwp(nd: &mut Profile, p: &ComputationParameters, modes: &FcuModes, has_descent_profile: bool) {
    if !modes.is_lat_auto_control_active() || !has_descent_profile || !in_descent_or_approach(p) {
        return;
    }
    let current_alt = p.present_altitude;
    let descent_armed = is_armed(modes.armed_vertical, armed_vertical::DES) || is_armed(modes.armed_vertical, armed_vertical::FINAL);
    let level = modes.is_in_level_flight_mode();
    let mut constraint_alt = f64::NEG_INFINITY;
    for i in 1..nd.checkpoints.len() {
        if nd.checkpoints[i].reason == Reason::LevelOffForDescentConstraint {
            if current_alt - nd.checkpoints[i].altitude < 100.0 {
                nd.checkpoints[i].reason = Reason::AtmosphericConditions;
            } else {
                constraint_alt = (nd.checkpoints[i].altitude + 0.5).floor();
            }
        }
        let alt = nd.checkpoints[i].altitude;
        if (level && descent_armed && current_alt - alt > 100.0) || constraint_alt - alt > 100.0 {
            let d = nd.checkpoints[i - 1].distance_from_start;
            nd.add_interpolated_checkpoint(d, Reason::ContinueDescentArmed);
            break;
        }
    }
}

pub fn insert_next_descent_pwp(nd: &mut Profile, p: &ComputationParameters, modes: &FcuModes) {
    if !modes.is_lat_auto_control_active() || !in_descent_or_approach(p) {
        return;
    }
    let mut level_off_distance: f64 = 0.0;
    for i in 1..nd.checkpoints.len() {
        let c = nd.checkpoints[i];
        if c.reason == Reason::CrossingFcuAltitudeDescent {
            level_off_distance = c.distance_from_start;
        }
        if c.reason == Reason::ContinueDescentArmed {
            return;
        }
        if c.altitude - p.fcu_altitude >= -100.0 {
            continue;
        }
        let d = level_off_distance.max(nd.checkpoints[i - 1].distance_from_start);
        nd.add_interpolated_checkpoint(d, Reason::ContinueDescent);
        return;
    }
}

/// `computeVerticalProfileForExpediteClimb`. `None` when FBW leaves the previous expedite profile in place (no green dot speed).
pub fn compute_expedite_climb_profile<'a>(
    p: &ComputationParameters,
    config: &'a AircraftConfig,
    winds: Box<dyn Winds + 'a>,
    modes: &FcuModes,
    present: PresentState,
    green_dot_speed: f64,
) -> Option<Profile<'a>> {
    if green_dot_speed == 0.0 || green_dot_speed.is_nan() {
        return None;
    }
    let sp = ExpediteSpeedProfile { green_dot_speed };
    let mut profile = Profile::new(config, winds);
    let climb = ClimbThrustClimbStrategy { params: p, atm: p.atmosphere(), config };
    add_present_position_checkpoint(&mut profile, p, p.managed_climb_speed_mach, present);
    ClimbPathBuilder::new(p).compute_climb_path(&mut profile, &climb, &sp, p.fcu_altitude, modes.climb_hint());
    profile.finalize_profile();
    Some(profile)
}

/// `VerticalProfileManager.shouldShowTooSteepPathAhead`; `has_too_steep_path_ahead` is the flight plan's answer.
pub fn should_show_too_steep_path_ahead(p: &ComputationParameters, modes: &FcuModes, distance_to_end: Option<f64>, has_too_steep_path_ahead: bool) -> bool {
    let close = (distance_to_end.unwrap_or(f64::INFINITY) < 150.0 && p.flight_phase == FlightPhase::Cruise) || in_descent_or_approach(p);
    modes.is_lat_auto_control_active() && close && has_too_steep_path_ahead
}

// Translated from FlyByWire's fmgc guidance/vnav/VerticalProfileManager.ts (GPL-3.0-or-later): computeTacticalMcduPath,
// computeDescentPath, the tactical descent branch and its helpers. The ND and expedite profiles are in `nd.rs`.

use crate::approach::ApproachPathBuilder;
use crate::atmosphere::interpolate;
use crate::climb::ClimbPathBuilder;
use crate::config::AircraftConfig;
use crate::constraints::ConstraintReader;
use crate::coordinator::{calculate_intercept, CoordinatorError, CruiseToDescentCoordinator};
use crate::cruise::CruisePathBuilder;
use crate::descent::{DescentPathBuilder, DescentStrategy};
use crate::modes::FcuModes;
use crate::profile::{Checkpoint, ComputationParameters, FlightPhase, Profile, ProfilePhase, Reason, Winds};
use crate::speed_profile::McduSpeedProfile;
use crate::strategy::{ClimbThrustClimbStrategy, VerticalSpeedStrategy};
use crate::tactical::{DecelerationSchedule, DesModeStrategy, TacticalDescentPathBuilder};
use crate::takeoff::build_takeoff_path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum McduPathError {
    Coordinator(CoordinatorError),
}

pub struct McduPath<'a> {
    pub profile: Profile<'a>,
    pub too_steep_path_legs: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PresentState {
    pub indicated_airspeed: f64,
    pub vman: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DescentProfileRelation {
    pub is_valid: bool,
    pub distance_from_start: f64,
    pub past_top_of_descent: bool,
    pub linear_deviation: f64,
    pub on_geometric_path: bool,
    pub current_target_path_angle: f64,
    pub above_speed_limit_altitude: bool,
    pub close_to_airfield_elevation: bool,
}

#[derive(Default)]
pub struct TacticalInputs<'b, 'a> {
    pub modes: FcuModes,
    pub present: PresentState,
    pub relation: DescentProfileRelation,
    pub descent_profile: Option<&'b Profile<'a>>,
}

pub fn managed_mach_target(p: &ComputationParameters) -> f64 {
    match p.flight_phase {
        FlightPhase::Cruise => p.managed_cruise_speed_mach,
        FlightPhase::Descent | FlightPhase::Approach | FlightPhase::Done => p.managed_descent_speed_mach,
        _ => p.managed_climb_speed_mach,
    }
}

pub fn profile_phase_from_flight_phase(phase: FlightPhase) -> ProfilePhase {
    match phase {
        FlightPhase::Cruise => ProfilePhase::Cruise,
        FlightPhase::Descent | FlightPhase::Approach => ProfilePhase::Descent,
        _ => ProfilePhase::Climb,
    }
}

pub fn add_present_position_checkpoint(profile: &mut Profile, p: &ComputationParameters, mach: f64, state: PresentState) {
    profile.checkpoints.push(Checkpoint {
        reason: Reason::PresentPosition,
        distance_from_start: profile.distance_to_present_position,
        seconds_from_present: 0.0,
        altitude: p.present_altitude,
        remaining_fuel_on_board: p.fuel_on_board,
        speed: state.indicated_airspeed.max(state.vman),
        mach,
        profile_phase: profile_phase_from_flight_phase(p.flight_phase),
        target_speed: None,
    });
}

pub fn tactical_to_guidance_offset(descent_profile: Option<&Profile>, relation: &DescentProfileRelation, distance_to_present_position: f64) -> f64 {
    if descent_profile.is_none() || !relation.is_valid {
        return 0.0;
    }
    distance_to_present_position - relation.distance_from_start
}

pub fn deceleration_schedule_from_descent_path(descent_profile: Option<&Profile>, offset: f64) -> DecelerationSchedule {
    let mut schedule = DecelerationSchedule::default();
    let Some(dp) = descent_profile else { return schedule };
    for c in &dp.checkpoints {
        let shifted = Checkpoint { distance_from_start: c.distance_from_start + offset, ..*c };
        match c.reason {
            Reason::StartDecelerationToConstraint | Reason::StartDecelerationToLimit => schedule.speed_changes.push(shifted),
            Reason::Decel | Reason::Flaps1 | Reason::Flaps2 | Reason::Flaps3 | Reason::FlapsFull => schedule.approach_points.push(shifted),
            _ => {}
        }
    }
    schedule
}

pub fn des_mode_strategy<'a>(params: &'a ComputationParameters, config: &'a AircraftConfig, relation: &DescentProfileRelation) -> DesModeStrategy<'a> {
    if relation.linear_deviation > 0.0 && relation.past_top_of_descent {
        return DesModeStrategy::above_profile(params, config);
    }
    if relation.on_geometric_path {
        return DesModeStrategy::below_profile_fpa(params, config, relation.current_target_path_angle / 2.0);
    }
    let vs = if relation.above_speed_limit_altitude && !relation.close_to_airfield_elevation { -1000.0 } else { -500.0 };
    DesModeStrategy::below_profile_vs(params, config, vs)
}

pub fn interpolate_distance_at_altitude_backwards(checkpoints: &[Checkpoint], altitude: f64, snap_reverse: bool) -> f64 {
    let n = checkpoints.len();
    let key = |c: &Checkpoint| c.altitude.round();
    if altitude < key(&checkpoints[n - 1]) {
        return checkpoints[n - 1].distance_from_start;
    }
    for i in (0..n - 1).rev() {
        let (a, b) = (key(&checkpoints[i]), key(&checkpoints[i + 1]));
        if (!snap_reverse && altitude <= a && altitude > b) || (snap_reverse && altitude < a && altitude >= b) {
            return interpolate(altitude, a, b, checkpoints[i].distance_from_start, checkpoints[i + 1].distance_from_start);
        }
    }
    checkpoints[0].distance_from_start
}

pub fn merge_mcdu_with_guidance_profile(mcdu: &mut Profile, descent_profile: Option<&Profile>, flight_phase: FlightPhase, offset: f64) {
    let Some(dp) = descent_profile else { return };
    if flight_phase != FlightPhase::Descent && flight_phase != FlightPhase::Approach {
        return;
    }
    if let Some((index, intercept)) = calculate_intercept(&mcdu.checkpoints, &dp.checkpoints, offset) {
        let tactical = mcdu.interpolate_everything_from_start(intercept, false);
        let guidance = dp.interpolate_everything_from_start(intercept - offset, false);
        let fuel_offset = tactical.remaining_fuel_on_board - guidance.remaining_fuel_on_board;
        let time_offset = tactical.seconds_from_present - guidance.seconds_from_present;
        mcdu.checkpoints.truncate(index + 1);
        mcdu.checkpoints.extend(
            dp.checkpoints
                .iter()
                .map(|c| Checkpoint {
                    distance_from_start: c.distance_from_start + offset,
                    remaining_fuel_on_board: c.remaining_fuel_on_board + fuel_offset,
                    seconds_from_present: c.seconds_from_present + time_offset,
                    ..*c
                })
                .filter(|c| c.distance_from_start > intercept),
        );
    }
    if !mcdu.checkpoints.iter().any(|c| c.reason == Reason::Decel) {
        let decel = dp.checkpoints.iter().find(|c| c.reason == Reason::Decel).expect("guidance profile has a DECEL checkpoint");
        mcdu.add_interpolated_checkpoint(decel.distance_from_start + offset, Reason::Decel);
    }
    if !mcdu.checkpoints.iter().any(|c| c.reason == Reason::CrossingDescentSpeedLimit) {
        if let Some(lim) = dp.checkpoints.iter().find(|c| c.reason == Reason::CrossingDescentSpeedLimit) {
            let d = interpolate_distance_at_altitude_backwards(&mcdu.checkpoints, lim.altitude, false);
            mcdu.add_interpolated_checkpoint(d, Reason::CrossingDescentSpeedLimit);
        }
    }
}

pub fn compute_tactical_mcdu_path<'a>(
    params: &ComputationParameters,
    config: &'a AircraftConfig,
    reader: &ConstraintReader,
    winds: Box<dyn Winds + 'a>,
    coordinator: &mut CruiseToDescentCoordinator,
    estimated_destination_fuel: Option<f64>,
    tactical: &TacticalInputs,
) -> Result<McduPath<'a>, McduPathError> {
    let climb = ClimbThrustClimbStrategy { params, atm: params.atmosphere(), config };
    let step_descent = VerticalSpeedStrategy { params, atm: params.atmosphere(), vertical_speed: -1000.0, config };
    let mut profile = Profile::new(config, winds);
    reader.apply_to_profile(&mut profile);
    let sp = McduSpeedProfile::new(params, reader.distance_to_present_position(), reader.climb_speed_constraints.clone(), reader.descent_speed_constraints.clone());

    if params.flight_phase < FlightPhase::Climb {
        build_takeoff_path(&mut profile, params);
    } else {
        add_present_position_checkpoint(&mut profile, params, managed_mach_target(params), tactical.present);
    }
    if params.flight_phase < FlightPhase::Cruise {
        ClimbPathBuilder::new(params).compute_climb_path(&mut profile, &climb, &sp, params.cruise_altitude, tactical.modes.climb_hint());
    }

    let mut too_steep_path_legs = Vec::new();
    let rel = &tactical.relation;
    if rel.is_valid && ((params.flight_phase >= FlightPhase::Descent && params.flight_phase <= FlightPhase::Approach) || rel.past_top_of_descent) {
        let offset = tactical_to_guidance_offset(tactical.descent_profile, rel, reader.distance_to_present_position());
        let schedule = deceleration_schedule_from_descent_path(tactical.descent_profile, offset);
        let strategy = des_mode_strategy(params, config, rel);
        TacticalDescentPathBuilder::new(params, config).build_mcdu_prediction_path(
            &mut profile,
            &strategy as &dyn DescentStrategy,
            sp.should_take_descent_speed_limit(),
            &schedule,
        );
        merge_mcdu_with_guidance_profile(&mut profile, tactical.descent_profile, params.flight_phase, offset);
    } else {
        coordinator
            .build_cruise_and_descent_path(params, &mut profile, &sp, &climb, &step_descent, estimated_destination_fuel, &mut too_steep_path_legs)
            .map_err(McduPathError::Coordinator)?;
    }
    profile.finalize_profile();
    Ok(McduPath { profile, too_steep_path_legs })
}

pub fn compute_descent_path<'a>(
    params: &ComputationParameters,
    config: &'a AircraftConfig,
    reader: &ConstraintReader,
    winds: Box<dyn Winds + 'a>,
    estimated_destination_fuel: Option<f64>,
) -> McduPath<'a> {
    let mut profile = Profile::new(config, winds);
    reader.apply_to_profile(&mut profile);
    let sp = McduSpeedProfile::new(params, reader.distance_to_present_position(), reader.climb_speed_constraints.clone(), reader.descent_speed_constraints.clone());
    let fuel = match estimated_destination_fuel.filter(|f| f.is_finite()) {
        Some(f) => f.max(config.vnav.lowest_fuel_estimate).min(config.vnav.maximum_fuel_estimate),
        None => 4000.0,
    };
    let final_cruise_altitude = CruisePathBuilder::new(params).final_cruise_altitude(&profile.cruise_steps);
    let mut too_steep_path_legs = Vec::new();
    let mut sequence = ApproachPathBuilder::new(params, config).compute_approach_path(&profile, &sp, fuel, 0.0, &mut too_steep_path_legs);
    DescentPathBuilder::new(params, config).compute_managed_descent_path(
        &mut sequence,
        &profile,
        &sp,
        final_cruise_altitude,
        f64::NEG_INFINITY,
        &mut too_steep_path_legs,
    );
    profile.checkpoints.extend(sequence.checkpoints.iter().rev().copied());
    profile.finalize_profile();
    McduPath { profile, too_steep_path_legs }
}

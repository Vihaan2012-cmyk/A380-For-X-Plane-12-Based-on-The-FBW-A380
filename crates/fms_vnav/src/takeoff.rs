// Translated from FlyByWire's fmgc guidance/vnav/takeoff/TakeoffPathBuilder.ts (GPL-3.0-or-later).

use crate::atmosphere as atm;
use crate::config::FlapConf;
use crate::engine as eng;
use crate::predictions::{self as pred, StepEnv};
use crate::profile::{Checkpoint, ComputationParameters, Profile, ProfilePhase, Reason};

pub fn build_takeoff_path(profile: &mut Profile, p: &ComputationParameters) {
    add_takeoff_roll_checkpoint(profile, p);
    build_path_to_thrust_reduction_altitude(profile, p);
    build_path_to_acceleration_altitude(profile, p);
}

fn add_takeoff_roll_checkpoint(profile: &mut Profile, p: &ComputationParameters) {
    profile.checkpoints.push(Checkpoint {
        reason: Reason::Liftoff,
        distance_from_start: 0.0,
        seconds_from_present: 0.0,
        altitude: p.departure_elevation,
        remaining_fuel_on_board: p.fuel_on_board,
        speed: p.v2_speed + 10.0,
        mach: p.managed_climb_speed_mach,
        profile_phase: ProfilePhase::Climb,
        target_speed: None,
    });
}

fn build_path_to_thrust_reduction_altitude(profile: &mut Profile, p: &ComputationParameters) {
    let last = *profile.last();
    let starting_altitude = last.altitude;
    let midway = (starting_altitude + p.thrust_reduction_altitude) / 2.0;
    let predicted_n1 = p.takeoff_toga_n1;
    let speed = p.v2_speed + 10.0;

    let above = midway > p.tropopause;
    let delta = atm::delta(midway, above);
    let mach = atm::cas_to_mach(speed, delta);
    let theta = atm::theta(midway, p.isa_deviation, above);
    let theta2 = atm::theta2(theta, mach);
    let corrected_n1 = eng::corrected_n1(predicted_n1, theta2);

    let mut env = StepEnv::new(p.zero_fuel_weight, p.isa_deviation, p.tropopause, p.perf_factor);
    env.flaps = p.takeoff_flaps;
    let step = pred::altitude_step(
        profile.config,
        starting_altitude,
        p.thrust_reduction_altitude - starting_altitude,
        speed,
        p.managed_climb_speed_mach,
        corrected_n1,
        last.remaining_fuel_on_board,
        -profile.winds.climb_tailwind(last.distance_from_start, midway),
        &env,
    );

    profile.checkpoints.push(Checkpoint {
        reason: Reason::ThrustReductionAltitude,
        distance_from_start: last.distance_from_start + step.distance_traveled,
        seconds_from_present: last.seconds_from_present + step.time_elapsed,
        altitude: p.thrust_reduction_altitude,
        remaining_fuel_on_board: last.remaining_fuel_on_board - step.fuel_burned,
        speed,
        mach: p.managed_climb_speed_mach,
        profile_phase: ProfilePhase::Climb,
        target_speed: None,
    });
}

fn build_path_to_acceleration_altitude(profile: &mut Profile, p: &ComputationParameters) {
    let last = *profile.last();
    let speed = p.v2_speed + 10.0;
    let starting_altitude = last.altitude;
    let midway = (starting_altitude + p.acceleration_altitude) / 2.0;
    let sat = p.atmosphere().predict_sat(midway);
    let predicted_n1 = eng::climb_thrust_corrected_n1(&profile.config.engine, midway, sat);

    let mut env = StepEnv::new(p.zero_fuel_weight, p.isa_deviation, p.tropopause, p.perf_factor);
    env.flaps = FlapConf::Clean;
    let step = pred::altitude_step(
        profile.config,
        starting_altitude,
        p.acceleration_altitude - starting_altitude,
        speed,
        1.0,
        predicted_n1,
        last.remaining_fuel_on_board,
        -profile.winds.climb_tailwind(last.distance_from_start, midway),
        &env,
    );

    profile.checkpoints.push(Checkpoint {
        reason: Reason::AccelerationAltitude,
        distance_from_start: last.distance_from_start + step.distance_traveled,
        seconds_from_present: last.seconds_from_present + step.time_elapsed,
        altitude: p.acceleration_altitude,
        remaining_fuel_on_board: last.remaining_fuel_on_board - step.fuel_burned,
        speed,
        mach: p.managed_climb_speed_mach,
        profile_phase: ProfilePhase::Climb,
        target_speed: None,
    });
}

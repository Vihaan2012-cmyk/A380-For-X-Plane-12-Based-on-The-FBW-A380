// Translated from FlyByWire's fmgc guidance/vnav/cruise/CruisePathBuilder.ts (GPL-3.0-or-later).

use crate::engine as eng;
use crate::predictions::{self as pred, StepEnv, StepResults};
use crate::profile::{Checkpoint, ComputationParameters, GeographicCruiseStep, MaxSpeedConstraint, Profile, ProfilePhase, Reason};
use crate::strategy::{ClimbStrategy, ClimbThrustClimbStrategy, SurfaceConfig, VerticalSpeedStrategy};
use crate::speed_profile::{ManagedSpeedType, McduSpeedProfile};

pub struct CruisePathBuilder<'a> {
    pub params: &'a ComputationParameters,
}

impl<'a> CruisePathBuilder<'a> {
    pub fn new(params: &'a ComputationParameters) -> Self {
        Self { params }
    }

    pub fn compute_cruise_path(
        &self,
        profile: &mut Profile,
        start_of_cruise: Checkpoint,
        target_distance_from_start: f64,
        sp: &McduSpeedProfile,
    ) -> Vec<Checkpoint> {
        let p = self.params;
        let climb = ClimbThrustClimbStrategy { params: p, atm: p.atmosphere(), config: profile.config };
        let descent = VerticalSpeedStrategy { params: p, atm: p.atmosphere(), vertical_speed: -1000.0, config: profile.config };
        self.compute_cruise_path_with_steps(profile, start_of_cruise, target_distance_from_start, &climb, &descent, sp)
    }

    pub fn compute_cruise_path_with_steps(
        &self,
        profile: &mut Profile,
        start_of_cruise: Checkpoint,
        target_distance_from_start: f64,
        step_climb: &dyn ClimbStrategy,
        step_descent: &dyn ClimbStrategy,
        sp: &McduSpeedProfile,
    ) -> Vec<Checkpoint> {
        let mut seq = vec![start_of_cruise];
        let constraints = profile.max_climb_speed_constraints.clone();

        for i in 0..profile.cruise_steps.len() {
            let step = profile.cruise_steps[i];
            if step.is_ignored {
                continue;
            }
            if step.distance_from_start < start_of_cruise.distance_from_start || step.distance_from_start > target_distance_from_start {
                profile.ignore_cruise_step(i);
                continue;
            }
            for sc in &constraints {
                if sc.distance_from_start > step.distance_from_start {
                    continue;
                }
                self.add_segment_to_speed_constraint(profile, &mut seq, *sc, sp);
            }
            let last = *seq.last().unwrap();
            let speed = sp.get_target(last.distance_from_start, last.altitude, ManagedSpeedType::Cruise);
            self.compute_cruise_segment(profile, &mut seq, step.distance_from_start, speed, Reason::AtmosphericConditions);
            if !self.try_add_step_from_last_checkpoint(profile, &mut seq, step, step_climb, step_descent, target_distance_from_start) {
                profile.ignore_cruise_step(i);
            }
        }

        for sc in constraints {
            if sc.distance_from_start > target_distance_from_start {
                continue;
            }
            self.add_segment_to_speed_constraint(profile, &mut seq, sc, sp);
        }

        let last = *seq.last().unwrap();
        if last.distance_from_start >= target_distance_from_start {
            return seq;
        }

        let speed_target = sp.get_target(last.distance_from_start, last.altitude, ManagedSpeedType::Cruise);
        if speed_target - last.speed > 1.0 {
            let headwind = -profile.winds.cruise_tailwind(
                last.distance_from_start,
                last.distance_from_start - profile.distance_to_present_position,
                last.altitude,
            );
            // FBW passes the distance from start where the fuel on board belongs.
            let step = self.level_acceleration_step(profile, last.distance_from_start, last.speed, speed_target, headwind);
            push_step(&mut seq, &step, Reason::AtmosphericConditions, last.mach);
        }

        self.compute_cruise_segment(profile, &mut seq, target_distance_from_start, speed_target, Reason::AtmosphericConditions);
        seq
    }

    fn add_segment_to_speed_constraint(
        &self,
        profile: &Profile,
        seq: &mut Vec<Checkpoint>,
        sc: MaxSpeedConstraint,
        sp: &McduSpeedProfile,
    ) {
        let last = *seq.last().unwrap();
        if sc.distance_from_start < last.distance_from_start {
            return;
        }
        let speed = sp.get_target(last.distance_from_start, last.altitude, ManagedSpeedType::Cruise);
        self.compute_cruise_segment(profile, seq, sc.distance_from_start, speed, Reason::SpeedConstraint);
    }

    fn try_add_step_from_last_checkpoint(
        &self,
        profile: &Profile,
        seq: &mut Vec<Checkpoint>,
        step: GeographicCruiseStep,
        step_climb: &dyn ClimbStrategy,
        step_descent: &dyn ClimbStrategy,
        max_distance: f64,
    ) -> bool {
        let p = self.params;
        let last = *seq.last().unwrap();
        let is_climb = step.to_altitude > last.altitude;
        if last.reason == Reason::AtmosphericConditions {
            seq.last_mut().unwrap().reason = if is_climb { Reason::StepClimb } else { Reason::StepDescent };
        }
        let headwind = -profile.winds.cruise_tailwind(last.distance_from_start, last.distance_from_start - profile.distance_to_present_position, last.altitude);
        let results = if is_climb {
            step_climb.predict_to_altitude(last.altitude, step.to_altitude, p.managed_cruise_speed, p.managed_cruise_speed_mach, last.remaining_fuel_on_board, headwind, SurfaceConfig::default())
        } else {
            step_descent.predict_to_altitude(last.altitude, step.to_altitude, p.managed_cruise_speed, p.managed_cruise_speed, last.remaining_fuel_on_board, headwind, SurfaceConfig::default())
        };
        if last.distance_from_start + results.distance_traveled - max_distance > -50.0 {
            return false;
        }
        push_step(seq, &results, if is_climb { Reason::TopOfStepClimb } else { Reason::BottomOfStepDescent }, last.mach);
        true
    }

    pub fn final_cruise_altitude(&self, steps: &[GeographicCruiseStep]) -> f64 {
        steps.iter().rev().find(|s| !s.is_ignored).map(|s| s.to_altitude).unwrap_or(self.params.cruise_altitude)
    }

    pub fn compute_cruise_segment(
        &self,
        profile: &Profile,
        seq: &mut Vec<Checkpoint>,
        target_distance_from_start: f64,
        speed: f64,
        reason: Reason,
    ) {
        let max_step_distance = 50.0;
        let p = self.params;
        let last = *seq.last().unwrap();
        let (mut distance, mut seconds, mut fuel) = (last.distance_from_start, last.seconds_from_present, last.remaining_fuel_on_board);
        let env = StepEnv::new(p.zero_fuel_weight, p.isa_deviation, p.tropopause, p.perf_factor);

        let mut iterations = 0;
        while iterations < 1000 && distance < target_distance_from_start {
            let headwind = -profile.winds.cruise_tailwind(distance, distance - profile.distance_to_present_position, last.altitude);
            let step = pred::level_flight_step(
                profile.config,
                last.altitude,
                (target_distance_from_start - distance).min(max_step_distance),
                speed,
                p.managed_cruise_speed_mach,
                fuel,
                headwind,
                &env,
            );
            distance += step.distance_traveled;
            seconds += step.time_elapsed;
            fuel -= step.fuel_burned;
            iterations += 1;
        }

        if iterations > 0 {
            seq.push(Checkpoint {
                reason,
                distance_from_start: distance,
                altitude: last.altitude,
                seconds_from_present: seconds,
                remaining_fuel_on_board: fuel,
                speed,
                mach: p.managed_cruise_speed_mach,
                profile_phase: ProfilePhase::Cruise,
                target_speed: None,
            });
        }
    }

    fn level_acceleration_step(&self, profile: &Profile, fuel: f64, speed: f64, final_speed: f64, headwind: f64) -> StepResults {
        let p = self.params;
        let sat = p.atmosphere().predict_sat(p.cruise_altitude);
        let env = StepEnv::new(p.zero_fuel_weight, p.isa_deviation, p.tropopause, p.perf_factor);
        pred::speed_change_step(
            profile.config,
            0.0,
            p.cruise_altitude,
            speed,
            final_speed,
            p.managed_cruise_speed_mach,
            p.managed_cruise_speed_mach,
            eng::climb_thrust_corrected_n1(&profile.config.engine, p.cruise_altitude, sat),
            fuel,
            headwind,
            &env,
            None,
        )
    }
}

fn push_step(seq: &mut Vec<Checkpoint>, step: &StepResults, reason: Reason, mach: f64) {
    let l = *seq.last().unwrap();
    seq.push(Checkpoint {
        reason,
        distance_from_start: l.distance_from_start + step.distance_traveled,
        altitude: step.final_altitude,
        seconds_from_present: l.seconds_from_present + step.time_elapsed,
        speed: step.speed,
        remaining_fuel_on_board: l.remaining_fuel_on_board - step.fuel_burned,
        mach,
        profile_phase: ProfilePhase::Cruise,
        target_speed: None,
    });
}

// Translated from FlyByWire's fmgc guidance/vnav/climb/ClimbPathBuilder.ts (GPL-3.0-or-later).

use crate::atmosphere::Atmosphere;
use crate::engine as eng;
use crate::predictions::{self as pred, StepEnv, StepResults};
use crate::profile::{Checkpoint, ComputationParameters, ProfilePhase, Profile, Reason};
use crate::speed_profile::{ManagedSpeedType, SpeedProfile};
use crate::strategy::{ClimbStrategy, SurfaceConfig};

pub struct ClimbPathBuilder<'a> {
    pub params: &'a ComputationParameters,
    pub atm: Atmosphere,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FcuClimbHint {
    pub add_fcu_altitude: bool,
}

impl<'a> ClimbPathBuilder<'a> {
    pub fn new(params: &'a ComputationParameters) -> Self {
        Self { params, atm: params.atmosphere() }
    }

    pub fn compute_climb_path(
        &self,
        profile: &mut Profile,
        strategy: &dyn ClimbStrategy,
        speed_profile: &dyn SpeedProfile,
        target_altitude: f64,
        hint: FcuClimbHint,
    ) {
        self.add_climb_steps(profile, strategy, speed_profile, target_altitude, Reason::TopOfClimb);
        if hint.add_fcu_altitude {
            self.add_fcu_altitude_as_checkpoint(profile);
        }
        if speed_profile.should_take_climb_speed_limit() {
            self.add_speed_limit_as_checkpoint(profile);
        }
    }

    fn add_climb_steps(
        &self,
        profile: &mut Profile,
        strategy: &dyn ClimbStrategy,
        sp: &dyn SpeedProfile,
        final_altitude: f64,
        final_reason: Reason,
    ) {
        let mach = self.params.managed_climb_speed_mach;
        let constraints = profile.max_altitude_constraints.clone();

        for constraint in constraints {
            let (constraint_alt, constraint_dist) = (constraint.max_altitude, constraint.distance_from_start);
            if constraint_alt >= final_altitude {
                break;
            }
            if constraint_alt > profile.last().altitude {
                if profile.last().reason == Reason::AltitudeConstraint {
                    profile.last_mut().reason = Reason::ContinueClimb;
                }
                let mut index_to_reset_to = profile.checkpoints.len();
                self.build_iterated_climb_segment(profile, strategy, sp, profile.last().altitude, constraint_alt);

                let mut i = 0;
                let mut current = sp.max_climb_speed_constraint(profile.last().distance_from_start);
                while i < 10 {
                    i += 1;
                    let Some(c) = current else { break };
                    if c.distance_from_start > profile.last().distance_from_start {
                        break;
                    }
                    profile.checkpoints.truncate(index_to_reset_to);
                    let d = c.distance_from_start - profile.last().distance_from_start;
                    self.build_iterated_distance_step(profile, strategy, d, final_altitude, Reason::SpeedConstraint);
                    index_to_reset_to = profile.checkpoints.len();
                    self.build_iterated_climb_segment(profile, strategy, sp, profile.last().altitude, constraint_alt);
                    current = sp.max_climb_speed_constraint(profile.last().distance_from_start);
                }

                if profile.last().distance_from_start < constraint_dist {
                    profile.last_mut().reason = Reason::LevelOffForClimbConstraint;
                    self.add_level_segment_steps(profile, sp, constraint_dist);
                }
            } else if (profile.last().altitude - constraint_alt).abs() < 250.0 {
                self.add_level_segment_steps(profile, sp, constraint_dist);
            }
        }

        if profile.last().reason == Reason::AltitudeConstraint {
            profile.last_mut().reason = Reason::ContinueClimb;
        }

        let speed_constraints = profile.max_climb_speed_constraints.clone();
        for sc in speed_constraints {
            let Checkpoint { distance_from_start, altitude, speed, remaining_fuel_on_board, .. } = *profile.last();
            if distance_from_start > sc.distance_from_start {
                continue;
            }
            let speed_target = sp.get_target(distance_from_start, altitude, ManagedSpeedType::Climb);
            if speed_target - speed > 1.0 {
                let mut step = strategy.predict_to_speed(
                    altitude,
                    speed_target,
                    speed,
                    mach,
                    remaining_fuel_on_board,
                    -profile.winds.climb_tailwind(distance_from_start, altitude),
                    SurfaceConfig::default(),
                );
                if step.final_altitude > final_altitude {
                    let scaling = if step.final_altitude - step.initial_altitude != 0.0 {
                        (final_altitude - step.initial_altitude) / (step.final_altitude - step.initial_altitude)
                    } else {
                        0.0
                    };
                    scale_step(profile.last(), &mut step, scaling);
                }
                self.add_checkpoint_from_step(profile, &step, Reason::AtmosphericConditions);
            }

            if sc.distance_from_start > profile.last().distance_from_start {
                let d = sc.distance_from_start - profile.last().distance_from_start;
                self.build_iterated_distance_step(profile, strategy, d, final_altitude, Reason::SpeedConstraint);
                if profile.last().altitude > final_altitude {
                    profile.checkpoints.retain(|c| c.altitude <= final_altitude);
                    self.build_iterated_climb_segment(profile, strategy, sp, profile.last().altitude, final_altitude);
                }
            }
        }

        self.build_iterated_climb_segment(profile, strategy, sp, profile.last().altitude, final_altitude);
        profile.last_mut().reason = final_reason;
    }

    fn build_iterated_climb_segment(
        &self,
        profile: &mut Profile,
        strategy: &dyn ClimbStrategy,
        sp: &dyn SpeedProfile,
        starting_altitude: f64,
        target_altitude: f64,
    ) {
        let mach = self.params.managed_climb_speed_mach;
        let mut altitude = starting_altitude;
        let mut i = 0;
        while i < 100 && altitude < target_altitude {
            i += 1;
            let Checkpoint { speed, remaining_fuel_on_board, distance_from_start, .. } = *profile.last();
            let speed_target = sp.get_target(distance_from_start, altitude, ManagedSpeedType::Climb);
            let above_crossover = speed_target > self.atm.cas_from_mach(altitude, mach);
            let headwind = -profile.winds.climb_tailwind(distance_from_start, altitude);

            let mut step = if above_crossover || speed_target - speed < 1.0 {
                strategy.predict_to_altitude(
                    altitude,
                    (altitude + 1500.0).min(target_altitude),
                    speed_target,
                    mach,
                    remaining_fuel_on_board,
                    headwind,
                    SurfaceConfig::default(),
                )
            } else {
                strategy.predict_to_speed(altitude, speed_target, speed, mach, remaining_fuel_on_board, headwind, SurfaceConfig::default())
            };

            if step.final_altitude - target_altitude > 10.0 {
                let scaling = if step.final_altitude - step.initial_altitude != 0.0 {
                    (target_altitude - step.initial_altitude) / (step.final_altitude - step.initial_altitude)
                } else {
                    0.0
                };
                scale_step(profile.last(), &mut step, scaling);
            }
            self.add_checkpoint_from_step(profile, &step, Reason::AtmosphericConditions);
            altitude = step.final_altitude;
        }
    }

    fn build_iterated_distance_step(
        &self,
        profile: &mut Profile,
        strategy: &dyn ClimbStrategy,
        distance: f64,
        final_altitude: f64,
        reason: Reason,
    ) {
        let step_distance = 3.0;
        let num_steps = (distance / step_distance).ceil() as i64;
        for i in 0..num_steps {
            self.distance_step_from_last_checkpoint(
                profile,
                strategy,
                step_distance.min(distance - i as f64 * step_distance),
                final_altitude,
                Reason::AtmosphericConditions,
            );
            if profile.last().altitude >= final_altitude {
                break;
            }
        }
        if num_steps > 0 {
            profile.last_mut().reason = reason;
        }
    }

    fn distance_step_from_last_checkpoint(
        &self,
        profile: &mut Profile,
        strategy: &dyn ClimbStrategy,
        distance: f64,
        final_altitude: f64,
        reason: Reason,
    ) {
        let mach = self.params.managed_climb_speed_mach;
        let Checkpoint { distance_from_start, altitude, speed, remaining_fuel_on_board, .. } = *profile.last();
        let mut step = strategy.predict_to_distance(
            altitude,
            distance,
            speed,
            mach,
            remaining_fuel_on_board,
            -profile.winds.climb_tailwind(distance_from_start, altitude),
            SurfaceConfig::default(),
        );
        if step.final_altitude > final_altitude {
            let scaling = (final_altitude - altitude) / (step.final_altitude - altitude);
            scale_step(profile.last(), &mut step, scaling);
        }
        self.add_checkpoint_from_step(profile, &step, reason);
    }

    fn add_level_segment_steps(&self, profile: &mut Profile, sp: &dyn SpeedProfile, to_distance: f64) {
        let cfg = profile.config;
        let altitude = profile.last().altitude;
        let constraints = profile.max_climb_speed_constraints.clone();

        for sc in constraints {
            if profile.last().distance_from_start >= sc.distance_from_start || to_distance <= sc.distance_from_start {
                continue;
            }
            let current_speed = profile.last().speed;
            let speed_target = sp.get_target(profile.last().distance_from_start, altitude, ManagedSpeedType::Climb);
            if speed_target > current_speed {
                let mut step = self.level_acceleration_step(cfg, altitude, current_speed, speed_target, profile.last().remaining_fuel_on_board);
                if profile.last().distance_from_start + step.distance_traveled > sc.distance_from_start {
                    let scaling = step.distance_traveled / (sc.distance_from_start - profile.last().distance_from_start);
                    scale_step(profile.last(), &mut step, scaling);
                    self.add_checkpoint_from_step(profile, &step, Reason::AtmosphericConditions);
                    continue;
                } else {
                    self.add_checkpoint_from_step(profile, &step, Reason::AtmosphericConditions);
                }
            }
            let level = self.level_segment_prediction(
                cfg,
                sc.distance_from_start - profile.last().distance_from_start,
                altitude,
                profile.last().speed,
                profile.last().remaining_fuel_on_board,
            );
            self.add_checkpoint_from_step(profile, &level, Reason::AltitudeConstraint);
        }

        let current_speed = profile.last().speed;
        let speed_target = sp.get_target(profile.last().distance_from_start, altitude, ManagedSpeedType::Climb);
        if speed_target > current_speed {
            let mut step = self.level_acceleration_step(cfg, altitude, current_speed, speed_target, profile.last().remaining_fuel_on_board);
            if profile.last().distance_from_start + step.distance_traveled > to_distance {
                let scaling = step.distance_traveled / (to_distance - profile.last().distance_from_start);
                scale_step(profile.last(), &mut step, scaling);
                self.add_checkpoint_from_step(profile, &step, Reason::AtmosphericConditions);
                return;
            }
            self.add_checkpoint_from_step(profile, &step, Reason::AtmosphericConditions);
        }

        let level = self.level_segment_prediction(
                cfg,
            to_distance - profile.last().distance_from_start,
            altitude,
            profile.last().speed,
            profile.last().remaining_fuel_on_board,
        );
        self.add_checkpoint_from_step(profile, &level, Reason::AltitudeConstraint);
    }

    fn level_segment_prediction(&self, cfg: &crate::config::AircraftConfig, step_size: f64, altitude: f64, initial_speed: f64, fuel_weight: f64) -> StepResults {
        let env = StepEnv::new(self.params.zero_fuel_weight, self.atm.isa_deviation, self.params.tropopause, self.params.perf_factor);
        pred::level_flight_step(
            cfg,
            altitude,
            step_size,
            initial_speed,
            self.params.managed_climb_speed_mach,
            fuel_weight,
            0.0,
            &env,
        )
    }

    fn level_acceleration_step(&self, cfg: &crate::config::AircraftConfig, altitude: f64, initial_speed: f64, speed_target: f64, fuel_weight: f64) -> StepResults {
        let sat = self.atm.predict_sat(altitude);
        let env = StepEnv::new(self.params.zero_fuel_weight, self.atm.isa_deviation, self.params.tropopause, self.params.perf_factor);
        pred::speed_change_step(
            cfg,
            0.0,
            altitude,
            initial_speed,
            speed_target,
            self.params.managed_climb_speed_mach,
            self.params.managed_climb_speed_mach,
            eng::climb_thrust_corrected_n1(&cfg.engine, altitude, sat),
            fuel_weight,
            0.0,
            &env,
            None,
        )
    }

    pub fn add_speed_limit_as_checkpoint(&self, profile: &mut Profile) {
        let Some(limit) = self.params.climb_speed_limit else { return };
        if limit.under_altitude <= self.params.present_altitude || limit.under_altitude > self.params.cruise_altitude {
            return;
        }
        let d = profile.interpolate_distance_at_altitude(limit.under_altitude);
        profile.add_interpolated_checkpoint(d, Reason::CrossingClimbSpeedLimit);
    }

    fn add_fcu_altitude_as_checkpoint(&self, profile: &mut Profile) {
        let fcu = self.params.fcu_altitude;
        if fcu <= self.params.present_altitude || fcu > self.params.cruise_altitude {
            return;
        }
        let d = profile.interpolate_distance_at_altitude(fcu);
        profile.add_interpolated_checkpoint(d, Reason::CrossingFcuAltitudeClimb);
    }

    fn add_checkpoint_from_step(&self, profile: &mut Profile, step: &StepResults, reason: Reason) {
        profile.add_checkpoint_from_step(step, reason, self.params.managed_climb_speed_mach, ProfilePhase::Climb);
    }
}

pub fn scale_step(last: &Checkpoint, step: &mut StepResults, scaling: f64) {
    step.distance_traveled *= scaling;
    step.fuel_burned *= scaling;
    step.time_elapsed *= scaling;
    step.final_altitude = (1.0 - scaling) * last.altitude + scaling * step.final_altitude;
    step.speed = (1.0 - scaling) * last.speed + scaling * step.speed;
}

// Translated from FlyByWire's fmgc guidance/vnav/descent/ApproachPathBuilder.ts and BisectionMethod.ts (GPL-3.0-or-later).

use crate::config::{AircraftConfig, FlapConf};
use crate::descent::{minimum_altitude, scale_step_based_on_last_checkpoint, DescentStrategy, IdleDescentStrategy, TemporaryCheckpointSequence};
use crate::predictions::StepResults;
use crate::profile::{Checkpoint, ComputationParameters, DescentAltitudeConstraint, Profile, ProfilePhase, Reason};
use crate::speed_profile::{ManagedSpeedType, McduSpeedProfile};
use crate::strategy::{ClimbStrategy, FlightPathAngleStrategy, SurfaceConfig};

const EPSILON: f64 = 1e-4;

fn close_to_negative(x: f64) -> bool {
    -x > -EPSILON
}

fn close_to_less_than(a: f64, b: f64) -> bool {
    close_to_negative(a - b)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NonTerminationStrategy {
    PositiveErrorResult,
    NegativeErrorResult,
    LowerAbsoluteErrorResult,
}

pub fn find_zero<F: FnMut(f64) -> f64>(mut f: F, bounds: (f64, f64), tolerance: (f64, f64), strategy: NonTerminationStrategy, max_iterations: usize) -> f64 {
    let (mut a, mut b) = if bounds.0 > bounds.1 { (bounds.1, bounds.0) } else { bounds };
    let within = |e: f64| e >= tolerance.0 && e <= tolerance.1;
    let fa = f(a);
    let fb = f(b);
    if fa * fb > 0.0 {
        return if fa.abs() < fb.abs() { a } else { b };
    }
    if fa * fb < 0.0 {
        for _ in 0..max_iterations {
            let c = (a + b) / 2.0;
            let fc = f(c);
            if within(fc) {
                return c;
            }
            if fa * fc > 0.0 {
                a = c;
            } else {
                b = c;
            }
        }
    }
    match strategy {
        NonTerminationStrategy::PositiveErrorResult => {
            if fa > 0.0 {
                a
            } else {
                b
            }
        }
        NonTerminationStrategy::NegativeErrorResult => {
            if fa < 0.0 {
                a
            } else {
                b
            }
        }
        NonTerminationStrategy::LowerAbsoluteErrorResult => {
            if fa.abs() < fb.abs() {
                a
            } else {
                b
            }
        }
    }
}

pub fn flap_conf_by_speed(speed: f64, p: &ComputationParameters) -> FlapConf {
    if speed >= p.clean_speed {
        FlapConf::Clean
    } else if speed >= p.slat_retraction_speed {
        FlapConf::Conf1
    } else if speed >= p.flap_retraction_speed {
        FlapConf::Conf2
    } else if speed >= (p.flap_retraction_speed + p.approach_speed) / 2.0 {
        FlapConf::Conf3
    } else {
        FlapConf::ConfFull
    }
}

pub fn next_extension_speed(speed: f64, p: &ComputationParameters) -> f64 {
    let conf3 = (p.flap_retraction_speed + p.approach_speed) / 2.0;
    if speed < conf3 {
        conf3
    } else if speed < p.flap_retraction_speed {
        p.flap_retraction_speed
    } else if speed < p.slat_retraction_speed {
        p.slat_retraction_speed
    } else if speed < p.clean_speed {
        p.clean_speed
    } else {
        f64::INFINITY
    }
}

pub fn approach_phase_target_speed(flaps: FlapConf, p: &ComputationParameters) -> f64 {
    match flaps {
        FlapConf::ConfFull => p.approach_speed,
        FlapConf::Conf3 | FlapConf::Conf2 => p.flap_retraction_speed,
        FlapConf::Conf1 => p.slat_retraction_speed,
        _ => p.clean_speed,
    }
}

pub fn flap_checkpoint_reason(flaps: FlapConf) -> Reason {
    match flaps {
        FlapConf::ConfFull => Reason::FlapsFull,
        FlapConf::Conf3 => Reason::Flaps3,
        FlapConf::Conf2 => Reason::Flaps2,
        FlapConf::Conf1 => Reason::Flaps1,
        _ => Reason::Decel,
    }
}

pub fn surface_config_from_speed(speed: f64, p: &ComputationParameters, use_speedbrakes: bool) -> SurfaceConfig {
    SurfaceConfig { flap_conf: flap_conf_by_speed(speed, p), speedbrakes_extended: use_speedbrakes, gear_extended: speed < p.flap_retraction_speed }
}

pub struct ApproachPathBuilder<'a> {
    pub params: &'a ComputationParameters,
    pub config: &'a AircraftConfig,
}

struct DecelAttempt {
    deceleration: TemporaryCheckpointSequence,
    descent: StepResults,
    second_deceleration: TemporaryCheckpointSequence,
}

impl<'a> ApproachPathBuilder<'a> {
    pub fn new(params: &'a ComputationParameters, config: &'a AircraftConfig) -> Self {
        Self { params, config }
    }

    fn fpa(&self, angle: f64) -> FlightPathAngleStrategy<'a> {
        FlightPathAngleStrategy { params: self.params, atm: self.params.atmosphere(), flight_path_angle_deg: angle, config: self.config }
    }

    pub fn compute_approach_path(
        &self,
        profile: &Profile,
        speed_profile: &McduSpeedProfile,
        estimated_fuel_on_board_at_destination: f64,
        estimated_seconds_from_present_at_destination: f64,
        too_steep_legs: &mut Vec<usize>,
    ) -> TemporaryCheckpointSequence {
        let p = self.params;
        let approach_constraints: Vec<DescentAltitudeConstraint> = profile.descent_altitude_constraints.iter().rev().copied().collect();
        let final_altitude = profile.final_altitude;

        let mut sequence = TemporaryCheckpointSequence::new(Checkpoint {
            reason: Reason::Landing,
            speed: p.approach_speed,
            distance_from_start: profile.get_distance_from_start(0.0),
            altitude: final_altitude,
            remaining_fuel_on_board: estimated_fuel_on_board_at_destination,
            seconds_from_present: estimated_seconds_from_present_at_destination,
            mach: p.managed_descent_speed_mach,
            profile_phase: ProfilePhase::Descent,
            target_speed: None,
        });

        let distance_to_one_thousand_agl = 1000.0 / profile.final_descent_angle.to_radians().tan() / 6076.12;
        let final_fpa = self.fpa(profile.final_descent_angle);
        let final_approach_step = final_fpa.predict_to_distance(
            final_altitude,
            distance_to_one_thousand_agl,
            p.approach_speed,
            p.managed_descent_speed_mach,
            estimated_fuel_on_board_at_destination,
            -profile.winds.descent_tailwind(profile.get_distance_from_start(0.0), final_altitude),
            surface_config_from_speed(p.approach_speed, p, false),
        );
        sequence.add_checkpoint_from_step(&final_approach_step, Reason::AtmosphericConditions, ProfilePhase::Descent);

        let faf_step = self.build_deceleration_path(&final_fpa, profile, sequence.last(), speed_profile, profile.get_distance_from_start(profile.faf_distance_to_end), false);
        sequence.push_all(faf_step.get());

        let level = self.fpa(0.0);
        for constraint in &approach_constraints {
            self.handle_altitude_constraint(&level, profile, &mut sequence, speed_profile, constraint, too_steep_legs);
            let last = sequence.last();
            let speed_target = speed_profile.get_target(last.distance_from_start, last.altitude, ManagedSpeedType::Descent);
            if last.reason == Reason::Decel || (last.speed > p.clean_speed && last.speed > speed_target) {
                break;
            }
        }

        let last = sequence.last();
        let speed_target = speed_profile.get_target(last.distance_from_start - EPSILON, last.altitude, ManagedSpeedType::Descent);
        if speed_target - last.speed > 0.1 {
            let to_descent_speed = self.build_deceleration_path(&level, profile, last, speed_profile, f64::NEG_INFINITY, false);
            sequence.push_all(to_descent_speed.get());
        }

        if sequence.last().reason != Reason::Decel {
            sequence.copy_last_checkpoint(Reason::Decel);
        }
        sequence
    }

    fn handle_altitude_constraint(
        &self,
        fpa: &FlightPathAngleStrategy,
        profile: &Profile,
        sequence: &mut TemporaryCheckpointSequence,
        speed_profile: &McduSpeedProfile,
        constraint: &DescentAltitudeConstraint,
        too_steep_legs: &mut Vec<usize>,
    ) {
        let p = self.params;
        let last = sequence.last();
        let (distance_from_start, altitude) = (last.distance_from_start, last.altitude);
        let min_alt = minimum_altitude(&constraint.constraint);
        if distance_from_start < constraint.distance_from_start || !min_alt.is_finite() || altitude - min_alt > -50.0 {
            return;
        }

        let desired = distance_from_start - constraint.distance_from_start;
        let idle = IdleDescentStrategy::new(p, self.config);

        let attempt = |decel_distance: f64, speedbrakes: bool| -> (f64, DecelAttempt) {
            let mut current = TemporaryCheckpointSequence::new(last);
            let deceleration = self.build_deceleration_path(fpa, profile, last, speed_profile, distance_from_start - decel_distance, speedbrakes);
            current.push_all(deceleration.get());
            let dl = deceleration.last();
            let descent = idle.predict_to_altitude(
                altitude,
                min_alt,
                dl.speed,
                p.managed_descent_speed_mach,
                dl.remaining_fuel_on_board,
                -profile.winds.descent_tailwind(distance_from_start - decel_distance, min_alt),
                Some(surface_config_from_speed(dl.speed, p, speedbrakes)),
            );
            current.add_checkpoint_from_step(&descent, Reason::AltitudeConstraint, ProfilePhase::Descent);
            let traveled = -descent.distance_traveled + (distance_from_start - dl.distance_from_start);
            let second_deceleration = self.build_deceleration_path(fpa, profile, current.last(), speed_profile, constraint.distance_from_start, speedbrakes);
            (traveled - desired, DecelAttempt { deceleration, descent, second_deceleration })
        };

        let solution = find_zero(|d| attempt(d, false).0, (0.0, desired), (-0.1, 0.1), NonTerminationStrategy::NegativeErrorResult, 10);
        let (_, mut result) = attempt(solution, false);
        let final_error = solution + -result.descent.distance_traveled - desired;

        if close_to_negative(solution) && !close_to_negative(final_error) {
            let with_brakes = find_zero(|d| attempt(d, true).0, (0.0, desired), (-0.1, 0.1), NonTerminationStrategy::NegativeErrorResult, 10);
            result = attempt(with_brakes, true).1;
            let error_with_brakes = with_brakes + -result.descent.distance_traveled - desired;
            if close_to_negative(with_brakes) && !close_to_negative(error_with_brakes) {
                let scaling = desired / -result.descent.distance_traveled;
                let mut step = result.descent;
                scale_step_based_on_last_checkpoint(&sequence.last(), &mut step, scaling);
                sequence.add_checkpoint_from_step(&step, Reason::GeometricPathTooSteep, ProfilePhase::Descent);
                sequence.copy_last_checkpoint(Reason::AltitudeConstraint);
                sequence.last_mut().altitude = min_alt;
                too_steep_legs.push(constraint.leg);
                return;
            }
        }

        sequence.push_all(result.deceleration.get());
        let dl = result.deceleration.last();
        let speed_target = speed_profile.get_target(dl.distance_from_start, dl.altitude, ManagedSpeedType::Descent);
        if speed_target - dl.speed > 1.0 || dl.speed < p.clean_speed {
            sequence.add_checkpoint_from_step(&result.descent, Reason::AltitudeConstraint, ProfilePhase::Descent);
            sequence.push_all(result.second_deceleration.get());
        }
    }

    fn build_deceleration_path(
        &self,
        fpa: &FlightPathAngleStrategy,
        profile: &Profile,
        last_checkpoint: Checkpoint,
        speed_profile: &McduSpeedProfile,
        target_distance_from_start: f64,
        use_speedbrakes: bool,
    ) -> TemporaryCheckpointSequence {
        let p = self.params;
        let mut seq = TemporaryCheckpointSequence::new(last_checkpoint);
        let mut last_acceleration_index = 1;
        let done = |s: &TemporaryCheckpointSequence| s.last().reason == Reason::Decel || close_to_less_than(s.last().distance_from_start, target_distance_from_start);

        let mut i = 0;
        while i < 10 && !done(&seq) {
            i += 1;
            let Checkpoint { distance_from_start, altitude, speed, remaining_fuel_on_board, .. } = seq.last();
            let speed_constraint = speed_profile.max_descent_speed_constraint(distance_from_start - EPSILON);
            let flap_target_speed = next_extension_speed(speed, p);
            let limiting_speed = speed_profile.target_without_constraints(altitude, ManagedSpeedType::Descent);

            match speed_constraint {
                Some(sc) if sc.max_speed < flap_target_speed && sc.max_speed < limiting_speed => {
                    let remaining_distance = sc.distance_from_start.max(target_distance_from_start) - distance_from_start;
                    let mut step = fpa.predict_to_speed(
                        altitude,
                        sc.max_speed.max(speed),
                        speed,
                        p.managed_descent_speed_mach,
                        remaining_fuel_on_board,
                        -profile.winds.descent_tailwind(distance_from_start, altitude),
                        surface_config_from_speed(speed, p, use_speedbrakes),
                    );
                    if step.error.is_some() || step.distance_traveled > 0.0 {
                        seq.checkpoints.truncate(last_acceleration_index);
                        break;
                    }
                    last_acceleration_index = seq.len();
                    if close_to_negative(step.distance_traveled) {
                        let scaling = (remaining_distance / step.distance_traveled).min(1.0);
                        scale_step_based_on_last_checkpoint(&seq.last(), &mut step, scaling);
                        seq.add_checkpoint_from_step(&step, Reason::AtmosphericConditions, ProfilePhase::Descent);
                    }
                    let remaining_to_constraint = distance_from_start + step.distance_traveled - sc.distance_from_start.max(target_distance_from_start);
                    if remaining_to_constraint > 0.05 {
                        if sc.max_speed > p.clean_speed {
                            seq.last_mut().reason = Reason::Decel;
                            return seq;
                        }
                        let l = seq.last();
                        let headwind = -profile.winds.descent_tailwind(l.distance_from_start, l.altitude);
                        let constant = fpa.predict_to_distance(
                            l.altitude,
                            -remaining_to_constraint,
                            sc.max_speed,
                            p.managed_descent_speed_mach,
                            l.remaining_fuel_on_board,
                            headwind,
                            surface_config_from_speed(sc.max_speed, p, use_speedbrakes),
                        );
                        seq.add_checkpoint_from_step(&constant, Reason::SpeedConstraint, ProfilePhase::Descent);
                    } else {
                        seq.copy_last_checkpoint(Reason::SpeedConstraint);
                    }
                }
                _ => {
                    let remaining_distance = target_distance_from_start - distance_from_start;
                    let target_speed = flap_target_speed.min(limiting_speed);
                    let config = surface_config_from_speed(speed, p, use_speedbrakes);
                    let mut step = fpa.predict_to_speed(
                        altitude,
                        target_speed,
                        speed,
                        p.managed_descent_speed_mach,
                        remaining_fuel_on_board,
                        -profile.winds.descent_tailwind(distance_from_start, altitude),
                        config,
                    );
                    if step.error.is_some() || step.distance_traveled > 0.0 {
                        seq.checkpoints.truncate(last_acceleration_index);
                        break;
                    }
                    last_acceleration_index = seq.len();
                    if step.distance_traveled < remaining_distance {
                        let scaling = (remaining_distance / step.distance_traveled).min(1.0);
                        scale_step_based_on_last_checkpoint(&seq.last(), &mut step, scaling);
                        seq.add_checkpoint_from_step(&step, Reason::AtmosphericConditions, ProfilePhase::Descent);
                        return seq;
                    }
                    seq.add_deceleration_checkpoint_from_step(&step, flap_checkpoint_reason(config.flap_conf), approach_phase_target_speed(config.flap_conf, p), ProfilePhase::Descent);
                }
            }
        }

        if !done(&seq) {
            let l = seq.last();
            let config = surface_config_from_speed(l.speed, p, use_speedbrakes);
            let headwind = -profile.winds.descent_tailwind(l.distance_from_start, l.altitude);
            let constant = fpa.predict_to_distance(
                l.altitude,
                target_distance_from_start - l.distance_from_start,
                l.speed,
                p.managed_descent_speed_mach,
                l.remaining_fuel_on_board,
                headwind,
                config,
            );
            seq.add_deceleration_checkpoint_from_step(&constant, flap_checkpoint_reason(config.flap_conf), approach_phase_target_speed(config.flap_conf, p), ProfilePhase::Descent);
        }
        seq
    }
}

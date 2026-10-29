// Translated from FlyByWire's fmgc guidance/vnav/descent: DescentStrategy.ts, IdlePathBuilder.ts,
// GeometricPathPlanner.ts, GeometricPathBuilder.ts, DescentPathBuilder.ts and profile/TemporaryCheckpointSequence.ts
// (GPL-3.0-or-later). TacticalDescentPathBuilder and the descent guidance laws are not translated yet.

use crate::atmosphere::{interpolate, Atmosphere};
use crate::config::{AircraftConfig, FlapConf};
use crate::engine as eng;
use crate::predictions::{self as pred, StepEnv, StepResults, VnavStepError};
use crate::profile::{
    AltitudeConstraint, AltitudeDescriptor, Checkpoint, ComputationParameters, DescentAltitudeConstraint, MaxSpeedConstraint, Profile,
    ProfilePhase, Reason, SpeedLimit,
};
use crate::speed_profile::{ManagedSpeedType, McduSpeedProfile};
use crate::strategy::{ClimbStrategy, FlightPathAngleStrategy, SurfaceConfig};

const CLOSE_EPSILON: f64 = 1e-4;

fn close_less_than(a: f64, b: f64) -> bool {
    b - a > -CLOSE_EPSILON
}

fn close_greater_than(a: f64, b: f64) -> bool {
    a - b > -CLOSE_EPSILON
}

pub fn is_altitude_constraint_met(c: &AltitudeConstraint, altitude: f64, tol: f64) -> bool {
    use AltitudeDescriptor::*;
    match c.descriptor {
        AtAlt1 | AtAlt1GsIntcptAlt2 | AtAlt1AngleAlt2 => (altitude - c.altitude1).abs() < tol,
        AtOrAboveAlt1 | AtOrAboveAlt1GsIntcptAlt2 | AtOrAboveAlt1AngleAlt2 => altitude - c.altitude1 > -tol,
        AtOrBelowAlt1 | AtOrBelowAlt1AngleAlt2 => altitude - c.altitude1 < tol,
        BetweenAlt1Alt2 => altitude - c.altitude2 > -tol && altitude - c.altitude1 < tol,
        AtOrAboveAlt2 => altitude - c.altitude2 > -tol,
        _ => true,
    }
}

pub fn evaluate_altitude_constraint(c: &AltitudeConstraint, altitude: f64, tol: f64) -> (bool, f64) {
    use AltitudeDescriptor::*;
    let met = is_altitude_constraint_met(c, altitude, tol);
    match c.descriptor {
        AtAlt1 | AtAlt1GsIntcptAlt2 | AtAlt1AngleAlt2 => (met, c.altitude1),
        AtOrAboveAlt1 | AtOrAboveAlt1GsIntcptAlt2 | AtOrAboveAlt1AngleAlt2 => (met, altitude.max(c.altitude1)),
        AtOrBelowAlt1 | AtOrBelowAlt1AngleAlt2 => (met, altitude.min(c.altitude1)),
        BetweenAlt1Alt2 => (met, altitude.min(c.altitude1).max(c.altitude2)),
        AtOrAboveAlt2 => (met, altitude.max(c.altitude2)),
        _ => (true, altitude),
    }
}

pub fn minimum_altitude(c: &AltitudeConstraint) -> f64 {
    use AltitudeDescriptor::*;
    match c.descriptor {
        AtAlt1 | AtAlt1GsIntcptAlt2 | AtAlt1AngleAlt2 | AtOrAboveAlt1 | AtOrAboveAlt1GsIntcptAlt2 | AtOrAboveAlt1AngleAlt2 => c.altitude1,
        BetweenAlt1Alt2 => c.altitude2,
        _ => f64::NEG_INFINITY,
    }
}

#[derive(Clone, Debug)]
pub struct TemporaryCheckpointSequence {
    pub checkpoints: Vec<Checkpoint>,
}

impl TemporaryCheckpointSequence {
    pub fn new(first: Checkpoint) -> Self {
        Self { checkpoints: vec![first] }
    }

    pub fn len(&self) -> usize {
        self.checkpoints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.checkpoints.is_empty()
    }

    pub fn at(&self, index: usize) -> Checkpoint {
        self.checkpoints[index]
    }

    pub fn reset(&mut self, first: Checkpoint) {
        self.checkpoints = vec![first];
    }

    pub fn last(&self) -> Checkpoint {
        *self.checkpoints.last().expect("empty checkpoint sequence")
    }

    pub fn last_mut(&mut self) -> &mut Checkpoint {
        self.checkpoints.last_mut().expect("empty checkpoint sequence")
    }

    pub fn get(&self) -> &[Checkpoint] {
        &self.checkpoints[1..]
    }

    pub fn push_all(&mut self, cps: &[Checkpoint]) {
        self.checkpoints.extend_from_slice(cps);
    }

    pub fn copy_last_checkpoint(&mut self, reason: Reason) {
        let mut c = self.last();
        c.reason = reason;
        self.checkpoints.push(c);
    }

    pub fn add_checkpoint_from_step(&mut self, step: &StepResults, reason: Reason, phase: ProfilePhase) {
        let l = self.last();
        self.checkpoints.push(Checkpoint {
            reason,
            distance_from_start: l.distance_from_start + step.distance_traveled,
            altitude: step.final_altitude,
            seconds_from_present: l.seconds_from_present + step.time_elapsed,
            remaining_fuel_on_board: l.remaining_fuel_on_board - step.fuel_burned,
            speed: step.speed,
            mach: l.mach,
            profile_phase: phase,
            target_speed: None,
        });
    }

    pub fn add_deceleration_checkpoint_from_step(&mut self, step: &StepResults, reason: Reason, target_speed: f64, phase: ProfilePhase) {
        self.add_checkpoint_from_step(step, reason, phase);
        self.last_mut().target_speed = Some(target_speed);
    }

    pub fn interpolate_altitude_backwards(&self, distance_from_start: f64) -> f64 {
        let c = &self.checkpoints;
        if distance_from_start >= c[0].distance_from_start {
            return c[0].altitude;
        }
        for i in 1..c.len() {
            if distance_from_start >= c[i].distance_from_start {
                return interpolate(distance_from_start, c[i - 1].distance_from_start, c[i].distance_from_start, c[i - 1].altitude, c[i].altitude);
            }
        }
        self.last().altitude
    }
}

pub fn scale_step_based_on_last_checkpoint(last: &Checkpoint, step: &mut StepResults, scaling: f64) {
    step.distance_traveled *= scaling;
    step.fuel_burned *= scaling;
    step.time_elapsed *= scaling;
    step.final_altitude = (1.0 - scaling) * last.altitude + scaling * step.final_altitude;
    step.speed = (1.0 - scaling) * last.speed + scaling * step.speed;
}

pub trait DescentStrategy {
    fn predict_to_altitude(&self, initial_altitude: f64, final_altitude: f64, speed: f64, mach: f64, fuel_on_board: f64, headwind: f64, config: Option<SurfaceConfig>) -> StepResults;
    fn predict_to_distance(&self, initial_altitude: f64, distance: f64, speed: f64, mach: f64, fuel_on_board: f64, headwind: f64, config: Option<SurfaceConfig>) -> StepResults;
    fn predict_to_speed(&self, initial_altitude: f64, final_speed: f64, initial_speed: f64, mach: f64, fuel_on_board: f64, headwind: f64, config: Option<SurfaceConfig>) -> StepResults;
}

pub struct IdleDescentStrategy<'a> {
    pub params: &'a ComputationParameters,
    pub atm: Atmosphere,
    pub config: &'a AircraftConfig,
    pub default_config: SurfaceConfig,
}

impl<'a> IdleDescentStrategy<'a> {
    pub fn new(params: &'a ComputationParameters, config: &'a AircraftConfig) -> Self {
        Self { params, atm: params.atmosphere(), config, default_config: SurfaceConfig::default() }
    }

    pub fn above_profile(params: &'a ComputationParameters, config: &'a AircraftConfig) -> Self {
        Self {
            params,
            atm: params.atmosphere(),
            config,
            default_config: SurfaceConfig { flap_conf: FlapConf::Clean, gear_extended: false, speedbrakes_extended: true },
        }
    }

    fn env(&self, c: SurfaceConfig) -> StepEnv {
        StepEnv {
            zero_fuel_weight: self.params.zero_fuel_weight,
            isa_dev: self.atm.isa_deviation,
            tropo_altitude: self.params.tropopause,
            perf_factor_percent: self.params.perf_factor,
            speedbrakes_extended: c.speedbrakes_extended,
            flaps: c.flap_conf,
            gear_extended: c.gear_extended,
        }
    }

    fn idle_n1(&self, altitude: f64, mach: f64) -> f64 {
        eng::idle_corrected_n1(&self.config.engine, altitude, mach, self.params.tropopause) + self.config.vnav.idle_n1_margin
    }
}

impl DescentStrategy for IdleDescentStrategy<'_> {
    fn predict_to_altitude(&self, ia: f64, fa: f64, speed: f64, mach: f64, fob: f64, hw: f64, config: Option<SurfaceConfig>) -> StepResults {
        let c = config.unwrap_or(self.default_config);
        let midway = (ia + fa) / 2.0;
        let computed_mach = self.atm.mach_from_cas(midway, speed).min(mach);
        let n1 = self.idle_n1(midway, computed_mach);
        pred::altitude_step(self.config, ia, fa - ia, speed, mach, n1, fob, hw, &self.env(c))
    }

    fn predict_to_distance(&self, ia: f64, distance: f64, speed: f64, mach: f64, fob: f64, hw: f64, config: Option<SurfaceConfig>) -> StepResults {
        let c = config.unwrap_or(self.default_config);
        let computed_mach = self.atm.mach_from_cas(ia, speed).min(mach);
        let n1 = self.idle_n1(ia, computed_mach);
        pred::distance_step(self.config, ia, distance, speed, mach, n1, fob, hw, &self.env(c))
    }

    fn predict_to_speed(&self, ia: f64, final_speed: f64, initial_speed: f64, mach: f64, fob: f64, hw: f64, config: Option<SurfaceConfig>) -> StepResults {
        let c = config.unwrap_or(self.default_config);
        let computed_mach = self.atm.mach_from_cas(ia, initial_speed).min(mach);
        let n1 = self.idle_n1(ia, computed_mach);
        let initial_mach = self.atm.mach_from_cas(ia, initial_speed).min(mach);
        let final_mach = self.atm.mach_from_cas(ia, final_speed).min(mach);
        pred::speed_change_step(self.config, -1.0, ia, initial_speed, final_speed, initial_mach, final_mach, n1, fob, hw, &self.env(c), None)
    }
}

const DISTANCE_EPSILON: f64 = 1e-4;

fn next_speed_constraint(constraints: &[MaxSpeedConstraint], last_distance: f64) -> Option<MaxSpeedConstraint> {
    for c in constraints.iter().rev() {
        if close_greater_than(c.distance_from_start, last_distance) {
            continue;
        }
        return Some(*c);
    }
    None
}

pub struct IdlePathBuilder<'a> {
    pub params: &'a ComputationParameters,
    pub atm: Atmosphere,
    pub config: &'a AircraftConfig,
}

impl<'a> IdlePathBuilder<'a> {
    pub fn new(params: &'a ComputationParameters, config: &'a AircraftConfig) -> Self {
        Self { params, atm: params.atmosphere(), config }
    }

    pub fn build_idle_path(
        &self,
        profile: &Profile,
        sequence: &mut TemporaryCheckpointSequence,
        descent_speed_constraints: &[MaxSpeedConstraint],
        speed_profile: &McduSpeedProfile,
        top_of_descent_altitude: f64,
        to_distance: f64,
    ) {
        sequence.copy_last_checkpoint(Reason::IdlePathEnd);
        self.build_idle_sequence(profile, sequence, descent_speed_constraints, speed_profile, top_of_descent_altitude, to_distance);
        if sequence.last().reason == Reason::IdlePathAtmosphericConditions {
            sequence.last_mut().reason = Reason::TopOfDescent;
        } else {
            sequence.copy_last_checkpoint(Reason::TopOfDescent);
        }
    }

    fn build_idle_sequence(
        &self,
        profile: &Profile,
        sequence: &mut TemporaryCheckpointSequence,
        descent_speed_constraints: &[MaxSpeedConstraint],
        speed_profile: &McduSpeedProfile,
        top_of_descent_altitude: f64,
        to_distance: f64,
    ) {
        let managed_descent_mach = self.params.managed_descent_speed_mach;
        let strategy = IdleDescentStrategy::new(self.params, self.config);
        let take_limit = speed_profile.should_take_descent_speed_limit();
        let limit = self.params.descent_speed_limit;

        let mut previous_cas_target = self
            .try_get_anticipated_target(sequence, descent_speed_constraints, if take_limit { limit } else { None })
            .unwrap_or_else(|| {
                let l = sequence.last();
                speed_profile.get_target(l.distance_from_start, l.altitude, ManagedSpeedType::Descent)
            });
        let mut was_previously_under_limit = take_limit && limit.is_some_and(|l| sequence.last().altitude < l.under_altitude);

        let mut i = 0;
        while i < 100 && !close_less_than(sequence.last().distance_from_start, to_distance) && !close_less_than_with(top_of_descent_altitude, sequence.last().altitude, 1.0) {
            let Checkpoint { distance_from_start, altitude, speed, remaining_fuel_on_board, .. } = sequence.last();
            let headwind = -profile.winds.descent_tailwind(distance_from_start, altitude);

            let is_under_limit = take_limit && limit.is_some_and(|l| altitude < l.under_altitude);

            let cas_target = speed_profile.get_target(distance_from_start - DISTANCE_EPSILON, altitude + DISTANCE_EPSILON, ManagedSpeedType::Descent);
            let current_speed_target = cas_target.min(self.atm.cas_from_mach(altitude, managed_descent_mach));
            let can_accelerate = current_speed_target > speed;

            if can_accelerate {
                let mut speed_step = strategy.predict_to_speed(altitude, cas_target, speed, managed_descent_mach, remaining_fuel_on_board, headwind, None);
                let scaling = 1.0_f64
                    .min((top_of_descent_altitude - altitude) / (speed_step.final_altitude - altitude))
                    .min((to_distance - sequence.last().distance_from_start) / speed_step.distance_traveled);
                scale_step_based_on_last_checkpoint(&sequence.last(), &mut speed_step, scaling);

                let crossed_limit = was_previously_under_limit && !is_under_limit;
                let reason = if crossed_limit { Reason::StartDecelerationToLimit } else { Reason::StartDecelerationToConstraint };
                sequence.add_deceleration_checkpoint_from_step(&speed_step, reason, previous_cas_target, ProfilePhase::Descent);
            } else {
                let mut final_altitude = (altitude + 1500.0).min(top_of_descent_altitude);
                if is_under_limit {
                    final_altitude = final_altitude.min(limit.unwrap().under_altitude);
                }

                let mut altitude_step = strategy.predict_to_altitude(altitude, final_altitude, speed, managed_descent_mach, remaining_fuel_on_board, headwind, None);
                let scaling = 1.0_f64.min((to_distance - sequence.last().distance_from_start) / altitude_step.distance_traveled);
                scale_step_based_on_last_checkpoint(&sequence.last(), &mut altitude_step, scaling);

                let reason = if is_under_limit && altitude_step.final_altitude >= limit.unwrap().under_altitude {
                    Reason::CrossingDescentSpeedLimit
                } else {
                    Reason::IdlePathAtmosphericConditions
                };

                match next_speed_constraint(descent_speed_constraints, sequence.last().distance_from_start) {
                    Some(c) if distance_from_start + altitude_step.distance_traveled < c.distance_from_start => {
                        let distance_to_constraint = c.distance_from_start - distance_from_start;
                        let distance_step = strategy.predict_to_distance(altitude, distance_to_constraint, speed, managed_descent_mach, remaining_fuel_on_board, headwind, None);
                        sequence.add_checkpoint_from_step(&distance_step, Reason::SpeedConstraint, ProfilePhase::Descent);
                    }
                    _ => sequence.add_checkpoint_from_step(&altitude_step, reason, ProfilePhase::Descent),
                }
            }

            previous_cas_target = cas_target;
            was_previously_under_limit = is_under_limit;
            i += 1;
        }
    }

    fn try_get_anticipated_target(&self, sequence: &TemporaryCheckpointSequence, speed_constraints: &[MaxSpeedConstraint], speed_limit: Option<SpeedLimit>) -> Option<f64> {
        let Checkpoint { distance_from_start: ppos_distance, speed: current_speed, altitude: current_altitude, .. } = sequence.last();

        let next = speed_constraints.iter().find(|c| c.distance_from_start >= ppos_distance && c.max_speed <= current_speed);
        let limit_is_candidate = speed_limit.filter(|l| current_altitude > l.under_altitude && current_speed > l.speed);

        if let Some(c) = next {
            if let Some(l) = limit_is_candidate {
                let alt_at_constraint = sequence.interpolate_altitude_backwards(c.distance_from_start);
                if l.under_altitude > alt_at_constraint {
                    return Some(l.speed);
                }
            }
            return Some(c.max_speed);
        }
        limit_is_candidate.map(|l| l.speed)
    }
}

fn close_less_than_with(a: f64, b: f64, eps: f64) -> bool {
    b - a > -eps.abs()
}

#[derive(Clone, Copy, Debug)]
pub struct GeometricPoint {
    pub distance_from_start: f64,
    pub altitude: f64,
    pub leg: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct PlannedGeometricSegment {
    pub gradient: f64,
    pub end: GeometricPoint,
    pub is_too_steep: bool,
}

fn calculate_gradient(start: &GeometricPoint, end: &GeometricPoint) -> f64 {
    if (start.distance_from_start - end.distance_from_start).abs() < 1e-9 {
        0.0
    } else {
        (start.altitude - end.altitude) / (start.distance_from_start - end.distance_from_start)
    }
}

pub fn plan_descent_segments(
    constraints: &[DescentAltitudeConstraint],
    start: GeometricPoint,
    end: GeometricPoint,
    segments: &mut Vec<PlannedGeometricSegment>,
    tolerance: f64,
) {
    let gradient = calculate_gradient(&start, &end);

    for c in constraints {
        if c.distance_from_start >= start.distance_from_start || c.distance_from_start <= end.distance_from_start {
            continue;
        }
        let alt_at_constraint = start.altitude + gradient * (c.distance_from_start - start.distance_from_start);
        let (met, altitude_to_continue_from) = evaluate_altitude_constraint(&c.constraint, alt_at_constraint, tolerance);
        if !met {
            let center = GeometricPoint { distance_from_start: c.distance_from_start, altitude: altitude_to_continue_from, leg: c.leg };
            plan_descent_segments(constraints, start, center, segments, tolerance);
            plan_descent_segments(constraints, center, end, segments, tolerance);
            return;
        }
    }
    segments.push(PlannedGeometricSegment { end, gradient, is_too_steep: false });
}

#[derive(Clone, Copy, Debug)]
struct AccelerationTarget {
    distance_from_start: f64,
    speed: f64,
    is_speed_limit: bool,
}

pub struct DescentPathBuilder<'a> {
    pub params: &'a ComputationParameters,
    pub atm: Atmosphere,
    pub config: &'a AircraftConfig,
}

impl<'a> DescentPathBuilder<'a> {
    pub fn new(params: &'a ComputationParameters, config: &'a AircraftConfig) -> Self {
        Self { params, atm: params.atmosphere(), config }
    }

    pub fn compute_managed_descent_path(
        &self,
        sequence: &mut TemporaryCheckpointSequence,
        profile: &Profile,
        speed_profile: &McduSpeedProfile,
        cruise_altitude: f64,
        to_distance: f64,
        too_steep_legs: &mut Vec<usize>,
    ) {
        const TOL: f64 = 10.0;
        let decel_point = sequence.last();
        let mut geometric_sequence = TemporaryCheckpointSequence::new(decel_point);
        let mut idle_sequence = TemporaryCheckpointSequence::new(decel_point);
        let idle_builder = IdlePathBuilder::new(self.params, self.config);

        idle_builder.build_idle_path(profile, &mut idle_sequence, &profile.descent_speed_constraints, speed_profile, cruise_altitude, to_distance);

        for i in (0..profile.descent_altitude_constraints.len()).rev() {
            let cp = profile.descent_altitude_constraints[i];

            if cp.distance_from_start >= decel_point.distance_from_start || cp.distance_from_start <= to_distance {
                continue;
            } else if !(minimum_altitude(&cp.constraint) <= cruise_altitude) {
                break;
            }

            let alt_at_constraint = idle_sequence.interpolate_altitude_backwards(cp.distance_from_start);
            let (met, altitude_to_continue_from) = evaluate_altitude_constraint(&cp.constraint, alt_at_constraint, TOL);
            if !met {
                let geometric_path_point = GeometricPoint { distance_from_start: cp.distance_from_start, altitude: altitude_to_continue_from, leg: cp.leg };

                let mut segments = Vec::new();
                let start = GeometricPoint { distance_from_start: decel_point.distance_from_start, altitude: decel_point.altitude, leg: usize::MAX };
                plan_descent_segments(&profile.descent_altitude_constraints, start, geometric_path_point, &mut segments, TOL);

                geometric_sequence.reset(geometric_sequence.at(0));
                self.execute_geometric_segments(&mut geometric_sequence, &mut segments, profile, speed_profile, too_steep_legs);

                idle_sequence.reset(geometric_sequence.last());
                idle_builder.build_idle_path(profile, &mut idle_sequence, &profile.descent_speed_constraints, speed_profile, cruise_altitude, to_distance);
            }
        }

        if geometric_sequence.len() > 1 {
            geometric_sequence.last_mut().reason = Reason::GeometricPathStart;
        } else {
            geometric_sequence.copy_last_checkpoint(Reason::GeometricPathStart);
        }

        let geometric: Vec<Checkpoint> = geometric_sequence.get().to_vec();
        let idle: Vec<Checkpoint> = idle_sequence.get().to_vec();
        sequence.push_all(&geometric);
        sequence.push_all(&idle);
    }

    pub fn execute_geometric_segments(
        &self,
        sequence: &mut TemporaryCheckpointSequence,
        segments: &mut [PlannedGeometricSegment],
        profile: &Profile,
        speed_profile: &McduSpeedProfile,
        too_steep_legs: &mut Vec<usize>,
    ) {
        let acceleration_targets = self.build_acceleration_targets(&sequence.last(), segments, &profile.descent_speed_constraints);

        for segment in segments.iter_mut() {
            let mut current = TemporaryCheckpointSequence::new(sequence.last());
            let fpa = (segment.gradient / 6076.12).atan().to_degrees();

            if !self.execute_geometric_segment(profile, &mut current, segment, &acceleration_targets, false, fpa) {
                if !self.execute_geometric_segment(profile, &mut current, segment, &acceleration_targets, true, fpa) {
                    self.compute_managed_descent_path(&mut current, profile, speed_profile, f64::INFINITY, segment.end.distance_from_start, too_steep_legs);

                    for cp in current.checkpoints.iter_mut() {
                        if matches!(cp.reason, Reason::GeometricPathStart | Reason::GeometricPathEnd | Reason::IdlePathEnd | Reason::TopOfDescent) {
                            cp.reason = Reason::AtmosphericConditions;
                        }
                    }

                    let appended: Vec<Checkpoint> = current.get().to_vec();
                    sequence.push_all(&appended);
                    sequence.last_mut().reason = Reason::GeometricPathTooSteep;

                    sequence.copy_last_checkpoint(Reason::AltitudeConstraint);
                    sequence.last_mut().altitude = segment.end.altitude;

                    segment.is_too_steep = true;
                    too_steep_legs.push(segment.end.leg);
                    continue;
                }
            }

            let appended: Vec<Checkpoint> = current.get().to_vec();
            sequence.push_all(&appended);
        }
    }

    fn execute_geometric_segment(
        &self,
        profile: &Profile,
        sequence: &mut TemporaryCheckpointSequence,
        segment: &PlannedGeometricSegment,
        acceleration_targets: &[AccelerationTarget],
        use_speedbrakes: bool,
        flight_path_angle: f64,
    ) -> bool {
        let managed_descent_mach = self.params.managed_descent_speed_mach;
        let strategy = FlightPathAngleStrategy { params: self.params, atm: self.atm, flight_path_angle_deg: flight_path_angle, config: self.config };
        let mut last_target: Option<AccelerationTarget> = None;

        for target in acceleration_targets {
            let max_distance = segment.end.distance_from_start - sequence.last().distance_from_start;

            if target.distance_from_start > sequence.last().distance_from_start || max_distance >= 0.0 {
                continue;
            }

            let lt = *last_target.get_or_insert(*target);
            let l = sequence.last();

            let mut deceleration_step = strategy.predict_to_speed(
                l.altitude,
                target.speed,
                l.speed,
                managed_descent_mach,
                l.remaining_fuel_on_board,
                -profile.winds.descent_tailwind(l.distance_from_start, l.altitude),
                SurfaceConfig { speedbrakes_extended: use_speedbrakes, flap_conf: FlapConf::Clean, gear_extended: false },
            );

            if deceleration_step.error == Some(VnavStepError::AvailableGradientInsufficient) {
                return false;
            }

            if deceleration_step.distance_traveled < max_distance.min(0.0) {
                let scaling = max_distance / deceleration_step.distance_traveled;
                scale_step_based_on_last_checkpoint(&l, &mut deceleration_step, scaling);
                sequence.add_checkpoint_from_step(&deceleration_step, Reason::SpeedConstraint, ProfilePhase::Descent);
            } else if target.distance_from_start.max(segment.end.distance_from_start) < l.distance_from_start + deceleration_step.distance_traveled {
                if deceleration_step.distance_traveled < 0.0 {
                    let reason = if lt.is_speed_limit { Reason::StartDecelerationToLimit } else { Reason::StartDecelerationToConstraint };
                    sequence.add_deceleration_checkpoint_from_step(&deceleration_step, reason, lt.speed, ProfilePhase::Descent);
                }

                let l = sequence.last();
                let headwind = -profile.winds.descent_tailwind(l.distance_from_start, l.altitude);
                let step_to_constraint = strategy.predict_to_distance(
                    l.altitude,
                    target.distance_from_start.max(segment.end.distance_from_start) - l.distance_from_start,
                    l.speed,
                    managed_descent_mach,
                    l.remaining_fuel_on_board,
                    headwind,
                    SurfaceConfig::default(),
                );

                let reason = if target.is_speed_limit { Reason::CrossingDescentSpeedLimit } else { Reason::SpeedConstraint };
                sequence.add_checkpoint_from_step(&step_to_constraint, reason, ProfilePhase::Descent);
                last_target = Some(*target);
            }
        }

        let l = sequence.last();
        if segment.end.distance_from_start - l.distance_from_start < 0.0 {
            let step = strategy.predict_to_distance(
                l.altitude,
                segment.end.distance_from_start - l.distance_from_start,
                l.speed,
                managed_descent_mach,
                l.remaining_fuel_on_board,
                -profile.winds.descent_tailwind(l.distance_from_start, l.altitude),
                SurfaceConfig::default(),
            );
            sequence.add_checkpoint_from_step(&step, Reason::AtmosphericConditions, ProfilePhase::Descent);
        }

        true
    }

    fn build_acceleration_targets(&self, starting_point: &Checkpoint, segments: &[PlannedGeometricSegment], speed_constraints: &[MaxSpeedConstraint]) -> Vec<AccelerationTarget> {
        let managed_descent_speed = self.params.managed_descent_speed;
        let limit = self.params.descent_speed_limit.unwrap_or(SpeedLimit { speed: f64::NAN, under_altitude: f64::NAN });

        let distance_at_limit = if self.params.descent_speed_limit.is_some() { find_distance_at_altitude(limit.under_altitude, starting_point, segments) } else { None };
        let mut should_add_limit = distance_at_limit.is_some();
        let limit_target = AccelerationTarget { distance_from_start: distance_at_limit.unwrap_or(f64::NAN), speed: limit.speed, is_speed_limit: true };

        let mut targets: Vec<AccelerationTarget> = Vec::new();
        for sc in speed_constraints.iter().rev() {
            if should_add_limit && sc.distance_from_start < distance_at_limit.unwrap() {
                targets.push(limit_target);
                should_add_limit = false;
            }
            targets.push(AccelerationTarget { distance_from_start: sc.distance_from_start, speed: sc.max_speed, is_speed_limit: false });
        }

        let econ = AccelerationTarget { distance_from_start: f64::NEG_INFINITY, speed: managed_descent_speed, is_speed_limit: false };

        if should_add_limit {
            targets.push(limit_target);
        }
        targets.push(econ);

        let mut max_speed = f64::INFINITY;
        for t in targets.iter_mut().rev() {
            max_speed = max_speed.min(t.speed);
            t.speed = max_speed;
        }
        targets
    }
}

fn find_distance_at_altitude(altitude: f64, starting_point: &Checkpoint, segments: &[PlannedGeometricSegment]) -> Option<f64> {
    if starting_point.altitude > altitude {
        return None;
    }
    for s in segments {
        if altitude <= s.end.altitude {
            return Some(s.end.distance_from_start + (altitude - s.end.altitude) / s.gradient);
        }
    }
    Some(f64::NEG_INFINITY)
}

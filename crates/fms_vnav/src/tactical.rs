// Translated from FlyByWire's fmgc guidance/vnav/descent/TacticalDescentPathBuilder.ts and the DesModeStrategy of
// DescentStrategy.ts (GPL-3.0-or-later).

use crate::approach::surface_config_from_speed;
use crate::config::AircraftConfig;
use crate::descent::{minimum_altitude, DescentStrategy, IdleDescentStrategy, TemporaryCheckpointSequence};
use crate::predictions::StepResults;
use crate::profile::{Checkpoint, ComputationParameters, MaxSpeedConstraint, Profile, ProfilePhase, Reason, SpeedLimit};
use crate::strategy::{ClimbStrategy, FlightPathAngleStrategy, SurfaceConfig, VerticalSpeedStrategy};

pub struct AsDescent<S>(pub S);

impl DescentStrategy for AsDescent<VerticalSpeedStrategy<'_>> {
    fn predict_to_altitude(&self, ia: f64, fa: f64, speed: f64, mach: f64, fob: f64, hw: f64, c: Option<SurfaceConfig>) -> StepResults {
        ClimbStrategy::predict_to_altitude(&self.0, ia, fa, speed, mach, fob, hw, c.unwrap_or_default())
    }

    fn predict_to_distance(&self, ia: f64, distance: f64, speed: f64, mach: f64, fob: f64, hw: f64, c: Option<SurfaceConfig>) -> StepResults {
        ClimbStrategy::predict_to_distance(&self.0, ia, distance, speed, mach, fob, hw, c.unwrap_or_default())
    }

    fn predict_to_speed(&self, ia: f64, fs: f64, is: f64, mach: f64, fob: f64, hw: f64, c: Option<SurfaceConfig>) -> StepResults {
        ClimbStrategy::predict_to_speed(&self.0, ia, fs, is, mach, fob, hw, c.unwrap_or_default())
    }
}

impl DescentStrategy for AsDescent<FlightPathAngleStrategy<'_>> {
    fn predict_to_altitude(&self, ia: f64, fa: f64, speed: f64, mach: f64, fob: f64, hw: f64, c: Option<SurfaceConfig>) -> StepResults {
        ClimbStrategy::predict_to_altitude(&self.0, ia, fa, speed, mach, fob, hw, c.unwrap_or_default())
    }

    fn predict_to_distance(&self, ia: f64, distance: f64, speed: f64, mach: f64, fob: f64, hw: f64, c: Option<SurfaceConfig>) -> StepResults {
        ClimbStrategy::predict_to_distance(&self.0, ia, distance, speed, mach, fob, hw, c.unwrap_or_default())
    }

    fn predict_to_speed(&self, ia: f64, fs: f64, is: f64, mach: f64, fob: f64, hw: f64, c: Option<SurfaceConfig>) -> StepResults {
        ClimbStrategy::predict_to_speed(&self.0, ia, fs, is, mach, fob, hw, c.unwrap_or_default())
    }
}

pub struct DesModeStrategy<'a> {
    descent: Box<dyn DescentStrategy + 'a>,
    deceleration: IdleDescentStrategy<'a>,
}

impl<'a> DesModeStrategy<'a> {
    pub fn above_profile(params: &'a ComputationParameters, config: &'a AircraftConfig) -> Self {
        Self { descent: Box::new(IdleDescentStrategy::above_profile(params, config)), deceleration: IdleDescentStrategy::new(params, config) }
    }

    pub fn below_profile_vs(params: &'a ComputationParameters, config: &'a AircraftConfig, vertical_speed: f64) -> Self {
        let vs = VerticalSpeedStrategy { params, atm: params.atmosphere(), vertical_speed, config };
        Self { descent: Box::new(AsDescent(vs)), deceleration: IdleDescentStrategy::new(params, config) }
    }

    pub fn below_profile_fpa(params: &'a ComputationParameters, config: &'a AircraftConfig, flight_path_angle_deg: f64) -> Self {
        let fpa = FlightPathAngleStrategy { params, atm: params.atmosphere(), flight_path_angle_deg, config };
        Self { descent: Box::new(AsDescent(fpa)), deceleration: IdleDescentStrategy::new(params, config) }
    }
}

impl DescentStrategy for DesModeStrategy<'_> {
    fn predict_to_altitude(&self, ia: f64, fa: f64, speed: f64, mach: f64, fob: f64, hw: f64, c: Option<SurfaceConfig>) -> StepResults {
        self.descent.predict_to_altitude(ia, fa, speed, mach, fob, hw, c)
    }

    fn predict_to_distance(&self, ia: f64, distance: f64, speed: f64, mach: f64, fob: f64, hw: f64, c: Option<SurfaceConfig>) -> StepResults {
        self.descent.predict_to_distance(ia, distance, speed, mach, fob, hw, c)
    }

    fn predict_to_speed(&self, ia: f64, fs: f64, is: f64, mach: f64, fob: f64, hw: f64, c: Option<SurfaceConfig>) -> StepResults {
        self.deceleration.predict_to_speed(ia, fs, is, mach, fob, hw, c)
    }
}

#[derive(Clone, Debug, Default)]
pub struct DecelerationSchedule {
    pub speed_changes: Vec<Checkpoint>,
    pub approach_points: Vec<Checkpoint>,
}

#[derive(Clone, Copy, Debug)]
struct MinimumDescentAltitudeConstraint {
    distance_from_start: f64,
    minimum_altitude: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Deceleration { to_speed: f64, max_distance: f64, min_altitude: f64 },
    ToAltitude { to_altitude: f64 },
    ToDistance { to_distance: f64, min_altitude: f64 },
}

#[derive(Clone, Copy, Debug)]
struct SubPhase {
    kind: Kind,
    last_result: Option<Checkpoint>,
    level: bool,
    reason_after: Reason,
    reason_before: Reason,
}

impl SubPhase {
    fn new(kind: Kind) -> Self {
        Self { kind, last_result: None, level: false, reason_after: Reason::AtmosphericConditions, reason_before: Reason::AtmosphericConditions }
    }

    fn deceleration(to_speed: f64) -> Self {
        Self::new(Kind::Deceleration { to_speed, max_distance: f64::INFINITY, min_altitude: f64::NEG_INFINITY })
    }

    fn to_altitude(to_altitude: f64) -> Self {
        Self::new(Kind::ToAltitude { to_altitude })
    }

    fn to_distance(to_distance: f64) -> Self {
        Self::new(Kind::ToDistance { to_distance, min_altitude: f64::NEG_INFINITY })
    }

    fn before(mut self, r: Reason) -> Self {
        self.reason_before = r;
        self
    }

    fn after(mut self, r: Reason) -> Self {
        self.reason_after = r;
        self
    }

    fn level(mut self) -> Self {
        self.level = true;
        self
    }

    fn with_min_altitude(mut self, alt: f64) -> Self {
        if let Kind::Deceleration { min_altitude, .. } = &mut self.kind {
            *min_altitude = alt;
        }
        self
    }

    fn with_max_distance(mut self, d: f64) -> Self {
        if let Kind::Deceleration { max_distance, .. } = &mut self.kind {
            *max_distance = d;
        }
        self
    }

    fn deceleration_to_speed(&self) -> Option<f64> {
        match self.kind {
            Kind::Deceleration { to_speed, .. } => Some(to_speed),
            _ => None,
        }
    }

    fn set_to_speed(&mut self, v: f64) {
        if let Kind::Deceleration { to_speed, .. } = &mut self.kind {
            *to_speed = v;
        }
    }

    fn should_execute(&self, start: &Checkpoint) -> bool {
        match self.kind {
            Kind::Deceleration { to_speed, max_distance, min_altitude } => {
                start.speed > to_speed && start.distance_from_start < max_distance && (self.level || start.altitude > min_altitude)
            }
            Kind::ToAltitude { to_altitude } => start.altitude > to_altitude,
            Kind::ToDistance { to_distance, .. } => start.distance_from_start < to_distance,
        }
    }

    fn execute(&self, strategy: &dyn DescentStrategy, start: &Checkpoint, headwind: f64, configuration: SurfaceConfig) -> StepResults {
        match self.kind {
            Kind::Deceleration { to_speed, max_distance, min_altitude } => {
                let mut step = strategy.predict_to_speed(start.altitude, to_speed, start.speed, start.mach, start.remaining_fuel_on_board, headwind, Some(configuration));
                if step.final_altitude < min_altitude || start.distance_from_start + step.distance_traveled > max_distance {
                    let scaling = ((max_distance - start.distance_from_start) / step.distance_traveled)
                        .min((start.altitude - min_altitude) / (start.altitude - step.final_altitude))
                        .min(1.0)
                        .max(0.0);
                    scale_step(start, &mut step, scaling);
                }
                step
            }
            Kind::ToAltitude { to_altitude } => {
                strategy.predict_to_altitude(start.altitude, to_altitude, start.speed, start.mach, start.remaining_fuel_on_board, headwind, None)
            }
            Kind::ToDistance { to_distance, min_altitude } => {
                let mut step = strategy.predict_to_distance(
                    start.altitude,
                    to_distance - start.distance_from_start,
                    start.speed,
                    start.mach,
                    start.remaining_fuel_on_board,
                    headwind,
                    Some(configuration),
                );
                if step.final_altitude < min_altitude {
                    let scaling = (min_altitude - start.altitude) / (step.final_altitude - start.altitude);
                    scale_step(start, &mut step, scaling);
                }
                step
            }
        }
    }
}

fn scale_step(last: &Checkpoint, step: &mut StepResults, scaling: f64) {
    step.distance_traveled *= scaling;
    step.fuel_burned *= scaling;
    step.time_elapsed *= scaling;
    step.final_altitude = (1.0 - scaling) * last.altitude + scaling * step.final_altitude;
    step.speed = (1.0 - scaling) * last.speed + scaling * step.speed;
}

fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

struct PhaseTable<'a> {
    params: &'a ComputationParameters,
    start: Checkpoint,
    phases: Vec<SubPhase>,
}

impl PhaseTable<'_> {
    fn execute(&mut self, descent: &dyn DescentStrategy, level: &dyn DescentStrategy, profile: &Profile) -> TemporaryCheckpointSequence {
        let mut sequence = TemporaryCheckpointSequence::new(self.start);
        for i in 0..self.phases.len() {
            let phase = self.phases[i];
            if phase.reason_before != Reason::AtmosphericConditions {
                if sequence.last().reason == Reason::AtmosphericConditions {
                    sequence.last_mut().reason = phase.reason_before;
                } else {
                    sequence.copy_last_checkpoint(phase.reason_before);
                }
            }
            let last = sequence.last();
            if phase.should_execute(&last) {
                let headwind = -profile.winds.descent_tailwind(last.distance_from_start, last.altitude);
                let configuration = surface_config_from_speed(last.speed, self.params, false);
                let step = phase.execute(if phase.level { level } else { descent }, &last, headwind, configuration);
                if let Some(to_speed) = phase.deceleration_to_speed() {
                    sequence.last_mut().target_speed = Some(to_speed);
                }
                sequence.add_checkpoint_from_step(&step, phase.reason_after, ProfilePhase::Descent);
                self.phases[i].last_result = Some(sequence.last());
            } else {
                self.phases[i].last_result = None;
            }
        }
        self.start = sequence.at(0);
        sequence
    }
}

pub struct TacticalDescentPathBuilder<'a> {
    params: &'a ComputationParameters,
    level_flight: AsDescent<VerticalSpeedStrategy<'a>>,
}

impl<'a> TacticalDescentPathBuilder<'a> {
    pub fn new(params: &'a ComputationParameters, config: &'a AircraftConfig) -> Self {
        Self { params, level_flight: AsDescent(VerticalSpeedStrategy { params, atm: params.atmosphere(), vertical_speed: 0.0, config }) }
    }

    fn constraints(profile: &Profile, schedule: &DecelerationSchedule) -> (Vec<MinimumDescentAltitudeConstraint>, Vec<MaxSpeedConstraint>) {
        let mut min_alt = f64::INFINITY;
        let alts = profile
            .descent_altitude_constraints
            .iter()
            .map(|c| {
                min_alt = js_min(min_alt, minimum_altitude(&c.constraint));
                MinimumDescentAltitudeConstraint { distance_from_start: c.distance_from_start, minimum_altitude: min_alt }
            })
            .collect();
        let decel = schedule.approach_points.first().map_or(f64::INFINITY, |p| p.distance_from_start);
        let speeds = profile.descent_speed_constraints.iter().filter(|c| c.distance_from_start < decel).copied().collect();
        (alts, speeds)
    }

    fn run(
        &self,
        profile: &mut Profile,
        descent: &dyn DescentStrategy,
        take_descent_speed_limit: bool,
        schedule: &DecelerationSchedule,
        last: SubPhase,
    ) -> TemporaryCheckpointSequence {
        let (alts, speeds) = Self::constraints(profile, schedule);
        let mut table = PhaseTable { params: self.params, start: *profile.last(), phases: vec![last] };
        let mut valid = false;
        let mut n = 0;
        let mut sequence = None;
        while !valid && n < 100 {
            n += 1;
            sequence = Some(table.execute(descent, &self.level_flight, profile));
            valid = self.check_for_violations(&mut table, &alts, &speeds, take_descent_speed_limit, schedule);
        }
        *profile.last_mut() = table.start;
        sequence.expect("tactical path executed at least once")
    }

    pub fn build_mcdu_prediction_path(&self, profile: &mut Profile, descent: &dyn DescentStrategy, take_descent_speed_limit: bool, schedule: &DecelerationSchedule) {
        let last = SubPhase::to_altitude(profile.final_altitude).after(Reason::Landing);
        let sequence = self.run(profile, descent, take_descent_speed_limit, schedule, last);
        profile.checkpoints.extend_from_slice(sequence.get());
    }

    pub fn build_tactical_descent_path_to_altitude(
        &self,
        profile: &mut Profile,
        descent: &dyn DescentStrategy,
        take_descent_speed_limit: bool,
        final_altitude: f64,
        schedule: &DecelerationSchedule,
    ) {
        let last = SubPhase::to_altitude(final_altitude).after(Reason::CrossingFcuAltitudeDescent);
        let mut sequence = self.run(profile, descent, take_descent_speed_limit, schedule, last);
        if sequence.last().reason == Reason::AtmosphericConditions {
            sequence.last_mut().reason = Reason::CrossingFcuAltitudeDescent;
            if sequence.len() == 1 {
                profile.last_mut().reason = Reason::CrossingFcuAltitudeDescent;
            }
        }
        profile.checkpoints.extend_from_slice(sequence.get());
    }

    fn check_for_violations(
        &self,
        table: &mut PhaseTable,
        alts: &[MinimumDescentAltitudeConstraint],
        speeds: &[MaxSpeedConstraint],
        take_descent_speed_limit: bool,
        schedule: &DecelerationSchedule,
    ) -> bool {
        let mut previous = table.start;
        for i in 0..table.phases.len() {
            let phase = table.phases[i];
            let Some(result) = phase.last_result else { continue };
            for ap in &schedule.approach_points {
                if violates_approach_point(&phase, &result, ap) {
                    handle_approach_point(table, i, ap);
                    return false;
                }
            }
            for sc in &schedule.speed_changes {
                if violates_speed_change(&phase, &result, sc) {
                    if sc.reason == Reason::StartDecelerationToConstraint {
                        handle_constraint_speed_change(table, i, sc);
                    } else if sc.reason == Reason::StartDecelerationToLimit {
                        handle_limit_speed_change(table, i, sc);
                    }
                    return false;
                }
            }
            for c in speeds {
                if violates_speed_constraint(&previous, &phase, &result, c) {
                    handle_speed_constraint(table, i, c);
                    return false;
                }
            }
            for c in alts {
                if violates_altitude_constraint(&previous, &phase, &result, c) {
                    handle_altitude_constraint(table, i, c);
                    return false;
                }
            }
            if take_descent_speed_limit {
                if let Some(limit) = self.params.descent_speed_limit {
                    if violates_speed_limit(&previous, &phase, &result, &limit) {
                        handle_speed_limit(table, i, &limit);
                        return false;
                    }
                }
            }
            previous = result;
        }
        true
    }
}

fn speed_change_target(c: &Checkpoint) -> f64 {
    c.target_speed.unwrap_or(f64::NAN)
}

fn handle_constraint_speed_change(t: &mut PhaseTable, i: usize, sc: &Checkpoint) {
    let target = speed_change_target(sc);
    if let Some(to) = t.phases[i].deceleration_to_speed() {
        t.phases[i].set_to_speed(js_min(target, to));
    } else {
        t.phases.splice(i..i, [SubPhase::to_distance(sc.distance_from_start), SubPhase::deceleration(target).before(sc.reason)]);
    }
}

fn handle_limit_speed_change(t: &mut PhaseTable, i: usize, sc: &Checkpoint) {
    let target = speed_change_target(sc);
    if let Some(to) = t.phases[i].deceleration_to_speed() {
        t.phases[i].set_to_speed(js_min(target, to));
        let overshoot = t.phases[i].last_result.map_or(f64::NAN, |r| r.altitude) - sc.altitude;
        if i >= 1 {
            let j = i - 1;
            match t.phases[j].kind {
                Kind::ToAltitude { to_altitude } => t.phases[j].kind = Kind::ToAltitude { to_altitude: to_altitude - overshoot },
                Kind::ToDistance { .. } => {
                    let alt = t.phases[j].last_result.map_or(f64::NAN, |r| r.altitude);
                    t.phases[j] = SubPhase::to_altitude(alt - overshoot);
                }
                Kind::Deceleration { .. } => {}
            }
        }
    } else {
        t.phases.splice(i..i, [SubPhase::to_altitude(sc.altitude), SubPhase::deceleration(target).before(Reason::StartDecelerationToLimit)]);
    }
}

fn handle_speed_constraint(t: &mut PhaseTable, i: usize, c: &MaxSpeedConstraint) {
    if let Some(to) = t.phases[i].deceleration_to_speed() {
        t.phases[i].set_to_speed(js_min(c.max_speed, to));
        let overshoot = t.phases[i].last_result.map_or(f64::NAN, |r| r.distance_from_start) - c.distance_from_start;
        for j in (0..i).rev() {
            let Some(prev) = t.phases[j].last_result else { continue };
            match t.phases[j].kind {
                Kind::ToAltitude { .. } => {
                    t.phases[j] = SubPhase::to_distance(prev.distance_from_start - overshoot);
                    return;
                }
                Kind::ToDistance { to_distance, min_altitude } => {
                    t.phases[j].kind = Kind::ToDistance { to_distance: to_distance - overshoot, min_altitude };
                    return;
                }
                Kind::Deceleration { .. } => {}
            }
        }
    } else {
        t.phases.splice(
            i..i,
            [SubPhase::to_distance(c.distance_from_start - 3.0), SubPhase::deceleration(c.max_speed).before(Reason::StartDecelerationToConstraint)],
        );
    }
}

fn handle_altitude_constraint(t: &mut PhaseTable, i: usize, c: &MinimumDescentAltitudeConstraint) {
    let phase = t.phases[i];
    match phase.kind {
        Kind::Deceleration { to_speed, .. } => {
            t.phases.splice(
                i..i,
                [
                    SubPhase::deceleration(to_speed)
                        .before(phase.reason_before)
                        .with_min_altitude(c.minimum_altitude)
                        .after(Reason::LevelOffForDescentConstraint),
                    SubPhase::deceleration(to_speed).level().with_max_distance(c.distance_from_start),
                ],
            );
            t.phases[i + 2].reason_before = Reason::AtmosphericConditions;
        }
        Kind::ToAltitude { .. } => {
            t.phases.splice(
                i..i,
                [
                    SubPhase::to_altitude(c.minimum_altitude).after(Reason::LevelOffForDescentConstraint),
                    SubPhase::to_distance(c.distance_from_start).level(),
                ],
            );
        }
        Kind::ToDistance { to_distance, .. } => {
            if c.distance_from_start < to_distance {
                t.phases.splice(
                    i..i,
                    [
                        SubPhase::to_altitude(c.minimum_altitude).after(Reason::LevelOffForDescentConstraint),
                        SubPhase::to_distance(c.distance_from_start).level(),
                    ],
                );
            } else {
                t.phases.splice(
                    i..i + 1,
                    [SubPhase::to_altitude(c.minimum_altitude).after(Reason::LevelOffForDescentConstraint), SubPhase::to_distance(to_distance).level()],
                );
            }
        }
    }
}

fn handle_speed_limit(t: &mut PhaseTable, i: usize, limit: &SpeedLimit) {
    if let Some(to) = t.phases[i].deceleration_to_speed() {
        t.phases[i].set_to_speed(js_min(limit.speed, to));
        let overshoot = t.phases[i].last_result.map_or(f64::NAN, |r| r.altitude) - limit.under_altitude;
        for j in (0..i).rev() {
            let Some(prev) = t.phases[j].last_result else { continue };
            match t.phases[j].kind {
                Kind::ToAltitude { to_altitude } => t.phases[j].kind = Kind::ToAltitude { to_altitude: to_altitude - overshoot },
                Kind::ToDistance { .. } => t.phases[i] = SubPhase::to_altitude(prev.altitude - overshoot),
                Kind::Deceleration { .. } => {}
            }
            return;
        }
    } else {
        t.phases.splice(
            i..i,
            [SubPhase::to_altitude(limit.under_altitude + 100.0), SubPhase::deceleration(limit.speed).before(Reason::StartDecelerationToLimit)],
        );
    }
}

fn handle_approach_point(t: &mut PhaseTable, i: usize, ap: &Checkpoint) {
    let target = speed_change_target(ap);
    if let Some(to) = t.phases[i].deceleration_to_speed() {
        t.phases[i].set_to_speed(js_min(target, to));
    } else {
        t.phases.splice(i..i, [SubPhase::to_distance(ap.distance_from_start), SubPhase::deceleration(target)]);
    }
}

fn violates_approach_point(phase: &SubPhase, result: &Checkpoint, ap: &Checkpoint) -> bool {
    if result.distance_from_start <= ap.distance_from_start {
        return false;
    }
    let target = speed_change_target(ap);
    match phase.deceleration_to_speed() {
        Some(to) => to > target,
        None => result.speed > target,
    }
}

fn violates_speed_change(phase: &SubPhase, result: &Checkpoint, sc: &Checkpoint) -> bool {
    if sc.reason == Reason::StartDecelerationToLimit {
        if result.altitude >= sc.altitude {
            return false;
        }
    } else if result.distance_from_start <= sc.distance_from_start {
        return false;
    }
    let target = speed_change_target(sc);
    match phase.deceleration_to_speed() {
        Some(to) => to > target,
        None => result.speed > target,
    }
}

fn violates_speed_constraint(previous: &Checkpoint, phase: &SubPhase, result: &Checkpoint, c: &MaxSpeedConstraint) -> bool {
    if result.distance_from_start < c.distance_from_start {
        return false;
    }
    if previous.distance_from_start > c.distance_from_start {
        return match phase.deceleration_to_speed() {
            Some(to) => to > c.max_speed,
            None => previous.speed > c.max_speed,
        };
    }
    let per_distance = (previous.speed - result.speed) / (previous.distance_from_start - result.distance_from_start);
    let at_constraint = result.speed + per_distance * (c.distance_from_start - result.distance_from_start);
    at_constraint - c.max_speed > 1.0
}

fn violates_altitude_constraint(previous: &Checkpoint, phase: &SubPhase, result: &Checkpoint, c: &MinimumDescentAltitudeConstraint) -> bool {
    if result.altitude - c.minimum_altitude >= -1.0
        || previous.altitude - c.minimum_altitude < -100.0
        || previous.distance_from_start >= c.distance_from_start
        || phase.level
    {
        return false;
    }
    let gradient = (previous.altitude - result.altitude) / (previous.distance_from_start - result.distance_from_start);
    let at_constraint = previous.altitude + gradient * (c.distance_from_start - previous.distance_from_start);
    at_constraint < c.minimum_altitude
}

fn violates_speed_limit(previous: &Checkpoint, phase: &SubPhase, result: &Checkpoint, limit: &SpeedLimit) -> bool {
    if result.altitude > limit.under_altitude {
        return false;
    }
    if previous.altitude < limit.under_altitude {
        return match phase.deceleration_to_speed() {
            Some(to) => to > limit.speed,
            None => previous.speed > limit.speed,
        };
    }
    let per_altitude = (previous.speed - result.speed) / (previous.altitude - result.altitude);
    let at_limit = result.speed + per_altitude * (limit.under_altitude - result.altitude);
    at_limit - limit.speed > 1.0
}

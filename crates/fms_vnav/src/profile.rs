// Translated from FlyByWire's fmgc guidance/vnav/profile/BaseGeometryProfile.ts, NavGeometryProfile.ts (types)
// and VerticalProfileComputationParameters.ts (GPL-3.0-or-later).

use crate::atmosphere::{interpolate, Atmosphere};
use crate::config::{AircraftConfig, FlapConf};
use crate::predictions::StepResults;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reason {
    Liftoff,
    ThrustReductionAltitude,
    AccelerationAltitude,
    TopOfClimb,
    AtmosphericConditions,
    PresentPosition,
    LevelOffForClimbConstraint,
    AltitudeConstraint,
    ContinueClimb,
    CrossingClimbSpeedLimit,
    SpeedConstraint,
    CrossingFcuAltitudeClimb,
    StepClimb,
    TopOfStepClimb,
    StepDescent,
    BottomOfStepDescent,
    CrossingFcuAltitudeDescent,
    InterceptDescentProfileManaged,
    InterceptDescentProfileSelected,
    LevelOffForDescentConstraint,
    ContinueDescent,
    ContinueDescentArmed,
    TopOfDescent,
    CrossingDescentSpeedLimit,
    IdlePathAtmosphericConditions,
    IdlePathEnd,
    GeometricPathStart,
    GeometricPathConstraint,
    GeometricPathTooSteep,
    GeometricPathEnd,
    StartDecelerationToConstraint,
    StartDecelerationToLimit,
    Decel,
    Flaps1,
    Flaps2,
    Flaps3,
    FlapsFull,
    Landing,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProfilePhase {
    Climb,
    Cruise,
    Descent,
}

#[derive(Clone, Copy, Debug)]
pub struct Checkpoint {
    pub reason: Reason,
    pub distance_from_start: f64,
    pub seconds_from_present: f64,
    pub altitude: f64,
    pub remaining_fuel_on_board: f64,
    pub speed: f64,
    pub mach: f64,
    pub profile_phase: ProfilePhase,
    pub target_speed: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
pub struct MaxSpeedConstraint {
    pub distance_from_start: f64,
    pub max_speed: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct MaxAltitudeConstraint {
    pub distance_from_start: f64,
    pub max_altitude: f64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AltitudeDescriptor {
    None,
    AtAlt1,
    AtOrAboveAlt1,
    AtOrBelowAlt1,
    BetweenAlt1Alt2,
    AtOrAboveAlt2,
    AtAlt1GsMslAlt2,
    AtOrAboveAlt1GsMslAlt2,
    AtAlt1GsIntcptAlt2,
    AtOrAboveAlt1GsIntcptAlt2,
    AtOrAboveAlt1AngleAlt2,
    AtAlt1AngleAlt2,
    AtOrBelowAlt1AngleAlt2,
}

#[derive(Clone, Copy, Debug)]
pub struct AltitudeConstraint {
    pub descriptor: AltitudeDescriptor,
    pub altitude1: f64,
    pub altitude2: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct DescentAltitudeConstraint {
    pub distance_from_start: f64,
    pub constraint: AltitudeConstraint,
    pub leg: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct GeographicCruiseStep {
    pub distance_from_start: f64,
    pub to_altitude: f64,
    pub waypoint_index: usize,
    pub is_ignored: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct SpeedLimit {
    pub speed: f64,
    pub under_altitude: f64,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum FlightPhase {
    Preflight,
    Takeoff,
    Climb,
    Cruise,
    Descent,
    Approach,
    GoAround,
    Done,
}

pub trait Winds {
    fn climb_tailwind(&self, distance_from_start: f64, altitude: f64) -> f64;
    fn cruise_tailwind(&self, distance_from_start: f64, distance_from_present: f64, altitude: f64) -> f64;
    fn descent_tailwind(&self, distance_from_start: f64, altitude: f64) -> f64;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ConstantWinds {
    pub tailwind: f64,
}

impl Winds for ConstantWinds {
    fn climb_tailwind(&self, _: f64, _: f64) -> f64 {
        self.tailwind
    }
    fn cruise_tailwind(&self, _: f64, _: f64, _: f64) -> f64 {
        self.tailwind
    }
    fn descent_tailwind(&self, _: f64, _: f64) -> f64 {
        self.tailwind
    }
}

#[derive(Clone, Debug)]
pub struct ComputationParameters {
    pub present_altitude: f64,
    pub fcu_altitude: f64,
    pub fcu_speed: f64,
    pub managed_climb_speed: f64,
    pub managed_climb_speed_mach: f64,
    pub managed_cruise_speed: f64,
    pub managed_cruise_speed_mach: f64,
    pub managed_descent_speed: f64,
    pub managed_descent_speed_mach: f64,
    pub zero_fuel_weight: f64,
    pub fuel_on_board: f64,
    pub v2_speed: f64,
    pub tropopause: f64,
    pub perf_factor: f64,
    pub departure_elevation: f64,
    pub destination_elevation: f64,
    pub acceleration_altitude: f64,
    pub thrust_reduction_altitude: f64,
    pub cruise_altitude: f64,
    pub climb_speed_limit: Option<SpeedLimit>,
    pub descent_speed_limit: Option<SpeedLimit>,
    pub flight_phase: FlightPhase,
    pub preselected_clb_speed: f64,
    pub preselected_cruise_speed: f64,
    pub takeoff_flaps: FlapConf,
    pub takeoff_toga_n1: f64,
    pub isa_deviation: f64,
    pub approach_speed: f64,
    pub flap_retraction_speed: f64,
    pub slat_retraction_speed: f64,
    pub clean_speed: f64,
    pub gross_weight_valid: bool,
}

impl ComputationParameters {
    pub fn atmosphere(&self) -> Atmosphere {
        Atmosphere::new(self.isa_deviation, self.tropopause)
    }

    pub fn can_compute_profile(&self) -> bool {
        let approach_speeds_valid = self.clean_speed > 100.0
            && self.slat_retraction_speed > 100.0
            && self.flap_retraction_speed > 100.0
            && self.approach_speed > 100.0;
        let has_takeoff = self.thrust_reduction_altitude > 0.0 && self.acceleration_altitude > 0.0;
        (self.flight_phase > FlightPhase::Takeoff || has_takeoff)
            && approach_speeds_valid
            && self.zero_fuel_weight.is_finite()
            && self.gross_weight_valid
            && self.cruise_altitude.is_finite()
            && self.fuel_on_board.is_finite()
    }
}

pub struct Profile<'a> {
    pub config: &'a AircraftConfig,
    pub checkpoints: Vec<Checkpoint>,
    pub max_altitude_constraints: Vec<MaxAltitudeConstraint>,
    pub max_climb_speed_constraints: Vec<MaxSpeedConstraint>,
    pub descent_speed_constraints: Vec<MaxSpeedConstraint>,
    pub descent_altitude_constraints: Vec<DescentAltitudeConstraint>,
    pub distance_to_present_position: f64,
    pub winds: Box<dyn Winds + 'a>,
    pub cruise_steps: Vec<GeographicCruiseStep>,
    pub ignored_cruise_step_waypoints: Vec<usize>,
    pub total_flight_plan_distance: f64,
    pub final_descent_angle: f64,
    pub faf_distance_to_end: f64,
    pub final_altitude: f64,
    pub is_ready_to_display: bool,
}

impl<'a> Profile<'a> {
    pub fn new(config: &'a AircraftConfig, winds: Box<dyn Winds + 'a>) -> Self {
        Self {
            config,
            checkpoints: Vec::new(),
            max_altitude_constraints: Vec::new(),
            max_climb_speed_constraints: Vec::new(),
            descent_speed_constraints: Vec::new(),
            descent_altitude_constraints: Vec::new(),
            distance_to_present_position: 0.0,
            winds,
            cruise_steps: Vec::new(),
            ignored_cruise_step_waypoints: Vec::new(),
            total_flight_plan_distance: 0.0,
            final_descent_angle: -3.0,
            faf_distance_to_end: default_faf_distance_to_end(-3.0),
            final_altitude: 50.0,
            is_ready_to_display: false,
        }
    }

    pub fn get_distance_from_start(&self, distance_from_end: f64) -> f64 {
        self.total_flight_plan_distance - distance_from_end
    }

    pub fn finalize_profile(&mut self) {
        self.checkpoints.sort_by(|a, b| a.distance_from_start.partial_cmp(&b.distance_from_start).unwrap_or(std::cmp::Ordering::Equal));
        self.is_ready_to_display = true;
    }

    pub fn invalidate(&mut self) {
        self.is_ready_to_display = false;
    }

    pub fn find_vertical_checkpoint(&self, reasons: &[Reason]) -> Option<&Checkpoint> {
        self.checkpoints.iter().find(|c| reasons.contains(&c.reason))
    }

    pub fn find_last_checkpoint_index(&self, reasons: &[Reason]) -> Option<usize> {
        self.checkpoints.iter().rposition(|c| reasons.contains(&c.reason))
    }

    pub fn ignore_cruise_step(&mut self, index: usize) {
        let step = &mut self.cruise_steps[index];
        step.is_ignored = true;
        self.ignored_cruise_step_waypoints.push(step.waypoint_index);
    }

    pub fn last(&self) -> &Checkpoint {
        self.checkpoints.last().expect("profile has no checkpoints")
    }

    pub fn last_mut(&mut self) -> &mut Checkpoint {
        self.checkpoints.last_mut().expect("profile has no checkpoints")
    }

    pub fn add_checkpoint_from_step(&mut self, step: &StepResults, reason: Reason, mach: f64, phase: ProfilePhase) {
        let l = *self.last();
        self.checkpoints.push(Checkpoint {
            reason,
            distance_from_start: l.distance_from_start + step.distance_traveled,
            altitude: step.final_altitude,
            seconds_from_present: l.seconds_from_present + step.time_elapsed,
            speed: step.speed,
            remaining_fuel_on_board: l.remaining_fuel_on_board - step.fuel_burned,
            mach,
            profile_phase: phase,
            target_speed: None,
        });
    }

    fn interp_from<F: Fn(&Checkpoint) -> f64, G: Fn(&Checkpoint) -> f64>(&self, index_value: f64, key: F, value: G) -> f64 {
        let c = &self.checkpoints;
        if index_value <= key(&c[0]) {
            return value(&c[0]);
        }
        for i in 0..c.len() - 1 {
            if index_value > key(&c[i]) && index_value <= key(&c[i + 1]) {
                return interpolate(index_value, key(&c[i]), key(&c[i + 1]), value(&c[i]), value(&c[i + 1]));
            }
        }
        value(&c[c.len() - 1])
    }

    pub fn interpolate_time_at_distance(&self, d: f64) -> f64 {
        self.interp_from(d, |c| c.distance_from_start, |c| c.seconds_from_present)
    }

    pub fn interpolate_altitude_at_distance(&self, d: f64) -> f64 {
        self.interp_from(d, |c| c.distance_from_start, |c| c.altitude)
    }

    pub fn interpolate_speed_at_distance(&self, d: f64) -> f64 {
        self.interp_from(d, |c| c.distance_from_start, |c| c.speed)
    }

    pub fn interpolate_fuel_at_distance(&self, d: f64) -> f64 {
        self.interp_from(d, |c| c.distance_from_start, |c| c.remaining_fuel_on_board)
    }

    pub fn interpolate_distance_at_time(&self, s: f64) -> f64 {
        self.interp_from(s, |c| c.seconds_from_present, |c| c.distance_from_start)
    }

    pub fn interpolate_distance_at_altitude(&self, alt: f64) -> f64 {
        self.interp_from(alt, |c| c.altitude, |c| c.distance_from_start)
    }

    pub fn interpolate_path_angle_at_distance(&self, d: f64) -> f64 {
        let c = &self.checkpoints;
        if d < c[0].distance_from_start {
            return 0.0;
        }
        for i in 0..c.len() - 1 {
            if d > c[i].distance_from_start && d <= c[i + 1].distance_from_start {
                return ((c[i + 1].altitude - c[i].altitude) / (c[i + 1].distance_from_start - c[i].distance_from_start) / 6076.12)
                    .atan()
                    .to_degrees();
            }
        }
        0.0
    }

    pub fn interpolate_everything_from_start(&self, d: f64, interpolate_altitude: bool) -> Checkpoint {
        let c = &self.checkpoints;
        let l = |f: fn(&Checkpoint) -> f64, i: usize| interpolate(d, c[i].distance_from_start, c[i + 1].distance_from_start, f(&c[i]), f(&c[i + 1]));
        if d <= c[0].distance_from_start {
            return Checkpoint { distance_from_start: d, target_speed: None, ..c[0] };
        }
        for i in 0..c.len() - 1 {
            if d > c[i].distance_from_start && d <= c[i + 1].distance_from_start {
                return Checkpoint {
                    reason: c[i].reason,
                    distance_from_start: d,
                    seconds_from_present: l(|x| x.seconds_from_present, i),
                    altitude: if interpolate_altitude { l(|x| x.altitude, i) } else { c[i].altitude },
                    remaining_fuel_on_board: self
                        .config
                        .vnav
                        .lowest_fuel_estimate
                        .max(l(|x| x.remaining_fuel_on_board, i)),
                    speed: l(|x| x.speed, i),
                    mach: l(|x| x.mach, i),
                    profile_phase: choose_phase(c[i].profile_phase, c[i + 1].profile_phase),
                    target_speed: None,
                };
            }
        }
        Checkpoint { distance_from_start: d, target_speed: None, ..c[c.len() - 1] }
    }

    pub fn add_interpolated_checkpoint(&mut self, d: f64, reason: Reason) -> usize {
        if d <= self.checkpoints[0].distance_from_start {
            let first = self.checkpoints[0];
            self.checkpoints.insert(0, Checkpoint { distance_from_start: d, reason, target_speed: None, ..first });
            return 0;
        }
        let n = self.checkpoints.len();
        for i in 0..n - 1 {
            if d > self.checkpoints[i].distance_from_start && d <= self.checkpoints[i + 1].distance_from_start {
                let mut cp = self.interpolate_everything_from_start(d, true);
                cp.reason = reason;
                self.checkpoints.insert(i + 1, cp);
                return i + 1;
            }
        }
        let last = *self.last();
        self.checkpoints.push(Checkpoint { distance_from_start: d, reason, target_speed: None, ..last });
        self.checkpoints.len() - 1
    }
}

pub fn default_faf_distance_to_end(final_descent_angle: f64) -> f64 {
    1000.0 / (-final_descent_angle).to_radians().tan() / 6076.12
}

fn choose_phase(a: ProfilePhase, b: ProfilePhase) -> ProfilePhase {
    if a != ProfilePhase::Descent && b == ProfilePhase::Descent {
        ProfilePhase::Cruise
    } else {
        b
    }
}

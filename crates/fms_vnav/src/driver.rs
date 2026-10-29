// Translated from FlyByWire's fmgc guidance/vnav/VnavDriver.ts (GPL-3.0-or-later). The profile manager's state lives
// here too (mcdu, descent, ND and expedite profiles). SimVar reads arrive in the input structs, writes are kept in
// `DriverOutputs` and the descent guidance's `GuidanceOutputs`.

use crate::atmosphere::Atmosphere;
use crate::config::AircraftConfig;
use crate::constraints::{ConstraintPlan, ConstraintReader};
use crate::coordinator::CruiseToDescentCoordinator;
use crate::guidance::{AircraftToDescentProfileRelation, AnyDescentGuidance, GuidanceInputs};
use crate::manager::{compute_descent_path, compute_tactical_mcdu_path, McduPathError, PresentState, TacticalInputs};
use crate::modes::{lateral, FcuModes};
use crate::nd::{compute_expedite_climb_profile, compute_tactical_nd_profile, NdInputs};
use crate::profile::{Checkpoint, ComputationParameters, FlightPhase, Profile, Reason, SpeedLimit, Winds};
use crate::speed_profile::{ManagedSpeedType, McduSpeedProfile};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WindsFor {
    Mcdu,
    Descent,
    NdManaged,
    NdSelected,
    Expedite,
}

/// Everything `recompute` reads besides the computation parameters.
pub struct RecomputeInputs<'b, 'a> {
    pub modes: FcuModes,
    pub plan: &'b ConstraintPlan,
    pub distance_to_end: f64,
    /// Geometry legs as (index, has a path); an empty list is FBW's `geometry.legs.size <= 0`.
    pub legs: &'b [(usize, bool)],
    pub active_leg_index: usize,
    pub has_temporary: bool,
    pub present: PresentState,
    pub green_dot_speed: f64,
    pub approach_qnh: f64,
    pub approach_temperature: f64,
    pub estimated_destination_fuel: Option<f64>,
    pub winds: &'b dyn Fn(WindsFor) -> Box<dyn Winds + 'a>,
}

/// Everything `update` reads from the sim and guidance controller.
pub struct UpdateInputs<'f> {
    pub guidance: GuidanceInputs<'f>,
    pub distance_to_end: f64,
    pub manual_hold_next: bool,
    pub hold_speed: f64,
    pub hold_decel_reached: bool,
    pub autopilot_airspeed_selected: bool,
    pub autopilot_mach_mode_active: bool,
    pub autopilot_mach_hold_value: f64,
    pub autopilot_airspeed_hold_value: f64,
}

#[derive(Clone, Debug, Default)]
pub struct DriverOutputs {
    pub fm_vertical_profile_avail: bool,
    pub speeds_managed_pfd: Option<f64>,
    pub hold_speed_tas: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
pub struct PerfCrzToPrediction {
    pub reason: Reason,
    pub distance_from_present_position: f64,
    pub seconds_from_present: f64,
}

#[derive(Clone, Debug)]
struct LastParameters {
    cruise_altitude: f64,
    managed_descent_speed: f64,
    managed_descent_speed_mach: f64,
    approach_qnh: f64,
    approach_temperature: f64,
    descent_speed_limit: Option<SpeedLimit>,
}

/// `numberOrNanChanged`, with `None` standing for `undefined`.
pub fn number_or_nan_changed(old: Option<f64>, new: Option<f64>) -> bool {
    match (old, new) {
        (Some(a), Some(b)) => (!a.is_nan() || !b.is_nan()) && a != b,
        (None, None) => false,
        _ => true,
    }
}

fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

fn is_speed_change_point(c: &Checkpoint) -> bool {
    c.reason == Reason::StartDecelerationToConstraint || c.reason == Reason::StartDecelerationToLimit
}

fn is_close_to_greater_than(a: f64, b: f64) -> bool {
    a - b > -1e-4
}

fn is_close_to_less_than(a: f64, b: f64) -> bool {
    a - b < 1e-4
}

pub struct VnavDriver<'a> {
    pub version: u64,
    config: &'a AircraftConfig,
    pub constraint_reader: ConstraintReader,
    pub relation: AircraftToDescentProfileRelation,
    pub descent_guidance: AnyDescentGuidance,
    coordinator: CruiseToDescentCoordinator,
    pub mcdu_profile: Option<Profile<'a>>,
    pub descent_profile: Option<Profile<'a>>,
    pub nd_profile: Option<Profile<'a>>,
    pub expedite_profile: Option<Profile<'a>>,
    pub too_steep_path_legs: Vec<usize>,
    pub last_error: Option<McduPathError>,
    decel_point: Option<Checkpoint>,
    previous_managed_descent_speed_target: f64,
    last_parameters: Option<LastParameters>,
    old_legs: Vec<(usize, bool)>,
    request_descent_profile_recomputation: bool,
    prev_mcdu_pred_ready_to_display: bool,
    pub out: DriverOutputs,
}

impl<'a> VnavDriver<'a> {
    pub fn new(config: &'a AircraftConfig) -> Self {
        Self {
            version: 0,
            config,
            constraint_reader: ConstraintReader::default(),
            relation: AircraftToDescentProfileRelation::default(),
            descent_guidance: AnyDescentGuidance::new(config, config.vnav.use_latched_descent_mode),
            coordinator: CruiseToDescentCoordinator::default(),
            mcdu_profile: None,
            descent_profile: None,
            nd_profile: None,
            expedite_profile: None,
            too_steep_path_legs: Vec::new(),
            last_error: None,
            decel_point: None,
            previous_managed_descent_speed_target: f64::NAN,
            last_parameters: None,
            old_legs: Vec::new(),
            request_descent_profile_recomputation: false,
            prev_mcdu_pred_ready_to_display: false,
            out: DriverOutputs::default(),
        }
    }

    pub fn coordinator(&self) -> &CruiseToDescentCoordinator {
        &self.coordinator
    }

    pub fn update(&mut self, dt_ms: f64, p: &ComputationParameters, i: &UpdateInputs) {
        if p.flight_phase < FlightPhase::Takeoff {
            return;
        }
        self.update_hold_speed(p, i);
        self.update_descent_speed_guidance(p, &i.guidance, i.manual_hold_next);
        let pfd = self.out.speeds_managed_pfd.unwrap_or(i.guidance.speeds_managed_pfd);
        let g = GuidanceInputs { speeds_managed_pfd: pfd, ..i.guidance };
        self.descent_guidance.update(dt_ms, i.distance_to_end, &mut self.relation, self.descent_profile.as_ref(), p, &g);
    }

    pub fn recompute(&mut self, p: &ComputationParameters, i: &RecomputeInputs<'_, 'a>) {
        self.constraint_reader.update_flight_plan(i.plan, i.distance_to_end);
        if i.legs.is_empty() || !p.can_compute_profile() {
            self.reset();
            return;
        }

        if p.flight_phase != FlightPhase::GoAround {
            self.compute_tactical_mcdu_path(p, i);
        } else if let Some(m) = self.mcdu_profile.as_mut().filter(|m| m.is_ready_to_display) {
            m.invalidate();
        }

        if self.should_update_descent_profile(p, i) || self.request_descent_profile_recomputation {
            self.old_legs = i.legs.to_vec();
            self.last_parameters = Some(LastParameters {
                cruise_altitude: p.cruise_altitude,
                managed_descent_speed: p.managed_descent_speed,
                managed_descent_speed_mach: p.managed_descent_speed_mach,
                approach_qnh: i.approach_qnh,
                approach_temperature: i.approach_temperature,
                descent_speed_limit: p.descent_speed_limit,
            });
            self.request_descent_profile_recomputation = false;
            self.previous_managed_descent_speed_target = p.managed_descent_speed;

            let path = compute_descent_path(p, self.config, &self.constraint_reader, (i.winds)(WindsFor::Descent), i.estimated_destination_fuel);
            self.descent_profile = Some(path.profile);
            self.descent_guidance.update_profile(&mut self.relation, self.descent_profile.as_ref());
            self.decel_point = self.descent_profile.as_ref().and_then(|d| d.find_vertical_checkpoint(&[Reason::Decel]).copied());
        }

        let ready = self.mcdu_profile.as_ref().is_some_and(|m| m.is_ready_to_display);
        if ready != self.prev_mcdu_pred_ready_to_display {
            self.out.fm_vertical_profile_avail = ready;
            self.prev_mcdu_pred_ready_to_display = ready;
        }

        let relation = self.relation.snapshot(self.descent_profile.as_ref(), p);
        let nd_winds = (i.winds)(if i.modes.is_lat_auto_control_active() { WindsFor::NdManaged } else { WindsFor::NdSelected });
        let nd_inputs = NdInputs { modes: i.modes, present: i.present, relation, descent_profile: self.descent_profile.as_ref() };
        self.nd_profile = Some(compute_tactical_nd_profile(p, self.config, &self.constraint_reader, nd_winds, &nd_inputs));
        if let Some(e) = compute_expedite_climb_profile(p, self.config, (i.winds)(WindsFor::Expedite), &i.modes, i.present, i.green_dot_speed) {
            self.expedite_profile = Some(e);
        }

        self.version += 1;
    }

    fn compute_tactical_mcdu_path(&mut self, p: &ComputationParameters, i: &RecomputeInputs<'_, 'a>) {
        let relation = self.relation.snapshot(self.descent_profile.as_ref(), p);
        let tactical = TacticalInputs { modes: i.modes, present: i.present, relation, descent_profile: self.descent_profile.as_ref() };
        let winds = (i.winds)(WindsFor::Mcdu);
        match compute_tactical_mcdu_path(p, self.config, &self.constraint_reader, winds, &mut self.coordinator, i.estimated_destination_fuel, &tactical) {
            Ok(path) => {
                self.too_steep_path_legs = path.too_steep_path_legs;
                self.mcdu_profile = Some(path.profile);
                self.last_error = None;
            }
            Err(e) => self.last_error = Some(e),
        }
    }

    fn reset(&mut self) {
        if self.version == 0 {
            return;
        }
        self.version = 0;
        self.mcdu_profile = None;
        self.descent_profile = None;
        self.nd_profile = None;
        self.expedite_profile = None;
        let distance_to_end = self.constraint_reader.distance_to_end;
        self.constraint_reader = ConstraintReader { distance_to_end, ..ConstraintReader::default() };
        self.relation.reset();
        self.descent_guidance.reset();
        self.decel_point = None;
        self.last_parameters = None;
        self.old_legs.clear();
        self.previous_managed_descent_speed_target = f64::NAN;
        self.out.fm_vertical_profile_avail = false;
        self.prev_mcdu_pred_ready_to_display = false;
    }

    fn should_update_descent_profile(&self, p: &ComputationParameters, i: &RecomputeInputs) -> bool {
        let Some(last) = &self.last_parameters else { return true };
        let lim = |l: &Option<SpeedLimit>| (l.map(|l| l.speed), l.map(|l| l.under_altitude));
        let (old_speed, old_alt) = lim(&last.descent_speed_limit);
        let (new_speed, new_alt) = lim(&p.descent_speed_limit);
        p.flight_phase < FlightPhase::Descent
            || p.flight_phase > FlightPhase::Approach
            || (!i.has_temporary && self.did_legs_change(i.legs, i.active_leg_index))
            || number_or_nan_changed(Some(last.cruise_altitude), Some(p.cruise_altitude))
            || number_or_nan_changed(Some(last.managed_descent_speed), Some(p.managed_descent_speed))
            || number_or_nan_changed(Some(last.managed_descent_speed_mach), Some(p.managed_descent_speed_mach))
            || number_or_nan_changed(Some(last.approach_qnh), Some(i.approach_qnh))
            || number_or_nan_changed(Some(last.approach_temperature), Some(i.approach_temperature))
            || number_or_nan_changed(old_speed, new_speed)
            || number_or_nan_changed(old_alt, new_alt)
    }

    fn did_legs_change(&self, new_legs: &[(usize, bool)], active_leg_index: usize) -> bool {
        new_legs.iter().filter(|(index, _)| *index >= active_leg_index).any(|(index, has_repr)| {
            let old = self.old_legs.iter().find(|(i, _)| i == index).map_or(false, |(_, r)| *r);
            *has_repr != old
        })
    }

    pub fn linear_deviation(&self, present_altitude: f64) -> Option<f64> {
        let profile = self.descent_profile.as_ref().filter(|_| self.relation.is_valid)?;
        Some(self.relation.compute_linear_deviation(profile, present_altitude))
    }

    fn update_descent_speed_guidance(&mut self, p: &ComputationParameters, g: &GuidanceInputs, manual_hold_next: bool) {
        let Some(nd) = self.nd_profile.as_ref().filter(|n| n.is_ready_to_display) else { return };
        let hold_active = g.manual_hold_active || manual_hold_next;
        let lat_auto = g.modes.driver_lat_auto_control_active();
        let current_distance = if lat_auto { self.constraint_reader.distance_to_present_position() } else { 0.0 };
        let current_altitude = p.present_altitude;
        if p.flight_phase != FlightPhase::Descent || g.modes.expedite || hold_active {
            return;
        }
        let decel_speed = self.decel_point.map_or(0.0, |d| d.speed);
        let mut target = js_min(p.managed_descent_speed, self.previous_managed_descent_speed_target);
        if lat_auto {
            let sp = McduSpeedProfile::new(p, self.constraint_reader.distance_to_present_position(), Vec::new(), Vec::new());
            target = js_min(target, sp.managed_target(current_distance, current_altitude, ManagedSpeedType::Descent));
        }
        let n = nd.checkpoints.len();
        for c in nd.checkpoints.iter().take(n.saturating_sub(2)) {
            if c.distance_from_start - current_distance > 1.0 {
                break;
            }
            let past_cstr = c.reason == Reason::StartDecelerationToConstraint && is_close_to_greater_than(current_distance, c.distance_from_start);
            let past_limit = c.reason == Reason::StartDecelerationToLimit && is_close_to_less_than(current_altitude, c.altitude);
            let ts = c.target_speed.unwrap_or(f64::NAN);
            if is_speed_change_point(c) && ts >= decel_speed && (past_cstr || past_limit) {
                target = js_min(target, ts);
                break;
            }
        }
        self.previous_managed_descent_speed_target = target;
        let vman = g.speeds.vman(p.approach_speed);
        let econ_mach_as_cas = p.atmosphere().cas_from_mach(p.present_altitude, p.managed_descent_speed_mach);
        self.out.speeds_managed_pfd = Some(js_max(js_max(g.speeds.vls, vman), js_min(target, econ_mach_as_cas)));
    }

    fn update_hold_speed(&mut self, p: &ComputationParameters, i: &UpdateInputs) {
        if !i.guidance.manual_hold_active && !i.manual_hold_next {
            return;
        }
        let atm = p.atmosphere();
        let altitude = i.guidance.current_altitude.unwrap_or(p.present_altitude);
        let mut cas = i.hold_speed;
        if i.autopilot_airspeed_selected && i.hold_decel_reached {
            cas = if i.autopilot_mach_mode_active { atm.cas_from_mach(altitude, i.autopilot_mach_hold_value) } else { i.autopilot_airspeed_hold_value };
        }
        self.out.hold_speed_tas = Some(atm.tas_from_cas(altitude, cas));
    }

    pub fn perf_crz_to_prediction(&self) -> Option<PerfCrzToPrediction> {
        let m = self.mcdu_profile.as_ref().filter(|m| m.is_ready_to_display)?;
        let c = m.find_vertical_checkpoint(&[Reason::StepClimb, Reason::StepDescent, Reason::TopOfDescent])?;
        Some(PerfCrzToPrediction {
            reason: c.reason,
            distance_from_present_position: c.distance_from_start - self.constraint_reader.distance_to_present_position(),
            seconds_from_present: c.seconds_from_present,
        })
    }

    /// `findNextSpeedChange`; `managed_pfd_speed` is `L:A32NX_SPEEDS_MANAGED_PFD`.
    pub fn find_next_speed_change(&self, p: &ComputationParameters, modes: &FcuModes, managed_pfd_speed: f64) -> Option<f64> {
        let nd = self.nd_profile.as_ref()?;
        if !modes.speed_managed || modes.expedite || p.flight_phase == FlightPhase::Approach {
            return None;
        }
        let ty = match p.flight_phase {
            FlightPhase::Cruise => ManagedSpeedType::Cruise,
            FlightPhase::Descent => ManagedSpeedType::Descent,
            _ => ManagedSpeedType::Climb,
        };
        let distance = nd.distance_to_present_position;
        let decel_speed = self.decel_point.map_or(f64::NAN, |d| d.speed);
        let speed_target = if p.flight_phase < FlightPhase::Climb {
            McduSpeedProfile::new(p, self.constraint_reader.distance_to_present_position(), Vec::new(), Vec::new())
                .get_target(distance, p.present_altitude, ManagedSpeedType::Climb)
        } else {
            managed_pfd_speed
        };
        let atm: Atmosphere = p.atmosphere();
        let cps = &nd.checkpoints;
        for k in 1..cps.len().saturating_sub(1) {
            let (c, prev) = (&cps[k], &cps[k - 1]);
            if c.distance_from_start < distance {
                continue;
            } else if prev.reason == Reason::TopOfClimb || prev.reason == Reason::TopOfDescent {
                return None;
            }
            if ty == ManagedSpeedType::Climb || ty == ManagedSpeedType::Cruise {
                if js_min(c.speed, atm.cas_from_mach(c.altitude, c.mach)) - js_max(prev.speed, speed_target) > 1.0 {
                    return Some(prev.distance_from_start);
                }
            } else if is_speed_change_point(c) {
                let ts = c.target_speed.unwrap_or(f64::NAN);
                if ts - speed_target < -1.0 && ts >= decel_speed && (c.reason == Reason::StartDecelerationToConstraint || p.fcu_altitude < c.altitude) {
                    return Some(c.distance_from_start);
                }
            }
        }
        None
    }

    pub fn invalidate_flight_plan_profile(&mut self) {
        self.request_descent_profile_recomputation = true;
        if let Some(m) = self.mcdu_profile.as_mut() {
            m.invalidate();
        }
    }

    pub fn compute_tactical_to_guidance_profile_offset(&self) -> f64 {
        if self.descent_profile.is_none() || !self.relation.is_valid {
            return 0.0;
        }
        self.constraint_reader.distance_to_present_position() - self.relation.distance_from_start()
    }

    /// `updateLegSpeedPredictions`: predicted (TAS, GS) for each leg from the active leg on; `None` for legs without a path.
    pub fn leg_speed_predictions(&self, p: &ComputationParameters, legs: &[(usize, Option<f64>)], active_leg_index: usize, current_wind_speed: f64) -> Vec<(usize, Option<(f64, f64)>)> {
        let Some(m) = self.mcdu_profile.as_ref().filter(|m| m.is_ready_to_display) else { return Vec::new() };
        let atm = p.atmosphere();
        let get = |i: usize| legs.iter().find(|(k, _)| *k == i);
        let mut out = Vec::new();
        let mut i = active_leg_index;
        while get(i).is_some() || get(i + 1).is_some() {
            if let Some((_, cumulative)) = get(i) {
                out.push((
                    i,
                    cumulative.map(|d| {
                        let pr = m.interpolate_everything_from_start(d, true);
                        let tas = atm.tas_from_cas(pr.altitude, pr.speed);
                        (tas, tas + current_wind_speed)
                    }),
                ));
            }
            i += 1;
        }
        out
    }
}

/// `VnavDriver.shouldShowLatDiscontinuityAhead`, with the flight plan lookups done by the caller.
pub fn should_show_lat_discontinuity_ahead(
    modes: &FcuModes,
    active_leg_is_hold_without_exit: bool,
    last_leg_before_discontinuity: Option<(usize, bool)>,
    active_leg_index: usize,
    prediction_seconds_from_present: Option<f64>,
    active_leg_seconds_to_go: Option<f64>,
) -> bool {
    if modes.lateral != lateral::NAV || active_leg_is_hold_without_exit {
        return false;
    }
    let Some((index, is_vectors)) = last_leg_before_discontinuity else { return false };
    if is_vectors {
        return false;
    }
    match prediction_seconds_from_present {
        Some(s) => s < 30.0,
        None => index == active_leg_index && active_leg_seconds_to_go.unwrap_or(f64::INFINITY) < 30.0,
    }
}

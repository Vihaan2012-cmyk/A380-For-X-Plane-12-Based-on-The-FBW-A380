use std::collections::BTreeMap;
use std::sync::OnceLock;

use fms::plan::{FlightPlan, LegType};
use fms_data::PerformanceData;
use fms_lnav::geo::bearing_to;
use fms_lnav::geometry::LegCalc;
use fms_lnav::Coord;
use fms_perf::{FlightPhase, PerfSpeeds};
use fms_vnav::config::{AircraftConfig, FlapConf};
use fms_vnav::constraints::{ConstraintLeg, ConstraintPlan, ConstraintType, CruiseStepEntry};
use fms_vnav::descent::is_altitude_constraint_met;
use fms_vnav::driver::{RecomputeInputs, UpdateInputs, VnavDriver, WindsFor};
use fms_vnav::guidance::{GuidanceInputs, SpeedData};
use fms_vnav::manager::PresentState;
use fms_vnav::modes::FcuModes;
use fms_vnav::profile::{self as vp, AltitudeDescriptor, ComputationParameters, ConstantWinds, Profile, SpeedLimit, Winds};
use serde_json::{json, Value};

use crate::inputs::Inputs;
use crate::outputs::Sink;

const TONNE_TO_LB: f64 = 2204.62262;
const FINAL_APPROACH_FIX: i32 = 5;
const MISSED_APPROACH_POINT: i32 = 8;

fn config() -> &'static AircraftConfig {
    static CONFIG: OnceLock<AircraftConfig> = OnceLock::new();
    CONFIG.get_or_init(AircraftConfig::a380)
}

pub fn vnav_phase(p: FlightPhase) -> vp::FlightPhase {
    match p {
        FlightPhase::Preflight => vp::FlightPhase::Preflight,
        FlightPhase::Takeoff => vp::FlightPhase::Takeoff,
        FlightPhase::Climb => vp::FlightPhase::Climb,
        FlightPhase::Cruise => vp::FlightPhase::Cruise,
        FlightPhase::Descent => vp::FlightPhase::Descent,
        FlightPhase::Approach => vp::FlightPhase::Approach,
        FlightPhase::GoAround => vp::FlightPhase::GoAround,
        FlightPhase::Done => vp::FlightPhase::Done,
    }
}

pub fn altitude_descriptor(s: &str) -> AltitudeDescriptor {
    use AltitudeDescriptor::*;
    match s {
        "@" => AtAlt1,
        "+" => AtOrAboveAlt1,
        "-" => AtOrBelowAlt1,
        "B" => BetweenAlt1Alt2,
        "C" => AtOrAboveAlt2,
        "G" => AtAlt1GsMslAlt2,
        "H" => AtOrAboveAlt1GsMslAlt2,
        "I" => AtAlt1GsIntcptAlt2,
        "J" => AtOrAboveAlt1GsIntcptAlt2,
        "V" => AtOrAboveAlt1AngleAlt2,
        "X" => AtAlt1AngleAlt2,
        "Y" => AtOrBelowAlt1AngleAlt2,
        _ => None,
    }
}

pub fn vnav_constraint(c: &fms::plan::AltitudeConstraint) -> vp::AltitudeConstraint {
    vp::AltitudeConstraint { descriptor: altitude_descriptor(&c.descriptor), altitude1: c.altitude1, altitude2: c.altitude2.unwrap_or(f64::NAN) }
}

pub fn constraint_json(c: &fms::plan::AltitudeConstraint) -> Value {
    let mut v = json!({ "altitudeDescriptor": c.descriptor, "altitude1": c.altitude1 });
    if let Some(a2) = c.altitude2 {
        v["altitude2"] = json!(a2);
    }
    v
}

/// One leg's `VerticalWaypointPrediction` (the parts the host publishes).
#[derive(Clone, Debug)]
pub struct LegPrediction {
    pub distance_from_start: f64,
    pub distance_from_aircraft: f64,
    pub seconds_from_present: f64,
    pub altitude: f64,
    pub speed: f64,
    pub estimated_fuel_on_board: f64,
    pub altitude_constraint: Option<fms::plan::AltitudeConstraint>,
    pub is_altitude_constraint_met: Option<bool>,
}

#[derive(Clone, Copy, Debug)]
pub struct ManagedProfilePoint {
    pub climb_altitude: f64,
    pub descent_altitude: f64,
    pub climb_speed: f64,
    pub descent_speed: f64,
    pub previous_descent_speed: f64,
}

impl Default for ManagedProfilePoint {
    fn default() -> Self {
        ManagedProfilePoint { climb_altitude: f64::INFINITY, descent_altitude: f64::NEG_INFINITY, climb_speed: f64::INFINITY, descent_speed: f64::INFINITY, previous_descent_speed: f64::INFINITY }
    }
}

/// FBW `qnhToMillibar`.
fn qnh_to_millibar(qnh: f64) -> f64 {
    if qnh < 100. {
        qnh * 33.8639
    } else {
        qnh
    }
}

pub struct VnavContext<'a> {
    pub plan: Option<&'a FlightPlan>,
    pub calc: Option<&'a [Option<LegCalc>]>,
    pub perf: Option<&'a PerformanceData>,
    pub speeds: &'a PerfSpeeds,
    pub phase: FlightPhase,
    pub inputs: &'a Inputs,
    pub distance_to_end: Option<f64>,
    pub has_temporary: bool,
    pub engines_running: bool,
    pub managed_pfd: f64,
    pub manual_hold_active: bool,
    pub manual_hold_next: bool,
    pub hold_speed: f64,
    pub hold_decel_reached: bool,
}

/// FBW `VnavDriver` and its host-side glue (computation parameters observer, MFD
/// vertical path topics, `updateConstraints`).
pub struct VnavHost {
    pub driver: VnavDriver<'static>,
    pub params: Option<ComputationParameters>,
    pub modes: FcuModes,
    pub predictions: BTreeMap<usize, LegPrediction>,
    managed_profile: BTreeMap<usize, ManagedProfilePoint>,
    constraint_alt: f64,
    last_ready: Option<bool>,
    athr_speed_published: bool,
}

impl Default for VnavHost {
    fn default() -> Self {
        VnavHost { driver: VnavDriver::new(config()), params: None, modes: FcuModes::default(), predictions: BTreeMap::new(), managed_profile: BTreeMap::new(), constraint_alt: 0., last_ready: None, athr_speed_published: false }
    }
}

fn opt(v: Option<f64>) -> f64 {
    v.unwrap_or(f64::NAN)
}

impl VnavHost {
    pub fn version(&self) -> u64 {
        self.driver.version
    }

    pub fn mcdu_profile(&self) -> Option<&Profile<'static>> {
        self.driver.mcdu_profile.as_ref()
    }

    pub fn managed_point(&self, leg_index: usize) -> Option<ManagedProfilePoint> {
        self.managed_profile.get(&leg_index).copied()
    }

    pub fn is_lat_auto_control_active(&self) -> bool {
        self.modes.is_lat_auto_control_active()
    }

    pub fn is_lat_auto_control_armed_with_intercept(&self) -> bool {
        self.modes.is_lat_auto_control_armed_with_intercept()
    }

    pub fn is_selected_vertical_mode_active(&self) -> bool {
        self.modes.is_selected_vertical_mode_active()
    }

    fn modes_from(i: &Inputs) -> FcuModes {
        FcuModes {
            lateral: i.lateral_mode,
            armed_lateral: i.lateral_armed as i32,
            vertical: i.vertical_mode,
            armed_vertical: i.vertical_armed as i32,
            vertical_speed: i.fcu_vertical_speed,
            flight_path_angle: i.fcu_flight_path_angle,
            speed_managed: i.fcu_speed_managed,
            expedite: i.expedite,
        }
    }

    /// FBW `VerticalProfileComputationParametersObserver.update` with the A380 `FmgcData` getters.
    pub fn compute_parameters(&self, c: &VnavContext) -> ComputationParameters {
        let pd = c.perf;
        let g = |f: fn(&PerformanceData) -> Option<f64>| pd.and_then(f);
        let plan = c.plan;
        let ci = g(|d| d.cost_index).or(plan.and_then(|p| p.performance.cost_index));
        let zfw_t = g(|d| d.zero_fuel_weight);
        let fob_t = fms_perf::weights::fuel_on_board_tonnes(c.engines_running, c.inputs.fuel_on_board_kg_fqms(), g(|d| d.block_fuel));
        let cruise = g(|d| d.cruise_flight_level).or(plan.and_then(|p| p.performance.cruise_flight_level)).map(|fl| fl * 100.);
        let lim = |s: Option<f64>, a: Option<f64>| match (s, a) {
            (Some(s), Some(a)) if s != 0. && a != 0. => Some(SpeedLimit { speed: s, under_altitude: a }),
            _ => None,
        };
        let dest_elev = plan
            .and_then(|p| p.destination_runway.as_ref().and_then(|r| r.threshold_location.alt).or(p.destination_airport.as_ref().and_then(|a| a.location.alt)))
            .unwrap_or(0.);
        let takeoff_flaps = match g(|d| d.takeoff_flaps.map(f64::from)).map(|f| f as u8) {
            Some(2) => FlapConf::Conf2,
            Some(3) => FlapConf::Conf3,
            _ => FlapConf::Conf1,
        };
        ComputationParameters {
            present_altitude: c.inputs.indicated_altitude_ft,
            fcu_altitude: c.inputs.fcu_selected_altitude_ft,
            fcu_speed: c.inputs.fcu_speed,
            managed_climb_speed: fms_perf::cost_index::managed_climb_speed(ci),
            managed_climb_speed_mach: fms_perf::cost_index::managed_climb_speed_mach(),
            managed_cruise_speed: fms_perf::cost_index::managed_cruise_speed(),
            managed_cruise_speed_mach: fms_perf::cost_index::managed_cruise_speed_mach(),
            managed_descent_speed: fms_perf::cost_index::managed_descent_speed(ci, g(|d| d.pilot_managed_descent_speed)),
            managed_descent_speed_mach: fms_perf::cost_index::managed_descent_speed_mach(g(|d| d.pilot_managed_descent_mach)),
            zero_fuel_weight: zfw_t.unwrap_or(fms_perf::constants::MIN_GW / 1000.) * TONNE_TO_LB,
            fuel_on_board: opt(fob_t.map(|t| t * TONNE_TO_LB)),
            v2_speed: g(|d| d.v2).unwrap_or(150.),
            tropopause: pd.map(|d| d.tropopause()).unwrap_or(36000.),
            perf_factor: 0.,
            departure_elevation: plan.and_then(|p| p.origin_runway.as_ref()).and_then(|r| r.threshold_location.alt).unwrap_or(0.),
            destination_elevation: dest_elev,
            acceleration_altitude: g(|d| d.acceleration_altitude()).unwrap_or(1500.),
            thrust_reduction_altitude: g(|d| d.thrust_reduction_altitude()).unwrap_or(1500.),
            cruise_altitude: cruise.or(self.params.as_ref().map(|p| p.cruise_altitude)).unwrap_or(f64::NAN),
            climb_speed_limit: lim(g(|d| d.climb_speed_limit_speed), g(|d| d.climb_speed_limit_altitude)),
            descent_speed_limit: lim(g(|d| d.descent_speed_limit_speed), g(|d| d.descent_speed_limit_altitude)),
            flight_phase: vnav_phase(c.phase),
            preselected_clb_speed: g(|d| d.preselected_climb_speed).unwrap_or(0.),
            preselected_cruise_speed: g(|d| d.preselected_cruise_speed).unwrap_or(0.),
            takeoff_flaps,
            takeoff_toga_n1: c.inputs.thrust_limit_toga,
            isa_deviation: c.inputs.isa_deviation(),
            approach_speed: c.speeds.approach_vapp.unwrap_or(0.),
            flap_retraction_speed: c.speeds.approach_flap_retraction.unwrap_or(0.),
            slat_retraction_speed: c.speeds.approach_slat_retraction.unwrap_or(0.),
            clean_speed: c.speeds.approach_green_dot.unwrap_or(0.),
            gross_weight_valid: c.inputs.fqms_gross_weight_kg.is_some() || (zfw_t.is_some() && fob_t.is_some()),
        }
    }

    fn constraint_plan(plan: &FlightPlan, calc: &[Option<LegCalc>]) -> ConstraintPlan {
        let all = plan.all_legs();
        let elements = all
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let leg = e.leg()?;
                let c = calc.get(i).and_then(|c| c.as_ref());
                Some(ConstraintLeg {
                    cumulative_distance_with_transitions: c.map(|c| c.cumulative_distance_with_transitions),
                    cumulative_distance_to_end_with_transitions: c.map(|c| c.cumulative_distance_to_end_with_transitions),
                    constraint_type: match leg.constraint_type {
                        1 => ConstraintType::Climb,
                        2 => ConstraintType::Descent,
                        _ => ConstraintType::Unknown,
                    },
                    altitude_constraint: leg.altitude_constraint().map(|a| vnav_constraint(&a)),
                    speed_constraint: leg.speed_constraint().map(|s| s.speed),
                    cruise_step: leg.cruise_step.as_ref().map(|s| CruiseStepEntry { waypoint_index: s.waypoint_index, to_altitude: s.to_altitude, distance_before_termination: s.distance_before_termination, is_ignored: s.is_ignored }),
                    vertical_angle: leg.definition.vertical_angle,
                    is_final_approach_fix: leg.definition.approach_waypoint_descriptor == Some(FINAL_APPROACH_FIX),
                })
            })
            .collect();
        let mapt = plan.approach.as_ref().filter(|a| a.approach_type != 0).and_then(|a| a.common_legs.iter().find(|l| l.approach_waypoint_descriptor == Some(MISSED_APPROACH_POINT) && l.altitude1.is_some_and(f64::is_finite)).and_then(|l| l.altitude1));
        ConstraintPlan {
            elements,
            active_leg_index: plan.active_leg_index.max(0) as usize,
            first_missed_approach_leg_index: plan.first_missed_approach_leg_index(),
            destination_leg_index: plan.destination_leg_index(),
            missed_approach_point_altitude: mapt,
            destination_runway_threshold_altitude: plan.destination_runway.as_ref().and_then(|r| r.threshold_location.alt),
            destination_airport_altitude: plan.destination_airport.as_ref().and_then(|a| a.location.alt),
        }
    }

    /// `VnavDriver.acceptMultipleLegGeometry` (the guidance controller calls it after each forced geometry update).
    pub fn recompute(&mut self, c: &VnavContext, legs_with_path: &[(usize, bool)]) {
        self.modes = Self::modes_from(c.inputs);
        let params = self.compute_parameters(c);
        let (Some(plan), Some(calc)) = (c.plan, c.calc) else {
            self.params = Some(params);
            return;
        };
        let cplan = Self::constraint_plan(plan, calc);
        let winds = |_: WindsFor| -> Box<dyn Winds> { Box::new(ConstantWinds::default()) };
        let vman = match c.inputs.flaps_handle {
            0 => c.speeds.green_dot.unwrap_or(0.),
            1 => c.speeds.slat_retraction.unwrap_or(0.),
            2 => c.speeds.flap_retraction.unwrap_or(0.),
            3 | 4 => params.approach_speed,
            _ => c.speeds.vls,
        };
        let ri = RecomputeInputs {
            modes: self.modes,
            plan: &cplan,
            distance_to_end: c.distance_to_end.unwrap_or(0.),
            legs: legs_with_path,
            active_leg_index: plan.active_leg_index.max(0) as usize,
            has_temporary: c.has_temporary,
            present: PresentState { indicated_airspeed: c.inputs.cas_kt.unwrap_or(0.), vman },
            green_dot_speed: c.speeds.green_dot.unwrap_or(0.),
            approach_qnh: qnh_to_millibar(c.perf.and_then(|d| d.approach_qnh).unwrap_or(1013.25)),
            approach_temperature: c.perf.and_then(|d| d.approach_temperature).unwrap_or(0.),
            estimated_destination_fuel: None,
            winds: &winds,
        };
        self.driver.recompute(&params, &ri);
        self.params = Some(params);
        self.update_predictions(plan, calc);
    }

    fn update_predictions(&mut self, plan: &FlightPlan, calc: &[Option<LegCalc>]) {
        self.predictions.clear();
        let Some(m) = self.driver.mcdu_profile.as_ref().filter(|m| m.is_ready_to_display) else { return };
        let atm = self.params.as_ref().map(|p| p.atmosphere());
        let start = (plan.active_leg_index - 1).max(0) as usize;
        for i in start..plan.first_missed_approach_leg_index() {
            let Some(leg) = plan.leg_at(i) else { continue };
            let d = calc.get(i).and_then(|c| c.as_ref()).map_or(f64::NAN, |c| c.cumulative_distance_with_transitions);
            let p = m.interpolate_everything_from_start(d, true);
            let speed = match &atm {
                Some(a) if a.cas_or_mach_is_mach(p.speed, p.mach, p.altitude) => p.mach,
                _ => p.speed,
            };
            let ac = leg.altitude_constraint();
            self.predictions.insert(
                i,
                LegPrediction {
                    distance_from_start: d,
                    distance_from_aircraft: d - m.distance_to_present_position,
                    seconds_from_present: p.seconds_from_present,
                    altitude: p.altitude,
                    speed,
                    estimated_fuel_on_board: p.remaining_fuel_on_board,
                    is_altitude_constraint_met: ac.as_ref().map(|a| is_altitude_constraint_met(&vnav_constraint(a), p.altitude, 250.)),
                    altitude_constraint: ac,
                },
            );
        }
    }

    /// `VnavDriver.update` plus the L:var writes of the descent guidance.
    pub fn update(&mut self, dt_ms: f64, c: &VnavContext, sink: &mut Sink) {
        self.modes = Self::modes_from(c.inputs);
        if let Some(params) = self.params.clone() {
            let p = ComputationParameters { flight_phase: vnav_phase(c.phase), present_altitude: c.inputs.indicated_altitude_ft, fcu_altitude: c.inputs.fcu_selected_altitude_ft, isa_deviation: c.inputs.isa_deviation(), ..params };
            let mach_to_kias = |m: f64| p.atmosphere().cas_from_mach(c.inputs.indicated_altitude_ft, m);
            let speeds = SpeedData {
                vls: c.speeds.vls,
                vmax: c.speeds.vmax,
                green_dot: c.speeds.green_dot.unwrap_or(0.),
                s_speed: c.speeds.slat_retraction.unwrap_or(0.),
                f_speed: c.speeds.flap_retraction.unwrap_or(0.),
                flaps_handle_index: c.inputs.flaps_handle,
                mach_to_kias: &mach_to_kias,
            };
            let ui = UpdateInputs {
                guidance: GuidanceInputs {
                    modes: self.modes,
                    speeds,
                    autopilot_airspeed_managed: c.inputs.fcu_speed_managed,
                    manual_hold_active: c.manual_hold_active,
                    inertial_vertical_speed: c.inputs.inertial_vertical_speed,
                    barometric_vertical_speed: c.inputs.barometric_vertical_speed,
                    ground_speed: c.inputs.ground_speed_kt,
                    current_airspeed: c.inputs.cas_kt.unwrap_or(0.),
                    current_pressure_altitude: c.inputs.pressure_altitude_ft.unwrap_or(c.inputs.indicated_altitude_ft),
                    current_altitude: c.inputs.baro_altitude_ft,
                    speeds_managed_pfd: c.managed_pfd,
                    speeds_managed_athr: c.managed_pfd,
                    origin_transition_altitude: c.perf.and_then(|d| d.pilot_transition_altitude.or(d.database_transition_altitude)).unwrap_or(f64::NAN),
                    autopilot_active: c.inputs.autopilot_active,
                    fma_lateral_mode: c.inputs.lateral_mode,
                    pause_at_tod_enabled: false,
                    pause_at_tod_distance: 0.,
                },
                distance_to_end: c.distance_to_end.unwrap_or(0.),
                manual_hold_next: c.manual_hold_next,
                hold_speed: c.hold_speed,
                hold_decel_reached: c.hold_decel_reached,
                autopilot_airspeed_selected: !c.inputs.fcu_speed_managed,
                autopilot_mach_mode_active: !c.inputs.fcu_speed_managed && c.inputs.fcu_speed > 0. && c.inputs.fcu_speed < 1.,
                autopilot_mach_hold_value: c.inputs.fcu_speed,
                autopilot_airspeed_hold_value: c.inputs.fcu_speed,
            };
            let descending = p.flight_phase == vp::FlightPhase::Descent;
            if !descending {
                self.driver.out.speeds_managed_pfd = None;
            }
            self.driver.update(dt_ms, &p, &ui);
            if p.flight_phase >= vp::FlightPhase::Takeoff {
                let o = self.driver.descent_guidance.out();
                sink.set("A32NX_FG_REQUESTED_VERTICAL_MODE", o.fg_requested_vertical_mode as i32 as f64);
                sink.set("A32NX_FG_TARGET_ALTITUDE", o.fg_target_altitude);
                sink.set("A32NX_FG_TARGET_VERTICAL_SPEED", o.fg_target_vertical_speed);
                sink.set("A32NX_PFD_TARGET_ALTITUDE", o.pfd_target_altitude);
                sink.set_bool("A32NX_PFD_LINEAR_DEVIATION_ACTIVE", o.pfd_linear_deviation_active);
                sink.set_bool("A32NX_PFD_VERTICAL_PROFILE_LATCHED", o.pfd_vertical_profile_latched);
                sink.set_bool("A32NX_PFD_SHOW_SPEED_MARGINS", o.pfd_show_speed_margins);
                sink.set("A32NX_PFD_LOWER_SPEED_MARGIN", o.pfd_lower_speed_margin);
                sink.set("A32NX_PFD_UPPER_SPEED_MARGIN", o.pfd_upper_speed_margin);
                sink.set_bool("A32NX_PFD_MSG_TD_REACHED", o.pfd_msg_td_reached);
                if descending {
                    if let Some(v) = o.speeds_managed_athr {
                        sink.set("A32NX_SPEEDS_MANAGED_ATHR", v);
                        self.athr_speed_published = true;
                    }
                    if let Some(v) = self.driver.out.speeds_managed_pfd.or(o.speeds_managed_pfd) {
                        sink.set("A32NX_SPEEDS_MANAGED_PFD", v);
                    }
                }
            }
            if !descending && self.athr_speed_published {
                self.athr_speed_published = false;
                sink.set("A32NX_SPEEDS_MANAGED_ATHR", -1.);
            }
        }
        let ready = self.driver.out.fm_vertical_profile_avail;
        if self.last_ready != Some(ready) {
            self.last_ready = Some(ready);
            sink.set_bool("A32NX_FM_VERTICAL_PROFILE_AVAIL", ready);
        }
    }

    /// `FmcAircraftInterface.updateManagedProfile`.
    pub fn update_managed_profile(&mut self, plan: Option<&FlightPlan>) {
        self.managed_profile.clear();
        let Some(plan) = plan else { return };
        let dest_elev = plan.destination_airport.as_ref().and_then(|a| a.location.alt).unwrap_or(0.);
        let end = plan.first_missed_approach_leg_index().min(plan.leg_count());
        let mut current_speed = f64::INFINITY;
        let mut previous_speed = f64::INFINITY;
        for i in 0..end {
            let Some(leg) = plan.leg_at(i) else { continue };
            if leg.constraint_type == 2 {
                if let Some(s) = leg.speed_constraint().filter(|s| s.speed != 0.) {
                    current_speed = current_speed.min(s.speed.round());
                }
            }
            self.managed_profile.insert(i, ManagedProfilePoint { descent_speed: current_speed, previous_descent_speed: previous_speed, ..ManagedProfilePoint::default() });
            previous_speed = current_speed;
        }
        let mut clb_speed = f64::INFINITY;
        let mut clb = f64::INFINITY;
        let mut des = f64::NEG_INFINITY;
        for i in (0..end).rev() {
            let Some(leg) = plan.leg_at(i) else { continue };
            if leg.constraint_type == 1 {
                if let Some(s) = leg.speed_constraint().filter(|s| s.speed != 0.) {
                    clb_speed = clb_speed.min(s.speed.round());
                }
            }
            if let Some(ac) = leg.altitude_constraint().filter(|a| a.altitude1 != 0.) {
                if leg.constraint_type == 1 && matches!(ac.descriptor.as_str(), "@" | "-" | "B") {
                    clb = clb.min(ac.altitude1.round());
                } else if leg.constraint_type == 2 {
                    match ac.descriptor.as_str() {
                        "@" | "+" => des = des.max(ac.altitude1.round()),
                        "B" => des = des.max(ac.altitude2.unwrap_or(ac.altitude1).round()),
                        _ => {}
                    }
                }
            }
            if let Some(p) = self.managed_profile.get_mut(&i) {
                p.climb_speed = clb_speed;
                p.climb_altitude = clb;
                p.descent_altitude = dest_elev.max(des);
            }
        }
    }

    /// `FmcAircraftInterface.updateConstraints`.
    pub fn update_constraints(&mut self, plan: Option<&FlightPlan>, phase: FlightPhase, fcu_alt: f64, sink: &mut Sink) {
        let Some(plan) = plan else { return };
        let mut alt = 0.;
        if let Some(c) = self.managed_profile.get(&(plan.active_leg_index.max(0) as usize)) {
            if (phase < FlightPhase::Cruise || phase == FlightPhase::GoAround) && c.climb_altitude.is_finite() && c.climb_altitude < fcu_alt {
                alt = c.climb_altitude;
            }
            if phase > FlightPhase::Cruise && phase < FlightPhase::GoAround && c.descent_altitude.is_finite() && c.descent_altitude > fcu_alt {
                alt = c.descent_altitude;
            }
        }
        if alt != self.constraint_alt {
            self.constraint_alt = alt;
            sink.set("A32NX_FG_ALTITUDE_CONSTRAINT", alt);
        }
    }

    /// `FlightManagementComputer.updateVerticalPath`.
    pub fn transmit_vertical_path(&self, plan: Option<&FlightPlan>, ppos: Coord, sink: &mut Sink) {
        let empty = || json!([]);
        let (Some(plan), true) = (plan, self.driver.mcdu_profile.is_some()) else {
            for t in ["a32nx_fms_vertical_target_profile", "a32nx_fms_vertical_constraints", "a32nx_fms_vertical_actual_profile", "a32nx_fms_vertical_descent_profile"] {
                sink.bus(t, empty());
            }
            sink.bus("a32nx_fms_vd_track_change_distance", Value::Null);
            return;
        };
        let target: Vec<Value> = self
            .driver
            .mcdu_profile
            .as_ref()
            .map(|m| m.checkpoints.iter().map(|c| json!({ "distanceFromAircraft": c.distance_from_start - m.distance_to_present_position, "altitude": c.altitude })).collect())
            .unwrap_or_default();
        let show_from = (plan.active_leg_index - 1).max(0) as usize;
        let all = plan.all_legs();
        let constraints: Vec<Value> = (show_from..all.len())
            .filter(|i| all[*i].leg().is_some())
            .filter_map(|i| self.predictions.get(&i))
            .map(|p| {
                let mut v = json!({});
                if let Some(ac) = &p.altitude_constraint {
                    v["altitudeConstraint"] = constraint_json(ac);
                }
                if let Some(m) = p.is_altitude_constraint_met {
                    v["isAltitudeConstraintMet"] = json!(m);
                }
                v
            })
            .collect();
        let offset = self.driver.compute_tactical_to_guidance_profile_offset();
        let descent: Vec<Value> = self
            .driver
            .descent_profile
            .as_ref()
            .map(|d| d.checkpoints.iter().map(|c| json!({ "distanceFromAircraft": c.distance_from_start + offset - d.distance_to_present_position, "altitude": c.altitude })).collect())
            .unwrap_or_default();
        let actual: Vec<Value> = self
            .driver
            .nd_profile
            .as_ref()
            .map(|n| n.checkpoints.iter().map(|c| json!({ "distanceFromAircraft": c.distance_from_start - n.distance_to_present_position, "altitude": c.altitude })).collect())
            .unwrap_or_default();
        let mut last_bearing: Option<f64> = None;
        let mut change: Option<f64> = None;
        let mut last = match all.get(show_from.wrapping_sub(1)).and_then(|e| e.leg()).and_then(|l| l.definition.waypoint.as_ref()) {
            Some(w) if show_from > 0 => Coord::new(w.location.lat, w.location.long),
            _ => ppos,
        };
        for (i, e) in all.iter().enumerate().skip(show_from) {
            let (Some(leg), Some(p)) = (e.leg(), self.predictions.get(&i)) else { continue };
            if let Some(w) = leg.definition.waypoint.as_ref() {
                let wc = Coord::new(w.location.lat, w.location.long);
                if change.is_none() {
                    let b = bearing_to(last, wc);
                    if last_bearing.is_some_and(|lb| (lb - b).abs() > 3.) {
                        change = Some(p.distance_from_aircraft);
                    }
                    last_bearing = Some(b);
                }
                last = wc;
            }
        }
        sink.bus("a32nx_fms_vertical_target_profile", Value::Array(target));
        sink.bus("a32nx_fms_vertical_constraints", Value::Array(constraints));
        sink.bus("a32nx_fms_vertical_actual_profile", Value::Array(actual));
        sink.bus("a32nx_fms_vertical_descent_profile", Value::Array(descent));
        sink.bus("a32nx_fms_vd_track_change_distance", change.map_or(Value::Null, |d| json!(d)));
    }

    pub fn is_xa(t: LegType) -> bool {
        matches!(t, LegType::CA | LegType::FA | LegType::VA)
    }
}

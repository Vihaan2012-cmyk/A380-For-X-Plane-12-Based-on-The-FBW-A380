use std::collections::BTreeMap;

use fms::navdb::NavDatabase;
use fms::plan::{Element, FlightPlan, LegType};
use fms::service::{Command, Fms, PlanRef};
use fms_lnav::common::max_bank;
use fms_lnav::driver::{nav_capture_condition, CaptureInputs, TurnState, FORCED_TURN_TKAE_THRESHOLD};
use fms_lnav::geo::{bearing_to, distance_to};
use fms_lnav::geometry::{LegCalc, Transition};
use fms_lnav::{Coord, Environment, Geometry, PlanElement, TurnDirection};
use fms_perf::FlightPhase;

use crate::convert::plan_elements;
use crate::efis::{departure_procedure_active, FIRST_SECONDARY, NUM_SECONDARY, SIDES};
use crate::inputs::Inputs;
use crate::outputs::Sink;

const GEOMETRY_RECOMPUTATION_MS: f64 = 5_000.;
const DEFAULT_MIN_PREDICTED_TAS: f64 = 160.;
const NAV_CAPTURE_INHIBIT_MS: f64 = 300.;
const APPROACH_IDENT_SIZE: usize = 14;

pub const LAW_HEADING: i32 = 1;
pub const LAW_TRACK: i32 = 2;
pub const LAW_LATERAL_PATH: i32 = 3;

/// FBW `SimVarString.pack`.
pub fn pack_string(value: &str, max_length: usize) -> Vec<f64> {
    let mut words: Vec<f64> = Vec::new();
    for (i, ch) in value.chars().take(max_length).enumerate() {
        if i % 8 == 0 {
            words.push(0.);
        }
        let mut code = ch as i64 - 31;
        if !(1..=63).contains(&code) {
            code = 0;
        }
        *words.last_mut().unwrap() += 2f64.powi((i % 8) as i32 * 6) * code as f64;
    }
    let want = max_length.div_ceil(8);
    while words.len() < want {
        words.push(0.);
    }
    words
}

fn true_to_magnetic(true_deg: f64, mag_var: f64) -> f64 {
    (true_deg - mag_var).rem_euclid(360.)
}

fn is_xf(t: LegType) -> bool {
    matches!(t, LegType::AF | LegType::CF | LegType::IF | LegType::DF | LegType::RF | LegType::TF | LegType::HF)
}

fn element_key(e: Option<&Element>) -> Option<String> {
    e.map(|e| match e {
        Element::Disco => "disco".to_string(),
        Element::Leg(l) => format!("{}|{}|{}", l.ident, l.leg_type.as_str(), l.termination_waypoint().map(|w| w.database_id.as_str()).unwrap_or("")),
    })
}

fn elements_repr(elements: &[PlanElement]) -> String {
    format!("{elements:?}")
}

pub struct PlanGeometryState {
    repr: String,
    version: u32,
    legs: Vec<bool>,
}

#[derive(Default)]
pub struct Guidance {
    pub geometries: BTreeMap<(usize, bool), Geometry>,
    pub calculated: BTreeMap<(usize, bool), Vec<Option<LegCalc>>>,
    states: BTreeMap<(usize, bool), PlanGeometryState>,
    recompute_timer_ms: f64,
    last_avail: Option<bool>,
    last_law: Option<i32>,
    pub last_xte: Option<f64>,
    last_tae: Option<f64>,
    last_phi: Option<f64>,
    turn_state: Option<TurnState>,
    nav_capture_inhibit_ms: f64,
    pub active_leg_dtg: Option<f64>,
    pub active_leg_complete_dtg: Option<f64>,
    pub active_leg_seconds_to_go: Option<f64>,
    env: Environment,
    approach_message: String,
    pub data_limit_reached: [bool; 2],
}

impl Guidance {
    pub fn new() -> Self {
        Guidance { recompute_timer_ms: GEOMETRY_RECOMPUTATION_MS + 1., turn_state: Some(TurnState::Normal), ..Default::default() }
    }

    fn turn_state(&self) -> TurnState {
        self.turn_state.unwrap_or(TurnState::Normal)
    }

    pub fn force_recompute(&mut self) {
        self.recompute_timer_ms = GEOMETRY_RECOMPUTATION_MS + 1.;
    }

    /// Returns true on the periodic forced recomputation (FBW then hands the active geometry to VNAV and the pseudo waypoints).
    pub fn update_geometries(&mut self, dt_ms: f64, fms: &Fms, inputs: &Inputs) -> bool {
        self.recompute_timer_ms += dt_ms;
        self.env = Environment { indicated_altitude: inputs.indicated_altitude_ft, wind_direction: inputs.ambient_wind_direction, wind_speed: inputs.ambient_wind_speed_kt, bank: 0. };
        let env = self.env;
        for g in self.geometries.values_mut() {
            g.set_environment(env);
        }
        let force = self.recompute_timer_ms > GEOMETRY_RECOMPUTATION_MS;
        if force {
            self.recompute_timer_ms = 0.;
        }
        let mut keys = vec![(0, false), (0, true), (1, false)];
        for i in 0..NUM_SECONDARY {
            keys.push((FIRST_SECONDARY + i, false));
            keys.push((FIRST_SECONDARY + i, true));
        }
        for key in keys {
            self.update_geometry(key, fms, inputs, false);
            if force {
                self.update_geometry(key, fms, inputs, true);
            }
        }
        force
    }

    fn update_geometry(&mut self, key: (usize, bool), fms: &Fms, inputs: &Inputs, force: bool) {
        let Some(main) = fms.plans.get(key.0).and_then(|p| p.as_ref()) else {
            self.geometries.remove(&key);
            self.calculated.remove(&key);
            self.states.remove(&key);
            return;
        };
        let empty;
        let plan: &FlightPlan = if key.1 {
            match main.alternate.as_deref() {
                Some(a) => a,
                None => {
                    empty = FlightPlan::empty(key.0);
                    &empty
                }
            }
        } else {
            main
        };
        let version = plan.version;
        if !force && self.states.get(&key).is_some_and(|s| s.version == version) {
            return;
        }
        let elements = plan_elements(plan);
        let repr = elements_repr(&elements);
        let rebuild = self.states.get(&key).is_none_or(|s| s.repr != repr) || !self.geometries.contains_key(&key);
        if rebuild {
            self.geometries.insert(key, Geometry::with_environment(&elements, self.env));
        }
        let legs = elements.iter().map(|e| matches!(e, PlanElement::Leg(_))).collect();
        self.states.insert(key, PlanGeometryState { repr, version, legs });
        let geometry = self.geometries.get_mut(&key).unwrap();
        let active = (plan.active_leg_index.max(0) as usize).min(geometry.leg_count().saturating_sub(1));
        let ppos = Coord::new(inputs.lat, inputs.lon);
        if geometry.leg_count() > 0 {
            geometry.recompute(active, ppos, inputs.true_track, inputs.true_airspeed_kt, inputs.ground_speed_kt);
        }
        let first_missed = plan.first_missed_approach_leg_index();
        let mut calc = geometry.update_calculated(plan.active_leg_index.saturating_sub(1).max(0) as usize, first_missed);
        let missed = geometry.update_calculated(first_missed, plan.leg_count());
        for (i, c) in missed.into_iter().enumerate() {
            if i >= first_missed {
                if let Some(slot) = calc.get_mut(i) {
                    *slot = c;
                }
            }
        }
        self.calculated.insert(key, calc);
    }

    pub fn is_leg(&self, key: (usize, bool), i: usize) -> bool {
        self.states.get(&key).is_some_and(|s| s.legs.get(i).copied().unwrap_or(false))
    }

    pub fn write_efis_ident(&self, fms: &Fms, sink: &mut Sink) {
        let ident = fms.active().and_then(|p| p.element_at(p.active_leg_index.max(0) as usize)).and_then(|e| e.leg()).map(|l| l.ident.clone()).unwrap_or_else(|| "PPOS".to_string());
        let words = pack_string(&ident, 9);
        for side in SIDES {
            sink.set(&format!("A32NX_EFIS_{side}_TO_WPT_IDENT_0"), words[0]);
            sink.set(&format!("A32NX_EFIS_{side}_TO_WPT_IDENT_1"), words[1]);
        }
    }

    pub fn write_approach_message(&mut self, fms: &Fms, phase: FlightPhase, distance_to_destination: Option<f64>, sink: &mut Sink) {
        let Some(active) = fms.active() else { return };
        let mut msg = String::new();
        if phase < FlightPhase::Cruise {
            if departure_procedure_active(active) {
                if let Some(dep) = active.departure.get() {
                    msg = format!("{:<width$}", dep.ident, width = APPROACH_IDENT_SIZE);
                }
            }
        } else if active.destination_runway.is_some() {
            let dist = distance_to_destination.unwrap_or(-1.);
            if phase > FlightPhase::Cruise || (phase == FlightPhase::Cruise && dist < 250.) {
                if let Some(appr) = active.approach.as_ref().filter(|a| a.approach_type != 0) {
                    msg = format!("{:<width$}", appr.ident, width = APPROACH_IDENT_SIZE);
                }
            }
        }
        if msg != self.approach_message {
            self.approach_message = msg.clone();
            let words = pack_string(&msg, APPROACH_IDENT_SIZE);
            for side in SIDES {
                sink.set(&format!("A32NX_EFIS_{side}_APPR_MSG_0"), words[0]);
                sink.set(&format!("A32NX_EFIS_{side}_APPR_MSG_1"), words[1]);
            }
        }
    }

    pub fn write_map_partly_displayed(&self, legs_culled: [bool; 2], sink: &mut Sink) {
        for (i, side) in SIDES.iter().enumerate() {
            sink.set_bool(&format!("A32NX_EFIS_{side}_MAP_PARTLY_DISPLAYED"), self.data_limit_reached[i] || legs_culled[i]);
        }
    }

    /// FBW `LnavDriver.update`. Returns early (as FBW does) when the active element has no
    /// geometry leg, which leaves the last written FG values in place.
    pub fn lnav_update(&mut self, dt_ms: f64, fms: &mut Fms, db: &dyn NavDatabase, inputs: &Inputs, phase: FlightPhase, sink: &mut Sink) {
        self.nav_capture_inhibit_ms = (self.nav_capture_inhibit_ms - dt_ms).max(0.);
        let ppos = Coord::new(inputs.lat, inputs.lon);
        let (track, tas, gs) = (inputs.true_track, inputs.true_airspeed_kt, inputs.ground_speed_kt);
        let mut available = false;
        let active_idx = fms.active().map(|p| p.active_leg_index.max(0) as usize);
        let key = (0, false);
        let has_legs = self.geometries.contains_key(&key) && self.states.get(&key).is_some_and(|s| s.legs.iter().any(|l| *l));
        if let (Some(active_idx), true) = (active_idx, has_legs) {
            if !self.is_leg(key, active_idx) {
                return;
            }
            let (next_is_leg, following_is_leg) = (self.is_leg(key, active_idx + 1), self.is_leg(key, active_idx + 2));
            let geometry = self.geometries.get_mut(&key).unwrap();
            let dtg = geometry.distance_to_go(active_idx, ppos);
            self.active_leg_dtg = dtg;
            self.active_leg_complete_dtg = dtg.map(|d| d + geometry.leg_lengths(active_idx)[2]);
            let Some(params) = geometry.guidance_parameters(active_idx, ppos, track, gs, tas) else { return };
            let active_spec_ci = matches!(fms.active().and_then(|p| p.leg_at(active_idx)).map(|l| l.leg_type), Some(LegType::CI | LegType::VI));
            let law = if active_spec_ci && params.cross_track_error == 0. && params.phi_command == 0. { LAW_TRACK } else { LAW_LATERAL_PATH };
            if self.last_law != Some(law) {
                self.last_law = Some(law);
                sink.set("A32NX_FG_CURRENT_LATERAL_LAW", law as f64);
            }
            sink.set("A32NX_FG_PHI_LIMIT", params.phi_limit);
            let (mut xte, mut tae, mut phi) = (params.cross_track_error, params.track_angle_error, params.phi_command);
            if law == LAW_TRACK {
                xte = 0.;
            }
            if self.turn_state() != TurnState::Normal {
                if tae.abs() < FORCED_TURN_TKAE_THRESHOLD {
                    self.turn_state = Some(TurnState::Normal);
                }
                let forced = if self.turn_state() == TurnState::ForceLeft { -max_bank(tas, true) } else { max_bank(tas, true) };
                phi = forced;
                tae = 0.;
                if law == LAW_LATERAL_PATH {
                    xte = 0.;
                }
            }
            if self.last_avail != Some(true) {
                sink.set_bool("A32NX_FM_LATERAL_FLIGHTPLAN_AVAIL", true);
                self.last_avail = Some(true);
            }
            if self.last_xte != Some(xte) {
                sink.set("A32NX_FG_CROSS_TRACK_ERROR", xte);
                self.last_xte = Some(xte);
            }
            if self.last_tae != Some(tae) {
                sink.set("A32NX_FG_TRACK_ANGLE_ERROR", tae);
                self.last_tae = Some(tae);
            }
            if self.last_phi != Some(phi) {
                sink.set("A32NX_FG_PHI_COMMAND", phi);
                self.last_phi = Some(phi);
            }
            available = true;
            sink.set("A32NX_GPS_WP_DISTANCE", dtg.unwrap_or(0.));

            let within = law != LAW_LATERAL_PATH || (params.cross_track_error.abs() < 7. && params.track_angle_error.abs() < 90.);
            let geometry = self.geometries.get_mut(&(0, false)).unwrap();
            let can_sequence = phase >= FlightPhase::Takeoff && !geometry.disable_automatic_sequencing(active_idx);
            let leg_is_null = geometry.is_null(active_idx);
            if (can_sequence && within && geometry.should_sequence_leg(active_idx, ppos)) || leg_is_null {
                let outbound = geometry.transition(active_idx).cloned();
                if next_is_leg {
                    self.sequence_leg(fms, db, Some(active_idx), outbound.as_ref());
                } else {
                    self.sequence_leg(fms, db, None, None);
                    self.nav_capture_inhibit_ms = NAV_CAPTURE_INHIBIT_MS;
                    if following_is_leg {
                        self.sequence_leg(fms, db, None, None);
                    }
                }
            }
        }

        self.update_nav_capture(fms, inputs, sink);

        if !available && self.last_avail != Some(false) {
            sink.set_bool("A32NX_FM_LATERAL_FLIGHTPLAN_AVAIL", false);
            sink.set("A32NX_FG_CROSS_TRACK_ERROR", 0.);
            sink.set("A32NX_FG_TRACK_ANGLE_ERROR", 0.);
            sink.set("A32NX_FG_PHI_COMMAND", 0.);
            self.last_avail = Some(false);
            self.last_tae = None;
            self.last_xte = None;
            self.last_phi = None;
            self.turn_state = Some(TurnState::Normal);
        }
    }

    fn sequence_leg(&mut self, fms: &mut Fms, db: &dyn NavDatabase, from_leg: Option<usize>, outbound: Option<&Transition>) {
        let before = fms.active().map(|p| (p.active_leg_index, element_key(p.element_at((p.active_leg_index - 1).max(0) as usize)), element_key(p.element_at(p.active_leg_index.max(0) as usize))));
        let _ = fms.execute(db, Command::Sequence { plan: PlanRef::Active });
        if let Some((old_active, from_key, to_key)) = before {
            for sec in 0..NUM_SECONDARY {
                let slot = FIRST_SECONDARY + sec;
                let Some(sp) = fms.plans[slot].as_ref() else { continue };
                let should = sp.active_leg_index == old_active && element_key(sp.element_at((old_active - 1).max(0) as usize)) == from_key && element_key(sp.element_at(old_active.max(0) as usize)) == to_key && from_key.is_some();
                if should {
                    let _ = fms.execute(db, Command::Sequence { plan: PlanRef::Secondary(sec + 1) });
                }
            }
        }
        if let (Some(i), Some(_)) = (from_leg, outbound) {
            if let Some(g) = self.geometries.get_mut(&(0, false)) {
                g.freeze_transition(i);
            }
        }
        self.turn_state = Some(match outbound {
            Some(Transition::PathCapture(pc)) => turn_state_for(pc.computed_turn),
            Some(Transition::CourseCapture(cc)) => turn_state_for(cc.computed_turn),
            _ => TurnState::Normal,
        });
    }

    fn update_nav_capture(&self, fms: &Fms, inputs: &Inputs, sink: &mut Sink) {
        let value = (|| {
            let plan = fms.active()?;
            let active = plan.active_leg_index.max(0) as usize;
            let leg = plan.leg_at(active)?;
            self.geometries.get(&(0, false))?;
            if !self.is_leg((0, false), active) || leg.leg_type == LegType::IF || self.nav_capture_inhibit_ms > 0. {
                return Some(false);
            }
            if matches!(leg.leg_type, LegType::VA | LegType::VD | LegType::VI | LegType::VM | LegType::VR | LegType::CA | LegType::CD | LegType::CI | LegType::CR) {
                return Some(true);
            }
            let last_xte = self.last_xte.unwrap_or(0.);
            Some(nav_capture_condition(&CaptureInputs {
                tas: inputs.true_airspeed_kt,
                leg_gs: inputs.ground_speed_kt,
                nominal_roll: 0.,
                last_xte,
                last_tae: self.last_tae.unwrap_or(0.),
                nav_engaged: inputs.nav_mode_engaged(),
            }))
        })()
        .unwrap_or(false);
        sink.set_bool("A32NX_FM1_NAV_CAPTURE_CONDITION", value);
        sink.set_bool("A32NX_FM2_NAV_CAPTURE_CONDITION", value);
    }

    /// FBW `GuidanceController.updateEfisData`.
    pub fn write_efis_data(&mut self, fms: &Fms, inputs: &Inputs, phase: FlightPhase, sink: &mut Sink) {
        let eta_computable = phase >= FlightPhase::Takeoff && inputs.ground_speed_kt > 100.;
        let ppos = Coord::new(inputs.lat, inputs.lon);
        let active = fms.active().and_then(|p| {
            let i = p.active_leg_index.max(0) as usize;
            let g = self.geometries.get(&(0, false))?;
            self.is_leg((0, false), i).then(|| (p.leg_at(i).cloned(), g.leg_end(i)))
        });
        let (bearing, true_bearing, distance, eta) = match active {
            Some((Some(leg), end)) => {
                let is_xm = matches!(leg.leg_type, LegType::FM | LegType::VM);
                let termination = if is_xm {
                    None
                } else if is_xf(leg.leg_type) {
                    leg.termination_waypoint().map(|w| Coord::new(w.location.lat, w.location.long))
                } else {
                    Some(end)
                };
                let true_bearing = termination.map_or(-1., |t| bearing_to(ppos, t));
                let bearing = match (termination, inputs.mag_var_deg) {
                    (None, _) => -1.,
                    (Some(_), Some(mv)) => true_to_magnetic(true_bearing, mv),
                    (Some(_), None) => true_bearing,
                };
                let distance = if is_xm { -1. } else { termination.map_or(-1., |t| distance_to(ppos, t)) };
                self.active_leg_seconds_to_go = if is_xm || !eta_computable {
                    None
                } else {
                    termination.map(|t| distance_to(ppos, t) / inputs.ground_speed_kt.max(DEFAULT_MIN_PREDICTED_TAS) * 3600.)
                };
                let eta = self.active_leg_seconds_to_go.map_or(-1., |s| (inputs.zulu_time_s.floor() + s) % 86_400.);
                (bearing, true_bearing, distance, eta)
            }
            _ => {
                self.active_leg_seconds_to_go = None;
                (-1., -1., -1., -1.)
            }
        };
        for side in SIDES {
            sink.set(&format!("A32NX_EFIS_{side}_TO_WPT_BEARING"), bearing);
            sink.set(&format!("A32NX_EFIS_{side}_TO_WPT_TRUE_BEARING"), true_bearing);
            sink.set(&format!("A32NX_EFIS_{side}_TO_WPT_DISTANCE"), distance);
            sink.set(&format!("A32NX_EFIS_{side}_TO_WPT_ETA"), eta);
        }
    }

    /// FBW `Geometry.computeAlongTrackDistanceToDestination` (LNAV driver), from the reference leg.
    pub fn along_track_distance_to_destination(&self, fms: &Fms, inputs: &Inputs) -> Option<f64> {
        let plan = fms.active()?;
        let key = (0, false);
        let g = self.geometries.get(&key)?;
        if !self.states.get(&key).is_some_and(|s| s.legs.iter().any(|l| *l)) {
            return Some(0.);
        }
        let active = plan.active_leg_index.max(0) as usize;
        let reference = if !self.is_leg(key, active) {
            active + 1
        } else if matches!(plan.leg_at(active).map(|l| l.leg_type), Some(LegType::VM | LegType::FM)) {
            active + 2
        } else {
            active
        };
        if !self.is_leg(key, reference) {
            return None;
        }
        let to_end = self.calculated.get(&key)?.get(reference)?.as_ref()?.cumulative_distance_to_end_with_transitions;
        let ppos = Coord::new(inputs.lat, inputs.lon);
        let leg = plan.leg_at(reference)?;
        let dtg = match (leg.leg_type, leg.termination_waypoint()) {
            (LegType::IF, Some(w)) => distance_to(ppos, Coord::new(w.location.lat, w.location.long)),
            _ => g.distance_to_go(reference, ppos)?,
        };
        to_end.is_finite().then_some(dtg + to_end)
    }

    pub fn distance_to_destination(&self, fms: &Fms, inputs: &Inputs) -> Option<f64> {
        let plan = fms.active()?;
        let calc = self.calculated.get(&(0, false))?;
        let active = plan.active_leg_index.max(0) as usize;
        let to_end = calc.get(active)?.as_ref()?.cumulative_distance_to_end_with_transitions;
        let dtg = self.geometries.get(&(0, false))?.distance_to_go(active, Coord::new(inputs.lat, inputs.lon)).unwrap_or(0.);
        Some(to_end + dtg)
    }
}

fn turn_state_for(t: TurnDirection) -> TurnState {
    match t {
        TurnDirection::Left => TurnState::ForceLeft,
        TurnDirection::Right => TurnState::ForceRight,
        TurnDirection::Either => TurnState::Normal,
    }
}

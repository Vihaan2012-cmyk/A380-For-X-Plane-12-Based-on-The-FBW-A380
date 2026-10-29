use std::collections::BTreeMap;
use std::sync::Arc;

use fms::navdb::NavDatabase;
use fms::perf::PerfState;
use fms::plan::{FlightPlan, LegType};
use fms::revisions::true_to_magnetic;
use fms::service::{AircraftState, Command as PlanCommand, Fms, PlanRef};
use fms_data::{PerfDataStore, PerformanceData};
use fms_perf::outputs::{managed_speed_lvar, managed_speed_reset_lvar, perf_speed_lvars, phase_lvars};
use fms_perf::managed::{approach_managed_speed, calculate_decel_dist, holding_speed, speed_constraint, update_managed_speed, HoldingSpeedInputs, LegSpeedConstraints, ManagedSpeedInputs, SpeedLimit};
use fms_perf::phase::{ConfirmationNode, VsKnobResult};
use fms_perf::weights::{self, LandingWeightInputs};
use fms_perf::{FlightPhase, FlightPhaseManager, PerfComputer, PerfInputs, PerfSpeeds, PhaseInputs};
use fms_lnav::{Coord, Geometry};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::command::{Envelope, HostCommand};
use crate::convert::elements_from;
use crate::efis::{EfisInterface, EfisVectors, VectorsContext, FIRST_SECONDARY};
use crate::facilities::{Facilities, NoFacilities};
use crate::guidance::Guidance;
use crate::inputs::{arinc_discrete, arinc_value, Inputs};
use crate::navaids::{NavInputs, NavaidTuner};
use crate::outputs::{Outputs, Reply, Sink};
use crate::pwp::{EfisState, PseudoWaypoints, PwpContext};
use crate::runway::RunwayData;
use crate::symbols::{bnr_word, EfisSymbols, SymbolsContext, A380_RANGE_VALUES};
use crate::vnav::{VnavContext, VnavHost};

mod plan_sync;
pub use plan_sync::{fix_json, leg_json, segment_json, PlanSync, SYNC_CLIENT_ID};

const ND_FLAG_NAV_PRIMARY: u32 = 1 << 6;
const ND_FLAG_NAV_PRIMARY_LOST: u32 = 1 << 9;

/// What survives a worker crash: the plans, the performance data and the flight phase.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub plans: Vec<Option<FlightPlan>>,
    pub perf: BTreeMap<usize, PerformanceData>,
    pub phase: u8,
    /// Active plan F-PLN predictions by leg index (FBW `VerticalWaypointPrediction`).
    #[serde(default)]
    pub predictions: BTreeMap<usize, WaypointPrediction>,
    /// Pseudo waypoints of the active plan, as the MFD F-PLN lists them.
    #[serde(default)]
    pub pseudo_waypoints: Vec<PseudoWaypointPrediction>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WaypointPrediction {
    pub distance_from_start: f64,
    pub distance_from_aircraft: f64,
    pub seconds_from_present: f64,
    pub altitude: f64,
    /// Knots, or Mach when below 1.
    pub speed: f64,
    pub estimated_fuel_on_board: f64,
    pub is_altitude_constraint_met: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PseudoWaypointPrediction {
    pub ident: String,
    pub mcdu_ident: Option<String>,
    pub along_leg_index: Option<usize>,
    pub distance_from_start: f64,
    pub displayed_on_mcdu: bool,
    pub altitude: Option<f64>,
    pub seconds_from_present: Option<f64>,
}

const ENGINE_ON_N2: f64 = 20.;
const FMS_CYCLE_TIME_MS: f64 = 250.;
const CONFIG_ACCEL_ALT_FT: f64 = 1500.;
const LB_TO_KG: f64 = 0.453_592_37;
const DEST_DATA_CHECK_PERIOD_MS: f64 = 15_000.;

#[derive(Default)]
struct HoldState {
    decel_reached: bool,
    speed_target: Option<f64>,
    message_active: bool,
}

/// The FMS host logic, single threaded and deterministic; `FmsHost` runs it on a worker.
pub struct FmsCore {
    pub fms: Fms,
    perf: PerfComputer,
    phase: FlightPhaseManager,
    pub guidance: Guidance,
    vectors: EfisVectors,
    pub ifaces: [EfisInterface; 2],
    sink: Sink,
    gps_primary: Option<bool>,
    nd_flags: [u32; 2],
    started: bool,
    facilities: Arc<dyn Facilities>,
    pub navaids: NavaidTuner,
    runway: RunwayData,
    pub vnav: VnavHost,
    pub pwp: PseudoWaypoints,
    symbols: [EfisSymbols; 2],
    sync: PlanSync,
    eosid: Option<(u32, Geometry)>,
    speeds: PerfSpeeds,
    managed_pfd: f64,
    last_fp_version: Option<(bool, u32)>,
    last_efis: Option<[(i32, f64); 2]>,
    last_master_prim_fcu: Option<[f64; 4]>,
    last: Inputs,
    fms_cycle_ms: f64,
    cruise_latch: Option<f64>,
    engine_out: bool,
    engine_failure_prev: bool,
    spawned: bool,
    is_reset: bool,
    engines_were_started: bool,
    alt_active_in_climb: ConfirmationNode,
    dest_data_checked: bool,
    dest_check_ms: f64,
    dest_efob_below_min: bool,
    dest_efob_message: bool,
    td_reached: bool,
    speed_limit_exceeded: bool,
    step_ahead_altitude: Option<f64>,
    too_steep: bool,
    lat_discontinuity: bool,
    hold: HoldState,
    managed_is_mach: bool,
    check_speed_mode: bool,
    zfw_init_displayed: u32,
    zfw_init_settable: bool,
}

impl Default for FmsCore {
    fn default() -> Self {
        Self::new()
    }
}

fn phase_inputs(i: &Inputs, cruise_ft: f64) -> PhaseInputs {
    PhaseInputs {
        ready: i.ready,
        slew_active: i.slew_active,
        enable_approach_phase: i.enable_approach_phase,
        on_ground: i.on_ground,
        n1_pct: i.n1,
        n2_pct: i.n2,
        vertical_mode: i.vertical_mode,
        ground_speed_kt: i.ground_speed_kt,
        pressure_altitude_ft: i.pressure_altitude_ft,
        baro_corrected_altitude_ft: i.baro_altitude_ft,
        cas_kt: i.cas_kt,
        engine_out: i.engine_out,
        cruise_altitude_ft: cruise_ft,
        fcu_selected_altitude_ft: i.fcu_selected_altitude_ft,
        altitude_above_ground_ft: i.altitude_above_ground_ft,
        indicated_altitude_ft: i.indicated_altitude_ft,
        ..PhaseInputs::default()
    }
}

impl FmsCore {
    pub fn new() -> Self {
        Self::with_facilities(Arc::new(NoFacilities))
    }

    pub fn with_facilities(facilities: Arc<dyn Facilities>) -> Self {
        FmsCore {
            fms: Fms::new(),
            perf: PerfComputer::new(),
            phase: FlightPhaseManager::new(),
            guidance: Guidance::new(),
            vectors: EfisVectors::default(),
            ifaces: [EfisInterface::default(), EfisInterface::default()],
            sink: Sink::default(),
            gps_primary: None,
            nd_flags: [0, 0],
            started: false,
            navaids: NavaidTuner::new(facilities.clone()),
            facilities,
            runway: RunwayData::default(),
            vnav: VnavHost::default(),
            pwp: PseudoWaypoints::default(),
            symbols: [EfisSymbols::new(0), EfisSymbols::new(1)],
            sync: PlanSync::default(),
            eosid: None,
            speeds: PerfSpeeds::default(),
            managed_pfd: 0.,
            last_fp_version: None,
            last_efis: None,
            last_master_prim_fcu: None,
            last: Inputs::default(),
            fms_cycle_ms: 0.,
            cruise_latch: None,
            engine_out: false,
            engine_failure_prev: false,
            spawned: false,
            is_reset: false,
            engines_were_started: false,
            alt_active_in_climb: ConfirmationNode::new(600_000.),
            dest_data_checked: false,
            dest_check_ms: 0.,
            dest_efob_below_min: false,
            dest_efob_message: false,
            td_reached: false,
            speed_limit_exceeded: false,
            step_ahead_altitude: None,
            too_steep: false,
            lat_discontinuity: false,
            hold: HoldState::default(),
            managed_is_mach: false,
            check_speed_mode: false,
            zfw_init_displayed: 0,
            zfw_init_settable: false,
        }
    }

    pub fn phase(&self) -> FlightPhase {
        self.phase.phase()
    }

    pub fn snapshot(&self) -> Snapshot {
        let predictions = self
            .vnav
            .predictions
            .iter()
            .map(|(i, p)| {
                let w = WaypointPrediction {
                    distance_from_start: p.distance_from_start,
                    distance_from_aircraft: p.distance_from_aircraft,
                    seconds_from_present: p.seconds_from_present,
                    altitude: p.altitude,
                    speed: p.speed,
                    estimated_fuel_on_board: p.estimated_fuel_on_board,
                    is_altitude_constraint_met: p.is_altitude_constraint_met,
                };
                (*i, w)
            })
            .collect();
        let pseudo_waypoints = self
            .pwp
            .all
            .iter()
            .map(|p| PseudoWaypointPrediction {
                ident: p.ident.clone(),
                mcdu_ident: p.mcdu_ident.clone(),
                along_leg_index: p.along_leg_index,
                distance_from_start: p.distance_from_start,
                displayed_on_mcdu: p.displayed_on_mcdu,
                altitude: p.predicted_altitude,
                seconds_from_present: p.seconds_from_present,
            })
            .collect();
        Snapshot { plans: self.fms.plans.to_vec(), perf: self.fms.perf.plans.clone(), phase: self.phase.phase() as u8, predictions, pseudo_waypoints }
    }

    pub fn restore(s: &Snapshot) -> Self {
        Self::restore_with(s, Arc::new(NoFacilities))
    }

    pub fn restore_with(s: &Snapshot, facilities: Arc<dyn Facilities>) -> Self {
        let mut core = FmsCore::with_facilities(facilities);
        for (i, p) in s.plans.iter().enumerate().take(6) {
            core.fms.plans[i] = p.clone();
        }
        core.fms.perf.plans = s.perf.clone();
        if let Some(p) = FlightPhase::from_index(s.phase as i32).filter(|p| *p != FlightPhase::Preflight) {
            core.phase.change_phase(p, &PhaseInputs::default());
            core.phase.take_transitions();
            core.phase.take_flex_temp_cleared();
            core.phase.take_cold_and_dark_cleared();
            core.spawned = true;
        }
        core
    }

    pub fn full_state(&self) -> Outputs {
        self.sink.full_state()
    }

    fn cruise_live(&self) -> Option<f64> {
        let p = self.fms.active()?;
        self.fms.perf.get(0).and_then(|d| d.cruise_flight_level).or(p.performance.cruise_flight_level).map(|fl| fl * 100.)
    }

    /// `L:A32NX_AIRLINER_CRUISE_ALTITUDE` keeps its value when the cruise level is cleared on entering descent.
    fn cruise_ft(&mut self) -> Option<f64> {
        let live = self.cruise_live();
        if live.is_some() {
            self.cruise_latch = live;
        }
        live.or(self.cruise_latch)
    }

    fn phase_inputs_for(&self, i: &Inputs, cruise_ft: f64) -> PhaseInputs {
        let mut p = phase_inputs(i, cruise_ft);
        p.green_dot_kt = self.speeds.green_dot.unwrap_or(0.);
        p.config_accel_alt_ft = CONFIG_ACCEL_ALT_FT;
        let pd = self.fms.perf.get(0);
        let (acc, eo) = match self.phase.phase() {
            FlightPhase::GoAround => (pd.and_then(|d| d.missed_acceleration_altitude()), pd.and_then(|d| d.missed_engine_out_acceleration_altitude())),
            ph if ph <= FlightPhase::Takeoff => (pd.and_then(|d| d.acceleration_altitude()), pd.and_then(|d| d.engine_out_acceleration_altitude())),
            _ => (None, None),
        };
        p.fm_acc_alt_ft = acc;
        p.fm_eo_acc_alt_ft = eo;
        p
    }

    fn current_phase_inputs(&self) -> PhaseInputs {
        let cruise = self.cruise_live().or(self.cruise_latch).unwrap_or(0.);
        self.phase_inputs_for(&self.last, cruise)
    }

    fn set_preselected(&mut self, v: Option<f64>) {
        let (spd, mach) = match v.filter(|v| *v != 0.) {
            Some(v) if v > 1. => (v, -1.),
            Some(v) => (-1., v),
            None => (-1., -1.),
        };
        self.sink.set("A32NX_SpeedPreselVal", spd);
        self.sink.set("A32NX_MachPreselVal", mach);
    }

    pub fn command(&mut self, db: &dyn NavDatabase, env: Envelope) {
        let id = env.id;
        let result: Result<(), String> = match env.command {
            HostCommand::Plan(cmd) => {
                let copy = match &cmd {
                    PlanCommand::UplinkInsert => Some((2, 0)),
                    PlanCommand::TemporaryInsert => Some((1, 0)),
                    PlanCommand::SecondaryCopyFromActive { index } => Some((0, FIRST_SECONDARY + index - 1)),
                    _ => None,
                };
                let swap = match &cmd {
                    PlanCommand::ActiveAndSecondarySwap { index } => Some(FIRST_SECONDARY + index - 1),
                    _ => None,
                };
                let had_tmpy = self.fms.plans[1].is_some();
                let old_destination = swap.and(self.fms.active()).and_then(|p| p.destination_airport.as_ref()).map(|a| a.ident.clone());
                let r = self.fms.execute(db, cmd).map_err(|e| e.to_string());
                if let (Ok(()), Some(old)) = (&r, old_destination) {
                    let new = self.fms.active().and_then(|p| p.destination_airport.as_ref()).map(|a| a.ident.clone());
                    if new.is_some_and(|n| n != old) {
                        let pi = self.current_phase_inputs();
                        self.phase.handle_new_destination_airport_entered(&pi);
                    }
                }
                if let (Ok(()), Some((from, to))) = (&r, copy) {
                    self.sync.copied(from, to, if from == 1 { 1 } else { 0 }, &mut self.sink);
                } else if let (Ok(()), Some(sec)) = (&r, swap) {
                    self.sync.swapped(sec, 0, &mut self.sink);
                } else if r.is_ok() && !had_tmpy && self.fms.plans[1].is_some() {
                    self.sync.copied(0, 1, 1, &mut self.sink);
                }
                r
            }
            HostCommand::Perf(cmd) => {
                self.fms.execute(db, PlanCommand::PerfData(cmd)).map_err(|e| e.to_string())
            }
            HostCommand::UplinkSimbrief(ofp) => {
                let mut log = Vec::new();
                let r = fms::uplink::uplink_flight_plan_from_simbrief(&mut self.fms, db, &ofp, &mut log).map_err(|e| e.to_string());
                self.sink.out.messages.extend(log);
                r
            }
            HostCommand::SetFlightNumber { plan, number } => match self.fms.plans.get_mut(plan).and_then(|p| p.as_mut()) {
                Some(p) => {
                    p.flight_number = number;
                    p.increment_version();
                    Ok(())
                }
                None => Err("no such plan".into()),
            },
            HostCommand::SetPlanCentre { side, fp_index, index, in_alternate } => {
                self.ifaces[side.min(1)].set_plan_centre(fp_index, index, in_alternate);
                Ok(())
            }
            HostCommand::SetSecRelatedPageOpen { side, sec_index } => {
                self.ifaces[side.min(1)].set_sec_related_page_open(sec_index);
                Ok(())
            }
            HostCommand::SetShownFplnLegs { side, missed, alternate, alternate_missed } => {
                let i = &mut self.ifaces[side.min(1)];
                i.shown_missed = missed;
                i.shown_alternate = alternate;
                i.shown_alternate_missed = alternate_missed;
                Ok(())
            }
            HostCommand::ClearNdMessage { flag } => {
                for side in 0..2 {
                    self.nd_flags[side] &= !flag;
                    self.sink.set(&format!("A32NX_EFIS_{}_ND_FM_MESSAGE_FLAGS", crate::efis::SIDES[side]), self.nd_flags[side] as f64);
                }
                Ok(())
            }
            HostCommand::ForcePhase(p) => {
                let pi = self.current_phase_inputs();
                self.phase.change_phase(p, &pi);
                Ok(())
            }
            HostCommand::PresetSpeed(v) => {
                self.set_preselected(v);
                Ok(())
            }
            HostCommand::ResetSystem => {
                self.reset_plans();
                Ok(())
            }
            HostCommand::InjectPanic => panic!("fms_host: injected panic"),
        };
        self.sync.update(&self.fms.plans, &self.fms.perf.plans, &mut self.sink);
        self.drain_perf_events();
        self.process_transitions(db);
        self.sink.out.replies.push(match result {
            Ok(()) => Reply::Ok { id },
            Err(message) => Reply::Err { id, message },
        });
    }

    pub fn tick(&mut self, db: &dyn NavDatabase, dt_ms: f64, raw: &Inputs) -> Outputs {
        let owned = Inputs { engine_out: self.engine_out, ..raw.clone() };
        self.last = owned.clone();
        let inputs = &owned;
        if !self.started {
            self.started = true;
            for r in [PlanRef::Active, PlanRef::Uplink] {
                if self.fms.plan(r).is_none() {
                    self.fms.create(r);
                }
            }
            for v in ["A32NX_FMC_A_IS_HEALTHY", "A32NX_FMC_B_IS_HEALTHY", "A32NX_FMC_C_IS_HEALTHY", "A32NX_FM1_HEALTHY_DISCRETE", "A32NX_FM2_HEALTHY_DISCRETE"] {
                self.sink.set(v, 1.);
            }
            self.sink.set("A32NX_FMS_L_FAILED", 0.);
            self.sink.set("A32NX_FMS_R_FAILED", 0.);
            self.sink.set("A32NX_SpeedPreselVal", -1.);
            self.sink.set("A32NX_MachPreselVal", -1.);
            self.write_vnav_defaults();
            if inputs.autothrust_disabled {
                self.sink.key("A32NX.ATHR_RESET_DISABLE", 1.);
            }
        }
        self.fms.aircraft = AircraftState {
            position: fms::geo::Coordinates::new(inputs.lat, inputs.lon),
            track: inputs.mag_var_deg.map_or(inputs.true_track, |m| true_to_magnetic(inputs.true_track, Some(m))),
            mag_var: inputs.mag_var_deg,
        };
        let cruise_ft = self.cruise_ft();
        self.update_perf_state(inputs);
        let pi = self.phase_inputs_for(inputs, cruise_ft.unwrap_or(0.));
        if !self.spawned && (inputs.on_ground || inputs.plane_altitude_ft > 1.) {
            self.spawned = true;
            self.phase.spawn(inputs.on_ground, inputs.plane_altitude_ft, inputs.flaps_handle, &pi);
        }
        self.phase.update(dt_ms, &pi);
        self.fcu_phase_events(inputs, &pi);
        self.process_transitions(db);
        if self.phase.phase() == FlightPhase::Done && self.is_reset && self.fms.active().is_some_and(|p| p.origin_airport.is_some()) {
            let pi = self.current_phase_inputs();
            self.phase.change_phase(FlightPhase::Preflight, &pi);
            self.process_transitions(db);
        }
        let phase = self.phase.phase();
        for v in phase_lvars(&self.phase) {
            self.sink.set(v.name, v.value);
        }
        self.write_perf(inputs, phase, dt_ms);
        self.write_navigation(inputs);
        self.write_fm_data(cruise_ft);
        self.write_fm_words(phase);
        self.update_navaids(dt_ms, inputs, phase);
        let fcu_alt = inputs.prim_selected_altitude().unwrap_or(inputs.fcu_selected_altitude_ft);
        self.vnav.update_constraints(self.fms.active(), phase, fcu_alt, &mut self.sink);
        self.vnav.transmit_vertical_path(self.fms.active(), Coord::new(inputs.lat, inputs.lon), &mut self.sink);
        let shown = if self.fms.has_temporary() { self.fms.plans[1].as_ref() } else { self.fms.active() };
        let fp_version = shown.map(|p| (self.fms.has_temporary(), p.version));
        if fp_version != self.last_fp_version {
            self.last_fp_version = fp_version;
            self.vnav.update_managed_profile(self.fms.active());
            self.runway.update(self.fms.active(), self.facilities.as_ref(), &mut self.sink);
        }

        let forced = self.guidance.update_geometries(dt_ms, &self.fms, inputs);
        self.update_eosid_geometry(inputs);
        let efis = [0, 1].map(|s| (inputs.nd_mode[s], inputs.nd_range.get(s).and_then(|r| A380_RANGE_VALUES.get(*r)).copied().unwrap_or(f64::NAN)));
        let efis_changed = self.last_efis.is_none_or(|l| l.iter().zip(&efis).any(|(a, b)| a.0 != b.0 || a.1.to_bits() != b.1.to_bits()));
        self.last_efis = Some(efis);
        if forced && self.guidance.geometries.contains_key(&(0, false)) {
            let legs = self.legs_with_representation();
            let mut vnav = std::mem::take(&mut self.vnav);
            vnav.recompute(&self.vnav_context(inputs, phase), &legs);
            self.vnav = vnav;
            self.update_leg_speed_predictions(inputs);
        }
        if forced || efis_changed {
            let mut pwp = std::mem::take(&mut self.pwp);
            pwp.recompute(&self.pwp_context(inputs, phase, efis));
            self.pwp = pwp;
        }
        self.guidance.write_efis_ident(&self.fms, &mut self.sink);
        self.guidance.write_map_partly_displayed(self.vectors.legs_culled, &mut self.sink);
        self.guidance.lnav_update(dt_ms, &mut self.fms, db, inputs, phase, &mut self.sink);
        self.guidance.write_efis_data(&self.fms, inputs, phase, &mut self.sink);
        let mut vnav = std::mem::take(&mut self.vnav);
        let mut sink = std::mem::take(&mut self.sink);
        vnav.update(dt_ms, &self.vnav_context(inputs, phase), &mut sink);
        self.vnav = vnav;
        let mut pwp = std::mem::take(&mut self.pwp);
        pwp.update(dt_ms, &self.pwp_context(inputs, phase, efis), self.guidance.active_leg_complete_dtg, &mut sink);
        self.pwp = pwp;
        self.sink = sink;
        self.apply_hold_speed();
        self.track_td_reached();
        let dist = self.guidance.distance_to_destination(&self.fms, inputs);
        self.guidance.write_approach_message(&self.fms, phase, dist, &mut self.sink);
        let ctx = VectorsContext {
            plans: &self.fms.plans,
            geometries: &self.guidance.geometries,
            phase,
            lateral_mode: inputs.lateral_mode,
            lateral_armed: inputs.lateral_armed,
            nd_mode: inputs.nd_mode,
            engine_out_active: inputs.engine_out,
            eosid: self.eosid.as_ref().map(|e| &e.1),
        };
        self.vectors.update(dt_ms, &ctx, &self.ifaces, &mut self.sink);

        let sc = SymbolsContext {
            plans: &self.fms.plans,
            geometries: &self.guidance.geometries,
            calculated: &self.guidance.calculated,
            inputs,
            phase,
            perf: self.fms.perf.get(0),
            vnav: &self.vnav,
            pwps: &self.pwp.current,
            navaids: &self.navaids,
            facilities: self.facilities.as_ref(),
        };
        for side in 0..2 {
            self.symbols[side].update(dt_ms, &sc, &self.ifaces[side], &mut self.sink);
            self.guidance.data_limit_reached[side] = self.symbols[side].data_limit_reached;
        }
        self.fms_cycle_ms += dt_ms;
        if self.fms_cycle_ms >= FMS_CYCLE_TIME_MS {
            let cycle_ms = self.fms_cycle_ms;
            self.fms_cycle_ms = 0.;
            self.fms_cycle(cycle_ms, inputs, phase);
        }
        self.sync.update(&self.fms.plans, &self.fms.perf.plans, &mut self.sink);
        self.sink.take()
    }

    fn drain_perf_events(&mut self) {
        let events = std::mem::take(&mut self.fms.perf_events);
        self.handle_perf_events(events, false);
    }

    fn run_sync_perf(&mut self) {
        let events = self.fms.sync_perf();
        self.handle_perf_events(events, false);
    }

    /// The host side of the performance data events: messages, `L:A32NX_AIRLINER_TO_FLEX_TEMP` and `FmcAircraftInterface.onUpdateCruiseLevel`.
    fn handle_perf_events(&mut self, events: Vec<fms_data::Event>, from_update: bool) {
        for e in events {
            match e {
                fms_data::Event::Message(m) => self.sink.out.messages.push(m.text()),
                fms_data::Event::Effect(fms_data::Effect::AirlinerFlexTemp(v)) => self.sink.set("A32NX_AIRLINER_TO_FLEX_TEMP", v),
                fms_data::Event::Effect(fms_data::Effect::CruiseLevelUpdated(fl)) => self.on_update_cruise_level(fl),
                fms_data::Event::Changed("pilotAccelerationAltitude") if from_update => {
                    if let Some(alt) = self.fms.perf.get(0).and_then(|d| d.acceleration_altitude()) {
                        self.sink.out.messages.push(format!("NEW ACCEL ALT: {alt:.0}"));
                    }
                }
                fms_data::Event::Changed("pilotThrustReductionAltitude") if from_update => {
                    if let Some(alt) = self.fms.perf.get(0).and_then(|d| d.thrust_reduction_altitude()) {
                        self.sink.out.messages.push(format!("NEW THR RED ALT: {alt:.0}"));
                    }
                }
                _ => {}
            }
        }
    }

    fn on_update_cruise_level(&mut self, fl: f64) {
        self.cruise_latch = Some(fl * 100.);
        self.sink.set("A32NX_AIRLINER_CRUISE_ALTITUDE", fl * 100.);
        let phase = self.phase.phase();
        let fcu_alt = self.last.prim_selected_altitude().unwrap_or(self.last.fcu_selected_altitude_ft);
        self.vnav.update_constraints(self.fms.active(), phase, fcu_alt, &mut self.sink);
        let pi = self.current_phase_inputs();
        self.phase.handle_new_cruise_altitude_entered(fl, &pi);
    }

    fn update_perf_state(&mut self, inputs: &Inputs) {
        let phase = self.phase.phase();
        self.engines_were_started = phase >= FlightPhase::Takeoff || (phase == FlightPhase::Preflight && inputs.n2.iter().any(|n| *n > ENGINE_ON_N2));
        let isa = inputs.ambient_temperature_c - (15. - 0.0019812 * inputs.indicated_altitude_ft.min(36_089.));
        let gw = self.gross_weight_kg(inputs);
        self.fms.perf_state = PerfState {
            phase: entry_phase(phase),
            engines_started: self.engines_were_started,
            fcu_altitude_ft: inputs.prim_selected_altitude(),
            max_zfw_kg: None,
            rec_max_flight_level: weights::rec_max_flight_level(gw, isa),
        };
    }

    fn gross_weight_kg(&self, inputs: &Inputs) -> Option<f64> {
        let pd = self.fms.perf.get(0);
        let engines_on = inputs.n2.iter().any(|n| *n > ENGINE_ON_N2);
        let fob = weights::fuel_on_board_tonnes(engines_on, inputs.fuel_on_board_kg_fqms(), pd.and_then(|d| d.block_fuel));
        weights::gross_weight_kg(inputs.fqms_gross_weight_kg, pd.and_then(|d| d.zero_fuel_weight), fob)
    }

    fn process_transitions(&mut self, db: &dyn NavDatabase) {
        for _ in 0..8 {
            let changes = self.phase.take_transitions();
            if changes.is_empty() {
                break;
            }
            for c in changes {
                self.on_phase_changed(db, c.previous, c.next);
            }
        }
        if self.phase.take_flex_temp_cleared() {
            self.sink.set("A32NX_AIRLINER_TO_FLEX_TEMP", 0.);
        }
        if self.phase.take_cold_and_dark_cleared() {
            self.sink.set("A32NX_COLD_AND_DARK_SPAWN", 0.);
        }
    }

    /// `FlightManagementComputer.onFlightPhaseChanged`.
    fn on_phase_changed(&mut self, db: &dyn NavDatabase, prev: FlightPhase, next: FlightPhase) {
        self.sink.set("A32NX_CABIN_READY", 0.);
        self.is_reset = false;
        let indicated = self.last.indicated_altitude_ft;
        let has_active = self.fms.active().is_some();
        match next {
            FlightPhase::Takeoff => {
                self.dest_data_checked = false;
                if has_active {
                    let pd = self.fms.perf.ensure(0);
                    if pd.acceleration_altitude().is_none_or(|v| v == 0.) {
                        pd.pilot_acceleration_altitude = Some(indicated + CONFIG_ACCEL_ALT_FT);
                    }
                    if pd.engine_out_acceleration_altitude().is_none_or(|v| v == 0.) {
                        pd.pilot_engine_out_acceleration_altitude = Some(indicated + CONFIG_ACCEL_ALT_FT);
                    }
                    pd.pilot_taxi_fuel = None;
                    pd.pilot_route_reserve_fuel = None;
                    pd.pilot_route_reserve_fuel_percentage = Some(0.);
                    let climb = pd.preselected_climb_speed;
                    self.run_sync_perf();
                    if climb.is_some_and(|v| v != 0.) {
                        self.set_preselected(climb);
                    }
                }
            }
            FlightPhase::Climb => {
                self.dest_data_checked = false;
                if has_active {
                    let pd = self.fms.perf.ensure(0);
                    let (climb, cruise) = (pd.preselected_climb_speed.filter(|v| *v != 0.), pd.preselected_cruise_speed);
                    if prev == FlightPhase::Takeoff && climb.is_some() {
                        self.sink.key("A32NX.FMS_PRESET_SPD_ACTIVATE", 1.);
                    }
                    self.set_preselected(cruise);
                    let have_cruise = self.cruise_live().is_some_and(|v| v != 0.);
                    if let (Some(alt), false) = (self.last.prim_selected_altitude(), have_cruise) {
                        self.fms.perf.ensure(0).cruise_flight_level = Some(alt / 100.);
                        self.cruise_latch = Some(alt);
                        self.sink.set("A32NX_AIRLINER_CRUISE_ALTITUDE", alt);
                        self.run_sync_perf();
                    }
                }
            }
            FlightPhase::Cruise => {
                self.dest_data_checked = false;
                let cruise = self.fms.perf.get(0).and_then(|d| d.preselected_cruise_speed).filter(|v| *v != 0.);
                if prev == FlightPhase::Climb && cruise.is_some() {
                    self.sink.key("A32NX.FMS_PRESET_SPD_ACTIVATE", 1.);
                }
            }
            FlightPhase::Descent => {
                self.check_dest_data();
                self.trigger_check_speed_mode();
                self.set_preselected(None);
                if has_active {
                    self.fms.perf.ensure(0).cruise_flight_level = None;
                    self.run_sync_perf();
                }
            }
            FlightPhase::Approach => self.check_dest_data(),
            FlightPhase::GoAround => {
                self.sink.set("A32NX_GOAROUND_INIT_SPEED", self.last.cas_kt.unwrap_or(0.));
                if let Some(plan) = self.fms.plans[0].as_mut() {
                    let ident = plan
                        .all_legs()
                        .iter()
                        .filter_map(|e| e.leg())
                        .find(|l| l.definition.approach_waypoint_descriptor == Some(fms::plan::APPROACH_DESCRIPTOR_MISSED_APPROACH_POINT))
                        .map(|l| l.ident.clone());
                    let version = plan.version;
                    let _ = plan.string_missed_approach(db);
                    if let (Some(ident), true) = (ident, plan.version != version) {
                        self.sink.out.messages.push(format!("CONSTRAINTS BEFORE {ident} : DELETED"));
                    }
                }
                if has_active {
                    let pd = self.fms.perf.ensure(0);
                    if pd.missed_acceleration_altitude().is_none_or(|v| v == 0.) {
                        pd.pilot_missed_acceleration_altitude = Some(indicated + CONFIG_ACCEL_ALT_FT);
                    }
                    if pd.missed_engine_out_acceleration_altitude().is_none_or(|v| v == 0.) {
                        pd.pilot_missed_engine_out_acceleration_altitude = Some(indicated + CONFIG_ACCEL_ALT_FT);
                    }
                    self.run_sync_perf();
                }
            }
            FlightPhase::Done => {
                self.reset_plans();
                self.sink.set("A32NX_COLD_AND_DARK_SPAWN", 1.);
                self.is_reset = true;
            }
            FlightPhase::Preflight => {}
        }
    }

    /// `FlightPlanService.reset` with `FmgcData.reset`.
    fn reset_plans(&mut self) {
        self.fms.delete_all();
        self.fms.perf = PerfDataStore::default();
        for r in [PlanRef::Active, PlanRef::Uplink] {
            self.fms.create(r);
        }
        self.cruise_latch = None;
        self.engine_out = false;
        self.engine_failure_prev = false;
        self.dest_data_checked = false;
        self.zfw_init_displayed = 0;
        self.zfw_init_settable = false;
        self.step_ahead_altitude = None;
    }

    fn check_dest_data(&mut self) {
        let entered = self.fms.perf.get(0).is_some_and(|d| d.approach_qnh.is_some() && d.approach_temperature.is_some() && d.approach_wind_direction.is_some() && d.approach_wind_magnitude.is_some());
        if !entered {
            self.sink.out.messages.push("ENTER DEST DATA".to_string());
        }
    }

    fn trigger_check_speed_mode(&mut self) {
        if !self.check_speed_mode && !self.last.fcu_speed_managed {
            self.check_speed_mode = true;
            self.sink.out.messages.push("CHECK SPD MODE".to_string());
            self.sink.set_bool("A32NX_PFD_MSG_CHECK_SPEED_MODE", true);
        }
    }

    fn on_step_climb_descent(&mut self) {
        let phase = self.phase.phase();
        if !matches!(phase, FlightPhase::Climb | FlightPhase::Cruise) {
            return;
        }
        let Some(alt) = self.last.prim_selected_altitude() else { return };
        if self.fms.active().is_none() {
            return;
        }
        let target = alt / 100.;
        let cruise = self.fms.perf.get(0).and_then(|d| d.cruise_flight_level);
        let climb = phase == FlightPhase::Climb;
        if !((climb && target > cruise.unwrap_or(0.)) || (!climb && Some(target) != cruise)) {
            return;
        }
        if let Some(old) = cruise {
            self.delete_outdated_cruise_steps(old, target);
        }
        self.sink.out.messages.push(fms_data::FmsMessage::NewCrzAlt(alt).text());
        self.fms.perf.ensure(0).cruise_flight_level = Some(target);
        self.cruise_latch = Some(alt);
        self.sink.set("A32NX_AIRLINER_CRUISE_ALTITUDE", alt);
        self.run_sync_perf();
    }

    fn delete_outdated_cruise_steps(&mut self, old_level: f64, new_level: f64) {
        let climb = new_level > old_level;
        let Some(plan) = self.fms.plans[0].as_mut() else { return };
        let active = plan.active_leg_index.max(0) as usize;
        let remove: Vec<usize> = plan
            .all_legs()
            .iter()
            .enumerate()
            .skip(active)
            .filter_map(|(i, e)| {
                let step = e.leg()?.cruise_step.as_ref()?;
                let level = (step.to_altitude / 100.).round();
                ((climb && level >= old_level && level <= new_level) || (!climb && level <= old_level && level >= new_level)).then_some(i)
            })
            .collect();
        for i in remove {
            plan.remove_cruise_step(i);
        }
    }

    fn enter_engine_out(&mut self) {
        let Some(plan) = self.fms.plans[0].as_mut() else { return };
        let pd = self.fms.perf.ensure(0);
        pd.preselected_climb_speed = None;
        pd.preselected_cruise_speed = None;
        let active = plan.active_leg_index.max(0) as usize;
        let steps: Vec<usize> = plan.all_legs().iter().enumerate().skip(active).filter(|(_, e)| e.leg().is_some_and(|l| l.cruise_step.is_some())).map(|(i, _)| i).collect();
        for i in &steps {
            plan.remove_cruise_step(*i);
        }
        if !steps.is_empty() {
            self.sink.out.messages.push("STEP DELETED".to_string());
        }
        self.run_sync_perf();
    }

    fn check_engine_out(&mut self, inputs: &Inputs, phase: FlightPhase) {
        let all_on = inputs.n2.iter().all(|n| *n > ENGINE_ON_N2);
        let failing = phase >= FlightPhase::Takeoff && phase < FlightPhase::Done && !all_on;
        let rising = failing && !self.engine_failure_prev;
        self.engine_failure_prev = failing;
        if !self.engine_out && rising {
            self.engine_out = true;
            self.enter_engine_out();
        }
        if self.engine_out && all_on {
            self.engine_out = false;
        }
    }

    /// The 250 ms part of `FlightManagementComputer.onUpdate`.
    fn fms_cycle(&mut self, dt_ms: f64, inputs: &Inputs, phase: FlightPhase) {
        if self.fms.active().is_none() {
            return;
        }
        self.update_weights(inputs);
        let (_, events) = self.fms.perf_update();
        self.handle_perf_events(events, true);
        self.update_ils_course(inputs);
        self.check_for_cruise_step();
        self.check_too_steep_path(phase);
        self.check_dest_efob_below_min(dt_ms, inputs, phase);
        self.check_engine_out(inputs, phase);
        self.check_lateral_discontinuity();
        self.update_holding_speed(inputs, phase);
        self.check_zfw_params(inputs, phase);
        self.check_speed_limit(inputs, phase);
        self.check_dest_data_in_cruise(dt_ms, phase);
        self.update_approach_outputs();
    }

    /// `FmcAircraftInterface.updateWeights`.
    fn update_weights(&mut self, inputs: &Inputs) {
        let gw = self.gross_weight_kg(inputs);
        self.sink.set("A32NX_FM_GROSS_WEIGHT", gw.unwrap_or(0.));
        if !inputs.cold_and_dark_spawn && inputs.airframe_zfw_kg > 0. && inputs.airframe_zfw_cg_percent > 0. {
            let pd = self.fms.perf.ensure(0);
            if pd.zero_fuel_weight.is_none_or(|z| z == 0.) && pd.zero_fuel_weight_center_of_gravity.is_none_or(|c| c == 0.) {
                pd.zero_fuel_weight = Some(inputs.airframe_zfw_kg / 1_000.);
                pd.zero_fuel_weight_center_of_gravity = Some(inputs.airframe_zfw_cg_percent);
            }
        }
    }

    /// `FmcAircraftInterface.updateIlsCourse`.
    fn update_ils_course(&mut self, inputs: &Inputs) {
        let mmr = &self.navaids.mmr[0];
        let course = match (mmr.course, mmr.frequency) {
            (Some(c), _) => c,
            (None, Some(_)) if inputs.loc_receiver_valid => inputs.nav_localizer_course,
            _ => -1.,
        };
        self.sink.set("A32NX_FM_LS_COURSE", course);
    }

    fn check_for_cruise_step(&mut self) {
        let Some(plan) = self.fms.active() else { return };
        let active = plan.active_leg_index.max(0) as usize;
        let all = plan.all_legs();
        let step = all.iter().enumerate().skip(active).find_map(|(i, e)| e.leg().and_then(|l| l.cruise_step.clone()).map(|s| (i, s)));
        let Some((index, step)) = step else { return };
        if step.is_ignored || index == 0 {
            return;
        }
        let near = self.vnav.predictions.get(&index).is_some_and(|p| p.distance_from_aircraft < 20.);
        if near && self.step_ahead_altitude != Some(step.to_altitude) {
            self.sink.out.messages.push("STEP AHEAD".to_string());
            self.step_ahead_altitude = Some(step.to_altitude);
        }
    }

    fn check_too_steep_path(&mut self, phase: FlightPhase) {
        let Some(plan) = self.fms.active() else { return };
        let (active, end) = (plan.active_leg_index.max(0) as usize, plan.first_missed_approach_leg_index());
        let close = (phase == FlightPhase::Cruise && self.vnav.driver.constraint_reader.distance_to_end < 150.) || matches!(phase, FlightPhase::Descent | FlightPhase::Approach);
        let steep = self.vnav.modes.is_lat_auto_control_active() && close && self.vnav.driver.too_steep_path_legs.iter().any(|i| *i >= active && *i < end);
        if steep && !self.too_steep {
            self.sink.out.messages.push("TOO STEEP PATH AHEAD".to_string());
        }
        self.too_steep = steep;
    }

    fn check_dest_efob_below_min(&mut self, dt_ms: f64, inputs: &Inputs, phase: FlightPhase) {
        let Some(plan) = self.fms.active() else { return };
        let efob_t = plan.destination_leg_index().and_then(|i| self.vnav.predictions.get(&i)).map(|p| p.estimated_fuel_on_board * LB_TO_KG / 1000.);
        let min_t = self.fms.perf.get(0).and_then(|d| d.minimum_destination_fuel_on_board());
        let below = match (efob_t, min_t) {
            (Some(e), Some(m)) => {
                if self.dest_efob_below_min {
                    e - m <= 0.3
                } else {
                    e < m
                }
            }
            _ => false,
        };
        self.dest_efob_below_min = below;
        self.sink.set_bool("A380X_FMS_DEST_EFOB_BELOW_MIN", below);
        let alt_active = self.alt_active_in_climb.update(phase == FlightPhase::Climb && inputs.word3_bit(20), dt_ms);
        let show = below
            && (matches!(phase, FlightPhase::Cruise | FlightPhase::Descent) || alt_active || (matches!(phase, FlightPhase::Approach | FlightPhase::GoAround) && inputs.radio_height_ft.unwrap_or(0.) > 800.));
        if show && !self.dest_efob_message {
            self.sink.out.messages.push("DEST EFOB BELOW MIN".to_string());
        }
        self.dest_efob_message = show;
    }

    fn check_lateral_discontinuity(&mut self) {
        let show = self.lateral_discontinuity_ahead();
        if show && !self.lat_discontinuity {
            self.sink.out.messages.push("LATERAL DISCONTINUITY AHEAD".to_string());
        }
        self.lat_discontinuity = show;
    }

    fn lateral_discontinuity_ahead(&self) -> bool {
        let Some(plan) = self.fms.active() else { return false };
        let active = plan.active_leg_index.max(0) as usize;
        let Some(active_leg) = plan.leg_at(active) else { return false };
        let all = plan.all_legs();
        let last = (active..all.len())
            .find(|i| all.get(i + 1).is_none_or(|n| n.is_disco()))
            .and_then(|i| all.get(i).and_then(|e| e.leg()).map(|l| (i, l.is_vectors())));
        let prediction = last.and_then(|(i, _)| self.vnav.predictions.get(&i)).map(|p| p.seconds_from_present);
        fms_vnav::driver::should_show_lat_discontinuity_ahead(&self.vnav.modes, active_leg.leg_type == LegType::HM, last, active, prediction, self.guidance.active_leg_seconds_to_go)
    }

    fn holding_speed_for(&self, leg_index: usize, inputs: &Inputs, phase: FlightPhase) -> f64 {
        let point = self.vnav.managed_point(leg_index);
        let pd = self.fms.perf.get(0);
        let limit = |s: Option<f64>, a: Option<f64>| match (s, a) {
            (Some(speed), Some(under_altitude)) if speed != 0. && under_altitude != 0. => Some(SpeedLimit { speed, under_altitude }),
            _ => None,
        };
        holding_speed(&HoldingSpeedInputs {
            fcu_altitude_ft: inputs.fcu_selected_altitude_ft,
            altitude_ft: point.map(|p| p.descent_altitude).filter(|a| a.is_finite()),
            green_dot_kt: self.speeds.green_dot.unwrap_or(0.),
            all_engines_on: inputs.n2.iter().all(|n| *n > ENGINE_ON_N2),
            phase,
            approach_managed_speed_kt: approach_managed_speed(inputs.flaps_handle, pd.is_some_and(|d| d.approach_flaps_three_selected), self.speeds.green_dot, self.speeds.slat_retraction, self.speeds.flap_retraction, self.speeds.approach_vapp),
            speed_constraint_kt: point.map(|p| p.descent_speed).filter(|s| s.is_finite()),
            climb_speed_limit: limit(pd.and_then(|d| d.climb_speed_limit_speed), pd.and_then(|d| d.climb_speed_limit_altitude)),
            descent_speed_limit: limit(pd.and_then(|d| d.descent_speed_limit_speed), pd.and_then(|d| d.descent_speed_limit_altitude)),
            vls_kt: self.speeds.vls,
        })
    }

    /// `FmcAircraftInterface.updateHoldingSpeed`.
    fn update_holding_speed(&mut self, inputs: &Inputs, phase: FlightPhase) {
        let Some(plan) = self.fms.active() else { return };
        let active = plan.active_leg_index.max(0) as usize;
        let current_hold = plan.leg_at(active).is_some_and(|l| l.leg_type == LegType::HM);
        let next_hold = plan.leg_at(active + 1).is_some_and(|l| l.leg_type == LegType::HM);
        let cas = inputs.cas_kt.unwrap_or(0.);
        let speed_selected = !inputs.fcu_speed_managed;
        let mut target = None;
        let mut decel = self.hold.decel_reached;
        let mut warn = false;
        if current_hold {
            target = Some(self.holding_speed_for(active, inputs, phase));
            decel = true;
            warn = speed_selected;
        } else if next_hold {
            if inputs.gps_irs_available() {
                let t = self.holding_speed_for(active + 1, inputs, phase);
                target = Some(t);
                let dtg = self.guidance.active_leg_dtg;
                let decel_dist = calculate_decel_dist(cas, t);
                if dtg.is_some_and(|d| d < decel_dist) {
                    decel = true;
                }
                warn = speed_selected && dtg.is_some_and(|d| d <= decel_dist + inputs.ground_speed_kt / 120.);
            }
        } else {
            decel = false;
        }
        self.hold.decel_reached = decel;
        self.hold.speed_target = target;
        self.sink.set_bool("A32NX_FM_HOLD_DECEL", decel);
        self.sink.set("A32NX_FM_HOLD_SPEED", target.unwrap_or(0.));
        if warn && decel && target.is_some_and(|t| cas - t > 5.) {
            if !self.hold.message_active {
                self.hold.message_active = true;
                self.sink.out.messages.push("SET HOLD SPD".to_string());
                self.sink.set_bool("A32NX_PFD_MSG_SET_HOLD_SPEED", true);
            }
        } else if self.hold.message_active {
            self.hold.message_active = false;
            self.sink.set_bool("A32NX_PFD_MSG_SET_HOLD_SPEED", false);
        }
    }

    /// `GuidanceController.setHoldSpeed`, fed by `VnavDriver.updateHoldSpeed`.
    fn apply_hold_speed(&mut self) {
        let Some(plan) = self.fms.active() else { return };
        let active = plan.active_leg_index.max(0) as usize;
        let hold = plan.leg_at(active).is_some_and(|l| l.leg_type == LegType::HM) || plan.leg_at(active + 1).is_some_and(|l| l.leg_type == LegType::HM);
        if let (true, Some(tas), Some(g)) = (hold, self.vnav.driver.out.hold_speed_tas, self.guidance.geometries.get_mut(&(0, false))) {
            g.set_hold_speed(active, tas);
        }
    }

    fn legs_with_representation(&self) -> Vec<(usize, bool)> {
        self.guidance.geometries.get(&(0, false)).map(|g| (0..g.leg_count()).filter(|i| self.guidance.is_leg((0, false), *i)).map(|i| (i, true)).collect()).unwrap_or_default()
    }

    /// `VnavDriver.updateLegSpeedPredictions`.
    fn update_leg_speed_predictions(&mut self, inputs: &Inputs) {
        let Some(plan) = self.fms.active() else { return };
        let Some(params) = self.vnav.params.as_ref() else { return };
        let Some(calc) = self.guidance.calculated.get(&(0, false)) else { return };
        let legs: Vec<(usize, Option<f64>)> = (0..calc.len().max(plan.leg_count()))
            .filter(|i| self.guidance.is_leg((0, false), *i))
            .map(|i| (i, calc.get(i).and_then(|c| c.as_ref()).map(|c| c.cumulative_distance_with_transitions)))
            .collect();
        let predictions = self.vnav.driver.leg_speed_predictions(params, &legs, plan.active_leg_index.max(0) as usize, inputs.ambient_wind_speed_kt);
        if let Some(g) = self.guidance.geometries.get_mut(&(0, false)) {
            for (i, p) in predictions {
                g.set_predictions(i, p.map(|x| x.0), p.map(|x| x.1));
            }
        }
    }

    fn track_td_reached(&mut self) {
        let reached = self.sink.written.get("A32NX_PFD_MSG_TD_REACHED").is_some_and(|v| *v != 0.);
        if reached && !self.td_reached {
            self.sink.out.messages.push("T/D REACHED".to_string());
        }
        self.td_reached = reached;
    }

    fn check_zfw_params(&mut self, inputs: &Inputs, phase: FlightPhase) {
        if (inputs.engine_state[0] == 2. || inputs.engine_state[1] == 2.) && self.zfw_init_displayed < 1 && phase < FlightPhase::Takeoff {
            self.zfw_init_settable = true;
        }
        let pd = self.fms.perf.get(0);
        let undefined = pd.is_none_or(|d| d.zero_fuel_weight.is_none() || d.zero_fuel_weight_center_of_gravity.is_none());
        if inputs.n2.iter().any(|n| *n > ENGINE_ON_N2) && undefined && self.zfw_init_settable {
            self.sink.out.messages.push("INITIALIZE ZFW / ZFWCG".to_string());
            self.zfw_init_displayed += 1;
            self.zfw_init_settable = false;
        }
    }

    fn check_speed_limit(&mut self, inputs: &Inputs, phase: FlightPhase) {
        let pd = self.fms.perf.get(0);
        let limit = match phase {
            FlightPhase::Climb | FlightPhase::Cruise => pd.and_then(|d| d.climb_speed_limit_speed.zip(d.climb_speed_limit_altitude)),
            FlightPhase::Descent => pd.and_then(|d| d.descent_speed_limit_speed.zip(d.descent_speed_limit_altitude)),
            _ => None,
        };
        let Some((speed, limit_alt)) = limit.filter(|(s, a)| *s != 0. && *a != 0.) else {
            self.speed_limit_exceeded = false;
            return;
        };
        let (cas, alt) = (inputs.cas_kt, inputs.baro_altitude_ft);
        if self.speed_limit_exceeded {
            let reset = match (cas, alt) {
                (Some(c), Some(a)) => a > limit_alt || c <= speed + 5.,
                _ => true,
            };
            if reset {
                self.speed_limit_exceeded = false;
            }
        } else if let (Some(c), Some(a)) = (cas, alt) {
            if a < limit_alt - 150. && c > speed + 10. {
                self.speed_limit_exceeded = true;
                self.sink.out.messages.push("SPD LIMIT EXCEEDED".to_string());
            }
        }
    }

    fn check_dest_data_in_cruise(&mut self, dt_ms: f64, phase: FlightPhase) {
        self.dest_check_ms += dt_ms;
        if self.dest_check_ms < DEST_DATA_CHECK_PERIOD_MS {
            return;
        }
        self.dest_check_ms = 0.;
        if phase != FlightPhase::Cruise || self.dest_data_checked {
            return;
        }
        let distance = self.fms.active().and_then(|p| p.destination_leg_index()).and_then(|i| self.vnav.predictions.get(&i)).map(|p| p.distance_from_aircraft);
        if distance.is_some_and(|d| d.is_finite() && d < 180.) {
            self.dest_data_checked = true;
            self.check_dest_data();
        }
    }

    /// `A380X_FM_LANDING_CONF3` and the approach headwind component ARINC word.
    fn update_approach_outputs(&mut self) {
        let pd = self.fms.perf.get(0);
        self.sink.set_bool("A380X_FM_LANDING_CONF3", pd.is_some_and(|d| d.approach_flaps_three_selected));
        let headwind = self.fms.active().and_then(|p| {
            let runway = p.destination_runway.as_ref()?;
            let d = pd?;
            Some(fms_perf::speeds::SpeedsUtils::headwind(d.approach_wind_magnitude?, d.approach_wind_direction?, runway.magnetic_bearing))
        });
        let ssm: f64 = if headwind.is_some() { 3. } else { 1. };
        self.sink.set("A380X_FM_APPROACH_HEADWIND_COMPONENT", ssm * 4_294_967_296. + (headwind.unwrap_or(0.) as f32).to_bits() as f64);
    }

    fn update_navaids(&mut self, dt_ms: f64, inputs: &Inputs, phase: FlightPhase) {
        let epe = if inputs.gps_irs_available() { 0.04 + inputs.ground_speed_kt * 0.00015 } else { f64::INFINITY };
        let ni = NavInputs {
            plan: self.fms.active(),
            phase,
            ppos: Coord::new(inputs.lat, inputs.lon),
            epe,
            altitude_ft: inputs.baro_altitude_ft,
            height_above_ground_ft: inputs.altitude_above_ground_ft,
            radio_height_ft: inputs.radio_height_ft,
            rmp_nav_selected: inputs.rmp_nav_selected,
        };
        self.navaids.update(dt_ms, &ni, &mut self.sink);
    }

    /// FBW `GuidanceController.updateGeometries` for the EOSID: rebuilt only when the procedure changes.
    fn update_eosid_geometry(&mut self, inputs: &Inputs) {
        let Some(plan) = self.fms.active().filter(|p| !p.engine_out_legs().is_empty()) else {
            self.eosid = None;
            return;
        };
        let version = plan.engine_out_departure.version;
        if self.eosid.as_ref().is_some_and(|e| e.0 == version) {
            return;
        }
        let legs = plan.engine_out_legs();
        let mut g = Geometry::engine_out(&elements_from(legs, legs.len()));
        let ppos = Coord::new(inputs.lat, inputs.lon);
        g.recompute_engine_out(ppos, inputs.true_track);
        g.recompute_engine_out(ppos, inputs.true_track);
        self.eosid = Some((version, g));
    }

    fn vnav_context<'a>(&'a self, inputs: &'a Inputs, phase: FlightPhase) -> VnavContext<'a> {
        VnavContext {
            plan: self.fms.active(),
            calc: self.guidance.calculated.get(&(0, false)).map(|v| v.as_slice()),
            perf: self.fms.perf.get(0),
            speeds: &self.speeds,
            phase,
            inputs,
            distance_to_end: self.guidance.along_track_distance_to_destination(&self.fms, inputs),
            has_temporary: self.fms.has_temporary(),
            engines_running: inputs.n2.iter().any(|n| *n > ENGINE_ON_N2),
            managed_pfd: self.managed_pfd,
            manual_hold_active: self.fms.active().is_some_and(|p| p.leg_at(p.active_leg_index.max(0) as usize).is_some_and(|l| l.leg_type == LegType::HM)),
            manual_hold_next: self.fms.active().is_some_and(|p| p.leg_at(p.active_leg_index.max(0) as usize + 1).is_some_and(|l| l.leg_type == LegType::HM)),
            hold_speed: self.hold.speed_target.unwrap_or(0.),
            hold_decel_reached: self.hold.decel_reached,
        }
    }

    fn pwp_context<'a>(&'a self, inputs: &Inputs, phase: FlightPhase, efis: [(i32, f64); 2]) -> PwpContext<'a> {
        let plan = self.fms.active();
        let next_speed_change = self.vnav.params.as_ref().and_then(|p| self.vnav.driver.find_next_speed_change(p, &self.vnav.modes, self.managed_pfd));
        PwpContext {
            geometry: self.guidance.geometries.get(&(0, false)),
            calc: self.guidance.calculated.get(&(0, false)).map(|v| v.as_slice()),
            has_active: plan.is_some(),
            first_missed: plan.map_or(0, |p| p.first_missed_approach_leg_index()),
            active_leg_index: plan.map_or(0, |p| p.active_leg_index.max(0) as usize),
            mcdu: self.vnav.mcdu_profile(),
            nd: self.vnav.driver.nd_profile.as_ref(),
            modes: self.vnav.modes,
            next_speed_change,
            airspeed_managed: inputs.fcu_speed_managed,
            phase_preflight: phase == FlightPhase::Preflight,
            phase_descent: phase == FlightPhase::Descent,
            efis: efis.map(|(mode, range)| EfisState { mode, range }),
        }
    }

    fn write_vnav_defaults(&mut self) {
        for v in ["A32NX_FG_REQUESTED_VERTICAL_MODE", "A32NX_FG_TARGET_ALTITUDE", "A32NX_FG_TARGET_VERTICAL_SPEED", "A32NX_PFD_TARGET_ALTITUDE", "A32NX_PFD_LINEAR_DEVIATION_ACTIVE", "A32NX_PFD_VERTICAL_PROFILE_LATCHED", "A32NX_FG_ALTITUDE_CONSTRAINT", "A32NX_PFD_MSG_SET_HOLD_SPEED"] {
            self.sink.set(v, 0.);
        }
        for t in ["a32nx_fms_vertical_target_profile", "a32nx_fms_vertical_constraints", "a32nx_fms_vertical_actual_profile", "a32nx_fms_vertical_descent_profile", "vdSymbols_L", "vdSymbols_R"] {
            self.sink.bus(t, json!([]));
        }
        self.sink.bus("a32nx_fms_vd_track_change_distance", Value::Null);
    }

    fn write_perf(&mut self, inputs: &Inputs, phase: FlightPhase, dt_ms: f64) {
        let pd = self.fms.perf.get(0).cloned();
        let active = self.fms.active();
        let dest_elev = active
            .and_then(|p| p.destination_runway.as_ref().and_then(|r| r.threshold_location.alt).or(p.destination_airport.as_ref().and_then(|a| a.location.alt)))
            .unwrap_or(0.);
        let engines_on = inputs.n2.iter().any(|n| *n > ENGINE_ON_N2);
        let fob_t = weights::fuel_on_board_tonnes(engines_on, inputs.fuel_on_board_kg_fqms(), pd.as_ref().and_then(|d| d.block_fuel));
        let gw_kg = weights::gross_weight_kg(inputs.fqms_gross_weight_kg, pd.as_ref().and_then(|d| d.zero_fuel_weight), fob_t);
        let dest_efob_kg = active.and_then(|p| self.vnav.predictions.get(&p.destination_leg_index()?)).map(|d| d.estimated_fuel_on_board * LB_TO_KG);
        let trip_kg = fob_t.zip(dest_efob_kg).map(|(f, d)| f * 1000. - d);
        let landing_kg = weights::landing_weight_kg(&LandingWeightInputs {
            takeoff_weight_kg: pd.as_ref().and_then(|d| d.takeoff_weight),
            gross_weight_kg: gw_kg,
            trip_fuel_kg: trip_kg,
            engines_were_started: self.engines_were_started,
            phase,
            taxi_fuel_tonnes: pd.as_ref().map(|d| d.taxi_fuel()),
        });
        let conf3 = pd.as_ref().is_some_and(|d| d.approach_flaps_three_selected);
        let speeds = self.perf.update_perf_speeds(&PerfInputs {
            phase,
            pressure_altitude_ft: inputs.pressure_altitude_ft,
            computed_airspeed_kt: inputs.cas_kt,
            flap_lever: inputs.flaps_handle,
            left_spoiler_1_position: inputs.left_spoiler_1,
            static_pressure_hpa: inputs.static_pressure_hpa,
            destination_elevation_ft: dest_elev,
            fuel_on_board_tonnes: fob_t,
            gross_weight_kg: gw_kg,
            gross_weight_cg: inputs.fqms_cg_percent,
            estimated_landing_weight_kg: landing_kg,
            approach_wind_direction_deg: pd.as_ref().and_then(|d| d.approach_wind_direction),
            approach_wind_magnitude_kt: pd.as_ref().and_then(|d| d.approach_wind_magnitude),
            destination_runway_magnetic_bearing_deg: active.and_then(|p| p.destination_runway.as_ref()).map(|r| r.magnetic_bearing),
            approach_flaps_three_selected: conf3,
            v2_kt: pd.as_ref().and_then(|d| d.v2),
            pilot_vapp_kt: pd.as_ref().and_then(|d| d.pilot_vapp),
            ..PerfInputs::default()
        });
        for v in perf_speed_lvars(&speeds) {
            self.sink.set(v.name, v.value);
        }
        self.speeds = speeds;
        let cost_index = pd.as_ref().and_then(|d| d.cost_index).or(active.and_then(|p| p.performance.cost_index));
        let managed_descent = fms_perf::cost_index::managed_descent_speed(cost_index, pd.as_ref().and_then(|d| d.pilot_managed_descent_speed));
        let leg_constraints = active
            .and_then(|p| self.vnav.managed_point(p.active_leg_index.max(0) as usize))
            .map(|p| LegSpeedConstraints { climb_speed: p.climb_speed, descent_speed: p.descent_speed, previous_descent_speed: p.previous_descent_speed });
        let lateral_managed = matches!(inputs.lateral_mode, 20 | 30 | 31 | 32 | 33 | 34);
        let managed = update_managed_speed(&ManagedSpeedInputs {
            phase,
            hold_decel_reached: self.hold.decel_reached,
            hold_speed_target: self.hold.speed_target,
            v2_kt: pd.as_ref().and_then(|d| d.v2),
            cost_index,
            pilot_descent_speed_kt: pd.as_ref().and_then(|d| d.pilot_managed_descent_speed),
            pilot_descent_mach: pd.as_ref().and_then(|d| d.pilot_managed_descent_mach),
            green_dot_kt: speeds.green_dot,
            cas_kt: inputs.cas_kt,
            approach_vapp_kt: speeds.approach_vapp,
            static_pressure_hpa: inputs.static_pressure_hpa,
            indicated_altitude_ft: inputs.indicated_altitude_ft,
            speed_constraint_kt: speed_constraint(lateral_managed, phase, leg_constraints, self.guidance.active_leg_dtg, managed_descent),
            engine_out: inputs.engine_out,
            open_or_managed_climb_active: inputs.word3_bit(11) || inputs.word3_bit(13),
            fms_cycle_time_ms: dt_ms.clamp(0., FMS_CYCLE_TIME_MS),
            climb_speed_limit: pd.as_ref().and_then(|d| match (d.climb_speed_limit_speed, d.climb_speed_limit_altitude) {
                (Some(speed), Some(under_altitude)) if speed != 0. && under_altitude != 0. => Some(SpeedLimit { speed, under_altitude }),
                _ => None,
            }),
            vnav_managed_pfd_kt: self.vnav.driver.out.speeds_managed_pfd.unwrap_or(self.managed_pfd),
        });
        if managed.is_mach != self.managed_is_mach {
            self.managed_is_mach = managed.is_mach;
            self.sink.key(if managed.is_mach { "AP_MANAGED_SPEED_IN_MACH_ON" } else { "AP_MANAGED_SPEED_IN_MACH_OFF" }, 1.);
        }
        if self.check_speed_mode && inputs.fcu_speed_managed {
            self.check_speed_mode = false;
            self.sink.set_bool("A32NX_PFD_MSG_CHECK_SPEED_MODE", false);
        }
        let v = if managed.target_kt.is_some() { managed_speed_lvar(&managed) } else { managed_speed_reset_lvar() };
        self.managed_pfd = managed.target_kt.unwrap_or(0.);
        self.sink.set(v.name, v.value);
        self.sink.set("A32NX_FM_GROSS_WEIGHT", gw_kg.unwrap_or(0.));
        let get = |f: fn(&PerformanceData) -> Option<f64>| pd.as_ref().and_then(f);
        self.sink.set("AIRLINER_V1_SPEED", get(|d| d.v1).unwrap_or(-1.));
        self.sink.set("AIRLINER_VR_SPEED", get(|d| d.vr).unwrap_or(-1.));
        self.sink.set("AIRLINER_V2_SPEED", get(|d| d.v2).unwrap_or(0.));
        self.sink.set("AIRLINER_MINIMUM_DESCENT_ALTITUDE", get(|d| d.approach_baro_minimum).unwrap_or(0.));
        self.sink.set("AIRLINER_DECISION_HEIGHT", get(|d| d.approach_radio_minimum).unwrap_or(-1.));
    }

    fn write_navigation(&mut self, inputs: &Inputs) {
        let rnp = self.fms.active().map_or(2., |p| {
            let active = p.active_leg_index.max(0) as usize;
            if active >= p.leg_count() {
                return 2.;
            }
            let Some((s, _)) = p.segment_position(active) else { return 2. };
            use fms::plan::{SegmentClass, SegmentKind::*};
            match p.segments[s].kind {
                MissedApproach | Destination | Approach | ApproachVia => match p.approach.as_ref().map_or(0, |a| a.approach_type) {
                    5 => 0.5,
                    10 | 12 => 0.3,
                    2 | 16 => 0.5,
                    _ => 0.5,
                },
                k if k.class() != SegmentClass::Enroute => 1.,
                _ => 2.,
            }
        });
        self.sink.set("A32NX_FMGC_L_RNP", rnp);
        self.sink.set("A32NX_FMGC_R_RNP", rnp);
        let available = inputs.gps_irs_available();
        let accuracy_high = available && 0.04 + inputs.ground_speed_kt * 0.00015 <= rnp;
        self.sink.set_bool("A32NX_FMGC_L_NAV_ACCURACY_HIGH", accuracy_high);
        self.sink.set_bool("A32NX_FMGC_R_NAV_ACCURACY_HIGH", accuracy_high);
        let gps_primary = available && accuracy_high;
        if self.gps_primary != Some(gps_primary) {
            self.gps_primary = Some(gps_primary);
            let text = if gps_primary { "NAV PRIMARY" } else { "NAV PRIMARY LOST" };
            self.sink.out.messages.push(text.to_string());
            for side in 0..2 {
                if gps_primary {
                    self.nd_flags[side] &= !ND_FLAG_NAV_PRIMARY_LOST;
                    self.nd_flags[side] |= ND_FLAG_NAV_PRIMARY;
                } else {
                    self.nd_flags[side] &= !ND_FLAG_NAV_PRIMARY;
                    self.nd_flags[side] |= ND_FLAG_NAV_PRIMARY_LOST;
                }
                self.sink.set(&format!("A32NX_EFIS_{}_ND_FM_MESSAGE_FLAGS", crate::efis::SIDES[side]), self.nd_flags[side] as f64);
            }
        }
    }

    fn write_fm_data(&mut self, cruise_ft: Option<f64>) {
        self.sink.set("A32NX_AIRLINER_CRUISE_ALTITUDE", cruise_ft.unwrap_or(-1.));
        let active = self.fms.active();
        let elev = active
            .and_then(|p| match &p.destination_runway {
                Some(r) => r.threshold_location.alt,
                None => p.destination_airport.as_ref().and_then(|a| a.location.alt).filter(|e| e.is_finite()),
            })
            .unwrap_or(0.);
        self.sink.set("A32NX_PRESS_AUTO_LANDING_ELEVATION", elev);
        let s = |v: Option<&str>| v.map_or(Value::Null, |x| Value::String(x.to_string()));
        let origin = s(active.and_then(|p| p.origin_airport.as_ref()).map(|a| a.ident.as_str()));
        let dep_rwy = s(active.and_then(|p| p.origin_runway.as_ref()).map(|r| r.fix.ident.as_str()));
        let dest = s(active.and_then(|p| p.destination_airport.as_ref()).map(|a| a.ident.as_str()));
        let ldg_rwy = s(active.and_then(|p| p.destination_runway.as_ref()).map(|r| r.fix.ident.as_str()));
        let altn = s(active.and_then(|p| p.alternate.as_ref()).and_then(|a| a.destination_airport.as_ref()).map(|a| a.ident.as_str()));
        let number = active.map_or(Value::Null, |p| if p.flight_number.is_empty() { Value::Null } else { Value::String(p.flight_number.clone()) });
        for (t, v) in [("fmsOrigin", origin), ("fmsDepartureRunway", dep_rwy), ("fmsDestination", dest), ("fmsLandingRunway", ldg_rwy), ("fmsAlternate", altn), ("fmsFlightNumber", number)] {
            self.sink.bus_if_changed(t, v);
        }
    }

    /// FBW `FmcAircraftInterface.init`'s master PRIM subscriptions: an FCU altitude change, an open or
    /// managed vertical mode engaging, and V/S or FPA engaging or changing reach the phase manager.
    fn fcu_phase_events(&mut self, inputs: &Inputs, pi: &PhaseInputs) {
        let now = inputs.master_prim_fcu;
        let Some(last) = self.last_master_prim_fcu.replace(now) else { return };
        let bit = |raw: f64, b: u32| {
            let (ssm, bits) = arinc_discrete(raw);
            ssm != 0 && bits & (1 << (b - 1)) != 0
        };
        let normal = |raw: f64| arinc_value(raw).is_some();
        let open_or_managed = |w: f64| bit(w, 11) || bit(w, 12) || bit(w, 13) || bit(w, 14);
        let vs_or_fpa = |w: f64| bit(w, 17) || bit(w, 18);
        let dist = self.guidance.distance_to_destination(&self.fms, inputs).unwrap_or(-1.);
        let (vs, fpa) = (arinc_value(now[1]).unwrap_or(0.), arinc_value(now[2]).unwrap_or(0.));
        if now[0] != last[0] && normal(now[0]) {
            self.phase.handle_fcu_alt_knob_turn(dist, vs, fpa, pi);
        }
        let mut step = false;
        if (now[1] != last[1] && normal(now[1])) || (now[2] != last[2] && normal(now[2])) {
            step |= self.phase.handle_fcu_vs_knob(dist, vs, fpa, pi) == VsKnobResult::StepClimbDescent;
        }
        if open_or_managed(now[3]) && !open_or_managed(last[3]) {
            self.phase.handle_fcu_alt_knob_push_pull(dist, pi);
            step = true;
        }
        if vs_or_fpa(now[3]) && !vs_or_fpa(last[3]) {
            step |= self.phase.handle_fcu_vs_knob(dist, vs, fpa, pi) == VsKnobResult::StepClimbDescent;
        }
        if step {
            self.on_step_climb_descent();
        }
    }

    /// FBW `FmcAircraftInterface.arincBusOutputs`, as `FlightManagementComputer.onUpdate` sets them.
    fn write_fm_words(&mut self, phase: FlightPhase) {
        const NCD: u32 = 1;
        const NO: u32 = 3;
        let pd = self.fms.perf.get(0).cloned().unwrap_or_default();
        let ssm = |v: Option<f64>| if v.is_some() { NO } else { NCD };
        let mut words: Vec<(&str, f64)> = Vec::new();

        words.push(("ZERO_FUEL_WEIGHT", bnr_word(pd.zero_fuel_weight.map_or(0., |z| z * 1_000.), ssm(pd.zero_fuel_weight.filter(|z| *z != 0.)), 19, 524288., 0.)));
        let cg = pd.zero_fuel_weight_center_of_gravity.filter(|c| *c != 0.);
        words.push(("ZERO_FUEL_WEIGHT_CG", bnr_word(cg.unwrap_or(0.), ssm(cg), 12, 64., 0.)));

        let (thr_red, acc, eo_acc) = if phase <= FlightPhase::Takeoff {
            (pd.thrust_reduction_altitude(), pd.acceleration_altitude(), pd.engine_out_acceleration_altitude())
        } else if phase == FlightPhase::GoAround {
            (pd.missed_thrust_reduction_altitude(), pd.missed_acceleration_altitude(), pd.missed_engine_out_acceleration_altitude())
        } else {
            (None, None, None)
        };
        words.push(("THR_RED_ALT", bnr_word(thr_red.unwrap_or(0.), ssm(thr_red), 17, 131072., 0.)));
        words.push(("ACC_ALT", bnr_word(acc.unwrap_or(0.), ssm(acc), 17, 131072., 0.)));
        words.push(("EO_ACC_ALT", bnr_word(eo_acc.unwrap_or(0.), ssm(eo_acc), 17, 131072., 0.)));
        let (trans_alt, trans_lvl) = (pd.transition_altitude(), pd.transition_level());
        words.push(("TRANS_ALT", bnr_word(trans_alt.unwrap_or(0.), ssm(trans_alt), 17, 131072., 0.)));
        words.push(("TRANS_LVL", bnr_word(trans_lvl.unwrap_or(0.), ssm(trans_lvl), 9, 512., 0.)));

        let not_inserted = [pd.v1, pd.vr, pd.v2].iter().any(|v| v.is_none_or(|v| v == 0.));
        let le = |a: Option<f64>, b: Option<f64>| match (a.filter(|v| *v != 0.), b.filter(|v| *v != 0.)) {
            (Some(a), Some(b)) => a <= b,
            _ => true,
        };
        let disagree = !(le(pd.v1, pd.vr) && le(pd.vr, pd.v2) && le(pd.v1, pd.v2));
        let dw3 = (disagree as u32) << 15 | (not_inserted as u32) << 17;
        words.push(("DISCRETE_WORD_3", discrete_word(dw3, NO)));
        let flaps = pd.takeoff_flaps;
        let dw2 = (0..4u8).fold(0u32, |w, f| w | ((flaps == Some(f)) as u32) << (12 + f as u32));
        words.push(("DISCRETE_WORD_2", discrete_word(dw2, NO)));
        if let Some(ths) = pd.takeoff_ths_for.filter(|t| *t != 0.) {
            words.push(("TO_PITCH_TRIM", bnr_word(ths, NO, 12, 180., -180.)));
        }

        let active = self.fms.active();
        let dest = active.and_then(|p| match &p.destination_runway {
            Some(r) => Some((r.threshold_location.alt, r.threshold_location.lat, r.threshold_location.long)),
            None => p.destination_airport.as_ref().map(|a| (a.location.alt.filter(|e| e.is_finite()), a.location.lat, a.location.long)),
        });
        let (elev, lat, long) = dest.map_or((None, None, None), |(e, la, lo)| (e, Some(la), Some(lo)));
        words.push(("LANDING_ELEVATION", bnr_word(elev.unwrap_or(0.), NO, 14, 16384., -2048.)));
        words.push(("DEST_LAT", bnr_word(lat.unwrap_or(0.), NO, 18, 180., -180.)));
        words.push(("DEST_LONG", bnr_word(long.unwrap_or(0.), NO, 18, 180., -180.)));

        let dest_pred = active.and_then(|p| self.vnav.predictions.get(&p.destination_leg_index()?)).cloned();
        let close = dest_pred.as_ref().is_none_or(|d| d.distance_from_aircraft < 250.);
        let in_range = phase > FlightPhase::Cruise || (phase == FlightPhase::Cruise && close);
        let mda_valid = in_range && pd.approach_baro_minimum.is_some();
        let dh = pd.approach_radio_minimum;
        let dh_valid = !mda_valid && in_range && dh.is_some_and(|d| d > 0.);
        words.push(("MINIMUM_DESCENT_ALTITUDE", bnr_word(if mda_valid { pd.approach_baro_minimum.unwrap_or(0.) } else { 0. }, if mda_valid { NO } else { NCD }, 17, 131072., 0.)));
        words.push(("DECISION_HEIGHT", bnr_word(if dh_valid { dh.unwrap_or(0.) } else { 0. }, if dh_valid { NO } else { NCD }, 16, 8192., 0.)));
        words.push(("EIS_DISCRETE_WORD_2", discrete_word(((in_range && dh == Some(0.)) as u32) << 28, NO)));
        words.push(("REMAINING_FLIGHT_TIME", bnr_word(dest_pred.as_ref().map_or(0., |d| d.seconds_from_present), if dest_pred.is_some() { NO } else { NCD }, 17, 131072., 0.)));

        if let Some(number) = active.map(|p| p.flight_number.clone()) {
            let chars: Vec<u32> = number.chars().map(|c| c as u32).collect();
            let iso5 = |from: usize, to: usize| {
                let part: Vec<u32> = chars.iter().skip(from).take(to.saturating_sub(from)).copied().collect();
                discrete_word(part.first().copied().unwrap_or(0) << 10 | part.get(1).copied().unwrap_or(0) << 18, NO)
            };
            for (name, from, to) in [("FLIGHT_NUMBER_1", 0, 2), ("FLIGHT_NUMBER_2", 2, 4), ("FLIGHT_NUMBER_3", 4, 6), ("FLIGHT_NUMBER_4", 6, 8), ("FLIGHT_NUMBER_5", 9, 10)] {
                words.push((name, iso5(from, to)));
            }
        }

        for (name, value) in words {
            self.sink.set(&format!("A32NX_FM1_{name}"), value);
            self.sink.set(&format!("A32NX_FM2_{name}"), value);
        }
    }
}

fn entry_phase(p: FlightPhase) -> fms_data::FlightPhase {
    match p {
        FlightPhase::Preflight => fms_data::FlightPhase::Preflight,
        FlightPhase::Takeoff => fms_data::FlightPhase::Takeoff,
        FlightPhase::Climb => fms_data::FlightPhase::Climb,
        FlightPhase::Cruise => fms_data::FlightPhase::Cruise,
        FlightPhase::Descent => fms_data::FlightPhase::Descent,
        FlightPhase::Approach => fms_data::FlightPhase::Approach,
        FlightPhase::GoAround => fms_data::FlightPhase::GoAround,
        FlightPhase::Done => fms_data::FlightPhase::Done,
    }
}

/// FBW `Arinc429Word.toSimVarValue` of a discrete or raw word.
fn discrete_word(raw: u32, ssm: u32) -> f64 {
    ssm as f64 * 4_294_967_296. + (raw as f32).to_bits() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use fms::navdb::MemoryNavDb;

    fn enter(core: &mut FmsCore, phase: FlightPhase) {
        let pi = core.current_phase_inputs();
        core.phase.change_phase(phase, &pi);
    }

    fn written(core: &FmsCore, name: &str) -> Option<f64> {
        core.sink.written.get(name).copied()
    }

    #[test]
    fn approach_managed_speed_is_vapp_after_a_descent() {
        let db = MemoryNavDb::default();
        let inputs = Inputs { indicated_altitude_ft: 8000., plane_altitude_ft: 8000., on_ground: false, n2: [60.; 4], ..Inputs::default() };
        let mut core = FmsCore::new();
        core.tick(&db, 50., &inputs);
        enter(&mut core, FlightPhase::Descent);
        let mut vnav = std::mem::take(&mut core.vnav);
        vnav.recompute(&core.vnav_context(&inputs, FlightPhase::Descent), &[]);
        core.vnav = vnav;
        core.vnav.driver.out.speeds_managed_pfd = Some(230.);
        core.tick(&db, 50., &inputs);
        assert_eq!(core.phase.phase(), FlightPhase::Descent);
        assert_eq!(written(&core, "A32NX_SPEEDS_MANAGED_PFD"), Some(230.));

        enter(&mut core, FlightPhase::Approach);
        for _ in 0..3 {
            core.tick(&db, 50., &inputs);
        }
        assert_eq!(core.phase.phase(), FlightPhase::Approach);
        let vapp = core.speeds.approach_vapp.expect("vapp");
        assert_eq!(written(&core, "A32NX_SPEEDS_MANAGED_PFD"), Some(vapp));
        assert_ne!(written(&core, "A32NX_SPEEDS_MANAGED_ATHR"), Some(230.));
        assert!(core.vnav.driver.out.speeds_managed_pfd.is_none());
    }

    #[test]
    fn cruise_after_a_descent_does_not_keep_the_descent_speed() {
        let db = MemoryNavDb::default();
        let inputs = Inputs { indicated_altitude_ft: 34000., plane_altitude_ft: 34000., on_ground: false, n2: [60.; 4], ..Inputs::default() };
        let mut core = FmsCore::new();
        core.tick(&db, 50., &inputs);
        enter(&mut core, FlightPhase::Descent);
        let mut vnav = std::mem::take(&mut core.vnav);
        vnav.recompute(&core.vnav_context(&inputs, FlightPhase::Descent), &[]);
        core.vnav = vnav;
        core.vnav.driver.out.speeds_managed_pfd = Some(230.);
        core.tick(&db, 50., &inputs);
        enter(&mut core, FlightPhase::Cruise);
        core.tick(&db, 50., &inputs);
        assert_ne!(written(&core, "A32NX_SPEEDS_MANAGED_PFD"), Some(230.));
    }

    fn star_leg(ident: &str, lat: f64, lon: f64, leg_type: LegType, altitude: Option<(&str, f64)>, speed: Option<f64>) -> fms::plan::Leg {
        let mut leg = fms::plan::Leg::from_enroute_fix(&fms::navdb::Fix::lat_long(ident, lat, lon), None, leg_type);
        if let Some((descriptor, altitude1)) = altitude {
            leg.definition.altitude_descriptor = Some(descriptor.to_string());
            leg.definition.altitude1 = Some(altitude1);
            leg.definition.altitude2 = Some(0.);
        }
        if let Some(speed) = speed {
            leg.definition.speed_descriptor = Some("-".to_string());
            leg.definition.speed = Some(speed);
        }
        leg.constraint_type = if altitude.is_some() || speed.is_some() { fms::plan::CONSTRAINT_DES } else { fms::plan::CONSTRAINT_UNKNOWN };
        leg
    }

    type StarRow = (&'static str, f64, f64, LegType, Option<(&'static str, f64)>, Option<f64>);

    fn star_rows() -> [StarRow; 25] {
        [
            ("GEXAN", 24.215833333, 56.946944444, LegType::IF, None, None),
            ("TAPRA", 24.435277778, 56.634166667, LegType::TF, None, None),
            ("VAXAS", 24.718888889, 56.301944444, LegType::TF, None, None),
            ("RUDAT", 24.768083333, 56.28725, LegType::TF, None, None),
            ("MIVEK", 24.877777778, 56.254444444, LegType::TF, None, None),
            ("IMPED", 24.973472222, 56.068388889, LegType::IF, Some(("-", 12000.)), Some(230.)),
            ("DB520", 25.060805556, 55.961583333, LegType::TF, Some(("-", 11000.)), Some(230.)),
            ("DB518", 25.15625, 55.844444444, LegType::TF, Some(("-", 10000.)), None),
            ("DB517", 25.19125, 55.781805556, LegType::TF, None, None),
            ("VUTON", 25.226111111, 55.719166667, LegType::TF, Some(("-", 8000.)), None),
            ("DB515", 25.261166667, 55.656444444, LegType::TF, None, Some(210.)),
            ("DB514", 25.304805556, 55.578027778, LegType::TF, Some(("-", 6000.)), None),
            ("DB513", 25.240611111, 55.534861111, LegType::TF, None, None),
            ("GIRGO", 25.196972222, 55.61325, LegType::TF, None, None),
            ("VELAR", 25.162083333, 55.675916667, LegType::TF, None, None),
            ("RIDEV", 25.127111111, 55.738583333, LegType::TF, None, None),
            ("DB508", 25.092111111, 55.801194444, LegType::TF, None, Some(185.)),
            ("DB507", 25.057083333, 55.86375, LegType::TF, None, None),
            ("DB506", 24.997222222, 55.823333333, LegType::TF, None, None),
            ("DB505", 25.032166667, 55.760777778, LegType::TF, None, None),
            ("DB504", 25.067138889, 55.698194444, LegType::TF, None, None),
            ("ULDOT", 25.102083333, 55.635555556, LegType::TF, Some(("+", 2000.)), Some(185.)),
            ("SEDPO", 25.141638889, 55.564527778, LegType::TF, Some(("+", 2000.)), None),
            ("MODUS", 25.185, 55.486583333, LegType::TF, None, None),
            ("RW30L", 25.234885, 55.390262, LegType::TF, None, None),
        ]
    }

    fn star_core(db: &MemoryNavDb, inputs: &Inputs) -> FmsCore {
        use fms::plan::{Element, SegmentKind};
        let mut core = FmsCore::new();
        core.tick(db, 50., inputs);
        {
            let plan = core.fms.plans[0].as_mut().expect("active plan");
            for (ident, lat, lon, leg_type, altitude, speed) in star_rows() {
                let mut leg = star_leg(ident, lat, lon, leg_type, altitude, speed);
                if ident == "MODUS" {
                    leg.definition.approach_waypoint_descriptor = Some(5);
                }
                if ident == "RW30L" {
                    leg.definition.approach_waypoint_descriptor = Some(8);
                    leg.definition.vertical_angle = Some(-3.);
                }
                plan.seg_mut(SegmentKind::Enroute).legs.push(Element::Leg(Box::new(leg)));
            }
            plan.active_leg_index = 1;
            plan.increment_version();
        }
        let perf = core.fms.perf.ensure(0);
        perf.cruise_flight_level = Some(340.);
        perf.cost_index = Some(48.);
        perf.zero_fuel_weight = Some(300.);
        perf.zero_fuel_weight_center_of_gravity = Some(35.);
        core
    }

    #[test]
    fn star_transitions_are_sized_with_descent_speeds_in_cruise() {
        let db = MemoryNavDb::default();
        let inputs = Inputs {
            lat: 24.239,
            lon: 56.914,
            true_track: 308.,
            true_heading: 308.,
            ground_speed_kt: 470.,
            true_airspeed_kt: 475.,
            on_ground: false,
            indicated_altitude_ft: 34000.,
            plane_altitude_ft: 34000.,
            pressure_altitude_ft: Some(34000.),
            cas_kt: Some(279.),
            n2: [90.; 4],
            fcu_selected_altitude_ft: 34000.,
            fcu_speed: -1.,
            fcu_speed_managed: true,
            fqms_gross_weight_kg: Some(336000.),
            fqms_fuel_on_board_kg: Some(36000.),
            fqms_cg_percent: Some(35.),
            ..Inputs::default()
        };
        let mut core = star_core(&db, &inputs);
        let rows = star_rows();
        let stages = [
            ("cruise", FlightPhase::Cruise, inputs.clone()),
            ("descent, FL330", FlightPhase::Descent, Inputs { indicated_altitude_ft: 33000., plane_altitude_ft: 33000., pressure_altitude_ft: Some(33000.), true_airspeed_kt: 465., ground_speed_kt: 460., cas_kt: Some(280.), lateral_mode: 20, vertical_mode: 23, ..inputs.clone() }),
            ("descent, FL220", FlightPhase::Descent, Inputs { indicated_altitude_ft: 22000., plane_altitude_ft: 22000., pressure_altitude_ft: Some(22000.), true_airspeed_kt: 390., ground_speed_kt: 390., cas_kt: Some(300.), lateral_mode: 20, vertical_mode: 23, ..inputs.clone() }),
        ];
        let mut failures: Vec<String> = Vec::new();
        for (label, phase, stage_inputs) in stages {
            enter(&mut core, phase);
            for _ in 0..8 {
                core.tick(&db, 5100., &stage_inputs);
            }
            let calc = core.guidance.calculated.get(&(0, false)).expect("lnav distances");
            let to_end = |ident: &str| {
                let index = rows.iter().position(|r| r.0 == ident).unwrap();
                calc[index].as_ref().expect("leg distances").cumulative_distance_to_end_with_transitions
            };
            let legs: Vec<(usize, Option<f64>)> = (0..rows.len()).map(|i| (i, calc[i].as_ref().map(|c| c.cumulative_distance_with_transitions))).collect();
            let params = core.vnav.params.as_ref().expect("vnav parameters");
            let predictions = core.vnav.driver.leg_speed_predictions(params, &legs, 1, 0.);
            let tas = |ident: &str| {
                let index = rows.iter().position(|r| r.0 == ident).unwrap();
                predictions.iter().find(|p| p.0 == index).and_then(|p| p.1).map(|p| p.0)
            };
            eprintln!("{label}: vnav ready {:?}, phase {:?}", core.vnav.driver.mcdu_profile.as_ref().map(|m| m.is_ready_to_display), core.phase.phase());
            eprintln!("{label}: to end IMPED {:.1} DB520 {:.1} DB514 {:.1} DB508 {:.1} total {:.1}", to_end("IMPED"), to_end("DB520"), to_end("DB514"), to_end("DB508"), to_end("GEXAN"));
            eprintln!("{label}: predicted tas TAPRA {:?} IMPED {:?} DB518 {:?} DB514 {:?} DB508 {:?} MODUS {:?}", tas("TAPRA"), tas("IMPED"), tas("DB518"), tas("DB514"), tas("DB508"), tas("MODUS"));
            for ident in ["DB518", "DB514", "DB508", "ULDOT"] {
                match tas(ident) {
                    Some(v) if v <= 350. => {}
                    other => failures.push(format!("{label}: {ident} predicted at {other:?} kt TAS")),
                }
            }
            for (ident, short) in [("IMPED", 90.9), ("DB514", 58.1)] {
                if (to_end(ident) - short).abs() >= 8. {
                    failures.push(format!("{label}: {ident} is {:.1} nm from the end, expected about {short}", to_end(ident)));
                }
            }
            if (to_end("GEXAN") - 147.3).abs() >= 12. {
                failures.push(format!("{label}: route is {:.1} nm long, expected about 147.3", to_end("GEXAN")));
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn every_plan_leg_has_a_representation_for_vnav() {
        let db = MemoryNavDb::default();
        let inputs = Inputs {
            lat: 24.8,
            lon: 56.28,
            true_track: 330.,
            on_ground: false,
            indicated_altitude_ft: 34000.,
            plane_altitude_ft: 34000.,
            pressure_altitude_ft: Some(34000.),
            true_airspeed_kt: 470.,
            ground_speed_kt: 460.,
            n2: [90.; 4],
            ..Inputs::default()
        };
        let mut core = star_core(&db, &inputs);
        if let Some(plan) = core.fms.plans[0].as_mut() {
            plan.active_leg_index = 3;
            plan.increment_version();
        }
        core.tick(&db, 5100., &inputs);
        let legs = core.legs_with_representation();
        assert_eq!(legs.len(), star_rows().len());
        assert!(legs.iter().all(|l| l.1), "{legs:?}");
    }

    #[test]
    #[ignore]
    fn star_descent_target_is_continuous_while_flying_the_route() {
        let db = MemoryNavDb::default();
        let rows = star_rows();
        let at = |i: usize| Coord::new(rows[i].1, rows[i].2);
        let seg = |i: usize| fms_lnav::geo::distance_to(at(i), at(i + 1));
        let mut inputs = Inputs {
            on_ground: false,
            indicated_altitude_ft: 34000.,
            plane_altitude_ft: 34000.,
            pressure_altitude_ft: Some(34000.),
            n2: [90.; 4],
            fcu_selected_altitude_ft: 34000.,
            fcu_speed: -1.,
            fcu_speed_managed: true,
            lateral_mode: 20,
            vertical_mode: 10,
            fqms_gross_weight_kg: Some(336000.),
            fqms_fuel_on_board_kg: Some(36000.),
            fqms_cg_percent: Some(35.),
            ..Inputs::default()
        };
        let mut core = star_core(&db, &inputs);
        let (mut leg, mut along, mut altitude) = (0usize, 3.0_f64, 34000.0_f64);
        let mut samples: Vec<(f64, f64, f64, f64)> = Vec::new();
        for tick in 0..2600 {
            let tas = (470. - (34000. - altitude) * 0.012).max(250.);
            let ground_speed = tas - 8.;
            along += ground_speed / 3600.;
            while leg < 23 && along > seg(leg) {
                along -= seg(leg);
                leg += 1;
            }
            if leg >= 12 {
                break;
            }
            let f = along / seg(leg);
            let (a, b) = (at(leg), at(leg + 1));
            if tick == 120 {
                enter(&mut core, FlightPhase::Descent);
            }
            if tick >= 120 {
                altitude -= 1000. / 60.;
            }
            inputs.lat = a.lat + f * (b.lat - a.lat);
            inputs.lon = a.long + f * (b.long - a.long);
            inputs.true_track = fms_lnav::geo::bearing_to(a, b);
            inputs.true_heading = inputs.true_track;
            inputs.true_airspeed_kt = tas;
            inputs.ground_speed_kt = ground_speed;
            inputs.cas_kt = Some(275.);
            inputs.indicated_altitude_ft = altitude;
            inputs.plane_altitude_ft = altitude;
            inputs.pressure_altitude_ft = Some(altitude);
            inputs.vertical_mode = if tick >= 120 { 23 } else { 10 };
            if let Some(plan) = core.fms.plans[0].as_mut() {
                if plan.active_leg_index != leg as i64 + 1 {
                    plan.active_leg_index = leg as i64 + 1;
                    plan.increment_version();
                }
            }
            core.tick(&db, 1000., &inputs);
            if tick % 10 == 0 {
                let remaining = seg(leg) - along + (leg + 1..24).map(seg).sum::<f64>();
                let to_end = core.guidance.along_track_distance_to_destination(&core.fms, &inputs).unwrap_or(f64::NAN);
                let target = written(&core, "A32NX_PFD_TARGET_ALTITUDE").unwrap_or(f64::NAN);
                samples.push((remaining, to_end, target, altitude));
                let calc = core.guidance.calculated.get(&(0, false));
                let end_of = |i: usize| calc.and_then(|c| c.get(i)).and_then(|c| c.as_ref()).map_or(f64::NAN, |c| c.cumulative_distance_to_end_with_transitions);
                let legs: Vec<(usize, Option<f64>)> = (0..rows.len()).map(|i| (i, calc.and_then(|c| c.get(i)).and_then(|c| c.as_ref()).map(|c| c.cumulative_distance_with_transitions))).collect();
                let predicted = core.vnav.params.as_ref().map(|p| core.vnav.driver.leg_speed_predictions(p, &legs, leg + 1, 0.)).unwrap_or_default();
                let predicted_tas = |i: usize| predicted.iter().find(|p| p.0 == i).and_then(|p| p.1).map_or(f64::NAN, |p| p.0);
                let descent = core.vnav.driver.descent_profile.as_ref();
                let tod = descent.and_then(|d| d.find_vertical_checkpoint(&[fms_vnav::profile::Reason::TopOfDescent]).map(|c| d.total_flight_plan_distance - c.distance_from_start));
                let relation = core.vnav.driver.relation.current_distance_to_end();
                eprintln!(
                    "t {tick:4} alt {altitude:6.0} tas {tas:3.0} path to end {remaining:6.1} along-track {to_end:6.1} relation {relation:6.1} profile tod {tod:?} pfd target {target:6.0} | to end IMPED {:.1} DB514 {:.1} | predicted tas IMPED {:.0} DB514 {:.0} MODUS {:.0}",
                    end_of(5),
                    end_of(11),
                    predicted_tas(5),
                    predicted_tas(11),
                    predicted_tas(23)
                );
            }
        }
        let mut failures: Vec<String> = Vec::new();
        for w in samples.windows(2) {
            if w[0].2.is_finite() && w[1].2.is_finite() && w[0].2 - w[1].2 > 900. {
                failures.push(format!("target fell {:.0} ft between {:.1} and {:.1} nm from the end", w[0].2 - w[1].2, w[0].0, w[1].0));
            }
            if (w[1].1 - w[1].0).abs() > 8. {
                failures.push(format!("along-track distance {:.1} nm but the route has {:.1} nm left", w[1].1, w[1].0));
            }
        }
        failures.dedup();
        assert!(failures.is_empty(), "{failures:#?}");
    }
}

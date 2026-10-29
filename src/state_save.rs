//! Whole-flight save and load (docs/superpowers/plans/2026-10-22-state-save-load.md):
//! snapshot assembly, autosave/phase/quit saves, the EFB's Saves page
//! requests and the paused load sequence. Format and slots live in
//! `crates/state_snapshot`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use state_snapshot::phase::{AutosaveClock, PhaseWatch, Sample};
use state_snapshot::restore::{Poll, Sequence, SettleLimits, Stage, Step};
use state_snapshot::slots::{AutosaveSettings, SlotManager};
use state_snapshot::{vars, AircraftInfo, Restored, SaveReason, Section, Snapshot, Summary};
use systems::simulation::{SimulatorReaderWriter, StartState, VariableIdentifier, VariableRegistry};

use crate::xp::{DataRef, Xplm};

pub const STATES_FOLDER: &str = "a380_states";
pub const SETTINGS_FILE: &str = "a380_state_settings.json";
/// GENERIC: system time step for the paused settle frames.
pub const SYNTHETIC_DT_S: f64 = 1.0 / 30.0;
const SITUATION: &str = "xp_situation";
const FAILURES: &str = "flight_failures";
const CONDITION: &str = "airframe_condition";
const DEEP: &str = "deep";
const SYSTEMS: &str = "systems";
const PLUGIN_SYSTEMS: &str = "plugin_systems";
const FMS: &str = "fms";
/// GENERIC: how often the JS FMS's flight plans are copied for the next save.
const FMS_POLL_S: f64 = 10.0;

/// FlyByWire's own flight plans (`BaseFlightPlan::serialize`), by plan index,
/// from the JS FMS (`globalThis.__xpFmc`); empty where there is no FMS.
pub const FMS_SAVE_JS: &str = r#"(() => {
  const fmc = (globalThis.__xpFmc || []).find(Boolean);
  const fpm = fmc && fmc.flightPlanInterface && fmc.flightPlanInterface.flightPlanManager;
  if (!fpm || !fpm.plans) return '';
  const plans = {};
  fpm.plans.forEach((p, i) => { if (p) plans[i] = p.serialize(); });
  return JSON.stringify({ plans });
})()"#;

/// Rebuilds each saved plan with FlyByWire's `FlightPlan.fromSerializedFlightPlan`
/// the way `FlightPlanManager`'s sync response does, and deletes plans the
/// save did not have. Progress lands in `globalThis.__xpFmsLoad`.
pub fn fms_restore_js(saved: &Value) -> String {
    let saved = serde_json::to_string(saved).unwrap_or_else(|_| "{}".into());
    format!(
        r#"(() => {{
  const fmc = (globalThis.__xpFmc || []).find(Boolean);
  const fpm = fmc && fmc.flightPlanInterface && fmc.flightPlanInterface.flightPlanManager;
  if (!fpm || !fpm.plans) return '';
  const saved = {saved};
  const any = fpm.plans.find(Boolean);
  if (!any || !saved.plans) return 'no plan to restore into';
  const FP = any.constructor;
  const c = fpm.config || {{}};
  globalThis.__xpFmsLoad = 'running';
  (async () => {{
    fpm.plans.forEach((p, i) => {{ if (p && i !== 0 && !(String(i) in saved.plans)) fpm.delete(i); }});
    for (const [k, s] of Object.entries(saved.plans)) {{
      const i = parseInt(k);
      const p = await FP.fromSerializedFlightPlan(fpm.context, i, s, fpm.bus, fpm.performanceDataInit.clone(), fpm.time.get(), c.DRAFT_ON_WIND_EDIT ?? false, c.NUM_CLIMB_WIND_LEVELS ?? 0, c.NUM_CRUISE_WIND_LEVELS ?? 0, c.NUM_DESCENT_WIND_LEVELS ?? 0, c.SORT_CLIMB_WIND_DESCENDING ?? true);
      fpm.set(i, p);
    }}
  }})().then(() => {{ globalThis.__xpFmsLoad = 'ok'; }}, (e) => {{ globalThis.__xpFmsLoad = 'error: ' + ((e && e.stack) || e); }});
  return 'started';
}})()"#
    )
}

const SCALARS: &[&str] = &[
    "sim/flightmodel/position/latitude",
    "sim/flightmodel/position/longitude",
    "sim/flightmodel/position/elevation",
    "sim/flightmodel/position/local_x",
    "sim/flightmodel/position/local_y",
    "sim/flightmodel/position/local_z",
    "sim/flightmodel/position/psi",
    "sim/flightmodel/position/theta",
    "sim/flightmodel/position/phi",
    "sim/flightmodel/position/local_vx",
    "sim/flightmodel/position/local_vy",
    "sim/flightmodel/position/local_vz",
    "sim/flightmodel/position/P",
    "sim/flightmodel/position/Q",
    "sim/flightmodel/position/R",
    "sim/flightmodel/position/Prad",
    "sim/flightmodel/position/Qrad",
    "sim/flightmodel/position/Rrad",
    "sim/flightmodel/position/groundspeed",
    "sim/flightmodel/position/indicated_airspeed",
    "sim/flightmodel/failures/onground_any",
    "sim/time/zulu_time_sec",
    "sim/time/local_date_days",
    "sim/time/paused",
    "sim/flightmodel/weight/m_fixed",
    "sim/flightmodel/controls/flaprqst",
    "sim/cockpit2/controls/parking_brake_ratio",
    "sim/flightmodel/controls/elv_trim",
    "sim/flightmodel/controls/ail_trim",
    "sim/flightmodel/controls/rud_trim",
];

const READ_ONLY: &[&str] = &[
    "sim/flightmodel/position/latitude",
    "sim/flightmodel/position/longitude",
    "sim/flightmodel/position/elevation",
    "sim/flightmodel/position/groundspeed",
    "sim/flightmodel/position/indicated_airspeed",
    "sim/flightmodel/failures/onground_any",
    "sim/time/paused",
];

/// Position-only values: written on `SetSituation` and `ReapplySituation`.
const POSITION: &[&str] = &[
    "sim/flightmodel/position/local_x",
    "sim/flightmodel/position/local_y",
    "sim/flightmodel/position/local_z",
    "sim/flightmodel/position/psi",
    "sim/flightmodel/position/theta",
    "sim/flightmodel/position/phi",
    "sim/flightmodel/position/local_vx",
    "sim/flightmodel/position/local_vy",
    "sim/flightmodel/position/local_vz",
    "sim/flightmodel/position/P",
    "sim/flightmodel/position/Q",
    "sim/flightmodel/position/R",
    "sim/flightmodel/position/Prad",
    "sim/flightmodel/position/Qrad",
    "sim/flightmodel/position/Rrad",
    "sim/flightmodel/position/q",
];

const ARRAYS: &[(&str, usize)] = &[
    ("sim/flightmodel/position/q", 4),
    ("sim/flightmodel/weight/m_fuel", 9),
    ("sim/flightmodel/weight/m_stations", 10),
    ("sim/flightmodel2/gear/deploy_ratio", 10),
];

const MS_TO_KT: f64 = 1.943_844_5;

// ---------------------------------------------------------------------
// Sections
// ---------------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct FlightFailures {
    pub active_ids: Vec<u64>,
    pub magnitudes: BTreeMap<u64, f64>,
    pub random_config: crate::random_failures::Config,
    pub random_rng: crate::random_failures::Rng,
    pub scripted: Vec<crate::scripted_failures::ScriptedTrigger>,
    pub deferred: Vec<crate::mel::Deferral>,
    pub tech_log: Vec<crate::mel::TechLogEntry>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AirframeCondition {
    pub engines: [crate::physics::damage::EngineWear; 4],
    pub wear: crate::wear::WearStore,
    pub components: Vec<crate::components::Direct>,
    pub walkaround: BTreeMap<String, bool>,
    #[serde(default)]
    pub structure: crate::physics::structure::Strength,
}

/// Plugin-side system state that is not in variables: FADEC internals,
/// the fuel network, the oxygen bottles, the tyres.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginSystems {
    pub fadec: Option<crate::fadec::SavedFadec>,
    pub fuel: Option<crate::fuel::SavedFuel>,
    pub oxygen: Option<(f64, f64, bool)>,
    /// `[temperature C, leaked fraction, tread mm, fuse plug melted, burst]`.
    pub tyres: Vec<[f64; 5]>,
    /// Ground services connected, by `gse::Service::name`.
    pub gse: Vec<String>,
}

/// What a snapshot is built from, copied on the flight loop.
pub struct Parts {
    pub vars: Vec<(bool, String, f64)>,
    pub failures: FlightFailures,
    pub condition: AirframeCondition,
    pub deep: Map<String, Value>,
    pub plugin_systems: PluginSystems,
    pub fms: Option<Value>,
    pub situation: Value,
    pub start_state: StartState,
    pub fmgc_phase: u8,
}

fn start_state_name(s: StartState) -> &'static str {
    match s {
        StartState::Hangar => "hangar",
        StartState::Apron => "apron",
        StartState::Taxi => "taxi",
        StartState::Runway => "runway",
        StartState::Climb => "climb",
        StartState::Cruise => "cruise",
        StartState::Approach => "approach",
        StartState::Final => "final",
    }
}

fn start_state_from(name: &str) -> Option<StartState> {
    [
        StartState::Hangar,
        StartState::Apron,
        StartState::Taxi,
        StartState::Runway,
        StartState::Climb,
        StartState::Cruise,
        StartState::Approach,
        StartState::Final,
    ]
    .into_iter()
    .find(|s| start_state_name(*s) == name)
}

fn phase_name(phase: u8) -> &'static str {
    use state_snapshot::phase::fmgc::*;
    match phase {
        PREFLIGHT => "preflight",
        TAKEOFF => "takeoff",
        CLIMB => "climb",
        CRUISE => "cruise",
        DESCENT => "descent",
        APPROACH => "approach",
        GOAROUND => "go-around",
        DONE => "done",
        _ => "unknown",
    }
}

fn situation_number(situation: &Value, name: &str) -> Option<f64> {
    situation.get("scalars")?.get(name).and_then(vars::decode_number)
}

pub fn assemble(name: &str, reason: SaveReason, parts: Parts) -> Snapshot {
    let aircraft = AircraftInfo { name: "A380 for X-Plane 12".into(), plugin_version: env!("CARGO_PKG_VERSION").into(), ..Default::default() };
    let mut snap = Snapshot::new(name, reason, aircraft);
    let s = &parts.situation;
    snap.summary = Summary {
        latitude: situation_number(s, "sim/flightmodel/position/latitude"),
        longitude: situation_number(s, "sim/flightmodel/position/longitude"),
        altitude_ft: situation_number(s, "sim/flightmodel/position/elevation").map(|m| m / 0.3048),
        ias_kt: situation_number(s, "sim/flightmodel/position/indicated_airspeed"),
        on_ground: situation_number(s, "sim/flightmodel/failures/onground_any").map(|v| v != 0.0),
        flight_phase: Some(phase_name(parts.fmgc_phase).into()),
        sim_zulu_s: situation_number(s, "sim/time/zulu_time_sec"),
        ..Default::default()
    };
    snap.put_raw(vars::SECTION, vars::VERSION, vars::save(parts.vars.iter().map(|(n, k, v)| (*n, k.as_str(), *v))));
    snap.put_raw(FAILURES, 1, serde_json::to_value(&parts.failures).unwrap_or(Value::Null));
    snap.put_raw(CONDITION, 1, serde_json::to_value(&parts.condition).unwrap_or(Value::Null));
    snap.put_raw(DEEP, 1, Value::Object(parts.deep));
    snap.put_raw(PLUGIN_SYSTEMS, 1, serde_json::to_value(&parts.plugin_systems).unwrap_or(Value::Null));
    if let Some(fms) = parts.fms {
        snap.put_raw(FMS, 1, fms);
    }
    snap.put_raw(SYSTEMS, 1, json!({ "start_state": start_state_name(parts.start_state) }));
    snap.put_raw(SITUATION, 1, parts.situation);
    snap
}

struct Typed<T> {
    name: &'static str,
    value: Option<T>,
}

impl<T: Serialize + for<'de> Deserialize<'de>> Section for Typed<T> {
    fn name(&self) -> &'static str {
        self.name
    }
    fn version(&self) -> u32 {
        1
    }
    fn save(&self) -> Value {
        serde_json::to_value(self.value.as_ref()).unwrap_or(Value::Null)
    }
    fn load(&mut self, _version: u32, data: &Value) -> Result<(), String> {
        self.value = Some(serde_json::from_value(data.clone()).map_err(|e| e.to_string())?);
        Ok(())
    }
}

fn typed<T: Serialize + for<'de> Deserialize<'de>>(snap: &Snapshot, name: &'static str, log: &mut Vec<String>) -> Option<T> {
    let mut section = Typed::<T> { name, value: None };
    match snap.restore(&mut section) {
        Restored::Done => section.value,
        other => {
            log.push(format!("section {name}: {other:?}"));
            None
        }
    }
}

/// The named variables to write back, filtered.
pub fn variables_to_write(snap: &Snapshot) -> vars::Loaded {
    let Some(section) = snap.sections.get(vars::SECTION) else {
        return vars::Loaded { write: Vec::new(), skipped: 0, unreadable: Vec::new() };
    };
    vars::load(&section.data, &skip_filter())
}

/// GENERIC skip list: momentary events, spawn/start-state flags, the
/// plugin's own diagnostics.
pub fn skip_filter() -> vars::Filter {
    let mut f = vars::Filter::named_only();
    f.skip_exact.extend(["A32NX_COLD_AND_DARK_SPAWN", "A32NX_START_STATE"].map(String::from));
    f.skip_contains.extend(["_PRESSED", "_PUSHED", "_EVENT", "XPHFBW_DIAG", "LIVE_LOG", "STATE_DUMP"].map(String::from));
    f
}

// ---------------------------------------------------------------------
// X-Plane situation
// ---------------------------------------------------------------------

fn find(x: &Xplm, name: &str) -> Option<DataRef> {
    x.find(name)
}

pub fn read_situation(x: &Xplm) -> Value {
    let mut scalars = Map::new();
    for name in SCALARS {
        if let Some(d) = find(x, name) {
            scalars.insert(name.to_string(), vars::encode_number(x.get_d(d)));
        }
    }
    let mut arrays = Map::new();
    for (name, n) in ARRAYS {
        if let Some(d) = find(x, name) {
            let mut buf = vec![0f32; *n];
            let got = x.get_vf(d, &mut buf).min(*n);
            arrays.insert(name.to_string(), Value::Array(buf[..got].iter().map(|v| vars::encode_number(*v as f64)).collect()));
        }
    }
    json!({ "scalars": scalars, "arrays": arrays })
}

fn write_scalar(x: &Xplm, d: DataRef, v: f64) {
    let types = crate::xp::data_ref_types(d);
    if types & 4 != 0 {
        crate::xp::set_d(d, v);
    } else if types & 1 != 0 && types & 2 == 0 {
        x.set_i(d, v.round() as std::ffi::c_int);
    } else {
        x.set_f(d, v as f32);
    }
}

/// Write the saved situation. `position_only` limits it to position,
/// attitude and velocities (the after-settle reapply).
pub fn write_situation(x: &Xplm, situation: &Value, position_only: bool) -> usize {
    let mut written = 0;
    let wanted = |name: &str| !READ_ONLY.contains(&name) && (!position_only || POSITION.contains(&name));
    let local = local_from_world(situation);
    if let Some(scalars) = situation.get("scalars").and_then(Value::as_object) {
        for (name, v) in scalars {
            if !wanted(name) {
                continue;
            }
            let value = match (name.as_str(), local) {
                ("sim/flightmodel/position/local_x", Some((lx, _, _))) => Some(lx),
                ("sim/flightmodel/position/local_y", Some((_, ly, _))) => Some(ly),
                ("sim/flightmodel/position/local_z", Some((_, _, lz))) => Some(lz),
                _ => vars::decode_number(v),
            };
            if let (Some(d), Some(value)) = (find(x, name), value) {
                write_scalar(x, d, value);
                written += 1;
            }
        }
    }
    if let Some(arrays) = situation.get("arrays").and_then(Value::as_object) {
        for (name, v) in arrays {
            if !wanted(name) {
                continue;
            }
            let values: Vec<f32> = v.as_array().map(|a| a.iter().filter_map(vars::decode_number).map(|f| f as f32).collect()).unwrap_or_default();
            if let (Some(d), false) = (find(x, name), values.is_empty()) {
                x.set_vf(d, &values);
                written += 1;
            }
        }
    }
    written
}

/// `local_x/y/z` re-derived from latitude/longitude/elevation: the local
/// origin moves with the scenery X-Plane loaded around the new position.
fn local_from_world(situation: &Value) -> Option<(f64, f64, f64)> {
    let lat = situation_number(situation, "sim/flightmodel/position/latitude")?;
    let lon = situation_number(situation, "sim/flightmodel/position/longitude")?;
    let elev = situation_number(situation, "sim/flightmodel/position/elevation")?;
    crate::xp::world_to_local(lat, lon, elev)
}

pub fn place_aircraft(situation: &Value) -> bool {
    let n = |k| situation_number(situation, k);
    let (Some(lat), Some(lon), Some(elev)) = (n("sim/flightmodel/position/latitude"), n("sim/flightmodel/position/longitude"), n("sim/flightmodel/position/elevation")) else {
        return false;
    };
    let heading = n("sim/flightmodel/position/psi").unwrap_or(0.0);
    let speed = n("sim/flightmodel/position/groundspeed").unwrap_or(0.0);
    crate::xp::place_user_at_location(lat, lon, elev as f32, heading as f32, speed as f32)
}

// ---------------------------------------------------------------------
// Load driver
// ---------------------------------------------------------------------

/// What the load sequence acts on; the plugin implements it, tests fake it.
pub trait LoadTarget {
    fn sim_paused(&self) -> bool;
    fn pause(&mut self, on: bool);
    fn place(&mut self, situation: &Value) -> bool;
    fn scenery_loading(&self) -> bool;
    fn set_situation(&mut self, situation: &Value, position_only: bool);
    fn rebuild(&mut self, start: StartState);
    fn write_vars(&mut self, values: &[(String, f64)]);
    fn read_vars(&mut self, names: &[String]) -> Vec<f64>;
    fn restore_condition(&mut self, condition: AirframeCondition);
    fn restore_plugin_systems(&mut self, systems: PluginSystems);
    fn restore_deep(&mut self, saved: &Map<String, Value>) -> Vec<(&'static str, String)>;
    fn restore_failures(&mut self, failures: FlightFailures);
    fn restore_fms(&mut self, fms: &Value);
}

#[derive(Debug, PartialEq)]
pub enum Frame {
    /// Tick the systems with this delta (the settle frames) or as normal.
    Running { systems_dt: Option<f64> },
    Finished { settle_frames: u32, converged: bool },
}

pub struct LoadDriver {
    snap: Snapshot,
    seq: Sequence,
    resume_unpaused: bool,
    written: Vec<String>,
    last: Vec<f64>,
    pub log: Vec<String>,
}

impl LoadDriver {
    pub fn new(snap: Snapshot, limits: SettleLimits) -> Self {
        Self { snap, seq: Sequence::new(limits), resume_unpaused: true, written: Vec::new(), last: Vec::new(), log: Vec::new() }
    }

    pub fn stage(&self) -> Stage {
        self.seq.stage()
    }

    pub fn snapshot(&self) -> &Snapshot {
        &self.snap
    }

    fn situation(&self) -> Value {
        self.snap.sections.get(SITUATION).map(|s| s.data.clone()).unwrap_or(Value::Null)
    }

    /// One flight-loop frame of the load. `wall_s` is real time since the
    /// last frame (scenery timeout).
    pub fn frame(&mut self, t: &mut dyn LoadTarget, wall_s: f64) -> Frame {
        let settling = self.seq.stage() == Stage::Settle;
        let quiet = if settling {
            let now = t.read_vars(&self.written);
            let quiet = now.len() == self.last.len() && now.iter().zip(&self.last).all(|(a, b)| (a - b).abs() <= 1e-3 * a.abs().max(1.0) || (a.is_nan() && b.is_nan()));
            self.last = now;
            quiet
        } else {
            false
        };
        let poll = Poll { scenery_loading: t.scenery_loading(), frame_s: if settling { SYNTHETIC_DT_S } else { wall_s }, quiet };
        for _ in 0..crate_stage_budget() {
            match self.seq.step(poll) {
                Step::Do(stage) => {
                    self.perform(t, stage);
                    self.seq.advance();
                    if stage == Stage::PlaceAircraft {
                        return Frame::Running { systems_dt: None };
                    }
                }
                Step::Wait(Stage::Settle) => return Frame::Running { systems_dt: Some(SYNTHETIC_DT_S) },
                Step::Wait(_) => return Frame::Running { systems_dt: None },
                Step::Finished { settle_frames, converged } => return Frame::Finished { settle_frames, converged },
            }
        }
        Frame::Running { systems_dt: None }
    }

    fn perform(&mut self, t: &mut dyn LoadTarget, stage: Stage) {
        match stage {
            Stage::PauseSim => {
                let saved_paused = situation_number(&self.situation(), "sim/time/paused").unwrap_or(0.0) != 0.0;
                self.resume_unpaused = !saved_paused;
                if !t.sim_paused() {
                    t.pause(true);
                }
            }
            Stage::PlaceAircraft => {
                if !t.place(&self.situation()) {
                    self.log.push("no saved position; aircraft left where it is".into());
                }
            }
            Stage::SetSituation => t.set_situation(&self.situation(), false),
            Stage::RebuildSystems => {
                let name = self.snap.sections.get(SYSTEMS).and_then(|s| s.data.get("start_state")).and_then(Value::as_str).unwrap_or("");
                match start_state_from(name) {
                    Some(start) => t.rebuild(start),
                    None => self.log.push(format!("unknown start state {name:?}; systems not rebuilt")),
                }
            }
            Stage::WriteVariables => {
                let loaded = variables_to_write(&self.snap);
                let values: Vec<(String, f64)> = loaded.write.into_iter().map(|(_, n, v)| (n, v)).collect();
                self.log.push(format!("{} variables written, {} skipped, {} unreadable", values.len(), loaded.skipped, loaded.unreadable.len()));
                self.written = values.iter().map(|(n, _)| n.clone()).collect();
                t.write_vars(&values);
            }
            Stage::RestoreModules => {
                if let Some(c) = typed::<AirframeCondition>(&self.snap, CONDITION, &mut self.log) {
                    t.restore_condition(c);
                }
                if let Some(s) = typed::<PluginSystems>(&self.snap, PLUGIN_SYSTEMS, &mut self.log) {
                    t.restore_plugin_systems(s);
                }
            }
            Stage::RestoreDeepAreas => {
                if let Some(Value::Object(m)) = self.snap.sections.get(DEEP).map(|s| &s.data) {
                    for (area, e) in t.restore_deep(m) {
                        self.log.push(format!("deep area {area}: {e}"));
                    }
                }
            }
            Stage::RestoreFailures => {
                if let Some(f) = typed::<FlightFailures>(&self.snap, FAILURES, &mut self.log) {
                    t.restore_failures(f);
                }
            }
            Stage::RestoreFms => {
                if let Some(s) = self.snap.sections.get(FMS) {
                    t.restore_fms(&s.data);
                }
            }
            Stage::RestoreFbwInternals | Stage::RestoreUi => {}
            Stage::ReapplySituation => t.set_situation(&self.situation(), true),
            Stage::Resume => {
                if self.resume_unpaused {
                    t.pause(false);
                }
            }
            Stage::WaitForScenery | Stage::Settle | Stage::Done => {}
        }
    }
}

fn crate_stage_budget() -> usize {
    state_snapshot::restore::ORDER.len() + 1
}

// ---------------------------------------------------------------------
// EFB requests (HTTP thread -> flight loop)
// ---------------------------------------------------------------------

enum Request {
    Save(String),
    Load(String),
    Settings(AutosaveSettings),
}

static REQUESTS: Mutex<Vec<Request>> = Mutex::new(Vec::new());
static DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
static SETTINGS: Mutex<Option<AutosaveSettings>> = Mutex::new(None);
static STATUS: Mutex<String> = Mutex::new(String::new());

fn push(r: Request) {
    REQUESTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(r);
}

fn set_status(s: impl Into<String>) {
    *STATUS.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = s.into();
}

fn shared_slots() -> Option<SlotManager> {
    let dir = DIR.lock().ok()?.clone()?;
    let settings = SETTINGS.lock().ok().and_then(|s| *s).unwrap_or_default();
    Some(SlotManager::with_settings(dir, settings))
}

fn field<'a>(body: &'a Value, k: &str) -> Result<&'a str, String> {
    body.get(k).and_then(Value::as_str).ok_or_else(|| format!("missing \"{k}\""))
}

/// `/states/*` for the EFB's Saves page. List, rename and delete touch
/// only files and run here; save, load and settings go to the flight loop.
pub fn route(method: &str, path: &str, body: &str) -> Option<(u16, String)> {
    let rest = path.strip_prefix("/states/")?;
    let post = method.eq_ignore_ascii_case("POST");
    let body: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let ok = |v: Value| (200, v.to_string());
    let err = |status: u16, m: String| (status, json!({ "ok": false, "message": m }).to_string());
    let Some(slots) = shared_slots() else {
        return Some(err(503, "the aircraft is not loaded".into()));
    };
    let result: Result<Value, String> = match (rest, post) {
        ("list", false) => {
            let status = STATUS.lock().map(|s| s.clone()).unwrap_or_default();
            Ok(json!({ "states": slots.list(), "settings": slots.settings(), "status": status }))
        }
        ("save", true) => {
            push(Request::Save(body.get("name").and_then(Value::as_str).unwrap_or("").to_string()));
            Ok(json!({ "ok": true }))
        }
        ("load", true) => field(&body, "id").and_then(|id| {
            if slots.path_of(id).is_none() {
                return Err(format!("no such state {id:?}"));
            }
            push(Request::Load(id.to_string()));
            Ok(json!({ "ok": true }))
        }),
        ("rename", true) => field(&body, "id").and_then(|id| {
            let name = field(&body, "name")?;
            slots.rename(id, name).map_err(|e| format!("{e:?}"))?;
            Ok(json!({ "ok": true }))
        }),
        ("delete", true) => field(&body, "id").and_then(|id| {
            let deleted = slots.delete(id).map_err(|e| format!("{e:?}"))?;
            if deleted {
                let _ = std::fs::remove_file(slots.dir().join(format!("{id}.sit")));
            }
            Ok(json!({ "ok": deleted }))
        }),
        ("settings", true) => serde_json::from_value::<AutosaveSettings>(body.clone()).map_err(|e| e.to_string()).map(|s| {
            push(Request::Settings(s.normalized()));
            json!({ "ok": true, "settings": s.normalized() })
        }),
        _ => return Some(err(404, format!("no route {rest}"))),
    };
    Some(match result {
        Ok(v) => ok(v),
        Err(m) => err(400, m),
    })
}

// ---------------------------------------------------------------------
// The plugin side
// ---------------------------------------------------------------------

pub struct StateSave {
    slots: SlotManager,
    settings_path: PathBuf,
    clock: AutosaveClock,
    watch: PhaseWatch,
    loading: Option<LoadDriver>,
    writer: Option<std::thread::JoinHandle<()>>,
    pushback_id: Option<VariableIdentifier>,
    sample_refs: Option<[Option<DataRef>; 4]>,
    last_tick: Option<Instant>,
    fms_cache: Option<Value>,
    fms_poll_s: f64,
}

fn read_settings(path: &Path) -> AutosaveSettings {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<AutosaveSettings>(&t).ok()).unwrap_or_default().normalized()
}

impl StateSave {
    /// `xplane_root` is the folder `Output/` lives under.
    pub fn new(xplane_root: &Path) -> Self {
        let prefs = xplane_root.join("Output").join("preferences");
        let settings_path = prefs.join(SETTINGS_FILE);
        let settings = read_settings(&settings_path);
        let dir = prefs.join(STATES_FOLDER);
        let slots = SlotManager::with_settings(&dir, settings);
        let swept = slots.sweep_temp_files();
        if swept > 0 {
            crate::log(&format!("state save: removed {swept} unfinished temp file(s)"));
        }
        *DIR.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(dir);
        *SETTINGS.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(settings);
        Self {
            slots,
            settings_path,
            clock: AutosaveClock::from_settings(&settings),
            watch: PhaseWatch::new(),
            loading: None,
            writer: None,
            pushback_id: None,
            sample_refs: None,
            last_tick: None,
            fms_cache: None,
            fms_poll_s: 0.0,
        }
    }

    fn apply_settings(&mut self, s: AutosaveSettings) {
        let removed = self.slots.set_settings(s);
        self.clock = AutosaveClock::from_settings(&s);
        *SETTINGS.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(s);
        if let Ok(text) = serde_json::to_string_pretty(&s) {
            let _ = std::fs::write(&self.settings_path, text);
        }
        crate::log(&format!("state save: settings {s:?}, {removed} surplus autosave(s) removed"));
    }

    /// Hand a snapshot to the writer thread (or write it here when `sync`).
    fn write(&mut self, mut snap: Snapshot, sync: bool) {
        if !self.slots.settings().allows(snap.reason) {
            return;
        }
        let id = self.slots.id_for(snap.reason, &snap.name, snap.created_unix);
        let _ = std::fs::create_dir_all(self.slots.dir());
        let sit = self.slots.dir().join(format!("{id}.sit"));
        crate::xp::save_data_file(crate::xp::DATA_FILE_SITUATION, &sit.to_string_lossy());
        let slots = SlotManager::with_settings(self.slots.dir(), self.slots.settings());
        let mut job = move || match slots.save_if_allowed(&mut snap) {
            Ok(Some(info)) => crate::log(&format!("state save: {} ({:?}) {} bytes", info.id, info.reason, info.bytes)),
            Ok(None) => {}
            Err(e) => crate::log(&format!("state save failed: {e:?}")),
        };
        if sync {
            job();
            return;
        }
        if let Some(h) = self.writer.take() {
            let _ = h.join();
        }
        self.writer = std::thread::Builder::new().name("state-save".into()).spawn(job).ok();
    }
}

fn parts(p: &mut crate::Plugin) -> Parts {
    let x = p.vars.xplm;
    let (random_config, random_rng) = p.random_failures.snapshot();
    let fmgc_phase = p.vars.read(&p.scripted_phase_id).round().clamp(0.0, 255.0) as u8;
    Parts {
        vars: p.vars.snapshot_kinds(),
        failures: FlightFailures {
            active_ids: crate::failures::armed_ids(),
            magnitudes: crate::failures::active_magnitudes(),
            random_config,
            random_rng,
            scripted: p.scripted.snapshot(),
            deferred: p.mel.snapshot(),
            tech_log: p.mel.tech_log(),
        },
        condition: AirframeCondition {
            engines: p.damage.engines,
            wear: crate::wear::snapshot(),
            components: crate::components::snapshot_direct(),
            walkaround: p.walkaround.snapshot(),
            structure: p.damage.structure.strength,
        },
        deep: p.deep.save_areas(),
        plugin_systems: PluginSystems {
            fadec: Some(p.fadec.saved_state()),
            fuel: p.fuel.as_ref().map(crate::fuel::Fuel::saved_state),
            oxygen: Some(p.oxygen.saved_state()),
            tyres: p.tyres.wheels.iter().map(|w| [w.temp_c, w.leaked_fraction, w.tread_mm, f64::from(u8::from(w.fuse_plug_melted)), f64::from(u8::from(w.burst))]).collect(),
            gse: crate::gse::Service::ALL.into_iter().filter(|s| p.gse.connected(*s)).map(|s| s.name().to_string()).collect(),
        },
        fms: p.state_save.fms_cache.clone(),
        situation: read_situation(x),
        start_state: p.start_state_built,
        fmgc_phase,
    }
}

fn sample(p: &mut crate::Plugin) -> Sample {
    let x = p.vars.xplm;
    let [on_ground, ground_speed, parking_brake, engines] = *p.state_save.sample_refs.get_or_insert_with(|| {
        ["sim/flightmodel/failures/onground_any", "sim/flightmodel/position/groundspeed", "sim/cockpit2/controls/parking_brake_ratio", "sim/flightmodel/engine/ENGN_running"].map(|n| x.find(n))
    });
    let num = |d: Option<DataRef>| d.map_or(0.0, |d| x.get_d(d));
    let mut running = [0 as std::ffi::c_int; 4];
    if let Some(d) = engines {
        x.get_vi(d, &mut running);
    }
    let pushback_id = *p.state_save.pushback_id.get_or_insert_with(|| p.vars.register_named("PUSHBACK ATTACHED"));
    Sample {
        fmgc_phase: p.vars.read(&p.scripted_phase_id).round().clamp(0.0, 255.0) as u8,
        on_ground: num(on_ground) != 0.0,
        ground_speed_kt: num(ground_speed) * MS_TO_KT,
        pushback_active: p.vars.read(&pushback_id) != 0.0,
        any_engine_running: running.iter().any(|&r| r != 0),
        parking_brake_set: num(parking_brake) > 0.5,
    }
}

fn save_now(p: &mut crate::Plugin, name: &str, reason: SaveReason, sync: bool) {
    let parts = parts(p);
    let snap = assemble(name, reason, parts);
    p.state_save.write(snap, sync);
}

struct PluginTarget<'a>(&'a mut crate::Plugin);

impl LoadTarget for PluginTarget<'_> {
    fn sim_paused(&self) -> bool {
        let x = self.0.vars.xplm;
        x.find("sim/time/paused").is_some_and(|d| x.get_i(d) != 0)
    }
    fn pause(&mut self, on: bool) {
        crate::xp::command_once(if on { "sim/operation/pause_on" } else { "sim/operation/pause_off" });
    }
    fn place(&mut self, situation: &Value) -> bool {
        place_aircraft(situation)
    }
    fn scenery_loading(&self) -> bool {
        let x = self.0.vars.xplm;
        x.find("sim/graphics/scenery/async_scenery_load_in_progress").is_some_and(|d| x.get_i(d) != 0)
    }
    fn set_situation(&mut self, situation: &Value, position_only: bool) {
        let n = write_situation(self.0.vars.xplm, situation, position_only);
        crate::log(&format!("state load: {n} situation values written"));
    }
    fn rebuild(&mut self, start: StartState) {
        let p = &mut *self.0;
        let xplm = p.vars.xplm;
        let id = p.vars.get("START_STATE".to_owned());
        p.vars.write(&id, start.into());
        p.simulation.rebuild(start, &mut p.vars);
        p.fadec = crate::fadec::Fadec::new(&mut p.vars, xplm);
        p.deep = crate::deep::plugin::DeepLayer::new(&mut p.vars, Some(xplm));
        if let Some(fuel) = p.fuel.as_mut() {
            fuel.restart_with(None);
        }
        if crate::start_state::engines_running_state(start) {
            p.engine_commands.spawn_at_idle();
        } else {
            p.engine_commands.spawn_stopped();
        }
        p.start_state_built = start;
    }
    fn write_vars(&mut self, values: &[(String, f64)]) {
        for (name, v) in values {
            let id = self.0.vars.register_named(name);
            SimulatorReaderWriter::write(&mut self.0.vars, &id, *v);
        }
    }
    fn read_vars(&mut self, names: &[String]) -> Vec<f64> {
        names
            .iter()
            .map(|n| {
                let id = self.0.vars.register_named(n);
                self.0.vars.read(&id)
            })
            .collect()
    }
    fn restore_condition(&mut self, c: AirframeCondition) {
        let p = &mut *self.0;
        let (engines, rejected) = crate::physics::damage::load_engines(c.engines);
        p.damage.engines = engines;
        p.damage.structure.strength = c.structure.sanitized();
        let (wear, wear_rejected) = crate::wear::load_checked(c.wear);
        crate::wear::publish(wear);
        crate::components::restore_direct(c.components);
        p.walkaround.confirm_start(!c.walkaround.is_empty(), &c.walkaround);
        for r in rejected.iter().chain(wear_rejected.iter()) {
            crate::log(&format!("state load: {r}"));
        }
    }
    fn restore_plugin_systems(&mut self, s: PluginSystems) {
        let p = &mut *self.0;
        if let Some(f) = &s.fadec {
            p.fadec.restore_state(f);
        }
        if let (Some(fuel), Some(saved)) = (p.fuel.as_mut(), s.fuel) {
            fuel.restart_with(Some(saved));
        }
        if let Some(o) = s.oxygen {
            p.oxygen.restore_state(o);
        }
        for (w, [temp_c, leaked, tread, plug, burst]) in p.tyres.wheels.iter_mut().zip(s.tyres) {
            w.temp_c = temp_c;
            w.leaked_fraction = leaked.clamp(0.0, 1.0);
            w.tread_mm = tread.max(0.0);
            w.fuse_plug_melted = plug != 0.0;
            w.burst = burst != 0.0;
        }
        for service in crate::gse::Service::ALL {
            if s.gse.iter().any(|n| n == service.name()) != p.gse.connected(service) {
                crate::gse::request(service);
            }
        }
    }
    fn restore_deep(&mut self, saved: &Map<String, Value>) -> Vec<(&'static str, String)> {
        self.0.deep.load_areas(saved)
    }
    fn restore_fms(&mut self, fms: &Value) {
        #[cfg(feature = "js")]
        crate::js_worker::request_eval("state_fms_load", fms_restore_js(fms));
        #[cfg(not(feature = "js"))]
        let _ = fms;
        self.0.state_save.fms_cache = Some(fms.clone());
    }
    fn restore_failures(&mut self, f: FlightFailures) {
        let p = &mut *self.0;
        for id in crate::failures::armed_ids() {
            crate::failures::set_active(id, false);
        }
        for id in &f.active_ids {
            crate::failures::set_active(*id, true);
        }
        crate::failures::restore_magnitudes(f.magnitudes);
        p.scripted.restore(f.scripted);
        p.mel.restore(f.deferred);
        p.mel.restore_tech_log(f.tech_log);
        p.random_failures.restore(f.random_config, f.random_rng);
    }
}

/// Before the systems tick. Returns the delta to tick the systems with
/// while a load is settling (the sim is paused then).
pub fn before_systems(p: &mut crate::Plugin) -> Option<f64> {
    let now = Instant::now();
    let wall = p.state_save.last_tick.map_or(0.0, |t| now.duration_since(t).as_secs_f64());
    p.state_save.last_tick = Some(now);
    let mut driver = p.state_save.loading.take()?;
    let frame = driver.frame(&mut PluginTarget(p), wall);
    set_status(format!("Loading {}: {:?}", driver.snapshot().name, driver.stage()));
    match frame {
        Frame::Running { systems_dt } => {
            p.state_save.loading = Some(driver);
            systems_dt
        }
        Frame::Finished { settle_frames, converged } => {
            for line in &driver.log {
                crate::log(&format!("state load: {line}"));
            }
            crate::log(&format!("state load: settled in {settle_frames} frames ({})", if converged { "converged" } else { "gave up" }));
            let s = sample(p);
            let airborne = !s.on_ground;
            p.state_save.watch.prime(s, airborne);
            p.state_save.clock.reset();
            set_status(format!("Loaded {}", driver.snapshot().name));
            None
        }
    }
}

/// After the systems tick: EFB requests, phase saves and the autosave clock.
pub fn tick(p: &mut crate::Plugin, sim_delta: f64) {
    let requests = std::mem::take(&mut *REQUESTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
    for r in requests {
        match r {
            Request::Settings(s) => p.state_save.apply_settings(s),
            Request::Save(name) => {
                let name = if name.trim().is_empty() { "Saved flight".to_string() } else { name };
                save_now(p, &name, SaveReason::Manual, false);
                set_status(format!("Saved {name}"));
            }
            Request::Load(id) => match p.state_save.slots.load(&id) {
                Ok(snap) => {
                    crate::log(&format!("state load: {id} ({})", snap.name));
                    p.state_save.loading = Some(LoadDriver::new(snap, SettleLimits::default()));
                }
                Err(e) => {
                    crate::log(&format!("state load: {id} failed: {e:?}"));
                    set_status(format!("Could not load {id}"));
                }
            },
        }
    }
    poll_fms(&mut p.state_save, real_delta_s(sim_delta));
    if p.state_save.loading.is_some() {
        return;
    }
    let s = sample(p);
    if let Some(event) = p.state_save.watch.update(s) {
        save_now(p, event.label(), SaveReason::Phase(event), false);
    }
    if p.state_save.clock.tick(sim_delta, sim_delta > 0.0) {
        save_now(p, "Autosave", SaveReason::Autosave, false);
    }
}

fn real_delta_s(sim_delta: f64) -> f64 {
    sim_delta.max(0.0)
}

/// Keeps a recent copy of the JS FMS's flight plans for the next save, and
/// logs how a requested FMS restore went.
fn poll_fms(s: &mut StateSave, dt: f64) {
    #[cfg(feature = "js")]
    {
        if let Some(r) = crate::js_worker::take_eval("state_fms_save") {
            match r.ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
                Some(v) => s.fms_cache = Some(v),
                None => crate::log("state save: the FMS's flight plans could not be read"),
            }
        }
        if let Some(r) = crate::js_worker::take_eval("state_fms_load") {
            crate::log(&format!("state load: FMS restore {}", r.unwrap_or_else(|e| e)));
        }
        s.fms_poll_s += dt;
        if s.fms_poll_s >= FMS_POLL_S && s.loading.is_none() {
            s.fms_poll_s = 0.0;
            crate::js_worker::request_eval("state_fms_save", FMS_SAVE_JS.to_string());
        }
    }
    #[cfg(not(feature = "js"))]
    let _ = (s, dt);
}

/// On plugin stop/disable, synchronous.
pub fn on_exit(p: &mut crate::Plugin) {
    if p.state_save.loading.is_none() {
        save_now(p, "Last flight", SaveReason::Quit, true);
    }
    if let Some(h) = p.state_save.writer.take() {
        let _ = h.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failures() -> FlightFailures {
        let (random_config, random_rng) = crate::random_failures::RandomFailures::new(7).snapshot();
        FlightFailures { active_ids: vec![26001, 29002], magnitudes: BTreeMap::from([(26001, 1.0), (29002, 0.35)]), random_config, random_rng, scripted: Vec::new(), deferred: Vec::new(), tech_log: Vec::new() }
    }

    fn condition() -> AirframeCondition {
        AirframeCondition { engines: Default::default(), wear: Default::default(), components: Vec::new(), walkaround: BTreeMap::from([("pitot_covers".to_string(), false)]), structure: Default::default() }
    }

    fn parts() -> Parts {
        let mut deep = Map::new();
        deep.insert("fire_ice".into(), json!({ "burning": [1, 0] }));
        let plugin_systems = PluginSystems {
            fadec: Some(crate::fadec::SavedFadec { initialized: true, thermal_energy: [1.0, 2.0, 3.0, 4.0], latched_flex_temperature: 45.0, ..Default::default() }),
            fuel: Some(crate::fuel::SavedFuel { gallons: vec![1000.0, 2500.5], temp_c: vec![-12.0; 11], valves: vec![true, false], pumps: vec![2, 0] }),
            oxygen: Some((3.25, 88.0, true)),
            tyres: vec![[40.0, 0.0, 12.0, 0.0, 0.0], [150.0, 1.0, 3.5, 1.0, 1.0]],
            gse: vec!["gpu".into()],
        };
        Parts {
            plugin_systems,
            fms: Some(json!({ "plans": { "0": { "activeLegIndex": 7 } } })),
            vars: vec![(true, "A32NX_OVHD_ELEC_BAT_1_PB_IS_AUTO".into(), 1.0), (true, "A32NX_ADIRS_IR_1_STATE".into(), f64::NAN), (true, "A32NX_RMP_L_BTN_PRESSED".into(), 1.0), (false, "PLANE LATITUDE".into(), 51.4)],
            failures: failures(),
            condition: condition(),
            deep,
            situation: json!({ "scalars": { "sim/flightmodel/position/latitude": 51.47, "sim/flightmodel/position/longitude": -0.45, "sim/flightmodel/position/elevation": 3048.0, "sim/time/paused": 0.0, "sim/flightmodel/position/psi": 270.0 }, "arrays": { "sim/flightmodel/position/q": [1.0, 0.0, 0.0, 0.0] } }),
            start_state: StartState::Cruise,
            fmgc_phase: state_snapshot::phase::fmgc::CRUISE,
        }
    }

    fn round_trip() -> Snapshot {
        let snap = assemble("Over the Channel", SaveReason::Manual, parts());
        let bytes = state_snapshot::codec::encode(&snap).unwrap();
        state_snapshot::codec::decode(&bytes).unwrap()
    }

    #[test]
    fn a_snapshot_carries_every_section_and_a_summary() {
        let snap = round_trip();
        for name in [vars::SECTION, FAILURES, CONDITION, DEEP, PLUGIN_SYSTEMS, SYSTEMS, SITUATION] {
            assert!(snap.sections.contains_key(name), "{name} missing");
        }
        assert_eq!(snap.summary.flight_phase.as_deref(), Some("cruise"));
        assert_eq!(snap.summary.altitude_ft.map(|f| f.round()), Some(10000.0));
        assert_eq!(snap.aircraft.name, "A380 for X-Plane 12");
    }

    #[test]
    fn named_variables_come_back_without_momentary_ones_or_simulator_ones() {
        let snap = round_trip();
        let loaded = variables_to_write(&snap);
        let names: Vec<&str> = loaded.write.iter().map(|(_, n, _)| n.as_str()).collect();
        assert!(names.contains(&"A32NX_OVHD_ELEC_BAT_1_PB_IS_AUTO"));
        assert!(!names.contains(&"A32NX_RMP_L_BTN_PRESSED"));
        assert!(!names.contains(&"PLANE LATITUDE"));
        let ir = loaded.write.iter().find(|(_, n, _)| n == "A32NX_ADIRS_IR_1_STATE").unwrap();
        assert!(ir.2.is_nan());
    }

    #[derive(Default)]
    struct Fake {
        paused: bool,
        scenery_frames: u32,
        calls: Vec<String>,
        vars: BTreeMap<String, f64>,
        failures: Option<FlightFailures>,
        systems: Option<PluginSystems>,
        fms: Option<Value>,
        deep: Option<Map<String, Value>>,
        walkaround: Option<BTreeMap<String, bool>>,
    }

    impl LoadTarget for Fake {
        fn sim_paused(&self) -> bool {
            self.paused
        }
        fn pause(&mut self, on: bool) {
            self.paused = on;
            self.calls.push(format!("pause {on}"));
        }
        fn place(&mut self, s: &Value) -> bool {
            self.calls.push("place".into());
            self.scenery_frames = 3;
            s.get("scalars").is_some()
        }
        fn scenery_loading(&self) -> bool {
            self.scenery_frames > 0
        }
        fn set_situation(&mut self, _: &Value, position_only: bool) {
            self.calls.push(format!("situation {position_only}"));
        }
        fn rebuild(&mut self, start: StartState) {
            self.calls.push(format!("rebuild {start:?}"));
        }
        fn write_vars(&mut self, values: &[(String, f64)]) {
            self.calls.push("vars".into());
            self.vars.extend(values.iter().cloned());
        }
        fn read_vars(&mut self, names: &[String]) -> Vec<f64> {
            if self.scenery_frames > 0 {
                self.scenery_frames -= 1;
            }
            names.iter().map(|n| self.vars.get(n).copied().unwrap_or(0.0)).collect()
        }
        fn restore_condition(&mut self, c: AirframeCondition) {
            self.calls.push("condition".into());
            self.walkaround = Some(c.walkaround);
        }
        fn restore_plugin_systems(&mut self, s: PluginSystems) {
            self.calls.push("systems".into());
            self.systems = Some(s);
        }
        fn restore_deep(&mut self, saved: &Map<String, Value>) -> Vec<(&'static str, String)> {
            self.calls.push("deep".into());
            self.deep = Some(saved.clone());
            Vec::new()
        }
        fn restore_failures(&mut self, f: FlightFailures) {
            self.calls.push("failures".into());
            self.failures = Some(f);
        }
        fn restore_fms(&mut self, fms: &Value) {
            self.calls.push("fms".into());
            self.fms = Some(fms.clone());
        }
    }

    fn run(fake: &mut Fake, snap: Snapshot) -> (u32, bool, u32) {
        let limits = SettleLimits { min_frames: 5, min_system_s: 0.1, max_frames: 50, quiet_frames: 3, scenery_timeout_s: 5.0 };
        let mut driver = LoadDriver::new(snap, limits);
        let mut settle_ticks = 0;
        for _ in 0..200 {
            if driver.stage() == Stage::WaitForScenery && fake.scenery_frames > 0 {
                fake.scenery_frames -= 1;
            }
            match driver.frame(fake, 1.0 / 30.0) {
                Frame::Running { systems_dt: Some(dt) } => {
                    assert_eq!(dt, SYNTHETIC_DT_S);
                    settle_ticks += 1;
                }
                Frame::Running { systems_dt: None } => {}
                Frame::Finished { settle_frames, converged } => return (settle_frames, converged, settle_ticks),
            }
        }
        panic!("load never finished at {:?}", driver.stage());
    }

    #[test]
    fn a_load_pauses_places_restores_in_order_settles_and_resumes() {
        let mut fake = Fake::default();
        let (frames, converged, ticks) = run(&mut fake, round_trip());
        assert!(converged);
        assert!(frames >= 5 && ticks + 1 >= frames);
        let expected = ["pause true", "place", "situation false", "rebuild Cruise", "vars", "condition", "systems", "deep", "failures", "fms", "situation true", "pause false"];
        assert_eq!(fake.calls, expected);
        assert_eq!(fake.vars.get("A32NX_OVHD_ELEC_BAT_1_PB_IS_AUTO"), Some(&1.0));
        assert_eq!(fake.failures.unwrap().magnitudes.get(&29002), Some(&0.35));
        assert_eq!(fake.deep.unwrap()["fire_ice"], json!({ "burning": [1, 0] }));
        assert_eq!(fake.walkaround.unwrap().get("pitot_covers"), Some(&false));
        let systems = fake.systems.unwrap();
        assert_eq!(systems.fadec.unwrap().thermal_energy, [1.0, 2.0, 3.0, 4.0]);
        assert_eq!(systems.fuel.unwrap().gallons, vec![1000.0, 2500.5]);
        assert_eq!(systems.oxygen, Some((3.25, 88.0, true)));
        assert_eq!(systems.tyres[1], [150.0, 1.0, 3.5, 1.0, 1.0]);
        assert_eq!(systems.gse, vec!["gpu".to_string()]);
        assert_eq!(fake.fms.unwrap()["plans"]["0"]["activeLegIndex"], 7);
        assert!(serde_json::from_value::<PluginSystems>(json!({ "oxygen": [1.0, 2.0, false] })).unwrap().gse.is_empty());
    }

    #[test]
    fn the_fms_restore_script_carries_the_saved_plans() {
        let saved = json!({ "plans": { "0": { "activeLegIndex": 7, "segments": {} }, "3": { "activeLegIndex": 1 } } });
        let js = fms_restore_js(&saved);
        assert!(js.contains(r#""activeLegIndex":7"#));
        assert!(js.contains("fromSerializedFlightPlan"));
        assert_eq!(js.matches('{').count(), js.matches('}').count());
        assert!(FMS_SAVE_JS.contains("serialize()"));
    }

    #[test]
    fn a_state_saved_while_paused_stays_paused_after_the_load() {
        let mut p = parts();
        p.situation["scalars"]["sim/time/paused"] = json!(1.0);
        let snap = assemble("paused", SaveReason::Manual, p);
        let mut fake = Fake::default();
        run(&mut fake, snap);
        assert!(fake.paused);
        assert!(!fake.calls.contains(&"pause false".to_string()));
    }

    #[test]
    fn efb_routes_list_queue_and_refuse_bad_input() {
        let dir = std::env::temp_dir().join(format!("a380-state-route-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        *DIR.lock().unwrap() = Some(dir.clone());
        let slots = SlotManager::new(&dir);
        let mut snap = assemble("Route test", SaveReason::Manual, parts());
        let info = slots.save(&mut snap).unwrap();
        let (status, body) = route("GET", "/states/list", "").unwrap();
        assert_eq!(status, 200);
        assert!(body.contains("Route test"));
        let (status, _) = route("POST", "/states/rename", &json!({ "id": info.id, "name": "Renamed" }).to_string()).unwrap();
        assert_eq!(status, 200);
        assert_eq!(slots.load(&info.id).unwrap().name, "Renamed");
        let (status, _) = route("POST", "/states/load", r#"{"id":"../../etc"}"#).unwrap();
        assert_eq!(status, 400);
        let (status, _) = route("POST", "/states/load", &json!({ "id": info.id }).to_string()).unwrap();
        assert_eq!(status, 200);
        let (status, _) = route("POST", "/states/settings", r#"{"interval_min":7,"rolling_slots":40,"phase_saves":false,"quit_save":true}"#).unwrap();
        assert_eq!(status, 200);
        let queued = std::mem::take(&mut *REQUESTS.lock().unwrap());
        assert!(queued.iter().any(|r| matches!(r, Request::Load(id) if *id == info.id)));
        assert!(queued.iter().any(|r| matches!(r, Request::Settings(s) if s.rolling_slots == 10 && !s.phase_saves)));
        let (status, _) = route("POST", "/states/delete", &json!({ "id": info.id }).to_string()).unwrap();
        assert_eq!(status, 200);
        assert!(slots.list().is_empty());
        assert!(route("GET", "/other", "").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn situation_values_round_trip_through_datarefs() {
        Xplm::reset_memory();
        let x: &'static Xplm = Box::leak(Box::new(Xplm::memory()));
        Xplm::memory_set("sim/flightmodel/position/psi", 0, 123.5);
        Xplm::memory_set("sim/flightmodel/weight/m_fixed", 0, 42000.0);
        for (i, q) in [0.5, 0.5, 0.5, 0.5].into_iter().enumerate() {
            Xplm::memory_set("sim/flightmodel/position/q", i, q);
        }
        let saved = read_situation(x);
        Xplm::memory_set("sim/flightmodel/position/psi", 0, 0.0);
        Xplm::memory_set("sim/flightmodel/weight/m_fixed", 0, 0.0);
        Xplm::memory_set("sim/flightmodel/position/q", 0, 1.0);
        write_situation(x, &saved, false);
        let back = read_situation(x);
        assert_eq!(situation_number(&back, "sim/flightmodel/position/psi"), Some(123.5));
        assert_eq!(situation_number(&back, "sim/flightmodel/weight/m_fixed"), Some(42000.0));
        assert_eq!(back["arrays"]["sim/flightmodel/position/q"], saved["arrays"]["sim/flightmodel/position/q"]);
    }
}

//! The native computers (crates/gpws, crates/tcas, crates/hosts_misc) running
//! beside FlyByWire's JS. In shadow mode each frame builds a computer's
//! inputs from the variable store, runs it, and compares its outputs with
//! what the JS wrote to the same variables; nothing native reaches the sim.
//! A system named in `FBW_NATIVE_SYSTEMS` (or `nativeSystems` in the app
//! settings, comma separated) is authoritative instead: its outputs are
//! written, and `L:A380X_NATIVE_SYSTEMS_MASK` tells the JS side which
//! modules to stand down.
//!
//! An authoritative GPWS or TCAS also plays its aurals through the instrument
//! sound path and hands its bus events and ND traffic to SystemsHost through
//! `Coherent.call("A380X_NATIVE_SYSTEMS_TAKE")` (`native_shadow_patches.rs`).
//!
//! Log lines: `native shadow: <system> <var> native=<x> fbw=<y>` on the first
//! sustained difference of a variable, and a `native shadow: summary ...`
//! block every 10 s.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;

use gpws::{vars as gv, Event as GpwsEvent, Gpws};
use hosts_misc::fws_host::{FwsHost, FwsHostInputs};
use tcas::{consts as tc, NdTraffic, RawTraffic, Tcas};

use crate::js_bridge::{EnvRefs, HostState, Resolved, VarsHost};
use crate::native_shadow_patches::{FWS_STARTUP_VAR, TAKE_CALL, TCAS_ALERT_LEVEL_VAR, TCAS_ALT_SELECT_VAR, TCAS_MIRROR_VAR};
use crate::Vars;

pub use crate::native_shadow_patches::MASK_VAR;
const MAX_QUEUED_BUS_EVENTS: usize = 64;
const SUMMARY_PERIOD_S: f64 = 10.;
const SETTINGS_PERIOD_S: f64 = 10.;
const TCAS_PERIOD_MS: f64 = 1000.;
const MAX_STEP_MS: f64 = 250.;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum System {
    Gpws,
    Tcas,
    HostsMisc,
}

impl System {
    pub const ALL: [System; 3] = [System::Gpws, System::Tcas, System::HostsMisc];

    pub fn name(self) -> &'static str {
        match self {
            System::Gpws => "gpws",
            System::Tcas => "tcas",
            System::HostsMisc => "hosts_misc",
        }
    }

    fn mask_bit(self) -> f64 {
        match self {
            System::Gpws => 1.,
            System::Tcas => 2.,
            System::HostsMisc => 4.,
        }
    }

    fn debounce_s(self) -> f64 {
        match self {
            System::Gpws => 0.6,
            System::Tcas => 2.5,
            System::HostsMisc => 1.,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub off: bool,
    pub authoritative: BTreeSet<System>,
}

impl Config {
    pub fn parse(raw: &str) -> Config {
        let mut c = Config::default();
        for token in raw.split(|ch: char| ch == ',' || ch == ';' || ch.is_whitespace()).map(str::trim).filter(|t| !t.is_empty()) {
            match token.to_ascii_lowercase().as_str() {
                "off" | "none" => c.off = true,
                "all" => c.authoritative.extend(System::ALL),
                other => {
                    if let Some(s) = System::ALL.iter().find(|s| s.name() == other) {
                        c.authoritative.insert(*s);
                    }
                }
            }
        }
        c
    }

    pub fn authoritative(&self, s: System) -> bool {
        !self.off && self.authoritative.contains(&s)
    }

    fn mask(&self) -> f64 {
        System::ALL.iter().filter(|s| self.authoritative(**s)).map(|s| s.mask_bit()).sum()
    }
}

pub(crate) fn settings_text() -> String {
    crate::app_settings::native_mode::systems_spec()
}

pub trait Io {
    fn get(&mut self, name: &str, unit: &str) -> f64;
    fn set(&mut self, name: &str, value: f64);
    fn traffic(&mut self) -> Vec<RawTraffic>;
    fn failure(&mut self, id: u64) -> bool;
    fn play_sound(&mut self, name: &str);
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToJs {
    pub bus: Vec<(&'static str, &'static str)>,
    pub traffic: Option<(Vec<NdTraffic>, Vec<NdTraffic>)>,
}

impl ToJs {
    fn merge(&mut self, newer: ToJs) {
        self.bus.extend(newer.bus);
        let excess = self.bus.len().saturating_sub(MAX_QUEUED_BUS_EVENTS);
        self.bus.drain(..excess);
        if newer.traffic.is_some() {
            self.traffic = newer.traffic;
        }
    }

    fn to_json(&self) -> String {
        let traffic = |list: &[NdTraffic]| {
            list.iter()
                .map(|t| {
                    serde_json::json!({
                        "ID": t.id,
                        "lat": t.lat,
                        "lon": t.lon,
                        "relativeAlt": t.relative_alt,
                        "intrusionLevel": t.intrusion_level,
                        "vertSpeed": t.vert_speed,
                    })
                })
                .collect::<Vec<_>>()
        };
        serde_json::json!({
            "bus": self.bus.iter().map(|(topic, data)| [topic, data]).collect::<Vec<_>>(),
            "traffic": self.traffic.as_ref().map(|(l, r)| serde_json::json!({ "l": traffic(l), "r": traffic(r) })),
        })
        .to_string()
    }
}

static OUTBOX: Mutex<Option<ToJs>> = Mutex::new(None);

pub fn coherent_call(name: &str) -> Option<String> {
    if name != TAKE_CALL {
        return None;
    }
    let taken = OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap_or_default();
    Some(taken.to_json())
}

fn gpws_sound_length(name: &str) -> f64 {
    use gpws::sound::*;
    [PULL_UP, SINK_RATE, DONT_SINK, TOO_LOW_GEAR, TOO_LOW_FLAPS, TOO_LOW_TERRAIN, TERRAIN_AHEAD, TERRAIN_AHEAD_PULL_UP, OBSTACLE_AHEAD, OBSTACLE_AHEAD_PULL_UP]
        .iter()
        .find(|s| s.name == name)
        .map_or(1., |s| s.length)
}

pub fn parse_traffic(json: &str) -> Vec<RawTraffic> {
    let Ok(serde_json::Value::Array(list)) = serde_json::from_str::<serde_json::Value>(json) else { return Vec::new() };
    list.iter()
        .map(|t| {
            let n = |k: &str| t.get(k).and_then(serde_json::Value::as_f64).unwrap_or(0.);
            RawTraffic { uid: n("uId"), lat: n("lat"), lon: n("lon"), alt: n("alt"), heading: n("heading") }
        })
        .collect()
}

fn same(a: f64, b: f64) -> bool {
    if a.is_nan() && b.is_nan() {
        return true;
    }
    (a - b).abs() <= 1e-9 + 1e-6 * a.abs().max(b.abs())
}

pub trait Compared: Copy + Ord {
    fn label(self) -> &'static str;
    fn debounce(self) -> f64;
}

impl Compared for System {
    fn label(self) -> &'static str {
        self.name()
    }

    fn debounce(self) -> f64 {
        self.debounce_s()
    }
}

#[derive(Default)]
struct Track {
    since: Option<f64>,
    logged: bool,
    window: u64,
    native: String,
    fbw: String,
}

pub struct Comparator<S: Compared = System> {
    tracks: BTreeMap<(S, String), Track>,
    compared: BTreeMap<S, u64>,
    next_summary: Option<f64>,
}

impl<S: Compared> Default for Comparator<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Compared> Comparator<S> {
    pub fn new() -> Self {
        Comparator { tracks: BTreeMap::new(), compared: BTreeMap::new(), next_summary: None }
    }

    pub fn observe(&mut self, system: S, var: &str, native: f64, fbw: f64, now: f64) -> Option<String> {
        self.observe_with(system, var, same(native, fbw), || (native.to_string(), fbw.to_string()), now)
    }

    pub fn observe_text(&mut self, system: S, var: &str, native: &str, fbw: &str, now: f64) -> Option<String> {
        self.observe_with(system, var, native == fbw, || (format!("{native:?}"), format!("{fbw:?}")), now)
    }

    fn observe_with(&mut self, system: S, var: &str, equal: bool, shown: impl FnOnce() -> (String, String), now: f64) -> Option<String> {
        *self.compared.entry(system).or_default() += 1;
        let t = self.tracks.entry((system, var.to_string())).or_default();
        if equal {
            t.since = None;
            return None;
        }
        let since = *t.since.get_or_insert(now);
        if now - since < system.debounce() {
            return None;
        }
        t.window += 1;
        (t.native, t.fbw) = shown();
        if t.logged {
            return None;
        }
        t.logged = true;
        Some(format!("native shadow: {} {} native={} fbw={}", system.label(), var, t.native, t.fbw))
    }

    pub fn summary(&mut self, now: f64) -> Vec<String> {
        let due = *self.next_summary.get_or_insert(now + SUMMARY_PERIOD_S);
        if now < due {
            return Vec::new();
        }
        self.next_summary = Some(now + SUMMARY_PERIOD_S);
        let mut lines = Vec::new();
        for (system, compared) in std::mem::take(&mut self.compared) {
            let mut differing = Vec::new();
            for ((s, var), t) in self.tracks.iter_mut() {
                if *s == system && t.window > 0 {
                    differing.push(format!("{var} x{} native={} fbw={}", t.window, t.native, t.fbw));
                    t.window = 0;
                }
            }
            if differing.is_empty() {
                lines.push(format!("native shadow: summary {} clean ({compared} comparisons)", system.label()));
            } else {
                lines.push(format!("native shadow: summary {} {} variables differ ({compared} comparisons): {}", system.label(), differing.len(), differing.join("; ")));
            }
        }
        lines
    }
}

fn gpws_unit(name: &str) -> &'static str {
    match name {
        "SIM ON GROUND" => "bool",
        "VERTICAL SPEED" => "feet per minute",
        "AIRSPEED INDICATED" => "knots",
        "GEAR TOTAL PCT EXTENDED" => "percent",
        "PLANE ALTITUDE" => "feet",
        _ => "number",
    }
}

fn gpws_inputs(io: &mut dyn Io) -> gpws::Inputs {
    let mut inputs = gpws::Inputs::default();
    for name in gv::INPUT_VARS.iter().copied().chain([gv::F_FACTOR_VAR]) {
        let v = io.get(name, gpws_unit(name));
        inputs.set_var(name, v);
    }
    inputs
}

fn gpws_outputs(g: &Gpws) -> [(&'static str, f64); 6] {
    let o = g.outputs();
    [
        (gv::OUTPUT_GPWS_WARNING, o.gpws_warning_active as u8 as f64),
        (gv::OUTPUT_GS_WARNING, o.gs_warning_active as u8 as f64),
        (gv::OUTPUT_ALERT_1_WORD_1, o.alert_word_1),
        (gv::OUTPUT_ALERT_2_WORD_1, o.alert_word_1),
        (gv::OUTPUT_ALERT_1_WORD_2, o.alert_word_2),
        (gv::OUTPUT_ALERT_2_WORD_2, o.alert_word_2),
    ]
}

const TCAS_MODE: &str = "L:A32NX_TCAS_MODE";

fn tcas_inputs(io: &mut dyn Io, alert_level: u8, alt_select: u8) -> tcas::Inputs {
    let mut i = tcas::Inputs::default();
    i.lat = io.get("PLANE LATITUDE", "degree latitude");
    i.lon = io.get("PLANE LONGITUDE", "degree longitude");
    i.alt_ft = io.get("PLANE ALTITUDE", "feet");
    i.vertical_speed = io.get("VERTICAL SPEED", "feet per minute");
    i.alt_above_ground = io.get("PLANE ALT ABOVE GROUND", "feet");
    i.ac_ess_powered = io.get("L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED", "bool") != 0.;
    i.ac_2_powered = io.get("L:A32NX_ELEC_AC_2_BUS_IS_POWERED", "bool") != 0.;
    i.xpdr_system = io.get("L:A32NX_TRANSPONDER_SYSTEM", "number");
    for n in 0..3 {
        i.transponder_state[n] = io.get(&format!("TRANSPONDER STATE:{}", n + 1), "number");
        i.adr_altitude[n] = io.get(&format!("L:A32NX_ADIRS_ADR_{}_ALTITUDE", n + 1), "number");
        i.ir_true_heading[n] = io.get(&format!("L:A32NX_ADIRS_IR_{}_TRUE_HEADING", n + 1), "number");
    }
    i.air_data_knob = io.get("L:A32NX_AIR_DATA_SWITCHING_KNOB", "number");
    i.radio_alt[0] = io.get("L:A32NX_RA_1_RADIO_ALTITUDE", "number");
    i.radio_alt[1] = io.get("L:A32NX_RA_2_RADIO_ALTITUDE", "number");
    i.att_hdg_knob = io.get("L:A32NX_ATT_HDG_SWITCHING_KNOB", "number");
    i.slew_active = io.get("IS SLEW ACTIVE", "bool") != 0.;
    i.gpws_warning = io.get(gv::OUTPUT_GPWS_WARNING, "bool") != 0.;
    i.traffic_left_efis = io.get("L:A380X_EFIS_L_TRAF_BUTTON_IS_ON", "bool") != 0.;
    i.traffic_right_efis = io.get("L:A380X_EFIS_R_TRAF_BUTTON_IS_ON", "bool") != 0.;
    i.alert_level = alert_level;
    i.alt_select = alt_select;
    i
}

fn tcas_outputs(v: &tcas::Vars) -> [(&'static str, f64); 14] {
    [
        (TCAS_MODE, v.mode as f64),
        ("L:A32NX_TCAS_STATE", v.state as f64),
        ("L:A32NX_TCAS_FAULT", v.fault as u8 as f64),
        ("L:A32NX_TCAS_TA_ONLY", v.ta_only as u8 as f64),
        ("L:A32NX_TCAS_RA_CORRECTIVE", v.ra_corrective as u8 as f64),
        ("L:A32NX_TCAS_RA_TYPE", v.ra_type as f64),
        ("L:A32NX_TCAS_RA_RATE_TO_MAINTAIN", v.rate_to_maintain),
        ("L:A32NX_TCAS_RA_UP_ADVISORY_STATUS", v.up_advisory as f64),
        ("L:A32NX_TCAS_RA_DOWN_ADVISORY_STATUS", v.down_advisory as f64),
        ("L:A32NX_TCAS_SENSITIVITY", v.sensitivity as f64),
        ("L:A32NX_TCAS_VSPEED_RED:1", v.vspeed_red[0]),
        ("L:A32NX_TCAS_VSPEED_RED:2", v.vspeed_red[1]),
        ("L:A32NX_TCAS_VSPEED_GREEN:1", v.vspeed_green[0]),
        ("L:A32NX_TCAS_VSPEED_GREEN:2", v.vspeed_green[1]),
    ]
}

const FWS1_FAILURE: u64 = 31100;
const FWS2_FAILURE: u64 = 31101;
const FWS_ECP_FAILURE: u64 = 31104;

fn fws_inputs(io: &mut dyn Io) -> FwsHostInputs {
    FwsHostInputs {
        fws1_failure_active: io.failure(FWS1_FAILURE),
        fws2_failure_active: io.failure(FWS2_FAILURE),
        fws_ecp_failure_active: io.failure(FWS_ECP_FAILURE),
        reset_panel_fws1_pulled: io.get("L:A32NX_RESET_PANEL_FWS1", "bool") != 0.,
        reset_panel_fws2_pulled: io.get("L:A32NX_RESET_PANEL_FWS2", "bool") != 0.,
        cpiom_c1_available: io.get("L:A32NX_CPIOM_C1_AVAIL", "bool") != 0.,
        cpiom_c2_available: io.get("L:A32NX_CPIOM_C2_AVAIL", "bool") != 0.,
        core_startup_completed: io.get(FWS_STARTUP_VAR, "bool") != 0.,
        dc_ess_powered: io.get("L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED", "bool") != 0.,
        afdx_3_3_reachable: io.get("L:A32NX_AFDX_3_3_REACHABLE", "bool") != 0.,
        afdx_13_13_reachable: io.get("L:A32NX_AFDX_13_13_REACHABLE", "bool") != 0.,
        afdx_4_4_reachable: io.get("L:A32NX_AFDX_4_4_REACHABLE", "bool") != 0.,
        afdx_14_14_reachable: io.get("L:A32NX_AFDX_14_14_REACHABLE", "bool") != 0.,
    }
}

const FWS1_HEALTHY: &str = "L:A32NX_FWS1_IS_HEALTHY";
const FWS2_HEALTHY: &str = "L:A32NX_FWS2_IS_HEALTHY";
const FWS_ECP_FAILED: &str = "L:A32NX_FWS_ECP_FAILED";

pub struct Shadow {
    config: Config,
    config_read_at: Option<f64>,
    config_fixed: bool,
    last_time: Option<f64>,
    gpws: Gpws,
    tcas: Tcas,
    tcas_acc_ms: f64,
    tcas_outputs: Option<[(&'static str, f64); 14]>,
    alert_level: u8,
    alt_select: u8,
    fws: FwsHost,
    comparator: Comparator,
    mask_written: f64,
    announced: bool,
    to_js: ToJs,
    gpws_sound_until: f64,
}

impl Default for Shadow {
    fn default() -> Self {
        Self::new()
    }
}

impl Shadow {
    pub fn new() -> Self {
        Shadow {
            config: Config::default(),
            config_read_at: None,
            config_fixed: false,
            last_time: None,
            gpws: Gpws::new(),
            tcas: Tcas::new(),
            tcas_acc_ms: 0.,
            tcas_outputs: None,
            alert_level: tc::MODE_TARA,
            alt_select: 0,
            fws: FwsHost::new(),
            comparator: Comparator::new(),
            mask_written: -1.,
            announced: false,
            to_js: ToJs::default(),
            gpws_sound_until: f64::NEG_INFINITY,
        }
    }

    pub fn take_to_js(&mut self) -> ToJs {
        std::mem::take(&mut self.to_js)
    }

    #[cfg(test)]
    fn set_config(&mut self, config: Config) {
        self.config = config;
    }

    pub fn frame(&mut self, io: &mut dyn Io, time_s: f64, lines: &mut Vec<String>) {
        if !self.config_fixed && self.config_read_at.is_none_or(|t| time_s - t >= SETTINGS_PERIOD_S || time_s < t) {
            self.config_read_at = Some(time_s);
            let config = Config::parse(&settings_text());
            if config != self.config || !self.announced {
                self.announced = true;
                let names = |auth: bool| System::ALL.iter().filter(|s| config.authoritative(**s) == auth).map(|s| s.name()).collect::<Vec<_>>().join(",");
                lines.push(if config.off {
                    "native shadow: off".to_string()
                } else {
                    format!("native shadow: shadow=[{}] authoritative=[{}]", names(false), names(true))
                });
            }
            self.config = config;
        }
        let dt_ms = self.last_time.map_or(0., |t| ((time_s - t) * 1000.).clamp(0., MAX_STEP_MS));
        self.last_time = Some(time_s);
        if self.config.off {
            return;
        }
        let mask = self.config.mask();
        if mask != self.mask_written {
            self.mask_written = mask;
            io.set(MASK_VAR, mask);
        }
        if dt_ms <= 0. {
            return;
        }
        self.run_gpws(io, dt_ms, time_s, lines);
        self.run_tcas(io, dt_ms, time_s, lines);
        self.run_hosts(io, time_s, lines);
        lines.extend(self.comparator.summary(time_s));
    }

    fn settle(&mut self, system: System, io: &mut dyn Io, outputs: &[(&'static str, f64)], now: f64, lines: &mut Vec<String>) {
        let authoritative = self.config.authoritative(system);
        for (var, native) in outputs {
            if authoritative {
                io.set(var, *native);
            } else {
                let fbw = io.get(var, "number");
                lines.extend(self.comparator.observe(system, var, *native, fbw, now));
            }
        }
    }

    fn run_gpws(&mut self, io: &mut dyn Io, dt_ms: f64, now: f64, lines: &mut Vec<String>) {
        let inputs = gpws_inputs(io);
        self.gpws.update(dt_ms, &inputs);
        let events = self.gpws.take_events();
        let sounds = self.gpws.take_played_sounds();
        if self.config.authoritative(System::Gpws) {
            for e in &events {
                if let GpwsEvent::Enqueue(name) = e {
                    crate::native_systems_host::enqueue_fws_sound(name);
                }
            }
            for name in sounds {
                io.play_sound(name);
                self.gpws_sound_until = now + gpws_sound_length(name);
            }
            self.to_js.bus.extend(events.into_iter().map(|e| match e {
                GpwsEvent::Enqueue(name) => ("enqueueSound", name),
                GpwsEvent::Dequeue(name) => ("dequeueSound", name),
            }));
        }
        let outputs = gpws_outputs(&self.gpws);
        self.settle(System::Gpws, io, &outputs, now, lines);
    }

    fn run_tcas(&mut self, io: &mut dyn Io, dt_ms: f64, now: f64, lines: &mut Vec<String>) {
        self.tcas_acc_ms += dt_ms;
        if self.tcas_acc_ms >= TCAS_PERIOD_MS {
            let authoritative = self.config.authoritative(System::Tcas);
            if io.get(TCAS_MIRROR_VAR, "number") != 0. {
                self.alert_level = (io.get(TCAS_ALERT_LEVEL_VAR, "number").max(0.) as u8).min(tc::MODE_TARA);
                self.alt_select = (io.get(TCAS_ALT_SELECT_VAR, "number").max(0.) as u8).min(2);
            } else if !authoritative {
                let fbw_mode = io.get(TCAS_MODE, "number");
                if fbw_mode == tc::MODE_TA as f64 || fbw_mode == tc::MODE_TARA as f64 {
                    self.alert_level = fbw_mode as u8;
                }
            }
            let inputs = tcas_inputs(io, self.alert_level, self.alt_select);
            let traffic = io.traffic();
            let update = self.tcas.update(self.tcas_acc_ms, &inputs, &traffic);
            self.tcas_acc_ms = 0.;
            self.tcas_outputs = Some(tcas_outputs(&update.vars));
            if authoritative {
                for sound in &update.sounds {
                    if now >= self.gpws_sound_until {
                        io.play_sound(sound.name);
                    }
                }
                if update.traffic_l.is_some() || update.traffic_r.is_some() {
                    self.to_js.traffic = Some((update.traffic_l.unwrap_or_default(), update.traffic_r.unwrap_or_default()));
                }
            }
        }
        if let Some(outputs) = self.tcas_outputs {
            self.settle(System::Tcas, io, &outputs, now, lines);
        }
    }

    fn run_hosts(&mut self, io: &mut dyn Io, now: f64, lines: &mut Vec<String>) {
        let inputs = fws_inputs(io);
        let (out, _) = self.fws.update(&inputs);
        let flag = |b: bool| b as u8 as f64;
        let outputs = [(FWS1_HEALTHY, flag(out.fws1_healthy)), (FWS2_HEALTHY, flag(out.fws2_healthy)), (FWS_ECP_FAILED, flag(out.ecp_failed))];
        self.settle(System::HostsMisc, io, &outputs, now, lines);
    }
}

struct HostIo<'a, 'b> {
    host: &'a mut VarsHost<'b>,
    resolved: &'a mut HashMap<(String, String), Resolved>,
}

impl Io for HostIo<'_, '_> {
    fn get(&mut self, name: &str, unit: &str) -> f64 {
        let key = (name.to_string(), unit.to_string());
        let r = *self.resolved.entry(key).or_insert_with(|| self.host.resolve(name, unit));
        self.host.read(name, r)
    }

    fn set(&mut self, name: &str, value: f64) {
        let key = (name.to_string(), "number".to_string());
        let r = *self.resolved.entry(key).or_insert_with(|| self.host.resolve(name, "number"));
        self.host.write(name, r, value);
    }

    fn traffic(&mut self) -> Vec<RawTraffic> {
        match crate::mapdata::traffic::call("GET_AIR_TRAFFIC", "[]") {
            Some(Ok(json)) => parse_traffic(&json),
            _ => Vec::new(),
        }
    }

    fn failure(&mut self, id: u64) -> bool {
        crate::failures::is_active(id)
    }

    fn play_sound(&mut self, name: &str) {
        crate::sound::play_instrument_sound(name);
    }
}

struct Running {
    shadow: Shadow,
    state: HostState,
    env: EnvRefs,
    resolved: HashMap<(String, String), Resolved>,
}

thread_local! {
    static RUNNING: std::cell::RefCell<Option<Running>> = const { std::cell::RefCell::new(None) };
}

pub fn tick(vars: &mut Vars, time: f64) {
    let mut lines = Vec::new();
    RUNNING.with(|cell| {
        let mut guard = cell.borrow_mut();
        let running = guard.get_or_insert_with(|| Running {
            shadow: Shadow::new(),
            state: HostState::default(),
            env: EnvRefs::new(vars.xplm),
            resolved: HashMap::new(),
        });
        let mut host = VarsHost::new(vars, &mut running.state, &running.env, time);
        let mut io = HostIo { host: &mut host, resolved: &mut running.resolved };
        running.shadow.frame(&mut io, time, &mut lines);
        let to_js = running.shadow.take_to_js();
        if to_js != ToJs::default() {
            if crate::js_bridge::INSTRUMENTS_RUNNING.load(std::sync::atomic::Ordering::Relaxed) {
                OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(ToJs::default).merge(to_js);
            } else {
                #[cfg(feature = "native-displays")]
                if let Some(traffic) = serde_json::from_str::<serde_json::Value>(&to_js.to_json()).ok().and_then(|v| v.get("traffic").cloned()).filter(|t| !t.is_null()) {
                    crate::native_displays::inputs::tap_generic_data("A32NX_TCAS_L_TRAFFIC", &traffic["l"].to_string());
                    crate::native_displays::inputs::tap_generic_data("A32NX_TCAS_R_TRAFFIC", &traffic["r"].to_string());
                }
            }
        }
    });
    for line in lines {
        crate::log(&line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fake {
        vars: HashMap<String, f64>,
        written: Vec<(String, f64)>,
        traffic: Vec<RawTraffic>,
        failures: BTreeSet<u64>,
        played: Vec<String>,
    }

    impl Io for Fake {
        fn get(&mut self, name: &str, _unit: &str) -> f64 {
            self.vars.get(name).copied().unwrap_or(0.)
        }

        fn set(&mut self, name: &str, value: f64) {
            self.written.push((name.to_string(), value));
            self.vars.insert(name.to_string(), value);
        }

        fn traffic(&mut self) -> Vec<RawTraffic> {
            self.traffic.clone()
        }

        fn failure(&mut self, id: u64) -> bool {
            self.failures.contains(&id)
        }

        fn play_sound(&mut self, name: &str) {
            self.played.push(name.to_string());
        }
    }

    fn shadow(raw: &str) -> Shadow {
        let mut s = Shadow::new();
        s.config_fixed = true;
        s.set_config(Config::parse(raw));
        s
    }

    fn approach_inputs() -> Fake {
        let mut f = Fake::default();
        for (k, v) in [
            ("L:A32NX_RA_1_RADIO_ALTITUDE", gpws::arinc::Word::normal(1200.).to_raw()),
            ("L:A32NX_FMGC_FLIGHT_PHASE", 5.),
            ("L:A32NX_FLAPS_HANDLE_INDEX", 4.),
            ("GEAR TOTAL PCT EXTENDED", 1.),
            ("AIRSPEED INDICATED", 140.),
            ("VERTICAL SPEED", -900.),
            ("PLANE ALTITUDE", 1200.),
        ] {
            f.vars.insert(k.to_string(), v);
        }
        f
    }

    fn run_frames(s: &mut Shadow, f: &mut Fake, frames: usize, t0: f64, lines: &mut Vec<String>) -> f64 {
        let mut t = t0;
        for _ in 0..frames {
            t += 0.05;
            s.frame(f, t, lines);
        }
        t
    }

    fn as_fbw(f: &mut Fake, s: &Shadow) {
        for (k, v) in gpws_outputs(&s.gpws) {
            f.vars.insert(k.to_string(), v);
        }
    }

    #[test]
    fn identical_outputs_report_no_differences() {
        let mut c = Comparator::new();
        c.summary(0.);
        for i in 0..200 {
            assert_eq!(c.observe(System::Gpws, "L:X", 3., 3., i as f64 * 0.05), None);
            assert_eq!(c.observe(System::Tcas, "L:Y", f64::NAN, f64::NAN, i as f64 * 0.05), None);
        }
        let lines = c.summary(11.);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines.iter().all(|l| l.contains("clean")), "{lines:?}");
    }

    #[test]
    fn a_sustained_mismatch_is_reported_once_in_the_agreed_format() {
        let mut c = Comparator::new();
        let mut reports = Vec::new();
        c.summary(0.);
        for i in 0..100 {
            reports.extend(c.observe(System::Gpws, "L:A32NX_GPWS_Warning_Active", 1., 0., i as f64 * 0.05));
        }
        assert_eq!(reports, vec!["native shadow: gpws L:A32NX_GPWS_Warning_Active native=1 fbw=0".to_string()]);
        let summary = c.summary(10.5);
        assert!(summary.iter().any(|l| l.starts_with("native shadow: summary gpws 1 variables differ") && l.contains("L:A32NX_GPWS_Warning_Active")), "{summary:?}");
    }

    #[test]
    fn a_short_lag_between_the_two_is_not_a_difference() {
        let mut c = Comparator::new();
        assert_eq!(c.observe(System::Gpws, "L:X", 1., 0., 0.), None);
        assert_eq!(c.observe(System::Gpws, "L:X", 1., 0., 0.3), None);
        assert_eq!(c.observe(System::Gpws, "L:X", 1., 1., 0.4), None);
        assert_eq!(c.observe(System::Gpws, "L:X", 1., 0., 5.), None);
    }

    #[test]
    fn gpws_in_shadow_matches_a_twin_fed_the_same_recorded_inputs() {
        let mut s = shadow("");
        let mut twin = Gpws::new();
        let mut f = approach_inputs();
        let mut lines = Vec::new();
        let mut t = 0.;
        for frame in 0..400 {
            t += 0.05;
            f.vars.insert("L:A32NX_RA_1_RADIO_ALTITUDE".into(), gpws::arinc::Word::normal(1200. - frame as f32 * 2.).to_raw());
            f.vars.insert("PLANE ALTITUDE".into(), 1200. - frame as f64 * 2.);
            if frame > 0 {
                let inputs = gpws_inputs(&mut f);
                twin.update(50., &inputs);
            }
            for (k, v) in gpws_outputs(&twin) {
                f.vars.insert(k.to_string(), v);
            }
            s.frame(&mut f, t, &mut lines);
        }
        assert!(lines.iter().all(|l| !l.starts_with("native shadow: gpws ")), "{lines:?}");
        assert!(f.written.iter().all(|(n, _)| n == MASK_VAR), "shadow wrote {:?}", f.written);
    }

    #[test]
    fn gpws_in_shadow_reports_when_fbw_disagrees() {
        let mut s = shadow("");
        let mut f = approach_inputs();
        let mut lines = Vec::new();
        let mut t = 0.;
        t = run_frames(&mut s, &mut f, 40, t, &mut lines);
        as_fbw(&mut f, &s);
        f.vars.insert(gv::OUTPUT_GPWS_WARNING.to_string(), 1.);
        run_frames(&mut s, &mut f, 40, t, &mut lines);
        assert!(lines.iter().any(|l| l == &format!("native shadow: gpws {} native=0 fbw=1", gv::OUTPUT_GPWS_WARNING)), "{lines:?}");
    }

    #[test]
    fn authoritative_writes_native_outputs_and_the_gate_mask() {
        let mut s = shadow("gpws");
        let mut f = approach_inputs();
        let mut lines = Vec::new();
        run_frames(&mut s, &mut f, 10, 0., &mut lines);
        assert!(f.written.contains(&(MASK_VAR.to_string(), 1.)));
        assert!(f.written.iter().any(|(n, _)| n == gv::OUTPUT_GPWS_WARNING));
        assert!(lines.iter().all(|l| !l.starts_with("native shadow: gpws ")));
    }

    #[test]
    fn tcas_ticks_once_a_second_and_reports_a_mode_mismatch() {
        let mut s = shadow("");
        let mut f = Fake::default();
        f.vars.insert("L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED".into(), 1.);
        f.vars.insert("L:A32NX_TRANSPONDER_SYSTEM".into(), 0.);
        f.vars.insert("TRANSPONDER STATE:1".into(), 4.);
        f.vars.insert(TCAS_MODE.into(), 2.);
        let mut lines = Vec::new();
        run_frames(&mut s, &mut f, 120, 0., &mut lines);
        assert!(s.tcas_outputs.is_some());
        let native_mode = s.tcas_outputs.unwrap()[0].1;
        f.vars.insert(TCAS_MODE.into(), native_mode + 1.);
        f.vars.insert("L:A32NX_TCAS_STATE".into(), 3.);
        run_frames(&mut s, &mut f, 200, 6., &mut lines);
        assert!(lines.iter().any(|l| l.starts_with("native shadow: tcas L:A32NX_TCAS_STATE native=")), "{lines:?}");
    }

    #[test]
    fn settings_text_selects_systems() {
        let c = Config::parse("gpws, TCAS");
        assert!(c.authoritative(System::Gpws) && c.authoritative(System::Tcas) && !c.authoritative(System::HostsMisc));
        assert_eq!(c.mask(), 3.);
        assert!(Config::parse("hosts_misc").authoritative(System::HostsMisc));
        assert_eq!(Config::parse("all").mask(), 7.);
        assert!(Config::parse("off,gpws").off);
        assert_eq!(Config::parse("").mask(), 0.);
    }

    fn sinking_approach(frame: usize) -> [(&'static str, f64); 3] {
        let ra = 2400. - frame as f64 * 6.;
        [
            ("L:A32NX_RA_1_RADIO_ALTITUDE", gpws::arinc::Word::normal(ra as f32).to_raw()),
            ("PLANE ALTITUDE", ra),
            ("VERTICAL SPEED", -6000.),
        ]
    }

    #[test]
    fn authoritative_gpws_plays_the_aurals_and_queues_the_callouts_a_twin_produces() {
        let mut s = shadow("gpws");
        let mut twin = Gpws::new();
        let mut f = approach_inputs();
        f.vars.insert("GEAR TOTAL PCT EXTENDED".into(), 0.);
        f.vars.insert("L:A32NX_FLAPS_HANDLE_INDEX".into(), 0.);
        f.vars.insert("L:A32NX_WXR_TAWS_SYS_SELECTED".into(), 1.);
        let (mut lines, mut twin_sounds, mut twin_events) = (Vec::new(), Vec::new(), Vec::new());
        let mut t = 0.;
        for frame in 0..380 {
            t += 0.05;
            for (k, v) in sinking_approach(frame) {
                f.vars.insert(k.into(), v);
            }
            if frame > 0 {
                let inputs = gpws_inputs(&mut f);
                twin.update(50., &inputs);
                twin_sounds.extend(twin.take_played_sounds().into_iter().map(str::to_string));
                twin_events.extend(twin.take_events().into_iter().map(|e| match e {
                    GpwsEvent::Enqueue(n) => ("enqueueSound", n),
                    GpwsEvent::Dequeue(n) => ("dequeueSound", n),
                }));
            }
            s.frame(&mut f, t, &mut lines);
        }
        assert!(!twin_sounds.is_empty() && !twin_events.is_empty(), "profile too gentle: {twin_sounds:?} {twin_events:?}");
        assert_eq!(f.played, twin_sounds);
        assert_eq!(s.take_to_js().bus, twin_events);
    }

    #[test]
    fn gpws_in_shadow_plays_nothing_and_queues_nothing() {
        let mut s = shadow("");
        let mut f = approach_inputs();
        let mut lines = Vec::new();
        let mut t = 0.;
        for frame in 0..380 {
            t += 0.05;
            for (k, v) in sinking_approach(frame) {
                f.vars.insert(k.into(), v);
            }
            s.frame(&mut f, t, &mut lines);
        }
        assert!(f.played.is_empty(), "{:?}", f.played);
        assert_eq!(s.take_to_js(), ToJs::default());
    }

    fn tcas_ready() -> Fake {
        let nominal = |v: f32| tcas::util::Arinc429::to_raw(v, tcas::util::Ssm::NormalOperation);
        let mut f = Fake::default();
        for (k, v) in [
            ("L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED", 1.),
            ("L:A32NX_ELEC_AC_2_BUS_IS_POWERED", 1.),
            ("TRANSPONDER STATE:1", 4.),
            ("L:A32NX_AIR_DATA_SWITCHING_KNOB", 1.),
            ("L:A32NX_ATT_HDG_SWITCHING_KNOB", 1.),
            ("L:A32NX_ADIRS_ADR_1_ALTITUDE", nominal(10000.)),
            ("L:A32NX_ADIRS_IR_1_TRUE_HEADING", nominal(90.)),
            ("L:A32NX_RA_1_RADIO_ALTITUDE", nominal(5000.)),
            ("L:A32NX_RA_2_RADIO_ALTITUDE", nominal(5000.)),
            ("PLANE ALTITUDE", 10000.),
            ("PLANE ALT ABOVE GROUND", 5000.),
            ("PLANE LATITUDE", 50.),
            ("PLANE LONGITUDE", 8.),
        ] {
            f.vars.insert(k.into(), v);
        }
        f
    }

    #[test]
    fn tcas_takes_the_mfd_alert_level_and_altitude_band_from_the_mirror() {
        for (level, alt_select) in [(1., 1.), (2., 2.), (0., 0.)] {
            let mut s = shadow("");
            let mut f = tcas_ready();
            f.vars.insert(TCAS_MIRROR_VAR.into(), 1.);
            f.vars.insert(TCAS_ALERT_LEVEL_VAR.into(), level);
            f.vars.insert(TCAS_ALT_SELECT_VAR.into(), alt_select);
            run_frames(&mut s, &mut f, 50, 0., &mut Vec::new());
            assert_eq!((s.alert_level as f64, s.alt_select as f64), (level, alt_select));
            assert_eq!(s.tcas_outputs.unwrap()[0], (TCAS_MODE, level), "fault={:?}", s.tcas_outputs.unwrap()[2]);
        }
    }

    #[test]
    fn authoritative_tcas_hands_nd_traffic_to_the_js_side() {
        let mut s = shadow("tcas");
        let mut f = tcas_ready();
        f.vars.insert(TCAS_MIRROR_VAR.into(), 1.);
        f.vars.insert(TCAS_ALERT_LEVEL_VAR.into(), 2.);
        f.vars.insert("L:A380X_EFIS_L_TRAF_BUTTON_IS_ON".into(), 1.);
        f.vars.insert("L:A380X_EFIS_R_TRAF_BUTTON_IS_ON".into(), 1.);
        let mut lines = Vec::new();
        for frame in 0..120 {
            f.traffic = vec![RawTraffic { uid: 4242., lat: 50.02, lon: 8.02 - frame as f64 * 0.000072, alt: 3048. + 150., heading: 270. }];
            s.frame(&mut f, 0.05 * (frame + 1) as f64, &mut lines);
        }
        let to_js = s.take_to_js();
        let (l, r) = to_js.traffic.clone().expect("no traffic handed over");
        assert_eq!(l.len(), 1, "{l:?}");
        assert_eq!(r.len(), 1, "{r:?}");
        let json: serde_json::Value = serde_json::from_str(&to_js.to_json()).unwrap();
        assert_eq!(json["traffic"]["l"][0]["ID"], "4242");
        assert!(json["traffic"]["l"][0]["relativeAlt"].is_number());
        assert!(f.written.contains(&(TCAS_MODE.to_string(), 2.)));
    }

    fn fws_ready() -> Fake {
        let mut f = Fake::default();
        for k in ["L:A32NX_CPIOM_C1_AVAIL", "L:A32NX_CPIOM_C2_AVAIL", "L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED", "L:A32NX_AFDX_3_3_REACHABLE", FWS_STARTUP_VAR] {
            f.vars.insert(k.into(), 1.);
        }
        f
    }

    #[test]
    fn fws_health_follows_failures_reset_panel_and_cpiom() {
        let mut s = shadow("hosts_misc");
        let mut f = fws_ready();
        let mut t = run_frames(&mut s, &mut f, 5, 0., &mut Vec::new());
        let state = |f: &Fake| [FWS1_HEALTHY, FWS2_HEALTHY, FWS_ECP_FAILED].map(|v| f.vars.get(v).copied().unwrap_or(-1.));
        assert_eq!(state(&f), [1., 1., 0.]);
        f.failures.insert(FWS1_FAILURE);
        t = run_frames(&mut s, &mut f, 2, t, &mut Vec::new());
        assert_eq!(state(&f), [0., 1., 0.]);
        f.failures.clear();
        f.vars.insert("L:A32NX_RESET_PANEL_FWS2".into(), 1.);
        t = run_frames(&mut s, &mut f, 2, t, &mut Vec::new());
        assert_eq!(state(&f), [1., 0., 0.]);
        f.vars.insert("L:A32NX_RESET_PANEL_FWS2".into(), 0.);
        f.vars.insert("L:A32NX_CPIOM_C1_AVAIL".into(), 0.);
        f.failures.insert(FWS_ECP_FAILURE);
        run_frames(&mut s, &mut f, 2, t, &mut Vec::new());
        assert_eq!(state(&f), [0., 1., 1.]);
    }

    #[test]
    fn fws_health_in_shadow_reports_when_fbw_disagrees() {
        let mut s = shadow("");
        let mut f = fws_ready();
        f.vars.insert(FWS1_HEALTHY.into(), 1.);
        f.vars.insert(FWS2_HEALTHY.into(), 0.);
        let mut lines = Vec::new();
        run_frames(&mut s, &mut f, 60, 0., &mut lines);
        assert!(lines.iter().any(|l| l == &format!("native shadow: hosts_misc {FWS2_HEALTHY} native=1 fbw=0")), "{lines:?}");
        assert!(!lines.iter().any(|l| l.contains(&format!("hosts_misc {FWS1_HEALTHY} "))), "{lines:?}");
    }

    #[test]
    fn the_take_call_drains_the_outbox_once() {
        assert_eq!(coherent_call("SOMETHING_ELSE"), None);
        let mut first = ToJs::default();
        first.bus.push(("enqueueSound", "alt_50"));
        OUTBOX.lock().unwrap().get_or_insert_with(ToJs::default).merge(first);
        let json: serde_json::Value = serde_json::from_str(&coherent_call(TAKE_CALL).unwrap()).unwrap();
        assert_eq!(json["bus"], serde_json::json!([["enqueueSound", "alt_50"]]));
        assert!(json["traffic"].is_null());
        let again: serde_json::Value = serde_json::from_str(&coherent_call(TAKE_CALL).unwrap()).unwrap();
        assert_eq!(again["bus"], serde_json::json!([]));
        let mut flood = ToJs::default();
        for _ in 0..(MAX_QUEUED_BUS_EVENTS + 10) {
            flood.merge(ToJs { bus: vec![("enqueueSound", "alt_5")], traffic: None });
        }
        assert_eq!(flood.bus.len(), MAX_QUEUED_BUS_EVENTS);
    }

    #[test]
    fn air_traffic_json_becomes_raw_traffic() {
        let json = crate::mapdata::traffic::to_json(&[crate::mapdata::traffic::Target {
            mode_s_id: 77,
            flight_id: "ABC".into(),
            latitude: 1.5,
            longitude: 2.5,
            elevation_m: 3000.,
            heading: 90.,
            on_ground: false,
        }]);
        let t = parse_traffic(&json);
        assert_eq!(t.len(), 1);
        assert_eq!((t[0].uid, t[0].lat, t[0].lon, t[0].alt, t[0].heading), (77., 1.5, 2.5, 3000., 90.));
        assert!(parse_traffic("nonsense").is_empty());
    }
}

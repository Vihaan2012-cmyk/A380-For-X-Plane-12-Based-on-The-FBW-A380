//! The rest of FlyByWire's SystemsHost and ExtrasHost modules run natively:
//! in shadow (compared with what the JS wrote, nothing reaches the sim) or,
//! when named in `nativeSystems`, authoritative with the JS module stood down
//! through `L:A380X_NATIVE_SYSHOST_MASK` (`native_systems_host_patches.rs`).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;

use hosts_misc::baro_unit::{self, BaroSelection};
use hosts_misc::key_interceptor::{self as ki, KeyInterceptor, RmpEnv, RmpState};
use hosts_misc::light_sync::{LightSync, LightSyncConfig, LightSyncInputs};
use hosts_misc::pilot_seat::{PilotSeat, PilotSeatConfig, PilotSeatManager, A380X_FLIGHT_DECK};
use hosts_misc::pushbutton_check::PushbuttonCheck;
use hosts_misc::Command;
use oans_native::arinc::Arinc;
use oans_native::btv::{BtvInputs, NavDb, Oans, Runway};
use oans_native::geo::Coordinates;
use serde_json::{json, Value};

use crate::native_shadow::{Comparator, Compared};
use crate::native_systems_host_patches::*;

const SETTINGS_PERIOD_S: f64 = 10.;
const MAX_STEP_MS: f64 = 250.;
const FWS_COMPARE_PERIOD_S: f64 = 0.2;
const ONCE_COMPARE_WINDOW_S: f64 = 10.;
const LIGHT_STEADY_MS: f64 = 5000.;
const LIGHT_GIVE_UP_MS: f64 = 180_000.;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Module {
    Vhf,
    Xpdr,
    Amu,
    Btv,
    EfisTaws,
    Fws,
    LightSync,
    KeyInterceptor,
    PilotSeat,
    PushbuttonCheck,
    BaroUnit,
    Ready,
    TerrainFeed,
    OitHealth,
}

impl Module {
    pub const ALL: [Module; 14] = [
        Module::Vhf,
        Module::Xpdr,
        Module::Amu,
        Module::Btv,
        Module::EfisTaws,
        Module::Fws,
        Module::LightSync,
        Module::KeyInterceptor,
        Module::PilotSeat,
        Module::PushbuttonCheck,
        Module::BaroUnit,
        Module::Ready,
        Module::TerrainFeed,
        Module::OitHealth,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Module::Vhf => "vhf",
            Module::Xpdr => "xpdr",
            Module::Amu => "amu",
            Module::Btv => "btv",
            Module::EfisTaws => "efis_taws",
            Module::Fws => "fws",
            Module::LightSync => "light_sync",
            Module::KeyInterceptor => "key_interceptor",
            Module::PilotSeat => "pilot_seat",
            Module::PushbuttonCheck => "pushbutton_check",
            Module::BaroUnit => "baro_unit",
            Module::Ready => "ready",
            Module::TerrainFeed => "terrain_feed",
            Module::OitHealth => "oit_health",
        }
    }

    pub fn bit(self) -> u32 {
        match self {
            Module::Vhf => BIT_VHF,
            Module::Xpdr => BIT_XPDR,
            Module::Amu => BIT_AMU,
            Module::Btv => BIT_BTV,
            Module::EfisTaws => BIT_EFIS_TAWS,
            Module::Fws => BIT_FWS,
            Module::LightSync => BIT_LIGHT_SYNC,
            Module::KeyInterceptor => BIT_KEY_INTERCEPTOR,
            Module::PilotSeat => BIT_PILOT_SEAT,
            Module::PushbuttonCheck => BIT_PUSHBUTTON_CHECK,
            Module::BaroUnit => BIT_BARO_UNIT,
            Module::Ready => BIT_READY,
            Module::TerrainFeed => BIT_TERRAIN_FEED,
            Module::OitHealth => BIT_OIT_HEALTH,
        }
    }

    /// `all` leaves out the modules whose JS still owns outputs the native one lacks.
    fn in_all(self) -> bool {
        !matches!(self, Module::Fws | Module::Btv | Module::BaroUnit | Module::PilotSeat | Module::TerrainFeed)
    }
}

impl Compared for Module {
    fn label(self) -> &'static str {
        self.name()
    }

    fn debounce(self) -> f64 {
        match self {
            Module::Fws | Module::Btv => 1.5,
            _ => 1.,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub off: bool,
    pub authoritative: BTreeSet<Module>,
}

impl Config {
    pub fn parse(raw: &str) -> Config {
        let mut c = Config::default();
        for token in raw.split(|ch: char| ch == ',' || ch == ';' || ch.is_whitespace()).filter(|t| !t.is_empty()) {
            match token.to_ascii_lowercase().as_str() {
                "off" | "none" => c.off = true,
                "all" => c.authoritative.extend(Module::ALL.into_iter().filter(|m| m.in_all())),
                other => c.authoritative.extend(Module::ALL.into_iter().find(|m| m.name() == other)),
            }
        }
        c
    }

    pub fn authoritative(&self, m: Module) -> bool {
        !self.off && self.authoritative.contains(&m)
    }

    pub fn mask(&self) -> f64 {
        Module::ALL.iter().filter(|m| self.authoritative(**m)).map(|m| m.bit()).sum::<u32>() as f64
    }
}

pub trait Io {
    fn get(&mut self, name: &str, unit: &str) -> f64;
    fn set(&mut self, name: &str, value: f64);
    fn get_string(&mut self, name: &str) -> String;
    fn set_string(&mut self, name: &str, value: &str);
    fn failure(&mut self, id: u64) -> bool;
    fn key(&mut self, name: &str, args: &[f64]);
    fn stored(&mut self, key: &str) -> Option<String>;
    fn nearest_airport(&mut self) -> Option<String>;
    fn call(&mut self, _name: &str, _args: &str) {}
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Step {
    pub vars: Vec<(String, f64)>,
    pub compare: Vec<(String, f64)>,
    pub strings: Vec<(String, String)>,
    pub keys: Vec<(String, Vec<f64>)>,
    pub bus: Vec<(String, Value)>,
    pub notifications: Vec<(String, String)>,
    pub h_events: Vec<String>,
    pub delayed: Vec<(f64, String, f64)>,
    pub calls: Vec<(String, String)>,
}

impl Step {
    fn both(&mut self, name: impl Into<String>, value: f64) {
        let name = name.into();
        self.vars.push((name.clone(), value));
        self.compare.push((name, value));
    }

    fn command(&mut self, c: Command, delayed_h: &mut Vec<String>) {
        let lvar = |n: String| if n.starts_with("L:") { n } else { format!("L:{n}") };
        match c {
            Command::SetLvar { name, value } => self.vars.push((lvar(name), value)),
            Command::KeyEvent { name, value } => self.keys.push((name, vec![value])),
            Command::TriggerKey { key, args, .. } => self.keys.push((key, args)),
            Command::HEvent { name } => self.h_events.push(name),
            Command::HEventNextFrame { name } => delayed_h.push(name),
            Command::SetLvarAfter { name, value, delay_ms } => self.delayed.push((delay_ms as f64 / 1000., lvar(name), value)),
        }
    }
}

fn unit_of(name: &str) -> &'static str {
    if name.starts_with("TRANSPONDER STATE") {
        "enum"
    } else if name.starts_with("COM RECEIVE") || name.starts_with("COM TRANSMIT") {
        "bool"
    } else {
        "number"
    }
}

fn on(io: &mut dyn Io, name: &str) -> bool {
    io.get(name, "number") != 0.
}

#[derive(Default)]
pub struct Outbox {
    pub bus: Vec<(String, Value)>,
    pub notifications: Vec<(String, String)>,
    pub h_events: Vec<String>,
}

static SYSTEMS_OUT: Mutex<Vec<(String, Value)>> = Mutex::new(Vec::new());
static EXTRAS_OUT: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
static H_EVENTS: Mutex<Vec<String>> = Mutex::new(Vec::new());
static KEYS_IN: Mutex<Vec<(String, Vec<f64>)>> = Mutex::new(Vec::new());
static FWS_SOUNDS_IN: Mutex<Vec<String>> = Mutex::new(Vec::new());
const MAX_QUEUED: usize = 256;

fn push_capped<T>(queue: &Mutex<Vec<T>>, items: impl IntoIterator<Item = T>) {
    let mut q = queue.lock().unwrap_or_else(|e| e.into_inner());
    q.extend(items);
    let excess = q.len().saturating_sub(MAX_QUEUED);
    q.drain(..excess);
}

fn take<T>(queue: &Mutex<Vec<T>>) -> Vec<T> {
    std::mem::take(&mut *queue.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Answers the three Coherent calls the patched hosts make; any thread.
pub fn coherent_call(name: &str, args_json: &str) -> Option<String> {
    match name {
        TAKE_SYSTEMS_CALL => Some(json!({ "bus": take(&SYSTEMS_OUT).into_iter().map(|(t, d)| json!([t, d])).collect::<Vec<_>>() }).to_string()),
        TAKE_EXTRAS_CALL => Some(
            json!({ "notifications": take(&EXTRAS_OUT).into_iter().map(|(t, m)| json!({ "title": t, "message": m })).collect::<Vec<_>>() }).to_string(),
        ),
        KEY_CALL => {
            let args: Vec<Value> = serde_json::from_str(args_json).unwrap_or_default();
            if let Some(key) = args.first().and_then(Value::as_str) {
                let values = args[1..].iter().map_while(Value::as_f64).collect();
                push_capped(&KEYS_IN, [(key.to_string(), values)]);
            }
            Some("true".to_string())
        }
        _ => None,
    }
}

/// GPWS `enqueueSound` keys for the native FWS's sound queue.
pub fn enqueue_fws_sound(key: &str) {
    push_capped(&FWS_SOUNDS_IN, [key.to_string()]);
}

/// H events the native modules raised, for lib.rs to add to the frame's list.
pub fn take_h_events() -> Vec<String> {
    take(&H_EVENTS)
}

static BTV_NAV: Mutex<Option<&'static (dyn NavDb + Sync)>> = Mutex::new(None);

/// The airport/runway source native BTV runs on (until set: none, so no runway).
pub fn set_btv_navdb(nav: Box<dyn NavDb + Send + Sync>) {
    *BTV_NAV.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::leak(nav));
}

struct NoNav;

impl NavDb for NoNav {
    fn airport_location(&self, _icao: &str) -> Option<Coordinates> {
        None
    }

    fn runways(&self, _icao: &str) -> Vec<Runway> {
        Vec::new()
    }

    fn nearby_airports(&self, _ppos: Coordinates, _radius_nm: f64, _max_results: usize) -> Vec<String> {
        Vec::new()
    }
}

static NO_NAV: NoNav = NoNav;

struct Ctx {
    now: f64,
    dt_ms: f64,
    authoritative: bool,
}

struct Vhf {
    units: [rmp::VhfRadioUnit; 3],
    vars: rmp::Vars,
}

const VHF_FAILURES: [(u64, &str); 3] = [(23005, "Vhf1"), (23006, "Vhf2"), (23007, "Vhf3")];
const VHF_POWER: [&str; 3] = ["L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED", "L:A32NX_ELEC_DC_2_BUS_IS_POWERED", "L:A32NX_ELEC_DC_1_BUS_IS_POWERED"];

impl Vhf {
    fn new() -> Self {
        Vhf { units: [rmp::VhfRadioUnit::new(1), rmp::VhfRadioUnit::new(2), rmp::VhfRadioUnit::new(3)], vars: rmp::Vars::new() }
    }

    fn step(&mut self, io: &mut dyn Io) -> Step {
        let mut s = Step::default();
        for i in 1..=3usize {
            let (a, b) = if i == 2 { (2, 1) } else { (1, 2) };
            let circuit = [36, 38, 40][i - 1];
            for (name, unit) in [
                (format!("L:FBW_VHF{i}_BUS_A"), "number"),
                (format!("L:FBW_RMP{a}_PRIMARY_VHF{i}_FREQUENCY"), "number"),
                (format!("L:FBW_RMP{b}_BACKUP_VHF{i}_FREQUENCY"), "number"),
                (format!("CIRCUIT SWITCH ON:{circuit}"), "bool"),
                (format!("COM ACTIVE FREQUENCY:{i}"), "frequency bcd32"),
            ] {
                let v = io.get(&name, unit);
                self.vars.set_quiet(&name, v);
            }
            let (id, key) = VHF_FAILURES[i - 1];
            let failed = io.failure(id);
            self.vars.set_failure(key, failed);
            let powered = on(io, VHF_POWER[i - 1]);
            self.units[i - 1].update(powered, &mut self.vars);
            let out = format!("L:FBW_VHF{i}_FREQUENCY");
            s.both(out.clone(), self.vars.get(&out));
        }
        self.vars.take_writes();
        s.keys = self.vars.take_keys().into_iter().map(|k| (k.name, k.args)).collect();
        s
    }
}

const RMP_SCREENS: [&str; 3] = ["SCREEN_DU_RMP_1", "SCREEN_DU_RMP_2", "SCREEN_DU_RMP_3"];
const RMP_FAILURES: [u64; 3] = [23002, 23003, 23004];
const RMP_RX_SWITCHES: [&str; 11] = [
    "VHF_VOL_RX_SWITCH_1",
    "VHF_VOL_RX_SWITCH_2",
    "VHF_VOL_RX_SWITCH_3",
    "HF_VOL_RX_SWITCH_1",
    "HF_VOL_RX_SWITCH_2",
    "TEL_VOL_RX_SWITCH_1",
    "TEL_VOL_RX_SWITCH_2",
    "INT_VOL_RX_SWITCH",
    "CAB_VOL_RX_SWITCH",
    "PA_VOL_RX_SWITCH",
    "NAV_VOL_RX_SWITCH",
];
const RMP_TX: [(&str, usize); 7] = [("VHF", 1), ("VHF", 2), ("VHF", 3), ("HF", 1), ("HF", 2), ("TEL", 1), ("TEL", 2)];

#[cfg(feature = "native-displays")]
fn rmp_screens_native() -> [bool; 3] {
    let selection = crate::native_displays::screen::selection();
    RMP_SCREENS.map(|s| selection.target_for(s).is_some())
}

#[cfg(not(feature = "native-displays"))]
fn rmp_screens_native() -> [bool; 3] {
    [false; 3]
}

/// FlyByWire's RMP instrument (fbw-a380x `RMP/`) for the RMP screens the
/// native displays draw: its page no longer loads for them
/// (`xphfbw_bridge_views::PAGE_STOPS_WHEN_NATIVE`), so the panels run here.
struct RmpPanels {
    panels: Vec<rmp::Rmp>,
    vars: rmp::Vars,
    started: bool,
    views: [String; 3],
}

impl RmpPanels {
    fn new(native: [bool; 3]) -> Self {
        RmpPanels { panels: (1..=3).filter(|n| native[n - 1]).map(rmp::Rmp::new).collect(), vars: rmp::Vars::new(), started: false, views: Default::default() }
    }

    fn inputs() -> Vec<(String, &'static str)> {
        let mut v: Vec<(String, &'static str)> = ["L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED", "L:A32NX_ELEC_DC_1_BUS_IS_POWERED", "L:FBW_RMP3_BUS_A"]
            .iter()
            .map(|n| (n.to_string(), "number"))
            .collect();
        v.push(("TRANSPONDER CODE:1".to_string(), "number"));
        v.push(("TRANSPONDER STATE:1".to_string(), "enum"));
        for i in 1..=3 {
            v.push((format!("L:FBW_VHF{i}_FREQUENCY"), "number"));
            v.push((format!("L:FBW_RMP3_BACKUP_VHF{i}_FREQUENCY"), "number"));
            v.push((format!("COM ACTIVE FREQUENCY:{i}"), "frequency bcd32"));
            v.push((format!("COM STANDBY FREQUENCY:{i}"), "frequency bcd32"));
            for kind in ["FREQUENCY", "MODE"] {
                for which in ["ACTIVE", "STANDBY"] {
                    v.push((format!("L:FBW_RMP_{kind}_{which}_{i}"), "number"));
                }
            }
        }
        for r in 1..=3 {
            v.push((format!("L:A380X_RMP_{r}_BRIGHTNESS_KNOB"), "number"));
            for s in RMP_RX_SWITCHES {
                v.push((format!("L:A380X_RMP_{r}_{s}"), "number"));
            }
            for (kind, i) in RMP_TX {
                v.push((format!("L:A380X_RMP_{r}_{kind}_TX_{i}"), "number"));
            }
        }
        v
    }

    fn view_json(view: &rmp::View) -> Value {
        let rows: Vec<Value> = view
            .rows
            .iter()
            .map(|r| {
                json!({
                    "active_text": r.active_text,
                    "active_amber": r.active_amber,
                    "standby_digits": r.standby_digits,
                    "standby_pilot_entered": r.standby_pilot_entered,
                    "standby_amber": r.standby_amber,
                    "standby_mode_title": r.standby_mode_title,
                    "standby_entry_in_progress": r.standby_entry_in_progress,
                    "loudspeaker_hidden": r.loudspeaker_hidden,
                    "no_reception_cross": r.no_reception_cross,
                    "transmitting": r.transmitting,
                    "selected": r.selected,
                })
            })
            .collect();
        json!({
            "screen_on": view.screen_on,
            "page": if view.page == rmp::Page::Transponder { "Transponder" } else { "Vhf" },
            "rows": rows,
            "xpdr": {
                "auto": view.xpdr.auto,
                "code": view.xpdr.code,
                "entry_in_progress": view.xpdr.entry_in_progress,
                "entry_invalid": view.xpdr.entry_invalid,
                "ident": view.xpdr.ident,
            },
            "message": view.message,
        })
    }

    fn step(&mut self, io: &mut dyn Io, dt_ms: f64, h_events: &[String]) -> Vec<(usize, Value)> {
        if self.panels.is_empty() {
            return Vec::new();
        }
        for (name, unit) in Self::inputs() {
            let v = io.get(&name, unit);
            self.vars.set_quiet(&name, v);
        }
        for (i, id) in RMP_FAILURES.iter().enumerate() {
            let failed = io.failure(*id);
            self.vars.set_failure(&format!("RadioManagementPanel{}", i + 1), failed);
        }
        if !self.started {
            self.started = true;
            for p in self.panels.iter_mut() {
                p.init(&mut self.vars);
                p.flight_start(&mut self.vars);
            }
        }
        for h in h_events {
            for p in self.panels.iter_mut() {
                p.hevent(h, &mut self.vars);
            }
        }
        for p in self.panels.iter_mut() {
            p.update(dt_ms, &mut self.vars);
        }
        for w in self.vars.take_writes() {
            io.set(&w.name, w.value);
        }
        for k in self.vars.take_keys() {
            io.key(&k.name, &k.args);
        }
        let mut changed = Vec::new();
        for p in &self.panels {
            let n = p.index();
            let view = Self::view_json(&p.view());
            let text = view.to_string();
            if self.views[n - 1] != text {
                self.views[n - 1] = text;
                changed.push((n, view));
            }
        }
        changed
    }
}

struct Xpdr {
    unit: rmp::TransponderUnit,
    vars: rmp::Vars,
    started: bool,
}

impl Xpdr {
    fn step(&mut self, io: &mut dyn Io) -> Step {
        let mut s = Step::default();
        for (name, unit) in [("CIRCUIT SWITCH ON:41", "bool"), ("L:A32NX_LGCIU_1_LEFT_GEAR_COMPRESSED", "number"), ("L:A32NX_COLD_AND_DARK_SPAWN", "number")] {
            let v = io.get(name, unit);
            self.vars.set_quiet(name, v);
        }
        let failed = io.failure(34003);
        self.vars.set_failure("Transponder1", failed);
        if !self.started {
            self.started = true;
            self.unit.flight_start(&mut self.vars);
            if self.unit.auto {
                s.bus.push(("mfd_xpdr_set_auto".to_string(), json!(true)));
            }
        }
        if on(io, XPDR_MIRROR_VAR) {
            self.unit.auto = on(io, XPDR_AUTO_VAR);
            self.unit.alt_reporting = on(io, XPDR_ALT_RPTG_VAR);
        }
        let powered = on(io, "L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED") || on(io, "L:A32NX_ELEC_AC_2_BUS_IS_POWERED");
        self.unit.update(powered, &mut self.vars);
        s.both("TRANSPONDER STATE:1", self.vars.get("TRANSPONDER STATE:1"));
        self.vars.take_writes();
        s.keys = self.vars.take_keys().into_iter().map(|k| (k.name, k.args)).collect();
        s
    }
}

#[derive(Clone, Copy, PartialEq)]
struct ComOut {
    receive: bool,
    transmit: bool,
    volume: f64,
}

struct Amu {
    amus: [rmp::Amu; 2],
    vars: rmp::Vars,
    nav: Option<[(bool, f64); 6]>,
    com: Option<[ComOut; 3]>,
}

const AMU_SUFFIXES: [&str; 14] = [
    "STATE", "VHF_TX_1", "VHF_TX_2", "VHF_TX_3", "VHF_VOL_RX_1", "VHF_VOL_RX_2", "VHF_VOL_RX_3", "VHF_VOL_1", "VHF_VOL_2", "VHF_VOL_3", "NAV_SEL", "NAV_VOL_RX",
    "NAV_FILTER", "NAV_VOL",
];

fn nav_keys(navaid: usize, ident: Option<bool>, volume: Option<f64>, amu: usize, marker_on: bool, keys: &mut Vec<(String, Vec<f64>)>) {
    let b = |v: bool| v as u8 as f64;
    let mut k = |n: &str, v: f64| keys.push((n.to_string(), vec![v]));
    match navaid {
        0 | 1 | 3 | 4 => {
            let (vol, id) = match navaid {
                0 => ("ADF_VOLUME_SET", "RADIO_ADF_IDENT_SET"),
                1 => ("ADF2_VOLUME_SET", "RADIO_ADF2_IDENT_SET"),
                3 => ("NAV1_VOLUME_SET_EX1", "RADIO_VOR1_IDENT_SET"),
                _ => ("NAV2_VOLUME_SET_EX1", "RADIO_VOR2_IDENT_SET"),
            };
            if let Some(v) = volume {
                k(vol, v);
            }
            if let Some(o) = ident {
                k(id, b(o));
            }
        }
        2 => {
            if let Some(v) = volume {
                k("NAV3_VOLUME_SET_EX1", if amu == 2 { 0. } else { v });
                k("NAV4_VOLUME_SET_EX1", if amu == 2 { v } else { 0. });
            }
            if let Some(o) = ident {
                k("RADIO_VOR3_IDENT_SET", if amu == 2 { 0. } else { b(o) });
                k("RADIO_VOR4_IDENT_SET", if amu == 2 { b(o) } else { 0. });
            }
        }
        _ => {
            if let Some(o) = ident {
                if o != marker_on {
                    keys.push(("MARKER_SOUND_TOGGLE".to_string(), Vec::new()));
                }
            }
        }
    }
}

impl Amu {
    fn new() -> Self {
        Amu { amus: [rmp::Amu::new(1), rmp::Amu::new(2)], vars: rmp::Vars::new(), nav: None, com: None }
    }

    fn step(&mut self, io: &mut dyn Io) -> Step {
        let mut s = Step::default();
        let power = io.get("L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED", "number");
        self.vars.set_quiet("L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED", power);
        for r in 1..=3 {
            for suffix in AMU_SUFFIXES {
                let name = format!("L:A380X_RMP_{r}_{suffix}");
                let v = io.get(&name, "number");
                self.vars.set_quiet(&name, v);
            }
        }
        for (id, key) in [(23000, "AudioManagementUnit1"), (23001, "AudioManagementUnit2")] {
            let failed = io.failure(id);
            self.vars.set_failure(key, failed);
        }
        for a in self.amus.iter_mut() {
            a.update(&self.vars);
        }
        let seat_right = io.get("L:FBW_PILOT_SEAT", "number") == 1.;
        let t = rmp::sim_audio_target(&self.amus[0], &self.amus[1], seat_right, &self.vars);
        let marker_on = on(io, "MARKER SOUND");
        let nav: [(bool, f64); 6] = std::array::from_fn(|n| {
            if t.nav_selected == Some(n as i32) {
                (t.nav_ident_on, t.nav_volume)
            } else {
                (false, 0.)
            }
        });
        for (n, now) in nav.iter().enumerate() {
            let before = self.nav.map(|p| p[n]);
            let ident = (before.map(|b| b.0) != Some(now.0)).then_some(now.0);
            let volume = (before.map(|b| b.1) != Some(now.1)).then_some(now.1);
            nav_keys(n, ident, volume, t.active_amu, marker_on, &mut s.keys);
        }
        self.nav = Some(nav);
        let com: [ComOut; 3] = std::array::from_fn(|i| ComOut { receive: t.com[i].receive, transmit: t.com[i].transmit, volume: t.com[i].volume });
        for (i, now) in com.iter().enumerate() {
            let n = i + 1;
            let before = self.com.map(|p| p[i]);
            if before.map(|b| b.receive) != Some(now.receive) {
                s.keys.push((format!("COM{n}_RECEIVE_SELECT"), vec![now.receive as u8 as f64]));
            }
            if before.map(|b| b.transmit) != Some(now.transmit) {
                let (own, other) = if t.active_amu == 2 { ("COPILOT_TRANSMITTER_SET", "PILOT_TRANSMITTER_SET") } else { ("PILOT_TRANSMITTER_SET", "COPILOT_TRANSMITTER_SET") };
                s.keys.push((own.to_string(), vec![if now.transmit { i as f64 } else { 4. }]));
                s.keys.push((other.to_string(), vec![4.]));
            }
            if before.map(|b| b.volume) != Some(now.volume) {
                s.keys.push((format!("COM{n}_VOLUME_SET"), vec![now.volume]));
            }
            s.compare.push((format!("COM RECEIVE:{n}"), now.receive as u8 as f64));
            s.compare.push((format!("COM TRANSMIT:{n}"), now.transmit as u8 as f64));
        }
        self.com = Some(com);
        s
    }
}

struct Btv {
    oans: Option<Oans<'static>>,
    with_nav: bool,
}

fn arincs<const N: usize>(io: &mut dyn Io, pattern: &str) -> [Arinc; N] {
    std::array::from_fn(|i| Arinc::from_raw(io.get(&pattern.replace('#', &(i + 1).to_string()), "number")))
}

impl Btv {
    fn step(&mut self, io: &mut dyn Io, ctx: &Ctx) -> Step {
        let nav = *BTV_NAV.lock().unwrap_or_else(|e| e.into_inner());
        if self.oans.is_none() || (nav.is_some() && !self.with_nav) {
            self.with_nav = nav.is_some();
            let nav: &'static dyn NavDb = match nav {
                Some(n) => n,
                None => &NO_NAV,
            };
            self.oans = Some(Oans::new(nav));
        }
        let oans = self.oans.as_mut().expect("created above");
        oans.inputs = BtvInputs {
            radio_altitude: arincs(io, "L:A32NX_RA_#_RADIO_ALTITUDE"),
            vertical_speed: arincs(io, "L:A32NX_ADIRS_IR_#_VERTICAL_SPEED"),
            latitude: arincs(io, "L:A32NX_ADIRS_IR_#_LATITUDE"),
            longitude: arincs(io, "L:A32NX_ADIRS_IR_#_LONGITUDE"),
            true_heading: arincs(io, "L:A32NX_ADIRS_IR_#_TRUE_HEADING"),
            lgciu_discrete_word_2: arincs(io, "L:A32NX_LGCIU_#_DISCRETE_WORD_2"),
            fws_flight_phase: io.get("L:A32NX_FWC_FLIGHT_PHASE", "number"),
            auto_brake_mode: io.get("L:A32NX_AUTOBRAKES_ARMED_MODE", "number"),
            auto_brake_active: on(io, "L:A32NX_AUTOBRAKES_ACTIVE"),
        };
        oans.on_update(ctx.dt_ms);
        let mut s = Step::default();
        for (k, v) in &oans.out.vars {
            s.both(k.clone(), *v);
        }
        s.bus = std::mem::take(&mut oans.out.log);
        s
    }
}

const TAWS_FAILURES: [u64; 4] = [34005, 34006, 34007, 34008];

/// FlyByWire's AircraftNetworkServerUnit (NSS AVNCS 1/2, FLT OPS 1) and OisLaptop (1/2) health,
/// which the OITs read to show their pages or "Not available.".
fn oit_health(io: &mut dyn Io) -> Step {
    let mut s = Step::default();
    let master_off = on(io, "L:A32NX_NSS_MASTER_OFF");
    let nss_reset = on(io, "L:A32NX_RESET_PANEL_NSS_AVNCS");
    let ops_reset = on(io, "L:A32NX_RESET_PANEL_NSS_FLT_OPS");
    let nss1 = on(io, "L:A32NX_ELEC_AC_2_BUS_IS_POWERED") || on(io, "L:A32NX_ELEC_DC_HOT_2_BUS_IS_POWERED");
    let nss2 = on(io, "L:A32NX_ELEC_AC_1_BUS_IS_POWERED") || on(io, "L:A32NX_ELEC_AC_EMER_BUS_IS_POWERED") || on(io, "L:A32NX_ELEC_DC_HOT_1_BUS_IS_POWERED");
    let ops = on(io, "L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED") || on(io, "L:A32NX_ELEC_DC_HOT_1_BUS_IS_POWERED");
    let laptop1 = on(io, "L:A32NX_ELEC_DC_1_BUS_IS_POWERED") && on(io, "L:A380X_SWITCH_LAPTOP_POWER_LEFT");
    let laptop2 = (on(io, "L:A32NX_ELEC_DC_2_BUS_IS_POWERED") || on(io, "L:A32NX_ELEC_DC_HOT_2_BUS_IS_POWERED")) && on(io, "L:A380X_SWITCH_LAPTOP_POWER_RIGHT");
    let healthy = |failure: u64, powered: bool, reset: bool, io: &mut dyn Io| (!io.failure(failure) && powered && !reset && !master_off) as u8 as f64;
    let v = healthy(46001, nss1, nss_reset, io);
    s.both("L:A32NX_NSS_ANSU_1_IS_HEALTHY", v);
    let v = healthy(46002, nss2, nss_reset, io);
    s.both("L:A32NX_NSS_ANSU_2_IS_HEALTHY", v);
    let v = healthy(46003, ops, ops_reset, io);
    s.both("L:A32NX_FLTOPS_ANSU_1_IS_HEALTHY", v);
    let v = healthy(46004, laptop1, false, io);
    s.both("L:A32NX_FLTOPS_LAPTOP_1_IS_HEALTHY", v);
    let v = healthy(46005, laptop2, false, io);
    s.both("L:A32NX_FLTOPS_LAPTOP_2_IS_HEALTHY", v);
    s.both("L:A380X_OIT_CAPT_FAILED", io.failure(46006) as u8 as f64);
    s.both("L:A380X_OIT_FO_FAILED", io.failure(46007) as u8 as f64);
    s
}

fn efis_taws(io: &mut dyn Io) -> Step {
    let f: [bool; 4] = std::array::from_fn(|i| io.failure(TAWS_FAILURES[i]));
    let reset = [on(io, "L:A32NX_RESET_PANEL_AESU1"), on(io, "L:A32NX_RESET_PANEL_AESU2")];
    let unpowered = [!on(io, "L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED"), !on(io, "L:A32NX_ELEC_AC_4_BUS_IS_POWERED")];
    let words: [Arinc; 3] = arincs(io, "L:A32NX_ADIRS_IR_#_MAINT_WORD");
    let extreme = words.iter().find(|w| w.is_normal()).is_some_and(|w| w.bit_value_or(15, false));
    let mut s = Step::default();
    for lane in 0..2 {
        let n = lane + 1;
        s.both(format!("L:A32NX_TERR_{n}_FAILED"), (f[lane] || reset[lane] || unpowered[lane] || extreme) as u8 as f64);
        s.both(format!("L:A32NX_GPWS_{n}_FAILED"), (f[lane + 2] || reset[lane] || unpowered[lane]) as u8 as f64);
        s.both(format!("L:A32NX_WXR_{n}_FAILED"), (reset[lane] || unpowered[lane]) as u8 as f64);
    }
    s
}

struct FwsVars<'a, 'b>(RefCell<&'a mut (dyn Io + 'b)>);

/// The ECP pushbuttons the FWS reads, with the names the converted cockpit
/// writes for three of them (`ecam_patches::ecl`).
const ECP_BUTTON_ALIASES: [(&str, &str); 4] = [
    ("L:A32NX_BTN_TOCONFIG", "L:A32NX_BTN_TOCONF"),
    ("L:A32NX_BTN_CLR", "L:A32NX_BTN_CLR_LH"),
    ("L:A32NX_BTN_CLR2", "L:A32NX_BTN_CLR_RH"),
    ("L:A32NX_BTN_RCL", "L:A32NX_BTN_RCLLAST"),
];
/// The cockpit only ever writes 1 to these (its SASL bindings have no
/// release), so the host returns each to 0 once a tick has read it, as
/// FlyByWire's `LEFT_LEAVE_CODE` does.
const ECP_BUTTONS: [&str; 14] = [
    "L:A32NX_BTN_TOCONFIG",
    "L:A32NX_BTN_TOCONF",
    "L:A32NX_BTN_CLR",
    "L:A32NX_BTN_CLR2",
    "L:A32NX_BTN_CLR_LH",
    "L:A32NX_BTN_CLR_RH",
    "L:A32NX_BTN_RCL",
    "L:A32NX_BTN_RCLLAST",
    "L:A32NX_BTN_CL",
    "L:A32NX_BTN_CHECK_LH",
    "L:A32NX_BTN_CHECK_RH",
    "L:A32NX_BTN_UP",
    "L:A32NX_BTN_DOWN",
    "L:A32NX_BTN_ABNPROC",
];

impl fws::Vars for FwsVars<'_, '_> {
    fn get(&self, name: &str) -> f64 {
        let mut io = self.0.borrow_mut();
        let value = io.get(name, "number");
        match ECP_BUTTON_ALIASES.iter().find(|(n, _)| *n == name) {
            Some((_, alias)) => value.max(io.get(alias, "number")),
            None => value,
        }
    }
}

struct FwsModule {
    fws: fws::Fws,
    latest: BTreeMap<String, fws::Out>,
    compared_at: f64,
}

impl FwsModule {
    fn step(&mut self, io: &mut dyn Io, ctx: &Ctx) -> Step {
        let events = self.fws.tick(&FwsVars(RefCell::new(io)), ctx.now * 1000.);
        let mut s = Step::default();
        for e in events {
            let name = match &e.out {
                fws::Out::Var(n, v) => {
                    s.vars.push((n.clone(), *v));
                    n.clone()
                }
                fws::Out::Str(n, v) => {
                    s.strings.push((n.clone(), v.clone()));
                    n.clone()
                }
            };
            self.latest.insert(name, e.out);
        }
        s.vars.push((crate::native_shadow_patches::FWS_STARTUP_VAR.to_string(), self.fws.startup_completed() as u8 as f64));
        for (_, e) in self.fws.take_extra() {
            match e {
                fws::Emit::Topic(t, v) => s.bus.push((t, v)),
                fws::Emit::Call(n, a) => s.calls.push((n, a)),
                _ => {}
            }
        }
        if !ctx.authoritative && ctx.now - self.compared_at >= FWS_COMPARE_PERIOD_S {
            self.compared_at = ctx.now;
            for out in self.latest.values() {
                match out {
                    fws::Out::Var(n, v) => s.compare.push((n.clone(), *v)),
                    fws::Out::Str(n, v) => s.strings.push((n.clone(), v.clone())),
                }
            }
        }
        s
    }
}

struct Light {
    sync: LightSync,
    started_ms: Option<f64>,
    last: f64,
    steady_ms: f64,
    inited: bool,
    autobrightness: Option<f64>,
}

impl Light {
    fn new() -> Self {
        Light { sync: LightSync::new(LightSyncConfig { integral_follows_glareshield: true }), started_ms: None, last: -1., steady_ms: 0., inited: false, autobrightness: None }
    }

    fn step(&mut self, io: &mut dyn Io, ctx: &Ctx, later: &mut Vec<String>) -> Step {
        let i = LightSyncInputs {
            glasscockpit_automatic_brightness_pct: io.get("GLASSCOCKPIT AUTOMATIC BRIGHTNESS", "percent"),
            cabin_circuit_switch_on: on(io, "A:CIRCUIT SWITCH ON:151"),
            cabin_using_autobrightness: on(io, "L:A32NX_CABIN_USING_AUTOBRIGHTNESS"),
            time_of_day: io.get("E:TIME OF DAY", "Enum") as i32,
            fmgc_flight_phase: io.get("L:A32NX_FMGC_FLIGHT_PHASE", "number") as i32,
            fwc_flight_phase: io.get("L:A32NX_FWC_FLIGHT_PHASE", "number") as i32,
            on_any_runway: on(io, "A:ON ANY RUNWAY"),
            plane_altitude_ft: io.get("A:PLANE ALTITUDE", "feet"),
            airliner_cruise_altitude: io.get("L:A32NX_AIRLINER_CRUISE_ALTITUDE", "number"),
            cabin_manual_brightness: io.get("L:A32NX_CABIN_MANUAL_BRIGHTNESS", "number"),
        };
        let now_ms = ctx.now * 1000.;
        let start = *self.started_ms.get_or_insert(now_ms);
        let mut s = Step::default();
        let mut commands = Vec::new();
        if !self.inited {
            let v = i.glasscockpit_automatic_brightness_pct;
            if (v - self.last).abs() > 1. {
                self.last = v;
                self.steady_ms = now_ms;
            }
            if v > 0. && now_ms - self.steady_ms > LIGHT_STEADY_MS {
                self.inited = true;
                commands = self.sync.on_ingame(now_ms as u64, &i);
            } else if now_ms - start >= LIGHT_GIVE_UP_MS {
                self.inited = true;
            }
        } else {
            commands = self.sync.on_real_time(now_ms as u64, &i);
        }
        for c in commands {
            s.command(c, later);
        }
        if let Some((_, v)) = s.vars.iter().find(|(n, _)| n == "L:A32NX_CABIN_AUTOBRIGHTNESS") {
            self.autobrightness = Some(*v);
        }
        if let Some(v) = self.autobrightness {
            s.compare.push(("L:A32NX_CABIN_AUTOBRIGHTNESS".to_string(), v));
        }
        s
    }
}

fn rmp_state(v: f64) -> RmpState {
    RmpState::from_value(v).unwrap_or(RmpState::OffStandby)
}

fn rmp_env(io: &mut dyn Io) -> RmpEnv {
    let mut env = RmpEnv {
        states: std::array::from_fn(|r| rmp_state(io.get(&format!("L:A380X_RMP_{}_STATE", r + 1), "number"))),
        fo_sync_efis_enabled: on(io, "L:A32NX_FO_SYNC_EFIS_ENABLED"),
        pilot_seat: if io.get("L:FBW_PILOT_SEAT", "number") == 1. { PilotSeat::Right } else { PilotSeat::Left },
        ..RmpEnv::default()
    };
    for r in 0..3 {
        for v in 0..3 {
            env.vhf_tx[r][v] = on(io, &format!("L:A380X_RMP_{}_VHF_TX_{}", r + 1, v + 1));
            env.vhf_vol[r][v] = io.get(&format!("L:A380X_RMP_{}_VHF_VOL_{}", r + 1, v + 1), "number");
            env.vhf_rx_switch[r][v] = on(io, &format!("L:A380X_RMP_{}_VHF_VOL_RX_SWITCH_{}", r + 1, v + 1));
        }
    }
    env
}

fn com_key(name: &str) -> Option<(usize, &str)> {
    let rest = name.strip_prefix("COM")?;
    match rest.as_bytes().first()? {
        d @ b'1'..=b'3' => Some(((d - b'0') as usize, rest.get(2..)?)),
        b'_' => Some((1, &rest[1..])),
        _ => None,
    }
}

/// One intercepted key: true when handled.
fn handle_key(name: &str, args: &[f64], io: &mut dyn Io, interceptor: &KeyInterceptor, s: &mut Step, later: &mut Vec<String>) -> bool {
    let v0 = args.first().copied();
    let env = rmp_env(io);
    let mut commands = Vec::new();
    let mut freq: Option<Box<dyn FnOnce(&rmp::KeyInterceptorRmp, &mut rmp::Vars)>> = None;
    match name {
        "MASTER_CAUTION_ACKNOWLEDGE" => commands = interceptor.master_caution_acknowledge(env.pilot_seat),
        "MASTER_WARNING_ACKNOWLEDGE" => commands = interceptor.master_warning_acknowledge(env.pilot_seat),
        "PILOT_TRANSMITTER_SET" | "COPILOT_TRANSMITTER_SET" => commands = ki::com_tx_select(&env, v0),
        _ => {
            let Some((idx, rest)) = com_key(name) else { return false };
            let v = v0.unwrap_or(0.);
            match rest {
                "RADIO_FRACT_DEC" => freq = Some(Box::new(move |k, vars| k.fract_increment(idx, -1, false, vars))),
                "RADIO_FRACT_DEC_CARRY" => freq = Some(Box::new(move |k, vars| k.fract_increment(idx, -1, true, vars))),
                "RADIO_FRACT_INC" => freq = Some(Box::new(move |k, vars| k.fract_increment(idx, 1, false, vars))),
                "RADIO_FRACT_INC_CARRY" => freq = Some(Box::new(move |k, vars| k.fract_increment(idx, 1, true, vars))),
                "RADIO_WHOLE_DEC" => freq = Some(Box::new(move |k, vars| k.whole_increment(idx, -1, vars))),
                "RADIO_WHOLE_INC" => freq = Some(Box::new(move |k, vars| k.whole_increment(idx, 1, vars))),
                "RADIO_SET" => freq = Some(Box::new(move |k, vars| k.active_set(idx, false, v, vars))),
                "RADIO_SET_HZ" => freq = Some(Box::new(move |k, vars| k.active_set(idx, true, v, vars))),
                "STBY_RADIO_SET" => freq = Some(Box::new(move |k, vars| k.standby_set(idx, false, v, vars))),
                "STBY_RADIO_SET_HZ" => freq = Some(Box::new(move |k, vars| k.standby_set(idx, true, v, vars))),
                "RADIO_SWAP" | "STBY_RADIO_SWAP" => freq = Some(Box::new(move |k, vars| k.swap(idx, vars))),
                "RECEIVE_SELECT" => commands = ki::com_rx_select(&env, idx, v),
                "TRANSMIT_SELECT" if idx <= 2 => commands = ki::com_tx_select(&env, Some(idx as f64 - 1.)),
                "VOLUME_SET" => commands = ki::com_volume_set(&env, idx, v0),
                "VOLUME_INC" => commands = ki::com_volume_inc(&env, idx, 1),
                "VOLUME_DEC" => commands = ki::com_volume_inc(&env, idx, -1),
                "RECEIVE_ALL_SET" if idx == 1 => commands = ki::com_receive_all(&env, false, v0),
                "RECEIVE_ALL_TOGGLE" if idx == 1 => commands = ki::com_receive_all(&env, true, v0),
                _ => return false,
            }
        }
    }
    if let Some(op) = freq {
        let mut vars = rmp::Vars::new();
        for vhf in 1..=3 {
            for kind in ["FREQUENCY_ACTIVE", "FREQUENCY_STANDBY", "MODE_ACTIVE", "MODE_STANDBY"] {
                let n = format!("L:FBW_RMP_{kind}_{vhf}");
                let x = io.get(&n, "number");
                vars.set_quiet(&n, x);
            }
        }
        op(&rmp::KeyInterceptorRmp { sync_enabled: env.fo_sync_efis_enabled, pilot_seat_right: env.pilot_seat == PilotSeat::Right }, &mut vars);
        s.vars.extend(vars.take_writes().into_iter().map(|w| (w.name, w.value)));
    }
    for c in commands {
        s.command(c, later);
    }
    true
}

struct Seat {
    manager: PilotSeatManager,
    started: bool,
    seat: f64,
    in_deck: f64,
}

impl Seat {
    fn step(&mut self, io: &mut dyn Io, later: &mut Vec<String>) -> Step {
        let mut s = Step::default();
        let mut commands = Vec::new();
        let config = PilotSeatConfig::parse(&io.stored("CONFIG_PILOT_SEAT").unwrap_or_else(|| "Left".to_string()));
        if !self.started {
            self.started = true;
            commands.extend(self.manager.init(config));
        }
        self.manager.set_config(config);
        let pos = (
            io.get("GAME:CAMERA POS IN PLANE:X", "meters"),
            io.get("GAME:CAMERA POS IN PLANE:Y", "meters"),
            io.get("GAME:CAMERA POS IN PLANE:Z", "meters"),
        );
        let state = io.get("CAMERA STATE", "enum") as i32;
        commands.extend(self.manager.update(pos, state));
        for c in commands {
            s.command(c, later);
        }
        for (n, v) in &s.vars {
            match n.as_str() {
                "L:FBW_PILOT_SEAT" => self.seat = *v,
                "L:FBW_IN_FLIGHT_DECK" => self.in_deck = *v,
                _ => {}
            }
        }
        s.compare.push(("L:FBW_PILOT_SEAT".to_string(), self.seat));
        s.compare.push(("L:FBW_IN_FLIGHT_DECK".to_string(), self.in_deck));
        s
    }
}

struct Baro {
    decided: Option<(f64, bool)>,
}

impl Baro {
    fn step(&mut self, io: &mut dyn Io, ctx: &Ctx) -> Step {
        let mut s = Step::default();
        if self.decided.is_none() {
            let config = io.stored("CONFIG_INIT_BARO_UNIT").unwrap_or_else(|| "AUTO".to_string());
            let hpa = match baro_unit::perform_selection(&config) {
                BaroSelection::Fixed { is_hpa } => Some(is_hpa),
                BaroSelection::NeedsNearestAirport => io.nearest_airport().map(|icao| baro_unit::is_hpa_from_nearest(&[icao.as_str()])),
            };
            if let Some(hpa) = hpa {
                self.decided = Some((ctx.now, hpa));
                for n in ["L:XMLVAR_Baro_Selector_HPA_1", "L:XMLVAR_Baro_Selector_HPA_2"] {
                    s.vars.push((n.to_string(), hpa as u8 as f64));
                }
            }
        }
        if let Some((at, hpa)) = self.decided {
            if ctx.now - at < ONCE_COMPARE_WINDOW_S {
                for n in ["L:XMLVAR_Baro_Selector_HPA_1", "L:XMLVAR_Baro_Selector_HPA_2"] {
                    s.compare.push((n.to_string(), hpa as u8 as f64));
                }
            }
        }
        s
    }
}

pub struct Natives {
    config: Config,
    config_read_at: Option<f64>,
    config_fixed: bool,
    last_time: Option<f64>,
    start: Option<f64>,
    comparator: Comparator<Module>,
    mask_written: f64,
    announced: bool,
    vhf: Vhf,
    xpdr: Xpdr,
    amu: Amu,
    btv: Btv,
    fws: FwsModule,
    light: Light,
    keys: KeyInterceptor,
    seat: Seat,
    pushbutton: PushbuttonCheck,
    baro: Baro,
    ready_written: bool,
    pending: Vec<(f64, String, f64)>,
    next_frame_h: Vec<String>,
    rmp: RmpPanels,
    pub rmp_h: Vec<String>,
    pub outbox: Outbox,
}

impl Default for Natives {
    fn default() -> Self {
        Self::new()
    }
}

impl Natives {
    pub fn new() -> Self {
        Natives {
            config: Config::default(),
            config_read_at: None,
            config_fixed: false,
            last_time: None,
            start: None,
            comparator: Comparator::new(),
            mask_written: -1.,
            announced: false,
            vhf: Vhf::new(),
            xpdr: Xpdr { unit: rmp::TransponderUnit::new(), vars: rmp::Vars::new(), started: false },
            amu: Amu::new(),
            btv: Btv { oans: None, with_nav: false },
            fws: FwsModule { fws: fws::Fws::new(0.), latest: BTreeMap::new(), compared_at: f64::NEG_INFINITY },
            light: Light::new(),
            keys: KeyInterceptor::new(),
            seat: Seat { manager: PilotSeatManager::new(A380X_FLIGHT_DECK), started: false, seat: 0., in_deck: 0. },
            pushbutton: PushbuttonCheck::new(),
            baro: Baro { decided: None },
            ready_written: false,
            pending: Vec::new(),
            next_frame_h: Vec::new(),
            rmp: RmpPanels::new(rmp_screens_native()),
            rmp_h: Vec::new(),
            outbox: Outbox::default(),
        }
    }

    pub fn set_config(&mut self, config: Config) {
        self.config = config;
        self.config_fixed = true;
    }

    pub fn frame(&mut self, io: &mut dyn Io, time_s: f64, keys_in: Vec<(String, Vec<f64>)>, lines: &mut Vec<String>) {
        if !self.config_fixed && self.config_read_at.is_none_or(|t| time_s - t >= SETTINGS_PERIOD_S || time_s < t) {
            self.config_read_at = Some(time_s);
            let config = Config::parse(&crate::native_shadow::settings_text());
            if config != self.config || !self.announced {
                self.announced = true;
                let names = |auth: bool| Module::ALL.iter().filter(|m| config.authoritative(**m) == auth).map(|m| m.name()).collect::<Vec<_>>().join(",");
                lines.push(if config.off {
                    "native systems host: off".to_string()
                } else {
                    format!("native systems host: shadow=[{}] authoritative=[{}]", names(false), names(true))
                });
            }
            self.config = config;
        }
        let dt_ms = self.last_time.map_or(0., |t| ((time_s - t) * 1000.).clamp(0., MAX_STEP_MS));
        self.last_time = Some(time_s);
        let rmp_h = std::mem::take(&mut self.rmp_h);
        if dt_ms > 0. {
            for (n, view) in self.rmp.step(io, dt_ms, &rmp_h) {
                publish_rmp_view(n, &view);
            }
        }
        if self.config.off {
            return;
        }
        let mask = self.config.mask();
        if mask != self.mask_written {
            self.mask_written = mask;
            io.set(MASK_VAR, mask);
        }
        let start = *self.start.get_or_insert(time_s);
        self.outbox.h_events.append(&mut self.next_frame_h);
        let due: Vec<_> = self.pending.iter().filter(|(t, _, _)| *t <= time_s).cloned().collect();
        self.pending.retain(|(t, _, _)| *t > time_s);
        for (_, n, v) in due {
            io.set(&n, v);
        }
        if dt_ms <= 0. {
            return;
        }
        let config = self.config.clone();
        let ctx = |m: Module| Ctx { now: time_s, dt_ms, authoritative: config.authoritative(m) };
        let mut later = Vec::new();
        let s = self.vhf.step(io);
        self.settle(Module::Vhf, io, s, time_s, lines);
        let s = self.xpdr.step(io);
        self.settle(Module::Xpdr, io, s, time_s, lines);
        let s = self.amu.step(io);
        self.settle(Module::Amu, io, s, time_s, lines);
        let s = self.btv.step(io, &ctx(Module::Btv));
        self.settle(Module::Btv, io, s, time_s, lines);
        let s = efis_taws(io);
        self.settle(Module::EfisTaws, io, s, time_s, lines);
        let s = oit_health(io);
        self.settle(Module::OitHealth, io, s, time_s, lines);
        let s = self.fws.step(io, &ctx(Module::Fws));
        self.settle(Module::Fws, io, s, time_s, lines);
        let s = self.light.step(io, &ctx(Module::LightSync), &mut later);
        self.settle(Module::LightSync, io, s, time_s, lines);
        let mut s = Step::default();
        for (name, args) in keys_in {
            if !handle_key(&name, &args, io, &self.keys, &mut s, &mut later) {
                lines.push(format!("native systems host: key {name} not handled"));
            }
        }
        self.settle(Module::KeyInterceptor, io, s, time_s, lines);
        let s = self.seat.step(io, &mut later);
        self.settle(Module::PilotSeat, io, s, time_s, lines);
        let mut s = Step::default();
        let to_config = on(io, "L:A32NX_BTN_TOCONFIG") || on(io, "L:A32NX_BTN_TOCONF");
        let phase = io.get("L:A32NX_FWC_FLIGHT_PHASE", "number") as i32;
        if let Some(n) = self.pushbutton.update((time_s * 1000.) as u64, to_config, phase) {
            s.notifications.push((n.title.to_string(), n.message.to_string()));
        }
        self.settle(Module::PushbuttonCheck, io, s, time_s, lines);
        if self.config.authoritative(Module::Fws) {
            for name in ECP_BUTTONS {
                if on(io, name) {
                    io.set(name, 0.);
                }
            }
        }
        let s = self.baro.step(io, &ctx(Module::BaroUnit));
        self.settle(Module::BaroUnit, io, s, time_s, lines);
        let mut s = Step::default();
        if self.config.authoritative(Module::Ready) && !self.ready_written {
            self.ready_written = true;
            s.vars.push(("L:A32NX_IS_READY".to_string(), 1.));
        }
        if time_s - start >= ONCE_COMPARE_WINDOW_S {
            s.compare.push(("L:A32NX_IS_READY".to_string(), 1.));
        }
        self.settle(Module::Ready, io, s, time_s, lines);
        if !later.is_empty() && self.config.authoritative(Module::KeyInterceptor) {
            self.next_frame_h = later;
        }
        lines.extend(self.comparator.summary(time_s));
    }

    fn settle(&mut self, m: Module, io: &mut dyn Io, s: Step, now: f64, lines: &mut Vec<String>) {
        if self.config.authoritative(m) {
            for (n, v) in &s.vars {
                io.set(n, *v);
            }
            for (n, v) in &s.strings {
                io.set_string(n, v);
            }
            for (n, args) in &s.keys {
                io.key(n, args);
            }
            for (n, args) in &s.calls {
                io.call(n, args);
            }
            self.pending.extend(s.delayed.into_iter().map(|(after, n, v)| (now + after, n, v)));
            self.outbox.bus.extend(s.bus);
            self.outbox.notifications.extend(s.notifications);
            self.outbox.h_events.extend(s.h_events);
            return;
        }
        for (n, native) in &s.compare {
            let fbw = io.get(n, unit_of(n));
            lines.extend(self.comparator.observe(m, n, *native, fbw, now));
        }
        for (n, native) in &s.strings {
            let fbw = io.get_string(n);
            lines.extend(self.comparator.observe_text(m, n, native, &fbw, now));
        }
    }

    pub fn take_outbox(&mut self) -> Outbox {
        std::mem::take(&mut self.outbox)
    }
}

struct LiveIo<'a, 'b> {
    host: &'a mut crate::js_bridge::VarsHost<'b>,
    resolved: &'a mut HashMap<(String, String), crate::js_bridge::Resolved>,
}

impl Io for LiveIo<'_, '_> {
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

    fn get_string(&mut self, name: &str) -> String {
        crate::js::Host::get_string(self.host, name)
    }

    fn set_string(&mut self, name: &str, value: &str) {
        crate::js::Host::set_string(self.host, name, value)
    }

    fn failure(&mut self, id: u64) -> bool {
        crate::failures::is_active(id)
    }

    fn key(&mut self, name: &str, args: &[f64]) {
        crate::key_events::push(name, args);
    }

    fn stored(&mut self, _key: &str) -> Option<String> {
        None
    }

    fn nearest_airport(&mut self) -> Option<String> {
        None
    }

    fn call(&mut self, name: &str, args: &str) {
        let _ = crate::js_bridge::direct_call(name, args);
    }
}

struct Running {
    natives: Natives,
    state: crate::js_bridge::HostState,
    env: crate::js_bridge::EnvRefs,
    resolved: HashMap<(String, String), crate::js_bridge::Resolved>,
}

thread_local! {
    static RUNNING: RefCell<Option<Running>> = const { RefCell::new(None) };
}

#[cfg(feature = "native-displays")]
fn publish_rmp_view(n: usize, view: &Value) {
    let batch = json!({ "events": [{ "topic": format!("a380.rmp.view.{n}"), "data": view }] });
    crate::native_displays::inputs::tap_generic_data("xp-ui.bus", &batch.to_string());
}

#[cfg(not(feature = "native-displays"))]
fn publish_rmp_view(_: usize, _: &Value) {}

pub fn tick(vars: &mut crate::Vars, time: f64, h_events: &[String]) {
    let mut lines = Vec::new();
    RUNNING.with(|cell| {
        let mut guard = cell.borrow_mut();
        let running = guard.get_or_insert_with(|| Running {
            natives: Natives::new(),
            state: crate::js_bridge::HostState::default(),
            env: crate::js_bridge::EnvRefs::new(vars.xplm),
            resolved: HashMap::new(),
        });
        let keys_in = take(&KEYS_IN);
        for h in h_events {
            running.natives.fws.fws.h_event(h.trim_start_matches("H:"));
        }
        running.natives.rmp_h.extend(h_events.iter().map(|h| h.trim_start_matches("H:").to_string()));
        for key in take(&FWS_SOUNDS_IN) {
            running.natives.fws.fws.enqueue_sound(&key);
        }
        let mut host = crate::js_bridge::VarsHost::new(vars, &mut running.state, &running.env, time);
        let mut io = LiveIo { host: &mut host, resolved: &mut running.resolved };
        running.natives.frame(&mut io, time, keys_in, &mut lines);
        let out = running.natives.take_outbox();
        push_capped(&SYSTEMS_OUT, out.bus);
        push_capped(&EXTRAS_OUT, out.notifications);
        push_capped(&H_EVENTS, out.h_events);
    });
    for line in lines {
        crate::log(&line);
    }
}

/// Queue a key for the native key interceptor's next frame, as the JS host's key call does.
pub fn inject_key(name: &str, args: &[f64]) {
    push_capped(&KEYS_IN, [(name.to_string(), args.to_vec())]);
}

#[cfg(test)]
#[path = "native_systems_host_tests.rs"]
mod tests;

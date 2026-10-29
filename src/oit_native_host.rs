//! The native OIT (crates/oit_native, crates/oit_pages): FlyByWire's OIT state
//! (display unit, FLT OPS and NSS AVNCS sessions, navigation, menus, fields)
//! driven from plugin data, published per side on `a380.oit.dom.N` for oitx.
//! While the CEF OIT still runs, its snapshot is compared with ours, log only.

use std::collections::{BTreeMap, HashMap, HashSet};

use mfd_tree::{Kid, Node};
use oit_native::act::{self, Act};
use oit_native::data::{self, Backend, FmsInfo, Oooi, OooiInputs};
use oit_native::hit::{self, Rect};
use oit_native::ofp::{self, Loaded};
use oit_native::style::{self, NodeInfo, Styler};
use oit_pages::avncs::{
    AppLdgInputs, AvncsPage, Breaker, CompanyComContent, CompanyComModel, DebugModel, DebugRow, FbwContent, FbwSystemsModel, FlightLogModel, InboxMessage, InboxModel,
    MaintContent, MaintenanceModel, RemoteCbModel,
};
use oit_pages::flt_ops::{
    AocModel, AocMsg, AocOpen, ChartGroup, ChartList, ChartStage, ChartsModel, Chip, FltFolderModel, FltOpsPage, Message, Runway, SheetView, StsModel, ToPerfModel, Weather,
    Weights, AIR_COND, ANTI_ICE, ENTRY_ANGLES, FLAPS, RUNWAY_CONDITIONS, THRUST,
};
use oit_pages::ops_pages::{build_loadsheet, ofp_lines, DocEntry, ExportBoxModel, LoadBoxModel, LoadsheetInput, LoadsheetModel, MelItem, OpsLibraryModel, OpsView, ATA, EXPORT_ITEMS};
use oit_pages::perf_common::{AltimeterUnit, DistanceUnit, Units, WeightUnit};
use oit_pages::perf_model::{PerfCalibration, NO_CALIBRATION};
use oit_pages::perf_pages::{learn_flight_model, resolve_inputs, FlightModelCal, InFltModel, LdgCompute, LdgPerfModel, Live, Module};
use oit_pages::shell::{Avncs, FltOps, OitModel};
use oit_pages::to_compute::{TlrPhase, ToCompute};
use oit_pages::widgets::{Dropdown, Field};
use serde_json::{json, Value};

pub const SCREENS: [&str; 2] = ["SCREEN_OIT_LEFT", "SCREEN_OIT_RIGHT"];
pub const DOM_PREFIX: &str = "a380.oit.dom.";
/// GENERIC: FlyByWire waits 0.25 + random(0.2) s.
const BOOTUP_S: f64 = 0.35;
/// GENERIC: rebuild cadence when nothing was touched.
const REBUILD_S: f64 = 0.25;
const LDG_RUNWAY_CONDITIONS: [&str; 3] = ["DRY", "GOOD", "MEDIUM"];
const LDG_FLAPS: [&str; 2] = ["CONF FULL", "CONF 3"];
const TECHNIQUE: [&str; 2] = ["MANUAL", "AUTOLAND"];
const BRAKE_MODE: [&str; 4] = ["LO", "2", "3", "HI"];
const REVERSE: [&str; 2] = ["YES", "NO"];
const VPILOT: [&str; 5] = ["0", "5", "10", "15", "20"];
const CB_ROWS: usize = 12;
const REMOTE_CB_CTL: &str = "L:A380X_REMOTE_CB_CTRL";

// ---------------------------------------------------------------- display unit

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Du {
    Off,
    Standby,
    Bootup,
    Selftest,
    On,
}

/// OitDisplayUnit.tsx: `updateState` runs when brightness, power or the
/// failed flag change and when its one timer fires.
#[derive(Debug)]
pub struct DisplayUnit {
    pub state: Du,
    timer: Option<f64>,
    last: Option<(bool, bool, bool)>,
}

impl DisplayUnit {
    pub fn new(cold_and_dark: bool) -> Self {
        DisplayUnit { state: if cold_and_dark { Du::Off } else { Du::Standby }, timer: None, last: None }
    }

    pub fn update(&mut self, bright: bool, powered: bool, failed: bool, self_test_s: f64, dt: f64) {
        let mut run = self.last != Some((bright, powered, failed));
        self.last = Some((bright, powered, failed));
        if let Some(t) = self.timer.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                self.timer = None;
                match self.state {
                    Du::Standby => self.state = Du::Off,
                    Du::Selftest => self.state = Du::On,
                    _ => {}
                }
                run = true;
            }
        }
        if run {
            self.step(bright && powered, self_test_s);
        }
    }

    fn step(&mut self, on: bool, self_test_s: f64) {
        match self.state {
            Du::On if !on => {
                self.state = Du::Standby;
                self.timer = Some(10.0);
            }
            Du::Standby if on => {
                self.state = Du::On;
                self.timer = None;
            }
            Du::Off if on => {
                self.state = Du::Bootup;
                self.timer = Some(BOOTUP_S);
            }
            Du::Bootup if on => {
                self.state = Du::Selftest;
                self.timer = Some(self_test_s);
            }
            Du::Selftest | Du::Bootup if !on => {
                self.state = Du::Off;
                self.timer = None;
            }
            _ => {}
        }
    }

    pub fn self_test_visible(&self) -> bool {
        matches!(self.state, Du::Selftest | Du::Bootup)
    }
}

// ------------------------------------------------------------------ navigation

/// OitUiService.tsx's navigation stack.
#[derive(Clone, Debug, Default)]
pub struct Nav {
    stack: Vec<String>,
    fwd: Vec<String>,
    pub uri: String,
}

impl Nav {
    pub fn navigate(&mut self, uri: &str) -> bool {
        if uri == self.uri {
            return false;
        }
        let next = match uri {
            "back" => {
                if self.stack.len() < 2 {
                    return false;
                }
                if let Some(p) = self.stack.pop() {
                    self.fwd.push(p);
                }
                self.stack.last().cloned().unwrap_or_default()
            }
            "forward" => {
                let Some(p) = self.fwd.pop() else { return false };
                self.stack.push(p.clone());
                p
            }
            _ => {
                self.stack.push(uri.to_string());
                self.fwd.clear();
                uri.to_string()
            }
        };
        self.uri = next;
        true
    }

    pub fn reset(&mut self, root: &str) {
        self.stack = vec![root.to_string()];
        self.fwd.clear();
        self.uri = root.to_string();
    }

    pub fn can_back(&self) -> bool {
        self.stack.len() > 1
    }

    pub fn can_forward(&self) -> bool {
        !self.fwd.is_empty()
    }
}

/// The FLT OPS / NSS AVNCS loading bars: 4 Hz countdown from the self-test time.
#[derive(Debug, Default)]
pub struct Loading {
    total: f64,
    remaining: f64,
    last: Option<bool>,
    acc: f64,
}

impl Loading {
    pub fn update(&mut self, healthy: bool, total_if_healthy: f64, dt: f64) {
        if self.last != Some(healthy) {
            self.last = Some(healthy);
            self.total = if healthy { total_if_healthy } else { 0.0 };
            self.remaining = self.total;
            self.acc = 0.0;
            return;
        }
        if healthy && self.remaining > 0.0 {
            self.acc += dt;
            while self.acc >= 0.25 && self.remaining > 0.0 {
                self.acc -= 0.25;
                self.remaining = (self.remaining - 0.25).max(0.0);
            }
        }
    }

    pub fn hidden(&self) -> bool {
        self.last != Some(true) || self.remaining <= 0.0
    }

    pub fn width(&self) -> String {
        format!("{}%", js_num((1.0 - self.remaining / self.total) * 40.0))
    }
}

fn js_num(x: f64) -> String {
    if x == 0.0 {
        "0".into()
    } else {
        format!("{x}")
    }
}

// ---------------------------------------------------------------- page states

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldId {
    StsFlt,
    StsFrom,
    StsTo,
    ToAirport,
    ToShift,
    LdgAirport,
    LdgWeight,
    CbSearch,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Focus {
    Field(FieldId, String),
    Text(char, usize),
    Icao,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Delete,
    Escape,
}

#[derive(Clone, Debug, Default)]
struct Laptop {
    flt: Option<String>,
    from: Option<String>,
    to: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct RunwaySel {
    airport: Option<String>,
    runways: Vec<Runway>,
    selected: Option<usize>,
    message: String,
}

impl RunwaySel {
    fn runway(&self) -> Option<&Runway> {
        self.selected.and_then(|i| self.runways.get(i))
    }

    fn select(&mut self, ident: Option<&str>) {
        self.selected = ident.and_then(|id| self.runways.iter().position(|r| r.ident == id));
    }
}

#[derive(Clone, Debug)]
struct ToState {
    rwy: RunwaySel,
    shift: Option<f64>,
    entry: usize,
    cond: usize,
    air: usize,
    flaps: usize,
    anti_ice: usize,
    thrust: usize,
    weather: Weather,
    weights: Weights,
    thr_red: Option<f64>,
    eo_accel: Option<f64>,
    computed: bool,
    tlr: Option<TlrPhase>,
    tlr_units: String,
}

impl Default for ToState {
    fn default() -> Self {
        ToState {
            rwy: RunwaySel::default(),
            shift: None,
            entry: 0,
            cond: 0,
            air: 0,
            flaps: 0,
            anti_ice: 0,
            thrust: 0,
            weather: Weather::default(),
            weights: Weights::default(),
            thr_red: None,
            eo_accel: None,
            computed: false,
            tlr: None,
            tlr_units: "KG".into(),
        }
    }
}

#[derive(Clone, Debug)]
struct LdgState {
    rwy: RunwaySel,
    weight: Option<f64>,
    source: String,
    weather: Weather,
    cg: Option<f64>,
    cond: usize,
    flaps: usize,
    vpilot: usize,
    technique: usize,
    brake: usize,
    reverse: usize,
    computed: bool,
    tlr: Option<TlrPhase>,
    tlr_units: String,
}

impl Default for LdgState {
    fn default() -> Self {
        LdgState {
            rwy: RunwaySel::default(),
            weight: None,
            source: "NOT ENTERED".into(),
            weather: Weather::default(),
            cg: None,
            cond: 0,
            flaps: 0,
            vpilot: 0,
            technique: 0,
            brake: 0,
            reverse: 0,
            computed: false,
            tlr: None,
            tlr_units: "KG".into(),
        }
    }
}

#[derive(Clone, Debug)]
struct ChartSheet {
    title: String,
    url: String,
    night_url: Option<String>,
    pdf: bool,
}

#[derive(Clone, Debug)]
struct ChartsState {
    enroute: bool,
    source: String,
    icao: String,
    status: String,
    groups: Vec<(String, Vec<ChartSheet>)>,
    list_message: String,
    selected: Option<usize>,
    stage: Option<(String, bool)>,
    view: SheetView,
    night: bool,
}

impl Default for ChartsState {
    fn default() -> Self {
        ChartsState {
            enroute: false,
            source: "navigraph".into(),
            icao: String::new(),
            status: String::new(),
            groups: vec![],
            list_message: String::new(),
            selected: None,
            stage: Some((String::new(), false)),
            view: SheetView::default(),
            night: false,
        }
    }
}

impl ChartsState {
    fn sheet(&self) -> Option<&ChartSheet> {
        self.groups.iter().flat_map(|g| g.1.iter()).nth(self.selected?)
    }
}

#[derive(Clone, Debug, Default)]
struct FolderState {
    tree_hidden: bool,
    selected: Option<usize>,
    message: Option<Message>,
}

#[derive(Clone, Debug)]
struct DocFile {
    path: String,
    entry: DocEntry,
}

#[derive(Clone, Debug, Default)]
struct LibraryState {
    mel: bool,
    docs: Option<Option<Vec<DocFile>>>,
    title: String,
    view: OpsView,
}

#[derive(Clone, Debug)]
pub struct AocStored {
    pub id: u64,
    pub folder: String,
    pub title: String,
    pub from: String,
    pub zulu: String,
    pub time_ms: f64,
    pub lines: Vec<String>,
    pub is_new: bool,
}

/// AocMessageStore.ts: newest first.
#[derive(Clone, Debug, Default)]
pub struct AocStore {
    pub messages: Vec<AocStored>,
    next: u64,
}

impl AocStore {
    pub fn add(&mut self, folder: &str, title: &str, from: &str, lines: Vec<String>, zulu_ms: f64) {
        self.next += 1;
        let t = (zulu_ms / 1000.0).floor() as i64;
        let zulu = format!("{:02}{:02}", t.rem_euclid(86400) / 3600, t.rem_euclid(3600) / 60);
        self.messages.insert(0, AocStored { id: self.next, folder: folder.into(), title: title.into(), from: from.into(), zulu, time_ms: zulu_ms, lines, is_new: true });
    }

    fn mark_read(&mut self, id: u64) {
        if let Some(m) = self.messages.iter_mut().find(|m| m.id == id) {
            m.is_new = false;
        }
    }

    fn in_folder(&self, folder: &str) -> Vec<&AocStored> {
        self.messages.iter().filter(|m| m.folder == folder).collect()
    }

    fn received(&self) -> Vec<&AocStored> {
        self.messages.iter().filter(|m| m.folder != "SENT").collect()
    }
}

#[derive(Clone, Debug)]
struct AocState {
    folder: String,
    open: Option<u64>,
}

impl Default for AocState {
    fn default() -> Self {
        AocState { folder: "INBOX".into(), open: None }
    }
}

#[derive(Clone, Debug, Default)]
struct CbState {
    show_all: bool,
    search: Option<String>,
    selected: Option<String>,
    page: usize,
}

// ------------------------------------------------------------------- requests

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OfpFor {
    Folder,
    Charts,
    LoadBox,
    ToSync,
    LdgSync,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PerfPage {
    To,
    Ldg,
}

#[derive(Clone, Debug, PartialEq)]
enum JobKind {
    Ofp(OfpFor),
    Runways(PerfPage, String),
    ChartIndex(String),
    Docs(bool),
    Doc,
    Mel,
    Export(Option<usize>),
    Cal,
}

#[derive(Clone, Debug, PartialEq)]
enum Req {
    Fetch(String),
    Route { method: String, path: String, query: String, body: String },
    Airport(String),
}

#[derive(Clone, Debug)]
struct Job {
    key: String,
    kind: JobKind,
    req: Req,
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

// ---------------------------------------------------------------------- live

/// Everything a side reads from the plugin for one rebuild.
#[derive(Clone, Debug, Default)]
pub struct LiveData {
    pub fms: FmsInfo,
    pub zulu_ms: f64,
    pub cold_and_dark: bool,
    pub self_test_s: f64,
    pub loading_s: f64,
    pub metric: bool,
    pub powered: [bool; 2],
    pub bright: [bool; 2],
    pub avncs_mode: [bool; 2],
    pub laptop_healthy: [bool; 2],
    pub fltops_ansu: bool,
    pub nss_ansu: [bool; 2],
}

fn setting_num(b: &mut dyn Backend, key: &str, default: f64) -> f64 {
    b.setting(key).and_then(|s| s.trim().parse::<f64>().ok()).unwrap_or(default)
}

fn zulu_ms(b: &mut dyn Backend) -> f64 {
    let y = b.var("E:ZULU YEAR", "number") as i64;
    let m = b.var("E:ZULU MONTH OF YEAR", "number").max(1.0) as u32;
    let d = b.var("E:ZULU DAY OF MONTH", "number").max(1.0) as u32;
    let s = b.var("E:ZULU TIME", "seconds");
    if y < 1971 {
        return s * 1000.0;
    }
    (ofp::days_from_civil(y, m, d) as f64 * 86400.0 + s) * 1000.0
}

fn read_live(b: &mut dyn Backend) -> LiveData {
    let bus = |b: &mut dyn Backend, n: &str| b.var(&format!("L:A32NX_ELEC_{n}_BUS_IS_POWERED"), "bool") != 0.0;
    let capt = bus(b, "AC_2") || bus(b, "DC_2");
    let fo = bus(b, "AC_ESS") || bus(b, "108PH") || bus(b, "DC_1");
    LiveData {
        fms: b.fms(),
        zulu_ms: zulu_ms(b),
        cold_and_dark: data::flag(b, "L:A32NX_COLD_AND_DARK_SPAWN"),
        self_test_s: setting_num(b, "CONFIG_SELF_TEST_TIME", 15.0),
        loading_s: setting_num(b, "CONFIG_SELF_TEST_TIME", 12.0),
        metric: b.setting("CONFIG_USING_METRIC_UNIT").map_or(true, |s| s.trim() != "0"),
        powered: [capt && !data::flag(b, "L:A380X_OIT_CAPT_FAILED"), fo && !data::flag(b, "L:A380X_OIT_FO_FAILED")],
        bright: [b.var("LIGHT POTENTIOMETER:78", "number") != 0.0, b.var("LIGHT POTENTIOMETER:79", "number") != 0.0],
        avncs_mode: [data::flag(b, "L:A380X_SWITCH_OIT_SIDE_LEFT"), data::flag(b, "L:A380X_SWITCH_OIT_SIDE_RIGHT")],
        laptop_healthy: [data::flag(b, "L:A32NX_FLTOPS_LAPTOP_1_IS_HEALTHY"), data::flag(b, "L:A32NX_FLTOPS_LAPTOP_2_IS_HEALTHY")],
        fltops_ansu: data::flag(b, "L:A32NX_FLTOPS_ANSU_1_IS_HEALTHY"),
        nss_ansu: [data::flag(b, "L:A32NX_NSS_ANSU_1_IS_HEALTHY"), data::flag(b, "L:A32NX_NSS_ANSU_2_IS_HEALTHY")],
    }
}

fn read_weather(b: &mut dyn Backend) -> Weather {
    Weather {
        oat: Some(b.var("AMBIENT TEMPERATURE", "celsius").round()),
        qnh: Some(b.var("SEA LEVEL PRESSURE", "millibars")),
        wind_direction_true: Some(b.var("AMBIENT WIND DIRECTION", "degrees")),
        wind_kt: Some(b.var("AMBIENT WIND VELOCITY", "knots")),
        source: "SIMULATED AMBIENT AT AIRCRAFT".into(),
    }
}

fn read_weights(b: &mut dyn Backend) -> Weights {
    Weights {
        zero_fuel_weight_kg: Some(b.var("L:A32NX_AIRFRAME_ZFW", "number")),
        zero_fuel_cg: Some(b.var("L:A32NX_AIRFRAME_ZFW_CG_PERCENT_MAC", "number")),
        fob_kg: data::arinc_value(b, "L:A32NX_FQMS_TOTAL_FUEL_ON_BOARD"),
        gross_weight_kg: data::arinc_value(b, "L:A32NX_FQMS_GROSS_WEIGHT"),
        cg: data::arinc_value(b, "L:A32NX_FQMS_CENTER_OF_GRAVITY_MAC"),
    }
}

fn read_loadsheet(b: &mut dyn Backend, fms: &FmsInfo) -> LoadsheetInput {
    const PAX: [&str; 14] = [
        "MAIN_FWD_A", "MAIN_FWD_B", "MAIN_MID_1A", "MAIN_MID_1B", "MAIN_MID_1C", "MAIN_MID_2A", "MAIN_MID_2B", "MAIN_MID_2C", "MAIN_AFT_A", "MAIN_AFT_B", "UPPER_FWD", "UPPER_MID_A",
        "UPPER_MID_B", "UPPER_AFT",
    ];
    let flight_number = fms.flight_number.clone().unwrap_or_else(|| b.text("ATC FLIGHT NUMBER"));
    LoadsheetInput {
        zfw: b.var("L:A32NX_AIRFRAME_ZFW", "number"),
        gw: b.var("L:A32NX_AIRFRAME_GW", "number"),
        main_door_open: b.var("INTERACTIVE POINT OPEN:0", "percent over 100"),
        mac_zfw: b.var("L:A32NX_AIRFRAME_ZFW_CG_PERCENT_MAC", "number"),
        mac_tow: b.var("L:A32NX_AIRFRAME_GW_CG_PERCENT_MAC", "number"),
        pax_stations: PAX.iter().map(|p| b.var(&format!("L:A32NX_PAX_{p}"), "number")).collect(),
        water_pct: b.var("L:A32NX_CABIN_WATER_QTY_PERCENT", "number"),
        reg: b.text("ATC ID"),
        airline: b.text("ATC AIRLINE"),
        flight_number,
        zulu_seconds: b.var("E:ZULU TIME", "seconds"),
        day: b.var("E:ZULU DAY OF MONTH", "number"),
        month: b.var("E:ZULU MONTH OF YEAR", "number"),
        year: b.var("E:ZULU YEAR", "number"),
        origin: fms.origin.clone(),
        destination: fms.destination.clone(),
    }
}

fn read_inflight(b: &mut dyn Backend) -> Live {
    let w = read_weights(b);
    let alt = data::arinc_value(b, "L:A32NX_ADIRS_ADR_1_ALTITUDE").unwrap_or_else(|| b.var("INDICATED ALTITUDE", "feet"));
    let mach = data::arinc_value(b, "L:A32NX_ADIRS_ADR_1_MACH").unwrap_or_else(|| b.var("AIRSPEED MACH", "mach"));
    let sat = data::arinc_value(b, "L:A32NX_ADIRS_ADR_1_STATIC_AIR_TEMPERATURE").unwrap_or_else(|| b.var("AMBIENT TEMPERATURE", "celsius"));
    let tat = data::arinc_value(b, "L:A32NX_ADIRS_ADR_1_TOTAL_AIR_TEMPERATURE");
    let ff: f64 = (1..=4).map(|e| b.var(&format!("L:A32NX_ENGINE_FF:{e}"), "number")).sum();
    let isa = sat - (oit_pages::inflight_model::atmosphere(alt, 0.0).temperature_k - 273.15);
    Live {
        weight_kg: w.gross_weight_kg,
        fob_kg: w.fob_kg,
        fl: Some((alt / 100.0).round().max(0.0)),
        mach: Some(mach),
        sat_c: Some(sat),
        tat_c: tat,
        isa_dev: Some(isa),
        ff_kg_h: ff,
        gs_kt: b.var("GPS GROUND SPEED", "knots"),
        wind_kt: b.var("AMBIENT WIND VELOCITY", "knots"),
        wind_dir: b.var("AMBIENT WIND DIRECTION", "degrees"),
    }
}

fn read_app_ldg(b: &mut dyn Backend) -> AppLdgInputs {
    let f = |b: &mut dyn Backend, n: &str| data::flag(b, n);
    let v = |b: &mut dyn Backend, n: &str| b.var(n, "number");
    AppLdgInputs {
        ap1: f(b, "L:A32NX_AUTOPILOT_1_ACTIVE"),
        ap2: f(b, "L:A32NX_AUTOPILOT_2_ACTIVE"),
        athr_status: v(b, "L:A32NX_AUTOTHRUST_STATUS"),
        athr_mode: v(b, "L:A32NX_AUTOTHRUST_MODE"),
        prim: [f(b, "L:A32NX_PRIM_1_HEALTHY"), f(b, "L:A32NX_PRIM_2_HEALTHY"), f(b, "L:A32NX_PRIM_3_HEALTHY")],
        sec: [f(b, "L:A32NX_SEC_1_HEALTHY"), f(b, "L:A32NX_SEC_2_HEALTHY"), f(b, "L:A32NX_SEC_3_HEALTHY")],
        sfcc_words: [v(b, "L:A32NX_SFCC_1_SLAT_FLAP_SYSTEM_STATUS_WORD"), v(b, "L:A32NX_SFCC_2_SLAT_FLAP_SYSTEM_STATUS_WORD")],
        rudder_words: [v(b, "L:A32NX_SEC_1_RUDDER_STATUS_WORD"), v(b, "L:A32NX_SEC_3_RUDDER_STATUS_WORD")],
        hyd_green: f(b, "L:A32NX_HYD_GREEN_SYSTEM_1_SECTION_PRESSURE_SWITCH"),
        hyd_yellow: f(b, "L:A32NX_HYD_YELLOW_SYSTEM_1_SECTION_PRESSURE_SWITCH"),
        fws_dw126: [v(b, "L:A32NX_FWC_1_DISCRETE_WORD_126"), v(b, "L:A32NX_FWC_2_DISCRETE_WORD_126")],
        fcdc_healthy: [f(b, "L:A32NX_FCDC_1_HEALTHY"), f(b, "L:A32NX_FCDC_2_HEALTHY")],
        antiskid: f(b, "A:ANTISKID BRAKES ACTIVE"),
        fcdc_dw4: [v(b, "L:A32NX_FCDC_1_FG_DISCRETE_WORD_4"), v(b, "L:A32NX_FCDC_2_FG_DISCRETE_WORD_4")],
        fcdc_dw8: [v(b, "L:A32NX_FCDC_1_FG_DISCRETE_WORD_8"), v(b, "L:A32NX_FCDC_2_FG_DISCRETE_WORD_8")],
        adr_aoa: [v(b, "L:A32NX_ADIRS_ADR_1_ANGLE_OF_ATTACK"), v(b, "L:A32NX_ADIRS_ADR_2_ANGLE_OF_ATTACK"), v(b, "L:A32NX_ADIRS_ADR_3_ANGLE_OF_ATTACK")],
        engines: [f(b, "A:ENG COMBUSTION:1"), f(b, "A:ENG COMBUSTION:2"), f(b, "A:ENG COMBUSTION:3"), f(b, "A:ENG COMBUSTION:4")],
        apu_contactors: [f(b, "L:A32NX_ELEC_CONTACTOR_990XS1_IS_CLOSED"), f(b, "L:A32NX_ELEC_CONTACTOR_990XS2_IS_CLOSED")],
    }
}

fn runway_suffix(raw: &Option<String>) -> Option<String> {
    raw.as_deref().filter(|r| r.len() > 4).map(|r| r[4..].to_string())
}

fn calibration_of(text: &str) -> PerfCalibration {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let f = &v["factors"];
    let n = |k: &str| f[k].as_f64();
    let c = |k: &str| f[k].as_u64().unwrap_or(0) as u32;
    if f.is_null() {
        return NO_CALIBRATION;
    }
    PerfCalibration::clamped(n("thrust"), n("roll"), n("brake"), c("takeoffs"), c("landings"))
}

fn mel_item(v: &Value) -> MelItem {
    let s = |k: &str| v[k].as_str().map(str::to_string);
    MelItem {
        id: s("id").unwrap_or_default(),
        title: s("title").unwrap_or_default(),
        category: s("category"),
        installed: s("installed"),
        required: s("required"),
        placard: s("placard"),
        conditions: s("conditions").unwrap_or_default(),
        ops_procedure_required: v["opsProcedureRequired"].as_bool().unwrap_or(false),
        maintenance_procedure_required: v["maintenanceProcedureRequired"].as_bool().unwrap_or(false),
        ops_procedure: s("opsProcedure"),
    }
}

fn cb_matches(b: &Breaker, query: &Option<String>, show_all: bool) -> bool {
    match query {
        None => show_all || b.state != 0,
        Some(q) => {
            let hay = format!("{} {} {} ATA {} {}", b.name, b.label, b.bus, b.ata, b.ata).to_uppercase();
            q.split_whitespace().all(|w| hay.contains(w))
        }
    }
}

fn page_text(n: &Node, lines: &mut Vec<String>) {
    let mut own = String::new();
    for k in &n.kids {
        match k {
            Kid::Text { text } => own.push_str(text),
            Kid::Node(c) => {
                if !own.trim().is_empty() {
                    lines.push(own.trim().to_string());
                }
                own.clear();
                page_text(c, lines);
            }
        }
    }
    if !own.trim().is_empty() {
        lines.push(own.trim().to_string());
    }
}

fn stamp(zulu_ms: f64) -> String {
    let t = (zulu_ms / 1000.0).floor() as i64;
    let (y, m, d) = ofp::civil_from_days(t.div_euclid(86400));
    let s = t.rem_euclid(86400);
    format!("{y}-{m:02}-{d:02} {:02}{:02}Z", s / 3600, (s % 3600) / 60)
}

// ------------------------------------------------------------------------ side

/// One OIT (captain's or first officer's) and everything only it keeps.
pub struct Side {
    pub capt: bool,
    pub du: DisplayUnit,
    pub flt: Nav,
    pub avn: Nav,
    pub flt_login: bool,
    pub avn_login: bool,
    flt_load: Loading,
    avn_load: Loading,
    laptop_was: Option<bool>,
    ansu_was: Option<bool>,
    live_sig: Option<[bool; 5]>,
    pub menu_open: bool,
    pub functions_open: bool,
    pub open_dd: Option<String>,
    pub switching_bar_hidden: bool,
    pub focus: Option<Focus>,
    pub folders: BTreeMap<String, bool>,
    folder_now: BTreeMap<String, bool>,
    pub texts: HashMap<(char, usize), String>,
    laptop: Laptop,
    units: Units,
    to: ToState,
    ldg: LdgState,
    inflt: InFltModel,
    fm: FlightModelCal,
    inflt_acc: f64,
    loadsheet_kind: &'static str,
    charts: ChartsState,
    folder: FolderState,
    library: LibraryState,
    load_box: LoadBoxModel,
    export: Vec<String>,
    pub aoc: AocStore,
    aoc_page: AocState,
    uplink_acc: f64,
    prelim_sent: bool,
    final_sent: bool,
    seen_uplinks: HashSet<i64>,
    inbox_selected: Option<u64>,
    cb: CbState,
    fws_debug: bool,
    pub ofp: Option<Loaded>,
    calibration: PerfCalibration,
    jobs: Vec<Job>,
    job_seq: u64,
    maint: Option<(String, Value)>,
    maint_age: f64,
    pub writes: Vec<(String, f64)>,
    pub model: OitModel,
    pub nodes: Vec<NodeInfo>,
    pub boxes: Vec<Rect>,
    pub html: String,
    pub published: String,
    pub dirty: bool,
    since_build: f64,
}

fn dd(values: &[&str], idx: Option<usize>, digits: usize, opened: bool) -> Dropdown {
    Dropdown { text: idx.and_then(|i| values.get(i)).map_or_else(|| "-".repeat(digits), |s| s.to_string()), opened, ..Default::default() }
}

fn field(value: Option<String>, placeholder: &str, mandatory: bool) -> Field {
    Field { mandatory: mandatory && value.is_none(), text: value.unwrap_or_else(|| placeholder.to_string()), ..Default::default() }
}

impl Side {
    pub fn new(capt: bool, cold_and_dark: bool) -> Side {
        let mut s = Side {
            capt,
            du: DisplayUnit::new(cold_and_dark),
            flt: Nav::default(),
            avn: Nav::default(),
            flt_login: true,
            avn_login: true,
            flt_load: Loading::default(),
            avn_load: Loading::default(),
            laptop_was: None,
            ansu_was: None,
            live_sig: None,
            menu_open: false,
            functions_open: false,
            open_dd: None,
            switching_bar_hidden: false,
            focus: None,
            folders: BTreeMap::new(),
            folder_now: BTreeMap::new(),
            texts: HashMap::new(),
            laptop: Laptop::default(),
            units: Units::default(),
            to: ToState::default(),
            ldg: LdgState::default(),
            inflt: InFltModel::default(),
            fm: FlightModelCal::default(),
            inflt_acc: 0.0,
            loadsheet_kind: "PRELIM",
            charts: ChartsState::default(),
            folder: FolderState::default(),
            library: LibraryState::default(),
            load_box: LoadBoxModel::default(),
            export: vec!["READY".into(); EXPORT_ITEMS.len()],
            aoc: AocStore::default(),
            aoc_page: AocState::default(),
            uplink_acc: 0.0,
            prelim_sent: false,
            final_sent: false,
            seen_uplinks: HashSet::new(),
            inbox_selected: None,
            cb: CbState::default(),
            fws_debug: false,
            ofp: None,
            calibration: NO_CALIBRATION,
            jobs: vec![],
            job_seq: 0,
            maint: None,
            maint_age: 0.0,
            writes: vec![],
            model: OitModel::default(),
            nodes: vec![],
            boxes: vec![],
            html: String::new(),
            published: String::new(),
            dirty: true,
            since_build: 0.0,
        };
        s.flt.navigate("flt-ops");
        s.avn.navigate("nss-avncs");
        s
    }

    fn index(&self) -> usize {
        if self.capt { 0 } else { 1 }
    }

    pub fn who(&self) -> &'static str {
        if self.capt { "CAPT" } else { "FO" }
    }

    fn job(&mut self, kind: JobKind, req: Req) {
        self.job_seq += 1;
        let key = format!("oit-native-{}-{}", self.index() + 1, self.job_seq);
        self.jobs.retain(|j| j.kind != kind);
        self.jobs.push(Job { key, kind, req });
    }

    fn route(&mut self, kind: JobKind, method: &str, path: &str, query: &str, body: &str) {
        self.job(kind, Req::Route { method: method.into(), path: path.into(), query: query.into(), body: body.into() });
    }

    fn fetch_ofp(&mut self, b: &mut dyn Backend, why: OfpFor) {
        let id = b.setting("CONFIG_OVERRIDE_SIMBRIEF_USERID").unwrap_or_default();
        let user = b.setting("NAVIGRAPH_USERNAME").unwrap_or_default();
        if id.trim().is_empty() && user.trim().is_empty() {
            self.ofp_done(b, why, Err("SET YOUR SIMBRIEF ID IN THE EFB SETTINGS".into()));
            return;
        }
        let q = if id.trim().is_empty() { format!("username={}", enc(user.trim())) } else { format!("userid={}", enc(id.trim())) };
        self.job(JobKind::Ofp(why), Req::Fetch(format!("https://www.simbrief.com/api/xml.fetcher.php?{q}&json=1")));
    }

    fn load_runways(&mut self, page: PerfPage, icao: &str) {
        let sel = match page {
            PerfPage::To => &mut self.to.rwy,
            PerfPage::Ldg => &mut self.ldg.rwy,
        };
        sel.message = "LOADING RUNWAY DATA...".into();
        sel.runways.clear();
        sel.selected = None;
        self.job(JobKind::Runways(page, icao.to_string()), Req::Airport(icao.to_string()));
    }

    fn set_airport(&mut self, page: PerfPage, icao: Option<String>) {
        let sel = match page {
            PerfPage::To => &mut self.to.rwy,
            PerfPage::Ldg => &mut self.ldg.rwy,
        };
        sel.airport = icao.clone();
        match icao {
            Some(i) => self.load_runways(page, &i),
            None => {
                sel.runways.clear();
                sel.selected = None;
            }
        }
    }

    fn refresh_to(&mut self, b: &mut dyn Backend) {
        self.to.weather = read_weather(b);
        self.to.weights = read_weights(b);
        self.to.thr_red = data::arinc_value(b, "L:A32NX_FM1_THR_RED_ALT");
        self.to.eo_accel = data::arinc_value(b, "L:A32NX_FM1_EO_ACC_ALT");
    }

    fn refresh_ldg(&mut self, b: &mut dyn Backend) {
        self.ldg.weather = read_weather(b);
        self.ldg.cg = data::arinc_value(b, "L:A32NX_FQMS_CENTER_OF_GRAVITY_MAC");
    }

    fn ofp_done(&mut self, b: &mut dyn Backend, why: OfpFor, r: Result<Loaded, String>) {
        let z = zulu_ms(b);
        match why {
            OfpFor::Folder => match r {
                Ok(o) => {
                    self.folder.message = None;
                    self.folder.selected = None;
                    self.ofp = Some(o);
                }
                Err(e) => self.folder.message = Some(Message { text: e, warn: true }),
            },
            OfpFor::Charts => match r {
                Ok(o) => {
                    self.ofp = Some(o);
                    self.charts_enroute_list();
                }
                Err(e) => {
                    self.charts.list_message.clear();
                    self.charts.stage = Some((e, true));
                }
            },
            OfpFor::LoadBox => match r {
                Ok(o) => {
                    let s = &o.summary;
                    let name = if s.callsign.is_empty() { s.flight_number.clone() } else { s.callsign.clone() };
                    self.laptop.flt = Some(name.to_uppercase()).filter(|n| !n.is_empty());
                    self.laptop.from = Some(s.origin.clone());
                    self.laptop.to = Some(s.destination.clone());
                    self.load_box.ofp_status = format!("{}-{} LOADED", s.origin, s.destination);
                    let lines = ofp_lines(s);
                    self.aoc.add("OPS", &format!("OFP {}-{}", s.origin, s.destination), "LOAD BOX", lines.clone(), z);
                    self.load_box.detail = lines;
                    self.ofp = Some(o);
                }
                Err(_) => self.load_box.ofp_status = "LOAD FAILED: CHECK SIMBRIEF ID ON THE EFB".into(),
            },
            OfpFor::ToSync => match r {
                Ok(o) => {
                    let origin = o.ofp.origin.clone();
                    if origin.is_empty() {
                        self.to.rwy.message = "SIMBRIEF OFP HAS NO DEPARTURE".into();
                        return;
                    }
                    if self.to.rwy.airport.as_deref() == Some(origin.as_str()) {
                        let r = Some(o.origin_runway.clone()).filter(|r| !r.is_empty());
                        self.to.rwy.select(r.as_deref());
                    } else {
                        self.set_airport(PerfPage::To, Some(origin.clone()));
                    }
                    self.to.tlr = o.tlr_takeoff.clone();
                    self.to.tlr_units = if o.lbs { "LB".into() } else { "KG".into() };
                    let rwy = if o.origin_runway.is_empty() { "--".to_string() } else { o.origin_runway.clone() };
                    self.to.rwy.message = match o.planned_tow.filter(|p| *p > 0.0) {
                        Some(p) => format!("SIMBRIEF {origin}/{rwy} - PLANNED TOW {} T (WEIGHTS BELOW ARE THE AIRCRAFT'S OWN)", (p / 1000.0).round()),
                        None => format!("SIMBRIEF {origin}/{rwy}"),
                    };
                    self.ofp = Some(o);
                    self.refresh_to(b);
                }
                Err(e) => self.to.rwy.message = simbrief_error(&e),
            },
            OfpFor::LdgSync => match r {
                Ok(o) => {
                    let dest = o.ofp.destination.clone();
                    self.ldg.tlr = o.tlr_landing.clone();
                    self.ldg.tlr_units = if o.lbs { "LB".into() } else { "KG".into() };
                    if dest.is_empty() {
                        self.ldg.rwy.message = "SIMBRIEF OFP HAS NO DESTINATION".into();
                        return;
                    }
                    if self.ldg.rwy.airport.as_deref() == Some(dest.as_str()) {
                        let r = Some(o.dest_runway.clone()).filter(|r| !r.is_empty());
                        self.ldg.rwy.select(r.as_deref());
                    } else {
                        self.set_airport(PerfPage::Ldg, Some(dest.clone()));
                    }
                    let rwy = if o.dest_runway.is_empty() { "--".to_string() } else { o.dest_runway.clone() };
                    self.ldg.rwy.message = format!("SIMBRIEF {dest}/{rwy}");
                    self.ofp = Some(o);
                }
                Err(e) => self.ldg.rwy.message = simbrief_error(&e),
            },
        }
        self.dirty = true;
    }

    fn charts_enroute_list(&mut self) {
        let maps: Vec<ChartSheet> = self
            .ofp
            .as_ref()
            .map(|o| o.ofp.sections.iter().filter_map(|s| s.image.clone().map(|u| ChartSheet { title: s.title.clone(), url: u, night_url: None, pdf: false })).collect())
            .unwrap_or_default();
        if maps.is_empty() {
            self.charts.list_message.clear();
            self.charts.groups.clear();
            self.charts.stage = Some(("THE OFP HAS NO ROUTE MAPS".into(), false));
            return;
        }
        self.charts.groups = vec![("ROUTE MAPS".into(), maps)];
        self.open_chart(0);
    }

    fn open_chart(&mut self, i: usize) {
        self.charts.selected = Some(i);
        self.charts.stage = None;
        self.charts.view = SheetView::default();
    }

    fn charts_load(&mut self, b: &mut dyn Backend) {
        self.charts.selected = None;
        self.charts.groups.clear();
        if self.charts.enroute {
            self.charts.status = "ROUTE MAPS FROM THE SIMBRIEF OFP".into();
            if self.ofp.is_some() {
                self.charts_enroute_list();
            } else {
                self.charts.list_message = "FETCHING THE OFP...".into();
                self.fetch_ofp(b, OfpFor::Charts);
            }
            return;
        }
        let icao = self.charts.icao.trim().to_uppercase();
        if icao.len() != 4 {
            self.charts.status.clear();
            self.charts.list_message.clear();
            self.charts.stage = Some(("ENTER A FOUR-LETTER ICAO CODE".into(), false));
            return;
        }
        let label = source_label(&self.charts.source);
        self.charts.status = format!("{label} - {icao}");
        self.charts.list_message = "LOADING...".into();
        self.charts.stage = Some((if self.charts.source == "faa" { "LOADING THE FAA CHART INDEX..." } else { "LOADING..." }.into(), false));
        if self.charts.source == "navigraph" {
            self.route(JobKind::ChartIndex(icao.clone()), "GET", "/study/charts", &format!("icao={}", enc(&icao)), "");
        } else {
            let q = format!("source={}&icao={}", self.charts.source, enc(&icao));
            self.route(JobKind::ChartIndex(icao.clone()), "GET", "/oit/charts", &q, "");
        }
    }

    fn charts_index(&mut self, icao: &str, status: u16, body: &str) {
        let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
        let (charts, error) = if status == 0 {
            (vec![], Some("CHART SERVER NOT AVAILABLE".to_string()))
        } else if let Some(e) = v.get("error") {
            (vec![], Some(e.as_str().map_or_else(|| e.to_string(), str::to_string)))
        } else if !(200..300).contains(&status) {
            (vec![], Some(format!("THE PLUGIN REPLIED {status}")))
        } else {
            let list = v.as_array().cloned().or_else(|| v["charts"].as_array().cloned()).unwrap_or_default();
            (list, None)
        };
        if error.is_some() || charts.is_empty() {
            self.charts.list_message.clear();
            self.charts.stage = Some((error.unwrap_or_else(|| format!("NO CHARTS FOR {icao}")).to_uppercase(), true));
            return;
        }
        let s = |c: &Value, k: &str| c[k].as_str().unwrap_or("").to_string();
        let mut groups: Vec<(String, Vec<ChartSheet>)> = vec![];
        for c in &charts {
            let cat = [s(c, "category"), s(c, "type_code")].into_iter().find(|x| !x.is_empty()).unwrap_or_else(|| "OTHER".into()).to_uppercase();
            let url = [s(c, "url"), s(c, "image_day_url")].into_iter().find(|x| !x.is_empty()).unwrap_or_default();
            if url.is_empty() {
                continue;
            }
            let name = [s(c, "name"), s(c, "id")].into_iter().find(|x| !x.is_empty()).unwrap_or_default().to_uppercase();
            let pdf = s(c, "kind") == "pdf" || url.to_ascii_lowercase().split('?').next().is_some_and(|p| p.ends_with(".pdf"));
            let night = Some(s(c, "image_night_url")).filter(|x| !x.is_empty());
            let sheet = ChartSheet { title: name, url, night_url: night, pdf };
            match groups.iter_mut().find(|g| g.0 == cat) {
                Some(g) => g.1.push(sheet),
                None => groups.push((cat, vec![sheet])),
            }
        }
        self.charts.status = format!("{} - {icao} - {} CHARTS", source_label(&self.charts.source), charts.len());
        self.charts.list_message.clear();
        self.charts.groups = groups;
        if self.charts.groups.iter().any(|g| !g.1.is_empty()) {
            self.open_chart(0);
        }
    }

    fn library_list(&mut self) {
        self.library.title.clear();
        self.library.view = OpsView::Empty;
        if !self.library.mel {
            self.library.docs = None;
            self.route(JobKind::Docs(true), "GET", "/oit/docs", "", "");
        }
    }

    fn docs_of(body: &str) -> Option<Vec<DocFile>> {
        let v: Value = serde_json::from_str(body).ok()?;
        let files = v["files"].as_array()?;
        Some(
            files
                .iter()
                .map(|f| {
                    let s = |k: &str| f[k].as_str().unwrap_or("").to_string();
                    DocFile { path: s("path"), entry: DocEntry { name: s("name"), folder: s("folder"), kind: s("type") } }
                })
                .collect(),
        )
    }

    fn poll_jobs(&mut self, b: &mut dyn Backend) {
        let jobs = std::mem::take(&mut self.jobs);
        let mut keep = vec![];
        let mut done = vec![];
        for j in jobs {
            let r = match &j.req {
                Req::Fetch(url) => b.fetch(&j.key, url),
                Req::Route { method, path, query, body } => b.request(&j.key, method, path, query, body),
                Req::Airport(icao) => b.airport(icao).map(|a| a.map_or((404, String::new()), |t| (200, t))),
            };
            match r {
                Some(r) => done.push((j.kind, r)),
                None => keep.push(j),
            }
        }
        self.jobs = keep;
        for (kind, (status, body)) in done {
            self.dirty = true;
            self.job_done(b, kind, status, &body);
        }
    }

    fn job_done(&mut self, b: &mut dyn Backend, kind: JobKind, status: u16, body: &str) {
        match kind {
            JobKind::Ofp(why) => {
                let r = if status == 0 { Err("SIMBRIEF NOT REACHABLE".into()) } else { ofp::parse(body) };
                self.ofp_done(b, why, r);
            }
            JobKind::Runways(page, icao) => {
                let sel = match page {
                    PerfPage::To => &mut self.to.rwy,
                    PerfPage::Ldg => &mut self.ldg.rwy,
                };
                if sel.airport.as_deref() != Some(icao.as_str()) {
                    return;
                }
                if status != 200 {
                    sel.message = format!("{icao} NOT IN NAVIGATION DATABASE");
                    return;
                }
                let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
                sel.runways = data::runway_ends(&v);
                sel.message = if sel.runways.is_empty() { format!("{icao} HAS NO RUNWAY DATA") } else { String::new() };
                let fms = b.fms();
                let want = match page {
                    PerfPage::To => runway_suffix(&fms.departure_runway),
                    PerfPage::Ldg => runway_suffix(&fms.arrival_runway),
                };
                let sel = match page {
                    PerfPage::To => &mut self.to.rwy,
                    PerfPage::Ldg => &mut self.ldg.rwy,
                };
                sel.select(want.as_deref());
                match page {
                    PerfPage::To => self.to.weather = read_weather(b),
                    PerfPage::Ldg => self.ldg.weather = read_weather(b),
                }
            }
            JobKind::ChartIndex(icao) => self.charts_index(&icao, status, body),
            JobKind::Docs(library) => {
                let docs = if status == 200 { Self::docs_of(body) } else { None };
                if library {
                    self.library.docs = Some(docs);
                } else {
                    match docs {
                        None => self.load_box.docs_status = "LIBRARY SERVER NOT AVAILABLE".into(),
                        Some(d) => {
                            self.load_box.docs_status = format!("{} DOCUMENTS IN OPS LIBRARY", d.len());
                            self.load_box.detail = d
                                .iter()
                                .take(30)
                                .map(|f| format!("{}{}  ({})", if f.entry.folder.is_empty() { String::new() } else { format!("{}/", f.entry.folder) }, f.entry.name, f.entry.kind.to_uppercase()))
                                .collect();
                        }
                    }
                }
            }
            JobKind::Doc => {
                let text = serde_json::from_str::<Value>(body).ok().filter(|_| status == 200).and_then(|v| v["text"].as_str().map(str::to_string));
                self.library.view = match text {
                    Some(t) => OpsView::Markdown { text: t },
                    None => OpsView::Message { text: "DOCUMENT NOT AVAILABLE".into(), amber: true },
                };
            }
            JobKind::Mel => {
                let items = serde_json::from_str::<Value>(body).ok().filter(|_| status == 200).and_then(|v| v["items"].as_array().cloned());
                self.library.view = match items {
                    None => OpsView::Message { text: "MEL NOT AVAILABLE".into(), amber: true },
                    Some(i) if i.is_empty() => OpsView::MelEmpty,
                    Some(i) => OpsView::Mel { items: i.iter().map(mel_item).collect() },
                };
            }
            JobKind::Export(slot) => {
                let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
                let text = if v["ok"].as_bool() == Some(true) { format!("SAVED {}", v["path"].as_str().unwrap_or("")).trim().to_uppercase() } else { "EXPORT FAILED".into() };
                if let Some(s) = slot.and_then(|i| self.export.get_mut(i)) {
                    *s = text;
                }
            }
            JobKind::Cal => self.calibration = if status == 200 { calibration_of(body) } else { NO_CALIBRATION },
        }
    }

    // ---------------------------------------------------------------- navigation

    fn go_flt(&mut self, b: &mut dyn Backend, uri: &str) {
        self.blur(b);
        if self.flt.navigate(uri) {
            self.entered(b);
        }
    }

    fn go_avn(&mut self, b: &mut dyn Backend, uri: &str) {
        self.blur(b);
        self.avn.navigate(uri);
        self.dirty = true;
    }

    /// What a page does when it is (re)rendered.
    fn entered(&mut self, b: &mut dyn Backend) {
        self.open_dd = None;
        self.switching_bar_hidden = false;
        self.dirty = true;
        match self.flt.uri.as_str() {
            "flt-ops/to-perf" => {
                self.refresh_to(b);
                self.route(JobKind::Cal, "GET", "/perf/cal", "", "");
            }
            "flt-ops/ldg-perf" => {
                self.refresh_ldg(b);
                self.route(JobKind::Cal, "GET", "/perf/cal", "", "");
            }
            "flt-ops/flt-folder" => {
                if self.ofp.is_none() {
                    self.folder.message = Some(Message { text: "FETCHING THE OFP FROM SIMBRIEF...".into(), warn: false });
                    self.fetch_ofp(b, OfpFor::Folder);
                }
            }
            "flt-ops/charts" => self.charts_load(b),
            "flt-ops/ops-library" => self.library_list(),
            "flt-ops/aoc" => {
                self.aoc_page = AocState::default();
                self.texts.clear();
            }
            _ => {}
        }
    }

    fn reset_flt_session(&mut self) {
        self.flt.reset("flt-ops");
        self.flt_login = true;
        self.dirty = true;
    }

    // ------------------------------------------------------------------- tick

    pub fn tick(&mut self, b: &mut dyn Backend, live: &LiveData, oooi: &Oooi, dt: f64) {
        let i = self.index();
        let avncs = live.avncs_mode[i];
        let laptop = live.laptop_healthy[i];
        let nss_failed = !live.nss_ansu[0] && !live.nss_ansu[1];
        let flt_failed = !live.fltops_ansu || !laptop;
        let failed = self.du.state == Du::On && ((avncs && nss_failed) || (!avncs && flt_failed));
        let before = self.du.state;
        let sig = [avncs, laptop, live.nss_ansu[0], nss_failed, flt_failed];
        if self.live_sig != Some(sig) {
            self.live_sig = Some(sig);
            self.dirty = true;
        }
        self.du.update(live.bright[i], live.powered[i], failed, live.self_test_s, dt);
        if self.laptop_was != Some(laptop) {
            self.laptop_was = Some(laptop);
            if !laptop {
                self.reset_flt_session();
            }
        }
        let ansu = live.nss_ansu[0];
        if self.ansu_was != Some(ansu) {
            if self.ansu_was.is_some() && !ansu {
                self.avn.navigate("nss-avncs");
            }
            self.ansu_was = Some(ansu);
        }
        let flt_hidden = self.flt_load.hidden();
        let avn_hidden = self.avn_load.hidden();
        self.flt_load.update(laptop, live.loading_s * 2.0, dt);
        self.avn_load.update(ansu, live.loading_s * 1.5, dt);
        if before != self.du.state || flt_hidden != self.flt_load.hidden() || avn_hidden != self.avn_load.hidden() {
            self.dirty = true;
        }
        self.follow_fms(b, &live.fms);
        self.uplink(b, live, dt);
        self.ingest_datalink(b, live.zulu_ms);
        self.poll_jobs(b);
        if self.flt.uri == "flt-ops/in-flt-perf" {
            self.inflt_acc += dt;
            if self.inflt_acc >= 1.0 {
                self.inflt_acc = 0.0;
                self.inflt.live = read_inflight(b);
                learn_flight_model(&mut self.fm, &self.inflt.live, b.var("VERTICAL SPEED", "feet per minute"));
                self.dirty = true;
            }
        }
        self.maint_age += dt;
        self.since_build += dt;
        if self.dirty || self.since_build >= REBUILD_S {
            self.model = self.build_model(b, live, oooi, failed);
        }
    }

    fn follow_fms(&mut self, b: &mut dyn Backend, fms: &FmsInfo) {
        if self.to.rwy.airport.is_none() {
            if let Some(o) = fms.origin.clone() {
                self.set_airport(PerfPage::To, Some(o));
            }
        } else if self.to.rwy.selected.is_none() && !self.to.rwy.runways.is_empty() {
            let r = runway_suffix(&fms.departure_runway);
            self.to.rwy.select(r.as_deref());
            if self.to.rwy.selected.is_some() {
                self.to.weather = read_weather(b);
            }
        }
        if self.ldg.rwy.airport.is_none() {
            if let Some(d) = fms.destination.clone() {
                self.set_airport(PerfPage::Ldg, Some(d));
            }
        } else if self.ldg.rwy.selected.is_none() && !self.ldg.rwy.runways.is_empty() {
            let r = runway_suffix(&fms.arrival_runway);
            self.ldg.rwy.select(r.as_deref());
        }
    }

    /// AocUplink.ts: a loadsheet every 10 s once the flight is known, PRELIM then FINAL.
    fn uplink(&mut self, b: &mut dyn Backend, live: &LiveData, dt: f64) {
        self.uplink_acc += dt;
        if self.uplink_acc < 10.0 {
            return;
        }
        self.uplink_acc = 0.0;
        if self.final_sent || live.fms.origin.is_none() {
            return;
        }
        let kind = if self.prelim_sent { "FINAL" } else { "PRELIM" };
        let input = read_loadsheet(b, &live.fms);
        if let Some(s) = build_loadsheet(kind, &input) {
            self.aoc.add("LOADSHEET", &s.title, "LOAD CONTROL", s.lines, live.zulu_ms);
            if kind == "PRELIM" {
                self.prelim_sent = true;
            } else {
                self.final_sent = true;
            }
            self.dirty = true;
        }
    }

    /// Uplinked AOC free text and weather from the datalink, into the inbox.
    fn ingest_datalink(&mut self, b: &mut dyn Backend, zulu: f64) {
        for topic in ["aocResynchronizeFreetextMessage", "aocResynchronizeWeatherMessage"] {
            let Some(text) = b.topic(topic) else { continue };
            let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
            let uid = v["UniqueMessageID"].as_i64().unwrap_or(-1);
            if v["Direction"].as_i64() == Some(1) || !self.seen_uplinks.insert(uid) {
                continue;
            }
            let station = v["Station"].as_str().unwrap_or("AOC").to_string();
            let body = v["Message"].as_str().unwrap_or("").to_string();
            let folder = if topic.contains("Weather") { "METAR" } else { "INBOX" };
            let title = if folder == "METAR" { "WEATHER" } else { "FREE TEXT" };
            self.aoc.add(folder, title, &station, body.lines().map(str::to_string).collect(), zulu);
            self.dirty = true;
        }
    }

    // ------------------------------------------------------------------ models

    fn build_model(&mut self, b: &mut dyn Backend, live: &LiveData, oooi: &Oooi, failed: bool) -> OitModel {
        let i = self.index();
        let avncs = live.avncs_mode[i];
        let flt_page = self.flt_page(b, live);
        let avn_page = self.avn_page(b, live, oooi);
        OitModel {
            capt: self.capt,
            self_test_visible: self.du.self_test_visible(),
            oit_visible: self.du.state == Du::On,
            ansu_failed: failed,
            flt_ops: FltOps {
                hidden: avncs,
                content_hidden: avncs || !live.laptop_healthy[i],
                login_hidden: !self.flt_login,
                loading_hidden: self.flt_load.hidden(),
                loading_fill_width: self.flt_load.width(),
                menu_opened: self.menu_open,
                functions_opened: self.functions_open,
                page: flt_page,
            },
            avncs: Avncs {
                hidden: !avncs,
                content_hidden: !avncs || !live.nss_ansu[0],
                login_hidden: !self.avn_login,
                loading_hidden: self.avn_load.hidden(),
                loading_fill_width: self.avn_load.width(),
                flt_number: live.fms.flight_number.clone().unwrap_or_default(),
                back_disabled: !self.avn.can_back(),
                forward_disabled: !self.avn.can_forward(),
                connected_text: "Connected.".into(),
                page: avn_page,
            },
        }
    }

    fn dd_open(&self, id: &str) -> bool {
        self.open_dd.as_deref() == Some(&format!("{}_OIT_{id}", self.who()))
    }

    fn with_focus(&self, id: FieldId, mut f: Field) -> Field {
        if let Some(Focus::Field(fid, buf)) = &self.focus {
            if *fid == id {
                if buf.is_empty() {
                    f.value_selected = true;
                } else {
                    f.editing = true;
                    f.text = buf.clone();
                }
            }
        }
        f
    }

    fn flt_page(&mut self, b: &mut dyn Backend, live: &LiveData) -> FltOpsPage {
        match self.flt.uri.as_str() {
            "flt-ops" => FltOpsPage::Menu,
            "flt-ops/sts" => FltOpsPage::Sts(StsModel {
                registration: dd(&["F-FBWA"], Some(0), 8, self.dd_open("acftRegistration")),
                msn: "225".into(),
                ois_version: "20-JAN 25 V1.0".into(),
                flt_number: self.with_focus(FieldId::StsFlt, field(self.laptop.flt.clone(), "----------", true)),
                from_airport: self.with_focus(FieldId::StsFrom, field(self.laptop.from.clone(), "----", true)),
                to_airport: self.with_focus(FieldId::StsTo, field(self.laptop.to.clone(), "----", true)),
            }),
            "flt-ops/to-perf" => FltOpsPage::ToPerf(Box::new(self.to_model(b))),
            "flt-ops/ldg-perf" => FltOpsPage::LdgPerf(Box::new(self.ldg_model())),
            "flt-ops/in-flt-perf" => {
                let mut m = self.inflt.clone();
                m.fm = self.fm.clone();
                FltOpsPage::InFltPerf(Box::new(m))
            }
            "flt-ops/loadsheet" => FltOpsPage::Loadsheet(LoadsheetModel { sheet: self.loadsheet_kind.into(), input: Some(read_loadsheet(b, &live.fms)) }),
            "flt-ops/flt-folder" => FltOpsPage::FltFolder(FltFolderModel {
                tree_hidden: self.folder.tree_hidden,
                ofp: self.ofp.as_ref().map(|o| o.ofp.clone()),
                selected: self.folder.selected,
                message: self.folder.message.clone(),
            }),
            "flt-ops/charts" => FltOpsPage::Charts(self.charts_model()),
            "flt-ops/aoc" => FltOpsPage::Aoc(self.aoc_model()),
            "flt-ops/ops-library" => FltOpsPage::OpsLibrary(Box::new(OpsLibraryModel {
                mel_shelf: self.library.mel,
                docs_loading: self.library.docs.is_none(),
                docs: self.library.docs.clone().flatten().map(|d| d.into_iter().map(|f| f.entry).collect()),
                title: self.library.title.clone(),
                view: self.library.view.clone(),
            })),
            "flt-ops/load-box" => FltOpsPage::LoadBox(self.load_box.clone()),
            "flt-ops/export-box" => FltOpsPage::ExportBox(ExportBoxModel { statuses: self.export.clone() }),
            other => FltOpsPage::Other { uri: other.to_string() },
        }
    }

    fn to_model(&self, b: &mut dyn Backend) -> ToPerfModel {
        let t = &self.to;
        let idents: Vec<&str> = t.rwy.runways.iter().map(|r| r.ident.as_str()).collect();
        let mut rdd = dd(&idents, t.rwy.selected, 4, self.dd_open("toPerfRunway"));
        rdd.values = idents.iter().map(|s| s.to_string()).collect();
        let rating = Some(b.var("L:A32NX_A380X_ENGINE_RATING_TO_KN", "number")).filter(|r| *r > 0.0).unwrap_or(356.81);
        ToPerfModel {
            units: self.units,
            airport: self.with_focus(FieldId::ToAirport, field(t.rwy.airport.clone(), "----", true)),
            runway_dropdown: rdd,
            database_message: t.rwy.message.clone(),
            runway: t.rwy.runway().cloned(),
            takeoff_shift: t.shift,
            shift_field: self.with_focus(FieldId::ToShift, field(t.shift.map(|v| format!("{}", v.round())), "----", false)),
            entry_angle: dd(&ENTRY_ANGLES, Some(t.entry), 3, self.dd_open("toPerfEntryAngle")),
            weather: t.weather.clone(),
            rwy_cond: dd(&RUNWAY_CONDITIONS, Some(t.cond), 3, self.dd_open("toPerfRwyCond")),
            air_cond: dd(&AIR_COND, Some(t.air), 8, self.dd_open("toPerfAirCond")),
            flaps: dd(&FLAPS, Some(t.flaps), 7, self.dd_open("toPerfFlaps")),
            anti_ice: dd(&ANTI_ICE, Some(t.anti_ice), 15, self.dd_open("toPerfAntiIce")),
            thrust: dd(&THRUST, Some(t.thrust), 10, self.dd_open("toPerfThrust")),
            weights: t.weights.clone(),
            thr_red_ft: t.thr_red,
            eo_accel_ft: t.eo_accel,
            compute: Some(ToCompute { pressed: t.computed, tlr: t.tlr.clone(), tlr_weight_units: t.tlr_units.clone(), calibration: self.calibration, rating_kn: Some(rating) }),
            ..Default::default()
        }
    }

    fn ldg_model(&self) -> LdgPerfModel {
        let l = &self.ldg;
        let idents: Vec<&str> = l.rwy.runways.iter().map(|r| r.ident.as_str()).collect();
        let mut rdd = dd(&idents, l.rwy.selected, 4, self.dd_open("ldgPerfRunway"));
        rdd.values = idents.iter().map(|s| s.to_string()).collect();
        let mut lw = self.with_focus(FieldId::LdgWeight, field(l.weight.map(|w| format!("{:.1}", w / 1000.0)), "---.-", true));
        lw.trailing = "T".into();
        LdgPerfModel {
            units: self.units,
            airport: self.with_focus(FieldId::LdgAirport, field(l.rwy.airport.clone(), "----", true)),
            runway_dropdown: rdd,
            database_message: l.rwy.message.clone(),
            runway: l.rwy.runway().cloned(),
            landing_weight_kg: l.weight,
            lw_field: lw,
            lw_source: l.source.clone(),
            weather: l.weather.clone(),
            cg: l.cg,
            rwy_cond: dd(&LDG_RUNWAY_CONDITIONS, Some(l.cond), 6, self.dd_open("ldgPerfRwyCond")),
            flaps: dd(&LDG_FLAPS, Some(l.flaps), 9, self.dd_open("ldgPerfFlaps")),
            vpilot: dd(&VPILOT, Some(l.vpilot), 2, self.dd_open("ldgPerfVpilot")),
            technique: dd(&TECHNIQUE, Some(l.technique), 8, self.dd_open("ldgPerfTechnique")),
            brake: dd(&BRAKE_MODE, Some(l.brake), 2, self.dd_open("ldgPerfBrake")),
            reverse: dd(&REVERSE, Some(l.reverse), 3, self.dd_open("ldgPerfReverse")),
            compute: LdgCompute { pressed: l.computed, tlr: l.tlr.clone(), tlr_weight_units: l.tlr_units.clone(), calibration: self.calibration },
        }
    }

    fn charts_model(&self) -> ChartsModel {
        let c = &self.charts;
        let chips = self
            .ofp
            .as_ref()
            .map(|o| {
                [("DEP", &o.ofp.origin), ("ARR", &o.ofp.destination), ("ALTN", &o.ofp.alternate)]
                    .into_iter()
                    .filter(|(_, i)| !i.is_empty())
                    .map(|(l, i)| Chip { label: l.into(), icao: i.clone() })
                    .collect()
            })
            .unwrap_or_default();
        let list = if c.groups.is_empty() {
            ChartList::Message { text: c.list_message.clone() }
        } else {
            ChartList::Groups { groups: c.groups.iter().map(|g| ChartGroup { name: g.0.clone(), items: g.1.iter().map(|s| s.title.clone()).collect() }).collect(), selected: c.selected }
        };
        let stage = match (&c.stage, c.sheet()) {
            (Some((text, warn)), _) => ChartStage::Message { text: text.clone(), warn: *warn },
            (None, Some(s)) if s.pdf => ChartStage::Pdf,
            (None, Some(s)) => ChartStage::Image { view: SheetView { night_filter: c.night && s.night_url.is_none(), ..c.view.clone() } },
            (None, None) => ChartStage::Message { text: String::new(), warn: false },
        };
        ChartsModel { enroute: c.enroute, source: c.source.clone(), status: c.status.clone(), chips, icao: c.icao.clone(), list, stage, night: c.night }
    }

    fn aoc_model(&self) -> AocModel {
        let folder = self.aoc_page.folder.clone();
        let msgs = self.aoc.in_folder(&folder);
        let open = self.aoc_page.open.and_then(|id| self.aoc.messages.iter().find(|m| m.id == id)).map(|m| AocOpen { from: m.from.clone(), zulu: m.zulu.clone(), lines: m.lines.clone() });
        AocModel {
            folder,
            inboxes_expanded: true,
            messages: msgs.iter().map(|m| AocMsg { title: m.title.clone(), zulu: m.zulu.clone(), is_new: m.is_new }).collect(),
            open,
        }
    }

    fn maintenance(&mut self, b: &mut dyn Backend, uri: &str) -> Value {
        let ask = if uri == "nss-avncs/maintenance" { "nss-avncs/maintenance/current-flight-report" } else { uri };
        let ata = uri.strip_prefix("nss-avncs/maintenance/system-status/").and_then(|a| a.parse::<u32>().ok());
        let ask = if ata.is_some() { "nss-avncs/maintenance/current-flight-report" } else { ask };
        let stale = self.maint.as_ref().map_or(true, |(u, _)| u != uri) || self.maint_age >= 1.0;
        if stale {
            self.maint = Some((uri.to_string(), b.maintenance(ask, ata)));
            self.maint_age = 0.0;
        }
        self.maint.as_ref().map(|m| m.1.clone()).unwrap_or(Value::Null)
    }

    fn breakers(&mut self, b: &mut dyn Backend, v: &Value) -> Vec<Breaker> {
        let list = v["content"]["breakers"].as_array().cloned().unwrap_or_default();
        list.iter()
            .filter_map(|x| serde_json::from_value::<Breaker>(x.clone()).ok())
            .map(|mut br| {
                br.state = b.var(&format!("L:A32NX_BKR_{}_STATUS", br.id), "number").max(0.0) as u32;
                br
            })
            .collect()
    }

    fn avn_page(&mut self, b: &mut dyn Backend, live: &LiveData, oooi: &Oooi) -> AvncsPage {
        let uri = self.avn.uri.clone();
        if uri.starts_with("nss-avncs/company-com") {
            let content = if uri.ends_with("/inbox") {
                let received = self.aoc.received();
                if self.inbox_selected.is_none() {
                    self.inbox_selected = received.first().map(|m| m.id);
                }
                let sel = self.inbox_selected;
                let model = InboxModel {
                    flt_number: live.fms.flight_number.clone(),
                    messages: received
                        .iter()
                        .map(|m| InboxMessage { title: m.title.clone(), from: m.from.clone(), time_ms: Some(m.time_ms), is_new: m.is_new, lines: m.lines.clone() })
                        .collect(),
                    selected: received.iter().position(|m| Some(m.id) == sel),
                };
                if let Some(id) = sel {
                    self.aoc.mark_read(id);
                }
                CompanyComContent::Inbox(model)
            } else if uri.ends_with("/flight-log") {
                CompanyComContent::FlightLog(Box::new(FlightLogModel {
                    flt_number: live.fms.flight_number.clone(),
                    origin: live.fms.origin.clone(),
                    destination: live.fms.destination.clone(),
                    sim_time_ms: Some(live.zulu_ms),
                    out_time: oooi.out.0,
                    off_time: oooi.off.0,
                    on_time: oooi.on.0,
                    in_time: oooi.inb.0,
                    out_fob: oooi.out.1,
                    off_fob: oooi.off.1,
                    on_fob: oooi.on.1,
                    in_fob: oooi.inb.1,
                    flight_time: oooi.flight_ms,
                    block_time: oooi.block_ms,
                    metric: live.metric,
                }))
            } else {
                CompanyComContent::None
            };
            let inbox_count = self.aoc.received().len() as u32;
            return AvncsPage::CompanyCom(CompanyComModel { uri, content, inbox_count, ..Default::default() });
        }
        if uri.starts_with("nss-avncs/maintenance") {
            let v = self.maintenance(b, &uri);
            if uri.ends_with("/remote-cb") {
                let breakers = self.breakers(b, &v);
                let r = RemoteCbModel {
                    ctl_on: b.var(REMOTE_CB_CTL, "number") > 0.0,
                    show_all: self.cb.show_all,
                    search: self.cb.search.clone(),
                    selected: self.cb.selected.clone(),
                    page: self.cb.page,
                    breakers,
                };
                return AvncsPage::Maintenance(MaintenanceModel { uri, content: MaintContent::RemoteCb(Box::new(r)) });
            }
            let content = serde_json::from_value::<MaintenanceModel>(v).map(|m| m.content).unwrap_or_default();
            return AvncsPage::Maintenance(MaintenanceModel { uri, content });
        }
        if uri.starts_with("nss-avncs/a380x-systems") {
            let content = if uri.ends_with("/app-ldg-cap") {
                FbwContent::AppLdgCap(Box::new(read_app_ldg(b)))
            } else if uri.ends_with("/debug-data") {
                let rows = b
                    .topic("a380x_ois_fws_debug_data")
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                    .and_then(|v| v.as_array().cloned())
                    .unwrap_or_default()
                    .iter()
                    .map(|r| match r {
                        Value::Array(a) => DebugRow { label: value_text(a.first()), value: value_text(a.get(1)) },
                        o => DebugRow { label: value_text(o.get("label")), value: value_text(o.get("value")) },
                    })
                    .collect();
                FbwContent::FwsDebug(DebugModel { transmitting: self.fws_debug, rows })
            } else {
                FbwContent::None
            };
            return AvncsPage::FbwSystems(FbwSystemsModel { uri, content });
        }
        AvncsPage::Menu
    }

    // -------------------------------------------------------------- document

    /// Page tree, tap marks, the switching bar, then the cascade.
    pub fn document(&mut self, styler: &Styler) -> bool {
        let mut content = oit_pages::build(&self.model);
        act::annotate(&mut content, &act::Marks { folders: &self.folders, texts: &self.texts });
        self.folder_now = act::folder_state(&content);
        if self.switching_bar_hidden {
            hide_switching_bar(&mut content);
        }
        if let Some(url) = self.chart_url() {
            set_img_src(&mut content, "oefb-sheet", &url);
        }
        if let Some(url) = self.ofp_picture() {
            set_img_src(&mut content, "oefb-picture", &url);
        }
        let key = content.to_json();
        let side = usize::from(!self.capt);
        if LAST_CONTENT.with(|l| l.borrow()[side].as_deref() == Some(key.as_str())) && !self.html.is_empty() {
            self.dirty = false;
            self.since_build = 0.0;
            return false;
        }
        LAST_CONTENT.with(|l| l.borrow_mut()[side] = Some(key));
        let built = styler.build(&style::wrap(content, if self.capt { 1 } else { 2 }));
        self.nodes = built.nodes;
        self.dirty = false;
        self.since_build = 0.0;
        if built.html != self.html {
            self.html = built.html;
            return true;
        }
        false
    }

    fn chart_url(&self) -> Option<String> {
        let s = self.charts.sheet()?;
        Some(if self.charts.night { s.night_url.clone().unwrap_or_else(|| s.url.clone()) } else { s.url.clone() })
    }

    fn ofp_picture(&self) -> Option<String> {
        let o = self.ofp.as_ref()?;
        let i = self.folder.selected.or_else(|| o.ofp.sections.iter().position(|s| s.text.as_deref().is_some_and(|t| !t.is_empty()) || s.image.is_some()))?;
        o.ofp.sections.get(i)?.image.clone()
    }

    // ------------------------------------------------------------------ input

    pub fn pointer(&mut self, b: &mut dyn Backend, x: f64, y: f64) -> bool {
        let act = hit::hit(&self.nodes, &self.boxes, x, y);
        self.dirty = true;
        let Some(a) = act.as_deref().and_then(Act::parse) else {
            self.blur(b);
            self.close_menus();
            return self.du.state == Du::On;
        };
        self.click(b, &a);
        true
    }

    fn close_menus(&mut self) {
        self.menu_open = false;
        self.functions_open = false;
        self.open_dd = None;
    }

    pub fn click(&mut self, b: &mut dyn Backend, a: &Act) {
        let keeps_focus = matches!((a.kind.as_str(), &self.focus), ("f", Some(Focus::Field(..))) | ("t", Some(Focus::Text(..))) | ("icao", Some(Focus::Icao)));
        if !keeps_focus {
            self.blur(b);
        }
        let (menu, functions, open) = (self.menu_open, self.functions_open, self.open_dd.clone());
        self.close_menus();
        self.dirty = true;
        match (a.scope, a.kind.as_str()) {
            (_, "ps") => {
                if a.key.ends_with("_menu_menu") {
                    self.menu_open = !menu;
                } else {
                    self.functions_open = !functions;
                }
            }
            (_, "dd") => {
                if a.key.ends_with("fws_debug_data_enabled_control") {
                    self.fws_debug = !self.fws_debug;
                } else if open.as_deref() != Some(a.key.as_str()) {
                    self.open_dd = Some(a.key.clone());
                }
            }
            (_, "mi") => self.menu_item(b, &a.key),
            ('F', k) => self.click_flt(b, k, &a.key, a.n),
            ('A', k) => self.click_avn(b, k, &a.key, a.n),
            _ => {}
        }
    }

    fn menu_item(&mut self, b: &mut dyn Backend, id: &str) {
        let Some((prefix, i)) = id.rsplit_once('_') else { return };
        let Ok(i) = i.parse::<usize>() else { return };
        let who = self.who();
        if prefix == format!("{who}_OIT_menu_menu") {
            const TARGETS: [&str; 12] = [
                "flt-ops", "flt-ops/flt-folder", "flt-ops/charts", "flt-ops/ops-library", "flt-ops/to-perf", "flt-ops/loadsheet", "flt-ops/ldg-perf", "flt-ops/in-flt-perf", "flt-ops/aoc",
                "flt-ops/sts", "flt-ops/load-box", "flt-ops/export-box",
            ];
            match TARGETS.get(i) {
                Some(t) => self.go_flt(b, t),
                None if i == 12 => {
                    self.go_flt(b, "flt-ops");
                    self.flt_login = true;
                }
                None => {}
            }
            return;
        }
        if prefix == format!("{who}_OIT_menu_functions") {
            match i {
                0 | 10 => self.go_flt(b, "flt-ops"),
                1 => self.go_flt(b, "back"),
                2 => self.go_flt(b, "forward"),
                3 => self.save_page(b, "PRINT"),
                4 => self.save_page(b, "STORED"),
                5 => self.entered(b),
                8 => self.go_flt(b, "flt-ops/ops-library"),
                9 => self.switching_bar_hidden = !self.switching_bar_hidden,
                _ => {}
            }
            return;
        }
        let Some(id) = prefix.strip_prefix(&format!("{who}_OIT_")) else { return };
        match id {
            "toPerfRunway" => {
                self.to.rwy.selected = Some(i).filter(|i| *i < self.to.rwy.runways.len());
                self.to.weather = read_weather(b);
            }
            "toPerfEntryAngle" => self.to.entry = i,
            "toPerfRwyCond" => self.to.cond = i,
            "toPerfAirCond" => self.to.air = i,
            "toPerfFlaps" => self.to.flaps = i,
            "toPerfAntiIce" => self.to.anti_ice = i,
            "toPerfThrust" => self.to.thrust = i,
            "ldgPerfRunway" => {
                self.ldg.rwy.selected = Some(i).filter(|i| *i < self.ldg.rwy.runways.len());
                self.ldg.weather = read_weather(b);
            }
            "ldgPerfRwyCond" => self.ldg.cond = i,
            "ldgPerfFlaps" => self.ldg.flaps = i,
            "ldgPerfVpilot" => self.ldg.vpilot = i,
            "ldgPerfTechnique" => self.ldg.technique = i,
            "ldgPerfBrake" => self.ldg.brake = i,
            "ldgPerfReverse" => self.ldg.reverse = i,
            _ => {}
        }
    }

    fn save_page(&mut self, b: &mut dyn Backend, prefix: &str) {
        let page = oit_pages::flt_ops::render(&self.model.flt_ops.page, self.who());
        let mut lines = vec![];
        page_text(&page.1, &mut lines);
        let st = stamp(zulu_ms(b));
        let title = page.0;
        let body = json!({ "name": format!("{prefix} {title} {st}"), "text": format!("{title}\n{st}\n\n{}\n", lines.join("\n")) });
        self.route(JobKind::Export(None), "POST", "/oit/export", "", &body.to_string());
    }

    fn click_flt(&mut self, b: &mut dyn Backend, kind: &str, key: &str, n: usize) {
        let uri = self.flt.uri.clone();
        let z = zulu_ms(b);
        match kind {
            "b" => match key {
                "PILOT" => self.flt_login = false,
                "SWITCH OFF LAPTOP" => b.set_var(if self.capt { "L:A380X_SWITCH_LAPTOP_POWER_LEFT" } else { "L:A380X_SWITCH_LAPTOP_POWER_RIGHT" }, 0.0),
                "EXIT SESSION" => {
                    self.go_flt(b, "flt-ops");
                    self.flt_login = true;
                }
                "FLT FOLDER" => self.go_flt(b, "flt-ops/flt-folder"),
                "CHARTS" => self.go_flt(b, "flt-ops/charts"),
                "OPS LIBRARY" => self.go_flt(b, "flt-ops/ops-library"),
                "T.O PERF" => self.go_flt(b, "flt-ops/to-perf"),
                "LOADSHEET" => self.go_flt(b, "flt-ops/loadsheet"),
                "LDG PERF" => self.go_flt(b, "flt-ops/ldg-perf"),
                "IN-FLT PERF" => self.go_flt(b, "flt-ops/in-flt-perf"),
                "FLT OPS STS" => self.go_flt(b, "flt-ops/sts"),
                "LOAD BOX" => self.go_flt(b, "flt-ops/load-box"),
                "EXPORT BOX" => self.go_flt(b, "flt-ops/export-box"),
                "AOC" => self.go_flt(b, "flt-ops/aoc"),
                "SYNCHRO AVIONICS" => {
                    let f = b.fms();
                    self.laptop = Laptop { flt: f.flight_number, from: f.origin, to: f.destination };
                }
                "SYNC FMS" if uri == "flt-ops/to-perf" => {
                    let f = b.fms();
                    match f.origin.clone() {
                        Some(o) if self.to.rwy.airport.as_deref() == Some(o.as_str()) => self.to.rwy.select(runway_suffix(&f.departure_runway).as_deref()),
                        Some(o) => self.set_airport(PerfPage::To, Some(o)),
                        None => self.to.rwy.message = "FMS HAS NO DEPARTURE".into(),
                    }
                    self.refresh_to(b);
                }
                "SYNC FMS" => {
                    let f = b.fms();
                    match f.destination.clone() {
                        Some(d) if self.ldg.rwy.airport.as_deref() == Some(d.as_str()) => self.ldg.rwy.select(runway_suffix(&f.arrival_runway).as_deref()),
                        Some(d) => self.set_airport(PerfPage::Ldg, Some(d)),
                        None => self.ldg.rwy.message = "FMS HAS NO DESTINATION".into(),
                    }
                    match data::arinc_value(b, "L:A32NX_FQMS_GROSS_WEIGHT") {
                        Some(gw) => {
                            self.ldg.weight = Some(gw);
                            self.ldg.source = "CURRENT GROSS WEIGHT (NO FMS LANDING WEIGHT PREDICTION AVAILABLE)".into();
                        }
                        None => self.ldg.source = "FQMS GROSS WEIGHT INVALID".into(),
                    }
                    self.refresh_ldg(b);
                }
                "SYNC SIMBRIEF" if uri == "flt-ops/to-perf" => {
                    self.to.rwy.message = "SIMBRIEF REQUEST SENT".into();
                    self.fetch_ofp(b, OfpFor::ToSync);
                }
                "SYNC SIMBRIEF" => {
                    self.ldg.rwy.message = "SIMBRIEF REQUEST SENT".into();
                    self.fetch_ofp(b, OfpFor::LdgSync);
                }
                "SYNC WX" => self.to.weather = read_weather(b),
                "COMPUTE" if uri == "flt-ops/to-perf" => {
                    self.refresh_to(b);
                    self.to.computed = self.to.rwy.selected.is_some();
                    self.route(JobKind::Cal, "GET", "/perf/cal", "", "");
                }
                "COMPUTE" => {
                    self.refresh_ldg(b);
                    self.ldg.computed = true;
                    self.route(JobKind::Cal, "GET", "/perf/cal", "", "");
                }
                "PRELIM" => self.loadsheet_kind = "PRELIM",
                "FINAL" => self.loadsheet_kind = "FINAL",
                "SEND TO AOC" => {
                    let fms = b.fms();
                    if let Some(s) = build_loadsheet(self.loadsheet_kind, &read_loadsheet(b, &fms)) {
                        self.aoc.add("LOADSHEET", &s.title, "LOADSHEET", s.lines, z);
                    }
                }
                "REQUEST PRELIM" | "REQUEST FINAL" => {
                    let fms = b.fms();
                    let kind = if key == "REQUEST PRELIM" { "PRELIM" } else { "FINAL" };
                    if let Some(s) = build_loadsheet(kind, &read_loadsheet(b, &fms)) {
                        self.aoc.add("LOADSHEET", &s.title, "LOADSHEET", s.lines, z);
                    }
                }
                "SEND WX REQUEST" => self.aoc.add("SENT", "WX REQUEST", "CAPT", vec!["WX REQUEST SENT".into(), "AWAITING AIRLINE REPLY".into()], z),
                "SEND" => self.aoc_send(z),
                "CLEAR ALL" => {
                    let f = self.aoc_page.folder.clone();
                    self.aoc.messages.retain(|m| m.folder != f);
                    self.aoc_page.open = None;
                }
                "LOAD" => {
                    self.load_box.ofp_status = "LOADING...".into();
                    self.fetch_ofp(b, OfpFor::LoadBox);
                }
                "CHECK" => {
                    self.load_box.docs_status = "CHECKING...".into();
                    self.route(JobKind::Docs(false), "GET", "/oit/docs", "", "");
                }
                "EXPORT" => self.export_item(b, n, z),
                "DOCUMENTS" | "MEL" => {
                    self.library.mel = key == "MEL";
                    self.library_list();
                }
                "−" | "+" => self.step_inflight(n, key == "+"),
                "LIVE" => {
                    if n == 0 {
                        self.inflt.live_weight = !self.inflt.live_weight;
                    } else {
                        self.inflt.live_level = !self.inflt.live_level;
                    }
                    self.inflt.live = read_inflight(b);
                }
                "CRZ" => self.inflt.module = Module::Crz,
                "HLD" => self.inflt.module = Module::Hld,
                "DES" => self.inflt.module = Module::Des,
                "PLAN" => self.inflt.module = Module::Plan,
                "ATMO" => self.inflt.module = Module::Atmo,
                "AEO" => self.inflt.engines = 4,
                "OEI" => self.inflt.engines = 3,
                "TEO" => self.inflt.engines = 2,
                _ => {}
            },
            "f" => {
                let id = match (uri.as_str(), n) {
                    ("flt-ops/sts", 0) => FieldId::StsFlt,
                    ("flt-ops/sts", 1) => FieldId::StsFrom,
                    ("flt-ops/sts", 2) => FieldId::StsTo,
                    ("flt-ops/to-perf", 0) => FieldId::ToAirport,
                    ("flt-ops/to-perf", 1) => FieldId::ToShift,
                    ("flt-ops/ldg-perf", 0) => FieldId::LdgAirport,
                    ("flt-ops/ldg-perf", 1) => FieldId::LdgWeight,
                    _ => return,
                };
                self.focus_field(b, id);
            }
            "r" => {
                if let Some((group, i)) = key.rsplit_once('_') {
                    let one = i == "1";
                    match group {
                        "oit-perf-opt-weight" => self.units.weight = if one { WeightUnit::Klb } else { WeightUnit::T },
                        "oit-perf-opt-alt" => self.units.altimeter = if one { AltimeterUnit::Inhg } else { AltimeterUnit::Hpa },
                        "oit-perf-opt-dist" => self.units.distance = if one { DistanceUnit::Ft } else { DistanceUnit::M },
                        _ => {}
                    }
                }
            }
            "nf" => {
                self.aoc_page = AocState { folder: key.to_string(), open: None };
                self.texts.clear();
            }
            "row" => {
                let id = self.aoc.in_folder(&self.aoc_page.folder).get(n).map(|m| m.id);
                if let Some(id) = id {
                    self.aoc.mark_read(id);
                    self.aoc_page.open = Some(id);
                }
            }
            "fo" => self.toggle_folder(key),
            "oi" if uri == "flt-ops/flt-folder" => {
                if let Some(o) = &self.ofp {
                    self.folder.selected = o.ofp.sections.iter().enumerate().filter(|(_, s)| s.text.as_deref().is_some_and(|t| !t.is_empty()) || s.image.is_some()).map(|(i, _)| i).nth(n);
                }
            }
            "oi" => self.open_chart(n),
            "ob" => match key {
                "HIDE TREE" | "SHOW TREE" => self.folder.tree_hidden = !self.folder.tree_hidden,
                "FETCH OFP" => {
                    self.folder.message = Some(Message { text: "FETCHING THE OFP FROM SIMBRIEF...".into(), warn: false });
                    self.fetch_ofp(b, OfpFor::Folder);
                }
                "LOAD" => self.charts_load(b),
                "-" => self.charts.view.zoom = (self.charts.view.zoom / 1.25).clamp(0.05, 8.0),
                "+" => self.charts.view.zoom = (self.charts.view.zoom * 1.25).clamp(0.05, 8.0),
                "FIT" => self.charts.view = SheetView { rotation: self.charts.view.rotation, ..SheetView::default() },
                "ROTATE" => self.charts.view.rotation = (self.charts.view.rotation + 90.0) % 360.0,
                "NIGHT" | "DAY" => self.charts.night = !self.charts.night,
                _ => {}
            },
            "oc" => {
                let chips = self.charts_model().chips;
                if let Some(c) = chips.get(n) {
                    self.charts.icao = c.icao.clone();
                    self.charts_load(b);
                }
            }
            "os" => {
                let (seg, i) = key.split_once(':').unwrap_or(("", ""));
                let i: usize = i.parse().unwrap_or(0);
                if seg == "0" {
                    self.charts.enroute = i == 1;
                } else if let Some(s) = ["navigraph", "faa", "local"].get(i) {
                    self.charts.source = s.to_string();
                }
                self.charts_load(b);
            }
            "icao" => self.focus = Some(Focus::Icao),
            "t" => self.focus = Some(Focus::Text('F', n)),
            "lib" => self.open_library(n),
            _ => {}
        }
    }

    fn aoc_send(&mut self, z: f64) {
        let t = |s: &Self, i: usize| s.texts.get(&('F', i)).cloned().unwrap_or_default();
        match self.aoc_page.folder.as_str() {
            "REQUESTS" => {
                let icao = t(self, 0).trim().to_uppercase();
                if !icao.is_empty() {
                    self.aoc.add("SENT", &format!("METAR {icao}"), "CAPT", vec![format!("METAR REQUEST FOR {icao} SENT"), "AWAITING AIRLINE REPLY".into()], z);
                }
            }
            "REPORTS" => {
                let text = t(self, 0);
                let lines = if text.is_empty() { vec!["(EMPTY REPORT)".to_string()] } else { text.split('\n').map(str::to_string).collect() };
                self.aoc.add("SENT", "REPORT", "CAPT", lines, z);
            }
            "ADC DELAY" => {
                let code = Some(t(self, 0)).filter(|c| !c.is_empty()).unwrap_or_else(|| "--".into());
                let mins = Some(t(self, 1)).filter(|c| !c.is_empty()).unwrap_or_else(|| "0".into());
                self.aoc.add("SENT", "ADC DELAY REPORT", "CAPT", vec![format!("DELAY CODE {code}"), format!("DELAY {mins} MIN")], z);
            }
            "DIVERSION" => {
                let ap = Some(t(self, 0).to_uppercase()).filter(|c| !c.is_empty()).unwrap_or_else(|| "----".into());
                let reason = Some(t(self, 1)).filter(|c| !c.is_empty()).unwrap_or_else(|| "N/A".into());
                self.aoc.add("SENT", "DIVERSION REPORT", "CAPT", vec![format!("DIVERSION TO {ap}"), format!("REASON: {reason}")], z);
            }
            _ => {}
        }
    }

    fn export_item(&mut self, b: &mut dyn Backend, i: usize, z: f64) {
        let fms = b.fms();
        let lines: Option<Vec<String>> = match i {
            0 | 1 => build_loadsheet(if i == 0 { "PRELIM" } else { "FINAL" }, &read_loadsheet(b, &fms)).map(|s| s.lines),
            2 => Some(self.aoc.messages.iter().flat_map(|m| {
                let mut v = vec![format!("{}Z {} {} ({})", m.zulu, m.folder, m.title, m.from)];
                v.extend(m.lines.iter().cloned());
                v.push(String::new());
                v
            }).collect::<Vec<_>>()).filter(|v| !v.is_empty()),
            3 => {
                let v = b.maintenance("nss-avncs/maintenance/current-flight-report", None);
                let ev = v["content"]["events"].as_array().cloned().unwrap_or_default();
                Some(ev.iter().map(|e| format!("{}Z  {:<12} ATA {}  {}", oit_pages::avncs::format_zulu(e["zulu_s"].as_f64().unwrap_or(0.0)), oit_pages::avncs::kind_label(e["kind"].as_u64().unwrap_or(0) as u32), e["ata"], e["message"].as_str().unwrap_or(""))).collect::<Vec<_>>()).filter(|v| !v.is_empty())
            }
            4 => Some(vec![
                format!("FLT {}", self.laptop.flt.clone().or(fms.flight_number.clone()).unwrap_or_else(|| "----".into())),
                format!("FROM {}  TO {}", self.laptop.from.clone().or(fms.origin.clone()).unwrap_or_else(|| "----".into()), self.laptop.to.clone().or(fms.destination.clone()).unwrap_or_else(|| "----".into())),
            ]),
            _ => return,
        };
        let Some(lines) = lines else {
            if let Some(s) = self.export.get_mut(i) {
                *s = "NOTHING TO EXPORT".into();
            }
            return;
        };
        if let Some(s) = self.export.get_mut(i) {
            *s = "EXPORTING...".into();
        }
        let st = stamp(z);
        let label = EXPORT_ITEMS[i];
        let body = json!({ "name": format!("{label} {st}"), "text": format!("{label}\n{st}\n\n{}\n", lines.join("\n")) });
        self.route(JobKind::Export(Some(i)), "POST", "/oit/export", "", &body.to_string());
    }

    fn step_inflight(&mut self, row: usize, up: bool) {
        let r = resolve_inputs(&self.inflt);
        let s = if up { 1.0 } else { -1.0 };
        let step = |v: f64, d: f64, lo: f64, hi: f64, digits: i32| {
            let p = 10f64.powi(digits);
            (((v + d * s) * p).round() / p).clamp(lo, hi)
        };
        let m = &mut self.inflt;
        match row {
            0 => m.weight_kg = step(r.weight_kg, 5000.0, 250000.0, 600000.0, 0),
            1 => m.level = step(r.level, 10.0, 100.0, 431.0, 0),
            2 => m.mach = step(r.mach, 0.01, 0.6, 0.89, 2),
            3 => m.isa_dev = step(r.isa_dev, 1.0, -40.0, 40.0, 0),
            4 => m.fuel_factor = step(r.fuel_factor, 0.01, 0.9, 1.3, 2),
            5 => m.distance_nm = step(r.distance_nm, 50.0, 0.0, 8000.0, 0),
            6 => m.wind_kt = step(r.wind_kt, 10.0, -200.0, 200.0, 0),
            7 => m.elevation_ft = step(r.elevation_ft, 100.0, -1000.0, 14000.0, 0),
            _ => {}
        }
    }

    fn open_library(&mut self, n: usize) {
        if self.library.mel {
            let Some((ata, name)) = ATA.get(n) else { return };
            self.library.title = format!("MEL ATA {ata} {name}");
            self.library.view = OpsView::Loading;
            self.route(JobKind::Mel, "GET", "/study/mel", &format!("ata={}", enc(ata)), "");
            return;
        }
        let Some(d) = self.library.docs.clone().flatten().and_then(|d| d.get(n).cloned()) else { return };
        self.library.title = d.entry.name.to_uppercase();
        if d.entry.kind != "text" {
            self.library.view = OpsView::Message { text: format!("{} FILE: OPEN IT ON THE EFB, DOCUMENTS > MY DOCUMENTS.", d.entry.kind.to_uppercase()), amber: false };
            return;
        }
        self.library.view = OpsView::Loading;
        self.route(JobKind::Doc, "GET", "/oit/doc", &format!("path={}", enc(&d.path)), "");
    }

    fn toggle_folder(&mut self, path: &str) {
        let now = self.folder_now.get(path).copied().unwrap_or(true);
        self.folders.insert(path.to_string(), !now);
    }

    fn click_avn(&mut self, b: &mut dyn Backend, kind: &str, key: &str, n: usize) {
        match kind {
            "b" => match key {
                "PILOT" => self.avn_login = false,
                "NSS AVNCS MENU" | "HOME" => self.go_avn(b, "nss-avncs"),
                "COMPANY COM" => self.go_avn(b, "nss-avncs/company-com"),
                "MAINTENANCE" => self.go_avn(b, "nss-avncs/maintenance"),
                "BACK" => self.go_avn(b, "back"),
                "FORWARD" => self.go_avn(b, "forward"),
                "EXIT OIS" => {
                    self.go_avn(b, "nss-avncs");
                    self.avn_login = true;
                }
                "CLEAR" => self.cb.search = None,
                "ABNORMAL" => self.cb.show_all = false,
                "ALL" => self.cb.show_all = true,
                "PREV" => self.cb.page = self.cb.page.saturating_sub(1),
                "NEXT" => self.cb.page += 1,
                "OPEN" | "CLOSE / RESET" => {
                    if let Some(id) = self.cb.selected.clone() {
                        if b.var(REMOTE_CB_CTL, "number") > 0.0 {
                            b.set_var(&format!("L:A32NX_BKR_{id}_CMD"), if key == "OPEN" { 1.0 } else { 2.0 });
                        }
                    }
                }
                _ => {}
            },
            "nf" => {
                let target = match key {
                    "COMPANY COM" => "nss-avncs/company-com",
                    "MAINTENANCE" => "nss-avncs/maintenance",
                    "A380X SYSTEMS" => "nss-avncs/a380x-systems",
                    "Flight Log" => ["nss-avncs/company-com/pre-flight/flight-log", "nss-avncs/company-com/in-flight/flight-log", "nss-avncs/company-com/post-flight/flight-log"]
                        .get(n)
                        .copied()
                        .unwrap_or(""),
                    "Current Flight Report" => "nss-avncs/maintenance/current-flight-report",
                    "Post Flight Report" => "nss-avncs/maintenance/post-flight-report",
                    "By ATA Chapter" => "nss-avncs/maintenance/system-status",
                    "Remote C/B Control" => "nss-avncs/maintenance/remote-cb",
                    "MEL Deferred Items" => "nss-avncs/maintenance/logbook",
                    "App & Ldg Capability" => "nss-avncs/a380x-systems/app-ldg-cap",
                    "FWS Debug" => "nss-avncs/a380x-systems/debug-data",
                    k if k.starts_with("Inbox") => "nss-avncs/company-com/inbox",
                    _ => "",
                };
                if !target.is_empty() {
                    self.go_avn(b, target);
                }
            }
            "fo" => self.toggle_folder(key),
            "ib" => {
                let id = self.aoc.received().get(n).map(|m| m.id);
                if let Some(id) = id {
                    self.inbox_selected = Some(id);
                    self.aoc.mark_read(id);
                }
            }
            "cb" => {
                let v = self.maint.as_ref().map(|m| m.1.clone()).unwrap_or(Value::Null);
                let list = self.breakers(b, &v);
                let matching: Vec<&Breaker> = list.iter().filter(|x| cb_matches(x, &self.cb.search, self.cb.show_all)).collect();
                let pages = matching.len().div_ceil(CB_ROWS).max(1);
                let page = self.cb.page.min(pages - 1);
                if let Some(x) = matching.get(page * CB_ROWS + n) {
                    self.cb.selected = Some(x.id.clone());
                }
            }
            "f" => self.focus_field(b, FieldId::CbSearch),
            _ => {}
        }
    }

    fn focus_field(&mut self, b: &mut dyn Backend, id: FieldId) {
        if !matches!(&self.focus, Some(Focus::Field(f, _)) if *f == id) {
            self.blur(b);
            self.focus = Some(Focus::Field(id, String::new()));
        }
    }

    /// FlyByWire's InputField commits what was typed when it loses focus.
    fn blur(&mut self, b: &mut dyn Backend) {
        if let Some(Focus::Field(id, buf)) = self.focus.take() {
            if !buf.is_empty() {
                self.commit(b, id, &buf);
            }
        }
        self.focus = None;
    }

    fn commit(&mut self, b: &mut dyn Backend, id: FieldId, text: &str) {
        let clear = text == "CLR";
        let t = text.trim().to_uppercase();
        let airport = || Some(t.clone()).filter(|s| s.len() == 4 && s.chars().all(|c| c.is_ascii_alphanumeric()));
        match id {
            FieldId::StsFlt => self.laptop.flt = if clear { None } else { Some(t.clone()).filter(|s| !s.is_empty() && s.len() <= 10) },
            FieldId::StsFrom => {
                if clear || airport().is_some() {
                    self.laptop.from = if clear { None } else { airport() };
                }
            }
            FieldId::StsTo => {
                if clear || airport().is_some() {
                    self.laptop.to = if clear { None } else { airport() };
                }
            }
            FieldId::ToAirport => {
                let v = if clear { None } else { airport() };
                if clear || v.is_some() {
                    self.set_airport(PerfPage::To, v);
                }
            }
            FieldId::LdgAirport => {
                let v = if clear { None } else { airport() };
                if clear || v.is_some() {
                    self.set_airport(PerfPage::Ldg, v);
                }
            }
            FieldId::ToShift => match t.parse::<f64>() {
                _ if clear => self.to.shift = None,
                Ok(v) if (0.0..=6000.0).contains(&v) => self.to.shift = Some(v),
                _ => {}
            },
            FieldId::LdgWeight => match t.parse::<f64>() {
                _ if clear => self.ldg.weight = None,
                Ok(v) if v > 0.0 => self.ldg.weight = Some(v * 1000.0),
                _ => {}
            },
            FieldId::CbSearch => self.cb.search = if clear || t.is_empty() { None } else { Some(t.clone()) },
        }
        let _ = b;
        self.dirty = true;
    }

    pub fn key(&mut self, b: &mut dyn Backend, k: Key) -> bool {
        let Some(focus) = self.focus.clone() else { return false };
        self.dirty = true;
        match focus {
            Focus::Field(id, mut buf) => match k {
                Key::Escape => self.focus = None,
                Key::Enter => self.blur(b),
                Key::Delete => {
                    self.focus = None;
                    self.commit(b, id, "CLR");
                }
                Key::Backspace => {
                    buf.pop();
                    self.focus = Some(Focus::Field(id, buf));
                }
                Key::Char(c) => {
                    let c = c.to_ascii_uppercase();
                    let max = match id {
                        FieldId::StsFlt => 10,
                        FieldId::StsFrom | FieldId::StsTo | FieldId::ToAirport | FieldId::LdgAirport | FieldId::ToShift => 4,
                        FieldId::LdgWeight => 5,
                        FieldId::CbSearch => 20,
                    };
                    if (c.is_ascii_alphanumeric() || " ./-".contains(c)) && buf.chars().count() < max {
                        buf.push(c);
                    }
                    if id == FieldId::CbSearch {
                        self.cb.search = Some(buf.clone()).filter(|s| !s.trim().is_empty());
                        self.cb.page = 0;
                    }
                    self.focus = Some(Focus::Field(id, buf));
                }
            },
            Focus::Text(scope, i) => {
                let t = self.texts.entry((scope, i)).or_default();
                match k {
                    Key::Escape | Key::Enter if !(k == Key::Enter && self.aoc_page.folder == "REPORTS") => self.focus = None,
                    Key::Enter => t.push('\n'),
                    Key::Backspace => {
                        t.pop();
                    }
                    Key::Delete => t.clear(),
                    Key::Char(c) => t.push(c),
                    _ => {}
                }
            }
            Focus::Icao => match k {
                Key::Escape => self.focus = None,
                Key::Enter => {
                    self.focus = None;
                    self.charts_load(b);
                }
                Key::Backspace => {
                    self.charts.icao.pop();
                }
                Key::Delete => self.charts.icao.clear(),
                Key::Char(c) => {
                    if c.is_ascii_alphanumeric() && self.charts.icao.len() < 4 {
                        self.charts.icao.push(c.to_ascii_uppercase());
                    }
                }
            },
        }
        true
    }

    pub fn keyboard(&self) -> bool {
        self.focus.is_some()
    }
}

fn value_text(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) | None => String::new(),
        Some(o) => o.to_string(),
    }
}

fn simbrief_error(e: &str) -> String {
    if e.starts_with("SIMBRIEF") { e.to_string() } else { format!("SIMBRIEF: {}", e.to_uppercase()) }
}

fn source_label(s: &str) -> &'static str {
    match s {
        "faa" => "FAA",
        "local" => "MY CHARTS",
        _ => "NAVIGRAPH",
    }
}

fn has(n: &Node, c: &str) -> bool {
    n.cls.iter().any(|x| x == c)
}

fn kids_mut(n: &mut Node) -> impl Iterator<Item = &mut Node> {
    n.kids.iter_mut().filter_map(|k| if let Kid::Node(c) = k { Some(c) } else { None })
}

/// OitFltOpsHeader.tsx's HIDE SWITCHING BAR: the last header row of the
/// FLT OPS page container (its footer).
fn hide_switching_bar(root: &mut Node) {
    fn find(n: &mut Node) -> bool {
        if has(n, "oit-main") {
            if let Some(page) = kids_mut(n).find(|k| has(k, "oit-page-container")) {
                if let Some(bar) = kids_mut(page).filter(|k| has(k, "oit-header-row")).last() {
                    bar.inline = Some("display: none;".into());
                }
            }
            return true;
        }
        kids_mut(n).any(find)
    }
    find(root);
}

fn set_img_src(n: &mut Node, class: &str, url: &str) {
    if n.tag == "img" && has(n, class) {
        n.attrs.insert("src".into(), url.to_string());
    }
    for k in kids_mut(n) {
        set_img_src(k, class, url);
    }
}

// ------------------------------------------------------------------------ host

/// Both OITs, the shared ANSU flight log and the cascade.
pub struct Oit {
    pub sides: [Side; 2],
    pub oooi: Oooi,
    pub styler: Styler,
    pub mismatches: [u64; 2],
    seeded: bool,
}

impl Oit {
    pub fn new(cold_and_dark: bool) -> Oit {
        Oit { sides: [Side::new(true, cold_and_dark), Side::new(false, cold_and_dark)], oooi: Oooi::default(), styler: Styler::default(), mismatches: [0; 2], seeded: false }
    }

    /// One flight loop: state, models, documents. Returns the sides whose
    /// document changed, with their HTML.
    pub fn tick(&mut self, b: &mut dyn Backend, active: [bool; 2], dt: f64) -> Vec<(usize, String)> {
        let live = read_live(b);
        if !self.seeded {
            self.seeded = true;
            if live.cold_and_dark {
                for s in self.sides.iter_mut() {
                    s.du = DisplayUnit::new(true);
                }
            }
        }
        let hyd =data::flag(b, "L:A32NX_HYD_GREEN_SYSTEM_1_SECTION_PRESSURE_SWITCH") || data::flag(b, "L:A32NX_HYD_YELLOW_SYSTEM_1_SECTION_PRESSURE_SWITCH");
        if !live.nss_ansu[0] {
            self.oooi.reset();
        }
        self.oooi.update(&OooiInputs {
            zulu_ms: live.zulu_ms,
            park_brake: data::flag(b, "L:A32NX_PARK_BRAKE_LEVER_POS"),
            on_ground: oit_pages::avncs::arinc(b.var("L:A32NX_FWC_1_DISCRETE_WORD_126", "number")).bit_or(28, true),
            fob_kg: data::arinc_value(b, "L:A32NX_FQMS_TOTAL_FUEL_ON_BOARD"),
            door_open: b.var("INTERACTIVE POINT OPEN:0", "percent over 100"),
            airspeed: data::arinc_value(b, "L:A32NX_ADIRS_ADR_1_COMPUTED_AIRSPEED").unwrap_or(0.0),
            hyd_pressurised: hyd,
        });
        let mut out = vec![];
        for (i, s) in self.sides.iter_mut().enumerate() {
            if !active[i] {
                continue;
            }
            s.tick(b, &live, &self.oooi, dt);
            if (s.dirty || s.since_build >= REBUILD_S) && s.document(&self.styler) {
                out.push((i, s.html.clone()));
            }
        }
        out
    }

    /// Element-by-element agreement of the CEF snapshot with ours.
    pub fn compare(&mut self, side: usize, cef: &str) -> Option<String> {
        let s = self.sides.get(side)?;
        if s.html.is_empty() {
            return None;
        }
        let (same, total, first) = oit_native::compare(&s.html, cef);
        if same == total {
            return None;
        }
        self.mismatches[side] += 1;
        let n = self.mismatches[side];
        (n == 1 || n % 600 == 0).then(|| format!("native shadow: oit {} {same}/{total} elements agree x{n}, first {}", side + 1, first.unwrap_or_default()))
    }
}

/// The FMS fields the OIT shows, from the active flight plan.
thread_local! {
    static LAST_CONTENT: std::cell::RefCell<[Option<String>; 2]> = const { std::cell::RefCell::new([None, None]) };
}

pub fn fms_info(plan: Option<&fms::plan::FlightPlan>) -> FmsInfo {
    let Some(p) = plan else { return FmsInfo::default() };
    let some = |s: &str| Some(s.to_string()).filter(|s| !s.is_empty());
    FmsInfo {
        flight_number: some(&p.flight_number),
        origin: p.origin_airport.as_ref().and_then(|f| some(&f.ident)),
        departure_runway: p.origin_runway.as_ref().and_then(|r| some(&r.fix.ident)),
        destination: p.destination_airport.as_ref().and_then(|f| some(&f.ident)),
        arrival_runway: p.destination_runway.as_ref().and_then(|r| some(&r.fix.ident)),
    }
}

/// An MSFS airport facility (`runways` only, as `data::runway_ends` reads it)
/// from the navigation database's runway ends.
pub fn facility_json(runways: &[fms::navdb::Runway]) -> String {
    let opposite = |r: &fms::navdb::Runway| {
        let number = (r.number + 17) % 36 + 1;
        let designator = match r.designator {
            1 => 2,
            2 => 1,
            d => d,
        };
        runways.iter().find(|o| o.number == number && o.designator == designator)
    };
    let list: Vec<Value> = runways
        .iter()
        .map(|r| {
            let elev = r.threshold_location.alt.unwrap_or(0.0) * 0.3048;
            let far = opposite(r).and_then(|o| o.threshold_location.alt).map_or(elev, |a| a * 0.3048);
            json!({
                "designation": r.number.to_string(),
                "designatorCharPrimary": r.designator,
                "length": r.length,
                "direction": r.bearing,
                "width": 0.0,
                "primaryElevation": elev,
                "secondaryElevation": far,
            })
        })
        .collect();
    json!({ "runways": list }).to_string()
}

pub fn side_of_topic(topic: &str) -> Option<usize> {
    match topic.strip_prefix(DOM_PREFIX)? {
        "1" => Some(0),
        "2" => Some(1),
        _ => None,
    }
}

pub fn key_of(ch: u32, vkey: u32) -> Option<Key> {
    Some(match vkey {
        0x1B => Key::Escape,
        0x0D => Key::Enter,
        0x08 => Key::Backspace,
        0x2E => Key::Delete,
        _ => match char::from_u32(ch) {
            Some(c) if !c.is_control() => Key::Char(c),
            _ => return None,
        },
    })
}

// ---------------------------------------------------------------- plugin glue

mod plugin {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::Mutex;

    use oit_native::data::{Backend, FmsInfo};
    use oit_native::hit::Rect;
    use serde_json::{json, Value};

    use super::{key_of, side_of_topic, Oit, DOM_PREFIX, SCREENS};
    use crate::js_bridge::{EnvRefs, HostState, Resolved, VarsHost};

    struct PluginBackend<'a> {
        host: VarsHost<'a>,
        resolved: &'a mut HashMap<(String, String), Resolved>,
    }

    impl Backend for PluginBackend<'_> {
        fn var(&mut self, name: &str, unit: &str) -> f64 {
            if name.starts_with("L:") {
                if let Some(v) = crate::datalink_host::var(name) {
                    return v;
                }
            }
            let host = &mut self.host;
            let r = *self.resolved.entry((name.to_string(), unit.to_string())).or_insert_with(|| host.resolve(name, unit));
            self.host.read(name, r)
        }

        fn text(&mut self, name: &str) -> String {
            crate::livery_identity::string_var(name).unwrap_or_default()
        }

        fn set_var(&mut self, name: &str, value: f64) {
            let host = &mut self.host;
            let r = *self.resolved.entry((name.to_string(), "number".to_string())).or_insert_with(|| host.resolve(name, "number"));
            self.host.write(name, r, value);
        }

        fn request(&mut self, key: &str, method: &str, path: &str, query: &str, body: &str) -> Option<(u16, String)> {
            let (method, path, query, body) = (method.to_string(), path.to_string(), query.to_string(), body.to_string());
            crate::net::poll_job(key, move || match path.as_str() {
                "/perf/cal" => (200, crate::perf_cal::cal_json()),
                "/study/mel" => (200, crate::study::web::mel_search_json(&query)),
                "/study/charts" => {
                    let icao = query.split('&').find_map(|kv| kv.strip_prefix("icao=")).unwrap_or("").to_string();
                    crate::oans::plugin::charts_index(&icao)
                }
                _ => crate::oit_files::route(&method, &path, &query, &body).unwrap_or((404, "{}".into())),
            })
        }

        fn fetch(&mut self, key: &str, url: &str) -> Option<(u16, String)> {
            crate::net::poll(key, "GET", url, "")
        }

        fn setting(&mut self, key: &str) -> Option<String> {
            let root = crate::xp::system_path()?;
            crate::efb_native::host::load_settings(&root).get(key).map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string))
        }

        fn airport(&mut self, icao: &str) -> Option<Option<String>> {
            let db = crate::native_fms_shadow::navdb()?;
            if db.airport(icao).is_none() {
                return Some(None);
            }
            let runways = db.runways(icao);
            Some(Some(super::facility_json(&runways)))
        }

        fn topic(&mut self, name: &str) -> Option<String> {
            crate::datalink_host::string_var(name)
        }

        fn fms(&mut self) -> FmsInfo {
            FMS.lock().ok().and_then(|f| f.clone()).unwrap_or_default()
        }

        fn maintenance(&mut self, uri: &str, ata: Option<u32>) -> Value {
            crate::maintenance_log::avncs_model(uri, ata)
        }
    }

    struct Glue {
        oit: Oit,
        state: HostState,
        env: EnvRefs,
        resolved: HashMap<(String, String), Resolved>,
        last_time: Option<f64>,
        published: [String; 2],
    }

    thread_local! {
        static GLUE: RefCell<Option<Glue>> = const { RefCell::new(None) };
    }
    static BOXES: Mutex<[Option<Vec<Rect>>; 2]> = Mutex::new([None, None]);
    static POINTERS: Mutex<Vec<(usize, f64, f64)>> = Mutex::new(Vec::new());
    static KEYS: Mutex<Vec<(usize, u32, u32)>> = Mutex::new(Vec::new());
    static FMS: Mutex<Option<FmsInfo>> = Mutex::new(None);
    static FOCUSED: Mutex<[bool; 2]> = Mutex::new([false, false]);

    fn publish(side: usize, html: &str) {
        let batch = json!({ "events": [{ "topic": format!("{DOM_PREFIX}{}", side + 1), "data": Value::String(html.to_string()) }] });
        crate::native_displays::inputs::tap_generic_data("xp-ui.bus", &batch.to_string());
    }

    static FMS_AT: Mutex<Option<f64>> = Mutex::new(None);

    /// Once a second: whether the caller should hand over a fresh [`set_fms`].
    pub fn fms_due(time: f64) -> bool {
        let Ok(mut at) = FMS_AT.lock() else { return false };
        let due = at.map_or(true, |t| time < t || time - t >= 1.0);
        if due {
            *at = Some(time);
        }
        due
    }

    /// The FMS's flight number, airports and runways, from wherever the native FMS runs.
    pub fn set_fms(info: FmsInfo) {
        if let Ok(mut f) = FMS.lock() {
            *f = Some(info);
        }
    }

    /// Once per flight loop, after the systems.
    pub fn tick(vars: &mut crate::Vars, time: f64) {
        let active = [crate::native_displays::input::is_native(SCREENS[0]), crate::native_displays::input::is_native(SCREENS[1])];
        if !active[0] && !active[1] {
            return;
        }
        let pointers = POINTERS.lock().map(|mut p| std::mem::take(&mut *p)).unwrap_or_default();
        let keys = KEYS.lock().map(|mut k| std::mem::take(&mut *k)).unwrap_or_default();
        let boxes = BOXES.lock().map(|mut b| [b[0].take(), b[1].take()]).unwrap_or([None, None]);
        let _ = GLUE.try_with(|cell| {
            let Ok(mut guard) = cell.try_borrow_mut() else { return };
            let g = guard.get_or_insert_with(|| Glue {
                oit: Oit::new(false),
                state: HostState::default(),
                env: EnvRefs::new(vars.xplm),
                resolved: HashMap::new(),
                last_time: None,
                published: [String::new(), String::new()],
            });
            let dt = g.last_time.map_or(0.0, |t| (time - t).clamp(0.0, 1.0));
            g.last_time = Some(time);
            let mut b = PluginBackend { host: VarsHost::new(vars, &mut g.state, &g.env, time), resolved: &mut g.resolved };
            for (i, bx) in boxes.into_iter().enumerate() {
                if let Some(bx) = bx {
                    g.oit.sides[i].boxes = bx;
                }
            }
            for (i, x, y) in pointers {
                g.oit.sides[i].pointer(&mut b, x, y);
            }
            for (i, ch, vkey) in keys {
                if let Some(k) = key_of(ch, vkey) {
                    g.oit.sides[i].key(&mut b, k);
                }
            }
            for (i, html) in g.oit.tick(&mut b, active, dt) {
                if html != g.published[i] {
                    publish(i, &html);
                    g.published[i] = html;
                }
            }
            if let Ok(mut f) = FOCUSED.lock() {
                *f = [g.oit.sides[0].keyboard(), g.oit.sides[1].keyboard()];
            }
        });
    }

    fn side_of(screen: &str) -> Option<usize> {
        SCREENS.iter().position(|s| *s == screen)
    }

    /// oitx's boxes for the published document (element order, CSS pixels). Any thread.
    pub fn set_boxes(variant: &str, xywh: &[[f32; 4]]) {
        let i = if variant == "2" { 1 } else { 0 };
        let rects = xywh.iter().map(|b| Rect::new(b[0] as f64, b[1] as f64, b[2] as f64, b[3] as f64)).collect();
        if let Ok(mut b) = BOXES.lock() {
            b[i] = Some(rects);
        }
    }

    /// A press on an OIT in CSS pixels; handled on the next flight loop.
    pub fn pointer(screen: &str, x: f64, y: f64) -> bool {
        let Some(i) = side_of(screen) else { return false };
        if let Ok(mut p) = POINTERS.lock() {
            p.push((i, x, y));
        }
        true
    }

    /// A console keystroke; `false` while no OIT field has the keyboard.
    pub fn key(screen: &str, ch: u32, vkey: u32) -> bool {
        let Some(i) = side_of(screen) else { return false };
        if !FOCUSED.lock().map(|f| f[i]).unwrap_or(false) {
            return false;
        }
        if let Ok(mut k) = KEYS.lock() {
            k.push((i, ch, vkey));
        }
        true
    }

    /// The CEF view's OIT DOM batch: compared with ours (log only); `true`
    /// when the native OIT publishes instead, so the caller drops it.
    pub fn observe_cef(batch: &str) -> bool {
        let v: Value = serde_json::from_str(batch).unwrap_or(Value::Null);
        let Some(i) = v["events"][0]["topic"].as_str().and_then(side_of_topic) else { return false };
        let html = v["events"][0]["data"].as_str().unwrap_or("").to_string();
        GLUE.try_with(|cell| {
            let mut guard = cell.try_borrow_mut().ok()?;
            let g = guard.as_mut()?;
            if g.published[i].is_empty() {
                return Some(false);
            }
            if let Some(line) = g.oit.compare(i, &html) {
                crate::log(&line);
            }
            Some(true)
        })
        .ok()
        .flatten()
        .unwrap_or(false)
    }
}

pub use plugin::{fms_due, key, observe_cef, pointer, set_boxes, set_fms, tick};

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fake {
        vars: HashMap<String, f64>,
        texts: HashMap<String, String>,
        settings: HashMap<String, String>,
        writes: Vec<(String, f64)>,
        requests: Vec<String>,
        answers: HashMap<String, (u16, String)>,
        airports: HashMap<String, String>,
        fms: FmsInfo,
        maintenance: Value,
    }

    impl Backend for Fake {
        fn var(&mut self, name: &str, _unit: &str) -> f64 {
            self.vars.get(name).copied().unwrap_or(0.0)
        }
        fn text(&mut self, name: &str) -> String {
            self.texts.get(name).cloned().unwrap_or_default()
        }
        fn set_var(&mut self, name: &str, value: f64) {
            self.writes.push((name.into(), value));
            self.vars.insert(name.into(), value);
        }
        fn request(&mut self, _key: &str, method: &str, path: &str, query: &str, _body: &str) -> Option<(u16, String)> {
            let k = format!("{method} {path}?{query}");
            self.requests.push(k.clone());
            self.answers.get(&k).cloned()
        }
        fn fetch(&mut self, _key: &str, url: &str) -> Option<(u16, String)> {
            self.requests.push(format!("FETCH {url}"));
            self.answers.get(url).cloned()
        }
        fn setting(&mut self, key: &str) -> Option<String> {
            self.settings.get(key).cloned()
        }
        fn airport(&mut self, icao: &str) -> Option<Option<String>> {
            Some(self.airports.get(icao).cloned())
        }
        fn topic(&mut self, _name: &str) -> Option<String> {
            None
        }
        fn fms(&mut self) -> FmsInfo {
            self.fms.clone()
        }
        fn maintenance(&mut self, _uri: &str, _ata: Option<u32>) -> Value {
            self.maintenance.clone()
        }
    }

    fn powered() -> Fake {
        let mut f = Fake::default();
        for (k, v) in [
            ("L:A32NX_ELEC_AC_2_BUS_IS_POWERED", 1.0),
            ("L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED", 1.0),
            ("LIGHT POTENTIOMETER:78", 75.0),
            ("LIGHT POTENTIOMETER:79", 75.0),
            ("L:A32NX_FLTOPS_LAPTOP_1_IS_HEALTHY", 1.0),
            ("L:A32NX_FLTOPS_LAPTOP_2_IS_HEALTHY", 1.0),
            ("L:A32NX_FLTOPS_ANSU_1_IS_HEALTHY", 1.0),
            ("L:A32NX_NSS_ANSU_1_IS_HEALTHY", 1.0),
            ("L:A32NX_NSS_ANSU_2_IS_HEALTHY", 1.0),
            ("E:ZULU YEAR", 2026.0),
            ("E:ZULU MONTH OF YEAR", 10.0),
            ("E:ZULU DAY OF MONTH", 22.0),
            ("E:ZULU TIME", 3600.0 * 12.5),
        ] {
            f.vars.insert(k.into(), v);
        }
        f.settings.insert("CONFIG_SELF_TEST_TIME".into(), "2".into());
        f
    }

    fn run(oit: &mut Oit, b: &mut Fake, secs: f64) {
        let mut t = 0.0;
        while t < secs {
            oit.tick(b, [true, true], 0.05);
            t += 0.05;
        }
    }

    fn ready() -> (Oit, Fake) {
        let mut b = powered();
        let mut oit = Oit::new(false);
        run(&mut oit, &mut b, 6.0);
        (oit, b)
    }

    fn acts(s: &Side) -> Vec<String> {
        s.nodes.iter().filter_map(|n| n.act.clone()).collect()
    }

    fn tap(oit: &mut Oit, b: &mut Fake, side: usize, act: &str) {
        let a = Act::parse(act).unwrap();
        oit.sides[side].click(b, &a);
        oit.tick(b, [true, true], 0.05);
    }

    #[test]
    fn the_display_unit_boots_through_self_test() {
        let mut du = DisplayUnit::new(true);
        du.update(true, true, false, 2.0, 0.05);
        assert_eq!(du.state, Du::Bootup);
        assert!(du.self_test_visible());
        du.update(true, true, false, 2.0, 0.4);
        assert_eq!(du.state, Du::Selftest);
        du.update(true, true, false, 2.0, 1.0);
        assert_eq!(du.state, Du::Selftest);
        du.update(true, true, false, 2.0, 1.1);
        assert_eq!(du.state, Du::On);
        du.update(false, true, false, 2.0, 0.1);
        assert_eq!(du.state, Du::Standby);
        du.update(true, true, false, 2.0, 0.1);
        assert_eq!(du.state, Du::On);
        du.update(true, false, false, 2.0, 0.1);
        du.update(true, false, false, 2.0, 10.0);
        assert_eq!(du.state, Du::Off);
        du.update(true, true, false, 2.0, 0.1);
        du.update(true, false, false, 2.0, 0.1);
        assert_eq!(du.state, Du::Off);
    }

    #[test]
    fn navigation_keeps_back_and_forward_stacks() {
        let mut n = Nav::default();
        assert!(n.navigate("flt-ops"));
        assert!(!n.navigate("flt-ops"));
        n.navigate("flt-ops/sts");
        n.navigate("flt-ops/aoc");
        assert!(n.navigate("back"));
        assert_eq!(n.uri, "flt-ops/sts");
        assert!(n.can_forward());
        assert!(n.navigate("forward"));
        assert_eq!(n.uri, "flt-ops/aoc");
        n.navigate("back");
        n.navigate("flt-ops/charts");
        assert!(!n.can_forward());
        n.reset("flt-ops");
        assert!(!n.can_back());
        assert!(!n.navigate("back"));
    }

    #[test]
    fn loading_counts_down_at_four_hertz() {
        let mut l = Loading::default();
        l.update(true, 4.0, 0.0);
        assert!(!l.hidden());
        assert_eq!(l.width(), "0%");
        l.update(true, 4.0, 1.0);
        assert_eq!(l.width(), "10%");
        l.update(true, 4.0, 3.0);
        assert!(l.hidden());
        l.update(false, 4.0, 0.1);
        assert_eq!(l.width(), "NaN%");
    }

    #[test]
    fn power_up_shows_self_test_then_login_then_menu() {
        let mut b = powered();
        let mut oit = Oit::new(true);
        oit.tick(&mut b, [true, false], 0.05);
        assert!(oit.sides[0].model.self_test_visible);
        assert!(!oit.sides[0].model.oit_visible);
        run(&mut oit, &mut b, 6.0);
        let s = &oit.sides[0];
        assert!(s.model.oit_visible);
        assert!(!s.model.flt_ops.login_hidden);
        assert!(s.html.contains("PILOT"));
        assert!(acts(s).contains(&"F|b:PILOT#0".to_string()));
        tap(&mut oit, &mut b, 0, "F|b:PILOT#0");
        assert!(oit.sides[0].model.flt_ops.login_hidden);
        assert!(matches!(oit.sides[0].model.flt_ops.page, FltOpsPage::Menu));
    }

    #[test]
    fn the_domain_switch_and_a_lost_laptop() {
        let (mut oit, mut b) = ready();
        tap(&mut oit, &mut b, 0, "F|b:PILOT#0");
        tap(&mut oit, &mut b, 0, "F|b:T.O PERF#0");
        assert_eq!(oit.sides[0].flt.uri, "flt-ops/to-perf");
        b.vars.insert("L:A380X_SWITCH_OIT_SIDE_LEFT".into(), 1.0);
        oit.tick(&mut b, [true, true], 0.05);
        assert!(oit.sides[0].model.flt_ops.hidden);
        assert!(!oit.sides[0].model.avncs.hidden);
        b.vars.insert("L:A380X_SWITCH_OIT_SIDE_LEFT".into(), 0.0);
        b.vars.insert("L:A32NX_FLTOPS_LAPTOP_1_IS_HEALTHY".into(), 0.0);
        oit.tick(&mut b, [true, true], 0.05);
        assert_eq!(oit.sides[0].flt.uri, "flt-ops");
        assert!(!oit.sides[0].model.flt_ops.login_hidden);
        assert!(oit.sides[0].model.flt_ops.content_hidden);
    }

    #[test]
    fn menus_open_select_and_close() {
        let (mut oit, mut b) = ready();
        tap(&mut oit, &mut b, 1, "F|b:PILOT#0");
        tap(&mut oit, &mut b, 1, "F|ps:FO_OIT_menu_menu#0");
        assert!(oit.sides[1].model.flt_ops.menu_opened);
        tap(&mut oit, &mut b, 1, "F|mi:FO_OIT_menu_menu_9#0");
        assert!(!oit.sides[1].model.flt_ops.menu_opened);
        assert_eq!(oit.sides[1].flt.uri, "flt-ops/sts");
        tap(&mut oit, &mut b, 1, "F|mi:FO_OIT_menu_functions_1#0");
        assert_eq!(oit.sides[1].flt.uri, "flt-ops");
        tap(&mut oit, &mut b, 1, "F|mi:FO_OIT_menu_functions_2#0");
        assert_eq!(oit.sides[1].flt.uri, "flt-ops/sts");
        tap(&mut oit, &mut b, 1, "F|mi:FO_OIT_menu_functions_9#0");
        assert!(oit.sides[1].switching_bar_hidden);
        tap(&mut oit, &mut b, 1, "F|mi:FO_OIT_menu_menu_12#0");
        assert!(!oit.sides[1].model.flt_ops.login_hidden);
    }

    #[test]
    fn status_fields_take_typing_and_synchro() {
        let (mut oit, mut b) = ready();
        b.fms = FmsInfo { flight_number: Some("FBW123".into()), origin: Some("EGLL".into()), destination: Some("KJFK".into()), ..Default::default() };
        tap(&mut oit, &mut b, 0, "F|b:PILOT#0");
        tap(&mut oit, &mut b, 0, "F|b:FLT OPS STS#0");
        let a = acts(&oit.sides[0]);
        assert!(a.contains(&"F|f:#0".to_string()) && a.contains(&"F|f:#2".to_string()), "{a:?}");
        tap(&mut oit, &mut b, 0, "F|f:#1");
        assert!(oit.sides[0].keyboard());
        for c in "lfpg".chars() {
            oit.sides[0].key(&mut b, Key::Char(c));
        }
        oit.tick(&mut b, [true, true], 0.05);
        let FltOpsPage::Sts(m) = &oit.sides[0].model.flt_ops.page else { panic!() };
        assert!(m.from_airport.editing);
        assert_eq!(m.from_airport.text, "LFPG");
        oit.sides[0].key(&mut b, Key::Enter);
        oit.tick(&mut b, [true, true], 0.05);
        let FltOpsPage::Sts(m) = &oit.sides[0].model.flt_ops.page else { panic!() };
        assert_eq!(m.from_airport.text, "LFPG");
        assert!(!m.from_airport.mandatory);
        assert!(m.flt_number.mandatory);
        assert!(!oit.sides[0].keyboard());
        tap(&mut oit, &mut b, 0, "F|b:SYNCHRO AVIONICS#0");
        let FltOpsPage::Sts(m) = &oit.sides[0].model.flt_ops.page else { panic!() };
        assert_eq!((m.flt_number.text.as_str(), m.from_airport.text.as_str(), m.to_airport.text.as_str()), ("FBW123", "EGLL", "KJFK"));
        tap(&mut oit, &mut b, 0, "F|f:#0");
        oit.sides[0].key(&mut b, Key::Char('x'));
        tap(&mut oit, &mut b, 0, "F|b:CHARTS#0");
        assert_eq!(oit.sides[0].laptop.flt.as_deref(), Some("X"));
    }

    #[test]
    fn takeoff_perf_loads_runways_selects_and_computes() {
        let (mut oit, mut b) = ready();
        b.fms = FmsInfo { origin: Some("EGLL".into()), departure_runway: Some("EGLL27R".into()), ..Default::default() };
        b.airports.insert(
            "EGLL".into(),
            r#"{"runways":[{"designation":"9-27","designatorCharPrimary":1,"designatorCharSecondary":2,"length":3900.0,"direction":90.0,"width":50.0,"primaryElevation":24.0,"secondaryElevation":24.0}]}"#.into(),
        );
        tap(&mut oit, &mut b, 0, "F|b:PILOT#0");
        tap(&mut oit, &mut b, 0, "F|b:T.O PERF#0");
        oit.tick(&mut b, [true, true], 0.05);
        let FltOpsPage::ToPerf(m) = &oit.sides[0].model.flt_ops.page else { panic!() };
        assert_eq!(m.airport.text, "EGLL");
        assert_eq!(m.runway_dropdown.values, ["09L", "27R"]);
        assert_eq!(m.runway.as_ref().map(|r| r.ident.as_str()), Some("27R"));
        tap(&mut oit, &mut b, 0, "F|dd:CAPT_OIT_toPerfFlaps#0");
        assert!(oit.sides[0].dd_open("toPerfFlaps"));
        tap(&mut oit, &mut b, 0, "F|mi:CAPT_OIT_toPerfFlaps_2#0");
        assert!(oit.sides[0].open_dd.is_none());
        tap(&mut oit, &mut b, 0, "F|b:COMPUTE#0");
        let FltOpsPage::ToPerf(m) = &oit.sides[0].model.flt_ops.page else { panic!() };
        assert_eq!(m.flaps.text, "2");
        assert!(m.compute.as_ref().unwrap().pressed);
        assert!(b.requests.iter().any(|r| r == "GET /perf/cal?"));
    }

    #[test]
    fn aoc_folders_compose_and_read() {
        let (mut oit, mut b) = ready();
        tap(&mut oit, &mut b, 0, "F|b:PILOT#0");
        tap(&mut oit, &mut b, 0, "F|b:AOC#0");
        tap(&mut oit, &mut b, 0, "F|nf:REQUESTS#0");
        assert!(acts(&oit.sides[0]).contains(&"F|t:#0".to_string()));
        tap(&mut oit, &mut b, 0, "F|t:#0");
        for c in "egll".chars() {
            oit.sides[0].key(&mut b, Key::Char(c));
        }
        tap(&mut oit, &mut b, 0, "F|b:SEND#0");
        assert_eq!(oit.sides[0].aoc.messages[0].title, "METAR EGLL");
        tap(&mut oit, &mut b, 0, "F|nf:SENT#0");
        tap(&mut oit, &mut b, 0, "F|row:#0");
        let FltOpsPage::Aoc(m) = &oit.sides[0].model.flt_ops.page else { panic!() };
        assert_eq!(m.open.as_ref().map(|o| o.lines[0].as_str()), Some("METAR REQUEST FOR EGLL SENT"));
        assert!(!m.messages[0].is_new);
        assert_eq!(oit.sides[0].aoc.messages[0].zulu, "1230");
    }

    #[test]
    fn avncs_login_navigation_and_inbox() {
        let (mut oit, mut b) = ready();
        b.vars.insert("L:A380X_SWITCH_OIT_SIDE_RIGHT".into(), 1.0);
        oit.sides[1].aoc.add("INBOX", "HELLO", "OPS", vec!["LINE".into()], 0.0);
        oit.tick(&mut b, [true, true], 0.05);
        assert!(acts(&oit.sides[1]).contains(&"A|b:PILOT#0".to_string()));
        tap(&mut oit, &mut b, 1, "A|b:PILOT#0");
        assert!(oit.sides[1].model.avncs.login_hidden);
        assert!(oit.sides[1].model.avncs.back_disabled);
        tap(&mut oit, &mut b, 1, "A|nf:COMPANY COM#0");
        assert!(!oit.sides[1].model.avncs.back_disabled);
        let inbox = acts(&oit.sides[1]).into_iter().find(|a| a.starts_with("A|nf:Inbox")).unwrap();
        tap(&mut oit, &mut b, 1, &inbox);
        let AvncsPage::CompanyCom(c) = &oit.sides[1].model.avncs.page else { panic!() };
        let CompanyComContent::Inbox(i) = &c.content else { panic!() };
        assert_eq!(i.selected, Some(0));
        assert!(!oit.sides[1].aoc.messages[0].is_new);
        tap(&mut oit, &mut b, 1, "A|nf:Flight Log#1");
        assert_eq!(oit.sides[1].avn.uri, "nss-avncs/company-com/in-flight/flight-log");
        tap(&mut oit, &mut b, 1, "A|b:BACK#0");
        assert_eq!(oit.sides[1].avn.uri, "nss-avncs/company-com/inbox");
        tap(&mut oit, &mut b, 1, "A|b:HOME#0");
        tap(&mut oit, &mut b, 1, "A|b:EXIT OIS#0");
        assert!(!oit.sides[1].model.avncs.login_hidden);
    }

    #[test]
    fn remote_cb_search_select_and_command() {
        let (mut oit, mut b) = ready();
        b.vars.insert("L:A380X_SWITCH_OIT_SIDE_LEFT".into(), 1.0);
        b.vars.insert(REMOTE_CB_CTL.into(), 1.0);
        b.vars.insert("L:A32NX_BKR_7_STATUS".into(), 2.0);
        b.maintenance = json!({"uri":"x","content":{"kind":"none","breakers":[{"id":"7","name":"Fuel pump","label":"FP","bus":"DC1","ata":28,"rating_a":5.0,"state":0}]}});
        tap(&mut oit, &mut b, 0, "A|b:PILOT#0");
        tap(&mut oit, &mut b, 0, "A|nf:MAINTENANCE#0");
        tap(&mut oit, &mut b, 0, "A|nf:Remote C/B Control#0");
        oit.tick(&mut b, [true, true], 1.1);
        assert!(acts(&oit.sides[0]).contains(&"A|cb:#0".to_string()), "{:?}", acts(&oit.sides[0]));
        tap(&mut oit, &mut b, 0, "A|cb:#0");
        assert_eq!(oit.sides[0].cb.selected.as_deref(), Some("7"));
        tap(&mut oit, &mut b, 0, "A|b:CLOSE / RESET#0");
        assert_eq!(b.writes.last(), Some(&("L:A32NX_BKR_7_CMD".to_string(), 2.0)));
        tap(&mut oit, &mut b, 0, "A|f:#0");
        oit.sides[0].key(&mut b, Key::Char('z'));
        assert_eq!(oit.sides[0].cb.search.as_deref(), Some("Z"));
    }

    #[test]
    fn flt_folder_fetches_the_ofp_and_reports_a_missing_id() {
        let (mut oit, mut b) = ready();
        tap(&mut oit, &mut b, 0, "F|b:PILOT#0");
        tap(&mut oit, &mut b, 0, "F|b:FLT FOLDER#0");
        let FltOpsPage::FltFolder(m) = &oit.sides[0].model.flt_ops.page else { panic!() };
        assert_eq!(m.message.as_ref().map(|x| (x.text.as_str(), x.warn)), Some(("SET YOUR SIMBRIEF ID IN THE EFB SETTINGS", true)));
        b.settings.insert("CONFIG_OVERRIDE_SIMBRIEF_USERID".into(), "42".into());
        let url = "https://www.simbrief.com/api/xml.fetcher.php?userid=42&json=1";
        b.answers.insert(url.into(), (200, r#"{"fetch":{"status":"Success"},"origin":{"icao_code":"EGLL"},"destination":{"icao_code":"KJFK"},"text":{"plan_html":"<div x>HEAD<!--BKMK///Fuel///0-->F</div>"}}"#.into()));
        tap(&mut oit, &mut b, 0, "F|ob:FETCH OFP#0");
        oit.tick(&mut b, [true, true], 0.05);
        let FltOpsPage::FltFolder(m) = &oit.sides[0].model.flt_ops.page else { panic!() };
        assert!(m.message.is_none());
        assert_eq!(m.ofp.as_ref().map(|o| o.sections.len()), Some(2));
        tap(&mut oit, &mut b, 0, "F|oi:#1");
        assert_eq!(oit.sides[0].folder.selected, Some(1));
    }

    #[test]
    fn charts_type_an_icao_and_load_the_index() {
        let (mut oit, mut b) = ready();
        b.answers.insert("GET /oit/charts?source=local&icao=EGLL".into(), (200, r#"[{"id":"a","name":"Gate","category":"apt","kind":"image","url":"/oit/file?path=a.png"}]"#.into()));
        tap(&mut oit, &mut b, 0, "F|b:PILOT#0");
        tap(&mut oit, &mut b, 0, "F|b:CHARTS#0");
        tap(&mut oit, &mut b, 0, "F|os:1:2#0");
        tap(&mut oit, &mut b, 0, "F|icao:#0");
        for c in "egll".chars() {
            oit.sides[0].key(&mut b, Key::Char(c));
        }
        oit.sides[0].key(&mut b, Key::Enter);
        oit.tick(&mut b, [true, true], 0.05);
        oit.tick(&mut b, [true, true], 0.05);
        let FltOpsPage::Charts(m) = &oit.sides[0].model.flt_ops.page else { panic!() };
        assert_eq!(m.status, "MY CHARTS - EGLL - 1 CHARTS");
        assert!(matches!(m.stage, ChartStage::Image { .. }));
        assert!(oit.sides[0].html.contains("a.png"));
        tap(&mut oit, &mut b, 0, "F|ob:ROTATE#0");
        assert_eq!(oit.sides[0].charts.view.rotation, 90.0);
    }

    #[test]
    fn inflight_steppers_and_loadsheet_to_aoc() {
        let (mut oit, mut b) = ready();
        tap(&mut oit, &mut b, 0, "F|b:PILOT#0");
        tap(&mut oit, &mut b, 0, "F|b:IN-FLT PERF#0");
        tap(&mut oit, &mut b, 0, "F|b:LIVE#0");
        tap(&mut oit, &mut b, 0, "F|b:−#0");
        assert_eq!(oit.sides[0].inflt.weight_kg, 445000.0);
        tap(&mut oit, &mut b, 0, "F|b:+#2");
        assert_eq!(oit.sides[0].inflt.mach, 0.86);
        tap(&mut oit, &mut b, 0, "F|b:OEI#0");
        assert_eq!(oit.sides[0].inflt.engines, 3);
        b.vars.insert("L:A32NX_AIRFRAME_ZFW".into(), 300000.0);
        b.vars.insert("L:A32NX_AIRFRAME_GW".into(), 400000.0);
        tap(&mut oit, &mut b, 0, "F|b:LOADSHEET#0");
        tap(&mut oit, &mut b, 0, "F|b:SEND TO AOC#0");
        assert_eq!(oit.sides[0].aoc.messages[0].folder, "LOADSHEET");
    }

    #[test]
    fn a_hit_on_the_published_document_acts() {
        let (mut oit, mut b) = ready();
        let s = &mut oit.sides[0];
        let i = s.nodes.iter().position(|n| n.act.as_deref() == Some("F|b:PILOT#0")).unwrap();
        let mut boxes = vec![Rect::default(); s.nodes.len()];
        let mut up = Some(i);
        while let Some(a) = up {
            boxes[a] = Rect::new(0.0, 0.0, 1333.0, 1000.0);
            up = s.nodes[a].parent;
        }
        boxes[i] = Rect::new(100.0, 100.0, 200.0, 60.0);
        for n in s.nodes.iter_mut() {
            n.hidden = false;
        }
        s.boxes = boxes;
        assert!(s.pointer(&mut b, 150.0, 120.0));
        assert!(!s.flt_login);
    }

    #[test]
    fn keys_map_and_topics_name_sides() {
        assert_eq!(key_of(0, 0x0D), Some(Key::Enter));
        assert_eq!(key_of('a' as u32, 0x41), Some(Key::Char('a')));
        assert_eq!(key_of(0, 0x70), None);
        assert_eq!(side_of_topic("a380.oit.dom.2"), Some(1));
        assert_eq!(side_of_topic("a380.mfd.dom"), None);
    }

    #[test]
    fn unfocused_keys_pass_through() {
        let (mut oit, mut b) = ready();
        assert!(!oit.sides[0].key(&mut b, Key::Char('a')));
    }

    #[test]
    fn identical_documents_compare_clean() {
        let (mut oit, _b) = ready();
        let html = oit.sides[0].html.clone();
        assert!(oit.compare(0, &html).is_none());
        assert!(oit.compare(0, "<body></body>").is_some());
    }

    #[test]
    fn navdb_runways_become_a_facility() {
        let rw = |n: i32, d: i32, b: f64, alt: f64| fms::navdb::Runway {
            fix: fms::navdb::Fix { ident: format!("EGLL{n:02}"), location: fms::navdb::Location { lat: 0.0, long: 0.0, alt: Some(alt) }, ..Default::default() },
            bearing: b,
            magnetic_bearing: b,
            length: 3000.0,
            number: n,
            designator: d,
            threshold_location: fms::navdb::Location { lat: 0.0, long: 0.0, alt: Some(alt) },
        };
        let v: Value = serde_json::from_str(&facility_json(&[rw(9, 1, 90.0, 100.0), rw(27, 2, 270.0, 110.0)])).unwrap();
        let ends = data::runway_ends(&v);
        assert_eq!(ends.iter().map(|r| r.ident.as_str()).collect::<Vec<_>>(), ["09L", "27R"]);
        assert!(ends[0].slope_percent > 0.0);
        assert!((ends[0].elevation_ft - 100.0).abs() < 1e-6);
    }
}

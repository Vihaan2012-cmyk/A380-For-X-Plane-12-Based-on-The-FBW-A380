use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use super::data::Source;

use efb_pages::docs::{DocsData, DocsState};
use efb_pages::flight::{flight_plan_content, FlightPlanView};
use efb_pages::home::home_page;
use efb_pages::model::{ChecklistRow, EfbModel, Landing, Maintenance, StatusModel};
use efb_pages::settings::{settings_page, Item, SettingsTable, CALLOUTS};
use mfd_tree::{Kid, Node, Shape};
use serde_json::{json, Map, Value};

use super::doc::{Built, NodeInfo, Styler};

/// Styles EFB pages on their own thread: a page with many elements takes tens of
/// milliseconds to cascade, too long for the flight loop.
struct StyleWorker {
    work: std::sync::mpsc::Sender<(u64, Node, String)>,
    done: std::sync::mpsc::Receiver<(u64, Built)>,
}

impl StyleWorker {
    fn spawn() -> Option<StyleWorker> {
        let (work, rx) = std::sync::mpsc::channel::<(u64, Node, String)>();
        let (tx, done) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("fbw-efb-style".into())
            .spawn(move || {
                let styler = Styler::new(&[]);
                while let Ok(mut job) = rx.recv() {
                    while let Ok(newer) = rx.try_recv() {
                        job = newer;
                    }
                    let (gen, body, tab) = job;
                    let started = std::time::Instant::now();
                    let built = styler.build(&body, &tab);
                    let ms = started.elapsed().as_secs_f64() * 1e3;
                    if ms >= 15.0 {
                        crate::log(&format!("efb: slow page styling {ms:.1} ms (off the flight loop), tab {tab}, {} elements", built.nodes.len()));
                    }
                    if tx.send((gen, built)).is_err() {
                        return;
                    }
                }
            })
            .ok()?;
        Some(StyleWorker { work, done })
    }
}
use super::hit::{self, Rect};
use super::scroll::{Scrolls, Tracker, DEAD_ZONE_PX, WHEEL_STEP_PX};
use super::text::{self, Key, Outcome, TextEdit};

/// Everything the EFB reads from or does to the rest of the plugin. The
/// plugin implements it with the functions its HTTP routes call
/// (`host::PluginBackend`); tests use an in-memory one.
pub trait Backend {
    fn load_settings(&mut self) -> Map<String, Value>;
    fn save_settings(&mut self, values: &Map<String, Value>) -> Result<(), String>;
    /// The plugin route `method path` with `body`, answered in-process.
    fn route(&mut self, method: &str, path: &str, body: &str) -> Option<(u16, String)>;
    /// An app action (`restart-all`, `open-logs`, ...); the message to show.
    fn app_action(&mut self, name: &str) -> Option<String>;
    /// GET `url` without blocking: `None` until the answer is in.
    fn fetch(&mut self, url: &str) -> Option<(u16, String)>;
    fn status(&mut self) -> StatusModel;
    fn clipboard_get(&mut self) -> String;
    fn clipboard_set(&mut self, text: &str);
    /// Take X-Plane's keyboard while a text field has focus.
    fn keyboard_capture(&mut self, on: bool);
}

pub const SECTIONS: &[(&str, &str, &[(&str, &str)])] = &[
    ("home", "Home", &[("Home", "home")]),
    ("flight", "Flight", &[("Flight Plan", "flightplan"), ("Payload & Fuel", "loadsheet"), ("Cargo", "cargo"), ("Performance", "performance"), ("Charts", "charts"), ("ATC", "atc")]),
    ("checklists", "Documents", &[("Documents", "checklists")]),
    ("ground", "Ground", &[("Ground Services", "ground"), ("Pushback", "pushback")]),
    ("cabin", "Cabin", &[("Flight Attendant Panel", "fap")]),
    ("aircraft", "Systems", &[("Systems", "study")]),
    ("components", "Components", &[("Components", "components")]),
    ("wear", "Wear & Tear", &[("Wear & Tear", "wear")]),
    ("maint", "Maintenance", &[("MEL & Maintenance", "maintenance")]),
    ("breakers", "Breakers", &[("Circuit Breakers", "breakers")]),
    ("failures", "Failures", &[("Failures", "failures")]),
    ("settings", "Settings", &[("Settings", "settings"), ("Saves", "saves"), ("Throttle", "throttle"), ("Status", "status"), ("Diagnostics", "diagnostics"), ("Support", "support")]),
];

const TABS: &[(&str, &str)] = &[
    ("home", "Home"), ("study", "Study"), ("failures", "Failures"), ("breakers", "Circuit Breakers"), ("maintenance", "MEL & Maintenance"),
    ("components", "Components"), ("wear", "Wear & Tear"), ("flightplan", "Flight Plan"), ("ground", "Ground Services"), ("pushback", "Pushback"),
    ("fap", "FAP"), ("charts", "Charts"), ("loadsheet", "Payload & Fuel"), ("cargo", "Cargo"), ("atc", "ATC"), ("performance", "Performance"),
    ("checklists", "Documents"), ("throttle", "Throttle"), ("saves", "Saves"), ("settings", "Settings"), ("status", "Status"), ("support", "Support"),
    ("diagnostics", "Diagnostics"),
];

const ICONS: &[(&str, &[&str])] = &[
    ("home", &["M4 11.5 12 5l8 6.5", "M6.5 10v9h11v-9"]),
    ("flight", &["M21 14v-2l-8-5V3.5a1.3 1.3 0 0 0-2.6 0V7l-8 5v2l8-2.5V16l-2 1.4v1.6l3.3-1 3.3 1v-1.6L13 16v-4.5z"]),
    ("checklists", &["M4 5.5A2.5 2.5 0 0 1 6.5 3H20v15H6.5A2.5 2.5 0 0 0 4 20.5z", "M4 20.5A2.5 2.5 0 0 0 6.5 23H20v-5", "M8 7.5h8M8 11h6"]),
    ("ground", &["M3 10.5h12v6H3z", "M15 12.5h3l2.5 2.5v1.5H15"]),
    ("cabin", &["M4 18V9a3 3 0 0 1 3-3h10a3 3 0 0 1 3 3v9", "M4 18h16", "M8 6v12M12 6v12M16 6v12"]),
    ("aircraft", &["M3 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0", "M12 7v5l3.4 2"]),
    ("components", &["M6.5 6.5h11v11h-11z", "M9.5 3v3.5M14.5 3v3.5M9.5 17.5V21M14.5 17.5V21M3 9.5h3.5M3 14.5h3.5M17.5 9.5H21M17.5 14.5H21"]),
    ("wear", &["M4.5 16.5a8 8 0 1 1 15 0", "M12 13.5l4-4.5"]),
    ("maint", &["M14.7 6.3a4 4 0 0 0-5.4 5.2L4 16.8V20h3.2l5.3-5.3a4 4 0 0 0 5.2-5.4l-2.6 2.6-2.4-.6-.6-2.4z"]),
    ("breakers", &["M5 3.5h14v17H5z", "M9 8h6M9 12h6M9 16h6"]),
    ("failures", &["M12 3 2.5 20h19z", "M12 10v4.5", "M12 17.4v.1"]),
    ("settings", &["M15 12a3 3 0 1 1-6 0a3 3 0 1 1 6 0", "M12 2.8v2.4M12 18.8v2.4M2.8 12h2.4M18.8 12h2.4M5.3 5.3l1.7 1.7M17 17l1.7 1.7M5.3 18.7l1.7-1.7M17 7l1.7-1.7"]),
];

const DROPDOWN_ID: &str = "efbn-dropdown";
const MENU_W: f64 = 190.0;
const MENU_H: f64 = 96.0;
const ZOOMS: &[(&str, &str)] = &[("0.9", "Small"), ("1", "Normal"), ("1.15", "Large"), ("1.3", "Larger"), ("1.5", "Largest")];
const INTERVALS: &[(&str, &str)] = &[("0", "Off"), ("2", "Every 2 min"), ("5", "Every 5 min"), ("10", "Every 10 min"), ("15", "Every 15 min"), ("30", "Every 30 min")];
const ARM_MS: f64 = 4000.0;
const POLL_MS: f64 = 1000.0;
const FAIL_WINDOW_PX: f64 = 1600.0;

#[derive(Clone, Debug, PartialEq)]
pub enum Field {
    Setting(String),
    Search,
    FpUser,
    SavesName,
    Modal,
    Efb(String),
    FailSearch,
}

impl Field {
    fn parse(s: &str) -> Option<Field> {
        Some(match s {
            "search" => Field::Search,
            "fp-user" => Field::FpUser,
            "saves-name" => Field::SavesName,
            "modal" => Field::Modal,
            "fail-search" => Field::FailSearch,
            _ if s.starts_with("payload/") || s.starts_with("refuel/") => Field::Efb(s.to_string()),
            _ => Field::Setting(s.strip_prefix("set:")?.to_string()),
        })
    }
}

#[derive(Clone, Debug)]
struct Dropdown {
    target: String,
    options: Vec<(String, String)>,
    anchor: Option<Rect>,
}

#[derive(Clone, Debug)]
struct Modal {
    title: String,
    prompt: bool,
    ok: String,
}

#[derive(Clone, Debug)]
enum Gesture {
    Press,
    Scroll(String),
    Range(String, usize),
    Pad(usize),
}

pub const PB_VARS: &[&str] = &["PLANE HEADING DEGREES TRUE", "PLANE LATITUDE", "PLANE LONGITUDE", "GROUND VELOCITY", "PUSHBACK ATTACHED", "A32NX_PARK_BRAKE_LEVER_POS"];

#[derive(Clone, Debug)]
struct PushStep {
    turn: bool,
    dist: f64,
    angle: f64,
    tail_left: bool,
    speed: f64,
}

#[derive(Clone, Debug)]
struct AutoPush {
    steps: Vec<PushStep>,
    i: usize,
    hdg: f64,
    pos: (f64, f64),
}

fn pb_dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (p1, p2) = (a.0.to_radians(), b.0.to_radians());
    let (dp, dl) = (p2 - p1, (b.1 - a.1).to_radians());
    let h = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * 6_371_000.0 * h.sqrt().asin()
}

fn pb_turned(from: f64, to: f64) -> f64 {
    let mut d = to - from;
    while d > 180.0 {
        d -= 360.0;
    }
    while d < -180.0 {
        d += 360.0;
    }
    d
}

#[derive(Clone, Debug)]
struct Press {
    x: f64,
    y: f64,
    last_y: f64,
    act: Option<String>,
    scroller: Option<String>,
    gesture: Gesture,
    track: Tracker,
}

pub struct Efb<B: Backend> {
    pub backend: B,
    styler: Styler,
    table: SettingsTable,
    docs: DocsData,
    pub docs_state: DocsState,
    raw: Map<String, Value>,
    pub tab: String,
    category: usize,
    search: String,
    compact: bool,
    focus: Option<Field>,
    edit: TextEdit,
    kb: bool,
    kb_shift: bool,
    kb_sym: bool,
    dropdown: Option<Dropdown>,
    menu: Option<(String, Option<Rect>)>,
    toast: Option<(String, f64)>,
    sleep: bool,
    pub locked: bool,
    armed: Option<(String, f64)>,
    fp_user: String,
    fp_state: String,
    ofp: Option<Value>,
    fp_pending: Option<String>,
    saves: Option<Value>,
    saves_name: String,
    modal: Option<Modal>,
    pub scrolls: Scrolls,
    press: Option<Press>,
    boxes: Vec<Rect>,
    built: Option<Built>,
    last_body: Option<Node>,
    styling: Option<StyleWorker>,
    style_gen: u64,
    style_pending: bool,
    fail_built_top: f64,
    dirty: bool,
    now_ms: f64,
    last_poll: f64,
    status: StatusModel,
    landings: Vec<Landing>,
    maintenance: Option<Maintenance>,
    pub bg_url: String,
    pub zoom: String,
    pub clock: (String, String),
    chrono: (bool, f64, f64),
    fail_view: efb_pages::failures::FailuresView,
    cb_view: efb_pages::maint::ListView,
    comp_view: efb_pages::maint::ListView,
    study_view: efb_pages::study::StudyView,
    pub study_vars: efb_pages::jv::Vars,
    pb: efb_pages::pushback::PbView,
    pb_auto: Option<AutoPush>,
    pb_sent: String,
    gc: super::ground_eng::GroundEngineer,
    fap: efb_pages::fap::FapView,
    pub ground_vars: efb_pages::jv::Vars,
    gse: HashMap<String, f64>,
    lp_armed: u32,
}

fn jstr(v: &Value) -> String {
    efb_pages::model::jstr(v)
}

impl<B: Backend> Efb<B> {
    pub fn new(mut backend: B) -> Self {
        let raw = backend.load_settings();
        let fp_user = raw.get("CONFIG_OVERRIDE_SIMBRIEF_USERID").map(jstr).unwrap_or_default();
        Efb {
            backend,
            styler: Styler::new(&[]),
            table: SettingsTable::bundled(),
            docs: DocsData::bundled(),
            docs_state: DocsState::default(),
            raw,
            tab: "home".into(),
            category: 0,
            search: String::new(),
            compact: false,
            focus: None,
            edit: TextEdit::default(),
            kb: false,
            kb_shift: false,
            kb_sym: false,
            dropdown: None,
            menu: None,
            toast: None,
            sleep: false,
            locked: true,
            armed: None,
            fp_user,
            fp_state: String::new(),
            ofp: None,
            fp_pending: None,
            saves: None,
            saves_name: String::new(),
            modal: None,
            scrolls: Scrolls::default(),
            press: None,
            boxes: Vec::new(),
            built: None,
            last_body: None,
            styling: if cfg!(test) { None } else { StyleWorker::spawn() },
            style_gen: 0,
            style_pending: false,
            fail_built_top: 0.0,
            dirty: true,
            now_ms: 0.0,
            last_poll: f64::NEG_INFINITY,
            status: StatusModel::default(),
            landings: Vec::new(),
            maintenance: None,
            bg_url: crate::js_bridge::plugin_dir().map(|d| efb_pages::bg::Backgrounds::load(&d.join("bg")).url("gate")).unwrap_or_else(|| efb_pages::bg::asset_path("gate")),
            zoom: "1".into(),
            clock: ("--:--Z".into(), "--:--".into()),
            chrono: (false, 0.0, 0.0),
            fail_view: Default::default(),
            cb_view: Default::default(),
            comp_view: Default::default(),
            study_view: Default::default(),
            study_vars: Default::default(),
            pb: Default::default(),
            pb_auto: None,
            pb_sent: String::new(),
            gc: Default::default(),
            fap: Default::default(),
            ground_vars: Default::default(),
            gse: HashMap::new(),
            lp_armed: 0,
        }
    }

    // ---- model ----

    fn setting(&self, key: &str) -> String {
        self.raw.get(key).map(jstr).or_else(|| self.table.defaults.get(key).cloned()).unwrap_or_default()
    }

    fn item(&self, key: &str) -> Option<&Item> {
        self.table.categories.iter().flat_map(|c| c.items.iter()).find(|i| i.key.as_deref() == Some(key))
    }

    pub fn model(&self) -> EfbModel {
        let settings: BTreeMap<String, String> = self.raw.iter().map(|(k, v)| (k.clone(), jstr(v))).collect();
        let livery_values = serde_json::from_str::<Map<String, Value>>(&self.setting("xphfbw.liveryValues")).unwrap_or_default().iter().map(|(k, v)| (k.clone(), jstr(v))).collect();
        let session_keys = serde_json::from_str(&self.setting("xphfbw.sessionKeys")).unwrap_or_default();
        let mut checklists = Vec::new();
        for (i, c) in self.docs.normal.iter().enumerate() {
            let items: Vec<usize> = c.i.iter().enumerate().filter(|(_, l)| !l.3.contains(['c', 'h', 'b'])).map(|(j, _)| j).collect();
            if items.is_empty() {
                continue;
            }
            let ticks = self.docs_state.ticks.get(&format!("n:{i}"));
            let done = items.iter().filter(|j| ticks.is_some_and(|t| t.contains(j))).count();
            checklists.push(ChecklistRow { title: c.t.clone(), done, total: items.len() });
        }
        let pins = self.docs_state.pins.iter().filter_map(|id| self.docs.normal.iter().find(|c| &c.id == id).map(|c| c.t.clone())).collect();
        EfbModel {
            settings,
            livery_values,
            session_keys,
            category: self.category,
            search: if self.focus == Some(Field::Search) { self.edit.display(self.caret_on()) } else { self.search.clone() },
            compact: self.compact,
            status: self.status.clone(),
            checklists,
            pins,
            ofp: self.ofp.clone(),
            landings: self.landings.clone(),
            maintenance: self.maintenance.clone(),
            offline: false,
        }
    }

    fn caret_on(&self) -> bool {
        (self.now_ms / text::BLINK_MS).floor() as i64 % 2 == 0
    }

    fn show(&mut self, text: &str) {
        let ms = (text.chars().count() as f64 * 60.0).max(2200.0);
        self.toast = Some((text.to_string(), self.now_ms + ms));
        self.dirty = true;
    }

    // ---- frame ----

    /// Advance to `now_ms`: flings, toasts, polls, the SimBrief fetch.
    /// True when the document or the scroll offsets need publishing again.
    pub fn tick(&mut self, now_ms: f64) -> bool {
        let dt = if self.now_ms > 0.0 { now_ms - self.now_ms } else { 0.0 };
        let blink_before = self.caret_on();
        self.now_ms = now_ms;
        let mut changed = self.scrolls.tick(dt);
        if self.tab == "failures" {
            let top = self.failures_scroll_top();
            if (top - self.fail_built_top).abs() > FAIL_WINDOW_PX / 5.0 {
                self.fail_built_top = top;
                self.dirty = true;
            }
        }
        if self.focus.is_some() && self.caret_on() != blink_before {
            self.dirty = true;
        }
        if self.toast.as_ref().is_some_and(|t| now_ms >= t.1) {
            self.toast = None;
            self.dirty = true;
        }
        if self.armed.as_ref().is_some_and(|a| now_ms >= a.1) {
            self.armed = None;
            self.dirty = true;
        }
        if now_ms - self.last_poll >= POLL_MS {
            self.last_poll = now_ms;
            self.poll();
        }
        if let Some(url) = self.fp_pending.clone() {
            if let Some((status, body)) = self.backend.fetch(&url) {
                self.fp_pending = None;
                self.fp_loaded(status, &body);
            }
        }
        if self.chrono.0 {
            self.dirty = true;
        }
        changed |= self.dirty || self.style_pending;
        changed
    }

    fn poll(&mut self) {
        if matches!(self.tab.as_str(), "study" | "failures" | "breakers" | "components" | "maintenance" | "wear" | "loadsheet") {
            self.dirty = true;
        }
        let status = self.backend.status();
        if status.xplane.text != self.status.xplane.text || status.systems.text != self.status.systems.text || status.displays.text != self.status.displays.text {
            self.status = status;
            self.dirty = true;
        }
        if self.tab == "home" {
            if let Some((200, body)) = self.backend.route("GET", "/efb/landings", "") {
                let l: Vec<Landing> = serde_json::from_str(&body).unwrap_or_default();
                if l.len() != self.landings.len() {
                    self.landings = l;
                    self.dirty = true;
                }
            }
            if let Some((200, body)) = self.backend.route("GET", "/study/maintenance", "") {
                let m = maintenance_of(&body);
                if m.as_ref().map(|m| (m.legal, m.defects, m.deferrals, m.tech_log)) != self.maintenance.as_ref().map(|m| (m.legal, m.defects, m.deferrals, m.tech_log)) {
                    self.maintenance = m;
                    self.dirty = true;
                }
            }
        }
        if self.tab == "saves" {
            self.load_saves();
        }
    }

    fn load_saves(&mut self) {
        let data = self.backend.route("GET", "/states/list", "").and_then(|(s, b)| if s == 200 { serde_json::from_str::<Value>(&b).ok() } else { None });
        if data != self.saves {
            self.saves = data;
            self.dirty = true;
        }
    }

    /// The document to publish when anything changed since the last call.
    pub fn document(&mut self) -> Option<&str> {
        if let Some(w) = &self.styling {
            let mut newest = None;
            while let Ok(done) = w.done.try_recv() {
                newest = Some(done);
            }
            if let Some((gen, built)) = newest {
                if gen == self.style_gen {
                    self.style_pending = false;
                    self.built = Some(built);
                    self.boxes.clear();
                    return self.built.as_ref().map(|b| b.html.as_str());
                }
            }
        }
        let have = self.built.is_some() || self.style_pending;
        if !self.dirty && have {
            return None;
        }
        self.dirty = false;
        let started = std::time::Instant::now();
        let body = self.tree();
        if have && self.last_body.as_ref() == Some(&body) {
            return None;
        }
        if let Some(w) = &self.styling {
            self.style_gen += 1;
            if w.work.send((self.style_gen, body.clone(), self.tab.clone())).is_ok() {
                self.style_pending = true;
                self.last_body = Some(body);
                return None;
            }
            self.styling = None;
        }
        let tree_ms = started.elapsed().as_secs_f64() * 1e3;
        let built = self.styler.build(&body, &self.tab);
        let total_ms = started.elapsed().as_secs_f64() * 1e3;
        if total_ms >= 15.0 {
            crate::log(&format!("efb: slow page build {total_ms:.1} ms ({tree_ms:.1} ms page, {:.1} ms styles), tab {}, {} elements", total_ms - tree_ms, self.tab, built.nodes.len()));
        }
        self.built = Some(built);
        self.last_body = Some(body);
        self.boxes.clear();
        self.built.as_ref().map(|b| b.html.as_str())
    }

    pub fn nodes(&self) -> &[NodeInfo] {
        self.built.as_ref().map(|b| b.nodes.as_slice()).unwrap_or(&[])
    }

    /// The display's laid-out boxes for the current document, element order.
    pub fn set_boxes(&mut self, boxes: Vec<Rect>) {
        self.boxes = boxes;
        let nodes = self.nodes().to_vec();
        for (i, n) in nodes.iter().enumerate() {
            if let Some(id) = &n.scroller {
                let max = hit::max_scroll(&nodes, &self.boxes, i);
                self.scrolls.set_max(id, max);
            }
        }
    }

    pub fn scroll_json(&self) -> String {
        self.scrolls.to_json()
    }

    // ---- input ----

    fn hit_at(&self, x: f64, y: f64) -> Option<hit::Hit> {
        let s = &self.scrolls;
        hit::hit(self.nodes(), &self.boxes, &|id| s.top(id), x, y)
    }

    fn rect_of(&self, idx: usize) -> Option<Rect> {
        let s = &self.scrolls;
        let p = hit::place(self.nodes(), &self.boxes, &|id| s.top(id));
        p.rect.get(idx).copied()
    }

    /// Pointer event in CSS pixels: kind 0 down, 1 drag, 2 up, 3 move,
    /// 4 wheel (`button` = clicks, positive away from the user).
    pub fn pointer(&mut self, x: f64, y: f64, kind: i32, button: i32) -> bool {
        match kind {
            0 => {
                let h = self.hit_at(x, y);
                let act = h.as_ref().and_then(|h| h.act.clone());
                let scroller = h.as_ref().and_then(|h| h.scroller.as_ref().map(|s| s.1.clone()));
                if let Some(s) = &scroller {
                    self.scrolls.stop(s);
                }
                let gesture = match &act {
                    Some((i, a)) if a.starts_with("range:") => Gesture::Range(a["range:".len()..].to_string(), *i),
                    Some((i, a)) if a == "pb:pad" => Gesture::Pad(*i),
                    _ => Gesture::Press,
                };
                let mut track = Tracker::default();
                track.push(self.now_ms, y);
                self.press = Some(Press { x, y, last_y: y, act: act.map(|a| a.1), scroller, gesture, track });
                match self.press.clone() {
                    Some(Press { gesture: Gesture::Range(key, i), .. }) => self.range_to(&key, i, x, false),
                    Some(Press { gesture: Gesture::Pad(i), .. }) => self.pad_to(i, x, y),
                    _ => {}
                }
                true
            }
            1 | 3 => {
                let Some(mut p) = self.press.take() else { return false };
                p.track.push(self.now_ms, y);
                match p.gesture.clone() {
                    Gesture::Range(key, i) => self.range_to(&key, i, x, false),
                    Gesture::Pad(i) => self.pad_to(i, x, y),
                    Gesture::Scroll(id) => {
                        if self.scrolls.by(&id, p.last_y - y) {
                            self.scrolled_away(&id);
                        }
                    }
                    Gesture::Press => {
                        if let Some(id) = &p.scroller {
                            if (x - p.x).hypot(y - p.y) > DEAD_ZONE_PX {
                                if self.scrolls.by(id, p.y - y) {
                                    self.scrolled_away(id);
                                }
                                p.gesture = Gesture::Scroll(id.clone());
                            }
                        }
                    }
                }
                p.last_y = y;
                self.press = Some(p);
                true
            }
            2 => {
                let Some(mut p) = self.press.take() else { return false };
                p.track.push(self.now_ms, y);
                match p.gesture {
                    Gesture::Range(key, i) => self.range_to(&key, i, x, true),
                    Gesture::Pad(_) => {
                        self.pb_send(0.0, 0.0);
                        self.dirty = true;
                    }
                    Gesture::Scroll(id) => self.scrolls.fling(&id, p.track.velocity()),
                    Gesture::Press => match p.act {
                        Some(a) => self.activate(&a),
                        None if p.scroller.as_deref() == Some(DROPDOWN_ID) => {}
                        None => self.tap_nothing(),
                    },
                }
                true
            }
            4 => match self.hit_at(x, y).and_then(|h| h.scroller) {
                Some((_, id)) => {
                    if self.scrolls.by(&id, -(button as f64) * WHEEL_STEP_PX) {
                        self.scrolled_away(&id);
                    }
                    true
                }
                None => false,
            },
            _ => false,
        }
    }

    fn range_to(&mut self, key: &str, idx: usize, x: f64, commit: bool) {
        let Some(r) = self.rect_of(idx) else { return };
        if let Some((lo, hi, step)) = match key {
            "SOUND_LEVEL_CABIN" => Some((0.0, 100.0, 5.0)),
            "xphfbw.cabinBrightness" => Some((0.0, 1.0, 0.05)),
            _ => None,
        } {
            let f = ((x - r.x) / r.w.max(1.0)).clamp(0.0, 1.0);
            let v = (lo + ((f * (hi - lo)) / step).round() * step).clamp(lo, hi);
            let text = if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{}", (v * 100.0).round() / 100.0) };
            if commit {
                self.commit_setting(key, Value::String(text));
            } else if self.setting(key) != text {
                self.raw.insert(key.to_string(), Value::String(text));
                self.dirty = true;
            }
            return;
        }
        if key.starts_with("refuel/") || key.starts_with("payload/") {
            let v = (((x - r.x) / r.w.max(1.0)).clamp(0.0, 1.0) * 100.0).round();
            if efb_value(key) != Some(v) {
                self.study_action(json!({ "kind": "efbSet", "name": key, "value": v }));
                self.dirty = true;
            }
            return;
        }
        let Some([lo, hi, step]) = self.item(key).and_then(|i| i.range) else { return };
        let f = ((x - r.x) / r.w.max(1.0)).clamp(0.0, 1.0);
        let mut v = lo + f * (hi - lo);
        if step > 0.0 {
            v = lo + ((v - lo) / step).round() * step;
        }
        let v = v.clamp(lo, hi);
        let text = if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{}", (v * 1e6).round() / 1e6) };
        if commit {
            self.commit_setting(key, Value::String(text));
        } else if self.setting(key) != text {
            self.raw.insert(key.to_string(), Value::String(text));
            self.dirty = true;
        }
    }

    fn pad_to(&mut self, idx: usize, x: f64, y: f64) {
        let Some(r) = self.rect_of(idx) else { return };
        let nx = (((x - r.x) / r.w.max(1.0) - 0.5) / 0.42).clamp(-1.0, 1.0);
        let ny = (-((y - r.y) / r.h.max(1.0) - 0.5) / 0.42).clamp(-1.0, 1.0);
        self.pb_auto = None;
        let speed = if ny.abs() < 0.06 { 0.0 } else { ny * 0.6 };
        let heading = if nx.abs() < 0.06 { 0.0 } else if speed < 0.0 { -nx } else { nx };
        self.pb_send(speed, heading);
        self.dirty = true;
    }

    fn pb_send(&mut self, speed: f64, heading: f64) {
        let (speed, heading) = (speed.clamp(-1.0, 1.0), heading.clamp(-1.0, 1.0));
        self.pb.speed = speed;
        self.pb.heading = heading;
        let key = format!("{speed:.2}|{heading:.2}");
        if key == self.pb_sent {
            return;
        }
        self.pb_sent = key;
        if !self.study_action(json!({ "kind": "pushback", "speed": speed, "heading": heading })) {
            self.pb_sent.clear();
        }
    }

    fn pb_var(&self, k: &str) -> f64 {
        self.pb.vars.get(k).and_then(Value::as_f64).unwrap_or(0.0)
    }

    pub fn fap_wants_vars(&self) -> bool {
        self.tab == "fap"
    }

    pub fn fap_update(&mut self, vars: efb_pages::jv::Vars) {
        if !self.fap.ok || self.fap.vars != vars {
            self.fap.vars = vars;
            self.fap.ok = true;
            self.dirty = true;
        }
    }

    fn fap_act(&mut self, arg: &str) {
        let (what, rest) = arg.split_once(':').unwrap_or((arg, ""));
        match what {
            "page" => self.fap.page = rest.to_string(),
            "temp" => {
                let delta: f64 = rest.parse().unwrap_or(0.0);
                let now = self.fap.vars.get("A32NX_COND_PURS_SEL_TEMPERATURE").and_then(Value::as_f64).unwrap_or(24.0);
                self.study_action(json!({ "kind": "fapSet", "item": "pursTemp", "value": (now + delta).clamp(18.0, 30.0) }));
            }
            _ => {}
        }
    }

    /// Whether the host should feed [`PB_VARS`] to [`Efb::pb_update`].
    pub fn pb_wants_vars(&self) -> bool {
        self.tab == "pushback" || self.pb_auto.is_some()
    }

    pub fn pb_update(&mut self, vars: efb_pages::jv::Vars) {
        self.pb.vars = vars;
        self.pb_auto_tick();
        self.pb.auto = self.pb_auto.as_ref().map(|a| (a.i, a.steps.len()));
        if self.tab == "pushback" {
            self.dirty = true;
        }
    }

    /// Once a second: the ground engineer with the variables
    /// ([`super::ground_eng::var_names`] and [`PB_VARS`]), `fbw/gse/*` and the
    /// interphone press counts, all without their prefixes.
    pub fn ground_tick(&mut self, now: f64, vars: HashMap<String, f64>, gse: HashMap<String, f64>, presses: HashMap<String, f64>) {
        use super::ground_eng::Event;
        let before = (self.gc.log.len(), self.gc.phase.clone(), self.gc.present, self.gc.coming());
        if self.tab == "ground" && gse != self.gse {
            self.dirty = true;
        }
        self.gse = gse.clone();
        for e in self.gc.tick(now, vars.clone(), gse, presses, self.pb_auto.is_some()) {
            match e {
                Event::Command(c) => {
                    self.study_action(json!({ "kind": "command", "command": c }));
                }
                Event::StartAuto => {
                    for k in PB_VARS {
                        if let Some(v) = vars.get(*k) {
                            self.pb.vars.insert(k.to_string(), json!(v));
                        }
                    }
                    self.pb_start_auto();
                }
                Event::StopAuto => {
                    self.pb_auto = None;
                    self.pb.auto = None;
                    self.pb_send(0.0, 0.0);
                }
                Event::Say(k) => {
                    crate::copilot::say(&k);
                    if let Some(l) = self.gc.log.last() {
                        let t = format!("Ground: {}", l.1);
                        self.show(&t);
                    }
                }
            }
        }
        if before != (self.gc.log.len(), self.gc.phase.clone(), self.gc.present, self.gc.coming()) && self.tab == "pushback" {
            self.dirty = true;
        }
    }

    pub fn touch(&mut self) {
        self.dirty = true;
    }

    /// Names the Ground Services page reads, for the host to fill `ground_vars`.
    pub fn ground_var_names(&self) -> Vec<String> {
        if self.tab != "ground" {
            return Vec::new();
        }
        let mut out: Vec<String> = ["A32NX_AIRCRAFT_PRESET_LOAD", "A32NX_AIRCRAFT_PRESET_LOAD_PROGRESS", "A32NX_AIRCRAFT_PRESET_LOAD_EXPEDITE", "A32NX_ELEC_AC_1_BUS_IS_POWERED", "PUSHBACK ATTACHED"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        out.extend(efb_pages::ground::GM_CHOCKS.iter().map(|c| format!("A32NX_WALKAROUND_CHOCKS_{}", c.to_uppercase())));
        out.extend((1..=4).map(|e| format!("A32NX_ENGINE_COWL_OPEN_{e}")));
        for g in ground_groups() {
            out.extend(g.fields.into_iter().map(|f| f.name));
        }
        out
    }

    fn ground_act(&mut self, arg: &str) {
        let (what, rest) = arg.split_once(':').unwrap_or((arg, ""));
        let cmd = |me: &mut Self, c: &str| {
            me.study_action(json!({ "kind": "command", "command": c }));
        };
        let on = |me: &Self, k: &str| me.ground_vars.get(k).and_then(Value::as_f64).unwrap_or(0.0) > 0.5;
        match what {
            "preset" => {
                let id: u64 = rest.parse().unwrap_or(0);
                if self.study_action(json!({ "kind": "loadAircraftPreset", "id": id })) && id > 0 {
                    self.show("Loading the aircraft state");
                }
            }
            "chocks" => {
                let want = !efb_pages::ground::GM_CHOCKS.iter().all(|c| on(self, &format!("A32NX_WALKAROUND_CHOCKS_{}", c.to_uppercase())));
                for c in efb_pages::ground::GM_CHOCKS {
                    if on(self, &format!("A32NX_WALKAROUND_CHOCKS_{}", c.to_uppercase())) != want {
                        cmd(self, &format!("fbw/walkaround/chocks_{c}_toggle"));
                    }
                }
            }
            "cowl" => cmd(self, &format!("fbw/engine/cowl_{rest}_toggle")),
            "svc" => match rest {
                "gpu" => cmd(self, "fbw/efb/ground/gpu"),
                "asu" | "pca" | "water" | "lav" | "clean" | "deice" => cmd(self, &format!("fbw/gse/{rest}/toggle")),
                key => {
                    let places: &[&str] = match key {
                        "cat_main" => &["cat_m2r"],
                        "cat_upper" => &["udcat_u1r"],
                        "hold_fwd" => &["ldcl_fwd", "uld_fwd"],
                        "hold_aft" => &["ldcl_aft", "uld_aft"],
                        "hold_bulk" => &["belt_bulk", "bag_tractor", "bulk_cart"],
                        "fuel_hydrant" => &["fuel_hydrant"],
                        "fuel_tanker" => &["fuel_tanker"],
                        "stairs_m1l" => &["stairs_m1l"],
                        "stairs_m2l" => &["stairs_m2l"],
                        "stairs_u1l" => &["stairs_u1l"],
                        _ => &[],
                    };
                    let present: Vec<&str> = places.iter().copied().filter(|p| matches!(self.gse.get(&format!("place/{p}/state")).copied().unwrap_or(0.0) as i64, 1 | 2)).collect();
                    for p in if present.is_empty() { places.to_vec() } else { present } {
                        cmd(self, &format!("fbw/gse/place/{p}/toggle"));
                    }
                }
            },
            "lpload" => {
                let id: u64 = rest.parse().unwrap_or(0);
                self.study_action(json!({ "kind": "lightingPreset", "op": "load", "id": id }));
            }
            "lpsave" => {
                let id: u32 = rest.parse().unwrap_or(0);
                if self.lp_armed == id {
                    self.lp_armed = 0;
                    if self.study_action(json!({ "kind": "lightingPreset", "op": "save", "id": id })) {
                        self.show("Lighting saved");
                    }
                } else {
                    self.lp_armed = id;
                }
            }
            "slides" => self.show("Slides are repacked by the maintenance crew between flights"),
            _ => {}
        }
    }

    fn pb_start_auto(&mut self) {
        let hdg = self.pb_var("PLANE HEADING DEGREES TRUE");
        let pos = (self.pb_var("PLANE LATITUDE"), self.pb_var("PLANE LONGITUDE"));
        let plan = self.pb.plan.clone();
        let pace = match plan.pace.as_str() {
            "SLOW" => 0.22,
            "FAST" => 0.45,
            _ => 0.32,
        };
        let straight = |dist: f64, speed: f64| PushStep { turn: false, dist, angle: 0.0, tail_left: false, speed };
        let mut steps = Vec::new();
        if plan.dir == "STRAIGHT" {
            steps.push(straight(plan.after.max(10.0), -pace));
        } else {
            steps.push(straight(12.0, -pace));
            steps.push(PushStep { turn: true, dist: 0.0, angle: plan.angle, tail_left: plan.dir == "LEFT", speed: -pace });
            if plan.after > 0.0 {
                steps.push(straight(plan.after, -pace));
            }
        }
        if plan.pull > 0.0 {
            steps.push(straight(plan.pull, pace * 0.8));
        }
        self.pb_auto = Some(AutoPush { steps, i: 0, hdg, pos });
        self.pb.auto = Some((0, self.pb_auto.as_ref().map_or(0, |a| a.steps.len())));
    }

    fn pb_auto_tick(&mut self) {
        let Some(mut a) = self.pb_auto.take() else { return };
        if self.pb_var("PUSHBACK ATTACHED") == 0.0 {
            self.pb_send(0.0, 0.0);
            return;
        }
        let hdg = self.pb_var("PLANE HEADING DEGREES TRUE");
        let pos = (self.pb_var("PLANE LATITUDE"), self.pb_var("PLANE LONGITUDE"));
        let Some(s) = a.steps.get(a.i).cloned() else {
            self.pb_send(0.0, 0.0);
            return;
        };
        let done = if s.turn {
            let left = s.angle - pb_turned(a.hdg, hdg).abs();
            let h = if s.tail_left { -1.0 } else { 1.0 } * (left / 25.0).clamp(0.25, 1.0);
            self.pb_send(s.speed * if left < 15.0 { 0.7 } else { 1.0 }, h);
            left <= 1.5
        } else {
            let drift = pb_turned(a.hdg, hdg);
            self.pb_send(s.speed, (drift * 0.08 * if s.speed < 0.0 { 1.0 } else { -1.0 }).clamp(-0.3, 0.3));
            pb_dist(a.pos, pos) >= s.dist
        };
        if done {
            a.i += 1;
            a.hdg = hdg;
            a.pos = pos;
            if a.i >= a.steps.len() {
                self.pb_send(0.0, 0.0);
                return;
            }
        }
        self.pb_auto = Some(a);
    }

    fn pb_act(&mut self, arg: &str) {
        match arg {
            "tug" => {
                self.study_action(json!({ "kind": "command", "command": "fbw/pushback/toggle" }));
            }
            "stop" => {
                self.pb_auto = None;
                self.pb.auto = None;
                self.pb_send(0.0, 0.0);
            }
            "go" => {
                if self.pb_var("PUSHBACK ATTACHED") != 0.0 && self.pb_var("A32NX_PARK_BRAKE_LEVER_POS") < 0.5 {
                    self.pb_start_auto();
                }
            }
            "pad" => {}
            _ => {
                let Some((key, v)) = arg.split_once(':') else { return };
                let plan = &mut self.pb.plan;
                match key {
                    "dir" => plan.dir = v.to_string(),
                    "pace" => plan.pace = v.to_string(),
                    "angle" => plan.angle = v.parse().unwrap_or(plan.angle),
                    "after" => plan.after = v.parse().unwrap_or(plan.after),
                    "pull" => plan.pull = v.parse().unwrap_or(plan.pull),
                    _ => {}
                }
            }
        }
    }

    /// A page scroll moves the anchor of an open menu, so the menu goes (the dropdown's own list scrolls in place).
    fn scrolled_away(&mut self, id: &str) {
        if id != DROPDOWN_ID && (self.dropdown.is_some() || self.menu.is_some()) {
            self.dropdown = None;
            self.menu = None;
            self.dirty = true;
        }
    }

    fn tap_nothing(&mut self) {
        if self.dropdown.take().is_some() || self.menu.take().is_some() {
            self.dirty = true;
        }
        self.blur();
    }

    /// One X-Plane key callback; true when the EFB used it.
    pub fn key(&mut self, ch: u32, vkey: u32, flags: u32) -> bool {
        if self.locked {
            if flags & text::vk::UP_FLAG == 0 {
                self.locked = false;
                self.dirty = true;
            }
            return true;
        }
        if self.focus.is_none() {
            let open = self.dropdown.is_some() || self.menu.is_some();
            if open && flags & text::vk::UP_FLAG == 0 && (vkey == text::vk::ESCAPE || ch == 27) {
                self.dropdown = None;
                self.menu = None;
                self.dirty = true;
                return true;
            }
            return false;
        }
        let clip =self.backend.clipboard_get();
        let Some(k) = text::key_of(ch, vkey, flags, &move || clip.clone()) else { return true };
        self.edit_key(k);
        true
    }

    fn edit_key(&mut self, k: Key) {
        match self.edit.apply(k) {
            Outcome::Edited => {
                if self.focus == Some(Field::Search) {
                    self.search = self.edit.text.clone();
                }
                self.dirty = true;
            }
            Outcome::Moved => self.dirty = true,
            Outcome::Commit | Outcome::Release => {
                if self.focus == Some(Field::Modal) {
                    self.activate("modal:ok");
                } else {
                    self.blur();
                }
            }
            Outcome::Copy(t) => {
                self.backend.clipboard_set(&t);
                self.dirty = true;
            }
            Outcome::Nothing => {}
        }
    }

    fn field_value(&self, f: &Field) -> String {
        match f {
            Field::Setting(k) => self.setting(k),
            Field::Search => self.search.clone(),
            Field::FpUser => self.fp_user.clone(),
            Field::SavesName => self.saves_name.clone(),
            Field::Modal => String::new(),
            Field::FailSearch => self.fail_view.query.clone(),
            Field::Efb(k) => efb_value(k).filter(|v| v.is_finite()).map(|v| format!("{}", v.round() as i64)).unwrap_or_default(),
        }
    }

    fn focus_on(&mut self, f: Field) {
        if self.focus.as_ref() == Some(&f) {
            self.kb = true;
            self.dirty = true;
            return;
        }
        self.blur();
        let numeric = matches!(&f, Field::Setting(k) if self.item(k).is_some_and(|i| i.num.is_some())) || matches!(f, Field::Efb(_));
        let max = if f == Field::SavesName { Some(64) } else { None };
        self.edit = TextEdit::new(&self.field_value(&f), numeric, max);
        self.focus = Some(f);
        self.kb = true;
        self.backend.keyboard_capture(true);
        self.dirty = true;
    }

    fn blur(&mut self) {
        let Some(f) = self.focus.take() else { return };
        self.kb = false;
        self.backend.keyboard_capture(false);
        self.dirty = true;
        let t = self.edit.text.clone();
        match f {
            Field::Setting(k) => {
                let t = match self.item(&k).filter(|i| i.num.is_some()) {
                    Some(i) => match i.normalize(&t) {
                        Some(n) => n,
                        None => return,
                    },
                    None => t,
                };
                if t != self.setting(&k) {
                    self.commit_setting(&k, Value::String(t));
                }
            }
            Field::Search => self.search = t,
            Field::FpUser => self.fp_user = t.trim().to_string(),
            Field::SavesName => self.saves_name = t,
            Field::Modal => {}
            Field::FailSearch => self.fail_view.query = t,
            Field::Efb(k) => {
                if let Ok(v) = t.trim().parse::<f64>() {
                    self.study_action(json!({ "kind": "efbSet", "name": k, "value": v }));
                }
            }
        }
    }

    // ---- actions ----

    /// Runs what tapping an element with `data-act` = `act` does.
    pub fn activate(&mut self, act: &str) {
        self.dirty = true;
        let (kind, arg) = act.split_once(':').unwrap_or((act, ""));
        let was_open = self.dropdown.as_ref().map(|d| d.target.clone());
        if !matches!(kind, "kb" | "opt" | "menu" | "modal") {
            self.dropdown = None;
            self.menu = None;
        }
        if !matches!(kind, "kb" | "focus" | "modal") {
            self.blur();
        }
        match kind {
            "unlock" => self.locked = false,
            "wake" => self.sleep = false,
            "dismiss" => {}
            "sec" => {
                if let Some(s) = SECTIONS.iter().find(|s| s.0 == arg) {
                    self.go(s.2[0].1);
                }
            }
            "tab" => self.go(arg),
            "chrono" => {
                let (run, start, acc) = self.chrono;
                self.chrono = match (run, acc > 0.0) {
                    (false, false) => (true, self.now_ms, 0.0),
                    (true, _) => (false, 0.0, acc + self.now_ms - start),
                    (false, true) => (false, 0.0, 0.0),
                };
            }
            "st-cat" => {
                self.category = arg.parse().unwrap_or(0);
                self.search.clear();
                if let Some(id) = self.scroller_with_class("st-list") {
                    self.scrolls.reset(&id);
                }
            }
            "st-view" => self.compact = arg == "list",
            "st-clear" => self.search.clear(),
            "st-restart" => {
                self.category = 0;
                self.search.clear();
                self.compact = false;
                self.raw = self.backend.load_settings();
            }
            "st-sleep" => self.sleep = true,
            "toggle" => {
                let Some(it) = self.item(arg).cloned() else { return };
                let v = match &it.bool {
                    Some(b) => {
                        let (on, off) = b.split_once(',').unwrap_or((b, ""));
                        Value::String(if self.setting(arg) == on { off } else { on }.to_string())
                    }
                    None => Value::Bool(self.setting(arg) != "true"),
                };
                self.commit_setting(arg, v);
            }
            "set" => {
                if let Some((k, v)) = arg.split_once('=') {
                    self.commit_setting(k, Value::String(v.to_string()));
                }
            }
            "chip" => {
                let Some((k, bit)) = arg.rsplit_once(':') else { return };
                let bit: u32 = bit.parse().unwrap_or(0);
                let mut f = self.setting(k).parse::<u32>().unwrap_or(0) ^ bit;
                if f & 3 == 3 {
                    f &= if bit == 2 { !1 } else { !2 };
                }
                if f & ((1 << 4) | (1 << 6)) == 0 {
                    f |= if bit == 1 << 6 { 1 << 4 } else { 1 << 6 };
                }
                if f & (1 << 4) == 0 {
                    f &= !(1 << 5);
                }
                self.commit_setting(k, Value::String(f.to_string()));
            }
            "basic" => {
                let d = self.table.defaults.get(arg).cloned().unwrap_or_default();
                self.commit_setting(arg, Value::String(d));
            }
            "select" => {
                if was_open.as_deref() != Some(arg) {
                    self.open_dropdown(arg);
                }
            }
            "opt" => {
                let Some((target, v)) = arg.rsplit_once('=') else { return };
                self.dropdown = None;
                if target == "zoom" {
                    self.zoom = v.to_string();
                } else if let Some(k) = target.strip_prefix("set:") {
                    self.commit_setting(k, Value::String(v.to_string()));
                } else if let Some(f) = target.strip_prefix("saves:") {
                    self.saves_setting(f, json!(v.parse::<f64>().unwrap_or(0.0)));
                }
            }
            "focus" => {
                if let Some(f) = Field::parse(arg) {
                    self.focus_on(f);
                }
            }
            "kebab" => {
                let anchor = self.anchor_of(act);
                self.menu = Some((arg.to_string(), anchor));
            }
            "menu" => {
                self.menu = None;
                if let Some(k) = arg.strip_prefix("reset:") {
                    if let Some(d) = self.table.defaults.get(k).cloned() {
                        let ok = self.save(&[(k.to_string(), Value::String(d))]);
                        if ok {
                            self.show("Reset to default");
                        }
                    } else {
                        self.show("No default for this setting");
                    }
                } else if let Some(k) = arg.strip_prefix("copy:") {
                    self.backend.clipboard_set(k);
                }
            }
            "app" => {
                if let Some(m) = self.backend.app_action(arg) {
                    self.show(&m);
                }
            }
            "plugin" => {
                if self.armed.as_ref().map(|a| a.0.as_str()) != Some(arg) {
                    self.armed = Some((arg.to_string(), self.now_ms + ARM_MS));
                    self.show(if arg == "resetAirframe" { "New airframe also clears wear, hours, the MEL and the tech log" } else { "Clears failures and damage and restarts the systems" });
                    return;
                }
                self.armed = None;
                let r = self.backend.route("POST", "/study/action", &json!({ "kind": arg }).to_string());
                let failed = r.as_ref().is_some_and(|(s, _)| *s != 200);
                self.show(if failed { "Could not reset" } else { "Resetting: the systems restart and the start state is applied again" });
            }
            "fp-fetch" => self.fp_fetch(),
            "saves" => self.saves_act(arg),
            "efb" => self.efb_act(arg),
            "pb" => self.pb_act(arg),
            "gc" => self.gc.request(arg),
            "gs" => self.ground_act(arg),
            "fap" => self.fap_act(arg),
            "fail" => self.fail_act(arg),
            "cb" => self.cb_act(arg),
            "comp" => self.comp_act(arg),
            "study" => self.study_act(arg),
            "modal" => self.modal_act(arg),
            "kb" => self.kb_act(arg),
            _ => {}
        }
    }

    fn study_action(&mut self, body: Value) -> bool {
        let r = self.backend.route("POST", "/study/action", &body.to_string());
        super::data::poke();
        match r {
            Some((200, _)) => true,
            Some((_, b)) => {
                let msg = parse_json(&b)["message"].as_str().unwrap_or("Not done").to_string();
                self.show(&msg);
                false
            }
            None => {
                self.show("Not available");
                false
            }
        }
    }

    fn efb_act(&mut self, arg: &str) {
        match arg {
            "board" => {
                let on = efb_value("payload/boarding_started") == Some(1.0);
                self.study_action(json!({ "kind": "efbSet", "name": "payload/boarding_started", "value": if on { 0.0 } else { 1.0 } }));
            }
            "deboard" => {
                self.study_action(json!({ "kind": "efbPress", "name": "payload/deboard" }));
            }
            "refuel" => {
                self.study_action(json!({ "kind": "efbPress", "name": "refuel/start_stop" }));
            }
            "simbrief" => self.simbrief_load(),
            other => {
                if let Some((name, v)) = other.rsplit_once(':') {
                    if let Ok(v) = v.parse::<f64>() {
                        self.study_action(json!({ "kind": "efbSet", "name": name, "value": v }));
                    }
                }
            }
        }
    }

    fn simbrief_load(&mut self) {
        let Some(ofp) = self.ofp.clone() else {
            self.show("Fetch the OFP on the Flight Plan page first");
            return;
        };
        let f = |v: &Value| v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())).unwrap_or(0.0);
        let to_kg = if ofp["params"]["units"].as_str() == Some("lbs") { 0.453_592_37 } else { 1.0 };
        let w = &ofp["weights"];
        let body = json!({
            "kind": "loadSimbrief",
            "pax": f(&w["pax_count"]),
            "bags": f(&w["bag_count"]),
            "paxWeightKg": f(&w["pax_weight"]) * to_kg,
            "bagWeightKg": f(&w["bag_weight"]) * to_kg,
            "freightKg": f(&w["freight_added"]) * to_kg,
            "blockFuelKg": f(&ofp["fuel"]["plan_ramp"]) * to_kg,
        });
        if self.study_action(body) {
            self.show("Loading the SimBrief payload and fuel");
        }
    }

    fn fail_act(&mut self, arg: &str) {
        use efb_pages::failures::FailSel;
        let v = &mut self.fail_view;
        let mut parts = arg.splitn(3, ':');
        let (what, k, rest) = (parts.next().unwrap_or(""), parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
        match what {
            "armed" => v.armed_only = !v.armed_only,
            "clear" => {
                let ids: Vec<u64> = failures_model().families.iter().flat_map(|f| f.units().filter(|u| u.active).map(|u| u.id).collect::<Vec<_>>()).collect();
                for id in ids {
                    self.study_action(json!({ "kind": "toggleFailure", "id": id }));
                }
            }
            "open" => {
                if !v.open.remove(&k) {
                    v.open.insert(k);
                }
            }
            "chapter" => v.chapter = if v.chapter == k { String::new() } else { k },
            "off" => {
                if let Ok(id) = k.parse::<u64>() {
                    self.study_action(json!({ "kind": "toggleFailure", "id": id }));
                }
            }
            "mode" | "units" | "unit" | "sev" | "arm" | "disarm" => {
                let model = failures_model();
                let Some(fam) = model.families.iter().find(|f| f.key == k) else { return };
                let sel = v.sel.entry(k.clone()).or_insert_with(|| FailSel { mode: fam.modes[0].0.clone(), ..Default::default() });
                if !fam.modes.iter().any(|(m, _)| *m == sel.mode) {
                    sel.mode = fam.modes[0].0.clone();
                }
                let members: Vec<efb_pages::failures::FailUnit> = fam.modes.iter().find(|(m, _)| *m == sel.mode).map(|(_, us)| us.clone()).unwrap_or_default();
                match what {
                    "mode" => {
                        sel.mode = rest;
                        sel.units.clear();
                    }
                    "units" => {
                        if members.iter().all(|u| sel.units.contains(&u.unit)) {
                            sel.units.clear();
                        } else {
                            sel.units = members.iter().map(|u| u.unit.clone()).collect();
                        }
                    }
                    "unit" => {
                        if !sel.units.remove(&rest) {
                            sel.units.insert(rest);
                        }
                    }
                    "sev" => sel.sev = rest.parse().unwrap_or(1.0),
                    _ => {
                        let sev = sel.sev;
                        let chosen: Vec<_> = members.iter().filter(|u| members.len() == 1 || sel.units.contains(&u.unit)).cloned().collect();
                        let arm = what == "arm";
                        for u in chosen {
                            if arm {
                                if sev < 1.0 {
                                    self.study_action(json!({ "kind": "setFailureMagnitude", "id": u.id, "magnitude": sev }));
                                } else if !u.active {
                                    self.study_action(json!({ "kind": "toggleFailure", "id": u.id }));
                                }
                            } else if u.active {
                                self.study_action(json!({ "kind": "toggleFailure", "id": u.id }));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn cb_act(&mut self, arg: &str) {
        let (what, key) = arg.split_once(':').unwrap_or((arg, ""));
        match what {
            "chapter" => self.cb_view.section = if self.cb_view.section == key { String::new() } else { key.to_string() },
            "info" => {
                if let Some(i) = self.cb_view.open.iter().position(|o| o == key) {
                    self.cb_view.open.remove(i);
                } else {
                    self.cb_view.open.push(key.to_string());
                }
            }
            "reset-all" => {
                if self.study_action(json!({ "kind": "resetAllBreakers" })) {
                    self.show("Resetting every breaker");
                }
            }
            "toggle" => {
                let (source, id) = key.split_once(':').unwrap_or(("", key));
                let list = cached(Source::Breakers);
                let closed = list["breakers"].as_array().and_then(|l| {
                    l.iter().find(|c| c["id"].as_str().map(str::to_string).unwrap_or_else(|| c["id"].to_string()) == id && c["source"].as_str().unwrap_or("") == source)
                })
                .map(|c| c["closed"].as_bool().unwrap_or(true));
                let kind = if closed.unwrap_or(true) { "pullBreaker" } else { "resetBreaker" };
                self.study_action(json!({ "kind": kind, "id": id }));
            }
            _ => {}
        }
    }

    fn comp_act(&mut self, arg: &str) {
        let (what, key) = arg.split_once(':').unwrap_or((arg, ""));
        let list = || cached(Source::Components)["components"].as_array().cloned().unwrap_or_default();
        let find = |key: &str| {
            let (comp, param) = key.split_once('|').unwrap_or((key, ""));
            list().into_iter().find(|c| c["component"].as_str() == Some(comp) && c["param"].as_str().map(str::to_string).unwrap_or_else(|| c["param"].to_string()) == param)
        };
        let set = |me: &mut Self, c: &Value, fr: f64| {
            let h = c["healthy"].as_f64().unwrap_or(0.0);
            let (mx, mn) = (c["max"].as_f64().unwrap_or(h), c["min"].as_f64().unwrap_or(h));
            let limit = if (mx - h).abs() >= (mn - h).abs() { mx } else { mn };
            me.study_action(json!({ "kind": "setComponentParam", "component": c["component"], "param": c["param"], "value": h + (limit - h) * fr }));
        };
        match what {
            "system" => self.comp_view.section = if self.comp_view.section == key { String::new() } else { key.to_string() },
            "info" => {
                if let Some(i) = self.comp_view.open.iter().position(|o| o == key) {
                    self.comp_view.open.remove(i);
                } else {
                    self.comp_view.open.push(key.to_string());
                }
            }
            "refresh" => {}
            "restore-all" => {
                for c in list() {
                    if efb_pages::maint::comp_degradation(&c) > 1e-6 {
                        set(self, &c, 0.0);
                    }
                }
            }
            "restore" | "fail" => {
                if let Some(c) = find(key) {
                    set(self, &c, if what == "fail" { 1.0 } else { 0.0 });
                }
            }
            "set" => {
                let (k, fr) = key.rsplit_once(':').unwrap_or((key, "0"));
                if let Some(c) = find(k) {
                    set(self, &c, fr.parse().unwrap_or(0.0));
                }
            }
            _ => {}
        }
    }

    fn study_act(&mut self, arg: &str) {
        let (what, rest) = arg.split_once(':').unwrap_or((arg, ""));
        match what {
            "page" => self.study_view.selected = rest.parse::<f64>().unwrap_or(0.0) as usize,
            "physics" => self.study_view.physics = !self.study_view.physics,
            "command" => {
                self.study_action(json!({ "kind": "command", "command": rest }));
            }
            "toggleFailure" => {
                if let Ok(id) = rest.parse::<u64>() {
                    self.study_action(json!({ "kind": "toggleFailure", "id": id }));
                }
            }
            "serviceOxygen" => {
                if self.study_action(json!({ "kind": "serviceOxygen" })) {
                    self.show("Oxygen serviced");
                }
            }
            "" => self.study_view.group = None,
            group => {
                self.study_view.group = Some(group.to_string());
                self.study_view.selected = 0;
            }
        }
    }

    /// Names the study pages read, for the host to fill `study_vars` from.
    pub fn study_var_names(&self) -> Vec<String> {
        if self.tab != "study" {
            return Vec::new();
        }
        let mut out = std::collections::BTreeSet::new();
        fn walk(v: &Value, out: &mut std::collections::BTreeSet<String>) {
            match v {
                Value::Object(m) => {
                    for (k, x) in m {
                        if matches!(k.as_str(), "name" | "a" | "b" | "var") {
                            if let Some(n) = x.as_str() {
                                if n.contains(':') || n.contains('_') {
                                    out.insert(n.to_string());
                                }
                            }
                        }
                        walk(x, out);
                    }
                }
                Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
                _ => {}
            }
        }
        walk(&cached(Source::Pages), &mut out);
        out.into_iter().collect()
    }

    fn go(&mut self, tab: &str) {
        if self.tab == tab {
            return;
        }
        self.tab = tab.to_string();
        self.last_poll = f64::NEG_INFINITY;
        if tab == "flightplan" && self.fp_user.is_empty() {
            self.fp_user = self.setting("CONFIG_OVERRIDE_SIMBRIEF_USERID");
        }
    }

    /// How far the scroller holding the failures list is scrolled.
    fn failures_scroll_top(&self) -> f64 {
        let nodes = self.nodes();
        let mut at = nodes.iter().position(|n| n.id.as_deref() == Some("failures-content"));
        while let Some(i) = at {
            if let Some(id) = &nodes[i].scroller {
                return self.scrolls.top(id);
            }
            at = nodes[i].parent;
        }
        0.0
    }

    fn scroller_with_class(&self, _class: &str) -> Option<String> {
        self.nodes().iter().find(|n| n.tag == "div" && n.scroller.is_some() && n.id.is_none()).and_then(|n| n.scroller.clone())
    }

    fn anchor_of(&self, act: &str) -> Option<Rect> {
        hit::find_act(self.nodes(), act).and_then(|i| self.rect_of(i))
    }

    fn select_options(&self, key: &str) -> Vec<(String, String)> {
        let rating_gp = self.setting("xphfbw.engineType") == "GP7200";
        self.item(key).and_then(|i| i.sel.clone()).unwrap_or_default().into_iter().filter(|(v, _)| key != "xphfbw.engineRating" || v.starts_with("GP") == rating_gp).collect()
    }

    fn open_dropdown(&mut self, target: &str) {
        let options: Vec<(String, String)> = match target {
            "zoom" => ZOOMS.iter().map(|(v, t)| (v.to_string(), t.to_string())).collect(),
            "saves:interval_min" => INTERVALS.iter().map(|(v, t)| (v.to_string(), t.to_string())).collect(),
            "saves:rolling_slots" => (1..=10).map(|i| (i.to_string(), i.to_string())).collect(),
            t => {
                let Some(k) = t.strip_prefix("set:") else { return };
                self.select_options(k)
            }
        };
        let anchor = self.anchor_of(&format!("select:{target}"));
        self.scrolls.reset(DROPDOWN_ID);
        self.dropdown = Some(Dropdown { target: target.to_string(), options, anchor });
    }

    fn save(&mut self, values: &[(String, Value)]) -> bool {
        let map: Map<String, Value> = values.iter().cloned().collect();
        match self.backend.save_settings(&map) {
            Ok(()) => {
                for (k, v) in values {
                    self.raw.insert(k.clone(), v.clone());
                }
                self.dirty = true;
                true
            }
            Err(m) => {
                self.show(if m.is_empty() { "Could not save" } else { &m });
                false
            }
        }
    }

    /// What the page's `change` handler does for a `[data-key]` control.
    pub fn commit_setting(&mut self, key: &str, value: Value) {
        let mut values = vec![(key.to_string(), value.clone())];
        if key == "xphfbw.cabinVisible" {
            if self.save(&values) {
                self.show(if value == Value::Bool(true) { "Cabin shown" } else { "Cabin hidden" });
            }
            return;
        }
        if key == "xphfbw.engineType" {
            let gp = jstr(&value) == "GP7200";
            let rating = self.setting("xphfbw.engineRating");
            if rating.starts_with("GP") != gp {
                values.push(("xphfbw.engineRating".into(), Value::String(if gp { "GP7270" } else { "972B" }.into())));
            }
        }
        let mut mine: Vec<String> = serde_json::from_str(&self.setting("xphfbw.userKeys")).unwrap_or_default();
        let before = mine.len();
        for (k, _) in &values {
            if !mine.contains(k) {
                mine.push(k.clone());
            }
        }
        if mine.len() != before {
            values.push(("xphfbw.userKeys".into(), Value::String(serde_json::to_string(&mine).unwrap_or_default())));
        }
        if self.save(&values) {
            self.show("Saved");
        }
    }

    fn fp_fetch(&mut self) {
        let id = self.fp_user.trim().to_string();
        if id.is_empty() {
            self.fp_state = "Enter a SimBrief user ID first".into();
            return;
        }
        self.fp_state = "Fetching\u{2026}".into();
        self.fp_pending = Some(format!("https://www.simbrief.com/api/xml.fetcher.php?userid={}&json=v2", urlencode(&id)));
    }

    fn fp_loaded(&mut self, status: u16, body: &str) {
        self.dirty = true;
        let ofp: Option<Value> = serde_json::from_str(body).ok();
        let fetch_status = ofp.as_ref().map(|o| efb_pages::model::path_str(o, &["fetch", "status"])).unwrap_or_default();
        match ofp {
            Some(o) if status == 200 && (fetch_status.is_empty() || fetch_status == "Success") => {
                let generated = efb_pages::model::path_f64(&o, &["params", "time_generated"]);
                self.fp_state = format!("Generated {}", generated.map(utc_date).unwrap_or_default());
                self.ofp = Some(o);
                let id = self.fp_user.trim().to_string();
                if id != self.setting("CONFIG_OVERRIDE_SIMBRIEF_USERID") {
                    let mut values = vec![("CONFIG_OVERRIDE_SIMBRIEF_USERID".to_string(), Value::String(id))];
                    let mut mine: Vec<String> = serde_json::from_str(&self.setting("xphfbw.userKeys")).unwrap_or_default();
                    if !mine.iter().any(|k| k == "CONFIG_OVERRIDE_SIMBRIEF_USERID") {
                        mine.push("CONFIG_OVERRIDE_SIMBRIEF_USERID".into());
                        values.push(("xphfbw.userKeys".into(), Value::String(serde_json::to_string(&mine).unwrap_or_default())));
                    }
                    self.save(&values);
                }
            }
            _ => self.fp_state = format!("SimBrief: {}", if fetch_status.is_empty() { "could not fetch".to_string() } else { fetch_status }),
        }
    }

    fn saves_post(&mut self, path: &str, body: Value) -> bool {
        match self.backend.route("POST", path, &body.to_string()) {
            Some((_, text)) => {
                let j: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                if j.get("ok") == Some(&Value::Bool(false)) {
                    let m = j.get("message").and_then(Value::as_str).unwrap_or("Not done").to_string();
                    self.show(&m);
                    return false;
                }
                true
            }
            None => {
                self.show("Start X-Plane with the A380X to use saved flights");
                false
            }
        }
    }

    fn saves_setting(&mut self, field: &str, v: Value) {
        let st = self.saves.as_ref().and_then(|s| s.get("settings")).cloned().unwrap_or(json!({}));
        let mut body = json!({
            "interval_min": st.get("interval_min").cloned().unwrap_or(json!(5)),
            "rolling_slots": st.get("rolling_slots").cloned().unwrap_or(json!(3)),
            "phase_saves": st.get("phase_saves").cloned().unwrap_or(json!(true)),
            "quit_save": st.get("quit_save").cloned().unwrap_or(json!(true)),
        });
        body[field] = v;
        self.saves_post("/states/settings", body);
        self.load_saves();
    }

    fn saves_act(&mut self, arg: &str) {
        let (what, id) = arg.split_once(':').unwrap_or((arg, ""));
        let name = |s: &Self| s.save_entry(id).and_then(|e| e.get("name").and_then(Value::as_str).map(str::to_string)).unwrap_or_else(|| id.to_string());
        match what {
            "save" => {
                let n = self.saves_name.clone();
                if self.saves_post("/states/save", json!({ "name": n })) {
                    self.show("Saving\u{2026}");
                    self.saves_name.clear();
                }
            }
            "load" => self.modal = Some(Modal { title: format!("Load \"{}\"? The current flight will be replaced.", name(self)), prompt: false, ok: format!("saves:do-load:{id}") }),
            "delete" => self.modal = Some(Modal { title: format!("Delete \"{}\"?", name(self)), prompt: false, ok: format!("saves:do-delete:{id}") }),
            "rename" => {
                self.modal = Some(Modal { title: "New name".into(), prompt: true, ok: format!("saves:do-rename:{id}") });
                let n = name(self);
                self.focus_on(Field::Modal);
                self.edit = TextEdit::new(&n, false, Some(64));
            }
            "do-load" => {
                if self.saves_post("/states/load", json!({ "id": id })) {
                    self.show("Loading\u{2026} settling systems");
                }
            }
            "do-delete" => {
                self.saves_post("/states/delete", json!({ "id": id }));
                self.load_saves();
            }
            "do-rename" => {
                let n = self.edit.text.trim().to_string();
                if !n.is_empty() && self.saves_post("/states/rename", json!({ "id": id, "name": n })) {
                    self.load_saves();
                }
            }
            "phase_saves" | "quit_save" => {
                let cur = self.saves.as_ref().and_then(|s| s.pointer(&format!("/settings/{what}"))).and_then(Value::as_bool).unwrap_or(true);
                self.saves_setting(what, Value::Bool(!cur));
            }
            _ => {}
        }
    }

    fn save_entry(&self, id: &str) -> Option<&Value> {
        self.saves.as_ref()?.get("states")?.as_array()?.iter().find(|s| s.get("id").and_then(Value::as_str) == Some(id))
    }

    fn modal_act(&mut self, arg: &str) {
        let Some(m) = self.modal.take() else { return };
        if arg == "ok" {
            self.activate(&m.ok);
        }
        if self.focus == Some(Field::Modal) {
            self.focus = None;
            self.kb = false;
            self.backend.keyboard_capture(false);
        }
    }

    fn kb_act(&mut self, arg: &str) {
        match arg {
            "shift" => self.kb_shift = !self.kb_shift,
            "sym" => self.kb_sym = true,
            "abc" => self.kb_sym = false,
            "hide" => self.blur(),
            "bksp" => self.edit_key(Key::Backspace),
            "enter" => self.edit_key(Key::Enter),
            _ => {
                if let Some(c) = arg.strip_prefix("char:").and_then(|c| c.parse::<u32>().ok()).and_then(char::from_u32) {
                    self.edit_key(Key::Char(c));
                    if self.kb_shift {
                        self.kb_shift = false;
                    }
                }
            }
        }
    }

    // ---- trees ----

    fn tree(&self) -> Node {
        let m = self.model();
        let mut body = Node::new("body").style(&format!("zoom: {};", self.zoom));
        body = body.child(Node::el("div", "efb-bg").style(&format!("background-image: url(\"{}\");", self.bg_url))).child(Node::el("div", "efb-scrim"));
        let mut tabs = Node::el("div", "tabs").id("tabs");
        for (t, label) in TABS {
            tabs = tabs.child(Node::el("button", "tab-btn").class_if("active", *t == self.tab).attr("type", "button").act(&format!("tab:{t}")).text(label));
        }
        body = body.child(tabs);
        let mut toast = Node::el("div", "toast").id("toast");
        if let Some((t, _)) = &self.toast {
            toast = toast.class_if("show", true).text(t);
        }
        body = body.child(toast);
        body = body.child(Node::el("div", "efb-screen").child(self.rail()).child(Node::el("div", "efb-maincol").child(self.topbar()).child(Node::new("main").child(self.panel(&m)))));
        if let Some((key, anchor)) = &self.menu {
            let (top, left) = anchor.map_or((300.0, 600.0), |r| {
                let top = if r.bottom() + 4.0 + MENU_H > super::doc::VIEW_H { r.y - 4.0 - MENU_H } else { r.bottom() + 4.0 };
                (top.max(0.0), (r.right() - MENU_W).clamp(0.0, super::doc::VIEW_W - MENU_W))
            });
            body = body.child(
                Node::el("div", "st-menu")
                    .style(&format!("top: {top}px; left: {left}px;"))
                    .child(Node::new("button").attr("type", "button").act(&format!("menu:reset:{key}")).text("Reset to default"))
                    .child(Node::new("button").attr("type", "button").act(&format!("menu:copy:{key}")).text("Copy setting name")),
            );
        }
        if let Some(d) = &self.dropdown {
            let current = match d.target.as_str() {
                "zoom" => self.zoom.clone(),
                t => t.strip_prefix("set:").map(|k| self.setting(k)).unwrap_or_default(),
            };
            let mut list = Node::el("div", "select-list no-drag-scroll").id(DROPDOWN_ID).style(&select_list_style(d.anchor, d.options.len()));
            for (v, t) in &d.options {
                list = list.child(Node::el("div", "select-item").class_if("on", *v == current).act(&format!("opt:{}={v}", d.target)).text(t));
            }
            body = body.child(list);
        }
        if let Some(md) = &self.modal {
            let mut b = Node::el("div", "efbn-modal").child(Node::el("div", "efbn-modal-t").text(&md.title));
            if md.prompt {
                b = b.child(Node::el("input", "efbn-modal-in").attr("type", "text").act("focus:modal").value(&self.edit.display(self.focus == Some(Field::Modal) && self.caret_on())));
            }
            b = b.child(
                Node::el("div", "efbn-modal-b")
                    .child(Node::el("button", "small grey").attr("type", "button").act("modal:cancel").text("Cancel"))
                    .child(Node::el("button", "small").attr("type", "button").act("modal:ok").text("OK")),
            );
            body = body.child(Node::el("div", "efbn-back dim").act("modal:cancel")).child(b);
        }
        if self.kb {
            let numeric = self.edit.numeric;
            body = body.child(text::keyboard(self.kb_shift, self.kb_sym, numeric));
        }
        if self.sleep {
            body = body.child(Node::el("div", "st-sleep").act("wake"));
        }
        if self.locked {
            body = body.child(self.lock());
        }
        body
    }

    fn rail(&self) -> Node {
        let sec = SECTIONS.iter().find(|s| s.2.iter().any(|t| t.1 == self.tab)).map_or("home", |s| s.0);
        let mut rail = Node::el("nav", "efb-rail").child(Node::el("div", "logo").text("A380"));
        for (key, label, _) in SECTIONS {
            rail = rail.child(Node::new("a").class_if("on", *key == sec).act(&format!("sec:{key}")).child(icon(key)).child(Node::new("span").text(label)));
        }
        rail.child(Node::el("div", "spacer"))
    }

    fn topbar(&self) -> Node {
        let s = SECTIONS.iter().find(|s| s.2.iter().any(|t| t.1 == self.tab)).unwrap_or(&SECTIONS[0]);
        let mut title = s.1.to_string();
        let mut subs = Node::el("div", "subs");
        if s.2.len() > 1 {
            for (label, tab) in s.2 {
                let on = *tab == self.tab;
                if on {
                    title = label.to_string();
                }
                subs = subs.child(Node::new("a").class_if("on", on).act(&format!("tab:{tab}")).text(label));
            }
        }
        let chrono_ms = if self.chrono.0 { self.chrono.2 + self.now_ms - self.chrono.1 } else { self.chrono.2 };
        let secs = (chrono_ms / 1000.0) as u64;
        let clock = Node::el("div", "efb-clock")
            .child(Node::new("span").id("clock-utc").text(&self.clock.0))
            .child(Node::new("span").id("clock-local").text(&self.clock.1))
            .child(Node::el("button", "chrono").id("chrono").attr("type", "button").act("chrono").text(&format!("CHR {:02}:{:02}", secs / 60, secs % 60)));
        let st = Node::el("div", "st").child(Node::el("div", "sb-prog").attr("hidden", "")).child(clock);
        Node::el("div", "efb-topbar").child(Node::new("h1").text(&title)).child(subs).child(st)
    }

    fn lock(&self) -> Node {
        Node::new("div").id("efb-lock").act("unlock")
            .child(Node::el("div", "lock-bg").style(&format!("background-image: url(\"{}\");", self.bg_url)))
            .child(Node::el("div", "lock-scrim"))
            .child(
                Node::el("div", "lock-inner")
                    .child(Node::el("div", "lock-top").child(Node::el("span", "mono").text("A380 / EFB 1")).child(Node::el("span", "mono").id("lock-pct").text("100%")))
                    .child(
                        Node::el("div", "lock-mid")
                            .child(Node::el("div", "mono lock-date").id("lock-date"))
                            .child(Node::el("div", "lock-clock").id("lock-clock").text(self.clock.1.trim_end_matches(" LT")))
                            .child(Node::el("div", "mono lock-sub").child(Node::new("span").text("UTC ").child(Node::new("b").id("lock-utc").text(self.clock.0.trim_end_matches('Z'))))),
                    )
                    .child(Node::el("div", "lock-foot").child(Node::el("span", "mono").text(&self.lock_route())).child(Node::el("span", "lock-hint").text("Tap to unlock"))),
            )
    }

    fn lock_route(&self) -> String {
        match &self.ofp {
            Some(o) => format!("{} -> {} / A380-842", efb_pages::model::path_str(o, &["origin", "icao_code"]), efb_pages::model::path_str(o, &["destination", "icao_code"])),
            None => "A380-842".into(),
        }
    }

    fn panel(&self, m: &EfbModel) -> Node {
        let panel = Node::el("div", "tab-panel active").id(&format!("tab-{}", self.tab));
        match self.tab.as_str() {
            "home" => panel.child(decorate_home(home_page(m))),
            "settings" => panel.child(self.decorate_settings(settings_page(&self.table, m))),
            "flightplan" => {
                let user = if self.focus == Some(Field::FpUser) { self.edit.display(self.caret_on()) } else { self.fp_user.clone() };
                let bar = Node::el("div", "study-toolbar")
                    .child(Node::el("span", "label").text("SimBrief user ID"))
                    .child(Node::new("input").id("fp-user").attr("type", "text").style("width:220px").act("focus:fp-user").value(&user))
                    .child(Node::el("button", "small").id("fp-fetch").attr("type", "button").act("fp-fetch").text("Fetch latest OFP"))
                    .child(Node::el("span", "label dim").id("fp-state").text(&self.fp_state));
                panel.child(Node::el("div", "page-main").child(bar).child(flight_plan_content(m, &FlightPlanView::default())))
            }
            "saves" => panel.child(self.saves_page()),
            other => match self.live_page(other, m) {
                Some(mut page) if page.cls.iter().any(|c| c == "tab-panel") => {
                    if !page.cls.iter().any(|c| c == "active") {
                        page.cls.push("active".into());
                    }
                    page
                }
                Some(page) => panel.child(frame(other, page)),
                None => panel.child(Node::el("div", "page-main").child(Node::el("div", "box").child(Node::el("span", "label dim").text(&format!("{} is not drawn natively yet.", tab_label(other)))))),
            },
        }
    }

    fn live_page(&self, tab: &str, m: &EfbModel) -> Option<Node> {
        use crate::study::web;
        use efb_pages::{maint, study};
        let table = efb_pages::settings::SettingsTable::bundled();
        Some(match tab {
            "study" => {
                let pages = study_pages();
                study::study_tab(&pages, &self.study_vars, &self.study_view, true)
            }
            "failures" => {
                let fm = failures_model();
                let search = if self.focus == Some(Field::FailSearch) { self.edit.display(self.caret_on()) } else { self.fail_view.query.clone() };
                let mut armed = Node::new("input").attr("type", "checkbox").id("failures-armed");
                if self.fail_view.armed_only {
                    armed = armed.attr("checked", "");
                }
                let toolbar = Node::el("div", "study-toolbar")
                    .id("failures-toolbar")
                    .child(Node::new("input").attr("type", "text").id("failures-search").attr("placeholder", "Search by ATA number or name\u{2026}").act("focus:fail-search").value(&search))
                    .child(Node::el("label", "row").style("margin:0").act("fail:armed").child(armed).child(Node::el("span", "label").text("Armed only")))
                    .child(Node::el("button", "small grey").attr("type", "button").id("failures-clear").act("fail:clear").text("Disarm all"));
                Node::el("div", "study-layout").child(efb_pages::failures::failures_nav(&fm, &self.fail_view)).child(
                    Node::el("div", "study-main").child(toolbar).child(Node::new("div").id("failures-content").child(efb_pages::failures::failures_content_with(
                        &fm,
                        &{
                            let mut v = self.fail_view.clone();
                            v.viewport = Some(FAIL_WINDOW_PX);
                            v.scroll_top = (self.failures_scroll_top() - FAIL_WINDOW_PX / 4.0).max(0.0);
                            v
                        },
                        &efb_pages::diagrams::fail_diagram,
                    ))),
                )
            }
            "breakers" => {
                let v = cached(Source::Breakers);
                let list = v["breakers"].as_array().map(|l| maint::breaker_list(l));
                let mut view = self.cb_view.clone();
                if view.section.is_empty() {
                    view.section = v["breakers"].as_array().and_then(|l| l.iter().filter_map(|c| c["chapter"].as_str()).min_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()))).unwrap_or("").to_string();
                }
                maint::breakers_tab(list.as_deref(), &view, true)
            }
            "components" => {
                let v = cached(Source::Components);
                let mut view = self.comp_view.clone();
                if view.section.is_empty() {
                    view.section = v["components"].as_array().and_then(|l| l.first()).map(maint::comp_system).unwrap_or_default();
                }
                maint::components_tab(v["components"].as_array().map(Vec::as_slice), &view, true)
            }
            "maintenance" => {
                let v = parse_json(&web::maintenance_json());
                maint::maintenance_tab(Some(&v), &Default::default(), &Default::default(), &maint::ListView::default(), true)
            }
            "wear" => {
                let v = efb_pages::jv::parse_ordered(&web::wear_json());
                maint::wear_tab(Some(&v), true)
            }
            "ground" => {
            let m = efb_pages::ground::GroundModel {
                vars: self.ground_vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                gse: self.gse.iter().map(|(k, v)| (k.clone(), json!(v))).collect(),
                groups: ground_groups(),
                settings: self.raw.iter().map(|(k, v)| (k.clone(), jstr(v))).collect(),
                lp_editing: 0,
                lp_armed: self.lp_armed,
                preview: false,
            };
            efb_pages::ground::ground_content(&m)
        }
            "pushback" => {
                let mut v = self.pb.clone();
                v.gc_present = self.gc.present;
                v.gc_coming = self.gc.coming();
                v.gc_phase = self.gc.phase.clone();
                v.gc_log = self.gc.log.iter().map(|(who, text)| efb_pages::pushback::GcLine { who: who.clone(), text: text.clone() }).collect();
                efb_pages::pushback::pushback_content(&v)
            }
            "fap" => {
                let mut v = self.fap.clone();
                v.utc = self.clock.0.trim_end_matches('Z').to_string() + " UTC";
                efb_pages::fap::fap_content(&v, m, &table)
            }
            "atc" => efb_pages::ops::atc_content(&Default::default()),
            "cargo" => efb_pages::ops::cargo_content(&Default::default()),
            "performance" => {
                let mut v = efb_pages::perf::PerfView::default();
                let mut to = efb_pages::perf::initial("to");
                to.extend(std::mem::take(&mut v.to));
                let mut ldg = efb_pages::perf::initial("ldg");
                ldg.extend(std::mem::take(&mut v.ldg));
                let mut applied = Default::default();
                efb_pages::perf::operator_defaults(&mut to, &mut ldg, &mut applied, &v.settings, &table.defaults);
                v.to = to;
                v.ldg = ldg;
                efb_pages::perf::perf_content(&v)
            }
            "loadsheet" => {
                let page = cached(Source::Pages)["pages"].as_array().and_then(|p| p.iter().find(|x| x.get("zones").is_some()).cloned()).unwrap_or(Value::Null);
                let state: Map<String, Value> = crate::published::numbers_named("fbw/efb/").into_iter().map(|(k, v, _)| (k["fbw/efb/".len()..].to_string(), json!(v))).collect();
                let mut node = efb_pages::payload::loadsheet_page(&page, None, &state, "");
                if let Some(Field::Efb(k)) = &self.focus {
                    let text = self.edit.display(self.caret_on());
                    let act = format!("focus:{k}");
                    visit(&mut node, &mut |n| {
                        if n.data.get("act") == Some(&act) {
                            n.value = Some(text.clone());
                        }
                    });
                }
                node
            }
            "checklists" => efb_pages::docs::docs_page(&efb_pages::docs::DocsData::bundled(), m, &Default::default()),
            "charts" => efb_pages::misc::charts_tab(&Default::default(), true),
            "throttle" => efb_pages::misc::throttle_content(&Default::default()),
            "status" => efb_pages::misc::status_tab(&Default::default(), true),
            "diagnostics" => efb_pages::misc::diagnostics_tab(&Default::default(), true),
            "support" => efb_pages::misc::support_tab(true),
            _ => return None,
        })
    }

    fn input_value(&self, key: &str, base: &str) -> String {
        if self.focus == Some(Field::Setting(key.to_string())) {
            self.edit.display(self.caret_on())
        } else {
            base.to_string()
        }
    }

    fn decorate_settings(&self, mut page: Node) -> Node {
        let items: Vec<Item> = self.table.categories.iter().flat_map(|c| c.items.iter().cloned()).collect();
        let streamer = self.setting("xphfbw.streamerMode") == "true";
        visit(&mut page, &mut |n| {
            if has(n, "st-cats") {
                for (ci, b) in nodes_mut(n).enumerate() {
                    *b = std::mem::take(b).act(&format!("st-cat:{ci}"));
                }
            } else if has(n, "st-list") {
                for (r, it) in nodes_mut(n).zip(items.iter()) {
                    self.decorate_row(r, it, streamer);
                }
            } else if has(n, "st-searchbox") {
                for k in nodes_mut(n) {
                    if has(k, "st-search") {
                        k.attrs.insert("type".into(), "text".into());
                        k.data.insert("act".into(), "focus:search".into());
                        k.value = Some(if self.focus == Some(Field::Search) { self.edit.display(self.caret_on()) } else { self.search.clone() });
                    } else if has(k, "st-clear") {
                        k.data.insert("act".into(), "st-clear".into());
                    }
                }
            } else if has(n, "st-views") {
                for (i, k) in nodes_mut(n).enumerate() {
                    k.data.insert("act".into(), if i == 0 { "st-view:cards" } else { "st-view:list" }.into());
                    set_icon(k, if i == 0 { &["M3.5 5h13v10h-13z", "M7.5 19h13V9"] } else { &["M9 6h11M9 12h11M9 18h11"] });
                }
            } else if has(n, "st-side") {
                let mut i = 0;
                for k in nodes_mut(n) {
                    if has(k, "st-sidebtn") {
                        k.data.insert("act".into(), if i == 0 { "st-restart" } else { "st-sleep" }.into());
                        i += 1;
                    }
                }
            }
        });
        page
    }

    fn decorate_row(&self, row: &mut Node, it: &Item, streamer: bool) {
        let Some(key) = it.key.clone() else {
            if let Some(ctl) = find_cls(row, "st-ctl") {
                if let Some(first) = nodes_mut(ctl).next() {
                    self.decorate_actions(first, it);
                }
            }
            return;
        };
        let Some(ctl) = find_cls(row, "st-ctl") else { return };
        let mut kids = nodes_mut(ctl);
        let (Some(control), Some(kebab)) = (kids.next(), kids.next()) else { return };
        kebab.data.insert("act".into(), format!("kebab:{key}"));
        if it.zoom {
            to_dropdown(control, "zoom", ZOOMS.iter().find(|z| z.0 == self.zoom).map_or("Normal", |z| z.1), ZOOMS.iter().map(|z| z.1));
        } else if it.callouts {
            for c in nodes_mut(control) {
                if has(c, "st-chips") {
                    for (b, (_, bit)) in nodes_mut(c).zip(CALLOUTS.iter()) {
                        b.data.insert("act".into(), format!("chip:{key}:{bit}"));
                    }
                } else if c.tag == "button" {
                    c.data.insert("act".into(), format!("basic:{key}"));
                }
            }
        } else if it.seg.is_some() || it.sel.as_ref().is_some_and(|s| s.len() <= 3) {
            let opts = it.seg.clone().or_else(|| it.sel.clone()).unwrap_or_default();
            for c in nodes_mut(control) {
                if has(c, "st-seg") {
                    for (b, (v, _)) in nodes_mut(c).zip(opts.iter()) {
                        b.data.insert("act".into(), format!("set:{key}={v}"));
                    }
                }
            }
        } else if it.sel.is_some() {
            let cur = self.setting(&key);
            let shown = self.select_options(&key);
            let label = shown.iter().find(|o| o.0 == cur).map_or(String::new(), |o| o.1.clone());
            to_dropdown(control, &format!("set:{key}"), &label, shown.iter().map(|o| o.1.as_str()));
        } else if let Some([lo, hi, _]) = it.range {
            for c in nodes_mut(control) {
                if c.tag == "input" {
                    c.attrs.insert("type".into(), "range".into());
                    c.attrs.insert("min".into(), lo.to_string());
                    c.attrs.insert("max".into(), hi.to_string());
                    c.data.insert("act".into(), format!("range:{key}"));
                }
            }
        } else if it.num.is_some() {
            for c in nodes_mut(control) {
                if c.tag == "input" {
                    c.attrs.insert("type".into(), "number".into());
                    c.data.insert("act".into(), format!("focus:set:{key}"));
                    let v = self.input_value(&key, c.value.as_deref().unwrap_or(""));
                    c.value = Some(v);
                }
            }
        } else if it.text.is_some() {
            control.attrs.insert("type".into(), "text".into());
            control.data.insert("act".into(), format!("focus:set:{key}"));
            let mut v = self.input_value(&key, control.value.as_deref().unwrap_or(""));
            if it.secret && streamer && self.focus != Some(Field::Setting(key.clone())) {
                v = "\u{2022}".repeat(v.chars().count());
            }
            if v.is_empty() {
                if let Some(ph) = &it.text {
                    control.attrs.insert("placeholder".into(), ph.clone());
                }
            }
            control.value = Some(v);
        } else if control.tag == "label" {
            control.data.insert("act".into(), format!("toggle:{key}"));
            let on = match &it.bool {
                Some(b) => self.setting(&key) == b.split(',').next().unwrap_or(""),
                None => self.setting(&key) == "true",
            };
            if let Some(input) = nodes_mut(control).next() {
                input.attrs.insert("type".into(), "checkbox".into());
                if on {
                    input.attrs.insert("checked".into(), String::new());
                }
            }
        }
    }

    fn decorate_actions(&self, control: &mut Node, it: &Item) {
        let armed = self.armed.as_ref().map(|a| a.0.clone());
        if let Some(action) = &it.action {
            for c in nodes_mut(control) {
                if c.tag == "button" {
                    c.data.insert("act".into(), format!("app:{action}"));
                }
            }
        } else if it.reset {
            for (c, kind) in nodes_mut(control).zip(["resetAircraftState", "resetAirframe"]) {
                c.data.insert("act".into(), format!("plugin:{kind}"));
                if armed.as_deref() == Some(kind) {
                    c.kids = vec![Kid::Text { text: "Press again to confirm".into() }];
                }
            }
        }
    }

    fn saves_page(&self) -> Node {
        let content = Node::el("div", "page-main").id("saves-content");
        let Some(data) = self.saves.as_ref().filter(|d| d.get("states").is_some()) else {
            return content.child(Node::el("div", "box").text("Start X-Plane with the A380X to save and load flights."));
        };
        let name = if self.focus == Some(Field::SavesName) { self.edit.display(self.caret_on()) } else { self.saves_name.clone() };
        let mut name_in = Node::new("input").attr("type", "text").attr("maxlength", "64").style("width:280px").act("focus:saves-name").value(&name);
        if name.is_empty() {
            name_in = name_in.attr("placeholder", "Name this save");
        }
        let toolbar = Node::el("div", "study-toolbar")
            .child(name_in)
            .child(Node::el("button", "small primary").attr("type", "button").act("saves:save").text("Save now"))
            .child(Node::el("span", "label dim").text(data.get("status").and_then(Value::as_str).unwrap_or("")));
        let states = data["states"].as_array().cloned().unwrap_or_default();
        let list = if states.is_empty() {
            Node::el("div", "label dim").text("No saved flights yet.")
        } else {
            let th = |t: &str| Node::new("th").text(t);
            let mut table = Node::new("table").style("width:100%").child(Node::new("tr").child(th("Name")).child(th("Kind")).child(th("Saved")).child(th("Size")).child(Node::new("th")));
            for s in &states {
                let id = s.get("id").and_then(Value::as_str).unwrap_or("");
                let err = s.get("error").and_then(Value::as_str);
                let mut actions = Node::new("td").style("white-space:nowrap;text-align:right");
                let btn = |cls: &str, act: &str, t: &str| Node::el("button", cls).attr("type", "button").act(act).text(t);
                if err.is_none() {
                    actions = actions.child(btn("small", &format!("saves:load:{id}"), "Load")).text(" ").child(btn("small grey", &format!("saves:rename:{id}"), "Rename")).text(" ");
                }
                actions = actions.child(btn("small grey", &format!("saves:delete:{id}"), "Delete"));
                let sub = match err {
                    Some(e) => format!("Unreadable: {e}"),
                    None => saves_summary(s),
                };
                let bytes = s.get("bytes").and_then(Value::as_f64).unwrap_or(0.0);
                let size = if bytes > 0.0 { format!("{} kB", ((bytes / 1024.0).round() as i64).max(1)) } else { String::new() };
                let when = s.get("created_unix").and_then(Value::as_f64).map(utc_date).unwrap_or_default();
                let name = s.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()).unwrap_or(id);
                table = table.child(
                    Node::new("tr")
                        .child(Node::new("td").child(Node::new("div").text(name)).child(Node::el("div", "label dim").text(&sub)))
                        .child(Node::new("td").text(&saves_kind(s)))
                        .child(Node::new("td").text(&when))
                        .child(Node::new("td").text(&size))
                        .child(actions),
                );
            }
            table
        };
        let st = data.get("settings").cloned().unwrap_or(json!({}));
        let num = |k: &str, d: f64| st.get(k).and_then(Value::as_f64).unwrap_or(d);
        let flag = |k: &str| st.get(k).and_then(Value::as_bool).unwrap_or(true);
        let interval = num("interval_min", 5.0).to_string();
        let interval_label = INTERVALS.iter().find(|i| i.0 == interval).map_or(interval.clone(), |i| i.1.to_string());
        let row = |label: &str, control: Node| Node::el("div", "row").style("gap:12px;align-items:center;margin:8px 0").child(Node::el("span", "label").style("width:260px").text(label)).child(control);
        let dropdown = |target: &str, label: &str, options: &[String]| {
            let mut n = Node::new("select");
            to_dropdown(&mut n, &format!("saves:{target}"), label, options.iter().map(String::as_str));
            n
        };
        let intervals: Vec<String> = INTERVALS.iter().map(|i| i.1.to_string()).collect();
        let slots: Vec<String> = (1..=10).map(|i: i32| i.to_string()).collect();
        let switch = |field: &str| {
            let mut input = Node::new("input").attr("type", "checkbox");
            if flag(field) {
                input = input.attr("checked", "");
            }
            Node::el("label", "st-switch").act(&format!("saves:{field}")).child(input).child(Node::new("span"))
        };
        let settings_box = Node::el("div", "box")
            .child(Node::new("h3").text("Autosave"))
            .child(row("Autosave interval", dropdown("interval_min", &interval_label, &intervals)))
            .child(row("Rolling autosave slots", dropdown("rolling_slots", &num("rolling_slots", 3.0).to_string(), &slots)))
            .child(row("Save at pushback, take-off, top of descent, landing and the gate", switch("phase_saves")))
            .child(row("Save when X-Plane quits", switch("quit_save")));
        content.child(toolbar).child(Node::el("div", "box").child(Node::new("h3").text("Saved flights")).child(list)).child(settings_box)
    }
}

fn parse_json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).unwrap_or(serde_json::Value::Null)
}

fn from_or_default<T: serde::de::DeserializeOwned + Default>(v: serde_json::Value) -> T {
    serde_json::from_value(v).unwrap_or_default()
}

/// The container the old shell gave each tab's content: a scrolling `page-main`.
fn frame(tab: &str, mut page: Node) -> Node {
    let framed = page.cls.iter().any(|c| matches!(c.as_str(), "page-main" | "study-layout" | "study-main"));
    if framed {
        return page;
    }
    let id = match tab {
        "pushback" => "pb-content",
        "performance" => "perf-content",
        "ground" => "ground-content",
        "loadsheet" => "loadsheet-content",
        "fap" if page.id.as_deref() == Some("fap-content") => return Node::el("div", "page-main").child(page),
        "fap" => return Node::el("div", "page-main").child(Node::new("div").id("fap-content").child(page)),
        other => return Node::el("div", "page-main").id(&format!("{other}-content")).child(page),
    };
    if page.id.as_deref() == Some(id) {
        page.cls.push("page-main".into());
        page
    } else {
        Node::el("div", "page-main").id(id).child(page)
    }
}

fn ground_groups() -> Vec<efb_pages::ground::Group> {
    cached(Source::Pages)["pages"]
        .as_array()
        .and_then(|p| p.iter().find(|x| x["kind"].as_str() == Some("groundServices")))
        .and_then(|p| serde_json::from_value(p["groups"].clone()).ok())
        .unwrap_or_default()
}

fn efb_value(name: &str) -> Option<f64> {
    let full = format!("fbw/efb/{name}");
    crate::published::numbers_named(&full).into_iter().find(|(k, _, _)| *k == full).map(|(_, v, _)| v)
}

fn cached(source: Source) -> Arc<Value> {
    super::data::get(source).0.unwrap_or_else(|| Arc::new(Value::Null))
}

fn study_pages() -> Arc<Vec<Value>> {
    thread_local! {
        static PAGES: std::cell::RefCell<Option<(u64, Arc<Vec<Value>>)>> = const { std::cell::RefCell::new(None) };
    }
    let (v, version) = super::data::get(Source::Pages);
    PAGES.with(|c| {
        if let Some((ver, p)) = c.borrow().as_ref().filter(|(ver, _)| *ver == version) {
            let _ = ver;
            return p.clone();
        }
        let mut pages = v.as_ref().and_then(|v| v["pages"].as_array().cloned()).unwrap_or_default();
        efb_pages::study::merge_deep(&mut pages);
        let pages = Arc::new(pages);
        *c.borrow_mut() = Some((version, pages.clone()));
        pages
    })
}

fn failures_model() -> Arc<efb_pages::failures::FailuresModel> {
    thread_local! {
        static MODEL: std::cell::RefCell<Option<(u64, Arc<efb_pages::failures::FailuresModel>)>> = const { std::cell::RefCell::new(None) };
    }
    let (v, version) = super::data::get(Source::Failures);
    MODEL.with(|c| {
        if let Some((_, m)) = c.borrow().as_ref().filter(|(ver, _)| *ver == version) {
            return m.clone();
        }
        let v = v.map(|v| (*v).clone()).unwrap_or(Value::Null);
        let m = Arc::new(match v.get("failures").filter(|l| l.is_array()) {
            Some(list) => efb_pages::failures::FailuresModel::from_list(
                serde_json::from_value(list.clone()).unwrap_or_default(),
                v.get("pending").and_then(|p| serde_json::from_value(p.clone()).ok()).unwrap_or_default(),
            ),
            None => from_or_default(v),
        });
        *c.borrow_mut() = Some((version, m.clone()));
        m
    })
}

fn tab_label(tab: &str) -> &str {
    TABS.iter().find(|t| t.0 == tab).map_or(tab, |t| t.1)
}

fn icon(key: &str) -> Node {
    let mut svg = Node::new("svg");
    for (k, v) in [("viewBox", "0 0 24 24"), ("fill", "none"), ("stroke", "currentColor"), ("stroke-width", "1.7"), ("stroke-linecap", "round"), ("stroke-linejoin", "round")] {
        svg.svgattr.insert(k.into(), v.into());
    }
    for d in ICONS.iter().find(|i| i.0 == key).map_or(&[][..], |i| i.1) {
        let mut a = BTreeMap::new();
        a.insert("d".to_string(), d.to_string());
        a.insert("_d".to_string(), "0".to_string());
        svg.shapes.push(Shape { t: "path".into(), a });
    }
    svg
}

fn set_icon(n: &mut Node, paths: &[&str]) {
    visit(n, &mut |k| {
        if k.tag == "svg" && k.shapes.is_empty() {
            for (a, v) in [("viewBox", "0 0 24 24"), ("width", "18"), ("height", "18"), ("fill", "none"), ("stroke", "currentColor"), ("stroke-width", "1.8")] {
                k.svgattr.insert(a.into(), v.into());
            }
            for d in paths {
                let mut a = BTreeMap::new();
                a.insert("d".to_string(), d.to_string());
                a.insert("_d".to_string(), "0".to_string());
                k.shapes.push(Shape { t: "path".into(), a });
            }
        }
    });
}

/// A closed `select` showing `label`; doc.rs gives it Chromium's menulist
/// look, sized by the widest of `options`.
fn to_dropdown<'a>(n: &mut Node, target: &str, label: &str, options: impl Iterator<Item = &'a str>) {
    n.tag = "select".into();
    n.attrs.remove("type");
    n.data.insert("act".into(), format!("select:{target}"));
    n.data.insert("label".into(), label.to_string());
    n.data.insert("options".into(), options.map(|o| format!("{o}={o}")).collect::<Vec<_>>().join("|"));
    n.value = None;
    n.kids.clear();
}

/// The EFB app's own `.select-list` placement: the select's left and width,
/// below it unless there is more room above, at most the room there less 8 px.
pub(crate) fn select_list_style(anchor: Option<Rect>, items: usize) -> String {
    const ITEM_H: f64 = 35.0;
    let r = anchor.unwrap_or(Rect::new(500.0, 200.0, 300.0, 40.0));
    let below = super::doc::VIEW_H - r.bottom();
    let height = (items as f64 * ITEM_H + 2.0).min(below.max(r.y) - 8.0).max(0.0);
    let top = if below >= height || below >= r.y { r.bottom() } else { r.y - height };
    format!("left:{}px;width:{}px;max-height:{}px;top:{}px", r.x, r.w, height, top)
}

fn decorate_home(mut page: Node) -> Node {
    visit(&mut page, &mut |n| {
        if n.tag == "button" && n.cls.is_empty() && n.kids.iter().any(|k| matches!(k, Kid::Text { text } if text == "Open flight plan")) {
            n.data.insert("act".into(), "tab:flightplan".into());
        }
        if has(n, "home-rem-col") {
            let first = n.kids.iter().any(|k| matches!(k, Kid::Node(c) if c.kids.iter().any(|t| matches!(t, Kid::Text { text } if text == "Checklists"))));
            for b in nodes_mut(n) {
                if has(b, "home-rem-row") {
                    let act = if first || has(b, "pin") { "tab:checklists" } else { "tab:maintenance" };
                    b.data.insert("act".into(), act.into());
                }
            }
        }
    });
    page
}

fn has(n: &Node, c: &str) -> bool {
    n.cls.iter().any(|x| x == c)
}

fn nodes_mut(n: &mut Node) -> impl Iterator<Item = &mut Node> {
    n.kids.iter_mut().filter_map(|k| if let Kid::Node(c) = k { Some(c) } else { None })
}

fn find_cls<'a>(n: &'a mut Node, c: &str) -> Option<&'a mut Node> {
    if has(n, c) {
        return Some(n);
    }
    for k in nodes_mut(n) {
        if let Some(f) = find_cls(k, c) {
            return Some(f);
        }
    }
    None
}

fn visit(n: &mut Node, f: &mut dyn FnMut(&mut Node)) {
    f(n);
    for k in nodes_mut(n) {
        visit(k, f);
    }
}

fn maintenance_of(body: &str) -> Option<Maintenance> {
    let d: Value = serde_json::from_str(body).ok()?;
    let len = |k: &str| d.get(k).and_then(Value::as_array).map_or(0, Vec::len);
    Some(Maintenance {
        legal: d.pointer("/dispatch/legal") != Some(&Value::Bool(false)),
        blocking: d.pointer("/dispatch/blocking").and_then(Value::as_array).map(|a| a.iter().map(jstr).collect()).unwrap_or_default(),
        defects: len("defects"),
        deferrals: len("deferrals"),
        tech_log: len("techLog"),
        airframe_hours: d.get("airframeHours").and_then(Value::as_f64).filter(|h| *h != 0.0),
    })
}

fn saves_kind(s: &Value) -> String {
    let r = s.get("reason");
    match r {
        None | Some(Value::Null) => "Unreadable".into(),
        Some(Value::String(r)) => match r.as_str() {
            "manual" => "Manual",
            "autosave" => "Autosave",
            "quit" => "On quit",
            _ => "",
        }
        .into(),
        Some(o) => match o.get("phase").and_then(Value::as_str) {
            Some("pushback") => "Pushback",
            Some("takeoff") => "Take-off",
            Some("top_of_descent") => "Top of descent",
            Some("landing") => "Landing",
            Some("gate") => "At the gate",
            Some(_) => "Phase",
            None => "",
        }
        .into(),
    }
}

fn saves_summary(s: &Value) -> String {
    let m = s.get("summary").cloned().unwrap_or(json!({}));
    let st = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let mut parts = Vec::new();
    if !st("origin").is_empty() || !st("destination").is_empty() {
        let or = |v: String| if v.is_empty() { "----".to_string() } else { v };
        parts.push(format!("{}-{}", or(st("origin")), or(st("destination"))));
    }
    let ground = m.get("on_ground").and_then(Value::as_bool).unwrap_or(false);
    if ground {
        parts.push("On ground".into());
    } else if let Some(a) = m.get("altitude_ft").and_then(Value::as_f64) {
        parts.push(format!("{} ft", (a / 100.0).round() as i64 * 100));
    }
    if let (Some(k), false) = (m.get("ias_kt").and_then(Value::as_f64), ground) {
        parts.push(format!("{} kt", k.round() as i64));
    }
    if !st("flight_phase").is_empty() {
        parts.push(st("flight_phase").to_uppercase());
    }
    parts.join(" \u{b7} ")
}

fn urlencode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// `YYYY-MM-DD HH:MMZ` for a Unix time.
pub fn utc_date(unix: f64) -> String {
    let secs = unix.max(0.0) as i64;
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}Z", rem / 3600, rem % 3600 / 60)
}

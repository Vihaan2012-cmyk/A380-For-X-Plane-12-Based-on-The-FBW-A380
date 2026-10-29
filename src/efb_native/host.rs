use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use efb_pages::model::{StatusLine, StatusModel};
use serde_json::{json, Map, Value};

use super::engine::{Backend, Efb};
use super::hit::Rect;
use crate::settings_files as files;

pub const DOM_TOPIC: &str = "a380.efb.dom";
pub const SCROLL_TOPIC: &str = "a380.efb.scroll";
const JSON_ENCODED: &[&str] = &["ACARS_PROVIDER", "CONFIG_ATIS_SRC"];

/// The EFB's view of the plugin: the same functions `panel.rs` serves over
/// HTTP and the settings files `app/src/settings.rs` writes, called in-process.
pub struct PluginBackend {
    root: Option<PathBuf>,
    clipboard: String,
}

fn read_json(path: &Path) -> Map<String, Value> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

fn write_json(path: &Path, map: &Map<String, Value>) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&Value::Object(map.clone())).map_err(|e| e.to_string())?;
    let partial = path.with_extension("json.part");
    std::fs::write(&partial, text).map_err(|e| e.to_string())?;
    std::fs::rename(&partial, path).map_err(|e| e.to_string())
}

/// What `getSettings` answered: FlyByWire's stored keys and `xphfbw.*`.
pub fn load_settings(root: &Path) -> Map<String, Value> {
    let mut out = Map::new();
    let prefix = files::stored_key("");
    for (k, v) in read_json(&files::datastore_path(root)) {
        let Some(key) = k.strip_prefix(&prefix) else { continue };
        let text = v.as_str().unwrap_or_default();
        let value = serde_json::from_str::<String>(text).unwrap_or_else(|_| text.to_string());
        out.insert(key.to_string(), Value::String(value));
    }
    if let Ok(text) = std::fs::read_to_string(files::flypad_ini_path(root)) {
        for (k, v) in files::parse_ini(&text) {
            if let Some(key) = k.strip_prefix(&prefix) {
                out.insert(key.to_string(), Value::String(v));
            }
        }
    }
    for (k, v) in read_json(&files::app_settings_path(root)) {
        out.insert(format!("xphfbw.{k}"), v);
    }
    out
}

/// What `setSettings` did, under the settings-files mutex (rule 8).
pub fn save_settings(root: &Path, values: &Map<String, Value>) -> Result<(), String> {
    crate::app_settings::with_settings_lock(|| {
        let mut datastore = read_json(&files::datastore_path(root));
        let ini_path = files::flypad_ini_path(root);
        let mut ini: BTreeMap<String, String> = std::fs::read_to_string(&ini_path).map(|t| files::parse_ini(&t)).unwrap_or_default();
        let mut app = read_json(&files::app_settings_path(root));
        let (mut ds, mut ii, mut ap) = (false, false, false);
        for (key, value) in values {
            if let Some(own) = key.strip_prefix("xphfbw.") {
                app.insert(own.to_string(), value.clone());
                ap = true;
                continue;
            }
            let text = match value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let stored = files::stored_key(key);
            let encoded = if JSON_ENCODED.contains(&key.as_str()) { Value::String(text.clone()).to_string() } else { text.clone() };
            datastore.insert(stored.clone(), Value::String(encoded));
            ds = true;
            if files::flypad_owns(key) {
                ini.insert(stored, text);
                ii = true;
            }
        }
        if ds {
            write_json(&files::datastore_path(root), &datastore)?;
        }
        if ii {
            if let Some(dir) = ini_path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::write(&ini_path, files::write_ini(&ini)).map_err(|e| e.to_string())?;
        }
        if ap {
            write_json(&files::app_settings_path(root), &app)?;
        }
        Ok(())
    })
}

impl Backend for PluginBackend {
    fn load_settings(&mut self) -> Map<String, Value> {
        self.root.as_deref().map(load_settings).unwrap_or_default()
    }

    fn save_settings(&mut self, values: &Map<String, Value>) -> Result<(), String> {
        let root = self.root.clone().ok_or("X-Plane's folder is not known yet")?;
        save_settings(&root, values)
    }

    fn route(&mut self, method: &str, path: &str, body: &str) -> Option<(u16, String)> {
        if let Some(r) = crate::state_save::route(method, path, body) {
            return Some(r);
        }
        match (method, path) {
            ("GET", "/efb/landings") => Some((200, crate::landing_report::history_json())),
            ("GET", "/study/maintenance") => Some((200, crate::study::web::maintenance_json())),
            ("POST", "/study/action") => Some(match crate::study::web::apply_action(body) {
                Ok(()) => (200, "{\"ok\":true}".into()),
                Err(m) => (400, json!({ "ok": false, "message": m }).to_string()),
            }),
            _ => None,
        }
    }

    fn app_action(&mut self, name: &str) -> Option<String> {
        Some(format!("{name}: needs the A380 for X-Plane 12 app"))
    }

    fn fetch(&mut self, url: &str) -> Option<(u16, String)> {
        crate::net::poll(&format!("efb-native GET {url}"), "GET", url, "")
    }

    fn status(&mut self) -> StatusModel {
        let line = |class: &str, text: &str| StatusLine { class: class.into(), text: text.into() };
        StatusModel { xplane: line("ok", "X-Plane: connected"), systems: line("ok", "Systems: running"), displays: line("ok", "Displays: native") }
    }

    fn clipboard_get(&mut self) -> String {
        self.clipboard.clone()
    }

    fn clipboard_set(&mut self, text: &str) {
        self.clipboard = text.to_string();
    }

    fn keyboard_capture(&mut self, on: bool) {
        crate::display::set_efb_text_focus(on);
    }
}

struct Host {
    efb: Efb<PluginBackend>,
    started: Instant,
    published_scroll: String,
    var_names: Vec<String>,
    vars_at: f64,
    pb_at: f64,
    gc_at: f64,
    ground_at: f64,
    fap_at: f64,
}

thread_local! {
    static HOST: std::cell::RefCell<Option<Host>> = const { std::cell::RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Host) -> R) -> Option<R> {
    HOST.with(|cell| {
        let mut g = cell.try_borrow_mut().ok()?;
        if g.is_none() {
            let backend = PluginBackend { root: crate::xp::system_path(), clipboard: String::new() };
            *g = Some(Host { efb: Efb::new(backend), started: Instant::now(), published_scroll: String::new(), var_names: Vec::new(), vars_at: f64::NEG_INFINITY, pb_at: f64::NEG_INFINITY, gc_at: f64::NEG_INFINITY, ground_at: f64::NEG_INFINITY, fap_at: f64::NEG_INFINITY });
        }
        g.as_mut().map(f)
    })
}

fn clock() -> (String, String) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    let d = super::engine::utc_date(now);
    let hm = &d[11..16];
    (format!("{hm}Z"), format!("{hm} LT"))
}

fn publish(topic: &str, data: Value) {
    let batch = json!({ "events": [{ "topic": topic, "data": data }] });
    crate::native_displays::inputs::tap_generic_data("xp-ui.bus", &batch.to_string());
}

/// Once per flight loop: advance the EFB and publish what changed.
pub fn tick(get: &dyn Fn(&str) -> Option<f64>) {
    with(|h| {
        take_boxes(h);
        let now = h.started.elapsed().as_secs_f64() * 1000.0;
        h.efb.clock = clock();
        if h.efb.tab == "study" && now - h.vars_at >= 1000.0 {
            h.vars_at = now;
            if h.var_names.is_empty() {
                h.var_names = h.efb.study_var_names();
            }
            h.efb.study_vars = h.var_names.iter().filter_map(|n| Some((n.clone(), Value::from(get(n)?)))).collect();
        }
        if now - h.gc_at >= 1000.0 {
            h.gc_at = now;
            let mut names = super::ground_eng::var_names();
            names.extend(super::engine::PB_VARS.iter().map(|s| s.to_string()));
            let vars = names.into_iter().filter_map(|n| Some((n.clone(), get(&n)?))).collect();
            let strip = |prefix: &str| crate::published::numbers_named(prefix).into_iter().map(|(k, v, _)| (k[prefix.len()..].to_string(), v)).collect();
            h.efb.ground_tick(now, vars, strip("fbw/gse/"), strip("fbw/efb/interphone/"));
        }
        if h.efb.tab == "ground" && now - h.ground_at >= 500.0 {
            h.ground_at = now;
            let names = h.efb.ground_var_names();
            let vars: Map<String, Value> = names.into_iter().filter_map(|n| Some((n.clone(), Value::from(get(&n)?)))).collect();
            if vars != h.efb.ground_vars {
                h.efb.ground_vars = vars;
                h.efb.touch();
            }
        }
        if h.efb.fap_wants_vars() && now - h.fap_at >= 500.0 {
            h.fap_at = now;
            let vars = efb_pages::fap::fap_names().into_iter().filter_map(|n| Some((n.clone(), Value::from(get(&n)?)))).collect();
            h.efb.fap_update(vars);
        }
        if h.efb.pb_wants_vars() && now - h.pb_at >= 200.0 {
            h.pb_at = now;
            let vars = super::engine::PB_VARS.iter().filter_map(|n| Some((n.to_string(), Value::from(get(n)?)))).collect();
            h.efb.pb_update(vars);
        }
        if !h.efb.tick(now) {
            return;
        }
        if let Some(html) = h.efb.document() {
            publish(DOM_TOPIC, Value::String(html.to_string()));
        }
        let scroll = h.efb.scroll_json();
        if scroll != h.published_scroll {
            publish(SCROLL_TOPIC, serde_json::from_str(&scroll).unwrap_or(Value::Null));
            h.published_scroll = scroll;
        }
    });
}

static BOXES: Mutex<Option<Vec<Rect>>> = Mutex::new(None);

/// The efbx display's boxes for the published document (`a380d_boxes`, see
/// the plan), element order, CSS pixels. Any thread (the render thread
/// draws the EFB); the flight loop picks them up on its next call.
pub fn set_boxes(xywh: &[[f32; 4]]) {
    let rects = xywh.iter().map(|b| Rect::new(b[0] as f64, b[1] as f64, b[2] as f64, b[3] as f64)).collect();
    *BOXES.lock().unwrap_or_else(|e| e.into_inner()) = Some(rects);
}

fn take_boxes(h: &mut Host) {
    if let Some(b) = BOXES.lock().unwrap_or_else(|e| e.into_inner()).take() {
        h.efb.set_boxes(b);
    }
}

/// A pointer event on the EFB in CSS pixels (kinds as `a380d_pointer`).
pub fn pointer(x: f64, y: f64, kind: i32, button: i32) -> bool {
    with(|h| {
        take_boxes(h);
        let now = h.started.elapsed().as_secs_f64() * 1000.0;
        h.efb.tick(now);
        h.efb.pointer(x, y, kind, button)
    })
    .unwrap_or(false)
}

/// An X-Plane key callback while the EFB has the keyboard.
pub fn key(ch: u32, vkey: u32, flags: u32) -> bool {
    with(|h| h.efb.key(ch, vkey, flags)).unwrap_or(false)
}

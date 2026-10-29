//! The native MFD (crates/mfd_host): page trees from the native FMS host and
//! variables, published on the topic mfdx reads instead of the CEF DOM snapshot.
//! When both exist the CEF snapshot is compared with the native one, log only.

use std::collections::HashMap;
use std::sync::Mutex;

use fms::navdb::NavDatabase;
use fms_host::{FmsHost, Snapshot};
use mfd_host::{Key, Mfd, Out, Sources};
use serde_json::{json, Value};

use crate::native_displays::inputs::MFD_DOM_TOPIC;

struct Host {
    mfd: Mfd,
    doc: Option<std::sync::mpsc::Sender<mfd_tree::Node>>,
    published: std::sync::Arc<Mutex<String>>,
    snapshot: Option<Snapshot>,
    pending: Vec<Out>,
    mismatches: u64,
}

thread_local! {
    static HOST: std::cell::RefCell<Option<Host>> = const { std::cell::RefCell::new(None) };
}
static BOXES: Mutex<Option<Vec<[f32; 4]>>> = Mutex::new(None);
static HOVER: Mutex<(Option<String>, bool)> = Mutex::new((None, false));

fn mark_hover(n: &mut mfd_tree::Node, act: &str) -> bool {
    if n.attrs.get(mfd_host::tree::ACT).map(String::as_str) == Some(act) {
        n.attrs.insert("data-hover".into(), String::new());
        return true;
    }
    for k in n.kids.iter_mut() {
        if let mfd_tree::Kid::Node(c) = k {
            if mark_hover(c, act) {
                n.attrs.insert("data-hover".into(), String::new());
                return true;
            }
        }
    }
    false
}

/// The pointer moved over the MFD (CSS pixels).
pub fn hover(x: f64, y: f64) {
    let act = with(|h| h.mfd.act_at(x, y)).flatten();
    let mut g = HOVER.lock().unwrap_or_else(|e| e.into_inner());
    if g.0 != act {
        *g = (act, true);
    }
}
static VARS: Mutex<Option<HashMap<String, f64>>> = Mutex::new(None);

/// The company flight plan request (FlightManagementComputer.cpnyFplnRequest): the OFP
/// download runs off the flight loop; `tick` uplinks it when it lands.
struct Cpny {
    plan: usize,
    download: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
}

static CPNY: Mutex<Option<Cpny>> = Mutex::new(None);

fn simbrief_user() -> Option<String> {
    let root = crate::xp::system_path()?;
    let settings = crate::efb_native::host::load_settings(&root);
    ["CONFIG_OVERRIDE_SIMBRIEF_USERID", "NAVIGRAPH_USERNAME"]
        .iter()
        .filter_map(|k| settings.get(*k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string))
        .next()
}

fn cpny_request(plan: usize) -> Vec<(String, f64)> {
    let Some(user) = simbrief_user() else {
        crate::log("native mfd: CPNY F-PLN REQUEST: no SimBrief user ID in the EFB settings");
        return Vec::new();
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let url = format!("https://www.simbrief.com/api/xml.fetcher.php?userid={user}&json=1");
    let saved_ofp = if offline() { std::env::var("FBW_OFFLINE_OFP").ok() } else { None };
    let started = std::thread::Builder::new().name("fbw-cpny-fpln".into()).spawn(move || {
        let r = match saved_ofp {
            Some(path) => std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}")),
            None => match crate::net::request("GET", &url, "") {
                Some((200, body)) => Ok(body),
                Some((status, _)) => Err(format!("HTTP {status}")),
                None => Err("no response".into()),
            },
        };
        let _ = tx.send(r);
    });
    if started.is_err() {
        return Vec::new();
    }
    if let Ok(mut c) = CPNY.lock() {
        *c = Some(Cpny { plan, download: Some(rx) });
    }
    vec![(mfd_host::model::CPNY_FPLN_STATE.into(), 1.), (mfd_host::model::CPNY_FPLN_PLAN.into(), plan as f64)]
}

fn cpny_poll(fms: &mut FmsHost) -> Vec<(String, f64)> {
    let Ok(mut guard) = CPNY.lock() else { return Vec::new() };
    let Some(job) = guard.as_mut() else { return Vec::new() };
    let Some(rx) = &job.download else { return Vec::new() };
    let result = match rx.try_recv() {
        Ok(r) => r,
        Err(std::sync::mpsc::TryRecvError::Empty) => return Vec::new(),
        Err(std::sync::mpsc::TryRecvError::Disconnected) => Err("download stopped".into()),
    };
    job.download = None;
    match result.and_then(|body| fms::uplink::Ofp::from_simbrief_json(&body)) {
        Ok(ofp) => {
            fms.send(fms_host::HostCommand::UplinkSimbrief(ofp));
            vec![(mfd_host::model::CPNY_FPLN_STATE.into(), 2.)]
        }
        Err(e) => {
            crate::log(&format!("native mfd: CPNY F-PLN REQUEST failed: {e}"));
            *guard = None;
            vec![(mfd_host::model::CPNY_FPLN_STATE.into(), 0.)]
        }
    }
}

fn cpny_clear(fms: &mut FmsHost) -> Vec<(String, f64)> {
    if let Ok(mut c) = CPNY.lock() {
        *c = None;
    }
    fms.send(fms_host::HostCommand::Plan(fms::service::Command::UplinkDelete));
    vec![(mfd_host::model::CPNY_FPLN_STATE.into(), 0.)]
}

fn cpny_insert(fms: &mut FmsHost) -> Vec<(String, f64)> {
    let plan = CPNY.lock().ok().and_then(|mut c| c.take()).map(|c| c.plan);
    if plan.is_none() {
        return Vec::new();
    }
    fms.send(fms_host::HostCommand::Plan(fms::service::Command::UplinkInsert));
    vec![(mfd_host::model::CPNY_FPLN_STATE.into(), 0.)]
}

/// The last value `tick` read for `name`, for input outside the flight loop.
pub fn cached_var(name: &str) -> f64 {
    VARS.lock().ok().and_then(|v| v.as_ref()?.get(name).copied()).unwrap_or(0.)
}

fn with<R>(f: impl FnOnce(&mut Host) -> R) -> Option<R> {
    HOST.try_with(|cell| {
        let mut g = cell.try_borrow_mut().ok()?;
        let h = g.get_or_insert_with(|| {
            let html_ui = crate::js_bridge::plugin_dir()
                .and_then(|p| Some(p.parent()?.parent()?.join(crate::display::HTML_UI_DIR)))
                .or_else(|| offline().then(|| std::env::var_os("FBW_HTML_UI").map(std::path::PathBuf::from)).flatten());
            let published = std::sync::Arc::new(Mutex::new(String::new()));
            let doc = html_ui.and_then(|d| doc_worker(d, published.clone()));
            Host { mfd: Mfd::default(), doc, published, snapshot: None, pending: Vec::new(), mismatches: 0 }
        });
        if let Some(b) = BOXES.lock().ok().and_then(|mut b| b.take()) {
            h.mfd.boxes = b;
        }
        Some(f(h))
    })
    .ok()
    .flatten()
}

/// Styles and serialises the newest page on its own thread: a full document takes tens of
/// milliseconds, too long for the flight loop.
fn doc_worker(html_ui: std::path::PathBuf, published: std::sync::Arc<Mutex<String>>) -> Option<std::sync::mpsc::Sender<mfd_tree::Node>> {
    let (tx, rx) = std::sync::mpsc::channel::<mfd_tree::Node>();
    std::thread::Builder::new()
        .name("fbw-mfd-doc".into())
        .spawn(move || {
            let Ok(native) = mfd_native::Native::load(&html_ui.to_string_lossy(), 1.) else { return };
            let mut last = String::new();
            while let Ok(mut tree) = rx.recv() {
                while let Ok(newer) = rx.try_recv() {
                    tree = newer;
                }
                let html = native.document(&tree);
                if html != last {
                    publish(&html);
                    if let Ok(mut p) = published.lock() {
                        p.clone_from(&html);
                    }
                    last = html;
                }
            }
        })
        .ok()?;
    Some(tx)
}

fn publish(html: &str) {
    let batch = json!({ "events": [{ "topic": MFD_DOM_TOPIC, "data": Value::String(html.to_string()) }] });
    crate::native_displays::inputs::tap_generic_data("xp-ui.bus", &batch.to_string());
}

/// Once per flight loop: send queued commands to the FMS host, rebuild and publish.
/// Returns the variable writes the pages asked for.
const REBUILD_PERIOD: std::time::Duration = std::time::Duration::from_millis(100);

thread_local! {
    static LAST_BUILD: std::cell::Cell<Option<std::time::Instant>> = const { std::cell::Cell::new(None) };
    static LAST_TREE: std::cell::RefCell<Option<mfd_tree::Node>> = const { std::cell::RefCell::new(None) };
}

pub fn tick(get: &dyn Fn(&str) -> f64, fms: Option<&mut FmsHost>, messages: &[String]) -> Vec<(String, f64)> {
    let seen = std::cell::RefCell::new(HashMap::new());
    let get = |name: &str| {
        let v = get(name);
        seen.borrow_mut().insert(name.to_string(), v);
        v
    };
    let get: &dyn Fn(&str) -> f64 = &get;
    let out = with(|h| {
        let mut writes = Vec::new();
        for act in std::mem::take(&mut *OFFLINE_ACTS.lock().unwrap_or_else(|e| e.into_inner())) {
            let snap = h.snapshot.clone().unwrap_or_else(|| Snapshot { plans: vec![None; 6], ..Default::default() });
            let src = Sources::from_snapshot(&snap, get, &[]);
            let out = h.mfd.act(&src, &act);
            h.pending.extend(out);
        }
        let hover_changed = std::mem::take(&mut HOVER.lock().unwrap_or_else(|e| e.into_inner()).1);
        let had_input = !h.pending.is_empty() || hover_changed;
        if let Some(fms) = fms {
            for o in h.pending.drain(..) {
                match o {
                    Out::Fms(c) => {
                        fms.send(c);
                    }
                    Out::Var(n, v) => writes.push((n, v)),
                    Out::CpnyFplnRequest(plan) => writes.extend(cpny_request(plan)),
                    Out::CpnyFplnInsert(_) => writes.extend(cpny_insert(fms)),
                    Out::CpnyFplnClear => writes.extend(cpny_clear(fms)),
                    Out::KeyEvent(k) => crate::key_events::push(&k, &[0.]),
                }
            }
            writes.extend(cpny_poll(fms));
            if let Some(s) = fms.snapshot() {
                h.snapshot = Some(s);
            }
        }
        let due = had_input || offline() || LAST_BUILD.with(|t| t.get().is_none_or(|t| t.elapsed() >= REBUILD_PERIOD));
        if !due {
            return writes;
        }
        LAST_BUILD.with(|t| t.set(Some(std::time::Instant::now())));
        let snap = h.snapshot.clone().unwrap_or_else(|| Snapshot { plans: vec![None; 6], ..Default::default() });
        let src = Sources::from_snapshot(&snap, get, messages);
        let mut tree = h.mfd.build(&src).clone();
        if let Some(act) = HOVER.lock().unwrap_or_else(|e| e.into_inner()).0.clone() {
            mark_hover(&mut tree, &act);
        }
        if LAST_TREE.with(|t| t.borrow().as_ref() == Some(&tree)) {
            return writes;
        }
        if let Some(doc) = &h.doc {
            let _ = doc.send(tree.clone());
        }
        LAST_TREE.with(|t| *t.borrow_mut() = Some(tree));
        writes
    })
    .unwrap_or_default();
    if let Ok(mut v) = VARS.lock() {
        v.get_or_insert_with(HashMap::new).extend(seen.into_inner());
    }
    out
}

/// mfdx's boxes for the published document (element order, CSS pixels). Any thread.
pub fn set_boxes(xywh: &[[f32; 4]]) {
    if let Ok(mut b) = BOXES.lock() {
        *b = Some(xywh.to_vec());
    }
}

/// A click on the MFD in CSS pixels.
pub fn pointer(x: f64, y: f64, get: &dyn Fn(&str) -> f64) -> bool {
    thread_local! {
        static LAST_PRESS: std::cell::Cell<Option<(f64, f64, std::time::Instant)>> = const { std::cell::Cell::new(None) };
    }
    let repeat = LAST_PRESS.with(|l| l.get().is_some_and(|(px, py, t)| (px - x).abs() < 2. && (py - y).abs() < 2. && t.elapsed() < std::time::Duration::from_millis(150)));
    LAST_PRESS.with(|l| l.set(Some((x, y, std::time::Instant::now()))));
    if repeat {
        return true;
    }
    with(|h| {
        let snap = h.snapshot.clone().unwrap_or_else(|| Snapshot { plans: vec![None; 6], ..Default::default() });
        let src = Sources::from_snapshot(&snap, get, &[]);
        let out = h.mfd.pointer(&src, x, y);
        if out.is_empty() {
            let under: Vec<String> = h
                .mfd
                .boxes
                .iter()
                .enumerate()
                .filter(|(_, b)| x >= b[0] as f64 && y >= b[1] as f64 && x < (b[0] + b[2]) as f64 && y < (b[1] + b[3]) as f64)
                .rev()
                .take(4)
                .map(|(i, b)| {
                    let f = h.mfd.flat.get(i);
                    format!("#{i} [{:.0},{:.0} {:.0}x{:.0}] act={:?} hidden={}", b[0], b[1], b[2], b[3], f.and_then(|f| f.act.clone()), f.is_some_and(|f| f.hidden))
                })
                .collect();
            let first = h.mfd.boxes.first().map(|b| format!("[{:.0},{:.0} {:.0}x{:.0}]", b[0], b[1], b[2], b[3])).unwrap_or_default();
            crate::log(&format!("native mfd: click at ({x:.0},{y:.0}) sent no FMS command (elements {}, boxes {}, root {first}); under: {}", h.mfd.flat.len(), h.mfd.boxes.len(), under.join(" | ")));
        }
        h.pending.extend(out);
        crate::display::set_mfd_text_focus(h.mfd.keyboard());
        true
    })
    .unwrap_or(false)
}

static OFFLINE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the MFD runs with no screen (the offline flight harness).
pub fn offline() -> bool {
    OFFLINE.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(feature = "test-support")]
pub fn set_offline(on: bool) {
    OFFLINE.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Press the element with `data-act` = `act` (as `"c|btn:INIT"`), for the offline harness.
#[cfg(feature = "test-support")]
pub fn offline_act(act: &str, _get: &dyn Fn(&str) -> f64) {
    OFFLINE_ACTS.lock().unwrap_or_else(|e| e.into_inner()).push(act.to_string());
}

/// Presses queued by `offline_act`, applied in `tick` with the variables an on-screen click sees.
static OFFLINE_ACTS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The newest FMS snapshot and the acts on the page each side shows, for the offline harness.
#[cfg(feature = "test-support")]
pub fn offline_view() -> Option<(Option<Snapshot>, Vec<String>)> {
    with(|h| (h.snapshot.clone(), h.mfd.flat.iter().filter_map(|f| f.act.clone()).collect()))
}

/// The page each side shows (`fms/active/init`), for the offline harness.
#[cfg(feature = "test-support")]
pub fn offline_pages() -> Option<(String, String)> {
    with(|h| (h.mfd.capt.uri.uri.clone(), h.mfd.fo.uri.uri.clone()))
}

/// An X-Plane key while the MFD has the keyboard; `false` passes it on.
pub fn key(ch: u32, vkey: u32, db: Option<&dyn NavDatabase>, get: &dyn Fn(&str) -> f64) -> bool {
    with(|h| {
        if !h.mfd.keyboard() {
            return false;
        }
        let k = match vkey {
            0x1B => Key::Escape,
            0x0D => Key::Enter,
            0x08 => Key::Backspace,
            0x2E => Key::Delete,
            _ => match char::from_u32(ch) {
                Some(c) if !c.is_control() => Key::Char(c),
                _ => return true,
            },
        };
        let snap = h.snapshot.clone().unwrap_or_else(|| Snapshot { plans: vec![None; 6], ..Default::default() });
        let src = Sources::from_snapshot(&snap, get, &[]);
        let (used, out) = h.mfd.key(&src, db, k);
        h.pending.extend(out);
        crate::display::set_mfd_text_focus(h.mfd.keyboard());
        used
    })
    .unwrap_or(false)
}

fn visible_text(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_tag = false;
    let mut cur = String::new();
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                let t = cur.split_whitespace().collect::<Vec<_>>().join(" ");
                if !t.is_empty() {
                    out.push(t);
                }
                cur.clear();
            }
            '>' => in_tag = false,
            _ if !in_tag => cur.push(c),
            _ => {}
        }
    }
    out
}

/// The CEF view's MFD DOM batch. Returns `true` when the native MFD publishes
/// instead, so the caller drops it; the texts of both are compared, log only.
pub fn observe_cef(batch: &str) -> bool {
    with(|h| {
        let published = h.published.lock().map(|p| p.clone()).unwrap_or_default();
        if h.doc.is_none() || published.is_empty() {
            return false;
        }
        let html = serde_json::from_str::<Value>(batch)
            .ok()
            .and_then(|v| v["events"][0]["data"].as_str().map(str::to_string))
            .unwrap_or_default();
        let (a, b) = (visible_text(&html), visible_text(&published));
        if a != b {
            h.mismatches += 1;
            if h.mismatches == 1 || h.mismatches % 600 == 0 {
                let first = a.iter().zip(b.iter()).find(|(x, y)| x != y);
                crate::log(&format!("native shadow: mfd text differs x{} first cef={:?} native={:?}", h.mismatches, first.map(|f| f.0), first.map(|f| f.1)));
            }
        }
        true
    })
    .unwrap_or(false)
}

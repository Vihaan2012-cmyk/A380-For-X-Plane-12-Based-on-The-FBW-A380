//! The plugin's local server for pages in X-Plane's own browser: the files
//! XPHFBW's `coui://` scheme serves, the requests its renderer answers, and a
//! WebSocket per page carrying variables, events and calls. It talks to
//! `XphfbwHost` through the same shared-memory `Session` XPHFBW's renderers
//! use, so the frame loop is the same either way and nothing here runs on
//! X-Plane's main thread.

use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv6Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::efb_web::{self, Paths};
use crate::source_patch::SourcePatch;
use crate::xphfbw_bridge::{Downlink, Session, Uplink, MAX_VIEWS};

use super::Layout;

const SHIM: &str = include_str!("shim.js");
const PLUGIN_PORT: &str = "127.0.0.1:8390";
const GAME_STRING_WAIT: Duration = Duration::from_secs(5);
pub const EFB_VIEW: u32 = 31;

/// What one aircraft session serves.
pub struct Gate {
    pub generation: u64,
    session: Mutex<Session>,
    paths: Paths,
    patches: Vec<SourcePatch>,
    layout: Layout,
    files: Mutex<HashMap<String, Arc<Vec<u8>>>>,
    pages: Vec<ViewBox>,
}

#[derive(Default)]
struct ViewBox {
    stash: Mutex<Vec<Downlink>>,
    games: Mutex<HashMap<String, String>>,
    connected: AtomicBool,
    fps: AtomicU64,
    sent_bytes: AtomicU64,
}


static RUNNING: AtomicBool = AtomicBool::new(false);
static PORT: AtomicUsize = AtomicUsize::new(0);
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
static NEXT_SOCKET: AtomicU64 = AtomicU64::new(0);
static GENERATION: AtomicU64 = AtomicU64::new(0);

fn sockets() -> &'static Mutex<HashMap<u64, TcpStream>> {
    static S: OnceLock<Mutex<HashMap<u64, TcpStream>>> = OnceLock::new();
    S.get_or_init(Mutex::default)
}

fn gate_cell() -> &'static Mutex<Option<Arc<Gate>>> {
    static G: OnceLock<Mutex<Option<Arc<Gate>>>> = OnceLock::new();
    G.get_or_init(Mutex::default)
}

fn gate() -> Option<Arc<Gate>> {
    gate_cell().lock().ok()?.clone()
}

struct Worker;

impl Worker {
    fn new() -> Self {
        ACTIVE.fetch_add(1, Ordering::SeqCst);
        Self
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
    }
}

fn spawn(name: &str, f: impl FnOnce() + Send + 'static) {
    let worker = Worker::new();
    let run = move || {
        let _worker = worker;
        f();
    };
    if std::thread::Builder::new().name(name.to_string()).spawn(run).is_err() {
        super::log("could not start a server thread");
    }
}

/// Listens on loopback on both address families: `*.localhost` names try ::1
/// first, and Windows takes a second to refuse that when nothing is there.
pub fn start() -> Option<u16> {
    if RUNNING.load(Ordering::SeqCst) {
        return Some(PORT.load(Ordering::SeqCst) as u16);
    }
    let v4 = TcpListener::bind(("127.0.0.1", 0)).ok()?;
    let port = v4.local_addr().ok()?.port();
    let v6 = TcpListener::bind((Ipv6Addr::LOCALHOST, port)).ok();
    v4.set_nonblocking(true).ok()?;
    if let Some(l) = &v6 {
        l.set_nonblocking(true).ok()?;
    }
    PORT.store(port as usize, Ordering::SeqCst);
    RUNNING.store(true, Ordering::SeqCst);
    spawn("fbw-browser-listen", move || {
        while RUNNING.load(Ordering::SeqCst) {
            let mut idle = true;
            for listener in std::iter::once(&v4).chain(v6.iter()) {
                if let Ok((stream, _)) = listener.accept() {
                    idle = false;
                    spawn("fbw-browser-conn", move || {
                        let _ = connection(stream);
                    });
                }
            }
            if idle {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    });
    Some(port)
}

/// Every thread ends before this returns: the DLL unloads right after.
pub fn stop() {
    RUNNING.store(false, Ordering::SeqCst);
    if let Ok(mut g) = gate_cell().lock() {
        *g = None;
    }
    if let Ok(list) = sockets().lock() {
        for s in list.values() {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    }
    let until = Instant::now() + Duration::from_secs(5);
    while ACTIVE.load(Ordering::SeqCst) > 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub fn attach(tag: &str, paths: Paths, patches: Vec<SourcePatch>, layout: Layout) -> bool {
    let Some(session) = Session::open(tag) else { return false };
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let pages = (0..MAX_VIEWS).map(|_| ViewBox::default()).collect();
    let gate = Gate { generation, session: Mutex::new(session), paths, patches, layout, files: Mutex::default(), pages };
    if let Ok(mut g) = gate_cell().lock() {
        *g = Some(Arc::new(gate));
    }
    true
}

pub fn detach() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    if let Ok(mut g) = gate_cell().lock() {
        *g = None;
    }
}

pub fn view_connected(view: u32) -> bool {
    gate().and_then(|g| g.pages.get(view as usize).map(|p| p.connected.load(Ordering::Relaxed))).unwrap_or(false)
}

pub fn stats() -> Vec<(u32, u64, u64)> {
    let Some(g) = gate() else { return Vec::new() };
    g.pages.iter().enumerate().filter(|(_, p)| p.connected.load(Ordering::Relaxed)).map(|(i, p)| (i as u32, p.fps.load(Ordering::Relaxed), p.sent_bytes.swap(0, Ordering::Relaxed))).collect()
}

struct Request {
    method: String,
    path: String,
    query: String,
    body: Vec<u8>,
    websocket_key: Option<String>,
    keep_alive: bool,
}

fn alive() -> bool {
    RUNNING.load(Ordering::Relaxed)
}

/// `read`, sitting out the short socket timeout that lets a thread notice shutdown.
fn read_some(stream: &mut TcpStream, buf: &mut [u8], idle: &mut Duration, limit: Duration) -> std::io::Result<usize> {
    const TICK: Duration = Duration::from_millis(500);
    loop {
        match stream.read(buf) {
            Ok(n) => return Ok(n),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                *idle += TICK;
                if !alive() || *idle > limit {
                    return Ok(0);
                }
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
}

fn read_request(stream: &mut TcpStream, buf: &mut Vec<u8>) -> std::io::Result<Option<Request>> {
    let mut chunk = [0u8; 16384];
    let mut idle = Duration::ZERO;
    let head_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        let n = read_some(stream, &mut chunk, &mut idle, Duration::from_secs(120))?;
        if n == 0 || buf.len() > 1 << 20 {
            return Ok(None);
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or("").split_whitespace();
    let (method, target) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("/").to_string());
    let (mut length, mut key, mut upgrade, mut keep_alive) = (0usize, None, false, true);
    for line in lines {
        let Some((k, v)) = line.split_once(':') else { continue };
        let v = v.trim();
        match k.trim().to_ascii_lowercase().as_str() {
            "content-length" => length = v.parse().unwrap_or(0),
            "sec-websocket-key" => key = Some(v.to_string()),
            "upgrade" => upgrade = v.eq_ignore_ascii_case("websocket"),
            "connection" => keep_alive = !v.eq_ignore_ascii_case("close"),
            _ => {}
        }
    }
    while buf.len() < head_end + length {
        let n = read_some(stream, &mut chunk, &mut idle, Duration::from_secs(60))?;
        if n == 0 {
            return Ok(None);
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = buf[head_end..head_end + length].to_vec();
    buf.drain(..head_end + length);
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    Ok(Some(Request { method, path: decode(path), query: query.to_string(), body, websocket_key: key.filter(|_| upgrade), keep_alive }))
}

fn connection(mut stream: TcpStream) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    let id = NEXT_SOCKET.fetch_add(1, Ordering::Relaxed);
    if let (Ok(mut list), Ok(clone)) = (sockets().lock(), stream.try_clone()) {
        list.insert(id, clone);
    }
    let result = requests(&mut stream);
    if let Ok(mut list) = sockets().lock() {
        list.remove(&id);
    }
    result
}

fn requests(stream: &mut TcpStream) -> std::io::Result<()> {
    let mut buf = Vec::new();
    while alive() {
        let Some(req) = read_request(stream, &mut buf)? else { return Ok(()) };
        if let Some(key) = &req.websocket_key {
            return websocket(stream, key, &req.query);
        }
        if req.path.starts_with("/__plugin/") {
            return relay(stream, &req);
        }
        let reply = answer(&req);
        let connection = if req.keep_alive { "keep-alive" } else { "close" };
        write!(
            stream,
            "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\nConnection: {connection}\r\n\r\n",
            reply.status,
            reply.kind,
            reply.body.len()
        )?;
        stream.write_all(&reply.body)?;
        if !req.keep_alive {
            return Ok(());
        }
    }
    Ok(())
}

/// The plugin's own HTTP routes, which can hold a request open a long time.
fn relay(stream: &mut TcpStream, req: &Request) -> std::io::Result<()> {
    let target = &req.path["/__plugin".len()..];
    let query = if req.query.is_empty() { String::new() } else { format!("?{}", req.query) };
    let mut plugin = TcpStream::connect(PLUGIN_PORT)?;
    plugin.set_read_timeout(Some(Duration::from_millis(500)))?;
    write!(
        plugin,
        "{} {target}{query} HTTP/1.1\r\nHost: {PLUGIN_PORT}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        req.method,
        req.body.len()
    )?;
    plugin.write_all(&req.body)?;
    let mut chunk = [0u8; 65536];
    let mut idle = Duration::ZERO;
    loop {
        let n = read_some(&mut plugin, &mut chunk, &mut idle, Duration::from_secs(150))?;
        if n == 0 {
            return Ok(());
        }
        idle = Duration::ZERO;
        stream.write_all(&chunk[..n])?;
    }
}

struct Reply {
    status: &'static str,
    kind: &'static str,
    body: Arc<Vec<u8>>,
}

fn reply(status: &'static str, kind: &'static str, body: impl Into<Vec<u8>>) -> Reply {
    Reply { status, kind, body: Arc::new(body.into()) }
}

fn not_found() -> Reply {
    reply("404 Not Found", "text/plain", "not found")
}

fn answer(req: &Request) -> Reply {
    if req.path == "/__b/shim.js" {
        return reply("200 OK", "text/javascript", SHIM);
    }
    let Some(gate) = gate() else { return reply("503 Service Unavailable", "text/plain", "no session") };
    if let Some(rest) = req.path.strip_prefix("/__b/") {
        return bridge_route(&gate, rest, req);
    }
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") | ("GET", "/index.html") => {
            return match std::fs::read_to_string(gate.paths.ui_dir.join("index.html")) {
                Ok(page) => reply("200 OK", "text/html; charset=utf-8", inject_first(&page, r#"<script src="/__b/shim.js"></script>"#)),
                Err(_) => not_found(),
            };
        }
        ("GET", "/api/status") => {
            let screens = gate.pages.iter().filter(|p| p.connected.load(Ordering::Relaxed)).count();
            return reply(
                "200 OK",
                "application/json",
                json!({
                    "xplane": true,
                    "systems": { "ticks": 1, "lastTickMicros": 0, "variables": 0 },
                    "displays": { "screens": screens, "fps": 0 },
                    "cabinIncluded": gate.paths.aircraft_dir.join("objects").join("cabin_included.txt").is_file(),
                })
                .to_string(),
            );
        }
        ("POST", p) if p.starts_with("/api/action/") => {
            let result = efb_web::action(&gate.paths, &p["/api/action/".len()..], &|| super::request_refresh());
            return reply("200 OK", "application/json", result.to_string());
        }
        _ => {}
    }
    if let Some(r) = efb_web::route(&gate.paths, &req.method, &req.path, &req.query, &req.body) {
        return Reply { status: r.status, kind: r.kind, body: Arc::new(r.body) };
    }
    file(&gate, &req.path)
}

fn file(gate: &Gate, clean: &str) -> Reply {
    let kind = mime_of(clean);
    let patched = kind == "text/html" || kind == "text/javascript" || kind == "text/css";
    if patched {
        if let Some(body) = gate.files.lock().ok().and_then(|f| f.get(clean).cloned()) {
            return Reply { status: "200 OK", kind, body };
        }
    }
    let Some(path) = resolve(&gate.paths.aircraft_dir, clean) else { return not_found() };
    let Ok(bytes) = std::fs::read(&path) else {
        super::log(&format!("404 {clean}"));
        return not_found();
    };
    if kind == "font/ttf" || kind == "font/otf" {
        let body = crate::font_metrics::coherent_line_metrics(&bytes).unwrap_or(bytes);
        return reply("200 OK", kind, body);
    }
    if !patched {
        return reply("200 OK", kind, bytes);
    }
    let body = match String::from_utf8(bytes) {
        Ok(text) => {
            let text = crate::source_patch::apply(&gate.patches, clean, text, |ok, msg| {
                if !ok {
                    super::log(&format!("patch ERROR: {msg}"));
                }
            });
            if kind == "text/html" { inject_first(&text, RUNTIME_TAGS).into_bytes() } else { text.into_bytes() }
        }
        Err(e) => e.into_bytes(),
    };
    let body = Arc::new(body);
    if let Ok(mut f) = gate.files.lock() {
        f.insert(clean.to_string(), body.clone());
    }
    Reply { status: "200 OK", kind, body }
}

const RUNTIME_TAGS: &str = r#"<script src="/__b/shim.js"></script><script src="/__b/runtime/xplane-js-data.js"></script>"#;

/// `script` as the first thing in `<head>`, as app/src/scheme.rs places it.
fn inject_first(html: &str, script: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let at = ["<head", "<html"].iter().find_map(|t| {
        let start = lower.find(t)?;
        Some(start + lower[start..].find('>')? + 1)
    });
    match at {
        Some(at) => format!("{}{script}{}", &html[..at], &html[at..]),
        None => format!("{script}{html}"),
    }
}

/// `/VFS/` is the aircraft package root, anything else its `html_ui`.
fn resolve(aircraft: &Path, clean: &str) -> Option<PathBuf> {
    let (mut out, rest) = match clean.strip_prefix("/VFS/") {
        Some(rest) => (aircraft.to_path_buf(), rest),
        None => (aircraft.join("html_ui"), clean),
    };
    let base = out.clone();
    let mut any = false;
    for seg in rest.split(['/', '\\']) {
        match seg {
            "" | "." => {}
            ".." => return None,
            s if s.contains(':') => return None,
            s => {
                out.push(s);
                any = true;
            }
        }
    }
    if !any {
        return None;
    }
    if !cfg!(windows) && !out.exists() {
        return Some(on_disk(&base, &out).unwrap_or(out));
    }
    Some(out)
}

/// The file as the disk spells it. FlyByWire's panel asks for `FCU/FCU.html` and the file is
/// `fcu.html`, which only a disk that ignores case forgives.
fn on_disk(base: &Path, path: &Path) -> Option<PathBuf> {
    let mut found = base.to_path_buf();
    for part in path.strip_prefix(base).ok()?.components() {
        let want = part.as_os_str().to_string_lossy();
        let name = std::fs::read_dir(&found).ok()?.flatten().map(|e| e.file_name()).find(|n| n.to_string_lossy().eq_ignore_ascii_case(&want))?;
        found.push(name);
    }
    Some(found)
}

fn mime_of(path: &str) -> &'static str {
    let ext = path.rsplit_once('.').map_or("", |(_, e)| e).to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html",
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "wasm" => "application/wasm",
        "xml" => "application/xml",
        _ => "application/octet-stream",
    }
}

fn arg(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == key).map(|(_, v)| decode(v)))
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn view_arg(query: &str) -> u32 {
    arg(query, "v").and_then(|v| v.parse().ok()).unwrap_or(u32::MAX)
}

fn number(v: f64) -> Value {
    if v.is_finite() { json!(v) } else { Value::Null }
}

const SLOT_WAIT: Duration = Duration::from_millis(200);
const SLOT_WAITS_PER_SECOND: u32 = 4;

/// Whether this new variable may hold its page up for a frame. A page loading registers
/// thousands: those read 0 until the next frame reaches them, as they always did.
fn slot_may_wait() -> bool {
    static BUDGET: Mutex<(Option<Instant>, u32)> = Mutex::new((None, 0));
    let Ok(mut budget) = BUDGET.lock() else { return false };
    let now = Instant::now();
    if budget.0.is_none_or(|from| now - from >= Duration::from_secs(1)) {
        *budget = (Some(now), 0);
    }
    budget.1 += 1;
    budget.1 <= SLOT_WAITS_PER_SECOND
}

/// Waits until the plugin has given a new variable its value, so a page that reads it once
/// gets the value and not 0 (the RMP's swap threw the active frequency away). The plugin
/// resolves slots before the systems tick and publishes them after it.
fn wait_for_slot(gate: &Gate, slot: u32) -> bool {
    let header = |f: &dyn Fn(&crate::xphfbw_bridge::SlotHeader) -> u64| gate.session.lock().map_or(0, |session| f(session.slots.header()));
    let resolved = || header(&|h| h.resolved.load(Ordering::Relaxed) as u64) > slot as u64;
    let frame = || header(&|h| h.frame.load(Ordering::Relaxed));
    if resolved() {
        return true;
    }
    if !slot_may_wait() {
        return false;
    }
    let started = Instant::now();
    while !resolved() {
        if started.elapsed() > SLOT_WAIT {
            return false;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let was = frame();
    loop {
        let now = frame();
        if now != was && now % 2 == 0 {
            return true;
        }
        if started.elapsed() > SLOT_WAIT {
            return false;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// The airport map's AMDB request, answered from the plugin's data; it may wait on the AMDB Bridge, on this thread.
fn amdb_answer(method: &str, url: &str) -> Option<(u16, String)> {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        match crate::oans::plugin::amdb_fetch(method, url, url)? {
            Some(done) => return Some(done),
            None if std::time::Instant::now() < until => std::thread::sleep(std::time::Duration::from_millis(50)),
            None => return Some((504, String::new())),
        }
    }
}

fn http_status(code: u16) -> &'static str {
    match code {
        200 => "200 OK",
        400 => "400 Bad Request",
        404 => "404 Not Found",
        503 => "503 Service Unavailable",
        504 => "504 Gateway Timeout",
        _ => "500 Internal Server Error",
    }
}

fn bridge_route(gate: &Gate, rest: &str, req: &Request) -> Reply {
    match rest {
        "hello" => hello(gate),
        "slot" => {
            let (name, unit) = (arg(&req.query, "n").unwrap_or_default(), arg(&req.query, "u").unwrap_or_default());
            let found = {
                let Ok(session) = gate.session.lock() else { return not_found() };
                session.slots.register(&name, &unit)
            };
            let ready = found.is_some_and(|s| wait_for_slot(gate, s));
            let Ok(session) = gate.session.lock() else { return not_found() };
            let value = found.map_or(0., |s| session.slots.read(s));
            reply("200 OK", "application/json", json!({ "s": found.map_or(-1, |s| s as i64), "v": number(value), "ready": ready }).to_string())
        }
        "simbridge" => {
            let path = arg(&req.query, "p").unwrap_or_default();
            let (status, text) = crate::mapdata::plugin::simbridge_answer(&req.method, &path, &String::from_utf8_lossy(&req.body));
            match status {
                200 => reply("200 OK", "application/json", text),
                201 => reply("201 Created", "application/json", text),
                400 => reply("400 Bad Request", "application/json", text),
                503 => reply("503 Service Unavailable", "text/plain", text),
                _ => not_found(),
            }
        }
        "amdb" => {
            let url = arg(&req.query, "u").unwrap_or_default();
            match amdb_answer(&req.method, &url) {
                Some((status, text)) => reply(http_status(status), "application/json", text),
                None => not_found(),
            }
        }
        "game" => game_string(gate, view_arg(&req.query), &arg(&req.query, "n").unwrap_or_default()),
        "stored" => {
            let args: Vec<String> = serde_json::from_slice(&req.body).unwrap_or_default();
            let get = |i: usize| args.get(i).map_or("", String::as_str);
            reply("200 OK", "text/plain; charset=utf-8", crate::stored_data::run(&gate.paths.xplane_root, get(0), get(1), get(2)))
        }
        "file" => {
            let p = arg(&req.query, "p").unwrap_or_default();
            let clean = format!("/{}", p.trim_start_matches(['/', '\\']).replace('\\', "/"));
            let found = file_text(gate, &clean);
            match found {
                Some(body) => Reply { status: "200 OK", kind: "text/plain; charset=utf-8", body },
                None => not_found(),
            }
        }
        "mag" => {
            let n = |k: &str| arg(&req.query, k).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.);
            reply("200 OK", "text/plain", crate::wmm::declination(n("lat"), n("lon")).to_string())
        }
        "state" => state(req),
        r if r.starts_with("boot/") => reply("200 OK", "text/html", format!("<!doctype html><script>setTimeout(()=>location.replace('/__b/host/{}'),1000)</script>", &r["boot/".len()..])),
        r if r.starts_with("host/") => match gate.layout.page(&r["host/".len()..]) {
            Some(page) => reply("200 OK", "text/html; charset=utf-8", page),
            None => not_found(),
        },
        r if r.starts_with("runtime/") => {
            let name = &r["runtime/".len()..];
            if name.contains(['/', '\\', ':']) || name.contains("..") {
                return not_found();
            }
            match std::fs::read(gate.paths.ui_dir.parent().map(|d| d.join("js").join(name)).unwrap_or_default()) {
                Ok(bytes) => reply("200 OK", "text/javascript", bytes),
                Err(_) => not_found(),
            }
        }
        _ => not_found(),
    }
}

/// `readFile`: the same mapping and patches as a request for the file, without the page tags.
fn file_text(gate: &Gate, clean: &str) -> Option<Arc<Vec<u8>>> {
    let cache_key = format!("read:{clean}");
    if let Some(body) = gate.files.lock().ok().and_then(|f| f.get(&cache_key).cloned()) {
        return Some(body);
    }
    let text = std::fs::read_to_string(resolve(&gate.paths.aircraft_dir, clean)?).ok()?;
    let text = crate::source_patch::apply(&gate.patches, clean, text, |ok, msg| {
        if !ok {
            super::log(&format!("patch ERROR: {msg}"));
        }
    });
    let body = Arc::new(text.into_bytes());
    if let Ok(mut f) = gate.files.lock() {
        f.insert(cache_key, body.clone());
    }
    Some(body)
}

fn state(req: &Request) -> Reply {
    let screen = arg(&req.query, "s").unwrap_or_default();
    let dims: Vec<Value> = super::dimming(&screen).iter().map(|v| json!((*v as f64 * 1000.).round() / 1000.)).collect();
    let under = super::underlay(&screen).map(|(w, h, g, _)| json!({ "w": w, "h": h, "g": g }));
    reply("200 OK", "application/json", json!({ "d": dims, "u": under }).to_string())
}

fn seqlock_copy(session: &Session) -> (u64, u32, Vec<f64>) {
    let header = session.slots.header();
    loop {
        let before = header.frame.load(Ordering::Acquire);
        if before & 1 == 1 {
            std::thread::yield_now();
            continue;
        }
        let resolved = header.resolved.load(Ordering::Acquire);
        let values = session.slots.values()[..resolved as usize].to_vec();
        if header.frame.load(Ordering::Acquire) == before {
            return (before, resolved, values);
        }
    }
}

fn hello(gate: &Gate) -> Reply {
    let Ok(session) = gate.session.lock() else { return not_found() };
    let count = session.slots.header().count.load(Ordering::Relaxed) as usize;
    let slots: Vec<Value> = (0..count).map(|i| { let (n, u) = session.slots.entry(i); json!([n, u]) }).collect();
    let (frame, resolved, values) = seqlock_copy(&session);
    drop(session);
    let values: Vec<Value> = values.into_iter().map(number).collect();
    let mut games = serde_json::Map::new();
    if let Some(range) = crate::navdata_date_range(&gate.paths.xplane_root) {
        games.insert("fbw/FLIGHT_NAVDATA_DATE_RANGE".to_string(), Value::String(range));
    }
    let body = json!({
        "slots": slots,
        "values": values,
        "frame": frame,
        "resolved": resolved,
        "games": games,
        "aircraftDir": gate.paths.aircraft_dir.to_string_lossy(),
        "free": super::own_route(),
    });
    reply("200 OK", "application/json", body.to_string())
}

/// This view's downlink into its stash, game strings noted, for whoever is
/// waiting: the page's socket and a page blocked in `gameString`.
fn drain_view(gate: &Gate, view: u32) {
    let Some(page) = gate.pages.get(view as usize) else { return };
    let Ok(session) = gate.session.lock() else { return };
    let Some(ring) = session.downlinks.get(view as usize) else { return };
    let records = ring.drain();
    drop(session);
    if records.is_empty() {
        return;
    }
    let Ok(mut stash) = page.stash.lock() else { return };
    for record in records.iter().filter_map(|b| Downlink::decode(b)) {
        if let Downlink::GameString { name, value } = &record {
            if let Ok(mut games) = page.games.lock() {
                games.insert(name.clone(), value.clone());
            }
        }
        stash.push(record);
    }
}

fn game_string(gate: &Gate, view: u32, name: &str) -> Reply {
    let Some(page) = gate.pages.get(view as usize) else { return not_found() };
    if let Ok(session) = gate.session.lock() {
        session.uplink.push(&Uplink::GameString { view, name: name.to_string() }.encode());
    }
    let until = Instant::now() + GAME_STRING_WAIT;
    while Instant::now() < until && alive() {
        drain_view(gate, view);
        if let Some(value) = page.games.lock().ok().and_then(|g| g.get(name).cloned()) {
            return reply("200 OK", "text/plain; charset=utf-8", value);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    not_found()
}

fn websocket(stream: &mut TcpStream, key: &str, query: &str) -> std::io::Result<()> {
    let Some(gate) = gate() else { return Ok(()) };
    let view = view_arg(query);
    let lite = arg(query, "lite").is_some();
    if view as usize >= MAX_VIEWS {
        return Ok(());
    }
    let accept = websocket_accept(key);
    write!(stream, "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n")?;
    let open = Arc::new(AtomicBool::new(true));
    let writer = Arc::new(Mutex::new(stream.try_clone()?));
    {
        let (gate, open, writer) = (gate.clone(), open.clone(), writer.clone());
        spawn("fbw-browser-ws", move || {
            let _ = pump(&gate, view, lite, &open, &writer);
            open.store(false, Ordering::SeqCst);
        });
    }
    gate.pages[view as usize].connected.store(true, Ordering::Relaxed);
    let result = incoming(&gate, view, stream, &open, &writer);
    open.store(false, Ordering::SeqCst);
    gate.pages[view as usize].connected.store(false, Ordering::Relaxed);
    result
}

fn current(gate: &Gate) -> bool {
    alive() && GENERATION.load(Ordering::Relaxed) == gate.generation
}

/// Everything going to the page: each published frame as the slots that
/// changed for this connection, then downlink records and keystrokes.
fn pump(gate: &Gate, view: u32, lite: bool, open: &AtomicBool, writer: &Mutex<TcpStream>) -> std::io::Result<()> {
    let page = &gate.pages[view as usize];
    let (mut last_frame, mut last): (u64, Vec<f64>) = (0, Vec::new());
    let mut bands: Vec<Option<super::dead_band::Band>> = Vec::new();
    let bands_on = std::env::var_os("FBW_NO_DEAD_BAND").is_none();
    while open.load(Ordering::SeqCst) && current(gate) {
        if !lite {
            let values = gate.session.lock().ok().and_then(|session| {
                let frame = session.slots.header().frame.load(Ordering::Acquire);
                (frame != last_frame).then(|| seqlock_copy(&session))
            });
            if let Some((frame, resolved, values)) = values {
                last_frame = frame;
                if bands_on && bands.len() < values.len() {
                    if let Ok(session) = gate.session.lock() {
                        for slot in bands.len()..values.len() {
                            bands.push(super::dead_band::of(&session.slots.entry(slot).0));
                        }
                    }
                }
                let mut message = Vec::with_capacity(64);
                message.extend_from_slice(&(frame as f64).to_le_bytes());
                message.extend_from_slice(&(resolved as f64).to_le_bytes());
                last.resize(values.len(), f64::from_bits(u64::MAX));
                for (i, (now, was)) in values.iter().zip(last.iter_mut()).enumerate() {
                    if now.to_bits() != was.to_bits() && !bands.get(i).copied().flatten().is_some_and(|band| band.same_on_screen(*was, *now)) {
                        *was = *now;
                        message.extend_from_slice(&(i as f64).to_le_bytes());
                        message.extend_from_slice(&now.to_le_bytes());
                    }
                }
                page.sent_bytes.fetch_add(message.len() as u64, Ordering::Relaxed);
                send_frame(writer, 2, &message)?;
            }
            drain_view(gate, view);
            let records = page.stash.lock().map(|mut s| std::mem::take(&mut *s)).unwrap_or_default();
            if !records.is_empty() {
                let list: Vec<Value> = records.into_iter().map(record_json).collect();
                send_frame(writer, 1, Value::Array(list).to_string().as_bytes())?;
            }
        }
        if let Some((x, y)) = super::take_hover(view) {
            send_frame(writer, 1, json!([["c", x, y]]).to_string().as_bytes())?;
        }
        let keys = super::take_keys(view);
        if !keys.is_empty() {
            let list: Vec<Value> = keys.into_iter().map(|(ch, vk)| json!(["k", ch, vk])).collect();
            send_frame(writer, 1, Value::Array(list).to_string().as_bytes())?;
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    Ok(())
}

fn record_json(record: Downlink) -> Value {
    match record {
        Downlink::Reply { id, ok, text } => json!(["r", id & 0xFFFF_FFFF_FFFF, ok, text]),
        Downlink::HEvent { name } => json!(["h", name, ""]),
        Downlink::ProviderEvent { name, json } => json!(["provider", name, json]),
        Downlink::GameString { name, value } => json!(["game", name, value]),
        Downlink::StringValue { name, value } => json!(["string", name, value]),
    }
}

fn send_frame(writer: &Mutex<TcpStream>, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
    let mut head = Vec::with_capacity(10);
    head.push(0x80 | opcode);
    match payload.len() {
        n if n < 126 => head.push(n as u8),
        n if n < 65536 => {
            head.push(126);
            head.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            head.push(127);
            head.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    let mut stream = writer.lock().map_err(|_| ErrorKind::Other)?;
    stream.write_all(&head)?;
    stream.write_all(payload)
}

fn read_full(stream: &mut TcpStream, buf: &mut [u8], open: &AtomicBool) -> std::io::Result<bool> {
    let mut at = 0;
    while at < buf.len() {
        match stream.read(&mut buf[at..]) {
            Ok(0) => return Ok(false),
            Ok(n) => at += n,
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted) => {
                if !alive() || !open.load(Ordering::SeqCst) {
                    return Ok(false);
                }
            }
            Err(e) => return Err(e),
        }
    }
    Ok(true)
}

fn incoming(gate: &Gate, view: u32, stream: &mut TcpStream, open: &AtomicBool, writer: &Mutex<TcpStream>) -> std::io::Result<()> {
    let mut message = Vec::new();
    loop {
        let mut head = [0u8; 2];
        if !read_full(stream, &mut head, open)? {
            return Ok(());
        }
        let (fin, opcode, masked) = (head[0] & 0x80 != 0, head[0] & 0x0f, head[1] & 0x80 != 0);
        let mut len = (head[1] & 0x7f) as u64;
        if len == 126 {
            let mut b = [0u8; 2];
            if !read_full(stream, &mut b, open)? {
                return Ok(());
            }
            len = u16::from_be_bytes(b) as u64;
        } else if len == 127 {
            let mut b = [0u8; 8];
            if !read_full(stream, &mut b, open)? {
                return Ok(());
            }
            len = u64::from_be_bytes(b);
        }
        if len > 256 << 20 {
            return Ok(());
        }
        let mut mask = [0u8; 4];
        if masked && !read_full(stream, &mut mask, open)? {
            return Ok(());
        }
        let mut payload = vec![0u8; len as usize];
        if !read_full(stream, &mut payload, open)? {
            return Ok(());
        }
        if masked {
            for (i, b) in payload.iter_mut().enumerate() {
                *b ^= mask[i % 4];
            }
        }
        match opcode {
            8 => return Ok(()),
            9 => send_frame(writer, 10, &payload)?,
            10 => {}
            _ => {
                message.extend_from_slice(&payload);
                if fin {
                    apply_ops(gate, view, &message);
                    message.clear();
                }
            }
        }
    }
}

fn apply_ops(gate: &Gate, view: u32, message: &[u8]) {
    let Ok(Value::Array(ops)) = serde_json::from_slice::<Value>(message) else { return };
    let mut records = Vec::with_capacity(ops.len());
    for op in &ops {
        let Some(a) = op.as_array() else { continue };
        let s = |i: usize| a.get(i).and_then(Value::as_str).unwrap_or("").to_string();
        let n = |i: usize| a.get(i).and_then(Value::as_f64).unwrap_or(0.);
        match a.first().and_then(Value::as_str).unwrap_or("") {
            "w" => records.push(Uplink::Write { slot: n(1) as u32, value: n(2) }),
            "e" => {
                let values = a.get(2).and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
                records.push(Uplink::Event { name: s(1), values });
            }
            "s" => records.push(Uplink::SetString { name: s(1), value: s(2) }),
            "c" => records.push(Uplink::Call { view, id: ((view as u64) << 48) | n(1) as u64, name: s(2), args_json: s(3) }),
            "l" => records.push(Uplink::Log { view, level: n(1).clamp(0., 2.) as u8, text: s(2) }),
            "L" => {
                let ok = n(1) != 0.;
                if let Ok(session) = gate.session.lock() {
                    if let Some(cell) = session.slots.header().view_loaded.get(view as usize) {
                        cell.store(if ok { 1 } else { 2 }, Ordering::Relaxed);
                    }
                }
                records.push(Uplink::Loaded { view, ok, text: s(2) });
            }
            "f" => super::set_text_focus(view, n(1) != 0.),
            "p" => {
                if let Some(page) = gate.pages.get(view as usize) {
                    page.fps.store(n(1) as u64, Ordering::Relaxed);
                }
            }
            _ => {}
        }
    }
    for record in records {
        let bytes = record.encode();
        for _ in 0..500 {
            let pushed = gate.session.lock().map(|session| session.uplink.push(&bytes)).unwrap_or(true);
            if pushed || !alive() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

fn websocket_accept(key: &str) -> String {
    base64(&sha1(format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11").as_bytes()))
}

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in message.chunks(64) {
        let mut w = [0u32; 80];
        for (i, word) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, word) in w.iter().enumerate() {
            let (f, k) = match i / 20 {
                0 => ((b & c) | (!b & d), 0x5A827999),
                1 => (b ^ c ^ d, 0x6ED9EBA1),
                2 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6u32),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_handshake_answer_matches_the_websocket_rfc_example() {
        assert_eq!(websocket_accept("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn files_stay_inside_the_aircraft() {
        let root = Path::new("C:/ac");
        assert_eq!(resolve(root, "/Pages/a.js"), Some(root.join("html_ui").join("Pages").join("a.js")));
        assert_eq!(resolve(root, "/VFS/panel/panel.cfg"), Some(root.join("panel").join("panel.cfg")));
        assert_eq!(resolve(root, "/Pages/../../x"), None);
        assert_eq!(resolve(root, "/C:/windows"), None);
    }

    #[test]
    fn page_scripts_go_first_in_the_head() {
        assert_eq!(inject_first("<!doctype html><html><head><meta></head></html>", "<s>"), "<!doctype html><html><head><s><meta></head></html>");
        assert_eq!(inject_first("<div></div>", "<s>"), "<s><div></div>");
    }
}

#[cfg(test)]
mod case_tests {
    use super::*;

    #[test]
    fn a_page_asked_for_in_the_wrong_case_is_found_as_the_disk_spells_it() {
        let root = std::env::temp_dir().join(format!("fbw-case-{}", std::process::id()));
        let dir = root.join("Pages").join("FCU");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("fcu.html"), "x").unwrap();
        let found = on_disk(&root, &root.join("pages").join("fcu").join("FCU.html")).unwrap();
        assert_eq!(found, dir.join("fcu.html"));
        assert_eq!(found.file_name().unwrap().to_string_lossy(), "fcu.html");
        assert_eq!(on_disk(&root, &root.join("pages").join("nothing.html")), None);
        std::fs::remove_dir_all(&root).unwrap();
    }
}

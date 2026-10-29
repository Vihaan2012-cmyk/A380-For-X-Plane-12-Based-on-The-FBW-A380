use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use serde_json::Value;

const READ_SETS: &str = include_str!("read_sets.txt");

pub trait InputSink {
    fn set_var(&mut self, name: &str, unit: &str, value: f64);
    fn set_string(&mut self, name: &str, value: &str);
    fn set_bus(&mut self, topic: &str, json: &str);
    fn set_bus_string(&mut self, topic: &str, text: &str);
    fn set_generic(&mut self, key: &str, json: &str);
    fn set_fetched(&mut self, url: &str, body: &[u8]);
    fn set_env_number(&mut self, path: &str, value: f64);
    fn set_now(&mut self, ms: f64);
    fn set_dpr(&mut self, dpr: f64);
}

pub trait VarSource {
    fn read_var(&mut self, name: &str, unit: &str) -> Option<f64>;
    fn read_string(&mut self, name: &str) -> Option<String>;
    fn read_env(&mut self, _path: &str) -> Option<f64> {
        None
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct ReadSet {
    pub vars: Vec<(String, String)>,
    pub strings: Vec<String>,
    pub env: Vec<String>,
}

fn read_sets() -> &'static HashMap<String, ReadSet> {
    static SETS: OnceLock<HashMap<String, ReadSet>> = OnceLock::new();
    SETS.get_or_init(|| {
        let mut sets: HashMap<String, ReadSet> = HashMap::new();
        for line in READ_SETS.lines() {
            let mut f = line.split('\t');
            let (Some(display), Some(kind), Some(name)) = (f.next(), f.next(), f.next()) else { continue };
            let set = sets.entry(display.to_string()).or_default();
            match kind {
                "var" => set.vars.push((name.to_string(), f.next().unwrap_or("").to_string())),
                "str" => set.strings.push(name.to_string()),
                "env" => set.env.push(name.to_string()),
                _ => {}
            }
        }
        sets
    })
}

pub fn read_set(display: &str) -> Option<&'static ReadSet> {
    read_sets().get(display)
}

pub fn read_set_displays() -> Vec<&'static str> {
    let mut names: Vec<&str> = read_sets().keys().map(String::as_str).collect();
    names.sort_unstable();
    names
}

/// An EFB `CONFIG_*` setting as text, read again from the settings files every few seconds.
pub fn config_string(name: &str) -> Option<String> {
    if !name.starts_with("CONFIG_") {
        return None;
    }
    static CACHE: OnceLock<Mutex<(Option<std::time::Instant>, HashMap<String, String>)>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(|| Mutex::new((None, HashMap::new()))).lock().unwrap_or_else(|e| e.into_inner());
    if cache.0.map_or(true, |at| at.elapsed() > std::time::Duration::from_secs(5)) {
        cache.0 = Some(std::time::Instant::now());
        let root = crate::xp::system_path()?;
        cache.1 = crate::efb_native::host::load_settings(&root)
            .into_iter()
            .filter(|(k, _)| k.starts_with("CONFIG_"))
            .map(|(k, v)| {
                let text = v.as_str().map_or_else(|| v.to_string(), str::to_string);
                (k, text)
            })
            .collect();
    }
    cache.1.get(name).cloned()
}

pub const MFD_DOM_TOPIC: &str = "a380.mfd.dom";
pub const OIT_DOM_PREFIX: &str = "a380.oit.dom.";

fn is_dom_topic(topic: &str) -> bool {
    topic == MFD_DOM_TOPIC || topic.starts_with(OIT_DOM_PREFIX)
}

/// The one DOM snapshot topic a screen reads, by the read-set name its target has.
fn own_dom_topic(display: &str) -> String {
    match display.strip_prefix("oit-") {
        Some(index) => format!("{OIT_DOM_PREFIX}{index}"),
        None if display == "mfd" => MFD_DOM_TOPIC.to_string(),
        None => String::new(),
    }
}

const FETCHED_MAX_BODY: usize = 16 << 20;
const FETCHED_MAX_TOTAL: usize = 48 << 20;
const FETCHED_MAX_COUNT: usize = 128;

struct Entry {
    raw: String,
    json: String,
    text: bool,
    seq: u64,
}

struct Blob {
    body: Vec<u8>,
    seq: u64,
}

#[derive(Default)]
struct LogInner {
    seq: u64,
    bus: HashMap<String, Entry>,
    generic: HashMap<String, Entry>,
    fetched: HashMap<String, Blob>,
}

#[derive(Default)]
pub struct InputLog {
    inner: Mutex<LogInner>,
    seq: AtomicU64,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Changes {
    pub vars: usize,
    pub strings: usize,
    pub bus: usize,
    pub generic: usize,
    pub fetched: usize,
}

impl Changes {
    pub fn total(&self) -> usize {
        self.vars + self.strings + self.bus + self.generic + self.fetched
    }

    pub fn changed(&self) -> bool {
        self.total() > 0
    }
}

fn lock(m: &Mutex<LogInner>) -> MutexGuard<'_, LogInner> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn text_json(text: &str) -> String {
    if serde_json::from_str::<Value>(text).is_ok() {
        text.to_string()
    } else {
        Value::String(text.to_string()).to_string()
    }
}

fn value_json(v: &Value) -> String {
    match v {
        Value::String(s) => text_json(s),
        other => other.to_string(),
    }
}

fn topic_of(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

impl InputLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn seq(&self) -> u64 {
        self.seq.load(Ordering::Acquire)
    }

    pub fn set_bus(&self, topic: &str, json: String) {
        self.put_bus(topic, json, false);
    }

    pub fn set_bus_string(&self, topic: &str, text: String) {
        self.put_bus(topic, text, true);
    }

    fn put_bus(&self, topic: &str, json: String, text: bool) {
        let mut g = lock(&self.inner);
        if g.bus.get(topic).is_some_and(|e| e.text == text && e.json == json) {
            return;
        }
        g.seq += 1;
        let seq = g.seq;
        g.bus.insert(topic.to_string(), Entry { raw: String::new(), json, text, seq });
        self.seq.store(seq, Ordering::Release);
    }

    fn put_generic(&self, key: &str, raw: &str, json: impl FnOnce() -> String) {
        let mut g = lock(&self.inner);
        if g.generic.get(key).is_some_and(|e| e.raw == raw) {
            return;
        }
        let json = json();
        if g.generic.get(key).is_some_and(|e| e.json == json) {
            return;
        }
        g.seq += 1;
        let seq = g.seq;
        g.generic.insert(key.to_string(), Entry { raw: raw.to_string(), json, text: false, seq });
        self.seq.store(seq, Ordering::Release);
    }

    pub fn set_generic(&self, key: &str, data: &str) {
        self.put_generic(key, data, || text_json(data));
    }

    pub fn set_fetched(&self, url: &str, body: &[u8]) {
        if body.len() > FETCHED_MAX_BODY {
            return;
        }
        let mut g = lock(&self.inner);
        if g.fetched.get(url).is_some_and(|b| b.body == body) {
            return;
        }
        g.seq += 1;
        let seq = g.seq;
        g.fetched.insert(url.to_string(), Blob { body: body.to_vec(), seq });
        let mut total: usize = g.fetched.values().map(|b| b.body.len()).sum();
        while total > FETCHED_MAX_TOTAL || g.fetched.len() > FETCHED_MAX_COUNT {
            let Some(oldest) = g.fetched.iter().filter(|(_, b)| b.seq != seq).min_by_key(|(_, b)| b.seq).map(|(k, _)| k.clone()) else { break };
            if let Some(b) = g.fetched.remove(&oldest) {
                total -= b.body.len();
            }
        }
        self.seq.store(seq, Ordering::Release);
    }

    /// The same bookkeeping tools/golden-tour/rec.js does for a
    /// `JS_LISTENER_GENERICDATA` event: `xp-ui.bus` batches split per topic,
    /// `EB_EVENTS` packs also split per topic, everything else stored by key.
    pub fn generic_data(&self, key: &str, data: &str) {
        if key == "xp-ui.bus" {
            let Ok(batch) = serde_json::from_str::<Value>(data) else { return };
            for e in batch.get("events").and_then(Value::as_array).into_iter().flatten() {
                if let Some(topic) = topic_of(e.get("topic")) {
                    match e.get("data") {
                        Some(Value::String(text)) => self.set_bus_string(&topic, text.clone()),
                        Some(other) => self.set_bus(&topic, other.to_string()),
                        None => self.set_bus(&topic, "null".to_string()),
                    }
                }
            }
            return;
        }
        self.set_generic(key, data);
        if key == "EB_EVENTS" {
            let Ok(pack) = serde_json::from_str::<Value>(data) else { return };
            for e in pack.get("data").and_then(Value::as_array).into_iter().flatten() {
                if let Some(topic) = topic_of(e.get("topic")) {
                    let value = e.get("data").cloned().unwrap_or(Value::Null);
                    let json = value_json(&value);
                    self.put_generic(&topic, &json, || json.clone());
                }
            }
        }
    }

    fn push_since(&self, seen: u64, sink: &mut (impl InputSink + ?Sized), changes: &mut Changes, skip_dom: bool, own_dom: &str) -> u64 {
        let g = lock(&self.inner);
        for (topic, e) in &g.bus {
            if e.seq > seen && !(skip_dom && is_dom_topic(topic) && topic != own_dom) {
                if e.text {
                    sink.set_bus_string(topic, &e.json);
                } else {
                    sink.set_bus(topic, &e.json);
                }
                changes.bus += 1;
            }
        }
        for (key, e) in &g.generic {
            if e.seq > seen {
                sink.set_generic(key, &e.json);
                changes.generic += 1;
            }
        }
        for (url, b) in &g.fetched {
            if b.seq > seen {
                sink.set_fetched(url, &b.body);
                changes.fetched += 1;
            }
        }
        g.seq
    }
}

static USERS: AtomicUsize = AtomicUsize::new(0);
static ARMED: AtomicBool = AtomicBool::new(false);

fn global_log() -> &'static InputLog {
    static LOG: OnceLock<InputLog> = OnceLock::new();
    LOG.get_or_init(InputLog::new)
}

/// One atomic load: whether any native screen exists and wants the tap.
pub fn active() -> bool {
    ARMED.load(Ordering::Relaxed) || USERS.load(Ordering::Relaxed) > 0
}

pub fn log_seq() -> u64 {
    global_log().seq()
}

pub fn tap_generic_data(key: &str, data: &str) {
    if active() {
        global_log().generic_data(key, data);
    }
}

pub fn tap_fetched(url: &str, body: &[u8]) {
    if active() {
        global_log().set_fetched(url, body);
    }
}

struct VarState {
    name: String,
    unit: String,
    last: Option<u64>,
}

struct StrState {
    name: String,
    last: Option<String>,
}

struct EnvState {
    path: String,
    last: Option<u64>,
}

pub struct LiveInputs {
    vars: Vec<VarState>,
    strings: Vec<StrState>,
    env: Vec<EnvState>,
    started_ms: Option<f64>,
    epoch_ms: Option<f64>,
    frames: u64,
    seen: u64,
    counted: bool,
    skip_dom: bool,
    own_dom: String,
}

impl LiveInputs {
    pub fn new(display: &str) -> Self {
        let mut me = Self::with_read_set(read_set(display).cloned().unwrap_or_default());
        USERS.fetch_add(1, Ordering::Relaxed);
        me.counted = true;
        me
    }

    pub fn with_read_set(set: ReadSet) -> Self {
        Self {
            vars: set.vars.into_iter().map(|(name, unit)| VarState { name, unit, last: None }).collect(),
            strings: set.strings.into_iter().map(|name| StrState { name, last: None }).collect(),
            env: set.env.into_iter().map(|path| EnvState { path, last: None }).collect(),
            started_ms: None,
            epoch_ms: None,
            frames: 0,
            seen: 0,
            counted: false,
            skip_dom: false,
            own_dom: String::new(),
        }
    }

    /// Screens other than the MFD never read the MFD's DOM snapshot, which is
    /// the one large topic.
    pub fn skip_mfd_dom(&mut self, skip: bool) {
        self.skip_dom = skip;
    }

    pub fn own_dom_topic(&mut self, topic: String) {
        self.own_dom = topic;
    }

    pub fn var_count(&self) -> usize {
        self.vars.len()
    }

    pub fn update(&mut self, src: &mut (impl VarSource + ?Sized), sink: &mut (impl InputSink + ?Sized), now_ms: f64, dpr: f64) -> Changes {
        self.update_from(global_log(), src, sink, now_ms, dpr)
    }

    pub fn update_from(
        &mut self,
        log: &InputLog,
        src: &mut (impl VarSource + ?Sized),
        sink: &mut (impl InputSink + ?Sized),
        now_ms: f64,
        dpr: f64,
    ) -> Changes {
        let mut changes = Changes::default();
        for v in &mut self.vars {
            let Some(value) = src.read_var(&v.name, &v.unit) else { continue };
            let bits = value.to_bits();
            if v.last != Some(bits) {
                v.last = Some(bits);
                sink.set_var(&v.name, &v.unit, value);
                changes.vars += 1;
            }
        }
        for s in &mut self.strings {
            let Some(value) = src.read_string(&s.name) else { continue };
            if s.last.as_deref() != Some(value.as_str()) {
                sink.set_string(&s.name, &value);
                s.last = Some(value);
                changes.strings += 1;
            }
        }
        let started = *self.started_ms.get_or_insert(now_ms);
        let epoch = *self.epoch_ms.get_or_insert_with(|| {
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_millis() as f64)
        });
        self.frames += 1;
        for e in &mut self.env {
            let value = src.read_env(&e.path).or_else(|| match e.path.as_str() {
                "connectedAt.perf" => Some(0.0),
                "connectedAt.date" => Some(epoch),
                "frozenAt.date" => Some(epoch + (now_ms - started)),
                "frozenAt.perf" => Some(now_ms - started),
                "frozenAt.frames" => Some(self.frames as f64),
                _ => None,
            });
            let Some(value) = value else { continue };
            let bits = value.to_bits();
            if e.last != Some(bits) {
                e.last = Some(bits);
                sink.set_env_number(&e.path, value);
            }
        }
        if log.seq() > self.seen {
            self.seen = log.push_since(self.seen, sink, &mut changes, self.skip_dom, &self.own_dom);
        }
        sink.set_now(now_ms);
        sink.set_dpr(dpr);
        changes
    }
}

impl Drop for LiveInputs {
    fn drop(&mut self) {
        if self.counted {
            USERS.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

const UNKNOWN: u64 = u64::MAX;

const READY_PUBLISHES: u32 = 2;
const READY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// The longest a screen goes without a redraw, for its time-driven parts.
fn redraw_period_ms(display: &str) -> f64 {
    match display {
        "isislegacy" | "pfd" | "nd" | "ewd" | "sd" | "sdv2" | "mfd" | "mfdx" => 100.,
        "snd" => 250.,
        "clock" => 1000.,
        _ => 500.,
    }
}

struct ScreenKeys {
    id: &'static str,
    display: String,
    period_ms: f64,
    read: ReadSet,
    vars: Vec<usize>,
    strings: Vec<usize>,
}

struct Keys {
    screens: Vec<ScreenKeys>,
    vars: Vec<(String, String)>,
    strings: Vec<String>,
    var_lookup: HashMap<String, Vec<(String, usize)>>,
    str_lookup: HashMap<String, usize>,
}

impl Keys {
    fn new(screens: &[(&'static str, String)]) -> Self {
        let mut keys = Keys { screens: Vec::new(), vars: Vec::new(), strings: Vec::new(), var_lookup: HashMap::new(), str_lookup: HashMap::new() };
        for (id, display) in screens {
            let read = read_set(display).cloned().unwrap_or_default();
            let mut vars = Vec::with_capacity(read.vars.len());
            for (name, unit) in &read.vars {
                let known = keys.var_lookup.get(name).and_then(|u| u.iter().find(|(u, _)| u == unit)).map(|(_, i)| *i);
                let index = known.unwrap_or_else(|| {
                    keys.vars.push((name.clone(), unit.clone()));
                    let i = keys.vars.len() - 1;
                    keys.var_lookup.entry(name.clone()).or_default().push((unit.clone(), i));
                    i
                });
                vars.push(index);
            }
            let mut strings = Vec::with_capacity(read.strings.len());
            for name in &read.strings {
                let index = match keys.str_lookup.get(name) {
                    Some(&i) => i,
                    None => {
                        keys.strings.push(name.clone());
                        keys.str_lookup.insert(name.clone(), keys.strings.len() - 1);
                        keys.strings.len() - 1
                    }
                };
                strings.push(index);
            }
            keys.screens.push(ScreenKeys { id, display: display.clone(), period_ms: redraw_period_ms(display), read, vars, strings });
        }
        keys
    }

    fn screen(&self, id: &str) -> Option<usize> {
        self.screens.iter().position(|s| s.id.eq_ignore_ascii_case(id))
    }
}

/// What the main thread hands the render thread: the value of every
/// variable and string the selected screens read, and one revision per
/// screen that moves when any of its own values changed.
pub struct Snapshot {
    seq: u64,
    log_seq: u64,
    values: Vec<u64>,
    strings: Vec<Option<String>>,
    revs: Vec<u64>,
}

impl Snapshot {
    fn empty(keys: &Keys) -> Self {
        Self { seq: 0, log_seq: 0, values: vec![UNKNOWN; keys.vars.len()], strings: vec![None; keys.strings.len()], revs: vec![0; keys.screens.len()] }
    }
}

pub struct Hub {
    keys: Keys,
    log: &'static InputLog,
    current: Mutex<Arc<Snapshot>>,
    published: AtomicU64,
    time_bits: AtomicU64,
    ready: Vec<AtomicU32>,
}

impl Hub {
    pub fn with_log(screens: &[(&'static str, String)], log: &'static InputLog) -> Arc<Self> {
        let keys = Keys::new(screens);
        let current = Mutex::new(Arc::new(Snapshot::empty(&keys)));
        let ready = keys.screens.iter().map(|_| AtomicU32::new(0)).collect();
        Arc::new(Self { keys, log, current, published: AtomicU64::new(0), time_bits: AtomicU64::new(0), ready })
    }

    /// The time of the newest publish call, changed or not.
    fn now_ms(&self) -> f64 {
        f64::from_bits(self.time_bits.load(Ordering::Relaxed))
    }

    fn note_ready(&self, resolved: &[bool]) {
        for (count, &ok) in self.ready.iter().zip(resolved) {
            let next = if ok { (count.load(Ordering::Relaxed) + 1).min(READY_PUBLISHES) } else { 0 };
            count.store(next, Ordering::Relaxed);
        }
    }

    fn snapshot(&self) -> Arc<Snapshot> {
        self.current.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn publisher(self: &Arc<Self>) -> Publisher {
        Publisher { hub: self.clone(), last: self.snapshot() }
    }

    pub fn feed(self: &Arc<Self>) -> LiveFeed {
        LiveFeed { hub: self.clone(), snap: self.snapshot(), screens: self.keys.screens.iter().map(|_| None).collect() }
    }

    pub fn var_count(&self) -> usize {
        self.keys.vars.len()
    }
}

/// The main-thread half: reads the variable store once a frame and swaps in
/// a new snapshot only when something a screen reads moved.
pub struct Publisher {
    hub: Arc<Hub>,
    last: Arc<Snapshot>,
}

impl Publisher {
    pub fn publish(&mut self, src: &mut (impl VarSource + ?Sized), time_ms: f64) -> bool {
        let started = std::time::Instant::now();
        let published = self.publish_untimed(src, time_ms);
        super::cost::main_since(started);
        published
    }

    fn publish_untimed(&mut self, src: &mut (impl VarSource + ?Sized), time_ms: f64) -> bool {
        self.hub.time_bits.store(time_ms.to_bits(), Ordering::Relaxed);
        if self.hub.published.load(Ordering::Acquire) != self.last.seq {
            self.last = self.hub.snapshot();
        }
        let keys = &self.hub.keys;
        let last = &*self.last;
        let mut values = Vec::with_capacity(keys.vars.len());
        for (name, unit) in &keys.vars {
            values.push(src.read_var(name, unit).map_or(UNKNOWN, f64::to_bits));
        }
        let strings: Vec<Option<String>> = keys.strings.iter().map(|name| src.read_string(name)).collect();
        let resolved: Vec<bool> = keys.screens.iter().map(|s| s.vars.iter().all(|&i| values[i] != UNKNOWN)).collect();
        let log_seq = self.hub.log.seq();
        let mut revs = last.revs.clone();
        let mut any = log_seq != last.log_seq;
        for (rev, screen) in revs.iter_mut().zip(&keys.screens) {
            let moved = screen.vars.iter().any(|&i| values[i] != last.values[i]) || screen.strings.iter().any(|&i| strings[i] != last.strings[i]);
            if moved {
                *rev += 1;
                any = true;
            }
        }
        if !any {
            self.hub.note_ready(&resolved);
            return false;
        }
        let next = Arc::new(Snapshot { seq: last.seq + 1, log_seq, values, strings, revs });
        self.hub.published.store(next.seq, Ordering::Release);
        *self.hub.current.lock().unwrap_or_else(|e| e.into_inner()) = next.clone();
        self.last = next;
        self.hub.note_ready(&resolved);
        true
    }
}

struct SnapSource<'a> {
    keys: &'a Keys,
    snap: &'a Snapshot,
    dpr: f64,
    css: (f64, f64),
}

impl VarSource for SnapSource<'_> {
    fn read_var(&mut self, name: &str, unit: &str) -> Option<f64> {
        let (_, i) = self.keys.var_lookup.get(name)?.iter().find(|(u, _)| u == unit)?;
        let bits = self.snap.values[*i];
        (bits != UNKNOWN).then(|| f64::from_bits(bits))
    }

    fn read_string(&mut self, name: &str) -> Option<String> {
        self.snap.strings[*self.keys.str_lookup.get(name)?].clone()
    }

    fn read_env(&mut self, path: &str) -> Option<f64> {
        match path {
            "dpr" => Some(self.dpr),
            "w" => Some(self.css.0),
            "h" => Some(self.css.1),
            _ => None,
        }
    }
}

struct ScreenLive {
    live: LiveInputs,
    dpr: f64,
    css: (f64, f64),
    ready: bool,
    asked: Option<std::time::Instant>,
}

/// The render-thread half: one [`LiveInputs`] per screen, fed from the
/// newest snapshot and the shared change log.
pub struct LiveFeed {
    hub: Arc<Hub>,
    snap: Arc<Snapshot>,
    screens: Vec<Option<ScreenLive>>,
}

impl LiveFeed {
    fn refresh(&mut self) {
        if self.hub.published.load(Ordering::Acquire) != self.snap.seq {
            self.snap = self.hub.snapshot();
        }
    }

    fn event_revision_of(&mut self, screen: &str) -> u64 {
        self.refresh();
        let Some(i) = self.hub.keys.screen(screen) else { return 0 };
        self.snap.revs[i] + self.snap.log_seq
    }

    /// Moves when an input changed and at least once every redraw period.
    pub fn revision_of(&mut self, screen: &str) -> u64 {
        let events = self.event_revision_of(screen);
        let Some(i) = self.hub.keys.screen(screen) else { return 0 };
        events + (self.hub.now_ms() / self.hub.keys.screens[i].period_ms) as u64
    }

    /// Whether the feed has resolved the screen's variables, or
    /// [`READY_TIMEOUT`] passed since it first asked.
    pub fn ready_of(&mut self, screen: &str) -> bool {
        let Some(i) = self.hub.keys.screen(screen) else { return true };
        let Some(s) = self.screens[i].as_mut() else { return true };
        if !s.ready {
            let since = *s.asked.get_or_insert_with(std::time::Instant::now);
            s.ready = self.hub.ready[i].load(Ordering::Relaxed) >= READY_PUBLISHES || since.elapsed() >= READY_TIMEOUT;
        }
        s.ready
    }

    pub fn begin(&mut self, screen: &str, width_px: u32, height_px: u32, dpr: f64) {
        let Some(i) = self.hub.keys.screen(screen) else { return };
        let css = (f64::from(width_px) / dpr, f64::from(height_px) / dpr);
        let mut live = LiveInputs::with_read_set(self.hub.keys.screens[i].read.clone());
        live.skip_mfd_dom(true);
        live.own_dom_topic(own_dom_topic(&self.hub.keys.screens[i].display));
        self.screens[i] = Some(ScreenLive { live, dpr, css, ready: false, asked: None });
    }

    pub fn fill_sink(&mut self, screen: &str, sink: &mut (impl InputSink + ?Sized)) -> Changes {
        self.refresh();
        let Some(i) = self.hub.keys.screen(screen) else { return Changes::default() };
        let hub = &self.hub;
        let snap = &self.snap;
        let Some(s) = self.screens[i].as_mut() else { return Changes::default() };
        let mut src = SnapSource { keys: &hub.keys, snap, dpr: s.dpr, css: s.css };
        s.live.update_from(hub.log, &mut src, sink, hub.now_ms(), s.dpr)
    }
}

impl super::screen::Feed for LiveFeed {
    fn begin(&mut self, screen: &str, width_px: u32, height_px: u32, dpr: f64) {
        LiveFeed::begin(self, screen, width_px, height_px, dpr);
    }

    fn revision(&mut self, screen: &str) -> u64 {
        self.revision_of(screen)
    }

    fn ready(&mut self, screen: &str) -> bool {
        self.ready_of(screen)
    }

    fn fill(&mut self, screen: &str, inputs: &mut super::Inputs) {
        self.fill_sink(screen, inputs);
    }
}

/// The hub for `FBW_NATIVE_SCREENS`, made on first use by whichever side
/// asks first; making it arms the tap so topics sent before the first
/// screen exists are kept.
pub fn hub() -> &'static Arc<Hub> {
    static HUB: OnceLock<Arc<Hub>> = OnceLock::new();
    HUB.get_or_init(|| {
        let selection = super::screen::selection();
        let screens: Vec<(&'static str, String)> = selection.targets.iter().flat_map(|t| t.layers().into_iter().map(|(key, _, _, read)| (key, read))).collect();
        ARMED.store(!screens.is_empty(), Ordering::Relaxed);
        Hub::with_log(&screens, global_log())
    })
}

pub fn live_feed() -> LiveFeed {
    hub().feed()
}

/// `None` when no screen is native.
pub fn publisher() -> Option<Publisher> {
    (!super::screen::selection().is_empty()).then(|| hub().publisher())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    #[derive(Default)]
    struct Recording {
        vars: Vec<(String, String, f64)>,
        strings: Vec<(String, String)>,
        bus: Vec<(String, String)>,
        bus_strings: Vec<(String, String)>,
        env: Vec<(String, f64)>,
        generic: Vec<(String, String)>,
        fetched: Vec<(String, Vec<u8>)>,
        now: Option<f64>,
        dpr: Option<f64>,
    }

    impl InputSink for Recording {
        fn set_var(&mut self, name: &str, unit: &str, value: f64) {
            self.vars.push((name.into(), unit.into(), value));
        }
        fn set_string(&mut self, name: &str, value: &str) {
            self.strings.push((name.into(), value.into()));
        }
        fn set_bus(&mut self, topic: &str, json: &str) {
            self.bus.push((topic.into(), json.into()));
        }
        fn set_bus_string(&mut self, topic: &str, text: &str) {
            self.bus_strings.push((topic.into(), text.into()));
        }
        fn set_env_number(&mut self, path: &str, value: f64) {
            self.env.push((path.into(), value));
        }
        fn set_generic(&mut self, key: &str, json: &str) {
            self.generic.push((key.into(), json.into()));
        }
        fn set_fetched(&mut self, url: &str, body: &[u8]) {
            self.fetched.push((url.into(), body.to_vec()));
        }
        fn set_now(&mut self, ms: f64) {
            self.now = Some(ms);
        }
        fn set_dpr(&mut self, dpr: f64) {
            self.dpr = Some(dpr);
        }
    }

    #[derive(Default)]
    struct Store {
        vars: HashMap<(String, String), f64>,
        strings: HashMap<String, String>,
        env: HashMap<String, f64>,
    }

    impl VarSource for Store {
        fn read_env(&mut self, path: &str) -> Option<f64> {
            self.env.get(path).copied()
        }
        fn read_var(&mut self, name: &str, unit: &str) -> Option<f64> {
            self.vars.get(&(name.to_string(), unit.to_string())).copied()
        }
        fn read_string(&mut self, name: &str) -> Option<String> {
            self.strings.get(name).cloned()
        }
    }

    #[derive(Default)]
    struct Expected {
        vars: BTreeMap<(String, String), f64>,
        strings: BTreeMap<String, String>,
        bus: BTreeMap<String, Value>,
        generic: BTreeMap<String, Value>,
        fetched: BTreeMap<String, String>,
        env: BTreeMap<String, f64>,
        now_ms: f64,
        dpr: f64,
        skipped_bus: usize,
        skipped_generic: usize,
        skipped_fetched: usize,
    }

    fn parse_maybe(v: &Value) -> Value {
        if let Value::String(s) = v {
            if let Ok(p) = serde_json::from_str::<Value>(s) {
                return p;
            }
        }
        v.clone()
    }

    fn flatten_numbers(v: &Value, prefix: &str, out: &mut BTreeMap<String, f64>) {
        match v {
            Value::Number(n) => {
                out.insert(prefix.to_string(), n.as_f64().unwrap());
            }
            Value::Object(m) => {
                for (k, v) in m.iter().filter(|(k, _)| !k.contains('.')) {
                    let path = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                    flatten_numbers(v, &path, out);
                }
            }
            _ => {}
        }
    }

    fn state_env(s: &Value) -> BTreeMap<String, f64> {
        let mut env = BTreeMap::new();
        if let Some(e) = s.get("env").filter(|e| e.is_object()) {
            flatten_numbers(e, "", &mut env);
        }
        if let Some(f) = s.get("frozenAt").filter(|f| f.is_object()) {
            flatten_numbers(f, "frozenAt", &mut env);
        }
        env
    }

    fn expected_from_state(s: &Value) -> Expected {
        let mut e = Expected { dpr: 1.5, ..Default::default() };
        if let Some(vars) = s.get("vars").and_then(Value::as_object) {
            let mut keys: Vec<&String> = vars.keys().collect();
            keys.sort();
            for k in keys {
                let v = &vars[k];
                let value = match v {
                    Value::Number(n) => n.as_f64().unwrap(),
                    Value::Bool(b) => *b as i32 as f64,
                    _ => continue,
                };
                let (name, unit) = k.rsplit_once('|').unwrap_or((k, ""));
                e.vars.insert((name.to_string(), unit.to_ascii_lowercase()), value);
            }
        }
        if let Some(m) = s.get("strings").and_then(Value::as_object) {
            for (k, v) in m {
                if let Value::String(t) = v {
                    e.strings.insert(k.clone(), t.clone());
                }
            }
        }
        if let Some(m) = s.get("bus").and_then(Value::as_object) {
            for (k, v) in m {
                match v.get("data") {
                    Some(d) => {
                        e.bus.insert(k.clone(), d.clone());
                    }
                    None => e.skipped_bus += 1,
                }
            }
        }
        if let Some(m) = s.get("genericData").and_then(Value::as_object) {
            for (k, v) in m {
                match v.get("data") {
                    Some(d) => {
                        e.generic.insert(k.clone(), parse_maybe(d));
                    }
                    None => e.skipped_generic += 1,
                }
            }
        }
        if let Some(m) = s.get("fetched").and_then(Value::as_object) {
            for (k, v) in m {
                match v.get("body") {
                    Some(Value::String(b)) => {
                        e.fetched.insert(k.clone(), b.clone());
                    }
                    _ => e.skipped_fetched += 1,
                }
            }
        }
        if let Some(d) = s.pointer("/env/dpr").and_then(Value::as_f64) {
            e.dpr = d;
        }
        e.env = state_env(s);
        e.now_ms = match e.vars.get(&("E:SIMULATION TIME".to_string(), "seconds".to_string())) {
            Some(t) => t * 1000.0,
            None => s.pointer("/frozenAt/perf").and_then(Value::as_f64).unwrap_or(0.0),
        };
        e
    }

    fn replay(s: &Value, store: &mut Store, log: &InputLog) {
        if let Some(vars) = s.get("vars").and_then(Value::as_object) {
            for (k, v) in vars {
                let value = match v {
                    Value::Number(n) => n.as_f64().unwrap(),
                    Value::Bool(b) => *b as i32 as f64,
                    _ => continue,
                };
                let (name, unit) = k.rsplit_once('|').unwrap_or((k, ""));
                store.vars.insert((name.to_string(), unit.to_string()), value);
            }
        }
        store.env = state_env(s).into_iter().collect();
        if let Some(m) = s.get("strings").and_then(Value::as_object) {
            for (k, v) in m {
                if let Value::String(t) = v {
                    store.strings.insert(k.clone(), t.clone());
                }
            }
        }
        if let Some(m) = s.get("bus").and_then(Value::as_object) {
            let events: Vec<Value> = m
                .iter()
                .filter(|(_, v)| v.get("data").is_some())
                .map(|(k, v)| serde_json::json!({ "topic": k, "data": v["data"], "cached": v["cached"], "origin": v["origin"] }))
                .collect();
            for chunk in events.chunks(7) {
                log.generic_data("xp-ui.bus", &serde_json::json!({ "events": chunk }).to_string());
            }
        }
        if let Some(m) = s.get("genericData").and_then(Value::as_object) {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort_by_key(|k| k.as_str() != "EB_EVENTS");
            for k in keys {
                match m[k].get("data") {
                    Some(Value::String(data)) => log.generic_data(k, data),
                    Some(other) => log.set_generic(k, &other.to_string()),
                    None => {}
                }
            }
        }
        if let Some(m) = s.get("fetched").and_then(Value::as_object) {
            for (k, v) in m {
                if let Some(Value::String(b)) = v.get("body") {
                    log.set_fetched(k, b.as_bytes());
                }
            }
        }
    }

    fn golden_root() -> PathBuf {
        PathBuf::from(std::env::var("FBW_GOLDEN_DIR").unwrap_or_else(|_| "E:/golden-tour".to_string()))
    }

    fn anim(k: &(String, String)) -> bool {
        k.0.starts_with("A380_ANIM:")
    }

    fn check(sink: &Recording, exp: &Expected, env: bool) -> Vec<String> {
        let mut got_vars: BTreeMap<(String, String), f64> = BTreeMap::new();
        for (n, u, v) in &sink.vars {
            got_vars.insert((n.clone(), u.to_ascii_lowercase()), *v);
        }
        let mut errors = Vec::new();
        for (k, v) in exp.vars.iter().filter(|(k, _)| !anim(k)) {
            match got_vars.get(k) {
                None => errors.push(format!("var missing {k:?}")),
                Some(g) if g.to_bits() != v.to_bits() => errors.push(format!("var {k:?}: live {g} expected {v}")),
                _ => {}
            }
        }
        for k in got_vars.keys() {
            if !exp.vars.contains_key(k) {
                errors.push(format!("var extra {k:?}"));
            }
        }
        let got_strings: BTreeMap<String, String> = sink.strings.iter().cloned().collect();
        if got_strings != exp.strings {
            errors.push(format!("strings differ: live {got_strings:?} expected {:?}", exp.strings));
        }
        let mut got_bus: BTreeMap<String, Value> = sink.bus.iter().map(|(k, j)| (k.clone(), serde_json::from_str(j).unwrap())).collect();
        for (k, text) in &sink.bus_strings {
            got_bus.insert(k.clone(), Value::String(text.clone()));
        }
        let got_env: BTreeMap<String, f64> = sink.env.iter().cloned().collect();
        if env && got_env != exp.env {
            errors.push(format!("env differ: live {got_env:?} expected {:?}", exp.env));
        }
        if !env {
            for k in ["dpr", "w", "h"] {
                if got_env.get(k) != exp.env.get(k) {
                    errors.push(format!("env {k}: live {:?} expected {:?}", got_env.get(k), exp.env.get(k)));
                }
            }
        }
        if got_bus != exp.bus {
            for (k, v) in &exp.bus {
                if got_bus.get(k) != Some(v) {
                    errors.push(format!("bus {k}: live {:?} expected {v}", got_bus.get(k)));
                }
            }
            for k in got_bus.keys().filter(|k| !exp.bus.contains_key(*k)) {
                errors.push(format!("bus extra {k}"));
            }
        }
        let got_generic: BTreeMap<String, Value> = sink.generic.iter().map(|(k, j)| (k.clone(), serde_json::from_str(j).unwrap())).collect();
        if got_generic != exp.generic {
            for (k, v) in &exp.generic {
                if got_generic.get(k) != Some(v) {
                    errors.push(format!("generic {k}: live {:?} expected {v}", got_generic.get(k)));
                }
            }
            for k in got_generic.keys().filter(|k| !exp.generic.contains_key(*k)) {
                errors.push(format!("generic extra {k}"));
            }
        }
        let got_fetched: BTreeMap<String, String> = sink.fetched.iter().map(|(k, b)| (k.clone(), String::from_utf8(b.clone()).unwrap())).collect();
        if got_fetched != exp.fetched {
            errors.push(format!("fetched differ: live {:?} expected {:?}", got_fetched.keys().collect::<Vec<_>>(), exp.fetched.keys().collect::<Vec<_>>()));
        }
        if sink.now != Some(exp.now_ms) {
            errors.push(format!("now live {:?} expected {}", sink.now, exp.now_ms));
        }
        if sink.dpr != Some(exp.dpr) {
            errors.push(format!("dpr live {:?} expected {}", sink.dpr, exp.dpr));
        }

        errors
    }

    fn compare(display: &str, label: &str, state: &Value, skipped: &mut [usize; 4]) -> usize {
        let exp = expected_from_state(state);
        let log = InputLog::new();
        let mut store = Store::default();
        replay(state, &mut store, &log);
        let mut live = LiveInputs::with_read_set(read_set(display).unwrap().clone());
        let mut sink = Recording::default();
        let changes = live.update_from(&log, &mut store, &mut sink, exp.now_ms, exp.dpr);
        assert!(changes.changed(), "{label}: nothing pushed");

        let mut errors = check(&sink, &exp, true);

        let mut again = Recording::default();
        let second = live.update_from(&log, &mut store, &mut again, exp.now_ms, exp.dpr);
        if second.changed() || !again.vars.is_empty() || !again.bus.is_empty() || !again.bus_strings.is_empty() || !again.env.is_empty() || !again.generic.is_empty() || !again.fetched.is_empty() {
            errors.push(format!("second update pushed again: {second:?}"));
        }

        skipped[0] += exp.skipped_bus;
        skipped[1] += exp.skipped_generic;
        skipped[2] += exp.skipped_fetched;
        skipped[3] += exp.vars.keys().filter(|k| anim(k)).count();
        if !errors.is_empty() {
            for e in errors.iter().take(12) {
                eprintln!("{label}: {e}");
            }
        }
        errors.len()
    }

    fn state_path(root: &Path, tour: &str, capture: &str, display: &str) -> PathBuf {
        root.join(tour).join("captures").join(capture).join(format!("{display}.state.json"))
    }

    #[test]
    fn live_store_matches_the_recorded_state_files() {
        let root = golden_root();
        let picks: [(&str, &[&str]); 2] = [
            ("flight-v4/emu-10-cruise", &["0001_baseline", "0003_sd_ENG", "0044_final"]),
            ("yssy-omdb-v12", &["0001_baseline", "0008_mfd_system-menu", "0018_mfd_FMS_2_ACTIVE_F-PLN", "0028_mfd_FMS_2_ACTIVE_F-PLN_dropdown-1-F-PLN_INFO"]),
        ];
        let mut skipped = [0usize; 4];
        let (mut checked, mut failures) = (0, 0);
        for (tour, captures) in picks {
            if !root.join(tour).is_dir() {
                eprintln!("golden tour {tour} not found under {}; skipping", root.display());
                continue;
            }
            for capture in captures {
                for display in read_set_displays() {
                    let path = state_path(&root, tour, capture, display);
                    let Ok(text) = std::fs::read_to_string(&path) else { continue };
                    let state: Value = serde_json::from_str(&text).unwrap();
                    failures += compare(display, &format!("{tour}/{capture}/{display}"), &state, &mut skipped);
                    checked += 1;
                }
            }
        }
        eprintln!(
            "checked {checked} captures; recording quirks not compared: {} bus without data, {} generic without data, {} fetched without text body, {} CSS animation vars",
            skipped[0], skipped[1], skipped[2], skipped[3]
        );
        assert_eq!(failures, 0);
    }

    #[test]
    fn unchanged_values_are_not_pushed_again() {
        let log = InputLog::new();
        let mut store = Store::default();
        store.vars.insert(("L:X".into(), "number".into()), 3.0);
        let mut live = LiveInputs::with_read_set(ReadSet { vars: vec![("L:X".into(), "number".into()), ("L:MISSING".into(), "number".into())], ..Default::default() });
        let mut a = Recording::default();
        assert_eq!(live.update_from(&log, &mut store, &mut a, 10.0, 1.5).vars, 1);
        let mut b = Recording::default();
        assert!(!live.update_from(&log, &mut store, &mut b, 20.0, 1.5).changed());
        assert_eq!((b.now, b.dpr), (Some(20.0), Some(1.5)));
        store.vars.insert(("L:X".into(), "number".into()), 4.0);
        log.generic_data("xp-ui.bus", r#"{"events":[{"topic":"t","data":{"a":1}}]}"#);
        log.generic_data("xp-ui.bus", r#"{"events":[{"topic":"t","data":{"a":1}}]}"#);
        let mut c = Recording::default();
        let ch = live.update_from(&log, &mut store, &mut c, 30.0, 1.5);
        assert_eq!((ch.vars, ch.bus), (1, 1));
        assert_eq!(c.bus, vec![("t".to_string(), r#"{"a":1}"#.to_string())]);
        log.generic_data("xp-ui.bus", r#"{"events":[{"topic":"dom","data":"<html a=\"1\">"}]}"#);
        let mut e = Recording::default();
        live.update_from(&log, &mut store, &mut e, 35.0, 1.5);
        assert_eq!(e.bus_strings, vec![("dom".to_string(), "<html a=\"1\">".to_string())]);
        log.generic_data("xp-ui.bus", r#"{"events":[{"topic":"t","data":{"a":2}}]}"#);
        let mut d = Recording::default();
        assert_eq!(live.update_from(&log, &mut store, &mut d, 40.0, 1.5).bus, 1);
    }

    #[test]
    fn page_clock_numbers_are_made_up_from_the_sim_clock_when_the_store_has_none() {
        let log = InputLog::new();
        let set = ReadSet { env: vec!["connectedAt.perf".into(), "frozenAt.perf".into(), "frozenAt.frames".into(), "fmaChangedAt".into()], ..Default::default() };
        let mut live = LiveInputs::with_read_set(set);
        let mut store = Store::default();
        let mut a = Recording::default();
        live.update_from(&log, &mut store, &mut a, 5000.0, 1.0);
        let mut b = Recording::default();
        live.update_from(&log, &mut store, &mut b, 5500.0, 1.0);
        let a: BTreeMap<_, _> = a.env.into_iter().collect();
        let b: BTreeMap<_, _> = b.env.into_iter().collect();
        assert_eq!((a["connectedAt.perf"], a["frozenAt.perf"], a["frozenAt.frames"]), (0.0, 0.0, 1.0));
        assert_eq!((b["frozenAt.perf"], b["frozenAt.frames"]), (500.0, 2.0));
        assert!(!a.contains_key("fmaChangedAt") && !b.contains_key("connectedAt.perf"));
    }

    #[test]
    fn generic_payloads_follow_the_recorder() {
        let log = InputLog::new();
        log.generic_data("K", r#"{"x":[1,2]}"#);
        log.generic_data("PLAIN", "hello");
        log.generic_data("EB_EVENTS", r#"{"packagedId":1,"data":[{"topic":"A","data":null},{"topic":"B","data":{"v":1}},{"topic":"C","data":"7"}]}"#);
        let mut live = LiveInputs::with_read_set(ReadSet::default());
        let mut s = Recording::default();
        live.update_from(&log, &mut Store::default(), &mut s, 0.0, 1.0);
        let got: BTreeMap<_, _> = s.generic.into_iter().collect();
        assert_eq!(got["K"], r#"{"x":[1,2]}"#);
        assert_eq!(got["PLAIN"], "\"hello\"");
        assert_eq!(got["A"], "null");
        assert_eq!(got["B"], r#"{"v":1}"#);
        assert_eq!(got["C"], "7");
        assert!(got.contains_key("EB_EVENTS"));
    }

    #[test]
    fn the_tap_is_off_until_a_native_screen_exists() {
        assert!(!active());
        tap_generic_data("GATED", "1");
        let live = LiveInputs::new("ewd");
        assert!(active());
        assert!(live.var_count() > 100);
        let mut s = Recording::default();
        let mut live = live;
        live.update(&mut Store::default(), &mut s, 0.0, 1.0);
        assert!(!s.generic.iter().any(|(k, _)| k == "GATED"));
        drop(live);
        assert!(!active());
    }

    fn leaked_log() -> &'static InputLog {
        Box::leak(Box::new(InputLog::new()))
    }

    fn live_path(display: &str, label: &str, state: &Value) -> Vec<String> {
        let exp = expected_from_state(state);
        let log = leaked_log();
        let id: &'static str = Box::leak(label.to_string().into_boxed_str());
        let hub = Hub::with_log(&[(id, display.to_string())], log);
        let mut store = Store::default();
        replay(state, &mut store, log);
        let mut publisher = hub.publisher();
        let mut feed = hub.feed();
        let mut errors = Vec::new();
        if !publisher.publish(&mut store, exp.now_ms) {
            errors.push("first publish changed nothing".to_string());
        }
        let css = |k: &str| state.pointer(&format!("/env/{k}")).and_then(Value::as_f64).unwrap_or(0.0);
        feed.begin(id, (css("w") * exp.dpr).round() as u32, (css("h") * exp.dpr).round() as u32, exp.dpr);
        let first = feed.event_revision_of(id);
        let mut sink = Recording::default();
        feed.fill_sink(id, &mut sink);
        errors.extend(check(&sink, &exp, false));

        if publisher.publish(&mut store, exp.now_ms + 16.0) {
            errors.push("an unchanged frame published a snapshot".to_string());
        }
        if feed.event_revision_of(id) != first {
            errors.push("an unchanged frame bumped the revision".to_string());
        }
        let mut again = Recording::default();
        let pushed = feed.fill_sink(id, &mut again);
        if pushed.changed() {
            errors.push(format!("an unchanged frame pushed {pushed:?}"));
        }

        let moved = store.vars.iter().find(|((n, u), _)| read_set(display).unwrap().vars.iter().any(|(rn, ru)| rn == n && ru == u)).map(|(k, v)| (k.clone(), *v));
        if let Some((k, v)) = moved {
            store.vars.insert(k, v + 1.0);
            if !publisher.publish(&mut store, exp.now_ms + 32.0) || feed.event_revision_of(id) == first {
                errors.push("a changed variable did not bump the revision".to_string());
            }
            let mut third = Recording::default();
            if feed.fill_sink(id, &mut third).vars != 1 {
                errors.push("a changed variable was not pushed once".to_string());
            }
        }
        let before = feed.event_revision_of(id);
        log.generic_data("xp-ui.bus", r#"{"events":[{"topic":"late-topic","data":{"a":1}}]}"#);
        publisher.publish(&mut store, exp.now_ms + 48.0);
        if feed.event_revision_of(id) == before {
            errors.push("a bus change did not bump the revision".to_string());
        }
        let mut fourth = Recording::default();
        feed.fill_sink(id, &mut fourth);
        if fourth.bus != vec![("late-topic".to_string(), r#"{"a":1}"#.to_string())] {
            errors.push(format!("bus push was {:?}", fourth.bus));
        }
        errors
    }

    #[test]
    fn snapshot_path_matches_the_recorded_state_files() {
        let root = golden_root();
        let picks: [(&str, &[&str]); 2] = [
            ("flight-v4/emu-10-cruise", &["0001_baseline", "0003_sd_ENG", "0044_final"]),
            ("yssy-omdb-v12", &["0001_baseline", "0018_mfd_FMS_2_ACTIVE_F-PLN"]),
        ];
        let (mut checked, mut failures) = (0, 0);
        for (tour, captures) in picks {
            if !root.join(tour).is_dir() {
                eprintln!("golden tour {tour} not found under {}; skipping", root.display());
                continue;
            }
            for capture in captures {
                for display in read_set_displays() {
                    let Ok(text) = std::fs::read_to_string(state_path(&root, tour, capture, display)) else { continue };
                    let state: Value = serde_json::from_str(&text).unwrap();
                    let label = format!("{tour}/{capture}/{display}");
                    for e in live_path(display, &label, &state) {
                        eprintln!("{label}: {e}");
                        failures += 1;
                    }
                    checked += 1;
                }
            }
        }
        eprintln!("snapshot path: checked {checked} captures");
        assert_eq!(failures, 0);
    }

    #[test]
    fn snapshot_revisions_follow_only_what_a_screen_reads() {
        let log = leaked_log();
        let hub = Hub::with_log(&[("A", "bat".to_string()), ("B", "clock".to_string())], log);
        let clock = &read_set("clock").unwrap().vars;
        let bat = read_set("bat").unwrap().vars.iter().find(|v| !clock.contains(v)).unwrap().clone();
        let mut store = Store::default();
        let mut publisher = hub.publisher();
        let mut feed = hub.feed();
        assert_eq!((feed.revision_of("A"), feed.revision_of("B")), (0, 0));
        store.vars.insert(bat.clone(), 1.0);
        assert!(publisher.publish(&mut store, 10.0));
        let (a, b) = (feed.revision_of("A"), feed.revision_of("B"));
        assert!(a > 0);
        assert_eq!(b, 0);
        assert!(!publisher.publish(&mut store, 20.0));
        store.vars.insert(bat, 2.0);
        assert!(publisher.publish(&mut store, 30.0));
        assert!(feed.revision_of("A") > a);
        assert_eq!(feed.revision_of("B"), b);
        assert_eq!(feed.revision_of("UNKNOWN"), 0);
    }

    #[test]
    fn a_screens_revision_advances_with_time_alone() {
        let log = leaked_log();
        let hub = Hub::with_log(&[("A", "isislegacy".to_string()), ("B", "clock".to_string())], log);
        let mut store = Store::default();
        let mut publisher = hub.publisher();
        let mut feed = hub.feed();
        feed.begin("A", 100, 100, 1.0);
        publisher.publish(&mut store, 1000.0);
        let (a, b) = (feed.revision_of("A"), feed.revision_of("B"));
        assert!(!publisher.publish(&mut store, 1150.0));
        assert!(feed.revision_of("A") > a);
        assert_eq!(feed.revision_of("B"), b);
        assert!(!publisher.publish(&mut store, 2100.0));
        assert!(feed.revision_of("B") > b);
        let mut sink = Recording::default();
        feed.fill_sink("A", &mut sink);
        assert_eq!(sink.now, Some(2100.0));
        let mut last = feed.revision_of("A");
        for t in (2200..6000).step_by(7) {
            publisher.publish(&mut store, f64::from(t));
            let now = feed.revision_of("A");
            assert!(now >= last);
            last = now;
        }
    }

    #[test]
    fn a_screen_is_ready_once_its_variables_resolved() {
        let log = leaked_log();
        let hub = Hub::with_log(&[("A", "bat".to_string())], log);
        let mut store = Store::default();
        let mut publisher = hub.publisher();
        let mut feed = hub.feed();
        assert!(feed.ready_of("NOPE"));
        feed.begin("A", 100, 100, 1.0);
        for t in 0..4 {
            publisher.publish(&mut store, f64::from(t) * 16.0);
        }
        assert!(!feed.ready_of("A"));
        for (name, unit) in &read_set("bat").unwrap().vars {
            store.vars.insert((name.clone(), unit.clone()), 1.0);
        }
        publisher.publish(&mut store, 100.0);
        assert!(!feed.ready_of("A"));
        publisher.publish(&mut store, 116.0);
        assert!(feed.ready_of("A"));
    }

    #[test]
    fn topics_sent_before_a_screen_exists_are_replayed_when_it_appears() {
        let log = leaked_log();
        log.generic_data("xp-ui.bus", r#"{"events":[{"topic":"early","data":{"x":2}}]}"#);
        log.generic_data("K", "{\"y\":3}");
        let hub = Hub::with_log(&[("A", "bat".to_string())], log);
        let mut feed = hub.feed();
        feed.begin("A", 100, 100, 1.0);
        let mut sink = Recording::default();
        feed.fill_sink("A", &mut sink);
        assert_eq!(sink.bus, vec![("early".to_string(), r#"{"x":2}"#.to_string())]);
        assert_eq!(sink.generic, vec![("K".to_string(), r#"{"y":3}"#.to_string())]);
    }

    #[test]
    fn fetched_bodies_are_bounded() {
        let log = InputLog::new();
        for i in 0..(FETCHED_MAX_COUNT + 20) {
            log.set_fetched(&format!("u{i}"), b"x");
        }
        assert_eq!(lock(&log.inner).fetched.len(), FETCHED_MAX_COUNT);
        assert!(lock(&log.inner).fetched.contains_key(&format!("u{}", FETCHED_MAX_COUNT + 19)));
        log.set_fetched("big", &vec![0u8; FETCHED_MAX_BODY + 1]);
        assert!(!lock(&log.inner).fetched.contains_key("big"));
    }
}

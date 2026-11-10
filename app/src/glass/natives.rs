use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::Ordering;

use fbw_a380_systems::xphfbw_bridge::{Downlink, Session, Uplink};

use crate::renderer::snapshot::{seqlock_read, Overlay, Snapshot};

pub struct Frame {
    pub snapshot: RefCell<Option<Rc<Snapshot>>>,
}

impl Frame {
    pub fn take(&self, session: &Session) {
        let table = &session.slots;
        let header = table.header();
        let (frame, values) = seqlock_read(|| header.frame.load(Ordering::Acquire), || table.values().to_vec());
        let resolved = header.resolved.load(Ordering::Acquire);
        *self.snapshot.borrow_mut() = Some(Rc::new(Snapshot { frame, resolved, values }));
    }
}

pub struct View {
    pub view: u32,
    pub aircraft_dir: PathBuf,
    pub xp_root: PathBuf,
    pub runtime_dir: PathBuf,
    pub session: Rc<Session>,
    pub frame: Rc<Frame>,
    pub overlay: RefCell<Overlay>,
    pub slot_cache: RefCell<HashMap<(String, String), u32>>,
    pub call_seq: Cell<u64>,
    pub strings: RefCell<HashMap<String, String>>,
    pub game_strings: RefCell<HashMap<String, String>>,
    pub game_requested: RefCell<HashSet<String>>,
    pub game_gave_up: RefCell<HashSet<String>>,
    pub stash: RefCell<Vec<Downlink>>,
    pub loaded: Cell<Option<bool>>,
    pub errors: Cell<u32>,
}

thread_local! {
    static CALLS: Cell<u64> = const { Cell::new(0) };
    static START: std::time::Instant = std::time::Instant::now();
    static CURRENT: RefCell<Option<Rc<View>>> = const { RefCell::new(None) };
    static PATCHES: Vec<fbw_a380_systems::source_patch::SourcePatch> = fbw_a380_systems::static_source_patches();
}

pub fn set_current(view: Option<Rc<View>>) {
    CURRENT.with(|c| *c.borrow_mut() = view);
}

fn current() -> Option<Rc<View>> {
    CURRENT.with(|c| c.borrow().clone())
}

impl View {
    pub fn log(&self, level: u8, text: String) {
        self.session.uplink.push(&Uplink::Log { view: self.view, level, text }.encode());
    }

    fn slot_for(&self, name: &str, unit: &str) -> Option<u32> {
        let key = (name.to_owned(), unit.to_owned());
        if let Some(&slot) = self.slot_cache.borrow().get(&key) {
            return Some(slot);
        }
        let slot = self.session.slots.register(name, unit)?;
        self.slot_cache.borrow_mut().insert(key, slot);
        Some(slot)
    }

    fn snapshot(&self) -> Rc<Snapshot> {
        if self.frame.snapshot.borrow().is_none() {
            self.frame.take(&self.session);
        }
        self.frame.snapshot.borrow().clone().expect("taken above")
    }

    fn get_var(&self, name: &str, unit: &str) -> f64 {
        let Some(slot) = self.slot_for(name, unit) else { return 0.0 };
        let snap = self.snapshot();
        self.overlay.borrow_mut().resolve(slot, snap.raw(slot), snap.frame)
    }

    fn set_var(&self, name: &str, unit: &str, value: f64) {
        let Some(slot) = self.slot_for(name, unit) else { return };
        self.session.uplink.push(&Uplink::Write { slot, value }.encode());
        let frame = self.snapshot().frame;
        self.overlay.borrow_mut().set(slot, value, frame);
    }

    fn game_string(&self, name: &str) -> String {
        if let Some(v) = self.game_strings.borrow().get(name) {
            return v.clone();
        }
        if name == "FLIGHT NAVDATA DATE RANGE" {
            if let Some(range) = fbw_a380_systems::navdata_date_range(&self.xp_root) {
                self.game_strings.borrow_mut().insert(name.to_string(), range.clone());
                return range;
            }
        }
        if self.game_gave_up.borrow().contains(name) {
            return String::new();
        }
        if self.game_requested.borrow_mut().insert(name.to_string()) {
            self.session.uplink.push(&Uplink::GameString { view: self.view, name: name.to_string() }.encode());
        }
        let Some(ring) = self.session.downlinks.get(self.view as usize) else { return String::new() };
        let until = std::time::Instant::now() + std::time::Duration::from_millis(5_000);
        while std::time::Instant::now() < until {
            for record in ring.drain().into_iter().filter_map(|b| Downlink::decode(&b)) {
                if let Downlink::GameString { name: n, value } = &record {
                    self.game_strings.borrow_mut().insert(n.clone(), value.clone());
                }
                self.stash.borrow_mut().push(record);
            }
            if let Some(v) = self.game_strings.borrow().get(name) {
                return v.clone();
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        self.game_gave_up.borrow_mut().insert(name.to_string());
        String::new()
    }

    pub fn patched(&self, requested: &str, text: String) -> String {
        let clean = format!("/{}", requested.trim_start_matches(['/', '\\']).replace('\\', "/"));
        PATCHES.with(|patches| {
            fbw_a380_systems::source_patch::apply(patches, &clean, text, |ok, msg| {
                self.log(if ok { 0 } else { 2 }, format!("patch {}{msg}", if ok { "" } else { "ERROR: " }));
            })
        })
    }

    fn read_html_ui(&self, requested: &str) -> Option<String> {
        let text = crate::renderer::read_file(&self.aircraft_dir, requested)?;
        Some(self.patched(requested, text))
    }

    fn read_runtime(&self, requested: &str) -> Option<String> {
        let rel = requested.trim_start_matches(['/', '\\']);
        if rel.split(['/', '\\']).any(|p| p == "..") || rel.contains(':') {
            return None;
        }
        std::fs::read_to_string(self.runtime_dir.join(rel)).ok()
    }
}

fn arg_string(scope: &mut v8::PinScope, args: &v8::FunctionCallbackArguments, i: i32) -> String {
    let v = args.get(i);
    if v.is_undefined() || v.is_null() {
        return String::new();
    }
    v.to_rust_string_lossy(scope)
}

fn arg_number(scope: &mut v8::PinScope, args: &v8::FunctionCallbackArguments, i: i32) -> f64 {
    args.get(i).number_value(scope).unwrap_or(0.0)
}

fn set_string(scope: &mut v8::PinScope, rv: &mut v8::ReturnValue, s: &str) {
    if let Some(v) = v8::String::new(scope, s) {
        rv.set(v.into());
    }
}

fn set_optional_string(scope: &mut v8::PinScope, rv: &mut v8::ReturnValue, s: Option<String>) {
    match s {
        Some(s) => set_string(scope, rv, &s),
        None => rv.set_null(),
    }
}

fn snapshot(_scope: &mut v8::PinScope, _args: v8::FunctionCallbackArguments, _rv: v8::ReturnValue) {
    let Some(v) = current() else { return };
    let frame = v.snapshot().frame;
    v.overlay.borrow_mut().prune(frame);
}

fn now_ms(_scope: &mut v8::PinScope, _args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    rv.set_double(START.with(|s| s.elapsed().as_secs_f64() * 1000.0));
}

fn native_calls(_scope: &mut v8::PinScope, _args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    rv.set_double(CALLS.with(|c| c.replace(0)) as f64);
}

fn count() {
    CALLS.with(|c| c.set(c.get() + 1));
}

fn get_var(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    count();
    let Some(v) = current() else { return };
    let (name, unit) = (arg_string(scope, &args, 0), arg_string(scope, &args, 1));
    rv.set_double(v.get_var(&name, &unit));
}

fn slot(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    let Some(v) = current() else { return };
    let (name, unit) = (arg_string(scope, &args, 0), arg_string(scope, &args, 1));
    rv.set_double(v.slot_for(&name, &unit).map_or(-1.0, |s| s as f64));
}

fn get_slot(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    count();
    let Some(v) = current() else { return };
    let slot = arg_number(scope, &args, 0) as u32;
    let snap = v.snapshot();
    rv.set_double(v.overlay.borrow_mut().resolve(slot, snap.raw(slot), snap.frame));
}

fn set_slot(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, _rv: v8::ReturnValue) {
    let Some(v) = current() else { return };
    let (slot, value) = (arg_number(scope, &args, 0) as u32, arg_number(scope, &args, 1));
    v.session.uplink.push(&Uplink::Write { slot, value }.encode());
    let frame = v.snapshot().frame;
    v.overlay.borrow_mut().set(slot, value, frame);
}

fn set_var(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, _rv: v8::ReturnValue) {
    let Some(v) = current() else { return };
    let (name, unit, value) = (arg_string(scope, &args, 0), arg_string(scope, &args, 1), arg_number(scope, &args, 2));
    v.set_var(&name, &unit, value);
}

fn get_string(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    count();
    let Some(v) = current() else { return };
    let name = arg_string(scope, &args, 0);
    let value = v.strings.borrow().get(&name).cloned().unwrap_or_default();
    set_string(scope, &mut rv, &value);
}

fn set_string_native(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, _rv: v8::ReturnValue) {
    let Some(v) = current() else { return };
    let (name, value) = (arg_string(scope, &args, 0), arg_string(scope, &args, 1));
    v.strings.borrow_mut().insert(name.clone(), value.clone());
    v.session.uplink.push(&Uplink::SetString { name, value }.encode());
}

fn send_event(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, _rv: v8::ReturnValue) {
    let Some(v) = current() else { return };
    let name = arg_string(scope, &args, 0);
    let mut values = Vec::new();
    if let Ok(arr) = v8::Local::<v8::Array>::try_from(args.get(1)) {
        for i in 0..arr.length() {
            if let Some(x) = arr.get_index(scope, i) {
                values.push(x.number_value(scope).unwrap_or(0.0));
            }
        }
    }
    v.session.uplink.push(&Uplink::Event { name, values }.encode());
}

fn call(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    let Some(v) = current() else { return };
    let (name, args_json) = (arg_string(scope, &args, 0), arg_string(scope, &args, 1));
    let counter = v.call_seq.get() + 1;
    v.call_seq.set(counter);
    let id = ((v.view as u64) << 48) | counter;
    v.session.uplink.push(&Uplink::Call { view: v.view, id, name, args_json }.encode());
    rv.set_double(id as f64);
}

fn poll(scope: &mut v8::PinScope, _args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    count();
    let Some(v) = current() else { return };
    let mut items: Vec<v8::Local<v8::Value>> = Vec::new();
    let stashed = std::mem::take(&mut *v.stash.borrow_mut());
    let fresh: Vec<Downlink> = v.session.downlinks.get(v.view as usize).map(|r| r.drain().into_iter().filter_map(|b| Downlink::decode(&b)).collect()).unwrap_or_default();
    for record in stashed.into_iter().chain(fresh) {
        let entry: Vec<v8::Local<v8::Value>> = match record {
            Downlink::Reply { id, ok, text } => {
                let kind = v8::String::new(scope, "reply").unwrap().into();
                let id = v8::Number::new(scope, id as f64).into();
                let ok = v8::Boolean::new(scope, ok).into();
                let text = v8::String::new(scope, &text).unwrap_or_else(|| v8::String::empty(scope)).into();
                vec![kind, id, ok, text]
            }
            other => {
                let (kind, name, payload) = match other {
                    Downlink::HEvent { name } => ("h", name, String::new()),
                    Downlink::ProviderEvent { name, json } => ("provider", name, json),
                    Downlink::GameString { name, value } => {
                        v.game_strings.borrow_mut().insert(name.clone(), value.clone());
                        ("game", name, value)
                    }
                    Downlink::StringValue { name, value } => {
                        v.strings.borrow_mut().insert(name.clone(), value.clone());
                        ("string", name, value)
                    }
                    Downlink::Reply { .. } => unreachable!(),
                };
                let kind = v8::String::new(scope, kind).unwrap().into();
                let name = v8::String::new(scope, &name).unwrap_or_else(|| v8::String::empty(scope)).into();
                let payload = v8::String::new(scope, &payload).unwrap_or_else(|| v8::String::empty(scope)).into();
                vec![kind, name, payload]
            }
        };
        items.push(v8::Array::new_with_elements(scope, &entry).into());
    }
    rv.set(v8::Array::new_with_elements(scope, &items).into());
}

fn game_string(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    let Some(v) = current() else { return };
    let name = arg_string(scope, &args, 0);
    let value = v.game_string(&name);
    set_string(scope, &mut rv, &value);
}

fn stored_data(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    let Some(v) = current() else { return };
    let (op, key, value) = (arg_string(scope, &args, 0), arg_string(scope, &args, 1), arg_string(scope, &args, 2));
    let result = crate::renderer::stored_data::run(&v.xp_root, &op, &key, &value);
    set_string(scope, &mut rv, &result);
}

fn read_file(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    let Some(v) = current() else { return };
    let path = arg_string(scope, &args, 0);
    set_optional_string(scope, &mut rv, v.read_html_ui(&path));
}

fn binary_path(aircraft_dir: &std::path::Path, requested: &str) -> Option<std::path::PathBuf> {
    let clean = requested.replace('\\', "/");
    if let Some(name) = clean.strip_prefix("C:/Windows/Fonts/").or_else(|| clean.strip_prefix("c:/windows/fonts/")) {
        if name.contains('/') || name.contains("..") {
            return None;
        }
        return Some(std::path::Path::new("C:/Windows/Fonts").join(name));
    }
    let (base, rel) = match clean.strip_prefix("/VFS/") {
        Some(rest) => (aircraft_dir.to_path_buf(), rest),
        None => (aircraft_dir.join("html_ui"), clean.as_str()),
    };
    let rel = rel.trim_start_matches('/');
    if rel.split('/').any(|p| p == "..") || rel.contains(':') {
        return None;
    }
    Some(base.join(rel))
}

fn read_binary(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    let Some(v) = current() else { return };
    let path = arg_string(scope, &args, 0);
    let Some(bytes) = binary_path(&v.aircraft_dir, &path).and_then(|p| std::fs::read(p).ok()) else {
        rv.set_null();
        return;
    };
    let store = v8::ArrayBuffer::new_backing_store_from_vec(bytes).make_shared();
    let buffer = v8::ArrayBuffer::with_backing_store(scope, &store);
    rv.set(buffer.into());
}

fn read_runtime(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    let Some(v) = current() else { return };
    let path = arg_string(scope, &args, 0);
    set_optional_string(scope, &mut rv, v.read_runtime(&path));
}

fn log(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, _rv: v8::ReturnValue) {
    let Some(v) = current() else { return };
    let level = arg_number(scope, &args, 0).clamp(0.0, 2.0) as u8;
    let text = arg_string(scope, &args, 1);
    if level == 2 {
        v.errors.set(v.errors.get() + 1);
    }
    if v.errors.get() <= 200 || level < 2 {
        v.log(level, text);
    }
}

fn trace(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, _rv: v8::ReturnValue) {
    let text = arg_string(scope, &args, 0);
    crate::logging::log(&format!("glass trace: {text}"));
}

fn mag_var(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    let (lat, lon) = (arg_number(scope, &args, 0), arg_number(scope, &args, 1));
    rv.set_double(crate::renderer::wmm::declination(lat, lon));
}

fn loaded(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, _rv: v8::ReturnValue) {
    let Some(v) = current() else { return };
    let ok = args.get(0).boolean_value(scope);
    let text = arg_string(scope, &args, 1);
    v.loaded.set(Some(ok));
    v.session.uplink.push(&Uplink::Loaded { view: v.view, ok, text }.encode());
}

fn drain(scope: &mut v8::PinScope, _args: v8::FunctionCallbackArguments, _rv: v8::ReturnValue) {
    scope.perform_microtask_checkpoint();
}

fn run(scope: &mut v8::PinScope, args: v8::FunctionCallbackArguments, mut rv: v8::ReturnValue) {
    let code = args.get(0);
    let name = arg_string(scope, &args, 1);
    let ok = run_script(scope, code, &name);
    rv.set_bool(ok);
}

pub fn run_script(scope: &mut v8::PinScope, code: v8::Local<v8::Value>, name: &str) -> bool {
    v8::tc_scope!(let tc, scope);
    let Ok(source) = v8::Local::<v8::String>::try_from(code) else { return false };
    let file = v8::String::new(tc, name).unwrap();
    let origin = v8::ScriptOrigin::new(tc, file.into(), 0, 0, false, 0, None, false, false, false, None);
    let done = v8::Script::compile(tc, source, Some(&origin)).and_then(|s| s.run(tc)).is_some();
    if !done {
        report(tc, name);
    }
    done
}

pub fn report(tc: &mut v8::PinnedRef<v8::TryCatch<v8::HandleScope>>, name: &str) {
    let message = tc.exception().map(|e| e.to_rust_string_lossy(tc)).unwrap_or_else(|| "unknown exception".into());
    let stack = tc.stack_trace().map(|s| s.to_rust_string_lossy(tc)).unwrap_or_default();
    let text = format!("glass: {name}: {message}\n{stack}");
    match current() {
        Some(v) => v.log(2, text),
        None => crate::logging::log(&text),
    }
}

pub fn install(scope: &mut v8::PinScope, global: v8::Local<v8::Object>, data: v8::Local<v8::Object>) {
    let natives = v8::Object::new(scope);
    macro_rules! put {
        ($obj:expr, $name:expr, $f:expr) => {{
            let f = v8::Function::new(scope, $f).unwrap();
            let k = v8::String::new(scope, $name).unwrap();
            $obj.set(scope, k.into(), f.into());
        }};
    }
    put!(natives, "snapshot", snapshot);
    put!(natives, "getVar", get_var);
    put!(natives, "setVar", set_var);
    put!(natives, "slot", slot);
    put!(natives, "getSlot", get_slot);
    put!(natives, "setSlot", set_slot);
    put!(natives, "getString", get_string);
    put!(natives, "setString", set_string_native);
    put!(natives, "sendEvent", send_event);
    put!(natives, "call", call);
    put!(natives, "poll", poll);
    put!(natives, "gameString", game_string);
    put!(natives, "storedData", stored_data);
    put!(natives, "readFile", read_file);
    put!(natives, "log", log);
    put!(natives, "magVar", mag_var);
    put!(natives, "loaded", loaded);
    put!(data, "readHtmlUi", read_file);
    put!(data, "readRuntime", read_runtime);
    put!(data, "readBinary", read_binary);
    put!(data, "run", run);
    put!(data, "drain", drain);
    put!(data, "nowMs", now_ms);
    put!(data, "nativeCalls", native_calls);
    if std::env::var_os("XPHFBW_GLASS_TRACE").is_some() {
        put!(data, "trace", trace);
    }
    put!(data, "log", log);
    let k = v8::String::new(scope, "natives").unwrap();
    data.set(scope, k.into(), natives.into());
    let k = v8::String::new(scope, "__hostData").unwrap();
    global.set(scope, k.into(), data.into());
}

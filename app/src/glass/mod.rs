mod natives;
mod output;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use fbw_a380_systems::xphfbw_bridge::{ScreenBlock, Session};
use fbw_a380_systems::xphfbw_bridge_views::{device_size, supersample, ViewDef};

const ENGINE: &[(&str, &str)] = &[
    ("glass://web.js", include_str!("../../../glass/engine/web.js")),
    ("glass://dom.js", include_str!("../../../glass/engine/dom.js")),
    ("glass://html.js", include_str!("../../../glass/engine/html.js")),
    ("glass://css.js", include_str!("../../../glass/engine/css.js")),
    ("glass://env.js", include_str!("../../../glass/engine/env.js")),
    ("glass://fonts.js", include_str!("../../../glass/engine/fonts.js")),
    ("glass://layout.js", include_str!("../../../glass/engine/layout.js")),
    ("glass://scene.js", include_str!("../../../glass/engine/scene.js")),
    ("glass://canvas.js", include_str!("../../../glass/engine/canvas.js")),
    ("glass://input.js", include_str!("../../../glass/engine/input.js")),
    ("glass://encode.js", include_str!("../../../glass/engine/encode.js")),
    ("glass://boot.js", include_str!("../../../glass/engine/boot.js")),
];

static OWNED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
static INPUT: std::sync::Mutex<Vec<(String, fbw_a380_systems::xphfbw_bridge::Input)>> = std::sync::Mutex::new(Vec::new());

pub fn push_input(screen_id: &str, input: fbw_a380_systems::xphfbw_bridge::Input) {
    if let Ok(mut q) = INPUT.lock() {
        if q.len() < 4096 {
            q.push((screen_id.to_string(), input));
        }
    }
}

fn eval_in(isolate: &mut v8::OwnedIsolate, live: &Live, code: &str) -> Option<bool> {
    natives::set_current(Some(live.view.clone()));
    let out = {
        v8::scope!(let scope, isolate);
        let context = v8::Local::new(scope, &live.context);
        let scope = &mut v8::ContextScope::new(scope, context);
        v8::tc_scope!(let tc, scope);
        let source = v8::String::new(tc, code)?;
        let result = v8::Script::compile(tc, source, None).and_then(|s| s.run(tc));
        let value = match result {
            Some(v) => Some(v.boolean_value(tc)),
            None => {
                natives::report(tc, "glass://input");
                None
            }
        };
        tc.perform_microtask_checkpoint();
        value
    };
    natives::set_current(None);
    out
}

fn eval_string(isolate: &mut v8::OwnedIsolate, live: &Live, code: &str) -> String {
    natives::set_current(Some(live.view.clone()));
    let out = {
        v8::scope!(let scope, isolate);
        let context = v8::Local::new(scope, &live.context);
        let scope = &mut v8::ContextScope::new(scope, context);
        v8::tc_scope!(let tc, scope);
        match v8::String::new(tc, code).and_then(|src| v8::Script::compile(tc, src, None)).and_then(|s| s.run(tc)) {
            Some(v) => v.to_rust_string_lossy(tc),
            None => String::new(),
        }
    };
    natives::set_current(None);
    out
}

fn deliver_input(isolate: &mut v8::OwnedIsolate, lives: &mut [Live]) {
    let queued = INPUT.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default();
    for (screen, input) in queued {
        use fbw_a380_systems::xphfbw_bridge::InputKind;
        let kind = match input.kind {
            InputKind::Down => 0,
            InputKind::Up => 1,
            InputKind::Move => 2,
            InputKind::Wheel => 3,
            InputKind::Key => 4,
        };
        let mut views: Vec<&Live> = lives.iter().filter(|l| !l.failed && l.def.screen.eq_ignore_ascii_case(&screen)).collect();
        views.sort_by_key(|l| std::cmp::Reverse(l.def.layer));
        let target = if views.len() > 1 && kind != 4 {
            views.iter().copied().find(|l| eval_in(isolate, l, &format!("__glass.wouldHit({}, {})", input.x, input.y)) == Some(true)).or(views.last().copied())
        } else {
            views.first().copied()
        };
        if let Some(live) = target {
            eval_in(isolate, live, &format!("__glass.input({kind}, {}, {}, {}), true", input.x, input.y, input.delta));
            let index = live.def.index;
            if let Some(l) = lives.iter_mut().find(|l| l.def.index == index) {
                l.poked = true;
            }
        }
    }
}

pub fn owns(screen_id: &str) -> bool {
    OWNED.lock().map(|o| o.iter().any(|s| s.eq_ignore_ascii_case(screen_id))).unwrap_or(false)
}

fn release(screen_id: &str) {
    if let Ok(mut o) = OWNED.lock() {
        o.retain(|s| !s.eq_ignore_ascii_case(screen_id));
    }
}

const LOADED_TIMEOUT: Duration = Duration::from_secs(25);
const IDLE_TICK: Duration = Duration::from_millis(100);
const STATS_EVERY: Duration = Duration::from_secs(10);

pub fn selected(aircraft_root: &Path) -> Vec<String> {
    let from = |root: &Path| crate::settings::load(root).get("xphfbw.glass").and_then(|v| v.as_str().map(str::to_string));
    let xplane = crate::window::SHARED.get().map(|s| s.xplane_root.clone());
    let raw = std::env::var("XPHFBW_GLASS").ok().or_else(|| xplane.as_deref().and_then(from)).or_else(|| from(aircraft_root));
    raw.unwrap_or_default().split(',').map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty() && s != "off").collect()
}

fn folder(def: &ViewDef) -> String {
    def.gauge_url.split('/').nth(1).unwrap_or("").to_ascii_lowercase()
}

pub fn chosen(selected: &[String], defs: &[ViewDef]) -> Vec<bool> {
    let direct: Vec<bool> = defs.iter().map(|d| selected.iter().any(|s| s == "all" || *s == folder(d))).collect();
    defs.iter()
        .enumerate()
        .map(|(i, d)| if d.screen.is_empty() { direct[i] } else { defs.iter().zip(&direct).any(|(o, &on)| on && o.screen.eq_ignore_ascii_case(&d.screen)) })
        .collect()
}

fn label(def: &ViewDef) -> String {
    if def.screen.is_empty() { def.section.clone() } else { def.screen.clone() }
}

pub fn start(tag: String, defs: Vec<ViewDef>, fps: i32) {
    if defs.is_empty() {
        return;
    }
    let Some(shared) = crate::window::SHARED.get() else { return };
    let dirs = (shared.aircraft_dir.clone(), shared.xplane_root.clone(), shared.runtime_dir.clone());
    if let Ok(mut o) = OWNED.lock() {
        o.extend(defs.iter().filter(|d| !d.screen.is_empty()).map(|d| d.screen.clone()));
    }
    const FLIGHT: &[&str] = &["pfd", "nd", "ewd", "sd", "sdv2", "isislegacy", "snd"];
    let mut logic = Vec::new();
    let mut flight = Vec::new();
    let mut panels = Vec::new();
    for d in defs {
        if d.screen.is_empty() {
            logic.push(d);
        } else if FLIGHT.contains(&folder(&d).as_str()) {
            flight.push(d);
        } else {
            panels.push(d);
        }
    }
    for (name, group) in [("glass-logic", logic), ("glass-flight", flight), ("glass-panels", panels)] {
        if group.is_empty() {
            continue;
        }
        let names: Vec<String> = group.iter().map(label).collect();
        crate::logging::log(&format!("glass: {name} runs {} without Chromium", names.join(", ")));
        let (tag, dirs) = (tag.clone(), dirs.clone());
        let fallback = group.clone();
        let spawned = std::thread::Builder::new().name(name.into()).stack_size(16 << 20).spawn(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(tag, group, fps, dirs)));
            if let Err(e) = outcome {
                let why = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
                crate::logging::log(&format!("glass: stopped by a panic: {why}"));
            }
        });
        if spawned.is_err() {
            for d in &fallback {
                fall_back(d, "its thread did not start");
            }
        }
    }
}

fn divisor(def: &ViewDef) -> u32 {
    if def.screen.is_empty() {
        return 1;
    }
    match folder(def).as_str() {
        "pfd" | "nd" | "ewd" | "sd" | "sdv2" | "isislegacy" | "snd" => 1,
        "mfd" | "oit" | "rmp" | "fcu" => 2,
        "ife" => 6,
        _ => 3,
    }
}

struct Live {
    def: ViewDef,
    every: u32,
    poked: bool,
    view: Rc<natives::View>,
    context: v8::Global<v8::Context>,
    frame_fn: v8::Global<v8::Function>,
    out: Option<usize>,
    started: Instant,
    failed: bool,
    frames: u32,
    drawn: u32,
    js_ms: f64,
    js_max: f64,
    parse_ms: f64,
    sent: std::collections::HashMap<u64, Rc<serde_json::Value>>,
}

struct Screen {
    out: output::Output,
    renderer: Option<glass_render::Renderer>,
    dpr: f32,
    layers: Vec<(u32, Vec<Rc<serde_json::Value>>)>,
    fonts: serde_json::Value,
    dirty: bool,
    draw_ms: f64,
    render_ms: f64,
    draws: u32,
}

fn fall_back(def: &ViewDef, why: &str) {
    crate::logging::log(&format!("glass: {} goes back to Chromium: {why}", label(def)));
    if def.screen.is_empty() {
        crate::views::chromium_fallback_view(def.index);
    } else {
        release(&def.screen);
        crate::views::chromium_fallback(def.screen.clone());
    }
}

fn open_session(tag: &str) -> Option<Session> {
    for _ in 0..300 {
        if let Some(s) = Session::open(tag) {
            return Some(s);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    None
}

fn init_v8() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        v8::V8::set_flags_from_string("--max-semi-space-size=1 --stress-flush-code=false");
        let platform = v8::new_default_platform(1, false).make_shared();
        v8::V8::initialize_platform(platform);
        v8::V8::initialize();
    });
}

fn env_json(def: &ViewDef, aircraft_dir: &Path) -> String {
    let page = def.gauge_url.split('?').next().unwrap_or("");
    let headless = def.screen.is_empty();
    serde_json::json!({
        "live": true,
        "headless": headless,
        "epoch": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0),
        "env": {
            "view": def.index,
            "screen": def.screen,
            "aircraftDir": aircraft_dir.to_string_lossy().replace('\\', "/"),
            "dpr": if headless { 1.0 } else { supersample(&def.screen) },
            "w": if headless { 1 } else { def.width },
            "h": if headless { 1 } else { def.height },
            "href": format!("coui://html_ui/Pages/VCockpit/Instruments/{}", def.gauge_url),
            "pagePath": format!("/Pages/VCockpit/Instruments/{page}"),
        }
    })
    .to_string()
}

fn boot(isolate: &mut v8::OwnedIsolate, view: &Rc<natives::View>, def: &ViewDef, aircraft_dir: &Path) -> Option<(v8::Global<v8::Context>, v8::Global<v8::Function>)> {
    natives::set_current(Some(view.clone()));
    let result = (|| {
        v8::scope!(let scope, isolate);
        let context = v8::Context::new(scope, Default::default());
        let scope = &mut v8::ContextScope::new(scope, context);
        let global = context.global(scope);
        let json = v8::String::new(scope, &env_json(def, aircraft_dir))?;
        let data = v8::json::parse(scope, json)?;
        let data = v8::Local::<v8::Object>::try_from(data).ok()?;
        natives::install(scope, global, data);
        for (name, code) in ENGINE {
            let source = v8::String::new(scope, code)?;
            if !natives::run_script(scope, source.into(), name) {
                return None;
            }
        }
        let boot = v8::String::new(scope, "__glass.boot()")?;
        if !natives::run_script(scope, boot.into(), "glass://boot") {
            return None;
        }
        scope.perform_microtask_checkpoint();
        let key = v8::String::new(scope, "__glass")?;
        let g = global.get(scope, key.into())?;
        let g = v8::Local::<v8::Object>::try_from(g).ok()?;
        let key = v8::String::new(scope, "frame")?;
        let f = g.get(scope, key.into())?;
        let f = v8::Local::<v8::Function>::try_from(f).ok()?;
        Some((v8::Global::new(scope, context), v8::Global::new(scope, f)))
    })();
    natives::set_current(None);
    result
}

fn tick(isolate: &mut v8::OwnedIsolate, live: &Live, now: f64) -> String {
    natives::set_current(Some(live.view.clone()));
    let out = {
        v8::scope!(let scope, isolate);
        let context = v8::Local::new(scope, &live.context);
        let scope = &mut v8::ContextScope::new(scope, context);
        let f = v8::Local::new(scope, &live.frame_fn);
        v8::tc_scope!(let tc, scope);
        let undefined = v8::undefined(tc).into();
        let arg = v8::Number::new(tc, now).into();
        let out = match f.call(tc, undefined, &[arg]) {
            Some(v) => v.to_rust_string_lossy(tc),
            None => {
                natives::report(tc, "glass://frame");
                String::new()
            }
        };
        tc.perform_microtask_checkpoint();
        out
    };
    natives::set_current(None);
    out
}

fn record_frame(dir: &Path, id: &str, screen: &Screen) {
    let frames = dir.join(format!("{id}.frames"));
    if !frames.exists() {
        let header = serde_json::json!({
            "dpr": screen.dpr,
            "width": screen.out.width as f32 / screen.dpr,
            "height": screen.out.height as f32 / screen.dpr,
            "fonts": screen.fonts,
            "ops": [],
        });
        let _ = std::fs::write(dir.join(format!("{id}.scene.json")), header.to_string());
    }
    let mut flat = Vec::new();
    for (k, (_, ops)) in screen.layers.iter().enumerate() {
        for op in ops {
            let mut v = (**op).clone();
            if let Some(i) = v["i"].as_u64() {
                v["i"] = serde_json::Value::from(i + k as u64 * 1_000_000_000);
            }
            flat.push(v);
        }
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(frames) {
        let _ = writeln!(f, "{}", serde_json::Value::Array(flat));
    }
}

fn draw(screen: &mut Screen, gpu: &output::Gpu, html_ui: &str) -> Result<(), String> {
    if screen.renderer.is_none() {
        screen.renderer = Some(glass_render::Renderer::new(&gpu.device, html_ui, &screen.fonts, screen.dpr).map_err(|e| e.to_string())?);
    }
    let renderer = screen.renderer.as_mut().expect("made above");
    renderer.set_fonts(html_ui, &screen.fonts);
    if screen.out.target.is_none() {
        screen.out.target = Some(renderer.target(&screen.out.draw).map_err(|e| e.to_string())?);
    }
    screen.layers.sort_by_key(|l| l.0);
    let layers: Vec<&[Rc<serde_json::Value>]> = screen.layers.iter().map(|l| l.1.as_slice()).collect();
    let t = Instant::now();
    renderer.draw_layers(screen.out.target.as_ref().expect("made above"), &layers).map_err(|e| e.to_string())?;
    screen.render_ms += t.elapsed().as_secs_f64() * 1000.0;
    screen.out.present(gpu);
    Ok(())
}

fn run(tag: String, defs: Vec<ViewDef>, fps: i32, (aircraft_dir, xp_root, runtime_dir): (PathBuf, PathBuf, PathBuf)) {
    let Some(session) = open_session(&tag) else {
        for d in &defs {
            fall_back(d, "the plugin's session never opened");
        }
        return;
    };
    let session = Rc::new(session);
    let needs_gpu = defs.iter().any(|d| !d.screen.is_empty());
    let gpu = if needs_gpu {
        match output::gpu() {
            Some(g) => {
                crate::logging::log(&format!("glass: drawing on {}", g.adapter));
                Some(g)
            }
            None => {
                for d in &defs {
                    fall_back(d, "no Direct3D 11 device");
                }
                return;
            }
        }
    } else {
        None
    };
    init_v8();
    let mut isolate = v8::Isolate::new(v8::CreateParams::default().heap_limits(0, 384 << 20));
    isolate.set_microtasks_policy(v8::MicrotasksPolicy::Explicit);
    let frame = Rc::new(natives::Frame { snapshot: RefCell::new(None) });
    frame.take(&session);
    let html_ui = aircraft_dir.join("html_ui").to_string_lossy().replace('\\', "/");

    let mut screens: Vec<(String, Screen)> = Vec::new();
    let mut lives: Vec<Live> = Vec::new();
    for def in defs {
        let mut out_index = None;
        if let Some(gpu) = gpu.as_ref().filter(|_| !def.screen.is_empty()) {
            if let Some(i) = screens.iter().position(|(id, _)| id.eq_ignore_ascii_case(&def.screen)) {
                out_index = Some(i);
            } else {
                let (w, h) = device_size(&def.screen, def.width, def.height);
                let Some(block) = ScreenBlock::open(&tag, &def.screen, w, h) else {
                    fall_back(&def, "no screen block");
                    continue;
                };
                let Some(out) = output::Output::new(gpu, block) else {
                    fall_back(&def, "could not make its texture");
                    continue;
                };
                screens.push((def.screen.clone(), Screen { out, renderer: None, dpr: supersample(&def.screen) as f32, layers: Vec::new(), fonts: serde_json::Value::Array(Vec::new()), dirty: false, draw_ms: 0.0, render_ms: 0.0, draws: 0 }));
                out_index = Some(screens.len() - 1);
            }
        }
        let view = Rc::new(natives::View {
            view: def.index,
            aircraft_dir: aircraft_dir.clone(),
            xp_root: xp_root.clone(),
            runtime_dir: runtime_dir.clone(),
            session: session.clone(),
            frame: frame.clone(),
            overlay: RefCell::new(crate::renderer::snapshot::Overlay::new()),
            slot_cache: RefCell::new(Default::default()),
            call_seq: Cell::new(0),
            strings: RefCell::new(Default::default()),
            game_strings: RefCell::new(Default::default()),
            game_requested: RefCell::new(Default::default()),
            game_gave_up: RefCell::new(Default::default()),
            stash: RefCell::new(Vec::new()),
            loaded: Cell::new(None),
            errors: Cell::new(0),
        });
        let t0 = Instant::now();
        let Some((context, frame_fn)) = boot(&mut isolate, &view, &def, &aircraft_dir) else {
            fall_back(&def, "its page did not start (the errors are logged above)");
            continue;
        };
        crate::logging::log(&format!("glass: {} started in {:.0} ms", label(&def), t0.elapsed().as_secs_f64() * 1000.0));
        lives.push(Live { every: divisor(&def), poked: false, def, view, context, frame_fn, out: out_index, started: Instant::now(), failed: false, frames: 0, drawn: 0, js_ms: 0.0, js_max: 0.0, parse_ms: 0.0, sent: Default::default() });
    }
    if lives.is_empty() {
        return;
    }

    let started = Instant::now();
    let min_interval = Duration::from_secs_f64(1.0 / fps.max(1) as f64);
    let mut last_sim_frame = u64::MAX;
    let mut last_tick = Instant::now() - min_interval;
    let mut last_stats = Instant::now();
    let mut tick_no: u64 = 0;
    let mut record_check = Instant::now();
    let mut record_seen: Option<std::time::SystemTime> = None;
    let mut recording: Option<(PathBuf, Instant)> = None;
    loop {
        loop {
            let f = session.slots.header().frame.load(Ordering::Acquire);
            let since = last_tick.elapsed();
            if (f != last_sim_frame && since >= min_interval) || since >= IDLE_TICK {
                last_sim_frame = f;
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        last_tick = Instant::now();
        frame.take(&session);
        deliver_input(&mut isolate, &mut lives);
        let now = started.elapsed().as_secs_f64() * 1000.0;
        tick_no += 1;
        for live in lives.iter_mut().filter(|l| !l.failed) {
            if !live.poked && tick_no % live.every as u64 != 0 {
                continue;
            }
            live.poked = false;
            let t0 = Instant::now();
            let json = tick(&mut isolate, live, now);
            let js = t0.elapsed().as_secs_f64() * 1000.0;
            live.frames += 1;
            live.js_ms += js;
            live.js_max = live.js_max.max(js);
            if let (Some(i), false) = (live.out, json.is_empty()) {
                let tp = Instant::now();
                let parsed = serde_json::from_str::<serde_json::Value>(&json);
                live.parse_ms += tp.elapsed().as_secs_f64() * 1000.0;
                match parsed {
                    Ok(mut scene) => {
                        let items = match scene["ops"].take() {
                            serde_json::Value::Array(a) => a,
                            _ => Vec::new(),
                        };
                        let mut ops = Vec::with_capacity(items.len());
                        let mut next = std::collections::HashMap::with_capacity(items.len());
                        let mut missing = false;
                        for it in items {
                            let v = match it.as_u64() {
                                Some(id) => match live.sent.get(&id) {
                                    Some(v) => v.clone(),
                                    None => {
                                        missing = true;
                                        continue;
                                    }
                                },
                                None => Rc::new(it),
                            };
                            if let Some(id) = v["i"].as_u64() {
                                next.insert(id, v.clone());
                            }
                            ops.push(v);
                        }
                        live.sent = next;
                        if missing {
                            eval_in(&mut isolate, live, "__glass.forceFull = true");
                        }
                        let screen = &mut screens[i].1;
                        if let Some(list) = scene["fonts"].as_array().filter(|a| !a.is_empty()) {
                            let mut merged = screen.fonts.as_array().cloned().unwrap_or_default();
                            for f in list {
                                if !merged.contains(f) {
                                    merged.push(f.clone());
                                }
                            }
                            screen.fonts = serde_json::Value::Array(merged);
                        }
                        match screen.layers.iter_mut().find(|l| l.0 == live.def.layer) {
                            Some(l) => l.1 = ops,
                            None => screen.layers.push((live.def.layer, ops)),
                        }
                        screen.dirty = true;
                        live.drawn += 1;
                    }
                    Err(e) => live.view.log(2, format!("glass: scene was not JSON: {e}")),
                }
            }
            if live.view.loaded.get() != Some(true) && live.started.elapsed() > LOADED_TIMEOUT {
                live.failed = true;
                fall_back(&live.def, "it never reported loaded");
            }
        }
        if record_check.elapsed() >= Duration::from_secs(1) {
            record_check = Instant::now();
            let trigger = crate::logging::dir().parent().map(|d| d.join("record-glass"));
            if let Some(stamp) = trigger.as_ref().and_then(|t| std::fs::metadata(t).ok()).and_then(|m| m.modified().ok()) {
                if record_seen != Some(stamp) && stamp.elapsed().map_or(false, |e| e < Duration::from_secs(60)) {
                    record_seen = Some(stamp);
                    let secs = stamp.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
                    let dir = crate::logging::dir().join("glass-record").join(secs.to_string());
                    let _ = std::fs::create_dir_all(&dir);
                    crate::logging::log(&format!("glass: recording frames for 20 s into {}", dir.display()));
                    for (id, screen) in screens.iter() {
                        record_frame(&dir, id, screen);
                    }
                    recording = Some((dir, Instant::now() + Duration::from_secs(20)));
                }
            }
            if recording.as_ref().is_some_and(|(_, until)| Instant::now() > *until) {
                recording = None;
                crate::logging::log("glass: recording finished");
            }
        }
        if let Some(gpu) = gpu.as_ref() {
            for (id, screen) in screens.iter_mut().filter(|(_, s)| s.dirty) {
                screen.dirty = false;
                if let Some((dir, _)) = recording.as_ref() {
                    record_frame(dir, id, screen);
                }
                let t1 = Instant::now();
                if let Err(e) = draw(screen, gpu, &html_ui) {
                    crate::logging::log(&format!("glass: drawing {id} failed: {e}"));
                }
                screen.draw_ms += t1.elapsed().as_secs_f64() * 1000.0;
                screen.draws += 1;
            }
        }
        if last_stats.elapsed() >= STATS_EVERY {
            let secs = last_stats.elapsed().as_secs_f64();
            if std::env::var_os("XPHFBW_GLASS_GC_STATS").is_some() {
                isolate.low_memory_notification();
            }
            let heap = isolate.get_heap_statistics();
            for live in lives.iter_mut().filter(|l| !l.failed) {
                let n = live.frames.max(1) as f64;
                let breakdown = eval_string(&mut isolate, live, "__glass.perfReport ? __glass.perfReport() : ''");
                crate::logging::log(&format!(
                    "glass: {} {:.0} updates/s, {:.0} changed/s, js {:.2} ms (max {:.1}) parse {:.2} ms/changed [{breakdown}]",
                    label(&live.def),
                    live.frames as f64 / secs,
                    live.drawn as f64 / secs,
                    live.js_ms / n,
                    live.js_max,
                    live.parse_ms / live.drawn.max(1) as f64
                ));
                live.parse_ms = 0.0;
                live.frames = 0;
                live.drawn = 0;
                live.js_ms = 0.0;
                live.js_max = 0.0;
            }
            for (id, screen) in screens.iter_mut() {
                let d = screen.draws.max(1) as f64;
                crate::logging::log(&format!("glass: {id} drawn {:.0}/s, draw {:.2} ms (render {:.2}, present {:.2}), presented {}{}", screen.draws as f64 / secs, screen.draw_ms / d, screen.render_ms / d, (screen.draw_ms - screen.render_ms) / d, screen.out.presented, if screen.out.cpu_path() { " via CPU" } else { "" }));
                if let Some(r) = screen.renderer.as_mut().filter(|r| r.empty_clips > 0) {
                    crate::logging::log(&format!("glass: {id} drew {} empty clip paths as clip-everything, e.g. {}", r.empty_clips, r.empty_clip_sample));
                    r.empty_clips = 0;
                }
                screen.draws = 0;
                screen.draw_ms = 0.0;
                screen.render_ms = 0.0;
            }
            crate::logging::log(&format!(
                "glass: {} V8 heap {:.1} MB used of {:.1} MB",
                std::thread::current().name().unwrap_or("glass"),
                heap.used_heap_size() as f64 / 1048576.0,
                heap.total_heap_size() as f64 / 1048576.0
            ));
            last_stats = Instant::now();
        }
        if lives.iter().all(|l| l.failed) {
            return;
        }
    }
}

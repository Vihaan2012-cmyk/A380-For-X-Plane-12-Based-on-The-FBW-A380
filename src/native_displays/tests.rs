use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{Display, DrawError, GpuInit, Inputs};

const STATE: &str = "E:/golden-tour/flight-v4/emu-10-cruise/captures/0001_baseline/nd-1.state.json";
const REFERENCE: &str = "E:/fbw-build/a380-displays/gpu152/a380disp_render.exe";

fn parse_maybe(v: &Value) -> Value {
    match v {
        Value::String(s) => serde_json::from_str(s).unwrap_or_else(|_| v.clone()),
        _ => v.clone(),
    }
}

fn wrapped<'a>(v: &'a Value, key: &str) -> &'a Value {
    match v.as_object() {
        Some(o) if o.contains_key(key) => &o[key],
        _ => v,
    }
}

/// `Inputs::from_state_json`, through the C ABI's setters.
fn load_state(inputs: &mut Inputs, state: &Value) -> f64 {
    let mut dpr = 1.5;
    let mut named = std::collections::HashSet::new();
    if let Some(vars) = state.get("vars").and_then(Value::as_object) {
        for (k, v) in vars {
            let value = match v {
                Value::Number(n) => n.as_f64().unwrap(),
                Value::Bool(b) => f64::from(*b),
                _ => continue,
            };
            let (name, unit) = k.rsplit_once('|').unwrap_or((k, ""));
            named.insert(name.to_string());
            assert!(inputs.set_var(name, unit, value), "{k}");
        }
    }
    if let Some(strings) = state.get("strings").and_then(Value::as_object) {
        for (k, v) in strings {
            if let Some(s) = v.as_str() {
                assert!(inputs.set_string(k, s));
            }
        }
    }
    if let Some(bus) = state.get("bus").and_then(Value::as_object) {
        for (k, v) in bus {
            let data = wrapped(v, "data");
            match data {
                Value::String(s) => assert!(inputs.set_bus_string(k, s)),
                _ => assert!(inputs.set_bus(k, &data.to_string())),
            }
        }
    }
    if let Some(generic) = state.get("genericData").and_then(Value::as_object) {
        for (k, v) in generic {
            let data = parse_maybe(wrapped(v, "data"));
            let text = match &data {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            assert!(inputs.set_generic(k, &text));
        }
    }
    if let Some(fetched) = state.get("fetched").and_then(Value::as_object) {
        for (k, v) in fetched {
            if let Some(body) = v.get("body") {
                let text = body.as_str().map_or_else(|| body.to_string(), str::to_string);
                assert!(inputs.set_fetched(k, text.as_bytes()));
            }
        }
    }
    if let Some(env) = state.get("env").and_then(Value::as_object) {
        if let Some(d) = env.get("dpr").and_then(Value::as_f64) {
            dpr = d;
        }
    }
    if let Some(animations) = state.get("animations").and_then(Value::as_array) {
        for a in animations {
            if let (Some(name), Some(time)) = (a.get("name").and_then(Value::as_str), a.get("time").and_then(Value::as_f64)) {
                let name = format!("A380_ANIM:{name}");
                if !named.contains(&name) {
                    inputs.set_var(&name, "", time);
                }
            }
        }
    }
    let sim_time = state.get("vars").and_then(|v| v.get("E:SIMULATION TIME|seconds")).and_then(Value::as_f64);
    let perf = state.get("frozenAt").and_then(|f| f.get("perf")).and_then(Value::as_f64);
    if let Some(ms) = sim_time.map(|s| s * 1000.).or(perf) {
        inputs.set_now(ms);
    }
    inputs.set_dpr(dpr);
    dpr
}

fn decode_png(path: &Path) -> (u32, u32, usize, Vec<u8>) {
    let mut reader = png::Decoder::new(std::fs::File::open(path).unwrap()).read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    let channels = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        other => panic!("unexpected PNG colour type {other:?}"),
    };
    (info.width, info.height, channels, buf)
}

fn scratch_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("fbw-native-displays-test");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn nd_drawn_through_the_wrapper_equals_a380disp_render_byte_for_byte() {
    let state_path = std::env::var("A380D_TEST_STATE").unwrap_or_else(|_| STATE.to_string());
    if !Path::new(&state_path).exists() || !Path::new(REFERENCE).exists() {
        eprintln!("skipped: {state_path} or {REFERENCE} is missing");
        return;
    }
    let out = scratch_dir().join("nd1_reference.png");
    let _ = std::fs::remove_file(&out);
    let status = std::process::Command::new(REFERENCE).args(["nd", "1", &state_path]).arg(&out).status().unwrap();
    assert!(status.success(), "a380disp_render failed: {status}");
    let (rw, rh, channels, reference) = decode_png(&out);

    let state: Value = serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
    let (width, height, pixels) = draw_on_gpu_thread(move || {
        let mut display = Display::new("nd", "1").expect("nd display");
        let mut inputs = Inputs::new();
        let dpr = load_state(&mut inputs, &state);
        let (cw, ch) = display.size();
        let (w, h) = ((cw as f64 * dpr).round() as u32, (ch as f64 * dpr).round() as u32);
        let mut rgba = vec![0u8; w as usize * h as usize * 4];
        assert_eq!(display.draw(&inputs, &mut rgba, w, h, w as usize * 4), Ok(true));
        assert_eq!(display.draw(&inputs, &mut rgba, w, h, w as usize * 4), Ok(false), "an unchanged picture reports no change");
        let mut small = vec![0u8; 16];
        assert_eq!(display.draw(&inputs, &mut small, w, h, w as usize * 4), Err(DrawError::Buffer));
        assert_eq!(display.draw(&inputs, &mut small, 4, 1, 8), Err(DrawError::Buffer));
        assert_eq!(display.draw(&inputs, &mut small, 0, 0, 0), Err(DrawError::Buffer));
        (w, h, rgba)
    });

    assert_eq!((width, height), (rw, rh));
    let mut differing = 0usize;
    let mut first = None;
    for (i, (ours, theirs)) in pixels.chunks_exact(4).zip(reference.chunks_exact(channels)).enumerate() {
        if ours[..3] != theirs[..3] {
            differing += 1;
            first.get_or_insert(i);
        }
    }
    assert_eq!(differing, 0, "{differing} pixels differ, first at index {first:?}");
    assert!(pixels.chunks_exact(4).all(|p| p[3] == 255), "the picture is opaque");
}

const CAPTURE: &str = "E:/golden-tour/flight-v4/emu-10-cruise/captures/0001_baseline";
const UNDERLAY_CAPTURE: &str = "E:/golden-tour/flight-v4/emu-08-initial-climb/captures/0044_final";

type Job = Box<dyn FnOnce() + Send>;

fn gpu_thread() -> &'static std::sync::Mutex<std::sync::mpsc::Sender<Job>> {
    static THREAD: std::sync::OnceLock<std::sync::Mutex<std::sync::mpsc::Sender<Job>>> = std::sync::OnceLock::new();
    THREAD.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<Job>();
        std::thread::spawn(move || {
            assert!(matches!(super::gpu_init(), Ok(GpuInit::Gpu) | Ok(GpuInit::CpuOnly)));
            while let Ok(job) = rx.recv() {
                job();
            }
        });
        std::sync::Mutex::new(tx)
    })
}

fn draw_on_gpu_thread<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = std::sync::mpsc::channel();
    let job: Job = Box::new(move || {
        let _ = tx.send(std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)));
    });
    gpu_thread().lock().unwrap_or_else(|e| e.into_inner()).send(job).unwrap();
    match rx.recv().unwrap() {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

fn load_inputs(path: &str) -> (Inputs, f64) {
    let state: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut inputs = Inputs::new();
    let dpr = load_state(&mut inputs, &state);
    (inputs, dpr)
}

fn surface_of(display: &Display, dpr: f64) -> (u32, u32) {
    let (cw, ch) = display.size();
    ((cw as f64 * dpr).round() as u32, (ch as f64 * dpr).round() as u32)
}

#[test]
fn nd_with_an_underlay_equals_a380disp_render_layer_byte_for_byte() {
    let state_path = format!("{UNDERLAY_CAPTURE}/nd-1.state.json");
    let layer_path = format!("{UNDERLAY_CAPTURE}/nd-1.layer.png");
    if !Path::new(&state_path).exists() || !Path::new(&layer_path).exists() || !Path::new(REFERENCE).exists() {
        eprintln!("skipped: {state_path}, {layer_path} or {REFERENCE} is missing");
        return;
    }
    let out = scratch_dir().join("nd1_layer_reference.png");
    let _ = std::fs::remove_file(&out);
    let status = std::process::Command::new(REFERENCE).args(["nd", "1", &state_path]).arg(&out).args(["--layer", &layer_path]).status().unwrap();
    assert!(status.success(), "a380disp_render failed: {status}");
    let (rw, rh, channels, reference) = decode_png(&out);
    let (lw, lh, lchannels, layer) = decode_png(Path::new(&layer_path));
    assert_eq!(lchannels, 4);

    let (width, height, with, without, removed) = draw_on_gpu_thread(move || {
        let mut display = Display::new("nd", "1").expect("nd display");
        let (inputs, dpr) = load_inputs(&state_path);
        let (w, h) = surface_of(&display, dpr);
        let stride = w as usize * 4;
        let mut bare = vec![0u8; stride * h as usize];
        assert_eq!(display.draw(&inputs, &mut bare, w, h, stride), Ok(true));
        assert!(display.set_underlay(Some((&layer, lw, lh))));
        let mut with = vec![0u8; stride * h as usize];
        assert_eq!(display.draw(&inputs, &mut with, w, h, stride), Ok(true), "a new underlay changes the picture");
        assert_eq!(display.draw(&inputs, &mut with, w, h, stride), Ok(false));
        assert!(!display.set_underlay(Some((&layer[..16], lw, lh))));
        assert!(display.set_underlay(None));
        let mut removed = vec![0u8; stride * h as usize];
        display.draw(&inputs, &mut removed, w, h, stride).unwrap();
        (w, h, with, bare, removed)
    });
    assert_eq!((width, height), (rw, rh));
    let differing = with.chunks_exact(4).zip(reference.chunks_exact(channels)).filter(|(a, b)| a[..3] != b[..3]).count();
    assert_eq!(differing, 0, "{differing} pixels differ from a380disp_render --layer");
    assert_ne!(with, without);
    assert_eq!(without, removed, "removing the underlay restores the bare picture");
}

#[test]
fn sd_layers_draw_sdv2_over_sd_and_match_the_composited_golden() {
    let sd_state = format!("{CAPTURE}/sd.state.json");
    let v2_state = format!("{CAPTURE}/sdv2.state.json");
    let sd_png = format!("{CAPTURE}/sd.png");
    let v2_rgba = format!("{CAPTURE}/sdv2.rgba.png");
    if ![&sd_state, &v2_state, &sd_png, &v2_rgba].iter().all(|p| Path::new(p.as_str()).exists()) {
        eprintln!("skipped: the SD captures are missing");
        return;
    }
    let (w, h, ch3, golden) = decode_png(Path::new(&sd_png));
    let (vw, vh, vch, overlay) = decode_png(Path::new(&v2_rgba));
    assert_eq!((w, h, vw, vh, vch), (w, h, w, h, 4));

    let (width, height, composed, single, alone) = draw_on_gpu_thread(move || {
        let mut sd = Display::new("sd", "").expect("sd display");
        let mut sdv2 = Display::new("sdv2", "").expect("sdv2 display");
        let (sd_in, dpr) = load_inputs(&sd_state);
        let (v2_in, _) = load_inputs(&v2_state);
        let (w, h) = surface_of(&sd, dpr);
        let stride = w as usize * 4;
        let len = stride * h as usize;
        let mut alone = vec![0u8; len];
        assert_eq!(sd.draw(&sd_in, &mut alone, w, h, stride), Ok(true));
        let mut single = vec![0u8; len];
        super::draw_layers(&mut [(&mut sd, &sd_in)], &mut single, w, h, stride).unwrap();
        let mut composed = vec![0u8; len];
        assert_eq!(super::draw_layers(&mut [(&mut sd, &sd_in), (&mut sdv2, &v2_in)], &mut composed, w, h, stride), Ok(true));
        assert_eq!(super::draw_layers(&mut [(&mut sd, &sd_in), (&mut sdv2, &v2_in)], &mut composed, w, h, stride), Ok(false));
        assert_eq!(super::draw_layers(&mut [], &mut composed, w, h, stride), Err(DrawError::Buffer));
        assert!(matches!(super::draw_layers(&mut [(&mut sd, &sd_in)], &mut composed, w - 1, h, stride), Err(DrawError::Failed(_, _))));
        (w, h, composed, single, alone)
    });
    assert_eq!((width, height), (w, h));
    assert_eq!(single, alone, "one layer is the plain draw");

    let mut lit = 0usize;
    let mut good = 0usize;
    for ((native, base), over) in composed.chunks_exact(4).zip(golden.chunks_exact(ch3)).zip(overlay.chunks_exact(4)) {
        let a = f32::from(over[3]) / 255.;
        let reference: Vec<f32> = (0..3).map(|c| f32::from(over[c]) * a + f32::from(base[c]) * (1. - a)).collect();
        let bright = reference.iter().any(|v| *v > 24.) || native[..3].iter().any(|v| *v > 24);
        if bright {
            lit += 1;
            if (0..3).all(|c| (reference[c] - f32::from(native[c])).abs() <= 16.) {
                good += 1;
            }
        }
    }
    let score = 100. * good as f64 / lit.max(1) as f64;
    eprintln!("composed SD vs sd.png + sdv2.rgba.png: {score:.2}% over {lit} lit pixels");
    assert!(score > 97., "composed SD matches the reference at {score:.2}%");
}

//! A live run of XPHFBW's JS pipeline with no X-Plane: this test plays the
//! plugin's part (docs/briefs/xphfbw-js-bridge.md) — it creates the session
//! and every cockpit screen, starts the real XPHFBW.exe on the real aircraft,
//! ticks the remote systems and the bridge in lockstep at 60 Hz, prints what
//! the views report, and writes each screen's last picture to a PNG.
//!
//! ```text
//! XPHFBW_EXE=".../plugins/fbw_a380_systems/XPHFBW/XPHFBW.exe" \
//! XPHFBW_AIRCRAFT=".../Aircraft/A380 for X-Plane 12" \
//! XPHFBW_XP_ROOT=".../X-Plane 12" XPHFBW_OUT="E:/fbw-build/xphfbw-harness" \
//! XPHFBW_SECONDS=90 cargo test --release --features js --lib xphfbw_harness -- --ignored --nocapture
//! ```
//!
//! `xphfbw_alert_shots` boots the same way, then forces each alert listed in
//! `XPHFBW_ALERTS` (one FWS id per line) through the SystemsHost hook
//! `L:A32NX_FWS_FORCE_HI`/`_LO` and writes `<id>_EWD.png` and `<id>_SD.png`.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use systems::simulation::{SimulatorReaderWriter, StartState, VariableIdentifier, VariableRegistry};

use crate::display::screens::SCREENS;
use crate::mapdata::terrain::types::Side;
use crate::wxr::sampler::{Sample, SyntheticStorm, WeatherSampler};
use crate::wxr::Aircraft;
use crate::remote::client::RemoteSystems;
use crate::xp::Xplm;
use crate::xphfbw_bridge::ScreenBlock;
use crate::xphfbw_host::XphfbwHost;

fn env_path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var(name).unwrap_or_else(|_| panic!("set {name}")))
}

fn seed_from_snapshot(vars: &mut crate::Vars) -> Vec<(VariableIdentifier, f64, bool)> {
    let Ok(path) = std::env::var("XPHFBW_SEED_VARS") else { return Vec::new() };
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    let map = json.get("vars").and_then(|v| v.as_object()).unwrap_or_else(|| panic!("{path}: no vars object"));
    let mut pinned = Vec::new();
    for (name, value) in map {
        let Some(value) = value.as_f64() else { continue };
        let id = vars.get_unprefixed(name.clone());
        vars.write(&id, value);
        if name.contains(' ') {
            pinned.push((id, value, name.contains("HEADING")));
        }
    }
    eprintln!("harness: seeded {} variables from {path}, {} simulator inputs pinned", map.len(), pinned.len());
    pinned
}

fn write_png(path: &std::path::Path, block: &ScreenBlock) {
    let h = block.header();
    let (w, hgt) = (h.width.load(Ordering::Relaxed), h.height.load(Ordering::Relaxed));
    let mut rgba = block.pixels().to_vec();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
        // Premultiplied and possibly transparent: show it over black.
        px[3] = 255;
    }
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, hgt);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&rgba).unwrap();
}

fn write_rgba(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}

/// One convective or stratiform rain area: an ellipse `major_nm` x `minor_nm`
/// turned `heading_deg` (true) from north, its rain falling off from `peak`
/// at the centre with a Gaussian core, and turbulence only in the core.
struct RainCell {
    lat: f64,
    lon: f64,
    major_nm: f64,
    minor_nm: f64,
    heading_deg: f64,
    peak: f32,
    turbulence: f32,
    top_m: f64,
}

struct Storms {
    cells: Vec<RainCell>,
    round: Vec<SyntheticStorm>,
}

fn hash01(x: i64, y: i64) -> f64 {
    let mut h = (x.wrapping_mul(374_761_393) ^ y.wrapping_mul(668_265_263)) as u64;
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xffff) as f64 / 65535.
}

/// Smooth value noise in [-1, 1], about one feature per `1 / scale` NM.
fn noise(x: f64, y: f64) -> f64 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let (sx, sy) = (fx * fx * (3. - 2. * fx), fy * fy * (3. - 2. * fy));
    let (xi, yi) = (xi as i64, yi as i64);
    let a = hash01(xi, yi) + (hash01(xi + 1, yi) - hash01(xi, yi)) * sx;
    let b = hash01(xi, yi + 1) + (hash01(xi + 1, yi + 1) - hash01(xi, yi + 1)) * sx;
    (a + (b - a) * sy) * 2. - 1.
}

impl RainCell {
    fn at(&self, lat: f64, lon: f64) -> (f32, f32) {
        let east = (lon - self.lon) * 60. * self.lat.to_radians().cos();
        let north = (lat - self.lat) * 60.;
        let h = self.heading_deg.to_radians();
        let along = north * h.cos() + east * h.sin();
        let across = -north * h.sin() + east * h.cos();
        let warp_along = along + 0.9 * noise(east * 0.35 + 11.3, north * 0.35 - 7.9) * self.minor_nm;
        let warp_across = across + 0.9 * noise(east * 0.35 - 21.7, north * 0.35 + 4.2) * self.minor_nm;
        let r2 = (warp_along / self.major_nm).powi(2) + (warp_across / self.minor_nm).powi(2);
        let patchy = (1. + 0.3 * noise(east * 0.8 - 3.1, north * 0.8 + 5.7)).max(0.);
        let rain = self.peak as f64 * (-3. * r2).exp() * patchy;
        let turb = self.turbulence as f64 * (-10. * r2).exp();
        (rain as f32, turb as f32)
    }
}

impl WeatherSampler for Storms {
    fn sample(&mut self, lat: f64, lon: f64, alt_m: f64) -> Option<Sample> {
        let mut best = Sample::default();
        for cell in &self.cells {
            let (rain, turb) = cell.at(lat, lon);
            if rain > best.precip_rate_alt {
                best.precip_rate_alt = rain;
                best.storm_top_m = (rain > 0.02).then_some(cell.top_m);
            }
            best.turbulence_alt = best.turbulence_alt.max(turb);
        }
        for storm in &mut self.round {
            if let Some(s) = storm.sample(lat, lon, alt_m) {
                if s.precip_rate_alt > best.precip_rate_alt {
                    best = s;
                }
            }
        }
        Some(best)
    }
}

fn offset(lat: f64, lon: f64, bearing_deg: f64, distance_nm: f64) -> (f64, f64) {
    let b = bearing_deg.to_radians();
    (lat + distance_nm * b.cos() / 60., lon + distance_nm * b.sin() / (60. * lat.to_radians().cos()))
}

/// A squall line across the departure about 18 NM ahead (true bearing 330),
/// a supercell to the west-north-west, a stratiform rain band to the east
/// and scattered showers: the structure a real convective day shows on WXR.
fn storm_day(lat: f64, lon: f64) -> Vec<RainCell> {
    let mut cells = Vec::new();
    let line_bearing = 330.;
    let (line_lat, line_lon) = offset(lat, lon, line_bearing, 18.);
    let line_heading = 60.;
    for (k, (peak, turb)) in [(0.42, 0.0), (0.7, 0.55), (0.8, 0.85), (0.6, 0.0), (0.76, 0.7), (0.55, 0.0), (0.38, 0.0)].into_iter().enumerate() {
        let along = (k as f64 - 3.) * 4.6;
        let (clat, clon) = offset(line_lat, line_lon, line_heading, along);
        let jitter = (hash01(k as i64, 7) - 0.5) * 3.;
        let (clat, clon) = offset(clat, clon, line_heading + 90., jitter);
        cells.push(RainCell { lat: clat, lon: clon, major_nm: 3.6, minor_nm: 2.2, heading_deg: line_heading + jitter * 10., peak, turbulence: turb, top_m: 12_500. });
    }
    cells.push(RainCell { lat: line_lat, lon: line_lon, major_nm: 19., minor_nm: 3.2, heading_deg: line_heading, peak: 0.24, turbulence: 0., top_m: 10_000. });
    let (tlat, tlon) = offset(line_lat, line_lon, line_bearing, 4.5);
    cells.push(RainCell { lat: tlat, lon: tlon, major_nm: 17., minor_nm: 5., heading_deg: line_heading, peak: 0.12, turbulence: 0., top_m: 7_000. });
    let (slat, slon) = offset(lat, lon, 295., 34.);
    cells.push(RainCell { lat: slat, lon: slon, major_nm: 5.2, minor_nm: 4.2, heading_deg: 20., peak: 0.82, turbulence: 0.9, top_m: 14_000. });
    let (hlat, hlon) = offset(slat, slon, 200., 4.5);
    cells.push(RainCell { lat: hlat, lon: hlon, major_nm: 2.4, minor_nm: 1.1, heading_deg: 120., peak: 0.55, turbulence: 0.0, top_m: 13_000. });
    let (alat, alon) = offset(slat, slon, 60., 7.);
    cells.push(RainCell { lat: alat, lon: alon, major_nm: 9., minor_nm: 3.5, heading_deg: 60., peak: 0.14, turbulence: 0., top_m: 11_000. });
    let (blat, blon) = offset(lat, lon, 70., 22.);
    cells.push(RainCell { lat: blat, lon: blon, major_nm: 16., minor_nm: 5.5, heading_deg: 160., peak: 0.13, turbulence: 0., top_m: 6_000. });
    for (bearing, dist, size, peak) in [(15., 9., 1.4, 0.3), (250., 11., 1.8, 0.45), (350., 42., 2.6, 0.62), (40., 38., 2.2, 0.4), (300., 14., 1.2, 0.22)] {
        let (clat, clon) = offset(lat, lon, bearing, dist);
        cells.push(RainCell { lat: clat, lon: clon, major_nm: size * 1.3, minor_nm: size, heading_deg: bearing, peak, turbulence: 0., top_m: 9_000. });
    }
    cells
}

/// `XPHFBW_STORMS`: unset or `storm-day` for [`storm_day`]; `none` for clear
/// air; otherwise round cells `bearing_deg:distance_nm:radius_nm:precip:turbulence`
/// separated by `;`, bearings true from (lat, lon).
fn storms_around(lat: f64, lon: f64) -> Storms {
    let spec = std::env::var("XPHFBW_STORMS").unwrap_or_default();
    match spec.trim() {
        "" | "storm-day" => return Storms { cells: storm_day(lat, lon), round: Vec::new() },
        "none" => return Storms { cells: Vec::new(), round: Vec::new() },
        _ => {}
    }
    let round = spec
        .split(';')
        .filter_map(|cell| {
            let v: Vec<f64> = cell.split(':').filter_map(|x| x.trim().parse().ok()).collect();
            let [bearing, distance, radius, precip, turbulence] = v[..] else { return None };
            let (clat, clon) = offset(lat, lon, bearing, distance);
            Some(SyntheticStorm { lat: clat, lon: clon, radius_nm: radius, peak_precip: precip as f32, peak_turbulence: turbulence as f32 })
        })
        .collect();
    Storms { cells: Vec::new(), round }
}

struct NdLayers {
    dir: Option<PathBuf>,
    written: [Option<(bool, u64)>; 2],
    count: u32,
    checked_at: f64,
}

impl NdLayers {
    fn dump(&mut self, time: f64) {
        let Some(dir) = &self.dir else { return };
        if time - self.checked_at < 0.25 {
            return;
        }
        self.checked_at = time;
        let mut changed = false;
        for (i, side) in [Side::Left, Side::Right].into_iter().enumerate() {
            let wxr = crate::wxr::layer(side);
            let is_wxr = wxr.is_some();
            let Some((w, h, generation, rgba)) = wxr.or_else(|| crate::mapdata::plugin::terrain_layer(side)) else { continue };
            if self.written[i] == Some((is_wxr, generation)) {
                continue;
            }
            self.written[i] = Some((is_wxr, generation));
            self.count += 1;
            let name = format!("nd-{}-{:05}-{}.png", ["L", "R"][i], self.count, if is_wxr { "wxr" } else { "terrain" });
            write_rgba(&dir.join(&name), w, h, &rgba);
            std::fs::write(dir.join(format!("latest-{}.txt", ["L", "R"][i])), &name).unwrap();
            changed = true;
        }
        if changed && self.count % 50 == 0 {
            eprintln!("harness: {} ND layer pictures written", self.count);
        }
    }
}

struct Harness {
    vars: crate::Vars,
    host: XphfbwHost,
    screens: Vec<(&'static str, ScreenBlock)>,
    remote: RemoteSystems,
    pinned: Vec<(VariableIdentifier, f64, bool)>,
    pin_file: Option<PathBuf>,
    pin_stamp: Option<std::time::SystemTime>,
    pin_checked: f64,
    runtime_pins: Vec<(VariableIdentifier, f64)>,
    last: Instant,
    time: f64,
    xplm: &'static Xplm,
    mapdata: crate::mapdata::MapData,
    storms: Storms,
    layers: NdLayers,
    #[cfg(feature = "native-displays")]
    native: Option<NativeSide>,
}

#[cfg(feature = "native-displays")]
struct NativeSide {
    route: crate::native_displays::screen::Native,
    paint_off: bool,
    mask: u64,
    trigger: Option<PathBuf>,
}

#[cfg(feature = "native-displays")]
impl NativeSide {
    fn start() -> Option<Self> {
        use crate::native_displays::{inputs, screen};
        let selection = screen::selection();
        if !selection.unknown.is_empty() {
            eprintln!("harness: {}: unknown names {:?} ignored", screen::ENV, selection.unknown);
        }
        let route = screen::Native::start(selection, inputs::live_feed())?;
        eprintln!("harness: native route: {} screens from {}", selection.targets.len(), screen::ENV);
        Some(Self {
            route,
            paint_off: std::env::var("E2E_PAINT_OFF").is_ok_and(|v| v != "0"),
            mask: 0,
            trigger: std::env::var_os("E2E_SHOT_TRIGGER").map(PathBuf::from),
        })
    }

    fn tick(&mut self, host: &XphfbwHost) {
        let mask = if self.paint_off {
            crate::native_displays::screen::selection()
                .targets
                .iter()
                .filter(|t| self.route.slot_of(t.screen).is_some_and(|slot| self.route.seq(slot) != 0))
                .fold(0, |mask, t| mask | crate::xphfbw_bridge_views::paint_off_bit(t.screen))
        } else {
            0
        };
        if mask != self.mask {
            self.mask = mask;
            host.set_native_paint_off(mask);
            eprintln!("harness: native displays: Chromium paint off for {} screens (mask {mask:#x})", mask.count_ones());
        }
    }

    fn capture(&self, screens: &[(&'static str, ScreenBlock)], dir: &std::path::Path) {
        std::fs::create_dir_all(dir).unwrap();
        let mut done = 0;
        for (id, block) in screens {
            let Some(slot) = self.route.slot_of(id) else { continue };
            let Some(pending) = self.route.pending(slot, 0) else { continue };
            let (w, h) = pending.size();
            let mut rgba = pending.bgra().to_vec();
            drop(pending);
            for px in rgba.chunks_exact_mut(4) {
                px.swap(0, 2);
                px[3] = 255;
            }
            write_rgba(&dir.join(format!("{id}.native.png")), w, h, &rgba);
            if block.header().frame.load(Ordering::Relaxed) > 0 {
                write_png(&dir.join(format!("{id}.chromium.png")), block);
            }
            done += 1;
        }
        eprintln!("harness: captured {done} native screens into {}", dir.display());
    }

    fn poll_trigger(&self, screens: &[(&'static str, ScreenBlock)], out: &std::path::Path) {
        let Some(trigger) = &self.trigger else { return };
        let Ok(label) = std::fs::read_to_string(trigger) else { return };
        let _ = std::fs::remove_file(trigger);
        let label = label.trim();
        self.capture(screens, &out.join(if label.is_empty() { "shot" } else { label }));
    }
}

impl Harness {
    fn start() -> Harness {
        let exe = env_path("XPHFBW_EXE");
        let aircraft = env_path("XPHFBW_AIRCRAFT");
        let xp_root = env_path("XPHFBW_XP_ROOT");
        let xplm: &'static Xplm = Box::leak(Box::new(Xplm::dummy()));
        let mut vars = crate::Vars::new(xplm);
        let panel_cfg = std::fs::read_to_string(aircraft.join("panel").join("panel.cfg")).expect("panel.cfg");
        crate::xp::set_system_path(xp_root.clone());
        match crate::panel::start(8390) {
            Ok(port) => eprintln!("harness: plugin web server on port {port} (AMDB, SimBridge terrain, weather)"),
            Err(e) => eprintln!("harness: plugin web server did not start: {e}"),
        }

        // The facility database, as JsHost::find starts it on the plugin's main
        // thread (the providers are that thread's).
        match crate::navdata::NavData::load(&xp_root) {
            Ok(nav) => crate::js_bridge::add_provider(Box::new(nav)),
            Err(e) => eprintln!("harness: navdata: {e}"),
        }

        let host: RefCell<Option<XphfbwHost>> = RefCell::new(None);
        let screens: RefCell<Vec<(&'static str, ScreenBlock)>> = RefCell::new(Vec::new());
        let remote = RemoteSystems::connect(StartState::Taxi, &mut vars, |tag| {
            eprintln!("harness: session {tag}");
            *host.borrow_mut() = Some(XphfbwHost::start(tag, xplm, &panel_cfg, None).ok_or("the bridge host did not start")?);
            for def in SCREENS {
                let (dw, dh) = crate::xphfbw_bridge_views::device_size(def.id, def.width, def.height);
                match ScreenBlock::create(tag, def.id, dw, dh) {
                    Some(b) => screens.borrow_mut().push((def.id, b)),
                    None => eprintln!("harness: could not create screen {}", def.id),
                }
            }
            std::process::Command::new(&exe)
                .arg(tag)
                .arg(std::process::id().to_string())
                .arg(format!("--xp-root={}", xp_root.display()))
                .arg(format!("--aircraft={}", aircraft.display()))
                .spawn()
                .map(Some)
                .map_err(|e| format!("cannot start {}: {e}", exe.display()))
        })
        .expect("XPHFBW's systems did not come up");
        let host = host.into_inner().unwrap();
        let screens = screens.into_inner();
        eprintln!("harness: systems up, {} views, {} screens", host.view_count(), screens.len());

        // Nothing X-Plane would feed is here, so power the aircraft the way a
        // pilot on the stand would: batteries on auto, ground power in and on.
        let layers_dir = std::env::var("XPHFBW_ND_LAYERS").ok().map(PathBuf::from);
        if let Some(dir) = &layers_dir {
            std::fs::create_dir_all(dir).unwrap();
        }
        let mut h = Harness {
            vars,
            host,
            screens,
            remote,
            pinned: Vec::new(),
            pin_file: std::env::var("XPHFBW_PIN_FILE").ok().map(PathBuf::from),
            pin_stamp: None,
            pin_checked: 0.,
            runtime_pins: Vec::new(),
            last: Instant::now(),
            time: 0.,
            xplm,
            mapdata: crate::mapdata::MapData::new(xplm),
            storms: Storms { cells: Vec::new(), round: Vec::new() },
            layers: NdLayers { dir: layers_dir, written: [None, None], count: 0, checked_at: 0. },
            #[cfg(feature = "native-displays")]
            native: NativeSide::start(),
        };
        h.set("TOTAL WEIGHT", 400_000.);
        h.set("AMBIENT PRESSURE", 29.92);
        h.set("AMBIENT TEMPERATURE", 15.);
        h.set("SEA LEVEL PRESSURE", 1013.25);
        for bat in ["BAT_1", "BAT_2", "BAT_ESS", "BAT_APU"] {
            h.set(&format!("A32NX_OVHD_ELEC_{bat}_PB_IS_AUTO"), 1.);
        }
        // Display brightness knobs up (stored as a 0-1 ratio, bind.rs).
        for n in 80..=100 {
            h.set(&format!("LIGHT POTENTIOMETER:{n}"), 1.);
        }
        for n in 1..=4 {
            h.set(&format!("A32NX_EXT_PWR_AVAIL:{n}"), 1.);
            h.set(&format!("A32NX_OVHD_ELEC_EXT_PWR_{n}_PB_IS_ON"), 1.);
        }
        for pair in std::env::var("XPHFBW_SET").unwrap_or_default().split(',').filter(|p| !p.trim().is_empty()) {
            if let Some((name, value)) = pair.split_once('=') {
                h.set(name.trim(), value.trim().parse().unwrap_or(0.));
            }
        }
        h.pinned = seed_from_snapshot(&mut h.vars);
        h.storms = storms_around(h.get("PLANE LATITUDE"), h.get("PLANE LONGITUDE"));
        eprintln!("harness: {} synthetic rain cells", h.storms.cells.len() + h.storms.round.len());
        h.last = Instant::now();
        h
    }

    fn set(&mut self, name: &str, value: f64) {
        let id = self.vars.get_unprefixed(name.to_owned());
        self.vars.write(&id, value);
    }

    fn get(&mut self, name: &str) -> f64 {
        let id = self.vars.get_unprefixed(name.to_owned());
        self.vars.read(&id)
    }

    fn tick_displays_data(&mut self) {
        self.mapdata.update(&mut self.vars, self.xplm);
        let aircraft = Aircraft {
            lat: self.get("PLANE LATITUDE"),
            lon: self.get("PLANE LONGITUDE"),
            alt_m: self.get("PLANE ALTITUDE") * 0.3048,
            heading_true_deg: self.get("PLANE HEADING DEGREES TRUE"),
        };
        crate::wxr::tick_with(&self.vars, self.xplm, self.time, aircraft, &mut self.storms);
        self.layers.dump(self.time);
    }

    fn reload_pins(&mut self) {
        let Some(path) = self.pin_file.clone() else { return };
        if self.time - self.pin_checked < 0.25 {
            return;
        }
        self.pin_checked = self.time;
        let stamp = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if stamp == self.pin_stamp {
            return;
        }
        self.pin_stamp = stamp;
        let json: serde_json::Value = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        self.runtime_pins.clear();
        if let Some(map) = json.get("vars").and_then(|v| v.as_object()) {
            for (name, value) in map {
                if let Some(value) = value.as_f64() {
                    let id = self.vars.get(name.clone());
                    self.runtime_pins.push((id, value));
                }
            }
        }
        eprintln!("harness: {} runtime pins from {}", self.runtime_pins.len(), path.display());
    }

    fn apply_runtime_pins(&mut self) {
        for (id, value) in &self.runtime_pins {
            self.vars.write(id, *value);
        }
    }

    fn run_for(&mut self, seconds: f64, mut report: impl FnMut(&Harness)) {
        let until = self.time + seconds;
        while self.time < until {
            let now = Instant::now();
            let delta = now - self.last;
            self.last = now;
            self.time += delta.as_secs_f64();
            let jitter = if (self.time * 30.0) as i64 % 2 == 0 { 0.0 } else { 1e-4 };
            for (id, value, heading) in &self.pinned {
                self.vars.write(id, if *heading { *value + jitter } else { *value });
            }
            self.reload_pins();
            self.apply_runtime_pins();
            self.host.pre_tick(&mut self.vars);
            if let Some(message) = self.remote.tick(delta, self.time, &mut self.vars) {
                eprintln!("harness: systems: {message}");
            }
            self.apply_runtime_pins();
            self.host.post_tick(&mut self.vars, self.time, &[], &[]);
            self.tick_displays_data();
            #[cfg(feature = "native-displays")]
            if let Some(native) = self.native.as_mut() {
                native.tick(&self.host);
                if let Ok(out) = std::env::var("XPHFBW_OUT") {
                    native.poll_trigger(&self.screens, &PathBuf::from(out).join("native-shots"));
                }
            }
            report(self);
            if std::env::var("XPHFBW_STOP").is_ok_and(|stop| std::path::Path::new(&stop).exists()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
    }

    fn frames(&self) -> String {
        self.screens.iter().map(|(id, b)| format!("{}={}", id.trim_start_matches("SCREEN_"), b.header().frame.load(Ordering::Relaxed))).collect::<Vec<_>>().join(" ")
    }

    fn shoot(&self, id: &str, path: &std::path::Path) -> bool {
        match self.screens.iter().find(|(s, _)| *s == id) {
            Some((_, b)) if b.header().frame.load(Ordering::Relaxed) > 0 => {
                write_png(path, b);
                true
            }
            _ => false,
        }
    }
}

#[test]
#[ignore]
fn xphfbw_harness() {
    let out = env_path("XPHFBW_OUT");
    let seconds: f64 = std::env::var("XPHFBW_SECONDS").ok().and_then(|s| s.parse().ok()).unwrap_or(60.);
    std::fs::create_dir_all(&out).unwrap();
    let mut h = Harness::start();
    let mut report_at = 0.;
    h.run_for(seconds, |h| {
        if h.time >= report_at {
            report_at = h.time + 5.;
            eprintln!(
                "harness: t={:.0}s loaded {}/{} displays_active={} gone={} frames {}",
                h.time,
                h.host.views_loaded(),
                h.host.view_count(),
                h.host.displays_active(),
                h.host.gone(),
                h.frames()
            );
        }
    });
    for (id, _) in &h.screens {
        h.shoot(id, &out.join(format!("{id}.png")));
    }
    eprintln!("harness: pictures in {}", out.display());
}

#[test]
#[ignore]
fn xphfbw_alert_shots() {
    let out = env_path("XPHFBW_OUT");
    std::fs::create_dir_all(&out).unwrap();
    let list = std::fs::read_to_string(env_path("XPHFBW_ALERTS")).expect("alert list");
    let ids: Vec<u64> = list.lines().filter_map(|l| l.trim().parse().ok()).collect();
    let warm: f64 = std::env::var("XPHFBW_SECONDS").ok().and_then(|s| s.parse().ok()).unwrap_or(90.);
    let settle: f64 = std::env::var("XPHFBW_SETTLE").ok().and_then(|s| s.parse().ok()).unwrap_or(3.);
    let mut h = Harness::start();
    h.run_for(warm, |_| {});
    eprintln!("harness: warm, frames {}", h.frames());
    let started = Instant::now();
    for (k, id) in ids.iter().enumerate() {
        if out.join(format!("{id}_EWD.png")).exists() {
            continue;
        }
        h.set("A32NX_FWS_FORCE_HI", (id / 10_000) as f64);
        h.set("A32NX_FWS_FORCE_LO", (id % 10_000) as f64);
        h.run_for(settle, |_| {});
        h.shoot("SCREEN_DU_EWD", &out.join(format!("{id}_EWD.png")));
        h.shoot("SCREEN_DU_SD", &out.join(format!("{id}_SD.png")));
        h.set("A32NX_FWS_FORCE_HI", 0.);
        h.set("A32NX_FWS_FORCE_LO", 0.);
        h.run_for(1.0, |_| {});
        if k % 25 == 0 {
            eprintln!("harness: {}/{} alerts, {:.0} s", k + 1, ids.len(), started.elapsed().as_secs_f64());
        }
    }
    eprintln!("harness: alert pictures in {}", out.display());
}

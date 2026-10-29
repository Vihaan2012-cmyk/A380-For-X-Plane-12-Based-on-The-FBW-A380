//! The head-up displays (FCOM DSC-31-60): one per fitted seat (`xphfbw.hud`). Each has its
//! combiner (`fbw/hud/<l|r>/deploy`, 1 deployed and locked), its control panel (brightness knob
//! `fbw/hud/<l|r>/brightness`, `fbw/hud/<l|r>/declutter`, `fbw/hud/<l|r>/xwind`) and its page,
//! `A380X/HUD/hud.html` drawn natively as SCREEN_HUD_L/R. The page is a gnomonic image of the sky
//! ahead; each frame it is laid on the plane one unit ahead of the boresight at infinity,
//! projected from the viewer's own eye, and clipped to the combiner glass as seen from there.

use std::ffi::{c_int, c_void};
use std::sync::{Mutex, OnceLock};

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::published::{self, Command, Published, Value};
use crate::xp::{self, DataRef, Xplm};
use crate::Vars;

pub const PAGE_W: f64 = 1280.;
pub const PAGE_H: f64 = 1024.;
pub const PAGE_CX: f64 = 640.;
pub const PAGE_CY: f64 = 512.;
/// FlyByWire's HUD page (HUDUtils ONE_DEG): 1024 px over 28 deg, the same both ways.
pub const PX_PER_DEG: f64 = 1024. / 28.;
const GRID_X: usize = 24;
const GRID_Y: usize = 16;
/// The combiner glass relative to the design eye (aircraft axes: x right, y up, z aft), metres.
const COMBINER_CENTRE: [f64; 3] = [0., -0.025, -0.37];
const COMBINER_HALF: (f64, f64) = (0.130, 0.075);
const DEPLOY_SECONDS: f64 = 0.9;
const SCREENS: [&str; 2] = ["SCREEN_HUD_L", "SCREEN_HUD_R"];
const SIDE: [&str; 2] = ["l", "r"];
const SIDE_UPPER: [&str; 2] = ["L", "R"];
const POTENTIOMETER: [u32; 2] = [71, 72];
const POWER: &str = "ELEC_LOAD_hud_POWERED";
const COMPARED: [&str; 6] = ["DEEP_IR_CAPT_FO_PITCH_DIFF_DEG", "DEEP_IR_CAPT_FO_ROLL_DIFF_DEG", "DEEP_IR_CAPT_FO_HDG_DIFF_DEG", "DEEP_IR_CAPT_FO_FPA_DIFF_DEG", "DEEP_ADR_CAPT_FO_ALT_DIFF_FT", "DEEP_BARO_REF_DISAGREE"];
const COLOUR: [f32; 3] = [0.30, 1.0, 0.36];

/// The PFD comparison flags the HUD repeats: CHECK ATT past 5 deg pitch or roll, CHECK HDG past 5 deg,
/// CHECK FPV past 1 deg between the two selected IRS, and the altitude split (-1 when the two baro
/// references differ, as CHECK ALT then never shows).
fn comparisons(c: [f64; 6]) -> [f64; 4] {
    let flag = |b: bool| b as i32 as f64;
    [flag(c[0] > 5. || c[1] > 5.), flag(c[2] > 5.), flag(c[3] > 1.), if c[5] > 0.5 { -1. } else { c[4] }]
}

/// A page pixel's direction in aircraft axes: the page's angles are linear in pixels, centred on the
/// boresight.
pub fn page_direction(px: f64, py: f64) -> [f64; 3] {
    let az = ((px - PAGE_CX) / PX_PER_DEG).to_radians();
    let el = ((PAGE_CY - py) / PX_PER_DEG).to_radians();
    [az.sin() * el.cos(), el.sin(), -az.cos() * el.cos()]
}

/// Aircraft axes to X-Plane's local OpenGL axes for true heading `psi`, pitch `theta`, roll `phi` (degrees).
pub fn body_to_world(psi: f64, theta: f64, phi: f64, v: [f64; 3]) -> [f64; 3] {
    let (sr, cr) = (-phi).to_radians().sin_cos();
    let v = [v[0] * cr - v[1] * sr, v[0] * sr + v[1] * cr, v[2]];
    let (sp, cp) = theta.to_radians().sin_cos();
    let v = [v[0], v[1] * cp - v[2] * sp, v[1] * sp + v[2] * cp];
    let (sh, ch) = (-psi).to_radians().sin_cos();
    [v[0] * ch + v[2] * sh, v[1], -v[0] * sh + v[2] * ch]
}

pub fn world_to_body(psi: f64, theta: f64, phi: f64, v: [f64; 3]) -> [f64; 3] {
    let (sh, ch) = psi.to_radians().sin_cos();
    let v = [v[0] * ch + v[2] * sh, v[1], -v[0] * sh + v[2] * ch];
    let (sp, cp) = (-theta).to_radians().sin_cos();
    let v = [v[0], v[1] * cp - v[2] * sp, v[1] * sp + v[2] * cp];
    let (sr, cr) = phi.to_radians().sin_cos();
    [v[0] * cr - v[1] * sr, v[0] * sr + v[1] * cr, v[2]]
}

/// The combiner's outline in aircraft axes around `eye`: a rounded rectangle (superellipse, n = 4).
pub fn combiner_outline(eye: [f64; 3], points: usize) -> Vec<[f64; 3]> {
    (0..points)
        .map(|i| {
            let t = i as f64 / points as f64 * std::f64::consts::TAU;
            let (s, c) = t.sin_cos();
            let x = COMBINER_HALF.0 * c.signum() * c.abs().sqrt();
            let y = COMBINER_HALF.1 * s.signum() * s.abs().sqrt();
            [eye[0] + COMBINER_CENTRE[0] + x, eye[1] + COMBINER_CENTRE[1] + y, eye[2] + COMBINER_CENTRE[2]]
        })
        .collect()
}

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

/// How much of the image reaches an eye `offset` from the design eye: whole inside the eyebox,
/// gone a few centimetres beyond it.
pub fn eyebox_fade(offset: [f64; 3]) -> f64 {
    (1. - smoothstep(0.07, 0.12, offset[0].abs())) * (1. - smoothstep(0.05, 0.09, offset[1].abs())) * (1. - smoothstep(0.12, 0.22, offset[2].abs()))
}

fn mul(m: &[f32; 16], v: [f64; 4]) -> [f64; 4] {
    let mut out = [0.; 4];
    for (r, o) in out.iter_mut().enumerate() {
        *o = (0..4).map(|c| m[c * 4 + r] as f64 * v[c]).sum();
    }
    out
}

/// Window pixels (relative to the viewport) of a homogeneous world point, `None` behind the eye.
fn to_window(proj: &[f32; 16], world: &[f32; 16], p: [f64; 4], size: (f64, f64)) -> Option<[f64; 2]> {
    let clip = mul(proj, mul(world, p));
    if clip[3] <= 1e-6 {
        return None;
    }
    Some([(clip[0] / clip[3] + 1.) * 0.5 * size.0, (clip[1] / clip[3] + 1.) * 0.5 * size.1])
}

/// `poly` clipped by the convex `edge` loop (counter-clockwise or clockwise), Sutherland-Hodgman.
/// Each point is `[x, y, u, v]`.
pub fn clip_convex(poly: Vec<[f64; 4]>, edge: &[[f64; 2]]) -> Vec<[f64; 4]> {
    if edge.len() < 3 {
        return poly;
    }
    let area: f64 = (0..edge.len()).map(|i| {
        let (a, b) = (edge[i], edge[(i + 1) % edge.len()]);
        a[0] * b[1] - b[0] * a[1]
    }).sum();
    let sign = area.signum();
    let mut out = poly;
    for i in 0..edge.len() {
        if out.is_empty() {
            break;
        }
        let (a, b) = (edge[i], edge[(i + 1) % edge.len()]);
        let side = |p: &[f64; 4]| sign * ((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]));
        let input = std::mem::take(&mut out);
        for j in 0..input.len() {
            let (p, q) = (input[j], input[(j + 1) % input.len()]);
            let (sp, sq) = (side(&p), side(&q));
            if sp >= 0. {
                out.push(p);
            }
            if (sp >= 0.) != (sq >= 0.) {
                let t = sp / (sp - sq);
                out.push([p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t, p[2] + (q[2] - p[2]) * t, p[3] + (q[3] - p[3]) * t]);
            }
        }
    }
    out
}

/// Where the tuned ILS's runway is: the threshold from the glideslope antenna (abeam it, back
/// along the course by the 50 ft crossing height over the beam slope), its elevation, the
/// localizer's true course, and the length to the localizer less its usual 300 m beyond the end.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IlsRunway {
    pub lat: f64,
    pub lon: f64,
    pub elevation_ft: f64,
    pub true_course: f64,
    pub length_m: f64,
    pub width_m: f64,
    pub slope_deg: f64,
}

pub fn ils_runway(loc: &xp::NavAid, gs: &xp::NavAid) -> Option<IlsRunway> {
    let slope = (gs.heading / 100000.).floor() / 100.;
    let slope = if (2.0..=6.0).contains(&slope) { slope } else { 3.0 };
    let course = loc.heading.rem_euclid(360.);
    let m_lat = 111_320.;
    let m_lon = 111_320. * loc.lat.to_radians().cos();
    let (ue, un) = (course.to_radians().sin(), course.to_radians().cos());
    let (de, dn) = ((gs.lon - loc.lon) * m_lon, (gs.lat - loc.lat) * m_lat);
    let along = de * ue + dn * un;
    let back = 15.24 / slope.to_radians().tan();
    let thr_along = along - back;
    if thr_along > -800. {
        return None;
    }
    let (te, tn) = (ue * thr_along, un * thr_along);
    Some(IlsRunway {
        lat: loc.lat + tn / m_lat,
        lon: loc.lon + te / m_lon,
        elevation_ft: gs.height_m / 0.3048,
        true_course: course,
        length_m: (-thr_along - 300.).clamp(1800., 4500.),
        width_m: 45.,
        slope_deg: slope,
    })
}

struct Refs {
    lat: Option<DataRef>,
    lon: Option<DataRef>,
}

struct Ids {
    power: VariableIdentifier,
    compared: [VariableIdentifier; 6],
    checks: [VariableIdentifier; 4],
    declutter: [VariableIdentifier; 2],
    xwind: [VariableIdentifier; 2],
    stowed: [VariableIdentifier; 2],
    potentiometer: [VariableIdentifier; 2],
    unlocked: [VariableIdentifier; 2],
    ls_frequency: VariableIdentifier,
    rwy_valid: VariableIdentifier,
    rwy: [VariableIdentifier; 7],
}

pub struct Hud {
    _published: Published,
    installed: [Value; 2],
    deploy: [Value; 2],
    brightness: [Value; 2],
    xwind: [Value; 2],
    toggle: [Command; 2],
    declutter: [Command; 2],
    declutter_push: [Value; 2],
    xwind_toggle: [Command; 2],
    pushed: [bool; 2],
    position: [f64; 2],
    target: [f64; 2],
    last_drag: [f64; 2],
    presses: [f64; 2],
    knob: [f64; 2],
    switch: [bool; 2],
    time: f64,
    next_lookup: f64,
    runway: Option<IlsRunway>,
    ids: Ids,
    refs: Refs,
}

#[derive(Clone, Copy, Default)]
struct Shown {
    on: bool,
    intensity: f64,
}

static SHOWN: Mutex<[Shown; 2]> = Mutex::new([Shown { on: false, intensity: 0. }; 2]);
static XPLM: OnceLock<&'static Xplm> = OnceLock::new();

impl Hud {
    pub fn new(vars: &mut Vars, xplm: &'static Xplm) -> Self {
        let _ = XPLM.set(xplm);
        let mut p = Published::default();
        let installed = std::array::from_fn(|i| p.number(&format!("fbw/hud/{}/installed", SIDE[i]), 0., false));
        let deploy = std::array::from_fn(|i| p.number(&format!("fbw/hud/{}/deploy", SIDE[i]), 0., true));
        let brightness = std::array::from_fn(|i| p.number(&format!("fbw/hud/{}/brightness", SIDE[i]), 0.8, true));
        let xwind = std::array::from_fn(|i| p.number(&format!("fbw/hud/{}/xwind", SIDE[i]), 0., true));
        let toggle = std::array::from_fn(|i| p.command(&format!("fbw/hud/{}/combiner_toggle", SIDE[i]), "Deploy or stow the HUD combiner"));
        let declutter = std::array::from_fn(|i| p.command(&format!("fbw/hud/{}/declutter", SIDE[i]), "HUD DECLUTTER pushbutton"));
        let declutter_push = std::array::from_fn(|i| p.number(&format!("fbw/hud/{}/declutter_push", SIDE[i]), 0., true));
        let xwind_toggle = std::array::from_fn(|i| p.command(&format!("fbw/hud/{}/xwind_toggle", SIDE[i]), "HUD NORM/X WIND switch"));
        let get = |vars: &mut Vars, n: String| vars.get(n);
        let ids = Ids {
            power: get(vars, POWER.to_string()),
            compared: COMPARED.map(|n| get(vars, n.to_string())),
            checks: ["CHECK_ATT", "CHECK_HDG", "CHECK_FPV", "ALT_DIFF_FT"].map(|n| get(vars, format!("A380X_HUD_{n}"))),
            declutter: std::array::from_fn(|i| get(vars, format!("A380X_HUD_{}_DECLUTTER_MODE", SIDE_UPPER[i]))),
            xwind: std::array::from_fn(|i| get(vars, format!("A380X_HUD_{}_CROSSWIND_MODE", SIDE_UPPER[i]))),
            stowed: std::array::from_fn(|i| get(vars, format!("A320_Neo_HUD_{}_POS", SIDE_UPPER[i]))),
            potentiometer: std::array::from_fn(|i| get(vars, format!("LIGHT POTENTIOMETER:{}", POTENTIOMETER[i]))),
            unlocked: std::array::from_fn(|i| get(vars, format!("A380X_HUD_{}_UNLOCKED", SIDE_UPPER[i]))),
            ls_frequency: get(vars, "NAV FREQUENCY:3".to_string()),
            rwy_valid: get(vars, "A380X_HUD_RWY_VALID".to_string()),
            rwy: ["THR_LAT", "THR_LON", "THR_ELEV", "TRUE_COURSE", "LENGTH", "WIDTH", "GS_ANGLE"].map(|n| get(vars, format!("A380X_HUD_RWY_{n}"))),
        };
        Self {
            _published: p,
            installed,
            deploy,
            brightness,
            xwind,
            toggle,
            declutter,
            declutter_push,
            xwind_toggle,
            pushed: [false; 2],
            position: [0.; 2],
            target: [0.; 2],
            last_drag: [-1.; 2],
            presses: [0.; 2],
            knob: [0.8; 2],
            switch: [false; 2],
            time: 0.,
            next_lookup: 0.,
            runway: None,
            ids,
            refs: Refs { lat: xplm.find("sim/flightmodel/position/latitude"), lon: xplm.find("sim/flightmodel/position/longitude") },
        }
    }

    pub fn update(&mut self, vars: &mut Vars, xplm: &Xplm, delta: f64) {
        self.time += delta.max(0.);
        let seats = crate::app_settings::current().hud_seats;
        let mut shown = [Shown::default(); 2];
        for i in 0..2 {
            let fitted = (i == 0 && seats >= 1) || (i == 1 && seats >= 2);
            published::set(self.installed[i], fitted as i32 as f64);
            if let Some(v) = published::take(self.deploy[i]) {
                self.position[i] = v.clamp(0., 1.);
                self.target[i] = self.position[i];
                self.last_drag[i] = self.time;
            }
            for _ in 0..published::presses(self.toggle[i]) {
                self.target[i] = if self.target[i] > 0.5 { 0. } else { 1. };
                self.last_drag[i] = -1.;
            }
            if self.last_drag[i] >= 0. && self.time - self.last_drag[i] > 0.3 {
                self.target[i] = if self.position[i] > 0.85 { 1. } else if self.position[i] < 0.15 { 0. } else { self.position[i] };
                self.last_drag[i] = -1.;
            }
            if !fitted {
                self.target[i] = 0.;
                self.position[i] = 0.;
            }
            let step = delta.max(0.) / DEPLOY_SECONDS;
            self.position[i] += (self.target[i] - self.position[i]).clamp(-step, step);
            published::set(self.deploy[i], self.position[i]);
            if let Some(v) = published::take(self.brightness[i]) {
                self.knob[i] = v.clamp(0., 1.);
            }
            published::set(self.brightness[i], self.knob[i]);
            if let Some(v) = published::take(self.xwind[i]) {
                self.switch[i] = v > 0.5;
            }
            for _ in 0..published::presses(self.xwind_toggle[i]) {
                self.switch[i] = !self.switch[i];
            }
            published::set(self.xwind[i], self.switch[i] as i32 as f64);
            let mut presses = published::presses(self.declutter[i]);
            if let Some(v) = published::take(self.declutter_push[i]) {
                let down = v > 0.5;
                if down && !self.pushed[i] {
                    presses += 1;
                }
                self.pushed[i] = down;
            }
            published::set(self.declutter_push[i], self.pushed[i] as i32 as f64);
            if presses > 0 {
                let mode = vars.read(&self.ids.declutter[i]).round().clamp(0., 2.) as u32;
                vars.write(&self.ids.declutter[i], ((mode + presses) % 3) as f64);
            }
            vars.write(&self.ids.xwind[i], self.switch[i] as i32 as f64);
            vars.write(&self.ids.stowed[i], if self.position[i] >= 0.97 { 0. } else { 1. });
            vars.write(&self.ids.potentiometer[i], self.knob[i]);
            let unlocked = self.position[i] > 0.6 && self.position[i] < 0.995;
            vars.write(&self.ids.unlocked[i], unlocked as i32 as f64);
            let powered = vars.read(&self.ids.power) > 0.5;
            let on = fitted && powered && self.position[i] >= 0.97 && self.knob[i] > 0.02;
            shown[i] = Shown { on, intensity: self.knob[i].powf(1.4) };
            if fitted && self.position[i] > 0.5 && diag_due(i) {
                crate::log(&format!(
                    "hud {}: deploy {:.2} powered {} brightness {:.2} -> {}",
                    SIDE[i],
                    self.position[i],
                    powered,
                    self.knob[i],
                    if on { "drawing" } else { "blank" }
                ));
            }
        }
        if let Ok(mut s) = SHOWN.lock() {
            *s = shown;
        }
        let c = self.ids.compared.map(|id| vars.read(&id));
        let checks = comparisons(c);
        for (id, v) in self.ids.checks.iter().zip(checks) {
            vars.write(id, v);
        }
        if self.time >= self.next_lookup {
            self.next_lookup = self.time + 2.;
            self.runway = self.find_runway(vars, xplm);
        }
        vars.write(&self.ids.rwy_valid, self.runway.is_some() as i32 as f64);
        if let Some(r) = self.runway {
            let values = [r.lat, r.lon, r.elevation_ft, r.true_course, r.length_m, r.width_m, r.slope_deg];
            for (id, v) in self.ids.rwy.iter().zip(values) {
                vars.write(id, v);
            }
        }
    }

    fn find_runway(&self, vars: &mut Vars, xplm: &Xplm) -> Option<IlsRunway> {
        let mhz = vars.read(&self.ids.ls_frequency);
        if !(108.0..=118.0).contains(&mhz) {
            return None;
        }
        let lat = xplm.get_d(self.refs.lat?);
        let lon = xplm.get_d(self.refs.lon?);
        let freq = (mhz * 100.).round() as i32;
        let loc = xp::nearest_nav_aid(lat, lon, Some(freq), xp::NAV_ILS | xp::NAV_LOCALIZER)?;
        let gs = xp::nearest_nav_aid(loc.lat, loc.lon, Some(freq), xp::NAV_GLIDESLOPE)?;
        let km = |a: (f64, f64), b: (f64, f64)| {
            let m_lon = 111.32 * a.0.to_radians().cos();
            (((a.0 - b.0) * 111.32).powi(2) + ((a.1 - b.1) * m_lon).powi(2)).sqrt()
        };
        if km((lat, lon), (loc.lat, loc.lon)) > 80. || km((loc.lat, loc.lon), (gs.lat, gs.lon)) > 6. {
            return None;
        }
        ils_runway(&loc, &gs)
    }
}

struct Frame {
    proj: [f32; 16],
    world: [f32; 16],
    size: (f64, f64),
}

fn eye_design(xplm: &Xplm, side: usize) -> Option<[f64; 3]> {
    let read = |n: &str| xplm.find(n).map(|d| xplm.get_f(d) as f64);
    let (x, y, z) = (read("sim/aircraft/view/acf_peX")?, read("sim/aircraft/view/acf_peY")?, read("sim/aircraft/view/acf_peZ")?);
    let scale = if z.abs() > 60. { 0.3048 } else { 1. };
    let x = x.abs() * scale * if side == 0 { -1. } else { 1. };
    Some([x, y * scale, z * scale])
}

fn read16(xplm: &Xplm, name: &str) -> Option<[f32; 16]> {
    let d = xplm.find(name)?;
    let mut m = [0f32; 16];
    (xplm.get_vf(d, &mut m) == 16).then_some(m)
}

pub(crate) fn diag_due(slot: usize) -> bool {
    static LAST: Mutex<[Option<std::time::Instant>; 8]> = Mutex::new([None; 8]);
    let Ok(mut last) = LAST.lock() else { return false };
    let now = std::time::Instant::now();
    match last[slot] {
        Some(t) if now.duration_since(t).as_secs_f64() < 5. => false,
        _ => {
            last[slot] = Some(now);
            true
        }
    }
}

fn draw() {
    let Some(xplm) = XPLM.get().copied() else { return };
    let shown = SHOWN.lock().map(|s| *s).unwrap_or_default();
    if !shown.iter().any(|s| s.on) {
        return;
    }
    let geti = |n: &str| xplm.find(n).map(|d| xplm.get_i(d)).unwrap_or(0);
    let getf = |n: &str| xplm.find(n).map(|d| xplm.get_f(d) as f64).unwrap_or(0.);
    if geti("sim/graphics/view/view_is_external") != 0 {
        return;
    }
    let (Some(proj), Some(world)) = (read16(xplm, "sim/graphics/view/projection_matrix_3d"), read16(xplm, "sim/graphics/view/world_matrix")) else {
        if diag_due(4) {
            crate::log("hud draw: projection_matrix_3d / world_matrix unreadable");
        }
        return;
    };
    let mut vp = [0; 4];
    match xplm.find("sim/graphics/view/viewport") {
        Some(d) if xplm.get_vi(d, &mut vp) == 4 => {}
        _ => return,
    }
    let size = ((vp[2] - vp[0]) as f64, (vp[3] - vp[1]) as f64);
    if size.0 <= 0. || size.1 <= 0. {
        return;
    }
    let frame = Frame { proj, world, size };
    let pos = [getf("sim/flightmodel/position/local_x"), getf("sim/flightmodel/position/local_y"), getf("sim/flightmodel/position/local_z")];
    let (psi, theta, phi) = (getf("sim/flightmodel/position/psi"), getf("sim/flightmodel/position/theta"), getf("sim/flightmodel/position/phi"));
    let cam = [getf("sim/graphics/view/view_x"), getf("sim/graphics/view/view_y"), getf("sim/graphics/view/view_z")];
    let eye_now = world_to_body(psi, theta, phi, [cam[0] - pos[0], cam[1] - pos[1], cam[2] - pos[2]]);
    let daylight = getf("sim/graphics/misc/outside_light_level").clamp(0., 1.);
    for (side, s) in shown.iter().enumerate() {
        if !s.on {
            continue;
        }
        let report = diag_due(2 + side);
        let Some(eye) = eye_design(xplm, side) else {
            if report {
                crate::log("hud draw: no design eye (acf_peX/Y/Z)");
            }
            continue;
        };
        let offset = [eye_now[0] - eye[0], eye_now[1] - eye[1], eye_now[2] - eye[2]];
        let fade = eyebox_fade(offset);
        let edge: Vec<[f64; 2]> = combiner_outline(eye, 40)
            .into_iter()
            .filter_map(|p| {
                let w = body_to_world(psi, theta, phi, p);
                to_window(&frame.proj, &frame.world, [pos[0] + w[0], pos[1] + w[1], pos[2] + w[2], 1.], frame.size)
            })
            .collect();
        if report {
            let (lo, hi) = edge.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| {
                ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])])
            });
            crate::log(&format!(
                "hud draw {}: eye now {:.3?} design {:.3?} offset {:.3?} fade {:.2}; outline {}/40 points in window x {:.0}..{:.0} y {:.0}..{:.0} of {:?}",
                SIDE[side], eye_now, eye, offset, fade, edge.len(), lo[0], hi[0], lo[1], hi[1], size
            ));
        }
        if fade <= 0.01 || edge.len() < 40 {
            continue;
        }
        let mut grid = vec![None; (GRID_X + 1) * (GRID_Y + 1)];
        for j in 0..=GRID_Y {
            for i in 0..=GRID_X {
                let (u, v) = (i as f64 / GRID_X as f64, j as f64 / GRID_Y as f64);
                let d = body_to_world(psi, theta, phi, page_direction(u * PAGE_W, v * PAGE_H));
                grid[j * (GRID_X + 1) + i] = to_window(&frame.proj, &frame.world, [d[0], d[1], d[2], 0.], frame.size).map(|p| [p[0], p[1], u, 1. - v]);
            }
        }
        let alpha = (fade * (0.45 + 0.55 * daylight.max(0.25))) as f32;
        let mut triangles: Vec<[f32; 5]> = Vec::new();
        for j in 0..GRID_Y {
            for i in 0..GRID_X {
                let at = |a: usize, b: usize| grid[(j + b) * (GRID_X + 1) + i + a];
                let (Some(p00), Some(p10), Some(p11), Some(p01)) = (at(0, 0), at(1, 0), at(1, 1), at(0, 1)) else { continue };
                for tri in [[p00, p10, p11], [p00, p11, p01]] {
                    let clipped = clip_convex(tri.to_vec(), &edge);
                    for k in 1..clipped.len().saturating_sub(1) {
                        for p in [clipped[0], clipped[k], clipped[k + 1]] {
                            triangles.push([p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32, alpha]);
                        }
                    }
                }
            }
        }
        if report {
            crate::log(&format!("hud draw {}: {} page triangles inside the combiner, alpha {:.2}", SIDE[side], triangles.len() / 3, alpha));
        }
        let k = s.intensity as f32;
        #[cfg(feature = "js")]
        crate::display::draw_hud(&crate::display::HudFrame {
            screen: SCREENS[side],
            triangles,
            color: [COLOUR[0] * k, COLOUR[1] * k, COLOUR[2] * k],
            glow: 0.9,
            viewport: vp,
        });
        #[cfg(not(feature = "js"))]
        let _ = (triangles, k, SCREENS);
    }
}

unsafe extern "C" fn draw_callback(_phase: c_int, _before: c_int, _refcon: *mut c_void) -> c_int {
    let _ = std::panic::catch_unwind(draw);
    1
}

pub fn start() {
    if !xp::register_draw_callback(draw_callback, xp::PHASE_WINDOW, true) {
        crate::log("hud: could not register the HUD draw callback");
    }
}

pub fn stop() {
    xp::unregister_draw_callback(draw_callback, xp::PHASE_WINDOW, true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pfd_comparisons_follow_their_thresholds() {
        assert_eq!(comparisons([4.9, 4.9, 4.9, 0.9, 240., 0.]), [0., 0., 0., 240.]);
        assert_eq!(comparisons([5.1, 0., 0., 0., 0., 0.]), [1., 0., 0., 0.]);
        assert_eq!(comparisons([0., 5.1, 5.1, 1.1, 600., 1.]), [1., 1., 1., -1.]);
    }

    #[test]
    fn the_page_centre_is_the_boresight_and_the_page_spans_35_by_28_degrees() {
        let d = page_direction(PAGE_CX, PAGE_CY);
        assert!(d[0].abs() < 1e-12 && d[1].abs() < 1e-12 && (d[2] + 1.).abs() < 1e-12);
        let right = page_direction(PAGE_W, PAGE_CY);
        assert!((right[0].atan2(-right[2]).to_degrees() - 17.5).abs() < 1e-9);
        let top = page_direction(PAGE_CX, 0.);
        assert!((top[1].asin().to_degrees() - 14.).abs() < 1e-9);
    }

    #[test]
    fn body_and_world_axes_round_trip_and_heading_turns_forward_east() {
        let v = [0.3, -0.2, -1.];
        let w = body_to_world(73., 4., -12., v);
        let back = world_to_body(73., 4., -12., w);
        for k in 0..3 {
            assert!((back[k] - v[k]).abs() < 1e-12);
        }
        let east = body_to_world(90., 0., 0., [0., 0., -1.]);
        assert!((east[0] - 1.).abs() < 1e-12 && east[2].abs() < 1e-12);
        let up = body_to_world(0., 10., 0., [0., 0., -1.]);
        assert!(up[1] > 0.17);
        let right_wing = body_to_world(0., 0., 20., [1., 0., 0.]);
        assert!(right_wing[1] < -0.3, "right wing down in a right bank");
    }

    #[test]
    fn a_triangle_is_cut_to_the_combiner() {
        let square = [[0., 0.], [10., 0.], [10., 10.], [0., 10.]];
        let tri = vec![[-5., -5., 0., 0.], [20., -5., 1., 0.], [-5., 20., 0., 1.]];
        let out = clip_convex(tri, &square);
        assert!(out.len() >= 3);
        assert!(out.iter().all(|p| p[0] >= -1e-9 && p[0] <= 10. + 1e-9 && p[1] >= -1e-9 && p[1] <= 10. + 1e-9));
    }

    #[test]
    fn the_eyebox_keeps_the_image_near_the_design_eye_and_loses_it_beyond() {
        assert_eq!(eyebox_fade([0., 0., 0.]), 1.);
        assert_eq!(eyebox_fade([0.06, 0.04, 0.1]), 1.);
        assert_eq!(eyebox_fade([0.2, 0., 0.]), 0.);
    }

    #[test]
    fn an_ils_runway_starts_before_the_glideslope_antenna_by_the_crossing_height() {
        let loc = xp::NavAid { lat: 51.4775, lon: -0.4850, heading: 270., ..Default::default() };
        let gs = xp::NavAid { lat: 51.4780, lon: -0.4300, heading: 300_270., height_m: 24., ..Default::default() };
        let r = ils_runway(&loc, &gs).unwrap();
        assert!((r.slope_deg - 3.).abs() < 1e-9);
        assert!(r.lon > gs.lon, "the threshold is east of a westbound runway's glideslope");
        assert!((r.lat - loc.lat).abs() < 1e-6, "on the localizer course line");
        assert!((r.elevation_ft - 24. / 0.3048).abs() < 1e-9);
        assert!(r.length_m > 3000.);
    }
}

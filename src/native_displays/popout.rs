//! Pop-out windows for screens and panels (docs/superpowers/plans/2026-10-22-popouts-vr.md):
//! placement on monitors, remembered positions and named layouts, the
//! X-Plane calls that move an avionics device between cockpit, float,
//! OS window and VR, window-to-content click mapping, and the 2D panel
//! description with its hit-testing. No XPLM calls here; the hooks in the
//! plan run the returned [`Call`]s and [`Action`]s.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const PREFERENCES: &str = "Output/preferences/fbw_a380x_popouts.json";

/// Operating-system pixels, y down: `top < bottom`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OsRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl OsRect {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self { left, top, right, bottom }
    }

    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    fn centre(&self) -> (i32, i32) {
        ((self.left + self.right) / 2, (self.top + self.bottom) / 2)
    }

    fn tuple(&self) -> (i32, i32, i32, i32) {
        (self.left, self.top, self.right, self.bottom)
    }
}

/// X-Plane global boxels, y up: `top > bottom` (XPLMGetWindowGeometry).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XpRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Monitor {
    pub index: i32,
    pub rect: OsRect,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Cockpit,
    Float,
    PopOut,
    Vr,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub mode: Mode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<OsRect>,
}

/// One X-Plane avionics call, in the order they must run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    PopupVisible(bool),
    PopOut,
    MapVr(bool),
    GeometryOs(i32, i32, i32, i32),
}

/// The largest `content`-shaped rectangle centred in `avail`.
pub fn fit(content: (u32, u32), avail: OsRect) -> OsRect {
    let (cw, ch) = (content.0.max(1) as f64, content.1.max(1) as f64);
    let (aw, ah) = (avail.width().max(1) as f64, avail.height().max(1) as f64);
    let s = (aw / cw).min(ah / ch);
    let (w, h) = ((cw * s).round() as i32, (ch * s).round() as i32);
    let left = avail.left + (avail.width() - w) / 2;
    let top = avail.top + (avail.height() - h) / 2;
    OsRect::new(left, top, left + w, top + h)
}

/// A mouse position in a window (X-Plane boxels, y up) as content pixels
/// with the origin bottom-left, the way avionics screen callbacks count.
/// `None` in the letterbox bars.
pub fn window_to_content(win: XpRect, content: (u32, u32), x: i32, y: i32) -> Option<(f64, f64)> {
    let w = (win.right - win.left) as f64;
    let h = (win.top - win.bottom) as f64;
    if w <= 0. || h <= 0. || content.0 == 0 || content.1 == 0 {
        return None;
    }
    let (cw, ch) = (content.0 as f64, content.1 as f64);
    let s = (w / cw).min(h / ch);
    let (dw, dh) = (cw * s, ch * s);
    let ox = win.left as f64 + (w - dw) / 2.;
    let oy = win.bottom as f64 + (h - dh) / 2.;
    let (px, py) = ((x as f64 + 0.5 - ox) / s, (y as f64 + 0.5 - oy) / s);
    (px >= 0. && py >= 0. && px < cw && py < ch).then_some((px, py))
}

/// The same position with the origin top-left, as panel art and CSS count.
pub fn window_to_panel(win: XpRect, content: (u32, u32), x: i32, y: i32) -> Option<(f64, f64)> {
    window_to_content(win, content, x, y).map(|(px, py)| (px, content.1 as f64 - py))
}

/// The monitor new pop-outs go to: the first one X-Plane is not running
/// full-screen on, else X-Plane's own.
pub fn pick_monitor(monitors: &[Monitor], sim_monitor: i32) -> Option<&Monitor> {
    monitors.iter().find(|m| m.index != sim_monitor).or_else(|| monitors.iter().find(|m| m.index == sim_monitor)).or(monitors.first())
}

const NEW_WINDOW_SHARE: f64 = 0.6;
const CASCADE: i32 = 40;
const MIN_ON_SCREEN: i32 = 120;

/// Where a window opens: the saved rectangle when its centre is still on a
/// monitor (pulled fully onto that monitor), else a fresh one on
/// [`pick_monitor`], sized to a share of it in the content's shape and
/// cascaded by `nth` so several pop-outs do not stack.
pub fn place(saved: Option<OsRect>, monitors: &[Monitor], sim_monitor: i32, content: (u32, u32), nth: usize) -> Option<OsRect> {
    if let Some(r) = saved.filter(|r| r.width() >= MIN_ON_SCREEN && r.height() >= MIN_ON_SCREEN) {
        let (cx, cy) = r.centre();
        if let Some(m) = monitors.iter().find(|m| m.rect.contains(cx, cy)) {
            return Some(clamp_onto(r, m.rect));
        }
    }
    let m = pick_monitor(monitors, sim_monitor)?.rect;
    let share = OsRect::new(0, 0, (m.width() as f64 * NEW_WINDOW_SHARE) as i32, (m.height() as f64 * NEW_WINDOW_SHARE) as i32);
    let size = fit(content, share);
    let step = (nth as i32 % 8) * CASCADE;
    let left = m.left + (m.width() - size.width()) / 2 - 3 * CASCADE + step;
    let top = m.top + (m.height() - size.height()) / 2 - 3 * CASCADE + step;
    Some(clamp_onto(OsRect::new(left, top, left + size.width(), top + size.height()), m))
}

fn clamp_onto(r: OsRect, m: OsRect) -> OsRect {
    let w = r.width().min(m.width());
    let h = r.height().min(m.height());
    let left = r.left.clamp(m.left, m.right - w);
    let top = r.top.clamp(m.top, m.bottom - h);
    OsRect::new(left, top, left + w, top + h)
}

/// The calls that take a device from `now` to `want`. VR without a headset
/// running, or without XPLMSetAvionicsMappedToVR in this X-Plane, floats it
/// instead.
pub fn transition(now: Mode, want: Mode, vr_running: bool, vr_api: bool, rect: Option<OsRect>) -> (Mode, Vec<Call>) {
    let want = if want == Mode::Vr && !(vr_running && vr_api) { Mode::Float } else { want };
    let mut calls = Vec::new();
    if now == Mode::Vr && want != Mode::Vr && vr_api {
        calls.push(Call::MapVr(false));
    }
    match want {
        Mode::Cockpit => calls.push(Call::PopupVisible(false)),
        Mode::Float => calls.push(Call::PopupVisible(true)),
        Mode::PopOut => {
            calls.push(Call::PopupVisible(true));
            calls.push(Call::PopOut);
            if let Some(r) = rect {
                let (l, t, r, b) = r.tuple();
                calls.push(Call::GeometryOs(l, t, r, b));
            }
        }
        Mode::Vr => {
            calls.push(Call::MapVr(true));
            calls.push(Call::PopupVisible(true));
        }
    }
    (want, calls)
}

/// Per-window placements and named layouts, kept in [`PREFERENCES`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Store {
    #[serde(default)]
    pub windows: BTreeMap<String, Placement>,
    #[serde(default)]
    pub layouts: BTreeMap<String, BTreeMap<String, Placement>>,
}

impl Store {
    pub fn parse(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or_default()
    }

    pub fn to_text(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).map(|t| Self::parse(&t)).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, self.to_text())?;
        std::fs::rename(tmp, path)
    }

    /// Records where a window is now. A window put back in the cockpit keeps
    /// its last OS rectangle, so the next pop-out returns there.
    pub fn remember(&mut self, id: &str, mode: Mode, os: Option<OsRect>) {
        let p = self.windows.entry(id.to_string()).or_default();
        p.mode = mode;
        if os.is_some() {
            p.os = os;
        }
    }

    pub fn saved_rect(&self, id: &str) -> Option<OsRect> {
        self.windows.get(id).and_then(|p| p.os)
    }

    /// Saves every window not in the cockpit under `name`.
    pub fn save_layout(&mut self, name: &str) {
        let open = self.windows.iter().filter(|(_, p)| p.mode != Mode::Cockpit).map(|(k, p)| (k.clone(), *p)).collect();
        self.layouts.insert(name.to_string(), open);
    }

    /// What restoring `name` does: every window in it to its saved mode and
    /// rectangle, every other open window back to the cockpit.
    pub fn restore_layout(&self, name: &str) -> Option<Vec<(String, Placement)>> {
        let layout = self.layouts.get(name)?;
        let mut out: Vec<(String, Placement)> = layout.iter().map(|(k, p)| (k.clone(), *p)).collect();
        for (k, p) in &self.windows {
            if p.mode != Mode::Cockpit && !layout.contains_key(k) {
                out.push((k.clone(), Placement { mode: Mode::Cockpit, os: p.os }));
            }
        }
        Some(out)
    }
}

/// How a panel control is bound: the same command or dataref its 3D
/// manipulator in `a380_cockpit_click.obj` uses.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Binding {
    Command { name: String },
    Knob { dec: String, inc: String },
    Dataref { name: String, values: Vec<f32> },
    Axis { name: String, min: f32, max: f32, step: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Button,
    Switch,
    GuardedSwitch,
    Knob,
    Rotary,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Control {
    pub id: String,
    /// Panel pixels, origin top-left: left, top, right, bottom.
    pub rect: [f32; 4],
    pub kind: Kind,
    pub binding: Binding,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guard: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lamp: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Panel {
    pub id: String,
    pub title: String,
    pub size: (u32, u32),
    pub art: String,
    pub controls: Vec<Control>,
    #[serde(default)]
    pub screens: Vec<(String, [f32; 4])>,
}

impl Panel {
    pub fn hit(&self, x: f64, y: f64) -> Option<&Control> {
        let (x, y) = (x as f32, y as f32);
        self.controls.iter().rev().find(|c| x >= c.rect[0] && x < c.rect[2] && y >= c.rect[1] && y < c.rect[3])
    }

    /// The screen shown in a live display window at this point, and the
    /// position in that screen's own pixels (origin top-left, 0..1 scaled).
    pub fn screen_at(&self, x: f64, y: f64) -> Option<(&str, f64, f64)> {
        self.screens.iter().find_map(|(id, r)| {
            let (l, t, rr, b) = (r[0] as f64, r[1] as f64, r[2] as f64, r[3] as f64);
            (x >= l && x < rr && y >= t && y < b).then(|| (id.as_str(), (x - l) / (rr - l), (y - t) / (b - t)))
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gesture {
    /// A press, with where in the control it landed (0..1 across).
    Press { across: f32, right_button: bool },
    Release,
    Wheel(i32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    CommandBegin(String),
    CommandEnd(String),
    CommandOnce(String),
    Set(String, f32),
    None,
}

/// What a gesture on a control does, given the bound dataref's value (and
/// the guard's, for a guarded switch). Knobs follow X-Plane's
/// command_knob: left half or right button or wheel down decreases.
pub fn act(control: &Control, gesture: Gesture, value: f32, guard_open: bool) -> Action {
    if let (Kind::GuardedSwitch, Some(guard), Gesture::Press { .. }) = (control.kind, &control.guard, gesture) {
        if !guard_open {
            return Action::Set(guard.clone(), 1.);
        }
    }
    let down = match gesture {
        Gesture::Press { across, right_button } => right_button || across < 0.5,
        Gesture::Wheel(n) => n < 0,
        Gesture::Release => false,
    };
    match (&control.binding, gesture) {
        (Binding::Command { name }, Gesture::Press { .. }) => Action::CommandBegin(name.clone()),
        (Binding::Command { name }, Gesture::Release) => Action::CommandEnd(name.clone()),
        (Binding::Knob { dec, inc }, Gesture::Press { .. } | Gesture::Wheel(_)) => Action::CommandOnce(if down { dec.clone() } else { inc.clone() }),
        (Binding::Dataref { name, values }, Gesture::Press { .. } | Gesture::Wheel(_)) if !values.is_empty() => {
            let at = values.iter().position(|v| (v - value).abs() < 1e-3).unwrap_or(0);
            let next = match (control.kind, gesture) {
                (Kind::Button | Kind::Switch | Kind::GuardedSwitch, Gesture::Press { right_button: false, .. }) => (at + 1) % values.len(),
                _ if down => at.saturating_sub(1),
                _ => (at + 1).min(values.len() - 1),
            };
            Action::Set(name.clone(), values[next])
        }
        (Binding::Axis { name, min, max, step }, Gesture::Press { .. } | Gesture::Wheel(_)) => {
            let v = if down { value - step } else { value + step };
            Action::Set(name.clone(), v.clamp(*min, *max))
        }
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(l: i32, t: i32, r: i32, b: i32) -> XpRect {
        XpRect { left: l, top: t, right: r, bottom: b }
    }

    #[test]
    fn fit_keeps_the_screen_shape_and_centres_it() {
        let r = fit((768, 1024), OsRect::new(0, 0, 1920, 1080));
        assert_eq!((r.width(), r.height()), (810, 1080));
        assert_eq!(r.left, (1920 - 810) / 2);
        let r = fit((2560, 1280), OsRect::new(100, 100, 1100, 1100));
        assert_eq!(r, OsRect::new(100, 350, 1100, 850));
    }

    #[test]
    fn a_click_maps_to_device_pixels_with_the_origin_bottom_left() {
        let w = win(0, 1024, 768, 0);
        assert_eq!(window_to_content(w, (768, 1024), 0, 0), Some((0.5, 0.5)));
        let (x, y) = window_to_content(w, (768, 1024), 767, 1023).unwrap();
        assert!((x - 767.5).abs() < 1e-9 && (y - 1023.5).abs() < 1e-9);
        let half = win(100, 612, 484, 100);
        let (x, y) = window_to_content(half, (768, 1024), 292, 356).unwrap();
        assert!((x - 385.).abs() < 1e-9 && (y - 513.).abs() < 1e-9);
    }

    #[test]
    fn clicks_in_the_letterbox_bars_go_nowhere() {
        let wide = win(0, 1024, 2000, 0);
        let bar = (2000 - 768) / 2;
        assert_eq!(window_to_content(wide, (768, 1024), bar - 1, 500), None);
        assert!(window_to_content(wide, (768, 1024), bar, 500).is_some());
        assert_eq!(window_to_content(wide, (768, 1024), bar + 768, 500), None);
        assert_eq!(window_to_content(win(0, 0, 0, 0), (768, 1024), 0, 0), None);
    }

    #[test]
    fn panel_coordinates_count_from_the_top() {
        let w = win(0, 400, 1000, 0);
        let (x, y) = window_to_panel(w, (1000, 400), 10, 390).unwrap();
        assert!((x - 10.5).abs() < 1e-9 && (y - 9.5).abs() < 1e-9);
    }

    fn monitors() -> Vec<Monitor> {
        vec![
            Monitor { index: 0, rect: OsRect::new(0, 0, 2560, 1440) },
            Monitor { index: 1, rect: OsRect::new(2560, 0, 4480, 1080) },
        ]
    }

    #[test]
    fn a_new_pop_out_opens_on_the_monitor_x_plane_is_not_on() {
        let r = place(None, &monitors(), 0, (768, 1024), 0).unwrap();
        assert!(r.left >= 2560 && r.right <= 4480 && r.top >= 0 && r.bottom <= 1080, "{r:?}");
        assert!(((r.width() as f64 / r.height() as f64) - 0.75).abs() < 0.01);
        let r = place(None, &monitors(), 1, (768, 1024), 0).unwrap();
        assert!(r.right <= 2560);
    }

    #[test]
    fn consecutive_pop_outs_cascade() {
        let a = place(None, &monitors(), 0, (768, 1024), 0).unwrap();
        let b = place(None, &monitors(), 0, (768, 1024), 1).unwrap();
        assert_eq!((b.left - a.left, b.top - a.top), (CASCADE, CASCADE));
    }

    #[test]
    fn a_saved_rect_is_kept_and_one_on_a_missing_monitor_is_replaced() {
        let saved = OsRect::new(3000, 100, 3600, 900);
        assert_eq!(place(Some(saved), &monitors(), 0, (768, 1024), 0), Some(saved));
        let only_main = &monitors()[..1];
        let r = place(Some(saved), only_main, 0, (768, 1024), 0).unwrap();
        assert!(r.right <= 2560);
        let hanging = OsRect::new(2300, 1000, 2700, 1500);
        let r = place(Some(hanging), &monitors(), 0, (768, 1024), 0).unwrap();
        assert!(r.bottom <= 1440 && r.right <= 2560, "{r:?}");
    }

    #[test]
    fn no_monitors_means_no_rect() {
        assert_eq!(place(None, &[], 0, (768, 1024), 0), None);
    }

    #[test]
    fn transitions_issue_x_planes_calls_in_order() {
        let r = OsRect::new(1, 2, 3, 4);
        assert_eq!(transition(Mode::Cockpit, Mode::PopOut, false, true, Some(r)).1, vec![Call::PopupVisible(true), Call::PopOut, Call::GeometryOs(1, 2, 3, 4)]);
        assert_eq!(transition(Mode::Cockpit, Mode::Vr, true, true, None), (Mode::Vr, vec![Call::MapVr(true), Call::PopupVisible(true)]));
        assert_eq!(transition(Mode::Vr, Mode::Cockpit, true, true, None).1, vec![Call::MapVr(false), Call::PopupVisible(false)]);
        assert_eq!(transition(Mode::Cockpit, Mode::Vr, false, true, None), (Mode::Float, vec![Call::PopupVisible(true)]));
        assert_eq!(transition(Mode::Cockpit, Mode::Vr, true, false, None).0, Mode::Float);
    }

    #[test]
    fn the_store_round_trips_and_remembers_the_last_os_rect() {
        let mut s = Store::default();
        let r = OsRect::new(2600, 50, 3400, 1050);
        s.remember("SCREEN_DU_PFDL", Mode::PopOut, Some(r));
        s.remember("SCREEN_DU_PFDL", Mode::Cockpit, None);
        assert_eq!(s.saved_rect("SCREEN_DU_PFDL"), Some(r));
        let back = Store::parse(&s.to_text());
        assert_eq!(back, s);
        assert_eq!(Store::parse("not json"), Store::default());
    }

    #[test]
    fn a_named_layout_restores_its_windows_and_closes_the_rest() {
        let mut s = Store::default();
        let pfd = OsRect::new(2600, 0, 3400, 1066);
        s.remember("SCREEN_DU_PFDL", Mode::PopOut, Some(pfd));
        s.remember("panel/fcu", Mode::PopOut, Some(OsRect::new(3400, 0, 4400, 300)));
        s.remember("SCREEN_DU_EWD", Mode::Cockpit, None);
        s.save_layout("Captain 2nd monitor");
        s.remember("panel/fcu", Mode::Cockpit, None);
        s.remember("SCREEN_DU_SD", Mode::Float, None);
        let calls = s.restore_layout("Captain 2nd monitor").unwrap();
        assert!(calls.contains(&("SCREEN_DU_PFDL".into(), Placement { mode: Mode::PopOut, os: Some(pfd) })));
        assert!(calls.iter().any(|(k, p)| k == "panel/fcu" && p.mode == Mode::PopOut));
        assert!(calls.iter().any(|(k, p)| k == "SCREEN_DU_SD" && p.mode == Mode::Cockpit));
        assert!(!calls.iter().any(|(k, _)| k == "SCREEN_DU_EWD"));
        assert!(s.restore_layout("nope").is_none());
    }

    fn panel() -> Panel {
        let c = |id: &str, rect: [f32; 4], kind: Kind, binding: Binding| Control { id: id.into(), rect, kind, binding, guard: None, lamp: None };
        let mut guarded = c("apu_fire", [300., 0., 360., 60.], Kind::GuardedSwitch, Binding::Dataref { name: "fbw/cockpit/apu_fire".into(), values: vec![0., 1.] });
        guarded.guard = Some("fbw/cockpit/apu_fire_guard".into());
        Panel {
            id: "glareshield_capt".into(),
            title: "Captain's glareshield".into(),
            size: (400, 100),
            art: "glareshield_capt.png".into(),
            controls: vec![
                c("warn", [0., 0., 50., 50.], Kind::Button, Binding::Command { name: "fbw/cockpit/master_warn".into() }),
                c("baro", [100., 0., 160., 60.], Kind::Knob, Binding::Knob { dec: "fbw/baro_dn".into(), inc: "fbw/baro_up".into() }),
                c("nd_mode", [200., 0., 260., 60.], Kind::Rotary, Binding::Dataref { name: "fbw/nd_mode".into(), values: vec![0., 1., 2., 3., 4.] }),
                guarded,
            ],
            screens: vec![("FCU".into(), [0., 60., 400., 100.])],
        }
    }

    #[test]
    fn hit_testing_finds_the_control_under_the_pointer() {
        let p = panel();
        assert_eq!(p.hit(10., 10.).map(|c| c.id.as_str()), Some("warn"));
        assert_eq!(p.hit(55., 10.), None);
        assert_eq!(p.hit(259.9, 59.9).map(|c| c.id.as_str()), Some("nd_mode"));
        let (id, u, v) = p.screen_at(200., 80.).unwrap();
        assert_eq!(id, "FCU");
        assert!((u - 0.5).abs() < 1e-9 && (v - 0.5).abs() < 1e-9);
    }

    #[test]
    fn controls_do_what_their_manipulator_does() {
        let p = panel();
        let press = |across| Gesture::Press { across, right_button: false };
        assert_eq!(act(&p.controls[0], press(0.5), 0., false), Action::CommandBegin("fbw/cockpit/master_warn".into()));
        assert_eq!(act(&p.controls[0], Gesture::Release, 0., false), Action::CommandEnd("fbw/cockpit/master_warn".into()));
        assert_eq!(act(&p.controls[1], press(0.2), 0., false), Action::CommandOnce("fbw/baro_dn".into()));
        assert_eq!(act(&p.controls[1], Gesture::Wheel(1), 0., false), Action::CommandOnce("fbw/baro_up".into()));
        assert_eq!(act(&p.controls[2], press(0.8), 2., false), Action::Set("fbw/nd_mode".into(), 3.));
        assert_eq!(act(&p.controls[2], press(0.2), 0., false), Action::Set("fbw/nd_mode".into(), 0.));
        assert_eq!(act(&p.controls[2], Gesture::Wheel(1), 4., false), Action::Set("fbw/nd_mode".into(), 4.));
    }

    #[test]
    fn a_guarded_switch_lifts_its_guard_first() {
        let p = panel();
        let press = Gesture::Press { across: 0.5, right_button: false };
        assert_eq!(act(&p.controls[3], press, 0., false), Action::Set("fbw/cockpit/apu_fire_guard".into(), 1.));
        assert_eq!(act(&p.controls[3], press, 0., true), Action::Set("fbw/cockpit/apu_fire".into(), 1.));
    }

    #[test]
    fn a_panel_description_reads_from_json() {
        let p = panel();
        let text = serde_json::to_string(&p).unwrap();
        let back: Panel = serde_json::from_str(&text).unwrap();
        assert_eq!(back, p);
    }
}

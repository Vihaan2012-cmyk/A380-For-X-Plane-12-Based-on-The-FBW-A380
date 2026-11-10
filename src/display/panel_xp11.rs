//! X-Plane 11's screens: X-Plane 11 has no cockpit devices, so each screen is drawn offscreen as it would
//! be into a device and then as a quad into X-Plane's 3D panel texture, which the cockpit object maps onto
//! the screen glass through ATTR_cockpit. Where each screen sits on the panel comes from xp11_panel.json,
//! written by tools/xp11/make_xp11.py next to the plugin. A click on the glass is a 3D panel click:
//! X-Plane reports where it landed and [`Pointer`] turns that and the left button into presses.

use std::collections::HashMap;
use std::ffi::{c_int, c_void};
use std::sync::OnceLock;

use crate::xp::{self, Xplm};

/// Panel pixels from the bottom left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Default, PartialEq)]
pub struct Atlas {
    pub panel: (f32, f32),
    pub screens: HashMap<String, Rect>,
}

impl Atlas {
    pub fn parse(text: &str) -> Result<Self, String> {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let panel = v.get("panel").and_then(|p| p.as_array()).ok_or("no panel size")?;
        let num = |x: &serde_json::Value| x.as_f64().unwrap_or(0.) as f32;
        let panel = (num(panel.first().ok_or("no panel width")?), num(panel.get(1).ok_or("no panel height")?));
        let mut screens = HashMap::new();
        for (id, r) in v.get("screens").and_then(|s| s.as_object()).ok_or("no screens")? {
            let f = |k: &str| r.get(k).map(num).unwrap_or(0.);
            screens.insert(id.clone(), Rect { x: f("x"), y: f("y"), w: f("w"), h: f("h") });
        }
        Ok(Atlas { panel, screens })
    }

    /// The screen under a panel position, and the position on it in screen pixels from the bottom left.
    pub fn hit(&self, px: f32, py: f32, size: impl Fn(&str) -> Option<(u32, u32)>) -> Option<(String, c_int, c_int)> {
        for (id, r) in &self.screens {
            if px >= r.x && px < r.x + r.w && py >= r.y && py < r.y + r.h {
                let (w, h) = size(id)?;
                let x = ((px - r.x) / r.w * w as f32) as c_int;
                let y = ((py - r.y) / r.h * h as f32) as c_int;
                return Some((id.clone(), x, y));
            }
        }
        None
    }

    /// X-Plane 11 reports a 3D panel click as 0..1 across the panel texture; a reading above 1 is taken as pixels.
    pub fn click_to_panel(&self, u: f32, v: f32) -> (f32, f32) {
        if u > 1.5 || v > 1.5 {
            (u, v)
        } else {
            (u * self.panel.0, v * self.panel.1)
        }
    }
}

/// Presses on the panel texture from what X-Plane 11 reports each frame: the last 3D panel click position
/// (changed means a new click landed on the panel), the left button, and the mouse position (a second click
/// on the very same spot leaves the position unchanged).
#[derive(Default)]
pub struct Pointer {
    last: Option<(f32, f32)>,
    held: bool,
    was_down: bool,
    press_mouse: Option<(c_int, c_int)>,
}

impl Pointer {
    /// One frame; returns (status, u, v) for each mouse event.
    pub fn step(&mut self, uv: (f32, f32), down: bool, mouse: (c_int, c_int)) -> Vec<(c_int, f32, f32)> {
        let mut out = Vec::new();
        let Some(last) = self.last.replace(uv) else {
            self.was_down = down;
            return out;
        };
        let moved = (uv.0 - last.0).abs() > 1e-6 || (uv.1 - last.1).abs() > 1e-6;
        if uv.0 < 0. || uv.1 < 0. {
            if self.held {
                out.push((xp::MOUSE_UP, last.0, last.1));
                self.held = false;
            }
            self.was_down = down;
            return out;
        }
        if self.held {
            if moved {
                out.push((xp::MOUSE_DRAG, uv.0, uv.1));
            }
            if !down {
                out.push((xp::MOUSE_UP, uv.0, uv.1));
                self.held = false;
            }
        } else if moved {
            out.push((xp::MOUSE_DOWN, uv.0, uv.1));
            self.press_mouse = Some(mouse);
            if down {
                self.held = true;
            } else {
                out.push((xp::MOUSE_UP, uv.0, uv.1));
            }
        } else if down && !self.was_down && self.press_mouse == Some(mouse) {
            out.push((xp::MOUSE_DOWN, uv.0, uv.1));
            self.held = true;
        }
        self.was_down = down;
        out
    }
}

/// The left mouse button, read from Windows: X-Plane 11 has no API for it.
#[cfg(windows)]
pub fn left_button_down() -> bool {
    #[link(name = "user32")]
    extern "system" {
        fn GetAsyncKeyState(key: c_int) -> i16;
    }
    unsafe { (GetAsyncKeyState(0x01) as u16) & 0x8000 != 0 }
}

#[cfg(not(windows))]
pub fn left_button_down() -> bool {
    false
}

/// XPLMGetMouseLocationGlobal, in boxels.
pub fn mouse_location() -> (c_int, c_int) {
    let Some(f) = Xplm::xplm_symbol("XPLMGetMouseLocationGlobal") else { return (-1, -1) };
    let f = unsafe { std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*mut c_int, *mut c_int)>(f) };
    let (mut x, mut y) = (0, 0);
    unsafe { f(&mut x, &mut y) };
    (x, y)
}

static ATLAS: OnceLock<Option<Atlas>> = OnceLock::new();

pub fn atlas() -> Option<&'static Atlas> {
    ATLAS
        .get_or_init(|| {
            let path = crate::js_bridge::plugin_dir()?.join("xp11_panel.json");
            match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| Atlas::parse(&t)) {
                Ok(a) => {
                    super::log(&format!("xp11: {} screens on a {}x{} panel ({})", a.screens.len(), a.panel.0, a.panel.1, path.display()));
                    Some(a)
                }
                Err(e) => {
                    super::log(&format!("xp11: no panel layout at {}: {e}; the screens stay dark", path.display()));
                    None
                }
            }
        })
        .as_ref()
}

/// xplm_Phase_Gauges: X-Plane is drawing a panel.
const PHASE_GAUGES: c_int = 45;

unsafe extern "C" fn draw(_phase: c_int, _before: c_int, _refcon: *mut c_void) -> c_int {
    let _ = std::panic::catch_unwind(|| crate::perf::time("screens (panel)", || super::with_from_callback(|d| d.draw_panel())));
    1
}

unsafe extern "C" fn click(_command: xp::CommandRef, phase: c_int, _refcon: *mut c_void) -> c_int {
    let _ = std::panic::catch_unwind(|| super::with_from_callback(|d| d.panel_click(phase)));
    0
}

pub fn start(xplm: &'static Xplm) {
    if atlas().is_none() {
        return;
    }
    if !xp::register_draw_callback(draw, PHASE_GAUGES, false) {
        super::log("xp11: X-Plane refused the panel draw callback; the screens stay dark");
    }
    match xplm.create_command("fbw/xp11/screen_click", "Click on an A380 screen (X-Plane 11)") {
        Some(c) => xplm.register_command_handler(c, click, std::ptr::null_mut()),
        None => super::log("xp11: no screen click command; the screens cannot be clicked"),
    }
}

pub fn stop() {
    xp::unregister_draw_callback(draw, PHASE_GAUGES, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAYOUT: &str = r#"{"panel": [4096, 4096], "screens": {"SCREEN_DU_PFDL": {"x": 0, "y": 0, "w": 768, "h": 1024}, "FCU": {"x": 2048, "y": 0, "w": 1280, "h": 640}}}"#;

    #[test]
    fn the_layout_reads_the_panel_size_and_each_screens_rectangle() {
        let a = Atlas::parse(LAYOUT).unwrap();
        assert_eq!(a.panel, (4096., 4096.));
        assert_eq!(a.screens["FCU"], Rect { x: 2048., y: 0., w: 1280., h: 640. });
    }

    #[test]
    fn a_panel_position_finds_its_screen_and_the_pixel_on_it_at_full_size() {
        let a = Atlas::parse(LAYOUT).unwrap();
        let size = |id: &str| match id {
            "SCREEN_DU_PFDL" => Some((768, 1024)),
            "FCU" => Some((2560, 1280)),
            _ => None,
        };
        assert_eq!(a.hit(384., 512., size), Some(("SCREEN_DU_PFDL".into(), 384, 512)));
        assert_eq!(a.hit(2048. + 640., 320., size), Some(("FCU".into(), 1280, 640)));
        assert_eq!(a.hit(4000., 4000., size), None);
    }

    #[test]
    fn a_new_click_position_with_the_button_held_presses_drags_and_releases() {
        let mut p = Pointer::default();
        assert!(p.step((0., 0.), false, (5, 5)).is_empty());
        assert_eq!(p.step((0.2, 0.3), true, (10, 10)), vec![(xp::MOUSE_DOWN, 0.2, 0.3)]);
        assert!(p.step((0.2, 0.3), true, (10, 10)).is_empty());
        assert_eq!(p.step((0.25, 0.3), true, (12, 10)), vec![(xp::MOUSE_DRAG, 0.25, 0.3)]);
        assert_eq!(p.step((0.25, 0.3), false, (12, 10)), vec![(xp::MOUSE_UP, 0.25, 0.3)]);
    }

    #[test]
    fn a_click_inside_one_frame_is_a_tap_and_the_same_spot_again_presses_again() {
        let mut p = Pointer::default();
        p.step((0., 0.), false, (0, 0));
        assert_eq!(p.step((0.5, 0.5), false, (40, 40)), vec![(xp::MOUSE_DOWN, 0.5, 0.5), (xp::MOUSE_UP, 0.5, 0.5)]);
        assert_eq!(p.step((0.5, 0.5), true, (40, 40)), vec![(xp::MOUSE_DOWN, 0.5, 0.5)]);
        assert_eq!(p.step((0.5, 0.5), false, (40, 40)), vec![(xp::MOUSE_UP, 0.5, 0.5)]);
    }

    #[test]
    fn x_plane_clearing_the_click_to_minus_one_is_no_press() {
        let mut p = Pointer::default();
        p.step((0.3, 0.3), false, (0, 0));
        assert!(p.step((-1., -1.), false, (9, 9)).is_empty());
        assert_eq!(p.step((0.3, 0.3), false, (9, 9)), vec![(xp::MOUSE_DOWN, 0.3, 0.3), (xp::MOUSE_UP, 0.3, 0.3)]);
    }

    #[test]
    fn a_click_elsewhere_in_the_cockpit_does_nothing() {
        let mut p = Pointer::default();
        p.step((0.1, 0.1), false, (0, 0));
        assert!(p.step((0.1, 0.1), true, (300, 200)).is_empty());
        assert!(p.step((0.1, 0.1), false, (300, 200)).is_empty());
    }

    #[test]
    fn clicks_in_texture_units_scale_to_the_panel_and_pixels_pass_through() {
        let a = Atlas::parse(LAYOUT).unwrap();
        assert_eq!(a.click_to_panel(0.5, 0.25), (2048., 1024.));
        assert_eq!(a.click_to_panel(700., 300.), (700., 300.));
    }
}

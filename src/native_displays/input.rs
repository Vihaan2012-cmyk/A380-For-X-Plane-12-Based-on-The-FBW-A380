//! Clicks and keys on native-owned screens go to the display library
//! (`a380d_pointer`/`a380d_key`), not to Chromium. The library answers with
//! the writes FlyByWire's page makes for that element; [`apply`] hands them
//! to the variables and the event queues once they are due.
//!
//! Handles are registered by the render thread and used from the main
//! thread; the registry lock keeps a handle alive for the whole call.

use std::ffi::{c_char, c_int, c_void};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::ffi;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RawWrite {
    pub kind: c_char,
    pub name: [c_char; 127],
    pub value: f64,
    pub delay_ms: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pointer {
    Down = 0,
    Drag = 1,
    Up = 2,
    Move = 3,
    Wheel = 4,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Write {
    pub kind: char,
    pub name: String,
    pub value: f64,
}

/// Where an event on a screen goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// The native display took it (or the screen's input is all native and
    /// nothing was there): nothing goes to the page.
    Native,
    /// Native-owned screen whose page still runs part of its input logic.
    Page,
    /// Not a native screen.
    NotNative,
}

struct Handle(usize);

static HANDLES: Mutex<Vec<(&'static str, Handle)>> = Mutex::new(Vec::new());
static PENDING: Mutex<Vec<(Instant, Write)>> = Mutex::new(Vec::new());

/// The render thread opened `display` as a layer of `screen` (bottom first).
pub fn register(screen: &'static str, display: &super::Display) {
    if let Ok(mut h) = HANDLES.lock() {
        h.push((screen, Handle(display.raw() as usize)));
    }
}

/// Before the render thread drops `screen`'s displays.
pub fn unregister(screen: &str) {
    if let Ok(mut h) = HANDLES.lock() {
        h.retain(|(s, _)| *s != screen);
    }
}

pub fn unregister_all() {
    if let Ok(mut h) = HANDLES.lock() {
        h.clear();
    }
}

pub fn is_native(screen: &str) -> bool {
    HANDLES.lock().map(|h| h.iter().any(|(s, _)| *s == screen)).unwrap_or(false)
}

fn name_of(raw: &RawWrite) -> String {
    let bytes: Vec<u8> = raw.name.iter().take_while(|c| **c != 0).map(|c| *c as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn collect(handle: *mut c_void, now: Instant) {
    let mut buf = [RawWrite { kind: 0, name: [0; 127], value: 0., delay_ms: 0. }; 16];
    loop {
        let n = unsafe { ffi::a380d_take_writes(handle, buf.as_mut_ptr(), buf.len() as c_int) };
        if n <= 0 {
            return;
        }
        if let Ok(mut p) = PENDING.lock() {
            for raw in &buf[..n as usize] {
                let due = now + Duration::from_secs_f64(raw.delay_ms.max(0.) / 1000.);
                p.push((due, Write { kind: raw.kind as u8 as char, name: name_of(raw), value: raw.value }));
            }
        }
        if (n as usize) < buf.len() {
            return;
        }
    }
}

fn route(screen: &str, mut call: impl FnMut(*mut c_void) -> c_int) -> Route {
    let Ok(handles) = HANDLES.lock() else { return Route::NotNative };
    let layers: Vec<*mut c_void> = handles.iter().filter(|(s, _)| *s == screen).map(|(_, h)| h.0 as *mut c_void).collect();
    if layers.is_empty() {
        return Route::NotNative;
    }
    let now = Instant::now();
    let mut took = false;
    for &layer in layers.iter().rev() {
        took = call(layer) == 1;
        collect(layer, now);
        if took {
            break;
        }
    }
    if took || layers.iter().all(|&l| unsafe { ffi::a380d_input_native(l) } == 1) {
        Route::Native
    } else {
        Route::Page
    }
}

/// A pointer event at CSS pixels of `screen`'s page (origin top-left).
pub fn pointer(screen: &str, x: f64, y: f64, kind: Pointer, button: i32) -> Route {
    route(screen, |h| unsafe { ffi::a380d_pointer(h, x, y, kind as c_int, button) })
}

pub fn key(screen: &str, key: u32, mods: u32, down: bool) -> Route {
    route(screen, |h| unsafe { ffi::a380d_key(h, key as c_int, mods as c_int, down as c_int) })
}

/// Writes whose time has come, oldest first.
pub fn take_due(now: Instant) -> Vec<Write> {
    let Ok(mut p) = PENDING.lock() else { return Vec::new() };
    let mut due = Vec::new();
    p.retain(|(at, w)| {
        if *at <= now {
            due.push(w.clone());
            false
        } else {
            true
        }
    });
    due
}

/// Applies the due writes: `L:` into the variables, `H:` events onto
/// `h_events` (names without `H:`, as `js_bridge::take_hevents` gives them),
/// `K:` events onto `k_events`.
pub fn apply(vars: &mut crate::Vars, h_events: &mut Vec<String>, k_events: &mut Vec<(String, f64)>) {
    use systems::simulation::SimulatorReaderWriter;
    for w in take_due(Instant::now()) {
        match w.kind {
            'L' => {
                let id = vars.add(w.name.trim_start_matches("L:").to_string(), crate::NAMED);
                vars.write(&id, w.value);
            }
            'H' => h_events.push(w.name.trim_start_matches("H:").to_string()),
            'K' => k_events.push((w.name.clone(), w.value)),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delayed_writes_wait_for_their_time() {
        let now = Instant::now();
        if let Ok(mut p) = PENDING.lock() {
            p.push((now, Write { kind: 'L', name: "L:TEST_INPUT_A".into(), value: 1. }));
            p.push((now + Duration::from_millis(50), Write { kind: 'L', name: "L:TEST_INPUT_A".into(), value: 0. }));
        }
        let first: Vec<_> = take_due(now).into_iter().filter(|w| w.name == "L:TEST_INPUT_A").collect();
        assert_eq!(first, vec![Write { kind: 'L', name: "L:TEST_INPUT_A".into(), value: 1. }]);
        let later: Vec<_> = take_due(now + Duration::from_millis(60)).into_iter().filter(|w| w.name == "L:TEST_INPUT_A").collect();
        assert_eq!(later, vec![Write { kind: 'L', name: "L:TEST_INPUT_A".into(), value: 0. }]);
    }

    #[test]
    fn a_screen_with_no_native_layers_is_not_native() {
        assert_eq!(pointer("no-such-screen", 1., 1., Pointer::Down, 0), Route::NotNative);
        assert!(!is_native("no-such-screen"));
    }
}

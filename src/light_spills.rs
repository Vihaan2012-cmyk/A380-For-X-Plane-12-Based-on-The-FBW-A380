//! Hides the aircraft's spill lights in daylight (`lights.obj`, `light_spill_budget.py`). A spill
//! is one deferred pass over every pixel its cone covers, and the sun outshines all of them.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use crate::xp::{DataRef, Xplm};

const HIDE_ABOVE_DEG: f32 = 8.;
const SHOW_BELOW_DEG: f32 = 5.;

static HIDDEN: AtomicBool = AtomicBool::new(false);
static REGISTERED: AtomicBool = AtomicBool::new(false);
static NAME: OnceLock<std::ffi::CString> = OnceLock::new();

unsafe extern "C" fn hidden_f(_refcon: *mut std::ffi::c_void) -> f32 {
    HIDDEN.load(Ordering::Relaxed) as i32 as f32
}

unsafe extern "C" fn hidden_i(_refcon: *mut std::ffi::c_void) -> std::ffi::c_int {
    HIDDEN.load(Ordering::Relaxed) as std::ffi::c_int
}

/// Called from `XPluginEnable`, so the dataref exists before `lights.obj` looks it up.
pub fn register() {
    if REGISTERED.swap(true, Ordering::Relaxed) {
        return;
    }
    let name = NAME.get_or_init(|| std::ffi::CString::new("fbw/anim/spills_hidden").unwrap_or_default());
    let accessors = crate::xp::Accessors { get_i: Some(hidden_i), get_f: Some(hidden_f), ..Default::default() };
    let _ = crate::xp::register_owned_accessor(name, crate::xp::DATA_TYPE_INT | crate::xp::DATA_TYPE_FLOAT, false, &accessors, std::ptr::null_mut());
}

/// `xp::unregister_owned` removed it; the next `register` publishes it again.
pub fn forget() {
    REGISTERED.store(false, Ordering::Relaxed);
    HIDDEN.store(false, Ordering::Relaxed);
}

fn hidden_now(was: bool, sun_pitch_deg: f32) -> bool {
    if was {
        sun_pitch_deg > SHOW_BELOW_DEG
    } else {
        sun_pitch_deg > HIDE_ABOVE_DEG
    }
}

#[derive(Default)]
pub struct LightSpills {
    sun: Option<Option<DataRef>>,
}

impl LightSpills {
    pub fn update(&mut self, xplm: &Xplm) {
        let Some(sun) = *self.sun.get_or_insert_with(|| xplm.find("sim/graphics/scenery/sun_pitch_degrees")) else { return };
        let was = HIDDEN.load(Ordering::Relaxed);
        let now = hidden_now(was, xplm.get_f(sun));
        if now != was {
            HIDDEN.store(now, Ordering::Relaxed);
            crate::log(&format!("light spills {} (sun {:.1} deg)", if now { "hidden in daylight" } else { "shown" }, xplm.get_f(sun)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::hidden_now;

    #[test]
    fn daylight_hides_the_spills_with_a_margin_so_they_do_not_flick_at_dusk() {
        assert!(!hidden_now(false, 7.));
        assert!(hidden_now(false, 9.));
        assert!(hidden_now(true, 6.));
        assert!(!hidden_now(true, 4.));
        assert!(!hidden_now(false, -20.));
    }
}

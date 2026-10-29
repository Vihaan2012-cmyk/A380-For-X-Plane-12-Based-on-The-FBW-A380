//! `fbw/anim/eye_in_flight_deck`: 1 while the pilot's eye is inside the
//! flight deck, where the wings, engines, gear and tail cannot be seen, so the
//! exterior objects that are hidden by it are not drawn into the cockpit view
//! or its shadow passes.
//!
//! `fbw/anim/cabin_hidden`: 1 while the eye is in the flight deck and the
//! flight deck door is shut, the acf hide dataref of every cabin object (the
//! whole object is skipped, as FlightFactor's cabinRem does).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use crate::xp::{DataRef, Xplm};

const AFT_OF_DEFAULT_EYE_M: f32 = 2.5;
const DOOR_SHUT_BELOW: f32 = 0.02;
const DOOR: &str = "fbw/cockpit/COCKPITDOOR_OPEN_ANIM";

static IN_FLIGHT_DECK: AtomicBool = AtomicBool::new(false);
static CABIN_HIDDEN: AtomicBool = AtomicBool::new(false);
static REGISTERED: AtomicBool = AtomicBool::new(false);
static NAMES: OnceLock<[std::ffi::CString; 2]> = OnceLock::new();

unsafe extern "C" fn in_deck_f(_refcon: *mut std::ffi::c_void) -> f32 {
    IN_FLIGHT_DECK.load(Ordering::Relaxed) as i32 as f32
}

unsafe extern "C" fn in_deck_i(_refcon: *mut std::ffi::c_void) -> std::ffi::c_int {
    IN_FLIGHT_DECK.load(Ordering::Relaxed) as std::ffi::c_int
}

unsafe extern "C" fn cabin_f(_refcon: *mut std::ffi::c_void) -> f32 {
    CABIN_HIDDEN.load(Ordering::Relaxed) as i32 as f32
}

unsafe extern "C" fn cabin_i(_refcon: *mut std::ffi::c_void) -> std::ffi::c_int {
    CABIN_HIDDEN.load(Ordering::Relaxed) as std::ffi::c_int
}

/// Publishes both datarefs; called from `XPluginEnable` so they exist before
/// the aircraft's objects look their hide datarefs up.
pub fn register() {
    if REGISTERED.swap(true, Ordering::Relaxed) {
        return;
    }
    let [deck, cabin] = NAMES.get_or_init(|| {
        [
            std::ffi::CString::new("fbw/anim/eye_in_flight_deck").unwrap_or_default(),
            std::ffi::CString::new("fbw/anim/cabin_hidden").unwrap_or_default(),
        ]
    });
    let types = crate::xp::DATA_TYPE_INT | crate::xp::DATA_TYPE_FLOAT;
    let deck_acc = crate::xp::Accessors { get_i: Some(in_deck_i), get_f: Some(in_deck_f), ..Default::default() };
    let cabin_acc = crate::xp::Accessors { get_i: Some(cabin_i), get_f: Some(cabin_f), ..Default::default() };
    let _ = crate::xp::register_owned_accessor(deck, types, false, &deck_acc, std::ptr::null_mut());
    let _ = crate::xp::register_owned_accessor(cabin, types, false, &cabin_acc, std::ptr::null_mut());
}

/// `xp::unregister_owned` removed them; the next `register` publishes them again.
pub fn forget() {
    REGISTERED.store(false, Ordering::Relaxed);
    IN_FLIGHT_DECK.store(false, Ordering::Relaxed);
    CABIN_HIDDEN.store(false, Ordering::Relaxed);
}

fn in_flight_deck(inside: bool, head_z: f32, default_eye_z: Option<f32>) -> bool {
    inside && default_eye_z.is_some_and(|eye| head_z - eye < AFT_OF_DEFAULT_EYE_M)
}

fn cabin_hidden(in_deck: bool, door: Option<f32>) -> bool {
    in_deck && door.is_some_and(|d| d < DOOR_SHUT_BELOW)
}

#[derive(Default)]
pub struct FlightDeckView {
    external: Option<Option<DataRef>>,
    head_z: Option<Option<DataRef>>,
    door: Option<Option<DataRef>>,
    default_eye_z: Option<f32>,
}

impl FlightDeckView {
    pub fn update(&mut self, xplm: &Xplm) {
        register();
        let external = *self.external.get_or_insert_with(|| xplm.find("sim/graphics/view/view_is_external"));
        let head = *self.head_z.get_or_insert_with(|| xplm.find("sim/graphics/view/pilots_head_z"));
        let (Some(external), Some(head)) = (external, head) else { return };
        let inside = xplm.get_i(external) == 0;
        let z = xplm.get_f(head);
        if inside && self.default_eye_z.is_none() {
            self.default_eye_z = Some(z);
        }
        let deck = in_flight_deck(inside, z, self.default_eye_z);
        if self.door.flatten().is_none() {
            self.door = Some(xplm.find(DOOR));
        }
        let door = self.door.flatten().map(|d| xplm.get_f(d));
        IN_FLIGHT_DECK.store(deck, Ordering::Relaxed);
        CABIN_HIDDEN.store(cabin_hidden(deck, door), Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::{cabin_hidden, in_flight_deck};

    #[test]
    fn the_eye_counts_as_in_the_flight_deck_only_inside_and_near_the_default_eye() {
        assert!(in_flight_deck(true, -29.6, Some(-29.63)));
        assert!(in_flight_deck(true, -31.0, Some(-29.63)), "leaning forward");
        assert!(!in_flight_deck(true, -26.0, Some(-29.63)), "walked back into the cabin");
        assert!(!in_flight_deck(false, -29.6, Some(-29.63)), "outside view");
        assert!(!in_flight_deck(true, -29.6, None), "no default eye yet");
    }

    #[test]
    fn the_cabin_is_skipped_only_from_the_flight_deck_with_its_door_shut() {
        assert!(cabin_hidden(true, Some(0.0)));
        assert!(!cabin_hidden(true, Some(0.5)), "door open: the cabin shows through it");
        assert!(!cabin_hidden(false, Some(0.0)), "outside or in the cabin");
        assert!(!cabin_hidden(true, None), "no door dataref: keep the cabin");
    }
}

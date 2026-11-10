//! `fbw/anim/eye_in_flight_deck`: 1 while the pilot's eye is inside the
//! flight deck, where the wings, engines, gear and tail cannot be seen, so the
//! exterior objects that are hidden by it are not drawn into the cockpit view
//! or its shadow passes.
//!
//! `fbw/anim/cabin_hidden`: 1 while the eye is in the flight deck and the
//! flight deck door is shut, the acf hide dataref of every cabin object (the
//! whole object is skipped, as FlightFactor's cabinRem does).
//!
//! `fbw/anim/deck_unseen`: 1 while the eye is outside the flight deck and more
//! than `DECK_DETAIL_RANGE_M` from it, the hide dataref of the flight deck's
//! knobs, panels and lettering (FlightFactor's ckptOut); its walls, seats,
//! glareshield and windows stay.
//!
//! `fbw/anim/deck_far`: the same beyond `DECK_FAR_RANGE_M`, where the rest of the
//! flight deck behind the windscreen is a few pixels and is skipped too.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use crate::xp::{DataRef, Xplm};

const AFT_OF_DEFAULT_EYE_M: f32 = 2.5;
const DOOR_SHUT_BELOW: f32 = 0.02;
const DOOR: &str = "fbw/cockpit/COCKPITDOOR_OPEN_ANIM";
const DECK_DETAIL_RANGE_M: f32 = 20.0;
const DECK_FAR_RANGE_M: f32 = 60.0;

static IN_FLIGHT_DECK: AtomicBool = AtomicBool::new(false);
static CABIN_HIDDEN: AtomicBool = AtomicBool::new(false);
static DECK_UNSEEN: AtomicBool = AtomicBool::new(false);
static DECK_FAR: AtomicBool = AtomicBool::new(false);
static REGISTERED: AtomicBool = AtomicBool::new(false);
static NAMES: OnceLock<[std::ffi::CString; 4]> = OnceLock::new();

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

unsafe extern "C" fn unseen_f(_refcon: *mut std::ffi::c_void) -> f32 {
    DECK_UNSEEN.load(Ordering::Relaxed) as i32 as f32
}

unsafe extern "C" fn unseen_i(_refcon: *mut std::ffi::c_void) -> std::ffi::c_int {
    DECK_UNSEEN.load(Ordering::Relaxed) as std::ffi::c_int
}

unsafe extern "C" fn far_f(_refcon: *mut std::ffi::c_void) -> f32 {
    DECK_FAR.load(Ordering::Relaxed) as i32 as f32
}

unsafe extern "C" fn far_i(_refcon: *mut std::ffi::c_void) -> std::ffi::c_int {
    DECK_FAR.load(Ordering::Relaxed) as std::ffi::c_int
}

/// Publishes the datarefs; called from `XPluginEnable` so they exist before
/// the aircraft's objects look their hide datarefs up.
pub fn register() {
    if REGISTERED.swap(true, Ordering::Relaxed) {
        return;
    }
    let [deck, cabin, unseen, far] = NAMES.get_or_init(|| {
        [
            std::ffi::CString::new("fbw/anim/eye_in_flight_deck").unwrap_or_default(),
            std::ffi::CString::new("fbw/anim/cabin_hidden").unwrap_or_default(),
            std::ffi::CString::new("fbw/anim/deck_unseen").unwrap_or_default(),
            std::ffi::CString::new("fbw/anim/deck_far").unwrap_or_default(),
        ]
    });
    let types = crate::xp::DATA_TYPE_INT | crate::xp::DATA_TYPE_FLOAT;
    let deck_acc = crate::xp::Accessors { get_i: Some(in_deck_i), get_f: Some(in_deck_f), ..Default::default() };
    let cabin_acc = crate::xp::Accessors { get_i: Some(cabin_i), get_f: Some(cabin_f), ..Default::default() };
    let unseen_acc = crate::xp::Accessors { get_i: Some(unseen_i), get_f: Some(unseen_f), ..Default::default() };
    let _ = crate::xp::register_owned_accessor(deck, types, false, &deck_acc, std::ptr::null_mut());
    let _ = crate::xp::register_owned_accessor(cabin, types, false, &cabin_acc, std::ptr::null_mut());
    let far_acc = crate::xp::Accessors { get_i: Some(far_i), get_f: Some(far_f), ..Default::default() };
    let _ = crate::xp::register_owned_accessor(unseen, types, false, &unseen_acc, std::ptr::null_mut());
    let _ = crate::xp::register_owned_accessor(far, types, false, &far_acc, std::ptr::null_mut());
}

/// `xp::unregister_owned` removed them; the next `register` publishes them again.
pub fn forget() {
    REGISTERED.store(false, Ordering::Relaxed);
    IN_FLIGHT_DECK.store(false, Ordering::Relaxed);
    CABIN_HIDDEN.store(false, Ordering::Relaxed);
    DECK_UNSEEN.store(false, Ordering::Relaxed);
    DECK_FAR.store(false, Ordering::Relaxed);
}

/// Whether the eye is in the flight deck, as last published.
pub fn eye_in_flight_deck() -> bool {
    IN_FLIGHT_DECK.load(Ordering::Relaxed)
}

fn in_flight_deck(inside: bool, head_z: f32, default_eye_z: Option<f32>) -> bool {
    inside && default_eye_z.is_some_and(|eye| head_z - eye < AFT_OF_DEFAULT_EYE_M)
}

fn cabin_hidden(in_deck: bool, door: Option<f32>) -> bool {
    in_deck && door.is_some_and(|d| d < DOOR_SHUT_BELOW)
}

/// The default eye in OpenGL coordinates: the aircraft position plus the eye's body offset
/// (x right, y up, z aft) turned by heading and pitch.
fn deck_point(plane: [f32; 3], psi_deg: f32, theta_deg: f32, eye: [f32; 3]) -> [f32; 3] {
    let (sp, cp) = psi_deg.to_radians().sin_cos();
    let (st, ct) = theta_deg.to_radians().sin_cos();
    let fwd = -eye[2];
    let up = eye[1] * ct + fwd * st;
    let along = fwd * ct - eye[1] * st;
    [plane[0] + along * sp + eye[0] * cp, plane[1] + up, plane[2] - along * cp + eye[0] * sp]
}

fn deck_beyond(in_deck: bool, camera: [f32; 3], deck: [f32; 3], range: f32) -> bool {
    let d2: f32 = (0..3).map(|i| (camera[i] - deck[i]).powi(2)).sum();
    !in_deck && d2 > range * range
}

fn deck_unseen(in_deck: bool, camera: [f32; 3], deck: [f32; 3]) -> bool {
    deck_beyond(in_deck, camera, deck, DECK_DETAIL_RANGE_M)
}

#[derive(Default)]
pub struct FlightDeckView {
    external: Option<Option<DataRef>>,
    head_z: Option<Option<DataRef>>,
    door: Option<Option<DataRef>>,
    default_eye_z: Option<f32>,
    camera: Option<[Option<DataRef>; 9]>,
    default_eye: Option<[f32; 3]>,
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
        let refs = *self.camera.get_or_insert_with(|| {
            [
                "sim/graphics/view/view_x",
                "sim/graphics/view/view_y",
                "sim/graphics/view/view_z",
                "sim/flightmodel/position/local_x",
                "sim/flightmodel/position/local_y",
                "sim/flightmodel/position/local_z",
                "sim/flightmodel/position/psi",
                "sim/flightmodel/position/theta",
                "sim/graphics/view/pilots_head_x",
            ]
            .map(|n| xplm.find(n))
        });
        if inside && self.default_eye.is_none() {
            let y = xplm.find("sim/graphics/view/pilots_head_y").map(|d| xplm.get_f(d));
            if let (Some(x), Some(y)) = (refs[8].map(|d| xplm.get_f(d)), y) {
                self.default_eye = Some([x, y, z]);
            }
        }
        let read = |i: usize| refs[i].map(|d| xplm.get_f(d));
        let (unseen, far) = match (self.default_eye, read(0), read(1), read(2), read(3), read(4), read(5), read(6), read(7)) {
            (Some(eye), Some(cx), Some(cy), Some(cz), Some(px), Some(py), Some(pz), Some(psi), Some(theta)) => {
                let point = deck_point([px, py, pz], psi, theta, eye);
                (deck_unseen(deck, [cx, cy, cz], point), deck_beyond(deck, [cx, cy, cz], point, DECK_FAR_RANGE_M))
            }
            _ => (false, false),
        };
        DECK_UNSEEN.store(unseen, Ordering::Relaxed);
        DECK_FAR.store(far, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::{cabin_hidden, deck_beyond, deck_point, deck_unseen, in_flight_deck, DECK_FAR_RANGE_M};

    #[test]
    fn the_deck_point_sits_ahead_of_the_aircraft_along_its_heading() {
        let eye = [0.0, 3.0, -30.0];
        let north = deck_point([0.0, 0.0, 0.0], 0.0, 0.0, eye);
        assert!((north[2] + 30.0).abs() < 1e-3 && north[0].abs() < 1e-3 && (north[1] - 3.0).abs() < 1e-3);
        let east = deck_point([100.0, 0.0, 0.0], 90.0, 0.0, eye);
        assert!((east[0] - 130.0).abs() < 1e-3 && east[2].abs() < 1e-3);
        let nose_up = deck_point([0.0, 0.0, 0.0], 0.0, 10.0, eye);
        assert!(nose_up[1] > 3.0 + 4.0, "pitching up lifts the flight deck");
    }

    #[test]
    fn the_deck_is_unseen_only_from_beyond_its_range_and_never_from_inside_it() {
        let deck = [0.0, 3.0, -30.0];
        assert!(deck_unseen(false, [0.0, 10.0, 40.0], deck), "chase view behind the tail");
        assert!(!deck_unseen(false, [5.0, 4.0, -40.0], deck), "looking in through the windscreen");
        assert!(!deck_unseen(true, [0.0, 10.0, 40.0], deck), "the eye in the flight deck always sees it");
        assert!(deck_unseen(false, [0.0, 3.0, 0.0], deck), "far back in the cabin");
        assert!(!deck_beyond(false, [0.0, 10.0, 0.0], deck, DECK_FAR_RANGE_M), "30 m off: the deck still reads through the windscreen");
        assert!(deck_beyond(false, [0.0, 10.0, 60.0], deck, DECK_FAR_RANGE_M), "90 m behind");
    }

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

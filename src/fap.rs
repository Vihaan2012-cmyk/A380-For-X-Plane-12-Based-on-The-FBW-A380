//! The cabin Flight Attendant Panels' controls (html_ui A380X/FAP): the
//! page writes `L:A380X_FAP_*` variables and this module turns them into
//! what the cabin crew's panel really commands — pre-recorded
//! announcements and boarding music, passenger door operation, the cabin
//! lighting scene and dimming, and the cabin equipment switches the deep
//! cabin model reads (`deep::cabin::live`).

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::Vars;

pub const DOORS: [&str; 18] = ["M1L", "M1R", "M2L", "M2R", "M3L", "M3R", "M4L", "M4R", "M5L", "M5R", "U1L", "U1R", "U2L", "U2R", "U3L", "U3R", "CARGO_FWD", "CARGO_AFT"];
pub const ZONES: usize = 3;

static PA_REQUEST: AtomicI32 = AtomicI32::new(-1);
static SCENE: AtomicI32 = AtomicI32::new(0);
static LEVEL_PERCENT: AtomicI32 = AtomicI32::new(0);
static SEAT_POWER_OFF: AtomicBool = AtomicBool::new(false);
static HEATERS_OFF: AtomicBool = AtomicBool::new(false);
static GALLEY_OFF: AtomicU32 = AtomicU32::new(0);
static CALL_RESET: AtomicBool = AtomicBool::new(false);

/// A FAP announcement request since the last call: `Some(0)` stops the
/// PA, `Some(n)` plays announcement `n` (the `fbw/snd/pa` step).
pub fn take_pa_request() -> Option<u8> {
    let v = PA_REQUEST.swap(-1, Ordering::Relaxed);
    (v >= 0).then(|| v.clamp(0, 11) as u8)
}

/// The lighting scene the FAP selected: `None` leaves the EFB setting in
/// charge, `Some(0)` is automatic by flight phase, `Some(n)` scene `n`.
pub fn scene() -> Option<u8> {
    match SCENE.load(Ordering::Relaxed) {
        0 => None,
        12 => Some(0),
        n => Some(n.clamp(1, 11) as u8),
    }
}

/// The FAP's dimming as a fraction, or `None` when it has not set one.
pub fn level() -> Option<f64> {
    match LEVEL_PERCENT.load(Ordering::Relaxed) {
        0 => None,
        p => Some(((p - 1) as f64 / 100.).clamp(0., 1.)),
    }
}

/// A CALL RESET pressed on the FAP since the last call.
pub fn take_call_reset() -> bool {
    CALL_RESET.swap(false, Ordering::Relaxed)
}

pub fn seat_power_on() -> bool {
    !SEAT_POWER_OFF.load(Ordering::Relaxed)
}

pub fn water_heaters_on() -> bool {
    !HEATERS_OFF.load(Ordering::Relaxed)
}

pub fn galley_on(zone: usize) -> bool {
    GALLEY_OFF.load(Ordering::Relaxed) & (1 << zone) == 0
}

pub struct Fap {
    pa_cmd: VariableIdentifier,
    pa_seq: VariableIdentifier,
    door: VariableIdentifier,
    door_seq: VariableIdentifier,
    scene: VariableIdentifier,
    level: VariableIdentifier,
    seat_power_off: VariableIdentifier,
    pax_sys_off: VariableIdentifier,
    heaters_off: VariableIdentifier,
    galley_off: [VariableIdentifier; ZONES],
    door_refused: VariableIdentifier,
    call_reset_seq: VariableIdentifier,
    cabin_ready_seq: VariableIdentifier,
    cabin_ready: VariableIdentifier,
    engine_state: [VariableIdentifier; 4],
    stationary: VariableIdentifier,
    seen: Option<(f64, f64, f64, f64)>,
}

/// A passenger or cargo door may be worked from the FAP only with the
/// aircraft stopped and every engine shut down.
pub fn door_allowed(stationary: bool, engine_states: [f64; 4]) -> bool {
    stationary && engine_states.iter().all(|s| *s == 0.)
}

impl Fap {
    pub fn new(vars: &mut Vars) -> Self {
        let mut l = |n: &str| vars.get_unprefixed(format!("A380X_FAP_{n}"));
        let ids = (
            l("PA_CMD"),
            l("PA_SEQ"),
            l("DOOR"),
            l("DOOR_SEQ"),
            l("CABIN_SCENE"),
            l("CABIN_LEVEL"),
            l("SEAT_POWER_OFF"),
            l("WATER_HEATERS_OFF"),
            [l("GALLEY_OFF_1"), l("GALLEY_OFF_2"), l("GALLEY_OFF_3")],
            l("DOOR_REFUSED"),
            l("CALL_RESET_SEQ"),
            l("CABIN_READY_SEQ"),
            l("PAX_SYS_OFF"),
        );
        Self {
            pa_cmd: ids.0,
            pa_seq: ids.1,
            door: ids.2,
            door_seq: ids.3,
            scene: ids.4,
            level: ids.5,
            seat_power_off: ids.6,
            heaters_off: ids.7,
            galley_off: ids.8,
            door_refused: ids.9,
            call_reset_seq: ids.10,
            cabin_ready_seq: ids.11,
            pax_sys_off: ids.12,
            cabin_ready: vars.get("CABIN_READY".into()),
            engine_state: std::array::from_fn(|n| vars.get(format!("ENGINE_STATE:{}", n + 1))),
            stationary: vars.get("IS_STATIONARY".into()),
            seen: None,
        }
    }

    pub fn update(&mut self, vars: &mut Vars) {
        let pa_seq = vars.read(&self.pa_seq);
        let door_seq = vars.read(&self.door_seq);
        let call_seq = vars.read(&self.call_reset_seq);
        let ready_seq = vars.read(&self.cabin_ready_seq);
        if let Some((pa_was, door_was, call_was, ready_was)) = self.seen {
            if call_seq != call_was {
                CALL_RESET.store(true, Ordering::Relaxed);
            }
            if ready_seq != ready_was {
                vars.write(&self.cabin_ready, 1.);
            }
            if pa_seq != pa_was {
                PA_REQUEST.store(vars.read(&self.pa_cmd).round() as i32, Ordering::Relaxed);
            }
            if door_seq != door_was {
                let index = vars.read(&self.door).round() as i64 - 1;
                let engines = self.engine_state.map(|id| vars.read(&id));
                let stationary = vars.read(&self.stationary) != 0.;
                match DOORS.get(index.max(-1) as usize) {
                    Some(name) if door_allowed(stationary, engines) => {
                        if let Err(e) = crate::published::press_named(&format!("fbw/door/{name}/toggle")) {
                            crate::log(&format!("fap: door {name}: {e}"));
                        }
                        vars.write(&self.door_refused, 0.);
                    }
                    Some(_) => vars.write(&self.door_refused, door_seq),
                    None => {}
                }
            }
        }
        self.seen = Some((pa_seq, door_seq, call_seq, ready_seq));
        SCENE.store(vars.read(&self.scene).round() as i32, Ordering::Relaxed);
        LEVEL_PERCENT.store(vars.read(&self.level).round() as i32, Ordering::Relaxed);
        SEAT_POWER_OFF.store(vars.read(&self.seat_power_off) > 0.5 || vars.read(&self.pax_sys_off) > 0.5, Ordering::Relaxed);
        HEATERS_OFF.store(vars.read(&self.heaters_off) > 0.5, Ordering::Relaxed);
        let mut galley = 0;
        for (z, id) in self.galley_off.iter().enumerate() {
            if vars.read(id) > 0.5 {
                galley |= 1 << z;
            }
        }
        GALLEY_OFF.store(galley, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doors_work_only_stopped_with_every_engine_off() {
        assert!(door_allowed(true, [0.; 4]));
        assert!(!door_allowed(false, [0.; 4]));
        assert!(!door_allowed(true, [0., 0., 1., 0.]));
        assert!(!door_allowed(true, [2., 0., 0., 0.]), "not while an engine is starting");
    }

    #[test]
    fn the_scene_and_level_encodings_leave_the_efb_in_charge_at_zero() {
        SCENE.store(0, Ordering::Relaxed);
        LEVEL_PERCENT.store(0, Ordering::Relaxed);
        assert_eq!((scene(), level()), (None, None));
        SCENE.store(12, Ordering::Relaxed);
        LEVEL_PERCENT.store(26, Ordering::Relaxed);
        assert_eq!(scene(), Some(0), "AUTO");
        assert_eq!(level(), Some(0.25));
        SCENE.store(7, Ordering::Relaxed);
        LEVEL_PERCENT.store(1, Ordering::Relaxed);
        assert_eq!((scene(), level()), (Some(7), Some(0.)));
        SCENE.store(0, Ordering::Relaxed);
        LEVEL_PERCENT.store(0, Ordering::Relaxed);
    }

    #[test]
    fn every_door_name_matches_a_door() {
        for (i, name) in DOORS.iter().enumerate() {
            assert_eq!(crate::doors::NAMES[i], *name);
        }
    }
}

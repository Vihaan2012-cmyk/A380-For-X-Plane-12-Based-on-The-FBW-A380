//! The cabin crew's door slides: every passenger door's arming lever
//! (`ANIM_DOOR_<door>_DISARMED`, 1 disarmed, 0 armed, which the cabin
//! model's lever animation reads and its click toggles) is armed once all passenger doors are
//! closed and the aircraft is about to move (beacon on, an engine running,
//! moving, or airborne), and disarmed at the stand after shutdown (stopped,
//! beacon off, engines off). The crew work door by door, front to back; a
//! lever moved by hand stays where it was put until the next arm or disarm.

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::xp::{DataRef, Xplm};
use crate::Vars;

const DOORS: [&str; 16] = ["M1L", "M1R", "M2L", "M2R", "M3L", "M3R", "M4L", "M4R", "M5L", "M5R", "U1L", "U1R", "U2L", "U2R", "U3L", "U3R"];
const LEVER_PER_S: f64 = 0.6;
const DOOR_GAP_S: f64 = 0.8;
pub const DISARMED: f64 = 1.0;
pub const ARMED: f64 = 0.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Inputs {
    pub doors_closed: bool,
    pub beacon: bool,
    pub engine_running: bool,
    pub on_ground: bool,
    pub gs_kt: f64,
}

pub fn wants_armed(i: &Inputs, armed: bool) -> bool {
    if i.doors_closed && (i.beacon || i.engine_running || i.gs_kt > 2.0 || !i.on_ground) {
        return true;
    }
    if i.on_ground && i.gs_kt < 1.0 && !i.beacon && !i.engine_running {
        return false;
    }
    armed
}

pub struct Levers {
    pub position: [f64; 16],
    held: [bool; 16],
    target: f64,
    since: f64,
}

impl Levers {
    pub fn new(armed: bool) -> Self {
        let p = if armed { ARMED } else { DISARMED };
        Self { position: [p; 16], held: [false; 16], target: p, since: 0.0 }
    }

    pub fn step(&mut self, armed: bool, dt: f64) {
        let target = if armed { ARMED } else { DISARMED };
        if target != self.target {
            self.target = target;
            self.since = 0.0;
            self.held = [false; 16];
        }
        self.since += dt;
        for (i, p) in self.position.iter_mut().enumerate() {
            if self.held[i] || self.since < i as f64 * DOOR_GAP_S {
                continue;
            }
            let step = LEVER_PER_S * dt;
            *p = if (target - *p).abs() <= step { target } else { *p + step * (target - *p).signum() };
        }
    }

    pub fn set_by_hand(&mut self, door: usize, value: f64) {
        self.position[door] = value.clamp(ARMED, DISARMED);
        self.held[door] = true;
    }

    pub fn armed_count(&self) -> usize {
        self.position.iter().filter(|p| **p < 0.5).count()
    }
}

struct Refs {
    beacon: Option<DataRef>,
    running: Option<DataRef>,
    on_ground: Option<DataRef>,
    gs: Option<DataRef>,
}

pub struct CabinCrew {
    open: [VariableIdentifier; 16],
    levers_ids: [VariableIdentifier; 16],
    refs: Refs,
    levers: Option<Levers>,
    armed: bool,
    written: bool,
}

impl CabinCrew {
    pub fn new(vars: &mut Vars, xplm: &Xplm) -> Self {
        let f = |n: &str| xplm.find(n);
        Self {
            open: std::array::from_fn(|i| vars.get(format!("INTERACTIVE POINT OPEN:{i}"))),
            levers_ids: DOORS.map(|d| vars.get_unprefixed(format!("ANIM_DOOR_{d}_DISARMED"))),
            refs: Refs {
                beacon: f("sim/cockpit/electrical/beacon_lights_on"),
                running: f("sim/flightmodel/engine/ENGN_running"),
                on_ground: f("sim/flightmodel/failures/onground_any"),
                gs: f("sim/flightmodel/position/groundspeed"),
            },
            levers: None,
            armed: false,
            written: false,
        }
    }

    pub fn update(&mut self, vars: &mut Vars, xplm: &Xplm, dt: f64) {
        let mut running = [0i32; 4];
        if let Some(d) = self.refs.running {
            xplm.get_vi(d, &mut running);
        }
        let i = Inputs {
            doors_closed: self.open.iter().all(|id| vars.read(id) < 1.0),
            beacon: self.refs.beacon.is_some_and(|d| xplm.get_i(d) != 0),
            engine_running: running.iter().any(|r| *r != 0),
            on_ground: self.refs.on_ground.is_some_and(|d| xplm.get_i(d) != 0),
            gs_kt: self.refs.gs.map_or(0.0, |d| xplm.get_f(d) as f64) * 1.943_844,
        };
        let levers = self.levers.get_or_insert_with(|| {
            self.armed = !i.on_ground || (i.doors_closed && (i.beacon || i.engine_running));
            Levers::new(self.armed)
        });
        for (n, id) in self.levers_ids.iter().enumerate() {
            let now = vars.read(id);
            if (now - levers.position[n]).abs() > 0.01 && self.written {
                levers.set_by_hand(n, now);
            }
        }
        self.armed = wants_armed(&i, self.armed);
        levers.step(self.armed, dt.clamp(0.0, 0.5));
        self.written = true;
        for (id, p) in self.levers_ids.iter().zip(levers.position) {
            vars.write(id, p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slides_arm_for_departure_and_disarm_at_the_stand() {
        let gate = Inputs { doors_closed: false, on_ground: true, ..Default::default() };
        assert!(!wants_armed(&gate, false));
        let closed = Inputs { doors_closed: true, ..gate };
        assert!(!wants_armed(&closed, false), "doors closed alone is not departure");
        let beacon = Inputs { beacon: true, ..closed };
        assert!(wants_armed(&beacon, false));
        let taxi_in = Inputs { beacon: true, engine_running: true, gs_kt: 12.0, ..closed };
        assert!(wants_armed(&taxi_in, true));
        let parked_running = Inputs { beacon: true, engine_running: true, gs_kt: 0.0, ..closed };
        assert!(wants_armed(&parked_running, true), "still armed until shutdown");
        let shut_down = Inputs { beacon: false, engine_running: false, gs_kt: 0.0, ..closed };
        assert!(!wants_armed(&shut_down, true));
        let open_in_flight = Inputs { doors_closed: true, on_ground: false, ..Default::default() };
        assert!(wants_armed(&open_in_flight, false));
    }

    #[test]
    fn the_crew_work_door_by_door() {
        let mut l = Levers::new(false);
        l.step(true, 0.1);
        assert!(l.position[0] < DISARMED && l.position[15] == DISARMED);
        for _ in 0..300 {
            l.step(true, 0.1);
        }
        assert_eq!(l.armed_count(), 16);
        assert!(l.position.iter().all(|p| *p == ARMED));
        for _ in 0..300 {
            l.step(false, 0.1);
        }
        assert_eq!(l.armed_count(), 0);
    }

    #[test]
    fn a_lever_moved_by_hand_stays_until_the_next_change() {
        let mut l = Levers::new(true);
        l.set_by_hand(3, DISARMED);
        for _ in 0..50 {
            l.step(true, 0.1);
        }
        assert_eq!(l.position[3], DISARMED);
        assert_eq!(l.armed_count(), 15);
        for _ in 0..300 {
            l.step(false, 0.1);
        }
        for _ in 0..300 {
            l.step(true, 0.1);
        }
        assert_eq!(l.armed_count(), 16);
    }
}

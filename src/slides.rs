//! Escape slides (objects/a380_evac_slides.obj): a passenger door opened
//! with its slide armed (`ANIM_DOOR_<door>_DISARMED` 0) fires the slide,
//! which inflates in about six seconds and stays out until it is repacked
//! (`fbw/slides/repack`). Doors opened by a jet bridge or a service vehicle
//! do not fire.

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::published::{self, Command, Published, Value};
use crate::Vars;

pub const DOORS: [&str; 16] = ["M1L", "M1R", "M2L", "M2R", "M3L", "M3R", "M4L", "M4R", "M5L", "M5R", "U1L", "U1R", "U2L", "U2R", "U3L", "U3R"];
pub const INFLATE_S: f64 = 6.0;
const FIRE_OPEN_PERCENT: f64 = 5.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub fired: [bool; 16],
    pub deploy: [f64; 16],
    was_open: [f64; 16],
}

impl State {
    /// One tick: `open` percent per door, `armed` per door, `by_service`
    /// when the opening was the ground equipment's or a jet bridge's.
    pub fn step(&mut self, open: [f64; 16], armed: [bool; 16], by_service: [bool; 16], dt: f64) -> Vec<usize> {
        let mut fired = Vec::new();
        for i in 0..16 {
            let opening = open[i] > FIRE_OPEN_PERCENT && self.was_open[i] <= FIRE_OPEN_PERCENT;
            if opening && armed[i] && !by_service[i] && !self.fired[i] {
                self.fired[i] = true;
                fired.push(i);
            }
            self.was_open[i] = open[i];
            if self.fired[i] {
                self.deploy[i] = (self.deploy[i] + dt / INFLATE_S).min(1.);
            }
        }
        fired
    }

    pub fn repack(&mut self) {
        self.fired = [false; 16];
        self.deploy = [0.; 16];
    }
}

pub struct Slides {
    state: State,
    open: [VariableIdentifier; 16],
    levers: [VariableIdentifier; 16],
    values: [Value; 16],
    repack: Command,
    primed: bool,
    _published: Published,
}

impl Slides {
    pub fn new(vars: &mut Vars) -> Self {
        let mut p = Published::default();
        let values = DOORS.map(|d| p.number(&format!("fbw/slides/{d}/deploy"), 0., false));
        let repack = p.command("fbw/slides/repack", "Repack every deployed escape slide");
        Self {
            state: State::default(),
            open: std::array::from_fn(|i| vars.get(format!("INTERACTIVE POINT OPEN:{i}"))),
            levers: DOORS.map(|d| vars.get_unprefixed(format!("ANIM_DOOR_{d}_DISARMED"))),
            values,
            repack,
            primed: false,
            _published: p,
        }
    }

    pub fn update(&mut self, vars: &mut Vars, by_service: impl Fn(usize) -> bool, dt: f64) {
        if published::presses(self.repack) > 0 {
            self.state.repack();
            crate::log("slides: repacked");
        }
        let open = self.open.map(|id| vars.read(&id));
        if !self.primed {
            self.primed = true;
            self.state.was_open = open;
        }
        let armed = self.levers.map(|id| vars.read(&id) < 0.5);
        let service: [bool; 16] = std::array::from_fn(&by_service);
        for i in self.state.step(open, armed, service, dt) {
            crate::log(&format!("slides: {} fired (door opened armed)", DOORS[i]));
        }
        for (v, d) in self.values.iter().zip(self.state.deploy) {
            published::set(*v, d);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_armed_door_fires_its_slide_and_it_stays_out_until_repacked() {
        let mut s = State::default();
        let mut open = [0.; 16];
        let mut armed = [false; 16];
        armed[2] = true;
        assert!(s.step(open, armed, [false; 16], 1.).is_empty());
        open[2] = 20.;
        open[3] = 20.;
        assert_eq!(s.step(open, armed, [false; 16], 1.), vec![2], "only the armed door");
        for _ in 0..10 {
            s.step(open, armed, [false; 16], 1.);
        }
        assert_eq!(s.deploy[2], 1.);
        assert_eq!(s.deploy[3], 0.);
        open[2] = 0.;
        armed[2] = false;
        s.step(open, armed, [false; 16], 1.);
        assert_eq!(s.deploy[2], 1., "a fired slide stays out when the door closes");
        s.repack();
        assert_eq!(s.deploy, [0.; 16]);
    }

    #[test]
    fn inflation_takes_six_seconds() {
        let mut s = State::default();
        let mut open = [0.; 16];
        let armed = [true; 16];
        s.step(open, armed, [false; 16], 0.1);
        open[0] = 50.;
        s.step(open, armed, [false; 16], 3.);
        assert!((s.deploy[0] - 0.5).abs() < 1e-9);
        s.step(open, armed, [false; 16], 3.);
        assert_eq!(s.deploy[0], 1.);
    }

    #[test]
    fn a_door_opened_by_ground_equipment_or_already_open_does_not_fire() {
        let mut s = State::default();
        let armed = [true; 16];
        let mut open = [0.; 16];
        open[0] = 100.;
        let mut service = [false; 16];
        service[0] = true;
        s.step([0.; 16], armed, service, 0.1);
        assert!(s.step(open, armed, service, 0.1).is_empty());
        let mut t = State::default();
        t.was_open = [100.; 16];
        assert!(t.step([100.; 16], armed, [false; 16], 0.1).is_empty(), "arming a door that is already open does not fire it");
    }
}

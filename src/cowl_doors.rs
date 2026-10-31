//! Fan cowl doors opened for maintenance, as the ground crew does on the
//! real aircraft: each engine's two doors swing up about their hinges beside
//! the pylon and then the thrust reverser halves behind them
//! (`fbw/anim/cowl_open_<n>`: doors over 0..0.5, reverser halves 0.5..1),
//! uncovering the fan case and its accessories (`fbw/anim/engine_bay_<n>`,
//! also on while the cowl has separated). `fbw/engine/cowl_<n>_toggle` opens
//! or shuts them; the crew only does that parked with the engine stopped.
//! Doors left open come off once the aircraft rolls faster than
//! [`TEAR_OFF_SPEED_MS`] (cowl separation, 54_010+n).

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::failures::extra::COWL_SEPARATION;
use crate::published::{self, Command, Published, Value};
use crate::xp::{DataRef, Xplm};
use crate::Vars;

pub const OPEN_TIME_S: f64 = 12.0;
pub const PARKED_SPEED_MS: f64 = 0.5;
pub const STOPPED_N2_PERCENT: f64 = 5.0;
pub const TEAR_OFF_SPEED_MS: f64 = 15.0;

pub fn mirror_name(engine: usize) -> String {
    format!("{}ENGINE_COWL_OPEN_{}", crate::NAME_PREFIX, engine + 1)
}

pub fn may_operate(on_ground: bool, groundspeed_ms: f64, n2_percent: f64) -> bool {
    on_ground && groundspeed_ms.abs() < PARKED_SPEED_MS && n2_percent < STOPPED_N2_PERCENT
}

pub fn step(position: f64, target: bool, delta: f64) -> f64 {
    let rate = delta.max(0.0) / OPEN_TIME_S;
    if target {
        (position + rate).min(1.0)
    } else {
        (position - rate).max(0.0)
    }
}

pub fn torn_off(position: f64, groundspeed_ms: f64) -> bool {
    position > 0.05 && groundspeed_ms.abs() > TEAR_OFF_SPEED_MS
}

pub struct CowlDoors {
    _published: Published,
    open: [Value; 4],
    bay: [Value; 4],
    toggles: [Command; 4],
    target: [bool; 4],
    position: [f64; 4],
    bay_shown: [bool; 4],
    n2: [VariableIdentifier; 4],
    mirror: [VariableIdentifier; 4],
    on_ground: Option<DataRef>,
    groundspeed: Option<DataRef>,
}

impl CowlDoors {
    pub fn new(vars: &mut Vars, xplm: &Xplm) -> Self {
        let mut p = Published::default();
        let open = std::array::from_fn(|e| p.number(&format!("fbw/anim/cowl_open_{}", e + 1), 0.0, false));
        let bay = std::array::from_fn(|e| p.number(&format!("fbw/anim/engine_bay_{}", e + 1), 0.0, false));
        let toggles = std::array::from_fn(|e| p.command(&format!("fbw/engine/cowl_{}_toggle", e + 1), &format!("Open or shut engine {}'s fan cowl doors", e + 1)));
        Self {
            _published: p,
            open,
            bay,
            toggles,
            target: [false; 4],
            position: [0.0; 4],
            bay_shown: [false; 4],
            n2: std::array::from_fn(|e| vars.get(format!("ENGINE_N2:{}", e + 1))),
            mirror: std::array::from_fn(|e| vars.get(mirror_name(e))),
            on_ground: xplm.find("sim/flightmodel/failures/onground_any"),
            groundspeed: xplm.find("sim/flightmodel/position/groundspeed"),
        }
    }

    pub fn update(&mut self, vars: &mut Vars, xplm: &Xplm, delta: f64) {
        let on_ground = self.on_ground.is_some_and(|d| xplm.get_i(d) != 0);
        let groundspeed = self.groundspeed.map_or(0.0, |d| xplm.get_f(d) as f64);
        for e in 0..4 {
            let separated = crate::failures::is_active(COWL_SEPARATION + e as u64);
            let n2 = vars.read(&self.n2[e]);
            for _ in 0..published::presses(self.toggles[e]) {
                if separated {
                    crate::notices::post(format!("Engine {} fan cowl: it has separated, there is nothing left to open", e + 1));
                } else if may_operate(on_ground, groundspeed, n2) {
                    self.target[e] = !self.target[e];
                    crate::log(&format!("cowl doors: engine {} {}", e + 1, if self.target[e] { "opening" } else { "closing" }));
                } else if !on_ground || groundspeed.abs() >= PARKED_SPEED_MS {
                    crate::notices::post(format!("Engine {} fan cowl: only when parked on the ground ({:.0} kt)", e + 1, groundspeed * 1.943_844));
                } else {
                    crate::notices::post(format!("Engine {} fan cowl: shut the engine down first (N2 {:.0}%)", e + 1, n2));
                }
            }
            if separated {
                self.target[e] = false;
                self.position[e] = 0.0;
            } else if torn_off(self.position[e], groundspeed) {
                crate::log(&format!("cowl doors: engine {} fan cowl doors left open torn off at {:.0} kt", e + 1, groundspeed * 1.943_844));
                crate::failures::set_active(COWL_SEPARATION + e as u64, true);
                self.target[e] = false;
                self.position[e] = 0.0;
            } else {
                self.position[e] = step(self.position[e], self.target[e], delta);
            }
            published::set(self.open[e], self.position[e]);
            let bay = separated || self.position[e] > 0.0;
            if bay != self.bay_shown[e] {
                self.bay_shown[e] = bay;
                crate::log(&format!("cowl doors: engine {} bay {} (cowls separated {separated}, doors {:.2})", e + 1, if bay { "shown" } else { "hidden" }, self.position[e]));
            }
            published::set(self.bay[e], bay as i32 as f64);
            vars.write(&self.mirror[e], self.position[e]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_doors_take_their_full_time_to_open_and_shut() {
        let mut p = 0.0;
        for _ in 0..30 {
            p = step(p, true, 0.1);
        }
        assert!((p - 0.25).abs() < 1e-9);
        for _ in 0..200 {
            p = step(p, true, 0.1);
        }
        assert_eq!(p, 1.0);
        for _ in 0..200 {
            p = step(p, false, 0.1);
        }
        assert_eq!(p, 0.0);
    }

    #[test]
    fn only_a_parked_aircraft_with_the_engine_stopped_gets_its_cowls_opened() {
        assert!(may_operate(true, 0.0, 0.0));
        assert!(!may_operate(false, 0.0, 0.0));
        assert!(!may_operate(true, 2.0, 0.0));
        assert!(!may_operate(true, 0.0, 22.0));
    }

    #[test]
    fn open_doors_survive_a_tow_but_not_a_takeoff_roll() {
        assert!(!torn_off(1.0, 2.0));
        assert!(torn_off(1.0, 40.0));
        assert!(!torn_off(0.0, 80.0));
    }
}

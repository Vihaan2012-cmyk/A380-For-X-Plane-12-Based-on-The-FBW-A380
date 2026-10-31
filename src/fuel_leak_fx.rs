//! Visible fuel leaks: `objects/a380_brake_fire.obj`'s emitters per tank and
//! pylon stub (`fuel_leak_particles.py`), fed by what the fuel models actually
//! lose: `deep::fuel`'s tank wall leaks and `fuel.rs`'s punctures, torn tanks
//! and torn pylon lines. Mist and drips while slow, a trailing plume at speed.

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::published::{self, Published, Value};
use crate::xp::{DataRef, Xplm};
use crate::Vars;

const TANKS: usize = 11;
const PYLONS: usize = 4;
const BIG_LEAK_KG_S: f64 = 1.0;
const PLUME_TAS_M_S: f64 = 30.0;

/// `fbw/damage/fuel_leak_N`: 0 none, 1 and 2 small and big while slow, 3 and 4 at speed.
pub fn leak_state(kg_s: f64, tas_m_s: f64) -> f64 {
    if kg_s <= 0. {
        return 0.;
    }
    let size = if kg_s < BIG_LEAK_KG_S { 1. } else { 2. };
    if tas_m_s >= PLUME_TAS_M_S {
        size + 2.
    } else {
        size
    }
}

pub struct FuelLeakFx {
    _published: Published,
    out: [Value; TANKS + PYLONS],
    wall: [VariableIdentifier; TANKS],
    engine_line: [VariableIdentifier; PYLONS],
    tas: Option<DataRef>,
}

impl FuelLeakFx {
    pub fn new(vars: &mut Vars, xplm: &Xplm) -> Self {
        let mut p = Published::default();
        let out = std::array::from_fn(|n| p.number(&format!("fbw/damage/fuel_leak_{n}"), 0., false));
        Self {
            _published: p,
            out,
            wall: std::array::from_fn(|i| vars.get(format!("FUEL_TANK_LEAK_KG_S:{}", i + 1))),
            engine_line: std::array::from_fn(|i| vars.get(format!("FUEL_ENG_LEAK_DETECTED:{}", i + 1))),
            tas: xplm.find("sim/flightmodel/position/true_airspeed"),
        }
    }

    pub fn update(&mut self, vars: &mut Vars, xplm: &Xplm, fuel: Option<&crate::fuel::Fuel>) {
        let tas = self.tas.map_or(0., |d| xplm.get_f(d) as f64);
        let (tanks, lines) = fuel.map_or(([0.; TANKS], [0.; PYLONS]), |f| f.leaks_kg_s());
        for i in 0..TANKS {
            published::set(self.out[i], leak_state(tanks[i] + vars.read(&self.wall[i]).max(0.), tas));
        }
        for i in 0..PYLONS {
            let detected = if vars.read(&self.engine_line[i]) > 0.5 { BIG_LEAK_KG_S / 2. } else { 0. };
            published::set(self.out[TANKS + i], leak_state(lines[i] + detected, tas));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_fuel_lost_shows_nothing_and_size_follows_the_rate() {
        assert_eq!(leak_state(0., 0.), 0.);
        assert_eq!(leak_state(0., 200.), 0.);
        assert_eq!(leak_state(0.2, 0.), 1.);
        assert_eq!(leak_state(1.5, 0.), 2.);
    }

    #[test]
    fn at_speed_the_same_leak_trails_as_a_plume() {
        assert_eq!(leak_state(0.2, 100.), 3.);
        assert_eq!(leak_state(20., 100.), 4.);
    }
}

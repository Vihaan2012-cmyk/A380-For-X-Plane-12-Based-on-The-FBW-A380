//! Visible brake fires: `objects/a380_brake_fire.obj`'s fire and smoke
//! emitters per bogie show while any wheel brake on that bogie is on fire
//! (`deep::gear_structure`'s `BRAKE_FIRE:1..16`, grouped by
//! `LEG_WHEEL_INDICES`) and the leg is extended; inside a closed gear bay
//! nothing is visible from outside.

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::published::{self, Published, Value};
use crate::xp::{DataRef, Xplm};
use crate::Vars;

const BOGIES: [(&str, [usize; 4], usize); 4] = [
    ("lw", [0, 1, 4, 5], 3),
    ("rw", [2, 3, 6, 7], 4),
    ("lb", [8, 9, 12, 13], 1),
    ("rb", [10, 11, 14, 15], 2),
];

pub fn bogie_burning(fires: &[bool; 16], wheels: [usize; 4], deploy_ratio: f32) -> bool {
    deploy_ratio > 0.5 && wheels.iter().any(|w| fires[*w])
}

pub struct BrakeFireFx {
    _published: Published,
    out: [Value; 4],
    fires: [VariableIdentifier; 16],
    deploy: Option<DataRef>,
}

impl BrakeFireFx {
    pub fn new(vars: &mut Vars, xplm: &Xplm) -> Self {
        let mut p = Published::default();
        let out = BOGIES.map(|(name, _, _)| p.number(&format!("fbw/damage/brake_fire_{name}"), 0., false));
        Self {
            _published: p,
            out,
            fires: std::array::from_fn(|i| vars.get(format!("BRAKE_FIRE:{}", i + 1))),
            deploy: xplm.find("sim/flightmodel2/gear/deploy_ratio"),
        }
    }

    pub fn update(&mut self, vars: &mut Vars, xplm: &Xplm) {
        let fires: [bool; 16] = std::array::from_fn(|i| vars.read(&self.fires[i]) > 0.5);
        let mut deploy = [0f32; 5];
        if let Some(d) = self.deploy {
            xplm.get_vf(d, &mut deploy);
        }
        for (k, (_, wheels, gear)) in BOGIES.iter().enumerate() {
            published::set(self.out[k], if bogie_burning(&fires, *wheels, deploy[*gear]) { 1. } else { 0. });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bogie_burns_with_any_of_its_wheels_and_only_when_extended() {
        let mut fires = [false; 16];
        fires[13] = true;
        assert!(bogie_burning(&fires, BOGIES[2].1, 1.0));
        assert!(!bogie_burning(&fires, BOGIES[3].1, 1.0));
        assert!(!bogie_burning(&fires, BOGIES[2].1, 0.0));
        let all: Vec<usize> = BOGIES.iter().flat_map(|b| b.1).collect();
        let mut sorted = all.clone();
        sorted.sort();
        assert_eq!(sorted, (0..16).collect::<Vec<_>>(), "every braked wheel belongs to exactly one bogie");
    }
}

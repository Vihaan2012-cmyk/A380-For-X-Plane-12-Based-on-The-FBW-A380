//! Visible windshield cracks: each cockpit pane's crack overlay
//! (`objects/a380_windshield_cracks.obj`, made by
//! msfs2xp-aircraft/tools/optimize/windshield_cracks.py) shows once
//! `deep::environment` reports that pane cracked by a bird or by hail.

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::published::{self, Published, Value};
use crate::Vars;

pub const PANES: usize = 6;
pub const BIRD_CRACKED_FROM: f64 = 0.6;
pub const HAIL_CRACKED_FROM: f64 = 0.9;

pub fn cracked(bird: f64, hail: f64) -> bool {
    bird >= BIRD_CRACKED_FROM || hail >= HAIL_CRACKED_FROM
}

pub struct WindshieldCracksFx {
    _published: Published,
    panes: [Value; PANES],
    bird: [VariableIdentifier; PANES],
    hail: [VariableIdentifier; PANES],
}

impl WindshieldCracksFx {
    pub fn new(vars: &mut Vars) -> Self {
        let mut p = Published::default();
        Self {
            panes: std::array::from_fn(|i| p.number(&format!("fbw/damage/windshield_crack_{}", i + 1), 0., false)),
            _published: p,
            bird: std::array::from_fn(|i| vars.get(format!("ENV_BIRD_WINDSHIELD_DAMAGE:{}", i + 1))),
            hail: std::array::from_fn(|i| vars.get(format!("ENV_HAIL_WINDSHIELD_DAMAGE:{}", i + 1))),
        }
    }

    pub fn update(&mut self, vars: &mut Vars) {
        for i in 0..PANES {
            let on = cracked(vars.read(&self.bird[i]), vars.read(&self.hail[i]));
            published::set(self.panes[i], if on { 1. } else { 0. });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pane_shows_cracks_from_a_cracking_bird_or_heavy_hail() {
        assert!(!cracked(0.5, 0.0));
        assert!(cracked(0.6, 0.0));
        assert!(!cracked(0.0, 0.89));
        assert!(cracked(0.0, 0.9));
    }
}

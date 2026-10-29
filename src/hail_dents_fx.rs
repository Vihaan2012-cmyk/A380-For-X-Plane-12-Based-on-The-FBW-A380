//! Visible hail dents: the nose and wing skins swap to their dented
//! variants (`objects/*_hail.obj`, made by
//! msfs2xp-aircraft/tools/optimize/hail_dents.py) once `deep::environment`'s
//! hail damage there is past [`VISIBLE_FROM`]: the radome for the nose,
//! leading-edge segments 1-6 for the left wing and 7-12 for the right.

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::published::{self, Published, Value};
use crate::Vars;

pub const VISIBLE_FROM: f64 = 0.1;

pub fn dented(severities: &[f64]) -> bool {
    severities.iter().any(|s| *s >= VISIBLE_FROM)
}

pub struct HailDentsFx {
    _published: Published,
    nose: Value,
    wings: [Value; 2],
    radome: VariableIdentifier,
    leading_edge: [VariableIdentifier; 12],
}

impl HailDentsFx {
    pub fn new(vars: &mut Vars) -> Self {
        let mut p = Published::default();
        Self {
            nose: p.number("fbw/damage/hail_nose", 0., false),
            wings: [p.number("fbw/damage/hail_wing_l", 0., false), p.number("fbw/damage/hail_wing_r", 0., false)],
            _published: p,
            radome: vars.get("ENV_HAIL_RADOME_DAMAGE".into()),
            leading_edge: std::array::from_fn(|i| vars.get(format!("ENV_HAIL_WING_LE_DAMAGE:{}", i + 1))),
        }
    }

    pub fn update(&mut self, vars: &mut Vars) {
        let flag = |b: bool| if b { 1. } else { 0. };
        published::set(self.nose, flag(dented(&[vars.read(&self.radome)])));
        let le: Vec<f64> = self.leading_edge.iter().map(|id| vars.read(id)).collect();
        published::set(self.wings[0], flag(dented(&le[..6])));
        published::set(self.wings[1], flag(dented(&le[6..])));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dents_show_from_a_tenth_of_full_damage() {
        assert!(!dented(&[0.0, 0.09]));
        assert!(dented(&[0.0, 0.1]));
        assert!(!dented(&[]));
    }
}

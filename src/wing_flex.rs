use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::published::{self, Published, Value};

const SIDES: [&str; 2] = ["LEFT", "RIGHT"];
const JOINTS: [&str; 4] = ["INBOARD", "INBOARD_MID", "OUTBOARD_MID", "OUTBOARD"];
const LIMIT_DEG: f64 = 30.0;

pub fn dataref(side: usize, joint: usize) -> String {
    format!("fbw/anim/wing_flex/{}_{}", SIDES[side].to_lowercase(), JOINTS[joint].to_lowercase())
}

pub fn angle(raw: f64) -> f64 {
    if raw.is_finite() {
        raw.clamp(-LIMIT_DEG, LIMIT_DEG)
    } else {
        0.0
    }
}

const RESTING_SMOOTHING_S: f64 = 0.25;

pub fn smoothed(previous: f64, target: f64, dt: f64, resting: bool) -> f64 {
    if !resting || !previous.is_finite() {
        return target;
    }
    previous + (target - previous) * (dt / (RESTING_SMOOTHING_S + dt)).clamp(0., 1.)
}

pub struct WingFlexAnims {
    _published: Published,
    values: Vec<Value>,
    ids: Option<Vec<VariableIdentifier>>,
    shown: Vec<f64>,
}

impl WingFlexAnims {
    pub fn new() -> Self {
        let mut p = Published::default();
        let values = (0..2).flat_map(|s| (0..4).map(move |j| (s, j))).map(|(s, j)| p.number(&dataref(s, j), 0., false)).collect();
        Self { _published: p, values, ids: None, shown: vec![f64::NAN; 8] }
    }

    pub fn update<V: VariableRegistry + SimulatorReaderWriter>(&mut self, vars: &mut V, dt: f64, resting_on_airframe: bool) {
        let ids = self.ids.get_or_insert_with(|| {
            (0..2).flat_map(|s| (0..4).map(move |j| (s, j))).map(|(s, j)| vars.get(format!("WING_FLEX_{}_{}", SIDES[s], JOINTS[j]))).collect()
        });
        for ((value, id), shown) in self.values.iter().zip(ids.iter()).zip(self.shown.iter_mut()) {
            *shown = smoothed(*shown, angle(vars.read(id)), dt, resting_on_airframe);
            published::set(*value, *shown);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_match_the_wing_objects() {
        assert_eq!(dataref(0, 0), "fbw/anim/wing_flex/left_inboard");
        assert_eq!(dataref(1, 3), "fbw/anim/wing_flex/right_outboard");
        assert_eq!(dataref(0, 2), "fbw/anim/wing_flex/left_outboard_mid");
    }

    #[test]
    fn resting_on_the_airframe_smooths_the_bend_and_otherwise_it_passes_straight_through() {
        assert_eq!(smoothed(0.0, 2.0, 0.02, false), 2.0);
        assert_eq!(smoothed(f64::NAN, 2.0, 0.02, true), 2.0);
        let step = smoothed(0.0, 2.0, 0.02, true);
        assert!(step > 0.0 && step < 0.3, "{step}");
        let mut v = 0.0;
        for _ in 0..200 {
            v = smoothed(v, 2.0, 0.02, true);
        }
        assert!((v - 2.0).abs() < 1e-3, "settles on the real bend: {v}");
    }

    #[test]
    fn angles_are_bounded_and_never_nan() {
        assert_eq!(angle(f64::NAN), 0.0);
        assert_eq!(angle(90.0), LIMIT_DEG);
        assert_eq!(angle(-2.5), -2.5);
    }
}

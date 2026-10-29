use crate::types::AREAS;

pub const MIN_C: f64 = 18.;
pub const MAX_C: f64 = 30.;
pub const DEFAULT_C: f64 = 24.;

#[derive(Clone, Debug, Default)]
pub struct TempOut {
    pub setpoints: [f64; AREAS],
    pub purser_sel_c: f64,
    pub purser_active: bool,
}

pub struct Temp {
    setpoints: [f64; AREAS],
}

fn snap(x: f64) -> f64 {
    ((x * 2.).round() / 2.).clamp(MIN_C, MAX_C)
}

impl Temp {
    pub fn new() -> Self {
        Self { setpoints: [DEFAULT_C; AREAS] }
    }

    pub fn set(&mut self, area: usize, c: f64) {
        if area < AREAS && c.is_finite() {
            self.setpoints[area] = snap(c);
        }
    }

    pub fn step(&mut self, sel_fault: bool) -> TempOut {
        let mean = self.setpoints.iter().sum::<f64>() / AREAS as f64;
        TempOut { setpoints: self.setpoints, purser_sel_c: snap(mean), purser_active: !sel_fault }
    }

    pub fn step_by(&mut self, area: usize, delta: f64) {
        if area < AREAS {
            self.set(area, self.setpoints[area] + delta);
        }
    }
}

impl Default for Temp {
    fn default() -> Self {
        Self::new()
    }
}

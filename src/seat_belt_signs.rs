//! The overhead SEAT BELTS switch's AUTO position, held continuously. The
//! converted cockpit switch (ASOBO_GT_Switch_3States) decides AUTO only at
//! the moment it is clicked; FlyByWire's rule is evaluated here every tick.

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::xp::{DataRef, Xplm};
use crate::Vars;

pub fn auto_signs_on(n1_pct: [f64; 4], slats_deg: [f64; 2], gear_downlocked: [bool; 4]) -> bool {
    n1_pct.iter().any(|&n| n > 10.0) && (slats_deg.iter().any(|&s| s >= 19.0) || gear_downlocked.iter().any(|&g| g))
}

pub struct SeatBeltSigns {
    switch: Option<DataRef>,
    alert: VariableIdentifier,
    alert_1: VariableIdentifier,
    n1: [VariableIdentifier; 4],
    slats: [VariableIdentifier; 2],
    gear: [VariableIdentifier; 4],
}

impl SeatBeltSigns {
    pub fn new(vars: &mut Vars, xplm: &Xplm) -> Self {
        Self {
            switch: xplm.find("fbw/cockpit/SWITCH_OVHD_INTLT_SEATBELT"),
            alert: vars.get("CABIN SEATBELTS ALERT SWITCH".into()),
            alert_1: vars.get("CABIN SEATBELTS ALERT SWITCH:1".into()),
            n1: std::array::from_fn(|i| vars.get(format!("ENGINE_N1:{}", i + 1))),
            slats: [vars.get("LEFT_SLATS_ANGLE".into()), vars.get("RIGHT_SLATS_ANGLE".into())],
            gear: [
                vars.get("LGCIU_1_LEFT_GEAR_DOWNLOCKED".into()),
                vars.get("LGCIU_2_LEFT_GEAR_DOWNLOCKED".into()),
                vars.get("LGCIU_1_RIGHT_GEAR_DOWNLOCKED".into()),
                vars.get("LGCIU_2_RIGHT_GEAR_DOWNLOCKED".into()),
            ],
        }
    }

    pub fn update(&mut self, vars: &mut Vars, xplm: &Xplm) {
        let sign = vars.read(&self.alert);
        vars.write(&self.alert_1, sign);
        if self.switch.is_none() {
            self.switch = xplm.find("fbw/cockpit/SWITCH_OVHD_INTLT_SEATBELT");
        }
        let Some(switch) = self.switch else { return };
        if (xplm.get_f(switch) - 0.5).abs() > 0.1 {
            return;
        }
        let on = auto_signs_on(
            self.n1.map(|id| vars.read(&id)),
            self.slats.map(|id| vars.read(&id)),
            self.gear.map(|id| vars.read(&id) == 1.0),
        );
        if (vars.read(&self.alert) != 0.0) != on {
            vars.write(&self.alert, on as i32 as f64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::auto_signs_on;

    #[test]
    fn auto_lights_the_signs_with_an_engine_running_and_gear_or_slats_out() {
        assert!(!auto_signs_on([0.0; 4], [0.0; 2], [true; 4]), "cold and dark at the gate");
        assert!(auto_signs_on([20.0, 0.0, 0.0, 0.0], [0.0; 2], [true; 4]), "engine started, gear down");
        assert!(auto_signs_on([80.0; 4], [20.0, 20.0], [false; 4]), "gear up, slats still out");
        assert!(!auto_signs_on([80.0; 4], [0.0; 2], [false; 4]), "clean climb: off");
    }
}

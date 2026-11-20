//! The SURV panel's WXR/TAWS and XPDR/TCAS SYS 1(2) pushbuttons: a press selects that system.
//! FlyByWire's model only animates them; the MFD's SURV page shares `A32NX_WXR_TAWS_SYS_SELECTED`.

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier};

use crate::xp::{DataRef, Xplm};
use crate::Vars;

const BUTTONS: [(&str, usize, f64); 4] = [
    ("fbw/cockpit/PUSH_SURV_WXR_TAWS_SYS1", 0, 1.),
    ("fbw/cockpit/PUSH_SURV_WXR_TAWS_SYS2", 0, 2.),
    ("fbw/cockpit/PUSH_SURV_XPDR_TCAS_SYS1", 1, 1.),
    ("fbw/cockpit/PUSH_SURV_XPDR_TCAS_SYS2", 1, 2.),
];

pub struct SurvPanel {
    selected: [VariableIdentifier; 2],
    buttons: [(Option<DataRef>, bool); 4],
    looked: u32,
}

/// The system a press selects, on the press itself only.
fn pressed(was_down: bool, value: f32) -> bool {
    !was_down && value > 0.5
}

impl SurvPanel {
    pub fn new(vars: &mut Vars) -> Self {
        let selected = [vars.register_named("A32NX_WXR_TAWS_SYS_SELECTED"), vars.register_named("A380X_XPDR_TCAS_SYS_SELECTED")];
        for id in &selected {
            if vars.read(id) == 0. {
                vars.write(id, 1.);
            }
        }
        Self { selected, buttons: [(None, false); 4], looked: 0 }
    }

    pub fn update(&mut self, vars: &mut Vars, xplm: &Xplm) {
        self.looked = self.looked.wrapping_add(1);
        for (i, (name, group, system)) in BUTTONS.into_iter().enumerate() {
            let (dataref, down) = &mut self.buttons[i];
            if dataref.is_none() && self.looked % 100 == 1 {
                *dataref = xplm.find(name);
            }
            let Some(d) = *dataref else { continue };
            let value = xplm.get_f(d);
            if pressed(*down, value) {
                vars.write(&self.selected[group], system);
            }
            *down = value > 0.5;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::pressed;

    #[test]
    fn only_the_press_itself_selects() {
        assert!(pressed(false, 1.));
        assert!(!pressed(true, 1.));
        assert!(!pressed(false, 0.));
    }
}

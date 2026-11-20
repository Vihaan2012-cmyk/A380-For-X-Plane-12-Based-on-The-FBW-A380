//! The ground service panel's GND SELECT CTL pushbutton, as the command `fbw/elec/ground_service_toggle`:
//! ground servicing from EXT 1 (`deep::electrical`, A380 TTM ATA 24).

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier};

use crate::published::{self, Command, Published};
use crate::Vars;

pub struct GroundServicePanel {
    _published: Published,
    toggle: Command,
    selected: VariableIdentifier,
}

fn toggled(selected: f64, presses: u32) -> f64 {
    if presses % 2 == 1 {
        if selected != 0. {
            0.
        } else {
            1.
        }
    } else {
        selected
    }
}

impl GroundServicePanel {
    pub fn new(vars: &mut Vars) -> Self {
        let mut p = Published::default();
        let toggle = p.command("fbw/elec/ground_service_toggle", "Ground service panel: GND SELECT CTL (EXT 1 on the ground-service buses only)");
        Self { _published: p, toggle, selected: vars.register_named("A32NX_ELEC_GND_SVC_SELECT_PB_IS_ON") }
    }

    pub fn update(&mut self, vars: &mut Vars) {
        let presses = published::presses(self.toggle);
        if presses > 0 {
            let now = toggled(vars.read(&self.selected), presses);
            vars.write(&self.selected, now);
            crate::log(&format!("ground service panel: GND SELECT CTL {}", if now != 0. { "on" } else { "off" }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::toggled;

    #[test]
    fn each_press_toggles_the_selection() {
        assert_eq!(toggled(0., 1), 1.);
        assert_eq!(toggled(1., 1), 0.);
        assert_eq!(toggled(1., 2), 1.);
        assert_eq!(toggled(0., 0), 0.);
    }
}

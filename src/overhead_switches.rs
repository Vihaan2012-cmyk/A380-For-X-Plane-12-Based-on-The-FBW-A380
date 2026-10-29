//! Up/down commands for overhead three-position switches whose positions
//! count downwards (0 at the top), which an axis manipulator would step the
//! wrong way.

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier};

use crate::published::{self, Command, Published};
use crate::Vars;

const SWITCHES: [(&str, &str); 1] = [("SWITCH_OVHD_INTLT_EMEREXIT", "XMLVAR_SWITCH_OVHD_INTLT_EMEREXIT_Position")];

pub fn stepped(position: f64, up: u32, down: u32) -> f64 {
    (position.round() - up as f64 + down as f64).clamp(0.0, 2.0)
}

pub struct OverheadSwitches {
    _published: Published,
    switches: Vec<(Command, Command, VariableIdentifier)>,
}

impl OverheadSwitches {
    pub fn new(vars: &mut Vars) -> Self {
        let mut p = Published::default();
        let switches = SWITCHES
            .iter()
            .map(|(name, var)| {
                let label = name.replace('_', " ");
                (
                    p.command(&format!("fbw/cockpit/{name}_up"), &format!("{label} up")),
                    p.command(&format!("fbw/cockpit/{name}_down"), &format!("{label} down")),
                    vars.register_named(var),
                )
            })
            .collect();
        Self { _published: p, switches }
    }

    pub fn update(&mut self, vars: &mut Vars) {
        for (up, down, id) in &self.switches {
            let (u, d) = (published::presses(*up), published::presses(*down));
            if u + d > 0 {
                let position = stepped(vars.read(id), u, d);
                vars.write(id, position);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::stepped;

    #[test]
    fn up_moves_toward_on_at_the_top_and_down_toward_off() {
        assert_eq!(stepped(2.0, 1, 0), 1.0, "OFF -> ARM");
        assert_eq!(stepped(1.0, 1, 0), 0.0, "ARM -> ON");
        assert_eq!(stepped(0.0, 1, 0), 0.0, "stays ON");
        assert_eq!(stepped(0.0, 0, 1), 1.0, "ON -> ARM");
        assert_eq!(stepped(2.0, 0, 3), 2.0, "stays OFF");
    }
}

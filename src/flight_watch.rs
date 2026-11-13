use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::Vars;

const PHASES: [&str; 8] = ["preflight", "takeoff", "climb", "cruise", "descent", "approach", "go-around", "done"];
const LOOPS: [&str; 2] = ["GREEN", "YELLOW"];

struct Ids {
    phase: VariableIdentifier,
    nose: [VariableIdentifier; 2],
    n2: [VariableIdentifier; 2],
    altitude: VariableIdentifier,
    ias: VariableIdentifier,
    level: [VariableIdentifier; 2],
    low: [VariableIdentifier; 2],
    modes: [VariableIdentifier; 5],
    vs: VariableIdentifier,
}

#[derive(Default)]
pub struct FlightWatch {
    ids: Option<Ids>,
    phase: Option<f64>,
    low: [bool; 2],
    modes: Option<[f64; 5]>,
}

fn phase_name(v: f64) -> String {
    PHASES.get(v.round().max(0.) as usize).map_or_else(|| format!("{v}"), |s| s.to_string())
}

impl FlightWatch {
    pub fn update(&mut self, vars: &mut Vars) {
        let ids = self.ids.get_or_insert_with(|| Ids {
            phase: vars.get("FMGC_FLIGHT_PHASE".to_owned()),
            nose: [vars.get("LGCIU_1_NOSE_GEAR_COMPRESSED".to_owned()), vars.get("LGCIU_2_NOSE_GEAR_COMPRESSED".to_owned())],
            n2: [vars.get("ENGINE_N2:2".to_owned()), vars.get("ENGINE_N2:3".to_owned())],
            altitude: vars.get("PLANE ALTITUDE".to_owned()),
            ias: vars.get("AIRSPEED INDICATED".to_owned()),
            level: LOOPS.map(|l| vars.get(format!("HYD_{l}_RESERVOIR_LEVEL"))),
            low: LOOPS.map(|l| vars.get(format!("HYD_{l}_RESERVOIR_LEVEL_IS_LOW"))),
            modes: ["A32NX_FMA_LATERAL_MODE", "A32NX_FMA_VERTICAL_MODE", "A32NX_FMA_VERTICAL_ARMED", "A32NX_FG_ALTITUDE_CONSTRAINT", "A32NX_AUTOPILOT_1_ACTIVE"].map(|n| vars.get(n.to_owned())),
            vs: vars.get("VERTICAL SPEED".to_owned()),
        });
        let phase = vars.read(&ids.phase);
        if self.phase.is_some_and(|p| p != phase) {
            crate::log(&format!(
                "fms: flight phase {} -> {} (nose gear compressed {}/{}, N2 engine 2 {:.1} engine 3 {:.1}, altitude {:.0} ft, IAS {:.0} kt)",
                phase_name(self.phase.unwrap_or(0.)),
                phase_name(phase),
                vars.read(&ids.nose[0]),
                vars.read(&ids.nose[1]),
                vars.read(&ids.n2[0]),
                vars.read(&ids.n2[1]),
                vars.read(&ids.altitude),
                vars.read(&ids.ias)
            ));
        }
        self.phase = Some(phase);
        let modes = ids.modes.map(|id| vars.read(&id));
        if self.modes.is_some_and(|m| m != modes) {
            crate::log(&format!(
                "fma: lateral {} vertical {} armed {} | constraint {:.0} ft, AP1 {} | {:.0} ft, VS {:.0} fpm, IAS {:.0} kt, phase {}",
                modes[0], modes[1], modes[2], modes[3], modes[4], vars.read(&ids.altitude), vars.read(&ids.vs), vars.read(&ids.ias), phase_name(phase)
            ));
        }
        self.modes = Some(modes);
        for i in 0..2 {
            let low = vars.read(&ids.low[i]) != 0.;
            let level = vars.read(&ids.level[i]);
            if low && !self.low[i] {
                crate::log(&format!(
                    "hyd: {} reservoir level low ({level:.2} gal) at {:.0} ft, {:.0} kt; active failures {:?}",
                    LOOPS[i].to_lowercase(),
                    vars.read(&ids.altitude),
                    vars.read(&ids.ias),
                    crate::failures::active_ids()
                ));
            }
            self.low[i] = low;
        }
    }
}

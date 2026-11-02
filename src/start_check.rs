//! What a new spawn should be, checked once a few seconds after X-Plane
//! places the aircraft: the cockpit `start_state::spawn_cockpit` set, the
//! engines, exterior lights and chocks a cold or running start has, and
//! X-Plane's own engines, light switches, flaps and parking brake. One log
//! line when everything agrees, one per difference when not.

use std::ffi::c_int;

use systems::simulation::{SimulatorReaderWriter, StartState};

use crate::xp::Xplm;
use crate::Vars;

const SETTLE_S: f64 = 5.;
const MAX_LINES: usize = 40;

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// A systems variable by its full name.
    Var(String),
    /// An X-Plane dataref, with its index if it is an array.
    Dataref(&'static str, Option<usize>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expected {
    pub source: Source,
    pub value: f64,
    pub tolerance: f64,
    pub why: &'static str,
}

fn var(name: impl Into<String>, value: f64, why: &'static str) -> Expected {
    Expected { source: Source::Var(name.into()), value, tolerance: 0.01, why }
}

fn dataref(name: &'static str, index: Option<usize>, value: f64, tolerance: f64, why: &'static str) -> Expected {
    Expected { source: Source::Dataref(name, index), value, tolerance, why }
}

fn on_ground(state: StartState) -> bool {
    matches!(state, StartState::Hangar | StartState::Apron | StartState::Taxi | StartState::Runway)
}

/// The cockpit controls a spawn wrote, less the fault lights FlyByWire's
/// flight files clear but the systems drive.
fn cockpit_expected(cockpit: &[(String, f64)]) -> Vec<Expected> {
    cockpit.iter().filter(|(name, _)| !name.ends_with("_HAS_FAULT")).map(|(name, value)| var(name.clone(), *value, "the flight file")).collect()
}

/// Everything a spawn in `state` with `cockpit` written should still show
/// once the systems have run a few seconds.
pub fn expected(state: StartState, cockpit: &[(String, f64)]) -> Vec<Expected> {
    let running = crate::start_state::engines_running_state(state);
    let start = if running { "a running start" } else { "a cold start" };
    let mut list = cockpit_expected(cockpit);
    list.push(var("A32NX_COLD_AND_DARK_SPAWN", if running { 0. } else { 1. }, start));
    for n in 1..=4 {
        list.push(var(format!("A32NX_ENGINE_STATE:{n}"), if running { 1. } else { 0. }, start));
        list.push(var(format!("GENERAL ENG STARTER:{n}"), if running { 1. } else { 0. }, "the engine master on a start like this"));
        list.push(var(format!("TURB ENG IGNITION SWITCH EX1:{n}"), 1., "the start selector at NORM"));
        list.push(dataref("sim/flightmodel/engine/ENGN_running", Some(n - 1), if running { 1. } else { 0. }, 0.01, start));
    }
    if !running {
        for n in 0..4 {
            list.push(dataref("sim/flightmodel/engine/ENGN_mixt", Some(n), 0., 0.01, start));
            list.push(dataref("sim/flightmodel/engine/ENGN_N1_", Some(n), 0., 5., start));
        }
        list.push(var("A32NX_MODEL_WHEELCHOCKS_ENABLED", 1., "a cold start is chocked"));
        list.push(dataref("sim/cockpit2/controls/flap_handle_request_ratio", None, 0., 0.01, start));
        list.push(dataref("sim/cockpit2/controls/speedbrake_ratio", None, 0., 0.01, start));
    }
    if on_ground(state) {
        list.push(dataref("sim/cockpit2/controls/parking_brake_ratio", None, 1., 0.5, "the parking brake holding"));
        list.push(dataref("sim/flightmodel/position/groundspeed", None, 0., 0.5, "standing still"));
    }
    list
}

/// One line per expectation `read` disagrees with; an unreadable one is
/// skipped.
pub fn differences(expected: &[Expected], mut read: impl FnMut(&Source) -> Option<f64>) -> Vec<String> {
    expected
        .iter()
        .filter_map(|e| {
            let now = read(&e.source)?;
            ((now - e.value).abs() > e.tolerance).then(|| {
                let what = match &e.source {
                    Source::Var(name) => name.clone(),
                    Source::Dataref(name, Some(i)) => format!("{name}[{i}]"),
                    Source::Dataref(name, None) => name.to_string(),
                };
                format!("{what} is {now}, {} for {}", e.value, e.why)
            })
        })
        .collect()
}

#[derive(Default)]
pub struct StartCheck {
    state: Option<StartState>,
    expected: Vec<Expected>,
    age_s: f64,
}

impl StartCheck {
    pub fn armed(state: StartState, cockpit: &[(String, f64)], loading_a_save: bool) -> Self {
        if loading_a_save {
            return Self::default();
        }
        Self { state: Some(state), expected: expected(state, cockpit), age_s: 0. }
    }

    pub fn update(&mut self, vars: &mut Vars, xplm: &Xplm, lights: &crate::lights::Lights, delta: f64) {
        let Some(state) = self.state else { return };
        self.age_s += delta.max(0.);
        if self.age_s < SETTLE_S {
            return;
        }
        self.state = None;
        let mut lines = differences(&self.expected, |source| match source {
            Source::Var(name) => {
                let id = crate::extra_backend::calculator_variable(vars, "L", name);
                Some(vars.read(&id))
            }
            Source::Dataref(name, None) => Some(xplm.get_f(xplm.find(name)?) as f64),
            Source::Dataref(name, Some(i)) if name.ends_with("ENGN_running") => {
                let mut v = [0 as c_int; 8];
                xplm.get_vi(xplm.find(name)?, &mut v);
                Some(v[*i] as f64)
            }
            Source::Dataref(name, Some(i)) => {
                let mut v = [0f32; 8];
                xplm.get_vf(xplm.find(name)?, &mut v);
                Some(v[*i] as f64)
            }
        });
        if !crate::start_state::engines_running_state(state) {
            lines.extend(lights.exterior_switched_on().into_iter().map(|light| format!("{light} light switched on, off for a cold start")));
        }
        let name = crate::start_state::name(state);
        if lines.is_empty() {
            crate::log(&format!("start check ({name}): all {} items as a {name} start has them", self.expected.len()));
            return;
        }
        crate::log(&format!("start check ({name}): {} of {} items differ", lines.len(), self.expected.len()));
        for line in lines.iter().take(MAX_LINES) {
            crate::log(&format!("start check ({name}): {line}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::start_state::{spawn_cockpit, GEAR_LEVER_VAR, PARK_BRAKE_VAR};

    #[test]
    fn a_reading_off_by_more_than_its_tolerance_is_reported_and_an_unreadable_one_is_not() {
        let list = [
            var("A32NX_A", 1., "the flight file"),
            dataref("sim/x", None, 0., 0.5, "standing still"),
            dataref("sim/y", None, 0., 0.5, "standing still"),
        ];
        let lines = differences(&list, |s| match s {
            Source::Var(_) => Some(0.),
            Source::Dataref("sim/x", _) => Some(0.4),
            Source::Dataref(..) => None,
        });
        assert_eq!(lines, vec!["A32NX_A is 0, 1 for the flight file".to_string()]);
    }

    #[test]
    fn a_cold_start_expects_the_engines_off_and_a_running_one_expects_them_running() {
        let cold = expected(StartState::Apron, &[]);
        assert!(cold.contains(&var("A32NX_ENGINE_STATE:2", 0., "a cold start")));
        assert!(cold.iter().any(|e| e.source == Source::Var("A32NX_MODEL_WHEELCHOCKS_ENABLED".into())));
        let hot = expected(StartState::Cruise, &[]);
        assert!(hot.contains(&var("A32NX_ENGINE_STATE:2", 1., "a running start")));
        assert!(!hot.iter().any(|e| e.source == Source::Dataref("sim/cockpit2/controls/parking_brake_ratio", None)));
    }

    /// FlyByWire's real A380 systems, built as a cold apron spawn with the
    /// cockpit `spawn_cockpit` writes, run five seconds: nothing they do on
    /// their own may move a control the spawn set.
    #[test]
    fn the_systems_keep_every_control_a_spawn_sets() {
        use crate::aspects::test_vars::TestVars;
        use a380_systems::A380;
        use systems::simulation::Simulation;

        for state in [StartState::Apron, StartState::Runway, StartState::Cruise] {
            let mut vars = TestVars::default();
            vars.set("TOTAL WEIGHT", 600_000.0);
            vars.set("AMBIENT PRESSURE", 29.92);
            vars.set("AMBIENT TEMPERATURE", 15.0);
            vars.set("SIM ON GROUND", if on_ground(state) { 1. } else { 0. });
            let cockpit = spawn_cockpit(state);
            for (name, value) in &cockpit {
                let id = crate::extra_backend::calculator_variable(&mut vars, "L", name);
                vars.write(&id, *value);
            }
            let mut sim = Simulation::new(state, A380::new, &mut vars);
            let mut aspects = crate::aspects::a380(&mut vars);
            for i in 0..100u32 {
                aspects.pre_tick(&mut vars, 0.05);
                sim.tick(std::time::Duration::from_millis(50), i as f64 * 0.05, &mut vars);
                aspects.post_tick(&mut vars);
            }
            let lines = differences(&cockpit_expected(&cockpit), |s| match s {
                Source::Var(name) => {
                    let id = crate::extra_backend::calculator_variable(&mut vars, "L", name);
                    Some(vars.read(&id))
                }
                Source::Dataref(..) => None,
            });
            assert!(lines.is_empty(), "{state:?}: {lines:#?}");
        }
    }

    #[test]
    fn every_start_sets_the_gear_lever_parking_brake_and_signs_as_its_flight_file() {
        let value = |state, name: &str| spawn_cockpit(state).into_iter().find(|(n, _)| n == name).map(|(_, v)| v);
        for state in [StartState::Hangar, StartState::Apron, StartState::Taxi, StartState::Runway] {
            assert_eq!(value(state, GEAR_LEVER_VAR), Some(1.), "{state:?}");
            assert_eq!(value(state, PARK_BRAKE_VAR), Some(1.), "{state:?}");
        }
        for state in [StartState::Climb, StartState::Cruise, StartState::Approach] {
            assert_eq!(value(state, GEAR_LEVER_VAR), Some(0.), "{state:?}");
            assert_eq!(value(state, PARK_BRAKE_VAR), Some(0.), "{state:?}: released in the air");
        }
        assert_eq!(value(StartState::Final, GEAR_LEVER_VAR), Some(1.));
        for state in [StartState::Hangar, StartState::Apron, StartState::Taxi, StartState::Runway, StartState::Climb, StartState::Cruise, StartState::Approach, StartState::Final] {
            assert_eq!(value(state, "A32NX_OVHD_INTLT_ANN"), Some(1.), "{state:?}: ANN LT at BRT");
            assert_eq!(value(state, "A32NX_OVHD_APU_MASTER_SW_PB_IS_ON"), Some(0.), "{state:?}");
            assert_eq!(value(state, "A32NX_OVHD_COND_CABIN_SELECTOR_KNOB"), Some(200.), "{state:?}: FlyByWire's constructed 24 C");
        }
        assert_eq!(value(StartState::Apron, "XMLVAR_ENG_MODE_SEL"), Some(1.), "start selector NORM");
        assert_eq!(value(StartState::Apron, "XMLVAR_SWITCH_OVHD_INTLT_EMEREXIT_Position"), Some(2.), "EMER EXIT LT off");
        assert_eq!(value(StartState::Apron, "XMLVAR_SWITCH_OVHD_INTLT_NOSMOKING_Position"), Some(2.), "NO SMOKING off");
        assert_eq!(value(StartState::Apron, "A32NX_OVHD_ELEC_BAT_1_PB_IS_AUTO"), Some(0.), "batteries off cold");
        assert_eq!(value(StartState::Climb, "A32NX_OVHD_ELEC_BAT_1_PB_IS_AUTO"), Some(1.), "a climb has cruise.FLT's batteries");
        assert_eq!(value(StartState::Apron, "A32NX_OVHD_ELEC_ENG_GEN_1_PB_IS_ON"), Some(1.), "FlyByWire builds the generators on");
    }
}

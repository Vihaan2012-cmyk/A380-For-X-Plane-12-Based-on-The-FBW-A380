//! Cockpit lighting knobs onto X-Plane's own panel/instrument brightness
//! groups, and the cockpit test pushbutton sequences FlyByWire leaves to the
//! host (FCOM DSC-26-20-20, DSC-26-60-20, DSC-31-90-40, DSC-35-20-20).

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::published::{self, Command, Published};
use crate::xp::{DataRef, Xplm};
use crate::Vars;

/// `panel_brightness_ratio[0..4]`: main panel/pedestal integral, glareshield
/// integral, overhead integral, glareshield LCD (LightingPresets_A380X order).
pub const PANEL_POTS: [usize; 4] = [85, 84, 86, 87];
/// `instrument_brightness_ratio[0..]`: PFD/ND/EWD/SD/WX/MFD, RMP 1-3, HUD L/R.
pub const INSTRUMENT_POTS: [usize; 15] = [88, 89, 90, 91, 92, 93, 94, 95, 98, 99, 80, 81, 82, 71, 72];

/// FCOM DSC-26-20-20 / DSC-26-60-20: held at least three seconds.
pub const FIRE_TEST_MIN_HOLD_S: f64 = 3.;
/// FCOM PRO-NOR-SOP CVR: the fault alert comes 5 s after the press.
pub const CVR_RESULT_S: f64 = 5.;
/// GENERIC: how long a command-started test holds its button.
pub const COMMAND_HOLD_S: f64 = 5.;
/// GENERIC: "a few seconds" of yellow flow indicator (FCOM DSC-35-20-20).
pub const OXY_FLOW_S: f64 = 3.;
/// GENERIC: APU auto-extinguishing test duration before the OK light.
pub const APU_EXTG_TEST_S: f64 = 2.;
/// GENERIC: GPWS lamps on during the test.
pub const GPWS_TEST_S: f64 = 3.;

pub fn brightness(knob: f64, powered: bool) -> f32 {
    if powered {
        knob.clamp(0., 1.) as f32
    } else {
        0.
    }
}

/// A test held for at least `min_s` before it acts.
#[derive(Default, Clone, Copy, Debug)]
pub struct HeldTest {
    held_s: f64,
}
impl HeldTest {
    pub fn step(&mut self, pressed: bool, powered: bool, min_s: f64, dt: f64) -> bool {
        self.held_s = if pressed && powered { self.held_s + dt } else { 0. };
        self.held_s >= min_s
    }
}

/// A test that runs for `duration_s` from the press.
#[derive(Default, Clone, Copy, Debug)]
pub struct TimedTest {
    remaining_s: f64,
    last: bool,
}
impl TimedTest {
    pub fn step(&mut self, pressed: bool, duration_s: f64, dt: f64) -> (bool, bool) {
        let edge = pressed && !self.last;
        self.last = pressed;
        if edge {
            self.remaining_s = duration_s;
            return (true, true);
        }
        self.remaining_s = (self.remaining_s - dt).max(0.);
        (self.remaining_s > 0., false)
    }
}

/// CVR TEST: tone while pressed, fault verdict 5 s after the press.
#[derive(Default, Clone, Copy, Debug)]
pub struct CvrTest {
    since_press_s: Option<f64>,
    last: bool,
}
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct CvrOut {
    pub tone: bool,
    pub failed: bool,
}
impl CvrTest {
    pub fn step(&mut self, pressed: bool, cvr_powered: bool, dt: f64) -> CvrOut {
        if pressed && !self.last {
            self.since_press_s = Some(0.);
        }
        self.last = pressed;
        let Some(t) = self.since_press_s.as_mut() else { return CvrOut::default() };
        *t += dt;
        let failed = *t >= CVR_RESULT_S && !cvr_powered;
        if *t >= CVR_RESULT_S && cvr_powered {
            self.since_press_s = None;
        }
        CvrOut { tone: pressed && cvr_powered, failed }
    }
}

/// A command press holding a cockpit variable at 1 for `COMMAND_HOLD_S`.
struct CommandHold {
    command: Command,
    var: VariableIdentifier,
    remaining_s: f64,
}
impl CommandHold {
    fn step(&mut self, vars: &mut Vars, dt: f64) {
        if published::presses(self.command) > 0 {
            self.remaining_s = COMMAND_HOLD_S;
            vars.write(&self.var, 1.);
        } else if self.remaining_s > 0. {
            self.remaining_s -= dt;
            if self.remaining_s <= 0. {
                vars.write(&self.var, 0.);
            }
        }
    }
}

struct Ids {
    pots_panel: [VariableIdentifier; 4],
    pots_instrument: [VariableIdentifier; 15],
    light_panel: VariableIdentifier,
    ac_1: VariableIdentifier,
    dc_ess: VariableIdentifier,
    ac_ess_shed: VariableIdentifier,
    cargo_test_pb: VariableIdentifier,
    cargo_test_active: VariableIdentifier,
    gpws_test_pb: VariableIdentifier,
    gpws_test_active: VariableIdentifier,
    rcdr_test_pb: VariableIdentifier,
    cvr_powered: VariableIdentifier,
    cvr_tone: VariableIdentifier,
    cvr_failed: VariableIdentifier,
    apu_extg_on: VariableIdentifier,
    apu_extg_ok: VariableIdentifier,
    oxy_test_pb: [VariableIdentifier; 2],
    oxy_flow: [VariableIdentifier; 2],
    ann: VariableIdentifier,
}

pub struct CockpitTests {
    _published: Published,
    ids: Ids,
    panel_ratio: Option<DataRef>,
    instrument_ratio: Option<DataRef>,
    holds: Vec<CommandHold>,
    ann_test: Command,
    ann_restore: Option<(f64, f64)>,
    cargo: HeldTest,
    gpws: TimedTest,
    cvr: CvrTest,
    apu_extg: HeldTest,
    oxy: [TimedTest; 2],
}

impl CockpitTests {
    pub fn new(vars: &mut Vars, xplm: &Xplm) -> Self {
        let mut p = Published::default();
        let ids = Ids {
            pots_panel: PANEL_POTS.map(|n| vars.get(format!("LIGHT POTENTIOMETER:{n}"))),
            pots_instrument: INSTRUMENT_POTS.map(|n| vars.get(format!("LIGHT POTENTIOMETER:{n}"))),
            light_panel: vars.get("LIGHT PANEL".to_string()),
            ac_1: vars.get("ELEC_AC_1_BUS_IS_POWERED".to_string()),
            dc_ess: vars.get("ELEC_DC_ESS_BUS_IS_POWERED".to_string()),
            ac_ess_shed: vars.get("ELEC_AC_ESS_SHED_BUS_IS_POWERED".to_string()),
            cargo_test_pb: vars.get("FIRE_TEST_CARGO".to_string()),
            cargo_test_active: vars.get("FIRE_TEST_CARGO_ACTIVE".to_string()),
            gpws_test_pb: vars.get("GPWS_TEST".to_string()),
            gpws_test_active: vars.get("GPWS_TEST_ACTIVE".to_string()),
            rcdr_test_pb: vars.get("RCDR_TEST".to_string()),
            cvr_powered: vars.get("ELEC_LOAD_cvr_POWERED".to_string()),
            cvr_tone: vars.get("RCDR_TEST_TONE".to_string()),
            cvr_failed: vars.get("RCDR_CVR_TEST_FAILED".to_string()),
            apu_extg_on: vars.get("APU_AUTOEXITING_TEST_ON".to_string()),
            apu_extg_ok: vars.get("APU_AUTOEXITING_TEST_OK".to_string()),
            oxy_test_pb: [1, 2].map(|n| vars.get(format!("OXYGEN_CREW_MASK_TEST:{n}"))),
            oxy_flow: [1, 2].map(|n| vars.get(format!("OXYGEN_CREW_MASK_FLOW_IND:{n}"))),
            ann: vars.get("OVHD_INTLT_ANN".to_string()),
        };
        let holds = [
            ("fbw/test/eng_apu_fire", "Fire test: ENG/APU/MLG", "OVHD_FIRE_TEST_PB_IS_PRESSED"),
            ("fbw/test/cargo_smoke", "Fire test: cargo/avionics smoke", "FIRE_TEST_CARGO"),
            ("fbw/test/gpws", "GPWS test", "GPWS_TEST"),
            ("fbw/test/cvr", "CVR test", "RCDR_TEST"),
            ("fbw/test/apu_auto_exting", "APU auto extinguishing test", "APU_AUTOEXITING_TEST_ON"),
            ("fbw/test/crew_oxygen_capt", "Crew oxygen mask test, captain", "OXYGEN_CREW_MASK_TEST:1"),
            ("fbw/test/crew_oxygen_fo", "Crew oxygen mask test, first officer", "OXYGEN_CREW_MASK_TEST:2"),
        ]
        .into_iter()
        .map(|(name, desc, var)| CommandHold { command: p.command(name, desc), var: vars.get(var.to_string()), remaining_s: 0. })
        .collect();
        let ann_test = p.command("fbw/test/ann_lt", "Annunciator light test");
        Self {
            _published: p,
            ids,
            panel_ratio: xplm.find("sim/cockpit2/switches/panel_brightness_ratio"),
            instrument_ratio: xplm.find("sim/cockpit2/switches/instrument_brightness_ratio"),
            holds,
            ann_test,
            ann_restore: None,
            cargo: HeldTest::default(),
            gpws: TimedTest::default(),
            cvr: CvrTest::default(),
            apu_extg: HeldTest::default(),
            oxy: [TimedTest::default(); 2],
        }
    }

    pub fn update(&mut self, vars: &mut Vars, xplm: &Xplm, dt: f64) {
        let on = |vars: &mut Vars, id: &VariableIdentifier| vars.read(id) != 0.;
        for h in &mut self.holds {
            h.step(vars, dt);
        }
        if published::presses(self.ann_test) > 0 && self.ann_restore.is_none() {
            self.ann_restore = Some((vars.read(&self.ids.ann), COMMAND_HOLD_S));
            vars.write(&self.ids.ann, 0.);
        }
        if let Some((prev, left)) = self.ann_restore.as_mut() {
            *left -= dt;
            if *left <= 0. {
                vars.write(&self.ids.ann, *prev);
                self.ann_restore = None;
            }
        }

        let panel_powered = on(vars, &self.ids.light_panel);
        if let Some(d) = self.panel_ratio {
            for (i, id) in self.ids.pots_panel.iter().enumerate() {
                xplm.set_vf_at(d, i, brightness(vars.read(id), panel_powered));
            }
        }
        if let Some(d) = self.instrument_ratio {
            for (i, id) in self.ids.pots_instrument.iter().enumerate() {
                xplm.set_vf_at(d, i, brightness(vars.read(id), true));
            }
        }

        let (pb, shed) = (on(vars, &self.ids.cargo_test_pb), on(vars, &self.ids.ac_ess_shed));
        let cargo = self.cargo.step(pb, shed, FIRE_TEST_MIN_HOLD_S, dt);
        vars.write(&self.ids.cargo_test_active, cargo as u8 as f64);

        let (pb, ac_1) = (on(vars, &self.ids.gpws_test_pb), on(vars, &self.ids.ac_1));
        let (gpws, started) = self.gpws.step(pb && ac_1, GPWS_TEST_S, dt);
        if started {
            crate::sound_pack::request("aural_pullup_new");
        }
        vars.write(&self.ids.gpws_test_active, gpws as u8 as f64);

        let (pb, cvr_powered, dc_ess) = (on(vars, &self.ids.rcdr_test_pb), on(vars, &self.ids.cvr_powered), on(vars, &self.ids.dc_ess));
        let out = self.cvr.step(pb && dc_ess, cvr_powered, dt);
        vars.write(&self.ids.cvr_tone, out.tone as u8 as f64);
        vars.write(&self.ids.cvr_failed, out.failed as u8 as f64);

        let pb = on(vars, &self.ids.apu_extg_on);
        let ok = self.apu_extg.step(pb, ac_1, APU_EXTG_TEST_S, dt);
        vars.write(&self.ids.apu_extg_ok, ok as u8 as f64);

        for i in 0..2 {
            let pb = on(vars, &self.ids.oxy_test_pb[i]);
            let (flow, _) = self.oxy[i].step(pb, OXY_FLOW_S, dt);
            vars.write(&self.ids.oxy_flow[i], flow as u8 as f64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run<F: FnMut(f64) -> bool>(seconds: f64, mut f: F) -> Vec<(f64, bool)> {
        let dt = 0.1;
        (0..(seconds / dt).round() as usize).map(|i| (i as f64 * dt, f(i as f64 * dt))).collect()
    }

    #[test]
    fn knobs_dim_their_light_group_linearly_and_go_dark_unpowered() {
        assert_eq!(brightness(0.5, true), 0.5);
        assert_eq!(brightness(1.2, true), 1.);
        assert_eq!(brightness(-0.1, true), 0.);
        assert_eq!(brightness(0.8, false), 0.);
        assert_eq!(PANEL_POTS.len(), 4, "panel_brightness_ratio has four groups");
        assert!(INSTRUMENT_POTS.len() <= 16, "instrument_brightness_ratio has sixteen slots");
        let mut all: Vec<usize> = PANEL_POTS.iter().chain(INSTRUMENT_POTS.iter()).copied().collect();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), PANEL_POTS.len() + INSTRUMENT_POTS.len(), "no knob drives two groups");
    }

    #[test]
    fn cargo_smoke_test_acts_after_three_seconds_held_and_stops_on_release() {
        let mut t = HeldTest::default();
        let tl = run(6., |s| t.step(s < 5., true, FIRE_TEST_MIN_HOLD_S, 0.1));
        assert!(tl.iter().filter(|(s, _)| *s < 2.85).all(|(_, a)| !a));
        assert!(tl.iter().filter(|(s, _)| (3.05..4.9).contains(s)).all(|(_, a)| *a));
        assert!(!tl.last().unwrap().1);
        let mut t = HeldTest::default();
        assert!(run(5., |_| t.step(true, false, FIRE_TEST_MIN_HOLD_S, 0.1)).iter().all(|(_, a)| !a), "no power, no test");
    }

    #[test]
    fn a_short_press_never_reaches_the_test() {
        let mut t = HeldTest::default();
        assert!(run(5., |s| t.step(s < 2., true, FIRE_TEST_MIN_HOLD_S, 0.1)).iter().all(|(_, a)| !a));
    }

    #[test]
    fn gpws_test_runs_its_lamps_for_the_test_time_from_one_press() {
        let mut t = TimedTest::default();
        let mut starts = 0;
        let tl = run(5., |s| {
            let (a, st) = t.step(s < 0.3, GPWS_TEST_S, 0.1);
            starts += st as u32;
            a
        });
        assert_eq!(starts, 1, "one aural per press");
        assert!(tl.iter().filter(|(s, _)| *s < 2.85).all(|(_, a)| *a));
        assert!(tl.iter().filter(|(s, _)| *s > 3.15).all(|(_, a)| !a));
    }

    #[test]
    fn cvr_test_reports_a_fault_five_seconds_after_the_press_only_when_failed() {
        let mut t = CvrTest::default();
        let tl: Vec<(f64, CvrOut)> = (0..80).map(|i| (i as f64 * 0.1, t.step(i < 10, false, 0.1))).collect();
        assert!(tl.iter().filter(|(s, _)| *s < 4.85).all(|(_, o)| !o.failed));
        assert!(tl.iter().filter(|(s, _)| *s > 5.05).all(|(_, o)| o.failed));
        assert!(tl.iter().all(|(_, o)| !o.tone), "no tone from a dead recorder");
        let mut t = CvrTest::default();
        let tl: Vec<CvrOut> = (0..80).map(|i| t.step(i < 10, true, 0.1)).collect();
        assert!(tl[..10].iter().all(|o| o.tone));
        assert!(tl.iter().all(|o| !o.failed));
    }

    #[test]
    fn crew_oxygen_flow_indicator_shows_yellow_briefly() {
        let mut t = TimedTest::default();
        let tl = run(6., |s| t.step(s < 0.5, OXY_FLOW_S, 0.1).0);
        assert!(tl.iter().filter(|(s, _)| *s < 2.85).all(|(_, a)| *a));
        assert!(tl.iter().filter(|(s, _)| *s > 3.15).all(|(_, a)| !a));
    }

    #[test]
    fn apu_auto_extinguishing_test_lights_ok_after_its_test_time() {
        let mut t = HeldTest::default();
        let tl = run(4., |_| t.step(true, true, APU_EXTG_TEST_S, 0.1));
        assert!(tl.iter().filter(|(s, _)| *s < 1.85).all(|(_, a)| !a));
        assert!(tl.iter().filter(|(s, _)| *s > 2.05).all(|(_, a)| *a));
    }
}

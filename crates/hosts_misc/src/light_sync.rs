use crate::{js_round, Command};

pub const TIME_OF_DAY_DAY: i32 = 1;
pub const TIME_OF_DAY_DUSK_DAWN: i32 = 2;
pub const TIME_OF_DAY_NIGHT: i32 = 3;

pub const FMGC_PREFLIGHT: i32 = 0;
pub const FMGC_TAKEOFF: i32 = 1;
pub const FMGC_CLIMB: i32 = 2;
pub const FMGC_CRUISE: i32 = 3;
pub const FMGC_DESCENT: i32 = 4;
pub const FMGC_APPROACH: i32 = 5;
pub const FMGC_GO_AROUND: i32 = 6;
pub const FMGC_DONE: i32 = 7;

pub const FWC_FIRST_ENGINE_STARTED: i32 = 2;

const UPDATE_AFTER_MS: u64 = 1500;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LightSyncInputs {
    pub glasscockpit_automatic_brightness_pct: f64,
    pub cabin_circuit_switch_on: bool,
    pub cabin_using_autobrightness: bool,
    pub time_of_day: i32,
    pub fmgc_flight_phase: i32,
    pub fwc_flight_phase: i32,
    pub on_any_runway: bool,
    pub plane_altitude_ft: f64,
    pub airliner_cruise_altitude: f64,
    pub cabin_manual_brightness: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LightSyncConfig {
    pub integral_follows_glareshield: bool,
}

impl Default for LightSyncConfig {
    fn default() -> Self {
        Self { integral_follows_glareshield: false }
    }
}

fn potentiometer(out: &mut Vec<Command>, id: u32, brightness: f64, auto_brightness: bool) {
    out.push(Command::trigger("LIGHT_POTENTIOMETER_SET", false, &[id as f64, brightness]));
    if auto_brightness {
        out.push(Command::lvar("A32NX_CABIN_AUTOBRIGHTNESS", brightness));
    }
}

fn rmp_brightness(out: &mut Vec<Command>, _rmp: u8, brightness: f64) {
    out.push(Command::lvar("A380X_RMP_#RMP_ID#_BRIGHTNESS_KNOB", brightness));
}

pub fn auto_brightness(glasscockpit_automatic_brightness_pct: f64) -> f64 {
    glasscockpit_automatic_brightness_pct.min(85.).max(15.)
}

pub fn init_lighting(i: &LightSyncInputs, config: LightSyncConfig) -> Vec<Command> {
    let mut out = Vec::new();

    if !i.cabin_circuit_switch_on {
        out.push(Command::KeyEvent { name: "ELECTRICAL_CIRCUIT_TOGGLE".to_string(), value: 151. });
    }

    let ab = auto_brightness(i.glasscockpit_automatic_brightness_pct);

    potentiometer(&mut out, 96, 0., false);
    potentiometer(&mut out, 97, 0., false);

    let int_lt = if ab < 50. { 1.5 * ab } else { 0. };
    potentiometer(&mut out, 84, int_lt, false);
    potentiometer(&mut out, 87, ab, false);
    potentiometer(&mut out, 10, 0., false);
    potentiometer(&mut out, 11, 0., false);

    potentiometer(&mut out, 88, ab, false);
    potentiometer(&mut out, 89, ab, false);
    potentiometer(&mut out, 94, ab / 2., false);
    potentiometer(&mut out, 98, ab, false);
    potentiometer(&mut out, 78, ab, false);
    potentiometer(&mut out, 71, ab, false);

    potentiometer(&mut out, 90, ab, false);
    potentiometer(&mut out, 91, ab, false);
    potentiometer(&mut out, 95, ab / 2., false);
    potentiometer(&mut out, 99, ab, false);
    potentiometer(&mut out, 79, ab, false);
    potentiometer(&mut out, 72, ab, false);

    rmp_brightness(&mut out, 1, ab * 100.);
    rmp_brightness(&mut out, 2, ab * 100.);
    rmp_brightness(&mut out, 3, ab * 100.);
    potentiometer(&mut out, 92, ab, false);
    potentiometer(&mut out, 93, ab, false);
    potentiometer(&mut out, 76, ab, false);
    out.push(Command::lvar("A380X_PED_LIGHTING_MIP_FLOOD_LT_KNOB", ab / 100.));
    potentiometer(&mut out, 85, if config.integral_follows_glareshield { int_lt } else { ab }, false);
    out.push(Command::lvar("A380X_PED_LIGHTING_AMBIENT_LT_KNOB", ab / 100.));

    out
}

pub fn update_lighting(i: &LightSyncInputs) -> Vec<Command> {
    let mut out = Vec::new();
    if i.cabin_using_autobrightness {
        match i.time_of_day {
            TIME_OF_DAY_DAY => {
                potentiometer(&mut out, 60, 0., false);
                out.push(Command::lvar("A32NX_CABIN_AUTOBRIGHTNESS", 0.));
            }
            TIME_OF_DAY_DUSK_DAWN | TIME_OF_DAY_NIGHT => match i.fmgc_flight_phase {
                FMGC_PREFLIGHT => {
                    let b = if i.fwc_flight_phase >= FWC_FIRST_ENGINE_STARTED || i.on_any_runway { 1. } else { 85. };
                    potentiometer(&mut out, 60, b, true);
                }
                FMGC_TAKEOFF | FMGC_APPROACH | FMGC_GO_AROUND => potentiometer(&mut out, 60, 1., true),
                FMGC_CLIMB | FMGC_CRUISE => potentiometer(&mut out, 60, 0., true),
                FMGC_DESCENT => {
                    let near_cruise = js_round(i.plane_altitude_ft / 100.) * 100. >= i.airliner_cruise_altitude - 5000.;
                    potentiometer(&mut out, 60, if near_cruise { 5. } else { 0. }, true);
                }
                FMGC_DONE => potentiometer(&mut out, 60, 85., true),
                _ => {}
            },
            _ => {}
        }
    } else {
        potentiometer(&mut out, 60, i.cabin_manual_brightness, false);
    }
    out
}

pub struct LightSync {
    config: LightSyncConfig,
    started_at_ms: Option<u64>,
    first_event_ms: Option<u64>,
}

impl LightSync {
    pub fn new(config: LightSyncConfig) -> Self {
        Self { config, started_at_ms: None, first_event_ms: None }
    }

    pub fn on_ingame(&mut self, now_ms: u64, i: &LightSyncInputs) -> Vec<Command> {
        self.started_at_ms = Some(now_ms);
        self.first_event_ms = None;
        init_lighting(i, self.config)
    }

    pub fn on_real_time(&mut self, now_ms: u64, i: &LightSyncInputs) -> Vec<Command> {
        if self.started_at_ms.is_none() {
            return Vec::new();
        }
        let first = *self.first_event_ms.get_or_insert(now_ms);
        if now_ms.saturating_sub(first) < UPDATE_AFTER_MS {
            return Vec::new();
        }
        update_lighting(i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pot(id: u32, v: f64) -> Command {
        Command::trigger("LIGHT_POTENTIOMETER_SET", false, &[id as f64, v])
    }

    fn base() -> LightSyncInputs {
        LightSyncInputs { glasscockpit_automatic_brightness_pct: 60., cabin_circuit_switch_on: true, ..Default::default() }
    }

    #[test]
    fn auto_brightness_is_clamped_to_15_85() {
        assert_eq!(auto_brightness(0.), 15.);
        assert_eq!(auto_brightness(100.), 85.);
        assert_eq!(auto_brightness(40.), 40.);
    }

    #[test]
    fn init_follows_fbw_order_and_values() {
        let cmds = init_lighting(&base(), LightSyncConfig::default());
        assert_eq!(cmds.len(), 27);
        assert_eq!(cmds[0], pot(96, 0.));
        assert_eq!(cmds[2], pot(84, 0.));
        assert_eq!(cmds[3], pot(87, 60.));
        assert_eq!(cmds[8], pot(94, 30.));
        assert_eq!(cmds[18], Command::lvar("A380X_RMP_#RMP_ID#_BRIGHTNESS_KNOB", 6000.));
        assert_eq!(cmds[24], Command::lvar("A380X_PED_LIGHTING_MIP_FLOOD_LT_KNOB", 0.6));
        assert_eq!(cmds[25], pot(85, 60.));
        assert_eq!(cmds[26], Command::lvar("A380X_PED_LIGHTING_AMBIENT_LT_KNOB", 0.6));
    }

    #[test]
    fn init_toggles_cabin_circuit_only_when_off() {
        let mut i = base();
        i.cabin_circuit_switch_on = false;
        let cmds = init_lighting(&i, LightSyncConfig::default());
        assert_eq!(cmds[0], Command::KeyEvent { name: "ELECTRICAL_CIRCUIT_TOGGLE".to_string(), value: 151. });
        assert_eq!(cmds.len(), 28);
    }

    #[test]
    fn dim_light_turns_interior_light_up() {
        let mut i = base();
        i.glasscockpit_automatic_brightness_pct = 20.;
        let cmds = init_lighting(&i, LightSyncConfig::default());
        assert_eq!(cmds[2], pot(84, 30.));
        let patched = init_lighting(&i, LightSyncConfig { integral_follows_glareshield: true });
        assert_eq!(patched[25], pot(85, 30.));
        let faithful = init_lighting(&i, LightSyncConfig::default());
        assert_eq!(faithful[25], pot(85, 20.));
    }

    #[test]
    fn daylight_cabin_is_off() {
        let mut i = base();
        i.cabin_using_autobrightness = true;
        i.time_of_day = TIME_OF_DAY_DAY;
        assert_eq!(update_lighting(&i), vec![pot(60, 0.), Command::lvar("A32NX_CABIN_AUTOBRIGHTNESS", 0.)]);
    }

    #[test]
    fn night_preflight_dims_until_engine_start_or_runway() {
        let mut i = base();
        i.cabin_using_autobrightness = true;
        i.time_of_day = TIME_OF_DAY_NIGHT;
        i.fmgc_flight_phase = FMGC_PREFLIGHT;
        i.fwc_flight_phase = 1;
        assert_eq!(update_lighting(&i), vec![pot(60, 85.), Command::lvar("A32NX_CABIN_AUTOBRIGHTNESS", 85.)]);
        i.fwc_flight_phase = 2;
        assert_eq!(update_lighting(&i)[0], pot(60, 1.));
        i.fwc_flight_phase = 1;
        i.on_any_runway = true;
        assert_eq!(update_lighting(&i)[0], pot(60, 1.));
    }

    #[test]
    fn night_phases_map_to_cabin_levels() {
        let mut i = base();
        i.cabin_using_autobrightness = true;
        i.time_of_day = TIME_OF_DAY_DUSK_DAWN;
        for (phase, want) in [
            (FMGC_TAKEOFF, 1.),
            (FMGC_APPROACH, 1.),
            (FMGC_GO_AROUND, 1.),
            (FMGC_CLIMB, 0.),
            (FMGC_CRUISE, 0.),
            (FMGC_DONE, 85.),
        ] {
            i.fmgc_flight_phase = phase;
            assert_eq!(update_lighting(&i)[0], pot(60, want), "phase {phase}");
        }
        i.fmgc_flight_phase = 99;
        assert!(update_lighting(&i).is_empty());
    }

    #[test]
    fn descent_lights_up_the_cabin_within_5000_ft_of_cruise() {
        let mut i = base();
        i.cabin_using_autobrightness = true;
        i.time_of_day = TIME_OF_DAY_NIGHT;
        i.fmgc_flight_phase = FMGC_DESCENT;
        i.airliner_cruise_altitude = 36000.;
        i.plane_altitude_ft = 30949.;
        assert_eq!(update_lighting(&i)[0], pot(60, 0.));
        i.plane_altitude_ft = 30950.;
        assert_eq!(update_lighting(&i)[0], pot(60, 5.));
    }

    #[test]
    fn manual_brightness_when_autobrightness_is_off() {
        let mut i = base();
        i.cabin_manual_brightness = 42.;
        assert_eq!(update_lighting(&i), vec![pot(60, 42.)]);
    }

    #[test]
    fn updates_start_1500_ms_after_the_first_real_time_event_following_init() {
        let mut ls = LightSync::new(LightSyncConfig::default());
        let mut i = base();
        i.cabin_manual_brightness = 10.;
        assert!(ls.on_real_time(0, &i).is_empty());
        assert!(!ls.on_ingame(1000, &i).is_empty());
        assert!(ls.on_real_time(1000, &i).is_empty());
        assert!(ls.on_real_time(2499, &i).is_empty());
        assert_eq!(ls.on_real_time(2500, &i), vec![pot(60, 10.)]);
    }
}

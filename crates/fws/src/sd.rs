//! FwsSystemDisplayLogic.ts: the SD page shown per flight phase, the failure page takeover, the STS page timer, the
//! ALL button cycle and the STS NEXT PAGE / MORE h-events.

use crate::arinc::Word;
use crate::fwc::{b, Core};
use crate::vars::Vars;

const CRZ_CONDITION_TIMER_DURATION: f64 = 60.;
const ENG_CONDITION_TIMER_DURATION: f64 = 10.;
const APU_CONDITION_TIMER_DURATION: f64 = 10.;
const FCTL_CONDITION_TIMER_DURATION: f64 = 20.;
const STS_DISPLAY_TIMER_DURATION: f64 = 3.;
const ECAM_LIGHT_DELAY_ALL: f64 = 200.;
const ECAM_ALL_CYCLE_DELAY: f64 = 3000.;
const NO_TIMER: f64 = -9007199254740991.;

const NONE: f64 = -1.;
const ENG: f64 = 0.;
const APU: f64 = 1.;
const DOOR: f64 = 5.;
const WHEEL: f64 = 9.;
const FCTL: f64 = 11.;
const CB: f64 = 12.;
const CRZ: f64 = 13.;
const STATUS: f64 = 14.;

const CURRENT_PAGE_INDEX: &str = "L:A32NX_ECAM_SD_CURRENT_PAGE_INDEX";
const STS_PAGE_TO_SHOW: &str = "L:A32NX_ECAM_SD_STS_PAGE_TO_SHOW";
const MORE_SHOWN: &str = "L:A32NX_ECAM_SD_MORE_SHOWN";

pub struct SdLogic {
    user_selected: f64,
    current_page: f64,
    prev_fail_page: f64,
    page_when_unselected: f64,
    waiting_for_sync: f64,
    all_cycle_next: Option<f64>,
    start_page_all_cycle: f64,
    light_delay_timer: f64,
    prev_all_button: bool,
    crz_cond_timer: f64,
    fctl_timer: f64,
    main_engine_starter_off_timer: f64,
    apu_above_threshold_timer: f64,
    sts_pressed_timer: f64,
    sts_prev_page: f64,
}

impl Default for SdLogic {
    fn default() -> Self {
        Self {
            user_selected: NONE,
            current_page: DOOR,
            prev_fail_page: DOOR,
            page_when_unselected: DOOR,
            waiting_for_sync: NONE,
            all_cycle_next: None,
            start_page_all_cycle: NONE,
            light_delay_timer: NO_TIMER,
            prev_all_button: false,
            crz_cond_timer: CRZ_CONDITION_TIMER_DURATION,
            fctl_timer: FCTL_CONDITION_TIMER_DURATION,
            main_engine_starter_off_timer: -1.,
            apu_above_threshold_timer: -1.,
            sts_pressed_timer: -1.,
            sts_prev_page: NONE,
        }
    }
}

/// The `setInterval` of the ALL button cycle.
pub fn run_timers(c: &mut Core, now_ms: f64) {
    while let Some(next) = c.sd.all_cycle_next {
        if now_ms < next {
            break;
        }
        c.sd.all_cycle_next = Some(next + ECAM_ALL_CYCLE_DELAY);
        let sd = &mut c.sd;
        if sd.current_page != sd.start_page_all_cycle {
            sd.current_page = if sd.current_page == CB { ENG } else { sd.current_page + 1. };
            sd.light_delay_timer = ECAM_LIGHT_DELAY_ALL;
        }
    }
}

/// `hEvent` handler registered in `init()`.
pub fn h_event(c: &mut Core, v: &dyn Vars, name: &str) {
    if name == "A32NX_SD_STS_NEXT_PAGE" {
        let current = v.get(STS_PAGE_TO_SHOW);
        let pages = v.get("L:A32NX_ECAM_SD_STS_NUMBER_OF_PAGES");
        if current + 2. > pages {
            c.write_var(STS_PAGE_TO_SHOW, 0.);
            c.write_var(CURRENT_PAGE_INDEX, NONE);
            c.write_var(MORE_SHOWN, 0.);
        } else {
            c.write_var(STS_PAGE_TO_SHOW, current + 1.);
        }
    } else if name == "A32NX_SD_REQUEST_MORE" && c.sd.current_page == STATUS {
        if !b(v.get("L:A32NX_ECAM_SD_STS_MORE_AVAILABLE")) {
            c.write_var(MORE_SHOWN, 0.);
            return;
        }
        c.write_var(STS_PAGE_TO_SHOW, 0.);
        let more = if v.get(MORE_SHOWN) == 1. { 0. } else { 1. };
        c.write_var(MORE_SHOWN, more);
    }
}

pub fn update(c: &mut Core, v: &dyn Vars, dt: f64) {
    let fail_page = v.get("L:A32NX_ECAM_SFAIL");
    let all_pushed = b(v.get("L:A32NX_BTN_ALL"));
    let apu_rpm = Word::from_simvar(v.get("L:A32NX_APU_N"));
    let phase = c.s.flight_phase;
    let status_normal = c.s.ecam_status_normal;
    let pressure_alt = c.s.adr_pressure_altitude.unwrap_or(0.);
    let now_ms = c.now_ms;
    let mut writes: Vec<f64> = Vec::new();
    let sd = &mut c.sd;

    sd.user_selected = v.get(CURRENT_PAGE_INDEX);
    if sd.waiting_for_sync != NONE && (sd.user_selected == sd.waiting_for_sync || sd.user_selected == NONE) {
        sd.waiting_for_sync = NONE;
    }
    if sd.light_delay_timer != NO_TIMER {
        sd.light_delay_timer -= dt;
        if sd.light_delay_timer <= 0. {
            writes.push(sd.current_page);
            sd.waiting_for_sync = sd.current_page;
            sd.light_delay_timer = NO_TIMER;
        }
    }
    if all_pushed && !sd.prev_all_button {
        sd.start_page_all_cycle = sd.current_page;
        sd.light_delay_timer = ECAM_LIGHT_DELAY_ALL;
        let next = if sd.user_selected == NONE { ENG } else { sd.current_page + 1. };
        sd.current_page = next % CRZ;
        sd.all_cycle_next = Some(now_ms + ECAM_ALL_CYCLE_DELAY);
    } else if !all_pushed && sd.prev_all_button {
        sd.all_cycle_next = None;
    } else if !all_pushed {
        if sd.user_selected != STATUS {
            sd.sts_prev_page = sd.user_selected;
        }
        if sd.light_delay_timer == NO_TIMER && sd.waiting_for_sync == NONE {
            sd.current_page = if sd.user_selected != -1. { sd.user_selected } else { sd.page_when_unselected };
        }
        match phase as u8 {
            12 | 1 => {
                sd.crz_cond_timer = CRZ_CONDITION_TIMER_DURATION;
                sd.page_when_unselected = DOOR;
                sd.check_apu_page(v, &apu_rpm, dt);
                sd.check_engine_page(v, dt);
            }
            2 => {
                let moved = v.get("L:A32NX_SIDESTICK_POSITION_X").abs() > 0.05
                    || v.get("L:A32NX_SIDESTICK_POSITION_Y").abs() > 0.05
                    || v.get("A:RUDDER PEDAL POSITION").abs() > 0.2;
                sd.page_when_unselected = WHEEL;
                if moved {
                    sd.page_when_unselected = FCTL;
                    sd.fctl_timer = FCTL_CONDITION_TIMER_DURATION;
                } else if sd.fctl_timer >= 0. {
                    sd.page_when_unselected = FCTL;
                    sd.fctl_timer -= dt / 1000.;
                }
                sd.check_apu_page(v, &apu_rpm, dt);
                sd.check_engine_page(v, dt);
            }
            3..=7 => sd.page_when_unselected = ENG,
            8..=11 => {
                let gear_extended = v.get("A:GEAR TOTAL PCT EXTENDED") > 0.95;
                let tla = |n: usize| v.get(&format!("L:A32NX_AUTOTHRUST_TLA:{n}"));
                let n1 = |n: usize| v.get(&format!("A:ENG N1 RPM:{n}"));
                let to_power = tla(1).max(tla(2)).max(tla(3)).max(tla(3)) >= 35. && (1..=4).all(|n| n1(n) > 15.);
                let deployed = v.get("L:A32NX_FLAPS_HANDLE_INDEX") != 0. || v.get("L:A32NX_SPOILERS_HANDLE_POSITION") != 0.;
                if gear_extended && pressure_alt < 16000. {
                    sd.page_when_unselected = WHEEL;
                    sd.check_apu_page(v, &apu_rpm, dt);
                    sd.check_engine_page(v, dt);
                } else if deployed || to_power {
                    if sd.crz_cond_timer <= 0. {
                        sd.page_when_unselected = CRZ;
                        sd.check_apu_page(v, &apu_rpm, dt);
                        sd.check_engine_page(v, dt);
                    } else {
                        sd.crz_cond_timer -= dt / 1000.;
                    }
                } else {
                    sd.page_when_unselected = CRZ;
                    sd.check_apu_page(v, &apu_rpm, dt);
                    sd.check_engine_page(v, dt);
                }
            }
            _ => sd.page_when_unselected = DOOR,
        }
        if sd.current_page != STATUS {
            sd.sts_pressed_timer = STS_DISPLAY_TIMER_DURATION;
        } else if status_normal {
            if sd.sts_pressed_timer > 0. {
                sd.sts_pressed_timer -= dt / 1000.;
                sd.page_when_unselected = STATUS;
            } else {
                writes.push(sd.sts_prev_page);
            }
        } else {
            sd.sts_pressed_timer = STS_DISPLAY_TIMER_DURATION;
        }
        if fail_page != NONE {
            sd.page_when_unselected = fail_page;
            if sd.prev_fail_page != fail_page {
                writes.push(NONE);
                sd.user_selected = NONE;
                sd.current_page = fail_page;
            }
        }
        sd.prev_fail_page = fail_page;
    }
    let page = sd.current_page;
    sd.prev_all_button = all_pushed;
    for w in writes {
        c.write_var(CURRENT_PAGE_INDEX, w);
    }
    c.write_var("L:A32NX_ECAM_SD_PAGE_TO_SHOW", page);
}

impl SdLogic {
    fn check_engine_page(&mut self, v: &dyn Vars, dt: f64) {
        let mode = v.get("L:XMLVAR_ENG_MODE_SEL");
        let one_off = (1..=4).any(|n| v.get(&format!("L:A32NX_ENGINE_STATE:{n}")) != 1.);
        let starting = mode == 0. || (one_off && mode == 2.);
        if starting || self.main_engine_starter_off_timer >= 0. {
            if starting {
                self.main_engine_starter_off_timer = ENG_CONDITION_TIMER_DURATION;
            } else {
                self.main_engine_starter_off_timer -= dt / 1000.;
            }
            self.page_when_unselected = ENG;
        }
    }

    fn check_apu_page(&mut self, v: &dyn Vars, rpm: &Word, dt: f64) {
        let master = b(v.get("L:A32NX_OVHD_APU_MASTER_SW_PB_IS_ON"));
        if master && rpm.is_normal() && (rpm.value() <= 95. || self.apu_above_threshold_timer >= 0.) {
            if self.apu_above_threshold_timer <= 0. && rpm.value() <= 95. {
                self.apu_above_threshold_timer = APU_CONDITION_TIMER_DURATION;
            } else if rpm.value() > 95. {
                self.apu_above_threshold_timer -= dt / 1000.;
            }
            self.page_when_unselected = APU;
        }
    }
}

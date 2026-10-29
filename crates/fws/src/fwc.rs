//! The hand-written half of FwsCore: state the generated code reads and writes (`gen::fields::S`), the pieces of
//! `FwsCore.update` that are not table-driven, the subscriptions FwsCore wires in its constructor and `init()`, and
//! the bridge to the host (own var writes, bus topics, Coherent calls).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use serde_json::Value;

use crate::arinc::{Ssm, Word};
use crate::gen::fields::S;
use crate::nodes::Throttler;
use crate::phases::{PhaseInputs, Phases};
use crate::procs::Loop;
use crate::sd::SdLogic;
use crate::sound::Sound;
use crate::vars::Vars;

pub const UPDATE_INTERVAL_MS: f64 = 125.;

#[derive(Debug, Clone, PartialEq)]
pub enum Emit {
    Var(String, f64),
    Str(String, String),
    Topic(String, Value),
    Call(String, String),
}

#[derive(Default)]
pub struct Io {
    pub out: Vec<Emit>,
    own: HashMap<String, (f64, Option<f64>)>,
}

impl Io {
    pub fn var(&mut self, name: &str, value: f64) {
        self.out.push(Emit::Var(name.to_string(), value));
        self.own.insert(name.to_string(), (value, None));
    }

    pub fn string(&mut self, name: &str, value: &str) {
        self.out.push(Emit::Str(name.to_string(), value.to_string()));
    }

    pub fn topic(&mut self, name: &str, value: Value) {
        self.out.push(Emit::Topic(name.to_string(), value));
    }

    pub fn call(&mut self, name: &str, args: String) {
        self.out.push(Emit::Call(name.to_string(), args));
    }
}

pub type Shared = Rc<RefCell<Io>>;

/// The simulator vars as FwsCore sees them: its own `SimVar.SetSimVarValue` writes read back at once, until another
/// writer changes the variable.
pub struct Overlay<'a> {
    pub inner: &'a dyn Vars,
    pub io: Shared,
}

impl Vars for Overlay<'_> {
    fn get(&self, name: &str) -> f64 {
        let inner = self.inner.get(name);
        let mut io = self.io.borrow_mut();
        let Some(entry) = io.own.get_mut(name) else { return inner };
        match entry.1 {
            None => {
                entry.1 = Some(inner);
                entry.0
            }
            Some(base) if base.to_bits() == inner.to_bits() => entry.0,
            Some(_) => {
                io.own.remove(name);
                inner
            }
        }
    }
}

pub fn b(v: f64) -> bool {
    v != 0.
}

#[derive(Default)]
struct AltWarning {
    was_below: bool,
    was_above: bool,
    was_in_range: bool,
    was_reach_200: bool,
    c_chord_short: bool,
    previous_target: Option<f64>,
}

#[derive(Default)]
struct Watch {
    stall: Option<bool>,
    crc: Option<bool>,
    to_config_normal: bool,
    brake_max_braking: bool,
    run_way_too_short: bool,
    keep_max_reverse: bool,
    set_max_reverse: bool,
    auto_brake_off: bool,
    sts_keys: [Vec<Value>; 7],
}

pub struct Core {
    pub s: S,
    pub skip: bool,
    pub now_ms: f64,
    pub io: Shared,
    pub sound: Sound,
    pub phases: Phases,
    throttler: Throttler,
    pub failures: HashSet<String>,
    pub active_abnormal_non_sensed_keys: HashSet<u32>,
    pub fws1_failed: bool,
    pub fws2_failed: bool,
    pub presented_failures: Vec<u32>,
    pub all_current_failures: Vec<u32>,
    pub recall_failures: Vec<u32>,
    pub feed_tank_pumps: [bool; 8],
    pub raw: HashMap<&'static str, crate::js::J>,
    fire_ts: [Option<f64>; 9],
    fire_prev: [bool; 9],
    pub l: Loop,
    pub ecl: fws_ecl::ecl::Ecl,
    pub sd: SdLogic,
    pub init_called: bool,
    pub startup_scheduled: Option<(f64, f64)>,
    pub self_test_time_s: f64,
    dc_ess_prev: bool,
    alt: AltWarning,
    watch: Watch,
    pub callouts: crate::procs::AutoCallouts,
}

impl Core {
    pub fn new(io: Shared, refresh_offset_ms: f64, self_test_time_s: f64, v: &dyn Vars, now_ms: f64) -> Self {
        let sound = Sound::new(io.clone());
        let mut c = Self {
            s: S::default(),
            skip: false,
            now_ms,
            io: io.clone(),
            sound,
            phases: Phases::new(),
            throttler: Throttler::new(UPDATE_INTERVAL_MS, refresh_offset_ms),
            failures: HashSet::new(),
            active_abnormal_non_sensed_keys: HashSet::new(),
            fws1_failed: false,
            fws2_failed: false,
            presented_failures: Vec::new(),
            all_current_failures: Vec::new(),
            recall_failures: Vec::new(),
            feed_tank_pumps: [false; 8],
            raw: HashMap::new(),
            fire_ts: [None; 9],
            fire_prev: [false; 9],
            l: Loop::default(),
            ecl: fws_ecl::ecl::Ecl::new(),
            sd: SdLogic::default(),
            init_called: false,
            startup_scheduled: None,
            self_test_time_s,
            dc_ess_prev: false,
            alt: AltWarning::default(),
            watch: Watch::default(),
            callouts: Default::default(),
        };
        c.ecl.rudder_trim = fws_ecl::ecl::RudderTrimSensing::SignedAsFbw;
        c.s.sim_time = now_ms / 1000.;
        let ov = Overlay { inner: v, io: io.clone() };
        let mut l = std::mem::take(&mut c.l);
        l.observe_activation(&c, &ov, now_ms / 1000.);
        c.l = l;
        c.publish_normal_checklists_initial();
        crate::procs::publish_abnormal_initial(&mut c);
        c.send_failure_warning();
        c
    }

    pub fn write_var(&mut self, name: &str, value: f64) {
        self.io.borrow_mut().var(name, value);
    }

    pub fn write_str(&mut self, name: &str, value: &str) {
        self.io.borrow_mut().string(name, value);
    }

    pub fn publish(&mut self, topic: &str, value: Value) {
        self.io.borrow_mut().topic(topic, value);
    }

    pub fn set_raw(&mut self, field: &'static str, value: crate::js::J) {
        self.raw.insert(field, value);
    }

    pub fn failure_active(&self, id: &str) -> bool {
        self.failures.contains(id)
    }

    pub fn health_injector(&mut self) {
        self.write_var("L:A32NX_NO_SMOKING_MEMO", 1.);
        self.write_var("L:A32NX_CABIN_READY", 1.);
    }

    pub fn adirs_message1(&self, adirs: f64, engine_running: bool) -> f64 {
        match ((adirs / 60.).ceil(), engine_running) {
            (m, false) if m >= 7. => 0.,
            (m, true) if m >= 7. => 1.,
            (m, false) if m == 6. => 2.,
            (m, true) if m == 6. => 3.,
            (m, false) if m == 5. => 4.,
            (m, true) if m == 5. => 5.,
            (m, false) if m == 4. => 6.,
            (m, true) if m == 4. => 7.,
            _ => 0.,
        }
    }

    pub fn adirs_message2(&self, adirs: f64, engine_running: bool) -> f64 {
        match ((adirs / 60.).ceil(), engine_running) {
            (m, false) if m == 3. => 0.,
            (m, true) if m == 3. => 1.,
            (m, false) if m == 2. => 2.,
            (m, true) if m == 2. => 3.,
            (m, false) if m == 1. => 4.,
            (m, true) if m == 1. => 5.,
            _ => 0.,
        }
    }

    pub fn part_splrs(&self) -> bool {
        self.s.green_abnorm_lo_pressure || self.s.yellow_abnorm_lo_pressure || self.s.sec1_fault_condition || self.s.sec2_fault_condition || self.s.sec3_fault_condition
    }

    pub fn most_splrs(&self) -> bool {
        self.s.green_abnorm_lo_pressure && self.s.yellow_abnorm_lo_pressure
    }

    pub fn populate_cpiom_b_words(&mut self, v: &dyn Vars, index: usize, not_reachable: bool) {
        let s = &mut self.s;
        let words: [&mut Word; 4] = match index {
            1 => [&mut s.cpiom_b_ags_app_discrete_word1, &mut s.cpiom_b_vcs_app_discrete_word1, &mut s.cpiom_b_tcs_app_discrete_word1, &mut s.cpiom_b_cpcs_app_discrete_word1],
            2 => [&mut s.cpiom_b_ags_app_discrete_word2, &mut s.cpiom_b_vcs_app_discrete_word2, &mut s.cpiom_b_tcs_app_discrete_word2, &mut s.cpiom_b_cpcs_app_discrete_word2],
            3 => [&mut s.cpiom_b_ags_app_discrete_word3, &mut s.cpiom_b_vcs_app_discrete_word3, &mut s.cpiom_b_tcs_app_discrete_word3, &mut s.cpiom_b_cpcs_app_discrete_word3],
            _ => [&mut s.cpiom_b_ags_app_discrete_word4, &mut s.cpiom_b_vcs_app_discrete_word4, &mut s.cpiom_b_tcs_app_discrete_word4, &mut s.cpiom_b_cpcs_app_discrete_word4],
        };
        for (w, sys) in words.into_iter().zip(["AGS", "VCS", "TCS", "CPCS"]) {
            if not_reachable {
                w.ssm = Ssm::FailureWarning;
            } else {
                *w = Word::from_simvar(v.get(&format!("L:A32NX_COND_CPIOM_B{index}_{sys}_DISCRETE_WORD")));
            }
        }
    }

    pub fn sound_on_update(&mut self, dt: f64) {
        let lost = self.audio_function_lost();
        self.sound.on_update(dt, self.s.startup_completed, lost);
    }

    pub fn flight_phases_update(&mut self, v: &dyn Vars, dt: f64) {
        let tla = |n: usize| v.get(&format!("L:A32NX_AUTOTHRUST_TLA:{n}"));
        let pi = PhaseInputs {
            ra: [self.s.radio_height1, self.s.radio_height2, self.s.radio_height3],
            n1: [self.s.n1_eng1, self.s.n1_eng2, self.s.n1_eng3, self.s.n1_eng4],
            tla: [tla(1), tla(2), tla(3), tla(4)],
            flex_set: v.get("L:A32NX_AIRLINER_TO_FLEX_TEMP") != 0.,
            grounded: v.get("A:PLANE ALT ABOVE GROUND MINUS CG").max(0.) < 10.,
            ias: self.computed_air_speed_to_nearest2(),
            v1: v.get("L:AIRLINER_V1_SPEED"),
            fire_pb_eng1: b(self.s.fire_button_eng1),
            engines_running: [self.s.engine1_running, self.s.engine2_running, self.s.engine3_running, self.s.engine4_running],
            to_config_test: false,
            gear_pct_extended: v.get("A:GEAR TOTAL PCT EXTENDED"),
            skip_startup: self.skip,
        };
        self.phases.update(&pi, dt);
        let phase = self.phases.phase as f64;
        if phase != self.s.flight_phase {
            self.s.flight_phase = phase;
            if self.init_called {
                self.write_var("L:A32NX_FWC_FLIGHT_PHASE", phase);
                self.s.flight_phase_ended_pulse_node.write(true);
            }
        }
        self.write_var("L:A32NX_FWC_TOMEMO", self.phases.to_memo as u8 as f64);
        self.write_var("L:A32NX_FWC_LDGMEMO", self.phases.ldg_memo as u8 as f64);
        self.update_altitude_warning(v);
    }

    fn update_altitude_warning(&mut self, v: &dyn Vars) {
        let pressed = b(v.get("L:PUSH_AUTOPILOT_MASTERAWARN_L")) || b(v.get("L:PUSH_AUTOPILOT_MASTERAWARN_R"));
        let a = &mut self.alt;
        if pressed {
            a.was_below = false;
            a.was_above = false;
            a.was_in_range = false;
            a.c_chord_short = false;
            self.sound.dequeue_sound("cChordOnce");
            self.sound.dequeue_sound("cChordCont");
            return;
        }
        if v.get("A:PLANE ALT ABOVE GROUND MINUS CG").max(0.) < 10. {
            self.sound.dequeue_sound("cChordCont");
        }
        let constraint = v.get("L:A32NX_FG_ALTITUDE_CONSTRAINT");
        let fcu = v.get("A:AUTOPILOT ALTITUDE LOCK VAR:3");
        let managed = v.get("A:AUTOPILOT ALTITUDE SLOT INDEX") == 2.;
        let has_constraint = !(managed && v.get("L:AP_CURRENT_TARGET_ALTITUDE_IS_CONSTRAINT") != 0.);
        let target = if constraint != 0. && !has_constraint { constraint } else { fcu };
        let reset = |a: &mut AltWarning| {
            a.was_below = false;
            a.was_above = false;
            a.was_in_range = false;
            a.was_reach_200 = false;
            a.c_chord_short = false;
        };
        if a.previous_target != Some(target) {
            a.previous_target = Some(target);
            reset(a);
            self.sound.dequeue_sound("cChordOnce");
            self.sound.dequeue_sound("cChordCont");
            return;
        }
        let gear_down = v.get("L:A32NX_FLAPS_HANDLE_INDEX") >= 1. && v.get("L:A32NX_GEAR_HANDLE_POSITION") > 0.5;
        let vm = v.get("L:A32NX_FMA_VERTICAL_MODE");
        let gs = (30. ..=34.).contains(&vm);
        let locked = v.get("A:GEAR POSITION:0") > 0.9;
        let ra = v.get("L:A32NX_TCAS_STATE") > 1.;
        if gear_down || gs || locked || ra {
            reset(a);
            self.sound.dequeue_sound("cChordOnce");
            self.sound.dequeue_sound("cChordCont");
            return;
        }
        let delta = (v.get("A:INDICATED ALTITUDE") - target).abs();
        if delta < 200. {
            a.was_below = true;
            a.was_above = false;
            a.was_reach_200 = true;
        }
        if delta > 750. {
            a.was_above = true;
            a.was_below = false;
            a.c_chord_short = false;
        }
        if (200. ..=750.).contains(&delta) {
            a.was_in_range = true;
        }
        let ap = b(v.get("L:A32NX_AUTOPILOT_1_ACTIVE")) || b(v.get("L:A32NX_AUTOPILOT_2_ACTIVE"));
        if a.was_below && a.was_reach_200 {
            if delta >= 200. {
                self.sound.enqueue_sound("cChordCont");
            } else {
                self.sound.dequeue_sound("cChordCont");
            }
        } else if a.was_above && delta <= 750. && !a.was_reach_200 && !a.c_chord_short && !ap {
            a.c_chord_short = true;
            self.sound.dequeue_sound("cChordCont");
            self.sound.enqueue_sound("cChordOnce");
        } else if delta > 750. && a.was_in_range && !a.was_reach_200 {
            self.sound.enqueue_sound("cChordCont");
        }
    }

    pub fn refresh_consumers(&mut self, v: &dyn Vars) {
        let s = &mut self.s;
        let w = |n: &str| Word::from_simvar(v.get(n));
        s.fcdc1_fg_discrete_word2 = w("L:A32NX_FCDC_1_FG_DISCRETE_WORD_2");
        s.fcdc2_fg_discrete_word2 = w("L:A32NX_FCDC_2_FG_DISCRETE_WORD_2");
        s.fcdc1_fg_discrete_word3 = w("L:A32NX_FCDC_1_FG_DISCRETE_WORD_3");
        s.fcdc2_fg_discrete_word3 = w("L:A32NX_FCDC_2_FG_DISCRETE_WORD_3");
        s.fcdc1_landing_fct_discrete_word = w("L:A32NX_FCDC_1_LANDING_FCT_DISCRETE_WORD");
        s.fcdc2_landing_fct_discrete_word = w("L:A32NX_FCDC_2_LANDING_FCT_DISCRETE_WORD");
        s.fms_dest_efob = b(v.get("L:A380X_FMS_DEST_EFOB_BELOW_MIN"));
        for (i, f) in [&mut s.engine1_value_switch, &mut s.engine2_value_switch, &mut s.engine3_value_switch, &mut s.engine4_value_switch].into_iter().enumerate() {
            *f = b(v.get(&format!("A:FUELSYSTEM VALVE SWITCH:{}", i + 1)));
        }
        for (i, f) in [&mut s.engine1_master, &mut s.engine2_master, &mut s.engine3_master, &mut s.engine4_master].into_iter().enumerate() {
            *f = b(v.get(&format!("A:FUELSYSTEM VALVE SWITCH:{}", i + 1)));
        }
        s.fuel_on_board = w("L:A32NX_FQMS_TOTAL_FUEL_ON_BOARD");
        s.fqms_gross_weight_cg_percent = w("L:A32NX_FQMS_CENTER_OF_GRAVITY_MAC");
        s.fqms_status_word = w("L:A32NX_FQMS_STATUS_WORD");
        s.refuel_panel = v.get("A:INTERACTIVE POINT OPEN:18");
        if s.aircraft_on_ground {
            s.fueling_initiated = b(v.get("L:A32NX_REFUEL_STARTED_BY_USR"));
            s.fueling_target = v.get("L:A32NX_FUEL_DESIRED");
        }
        s.ir1_ground_speed = w("L:A32NX_ADIRS_IR_1_GROUND_SPEED");
        s.ir2_ground_speed = w("L:A32NX_ADIRS_IR_2_GROUND_SPEED");
        s.ir3_ground_speed = w("L:A32NX_ADIRS_IR_3_GROUND_SPEED");
        s.fma_lateral_mode = v.get("L:A32NX_FMA_LATERAL_MODE");
        s.stall_warning_raw = b(v.get("L:A32NX_STALL_WARNING_ON"));
        for i in 0..8 {
            self.feed_tank_pumps[i] = b(v.get(&format!("A:FUELSYSTEM PUMP ACTIVE:{}", i + 1)));
        }
    }

    pub fn fire_timestamp(&self, i: usize) -> Option<f64> {
        self.fire_ts[i]
    }

    fn after_acquire(&mut self) {
        let clocks = [
            self.s.fire_eng1_agent1_in_flight10_seconds_disch_clock,
            self.s.eng1_fire_in_flight_fire30_seconds_disch_clock,
            self.s.fire_eng2_agent1_in_flight10_seconds_disch_clock,
            self.s.eng2_fire_in_flight_fire30_seconds_disch_clock,
            self.s.fire_eng3_agent1_in_flight10_seconds_disch_clock,
            self.s.eng3_fire_in_flight_fire30_seconds_disch_clock,
            self.s.fire_eng4_agent1_in_flight10_seconds_disch_clock,
            self.s.eng4_fire_in_flight_fire30_seconds_disch_clock,
            self.s.apu_fire_agent1_discharge10_seconds_clock_active,
        ];
        for (i, on) in clocks.into_iter().enumerate() {
            if on != self.fire_prev[i] {
                self.fire_prev[i] = on;
                self.fire_ts[i] = on.then_some(self.now_ms);
            }
        }
        let stall = self.s.stall_warning;
        if self.init_called && self.watch.stall != Some(stall) {
            self.watch.stall = Some(stall);
            self.s.fwc1_out126.set_bit_value(17, stall);
            self.s.fwc2_out126.set_bit_value(17, stall);
            self.sound.handle_sound_condition("stall", stall);
        }
        if self.s.to_config_normal != self.watch.to_config_normal {
            self.watch.to_config_normal = self.s.to_config_normal;
            if self.init_called {
                self.write_var("L:A32NX_TO_CONFIG_NORMAL", self.s.to_config_normal as u8 as f64);
            }
        }
    }

    pub fn set_aural_crc(&mut self, on: bool) {
        self.s.aural_crc_active = on;
        if self.init_called && self.watch.crc != Some(on) {
            self.watch.crc = Some(on);
            self.sound.handle_sound_condition("continuousRepetitiveChime", on);
        }
    }

    pub fn callout_sounds(&mut self) {
        let c = (self.callouts.brake_max_braking, self.callouts.runway_too_short, self.callouts.keep_max_reverse, self.callouts.set_max_reverse, self.s.auto_brake_off_memo_and_audio);
        let w = &mut self.watch;
        let mut changes = Vec::new();
        for (now, prev, key) in [
            (c.0, &mut w.brake_max_braking, "brakeMaxBraking"),
            (c.1, &mut w.run_way_too_short, "runwayTooShort"),
            (c.2, &mut w.keep_max_reverse, "keepMaxReverse"),
            (c.3, &mut w.set_max_reverse, "setMaxReverse"),
            (c.4, &mut w.auto_brake_off, "autoBrakeOff"),
        ] {
            if now != *prev {
                *prev = now;
                changes.push((key, now));
            }
        }
        if self.init_called {
            for (k, on) in changes {
                self.sound.handle_sound_condition(k, on);
            }
        }
    }

    pub fn power_changed(&mut self, v: &dyn Vars) {
        let powered = self.s.dc_ess_bus_powered;
        if powered == self.dc_ess_prev {
            return;
        }
        self.dc_ess_prev = powered;
        if powered {
            let cold_and_dark = b(v.get("L:A32NX_COLD_AND_DARK_SPAWN"));
            let quick_mode = b(v.get("L:A32NX_AIRCRAFT_PRESET_QUICK_MODE"));
            let delay = if cold_and_dark && !quick_mode { self.self_test_time_s * 5_000. } else { 0. };
            self.startup_scheduled = Some((self.now_ms, delay));
        } else {
            self.startup_scheduled = None;
            if self.s.startup_completed {
                self.s.startup_completed = false;
                self.send_failure_warning();
            }
        }
    }

    pub fn run_startup_timer(&mut self, now_ms: f64) {
        let Some((scheduled, delay)) = self.startup_scheduled else { return };
        if now_ms > scheduled && now_ms >= scheduled + delay {
            self.startup_scheduled = None;
            self.s.startup_completed = true;
            self.on_startup_completed();
        }
    }

    fn on_startup_completed(&mut self) {
        self.init();
        self.ecl.on_startup_completed();
        crate::procs::reset_non_sensed(self);
        crate::procs::clear_lists(self);
        self.publish_normal_checklists(true);
    }

    fn init(&mut self) {
        self.init_called = true;
        self.l.sc_inhibit_until = Some(self.now_ms + crate::procs::AURAL_SC_INHIBIT_MS);
        self.watch.crc = Some(self.s.aural_crc_active);
        self.sound.handle_sound_condition("continuousRepetitiveChime", self.s.aural_crc_active);
        self.watch.stall = Some(self.s.stall_warning);
        self.s.fwc1_out126.set_bit_value(17, self.s.stall_warning);
        self.s.fwc2_out126.set_bit_value(17, self.s.stall_warning);
        self.sound.handle_sound_condition("stall", self.s.stall_warning);
        self.watch.sts_keys = Default::default();
        self.subscribed_outputs();
    }

    /// The `init()` subscriptions that FwsCore registers with an immediate first call: their outputs follow the
    /// subject on every change, so writing the current value each update (the host drops repeats) is the same.
    pub fn subscribed_outputs(&mut self) {
        if !self.init_called {
            return;
        }
        let mc = self.master_caution_output();
        let mw = self.master_warning_output();
        self.write_var("L:A32NX_MASTER_CAUTION", mc as u8 as f64);
        self.write_var("L:A32NX_MASTER_WARNING", mw as u8 as f64);
        let arrow = self.s.lg_lever_red_arrow as u8 as f64;
        self.write_var("L:A32NX_FWC_1_LG_RED_ARROW", arrow);
        self.write_var("L:A32NX_FWC_2_LG_RED_ARROW", arrow);
        self.write_out126();
        for (i, l) in self.l.left_lines.clone().iter().enumerate() {
            self.write_str(&format!("L:A32NX_EWD_LOWER_LEFT_LINE_{}", i + 1), l);
        }
        for (i, l) in self.l.right_lines.clone().iter().enumerate() {
            self.write_str(&format!("L:A32NX_EWD_LOWER_RIGHT_LINE_{}", i + 1), l);
        }
        for (i, l) in self.l.pfd_memo_lines.clone().iter().enumerate() {
            self.write_str(&format!("L:A32NX_PFD_MEMO_LINE_{}", i + 1), l);
        }
        for (i, l) in self.l.pfd_limitations_lines.clone().iter().enumerate() {
            self.write_str(&format!("L:A32NX_PFD_LIMITATIONS_LINE_{}", i + 1), l);
        }
        self.write_var("L:A32NX_FWC_1_ABN_PROC_IMPACT_LDG_PERF", self.s.abn_proc_impacting_ldg_perf_active as u8 as f64);
        self.write_var("L:A32NX_FWC_1_ABN_PROC_IMPACT_LDG_DIST", self.s.abn_proc_impacting_ldg_dist_active as u8 as f64);
        self.write_var("L:A32NX_STATUS_NORMAL", self.s.ecam_status_normal as u8 as f64);
        self.publish("fws_show_sts_indication", Value::Bool(self.s.ecam_ewd_show_sts_indication));
        self.publish("fws_show_failure_pending", Value::Bool(self.s.ecam_ewd_show_failure_pending_indication));
        let s = &self.s;
        let normal = serde_json::json!([s.eng1_start_or_crank, s.eng2_start_or_crank, s.eng3_start_or_crank, s.eng4_start_or_crank]);
        let abnormal = serde_json::json!([s.eng1_primary_abnormal_params, s.eng2_primary_abnormal_params, s.eng3_primary_abnormal_params, s.eng4_primary_abnormal_params]);
        self.publish("fws_normal_attention_getter_eng", normal);
        self.publish("fws_abnormal_primary_engine_parameters_attention_getter", abnormal);
        let keys = [
            ("fws_limitations_all_phases", self.l.limitations_all_phases_keys.clone()),
            ("fws_limitations_appr_ldg", self.l.limitations_appr_ldg_keys.clone()),
            ("fws_information", self.l.information_keys.clone()),
            ("fws_inop_sys_all_phases", self.l.inop_sys_all_phases_keys.clone()),
            ("fws_inop_sys_appr_ldg", self.l.inop_sys_appr_ldg_keys.clone()),
            ("fws_alerts_impacting_ldg_perf", Vec::new()),
            ("fws_inop_sys_redundancy_loss", self.l.inop_sys_redund_loss_keys.clone()),
        ];
        for (i, (topic, k)) in keys.into_iter().enumerate() {
            if self.watch.sts_keys[i] != k {
                self.watch.sts_keys[i] = k.clone();
                self.publish(topic, Value::from(k));
            }
        }
    }

    /// FWC discrete word 126: on-ground, audio and stall bits, SSM from the FWS failed / startup state.
    pub fn write_out126(&mut self) {
        if !self.init_called {
            return;
        }
        let ground = self.s.aircraft_on_ground;
        let (lost1, lost2) = (self.s.fws1_audio_function_lost, self.s.fws2_audio_function_lost);
        let ok1 = !self.fws1_failed && self.s.startup_completed;
        let ok2 = !self.fws2_failed && self.s.startup_completed;
        for (w, lost, ok) in [(&mut self.s.fwc1_out126, lost1, ok1), (&mut self.s.fwc2_out126, lost2, ok2)] {
            w.set_bit_value(28, ground);
            w.set_bit_value(16, !lost);
            w.ssm = if ok { Ssm::NormalOperation } else { Ssm::FailureWarning };
        }
        let (w1, w2) = (self.s.fwc1_out126.to_simvar_string(), self.s.fwc2_out126.to_simvar_string());
        self.write_str("L:A32NX_FWC_1_DISCRETE_WORD_126", &w1);
        self.write_str("L:A32NX_FWC_2_DISCRETE_WORD_126", &w2);
    }

    /// `FwsCore.sendFailureWarning`.
    pub fn send_failure_warning(&mut self) {
        send_failure_warning(&self.io);
    }

    fn publish_normal_checklists_initial(&mut self) {
        self.publish("fws_normal_checklists_id", Value::from(0));
        self.publish("fws_normal_checklists", Value::Array(Vec::new()));
        self.publish("fws_deferred_procedures", Value::Array(Vec::new()));
        self.l.ecl_published = (self.ecl_json(), Value::Array(Vec::new()), 0);
    }

    fn ecl_json(&self) -> Value {
        let state = |s: &fws_ecl::ecl::ChecklistState| {
            serde_json::json!({
                "id": s.id.to_string(),
                "procedureCompleted": s.procedure_completed,
                "procedureActivated": s.procedure_activated,
                "itemsChecked": s.items_checked,
                "itemsActive": s.items_active,
                "itemsToShow": s.items_to_show,
            })
        };
        Value::Array(self.ecl.states.iter().chain(std::iter::once(&self.ecl.overview)).map(state).collect())
    }

    fn deferred_json(&self) -> Value {
        Value::Array(
            self.ecl
                .deferred_procs
                .iter()
                .map(|e| {
                    let s = &e.state;
                    serde_json::json!({
                        "id": s.id.to_string(),
                        "procedureCompleted": s.procedure_completed,
                        "procedureActivated": s.procedure_activated,
                        "itemsChecked": s.items_checked,
                        "itemsActive": s.items_active,
                        "itemsToShow": s.items_to_show,
                    })
                })
                .collect(),
        )
    }

    pub fn publish_normal_checklists(&mut self, force: bool) {
        let lists = self.ecl_json();
        if force || lists != self.l.ecl_published.0 {
            self.l.ecl_published.0 = lists.clone();
            self.publish("fws_normal_checklists", lists);
        }
        let deferred = self.deferred_json();
        if deferred != self.l.ecl_published.1 {
            self.l.ecl_published.1 = deferred.clone();
            self.publish("fws_deferred_procedures", deferred);
        }
        if self.ecl.checklist_id != self.l.ecl_published.2 {
            self.l.ecl_published.2 = self.ecl.checklist_id;
            self.publish("fws_normal_checklists_id", Value::from(self.ecl.checklist_id));
        }
    }

    /// One FWS cycle (`FwsCore.update`).
    pub fn update(&mut self, v: &dyn Vars, raw_dt: f64, now_ms: f64) {
        self.now_ms = now_ms;
        self.s.sim_time = now_ms / 1000.;
        self.skip = b(v.get("L:A32NX_FWC_SKIP_STARTUP"));
        self.s.refresh_registered(v);
        self.refresh_consumers(v);
        let dt = self.throttler.can_update(raw_dt);

        let ecp_not_reachable = !b(v.get("L:A32NX_AFDX_3_3_REACHABLE"))
            && !b(v.get("L:A32NX_AFDX_13_13_REACHABLE"))
            && !b(v.get("L:A32NX_AFDX_4_4_REACHABLE"))
            && !b(v.get("L:A32NX_AFDX_14_14_REACHABLE"));
        self.s.fws_ecp_failed = self.failure_active("FwsEcp") || ecp_not_reachable;
        let ecp_failed = self.s.fws_ecp_failed;
        if b(v.get("L:A32NX_BTN_TOCONFIG")) && !ecp_failed {
            self.s.to_config_input_buffer.write(true, false);
        }
        if b(v.get("L:A32NX_BTN_CLR")) || b(v.get("L:A32NX_BTN_CLR2")) {
            self.s.clear_button_input_buffer.write(true, false);
        }
        if b(v.get("L:A32NX_BTN_RCL")) && !ecp_failed {
            self.s.recall_button_input_buffer.write(true, false);
        }
        if b(v.get("L:A32NX_BTN_CL")) {
            self.s.cl_input_buffer.write(true, false);
        }
        if b(v.get("L:A32NX_BTN_CHECK_LH")) || b(v.get("L:A32NX_BTN_CHECK_RH")) {
            self.s.cl_check_input_buffer.write(true, false);
        }
        if b(v.get("L:A32NX_BTN_UP")) {
            self.s.cl_up_input_buffer.write(true, false);
        }
        if b(v.get("L:A32NX_BTN_DOWN")) {
            self.s.cl_down_input_buffer.write(true, false);
        }
        if b(v.get("L:A32NX_BTN_ABNPROC")) {
            self.s.abn_proc_input_buffer.write(true, false);
        }
        if dt == -1. || raw_dt == 0. {
            return;
        }

        crate::gen::update::acquire(self, v, dt, raw_dt);
        self.power_changed(v);
        self.after_acquire();
        crate::procs::post(self, v, dt);
    }

    pub fn instinctive_disconnect(&mut self, autothrust: bool) {
        if autothrust {
            self.s.a_thr_disc_input_buffer.write(true, false);
            if self.s.auto_thrust_off_voluntary {
                self.s.auto_thrust_inhibit_caution = true;
                self.s.request_master_caution_from_a_thr_off = false;
            }
        } else {
            self.s.ap_disc_input_buffer.write(true, false);
            if self.now_ms - self.s.auto_pilot_instinctive_disc_last_pressed > 25. {
                self.s.auto_pilot_instinctive_disc_count_since_last_fws_cycle += 1.;
                self.s.auto_pilot_instinctive_disc_last_pressed = self.now_ms;
            }
        }
    }
}

pub fn send_failure_warning(io: &Shared) {
    let mut io = io.borrow_mut();
    io.var("L:A32NX_STATUS_NORMAL", 1.);
    io.var("L:A32NX_ECAM_FAILURE_ACTIVE", 0.);
    io.var("L:A32NX_ECAM_SFAIL", -1.);
    io.var("L:A32NX_MASTER_CAUTION", 0.);
    io.var("L:A32NX_MASTER_WARNING", 0.);
    io.var("L:A32NX_FWC_1_LG_RED_ARROW", 0.);
    io.var("L:A32NX_FWC_2_LG_RED_ARROW", 0.);
    let mut w = Word::empty();
    w.ssm = Ssm::FailureWarning;
    io.string("L:A32NX_FWC_1_DISCRETE_WORD_126", &w.to_simvar_string());
    io.string("L:A32NX_FWC_2_DISCRETE_WORD_126", &w.to_simvar_string());
    for t in [
        "fws_limitations_all_phases",
        "fws_limitations_appr_ldg",
        "fws_information",
        "fws_inop_sys_all_phases",
        "fws_inop_sys_appr_ldg",
        "fws_alerts_impacting_ldg_perf",
        "fws_inop_sys_redundancy_loss",
        "fws_cancelled_caution",
    ] {
        io.topic(t, Value::Array(Vec::new()));
    }
    io.topic("fws_show_sts_indication", Value::Bool(false));
    io.topic("fws_show_failure_pending", Value::Bool(false));
    io.topic("fws_abnormal_primary_engine_parameters_attention_getter", serde_json::json!([false, false, false, false]));
    io.topic("fws_normal_attention_getter_eng", serde_json::json!([false, false, false, false]));
}

pub fn pitch_trim_in_green_band(trim: f64) -> bool {
    !(trim > 5.8 || trim < -0.18)
}

fn cg_to_pitch_trim(cg: f64) -> f64 {
    if cg < 29. {
        5.8
    } else if cg <= 34.5 {
        5.8 - ((cg - 29.) * 56.) / 275.
    } else if cg <= 43. {
        4.68 - ((cg - 34.5) * 243.) / 425.
    } else {
        -0.18
    }
}

pub fn pitch_trim_in_cyan_band(cg: f64, trim: f64) -> bool {
    (cg_to_pitch_trim(cg) - trim).abs() < 1.5 && pitch_trim_in_green_band(trim)
}

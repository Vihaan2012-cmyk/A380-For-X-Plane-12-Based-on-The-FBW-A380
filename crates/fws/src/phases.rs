use crate::arinc::Word;
use crate::nodes::{Confirm, Memory, Monostable};

pub struct PhaseInputs {
    pub ra: [Word; 3],
    pub n1: [f64; 4],
    pub tla: [f64; 4],
    pub flex_set: bool,
    pub grounded: bool,
    pub ias: f64,
    pub v1: f64,
    pub fire_pb_eng1: bool,
    pub engines_running: [bool; 4],
    pub to_config_test: bool,
    pub gear_pct_extended: f64,
    pub skip_startup: bool,
}

pub struct Phases {
    pub phase: u8,
    pub to_memo: bool,
    pub ldg_memo: bool,
    one_eng_running_conf: Confirm,
    gnd_memo: Confirm,
    speed_above_80: Memory,
    speed_above_v1: Memory,
    mct_memo: Confirm,
    fire_pb_out_conf: Confirm,
    fire_pb_out_memo: Monostable,
    fire_pb_clear12: Memory,
    phase112_memo: Monostable,
    phase10_ground_memo: Monostable,
    ac80_memo: Monostable,
    prev_phase11_invert: Monostable,
    two_eng_to_power_invert: Monostable,
    phase9_nvm: Memory,
    prev_phase11: bool,
    ground_immediate_memo: Monostable,
    phase6_memo: Monostable,
    phase7_memo: Monostable,
    phase89_memo: Monostable,
    memo_to_conf: Confirm,
    memo_to_memo: Memory,
    ldg_conf01: Confirm,
    ldg_inhibit: Memory,
    ldg_conf02: Confirm,
    ldg_below_2000: Memory,
}

impl Phases {
    pub fn new() -> Self {
        Self {
            phase: 1,
            to_memo: false,
            ldg_memo: false,
            one_eng_running_conf: Confirm::rising(30.),
            gnd_memo: Confirm::rising(1.),
            speed_above_80: Memory::new(true),
            speed_above_v1: Memory::new(true),
            mct_memo: Confirm::new(60., false),
            fire_pb_out_conf: Confirm::rising(0.2),
            fire_pb_out_memo: Monostable::rising(2.),
            fire_pb_clear12: Memory::new(false),
            phase112_memo: Monostable::rising(300.),
            phase10_ground_memo: Monostable::rising(2.),
            ac80_memo: Monostable::rising(2.),
            prev_phase11_invert: Monostable::new(3., false, false),
            two_eng_to_power_invert: Monostable::new(1., false, false),
            phase9_nvm: Memory::new(true),
            prev_phase11: false,
            ground_immediate_memo: Monostable::rising(2.),
            phase6_memo: Monostable::rising(15.),
            phase7_memo: Monostable::rising(120.),
            phase89_memo: Monostable::rising(180.),
            memo_to_conf: Confirm::rising(120.),
            memo_to_memo: Memory::new(false),
            ldg_conf01: Confirm::rising(1.),
            ldg_inhibit: Memory::new(false),
            ldg_conf02: Confirm::rising(10.),
            ldg_below_2000: Memory::new(true),
        }
    }

    pub fn update(&mut self, i: &PhaseInputs, dt: f64) {
        self.update_flight_phase(i, dt);
        self.update_takeoff_memo(i, dt);
        self.update_landing_memo(i, dt);
    }

    fn update_flight_phase(&mut self, i: &PhaseInputs, dt: f64) {
        let s = i.skip_startup;
        let invalid = |w: &Word| w.is_failure_warning() || w.is_no_computed_data();
        let ra = if invalid(&i.ra[0]) {
            if invalid(&i.ra[1]) {
                i.ra[2]
            } else {
                i.ra[1]
            }
        } else {
            i.ra[0]
        };
        let one_eng_raw = i.n1.iter().any(|n| *n > 15.);
        let one_engine_running = self.one_eng_running_conf.write(one_eng_raw, dt, s);
        let no_engine_running = !one_engine_running;
        let h_fail = i.ra.iter().all(|w| w.is_failure_warning());
        let adc_test_inhib = false;

        let ground_immediate = i.grounded;
        let ground = self.gnd_memo.write(ground_immediate, dt, s);

        let ias = i.ias;
        let ac_speed_above_80 = self.speed_above_80.write(ias > 83., ias < 77.);

        let ac_above_v1 = if i.v1 > 0. { self.speed_above_v1.write(ias > i.v1 + 3., ias < i.v1 - 3.) } else { false };

        let h_abv1500 = ra.is_no_computed_data() || ra.value() > 1500.;
        let h_abv800 = ra.is_no_computed_data() || ra.value() > 800.;
        let h_abv400 = ra.is_no_computed_data() || ra.value() > 400.;

        let tla_fto = i.flex_set;
        let mct = |t: f64| t > 33.3 && t < 36.7;
        let full = |t: f64| t > 43.3;
        let mcl = |t: f64| t > 22.9;
        let sup_mct = |t: f64| !(t < 36.7);
        let eng_mcl = [mcl(i.tla[0]), mcl(i.tla[1]), mcl(i.tla[2]), mcl(i.tla[3])];
        let eng_full = [full(i.tla[0]), full(i.tla[1]), full(i.tla[2]), full(i.tla[2])];
        let two_engines_mcl = eng_mcl.iter().filter(|b| **b).count() >= 2;
        let mut to_power = [false; 4];
        for e in 0..4 {
            to_power[e] = (tla_fto && mct(i.tla[e])) || eng_full[e] || sup_mct(i.tla[e]);
        }
        let two_engines_to_power_signal = to_power.iter().filter(|b| **b).count() >= 2;
        let two_engines_to_power = two_engines_to_power_signal || (self.mct_memo.write(two_engines_to_power_signal, dt, s) && !h_abv1500 && two_engines_mcl);

        let fire_conf = self.fire_pb_out_conf.write(i.fire_pb_eng1, dt, s);
        let eng1_fire_pb_memo = self.fire_pb_out_memo.write(fire_conf, dt, s);
        let reset_fire_pb_clear12 = eng1_fire_pb_memo && ground;

        let p10_ground = self.phase10_ground_memo.write(ground_immediate, dt, s);
        let phase10 = (p10_ground || ground_immediate) && !two_engines_to_power && ac_speed_above_80;

        let phase345_cond = ground && two_engines_to_power;
        let phase3 = !ac_speed_above_80 && one_eng_raw && phase345_cond;
        let phase4 = ac_speed_above_80 && phase345_cond && !ac_above_v1;
        let phase5 = ac_speed_above_80 && phase345_cond && ac_above_v1;

        let set_phase11_nvm = phase3 || phase10;
        let ac80 = self.ac80_memo.write(!ac_speed_above_80, dt, s);
        let reset_phase11_nvm = if !ac80 {
            let any = (ground && self.prev_phase11_invert.write(self.prev_phase11, dt, s))
                || reset_fire_pb_clear12
                || (ground && self.two_eng_to_power_invert.write(two_engines_to_power, dt, s));
            any && !self.prev_phase11
        } else {
            false
        } || adc_test_inhib;
        let phase11_nvm = self.phase9_nvm.write(set_phase11_nvm, reset_phase11_nvm);
        let phase211_cond = ground && !two_engines_to_power && !ac_speed_above_80;
        let phase11 = one_eng_raw && phase11_nvm && phase211_cond;
        let phase2 = phase211_cond && !phase11_nvm && one_eng_raw;

        let phase112_memo_a = self.fire_pb_clear12.write(phase11, reset_fire_pb_clear12);
        let phase112_cond = !phase11 && no_engine_running && ground_immediate;
        let phase112_memo = self.phase112_memo.write(phase112_memo_a && phase112_cond, dt, s);
        let phase1 = phase112_cond && !phase112_memo;
        let phase12 = phase112_cond && phase112_memo;

        self.prev_phase11 = phase11;

        let ground2s = self.ground_immediate_memo.write(ground_immediate, dt, s) || ground_immediate;

        let phase6_cond = !h_abv400 && two_engines_to_power && !h_fail && !ground2s;
        let phase6 = self.phase6_memo.write(phase6_cond, dt, s) && phase6_cond;

        let phase7_cond = !phase6 && !h_abv1500 && two_engines_to_power && !h_fail && !ground2s;
        let phase7 = self.phase7_memo.write(phase7_cond, dt, s) && phase7_cond;

        let phase89_cond = !ground2s && !h_fail && !two_engines_to_power && !h_abv1500 && !h_abv800;
        let phase89 = self.phase89_memo.write(phase89_cond, dt, s) && phase89_cond;

        let phase8 = !phase7 && !ground2s && !phase89;
        let phase9 = phase89 && !phase10;

        let phases = [phase1, phase2, phase3, phase4, phase5, phase6, phase7, phase8, phase9, phase10, phase11, phase12];
        let active: Vec<u8> = phases.iter().enumerate().filter(|(_, b)| **b).map(|(n, _)| n as u8 + 1).collect();

        if active.len() == 1 {
            self.phase = active[0];
        } else if active.len() > 1 && !active.contains(&self.phase) {
            self.phase = active[0];
        }
    }

    fn update_takeoff_memo(&mut self, i: &PhaseInputs, dt: f64) {
        let set = self.phase == 2 && i.to_config_test;
        let reset = matches!(self.phase, 12 | 3 | 1 | 8);
        let memo = self.memo_to_memo.write(set, reset);
        let timer = self.memo_to_conf.write(i.engines_running.iter().all(|b| *b), dt, i.skip_startup);
        self.to_memo = memo || (self.phase == 2 && timer);
    }

    fn update_landing_memo(&mut self, i: &PhaseInputs, dt: f64) {
        let invalid = |w: &Word| w.is_failure_warning() || w.is_no_computed_data();
        let inv = [invalid(&i.ra[0]), invalid(&i.ra[1]), invalid(&i.ra[2])];
        let gear_down = i.gear_pct_extended > 0.95;
        let set_below = (0..3).any(|n| i.ra[n].value() < 2000. && !inv[n]);
        let reset_below = (0..3).all(|n| i.ra[n].value() > 2200. || inv[n]);
        let memo2 = self.ldg_below_2000.write(set_below, reset_below);
        let set_inhibit = self.ldg_conf01.write(reset_below && !inv[0] && !inv[1] && !inv[2], dt, i.skip_startup);
        let reset_inhibit = !matches!(self.phase, 9 | 10 | 8);
        let memo1 = self.ldg_inhibit.write(set_inhibit, reset_inhibit);
        let show_in_approach = memo1 && memo2 && self.phase == 8;
        let invalid_radio = self.ldg_conf02.write(inv[0] && inv[1] && inv[2] && gear_down && self.phase == 8, dt, i.skip_startup);
        self.ldg_memo = show_in_approach || invalid_radio || self.phase == 10 || self.phase == 9;
    }
}

impl Default for Phases {
    fn default() -> Self {
        Self::new()
    }
}

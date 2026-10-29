//! The FADEC start sequencer: auto start with its aborts, dry-crank and
//! retries, manual start, dry/wet crank, in-flight starter-assisted or
//! windmill starts, quick relight and auto relight. One path for both
//! engine types; every threshold comes from `EngineData`.
//!
//! FCOM: DSC-70-80-20 (ignition for starting), DSC-70-80-30-20 (auto start,
//! abort list, three attempts, no abort in flight, quick/auto relight,
//! windmill), DSC-70-80-30-30 (manual start), DSC-70-80-40 (cranking),
//! DSC-70-20-20 (EGT overlimit shuts down and motors), LIM-70 (starter).

use crate::physics::engine::engine_data::EngineData;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartSelector {
    Crank,
    Norm,
    IgnStart,
}

impl StartSelector {
    pub fn from_position(v: f64) -> Self {
        match v.round() as i32 {
            0 => Self::Crank,
            2 => Self::IgnStart,
            _ => Self::Norm,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ignition {
    Off,
    One,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartFault {
    HotStart,
    HungStart,
    NoLightUp,
    EgtOverlimit,
    Stall,
    N1Jam,
    StartValveAbnormal,
    StarterTimeExceeded,
    StarterFailure,
}

impl StartFault {
    pub fn code(self) -> f64 {
        match self {
            Self::HotStart => 1.0,
            Self::HungStart => 2.0,
            Self::NoLightUp => 3.0,
            Self::EgtOverlimit => 4.0,
            Self::Stall => 5.0,
            Self::N1Jam => 6.0,
            Self::StartValveAbnormal => 7.0,
            Self::StarterTimeExceeded => 8.0,
            Self::StarterFailure => 9.0,
        }
    }

    fn retryable(self) -> bool {
        !matches!(self, Self::N1Jam | Self::StarterFailure)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Off,
    AutoCrank,
    AutoFuel,
    AutoLit,
    DryCrank,
    Aborted,
    ManualCrank,
    ManualFuel,
    CrankDry,
    CrankWet,
    QuickRelight,
    Running,
}

impl Phase {
    pub fn code(self) -> f64 {
        match self {
            Self::Off => 0.0,
            Self::AutoCrank => 1.0,
            Self::AutoFuel => 2.0,
            Self::AutoLit => 3.0,
            Self::DryCrank => 4.0,
            Self::Aborted => 5.0,
            Self::ManualCrank => 6.0,
            Self::ManualFuel => 7.0,
            Self::CrankDry => 8.0,
            Self::CrankWet => 9.0,
            Self::QuickRelight => 10.0,
            Self::Running => 11.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StartInputs {
    pub selector: StartSelector,
    pub master_on: bool,
    pub man_start_on: bool,
    pub on_ground: bool,
    pub cas_kt: f64,
    pub hp_pct: f64,
    pub lp_pct: f64,
    /// Displayed EGT, C.
    pub egt_c: f64,
    /// Start valve position, 0 shut .. 1 open.
    pub valve_position: f64,
    pub starter_failed: bool,
    pub stall: bool,
    /// The combustor is lit (flame-out when false with fuel on).
    pub lit: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StartOutputs {
    pub phase: Phase,
    pub valve_open: bool,
    pub fuel_on: bool,
    pub ignition: Ignition,
    pub attempt: u8,
    pub fault: Option<StartFault>,
    pub windmill: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct StartSequencer {
    phase: Phase,
    attempt: u8,
    fault: Option<StartFault>,
    timer_s: f64,
    crank_at_20_s: f64,
    egt_at_fuel_c: f64,
    hung_ref_hp: f64,
    hung_timer_s: f64,
    valve_timer_s: f64,
    starter_on_s: f64,
    was_master_on: bool,
    was_running_master_off_s: Option<f64>,
    relight_hold_s: f64,
    in_flight: bool,
    windmill: bool,
    manual: bool,
}

impl Default for StartSequencer {
    fn default() -> Self {
        Self::new()
    }
}

impl StartSequencer {
    pub fn new() -> Self {
        Self {
            phase: Phase::Off,
            attempt: 0,
            fault: None,
            timer_s: 0.0,
            crank_at_20_s: 0.0,
            egt_at_fuel_c: 0.0,
            hung_ref_hp: 0.0,
            hung_timer_s: 0.0,
            valve_timer_s: 0.0,
            starter_on_s: 0.0,
            was_master_on: false,
            was_running_master_off_s: None,
            relight_hold_s: 0.0,
            in_flight: false,
            windmill: false,
            manual: false,
        }
    }

    pub fn running() -> Self {
        Self { phase: Phase::Running, was_master_on: true, ..Self::new() }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    fn reset(&mut self) {
        *self = Self { was_master_on: self.was_master_on, was_running_master_off_s: self.was_running_master_off_s, ..Self::new() };
    }

    fn enter(&mut self, phase: Phase) {
        self.phase = phase;
        self.timer_s = 0.0;
    }

    pub fn step(&mut self, d: &EngineData, i: &StartInputs, dt: f64) -> StartOutputs {
        let s = &d.start;
        let master_rose = i.master_on && !self.was_master_on;
        let master_fell = !i.master_on && self.was_master_on;
        self.was_master_on = i.master_on;
        self.timer_s += dt;

        if master_fell {
            let quick = matches!(self.phase, Phase::Running | Phase::QuickRelight);
            self.reset();
            self.was_running_master_off_s = if quick { Some(0.0) } else { None };
        }
        if let Some(t) = self.was_running_master_off_s.as_mut() {
            *t += dt;
            if *t > s.quick_relight_window_s {
                self.was_running_master_off_s = None;
            }
        }

        if master_rose {
            let quick = self.was_running_master_off_s.take().is_some() && i.hp_pct > s.quick_relight_min_hp_pct;
            if quick {
                self.enter(Phase::QuickRelight);
                self.relight_hold_s = s.relight_ignition_hold_s;
            } else if self.phase == Phase::Off && i.hp_pct >= d.idle_hp_pct - 2.0 && i.lit {
                self.enter(Phase::Running);
            } else {
                match (i.selector, self.phase) {
                    (StartSelector::IgnStart, Phase::ManualCrank) => self.enter(Phase::ManualFuel),
                    (StartSelector::IgnStart, Phase::Off) if i.man_start_on => {
                        self.manual = true;
                        self.enter(Phase::ManualFuel);
                    }
                    (StartSelector::IgnStart, Phase::Off) => self.begin_auto(d, i),
                    (StartSelector::Crank, Phase::CrankDry) | (StartSelector::Crank, Phase::Off) if i.man_start_on => self.enter(Phase::CrankWet),
                    _ => {}
                }
            }
        }

        if !i.master_on {
            match (self.phase, i.selector, i.man_start_on) {
                (Phase::Off, StartSelector::IgnStart, true) if i.hp_pct < s.starter_engage_max_hp_pct => {
                    self.manual = true;
                    self.enter(Phase::ManualCrank);
                }
                (Phase::Off, StartSelector::Crank, true) if i.hp_pct < s.starter_engage_max_hp_pct => self.enter(Phase::CrankDry),
                (Phase::ManualCrank, sel, pb) if !pb || sel != StartSelector::IgnStart => self.enter(Phase::Off),
                (Phase::CrankDry, sel, pb) if !pb || sel != StartSelector::Crank => self.enter(Phase::Off),
                _ => {}
            }
        } else if self.phase == Phase::CrankWet && i.selector != StartSelector::Crank {
            self.enter(Phase::Off);
        }

        let starter_engaged = matches!(self.phase, Phase::AutoCrank | Phase::AutoFuel | Phase::AutoLit | Phase::DryCrank | Phase::ManualCrank | Phase::ManualFuel)
            && !self.windmill
            && i.master_on;
        if starter_engaged {
            self.starter_on_s += dt;
        }

        match self.phase {
            Phase::AutoCrank => {
                if i.hp_pct >= s.fuel_on_hp_pct {
                    self.crank_at_20_s += dt;
                }
                let crank_done = self.in_flight || self.crank_at_20_s >= s.rotor_bow_crank_s;
                if i.hp_pct >= s.fuel_on_hp_pct && crank_done {
                    self.fuel_on(i);
                }
            }
            Phase::AutoFuel => {
                if i.egt_c - self.egt_at_fuel_c >= s.light_off_rise_c {
                    self.enter(Phase::AutoLit);
                    self.hung_ref_hp = i.hp_pct;
                    self.hung_timer_s = 0.0;
                }
            }
            Phase::AutoLit | Phase::ManualFuel => {
                let valve_close = if self.manual { s.manual_valve_close_hp_pct } else { s.auto_valve_close_hp_pct };
                if i.hp_pct >= valve_close {
                    self.enter(Phase::Running);
                }
            }
            Phase::DryCrank => {
                if self.timer_s >= s.dry_crank_s {
                    self.attempt += 1;
                    self.fault = None;
                    self.fuel_on(i);
                }
            }
            Phase::QuickRelight => {
                self.relight_hold_s -= dt;
                if self.relight_hold_s <= 0.0 {
                    self.enter(Phase::Running);
                }
            }
            Phase::Running => {
                if i.master_on && !i.lit {
                    self.relight_hold_s = s.relight_ignition_hold_s;
                } else if self.relight_hold_s > 0.0 {
                    self.relight_hold_s -= dt;
                }
            }
            _ => {}
        }

        self.monitor(d, i, dt, starter_engaged);
        self.outputs(d, i)
    }

    fn begin_auto(&mut self, d: &EngineData, i: &StartInputs) {
        let s = &d.start;
        self.manual = false;
        self.attempt = 1;
        self.in_flight = !i.on_ground;
        self.windmill = self.in_flight && i.hp_pct >= s.starter_assist_below_hp_pct && i.cas_kt >= s.windmill_min_cas_kt;
        self.enter(Phase::AutoCrank);
    }

    fn fuel_on(&mut self, i: &StartInputs) {
        self.egt_at_fuel_c = i.egt_c;
        self.enter(Phase::AutoFuel);
    }

    fn monitor(&mut self, d: &EngineData, i: &StartInputs, dt: f64, starter_engaged: bool) {
        let s = &d.start;
        let auto = matches!(self.phase, Phase::AutoCrank | Phase::AutoFuel | Phase::AutoLit);
        let manual = self.phase == Phase::ManualFuel;
        if !auto && !manual {
            return;
        }
        let valve_cmd = self.valve_commanded(d, i);
        let valve_ok = if valve_cmd { i.valve_position > 0.5 } else { i.valve_position < 0.5 };
        self.valve_timer_s = if valve_ok { 0.0 } else { self.valve_timer_s + dt };

        if self.phase == Phase::AutoLit {
            self.hung_timer_s += dt;
            if i.hp_pct - self.hung_ref_hp >= s.hung_min_gain_pct {
                self.hung_ref_hp = i.hp_pct;
                self.hung_timer_s = 0.0;
            }
        }

        let limit = if i.on_ground { d.egt_ground_start_c } else { d.egt_relight_c };
        let fuelled = matches!(self.phase, Phase::AutoFuel | Phase::AutoLit | Phase::ManualFuel);
        let detected = if fuelled && i.egt_c > limit && i.hp_pct < d.start_limit_hp_pct {
            Some(StartFault::HotStart)
        } else if fuelled && i.egt_c > limit {
            Some(StartFault::EgtOverlimit)
        } else if i.starter_failed && starter_engaged {
            Some(StartFault::StarterFailure)
        } else if i.hp_pct > s.n1_jam_hp_pct && i.lp_pct < s.n1_jam_lp_pct {
            Some(StartFault::N1Jam)
        } else if self.phase == Phase::AutoFuel && self.timer_s > s.no_light_up_s {
            Some(StartFault::NoLightUp)
        } else if self.phase == Phase::AutoLit && self.hung_timer_s > s.hung_window_s {
            Some(StartFault::HungStart)
        } else if i.stall && fuelled {
            Some(StartFault::Stall)
        } else if self.valve_timer_s > s.valve_abnormal_s && !self.windmill {
            Some(StartFault::StartValveAbnormal)
        } else if self.starter_on_s > s.starter_max_continuous_s {
            Some(StartFault::StarterTimeExceeded)
        } else {
            None
        };
        let Some(fault) = detected else { return };
        if self.fault.is_none() {
            self.fault = Some(fault);
        }
        if manual {
            if i.on_ground && matches!(fault, StartFault::HotStart | StartFault::EgtOverlimit) {
                self.enter(Phase::Aborted);
            }
            return;
        }
        if !i.on_ground || self.in_flight || i.hp_pct > s.no_abort_above_hp_pct {
            return;
        }
        if fault.retryable() && self.attempt < s.attempts {
            self.enter(Phase::DryCrank);
        } else {
            self.enter(Phase::Aborted);
        }
    }

    fn valve_commanded(&self, d: &EngineData, i: &StartInputs) -> bool {
        let s = &d.start;
        match self.phase {
            Phase::AutoCrank | Phase::AutoFuel | Phase::AutoLit => !self.windmill && i.hp_pct < s.auto_valve_close_hp_pct,
            Phase::DryCrank => true,
            Phase::ManualCrank | Phase::CrankDry | Phase::CrankWet => true,
            Phase::ManualFuel => i.hp_pct < s.manual_valve_close_hp_pct,
            _ => false,
        }
    }

    fn outputs(&self, d: &EngineData, i: &StartInputs) -> StartOutputs {
        let s = &d.start;
        let fuel_on = matches!(self.phase, Phase::AutoFuel | Phase::AutoLit | Phase::ManualFuel | Phase::CrankWet | Phase::QuickRelight | Phase::Running)
            && i.master_on;
        let ignition = match self.phase {
            Phase::AutoFuel | Phase::AutoLit => {
                if i.hp_pct >= s.auto_ignition_off_hp_pct {
                    Ignition::Off
                } else if self.in_flight || self.attempt >= 2 {
                    Ignition::Both
                } else {
                    Ignition::One
                }
            }
            Phase::ManualFuel if i.hp_pct < s.manual_ignition_off_hp_pct => Ignition::Both,
            Phase::QuickRelight => Ignition::Both,
            Phase::Running if self.relight_hold_s > 0.0 => Ignition::Both,
            _ => Ignition::Off,
        };
        StartOutputs {
            phase: self.phase,
            valve_open: self.valve_commanded(d, i),
            fuel_on,
            ignition,
            attempt: self.attempt,
            fault: self.fault,
            windmill: self.windmill && matches!(self.phase, Phase::AutoCrank | Phase::AutoFuel | Phase::AutoLit),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::engine::engine_data::{GP7200, TRENT_900};

    const DT: f64 = 0.1;
    const TYPES: [&EngineData; 2] = [&TRENT_900, &GP7200];

    #[derive(Default, Clone, Copy)]
    struct Faults {
        never_lights: bool,
        hot: bool,
        hung_at: Option<f64>,
        n1_jam: bool,
        valve_stuck_shut: bool,
        starter_failed: bool,
    }

    struct Rig {
        d: &'static EngineData,
        seq: StartSequencer,
        inp: StartInputs,
        fuel_ign_s: f64,
        f: Faults,
        t: f64,
        out: StartOutputs,
    }

    fn cold() -> StartInputs {
        StartInputs {
            selector: StartSelector::IgnStart,
            master_on: false,
            man_start_on: false,
            on_ground: true,
            cas_kt: 0.0,
            hp_pct: 0.0,
            lp_pct: 0.0,
            egt_c: 15.0,
            valve_position: 0.0,
            starter_failed: false,
            stall: false,
            lit: false,
        }
    }

    impl Rig {
        fn new(d: &'static EngineData, f: Faults) -> Self {
            let inp = cold();
            let mut seq = StartSequencer::new();
            let out = seq.step(d, &inp, DT);
            Self { d, seq, inp, fuel_ign_s: 0.0, f, t: 0.0, out }
        }

        fn tick(&mut self) {
            let d = self.d;
            let i = &mut self.inp;
            let target = if self.out.valve_open && !self.f.valve_stuck_shut { 1.0 } else { 0.0 };
            i.valve_position += (target - i.valve_position).clamp(-DT / 2.0, DT / 2.0);
            i.starter_failed = self.f.starter_failed;
            let starter = i.valve_position > 0.5 && !self.f.starter_failed;
            if self.out.fuel_on && (self.out.ignition != Ignition::Off || i.lit) && !self.f.never_lights && i.hp_pct >= 15.0 {
                self.fuel_ign_s += DT;
                if self.fuel_ign_s >= 3.0 {
                    i.lit = true;
                }
            } else if !self.out.fuel_on {
                i.lit = false;
                self.fuel_ign_s = 0.0;
            }
            if i.lit {
                let cap = self.f.hung_at.unwrap_or(f64::MAX).max(i.hp_pct.min(self.f.hung_at.unwrap_or(f64::MAX)));
                i.hp_pct = (i.hp_pct + 1.5 * DT).min(d.idle_hp_pct).min(cap);
                let egt_target = if self.f.hot {
                    d.egt_ground_start_c + 100.0
                } else if i.hp_pct < d.idle_hp_pct {
                    d.egt_ground_start_c - 120.0
                } else {
                    d.idle_egt_c
                };
                let rate = if self.f.hot { 100.0 } else { 40.0 };
                i.egt_c += (egt_target - i.egt_c).clamp(-rate * DT, rate * DT);
            } else {
                if starter && i.hp_pct < 30.0 {
                    i.hp_pct += 2.0 * DT;
                } else if starter {
                    i.hp_pct = (i.hp_pct - DT).max(30.0);
                } else {
                    i.hp_pct = (i.hp_pct - DT).max(if i.on_ground { 0.0 } else { 15.0 });
                }
                i.egt_c += (15.0 - i.egt_c).clamp(-20.0 * DT, 20.0 * DT);
            }
            i.lp_pct = if self.f.n1_jam { 0.0 } else { i.hp_pct * d.idle_lp_pct / d.idle_hp_pct };
            self.out = self.seq.step(d, i, DT);
            self.t += DT;
        }

        fn run_for(&mut self, s: f64) {
            for _ in 0..(s / DT).round() as usize {
                self.tick();
            }
        }

        fn run_until(&mut self, limit_s: f64, mut cond: impl FnMut(&StartOutputs, &StartInputs) -> bool) -> f64 {
            let start = self.t;
            while self.t - start < limit_s {
                self.tick();
                if cond(&self.out, &self.inp) {
                    return self.t - start;
                }
            }
            panic!("{:?}: condition not met in {limit_s} s; {:?} {:?}", self.d.engine_type, self.out, self.inp);
        }
    }

    #[test]
    fn auto_ground_start_cranks_20_s_past_20_pct_then_one_igniter_and_closes_the_valve_at_58_4() {
        for d in TYPES {
            let mut r = Rig::new(d, Faults::default());
            r.inp.master_on = true;
            r.tick();
            assert!(r.out.valve_open && !r.out.fuel_on);
            r.run_until(30.0, |_, i| i.hp_pct >= 20.0);
            let fuel_after = r.run_until(40.0, |o, _| o.fuel_on);
            assert!((fuel_after - d.start.rotor_bow_crank_s).abs() < 0.3, "{fuel_after}");
            assert_eq!(r.out.ignition, Ignition::One);
            r.run_until(60.0, |o, _| o.phase == Phase::Running);
            assert!(r.inp.hp_pct >= d.start.auto_valve_close_hp_pct);
            assert!(!r.out.valve_open && r.out.fuel_on && r.out.ignition == Ignition::Off);
            assert_eq!((r.out.fault, r.out.attempt), (None, 1));
        }
    }

    #[test]
    fn no_light_up_dry_cranks_30_s_and_retries_twice_with_both_igniters_then_aborts() {
        for d in TYPES {
            let mut r = Rig::new(d, Faults { never_lights: true, ..Faults::default() });
            r.inp.master_on = true;
            r.run_until(80.0, |o, _| o.fault == Some(StartFault::NoLightUp));
            assert_eq!(r.out.phase, Phase::DryCrank);
            assert!(r.out.valve_open && !r.out.fuel_on && r.out.ignition == Ignition::Off);
            let crank = r.run_until(40.0, |o, _| o.fuel_on);
            assert!((crank - d.start.dry_crank_s).abs() < 0.3, "{crank}");
            assert_eq!((r.out.attempt, r.out.ignition), (2, Ignition::Both));
            r.run_until(200.0, |o, _| o.phase == Phase::Aborted);
            assert_eq!(r.out.attempt, 3);
            assert!(!r.out.valve_open && !r.out.fuel_on && r.out.ignition == Ignition::Off);
            r.run_for(60.0);
            assert_eq!(r.out.phase, Phase::Aborted);
            r.inp.master_on = false;
            r.tick();
            assert_eq!((r.out.phase, r.out.fault), (Phase::Off, None));
        }
    }

    #[test]
    fn hot_start_aborts_at_the_type_start_limit() {
        for d in TYPES {
            let mut r = Rig::new(d, Faults { hot: true, ..Faults::default() });
            r.inp.master_on = true;
            r.run_until(120.0, |o, _| o.fault == Some(StartFault::HotStart));
            assert!(r.inp.egt_c > d.egt_ground_start_c && r.inp.egt_c < d.egt_ground_start_c + 15.0);
            assert_eq!(r.out.phase, Phase::DryCrank);
            r.run_until(300.0, |o, _| o.phase == Phase::Aborted);
            assert_eq!(r.out.attempt, 3);
        }
    }

    #[test]
    fn trent_and_gp_differ_only_through_their_data() {
        for (d, aborts) in [(&TRENT_900, true), (&GP7200, false)] {
            let mut r = Rig::new(d, Faults::default());
            r.inp.master_on = true;
            r.run_until(60.0, |o, _| o.fuel_on);
            r.inp.egt_c = 720.0;
            r.out = r.seq.step(d, &r.inp, DT);
            assert_eq!(r.out.fault == Some(StartFault::HotStart), aborts, "{:?}", d.engine_type);
        }
    }

    #[test]
    fn hung_start_retries() {
        for d in TYPES {
            let mut r = Rig::new(d, Faults { hung_at: Some(40.0), ..Faults::default() });
            r.inp.master_on = true;
            r.run_until(120.0, |o, _| o.fault == Some(StartFault::HungStart));
            assert!(r.inp.hp_pct <= 40.0 + 1e-9);
            assert_eq!(r.out.phase, Phase::DryCrank);
        }
    }

    #[test]
    fn n1_jam_and_starter_failure_abort_without_retry() {
        for d in TYPES {
            for (f, want) in [(Faults { n1_jam: true, ..Faults::default() }, StartFault::N1Jam), (Faults { starter_failed: true, ..Faults::default() }, StartFault::StarterFailure)] {
                let mut r = Rig::new(d, f);
                r.inp.master_on = true;
                r.run_until(120.0, |o, _| o.phase == Phase::Aborted);
                assert_eq!((r.out.fault, r.out.attempt), (Some(want), 1));
            }
        }
    }

    #[test]
    fn start_valve_stuck_shut_is_abnormal() {
        for d in TYPES {
            let mut r = Rig::new(d, Faults { valve_stuck_shut: true, ..Faults::default() });
            r.inp.master_on = true;
            let t = r.run_until(30.0, |o, _| o.fault == Some(StartFault::StartValveAbnormal));
            assert!(t > d.start.valve_abnormal_s);
        }
    }

    #[test]
    fn in_flight_start_is_never_aborted_and_windmills_above_260_kt() {
        for d in TYPES {
            let mut r = Rig::new(d, Faults { never_lights: true, ..Faults::default() });
            r.inp.on_ground = false;
            r.inp.cas_kt = 300.0;
            r.inp.hp_pct = 22.0;
            r.inp.master_on = true;
            r.tick();
            assert!(r.out.windmill && !r.out.valve_open);
            assert!(r.out.fuel_on, "in flight fuel goes on above 20 % without the rotor-bow crank");
            assert_eq!(r.out.ignition, Ignition::Both);
            r.run_for(120.0);
            assert_eq!(r.out.fault, Some(StartFault::NoLightUp));
            assert!(r.out.fuel_on && r.out.phase == Phase::AutoFuel);

            let mut r = Rig::new(d, Faults::default());
            r.inp.on_ground = false;
            r.inp.cas_kt = 220.0;
            r.inp.hp_pct = 15.0;
            r.inp.master_on = true;
            r.tick();
            assert!(!r.out.windmill && r.out.valve_open, "starter assisted below 260 kt");
        }
    }

    #[test]
    fn manual_start_valve_on_pb_fuel_and_both_igniters_on_master_only_egt_aborts() {
        for d in TYPES {
            let mut r = Rig::new(d, Faults { never_lights: true, ..Faults::default() });
            r.inp.man_start_on = true;
            r.tick();
            assert!(r.out.valve_open && !r.out.fuel_on && r.out.phase == Phase::ManualCrank);
            r.run_until(30.0, |_, i| i.hp_pct >= 22.0);
            r.inp.master_on = true;
            r.tick();
            assert!(r.out.fuel_on && r.out.ignition == Ignition::Both);
            r.run_for(120.0);
            assert_eq!(r.out.phase, Phase::ManualFuel, "no light-up is the crew's to abort in manual");
            r.inp.man_start_on = false;
            r.tick();
            assert_eq!(r.out.phase, Phase::ManualFuel, "pb off with the master on has no effect");

            let mut r = Rig::new(d, Faults { hot: true, ..Faults::default() });
            r.inp.man_start_on = true;
            r.run_until(30.0, |_, i| i.hp_pct >= 22.0);
            r.inp.master_on = true;
            r.run_until(120.0, |o, _| o.phase == Phase::Aborted);
            assert_eq!(r.out.fault, Some(StartFault::HotStart));
            assert!(!r.out.fuel_on && !r.out.valve_open);

            let mut r = Rig::new(d, Faults::default());
            r.inp.man_start_on = true;
            r.run_until(30.0, |_, i| i.hp_pct >= 22.0);
            r.inp.master_on = true;
            r.run_until(120.0, |o, _| o.phase == Phase::Running);
            assert!(r.inp.hp_pct >= d.start.manual_valve_close_hp_pct);
        }
    }

    #[test]
    fn manual_pb_off_before_master_stops_the_crank() {
        let mut r = Rig::new(&TRENT_900, Faults::default());
        r.inp.man_start_on = true;
        r.run_for(3.0);
        r.inp.man_start_on = false;
        r.tick();
        assert!(!r.out.valve_open && r.out.phase == Phase::Off);
    }

    #[test]
    fn dry_and_wet_crank() {
        for d in TYPES {
            let mut r = Rig::new(d, Faults::default());
            r.inp.selector = StartSelector::Crank;
            r.inp.man_start_on = true;
            r.run_for(5.0);
            assert!(r.out.valve_open && !r.out.fuel_on && r.out.ignition == Ignition::Off);
            r.inp.master_on = true;
            r.tick();
            assert_eq!(r.out.phase, Phase::CrankWet);
            assert!(r.out.fuel_on && r.out.ignition == Ignition::Off && r.out.valve_open);
            r.inp.selector = StartSelector::Norm;
            r.tick();
            assert!(!r.out.fuel_on && !r.out.valve_open);
        }
    }

    #[test]
    fn quick_relight_within_30_s_above_45_pct() {
        for d in TYPES {
            let mut r = Rig::new(d, Faults::default());
            r.seq = StartSequencer::running();
            r.inp = StartInputs { master_on: true, hp_pct: d.idle_hp_pct, lit: true, egt_c: d.idle_egt_c, lp_pct: d.idle_lp_pct, ..cold() };
            r.out = r.seq.step(d, &r.inp, DT);
            r.inp.master_on = false;
            r.run_for(4.0);
            r.inp.master_on = true;
            r.tick();
            assert_eq!(r.out.phase, Phase::QuickRelight);
            assert!(r.out.fuel_on && r.out.ignition == Ignition::Both && !r.out.valve_open);
            let held = r.run_until(70.0, |o, _| o.ignition == Ignition::Off);
            assert!((held - d.start.relight_ignition_hold_s).abs() < 0.3, "{held}");

            let mut r = Rig::new(d, Faults::default());
            r.seq = StartSequencer::running();
            r.inp = StartInputs { master_on: true, hp_pct: d.idle_hp_pct, lit: true, ..cold() };
            r.out = r.seq.step(d, &r.inp, DT);
            r.inp.master_on = false;
            r.run_for(31.0);
            r.inp.master_on = true;
            r.tick();
            assert_ne!(r.out.phase, Phase::QuickRelight);
        }
    }

    #[test]
    fn auto_relight_holds_both_igniters_60_s_after_relight() {
        for d in TYPES {
            let mut seq = StartSequencer::running();
            let mut i = StartInputs { selector: StartSelector::Norm, master_on: true, on_ground: false, cas_kt: 280.0, hp_pct: d.idle_hp_pct, lp_pct: d.idle_lp_pct, egt_c: 500.0, lit: true, ..cold() };
            seq.step(d, &i, DT);
            i.lit = false;
            assert_eq!(seq.step(d, &i, DT).ignition, Ignition::Both);
            for _ in 0..50 {
                seq.step(d, &i, DT);
            }
            i.lit = true;
            let mut held = 0.0;
            while seq.step(d, &i, DT).ignition == Ignition::Both {
                held += DT;
                assert!(held < 70.0);
            }
            assert!((held - d.start.relight_ignition_hold_s).abs() < 0.3, "{held}");
        }
    }

    #[test]
    fn master_off_resets_an_auto_start_and_selector_moves_with_master_on_do_nothing() {
        for d in TYPES {
            let mut r = Rig::new(d, Faults::default());
            r.inp.master_on = true;
            r.run_for(5.0);
            r.inp.selector = StartSelector::Norm;
            r.tick();
            assert_eq!(r.out.phase, Phase::AutoCrank);
            r.inp.master_on = false;
            r.tick();
            assert!(r.out.phase == Phase::Off && !r.out.valve_open && !r.out.fuel_on);
        }
    }
}

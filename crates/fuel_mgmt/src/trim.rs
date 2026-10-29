use crate::model::{capacity_kg, Kg, Tank, TANK_COUNT};

pub fn default_cg_target_pct(gw_kg: f64) -> f64 {
    const PTS: [(f64, f64); 5] = [(250_000.0, 40.0), (350_000.0, 41.0), (450_000.0, 41.5), (550_000.0, 41.9), (600_000.0, 42.0)];
    if gw_kg <= PTS[0].0 {
        return PTS[0].1;
    }
    for w in PTS.windows(2) {
        if gw_kg <= w[1].0 {
            let f = (gw_kg - w[0].0) / (w[1].0 - w[0].0);
            return w[0].1 + f * (w[1].1 - w[0].1);
        }
    }
    PTS[PTS.len() - 1].1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightPhase {
    Ground,
    Takeoff,
    Climb,
    Cruise,
    Descent,
    Approach,
    Landing,
}

impl FlightPhase {
    fn airborne(self) -> bool {
        !matches!(self, FlightPhase::Ground | FlightPhase::Landing)
    }

    fn aft_allowed(self) -> bool {
        matches!(self, FlightPhase::Climb | FlightPhase::Cruise)
    }

    fn landing_prep(self) -> bool {
        matches!(self, FlightPhase::Descent | FlightPhase::Approach)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManualTransfer {
    Off,
    Forward,
    Aft,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrimMode {
    Idle,
    Forward,
    Aft,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrimReason {
    None,
    CgAftOfTarget,
    CgForwardOfTarget,
    LandingPreparation,
    LowFeedTank,
    Manual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrimInhibit {
    OnGround,
    Refuelling,
    Jettison,
    NoPower,
    CgInvalid,
    AutoOff,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TrimFaults {
    pub pump_failed: [bool; 2],
    pub inlet_valve_failed: [bool; 2],
    pub iso_aft_failed: bool,
}

#[derive(Clone, Debug)]
pub struct TrimConfig {
    pub target_fn: fn(f64) -> f64,
    pub fwd_start_pct: f64,
    pub fwd_stop_pct: f64,
    pub aft_start_pct: f64,
    pub aft_stop_pct: f64,
    pub taper_pct: f64,
    pub fwd_rate_per_pump_kg_s: f64,
    pub aft_rate_kg_s: f64,
    pub trim_empty_kg: f64,
    pub trim_full_margin_kg: f64,
    pub inner_reserve_kg: f64,
    pub feed_low_kg: f64,
    pub cg_fwd_limit_pct: f64,
    pub cg_aft_limit_pct: f64,
    pub fault_confirm_s: f64,
}

impl Default for TrimConfig {
    fn default() -> Self {
        Self {
            target_fn: default_cg_target_pct,
            fwd_start_pct: 0.0,
            fwd_stop_pct: -0.7,
            aft_start_pct: -1.7,
            aft_stop_pct: -0.5,
            taper_pct: 0.3,
            fwd_rate_per_pump_kg_s: 8.3,
            aft_rate_kg_s: 13.3,
            trim_empty_kg: 20.0,
            trim_full_margin_kg: 20.0,
            inner_reserve_kg: 1000.0,
            feed_low_kg: 4000.0,
            cg_fwd_limit_pct: 28.0,
            cg_aft_limit_pct: 43.5,
            fault_confirm_s: 10.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TrimInputs {
    pub tanks: Kg,
    pub gw_kg: f64,
    pub cg_pct: Option<f64>,
    pub phase: FlightPhase,
    pub refuel_active: bool,
    pub jettison_active: bool,
    pub auto_on: bool,
    pub pump_sw_on: [bool; 2],
    pub manual: ManualTransfer,
    pub faults: TrimFaults,
    pub power_ok: bool,
}

impl Default for TrimInputs {
    fn default() -> Self {
        Self {
            tanks: [0.0; TANK_COUNT],
            gw_kg: 0.0,
            cg_pct: None,
            phase: FlightPhase::Cruise,
            refuel_active: false,
            jettison_active: false,
            auto_on: true,
            pump_sw_on: [true; 2],
            manual: ManualTransfer::Off,
            faults: TrimFaults::default(),
            power_ok: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TrimOutput {
    pub mode: TrimMode,
    pub reason: TrimReason,
    pub inhibit: Option<TrimInhibit>,
    pub cg_target_pct: f64,
    pub trim_pump_cmd: [bool; 2],
    pub inlet_valve_cmd: [bool; 2],
    pub iso_fwd_cmd: bool,
    pub iso_aft_cmd: bool,
    pub aft_source_pumps_on: bool,
    pub rate_kg_s: f64,
    pub flow_kg_s: Kg,
    pub trim_empty: bool,
    pub trim_full: bool,
    pub alert_auto_xfr_fault: bool,
    pub alert_xfr_degraded: bool,
    pub alert_cg_out_of_limits: bool,
    pub memo_trim_xfr: bool,
}

impl TrimOutput {
    pub fn delta_kg(&self, dt_s: f64) -> Kg {
        let mut d = [0.0; TANK_COUNT];
        for i in 0..TANK_COUNT {
            d[i] = self.flow_kg_s[i] * dt_s;
        }
        d
    }
}

#[derive(Clone, Debug, Default)]
pub struct TrimController {
    cfg: TrimConfig,
    mode: Option<TrimMode>,
    reason: Option<TrimReason>,
    fault_s: f64,
    degraded_s: f64,
}

impl TrimController {
    pub fn new(cfg: TrimConfig) -> Self {
        Self { cfg, ..Default::default() }
    }

    pub fn config(&self) -> &TrimConfig {
        &self.cfg
    }

    fn inhibit(&self, i: &TrimInputs) -> Option<TrimInhibit> {
        if !i.phase.airborne() {
            return Some(TrimInhibit::OnGround);
        }
        if i.refuel_active {
            return Some(TrimInhibit::Refuelling);
        }
        if i.jettison_active {
            return Some(TrimInhibit::Jettison);
        }
        if !i.power_ok {
            return Some(TrimInhibit::NoPower);
        }
        if i.manual == ManualTransfer::Off {
            if !i.auto_on {
                return Some(TrimInhibit::AutoOff);
            }
            if i.cg_pct.is_none() {
                return Some(TrimInhibit::CgInvalid);
            }
        }
        None
    }

    fn aft_source(&self, i: &TrimInputs) -> [(Tank, f64); 4] {
        let q = |t: Tank| (i.tanks[t.idx()] - self.cfg.inner_reserve_kg).max(0.0);
        let inner = [q(Tank::LeftInner), q(Tank::RightInner)];
        if inner[0] + inner[1] > 0.0 {
            return [(Tank::LeftInner, inner[0]), (Tank::RightInner, inner[1]), (Tank::LeftMid, 0.0), (Tank::RightMid, 0.0)];
        }
        [(Tank::LeftInner, 0.0), (Tank::RightInner, 0.0), (Tank::LeftMid, q(Tank::LeftMid)), (Tank::RightMid, q(Tank::RightMid))]
    }

    fn decide(&mut self, i: &TrimInputs, target: f64, trim_q: f64, trim_cap: f64) -> (TrimMode, TrimReason) {
        let prev_mode = self.mode.unwrap_or(TrimMode::Idle);
        let has_trim = trim_q > self.cfg.trim_empty_kg;
        let has_room = trim_q < trim_cap - self.cfg.trim_full_margin_kg;
        let source: f64 = self.aft_source(i).iter().map(|s| s.1).sum();

        match i.manual {
            ManualTransfer::Forward => {
                let cg_ok = i.cg_pct.map_or(true, |c| c > self.cfg.cg_fwd_limit_pct);
                return if has_trim && cg_ok { (TrimMode::Forward, TrimReason::Manual) } else { (TrimMode::Idle, TrimReason::None) };
            }
            ManualTransfer::Aft => {
                let cg_ok = i.cg_pct.map_or(false, |c| c < target && c < self.cfg.cg_aft_limit_pct);
                return if has_room && source > 0.0 && cg_ok { (TrimMode::Aft, TrimReason::Manual) } else { (TrimMode::Idle, TrimReason::None) };
            }
            ManualTransfer::Off => {}
        }

        let feed_low = Tank::FEED.iter().any(|t| i.tanks[t.idx()] < self.cfg.feed_low_kg);
        if has_trim && feed_low && prev_mode != TrimMode::Aft {
            return (TrimMode::Forward, TrimReason::LowFeedTank);
        }
        if i.phase.landing_prep() {
            return if has_trim { (TrimMode::Forward, TrimReason::LandingPreparation) } else { (TrimMode::Idle, TrimReason::None) };
        }

        let cg = match i.cg_pct {
            Some(c) => c,
            None => return (TrimMode::Idle, TrimReason::None),
        };
        let rel = cg - target;
        match prev_mode {
            TrimMode::Forward => {
                if has_trim && rel > self.cfg.fwd_stop_pct {
                    return (TrimMode::Forward, TrimReason::CgAftOfTarget);
                }
            }
            TrimMode::Aft => {
                if has_room && source > 0.0 && rel < self.cfg.aft_stop_pct && i.phase.aft_allowed() {
                    return (TrimMode::Aft, TrimReason::CgForwardOfTarget);
                }
            }
            TrimMode::Idle => {}
        }
        if has_trim && rel > self.cfg.fwd_start_pct {
            return (TrimMode::Forward, TrimReason::CgAftOfTarget);
        }
        if i.phase.aft_allowed() && has_room && source > 0.0 && rel < self.cfg.aft_start_pct && cg < self.cfg.cg_aft_limit_pct {
            return (TrimMode::Aft, TrimReason::CgForwardOfTarget);
        }
        (TrimMode::Idle, TrimReason::None)
    }

    pub fn update(&mut self, i: &TrimInputs, dt_s: f64) -> TrimOutput {
        let dt = dt_s.max(0.0);
        let cap = capacity_kg();
        let target = (self.cfg.target_fn)(i.gw_kg);
        let trim_q = i.tanks[Tank::Trim.idx()];
        let trim_cap = cap[Tank::Trim.idx()];
        let inhibit = self.inhibit(i);

        let (mode, reason) = if inhibit.is_some() { (TrimMode::Idle, TrimReason::None) } else { self.decide(i, target, trim_q, trim_cap) };
        self.mode = Some(mode);
        self.reason = Some(reason);

        let pumps_ok = [i.pump_sw_on[0] && !i.faults.pump_failed[0], i.pump_sw_on[1] && !i.faults.pump_failed[1]];
        let inlets_ok = [!i.faults.inlet_valve_failed[0], !i.faults.inlet_valve_failed[1]];
        let n_pumps = pumps_ok.iter().filter(|p| **p).count() as f64;
        let n_inlets = inlets_ok.iter().filter(|p| **p).count() as f64;
        let iso_ok = !i.faults.iso_aft_failed;

        let nominal = match mode {
            TrimMode::Forward => self.cfg.fwd_rate_per_pump_kg_s * 2.0,
            TrimMode::Aft => self.cfg.aft_rate_kg_s,
            TrimMode::Idle => 0.0,
        };
        let capability = match mode {
            TrimMode::Forward if iso_ok => self.cfg.fwd_rate_per_pump_kg_s * n_pumps,
            TrimMode::Aft if iso_ok => self.cfg.aft_rate_kg_s * n_inlets / 2.0,
            _ => 0.0,
        };

        let taper = match (mode, reason, i.cg_pct) {
            (TrimMode::Forward, TrimReason::CgAftOfTarget, Some(cg)) => ((cg - (target + self.cfg.fwd_stop_pct)) / self.cfg.taper_pct).clamp(0.15, 1.0),
            (TrimMode::Aft, TrimReason::CgForwardOfTarget, Some(cg)) => (((target + self.cfg.aft_stop_pct) - cg) / self.cfg.taper_pct).clamp(0.15, 1.0),
            _ => 1.0,
        };

        let mut flow = [0.0; TANK_COUNT];
        let mut rate = capability * taper;
        match mode {
            TrimMode::Forward => {
                let mut weights = [0.0; 4];
                for (k, t) in Tank::FEED.iter().enumerate() {
                    let room = cap[t.idx()] - i.tanks[t.idx()];
                    weights[k] = if room > 1.0 { room } else { 0.0 };
                }
                let wsum: f64 = weights.iter().sum();
                if wsum <= 0.0 {
                    rate = 0.0;
                }
                if dt > 0.0 {
                    rate = rate.min(trim_q / dt).min(wsum / dt);
                }
                if rate > 0.0 {
                    flow[Tank::Trim.idx()] = -rate;
                    for (k, t) in Tank::FEED.iter().enumerate() {
                        flow[t.idx()] = rate * weights[k] / wsum;
                    }
                }
            }
            TrimMode::Aft => {
                let src = self.aft_source(i);
                let ssum: f64 = src.iter().map(|s| s.1).sum();
                if dt > 0.0 {
                    rate = rate.min(ssum / dt).min((trim_cap - trim_q) / dt);
                }
                if rate > 0.0 && ssum > 0.0 {
                    flow[Tank::Trim.idx()] = rate;
                    for (t, q) in src {
                        flow[t.idx()] -= rate * q / ssum;
                    }
                }
            }
            TrimMode::Idle => {}
        }

        let transferring = mode != TrimMode::Idle;
        let wanted_stuck = transferring && capability <= 0.0;
        self.fault_s = if wanted_stuck { self.fault_s + dt } else { 0.0 };
        let degraded = transferring && capability > 0.0 && capability < nominal * 0.99;
        self.degraded_s = if degraded { self.degraded_s + dt } else { 0.0 };

        let cg_bad = i.cg_pct.map_or(false, |c| c > self.cfg.cg_aft_limit_pct || c < self.cfg.cg_fwd_limit_pct);

        TrimOutput {
            mode,
            reason,
            inhibit,
            cg_target_pct: target,
            trim_pump_cmd: [mode == TrimMode::Forward && pumps_ok[0], mode == TrimMode::Forward && pumps_ok[1]],
            inlet_valve_cmd: [mode == TrimMode::Aft && inlets_ok[0], mode == TrimMode::Aft && inlets_ok[1]],
            iso_fwd_cmd: false,
            iso_aft_cmd: transferring && iso_ok,
            aft_source_pumps_on: mode == TrimMode::Aft && rate > 0.0,
            rate_kg_s: rate,
            flow_kg_s: flow,
            trim_empty: trim_q <= self.cfg.trim_empty_kg,
            trim_full: trim_q >= trim_cap - self.cfg.trim_full_margin_kg,
            alert_auto_xfr_fault: self.fault_s >= self.cfg.fault_confirm_s,
            alert_xfr_degraded: self.degraded_s >= self.cfg.fault_confirm_s,
            alert_cg_out_of_limits: cg_bad,
            memo_trim_xfr: transferring && rate > 0.0,
        }
    }
}

use crate::model::{moved, total_kg, Kg, Tank, TANK_COUNT};

pub const MLW_KG: f64 = 391_000.0;

#[derive(Clone, Debug)]
pub struct JettisonConfig {
    pub max_side_rate_kg_s: f64,
    pub valve_travel_s: f64,
    pub taper_kg: f64,
    pub min_total_fuel_kg: f64,
    pub default_target_gw_kg: f64,
    pub max_flap_config: u8,
    pub imbalance_limit_kg: f64,
    pub fault_confirm_s: f64,
    pub valve_not_closed_confirm_s: f64,
    pub trim_weight: f64,
    pub outer_after_kg: f64,
    pub cg_bias_band_pct: f64,
}

impl Default for JettisonConfig {
    fn default() -> Self {
        Self {
            max_side_rate_kg_s: 20.8,
            valve_travel_s: 5.0,
            taper_kg: 4000.0,
            min_total_fuel_kg: 10_000.0,
            default_target_gw_kg: MLW_KG,
            max_flap_config: 1,
            imbalance_limit_kg: 3000.0,
            fault_confirm_s: 8.0,
            valve_not_closed_confirm_s: 10.0,
            trim_weight: 0.6,
            outer_after_kg: 3000.0,
            cg_bias_band_pct: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct JettisonInputs {
    pub arm_pb: bool,
    pub active_pb: bool,
    pub target_gw_kg: Option<f64>,
    pub gw_kg: f64,
    pub tanks: Kg,
    pub on_ground: bool,
    pub flap_config: u8,
    pub power_ok: bool,
    pub valve_failed_closed: [bool; 2],
    pub valve_stuck_open: [bool; 2],
    pub pumps_available: [u8; 2],
    pub cg_pct: Option<f64>,
    pub cg_target_pct: Option<f64>,
}

impl Default for JettisonInputs {
    fn default() -> Self {
        Self {
            arm_pb: false,
            active_pb: false,
            target_gw_kg: None,
            gw_kg: 0.0,
            tanks: [0.0; TANK_COUNT],
            on_ground: false,
            flap_config: 0,
            power_ok: true,
            valve_failed_closed: [false; 2],
            valve_stuck_open: [false; 2],
            pumps_available: [2; 2],
            cg_pct: None,
            cg_target_pct: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JettisonInhibit {
    OnGround,
    FlapsExtended,
    NoPower,
    MinFuel,
    AtTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JettisonState {
    Off,
    Armed,
    Active,
    Completed,
    Stopped(JettisonInhibit),
}

#[derive(Clone, Debug)]
pub struct JettisonOutput {
    pub state: JettisonState,
    pub inhibit: Option<JettisonInhibit>,
    pub valve_cmd: [bool; 2],
    pub valve_pos: [f64; 2],
    pub flow_kg_s: [f64; 2],
    pub draw_kg_s: Kg,
    pub aft_pumps_on: [bool; 2],
    pub trim_pumps_on: bool,
    pub remaining_to_shed_kg: f64,
    pub completed_short: bool,
    pub arm_lamp: bool,
    pub active_lamp: bool,
    pub fault_lamp: bool,
    pub memo_jettison: bool,
    pub memo_completed: bool,
    pub alert_fault: bool,
    pub alert_valve_not_closed: bool,
    pub inhibits_auto_trim: bool,
}

impl JettisonOutput {
    pub fn delta_kg(&self, dt_s: f64) -> Kg {
        let mut d = [0.0; TANK_COUNT];
        for i in 0..TANK_COUNT {
            d[i] = -self.draw_kg_s[i] * dt_s;
        }
        d
    }
}

#[derive(Clone, Debug, Default)]
pub struct JettisonController {
    cfg: JettisonConfig,
    state: Option<JettisonState>,
    valve_pos: [f64; 2],
    last_flow: [f64; 2],
    completed_short: bool,
    fault_s: f64,
    not_closed_s: f64,
}

impl JettisonController {
    pub fn new(cfg: JettisonConfig) -> Self {
        Self { cfg, ..Default::default() }
    }

    pub fn config(&self) -> &JettisonConfig {
        &self.cfg
    }

    fn state(&self) -> JettisonState {
        self.state.unwrap_or(JettisonState::Off)
    }

    fn inhibit(&self, i: &JettisonInputs, target: f64) -> Option<JettisonInhibit> {
        if i.on_ground {
            return Some(JettisonInhibit::OnGround);
        }
        if !i.power_ok {
            return Some(JettisonInhibit::NoPower);
        }
        if i.flap_config > self.cfg.max_flap_config {
            return Some(JettisonInhibit::FlapsExtended);
        }
        if total_kg(&i.tanks) <= self.cfg.min_total_fuel_kg {
            return Some(JettisonInhibit::MinFuel);
        }
        if i.gw_kg <= target {
            return Some(JettisonInhibit::AtTarget);
        }
        None
    }

    fn side_tanks(side: usize) -> (Tank, Tank, Tank) {
        if side == 0 {
            (Tank::LeftInner, Tank::LeftMid, Tank::LeftOuter)
        } else {
            (Tank::RightInner, Tank::RightMid, Tank::RightOuter)
        }
    }

    fn side_weights(&self, i: &JettisonInputs, side: usize) -> ([(Tank, f64); 4], f64) {
        let (inner, mid, outer) = Self::side_tanks(side);
        let q = |t: Tank| i.tanks[t.idx()].max(0.0);
        let mut bias = 1.0;
        if let (Some(cg), Some(target)) = (i.cg_pct, i.cg_target_pct) {
            if cg > target + self.cfg.cg_bias_band_pct {
                bias = 2.5;
            } else if cg < target - self.cfg.cg_bias_band_pct {
                bias = 0.2;
            }
        }
        let trim_share = 0.5 * q(Tank::Trim);
        let main = q(inner) + q(mid) + trim_share;
        let outer_w = if main < self.cfg.outer_after_kg { q(outer) } else { 0.0 };
        let w = [
            (inner, q(inner)),
            (mid, q(mid)),
            (Tank::Trim, trim_share * self.cfg.trim_weight * bias),
            (outer, outer_w),
        ];
        let avail = q(inner) + q(mid) + trim_share + outer_w;
        (w, avail)
    }

    pub fn update(&mut self, i: &JettisonInputs, dt_s: f64) -> JettisonOutput {
        let dt = dt_s.max(0.0);
        let target = i.target_gw_kg.unwrap_or(self.cfg.default_target_gw_kg);
        let prev = self.state();
        let mut inhibit = None;
        let mut state = prev;

        if !i.arm_pb {
            state = JettisonState::Off;
            self.completed_short = false;
        } else if !i.active_pb {
            state = JettisonState::Armed;
            self.completed_short = false;
        } else {
            match prev {
                JettisonState::Off | JettisonState::Armed => {
                    inhibit = self.inhibit(i, target);
                    state = if inhibit.is_none() { JettisonState::Active } else { JettisonState::Armed };
                }
                JettisonState::Active => {
                    inhibit = self.inhibit(i, target);
                    match inhibit {
                        Some(JettisonInhibit::AtTarget) => state = JettisonState::Completed,
                        Some(JettisonInhibit::MinFuel) => {
                            self.completed_short = true;
                            state = JettisonState::Completed;
                        }
                        Some(r) => state = JettisonState::Stopped(r),
                        None => {
                            let lead: f64 = (0..2).map(|s| self.last_flow[s] * self.cfg.valve_travel_s * 0.5).sum();
                            let remaining = i.gw_kg - target;
                            let fuel_room = total_kg(&i.tanks) - self.cfg.min_total_fuel_kg;
                            let sources: f64 = (0..2).map(|side| self.side_weights(i, side).1).sum();
                            if remaining <= lead {
                                state = JettisonState::Completed;
                            } else if fuel_room <= lead || sources < 100.0 {
                                self.completed_short = true;
                                state = JettisonState::Completed;
                            }
                        }
                    }
                }
                JettisonState::Completed | JettisonState::Stopped(_) => {}
            }
        }
        self.state = Some(state);

        let active = state == JettisonState::Active;
        let valve_cmd = [active; 2];
        let step = if self.cfg.valve_travel_s > 0.0 { dt / self.cfg.valve_travel_s } else { 1.0 };
        for s in 0..2 {
            let before = self.valve_pos[s];
            let tgt = if valve_cmd[s] && !i.valve_failed_closed[s] { 1.0 } else { 0.0 };
            let mut p = moved(before, tgt, step);
            if i.valve_failed_closed[s] {
                p = before.min(p);
            }
            if i.valve_stuck_open[s] {
                p = p.max(before);
            }
            self.valve_pos[s] = p.clamp(0.0, 1.0);
        }

        let left_total: f64 = Tank::ALL.iter().filter(|t| t.is_left()).map(|t| i.tanks[t.idx()]).sum();
        let right_total: f64 = Tank::ALL.iter().filter(|t| t.is_right()).map(|t| i.tanks[t.idx()]).sum();
        let imb = left_total - right_total;
        let lat = |side: usize| -> f64 {
            let heavier = if side == 0 { imb } else { -imb };
            if heavier >= 0.0 || i.valve_failed_closed[1 - side] {
                1.0
            } else {
                (1.0 - (-heavier) / self.cfg.imbalance_limit_kg).clamp(0.3, 1.0)
            }
        };

        let mut flow = [0.0; 2];
        let mut draw = [0.0; TANK_COUNT];
        for s in 0..2 {
            let pump_factor = match i.pumps_available[s] {
                0 => 0.3,
                1 => 0.65,
                _ => 1.0,
            };
            let (weights, avail) = self.side_weights(i, s);
            let taper = (avail / self.cfg.taper_kg).clamp(0.0, 1.0);
            let f = self.cfg.max_side_rate_kg_s * self.valve_pos[s] * pump_factor * lat(s) * taper;
            let wsum: f64 = weights.iter().map(|w| w.1).sum();
            if f > 0.0 && wsum > 1e-6 {
                flow[s] = f;
                for (t, w) in weights {
                    draw[t.idx()] += f * w / wsum;
                }
            }
        }
        self.last_flow = flow;

        let excess = (i.gw_kg - target).max(0.0);
        let any_closed_fail = (0..2).any(|s| valve_cmd[s] && i.valve_failed_closed[s]);
        let pumps_lost = active && i.pumps_available == [0, 0];
        self.fault_s = if any_closed_fail || pumps_lost { self.fault_s + dt } else { 0.0 };
        let stuck = (0..2).any(|s| !valve_cmd[s] && self.valve_pos[s] > 0.1 && i.valve_stuck_open[s]);
        self.not_closed_s = if stuck { self.not_closed_s + dt } else { 0.0 };

        let alert_fault = self.fault_s >= self.cfg.fault_confirm_s;
        let alert_vnc = self.not_closed_s >= self.cfg.valve_not_closed_confirm_s;

        JettisonOutput {
            state,
            inhibit,
            valve_cmd,
            valve_pos: self.valve_pos,
            flow_kg_s: flow,
            draw_kg_s: draw,
            aft_pumps_on: [active && i.pumps_available[0] > 0, active && i.pumps_available[1] > 0],
            trim_pumps_on: active && draw[Tank::Trim.idx()] > 0.0,
            remaining_to_shed_kg: excess,
            completed_short: self.completed_short,
            arm_lamp: i.arm_pb,
            active_lamp: active,
            fault_lamp: alert_fault,
            memo_jettison: active,
            memo_completed: state == JettisonState::Completed,
            alert_fault,
            alert_valve_not_closed: alert_vnc,
            inhibits_auto_trim: active,
        }
    }
}

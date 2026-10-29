use crate::model::*;
use crate::types::*;
use crate::vars;

pub struct RowRop {
    cfg: Config,
    accel: f64,
    accel_init: bool,
    time_on_ground: f64,
    row_dry: bool,
    row_wet: bool,
    rop_latch: bool,
    rop_hold: f64,
    keep_memory: bool,
    keep_prev: bool,
    phase10_prev: bool,
    bounce_left: f64,
}

impl Default for RowRop {
    fn default() -> Self {
        RowRop::new(Config::default())
    }
}

impl RowRop {
    pub fn new(cfg: Config) -> RowRop {
        RowRop {
            cfg,
            accel: 0.,
            accel_init: false,
            time_on_ground: 0.,
            row_dry: false,
            row_wet: false,
            rop_latch: false,
            rop_hold: 0.,
            keep_memory: false,
            keep_prev: false,
            phase10_prev: false,
            bounce_left: 0.,
        }
    }

    pub fn config(&self) -> &Config {
        &self.cfg
    }

    pub fn reset(&mut self) {
        *self = RowRop::new(self.cfg);
    }

    fn filter_decel(&mut self, dt: f64, ac: &Aircraft) {
        let raw = if ac.on_ground { -ac.long_accel_ms2 } else { 0. };
        if !self.accel_init || !ac.on_ground {
            self.accel = raw;
            self.accel_init = true;
        } else {
            let alpha = dt / (self.cfg.accel_filter_s + dt);
            self.accel += (raw - self.accel) * alpha;
        }
    }

    pub fn update(&mut self, dt: f64, input: &Inputs) -> Outputs {
        let cfg = self.cfg;
        let dt = dt.max(0.);
        let ac = &input.aircraft;
        self.filter_decel(dt, ac);
        if ac.on_ground {
            self.time_on_ground += dt;
        } else {
            self.time_on_ground = 0.;
        }

        let phase10 = ac.flight_phase == 10;
        if self.phase10_prev && !phase10 {
            self.bounce_left = cfg.bounce_window_s;
        } else {
            self.bounce_left = (self.bounce_left - dt).max(0.);
        }
        self.phase10_prev = phase10;
        let rollout_or_bounce = ac.flight_phase == 11
            || phase10
            || (self.bounce_left > 0. && (ac.flight_phase == 8 || ac.flight_phase == 9));

        let mut out = Outputs::default();
        let runway = match (input.runway, ac.position_valid) {
            (Some(r), true) => r,
            _ => {
                self.clear_latches();
                out.status_valid = false;
                out.status_word = 0;
                return out;
            }
        };
        let remaining = runway.lda_m - ac.dist_from_threshold_m - cfg.nose_offset_m;
        out.remaining_to_end_m = remaining;

        let vs_ok = ac.vertical_speed_fpm <= cfg.row_max_vs_fpm;
        let in_window = !ac.on_ground
            && ac.gear_down
            && vs_ok
            && ac.radio_alt_ft <= cfg.row_window_ft
            && ac.radio_alt_ft >= cfg.row_floor_ft;
        out.row_operative = !input.row_lost;
        out.rop_operative = !input.rop_lost
            && ac.on_ground
            && ac.ground_speed_kt * KT_TO_MS > cfg.rop_arming_speed_ms
            && input.braking.autobrake_mode != AutobrakeMode::Rto;

        self.update_row(&mut out, input, &runway, remaining, in_window);
        self.update_rop(dt, &mut out, input, &runway, remaining);
        self.gate_fws(&mut out, input, rollout_or_bounce);
        out
    }

    fn clear_latches(&mut self) {
        self.row_dry = false;
        self.row_wet = false;
        self.rop_latch = false;
        self.rop_hold = 0.;
    }

    fn roll_for(&self, ac: &Aircraft, runway: &Runway, cond: RunwayCondition, speed_ms: f64) -> Roll {
        Roll {
            speed_ms,
            condition: cond,
            slope_pct: runway.slope_pct,
            weight_kg: ac.weight_kg,
            reversers: 0,
            reverse_level: 0.,
            spoilers: true,
        }
    }

    fn update_row(&mut self, out: &mut Outputs, input: &Inputs, runway: &Runway, remaining: f64, in_window: bool) {
        let cfg = self.cfg;
        let ac = &input.aircraft;
        let wet_cond = runway.condition.max(RunwayCondition::Wet);
        let td_speed = touchdown_ground_speed_ms(&cfg, ac.tas_kt, ac.headwind_kt);
        let d_air = if ac.on_ground { 0. } else { air_distance_m(&cfg, ac.radio_alt_ft) };
        let actual = ground_roll_distance(&cfg, &self.roll_for(ac, runway, runway.condition, td_speed));
        let wet = ground_roll_distance(&cfg, &self.roll_for(ac, runway, wet_cond, td_speed));
        let dry = ground_roll_distance(&cfg, &self.roll_for(ac, runway, RunwayCondition::Dry, td_speed));
        if !ac.on_ground && td_speed > 15. {
            out.dry_stop_point_m = d_air + dry;
            out.wet_stop_point_m = d_air + wet;
        }

        if !in_window || input.row_lost {
            self.row_dry = false;
            self.row_wet = false;
            return;
        }
        let avail = remaining - d_air;
        let margin = |latched: bool| if latched { cfg.row_release_margin_m } else { 0. };
        let too_short = actual >= avail - margin(self.row_dry);
        let if_wet = runway.condition == RunwayCondition::Dry
            && !too_short
            && wet >= avail - margin(self.row_wet);
        self.row_dry = too_short;
        self.row_wet = if_wet;
        out.rwy_too_short = too_short;
        out.if_wet_rwy_too_short = if_wet;
    }

    fn update_rop(&mut self, dt: f64, out: &mut Outputs, input: &Inputs, runway: &Runway, remaining: f64) {
        let cfg = self.cfg;
        let ac = &input.aircraft;
        if !out.rop_operative {
            if !ac.on_ground || input.rop_lost || input.braking.autobrake_mode == AutobrakeMode::Rto {
                self.rop_latch = false;
                self.rop_hold = 0.;
            }
            out.auto_max_braking = self.rop_latch;
            return;
        }
        let v = ac.ground_speed_kt * KT_TO_MS;
        let cap_roll = Roll {
            speed_ms: v,
            condition: runway.condition,
            slope_pct: runway.slope_pct,
            weight_kg: ac.weight_kg,
            reversers: input.reversers.inboard_available.iter().filter(|a| **a).count() as u8,
            reverse_level: 1.,
            spoilers: true,
        };
        let cap = decel_at(&cfg, &cap_roll, v.max(cfg.reverser_full_kt * KT_TO_MS));
        let in_transition = self.time_on_ground < cfg.transition_s;
        let decel = if in_transition {
            self.accel.max(cfg.rop_assumed_brake_fraction * cap)
        } else {
            self.accel
        };
        let roll_left = if in_transition { cfg.transition_s - self.time_on_ground } else { 0. };
        let predicted = predicted_stop_distance(&cfg, v, decel, roll_left);
        let at_cap = braking_distance(&cfg, &cap_roll);
        out.stop_bar_m = if self.accel > cfg.min_decel { predicted + cfg.stop_bar_offset_m } else { 0. };

        let detected = predicted >= remaining;
        let fwd_thrust = input.reversers.tla.iter().any(|t| *t > cfg.idle_tla);
        out.overrun_detected = detected && !fwd_thrust;

        if self.rop_latch {
            self.rop_hold += dt;
            let recovered = self.rop_hold >= cfg.rop_hold_s && at_cap * cfg.rop_release_ratio < remaining;
            if !input.braking.autobrake_active || recovered || fwd_thrust {
                self.rop_latch = false;
                self.rop_hold = 0.;
            }
        } else if out.overrun_detected && input.braking.autobrake_active {
            self.rop_latch = true;
            self.rop_hold = 0.;
        }
        out.auto_max_braking = self.rop_latch;
        out.btv_overridden = self.rop_latch && input.braking.autobrake_mode == AutobrakeMode::Btv;

        let full_reverse = input.reversers.tla[1] <= cfg.full_reverse_tla && input.reversers.tla[2] <= cfg.full_reverse_tla;
        let any_reverser = input.reversers.inboard_available.iter().any(|a| *a);
        let gs_kt = ac.ground_speed_kt;
        let manual_only = !fwd_thrust && !input.braking.autobrake_active;
        out.max_braking_request = out.overrun_detected && manual_only;
        out.set_max_reverse_request =
            out.overrun_detected && !full_reverse && gs_kt >= cfg.max_reverse_request_kt && any_reverser;
        out.keep_max_reverse_request =
            out.overrun_detected && full_reverse && gs_kt <= cfg.max_reverse_request_kt && any_reverser;
    }

    fn gate_fws(&mut self, out: &mut Outputs, input: &Inputs, rollout: bool) {
        let cfg = self.cfg;
        let pedals_max = input.braking.pedal_left_pct > cfg.pedal_max_pct || input.braking.pedal_right_pct > cfg.pedal_max_pct;
        let brake_msg = out.max_braking_request && !pedals_max && rollout;
        let set_rev = out.set_max_reverse_request && rollout && !brake_msg;
        let keep_now = out.keep_max_reverse_request && !brake_msg && rollout;
        let rising = keep_now && !self.keep_prev;
        let falling = !keep_now && self.keep_prev;
        self.keep_prev = keep_now;
        if falling || input.keep_max_reverse_played {
            self.keep_memory = false;
        }
        if rising {
            self.keep_memory = true;
        }
        let phase = input.aircraft.flight_phase;
        let too_short_aural = (8..=10).contains(&phase) && out.rwy_too_short;

        out.aurals = Aurals {
            runway_too_short: too_short_aural,
            max_braking: brake_msg,
            set_max_reverse: set_rev,
            keep_max_reverse: self.keep_memory,
        };

        out.pfd_messages.clear();
        if out.rwy_too_short {
            out.pfd_messages.push(PfdMessage::RwyTooShort);
        } else if out.if_wet_rwy_too_short {
            out.pfd_messages.push(PfdMessage::IfWetRwyTooShort);
        }
        if brake_msg {
            out.pfd_messages.push(PfdMessage::MaxBraking);
        } else if set_rev {
            out.pfd_messages.push(PfdMessage::SetMaxReverse);
        } else if out.keep_max_reverse_request && rollout {
            out.pfd_messages.push(PfdMessage::KeepMaxReverse);
        }

        let mut word = 0u32;
        if out.max_braking_request {
            word |= vars::bit(vars::BIT_MAX_BRAKING);
        }
        if out.set_max_reverse_request {
            word |= vars::bit(vars::BIT_SET_MAX_REVERSE);
        }
        if out.keep_max_reverse_request {
            word |= vars::bit(vars::BIT_KEEP_MAX_REVERSE);
        }
        if out.if_wet_rwy_too_short || out.rwy_too_short {
            word |= vars::bit(vars::BIT_IF_WET_TOO_SHORT);
        }
        if out.rwy_too_short {
            word |= vars::bit(vars::BIT_TOO_SHORT);
        }
        out.status_word = word;
        out.status_valid = out.row_operative && (!input.aircraft.on_ground || out.rop_operative);
        if !out.status_valid {
            out.status_word = 0;
        }
    }
}

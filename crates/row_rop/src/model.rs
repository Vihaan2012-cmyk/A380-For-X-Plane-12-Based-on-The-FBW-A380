pub const G: f64 = 9.80665;
pub const KT_TO_MS: f64 = 0.514_444;
pub const FT_TO_M: f64 = 0.3048;
pub const MAX_STOPPING_DISTANCE_M: f64 = 5000.;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum RunwayCondition {
    Dry,
    Wet,
    Good,
    Medium,
    MediumToPoor,
    Poor,
}

impl RunwayCondition {
    pub fn max_brake_decel(self) -> f64 {
        match self {
            RunwayCondition::Dry => 2.8,
            RunwayCondition::Wet => 1.8,
            RunwayCondition::Good => 1.5,
            RunwayCondition::Medium => 1.1,
            RunwayCondition::MediumToPoor => 0.8,
            RunwayCondition::Poor => 0.55,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub transition_s: f64,
    pub headwind_credit: f64,
    pub tailwind_factor: f64,
    pub no_spoiler_factor: f64,
    pub reverser_thrust_n: f64,
    pub reverser_cutoff_kt: f64,
    pub reverser_full_kt: f64,
    pub approach_path_deg: f64,
    pub min_decel: f64,
    pub accel_filter_s: f64,
    pub row_window_ft: f64,
    pub row_floor_ft: f64,
    pub row_max_vs_fpm: f64,
    pub row_release_margin_m: f64,
    pub rop_arming_speed_ms: f64,
    pub rop_assumed_brake_fraction: f64,
    pub rop_hold_s: f64,
    pub rop_release_ratio: f64,
    pub rop_decel_floor: f64,
    pub max_reverse_request_kt: f64,
    pub full_reverse_tla: f64,
    pub idle_tla: f64,
    pub pedal_max_pct: f64,
    pub bounce_window_s: f64,
    pub nose_offset_m: f64,
    pub stop_bar_offset_m: f64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            transition_s: 5.,
            headwind_credit: 0.5,
            tailwind_factor: 1.5,
            no_spoiler_factor: 0.85,
            reverser_thrust_n: 75_000.,
            reverser_cutoff_kt: 30.,
            reverser_full_kt: 100.,
            approach_path_deg: 3.,
            min_decel: 0.2,
            accel_filter_s: 0.5,
            row_window_ft: 500.,
            row_floor_ft: 50.,
            row_max_vs_fpm: 400.,
            row_release_margin_m: 50.,
            rop_arming_speed_ms: 10.28,
            rop_assumed_brake_fraction: 0.6,
            rop_hold_s: 5.,
            rop_release_ratio: 1.2,
            rop_decel_floor: 0.2,
            max_reverse_request_kt: 80.,
            full_reverse_tla: -20.,
            idle_tla: 2.,
            pedal_max_pct: 90.,
            bounce_window_s: 4.5,
            nose_offset_m: 0.,
            stop_bar_offset_m: 40.,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Roll {
    pub speed_ms: f64,
    pub condition: RunwayCondition,
    pub slope_pct: f64,
    pub weight_kg: f64,
    pub reversers: u8,
    pub reverse_level: f64,
    pub spoilers: bool,
}

pub fn reverser_decel(cfg: &Config, weight_kg: f64, speed_ms: f64, reversers: u8, level: f64) -> f64 {
    if reversers == 0 || level <= 0. || weight_kg <= 0. {
        return 0.;
    }
    let kt = speed_ms / KT_TO_MS;
    let span = (cfg.reverser_full_kt - cfg.reverser_cutoff_kt).max(1.);
    let k = ((kt - cfg.reverser_cutoff_kt) / span).clamp(0., 1.);
    f64::from(reversers) * cfg.reverser_thrust_n * level.clamp(0., 1.) * k / weight_kg
}

pub fn decel_at(cfg: &Config, roll: &Roll, speed_ms: f64) -> f64 {
    let factor = if roll.spoilers { 1. } else { cfg.no_spoiler_factor };
    let brake = roll.condition.max_brake_decel() * factor;
    let slope = G * roll.slope_pct / 100.;
    let rev = reverser_decel(cfg, roll.weight_kg, speed_ms, roll.reversers, roll.reverse_level);
    (brake + slope + rev).max(cfg.min_decel)
}

pub fn braking_distance(cfg: &Config, roll: &Roll) -> f64 {
    let mut v = roll.speed_ms.max(0.);
    let mut dist = 0.;
    let step: f64 = 0.5;
    while v > 0. {
        let dv = step.min(v);
        let mid = v - dv / 2.;
        dist += mid * dv / decel_at(cfg, roll, mid);
        v -= dv;
        if dist >= MAX_STOPPING_DISTANCE_M {
            return MAX_STOPPING_DISTANCE_M;
        }
    }
    dist
}

pub fn ground_roll_distance(cfg: &Config, roll: &Roll) -> f64 {
    (roll.speed_ms * cfg.transition_s + braking_distance(cfg, roll)).min(MAX_STOPPING_DISTANCE_M)
}

pub fn touchdown_ground_speed_ms(cfg: &Config, tas_kt: f64, headwind_kt: f64) -> f64 {
    let wind = if headwind_kt >= 0. {
        headwind_kt * cfg.headwind_credit
    } else {
        headwind_kt * cfg.tailwind_factor
    };
    ((tas_kt - wind) * KT_TO_MS).max(0.)
}

pub fn air_distance_m(cfg: &Config, radio_alt_ft: f64) -> f64 {
    radio_alt_ft.max(0.) * FT_TO_M / cfg.approach_path_deg.to_radians().tan()
}

pub fn predicted_stop_distance(cfg: &Config, speed_ms: f64, decel: f64, roll_left_s: f64) -> f64 {
    let a = decel.max(cfg.rop_decel_floor);
    (speed_ms * roll_left_s.max(0.) + speed_ms * speed_ms / (2. * a)).min(MAX_STOPPING_DISTANCE_M)
}

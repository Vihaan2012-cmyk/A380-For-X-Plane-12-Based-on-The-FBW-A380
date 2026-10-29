use crate::model::RunwayCondition;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AutobrakeMode {
    Off,
    Low,
    L2,
    L3,
    High,
    Rto,
    Btv,
}

impl AutobrakeMode {
    pub fn target_decel(self) -> f64 {
        match self {
            AutobrakeMode::Off => 0.,
            AutobrakeMode::Low => 2.,
            AutobrakeMode::L2 => 2.5,
            AutobrakeMode::L3 => 3.,
            AutobrakeMode::High => 3.5,
            AutobrakeMode::Rto => 6.,
            AutobrakeMode::Btv => 0.,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Runway {
    pub lda_m: f64,
    pub slope_pct: f64,
    pub condition: RunwayCondition,
}

#[derive(Clone, Copy, Debug)]
pub struct Aircraft {
    pub radio_alt_ft: f64,
    pub vertical_speed_fpm: f64,
    pub ground_speed_kt: f64,
    pub tas_kt: f64,
    pub headwind_kt: f64,
    pub long_accel_ms2: f64,
    pub weight_kg: f64,
    pub dist_from_threshold_m: f64,
    pub position_valid: bool,
    pub on_ground: bool,
    pub gear_down: bool,
    pub ground_spoilers: bool,
    pub flight_phase: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct Braking {
    pub pedal_left_pct: f64,
    pub pedal_right_pct: f64,
    pub autobrake_mode: AutobrakeMode,
    pub autobrake_active: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Reversers {
    pub tla: [f64; 4],
    pub inboard_available: [bool; 2],
}

#[derive(Clone, Copy, Debug)]
pub struct Inputs {
    pub runway: Option<Runway>,
    pub aircraft: Aircraft,
    pub braking: Braking,
    pub reversers: Reversers,
    pub row_lost: bool,
    pub rop_lost: bool,
    pub keep_max_reverse_played: bool,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Aurals {
    pub runway_too_short: bool,
    pub max_braking: bool,
    pub set_max_reverse: bool,
    pub keep_max_reverse: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Amber,
    Red,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PfdMessage {
    RwyTooShort,
    IfWetRwyTooShort,
    MaxBraking,
    SetMaxReverse,
    KeepMaxReverse,
}

impl PfdMessage {
    pub fn text(self) -> &'static str {
        match self {
            PfdMessage::RwyTooShort => "RWY TOO SHORT",
            PfdMessage::IfWetRwyTooShort => "IF WET: RWY TOO SHORT",
            PfdMessage::MaxBraking => "MAX BRAKING",
            PfdMessage::SetMaxReverse => "SET MAX REVERSE",
            PfdMessage::KeepMaxReverse => "KEEP MAX REVERSE",
        }
    }

    pub fn level(self) -> Level {
        match self {
            PfdMessage::RwyTooShort | PfdMessage::IfWetRwyTooShort => Level::Amber,
            _ => Level::Red,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Outputs {
    pub row_operative: bool,
    pub rop_operative: bool,
    pub rwy_too_short: bool,
    pub if_wet_rwy_too_short: bool,
    pub overrun_detected: bool,
    pub max_braking_request: bool,
    pub set_max_reverse_request: bool,
    pub keep_max_reverse_request: bool,
    pub auto_max_braking: bool,
    pub btv_overridden: bool,
    pub aurals: Aurals,
    pub pfd_messages: Vec<PfdMessage>,
    pub status_word: u32,
    pub status_valid: bool,
    pub dry_stop_point_m: f64,
    pub wet_stop_point_m: f64,
    pub stop_bar_m: f64,
    pub remaining_to_end_m: f64,
}

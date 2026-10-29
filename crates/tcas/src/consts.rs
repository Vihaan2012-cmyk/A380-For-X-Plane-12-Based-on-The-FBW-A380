pub const DISPLAY_MAX: usize = 8;
pub const MEMORY_MAX: usize = 200;
pub const MIN_VS: f64 = -6000.0;
pub const MAX_VS: f64 = 6000.0;
pub const REALLY_BIG_NUMBER: f64 = 1_000_000.0;
pub const INITIAL_DELAY: f64 = 5.0;
pub const FOLLOWUP_DELAY: f64 = 2.5;
pub const INITIAL_ACCEL: f64 = 8.04;
pub const FOLLOWUP_ACCEL: f64 = 10.62;
pub const TA_EXPIRATION_DELAY: f64 = 4.0;
pub const MIN_RA_DURATION: f64 = 5.0;
pub const VOL_BOOST: f64 = 1.25;
pub const CLOSURE_RATE_THRESH: f64 = -40.0;

pub const RANGE_FORWARD_MIN: f64 = 60.0;
pub const RANGE_SIDE: f64 = 30.0;
pub const RANGE_BACK: f64 = 20.0;
pub const RANGE_ALT: f64 = 9900.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcasState {
    None = 0,
    Ta = 1,
    Ra = 2,
}

pub const MODE_STBY: u8 = 0;
pub const MODE_TA: u8 = 1;
pub const MODE_TARA: u8 = 2;

pub const XPDR_STBY: f64 = 1.0;

pub const TA: usize = 0;
pub const RA: usize = 1;

pub const INTR_TRAFFIC: u8 = 0;
pub const INTR_PROXIMITY: u8 = 1;
pub const INTR_TA: u8 = 2;
pub const INTR_RA: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaSense {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaType {
    Correct,
    Prevent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RaType2 {
    None = 0,
    Crossing = 1,
    Reversal = 2,
    Increase = 3,
    Maintain = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpDown {
    NoAdvisory = 0,
    ClimbDescend = 1,
    DontClimbDontDescend = 2,
    Greater500 = 3,
    Greater1000 = 4,
    Greater2000 = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inhibit {
    None = 0,
    AllRaAuralTa = 1,
    AllRa = 2,
    AllClimbRa = 3,
    AllDescRa = 4,
    AllIncrDescRa = 5,
}

/// THREAT[altitude select + 1]: ALL, ABOVE, BELOW (index 0 of FBW's table, THREAT, is never read).
pub const THREAT: [[f64; 2]; 3] = [[-2700.0, 2700.0], [-2700.0, 9900.0], [-9900.0, 2700.0]];

/// Altitude to sensitivity, index 3..=7 (minus 3).
pub const SENSE: [[f64; 2]; 5] =
    [[1000.0, 2350.0], [2350.0, 5000.0], [5000.0, 10000.0], [10000.0, 20000.0], [20000.0, 47000.0]];

pub const TAU: [[f64; 2]; 8] =
    [[-1.0, -1.0], [20.0, -1.0], [25.0, 15.0], [30.0, 20.0], [40.0, 25.0], [45.0, 30.0], [48.0, 35.0], [48.0, 35.0]];
pub const DMOD: [[f64; 2]; 8] =
    [[-1.0, -1.0], [0.3, -1.0], [0.33, 0.2], [0.48, 0.35], [0.75, 0.55], [1.0, 0.8], [1.3, 1.1], [1.3, 1.1]];
pub const ZTHR: [[f64; 2]; 8] = [
    [-1.0, -1.0],
    [850.0, -1.0],
    [850.0, 300.0],
    [850.0, 300.0],
    [850.0, 350.0],
    [850.0, 400.0],
    [850.0, 600.0],
    [1200.0, 700.0],
];
pub const TVTHR: [f64; 8] = [-1.0, -1.0, 15.0, 18.0, 20.0, 22.0, 25.0, 25.0];
pub const ALIM: [f64; 8] = [-1.0, -1.0, 300.0, 300.0, 350.0, 400.0, 600.0, 700.0];
pub const ACCEL: [[f64; 2]; 8] = [
    [900.0, 900.0],
    [1000.0, 1000.0],
    [1100.0, 1200.0],
    [1200.0, 1300.0],
    [1300.0, 1400.0],
    [2000.0, 2200.0],
    [2100.0, 2300.0],
    [2200.0, 2500.0],
];

#[derive(Debug, PartialEq)]
pub struct Callout {
    pub id: u8,
    pub repeat: bool,
    pub sound: &'static str,
}

pub const CALLOUT_CLIMB: Callout = Callout { id: 0, repeat: false, sound: "climb_climb" };
pub const CALLOUT_CLIMB_CROSS: Callout = Callout { id: 1, repeat: true, sound: "climb_crossing_climb" };
pub const CALLOUT_CLIMB_INCREASE: Callout = Callout { id: 2, repeat: true, sound: "increase_climb" };
pub const CALLOUT_CLIMB_NOW: Callout = Callout { id: 3, repeat: true, sound: "climb_climb_now" };
pub const CALLOUT_DESCEND: Callout = Callout { id: 5, repeat: false, sound: "descend_descend" };
pub const CALLOUT_DESCEND_CROSS: Callout = Callout { id: 6, repeat: true, sound: "descend_crossing_descend" };
pub const CALLOUT_DESCEND_INCREASE: Callout = Callout { id: 7, repeat: true, sound: "increase_descent" };
pub const CALLOUT_DESCEND_NOW: Callout = Callout { id: 8, repeat: true, sound: "descend_descend_now" };
pub const CALLOUT_MONITOR_VS: Callout = Callout { id: 9, repeat: false, sound: "monitor_vs" };
pub const CALLOUT_MAINTAIN_VS: Callout = Callout { id: 10, repeat: false, sound: "maint_vs_maint" };
pub const CALLOUT_LEVEL_OFF: Callout = Callout { id: 12, repeat: false, sound: "level_off_level_off" };

pub const SOUND_CLEAR_OF_CONFLICT: &str = "clear_of_conflict";
pub const SOUND_TRAFFIC_TRAFFIC: &str = "traffic_traffic";

pub const CALLOUT_ID_CLIMB: u8 = 0;
pub const CALLOUT_ID_CLIMB_CROSS: u8 = 1;
pub const CALLOUT_ID_CLIMB_NOW: u8 = 3;
pub const CALLOUT_ID_DESCEND: u8 = 5;
pub const CALLOUT_ID_DESCEND_NOW: u8 = 8;
pub const CALLOUT_ID_MONITOR_VS: u8 = 9;
pub const CALLOUT_ID_MAINTAIN_VS: u8 = 10;
pub const CALLOUT_ID_MAINTAIN_VS_CROSS: u8 = 11;
pub const CALLOUT_ID_LEVEL_OFF: u8 = 12;

#[derive(Debug, PartialEq)]
pub struct RaParams {
    pub callout: &'static Callout,
    pub sense: RaSense,
    pub kind: RaType,
    pub type2: RaType2,
    pub green: [f64; 2],
    pub red: [f64; 2],
    pub rate_to_maintain: f64,
    pub up: UpDown,
    pub down: UpDown,
}

const fn ra(
    callout: &'static Callout,
    sense: RaSense,
    kind: RaType,
    type2: RaType2,
    green: [f64; 2],
    red: [f64; 2],
    rate_to_maintain: f64,
    up: UpDown,
    down: UpDown,
) -> RaParams {
    RaParams { callout, sense, kind, type2, green, red, rate_to_maintain, up, down }
}

use RaSense::{Down as D, Up as U};
use RaType::{Correct as C, Prevent as P};
use RaType2 as T2;
use UpDown as UD;

pub static MONITOR_VS_CLIMB_0: RaParams =
    ra(&CALLOUT_MONITOR_VS, U, P, T2::None, [0.0, MAX_VS], [MIN_VS, 0.0], 0.0, UD::DontClimbDontDescend, UD::NoAdvisory);
pub static MONITOR_VS_CLIMB_500: RaParams =
    ra(&CALLOUT_MONITOR_VS, U, P, T2::None, [-500.0, MAX_VS], [MIN_VS, -500.0], 0.0, UD::Greater500, UD::NoAdvisory);
pub static MONITOR_VS_CLIMB_1000: RaParams =
    ra(&CALLOUT_MONITOR_VS, U, P, T2::None, [-1000.0, MAX_VS], [MIN_VS, -1000.0], 0.0, UD::Greater1000, UD::NoAdvisory);
pub static MONITOR_VS_CLIMB_2000: RaParams =
    ra(&CALLOUT_MONITOR_VS, U, P, T2::None, [-2000.0, MAX_VS], [MIN_VS, -2000.0], 0.0, UD::Greater2000, UD::NoAdvisory);
pub static MONITOR_VS_DESCEND_0: RaParams =
    ra(&CALLOUT_MONITOR_VS, D, P, T2::None, [MIN_VS, 0.0], [0.0, MAX_VS], 0.0, UD::NoAdvisory, UD::DontClimbDontDescend);
pub static MONITOR_VS_DESCEND_500: RaParams =
    ra(&CALLOUT_MONITOR_VS, D, P, T2::None, [MIN_VS, 500.0], [500.0, MAX_VS], 0.0, UD::NoAdvisory, UD::Greater500);
pub static MONITOR_VS_DESCEND_1000: RaParams =
    ra(&CALLOUT_MONITOR_VS, D, P, T2::None, [MIN_VS, 1000.0], [1000.0, MAX_VS], 0.0, UD::NoAdvisory, UD::Greater1000);
pub static MONITOR_VS_DESCEND_2000: RaParams =
    ra(&CALLOUT_MONITOR_VS, D, P, T2::None, [MIN_VS, 2000.0], [2000.0, MAX_VS], 0.0, UD::NoAdvisory, UD::Greater2000);

pub static CLIMB: RaParams =
    ra(&CALLOUT_CLIMB, U, C, T2::None, [1500.0, 2000.0], [MIN_VS, 1500.0], 1500.0, UD::ClimbDescend, UD::NoAdvisory);
pub static CLIMB_CROSS: RaParams = ra(
    &CALLOUT_CLIMB_CROSS,
    U,
    C,
    T2::Crossing,
    [1500.0, 2000.0],
    [MIN_VS, 1500.0],
    1500.0,
    UD::ClimbDescend,
    UD::NoAdvisory,
);
pub static CLIMB_INCREASE: RaParams = ra(
    &CALLOUT_CLIMB_INCREASE,
    U,
    C,
    T2::Increase,
    [2500.0, 4400.0],
    [MIN_VS, 2500.0],
    2500.0,
    UD::ClimbDescend,
    UD::NoAdvisory,
);
pub static CLIMB_NOW: RaParams = ra(
    &CALLOUT_CLIMB_NOW,
    U,
    C,
    T2::Reversal,
    [1500.0, 2000.0],
    [MIN_VS, 1500.0],
    1500.0,
    UD::ClimbDescend,
    UD::NoAdvisory,
);
pub static DESCEND: RaParams =
    ra(&CALLOUT_DESCEND, D, C, T2::None, [-2000.0, -1500.0], [-1500.0, MAX_VS], -1500.0, UD::NoAdvisory, UD::ClimbDescend);
pub static DESCEND_CROSS: RaParams = ra(
    &CALLOUT_DESCEND_CROSS,
    D,
    C,
    T2::Crossing,
    [-2000.0, -1500.0],
    [-1500.0, MAX_VS],
    -1500.0,
    UD::NoAdvisory,
    UD::ClimbDescend,
);
pub static DESCEND_INCREASE: RaParams = ra(
    &CALLOUT_DESCEND_INCREASE,
    D,
    C,
    T2::Increase,
    [-4400.0, -2500.0],
    [-2500.0, MAX_VS],
    -2500.0,
    UD::NoAdvisory,
    UD::ClimbDescend,
);
pub static DESCEND_NOW: RaParams = ra(
    &CALLOUT_DESCEND_NOW,
    D,
    C,
    T2::Reversal,
    [-2000.0, -1500.0],
    [-1500.0, MAX_VS],
    -1500.0,
    UD::NoAdvisory,
    UD::ClimbDescend,
);
pub static LEVEL_OFF_300_BELOW: RaParams =
    ra(&CALLOUT_LEVEL_OFF, D, C, T2::None, [-400.0, 0.0], [0.0, MAX_VS], 0.0, UD::NoAdvisory, UD::DontClimbDontDescend);
pub static LEVEL_OFF_300_ABOVE: RaParams =
    ra(&CALLOUT_LEVEL_OFF, U, C, T2::None, [0.0, 400.0], [MIN_VS, 0.0], 0.0, UD::DontClimbDontDescend, UD::NoAdvisory);
pub static CLIMB_MAINTAIN_VS: RaParams = ra(
    &CALLOUT_MAINTAIN_VS,
    U,
    C,
    T2::Maintain,
    [1500.0, 4400.0],
    [MIN_VS, 1500.0],
    1500.0,
    UD::ClimbDescend,
    UD::NoAdvisory,
);
pub static CLIMB_MAINTAIN_VS_CROSSING: RaParams = ra(
    &CALLOUT_MAINTAIN_VS,
    U,
    C,
    T2::Maintain,
    [1500.0, 4400.0],
    [MIN_VS, 1500.0],
    1500.0,
    UD::ClimbDescend,
    UD::NoAdvisory,
);
pub static DESCEND_MAINTAIN_VS: RaParams = ra(
    &CALLOUT_MAINTAIN_VS,
    D,
    C,
    T2::Maintain,
    [-4400.0, -1500.0],
    [-1500.0, MAX_VS],
    -1500.0,
    UD::NoAdvisory,
    UD::ClimbDescend,
);
pub static DESCEND_MAINTAIN_VS_CROSSING: RaParams = ra(
    &CALLOUT_MAINTAIN_VS,
    D,
    C,
    T2::Maintain,
    [-4400.0, -1500.0],
    [-1500.0, MAX_VS],
    -1500.0,
    UD::NoAdvisory,
    UD::ClimbDescend,
);

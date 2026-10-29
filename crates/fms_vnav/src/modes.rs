// Translated from FlyByWire's shared/src/autopilot.ts (mode numbers) and the FcuModeObserver of
// fmgc guidance/vnav/VerticalProfileManager.ts (GPL-3.0-or-later).

use crate::climb::FcuClimbHint;

pub mod lateral {
    pub const NONE: i32 = 0;
    pub const HDG: i32 = 10;
    pub const TRACK: i32 = 11;
    pub const NAV: i32 = 20;
    pub const LOC_CPT: i32 = 30;
    pub const LOC_TRACK: i32 = 31;
    pub const LAND: i32 = 32;
    pub const FLARE: i32 = 33;
    pub const ROLL_OUT: i32 = 34;
    pub const RWY: i32 = 40;
    pub const RWY_TRACK: i32 = 41;
    pub const GA_TRACK: i32 = 50;
}

pub mod armed_lateral {
    pub const NAV: i32 = 0;
    pub const LOC: i32 = 1;
}

pub mod vertical {
    pub const NONE: i32 = 0;
    pub const ALT: i32 = 10;
    pub const ALT_CPT: i32 = 11;
    pub const OP_CLB: i32 = 12;
    pub const OP_DES: i32 = 13;
    pub const VS: i32 = 14;
    pub const FPA: i32 = 15;
    pub const ALT_CST: i32 = 20;
    pub const ALT_CST_CPT: i32 = 21;
    pub const CLB: i32 = 22;
    pub const DES: i32 = 23;
    pub const FINAL: i32 = 24;
    pub const GS_CPT: i32 = 30;
    pub const GS_TRACK: i32 = 31;
    pub const LAND: i32 = 32;
    pub const FLARE: i32 = 33;
    pub const ROLL_OUT: i32 = 34;
    pub const SRS: i32 = 40;
    pub const SRS_GA: i32 = 41;
    pub const TCAS: i32 = 50;
}

pub mod armed_vertical {
    pub const ALT: i32 = 0;
    pub const ALT_CST: i32 = 1;
    pub const CLB: i32 = 2;
    pub const DES: i32 = 3;
    pub const GS: i32 = 4;
    pub const FINAL: i32 = 5;
    pub const TCAS: i32 = 6;
}

pub fn is_armed(bitmask: i32, bit: i32) -> bool {
    (bitmask >> bit) & 1 == 1
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FcuModes {
    pub lateral: i32,
    pub armed_lateral: i32,
    pub vertical: i32,
    pub armed_vertical: i32,
    pub vertical_speed: f64,
    pub flight_path_angle: f64,
    pub speed_managed: bool,
    pub expedite: bool,
}

impl FcuModes {
    pub fn is_lat_auto_control_active(&self) -> bool {
        use lateral::*;
        [NAV, LOC_CPT, LOC_TRACK, LAND, RWY, GA_TRACK].contains(&self.lateral)
    }

    /// `VnavDriver.isLatAutoControlActive`, which unlike the profile manager's list leaves out LAND and GA TRACK.
    pub fn driver_lat_auto_control_active(&self) -> bool {
        use lateral::*;
        [NAV, LOC_CPT, LOC_TRACK, RWY].contains(&self.lateral)
    }

    pub fn is_lat_auto_control_armed_with_intercept(&self) -> bool {
        use lateral::*;
        let intercept_exists = ![HDG, TRACK, GA_TRACK, RWY_TRACK].contains(&self.lateral);
        is_armed(self.armed_lateral, armed_lateral::NAV) && intercept_exists
    }

    pub fn is_speed_auto_control_active(&self) -> bool {
        self.speed_managed
    }

    pub fn is_in_takeoff_mode(&self) -> bool {
        self.vertical == vertical::SRS
    }

    pub fn is_in_climbing_mode(&self) -> bool {
        use vertical::*;
        [CLB, OP_CLB, SRS, SRS_GA].contains(&self.vertical)
            || (self.vertical == FPA && self.flight_path_angle > 0.0)
            || (self.vertical == VS && self.vertical_speed > 0.0)
    }

    pub fn is_in_descent_mode(&self) -> bool {
        use vertical::*;
        [DES, OP_DES].contains(&self.vertical)
            || (self.vertical == FPA && self.flight_path_angle < 0.0)
            || (self.vertical == VS && self.vertical_speed < 0.0)
    }

    pub fn is_in_level_flight_mode(&self) -> bool {
        use vertical::*;
        [ALT, ALT_CPT, ALT_CST, ALT_CST_CPT].contains(&self.vertical)
            || (self.vertical == FPA && self.flight_path_angle.abs() < 10.0)
            || (self.vertical == VS && self.vertical_speed.abs() < 10.0)
    }

    pub fn is_expedite_mode_active(&self) -> bool {
        self.expedite
    }

    pub fn is_in_go_around_mode(&self) -> bool {
        self.vertical == vertical::SRS_GA
    }

    pub fn is_in_selected_vertical_mode(&self) -> bool {
        use vertical::*;
        [OP_CLB, OP_DES, VS, FPA].contains(&self.vertical)
    }

    /// `VnavDriver.isSelectedVerticalModeActive` (counts expedite as selected).
    pub fn is_selected_vertical_mode_active(&self) -> bool {
        self.expedite || self.is_in_selected_vertical_mode()
    }

    /// `ClimbPathBuilder.shouldAddFcuAltAsCheckpoint`.
    pub fn climb_hint(&self) -> FcuClimbHint {
        use vertical::*;
        FcuClimbHint {
            add_fcu_altitude: is_armed(self.armed_vertical, armed_vertical::CLB) || [OP_CLB, VS, FPA, CLB, SRS, SRS_GA].contains(&self.vertical),
        }
    }
}

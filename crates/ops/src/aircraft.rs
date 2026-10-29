//! Stations, holds, tanks and doors of FlyByWire's A380X.
//!
//! Arms are feet forward of the reference datum along MSFS's longitudinal
//! axis, as FlyByWire uses them.
//! - Passenger zones: a380_systems/src/payload/mod.rs `A380_PAX` (positions,
//!   max_pax) and flight_model.cfg `station_load.0..13`.
//! - Cargo holds: payload/mod.rs `A380_CARGO`, flight_model.cfg
//!   `station_load.14..16`.
//! - Fuel tanks: a380_systems/src/fuel/mod.rs `A380_FUEL`, flight_model.cfg
//!   `Tank.1..11`.
//! - Loadsheet constants: a380_systems/src/airframe/mod.rs `LOADSHEET`.
//! - Doors: payload/mod.rs boarding agents (INTERACTIVE POINT 0, 2, 6, 8, 10).
//! - Limits: fbw-a380x/src/systems/shared/src/PerformanceConstants.ts.

use serde::{Deserialize, Serialize};

pub const OEW_KG: f64 = 300_007.12;
pub const OEW_ARM_FT: f64 = 6.47;
pub const MAC_FT: f64 = 40.35;
pub const LEMAC_FT: f64 = 21.09;
pub const PAX_KG: f64 = 84.0;
pub const FUEL_GALLONS_TO_KG: f64 = 3.039_075_693_483_925;
pub const KG_PER_LB: f64 = 0.453_592_37;

pub const MAX_ZFW_KG: f64 = 373_000.0;
pub const MAX_TOW_KG: f64 = 510_000.0;
pub const MAX_GW_KG: f64 = 512_000.0;
pub const MAX_LW_KG: f64 = 395_000.0;
pub const MIN_ZFW_CG: f64 = 23.0;
pub const MAX_ZFW_CG: f64 = 48.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Deck {
    Main,
    Upper,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaxZone {
    pub id: &'static str,
    pub label: &'static str,
    pub deck: Deck,
    pub seats: u32,
    pub arm_ft: f64,
}

const fn zone(id: &'static str, label: &'static str, deck: Deck, seats: u32, arm_ft: f64) -> PaxZone {
    PaxZone { id, label, deck, seats, arm_ft }
}

pub const PAX_ZONES: [PaxZone; 14] = [
    zone("PAX_MAIN_FWD_A", "MAIN FWD A", Deck::Main, 28, 75.7),
    zone("PAX_MAIN_FWD_B", "MAIN FWD B", Deck::Main, 28, 75.7),
    zone("PAX_MAIN_MID_1A", "MAIN MID 1A", Deck::Main, 39, 30.6),
    zone("PAX_MAIN_MID_1B", "MAIN MID 1B", Deck::Main, 50, 30.6),
    zone("PAX_MAIN_MID_1C", "MAIN MID 1C", Deck::Main, 43, 30.6),
    zone("PAX_MAIN_MID_2A", "MAIN MID 2A", Deck::Main, 48, -11.1),
    zone("PAX_MAIN_MID_2B", "MAIN MID 2B", Deck::Main, 40, -11.1),
    zone("PAX_MAIN_MID_2C", "MAIN MID 2C", Deck::Main, 36, -11.1),
    zone("PAX_MAIN_AFT_A", "MAIN AFT A", Deck::Main, 42, -46.9),
    zone("PAX_MAIN_AFT_B", "MAIN AFT B", Deck::Main, 40, -46.9),
    zone("PAX_UPPER_FWD", "UPPER FWD", Deck::Upper, 14, 60.8),
    zone("PAX_UPPER_MID_A", "UPPER MID A", Deck::Upper, 30, 11.7),
    zone("PAX_UPPER_MID_B", "UPPER MID B", Deck::Upper, 28, 11.7),
    zone("PAX_UPPER_AFT", "UPPER AFT", Deck::Upper, 18, -29.3),
];

pub fn total_seats() -> u32 {
    PAX_ZONES.iter().map(|z| z.seats).sum()
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hold {
    pub id: &'static str,
    pub label: &'static str,
    pub max_kg: f64,
    pub arm_ft: f64,
}

pub const HOLDS: [Hold; 3] = [
    Hold { id: "CARGO_FWD", label: "FWD", max_kg: 28_577.0, arm_ft: 67.4 },
    Hold { id: "CARGO_AFT", label: "AFT", max_kg: 20_310.0, arm_ft: -18.5 },
    Hold { id: "CARGO_BULK", label: "BULK", max_kg: 2_513.0, arm_ft: -52.9 },
];

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tank {
    pub id: &'static str,
    pub label: &'static str,
    pub capacity_gal: f64,
    pub arm_ft: f64,
}

impl Tank {
    pub fn capacity_kg(&self) -> f64 {
        self.capacity_gal * FUEL_GALLONS_TO_KG
    }
}

const fn tank(id: &'static str, label: &'static str, capacity_gal: f64, arm_ft: f64) -> Tank {
    Tank { id, label, capacity_gal, arm_ft }
}

pub const LEFT_OUTER: usize = 0;
pub const FEED_1: usize = 1;
pub const LEFT_MID: usize = 2;
pub const LEFT_INNER: usize = 3;
pub const FEED_2: usize = 4;
pub const FEED_3: usize = 5;
pub const RIGHT_INNER: usize = 6;
pub const RIGHT_MID: usize = 7;
pub const FEED_4: usize = 8;
pub const RIGHT_OUTER: usize = 9;
pub const TRIM: usize = 10;
pub const TANK_COUNT: usize = 11;

pub const TANKS: [Tank; TANK_COUNT] = [
    tank("LEFT_OUTER", "LEFT OUTER", 2731.5, -25.0),
    tank("FEED_ONE", "FEED 1", 7299.6, -7.45),
    tank("LEFT_MID", "LEFT MID", 9632.0, 7.1),
    tank("LEFT_INNER", "LEFT INNER", 12189.4, 16.5),
    tank("FEED_TWO", "FEED 2", 7753.2, 27.3),
    tank("FEED_THREE", "FEED 3", 7753.2, 27.3),
    tank("RIGHT_INNER", "RIGHT INNER", 12189.4, 16.5),
    tank("RIGHT_MID", "RIGHT MID", 9632.0, 7.1),
    tank("FEED_FOUR", "FEED 4", 7299.6, -7.45),
    tank("RIGHT_OUTER", "RIGHT OUTER", 2731.5, -25.0),
    tank("TRIM", "TRIM", 6260.3, -87.14),
];

pub const FEED_TANKS: [usize; 4] = [FEED_1, FEED_2, FEED_3, FEED_4];

pub fn fuel_capacity_kg() -> f64 {
    TANKS.iter().map(Tank::capacity_kg).sum()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Door {
    M1L,
    M2L,
    M4L,
    M5L,
    U1L,
}

impl Door {
    pub const ALL: [Door; 5] = [Door::M1L, Door::M2L, Door::M4L, Door::M5L, Door::U1L];

    pub fn interactive_point(self) -> u32 {
        match self {
            Door::M1L => 0,
            Door::M2L => 2,
            Door::M4L => 6,
            Door::M5L => 8,
            Door::U1L => 10,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Door::M1L => "M1L",
            Door::M2L => "M2L",
            Door::M4L => "M4L",
            Door::M5L => "M5L",
            Door::U1L => "U1L",
        }
    }
}

pub fn percent_mac(arm_ft: f64) -> f64 {
    -100.0 * (arm_ft - LEMAC_FT) / MAC_FT
}

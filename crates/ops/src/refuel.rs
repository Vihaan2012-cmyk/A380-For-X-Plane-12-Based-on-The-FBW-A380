//! Refuel: FlyByWire's tank distribution and timed refuel
//! (a380_systems/src/fuel/cpiom_f/mod.rs `calculate_auto_refuel`,
//! `RefuelDriver::execute_timed_refuel`, `resolve_delta_in_channel`, and the
//! stage/limit values in cpiom_f/wing_tank_values.toml). The trim tank share
//! is FlyByWire's trim_tank_targets.toml lookup, passed in by the caller.

use serde::{Deserialize, Serialize};

use crate::aircraft::*;
use crate::settings::{FuelSource, Speed};

pub const HYDRANT_GAL_PER_S: f64 = 16.0;
pub const FAST_FACTOR: f64 = 5.0;
/// GENERIC: a single bowser feeds about half what a hydrant dispenser does.
pub const TRUCK_GAL_PER_S: f64 = 8.0;

const STAGE: [f64; 8] = [18_000.0, 26_000.0, 36_000.0, 47_000.0, 103_788.0, 158_042.0, 215_702.0, 223_088.0];
const FEED_A: f64 = 4_500.0;
const FEED_C: f64 = 7_000.0;
const OUTER_FEED_E: f64 = 20_558.0;
const INNER_FEED_E: f64 = 21_836.0;
const OUTER_TANK_B: f64 = 4_000.0;
const OUTER_TANK_H: f64 = 7_693.0;
const INNER_TANK_D: f64 = 5_500.0;
const INNER_TANK_G: f64 = 34_330.0;
const MID_TANK_F: f64 = 27_127.0;

pub fn rate_kg_per_s(speed: Speed, source: FuelSource) -> Option<f64> {
    let gal = match source {
        FuelSource::Hydrant => HYDRANT_GAL_PER_S,
        FuelSource::Truck => TRUCK_GAL_PER_S,
    };
    match speed {
        Speed::Realistic => Some(gal * FUEL_GALLONS_TO_KG),
        Speed::Fast => Some(gal * FAST_FACTOR * FUEL_GALLONS_TO_KG),
        Speed::Instant => None,
    }
}

pub fn distribute(total_kg: f64, trim_kg: f64) -> [f64; TANK_COUNT] {
    let [a, b, c, d, e, f, g, h] = STAGE;
    let trim = trim_kg.clamp(0.0, total_kg.max(0.0));
    let wing = total_kg.max(0.0) - trim;
    let total_feed_e = 2.0 * OUTER_FEED_E + 2.0 * INNER_FEED_E;
    let wing_size = 2.0 * (TANKS[FEED_1].capacity_gal
        + TANKS[FEED_2].capacity_gal
        + TANKS[LEFT_OUTER].capacity_gal
        + TANKS[LEFT_MID].capacity_gal
        + TANKS[LEFT_INNER].capacity_gal);
    let over = |size_gal: f64| (wing - h) * size_gal / wing_size;
    let feed = |feed_e: f64, size: f64| match wing {
        x if x <= a => wing / 4.0,
        x if x <= b => FEED_A,
        x if x <= c => FEED_A + (wing - b) / 4.0,
        x if x <= d => FEED_C,
        x if x <= e => FEED_C + (wing - d) * (feed_e / total_feed_e),
        x if x <= h => feed_e,
        _ => feed_e + over(size),
    };
    let outer_feed = feed(OUTER_FEED_E, TANKS[FEED_1].capacity_gal);
    let inner_feed = feed(INNER_FEED_E, TANKS[FEED_2].capacity_gal);
    let outer = match wing {
        x if x <= a => 0.0,
        x if x <= b => (wing - a) / 2.0,
        x if x <= g => OUTER_TANK_B,
        x if x <= h => OUTER_TANK_B + (wing - g) / 2.0,
        _ => OUTER_TANK_H + over(TANKS[LEFT_OUTER].capacity_gal),
    };
    let inner = match wing {
        x if x <= c => 0.0,
        x if x <= d => (wing - c) / 2.0,
        x if x <= f => INNER_TANK_D,
        x if x <= g => INNER_TANK_D + (wing - f) / 2.0,
        x if x <= h => INNER_TANK_G,
        _ => INNER_TANK_G + over(TANKS[LEFT_INNER].capacity_gal),
    };
    let mid = match wing {
        x if x <= e => 0.0,
        x if x <= f => (wing - e) / 2.0,
        x if x <= h => MID_TANK_F,
        _ => MID_TANK_F + over(TANKS[LEFT_MID].capacity_gal),
    };
    let mut out = [0.0; TANK_COUNT];
    out[LEFT_OUTER] = outer;
    out[RIGHT_OUTER] = outer;
    out[LEFT_MID] = mid;
    out[RIGHT_MID] = mid;
    out[LEFT_INNER] = inner;
    out[RIGHT_INNER] = inner;
    out[FEED_1] = outer_feed;
    out[FEED_4] = outer_feed;
    out[FEED_2] = inner_feed;
    out[FEED_3] = inner_feed;
    out[TRIM] = trim;
    for (q, t) in out.iter_mut().zip(TANKS.iter()) {
        *q = q.clamp(0.0, t.capacity_kg());
    }
    out
}

const LEFT_CHANNEL: [usize; TANK_COUNT] =
    [FEED_1, FEED_2, LEFT_OUTER, LEFT_MID, LEFT_INNER, TRIM, FEED_4, FEED_3, RIGHT_OUTER, RIGHT_MID, RIGHT_INNER];
const RIGHT_CHANNEL: [usize; TANK_COUNT] =
    [FEED_4, FEED_3, RIGHT_OUTER, RIGHT_MID, RIGHT_INNER, TRIM, FEED_1, FEED_2, LEFT_OUTER, LEFT_MID, LEFT_INNER];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Refuel {
    pub tanks_kg: [f64; TANK_COUNT],
    pub target_kg: [f64; TANK_COUNT],
    pub speed: Speed,
    pub source: FuelSource,
    pub elapsed_s: f64,
}

impl Refuel {
    pub fn new(tanks_kg: [f64; TANK_COUNT], target_kg: [f64; TANK_COUNT], speed: Speed, source: FuelSource) -> Self {
        Self { tanks_kg, target_kg, speed, source, elapsed_s: 0.0 }
    }

    pub fn done(&self) -> bool {
        self.tanks_kg.iter().zip(self.target_kg.iter()).all(|(a, b)| (a - b).abs() < 1e-6)
    }

    pub fn total_kg(&self) -> f64 {
        self.tanks_kg.iter().sum()
    }

    fn resolve(&mut self, mut remaining: f64, channel: &[usize; TANK_COUNT]) {
        for &i in channel {
            if remaining <= 0.0 {
                break;
            }
            let diff = self.target_kg[i] - self.tanks_kg[i];
            let moved = diff.abs().min(remaining);
            self.tanks_kg[i] += moved * diff.signum();
            remaining -= moved;
        }
    }

    pub fn step(&mut self, dt: f64) -> bool {
        if self.done() {
            return true;
        }
        let Some(rate) = rate_kg_per_s(self.speed, self.source) else {
            self.tanks_kg = self.target_kg;
            return true;
        };
        self.elapsed_s += dt;
        let half = rate * dt / 2.0;
        self.resolve(half, &LEFT_CHANNEL);
        self.resolve(half, &RIGHT_CHANNEL);
        self.done()
    }

    pub fn estimate_seconds(&self) -> f64 {
        match rate_kg_per_s(self.speed, self.source) {
            None => 0.0,
            Some(rate) => {
                let moved: f64 = self.tanks_kg.iter().zip(self.target_kg.iter()).map(|(a, b)| (a - b).abs()).sum();
                moved / rate
            }
        }
    }
}

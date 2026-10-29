//! Boarding and deboarding on FlyByWire's model (fbw-common
//! systems/src/payload/mod.rs `PayloadManager::update`,
//! `PassengerDeck::update_one_tick`, `Cargo::move_one_cargo`; A380 rates and
//! agents in a380_systems/src/payload/mod.rs): every tick each door in use
//! moves one passenger, taking the first zone in that door's order that is
//! short of (or over) its target; with no door in use one passenger moves
//! per tick. Each hold moves up to 60 kg per tick. A tick is 5 s realistic,
//! 0.5 s fast; instant moves everything at once.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::aircraft::{Door, HOLDS};
use crate::settings::Speed;

pub const REAL_TICK_S: f64 = 5.0;
pub const FAST_TICK_S: f64 = 0.5;
pub const CARGO_KG_PER_TICK: f64 = 60.0;

const DEFAULT_ORDER: [usize; 14] = [10, 13, 12, 11, 1, 0, 9, 8, 7, 6, 5, 4, 3, 2];

pub fn door_order(door: Door) -> [usize; 14] {
    match door {
        Door::M1L => [10, 13, 12, 11, 1, 0, 9, 8, 7, 6, 5, 4, 3, 2],
        Door::M2L => [1, 0, 9, 8, 7, 6, 5, 2, 3, 4, 10, 11, 12, 13],
        Door::M4L | Door::M5L => [13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
        Door::U1L => [10, 13, 12, 11, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
    }
}

pub fn tick_seconds(speed: Speed) -> f64 {
    match speed {
        Speed::Realistic => REAL_TICK_S,
        Speed::Fast => FAST_TICK_S,
        Speed::Instant => 0.0,
    }
}

/// Doors passengers use: every door with a jet bridge on it plus any door
/// opened with stairs, each once.
pub fn doors_in_use(jet_bridges: &[Door], open_with_stairs: &[Door]) -> Vec<Door> {
    jet_bridges.iter().chain(open_with_stairs).copied().collect::<BTreeSet<_>>().into_iter().collect()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Boarding {
    pub pax: [u32; 14],
    pub pax_target: [u32; 14],
    pub cargo_kg: [f64; 3],
    pub cargo_target_kg: [f64; 3],
    pub doors: Vec<Door>,
    pub speed: Speed,
    timer_s: f64,
    pub elapsed_s: f64,
}

impl Boarding {
    pub fn new(pax: [u32; 14], cargo_kg: [f64; 3], doors: Vec<Door>, speed: Speed) -> Self {
        Self { pax, pax_target: pax, cargo_kg, cargo_target_kg: cargo_kg, doors, speed, timer_s: 0.0, elapsed_s: 0.0 }
    }

    pub fn set_target(&mut self, pax: [u32; 14], cargo_kg: [f64; 3]) {
        self.pax_target = pax;
        for (t, (c, h)) in self.cargo_target_kg.iter_mut().zip(cargo_kg.iter().zip(HOLDS.iter())) {
            *t = c.clamp(0.0, h.max_kg);
        }
    }

    pub fn done(&self) -> bool {
        self.pax == self.pax_target && self.cargo_kg.iter().zip(self.cargo_target_kg.iter()).all(|(a, b)| (a - b).abs() < 1e-6)
    }

    fn move_one(&mut self, order: &[usize; 14]) {
        if let Some(&i) = order.iter().find(|&&i| self.pax[i] != self.pax_target[i]) {
            if self.pax[i] < self.pax_target[i] {
                self.pax[i] += 1;
            } else {
                self.pax[i] -= 1;
            }
        }
    }

    fn tick(&mut self) {
        if self.doors.is_empty() {
            self.move_one(&DEFAULT_ORDER);
        } else {
            for door in self.doors.clone() {
                self.move_one(&door_order(door));
            }
        }
        for (c, t) in self.cargo_kg.iter_mut().zip(self.cargo_target_kg.iter()) {
            let step = (t - *c).abs().min(CARGO_KG_PER_TICK);
            *c += step * (t - *c).signum();
        }
    }

    /// Advances by `dt` seconds; true once everything matches the target.
    pub fn step(&mut self, dt: f64) -> bool {
        if self.done() {
            return true;
        }
        let tick = tick_seconds(self.speed);
        if tick <= 0.0 {
            self.pax = self.pax_target;
            self.cargo_kg = self.cargo_target_kg;
            return true;
        }
        self.elapsed_s += dt;
        self.timer_s += dt;
        while self.timer_s >= tick && !self.done() {
            self.timer_s -= tick;
            self.tick();
        }
        self.done()
    }

    pub fn estimate_seconds(&self) -> f64 {
        let mut sim = self.clone();
        let tick = tick_seconds(self.speed);
        if tick <= 0.0 {
            return 0.0;
        }
        let mut ticks = 0u64;
        while !sim.done() && ticks < 1_000_000 {
            sim.tick();
            ticks += 1;
        }
        ticks as f64 * tick
    }
}

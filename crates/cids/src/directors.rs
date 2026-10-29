use crate::types::{CabinIn, Power};

const UP_S: f64 = 8.;

pub struct Directors {
    pub healthy: [bool; 3],
    pub active: Option<usize>,
    up_s: f64,
}

impl Directors {
    pub fn new() -> Self {
        Self { healthy: [false; 3], active: None, up_s: 0. }
    }

    pub fn step(&mut self, power: &Power, cabin: &CabinIn, dt: f64) {
        for i in 0..3 {
            self.healthy[i] = power.cids_bus[i] && !cabin.director_fault[i];
        }
        match self.active {
            Some(a) if self.healthy[a] => {}
            _ => self.active = self.healthy.iter().position(|h| *h),
        }
        if self.active.is_some() {
            self.up_s += dt;
        } else {
            self.up_s = 0.;
        }
    }

    pub fn operating(&self) -> bool {
        self.active.is_some() && self.up_s >= UP_S
    }

    pub fn healthy_count(&self) -> usize {
        self.healthy.iter().filter(|h| **h).count()
    }

    pub fn degraded(&self) -> bool {
        self.healthy_count() < 3
    }
}

impl Default for Directors {
    fn default() -> Self {
        Self::new()
    }
}

//! Zone-by-zone weight and balance, the way FlyByWire's A380 airframe
//! computes it (a380_systems/src/airframe/mod.rs `update`): operating empty
//! weight at its arm, plus passengers, cargo and fuel at theirs, each CG
//! turned into % MAC with `convert_cg`. Take-off figures take the taxi fuel
//! out of the feed tanks (FlyByWire leaves taxi fuel as a TODO and uses the
//! gross weight).

use serde::{Deserialize, Serialize};

use crate::aircraft::{
    percent_mac, FEED_TANKS, HOLDS, MAX_LW_KG, MAX_TOW_KG, MAX_ZFW_CG, MAX_ZFW_KG, MIN_ZFW_CG, OEW_ARM_FT, OEW_KG,
    PAX_KG, PAX_ZONES, TANKS, TANK_COUNT,
};
use crate::trim::ths_for_cg;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Moment {
    pub kg: f64,
    pub kg_ft: f64,
}

impl Moment {
    pub fn of(kg: f64, arm_ft: f64) -> Self {
        Self { kg, kg_ft: kg * arm_ft }
    }

    pub fn add(self, other: Moment) -> Self {
        Self { kg: self.kg + other.kg, kg_ft: self.kg_ft + other.kg_ft }
    }

    pub fn arm_ft(&self) -> f64 {
        if self.kg.abs() < 1e-9 {
            0.0
        } else {
            self.kg_ft / self.kg
        }
    }

    pub fn percent_mac(&self) -> f64 {
        percent_mac(self.arm_ft())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Load {
    pub pax: [u32; 14],
    pub cargo_kg: [f64; 3],
    pub fuel_kg: [f64; TANK_COUNT],
    pub pax_kg: f64,
    pub taxi_fuel_kg: f64,
    pub trip_fuel_kg: f64,
}

impl Default for Load {
    fn default() -> Self {
        Self { pax: [0; 14], cargo_kg: [0.0; 3], fuel_kg: [0.0; TANK_COUNT], pax_kg: PAX_KG, taxi_fuel_kg: 0.0, trip_fuel_kg: 0.0 }
    }
}

impl Load {
    pub fn full_cabin() -> [u32; 14] {
        let mut pax = [0; 14];
        for (p, z) in pax.iter_mut().zip(PAX_ZONES.iter()) {
            *p = z.seats;
        }
        pax
    }

    pub fn clamped(mut self) -> Self {
        for (p, z) in self.pax.iter_mut().zip(PAX_ZONES.iter()) {
            *p = (*p).min(z.seats);
        }
        for (c, h) in self.cargo_kg.iter_mut().zip(HOLDS.iter()) {
            *c = c.clamp(0.0, h.max_kg);
        }
        for (f, t) in self.fuel_kg.iter_mut().zip(TANKS.iter()) {
            *f = f.clamp(0.0, t.capacity_kg());
        }
        self
    }

    pub fn total_pax(&self) -> u32 {
        self.pax.iter().sum()
    }

    pub fn total_cargo_kg(&self) -> f64 {
        self.cargo_kg.iter().sum()
    }

    pub fn total_fuel_kg(&self) -> f64 {
        self.fuel_kg.iter().sum()
    }

    pub fn pax_moment(&self) -> Moment {
        self.pax
            .iter()
            .zip(PAX_ZONES.iter())
            .fold(Moment::default(), |m, (n, z)| m.add(Moment::of(*n as f64 * self.pax_kg, z.arm_ft)))
    }

    pub fn cargo_moment(&self) -> Moment {
        self.cargo_kg.iter().zip(HOLDS.iter()).fold(Moment::default(), |m, (kg, h)| m.add(Moment::of(*kg, h.arm_ft)))
    }

    pub fn fuel_moment(fuel_kg: &[f64; TANK_COUNT]) -> Moment {
        fuel_kg.iter().zip(TANKS.iter()).fold(Moment::default(), |m, (kg, t)| m.add(Moment::of(*kg, t.arm_ft)))
    }

    pub fn after_taxi(&self) -> [f64; TANK_COUNT] {
        let mut fuel = self.fuel_kg;
        let feed: f64 = FEED_TANKS.iter().map(|&i| fuel[i]).sum();
        if feed > 0.0 {
            let burn = self.taxi_fuel_kg.min(feed);
            for &i in &FEED_TANKS {
                fuel[i] -= burn * fuel[i] / feed;
            }
        }
        fuel
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Loadsheet {
    pub pax: u32,
    pub pax_main: u32,
    pub pax_upper: u32,
    pub cargo_kg: f64,
    pub fuel_kg: f64,
    pub zfw_kg: f64,
    pub zfw_cg: f64,
    pub gw_kg: f64,
    pub gw_cg: f64,
    pub tow_kg: f64,
    pub tow_cg: f64,
    pub lw_kg: f64,
    pub ths_deg: f64,
    pub over_mzfw: bool,
    pub over_mtow: bool,
    pub over_mlw: bool,
    pub zfw_cg_out_of_range: bool,
}

impl Loadsheet {
    pub fn compute(load: &Load) -> Self {
        let oew = Moment::of(OEW_KG, OEW_ARM_FT);
        let zfw = oew.add(load.pax_moment()).add(load.cargo_moment());
        let gw = zfw.add(Load::fuel_moment(&load.fuel_kg));
        let tow = zfw.add(Load::fuel_moment(&load.after_taxi()));
        let tow_cg = tow.percent_mac();
        let lw_kg = tow.kg - load.trip_fuel_kg.min(tow.kg - zfw.kg);
        let zfw_cg = zfw.percent_mac();
        let upper: u32 = load
            .pax
            .iter()
            .zip(PAX_ZONES.iter())
            .filter(|(_, z)| z.deck == crate::aircraft::Deck::Upper)
            .map(|(n, _)| *n)
            .sum();
        Self {
            pax: load.total_pax(),
            pax_main: load.total_pax() - upper,
            pax_upper: upper,
            cargo_kg: load.total_cargo_kg(),
            fuel_kg: load.total_fuel_kg(),
            zfw_kg: zfw.kg,
            zfw_cg,
            gw_kg: gw.kg,
            gw_cg: gw.percent_mac(),
            tow_kg: tow.kg,
            tow_cg,
            lw_kg,
            ths_deg: ths_for_cg(tow_cg),
            over_mzfw: zfw.kg > MAX_ZFW_KG,
            over_mtow: tow.kg > MAX_TOW_KG,
            over_mlw: lw_kg > MAX_LW_KG,
            zfw_cg_out_of_range: !(MIN_ZFW_CG..=MAX_ZFW_CG).contains(&zfw_cg),
        }
    }

    pub fn within_limits(&self) -> bool {
        !(self.over_mzfw || self.over_mtow || self.over_mlw || self.zfw_cg_out_of_range)
    }
}

//! The flyer's ops choices, as the EFB stores them.

use serde::{Deserialize, Serialize};

use crate::aircraft::KG_PER_LB;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Units {
    #[default]
    Kg,
    Lb,
}

impl Units {
    pub fn from_kg(self, kg: f64) -> f64 {
        match self {
            Units::Kg => kg,
            Units::Lb => kg / KG_PER_LB,
        }
    }

    pub fn to_kg(self, v: f64) -> f64 {
        match self {
            Units::Kg => v,
            Units::Lb => v * KG_PER_LB,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Units::Kg => "KG",
            Units::Lb => "LB",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnitsChoice {
    #[default]
    Airline,
    Kg,
    Lb,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Speed {
    #[default]
    Realistic,
    Fast,
    Instant,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FuelSource {
    #[default]
    Hydrant,
    Truck,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OpsSettings {
    pub units: UnitsChoice,
    pub boarding: Speed,
    pub refuel: Speed,
    pub fuel_source: FuelSource,
    pub wear_failures: bool,
    pub simulated_aoc: bool,
    pub pax_kg: f64,
}

impl Default for OpsSettings {
    fn default() -> Self {
        Self {
            units: UnitsChoice::Airline,
            boarding: Speed::Realistic,
            refuel: Speed::Realistic,
            fuel_source: FuelSource::Hydrant,
            wear_failures: true,
            simulated_aoc: true,
            pax_kg: crate::aircraft::PAX_KG,
        }
    }
}

impl OpsSettings {
    pub fn resolved_units(&self, airline_icao: Option<&str>) -> Units {
        match self.units {
            UnitsChoice::Kg => Units::Kg,
            UnitsChoice::Lb => Units::Lb,
            UnitsChoice::Airline => airline_icao.map(airline_units).unwrap_or_default(),
        }
    }

    pub fn aoc_active(&self, on_network: bool) -> bool {
        self.simulated_aoc && !on_network
    }
}

/// GENERIC: operators whose load control works in pounds. Every current A380
/// operator works in kilograms, so only US carriers (whose fictional A380
/// liveries exist) are listed.
const LB_AIRLINES: &[&str] = &["AAL", "UAL", "DAL", "SWA", "ASA", "JBU", "FDX", "UPS", "ATN", "GTI"];

pub fn airline_units(icao: &str) -> Units {
    let code = icao.trim().to_ascii_uppercase();
    if LB_AIRLINES.contains(&code.as_str()) {
        Units::Lb
    } else {
        Units::Kg
    }
}

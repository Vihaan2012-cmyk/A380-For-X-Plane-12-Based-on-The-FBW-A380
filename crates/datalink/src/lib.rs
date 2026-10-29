pub mod acars;
pub mod aoc;
pub mod atc;
pub mod atis_gen;
pub mod ats623;
pub mod catalogue;
pub mod convert;
pub mod elements;
pub mod fans;
pub mod host_bridge;
pub mod inputs;
pub mod input_checks;
pub mod interpretation;
pub mod message;
pub mod mfd_atccom;
pub mod monitor;
pub mod nxapi;
pub mod online;
pub mod router;
pub mod sayintentions;
pub mod status;
pub mod system;
pub mod tables;
pub mod validation;
pub mod vhf;
pub mod weather;
pub mod weather_sources;
pub mod winds;
pub mod xp_weather;

pub mod topics {
    pub use crate::tables::*;

    pub fn all() -> Vec<&'static str> {
        let mut names: Vec<&'static str> = Vec::new();
        for (_, group) in ALL_GROUPS {
            for name in group.iter() {
                if !names.contains(name) {
                    names.push(name);
                }
            }
        }
        names
    }
}

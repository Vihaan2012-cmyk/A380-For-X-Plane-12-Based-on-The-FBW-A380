// Translated from FlyByWire's fmgc guidance/vnav/CostIndex.ts and the A380X MFD FMC/fmgc.ts managed speed getters (GPL-3.0-or-later).

use crate::atmosphere as atm;
use crate::config::{AircraftConfig, FlapConf};
use crate::engine as eng;
use crate::flight_model as fm;

pub fn managed_climb_speed(cost_index: Option<f64>) -> f64 {
    match cost_index {
        Some(ci) => {
            let d = (ci / 999.0).powi(2);
            290.0 * (1.0 - d) + 330.0 * d
        }
        None => 250.0,
    }
}

pub fn managed_climb_mach() -> f64 {
    0.84
}

pub fn managed_cruise_speed() -> f64 {
    310.0
}

pub fn managed_cruise_mach() -> f64 {
    0.85
}

pub fn managed_descent_speed(pilot_speed: Option<f64>, cost_index: Option<f64>) -> f64 {
    if let Some(s) = pilot_speed {
        return s;
    }
    match cost_index {
        Some(ci) => {
            let d = ci / 999.0;
            (288.0 * (1.0 - d) + 300.0 * d).round()
        }
        None => 300.0,
    }
}

pub fn managed_descent_mach(pilot_mach: Option<f64>) -> f64 {
    pilot_mach.unwrap_or(0.84)
}

pub fn calculate_specific_range(config: &AircraftConfig, mach: f64, altitude: f64, weight: f64, isa_dev: f64) -> f64 {
    let theta = atm::theta(altitude, isa_dev, false);
    let theta2 = atm::theta2(theta, mach);
    let delta = atm::delta(altitude, false);
    let delta2 = atm::delta2(delta, mach);

    let vt = atm::mach_to_tas(mach, theta);
    let thrust = fm::drag(&config.flight, weight, mach, delta, false, false, FlapConf::Clean);

    let corrected_thrust = thrust / delta2 / 2.0;
    let corrected_n1 = eng::reverse_table_interpolation(&config.engine.table1506, mach, corrected_thrust / config.engine.max_thrust);
    let corrected_fuel_flow = eng::corrected_fuel_flow(&config.engine, corrected_n1, mach, altitude);
    let fuel_flow = eng::uncorrected_fuel_flow(corrected_fuel_flow, delta2, theta2);
    vt / fuel_flow
}

pub fn initial_mach_estimate(config: &AircraftConfig, weight: f64, delta: f64) -> f64 {
    1.565 * (weight / (1481.0 * config.flight.wing_area * delta)).sqrt()
}

pub fn naive_find_mmrc(config: &AircraftConfig, altitude: f64, weight: f64, isa_dev: f64) -> f64 {
    let delta = atm::delta(altitude, false);
    let m1 = initial_mach_estimate(config, weight, delta).min(0.78);
    let m_round = ((m1 + f64::EPSILON) * 100.0).round() / 100.0;

    let lower = m_round - 0.1;
    let upper = (m_round + 0.1).min(0.79);
    let mut results = Vec::new();
    let mut i = lower;
    while i < upper {
        results.push(calculate_specific_range(config, i, altitude, weight, isa_dev));
        i += 0.01;
    }
    let mut index_of_max = 0;
    for (k, x) in results.iter().enumerate() {
        if *x > results[index_of_max] {
            index_of_max = k;
        }
    }
    lower + index_of_max as f64 * 0.01
}

pub fn cost_index_to_mach(config: &AircraftConfig, ci: f64, flight_level: f64, weight: f64, isa_dev: f64, headwind: f64) -> f64 {
    let mmrc = naive_find_mmrc(config, flight_level * 100.0, weight, isa_dev) + headwind / 10000.0;
    -1.0 * (0.8 - mmrc) * (-0.05 * ci).exp() + 0.8
}

pub fn cost_index_to_climb_cas(ci: f64, flight_level: f64, weight: f64) -> f64 {
    let tons = atm::pounds_to_metric_tons(weight).clamp(50.0, 77.0);
    let airspeed = 240.0 + 0.1 * flight_level + ci + 0.5 * (tons - 50.0);
    airspeed.clamp(270.0, 340.0)
}

pub fn cost_index_to_descent_cas(ci: f64, flight_level: f64, weight: f64) -> f64 {
    let tons = atm::pounds_to_metric_tons(weight).clamp(50.0, 77.0);
    let airspeed = 205.0 + 0.13 * flight_level + 1.5 * ci - 0.05 * (tons - 50.0);
    airspeed.clamp(270.0, 340.0)
}

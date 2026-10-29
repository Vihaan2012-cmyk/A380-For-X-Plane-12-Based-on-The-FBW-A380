use crate::constants::*;
use crate::speeds::{js_round, FlightPhase};

pub fn zero_fuel_weight_tonnes(entered_zfw_tonnes: Option<f64>) -> f64 {
    entered_zfw_tonnes.unwrap_or(MIN_GW / 1000.0)
}

pub fn fuel_on_board_tonnes(
    an_engine_on: bool,
    fqms_fob_kg: Option<f64>,
    entered_block_fuel_tonnes: Option<f64>,
) -> Option<f64> {
    if an_engine_on {
        fqms_fob_kg.map(|kg| kg / 1000.0)
    } else {
        entered_block_fuel_tonnes
    }
}

pub fn gross_weight_tonnes(
    fqms_gross_weight_kg: Option<f64>,
    entered_zfw_tonnes: Option<f64>,
    fob_tonnes: Option<f64>,
) -> Option<f64> {
    if let Some(gw) = fqms_gross_weight_kg {
        return Some(gw / 1000.0);
    }
    match (entered_zfw_tonnes, fob_tonnes) {
        (Some(zfw), Some(fob)) => Some(zfw + fob),
        _ => None,
    }
}

pub fn gross_weight_kg(
    fqms_gross_weight_kg: Option<f64>,
    entered_zfw_tonnes: Option<f64>,
    fob_tonnes: Option<f64>,
) -> Option<f64> {
    match gross_weight_tonnes(fqms_gross_weight_kg, entered_zfw_tonnes, fob_tonnes) {
        Some(gw) if gw != 0.0 => Some(gw * 1000.0),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LandingWeightInputs {
    pub takeoff_weight_kg: Option<f64>,
    pub gross_weight_kg: Option<f64>,
    pub trip_fuel_kg: Option<f64>,
    pub engines_were_started: bool,
    pub phase: FlightPhase,
    pub taxi_fuel_tonnes: Option<f64>,
}

pub fn landing_weight_kg(i: &LandingWeightInputs) -> Option<f64> {
    let trip = i.trip_fuel_kg.filter(|t| *t != 0.0);
    let tow = i.takeoff_weight_kg.filter(|t| *t != 0.0);
    let gw = i.gross_weight_kg.filter(|t| *t != 0.0);
    if !i.engines_were_started {
        return match (tow, trip) {
            (Some(tow), Some(tf)) => Some(tow - tf),
            _ => None,
        };
    }
    if let (Some(gw), Some(tf)) = (gw, trip) {
        if i.phase >= FlightPhase::Takeoff {
            return Some(gw - tf);
        }
    }
    if let Some(gw) = gw {
        return Some(gw - i.trip_fuel_kg.unwrap_or(0.0) - i.taxi_fuel_tonnes.unwrap_or(0.0) * 1000.0);
    }
    None
}

#[derive(Clone, Copy, Debug)]
pub struct TakeoffWeightInputs {
    pub active_or_copy_of_active: bool,
    pub phase: FlightPhase,
    pub engines_were_started: bool,
    pub zero_fuel_weight_tonnes: Option<f64>,
    pub block_fuel_tonnes: Option<f64>,
    pub taxi_fuel_tonnes: Option<f64>,
    pub gross_weight_kg: Option<f64>,
}

pub enum TakeoffWeight {
    Frozen,
    Value(Option<f64>),
}

pub fn calculate_takeoff_weight(i: &TakeoffWeightInputs) -> TakeoffWeight {
    if i.active_or_copy_of_active && i.phase >= FlightPhase::Takeoff {
        return TakeoffWeight::Frozen;
    }
    let tow = if !i.active_or_copy_of_active || !i.engines_were_started {
        match (i.zero_fuel_weight_tonnes, i.block_fuel_tonnes, i.taxi_fuel_tonnes) {
            (Some(zfw), Some(block), Some(taxi)) => Some((zfw + block - taxi) * 1000.0),
            _ => None,
        }
    } else {
        i.gross_weight_kg
            .filter(|g| *g != 0.0)
            .map(|gw| gw - i.taxi_fuel_tonnes.unwrap_or(0.0) * 1000.0)
    };
    TakeoffWeight::Value(tow)
}

pub fn recommended_max_altitude_ft(gw_kg: f64, isa_deviation: f64, round_to: f64) -> f64 {
    let v = 52442.0 - 1.241_949_43e-2 * gw_kg - 3.508_131_74e-8 * gw_kg.powi(2) - 53.779_226_9 * isa_deviation
        - 1.879_897_79e-4 * gw_kg * isa_deviation
        - 4.461_817_79 * isa_deviation.powi(2);
    js_round(v / round_to) * round_to
}

pub fn rec_max_altitude_ft(gw_kg: Option<f64>, isa_deviation: f64) -> Option<f64> {
    let gw = gw_kg.filter(|g| *g != 0.0)?;
    Some(recommended_max_altitude_ft(gw, isa_deviation, 500.0).min(MAX_CERTIFIED_ALT))
}

pub fn rec_max_flight_level(gw_kg: Option<f64>, isa_deviation: f64) -> Option<f64> {
    rec_max_altitude_ft(gw_kg, isa_deviation).map(|a| a / 100.0)
}

pub fn opt_flight_level(rec_max_fl: Option<f64>, engine_out: bool, phase: FlightPhase) -> Option<f64> {
    match rec_max_fl {
        Some(r) if !engine_out && phase <= FlightPhase::Cruise => Some(((0.96 * r) / 5.0).floor() * 5.0),
        _ => None,
    }
}

pub fn eo_max_flight_level(rec_max_fl: Option<f64>) -> Option<f64> {
    rec_max_fl.map(|r| ((0.8 * r) / 5.0).floor() * 5.0)
}

pub fn route_reserve_fuel_kg(
    pilot_entry_tonnes: Option<f64>,
    trip_fuel_kg: Option<f64>,
    percentage: Option<f64>,
) -> Option<f64> {
    if let Some(p) = pilot_entry_tonnes {
        return Some(p * 1000.0);
    }
    match (trip_fuel_kg, percentage) {
        (Some(trip), Some(pct)) => Some(trip * (pct / 100.0)),
        _ => None,
    }
}

pub fn extra_fuel_kg(
    in_preflight: bool,
    engines_were_started: bool,
    fob_tonnes: f64,
    block_fuel_tonnes: Option<f64>,
    taxi_fuel_tonnes: Option<f64>,
    trip_fuel_kg: f64,
    min_dest_fob_tonnes: Option<f64>,
    route_reserve_kg: Option<f64>,
    predicted_dest_fob_kg: f64,
) -> f64 {
    let lowest = LOWEST_FUEL_ESTIMATE_LB * POUND_TO_KG;
    if in_preflight {
        let fuel = if engines_were_started { fob_tonnes } else { block_fuel_tonnes.unwrap_or(0.0) };
        (fuel * 1000.0
            - taxi_fuel_tonnes.unwrap_or(0.0) * 1000.0
            - trip_fuel_kg
            - min_dest_fob_tonnes.unwrap_or(0.0) * 1000.0
            - route_reserve_kg.unwrap_or(0.0))
        .max(lowest)
    } else {
        (predicted_dest_fob_kg - min_dest_fob_tonnes.unwrap_or(0.0) * 1000.0).max(lowest)
    }
}

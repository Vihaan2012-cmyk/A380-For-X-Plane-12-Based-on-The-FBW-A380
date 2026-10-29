use crate::command::{Effect, Event, FmsMessage};
use crate::data::{CostIndexMode, PerformanceData};
use crate::js_round;
use crate::limits::*;

/// FBW's `CONFIG_THR_RED_ALT`, `CONFIG_ACCEL_ALT` and `CONFIG_ENG_OUT_ACCEL_ALT` (feet above the airport).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DefaultsConfig {
    pub thr_red_agl: f64,
    pub accel_agl: f64,
    pub eng_out_accel_agl: f64,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        DefaultsConfig { thr_red_agl: DEFAULT_THR_RED_AGL, accel_agl: DEFAULT_ACCEL_AGL, eng_out_accel_agl: DEFAULT_ENG_OUT_ACCEL_AGL }
    }
}

/// FBW `FlightPlan.setOriginDefaultPerformanceData`.
pub fn apply_origin_defaults(pd: &mut PerformanceData, cfg: &DefaultsConfig, elevation_ft: Option<f64>, transition_altitude_ft: Option<f64>) -> Vec<Event> {
    pd.default_thrust_reduction_altitude = elevation_ft.map(|e| e + cfg.thr_red_agl);
    pd.default_acceleration_altitude = elevation_ft.map(|e| e + cfg.accel_agl);
    pd.default_engine_out_acceleration_altitude = elevation_ft.map(|e| e + cfg.eng_out_accel_agl);
    pd.pilot_thrust_reduction_altitude = None;
    pd.pilot_acceleration_altitude = None;
    pd.pilot_engine_out_acceleration_altitude = None;
    pd.database_transition_altitude = transition_altitude_ft;
    [
        "defaultThrustReductionAltitude",
        "defaultAccelerationAltitude",
        "defaultEngineOutAccelerationAltitude",
        "pilotThrustReductionAltitude",
        "pilotAccelerationAltitude",
        "pilotEngineOutAccelerationAltitude",
        "databaseTransitionAltitude",
    ]
    .into_iter()
    .map(Event::Changed)
    .collect()
}

/// FBW `FlightPlan.setDestinationDefaultPerformanceData`.
pub fn apply_destination_defaults(pd: &mut PerformanceData, cfg: &DefaultsConfig, elevation_ft: Option<f64>, transition_level: Option<f64>) -> Vec<Event> {
    pd.default_missed_thrust_reduction_altitude = elevation_ft.map(|e| e + cfg.thr_red_agl);
    pd.default_missed_acceleration_altitude = elevation_ft.map(|e| e + cfg.accel_agl);
    pd.default_missed_engine_out_acceleration_altitude = elevation_ft.map(|e| e + cfg.eng_out_accel_agl);
    pd.pilot_missed_thrust_reduction_altitude = None;
    pd.pilot_missed_acceleration_altitude = None;
    pd.pilot_missed_engine_out_acceleration_altitude = None;
    pd.database_transition_level = transition_level;
    let mut ev: Vec<Event> = [
        "defaultMissedThrustReductionAltitude",
        "defaultMissedAccelerationAltitude",
        "defaultMissedEngineOutAccelerationAltitude",
        "pilotMissedThrustReductionAltitude",
        "pilotMissedAccelerationAltitude",
        "pilotMissedEngineOutAccelerationAltitude",
        "databaseTransitionLevel",
    ]
    .into_iter()
    .map(Event::Changed)
    .collect();
    ev.push(Event::Effect(Effect::DeleteDescentWinds));
    ev
}

/// FBW `FlightPlan.reconcileThrustReductionWithConstraints`; true when the altitude was lowered.
pub fn reconcile_thrust_reduction(pd: &mut PerformanceData, lowest_climb_constraint_ft: f64) -> bool {
    let limit = js_round(lowest_climb_constraint_ft, 10.0);
    match pd.thrust_reduction_altitude() {
        Some(thr) if limit.is_finite() && thr > limit => {
            pd.default_thrust_reduction_altitude = pd.default_thrust_reduction_altitude.map(|d| d.min(limit));
            pd.pilot_thrust_reduction_altitude = pd.pilot_thrust_reduction_altitude.map(|p| p.min(limit));
            true
        }
        _ => false,
    }
}

/// FBW `FlightPlan.reconcileAccelerationWithConstraints`; true when the altitude was lowered.
pub fn reconcile_acceleration(pd: &mut PerformanceData, lowest_climb_constraint_ft: f64) -> bool {
    let limit = js_round(lowest_climb_constraint_ft, 10.0);
    match pd.acceleration_altitude() {
        Some(acc) if limit.is_finite() && acc > limit => {
            pd.default_acceleration_altitude = pd.default_acceleration_altitude.map(|d| d.min(limit));
            pd.pilot_acceleration_altitude = pd.pilot_acceleration_altitude.map(|p| p.min(limit));
            true
        }
        _ => false,
    }
}

/// FBW `A380FlightPlanPerformanceData.pipeTo`, applied once: the fields that follow
/// the active plan into a secondary (weights and block only before engine start).
pub fn pipe_to(from: &PerformanceData, to: &mut PerformanceData, before_engine_start: bool) {
    to.cruise_flight_level = from.cruise_flight_level;
    to.cruise_temperature_pilot_entry = from.cruise_temperature_pilot_entry;
    to.pilot_tropopause = from.pilot_tropopause;
    to.cost_index = from.cost_index;
    to.pilot_route_reserve_fuel = from.pilot_route_reserve_fuel;
    to.pilot_route_reserve_fuel_percentage = from.pilot_route_reserve_fuel_percentage;
    to.pilot_final_holding_fuel = from.pilot_final_holding_fuel;
    to.pilot_final_holding_time = from.pilot_final_holding_time;
    to.cruise_temperature_isa_temp = from.cruise_temperature_isa_temp;
    to.pax_number = from.pax_number;
    to.cost_index_mode = from.cost_index_mode;
    if before_engine_start {
        to.zero_fuel_weight = from.zero_fuel_weight;
        to.zero_fuel_weight_center_of_gravity = from.zero_fuel_weight_center_of_gravity;
        to.pilot_minimum_destination_fuel_on_board = from.pilot_minimum_destination_fuel_on_board;
        to.block_fuel = from.block_fuel;
        to.estimated_takeoff_time = from.estimated_takeoff_time;
        to.estimated_takeoff_time_expired = from.estimated_takeoff_time_expired;
    }
}

/// FBW `FlightManagementComputer.setCi0AndLrcIfActiveFlightplanHasNoCi`.
pub fn default_cost_index_if_missing(pd: &mut PerformanceData, has_destination: bool) -> Vec<Event> {
    if !has_destination || pd.cost_index.is_some() {
        return Vec::new();
    }
    pd.cost_index = Some(0.0);
    pd.cost_index_mode = Some(CostIndexMode::Lrc);
    vec![Event::Changed("costIndex"), Event::Changed("costIndexMode"), Event::Message(FmsMessage::LrcInUse)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_defaults_from_elevation_and_navdata() {
        let mut pd = PerformanceData { pilot_thrust_reduction_altitude: Some(3000.0), ..Default::default() };
        apply_origin_defaults(&mut pd, &DefaultsConfig::default(), Some(83.0), Some(6000.0));
        assert_eq!(pd.default_thrust_reduction_altitude, Some(1583.0));
        assert_eq!(pd.thrust_reduction_altitude(), Some(1580.0));
        assert_eq!(pd.acceleration_altitude(), Some(1580.0));
        assert_eq!(pd.engine_out_acceleration_altitude(), Some(1580.0));
        assert_eq!(pd.pilot_thrust_reduction_altitude, None);
        assert_eq!(pd.transition_altitude(), Some(6000.0));
        assert!(pd.transition_altitude_is_from_database());
        apply_origin_defaults(&mut pd, &DefaultsConfig::default(), None, None);
        assert_eq!(pd.thrust_reduction_altitude(), None);
    }

    #[test]
    fn rounding_matches_js_math_round() {
        let pd = PerformanceData { default_thrust_reduction_altitude: Some(1585.0), pilot_transition_level: Some(179.5), ..Default::default() };
        assert_eq!(pd.thrust_reduction_altitude(), Some(1590.0));
        assert_eq!(pd.transition_level(), Some(180.0));
        assert_eq!(PerformanceData { pilot_tropopause: Some(-15.0), ..Default::default() }.tropopause(), -10.0);
    }

    #[test]
    fn destination_defaults_drop_descent_winds() {
        let mut pd = PerformanceData::default();
        let ev = apply_destination_defaults(&mut pd, &DefaultsConfig::default(), Some(13.0), Some(180.0));
        assert_eq!(pd.missed_thrust_reduction_altitude(), Some(1510.0));
        assert_eq!(pd.transition_level(), Some(180.0));
        assert!(ev.contains(&Event::Effect(Effect::DeleteDescentWinds)));
    }

    #[test]
    fn climb_constraint_caps_thr_red_and_accel() {
        let mut pd = PerformanceData::default();
        apply_origin_defaults(&mut pd, &DefaultsConfig::default(), Some(83.0), Some(6000.0));
        pd.pilot_acceleration_altitude = Some(3000.0);
        assert!(reconcile_thrust_reduction(&mut pd, 1204.0));
        assert!(reconcile_acceleration(&mut pd, 1204.0));
        assert_eq!(pd.thrust_reduction_altitude(), Some(1200.0));
        assert_eq!(pd.pilot_acceleration_altitude, Some(1200.0));
        assert_eq!(pd.default_acceleration_altitude, Some(1200.0));
        assert!(!reconcile_thrust_reduction(&mut pd, f64::INFINITY));
    }

    #[test]
    fn pipe_keeps_weights_only_before_engine_start() {
        let from = PerformanceData { zero_fuel_weight: Some(330.0), cost_index: Some(30.0), ..Default::default() };
        let mut to = PerformanceData::default();
        pipe_to(&from, &mut to, false);
        assert_eq!((to.zero_fuel_weight, to.cost_index), (None, Some(30.0)));
        pipe_to(&from, &mut to, true);
        assert_eq!(to.zero_fuel_weight, Some(330.0));
    }

    #[test]
    fn missing_cost_index_becomes_lrc() {
        let mut pd = PerformanceData::default();
        assert!(default_cost_index_if_missing(&mut pd, false).is_empty());
        let ev = default_cost_index_if_missing(&mut pd, true);
        assert_eq!((pd.cost_index, pd.cost_index_mode), (Some(0.0), Some(CostIndexMode::Lrc)));
        assert!(ev.contains(&Event::Message(FmsMessage::LrcInUse)));
        assert!(default_cost_index_if_missing(&mut pd, true).is_empty());
    }
}

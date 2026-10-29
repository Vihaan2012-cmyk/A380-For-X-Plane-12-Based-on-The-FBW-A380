use serde::{Deserialize, Serialize};

use crate::js_round;
use crate::limits::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TakeoffPowerSetting {
    #[default]
    Toga = 0,
    Flex = 1,
    Derated = 2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TakeoffDerated {
    #[default]
    D01 = 0,
    D02 = 1,
    D03 = 2,
    D04 = 3,
    D05 = 4,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TakeoffPacks {
    OffApu = 0,
    #[default]
    On = 1,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TakeoffAntiIce {
    #[default]
    Off = 0,
    EngOnly = 1,
    EngWings = 2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CostIndexMode {
    Lrc = 0,
    #[default]
    Econ = 1,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClimbDerated {
    #[default]
    None = 0,
    D01 = 1,
    D02 = 2,
    D03 = 3,
    D04 = 4,
    D05 = 5,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WindVector {
    pub direction: Option<f64>,
    pub magnitude: Option<f64>,
}

/// Weights and fuel are in tonnes, as FBW stores them; entries take kilograms.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceData {
    pub cruise_flight_level: Option<f64>,
    pub cruise_temperature_pilot_entry: Option<f64>,
    pub cruise_temperature_isa_temp: Option<f64>,
    pub cost_index: Option<f64>,
    pub default_tropopause: f64,
    pub pilot_tropopause: Option<f64>,

    pub v1: Option<f64>,
    pub vr: Option<f64>,
    pub v2: Option<f64>,

    pub pilot_thrust_reduction_altitude: Option<f64>,
    pub default_thrust_reduction_altitude: Option<f64>,
    pub pilot_acceleration_altitude: Option<f64>,
    pub default_acceleration_altitude: Option<f64>,
    pub pilot_engine_out_acceleration_altitude: Option<f64>,
    pub default_engine_out_acceleration_altitude: Option<f64>,
    pub pilot_missed_thrust_reduction_altitude: Option<f64>,
    pub default_missed_thrust_reduction_altitude: Option<f64>,
    pub pilot_missed_acceleration_altitude: Option<f64>,
    pub default_missed_acceleration_altitude: Option<f64>,
    pub pilot_missed_engine_out_acceleration_altitude: Option<f64>,
    pub default_missed_engine_out_acceleration_altitude: Option<f64>,

    pub database_transition_altitude: Option<f64>,
    pub pilot_transition_altitude: Option<f64>,
    pub database_transition_level: Option<f64>,
    pub pilot_transition_level: Option<f64>,

    pub climb_speed_limit_speed: Option<f64>,
    pub climb_speed_limit_altitude: Option<f64>,
    pub is_climb_speed_limit_pilot_entered: bool,
    pub descent_speed_limit_speed: Option<f64>,
    pub descent_speed_limit_altitude: Option<f64>,
    pub is_descent_speed_limit_pilot_entered: bool,
    #[serde(default = "default_speed_limit_speed")]
    pub alternate_climb_speed_limit_speed: Option<f64>,
    #[serde(default = "default_speed_limit_altitude")]
    pub alternate_climb_speed_limit_altitude: Option<f64>,
    #[serde(default)]
    pub is_alternate_climb_speed_limit_pilot_entered: bool,
    #[serde(default = "default_speed_limit_speed")]
    pub alternate_descent_speed_limit_speed: Option<f64>,
    #[serde(default = "default_speed_limit_altitude")]
    pub alternate_descent_speed_limit_altitude: Option<f64>,
    #[serde(default)]
    pub is_alternate_descent_speed_limit_pilot_entered: bool,

    pub zero_fuel_weight: Option<f64>,
    pub zero_fuel_weight_center_of_gravity: Option<f64>,
    pub block_fuel: Option<f64>,
    pub pilot_taxi_fuel: Option<f64>,
    pub default_taxi_fuel: f64,
    pub pilot_route_reserve_fuel: Option<f64>,
    pub pilot_route_reserve_fuel_percentage: Option<f64>,
    pub default_route_reserve_fuel_percentage: f64,
    pub pilot_alternate_fuel: Option<f64>,
    pub calculated_alternate_fuel: Option<f64>,
    pub pilot_final_holding_fuel: Option<f64>,
    pub calculated_final_holding_fuel: Option<f64>,
    pub pilot_final_holding_time: Option<f64>,
    pub calculated_final_holding_time: Option<f64>,
    pub default_final_holding_time: f64,
    pub pilot_minimum_destination_fuel_on_board: Option<f64>,
    pub pilot_trip_wind: Option<f64>,
    pub pax_number: Option<f64>,
    pub takeoff_weight: Option<f64>,

    pub takeoff_shift: Option<f64>,
    pub takeoff_flaps: Option<u8>,
    pub trimmable_horizontal_stabilizer: Option<f64>,
    #[serde(rename = "flexTemperature")]
    pub flex_takeoff_temperature: Option<f64>,
    pub takeoff_power_setting: TakeoffPowerSetting,
    pub takeoff_derated_setting: TakeoffDerated,
    pub takeoff_ths_for: Option<f64>,
    pub takeoff_packs: Option<TakeoffPacks>,
    pub takeoff_anti_ice: Option<TakeoffAntiIce>,
    pub noise_enabled: bool,
    pub noise_n1: Option<f64>,
    pub noise_speed: Option<f64>,
    pub noise_end_altitude: Option<f64>,

    pub preselected_climb_speed: Option<f64>,
    pub preselected_cruise_speed: Option<f64>,
    pub pilot_managed_descent_speed: Option<f64>,
    pub pilot_managed_descent_mach: Option<f64>,
    pub cost_index_mode: Option<CostIndexMode>,
    pub climb_derated: Option<ClimbDerated>,
    pub descent_cabin_rate: Option<f64>,

    pub approach_qnh: Option<f64>,
    pub approach_temperature: Option<f64>,
    pub approach_wind_direction: Option<f64>,
    pub approach_wind_magnitude: Option<f64>,
    pub is_approach_wind_pilot_entered: bool,
    pub pilot_vapp: Option<f64>,
    pub approach_baro_minimum: Option<f64>,
    pub approach_radio_minimum: Option<f64>,
    pub approach_flaps_three_selected: bool,

    pub alternate_wind: WindVector,
    pub estimated_takeoff_time: Option<f64>,
    pub estimated_takeoff_time_expired: Option<bool>,
}

impl Default for PerformanceData {
    fn default() -> Self {
        PerformanceData {
            cruise_flight_level: None,
            cruise_temperature_pilot_entry: None,
            cruise_temperature_isa_temp: None,
            cost_index: None,
            default_tropopause: DEFAULT_TROPOPAUSE_ALT,
            pilot_tropopause: None,
            v1: None,
            vr: None,
            v2: None,
            pilot_thrust_reduction_altitude: None,
            default_thrust_reduction_altitude: None,
            pilot_acceleration_altitude: None,
            default_acceleration_altitude: None,
            pilot_engine_out_acceleration_altitude: None,
            default_engine_out_acceleration_altitude: None,
            pilot_missed_thrust_reduction_altitude: None,
            default_missed_thrust_reduction_altitude: None,
            pilot_missed_acceleration_altitude: None,
            default_missed_acceleration_altitude: None,
            pilot_missed_engine_out_acceleration_altitude: None,
            default_missed_engine_out_acceleration_altitude: None,
            database_transition_altitude: None,
            pilot_transition_altitude: None,
            database_transition_level: None,
            pilot_transition_level: None,
            climb_speed_limit_speed: Some(DEFAULT_CLIMB_SPEED_LIMIT.0),
            climb_speed_limit_altitude: Some(DEFAULT_CLIMB_SPEED_LIMIT.1),
            is_climb_speed_limit_pilot_entered: false,
            descent_speed_limit_speed: Some(DEFAULT_CLIMB_SPEED_LIMIT.0),
            descent_speed_limit_altitude: Some(DEFAULT_CLIMB_SPEED_LIMIT.1),
            is_descent_speed_limit_pilot_entered: false,
            alternate_climb_speed_limit_speed: default_speed_limit_speed(),
            alternate_climb_speed_limit_altitude: default_speed_limit_altitude(),
            is_alternate_climb_speed_limit_pilot_entered: false,
            alternate_descent_speed_limit_speed: default_speed_limit_speed(),
            alternate_descent_speed_limit_altitude: default_speed_limit_altitude(),
            is_alternate_descent_speed_limit_pilot_entered: false,
            zero_fuel_weight: None,
            zero_fuel_weight_center_of_gravity: None,
            block_fuel: None,
            pilot_taxi_fuel: None,
            default_taxi_fuel: DEFAULT_TAXI_FUEL_T,
            pilot_route_reserve_fuel: None,
            pilot_route_reserve_fuel_percentage: None,
            default_route_reserve_fuel_percentage: DEFAULT_ROUTE_RESERVE_PERC,
            pilot_alternate_fuel: None,
            calculated_alternate_fuel: Some(0.0),
            pilot_final_holding_fuel: None,
            calculated_final_holding_fuel: None,
            pilot_final_holding_time: None,
            calculated_final_holding_time: None,
            default_final_holding_time: DEFAULT_FINAL_HOLDING_TIME_MIN,
            pilot_minimum_destination_fuel_on_board: None,
            pilot_trip_wind: None,
            pax_number: None,
            takeoff_weight: None,
            takeoff_shift: None,
            takeoff_flaps: None,
            trimmable_horizontal_stabilizer: None,
            flex_takeoff_temperature: None,
            takeoff_power_setting: TakeoffPowerSetting::Toga,
            takeoff_derated_setting: TakeoffDerated::D01,
            takeoff_ths_for: None,
            takeoff_packs: Some(TakeoffPacks::On),
            takeoff_anti_ice: Some(TakeoffAntiIce::Off),
            noise_enabled: false,
            noise_n1: None,
            noise_speed: None,
            noise_end_altitude: None,
            preselected_climb_speed: None,
            preselected_cruise_speed: None,
            pilot_managed_descent_speed: None,
            pilot_managed_descent_mach: None,
            cost_index_mode: Some(CostIndexMode::Econ),
            climb_derated: Some(ClimbDerated::None),
            descent_cabin_rate: Some(DEFAULT_DESCENT_CABIN_RATE),
            approach_qnh: None,
            approach_temperature: None,
            approach_wind_direction: None,
            approach_wind_magnitude: None,
            is_approach_wind_pilot_entered: false,
            pilot_vapp: None,
            approach_baro_minimum: None,
            approach_radio_minimum: None,
            approach_flaps_three_selected: false,
            alternate_wind: WindVector::default(),
            estimated_takeoff_time: None,
            estimated_takeoff_time_expired: Some(false),
        }
    }
}

fn default_speed_limit_speed() -> Option<f64> {
    Some(DEFAULT_CLIMB_SPEED_LIMIT.0)
}

fn default_speed_limit_altitude() -> Option<f64> {
    Some(DEFAULT_CLIMB_SPEED_LIMIT.1)
}

fn round10(v: Option<f64>) -> Option<f64> {
    v.map(|x| js_round(x, 10.0))
}

impl PerformanceData {
    pub fn cruise_temperature(&self) -> Option<f64> {
        self.cruise_temperature_pilot_entry.or(self.cruise_temperature_isa_temp)
    }

    pub fn tropopause(&self) -> f64 {
        js_round(self.pilot_tropopause.unwrap_or(self.default_tropopause), 10.0)
    }

    pub fn thrust_reduction_altitude(&self) -> Option<f64> {
        round10(self.pilot_thrust_reduction_altitude.or(self.default_thrust_reduction_altitude))
    }

    pub fn acceleration_altitude(&self) -> Option<f64> {
        round10(self.pilot_acceleration_altitude.or(self.default_acceleration_altitude))
    }

    pub fn engine_out_acceleration_altitude(&self) -> Option<f64> {
        round10(self.pilot_engine_out_acceleration_altitude.or(self.default_engine_out_acceleration_altitude))
    }

    pub fn missed_thrust_reduction_altitude(&self) -> Option<f64> {
        round10(self.pilot_missed_thrust_reduction_altitude.or(self.default_missed_thrust_reduction_altitude))
    }

    pub fn missed_acceleration_altitude(&self) -> Option<f64> {
        round10(self.pilot_missed_acceleration_altitude.or(self.default_missed_acceleration_altitude))
    }

    pub fn missed_engine_out_acceleration_altitude(&self) -> Option<f64> {
        round10(self.pilot_missed_engine_out_acceleration_altitude.or(self.default_missed_engine_out_acceleration_altitude))
    }

    pub fn transition_altitude(&self) -> Option<f64> {
        round10(self.pilot_transition_altitude.or(self.database_transition_altitude))
    }

    pub fn transition_altitude_is_from_database(&self) -> bool {
        self.pilot_transition_altitude.is_none()
    }

    pub fn transition_level(&self) -> Option<f64> {
        self.pilot_transition_level.or(self.database_transition_level).map(|x| js_round(x, 1.0))
    }

    pub fn transition_level_is_from_database(&self) -> bool {
        self.pilot_transition_level.is_none()
    }

    pub fn taxi_fuel(&self) -> f64 {
        self.pilot_taxi_fuel.unwrap_or(self.default_taxi_fuel)
    }

    pub fn route_reserve_fuel_percentage(&self) -> f64 {
        self.pilot_route_reserve_fuel_percentage.unwrap_or(self.default_route_reserve_fuel_percentage)
    }

    pub fn is_route_reserve_fuel_pilot_entered(&self) -> bool {
        self.pilot_route_reserve_fuel.is_some()
    }

    pub fn alternate_fuel(&self) -> Option<f64> {
        self.pilot_alternate_fuel.or(self.calculated_alternate_fuel)
    }

    pub fn final_holding_time(&self) -> f64 {
        self.pilot_final_holding_time.or(self.calculated_final_holding_time).unwrap_or(self.default_final_holding_time)
    }

    pub fn final_holding_fuel(&self) -> Option<f64> {
        self.pilot_final_holding_fuel.or(self.calculated_final_holding_fuel)
    }

    pub fn minimum_destination_fuel_on_board(&self) -> Option<f64> {
        self.pilot_minimum_destination_fuel_on_board.or(match (self.alternate_fuel(), self.final_holding_fuel()) {
            (Some(a), Some(f)) => Some(a + f),
            _ => None,
        })
    }

    pub fn v_speeds_in_order(&self) -> bool {
        let le = |a: Option<f64>, b: Option<f64>| match (a, b) {
            (Some(a), Some(b)) if a != 0.0 && b != 0.0 => a <= b,
            _ => true,
        };
        le(self.v1, self.vr) && le(self.vr, self.v2) && le(self.v1, self.v2)
    }

    pub fn v_speeds_inserted(&self) -> bool {
        [self.v1, self.vr, self.v2].iter().all(|v| v.is_some_and(|x| x != 0.0))
    }

    /// The value FBW writes to `L:A32NX_AIRLINER_TO_FLEX_TEMP`: 0 means no flex, 0.1 means a flex of 0 degrees.
    pub fn airliner_flex_lvar(&self) -> f64 {
        if self.takeoff_power_setting != TakeoffPowerSetting::Flex {
            return 0.0;
        }
        match self.flex_takeoff_temperature {
            Some(t) if t == 0.0 => 0.1,
            Some(t) => t,
            None => 0.0,
        }
    }
}

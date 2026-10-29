use crate::status::{Clock, Waypoint};

#[derive(Clone, Debug, PartialEq)]
pub struct RouteData {
    pub last_waypoint: Option<Waypoint>,
    pub active_waypoint: Option<Waypoint>,
    pub next_waypoint: Option<Waypoint>,
    pub destination: Option<Waypoint>,
}

impl RouteData {
    pub fn empty() -> RouteData {
        RouteData { last_waypoint: None, active_waypoint: None, next_waypoint: None, destination: None }
    }
}

/// Snapshot of the digital inputs the ATC reads. `None` stands for a value that
/// is not in normal operation (failed, no computed data, no computed data).
#[derive(Clone, Debug, PartialEq)]
pub struct AtcInputs {
    pub clock: Clock,
    pub baro_std: bool,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub altitude: Option<f64>,
    pub heading: Option<f64>,
    pub track: Option<f64>,
    pub computed_airspeed: Option<f64>,
    pub mach: Option<f64>,
    pub ground_speed: Option<f64>,
    pub vertical_speed: Option<f64>,
    pub ap_active: Option<bool>,
    pub autothrust_active: Option<bool>,
    pub selected_altitude: f64,
    pub selected_speed: Option<f64>,
    pub selected_mach: Option<f64>,
    pub mach_mode: bool,
    pub wind_direction: Option<f64>,
    pub wind_speed: Option<f64>,
    pub static_air_temperature: Option<f64>,
    pub transponder_code: u32,
    pub route: RouteData,
}

impl AtcInputs {
    pub fn new(clock: Clock) -> AtcInputs {
        AtcInputs {
            clock,
            baro_std: true,
            latitude: None,
            longitude: None,
            altitude: None,
            heading: None,
            track: None,
            computed_airspeed: None,
            mach: None,
            ground_speed: None,
            vertical_speed: None,
            ap_active: None,
            autothrust_active: None,
            selected_altitude: 0.0,
            selected_speed: None,
            selected_mach: None,
            mach_mode: false,
            wind_direction: None,
            wind_speed: None,
            static_air_temperature: None,
            transponder_code: 2000,
            route: RouteData::empty(),
        }
    }
}

pub fn js_number(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{}", value)
    }
}

pub fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

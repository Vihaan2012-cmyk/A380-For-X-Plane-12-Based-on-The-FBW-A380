pub const WING_AREA_FT2: f64 = 9096.0;
const POUND_TO_TONNE: f64 = 0.000_453_592_37;

pub fn initial_mach_estimate(weight_lb: f64, delta: f64) -> f64 {
    1.565 * (weight_lb / (1481.0 * WING_AREA_FT2 * delta)).sqrt()
}

pub fn cost_index_to_mach(ci: f64, mmrc: f64, headwind_kt: f64) -> f64 {
    let mmrc = mmrc + headwind_kt / 10_000.0;
    -1.0 * (0.8 - mmrc) * (-0.05 * ci).exp() + 0.8
}

pub fn cost_index_to_climb_cas(ci: f64, flight_level: f64, weight_lb: f64) -> f64 {
    let tons = (weight_lb * POUND_TO_TONNE).max(50.0).min(77.0);
    let airspeed = 240.0 + 0.1 * flight_level + ci + 0.5 * (tons - 50.0);
    airspeed.max(270.0).min(340.0)
}

pub fn cost_index_to_descent_cas(ci: f64, flight_level: f64, weight_lb: f64) -> f64 {
    let tons = (weight_lb * POUND_TO_TONNE).max(50.0).min(77.0);
    let airspeed = 205.0 + 0.13 * flight_level + 1.5 * ci - 0.05 * (tons - 50.0);
    airspeed.max(270.0).min(340.0)
}

pub fn managed_climb_speed(cost_index: Option<f64>) -> f64 {
    match cost_index {
        Some(ci) => {
            let d = (ci / 999.0).powi(2);
            290.0 * (1.0 - d) + 330.0 * d
        }
        None => 250.0,
    }
}

pub fn managed_climb_speed_mach() -> f64 {
    0.84
}

pub fn managed_cruise_speed() -> f64 {
    310.0
}

pub fn managed_cruise_speed_mach() -> f64 {
    0.85
}

pub fn managed_descent_speed(cost_index: Option<f64>, pilot_entry: Option<f64>) -> f64 {
    if let Some(p) = pilot_entry {
        return p;
    }
    match cost_index {
        Some(ci) => {
            let d = ci / 999.0;
            crate::speeds::js_round(288.0 * (1.0 - d) + 300.0 * d)
        }
        None => 300.0,
    }
}

pub fn managed_descent_speed_mach(pilot_entry: Option<f64>) -> f64 {
    pilot_entry.unwrap_or(0.84)
}

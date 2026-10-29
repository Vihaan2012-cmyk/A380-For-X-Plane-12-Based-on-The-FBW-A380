// Translated from FlyByWire's fmgc guidance/vnav/FlightModel.ts (GPL-3.0-or-later).

use crate::atmosphere::interpolate as lerp;
use crate::config::{FlapConf, FlightModelParameters};

pub fn lift_coefficient(p: &FlightModelParameters, weight: f64, mach: f64, delta: f64, load_factor: f64) -> f64 {
    (weight * load_factor) / (1481.4 * mach * mach * delta * p.wing_area)
}

pub fn lift_coefficient_from_eas(p: &FlightModelParameters, lift: f64, eas: f64) -> f64 {
    (295.369 * lift) / (eas * eas * p.wing_area)
}

pub fn drag_coefficient(
    p: &FlightModelParameters,
    cl: f64,
    spd_brk_deflected: bool,
    gear_extended: bool,
    flap_conf: FlapConf,
) -> f64 {
    let base: f64 = p.drag_polar_coefficients[flap_conf as usize]
        .iter()
        .enumerate()
        .map(|(i, c)| c * cl.powi(i as i32))
        .sum();
    let spd = if spd_brk_deflected { p.speed_brake_drag } else { 0.0 };
    let gear = if gear_extended { p.gear_drag } else { 0.0 };
    base + spd + gear
}

pub fn drag(
    p: &FlightModelParameters,
    weight: f64,
    mach: f64,
    delta: f64,
    spd_brk_deflected: bool,
    gear_extended: bool,
    flap_conf: FlapConf,
) -> f64 {
    let cl = lift_coefficient(p, weight, mach, delta, 1.0);
    let cd = drag_coefficient(p, cl, spd_brk_deflected, gear_extended, flap_conf);
    let delta_cd = mach_correction(p, mach, flap_conf);
    1481.4 * mach * mach * delta * p.wing_area * (cd + delta_cd)
}

pub fn mach_correction(p: &FlightModelParameters, mach: f64, flap_conf: FlapConf) -> f64 {
    if flap_conf != FlapConf::Clean {
        return 0.0;
    }
    table_lookup(mach, &p.mach_values, &p.drag_coefficient_corrections)
}

pub fn table_lookup(x: f64, xs: &[f64], ys: &[f64]) -> f64 {
    if x <= xs[0] {
        return ys[0];
    }
    for i in 0..xs.len() - 1 {
        if x > xs[i] && x <= xs[i + 1] {
            return lerp(x, xs[i], xs[i + 1], ys[i], ys[i + 1]);
        }
    }
    ys[ys.len() - 1]
}

pub fn available_gradient(thrust: f64, drag: f64, weight: f64) -> f64 {
    ((thrust - drag) / weight).asin()
}

pub fn acceleration_for_gradient(available_gradient: f64, fpa: f64, accel_factor: f64) -> f64 {
    (available_gradient.sin() - fpa.sin()) * accel_factor
}

pub fn fpa_for_gradient(available_gradient: f64, acceleration: f64, accel_factor: f64) -> f64 {
    (available_gradient.sin() - acceleration / accel_factor).asin()
}

pub fn constant_thrust_path_angle(thrust: f64, weight: f64, drag: f64, accel_factor: f64) -> f64 {
    ((thrust - drag) / weight / accel_factor).asin()
}

pub fn constant_thrust_path_angle_from_coefficients(thrust: f64, weight: f64, cl: f64, cd: f64, accel_factor: f64) -> f64 {
    ((thrust / weight - cd / cl) / accel_factor).asin()
}

pub fn thrust_from_constant_path_angle(fpa_deg: f64, weight: f64, drag: f64, accel_factor: f64) -> f64 {
    weight * (accel_factor * fpa_deg.to_radians().sin()) + drag
}

pub fn thrust_from_constant_path_angle_coefficients(fpa_deg: f64, weight: f64, cl: f64, cd: f64, accel_factor: f64) -> f64 {
    weight * (accel_factor * fpa_deg.to_radians().sin() + cd / cl)
}

pub fn speed_change_path_angle(p: &FlightModelParameters, thrust: f64, weight: f64, drag: f64) -> f64 {
    ((thrust - drag) / weight - (1.0 / p.gravity_const_ms2) * p.required_accel_rate_ms2).asin()
}

pub fn speed_change_path_angle_from_coefficients(p: &FlightModelParameters, thrust: f64, weight: f64, cl: f64, cd: f64) -> f64 {
    (thrust / weight - cd / cl - (1.0 / p.gravity_const_ms2) * p.required_accel_rate_ms2).asin()
}

pub fn accel_rate_from_idle_geo_path(p: &FlightModelParameters, thrust: f64, weight: f64, drag: f64, fpa_deg: f64) -> f64 {
    p.gravity_const_kns * ((thrust - drag) / weight - fpa_deg.to_radians().sin())
}

pub fn accel_rate_from_idle_geo_path_coefficients(
    p: &FlightModelParameters,
    thrust: f64,
    weight: f64,
    cl: f64,
    cd: f64,
    fpa_deg: f64,
) -> f64 {
    p.gravity_const_kns * (thrust / weight - cd / cl - fpa_deg.to_radians().sin())
}

pub fn acceleration_distance(
    thrust: f64,
    drag: f64,
    weight: f64,
    initial_speed: f64,
    target_speed: f64,
    fpa: f64,
    accel_factor: f64,
) -> f64 {
    let sign = if fpa > 0.0 {
        1.0
    } else if fpa < 0.0 {
        -1.0
    } else {
        0.0
    };
    let force = thrust - drag + sign * weight * (fpa * (std::f64::consts::PI / 180.0)).sin() * accel_factor;
    let accel = force / weight;
    let time = (target_speed - initial_speed) / accel;
    initial_speed * time + 0.5 * accel * time * time
}

pub fn green_dot_speed_cas(altitude: f64, weight_kg: f64) -> f64 {
    weight_kg / 500.0 + 85.0 + 0.0f64.max((altitude - 20000.0) / 1000.0)
}

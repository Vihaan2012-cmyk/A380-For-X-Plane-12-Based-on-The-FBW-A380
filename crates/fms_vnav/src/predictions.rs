// Translated from FlyByWire's fmgc guidance/vnav/Predictions.ts (GPL-3.0-or-later).

use crate::atmosphere::{self as atm, AccelFactorMode};
use crate::config::{AircraftConfig, FlapConf};
use crate::engine as eng;
use crate::flight_model as fm;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VnavStepError {
    AvailableGradientInsufficient,
    TooLowDeceleration,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StepResults {
    pub path_angle: f64,
    pub vertical_speed: f64,
    pub distance_traveled: f64,
    pub fuel_burned: f64,
    pub time_elapsed: f64,
    pub initial_altitude: f64,
    pub final_altitude: f64,
    pub error: Option<VnavStepError>,
    pub speed: f64,
    pub predicted_n1: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct StepEnv {
    pub zero_fuel_weight: f64,
    pub isa_dev: f64,
    pub tropo_altitude: f64,
    pub perf_factor_percent: f64,
    pub speedbrakes_extended: bool,
    pub flaps: FlapConf,
    pub gear_extended: bool,
}

impl StepEnv {
    pub fn new(zero_fuel_weight: f64, isa_dev: f64, tropo_altitude: f64, perf_factor_percent: f64) -> Self {
        Self {
            zero_fuel_weight,
            isa_dev,
            tropo_altitude,
            perf_factor_percent,
            speedbrakes_extended: false,
            flaps: FlapConf::Clean,
            gear_extended: false,
        }
    }
}

struct Air {
    theta: f64,
    delta: f64,
    mach: f64,
    tas: f64,
    mode: AccelFactorMode,
}

fn air_at(alt: f64, cas: f64, econ_mach: f64, env: &StepEnv) -> Air {
    let above = alt > env.tropo_altitude;
    let theta = atm::theta(alt, env.isa_dev, above);
    let delta = atm::delta(alt, above);
    let mut mach = atm::cas_to_mach(cas, delta);
    if mach > econ_mach {
        mach = econ_mach;
        Air { theta, delta, mach, tas: atm::mach_to_tas(mach, theta), mode: AccelFactorMode::ConstantMach }
    } else {
        Air { theta, delta, mach, tas: atm::cas_to_tas(cas, theta, delta), mode: AccelFactorMode::ConstantCas }
    }
}

fn thrust_and_flow(
    cfg: &AircraftConfig,
    n1: f64,
    mach: f64,
    alt_for_flow: f64,
    theta: f64,
    delta: f64,
    env: &StepEnv,
) -> (f64, f64) {
    let theta2 = atm::theta2(theta, mach);
    let delta2 = atm::delta2(delta, mach);
    let e = &cfg.engine;
    let corrected_thrust = eng::table_interpolation(&e.table1506, n1, mach) * e.number_of_engines * e.max_thrust;
    let corrected_flow = eng::corrected_fuel_flow(e, n1, mach, alt_for_flow) * e.number_of_engines;
    let thrust = eng::uncorrected_thrust(corrected_thrust, delta2);
    let flow = 0.0f64.max(eng::uncorrected_fuel_flow(corrected_flow, delta2, theta2) * (1.0 + env.perf_factor_percent / 100.0));
    (thrust, flow)
}

fn flow_from_thrust(
    cfg: &AircraftConfig,
    thrust: f64,
    mach: f64,
    alt: f64,
    theta: f64,
    delta: f64,
    env: &StepEnv,
) -> (f64, f64) {
    let theta2 = atm::theta2(theta, mach);
    let delta2 = atm::delta2(delta, mach);
    let e = &cfg.engine;
    let corrected_thrust = thrust / delta2 / e.number_of_engines;
    let n1 = eng::reverse_table_interpolation(&e.table1506, mach, corrected_thrust / e.max_thrust);
    let corrected_flow = eng::corrected_fuel_flow(e, n1, mach, alt) * e.number_of_engines;
    let flow = 0.0f64.max(eng::uncorrected_fuel_flow(corrected_flow, delta2, theta2) * (1.0 + env.perf_factor_percent / 100.0));
    (n1, flow)
}

pub fn altitude_step(
    cfg: &AircraftConfig,
    initial_altitude: f64,
    step_size: f64,
    econ_cas: f64,
    econ_mach: f64,
    corrected_n1: f64,
    initial_fuel: f64,
    headwind: f64,
    env: &StepEnv,
) -> StepResults {
    let mid = initial_altitude + step_size / 2.0;
    let a = air_at(mid, econ_cas, econ_mach, env);
    let (thrust, fuel_flow) = thrust_and_flow(cfg, corrected_n1, a.mach, mid, a.theta, a.delta, env);

    let mut mid_weight = env.zero_fuel_weight + initial_fuel;
    let mut prev;
    let mut iterations = 0;
    let (mut path_angle, mut vs, mut time, mut dist, mut burned);
    loop {
        let drag = fm::drag(&cfg.flight, mid_weight, a.mach, a.delta, env.speedbrakes_extended, env.gear_extended, env.flaps);
        let af = atm::acceleration_factor(a.mach, mid, env.isa_dev, mid > env.tropo_altitude, a.mode);
        path_angle = fm::constant_thrust_path_angle(thrust, mid_weight, drag, af);
        vs = 101.268 * a.tas * path_angle.sin();
        time = if vs != 0.0 { 60.0 * step_size / vs } else { 0.0 };
        dist = (a.tas - headwind) * (time / 3600.0);
        burned = fuel_flow / 3600.0 * time;
        prev = mid_weight;
        mid_weight = env.zero_fuel_weight + (initial_fuel - burned / 2.0);
        iterations += 1;
        if !(iterations < 4 && (prev - mid_weight).abs() > 10.0) {
            break;
        }
    }
    StepResults {
        path_angle: path_angle.to_degrees(),
        vertical_speed: vs,
        time_elapsed: time,
        distance_traveled: dist,
        fuel_burned: burned,
        initial_altitude,
        final_altitude: initial_altitude + step_size,
        speed: econ_cas,
        ..Default::default()
    }
}

pub fn distance_step(
    cfg: &AircraftConfig,
    initial_altitude: f64,
    distance: f64,
    econ_cas: f64,
    econ_mach: f64,
    corrected_n1: f64,
    initial_fuel: f64,
    headwind: f64,
    env: &StepEnv,
) -> StepResults {
    let mut final_alt = initial_altitude;
    let mut prev_final;
    let mut mid_weight = env.zero_fuel_weight + initial_fuel;
    let mut iterations = 0;
    let (mut path_angle, mut vs, mut time, mut burned);
    loop {
        let mid = (initial_altitude + final_alt) / 2.0;
        let a = air_at(mid, econ_cas, econ_mach, env);
        let (thrust, fuel_flow) = thrust_and_flow(cfg, corrected_n1, a.mach, initial_altitude, a.theta, a.delta, env);
        let drag = fm::drag(&cfg.flight, mid_weight, a.mach, a.delta, env.speedbrakes_extended, env.gear_extended, env.flaps);
        let af = atm::acceleration_factor(a.mach, mid, env.isa_dev, mid > env.tropo_altitude, a.mode);
        path_angle = fm::constant_thrust_path_angle(thrust, mid_weight, drag, af);
        vs = 101.268 * a.tas * path_angle.sin();
        time = if a.tas - headwind != 0.0 { 3600.0 * distance / (a.tas - headwind) } else { 0.0 };
        let step_size = (time / 60.0) * vs;
        burned = fuel_flow / 3600.0 * time;
        prev_final = final_alt;
        mid_weight = env.zero_fuel_weight + (initial_fuel - burned / 2.0);
        final_alt = initial_altitude + step_size;
        iterations += 1;
        if !(iterations < 4 && (final_alt - prev_final).abs() > 10.0) {
            break;
        }
    }
    StepResults {
        path_angle: path_angle.to_degrees(),
        vertical_speed: vs,
        time_elapsed: time,
        distance_traveled: distance,
        fuel_burned: burned,
        initial_altitude,
        final_altitude: final_alt,
        speed: econ_cas,
        ..Default::default()
    }
}

pub fn level_flight_step(
    cfg: &AircraftConfig,
    altitude: f64,
    step_size: f64,
    econ_cas: f64,
    econ_mach: f64,
    initial_fuel: f64,
    headwind: f64,
    env: &StepEnv,
) -> StepResults {
    let above = altitude > env.tropo_altitude;
    let theta = atm::theta(altitude, env.isa_dev, above);
    let delta = atm::delta(altitude, above);
    let mut mach = atm::cas_to_mach(econ_cas, delta);
    let tas = if mach > econ_mach {
        mach = econ_mach;
        atm::mach_to_tas(mach, theta)
    } else {
        atm::cas_to_tas(econ_cas, theta, delta)
    };
    let weight = env.zero_fuel_weight + initial_fuel;
    let thrust = fm::drag(&cfg.flight, weight, mach, delta, env.speedbrakes_extended, env.gear_extended, env.flaps);
    let (_, fuel_flow) = flow_from_thrust(cfg, thrust, mach, altitude, theta, delta, env);
    let time = step_size / (tas - headwind) * 3600.0;
    StepResults {
        time_elapsed: time,
        distance_traveled: step_size,
        fuel_burned: fuel_flow / 3600.0 * time,
        final_altitude: altitude,
        initial_altitude: altitude,
        speed: econ_cas,
        ..Default::default()
    }
}

pub fn speed_change_step(
    cfg: &AircraftConfig,
    flight_path_angle: f64,
    initial_altitude: f64,
    initial_cas: f64,
    final_cas: f64,
    initial_mach: f64,
    final_mach: f64,
    corrected_n1: f64,
    initial_fuel: f64,
    headwind: f64,
    env: &StepEnv,
    minimum_absolute_acceleration: Option<f64>,
) -> StepResults {
    let above = initial_altitude > env.tropo_altitude;
    let theta = atm::theta(initial_altitude, env.isa_dev, above);
    let delta = atm::delta(initial_altitude, above);

    let mut act_initial_mach = atm::cas_to_mach(initial_cas, delta);
    let mut act_final_mach = atm::cas_to_mach(final_cas, delta);
    let initial_tas = if act_initial_mach > initial_mach {
        act_initial_mach = initial_mach;
        atm::mach_to_tas(act_initial_mach, theta)
    } else {
        atm::cas_to_tas(initial_cas, theta, delta)
    };
    let final_tas = if act_final_mach > final_mach {
        act_final_mach = final_mach;
        atm::mach_to_tas(act_final_mach, theta)
    } else {
        atm::cas_to_tas(final_cas, theta, delta)
    };
    let avg_mach = (act_initial_mach + act_final_mach) / 2.0;
    let avg_tas = (initial_tas + final_tas) / 2.0;

    let (thrust, fuel_flow) = thrust_and_flow(cfg, corrected_n1, avg_mach, initial_altitude, theta, delta, env);
    let weight_estimate = env.zero_fuel_weight + initial_fuel;

    let mut path_angle;
    let mut error = None;
    let (mut vs, mut time, mut dist, mut burned, mut final_alt) = (0.0, 0.0, 0.0, 0.0, f64::NAN);
    let mut lift = weight_estimate;
    let mut mid_weight = weight_estimate;
    let mut prev;
    let mut iterations = 0;
    loop {
        let drag = fm::drag(&cfg.flight, lift, avg_mach, delta, env.speedbrakes_extended, env.gear_extended, env.flaps);
        let gradient = fm::available_gradient(thrust, drag, weight_estimate);
        path_angle = flight_path_angle.to_radians();
        if gradient.abs() < path_angle.abs() {
            error = Some(VnavStepError::AvailableGradientInsufficient);
            path_angle = gradient;
            break;
        }
        let acceleration = fm::acceleration_for_gradient(gradient, path_angle, cfg.flight.gravity_const_kns);
        if let Some(min) = minimum_absolute_acceleration {
            if acceleration.abs() < min {
                error = Some(VnavStepError::TooLowDeceleration);
                break;
            }
        }
        time = (final_tas - initial_tas) / acceleration;
        dist = (time / 3600.0) * (avg_tas - headwind);
        final_alt = initial_altitude + 6076.12 * dist * path_angle.tan();
        vs = if time.abs() < 1e-12 { 0.0 } else { 60.0 * (final_alt - initial_altitude) / time };
        burned = fuel_flow / 3600.0 * time;
        prev = mid_weight;
        mid_weight = env.zero_fuel_weight + (initial_fuel - burned / 2.0);
        lift = mid_weight * path_angle.cos();
        iterations += 1;
        if !(iterations < 4 && (prev - mid_weight).abs() > 100.0) {
            break;
        }
    }
    StepResults {
        path_angle: path_angle.to_degrees(),
        vertical_speed: vs,
        time_elapsed: time,
        distance_traveled: dist,
        fuel_burned: burned,
        initial_altitude,
        final_altitude: final_alt,
        error,
        speed: final_cas,
        ..Default::default()
    }
}

pub fn geometric_step(
    cfg: &AircraftConfig,
    initial_altitude: f64,
    final_altitude: f64,
    distance: f64,
    econ_cas: f64,
    econ_mach: f64,
    initial_fuel: f64,
    headwind: f64,
    env: &StepEnv,
) -> StepResults {
    let fpa = ((final_altitude - initial_altitude) / (distance * 6076.12)).atan();
    let fpa_deg = fpa.to_degrees();
    let mid = (initial_altitude + final_altitude) / 2.0;
    let above = mid > env.tropo_altitude;
    let theta = atm::theta(mid, env.isa_dev, above);
    let delta = atm::delta(mid, above);
    let mut mach = atm::cas_to_mach(econ_cas, delta);
    let (eas, tas, mode) = if mach > econ_mach {
        mach = econ_mach;
        (atm::mach_to_eas(mach, delta), atm::mach_to_tas(mach, theta), AccelFactorMode::ConstantMach)
    } else {
        (atm::cas_to_eas(econ_cas, delta), atm::cas_to_tas(econ_cas, theta, delta), AccelFactorMode::ConstantCas)
    };

    let weight_estimate = env.zero_fuel_weight + initial_fuel;
    let mut lift = weight_estimate * fpa.cos();
    let mut mid_weight = weight_estimate;
    let mut prev;
    let mut iterations = 0;
    let (mut vs, mut time, mut burned);
    loop {
        let cl = fm::lift_coefficient_from_eas(&cfg.flight, lift, eas);
        let cd = fm::drag_coefficient(&cfg.flight, cl, env.speedbrakes_extended, env.gear_extended, env.flaps);
        let af = atm::acceleration_factor(mach, mid, env.isa_dev, above, mode);
        let thrust = fm::thrust_from_constant_path_angle_coefficients(fpa_deg, mid_weight, cl, cd, af);
        vs = 101.268 * (tas - headwind) * fpa.sin();
        time = if vs != 0.0 { 60.0 * (final_altitude - initial_altitude) / vs } else { 0.0 };
        let (_, fuel_flow) = flow_from_thrust(cfg, thrust, mach, mid, theta, delta, env);
        burned = fuel_flow / 3600.0 * time;
        prev = mid_weight;
        mid_weight = env.zero_fuel_weight + (initial_fuel - burned / 2.0);
        lift = mid_weight * fpa.cos();
        iterations += 1;
        if !(iterations < 4 && (prev - mid_weight).abs() > 100.0) {
            break;
        }
    }
    StepResults {
        path_angle: fpa_deg,
        vertical_speed: vs,
        time_elapsed: time,
        distance_traveled: distance,
        fuel_burned: burned,
        final_altitude,
        initial_altitude,
        speed: econ_cas,
        ..Default::default()
    }
}

pub fn vertical_speed_step(
    cfg: &AircraftConfig,
    initial_altitude: f64,
    final_altitude: f64,
    vertical_speed: f64,
    econ_cas: f64,
    econ_mach: f64,
    initial_fuel: f64,
    headwind: f64,
    env: &StepEnv,
) -> StepResults {
    let mid = (initial_altitude + final_altitude) / 2.0;
    let above = mid > env.tropo_altitude;
    let theta = atm::theta(mid, env.isa_dev, above);
    let delta = atm::delta(mid, above);
    let mut mach = atm::cas_to_mach(econ_cas, delta);
    let delta2 = atm::delta2(delta, mach);
    let theta2 = atm::theta2(theta, mach);
    let (tas, mode) = if mach > econ_mach {
        mach = econ_mach;
        (atm::mach_to_tas(mach, theta), AccelFactorMode::ConstantMach)
    } else {
        (atm::cas_to_tas(econ_cas, theta, delta), AccelFactorMode::ConstantCas)
    };

    let path_angle = vertical_speed.atan2(tas * 101.269);
    let time = 60.0 * (final_altitude - initial_altitude) / vertical_speed;
    let dist = (tas - headwind) * path_angle.cos() * time / 3600.0;

    let mut burned;
    let mut iterations = 0;
    let mut mid_weight = env.zero_fuel_weight + initial_fuel;
    let mut prev;
    let mut predicted_n1;
    loop {
        let drag = fm::drag(&cfg.flight, mid_weight, mach, delta, env.speedbrakes_extended, env.gear_extended, env.flaps);
        let thrust = fm::thrust_from_constant_path_angle(path_angle.to_degrees(), mid_weight, drag, mode as i32 as f64);
        let e = &cfg.engine;
        let corrected_thrust = thrust / delta2 / e.number_of_engines;
        predicted_n1 = eng::reverse_table_interpolation(&e.table1506, mach, corrected_thrust / e.max_thrust);
        let corrected_flow = eng::corrected_fuel_flow(e, predicted_n1, mach, mid) * e.number_of_engines;
        let fuel_flow = 0.0f64.max(eng::uncorrected_fuel_flow(corrected_flow, delta2, theta2) * (1.0 + env.perf_factor_percent / 100.0));
        burned = fuel_flow / 3600.0 * time;
        prev = mid_weight;
        mid_weight -= burned / 2.0;
        iterations += 1;
        if !(iterations < 4 && (prev - mid_weight).abs() > 100.0) {
            break;
        }
    }
    StepResults {
        path_angle: path_angle.to_degrees(),
        vertical_speed,
        distance_traveled: dist,
        fuel_burned: burned,
        time_elapsed: time,
        final_altitude,
        predicted_n1,
        speed: econ_cas,
        ..Default::default()
    }
}

pub fn vertical_speed_distance_step(
    cfg: &AircraftConfig,
    initial_altitude: f64,
    distance: f64,
    vertical_speed: f64,
    econ_cas: f64,
    econ_mach: f64,
    initial_fuel: f64,
    headwind: f64,
    env: &StepEnv,
) -> StepResults {
    let mut final_alt = initial_altitude;
    let mut prev_final;
    let mut path_angle;
    let mut time;
    let mut burned;
    let mut iterations = 0;
    let mut mid_weight = env.zero_fuel_weight + initial_fuel;
    let mut predicted_n1;
    loop {
        let mid = (initial_altitude + final_alt) / 2.0;
        let above = mid > env.tropo_altitude;
        let theta = atm::theta(mid, env.isa_dev, above);
        let delta = atm::delta(mid, above);
        let mut mach = atm::cas_to_mach(econ_cas, delta);
        let delta2 = atm::delta2(delta, mach);
        let theta2 = atm::theta2(theta, mach);
        let (tas, mode) = if mach > econ_mach {
            mach = econ_mach;
            (atm::mach_to_tas(mach, theta), AccelFactorMode::ConstantMach)
        } else {
            (atm::cas_to_tas(econ_cas, theta, delta), AccelFactorMode::ConstantCas)
        };
        path_angle = vertical_speed.atan2(tas * 101.269);
        time = if tas - headwind != 0.0 { 3600.0 * distance / (tas - headwind) } else { 0.0 };
        let drag = fm::drag(&cfg.flight, mid_weight, mach, delta, env.speedbrakes_extended, env.gear_extended, env.flaps);
        let thrust = fm::thrust_from_constant_path_angle(path_angle.to_degrees(), mid_weight, drag, mode as i32 as f64);
        let e = &cfg.engine;
        let corrected_thrust = thrust / delta2 / e.number_of_engines;
        predicted_n1 = eng::reverse_table_interpolation(&e.table1506, mach, corrected_thrust / e.max_thrust);
        let corrected_flow = eng::corrected_fuel_flow(e, predicted_n1, mach, mid) * e.number_of_engines;
        let fuel_flow = 0.0f64.max(eng::uncorrected_fuel_flow(corrected_flow, delta2, theta2) * (1.0 + env.perf_factor_percent / 100.0));
        prev_final = final_alt;
        final_alt = initial_altitude + vertical_speed * time / 60.0;
        burned = fuel_flow / 3600.0 * time;
        mid_weight -= burned / 2.0;
        iterations += 1;
        if !(iterations < 4 && (prev_final - final_alt).abs() > 10.0) {
            break;
        }
    }
    StepResults {
        path_angle: path_angle.to_degrees(),
        vertical_speed,
        distance_traveled: distance,
        fuel_burned: burned,
        time_elapsed: time,
        initial_altitude,
        final_altitude: final_alt,
        predicted_n1,
        speed: econ_cas,
        ..Default::default()
    }
}

fn two_speeds(mid: f64, initial_cas: f64, final_cas: f64, econ_mach: f64, env: &StepEnv) -> (f64, f64, f64, f64, f64, f64) {
    let above = mid > env.tropo_altitude;
    let theta = atm::theta(mid, env.isa_dev, above);
    let delta = atm::delta(mid, above);
    let mut im = atm::cas_to_mach(initial_cas, delta);
    let mut fmach = atm::cas_to_mach(final_cas, delta);
    let itas = if im > econ_mach {
        im = econ_mach;
        atm::mach_to_tas(im, theta)
    } else {
        atm::cas_to_tas(initial_cas, theta, delta)
    };
    let ftas = if fmach > econ_mach {
        fmach = econ_mach;
        atm::mach_to_tas(fmach, theta)
    } else {
        atm::cas_to_tas(final_cas, theta, delta)
    };
    (theta, delta, (im + fmach) / 2.0, (itas + ftas) / 2.0, im, fmach)
}

pub fn vertical_speed_step_with_speed_change(
    cfg: &AircraftConfig,
    initial_altitude: f64,
    initial_cas: f64,
    final_cas: f64,
    vertical_speed: f64,
    econ_mach: f64,
    corrected_n1: f64,
    initial_fuel: f64,
    headwind: f64,
    env: &StepEnv,
) -> StepResults {
    let mut final_alt = initial_altitude;
    let mut prev_final;
    let mut mid_weight = env.zero_fuel_weight + initial_fuel;
    let mut iterations = 0;
    let (mut path_angle, mut time, mut dist, mut burned);
    loop {
        let mid = (initial_altitude + final_alt) / 2.0;
        let (theta, delta, mid_mach, mid_tas, _, _) = two_speeds(mid, initial_cas, final_cas, econ_mach, env);
        let (thrust, fuel_flow) = thrust_and_flow(cfg, corrected_n1, mid_mach, mid, theta, delta, env);
        let drag = fm::drag(&cfg.flight, mid_weight, mid_mach, delta, env.speedbrakes_extended, env.gear_extended, env.flaps);
        let gradient = fm::available_gradient(thrust, drag, mid_weight);
        path_angle = vertical_speed.atan2(mid_tas * 101.269);
        let acceleration = fm::acceleration_for_gradient(gradient, path_angle, cfg.flight.gravity_const_kns);
        time = (final_cas - initial_cas) / acceleration;
        dist = (mid_tas - headwind) * (time / 3600.0);
        burned = fuel_flow / 3600.0 * time;
        prev_final = final_alt;
        final_alt = initial_altitude + (time / 60.0) * vertical_speed;
        mid_weight = env.zero_fuel_weight + (initial_fuel - burned / 2.0);
        iterations += 1;
        if !(iterations < 4 && (prev_final - final_alt).abs() > 10.0) {
            break;
        }
    }
    StepResults {
        path_angle: path_angle.to_degrees(),
        vertical_speed,
        time_elapsed: time,
        distance_traveled: dist,
        fuel_burned: burned,
        initial_altitude,
        final_altitude: final_alt,
        speed: final_cas,
        ..Default::default()
    }
}

pub fn altitude_step_with_speed_change(
    cfg: &AircraftConfig,
    initial_altitude: f64,
    initial_cas: f64,
    final_cas: f64,
    econ_mach: f64,
    corrected_n1: f64,
    initial_fuel: f64,
    headwind: f64,
    env: &StepEnv,
) -> StepResults {
    let mut final_alt = initial_altitude;
    let mut prev_final;
    let mut mid_weight = env.zero_fuel_weight + initial_fuel;
    let mut iterations = 0;
    let (mut path_angle, mut vs, mut time, mut dist, mut burned);
    loop {
        let mid = (initial_altitude + final_alt) / 2.0;
        let (theta, delta, mid_mach, mid_tas, _, _) = two_speeds(mid, initial_cas, final_cas, econ_mach, env);
        let (thrust, fuel_flow) = thrust_and_flow(cfg, corrected_n1, mid_mach, mid, theta, delta, env);
        let drag = fm::drag(&cfg.flight, mid_weight, mid_mach, delta, env.speedbrakes_extended, env.gear_extended, env.flaps);
        let gradient = fm::available_gradient(thrust, drag, mid_weight);
        path_angle = fm::speed_change_path_angle(&cfg.flight, thrust, mid_weight, drag);
        let acceleration = fm::acceleration_for_gradient(gradient, path_angle, cfg.flight.gravity_const_kns);
        vs = 101.268 * mid_tas * path_angle.sin();
        time = (final_cas - initial_cas) / acceleration;
        dist = (mid_tas - headwind) * (time / 3600.0);
        burned = fuel_flow / 3600.0 * time;
        prev_final = final_alt;
        final_alt = initial_altitude + (time / 60.0) * vs;
        mid_weight = env.zero_fuel_weight + (initial_fuel - burned / 2.0);
        iterations += 1;
        if !(iterations < 4 && (prev_final - final_alt).abs() > 10.0) {
            break;
        }
    }
    StepResults {
        path_angle: path_angle.to_degrees(),
        vertical_speed: vs,
        time_elapsed: time,
        distance_traveled: dist,
        fuel_burned: burned,
        initial_altitude,
        final_altitude: final_alt,
        speed: final_cas,
        ..Default::default()
    }
}

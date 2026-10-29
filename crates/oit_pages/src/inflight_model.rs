pub const WING_AREA_M2: f64 = 845.0;
pub const CD0: f64 = 0.0135;
pub const K_INDUCED: f64 = 0.045;
pub const MAX_CERT_ALT_FT: f64 = 43100.0;
pub const G: f64 = 9.80665;
const MCL_THRUST_N: f64 = 290000.0;
const MCT_THRUST_N: f64 = 242400.0;
const TSFC_CRUISE: f64 = 1.5e-5;
const TSFC_LOW: f64 = 1.3e-5;
const BUFFET_MARGIN_G: f64 = 1.3;
const MIN_ROC_FPM: f64 = 300.0;
const KT: f64 = 0.514444;

#[derive(Clone, Copy, Debug)]
pub struct Atmosphere {
    pub pressure_pa: f64,
    pub temperature_k: f64,
    pub density: f64,
    pub sigma: f64,
    pub delta: f64,
    pub sound_ms: f64,
}

pub fn atmosphere(pressure_alt_ft: f64, isa_dev_c: f64) -> Atmosphere {
    let h = pressure_alt_ft * 0.3048;
    let (t_isa, p) = if h <= 11000.0 {
        let t = 288.15 - 0.0065 * h;
        (t, 101325.0 * (t / 288.15).powf(5.255876))
    } else {
        (216.65, 22632.06 * ((-G * (h - 11000.0)) / (287.053 * 216.65)).exp())
    };
    let t = t_isa + isa_dev_c;
    let rho = p / (287.053 * t);
    Atmosphere { pressure_pa: p, temperature_k: t, density: rho, sigma: rho / 1.225, delta: p / 101325.0, sound_ms: (1.4 * 287.053 * t).sqrt() }
}

pub fn buffet_cl(mach: f64) -> f64 {
    (0.7 - 1.2 * (mach - 0.85).max(0.0) + 0.3 * (0.8 - mach).max(0.0)).max(0.45)
}

pub fn drag_coefficient(cl: f64, mach: f64) -> f64 {
    let mdd = 0.875 - 0.08 * (cl - 0.45);
    let wave = 20.0 * (mach - mdd).max(0.0).powi(4);
    let rise = 0.12 * (cl - 0.42).max(0.0).powi(2);
    CD0 + K_INDUCED * cl * cl + rise + wave
}

#[derive(Clone, Copy, Debug)]
pub struct CruisePoint {
    pub fl: f64,
    pub mach: f64,
    pub tas_kt: f64,
    pub cl: f64,
    pub drag_n: f64,
    pub fuel_flow_kg_h: f64,
    pub specific_range_nm_per_t: f64,
    pub thrust_per_engine_n: f64,
}

pub fn climb_thrust_per_engine(atm: &Atmosphere, isa_dev_c: f64) -> f64 {
    let temp_lapse = 1.0 - 0.006 * (isa_dev_c - 10.0).max(0.0);
    MCL_THRUST_N * atm.sigma.powf(1.12) * temp_lapse
}

pub fn continuous_thrust_per_engine(atm: &Atmosphere, isa_dev_c: f64) -> f64 {
    let temp_lapse = 1.0 - 0.006 * (isa_dev_c - 10.0).max(0.0);
    MCT_THRUST_N * atm.sigma.powf(1.12) * temp_lapse
}

pub fn cruise(weight_kg: f64, fl: f64, mach: f64, isa_dev_c: f64, fuel_factor: f64, engines: f64) -> CruisePoint {
    let atm = atmosphere(fl * 100.0, isa_dev_c);
    let q = 0.7 * atm.pressure_pa * mach * mach;
    let cl = (weight_kg * G) / (q * WING_AREA_M2);
    let out_drag = if engines < 4.0 { 1.0 + 0.025 * (4.0 - engines) } else { 1.0 };
    let drag = q * WING_AREA_M2 * drag_coefficient(cl, mach) * out_drag;
    let tsfc = TSFC_CRUISE * (1.0 + 0.2 * (mach - 0.85)) * (atm.temperature_k / 216.65).sqrt();
    let ff = tsfc * drag * 3600.0 * fuel_factor;
    let tas_kt = (mach * atm.sound_ms) / KT;
    CruisePoint {
        fl,
        mach,
        tas_kt,
        cl,
        drag_n: drag,
        fuel_flow_kg_h: ff,
        specific_range_nm_per_t: tas_kt / (ff / 1000.0),
        thrust_per_engine_n: drag / engines,
    }
}

pub fn rec_max_fl(weight_kg: f64, mach: f64, isa_dev_c: f64) -> f64 {
    let mut best = 100.0;
    let mut fl = 100.0;
    while fl * 100.0 <= MAX_CERT_ALT_FT {
        let atm = atmosphere(fl * 100.0, isa_dev_c);
        let q = 0.7 * atm.pressure_pa * mach * mach;
        let cl_needed = (BUFFET_MARGIN_G * weight_kg * G) / (q * WING_AREA_M2);
        if cl_needed > buffet_cl(mach) {
            break;
        }
        let point = cruise(weight_kg, fl, mach, isa_dev_c, 1.0, 4.0);
        let tas_ms = point.tas_kt * KT;
        let climb_margin = (weight_kg * G * (MIN_ROC_FPM * 0.3048)) / 60.0 / tas_ms;
        if 4.0 * climb_thrust_per_engine(&atm, isa_dev_c) < point.drag_n + climb_margin {
            break;
        }
        best = fl;
        fl += 10.0;
    }
    best.min((MAX_CERT_ALT_FT / 1000.0).floor() * 10.0)
}

pub fn optimum_fl(weight_kg: f64, mach: f64, isa_dev_c: f64) -> f64 {
    let max = rec_max_fl(weight_kg, mach, isa_dev_c);
    let mut best_fl = 100.0;
    let mut best_sr = 0.0;
    let mut fl = 100.0;
    while fl <= max {
        let sr = cruise(weight_kg, fl, mach, isa_dev_c, 1.0, 4.0).specific_range_nm_per_t;
        if sr > best_sr {
            best_sr = sr;
            best_fl = fl;
        }
        fl += 10.0;
    }
    best_fl
}

pub fn green_dot_cl() -> f64 {
    (CD0 / K_INDUCED).sqrt()
}

pub fn green_dot_kcas(weight_kg: f64, fl: f64) -> f64 {
    let atm = atmosphere(fl * 100.0, 0.0);
    let tas = ((2.0 * weight_kg * G) / (atm.density * WING_AREA_M2 * green_dot_cl())).sqrt();
    (tas * atm.sigma.sqrt()) / KT
}

pub fn green_dot_drag_n(weight_kg: f64) -> f64 {
    weight_kg * G * 2.0 * (CD0 * K_INDUCED).sqrt()
}

pub fn engine_out_ceiling_fl(weight_kg: f64, engines: f64, isa_dev_c: f64) -> f64 {
    let drag = green_dot_drag_n(weight_kg) * (1.0 + 0.025 * (4.0 - engines));
    let mut best = 0.0;
    let mut fl = 0.0;
    while fl * 100.0 <= MAX_CERT_ALT_FT {
        let atm = atmosphere(fl * 100.0, isa_dev_c);
        if engines * continuous_thrust_per_engine(&atm, isa_dev_c) < drag {
            break;
        }
        best = fl;
        fl += 5.0;
    }
    best
}

#[derive(Clone, Copy, Debug)]
pub struct Holding {
    pub kcas: f64,
    pub fuel_flow_kg_h: f64,
}

pub fn holding(weight_kg: f64, fl: f64, isa_dev_c: f64, engines: f64) -> Holding {
    let atm = atmosphere(fl * 100.0, isa_dev_c);
    let kcas = green_dot_kcas(weight_kg, fl);
    let drag = green_dot_drag_n(weight_kg) * (1.0 + 0.025 * (4.0 - engines)) * 1.05;
    let tsfc = TSFC_LOW * (atm.temperature_k / 255.0).sqrt();
    Holding { kcas, fuel_flow_kg_h: tsfc * drag * 3600.0 }
}

#[derive(Clone, Copy, Debug)]
pub struct Descent {
    pub distance_nm: f64,
    pub time_min: f64,
    pub fuel_kg: f64,
}

pub fn descent(from_fl: f64, to_elevation_ft: f64, weight_kg: f64, tailwind_kt: f64) -> Descent {
    let dh = (from_fl * 100.0 - to_elevation_ft).max(0.0);
    let still = (dh / 1000.0) * 3.1 + 10.0;
    let avg_gs = 330.0 + tailwind_kt;
    let distance_nm = still * (avg_gs / 330.0);
    let time_min = (distance_nm / avg_gs) * 60.0;
    let fuel_kg = (dh / 1000.0) * 42.0 * (weight_kg / 450000.0) + 250.0;
    Descent { distance_nm, time_min, fuel_kg }
}

#[derive(Clone, Copy, Debug)]
pub struct PlanResult {
    pub trip_fuel_kg: f64,
    pub time_min: f64,
    pub efob_kg: f64,
}

#[allow(clippy::too_many_arguments)]
pub fn plan(weight_kg: f64, fob_kg: f64, distance_nm: f64, fl: f64, mach: f64, isa_dev_c: f64, wind_kt: f64, fuel_factor: f64) -> PlanResult {
    let mut w = weight_kg;
    let mut fuel = 0.0;
    let mut time = 0.0;
    let mut left = distance_nm;
    let step: f64 = 50.0;
    while left > 0.0 && fuel < fob_kg {
        let d = step.min(left);
        let p = cruise(w, fl, mach, isa_dev_c, fuel_factor, 4.0);
        let gs = (p.tas_kt + wind_kt).max(150.0);
        let hours = d / gs;
        let burn = p.fuel_flow_kg_h * hours;
        fuel += burn;
        w -= burn;
        time += hours * 60.0;
        left -= d;
    }
    PlanResult { trip_fuel_kg: fuel, time_min: time, efob_kg: fob_kg - fuel }
}

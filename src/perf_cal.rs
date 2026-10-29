//! Calibrates the EFB and OIT take-off/landing model to X-Plane's own
//! flight model: each take-off roll and each landing rollout under maximum
//! braking is measured and compared with what the model predicts for the
//! same mass, air and thrust setting. The factors (thrust, ground roll,
//! braking) are medians of the last few samples, kept in
//! `Output/preferences/fbw_a380x_perfcal.json` and served at `/perf/cal`.

use std::sync::Mutex;

use serde_json::{json, Value};
use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::xp::{DataRef, Xplm};
use crate::Vars;

const FILE: &str = "Output/preferences/fbw_a380x_perfcal.json";
const KEEP: usize = 12;
const G: f64 = 9.80665;
const S: f64 = 845.0;
const KT: f64 = 0.514444;
const FLX_TLA: f64 = 34.0;
const TOGA_TLA: f64 = 44.0;
const DRY_MU: f64 = 0.38;
const ROLL_MU: f64 = 0.015;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Samples {
    pub thrust: Vec<f64>,
    pub roll: Vec<f64>,
    pub brake: Vec<f64>,
}

static SAMPLES: Mutex<Option<Samples>> = Mutex::new(None);

fn median(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 1.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2]
    } else {
        (s[n / 2 - 1] + s[n / 2]) / 2.0
    }
}

pub fn factors_json(s: &Samples) -> Value {
    let f = |v: &[f64]| (median(v).clamp(0.7, 1.3) * 1000.0).round() / 1000.0;
    json!({
        "factors": { "thrust": f(&s.thrust), "roll": f(&s.roll), "brake": f(&s.brake), "takeoffs": s.roll.len().max(s.thrust.len()), "landings": s.brake.len() },
        "samples": { "thrust": s.thrust, "roll": s.roll, "brake": s.brake },
    })
}

fn parse(text: &str) -> Samples {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let list = |k: &str| -> Vec<f64> { v["samples"][k].as_array().map(|a| a.iter().filter_map(Value::as_f64).filter(|x| x.is_finite()).collect()).unwrap_or_default() };
    Samples { thrust: list("thrust"), roll: list("roll"), brake: list("brake") }
}

fn with_samples<R>(f: impl FnOnce(&mut Samples) -> R) -> R {
    let mut guard = SAMPLES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let s = guard.get_or_insert_with(|| parse(&std::fs::read_to_string(FILE).unwrap_or_default()));
    f(s)
}

pub fn cal_json() -> String {
    with_samples(|s| factors_json(s).to_string())
}

fn push(list: &mut Vec<f64>, v: f64) {
    if v.is_finite() && (0.4..=2.5).contains(&v) {
        list.push(v);
        if list.len() > KEEP {
            list.remove(0);
        }
    }
}

fn record(thrust: Option<f64>, roll: Option<f64>, brake: Option<f64>) {
    let text = with_samples(|s| {
        if let Some(v) = thrust {
            push(&mut s.thrust, v);
        }
        if let Some(v) = roll {
            push(&mut s.roll, v);
        }
        if let Some(v) = brake {
            push(&mut s.brake, v);
        }
        factors_json(s).to_string()
    });
    let _ = std::fs::write(FILE, text);
    crate::log(&format!("perf cal: thrust {thrust:?} roll {roll:?} brake {brake:?}"));
}

#[derive(Clone, Copy, Debug)]
pub struct Air {
    pub delta: f64,
    pub rho: f64,
    pub isa_c: f64,
}

pub fn air(pressure_pa: f64, oat_c: f64) -> Air {
    let delta = (pressure_pa / 101_325.0).clamp(0.3, 1.2);
    let pa_ft = (1.0 - delta.powf(1.0 / 5.2559)) / 6.8756e-6;
    Air { delta, rho: pressure_pa / (287.053 * (oat_c + 273.15)), isa_c: 15.0 - 1.98 * pa_ft / 1000.0 }
}

pub fn model_thrust_per_engine(rating_kn: f64, a: &Air, temp_eff_c: f64, tas_kt: f64) -> f64 {
    let flat = a.isa_c + 15.0;
    let temp = if temp_eff_c <= flat { 1.0 } else { (1.0 - 0.0095 * (temp_eff_c - flat)).max(0.35) };
    let f0 = rating_kn * 1000.0 * a.delta.powf(0.8) * temp * 0.988;
    f0 * (1.0 - 0.00165 * tas_kt + 1.3e-6 * tas_kt * tas_kt)
}

pub fn model_roll_m(mass_kg: f64, cd: f64, a: &Air, rating_kn: f64, temp_eff_c: f64, thrust_scale: f64, vlof_tas_ms: f64, headwind_ms: f64) -> f64 {
    let (mut v, mut x, mut t) = (0.0_f64, 0.0_f64, 0.0_f64);
    let dt = 0.05;
    while v < vlof_tas_ms && t < 200.0 {
        let q = 0.5 * a.rho * v * v;
        let thrust = 4.0 * model_thrust_per_engine(rating_kn, a, temp_eff_c, v / KT) * thrust_scale;
        let acc = (thrust - q * S * cd - ROLL_MU * (mass_kg * G - q * S * 0.45).max(0.0)) / mass_kg;
        if acc <= 0.01 {
            return f64::INFINITY;
        }
        v += acc * dt;
        x += (v - headwind_ms).max(0.0) * dt;
        t += dt;
    }
    x
}

pub fn model_brake_decel(mass_kg: f64, rho: f64, tas_ms: f64) -> f64 {
    let q = 0.5 * rho * tas_ms * tas_ms;
    DRY_MU * G * (1.0 - q * S * 0.15 / (mass_kg * G)).max(0.3)
}

pub fn flap_cd(handle: f64) -> f64 {
    match handle.round() as i64 {
        ..=1 => 0.074,
        2 => 0.081,
        _ => 0.092,
    }
}

#[derive(Default)]
struct Takeoff {
    armed: bool,
    rolling: bool,
    distance_m: f64,
    measured_n: f64,
    model_n: f64,
    headwind_sum: f64,
    samples: u32,
    temp_eff: f64,
    wet: bool,
}

#[derive(Default)]
struct Landing {
    active: bool,
    measured: f64,
    model: f64,
    samples: u32,
    last_gs: Option<f64>,
    wet: bool,
}

struct Refs {
    on_ground: Option<DataRef>,
    gs: Option<DataRef>,
    tas: Option<DataRef>,
    thrust: Option<DataRef>,
    mass: Option<DataRef>,
    oat: Option<DataRef>,
    pressure: Option<DataRef>,
    rain: Option<DataRef>,
    left_brake: Option<DataRef>,
    right_brake: Option<DataRef>,
}

pub struct PerfCal {
    refs: Refs,
    tla: [VariableIdentifier; 4],
    flex: VariableIdentifier,
    rating: VariableIdentifier,
    flaps: VariableIdentifier,
    takeoff: Takeoff,
    landing: Landing,
    was_on_ground: bool,
}

impl PerfCal {
    pub fn new(vars: &mut Vars, xplm: &Xplm) -> Self {
        let f = |n: &str| xplm.find(n);
        Self {
            refs: Refs {
                on_ground: f("sim/flightmodel/failures/onground_any"),
                gs: f("sim/flightmodel/position/groundspeed"),
                tas: f("sim/flightmodel/position/true_airspeed"),
                thrust: f("sim/flightmodel/engine/POINT_thrust"),
                mass: f("sim/flightmodel/weight/m_total"),
                oat: f("sim/weather/aircraft/temperature_ambient_deg_c"),
                pressure: f("sim/weather/aircraft/barometer_current_pas"),
                rain: f("sim/weather/aircraft/precipitation_on_aircraft_ratio"),
                left_brake: f("sim/cockpit2/controls/left_brake_ratio"),
                right_brake: f("sim/cockpit2/controls/right_brake_ratio"),
            },
            tla: [1, 2, 3, 4].map(|n| vars.get(format!("AUTOTHRUST_TLA:{n}"))),
            flex: vars.get_unprefixed("AIRLINER_TO_FLEX_TEMP".into()),
            rating: vars.get("A380X_ENGINE_RATING_TO_KN".into()),
            flaps: vars.get("FLAPS_HANDLE_INDEX".into()),
            takeoff: Takeoff::default(),
            landing: Landing::default(),
            was_on_ground: true,
        }
    }

    pub fn update(&mut self, vars: &mut Vars, xplm: &Xplm, dt: f64) {
        if dt <= 0.0 || dt > 0.5 {
            return;
        }
        let r = |d: Option<DataRef>| d.map_or(0.0, |d| xplm.get_f(d) as f64);
        let on_ground = self.refs.on_ground.is_some_and(|d| xplm.get_i(d) != 0);
        let gs = r(self.refs.gs);
        let tas = r(self.refs.tas);
        let mut pt = [0f32; 4];
        if let Some(d) = self.refs.thrust {
            xplm.get_vf(d, &mut pt);
        }
        let thrust_n: f64 = pt.iter().map(|&x| x as f64).sum();
        let mass = r(self.refs.mass).max(100_000.0);
        let pressure = self.refs.pressure.map_or(101_325.0, |d| xplm.get_f(d) as f64);
        let oat = r(self.refs.oat);
        let a = air(if pressure > 30_000.0 { pressure } else { 101_325.0 }, oat);
        let wet = r(self.refs.rain) > 0.05;
        let tla_max = self.tla.iter().map(|id| vars.read(id)).fold(f64::MIN, f64::max);
        let rating = vars.read(&self.rating);
        let rating = if rating > 200.0 { rating } else { 356.81 };

        let t = &mut self.takeoff;
        if on_ground && gs < 1.5 {
            *t = Takeoff { armed: true, ..Takeoff::default() };
        } else if t.armed && on_ground && gs >= 1.5 {
            if !t.rolling && tla_max >= FLX_TLA {
                t.rolling = true;
                let flex = vars.read(&self.flex);
                t.temp_eff = if tla_max >= TOGA_TLA || flex <= oat { oat } else { flex };
            }
            if t.rolling {
                if tla_max < 20.0 {
                    *t = Takeoff::default();
                } else {
                    t.distance_m += gs * dt;
                    t.wet |= wet;
                    let tas_kt = tas / KT;
                    if (30.0..=140.0).contains(&tas_kt) {
                        t.measured_n += thrust_n;
                        t.model_n += 4.0 * model_thrust_per_engine(rating, &a, t.temp_eff, tas_kt);
                        t.headwind_sum += tas - gs;
                        t.samples += 1;
                    }
                }
            }
        } else if t.rolling && !on_ground {
            if t.distance_m > 300.0 && t.samples > 40 && t.model_n > 0.0 {
                let thrust_ratio = t.measured_n / t.model_n;
                let hw = t.headwind_sum / t.samples as f64;
                let model = model_roll_m(mass, flap_cd(vars.read(&self.flaps)), &a, rating, t.temp_eff, thrust_ratio, tas, hw);
                let roll = (!t.wet && model.is_finite()).then(|| t.distance_m / model);
                record(Some(thrust_ratio), roll, None);
            }
            *t = Takeoff::default();
        }

        let l = &mut self.landing;
        if on_ground && !self.was_on_ground && gs > 50.0 {
            *l = Landing { active: true, ..Landing::default() };
        }
        if l.active && on_ground {
            let braking = r(self.refs.left_brake).min(r(self.refs.right_brake)) >= 0.9;
            let gs_kt = gs / KT;
            if let Some(last) = l.last_gs {
                let decel = (last - gs) / dt;
                if braking && (60.0..=120.0).contains(&gs_kt) {
                    let q = 0.5 * a.rho * tas * tas;
                    let drag = q * S * 0.16;
                    let brake = decel - drag / mass + thrust_n / mass;
                    l.measured += brake;
                    l.model += model_brake_decel(mass, a.rho, tas);
                    l.samples += 1;
                }
            }
            l.last_gs = Some(gs);
            l.wet |= wet;
            if gs_kt < 40.0 {
                if l.samples >= 20 && !l.wet && l.model > 0.0 {
                    record(None, None, Some(l.measured / l.model));
                }
                *l = Landing::default();
            }
        } else if !on_ground {
            l.active = false;
        }
        self.was_on_ground = on_ground;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factors_are_medians_clamped_and_default_to_one() {
        let s = Samples::default();
        let v = factors_json(&s);
        assert_eq!(v["factors"]["thrust"], 1.0);
        assert_eq!(v["factors"]["landings"], 0);
        let s = Samples { thrust: vec![0.9, 1.1, 1.0], roll: vec![2.0], brake: vec![0.8, 0.9] };
        let v = factors_json(&s);
        assert_eq!(v["factors"]["thrust"], 1.0);
        assert_eq!(v["factors"]["roll"], 1.3);
        assert!((v["factors"]["brake"].as_f64().unwrap() - 0.85).abs() < 1e-9);
        assert_eq!(parse(&v.to_string()), s);
    }

    #[test]
    fn outliers_are_not_kept_and_the_list_is_bounded() {
        let mut l = Vec::new();
        push(&mut l, f64::NAN);
        push(&mut l, 9.0);
        for i in 0..20 {
            push(&mut l, 1.0 + i as f64 * 0.01);
        }
        assert_eq!(l.len(), KEEP);
        assert!((l[0] - 1.08).abs() < 1e-9);
    }

    #[test]
    fn the_model_matches_the_efb_model_at_sea_level() {
        let a = air(101_325.0, 15.0);
        assert!((a.rho - 1.225).abs() < 0.002 && a.isa_c.abs() < 15.1);
        let static_n = model_thrust_per_engine(356.81, &a, 15.0, 0.0);
        assert!((static_n - 356_810.0 * 0.988).abs() < 1.0);
        let hot = model_thrust_per_engine(356.81, &a, 60.0, 0.0);
        assert!(hot < static_n * 0.75);
        let roll = model_roll_m(500_000.0, 0.081, &a, 356.81, 15.0, 1.0, 165.0 * KT, 0.0);
        assert!((1800.0..3200.0).contains(&roll), "TOGA roll at 500 t {roll:.0} m");
        assert!(model_brake_decel(400_000.0, 1.225, 0.0) > 3.6);
    }
}

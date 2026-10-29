use crate::js::{el, num, round};
use crate::jv::{fx, n};
use crate::{button, label, span, Node};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

const G: f64 = 9.80665;
const S: f64 = 845.0;
const KT: f64 = 0.514444;
const FT: f64 = 0.3048;

fn rating(k: &str) -> Option<f64> {
    Some(match k {
        "GP7270" | "GP7270E" => 332.44,
        "GP7277" => 357.15,
        "970" => 334.29,
        "972" | "972E" => 341.41,
        "970B" => 348.31,
        "972B" => 356.81,
        "977" => 359.33,
        "977B" => 372.92,
        "980" => 374.09,
        _ => return None,
    })
}

fn wv(k: &str) -> Option<(f64, f64)> {
    Some(match k {
        "WV000" => (560.0, 386.0),
        "WV001" => (510.0, 394.0),
        "WV002" => (569.0, 391.0),
        "WV003" => (510.0, 395.0),
        "WV004" => (560.0, 391.0),
        "WV005" => (560.0, 386.0),
        "WV006" => (573.0, 393.0),
        "WV007" => (490.0, 395.0),
        "WV008" => (575.0, 394.0),
        "WV009" => (510.0, 386.0),
        "WV010" => (480.0, 386.0),
        "WV011" => (575.0, 395.0),
        "WV012" => (569.0, 395.0),
        "WV013" => (492.0, 386.0),
        "WV014" => (572.0, 391.0),
        _ => return None,
    })
}

fn vls_table(conf: &str) -> [[f64; 8]; 2] {
    match conf {
        "1+F" => [[123.0, 133.0, 144.0, 154.0, 163.0, 172.0, 180.0, 188.0], [123.0, 130.0, 140.0, 150.0, 159.0, 167.0, 176.0, 183.0]],
        "2" => [[123.0, 128.0, 138.0, 148.0, 157.0, 165.0, 173.0, 181.0], [123.0, 125.0, 135.0, 144.0, 153.0, 161.0, 169.0, 176.0]],
        "3" => [[123.0, 124.0, 134.0, 144.0, 152.0, 161.0, 169.0, 176.0], [123.0, 123.0, 131.0, 140.0, 148.0, 156.0, 164.0, 171.0]],
        _ => [[123.0, 123.0, 131.0, 140.0, 148.0, 157.0, 165.0, 173.0], [123.0, 123.0, 127.0, 136.0, 144.0, 152.0, 160.0, 168.0]],
    }
}

fn ld(conf: &str) -> (f64, f64) {
    match conf {
        "1+F" => (0.074, 13.2),
        "2" => (0.081, 12.1),
        _ => (0.092, 10.6),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Cond {
    mu: f64,
    roll: f64,
    wet: bool,
    rev: bool,
}

const TO_CONDS: [(&str, f64, f64, bool, bool); 11] = [
    ("DRY", 0.42, 0.015, false, false), ("WET", 0.25, 0.02, true, true), ("WATER 6 MM", 0.15, 0.045, true, true), ("WATER 13 MM", 0.13, 0.07, true, true), ("SLUSH 6 MM", 0.14, 0.05, true, true),
    ("SLUSH 13 MM", 0.12, 0.08, true, true), ("COMPACTED SNOW", 0.2, 0.02, true, true), ("WET SNOW 5 MM", 0.17, 0.035, true, true), ("WET SNOW 15 MM", 0.16, 0.05, true, true),
    ("DRY SNOW 10 MM", 0.2, 0.035, true, true), ("DRY SNOW 100 MM", 0.17, 0.09, true, true),
];
const LDG_CONDS: [(&str, f64); 6] = [("6 DRY", 0.38), ("5 GOOD", 0.28), ("4 GOOD/MEDIUM", 0.22), ("3 MEDIUM", 0.17), ("2 MEDIUM/POOR", 0.12), ("1 POOR", 0.08)];

fn to_cond(k: &str) -> Cond {
    let c = TO_CONDS.iter().find(|c| c.0 == k).unwrap_or(&TO_CONDS[0]);
    Cond { mu: c.1, roll: c.2, wet: c.3, rev: c.4 }
}

fn autobrake(k: &str) -> f64 {
    match k {
        "LO" => 1.7,
        "2" => 2.2,
        "3" => 3.0,
        "HI" => 3.5,
        _ => 99.0,
    }
}

fn lineup(k: &str) -> [f64; 2] {
    match k {
        "0" => [0.0, 0.0],
        "180" => [35.0, 56.0],
        _ => [24.0, 45.0],
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default)]
pub struct Cal {
    pub thrust: f64,
    pub roll: f64,
    pub brake: f64,
    pub takeoffs: f64,
    pub landings: f64,
}

impl Default for Cal {
    fn default() -> Self {
        Cal { thrust: 1.0, roll: 1.0, brake: 1.0, takeoffs: 0.0, landings: 0.0 }
    }
}

pub fn cal_text(c: &Cal) -> String {
    if c.takeoffs + c.landings == 0.0 {
        "X-Plane calibration: none yet. Each take-off and maximum-braking landing you fly tunes these numbers to the X-Plane flight model.".into()
    } else {
        format!(
            "X-Plane calibration from {} take-off(s) and {} landing(s): thrust x{}, ground roll x{}, braking x{}.",
            num(c.takeoffs),
            num(c.landings),
            fx(c.thrust, 2),
            fx(c.roll, 2),
            fx(c.brake, 2)
        )
    }
}

pub fn pc_num(v: &str, d: f64) -> f64 {
    let s = v.replacen(',', ".", 1);
    let t = s.trim_start();
    let mut end = 0;
    let b = t.as_bytes();
    let mut seen_dot = false;
    let mut seen_e = false;
    while end < b.len() {
        let c = b[end];
        let ok = c.is_ascii_digit()
            || ((c == b'-' || c == b'+') && (end == 0 || matches!(b[end - 1], b'e' | b'E')))
            || (c == b'.' && !seen_dot && !seen_e)
            || ((c == b'e' || c == b'E') && !seen_e && end > 0);
        if !ok {
            break;
        }
        seen_dot |= c == b'.';
        seen_e |= c == b'e' || c == b'E';
        end += 1;
    }
    let mut cut = &t[..end];
    while !cut.is_empty() {
        if let Ok(x) = cut.parse::<f64>() {
            return if x.is_finite() { x } else { d };
        }
        cut = &cut[..cut.len() - 1];
    }
    if t.starts_with("Infinity") || t.starts_with("+Infinity") || t.starts_with("-Infinity") {
        return d;
    }
    d
}

#[derive(Clone, Copy, Debug)]
struct Atm {
    pa: f64,
    delta: f64,
    isa: f64,
    sigma: f64,
    rho: f64,
}

fn atmos(elev: f64, qnh: f64, oat: f64) -> Atm {
    let pa = elev + (1013.25 - qnh) * 27.3;
    let delta = (1.0 - 6.8756e-6 * pa).max(0.2).powf(5.2559);
    let isa = 15.0 - 1.98 * pa / 1000.0;
    let sigma = delta * 288.15 / (oat + 273.15);
    Atm { pa, delta, isa, sigma, rho: 1.225 * sigma }
}

fn vls(conf: &str, tons: f64, cg: f64) -> f64 {
    let t = vls_table(conf);
    let at = |row: &[f64; 8]| {
        let x = tons.min(600.0).max(350.0);
        let i = (((x - 250.0) / 50.0).floor() as usize).min(6);
        let v = row[i] + (row[i + 1] - row[i]) * ((x - 250.0) / 50.0 - i as f64);
        if tons >= 350.0 { v } else { v * (tons / 350.0).sqrt() }
    };
    let k = ((cg - 29.0) / 14.0).min(1.0).max(0.0);
    at(&t[0]) * (1.0 - k) + at(&t[1]) * k
}

fn vmcg(pa: f64) -> f64 {
    118.0 - 0.6 * pa / 1000.0
}

fn vmca(pa: f64) -> f64 {
    123.0 - 0.8 * pa / 1000.0
}

#[derive(Clone, Debug)]
pub struct ToParams {
    atm: Atm,
    pub hw: f64,
    pub xw: f64,
    pub rw: Option<Value>,
    rating: f64,
    pub mtow: f64,
    oat: f64,
    pub tow: f64,
    cg: f64,
    conf: String,
    thrust: String,
    pub derate_pct: f64,
    packs: bool,
    ai: String,
    slope: f64,
    c: Cond,
    pub tora: f64,
    pub toda: f64,
    pub asda: f64,
    pub lineup: [f64; 2],
    dry_check: bool,
    cal: Cal,
}

fn engine_thrust(p: &ToParams, temp_eff: f64) -> f64 {
    let flat = p.atm.isa + 15.0;
    let temp = if temp_eff <= flat { 1.0 } else { (1.0 - 0.0095 * (temp_eff - flat)).max(0.35) };
    let bleed = (if p.packs { 0.988 } else { 1.0 }) * if p.ai == "ENG+WING" { 0.975 } else if p.ai == "ENG" { 0.99 } else { 1.0 };
    p.rating * 1000.0 * p.atm.delta.powf(0.8) * temp * bleed * p.cal.thrust
}

fn thrust_at(f0: f64, tas_kt: f64) -> f64 {
    f0 * (1.0 - 0.00165 * tas_kt + 1.3e-6 * tas_kt * tas_kt)
}

#[allow(clippy::too_many_arguments)]
fn ground(m: f64, cd: f64, mu: f64, slope: f64, atm: &Atm, f0: f64, engines: f64, v0: f64, v1: f64, hw: f64, cal: &Cal) -> (f64, f64) {
    let dt = 0.05;
    let (mut v, mut x, mut t) = (v0, 0.0, 0.0);
    let sin = (slope / 100.0).atan().sin();
    while v < v1 && t < 200.0 {
        let q = 0.5 * atm.rho * v * v;
        let lift = q * S * 0.45;
        let a = (engines * thrust_at(f0, v / KT) - q * S * cd - mu * (m * G - lift).max(0.0) - m * G * sin) / m;
        if a <= 0.01 {
            return (f64::INFINITY, f64::INFINITY);
        }
        v += a * dt;
        x += (v - hw) * dt;
        t += dt;
    }
    (x * cal.roll, t)
}

#[allow(clippy::too_many_arguments)]
fn air(m: f64, conf: &str, f0: f64, engines: f64, vlof: f64, v2: f64, hw: f64, screen_ft: f64) -> f64 {
    let (_, l) = ld(conf);
    let h = screen_ft * FT;
    let v = (vlof + v2) / 2.0;
    let tt = engines * thrust_at(f0, v / KT) - if engines < 4.0 { 0.012 * m * G } else { 0.0 };
    let gamma = tt / (m * G) - 1.0 / (l - 3.2);
    if gamma <= 0.002 {
        return f64::INFINITY;
    }
    (h / (0.5 * gamma)) * (v - hw) / v
}

#[derive(Clone, Debug)]
pub struct ToCase {
    pub ok: bool,
    pub limits: Vec<String>,
    pub v1: f64,
    pub v1x: f64,
    pub vr: f64,
    pub v2: f64,
    pub conf: String,
    pub tod_aeo: f64,
    pub tod_oei: f64,
    pub asd: f64,
    pub toda: f64,
    pub asda: f64,
    pub grad2: f64,
    pub vlof_gs: f64,
    pub f0: f64,
    pub flex: Option<f64>,
    pub trim: f64,
    pub limit_tow: f64,
}

fn takeoff_case(p: &ToParams, temp_eff: f64, conf: &str) -> ToCase {
    let atm = &p.atm;
    let m = p.tow * 1000.0;
    let f0 = engine_thrust(p, temp_eff) * (1.0 - p.derate_pct / 100.0);
    let (cd, l) = ld(conf);
    let vs_cas = vls(conf, p.tow, p.cg) / 1.23;
    let to_tas = |cas: f64| cas / atm.sigma.sqrt();
    let vmc_ratio = (1.0 - p.derate_pct / 100.0).sqrt();
    let (vmcg_v, vmca_v) = (vmcg(atm.pa) * vmc_ratio, vmca(atm.pa) * vmc_ratio);
    let v2 = (1.13 * vs_cas).max(1.1 * vmca_v).ceil();
    let vr = (1.05 * vmca_v).max(v2 - 6.0).ceil();
    let hw = if p.hw >= 0.0 { p.hw * 0.5 } else { p.hw * 1.5 };
    let hw_m = hw * KT;
    let vr_t = to_tas(vr) * KT;
    let v2_t = to_tas(v2) * KT;
    let vlof_t = to_tas(vr + 4.0) * KT;
    let gnd = ground(m, cd, p.c.roll, p.slope, atm, f0, 4.0, 0.0, vr_t, hw_m, &p.cal);
    let rot = ground(m, cd * 1.15, p.c.roll * 0.4, p.slope, atm, f0, 4.0, vr_t, vlof_t, hw_m, &p.cal);
    let tod_aeo = 1.15 * (gnd.0 + rot.0 + air(m, conf, f0, 4.0, vlof_t, v2_t * 1.04, hw_m, 35.0));
    let grad2 = (3.0 * thrust_at(f0, to_tas(v2)) - 0.012 * m * G) / (m * G) - 1.0 / l;
    let vlof_gs = to_tas(vr + 4.0) - hw;
    let case_for = |v1: f64| {
        let v1_t = to_tas(v1) * KT;
        let vef_t = (to_tas(v1 - 3.0) * KT).max(1.0);
        let a1 = ground(m, cd, p.c.roll, p.slope, atm, f0, 4.0, 0.0, vef_t, hw_m, &p.cal);
        let a2 = ground(m, cd, p.c.roll, p.slope, atm, f0, 3.0, vef_t, vr_t, hw_m, &p.cal);
        let a3 = ground(m, cd * 1.15, p.c.roll * 0.4, p.slope, atm, f0, 3.0, vr_t, to_tas(vr + 3.0) * KT, hw_m, &p.cal);
        let tod_oei = a1.0 + a2.0 + a3.0 + air(m, conf, f0, 3.0, to_tas(vr + 3.0) * KT, v2_t, hw_m, if p.c.wet { 15.0 } else { 35.0 });
        let acc = ground(m, cd, p.c.roll, p.slope, atm, f0, 4.0, 0.0, v1_t, hw_m, &p.cal);
        let mut v = v1_t;
        let mut x = acc.0 + 2.0 * (v1_t - hw_m);
        let sin = (p.slope / 100.0).atan().sin();
        let mut t = 0.0;
        while v > 0.5 && t < 120.0 {
            let q = 0.5 * atm.rho * v * v;
            let rev = if p.c.rev { 2.0 * 0.18 * thrust_at(f0, v / KT) * (v / (60.0 * KT)).min(1.0) } else { 0.0 };
            let dec = p.c.mu * p.cal.brake * G * (1.0 - (q * S * 0.2) / (m * G)).max(0.2) + (q * S * (cd + 0.06) + rev - 4.0 * 0.06 * f0) / m + G * sin;
            v -= dec.max(0.3) * 0.05;
            x += (v - hw_m).max(0.0) * 0.05;
            t += 0.05;
        }
        (v1, tod_oei, x, acc.0, 0.5 * m * v1_t * v1_t)
    };
    let toda = p.toda - p.lineup[0];
    let asda = p.asda - p.lineup[1];
    let mut best: Option<((f64, f64, f64, f64, f64), f64)> = None;
    let lo = vmcg_v.ceil();
    let mut v1 = lo;
    while v1 <= vr {
        let c = case_for(v1);
        let margin = (toda - c.1).min(asda - c.2);
        if best.as_ref().map_or(true, |b| margin > b.1) {
            best = Some((c, margin));
        }
        v1 += 1.0;
    }
    let (b, _) = best.unwrap_or(((vr, f64::INFINITY, f64::INFINITY, 0.0, 0.0), 0.0));
    let mut limits: Vec<String> = Vec::new();
    if tod_aeo > toda {
        limits.push("RUNWAY (TOD AEO)".into());
    }
    if b.1 > toda {
        limits.push("RUNWAY (TOD OEI)".into());
    }
    if b.2 > asda {
        limits.push("RUNWAY (ASD)".into());
    }
    if grad2 < 0.03 {
        limits.push("SECOND SEGMENT".into());
    }
    if vlof_gs > 204.0 {
        limits.push("TYRE SPEED".into());
    }
    if b.4 > 16.0 * 130e6 {
        limits.push("BRAKE ENERGY".into());
    }
    if vr < vmcg_v {
        limits.push("VMCG".into());
    }
    if p.c.wet && !p.dry_check {
        let mut q = p.clone();
        q.c = to_cond("DRY");
        q.dry_check = true;
        let dry = takeoff_case(&q, temp_eff, conf);
        for l in dry.limits {
            if !limits.contains(&l) {
                limits.push(format!("{l} (DRY)"));
            }
        }
    }
    ToCase {
        ok: limits.is_empty(),
        limits,
        v1: b.0,
        v1x: b.3,
        vr,
        v2,
        conf: conf.into(),
        tod_aeo,
        tod_oei: b.1,
        asd: b.2,
        toda,
        asda,
        grad2,
        vlof_gs,
        f0,
        flex: None,
        trim: 0.0,
        limit_tow: 0.0,
    }
}

fn trim(cg: f64) -> f64 {
    if cg < 29.0 {
        5.8
    } else if cg <= 34.5 {
        5.8 - ((cg - 29.0) * 56.0) / 275.0
    } else if cg <= 43.0 {
        4.68 - ((cg - 34.5) * 243.0) / 425.0
    } else {
        -0.18
    }
}

fn limit_weight(p: &ToParams, conf: &str) -> f64 {
    let (mut lo, mut hi) = (250.0, 620.0);
    for _ in 0..18 {
        let mid = (lo + hi) / 2.0;
        let mut q = p.clone();
        q.tow = mid;
        if takeoff_case(&q, p.oat, conf).ok {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

pub fn takeoff(p: &ToParams) -> ToCase {
    let confs: Vec<&str> = if p.conf == "AUTO" { vec!["1+F", "2", "3"] } else { vec![p.conf.as_str()] };
    let oat = p.oat;
    let flat = p.atm.isa + 15.0;
    let tmax = p.atm.isa + 70.0;
    let mut best: Option<ToCase> = None;
    let score = |x: &ToCase| if x.ok { 1000.0 + x.flex.unwrap_or(0.0) * 10.0 + if x.conf == "2" { 0.5 } else { 0.0 } } else { -1.0 };
    for conf in confs {
        let toga = takeoff_case(p, oat, conf);
        if !toga.ok {
            if best.as_ref().map_or(true, |b| !b.ok && toga.tod_aeo < b.tod_aeo) {
                best = Some(toga);
            }
            continue;
        }
        let mut flex = None;
        if p.thrust == "FLEX" {
            let min_flex = (oat + 1.0).max(flat.ceil());
            let foga = engine_thrust(p, oat);
            let (mut lo, mut hi) = (min_flex, tmax.floor());
            let usable = |t: f64| engine_thrust(p, t) >= 0.75 * foga && takeoff_case(p, t, conf).ok;
            if lo <= hi && usable(lo) {
                while lo < hi {
                    let mid = ((lo + hi + 1.0) / 2.0).ceil();
                    if usable(mid) {
                        lo = mid;
                    } else {
                        hi = mid - 1.0;
                    }
                }
                flex = Some(lo);
            }
        }
        let r = match flex {
            Some(fl) if fl != 0.0 => {
                let mut c = takeoff_case(p, fl, conf);
                c.flex = Some(fl);
                c
            }
            _ => toga,
        };
        if best.as_ref().map_or(true, |b| score(&r) > score(b)) {
            best = Some(r);
        }
    }
    let mut b = best.unwrap();
    b.trim = trim(p.cg);
    b.limit_tow = limit_weight(p, &b.conf.clone());
    b
}

#[derive(Clone, Debug)]
pub struct LdgParams {
    atm: Atm,
    pub hw: f64,
    pub xw: f64,
    pub rw: Option<Value>,
    pub mlw: f64,
    pub lw: f64,
    cg: f64,
    pub conf: String,
    ab: String,
    rev: bool,
    ice: bool,
    slope: f64,
    mu: f64,
    pub lda: f64,
    cal: Cal,
}

pub struct LdgRow {
    pub ab: String,
    pub actual: f64,
    pub inflight: f64,
    pub dispatch: f64,
}

pub struct LdgOut {
    pub vls: f64,
    pub vapp: f64,
    pub rows: Vec<LdgRow>,
    pub lda: f64,
    pub mlw_over: bool,
}

pub fn landing(p: &LdgParams) -> LdgOut {
    let atm = &p.atm;
    let m = p.lw * 1000.0;
    let vls_v = vls(&p.conf, p.lw, p.cg).max(123.0) + if p.ice { 5.0 } else { 0.0 };
    let wind_add = (p.hw / 3.0).max(5.0).min(15.0);
    let vapp = round(vls_v + wind_add);
    let hw_f = if p.hw >= 0.0 { p.hw * 0.5 } else { p.hw * 1.5 };
    let tas = |cas: f64| cas / atm.sigma.sqrt();
    let gs_td = (tas(vapp - 5.0) - hw_f) * KT;
    let air_d = 430.0 * ((tas(vapp) - hw_f) / 140.0);
    let mu = p.mu * p.cal.brake;
    let sin = (p.slope / 100.0).atan().sin();
    let run = |target: f64| {
        let mut v = tas(vapp - 5.0) * KT;
        let mut x = air_d + 2.5 * gs_td;
        let mut t = 0.0;
        while v > 0.5 && t < 200.0 {
            let q = 0.5 * atm.rho * v * v;
            let aero = (q * S * 0.16) / m;
            let rev = if p.rev { (2.0 * 0.2 * 350e3 * atm.delta * ((v / KT - 70.0) / 70.0).max(0.0).min(1.0)) / m } else { 0.0 };
            let brake_max = mu * G * (1.0 - (q * S * 0.15) / (m * G)).max(0.3);
            let dec = target.min(brake_max + aero + rev) + G * sin;
            v -= dec.max(0.3) * 0.05;
            x += (v - hw_f * KT).max(0.0) * 0.05;
            t += 0.05;
        }
        x
    };
    let modes: Vec<&str> = if p.ab == "ALL" { vec!["LO", "2", "3", "HI", "MAX"] } else { vec![p.ab.as_str()] };
    let rows = modes
        .iter()
        .map(|ab| {
            let d = run(autobrake(ab));
            LdgRow { ab: ab.to_string(), actual: d, inflight: d * 1.15, dispatch: d / 0.6 * if mu < 0.3 { 1.15 } else { 1.0 } }
        })
        .collect();
    LdgOut { vls: round(vls_v), vapp, rows, lda: p.lda, mlw_over: p.lw > p.mlw }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct PerfView {
    pub tab: String,
    pub cal: Cal,
    pub to: BTreeMap<String, String>,
    pub ldg: BTreeMap<String, String>,
    pub airport_to: Option<Value>,
    pub airport_ldg: Option<Value>,
    /// CALCULATE pressed on the takeoff / landing page.
    pub calc_to: bool,
    pub calc_ldg: bool,
    pub settings: BTreeMap<String, String>,
}

/// `pcIn` as the page starts.
pub fn initial(kind: &str) -> BTreeMap<String, String> {
    let pairs: &[(&str, &str)] = if kind == "to" {
        &[
            ("icao", ""), ("rwy", ""), ("tora", ""), ("toda", ""), ("asda", ""), ("slope", "0"), ("lineup", "90"), ("wdir", ""), ("wspd", "0"), ("oat", "15"), ("qnh", "1013"), ("cond", "DRY"),
            ("tow", ""), ("cg", "33"), ("conf", "AUTO"), ("thrust", "FLEX"), ("drt", "D01"), ("packs", "ON"), ("ai", "OFF"),
        ]
    } else {
        &[
            ("icao", ""), ("rwy", ""), ("lda", ""), ("slope", "0"), ("wdir", ""), ("wspd", "0"), ("oat", "15"), ("qnh", "1013"), ("cond", "6 DRY"), ("lw", ""), ("cg", "33"), ("conf", "FULL"),
            ("ab", "ALL"), ("rev", "ON"), ("autoland", "NO"), ("ice", "NO"),
        ]
    };
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// `pcOperatorDefaults`: operator settings replace inputs the user has not changed.
pub fn operator_defaults(to: &mut BTreeMap<String, String>, ldg: &mut BTreeMap<String, String>, applied: &mut BTreeMap<String, String>, settings: &BTreeMap<String, String>, defaults: &BTreeMap<String, String>) {
    for (kind, field, key) in [("to", "conf", "xphfbw.opToConf"), ("to", "thrust", "xphfbw.opToThrust"), ("to", "packs", "xphfbw.opToPacks"), ("ldg", "conf", "xphfbw.opLdgConf"), ("ldg", "ab", "xphfbw.opLdgAutobrake"), ("ldg", "rev", "xphfbw.opLdgReversers")] {
        let want = settings.get(key).filter(|v| !v.is_empty()).or(defaults.get(key)).cloned();
        let Some(want) = want else { continue };
        let m = if kind == "to" { &mut *to } else { &mut *ldg };
        let ak = format!("{kind}{field}");
        let prev = applied.get(&ak).cloned().unwrap_or_else(|| initial(kind)[field].clone());
        if m.get(field) == Some(&prev) && prev != want {
            m.insert(field.into(), want.clone());
        }
        if m.get(field) == Some(&want) {
            applied.insert(ak, want);
        }
    }
}

fn runway<'a>(ap: Option<&'a Value>, id: &str) -> Option<&'a Value> {
    ap?.get("runways")?.as_array()?.iter().find(|r| crate::jv::sv(r, "id", "") == id)
}

fn common(i: &BTreeMap<String, String>, ap: Option<&Value>) -> (Atm, f64, f64, Option<Value>) {
    let g = |k: &str| i.get(k).cloned().unwrap_or_default();
    let rw = runway(ap, &g("rwy")).cloned();
    let elev = ap.map(|a| crate::jv::nf(a, "elevation_ft")).unwrap_or(0.0);
    let atm = atmos(elev, pc_num(&g("qnh"), 1013.0), pc_num(&g("oat"), 15.0));
    let hdg = rw.as_ref().map(|r| crate::jv::nf(r, "heading")).unwrap_or(0.0);
    let wdir = if g("wdir").is_empty() { hdg } else { pc_num(&g("wdir"), 0.0) };
    let ang = ((wdir - hdg) * std::f64::consts::PI) / 180.0;
    let hw = pc_num(&g("wspd"), 0.0) * ang.cos();
    let xw = pc_num(&g("wspd"), 0.0) * ang.sin();
    (atm, hw, xw, rw)
}

pub fn to_params(v: &PerfView) -> ToParams {
    let i = &v.to;
    let g = |k: &str| i.get(k).cloned().unwrap_or_default();
    let (atm, hw, xw, rw) = common(i, v.airport_to.as_ref().filter(|a| a.get("error").is_none()));
    let s = &v.settings;
    let derates: Vec<f64> = s.get("xphfbw.takeoffDerates").filter(|x| !x.is_empty()).cloned().unwrap_or("4,8,12,16,20".into()).split(',').map(|x| pc_num(x, 0.0)).collect();
    let w = s.get("xphfbw.weightVariant").filter(|x| !x.is_empty()).and_then(|k| wv(k)).unwrap_or(wv("WV003").unwrap());
    let r = s.get("xphfbw.engineRating").filter(|x| !x.is_empty()).and_then(|k| rating(k)).unwrap_or(rating("972B").unwrap());
    let rwn = |k: &str| rw.as_ref().map(|r| crate::jv::nf(r, k)).unwrap_or(0.0);
    let drt = g("drt");
    ToParams {
        atm,
        hw,
        xw,
        rating: r,
        mtow: w.0,
        oat: pc_num(&g("oat"), 15.0),
        tow: pc_num(&g("tow"), 0.0),
        cg: pc_num(&g("cg"), 33.0),
        conf: g("conf"),
        thrust: g("thrust"),
        derate_pct: if g("thrust") == "DERATE" { drt.get(1..).and_then(|x| x.parse::<usize>().ok()).and_then(|k| derates.get(k.wrapping_sub(1)).copied()).filter(|x| *x != 0.0).unwrap_or(0.0) } else { 0.0 },
        packs: g("packs") == "ON",
        ai: g("ai"),
        slope: pc_num(&g("slope"), 0.0),
        c: to_cond(&g("cond")),
        tora: pc_num(&g("tora"), rwn("tora_m")),
        toda: pc_num(&g("toda"), rwn("toda_m")),
        asda: pc_num(&g("asda"), rwn("asda_m")),
        lineup: lineup(&g("lineup")),
        rw,
        dry_check: false,
        cal: v.cal,
    }
}

pub fn ldg_params(v: &PerfView) -> LdgParams {
    let i = &v.ldg;
    let g = |k: &str| i.get(k).cloned().unwrap_or_default();
    let (atm, hw, xw, rw) = common(i, v.airport_ldg.as_ref().filter(|a| a.get("error").is_none()));
    let w = v.settings.get("xphfbw.weightVariant").filter(|x| !x.is_empty()).and_then(|k| wv(k)).unwrap_or(wv("WV003").unwrap());
    let rwn = |k: &str| rw.as_ref().map(|r| crate::jv::nf(r, k)).unwrap_or(0.0);
    LdgParams {
        atm,
        hw,
        xw,
        mlw: w.1,
        lw: pc_num(&g("lw"), 0.0),
        cg: pc_num(&g("cg"), 33.0),
        conf: g("conf"),
        ab: g("ab"),
        rev: g("rev") == "ON",
        ice: g("ice") == "YES",
        slope: pc_num(&g("slope"), 0.0),
        mu: LDG_CONDS.iter().find(|c| c.0 == g("cond")).map(|c| c.1).unwrap_or(0.42),
        lda: pc_num(&g("lda"), rwn("lda_m")),
        rw,
        cal: v.cal,
    }
}

fn field(v: &PerfView, kind: &str, key: &str, lbl: &str, opts: Opts, unit: &str) -> Node {
    let i = if kind == "to" { &v.to } else { &v.ldg };
    let cur = i.get(key).cloned().unwrap_or_default();
    let input = match opts {
        Opts::Seg(list) => el("div", "pc-seg").children(list.iter().map(|o| el("button", "pc-opt").class_if("on", cur == *o).attr("type", "button").act(&format!("pc:{kind}:{key}={o}")).text(o))),
        Opts::Select(list) => {
            let pairs: Vec<(&str, &str)> = list.iter().map(|o| (o.as_str(), o.as_str())).collect();
            let mut s = crate::js::select(&pairs, &format!("pc:{kind}:{key}"), &cur);
            s.cls.push("pc-in".into());
            s
        }
        Opts::Text(ph) => el("input", "pc-in").attr("type", "text").value(&cur).attr("placeholder", &ph).act(&format!("focus:pc:{kind}:{key}")),
    };
    let mut row = el("span", "pc-row").child(input);
    if !unit.is_empty() {
        row = row.child(span("pc-u", unit));
    }
    el("label", "pc-f").child(span("pc-l", lbl)).child(row)
}

enum Opts {
    Seg(Vec<String>),
    Select(Vec<String>),
    Text(String),
}

fn seg(l: &[&str]) -> Opts {
    Opts::Seg(l.iter().map(|x| x.to_string()).collect())
}

fn text(ph: &str) -> Opts {
    Opts::Text(ph.into())
}

fn runway_field(v: &PerfView, kind: &str) -> Node {
    let ap = if kind == "to" { v.airport_to.as_ref() } else { v.airport_ldg.as_ref() };
    let rws = ap.filter(|a| a.get("error").is_none()).and_then(|a| a.get("runways")).and_then(Value::as_array);
    match rws {
        Some(r) => field(v, kind, "rwy", "Runway", Opts::Select(r.iter().map(|x| crate::jv::sv(x, "id", "undefined")).collect()), ""),
        None => {
            let note = ap.and_then(|a| a.get("error")).filter(|e| crate::jv::truthy(e)).map(crate::jv::jstring).unwrap_or("Enter an airport".into());
            el("label", "pc-f").child(span("pc-l", "Runway")).child(span("pc-note", &note))
        }
    }
}

fn strip(len: f64, marks: &[(f64, &str, &str, bool)]) -> Node {
    let (w, x0) = (760.0, 10.0);
    let ww = w - 20.0;
    let sx = |m: f64| x0 + (m / len).min(1.0).max(0.0) * ww;
    let mut p = vec![format!("<svg viewBox=\"0 0 {} 86\" class=\"pc-strip\"><rect x=\"{}\" y=\"30\" width=\"{}\" height=\"26\" rx=\"3\" fill=\"#2b2e33\" stroke=\"#8a8f96\" stroke-width=\"1.4\"/>", n(w), n(x0), n(ww))];
    for k in 0..14 {
        p.push(format!("<rect x=\"{}\" y=\"42\" width=\"{}\" height=\"2\" fill=\"#8a8f96\"/>", n(x0 + 30.0 + k as f64 * (ww - 60.0) / 14.0), n((ww - 60.0) / 28.0)));
    }
    for (m, lbl, color, up) in marks {
        if !m.is_finite() {
            continue;
        }
        let x = sx(*m);
        p.push(format!("<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{color}\" stroke-width=\"2.4\"/>", n(x), if *up { 18 } else { 30 }, n(x), if *up { 56 } else { 68 }));
        let anchor = if x < 60.0 { "start" } else if x > w - 60.0 { "end" } else { "middle" };
        let tx = match anchor {
            "start" => 2f64.max(x - 4.0),
            "end" => (w - 2.0).min(x + 4.0),
            _ => x,
        };
        p.push(format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"{anchor}\" fill=\"{color}\" font-size=\"12.5\" font-weight=\"700\" font-family=\"Manrope, Segoe UI\">{lbl}</text>",
            n(tx),
            if *up { 13 } else { 82 }
        ));
    }
    p.push("</svg>".into());
    crate::js::svg_div("pc-stripbox", &p.concat())
}

fn big(l: &str, v: &str, cls: &str) -> Node {
    el("div", "pc-big").child(label("pc-bl", l)).child(Node::el("div", &format!("pc-bv {cls}")).text(v))
}

fn kv(rows: &[(String, String, &str)]) -> Node {
    let mut t = el("div", "pc-kv");
    for (k, v, c) in rows {
        t = t.child(span("pc-k", k)).child(Node::el("span", &format!("pc-v {c}")).text(v));
    }
    t
}

fn actions(kind: &str) -> Node {
    el("div", "pc-actions")
        .child(button("pc-btn", "FROM OFP").attr("type", "button").act("pc:ofp"))
        .child(button("pc-btn", "FROM AIRCRAFT").attr("type", "button").act(&format!("pc:aircraft:{kind}")))
        .child(button("pc-btn", "FROM METAR").attr("type", "button").act(&format!("pc:metar:{kind}")))
        .child(button("pc-btn pc-go", "CALCULATE").attr("type", "button").act(&format!("pc:calc:{kind}")))
}

fn grid(kids: Vec<Node>) -> Node {
    el("div", "pc-grid").children(kids)
}

fn takeoff_view(v: &PerfView) -> Node {
    let p = to_params(v);
    let ph = |k: &str| p.rw.as_ref().map(|r| crate::jv::sv(r, k, "undefined")).unwrap_or_default();
    let mut inputs = el("div", "pc-card pc-inputs")
        .child(label("pc-h", "RUNWAY"))
        .child(grid(vec![field(v, "to", "icao", "Airport", text("ICAO"), ""), runway_field(v, "to"), field(v, "to", "tora", "TORA override", text(&ph("tora_m")), "m"), field(v, "to", "slope", "Slope", text(""), "%")]))
        .child(field(v, "to", "lineup", "Line-up angle", seg(&["0", "90", "180"]), ""))
        .child(field(v, "to", "cond", "Runway condition", Opts::Select(TO_CONDS.iter().map(|c| c.0.to_string()).collect()), ""))
        .child(label("pc-h", "CONDITIONS"))
        .child(grid(vec![
            field(v, "to", "wdir", "Wind dir", text("calm"), "\u{b0}"),
            field(v, "to", "wspd", "Wind speed", text(""), "kt"),
            field(v, "to", "oat", "OAT", text(""), "\u{b0}C"),
            field(v, "to", "qnh", "QNH", text(""), "hPa"),
        ]))
        .child(label("pc-h", "AIRCRAFT"))
        .child(grid(vec![field(v, "to", "tow", "TOW", text("tonnes"), "t"), field(v, "to", "cg", "CG", text(""), "% MAC")]))
        .child(field(v, "to", "conf", "Config", seg(&["AUTO", "1+F", "2", "3"]), ""))
        .child(field(v, "to", "thrust", "Thrust", seg(&["FLEX", "TOGA", "DERATE"]), ""));
    if v.to.get("thrust").map(String::as_str) == Some("DERATE") {
        inputs = inputs.child(field(v, "to", "drt", "Derate level", seg(&["D01", "D02", "D03", "D04", "D05"]), ""));
    }
    inputs = inputs.child(grid(vec![field(v, "to", "packs", "Packs", seg(&["ON", "OFF"]), ""), field(v, "to", "ai", "Anti-ice", seg(&["OFF", "ENG", "ENG+WING"]), "")])).child(actions("to"));
    let mut out = el("div", "pc-card pc-out");
    let g = |k: &str| v.to.get(k).cloned().unwrap_or_default();
    if !v.calc_to {
        out = out.child(label("pc-empty", "Enter the runway, conditions and weight, then CALCULATE."));
    } else if p.rw.is_none() && pc_num(&g("tora"), 0.0) == 0.0 {
        out = out.child(label("pc-empty pc-bad", "Pick an airport and runway, or enter TORA."));
    } else if p.tow == 0.0 {
        out = out.child(label("pc-empty pc-bad", "Enter the takeoff weight."));
    } else {
        let mut q = p.clone();
        if q.rw.is_none() {
            if q.toda == 0.0 {
                q.toda = q.tora;
            }
            if q.asda == 0.0 {
                q.asda = q.tora;
            }
        }
        let r = takeoff(&q);
        out = out.child(label("pc-h", &format!("{} {}  \u{2022}  CONF {}", g("icao"), g("rwy"), r.conf)));
        let thrust_v = match r.flex {
            Some(f) => num(f),
            None if q.derate_pct != 0.0 => g("drt"),
            None => "TOGA".into(),
        };
        out = out.child(
            el("div", "pc-bigs")
                .child(big("V1", &num(r.v1), ""))
                .child(big("VR", &num(r.vr), ""))
                .child(big("V2", &num(r.v2), ""))
                .child(big(if r.flex.is_some() { "FLEX" } else { "THRUST" }, &thrust_v, if r.ok { "" } else { "pc-bad" }))
                .child(big("THS", &format!("{}{}", if r.trim >= 0.0 { "UP " } else { "DN " }, fx(r.trim.abs(), 1)), "")),
        );
        if !r.ok {
            out = out.child(label("pc-warn", &format!("NOT PERMITTED: {}. Max TOW for this runway {} t.", r.limits.join(", "), fx(r.limit_tow, 1))));
        }
        if q.tow > q.mtow {
            out = out.child(label("pc-warn", &format!("TOW above structural MTOW ({} t).", num(q.mtow))));
        }
        out = out.child(strip(
            q.toda.max(q.asda),
            &[(q.lineup[0], "LINE-UP", "#9aa0a6", true), (r.v1x, "V1", "#3fb0ff", true), (r.asd, "ASD", "#f5a623", false), (r.tod_oei, "TOD OEI", "#ff4fd8", false), (r.tod_aeo, "TOD", "#4cd964", true)],
        ));
        let rd = |x: f64| num(round(x));
        let gb = |bad: bool| if bad { "pc-bad" } else { "pc-good" };
        out = out.child(kv(&[
            ("TODA / TOD AEO (x1.15)".into(), format!("{} / {} m", rd(r.toda), rd(r.tod_aeo)), gb(r.tod_aeo > r.toda)),
            ("TODA / TOD OEI".into(), format!("{} / {} m", rd(r.toda), rd(r.tod_oei)), gb(r.tod_oei > r.toda)),
            ("ASDA / ASD".into(), format!("{} / {} m", rd(r.asda), rd(r.asd)), gb(r.asd > r.asda)),
            ("2nd segment gradient".into(), format!("{} % (min 3.0)", fx(r.grad2 * 100.0, 1)), gb(r.grad2 < 0.03)),
            ("Lift-off ground speed".into(), format!("{} kt (tyres 204)", rd(r.vlof_gs)), if r.vlof_gs > 204.0 { "pc-bad" } else { "" }),
            ("Runway limit TOW".into(), format!("{} t", fx(r.limit_tow, 1)), ""),
            ("Head / cross wind".into(), format!("{} / {} kt", rd(q.hw), num(round(q.xw).abs())), if q.xw.abs() > 35.0 { "pc-bad" } else { "" }),
            ("Pressure altitude".into(), format!("{} ft", rd(q.atm.pa)), ""),
            ("Thrust per engine".into(), format!("{} kN static", fx(r.f0 / 1000.0, 0)), ""),
        ]));
    }
    el("div", "pc-wrap").child(inputs).child(out)
}

fn landing_view(v: &PerfView) -> Node {
    let p = ldg_params(v);
    let ph = p.rw.as_ref().map(|r| crate::jv::sv(r, "lda_m", "undefined")).unwrap_or_default();
    let inputs = el("div", "pc-card pc-inputs")
        .child(label("pc-h", "RUNWAY"))
        .child(grid(vec![field(v, "ldg", "icao", "Airport", text("ICAO"), ""), runway_field(v, "ldg"), field(v, "ldg", "lda", "LDA override", text(&ph), "m"), field(v, "ldg", "slope", "Slope", text(""), "%")]))
        .child(field(v, "ldg", "cond", "Runway condition code", Opts::Select(LDG_CONDS.iter().map(|c| c.0.to_string()).collect()), ""))
        .child(label("pc-h", "CONDITIONS"))
        .child(grid(vec![
            field(v, "ldg", "wdir", "Wind dir", text("calm"), "\u{b0}"),
            field(v, "ldg", "wspd", "Wind speed", text(""), "kt"),
            field(v, "ldg", "oat", "OAT", text(""), "\u{b0}C"),
            field(v, "ldg", "qnh", "QNH", text(""), "hPa"),
        ]))
        .child(label("pc-h", "AIRCRAFT"))
        .child(grid(vec![field(v, "ldg", "lw", "Landing weight", text("tonnes"), "t"), field(v, "ldg", "cg", "CG", text(""), "% MAC")]))
        .child(field(v, "ldg", "conf", "Config", seg(&["3", "FULL"]), ""))
        .child(field(v, "ldg", "ab", "Autobrake", seg(&["ALL", "LO", "2", "3", "HI", "MAX"]), ""))
        .child(grid(vec![field(v, "ldg", "rev", "Reversers", seg(&["ON", "OFF"]), ""), field(v, "ldg", "autoland", "Autoland", seg(&["NO", "YES"]), ""), field(v, "ldg", "ice", "Ice accretion", seg(&["NO", "YES"]), "")]))
        .child(actions("ldg"));
    let mut out = el("div", "pc-card pc-out");
    let g = |k: &str| v.ldg.get(k).cloned().unwrap_or_default();
    if !v.calc_ldg {
        out = out.child(label("pc-empty", "Enter the runway, conditions and weight, then CALCULATE."));
    } else if p.lda == 0.0 {
        out = out.child(label("pc-empty pc-bad", "Pick an airport and runway, or enter LDA."));
    } else if p.lw == 0.0 {
        out = out.child(label("pc-empty pc-bad", "Enter the landing weight."));
    } else {
        let r = landing(&p);
        out = out.child(label("pc-h", &format!("{} {}  \u{2022}  CONF {}", g("icao"), g("rwy"), p.conf)));
        out = out.child(
            el("div", "pc-bigs")
                .child(big("VLS", &num(r.vls), ""))
                .child(big("VAPP", &num(r.vapp), ""))
                .child(big("LDA", &format!("{} m", num(round(r.lda))), ""))
                .child(big("WIND", &format!("{} HW", num(round(p.hw))), "")),
        );
        if r.mlw_over {
            out = out.child(label("pc-warn", &format!("Landing weight above MLW ({} t): overweight landing.", num(p.mlw))));
        }
        let max = r.rows.iter().map(|x| x.dispatch).fold(r.lda, f64::max);
        let mut bars = el("div", "pc-bars");
        for row in &r.rows {
            let ok = row.inflight <= r.lda;
            bars = bars.child(
                el("div", "pc-bar")
                    .child(span("pc-barl", &if row.ab == "MAX" { "MAX MAN".to_string() } else { format!("AUTO {}", row.ab) }))
                    .child(
                        el("span", "pc-track")
                            .child(el("span", "pc-fill").class_if("bad", !ok).style(&format!("width:{}%", num((row.actual / max) * 100.0))))
                            .child(el("span", "pc-fact").style(&format!("left:{}%", num((row.inflight / max) * 100.0))))
                            .child(el("span", "pc-lda").style(&format!("left:{}%", num((r.lda / max) * 100.0)))),
                    )
                    .child(span("pc-barv", &format!("{} / {} m", num(round(row.actual)), num(round(row.inflight)))).class_if("pc-bad", !ok)),
            );
        }
        out = out.child(label("pc-h", "LANDING DISTANCE  (ACTUAL / IN-FLIGHT x1.15)")).child(bars);
        let mut rows: Vec<(String, String, &str)> = r.rows.iter().map(|x| (format!("Dispatch required, {}", x.ab), format!("{} m", num(round(x.dispatch))), if x.dispatch > r.lda { "pc-bad" } else { "pc-good" })).collect();
        rows.push(("Head / cross wind".into(), format!("{} / {} kt", num(round(p.hw)), num(round(p.xw).abs())), if p.xw.abs() > 40.0 { "pc-bad" } else { "" }));
        rows.push(("Pressure altitude".into(), format!("{} ft", num(round(p.atm.pa))), ""));
        out = out.child(kv(&rows));
    }
    el("div", "pc-wrap").child(inputs).child(out)
}

pub fn perf_content(v: &PerfView) -> Node {
    let tab = if v.tab.is_empty() { "to" } else { v.tab.as_str() };
    let mut tabs = el("div", "pc-tabs");
    for (k, t) in [("to", "TAKEOFF"), ("ldg", "LANDING")] {
        tabs = tabs.child(el("button", "pc-tab").class_if("on", tab == k).attr("type", "button").act(&format!("pc:tab:{k}")).text(t));
    }
    Node::new("div").id("perf-content").child(tabs).child(label("pc-note", &cal_text(&v.cal))).child(if tab == "to" { takeoff_view(v) } else { landing_view(v) })
}

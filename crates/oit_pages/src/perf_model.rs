use crate::perf_common::js_round;
use serde::Deserialize;

const G: f64 = 9.80665;
const S: f64 = 845.0;
const KT: f64 = 0.514444;
const FT: f64 = 0.3048;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum ToConf {
    #[serde(rename = "1+F")]
    OnePlusF,
    #[serde(rename = "2")]
    Two,
    #[serde(rename = "3")]
    Three,
}

impl ToConf {
    pub fn label(self) -> &'static str {
        match self {
            ToConf::OnePlusF => "1+F",
            ToConf::Two => "2",
            ToConf::Three => "3",
        }
    }

    fn ld(self) -> (f64, f64) {
        match self {
            ToConf::OnePlusF => (0.074, 13.2),
            ToConf::Two => (0.081, 12.1),
            ToConf::Three => (0.092, 10.6),
        }
    }

    fn vls_key(self) -> VlsKey {
        match self {
            ToConf::OnePlusF => VlsKey::OnePlusF,
            ToConf::Two => VlsKey::Two,
            ToConf::Three => VlsKey::Three,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum ToConfChoice {
    #[serde(rename = "AUTO")]
    Auto,
    #[serde(rename = "1+F")]
    OnePlusF,
    #[serde(rename = "2")]
    Two,
    #[serde(rename = "3")]
    Three,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum LdgConf {
    #[serde(rename = "3")]
    Three,
    #[serde(rename = "FULL")]
    Full,
}

#[derive(Clone, Copy)]
enum VlsKey {
    OnePlusF,
    Two,
    Three,
    Full,
}

fn vls_table(k: VlsKey) -> [[f64; 8]; 2] {
    match k {
        VlsKey::OnePlusF => [
            [123.0, 133.0, 144.0, 154.0, 163.0, 172.0, 180.0, 188.0],
            [123.0, 130.0, 140.0, 150.0, 159.0, 167.0, 176.0, 183.0],
        ],
        VlsKey::Two => [
            [123.0, 128.0, 138.0, 148.0, 157.0, 165.0, 173.0, 181.0],
            [123.0, 125.0, 135.0, 144.0, 153.0, 161.0, 169.0, 176.0],
        ],
        VlsKey::Three => [
            [123.0, 124.0, 134.0, 144.0, 152.0, 161.0, 169.0, 176.0],
            [123.0, 123.0, 131.0, 140.0, 148.0, 156.0, 164.0, 171.0],
        ],
        VlsKey::Full => [
            [123.0, 123.0, 131.0, 140.0, 148.0, 157.0, 165.0, 173.0],
            [123.0, 123.0, 127.0, 136.0, 144.0, 152.0, 160.0, 168.0],
        ],
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RunwayCondition {
    pub mu: f64,
    pub roll: f64,
    pub wet: bool,
    pub rev: bool,
}

pub const TO_COND_DRY: RunwayCondition = RunwayCondition { mu: 0.42, roll: 0.015, wet: false, rev: false };
pub const TO_COND_WET: RunwayCondition = RunwayCondition { mu: 0.25, roll: 0.02, wet: true, rev: true };

pub fn ldg_mu(name: &str) -> Option<f64> {
    match name {
        "DRY" => Some(0.38),
        "GOOD" => Some(0.28),
        "MEDIUM" => Some(0.17),
        _ => None,
    }
}

pub fn autobrake_decel(name: &str) -> f64 {
    match name {
        "LO" => 1.7,
        "2" => 2.2,
        "3" => 3.0,
        "HI" => 3.5,
        _ => 99.0,
    }
}

pub fn lineup(angle: &str) -> (f64, f64) {
    match angle {
        "0" => (0.0, 0.0),
        "180" => (35.0, 56.0),
        _ => (24.0, 45.0),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct PerfCalibration {
    pub thrust: f64,
    pub roll: f64,
    pub brake: f64,
    pub takeoffs: u32,
    pub landings: u32,
}

impl Default for PerfCalibration {
    fn default() -> Self {
        NO_CALIBRATION
    }
}

pub const NO_CALIBRATION: PerfCalibration = PerfCalibration { thrust: 1.0, roll: 1.0, brake: 1.0, takeoffs: 0, landings: 0 };

impl PerfCalibration {
    pub fn clamped(thrust: Option<f64>, roll: Option<f64>, brake: Option<f64>, takeoffs: u32, landings: u32) -> PerfCalibration {
        let c = |v: Option<f64>| match v {
            Some(x) if x.is_finite() => x.clamp(0.7, 1.3),
            _ => 1.0,
        };
        PerfCalibration { thrust: c(thrust), roll: c(roll), brake: c(brake), takeoffs, landings }
    }
}

pub fn calibration_text(c: &PerfCalibration) -> String {
    if c.takeoffs + c.landings == 0 {
        "NO X-PLANE CALIBRATION YET: FACTORS ARE LEARNT FROM YOUR TAKE-OFFS AND LANDINGS".into()
    } else {
        format!(
            "X-PLANE CALIBRATION FROM {} T/O AND {} LDG: THRUST {:.2}, ROLL {:.2}, BRAKING {:.2}",
            c.takeoffs, c.landings, c.thrust, c.roll, c.brake
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Atmos {
    pub pa: f64,
    pub delta: f64,
    pub isa: f64,
    pub sigma: f64,
    pub rho: f64,
}

pub fn atmos(elev_ft: f64, qnh: f64, oat: f64) -> Atmos {
    let pa = elev_ft + (1013.25 - qnh) * 27.3;
    let delta = (1.0 - 6.8756e-6 * pa).max(0.2).powf(5.2559);
    let isa = 15.0 - (1.98 * pa) / 1000.0;
    let sigma = (delta * 288.15) / (oat + 273.15);
    Atmos { pa, delta, isa, sigma, rho: 1.225 * sigma }
}

fn vls_key(conf: VlsKey, tons: f64, cg: f64) -> f64 {
    let t = vls_table(conf);
    let at = |row: &[f64; 8]| {
        let x = tons.min(600.0).max(350.0);
        let i = (((x - 250.0) / 50.0).floor()).min(6.0);
        let iu = i as usize;
        let v = row[iu] + (row[iu + 1] - row[iu]) * ((x - 250.0) / 50.0 - i);
        if tons >= 350.0 { v } else { v * (tons / 350.0).sqrt() }
    };
    let k = ((cg - 29.0) / 14.0).min(1.0).max(0.0);
    at(&t[0]) * (1.0 - k) + at(&t[1]) * k
}

pub fn vls_to(conf: ToConf, tons: f64, cg: f64) -> f64 {
    vls_key(conf.vls_key(), tons, cg)
}

pub fn vls_ldg(conf: LdgConf, tons: f64, cg: f64) -> f64 {
    vls_key(if conf == LdgConf::Three { VlsKey::Three } else { VlsKey::Full }, tons, cg)
}

fn vmcg(pa: f64) -> f64 {
    118.0 - (0.6 * pa) / 1000.0
}

fn vmca(pa: f64) -> f64 {
    123.0 - (0.8 * pa) / 1000.0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum AntiIce {
    #[serde(rename = "OFF")]
    Off,
    #[serde(rename = "ENG")]
    Eng,
    #[serde(rename = "ENG+WING")]
    EngWing,
}

fn engine_thrust(rating_kn: f64, atm: &Atmos, temp_eff: f64, packs: bool, ai: AntiIce, cal: &PerfCalibration) -> f64 {
    let flat = atm.isa + 15.0;
    let temp = if temp_eff <= flat { 1.0 } else { (1.0 - 0.0095 * (temp_eff - flat)).max(0.35) };
    let bleed = (if packs { 0.988 } else { 1.0 })
        * (match ai {
            AntiIce::EngWing => 0.975,
            AntiIce::Eng => 0.99,
            AntiIce::Off => 1.0,
        });
    rating_kn * 1000.0 * atm.delta.powf(0.8) * temp * bleed * cal.thrust
}

fn thrust_at(f0: f64, tas_kt: f64) -> f64 {
    f0 * (1.0 - 0.00165 * tas_kt + 1.3e-6 * tas_kt * tas_kt)
}

#[allow(clippy::too_many_arguments)]
fn ground(m: f64, cd: f64, mu: f64, slope: f64, atm: &Atmos, f0: f64, engines: f64, v0: f64, v1: f64, hw: f64, roll: f64) -> f64 {
    let mut v = v0;
    let mut x = 0.0;
    let mut t = 0.0;
    let dt = 0.05;
    let sin = (slope / 100.0).atan().sin();
    while v < v1 && t < 200.0 {
        let q = 0.5 * atm.rho * v * v;
        let lift = q * S * 0.45;
        let a = (engines * thrust_at(f0, v / KT) - q * S * cd - mu * (m * G - lift).max(0.0) - m * G * sin) / m;
        if a <= 0.01 {
            return f64::INFINITY;
        }
        v += a * dt;
        x += (v - hw) * dt;
        t += dt;
    }
    x * roll
}

#[allow(clippy::too_many_arguments)]
fn air(m: f64, conf: ToConf, f0: f64, engines: f64, vlof: f64, v2: f64, hw: f64, screen_ft: f64) -> f64 {
    let ld = conf.ld().1;
    let h = screen_ft * FT;
    let v = (vlof + v2) / 2.0;
    let thrust = engines * thrust_at(f0, v / KT) - if engines < 4.0 { 0.012 * m * G } else { 0.0 };
    let gamma = thrust / (m * G) - 1.0 / (ld - 3.2);
    if gamma <= 0.002 {
        return f64::INFINITY;
    }
    ((h / (0.5 * gamma)) * (v - hw)) / v
}

#[derive(Clone, Debug)]
pub struct TakeoffInput {
    pub atm: Atmos,
    pub oat: f64,
    pub tow_t: f64,
    pub cg: f64,
    pub conf: ToConfChoice,
    pub flex: bool,
    pub derate_pct: f64,
    pub rating_kn: f64,
    pub packs: bool,
    pub ai: AntiIce,
    pub slope: f64,
    pub cond: RunwayCondition,
    pub toda: f64,
    pub asda: f64,
    pub lineup: (f64, f64),
    pub hw: f64,
    pub cal: PerfCalibration,
}

#[derive(Clone, Debug)]
pub struct TakeoffResult {
    pub ok: bool,
    pub limits: Vec<String>,
    pub v1: f64,
    pub vr: f64,
    pub v2: f64,
    pub conf: ToConf,
    pub flex: Option<f64>,
    pub tod_aeo: f64,
    pub tod_oei: f64,
    pub asd: f64,
    pub toda: f64,
    pub asda: f64,
    pub grad2: f64,
    pub trim: f64,
    pub limit_tow_t: f64,
}

#[derive(Clone, Debug)]
struct Case {
    ok: bool,
    limits: Vec<String>,
    v1: f64,
    vr: f64,
    v2: f64,
    conf: ToConf,
    tod_aeo: f64,
    tod_oei: f64,
    asd: f64,
    toda: f64,
    asda: f64,
    grad2: f64,
}

struct V1Case {
    v1: f64,
    tod_oei: f64,
    asd: f64,
    ke: f64,
    margin: f64,
}

fn takeoff_case(p: &TakeoffInput, temp_eff: f64, conf: ToConf, dry_check: bool) -> Case {
    let atm = &p.atm;
    let m = p.tow_t * 1000.0;
    let f0 = engine_thrust(p.rating_kn, atm, temp_eff, p.packs, p.ai, &p.cal) * (1.0 - p.derate_pct / 100.0);
    let (cd, ld) = conf.ld();
    let vs_cas = vls_to(conf, p.tow_t, p.cg) / 1.23;
    let to_tas = |cas: f64| cas / atm.sigma.sqrt();
    let vmc_ratio = (1.0 - p.derate_pct / 100.0).sqrt();
    let vmcg_v = vmcg(atm.pa) * vmc_ratio;
    let vmca_v = vmca(atm.pa) * vmc_ratio;
    let v2 = (1.13 * vs_cas).max(1.1 * vmca_v).ceil();
    let vr = (1.05 * vmca_v).max(v2 - 6.0).ceil();
    let hw = if p.hw >= 0.0 { p.hw * 0.5 } else { p.hw * 1.5 };
    let hw_m = hw * KT;
    let vr_t = to_tas(vr) * KT;
    let v2_t = to_tas(v2) * KT;
    let vlof_t = to_tas(vr + 4.0) * KT;
    let r = p.cal.roll;
    let gnd = ground(m, cd, p.cond.roll, p.slope, atm, f0, 4.0, 0.0, vr_t, hw_m, r);
    let rot = ground(m, cd * 1.15, p.cond.roll * 0.4, p.slope, atm, f0, 4.0, vr_t, vlof_t, hw_m, r);
    let tod_aeo = 1.15 * (gnd + rot + air(m, conf, f0, 4.0, vlof_t, v2_t * 1.04, hw_m, 35.0));
    let grad2 = (3.0 * thrust_at(f0, to_tas(v2)) - 0.012 * m * G) / (m * G) - 1.0 / ld;
    let vlof_gs = to_tas(vr + 4.0) - hw;
    let sin = (p.slope / 100.0).atan().sin();
    let case_for = |v1: f64| -> V1Case {
        let v1_t = to_tas(v1) * KT;
        let vef_t = (to_tas(v1 - 3.0) * KT).max(1.0);
        let a1 = ground(m, cd, p.cond.roll, p.slope, atm, f0, 4.0, 0.0, vef_t, hw_m, r);
        let a2 = ground(m, cd, p.cond.roll, p.slope, atm, f0, 3.0, vef_t, vr_t, hw_m, r);
        let a3 = ground(m, cd * 1.15, p.cond.roll * 0.4, p.slope, atm, f0, 3.0, vr_t, to_tas(vr + 3.0) * KT, hw_m, r);
        let tod_oei = a1 + a2 + a3 + air(m, conf, f0, 3.0, to_tas(vr + 3.0) * KT, v2_t, hw_m, if p.cond.wet { 15.0 } else { 35.0 });
        let acc = ground(m, cd, p.cond.roll, p.slope, atm, f0, 4.0, 0.0, v1_t, hw_m, r);
        let mut v = v1_t;
        let mut x = acc + 2.0 * (v1_t - hw_m);
        let mu = p.cond.mu * p.cal.brake;
        let mut t = 0.0;
        while v > 0.5 && t < 120.0 {
            let q = 0.5 * atm.rho * v * v;
            let rev = if p.cond.rev { 2.0 * 0.18 * thrust_at(f0, v / KT) * (v / (60.0 * KT)).min(1.0) } else { 0.0 };
            let dec = mu * G * (1.0 - (q * S * 0.2) / (m * G)).max(0.2) + (q * S * (cd + 0.06) + rev - 4.0 * 0.06 * f0) / m + G * sin;
            v -= dec.max(0.3) * 0.05;
            x += (v - hw_m).max(0.0) * 0.05;
            t += 0.05;
        }
        V1Case { v1, tod_oei, asd: x, ke: 0.5 * m * v1_t * v1_t, margin: 0.0 }
    };
    let toda = p.toda - p.lineup.0;
    let asda = p.asda - p.lineup.1;
    let mut best: Option<V1Case> = None;
    let mut v1 = vmcg_v.ceil();
    while v1 <= vr {
        let mut c = case_for(v1);
        c.margin = (toda - c.tod_oei).min(asda - c.asd);
        if best.as_ref().map_or(true, |b| c.margin > b.margin) {
            best = Some(c);
        }
        v1 += 1.0;
    }
    let best = best.unwrap_or_else(|| {
        let mut c = case_for(vr);
        c.margin = f64::NEG_INFINITY;
        c
    });
    let mut limits: Vec<String> = vec![];
    if tod_aeo > toda {
        limits.push("RUNWAY (TOD AEO)".into());
    }
    if best.tod_oei > toda {
        limits.push("RUNWAY (TOD OEI)".into());
    }
    if best.asd > asda {
        limits.push("RUNWAY (ASD)".into());
    }
    if grad2 < 0.03 {
        limits.push("SECOND SEGMENT".into());
    }
    if vlof_gs > 204.0 {
        limits.push("TYRE SPEED".into());
    }
    if best.ke > 16.0 * 130e6 {
        limits.push("BRAKE ENERGY".into());
    }
    if vr < vmcg_v {
        limits.push("VMCG".into());
    }
    if p.cond.wet && !dry_check {
        let mut dp = p.clone();
        dp.cond = TO_COND_DRY;
        let dry = takeoff_case(&dp, temp_eff, conf, true);
        for l in dry.limits {
            if !limits.contains(&l) {
                limits.push(format!("{l} (DRY)"));
            }
        }
    }
    Case {
        ok: limits.is_empty(),
        limits,
        v1: best.v1,
        vr,
        v2,
        conf,
        tod_aeo,
        tod_oei: best.tod_oei,
        asd: best.asd,
        toda,
        asda,
        grad2,
    }
}

pub fn trim_for(cg: f64) -> f64 {
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

fn limit_weight(p: &TakeoffInput, conf: ToConf) -> f64 {
    let mut lo = 250.0;
    let mut hi = 620.0;
    for _ in 0..18 {
        let mid = (lo + hi) / 2.0;
        let mut q = p.clone();
        q.tow_t = mid;
        if takeoff_case(&q, p.oat, conf, false).ok {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

struct Scored {
    case: Case,
    flex: Option<f64>,
}

fn score(x: &Scored) -> f64 {
    if x.case.ok {
        1000.0 + x.flex.unwrap_or(0.0) * 10.0 + if x.case.conf == ToConf::Two { 0.5 } else { 0.0 }
    } else {
        -1.0
    }
}

pub fn takeoff(p: &TakeoffInput) -> TakeoffResult {
    let confs: Vec<ToConf> = match p.conf {
        ToConfChoice::Auto => vec![ToConf::OnePlusF, ToConf::Two, ToConf::Three],
        ToConfChoice::OnePlusF => vec![ToConf::OnePlusF],
        ToConfChoice::Two => vec![ToConf::Two],
        ToConfChoice::Three => vec![ToConf::Three],
    };
    let flat = p.atm.isa + 15.0;
    let tmax = p.atm.isa + 70.0;
    let mut best: Option<Scored> = None;
    for conf in confs {
        let toga = takeoff_case(p, p.oat, conf, false);
        if !toga.ok {
            let take = match &best {
                None => true,
                Some(b) => !b.case.ok && toga.tod_aeo < b.case.tod_aeo,
            };
            if take {
                best = Some(Scored { case: toga, flex: None });
            }
            continue;
        }
        let mut flex: Option<f64> = None;
        if p.flex {
            let toga_f0 = engine_thrust(p.rating_kn, &p.atm, p.oat, p.packs, p.ai, &p.cal);
            let mut lo = (p.oat + 1.0).max(flat.ceil());
            let mut hi = tmax.floor();
            let usable = |t: f64| {
                engine_thrust(p.rating_kn, &p.atm, t, p.packs, p.ai, &p.cal) >= 0.75 * toga_f0 && takeoff_case(p, t, conf, false).ok
            };
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
            Some(f) => Scored { case: takeoff_case(p, f, conf, false), flex: Some(f) },
            None => Scored { case: toga, flex: None },
        };
        let take = match &best {
            None => true,
            Some(b) => score(&r) > score(b),
        };
        if take {
            best = Some(r);
        }
    }
    let b = best.expect("at least one configuration");
    let limit_tow_t = limit_weight(p, b.case.conf);
    let c = b.case;
    TakeoffResult {
        ok: c.ok,
        limits: c.limits,
        v1: c.v1,
        vr: c.vr,
        v2: c.v2,
        conf: c.conf,
        flex: b.flex,
        tod_aeo: c.tod_aeo,
        tod_oei: c.tod_oei,
        asd: c.asd,
        toda: c.toda,
        asda: c.asda,
        grad2: c.grad2,
        trim: trim_for(p.cg),
        limit_tow_t,
    }
}

#[derive(Clone, Debug)]
pub struct LandingInput {
    pub atm: Atmos,
    pub lw_t: f64,
    pub cg: f64,
    pub conf: LdgConf,
    pub mu: f64,
    pub rev: bool,
    pub autoland: bool,
    pub slope: f64,
    pub hw: f64,
    pub lda: f64,
    pub cal: PerfCalibration,
}

#[derive(Clone, Debug)]
pub struct LandingRow {
    pub ab: String,
    pub actual: f64,
    pub inflight: f64,
    pub dispatch: f64,
}

#[derive(Clone, Debug)]
pub struct LandingResult {
    pub vls: f64,
    pub vapp: f64,
    pub rows: Vec<LandingRow>,
}

pub fn landing(p: &LandingInput, modes: &[&str]) -> LandingResult {
    let atm = &p.atm;
    let m = p.lw_t * 1000.0;
    let vls_v = vls_ldg(p.conf, p.lw_t, p.cg).max(123.0);
    let vapp = js_round(vls_v + (p.hw / 3.0).max(5.0).min(15.0));
    let hw_f = if p.hw >= 0.0 { p.hw * 0.5 } else { p.hw * 1.5 };
    let tas = |cas: f64| cas / atm.sigma.sqrt();
    let gs_td = (tas(vapp - 5.0) - hw_f) * KT;
    let air_dist = 430.0 * ((tas(vapp) - hw_f) / 140.0);
    let mu = p.mu * p.cal.brake;
    let sin = (p.slope / 100.0).atan().sin();
    let run = |target: f64| {
        let mut v = tas(vapp - 5.0) * KT;
        let mut x = air_dist + 2.5 * gs_td;
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
    let rows = modes
        .iter()
        .map(|ab| {
            let d = run(autobrake_decel(ab));
            LandingRow {
                ab: ab.to_string(),
                actual: d,
                inflight: d * 1.15,
                dispatch: (d / 0.6) * if p.mu < 0.3 { 1.15 } else { 1.0 },
            }
        })
        .collect();
    LandingResult { vls: js_round(vls_v), vapp, rows }
}

pub fn crosswind_limit_kt(mu: f64, autoland: bool) -> f64 {
    if autoland {
        20.0
    } else if mu >= 0.38 {
        40.0
    } else if mu >= 0.28 {
        38.0
    } else {
        29.0
    }
}

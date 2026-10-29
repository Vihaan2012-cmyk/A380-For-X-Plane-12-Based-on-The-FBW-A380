use crate::flt_ops::{PerfResults, ResultSource, ToPerfModel};
use crate::perf_common::*;
use crate::perf_model::*;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct TlrRunway {
    pub identifier: String,
    pub speeds_v1: Option<String>,
    pub speeds_vr: Option<String>,
    pub speeds_v2: Option<String>,
    pub flap_setting: Option<String>,
    pub flex_temperature: Option<String>,
    pub thrust_setting: Option<String>,
    pub length_tora: Option<String>,
    pub length: Option<String>,
    pub distance_margin: Option<String>,
    pub speeds_vref: Option<String>,
    pub max_weight_dry: Option<String>,
    pub max_weight_wet: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum TlrRunwayList {
    Many(Vec<TlrRunway>),
    One(Box<TlrRunway>),
}

impl Default for TlrRunwayList {
    fn default() -> Self {
        TlrRunwayList::Many(vec![])
    }
}

impl TlrRunwayList {
    fn list(&self) -> Vec<&TlrRunway> {
        match self {
            TlrRunwayList::Many(v) => v.iter().collect(),
            TlrRunwayList::One(r) => vec![r],
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct TlrConditions {
    pub planned_weight: Option<String>,
    pub temperature: Option<String>,
    pub wind_direction: Option<String>,
    pub wind_speed: Option<String>,
    pub surface_condition: Option<String>,
    pub flap_setting: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct TlrPhase {
    pub runway: TlrRunwayList,
    pub conditions: TlrConditions,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct ToCompute {
    pub pressed: bool,
    pub tlr: Option<TlrPhase>,
    pub tlr_weight_units: String,
    pub calibration: PerfCalibration,
    pub rating_kn: Option<f64>,
}

impl Default for ToCompute {
    fn default() -> Self {
        ToCompute { pressed: false, tlr: None, tlr_weight_units: "KG".into(), calibration: NO_CALIBRATION, rating_kn: None }
    }
}

fn num(v: &Option<String>) -> Option<f64> {
    let s = v.as_deref()?.trim();
    if s.is_empty() {
        return None;
    }
    s.parse::<f64>().ok().filter(|n| n.is_finite())
}

fn unavailable_results() -> PerfResults {
    PerfResults {
        source: ResultSource::None,
        runway_text: "----/---".into(),
        v1: UNAVAILABLE.into(),
        vr: UNAVAILABLE.into(),
        v2: UNAVAILABLE.into(),
        flaps: UNAVAILABLE.into(),
        flex: UNAVAILABLE.into(),
        ths: UNAVAILABLE.into(),
        mtow: UNAVAILABLE.into(),
        stop_margin: UNAVAILABLE.into(),
        note: UNAVAILABLE_REASON.into(),
    }
}

fn kt_text(v: &Option<String>) -> String {
    num(v).map_or(UNAVAILABLE.to_string(), |n| format!("{} KT", fixed(js_round(n), 0)))
}

pub fn compute_results(m: &ToPerfModel, c: &ToCompute) -> PerfResults {
    let mut r = unavailable_results();
    let rwy = m.runway.as_ref();
    if !c.pressed {
        return r;
    }
    if let Some(rw) = rwy {
        r.runway_text = format!("{}/{}", if m.airport.text.is_empty() { "----" } else { &m.airport.text }, rw.ident);
    }
    let ident = rwy.map(|x| x.ident.as_str());
    let tlr = match &c.tlr {
        None => return show_model(m, c, r, "NO SIMBRIEF TAKE-OFF REPORT"),
        Some(t) => t,
    };
    let list = tlr.runway.list();
    let entry = ident.and_then(|id| list.iter().find(|e| e.identifier == id));
    let entry = match entry {
        None => {
            let why = if list.is_empty() {
                "THE SIMBRIEF OFP HAS NO TAKE-OFF REPORT".to_string()
            } else {
                format!("THE SIMBRIEF TAKE-OFF REPORT HAS NO RWY {}", ident.unwrap_or("---"))
            };
            return show_model(m, c, r, &why);
        }
        Some(e) => e,
    };
    let u = &m.units;
    r.v1 = kt_text(&entry.speeds_v1);
    r.vr = kt_text(&entry.speeds_vr);
    r.v2 = kt_text(&entry.speeds_v2);
    let flaps = entry.flap_setting.as_deref().map(str::trim).unwrap_or("");
    r.flaps = if flaps.is_empty() {
        UNAVAILABLE.into()
    } else if flaps.chars().next().map_or(false, |ch| ch.is_ascii_digit()) {
        format!("CONF {flaps}")
    } else {
        flaps.to_string()
    };
    let flex = num(&entry.flex_temperature);
    let thrust = entry.thrust_setting.as_deref().map(|s| s.trim().to_uppercase()).unwrap_or_default();
    r.flex = match flex {
        Some(f) => format!("{}°C", fixed(js_round(f), 0)),
        None if thrust == "FULL" || thrust == "TOGA" => "TOGA".into(),
        None if !thrust.is_empty() => thrust,
        None => UNAVAILABLE.into(),
    };
    let tlr_length = num(&entry.length_tora).or_else(|| num(&entry.length));
    let mut to_metres: Option<f64> = None;
    if let (Some(rw), Some(len)) = (rwy, tlr_length) {
        if rw.tora > 0.0 {
            let ratio = len / rw.tora;
            to_metres = if (ratio - 1.0).abs() < 0.2 {
                Some(1.0)
            } else if (ratio - 3.28084).abs() < 0.66 {
                Some(0.3048)
            } else {
                None
            };
        }
    }
    r.stop_margin = match (num(&entry.distance_margin), to_metres) {
        (Some(margin), Some(k)) => format!("{} {}", format_distance(Some(margin * k), u), distance_unit_label(u)),
        _ => UNAVAILABLE.into(),
    };
    r.source = ResultSource::Simbrief;
    let cond = &tlr.conditions;
    let planned = num(&cond.planned_weight);
    let weight_kg = planned.map(|w| if c.tlr_weight_units == "LB" { w * 0.45359237 } else { w });
    let mut parts = vec![format!("SIMBRIEF TAKE-OFF REPORT, RWY {}", entry.identifier)];
    if let Some(w) = weight_kg {
        parts.push(format!("AT {} {}", format_weight(Some(w), u), weight_unit_label(u)));
    }
    let mut assumed: Vec<String> = vec![];
    if let Some(t) = cond.temperature.as_deref().filter(|t| !t.is_empty()) {
        assumed.push(format!("{t}°C"));
    }
    if let (Some(d), Some(s)) = (cond.wind_direction.as_deref().filter(|x| !x.is_empty()), cond.wind_speed.as_deref().filter(|x| !x.is_empty())) {
        assumed.push(format!("WIND {:0>3}/{s}KT", d));
    }
    if let Some(sc) = cond.surface_condition.as_deref().filter(|x| !x.is_empty()) {
        assumed.push(sc.to_uppercase());
    }
    let tail = if assumed.is_empty() { String::new() } else { format!(" (SIMBRIEF ASSUMED {})", assumed.join(", ")) };
    r.note = format!(
        "{}{}: SIMBRIEF PLANNING DATA, NOT AIRBUS-CERTIFIED A380 PERFORMANCE. THS AND PERF MTOW ARE NOT IN THE REPORT.",
        parts.join(" "),
        tail
    );
    r
}

fn show_model(m: &ToPerfModel, c: &ToCompute, mut r: PerfResults, why: &str) -> PerfResults {
    let rwy = match &m.runway {
        None => {
            r.note = format!("{why}. SELECT A RUNWAY FOR THE OIS MODEL.");
            return r;
        }
        Some(x) => x,
    };
    let tow_kg = match m.weights.gross_weight_kg {
        None => {
            r.note = format!("{why}. AIRCRAFT WEIGHT NOT YET COMPUTED FOR THE OIS MODEL.");
            return r;
        }
        Some(x) => x,
    };
    let w = &m.weather;
    let oat = w.oat.unwrap_or(0.0);
    let qnh = w.qnh.unwrap_or(1013.25);
    let hw = match (w.wind_direction_true, w.wind_kt) {
        (Some(d), Some(k)) => k * ((d - rwy.bearing_true) * std::f64::consts::PI / 180.0).cos(),
        _ => 0.0,
    };
    let conf = match m.flaps.text.as_str() {
        "1+F" => ToConfChoice::OnePlusF,
        "2" => ToConfChoice::Two,
        "3" => ToConfChoice::Three,
        _ => ToConfChoice::Auto,
    };
    let ai = match m.anti_ice.text.as_str() {
        "ON [ENG]" => AntiIce::Eng,
        "ON [ENG & WING]" => AntiIce::EngWing,
        _ => AntiIce::Off,
    };
    let angle = if m.entry_angle.text.is_empty() { "0" } else { m.entry_angle.text.as_str() };
    let shift = m.takeoff_shift.unwrap_or(0.0);
    let input = TakeoffInput {
        atm: atmos(rwy.elevation_ft, qnh, oat),
        oat,
        tow_t: tow_kg / 1000.0,
        cg: m.weights.cg.unwrap_or(33.0),
        conf,
        flex: !m.thrust.text.starts_with("TOGA"),
        derate_pct: 0.0,
        rating_kn: c.rating_kn.filter(|x| *x != 0.0).unwrap_or(356.81),
        packs: m.air_cond.text != "OFF",
        ai,
        slope: rwy.slope_percent,
        cond: if m.rwy_cond.text == "WET" { TO_COND_WET } else { TO_COND_DRY },
        toda: (rwy.tora - shift).max(0.0),
        asda: (rwy.asda - shift).max(0.0),
        lineup: lineup(angle),
        hw,
        cal: c.calibration,
    };
    let res = takeoff(&input);
    let u = &m.units;
    r.v1 = format!("{} KT", fixed(res.v1, 0));
    r.vr = format!("{} KT", fixed(res.vr, 0));
    r.v2 = format!("{} KT", fixed(res.v2, 0));
    r.flaps = format!("CONF {}", res.conf.label());
    r.flex = res.flex.map_or("TOGA".to_string(), |f| format!("{}°C", fixed(f, 0)));
    r.ths = if res.trim >= 0.0 { format!("UP{}", fixed(res.trim, 1)) } else { format!("DN{}", fixed(res.trim.abs(), 1)) };
    r.mtow = format!("{} {}", format_weight(Some((res.limit_tow_t * 1000.0).min(A380_MAX_GW_KG)), u), weight_unit_label(u));
    r.stop_margin = format!("{} {}", format_distance(Some(res.asda - res.asd), u), distance_unit_label(u));
    r.source = ResultSource::Model;
    r.note = format!(
        "{why}: OIS MODEL RESULT, NOT AIRBUS-CERTIFIED.{} {}",
        if res.ok { String::new() } else { format!(" TAKE-OFF NOT POSSIBLE: {}.", res.limits.join(", ")) },
        calibration_text(&c.calibration)
    );
    r
}

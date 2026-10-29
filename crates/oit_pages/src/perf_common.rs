use serde::Deserialize;

pub const UNAVAILABLE: &str = "NOT AVAIL";

pub const UNAVAILABLE_REASON: &str = "PRESS COMPUTE. RESULTS COME FROM THE SIMBRIEF TAKE-OFF REPORT WHEN IT HAS THE RUNWAY, OTHERWISE FROM THE OIS MODEL CALIBRATED TO X-PLANE (NOT AIRBUS-CERTIFIED).";

pub const M_TO_FT: f64 = 1.0 / 0.3048;
pub const KG_TO_LB: f64 = 2.20462262;
pub const A380_MAX_GW_KG: f64 = 510_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WeightUnit {
    T,
    Klb,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AltimeterUnit {
    Hpa,
    Inhg,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DistanceUnit {
    M,
    Ft,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct Units {
    pub weight: WeightUnit,
    pub altimeter: AltimeterUnit,
    pub distance: DistanceUnit,
}

impl Default for Units {
    fn default() -> Self {
        Units { weight: WeightUnit::T, altimeter: AltimeterUnit::Hpa, distance: DistanceUnit::M }
    }
}

pub fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

fn to_fixed(x: f64, digits: usize) -> String {
    let scaled = x * 10f64.powi(digits as i32);
    let tie = scaled.is_finite() && scaled.abs() < 4.0e15 && (scaled.abs().fract() - 0.5).abs() == 0.0;
    let x = if tie { f64::from_bits(x.to_bits() + 1) } else { x };
    let s = format!("{:.*}", digits, x);
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        s[1..].to_string()
    } else {
        s
    }
}

pub fn format_weight(kg: Option<f64>, u: &Units) -> String {
    match kg {
        Some(v) if v.is_finite() => to_fixed(
            if u.weight == WeightUnit::T { v / 1000.0 } else { v * KG_TO_LB / 1000.0 },
            1,
        ),
        _ => "---.-".into(),
    }
}

pub fn weight_unit_label(u: &Units) -> &'static str {
    if u.weight == WeightUnit::T { "T" } else { "KLB" }
}

pub fn format_distance(metres: Option<f64>, u: &Units) -> String {
    match metres {
        Some(v) if v.is_finite() => to_fixed(js_round(if u.distance == DistanceUnit::M { v } else { v * M_TO_FT }), 0),
        _ => "----".into(),
    }
}

pub fn distance_unit_label(u: &Units) -> &'static str {
    if u.distance == DistanceUnit::M { "M" } else { "FT" }
}

pub fn format_pressure(hpa: f64, u: &Units) -> String {
    if !hpa.is_finite() {
        return "----".into();
    }
    if u.altimeter == AltimeterUnit::Hpa {
        to_fixed(js_round(hpa), 0)
    } else {
        to_fixed(hpa * 0.0295299830714, 2)
    }
}

pub fn pressure_unit_label(u: &Units) -> &'static str {
    if u.altimeter == AltimeterUnit::Hpa { "HPA" } else { "IN HG" }
}

pub fn fixed(x: f64, digits: usize) -> String {
    to_fixed(x, digits)
}

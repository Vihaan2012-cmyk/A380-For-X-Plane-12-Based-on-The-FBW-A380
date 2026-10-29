use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct StatusLine {
    pub class: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct StatusModel {
    pub xplane: StatusLine,
    pub systems: StatusLine,
    pub displays: StatusLine,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ChecklistRow {
    pub title: String,
    pub done: usize,
    pub total: usize,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Landing {
    pub vs_fpm: f64,
    pub g: f64,
    pub ias_kt: f64,
    pub gs_kt: f64,
    pub pitch_deg: f64,
    pub roll_deg: f64,
    pub weight_kg: f64,
    pub limit_fpm: f64,
    pub bounces: u32,
    pub grade: String,
    pub zulu_s: f64,
    pub airport: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Maintenance {
    pub legal: bool,
    pub blocking: Vec<String>,
    pub defects: usize,
    pub deferrals: usize,
    pub tech_log: usize,
    pub airframe_hours: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct EfbModel {
    pub settings: BTreeMap<String, String>,
    pub livery_values: BTreeMap<String, String>,
    pub session_keys: Vec<String>,
    pub category: usize,
    pub search: String,
    pub compact: bool,
    pub status: StatusModel,
    pub checklists: Vec<ChecklistRow>,
    pub pins: Vec<String>,
    pub ofp: Option<Value>,
    pub landings: Vec<Landing>,
    pub maintenance: Option<Maintenance>,
    pub offline: bool,
}

pub fn jstr(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

pub fn path<'a>(v: &'a Value, keys: &[&str]) -> &'a Value {
    let mut cur = v;
    for k in keys {
        cur = cur.get(*k).unwrap_or(&Value::Null);
    }
    cur
}

pub fn path_str(v: &Value, keys: &[&str]) -> String {
    jstr(path(v, keys))
}

pub fn path_f64(v: &Value, keys: &[&str]) -> Option<f64> {
    match path(v, keys) {
        Value::Number(n) => n.as_f64(),
        Value::String(s) if !s.is_empty() => s.trim().parse().ok(),
        _ => None,
    }
}

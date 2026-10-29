//! Three preferences the EFB writes into xphfbw.json (unprefixed keys, as
//! `app_settings.rs` documents): `failureSeverity`, `baroLink` and
//! `encoderAccel`. Read here, not through `app_settings`, and cached by the
//! file's modified time.

use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FailureSeverity {
    None,
    Minor,
    All,
}

impl FailureSeverity {
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_uppercase().as_str() {
            "NONE" => Some(Self::None),
            "MINOR" => Some(Self::Minor),
            "ALL" => Some(Self::All),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaroLink {
    Independent,
    LeftRight,
    LeftRightStandby,
}

impl BaroLink {
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_uppercase().as_str() {
            "INDEPENDENT" => Some(Self::Independent),
            "LR" => Some(Self::LeftRight),
            "LRS" => Some(Self::LeftRightStandby),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prefs {
    pub failure_severity: FailureSeverity,
    pub baro_link: BaroLink,
    pub encoder_accel: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Prefs {
    const DEFAULT: Self = Self { failure_severity: FailureSeverity::All, baro_link: BaroLink::Independent, encoder_accel: false };
}

fn as_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "on" | "yes" => Some(true),
            "false" | "0" | "off" | "no" => Some(false),
            _ => None,
        },
        Value::Number(n) => n.as_f64().map(|v| v != 0.0),
        _ => None,
    }
}

pub fn parse(text: &str) -> Prefs {
    let defaults = Prefs::DEFAULT;
    let Ok(Value::Object(map)) = serde_json::from_str::<Value>(text) else { return defaults };
    Prefs {
        failure_severity: map.get("failureSeverity").and_then(Value::as_str).and_then(FailureSeverity::parse).unwrap_or(defaults.failure_severity),
        baro_link: map.get("baroLink").and_then(Value::as_str).and_then(BaroLink::parse).unwrap_or(defaults.baro_link),
        encoder_accel: map.get("encoderAccel").and_then(as_bool).unwrap_or(defaults.encoder_accel),
    }
}

const RECHECK: Duration = Duration::from_millis(500);

struct Cache {
    checked: Option<Instant>,
    modified: Option<SystemTime>,
    prefs: Prefs,
}

static CACHE: Mutex<Cache> = Mutex::new(Cache { checked: None, modified: None, prefs: Prefs::DEFAULT });

pub fn get() -> Prefs {
    let mut cache = CACHE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if cache.checked.is_some_and(|t| t.elapsed() < RECHECK) {
        return cache.prefs;
    }
    let Some(root) = crate::xp::system_path() else { return cache.prefs };
    cache.checked = Some(Instant::now());
    let path = crate::settings_files::app_settings_path(&root);
    let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    if modified != cache.modified || cache.modified.is_none() {
        cache.modified = modified;
        cache.prefs = std::fs::read_to_string(&path).map(|t| parse(&t)).unwrap_or_default();
    }
    cache.prefs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_bad_keys_give_the_defaults() {
        assert_eq!(parse("{}"), Prefs::default());
        assert_eq!(parse("not json"), Prefs::default());
        assert_eq!(parse(r#"{"failureSeverity": "loud", "baroLink": 3, "encoderAccel": "maybe"}"#), Prefs::default());
    }

    #[test]
    fn the_three_keys_parse_in_any_case_and_bool_shape() {
        let p = parse(r#"{"failureSeverity": "minor", "baroLink": "lrs", "encoderAccel": "true"}"#);
        assert_eq!(p.failure_severity, FailureSeverity::Minor);
        assert_eq!(p.baro_link, BaroLink::LeftRightStandby);
        assert!(p.encoder_accel);
        let p = parse(r#"{"failureSeverity": "NONE", "baroLink": "LR", "encoderAccel": true}"#);
        assert_eq!(p.failure_severity, FailureSeverity::None);
        assert_eq!(p.baro_link, BaroLink::LeftRight);
        assert!(p.encoder_accel);
        assert!(!parse(r#"{"encoderAccel": false}"#).encoder_accel);
    }
}

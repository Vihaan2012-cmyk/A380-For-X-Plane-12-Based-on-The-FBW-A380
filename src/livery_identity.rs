use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{Map, Value};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LiveryIdentity {
    pub registration: String,
    pub msn: String,
    pub selcal: String,
    pub airline: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Effective {
    pub registration: String,
    pub msn: String,
    pub selcal: String,
    pub airline: String,
}

fn first_text(object: &Map<String, Value>, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|k| object.iter().find(|(name, _)| name.eq_ignore_ascii_case(k)).and_then(|(_, v)| v.as_str()))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn looks_like_registration(token: &str) -> bool {
    let len = token.chars().count();
    if !(4..=8).contains(&len) || !token.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-') {
        return false;
    }
    let letters = token.chars().any(|c| c.is_ascii_uppercase());
    let digits = token.chars().any(|c| c.is_ascii_digit());
    (token.contains('-') && letters && !token.starts_with('-') && !token.ends_with('-')) || (letters && digits)
}

/// A livery folder is named "<Airline> <registration>" with an optional "4K".
pub fn from_folder_name(folder: &str) -> LiveryIdentity {
    let mut words: Vec<&str> = folder.split_whitespace().collect();
    while words.last().is_some_and(|w| w.eq_ignore_ascii_case("4K") || w.eq_ignore_ascii_case("2K") || w.eq_ignore_ascii_case("8K")) {
        words.pop();
    }
    let registration = match words.last() {
        Some(last) if looks_like_registration(last) => {
            let r = (*last).to_string();
            words.pop();
            r
        }
        _ => String::new(),
    };
    LiveryIdentity { registration, airline: words.join(" "), ..Default::default() }
}

/// `operator.json` over `validation.json` over the folder name, field by field.
pub fn merge(folder: &str, operator: Option<&Value>, validation: Option<&Value>) -> LiveryIdentity {
    let mut out = from_folder_name(folder);
    if let Some(v) = validation.and_then(Value::as_object) {
        let r = first_text(v, &["registration"]);
        if !r.is_empty() {
            out.registration = r;
        }
    }
    if let Some(o) = operator.and_then(Value::as_object) {
        let pick = |keys: &[&str], into: &mut String| {
            let v = first_text(o, keys);
            if !v.is_empty() {
                *into = v;
            }
        };
        pick(&["registration", "tailNumber", "tail"], &mut out.registration);
        pick(&["msn", "serialNumber"], &mut out.msn);
        pick(&["selcal"], &mut out.selcal);
        pick(&["airline", "atcAirline", "operator"], &mut out.airline);
    }
    out.registration = out.registration.to_ascii_uppercase();
    out.selcal = out.selcal.to_ascii_uppercase();
    out
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

pub fn load(liveries_dir: &Path, folder: &str) -> LiveryIdentity {
    let dir = liveries_dir.join(folder);
    merge(folder, read_json(&dir.join("operator.json")).as_ref(), read_json(&dir.join("validation.json")).as_ref())
}

pub fn combine(settings: &crate::app_settings::AppSettings, livery: &LiveryIdentity) -> Effective {
    use crate::app_settings::effective;
    Effective {
        registration: effective(&settings.registration, &livery.registration),
        msn: effective(&settings.msn, &livery.msn),
        selcal: effective(&settings.selcal, &livery.selcal),
        airline: livery.airline.trim().to_string(),
    }
}

/// The text of the string simvars FlyByWire reads for the aircraft's identity.
pub fn string_for(name: &str, e: &Effective) -> Option<String> {
    match name.strip_prefix("A:").unwrap_or(name).trim().to_ascii_uppercase().as_str() {
        "ATC ID" => Some(e.registration.clone()),
        "ATC AIRLINE" => Some(e.airline.clone()),
        _ => None,
    }
}

struct State {
    livery: Option<String>,
    identity: LiveryIdentity,
    published: Option<LiveryIdentity>,
}

static STATE: Mutex<State> = Mutex::new(State { livery: None, identity: LiveryIdentity { registration: String::new(), msn: String::new(), selcal: String::new(), airline: String::new() }, published: None });

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn liveries_dir() -> Option<PathBuf> {
    crate::sound_pack::plugin_dir().and_then(|d| Some(d.parent()?.parent()?.join("liveries")))
}

/// The loaded livery changed (`None`: the aircraft's own paint).
pub fn livery_changed(name: Option<&str>) {
    let identity = match (name, liveries_dir()) {
        (Some(n), Some(dir)) => load(&dir, n),
        _ => LiveryIdentity::default(),
    };
    let mut s = state();
    s.livery = name.map(str::to_string);
    s.identity = identity;
}

pub fn current() -> Effective {
    let settings = crate::app_settings::current();
    combine(&settings, &state().identity)
}

/// `Some` for the string simvars the livery and the settings answer.
pub fn string_var(name: &str) -> Option<String> {
    if !matches!(name.strip_prefix("A:").unwrap_or(name).trim().to_ascii_uppercase().as_str(), "ATC ID" | "ATC AIRLINE") {
        return None;
    }
    string_for(name, &current())
}

/// Writes the livery's own values into xphfbw.json (`liveryRegistration`,
/// `liveryMsn`, `liverySelcal`) for the EFB to show, when they changed.
pub fn publish() {
    let identity = {
        let s = state();
        if s.published.as_ref() == Some(&s.identity) {
            return;
        }
        s.identity.clone()
    };
    let Some(root) = crate::xp::system_path() else { return };
    let path = crate::settings_files::app_settings_path(&root);
    let result = crate::app_settings::with_settings_lock(|| {
        let mut on_disk: Map<String, Value> = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        let mut changed = false;
        for (key, value) in [("liveryRegistration", &identity.registration), ("liveryMsn", &identity.msn), ("liverySelcal", &identity.selcal)] {
            if on_disk.get(key).and_then(Value::as_str) != Some(value.as_str()) {
                on_disk.insert(key.to_string(), Value::String(value.clone()));
                changed = true;
            }
        }
        if changed {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let partial = path.with_extension("json.part");
            std::fs::write(&partial, serde_json::to_string_pretty(&Value::Object(on_disk)).unwrap_or_default())?;
            std::fs::rename(&partial, &path)?;
        }
        Ok::<(), std::io::Error>(())
    });
    match result {
        Ok(()) => state().published = Some(identity),
        Err(e) => crate::log(&format!("livery identity: could not write xphfbw.json: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_settings::AppSettings;

    #[test]
    fn folder_names_give_the_registration_and_airline() {
        let a = from_folder_name("Emirates A6-EDY 4K");
        assert_eq!((a.registration.as_str(), a.airline.as_str()), ("A6-EDY", "Emirates"));
        let b = from_folder_name("British Airways G-XLEF");
        assert_eq!((b.registration.as_str(), b.airline.as_str()), ("G-XLEF", "British Airways"));
        let c = from_folder_name("ANA JA381A 4K");
        assert_eq!((c.registration.as_str(), c.airline.as_str()), ("JA381A", "ANA"));
        let d = from_folder_name("Lufthansa 100 Years");
        assert_eq!((d.registration.as_str(), d.airline.as_str()), ("", "Lufthansa 100 Years"));
        let e = from_folder_name("Singapore Airlines 9V-SKA");
        assert_eq!(e.registration, "9V-SKA");
    }

    #[test]
    fn operator_json_beats_validation_json_beats_the_folder_name() {
        let validation = serde_json::json!({ "registration": "a6-eet" });
        let operator = serde_json::json!({ "registration": "a6-eex", "msn": "0123", "selcal": "ab-cd", "airline": "Emirates" });
        let both = merge("Emirates A6-EDY", Some(&operator), Some(&validation));
        assert_eq!((both.registration.as_str(), both.msn.as_str(), both.selcal.as_str(), both.airline.as_str()), ("A6-EEX", "0123", "AB-CD", "Emirates"));
        let only_validation = merge("Emirates A6-EDY", None, Some(&validation));
        assert_eq!(only_validation.registration, "A6-EET");
        assert_eq!(merge("Emirates A6-EDY", None, None).registration, "A6-EDY");
    }

    #[test]
    fn the_pilots_settings_win_per_field() {
        let livery = LiveryIdentity { registration: "A6-EDY".into(), msn: "0123".into(), selcal: "ABCD".into(), airline: "Emirates".into() };
        let settings = AppSettings { registration: "G-TEST".into(), ..AppSettings::default() };
        let e = combine(&settings, &livery);
        assert_eq!((e.registration.as_str(), e.msn.as_str(), e.selcal.as_str(), e.airline.as_str()), ("G-TEST", "0123", "ABCD", "Emirates"));
        assert_eq!(combine(&AppSettings::default(), &livery).registration, "A6-EDY");
    }

    #[test]
    fn atc_id_and_airline_are_answered_and_nothing_else() {
        let e = Effective { registration: "A6-EDY".into(), airline: "Emirates".into(), ..Default::default() };
        assert_eq!(string_for("ATC ID", &e).as_deref(), Some("A6-EDY"));
        assert_eq!(string_for("A:ATC AIRLINE", &e).as_deref(), Some("Emirates"));
        assert_eq!(string_for("ATC FLIGHT NUMBER", &e), None);
    }
}

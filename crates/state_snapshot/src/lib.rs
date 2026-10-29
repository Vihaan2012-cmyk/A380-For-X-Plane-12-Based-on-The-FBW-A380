pub mod codec;
pub mod migrate;
pub mod phase;
pub mod restore;
pub mod secrets;
pub mod slots;
pub mod vars;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FORMAT: &str = "a380-xp12-state";
pub const CURRENT_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AircraftInfo {
    pub name: String,
    pub plugin_version: String,
    #[serde(default)]
    pub livery: String,
    #[serde(default)]
    pub engine_variant: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveReason {
    Manual,
    Autosave,
    Quit,
    Phase(phase::PhaseEvent),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub altitude_ft: Option<f64>,
    pub ias_kt: Option<f64>,
    pub on_ground: Option<bool>,
    pub flight_phase: Option<String>,
    pub origin: Option<String>,
    pub destination: Option<String>,
    pub flight_number: Option<String>,
    pub sim_zulu_s: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub format: String,
    pub version: u32,
    pub created_unix: u64,
    pub name: String,
    pub reason: SaveReason,
    pub aircraft: AircraftInfo,
    #[serde(default)]
    pub summary: Summary,
    pub sections: BTreeMap<String, SectionData>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SectionData {
    pub v: u32,
    pub data: Value,
}

pub trait Section {
    fn name(&self) -> &'static str;
    fn version(&self) -> u32;
    fn save(&self) -> Value;
    fn load(&mut self, version: u32, data: &Value) -> Result<(), String>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum Restored {
    Done,
    Missing,
    NewerThanReader { saved: u32, reader: u32 },
    Failed(String),
}

impl Snapshot {
    pub fn new(name: impl Into<String>, reason: SaveReason, aircraft: AircraftInfo) -> Self {
        Self {
            format: FORMAT.to_string(),
            version: CURRENT_VERSION,
            created_unix: now_unix(),
            name: name.into(),
            reason,
            aircraft,
            summary: Summary::default(),
            sections: BTreeMap::new(),
        }
    }

    pub fn put(&mut self, section: &dyn Section) {
        let mut data = section.save();
        secrets::scrub(&mut data);
        self.sections.insert(section.name().to_string(), SectionData { v: section.version(), data });
    }

    pub fn put_raw(&mut self, name: &str, version: u32, mut data: Value) {
        secrets::scrub(&mut data);
        self.sections.insert(name.to_string(), SectionData { v: version, data });
    }

    pub fn restore(&self, section: &mut dyn Section) -> Restored {
        let Some(saved) = self.sections.get(section.name()) else {
            return Restored::Missing;
        };
        if saved.v > section.version() {
            return Restored::NewerThanReader { saved: saved.v, reader: section.version() };
        }
        match section.load(saved.v, &saved.data) {
            Ok(()) => Restored::Done,
            Err(e) => Restored::Failed(e),
        }
    }

    pub fn restore_all(&self, sections: &mut [&mut dyn Section]) -> Vec<(&'static str, Restored)> {
        sections.iter_mut().map(|s| (s.name(), self.restore(&mut **s))).collect()
    }
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Counter {
        n: i64,
    }

    impl Section for Counter {
        fn name(&self) -> &'static str {
            "counter"
        }
        fn version(&self) -> u32 {
            2
        }
        fn save(&self) -> Value {
            serde_json::json!({ "n": self.n })
        }
        fn load(&mut self, version: u32, data: &Value) -> Result<(), String> {
            let n = data.get("n").and_then(Value::as_i64).ok_or("no n")?;
            self.n = if version < 2 { n * 10 } else { n };
            Ok(())
        }
    }

    #[test]
    fn a_section_round_trips_and_an_old_section_version_is_upgraded_by_its_owner() {
        let mut s = Snapshot::new("t", SaveReason::Manual, AircraftInfo::default());
        s.put(&Counter { n: 7 });
        let mut c = Counter { n: 0 };
        assert_eq!(s.restore(&mut c), Restored::Done);
        assert_eq!(c.n, 7);
        s.sections.get_mut("counter").unwrap().v = 1;
        assert_eq!(s.restore(&mut c), Restored::Done);
        assert_eq!(c.n, 70);
    }

    #[test]
    fn a_missing_or_newer_section_is_reported_not_applied() {
        let mut s = Snapshot::new("t", SaveReason::Manual, AircraftInfo::default());
        let mut c = Counter { n: 3 };
        assert_eq!(s.restore(&mut c), Restored::Missing);
        s.put_raw("counter", 9, serde_json::json!({ "n": 1 }));
        assert_eq!(s.restore(&mut c), Restored::NewerThanReader { saved: 9, reader: 2 });
        assert_eq!(c.n, 3);
    }

    #[test]
    fn a_failing_section_does_not_stop_the_others() {
        let mut s = Snapshot::new("t", SaveReason::Manual, AircraftInfo::default());
        s.put_raw("counter", 2, serde_json::json!({ "x": 1 }));
        let mut c = Counter { n: 1 };
        let results = s.restore_all(&mut [&mut c]);
        assert!(matches!(results[0].1, Restored::Failed(_)));
    }
}

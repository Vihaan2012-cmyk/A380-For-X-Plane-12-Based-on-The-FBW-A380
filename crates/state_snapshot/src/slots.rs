use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::codec::{self, Error};
use crate::phase::PhaseEvent;
use crate::{AircraftInfo, SaveReason, Snapshot, Summary};

pub const EXTENSION: &str = "a380state";
pub const DEFAULT_AUTOSAVE_SLOTS: u8 = 3;
pub const MAX_AUTOSAVE_SLOTS: u8 = 10;
pub const AUTOSAVE_INTERVALS_MIN: [u32; 6] = [0, 2, 5, 10, 15, 30];
pub const QUIT_SLOT: &str = "quit";
pub const MAX_NAME_CHARS: usize = 64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlotInfo {
    pub id: String,
    pub name: String,
    pub reason: Option<SaveReason>,
    pub created_unix: u64,
    pub bytes: u64,
    pub aircraft: Option<AircraftInfo>,
    pub summary: Option<Summary>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
struct Header {
    name: String,
    reason: SaveReason,
    created_unix: u64,
    aircraft: AircraftInfo,
    #[serde(default)]
    summary: Summary,
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 96 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

pub fn clean_name(name: &str) -> String {
    let n: String = name.chars().filter(|c| !c.is_control()).take(MAX_NAME_CHARS).collect();
    let n = n.trim();
    if n.is_empty() {
        "Saved state".to_string()
    } else {
        n.to_string()
    }
}

fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
        if out.len() >= 32 {
            break;
        }
    }
    out.trim_end_matches('-').to_string()
}

/// The EFB's save settings. `interval_min` 0 is off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutosaveSettings {
    pub interval_min: u32,
    pub rolling_slots: u8,
    pub phase_saves: bool,
    pub quit_save: bool,
}

impl Default for AutosaveSettings {
    fn default() -> Self {
        Self { interval_min: 5, rolling_slots: DEFAULT_AUTOSAVE_SLOTS, phase_saves: true, quit_save: true }
    }
}

impl AutosaveSettings {
    /// Snaps a hand-edited or out-of-range value to the nearest choice the
    /// EFB offers.
    pub fn normalized(self) -> Self {
        let interval_min = *AUTOSAVE_INTERVALS_MIN.iter().min_by_key(|&&c| (c as i64 - self.interval_min as i64).abs()).unwrap_or(&5);
        Self { interval_min, rolling_slots: self.rolling_slots.clamp(1, MAX_AUTOSAVE_SLOTS), ..self }
    }

    pub fn interval_s(&self) -> f64 {
        self.interval_min as f64 * 60.0
    }

    pub fn allows(&self, reason: SaveReason) -> bool {
        match reason {
            SaveReason::Manual => true,
            SaveReason::Autosave => self.interval_min > 0,
            SaveReason::Quit => self.quit_save,
            SaveReason::Phase(_) => self.phase_saves,
        }
    }
}

pub struct SlotManager {
    dir: PathBuf,
    settings: AutosaveSettings,
}

impl SlotManager {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self::with_settings(dir, AutosaveSettings::default())
    }

    pub fn with_settings(dir: impl Into<PathBuf>, settings: AutosaveSettings) -> Self {
        Self { dir: dir.into(), settings: settings.normalized() }
    }

    pub fn settings(&self) -> AutosaveSettings {
        self.settings
    }

    /// Fewer rolling slots than before deletes the surplus autosaves.
    pub fn set_settings(&mut self, settings: AutosaveSettings) -> usize {
        self.settings = settings.normalized();
        self.prune_autosaves()
    }

    pub fn prune_autosaves(&self) -> usize {
        (self.settings.rolling_slots as usize + 1..=MAX_AUTOSAVE_SLOTS as usize)
            .filter_map(|i| self.path_of(&format!("autosave-{i}")))
            .filter(|p| std::fs::remove_file(p).is_ok())
            .count()
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn path_of(&self, id: &str) -> Option<PathBuf> {
        valid_id(id).then(|| self.dir.join(format!("{id}.{EXTENSION}")))
    }

    /// Where `reason` writes: a new file per manual save, the oldest of the
    /// rolling autosaves, one fixed slot for quit and one per phase event.
    pub fn id_for(&self, reason: SaveReason, name: &str, created_unix: u64) -> String {
        match reason {
            SaveReason::Manual => {
                let base = match slug(name) {
                    s if s.is_empty() => format!("manual-{created_unix}"),
                    s => format!("manual-{created_unix}-{s}"),
                };
                let mut id = base.clone();
                let mut n = 2;
                while self.path_of(&id).is_some_and(|p| p.exists()) {
                    id = format!("{base}-{n}");
                    n += 1;
                }
                id
            }
            SaveReason::Autosave => self.next_autosave_id(),
            SaveReason::Quit => QUIT_SLOT.to_string(),
            SaveReason::Phase(e) => e.slot_id().to_string(),
        }
    }

    fn next_autosave_id(&self) -> String {
        let ids: Vec<String> = (1..=self.settings.rolling_slots as usize).map(|i| format!("autosave-{i}")).collect();
        let mut oldest: Option<(u64, &String)> = None;
        for id in &ids {
            let Some(path) = self.path_of(id) else { continue };
            if !path.exists() {
                return id.clone();
            }
            let created = self.header(&path).map(|h| h.created_unix).unwrap_or(0);
            if oldest.is_none_or(|(t, _)| created < t) {
                oldest = Some((created, id));
            }
        }
        oldest.map(|(_, id)| id.clone()).unwrap_or_else(|| ids[0].clone())
    }

    /// `Ok(None)` when the EFB settings turned this kind of save off.
    pub fn save_if_allowed(&self, snapshot: &mut Snapshot) -> Result<Option<SlotInfo>, Error> {
        if !self.settings.allows(snapshot.reason) {
            return Ok(None);
        }
        self.save(snapshot).map(Some)
    }

    pub fn save(&self, snapshot: &mut Snapshot) -> Result<SlotInfo, Error> {
        snapshot.name = clean_name(&snapshot.name);
        let id = self.id_for(snapshot.reason, &snapshot.name, snapshot.created_unix);
        let path = self.path_of(&id).ok_or_else(|| Error::WrongFormat(format!("bad slot id {id}")))?;
        let bytes = codec::save(&path, snapshot)?;
        Ok(SlotInfo {
            id,
            name: snapshot.name.clone(),
            reason: Some(snapshot.reason),
            created_unix: snapshot.created_unix,
            bytes,
            aircraft: Some(snapshot.aircraft.clone()),
            summary: Some(snapshot.summary.clone()),
            error: None,
        })
    }

    pub fn load(&self, id: &str) -> Result<Snapshot, Error> {
        let path = self.path_of(id).ok_or_else(|| Error::WrongFormat(format!("bad slot id {id}")))?;
        codec::load(&path)
    }

    pub fn delete(&self, id: &str) -> Result<bool, Error> {
        let path = self.path_of(id).ok_or_else(|| Error::WrongFormat(format!("bad slot id {id}")))?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(Error::Io(e)),
        }
    }

    pub fn rename(&self, id: &str, name: &str) -> Result<(), Error> {
        let path = self.path_of(id).ok_or_else(|| Error::WrongFormat(format!("bad slot id {id}")))?;
        let mut snapshot = codec::load(&path)?;
        snapshot.name = clean_name(name);
        codec::save(&path, &snapshot).map(|_| ())
    }

    fn header(&self, path: &Path) -> Result<Header, Error> {
        let value = codec::decode_value(&std::fs::read(path)?)?;
        serde_json::from_value(value).map_err(|e| Error::Corrupt(e.to_string()))
    }

    /// Newest first. Unreadable files are listed with `error` set so the
    /// EFB can offer to delete them.
    pub fn list(&self) -> Vec<SlotInfo> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return Vec::new() };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some(EXTENSION) {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()).filter(|s| valid_id(s)).map(str::to_string) else { continue };
            let bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
            out.push(match self.header(&path) {
                Ok(h) => SlotInfo { id, name: h.name, reason: Some(h.reason), created_unix: h.created_unix, bytes, aircraft: Some(h.aircraft), summary: Some(h.summary), error: None },
                Err(e) => SlotInfo { id: id.clone(), name: id, reason: None, created_unix: 0, bytes, aircraft: None, summary: None, error: Some(e.to_string()) },
            });
        }
        out.sort_by(|a, b| b.created_unix.cmp(&a.created_unix).then_with(|| a.id.cmp(&b.id)));
        out
    }

    pub fn phase_slot_ids() -> impl Iterator<Item = &'static str> {
        PhaseEvent::ALL.into_iter().map(PhaseEvent::slot_id)
    }

    /// Temp files a crash left behind.
    pub fn sweep_temp_files(&self) -> usize {
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return 0 };
        entries
            .flatten()
            .filter(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.starts_with('.') && n.ends_with(".tmp") && n.contains(&format!(".{EXTENSION}."))
            })
            .filter(|e| std::fs::remove_file(e.path()).is_ok())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("state_snapshot_slots_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn snap(name: &str, reason: SaveReason, t: u64) -> Snapshot {
        let mut s = Snapshot::new(name, reason, AircraftInfo { name: "A380 for X-Plane 12".into(), ..Default::default() });
        s.created_unix = t;
        s
    }

    #[test]
    fn autosaves_rotate_through_three_slots_overwriting_the_oldest() {
        let d = dir("rotate");
        let m = SlotManager::new(&d);
        let mut ids = Vec::new();
        for t in 1..=5u64 {
            ids.push(m.save(&mut snap("Autosave", SaveReason::Autosave, 100 + t)).unwrap().id);
        }
        assert_eq!(ids, ["autosave-1", "autosave-2", "autosave-3", "autosave-1", "autosave-2"]);
        let list = m.list();
        assert_eq!(list.len(), 3);
        assert_eq!(list.iter().map(|s| s.created_unix).collect::<Vec<_>>(), [105, 104, 103]);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn manual_saves_never_overwrite_and_quit_and_phase_saves_have_fixed_slots() {
        let d = dir("kinds");
        let m = SlotManager::new(&d);
        let a = m.save(&mut snap("EGLL stand 501", SaveReason::Manual, 10)).unwrap();
        let b = m.save(&mut snap("EGLL stand 501", SaveReason::Manual, 10)).unwrap();
        assert_eq!(a.id, "manual-10-egll-stand-501");
        assert_eq!(b.id, "manual-10-egll-stand-501-2");
        assert_eq!(m.save(&mut snap("q", SaveReason::Quit, 11)).unwrap().id, "quit");
        assert_eq!(m.save(&mut snap("q", SaveReason::Quit, 12)).unwrap().id, "quit");
        assert_eq!(m.save(&mut snap("tod", SaveReason::Phase(PhaseEvent::TopOfDescent), 13)).unwrap().id, "phase-tod");
        assert_eq!(m.list().len(), 4);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn rename_delete_and_load_work_by_id_and_reject_paths() {
        let d = dir("ops");
        let m = SlotManager::new(&d);
        let info = m.save(&mut snap("first", SaveReason::Manual, 5)).unwrap();
        m.rename(&info.id, "  Over the Alps\n ").unwrap();
        assert_eq!(m.load(&info.id).unwrap().name, "Over the Alps");
        assert!(m.load("../../secrets").is_err());
        assert!(m.delete("..\\x").is_err());
        assert!(m.delete(&info.id).unwrap());
        assert!(!m.delete(&info.id).unwrap());
        assert!(m.list().is_empty());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_corrupt_file_is_listed_with_an_error_and_can_be_deleted() {
        let d = dir("corrupt");
        let m = SlotManager::new(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("manual-1-bad.a380state"), b"not gzip").unwrap();
        std::fs::write(d.join(".quit.a380state.1.0.tmp"), b"half").unwrap();
        m.save(&mut snap("ok", SaveReason::Manual, 9)).unwrap();
        let list = m.list();
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|s| s.id == "manual-1-bad" && s.error.is_some()));
        assert_eq!(m.sweep_temp_files(), 1);
        assert!(m.delete("manual-1-bad").unwrap());
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn the_rolling_slot_count_follows_the_efb_setting_and_shrinking_it_prunes() {
        let d = dir("count");
        let mut m = SlotManager::with_settings(&d, AutosaveSettings { rolling_slots: 5, ..Default::default() });
        for t in 1..=7u64 {
            m.save(&mut snap("a", SaveReason::Autosave, t)).unwrap();
        }
        assert_eq!(m.list().len(), 5);
        assert_eq!(m.set_settings(AutosaveSettings { rolling_slots: 2, ..Default::default() }), 3);
        assert_eq!(m.list().len(), 2);
        assert_eq!(m.save(&mut snap("a", SaveReason::Autosave, 8)).unwrap().id, "autosave-1");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn settings_snap_to_the_efb_choices_and_gate_each_kind_of_save() {
        let s = AutosaveSettings { interval_min: 7, rolling_slots: 40, phase_saves: false, quit_save: false }.normalized();
        assert_eq!((s.interval_min, s.rolling_slots), (5, 10));
        assert_eq!(AutosaveSettings { rolling_slots: 0, interval_min: 1, ..Default::default() }.normalized().rolling_slots, 1);
        assert_eq!(AutosaveSettings::default(), AutosaveSettings { interval_min: 5, rolling_slots: 3, phase_saves: true, quit_save: true });
        assert!(!s.allows(SaveReason::Quit) && !s.allows(SaveReason::Phase(PhaseEvent::Gate)) && s.allows(SaveReason::Manual));
        let off = AutosaveSettings { interval_min: 0, ..Default::default() }.normalized();
        assert!(!off.allows(SaveReason::Autosave));
        let d = dir("gated");
        let m = SlotManager::with_settings(&d, s);
        assert_eq!(m.save_if_allowed(&mut snap("q", SaveReason::Quit, 1)).unwrap(), None);
        assert!(m.list().is_empty());
        let parsed: AutosaveSettings = serde_json::from_str(r#"{"interval_min":10}"#).unwrap();
        assert_eq!(parsed, AutosaveSettings { interval_min: 10, ..Default::default() });
    }

    #[test]
    fn a_missing_directory_lists_nothing() {
        assert!(SlotManager::new(dir("missing")).list().is_empty());
    }
}

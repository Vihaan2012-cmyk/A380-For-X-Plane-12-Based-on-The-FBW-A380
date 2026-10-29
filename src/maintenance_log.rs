use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Mutex, OnceLock};

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::xp::{DataRef, Xplm};
use crate::Vars;

const LOG_CAPACITY: usize = 64;
const HISTORY_CAPACITY: usize = 1024;
const LEGS_KEPT: usize = 10;

const KIND_FAILURE_ARMED: f64 = 1.;
const KIND_FAILURE_CLEARED: f64 = 2.;
const KIND_DERIVED_ACTIVE: f64 = 3.;
const KIND_DERIVED_CLEARED: f64 = 4.;
const KIND_BREAKER_TRIPPED: f64 = 5.;
const KIND_BREAKER_RESET: f64 = 6.;
const KIND_MEL_DEFERRED: f64 = 7.;
const KIND_MEL_RELEASED: f64 = 8.;

const CMD_OPEN: i64 = 1;
const CMD_CLOSE: i64 = 2;

#[derive(Clone, Copy, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Event {
    kind: f64,
    id: f64,
    time_s: f64,
    phase: f64,
    ata: f64,
    #[serde(default)]
    class: u8,
    #[serde(default)]
    date: u32,
    #[serde(default)]
    leg: u32,
    #[serde(default)]
    count: u32,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Leg {
    pub id: u32,
    pub open_date: u32,
    pub open_zulu_s: f64,
    pub close_date: Option<u32>,
    pub close_zulu_s: Option<f64>,
}

/// The CMS history kept in airframe persistence.
#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedCms {
    #[serde(default)]
    pub events: Vec<Event>,
    #[serde(default)]
    pub legs: Vec<Leg>,
    #[serde(default)]
    pub next_leg: u32,
    #[serde(default)]
    pub open_leg: Option<u32>,
}

static SEED: Mutex<Option<SavedCms>> = Mutex::new(None);
static LATEST: Mutex<Option<SavedCms>> = Mutex::new(None);

/// Hands a persisted history to the log built next.
pub fn seed(saved: SavedCms) {
    if let Ok(mut g) = SEED.lock() {
        *g = Some(saved.clone());
    }
    if let Ok(mut g) = LATEST.lock() {
        *g = Some(saved);
    }
}

pub fn saved() -> SavedCms {
    LATEST.lock().ok().and_then(|g| g.clone()).unwrap_or_default()
}

/// "New airframe": forget the history.
pub fn clear() {
    if let Ok(mut g) = LATEST.lock() {
        *g = Some(SavedCms::default());
    }
    if let Ok(mut g) = SEED.lock() {
        *g = Some(SavedCms::default());
    }
    CLEAR.store(true, std::sync::atomic::Ordering::Relaxed);
}

static CLEAR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn fault_key(kind: f64, id: f64) -> (u8, u64) {
    let group = match kind as u8 {
        1 | 2 => 0,
        3 | 4 => 1,
        5 | 6 => 2,
        _ => 3,
    };
    (group, id as u64)
}

fn is_onset(kind: f64) -> bool {
    matches!(kind as u8, 1 | 3 | 5 | 7)
}

/// CMS fault class, GENERIC mapping: 1 = flight deck effect (failures the
/// random-failure severity puts at caution or above), 2 = no flight deck
/// effect (nuisance failures, breaker trips), 3 = maintenance record (MEL).
fn class_of(kind: f64, id: f64) -> u8 {
    use crate::user_prefs::FailureSeverity;
    match kind as u8 {
        1..=4 => {
            static SEVERITY: OnceLock<HashMap<u64, FailureSeverity>> = OnceLock::new();
            let map = SEVERITY.get_or_init(|| crate::failures::extra::extra_failures().into_iter().map(|x| (x.id, crate::random_failures::severity_class(x.id, &x.name, x.mel))).collect());
            let id = id as u64;
            let sev = map.get(&id).copied().unwrap_or_else(|| crate::random_failures::severity_class(id, &crate::failures::failure_name(id), None));
            if sev == FailureSeverity::None {
                2
            } else {
                1
            }
        }
        5 | 6 => 2,
        _ => 3,
    }
}

/// `YYYYMMDD` from a year and a 0-based day of that year.
fn ymd(year: i64, day_of_year: i64) -> u32 {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let months = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut d = day_of_year.rem_euclid(if leap { 366 } else { 365 });
    let mut m = 0;
    while d >= months[m] {
        d -= months[m];
        m += 1;
    }
    (year * 10_000 + (m as i64 + 1) * 100 + d + 1) as u32
}

/// The UTC date: X-Plane's local day of year moved across midnight by the
/// zulu/local offset; the year is the host clock's.
fn utc_date(local_day: i64, local_s: f64, zulu_s: f64) -> u32 {
    let shift = if zulu_s < local_s - 43_200.0 {
        1
    } else if zulu_s > local_s + 43_200.0 {
        -1
    } else {
        0
    };
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let year = 1970 + (secs as f64 / 31_556_952.0).floor() as i64;
    ymd(year, local_day + shift)
}

struct Slot {
    kind: VariableIdentifier,
    id: VariableIdentifier,
    time: VariableIdentifier,
    phase: VariableIdentifier,
    ata: VariableIdentifier,
}

struct Breaker {
    id: &'static str,
    ata: u16,
    status: VariableIdentifier,
    cmd: VariableIdentifier,
}

struct MelItem {
    failures: &'static [u64],
    deferred: VariableIdentifier,
    remaining_hours: VariableIdentifier,
}

#[derive(Default)]
struct Log {
    events: Vec<Event>,
    seq: u64,
    legs: Vec<Leg>,
    next_leg: u32,
    open_leg: Option<u32>,
    date: u32,
    primed: bool,
    prev_armed: BTreeSet<u64>,
    prev_derived: BTreeSet<u64>,
    prev_open: Vec<bool>,
    prev_mel: BTreeSet<usize>,
}

impl Log {
    fn from_saved(s: SavedCms) -> Self {
        let primed = !s.events.is_empty() || !s.legs.is_empty();
        Self { events: s.events, legs: s.legs, next_leg: s.next_leg, open_leg: s.open_leg, primed, ..Self::default() }
    }

    fn saved(&self) -> SavedCms {
        SavedCms { events: self.events.clone(), legs: self.legs.clone(), next_leg: self.next_leg, open_leg: self.open_leg }
    }

    fn push(&mut self, kind: f64, id: f64, time_s: f64, phase: f64, ata: u16) {
        let leg = self.open_leg.unwrap_or(0);
        let key = fault_key(kind, id);
        let count = if is_onset(kind) { 1 + self.events.iter().filter(|e| e.leg == leg && is_onset(e.kind) && fault_key(e.kind, e.id) == key).count() as u32 } else { 0 };
        self.events.insert(0, Event { kind, id, time_s, phase, ata: ata as f64, class: class_of(kind, id), date: self.date, leg, count });
        self.events.truncate(HISTORY_CAPACITY);
        self.seq += 1;
    }

    /// A leg opens at the first engine start and closes when the last
    /// engine stops.
    fn update_leg(&mut self, any_engine_running: bool, date: u32, zulu_s: f64) -> bool {
        self.date = date;
        match (any_engine_running, self.open_leg) {
            (true, None) => {
                self.next_leg += 1;
                self.open_leg = Some(self.next_leg);
                self.legs.push(Leg { id: self.next_leg, open_date: date, open_zulu_s: zulu_s, close_date: None, close_zulu_s: None });
                if self.legs.len() > LEGS_KEPT {
                    let drop = self.legs.len() - LEGS_KEPT;
                    self.legs.drain(..drop);
                    let oldest = self.legs[0].id;
                    self.events.retain(|e| e.leg == 0 || e.leg >= oldest);
                }
                true
            }
            (false, Some(id)) => {
                if let Some(l) = self.legs.iter_mut().find(|l| l.id == id) {
                    l.close_date = Some(date);
                    l.close_zulu_s = Some(zulu_s);
                }
                self.open_leg = None;
                true
            }
            _ => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn update(
        &mut self,
        time_s: f64,
        phase: f64,
        armed: &BTreeSet<u64>,
        derived: &BTreeSet<u64>,
        breakers: &[(bool, bool, u16)],
        mel: &BTreeSet<usize>,
        ata_of: impl Fn(u64) -> u16,
    ) {
        if self.prev_open.len() != breakers.len() {
            self.prev_open = breakers.iter().map(|b| b.0).collect();
        }
        if self.primed {
            self.primed = false;
            self.prev_armed = armed.clone();
            self.prev_derived = derived.clone();
            self.prev_mel = mel.clone();
            return;
        }
        for &id in armed.difference(&self.prev_armed.clone()) {
            self.push(KIND_FAILURE_ARMED, id as f64, time_s, phase, ata_of(id));
        }
        for &id in self.prev_armed.clone().difference(armed) {
            self.push(KIND_FAILURE_CLEARED, id as f64, time_s, phase, ata_of(id));
        }
        self.prev_armed = armed.clone();
        for &id in derived.difference(&self.prev_derived.clone()) {
            self.push(KIND_DERIVED_ACTIVE, id as f64, time_s, phase, ata_of(id));
        }
        for &id in self.prev_derived.clone().difference(derived) {
            self.push(KIND_DERIVED_CLEARED, id as f64, time_s, phase, ata_of(id));
        }
        self.prev_derived = derived.clone();
        for (i, &(open, commanded, ata)) in breakers.iter().enumerate() {
            let was_open = self.prev_open[i];
            if open && !was_open && !commanded {
                self.push(KIND_BREAKER_TRIPPED, i as f64, time_s, phase, ata);
            } else if !open && was_open {
                self.push(KIND_BREAKER_RESET, i as f64, time_s, phase, ata);
            }
            self.prev_open[i] = open;
        }
        for &item in mel.difference(&self.prev_mel.clone()) {
            self.push(KIND_MEL_DEFERRED, item as f64, time_s, phase, 0);
        }
        for &item in self.prev_mel.clone().difference(mel) {
            self.push(KIND_MEL_RELEASED, item as f64, time_s, phase, 0);
        }
        self.prev_mel = mel.clone();
    }

    fn active_by_ata(&self, ata_of: impl Fn(u64) -> u16) -> BTreeMap<u16, u32> {
        let mut counts = BTreeMap::new();
        for &id in self.prev_armed.iter().chain(&self.prev_derived) {
            let ata = ata_of(id);
            if ata != 0 {
                *counts.entry(ata).or_insert(0) += 1;
            }
        }
        counts
    }
}

pub struct MaintenanceLog {
    log: Log,
    slots: Vec<Slot>,
    count: VariableIdentifier,
    seq: VariableIdentifier,
    breakers: Vec<Breaker>,
    mel: Vec<MelItem>,
    ata_active: BTreeMap<u16, VariableIdentifier>,
    phase: VariableIdentifier,
    zulu: Option<DataRef>,
    local_time: Option<DataRef>,
    local_day: Option<DataRef>,
    engines_running: Option<DataRef>,
}

fn ata_of(id: u64) -> u16 {
    crate::study::failures::ata_of(id) as u16
}

impl MaintenanceLog {
    pub fn new(vars: &mut Vars, xplm: &Xplm) -> Self {
        let slots = (0..LOG_CAPACITY)
            .map(|k| Slot {
                kind: vars.get(format!("A32NX_DEEP_MAINT_LOG_{k}_KIND")),
                id: vars.get(format!("A32NX_DEEP_MAINT_LOG_{k}_ID")),
                time: vars.get(format!("A32NX_DEEP_MAINT_LOG_{k}_TIME")),
                phase: vars.get(format!("A32NX_DEEP_MAINT_LOG_{k}_PHASE")),
                ata: vars.get(format!("A32NX_DEEP_MAINT_LOG_{k}_ATA")),
            })
            .collect();
        let mut defs: Vec<_> = crate::deep::breakers::catalog::all().iter().collect();
        defs.sort_by_key(|d| d.id);
        let breakers = defs
            .into_iter()
            .map(|d| Breaker {
                id: d.id,
                ata: d.ata,
                status: vars.get(format!("A32NX_BKR_{}_STATUS", d.id)),
                cmd: vars.get(format!("A32NX_BKR_{}_CMD", d.id)),
            })
            .collect::<Vec<_>>();
        let mut items: Vec<(&str, &'static [u64])> = crate::mel_catalog::MEL_FAILURES.to_vec();
        items.sort_by_key(|(r, _)| *r);
        let mel = items
            .into_iter()
            .enumerate()
            .map(|(k, (_, failures))| MelItem {
                failures,
                deferred: vars.get(format!("A32NX_DEEP_MEL_ITEM_{k}_DEFERRED")),
                remaining_hours: vars.get(format!("A32NX_DEEP_MEL_ITEM_{k}_REMAINING_HOURS")),
            })
            .collect();
        let mut chapters: BTreeSet<u16> = crate::failures::all_ids().into_iter().map(ata_of).collect();
        chapters.extend(breakers.iter().map(|b| b.ata));
        let ata_active = chapters
            .into_iter()
            .filter(|&a| a != 0)
            .map(|a| (a, vars.get(format!("A32NX_DEEP_MAINT_ATA_{a:02}_ACTIVE"))))
            .collect();
        let seeded = SEED.lock().ok().and_then(|mut g| g.take()).unwrap_or_default();
        Self {
            log: Log::from_saved(seeded),
            slots,
            count: vars.get("A32NX_DEEP_MAINT_LOG_COUNT".to_string()),
            seq: vars.get("A32NX_DEEP_MAINT_LOG_SEQ".to_string()),
            breakers,
            mel,
            ata_active,
            phase: vars.get("A32NX_FMGC_FLIGHT_PHASE".to_string()),
            zulu: xplm.find("sim/time/zulu_time_sec"),
            local_time: xplm.find("sim/time/local_time_sec"),
            local_day: xplm.find("sim/time/local_date_days"),
            engines_running: xplm.find("sim/flightmodel/engine/ENGN_running"),
        }
    }

    pub fn update(&mut self, vars: &mut Vars, xplm: &Xplm) {
        for b in &self.breakers {
            match vars.read(&b.cmd).round() as i64 {
                CMD_OPEN => crate::deep::breakers::live::request_pull(b.id),
                CMD_CLOSE => crate::deep::breakers::live::request_reset(b.id),
                0 => continue,
                _ => {}
            }
            vars.write(&b.cmd, 0.);
        }

        let t0 = std::time::Instant::now();
        let armed: BTreeSet<u64> = crate::failures::armed_ids().into_iter().collect();
        let derived: BTreeSet<u64> = crate::failures::active_ids().into_iter().filter(|id| !armed.contains(id)).collect();
        let breakers: Vec<(bool, bool, u16)> = self
            .breakers
            .iter()
            .map(|b| {
                let status = vars.read(&b.status).round() as i64;
                (status != 0, status == 1, b.ata)
            })
            .collect();
        let t1 = std::time::Instant::now();
        let deferred = crate::mel::deferred_remaining();
        let mut mel_deferred = BTreeSet::new();
        for (k, item) in self.mel.iter().enumerate() {
            let remaining = item.failures.iter().filter_map(|id| deferred.get(id).copied()).reduce(f64::min);
            if remaining.is_some() {
                mel_deferred.insert(k);
            }
            vars.write(&item.deferred, if remaining.is_some() { 1. } else { 0. });
            vars.write(&item.remaining_hours, remaining.unwrap_or(0.));
        }
        let time_s = self.zulu.map_or(0., |d| xplm.get_f(d) as f64);
        let phase = vars.read(&self.phase);
        if CLEAR.swap(false, std::sync::atomic::Ordering::Relaxed) {
            self.log = Log { primed: true, ..Log::default() };
        }
        let local_s = self.local_time.map_or(time_s, |d| xplm.get_f(d) as f64);
        let local_day = self.local_day.map_or(0, |d| xplm.get_i(d) as i64);
        let mut running = [0 as std::ffi::c_int; 4];
        if let Some(d) = self.engines_running {
            xplm.get_vi(d, &mut running);
        }
        let t2 = std::time::Instant::now();
        let seq_before = self.log.seq;
        let leg_changed = self.log.update_leg(running.iter().any(|&r| r != 0), utc_date(local_day, local_s, time_s), time_s);
        self.log.update(time_s, phase, &armed, &derived, &breakers, &mel_deferred, ata_of);
        if leg_changed || self.log.seq != seq_before {
            if let Ok(mut g) = LATEST.lock() {
                *g = Some(self.log.saved());
            }
        }

        vars.write(&self.count, self.log.events.len().min(LOG_CAPACITY) as f64);
        vars.write(&self.seq, self.log.seq as f64);
        for (k, slot) in self.slots.iter().enumerate() {
            let e = self.log.events.get(k).copied().unwrap_or_default();
            vars.write(&slot.kind, e.kind);
            vars.write(&slot.id, e.id);
            vars.write(&slot.time, e.time_s);
            vars.write(&slot.phase, e.phase);
            vars.write(&slot.ata, e.ata);
        }
        let counts = self.log.active_by_ata(ata_of);
        for (ata, id) in &self.ata_active {
            vars.write(id, counts.get(ata).copied().unwrap_or(0) as f64);
        }
        let total = t0.elapsed();
        if total.as_millis() >= 20 {
            crate::log(&format!(
                "maintenance log: slow frame {:.1} ms (failure sets {:.1}, MEL {:.1}, log {:.1}); {} armed, {} derived, {} events, {} pushed",
                total.as_secs_f64() * 1e3,
                (t1 - t0).as_secs_f64() * 1e3,
                (t2 - t1).as_secs_f64() * 1e3,
                t2.elapsed().as_secs_f64() * 1e3,
                armed.len(),
                derived.len(),
                self.log.events.len(),
                self.log.seq - seq_before
            ));
        }
    }
}

fn phase_label(phase: f64) -> &'static str {
    match phase.round() as i64 {
        0 => "PREFLT",
        1 => "TO",
        2 => "CLB",
        3 => "CRZ",
        4 => "DES",
        5 => "APPR",
        6 => "GA",
        7 => "DONE",
        _ => "",
    }
}

fn message(e: &Event) -> String {
    let id = e.id as u64;
    let text = match e.kind as u8 {
        1..=4 => crate::failures::failure_name(id),
        5 | 6 => {
            let mut defs: Vec<_> = crate::deep::breakers::catalog::all().iter().collect();
            defs.sort_by_key(|d| d.id);
            defs.get(id as usize).map_or_else(|| format!("C/B {id}"), |d| format!("C/B {} {}", d.id, d.name))
        }
        _ => {
            let mut items: Vec<&str> = crate::mel_catalog::MEL_FAILURES.iter().map(|(r, _)| *r).collect();
            items.sort();
            items.get(id as usize).map_or_else(|| format!("MEL {id}"), |r| format!("MEL {r}"))
        }
    };
    let repeat = if e.count > 1 { format!(" (x{})", e.count) } else { String::new() };
    format!("CL{} {}{}", e.class, text, repeat)
}

fn event_json(e: &Event) -> serde_json::Value {
    serde_json::json!({
        "kind": e.kind as u32,
        "id": e.id as i64,
        "zulu_s": e.time_s,
        "phase": phase_label(e.phase),
        "ata": e.ata as u32,
        "message": message(e),
        "class": e.class,
        "date": e.date,
        "leg": e.leg,
        "count": e.count,
    })
}

/// The leg a report covers: the open leg for the current-flight report,
/// the latest closed leg for the post-flight report.
fn report_leg(s: &SavedCms, post: bool) -> Option<u32> {
    if post {
        s.legs.iter().rev().find(|l| l.close_date.is_some()).map(|l| l.id)
    } else {
        s.open_leg
    }
}

/// `oit_pages::avncs::MaintenanceModel` JSON for the NSS AVNCS maintenance
/// pages, built from the CMS history.
pub fn avncs_model(uri: &str, ata: Option<u32>) -> serde_json::Value {
    avncs_model_from(&saved(), uri, ata)
}

fn avncs_model_from(s: &SavedCms, uri: &str, ata: Option<u32>) -> serde_json::Value {
    let content = if uri.ends_with("current-flight-report") || uri.ends_with("post-flight-report") {
        let post = uri.ends_with("post-flight-report");
        let leg = report_leg(s, post);
        let events: Vec<_> = s.events.iter().filter(|e| Some(e.leg) == leg).map(event_json).collect();
        let ata_name = ata.map_or(String::new(), |a| crate::study::chapter_name(a as u64).to_string());
        serde_json::json!({ "kind": "report", "post": post, "ata": ata, "ata_name": ata_name, "events": events })
    } else if uri.ends_with("system-status") {
        let mut open: BTreeMap<(u8, u64), &Event> = BTreeMap::new();
        for e in s.events.iter().rev() {
            if matches!(e.kind as u8, 1..=6) {
                if is_onset(e.kind) {
                    open.insert(fault_key(e.kind, e.id), e);
                } else {
                    open.remove(&fault_key(e.kind, e.id));
                }
            }
        }
        let mut by_ata: BTreeMap<u32, u32> = BTreeMap::new();
        for e in open.values() {
            *by_ata.entry(e.ata as u32).or_insert(0) += 1;
        }
        let chapters: Vec<_> = by_ata.into_iter().map(|(a, n)| serde_json::json!({ "name": crate::study::chapter_name(a as u64), "count": n })).collect();
        serde_json::json!({ "kind": "system_status", "chapters": chapters })
    } else {
        serde_json::json!({ "kind": "none" })
    };
    serde_json::json!({ "uri": uri, "content": content })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legs_open_and_close_with_the_engines_and_repeats_are_counted() {
        let mut log = Log::default();
        let none = BTreeSet::new();
        assert!(!log.update_leg(false, 20261022, 100.));
        assert!(log.update_leg(true, 20261022, 200.));
        log.update(210., 1., &BTreeSet::from([21001]), &none, &[], &BTreeSet::new(), ata);
        log.update(220., 2., &none, &none, &[], &BTreeSet::new(), ata);
        log.update(230., 3., &BTreeSet::from([21001]), &none, &[], &BTreeSet::new(), ata);
        assert_eq!(log.events[0].count, 2);
        assert_eq!(log.events[0].leg, 1);
        assert_eq!(log.events[0].date, 20261022);
        assert!(log.update_leg(false, 20261023, 300.));
        assert_eq!(log.legs[0].close_date, Some(20261023));
        let s = log.saved();
        let post = avncs_model_from(&s, "nss-avncs/maintenance/post-flight-report", None);
        assert_eq!(post["content"]["kind"], "report");
        assert_eq!(post["content"]["events"].as_array().unwrap().len(), 3);
        assert_eq!(post["content"]["events"][0]["phase"], "CRZ");
        let current = avncs_model_from(&s, "nss-avncs/maintenance/current-flight-report", None);
        assert_eq!(current["content"]["events"].as_array().unwrap().len(), 0);
        let status = avncs_model_from(&s, "nss-avncs/maintenance/system-status", None);
        assert_eq!(status["content"]["chapters"][0]["count"], 1);
    }

    #[test]
    fn only_the_last_ten_legs_are_kept_and_the_history_round_trips() {
        let mut log = Log::default();
        for leg in 0..12 {
            log.update_leg(true, 20261001 + leg, leg as f64);
            log.update(leg as f64, 3., &BTreeSet::from([21001]), &BTreeSet::new(), &[], &BTreeSet::new(), ata);
            log.update(leg as f64, 3., &BTreeSet::new(), &BTreeSet::new(), &[], &BTreeSet::new(), ata);
            log.update_leg(false, 20261001 + leg, leg as f64);
        }
        assert_eq!(log.legs.len(), LEGS_KEPT);
        assert!(log.events.iter().all(|e| e.leg >= 3));
        let s: SavedCms = serde_json::from_str(&serde_json::to_string(&log.saved()).unwrap()).unwrap();
        let mut restored = Log::from_saved(s.clone());
        assert_eq!(restored.saved(), s);
        restored.update(0., 0., &BTreeSet::from([21001]), &BTreeSet::new(), &[], &BTreeSet::new(), ata);
        assert_eq!(restored.events.len(), s.events.len(), "failures restored with the history are not logged twice");
    }

    #[test]
    fn dates_and_classes() {
        assert_eq!(ymd(2026, 0), 20260101);
        assert_eq!(ymd(2026, 294), 20261022);
        assert_eq!(ymd(2024, 59), 20240229);
        assert_eq!(class_of(7., 0.), 3);
        assert_eq!(class_of(5., 0.), 2);
    }

    fn ata(id: u64) -> u16 {
        (id / 1000) as u16
    }

    #[test]
    fn the_log_records_failures_breakers_and_mel_newest_first() {
        let mut log = Log::default();
        let none = BTreeSet::new();
        log.update(10., 2., &none, &none, &[(false, false, 24)], &BTreeSet::new(), ata);
        assert!(log.events.is_empty(), "a quiet aircraft logs nothing");
        log.update(20., 2., &BTreeSet::from([21001]), &none, &[(true, false, 24)], &BTreeSet::new(), ata);
        log.update(30., 3., &none, &BTreeSet::from([29000]), &[(false, false, 24)], &BTreeSet::from([5]), ata);
        let kinds: Vec<f64> = log.events.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, vec![KIND_MEL_DEFERRED, KIND_BREAKER_RESET, KIND_DERIVED_ACTIVE, KIND_FAILURE_CLEARED, KIND_BREAKER_TRIPPED, KIND_FAILURE_ARMED]);
        assert_eq!(log.seq, 6);
        assert_eq!(log.events[3].ata, 21.);
        assert_eq!(log.active_by_ata(ata).get(&29), Some(&1));
    }

    #[test]
    fn a_crew_opened_breaker_is_not_logged_as_a_trip() {
        let mut log = Log::default();
        let none = BTreeSet::new();
        log.update(1., 0., &none, &none, &[(false, false, 24)], &BTreeSet::new(), ata);
        log.update(2., 0., &none, &none, &[(true, true, 24)], &BTreeSet::new(), ata);
        assert!(log.events.is_empty());
    }

    #[test]
    fn the_log_keeps_every_event_of_the_kept_legs() {
        let mut log = Log::default();
        let none = BTreeSet::new();
        for i in 0..100u64 {
            let armed = if i % 2 == 0 { BTreeSet::from([21001]) } else { BTreeSet::new() };
            log.update(i as f64, 0., &armed, &none, &[], &BTreeSet::new(), ata);
        }
        assert_eq!(log.events.len(), 100);
        assert_eq!(log.seq, 100);
        assert_eq!(log.events[0].time_s, 99.);
    }
}

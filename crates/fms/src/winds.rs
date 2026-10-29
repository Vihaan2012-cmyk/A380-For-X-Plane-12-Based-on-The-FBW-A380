use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::plan::FlightPlan;

pub const NUM_CRUISE_WIND_LEVELS: usize = 4;
pub const NUM_CLIMB_WIND_LEVELS: usize = 5;
pub const NUM_DESCENT_WIND_LEVELS: usize = 5;
pub const SORT_CLIMB_WIND_DESCENDING: bool = true;
pub const GROUND_WIND_BAND_FT: f64 = 400.;
pub const DRAFT_ON_WIND_EDIT: bool = true;
pub const DIR_TO_ABEAM_POINT_IS_TO_WPT: bool = true;

pub const WIND_ENTRY_FLAG_INSERTED_FROM_HISTORY: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WindVector {
    pub magnitude: Option<f64>,
    pub direction: Option<f64>,
}

impl WindVector {
    pub fn is_complete(&self) -> bool {
        self.magnitude.is_some() && self.direction.is_some()
    }

    pub fn is_partly_filled(&self) -> bool {
        self.magnitude.is_some() != self.direction.is_some()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WindEntry {
    pub vector: WindVector,
    pub altitude: Option<f64>,
    #[serde(default)]
    pub flags: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindData {
    pub climb: Vec<WindEntry>,
    pub descent: Vec<WindEntry>,
    pub alternate: WindVector,
    pub approach_wind_direction: Option<f64>,
    pub approach_wind_magnitude: Option<f64>,
    pub is_approach_wind_pilot_entered: bool,
}

fn sort_by_altitude(entries: &mut [WindEntry], ascending: bool) {
    let mut sorted: Vec<WindEntry> = entries.iter().filter(|e| e.altitude.is_some()).copied().collect();
    sorted.sort_by(|a, b| {
        let (a, b) = (a.altitude.unwrap_or(0.), b.altitude.unwrap_or(0.));
        if ascending {
            a.total_cmp(&b)
        } else {
            b.total_cmp(&a)
        }
    });
    let mut next = sorted.into_iter();
    for e in entries.iter_mut() {
        if e.altitude.is_some() {
            if let Some(s) = next.next() {
                *e = s;
            }
        }
    }
}

fn set_clb_des_entry(entries: &mut Vec<WindEntry>, altitude: Option<f64>, entry: Option<WindEntry>, existing: Option<usize>, max_levels: usize, ascending: bool) {
    let mut sort = false;
    match entry {
        None => {
            let Some(i) = existing.filter(|i| *i < entries.len()) else { return };
            entries.remove(i);
            sort = true;
        }
        Some(mut entry) => {
            if let Some(i) = existing {
                let Some(old) = entries.get(i).copied() else { return };
                if (old.altitude.is_some() && entry.altitude.is_none()) || (old.vector.is_complete() && !entry.vector.is_complete()) {
                    entry.vector = WindVector::default();
                    entry.altitude = None;
                }
                entries[i] = entry;
            } else if entries.len() >= max_levels {
                // FBW's `altitude ?? 0 <= last` parses as `altitude ?? (0 <= last)`, so any defined altitude replaces the last level.
                let replace_last = match altitude {
                    Some(a) => a != 0.,
                    None => entries.last().and_then(|e| e.altitude).unwrap_or(0.) >= 0.,
                };
                if replace_last {
                    if let Some(last) = entries.last_mut() {
                        *last = entry;
                    }
                    sort = true;
                }
            } else {
                entries.push(entry);
                sort = true;
            }
        }
    }
    if sort {
        sort_by_altitude(entries, ascending);
    }
}

fn level(altitude: f64) -> f64 {
    (altitude / 100. + 0.5).floor()
}

fn same_level(a: Option<f64>, b: Option<f64>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if level(a) == level(b))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DraftWinds {
    pub climb: Option<Vec<WindEntry>>,
    pub descent: Option<Vec<WindEntry>>,
    pub cruise: Option<BTreeMap<usize, Vec<WindEntry>>>,
    pub alternate: Option<WindVector>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropagationType {
    Forward,
    Entry,
    Backward,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PropagatedWindEntry {
    pub altitude: Option<f64>,
    pub vector: WindVector,
    pub kind: PropagationType,
    pub source_leg_index: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PendingCruiseFix {
    Waypoint(String),
    LatLon { lat: f64, long: f64 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PendingCruiseWind {
    pub fix: PendingCruiseFix,
    pub levels: Vec<WindEntry>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PendingWindUplinkState {
    #[default]
    Idle,
    Requested,
    ReadyToInsert,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PendingWindUplink {
    pub climb_winds: Option<Vec<WindEntry>>,
    pub cruise_winds: Option<Vec<PendingCruiseWind>>,
    pub descent_winds: Option<Vec<WindEntry>>,
    pub alternate_wind: Option<WindEntry>,
    pub state: PendingWindUplinkState,
}

impl PendingWindUplink {
    pub fn on_uplink_requested(&mut self) {
        self.state = PendingWindUplinkState::Requested;
    }

    pub fn on_uplink_inserted(&mut self) {
        self.state = PendingWindUplinkState::Idle;
    }

    pub fn on_uplink_ready_to_insert(&mut self) {
        self.state = PendingWindUplinkState::ReadyToInsert;
    }

    pub fn delete(&mut self) {
        self.on_uplink_inserted();
    }

    pub fn on_uplink_aborted(&mut self) {
        self.on_uplink_inserted();
    }

    pub fn is_wind_uplink_in_progress(&self) -> bool {
        self.state == PendingWindUplinkState::Requested
    }

    pub fn is_wind_uplink_ready_to_insert(&self) -> bool {
        self.state == PendingWindUplinkState::ReadyToInsert
    }

    pub fn all_cruise_wind_altitudes(&self) -> Vec<Option<f64>> {
        let mut levels = Vec::new();
        for fix in self.cruise_winds.iter().flatten() {
            for l in &fix.levels {
                if !levels.contains(&l.altitude) {
                    levels.push(l.altitude);
                }
            }
        }
        levels
    }
}

fn filter_draft_winds_by_valid_entries(draft: &[WindEntry], non_draft: &[WindEntry]) -> Vec<WindEntry> {
    draft
        .iter()
        .map(|e| if e.vector.is_partly_filled() { non_draft.iter().find(|it| it.altitude == e.altitude).copied().unwrap_or_default() } else { *e })
        .filter(|e| e.altitude.is_some())
        .collect()
}

impl FlightPlan {
    pub fn draft_winds_enabled(&self) -> bool {
        DRAFT_ON_WIND_EDIT && self.index != 1 && self.index != 2
    }

    fn destination_mag_var(&self) -> f64 {
        self.destination_airport.as_ref().and_then(|a| a.mag_var).unwrap_or(0.)
    }

    pub fn set_climb_wind_entry(&mut self, altitude: Option<f64>, entry: Option<WindEntry>) {
        self.modify_climb_wind_entry(altitude, entry, true, None);
    }

    pub fn edit_climb_wind_entry(&mut self, index: usize, entry: WindEntry) {
        self.modify_climb_wind_entry(entry.altitude, Some(entry), true, Some(index));
    }

    fn modify_climb_wind_entry(&mut self, altitude: Option<f64>, entry: Option<WindEntry>, check_draft: bool, entry_index: Option<usize>) {
        if altitude.is_none() && entry_index.is_none() {
            return;
        }
        if check_draft {
            self.prepare_climb_wind_draft_modification();
        }
        let has_draft = self.draft_winds.climb.is_some();
        if entry.is_some_and(|e| !has_draft && (altitude.is_none() || e.vector.is_partly_filled())) {
            return;
        }
        let origin_elevation = self.origin_airport.as_ref().and_then(|a| a.location.alt).unwrap_or(0.);
        let altitude = altitude.map(|a| if a <= origin_elevation + GROUND_WIND_BAND_FT { origin_elevation } else { a });
        let entry = entry.map(|mut e| {
            if e.altitude.is_some() {
                e.altitude = altitude;
            }
            e
        });
        let entries = match self.draft_winds.climb.as_mut() {
            Some(d) if check_draft => d,
            _ => &mut self.winds.climb,
        };
        let existing = entry_index.or_else(|| entries.iter().position(|e| e.altitude == altitude));
        set_clb_des_entry(entries, altitude, entry, existing, NUM_CLIMB_WIND_LEVELS, !SORT_CLIMB_WIND_DESCENDING);
        self.increment_version();
    }

    pub fn set_descent_wind_entry(&mut self, altitude: Option<f64>, entry: Option<WindEntry>, destination_mag_var: f64) {
        self.modify_descent_wind_entry(altitude, entry, true, true, None, destination_mag_var);
    }

    pub fn set_descent_wind_entry_ex(&mut self, altitude: Option<f64>, entry: Option<WindEntry>, should_update_twr_wind: bool, check_draft: bool, destination_mag_var: f64) {
        self.modify_descent_wind_entry(altitude, entry, should_update_twr_wind, check_draft, None, destination_mag_var);
    }

    pub fn edit_descent_wind_entry(&mut self, index: usize, entry: WindEntry, destination_mag_var: f64) {
        self.modify_descent_wind_entry(entry.altitude, Some(entry), true, true, Some(index), destination_mag_var);
    }

    fn modify_descent_wind_entry(&mut self, altitude: Option<f64>, entry: Option<WindEntry>, should_update_twr_wind: bool, check_draft: bool, entry_index: Option<usize>, destination_mag_var: f64) {
        if altitude.is_none() && entry_index.is_none() {
            return;
        }
        if check_draft {
            self.prepare_descent_wind_draft_modification();
        }
        let has_draft = self.draft_winds.descent.is_some();
        if entry.is_some_and(|e| !has_draft && (altitude.is_none() || e.vector.is_partly_filled())) {
            return;
        }
        let destination_elevation = self.destination_airport.as_ref().and_then(|a| a.location.alt).unwrap_or(0.);
        let altitude = altitude.map(|a| if a <= destination_elevation + GROUND_WIND_BAND_FT { destination_elevation } else { a });
        let mut entry = entry;
        if let Some(e) = entry.as_mut() {
            self.parse_ground_wind_entry_and_set_twr_wind(e, should_update_twr_wind && !has_draft, destination_elevation, destination_mag_var);
        }
        let entries = match self.draft_winds.descent.as_mut() {
            Some(d) if check_draft => d,
            _ => &mut self.winds.descent,
        };
        let existing = entry_index.or_else(|| entries.iter().position(|e| e.altitude == altitude));
        set_clb_des_entry(entries, altitude, entry, existing, NUM_DESCENT_WIND_LEVELS, false);
        self.increment_version();
    }

    fn parse_ground_wind_entry_and_set_twr_wind(&mut self, e: &mut WindEntry, should_update_twr_wind: bool, destination_elevation: f64, destination_mag_var: f64) {
        if !e.altitude.is_some_and(|a| a <= destination_elevation + GROUND_WIND_BAND_FT) {
            return;
        }
        e.altitude = Some(destination_elevation);
        if let (true, Some(direction), Some(magnitude)) = (should_update_twr_wind, e.vector.direction, e.vector.magnitude) {
            self.winds.approach_wind_direction = Some((direction.to_degrees() - destination_mag_var).rem_euclid(360.));
            self.winds.approach_wind_magnitude = Some(magnitude);
            self.winds.is_approach_wind_pilot_entered = false;
        }
    }

    pub fn delete_climb_wind_entries(&mut self) {
        self.delete_draft_climb_wind_entries();
        self.winds.climb.clear();
        self.increment_version();
    }

    pub fn delete_descent_wind_entries(&mut self) {
        self.delete_draft_descent_wind_entries();
        self.winds.descent.clear();
        self.increment_version();
    }

    pub fn set_alternate_wind(&mut self, vector: WindVector) {
        self.set_alternate_wind_ex(vector, false);
    }

    pub fn set_alternate_wind_ex(&mut self, mut vector: WindVector, force_no_draft: bool) {
        if !force_no_draft && self.draft_winds_enabled() {
            let existing = self.draft_winds.alternate.get_or_insert_with(WindVector::default);
            if existing.is_complete() && !vector.is_complete() {
                vector = WindVector::default();
            }
            *existing = vector;
        } else {
            self.winds.alternate = vector;
        }
        self.increment_version();
    }

    pub fn clear_alternate_wind(&mut self) {
        if self.draft_winds.alternate.is_some() {
            self.draft_winds.alternate = None;
        } else {
            self.winds.alternate = WindVector::default();
        }
        self.increment_version();
    }

    pub fn climb_wind_entries(&self) -> &[WindEntry] {
        self.draft_winds.climb.as_deref().unwrap_or(&self.winds.climb)
    }

    pub fn descent_wind_entries(&self) -> &[WindEntry] {
        self.draft_winds.descent.as_deref().unwrap_or(&self.winds.descent)
    }

    pub fn alternate_wind(&self) -> WindVector {
        self.draft_winds.alternate.unwrap_or(self.winds.alternate)
    }

    pub fn has_draft_wind_entries(&self) -> bool {
        let d = &self.draft_winds;
        d.climb.is_some() || d.cruise.is_some() || d.descent.is_some() || d.alternate.is_some()
    }

    pub fn insert_draft_wind_entries(&mut self) {
        let d = self.draft_winds.clone();
        if d.climb.is_some() && d.cruise.is_some() && d.descent.is_some() && d.alternate.is_some() {
            return;
        }
        if let Some(climb) = d.climb.as_ref() {
            self.winds.climb = filter_draft_winds_by_valid_entries(climb, &self.winds.climb);
        }
        if let Some(cruise) = d.cruise.as_ref() {
            let start = self.active_leg_index.max(0) as usize;
            for i in start..self.first_missed_approach_leg_index() {
                let Some(draft) = cruise.get(&i) else { continue };
                if let Some(leg) = self.leg_at_mut(i) {
                    leg.cruise_wind_entries = filter_draft_winds_by_valid_entries(draft, &leg.cruise_wind_entries);
                }
            }
        }
        if let Some(descent) = d.descent.as_ref() {
            let mut valid = filter_draft_winds_by_valid_entries(descent, &self.winds.descent);
            let elevation = self.destination_airport.as_ref().and_then(|a| a.location.alt).unwrap_or(0.);
            let mag_var = self.destination_mag_var();
            for e in valid.iter_mut() {
                self.parse_ground_wind_entry_and_set_twr_wind(e, true, elevation, mag_var);
            }
            self.winds.descent = valid;
        }
        if let Some(alternate) = d.alternate.filter(|a| a.is_complete()) {
            self.set_alternate_wind_ex(alternate, true);
        }
        self.delete_draft_wind_entries();
    }

    pub fn delete_draft_wind_entries(&mut self) {
        let climb = self.delete_draft_climb_wind_entries();
        let cruise = self.delete_cruise_draft_wind_entries();
        let descent = self.delete_draft_descent_wind_entries();
        let alternate = self.delete_alternate_draft_wind_entries();
        if climb || cruise || descent || alternate {
            self.increment_version();
        }
    }

    fn prepare_climb_wind_draft_modification(&mut self) {
        if self.draft_winds_enabled() && self.draft_winds.climb.is_none() {
            self.draft_winds.climb = Some(self.winds.climb.clone());
        }
    }

    fn prepare_descent_wind_draft_modification(&mut self) {
        if self.draft_winds_enabled() && self.draft_winds.descent.is_none() {
            self.draft_winds.descent = Some(self.winds.descent.clone());
        }
    }

    fn prepare_cruise_wind_draft_modification(&mut self) {
        if !self.draft_winds_enabled() || self.draft_winds.cruise.is_some() {
            return;
        }
        let start = self.active_leg_index.max(0) as usize;
        let map = (start..self.first_missed_approach_leg_index()).filter_map(|i| self.leg_at(i).map(|l| (i, l.cruise_wind_entries.clone()))).collect();
        self.draft_winds.cruise = Some(map);
    }

    fn delete_draft_climb_wind_entries(&mut self) -> bool {
        self.draft_winds.climb = None;
        self.draft_winds_enabled()
    }

    fn delete_draft_descent_wind_entries(&mut self) -> bool {
        self.draft_winds.descent = None;
        self.draft_winds_enabled()
    }

    fn delete_cruise_draft_wind_entries(&mut self) -> bool {
        self.draft_winds.cruise = None;
        self.draft_winds_enabled()
    }

    fn delete_alternate_draft_wind_entries(&mut self) -> bool {
        self.draft_winds.alternate = None;
        self.draft_winds_enabled()
    }

    fn cruise_draft_at(&self, index: usize) -> Option<&Vec<WindEntry>> {
        self.draft_winds.cruise.as_ref().and_then(|m| m.get(&index))
    }

    fn cruise_entries(&self, index: usize) -> Option<&Vec<WindEntry>> {
        self.cruise_draft_at(index).or_else(|| self.leg_at(index).map(|l| &l.cruise_wind_entries))
    }

    fn cruise_entries_mut(&mut self, index: usize) -> Option<&mut Vec<WindEntry>> {
        if self.cruise_draft_at(index).is_some() {
            return self.draft_winds.cruise.as_mut().and_then(|m| m.get_mut(&index));
        }
        self.leg_at_mut(index).map(|l| &mut l.cruise_wind_entries)
    }

    pub fn propagate_winds_at(&self, at_index: usize) -> Vec<PropagatedWindEntry> {
        let mut result: Vec<PropagatedWindEntry> = Vec::new();
        for i in 0..self.first_missed_approach_leg_index() {
            let Some(entries) = self.cruise_entries(i) else { continue };
            for wind in entries {
                let kind = if i == at_index || wind.altitude.is_none() {
                    PropagationType::Entry
                } else if i < at_index {
                    PropagationType::Forward
                } else {
                    PropagationType::Backward
                };
                let propagated = PropagatedWindEntry { altitude: wind.altitude, vector: wind.vector, kind, source_leg_index: i };
                let existing = result.iter_mut().find(|e| (wind.altitude.is_none() && e.altitude.is_none() && e.source_leg_index == i) || same_level(e.altitude, wind.altitude));
                if let Some(e) = existing {
                    if kind != PropagationType::Backward {
                        *e = propagated;
                    }
                } else if result.len() < NUM_CRUISE_WIND_LEVELS {
                    result.push(propagated);
                }
            }
        }
        let mut sorted: Vec<PropagatedWindEntry> = result.iter().filter(|e| e.altitude.is_some()).copied().collect();
        sorted.sort_by(|a, b| b.altitude.unwrap_or(0.).total_cmp(&a.altitude.unwrap_or(0.)));
        let mut next = sorted.into_iter();
        result.into_iter().map(|e| if e.altitude.is_none() { e } else { next.next().unwrap_or(e) }).collect()
    }

    pub fn add_cruise_wind_entry(&mut self, at_index: usize, entry: WindEntry) -> bool {
        self.add_cruise_wind_entry_ex(at_index, entry, true, 0)
    }

    fn add_cruise_wind_entry_ex(&mut self, at_index: usize, entry: WindEntry, check_draft: bool, depth: u32) -> bool {
        if depth > 4 || !self.leg_at(at_index).is_some_and(|l| l.is_xf()) {
            return false;
        }
        if check_draft {
            self.prepare_cruise_wind_draft_modification();
        }
        let has_draft = self.cruise_draft_at(at_index).is_some();
        if !has_draft && (entry.altitude.is_none() || entry.vector.is_partly_filled()) {
            return false;
        }
        let existing = self.cruise_entries(at_index).and_then(|entries| entries.iter().position(|e| (entry.altitude.is_none() && e.altitude.is_none()) || same_level(e.altitude, entry.altitude)));
        match existing {
            Some(i) => {
                self.edit_cruise_wind_entry_ex(at_index, i, entry, check_draft, depth + 1);
            }
            None => {
                if let Some(entries) = self.cruise_entries_mut(at_index) {
                    entries.push(entry);
                }
            }
        }
        self.increment_version();
        true
    }

    pub fn delete_cruise_wind_entry(&mut self, at_index: usize, altitude: f64) -> bool {
        self.delete_cruise_wind_entry_ex(at_index, altitude, true)
    }

    fn delete_cruise_wind_entry_ex(&mut self, at_index: usize, altitude: f64, check_draft: bool) -> bool {
        if !self.leg_at(at_index).is_some_and(|l| l.is_xf()) {
            return false;
        }
        if check_draft {
            self.prepare_cruise_wind_draft_modification();
        }
        let Some(entries) = self.cruise_entries_mut(at_index) else { return false };
        if !entries.iter().any(|e| same_level(e.altitude, Some(altitude))) {
            return false;
        }
        // FBW splices inside a forward loop, so a second adjacent match is skipped.
        let mut i = 0;
        while i < entries.len() {
            if same_level(entries[i].altitude, Some(altitude)) {
                entries.remove(i);
            }
            i += 1;
        }
        self.increment_version();
        true
    }

    pub fn edit_cruise_wind_entry(&mut self, at_index: usize, entry_index: usize, new_entry: WindEntry) -> bool {
        self.edit_cruise_wind_entry_ex(at_index, entry_index, new_entry, true, 0)
    }

    fn edit_cruise_wind_entry_ex(&mut self, at_index: usize, entry_index: usize, mut new_entry: WindEntry, check_draft: bool, depth: u32) -> bool {
        if depth > 4 || !self.leg_at(at_index).is_some_and(|l| l.is_xf()) {
            return false;
        }
        if check_draft {
            self.prepare_cruise_wind_draft_modification();
        }
        let has_draft = self.cruise_draft_at(at_index).is_some();
        if !has_draft && (new_entry.altitude.is_none() || new_entry.vector.is_partly_filled()) {
            return false;
        }
        let propagated = self.propagate_winds_at(at_index);
        if propagated.get(entry_index).is_none() {
            return false;
        }
        let found = propagated.iter().find(|e| (new_entry.altitude.is_none() && e.altitude.is_none() && e.source_leg_index == at_index) || same_level(e.altitude, new_entry.altitude)).copied();
        match found {
            Some(p) if p.kind == PropagationType::Entry => {
                if let Some(old) = self.cruise_entries_mut(at_index).and_then(|entries| entries.get_mut(entry_index)) {
                    if old.vector.is_complete() && new_entry.vector.is_partly_filled() {
                        new_entry.vector = WindVector::default();
                    }
                    old.vector = new_entry.vector;
                }
                self.increment_version();
                true
            }
            Some(_) => self.add_cruise_wind_entry_ex(at_index, new_entry, check_draft, depth + 1),
            None => {
                if let Some(altitude) = new_entry.altitude {
                    let legs: Vec<usize> = (0..self.leg_count()).filter(|i| self.cruise_entries(*i).is_some_and(|entries| entries.iter().any(|e| same_level(e.altitude, Some(altitude))))).collect();
                    for i in legs {
                        self.delete_cruise_wind_entry_ex(i, altitude, check_draft);
                    }
                }
                self.add_cruise_wind_entry_ex(at_index, new_entry, check_draft, depth + 1)
            }
        }
    }

    pub fn delete_all_cruise_wind_entries(&mut self) {
        self.delete_cruise_draft_wind_entries();
        for seg in self.segments.iter_mut() {
            for leg in seg.legs.iter_mut().filter_map(|e| e.leg_mut()) {
                leg.cruise_wind_entries.clear();
            }
        }
        self.increment_version();
    }

    pub fn find_leg_index_by_fix_ident(&self, ident: &str) -> Option<usize> {
        (0..self.leg_count()).find(|i| self.leg_at(*i).is_some_and(|l| l.is_xf() && l.termination_waypoint().is_some_and(|w| w.ident == ident)))
    }

    pub fn find_leg_index_by_coordinates(&self, lat: f64, long: f64) -> Option<usize> {
        const EPSILON: f64 = 1e-4;
        (0..self.leg_count()).find(|i| self.leg_at(*i).is_some_and(|l| l.is_xf() && l.termination_waypoint().is_some_and(|w| (w.location.lat - lat).abs() < EPSILON && (w.location.long - long).abs() < EPSILON)))
    }

    pub fn insert_wind_uplink(&mut self) -> Result<(), String> {
        if !self.pending_wind_uplink.is_wind_uplink_ready_to_insert() {
            return Err("[FPM] Cannot insert wind uplink when it is not ready to insert".into());
        }
        let uplink = self.pending_wind_uplink.clone();
        if let Some(climb) = uplink.climb_winds {
            self.delete_climb_wind_entries();
            for wind in climb {
                self.modify_climb_wind_entry(wind.altitude, Some(wind), false, None);
            }
        }
        if let Some(cruise) = uplink.cruise_winds {
            self.delete_all_cruise_wind_entries();
            for fix in cruise {
                let index = match &fix.fix {
                    PendingCruiseFix::Waypoint(ident) => self.find_leg_index_by_fix_ident(ident),
                    PendingCruiseFix::LatLon { lat, long } => self.find_leg_index_by_coordinates(*lat, *long),
                };
                let Some(index) = index else { continue };
                for wind in fix.levels {
                    self.add_cruise_wind_entry_ex(index, wind, false, 0);
                }
            }
        }
        if let Some(descent) = uplink.descent_winds {
            self.delete_descent_wind_entries();
            let mag_var = self.destination_mag_var();
            for wind in descent {
                self.modify_descent_wind_entry(wind.altitude, Some(wind), true, false, None, mag_var);
            }
        }
        if let Some(alternate) = uplink.alternate_wind {
            self.set_alternate_wind_ex(alternate.vector, true);
        }
        self.pending_wind_uplink.on_uplink_inserted();
        Ok(())
    }

    pub fn has_wind_entries(&self) -> bool {
        !self.winds.climb.is_empty() || !self.winds.descent.is_empty() || self.segments.iter().any(|s| s.legs.iter().any(|e| e.leg().is_some_and(|l| !l.cruise_wind_entries.is_empty())))
    }
}

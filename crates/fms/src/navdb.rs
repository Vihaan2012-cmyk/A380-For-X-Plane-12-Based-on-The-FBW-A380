use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::geo::Coordinates;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub lat: f64,
    pub long: f64,
    #[serde(default)]
    pub alt: Option<f64>,
}

impl Location {
    pub fn coordinates(&self) -> Coordinates {
        Coordinates::new(self.lat, self.long)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Fix {
    pub database_id: String,
    pub ident: String,
    pub icao_code: String,
    pub location: Location,
    pub section_code: i32,
    pub sub_section_code: i32,
    pub area: i32,
    pub name: Option<String>,
    pub airport_ident: Option<String>,
    pub mag_var: Option<f64>,
    pub transition_altitude: Option<f64>,
    pub transition_level: Option<f64>,
    pub longest_runway_length: Option<f64>,
    pub longest_runway_surface_type: Option<i32>,
    pub station_declination: Option<f64>,
}

impl Fix {
    pub fn is_airport(&self) -> bool {
        self.section_code == 4 && self.sub_section_code == 0
    }

    pub fn lat_long(ident: &str, lat: f64, long: f64) -> Fix {
        Fix {
            database_id: format!("W      {ident}"),
            ident: ident.to_string(),
            icao_code: "  ".to_string(),
            location: Location { lat, long, alt: None },
            section_code: 2,
            ..Fix::default()
        }
    }

    pub fn same_item(&self, other: &Fix) -> bool {
        self.database_id == other.database_id
    }

    pub fn is_same_database_item(&self, other: &Fix) -> bool {
        self.section_code == other.section_code && self.sub_section_code == other.sub_section_code && self.icao_code == other.icao_code && self.ident == other.ident
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Runway {
    #[serde(flatten)]
    pub fix: Fix,
    pub bearing: f64,
    pub magnetic_bearing: f64,
    pub length: f64,
    pub number: i32,
    pub designator: i32,
    pub threshold_location: Location,
}

impl Runway {
    pub fn ident(&self) -> &str {
        &self.fix.ident
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcedureKind {
    #[default]
    Departure,
    Arrival,
    Approach,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProcedureLeg {
    pub procedure_ident: String,
    #[serde(rename = "type")]
    pub leg_type: String,
    pub waypoint: Option<Fix>,
    pub waypoint_descriptor: Option<i32>,
    pub approach_waypoint_descriptor: Option<i32>,
    pub overfly: bool,
    pub turn_direction: Option<char>,
    pub altitude_descriptor: Option<String>,
    pub altitude1: Option<f64>,
    pub altitude2: Option<f64>,
    pub speed_descriptor: Option<String>,
    pub speed: Option<f64>,
    pub course: Option<f64>,
    pub mag_var: Option<f64>,
    pub rho: Option<f64>,
    pub theta: Option<f64>,
    pub length: Option<f64>,
    pub length_time: Option<f64>,
    pub recommended_navaid: Option<Fix>,
    pub vertical_angle: Option<f64>,
    pub is_engine_out_branch: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Transition {
    pub database_id: String,
    pub ident: String,
    pub legs: Vec<ProcedureLeg>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Procedure {
    #[serde(skip)]
    pub kind: ProcedureKind,
    pub database_id: String,
    pub ident: String,
    pub runway_transitions: Vec<Transition>,
    #[serde(alias = "legs")]
    pub common_legs: Vec<ProcedureLeg>,
    #[serde(alias = "transitions")]
    pub enroute_transitions: Vec<Transition>,
    #[serde(alias = "missedLegs")]
    pub missed_legs: Vec<ProcedureLeg>,
    pub runway_ident: Option<String>,
    #[serde(rename = "type")]
    pub approach_type: i32,
    pub multiple_indicator: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Airway {
    pub ident: String,
    pub fixes: Vec<Fix>,
}

pub fn are_database_items_equal(a: Option<&Fix>, b: Option<&Fix>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.is_same_database_item(b),
        _ => false,
    }
}

pub trait NavDatabase {
    fn airport(&self, icao: &str) -> Option<Fix>;
    fn runways(&self, airport_icao: &str) -> Vec<Runway>;
    fn departures(&self, airport_icao: &str) -> Vec<Procedure>;
    fn arrivals(&self, airport_icao: &str) -> Vec<Procedure>;
    fn approaches(&self, airport_icao: &str) -> Vec<Procedure>;
    fn search_fix(&self, ident: &str) -> Vec<Fix>;
    fn search_airway(&self, ident: &str, via: &Fix) -> Vec<Airway>;
    fn navaids(&self, ident: &str) -> Vec<Fix>;
    fn engine_out_departure(&self, _airport_icao: &str, _departure_database_id: &str, _runway_ident: &str) -> Option<Procedure> {
        None
    }
}

pub fn is_engine_out_sid_ident(ident: &str) -> bool {
    let b = ident.as_bytes();
    if b.len() < 4 || &b[..2] != b"EO" || !b[2].is_ascii_digit() || !b[3].is_ascii_digit() {
        return false;
    }
    let mut rest = &b[4..];
    if let Some((c, r)) = rest.split_first() {
        if b"LCRT".contains(c) {
            rest = r;
        }
    }
    match rest.split_first() {
        None => true,
        Some((b'-', tail)) => !tail.is_empty() && tail.iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()),
        _ => false,
    }
}

fn engine_out_branch_index(sid: &Transition, eo: &Transition) -> i64 {
    let mut branch = -1;
    for (i, (s, e)) in sid.legs.iter().zip(eo.legs.iter()).enumerate() {
        let id = |l: &ProcedureLeg| l.waypoint.as_ref().map(|w| w.database_id.clone());
        if s.leg_type != e.leg_type || id(s) != id(e) {
            break;
        }
        branch = i as i64;
    }
    branch
}

/// FBW `MsfsMapping.matchEngineOutDepartures`: marks each SID runway transition's branch leg and returns, per (SID database id, runway), the matched EOSID.
pub fn match_engine_out_departures(departures: &mut [Procedure]) -> Vec<(String, String, Procedure)> {
    let eo_sids: Vec<Procedure> = departures.iter().filter(|d| is_engine_out_sid_ident(&d.ident)).cloned().collect();
    let mut matched = Vec::new();
    if eo_sids.is_empty() {
        return matched;
    }
    for sid in departures.iter_mut() {
        let sid_id = sid.database_id.clone();
        for sid_trans in sid.runway_transitions.iter_mut() {
            let mut best: Option<&Procedure> = None;
            let mut best_index = -1;
            for eo in eo_sids.iter().filter(|eo| eo.runway_transitions.iter().any(|t| t.ident == sid_trans.ident)) {
                let Some(eo_trans) = eo.runway_transitions.iter().find(|t| t.ident == sid_trans.ident) else { continue };
                let branch = engine_out_branch_index(sid_trans, eo_trans);
                if branch >= 1 && branch > best_index {
                    best = Some(eo);
                    best_index = branch;
                }
            }
            if let Some(eo) = best {
                sid_trans.legs[best_index as usize].is_engine_out_branch = true;
                matched.push((sid_id.clone(), sid_trans.ident.clone(), eo.clone()));
            }
        }
    }
    matched
}

pub fn engine_out_departure_from(departures: &[Procedure], departure_database_id: &str, runway_ident: &str) -> Option<Procedure> {
    let mut departures = departures.to_vec();
    match_engine_out_departures(&mut departures).into_iter().find(|(sid, rwy, _)| sid == departure_database_id && rwy == runway_ident).map(|(_, _, eo)| eo)
}

#[derive(Default)]
pub struct MemoryNavDb {
    pub fixes: Vec<Fix>,
    pub airways: Vec<Airway>,
    pub runways: HashMap<String, Vec<Runway>>,
    pub procedures: HashMap<String, Vec<Procedure>>,
}

#[derive(Deserialize)]
struct FixtureAirway {
    ident: String,
    fixes: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct FixtureAirport {
    runways: Vec<Runway>,
    departures: Vec<Procedure>,
    arrivals: Vec<Procedure>,
    approaches: Vec<Procedure>,
}

#[derive(Deserialize)]
struct Fixture {
    fixes: Vec<Fix>,
    airways: Vec<FixtureAirway>,
    #[serde(default)]
    airports: HashMap<String, FixtureAirport>,
}

impl MemoryNavDb {
    pub fn from_fixture_json(json: &str) -> Result<Self, String> {
        let fixture: Fixture = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let mut db = MemoryNavDb { fixes: fixture.fixes, ..MemoryNavDb::default() };
        for awy in fixture.airways {
            let fixes = awy
                .fixes
                .iter()
                .map(|id| db.fixes.iter().find(|f| &f.database_id == id).cloned().ok_or_else(|| format!("airway {} names unknown fix {id:?}", awy.ident)))
                .collect::<Result<Vec<_>, _>>()?;
            db.airways.push(Airway { ident: awy.ident, fixes });
        }
        for (icao, airport) in fixture.airports {
            let mut procedures = Vec::new();
            for (kind, list) in [(ProcedureKind::Departure, airport.departures), (ProcedureKind::Arrival, airport.arrivals), (ProcedureKind::Approach, airport.approaches)] {
                for mut p in list {
                    p.kind = kind;
                    procedures.push(p);
                }
            }
            db.runways.insert(icao.clone(), airport.runways);
            db.procedures.insert(icao, procedures);
        }
        Ok(db)
    }
}

impl NavDatabase for MemoryNavDb {
    fn airport(&self, icao: &str) -> Option<Fix> {
        self.fixes.iter().find(|f| f.is_airport() && f.ident == icao).cloned()
    }

    fn runways(&self, airport_icao: &str) -> Vec<Runway> {
        self.runways.get(airport_icao).cloned().unwrap_or_default()
    }

    fn departures(&self, airport_icao: &str) -> Vec<Procedure> {
        self.procedures_of(airport_icao, ProcedureKind::Departure)
    }

    fn arrivals(&self, airport_icao: &str) -> Vec<Procedure> {
        self.procedures_of(airport_icao, ProcedureKind::Arrival)
    }

    fn approaches(&self, airport_icao: &str) -> Vec<Procedure> {
        self.procedures_of(airport_icao, ProcedureKind::Approach)
    }

    fn search_fix(&self, ident: &str) -> Vec<Fix> {
        self.fixes.iter().filter(|f| f.ident == ident && !f.is_airport()).cloned().collect()
    }

    fn search_airway(&self, ident: &str, via: &Fix) -> Vec<Airway> {
        self.airways
            .iter()
            .filter(|a| a.ident == ident && a.fixes.iter().any(|f| f.ident == via.ident && f.icao_code == via.icao_code))
            .cloned()
            .collect()
    }

    fn navaids(&self, ident: &str) -> Vec<Fix> {
        self.fixes.iter().filter(|f| f.ident == ident && f.section_code == 1).cloned().collect()
    }
}

impl MemoryNavDb {
    fn procedures_of(&self, airport_icao: &str, kind: ProcedureKind) -> Vec<Procedure> {
        self.procedures.get(airport_icao).map(|v| v.iter().filter(|p| p.kind == kind).cloned().collect()).unwrap_or_default()
    }
}

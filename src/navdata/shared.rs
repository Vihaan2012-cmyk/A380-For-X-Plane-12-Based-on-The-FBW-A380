//! Read access to the loaded database from any thread, once indexing is done.

use std::sync::{Arc, RwLock};

use super::db::{bearing_deg, Db, FacRef};
use super::icao::{Code, Icao};

static DB: RwLock<Option<Arc<Db>>> = RwLock::new(None);

const M_PER_NM: f64 = 1852.;

pub(super) fn publish(db: &Arc<Db>) {
    *DB.write().unwrap_or_else(|e| e.into_inner()) = Some(db.clone());
}

fn current() -> Option<Arc<Db>> {
    DB.read().unwrap_or_else(|e| e.into_inner()).clone()
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunwayEndInfo {
    pub ident: String,
    pub heading_true_deg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AirportInfo {
    pub icao: String,
    pub lat: f64,
    pub lon: f64,
    pub elevation_ft: f64,
    pub runway_ends: Vec<RunwayEndInfo>,
    pub transition_level_ft: Option<i32>,
    pub transition_alt_ft: Option<i32>,
}

fn info(db: &Db, index: u32) -> AirportInfo {
    let a = &db.airports[index as usize];
    let mut runway_ends = Vec::new();
    for r in a.runways.iter().filter(|r| !r.water) {
        let [e0, e1] = &r.ends;
        runway_ends.push(RunwayEndInfo { ident: e0.id.as_str().to_string(), heading_true_deg: bearing_deg(e0.lat, e0.lon, e1.lat, e1.lon) });
        runway_ends.push(RunwayEndInfo { ident: e1.id.as_str().to_string(), heading_true_deg: bearing_deg(e1.lat, e1.lon, e0.lat, e0.lon) });
    }
    AirportInfo {
        icao: a.ident.as_str().to_string(),
        lat: a.lat,
        lon: a.lon,
        elevation_ft: a.elev_ft as f64,
        runway_ends,
        transition_level_ft: a.tl_ft,
        transition_alt_ft: a.ta_ft,
    }
}

/// An airport by ICAO ident; `None` until the database has loaded.
pub fn airport(icao: &str) -> Option<AirportInfo> {
    let db = current()?;
    let ident = Code::new(&icao.trim().to_ascii_uppercase())?;
    match db.find(&Icao::new(b'A', [0; 2], Code::EMPTY, ident))? {
        FacRef::Airport(i) => Some(info(&db, i)),
        _ => None,
    }
}

/// Open airports within `radius_nm`, nearest first: (ident, elevation ft, distance nm).
pub fn airports_within(lat: f64, lon: f64, radius_nm: f64, max: usize) -> Vec<(String, f64, f64)> {
    let Some(db) = current() else { return Vec::new() };
    let mut found = Vec::new();
    db.within(lat, lon, radius_nm * M_PER_NM, |r, d| {
        if let FacRef::Airport(i) = r {
            let a = &db.airports[i as usize];
            if !a.closed {
                found.push((a.ident.as_str().to_string(), a.elev_ft as f64, d / M_PER_NM));
            }
        }
    });
    found.sort_by(|a, b| a.2.total_cmp(&b.2));
    found.truncate(max);
    found
}

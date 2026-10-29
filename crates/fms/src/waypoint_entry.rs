use crate::geo::Coordinates;
use crate::navdb::{Fix, NavDatabase, Runway};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryError {
    FormatError,
    EntryOutOfRange,
    NotInDatabase,
    NotYetImplemented,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WaypointEntry {
    LatLon(Coordinates),
    PlaceBearingPlaceBearing(Fix, f64, Fix, f64),
    PlaceBearingDistance(Fix, f64, f64),
    Place(Fix),
    NewWaypoint(String),
}

fn all(s: &str, f: impl Fn(char) -> bool) -> bool {
    s.chars().all(f)
}

fn is_upper_alnum(c: char) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit()
}

pub fn is_runway_format(s: &str) -> bool {
    split_runway(s).is_some()
}

fn split_runway(s: &str) -> Option<(&str, &str)> {
    if !s.is_ascii() || s.len() < 6 || s.len() > 7 {
        return None;
    }
    let (airport, runway) = s.split_at(4);
    let digits = &runway[..2];
    let designator = &runway[2..];
    let ok = all(airport, |c| c.is_ascii_uppercase()) && all(digits, |c| c.is_ascii_digit()) && (designator.is_empty() || matches!(designator, "R" | "C" | "L"));
    ok.then_some((airport, runway))
}

pub fn is_place_format(s: &str) -> bool {
    (s.len() >= 2 && s.len() <= 7 && all(s, is_upper_alnum)) || is_runway_format(s)
}

struct LatLonParts<'a> {
    lat_hemisphere: String,
    lat_digits: &'a str,
    lon_hemisphere: String,
    lon_digits: &'a str,
}

fn decimal_part(s: &str, min_int: usize, max_int: usize) -> bool {
    let Some((int, frac)) = s.split_once('.') else { return false };
    int.len() >= min_int && int.len() <= max_int && all(int, |c| c.is_ascii_digit()) && frac.len() == 1 && all(frac, |c| c.is_ascii_digit())
}

fn hemisphere_split<'a>(s: &'a str, letters: [char; 2]) -> Option<(String, &'a str)> {
    let mut body = s;
    let mut hemisphere = String::new();
    if let Some(c) = body.chars().next().filter(|c| letters.contains(c)) {
        hemisphere.push(c);
        body = &body[1..];
    }
    if let Some(c) = body.chars().last().filter(|c| letters.contains(c)) {
        hemisphere.push(c);
        body = &body[..body.len() - 1];
    }
    Some((hemisphere, body))
}

fn lat_lon_parts(s: &str) -> Option<LatLonParts<'_>> {
    if !s.is_ascii() {
        return None;
    }
    let (lat, lon) = s.split_once('/')?;
    let (lat_hemisphere, lat_digits) = hemisphere_split(lat, ['N', 'S'])?;
    let (lon_hemisphere, lon_digits) = hemisphere_split(lon, ['E', 'W'])?;
    if !decimal_part(lat_digits, 2, 4) || !decimal_part(lon_digits, 2, 5) {
        return None;
    }
    Some(LatLonParts { lat_hemisphere, lat_digits, lon_hemisphere, lon_digits })
}

pub fn is_lat_lon_format(s: &str) -> bool {
    lat_lon_parts(s).is_some()
}

fn degrees_minutes(digits: &str) -> (f64, f64) {
    let minute_digits = if digits.len() == 4 { 3 } else { 4 };
    let split = digits.len().saturating_sub(minute_digits);
    let degrees = digits[..split].parse::<f64>().unwrap_or(f64::NAN);
    let minutes = digits[split..].parse::<f64>().unwrap_or(f64::NAN);
    (degrees, minutes)
}

pub fn parse_lat_lon(s: &str) -> Result<Coordinates, EntryError> {
    let parts = lat_lon_parts(s).ok_or(EntryError::FormatError)?;
    let (lat_d, lat_m) = degrees_minutes(parts.lat_digits);
    let (lon_d, lon_m) = degrees_minutes(parts.lon_digits);
    if parts.lat_hemisphere.len() != 1 || parts.lon_hemisphere.len() != 1 || !lat_m.is_finite() || !lon_m.is_finite() {
        return Err(EntryError::FormatError);
    }
    if lat_d > 90. || lat_m > 59.9 || lon_d > 180. || lon_m > 59.9 {
        return Err(EntryError::EntryOutOfRange);
    }
    let lat = (lat_d + lat_m / 60.) * if parts.lat_hemisphere == "S" { -1. } else { 1. };
    let long = (lon_d + lon_m / 60.) * if parts.lon_hemisphere == "W" { -1. } else { 1. };
    Ok(Coordinates::new(lat, long))
}

fn bearing_digits(s: &str) -> bool {
    !s.is_empty() && s.len() <= 3 && all(s, |c| c.is_ascii_digit())
}

fn pbx_parts(s: &str) -> Option<(&str, &str, &str, &str)> {
    let (first, second) = s.split_once('/')?;
    if second.contains('/') {
        return None;
    }
    let (place1, brg1) = first.split_once('-')?;
    let (place2, brg2) = second.split_once('-')?;
    let place_ok = |p: &str| !p.is_empty() && !p.contains('-');
    (place_ok(place1) && place_ok(place2) && bearing_digits(brg1) && bearing_digits(brg2)).then_some((place1, brg1, place2, brg2))
}

pub fn is_pbx_format(s: &str) -> bool {
    pbx_parts(s).is_some_and(|(p1, _, p2, _)| is_place_format(p1) && is_place_format(p2))
}

fn distance_digits(s: &str) -> bool {
    match s.split_once('.') {
        Some((int, frac)) => bearing_digits(int) && frac.len() == 1 && all(frac, |c| c.is_ascii_digit()),
        None => bearing_digits(s),
    }
}

pub fn is_pbd_format(s: &str) -> bool {
    let parts: Vec<&str> = s.split('/').collect();
    parts.len() == 3 && !parts[0].is_empty() && bearing_digits(parts[1]) && distance_digits(parts[2]) && is_place_format(parts[0])
}

pub fn is_pd_format(s: &str) -> bool {
    let Some((place, dist)) = s.split_once('/') else { return false };
    let dist = dist.strip_prefix(['-', '+']).unwrap_or(dist);
    !place.is_empty() && !dist.contains('/') && distance_digits(dist) && is_place_format(place)
}

pub fn split_pbd(s: &str) -> (String, f64, f64) {
    let mut parts = s.split('/');
    let place = parts.next().unwrap_or("").to_string();
    let brg = parts.next().and_then(|b| b.parse::<i64>().ok()).map(|b| b as f64).unwrap_or(f64::NAN);
    let dist = parts.next().and_then(|d| d.parse::<f64>().ok()).unwrap_or(f64::NAN);
    (place, brg, dist)
}

pub fn merge_navaids_with_fixes(navaids: Vec<Fix>, waypoints: Vec<Fix>) -> Vec<Fix> {
    let mut items = navaids;
    for wp in waypoints {
        if !items.iter().any(|i| i.database_id == wp.database_id) {
            items.push(wp);
        }
    }
    items
}

pub fn parse_runway(db: &dyn NavDatabase, place: &str) -> Result<Runway, EntryError> {
    let (airport, _) = split_runway(place).ok_or(EntryError::NotInDatabase)?;
    let airport = db.airport(airport).ok_or(EntryError::NotInDatabase)?;
    db.runways(&airport.ident).into_iter().find(|r| r.fix.ident == place).ok_or(EntryError::NotInDatabase)
}

pub fn parse_place(db: &dyn NavDatabase, place: &str, stored: &[Fix], deduplicate: &mut dyn FnMut(Vec<Fix>) -> Option<Fix>) -> Result<Fix, EntryError> {
    if is_runway_format(place) {
        return parse_runway(db, place).map(|r| r.fix);
    }
    let mut waypoints: Vec<Fix> = db.search_fix(place);
    if let Some(airport) = db.airport(place) {
        waypoints.push(airport);
    }
    waypoints.extend(stored.iter().filter(|s| s.ident == place).cloned());
    let items = merge_navaids_with_fixes(db.navaids(place), waypoints);
    if items.is_empty() {
        return Err(EntryError::NotInDatabase);
    }
    deduplicate(items).ok_or(EntryError::NotInDatabase)
}

pub fn parse_pbx(db: &dyn NavDatabase, s: &str, stored: &[Fix], deduplicate: &mut dyn FnMut(Vec<Fix>) -> Option<Fix>) -> Result<(Fix, f64, Fix, f64), EntryError> {
    let (p1, b1, p2, b2) = pbx_parts(s).ok_or(EntryError::FormatError)?;
    let brg1: f64 = b1.parse().map_err(|_| EntryError::FormatError)?;
    let brg2: f64 = b2.parse().map_err(|_| EntryError::FormatError)?;
    if brg1 > 360. || brg2 > 360. {
        return Err(EntryError::EntryOutOfRange);
    }
    let place1 = parse_place(db, p1, stored, deduplicate)?;
    let place2 = parse_place(db, p2, stored, deduplicate)?;
    Ok((place1, brg1, place2, brg2))
}

pub fn parse_pbd(db: &dyn NavDatabase, s: &str, stored: &[Fix], deduplicate: &mut dyn FnMut(Vec<Fix>) -> Option<Fix>) -> Result<(Fix, f64, f64), EntryError> {
    let (place, brg, dist) = split_pbd(s);
    if brg > 360. || dist > 999.9 {
        return Err(EntryError::EntryOutOfRange);
    }
    if !is_place_format(&place) {
        return Err(EntryError::FormatError);
    }
    Ok((parse_place(db, &place, stored, deduplicate)?, brg, dist))
}

pub fn get_or_create_waypoint(db: &dyn NavDatabase, place: &str, stored: &[Fix], deduplicate: &mut dyn FnMut(Vec<Fix>) -> Option<Fix>) -> Result<WaypointEntry, EntryError> {
    if is_lat_lon_format(place) {
        return parse_lat_lon(place).map(WaypointEntry::LatLon);
    }
    if is_pbx_format(place) {
        let (p1, b1, p2, b2) = parse_pbx(db, place, stored, deduplicate)?;
        return Ok(WaypointEntry::PlaceBearingPlaceBearing(p1, b1, p2, b2));
    }
    if is_pd_format(place) {
        return Err(EntryError::NotYetImplemented);
    }
    if is_pbd_format(place) {
        let (wp, brg, dist) = parse_pbd(db, place, stored, deduplicate)?;
        return Ok(WaypointEntry::PlaceBearingDistance(wp, brg, dist));
    }
    if is_place_format(place) {
        return match parse_place(db, place, stored, deduplicate) {
            Ok(fix) => Ok(WaypointEntry::Place(fix)),
            Err(EntryError::NotInDatabase) => Ok(WaypointEntry::NewWaypoint(place.to_string())),
            Err(e) => Err(e),
        };
    }
    Err(EntryError::FormatError)
}

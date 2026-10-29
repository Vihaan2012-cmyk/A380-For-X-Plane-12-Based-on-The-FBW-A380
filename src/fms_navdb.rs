use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

use fms::geo::Coordinates;
use fms::navdb::{engine_out_departure_from, match_engine_out_departures, Airway, Fix, Location, NavDatabase, Procedure, ProcedureKind, ProcedureLeg, Runway, Transition};
use serde_json::Value;

use crate::navdata::NavData;

const FT: f64 = 0.3048;
const NM: f64 = 1852.;

pub struct FmsNavDb {
    nav: RefCell<NavData>,
    facilities: RefCell<HashMap<String, Option<Value>>>,
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn n(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.)
}

fn array<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn icao_ident(icao: &str) -> String {
    icao.get(7..).unwrap_or("").trim().to_string()
}

fn icao_region(icao: &str) -> String {
    icao.get(1..3).unwrap_or("  ").to_string()
}

fn designator_char(code: i64) -> Option<&'static str> {
    match code {
        0 => Some(""),
        1 => Some("L"),
        2 => Some("R"),
        3 => Some("C"),
        4 => Some("W"),
        5 => Some("A"),
        6 => Some("B"),
        _ => None,
    }
}

fn runway_designator(code: i64) -> Option<i32> {
    match code {
        0 => Some(0),
        3 => Some(1),
        1 => Some(2),
        2 => Some(3),
        _ => None,
    }
}

fn leg_type(code: i64) -> Option<&'static str> {
    const NAMES: [&str; 24] = ["", "AF", "CA", "CD", "CF", "CI", "CR", "DF", "FA", "FC", "FD", "FM", "HA", "HF", "HM", "IF", "PI", "RF", "TF", "VA", "VD", "VI", "VM", "VR"];
    NAMES.get(code as usize).copied().filter(|n| !n.is_empty())
}

fn approach_type(code: i64) -> i32 {
    match code {
        1 => 10,
        2 => 16,
        3 => 9,
        4 => 5,
        5 => 7,
        6 => 15,
        7 => 18,
        8 => 2,
        9 => 11,
        10 => 12,
        11 => 1,
        _ => 0,
    }
}

fn approach_prefix(code: i64) -> char {
    match code {
        11 => 'B',
        8 => 'D',
        4 => 'I',
        5 => 'L',
        3 => 'N',
        1 => 'P',
        9 => 'Q',
        10 => 'R',
        6 => 'U',
        2 => 'V',
        7 => 'X',
        _ => ' ',
    }
}

/// FBW `MsfsMapping.mapVorType`, MSFS `VorType` to `VhfNavaidType`.
fn vhf_type(kind: u8) -> u32 {
    use fms_host::facilities::vhf_type::*;
    match kind {
        1 => VOR,
        2 => VOR_DME,
        3 => DME,
        4 => TACAN,
        5 => VORTAC,
        6 => ILS_DME,
        7 => VOT,
        _ => UNKNOWN,
    }
}

fn normalise180(x: f64) -> f64 {
    let r = x.rem_euclid(360.);
    if r > 180. {
        r - 360.
    } else {
        r
    }
}

impl FmsNavDb {
    pub fn new(mut nav: NavData) -> Result<Self, String> {
        nav.wait_ready()?;
        Ok(FmsNavDb { nav: RefCell::new(nav), facilities: RefCell::new(HashMap::new()) })
    }

    fn facility(&self, icao: &str) -> Option<Value> {
        let call = match icao.chars().next() {
            Some('A') => "LOAD_AIRPORT",
            Some('V') => "LOAD_VOR",
            Some('N') => "LOAD_NDB",
            Some('W') => "LOAD_INTERSECTION",
            _ => return None,
        };
        self.load(icao, call, icao.to_string())
    }

    /// FBW `MsfsMapping.mapLandingSystem` for a localizer's VOR record.
    pub fn ils(&self, database_id: &str) -> Option<fms_host::facilities::IlsNavaid> {
        let v = self.facility(database_id)?;
        let ils = v.get("ils").filter(|i| i.is_object())?;
        let has_gs = ils.get("hasGlideslope").and_then(Value::as_bool).unwrap_or(false);
        Some(fms_host::facilities::IlsNavaid {
            database_id: s(&v, "icao").to_string(),
            ident: icao_ident(s(&v, "icao")),
            frequency: n(&v, "freqMHz"),
            loc_bearing: n(ils, "localizerCourse"),
            gs_slope: has_gs.then(|| -n(ils, "glideslopeAngle")),
            is_localizer_glide_slope: true,
        })
    }

    /// FBW `MsfsMapping` for a `V` facility: figure of merit from `mapVorFigureOfMerit`, type from `mapVorType`.
    pub fn vhf_navaid(&self, database_id: &str) -> Option<fms_host::facilities::VhfNavaid> {
        let v = self.facility(database_id).filter(|_| database_id.starts_with('V'))?;
        let dme = v.get("dme").filter(|d| d.is_object());
        Some(fms_host::facilities::VhfNavaid {
            database_id: s(&v, "icao").to_string(),
            ident: icao_ident(s(&v, "icao")),
            location: Location { lat: n(&v, "lat"), long: n(&v, "lon"), alt: None },
            dme_location: dme.map(|d| Location { lat: n(d, "lat"), long: n(d, "lon"), alt: None }),
            frequency: n(&v, "freqMHz"),
            figure_of_merit: match n(&v, "vorClass") as i64 {
                2 => 1,
                3 => 2,
                _ => 0,
            },
            vhf_type: vhf_type(n(&v, "type") as u8),
        })
    }

    pub fn ndb_navaid(&self, database_id: &str) -> Option<fms_host::facilities::NdbNavaid> {
        let v = self.facility(database_id).filter(|_| database_id.starts_with('N'))?;
        Some(fms_host::facilities::NdbNavaid {
            database_id: s(&v, "icao").to_string(),
            ident: icao_ident(s(&v, "icao")),
            location: Location { lat: n(&v, "lat"), long: n(&v, "lon"), alt: None },
            frequency: n(&v, "freqMHz"),
        })
    }

    pub fn nearby(&self, kind: fms_host::facilities::NearbyKind, centre: fms_lnav::Coord, radius_nm: f64, max: usize) -> Vec<fms_host::facilities::NearbyFacility> {
        use fms_host::facilities::NearbyKind;
        let letter = match kind {
            NearbyKind::Airport => 'A',
            NearbyKind::VhfNavaid => 'V',
            NearbyKind::NdbNavaid => 'N',
            NearbyKind::Waypoint => 'W',
        };
        self.nav
            .borrow()
            .nearby(letter, centre.lat, centre.long, radius_nm * NM, max)
            .into_iter()
            .map(|h| fms_host::facilities::NearbyFacility {
                database_id: h.icao,
                ident: h.ident,
                location: Location { lat: h.lat, long: h.lon, alt: None },
                section_code: match (letter, h.terminal) {
                    ('V', _) => 1,
                    ('A', _) | (_, true) => 4,
                    ('N', false) => 1,
                    _ => 2,
                },
                vhf_type: if letter == 'V' { vhf_type(h.vor_kind) } else { 0 },
            })
            .collect()
    }

    /// FBW `MsfsMapping.mapAirport`'s runway gradient, threshold crossing height, width and landing system ident.
    pub fn runway_details(&self, airport_ident: &str, runway_ident: &str) -> Option<fms_host::facilities::RunwayDetails> {
        let ap = self.airport_facility(airport_ident)?;
        let airport = icao_ident(s(&ap, "icao"));
        for r in array(&ap, "runways") {
            let length = n(r, "length");
            let gradient = ((n(r, "primaryElevation") - n(r, "secondaryElevation")) / (length - n(r, "primaryThresholdLength") - n(r, "secondaryThresholdLength"))).asin().to_degrees();
            for (index, designation) in s(r, "designation").split('-').enumerate() {
                let primary = index == 0;
                let code = n(r, if primary { "designatorCharPrimary" } else { "designatorCharSecondary" }) as i64;
                let Some(letter) = designator_char(code) else { continue };
                if format!("{airport}{designation:0>2}{letter}") != runway_ident {
                    continue;
                }
                let number: i64 = designation.trim().parse().unwrap_or(0);
                let ls = array(&ap, "approaches").iter().find(|a| {
                    n(a, "runwayNumber") as i64 == number && n(a, "runwayDesignator") as i64 == code && matches!(n(a, "approachType") as i64, 4..=7)
                });
                let ls_ident = ls.and_then(|a| array(a, "finalLegs").last()).map(|l| icao_ident(s(l, "originIcao")));
                return Some(fms_host::facilities::RunwayDetails {
                    gradient: if primary { gradient } else { -gradient },
                    threshold_crossing_height: 50. + n(r, if primary { "primaryElevation" } else { "secondaryElevation" }) / FT,
                    width: n(r, "width"),
                    ls_ident,
                });
            }
        }
        None
    }

    pub fn ils_at_airport(&self, airport_ident: &str, ils_ident: &str) -> Option<fms_host::facilities::IlsNavaid> {
        let ap = self.airport_facility(airport_ident)?;
        array(&ap, "approaches")
            .iter()
            .filter_map(|a| array(a, "finalLegs").last().map(|l| s(l, "originIcao").to_string()))
            .find(|icao| icao.starts_with('V') && icao_ident(icao) == ils_ident)
            .and_then(|icao| self.ils(&icao))
    }

    /// The intersection record of a waypoint, VOR or NDB: the one carrying its airway routes.
    fn intersection(&self, icao: &str) -> Option<Value> {
        if !matches!(icao.chars().next(), Some('W' | 'V' | 'N')) {
            return None;
        }
        self.load(icao, "LOAD_INTERSECTION", format!("I:{icao}"))
    }

    fn load(&self, icao: &str, call: &str, key: String) -> Option<Value> {
        if let Some(hit) = self.facilities.borrow().get(&key) {
            return hit.clone();
        }
        let args = serde_json::to_string(&[icao]).ok()?;
        let mut nav = self.nav.borrow_mut();
        let found = nav.call(call, &args).ok().is_some_and(|r| r.trim() == "true");
        let mut result = None;
        if found {
            for (name, payload) in nav.take_events_blocking(Duration::from_secs(10)) {
                if name.starts_with("Send") {
                    result = serde_json::from_str::<Value>(&payload).ok().and_then(|v| v.get(0).cloned());
                    break;
                }
            }
        }
        drop(nav);
        self.facilities.borrow_mut().insert(key, result.clone());
        result
    }

    fn search(&self, ident: &str) -> Vec<String> {
        let args = serde_json::to_string(&serde_json::json!([ident, 0, 100])).unwrap_or_default();
        let result = self.nav.borrow_mut().call("SEARCH_BY_IDENT", &args).unwrap_or_default();
        let list: Vec<String> = serde_json::from_str(&result).unwrap_or_default();
        list.into_iter().filter(|i| icao_ident(i) == ident).collect()
    }

    fn airport_fix(&self, ap: &Value) -> Fix {
        let icao = s(ap, "icao").to_string();
        let runways = array(ap, "runways");
        let mut longest = (0., None);
        let mut elevations = Vec::new();
        for r in runways {
            if n(r, "length") > longest.0 {
                longest = (n(r, "length"), Some(r));
            }
            elevations.push(n(r, "elevation") / FT);
        }
        let elevation = if elevations.is_empty() { n(ap, "altitude") / FT } else { elevations.iter().sum::<f64>() / elevations.len() as f64 };
        let transition_altitude = n(ap, "transitionAlt");
        let transition_level = n(ap, "transitionLevel");
        let ident = icao_ident(&icao);
        Fix {
            database_id: icao.clone(),
            ident: ident.clone(),
            icao_code: icao_region(&icao),
            location: Location { lat: n(ap, "lat"), long: n(ap, "lon"), alt: Some(elevation) },
            section_code: 4,
            sub_section_code: 0,
            area: 1,
            name: Some(s(ap, "name").to_string()),
            airport_ident: Some(ident),
            mag_var: Some(n(ap, "magvar")),
            transition_altitude: (transition_altitude > 0.).then(|| (transition_altitude / FT).round()),
            transition_level: (transition_level > 0.).then(|| (transition_level / 30.48).round()),
            longest_runway_length: Some(longest.0),
            longest_runway_surface_type: longest.1.map(|r| if matches!(n(r, "surface") as i64, 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9) { 2 } else { 1 }),
            station_declination: None,
        }
    }

    fn region_of(&self, ap: &Value) -> String {
        let region = ap.get("icaoStruct").map(|i| s(i, "region")).unwrap_or("");
        if region.len() == 2 {
            return region.to_string();
        }
        array(ap, "approaches")
            .iter()
            .filter_map(|a| array(a, "finalLegs").last())
            .map(|l| icao_region(s(l, "fixIcao")))
            .next()
            .unwrap_or_else(|| "  ".to_string())
    }

    fn airport_runways(&self, ap: &Value) -> Vec<Runway> {
        let mut out = Vec::new();
        let airport_ident = icao_ident(s(ap, "icao"));
        let icao_code = self.region_of(ap);
        let mag_var = n(ap, "magvar");
        for r in array(ap, "runways") {
            let primary_threshold = n(r, "primaryThresholdLength");
            let secondary_threshold = n(r, "secondaryThresholdLength");
            let length = n(r, "length");
            let gradient = ((n(r, "primaryElevation") - n(r, "secondaryElevation")) / (length - primary_threshold - secondary_threshold)).asin().to_degrees();
            for (index, designation) in s(r, "designation").split('-').enumerate() {
                let primary = index == 0;
                let code = n(r, if primary { "designatorCharPrimary" } else { "designatorCharSecondary" }) as i64;
                let (Some(designator), Some(letter)) = (runway_designator(code), designator_char(code)) else { continue };
                let number: i32 = designation.trim().parse().unwrap_or(0);
                let ident = format!("{airport_ident}{designation:0>2}{letter}");
                let bearing = if primary { n(r, "direction") } else { (n(r, "direction") + 180.) % 360. };
                let center = Coordinates::new(n(r, "latitude"), n(r, "longitude"));
                let start = center.place_bearing_distance((bearing + 180.) % 360., length / 2. / NM);
                let threshold_distance = if primary { primary_threshold } else { secondary_threshold };
                let threshold = if threshold_distance > 0. { start.place_bearing_distance(bearing, threshold_distance / NM) } else { start };
                let elevation = n(r, if primary { "primaryElevation" } else { "secondaryElevation" }) / FT;
                let location = Location { lat: threshold.lat, long: threshold.long, alt: Some(elevation) };
                out.push(Runway {
                    fix: Fix {
                        database_id: format!("R{icao_code}{ident}"),
                        ident,
                        icao_code: icao_code.clone(),
                        location,
                        section_code: 4,
                        sub_section_code: 6,
                        area: 1,
                        airport_ident: Some(airport_ident.clone()),
                        mag_var: Some(mag_var),
                        ..Fix::default()
                    },
                    bearing,
                    magnetic_bearing: (bearing - mag_var).rem_euclid(360.),
                    length,
                    number,
                    designator,
                    threshold_location: location,
                });
                let _ = gradient;
            }
        }
        out
    }

    /// An airway point's fix: a VOR or NDB from its own record, which has its declination.
    fn route_fix(&self, intersection: &Value) -> Fix {
        let icao = s(intersection, "icao");
        let own = if matches!(icao.chars().next(), Some('V' | 'N')) { self.facility(icao) } else { None };
        self.fix_from_facility(own.as_ref().unwrap_or(intersection))
    }

    fn fix_from_facility(&self, f: &Value) -> Fix {
        let icao = s(f, "icao").to_string();
        let airport = icao.get(3..7).unwrap_or("").trim().to_string();
        let terminal = !airport.is_empty();
        let kind = icao.chars().next().unwrap_or('W');
        let (section_code, sub_section_code) = match kind {
            'N' if terminal => (4, 11),
            'N' => (1, 1),
            'V' => (1, 0),
            _ if terminal => (4, 2),
            _ => (2, 0),
        };
        let declination = if kind == 'V' { Some(normalise180(360. - n(f, "magneticVariation"))) } else { None };
        Fix {
            database_id: icao.clone(),
            ident: icao_ident(&icao),
            icao_code: icao_region(&icao),
            location: Location { lat: n(f, "lat"), long: n(f, "lon"), alt: None },
            section_code,
            sub_section_code,
            area: if terminal { 1 } else { 0 },
            name: Some(s(f, "name").to_string()),
            airport_ident: terminal.then_some(airport),
            station_declination: declination,
            ..Fix::default()
        }
    }

    fn waypoint(&self, ap: &Value, runways: &[Runway], icao: &str) -> Option<Fix> {
        match icao.chars().next() {
            Some('R') => {
                let ident = format!("{}{}", icao_ident(s(ap, "icao")), icao.get(9..).unwrap_or("").trim());
                runways.iter().find(|r| r.fix.ident == ident).map(|r| r.fix.clone())
            }
            Some('A') => self.facility(icao).map(|f| self.airport_fix(&f)),
            Some('W' | 'V' | 'N') => self.facility(icao).map(|f| self.fix_from_facility(&f)),
            _ => None,
        }
    }

    fn leg(&self, ap: &Value, runways: &[Runway], leg: &Value, index: usize, procedure_ident: &str, approach: Option<i64>) -> Option<ProcedureLeg> {
        let waypoint = self.waypoint(ap, runways, s(leg, "fixIcao"));
        let navaid = self.waypoint(ap, runways, s(leg, "originIcao"));
        let flags = n(leg, "fixTypeFlags") as i64;
        let approach_descriptor = approach.map(|kind| {
            if index == 0 && matches!(kind, 4 | 7 | 5 | 6) {
                7
            } else if flags & 8 != 0 {
                5
            } else if flags & 1 != 0 {
                0
            } else if flags & 4 != 0 {
                8
            } else {
                0
            }
        });
        let speed = n(leg, "speedRestriction");
        let true_degrees = leg.get("trueDegrees").and_then(Value::as_bool).unwrap_or(false);
        let mut mag_var = if true_degrees { None } else { Some(n(ap, "magvar")) };
        if !true_degrees {
            for icao in [s(leg, "fixIcao"), s(leg, "originIcao")] {
                if icao.starts_with('V') {
                    if let Some(v) = self.facility(icao) {
                        mag_var = if v.get("trueReferenced").and_then(Value::as_bool).unwrap_or(false) { None } else { Some(normalise180(360. - n(&v, "magneticVariation"))) };
                    }
                    break;
                }
            }
        }
        let alt_desc = match n(leg, "altDesc") as i64 {
            1 => Some("@"),
            2 => Some("+"),
            3 => Some("-"),
            4 => Some("B"),
            _ => None,
        };
        let altitude_descriptor = match (approach, alt_desc) {
            (Some(4), Some(d)) if flags & 8 != 0 && d == "@" => Some("G"),
            (Some(4), Some(d)) if flags & 8 != 0 && d == "+" => Some("H"),
            (Some(4), Some(d)) if approach_descriptor == Some(7) && d == "@" => Some("I"),
            (Some(4), Some(d)) if approach_descriptor == Some(7) && d == "+" => Some("J"),
            (_, d) => d,
        };
        let distance_minutes = leg.get("distanceMinutes").and_then(Value::as_bool).unwrap_or(false);
        let vertical_angle = n(leg, "verticalAngle");
        Some(ProcedureLeg {
            procedure_ident: procedure_ident.to_string(),
            leg_type: leg_type(n(leg, "type") as i64)?.to_string(),
            waypoint_descriptor: Some(match s(leg, "fixIcao").chars().next() {
                Some('A') => 1,
                Some('N') => 5,
                Some('R') => 4,
                Some('V') => 9,
                Some(' ') | None => 0,
                _ => 2,
            }),
            waypoint,
            approach_waypoint_descriptor: approach_descriptor,
            overfly: leg.get("flyOver").and_then(Value::as_bool).unwrap_or(false),
            turn_direction: Some(match n(leg, "turnDirection") as i64 {
                1 => 'L',
                2 => 'R',
                _ => 'E',
            }),
            altitude_descriptor: altitude_descriptor.map(str::to_string),
            altitude1: Some((n(leg, "altitude1") / FT).round()),
            altitude2: Some((n(leg, "altitude2") / FT).round()),
            speed_descriptor: (speed > 0.).then(|| "-".to_string()),
            speed: (speed > 0.).then_some(speed),
            course: Some(n(leg, "course")),
            mag_var,
            rho: Some(n(leg, "rho") / NM),
            theta: Some(n(leg, "theta")),
            length: (!distance_minutes).then(|| n(leg, "distance") / NM),
            length_time: distance_minutes.then(|| n(leg, "distance")),
            recommended_navaid: navaid,
            vertical_angle: (vertical_angle.abs() > f64::EPSILON).then(|| vertical_angle - 360.),
            is_engine_out_branch: false,
        })
    }

    fn legs(&self, ap: &Value, runways: &[Runway], legs: &[Value], procedure_ident: &str, approach: Option<i64>) -> Vec<ProcedureLeg> {
        legs.iter().enumerate().filter_map(|(i, l)| self.leg(ap, runways, l, i, procedure_ident, approach)).collect()
    }

    fn procedures(&self, airport_icao: &str, kind: ProcedureKind) -> Vec<Procedure> {
        let Some(ap) = self.airport_facility(airport_icao) else { return Vec::new() };
        let runways = self.airport_runways(&ap);
        let airport_ident = icao_ident(s(&ap, "icao"));
        let region = self.region_of(&ap);
        let mut out = Vec::new();
        match kind {
            ProcedureKind::Approach => {
                for a in array(&ap, "approaches") {
                    let number = n(a, "runwayNumber") as i64;
                    let code = n(a, "runwayDesignator") as i64;
                    if number == 0 || runway_designator(code).is_none() {
                        continue;
                    }
                    let letter = designator_char(code).unwrap_or("");
                    let kind_code = n(a, "approachType") as i64;
                    let suffix = s(a, "approachSuffix");
                    let suffix_text = if !suffix.is_empty() {
                        format!("{}{suffix}", if letter.is_empty() { "-" } else { letter })
                    } else if code > 0 {
                        letter.to_string()
                    } else {
                        String::new()
                    };
                    let ident = format!("{}{number:02}{suffix_text}", approach_prefix(kind_code));
                    let transitions = array(a, "transitions")
                        .iter()
                        .enumerate()
                        .map(|(i, t)| Transition { database_id: format!("{ident}{i}"), ident: s(t, "name").to_string(), legs: self.legs(&ap, &runways, array(t, "legs"), &ident, None) })
                        .collect();
                    out.push(Procedure {
                        kind,
                        database_id: format!("P{region}{airport_ident}{}", s(a, "name")),
                        ident: ident.clone(),
                        runway_transitions: Vec::new(),
                        common_legs: self.legs(&ap, &runways, array(a, "finalLegs"), &ident, Some(kind_code)),
                        enroute_transitions: transitions,
                        missed_legs: self.legs(&ap, &runways, array(a, "missedLegs"), &ident, None),
                        runway_ident: Some(format!("{airport_ident}{number:02}{letter}")),
                        approach_type: approach_type(kind_code),
                        multiple_indicator: suffix.to_string(),
                    });
                }
            }
            _ => {
                let key = if kind == ProcedureKind::Departure { "departures" } else { "arrivals" };
                for p in array(&ap, key) {
                    let name = s(p, "name");
                    let runway_transitions = array(p, "runwayTransitions")
                        .iter()
                        .enumerate()
                        .filter_map(|(i, t)| {
                            let code = n(t, "runwayDesignation") as i64;
                            let letter = designator_char(code).unwrap_or("");
                            Some(Transition {
                                database_id: format!("{name}{i}"),
                                ident: format!("{airport_ident}{:02}{letter}", n(t, "runwayNumber") as i64),
                                legs: self.legs(&ap, &runways, array(t, "legs"), name, None),
                            })
                        })
                        .collect();
                    let enroute_transitions = array(p, "enRouteTransitions")
                        .iter()
                        .enumerate()
                        .map(|(i, t)| Transition { database_id: format!("{name}{i}"), ident: s(t, "name").to_string(), legs: self.legs(&ap, &runways, array(t, "legs"), name, None) })
                        .collect();
                    out.push(Procedure {
                        kind,
                        database_id: format!("P{region}{airport_ident}{name}"),
                        ident: name.to_string(),
                        runway_transitions,
                        common_legs: self.legs(&ap, &runways, array(p, "commonLegs"), name, None),
                        enroute_transitions,
                        ..Procedure::default()
                    });
                }
            }
        }
        out
    }

    fn airport_facility(&self, airport_icao: &str) -> Option<Value> {
        self.facility(&format!("A      {airport_icao:<5}"))
    }
}

impl NavDatabase for FmsNavDb {
    fn airport(&self, icao: &str) -> Option<Fix> {
        self.airport_facility(icao).map(|a| self.airport_fix(&a))
    }

    fn runways(&self, airport_icao: &str) -> Vec<Runway> {
        self.airport_facility(airport_icao).map(|a| self.airport_runways(&a)).unwrap_or_default()
    }

    fn departures(&self, airport_icao: &str) -> Vec<Procedure> {
        let mut departures = self.procedures(airport_icao, ProcedureKind::Departure);
        match_engine_out_departures(&mut departures);
        departures
    }

    fn engine_out_departure(&self, airport_icao: &str, departure_database_id: &str, runway_ident: &str) -> Option<Procedure> {
        engine_out_departure_from(&self.procedures(airport_icao, ProcedureKind::Departure), departure_database_id, runway_ident)
    }

    fn arrivals(&self, airport_icao: &str) -> Vec<Procedure> {
        self.procedures(airport_icao, ProcedureKind::Arrival)
    }

    fn approaches(&self, airport_icao: &str) -> Vec<Procedure> {
        self.procedures(airport_icao, ProcedureKind::Approach)
    }

    fn search_fix(&self, ident: &str) -> Vec<Fix> {
        self.search(ident)
            .into_iter()
            .filter(|i| !i.starts_with('A'))
            .filter_map(|i| self.facility(&i))
            .map(|f| self.fix_from_facility(&f))
            .collect()
    }

    fn search_airway(&self, ident: &str, via: &Fix) -> Vec<Airway> {
        let candidates: Vec<Value> = self
            .search(&via.ident)
            .into_iter()
            .filter(|i| icao_region(i) == via.icao_code)
            .filter_map(|i| self.intersection(&i))
            .collect();
        let Some(best) = candidates.into_iter().find(|f| array(f, "routes").iter().any(|r| s(r, "name") == ident)) else { return Vec::new() };
        let here = self.route_fix(&best);
        let mut out = Vec::new();
        for route in array(&best, "routes").iter().filter(|r| s(r, "name") == ident) {
            let mut previous: Vec<Fix> = Vec::new();
            let mut next: Vec<Fix> = Vec::new();
            let mut prev_icao = s(route, "prevIcao").trim().to_string();
            let mut next_icao = s(route, "nextIcao").trim().to_string();
            let mut guard = 0;
            while (!prev_icao.is_empty() || !next_icao.is_empty()) && guard < 250 {
                guard += 1;
                if !prev_icao.is_empty() {
                    let Some(f) = self.intersection(&prev_icao) else { break };
                    previous.insert(0, self.route_fix(&f));
                    prev_icao = array(&f, "routes").iter().find(|r| s(r, "name") == ident).map(|r| s(r, "prevIcao").trim().to_string()).unwrap_or_default();
                }
                if !next_icao.is_empty() {
                    let Some(f) = self.intersection(&next_icao) else { break };
                    next.push(self.route_fix(&f));
                    next_icao = array(&f, "routes").iter().find(|r| s(r, "name") == ident).map(|r| s(r, "nextIcao").trim().to_string()).unwrap_or_default();
                }
            }
            let mut fixes = previous;
            fixes.push(here.clone());
            fixes.extend(next);
            out.push(Airway { ident: ident.to_string(), fixes });
        }
        out
    }

    fn navaids(&self, ident: &str) -> Vec<Fix> {
        self.search_fix(ident).into_iter().filter(|f| f.section_code == 1).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn xplane_root() -> Option<PathBuf> {
        let root = std::env::var("FMS_XPLANE_ROOT").unwrap_or_else(|_| r"D:\Steam Games\steamapps\common\X-Plane 12".to_string());
        Path::new(&root).join("Resources").is_dir().then(|| PathBuf::from(root))
    }

    #[test]
    fn real_navdata_resolves_airports_a_sid_and_an_airway() {
        let Some(root) = xplane_root() else {
            eprintln!("X-Plane 12 is not installed; skipping");
            return;
        };
        let db = FmsNavDb::new(NavData::load(&root).expect("load")).expect("ready");

        let egll = db.airport("EGLL").expect("EGLL");
        assert_eq!(egll.ident, "EGLL");
        assert!((egll.location.lat - 51.47).abs() < 0.05);
        let kjfk = db.airport("KJFK").expect("KJFK");
        assert!((kjfk.location.long + 73.78).abs() < 0.05);

        let runways = db.runways("EGLL");
        assert!(runways.iter().any(|r| r.fix.ident == "EGLL09L"));
        assert!(runways.iter().any(|r| r.fix.ident == "EGLL27R"));

        let departures = db.departures("EGLL");
        assert!(!departures.is_empty());
        let sid = departures.iter().find(|d| !d.common_legs.is_empty() || d.runway_transitions.iter().any(|t| !t.legs.is_empty())).expect("a SID with legs");
        assert!(sid.runway_transitions.iter().all(|t| t.ident.starts_with("EGLL")));

        let arrivals = db.arrivals("KJFK");
        assert!(!arrivals.is_empty());
        let approaches = db.approaches("KJFK");
        assert!(approaches.iter().any(|a| a.runway_ident.as_deref().is_some_and(|r| r.starts_with("KJFK"))));

        let ipnox = db.search_fix("CPT");
        assert!(!ipnox.is_empty(), "CPT resolves");
        let un57_fix = db.search_fix("IPNOX").pop().expect("IPNOX");
        let airways = db.search_airway("UN57", &un57_fix);
        assert!(!airways.is_empty());
        assert!(airways[0].fixes.len() > 2);
        assert!(airways[0].fixes.iter().any(|f| f.ident == "IPNOX"));
    }
}

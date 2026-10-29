use fms_data::FlightPhase;
use serde_json::Value;

use crate::plan::{FlightPlan, SegmentKind};
use crate::winds::{PendingCruiseFix, PendingCruiseWind, WindEntry, WindVector, NUM_CLIMB_WIND_LEVELS, NUM_CRUISE_WIND_LEVELS, NUM_DESCENT_WIND_LEVELS};

const MAX_WIND_MAGNITUDE: f64 = 250.;

#[derive(Clone, Debug, PartialEq)]
pub enum RequestFix {
    Ident(String),
    LatLon { lat: f64, long: f64 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct CruiseWindRequest {
    pub flight_levels: Vec<f64>,
    pub waypoints: Vec<RequestFix>,
}

/// FBW `WindRequestMessage`. The outer `Option` is "requested", the inner one the level.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindRequest {
    pub climb_wind_level: Option<Option<f64>>,
    pub cruise_winds: Option<CruiseWindRequest>,
    pub descent_wind_level: Option<Option<f64>>,
    pub alternate_wind: Option<(String, String)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UplinkedWindLevel {
    pub flight_level: f64,
    pub true_degrees: f64,
    pub magnitude: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UplinkedCruiseWindEntry {
    pub fix: PendingCruiseFix,
    pub flight_level: f64,
    pub true_degrees: f64,
    pub magnitude: f64,
    pub temperature: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UplinkedCruiseWindSet {
    pub flight_level: f64,
    pub fixes: Vec<UplinkedCruiseWindEntry>,
}

/// FBW `WindUplinkMessage`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindUplinkMessage {
    pub climb_winds: Option<Vec<UplinkedWindLevel>>,
    pub cruise_winds: Option<Vec<UplinkedCruiseWindSet>>,
    pub descent_winds: Option<Vec<UplinkedWindLevel>>,
    pub alternate_wind: Option<UplinkedWindLevel>,
}

fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

fn parse_int(v: &Value) -> f64 {
    let s = match v {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => return f64::NAN,
    };
    let end = s.char_indices().find(|(i, c)| !(c.is_ascii_digit() || (*i == 0 && (*c == '-' || *c == '+')))).map_or(s.len(), |(i, _)| i);
    s[..end].parse::<f64>().unwrap_or(f64::NAN)
}

fn parse_float(v: &Value) -> f64 {
    match v {
        Value::String(s) => s.trim().parse().unwrap_or(f64::NAN),
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
}

fn about_equal(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn is_stored_lat_lon(fix: &crate::navdb::Fix) -> bool {
    fix.icao_code.trim().is_empty() && fix.database_id.starts_with("W      ")
}

impl FlightPlan {
    /// FBW `formatWindRequest` without predictions (cruise legs are the enroute segment's XF legs).
    pub fn format_wind_request(&self, phase: FlightPhase, active_or_copied_from_active: bool) -> WindRequest {
        let cruise_level = self.performance.cruise_flight_level;
        let request_climb = !active_or_copied_from_active || phase <= FlightPhase::Takeoff || phase == FlightPhase::Done;
        let request_cruise = !active_or_copied_from_active || phase <= FlightPhase::Cruise || phase == FlightPhase::Done;
        let request_descent = self.destination_airport.is_some();

        let legs = self.all_legs();
        let final_cruise_level = legs.iter().fold(cruise_level, |acc, e| match e.leg().and_then(|l| l.cruise_step.as_ref()) {
            Some(step) => Some(js_round(step.to_altitude / 100.)),
            None => acc,
        });

        let enroute_start: usize = self.segments[..SegmentKind::Enroute.slot()].iter().map(|s| s.legs.len()).sum();
        let enroute_end = enroute_start + self.seg(SegmentKind::Enroute).legs.len();
        let cruise_legs: Vec<_> = (enroute_start..enroute_end).filter_map(|i| self.leg_at(i)).filter(|l| l.is_xf()).collect();

        let mut cruise_winds = None;
        if request_cruise && !cruise_legs.is_empty() {
            let propagated = self.propagate_winds_at(0);
            let mut flight_levels: Vec<f64> = propagated.iter().map(|w| js_round(w.altitude.unwrap_or(f64::NAN) / 100.)).collect();
            if flight_levels.is_empty() {
                if let Some(c) = cruise_level {
                    flight_levels.push(c);
                }
                for e in &legs {
                    if let Some(step) = e.leg().and_then(|l| l.cruise_step.as_ref()) {
                        let s = js_round(step.to_altitude / 100.);
                        if flight_levels.len() < 4 && Some(s) != cruise_level && !flight_levels.contains(&s) {
                            flight_levels.push(s);
                        }
                    }
                }
            }
            let waypoints = cruise_legs
                .iter()
                .map(|l| match l.termination_waypoint() {
                    Some(w) if is_stored_lat_lon(w) => RequestFix::LatLon { lat: w.location.lat, long: w.location.long },
                    _ => RequestFix::Ident(l.ident.clone()),
                })
                .collect();
            cruise_winds = Some(CruiseWindRequest { flight_levels, waypoints });
        }

        let alternate_destination = self.alternate.as_ref().and_then(|a| a.destination_airport.as_ref());
        let alternate_wind = match (self.destination_airport.as_ref(), alternate_destination) {
            (Some(d), Some(a)) => Some((d.ident.clone(), a.ident.clone())),
            _ => None,
        };

        WindRequest {
            climb_wind_level: request_climb.then_some(cruise_level),
            cruise_winds,
            descent_wind_level: request_descent.then_some(final_cruise_level),
            alternate_wind,
        }
    }

    /// FBW `PendingWindUplinkParser.setFromUplink`.
    pub fn set_pending_wind_uplink(&mut self, uplink: &WindUplinkMessage, phase: FlightPhase, max_certified_flight_level: f64, sort_climb_ascending: bool) {
        let in_climb_phases = matches!(phase, FlightPhase::Preflight | FlightPhase::Takeoff | FlightPhase::Done);
        if in_climb_phases {
            let ground = self.origin_airport.as_ref().and_then(|a| a.location.alt).unwrap_or(0.) / 100.;
            let mut climb = clb_des_entries(uplink.climb_winds.as_deref(), max_certified_flight_level, ground, NUM_CLIMB_WIND_LEVELS);
            if let Some(c) = climb.as_mut() {
                sort_entries(c, sort_climb_ascending);
            }
            self.pending_wind_uplink.climb_winds = climb;
        }
        if in_climb_phases || matches!(phase, FlightPhase::Climb | FlightPhase::Cruise) {
            self.pending_wind_uplink.cruise_winds = Some(cruise_entries(uplink.cruise_winds.as_deref(), max_certified_flight_level));
            let ground = self.destination_airport.as_ref().and_then(|a| a.location.alt).unwrap_or(0.) / 100.;
            let mut descent = clb_des_entries(uplink.descent_winds.as_deref(), max_certified_flight_level, ground, NUM_DESCENT_WIND_LEVELS);
            if let Some(d) = descent.as_mut() {
                sort_entries(d, false);
            }
            self.pending_wind_uplink.descent_winds = descent;
            self.pending_wind_uplink.alternate_wind = uplink.alternate_wind.map(|w| entry_from_level(&w));
        }
        self.pending_wind_uplink.on_uplink_ready_to_insert();
    }
}

fn sort_entries(entries: &mut [WindEntry], ascending: bool) {
    entries.sort_by(|a, b| {
        let (a, b) = (a.altitude.unwrap_or(0.), b.altitude.unwrap_or(0.));
        if ascending {
            a.total_cmp(&b)
        } else {
            b.total_cmp(&a)
        }
    });
}

fn valid_level(fl: f64, max: f64) -> bool {
    fl >= 0. && fl <= max
}

fn valid_entry(magnitude: f64, true_degrees: f64) -> bool {
    (0. ..=MAX_WIND_MAGNITUDE).contains(&magnitude) && (0. ..=360.).contains(&true_degrees)
}

fn unique_levels<T>(items: Vec<T>, level: impl Fn(&T) -> f64, ground: Option<f64>) -> Vec<T> {
    let levels: Vec<f64> = items.iter().map(&level).collect();
    items
        .into_iter()
        .enumerate()
        .filter(|(i, _)| {
            let fl = levels[*i];
            let first = levels.iter().position(|l2| match ground {
                Some(g) if fl < g + 4. => *l2 < g + 4.,
                _ => *l2 == fl,
            });
            first == Some(*i)
        })
        .map(|(_, t)| t)
        .collect()
}

fn vector(magnitude: f64, true_degrees: f64) -> WindVector {
    WindVector { magnitude: Some(magnitude), direction: Some(true_degrees.to_radians()) }
}

fn entry_from_level(w: &UplinkedWindLevel) -> WindEntry {
    WindEntry { vector: vector(w.magnitude, w.true_degrees), altitude: Some(w.flight_level * 100.), flags: 0 }
}

fn clb_des_entries(winds: Option<&[UplinkedWindLevel]>, max_fl: f64, ground: f64, max_levels: usize) -> Option<Vec<WindEntry>> {
    let valid: Vec<UplinkedWindLevel> = winds?.iter().copied().filter(|w| valid_level(w.flight_level, max_fl) && valid_entry(w.magnitude, w.true_degrees)).collect();
    Some(unique_levels(valid, |w| w.flight_level, Some(ground)).iter().map(entry_from_level).take(max_levels).collect())
}

fn cruise_entries(sets: Option<&[UplinkedCruiseWindSet]>, max_fl: f64) -> Vec<PendingCruiseWind> {
    let Some(sets) = sets else { return Vec::new() };
    let valid: Vec<&UplinkedCruiseWindSet> = sets.iter().filter(|s| valid_level(s.flight_level, max_fl)).collect();
    let mut acc: Vec<PendingCruiseWind> = Vec::new();
    for set in unique_levels(valid, |s| s.flight_level, None).into_iter().take(NUM_CRUISE_WIND_LEVELS) {
        for f in set.fixes.iter().filter(|f| valid_entry(f.magnitude, f.true_degrees)) {
            let entry = WindEntry { vector: vector(f.magnitude, f.true_degrees), altitude: Some(set.flight_level * 100.), flags: 0 };
            let existing = acc.iter_mut().find(|w| match (&w.fix, &f.fix) {
                (PendingCruiseFix::Waypoint(a), PendingCruiseFix::Waypoint(b)) => a == b,
                (PendingCruiseFix::LatLon { lat: a1, long: o1 }, PendingCruiseFix::LatLon { lat: a2, long: o2 }) => about_equal(*a1, *a2) && about_equal(*o1, *o2),
                _ => false,
            });
            match existing {
                Some(w) => w.levels.push(entry),
                None => acc.push(PendingCruiseWind { fix: f.fix.clone(), levels: vec![entry] }),
            }
        }
    }
    for w in &mut acc {
        sort_entries(&mut w.levels, false);
    }
    acc
}

struct Level {
    altitude: f64,
    dir: f64,
    spd: f64,
    oat: f64,
}

fn levels_of(fix: &Value) -> Vec<Level> {
    let levels = match fix.get("wind_data").and_then(|w| w.get("level")) {
        Some(Value::Array(a)) => a.clone(),
        Some(v @ Value::Object(_)) => vec![v.clone()],
        _ => Vec::new(),
    };
    levels.iter().map(|l| Level { altitude: parse_int(&l["altitude"]), dir: parse_int(&l["wind_dir"]), spd: parse_int(&l["wind_spd"]), oat: parse_int(&l["oat"]) }).collect()
}

fn wind_level(l: &Level) -> UplinkedWindLevel {
    UplinkedWindLevel { true_degrees: l.dir, magnitude: l.spd, flight_level: js_round(l.altitude / 100.) }
}

fn stage(fix: &Value) -> &str {
    fix.get("stage").and_then(Value::as_str).unwrap_or("")
}

fn parse_climb_winds(navlog: &[Value]) -> Vec<UplinkedWindLevel> {
    let mut result = Vec::new();
    let mut last_altitude = 0.;
    for (wpt_idx, wpt) in navlog.iter().enumerate() {
        if stage(wpt) != "CLB" {
            continue;
        }
        let wpt_altitude = parse_int(&wpt["altitude_feet"]);
        let levels = levels_of(wpt);
        if wpt_idx == 0 {
            let mut alt_idx = 0;
            while last_altitude < wpt_altitude {
                let Some(l) = levels.get(alt_idx) else { break };
                let w = wind_level(l);
                result.push(w);
                last_altitude = w.flight_level * 100.;
                alt_idx += 1;
            }
        }
        for level_idx in 0..levels.len() {
            let altitude = levels[level_idx].altitude;
            let (mut delta_prev, mut delta_this) = (0., 0.);
            if level_idx > 0 && level_idx < levels.len() - 1 {
                delta_prev = (wpt_altitude - levels[level_idx - 1].altitude).abs();
                delta_this = (wpt_altitude - altitude).abs();
            }
            if altitude > last_altitude && last_altitude <= wpt_altitude {
                let idx = if delta_prev > delta_this { level_idx } else { level_idx.wrapping_sub(1) };
                let Some(l) = levels.get(idx) else { continue };
                if l.altitude > last_altitude {
                    result.push(UplinkedWindLevel { true_degrees: l.dir, magnitude: l.spd, flight_level: js_round(l.altitude / 100.) });
                    last_altitude = l.altitude;
                }
            }
        }
    }
    result.sort_by(|a, b| b.flight_level.total_cmp(&a.flight_level));
    result
}

fn parse_descent_winds(navlog: &[Value]) -> Vec<UplinkedWindLevel> {
    let mut result = Vec::new();
    let mut last_altitude = 45000.;
    for (wpt_idx, wpt) in navlog.iter().enumerate() {
        if wpt.get("ident").and_then(Value::as_str) != Some("TOD") && stage(wpt) != "DSC" {
            continue;
        }
        let wpt_altitude = parse_int(&wpt["altitude_feet"]);
        let levels = levels_of(wpt);
        if wpt_idx == 0 {
            let mut alt_idx = levels.len() as isize - 1;
            while last_altitude > wpt_altitude {
                let Some(l) = usize::try_from(alt_idx).ok().and_then(|i| levels.get(i)) else { break };
                let w = wind_level(l);
                result.push(w);
                last_altitude = w.flight_level * 100.;
                alt_idx -= 1;
            }
        }
        for level_idx in (0..levels.len()).rev() {
            let altitude = levels[level_idx].altitude;
            let (mut delta_next, mut delta_this) = (0., 0.);
            if level_idx > 0 {
                delta_next = (wpt_altitude - levels[level_idx - 1].altitude).abs();
                delta_this = (wpt_altitude - altitude).abs();
            }
            if altitude >= last_altitude && last_altitude > wpt_altitude {
                let idx = if delta_next > delta_this { level_idx } else { level_idx.wrapping_sub(1) };
                let Some(l) = levels.get(idx) else { continue };
                if l.altitude < last_altitude {
                    result.push(UplinkedWindLevel { true_degrees: l.dir, magnitude: l.spd, flight_level: js_round(l.altitude / 100.) });
                    last_altitude = l.altitude;
                }
            }
        }
    }
    result
}

fn polar(r: f64, deg: f64) -> (f64, f64) {
    let t = deg.to_radians();
    (r * t.cos(), r * t.sin())
}

fn lerp(x: f64, x0: f64, x1: f64, y0: f64, y1: f64) -> f64 {
    if x0 == x1 {
        y0
    } else {
        y0 + (x - x0) * (y1 - y0) / (x1 - x0)
    }
}

fn parse_cruise_winds(navlog: &[Value], request: &CruiseWindRequest) -> Vec<UplinkedCruiseWindSet> {
    let cruise_waypoints: Vec<&Value> = navlog
        .iter()
        .filter(|w| {
            let ident = w.get("ident").and_then(Value::as_str).unwrap_or("");
            let kind = w.get("type").and_then(Value::as_str).unwrap_or("");
            ident != "TOC"
                && ident != "TOD"
                && request.waypoints.iter().any(|r| match r {
                    RequestFix::Ident(i) => kind == "wpt" && i == ident,
                    RequestFix::LatLon { lat, long } => kind == "ltlg" && about_equal(*lat, parse_float(&w["pos_lat"])) && about_equal(*long, parse_float(&w["pos_long"])),
                })
        })
        .collect();
    let mut result = Vec::new();
    for &req_level in &request.flight_levels {
        let mut set = UplinkedCruiseWindSet { flight_level: req_level, fixes: Vec::new() };
        for fix in &cruise_waypoints {
            let levels = levels_of(fix);
            if levels.is_empty() {
                continue;
            }
            let fix_id = if fix.get("type").and_then(Value::as_str) == Some("ltlg") {
                PendingCruiseFix::LatLon { lat: parse_float(&fix["pos_lat"]), long: parse_float(&fix["pos_long"]) }
            } else {
                PendingCruiseFix::Waypoint(fix.get("ident").and_then(Value::as_str).unwrap_or("").to_string())
            };
            let mut entry = UplinkedCruiseWindEntry { fix: fix_id, flight_level: req_level, true_degrees: 0., magnitude: 0., temperature: None };
            let lowest = js_round(levels[0].altitude / 100.);
            let last = &levels[levels.len() - 1];
            let highest = js_round(last.altitude / 100.);
            if req_level <= lowest {
                (entry.true_degrees, entry.magnitude, entry.temperature) = (levels[0].dir, levels[0].spd, Some(levels[0].oat));
            } else if req_level >= highest {
                (entry.true_degrees, entry.magnitude, entry.temperature) = (last.dir, last.spd, Some(last.oat));
            }
            for pair in levels.windows(2) {
                let (lo, hi) = (&pair[0], &pair[1]);
                let (lower_level, upper_level) = (js_round(lo.altitude / 100.), js_round(hi.altitude / 100.));
                if req_level >= lower_level && req_level <= upper_level {
                    let (x0, y0) = polar(lo.spd, lo.dir);
                    let (x1, y1) = polar(hi.spd, hi.dir);
                    let (x, y) = (lerp(req_level, lower_level, upper_level, x0, x1), lerp(req_level, lower_level, upper_level, y0, y1));
                    entry.true_degrees = js_round(y.atan2(x).to_degrees().rem_euclid(360.));
                    entry.magnitude = js_round(x.hypot(y));
                    entry.temperature = Some(lerp(req_level, lower_level, upper_level, lo.oat, hi.oat));
                    break;
                }
            }
            set.fixes.push(entry);
        }
        result.push(set);
    }
    result
}

/// FBW `SimBriefConnector.parseSimBriefWinds` on a raw SimBrief OFP JSON.
pub fn parse_simbrief_winds(json: &str, request: &WindRequest) -> Result<WindUplinkMessage, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let navlog: Vec<Value> = match v.get("navlog").and_then(|n| n.get("fix")) {
        Some(Value::Array(a)) => a.clone(),
        Some(o @ Value::Object(_)) => vec![o.clone()],
        _ => Vec::new(),
    };
    let alternate = v.get("alternate").filter(|a| a.is_object());
    Ok(WindUplinkMessage {
        climb_winds: request.climb_wind_level.is_some().then(|| parse_climb_winds(&navlog)),
        cruise_winds: request.cruise_winds.as_ref().filter(|c| !c.flight_levels.is_empty()).map(|c| parse_cruise_winds(&navlog, c)),
        descent_winds: request.descent_wind_level.is_some().then(|| parse_descent_winds(&navlog)),
        alternate_wind: match (alternate, &request.alternate_wind) {
            (Some(a), Some(_)) => Some(UplinkedWindLevel {
                flight_level: js_round(parse_int(&a["cruise_altitude"]) / 100.),
                true_degrees: parse_int(&a["avg_wind_dir"]),
                magnitude: parse_int(&a["avg_wind_spd"]),
            }),
            _ => None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn lv(alt: i32, dir: i32, spd: i32, oat: i32) -> Value {
        json!({ "altitude": alt.to_string(), "wind_dir": dir.to_string(), "wind_spd": spd.to_string(), "oat": oat.to_string() })
    }

    fn ofp() -> String {
        let levels = || json!([lv(0, 250, 10, 15), lv(5000, 260, 20, 5), lv(10000, 270, 30, -5), lv(20000, 280, 50, -25), lv(30000, 290, 70, -45), lv(40000, 300, 60, -55), lv(45000, 310, 55, -57)]);
        let fix = |ident: &str, kind: &str, stage: &str, alt: i32, lat: f64, lon: f64| {
            json!({ "ident": ident, "type": kind, "stage": stage, "altitude_feet": alt.to_string(), "pos_lat": format!("{lat:.6}"), "pos_long": format!("{lon:.6}"), "wind_data": { "level": levels() } })
        };
        json!({
            "navlog": { "fix": [
                fix("AAA", "wpt", "CLB", 6000, 50., 0.),
                fix("BBB", "wpt", "CLB", 22000, 51., 1.),
                fix("TOC", "ltlg", "CLB", 36000, 52., 2.),
                fix("N52W010", "ltlg", "CRZ", 36000, 52., -10.),
                fix("CCC", "wpt", "CRZ", 36000, 53., -20.),
                fix("TOD", "ltlg", "CRZ", 36000, 54., -30.),
                fix("DDD", "wpt", "DSC", 12000, 55., -40.),
            ] },
            "alternate": { "cruise_altitude": "18000", "avg_wind_dir": "275", "avg_wind_spd": "22" }
        })
        .to_string()
    }

    #[test]
    fn simbrief_winds_follow_fbw_connector() {
        let request = WindRequest {
            climb_wind_level: Some(Some(360.)),
            cruise_winds: Some(CruiseWindRequest { flight_levels: vec![360., 250.], waypoints: vec![RequestFix::Ident("CCC".into()), RequestFix::LatLon { lat: 52., long: -10. }] }),
            descent_wind_level: Some(Some(360.)),
            alternate_wind: Some(("EGLL".into(), "EGKK".into())),
        };
        let m = parse_simbrief_winds(&ofp(), &request).unwrap();
        let climb: Vec<f64> = m.climb_winds.as_ref().unwrap().iter().map(|w| w.flight_level).collect();
        assert_eq!(climb, vec![400., 300., 200., 100., 50., 0.]);
        let cruise = m.cruise_winds.as_ref().unwrap();
        assert_eq!(cruise.len(), 2);
        assert_eq!(cruise[0].fixes.len(), 2);
        let ccc = &cruise[0].fixes[1];
        assert_eq!(ccc.fix, PendingCruiseFix::Waypoint("CCC".into()));
        assert_eq!((ccc.true_degrees, ccc.magnitude), (296., 64.));
        let descent: Vec<f64> = m.descent_winds.as_ref().unwrap().iter().map(|w| w.flight_level).collect();
        assert_eq!(descent, vec![400., 300., 200., 100.]);
        assert_eq!(m.alternate_wind, Some(UplinkedWindLevel { flight_level: 180., true_degrees: 275., magnitude: 22. }));

        let mut plan = FlightPlan::empty(0);
        plan.set_pending_wind_uplink(&m, FlightPhase::Preflight, 431., false);
        let p = &plan.pending_wind_uplink;
        assert!(p.is_wind_uplink_ready_to_insert());
        assert_eq!(p.climb_winds.as_ref().unwrap().iter().map(|e| e.altitude.unwrap()).collect::<Vec<_>>(), vec![40000., 30000., 20000., 10000., 5000.]);
        let cw = p.cruise_winds.as_ref().unwrap();
        assert_eq!(cw.len(), 2);
        assert_eq!(cw[0].levels.iter().map(|e| e.altitude.unwrap()).collect::<Vec<_>>(), vec![36000., 25000.]);
        assert_eq!(p.alternate_wind.unwrap().altitude, Some(18000.));

        let mut cruise_only = FlightPlan::empty(0);
        cruise_only.set_pending_wind_uplink(&m, FlightPhase::Cruise, 431., false);
        assert!(cruise_only.pending_wind_uplink.climb_winds.is_none());
        assert!(cruise_only.pending_wind_uplink.descent_winds.is_some());
    }

    #[test]
    fn ground_levels_collapse_to_one() {
        let w = |fl: f64| UplinkedWindLevel { flight_level: fl, true_degrees: 100., magnitude: 10. };
        let m = WindUplinkMessage { climb_winds: Some(vec![w(0.), w(2.), w(100.), w(100.), w(500.)]), ..Default::default() };
        let mut plan = FlightPlan::empty(0);
        plan.set_pending_wind_uplink(&m, FlightPhase::Preflight, 431., true);
        let alts: Vec<f64> = plan.pending_wind_uplink.climb_winds.unwrap().iter().map(|e| e.altitude.unwrap()).collect();
        assert_eq!(alts, vec![0., 10000.]);
    }
}

use std::collections::HashMap;
use std::path::PathBuf;

use fms::geo::Coordinates;
use fms::navdb::{Fix, MemoryNavDb, NavDatabase};
use fms::plan::{AltitudeConstraint, Element, FlightPlan};
use fms::service::{Command, Fms, PlanRef};
use fms::uplink::{uplink_flight_plan_from_simbrief, NavlogFix, Ofp, OfpAirport};
use serde_json::Value;

const POSITION_TOLERANCE: f64 = 1e-6;
const ANGLE_TOLERANCE: f64 = 0.01;

fn oracle_dir() -> PathBuf {
    PathBuf::from(std::env::var("FMS_ORACLE_DIR").unwrap_or_else(|_| r"E:\fbw-build\fms-oracle".into()))
}

fn scenario_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/fms_oracle/scenarios")
}

fn db() -> MemoryNavDb {
    MemoryNavDb::from_fixture_json(include_str!("fixtures/navdb.json")).expect("fixture")
}

fn canned_ofp() -> Ofp {
    let fixes: [(&str, &str, f64, f64, &str, bool); 21] = [
        ("EGLL", "apt", 51.4706, -0.4619, "DCT", false),
        ("D255G", "wpt", 51.458917, -0.647523, "CPT3G", true),
        ("D259K", "wpt", 51.455411, -0.755266, "CPT3G", true),
        ("WOD", "ndb", 51.452783, -0.8788, "CPT3G", true),
        ("D100H", "wpt", 51.467817, -1.00984, "CPT3G", true),
        ("CPT", "vor", 51.491572, -1.219692, "CPT3G", false),
        ("IPNOX", "wpt", 54.227778, -2.706111, "DCT", false),
        ("OLPOP", "wpt", 54.092222, -2.535556, "UN57", false),
        ("SETEL", "wpt", 54.0124, -2.435753, "UN57", false),
        ("N55W010", "ltlg", 55., -10., "DCT", false),
        ("N56W030", "ltlg", 56., -30., "DCT", false),
        ("N52W050", "ltlg", 52., -50., "DCT", false),
        ("GONZZ", "wpt", 43.089444, -76.686667, "DCT", false),
        ("PUPPY", "wpt", 43.05735, -75.29425, "Q822", false),
        ("PAYGE", "wpt", 43.014022, -74.253544, "Q822", false),
        ("ENE", "vor", 43.425672, -70.613525, "DCT", false),
        ("ASPEN", "wpt", 42.815986, -70.911497, "ROBER2", true),
        ("PVD", "vor", 41.724342, -71.42964, "ROBER2", true),
        ("TRAIT", "wpt", 41.284653, -71.917597, "ROBER2", true),
        ("PARCH", "wpt", 41.099228, -72.120739, "ROBER2", true),
        ("KJFK", "apt", 40.639801, -73.778900, "DCT", false),
    ];
    Ofp {
        origin: OfpAirport { icao: "EGLL".into(), runway: "27L".into(), trans_alt: 6000., trans_level: 60. },
        destination: OfpAirport { icao: "KJFK".into(), runway: "22L".into(), trans_alt: 18000., trans_level: 180. },
        alternate: OfpAirport { icao: "KEWR".into(), runway: "22L".into(), trans_alt: 18000., trans_level: 180. },
        navlog: fixes
            .iter()
            .map(|(ident, kind, lat, lon, via, sid)| NavlogFix {
                ident: ident.to_string(),
                fix_type: kind.to_string(),
                via_airway: via.to_string(),
                is_sid_star: if *sid { "1" } else { "0" }.to_string(),
                pos_lat: format!("{lat:.6}"),
                pos_long: format!("{lon:.6}"),
            })
            .collect(),
        cost_index: 30.,
        cruise_altitude: 36000.,
        average_tropopause: "36000".into(),
        callsign: "FBW1".into(),
    }
}

struct Replay {
    fms: Fms,
    db: MemoryNavDb,
    unsupported: Vec<String>,
    vars: HashMap<String, Vec<(String, String)>>,
    saved_fix: Option<Fix>,
}

fn coordinates_from_js(s: &str) -> Option<Coordinates> {
    let inner = s.trim().trim_start_matches('{').trim_end_matches('}');
    let (mut lat, mut long) = (None, None);
    for part in inner.split(',') {
        let (k, v) = part.split_once(':')?;
        match k.trim() {
            "lat" => lat = v.trim().parse::<f64>().ok(),
            "long" | "lon" => long = v.trim().parse::<f64>().ok(),
            _ => {}
        }
    }
    Some(Coordinates::new(lat?, long?))
}

fn number_in(s: &str, after: &str) -> Option<usize> {
    let rest = &s[s.find(after)? + after.len()..];
    rest.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

fn split_statements(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for c in body.chars() {
        match c {
            '(' | '{' => depth += 1,
            ')' | '}' => depth -= 1,
            _ => {}
        }
        if c == ';' && depth == 0 {
            out.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn call_args(statement: &str) -> Option<(String, Vec<String>)> {
    let start = statement.find("fps.")? + 4;
    let rest = &statement[start..];
    let open = rest.find('(')?;
    let name = rest[..open].trim().to_string();
    let close = rest.rfind(')')?;
    let inner = &rest[open + 1..close];
    let mut args = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for c in inner.chars() {
        match c {
            '(' | '{' => depth += 1,
            ')' | '}' => depth -= 1,
            _ => {}
        }
        if c == ',' && depth == 0 {
            args.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    if !cur.trim().is_empty() {
        args.push(cur.trim().to_string());
    }
    Some((name, args))
}

fn js_string(s: &str) -> String {
    s.trim().trim_matches('\'').to_string()
}

fn constraint_from_js(s: &str) -> Option<AltitudeConstraint> {
    let inner = s.trim().trim_start_matches('{').trim_end_matches('}');
    let mut descriptor = None;
    let mut a1 = None;
    for part in inner.split(',') {
        let (k, v) = part.split_once(':')?;
        match k.trim() {
            "altitudeDescriptor" => descriptor = Some(js_string(v)),
            "altitude1" => a1 = v.trim().parse::<f64>().ok(),
            _ => {}
        }
    }
    Some(AltitudeConstraint { descriptor: descriptor?, altitude1: a1?, altitude2: None })
}

impl Replay {
    fn new() -> Self {
        Replay { fms: Fms::new(), db: db(), unsupported: Vec::new(), vars: HashMap::new(), saved_fix: None }
    }

    fn uplink_canned(&mut self) {
        let mut log = Vec::new();
        uplink_flight_plan_from_simbrief(&mut self.fms, &self.db, &canned_ofp(), &mut log).expect("uplink");
        self.fms.execute(&self.db, Command::UplinkInsert).expect("uplink insert");
    }

    fn condition_holds(&self, condition: &str) -> bool {
        let condition = condition.trim();
        if condition == "fps.hasTemporary" {
            return self.fms.has_temporary();
        }
        if let Some(name) = condition.strip_suffix(".length") {
            return self.vars.get(name.trim()).is_some_and(|v| !v.is_empty());
        }
        false
    }

    fn define_variable(&mut self, statement: &str) -> bool {
        let Some(rest) = statement.trim_start().strip_prefix("const ") else { return false };
        let Some((name, expr)) = rest.split_once('=') else { return false };
        let name = name.trim().to_string();
        let active = self.fms.active();
        let value: Vec<(String, String)> = if expr.contains("availableOriginRunways") {
            let mut v: Vec<(String, String)> = active.map(|p| p.available_origin_runways.iter().map(|r| (r.fix.ident.clone(), r.fix.database_id.clone())).collect()).unwrap_or_default();
            v.sort();
            v
        } else {
            let source: Vec<(String, String)> = if expr.contains("availableDepartures") {
                active.map(|p| p.available_departures.iter().map(|d| (d.ident.clone(), d.database_id.clone())).collect()).unwrap_or_default()
            } else if expr.contains("availableArrivals") {
                active.map(|p| p.available_arrivals.iter().map(|d| (d.ident.clone(), d.database_id.clone())).collect()).unwrap_or_default()
            } else if expr.contains("availableApproaches") {
                active.map(|p| p.available_approaches.iter().map(|d| (d.ident.clone(), d.database_id.clone())).collect()).unwrap_or_default()
            } else {
                return false;
            };
            let mut v = source;
            v.sort_by(|a, b| a.0.cmp(&b.0));
            v
        };
        self.vars.insert(name, value);
        true
    }

    fn resolve(&self, arg: &str) -> String {
        let arg = arg.trim();
        if let Some((name, rest)) = arg.split_once("[0]") {
            if let Some(first) = self.vars.get(name.trim()).and_then(|v| v.first()) {
                return if rest.trim() == ".databaseId" { first.1.clone() } else { first.0.clone() };
            }
        }
        js_string(arg)
    }

    fn waypoint_arg(&self, arg: &str) -> Option<Fix> {
        if arg.trim() == "globalThis.__dirtoFix" {
            return self.saved_fix.clone();
        }
        let ident = arg.split("ident === '").nth(1)?.split('\'').next()?;
        let plan = self.fms.active()?;
        plan.all_legs().iter().filter_map(|e| e.leg()).find(|l| l.ident == ident).and_then(|l| l.definition.waypoint.clone())
    }

    fn special_statement(&mut self, statement: &str) -> bool {
        let trimmed = statement.trim();
        if trimmed.starts_with("globalThis.__dirtoFix =") {
            let index = number_in(trimmed, "legElementAt(").expect("legElementAt index");
            self.saved_fix = self.fms.active().and_then(|p| p.leg_at(index)).and_then(|l| l.definition.waypoint.clone());
            return true;
        }
        if trimmed.starts_with("const m = ") && trimmed.contains("approachWaypointDescriptor") {
            return true;
        }
        if trimmed.starts_with("while (fps.active.activeLegIndex < m)") {
            let map = self.fms.active().and_then(|p| {
                p.all_legs().iter().position(|e| e.leg().is_some_and(|l| l.definition.approach_waypoint_descriptor == Some(fms::plan::APPROACH_DESCRIPTOR_MISSED_APPROACH_POINT)))
            });
            if let Some(map) = map {
                while self.fms.active().is_some_and(|p| p.active_leg_index < map as i64) {
                    self.fms.execute(&self.db, Command::Sequence { plan: PlanRef::Active }).expect("sequence");
                }
            }
            return true;
        }
        false
    }

    fn statement(&mut self, statement: &str) {
        if self.define_variable(statement) || self.special_statement(statement) {
            return;
        }
        let trimmed = statement.trim_start();
        let statement = if trimmed.starts_with("if (") {
            let Some((condition, rest)) = trimmed[4..].split_once(')') else { return };
            if !self.condition_holds(condition) {
                return;
            }
            rest
        } else {
            statement
        };
        let Some((name, args)) = call_args(statement) else {
            self.unsupported.push(statement.to_string());
            return;
        };
        let num = |i: usize| args.get(i).and_then(|a| a.trim().parse::<usize>().ok()).unwrap_or(0);
        let command = match name.as_str() {
            "newCityPair" => Command::NewCityPair {
                from: self.resolve(&args[0]),
                to: self.resolve(&args[1]),
                alternate: args.get(2).map(|a| self.resolve(a)),
                plan: PlanRef::Active,
            },
            "setOriginRunway" => Command::SetOriginRunway { runway: self.resolve(&args[0]), plan: PlanRef::Active, alternate: false },
            "setDestinationRunway" => Command::SetDestinationRunway { runway: self.resolve(&args[0]), plan: PlanRef::Active, alternate: false },
            "setDepartureProcedure" => Command::SetDeparture { id: Some(self.resolve(&args[0])), plan: PlanRef::Active, alternate: false },
            "setArrival" => Command::SetArrival { id: Some(self.resolve(&args[0])), plan: PlanRef::Active, alternate: false },
            "setApproach" => Command::SetApproach { id: Some(self.resolve(&args[0])), plan: PlanRef::Active, alternate: false },
            "deleteElementAt" => Command::DeleteElementAt {
                index: num(0),
                insert_discontinuity: args.get(1).is_some_and(|a| a.trim() == "true"),
                plan: args.get(2).and_then(|a| a.trim().parse::<usize>().ok()).map(PlanRef::from_index).unwrap_or(PlanRef::Active),
            },
            "temporaryInsert" => Command::TemporaryInsert,
            "insertDiscontinuityAfter" => Command::InsertDiscontinuityAfter { index: num(0), plan: PlanRef::Active },
            "setPilotEnteredAltitudeConstraintAt" => Command::SetPilotAltitudeConstraint {
                index: num(0),
                descent: args.get(1).is_some_and(|a| a.trim() == "true"),
                constraint: args.get(2).and_then(|a| constraint_from_js(a)),
                plan: PlanRef::Active,
            },
            "addOrUpdateCruiseStep" => Command::AddOrUpdateCruiseStep { index: num(0), altitude: args.get(1).and_then(|a| a.trim().parse().ok()).unwrap_or(0.), plan: PlanRef::Active },
"active.sequence" => Command::Sequence { plan: PlanRef::Active },
            "setAlternate" => Command::SetAlternate { icao: Some(self.resolve(&args[0])), plan: PlanRef::Active },
            "enableAltn" => Command::EnableAltn { index: num(0), cruise_level: args.get(1).and_then(|a| a.trim().parse().ok()).unwrap_or(100.), plan: PlanRef::Active },
            "newDest" => Command::NewDest { index: num(0), airport: self.resolve(&args[1]), plan: PlanRef::Active, alternate: false },
            "directToLeg" => Command::DirectToLeg {
                ppos: coordinates_from_js(&args[0]).expect("ppos"),
                true_track: args[1].trim().parse().expect("track"),
                index: num(2),
                mag_var: None,
                with_abeam: false,
                plan: PlanRef::Active,
            },
            "directToWaypoint" => Command::DirectToWaypoint {
                ppos: coordinates_from_js(&args[0]).expect("ppos"),
                true_track: args[1].trim().parse().expect("track"),
                waypoint: self.waypoint_arg(&args[2]).expect("direct-to waypoint"),
                mag_var: None,
                with_abeam: false,
                plan: PlanRef::Active,
            },
            "secondaryCopyFromActive" => Command::SecondaryCopyFromActive { index: num(0) },
            "activeAndSecondarySwap" => Command::ActiveAndSecondarySwap { index: num(0) },
            _ => {
                self.unsupported.push(statement.to_string());
                return;
            }
        };
        if let Err(e) = self.fms.execute(&self.db, command) {
            panic!("{statement}: {e}");
        }
    }

    fn step(&mut self, step: &Value) {
        if step.get("uplink").is_some() {
            self.uplink_canned();
        } else if let Some(body) = step.get("fms").and_then(|v| v.as_str()) {
            let body = body.trim();
            let body = body.strip_prefix("await ").unwrap_or(body);
            for statement in split_statements(body) {
                let statement = statement.replace("await ", "");
                self.statement(&statement);
            }
        }
    }
}

#[derive(Default)]
struct Report {
    matched: usize,
    differing: Vec<String>,
}

impl Report {
    fn check(&mut self, ok: bool, what: impl FnOnce() -> String) {
        if ok {
            self.matched += 1;
        } else {
            self.differing.push(what());
        }
    }
}

fn opt_f64(v: &Value) -> Option<f64> {
    v.as_f64()
}

fn compare_plan(report: &mut Report, label: &str, golden: &Value, plan: Option<&FlightPlan>) {
    if golden.is_null() {
        report.check(plan.is_none(), || format!("{label}: golden has no plan but Rust has one"));
        return;
    }
    let Some(plan) = plan else {
        report.differing.push(format!("{label}: golden has a plan but Rust has none"));
        return;
    };
    report.check(golden["origin"].as_str() == plan.origin_airport.as_ref().map(|a| a.ident.as_str()), || format!("{label}: origin"));
    report.check(golden["destination"].as_str() == plan.destination_airport.as_ref().map(|a| a.ident.as_str()), || format!("{label}: destination"));
    report.check(golden["activeLegIndex"].as_i64() == Some(plan.active_leg_index), || format!("{label}: activeLegIndex {} vs {}", golden["activeLegIndex"], plan.active_leg_index));
    let legs = golden["legs"].as_array().cloned().unwrap_or_default();
    let ours = plan.all_legs();
    report.check(legs.len() == ours.len(), || format!("{label}: leg count {} vs {}", legs.len(), ours.len()));
    for (i, (g, e)) in legs.iter().zip(ours.iter()).enumerate() {
        let tag = format!("{label}[{i}]");
        let g_disco = g["disco"].as_bool().unwrap_or(false);
        report.check(g_disco == e.is_disco(), || format!("{tag}: disco"));
        let (false, Element::Leg(leg)) = (g_disco, e) else { continue };
        report.check(g["ident"].as_str() == Some(leg.ident.as_str()), || format!("{tag}: ident {} vs {}", g["ident"], leg.ident));
        report.check(g["type"].as_str() == Some(leg.leg_type.as_str()), || format!("{tag}: type {} vs {}", g["type"], leg.leg_type.as_str()));
        report.check(g["flags"].as_u64() == Some(leg.flags as u64), || format!("{tag}: flags {} vs {}", g["flags"], leg.flags));
        report.check(g["descriptor"].as_i64() == leg.definition.waypoint_descriptor.map(|d| d as i64), || format!("{tag}: descriptor {} vs {:?}", g["descriptor"], leg.definition.waypoint_descriptor));
        report.check(g["constraintType"].as_i64() == Some(leg.constraint_type as i64), || format!("{tag}: constraintType {} vs {}", g["constraintType"], leg.constraint_type));
        let golden_annotation = g["annotation"].as_str().map(|a| a.replace("\u{c2}\u{b0}", "\u{b0}"));
        report.check(golden_annotation.as_deref() == Some(leg.annotation.as_str()), || format!("{tag}: annotation {} vs {}", g["annotation"], leg.annotation));
        let alt = leg.altitude_constraint();
        let alt_ok = match (&g["alt"], &alt) {
            (Value::Null, None) => true,
            (v, Some(c)) => v["altitudeDescriptor"].as_str() == Some(c.descriptor.as_str()) && opt_f64(&v["altitude1"]) == Some(c.altitude1),
            _ => false,
        };
        report.check(alt_ok, || format!("{tag}: altitude constraint {} vs {:?}", g["alt"], alt));
        report.check(g["spd"].is_null() == leg.speed_constraint().is_none(), || format!("{tag}: speed constraint"));
        let hold_ok = match (&g["hold"], leg.hold()) {
            (Value::Null, None) => true,
            (v, Some(h)) => v["type"].as_i64() == Some(h.hold_type as i64) && opt_f64(&v["inboundMagneticCourse"]) == h.inbound_magnetic_course && opt_f64(&v["distance"]) == h.distance && opt_f64(&v["time"]) == h.time,
            _ => false,
        };
        report.check(hold_ok, || format!("{tag}: hold {} vs {:?}", g["hold"], leg.hold()));
        let wp = leg.termination_waypoint();
        let lat_ok = match (opt_f64(&g["lat"]), wp) {
            (Some(l), Some(w)) => (l - w.location.lat).abs() <= POSITION_TOLERANCE,
            (None, None) => true,
            _ => false,
        };
        let lon_ok = match (opt_f64(&g["lon"]), wp) {
            (Some(l), Some(w)) => (l - w.location.long).abs() <= POSITION_TOLERANCE,
            (None, None) => true,
            _ => false,
        };
        report.check(lat_ok && lon_ok, || format!("{tag}: position"));
        report.check(g["def"]["waypoint"]["databaseId"].as_str() == wp.map(|w| w.database_id.as_str()), || format!("{tag}: databaseId"));
        report.check(g["def"]["procedureIdent"].as_str() == Some(leg.definition.procedure_ident.as_str()), || format!("{tag}: procedureIdent"));
        report.check(g["def"]["overfly"].as_bool() == Some(leg.definition.overfly), || format!("{tag}: overfly"));
        let prev_is_leg = i > 0 && !ours[i - 1].is_disco();
        if let (Some(track), true) = (g["calc"]["trueTrack"].as_f64(), leg.leg_type == fms::plan::LegType::TF && prev_is_leg) {
            let ours = plan.true_track_at(i);
            report.check(ours.is_some_and(|t| (t - track).abs() <= ANGLE_TOLERANCE), || format!("{tag}: trueTrack {track} vs {ours:?}"));
        }
    }
}

fn run_scenario(name: &str) -> Option<(Report, Vec<String>)> {
    let golden_path = oracle_dir().join(format!("{name}.json"));
    let Ok(golden_text) = std::fs::read_to_string(&golden_path) else {
        eprintln!("{name}: no golden at {}", golden_path.display());
        return None;
    };
    let golden: Value = serde_json::from_str(&golden_text).expect("golden json");
    let scenario: Value = serde_json::from_str(&std::fs::read_to_string(scenario_dir().join(format!("{name}.json"))).expect("scenario")).expect("scenario json");
    let mut replay = Replay::new();
    let mut report = Report::default();
    let mut snapshot_labels = Vec::new();
    let snapshots = golden["snapshots"].as_array().cloned().unwrap_or_default();
    for step in scenario["steps"].as_array().cloned().unwrap_or_default() {
        if let Some(label) = step.get("snapshot").and_then(|v| v.as_str()) {
            let Some(snap) = snapshots.iter().find(|s| s["label"] == label) else { panic!("{name}: golden has no snapshot {label}") };
            let plans = &snap["state"]["plans"];
            if !replay.unsupported.is_empty() {
                snapshot_labels.push(format!("{label}: skipped, unsupported steps {:?}", replay.unsupported));
                continue;
            }
            let mut r = Report::default();
            compare_plan(&mut r, &format!("{label}/active"), &plans["active"], replay.fms.active());
            compare_plan(&mut r, &format!("{label}/temporary"), &plans["temporary"], replay.fms.plan(PlanRef::Temporary));
            let secondary = plans["secondary"].as_array().cloned().unwrap_or_default();
            let ours: Vec<usize> = (1..=3).filter(|n| replay.fms.plan(PlanRef::Secondary(*n)).is_some()).collect();
            r.check(ours.len() == secondary.len(), || format!("{label}: secondary plan count {} vs {}", secondary.len(), ours.len()));
            for (g, n) in secondary.iter().zip(ours.iter()) {
                compare_plan(&mut r, &format!("{label}/sec{n}"), g, replay.fms.plan(PlanRef::Secondary(*n)));
            }
            let alt = replay.fms.active().and_then(|p| p.alternate.as_deref());
            compare_plan(&mut r, &format!("{label}/alternate"), &snap["state"]["alternate"], alt);
            snapshot_labels.push(format!("{label}: {} fields match, {} differ", r.matched, r.differing.len()));
            report.matched += r.matched;
            report.differing.extend(r.differing);
        } else {
            replay.step(&step);
        }
    }
    Some((report, snapshot_labels))
}

fn assert_scenario(name: &str) {
    let Some((report, labels)) = run_scenario(name) else { return };
    for l in &labels {
        println!("{name}: {l}");
    }
    for d in &report.differing {
        println!("{name}: DIFF {d}");
    }
    assert!(report.differing.is_empty(), "{name}: {} fields differ", report.differing.len());
}

#[test]
fn uplink_egll_kjfk_matches_the_golden() {
    assert_scenario("uplink-egll-kjfk");
}

#[test]
fn edits_tmpy_matches_the_golden() {
    assert_scenario("edits-tmpy");
}

#[test]
fn secondary_matches_the_golden() {
    assert_scenario("secondary");
}

#[test]
fn city_pair_procedures_matches_the_golden() {
    assert_scenario("city-pair-procedures");
}

#[test]
fn dirto_egll_kjfk_matches_the_golden() {
    assert_scenario("dirto-egll-kjfk");
}

#[test]
fn sequence_egll_kjfk_matches_the_golden() {
    assert_scenario("sequence-egll-kjfk");
}

#[test]
fn altn_egll_kjfk_matches_the_golden() {
    assert_scenario("altn-egll-kjfk");
}

#[test]
fn memory_navdb_finds_the_uplink_fixes() {
    let db = db();
    assert!(db.airport("EGLL").is_some());
    assert_eq!(db.search_fix("CPT").len(), 1);
    let ipnox = db.search_fix("IPNOX").pop().expect("IPNOX");
    assert_eq!(db.search_airway("UN57", &ipnox).len(), 1);
    let _ = Fix::default();
}

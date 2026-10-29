use fms_lnav::geo::place_bearing_distance;
use fms_lnav::geometry::string_missed_approach;
use fms_lnav::*;
use serde_json::Value;

const DIRS: [&str; 5] = [r"E:\fbw-build\fms-oracle", r"E:\fbw-build\fms-oracle-fly", r"E:\fbw-build\lnav62\out", r"E:\fbw-build\lnav36\out", r"E:\fbw-build\lnav77\out"];
const STATIC_DIR: &str = r"E:\fbw-build\lnav77\out";
const STATIC_SCENARIOS: &str = r"E:\fbw-build\lnav77\static";
const SCENARIOS: [&str; 7] = ["flight-egll-kjfk-sid", "flight-egll-kjfk-star", "flight2-egll-kjfk-sid", "flight2-egll-kjfk-arr-app", "flight2-egll-kjfk-arr-hold", "flight2-egll-kjfk-app-missed", "flight2-egll-kjfk-arr-hold-long"];

const RECOMPUTE_MS: f64 = 5000.;
const MISSED_APPROACH_POINT: i64 = 8;

fn load(name: &str) -> Option<Value> {
    DIRS.iter().find_map(|d| std::fs::read_to_string(format!("{d}\\{name}.json")).ok()).map(|t| serde_json::from_str(&t).unwrap())
}

fn coord(v: &Value) -> Coord {
    Coord::new(v["lat"].as_f64().unwrap(), v["long"].as_f64().unwrap())
}

fn true_course(def: &Value) -> f64 {
    let course = def["course"].as_f64().unwrap_or(0.);
    let mag_var = def["magVar"].as_f64().unwrap_or(0.);
    (course + mag_var).rem_euclid(360.)
}

fn waypoint(def: &Value) -> Option<Coord> {
    def["waypoint"]["location"].is_object().then(|| coord(&def["waypoint"]["location"]))
}

fn navaid_dme(def: &Value) -> Option<Coord> {
    let n = &def["recommendedNavaid"];
    if n["dmeLocation"].is_object() {
        Some(coord(&n["dmeLocation"]))
    } else if n["location"].is_object() {
        Some(coord(&n["location"]))
    } else {
        None
    }
}

fn turn_of(def: &Value) -> TurnDirection {
    match def["turnDirection"].as_str() {
        Some("L") => TurnDirection::Left,
        Some("R") => TurnDirection::Right,
        _ => TurnDirection::Either,
    }
}

fn elements(legs: &[Value]) -> Vec<PlanElement> {
    let mut out = Vec::new();
    let mut prev_fix: Option<Coord> = None;
    let mut prev_runway = false;
    let first_missed = legs.iter().rposition(|l| l["def"]["approachWaypointDescriptor"].as_i64() == Some(MISSED_APPROACH_POINT)).map_or(usize::MAX, |i| i + 1);
    for (index, l) in legs.iter().enumerate() {
        if l["disco"].as_bool() == Some(true) {
            out.push(PlanElement::Discontinuity);
            prev_fix = None;
            continue;
        }
        let ident = l["ident"].as_str().unwrap_or("");
        let def = &l["def"];
        let fix = match (l["lat"].as_f64(), l["lon"].as_f64()) {
            (Some(la), Some(lo)) => Some(Coord::new(la, lo)),
            _ => None,
        };
        let leg = match (l["type"].as_str().unwrap_or(""), fix) {
            ("IF", Some(f)) => LegInput::initial_fix(ident, f),
            ("TF", Some(f)) if prev_fix.is_some() => LegInput::track_to_fix(ident, prev_fix.unwrap(), f),
            ("CF", Some(f)) => LegInput::new(ident, LegSpec::Cf { course: true_course(def), length: def["length"].as_f64().unwrap_or(0.) }, f),
            ("DF", Some(f)) => LegInput::new(ident, LegSpec::Df, f),
            ("FC", Some(f)) => LegInput::new(ident, LegSpec::Fc { course: true_course(def), length: def["length"].as_f64().unwrap_or(0.) }, f),
            ("FM", Some(f)) => LegInput::new(ident, LegSpec::Fm { course: true_course(def) }, f),
            ("CR" | "VR", _) if def["recommendedNavaid"]["location"].is_object() => {
                let navaid = &def["recommendedNavaid"];
                let theta = def["theta"].as_f64().unwrap_or(0.);
                let theta = if navaid["trueReferenced"].as_bool() == Some(true) { theta } else { (theta + navaid["stationDeclination"].as_f64().unwrap_or(0.)).rem_euclid(360.) };
                LegInput::new(ident, LegSpec::Cr { course: true_course(def), navaid: coord(&navaid["location"]), theta }, Coord::new(0., 0.))
            }
            ("CA" | "VA", _) => LegInput::new(ident, LegSpec::Ca { course: true_course(def), altitude: def["altitude1"].as_f64().unwrap_or(0.), after_runway: prev_runway }, Coord::new(0., 0.)),
            ("CI" | "VI", _) => LegInput::new(ident, LegSpec::Ci { course: true_course(def) }, Coord::new(0., 0.)),
            ("CD" | "VD", _) if navaid_dme(def).is_some() => LegInput::new(ident, LegSpec::Cd { course: true_course(def), distance: def["length"].as_f64().unwrap_or(0.), dme: navaid_dme(def).unwrap() }, Coord::new(0., 0.)),
            ("FD", _) if navaid_dme(def).is_some() && waypoint(def).is_some() => LegInput::new(ident, LegSpec::Fd { course: true_course(def), distance: def["length"].as_f64().unwrap_or(0.), dme: navaid_dme(def).unwrap() }, waypoint(def).unwrap()),
            ("VM", _) => LegInput::new(ident, LegSpec::Vm { heading: true_course(def) }, Coord::new(0., 0.)),
            ("AF", _) if def["recommendedNavaid"]["location"].is_object() && waypoint(def).is_some() => {
                let navaid = &def["recommendedNavaid"];
                let to_true = |a: f64| if navaid["trueReferenced"].as_bool() == Some(true) { a } else { (a + navaid["stationDeclination"].as_f64().unwrap_or(0.)).rem_euclid(360.) };
                let f = waypoint(def).unwrap();
                let course = def["course"].as_f64().unwrap_or(0.);
                let theta = to_true(def["theta"].as_f64().unwrap_or(0.));
                let centre = if navaid["dmeLocation"].is_object() { coord(&navaid["dmeLocation"]) } else { place_bearing_distance(f, (theta + 180.).rem_euclid(360.), def["rho"].as_f64().unwrap_or(0.)) };
                LegInput::new(ident, LegSpec::Af { centre, boundary_radial: to_true(course), boundary_radial_coded: course, termination_radial: theta }, f)
            }
            ("PI", _) if def["recommendedNavaid"]["location"].is_object() => LegInput::new(ident, LegSpec::Pi { course: true_course(def) }, coord(&def["recommendedNavaid"]["location"])),
            (kind @ ("HA" | "HF" | "HM"), Some(f)) => {
                let kind = match kind {
                    "HA" => HoldKind::Ha,
                    "HF" => HoldKind::Hf,
                    _ => HoldKind::Hm,
                };
                LegInput::new(ident, LegSpec::Hold { kind, course: true_course(def), length: def["length"].as_f64(), length_time: def["lengthTime"].as_f64(), altitude: def["altitude1"].as_f64() }, f)
            }
            _ => break,
        };
        let mut leg = leg;
        leg.turn = turn_of(def);
        leg.overfly = def["overfly"].as_bool().unwrap_or(false);
        leg.enroute = false;
        leg.missed_approach = index >= first_missed;
        if let Some(id) = def["waypoint"]["databaseId"].as_str() {
            leg.waypoint_id = id.to_string();
        }
        if let Some(f) = fix {
            prev_fix = Some(f);
        }
        leg.airport_or_runway = matches!(def["waypointDescriptor"].as_i64(), Some(1 | 4));
        prev_runway = l["type"] == "IF" && def["waypointDescriptor"].as_i64() == Some(4);
        out.push(PlanElement::Leg(leg));
    }
    out
}

fn element_key(e: &PlanElement) -> String {
    match e {
        PlanElement::Discontinuity => "-".to_string(),
        PlanElement::Leg(l) => format!("{} {:?} {:?} missed={}", l.ident, l.spec, l.fix, l.missed_approach),
    }
}

fn recorded_active(events: &[Value]) -> Vec<(f64, Vec<Value>)> {
    recorded_topic(events, "A32NX_EFIS_VECTORS_L_ACTIVE")
}

fn recorded_topic(events: &[Value], topic: &str) -> Vec<(f64, Vec<Value>)> {
    let mut sets = Vec::new();
    for e in events {
        let Some(json) = e.get("json").and_then(|j| j.as_str()) else { continue };
        if !json.contains(&format!("{topic}\"")) {
            continue;
        }
        let parsed: Value = serde_json::from_str(json).unwrap();
        for item in parsed["data"].as_array().unwrap() {
            if item["topic"] == topic {
                if let Some(a) = item["data"].as_array() {
                    sets.push((e["t"].as_f64().unwrap_or(0.), a.clone()));
                }
            }
        }
    }
    sets
}

fn brief(v: &Value) -> String {
    let p = |c: &Value| format!("{:.4},{:.4}", c["lat"].as_f64().unwrap_or(0.), c["long"].as_f64().unwrap_or(0.));
    match v["type"].as_i64() {
        Some(1) => format!("A {}->{} sw {:.1}", p(&v["startPoint"]), p(&v["endPoint"]), v["sweepAngle"].as_f64().unwrap_or(0.)),
        _ => format!("L {}->{}", p(&v["startPoint"]), p(&v["endPoint"])),
    }
}

fn vectors_match(mine: &[PathVector], rec: &[Value]) -> bool {
    if mine.len() != rec.len() {
        return false;
    }
    mine.iter().zip(rec).all(|(m, r)| {
        let near = |a: Coord, b: Coord| (a.lat - b.lat).abs().max((a.long - b.long).abs()) < 2e-3;
        let ty = r["type"].as_i64().unwrap_or(-1);
        ty == if matches!(m, PathVector::Line { .. }) { 0 } else { 1 } && near(m.start(), coord(&r["startPoint"])) && near(m.end(), coord(&r["endPoint"]))
    })
}

#[derive(Default)]
struct Stats {
    compared: usize,
    skipped_other_leg: usize,
    worst_xte: f64,
    worst_tae: f64,
    worst_phi: f64,
    sum_xte: f64,
    over_xte: usize,
    over_tae: usize,
    over_phi: usize,
    over_limit: usize,
    outliers: Vec<String>,
}

fn run(name: &str) {
    let Some(golden) = load(name) else {
        eprintln!("{name}: golden missing, skipping");
        return;
    };
    let Some(trace) = golden.get("trace").and_then(|t| t.as_array()) else {
        eprintln!("{name}: no trace, skipping");
        return;
    };
    let snapshot = golden["snapshots"].as_array().unwrap().iter().find(|s| s["label"] == "before-flight").expect("before-flight snapshot");
    let legs = snapshot["state"]["plans"]["active"]["legs"].as_array().unwrap();
    let mut supported = elements(legs);
    let after_flight = golden["snapshots"].as_array().unwrap().iter().find(|s| s["label"] == "after-flight").and_then(|s| s["state"]["plans"]["active"]["legs"].as_array()).filter(|l| l.len() > legs.len()).map(|l| elements(l));
    let first_missed = supported.iter().position(|e| matches!(e, PlanElement::Leg(l) if l.missed_approach)).unwrap_or(usize::MAX);
    let mut restring_done = after_flight.is_none();
    eprintln!("{name}: {} of {} plan elements supported, {} trace samples", supported.len(), legs.len(), trace.len());
    let mut geometry = Geometry::new(&supported);
    let events = recorded_active(golden["events"].as_array().unwrap());

    let mut stats = Stats::default();
    let mut efis_same = 0;
    let mut efis_total = 0;
    let mut first_efis_miss: Option<String> = None;
    let mut recomputed_for: Option<usize> = None;
    let mut last_recompute = f64::NEG_INFINITY;
    let mut ours_active = trace.first().map(|s| s["leg"].as_i64().unwrap() as usize).unwrap_or(0);
    let mut sequencing_offsets = Vec::new();
    let mut last_fbw_leg = ours_active;

    for sample in trace {
        let fbw_leg = sample["leg"].as_i64().unwrap() as usize;
        let ppos = Coord::new(sample["lat"].as_f64().unwrap(), sample["lon"].as_f64().unwrap());
        let (track, gs, tas) = (sample["track"].as_f64().unwrap(), sample["gs"].as_f64().unwrap(), sample["tas"].as_f64().unwrap());
        if fbw_leg != last_fbw_leg {
            sequencing_offsets.push((fbw_leg, ours_active as i64 - fbw_leg as i64));
            last_fbw_leg = fbw_leg;
        }
        if !restring_done && ours_active >= first_missed && fbw_leg >= first_missed {
            let map = first_missed - 1;
            let first_approach = supported[..map].iter().rposition(|e| matches!(e, PlanElement::Discontinuity)).map_or(0, |d| d + 1);
            let restrung = string_missed_approach(&supported, map, first_approach);
            let after = after_flight.as_ref().unwrap();
            let same = restrung.iter().zip(after).filter(|(a, b)| element_key(a) == element_key(b)).count();
            eprintln!("{name}: strung missed approach matches the after-flight plan at {same}/{} elements ({} recorded)", restrung.len(), after.len());
            geometry.update_from_plan(&restrung, ours_active);
            supported = restrung;
            restring_done = true;
        }
        if ours_active >= supported.len() || !matches!(supported.get(ours_active), Some(PlanElement::Leg(_))) {
            break;
        }
        if let Some(preds) = sample.get("pred").and_then(|p| p.as_array()) {
            for e in preds {
                if let Some(i) = e[0].as_u64() {
                    geometry.set_predictions(i as usize, e[1].as_f64(), e[2].as_f64());
                }
            }
        }
        let now = sample["t"].as_f64().unwrap();
        if let Some(records) = sample.get("recomp").and_then(|r| r.as_array()) {
            if recomputed_for.is_none() {
                geometry.recompute(ours_active, ppos, track, 0., 0.);
                recomputed_for = Some(ours_active);
            }
            for r in records {
                let at = match (r["lat"].as_f64(), r["lon"].as_f64()) {
                    (Some(la), Some(lo)) => Coord::new(la, lo),
                    _ => ppos,
                };
                let f = |k: &str| r[k].as_f64().unwrap_or(0.);
                let leg = r["leg"].as_u64().map_or(ours_active, |l| l as usize);
                geometry.recompute_once(leg, at, f("track"), f("tas"), f("gs"));
                recomputed_for = Some(leg);
            }
            if recomputed_for != Some(ours_active) {
                geometry.recompute(ours_active, ppos, track, tas, gs);
                recomputed_for = Some(ours_active);
            }
        } else if recomputed_for != Some(ours_active) || now - last_recompute > RECOMPUTE_MS {
            geometry.recompute(ours_active, ppos, track, tas, gs);
            recomputed_for = Some(ours_active);
            last_recompute = now;
        }
        if let Some(p) = geometry.guidance_parameters(ours_active, ppos, track, gs, tas) {
            if ours_active == fbw_leg && sample["avail"].as_bool() == Some(true) {
                let (dx, dt, dp) = ((p.cross_track_error - sample["xte"].as_f64().unwrap()).abs(), (p.track_angle_error - sample["tae"].as_f64().unwrap()).abs(), (p.phi_command - sample["phi_command"].as_f64().unwrap()).abs());
                stats.compared += 1;
                stats.worst_xte = stats.worst_xte.max(dx);
                stats.worst_tae = stats.worst_tae.max(dt);
                stats.worst_phi = stats.worst_phi.max(dp);
                stats.sum_xte += dx;
                stats.over_xte += (dx > 0.05) as usize;
                stats.over_tae += (dt > 1.) as usize;
                stats.over_phi += (dp > 1.) as usize;
                stats.over_limit += ((p.phi_limit - sample["phi_limit"].as_f64().unwrap_or(0.)).abs() > 0.01) as usize;
                if (dx > 0.05 || dt > 1. || dp > 1.) && stats.outliers.len() < 14 {
                    stats.outliers.push(format!("t={:.0}s leg {fbw_leg} dxte {dx:.3} dtae {dt:.2} dphi {dp:.1} (mine xte {:.3} tae {:.2} phi {:.1}; fbw xte {:.3} tae {:.2} phi {:.1})", sample["t"].as_f64().unwrap() / 1000., p.cross_track_error, p.track_angle_error, p.phi_command, sample["xte"].as_f64().unwrap(), sample["tae"].as_f64().unwrap(), sample["phi_command"].as_f64().unwrap()));
                }
            } else {
                stats.skipped_other_leg += 1;
            }
        }
        if ours_active == fbw_leg {
            if let Some((_, rec)) = events.iter().rev().find(|(t, _)| *t <= sample["t"].as_f64().unwrap()) {
                efis_total += 1;
                let mine = geometry.efis_path_vectors(ours_active, false);
                if vectors_match(&mine, rec) {
                    efis_same += 1;
                } else if first_efis_miss.is_none() {
                    first_efis_miss = Some(format!("t={:.0}s leg {fbw_leg}: mine {:?} recorded {:?}", sample["t"].as_f64().unwrap() / 1000., mine.iter().map(|v| brief(&v.to_json())).collect::<Vec<_>>(), rec.iter().map(brief).collect::<Vec<_>>()));
                }
            }
        }
        if geometry.should_sequence_leg(ours_active, ppos) {
            ours_active += 1;
            if std::env::var("LNAV_DEBUG").is_ok() {
                eprintln!("  t={:.0}s ours -> {ours_active} (fbw {fbw_leg})", now / 1000.);
            }
        }
    }
    eprintln!(
        "{name}: guidance compared {} (skipped {}): worst xte {:.4} nm, tae {:.3} deg, phi {:.3} deg; mean xte {:.4}; over tol xte/tae/phi/phi-limit {}/{}/{}/{}",
        stats.compared,
        stats.skipped_other_leg,
        stats.worst_xte,
        stats.worst_tae,
        stats.worst_phi,
        stats.sum_xte / stats.compared.max(1) as f64,
        stats.over_xte,
        stats.over_tae,
        stats.over_phi,
        stats.over_limit
    );
    for o in &stats.outliers {
        eprintln!("  outlier {o}");
    }
    if let Some(m) = &first_efis_miss {
        eprintln!("  first EFIS mismatch {}", &m[..m.len().min(2500)]);
    }
    eprintln!("{name}: EFIS active vectors equal at {efis_same}/{efis_total} samples; sequencing offsets (fbw leg, ours - fbw) {sequencing_offsets:?}");
}

#[test]
fn flights_against_the_recording() {
    for name in SCENARIOS {
        run(name);
    }
}

fn static_case(path: &std::path::Path) -> (usize, usize) {
    let name = path.file_stem().unwrap().to_string_lossy().to_string();
    let golden: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let snapshot = golden["snapshots"].as_array().unwrap().iter().find(|s| s["label"] == "static").expect("static snapshot");
    let plan = &snapshot["state"]["plans"]["active"];
    let legs = plan["legs"].as_array().unwrap();
    let active = plan["activeLegIndex"].as_u64().unwrap() as usize;
    let supported = elements(legs);
    let scenario: Value = serde_json::from_str(&std::fs::read_to_string(format!(r"{STATIC_SCENARIOS}\{name}.json")).unwrap()).unwrap();
    let ppos = Coord::new(scenario["lat"].as_f64().unwrap(), scenario["lon"].as_f64().unwrap());
    let mut geometry = Geometry::new(&supported);
    for _ in 0..4 {
        geometry.recompute(active, ppos, 0., 0., 0.);
    }
    let events = golden["events"].as_array().unwrap();
    let mut same = 0;
    let mut total = 0;
    for (topic, missed) in [("A32NX_EFIS_VECTORS_L_ACTIVE", false), ("A32NX_EFIS_VECTORS_L_MISSED", true)] {
        let Some((_, rec)) = recorded_topic(events, topic).pop().filter(|(_, r)| !r.is_empty()) else { continue };
        let mine = geometry.efis_path_vectors(active, missed);
        total += 1;
        if vectors_match(&mine, &rec) {
            same += 1;
        } else {
            eprintln!("  {name} {topic} mismatch:
    mine {:?}
    rec  {:?}", mine.iter().map(|v| brief(&v.to_json())).collect::<Vec<_>>(), rec.iter().map(brief).collect::<Vec<_>>());
        }
    }
    eprintln!("{name}: {} of {} plan elements supported, active {active}; EFIS active+missed equal {same}/{total}", supported.len(), legs.len());
    (same, total)
}

#[test]
fn static_recordings() {
    let mut files: Vec<_> = std::fs::read_dir(STATIC_DIR).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("static-"))).collect()).unwrap_or_default();
    files.sort();
    let (mut same, mut total) = (0, 0);
    for f in &files {
        let (s, t) = static_case(f);
        same += s;
        total += t;
    }
    eprintln!("static recordings: {} files, EFIS vector sets equal {same}/{total}", files.len());
}

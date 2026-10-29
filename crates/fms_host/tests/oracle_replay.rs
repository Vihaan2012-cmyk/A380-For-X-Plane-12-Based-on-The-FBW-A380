use std::collections::BTreeMap;
use std::sync::Arc;

use fms::navdb::MemoryNavDb;
use fms::service::{Command, PlanRef};
use fms::uplink::{NavlogFix, Ofp, OfpAirport};
use fms_host::{Envelope, FmsCore, FmsHost, HostCommand, Inputs, Outputs};
use serde_json::Value;

const ORACLE: &str = "E:/fbw-build/fms-oracle/uplink-egll-kjfk.json";

fn db() -> MemoryNavDb {
    MemoryNavDb::from_fixture_json(include_str!("../../fms/tests/fixtures/navdb.json")).expect("fixture")
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
        destination: OfpAirport { icao: "KJFK".into(), runway: "22L".into(), trans_alt: 18000., trans_level: 18000. },
        alternate: OfpAirport { icao: "KEWR".into(), runway: "22L".into(), trans_alt: 18000., trans_level: 18000. },
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

struct Recording {
    vars: BTreeMap<String, f64>,
    gd: BTreeMap<String, Value>,
    inputs: BTreeMap<String, f64>,
    bus: Vec<(String, Value)>,
    raw: Value,
}

fn load() -> Option<Recording> {
    load_from(ORACLE)
}

fn load_from(path: &str) -> Option<Recording> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let mut rec = Recording { vars: BTreeMap::new(), gd: BTreeMap::new(), inputs: BTreeMap::new(), bus: Vec::new(), raw: Value::Null };
    for e in v["events"].as_array()? {
        if let (Some(name), Some(value)) = (e["var"].as_str(), e["value"].as_f64()) {
            if e["t"].as_f64() == Some(0.) {
                rec.inputs.insert(name.to_string(), value);
            } else if let Some(n) = name.strip_prefix("L:") {
                rec.vars.insert(n.to_string(), value);
            }
        } else if e["key"] == "xp-ui.bus" {
            let batch: Value = serde_json::from_str(e["json"].as_str()?).ok()?;
            for item in batch["events"].as_array()? {
                rec.bus.push((item["topic"].as_str()?.to_string(), item["data"].clone()));
            }
        } else if e["key"] == "EB_EVENTS" {
            let batch: Value = serde_json::from_str(e["json"].as_str()?).ok()?;
            for item in batch["data"].as_array()? {
                if let Some(t) = item["topic"].as_str().filter(|t| t.starts_with("A32NX_EFIS_")) {
                    rec.gd.insert(t.to_string(), item["data"].clone());
                }
            }
        }
    }
    rec.raw = v;
    Some(rec)
}

fn inputs_from(rec: &Recording) -> Inputs {
    Inputs::from_vars(|name| {
        let alt = name.trim_start_matches("A:").trim_start_matches("E:");
        rec.inputs.get(name).or_else(|| rec.inputs.get(alt)).copied().unwrap_or(0.)
    })
}

fn last_state(all: &[Outputs]) -> (BTreeMap<String, f64>, BTreeMap<String, Value>) {
    let mut vars = BTreeMap::new();
    let mut gd = BTreeMap::new();
    for o in all {
        for (k, v) in &o.vars {
            vars.insert(k.clone(), *v);
        }
        for (k, v) in &o.gd {
            gd.insert(k.clone(), v.clone());
        }
    }
    (vars, gd)
}

fn same_vectors(mine: &Value, rec: &Value) -> Result<(), String> {
    let (Some(a), Some(b)) = (mine.as_array(), rec.as_array()) else {
        return if mine == rec { Ok(()) } else { Err(format!("{mine} vs {rec}")) };
    };
    if a.len() != b.len() {
        return Err(format!("count {} vs {}", a.len(), b.len()));
    }
    for (i, (m, r)) in a.iter().zip(b).enumerate() {
        if m["type"] != r["type"] {
            return Err(format!("vector {i} type"));
        }
        for k in ["startPoint", "endPoint", "centrePoint"] {
            for c in ["lat", "long"] {
                let (x, y) = (m[k][c].as_f64(), r[k][c].as_f64());
                if let (Some(x), Some(y)) = (x, y) {
                    if (x - y).abs() > 1e-4 {
                        return Err(format!("vector {i} {k}.{c} {x} vs {y}"));
                    }
                } else if x.is_some() != y.is_some() {
                    return Err(format!("vector {i} {k} presence"));
                }
            }
        }
    }
    Ok(())
}

const KEY_VARS: &[&str] = &[
    "A32NX_SPEEDS_GD",
    "A32NX_SPEEDS_S",
    "A32NX_SPEEDS_F",
    "A32NX_SPEEDS_VS",
    "A32NX_SPEEDS_VLS",
    "A32NX_SPEEDS_VMAX",
    "A32NX_SPEEDS_VFEN",
    "A32NX_SPEEDS_VAPP",
    "A32NX_SPEEDS_MANAGED_PFD",
    "A32NX_FMGC_FLIGHT_PHASE",
    "AIRLINER_FLIGHT_PHASE",
    "AIRLINER_V1_SPEED",
    "AIRLINER_VR_SPEED",
    "AIRLINER_V2_SPEED",
    "A32NX_FM_LATERAL_FLIGHTPLAN_AVAIL",
    "A32NX_FG_CROSS_TRACK_ERROR",
    "A32NX_FG_TRACK_ANGLE_ERROR",
    "A32NX_FG_PHI_COMMAND",
    "A32NX_EFIS_L_TO_WPT_BEARING",
    "A32NX_EFIS_L_TO_WPT_DISTANCE",
    "A32NX_EFIS_L_TO_WPT_ETA",
    "A32NX_EFIS_R_TO_WPT_DISTANCE",
    "A32NX_AIRLINER_CRUISE_ALTITUDE",
    "A32NX_PRESS_AUTO_LANDING_ELEVATION",
    "A32NX_FMGC_L_RNP",
    "A32NX_FMGC_L_NAV_ACCURACY_HIGH",
    "A32NX_EFIS_L_ND_FM_MESSAGE_FLAGS",
    "A32NX_FM1_HEALTHY_DISCRETE",
    "A32NX_FMC_A_IS_HEALTHY",
    "A32NX_FMS_L_FAILED",
    "A32NX_EFIS_L_MAP_PARTLY_DISPLAYED",
    "A32NX_MachPreselVal",
    "A32NX_SpeedPreselVal",
    "A32NX_FG_REQUESTED_VERTICAL_MODE",
];

#[test]
fn uplink_egll_kjfk_host_outputs_match_the_js_recording() {
    let Some(rec) = load() else {
        eprintln!("oracle missing, skipping");
        return;
    };
    let db = db();
    let inputs = inputs_from(&rec);
    let mut core = FmsCore::new();
    let mut outs = Vec::new();
    for _ in 0..6 {
        outs.push(core.tick(&db, 200., &inputs));
    }
    core.command(&db, Envelope { id: 1, command: HostCommand::UplinkSimbrief(canned_ofp()) });
    core.command(&db, Envelope { id: 2, command: HostCommand::Plan(Command::UplinkInsert) });
    for _ in 0..80 {
        outs.push(core.tick(&db, 200., &inputs));
    }
    let (vars, gd) = last_state(&outs);

    let mut differing = Vec::new();
    for name in KEY_VARS {
        let (mine, js) = (vars.get(*name).copied(), rec.vars.get(*name).copied());
        let ok = match (mine, js) {
            (Some(a), Some(b)) => (a - b).abs() <= 1e-6 * b.abs().max(1.),
            (None, None) => true,
            _ => false,
        };
        if !ok {
            differing.push(format!("{name}: host {mine:?} js {js:?}"));
        }
    }
    let mut vectors_compared = 0;
    for topic in ["A32NX_EFIS_VECTORS_L_ACTIVE", "A32NX_EFIS_VECTORS_L_MISSED", "A32NX_EFIS_VECTORS_L_DASHED", "A32NX_EFIS_VECTORS_L_TEMPORARY", "A32NX_EFIS_VECTORS_L_ACTIVE_EOSID", "A32NX_EFIS_VECTORS_L_ALTERNATE", "A32NX_EFIS_VECTORS_R_ACTIVE"] {
        let mine = gd.get(topic).cloned().unwrap_or(Value::Null);
        let js = rec.gd.get(topic).cloned().unwrap_or(Value::Null);
        vectors_compared += mine.as_array().map_or(0, Vec::len);
        if let Err(e) = same_vectors(&mine, &js) {
            differing.push(format!("{topic}: {e}"));
        }
    }
    eprintln!("compared {} vars and {} topics ({vectors_compared} vectors); differing: {differing:#?}", KEY_VARS.len(), 7);
    assert!(differing.is_empty(), "{differing:#?}");
}

#[test]
fn worker_restarts_after_a_panic_and_keeps_the_plan() {
    let db: Arc<dyn fms::navdb::NavDatabase + Send + Sync> = Arc::new(db());
    let mut host = FmsHost::new(db);
    host.send(HostCommand::UplinkSimbrief(canned_ofp()));
    host.send(HostCommand::Plan(Command::UplinkInsert));
    host.tick(200., Inputs { lat: 51.4706, lon: -0.4619, ..Inputs::default() });
    host.send(HostCommand::InjectPanic);
    host.tick(200., Inputs { lat: 51.4706, lon: -0.4619, ..Inputs::default() });
    let mut restarted = None;
    for _ in 0..200 {
        let out = host.take_outputs();
        if out.restarted.is_some() {
            restarted = out.restarted;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(restarted.map(|r| r.count), Some(1));
    let snap = host.snapshot().expect("snapshot");
    let active = snap.plans[PlanRef::Active.slot()].as_ref().expect("active plan restored");
    assert_eq!(active.destination_airport.as_ref().map(|a| a.ident.as_str()), Some("KJFK"));
}

struct OracleFacilities;

impl fms_host::facilities::Facilities for OracleFacilities {
    fn nearby(&self, kind: fms_host::facilities::NearbyKind, centre: fms_lnav::Coord, radius_nm: f64, _max: usize) -> Vec<fms_host::facilities::NearbyFacility> {
        let lon = fms_host::facilities::NearbyFacility {
            database_id: "VEG    LON  ".into(),
            ident: "LON".into(),
            location: fms::navdb::Location { lat: 51.487247222, long: -0.466538889, alt: None },
            section_code: 1,
            vhf_type: fms_host::facilities::vhf_type::VOR_DME,
        };
        let near = fms_lnav::geo::distance_to(centre, fms_lnav::Coord::new(lon.location.lat, lon.location.long)) <= radius_nm;
        if kind == fms_host::facilities::NearbyKind::VhfNavaid && near {
            vec![lon]
        } else {
            Vec::new()
        }
    }

    fn vhf_navaid(&self, database_id: &str) -> Option<fms_host::facilities::VhfNavaid> {
        (database_id == "VEG    LON  ").then(|| fms_host::facilities::VhfNavaid {
            database_id: database_id.into(),
            ident: "LON".into(),
            location: fms::navdb::Location { lat: 51.487247222, long: -0.466538889, alt: None },
            dme_location: None,
            frequency: 113.6,
            figure_of_merit: 3,
            vhf_type: fms_host::facilities::vhf_type::VOR_DME,
        })
    }
}

fn run_uplink(rec: &Recording, facilities: bool) -> Vec<Outputs> {
    let db = db();
    let inputs = inputs_from(rec);
    let mut core = if facilities { FmsCore::with_facilities(Arc::new(OracleFacilities)) } else { FmsCore::new() };
    let mut outs = Vec::new();
    for _ in 0..6 {
        outs.push(core.tick(&db, 200., &inputs));
    }
    core.command(&db, Envelope { id: 1, command: HostCommand::UplinkSimbrief(canned_ofp()) });
    core.command(&db, Envelope { id: 2, command: HostCommand::Plan(Command::UplinkInsert) });
    for _ in 0..80 {
        outs.push(core.tick(&db, 200., &inputs));
    }
    outs
}

fn bus_of(outs: &[Outputs]) -> Vec<(String, Value)> {
    outs.iter().flat_map(|o| o.bus.iter().cloned()).collect()
}

fn last_bus<'a>(bus: &'a [(String, Value)], topic: &str) -> Option<&'a Value> {
    bus.iter().rev().find(|(t, _)| t == topic).map(|(_, v)| v)
}

/// A receiving `FlightPlanManager`: rebuilds the plans' segments from the sync stream.
fn replay_plans(bus: &[(String, Value)]) -> BTreeMap<usize, BTreeMap<(bool, u64), Value>> {
    let mut plans: BTreeMap<usize, BTreeMap<(bool, u64), Value>> = BTreeMap::new();
    for (topic, d) in bus {
        let p = d["planIndex"].as_u64().unwrap_or(99) as usize;
        match topic.as_str() {
            "flightPlanManager.create" => {
                plans.insert(p, BTreeMap::new());
            }
            "flightPlanManager.delete" => {
                plans.remove(&p);
            }
            "flightPlanManager.copy" => {
                let t = d["targetPlanIndex"].as_u64().unwrap() as usize;
                let src = plans.get(&p).cloned().unwrap_or_default();
                plans.insert(t, src);
            }
            "flightPlanManager.swap" => {
                let t = d["targetPlanIndex"].as_u64().unwrap() as usize;
                let (a, b) = (plans.remove(&p), plans.remove(&t));
                if let Some(a) = a {
                    plans.insert(t, a);
                }
                if let Some(b) = b {
                    plans.insert(p, b);
                }
            }
            "flightPlan.setSegment" => {
                let key = (d["forAlternate"].as_bool().unwrap_or(false), d["segmentIndex"].as_u64().unwrap());
                plans.entry(p).or_default().insert(key, d["serialized"].clone());
            }
            _ => {}
        }
    }
    plans
}

fn legs_summary(seg: &Value) -> Vec<String> {
    seg["allLegs"]
        .as_array()
        .map(|a| a.iter().map(|l| if l["isDiscontinuity"] == true { "DISCO".to_string() } else { format!("{} {}", l["ident"].as_str().unwrap_or("?"), l["definition"]["type"].as_str().unwrap_or("?")) }).collect())
        .unwrap_or_default()
}

fn symbols_summary(v: Option<&Value>, vd: bool) -> Vec<String> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|s| {
                    let mut line = format!("{} {} {} {:.6},{:.6}", s["databaseId"], s["ident"], s["type"], s["location"]["lat"].as_f64().unwrap_or(f64::NAN), s["location"]["long"].as_f64().unwrap_or(f64::NAN));
                    let n = |k: &str| s[k].as_f64().map(|v| format!("{v:.3}")).unwrap_or_else(|| s[k].to_string());
                    if vd {
                        line += &format!(" pa={} met={}", n("predictedAltitude"), s["isAltitudeConstraintMet"]);
                    }
                    line += &format!(" d={}", n("distanceFromAirplane"));
                    line
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn uplink_efis_symbols_and_vd_symbols_match_the_js_recording() {
    let Some(rec) = load() else {
        eprintln!("oracle missing, skipping");
        return;
    };
    let outs = run_uplink(&rec, true);
    let (vars, gd) = last_state(&outs);
    let bus = bus_of(&outs);
    let mut differing = Vec::new();
    for side in ["L", "R"] {
        let topic = format!("A32NX_EFIS_{side}_SYMBOLS");
        let (mine, js) = (symbols_summary(gd.get(&topic), false), symbols_summary(rec.gd.get(&topic), false));
        if mine != js {
            differing.push(format!("{topic}: host {mine:#?} js {js:#?}"));
        }
        let topic = format!("vdSymbols_{side}");
        let js_vd = rec.bus.iter().rev().find(|(t, _)| *t == topic).map(|(_, v)| v);
        let (mine, js) = (symbols_summary(last_bus(&bus, &topic), true), symbols_summary(js_vd, true));
        if mine != js {
            differing.push(format!("{topic}: host {mine:#?} js {js:#?}"));
        }
        let mrp = vars.get(&format!("A32NX_EFIS_{side}_MRP_LAT")).and_then(|w| fms_host::inputs::arinc_value(*w));
        if mrp.is_none_or(|m| (m - 51.4706).abs() > 1e-3) {
            differing.push(format!("MRP_LAT {side}: {mrp:?}"));
        }
    }
    eprintln!("symbols differing: {differing:#?}");
    assert!(differing.is_empty(), "{differing:#?}");
}

#[test]
fn uplink_flight_plan_sync_stream_rebuilds_the_js_plans() {
    let Some(rec) = load() else {
        eprintln!("oracle missing, skipping");
        return;
    };
    let outs = run_uplink(&rec, false);
    let bus = bus_of(&outs);
    for (t, _) in &bus {
        if let Some(plain) = t.strip_prefix("SYNC_") {
            assert!(bus.iter().any(|(u, _)| u == plain), "{t} without its local twin");
        }
    }
    let (mine, js) = (replay_plans(&bus), replay_plans(&rec.bus));
    assert_eq!(mine.keys().collect::<Vec<_>>(), js.keys().collect::<Vec<_>>(), "plans present after the uplink");
    let mut compared = 0;
    let mut differing = Vec::new();
    for (p, js_segs) in &js {
        let my_segs = &mine[p];
        let keys: std::collections::BTreeSet<_> = js_segs.keys().chain(my_segs.keys()).collect();
        for k in keys {
            let a = my_segs.get(k).map(legs_summary).unwrap_or_default();
            let b = js_segs.get(k).map(legs_summary).unwrap_or_default();
            compared += b.len();
            if a != b {
                differing.push(format!("plan {p} alt {} segment {}: host {a:?} js {b:?}", k.0, k.1));
            }
        }
    }
    let fln = |b: &[(String, Value)]| last_bus(b, "flightPlan.setFlightNumber").map(|d| d["flightNumber"].clone());
    if fln(&bus) != fln(&rec.bus) {
        differing.push(format!("flight number {:?} vs {:?}", fln(&bus), fln(&rec.bus)));
    }
    for key in ["costIndex", "cruiseFlightLevel", "pilotTropopause", "databaseTransitionAltitude", "databaseTransitionLevel"] {
        let topic = format!("flightPlan.setPerformanceData.{key}");
        let last = |b: &[(String, Value)]| b.iter().rev().find(|(t, d)| *t == topic && d["planIndex"] == 2).map(|(_, d)| d["value"].clone());
        if let Some(js) = last(&rec.bus) {
            let mine = last(&bus);
            let same = match (mine.as_ref().and_then(Value::as_f64), js.as_f64()) {
                (Some(a), Some(b)) => (a - b).abs() < 1e-9,
                _ => mine.as_ref() == Some(&js),
            };
            if !same {
                differing.push(format!("{topic}: host {:?} js {js}", last(&bus)));
            }
        }
    }
    eprintln!("compared {compared} legs; differing: {differing:#?}");
    assert!(differing.is_empty(), "{differing:#?}");
}

fn vnav_dump(raw: &Value, label: &str) -> Option<Value> {
    for e in raw["events"].as_array()? {
        let Some(json) = e.get("json").and_then(Value::as_str).filter(|j| j.contains("oracle_vnav_dump")) else { continue };
        let parsed: Value = serde_json::from_str(json).ok()?;
        for ev in parsed["events"].as_array()? {
            if ev["topic"] == "oracle_vnav_dump" && ev["data"]["label"] == label {
                return Some(ev["data"].clone());
            }
        }
    }
    None
}

#[test]
fn preflight_predictions_reach_the_snapshot_and_match_the_js_vnav_dump() {
    let Some(rec) = load_from("E:/fbw-build/fms-oracle/predictions-egll-kjfk.json") else {
        eprintln!("oracle missing, skipping");
        return;
    };
    let Some(dump) = vnav_dump(&rec.raw, "NOWIND") else { panic!("no NOWIND dump") };
    let db = db();
    let inputs = inputs_from(&rec);
    let mut core = FmsCore::new();
    let mut outs = Vec::new();
    for _ in 0..6 {
        outs.push(core.tick(&db, 200., &inputs));
    }
    core.command(&db, Envelope { id: 1, command: HostCommand::UplinkSimbrief(canned_ofp()) });
    core.command(&db, Envelope { id: 2, command: HostCommand::Plan(Command::UplinkInsert) });
    use fms_data::{Command as PerfCommand, Entry};
    for (id, entry) in [Entry::CostIndex(Some(30.)), Entry::CruiseFlightLevel(360.), Entry::ZeroFuelWeightKg(Some(330_000.)), Entry::ZeroFuelWeightCg(Some(38.5)), Entry::BlockFuelKg(Some(95_000.)), Entry::V2(Some(150.)), Entry::TakeoffFlaps(Some(1))].into_iter().enumerate() {
        core.command(&db, Envelope { id: 10 + id as u64, command: HostCommand::Perf(PerfCommand::Enter { plan: 0, entry }) });
    }
    for _ in 0..60 {
        outs.push(core.tick(&db, 200., &inputs));
    }
    let snap = core.snapshot();
    let back: fms_host::Snapshot = serde_json::from_str(&serde_json::to_string(&snap).unwrap()).unwrap();
    assert_eq!(back.predictions.len(), snap.predictions.len(), "predictions survive the snapshot round trip");
    assert!(back.predictions.iter().zip(&snap.predictions).all(|(a, b)| a.0 == b.0 && (a.1.seconds_from_present - b.1.seconds_from_present).abs() < 1e-6));
    assert!(!snap.predictions.is_empty(), "no predictions in the snapshot");
    assert!(snap.pseudo_waypoints.iter().any(|p| p.ident == "(T/C)"), "no T/C pseudo waypoint: {:?}", snap.pseudo_waypoints);
    let gcp: Vec<(f64, f64, f64)> = dump["checkpoints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["distanceFromStart"].as_f64().unwrap(), c["secondsFromPresent"].as_f64().unwrap(), c["altitude"].as_f64().unwrap()))
        .filter(|c| c.0 >= 0.)
        .collect();
    let golden_at = |d: f64| -> Option<(f64, f64)> {
        let k = gcp.windows(2).position(|w| w[0].0 <= d && d <= w[1].0)?;
        let (a, b) = (gcp[k], gcp[k + 1]);
        let r = if b.0 > a.0 { (d - a.0) / (b.0 - a.0) } else { 0. };
        Some((a.1 + r * (b.1 - a.1), a.2 + r * (b.2 - a.2)))
    };
    let mut worst = [0f64; 2];
    let mut compared = 0;
    for (leg, m) in snap.predictions.iter().filter(|(_, m)| m.distance_from_start < 2000.) {
        let Some((t, alt)) = golden_at(m.distance_from_start) else { continue };
        let d = [m.seconds_from_present - t, m.altitude - alt];
        eprintln!("leg {leg:>2} at {:>7.1} NM: dt {:+7.2} s dalt {:+7.1} ft", m.distance_from_start, d[0], d[1]);
        for k in 0..2 {
            worst[k] = worst[k].max(d[k].abs());
        }
        compared += 1;
    }
    for g in dump["waypointPredictions"].as_array().unwrap() {
        let leg = g["leg"].as_u64().unwrap() as usize;
        let m = &snap.predictions[&leg];
        eprintln!("leg {leg:>2} along-track distance host {:.2} js {:.2}", m.distance_from_start, g["distanceFromStart"].as_f64().unwrap());
    }
    let (_, gd) = last_state(&outs);
    let bus = bus_of(&outs);
    let vd = last_bus(&bus, "vdSymbols_L").and_then(Value::as_array).map_or(0, Vec::len);
    eprintln!("{compared} predictions on the JS profile: worst time {:.2} s, altitude {:.1} ft; vdSymbols_L {vd}; ND symbols {}", worst[0], worst[1], gd.get("A32NX_EFIS_L_SYMBOLS").and_then(Value::as_array).map_or(0, Vec::len));
    assert!(compared >= 6, "compared {compared}");
    assert!(worst[0] <= 2., "time");
    assert!(worst[1] <= 10., "altitude");
    assert!(last_bus(&bus, "a32nx_fms_vertical_target_profile").and_then(Value::as_array).is_some_and(|a| !a.is_empty()), "vertical target profile");
}

#[test]
fn engine_out_vectors_follow_the_eosid_after_the_branch() {
    let Some(rec) = load() else {
        eprintln!("oracle missing, skipping");
        return;
    };
    let outs = run_uplink(&rec, false);
    let (_, gd) = last_state(&outs);
    let mine = gd.get("A32NX_EFIS_VECTORS_L_ACTIVE_EOSID").cloned().unwrap_or(Value::Null);
    let js = rec.gd.get("A32NX_EFIS_VECTORS_L_ACTIVE_EOSID").cloned().unwrap_or(Value::Null);
    same_vectors(&mine, &js).unwrap();
}

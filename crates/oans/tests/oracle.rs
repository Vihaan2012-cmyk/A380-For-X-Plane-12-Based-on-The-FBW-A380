use std::collections::BTreeSet;
use std::path::PathBuf;

use oans::advisory::RunwayArea;
use oans::arinc::Arinc;
use oans::btv::{NavDb, Oans, Runway};
use oans::geo::{distance_to, Coordinates};
use oans::markers::{MarkerManager, Symbol};
use oans::oanc::{Feature, Geometry, Props};
use serde_json::{json, Value};

struct ScenarioNav(Value);

fn coord(v: &Value) -> Coordinates {
    Coordinates { lat: v["lat"].as_f64().unwrap(), long: v["long"].as_f64().unwrap() }
}

impl NavDb for ScenarioNav {
    fn airport_location(&self, icao: &str) -> Option<Coordinates> {
        self.0.get(icao).map(|a| coord(&a["location"]))
    }

    fn runways(&self, icao: &str) -> Vec<Runway> {
        self.0
            .get(icao)
            .map(|a| {
                a["runways"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| Runway {
                        ident: r["ident"].as_str().unwrap().to_string(),
                        threshold_location: coord(&r["thresholdLocation"]),
                        bearing: r["bearing"].as_f64().unwrap(),
                        length: r["length"].as_f64().unwrap(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn nearby_airports(&self, ppos: Coordinates, radius_nm: f64, max_results: usize) -> Vec<String> {
        let mut out = Vec::new();
        for (ident, a) in self.0.as_object().unwrap() {
            if distance_to(ppos, coord(&a["location"])) <= radius_nm {
                out.push(ident.clone());
            }
        }
        out.truncate(max_results);
        out
    }
}

fn pos(v: &Value) -> [f64; 2] {
    [v[0].as_f64().unwrap(), v[1].as_f64().unwrap()]
}

fn line(v: &Value) -> Vec<[f64; 2]> {
    v.as_array().unwrap().iter().map(pos).collect()
}

fn index_of(topic: &str, prefix: &str) -> Option<usize> {
    topic.strip_prefix(prefix).and_then(|n| n.parse::<usize>().ok()).map(|n| n - 1)
}

struct Ctx {
    arp: Coordinates,
    areas: Vec<RunwayArea>,
    features: Vec<Feature>,
}

fn rings(v: &Value) -> Vec<Vec<[f64; 2]>> {
    v.as_array().unwrap().iter().map(line).collect()
}

fn areas(v: &Value) -> Vec<RunwayArea> {
    v.as_array().map_or_else(Vec::new, |a| {
        a.iter().map(|r| RunwayArea { idrwy: r["idrwy"].as_str().map(str::to_string), rings: rings(&r["rings"]) }).collect()
    })
}

fn features(v: &Value) -> Vec<Feature> {
    v.as_array().map_or_else(Vec::new, |a| {
        a.iter()
            .map(|f| {
                let p = &f["properties"];
                let g = &f["geometry"];
                let s = |k: &str| p[k].as_str().map(str::to_string);
                Feature {
                    feattype: p["feattype"].as_u64().unwrap() as u8,
                    props: Props { id: p["id"].as_i64(), idlin: s("idlin"), idthr: s("idthr"), idstd: s("idstd"), ..Default::default() },
                    geometry: match g["type"].as_str().unwrap() {
                        "Point" => Geometry::Point(pos(&g["coordinates"])),
                        "LineString" => Geometry::LineString(line(&g["coordinates"])),
                        _ => Geometry::Polygon(rings(&g["coordinates"])),
                    },
                }
            })
            .collect()
    })
}

fn apply_marker(m: &mut MarkerManager, step: &Value, features: &[Feature]) {
    let a = &step["args"];
    let n = |i: usize| a[i].as_i64().unwrap();
    match step["fn"].as_str().unwrap() {
        "addCrossAtFeature" => m.add_symbol_at_feature(Symbol::Cross, features, n(0), n(1) as u8),
        "addFlagAtFeature" => m.add_symbol_at_feature(Symbol::Flag, features, n(0), n(1) as u8),
        "removeCrossAtFeature" => m.remove_symbol_at_feature(Symbol::Cross, n(0), n(1) as u8),
        "removeFlagAtFeature" => m.remove_symbol_at_feature(Symbol::Flag, n(0), n(1) as u8),
        "addCross" => m.add(Symbol::Cross, pos(&a[0]), None),
        "addFlag" => m.add(Symbol::Flag, pos(&a[0]), None),
        "removeCross" => m.remove(Symbol::Cross, n(0) as u32),
        "removeFlag" => m.remove(Symbol::Flag, n(0) as u32),
        "eraseAllCrosses" => m.erase_all(Symbol::Cross),
        "eraseAllFlags" => m.erase_all(Symbol::Flag),
        other => panic!("unknown marker fn {other}"),
    }
}

fn native_stop_lines(o: &Oans) -> Value {
    let lines = o.stop_lines(o.stop_line_radio_altitude());
    Value::Array(
        lines
            .iter()
            .map(|l| {
                let (text, p) = match l.kind {
                    "DRY" => ("DRY", l.points[2]),
                    "WET" => ("WET", l.points[2]),
                    _ => ("", l.points[0]),
                };
                json!({"text": text, "style": format!("btv-stop-line-{}", l.colour), "position": p})
            })
            .collect(),
    )
}

fn native_symbols(m: &MarkerManager) -> Vec<Value> {
    let mut v: Vec<Value> = m.labels().iter().map(|l| json!({"text": l.text, "style": l.style.as_str(), "position": l.position})).collect();
    v.sort_by_key(|x| x.to_string());
    v
}

fn apply(o: &mut Oans, m: &mut MarkerManager, step: &Value, c: &Ctx) {
    let arp = c.arp;
    match step["op"].as_str().unwrap() {
        "ppos_local" => o.aircraft_ppos_local = pos(&step["pos"]),
        "ac_on_ground" => o.aircraft_on_ground = step["value"].as_bool().unwrap(),
        "marker" => apply_marker(m, step, &c.features),
        "rwy_ahead" => {
            o.aircraft_ppos_local = pos(&step["ppos_local"]);
            let g = Coordinates { lat: step["lat"].as_f64().unwrap(), long: step["long"].as_f64().unwrap() };
            for _ in 0..step["repeat"].as_u64().unwrap() {
                o.update_rwy_ahead_advisory(g, arp, step["bearing"].as_f64().unwrap(), &c.areas, step["now"].as_f64().unwrap());
            }
        }
        "tick" => o.on_update(step["ms"].as_f64().unwrap()),
        "select_runway" => o.select_runway_from_oans(
            step["runway"].as_str().unwrap(),
            &line(&step["centerline"]),
            pos(&step["threshold"]),
            step["brngmag"].as_f64(),
        ),
        "select_exit" => o.select_exit_from_oans(step["exit"].as_str().unwrap(), &line(&step["line"]), Some(arp)),
        "bus" => {
            let topic = step["topic"].as_str().unwrap();
            let v = &step["value"];
            let raw = || Arinc::from_raw(v.as_f64().unwrap());
            if let Some(i) = index_of(topic, "ra_radio_altitude_") {
                o.inputs.radio_altitude[i] = raw();
            } else if let Some(i) = index_of(topic, "ir_vertical_speed_") {
                o.inputs.vertical_speed[i] = raw();
            } else if let Some(i) = index_of(topic, "ir_true_heading_") {
                o.inputs.true_heading[i] = raw();
            } else if let Some(i) = index_of(topic, "ir_latitude_") {
                o.inputs.latitude[i] = raw();
            } else if let Some(i) = index_of(topic, "ir_longitude_") {
                o.inputs.longitude[i] = raw();
            } else if let Some(i) = index_of(topic, "lgciu_discrete_word_2_") {
                o.inputs.lgciu_discrete_word_2[i] = raw();
            } else {
                match topic {
                    "a380x_btv_fws_flight_phase" => o.inputs.fws_flight_phase = v.as_f64().unwrap(),
                    "a380x_btv_auto_brake_mode" => o.inputs.auto_brake_mode = v.as_f64().unwrap(),
                    "a380x_btv_auto_brake_active" => o.inputs.auto_brake_active = v.as_bool().unwrap(),
                    "fmsLandingRunway" => o.set_fms_landing_runway(v.as_str()),
                    "groundSpeed" => o.ground_speed = raw(),
                    "a380x_btv_dry_stopping_distance" => o.dry_stopping_distance = v.as_f64().unwrap(),
                    "a380x_btv_wet_stopping_distance" => o.wet_stopping_distance = v.as_f64().unwrap(),
                    "a380x_btv_stop_bar_distance" => o.live_stopping_distance = v.as_f64().unwrap(),
                    "oansManualStoppingDistance" => o.set_manual_stopping_distance(v.as_f64()),
                    other => o.bus_event(other, v.clone()),
                }
            }
        }
        other => panic!("unknown step {other}"),
    }
}

fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            (x - y).abs() <= 1e-9 * x.abs().max(y.abs()).max(1.0)
        }
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(v, w)| same(v, w)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w))),
        _ => a == b,
    }
}

fn golden_dir() -> PathBuf {
    PathBuf::from(std::env::var("OANS_ORACLE_DIR").unwrap_or_else(|_| "E:/fbw-build/oans-oracle".into()))
}

#[test]
fn native_matches_fbw_javascript_in_every_scenario() {
    let dir = golden_dir();
    let Ok(read) = std::fs::read_dir(&dir) else {
        eprintln!("no goldens in {}; run the oracle first", dir.display());
        return;
    };
    let mut checked = 0;
    for entry in read.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let golden: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let scenario = &golden["scenario"];
        let name = scenario["name"].as_str().unwrap();
        let nav = ScenarioNav(scenario["airports"].clone());
        let c = Ctx { arp: coord(&scenario["arp"]), areas: areas(&scenario["runway_areas"]), features: features(&scenario["features"]) };
        let mut oans = Oans::new(&nav);
        let mut markers = MarkerManager::new();
        let steps = scenario["steps"].as_array().unwrap();
        let snaps = golden["snapshots"].as_array().unwrap();
        assert_eq!(steps.len(), snaps.len());
        for (i, (step, snap)) in steps.iter().zip(snaps).enumerate() {
            apply(&mut oans, &mut markers, step, &c);
            if let Some(want) = snap.get("stopLines") {
                let got = native_stop_lines(&oans);
                assert!(same(&got, want), "{name} step {i} {step}: stop lines: native {got} fbw {want}");
            }
            if let Some(want) = snap.get("symbols") {
                let mut want: Vec<Value> = want.as_array().unwrap().clone();
                want.sort_by_key(|x| x.to_string());
                let got = native_symbols(&markers);
                assert!(same(&Value::Array(got.clone()), &Value::Array(want.clone())), "{name} step {i}: symbols: native {got:?} fbw {want:?}");
            }
            if let Some(want) = snap.get("symbolIds").filter(|v| !v.is_null()) {
                let s = markers.symbols_for_feature_ids();
                let got = json!({"featureIdsWithCrosses": s.feature_ids_with_crosses, "featureIdsWithFlags": s.feature_ids_with_flags});
                assert_eq!(&got, want, "{name} step {i}: symbol ids");
            }
            let want_vars = snap["vars"].as_object().unwrap();
            for (k, v) in want_vars {
                let got = oans.out.vars.get(k).copied();
                assert_eq!(got, v.as_f64(), "{name} step {i} {step}: var {k}");
            }
            let got_keys: BTreeSet<_> = oans.out.vars.keys().cloned().collect();
            let want_keys: BTreeSet<_> = want_vars.keys().cloned().collect();
            assert_eq!(got_keys, want_keys, "{name} step {i}: var set");
            let want_bus = snap["bus"].as_object().unwrap();
            let got_keys: BTreeSet<_> = oans.out.bus.keys().cloned().collect();
            let want_keys: BTreeSet<_> = want_bus.keys().cloned().collect();
            assert_eq!(got_keys, want_keys, "{name} step {i} {step}: bus topics");
            for (k, v) in want_bus {
                assert!(same(&oans.out.bus[k], v), "{name} step {i} {step}: bus {k}: native {} fbw {}", oans.out.bus[k], v);
            }
        }
        checked += 1;
    }
    assert!(checked > 0, "no scenario goldens found in {}", dir.display());
}

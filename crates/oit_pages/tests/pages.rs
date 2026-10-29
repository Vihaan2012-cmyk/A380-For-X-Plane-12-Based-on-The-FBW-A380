use oit_pages::avncs::*;
use oit_pages::flt_ops::{ResultSource, ToPerfModel};
use oit_pages::inflight_model as ifm;
use oit_pages::ops_pages::*;
use oit_pages::perf_common::fixed;
use oit_pages::perf_model::*;
use oit_pages::perf_pages::*;
use oit_pages::to_compute::compute_results;
use serde_json::{json, Value};

fn tree(model: Value) -> Value {
    let out = oit_pages::build_json(&model.to_string()).unwrap();
    serde_json::from_str(&out).unwrap()
}

fn flt_page(page: Value) -> Value {
    tree(json!({"flt_ops": {"page": page}}))
}

fn cls(v: &Value) -> Vec<String> {
    v["cls"].as_array().map(|a| a.iter().map(|c| c.as_str().unwrap().to_string()).collect()).unwrap_or_default()
}

fn find_all<'a>(v: &'a Value, class: &str, out: &mut Vec<&'a Value>) {
    if cls(v).iter().any(|c| c == class) {
        out.push(v);
    }
    if let Some(kids) = v["kids"].as_array() {
        for k in kids {
            find_all(k, class, out);
        }
    }
}

fn all<'a>(v: &'a Value, class: &str) -> Vec<&'a Value> {
    let mut out = vec![];
    find_all(v, class, &mut out);
    out
}

fn own_text(v: &Value) -> String {
    v["kids"].as_array().map(|k| k.iter().filter_map(|x| x["text"].as_str()).collect::<Vec<_>>().join("")).unwrap_or_default()
}

fn deep_text(v: &Value) -> String {
    let mut s = String::new();
    if let Some(t) = v["text"].as_str() {
        s.push_str(t);
    }
    if let Some(k) = v["kids"].as_array() {
        for x in k {
            s.push_str(&deep_text(x));
        }
    }
    s
}

fn all_texts(v: &Value) -> Vec<String> {
    let mut out = vec![];
    fn walk(v: &Value, out: &mut Vec<String>) {
        if let Some(t) = v["text"].as_str() {
            out.push(t.to_string());
        }
        if let Some(k) = v["kids"].as_array() {
            for x in k {
                walk(x, out);
            }
        }
    }
    walk(v, &mut out);
    out
}

fn grid_pairs(grid: &Value) -> Vec<(String, String)> {
    let kids = grid["kids"].as_array().unwrap();
    kids.chunks(2).map(|c| (deep_text(&c[0]), deep_text(&c[1]))).collect()
}

fn runway() -> Value {
    json!({"ident": "16R", "elevation_ft": 21.0, "slope_percent": 0.2, "tora": 4000.0, "asda": 4100.0, "lda": 3800.0, "width": 60.0, "bearing_true": 163.0})
}

#[test]
fn js_to_fixed_ties_round_away_from_zero() {
    assert_eq!(fixed(0.5, 0), "1");
    assert_eq!(fixed(2.5, 0), "3");
    assert_eq!(fixed(-2.5, 0), "-3");
    assert_eq!(fixed(1.25, 1), "1.3");
    assert_eq!(fixed(0.125, 2), "0.13");
    assert_eq!(fixed(1.005, 2), "1.00");
    assert_eq!(fixed(-0.0, 1), "0.0");
}

fn to_model(extra: Value) -> ToPerfModel {
    let mut base = json!({
        "airport": {"text": "YSSY"},
        "runway": runway(),
        "weather": {"oat": 22.0, "qnh": 1015.0, "wind_direction_true": 180.0, "wind_kt": 10.0},
        "weights": {"gross_weight_kg": 450000.0, "cg": 34.0},
        "compute": {"pressed": true}
    });
    for (k, v) in extra.as_object().unwrap() {
        base[k] = v.clone();
    }
    serde_json::from_value(base).unwrap()
}

#[test]
fn takeoff_results_come_from_the_model_when_there_is_no_report() {
    let m = to_model(json!({}));
    let r = compute_results(&m, m.compute.as_ref().unwrap());
    assert_eq!(r.source, ResultSource::Model);
    assert_eq!(r.runway_text, "YSSY/16R");

    let rwy = m.runway.as_ref().unwrap();
    let hw = 10.0 * ((180.0f64 - 163.0) * std::f64::consts::PI / 180.0).cos();
    let direct = takeoff(&TakeoffInput {
        atm: atmos(rwy.elevation_ft, 1015.0, 22.0),
        oat: 22.0,
        tow_t: 450.0,
        cg: 34.0,
        conf: ToConfChoice::Auto,
        flex: true,
        derate_pct: 0.0,
        rating_kn: 356.81,
        packs: true,
        ai: AntiIce::Off,
        slope: 0.2,
        cond: TO_COND_DRY,
        toda: 4000.0,
        asda: 4100.0,
        lineup: lineup("0"),
        hw,
        cal: NO_CALIBRATION,
    });
    assert_eq!(r.v1, format!("{} KT", direct.v1));
    assert_eq!(r.vr, format!("{} KT", direct.vr));
    assert_eq!(r.v2, format!("{} KT", direct.v2));
    assert_eq!(r.flaps, format!("CONF {}", direct.conf.label()));
    assert_eq!(r.flex, direct.flex.map_or("TOGA".to_string(), |f| format!("{f}°C")));
    assert!(r.ths.starts_with("UP") || r.ths.starts_with("DN"));
    assert!(r.mtow.ends_with(" T"));
    assert!(r.note.contains("OIS MODEL RESULT, NOT AIRBUS-CERTIFIED."));
    assert!(r.note.starts_with("NO SIMBRIEF TAKE-OFF REPORT: OIS MODEL RESULT"));
    assert!(r.note.contains("NO X-PLANE CALIBRATION YET"));
}

#[test]
fn takeoff_results_note_the_missing_inputs() {
    let m = to_model(json!({"runway": null}));
    let r = compute_results(&m, m.compute.as_ref().unwrap());
    assert_eq!(r.source, ResultSource::None);
    assert_eq!(r.v1, "NOT AVAIL");
    assert_eq!(r.runway_text, "----/---");
    assert_eq!(r.note, "NO SIMBRIEF TAKE-OFF REPORT. SELECT A RUNWAY FOR THE OIS MODEL.");

    let m = to_model(json!({"weights": {}}));
    let r = compute_results(&m, m.compute.as_ref().unwrap());
    assert_eq!(r.note, "NO SIMBRIEF TAKE-OFF REPORT. AIRCRAFT WEIGHT NOT YET COMPUTED FOR THE OIS MODEL.");

    let m = to_model(json!({"compute": {"pressed": false}}));
    let r = compute_results(&m, m.compute.as_ref().unwrap());
    assert_eq!(r.v1, "NOT AVAIL");
    assert!(r.note.starts_with("PRESS COMPUTE."));
}

#[test]
fn takeoff_results_prefer_the_simbrief_report() {
    let tlr = json!({
        "runway": [{"identifier": "16R", "speeds_v1": "148.4", "speeds_vr": "152", "speeds_v2": "158", "flap_setting": "2",
            "flex_temperature": "47", "length_tora": "4000", "distance_margin": "310"}],
        "conditions": {"planned_weight": "480000", "temperature": "22", "wind_direction": "90", "wind_speed": "8", "surface_condition": "Dry"}
    });
    let m = to_model(json!({"compute": {"pressed": true, "tlr": tlr}}));
    let r = compute_results(&m, m.compute.as_ref().unwrap());
    assert_eq!(r.source, ResultSource::Simbrief);
    assert_eq!(r.v1, "148 KT");
    assert_eq!(r.vr, "152 KT");
    assert_eq!(r.v2, "158 KT");
    assert_eq!(r.flaps, "CONF 2");
    assert_eq!(r.flex, "47°C");
    assert_eq!(r.stop_margin, "310 M");
    assert_eq!(r.ths, "NOT AVAIL");
    assert_eq!(
        r.note,
        "SIMBRIEF TAKE-OFF REPORT, RWY 16R AT 480.0 T (SIMBRIEF ASSUMED 22°C, WIND 090/8KT, DRY): SIMBRIEF PLANNING DATA, NOT AIRBUS-CERTIFIED A380 PERFORMANCE. THS AND PERF MTOW ARE NOT IN THE REPORT."
    );

    let tlr_ft = json!({"runway": {"identifier": "16R", "thrust_setting": "full", "length": "13123", "distance_margin": "1000"}, "conditions": {"planned_weight": "1058219"}});
    let m = to_model(json!({"compute": {"pressed": true, "tlr": tlr_ft, "tlr_weight_units": "LB"}, "units": {"weight": "klb", "distance": "ft"}}));
    let r = compute_results(&m, m.compute.as_ref().unwrap());
    assert_eq!(r.flex, "TOGA");
    assert_eq!(r.v1, "NOT AVAIL");
    assert_eq!(r.stop_margin, "1000 FT");
    assert!(r.note.contains("AT 1058.2 KLB"), "{}", r.note);

    let other = json!({"runway": [{"identifier": "34L"}]});
    let m = to_model(json!({"compute": {"pressed": true, "tlr": other}}));
    let r = compute_results(&m, m.compute.as_ref().unwrap());
    assert_eq!(r.source, ResultSource::Model);
    assert!(r.note.starts_with("THE SIMBRIEF TAKE-OFF REPORT HAS NO RWY 16R: OIS MODEL RESULT"), "{}", r.note);
}

#[test]
fn takeoff_page_shows_computed_results() {
    let t = flt_page(json!({"kind": "to_perf", "airport": {"text": "YSSY"}, "runway": runway(),
        "weather": {"oat": 22.0, "qnh": 1015.0, "wind_direction_true": 180.0, "wind_kt": 10.0},
        "weights": {"gross_weight_kg": 450000.0, "cg": 34.0}, "compute": {"pressed": true}}));
    let boxes = all(&t, "oit-labeled-box-container");
    let results = boxes.iter().find(|b| own_text(&b["kids"][0]) == "RESULTS").unwrap();
    let grid = all(results, "oit-perf-grid")[0];
    let pairs = grid_pairs(grid);
    assert_eq!(pairs[0].0, "V1");
    assert!(pairs[0].1.ends_with(" KT"));
    assert_eq!(pairs[1].0, "FLAPS");
    assert!(pairs[1].1.starts_with("CONF "));
    let first_cell = &grid["kids"][1];
    assert!(cls(first_cell).contains(&"oit-green-text".to_string()));
}

fn ldg_json(extra: Value) -> Value {
    let mut base = json!({
        "kind": "ldg_perf",
        "airport": {"text": "OMDB"},
        "runway": runway(),
        "landing_weight_kg": 380000.0,
        "lw_field": {"text": "380.0", "trailing": "T"},
        "lw_source": "CURRENT GROSS WEIGHT (NO FMS LANDING WEIGHT PREDICTION AVAILABLE)",
        "weather": {"oat": 30.0, "qnh": 1008.0, "wind_direction_true": 150.0, "wind_kt": 12.0},
        "cg": 36.0,
        "compute": {"pressed": true}
    });
    for (k, v) in extra.as_object().unwrap() {
        base[k] = v.clone();
    }
    base
}

#[test]
fn landing_page_computes_from_raw_inputs() {
    let t = flt_page(ldg_json(json!({})));
    assert_eq!(deep_text(all(&t, "oit-heading")[0]), "LDG PERF");
    let titles: Vec<String> = all(&t, "oit-labeled-box-label").iter().map(|x| own_text(x)).collect();
    assert_eq!(titles, ["ARRIVAL", "CONDITIONS", "RESULTS", "SIMBRIEF LANDING REPORT", "OPTIONS"]);

    let hw = 12.0 * ((150.0f64 - 163.0) * std::f64::consts::PI / 180.0).cos();
    let direct = landing(
        &LandingInput {
            atm: atmos(21.0, 1008.0, 30.0),
            lw_t: 380.0,
            cg: 36.0,
            conf: LdgConf::Full,
            mu: 0.38,
            rev: true,
            autoland: false,
            slope: 0.2,
            hw,
            lda: 3800.0,
            cal: NO_CALIBRATION,
        },
        &["LO"],
    );
    let boxes = all(&t, "oit-labeled-box-container");
    let results = boxes.iter().find(|b| own_text(&b["kids"][0]) == "RESULTS").unwrap();
    let pairs = grid_pairs(all(results, "oit-perf-grid")[0]);
    let get = |k: &str| pairs.iter().find(|p| p.0 == k).unwrap().1.clone();
    assert_eq!(get("LW USED"), "380.0 T");
    assert_eq!(get("LDA"), "3800 M");
    assert_eq!(get("LD"), format!("{} M", fixed(js_round(direct.rows[0].inflight), 0)));
    assert_eq!(get("FLD"), format!("{} M", fixed(js_round(direct.rows[0].dispatch), 0)));
    assert_eq!(get("STOP MARGIN / LD"), format!("{} M", fixed(js_round(3800.0 - direct.rows[0].inflight), 0)));
    assert_eq!(get("VAPP"), format!("{} KT (VLS {})", direct.vapp, direct.vls));
    assert_eq!(get("MAX CROSSWIND"), "40 KT");
    let note = all(results, "oit-perf-note").last().map(|n| deep_text(n)).unwrap();
    assert!(note.starts_with("OIS MODEL, AUTOBRAKE LO, CONF FULL: NOT AIRBUS-CERTIFIED."), "{note}");
    assert_eq!(deep_text(all(results, "oit-green-text")[0]), "OMDB/16R");

    let tlr_box = boxes.iter().find(|b| own_text(&b["kids"][0]) == "SIMBRIEF LANDING REPORT").unwrap();
    assert_eq!(deep_text(all(tlr_box, "oit-perf-note")[0]), "SYNC SIMBRIEF FOR THE SIMBRIEF LANDING REPORT (OFP GENERATED WITH RUNWAY ANALYSIS ON)");
}

#[test]
fn landing_options_change_the_result() {
    let base: LdgPerfModel = serde_json::from_value(ldg_json(json!({}))).unwrap();
    let wet: LdgPerfModel = serde_json::from_value(ldg_json(json!({
        "rwy_cond": {"text": "MEDIUM"}, "flaps": {"text": "CONF 3"}, "technique": {"text": "AUTOLAND"}, "brake": {"text": "HI"}, "reverse": {"text": "NO"}, "vpilot": {"text": "10"}
    })))
    .unwrap();
    let a = ldg_results(&base);
    let b = ldg_results(&wet);
    assert_ne!(a.ld, b.ld);
    assert_eq!(b.xwind, "20 KT");
    assert!(b.model_note.starts_with("OIS MODEL, AUTOBRAKE HI, CONF 3"));
    assert!(b.vapp.contains(" KT (VLS "));
}

#[test]
fn landing_results_report_missing_inputs_and_tlr() {
    let none: LdgPerfModel = serde_json::from_value(ldg_json(json!({"runway": null}))).unwrap();
    assert_eq!(ldg_results(&none).model_note, "SELECT A RUNWAY");
    let no_lw: LdgPerfModel = serde_json::from_value(ldg_json(json!({"landing_weight_kg": null}))).unwrap();
    assert_eq!(ldg_results(&no_lw).model_note, "ENTER THE LANDING WEIGHT");
    let idle: LdgPerfModel = serde_json::from_value(ldg_json(json!({"compute": {"pressed": false}}))).unwrap();
    let r = ldg_results(&idle);
    assert_eq!(r.model_note, "PRESS COMPUTE FOR THE OIS MODEL RESULT");
    assert_eq!(r.runway_text, "----/---");

    let tlr = json!({
        "runway": [{"identifier": "16R", "speeds_vref": "142.2", "max_weight_dry": "391000", "max_weight_wet": "372000"}],
        "conditions": {"flap_setting": "FULL", "temperature": "31", "wind_direction": "5", "wind_speed": "6", "surface_condition": "wet"}
    });
    let m: LdgPerfModel = serde_json::from_value(ldg_json(json!({"compute": {"pressed": true, "tlr": tlr}}))).unwrap();
    let r = ldg_results(&m);
    assert!(r.from_simbrief);
    assert_eq!(r.tlr_vref, "142 KT");
    assert_eq!(r.tlr_max_dry, "391.0 T");
    assert_eq!(r.tlr_max_wet, "372.0 T");
    assert_eq!(
        r.tlr_note,
        "SIMBRIEF LANDING REPORT, RWY 16R (SIMBRIEF ASSUMED FLAPS FULL, 31°C, WIND 005/6KT, WET): SIMBRIEF PLANNING DATA, NOT AIRBUS-CERTIFIED A380 PERFORMANCE"
    );
    let t = flt_page(ldg_json(json!({"compute": {"pressed": true, "tlr": tlr}})));
    let boxes = all(&t, "oit-labeled-box-container");
    let tlr_box = boxes.iter().find(|b| own_text(&b["kids"][0]) == "SIMBRIEF LANDING REPORT").unwrap();
    assert!(cls(&all(tlr_box, "oit-perf-grid")[0]["kids"][1]).contains(&"cyan".to_string()));
}

#[test]
fn in_flight_rows_follow_the_inflight_model() {
    let m: InFltModel = serde_json::from_value(json!({
        "module": "CRZ", "live_weight": false, "live_level": false, "weight_kg": 420000.0, "level": 360.0, "mach": 0.84, "isa_dev": 5.0,
        "live": {"fob_kg": 90000.0, "ff_kg_h": 9000.0}
    }))
    .unwrap();
    let rows = inflight_rows(&m);
    let c = ifm::cruise(420000.0, 360.0, 0.84, 5.0, 1.0, 4.0);
    assert_eq!(rows[0], ("FUEL FLOW".to_string(), format!("{} KG/H", fixed(js_round(c.fuel_flow_kg_h), 0))));
    assert_eq!(rows[1].1, format!("{} NM/T", fixed(c.specific_range_nm_per_t, 1)));
    assert_eq!(rows[4].0, "REC MAX");
    assert_eq!(rows[4].1, format!("FL{}", fixed(ifm::rec_max_fl(420000.0, 0.84, 5.0), 0)));
    assert_eq!(rows[8], ("LIVE FUEL FLOW".to_string(), "9000 KG/H".to_string()));
    assert_eq!(rows[9], ("LIVE ENDURANCE".to_string(), "10H00".to_string()));
}

fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

#[test]
fn in_flight_modules_and_live_inputs() {
    let mk = |module: &str, extra: Value| -> InFltModel {
        let mut v = json!({"module": module, "live_weight": false, "live_level": false, "weight_kg": 400000.0, "level": 350.0, "mach": 0.85, "distance_nm": 600.0, "live": {"fob_kg": 80000.0}});
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        serde_json::from_value(v).unwrap()
    };
    let hld = inflight_rows(&mk("HLD", json!({})));
    assert_eq!(hld.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(), ["HOLD SPEED (GD)", "FUEL FLOW", "FUEL PER 30 MIN", "MAX HOLD TIME", "GREEN DOT FL200", "GREEN DOT FL100"]);
    let des = inflight_rows(&mk("DES", json!({"elevation_ft": 500.0})));
    let d = ifm::descent(350.0, 500.0, 400000.0, 0.0);
    assert_eq!(des[0].1, format!("{} NM", fixed(js_round(d.distance_nm), 0)));
    let plan = inflight_rows(&mk("PLAN", json!({})));
    assert_eq!(plan.len(), 5);
    assert_eq!(plan[2], ("FOB NOW".to_string(), "80.0 T".to_string()));
    let atmo = inflight_rows(&mk("ATMO", json!({"live": {"fl": 350.0, "sat_c": -56.4, "isa_dev": -0.3, "wind_dir": 45.0, "wind_kt": 33.0, "gs_kt": 512.0}})));
    assert_eq!(atmo[0].1, "35000 FT");
    assert_eq!(atmo[1].1, "-56°C");
    assert_eq!(atmo[3].1, "0°C");
    assert_eq!(atmo[4].1, "045°/33 KT");

    let live = mk("CRZ", json!({"live_weight": true, "live_level": true, "live": {"weight_kg": 412345.0, "fl": 384.0, "mach": 0.832, "isa_dev": 3.4}}));
    let r = resolve_inputs(&live);
    assert_eq!(r.weight_kg, 412300.0);
    assert_eq!(r.level, 380.0);
    assert_eq!(r.mach, 0.83);
    assert_eq!(r.isa_dev, 3.0);

    let eoi = inflight_rows(&mk("CRZ", json!({"engines": 3})));
    assert_eq!(eoi[4].0, "EO CEILING");
    assert_eq!(eoi[5].1, "---");
}

#[test]
fn flight_model_learning_follows_the_source() {
    let mut cal = FlightModelCal::default();
    let live = Live { weight_kg: Some(400000.0), fl: Some(350.0), mach: Some(0.85), isa_dev: Some(0.0), ff_kg_h: 10000.0, ..Default::default() };
    learn_flight_model(&mut cal, &live, 0.0);
    let model = ifm::cruise(400000.0, 350.0, 0.85, 0.0, 1.0, 4.0).fuel_flow_kg_h;
    assert_eq!(cal.samples, 1);
    assert!((cal.calibration - (10000.0 / model).max(0.6).min(1.6)).abs() < 1e-12);
    learn_flight_model(&mut cal, &live, 800.0);
    assert_eq!(cal.samples, 1);
    let low = Live { ff_kg_h: 1000.0, ..live.clone() };
    learn_flight_model(&mut cal, &low, 0.0);
    assert_eq!(cal.samples, 1);
}

#[test]
fn in_flight_page_tree() {
    let t = flt_page(json!({"kind": "in_flt_perf", "live_weight": false, "live_level": false, "module": "DES", "engines": 3}));
    assert_eq!(deep_text(all(&t, "oit-heading")[0]), "IN-FLT PERF");
    let root = all(&t, "oit-perf-page")[0];
    let head = &root["kids"][0];
    let labels: Vec<String> = all(head, "mfd-button").iter().map(|b| deep_text(b)).collect();
    assert_eq!(labels, ["CRZ", "HLD", "DES", "PLAN", "ATMO", "AEO", "OEI", "TEO"]);
    let selected: Vec<String> = all(head, "selected").iter().map(|b| deep_text(b)).collect();
    assert_eq!(selected, ["DES", "OEI"]);
    let steppers = all(root, "oit-perf-key");
    let keys: Vec<String> = steppers.iter().map(|k| deep_text(k)).collect();
    assert_eq!(keys, ["WEIGHT", "FLIGHT LEVEL", "MACH", "TEMPERATURE", "FUEL FACTOR", "DISTANCE", "WIND", "DEST ELEVATION"]);
    let values: Vec<String> = all(root, "oit-green-text").iter().take(8).map(|k| deep_text(k)).collect();
    assert_eq!(values, ["450.0 T", "FL370", "M 0.85", "ISA +0", "1.00", "500 NM", "TL 0 KT", "0 FT"]);
    let boxes = all(root, "oit-labeled-box-label");
    assert_eq!(own_text(boxes.last().unwrap()), "DES RESULTS");
    let lives = all(&root["kids"][1], "selected").len();
    assert_eq!(lives, 0);
}

fn sheet_input() -> LoadsheetInput {
    LoadsheetInput {
        zfw: 330_450.4,
        gw: 480_100.0,
        main_door_open: 0.0,
        mac_zfw: 31.24,
        mac_tow: 33.96,
        pax_stations: vec![7.0, 255.0, 0.0, 1.0],
        water_pct: 82.4,
        reg: "A6EDA".into(),
        airline: "uae".into(),
        flight_number: "412".into(),
        zulu_seconds: 3.0 * 3600.0 + 25.0 * 60.0 + 10.0,
        day: 5.0,
        month: 10.0,
        year: 2026.0,
        origin: Some("YSSY".into()),
        destination: Some("OMDB".into()),
    }
}

#[test]
fn loadsheet_text_matches_the_source_layout() {
    let r = build_loadsheet("PRELIM", &sheet_input()).unwrap();
    assert_eq!(r.title, "LOADSHEET PRELIM 0325");
    assert_eq!(
        r.lines,
        [
            "UAE412/05 05/OCT/26",
            "YSSY OMDB A6EDA",
            "ZFW 330450 MAX 373000",
            "TOF 149650",
            "TOW 480100 MAX 512000",
            "MACZFW 31.2",
            "MACTOW 34.0",
            "UNDLD 31900",
            "PAX TTL 12",
            "POTABLE WATER 82PCT",
            "END/UAE412",
        ]
    );
    let mut open = sheet_input();
    open.main_door_open = 0.3;
    assert!(build_loadsheet("FINAL", &open).is_none());
    assert!(build_loadsheet("PRELIM", &open).is_some());
    let mut empty = sheet_input();
    empty.zfw = 0.0;
    assert!(build_loadsheet("PRELIM", &empty).is_none());
    let mut dry = sheet_input();
    dry.water_pct = 0.0;
    dry.origin = None;
    dry.destination = None;
    dry.reg = String::new();
    let r = build_loadsheet("FINAL", &dry).unwrap();
    assert_eq!(r.lines.len(), 9);
    assert_eq!(count_set_bits(255.0), 8);
}

#[test]
fn loadsheet_page_tree() {
    let t = flt_page(json!({"kind": "loadsheet", "input": serde_json::to_value(sheet_input_json()).unwrap()}));
    let page = all(&t, "framed")[0];
    let buttons = all(page, "mfd-button");
    assert_eq!(buttons.len(), 3);
    assert_eq!(deep_text(buttons[0]), "PRELIM");
    assert!(cls(buttons[0]).contains(&"selected".to_string()));
    assert!(!cls(buttons[1]).contains(&"selected".to_string()));
    let cyan = all(page, "cyan");
    assert_eq!(deep_text(cyan[0]), "LOADSHEET PRELIM 0325");
    let lines = all(page, "bigger");
    assert_eq!(lines.len(), 12);

    let none = flt_page(json!({"kind": "loadsheet"}));
    let page = all(&none, "framed")[0];
    assert_eq!(all_texts(page).last().unwrap(), "AIRCRAFT WEIGHT NOT YET COMPUTED");
    let fin = flt_page(json!({"kind": "loadsheet", "sheet": "FINAL", "input": {"zfw": 1.0, "gw": 2.0, "main_door_open": 1.0}}));
    let page = all(&fin, "framed")[0];
    assert_eq!(all_texts(page).last().unwrap(), "MAIN DOOR MUST BE CLOSED FOR A FINAL LOADSHEET");
    assert!(cls(all(page, "mfd-button")[1]).contains(&"selected".to_string()));
}

fn sheet_input_json() -> Value {
    json!({"zfw": 330450.4, "gw": 480100.0, "mac_zfw": 31.24, "mac_tow": 33.96, "pax_stations": [7, 255, 0, 1], "water_pct": 82.4, "reg": "A6EDA",
        "airline": "uae", "flight_number": "412", "zulu_seconds": 12310.0, "day": 5, "month": 10, "year": 2026, "origin": "YSSY", "destination": "OMDB"})
}

#[test]
fn ops_library_renders_the_shelves() {
    let t = flt_page(json!({"kind": "ops_library", "docs": [{"name": "Fuel policy", "folder": "ops", "type": "text"}, {"name": "Map", "folder": "", "type": "pdf"}], "title": "FUEL POLICY",
        "view": {"kind": "markdown", "text": "# Title\n## Sub\n- [x] done\n- item\n**bold** text `code`\r\nplain"}}));
    assert_eq!(deep_text(all(&t, "oit-heading")[0]), "OPS LIBRARY");
    let page = all(&t, "framed")[0];
    let entries = all(page, "oit-avncs-navigator-file");
    assert_eq!(entries.len(), 2);
    assert_eq!(deep_text(&entries[0]["kids"][0]), "FUEL POLICY");
    assert_eq!(deep_text(&entries[0]["kids"][1]), "OPS - TEXT");
    assert_eq!(deep_text(&entries[1]["kids"][1]), "PDF");
    let lines = all_texts(&page["kids"][1]["kids"][1]);
    assert_eq!(lines, ["TITLE", "SUB", "□ done", "• item", "bold text code", "plain"]);
    let shelf = all(page, "mfd-button");
    assert!(cls(shelf[0]).contains(&"selected".to_string()));

    let mel = flt_page(json!({"kind": "ops_library", "mel_shelf": true, "title": "MEL ATA 21 AIR CONDITIONING",
        "view": {"kind": "mel", "items": [{"id": "21-31-01", "title": "Pack", "category": "C", "conditions": "ONE MAY BE INOP", "ops_procedure_required": true}]}}));
    let page = all(&mel, "framed")[0];
    assert_eq!(all(page, "oit-avncs-navigator-file").len(), 21);
    let texts = all_texts(&page["kids"][1]["kids"][1]);
    assert_eq!(texts[0], "21-31-01  PACK");
    assert_eq!(texts[1], "CAT C   INSTALLED -   REQUIRED -      (O) ");
    assert_eq!(texts[2], "ONE MAY BE INOP");

    let down = flt_page(json!({"kind": "ops_library"}));
    assert_eq!(all_texts(all(&down, "framed")[0]).iter().filter(|x| x.as_str() == "LIBRARY SERVER NOT AVAILABLE").count(), 1);
    let none = flt_page(json!({"kind": "ops_library", "docs": []}));
    assert!(all_texts(all(&none, "framed")[0]).iter().any(|x| x.starts_with("NO DOCUMENTS.")));
}

#[test]
fn load_and_export_boxes() {
    let t = flt_page(json!({"kind": "load_box", "ofp_status": "YSSY-OMDB LOADED", "detail": ["FLIGHT UAE412  YSSY-OMDB  ALTN OMDW"]}));
    assert_eq!(deep_text(all(&t, "oit-heading")[0]), "LOAD BOX");
    let page = all(&t, "framed")[0];
    assert_eq!(own_text(all(page, "oit-labeled-box-label")[0]), "LOADABLE DATA");
    let greens: Vec<String> = all(page, "oit-green-text").iter().map(|x| deep_text(x)).collect();
    assert_eq!(greens, ["YSSY-OMDB LOADED", "NOT CHECKED"]);
    assert_eq!(all(page, "mfd-button").len(), 2);
    assert_eq!(all_texts(page).last().unwrap(), "FLIGHT UAE412  YSSY-OMDB  ALTN OMDW");

    let e = flt_page(json!({"kind": "export_box", "statuses": ["SAVED X", "NOTHING TO EXPORT"]}));
    assert_eq!(deep_text(all(&e, "oit-heading")[0]), "EXPORT BOX");
    let page = all(&e, "framed")[0];
    let greens: Vec<String> = all(page, "oit-green-text").iter().map(|x| deep_text(x)).collect();
    assert_eq!(greens, ["SAVED X", "NOTHING TO EXPORT", "READY", "READY", "READY"]);
    assert_eq!(all(page, "mfd-button").len(), 5);

    let ofp = OfpSummary {
        callsign: "UAE412".into(),
        origin: "YSSY".into(),
        destination: "OMDB".into(),
        route: "DCT".into(),
        cruise_altitude: 35000.0,
        cost_index: "30".into(),
        distance: "6500".into(),
        units: "kgs".into(),
        ..Default::default()
    };
    let lines = ofp_lines(&ofp);
    assert_eq!(lines[0], "FLIGHT UAE412  YSSY-OMDB  ALTN ----");
    assert_eq!(lines[2], "CRZ FL350  CI 30  DIST 6500 NM");
}

#[test]
fn date_formats_follow_the_source() {
    assert_eq!(format_date_time(None), "------ --:--");
    assert_eq!(format_date_time(Some(1_791_196_500_000.0)), "05-OCT 10:35");
    assert_eq!(format_scheduled_date(1_791_196_500_000.0), "05-OCT-26");
    assert_eq!(format_zulu(12310.0), "0325");
    assert_eq!(format_zulu(90000.0), "0100");
}

fn avncs(page: Value) -> Value {
    tree(json!({"avncs": {"page": page}}))
}

fn avncs_main(t: &Value) -> Value {
    let containers = all(t, "mfd-navigator-container");
    containers[containers.len() - 1].clone()
}

#[test]
fn company_com_menu_and_inbox() {
    let t = avncs(json!({"kind": "company_com", "uri": "nss-avncs/company-com/inbox", "content": {"kind": "inbox", "flt_number": "UAE412",
        "messages": [{"title": "LOADSHEET", "from": "AOC", "time_ms": 1791196500000.0, "is_new": true, "lines": ["A", "B"]}, {"title": "METAR", "from": "OPS", "time_ms": 1791196560000.0}]}}));
    let main = avncs_main(&t);
    assert_eq!(deep_text(all(&main, "oit-avncs-sub-header-title")[0]), "COMPANY COM");
    let names: Vec<String> = all(&main, "oit-avncs-navigator-file-name").iter().map(|x| deep_text(x)).collect();
    assert_eq!(names[0], "Inbox (2)");
    assert_eq!(names[1], "Outbox (0)");
    assert_eq!(names.len(), 27);
    let selected: Vec<String> = all(&main, "selected").iter().map(|x| deep_text(x)).collect();
    assert_eq!(selected[0], "Inbox (2)");
    let disabled = all(&main, "oit-avncs-navigator-file-name").iter().filter(|f| cls(f).contains(&"disabled".to_string())).count();
    assert_eq!(disabled, 23);
    let right = all(&main, "oit-avncs-navigator-right")[0];
    assert!(cls(right).contains(&"oit-centered".to_string()));
    let rk = right["kids"].as_array().unwrap();
    assert_eq!(rk.len(), 5);
    assert_eq!(deep_text(&rk[0]), "Inbox");
    let rows = all(&rk[1], "selected");
    assert_eq!(rows.len(), 3);
    assert_eq!(deep_text(rows[0]), "\u{2709}");
    assert_eq!(deep_text(rows[1]), "LOADSHEET  (AOC)");
    assert_eq!(deep_text(rows[2]), "05-OCT 10:35");
    let details = all_texts(&rk[2]);
    assert_eq!(details, ["Message is read", "05-OCT 10:35", "To : UAE412", "Subject : LOADSHEET"]);
    assert_eq!(deep_text(&rk[3]), "A\nB");
    let folders = all(&main, "oit-avncs-navigator-folder");
    assert_eq!(folders.len(), 9);
}

#[test]
fn flight_log_page() {
    let t = avncs(json!({"kind": "company_com", "uri": "nss-avncs/company-com/in-flight/flight-log", "content": {"kind": "flight_log",
        "flt_number": "UAE412", "origin": "YSSY", "destination": "OMDB", "sim_time_ms": 1791196500000.0,
        "out_time": 1791190000000.0, "off_time": 1791191000000.0, "out_fob": 120300.0, "off_fob": 118900.0, "flight_time": 7_200_000.0, "block_time": null, "metric": true}}));
    let main = avncs_main(&t);
    let right = all(&main, "oit-avncs-navigator-right")[0];
    let texts = all_texts(right);
    assert!(texts.contains(&"Flight Log".to_string()));
    assert!(texts.contains(&"05-OCT-26".to_string()));
    assert!(texts.contains(&"IN-FLIGHT".to_string()));
    assert!(texts.contains(&"120.3 T".to_string()), "{texts:?}");
    assert!(texts.contains(&"\u{a0}118.9 T".to_string()) || texts.contains(&"118.9 T".to_string()), "{texts:?}");
    assert!(texts.contains(&"----- -".to_string()));
    assert!(texts.contains(&"02:00:00".to_string()));
    assert!(texts.contains(&"--:--:--".to_string()));
    assert_eq!(all(right, "oit-ccom-flight-log-oooi-table")[0]["kids"].as_array().unwrap().len(), 15);
    let sel = all(&main, "selected");
    assert_eq!(deep_text(sel[0]), "Flight Log");
    assert_eq!(all(&main, "mfd-radio-button").len(), 1);
}

fn ev(kind: u32, id: i64, zulu: f64, ata: u32, msg: &str) -> Value {
    json!({"kind": kind, "id": id, "zulu_s": zulu, "phase": "CRZ", "ata": ata, "message": msg})
}

#[test]
fn maintenance_pages() {
    let events = json!([ev(1, 5, 3600.0, 21, "PACK 1 FAULT"), ev(2, 5, 3700.0, 21, "PACK 1 FAULT"), ev(3, 9, 3800.0, 24, "GEN FAULT"), ev(1, 5, 3900.0, 21, "PACK 1 FAULT"), ev(6, 2, 4000.0, 0, "CB")]);
    let t = avncs(json!({"kind": "maintenance", "uri": "nss-avncs/maintenance/current-flight-report", "content": {"kind": "report", "events": events}}));
    let main = avncs_main(&t);
    assert_eq!(deep_text(all(&main, "oit-avncs-sub-header-title")[0]), "MAINTENANCE");
    let names: Vec<String> = all(&main, "oit-avncs-navigator-file-name").iter().map(|x| deep_text(x)).collect();
    assert_eq!(names, ["Current Flight Report", "Post Flight Report", "By ATA Chapter", "Remote C/B Control", "MEL Deferred Items"]);
    assert_eq!(deep_text(all(&main, "selected")[0]), "Current Flight Report");
    let rows = all(&main, "oit-maint-row");
    assert_eq!(rows.len(), 2);
    assert_eq!(all_texts(rows[0]), ["0100", "CRZ", "21", "PACK 1 FAULT", "FAULT"]);
    assert!(cls(rows[0]["kids"][0].as_object().map(|_| &rows[0]["kids"][0]).unwrap()).contains(&"oit-amber-text".to_string()));
    assert_eq!(all_texts(rows[1])[4], "SYS FAULT");

    let post = avncs(json!({"kind": "maintenance", "uri": "nss-avncs/maintenance/system-status/21", "content": {"kind": "report", "post": true, "ata": 21, "ata_name": "Air Conditioning", "events": events}}));
    let main = avncs_main(&post);
    assert_eq!(all(&main, "selected").len(), 0);
    assert_eq!(deep_text(all(&main, "oit-ccom-headline")[0]), "Post Flight Report - ATA Air Conditioning");
    let rows = all(&main, "oit-maint-row");
    assert_eq!(rows.len(), 3);
    assert!(cls(&rows[1]["kids"][0]).contains(&"oit-green-text".to_string()));

    let empty = avncs(json!({"kind": "maintenance", "content": {"kind": "report"}}));
    assert_eq!(deep_text(all(&avncs_main(&empty), "oit-maint-empty")[0]), "NO FAULT");
    let status = avncs(json!({"kind": "maintenance", "uri": "nss-avncs/maintenance/system-status", "content": {"kind": "system_status", "chapters": [{"name": "Air Conditioning", "count": 2}]}}));
    let status_main = avncs_main(&status);
    let rows = all(&status_main, "oit-maint-row");
    assert_eq!(all_texts(rows[0]), ["AIR CONDITIONING", "2"]);
    let none = avncs(json!({"kind": "maintenance", "content": {"kind": "system_status", "chapters": []}}));
    assert_eq!(deep_text(all(&avncs_main(&none), "oit-maint-empty")[0]), "ALL SYSTEMS NORMAL");
    let log = avncs(json!({"kind": "maintenance", "uri": "nss-avncs/maintenance/logbook", "content": {"kind": "logbook", "rows": [{"label": "MEL 21-31-01", "covers": ["Pack 1", "Pack 2"], "hours": 71.46}, {"label": "MEL 24-10-01", "covers": [], "hours": 0.0}]}}));
    let log_main = avncs_main(&log);
    let rows = all(&log_main, "oit-maint-row");
    assert_eq!(all_texts(rows[0]), ["MEL 21-31-01", "PACK 1, PACK 2", "71.5 H"]);
    assert_eq!(all_texts(rows[1]), ["MEL 24-10-01", "EXPIRED"]);
}

#[test]
fn remote_cb_page_pages_and_filters() {
    let breakers: Vec<Value> = (0..30)
        .map(|i| json!({"id": format!("B{i}"), "name": format!("Fuel pump {i}"), "label": "FP", "bus": "AC 1", "ata": 28, "rating_a": 5.0, "state": if i == 3 { 2 } else { 0 }}))
        .collect();
    let abnormal = avncs(json!({"kind": "maintenance", "uri": "nss-avncs/maintenance/remote-cb", "content": {"kind": "remote_cb", "ctl_on": true, "selected": "B3", "breakers": breakers}}));
    let main = avncs_main(&abnormal);
    let right = all(&main, "oit-avncs-navigator-right")[0];
    let rows: Vec<&Value> = all(right, "oit-cb-row").into_iter().filter(|r| !cls(r).contains(&"oit-cb-header".to_string())).collect();
    assert_eq!(rows.len(), 1);
    assert!(cls(rows[0]).contains(&"selected".to_string()));
    assert_eq!(all_texts(rows[0]), ["FUEL PUMP 3", "28", "AC 1", "5 A", "TRIPPED THERMAL"]);
    assert_eq!(deep_text(all(right, "oit-maint-pager")[0]), "1 C/B  PAGE 1/1");
    assert_eq!(deep_text(all(right, "oit-maint-selected")[0]), "FUEL PUMP 3");
    assert_eq!(deep_text(all(right, "oit-maint-ctl-line")[0]), "REMOTE C/B CTL ON");
    let buttons = all(right, "mfd-button");
    let disabled: Vec<String> = buttons.iter().filter(|b| cls(b).contains(&"disabled".to_string())).map(|b| deep_text(b)).collect();
    assert_eq!(disabled, ["CLEAR", "PREV", "NEXT"]);

    let all_cb = avncs(json!({"kind": "maintenance", "uri": "nss-avncs/maintenance/remote-cb", "content": {"kind": "remote_cb", "show_all": true, "page": 2, "breakers": breakers}}));
    let right = all(&avncs_main(&all_cb), "oit-avncs-navigator-right")[0].clone();
    let rows: Vec<&Value> = all(&right, "oit-cb-row").into_iter().filter(|r| !cls(r).contains(&"oit-cb-header".to_string())).collect();
    assert_eq!(rows.len(), 6);
    assert_eq!(deep_text(all(&right, "oit-maint-pager")[0]), "30 C/B  PAGE 3/3");
    assert_eq!(deep_text(all(&right, "oit-maint-ctl-line")[0]), "REMOTE C/B CTL OFF - SELECT ELEC REMOTE C/B CTL ON (OVHD MAINT PANEL)");

    let search = avncs(json!({"kind": "maintenance", "content": {"kind": "remote_cb", "search": "FUEL 29", "breakers": breakers}}));
    let right = all(&avncs_main(&search), "oit-avncs-navigator-right")[0].clone();
    let rows: Vec<&Value> = all(&right, "oit-cb-row").into_iter().filter(|r| !cls(r).contains(&"oit-cb-header".to_string())).collect();
    assert_eq!(rows.len(), 1);
    let none = avncs(json!({"kind": "maintenance", "content": {"kind": "remote_cb", "search": "ZZZ", "breakers": breakers}}));
    assert_eq!(deep_text(all(&avncs_main(&none), "oit-maint-empty")[0]), "NO C/B MATCHES ZZZ");
    let nodata = avncs(json!({"kind": "maintenance", "content": {"kind": "remote_cb"}}));
    assert_eq!(deep_text(all(&avncs_main(&nodata), "oit-maint-empty")[0]), "C/B DATA NOT AVAILABLE");
}

#[test]
fn fbw_systems_pages() {
    let t = avncs(json!({"kind": "fbw_systems", "uri": "nss-avncs/a380x-systems/debug-data", "content": {"kind": "fws_debug", "transmitting": true, "rows": [{"label": "FWS1", "value": "OK"}]}}));
    let main = avncs_main(&t);
    assert_eq!(deep_text(all(&main, "oit-avncs-sub-header-title")[0]), "A380X SYSTEMS");
    let names: Vec<String> = all(&main, "oit-avncs-navigator-file-name").iter().map(|x| deep_text(x)).collect();
    assert_eq!(names, ["App & Ldg Capability", "FWS Debug"]);
    assert_eq!(deep_text(all(&main, "selected")[0]), "FWS Debug");
    let right = all(&main, "oit-avncs-navigator-right")[0];
    let heads: Vec<String> = all(right, "oit-ccom-headline").iter().map(|h| deep_text(h)).collect();
    assert_eq!(heads[0], "Flight Warning System Debug");
    assert!(heads[1].starts_with("Transmission StatusON"));
    assert_eq!(all(right, "mfd-dropdown-menu-element").len(), 2);
    assert_eq!(all(right, "mfd-dropdown-menu-element")[1]["id"], "FO_OIT_a380x_ois_fws_debug_data_enabled_control_1");
    assert_eq!(all_texts(all(right, "oit-ccom-inbox-msg-table")[0]), ["Label", "Value", "FWS1", "OK"]);
}

fn word(value: u32, ssm: u64) -> f64 {
    value as f64 + (ssm * 4294967296) as f64
}

fn float_word(value: f32, ssm: u64) -> f64 {
    value.to_bits() as f64 + (ssm * 4294967296) as f64
}

#[test]
fn approach_and_landing_capability() {
    let healthy = AppLdgInputs {
        ap1: true,
        ap2: true,
        athr_status: 2.0,
        athr_mode: 0.0,
        prim: [true, true, true],
        sec: [true, true, true],
        sfcc_words: [word(0, 3), word(0, 3)],
        rudder_words: [float_word((1u32 << 26) as f32, 3), word(0, 3)],
        hyd_green: true,
        hyd_yellow: true,
        fws_dw126: [float_word((1u32 << 15) as f32, 3), float_word((1u32 << 15) as f32, 3)],
        fcdc_healthy: [true, true],
        antiskid: true,
        fcdc_dw4: [float_word((1u32 << 24) as f32, 3), word(0, 3)],
        fcdc_dw8: [word(0, 3), word(0, 3)],
        adr_aoa: [word(0, 3), word(0, 3), word(0, 3)],
        engines: [true, true, true, true],
        apu_contactors: [false, false],
    };
    let s = app_ldg_state(&healthy);
    assert_eq!(s.capability, "LAND3 DUAL");
    assert!(s.two_ap && s.athr && s.two_prim && s.rudder_trim && s.two_fws && s.nws && s.three_adr && s.four_eng);

    let mut degraded = healthy.clone();
    degraded.ap2 = false;
    degraded.sfcc_words[1] = word(0, 0);
    degraded.fcdc_dw4 = [word(0, 0), float_word((1u32 << 22) as f32, 3)];
    degraded.adr_aoa[2] = word(0, 0);
    degraded.engines = [true, false, true, true];
    let s = app_ldg_state(&degraded);
    assert_eq!(s.capability, "LAND2");
    assert!(s.one_ap && !s.two_ap && !s.one_sfcc && s.two_adr && !s.three_adr && s.three_eng && !s.four_eng);
    degraded.apu_contactors = [true, false];
    assert!(app_ldg_state(&degraded).four_eng);

    let none = app_ldg_state(&AppLdgInputs::default());
    assert_eq!(none.capability, "APPR1");

    let t = avncs(json!({"kind": "fbw_systems", "uri": "nss-avncs/a380x-systems/app-ldg-cap", "content": {"kind": "app_ldg_cap", "ap1": true, "engines": [true, true, true, true]}}));
    let main = avncs_main(&t);
    let right = all(&main, "oit-avncs-navigator-right")[0];
    assert_eq!(deep_text(all(right, "oit-ccom-headline")[0]), "Approach & Landing Capability");
    let table = all(right, "oit-a380x-systems-app-ldg-cap-table")[0];
    assert_eq!(table["kids"].as_array().unwrap().len(), 4 + 13 * 4);
    let ap_row: Vec<bool> = table["kids"].as_array().unwrap()[5..8].iter().map(|c| cls(c).contains(&"green".to_string())).collect();
    assert_eq!(ap_row, [true, true, false]);
    assert_eq!(deep_text(&table["kids"][0]), "MonitoredEquipment");
}

#[test]
fn shell_still_builds_the_plain_pages() {
    let t = tree(json!({}));
    assert_eq!(t["tag"], "div");
}

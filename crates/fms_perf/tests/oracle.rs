use fms_perf::managed::*;
use fms_perf::perf::*;
use fms_perf::phase::*;
use fms_perf::speeds::*;
use serde_json::Value;
use std::path::Path;

fn oracle_dir() -> String {
    std::env::var("FMS_ORACLE_DIR").unwrap_or_else(|_| "E:/fbw-build/fms-oracle".into())
}

fn golden_dir() -> String {
    std::env::var("FMS_GOLDEN_DIR").unwrap_or_else(|_| "E:/golden-tour/flight-v4".into())
}

fn arinc_value(raw: f64) -> Option<f64> {
    let ssm = (raw / 4_294_967_296.0).floor() as u32;
    let low = (raw % 4_294_967_296.0) as u32;
    if ssm == 3 {
        Some(f32::from_bits(low) as f64)
    } else {
        None
    }
}

struct Oracle {
    events: Vec<Value>,
    snapshots: Vec<Value>,
}

fn load_oracle(name: &str) -> Option<Oracle> {
    let p = format!("{}/{}", oracle_dir(), name);
    if !Path::new(&p).exists() {
        eprintln!("skipped: {} missing", p);
        return None;
    }
    let v: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    Some(Oracle {
        events: v["events"].as_array().unwrap().clone(),
        snapshots: v["snapshots"].as_array().unwrap().clone(),
    })
}

fn last_value(o: &Oracle, var: &str) -> Option<f64> {
    o.events
        .iter()
        .filter(|e| e["var"].as_str() == Some(var))
        .last()
        .and_then(|e| e["value"].as_f64())
}

fn first_value(o: &Oracle, var: &str) -> Option<f64> {
    o.events
        .iter()
        .find(|e| e["var"].as_str() == Some(var))
        .and_then(|e| e["value"].as_f64())
}

const ORACLES: &[&str] = &[
    "city-pair-procedures.json",
    "edits-tmpy.json",
    "secondary.json",
    "uplink-egll-kjfk.json",
];

#[test]
fn oracle_speeds_lvars_at_first_fms_update() {
    let mut checked = 0;
    for name in ORACLES {
        let Some(o) = load_oracle(name) else { continue };
        let alt = arinc_value(first_value(&o, "L:A32NX_ADIRS_ADR_1_ALTITUDE").unwrap()).unwrap();
        let flap = first_value(&o, "FLAPS HANDLE INDEX").unwrap() as i32;
        let mut c = PerfComputer::new();
        let s = c.update_perf_speeds(&PerfInputs {
            phase: FlightPhase::Preflight,
            pressure_altitude_ft: Some(alt),
            flap_lever: flap,
            destination_elevation_ft: 12.0,
            ..Default::default()
        });
        let expect = |var: &str| last_value(&o, var).unwrap();
        assert_eq!(s.green_dot.unwrap(), expect("L:A32NX_SPEEDS_GD"), "{} GD", name);
        assert_eq!(s.slat_retraction.unwrap(), expect("L:A32NX_SPEEDS_S"), "{} S", name);
        assert_eq!(s.flap_retraction.unwrap(), expect("L:A32NX_SPEEDS_F"), "{} F", name);
        assert_eq!(s.vs1g, expect("L:A32NX_SPEEDS_VS"), "{} VS", name);
        assert_eq!(s.vls, expect("L:A32NX_SPEEDS_VLS"), "{} VLS", name);
        assert_eq!(s.vmax, expect("L:A32NX_SPEEDS_VMAX"), "{} VMAX", name);
        assert_eq!(s.vfe_next, expect("L:A32NX_SPEEDS_VFEN"), "{} VFEN", name);
        assert_eq!(s.approach_vapp.unwrap(), expect("L:A32NX_SPEEDS_VAPP"), "{} VAPP", name);
        checked += 1;
    }
    eprintln!("oracle files compared: {}", checked);
}

#[test]
fn oracle_snapshot_computation_parameters() {
    let mut checked = 0;
    for name in ORACLES {
        let Some(o) = load_oracle(name) else { continue };
        let mut c = PerfComputer::new();
        let s = c.update_perf_speeds(&PerfInputs {
            phase: FlightPhase::Preflight,
            pressure_altitude_ft: Some(83.0),
            destination_elevation_ft: 12.0,
            ..Default::default()
        });
        for snap in &o.snapshots {
            let p = &snap["state"]["guidance"]["computationParametersObserver"]["parameters"];
            if p.is_null() {
                continue;
            }
            assert_eq!(s.approach_vapp.unwrap(), p["approachSpeed"].as_f64().unwrap(), "{} approachSpeed", name);
            assert_eq!(s.approach_green_dot.unwrap(), p["cleanSpeed"].as_f64().unwrap(), "{} cleanSpeed", name);
            assert_eq!(s.approach_flap_retraction.unwrap(), p["flapRetractionSpeed"].as_f64().unwrap(), "{} F", name);
            assert_eq!(s.approach_slat_retraction.unwrap(), p["slatRetractionSpeed"].as_f64().unwrap(), "{} S", name);
            checked += 1;
        }
    }
    eprintln!("oracle snapshots compared: {}", checked);
}

#[test]
fn oracle_managed_econ_speeds() {
    let mut checked = 0;
    for name in ORACLES {
        let Some(o) = load_oracle(name) else { continue };
        for snap in &o.snapshots {
            let p = &snap["state"]["guidance"]["computationParametersObserver"]["parameters"];
            if p.is_null() {
                continue;
            }
            let ci = snap["state"]["plans"]["active"]["performance"]["costIndex"].as_f64();
            let climb = fms_perf::cost_index::managed_climb_speed(ci);
            assert!((climb - p["managedClimbSpeed"].as_f64().unwrap()).abs() < 1e-9, "{} climb {:?}", name, ci);
            let desc = fms_perf::cost_index::managed_descent_speed(ci, None);
            assert_eq!(desc, p["managedDescentSpeed"].as_f64().unwrap(), "{} descent", name);
            assert_eq!(fms_perf::cost_index::managed_cruise_speed(), p["managedCruiseSpeed"].as_f64().unwrap());
            assert_eq!(fms_perf::cost_index::managed_cruise_speed_mach(), p["managedCruiseSpeedMach"].as_f64().unwrap());
            assert_eq!(fms_perf::cost_index::managed_climb_speed_mach(), p["managedClimbSpeedMach"].as_f64().unwrap());
            checked += 1;
        }
    }
    eprintln!("oracle ECON snapshots compared: {}", checked);
}

struct GoldenState {
    run: String,
    capture: String,
    phase: i32,
    flaps: i32,
    cg: Option<f64>,
    gw: Option<f64>,
    alt: Option<f64>,
    cas: Option<f64>,
}

fn golden_states() -> Vec<GoldenState> {
    let mut out = Vec::new();
    let golden = golden_dir();
    let Ok(rd) = std::fs::read_dir(&golden) else {
        eprintln!("skipped: {} missing", golden);
        return out;
    };
    let mut runs: Vec<_> = rd.flatten().filter(|e| e.file_name().to_string_lossy().starts_with("emu-") && e.path().is_dir()).collect();
    runs.sort_by_key(|e| e.file_name());
    for run in runs {
        let caps = run.path().join("captures");
        let Ok(rd) = std::fs::read_dir(&caps) else { continue };
        let mut caps: Vec<_> = rd.flatten().collect();
        caps.sort_by_key(|e| e.file_name());
        for cap in caps {
            let pfd = cap.path().join("pfd-1.state.json");
            let mfd = cap.path().join("mfd.state.json");
            let (Ok(ps), Ok(ms)) = (std::fs::read_to_string(&pfd), std::fs::read_to_string(&mfd)) else { continue };
            let pv: Value = serde_json::from_str(&ps).unwrap();
            let mv: Value = serde_json::from_str(&ms).unwrap();
            let get = |v: &Value, key: &str| -> Option<f64> {
                v["vars"].as_object()?.iter().find(|(k, _)| k.contains(key)).and_then(|(_, x)| x.as_f64())
            };
            out.push(GoldenState {
                run: run.file_name().to_string_lossy().into_owned(),
                capture: cap.file_name().to_string_lossy().into_owned(),
                phase: get(&pv, "FMGC_FLIGHT_PHASE").unwrap_or(0.0) as i32,
                flaps: get(&pv, "FLAPS_HANDLE_INDEX").unwrap_or(0.0) as i32,
                cg: get(&pv, "FQMS_CENTER_OF_GRAVITY_MAC").and_then(arinc_value),
                gw: get(&mv, "FQMS_GROSS_WEIGHT").and_then(arinc_value),
                alt: get(&mv, "ADR_1_ALTITUDE").and_then(arinc_value),
                cas: get(&mv, "ADR_1_COMPUTED_AIRSPEED").and_then(arinc_value),
            });
        }
    }
    out
}

#[test]
fn golden_tour_states_give_consistent_speeds() {
    let states = golden_states();
    let mut computed = 0;
    for g in &states {
        let (Some(gw), Some(cg), Some(alt)) = (g.gw, g.cg, g.alt) else { continue };
        let phase = FlightPhase::from_index(g.phase).unwrap();
        let mut c = PerfComputer::new();
        let s = c.update_perf_speeds(&PerfInputs {
            phase,
            gross_weight_kg: Some(gw),
            gross_weight_cg: Some(cg),
            pressure_altitude_ft: Some(alt),
            computed_airspeed_kt: g.cas,
            flap_lever: g.flaps,
            destination_elevation_ft: 12.0,
            ..Default::default()
        });
        let tag = format!("{}/{}", g.run, g.capture);
        assert!(s.vls > 100.0 && s.vls < 330.0, "{} vls {}", tag, s.vls);
        assert!(s.vs1g < s.vls + 1.0, "{} vs {} vls {}", tag, s.vs1g, s.vls);
        assert!(s.green_dot.unwrap() > 150.0 && s.green_dot.unwrap() < 300.0, "{} gd", tag);
        assert!(s.vmax >= s.vls, "{} vmax {} vls {}", tag, s.vmax, s.vls);
        if let Some(cas) = g.cas {
            if cas > 120.0 && phase >= FlightPhase::Climb {
                assert!(cas > s.vls - 1.0, "{}: flying at CAS {} below VLS {} in the golden state", tag, cas, s.vls);
                assert!(cas < s.vmax + 1.0, "{}: CAS {} above VMAX {}", tag, cas, s.vmax);
            }
        }
        computed += 1;
    }
    eprintln!("golden states computed: {}", computed);
}

fn replay_inputs(g: &GoldenState, cruise_alt_ft: f64) -> PhaseInputs {
    PhaseInputs {
        on_ground: g.alt.map_or(true, |a| a < 50.0),
        pressure_altitude_ft: g.alt,
        baro_corrected_altitude_ft: None,
        vertical_mode: if g.alt.map_or(true, |a| a < 50.0) { vertical_mode::NONE } else { vertical_mode::CLB },
        indicated_altitude_ft: g.alt.unwrap_or(0.0),
        ground_speed_kt: if g.alt.map_or(true, |a| a < 50.0) { g.cas.unwrap_or(0.0).min(60.0) } else { g.cas.unwrap_or(0.0) },
        n1_pct: [90.0; 4],
        n2_pct: [90.0; 4],
        cruise_altitude_ft: cruise_alt_ft,
        fcu_selected_altitude_ft: cruise_alt_ft,
        fm_acc_alt_ft: Some(1580.0),
        fm_eo_acc_alt_ft: Some(1580.0),
        ..Default::default()
    }
}

#[test]
fn golden_tour_phase_matches_manager_replay() {
    let states = golden_states();
    let mut compared = 0;
    for g in &states {
        let Some(alt) = g.alt else { continue };
        let mut m = FlightPhaseManager::new();
        let mut inp = replay_inputs(g, 0.0);
        m.init(&inp);
        m.update(100.0, &inp);
        m.update(100.0, &inp);
        m.update(100.0, &inp);
        let expected = g.phase;
        let tag = format!("{}/{}", g.run, g.capture);
        if expected == 3 {
            assert_eq!(m.phase(), FlightPhase::Climb, "{} climb before cruise altitude is entered", tag);
            inp.cruise_altitude_ft = (alt / 100.0).round() * 100.0;
            inp.fcu_selected_altitude_ft = inp.cruise_altitude_ft;
            m.update(100.0, &inp);
            assert_eq!(m.phase() as i32, expected, "{}", tag);
        } else {
            assert_eq!(m.phase() as i32, expected, "{}", tag);
        }
        compared += 1;
    }
    eprintln!("golden phases replayed: {}", compared);
}

#[test]
fn golden_phase_replay_uses_published_lvars() {
    let mut m = FlightPhaseManager::new();
    let inp = PhaseInputs { on_ground: true, ..Default::default() };
    m.init(&inp);
    assert_eq!(m.fmgc_flight_phase_lvar(), 0.0);
    assert_eq!(m.airliner_flight_phase_lvar(), 0.0);
    let managed = ManagedSpeed { target_kt: None, is_mach: false };
    assert_eq!(fms_perf::outputs::managed_speed_lvar(&managed).value, 0.0);
    assert_eq!(fms_perf::outputs::managed_speed_reset_lvar().value, -1.0);
}

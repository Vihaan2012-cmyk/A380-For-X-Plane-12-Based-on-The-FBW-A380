use gpws::callouts::Event;
use gpws::vars::*;
use gpws::{Gpws, Inputs};
use serde_json::Value;
use std::path::PathBuf;

fn golden_dir() -> PathBuf {
    PathBuf::from(std::env::var("GPWS_ORACLE_DIR").unwrap_or_else(|_| "E:/fbw-build/gpws-oracle/goldens".to_string()))
}

fn names(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect()
}

fn replay(path: &PathBuf) -> usize {
    let golden: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let name = golden["scenario"].as_str().unwrap().to_string();
    let step = golden["step_ms"].as_f64().unwrap();
    let mut gpws = Gpws::new();
    if let Some(pins) = golden["pins"].as_f64() {
        gpws.auto_call_out_pins = pins as u32;
    }
    let mut inputs = Inputs::default();
    let frames = golden["frames"].as_array().unwrap();

    for (i, frame) in frames.iter().enumerate() {
        for (k, v) in frame["set"].as_object().unwrap() {
            assert!(inputs.set_var(k, v.as_f64().unwrap()), "{name}: unknown input {k}");
        }
        gpws.update(step, &inputs);
        let o = gpws.outputs();
        let at = format!("{name} frame {i}");

        let check = |var: &str, ours: f64| {
            let want = &frame["out"][var];
            if !want.is_null() {
                assert_eq!(want.as_f64().unwrap(), ours, "{at}: {var}");
            }
        };
        check(OUTPUT_GPWS_WARNING, f64::from(u8::from(o.gpws_warning_active)));
        check(OUTPUT_GS_WARNING, f64::from(u8::from(o.gs_warning_active)));
        check(OUTPUT_ALERT_1_WORD_1, o.alert_word_1);
        check(OUTPUT_ALERT_2_WORD_1, o.alert_word_1);
        check(OUTPUT_ALERT_1_WORD_2, o.alert_word_2);
        check(OUTPUT_ALERT_2_WORD_2, o.alert_word_2);

        let events = gpws.take_events();
        let enq: Vec<String> =
            events.iter().filter_map(|e| if let Event::Enqueue(n) = e { Some(n.to_string()) } else { None }).collect();
        let deq: Vec<String> =
            events.iter().filter_map(|e| if let Event::Dequeue(n) = e { Some(n.to_string()) } else { None }).collect();
        assert_eq!(names(&frame["enqueue"]), enq, "{at}: enqueue");
        assert_eq!(names(&frame["dequeue"]), deq, "{at}: dequeue");
        let played: Vec<String> = gpws.take_played_sounds().iter().map(|s| s.to_string()).collect();
        assert_eq!(names(&frame["play"]), played, "{at}: played sounds");

        let modes: Vec<usize> = frame["modes"].as_array().unwrap().iter().map(|m| m.as_u64().unwrap() as usize).collect();
        assert_eq!(modes, gpws.mode_states().to_vec(), "{at}: modes");
        assert_eq!(frame["windshear"].as_u64().unwrap() == 1, gpws.windshear_warning(), "{at}: windshear");
        assert_eq!(frame["minimums_state"].as_u64().unwrap() as u8, gpws.minimums_state(), "{at}: minimums");
    }
    frames.len()
}

#[test]
fn every_golden_scenario_matches_flybywires_javascript() {
    let dir = golden_dir();
    if !dir.exists() {
        eprintln!("no goldens at {}, run oracle.mjs first", dir.display());
        return;
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map_or(false, |x| x == "json"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no scenarios in {}", dir.display());
    let mut total = 0;
    for f in &files {
        let n = replay(f);
        eprintln!("{}: {n} frames match", f.file_name().unwrap().to_string_lossy());
        total += n;
    }
    eprintln!("{} scenarios, {total} frames", files.len());
}

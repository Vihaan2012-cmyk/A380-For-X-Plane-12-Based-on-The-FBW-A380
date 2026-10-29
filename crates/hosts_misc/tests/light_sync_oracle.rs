use hosts_misc::light_sync::{init_lighting, update_lighting, LightSyncConfig, LightSyncInputs};
use hosts_misc::Command;
use serde_json::Value;

fn golden() -> Option<Vec<Value>> {
    let dir = std::env::var("HOSTS_ORACLE_DIR").unwrap_or_else(|_| "E:/fbw-build/hosts-oracle".to_string());
    let text = std::fs::read_to_string(format!("{dir}/light_sync.golden.json")).ok()?;
    serde_json::from_str(&text).ok()
}

fn normalise(commands: &[Command]) -> Vec<(String, String, f64, Option<f64>)> {
    commands
        .iter()
        .map(|c| match c {
            Command::SetLvar { name, value } => ("lvar".to_string(), name.clone(), *value, None),
            Command::KeyEvent { name, value } => ("key".to_string(), name.clone(), *value, None),
            Command::TriggerKey { key, args, .. } => ("trigger".to_string(), key.clone(), args[0], Some(args[1])),
            other => panic!("unexpected {other:?}"),
        })
        .collect()
}

fn expected(case: &Value) -> Vec<(String, String, f64, Option<f64>)> {
    case["out"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let c = c.as_array().unwrap();
            (c[0].as_str().unwrap().to_string(), c[1].as_str().unwrap().to_string(), c[2].as_f64().unwrap(), c[3].as_f64())
        })
        .collect()
}

fn same(a: &[(String, String, f64, Option<f64>)], b: &[(String, String, f64, Option<f64>)]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            x.0 == y.0
                && x.1 == y.1
                && (x.2 - y.2).abs() < 1e-9
                && match (x.3, y.3) {
                    (Some(p), Some(q)) => (p - q).abs() < 1e-9,
                    (None, None) => true,
                    _ => false,
                }
        })
}

#[test]
fn light_sync_matches_the_original_typescript() {
    let Some(cases) = golden() else {
        eprintln!("light_sync.golden.json not found, skipping (run E:/fbw-build/hosts-oracle/light_sync_oracle.mjs)");
        return;
    };
    assert!(cases.len() > 2000);
    let mut checked = 0;
    for case in &cases {
        let n = |k: &str| case[k].as_f64().unwrap();
        let got = match case["kind"].as_str().unwrap() {
            "init" => {
                let i = LightSyncInputs {
                    glasscockpit_automatic_brightness_pct: n("auto"),
                    cabin_circuit_switch_on: n("circuit") != 0.,
                    ..Default::default()
                };
                normalise(&init_lighting(&i, LightSyncConfig::default()))
            }
            _ => {
                let i = LightSyncInputs {
                    cabin_using_autobrightness: n("auto") != 0.,
                    time_of_day: n("tod") as i32,
                    fmgc_flight_phase: n("fmgc") as i32,
                    fwc_flight_phase: n("fwc") as i32,
                    on_any_runway: n("runway") != 0.,
                    plane_altitude_ft: n("alt"),
                    airliner_cruise_altitude: n("cruise"),
                    cabin_manual_brightness: n("manual"),
                    ..Default::default()
                };
                normalise(&update_lighting(&i))
            }
        };
        let want = expected(case);
        assert!(same(&got, &want), "mismatch for {case}\n got  {got:?}\n want {want:?}");
        checked += 1;
    }
    assert_eq!(checked, cases.len());
}

use fms_lnav::*;
use serde_json::Value;

const GOLDEN: &str = r"E:\fbw-build\fms-oracle\uplink-egll-kjfk.json";

fn coord(v: &Value) -> Coord {
    Coord::new(v["lat"].as_f64().unwrap(), v["long"].as_f64().unwrap())
}

fn elements(legs: &[Value]) -> Vec<PlanElement> {
    let mut out = Vec::new();
    let mut prev_fix: Option<Coord> = None;
    let mut arrival = false;
    for l in legs {
        if l["disco"].as_bool() == Some(true) {
            out.push(PlanElement::Discontinuity);
            continue;
        }
        let ident = l["ident"].as_str().unwrap();
        let fix = Coord::new(l["lat"].as_f64().unwrap(), l["lon"].as_f64().unwrap());
        let leg = match l["type"].as_str().unwrap() {
            "IF" => LegInput::initial_fix(ident, fix),
            "TF" => LegInput::track_to_fix(ident, prev_fix.unwrap(), fix),
            other => panic!("unsupported leg type {other}"),
        };
        prev_fix = Some(fix);
        let mut leg = leg;
        arrival |= ident == "GONZZ";
        leg.enroute = false; let _ = arrival;
        out.push(PlanElement::Leg(leg));
    }
    out
}

fn recorded_active(events: &[Value]) -> Vec<Vec<Value>> {
    let mut sets = Vec::new();
    for e in events {
        let Some(json) = e.get("json").and_then(|j| j.as_str()) else { continue };
        if !json.contains("A32NX_EFIS_VECTORS_L_ACTIVE\"") {
            continue;
        }
        let parsed: Value = serde_json::from_str(json).unwrap();
        for item in parsed["data"].as_array().unwrap() {
            if item["topic"] == "A32NX_EFIS_VECTORS_L_ACTIVE" {
                if let Some(a) = item["data"].as_array() {
                    sets.push(a.clone());
                }
            }
        }
    }
    sets
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn uplink_egll_kjfk_active_vectors_match_the_recording() {
    let Ok(text) = std::fs::read_to_string(GOLDEN) else {
        eprintln!("golden missing, skipping");
        return;
    };
    let golden: Value = serde_json::from_str(&text).unwrap();
    let snapshot = &golden["snapshots"][1]["state"]["plans"]["active"];
    let legs = snapshot["legs"].as_array().unwrap();
    let active = snapshot["activeLegIndex"].as_u64().unwrap() as usize;

    let mut geometry = Geometry::new(&elements(legs));
    geometry.recompute(active, Coord::new(51.4775, -0.461388889), 0., 0., 0.);
    let mine = geometry.all_path_vectors(Some(active), false);

    let recorded: Vec<Value> = recorded_active(golden["events"].as_array().unwrap()).into_iter().rfind(|s| !s.is_empty()).expect("no non-empty ACTIVE vectors");
    assert_eq!(mine.len(), recorded.len(), "vector count: mine {:#?}", mine.iter().map(|v| v.to_json()).collect::<Vec<_>>());

    let mut worst: f64 = 0.;
    for (i, (m, r)) in mine.iter().zip(&recorded).enumerate() {
        let ty = r["type"].as_i64().unwrap();
        let (ms, me) = (m.start(), m.end());
        assert_eq!(ty, if matches!(m, PathVector::Line { .. }) { 0 } else { 1 }, "vector {i} type");
        for (a, b) in [(ms, coord(&r["startPoint"])), (me, coord(&r["endPoint"]))] {
            let d = (a.lat - b.lat).abs().max((a.long - b.long).abs());
            worst = worst.max(d);
            assert!(d < 1e-4, "vector {i} point mine {a:?} recorded {b:?}");
        }
        if let PathVector::Arc { centre, sweep, .. } = m {
            let c = coord(&r["centrePoint"]);
            let d = (centre.lat - c.lat).abs().max((centre.long - c.long).abs());
            worst = worst.max(d);
            assert!(d < 1e-4, "vector {i} centre mine {centre:?} recorded {c:?}");
            assert!(close(*sweep, r["sweepAngle"].as_f64().unwrap(), 0.01), "vector {i} sweep {sweep} vs {}", r["sweepAngle"]);
        }
    }
    eprintln!("{} vectors compared, worst coordinate delta {worst:e} deg", mine.len());
}

use serde_json::Value;
use std::path::PathBuf;
use tcas::{Inputs, NdTraffic, RawTraffic, Tcas, Update};

fn golden_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("TCAS_GOLDEN_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("golden")
}

fn num(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap(),
        Value::String(s) => match s.as_str() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            other => panic!("bad number string {other}"),
        },
        Value::Bool(b) => *b as u8 as f64,
        Value::Null => 0.0,
        other => panic!("not a number: {other}"),
    }
}

fn arr3(v: &Value) -> [f64; 3] {
    let a = v.as_array().unwrap();
    [num(&a[0]), num(&a[1]), num(&a[2])]
}

fn inputs(v: &Value) -> Inputs {
    let b = |k: &str| v[k].as_bool().unwrap();
    let r = v["radioAlt"].as_array().unwrap();
    Inputs {
        lat: num(&v["lat"]),
        lon: num(&v["lon"]),
        alt_ft: num(&v["altFt"]),
        vertical_speed: num(&v["verticalSpeed"]),
        alt_above_ground: num(&v["altAboveGround"]),
        ac_ess_powered: b("acEssPowered"),
        ac_2_powered: b("ac2Powered"),
        xpdr_system: num(&v["xpdrSystem"]),
        transponder_state: arr3(&v["transponderState"]),
        air_data_knob: num(&v["airDataKnob"]),
        adr_altitude: arr3(&v["adrAltitude"]),
        radio_alt: [num(&r[0]), num(&r[1])],
        att_hdg_knob: num(&v["attHdgKnob"]),
        ir_true_heading: arr3(&v["irTrueHeading"]),
        slew_active: b("slewActive"),
        gpws_warning: b("gpwsWarning"),
        traffic_left_efis: b("trafficLeftEfis"),
        traffic_right_efis: b("trafficRightEfis"),
        alert_level: num(&v["alertLevel"]) as u8,
        alt_select: num(&v["altSelect"]) as u8,
    }
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    if a.is_nan() || b.is_nan() {
        return a.is_nan() && b.is_nan();
    }
    if a.is_infinite() || b.is_infinite() {
        return a == b;
    }
    // Near-zero closure rates differ by ~2e-9 between glibc and V8 libm.
    (a - b).abs() <= 1e-6 + rel * a.abs().max(b.abs())
}

fn check_list(ctx: &str, got: &Option<Vec<NdTraffic>>, want: &Value) -> Result<(), String> {
    match (got, want) {
        (None, Value::Null) => Ok(()),
        (Some(g), Value::Array(w)) => {
            if g.len() != w.len() {
                return Err(format!("{ctx}: {} contacts, want {}", g.len(), w.len()));
            }
            for (i, (g, w)) in g.iter().zip(w).enumerate() {
                if g.id != w["ID"].as_str().unwrap() {
                    return Err(format!("{ctx}[{i}]: id {} want {}", g.id, w["ID"]));
                }
                if g.intrusion_level as f64 != num(&w["intrusionLevel"]) {
                    return Err(format!("{ctx}[{i}]: intrusion {} want {}", g.intrusion_level, w["intrusionLevel"]));
                }
                if g.relative_alt != num(&w["relativeAlt"]) {
                    return Err(format!("{ctx}[{i}]: relative alt {} want {}", g.relative_alt, w["relativeAlt"]));
                }
                if !close(g.lat, num(&w["lat"]), 1e-12)
                    || !close(g.lon, num(&w["lon"]), 1e-12)
                    || !close(g.vert_speed, num(&w["vertSpeed"]), 1e-6)
                {
                    return Err(format!("{ctx}[{i}]: position or vertical speed differ: {g:?} want {w}"));
                }
            }
            Ok(())
        }
        (g, w) => Err(format!("{ctx}: sent {:?}, want {}", g.as_ref().map(|v| v.len()), w)),
    }
}

fn compare(tick: usize, up: &Update, out: &Value) -> Result<(), String> {
    let v = &out["vars"];
    let vars = &up.vars;
    let exact = [
        ("mode", vars.mode as f64),
        ("state", vars.state as f64),
        ("fault", vars.fault as u8 as f64),
        ("taOnly", vars.ta_only as u8 as f64),
        ("raCorrective", vars.ra_corrective as u8 as f64),
        ("raType", vars.ra_type as f64),
        ("rateToMaintain", vars.rate_to_maintain),
        ("upAdvisory", vars.up_advisory as f64),
        ("downAdvisory", vars.down_advisory as f64),
        ("sensitivity", vars.sensitivity as f64),
        ("vspeedRed", vars.vspeed_red[0]),
        ("vspeedRed", vars.vspeed_red[1]),
        ("vspeedGreen", vars.vspeed_green[0]),
        ("vspeedGreen", vars.vspeed_green[1]),
    ];
    let mut seen_pair = std::collections::HashMap::new();
    for (k, got) in exact {
        let n = seen_pair.entry(k).or_insert(0usize);
        let want = if v[k].is_array() { num(&v[k][*n]) } else { num(&v[k]) };
        *n += 1;
        if got != want {
            return Err(format!("tick {tick}: var {k} = {got}, want {want}"));
        }
    }
    check_list(&format!("tick {tick} traffic L"), &up.traffic_l, &out["trafficL"])?;
    check_list(&format!("tick {tick} traffic R"), &up.traffic_r, &out["trafficR"])?;

    let want_sounds: Vec<(String, Option<bool>)> = out["sounds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s[0].as_str().unwrap().to_string(), s[1].as_bool()))
        .collect();
    let got_sounds: Vec<(String, Option<bool>)> = up.sounds.iter().map(|s| (s.name.to_string(), s.repeat)).collect();
    if want_sounds != got_sounds {
        return Err(format!("tick {tick}: sounds {got_sounds:?}, want {want_sounds:?}"));
    }

    let tracks: Vec<&Value> = out["tracks"].as_array().unwrap().iter().filter(|t| !t.is_null()).collect();
    if tracks.len() != up.tracks.len() {
        return Err(format!("tick {tick}: {} tracks, want {}", up.tracks.len(), tracks.len()));
    }
    for (g, w) in up.tracks.iter().zip(tracks) {
        if g.id != w["id"].as_str().unwrap()
            || g.alive != w["alive"].as_bool().unwrap()
            || g.intrusion_level as f64 != num(&w["intrusion"])
            || g.is_displayed != w["displayed"].as_bool().unwrap()
        {
            return Err(format!("tick {tick}: track {} discrete state {g:?}, want {w}", g.id));
        }
        for (name, got, want) in [
            ("taTau", g.ta_tau, &w["taTau"]),
            ("raTau", g.ra_tau, &w["raTau"]),
            ("vTau", g.v_tau, &w["vTau"]),
            ("closureRate", g.closure_rate, &w["closureRate"]),
            ("slant", g.slant_distance, &w["slant"]),
            ("vertSpeed", g.vert_speed, &w["vertSpeed"]),
            ("groundSpeed", g.ground_speed, &w["groundSpeed"]),
        ] {
            if !close(got, num(want), 1e-6) {
                return Err(format!("tick {tick}: track {} {name} = {got}, want {want}", g.id));
            }
        }
    }
    Ok(())
}

fn run_file(path: &std::path::Path) -> Result<usize, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let sc: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut tcas = Tcas::new();
    let ticks = sc["ticks"].as_array().unwrap();
    for (n, tk) in ticks.iter().enumerate() {
        let traffic: Vec<RawTraffic> = tk["traffic"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| RawTraffic {
                uid: num(&t["uId"]),
                lat: num(&t["lat"]),
                lon: num(&t["lon"]),
                alt: num(&t["alt"]),
                heading: num(&t["heading"]),
            })
            .collect();
        let up = tcas.update(num(&tk["dt"]), &inputs(&tk["in"]), &traffic);
        compare(n, &up, &tk["out"])?;
    }
    Ok(ticks.len())
}

#[test]
fn every_golden_scenario_matches_the_fbw_javascript() {
    let dir = golden_dir();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("golden dir {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
        .collect();
    files.sort();
    assert!(files.len() >= 40, "only {} golden scenarios", files.len());
    let mut failures = Vec::new();
    let mut ticks = 0;
    for f in &files {
        match run_file(f) {
            Ok(n) => ticks += n,
            Err(e) => failures.push(format!("{}: {e}", f.file_name().unwrap().to_string_lossy())),
        }
    }
    assert!(failures.is_empty(), "{} of {} scenarios differ:\n{}", failures.len(), files.len(), failures.join("\n"));
    assert!(ticks > 3000);
}

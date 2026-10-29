use oans::geo::{place_bearing_distance, Coordinates};
use oans::projection::global_to_airport_coordinates;
use serde_json::{json, Value};

pub fn arinc(v: f64) -> f64 {
    (v as f32).to_bits() as f64 + 3.0 * 4294967296.0
}

pub fn arinc_ssm(v: f64, ssm: u32) -> f64 {
    (v as f32).to_bits() as f64 + ssm as f64 * 4294967296.0
}

const ARP: Coordinates = Coordinates { lat: -33.9461, long: 151.1772 };
const THR_34L: Coordinates = Coordinates { lat: -33.9745, long: 151.1790 };
const BRG_34L: f64 = 343.0;
const LEN: f64 = 3962.0;

fn coord(c: Coordinates) -> Value {
    json!({"lat": c.lat, "long": c.long})
}

fn airports() -> Value {
    let opp = place_bearing_distance(THR_34L, BRG_34L, LEN / 1852.0);
    json!({
        "YSSY": {
            "location": coord(ARP),
            "runways": [
                {"ident": "YSSY34L", "thresholdLocation": coord(THR_34L), "bearing": BRG_34L, "length": LEN},
                {"ident": "YSSY16R", "thresholdLocation": coord(opp), "bearing": (BRG_34L + 180.0) % 360.0, "length": LEN}
            ]
        },
        "YSCB": {
            "location": {"lat": -35.3069, "long": 149.195},
            "runways": [{"ident": "YSCB35", "thresholdLocation": {"lat": -35.3, "long": 149.19}, "bearing": 355.0, "length": 3200.0}]
        }
    })
}

fn bus(topic: &str, value: Value) -> Value {
    json!({"op": "bus", "topic": topic, "value": value})
}

fn tick(ms: f64) -> Value {
    json!({"op": "tick", "ms": ms})
}

fn local(c: Coordinates) -> [f64; 2] {
    global_to_airport_coordinates(ARP, c)
}

fn rwy_geometry() -> (Vec<[f64; 2]>, [f64; 2], [f64; 2]) {
    let opp = place_bearing_distance(THR_34L, BRG_34L, LEN / 1852.0);
    let t = local(THR_34L);
    let o = local(opp);
    (vec![t, o], t, o)
}

fn inputs_on_approach(ra: f64, vs: f64, heading: f64, phase: f64, pos: Coordinates) -> Vec<Value> {
    let mut v = Vec::new();
    for i in 1..=3 {
        v.push(bus(&format!("ra_radio_altitude_{i}"), json!(arinc(ra))));
        v.push(bus(&format!("ir_vertical_speed_{i}"), json!(arinc(vs))));
        v.push(bus(&format!("ir_true_heading_{i}"), json!(arinc(heading))));
        v.push(bus(&format!("ir_latitude_{i}"), json!(arinc(pos.lat))));
        v.push(bus(&format!("ir_longitude_{i}"), json!(arinc(pos.long))));
    }
    v.push(bus("a380x_btv_fws_flight_phase", json!(phase)));
    v
}

fn exit_line() -> Vec<[f64; 2]> {
    let (_, t, o) = rwy_geometry();
    let len = (o[0] - t[0]).hypot(o[1] - t[1]);
    let u = [(o[0] - t[0]) / len, (o[1] - t[1]) / len];
    let n = [-u[1], u[0]];
    let at = |along: f64, lat: f64| [t[0] + u[0] * along + n[0] * lat, t[1] + u[1] * along + n[1] * lat];
    vec![at(1800.0, 4.0), at(1830.0, 14.0), at(1860.0, 32.0), at(1880.0, 60.0)]
}

pub fn all() -> Vec<Value> {
    let (center, thr, _opp) = rwy_geometry();
    let mut out = Vec::new();

    out.push(json!({
        "name": "airport_loaded",
        "arp": coord(ARP),
        "airports": airports(),
        "steps": [
            bus("fmsLandingRunway", json!("YSSY34L")),
            bus("fmsLandingRunway", json!("YSSY16R")),
            bus("fmsLandingRunway", json!("YSXX99")),
            bus("fmsLandingRunway", Value::Null)
        ]
    }));

    out.push(json!({
        "name": "runway_selected",
        "arp": coord(ARP),
        "airports": airports(),
        "steps": [
            {"op": "select_runway", "runway": "YSSY34L", "centerline": center, "threshold": thr, "brngmag": 343.0},
            {"op": "select_runway", "runway": "YSSY16R", "centerline": center, "threshold": local(place_bearing_distance(THR_34L, BRG_34L, LEN / 1852.0)), "brngmag": 163.0},
            bus("oans_remove_btv_data", json!(true))
        ]
    }));

    out.push(json!({
        "name": "exit_selected",
        "arp": coord(ARP),
        "airports": airports(),
        "steps": [
            {"op": "select_runway", "runway": "YSSY34L", "centerline": center, "threshold": thr, "brngmag": 343.0},
            {"op": "select_exit", "exit": "A4", "line": exit_line()},
            {"op": "select_exit", "exit": "BAD", "line": [[thr[0] + 300.0, thr[1] + 10.0], [thr[0] + 330.0, thr[1] + 40.0]]},
            bus("oansManualStoppingDistance", json!(2100.0)),
            bus("oansManualStoppingDistance", json!(100.0))
        ]
    }));

    let mut rolling: Vec<Value> = vec![bus("fmsLandingRunway", json!("YSSY34L"))];
    for i in 1..=2 {
        rolling.push(bus(&format!("lgciu_discrete_word_2_{i}"), json!(arinc(0.0))));
    }
    rolling.push(json!({"op": "select_runway", "runway": "YSSY34L", "centerline": center, "threshold": thr, "brngmag": 343.0}));
    rolling.push(json!({"op": "select_exit", "exit": "A4", "line": exit_line()}));
    let heading = BRG_34L;
    for (dist_before_m, ra) in [(4000.0, 650.0), (2500.0, 450.0), (1200.0, 220.0), (300.0, 60.0)] {
        let p = place_bearing_distance(THR_34L, (BRG_34L + 180.0) % 360.0, dist_before_m / 1852.0);
        rolling.extend(inputs_on_approach(ra, -700.0, heading, 10.0, p));
        rolling.push(tick(200.0));
    }
    for along in [100.0, 600.0, 1500.0, 2400.0, 3300.0] {
        let p = place_bearing_distance(THR_34L, BRG_34L, along / 1852.0);
        rolling.extend(inputs_on_approach(0.0, 0.0, heading, 10.0, p));
        rolling.push(bus("lgciu_discrete_word_2_1", json!(arinc(1024.0))));
        rolling.push(bus("a380x_btv_auto_brake_mode", json!(1)));
        rolling.push(bus("a380x_btv_auto_brake_active", json!(true)));
        rolling.push(tick(200.0));
    }
    rolling.push(bus("a380x_btv_auto_brake_active", json!(false)));
    rolling.push(tick(200.0));
    out.push(json!({"name": "aircraft_rolling", "arp": coord(ARP), "airports": airports(), "steps": rolling}));

    let mut rops: Vec<Value> = Vec::new();
    for i in 1..=2 {
        rops.push(bus(&format!("lgciu_discrete_word_2_{i}"), json!(arinc(0.0))));
    }
    for (dist_before_m, ra) in [(2800.0, 480.0), (2400.0, 420.0), (1500.0, 280.0)] {
        let p = place_bearing_distance(THR_34L, (BRG_34L + 180.0) % 360.0, dist_before_m / 1852.0);
        rops.extend(inputs_on_approach(ra, -700.0, heading, 10.0, p));
        rops.push(tick(200.0));
        rops.push(tick(200.0));
    }
    rops.push(bus("oansSelectedLandingRunway", json!("YSSY16R")));
    let p = place_bearing_distance(THR_34L, (BRG_34L + 180.0) % 360.0, 1800.0 / 1852.0);
    rops.extend(inputs_on_approach(340.0, -700.0, heading, 10.0, p));
    rops.push(tick(200.0));
    out.push(json!({"name": "rops_detection", "arp": coord(ARP), "airports": airports(), "steps": rops}));

    let mut ra_modes: Vec<Value> = vec![bus("fmsLandingRunway", json!("YSSY34L"))];
    let p = place_bearing_distance(THR_34L, (BRG_34L + 180.0) % 360.0, 1000.0 / 1852.0);
    ra_modes.extend(inputs_on_approach(300.0, -700.0, heading, 10.0, p));
    ra_modes.push(bus("ra_radio_altitude_2", json!(arinc_ssm(0.0, 1))));
    ra_modes.push(tick(200.0));
    ra_modes.push(bus("ra_radio_altitude_3", json!(arinc_ssm(0.0, 0))));
    ra_modes.push(tick(200.0));
    ra_modes.push(bus("ra_radio_altitude_1", json!(arinc(480.0))));
    ra_modes.push(bus("ra_radio_altitude_2", json!(arinc(520.0))));
    ra_modes.push(bus("ra_radio_altitude_3", json!(arinc(500.0))));
    ra_modes.push(tick(200.0));
    out.push(json!({"name": "radio_altitude_selection", "arp": coord(ARP), "airports": airports(), "steps": ra_modes}));

    out.push(json!({"name": "rwy_ahead", "arp": coord(ARP), "airports": airports(), "runway_areas": runway_areas(), "steps": rwy_ahead_steps()}));
    out.push(json!({"name": "post_landing_exit", "arp": coord(ARP), "airports": airports(), "steps": post_landing_steps()}));
    out.push(json!({"name": "markers", "arp": coord(ARP), "airports": airports(), "features": marker_features(), "steps": marker_steps()}));

    out
}

fn runway_frame() -> ([f64; 2], [f64; 2], [f64; 2], f64) {
    let (_, t, o) = rwy_geometry();
    let len = (o[0] - t[0]).hypot(o[1] - t[1]);
    let u = [(o[0] - t[0]) / len, (o[1] - t[1]) / len];
    let n = [-u[1], u[0]];
    (t, u, n, len)
}

fn runway_areas() -> Value {
    let (t, u, n, len) = runway_frame();
    let at = |along: f64, lat: f64| [t[0] + u[0] * along + n[0] * lat, t[1] + u[1] * along + n[1] * lat];
    let ring = vec![at(0.0, -30.0), at(len, -30.0), at(len, 30.0), at(0.0, 30.0), at(0.0, -30.0)];
    let far = vec![at(0.0, 400.0), at(len, 400.0), at(len, 460.0), at(0.0, 460.0), at(0.0, 400.0)];
    json!([
        {"idrwy": "07.25", "rings": [far]},
        {"idrwy": "16R.34L", "rings": [ring]},
        {"idrwy": null, "rings": [[at(0.0, -5.0), at(10.0, -5.0), at(10.0, 5.0), at(0.0, -5.0)]]}
    ])
}

fn taxi_position(along_m: f64, lateral_m: f64) -> Coordinates {
    let on_cl = place_bearing_distance(THR_34L, BRG_34L, along_m / 1852.0);
    if lateral_m >= 0.0 {
        place_bearing_distance(on_cl, (BRG_34L + 270.0) % 360.0, lateral_m / 1852.0)
    } else {
        place_bearing_distance(on_cl, (BRG_34L + 90.0) % 360.0, -lateral_m / 1852.0)
    }
}

fn rwy_ahead_step(lateral_m: f64, gs: f64, now: f64, repeat: u32, heading: f64) -> Vec<Value> {
    let p = taxi_position(1000.0, lateral_m);
    vec![
        bus("groundSpeed", json!(arinc(gs))),
        json!({"op": "rwy_ahead", "lat": p.lat, "long": p.long, "bearing": heading, "now": now, "repeat": repeat, "ppos_local": local(p)}),
    ]
}

fn rwy_ahead_steps() -> Vec<Value> {
    let toward = (BRG_34L + 90.0) % 360.0;
    let mut v = vec![bus("lgciu_discrete_word_2_1", json!(arinc(1024.0))), bus("lgciu_discrete_word_2_2", json!(arinc(1024.0)))];
    v.extend(rwy_ahead_step(200.0, 20.0, 1_000.0, 10, toward));
    v.extend(rwy_ahead_step(120.0, 20.0, 2_000.0, 10, toward));
    v.extend(rwy_ahead_step(120.0, 20.0, 3_000.0, 3, toward));
    v.extend(rwy_ahead_step(100.0, 20.0, 20_000.0, 7, toward));
    v.extend(rwy_ahead_step(90.0, 20.0, 40_000.0, 10, toward));
    v.extend(rwy_ahead_step(90.0, 20.0, 41_000.0, 10, toward));
    v.extend(rwy_ahead_step(0.0, 20.0, 42_000.0, 10, toward));
    v.extend(rwy_ahead_step(-120.0, 20.0, 43_000.0, 10, (BRG_34L + 270.0) % 360.0));
    v.extend(rwy_ahead_step(120.0, 20.0, 44_000.0, 10, (BRG_34L + 270.0) % 360.0));
    v.extend(rwy_ahead_step(120.0, 50.0, 45_000.0, 10, toward));
    v.extend(rwy_ahead_step(120.0, 0.5, 46_000.0, 10, toward));
    v.push(bus("groundSpeed", json!(arinc_ssm(20.0, 1))));
    v.extend(rwy_ahead_step(120.0, 20.0, 47_000.0, 10, toward).into_iter().skip(1));
    v.push(bus("lgciu_discrete_word_2_1", json!(arinc(0.0))));
    v.extend(rwy_ahead_step(120.0, 20.0, 48_000.0, 10, toward));
    v.push(bus("lgciu_discrete_word_2_1", json!(arinc_ssm(0.0, 0))));
    v.extend(rwy_ahead_step(120.0, 20.0, 49_000.0, 10, toward));
    v
}

fn post_landing_steps() -> Vec<Value> {
    let (center, thr, _) = rwy_geometry();
    let heading = BRG_34L;
    let mut v: Vec<Value> = vec![bus("fmsLandingRunway", json!("YSSY34L"))];
    for i in 1..=2 {
        v.push(bus(&format!("lgciu_discrete_word_2_{i}"), json!(arinc(0.0))));
    }
    v.push(json!({"op": "ac_on_ground", "value": false}));
    v.push(json!({"op": "select_runway", "runway": "YSSY34L", "centerline": center, "threshold": thr, "brngmag": 343.0}));
    v.push(json!({"op": "select_exit", "exit": "A4", "line": exit_line()}));
    v.push(bus("a380x_btv_dry_stopping_distance", json!(1500.0)));
    v.push(bus("a380x_btv_wet_stopping_distance", json!(1900.0)));
    for (along, ra, dry, wet) in [(-3000.0, 900.0, 1500.0, 1900.0), (-1500.0, 450.0, 1500.0, 3300.0), (200.0, 40.0, 3000.0, 3200.0), (700.0, 5.0, 3700.0, 4200.0), (900.0, 0.0, 2500.0, 3000.0)] {
        let p = place_bearing_distance(THR_34L, BRG_34L, along / 1852.0);
        v.extend(inputs_on_approach(ra, -700.0, heading, 10.0, p));
        v.push(json!({"op": "ppos_local", "pos": local(p)}));
        v.push(bus("a380x_btv_dry_stopping_distance", json!(dry)));
        v.push(bus("a380x_btv_wet_stopping_distance", json!(wet)));
        v.push(tick(200.0));
    }
    v.push(bus("lgciu_discrete_word_2_1", json!(arinc(1024.0))));
    v.push(json!({"op": "ac_on_ground", "value": true}));
    for (along, gs, stop) in [(1000.0, 120.0, 1400.0), (1600.0, 80.0, 900.0), (2200.0, 60.0, 2500.0), (2600.0, 40.0, 1300.0), (3000.0, 20.0, 400.0)] {
        let p = place_bearing_distance(THR_34L, BRG_34L, along / 1852.0);
        v.extend(inputs_on_approach(0.0, 0.0, heading, 10.0, p));
        v.push(json!({"op": "ppos_local", "pos": local(p)}));
        v.push(bus("groundSpeed", json!(arinc(gs))));
        v.push(bus("a380x_btv_stop_bar_distance", json!(stop)));
        v.push(tick(200.0));
    }
    v
}

fn marker_features() -> Value {
    let tw = |id: i64, idlin: &str, coords: Value| json!({"type": "Feature", "properties": {"id": id, "feattype": 16, "idlin": idlin}, "geometry": {"type": "LineString", "coordinates": coords}});
    json!([
        tw(1, "A", json!([[100.0, 20.0], [100.0, 120.0], [180.0, 300.0]])),
        tw(2, "A", json!([[180.0, 300.0], [200.0, 320.0]])),
        tw(3, "B", json!([[-400.0, 10.0], [-380.0, 15.0]])),
        json!({"type": "Feature", "properties": {"id": 4, "feattype": 19, "idlin": "A"}, "geometry": {"type": "LineString", "coordinates": [[0.0, 0.0], [30.0, 40.0]]}}),
        json!({"type": "Feature", "properties": {"id": 5, "feattype": 2, "idthr": "34L"}, "geometry": {"type": "Point", "coordinates": [12.5, -40.0]}}),
        json!({"type": "Feature", "properties": {"id": 6, "feattype": 23, "idstd": "51"}, "geometry": {"type": "Point", "coordinates": [300.0, 300.0]}})
    ])
}

fn marker(f: &str, args: Value) -> Value {
    json!({"op": "marker", "fn": f, "args": args})
}

fn marker_steps() -> Vec<Value> {
    vec![
        marker("addCrossAtFeature", json!([1, 16])),
        marker("addFlagAtFeature", json!([5, 2])),
        marker("addFlagAtFeature", json!([6, 23])),
        marker("addCrossAtFeature", json!([3, 16])),
        marker("addCross", json!([[50.0, 50.0]])),
        marker("removeCrossAtFeature", json!([1, 16])),
        marker("removeFlag", json!([0])),
        marker("removeFlag", json!([9])),
        marker("addCrossAtFeature", json!([5, 2])),
        marker("eraseAllCrosses", json!([])),
        marker("eraseAllFlags", json!([])),
    ]
}

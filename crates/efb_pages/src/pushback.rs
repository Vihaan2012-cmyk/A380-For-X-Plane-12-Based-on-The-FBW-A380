use crate::js::{el, num, round};
use crate::jv::{fx, n, Vars};
use crate::{label, span, Node};
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct PbPlan {
    pub dir: String,
    pub angle: f64,
    pub after: f64,
    pub pull: f64,
    pub pace: String,
}

impl Default for PbPlan {
    fn default() -> Self {
        Self { dir: "LEFT".into(), angle: 90.0, after: 15.0, pull: 0.0, pace: "NORMAL".into() }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct GcLine {
    pub who: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct PbView {
    pub vars: Vars,
    pub plan: PbPlan,
    /// Running automatic pushback: (step index, step count).
    pub auto: Option<(usize, usize)>,
    pub speed: f64,
    pub heading: f64,
    pub map_icao: String,
    pub map_layers: Option<Value>,
    pub map_state: String,
    pub gc_present: bool,
    pub gc_coming: bool,
    pub gc_phase: String,
    pub gc_log: Vec<GcLine>,
}

fn num_of(v: &Vars, k: &str) -> f64 {
    match v.get(k) {
        Some(Value::Number(x)) => x.as_f64().unwrap_or(0.0),
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn map_layer(layers: &Value, lat0: f64, lon0: f64, hdg: f64, sx: f64, sy: f64, scale: f64) -> String {
    let k = (lat0 * std::f64::consts::PI / 180.0).cos();
    let (c, sn) = ((hdg * std::f64::consts::PI / 180.0).cos(), (hdg * std::f64::consts::PI / 180.0).sin());
    let pt = |p: &Value| -> String {
        let (lon, lat) = (p[0].as_f64().unwrap_or(0.0), p[1].as_f64().unwrap_or(0.0));
        let e = (lon - lon0) * 111320.0 * k;
        let nn = (lat - lat0) * 110540.0;
        let x = e * c - nn * sn;
        let y = e * sn + nn * c;
        format!("{},{}", fx(sx + x * scale, 1), fx(sy - y * scale, 1))
    };
    let near = |ring: &[Value]| ring.iter().any(|p| ((p[1].as_f64().unwrap_or(0.0) - lat0) * 110540.0).abs() < 400.0 && ((p[0].as_f64().unwrap_or(0.0) - lon0) * 111320.0 * k).abs() < 400.0);
    let feats = |name: &str| layers.get(name).and_then(|l| l.get("features")).and_then(Value::as_array).cloned().unwrap_or_default();
    let mut out = String::new();
    let mut polys = |name: &str, fill: &str, stroke: &str, sw: &str| {
        for f in feats(name) {
            let g = &f["geometry"];
            let rings: Vec<Value> = match g["type"].as_str() {
                Some("Polygon") => vec![g["coordinates"].clone()],
                Some("MultiPolygon") => g["coordinates"].as_array().cloned().unwrap_or_default(),
                _ => vec![],
            };
            for poly in rings {
                let Some(rs) = poly.as_array() else { continue };
                let Some(r0) = rs.first().and_then(Value::as_array) else { continue };
                if !near(r0) {
                    continue;
                }
                let d: String = rs.iter().map(|r| format!("M{}Z", r.as_array().map(|r| r.iter().map(pt).collect::<Vec<_>>().join("L")).unwrap_or_default())).collect();
                out += &format!("<path d=\"{d}\" fill=\"{fill}\" fill-rule=\"evenodd\" stroke=\"{stroke}\" stroke-width=\"{sw}\"/>");
            }
        }
    };
    polys("apronelement", "#2c2f34", "none", "0");
    polys("taxiwayelement", "#383b41", "none", "0");
    polys("runwayelement", "#45484e", "none", "0");
    polys("parkingstandarea", "none", "#6d7178", "1");
    polys("verticalpolygonalstructure", "#4d5158", "#62666d", "1");
    let mut lines = |name: &str, stroke: &str, sw: &str, dash: &str| {
        for f in feats(name) {
            let g = &f["geometry"];
            let ls: Vec<Value> = match g["type"].as_str() {
                Some("LineString") => vec![g["coordinates"].clone()],
                Some("MultiLineString") => g["coordinates"].as_array().cloned().unwrap_or_default(),
                _ => vec![],
            };
            for l in ls {
                let Some(l) = l.as_array() else { continue };
                if l.len() > 1 && near(l) {
                    out += &format!(
                        "<polyline points=\"{}\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"{sw}\"{}/>",
                        l.iter().map(pt).collect::<Vec<_>>().join(" "),
                        if dash.is_empty() { String::new() } else { format!(" stroke-dasharray=\"{dash}\"") }
                    );
                }
            }
        }
    };
    lines("taxiwayguidanceline", "#d9b23a", "1.4", "");
    lines("standguidanceline", "#d9b23a", "1", "4 3");
    for f in feats("parkingstandlocation") {
        let g = &f["geometry"];
        if g["type"].as_str() != Some("Point") || !near(std::slice::from_ref(&g["coordinates"])) {
            continue;
        }
        let p = pt(&g["coordinates"]);
        let (x, y) = p.split_once(',').unwrap();
        let props = &f["properties"];
        let id = ["idstd", "ident"].iter().find_map(|k| props.get(*k).filter(|v| crate::jv::truthy(v)).map(crate::jv::jstring)).unwrap_or_default();
        if !id.is_empty() {
            out += &format!(
                "<text x=\"{x}\" y=\"{y}\" fill=\"#c7cad0\" font-size=\"10\" font-family=\"Manrope, Segoe UI, sans-serif\" text-anchor=\"middle\">{}</text>",
                id.replace(['<', '&'], "")
            );
        }
    }
    out
}

/// `pbPathSvg`: the planned push drawn over the airport around the aircraft.
pub fn path_svg(v: &PbView) -> String {
    let (w, h, cx, cy, scale) = (420.0, 420.0, 210.0, 130.0, 1.6);
    let p = &v.plan;
    let mut pts = vec![[cx, cy]];
    let (mut x, mut y, mut hh) = (cx, cy, 0.0f64);
    let mut go = |d: f64, hh: f64, pts: &mut Vec<[f64; 2]>| {
        x -= hh.sin() * d * scale;
        y += hh.cos() * d * scale;
        pts.push([x, y]);
    };
    if p.dir == "STRAIGHT" {
        go(p.after.max(10.0), hh, &mut pts);
    } else {
        go(12.0, hh, &mut pts);
        let sign = if p.dir == "LEFT" { 1.0 } else { -1.0 };
        let (cnt, radius) = (24.0, 32.0);
        let arc = radius * p.angle * std::f64::consts::PI / 180.0;
        for _ in 0..24 {
            hh += (sign * p.angle * std::f64::consts::PI / 180.0) / cnt;
            go(arc / cnt, hh, &mut pts);
        }
        if p.after > 0.0 {
            go(p.after, hh, &mut pts);
        }
    }
    let path = pts.iter().map(|q| format!("{},{}", fx(q[0], 1), fx(q[1], 1))).collect::<Vec<_>>().join(" ");
    let end = pts[pts.len() - 1];
    let k = (72.7 * scale) / 56.0;
    let plane = |ang: f64, ex: f64, ey: f64, op: &str| {
        format!(
            "<g transform=\"translate({},{}) rotate({}) scale({})\" opacity=\"{op}\"><path d=\"M0,-26 L4,-21 L4,-6 L28,8 L28,12 L4,6 L3,22 L10,27 L10,29 L0,27 L-10,29 L-10,27 L-3,22 L-4,6 L-28,12 L-28,8 L-4,-6 L-4,-21 Z\" fill=\"#3a3c40\" stroke=\"#eef0f2\" stroke-width=\"{}\"/></g>",
            n(ex),
            n(ey),
            n(ang),
            fx(k, 3),
            fx(1.2 / k, 2)
        )
    };
    let end_ang = if p.dir == "STRAIGHT" { 0.0 } else { (if p.dir == "LEFT" { 1.0 } else { -1.0 }) * p.angle };
    let map = v.map_layers.as_ref().map(|l| map_layer(l, num_of(&v.vars, "PLANE LATITUDE"), num_of(&v.vars, "PLANE LONGITUDE"), num_of(&v.vars, "PLANE HEADING DEGREES TRUE"), cx, cy, scale)).unwrap_or_default();
    let lbl = if v.map_layers.is_some() {
        v.map_icao.clone()
    } else if v.map_state == "loading" {
        "LOADING AIRPORT...".into()
    } else {
        "NO AIRPORT DATA".into()
    };
    format!(
        "<svg viewBox=\"0 0 {w} {h}\" class=\"pb-path\"><defs><clipPath id=\"pbclip\"><rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" rx=\"12\"/></clipPath></defs><g clip-path=\"url(#pbclip)\"><rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" fill=\"#1f2124\"/>{map}<polyline points=\"{path}\" fill=\"none\" stroke=\"#ff4fd8\" stroke-width=\"3\" stroke-dasharray=\"7 5\"/>{}{}<text x=\"12\" y=\"{}\" fill=\"#9da3aa\" font-size=\"11\" font-weight=\"700\" font-family=\"Manrope, Segoe UI, sans-serif\">{lbl}  \u{2022}  50 m</text><line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#9da3aa\" stroke-width=\"2\"/></g></svg>",
        plane(end_ang, end[0], end[1], "0.55"),
        plane(0.0, cx, cy, "1"),
        n(h - 12.0),
        n(w - 12.0 - 50.0 * scale),
        n(h - 16.0),
        n(w - 12.0),
        n(h - 16.0)
    )
}

fn opt(v: &PbView, key: &str, opts: &[&str], lbl: &str) -> Node {
    let cur = match key {
        "dir" => v.plan.dir.clone(),
        "pace" => v.plan.pace.clone(),
        "angle" => num(v.plan.angle),
        "after" => num(v.plan.after),
        _ => num(v.plan.pull),
    };
    let mut row = el("div", "pc-seg");
    for o in opts {
        let numeric = o.parse::<f64>().is_ok();
        let text = if !numeric {
            o.to_string()
        } else if key == "angle" {
            format!("{o}\u{b0}")
        } else {
            format!("{o} m")
        };
        row = row.child(el("button", "pc-opt").class_if("on", cur == *o).attr("type", "button").act(&format!("pb:{key}:{o}")).text(&text));
    }
    el("div", "pc-f").child(span("pc-l", lbl)).child(row)
}

pub fn pb_top(v: &PbView) -> Node {
    let attached = num_of(&v.vars, "PUSHBACK ATTACHED") != 0.0;
    let brake = num_of(&v.vars, "A32NX_PARK_BRAKE_LEVER_POS") >= 0.5;
    let chip = |k: &str, val: &str, cls: &str| el("div", "pb-chip").child(span("pb-ck", k)).child(Node::el("span", &format!("pb-cv {cls}")).text(val));
    let hdg = round(num_of(&v.vars, "PLANE HEADING DEGREES TRUE"));
    let mode = match v.auto {
        Some((i, cnt)) => format!("AUTO {}/{cnt}", i + 1),
        None if v.speed != 0.0 => "MANUAL".into(),
        None => "IDLE".into(),
    };
    let status = el("div", "pb-status")
        .child(chip("TUG", if attached { "CONNECTED" } else { "NOT CONNECTED" }, if attached { "pc-good" } else { "" }))
        .child(chip("PARK BRK", if brake { "SET" } else { "RELEASED" }, if brake { "pc-bad" } else { "pc-good" }))
        .child(chip("GS", &format!("{} kt", fx(num_of(&v.vars, "GROUND VELOCITY").abs(), 1)), ""))
        .child(chip("HDG", &format!("{:0>3}\u{b0}", num(hdg)), ""))
        .child(chip("MODE", &mode, if v.auto.is_some() { "pc-good" } else { "" }));
    el("div", "pb-top").child(status).child(
        el("div", "pb-btns")
            .child(el("button", "pc-btn").class_if("pc-go", !attached).attr("type", "button").act("pb:tug").text(if attached { "RELEASE TUG" } else { "CALL TUG" }))
            .child(el("button", "pc-btn pb-stop").attr("type", "button").act("pb:stop").text("STOP")),
    )
}

fn gc_card(v: &PbView) -> Node {
    let (st_text, st_cls) = if v.gc_present {
        ("ON HEADSET", "on")
    } else if v.gc_coming {
        ("CALLING\u{2026}", "wait")
    } else {
        ("NOT CONNECTED", "off")
    };
    let btn = |what: &str, l: &str, wide: bool| el("button", "pc-btn").class_if("pc-go", wide).attr("type", "button").attr_if("disabled", what != "call" && !v.gc_present).act(&format!("gc:{what}")).text(l);
    let phase = match v.gc_phase.as_str() {
        "await_tug" => "Waiting for the tug",
        "await_brake" => "Waiting for the parking brake release",
        "pushing" => "Pushing back",
        "push_done" => "Pushback complete, set the parking brake",
        "push_stopped" => "Stopped, set the parking brake to disconnect",
        "releasing" => "Disconnecting the tug",
        "bye" => "Tug disconnected",
        _ => "",
    };
    let log = el("div", "gc-log").children(v.gc_log.iter().map(|l| {
        el("div", "gc-line")
            .class_if("gnd", l.who == "GND")
            .class_if("capt", l.who == "CAPT")
            .child(span("gc-who", if l.who.is_empty() { "\u{b7}" } else { &l.who }))
            .child(Node::new("span").text(&l.text))
    }));
    let mut card = el("div", "pc-card").id("gc-card").child(el("div", "gc-head").child(label("pc-h", "INTERPHONE \u{b7} GROUND ENGINEER")).child(span(&format!("gc-state {st_cls}"), st_text)));
    if !phase.is_empty() {
        card = card.child(label("gc-phase", phase));
    }
    card.child(
        el("div", "gc-btns")
            .child(btn("call", "GROUND, COCKPIT", true))
            .child(btn("push", "READY FOR PUSHBACK", false))
            .child(btn("start", "REQUEST ENGINE START", false))
            .child(btn("stop", "STOP PUSHBACK", false))
            .child(btn("disconnect", "DISCONNECT TUG", false))
            .child(btn("chocks", "CHOCKS", false))
            .child(btn("covers", "REMOVE COVERS + PINS", false))
            .child(btn("gpu", "GROUND POWER", false))
            .child(btn("asu", "AIR START UNIT", false))
            .child(btn("pca", "PRECONDITIONED AIR", false)),
    )
    .child(log)
    .child(label("pb-note", "The overhead CALLS MECH button calls the ground engineer too, and every request is an X-Plane command (fbw/interphone/...) you can bind to a key."))
}

pub fn pushback_content(v: &PbView) -> Node {
    let attached = num_of(&v.vars, "PUSHBACK ATTACHED") != 0.0;
    let brake = num_of(&v.vars, "A32NX_PARK_BRAKE_LEVER_POS") >= 0.5;
    let knob = el("div", "pb-knob").style(&format!("left: {}%; top: {}%;", crate::jv::css_num(50.0 + v.heading * 42.0), crate::jv::css_num(50.0 - v.speed * 42.0)));
    let pad = el("div", "pb-pad")
        .act("pb:pad")
        .child(span("pb-pl t", "FWD"))
        .child(span("pb-pl b", "BACK"))
        .child(span("pb-pl l", "NOSE L"))
        .child(span("pb-pl r", "NOSE R"))
        .child(knob);
    let note = if brake && attached { "Release the parking brake to move." } else { "Drag to push or pull and steer; let go to stop." };
    let manual = el("div", "pc-card").child(label("pc-h", "MANUAL")).child(pad).child(label("pb-note", note).id("pb-note"));
    let go_text = if v.auto.is_some() {
        "PUSHING..."
    } else if !attached {
        "CALL THE TUG FIRST"
    } else if brake {
        "RELEASE PARK BRAKE"
    } else {
        "START PUSHBACK"
    };
    let auto = el("div", "pc-card")
        .child(label("pc-h", "AUTOMATIC PUSHBACK"))
        .child(
            el("div", "pb-auto")
                .child(
                    Node::new("div")
                        .child(opt(v, "dir", &["LEFT", "STRAIGHT", "RIGHT"], "Tail"))
                        .child(opt(v, "angle", &["45", "90", "135", "180"], "Turn"))
                        .child(opt(v, "after", &["0", "15", "30", "60"], "Push after the turn"))
                        .child(opt(v, "pull", &["0", "10", "20", "40"], "Pull forward"))
                        .child(opt(v, "pace", &["SLOW", "NORMAL", "FAST"], "Speed")),
                )
                .child(crate::js::svg_div("pb-preview", &path_svg(v))),
        )
        .child(el("div", "pc-actions").child(el("button", "pc-btn pc-go").attr("type", "button").id("pb-go").attr_if("disabled", !attached || brake || v.auto.is_some()).act("pb:go").text(go_text)));
    Node::new("div").id("pb-content").child(pb_top(v)).child(el("div", "pb-grid").child(manual).child(auto)).child(gc_card(v))
}

use crate::js::{el, num};
use crate::jv::{a, b, f, format_value, g, n, s, sv, var, Svg, Vars};
use crate::{button, label, span, Node};
use serde_json::Value;
use std::collections::HashSet;

pub const ECAM_GREEN: &str = "#00ff00";
pub const ECAM_AMBER: &str = "#e68000";
pub const ECAM_WHITE: &str = "#ffffff";
pub const ECAM_CYAN: &str = "#00ffff";
pub const ECAM_RED: &str = "#ff0000";
pub const ECAM_GREY: &str = "#7c8291";

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct StudyView {
    pub selected: usize,
    pub physics: bool,
    pub preview: bool,
    /// `window.studyGroup`; `None` shows every menu group.
    pub group: Option<String>,
    /// `#study-content`'s client width (0 = the design width).
    pub width: f64,
    pub offline: bool,
}

const DEEP_MAP: [(&str, &str); 18] = [
    ("Deep: APU", "APU"), ("Deep: Electrical", "Electrical Network"), ("Deep: Wiring", "Electrical Network"), ("Deep: Hydraulics", "Hydraulic Network"),
    ("Deep: Fuel", "Fuel System"), ("Deep: Flight Controls", "Flight Controls"), ("Deep: Auto Flight", "Flight Controls"), ("Deep: Fire & Ice", "Fire Protection"),
    ("Deep: Pneumatic Ducts", "Bleed System"), ("Deep: Environment", "Air Conditioning"), ("Deep: Thermal Zones", "Air Conditioning"), ("Deep: Oxygen", "Cabin Pressurisation"),
    ("Deep: Cabin", "Cabin Pressurisation"), ("Deep: Gear & Structure", "Landing Gear and Brakes"), ("Deep: Avionics Network", "Air Data and Inertial"),
    ("Deep: Sensors", "Air Data and Inertial"), ("Deep: Communications", "Radios"), ("Deep: Engine Accessories", "Engine 1 State"),
];

/// `studyOnShow`'s merge of the deep area pages into their parent pages (`deepPages`, `merged`).
pub fn merge_deep(pages: &mut [Value]) {
    for i in 0..pages.len() {
        let title = s(&pages[i], "title").to_string();
        let Some((_, parent)) = DEEP_MAP.iter().find(|(k, _)| *k == title) else { continue };
        let Some(p) = pages.iter().rposition(|p| s(p, "title") == *parent) else { continue };
        let child = pages[i].clone();
        if let Some(o) = pages[p].as_object_mut() {
            let list = o.entry("deepPages").or_insert_with(|| Value::Array(vec![]));
            if let Some(l) = list.as_array_mut() {
                l.push(child);
            }
        }
        if let Some(o) = pages[i].as_object_mut() {
            o.insert("merged".into(), Value::Bool(true));
        }
    }
}

fn h2(text: &str, size: u32) -> Node {
    Node::new("h2").style(&format!("font-size:{size}px;margin:0 0 8px")).text(text)
}

fn has(vars: &Vars, name: &str) -> bool {
    vars.contains_key(name)
}

fn nonzero(vars: &Vars, name: &str) -> bool {
    match vars.get(name) {
        None => false,
        Some(Value::Number(x)) => x.as_f64() != Some(0.0),
        Some(_) => true,
    }
}

fn dec(fl: &Value) -> usize {
    let d = f(fl, "decimals");
    if d.is_finite() && d > 0.0 { d as usize } else { 0 }
}

fn fmt_field(vars: &Vars, fl: &Value) -> String {
    format_value(var(vars, s(fl, "name")), s(fl, "kind"), dec(fl), s(fl, "unit"))
}

fn dot(on: bool) -> Node {
    el("span", "dot").class_if("ok", on)
}

pub fn field_row(fl: &Value, vars: &Vars, preview: bool) -> (Node, bool) {
    let name = s(fl, "name");
    let h = has(vars, name);
    let lbl = if b(fl, "label") { s(fl, "label") } else { name };
    let row = el("div", "study-field-row").child(span("fname", lbl));
    if !h {
        return (row.child(span("fval missing", if preview { "--" } else { "not modelled" })), false);
    }
    let kind = s(fl, "kind");
    if kind == "lamp" || kind == "flag" {
        let on = nonzero(vars, name);
        let t = if kind == "flag" { if on { s(fl, "flagText") } else { "" } } else if on { "ON" } else { "OFF" };
        return (row.child(el("span", "fval").child(dot(on)).text(t)), true);
    }
    (row.child(span("fval", &fmt_field(vars, fl))), true)
}

fn group_box(gr: &Value, vars: &Vars, preview: bool) -> Option<Node> {
    let hide = b(gr, "hideMissing");
    let mut rows = Vec::new();
    for fl in a(gr, "fields") {
        let (row, h) = field_row(fl, vars, preview);
        if !h && hide && !preview {
            continue;
        }
        rows.push(row);
    }
    if hide && rows.is_empty() {
        return None;
    }
    Some(el("div", "box").child(h2(s(gr, "title"), 15)).children(rows))
}

pub fn render_groups(groups: &[Value], vars: &Vars, preview: bool) -> Node {
    el("div", "study-groups").children(groups.iter().filter_map(|gr| group_box(gr, vars, preview)))
}

fn stations_as_groups(list: &[Value]) -> Vec<Value> {
    list.iter().map(|st| serde_json::json!({ "title": g(st, "title"), "theme": g(st, "theme"), "hideMissing": false, "fields": g(st, "fields") })).collect()
}

fn unit_label(u: &str) -> Option<&'static str> {
    Some(match u {
        "V" => "Voltage",
        "Hz" => "Frequency",
        "A" => "Current",
        "%" => "Load",
        "psi" => "Pressure",
        "RPM" => "Speed",
        "gal" | "kg" | "lb" => "Quantity",
        "\u{b0}C" | "C" => "Temperature",
        _ => return None,
    })
}

pub fn field_label(fl: &Value) -> String {
    if b(fl, "label") {
        return s(fl, "label").into();
    }
    if s(fl, "kind") == "flag" {
        return sv(fl, "flagText", "undefined");
    }
    if let Some(l) = fl.get("unit").and_then(Value::as_str).and_then(unit_label) {
        return l.into();
    }
    let nm = s(fl, "name");
    let nm = nm.strip_prefix("A32NX_").or_else(|| nm.strip_prefix("A380X_")).unwrap_or(nm);
    nm.replace('_', " ").to_lowercase()
}

fn cmp_op(op: &str, l: f64, r: f64) -> Option<bool> {
    Some(match op {
        "lt" => l < r,
        "le" => l <= r,
        "gt" => l > r,
        "ge" => l >= r,
        "eq" => (l - r).abs() < 1e-9,
        "ne" => (l - r).abs() >= 1e-9,
        _ => return None,
    })
}

pub fn eval_gate(gate: &Value, vars: &Vars) -> bool {
    if gate.is_null() {
        return false;
    }
    let op = s(gate, "op");
    let v = |k: &str| -> Option<f64> { gate.get(k).and_then(Value::as_str).and_then(|nm| var(vars, nm)) };
    if cmp_op(op, 0.0, 0.0).is_some() {
        let left = if gate.get("a").is_some() { v("a") } else { v("name") };
        let right = if gate.get("b").is_some() { v("b") } else { gate.get("value").and_then(Value::as_f64) };
        return match (left, right) {
            (Some(l), Some(r)) => cmp_op(op, l, r).unwrap(),
            _ => false,
        };
    }
    match op {
        "on" => v("name").unwrap_or(0.0) != 0.0,
        "off" => v("name").unwrap_or(0.0) == 0.0,
        "absGt" => v("name").unwrap_or(0.0).abs() > f(gate, "value"),
        "all" => a(gate, "gates").iter().all(|x| eval_gate(x, vars)),
        "any" => a(gate, "gates").iter().any(|x| eval_gate(x, vars)),
        "not" => !eval_gate(g(gate, "gate"), vars),
        "always" => true,
        _ => false,
    }
}

fn gate_names(gate: &Value, out: &mut HashSet<String>) {
    if gate.is_null() {
        return;
    }
    for k in ["name", "a", "b"] {
        if let Some(x) = gate.get(k).and_then(Value::as_str).filter(|x| !x.is_empty()) {
            out.insert(x.into());
        }
    }
    for x in a(gate, "gates") {
        gate_names(x, out);
    }
    if gate.get("gate").is_some() {
        gate_names(g(gate, "gate"), out);
    }
}

pub fn collect_names(page: &Value) -> HashSet<String> {
    let mut out = HashSet::new();
    let fields = |list: &Value, out: &mut HashSet<String>| {
        for fl in list.as_array().map(Vec::as_slice).unwrap_or(&[]) {
            out.insert(sv(fl, "name", "undefined"));
        }
    };
    for gr in a(page, "groups") {
        fields(g(gr, "fields"), &mut out);
    }
    if let Some(t) = page.get("topology").filter(|t| !t.is_null()) {
        for nd in a(t, "nodes") {
            fields(g(nd, "fields"), &mut out);
        }
        for l in a(t, "links") {
            gate_names(g(l, "gate"), &mut out);
        }
    }
    for t in a(page, "tanks") {
        out.insert(sv(t, "name", "undefined"));
    }
    for k in ["stations", "summaries"] {
        for st in a(page, k) {
            fields(g(st, "fields"), &mut out);
        }
    }
    for col in a(page, "columns") {
        for sec in col.as_array().map(Vec::as_slice).unwrap_or(&[]) {
            for ga in a(sec, "gauges") {
                out.insert(sv(ga, "name", "undefined"));
            }
        }
    }
    let kind = s(page, "kind");
    if kind == "fuel" {
        let mut names = vec!["A32NX_EFB_USING_METRIC_UNIT".to_string(), "A32NX_OVHD_APU_MASTER_SW_PB_IS_ON".into()];
        for i in 1..=4 {
            names.extend([format!("FUELSYSTEM VALVE OPEN:{i}"), format!("A32NX_FUEL_USED:{i}"), format!("A32NX_ENGINE_FF:{i}"), format!("A32NX_ENGINE_N3:{i}")]);
        }
        names.extend((1..=11).map(|i| format!("FUELSYSTEM TANK WEIGHT:{i}")));
        names.extend((1..=21).map(|i| format!("FUELSYSTEM PUMP ACTIVE:{i}")));
        names.extend((43..=59).map(|i| format!("FUELSYSTEM VALVE OPEN:{i}")));
        out.extend(names);
    }
    if kind == "topology" && s(page, "title") == "Landing Gear and Brakes" {
        for x in [
            "A32NX_HYD_GREEN_SYSTEM_1_SECTION_PRESSURE_SWITCH", "A32NX_HYD_YELLOW_SYSTEM_1_SECTION_PRESSURE_SWITCH", "A32NX_ELEC_AC_2_BUS_IS_POWERED",
            "A32NX_ELEC_AC_3_BUS_IS_POWERED", "A32NX_ELEC_AC_4_BUS_IS_POWERED", "ANTISKID BRAKES ACTIVE",
        ] {
            out.insert(x.into());
        }
    }
    for al in a(page, "alerts") {
        gate_names(g(al, "trigger"), &mut out);
    }
    if kind == "walkaround" {
        for it in a(page, "items") {
            out.insert(sv(it, "var", "undefined"));
        }
    }
    out
}

fn px(v: f64, sc: f64) -> String {
    format!("{}px", crate::jv::css_num(v * sc))
}

fn scale(avail: f64, design: f64) -> f64 {
    let avail = if avail > 0.0 { avail } else { design };
    (avail / design).min(1.25).max(0.55)
}

fn points(list: &[Value]) -> String {
    list.iter().map(|p| a_join(p)).collect::<Vec<_>>().join(" ")
}

fn a_join(p: &Value) -> String {
    p.as_array().map(|xs| xs.iter().map(crate::jv::jstring).collect::<Vec<_>>().join(",")).unwrap_or_default()
}

fn box_style(x: f64, y: f64, w: f64, h: f64, sc: f64) -> String {
    format!("left: {}; top: {}; width: {}; height: {};", px(x, sc), px(y, sc), px(w, sc), px(h, sc))
}

fn dnode_rows(fields: &[Value], vars: &Vars, sc: f64, narrow: bool, own_label: bool) -> Vec<Node> {
    let mut out = Vec::new();
    for fl in fields {
        let name = s(fl, "name");
        let h = has(vars, name);
        let mut row = el("div", "drow").attr("title", name).style(&format!("height: {};", px(16.0, sc)));
        let kind = s(fl, "kind");
        let lbl = if own_label { sv(fl, "label", "") } else { field_label(fl) };
        if kind == "flag" || kind == "lamp" {
            let on = h && nonzero(vars, name);
            if !own_label && (kind == "flag" || narrow) {
                row = row.class_if("center", true);
                let t = if kind == "flag" { sv(fl, "flagText", "undefined") } else { lbl };
                row = row.child(Node::el("span", if on { "" } else { "lamp-off" }).child(dot(on)).text(&t));
            } else {
                row = row.child(span("dlabel", &lbl));
                let t = if h { if on { "ON" } else { "OFF" } } else { "--" };
                row = row.child(el("span", "dval").class_if("nodata", !h).child(dot(on)).text(t));
            }
        } else {
            if narrow && !own_label {
                row = row.class_if("center", true);
            } else {
                row = row.child(span("dlabel", &lbl));
            }
            let unit = s(fl, "unit");
            let t = if h { fmt_field(vars, fl) } else if unit.is_empty() { "--".into() } else { format!("-- {unit}") };
            row = row.child(span("dval", &t).class_if("nodata", !h));
        }
        out.push(row);
    }
    out
}

pub fn render_topology(topo: &Value, vars: &Vars, avail: f64) -> Node {
    let (dw, dh) = (f(topo, "designW"), f(topo, "designH"));
    let sc = scale(avail, dw);
    let mut svg = Svg::new("diagram-links", &[("viewBox", format!("0 0 {} {}", n(dw), n(dh))), ("preserveAspectRatio", "none".into())]);
    for l in a(topo, "links") {
        let live = eval_gate(g(l, "gate"), vars);
        svg.add("polyline", &[("points", points(a(l, "points"))), ("class", format!("link {} {}", s(l, "kind"), if live { "live" } else { "" }))]);
    }
    let mut d = el("div", "diagram").style(&format!("width: {}; height: {}; font-size: {};", px(dw, sc), px(dh, sc), px(11.5, sc))).child(svg.done());
    for nd in a(topo, "nodes") {
        let fields = a(nd, "fields");
        let live = fields.iter().any(|fl| nonzero(vars, s(fl, "name")));
        let mut bx = Node::el("div", &format!("dnode {}", s(nd, "kind"))).class_if("live", live).style(&box_style(f(nd, "x"), f(nd, "y"), f(nd, "w"), 22.0 + 16.0 * fields.len() as f64, sc));
        bx = bx.child(label("dtitle", s(nd, "title")).attr("title", s(nd, "title")).style(&format!("height: {};", px(22.0, sc))));
        bx = bx.children(dnode_rows(fields, vars, sc, f(nd, "w") < 130.0, false));
        d = d.child(bx);
    }
    el("div", "diagram-wrap").child(d)
}

pub fn render_cutaway(page: &Value, vars: &Vars, avail: f64) -> Node {
    let (w, h) = (1000.0, 600.0);
    let sc = scale(avail, w);
    let mut svg = Svg::new("diagram-links cutaway-shapes", &[("viewBox", "0 0 1000 600".into()), ("preserveAspectRatio", "none".into())]);
    svg.add("line", &[("x1", "0".into()), ("y1", "196".into()), ("x2", "1000".into()), ("y2", "196".into()), ("class", "band".into())]);
    svg.add("line", &[("x1", "0".into()), ("y1", "520".into()), ("x2", "1000".into()), ("y2", "520".into()), ("class", "band".into())]);
    let pts = |l: &[(f64, f64)]| l.iter().map(|(x, y)| format!("{},{}", n(*x), n(*y))).collect::<Vec<_>>().join(" ");
    for sh in a(page, "cutaway") {
        match s(sh, "type") {
            "poly" => {
                svg.add("polygon", &[("points", points(a(sh, "points"))), ("fill", sv(sh, "fill", "undefined")), ("class", "edge".into())]);
            }
            "rect" => {
                svg.add("rect", &[("x", sv(sh, "x", "undefined")), ("y", sv(sh, "y", "undefined")), ("width", sv(sh, "w", "undefined")), ("height", sv(sh, "h", "undefined")), ("fill", sv(sh, "fill", "undefined")), ("class", "edge".into())]);
            }
            "line" => {
                svg.add(
                    "line",
                    &[
                        ("x1", sv(sh, "x1", "undefined")), ("y1", sv(sh, "y1", "undefined")), ("x2", sv(sh, "x2", "undefined")), ("y2", sv(sh, "y2", "undefined")),
                        ("stroke", sv(sh, "stroke", "undefined")), ("stroke-width", sv(sh, "width", "undefined")), ("vector-effect", "non-scaling-stroke".into()),
                    ],
                );
            }
            "arrow" => {
                let (x, y, l, hh) = (f(sh, "x"), f(sh, "y"), f(sh, "len"), f(sh, "half"));
                let head = hh * 1.6;
                let p = pts(&[(x, y - hh * 0.5), (x + l - head, y - hh * 0.5), (x + l - head, y - hh), (x + l, y), (x + l - head, y + hh), (x + l - head, y + hh * 0.5), (x, y + hh * 0.5)]);
                svg.add("polygon", &[("points", p), ("fill", sv(sh, "fill", "undefined")), ("class", "edge".into())]);
            }
            "text" => {
                svg.text(&[("x", sv(sh, "x", "undefined")), ("y", sv(sh, "y", "undefined")), ("class", "caption".into())], s(sh, "text"));
            }
            _ => {}
        }
    }
    let any_vars = !vars.is_empty();
    let boxn = |title: &str, x: f64, y: f64, bw: f64, fields: &[Value]| -> Node {
        let live = any_vars && fields.iter().any(|fl| nonzero(vars, s(fl, "name")));
        el("div", "dnode source")
            .class_if("live", live)
            .style(&box_style(x, y, bw, 22.0 + 16.0 * fields.len() as f64, sc))
            .child(label("dtitle", title).style(&format!("height: {};", px(22.0, sc))))
            .children(dnode_rows(fields, vars, sc, false, true))
    };
    let mut boxes = Vec::new();
    for st in a(page, "stations") {
        let fields = a(st, "fields");
        boxes.push(boxn(s(st, "title"), f(st, "x"), f(st, "y"), f(st, "w"), fields));
        if let Some(ld) = st.get("leader").and_then(Value::as_array) {
            let (tx, ty) = (ld[0].as_f64().unwrap_or(0.0), ld[1].as_f64().unwrap_or(0.0));
            let lbl = ld[2].as_str().unwrap_or("");
            let bottom = f(st, "y") + 22.0 + 16.0 * fields.len() as f64;
            svg.add("line", &[("x1", n(f(st, "x") + f(st, "w") / 2.0)), ("y1", n(bottom)), ("x2", n(tx)), ("y2", n(ty)), ("class", "leader".into())]);
            let r = (lbl.encode_utf16().count() as f64 * 3.5 + 6.0).max(9.0);
            svg.add("ellipse", &[("cx", n(tx)), ("cy", n(ty)), ("rx", n(r)), ("ry", n(r * (w / h) * (h / w))), ("class", "marker".into())]);
            svg.text(&[("x", n(tx)), ("y", n(ty + 4.0)), ("text-anchor", "middle".into()), ("class", "marker-text".into())], lbl);
        }
    }
    for (i, st) in a(page, "summaries").iter().enumerate() {
        boxes.push(boxn(s(st, "title"), 6.0 + i as f64 * 124.5, 527.0, 118.0, a(st, "fields")));
    }
    let d = el("div", "diagram").style(&format!("width: {}; height: {}; font-size: {};", px(w, sc), px(h, sc), px(11.5, sc))).child(svg.done()).children(boxes);
    el("div", "diagram-wrap").child(d)
}

fn flight_controls(page: &Value, vars: &Vars, preview: bool) -> Vec<Node> {
    let mut out = Vec::new();
    for col in a(page, "columns") {
        for sec in col.as_array().map(Vec::as_slice).unwrap_or(&[]) {
            let mut bx = el("div", "box").child(h2(s(sec, "heading"), 14));
            for ga in a(sec, "gauges") {
                let fl = serde_json::json!({ "label": g(ga, "label"), "name": g(ga, "name"), "kind": "num", "decimals": 2, "unit": g(ga, "unit") });
                bx = bx.child(field_row(&fl, vars, preview).0);
            }
            out.push(bx);
        }
    }
    out
}

fn ground_services(page: &Value, vars: &Vars, preview: bool) -> Vec<Node> {
    let mut row = el("div", "status");
    for bt in a(page, "buttons").iter().chain(a(page, "tug")) {
        row = row.child(button("small grey", s(bt, "label")).attr("type", "button").act(&format!("study:command:{}", s(bt, "command"))));
    }
    row = row.child(button("small", "Service oxygen").attr("type", "button").act("study:serviceOxygen"));
    vec![el("div", "box").child(row), render_groups(a(page, "groups"), vars, preview)]
}

fn walkaround(page: &Value, vars: &Vars) -> Vec<Node> {
    let installed = |it: &Value| var(vars, s(it, "var")).unwrap_or(0.0) > 0.5;
    let mut all = el("div", "status");
    for (lbl, k) in [("Remove all", "removeAllCommand"), ("Install all", "installAllCommand")] {
        all = all.child(button("small", lbl).attr("type", "button").act(&format!("study:command:{}", s(page, k))));
    }
    let mut out = vec![el("div", "box").child(all)];
    let items = a(page, "items");
    let mut wrap = el("div", "study-groups");
    for gr in a(page, "groups") {
        let mut row = el("div", "status");
        for i in a(gr, "items") {
            let Some(it) = i.as_u64().and_then(|i| items.get(i as usize)) else { continue };
            let on = installed(it);
            let t = format!(" {}{}", s(it, "label"), if on { " - INSTALLED" } else { " - removed" });
            row = row.child(
                el("button", "small")
                    .class_if("grey", !on)
                    .attr("type", "button")
                    .act(&format!("study:command:{}", s(it, "toggleCommand")))
                    .child(el("span", "dot").class_if("bad", on).class_if("ok", !on))
                    .text(&t),
            );
        }
        wrap = wrap.child(el("div", "box").child(h2(s(gr, "name"), 15)).child(row));
    }
    out.push(wrap);
    let on = |p: &str| items.iter().any(|it| s(it, "id").starts_with(p) && installed(it));
    let mut notes = Vec::new();
    if on("pitot_") || on("aoa_") || on("static_") {
        notes.push("Probe covers installed: the covered pitot, static and AoA sources read blocked in the ADIRUs.");
    }
    if on("eng_inlet_") {
        notes.push("Inlet covers installed: starting that engine ingests the cover (foreign-object damage).");
    }
    if on("eng_exhaust_") {
        notes.push("Exhaust covers installed: starting that engine blows the cover out.");
    }
    if on("gear_pin_") {
        notes.push("Gear pins installed: a pinned leg cannot retract.");
    }
    if on("chocks_") {
        notes.push("Chocks installed: they hold the wheels until the beacon is on or engine 1 or 2 runs, and rolling over about 1 kt then overruns them.");
    }
    if !notes.is_empty() {
        out.push(el("div", "box").children(notes.into_iter().map(|t| Node::new("div").text(t))));
    }
    out
}

fn section_box(title: &str, rows: Vec<Node>) -> Node {
    el("div", "box").child(h2(title, 15)).children(rows)
}

pub fn render_area(page: &Value, vars: &Vars, preview: bool) -> Vec<Node> {
    let comps = a(page, "components");
    let fails = a(page, "failures");
    let alerts = a(page, "alerts");
    let armed = fails.iter().filter(|x| b(x, "armed")).count();
    let triggered = alerts.iter().filter(|x| eval_gate(g(x, "trigger"), vars)).count();
    let mut out = vec![label(
        "box",
        &format!(
            "{} components  /  {} failures ({armed} armed)  /  {} ECAM alerts ({triggered} triggered now)  /  {} published variables",
            comps.len(),
            fails.len(),
            alerts.len(),
            collect_names(page).len()
        ),
    )];
    if !comps.is_empty() {
        let grid = el("div", "study-grid-list").children(comps.iter().map(|c| {
            let params: Vec<String> = a(c, "params").iter().map(|p| sv(p, "name", "undefined")).collect();
            let t = if params.is_empty() { "no health parameters".to_string() } else { params.join(", ") };
            label("study-item-btn", &format!("{}  (ATA {})", sv(c, "name", "undefined"), sv(c, "ata", "undefined"))).attr("title", &t)
        }));
        out.push(section_box("Components", vec![grid]));
    }
    if !fails.is_empty() {
        let grid = el("div", "study-grid-list").children(fails.iter().map(|x| {
            button("study-item-btn", &format!("{} {}", sv(x, "id", "undefined"), sv(x, "name", "undefined")))
                .class_if("active", b(x, "armed"))
                .attr("type", "button")
                .act(&format!("study:toggleFailure:{}", sv(x, "id", "")))
        }));
        out.push(section_box("Failures", vec![grid]));
    }
    if !alerts.is_empty() {
        let rows = alerts
            .iter()
            .map(|al| {
                let on = eval_gate(g(al, "trigger"), vars);
                el("div", "study-field-row")
                    .child(el("span", "fname").child(el("span", "dot").class_if("warn", on)).text(&format!("{} ({})", sv(al, "title", "undefined"), s(al, "level").to_uppercase())))
                    .child(span("fval", if on { "TRIGGERED" } else { "clear" }))
            })
            .collect();
        out.push(section_box("ECAM alerts", rows));
    }
    if !a(page, "groups").is_empty() {
        out.push(render_groups(a(page, "groups"), vars, preview));
    } else {
        out.push(label("box", "This area publishes no variables of its own (its failures/components still show above)."));
    }
    out
}

fn all_vars(vars: &Vars) -> Node {
    let mut names: Vec<&String> = vars.keys().collect();
    names.sort_by(|x, y| x.encode_utf16().cmp(y.encode_utf16()));
    el("div", "box").children(names.into_iter().map(|nm| el("div", "study-field-row").child(span("fname", nm)).child(span("fval", &crate::jv::jstring(&vars[nm.as_str()])))))
}

fn ecam_num(vars: &Vars, name: &str) -> Option<f64> {
    let raw = var(vars, name)?;
    if !raw.is_finite() {
        return None;
    }
    if crate::jv::is_packed(raw) {
        let (v, ssm) = crate::jv::unpack_arinc(raw);
        return (ssm == 3).then_some(v);
    }
    Some(raw)
}

fn ecam_bool(vars: &Vars, name: &str, th: f64) -> Option<bool> {
    ecam_num(vars, name).map(|v| v >= th)
}

fn e_text(svg: &mut Svg, x: f64, y: f64, color: &str, size: f64, text: &str, anchor: &str) {
    svg.text(&[("x", n(x)), ("y", n(y)), ("fill", color.into()), ("font-size", n(size)), ("text-anchor", anchor.into())], text);
}

fn e_line(svg: &mut Svg, x1: f64, y1: f64, x2: f64, y2: f64, color: &str, w: f64) {
    svg.add("line", &[("x1", n(x1)), ("y1", n(y1)), ("x2", n(x2)), ("y2", n(y2)), ("stroke", color.into()), ("stroke-width", n(w)), ("stroke-linecap", "round".into())]);
}

fn e_rect(svg: &mut Svg, x: f64, y: f64, w: f64, h: f64, color: &str, sw: f64) {
    svg.add("rect", &[("x", n(x)), ("y", n(y)), ("width", n(w)), ("height", n(h)), ("fill", "none".into()), ("stroke", color.into()), ("stroke-width", n(sw))]);
}

fn e_circle(svg: &mut Svg, cx: f64, cy: f64, r: f64, color: &str, sw: f64) {
    svg.add("circle", &[("cx", n(cx)), ("cy", n(cy)), ("r", n(r)), ("fill", "none".into()), ("stroke", color.into()), ("stroke-width", n(sw))]);
}

fn e_valve(svg: &mut Svg, x: f64, y: f64, open: Option<bool>, horizontal: bool, normally_closed: bool) {
    let color = match open {
        None => ECAM_AMBER,
        Some(false) if !normally_closed => ECAM_AMBER,
        _ => ECAM_GREEN,
    };
    let rotate = if (open == Some(true)) != !horizontal { 90 } else { 0 };
    svg.open("g", &[("transform", format!("rotate({rotate} {} {})", n(x), n(y)))]);
    e_circle(svg, x, y, 16.0, color, 2.8);
    e_line(svg, x, y - 16.0, x, y + 16.0, color, 3.0);
    svg.close();
}

fn e_pump(svg: &mut Svg, x: f64, y: f64, running: Option<bool>) {
    let color = match running {
        None => ECAM_AMBER,
        Some(true) => ECAM_GREEN,
        Some(false) => ECAM_WHITE,
    };
    let w = 28.0;
    e_rect(svg, x - w / 2.0, y - w / 2.0, w, w, color, 2.8);
    if running == Some(true) {
        e_line(svg, x, y - w / 2.0, x, y + w / 2.0, color, 3.0);
    } else {
        e_line(svg, x - 9.0, y, x + 9.0, y, color, 3.0);
    }
}

fn ecam_panel() -> Svg {
    Svg::new("ecam-panel", &[("viewBox", "0 0 768 1024".into())])
}

pub fn render_fuel_ecam(vars: &Vars) -> Node {
    let metric = ecam_num(vars, "A32NX_EFB_USING_METRIC_UNIT").map_or(true, |v| v != 0.0);
    let uw = |kg: f64| if metric { kg } else { kg * 2.204625 };
    let unit = if metric { "KG" } else { "LBS" };
    let mut svg = ecam_panel();
    e_text(&mut svg, 10.0, 32.0, ECAM_WHITE, 24.0, "FUEL", "start");
    let eng_used: Vec<Option<f64>> = (1..=4).map(|i| ecam_num(vars, &format!("A32NX_FUEL_USED:{i}"))).collect();
    let apu_used = ecam_num(vars, "A32NX_APU_FUEL_USED");
    let total = if eng_used.iter().all(Option::is_some) { Some(eng_used.iter().map(|x| x.unwrap()).sum::<f64>() + apu_used.unwrap_or(0.0)) } else { None };
    e_text(&mut svg, 384.0, 56.0, ECAM_WHITE, 20.0, "FU", "middle");
    e_text(&mut svg, 384.0, 79.0, ECAM_WHITE, 20.0, if apu_used.is_some() { "TOTAL" } else { "ALL ENG" }, "middle");
    let t = total.map_or("XX".to_string(), |k| num((uw(k) / 50.0).floor() * 50.0));
    e_text(&mut svg, 384.0, 103.0, if total.is_none() { ECAM_AMBER } else { ECAM_GREEN }, 24.0, &t, "middle");
    e_text(&mut svg, 384.0, 126.0, ECAM_CYAN, 18.0, unit, "middle");
    const BOX: [(f64, f64); 4] = [(74.0, 105.0), (236.0, 81.0), (456.0, 81.0), (618.0, 105.0)];
    const LP: [(f64, f64, f64, f64); 4] = [(111.0, 150.0, 111.0, 84.0), (273.0, 123.0, 273.0, 68.0), (493.0, 123.0, 493.0, 68.0), (655.0, 150.0, 655.0, 84.0)];
    for i in 0..4 {
        let nn = i + 1;
        let (bx, by) = BOX[i];
        let (lx, ly, tx, ty) = LP[i];
        let n3 = ecam_num(vars, &format!("A32NX_ENGINE_N3:{nn}"));
        let running = n3.is_some_and(|v| v > 50.0);
        e_rect(&mut svg, bx, by, 75.0, 96.0, ECAM_GREY, 2.0);
        e_text(&mut svg, bx + 8.0, by + 32.0, if n3.is_none() { ECAM_AMBER } else if running { ECAM_WHITE } else { ECAM_AMBER }, 22.0, &nn.to_string(), "start");
        let open = ecam_bool(vars, &format!("FUELSYSTEM VALVE OPEN:{nn}"), 0.5);
        e_valve(&mut svg, lx, ly, open, false, false);
        let t = eng_used[i].map_or("XX".to_string(), |k| num((uw(k) / 50.0).floor() * 50.0));
        e_text(&mut svg, tx, ty, ECAM_GREEN, 20.0, &t, "middle");
    }
    let ff: Vec<Option<f64>> = (1..=4).map(|i| ecam_num(vars, &format!("A32NX_ENGINE_FF:{i}"))).collect();
    let ff_known = ff.iter().all(Option::is_some);
    e_text(&mut svg, 10.0, 615.0, ECAM_WHITE, 18.0, "ALL ENG FF", "start");
    let t = if ff_known { num((uw(ff.iter().map(|x| x.unwrap()).sum::<f64>()) / 60.0 / 10.0).floor() * 10.0) } else { "XX".into() };
    e_text(&mut svg, 24.0, 640.0, if ff_known { ECAM_GREEN } else { ECAM_AMBER }, 18.0, &t, "start");
    e_text(&mut svg, 68.0, 640.0, ECAM_CYAN, 16.0, &format!("{unit}/MIN"), "start");
    for (idx, x, y) in [(1, 95.0, 227.0), (2, 127.0, 227.0), (3, 258.0, 208.0), (4, 290.0, 208.0), (5, 476.0, 208.0), (6, 508.0, 208.0), (7, 639.0, 227.0), (8, 671.0, 227.0)] {
        let st = ecam_bool(vars, &format!("FUELSYSTEM PUMP ACTIVE:{idx}"), 0.5);
        e_pump(&mut svg, x, y, st);
    }
    let mut any = false;
    for (idx, x, y) in [(46, 154.0, 175.0), (47, 316.0, 148.0), (48, 450.0, 148.0), (49, 612.0, 175.0)] {
        let open = ecam_bool(vars, &format!("FUELSYSTEM VALVE OPEN:{idx}"), 0.5);
        if open == Some(true) {
            any = true;
        }
        e_valve(&mut svg, x, y, open, true, true);
    }
    let lc = |on: bool| if on { ECAM_GREEN } else { ECAM_GREY };
    e_line(&mut svg, 171.0, 175.0, 595.0, 175.0, lc(any), 3.0);
    e_line(&mut svg, 316.0, 148.0, 316.0, 175.0, lc(any), 3.0);
    e_line(&mut svg, 450.0, 148.0, 450.0, 175.0, lc(any), 3.0);
    {
        let apu_on = ecam_bool(vars, "A32NX_OVHD_APU_MASTER_SW_PB_IS_ON", 0.5);
        let iso = ecam_bool(vars, "FUELSYSTEM VALVE OPEN:50", 0.1);
        let lp = ecam_bool(vars, "FUELSYSTEM VALVE OPEN:51", 0.1);
        let normal = apu_on.is_some() && iso.is_some() && lp.is_some() && iso == apu_on && lp == apu_on;
        e_text(&mut svg, 705.0, 180.0, if normal { ECAM_WHITE } else { ECAM_AMBER }, 18.0, "APU", "middle");
    }
    let fwd = [(9, 84.0, 384.0), (10, 140.0, 384.0), (12, 232.0, 384.0), (13, 482.0, 384.0), (15, 584.0, 384.0), (14, 680.0, 384.0)];
    let aft = [(11, 182.0, 452.0), (17, 274.0, 452.0), (18, 524.0, 452.0), (16, 626.0, 452.0)];
    for grp in [&fwd[..], &aft[..]] {
        let states: Vec<Option<bool>> = grp.iter().map(|(i, _, _)| ecam_bool(vars, &format!("FUELSYSTEM PUMP ACTIVE:{i}"), 0.5)).collect();
        let any_on = states.iter().any(|s| *s == Some(true));
        e_line(&mut svg, grp[0].1, grp[0].2, grp[grp.len() - 1].1, grp[0].2, lc(any_on), 3.0);
        for (k, (_, x, y)) in grp.iter().enumerate() {
            e_pump(&mut svg, *x, *y, states[k]);
        }
    }
    for (idx, x) in [(57, 190.0), (58, 570.0)] {
        if ecam_bool(vars, &format!("FUELSYSTEM VALVE OPEN:{idx}"), 0.1) == Some(true) {
            e_line(&mut svg, x, 500.0, x, 522.0, ECAM_GREEN, 4.0);
            e_text(&mut svg, x - 56.0, 562.0, ECAM_WHITE, 18.0, "JETTISON", "start");
        }
    }
    let open = ecam_bool(vars, "FUELSYSTEM VALVE OPEN:43", 0.1);
    e_valve(&mut svg, 386.0, 540.0, open, false, true);
    e_pump(&mut svg, 298.0, 610.0, ecam_bool(vars, "FUELSYSTEM PUMP ACTIVE:19", 0.5));
    e_pump(&mut svg, 468.0, 610.0, ecam_bool(vars, "FUELSYSTEM PUMP ACTIVE:20", 0.5));
    let tanks = [
        (1, 102.0, 434.0, false, "OUTER"), (2, 154.0, 300.0, true, "FEED 1"), (3, 202.0, 430.0, false, "MID"), (4, 302.0, 430.0, false, "INNER"),
        (5, 322.0, 288.0, true, "FEED 2"), (6, 528.0, 288.0, true, "FEED 3"), (7, 548.0, 430.0, false, "INNER"), (8, 648.0, 430.0, false, "MID"),
        (9, 696.0, 300.0, true, "FEED 4"), (10, 748.0, 434.0, false, "OUTER"), (11, 418.0, 640.0, false, "TRIM"),
    ];
    for (tn, x, y, feed, lbl) in tanks {
        let kg = ecam_num(vars, &format!("FUELSYSTEM TANK WEIGHT:{tn}")).map(|l| l * 0.45359237);
        let low = feed && kg.map_or(true, |k| k < 1375.0);
        let q = kg.map_or("XX ".to_string(), |k| num((uw(k) / 20.0).floor() * 20.0));
        e_text(&mut svg, x, y, if low || kg.is_none() { ECAM_AMBER } else { ECAM_GREEN }, 22.0, &q, "end");
        e_text(&mut svg, x, y - 24.0, ECAM_WHITE, 13.0, lbl, "end");
    }
    el("div", "ecam-wrap").child(svg.done())
}

fn bogey_numbers(kind: &str, left: bool) -> Vec<u32> {
    if kind == "nose" {
        return vec![];
    }
    let cnt = if kind == "wlg" { 4 } else { 6 };
    let base = if kind == "wlg" { if left { 1 } else { 3 } } else if left { 9 } else { 11 };
    (0..cnt).map(|i| i + (i / 2) * 2 + base).collect()
}

pub fn render_gear_ecam(vars: &Vars) -> Node {
    let mut svg = ecam_panel();
    e_text(&mut svg, 10.0, 32.0, ECAM_WHITE, 24.0, "WHEEL", "start");
    let legs: [(&str, f64, f64, Option<&str>, bool, bool); 5] = [
        ("CENTER", 351.0, 59.0, Some("CENTER"), false, false),
        ("LEFT", 48.0, 223.0, Some("LEFT"), true, false),
        ("LEFT", 223.0, 260.0, None, false, true),
        ("RIGHT", 479.0, 260.0, None, false, true),
        ("RIGHT", 654.0, 223.0, Some("RIGHT"), true, false),
    ];
    for (key, x, y, door, _, _) in legs {
        if let Some(pos) = ecam_num(vars, &format!("A32NX_GEAR_{key}_POSITION")).filter(|p| *p >= 2.0) {
            let color = if pos > 98.0 { ECAM_GREEN } else { ECAM_RED };
            let (cx, cy) = (x + 14.0, y + 13.0);
            svg.add(
                "polygon",
                &[
                    ("points", format!("{},{} {},{} {},{}", n(cx - 15.0), n(cy + 13.0), n(cx + 15.0), n(cy + 13.0), n(cx), n(cy - 13.0))),
                    ("fill", "none".into()),
                    ("stroke", color.into()),
                    ("stroke-width", "3".into()),
                ],
            );
        }
        if let Some(d) = door {
            if let Some(dp) = ecam_num(vars, &format!("A32NX_GEAR_DOOR_{d}_POSITION")) {
                e_line(&mut svg, x - 10.0, y - 10.0, x + 38.0, y - 10.0, if dp > 2.0 { ECAM_AMBER } else { ECAM_GREEN }, 2.0);
            }
        }
    }
    if let Some(dp) = ecam_num(vars, "A32NX_GEAR_DOOR_CENTER_POSITION") {
        e_line(&mut svg, 331.0, 46.0, 379.0, 46.0, if dp > 2.0 { ECAM_AMBER } else { ECAM_GREEN }, 2.0);
    }
    let supply = |svg: &mut Svg, x: f64, y: f64, l: &str, av: Option<bool>| {
        svg.add("rect", &[("x", n(x)), ("y", n(y)), ("width", "21".into()), ("height", "21".into()), ("fill", "#4f5467".into())]);
        e_text(svg, x + 5.0, y + 16.0, if av == Some(true) { ECAM_GREEN } else { ECAM_AMBER }, 16.0, l, "start");
    };
    let green = ecam_bool(vars, "A32NX_HYD_GREEN_SYSTEM_1_SECTION_PRESSURE_SWITCH", 0.5);
    let yellow = ecam_bool(vars, "A32NX_HYD_YELLOW_SYSTEM_1_SECTION_PRESSURE_SWITCH", 0.5);
    let ac2 = ecam_bool(vars, "A32NX_ELEC_AC_2_BUS_IS_POWERED", 0.5);
    let ac4 = ecam_bool(vars, "A32NX_ELEC_AC_4_BUS_IS_POWERED", 0.5);
    supply(&mut svg, 374.0, 20.0, "G", green);
    for (_, x, y, _, wlg, _) in legs {
        if wlg {
            supply(&mut svg, x + 23.0, y - 32.0, "G", green);
        }
    }
    for (_, x, y, _, _, blg) in legs {
        if blg {
            supply(&mut svg, x - 30.0, y - 36.0, "Y", yellow);
        }
    }
    for (kind, left, x, y) in [("nose", false, 384.0, 140.0), ("wlg", true, 81.0, 403.0), ("blg", true, 257.0, 417.0), ("blg", false, 513.0, 417.0), ("wlg", false, 690.0, 403.0)] {
        let nums = bogey_numbers(kind, left);
        if kind == "nose" {
            e_circle(&mut svg, x - 18.0, y, 15.0, ECAM_GREY, 2.0);
            e_circle(&mut svg, x + 18.0, y, 15.0, ECAM_GREY, 2.0);
            continue;
        }
        let spacing = if kind == "wlg" { 92.0 } else { 90.0 };
        for row in 0..nums.len() / 2 {
            let yy = y + row as f64 * spacing;
            for (num_w, xx) in [(nums[row * 2], x - 22.0), (nums[row * 2 + 1], x + 22.0)] {
                e_circle(&mut svg, xx, yy, 15.0, ECAM_GREY, 2.0);
                e_text(&mut svg, xx, yy + 5.0, ECAM_WHITE, 13.0, &num_w.to_string(), "middle");
                if num_w < 17 {
                    let temp = ecam_num(vars, &format!("A32NX_BRAKE_TEMPERATURE_{num_w}"));
                    let l = temp.map_or("XX".to_string(), |t| num(crate::js::round(t.max(0.0).min(995.0) / 5.0) * 5.0));
                    let c = if temp.map_or(true, |t| t > 300.0) { ECAM_AMBER } else { ECAM_GREEN };
                    e_text(&mut svg, xx, yy - 20.0, c, 13.0, &l, "middle");
                }
            }
        }
    }
    e_text(&mut svg, 135.0, 300.0, ECAM_CYAN, 16.0, "\u{b0}C", "middle");
    e_text(&mut svg, 384.0, 320.0, ECAM_WHITE, 18.0, "BRK", "middle");
    let brk = [(81.0, true), (258.0, false), (513.0, false), (690.0, true)];
    for (x, wlg) in brk {
        supply(&mut svg, x - 23.0, 328.0, if wlg { "G" } else { "Y" }, if wlg { green } else { yellow });
    }
    for (x, wlg) in brk {
        supply(&mut svg, x + 1.0, 328.0, if wlg { "4" } else { "2" }, if wlg { ac4 } else { ac2 });
    }
    let antiskid = ecam_bool(vars, "ANTISKID BRAKES ACTIVE", 0.5);
    let off = antiskid == Some(false);
    e_text(&mut svg, 10.0, 660.0, if off { ECAM_AMBER } else { ECAM_WHITE }, 16.0, if off { "ANTI SKID OFF" } else { "" }, "start");
    el("div", "ecam-wrap").child(svg.done())
}

fn excluded(p: &Value) -> bool {
    matches!(s(p, "kind"), "failures" | "breakers" | "groundServices" | "loadsheet") || b(p, "merged")
}

pub fn study_nav(pages: &[Value], v: &StudyView) -> Node {
    let mut nav = el("nav", "study-nav").id("study-nav");
    let order: Vec<String> = match &v.group {
        Some(gr) => vec![gr.clone()],
        None => ["", "Engines", "Environmental", "Hydraulics", "Landing Gear", "Fire"].map(String::from).to_vec(),
    };
    for key in order {
        let list: Vec<&Value> = pages.iter().filter(|p| !excluded(p) && s(p, "menuGroup") == key).collect();
        if list.is_empty() {
            continue;
        }
        if !key.is_empty() {
            nav = nav.child(label("study-nav-group", &key));
        }
        for p in list {
            let idx = f(p, "index");
            nav = nav.child(Node::el("button", if idx == v.selected as f64 { "active" } else { "" }).attr("type", "button").text(s(p, "title")).act(&format!("study:page:{}", num(idx))));
        }
    }
    nav
}

/// `selectStudyPage`'s default: the first page of the group when the selection is outside it.
pub fn group_selection(pages: &[Value], v: &StudyView) -> usize {
    let gr = v.group.clone().unwrap_or_default();
    let list: Vec<&Value> = pages.iter().filter(|p| !excluded(p) && s(p, "menuGroup") == gr).collect();
    if list.is_empty() || list.iter().any(|p| f(p, "index") == v.selected as f64) {
        return v.selected;
    }
    f(list[0], "index") as usize
}

pub fn study_content(page: &Value, vars: &Vars, v: &StudyView) -> Vec<Node> {
    let pv = v.preview;
    let mut out = Vec::new();
    match s(page, "kind") {
        "groups" => out.push(render_groups(a(page, "groups"), vars, pv)),
        "topology" => {
            if v.physics {
                out.push(render_groups(a(page, "groups"), vars, pv));
            } else if s(page, "title") == "Landing Gear and Brakes" {
                out.push(render_gear_ecam(vars));
            } else {
                out.push(render_topology(g(page, "topology"), vars, v.width));
            }
        }
        "fuel" => out.push(if v.physics { render_groups(a(page, "groups"), vars, pv) } else { render_fuel_ecam(vars) }),
        "engine" => {
            if v.physics {
                out.push(render_groups(a(page, "groups"), vars, pv));
            } else if page.get("cutaway").is_some_and(super::jv::truthy) {
                out.push(render_cutaway(page, vars, v.width));
            } else {
                out.push(render_groups(&stations_as_groups(a(page, "stations")), vars, pv));
                out.push(render_groups(&stations_as_groups(a(page, "summaries")), vars, pv));
                out.push(render_groups(a(page, "groups"), vars, pv));
            }
        }
        "flightControls" => out.extend(flight_controls(page, vars, pv)),
        "groundServices" => out.extend(ground_services(page, vars, pv)),
        "walkaround" => out.extend(walkaround(page, vars)),
        "area" => out.extend(render_area(page, vars, pv)),
        "all" => out.push(all_vars(vars)),
        _ => {}
    }
    for dp in a(page, "deepPages") {
        let t = s(dp, "title");
        let t = match t.strip_prefix("Deep: ") {
            Some(r) => format!("Deep systems / {r}"),
            None => t.to_string(),
        };
        out.push(label("study-nav-group", &t).style("margin:26px 0 8px;border-top:1px solid var(--line-dim);padding-top:16px"));
        out.extend(render_area(dp, vars, pv));
    }
    out
}

pub fn preview_banner() -> Node {
    el("div", "box study-offline preview-banner").attr("hidden", "").child(span("label dim", "Preview: X-Plane is not running. Live values appear once the A380X is loaded."))
}

fn tab_panel(id: &str, active: bool) -> Node {
    el("div", "tab-panel").class_if("active", active).id(&format!("tab-{id}"))
}

pub fn study_tab(pages: &[Value], vars: &Vars, v: &StudyView, active: bool) -> Node {
    let page = pages.get(v.selected);
    let mut toolbar = el("div", "study-toolbar").id("study-toolbar");
    let mut content = Node::new("div").id("study-content");
    if let Some(p) = page {
        if b(p, "hasPhysics") {
            toolbar = toolbar.child(button("small", if v.physics { "SCHEMATIC" } else { "PHYSICS" }).class_if("grey", !v.physics).attr("type", "button").act("study:physics"));
        }
        if !matches!(s(p, "kind"), "failures" | "breakers") {
            content = content.children(study_content(p, vars, v));
        }
    }
    let mut banner = preview_banner();
    if v.preview {
        banner.attrs.remove("hidden");
    }
    let mut offline = el("div", "box study-offline").id("study-offline").child(span("label dim", "Not connected to X-Plane / the plugin. Retrying\u{2026}"));
    if !v.offline {
        offline = offline.attr("hidden", "");
    }
    let mut layout = el("div", "study-layout").id("study-layout");
    if v.offline {
        layout = layout.style("display: none;");
    }
    tab_panel("study", active)
        .child(banner)
        .child(offline)
        .child(layout.child(study_nav(pages, v)).child(el("div", "study-main").child(toolbar).child(content)))
}

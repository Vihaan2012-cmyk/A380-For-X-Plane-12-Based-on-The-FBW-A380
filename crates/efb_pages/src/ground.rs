use crate::{button, label, span, Node};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub const AIRCRAFT_PRESETS: [(u32, &str, &str); 8] = [
    (1, "Cold & Dark", "FlyByWire preset 1"),
    (2, "Powered", "FlyByWire preset 2"),
    (3, "Ready for Pushback", "FlyByWire preset 3"),
    (4, "Ready for Taxi", "FlyByWire preset 4"),
    (5, "Ready for Takeoff", "FlyByWire preset 5"),
    (7, "After Start", "first officer flow"),
    (8, "Taxi", "first officer flow"),
    (6, "After Landing", "first officer flow"),
];
const PRESET_LOAD: &str = "A32NX_AIRCRAFT_PRESET_LOAD";
const PRESET_PROGRESS: &str = "A32NX_AIRCRAFT_PRESET_LOAD_PROGRESS";
const PRESET_EXPEDITE: &str = "A32NX_AIRCRAFT_PRESET_LOAD_EXPEDITE";
const LP_COUNT: u32 = 8;
const LP_POWER: &str = "A32NX_ELEC_AC_1_BUS_IS_POWERED";
pub const GM_CHOCKS: [&str; 5] = ["nose", "lwing", "rwing", "lbody", "rbody"];
pub const GM_ENGINES: usize = 4;
const GM_OPERATORS: [(&str, &str); 4] = [("0", "AUTO"), ("dnata", "DNATA"), ("swissport", "SWISSPORT"), ("menzies", "MENZIES")];

pub enum Spec {
    Svc(&'static str),
    Places(&'static [&'static str]),
}

pub const GM_GROUPS: [(&str, &[(&str, &str, Spec)]); 6] = [
    ("POWER AND AIR", &[("gpu", "Ground power unit", Spec::Svc("gpu")), ("asu", "Air start unit", Spec::Svc("asu")), ("pca", "Preconditioned air", Spec::Svc("pca"))]),
    ("CATERING", &[("cat_main", "Main deck highloader at M2R", Spec::Places(&["cat_m2r"])), ("cat_upper", "Upper deck highloader at U1R", Spec::Places(&["udcat_u1r"]))]),
    (
        "BAGGAGE AND CARGO",
        &[
            ("hold_fwd", "Forward hold: loader and ULD train", Spec::Places(&["ldcl_fwd", "uld_fwd"])),
            ("hold_aft", "Aft hold: loader and ULD train", Spec::Places(&["ldcl_aft", "uld_aft"])),
            ("hold_bulk", "Bulk hold: belt loader, tractor and carts", Spec::Places(&["belt_bulk", "bag_tractor", "bulk_cart"])),
        ],
    ),
    ("FUEL", &[("fuel_hydrant", "Hydrant dispenser", Spec::Places(&["fuel_hydrant"])), ("fuel_tanker", "Fuel tanker", Spec::Places(&["fuel_tanker"]))]),
    (
        "STAIRS",
        &[
            ("stairs_m1l", "Main deck stairs at M1L", Spec::Places(&["stairs_m1l"])),
            ("stairs_m2l", "Main deck stairs at M2L", Spec::Places(&["stairs_m2l"])),
            ("stairs_u1l", "Upper deck stairs at U1L", Spec::Places(&["stairs_u1l"])),
        ],
    ),
    (
        "CABIN SERVICES",
        &[
            ("water", "Potable water truck", Spec::Svc("water")),
            ("lav", "Lavatory service truck", Spec::Svc("lav")),
            ("clean", "Cabin cleaning", Spec::Svc("clean")),
            ("deice", "De-icing truck", Spec::Svc("deice")),
        ],
    ),
];

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Field {
    pub name: String,
    pub label: Option<String>,
    pub kind: String,
    pub decimals: usize,
    pub unit: String,
    #[serde(rename = "flagText")]
    pub flag_text: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Group {
    pub title: String,
    pub fields: Vec<Field>,
    #[serde(rename = "hideMissing")]
    pub hide_missing: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct GroundModel {
    pub vars: BTreeMap<String, Value>,
    pub gse: BTreeMap<String, Value>,
    pub groups: Vec<Group>,
    pub settings: BTreeMap<String, String>,
    pub lp_editing: u32,
    pub lp_armed: u32,
    pub preview: bool,
}

fn num(v: Option<&Value>) -> f64 {
    let x = match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() { 0.0 } else { t.parse().unwrap_or(f64::NAN) }
        }
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        _ => 0.0,
    };
    if x.is_nan() { 0.0 } else { x }
}

impl GroundModel {
    fn var(&self, k: &str) -> f64 {
        num(self.vars.get(k))
    }
    fn gse(&self, k: &str) -> f64 {
        num(self.gse.get(k))
    }
    fn state(&self, spec: &Spec) -> u32 {
        match spec {
            Spec::Places(ps) => ps.iter().map(|p| self.gse(&format!("place/{p}/state"))).fold(0.0, f64::max) as u32,
            Spec::Svc(s) => {
                if *s == "gpu" && self.gse("connected/gpu") > 0.5 {
                    2
                } else {
                    self.gse(&format!("{s}/state")) as u32
                }
            }
        }
    }
    fn chocks(&self) -> usize {
        GM_CHOCKS.iter().filter(|c| self.var(&format!("A32NX_WALKAROUND_CHOCKS_{}", c.to_uppercase())) > 0.5).count()
    }
    fn cowl(&self, e: usize) -> f64 {
        self.var(&format!("A32NX_ENGINE_COWL_OPEN_{}", e + 1))
    }
    fn lp_names(&self) -> BTreeMap<u32, String> {
        let raw: Option<Value> = self.settings.get("LIGHT_PRESET_NAMES").and_then(|s| serde_json::from_str(s).ok());
        let mut out = BTreeMap::new();
        if let Some(r) = raw.filter(|r| r.get("dataType").and_then(Value::as_str) == Some("Map")) {
            for kv in r.get("value").and_then(Value::as_array).into_iter().flatten() {
                if let Some([k, v]) = kv.as_array().map(Vec::as_slice) {
                    let id = num(Some(k)) as u32;
                    out.insert(id, crate::model::jstr(v));
                }
            }
        }
        out
    }
}

fn state_text(st: u32) -> &'static str {
    ["", "On the way", "In place", "Leaving"].get(st as usize).copied().unwrap_or("")
}

pub fn to_fixed(v: f64, d: usize) -> String {
    let s = format!("{v:.d$}");
    if v == 0.0 && s.starts_with('-') { s[1..].to_string() } else { s }
}

fn with_unit(n: String, unit: &str) -> String {
    if unit.is_empty() { n } else { format!("{n} {unit}") }
}

pub fn format_value(value: f64, kind: &str, decimals: usize, unit: &str) -> String {
    if !value.is_finite() {
        return "no reading".into();
    }
    if kind == "lamp" {
        return if value != 0.0 { "ON" } else { "OFF" }.into();
    }
    const TOP: f64 = 17179869184.0;
    let packed = value >= 1.0e9 && value < TOP && value == value.trunc();
    if kind == "arinc" || (kind == "num" && packed) {
        let (v, ssm) = if value < 0.0 || value >= TOP || value != value.trunc() {
            (value, 3)
        } else {
            let low = (value % 4294967296.0) as u32;
            (f32::from_bits(low) as f64, ((value / 4294967296.0).floor() as u64 & 3) as u8)
        };
        let shown = with_unit(to_fixed(v, decimals), unit);
        return match ssm {
            0 => format!("{shown} FW"),
            1 => format!("{shown} NCD"),
            2 => format!("{shown} FT"),
            _ => shown,
        };
    }
    if value.abs() >= 1e7 {
        return with_unit(to_fixed(value, 0), unit);
    }
    with_unit(to_fixed(value, decimals), unit)
}

fn field_row(f: &Field, m: &GroundModel) -> (Node, bool) {
    let has = m.vars.contains_key(&f.name);
    let row = Node::el("div", "study-field-row").child(span("fname", f.label.as_deref().filter(|l| !l.is_empty()).unwrap_or(&f.name)));
    if !has {
        return (row.child(span("fval missing", if m.preview { "--" } else { "not modelled" })), false);
    }
    let raw = m.vars.get(&f.name);
    if f.kind == "lamp" || f.kind == "flag" {
        let on = !matches!(raw, Some(Value::Number(n)) if n.as_f64() == Some(0.0));
        let text = if f.kind == "flag" { if on { f.flag_text.as_str() } else { "" } } else if on { "ON" } else { "OFF" };
        return (row.child(Node::el("span", "fval").child(Node::el("span", if on { "dot ok" } else { "dot" })).text(text)), true);
    }
    let v = match raw {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        _ => f64::NAN,
    };
    (row.child(span("fval", &format_value(v, &f.kind, f.decimals, &f.unit))), true)
}

pub fn render_groups(groups: &[Group], m: &GroundModel) -> Node {
    let mut wrap = Node::el("div", "study-groups");
    for g in groups {
        let rows: Vec<Node> = g.fields.iter().map(|f| field_row(f, m)).filter(|(_, has)| *has || !g.hide_missing || m.preview).map(|(r, _)| r).collect();
        if g.hide_missing && rows.is_empty() {
            continue;
        }
        wrap = wrap.child(Node::el("div", "box").child(Node::new("h2").style("font-size:15px;margin:0 0 8px").text(&g.title)).children(rows));
    }
    wrap
}

fn lp_label(m: &GroundModel, names: &BTreeMap<u32, String>, key: &str) -> String {
    let id = num(m.settings.get(key).map(|s| Value::String(s.clone())).as_ref()) as u32;
    if id == 0 {
        "none".into()
    } else {
        names.get(&id).cloned().unwrap_or_else(|| format!("preset {id}"))
    }
}

pub fn lp_section(m: &GroundModel) -> Node {
    let powered = m.var(LP_POWER) != 0.0;
    let names = m.lp_names();
    let mut list = Node::el("div", "lp-list");
    for id in 1..=LP_COUNT {
        let name = names.get(&id).cloned().unwrap_or_default();
        let mut row = Node::el("div", "lp-row").child(span("lp-n mono", &id.to_string()));
        if m.lp_editing == id {
            row = row.child(Node::new("input").attr("type", "text").attr("placeholder", &format!("Preset {id}")).attr("maxlength", "28").value(&name));
        } else {
            let text = if name.is_empty() { format!("Preset {id}") } else { name.clone() };
            row = row.child(button("lp-name", &text).class_if("dim", name.is_empty()).attr("type", "button"));
        }
        row = row
            .child(button("small", "Load").attr("type", "button").attr_if("disabled", !powered).act(&format!("gs:lpload:{id}")))
            .child(button("small grey", if m.lp_armed == id { "Overwrite?" } else { "Save" }).attr("type", "button").attr_if("disabled", !powered).act(&format!("gs:lpsave:{id}")));
        list = list.child(row);
    }
    let auto = if m.settings.get("LIGHT_PRESET_AUTOLOAD").map(String::as_str) == Some("1") {
        format!(
            "Autoload on power-up: day {}, dawn/dusk {}, night {} (Settings > Misc).",
            lp_label(m, &names, "LIGHT_PRESET_AUTOLOAD_DAY"),
            lp_label(m, &names, "LIGHT_PRESET_AUTOLOAD_DAWNDUSK"),
            lp_label(m, &names, "LIGHT_PRESET_AUTOLOAD_NIGHT")
        )
    } else {
        "Autoload on power-up is off (Settings > Misc).".into()
    };
    Node::el("div", "lp-panel")
        .child(label("section-title", "Interior lighting presets"))
        .child(label(
            "label dim",
            if powered {
                "Save the flight deck lighting as it is now, or load a saved set. The lights fade to it over a few seconds."
            } else {
                "The aircraft must be powered (AC bus 1) to load or save lighting presets."
            },
        ))
        .child(list)
        .child(label("label dim", &auto))
}

pub fn js_fixed(v: f64, d: usize) -> String {
    let exact = format!("{:.60}", v.abs());
    let (int, frac) = exact.split_once('.').unwrap();
    let mut digits: Vec<u8> = int.bytes().chain(frac.bytes().take(d)).map(|b| b - b'0').collect();
    if frac.as_bytes()[d] >= b'5' {
        let mut i = digits.len();
        loop {
            if i == 0 {
                digits.insert(0, 1);
                break;
            }
            i -= 1;
            if digits[i] == 9 {
                digits[i] = 0;
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    let n = digits.len() - d;
    let mut s: String = digits[..n].iter().map(|x| char::from(b'0' + x)).collect();
    if d > 0 {
        s.push('.');
        s.extend(digits[n..].iter().map(|x| char::from(b'0' + x)));
    }
    if v < 0.0 { format!("-{s}") } else { s }
}

fn js_num(v: f64) -> String {
    if v == 0.0 { "0".into() } else { format!("{v}") }
}

pub fn squircle(x: f64, y: f64, w: f64, h: f64, r: f64) -> String {
    let c = (r * 1.5).min(w / 2.0).min(h / 2.0);
    let p = 2.0 / 4.5;
    let f = |v: f64| v.max(0.0).powf(p);
    let mut pts = Vec::new();
    let mut arc = |cx: f64, cy: f64, sx: f64, sy: f64, from: f64, to: f64| {
        for i in 0..=10 {
            let t = from + ((to - from) * f64::from(i)) / 10.0;
            pts.push((cx + sx * c * f(t.cos()), cy + sy * c * f(t.sin())));
        }
    };
    let q = std::f64::consts::PI / 2.0;
    arc(x + w - c, y + c, 1.0, -1.0, q, 0.0);
    arc(x + w - c, y + h - c, 1.0, 1.0, 0.0, q);
    arc(x + c, y + h - c, -1.0, 1.0, q, 0.0);
    arc(x + c, y + c, -1.0, -1.0, 0.0, q);
    format!("M{}Z", pts.iter().map(|(a, b)| format!("{},{}", js_fixed(*a, 1), js_fixed(*b, 1))).collect::<Vec<_>>().join(" L"))
}

const GM_DOORS: [(&str, f64, f64); 18] = [
    ("M1L", -3.05, -28.85), ("M1R", 3.05, -28.85), ("M2L", -3.58, -18.65), ("M2R", 3.58, -18.65), ("M3L", -3.71, -2.44), ("M3R", 3.71, -2.44),
    ("M4L", -3.57, 9.66), ("M4R", 3.57, 9.66), ("M5L", -3.35, 18.6), ("M5R", 3.35, 18.6), ("U1L", -3.38, -14.2), ("U1R", 3.38, -14.2),
    ("U2L", -3.36, 5.21), ("U2R", 3.36, 5.21), ("U3L", -3.37, 14.1), ("U3R", 3.37, 14.1), ("CARGO_FWD", 3.4, -23.45), ("CARGO_AFT", 3.37, 13.03),
];
const GM_ENGINE_POS: [(f64, f64); 4] = [(-25.77, -1.14), (-14.87, -9.01), (14.87, -9.01), (25.76, -1.14)];

fn shape(t: &str, d: usize, a: &[(&str, String)]) -> mfd_tree::Shape {
    let mut m: BTreeMap<String, String> = a.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    m.insert("_d".into(), d.to_string());
    mfd_tree::Shape { t: t.into(), a: m }
}

pub fn gm_plan(m: &GroundModel) -> Node {
    let (w, h, s) = (640.0, 720.0, 7.6);
    let cx = w / 2.0;
    let top = 70.0 + 35.2 * s;
    let p = |x: f64, z: f64| (cx + x * s, top + z * s);
    let sil: Vec<(f64, f64)> = serde_json::from_str(include_str!("gm_sil.json")).expect("silhouette");
    let d: String = sil.iter().enumerate().map(|(i, (x, z))| {
        let (a, b) = p(*x, *z);
        format!("{}{},{}", if i > 0 { "L" } else { "M" }, js_fixed(a, 1), js_fixed(b, 1))
    }).collect::<String>() + "Z";
    let mut out = vec![shape("path", 0, &[("d", d), ("class", "gm-ac".into())])];
    let chip = |out: &mut Vec<mfd_tree::Shape>, x: f64, y: f64, label: &str, on: bool, moving: bool, act: &str| {
        let wd = (label.chars().count() as f64 * 7.4 + 16.0).max(38.0);
        let mut cls = "gm-chip".to_string();
        if on {
            cls.push_str(" on");
        }
        if moving {
            cls.push_str(" moving");
        }
        out.push(shape("g", 0, &[("class", cls), ("data-gm", act.into())]));
        out.push(shape("path", 1, &[("d", squircle(x - wd / 2.0, y - 12.0, wd, 24.0, 7.0))]));
        out.push(shape("text", 1, &[("x", js_num(x)), ("y", js_num(y + 4.5)), ("text-anchor", "middle".into())]));
    };
    let leader = |out: &mut Vec<mfd_tree::Shape>, x1: f64, y1: f64, x2: f64, y2: f64, on: bool| {
        out.push(shape("line", 0, &[("x1", js_num(x1)), ("y1", js_num(y1)), ("x2", js_num(x2)), ("y2", js_num(y2)), ("class", if on { "gm-lead on" } else { "gm-lead" }.into())]));
    };
    for (i, (name, x, z)) in GM_DOORS.iter().enumerate() {
        let open = m.var(&format!("INTERACTIVE POINT OPEN:{i}"));
        let (px, py) = p(*x, *z);
        let cargo = name.starts_with("CARGO");
        let upper = name.starts_with('U');
        let side = if *x < 0.0 { -1.0 } else { 1.0 };
        let off = if cargo { 150.0 } else if upper { 96.0 } else { 46.0 };
        let lx = px + side * off;
        let label = if cargo { if *name == "CARGO_FWD" { "FWD" } else { "AFT" } } else { name };
        leader(&mut out, px, py, lx - side * 18.0, py, open > 2.0);
        chip(&mut out, lx, py, label, open > 98.0, open > 2.0 && open <= 98.0, &format!("door:{name}"));
    }
    let bulk = m.state(&Spec::Places(&["belt_bulk"]));
    let (bx, by) = p(3.25, 15.92 + 3.2);
    let (l1, l2) = p(3.25, 15.92);
    leader(&mut out, l1, l2, bx + 150.0 - 18.0, by, bulk == 2);
    chip(&mut out, bx + 150.0, by, "BULK", bulk == 2, bulk == 1 || bulk == 3, "svc:hold_bulk");
    let svc: [(f64, f64, f64, f64, &str, Spec, &str); 8] = [
        (0.45, -29.2, 70.0, -34.0, "GPU", Spec::Svc("gpu"), "svc:gpu"),
        (-19.69, -6.1, -40.0, -40.0, "FUEL", Spec::Places(&["fuel_hydrant", "fuel_tanker"]), "svc:fuel"),
        (-4.0, -8.0, -118.0, 52.0, "PCA", Spec::Svc("pca"), "svc:pca"),
        (4.0, -5.0, 118.0, 44.0, "ASU", Spec::Svc("asu"), "svc:asu"),
        (3.58, -18.65, 236.0, -22.0, "CATERING", Spec::Places(&["cat_m2r"]), "svc:cat_main"),
        (3.38, -14.2, 236.0, 22.0, "CATERING UD", Spec::Places(&["udcat_u1r"]), "svc:cat_upper"),
        (1.5, 24.0, 60.0, 18.0, "WATER", Spec::Svc("water"), "svc:water"),
        (-1.2, 26.5, -60.0, 18.0, "LAV", Spec::Svc("lav"), "svc:lav"),
    ];
    for (x, z, dx, dz, label, spec, act) in &svc {
        let st = m.state(spec);
        let (px, py) = p(*x, *z);
        let (lx, ly) = (px + dx, py + dz);
        leader(&mut out, px, py, lx, ly, st == 2);
        chip(&mut out, lx, ly, label, st == 2, st == 1 || st == 3, act);
    }
    let jet = ["m1l", "m2l", "u1l"].iter().filter(|d| m.gse(&format!("jetway_{d}/state")) > 1.0).count();
    let (jx, jy) = p(-3.05, -28.85);
    leader(&mut out, jx, jy, jx - 120.0, jy - 30.0, jet > 0);
    chip(&mut out, jx - 150.0, jy - 30.0, &if jet > 1 { format!("JETWAY x{jet}") } else { "JETWAY".into() }, jet > 0, false, "jetway");
    for (e, (x, z)) in GM_ENGINE_POS.iter().enumerate() {
        let open = m.cowl(e);
        let (px, py) = p(*x, z - 3.2);
        leader(&mut out, px, py, px, py - 26.0, open > 0.98);
        chip(&mut out, px, py - 38.0, &format!("COWL {}", e + 1), open > 0.98, open > 0.02 && open <= 0.98, &format!("cowl:{}", e + 1));
    }
    let chocks = m.chocks();
    let (gx, gy) = p(0.0, 1.5);
    chip(&mut out, gx, gy, "CHOCKS", chocks == GM_CHOCKS.len(), chocks > 0 && chocks < GM_CHOCKS.len(), "chocks");
    let tug = m.var("PUSHBACK ATTACHED") > 0.5;
    let (tx, ty) = p(0.0, -35.2);
    chip(&mut out, tx, ty - 26.0, "TUG", tug, m.gse("tug/state") == 1.0, "tug");
    out.push(shape("text", 0, &[("x", js_num(w - 16.0)), ("y", js_num(h - 14.0)), ("text-anchor", "end".into()), ("class", "gm-hint".into())]));
    let mut n = Node::el("svg", "gm-svg");
    n.svgattr.insert("viewBox".into(), "0 0 640 720".into());
    n.shapes = out;
    n
}

fn gm_row(lbl: &str, st: u32, extra: Option<String>, act: &str) -> Node {
    let away = st == 2 || st == 1;
    let b = button("pc-btn gm-act", if away { "SEND AWAY" } else if st == 3 { "LEAVING" } else { "CALL" })
        .class_if("pc-go", !away && st != 3)
        .attr("type", "button")
        .attr_if("disabled", st == 3)
        .act(act);
    let rs = Node::el("span", "gm-rs").class_if("ok", st == 2).class_if("chg", st != 2 && st != 0).text(&extra.unwrap_or_else(|| state_text(st).to_string()));
    Node::el("div", "gm-row").child(Node::el("div", "gm-rl").child(Node::new("span").text(lbl)).child(rs)).child(b)
}

pub fn gm_list(m: &GroundModel) -> Node {
    let op = m.settings.get("xphfbw.gseOperator").cloned().unwrap_or_else(|| "0".into());
    let known = ["dnata", "swissport", "menzies"].contains(&op.as_str());
    let seg = Node::el("div", "gm-seg").children(GM_OPERATORS.iter().map(|(v, l)| button("pc-opt", l).class_if("on", op == *v || (*v == "0" && !known)).attr("type", "button").act(&format!("set:xphfbw.gseOperator={v}"))));
    let mut col = Node::el("div", "gm-list").child(label("gm-gh", "HANDLING AGENT")).child(seg);
    let chocks = m.chocks();
    let n = GM_CHOCKS.len();
    col = col.child(label("gm-gh", "ON THE AIRCRAFT")).child(gm_row(
        "Wheel chocks",
        if chocks == n { 2 } else { 0 },
        Some(if chocks > 0 { format!("{chocks} of {n} in place") } else { String::new() }),
        "gs:chocks",
    ));
    let opened: Vec<f64> = (0..GM_ENGINES).map(|e| m.cowl(e)).collect();
    let cowls = Node::el("div", "gm-seg").children(opened.iter().enumerate().map(|(e, o)| button("pc-opt", &format!("ENG {}", e + 1)).class_if("on", *o > 0.98).attr("type", "button").act(&format!("gs:cowl:{}", e + 1))));
    let moving = opened.iter().any(|o| *o > 0.02 && *o <= 0.98);
    let open_list: Vec<String> = opened.iter().enumerate().filter(|(_, o)| **o > 0.98).map(|(e, _)| (e + 1).to_string()).collect();
    let text = if moving {
        "Moving".to_string()
    } else if !open_list.is_empty() {
        format!("Open on engine {}", open_list.join(", "))
    } else {
        "Parked, engine stopped".into()
    };
    col = col.child(
        Node::el("div", "gm-row")
            .child(Node::el("div", "gm-rl").child(Node::new("span").text("Fan cowl doors")).child(Node::el("span", "gm-rs").class_if("chg", moving).class_if("ok", !moving && !open_list.is_empty()).text(&text)))
            .child(cowls),
    );
    for (title, rows) in GM_GROUPS.iter() {
        col = col.child(label("gm-gh", title));
        for (key, lbl, spec) in rows.iter() {
            col = col.child(gm_row(lbl, m.state(spec), None, &format!("gs:svc:{key}")));
        }
    }
    col.child(button("gm-link", "Special cargo, NOTOC and load planning in Flight \u{203a} Cargo").attr("type", "button").act("tab:cargo"))
}

pub fn gm_section(m: &GroundModel) -> Node {
    Node::el("div", "pc-card gm-card")
        .child(label("pc-h", "GROUND SERVICES"))
        .child(Node::el("div", "gm-grid").child(Node::el("div", "gm-plan").child(gm_plan(m))).child(gm_list(m)))
}

fn tile(text: &str, state: &str, loading: bool, act: &str) -> Node {
    Node::el("button", "tile").class_if("loading", loading).attr("type", "button").act(act).child(Node::new("span").text(text)).child(span("tile-state", state))
}

pub fn ground_content(m: &GroundModel) -> Node {
    let loading = m.var(PRESET_LOAD);
    let presets = Node::el("div", "tiles").children(AIRCRAFT_PRESETS.iter().map(|(id, l, what)| {
        let on = loading == f64::from(*id);
        tile(l, if on { "loading\u{2026}" } else { what }, on, &format!("gs:preset:{id}"))
    }));
    let pct = (m.var(PRESET_PROGRESS).clamp(0.0, 1.0) * 100.0).round();
    let bar = Node::el("div", "preset-bar").child(Node::el("div", "preset-fill").style(&format!("width: {pct}%;")));
    let mut expedite = Node::new("input").attr("type", "checkbox");
    if m.var(PRESET_EXPEDITE) != 0.0 {
        expedite = expedite.attr("checked", "");
    }
    let cancel = button("small grey", "Cancel").attr("type", "button").attr_if("disabled", loading == 0.0).act("gs:preset:0");
    Node::el("div", "page-main")
        .id("ground-content")
        .child(label("section-title", "Aircraft state"))
        .child(presets)
        .child(Node::el("div", "row").child(bar).child(Node::el("label", "row").style("margin:0").child(expedite).child(span("label", "Expedite"))).child(cancel))
        .child(label("label dim", "On the ground only, as in the flyPad."))
        .child(lp_section(m))
        .child(gm_section(m))
        .child(label("section-title", "Maintenance"))
        .child(Node::el("div", "tiles").child(tile("Service oxygen", "refill crew bottles", false, "study:serviceOxygen")).child(tile("Repack escape slides", "after a deployment", false, "gs:slides")))
        .child(label("label dim", "Passenger doors open only with the cabin depressurised (below 0.8 psi), as their handles do."))
        .child(label("section-title", "State"))
        .child(render_groups(&m.groups, m))
}

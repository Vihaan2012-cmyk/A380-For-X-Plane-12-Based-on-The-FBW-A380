use crate::fmt::group_thousands;
use crate::model::{jstr, path, EfbModel};
use crate::{button, label, span, Node};
use serde::Serialize;
use serde_json::Value;

fn el(tag: &str, classes: &str) -> Node {
    Node::el(tag, classes)
}

pub fn js_number(v: &Value) -> f64 {
    match v {
        Value::Null => f64::NAN,
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                0.0
            } else {
                t.parse().unwrap_or(f64::NAN)
            }
        }
        _ => f64::NAN,
    }
}

pub fn js_num(v: &Value, d: usize) -> String {
    if v.is_null() || v.as_str() == Some("") {
        return "--".to_string();
    }
    let x = js_number(v);
    if x.is_nan() {
        return "--".to_string();
    }
    let s = format!("{:.*}", d, x.abs());
    let (int, frac) = s.split_once('.').map_or((s.as_str(), None), |(i, f)| (i, Some(f)));
    let neg = x < 0.0 && s.chars().any(|c| c.is_ascii_digit() && c != '0');
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    out.push_str(&group_thousands(int));
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    out
}

pub fn js_hhmm(v: &Value) -> String {
    let m = (js_number(v) / 60.0 + 0.5).floor();
    if m.is_nan() {
        return "--".to_string();
    }
    let m = m as i64;
    format!("{:02}:{:02}", m.div_euclid(60), m.rem_euclid(60))
}

fn or_null<'a>(a: &'a Value, b: &'a Value) -> &'a Value {
    if a.is_null() {
        b
    } else {
        a
    }
}

fn arr(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(a) => a.iter().collect(),
        Value::Null | Value::Bool(false) => Vec::new(),
        Value::String(s) if s.is_empty() => Vec::new(),
        Value::Number(n) if n.as_f64() == Some(0.0) => Vec::new(),
        other => vec![other],
    }
}

fn s(v: &Value, k: &str) -> String {
    jstr(path(v, &[k]))
}

fn flight_level(g: &Value) -> Option<i64> {
    let x = (js_number(path(g, &["initial_altitude"])) / 100.0 + 0.5).floor();
    if x.is_nan() || x == 0.0 {
        None
    } else {
        Some(x as i64)
    }
}

fn kv(pairs: &[(&str, String)]) -> Node {
    let mut g = el("div", "kv");
    for (k, v) in pairs {
        g = g.child(Node::new("div").child(span("k", k)).child(span("v", v)));
    }
    g
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct SimbriefLoad {
    pub pax: f64,
    pub bags: f64,
    pub pax_weight_kg: f64,
    pub bag_weight_kg: f64,
    pub freight_kg: f64,
    pub block_fuel_kg: f64,
}

fn or_zero(x: f64) -> f64 {
    if x.is_nan() || x == 0.0 {
        0.0
    } else {
        x
    }
}

fn or_default(x: f64, d: f64) -> f64 {
    if x.is_nan() || x == 0.0 {
        d
    } else {
        x
    }
}

pub fn simbrief_load(ofp: &Value) -> SimbriefLoad {
    let w = path(ofp, &["weights"]);
    let unit = path(ofp, &["params", "units"]).as_str().unwrap_or("kgs");
    let kg = |v: &Value| {
        let x = js_number(v);
        if unit == "lbs" {
            x * 0.45359237
        } else {
            x
        }
    };
    SimbriefLoad {
        pax: or_zero(js_number(or_null(path(w, &["pax_count_actual"]), path(w, &["pax_count"])))),
        bags: or_zero(js_number(or_null(path(w, &["bag_count_actual"]), path(w, &["bag_count"])))),
        pax_weight_kg: or_default(kg(path(w, &["pax_weight"])), 84.0),
        bag_weight_kg: or_default(kg(path(w, &["bag_weight"])), 20.0),
        freight_kg: or_zero(kg(path(w, &["freight_added"]))),
        block_fuel_kg: or_zero(kg(path(ofp, &["fuel", "plan_ramp"]))),
    }
}

pub fn loaded_message(l: &SimbriefLoad) -> String {
    format!(
        "Loaded: {} passengers, {} bags, {} kg freight, {} kg fuel.",
        crate::fmt::js_round(l.pax),
        crate::fmt::js_round(l.bags),
        crate::fmt::js_round(l.freight_kg),
        crate::fmt::js_round(l.block_fuel_kg)
    )
}

#[derive(Clone, Debug, Default)]
pub struct FlightPlanView {
    pub loaded: String,
}

fn apt(side: &Value, dest: bool) -> Node {
    let icao = s(side, "icao_code");
    let rwy = s(side, "plan_rwy");
    el("div", if dest { "fp-apt dest" } else { "fp-apt" })
        .child(label("icao", if icao.is_empty() { "----" } else { &icao }))
        .child(label("name", &format!("{} / RWY {}", s(side, "name"), if rwy.is_empty() { "--" } else { &rwy })))
}

fn route_text(o: &Value, dst: &Value, g: &Value) -> String {
    format!("{}/{} {} {}/{}", s(o, "icao_code"), s(o, "plan_rwy"), s(g, "route"), s(dst, "icao_code"), s(dst, "plan_rwy"))
}

pub fn flight_plan_content(m: &EfbModel, view: &FlightPlanView) -> Node {
    let root = Node::new("div").id("fp-content");
    let Some(ofp) = &m.ofp else {
        return root.child(el("div", "box").child(span("label dim", "Enter your SimBrief user ID and fetch your latest flight plan.")));
    };
    let g = path(ofp, &["general"]);
    let o = path(ofp, &["origin"]);
    let dst = path(ofp, &["destination"]);
    let alts = arr(path(ofp, &["alternate"]));
    let alt = alts.first().copied().unwrap_or(&Value::Null);
    let f = path(ofp, &["fuel"]);
    let w = path(ofp, &["weights"]);
    let t = path(ofp, &["times"]);
    let unit = path(ofp, &["params", "units"]).as_str().filter(|u| !u.is_empty()).unwrap_or("kgs").to_string();
    let ci = path(g, &["costindex"]);
    let head = el("div", "box fp-head")
        .child(apt(o, false))
        .child(
            el("div", "fp-mid")
                .child(label("big", &format!("{}{}", s(g, "icao_airline"), s(g, "flight_number"))))
                .child(Node::new("div").text(&format!("{} NM / {} block", js_num(path(g, &["route_distance"]), 0), js_hhmm(path(t, &["est_block"])))))
                .child(Node::new("div").text(&format!(
                    "FL{} / CI {}",
                    flight_level(g).map_or("--".to_string(), |x| x.to_string()),
                    if ci.is_null() { "--".to_string() } else { jstr(ci) }
                ))),
        )
        .child(apt(dst, true));
    let n = |v: &Value| js_num(v, 0);
    let fuel = kv(&[
        ("Block", n(path(f, &["plan_ramp"]))),
        ("Taxi", n(path(f, &["taxi"]))),
        ("Trip", n(path(f, &["enroute_burn"]))),
        ("Contingency", n(path(f, &["contingency"]))),
        ("Alternate", n(path(f, &["alternate_burn"]))),
        ("Final reserve", n(path(f, &["reserve"]))),
        ("Extra", n(path(f, &["extra"]))),
        ("Landing", n(path(f, &["plan_landing"]))),
    ]);
    let alt_icao = s(alt, "icao_code");
    let weights = kv(&[
        ("Passengers", n(or_null(path(w, &["pax_count_actual"]), path(w, &["pax_count"])))),
        ("Cargo", n(path(w, &["cargo"]))),
        ("Payload", n(path(w, &["payload"]))),
        ("Est ZFW", n(path(w, &["est_zfw"]))),
        ("Est TOW", n(path(w, &["est_tow"]))),
        ("Est LW", n(path(w, &["est_ldw"]))),
        ("Max TOW", n(path(w, &["max_tow"]))),
        ("Alternate", if alt_icao.is_empty() { "--".to_string() } else { alt_icao }),
    ]);
    let fixes = arr(path(ofp, &["navlog", "fix"]));
    let mut table = el("table", "navlog");
    let mut hr = Node::new("tr");
    for (h, cl) in [("Fix", ""), ("Via", ""), ("Alt ft", "num"), ("Leg NM", "num"), ("Dist NM", "num"), ("ETE", "num"), ("Fuel on board", "num")] {
        hr = hr.child(el("th", cl).text(h));
    }
    table = table.child(hr);
    let mut dist = 0.0;
    for x in &fixes {
        let leg = js_number(path(x, &["distance"]));
        if !leg.is_nan() {
            dist += leg;
        }
        let mut tr = Node::new("tr");
        for (v, cl) in [
            (s(x, "ident"), ""),
            (s(x, "via_airway"), ""),
            (n(path(x, &["altitude_feet"])), "num"),
            (n(path(x, &["distance"])), "num"),
            (js_num(&Value::from(dist), 0), "num"),
            (js_hhmm(path(x, &["time_total"])), "num"),
            (n(path(x, &["fuel_plan_onboard"])), "num"),
        ] {
            tr = tr.child(el("td", cl).text(&v));
        }
        table = table.child(tr);
    }
    root.child(head)
        .child(label("section-title", "Route"))
        .child(el("div", "box fp-route").text(&route_text(o, dst, g)))
        .child(label("section-title", &format!("Fuel ({unit})")))
        .child(el("div", "box").child(fuel))
        .child(label("section-title", &format!("Weights ({unit})")))
        .child(el("div", "box").child(weights))
        .child(label("section-title", &format!("Navigation log / {} fixes", fixes.len())))
        .child(el("div", "box").child(table))
        .child(
            el("div", "row")
                .style("margin-top:6px")
                .child(button("", "Load passengers, cargo and fuel now").attr("type", "button"))
                .child(span("label dim", &view.loaded).style("margin-left:8px")),
        )
        .child(label("label dim", "The route goes into the FMS with INIT > CPNY F-PLN REQUEST on the MFD.").style("margin-top:4px"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn js_number_semantics() {
        assert_eq!(js_num(&json!(""), 0), "--");
        assert_eq!(js_num(&json!("12345"), 0), "12,345");
        assert_eq!(js_hhmm(&json!("")), "00:00");
        assert_eq!(js_hhmm(&Value::Null), "--");
        assert_eq!(js_hhmm(&json!("5430")), "01:31");
    }

    #[test]
    fn simbrief_load_converts_pounds() {
        let ofp = json!({ "params": { "units": "lbs" }, "weights": { "pax_count": "500", "bag_count": "480", "pax_weight": "190", "bag_weight": "", "freight_added": "2000" }, "fuel": { "plan_ramp": "300000" } });
        let l = simbrief_load(&ofp);
        assert_eq!(l.pax, 500.0);
        assert!((l.pax_weight_kg - 86.18255).abs() < 1e-3);
        assert_eq!(l.bag_weight_kg, 20.0);
        assert_eq!(loaded_message(&l), "Loaded: 500 passengers, 480 bags, 907 kg freight, 136078 kg fuel.");
    }
}

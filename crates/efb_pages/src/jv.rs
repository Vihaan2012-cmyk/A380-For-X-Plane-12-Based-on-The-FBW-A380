use crate::js::{js_fixed, num};
use crate::Node;
use mfd_tree::Shape;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub type Vars = Map<String, Value>;

pub static NULL: Value = Value::Null;

pub fn g<'a>(v: &'a Value, k: &str) -> &'a Value {
    v.get(k).unwrap_or(&NULL)
}

pub fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// `String(v[k])` with `undefined` as given.
pub fn sv(v: &Value, k: &str, undef: &str) -> String {
    match v.get(k) {
        None => undef.into(),
        Some(x) => jstring(x),
    }
}

pub fn jstring(x: &Value) -> String {
    match x {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => num(n.as_f64().unwrap_or(f64::NAN)),
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(|e| if e.is_null() { String::new() } else { jstring(e) }).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

/// `Number(v[k])`-ish read: numbers only, NaN otherwise.
pub fn f(v: &Value, k: &str) -> f64 {
    v.get(k).and_then(Value::as_f64).unwrap_or(f64::NAN)
}

pub fn fo(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(Value::as_f64)
}

pub fn a<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

pub fn b(v: &Value, k: &str) -> bool {
    truthy(g(v, k))
}

pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0 && !x.is_nan()),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

pub fn var(vars: &Vars, name: &str) -> Option<f64> {
    vars.get(name).and_then(Value::as_f64)
}

pub fn fx(v: f64, d: usize) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.abs() >= 1e21 {
        return num(v);
    }
    js_fixed(v, d)
}

pub fn with_unit(n: String, unit: &str) -> String {
    if unit.is_empty() { n } else { format!("{n} {unit}") }
}

pub fn is_packed(v: f64) -> bool {
    v.is_finite() && (1.0e9..17179869184.0).contains(&v) && v == v.trunc()
}

pub fn unpack_arinc(v: f64) -> (f64, u8) {
    if !v.is_finite() || v < 0.0 || v >= 17179869184.0 || v != v.trunc() {
        return (v, 3);
    }
    let low = (v % 4294967296.0) as u32;
    (f32::from_bits(low) as f64, ((v / 4294967296.0).floor() as u64 & 3) as u8)
}

pub fn format_value(value: Option<f64>, kind: &str, decimals: usize, unit: &str) -> String {
    let Some(value) = value.filter(|v| v.is_finite()) else { return "no reading".into() };
    if kind == "lamp" {
        return if value != 0.0 { "ON" } else { "OFF" }.into();
    }
    if kind == "arinc" || (kind == "num" && is_packed(value)) {
        let (v, ssm) = unpack_arinc(value);
        let shown = with_unit(fx(v, decimals), unit);
        return match ssm {
            0 => format!("{shown} FW"),
            1 => format!("{shown} NCD"),
            2 => format!("{shown} FT"),
            _ => shown,
        };
    }
    if value.abs() >= 1e7 {
        return with_unit(fx(value, 0), unit);
    }
    with_unit(fx(value, decimals), unit)
}

/// SVG shapes as `svgEl` builds them; `text` content rides in `_text` (the compare ignores `_` keys).
pub struct Svg {
    pub node: Node,
    pub depth: usize,
}

impl Svg {
    pub fn new(classes: &str, attrs: &[(&str, String)]) -> Self {
        let mut node = Node::el("svg", classes);
        for (k, v) in attrs {
            node.svgattr.insert((*k).into(), v.clone());
        }
        Svg { node, depth: 0 }
    }
    pub fn add(&mut self, t: &str, attrs: &[(&str, String)]) -> &mut Self {
        let mut m: BTreeMap<String, String> = attrs.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect();
        m.insert("_d".into(), self.depth.to_string());
        self.node.shapes.push(Shape { t: t.to_ascii_lowercase(), a: m });
        self
    }
    pub fn text(&mut self, attrs: &[(&str, String)], text: &str) -> &mut Self {
        self.add("text", attrs);
        self.node.shapes.last_mut().unwrap().a.insert("_text".into(), text.into());
        self
    }
    pub fn open(&mut self, t: &str, attrs: &[(&str, String)]) -> &mut Self {
        self.add(t, attrs);
        self.depth += 1;
        self
    }
    pub fn close(&mut self) -> &mut Self {
        self.depth -= 1;
        self
    }
    pub fn done(self) -> Node {
        self.node
    }
}

pub fn n(v: f64) -> String {
    num(v)
}

/// A number as Blink serialises it in an inline style (6 significant digits).
pub fn css_num(v: f64) -> String {
    if v == 0.0 || !v.is_finite() {
        return num(v);
    }
    let mag = v.abs().log10().floor() as i32;
    let d = (5 - mag).max(0) as usize;
    let s = format!("{:.*}", d, v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

/// A `Value` parsed with each object's key order kept in a hidden `__order` key (JS `Object.entries` order).
pub struct Ordered(pub Value);

impl<'de> serde::Deserialize<'de> for Ordered {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(OrderedVisitor).map(Ordered)
    }
}

struct OrderedVisitor;

impl<'de> serde::de::Visitor<'de> for OrderedVisitor {
    type Value = Value;
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("json")
    }
    fn visit_bool<E>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_i64<E>(self, v: i64) -> Result<Value, E> {
        Ok(Value::from(v))
    }
    fn visit_u64<E>(self, v: u64) -> Result<Value, E> {
        Ok(Value::from(v))
    }
    fn visit_f64<E>(self, v: f64) -> Result<Value, E> {
        Ok(serde_json::Number::from_f64(v).map(Value::Number).unwrap_or(Value::Null))
    }
    fn visit_str<E>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.into()))
    }
    fn visit_string<E>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }
    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_some<D: serde::Deserializer<'de>>(self, d: D) -> Result<Value, D::Error> {
        serde::Deserialize::deserialize(d).map(|o: Ordered| o.0)
    }
    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut s: A) -> Result<Value, A::Error> {
        let mut out = Vec::new();
        while let Some(Ordered(v)) = s.next_element()? {
            out.push(v);
        }
        Ok(Value::Array(out))
    }
    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<Value, A::Error> {
        let mut map = Map::new();
        let mut order = Vec::new();
        while let Some((k, Ordered(v))) = m.next_entry::<String, Ordered>()? {
            if !map.contains_key(&k) {
                order.push(Value::String(k.clone()));
            }
            map.insert(k, v);
        }
        if order.len() > 1 {
            map.insert("__order".into(), Value::Array(order));
        }
        Ok(Value::Object(map))
    }
}

pub fn parse_ordered(text: &str) -> Value {
    serde_json::from_str::<Ordered>(text).map(|o| o.0).unwrap_or(Value::Null)
}

/// `Object.entries(v)`: the parse order when known, else the map's.
pub fn entries(v: &Value) -> Vec<(&str, &Value)> {
    let Some(o) = v.as_object() else { return vec![] };
    if let Some(ord) = o.get("__order").and_then(Value::as_array) {
        return ord.iter().filter_map(|k| k.as_str()).filter_map(|k| o.get_key_value(k)).map(|(k, v)| (k.as_str(), v)).collect();
    }
    o.iter().filter(|(k, _)| *k != "__order").map(|(k, v)| (k.as_str(), v)).collect()
}

/// `a.localeCompare(b, undefined, { numeric: true })` for the ASCII names the EFB sorts.
pub fn numeric_cmp(x: &str, y: &str) -> std::cmp::Ordering {
    fn chunks(s: &str) -> Vec<(bool, String)> {
        let mut out: Vec<(bool, String)> = Vec::new();
        for c in s.chars() {
            let d = c.is_ascii_digit();
            match out.last_mut() {
                Some((k, t)) if *k == d => t.push(c),
                _ => out.push((d, c.to_string())),
            }
        }
        out
    }
    let (cx, cy) = (chunks(x), chunks(y));
    for (p, q) in cx.iter().zip(cy.iter()) {
        let o = if p.0 && q.0 {
            let (a, b) = (p.1.trim_start_matches('0'), q.1.trim_start_matches('0'));
            a.len().cmp(&b.len()).then_with(|| a.cmp(b))
        } else {
            crate::js::locale_cmp(&p.1, &q.1)
        };
        if o != std::cmp::Ordering::Equal {
            return o;
        }
    }
    cx.len().cmp(&cy.len()).then_with(|| crate::js::locale_cmp(x, y))
}

/// `Number.prototype.toExponential(d)`.
pub fn to_exp(v: f64, d: usize) -> String {
    let s = format!("{:.*e}", d, v);
    let (m, e) = s.split_once('e').unwrap();
    let e: i32 = e.parse().unwrap();
    format!("{m}e{}{}", if e >= 0 { "+" } else { "-" }, e.abs())
}

/// `new Date(ms).toLocaleString('en-US')` at a fixed UTC offset in minutes.
pub fn locale_datetime(unix: f64, offset_min: i64) -> String {
    let t = unix.floor() as i64 + offset_min * 60;
    let days = t.div_euclid(86400);
    let sec = t.rem_euclid(86400);
    let (y, m, d) = civil(days);
    let (h, mi, s) = (sec / 3600, sec % 3600 / 60, sec % 60);
    let h12 = if h % 12 == 0 { 12 } else { h % 12 };
    format!("{m}/{d}/{y}, {h12}:{mi:02}:{s:02} {}", if h < 12 { "AM" } else { "PM" })
}

pub fn civil(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `Number(v[k])` as arithmetic sees it: null is 0, missing is NaN.
pub fn nf(v: &Value, k: &str) -> f64 {
    match v.get(k) {
        None => f64::NAN,
        Some(Value::Null) => 0.0,
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() { 0.0 } else { t.parse().unwrap_or(f64::NAN) }
        }
        Some(_) => f64::NAN,
    }
}

pub fn clamp01(x: f64) -> f64 {
    if x.is_nan() { x } else { x.min(1.0).max(0.0) }
}

/// `new Date(ms).toLocaleTimeString('en-US')` at a fixed UTC offset in minutes.
pub fn locale_time(unix: f64, offset_min: i64) -> String {
    let full = locale_datetime(unix, offset_min);
    full.split_once(", ").map(|x| x.1.to_string()).unwrap_or(full)
}

use std::collections::BTreeMap;

use serde_json::{Map, Number, Value};

pub const SECTION: &str = "vars";
pub const VERSION: u32 = 1;

pub fn encode_number(v: f64) -> Value {
    if v.is_nan() {
        Value::from("nan")
    } else if v == f64::INFINITY {
        Value::from("inf")
    } else if v == f64::NEG_INFINITY {
        Value::from("-inf")
    } else {
        Number::from_f64(v).map(Value::Number).unwrap_or(Value::Null)
    }
}

pub fn decode_number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => match s.as_str() {
            "nan" => Some(f64::NAN),
            "inf" => Some(f64::INFINITY),
            "-inf" => Some(f64::NEG_INFINITY),
            _ => None,
        },
        _ => None,
    }
}

/// Takes `(named, name, value)` as `Vars::snapshot_kinds` yields it.
pub fn save<'a>(entries: impl IntoIterator<Item = (bool, &'a str, f64)>) -> Value {
    let (mut named, mut sim) = (Map::new(), Map::new());
    for (is_named, name, value) in entries {
        let target = if is_named { &mut named } else { &mut sim };
        target.insert(name.to_string(), encode_number(value));
    }
    serde_json::json!({ "named": named, "sim": sim })
}

#[derive(Clone, Debug, Default)]
pub struct Filter {
    pub include_sim: bool,
    pub skip_exact: Vec<String>,
    pub skip_prefixes: Vec<String>,
    pub skip_contains: Vec<String>,
}

impl Filter {
    pub fn named_only() -> Self {
        Self::default()
    }

    pub fn allows(&self, is_named: bool, name: &str) -> bool {
        if !is_named && !self.include_sim {
            return false;
        }
        if crate::secrets::is_denied(name) {
            return false;
        }
        !(self.skip_exact.iter().any(|s| s == name)
            || self.skip_prefixes.iter().any(|p| name.starts_with(p.as_str()))
            || self.skip_contains.iter().any(|c| name.contains(c.as_str())))
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Loaded {
    pub write: Vec<(bool, String, f64)>,
    pub skipped: usize,
    pub unreadable: Vec<String>,
}

pub fn load(data: &Value, filter: &Filter) -> Loaded {
    let mut out = Loaded::default();
    for (key, is_named) in [("sim", false), ("named", true)] {
        let Some(map) = data.get(key).and_then(Value::as_object) else { continue };
        for (name, v) in map {
            if !filter.allows(is_named, name) {
                out.skipped += 1;
                continue;
            }
            match decode_number(v) {
                Some(x) => out.write.push((is_named, name.clone(), x)),
                None => out.unreadable.push(name.clone()),
            }
        }
    }
    out
}

/// Names whose value differs between two saves, for checking a load landed.
pub fn differences(a: &Value, b: &Value, tolerance: f64) -> BTreeMap<String, (Option<f64>, Option<f64>)> {
    let mut out = BTreeMap::new();
    for key in ["sim", "named"] {
        let empty = Map::new();
        let ma = a.get(key).and_then(Value::as_object).unwrap_or(&empty);
        let mb = b.get(key).and_then(Value::as_object).unwrap_or(&empty);
        for name in ma.keys().chain(mb.keys()) {
            let x = ma.get(name).and_then(decode_number);
            let y = mb.get(name).and_then(decode_number);
            let same = match (x, y) {
                (Some(x), Some(y)) => (x.is_nan() && y.is_nan()) || x == y || (x - y).abs() <= tolerance,
                (None, None) => true,
                _ => false,
            };
            if !same {
                out.insert(format!("{key}:{name}"), (x, y));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<(bool, String, f64)> {
        vec![
            (true, "A32NX_OVHD_ELEC_BAT_1_PB_IS_AUTO".into(), 1.0),
            (true, "A32NX_ADIRS_ADIRU_1_STATE".into(), 2.0),
            (true, "A32NX_ADIRS_ADR_1_ALTITUDE".into(), f64::NAN),
            (true, "A32NX_SOME_INF".into(), f64::NEG_INFINITY),
            (false, "AIRSPEED INDICATED".into(), 250.5),
        ]
    }

    #[test]
    fn values_including_nan_and_infinity_survive_a_round_trip_through_text() {
        let e = entries();
        let saved = save(e.iter().map(|(n, s, v)| (*n, s.as_str(), *v)));
        let text = serde_json::to_string(&saved).unwrap();
        let back: Value = serde_json::from_str(&text).unwrap();
        let filter = Filter { include_sim: true, ..Default::default() };
        let mut loaded = load(&back, &filter).write;
        loaded.sort_by(|a, b| a.1.cmp(&b.1));
        let mut expected = e.clone();
        expected.sort_by(|a, b| a.1.cmp(&b.1));
        assert_eq!(loaded.len(), expected.len());
        for (l, x) in loaded.iter().zip(expected.iter()) {
            assert_eq!((l.0, &l.1), (x.0, &x.1));
            assert!(l.2.to_bits() == x.2.to_bits() || (l.2.is_nan() && x.2.is_nan()));
        }
        assert!(differences(&saved, &back, 0.0).is_empty());
    }

    #[test]
    fn simulator_variables_are_left_to_x_plane_by_default_and_filters_apply() {
        let e = entries();
        let saved = save(e.iter().map(|(n, s, v)| (*n, s.as_str(), *v)));
        let filter = Filter { skip_prefixes: vec!["A32NX_ADIRS_ADR_".into()], skip_exact: vec!["A32NX_SOME_INF".into()], ..Filter::named_only() };
        let loaded = load(&saved, &filter);
        let names: Vec<_> = loaded.write.iter().map(|w| w.1.as_str()).collect();
        assert_eq!(names, ["A32NX_ADIRS_ADIRU_1_STATE", "A32NX_OVHD_ELEC_BAT_1_PB_IS_AUTO"]);
        assert_eq!(loaded.skipped, 3);
    }

    #[test]
    fn differences_name_what_moved() {
        let a = save([(true, "A", 1.0), (true, "B", 2.0)]);
        let b = save([(true, "A", 1.0), (true, "B", 2.5), (false, "C D", 3.0)]);
        let d = differences(&a, &b, 0.1);
        assert_eq!(d.keys().cloned().collect::<Vec<_>>(), ["named:B", "sim:C D"]);
    }
}

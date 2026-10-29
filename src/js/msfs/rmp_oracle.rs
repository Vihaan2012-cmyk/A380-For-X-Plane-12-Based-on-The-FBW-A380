use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::{json, Value};

use super::fms_offline::{new_cockpit, power_up_on_ground, VarStore, EGLL_LAT, EGLL_LON};
use super::{json_string, Cockpit};
use crate::js::{CallReply, Host, LogLevel};

const STEP_MS: f64 = 50.;

const LEAVES_JS: &str = r#"(() => {
  const docs = [...globalThis.__dom.documents.values()];
  const d = docs[0];
  if (!d) return '[]';
  const shown = (el) => {
    for (let n = el; n && n.nodeType === 1; n = n.parentNode) {
      let cs = null;
      try { cs = getComputedStyle(n); } catch (e) { return false; }
      if (!cs || cs.display === 'none' || cs.visibility === 'hidden') return false;
    }
    return true;
  };
  const out = [];
  for (const el of d.querySelectorAll('*')) {
    if (el.localName === 'style' || el.localName === 'script') continue;
    if (el.children && el.children.length > 0) continue;
    const t = String(el.textContent || '').replace(/\s+/g, ' ').trim();
    if (!t) continue;
    if (!shown(el)) continue;
    const r = el.getBoundingClientRect();
    const c = el.className;
    out.push([t, Math.round(r.left), Math.round(r.top), String(c && c.baseVal !== undefined ? c.baseVal : c || '')]);
  }
  return JSON.stringify(out);
})()"#;

const RELEVANT: [&str; 6] = ["RMP", "VHF", "TRANSPONDER", "FBW_HF", "FBW_TEL", "FBW_AMU"];

pub struct RmpHost {
    vars: VarStore,
    pub now_ms: f64,
    pub writes: Vec<Value>,
    pub keys: Vec<Value>,
    pub errors: Vec<String>,
}

impl RmpHost {
    fn new() -> Self {
        Self { vars: VarStore::new(), now_ms: 0., writes: Vec::new(), keys: Vec::new(), errors: Vec::new() }
    }

    fn put(&mut self, name: &str, value: f64) {
        if name.starts_with("L:") {
            self.vars.vars.insert(name.to_string(), value);
        } else {
            let bare = name.strip_prefix("A:").unwrap_or(name);
            self.vars.vars.insert(bare.to_string(), value);
            self.vars.vars.insert(format!("A:{bare}"), value);
        }
    }
}

impl Host for RmpHost {
    fn get_var(&mut self, name: &str, unit: &str) -> f64 {
        let offset = match name {
            "E:SIMULATION TIME" | "E:ABSOLUTE TIME" => self.now_ms / 1000.,
            _ => 0.,
        };
        self.vars.get_var(name, unit) + offset
    }

    fn set_var(&mut self, name: &str, unit: &str, value: f64) {
        let old = self.vars.vars.get(name).copied().unwrap_or(0.);
        if old.to_bits() != value.to_bits() && RELEVANT.iter().any(|r| name.contains(r)) {
            self.writes.push(json!([name.strip_prefix("A:").unwrap_or(name), value]));
        }
        self.vars.set_var(name, unit, value)
    }

    fn log(&mut self, level: LogLevel, message: &str) {
        if std::env::var("RMP_ORACLE_LOG").is_ok() && (message.contains("VHF") || message.contains("RMP") || level == LogLevel::Error) {
            println!("LOG {:?} {}", level, message.chars().take(200).collect::<String>());
        }
        if level == LogLevel::Error {
            self.errors.push(message.lines().take(3).collect::<Vec<_>>().join(" | ").chars().take(300).collect());
        }
    }

    fn get_string(&mut self, name: &str) -> String {
        self.vars.get_string(name)
    }

    fn set_string(&mut self, name: &str, value: &str) {
        match value.parse::<f64>() {
            Ok(number) if name.starts_with("L:") => self.set_var(name, "number", number),
            _ => self.vars.set_string(name, value),
        }
    }

    fn call(&mut self, name: &str, _args_json: &str) -> CallReply {
        if name == "fetch" {
            return CallReply::Resolved(json!({ "status": 404, "body": "" }).to_string());
        }
        CallReply::Resolved("null".to_string())
    }

    fn send_event(&mut self, name: &str, values: &[f64]) {
        if let Some(key) = name.strip_prefix("K:") {
            let key = match key.split_once(':') {
                Some((n, rest)) if n.chars().all(|c| c.is_ascii_digit()) => rest,
                _ => key,
            };
            self.keys.push(json!([key, values]));
            let arg = values.first().copied().unwrap_or(0.);
            if key == "ELECTRICAL_CIRCUIT_TOGGLE" {
                let var = format!("CIRCUIT SWITCH ON:{}", arg as i64);
                let on = self.vars.vars.get(&var).copied().unwrap_or(0.) > 0.;
                self.put(&var, if on { 0. } else { 1. });
            } else if let Some(n) = key.strip_prefix("COM").and_then(|k| k.strip_suffix("_RADIO_SET_HZ")) {
                let index: u32 = n.parse().unwrap_or(1);
                let digits = format!("{}", (arg / 100.).round() as u64);
                self.put(&format!("COM ACTIVE FREQUENCY:{index}"), u64::from_str_radix(&digits, 16).unwrap_or(0) as f64);
            } else if key == "COM_RADIO_SET_HZ" {
                let digits = format!("{}", (arg / 100.).round() as u64);
                self.put("COM ACTIVE FREQUENCY:1", u64::from_str_radix(&digits, 16).unwrap_or(0) as f64);
            }
        }
        self.vars.set_var(name, "number", values.first().copied().unwrap_or(0.));
    }

    fn set_now(&mut self, ms: f64) {
        self.now_ms = ms;
    }
}

fn pkg_dir() -> PathBuf {
    PathBuf::from(std::env::var("RMP_ORACLE_PKG").unwrap_or_else(|_| r"E:\fbw-build\rmp-oracle-pkg".into()))
}

fn rmp_view(cockpit: &Cockpit, index: usize) -> Option<String> {
    let needle = format!("RMP/rmp.html?Index={index}");
    cockpit.stats().into_iter().find(|s| s.gauges.iter().any(|g| g.contains(&needle))).map(|s| s.name)
}

fn leaves(cockpit: &Cockpit, view: &str) -> Value {
    let raw = cockpit.eval_in(view, LEAVES_JS).unwrap_or_default();
    serde_json::from_str(&raw).unwrap_or(Value::Null)
}

fn failure_data(ids: &[i64]) -> String {
    let package = json!({ "packagedId": 1, "data": [{ "topic": "FBW_FAILURE_UPDATE", "data": ids }] });
    format!("[\"JS_LISTENER_GENERICDATA\",\"EB_EVENTS\",{}]", json_string(&package.to_string()))
}

fn base_inputs(host: &mut RmpHost) {
    power_up_on_ground(host, EGLL_LAT, EGLL_LON);
    for (name, value) in [
        ("L:A380X_RMP_1_BRIGHTNESS_KNOB", 50.),
        ("L:A380X_RMP_2_BRIGHTNESS_KNOB", 50.),
        ("L:A380X_RMP_3_BRIGHTNESS_KNOB", 50.),
        ("COM ACTIVE FREQUENCY:1", 0x1189500 as f64),
        ("COM STANDBY FREQUENCY:1", 0x1219000 as f64),
        ("COM ACTIVE FREQUENCY:2", 0x1262000 as f64),
        ("COM STANDBY FREQUENCY:2", 0x1215000 as f64),
        ("TRANSPONDER CODE:1", 0x2000 as f64),
    ] {
        host.put(name, value);
    }
}

pub fn expand(scenario: &Value) -> Result<Vec<Vec<Value>>, String> {
    let steps = scenario["steps"].as_array().ok_or("scenario steps")?;
    let mut ticks: Vec<Vec<Value>> = vec![Vec::new()];
    let advance = |ticks: &mut Vec<Vec<Value>>, ms: f64| {
        for _ in 0..(ms / STEP_MS).ceil() as usize {
            ticks.push(Vec::new());
        }
    };
    for step in steps {
        if let Some(ms) = step.get("run_ms").and_then(Value::as_f64) {
            advance(&mut ticks, ms);
        } else if let Some(name) = step.get("press").and_then(Value::as_str) {
            let hold = step.get("hold_ms").and_then(Value::as_f64).unwrap_or(STEP_MS);
            ticks.last_mut().unwrap().push(json!({ "h": format!("{name}_PRESSED") }));
            advance(&mut ticks, hold);
            ticks.last_mut().unwrap().push(json!({ "h": format!("{name}_RELEASED") }));
            advance(&mut ticks, STEP_MS);
        } else if step.get("h").is_some() || step.get("set").is_some() || step.get("key").is_some() || step.get("failures").is_some() {
            ticks.last_mut().unwrap().push(step.clone());
        } else {
            return Err(format!("unknown step {step}"));
        }
    }
    Ok(ticks)
}

pub fn run_scenario(scenario: &Value) -> Result<Value, String> {
    let mut cockpit = new_cockpit(&pkg_dir())?;
    let mut host = RmpHost::new();
    base_inputs(&mut host);
    let ticks = expand(scenario)?;
    let boot = scenario.get("boot_ms").and_then(Value::as_f64).unwrap_or(4000.);
    for _ in 0..(boot / STEP_MS) as usize {
        let now = host.now_ms;
        cockpit.tick(&mut host, now);
        host.now_ms += STEP_MS;
    }
    let boot_writes = std::mem::take(&mut host.writes);
    let boot_keys = std::mem::take(&mut host.keys);
    let names: Vec<Option<String>> = (1..=3).map(|i| rmp_view(&cockpit, i)).collect();
    let mut out_ticks = Vec::new();
    let mut last_view = vec![Value::Null; 3];
    for inputs in &ticks {
        for input in inputs {
            if let Some(set) = input.get("set").and_then(Value::as_object) {
                for (name, value) in set {
                    host.put(name, value.as_f64().ok_or_else(|| format!("set {name}: not a number"))?);
                }
            }
            if let Some(h) = input.get("h").and_then(Value::as_str) {
                cockpit.h_event(&format!("H:{h}"));
            }
            if let Some(key) = input.get("key") {
                let name = key["name"].as_str().ok_or("key name")?;
                let v: Vec<f64> = key["values"].as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
                cockpit.key_event(name, [v.first().copied().unwrap_or(0.), v.get(1).copied().unwrap_or(0.), v.get(2).copied().unwrap_or(0.)]);
            }
            if let Some(ids) = input.get("failures").and_then(Value::as_array) {
                let ids: Vec<i64> = ids.iter().filter_map(Value::as_i64).collect();
                cockpit.broadcast("EVENT_FROM_VIEW_LISTENER", &failure_data(&ids));
            }
        }
        let now = host.now_ms;
        cockpit.tick(&mut host, now);
        let mut view = Vec::new();
        for (i, name) in names.iter().enumerate() {
            let state = name.as_ref().map(|n| leaves(&cockpit, n)).unwrap_or(Value::Null);
            view.push(if state == last_view[i] { json!("same") } else { state.clone() });
            last_view[i] = state;
        }
        out_ticks.push(json!({
            "t": host.now_ms,
            "inputs": inputs,
            "writes": std::mem::take(&mut host.writes),
            "keys": std::mem::take(&mut host.keys),
            "view": view,
        }));
        host.now_ms += STEP_MS;
    }
    Ok(json!({
        "scenario": scenario["name"],
        "boot_ms": boot,
        "boot_writes": boot_writes,
        "boot_keys": boot_keys,
        "errors": host.errors.iter().collect::<BTreeSet<_>>(),
        "ticks": out_ticks,
    }))
}

#[test]
#[ignore]
fn record_rmp_oracle() {
    let dir = PathBuf::from(std::env::var("RMP_ORACLE_SCENARIOS").unwrap_or_else(|_| "tools/rmp_oracle/scenarios".into()));
    let out = PathBuf::from(std::env::var("RMP_ORACLE_OUT").unwrap_or_else(|_| "E:/fbw-build/rmp-oracle/golden".into()));
    let only = std::env::var("RMP_ORACLE_ONLY").ok();
    std::fs::create_dir_all(&out).unwrap();
    let mut paths: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    paths.sort();
    for path in paths {
        let scenario: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let name = scenario["name"].as_str().unwrap().to_string();
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        let golden = run_scenario(&scenario).unwrap_or_else(|e| panic!("{name}: {e}"));
        let bytes = serde_json::to_string_pretty(&golden).unwrap();
        std::fs::write(out.join(format!("{name}.json")), &bytes).unwrap();
        let ticks = golden["ticks"].as_array().unwrap();
        let writes: usize = ticks.iter().map(|t| t["writes"].as_array().unwrap().len()).sum();
        let keys: usize = ticks.iter().map(|t| t["keys"].as_array().unwrap().len()).sum();
        println!("{name}: {} ticks, {writes} writes, {keys} keys, {} bytes, errors {}", ticks.len(), bytes.len(), golden["errors"]);
    }
}

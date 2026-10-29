use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use serde_json::json;

use super::fms_offline::{new_cockpit, VarStore};
use crate::js::{CallReply, Host, LogLevel};

const STEP_MS: f64 = 50.;

const FWS_OWNED_PREFIXES: [&str; 10] = [
    "A32NX_FWC_",
    "A32NX_FWS",
    "A32NX_MASTER_",
    "A32NX_ECAM_SFAIL",
    "A32NX_ECAM_FAILURE_ACTIVE",
    "A32NX_ECAM_SD_PAGE_TO_SHOW",
    "A32NX_STATUS_",
    "A32NX_TO_CONFIG",
    "A32NX_NO_SMOKING",
    "A32NX_CABIN_READY",
];

pub struct FwsHost {
    pub vars: VarStore,
    pub now_ms: f64,
    pub events: Vec<serde_json::Value>,
    last: HashMap<String, u64>,
    last_strings: HashMap<String, String>,
    last_bus: HashMap<String, String>,
    pub written: BTreeMap<String, f64>,
    pub written_strings: BTreeMap<String, String>,
    pub bus: BTreeMap<String, serde_json::Value>,
    pub errors: BTreeMap<String, usize>,
}

impl FwsHost {
    pub fn new() -> Self {
        let mut vars = VarStore::new();
        vars.trace_reads = true;
        Self {
            vars,
            now_ms: 0.,
            events: Vec::new(),
            last: HashMap::new(),
            last_strings: HashMap::new(),
            last_bus: HashMap::new(),
            written: BTreeMap::new(),
            written_strings: BTreeMap::new(),
            bus: BTreeMap::new(),
            errors: BTreeMap::new(),
        }
    }

    pub fn put(&mut self, name: &str, value: f64) {
        match name.strip_prefix("L:") {
            Some(_) => {
                self.vars.vars.insert(name.to_string(), value);
            }
            None => {
                let bare = name.strip_prefix("A:").unwrap_or(name);
                self.vars.vars.insert(bare.to_string(), value);
                self.vars.vars.insert(format!("A:{bare}"), value);
            }
        }
    }

    pub fn apply_state(&mut self, state: &serde_json::Value) -> Result<usize, String> {
        let list = state["vars"].as_array().ok_or("state without vars")?;
        let mut applied = 0;
        for item in list {
            let (Some(is_l), Some(name), Some(value)) = (item[0].as_bool(), item[1].as_str(), item[2].as_f64()) else {
                continue;
            };
            if is_l {
                if FWS_OWNED_PREFIXES.iter().any(|p| name.starts_with(p)) {
                    continue;
                }
                self.put(&format!("L:{name}"), value);
            } else {
                self.put(name, value);
            }
            applied += 1;
        }
        Ok(applied)
    }

    fn clock_offset(&self, name: &str) -> f64 {
        match name {
            "E:SIMULATION TIME" | "E:ABSOLUTE TIME" => self.now_ms / 1000.,
            _ => 0.,
        }
    }

    fn snapshot(&self, label: &str) -> serde_json::Value {
        json!({ "label": label, "t": self.now_ms, "vars": self.written, "strings": self.written_strings, "bus": self.bus })
    }
}

impl Host for FwsHost {
    fn get_var(&mut self, name: &str, unit: &str) -> f64 {
        self.vars.get_var(name, unit) + self.clock_offset(name)
    }

    fn set_var(&mut self, name: &str, unit: &str, value: f64) {
        if self.last.insert(name.to_string(), value.to_bits()) != Some(value.to_bits()) {
            self.events.push(json!({ "t": self.now_ms, "var": name, "value": value }));
        }
        self.written.insert(name.to_string(), value);
        self.vars.set_var(name, unit, value)
    }

    fn log(&mut self, level: LogLevel, message: &str) {
        if level == LogLevel::Error {
            let first: String = message.lines().take(3).collect::<Vec<_>>().join(" | ").chars().take(300).collect();
            *self.errors.entry(first).or_default() += 1;
        }
    }

    fn get_string(&mut self, name: &str) -> String {
        self.vars.get_string(name)
    }

    fn set_string(&mut self, name: &str, value: &str) {
        if self.last_strings.insert(name.to_string(), value.to_string()).as_deref() != Some(value) {
            self.events.push(json!({ "t": self.now_ms, "str": name, "value": value }));
        }
        self.written_strings.insert(name.to_string(), value.to_string());
        self.vars.set_string(name, value)
    }

    fn call(&mut self, name: &str, args_json: &str) -> CallReply {
        self.events.push(json!({ "t": self.now_ms, "call": name, "args": args_json }));
        if name == "fetch" {
            return CallReply::Resolved(json!({ "status": 404, "body": "" }).to_string());
        }
        if name == "GET_AIR_TRAFFIC" {
            return CallReply::Resolved("[]".to_string());
        }
        CallReply::Resolved("null".to_string())
    }

    fn trigger(&mut self, name: &str, args_json: &str) {
        self.events.push(json!({ "t": self.now_ms, "trigger": name, "args": args_json }));
    }

    fn send_event(&mut self, name: &str, values: &[f64]) {
        self.events.push(json!({ "t": self.now_ms, "key": name, "values": values }));
        self.vars.set_var(name, "number", values.first().copied().unwrap_or(0.));
    }

    fn generic_data(&mut self, key: &str, json: &str) {
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
        let items = match key {
            "xp-ui.bus" => parsed["events"].as_array().cloned().unwrap_or_default(),
            "EB_EVENTS" => parsed["data"].as_array().cloned().unwrap_or_default(),
            _ => Vec::new(),
        };
        if items.is_empty() {
            if self.last_bus.insert(key.to_string(), json.to_string()).as_deref() != Some(json) {
                self.events.push(json!({ "t": self.now_ms, "gd": key, "json": json }));
                self.bus.insert(key.to_string(), parsed);
            }
            return;
        }
        for item in items {
            let Some(topic) = item["topic"].as_str() else { continue };
            let data = item.get("data").cloned().unwrap_or(serde_json::Value::Null).to_string();
            if self.last_bus.insert(topic.to_string(), data.clone()).as_deref() != Some(data.as_str()) {
                self.events.push(json!({ "t": self.now_ms, "topic": topic, "data": item.get("data").cloned().unwrap_or(serde_json::Value::Null) }));
                self.bus.insert(topic.to_string(), item.get("data").cloned().unwrap_or(serde_json::Value::Null));
            }
        }
    }

    fn set_now(&mut self, ms: f64) {
        self.now_ms = ms;
    }
}

fn states_dir() -> PathBuf {
    PathBuf::from(std::env::var("FWS_ORACLE_STATES").unwrap_or_else(|_| r"E:\fbw-build\emu-snapshots".into()))
}

pub struct Outcome {
    pub golden: serde_json::Value,
    pub unset_reads: BTreeSet<String>,
}

pub fn run_scenario(scenario: &serde_json::Value) -> Result<Outcome, String> {
    let pkg = PathBuf::from(std::env::var("FWS_ORACLE_PKG").unwrap_or_else(|_| r"E:\fbw-build\fws-oracle-pkg".into()));
    let mut cockpit = new_cockpit(&pkg)?;
    let mut host = FwsHost::new();
    let steps = scenario["steps"].as_array().ok_or("scenario steps")?;
    let mut snapshots = Vec::new();
    let mut applied_states = Vec::new();
    let tick = |cockpit: &mut super::Cockpit, host: &mut FwsHost, n: usize| {
        for _ in 0..n {
            let now = host.now_ms;
            host.set_now(now);
            cockpit.tick(host, now);
            host.now_ms += STEP_MS;
        }
    };
    for step in steps {
        if let Some(name) = step.get("state").and_then(|v| v.as_str()) {
            let path = states_dir().join(format!("{name}.json"));
            let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let state: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{name}: {e}"))?;
            let n = host.apply_state(&state)?;
            applied_states.push(json!({ "state": name, "t": host.now_ms, "vars": n }));
        } else if let Some(set) = step.get("set").and_then(|v| v.as_object()) {
            for (name, value) in set {
                host.put(name, value.as_f64().ok_or_else(|| format!("set {name}: not a number"))?);
            }
        } else if let Some(ms) = step.get("run_ms").and_then(|v| v.as_f64()) {
            tick(&mut cockpit, &mut host, (ms / STEP_MS).ceil() as usize);
        } else if let Some(label) = step.get("snapshot").and_then(|v| v.as_str()) {
            snapshots.push(host.snapshot(label));
        } else {
            return Err(format!("unknown step {step}"));
        }
    }
    let unset_reads = host.vars.unset_reads.keys().cloned().collect();
    Ok(Outcome {
        golden: json!({
            "scenario": scenario["name"],
            "states": applied_states,
            "errors": host.errors,
            "snapshots": snapshots,
            "events": std::mem::take(&mut host.events),
        }),
        unset_reads,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_records_changes_once_and_stores_inputs_under_both_names() {
        let mut h = FwsHost::new();
        h.set_now(50.);
        h.set_var("L:A", "number", 1.);
        h.set_var("L:A", "number", 1.);
        h.generic_data("xp-ui.bus", r#"{"events":[{"topic":"fws_x","data":1}]}"#);
        h.generic_data("xp-ui.bus", r#"{"events":[{"origin":"o","topic":"fws_x","data":1}]}"#);
        h.generic_data("EB_EVENTS", r#"{"packagedId":1,"data":[{"topic":"fws_x","data":2}]}"#);
        h.put("PLANE ALTITUDE", 83.);
        h.put("L:B", 2.);
        assert_eq!(h.events.len(), 3);
        assert_eq!(h.get_var("A:PLANE ALTITUDE", "feet"), 83.);
        assert_eq!(h.get_var("PLANE ALTITUDE", "feet"), 83.);
        assert_eq!(h.get_var("L:B", "number"), 2.);
        assert_eq!(h.snapshot("s")["bus"]["fws_x"], 2);
    }

    #[test]
    fn applying_a_state_skips_the_variables_the_fws_writes() {
        let mut h = FwsHost::new();
        let state = json!({ "vars": [[true, "A32NX_FWC_FLIGHT_PHASE", 6], [true, "A32NX_FLAPS_HANDLE_INDEX", 2], [false, "GEAR HANDLE POSITION", 1]] });
        assert_eq!(h.apply_state(&state).unwrap(), 2);
        assert_eq!(h.get_var("L:A32NX_FWC_FLIGHT_PHASE", "number"), 0.);
        assert_eq!(h.get_var("L:A32NX_FLAPS_HANDLE_INDEX", "number"), 2.);
        assert_eq!(h.get_var("A:GEAR HANDLE POSITION", "bool"), 1.);
    }

    #[test]
    #[ignore]
    fn record_fws_oracle() {
        let dir = PathBuf::from(std::env::var("FWS_ORACLE_SCENARIOS").unwrap_or_else(|_| "tools/fws_oracle/scenarios".into()));
        let out = PathBuf::from(std::env::var("FWS_ORACLE_OUT").unwrap_or_else(|_| r"E:\fbw-build\fws-oracle".into()));
        let only = std::env::var("FWS_ORACLE_ONLY").ok();
        std::fs::create_dir_all(&out).unwrap();
        let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
        names.sort();
        for path in names {
            let scenario: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let name = scenario["name"].as_str().unwrap().to_string();
            if only.as_deref().is_some_and(|o| o != name) {
                continue;
            }
            let outcome = run_scenario(&scenario).unwrap_or_else(|e| panic!("{name}: {e}"));
            let bytes = serde_json::to_string_pretty(&outcome.golden).unwrap();
            std::fs::write(out.join(format!("{name}.json")), &bytes).unwrap();
            std::fs::write(out.join(format!("{name}.reads.json")), serde_json::to_string_pretty(&outcome.unset_reads).unwrap()).unwrap();
            let events = outcome.golden["events"].as_array().unwrap();
            let bus = events.iter().filter(|e| e.get("topic").is_some()).count();
            let vars = events.iter().filter(|e| e.get("var").is_some()).count();
            println!("{name}: {} events ({vars} var, {bus} bus), {} snapshots, {} bytes, errors {:?}", events.len(), outcome.golden["snapshots"].as_array().unwrap().len(), bytes.len(), outcome.golden["errors"]);
        }
    }
}

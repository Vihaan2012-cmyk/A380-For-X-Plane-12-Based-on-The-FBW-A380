use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Reply {
    Ok { id: u64 },
    Err { id: u64, message: String },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Restarted {
    pub count: u32,
    pub reason: String,
}

/// What one tick produced. `vars` are L: variables without the `L:` prefix and only
/// those whose value changed; `gd` are GENERIC_DATA topics (EFIS vectors and symbols);
/// `bus` are event bus topics; `keys` are K: events.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Outputs {
    pub vars: Vec<(String, f64)>,
    pub gd: Vec<(String, Value)>,
    pub bus: Vec<(String, Value)>,
    pub keys: Vec<(String, f64)>,
    pub replies: Vec<Reply>,
    pub messages: Vec<String>,
    pub restarted: Option<Restarted>,
}

impl Outputs {
    pub fn is_empty(&self) -> bool {
        self.vars.is_empty() && self.gd.is_empty() && self.bus.is_empty() && self.keys.is_empty() && self.replies.is_empty() && self.messages.is_empty() && self.restarted.is_none()
    }

    pub fn merge(&mut self, newer: Outputs) {
        self.vars.extend(newer.vars);
        self.gd.extend(newer.gd);
        self.bus.extend(newer.bus);
        self.keys.extend(newer.keys);
        self.replies.extend(newer.replies);
        self.messages.extend(newer.messages);
        if newer.restarted.is_some() {
            self.restarted = newer.restarted;
        }
    }

    pub fn last_var(&self, name: &str) -> Option<f64> {
        self.vars.iter().rev().find(|(n, _)| n == name).map(|(_, v)| *v)
    }

    pub fn last_gd(&self, topic: &str) -> Option<&Value> {
        self.gd.iter().rev().find(|(t, _)| t == topic).map(|(_, v)| v)
    }

    pub fn last_bus(&self, topic: &str) -> Option<&Value> {
        self.bus.iter().rev().find(|(t, _)| t == topic).map(|(_, v)| v)
    }
}

/// Collects one tick's writes. A var is written when its value differs from the last
/// written one, as FBW's subscriptions do; `set_always` mirrors FBW's unconditional writes
/// but still only reports changes.
#[derive(Clone, Debug, Default)]
pub struct Sink {
    pub written: BTreeMap<String, f64>,
    pub gd_state: BTreeMap<String, Value>,
    pub bus_state: BTreeMap<String, Value>,
    pub out: Outputs,
}

impl Sink {
    pub fn set(&mut self, name: &str, value: f64) {
        let changed = match self.written.get(name) {
            Some(old) => !(old == &value || (old.is_nan() && value.is_nan())),
            None => true,
        };
        if changed {
            self.written.insert(name.to_string(), value);
            self.out.vars.push((name.to_string(), value));
        }
    }

    pub fn set_bool(&mut self, name: &str, value: bool) {
        self.set(name, if value { 1. } else { 0. });
    }

    pub fn gd(&mut self, topic: &str, data: Value) {
        self.gd_state.insert(topic.to_string(), data.clone());
        self.out.gd.push((topic.to_string(), data));
    }

    pub fn bus(&mut self, topic: &str, data: Value) {
        self.bus_state.insert(topic.to_string(), data.clone());
        self.out.bus.push((topic.to_string(), data));
    }

    pub fn bus_if_changed(&mut self, topic: &str, data: Value) {
        if self.bus_state.get(topic) != Some(&data) {
            self.bus(topic, data);
        }
    }

    pub fn key(&mut self, name: &str, value: f64) {
        self.out.keys.push((name.to_string(), value));
    }

    pub fn take(&mut self) -> Outputs {
        std::mem::take(&mut self.out)
    }

    /// Every var, GD topic and cached bus topic last written, for a consumer that lost
    /// state (first tick after a restart).
    pub fn full_state(&self) -> Outputs {
        Outputs {
            vars: self.written.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            gd: self.gd_state.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            bus: self.bus_state.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            ..Outputs::default()
        }
    }
}

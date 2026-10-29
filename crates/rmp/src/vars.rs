use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub struct VarWrite {
    pub name: String,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyEvent {
    pub name: String,
    pub args: Vec<f64>,
}

#[derive(Default)]
pub struct Vars {
    values: HashMap<String, f64>,
    failures: HashSet<String>,
    pub writes: Vec<VarWrite>,
    pub keys: Vec<KeyEvent>,
}

impl Vars {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, name: &str) -> f64 {
        self.values.get(name).copied().unwrap_or(0.)
    }

    pub fn set(&mut self, name: &str, value: f64) {
        let old = self.values.insert(name.to_string(), value);
        if old.unwrap_or(0.).to_bits() != value.to_bits() {
            self.writes.push(VarWrite { name: name.to_string(), value });
        }
    }

    pub fn set_quiet(&mut self, name: &str, value: f64) {
        self.values.insert(name.to_string(), value);
    }

    pub fn key(&mut self, name: &str, args: &[f64]) {
        self.keys.push(KeyEvent { name: name.to_string(), args: args.to_vec() });
    }

    pub fn set_failure(&mut self, key: &str, active: bool) {
        if active {
            self.failures.insert(key.to_string());
        } else {
            self.failures.remove(key);
        }
    }

    pub fn failed(&self, key: &str) -> bool {
        self.failures.contains(key)
    }

    pub fn take_writes(&mut self) -> Vec<VarWrite> {
        std::mem::take(&mut self.writes)
    }

    pub fn take_keys(&mut self) -> Vec<KeyEvent> {
        std::mem::take(&mut self.keys)
    }
}

pub const SSM_FAILURE_WARNING: u32 = 0;
pub const SSM_NO_COMPUTED_DATA: u32 = 1;
pub const SSM_FUNCTIONAL_TEST: u32 = 2;
pub const SSM_NORMAL_OPERATION: u32 = 3;

pub fn arinc_raw(value: u32, ssm: u32) -> f64 {
    (value as f32).to_bits() as f64 + ssm as f64 * 4294967296.0
}

pub fn arinc_split(raw: f64) -> (u32, u32) {
    let ssm = (raw / 4294967296.0).floor() as u32;
    let low = (raw - ssm as f64 * 4294967296.0) as u32;
    (f32::from_bits(low) as u32, ssm)
}

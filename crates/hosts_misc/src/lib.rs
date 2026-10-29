pub mod baro_unit;
pub mod fws_host;
pub mod key_interceptor;
pub mod light_sync;
pub mod pilot_seat;
pub mod pushbutton_check;

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    SetLvar { name: String, value: f64 },
    KeyEvent { name: String, value: f64 },
    TriggerKey { key: String, bypass: bool, args: Vec<f64> },
    HEvent { name: String },
    HEventNextFrame { name: String },
    SetLvarAfter { name: String, value: f64, delay_ms: u64 },
}

impl Command {
    pub fn lvar(name: impl Into<String>, value: f64) -> Self {
        Command::SetLvar { name: name.into(), value }
    }

    pub fn flag(name: impl Into<String>, value: bool) -> Self {
        Command::SetLvar { name: name.into(), value: if value { 1. } else { 0. } }
    }

    pub fn trigger(key: &str, bypass: bool, args: &[f64]) -> Self {
        Command::TriggerKey { key: key.to_string(), bypass, args: args.to_vec() }
    }
}

pub fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

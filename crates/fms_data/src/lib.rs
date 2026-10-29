pub mod checks;
pub mod command;
pub mod data;
pub mod defaults;
pub mod entry;
pub mod limits;
pub mod service;

pub use checks::TakeoffChecks;
pub use command::{Command, Effect, Event, FmsMessage, WindPhase};
pub use data::{ClimbDerated, CostIndexMode, PerformanceData, TakeoffAntiIce, TakeoffDerated, TakeoffPacks, TakeoffPowerSetting};
pub use service::PerfDataStore;
pub use entry::{apply, Entry, EntryContext, EntryError, FlightPhase};

pub(crate) fn js_round(value: f64, quantum: f64) -> f64 {
    ((value / quantum) + 0.5).floor() * quantum
}

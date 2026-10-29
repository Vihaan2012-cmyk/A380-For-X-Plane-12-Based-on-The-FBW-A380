use std::collections::BTreeMap;

use fms::plan::FlightPlan;
use fms_data::PerformanceData;
use fms_host::Snapshot;

/// Everything the page models read: the FMS host's plans, performance data and phase,
/// variables by name, and the FMS messages waiting for the footer.
pub struct Sources<'a> {
    pub plans: &'a [Option<FlightPlan>],
    pub perf: &'a BTreeMap<usize, PerformanceData>,
    pub phase: u8,
    pub vars: &'a dyn Fn(&str) -> f64,
    pub messages: &'a [String],
}

pub const NO_VARS: &dyn Fn(&str) -> f64 = &|_| 0.;

impl<'a> Sources<'a> {
    pub fn from_snapshot(s: &'a Snapshot, vars: &'a dyn Fn(&str) -> f64, messages: &'a [String]) -> Sources<'a> {
        Sources { plans: &s.plans, perf: &s.perf, phase: s.phase, vars, messages }
    }

    pub fn var(&self, name: &str) -> f64 {
        let v = (self.vars)(name);
        if v.is_finite() {
            v
        } else {
            0.
        }
    }

    pub fn flag(&self, name: &str) -> bool {
        self.var(name) != 0.
    }

    pub fn plan(&self, index: usize) -> Option<&'a FlightPlan> {
        self.plans.get(index).and_then(|p| p.as_ref())
    }

    pub fn perf(&self, index: usize) -> Option<&'a PerformanceData> {
        self.perf.get(&index)
    }

    pub fn tmpy(&self) -> bool {
        self.plan(1).is_some()
    }
}

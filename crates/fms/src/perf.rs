use fms_data::checks::TakeoffSpeedBits;
use fms_data::command::{Command as PerfCommand, Effect, Event, WindPhase};
use fms_data::entry::{Entry, EntryContext, EntryError, FlightPhase};
use fms_data::PerformanceData;

use crate::plan::{FlightPlan, PerformanceData as PlanPerformance, CONSTRAINT_CLB};
use crate::service::{Fms, FLAG_COPIED_FROM_ACTIVE};
use crate::winds::{WindEntry, WindVector};

/// Aircraft inputs the performance data entry rules need.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PerfState {
    pub phase: FlightPhase,
    pub engines_started: bool,
    pub fcu_altitude_ft: Option<f64>,
    pub max_zfw_kg: Option<f64>,
    pub rec_max_flight_level: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PerfSeen {
    origin: Option<String>,
    destination: Option<String>,
    performance: PlanPerformance,
    approach: (Option<f64>, Option<f64>, bool),
}

macro_rules! plan_owned_fields {
    ($m:ident, $a:expr, $b:expr, $c:expr) => {
        $m!(
            $a, $b, $c;
            cost_index, cruise_flight_level, database_transition_altitude, database_transition_level, pilot_tropopause,
            zero_fuel_weight, block_fuel, pilot_thrust_reduction_altitude, pilot_acceleration_altitude,
            pilot_engine_out_acceleration_altitude, pilot_missed_thrust_reduction_altitude, pilot_missed_acceleration_altitude,
            pilot_missed_engine_out_acceleration_altitude, climb_speed_limit_speed, climb_speed_limit_altitude,
            descent_speed_limit_speed, descent_speed_limit_altitude, alternate_climb_speed_limit_speed,
            alternate_climb_speed_limit_altitude, alternate_descent_speed_limit_speed, alternate_descent_speed_limit_altitude,
            is_climb_speed_limit_pilot_entered, is_descent_speed_limit_pilot_entered,
            is_alternate_climb_speed_limit_pilot_entered, is_alternate_descent_speed_limit_pilot_entered, pilot_alternate_fuel
        )
    };
}

macro_rules! copy_changed {
    ($seen:expr, $plan:expr, $pd:expr; $($f:ident),*) => {
        $(if $plan.$f != $seen.$f { $pd.$f = $plan.$f; })*
    };
}

macro_rules! mirror {
    ($pd:expr, $plan:expr, $unused:expr; $($f:ident),*) => {
        $($plan.$f = $pd.$f;)*
    };
}

fn store_to_plan(pd: &PerformanceData, plan: &mut PlanPerformance) {
    plan_owned_fields!(mirror, pd, plan, ());
    plan.default_thrust_reduction_altitude = pd.default_thrust_reduction_altitude;
    plan.default_acceleration_altitude = pd.default_acceleration_altitude;
    plan.default_engine_out_acceleration_altitude = pd.default_engine_out_acceleration_altitude;
    plan.default_missed_thrust_reduction_altitude = pd.default_missed_thrust_reduction_altitude;
    plan.default_missed_acceleration_altitude = pd.default_missed_acceleration_altitude;
    plan.default_missed_engine_out_acceleration_altitude = pd.default_missed_engine_out_acceleration_altitude;
}

fn command_slot(c: &PerfCommand) -> usize {
    match c {
        PerfCommand::Enter { plan, .. }
        | PerfCommand::SetWind { plan, .. }
        | PerfCommand::DeleteWind { plan, .. }
        | PerfCommand::DeleteAllWinds { plan, .. }
        | PerfCommand::SetAlternateWind { plan, .. }
        | PerfCommand::OriginChanged { plan, .. }
        | PerfCommand::DestinationChanged { plan, .. }
        | PerfCommand::OriginRunwayChanged { plan, .. }
        | PerfCommand::ReconcileWithClimbConstraints { plan, .. }
        | PerfCommand::DefaultCostIndexIfMissing { plan } => *plan,
        PerfCommand::SecondaryActivated { .. } => 0,
    }
}

fn wind_entry(altitude: f64, true_direction_deg: Option<f64>, magnitude: Option<f64>) -> WindEntry {
    WindEntry { vector: WindVector { direction: true_direction_deg.map(f64::to_radians), magnitude }, altitude: Some(altitude), flags: 0 }
}

impl FlightPlan {
    /// FBW `BaseFlightPlan.lowestClimbConstraint` (feet, infinite when none).
    pub fn lowest_climb_constraint(&self) -> f64 {
        let mut lowest = f64::INFINITY;
        for i in 0..self.first_missed_approach_leg_index() {
            let Some(leg) = self.leg_at(i) else { continue };
            if leg.constraint_type != CONSTRAINT_CLB {
                continue;
            }
            let max = match leg.altitude_constraint() {
                Some(c) if matches!(c.descriptor.as_str(), "@" | "I" | "X" | "-" | "Y" | "B") => c.altitude1,
                _ => f64::INFINITY,
            };
            lowest = lowest.min(max);
        }
        lowest
    }
}

impl Fms {
    pub fn perf_data(&self, slot: usize) -> Option<&PerformanceData> {
        self.perf.get(slot)
    }

    pub fn perf_context(&self, slot: usize) -> EntryContext {
        let plan = self.plans.get(slot).and_then(|p| p.as_ref());
        let copy = slot >= 3 && plan.is_some_and(|p| p.flags & FLAG_COPIED_FROM_ACTIVE != 0);
        let s = self.perf_state;
        EntryContext {
            phase: s.phase,
            active_or_copy: slot <= 1 || copy,
            active: slot <= 1,
            engines_started: s.engines_started,
            has_destination: plan.is_some_and(|p| p.destination_airport.is_some()),
            destination_elevation_ft: plan.and_then(|p| p.destination_airport.as_ref()).and_then(|a| a.location.alt),
            fcu_altitude_ft: s.fcu_altitude_ft,
            max_zfw_kg: s.max_zfw_kg,
            rec_max_flight_level: s.rec_max_flight_level,
        }
    }

    pub(crate) fn forget_perf(&mut self, slot: usize) {
        self.perf.delete(slot);
        self.perf_seen[slot] = None;
    }

    pub(crate) fn copy_perf(&mut self, from: usize, to: usize) {
        self.perf.delete(to);
        self.perf.copy(from, to);
        self.perf_seen[to] = self.perf_seen[from].clone();
    }

    pub(crate) fn swap_perf(&mut self, a: usize, b: usize) {
        let (pa, pb) = (self.perf.plans.remove(&a), self.perf.plans.remove(&b));
        if let Some(p) = pa {
            self.perf.plans.insert(b, p);
        }
        if let Some(p) = pb {
            self.perf.plans.insert(a, p);
        }
        self.perf_seen.swap(a, b);
    }

    /// Brings the per-plan performance store and each plan's `performance` view into
    /// agreement after flight plan operations: origin/destination defaults, uplinked and
    /// revision-written fields, and the approach wind written by ground descent winds.
    pub fn sync_perf(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        for slot in 0..self.plans.len() {
            let Some(plan) = self.plans[slot].as_ref() else {
                self.forget_perf(slot);
                continue;
            };
            let seen = self.perf_seen[slot].clone().unwrap_or_default();
            let origin = plan.origin_airport.as_ref().map(|a| a.ident.clone());
            let destination = plan.destination_airport.as_ref().map(|a| a.ident.clone());
            let origin_data = plan.origin_airport.as_ref().map(|a| (a.location.alt, a.transition_altitude));
            let destination_data = plan.destination_airport.as_ref().map(|a| (a.location.alt, a.transition_level));
            let ctx = self.perf_context(slot);
            let mut slot_events = Vec::new();
            if origin != seen.origin {
                let (elevation_ft, transition_altitude_ft) = origin_data.unwrap_or((None, None));
                slot_events.extend(self.perf.execute(&ctx, PerfCommand::OriginChanged { plan: slot, elevation_ft, transition_altitude_ft }).unwrap_or_default());
            }
            if destination != seen.destination {
                if let Some((elevation_ft, transition_level)) = destination_data {
                    slot_events.extend(self.perf.execute(&ctx, PerfCommand::DestinationChanged { plan: slot, elevation_ft, transition_level }).unwrap_or_default());
                }
            }
            let plan = self.plans[slot].as_ref().expect("plan");
            let approach = (plan.winds.approach_wind_direction, plan.winds.approach_wind_magnitude, plan.winds.is_approach_wind_pilot_entered);
            let pd = self.perf.ensure(slot);
            plan_owned_fields!(copy_changed, seen.performance, plan.performance, pd);
            if approach != seen.approach {
                (pd.approach_wind_direction, pd.approach_wind_magnitude, pd.is_approach_wind_pilot_entered) = approach;
            }
            self.route_perf_effects(slot, &slot_events);
            let pd = self.perf.ensure(slot).clone();
            let plan = self.plans[slot].as_mut().expect("plan");
            store_to_plan(&pd, &mut plan.performance);
            plan.winds.approach_wind_direction = pd.approach_wind_direction;
            plan.winds.approach_wind_magnitude = pd.approach_wind_magnitude;
            plan.winds.is_approach_wind_pilot_entered = pd.is_approach_wind_pilot_entered;
            self.perf_seen[slot] = Some(PerfSeen {
                origin,
                destination,
                performance: plan.performance.clone(),
                approach: (pd.approach_wind_direction, pd.approach_wind_magnitude, pd.is_approach_wind_pilot_entered),
            });
            events.extend(slot_events);
        }
        events
    }

    fn route_perf_effects(&mut self, slot: usize, events: &[Event]) {
        for e in events {
            let Event::Effect(effect) = e else { continue };
            match effect {
                Effect::Wind(c) => self.route_wind(c),
                Effect::SetDescentGroundWind { magnetic_direction, magnitude } => {
                    if let Some(p) = self.plans[slot].as_mut() {
                        let mag_var = p.destination_airport.as_ref().and_then(|a| a.mag_var).unwrap_or(0.);
                        let true_dir = magnetic_direction.map(|d| (d + mag_var).rem_euclid(360.));
                        p.set_descent_wind_entry_ex(Some(0.), Some(wind_entry(0., true_dir, *magnitude)), false, false, mag_var);
                    }
                }
                Effect::DeleteDescentWindAt(alt) => {
                    if let Some(p) = self.plans[slot].as_mut() {
                        let mag_var = p.destination_airport.as_ref().and_then(|a| a.mag_var).unwrap_or(0.);
                        p.set_descent_wind_entry_ex(Some(*alt), None, false, false, mag_var);
                    }
                }
                Effect::DeleteDescentWinds => {
                    if let Some(p) = self.plans[slot].as_mut() {
                        p.delete_descent_wind_entries();
                    }
                }
                Effect::AirlinerFlexTemp(_) | Effect::CruiseLevelUpdated(_) => {}
            }
        }
    }

    fn route_wind(&mut self, c: &PerfCommand) {
        let slot = command_slot(c);
        let Some(p) = self.plans.get_mut(slot).and_then(|p| p.as_mut()) else { return };
        let mag_var = p.destination_airport.as_ref().and_then(|a| a.mag_var).unwrap_or(0.);
        match *c {
            PerfCommand::SetWind { phase, altitude, direction, magnitude, cruise_index, .. } => {
                let entry = wind_entry(altitude, Some(direction), Some(magnitude));
                match phase {
                    WindPhase::Climb => p.set_climb_wind_entry(Some(altitude), Some(entry)),
                    WindPhase::Descent => p.set_descent_wind_entry(Some(altitude), Some(entry), mag_var),
                    WindPhase::Cruise => {
                        if let Some(i) = cruise_index {
                            p.add_cruise_wind_entry(i, entry);
                        }
                    }
                }
            }
            PerfCommand::DeleteWind { phase, altitude, cruise_index, .. } => match phase {
                WindPhase::Climb => p.set_climb_wind_entry(Some(altitude), None),
                WindPhase::Descent => p.set_descent_wind_entry(Some(altitude), None, mag_var),
                WindPhase::Cruise => {
                    if let Some(i) = cruise_index {
                        p.delete_cruise_wind_entry(i, altitude);
                    }
                }
            },
            PerfCommand::DeleteAllWinds { phase, .. } => match phase {
                WindPhase::Climb => p.delete_climb_wind_entries(),
                WindPhase::Descent => p.delete_descent_wind_entries(),
                WindPhase::Cruise => p.delete_all_cruise_wind_entries(),
            },
            _ => {}
        }
    }

    /// Runs one performance data command (MFD page or AI F/O entry) against the plan's store.
    pub fn execute_perf(&mut self, command: PerfCommand) -> Result<Vec<Event>, EntryError> {
        self.sync_perf();
        let slot = command_slot(&command);
        let ctx = self.perf_context(slot);
        let mut events = self.perf.execute(&ctx, command.clone())?;
        match &command {
            PerfCommand::SetAlternateWind { plan, direction, magnitude } => {
                if let Some(p) = self.plans.get_mut(*plan).and_then(|p| p.as_mut()) {
                    p.set_alternate_wind(WindVector { direction: direction.map(f64::to_radians), magnitude: *magnitude });
                }
            }
            PerfCommand::Enter { entry: Entry::ZeroFuelWeightKg(Some(_)), .. } => {
                let ctx = self.perf_context(0);
                events.extend(self.perf.execute(&ctx, PerfCommand::DefaultCostIndexIfMissing { plan: 0 })?);
            }
            _ => {}
        }
        self.route_perf_effects(slot, &events);
        events.extend(self.sync_perf());
        Ok(events)
    }

    /// The periodic FMGC checks: thrust reduction / acceleration against climb constraints,
    /// CI 0 / LRC on engine start, V-speed checks. Returns discrete word 3 bits and events.
    pub fn perf_update(&mut self) -> (TakeoffSpeedBits, Vec<Event>) {
        let mut events = self.sync_perf();
        if let Some(lowest) = self.active().map(|p| p.lowest_climb_constraint()) {
            let ctx = self.perf_context(0);
            events.extend(self.perf.execute(&ctx, PerfCommand::ReconcileWithClimbConstraints { plan: 0, lowest_climb_constraint_ft: lowest }).unwrap_or_default());
        }
        if self.perf_state.engines_started && !self.perf_engines_were_started && self.active().is_some() {
            let ctx = self.perf_context(0);
            events.extend(self.perf.execute(&ctx, PerfCommand::DefaultCostIndexIfMissing { plan: 0 }).unwrap_or_default());
        }
        self.perf_engines_were_started = self.perf_state.engines_started;
        let (bits, ev) = self.perf.update();
        events.extend(ev);
        events.extend(self.sync_perf());
        (bits, events)
    }

    /// FBW `FlightPlanService.persistActivePerfDataAcrossModification` (A380 config) around an activation.
    pub(crate) fn persist_active_perf(&mut self, old: Option<PerformanceData>, old_flight_number: Option<String>) -> Vec<Event> {
        let Some(old) = old else { return Vec::new() };
        let before_engine_start = !self.perf_state.engines_started;
        let pd = self.perf.ensure(0);
        let new_zfw = pd.zero_fuel_weight;
        let new_cg = pd.zero_fuel_weight_center_of_gravity;
        if old.zero_fuel_weight.is_some() && (!before_engine_start || pd.zero_fuel_weight.is_none()) {
            pd.zero_fuel_weight = old.zero_fuel_weight;
        }
        if old.zero_fuel_weight_center_of_gravity.is_some() && (!before_engine_start || pd.zero_fuel_weight_center_of_gravity.is_none()) {
            pd.zero_fuel_weight_center_of_gravity = old.zero_fuel_weight_center_of_gravity;
        }
        if old.block_fuel.is_some() && (!before_engine_start || pd.block_fuel.is_none()) {
            pd.block_fuel = old.block_fuel;
        }
        if pd.pilot_taxi_fuel.is_none() {
            pd.pilot_taxi_fuel = Some(old.taxi_fuel());
        }
        if old.cost_index.is_some() && pd.cost_index.is_none() {
            pd.cost_index = old.cost_index;
        }
        if let (Some(n), Some(p)) = (old_flight_number, self.plans[0].as_mut()) {
            if p.flight_number.is_empty() {
                p.flight_number = n;
            }
        }
        let diff = |a: Option<f64>, b: Option<f64>| a.zip(b).map(|(a, b)| (a - b).abs());
        let ctx = self.perf_context(0);
        let command = PerfCommand::SecondaryActivated { zfw_diff_t: diff(old.zero_fuel_weight, new_zfw), zfw_cg_diff: diff(old.zero_fuel_weight_center_of_gravity, new_cg) };
        self.perf.execute(&ctx, command).unwrap_or_default()
    }

    pub(crate) fn pipe_active_perf_to(&mut self, slot: usize) {
        let Some(active) = self.perf.get(0).cloned() else { return };
        let before_engine_start = !self.perf_state.engines_started;
        fms_data::defaults::pipe_to(&active, self.perf.ensure(slot), before_engine_start);
    }
}

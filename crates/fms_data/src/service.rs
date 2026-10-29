use std::collections::BTreeMap;

use crate::checks::TakeoffChecks;
use crate::command::{Command, Effect, Event, WindPhase};
use crate::data::PerformanceData;
use crate::defaults::{self, DefaultsConfig};
use crate::entry::{self, EntryContext, EntryError};
use crate::limits::{WIND_DIRECTION, WIND_SPEED};

pub const ACTIVE: usize = 0;

/// Per-plan performance data plus the FMGC take-off checks. The FMS service owns one
/// and calls `execute`; wind edits are validated here and handed back as `Effect::Wind`.
#[derive(Clone, Debug, Default)]
pub struct PerfDataStore {
    pub plans: BTreeMap<usize, PerformanceData>,
    pub checks: TakeoffChecks,
    pub config: DefaultsConfig,
}

impl PerfDataStore {
    pub fn get(&self, plan: usize) -> Option<&PerformanceData> {
        self.plans.get(&plan)
    }

    pub fn ensure(&mut self, plan: usize) -> &mut PerformanceData {
        self.plans.entry(plan).or_default()
    }

    pub fn copy(&mut self, from: usize, to: usize) {
        if let Some(pd) = self.plans.get(&from).cloned() {
            self.plans.insert(to, pd);
        }
    }

    pub fn delete(&mut self, plan: usize) {
        self.plans.remove(&plan);
    }

    pub fn execute(&mut self, ctx: &EntryContext, command: Command) -> Result<Vec<Event>, EntryError> {
        match command {
            Command::Enter { plan, entry } => entry::apply(self.ensure(plan), ctx, entry),
            Command::SetWind { plan, phase, altitude, direction, magnitude, cruise_index } => {
                let altitude = match phase {
                    WindPhase::Cruise => {
                        entry::validate_wind(direction, magnitude, crate::limits::MAX_CERTIFIED_ALT, None)?;
                        if !(0.0..=crate::limits::MAX_CERTIFIED_ALT).contains(&altitude) {
                            return Err(EntryError::OutOfRange { min: 0.0, max: crate::limits::MAX_CERTIFIED_ALT });
                        }
                        altitude
                    }
                    WindPhase::Descent => entry::validate_wind(direction, magnitude, altitude, ctx.destination_elevation_ft)?,
                    WindPhase::Climb => entry::validate_wind(direction, magnitude, altitude, None)?,
                };
                Ok(vec![Event::Effect(Effect::Wind(Command::SetWind { plan, phase, altitude, direction, magnitude, cruise_index }))])
            }
            c @ (Command::DeleteWind { .. } | Command::DeleteAllWinds { .. }) => Ok(vec![Event::Effect(Effect::Wind(c))]),
            Command::SetAlternateWind { plan, direction, magnitude } => {
                let in_range = |v: Option<f64>, (lo, hi): (f64, f64)| v.map_or(true, |x| (lo..=hi).contains(&x));
                if !in_range(direction, WIND_DIRECTION) || !in_range(magnitude, WIND_SPEED) {
                    return Err(EntryError::OutOfRange { min: 0.0, max: WIND_SPEED.1 });
                }
                let pd = self.ensure(plan);
                pd.alternate_wind.direction = direction;
                pd.alternate_wind.magnitude = magnitude;
                Ok(vec![Event::Changed("alternateWind")])
            }
            Command::OriginChanged { plan, elevation_ft, transition_altitude_ft } => {
                let cfg = self.config;
                Ok(defaults::apply_origin_defaults(self.ensure(plan), &cfg, elevation_ft, transition_altitude_ft))
            }
            Command::DestinationChanged { plan, elevation_ft, transition_level } => {
                let cfg = self.config;
                Ok(defaults::apply_destination_defaults(self.ensure(plan), &cfg, elevation_ft, transition_level))
            }
            Command::OriginRunwayChanged { plan, runway } => {
                if plan != ACTIVE {
                    return Ok(Vec::new());
                }
                let pd = self.plans.entry(plan).or_default();
                Ok(self.checks.on_origin_runway(pd, &runway))
            }
            Command::ReconcileWithClimbConstraints { plan, lowest_climb_constraint_ft } => {
                let pd = self.ensure(plan);
                let mut ev = Vec::new();
                if defaults::reconcile_thrust_reduction(pd, lowest_climb_constraint_ft) {
                    ev.extend([Event::Changed("defaultThrustReductionAltitude"), Event::Changed("pilotThrustReductionAltitude")]);
                }
                if defaults::reconcile_acceleration(pd, lowest_climb_constraint_ft) {
                    ev.extend([Event::Changed("defaultAccelerationAltitude"), Event::Changed("pilotAccelerationAltitude")]);
                }
                Ok(ev)
            }
            Command::SecondaryActivated { zfw_diff_t, zfw_cg_diff } => {
                let pd = self.ensure(ACTIVE).clone();
                Ok(TakeoffChecks::on_secondary_activated(ctx.phase, &pd, zfw_diff_t, zfw_cg_diff))
            }
            Command::DefaultCostIndexIfMissing { plan } => Ok(defaults::default_cost_index_if_missing(self.ensure(plan), ctx.has_destination)),
        }
    }

    /// Run every FMS update; returns the discrete word 3 bits and any message edges.
    pub fn update(&mut self) -> (crate::checks::TakeoffSpeedBits, Vec<Event>) {
        let pd = self.plans.get(&ACTIVE).cloned().unwrap_or_default();
        self.checks.speed_checks(&pd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::Entry;

    #[test]
    fn ai_fo_style_preflight_sequence() {
        let mut s = PerfDataStore::default();
        let ctx = EntryContext { active: true, active_or_copy: true, has_destination: true, destination_elevation_ft: Some(13.0), ..Default::default() };
        s.execute(&ctx, Command::OriginChanged { plan: 0, elevation_ft: Some(83.0), transition_altitude_ft: Some(6000.0) }).unwrap();
        s.execute(&ctx, Command::DestinationChanged { plan: 0, elevation_ft: Some(13.0), transition_level: Some(180.0) }).unwrap();
        for e in [Entry::ZeroFuelWeightKg(Some(330_000.0)), Entry::ZeroFuelWeightCg(Some(38.5)), Entry::BlockFuelKg(Some(95_000.0)), Entry::CostIndex(Some(30.0)), Entry::CruiseFlightLevel(360.0), Entry::TakeoffFlaps(Some(2)), Entry::ThsFor(Some(30.0)), Entry::FlexTemperature(Some(45.0)), Entry::V1(Some(140.0)), Entry::Vr(Some(150.0)), Entry::V2(Some(158.0))] {
            s.execute(&ctx, Command::Enter { plan: 0, entry: e }).unwrap();
        }
        s.execute(&ctx, Command::OriginRunwayChanged { plan: 0, runway: "RW27L".into() }).unwrap();
        let (bits, _) = s.update();
        assert!(!bits.v_speed_disagree && !bits.to_speeds_not_inserted);
        let pd = s.get(0).unwrap();
        assert_eq!(pd.thrust_reduction_altitude(), Some(1580.0));
        assert_eq!(pd.transition_level(), Some(180.0));
        let ev = s.execute(&ctx, Command::SetWind { plan: 0, phase: WindPhase::Descent, altitude: 0.0, direction: 270.0, magnitude: 20.0, cruise_index: None }).unwrap();
        assert!(matches!(&ev[0], Event::Effect(Effect::Wind(Command::SetWind { altitude, .. })) if *altitude == 13.0));
        assert!(s.execute(&ctx, Command::SetWind { plan: 0, phase: WindPhase::Climb, altitude: 200.0, direction: 270.0, magnitude: 20.0, cruise_index: None }).is_err());
    }
}

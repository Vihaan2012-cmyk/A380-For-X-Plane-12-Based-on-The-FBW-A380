use crate::geo::Coordinates;
use crate::navdb::{Fix, NavDatabase};
use crate::plan::{AltitudeConstraint, FlightPlan, FmsError, FmsResult, FLAG_DIRECT_TO_TURNING_POINT, FLAG_PENDING_DIRECT_TO_TURNING_POINT};
use crate::procedures::ProcSel;
use crate::winds::{WindEntry, WindVector};

pub const FLAG_COPIED_FROM_ACTIVE: u32 = 1;
pub const FLAG_SWAPPED_WITH_ACTIVE: u32 = 2;
pub const FLAG_MANUAL_CREATION: u32 = 4;
pub const FLAG_COMPANY_FLIGHT_PLAN: u32 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanRef {
    Active,
    Temporary,
    Uplink,
    Secondary(usize),
}

impl PlanRef {
    pub fn from_index(i: usize) -> PlanRef {
        match i {
            0 => PlanRef::Active,
            1 => PlanRef::Temporary,
            2 => PlanRef::Uplink,
            n => PlanRef::Secondary(n - 2),
        }
    }

    pub fn slot(self) -> usize {
        match self {
            PlanRef::Active => 0,
            PlanRef::Temporary => 1,
            PlanRef::Uplink => 2,
            PlanRef::Secondary(n) => 2 + n,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Command {
    NewCityPair { from: String, to: String, alternate: Option<String>, plan: PlanRef },
    SetAlternate { icao: Option<String>, plan: PlanRef },
    SetOriginRunway { runway: String, plan: PlanRef, alternate: bool },
    SetDestinationRunway { runway: String, plan: PlanRef, alternate: bool },
    SetDeparture { id: Option<String>, plan: PlanRef, alternate: bool },
    SetDepartureEnrouteTransition { id: Option<String>, plan: PlanRef, alternate: bool },
    SetArrivalEnrouteTransition { id: Option<String>, plan: PlanRef, alternate: bool },
    SetArrival { id: Option<String>, plan: PlanRef, alternate: bool },
    SetApproachVia { id: Option<String>, plan: PlanRef, alternate: bool },
    SetApproach { id: Option<String>, plan: PlanRef, alternate: bool },
    NextWaypoint { index: usize, waypoint: Fix, plan: PlanRef },
    InsertWaypointBefore { index: usize, waypoint: Fix, plan: PlanRef },
    DeleteElementAt { index: usize, insert_discontinuity: bool, plan: PlanRef },
    InsertDiscontinuityAfter { index: usize, plan: PlanRef },
    SetPilotAltitudeConstraint { index: usize, descent: bool, constraint: Option<AltitudeConstraint>, plan: PlanRef },
    SetPilotSpeedConstraint { index: usize, descent: bool, speed: Option<f64>, plan: PlanRef },
    AddOrUpdateCruiseStep { index: usize, altitude: f64, plan: PlanRef },
    RemoveCruiseStep { index: usize, plan: PlanRef },
    TemporaryInsert,
    TemporaryDelete,
    SecondaryCopyFromActive { index: usize },
    ActiveAndSecondarySwap { index: usize },
    UplinkInsert,
    UplinkDelete,
    DirectToLeg { ppos: Coordinates, true_track: f64, index: usize, mag_var: Option<f64>, with_abeam: bool, plan: PlanRef },
    DirectToWaypoint { ppos: Coordinates, true_track: f64, waypoint: Fix, mag_var: Option<f64>, with_abeam: bool, plan: PlanRef },
    NewDest { index: usize, airport: String, plan: PlanRef, alternate: bool },
    EnableAltn { index: usize, cruise_level: f64, plan: PlanRef },
    Sequence { plan: PlanRef },
    SetClimbWindEntry { altitude: Option<f64>, entry: Option<WindEntry>, plan: PlanRef },
    EditClimbWindEntry { index: usize, entry: WindEntry, plan: PlanRef },
    SetDescentWindEntry { altitude: Option<f64>, entry: Option<WindEntry>, plan: PlanRef },
    EditDescentWindEntry { index: usize, entry: WindEntry, plan: PlanRef },
    AddCruiseWindEntry { index: usize, entry: WindEntry, plan: PlanRef },
    DeleteCruiseWindEntry { index: usize, altitude: f64, plan: PlanRef },
    EditCruiseWindEntry { index: usize, entry_index: usize, entry: WindEntry, plan: PlanRef },
    DeleteClimbWindEntries { plan: PlanRef },
    DeleteAllClimbWindEntries,
    DeleteDescentWindEntries { plan: PlanRef },
    SetAlternateWind { vector: WindVector, plan: PlanRef },
    ClearAlternateWind { plan: PlanRef },
    InsertDraftWinds { plan: PlanRef },
    DeleteDraftWinds { plan: PlanRef },
    InsertWindUplink { plan: PlanRef },
    PerfData(fms_data::Command),
}

#[derive(Default)]
pub struct Fms {
    pub plans: [Option<FlightPlan>; 6],
    pub aircraft: AircraftState,
    pub perf: fms_data::PerfDataStore,
    pub perf_state: crate::perf::PerfState,
    /// Performance data events (FBW `setPerformanceData` keys, messages, host effects) not yet taken by the host.
    pub perf_events: Vec<fms_data::Event>,
    pub(crate) perf_seen: [Option<crate::perf::PerfSeen>; 6],
    pub(crate) perf_engines_were_started: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AircraftState {
    pub position: Coordinates,
    pub track: f64,
    pub mag_var: Option<f64>,
}

impl Fms {
    pub fn new() -> Self {
        Fms::default()
    }

    pub fn create(&mut self, r: PlanRef) {
        self.plans[r.slot()] = Some(FlightPlan::empty(r.slot()));
        self.forget_perf(r.slot());
    }

    pub fn delete(&mut self, r: PlanRef) {
        self.plans[r.slot()] = None;
        self.forget_perf(r.slot());
    }

    pub fn delete_all(&mut self) {
        self.plans = Default::default();
        for slot in 0..self.plans.len() {
            self.forget_perf(slot);
        }
    }

    pub fn plan(&self, r: PlanRef) -> Option<&FlightPlan> {
        self.plans.get(r.slot()).and_then(|p| p.as_ref())
    }

    pub fn plan_mut(&mut self, r: PlanRef) -> FmsResult<&mut FlightPlan> {
        self.plans.get_mut(r.slot()).and_then(|p| p.as_mut()).ok_or_else(|| FmsError(format!("[FMS/FPM] Flight plan {r:?} does not exist")))
    }

    pub fn active(&self) -> Option<&FlightPlan> {
        self.plan(PlanRef::Active)
    }

    pub fn has_temporary(&self) -> bool {
        self.plans[1].is_some()
    }

    pub fn has_direct_to_in_temporary(&self) -> bool {
        let Some(tmpy) = self.plan(PlanRef::Temporary) else { return false };
        let from = usize::try_from(tmpy.active_leg_index - 1).ok().and_then(|i| tmpy.leg_at(i));
        from.is_some_and(|l| l.flags & FLAG_DIRECT_TO_TURNING_POINT != 0 && l.flags & FLAG_PENDING_DIRECT_TO_TURNING_POINT != 0)
    }

    pub fn copy(&mut self, from: PlanRef, to: PlanRef) -> FmsResult<()> {
        let source = self.plan(from).cloned().ok_or_else(|| FmsError(format!("[FMS/FPM] Flight plan {from:?} does not exist")))?;
        let mut new_plan = source.clone();
        new_plan.index = to.slot();
        match from {
            PlanRef::Uplink => new_plan.flags |= FLAG_COMPANY_FLIGHT_PLAN,
            PlanRef::Active => new_plan.flags |= FLAG_COPIED_FROM_ACTIVE,
            _ => {}
        }
        if let (PlanRef::Secondary(_), PlanRef::Secondary(_)) = (from, to) {
            new_plan.flags = source.flags;
        }
        new_plan.draft_winds = Default::default();
        new_plan.pending_wind_uplink = Default::default();
        new_plan.increment_version();
        self.plans[to.slot()] = Some(new_plan);
        self.copy_perf(from.slot(), to.slot());
        Ok(())
    }

    pub fn swap(&mut self, a: PlanRef, b: PlanRef) -> FmsResult<()> {
        let mut plan_a = self.plan(a).cloned().ok_or_else(|| FmsError(format!("[FMS/FPM] Flight plan {a:?} does not exist")))?;
        let mut plan_b = self.plan(b).cloned().ok_or_else(|| FmsError(format!("[FMS/FPM] Flight plan {b:?} does not exist")))?;
        plan_a.index = b.slot();
        plan_b.index = a.slot();
        if b == PlanRef::Active {
            plan_b.flags = FLAG_SWAPPED_WITH_ACTIVE;
        }
        self.plans[a.slot()] = Some(plan_b);
        self.plans[b.slot()] = Some(plan_a);
        self.swap_perf(a.slot(), b.slot());
        Ok(())
    }

    fn ensure_temporary_exists(&mut self) -> FmsResult<()> {
        if self.has_temporary() {
            return Ok(());
        }
        self.copy(PlanRef::Active, PlanRef::Temporary)
    }

    fn prepare_destructive_modification(&mut self, plan: PlanRef) -> FmsResult<PlanRef> {
        let mut final_ref = plan;
        if plan == PlanRef::Active {
            if let Some(active) = self.plans[0].as_mut().filter(|p| crate::winds::DRAFT_ON_WIND_EDIT && p.has_draft_wind_entries()) {
                active.insert_draft_wind_entries();
            }
            self.ensure_temporary_exists()?;
            final_ref = PlanRef::Temporary;
        }
        self.plan_mut(final_ref)?.was_modified = true;
        Ok(final_ref)
    }

    fn procedure_target(&mut self, plan: PlanRef, alternate: bool) -> FmsResult<&mut FlightPlan> {
        let final_ref = self.prepare_destructive_modification(plan)?;
        let main = self.plan_mut(final_ref)?;
        if alternate {
            main.alternate.as_deref_mut().ok_or_else(|| FmsError("[FMS/FPM] No alternate flight plan".into()))
        } else {
            Ok(main)
        }
    }

    pub fn execute(&mut self, db: &dyn NavDatabase, command: Command) -> FmsResult<()> {
        let result = self.execute_inner(db, command);
        let events = self.sync_perf();
        self.perf_events.extend(events);
        result
    }

    fn execute_inner(&mut self, db: &dyn NavDatabase, command: Command) -> FmsResult<()> {
        match command {
            Command::NewCityPair { from, to, alternate, plan } => {
                if plan == PlanRef::Temporary {
                    return Err(FmsError("[FMS/FPM] Cannot enter new city pair on temporary flight plan".into()));
                }
                let flight_number = self.plan(plan).map(|p| p.flight_number.clone());
                if plan == PlanRef::Active {
                    self.plans[1] = None;
                }
                let mut new_plan = FlightPlan::empty(plan.slot());
                new_plan.flags = FLAG_MANUAL_CREATION;
                if let Some(n) = flight_number {
                    new_plan.flight_number = n;
                }
                new_plan.set_origin_airport(db, Some(&from))?;
                new_plan.set_destination_airport(db, Some(&to))?;
                if let Some(a) = alternate {
                    set_alternate(&mut new_plan, db, Some(&a))?;
                }
                self.plans[plan.slot()] = Some(new_plan);
                self.forget_perf(plan.slot());
                Ok(())
            }
            Command::SetAlternate { icao, plan } => {
                if plan == PlanRef::Temporary {
                    return Err(FmsError("[FMS/FPM] Cannot set alternate on temporary flight plan".into()));
                }
                let p = self.plan_mut(plan)?;
                p.was_modified = true;
                set_alternate(p, db, icao.as_deref())
            }
            Command::SetOriginRunway { runway, plan, alternate } => {
                let target = self.procedure_target(plan, alternate)?;
                target.set_origin_runway(db, Some(&runway))?;
                if plan == PlanRef::Active && !alternate {
                    let ctx = self.perf_context(0);
                    let events = self.perf.execute(&ctx, fms_data::Command::OriginRunwayChanged { plan: 0, runway }).unwrap_or_default();
                    self.perf_events.extend(events);
                }
                Ok(())
            }
            Command::SetDestinationRunway { runway, plan, alternate } => {
                let target = self.procedure_target(plan, alternate)?;
                target.set_destination_runway(db, Some(&runway))
            }
            Command::SetDeparture { id, plan, alternate } => {
                let target = self.procedure_target(plan, alternate)?;
                target.set_departure(db, ProcSel::from_option(id.as_deref()))
            }
            Command::SetDepartureEnrouteTransition { id, plan, alternate } => {
                let target = self.procedure_target(plan, alternate)?;
                target.set_departure_enroute_transition(db, ProcSel::from_option(id.as_deref()))
            }
            Command::SetArrivalEnrouteTransition { id, plan, alternate } => {
                let target = self.procedure_target(plan, alternate)?;
                target.set_arrival_enroute_transition(db, ProcSel::from_option(id.as_deref()))
            }
            Command::SetArrival { id, plan, alternate } => {
                let target = self.procedure_target(plan, alternate)?;
                target.set_arrival(db, ProcSel::from_option(id.as_deref()))
            }
            Command::SetApproachVia { id, plan, alternate } => {
                let target = self.procedure_target(plan, alternate)?;
                target.set_approach_via(db, ProcSel::from_option(id.as_deref()))
            }
            Command::SetApproach { id, plan, alternate } => {
                let target = self.procedure_target(plan, alternate)?;
                target.set_approach(db, id.as_deref())
            }
            Command::NextWaypoint { index, waypoint, plan } => {
                let target = self.prepare_destructive_modification(plan)?;
                self.plan_mut(target)?.next_waypoint(index, &waypoint)
            }
            Command::InsertWaypointBefore { index, waypoint, plan } => {
                let target = self.prepare_destructive_modification(plan)?;
                self.plan_mut(target)?.insert_waypoint_before(index, &waypoint)
            }
            Command::DeleteElementAt { index, insert_discontinuity, plan } => {
                let target = self.prepare_destructive_modification(plan)?;
                self.plan_mut(target)?.remove_element_at(index, insert_discontinuity);
                Ok(())
            }
            Command::InsertDiscontinuityAfter { index, plan } => {
                let target = self.prepare_destructive_modification(plan)?;
                self.plan_mut(target)?.insert_discontinuity_after(index);
                Ok(())
            }
            Command::SetPilotAltitudeConstraint { index, descent, constraint, plan } => {
                let target = self.prepare_destructive_modification(plan)?;
                self.plan_mut(target)?.set_pilot_altitude_constraint_at(index, descent, constraint);
                Ok(())
            }
            Command::SetPilotSpeedConstraint { index, descent, speed, plan } => {
                let target = self.prepare_destructive_modification(plan)?;
                self.plan_mut(target)?.set_pilot_speed_constraint_at(index, descent, speed);
                Ok(())
            }
            Command::AddOrUpdateCruiseStep { index, altitude, plan } => {
                let target = self.prepare_destructive_modification(plan)?;
                self.plan_mut(target)?.add_or_update_cruise_step(index, altitude);
                Ok(())
            }
            Command::RemoveCruiseStep { index, plan } => {
                let target = self.prepare_destructive_modification(plan)?;
                self.plan_mut(target)?.remove_cruise_step(index);
                Ok(())
            }
            Command::TemporaryInsert => {
                let tmpy = self.plan_mut(PlanRef::Temporary).map_err(|_| FmsError("[FMS/FPS] Cannot insert temporary flight plan if none exists".into()))?;
                if tmpy.pending_airways.is_some() {
                    tmpy.finalise_airway_entry()?;
                }
                if let Some(alt) = tmpy.alternate.as_mut() {
                    if alt.pending_airways.is_some() {
                        alt.finalise_airway_entry()?;
                    }
                }
                let has_direct_to = self.has_direct_to_in_temporary();
                let aircraft = self.aircraft;
                let tmpy = self.plan_mut(PlanRef::Temporary)?;
                if has_direct_to {
                    let from = (tmpy.active_leg_index - 1) as usize;
                    if let Some(leg) = tmpy.leg_at_mut(from) {
                        leg.flags &= !FLAG_PENDING_DIRECT_TO_TURNING_POINT;
                        leg.definition.course = Some(aircraft.track);
                        leg.definition.mag_var = aircraft.mag_var;
                        if let Some(w) = leg.definition.waypoint.as_mut() {
                            w.location.lat = aircraft.position.lat;
                            w.location.long = aircraft.position.long;
                        }
                    }
                    tmpy.increment_version();
                }
                tmpy.was_modified = true;
                self.copy(PlanRef::Temporary, PlanRef::Active)?;
                self.plans[1] = None;
                Ok(())
            }
            Command::TemporaryDelete => {
                if !self.has_temporary() {
                    return Err(FmsError("[FMS/FPS] Cannot delete temporary flight plan if none exists".into()));
                }
                self.plans[1] = None;
                Ok(())
            }
            Command::SecondaryCopyFromActive { index } => {
                self.copy(PlanRef::Active, PlanRef::Secondary(index))?;
                if let Some(sec) = self.plans[PlanRef::Secondary(index).slot()].as_mut() {
                    sec.flags = FLAG_COPIED_FROM_ACTIVE;
                }
                self.pipe_active_perf_to(PlanRef::Secondary(index).slot());
                Ok(())
            }
            Command::ActiveAndSecondarySwap { index } => {
                let events = self.sync_perf();
                self.perf_events.extend(events);
                let old = self.perf.get(0).cloned();
                let old_flight_number = self.plan(PlanRef::Active).map(|p| p.flight_number.clone());
                self.swap(PlanRef::Secondary(index), PlanRef::Active)?;
                let events = self.persist_active_perf(old, old_flight_number);
                self.perf_events.extend(events);
                Ok(())
            }
            Command::UplinkInsert => {
                if self.plan(PlanRef::Uplink).is_none() {
                    return Err(FmsError("[FMS/FPS] Cannot insert uplink flight plan if none exists".into()));
                }
                self.copy(PlanRef::Uplink, PlanRef::Active)?;
                self.plans[2] = None;
                self.plans[1] = None;
                Ok(())
            }
            Command::UplinkDelete => {
                self.plans[2] = None;
                Ok(())
            }
            Command::DirectToLeg { ppos, true_track, index, mag_var, with_abeam: _, plan } => {
                let target = self.prepare_destructive_modification(plan)?;
                self.plan_mut(target)?.direct_to_leg(ppos, true_track, index, mag_var)
            }
            Command::DirectToWaypoint { ppos, true_track, waypoint, mag_var, with_abeam: _, plan } => {
                let target = self.prepare_destructive_modification(plan)?;
                self.plan_mut(target)?.direct_to_waypoint(ppos, true_track, &waypoint, mag_var)
            }
            Command::NewDest { index, airport, plan, alternate } => {
                let target = self.procedure_target(plan, alternate)?;
                target.new_dest(db, index, &airport)
            }
            Command::EnableAltn { index, cruise_level, plan } => {
                let target = self.prepare_destructive_modification(plan)?;
                self.plan_mut(target)?.enable_altn(db, index, cruise_level)
            }
            Command::Sequence { plan } => self.plan_mut(plan)?.sequence(db),
            Command::SetClimbWindEntry { altitude, entry, plan } => {
                self.plan_mut(plan)?.set_climb_wind_entry(altitude, entry);
                Ok(())
            }
            Command::EditClimbWindEntry { index, entry, plan } => {
                self.plan_mut(plan)?.edit_climb_wind_entry(index, entry);
                Ok(())
            }
            Command::SetDescentWindEntry { altitude, entry, plan } => {
                let p = self.plan_mut(plan)?;
                let mag_var = p.destination_airport.as_ref().and_then(|a| a.mag_var).unwrap_or(0.);
                p.set_descent_wind_entry(altitude, entry, mag_var);
                Ok(())
            }
            Command::EditDescentWindEntry { index, entry, plan } => {
                let p = self.plan_mut(plan)?;
                let mag_var = p.destination_airport.as_ref().and_then(|a| a.mag_var).unwrap_or(0.);
                p.edit_descent_wind_entry(index, entry, mag_var);
                Ok(())
            }
            Command::AddCruiseWindEntry { index, entry, plan } => {
                self.plan_mut(plan)?.add_cruise_wind_entry(index, entry);
                Ok(())
            }
            Command::DeleteCruiseWindEntry { index, altitude, plan } => {
                self.plan_mut(plan)?.delete_cruise_wind_entry(index, altitude);
                Ok(())
            }
            Command::EditCruiseWindEntry { index, entry_index, entry, plan } => {
                self.plan_mut(plan)?.edit_cruise_wind_entry(index, entry_index, entry);
                Ok(())
            }
            Command::DeleteClimbWindEntries { plan } => {
                self.plan_mut(plan)?.delete_climb_wind_entries();
                Ok(())
            }
            Command::DeleteAllClimbWindEntries => {
                for slot in [0, 3, 4, 5] {
                    if let Some(p) = self.plans[slot].as_mut() {
                        p.delete_climb_wind_entries();
                    }
                }
                Ok(())
            }
            Command::DeleteDescentWindEntries { plan } => {
                self.plan_mut(plan)?.delete_descent_wind_entries();
                Ok(())
            }
            Command::SetAlternateWind { vector, plan } => {
                self.plan_mut(plan)?.set_alternate_wind(vector);
                Ok(())
            }
            Command::ClearAlternateWind { plan } => {
                self.plan_mut(plan)?.clear_alternate_wind();
                Ok(())
            }
            Command::InsertDraftWinds { plan } => {
                self.plan_mut(plan)?.insert_draft_wind_entries();
                Ok(())
            }
            Command::DeleteDraftWinds { plan } => {
                self.plan_mut(plan)?.delete_draft_wind_entries();
                Ok(())
            }
            Command::InsertWindUplink { plan } => self.plan_mut(plan)?.insert_wind_uplink().map_err(FmsError),
            Command::PerfData(c) => {
                let events = self.execute_perf(c).map_err(|e| FmsError(format!("[FMS/PERF] {e:?}")))?;
                self.perf_events.extend(events);
                Ok(())
            }
        }
    }
}

fn set_alternate(plan: &mut FlightPlan, db: &dyn NavDatabase, icao: Option<&str>) -> FmsResult<()> {
    plan.alternate = None;
    let Some(icao) = icao else { return Ok(()) };
    let mut alt = FlightPlan::empty(plan.index);
    let dest = plan.destination_airport.as_ref().map(|d| d.ident.clone());
    alt.set_origin_airport(db, dest.as_deref())?;
    alt.set_destination_airport(db, Some(icao))?;
    plan.alternate = Some(Box::new(alt));
    Ok(())
}

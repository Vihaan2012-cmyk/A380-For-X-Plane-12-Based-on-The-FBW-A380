use serde::{Deserialize, Serialize};

use crate::geo::Coordinates;
use crate::navdb::{Fix, NavDatabase};
use crate::plan::{
    Element, FlightPlan, FmsError, FmsResult, Leg, LegDefinition, LegType, QueuedOp, Sel, SegmentClass, SegmentKind, APPROACH_DESCRIPTOR_MISSED_APPROACH_POINT, CONSTRAINT_UNKNOWN,
    FLAG_DIRECT_TO_TURNING_POINT, FLAG_PENDING_DIRECT_TO_TURNING_POINT, RESTRING_DEFAULT,
};
use crate::procedures::ProcSel;

pub const TEMPORARY_PLAN_INDEX: usize = 1;
pub const DEFAULT_SPEED_LIMIT_SPEED: f64 = 250.;
pub const DEFAULT_SPEED_LIMIT_ALTITUDE: f64 = 10_000.;
pub const DEFAULT_CONFIG_ALTITUDE_OFFSET: f64 = 1500.;

pub fn true_to_magnetic(true_track: f64, mag_var: Option<f64>) -> f64 {
    match mag_var {
        Some(v) => (true_track - v).rem_euclid(360.),
        None => true_track,
    }
}

fn mag_var_for_fix(fix: &Fix) -> Option<f64> {
    fix.mag_var.or(fix.station_declination).or(Some(0.))
}

impl Leg {
    fn bare(definition: LegDefinition, ident: &str) -> Leg {
        Leg {
            leg_type: definition.leg_type,
            definition,
            ident: ident.to_string(),
            annotation: String::new(),
            flags: 0,
            constraint_type: CONSTRAINT_UNKNOWN,
            pilot_altitude: None,
            pilot_speed: None,
            cruise_step: None,
            default_hold: None,
            modified_hold: None,
            cruise_wind_entries: Vec::new(),
        }
    }

    pub fn turning_point(location: Coordinates, course: f64, mag_var: Option<f64>) -> Leg {
        let mut definition = LegDefinition::blank(LegType::CF);
        let mut fix = Fix::lat_long("T-P", location.lat, location.long);
        fix.database_id = format!("W      {:<5}", "T-P");
        definition.waypoint = Some(fix);
        definition.course = Some(course);
        definition.mag_var = mag_var;
        definition.length = Some(0.);
        Leg::bare(definition, "T-P")
    }

    pub fn direct_to_turn_end(waypoint: &Fix, mag_var: Option<f64>) -> Leg {
        let mut definition = LegDefinition::blank(LegType::DF);
        definition.waypoint = Some(waypoint.clone());
        definition.mag_var = mag_var;
        Leg::bare(definition, &waypoint.ident)
    }

    pub fn with_pilot_entered_data_from(&mut self, from: &Leg) {
        self.pilot_altitude = from.pilot_altitude.clone();
        self.pilot_speed = from.pilot_speed.clone();
        self.constraint_type = from.constraint_type;
        self.cruise_step = from.cruise_step.clone();
    }
}

fn proc_sel<T>(sel: &Sel<T>, id: impl Fn(&T) -> &str) -> ProcSel<'_> {
    match sel {
        Sel::Set(t) => ProcSel::Id(id(t)),
        _ => ProcSel::Clear,
    }
}

impl FlightPlan {
    pub fn active_element(&self) -> Option<&Element> {
        usize::try_from(self.active_leg_index).ok().and_then(|i| self.element_at(i))
    }

    fn set_active_leg_index(&mut self, index: usize) {
        self.active_leg_index = index as i64;
    }

    fn turning_point_for(&self, ppos: Coordinates, true_track: f64, mag_var: Option<f64>) -> Leg {
        let mut turning_point = Leg::turning_point(ppos, true_to_magnetic(true_track, mag_var), mag_var);
        turning_point.flags |= FLAG_DIRECT_TO_TURNING_POINT;
        if self.index == TEMPORARY_PLAN_INDEX {
            turning_point.flags |= FLAG_PENDING_DIRECT_TO_TURNING_POINT;
        }
        turning_point
    }

    pub fn direct_to_leg(&mut self, ppos: Coordinates, true_track: f64, target_leg_index: usize, ppos_mag_var: Option<f64>) -> FmsResult<()> {
        if target_leg_index >= self.first_missed_approach_leg_index() {
            return Err(FmsError("[FPM] Cannot direct to a leg in the missed approach segment".into()));
        }
        let target_leg = self.leg_at(target_leg_index).cloned().ok_or_else(|| FmsError("[FPM] Cannot direct to a non-XF leg".into()))?;
        let Some(target_fix) = target_leg.termination_waypoint().cloned().filter(|_| target_leg.is_xf() || target_leg.is_hx()) else {
            return Err(FmsError("[FPM] Cannot direct to a non-XF leg".into()));
        };

        let turning_point = self.turning_point_for(ppos, true_track, ppos_mag_var);
        let mut turn_end = Leg::direct_to_turn_end(&target_fix, mag_var_for_fix(&target_fix));
        turn_end.with_definition_from(&target_leg);
        turn_end.with_pilot_entered_data_from(&target_leg);
        turn_end.ident = target_leg.ident.clone();

        self.redistribute_legs_at(0);
        self.redistribute_legs_at(target_leg_index);

        let enroute = SegmentKind::Enroute.slot();
        let index_in_enroute = match self.segment_position(target_leg_index) {
            Some((s, i)) if s == enroute => i,
            _ => return Err(FmsError("[FPM] Target leg of a direct to not found in enroute segment after leg redistribution!".into())),
        };
        self.segments[enroute].legs.splice(0..=index_in_enroute, [Element::new_leg(turning_point), Element::new_leg(turn_end)]);
        self.increment_version();

        let turn_end_index = self.plan_index_in_enroute(1);
        if !self.requires_turn_direction_at(turn_end_index + 1) {
            self.remove_forced_turn_at(turn_end_index + 1);
        }
        self.set_active_leg_index(turn_end_index);
        Ok(())
    }

    fn plan_index_in_enroute(&self, index_in_enroute: usize) -> usize {
        let enroute = SegmentKind::Enroute.slot();
        self.segments[..enroute].iter().map(|s| s.legs.len()).sum::<usize>() + index_in_enroute
    }

    pub fn direct_to_waypoint(&mut self, ppos: Coordinates, true_track: f64, waypoint: &Fix, ppos_mag_var: Option<f64>) -> FmsResult<()> {
        let existing = self.all_legs().iter().position(|e| e.leg().is_some_and(|l| l.terminates_with_waypoint(waypoint)));
        if let Some(i) = existing.filter(|i| *i < self.first_missed_approach_leg_index()) {
            return self.direct_to_leg(ppos, true_track, i, ppos_mag_var);
        }

        let mut turning_point = Leg::turning_point(ppos, true_to_magnetic(true_track, ppos_mag_var), ppos_mag_var);
        turning_point.flags = self.turning_point_for(ppos, true_track, ppos_mag_var).flags;
        let turn_end = Leg::direct_to_turn_end(waypoint, mag_var_for_fix(waypoint));

        let enroute = SegmentKind::Enroute.slot();
        let mut index_in_enroute = 0;
        self.redistribute_legs_at(0);
        if self.active_leg_index >= 1 {
            let active = self.active_leg_index as usize;
            self.redistribute_legs_at(active);
            index_in_enroute = match self.segment_position(active) {
                Some((s, i)) if s == enroute => i,
                _ => 0,
            };
        }

        self.segments[enroute].legs.splice(0..index_in_enroute, [Element::new_leg(turning_point), Element::new_leg(turn_end)]);
        self.increment_version();

        let turn_end_index = self.plan_index_in_enroute(1);
        if self.element_at(turn_end_index + 1).is_some_and(|e| !e.is_disco()) {
            self.segments[enroute].legs.insert(2, Element::Disco);
            self.increment_version();
            self.clean_up_after_discontinuity(turn_end_index + 1);
        }
        self.set_active_leg_index(turn_end_index);
        Ok(())
    }

    fn clean_up_after_discontinuity(&mut self, discontinuity_index: usize) {
        let legs = self.all_legs();
        let Some(xf_index) = legs.iter().enumerate().position(|(i, e)| i > discontinuity_index && e.leg().is_some_and(|l| l.is_xf() || l.is_hx())) else { return };
        self.remove_range(discontinuity_index + 1, xf_index);
        self.increment_version();

        let Some((seg, in_seg)) = self.segment_position(xf_index) else { return };
        let Some(xf) = self.segments[seg].legs[in_seg].leg().cloned() else { return };
        if xf.leg_type != LegType::IF && xf.leg_type != LegType::CF {
            if let Some(waypoint) = xf.definition.waypoint.clone() {
                let mut if_leg = Leg::from_enroute_fix(&waypoint, None, LegType::IF);
                if_leg.with_definition_from(&xf);
                if_leg.with_pilot_entered_data_from(&xf);
                self.segments[seg].legs[in_seg] = Element::new_leg(if_leg);
                self.increment_version();
            }
        }
    }

    pub fn sequence(&mut self, db: &dyn NavDatabase) -> FmsResult<()> {
        self.increment_version();
        let active = self.active_element().and_then(|e| e.leg()).cloned();
        if active.as_ref().is_some_and(|l| l.definition.approach_waypoint_descriptor == Some(APPROACH_DESCRIPTOR_MISSED_APPROACH_POINT)) {
            self.string_missed_approach(db)?;
        }
        if active.as_ref().is_some_and(|l| l.cruise_step.is_some()) {
            self.remove_cruise_step(self.active_leg_index as usize);
        }
        self.active_leg_index += 1;
        Ok(())
    }

    pub fn string_missed_approach(&mut self, db: &dyn NavDatabase) -> FmsResult<()> {
        let Ok(active) = usize::try_from(self.active_leg_index) else { return Ok(()) };
        if !self.element_at(active).is_some_and(|e| !e.is_disco()) {
            return Ok(());
        }
        let active_class = self.segment_position(active).map(|(s, _)| self.segments[s].kind.class());
        if !self.seg(SegmentKind::Approach).legs.is_empty() && active_class != Some(SegmentClass::Arrival) {
            return Ok(());
        }
        let Some(map_index) = self
            .all_legs()
            .iter()
            .position(|e| e.leg().is_some_and(|l| l.definition.approach_waypoint_descriptor == Some(APPROACH_DESCRIPTOR_MISSED_APPROACH_POINT)))
        else {
            return Ok(());
        };

        for i in active..=map_index {
            if let Some(l) = self.leg_at_mut(i) {
                l.clear_constraints();
            }
        }

        self.redistribute_legs_at(map_index);

        let missed: Vec<Element> = self.seg(SegmentKind::MissedApproach).legs.clone();
        let enroute = self.seg_mut(SegmentKind::Enroute);
        enroute.legs.extend(missed);
        enroute.legs.push(Element::Disco);
        enroute.strung = true;
        enroute.is_sequenced_missed_approach = true;

        self.arrival_set_procedure(db, ProcSel::Clear)?;
        self.enqueue_operation(QueuedOp::RebuildArrivalAndApproach);
        self.flush_operation_queue(db)?;
        self.increment_version();
        self.enqueue_operation(QueuedOp::Restring(RESTRING_DEFAULT));
        self.flush_operation_queue(db)?;
        Ok(())
    }

    pub fn new_dest(&mut self, db: &dyn NavDatabase, index: usize, airport_ident: &str) -> FmsResult<()> {
        let (seg, _) = self.segment_position(index).ok_or_else(|| FmsError(format!("[FMS/FPM] No leg at index {index}")))?;
        let is_departure = self.segments[seg].kind.class() == SegmentClass::Departure;
        self.redistribute_legs_at(if is_departure { index + 1 } else { index });

        let enroute = SegmentKind::Enroute.slot();
        let index_in_enroute: i64 = match self.segment_position(index) {
            Some((s, i)) if s == enroute => i as i64,
            _ => -1,
        };
        let legs_to_delete = self.segments[enroute].legs.len() as i64 - (index_in_enroute + 1);

        self.approach_set_procedure(db, None)?;
        self.approach_via_set_procedure(ProcSel::Clear)?;
        self.arrival_enroute_transition_set_procedure(ProcSel::Clear)?;
        self.arrival_set_procedure(db, ProcSel::Clear)?;

        self.set_destination_airport(db, Some(airport_ident))?;
        self.set_destination_runway(db, None)?;

        let start = (index_in_enroute + 1) as usize;
        let legs = &mut self.segments[enroute].legs;
        let end = (start + legs_to_delete.max(0) as usize).min(legs.len());
        if start < end {
            legs.drain(start..end);
        }
        if !legs.last().is_some_and(|e| e.is_disco()) {
            legs.push(Element::Disco);
        }
        self.segments[enroute].strung = true;
        self.flush_operation_queue(db)?;

        if self.alternate.is_some() {
            self.delete_alternate_flight_plan();
        }
        Ok(())
    }

    pub fn delete_alternate_flight_plan(&mut self) {
        let mut empty = FlightPlan::empty(self.index);
        if let Some(old) = self.alternate.as_ref() {
            empty.version = old.version + 1;
        }
        self.alternate = Some(Box::new(empty));
        let p = &mut self.performance;
        p.alternate_climb_speed_limit_speed = Some(DEFAULT_SPEED_LIMIT_SPEED);
        p.alternate_climb_speed_limit_altitude = Some(DEFAULT_SPEED_LIMIT_ALTITUDE);
        p.is_alternate_climb_speed_limit_pilot_entered = false;
        p.alternate_descent_speed_limit_speed = Some(DEFAULT_SPEED_LIMIT_SPEED);
        p.alternate_descent_speed_limit_altitude = Some(DEFAULT_SPEED_LIMIT_ALTITUDE);
        p.is_alternate_descent_speed_limit_pilot_entered = false;
        p.pilot_alternate_fuel = None;
    }

    pub fn enable_altn(&mut self, db: &dyn NavDatabase, at_index: usize, cruise_level: f64) -> FmsResult<()> {
        let alternate = self
            .alternate
            .as_deref()
            .filter(|a| a.destination_airport.is_some())
            .cloned()
            .ok_or_else(|| FmsError("[FMS/FPM] Cannot enable alternate with no alternate destination defined".into()))?;

        self.redistribute_legs_at(at_index);
        if self.leg_count() > at_index + 1 {
            self.remove_range(at_index + 1, self.leg_count());
        }

        let destination = alternate.destination_airport.as_ref().map(|a| a.ident.clone());
        self.destination_set_airport(db, destination.as_deref())?;
        let runway = alternate.destination_runway.as_ref().map(|r| r.fix.ident.clone());
        self.destination_set_runway(db, runway.as_deref(), false)?;
        let approach = alternate.approach.as_ref().map(|a| a.database_id.clone());
        self.approach_set_procedure(db, approach.as_deref())?;
        self.approach_via_set_procedure(proc_sel(&alternate.approach_via, |t| t.database_id.as_str()))?;
        self.arrival_set_procedure(db, proc_sel(&alternate.arrival, |p| p.database_id.as_str()))?;
        self.arrival_enroute_transition_set_procedure(proc_sel(&alternate.arrival_enroute_transition, |t| t.database_id.as_str()))?;

        let last_enroute: usize = [SegmentKind::Origin, SegmentKind::DepartureRunwayTransition, SegmentKind::Departure, SegmentKind::DepartureEnrouteTransition, SegmentKind::Enroute]
            .iter()
            .map(|k| alternate.seg(*k).legs.len())
            .sum();
        let to_insert: Vec<Element> = alternate.all_legs().into_iter().take(last_enroute).collect();

        let enroute = self.seg_mut(SegmentKind::Enroute);
        if enroute.legs.last().is_some_and(|e| !e.is_disco()) && to_insert.first().is_some_and(|e| !e.is_disco()) {
            enroute.legs.push(Element::Disco);
        }
        enroute.legs.extend(to_insert);
        enroute.strung = false;

        let p = &mut self.performance;
        p.cruise_flight_level = Some(cruise_level);
        p.cost_index = Some(0.);
        p.climb_speed_limit_speed = Some(p.alternate_climb_speed_limit_speed.unwrap_or(DEFAULT_SPEED_LIMIT_SPEED));
        p.climb_speed_limit_altitude = Some(p.alternate_climb_speed_limit_altitude.unwrap_or(DEFAULT_SPEED_LIMIT_ALTITUDE));
        p.is_climb_speed_limit_pilot_entered = p.is_alternate_climb_speed_limit_pilot_entered;
        p.descent_speed_limit_speed = Some(p.alternate_descent_speed_limit_speed.unwrap_or(DEFAULT_SPEED_LIMIT_SPEED));
        p.descent_speed_limit_altitude = Some(p.alternate_descent_speed_limit_altitude.unwrap_or(DEFAULT_SPEED_LIMIT_ALTITUDE));
        p.is_descent_speed_limit_pilot_entered = p.is_alternate_descent_speed_limit_pilot_entered;

        self.delete_alternate_flight_plan();

        self.enqueue_operation(QueuedOp::RebuildArrivalAndApproach);
        self.enqueue_operation(QueuedOp::Restring(RESTRING_DEFAULT));
        self.flush_operation_queue(db)
    }

    pub(crate) fn set_origin_default_performance_data(&mut self) {
        let airport = self.origin_airport.clone();
        let p = &mut self.performance;
        match airport.as_ref().and_then(|a| a.location.alt) {
            Some(alt) => {
                p.default_thrust_reduction_altitude = Some(alt + DEFAULT_CONFIG_ALTITUDE_OFFSET);
                p.default_acceleration_altitude = Some(alt + DEFAULT_CONFIG_ALTITUDE_OFFSET);
                p.default_engine_out_acceleration_altitude = Some(alt + DEFAULT_CONFIG_ALTITUDE_OFFSET);
            }
            None => {
                p.default_thrust_reduction_altitude = None;
                p.default_acceleration_altitude = None;
                p.default_engine_out_acceleration_altitude = None;
            }
        }
        p.pilot_thrust_reduction_altitude = None;
        p.pilot_acceleration_altitude = None;
        p.pilot_engine_out_acceleration_altitude = None;
        p.database_transition_altitude = airport.and_then(|a| a.transition_altitude);
    }

    pub(crate) fn set_destination_default_performance_data(&mut self) {
        let Some(airport) = self.destination_airport.clone() else { return };
        let p = &mut self.performance;
        match airport.location.alt {
            Some(alt) => {
                p.default_missed_thrust_reduction_altitude = Some(alt + DEFAULT_CONFIG_ALTITUDE_OFFSET);
                p.default_missed_acceleration_altitude = Some(alt + DEFAULT_CONFIG_ALTITUDE_OFFSET);
                p.default_missed_engine_out_acceleration_altitude = Some(alt + DEFAULT_CONFIG_ALTITUDE_OFFSET);
            }
            None => {
                p.default_missed_thrust_reduction_altitude = None;
                p.default_missed_acceleration_altitude = None;
                p.default_missed_engine_out_acceleration_altitude = None;
            }
        }
        p.pilot_missed_thrust_reduction_altitude = None;
        p.pilot_missed_acceleration_altitude = None;
        p.pilot_missed_engine_out_acceleration_altitude = None;
        p.database_transition_level = airport.transition_level;
    }

    pub fn thrust_reduction_altitude(&self) -> Option<f64> {
        self.performance.pilot_thrust_reduction_altitude.or(self.performance.default_thrust_reduction_altitude)
    }

    pub fn acceleration_altitude(&self) -> Option<f64> {
        self.performance.pilot_acceleration_altitude.or(self.performance.default_acceleration_altitude)
    }

    pub fn engine_out_acceleration_altitude(&self) -> Option<f64> {
        self.performance.pilot_engine_out_acceleration_altitude.or(self.performance.default_engine_out_acceleration_altitude)
    }
}

impl FlightPlan {
    pub fn refresh_engine_out_departure(&mut self, db: &dyn NavDatabase) {
        let runway = self.departure_runway_transition.as_ref().map(|t| t.ident.clone());
        let eosid = match (self.origin_airport.as_ref(), self.departure.get(), runway.as_deref()) {
            (Some(airport), Some(departure), Some(runway)) => db.engine_out_departure(&airport.ident, &departure.database_id, runway),
            _ => None,
        };
        let segment = &mut self.engine_out_departure;
        segment.legs.clear();
        segment.procedure_ident = None;
        segment.version += 1;
        let (Some(eosid), Some(runway)) = (eosid, runway) else { return };
        let Some(transition) = eosid.runway_transitions.iter().find(|t| t.ident == runway) else { return };
        segment.legs = transition.legs.iter().filter_map(|l| Leg::from_procedure_leg(l, &eosid.ident, crate::plan::CONSTRAINT_CLB)).map(Element::new_leg).collect();
        segment.procedure_ident = Some(eosid.ident.clone());
    }

    pub fn engine_out_legs(&self) -> &[Element] {
        &self.engine_out_departure.legs
    }
}

pub const SAVED_FLIGHT_PLAN_FORMAT: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedFlightPlan {
    pub format: u32,
    pub plan: FlightPlan,
}

impl FlightPlan {
    pub fn serialize_json(&self) -> String {
        serde_json::to_string(&SavedFlightPlan { format: SAVED_FLIGHT_PLAN_FORMAT, plan: self.clone() }).unwrap_or_default()
    }

    pub fn deserialize_json(json: &str, db: Option<&dyn NavDatabase>) -> FmsResult<FlightPlan> {
        let saved: SavedFlightPlan = serde_json::from_str(json).map_err(|e| FmsError(format!("[FMS/FPM] Bad saved flight plan: {e}")))?;
        if saved.format != SAVED_FLIGHT_PLAN_FORMAT {
            return Err(FmsError(format!("[FMS/FPM] Unsupported saved flight plan format {}", saved.format)));
        }
        let mut plan = saved.plan;
        if let Some(db) = db {
            plan.reload_available_from(db);
        }
        Ok(plan)
    }

    pub fn reload_available_from(&mut self, db: &dyn NavDatabase) {
        if let Some(origin) = self.origin_airport.as_ref() {
            self.available_origin_runways = db.runways(&origin.ident);
            let runway = self.origin_runway.as_ref().map(|r| r.fix.ident.clone());
            self.available_departures = db
                .departures(&origin.ident)
                .into_iter()
                .filter(|d| runway.is_none() || d.runway_transitions.is_empty() || d.runway_transitions.iter().any(|t| Some(&t.ident) == runway.as_ref()))
                .collect();
        }
        if let Some(destination) = self.destination_airport.as_ref() {
            self.available_destination_runways = db.runways(&destination.ident);
            self.available_arrivals = db.arrivals(&destination.ident);
            self.available_approaches = db.approaches(&destination.ident);
        }
        self.available_approach_vias = self.approach.as_ref().map(|a| a.enroute_transitions.clone()).unwrap_or_default();
        if let Some(alternate) = self.alternate.as_mut() {
            alternate.reload_available_from(db);
        }
    }
}

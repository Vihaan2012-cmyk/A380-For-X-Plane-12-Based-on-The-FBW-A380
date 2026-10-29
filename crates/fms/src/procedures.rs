use crate::naming::short_approach_name;
use crate::navdb::{are_database_items_equal, Fix, NavDatabase, Procedure, ProcedureLeg, Runway, Transition};
use crate::plan::{
    Element, FlightPlan, FmsError, FmsResult, Leg, LegType, QueuedOp, Sel, SegmentKind, CONSTRAINT_CLB, CONSTRAINT_DES, FLAG_ORIGIN, RESTRING_ARRIVAL, RESTRING_DEPARTURE,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcSel<'a> {
    Clear,
    NoneSelected,
    Id(&'a str),
}

impl<'a> ProcSel<'a> {
    pub fn from_option(id: Option<&'a str>) -> Self {
        match id {
            Some(id) => ProcSel::Id(id),
            None => ProcSel::Clear,
        }
    }
}

fn map_legs(legs: &[ProcedureLeg], procedure_ident: &str, constraint_type: i32) -> Vec<Leg> {
    legs.iter().filter_map(|l| Leg::from_procedure_leg(l, procedure_ident, constraint_type)).collect()
}

fn normalise180(x: f64) -> f64 {
    let r = x.rem_euclid(360.);
    if r > 180. {
        r - 360.
    } else {
        r
    }
}

fn if_leg_for(waypoint: &Option<Fix>) -> Option<Leg> {
    waypoint.as_ref().map(|w| Leg::from_enroute_fix(w, None, LegType::IF))
}

fn error(message: String) -> FmsError {
    FmsError(message)
}

impl FlightPlan {
    pub fn origin_departure(&self) -> Option<&Procedure> {
        self.departure.get()
    }

    fn departure_procedure_ident(&self) -> String {
        self.departure.get().map(|d| d.ident.clone()).unwrap_or_default()
    }

    pub fn set_origin_airport(&mut self, db: &dyn NavDatabase, icao: Option<&str>) -> FmsResult<()> {
        self.origin_set_airport(db, icao)?;
        self.flush_operation_queue(db)?;
        self.increment_version();
        self.set_origin_default_performance_data();
        Ok(())
    }

    pub fn set_origin_runway(&mut self, db: &dyn NavDatabase, runway: Option<&str>) -> FmsResult<()> {
        self.origin_set_runway(db, runway)?;
        self.flush_operation_queue(db)?;
        self.refresh_engine_out_departure(db);
        self.increment_version();
        Ok(())
    }

    pub fn set_departure(&mut self, db: &dyn NavDatabase, selection: ProcSel) -> FmsResult<()> {
        self.departure_set_procedure(db, selection)?;
        self.increment_version();
        self.flush_operation_queue(db)?;
        self.refresh_engine_out_departure(db);
        self.increment_version();
        Ok(())
    }

    pub fn set_departure_enroute_transition(&mut self, db: &dyn NavDatabase, selection: ProcSel) -> FmsResult<()> {
        self.departure_enroute_transition_set_procedure(selection)?;
        self.flush_operation_queue(db)?;
        self.increment_version();
        Ok(())
    }

    pub fn set_arrival_enroute_transition(&mut self, db: &dyn NavDatabase, selection: ProcSel) -> FmsResult<()> {
        self.arrival_enroute_transition_set_procedure(selection)?;
        self.flush_operation_queue(db)?;
        self.increment_version();
        Ok(())
    }

    pub fn set_arrival(&mut self, db: &dyn NavDatabase, selection: ProcSel) -> FmsResult<()> {
        self.arrival_set_procedure(db, selection)?;
        self.increment_version();
        self.flush_operation_queue(db)?;
        self.increment_version();
        Ok(())
    }

    pub fn set_approach_via(&mut self, db: &dyn NavDatabase, selection: ProcSel) -> FmsResult<()> {
        self.approach_via_set_procedure(selection)?;
        self.flush_operation_queue(db)?;
        self.increment_version();
        Ok(())
    }

    pub fn set_approach(&mut self, db: &dyn NavDatabase, selection: Option<&str>) -> FmsResult<()> {
        self.approach_set_procedure(db, selection)?;
        self.increment_version();
        self.flush_operation_queue(db)?;
        self.increment_version();
        Ok(())
    }

    pub fn set_destination_airport(&mut self, db: &dyn NavDatabase, icao: Option<&str>) -> FmsResult<()> {
        self.destination_set_airport(db, icao)?;
        self.increment_version();
        self.flush_operation_queue(db)?;
        self.increment_version();
        self.set_destination_default_performance_data();
        if let Some(alternate) = self.alternate.as_mut() {
            alternate.set_origin_airport(db, icao)?;
        }
        Ok(())
    }

    pub fn set_destination_runway(&mut self, db: &dyn NavDatabase, runway: Option<&str>) -> FmsResult<()> {
        self.destination_set_runway(db, runway, false)?;
        self.increment_version();
        self.flush_operation_queue(db)?;
        self.increment_version();
        Ok(())
    }

    pub(crate) fn origin_set_airport(&mut self, db: &dyn NavDatabase, icao: Option<&str>) -> FmsResult<()> {
        let Some(icao) = icao else {
            self.origin_airport = None;
            self.origin_runway = None;
            self.available_departures.clear();
            self.available_origin_runways.clear();
            self.refresh_departure_legs(db);
            return Ok(());
        };
        let airport = db.airport(icao).ok_or_else(|| error(format!("[FMS/FPM] Can't find airport with ICAO '{icao}'")))?;
        self.origin_airport = Some(airport.clone());
        self.refresh_departure_legs(db);
        self.available_origin_runways = db.runways(&airport.ident);
        self.available_departures = db.departures(&airport.ident);
        Ok(())
    }

    fn origin_set_runway(&mut self, db: &dyn NavDatabase, runway_ident: Option<&str>) -> FmsResult<()> {
        let Some(runway_ident) = runway_ident else {
            self.origin_runway = None;
            self.refresh_departure_legs(db);
            return Ok(());
        };
        let airport = self.origin_airport.clone().ok_or_else(|| error("[FMS/FPM] Cannot set origin runway with no origin airport".into()))?;
        let runway = db
            .runways(&airport.ident)
            .into_iter()
            .find(|r| r.fix.ident == runway_ident)
            .ok_or_else(|| error(format!("[FMS/FPM] Can't find runway '{runway_ident}' at {}", airport.ident)))?;
        self.origin_runway = Some(runway);
        self.refresh_departure_legs(db);
        self.seg_mut(SegmentKind::Origin).insert_necessary_discontinuities();
        Ok(())
    }

    fn refresh_departure_legs(&mut self, db: &dyn NavDatabase) {
        let Some(runway) = self.origin_runway.clone() else {
            self.refresh_origin_legs();
            return;
        };
        let airport_ident = runway.fix.airport_ident.clone().unwrap_or_default();
        let new_runway_compatible: Vec<Procedure> = db
            .departures(&airport_ident)
            .into_iter()
            .filter(|d| d.runway_transitions.iter().any(|t| t.ident == runway.fix.ident))
            .collect();
        let current_id = self.departure.get().map(|d| d.database_id.clone());
        let compatible = current_id.as_ref().is_some_and(|id| new_runway_compatible.iter().any(|d| &d.database_id == id));
        if compatible {
            let new_transition = self.departure.get().and_then(|d| d.runway_transitions.iter().find(|t| t.ident == runway.fix.ident).cloned());
            if let Some(t) = new_transition {
                if self.departure_runway_transition.as_ref().map(|c| c.ident.as_str()) != Some(t.ident.as_str()) {
                    self.departure_runway_transition_set_procedure(Some(&t.ident));
                    self.seg_mut(SegmentKind::Origin).strung = true;
                }
            }
        } else if self.departure.get().is_some() {
            let _ = self.departure_set_procedure(db, ProcSel::Clear);
        } else {
            self.refresh_origin_legs();
        }
        self.available_departures = new_runway_compatible;
    }

    fn reset_origin_leg_flag(&mut self) {
        for kind in [SegmentKind::Origin, SegmentKind::DepartureRunwayTransition, SegmentKind::Departure, SegmentKind::DepartureEnrouteTransition] {
            for e in self.seg_mut(kind).legs.iter_mut() {
                if let Some(l) = e.leg_mut() {
                    l.flags &= !FLAG_ORIGIN;
                }
            }
        }
    }

    pub fn refresh_origin_legs(&mut self) {
        self.reset_origin_leg_flag();
        let mut add_origin_leg = true;
        let mut add_initial_altitude_leg = false;
        let mut is_disconnected_idf = false;

        if let (Some(runway), Some(_)) = (self.origin_runway.clone(), self.departure.get()) {
            let first_departure_element = if !self.seg(SegmentKind::DepartureRunwayTransition).legs.is_empty() {
                self.seg(SegmentKind::DepartureRunwayTransition).legs.first().cloned()
            } else if !self.seg(SegmentKind::Departure).legs.is_empty() {
                self.seg(SegmentKind::Departure).legs.first().cloned()
            } else {
                self.seg(SegmentKind::DepartureEnrouteTransition).legs.first().cloned()
            };
            if let Some(Element::Leg(first)) = first_departure_element {
                if first.leg_type == LegType::IF {
                    let term = first.termination_waypoint().cloned();
                    if are_database_items_equal(term.as_ref(), Some(&runway.fix)) {
                        let target = if !self.seg(SegmentKind::DepartureRunwayTransition).legs.is_empty() {
                            SegmentKind::DepartureRunwayTransition
                        } else if !self.seg(SegmentKind::Departure).legs.is_empty() {
                            SegmentKind::Departure
                        } else {
                            SegmentKind::DepartureEnrouteTransition
                        };
                        if let Some(l) = self.seg_mut(target).legs[0].leg_mut() {
                            l.flags |= FLAG_ORIGIN;
                        }
                        add_origin_leg = false;
                    } else if let Some(term) = term {
                        let bearing = runway.threshold_location.coordinates().bearing_to(&term.location.coordinates());
                        let diff = normalise180(bearing - runway.bearing);
                        is_disconnected_idf = diff.abs() > 1.0;
                    }
                }
            }
        } else if self.origin_runway.is_some() && self.departure.get().is_none() {
            add_initial_altitude_leg = true;
        }

        let procedure_ident = self.departure_procedure_ident();
        let airport = self.origin_airport.clone();
        let runway = self.origin_runway.clone();
        self.seg_mut(SegmentKind::Origin).legs.clear();
        if add_origin_leg {
            if let Some(airport) = airport {
                let mut origin_leg = Leg::from_airport_and_runway(&procedure_ident, &airport, runway.as_ref());
                origin_leg.flags |= FLAG_ORIGIN;
                let seg = self.seg_mut(SegmentKind::Origin);
                seg.legs.push(Element::new_leg(origin_leg));
                seg.strung = false;
            }
        }
        if add_initial_altitude_leg {
            if let Some(runway) = &runway {
                let seg = self.seg_mut(SegmentKind::Origin);
                if let Some(Element::Leg(runway_leg)) = seg.legs.last().cloned() {
                    seg.legs.push(Element::new_leg(Leg::origin_extended_centerline(runway, &runway_leg)));
                }
            }
        }
        if is_disconnected_idf {
            self.seg_mut(SegmentKind::Origin).legs.push(Element::Disco);
        }
        self.enqueue_operation(QueuedOp::Restring(RESTRING_DEPARTURE));
    }

    fn departure_set_procedure(&mut self, db: &dyn NavDatabase, selection: ProcSel) -> FmsResult<()> {
        let id = match selection {
            ProcSel::Clear | ProcSel::NoneSelected => {
                self.departure = if selection == ProcSel::Clear { Sel::Unset } else { Sel::NoneSelected };
                self.seg_mut(SegmentKind::Departure).legs.clear();
                self.departure_runway_transition_set_procedure(None);
                self.departure_enroute_transition_set_procedure(ProcSel::Clear)?;
                self.refresh_origin_legs();
                return Ok(());
            }
            ProcSel::Id(id) => id,
        };
        let airport = self.origin_airport.clone().ok_or_else(|| error("[FMS/FPM] Cannot set departure procedure without origin airport".into()))?;
        let procedures = db.departures(&airport.ident);
        if procedures.is_empty() {
            return Err(error(format!("[FMS/FPM] Cannot find procedures at {}", airport.ident)));
        }
        let matching = procedures
            .into_iter()
            .find(|p| p.database_id == id)
            .ok_or_else(|| error(format!("[FMS/FPM] Can't find procedure '{id}' for {}", airport.ident)))?;
        self.departure = Sel::Set(matching.clone());

        let legs = map_legs(&matching.common_legs, &matching.ident, CONSTRAINT_CLB);
        let seg = self.seg_mut(SegmentKind::Departure);
        seg.legs.clear();
        if let Some(first) = legs.first() {
            if first.is_fx() && !first.is_runway() {
                if let Some(l) = if_leg_for(&first.definition.waypoint) {
                    seg.legs.push(Element::new_leg(l));
                }
            }
        }
        seg.legs.extend(legs.into_iter().map(Element::new_leg));
        seg.strung = false;

        let runway_ident = self.origin_runway.as_ref().map(|r| r.fix.ident.clone());
        self.departure_runway_transition_set_procedure(runway_ident.as_deref());
        self.enqueue_operation(QueuedOp::Restring(RESTRING_DEPARTURE));
        self.flush_operation_queue(db)
    }

    fn departure_runway_transition_set_procedure(&mut self, runway_ident: Option<&str>) {
        let existing = self.departure.get().cloned();
        self.departure_runway_transition = match (&existing, runway_ident) {
            (Some(dep), Some(r)) => dep.runway_transitions.iter().find(|t| t.ident == r).cloned(),
            _ => None,
        };
        let procedure_ident = existing.as_ref().map(|d| d.ident.clone()).unwrap_or_default();
        let legs = self.departure_runway_transition.as_ref().map(|t| map_legs(&t.legs, &procedure_ident, CONSTRAINT_CLB)).unwrap_or_default();
        let seg = self.seg_mut(SegmentKind::DepartureRunwayTransition);
        seg.legs = legs.into_iter().map(Element::new_leg).collect();
        seg.strung = false;
        self.refresh_origin_legs();
        self.enqueue_operation(QueuedOp::Restring(RESTRING_DEPARTURE));
    }

    fn departure_enroute_transition_set_procedure(&mut self, selection: ProcSel) -> FmsResult<()> {
        let id = match selection {
            ProcSel::Clear | ProcSel::NoneSelected => {
                self.departure_enroute_transition = if selection == ProcSel::Clear { Sel::Unset } else { Sel::NoneSelected };
                self.seg_mut(SegmentKind::DepartureEnrouteTransition).legs.clear();
                self.enqueue_operation(QueuedOp::Restring(RESTRING_DEPARTURE));
                return Ok(());
            }
            ProcSel::Id(id) => id,
        };
        let (Some(airport), Some(_), Some(departure)) = (self.origin_airport.clone(), self.origin_runway.clone(), self.departure.get().cloned()) else {
            return Err(error("[FMS/FPM] Cannot set origin enroute transition without destination airport, runway and SID".into()));
        };
        let transition = departure
            .enroute_transitions
            .iter()
            .find(|t| t.database_id == id)
            .cloned()
            .ok_or_else(|| error(format!("[FMS/FPM] Can't find origin enroute transition '{id}' for {} {}", airport.ident, departure.ident)))?;
        self.departure_enroute_transition = Sel::Set(transition.clone());
        let legs = map_legs(&transition.legs, &transition.ident, CONSTRAINT_CLB);
        let seg = self.seg_mut(SegmentKind::DepartureEnrouteTransition);
        seg.legs.clear();
        if let Some(first) = legs.first() {
            if first.is_fx() && !first.is_runway() {
                if let Some(l) = if_leg_for(&first.definition.waypoint) {
                    seg.legs.push(Element::new_leg(l));
                }
            }
        }
        seg.legs.extend(legs.into_iter().map(Element::new_leg));
        seg.strung = false;
        self.enqueue_operation(QueuedOp::Restring(RESTRING_DEPARTURE));
        Ok(())
    }

    pub(crate) fn destination_set_airport(&mut self, db: &dyn NavDatabase, icao: Option<&str>) -> FmsResult<()> {
        let Some(icao) = icao else {
            self.destination_airport = None;
            self.destination_runway = None;
            self.available_arrivals.clear();
            self.available_approaches.clear();
            self.available_destination_runways.clear();
            return self.destination_refresh(db, true);
        };
        let airport = db.airport(icao).ok_or_else(|| error(format!("[FMS/FPM] Can't find airport with ICAO '{icao}'")))?;
        self.destination_airport = Some(airport.clone());
        self.available_destination_runways = db.runways(&airport.ident);
        self.destination_refresh(db, true)?;
        self.available_arrivals = db.arrivals(&airport.ident);
        self.available_approaches = db.approaches(&airport.ident);
        Ok(())
    }

    pub(crate) fn destination_set_runway(&mut self, db: &dyn NavDatabase, runway_ident: Option<&str>, set_by_approach: bool) -> FmsResult<()> {
        let old_runway_ident = self.destination_runway.as_ref().map(|r| r.fix.ident.clone());
        match runway_ident {
            None => {
                self.destination_runway = None;
                self.arrival_runway_transition_set_procedure(None);
            }
            Some(ident) => {
                let airport = self.destination_airport.clone().ok_or_else(|| error("[FMS/FPM] Cannot set destination runway without destination airport".into()))?;
                let runway = db
                    .runways(&airport.ident)
                    .into_iter()
                    .find(|r| r.fix.ident == ident)
                    .ok_or_else(|| error(format!("[FMS/FPM] Can't find runway '{ident}' at {}", airport.ident)))?;
                self.destination_runway = Some(runway.clone());
                self.arrival_runway_transition_set_procedure(Some(&runway.fix.ident));
            }
        }
        let new_ident = self.destination_runway.as_ref().map(|r| r.fix.ident.clone());
        self.destination_refresh(db, old_runway_ident != new_ident && !set_by_approach)
    }

    fn destination_refresh(&mut self, db: &dyn NavDatabase, do_remove_approach: bool) -> FmsResult<()> {
        self.seg_mut(SegmentKind::Destination).legs.clear();
        if do_remove_approach {
            self.approach_set_procedure(db, None)?;
        }
        let airport = self.destination_airport.clone();
        match airport {
            Some(airport) if self.seg(SegmentKind::Approach).legs.is_empty() => {
                let leg = Leg::from_airport_and_runway("", &airport, None);
                self.seg_mut(SegmentKind::Destination).legs.push(Element::new_leg(leg));
            }
            _ => self.seg_mut(SegmentKind::Destination).legs.clear(),
        }
        self.available_approaches = self.destination_airport.as_ref().map(|a| db.approaches(&a.ident)).unwrap_or_default();
        self.enqueue_operation(QueuedOp::Restring(RESTRING_ARRIVAL));
        Ok(())
    }

    fn arrival_runway_transition_set_procedure(&mut self, runway_ident: Option<&str>) {
        let existing = self.arrival.get().cloned();
        self.arrival_runway_transition = match (&existing, runway_ident) {
            (Some(arr), Some(r)) => arr.runway_transitions.iter().find(|t| t.ident == r).cloned(),
            _ => None,
        };
        let procedure_ident = existing.as_ref().map(|a| a.ident.clone()).unwrap_or_default();
        let legs = self.arrival_runway_transition.as_ref().map(|t| map_legs(&t.legs, &procedure_ident, CONSTRAINT_DES)).unwrap_or_default();
        let seg = self.seg_mut(SegmentKind::ArrivalRunwayTransition);
        seg.legs = legs.into_iter().map(Element::new_leg).collect();
        seg.strung = false;
        self.enqueue_operation(QueuedOp::RebuildArrivalAndApproach);
        self.enqueue_operation(QueuedOp::Restring(RESTRING_ARRIVAL));
    }

    pub(crate) fn arrival_set_procedure(&mut self, db: &dyn NavDatabase, selection: ProcSel) -> FmsResult<()> {
        let old_arrival_name = self.arrival.get().map(|a| a.ident.clone());
        let id = match selection {
            ProcSel::Clear | ProcSel::NoneSelected => {
                self.arrival = if selection == ProcSel::Clear { Sel::Unset } else { Sel::NoneSelected };
                self.seg_mut(SegmentKind::Arrival).legs.clear();
                self.arrival_enroute_transition_set_procedure(ProcSel::Clear)?;
                self.arrival_runway_transition_set_procedure(None);
                return Ok(());
            }
            ProcSel::Id(id) => id,
        };
        let airport = self.destination_airport.clone().ok_or_else(|| error("[FMS/FPM] Cannot set approach without destination airport".into()))?;
        let matching = db
            .arrivals(&airport.ident)
            .into_iter()
            .find(|a| a.database_id == id)
            .ok_or_else(|| error(format!("[FMS/FPM] Can't find arrival procedure '{id}' for {}", airport.ident)))?;
        self.arrival = Sel::Set(matching.clone());

        let seg = self.seg_mut(SegmentKind::Arrival);
        seg.legs.clear();
        seg.strung = false;
        if old_arrival_name.as_deref() != Some(matching.ident.as_str()) {
            self.arrival_enroute_transition_set_procedure(ProcSel::Clear)?;
        }
        let legs = map_legs(&matching.common_legs, &matching.ident, CONSTRAINT_DES);
        let seg = self.seg_mut(SegmentKind::Arrival);
        if let Some(first) = legs.first() {
            if first.is_fx() {
                if let Some(l) = if_leg_for(&first.definition.waypoint) {
                    seg.legs.push(Element::new_leg(l));
                }
            }
        }
        seg.legs.extend(legs.into_iter().map(Element::new_leg));
        let runway_ident = self.destination_runway.as_ref().map(|r| r.fix.ident.clone());
        self.arrival_runway_transition_set_procedure(runway_ident.as_deref());
        self.enqueue_operation(QueuedOp::RebuildArrivalAndApproach);
        self.enqueue_operation(QueuedOp::Restring(RESTRING_ARRIVAL));
        Ok(())
    }

    pub(crate) fn arrival_enroute_transition_set_procedure(&mut self, selection: ProcSel) -> FmsResult<()> {
        let id = match selection {
            ProcSel::Clear | ProcSel::NoneSelected => {
                self.arrival_enroute_transition = if selection == ProcSel::Clear { Sel::Unset } else { Sel::NoneSelected };
                self.seg_mut(SegmentKind::ArrivalEnrouteTransition).legs.clear();
                return Ok(());
            }
            ProcSel::Id(id) => id,
        };
        let (Some(airport), Some(arrival)) = (self.destination_airport.clone(), self.arrival.get().cloned()) else {
            return Err(error("[FMS/FPM] Cannot set arrival enroute transition without destination airport and STAR".into()));
        };
        let transition = arrival
            .enroute_transitions
            .iter()
            .find(|t| t.database_id == id)
            .cloned()
            .ok_or_else(|| error(format!("[FMS/FPM] Can't find arrival enroute transition '{id}' for {} {}", airport.ident, arrival.ident)))?;
        self.arrival_enroute_transition = Sel::Set(transition.clone());
        let legs = map_legs(&transition.legs, &transition.ident, CONSTRAINT_DES);
        let seg = self.seg_mut(SegmentKind::ArrivalEnrouteTransition);
        seg.legs.clear();
        if let Some(first) = legs.first() {
            if first.is_fx() {
                if let Some(l) = if_leg_for(&first.definition.waypoint) {
                    seg.legs.push(Element::new_leg(l));
                }
            }
        }
        seg.legs.extend(legs.into_iter().map(Element::new_leg));
        seg.strung = false;
        self.enqueue_operation(QueuedOp::RebuildArrivalAndApproach);
        self.enqueue_operation(QueuedOp::Restring(RESTRING_ARRIVAL));
        Ok(())
    }

    pub(crate) fn approach_via_set_procedure(&mut self, selection: ProcSel) -> FmsResult<()> {
        let id = match selection {
            ProcSel::Clear | ProcSel::NoneSelected => {
                self.approach_via = if selection == ProcSel::Clear { Sel::Unset } else { Sel::NoneSelected };
                self.seg_mut(SegmentKind::ApproachVia).legs.clear();
                self.enqueue_operation(QueuedOp::RebuildArrivalAndApproach);
                self.enqueue_operation(QueuedOp::Restring(RESTRING_ARRIVAL));
                return Ok(());
            }
            ProcSel::Id(id) => id,
        };
        let approach = self.approach.clone().ok_or_else(|| error("[FMS/FPM] Cannot set approach via without approach".into()))?;
        let via = approach
            .enroute_transitions
            .iter()
            .find(|t| t.database_id == id)
            .cloned()
            .ok_or_else(|| error(format!("[FMS/FPM] Can't find arrival approach via '{id}' for {}", approach.ident)))?;
        self.approach_via = Sel::Set(via.clone());
        let legs = map_legs(&via.legs, &via.ident, CONSTRAINT_DES);
        let seg = self.seg_mut(SegmentKind::ApproachVia);
        seg.legs.clear();
        if let Some(first) = legs.first() {
            if first.leg_type == LegType::PI || first.is_fx() {
                if let Some(l) = if_leg_for(&first.definition.waypoint) {
                    seg.legs.push(Element::new_leg(l));
                }
            }
        }
        seg.legs.extend(legs.into_iter().map(Element::new_leg));
        seg.strung = false;
        self.enqueue_operation(QueuedOp::RebuildArrivalAndApproach);
        self.enqueue_operation(QueuedOp::Restring(RESTRING_ARRIVAL));
        Ok(())
    }

    pub(crate) fn approach_set_procedure(&mut self, db: &dyn NavDatabase, database_id: Option<&str>) -> FmsResult<()> {
        let old_approach_name = self.approach.as_ref().map(|a| a.ident.clone());
        let Some(id) = database_id else {
            self.approach = None;
            self.available_approach_vias.clear();
            self.approach_via_set_procedure(ProcSel::Clear)?;
            self.arrival_set_procedure(db, ProcSel::Clear)?;
            self.seg_mut(SegmentKind::MissedApproach).legs.clear();
            let legs = self.create_approach_leg_set(None, Vec::new());
            self.seg_mut(SegmentKind::Approach).legs = legs;
            self.enqueue_operation(QueuedOp::Restring(crate::plan::RESTRING_DEFAULT));
            return Ok(());
        };
        let airport = self.destination_airport.clone().ok_or_else(|| error("[FMS/FPM] Cannot set approach without destination airport".into()))?;
        let matching = db
            .approaches(&airport.ident)
            .into_iter()
            .find(|a| a.database_id == id)
            .ok_or_else(|| error(format!("[FMS/FPM] Can't find approach procedure '{id}' for {}", airport.ident)))?;
        self.approach = Some(matching.clone());

        let short_name = short_approach_name(matching.approach_type, matching.runway_ident.as_deref().unwrap_or(""), &matching.multiple_indicator);
        let approach_legs = map_legs(&matching.common_legs, &short_name, CONSTRAINT_DES);
        let legs = self.create_approach_leg_set(Some(&matching), approach_legs);
        let seg = self.seg_mut(SegmentKind::Approach);
        seg.legs = legs;
        seg.strung = false;

        if let Some(runway_ident) = matching.runway_ident.as_deref().filter(|r| !r.is_empty()) {
            self.destination_set_runway(db, Some(runway_ident), true)?;
        }

        let missed = map_legs(&matching.missed_legs, &short_name, CONSTRAINT_CLB);
        self.seg_mut(SegmentKind::MissedApproach).legs = missed.into_iter().map(Element::new_leg).collect();

        if old_approach_name.as_deref() != Some(matching.ident.as_str()) {
            self.approach_via_set_procedure(ProcSel::Clear)?;
            self.arrival_set_procedure(db, ProcSel::Clear)?;
        }
        self.available_approach_vias = matching.enroute_transitions.clone();
        self.enqueue_operation(QueuedOp::RebuildArrivalAndApproach);
        self.enqueue_operation(QueuedOp::Restring(RESTRING_ARRIVAL));
        Ok(())
    }

    fn create_approach_leg_set(&self, procedure: Option<&Procedure>, approach_legs: Vec<Leg>) -> Vec<Element> {
        let mut legs: Vec<Element> = Vec::new();
        let airport = self.destination_airport.as_ref();
        let runway = self.destination_runway.as_ref();
        let short_name = procedure.map(|p| short_approach_name(p.approach_type, p.runway_ident.as_deref().unwrap_or(""), &p.multiple_indicator)).unwrap_or_default();

        if let (Some(airport), Some(runway), true) = (airport, runway, approach_legs.is_empty()) {
            legs.push(Element::new_leg(Leg::destination_extended_centerline(runway)));
            legs.push(Element::new_leg(Leg::from_airport_and_runway(&short_name, airport, Some(runway))));
            return legs;
        }

        if let Some(first) = approach_legs.first() {
            if first.is_fx() {
                if let Some(l) = if_leg_for(&first.definition.waypoint) {
                    legs.push(Element::new_leg(l));
                }
            }
        }
        let last = approach_legs.last();
        let last_is_runway = last.is_some_and(|l| are_database_items_equal(l.termination_waypoint(), runway.map(|r| &r.fix)));
        match (airport, last_is_runway, last) {
            (Some(airport), true, Some(last)) => {
                legs.extend(approach_legs[..approach_legs.len() - 1].iter().cloned().map(Element::new_leg));
                let found: Option<&Runway> = self.available_destination_runways.iter().find(|r| are_database_items_equal(last.termination_waypoint(), Some(&r.fix)));
                let mut mapped = Leg::from_airport_and_runway(&short_name, airport, found);
                if approach_legs.len() > 1 {
                    mapped.leg_type = last.leg_type;
                    mapped.definition.assign_from(&last.definition);
                }
                legs.push(Element::new_leg(mapped));
            }
            _ => legs.extend(approach_legs.into_iter().map(Element::new_leg)),
        }
        legs
    }

    pub fn rebuild_arrival_and_approach_segments(&mut self, db: &dyn NavDatabase) -> FmsResult<()> {
        if let Some(approach) = self.approach.clone() {
            self.approach_set_procedure(db, Some(&approach.database_id))?;
        }
        if let Sel::Set(via) = self.approach_via.clone() {
            self.approach_via_set_procedure(ProcSel::Id(&via.database_id))?;
        }
        if let Sel::Set(arrival) = self.arrival.clone() {
            self.arrival_set_procedure(db, ProcSel::Id(&arrival.database_id))?;
        }
        if let Sel::Set(transition) = self.arrival_enroute_transition.clone() {
            self.arrival_enroute_transition_set_procedure(ProcSel::Id(&transition.database_id))?;
        }
        self.destination_refresh(db, false)
    }

    pub fn available_origin_runway_idents(&self) -> Vec<String> {
        self.available_origin_runways.iter().map(|r| r.fix.ident.clone()).collect()
    }

    pub fn transition_ident(transition: &Option<Transition>) -> Option<&str> {
        transition.as_ref().map(|t| t.ident.as_str())
    }
}

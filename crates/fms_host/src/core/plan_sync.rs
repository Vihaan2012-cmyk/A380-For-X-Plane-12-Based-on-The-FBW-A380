use std::collections::BTreeMap;

use fms::navdb::Fix;
use fms::plan::{Element, FlightPlan, Leg, SegmentKind};
use fms_data::PerformanceData;
use serde_json::{json, Map, Value};

use crate::outputs::Sink;

/// FBW picks a random `syncClientID` per instrument; any constant works for the receivers.
pub const SYNC_CLIENT_ID: u64 = 1_461_626_144;

#[derive(Clone, Debug, PartialEq)]
struct PlanState {
    flight_number: String,
    active_leg_index: i64,
    segments: [Vec<Value>; 2],
    perf: Map<String, Value>,
}

fn camel(snake: &str) -> String {
    let mut out = String::new();
    let mut up = false;
    for c in snake.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn some<T: serde::Serialize>(m: &mut Map<String, Value>, k: &str, v: &Option<T>) {
    if let Some(v) = v {
        m.insert(k.into(), json!(v));
    }
}

pub fn fix_json(f: &Fix) -> Value {
    let mut m = Map::new();
    m.insert("databaseId".into(), json!(f.database_id));
    m.insert("sectionCode".into(), json!(f.section_code));
    m.insert("subSectionCode".into(), json!(f.sub_section_code));
    m.insert("area".into(), json!(f.area));
    m.insert("ident".into(), json!(f.ident));
    some(&mut m, "airportIdent", &f.airport_ident);
    m.insert("icaoCode".into(), json!(f.icao_code));
    m.insert("name".into(), json!(f.name));
    let mut loc = json!({ "lat": f.location.lat, "long": f.location.long });
    if let Some(a) = f.location.alt {
        loc["alt"] = json!(a);
    }
    m.insert("location".into(), loc);
    some(&mut m, "longestRunwayLength", &f.longest_runway_length);
    some(&mut m, "longestRunwaySurfaceType", &f.longest_runway_surface_type);
    some(&mut m, "transitionAltitude", &f.transition_altitude);
    some(&mut m, "transitionLevel", &f.transition_level);
    some(&mut m, "magVar", &f.mag_var);
    some(&mut m, "stationDeclination", &f.station_declination);
    Value::Object(m)
}

fn uuid_for(seed: &str) -> String {
    let mut h: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    for b in seed.bytes() {
        h ^= b as u128;
        h = h.wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_013b);
    }
    let x = format!("{h:032x}");
    format!("{}-{}-{}-{}-{}", &x[0..8], &x[8..12], &x[12..16], &x[16..20], &x[20..32])
}

/// FBW `FlightPlanLeg.serialize`.
pub fn leg_json(leg: &Leg, position: usize) -> Value {
    let d = &leg.definition;
    let mut def = Map::new();
    def.insert("procedureIdent".into(), json!(d.procedure_ident));
    def.insert("type".into(), json!(format!("{:?}", d.leg_type)));
    def.insert("overfly".into(), json!(d.overfly));
    if let Some(w) = &d.waypoint {
        def.insert("waypoint".into(), fix_json(w));
    }
    some(&mut def, "waypointDescriptor", &d.waypoint_descriptor);
    some(&mut def, "approachWaypointDescriptor", &d.approach_waypoint_descriptor);
    some(&mut def, "turnDirection", &d.turn_direction.map(|c| c.to_string()));
    some(&mut def, "altitudeDescriptor", &d.altitude_descriptor);
    some(&mut def, "altitude1", &d.altitude1);
    some(&mut def, "altitude2", &d.altitude2);
    some(&mut def, "speedDescriptor", &d.speed_descriptor);
    some(&mut def, "speed", &d.speed);
    some(&mut def, "course", &d.course);
    some(&mut def, "magVar", &d.mag_var);
    some(&mut def, "rho", &d.rho);
    some(&mut def, "theta", &d.theta);
    some(&mut def, "length", &d.length);
    some(&mut def, "lengthTime", &d.length_time);
    if let Some(n) = &d.recommended_navaid {
        def.insert("recommendedNavaid".into(), fix_json(n));
    }
    some(&mut def, "verticalAngle", &d.vertical_angle);
    if d.is_engine_out_branch {
        def.insert("isEngineOutBranch".into(), json!(true));
    }
    let def = Value::Object(def);
    let mut m = Map::new();
    m.insert("uuid".into(), json!(uuid_for(&format!("{position}|{}|{def}", leg.ident))));
    m.insert("ident".into(), json!(leg.ident));
    m.insert("flags".into(), json!(leg.flags));
    m.insert("annotation".into(), json!(leg.annotation));
    m.insert("isDiscontinuity".into(), json!(false));
    m.insert("definition".into(), def);
    m.insert("effectiveType".into(), json!(format!("{:?}", leg.leg_type)));
    some(&mut m, "modifiedHold", &leg.modified_hold);
    some(&mut m, "defaultHold", &leg.default_hold);
    m.insert("constraintType".into(), json!(leg.constraint_type));
    some(&mut m, "cruiseStep", &leg.cruise_step);
    if let Some(a) = &leg.pilot_altitude {
        m.insert("pilotEnteredAltitudeConstraint".into(), crate::vnav::constraint_json(a));
    }
    if let Some(s) = &leg.pilot_speed {
        m.insert("pilotEnteredSpeedConstraint".into(), json!({ "speedDescriptor": s.descriptor, "speed": s.speed }));
    }
    m.insert("cruiseWindEntries".into(), json!(leg.cruise_wind_entries));
    Value::Object(m)
}

/// FBW `FlightPlanSegment.serialize`.
pub fn segment_json(plan: &FlightPlan, index: usize) -> Value {
    let Some(seg) = plan.segments.get(index) else { return json!({ "allLegs": [] }) };
    let legs: Vec<Value> = seg
        .legs
        .iter()
        .enumerate()
        .map(|(k, e)| match e {
            Element::Leg(l) => leg_json(l, k),
            Element::Disco => json!({ "isDiscontinuity": true }),
        })
        .collect();
    let mut m = Map::new();
    m.insert("allLegs".into(), Value::Array(legs));
    let (airport, runway) = match seg.kind {
        SegmentKind::Origin => (plan.origin_airport.as_ref(), plan.origin_runway.as_ref()),
        SegmentKind::Destination => (plan.destination_airport.as_ref(), plan.destination_runway.as_ref()),
        _ => (None, None),
    };
    if let Some(a) = airport {
        m.insert("facilityIdent".into(), json!(a.ident));
    }
    if let Some(r) = runway {
        m.insert("runwayIdent".into(), json!(r.fix.ident));
    }
    let procedure = match seg.kind {
        SegmentKind::Departure => plan.departure.get(),
        SegmentKind::Arrival => plan.arrival.get(),
        SegmentKind::Approach => plan.approach.as_ref(),
        _ => None,
    };
    if let Some(p) = procedure {
        m.insert("procedureIdent".into(), json!(p.ident));
        m.insert("procedureDatabaseId".into(), json!(p.database_id));
    }
    Value::Object(m)
}

fn perf_map(plan: &FlightPlan, perf: Option<&PerformanceData>) -> Map<String, Value> {
    let mut m = Map::new();
    if let Ok(Value::Object(o)) = serde_json::to_value(&plan.performance) {
        for (k, v) in o {
            m.insert(camel(&k), v);
        }
    }
    if let Some(Ok(Value::Object(o))) = perf.map(serde_json::to_value) {
        m.extend(o);
    }
    m
}

fn state_of(plan: &FlightPlan, perf: Option<&PerformanceData>) -> PlanState {
    let segs = |p: &FlightPlan| (0..p.segments.len()).map(|i| segment_json(p, i)).collect::<Vec<_>>();
    PlanState {
        flight_number: plan.flight_number.clone(),
        active_leg_index: plan.active_leg_index,
        segments: [segs(plan), plan.alternate.as_deref().map(segs).unwrap_or_default()],
        perf: perf_map(plan, perf),
    }
}

/// FBW's flight plan sync stream (`flightPlanManager.*`, `flightPlan.*` and their `SYNC_` twins),
/// rebuilt by diffing the plans after each command and tick.
pub struct PlanSync {
    last: Vec<Option<PlanState>>,
}

impl Default for PlanSync {
    fn default() -> Self {
        PlanSync { last: vec![None; 6] }
    }
}

fn emit(sink: &mut Sink, topic: &str, data: Value) {
    sink.bus(topic, data.clone());
    sink.bus(&format!("SYNC_{topic}"), data);
}

fn edit(plan_index: usize, alternate: bool) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("syncClientID".into(), json!(SYNC_CLIENT_ID));
    m.insert("planIndex".into(), json!(plan_index));
    m.insert("batchStack".into(), json!([]));
    m.insert("forAlternate".into(), json!(alternate));
    m
}

impl PlanSync {
    pub fn copied(&mut self, from: usize, to: usize, options: u32, sink: &mut Sink) {
        emit(sink, "flightPlanManager.copy", json!({ "syncClientID": SYNC_CLIENT_ID, "planIndex": from, "targetPlanIndex": to, "options": options }));
        if let Some(s) = self.last.get(from).cloned() {
            self.last[to] = s;
        }
    }

    pub fn swapped(&mut self, a: usize, b: usize, sink: &mut Sink) {
        emit(sink, "flightPlanManager.swap", json!({ "syncClientID": SYNC_CLIENT_ID, "planIndex": a, "targetPlanIndex": b }));
        self.last.swap(a, b);
    }

    pub fn update(&mut self, plans: &[Option<FlightPlan>], perf: &BTreeMap<usize, PerformanceData>, sink: &mut Sink) {
        for (index, plan) in plans.iter().enumerate().take(self.last.len()) {
            let Some(plan) = plan else {
                if self.last[index].take().is_some() {
                    emit(sink, "flightPlanManager.delete", json!({ "syncClientID": SYNC_CLIENT_ID, "planIndex": index }));
                }
                continue;
            };
            let now = state_of(plan, perf.get(&index));
            let before = match self.last[index].take() {
                Some(s) => s,
                None => {
                    emit(sink, "flightPlanManager.create", json!({ "syncClientID": SYNC_CLIENT_ID, "planIndex": index }));
                    state_of(&FlightPlan::empty(index), Some(&PerformanceData::default()))
                }
            };
            if before != now {
                Self::diff(index, &before, &now, sink);
            }
            self.last[index] = Some(now);
        }
    }

    fn diff(index: usize, before: &PlanState, now: &PlanState, sink: &mut Sink) {
        for (alt, (b, n)) in before.segments.iter().zip(&now.segments).enumerate() {
            for (k, seg) in n.iter().enumerate() {
                if b.get(k) != Some(seg) {
                    let mut m = edit(index, alt == 1);
                    m.insert("segmentIndex".into(), json!(k));
                    m.insert("serialized".into(), seg.clone());
                    emit(sink, "flightPlan.setSegment", Value::Object(m));
                }
            }
        }
        if before.active_leg_index != now.active_leg_index {
            let mut m = edit(index, false);
            m.insert("activeLegIndex".into(), json!(now.active_leg_index));
            emit(sink, "flightPlan.setActiveLegIndex", Value::Object(m));
        }
        if before.flight_number != now.flight_number {
            let mut m = edit(index, false);
            m.insert("flightNumber".into(), json!(now.flight_number));
            emit(sink, "flightPlan.setFlightNumber", Value::Object(m));
        }
        for (k, v) in &now.perf {
            if before.perf.get(k) != Some(v) {
                sink.bus(&format!("flightPlan.setPerformanceData.{k}"), json!({ "planIndex": index, "forAlternate": false, "value": v }));
            }
        }
    }
}

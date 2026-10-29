use fms::navdb::{MemoryNavDb, NavDatabase};
use fms::plan::FlightPlan;
use fms::revisions::{SavedFlightPlan, SAVED_FLIGHT_PLAN_FORMAT};
use fms::service::{Command, Fms, PlanRef};

fn db() -> MemoryNavDb {
    MemoryNavDb::from_fixture_json(include_str!("fixtures/navdb.json")).expect("fixture")
}

fn city_pair_plan(db: &MemoryNavDb) -> FlightPlan {
    let mut fms = Fms::new();
    fms.execute(db, Command::NewCityPair { from: "EGLL".into(), to: "KJFK".into(), alternate: Some("KBOS".into()), plan: PlanRef::Active }).expect("city pair");
    let runway = fms.active().unwrap().available_origin_runways.iter().map(|r| r.fix.ident.clone()).min().expect("runway");
    fms.execute(db, Command::SetOriginRunway { runway, plan: PlanRef::Active, alternate: false }).expect("runway");
    fms.execute(db, Command::TemporaryInsert).expect("insert");
    fms.execute(db, Command::Sequence { plan: PlanRef::Active }).expect("sequence");
    fms.active().cloned().expect("active")
}

#[test]
fn a_saved_plan_loads_back_identically() {
    let db = db();
    let plan = city_pair_plan(&db);
    let json = plan.serialize_json();
    let back = FlightPlan::deserialize_json(&json, Some(&db as &dyn NavDatabase)).expect("load");
    assert_eq!(back.segments, plan.segments);
    assert_eq!(back.active_leg_index, plan.active_leg_index);
    assert_eq!(back.origin_airport, plan.origin_airport);
    assert_eq!(back.destination_airport, plan.destination_airport);
    assert_eq!(back.origin_runway, plan.origin_runway);
    assert_eq!(back.performance, plan.performance);
    assert_eq!(back.alternate.as_ref().map(|a| a.segments.clone()), plan.alternate.as_ref().map(|a| a.segments.clone()));
    assert_eq!(back.available_origin_runways, plan.available_origin_runways);
    assert_eq!(back.serialize_json(), json);
}

#[test]
fn a_saved_plan_carries_its_format_version() {
    let db = db();
    let json = city_pair_plan(&db).serialize_json();
    let saved: SavedFlightPlan = serde_json::from_str(&json).expect("json");
    assert_eq!(saved.format, SAVED_FLIGHT_PLAN_FORMAT);
    let future = json.replacen(&format!("\"format\":{SAVED_FLIGHT_PLAN_FORMAT}"), "\"format\":999", 1);
    assert!(FlightPlan::deserialize_json(&future, None).is_err());
}

#[test]
fn origin_and_destination_set_default_performance_data() {
    let db = db();
    let plan = city_pair_plan(&db);
    let origin_alt = plan.origin_airport.as_ref().and_then(|a| a.location.alt).expect("origin elevation");
    assert_eq!(plan.thrust_reduction_altitude(), Some(origin_alt + 1500.));
    assert_eq!(plan.acceleration_altitude(), Some(origin_alt + 1500.));
    let dest_alt = plan.destination_airport.as_ref().and_then(|a| a.location.alt).expect("destination elevation");
    assert_eq!(plan.performance.default_missed_acceleration_altitude, Some(dest_alt + 1500.));
    assert_eq!(plan.alternate.as_ref().and_then(|a| a.origin_airport.as_ref()).map(|a| a.ident.as_str()), Some("KJFK"));
}

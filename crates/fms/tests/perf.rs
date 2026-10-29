use fms::navdb::MemoryNavDb;
use fms::service::{Command, Fms, PlanRef};
use fms_data::command::{Command as PerfCommand, Effect, Event, WindPhase};
use fms_data::entry::Entry;

fn db() -> MemoryNavDb {
    MemoryNavDb::from_fixture_json(include_str!("fixtures/navdb.json")).expect("fixture")
}

#[test]
fn perf_store_follows_plans_and_routes_wind_effects() {
    let db = db();
    let mut fms = Fms::new();
    fms.execute(&db, Command::NewCityPair { from: "EGLL".into(), to: "KJFK".into(), alternate: None, plan: PlanRef::Active }).unwrap();
    let pd = fms.perf_data(0).expect("store entry for the active plan").clone();
    assert!(pd.default_thrust_reduction_altitude.is_some());
    assert_eq!(pd.climb_speed_limit_speed, Some(250.));
    assert_eq!(pd.alternate_climb_speed_limit_altitude, Some(10_000.));
    assert_eq!(fms.active().unwrap().performance.climb_speed_limit_speed, Some(250.));

    fms.execute(&db, Command::PerfData(PerfCommand::Enter { plan: 0, entry: Entry::CostIndex(Some(42.)) })).unwrap();
    assert_eq!(fms.active().unwrap().performance.cost_index, Some(42.));

    fms.execute(&db, Command::PerfData(PerfCommand::SetWind { plan: 0, phase: WindPhase::Climb, altitude: 20000., direction: 270., magnitude: 40., cruise_index: None })).unwrap();
    let active = fms.active().unwrap();
    assert_eq!(active.draft_winds.climb.as_ref().map(|d| d.len()), Some(1));

    let events = fms.execute_perf(PerfCommand::Enter { plan: 0, entry: Entry::ApproachWindDirection(Some(220.)) }).unwrap();
    assert!(events.iter().any(|e| matches!(e, Event::Effect(Effect::SetDescentGroundWind { .. }))));
    assert_eq!(fms.active().unwrap().winds.approach_wind_direction, Some(220.));

    fms.execute(&db, Command::SecondaryCopyFromActive { index: 1 }).unwrap();
    assert_eq!(fms.perf_data(PlanRef::Secondary(1).slot()).and_then(|p| p.cost_index), Some(42.));
    fms.execute(&db, Command::PerfData(PerfCommand::Enter { plan: 0, entry: Entry::ZeroFuelWeightKg(Some(330_000.)) })).unwrap();
    fms.execute(&db, Command::PerfData(PerfCommand::Enter { plan: PlanRef::Secondary(1).slot(), entry: Entry::CostIndex(Some(80.)) })).unwrap();
    fms.execute(&db, Command::ActiveAndSecondarySwap { index: 1 }).unwrap();
    let swapped = fms.perf_data(0).unwrap();
    assert_eq!(swapped.cost_index, Some(80.));
    assert_eq!(swapped.zero_fuel_weight, Some(330.));
    assert_eq!(fms.perf_data(PlanRef::Secondary(1).slot()).and_then(|p| p.cost_index), Some(42.));

    fms.delete(PlanRef::Secondary(1));
    assert!(fms.perf_data(PlanRef::Secondary(1).slot()).is_none());
    let (bits, _) = fms.perf_update();
    assert!(bits.to_speeds_not_inserted);
}

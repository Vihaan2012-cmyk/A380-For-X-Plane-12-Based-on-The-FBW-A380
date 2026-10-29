use fms::leg_utils::merge_leg_sets;
use fms::navdb::{Airway, Fix, Location, MemoryNavDb, NavDatabase, Runway};
use fms::plan::{Element, FlightPlan, Leg, LegType};
use fms::service::{Command, Fms, PlanRef};

fn fake_airport(icao_code: &str, ident: &str, lat: f64, long: f64, alt: f64) -> Fix {
    Fix {
        database_id: format!("AIRPORT-{icao_code}-{ident}"),
        ident: ident.to_string(),
        icao_code: icao_code.to_string(),
        location: Location { lat, long, alt: Some(alt) },
        section_code: 4,
        sub_section_code: 0,
        area: 1,
        airport_ident: Some(ident.to_string()),
        mag_var: Some(0.),
        ..Fix::default()
    }
}

fn fake_runway(icao_code: &str, airport_ident: &str, number: &str, designator: i32, letter: &str, bearing: f64, lat: f64, long: f64) -> Runway {
    let ident = format!("{airport_ident}{number:0>2}{letter}");
    let location = Location { lat, long, alt: None };
    Runway {
        fix: Fix {
            database_id: format!("RUNWAY-{icao_code}-{airport_ident}-{number:0>2}{letter}"),
            ident,
            icao_code: icao_code.to_string(),
            location,
            section_code: 4,
            sub_section_code: 6,
            area: 1,
            airport_ident: Some("CYYZ".to_string()),
            mag_var: Some(0.),
            ..Fix::default()
        },
        bearing,
        magnetic_bearing: bearing,
        length: 0.,
        number: number.parse().unwrap_or(0),
        designator,
        threshold_location: Location { alt: Some(0.), ..location },
    }
}

fn fake_waypoint(icao_code: &str, ident: &str) -> Fix {
    Fix {
        database_id: format!("WAYPOINT-{icao_code}-{ident}"),
        ident: ident.to_string(),
        icao_code: icao_code.to_string(),
        location: Location { lat: 0., long: 0., alt: None },
        section_code: 2,
        sub_section_code: 0,
        area: 0,
        ..Fix::default()
    }
}

fn test_database() -> MemoryNavDb {
    let nosus = fake_waypoint("CY", "NOSUS");
    let debus = fake_waypoint("CY", "DEBUS");
    let dutil = fake_waypoint("CY", "DUTIL");
    let mut db = MemoryNavDb {
        fixes: vec![fake_airport("CY", "CYYZ", 43.67607222222222, -79.63047499999999, 569.), fake_airport("CY", "CYVR", 0., 0., 0.), nosus.clone(), debus.clone(), dutil.clone()],
        airways: vec![Airway { ident: "A1".to_string(), fixes: vec![nosus, debus, dutil] }],
        ..MemoryNavDb::default()
    };
    db.runways.insert("CYYZ".to_string(), vec![fake_runway("CY", "CYYZ", "06", 3, "R", 46., 43.66679444444444, -79.60958194444444)]);
    db.runways.insert("CYVR".to_string(), vec![fake_runway("CY", "CYVR", "08", 3, "R", 46., 43.66679444444444, -79.60958194444444)]);
    db
}

fn fixture_database() -> MemoryNavDb {
    MemoryNavDb::from_fixture_json(include_str!("fixtures/navdb.json")).expect("fixture")
}

fn leg(plan: &FlightPlan, index: usize) -> &Leg {
    plan.element_at(index).and_then(Element::leg).unwrap_or_else(|| panic!("element {index} is not a leg"))
}

#[test]
fn a_base_flight_plan_can_insert_a_leg() {
    let db = test_database();
    let mut fp = FlightPlan::empty(0);

    fp.set_origin_airport(&db, Some("CYVR")).unwrap();
    fp.set_destination_airport(&db, Some("CYYZ")).unwrap();

    let waypoint = db.search_fix("NOSUS");
    fp.insert_waypoint_before(1, &waypoint[0]).unwrap();

    assert_eq!(leg(&fp, 1).ident, "NOSUS");
    assert!(fp.element_at(2).unwrap().is_disco());
    assert_eq!(fp.leg_count(), 4);
}

#[test]
fn a_base_flight_plan_can_insert_an_airway() {
    let db = test_database();
    let mut fp = FlightPlan::empty(0);
    fp.set_origin_airport(&db, Some("CYYZ")).unwrap();
    fp.set_destination_airport(&db, Some("CYVR")).unwrap();

    let fix = db.search_fix("NOSUS");
    fp.next_waypoint(1, &fix[0]).unwrap();

    fp.start_airway_entry(2).unwrap();

    let airway = db.search_airway("A1", &fix[0]);
    fp.continue_airway_entry_via_airway(airway[0].clone()).unwrap();

    let termination_fix = db.search_fix("DUTIL");
    fp.continue_airway_entry_to_fix(&termination_fix[0], false).unwrap();

    fp.finalise_airway_entry().unwrap();

    assert_eq!(leg(&fp, 2).termination_waypoint().unwrap().ident, "NOSUS");
    assert_eq!(leg(&fp, 3).termination_waypoint().unwrap().ident, "DEBUS");
    assert_eq!(leg(&fp, 4).termination_waypoint().unwrap().ident, "DUTIL");
}

#[test]
fn a_flight_plan_leg_set_merges_downstream_with_a_matching_waypoint() {
    let code = |s: &&str| -> String { s.to_string() };
    let _ = code;
    let existing = ["Z", "C", "E", "F", "G"];
    let incoming = ["A", "B", "C", "G", "H"];
    let result = merge_leg_sets(&existing, &incoming, true, |s| s).expect("merged");
    for expected in ["A", "B", "C", "E", "F", "G"] {
        assert!(result.contains(&expected), "{expected} missing from {result:?}");
    }
}

#[test]
fn leg_sets_do_not_merge_downstream_without_a_matching_waypoint() {
    assert!(merge_leg_sets(&["A", "B"], &["C", "D"], true, |s| s).is_none());
}

#[test]
fn a_flight_plan_leg_set_merges_upstream_with_a_matching_waypoint() {
    let existing = ["A", "B", "C", "D"];
    let incoming = ["B", "E", "F"];
    let result = merge_leg_sets(&existing, &incoming, false, |s| s).expect("merged");
    for expected in ["A", "B", "E", "F"] {
        assert!(result.contains(&expected), "{expected} missing from {result:?}");
    }
}

#[test]
fn leg_sets_do_not_merge_upstream_without_a_matching_waypoint() {
    assert!(merge_leg_sets(&["A", "B"], &["C", "D"], false, |s| s).is_none());
}

#[test]
fn the_flight_plan_manager_can_create_and_delete_a_flight_plan() {
    let mut fms = Fms::new();
    fms.create(PlanRef::Secondary(1));
    assert!(fms.plan(PlanRef::Secondary(1)).is_some());
    fms.delete(PlanRef::Secondary(1));
    assert!(fms.plan(PlanRef::Secondary(1)).is_none());
    assert!(fms.plan_mut(PlanRef::Secondary(1)).is_err());
}

#[test]
fn the_flight_plan_manager_can_copy_a_flight_plan() {
    let db = test_database();
    let mut fms = Fms::new();
    fms.create(PlanRef::Secondary(1));
    {
        let plan = fms.plan_mut(PlanRef::Secondary(1)).unwrap();
        plan.set_origin_airport(&db, Some("CYYZ")).unwrap();
        plan.set_origin_runway(&db, Some("CYYZ06R")).unwrap();
    }
    fms.copy(PlanRef::Secondary(1), PlanRef::Secondary(2)).unwrap();

    let copied = fms.plan(PlanRef::Secondary(2)).unwrap();
    assert_eq!(copied.origin_airport.as_ref().map(|a| a.ident.as_str()), Some("CYYZ"));
    assert_eq!(copied.origin_runway.as_ref().map(|r| r.fix.ident.as_str()), Some("CYYZ06R"));
}

#[test]
fn the_flight_plan_manager_can_swap_two_flight_plans() {
    let db = test_database();
    let mut fms = Fms::new();
    fms.create(PlanRef::Secondary(1));
    fms.create(PlanRef::Secondary(2));
    {
        let a = fms.plan_mut(PlanRef::Secondary(1)).unwrap();
        a.set_origin_airport(&db, Some("CYYZ")).unwrap();
        a.set_origin_runway(&db, Some("CYYZ06R")).unwrap();
    }
    {
        let b = fms.plan_mut(PlanRef::Secondary(2)).unwrap();
        b.set_origin_airport(&db, Some("CYVR")).unwrap();
        b.set_origin_runway(&db, Some("CYVR08R")).unwrap();
    }
    fms.swap(PlanRef::Secondary(1), PlanRef::Secondary(2)).unwrap();

    let new_a = fms.plan(PlanRef::Secondary(2)).unwrap();
    assert_eq!(new_a.origin_airport.as_ref().map(|a| a.ident.as_str()), Some("CYYZ"));
    assert_eq!(new_a.origin_runway.as_ref().map(|r| r.fix.ident.as_str()), Some("CYYZ06R"));
    let new_b = fms.plan(PlanRef::Secondary(1)).unwrap();
    assert_eq!(new_b.origin_airport.as_ref().map(|a| a.ident.as_str()), Some("CYVR"));
    assert_eq!(new_b.origin_runway.as_ref().map(|r| r.fix.ident.as_str()), Some("CYVR08R"));
}

#[test]
fn the_flight_plan_service_accepts_a_city_pair_without_a_temporary_plan() {
    let db = fixture_database();
    let mut fms = Fms::new();
    fms.execute(&db, Command::NewCityPair { from: "EGLL".into(), to: "KJFK".into(), alternate: Some("KBOS".into()), plan: PlanRef::Active }).unwrap();

    assert!(!fms.has_temporary());
    let active = fms.active().unwrap();
    assert_eq!(active.origin_airport.as_ref().map(|a| a.ident.as_str()), Some("EGLL"));
    assert_eq!(active.destination_airport.as_ref().map(|a| a.ident.as_str()), Some("KJFK"));
    assert_eq!(active.alternate.as_ref().and_then(|a| a.destination_airport.as_ref()).map(|a| a.ident.as_str()), Some("KBOS"));
}

#[test]
fn the_flight_plan_service_deletes_the_temporary_flight_plan_properly() {
    let db = fixture_database();
    let mut fms = Fms::new();
    fms.execute(&db, Command::NewCityPair { from: "EGLL".into(), to: "KJFK".into(), alternate: None, plan: PlanRef::Active }).unwrap();
    fms.execute(&db, Command::SetOriginRunway { runway: "EGLL09L".into(), plan: PlanRef::Active, alternate: false }).unwrap();
    assert!(fms.has_temporary());

    fms.execute(&db, Command::TemporaryDelete).unwrap();

    assert!(!fms.has_temporary());
    assert!(fms.active().unwrap().origin_runway.is_none());
}

#[test]
fn the_flight_plan_service_inserts_the_temporary_flight_plan_properly() {
    let db = fixture_database();
    let mut fms = Fms::new();
    fms.execute(&db, Command::NewCityPair { from: "EGLL".into(), to: "KJFK".into(), alternate: None, plan: PlanRef::Active }).unwrap();
    fms.execute(&db, Command::SetOriginRunway { runway: "EGLL09L".into(), plan: PlanRef::Active, alternate: false }).unwrap();

    fms.execute(&db, Command::TemporaryInsert).unwrap();

    assert!(!fms.has_temporary());
    assert_eq!(fms.active().unwrap().origin_runway.as_ref().map(|r| r.fix.ident.as_str()), Some("EGLL09L"));
}

#[test]
fn the_flight_plan_service_creates_a_temporary_plan_when_changing_active_procedure_details() {
    let db = fixture_database();
    let mut fms = Fms::new();
    fms.execute(&db, Command::NewCityPair { from: "EGLL".into(), to: "KJFK".into(), alternate: None, plan: PlanRef::Active }).unwrap();

    let steps: Vec<Command> = vec![
        Command::SetOriginRunway { runway: "EGLL09L".into(), plan: PlanRef::Active, alternate: false },
        Command::SetDeparture { id: Some("PEGEGLLBPK5K".into()), plan: PlanRef::Active, alternate: false },
        Command::SetDestinationRunway { runway: "KJFK04R".into(), plan: PlanRef::Active, alternate: false },
        Command::SetArrival { id: Some("PK6KJFKCAMRN4".into()), plan: PlanRef::Active, alternate: false },
        Command::SetApproach { id: Some("PK6KJFKVORDME 04R".into()), plan: PlanRef::Active, alternate: false },
    ];
    for step in steps {
        fms.execute(&db, step).unwrap();
        assert!(fms.has_temporary());
        fms.execute(&db, Command::TemporaryInsert).unwrap();
    }
}

#[test]
fn the_flight_plan_service_does_not_create_a_temporary_plan_on_a_secondary_plan() {
    let db = fixture_database();
    let mut fms = Fms::new();
    let secondary = PlanRef::Secondary(1);
    fms.execute(&db, Command::NewCityPair { from: "EGLL".into(), to: "KJFK".into(), alternate: None, plan: secondary }).unwrap();

    let steps: Vec<Command> = vec![
        Command::SetOriginRunway { runway: "EGLL09L".into(), plan: secondary, alternate: false },
        Command::SetDeparture { id: Some("PEGEGLLBPK5K".into()), plan: secondary, alternate: false },
        Command::SetDestinationRunway { runway: "KJFK04R".into(), plan: secondary, alternate: false },
        Command::SetArrival { id: Some("PK6KJFKCAMRN4".into()), plan: secondary, alternate: false },
        Command::SetApproach { id: Some("PK6KJFKVORDME 04R".into()), plan: secondary, alternate: false },
    ];
    for step in steps {
        fms.execute(&db, step).unwrap();
        assert!(!fms.has_temporary());
    }
    let plan = fms.plan(secondary).unwrap();
    assert_eq!(plan.departure.get().map(|d| d.ident.as_str()), Some("BPK5K"));
    assert_eq!(plan.approach.as_ref().map(|a| a.ident.as_str()), Some("D04R"));
}

#[test]
fn an_approach_ending_away_from_the_runway_ends_the_plan_at_its_missed_approach_point() {
    let db = fixture_database();
    let mut fms = Fms::new();
    fms.execute(&db, Command::NewCityPair { from: "EGLL".into(), to: "KJFK".into(), alternate: None, plan: PlanRef::Active }).unwrap();
    fms.execute(&db, Command::SetApproach { id: Some("PK6KJFKVORDME 04R".into()), plan: PlanRef::Active, alternate: false }).unwrap();
    fms.execute(&db, Command::TemporaryInsert).unwrap();

    let plan = fms.active().unwrap();
    let destination = plan.destination_leg_index().expect("destination leg");
    assert_eq!(leg(plan, destination).ident, "MA04R");
    assert_ne!(leg(plan, destination).definition.waypoint_descriptor, Some(fms::plan::DESCRIPTOR_RUNWAY));
    assert_eq!(leg(plan, destination).leg_type, LegType::CF);
}

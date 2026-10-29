use fms::navdb::{Fix, MemoryNavDb, NavDatabase};
use fms::plan::FlightPlan;
use fms::service::{Command, Fms, PlanRef};
use fms::waypoint_entry::{self as wpe, EntryError, WaypointEntry};
use fms::winds::{PendingCruiseFix, PendingCruiseWind, PendingWindUplink, PropagationType, WindEntry, WindVector, NUM_CLIMB_WIND_LEVELS};

fn db() -> MemoryNavDb {
    MemoryNavDb::from_fixture_json(include_str!("fixtures/navdb.json")).expect("fixture")
}

fn first(items: Vec<Fix>) -> Option<Fix> {
    items.into_iter().next()
}

fn plan(db: &MemoryNavDb) -> FlightPlan {
    let mut p = draft_plan(db);
    p.index = 1;
    p
}

fn draft_plan(db: &MemoryNavDb) -> FlightPlan {
    let mut fms = Fms::new();
    fms.execute(db, Command::NewCityPair { from: "EGLL".into(), to: "KJFK".into(), alternate: None, plan: PlanRef::Active }).expect("city pair");
    fms.active().cloned().expect("active")
}

fn wind(altitude: f64, dir_deg: f64, kt: f64) -> WindEntry {
    WindEntry { vector: WindVector { direction: Some(dir_deg.to_radians()), magnitude: Some(kt) }, altitude: Some(altitude), flags: 0 }
}

#[test]
fn lat_lon_entries_parse_like_fbw() {
    let c = wpe::parse_lat_lon("N5130.0/W00030.0").expect("ok");
    assert!((c.lat - 51.5).abs() < 1e-9 && (c.long + 0.5).abs() < 1e-9);
    let c = wpe::parse_lat_lon("5130.0N/00030.0E").expect("ok");
    assert!((c.lat - 51.5).abs() < 1e-9 && (c.long - 0.5).abs() < 1e-9);
    assert_eq!(wpe::parse_lat_lon("5130.0/00030.0"), Err(EntryError::FormatError));
    assert_eq!(wpe::parse_lat_lon("N9130.0/W00030.0"), Err(EntryError::EntryOutOfRange));
    assert!(!wpe::is_lat_lon_format("N51/W000"));
}

#[test]
fn entry_formats_are_recognised() {
    assert!(wpe::is_place_format("CPT"));
    assert!(wpe::is_place_format("EGLL27L"));
    assert!(!wpe::is_place_format("C"));
    assert!(wpe::is_runway_format("KJFK04R"));
    assert!(wpe::is_pbx_format("CPT-090/OLPOP-180"));
    assert!(wpe::is_pbd_format("CPT/090/12.5"));
    assert!(wpe::is_pd_format("CPT/-12.5"));
    assert!(!wpe::is_pbd_format("CPT/0900/12"));
    assert_eq!(wpe::split_pbd("CPT/090/12.5"), ("CPT".to_string(), 90., 12.5));
}

#[test]
fn places_resolve_through_the_database() {
    let db = db();
    let entry = wpe::get_or_create_waypoint(&db, "CPT", &[], &mut first).expect("ok");
    assert!(matches!(entry, WaypointEntry::Place(f) if f.ident == "CPT"));
    let entry = wpe::get_or_create_waypoint(&db, "KBOS", &[], &mut first).expect("ok");
    assert!(matches!(entry, WaypointEntry::Place(f) if f.is_airport()));
    let entry = wpe::get_or_create_waypoint(&db, "ZZZZZ", &[], &mut first).expect("ok");
    assert_eq!(entry, WaypointEntry::NewWaypoint("ZZZZZ".into()));
    let entry = wpe::get_or_create_waypoint(&db, "CPT/090/12", &[], &mut first).expect("ok");
    assert!(matches!(entry, WaypointEntry::PlaceBearingDistance(f, b, d) if f.ident == "CPT" && b == 90. && d == 12.));
    assert_eq!(wpe::get_or_create_waypoint(&db, "CPT/-12", &[], &mut first), Err(EntryError::NotYetImplemented));
    let rwy = db.runways("EGLL").into_iter().next().expect("runway");
    assert_eq!(wpe::parse_runway(&db, &rwy.fix.ident).expect("runway").fix.ident, rwy.fix.ident);
}

#[test]
fn climb_winds_sort_descending_and_snap_to_ground() {
    let db = db();
    let mut p = plan(&db);
    let elevation = p.origin_airport.as_ref().and_then(|a| a.location.alt).unwrap_or(0.);
    p.set_climb_wind_entry(Some(10000.), Some(wind(10000., 270., 30.)));
    p.set_climb_wind_entry(Some(30000.), Some(wind(30000., 280., 60.)));
    p.set_climb_wind_entry(Some(elevation + 100.), Some(wind(elevation + 100., 250., 10.)));
    let altitudes: Vec<Option<f64>> = p.winds.climb.iter().map(|e| e.altitude).collect();
    assert_eq!(altitudes, vec![Some(30000.), Some(10000.), Some(elevation)]);
    p.set_climb_wind_entry(Some(10000.), None);
    assert_eq!(p.winds.climb.len(), 2);
    for n in 0..10 {
        p.set_climb_wind_entry(Some(5000. + n as f64 * 1000.), Some(wind(5000. + n as f64 * 1000., 270., 20.)));
    }
    assert_eq!(p.winds.climb.len(), NUM_CLIMB_WIND_LEVELS);
    let partial = WindEntry { vector: WindVector { direction: Some(1.), magnitude: None }, altitude: Some(20000.), flags: 0 };
    let before = p.winds.climb.clone();
    p.set_climb_wind_entry(Some(20000.), Some(partial));
    assert_eq!(p.winds.climb, before);
}

#[test]
fn ground_descent_wind_sets_the_approach_wind() {
    let db = db();
    let mut p = plan(&db);
    let elevation = p.destination_airport.as_ref().and_then(|a| a.location.alt).unwrap_or(0.);
    p.set_descent_wind_entry(Some(elevation), Some(wind(elevation, 220., 15.)), 13.);
    assert_eq!(p.winds.approach_wind_magnitude, Some(15.));
    assert!((p.winds.approach_wind_direction.unwrap() - 207.).abs() < 1e-9);
}

#[test]
fn cruise_winds_live_on_legs_and_survive_a_save() {
    let db = db();
    let mut p = plan(&db);
    p.execute_sequence_free_insert(&db);
    let at = (0..p.leg_count()).find(|i| p.leg_at(*i).is_some_and(|l| l.is_xf())).expect("xf leg");
    assert!(p.add_cruise_wind_entry(at, wind(35000., 270., 80.)));
    assert!(p.add_cruise_wind_entry(at, wind(35040., 260., 70.)));
    assert_eq!(p.leg_at(at).unwrap().cruise_wind_entries.len(), 1);
    assert!(p.has_wind_entries());
    let back = FlightPlan::deserialize_json(&p.serialize_json(), None).expect("load");
    assert_eq!(back.leg_at(at).unwrap().cruise_wind_entries, p.leg_at(at).unwrap().cruise_wind_entries);
    assert!(p.delete_cruise_wind_entry(at, 35000.));
    assert!(!p.has_wind_entries());
}

trait InsertCpt {
    fn execute_sequence_free_insert(&mut self, db: &MemoryNavDb);
}

impl InsertCpt for FlightPlan {
    fn execute_sequence_free_insert(&mut self, db: &MemoryNavDb) {
        let cpt = db.search_fix("CPT").pop().expect("CPT");
        self.next_waypoint(0, &cpt).expect("insert CPT");
    }
}

struct EosidDb(MemoryNavDb);

impl NavDatabase for EosidDb {
    fn airport(&self, icao: &str) -> Option<Fix> {
        self.0.airport(icao)
    }
    fn runways(&self, airport_icao: &str) -> Vec<fms::navdb::Runway> {
        self.0.runways(airport_icao)
    }
    fn departures(&self, airport_icao: &str) -> Vec<fms::navdb::Procedure> {
        self.0.departures(airport_icao)
    }
    fn arrivals(&self, airport_icao: &str) -> Vec<fms::navdb::Procedure> {
        self.0.arrivals(airport_icao)
    }
    fn approaches(&self, airport_icao: &str) -> Vec<fms::navdb::Procedure> {
        self.0.approaches(airport_icao)
    }
    fn search_fix(&self, ident: &str) -> Vec<Fix> {
        self.0.search_fix(ident)
    }
    fn search_airway(&self, ident: &str, via: &Fix) -> Vec<fms::navdb::Airway> {
        self.0.search_airway(ident, via)
    }
    fn navaids(&self, ident: &str) -> Vec<Fix> {
        self.0.navaids(ident)
    }
    fn engine_out_departure(&self, airport_icao: &str, departure_database_id: &str, runway_ident: &str) -> Option<fms::navdb::Procedure> {
        let mut eosid = self.0.departures(airport_icao).into_iter().find(|d| d.database_id == departure_database_id)?;
        eosid.ident = "EOSID".into();
        let legs = eosid.common_legs.clone();
        eosid.runway_transitions = vec![fms::navdb::Transition { database_id: "EOSID-RW".into(), ident: runway_ident.to_string(), legs }];
        Some(eosid)
    }
}

#[test]
fn the_engine_out_departure_follows_the_runway_and_sid() {
    let db = EosidDb(db());
    let mut fms = Fms::new();
    fms.execute(&db, Command::NewCityPair { from: "EGLL".into(), to: "KJFK".into(), alternate: None, plan: PlanRef::Active }).expect("city pair");
    let p = fms.active().unwrap();
    let (runway, sid) = p
        .available_origin_runways
        .iter()
        .find_map(|r| db.departures("EGLL").into_iter().find(|d| d.runway_transitions.iter().any(|t| t.ident == r.fix.ident)).map(|d| (r.fix.ident.clone(), d.database_id)))
        .expect("a runway with a SID transition");
    fms.execute(&db, Command::SetOriginRunway { runway, plan: PlanRef::Active, alternate: false }).expect("runway");
    fms.execute(&db, Command::SetDeparture { id: Some(sid), plan: PlanRef::Active, alternate: false }).expect("sid");
    let tmpy = fms.plan(PlanRef::Temporary).expect("tmpy");
    assert_eq!(tmpy.engine_out_departure.procedure_ident.as_deref(), Some("EOSID"));
    assert!(!tmpy.engine_out_legs().is_empty());
    assert!(tmpy.engine_out_legs().iter().filter_map(|e| e.leg()).all(|l| l.constraint_type == fms::plan::CONSTRAINT_CLB));
    fms.execute(&db, Command::SetDeparture { id: None, plan: PlanRef::Active, alternate: false }).expect("clear sid");
    assert!(fms.plan(PlanRef::Temporary).unwrap().engine_out_legs().is_empty());
}

fn two_fix_plan(db: &MemoryNavDb, index: usize) -> (FlightPlan, usize, usize) {
    let mut p = draft_plan(db);
    p.index = index;
    let cpt = db.search_fix("CPT").pop().expect("CPT");
    p.next_waypoint(0, &cpt).expect("CPT");
    let a = p.find_leg_index_by_fix_ident("CPT").expect("CPT leg");
    let ipnox = db.search_fix("IPNOX").pop().expect("IPNOX");
    p.next_waypoint(a, &ipnox).expect("IPNOX");
    let b = p.find_leg_index_by_fix_ident("IPNOX").expect("IPNOX leg");
    (p, a, b)
}

#[test]
fn draft_winds_hold_edits_until_inserted() {
    let db = db();
    let mut p = draft_plan(&db);
    assert!(p.draft_winds_enabled());
    p.set_climb_wind_entry(Some(10000.), Some(wind(10000., 270., 30.)));
    assert!(p.winds.climb.is_empty());
    assert_eq!(p.climb_wind_entries().len(), 1);
    let partial = WindEntry { vector: WindVector { direction: Some(1.), magnitude: None }, altitude: Some(20000.), flags: 0 };
    p.set_climb_wind_entry(Some(20000.), Some(partial));
    assert_eq!(p.climb_wind_entries().len(), 2);
    p.set_alternate_wind(WindVector { direction: Some(1.), magnitude: Some(20.) });
    assert_eq!(p.winds.alternate, WindVector::default());
    assert!(p.has_draft_wind_entries());
    p.insert_draft_wind_entries();
    assert!(!p.has_draft_wind_entries());
    let altitudes: Vec<Option<f64>> = p.winds.climb.iter().map(|e| e.altitude).collect();
    assert_eq!(altitudes, vec![Some(10000.)]);
    assert_eq!(p.winds.alternate.magnitude, Some(20.));
    p.set_climb_wind_entry(Some(30000.), Some(wind(30000., 280., 60.)));
    p.delete_draft_wind_entries();
    assert_eq!(p.climb_wind_entries().len(), 1);
}

#[test]
fn a_destructive_edit_on_the_active_plan_inserts_the_draft_winds() {
    let db = db();
    let mut fms = Fms::new();
    fms.execute(&db, Command::NewCityPair { from: "EGLL".into(), to: "KJFK".into(), alternate: None, plan: PlanRef::Active }).expect("city pair");
    fms.execute(&db, Command::SetClimbWindEntry { altitude: Some(10000.), entry: Some(wind(10000., 270., 30.)), plan: PlanRef::Active }).expect("wind");
    assert!(fms.active().unwrap().winds.climb.is_empty());
    let cpt = db.search_fix("CPT").pop().expect("CPT");
    fms.execute(&db, Command::NextWaypoint { index: 0, waypoint: cpt, plan: PlanRef::Active }).expect("insert");
    assert_eq!(fms.active().unwrap().winds.climb.len(), 1);
    assert!(!fms.active().unwrap().has_draft_wind_entries());
    let tmpy = fms.plan(PlanRef::Temporary).expect("tmpy");
    assert!(!tmpy.draft_winds_enabled() && tmpy.winds.climb.len() == 1);
}

#[test]
fn cruise_winds_propagate_forward_and_backward() {
    let db = db();
    let (mut p, a, b) = two_fix_plan(&db, 1);
    assert!(p.add_cruise_wind_entry(a, wind(35000., 270., 80.)));
    assert!(p.add_cruise_wind_entry(b, wind(30000., 250., 60.)));
    let at_b = p.propagate_winds_at(b);
    assert_eq!(at_b.iter().map(|e| (e.altitude, e.kind, e.source_leg_index)).collect::<Vec<_>>(), vec![(Some(35000.), PropagationType::Forward, a), (Some(30000.), PropagationType::Entry, b)]);
    let at_a = p.propagate_winds_at(a);
    assert_eq!(at_a.iter().map(|e| e.kind).collect::<Vec<_>>(), vec![PropagationType::Entry, PropagationType::Backward]);
    assert!(p.edit_cruise_wind_entry(b, 0, wind(35000., 200., 40.)));
    let at_b = p.propagate_winds_at(b);
    assert_eq!((at_b[0].kind, at_b[0].source_leg_index), (PropagationType::Entry, b));
    assert_eq!(p.leg_at(a).unwrap().cruise_wind_entries.len(), 1);
    assert!(p.edit_cruise_wind_entry(b, 0, wind(33000., 200., 40.)));
    assert!(p.leg_at(a).unwrap().cruise_wind_entries.is_empty() || p.leg_at(a).unwrap().cruise_wind_entries.iter().all(|e| e.altitude != Some(33000.)));
    assert!(p.leg_at(b).unwrap().cruise_wind_entries.iter().any(|e| e.altitude == Some(33000.)));
}

#[test]
fn cruise_wind_drafts_cover_the_legs_ahead() {
    let db = db();
    let (mut p, a, _) = two_fix_plan(&db, 0);
    assert!(p.add_cruise_wind_entry(a, wind(35000., 270., 80.)));
    assert!(p.leg_at(a).unwrap().cruise_wind_entries.is_empty());
    assert_eq!(p.propagate_winds_at(a).len(), 1);
    p.insert_draft_wind_entries();
    assert_eq!(p.leg_at(a).unwrap().cruise_wind_entries.len(), 1);
}

#[test]
fn a_wind_uplink_replaces_all_winds_without_drafts() {
    let db = db();
    let (mut p, _, _) = two_fix_plan(&db, 0);
    let dest = p.destination_airport.as_ref().and_then(|a| a.location.alt).unwrap_or(0.);
    assert!(p.insert_wind_uplink().is_err());
    p.pending_wind_uplink = PendingWindUplink {
        climb_winds: Some(vec![wind(5000., 250., 20.), wind(25000., 270., 50.)]),
        cruise_winds: Some(vec![PendingCruiseWind { fix: PendingCruiseFix::Waypoint("IPNOX".into()), levels: vec![wind(36000., 280., 90.)] }]),
        descent_winds: Some(vec![wind(dest, 200., 12.)]),
        alternate_wind: Some(wind(10000., 300., 25.)),
        ..Default::default()
    };
    p.pending_wind_uplink.on_uplink_ready_to_insert();
    p.insert_wind_uplink().expect("insert");
    assert!(!p.has_draft_wind_entries());
    assert_eq!(p.winds.climb.iter().map(|e| e.altitude).collect::<Vec<_>>(), vec![Some(25000.), Some(5000.)]);
    let ipnox = p.find_leg_index_by_fix_ident("IPNOX").unwrap();
    assert_eq!(p.leg_at(ipnox).unwrap().cruise_wind_entries.len(), 1);
    assert_eq!(p.winds.approach_wind_magnitude, Some(12.));
    assert_eq!(p.winds.alternate.magnitude, Some(25.));
    assert!(!p.pending_wind_uplink.is_wind_uplink_ready_to_insert());
}

#[test]
fn engine_out_sids_are_matched_by_ident_and_branch() {
    use fms::navdb::{engine_out_departure_from, is_engine_out_sid_ident, match_engine_out_departures, Procedure, ProcedureLeg, Transition};
    assert!(is_engine_out_sid_ident("EO27L") && is_engine_out_sid_ident("EO09") && is_engine_out_sid_ident("EO27R-A1"));
    assert!(!is_engine_out_sid_ident("CPT3G") && !is_engine_out_sid_ident("EO2") && !is_engine_out_sid_ident("EO27X"));
    let fix = |id: &str| Some(Fix { database_id: id.into(), ident: id.into(), ..Fix::default() });
    let leg = |t: &str, id: &str| ProcedureLeg { leg_type: t.into(), waypoint: fix(id), ..ProcedureLeg::default() };
    let sid = Procedure {
        database_id: "SID1".into(),
        ident: "CPT3G".into(),
        runway_transitions: vec![Transition { database_id: "t".into(), ident: "EGLL27L".into(), legs: vec![leg("CA", ""), leg("TF", "A"), leg("TF", "B"), leg("TF", "C")] }],
        ..Procedure::default()
    };
    let eo = Procedure {
        database_id: "EO1".into(),
        ident: "EO27L".into(),
        runway_transitions: vec![Transition { database_id: "e".into(), ident: "EGLL27L".into(), legs: vec![leg("CA", ""), leg("TF", "A"), leg("TF", "X")] }],
        ..Procedure::default()
    };
    let mut deps = vec![sid, eo];
    let matched = match_engine_out_departures(&mut deps);
    assert_eq!(matched.iter().filter(|m| m.0 == "SID1").count(), 1);
    assert!(deps[0].runway_transitions[0].legs[1].is_engine_out_branch);
    assert_eq!(deps[0].runway_transitions[0].legs.iter().filter(|l| l.is_engine_out_branch).count(), 1);
    assert_eq!(engine_out_departure_from(&deps, "SID1", "EGLL27L").map(|p| p.ident), Some("EO27L".into()));
    assert!(engine_out_departure_from(&deps, "SID1", "EGLL09R").is_none());
}

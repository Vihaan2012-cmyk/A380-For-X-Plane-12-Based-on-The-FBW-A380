use fms_vnav::atmosphere as atm;
use fms_vnav::config::{AircraftConfig, FlapConf};
use fms_vnav::engine as eng;
use fms_vnav::profile::*;
use fms_vnav::speed_profile::McduSpeedProfile;
use fms_vnav::strategy::ClimbThrustClimbStrategy;
use fms_vnav::{climb::*, cruise::*, flight_model as fm, takeoff::*};

fn params() -> ComputationParameters {
    ComputationParameters {
        present_altitude: 0.0,
        fcu_altitude: 0.0,
        fcu_speed: -1.0,
        managed_climb_speed: 300.0,
        managed_climb_speed_mach: 0.85,
        managed_cruise_speed: 300.0,
        managed_cruise_speed_mach: 0.85,
        managed_descent_speed: 300.0,
        managed_descent_speed_mach: 0.85,
        zero_fuel_weight: 770_000.0,
        fuel_on_board: 330_000.0,
        v2_speed: 150.0,
        tropopause: 36090.0,
        perf_factor: 0.0,
        departure_elevation: 80.0,
        destination_elevation: 13.0,
        acceleration_altitude: 1580.0,
        thrust_reduction_altitude: 1580.0,
        cruise_altitude: 36000.0,
        climb_speed_limit: Some(SpeedLimit { speed: 250.0, under_altitude: 10000.0 }),
        descent_speed_limit: Some(SpeedLimit { speed: 250.0, under_altitude: 10000.0 }),
        flight_phase: FlightPhase::Preflight,
        preselected_clb_speed: -1.0,
        preselected_cruise_speed: -1.0,
        takeoff_flaps: FlapConf::Conf1,
        takeoff_toga_n1: 90.0,
        isa_deviation: 0.0,
        approach_speed: 140.0,
        flap_retraction_speed: 180.0,
        slat_retraction_speed: 210.0,
        clean_speed: 240.0,
        gross_weight_valid: true,
    }
}

#[test]
fn isa_values_match_the_standard_atmosphere() {
    assert!((atm::delta(0.0, false) - 1.0).abs() < 1e-12);
    assert!((atm::delta(36089.24, false) - 0.22336).abs() < 5e-4);
    assert!((atm::delta(36089.24, true) - 0.22336).abs() < 1e-9);
    assert!((atm::theta(36089.0, 0.0, true) - 0.7519).abs() < 1e-3);
    assert!((atm::isa_temp(0.0, false) - 15.0).abs() < 1e-12);
}

#[test]
fn speed_conversions_round_trip() {
    for &(alt, cas) in &[(0.0, 250.0), (10000.0, 250.0), (30000.0, 300.0), (36000.0, 280.0)] {
        let above = alt > 36090.0;
        let delta = atm::delta(alt, above);
        let theta = atm::theta(alt, 0.0, above);
        let mach = atm::cas_to_mach(cas, delta);
        assert!((atm::mach_to_cas(mach, delta) - cas).abs() < 0.2, "{alt} {cas}");
        let tas = atm::cas_to_tas(cas, theta, delta);
        assert!((atm::tas_to_cas(tas, theta, delta) - cas).abs() < 0.2);
        assert!((atm::mach_to_tas(mach, theta) - tas).abs() < 1.0);
    }
    assert!((atm::cas_to_mach(250.0, 1.0) - 0.3787).abs() < 1e-3);
}

#[test]
fn engine_tables_interpolate() {
    let c = AircraftConfig::a380();
    let thrust_frac = eng::table_interpolation(&c.engine.table1506, 85.0, 0.0);
    assert!((thrust_frac - 1.23).abs() < 1e-9);
    let n1 = eng::reverse_table_interpolation(&c.engine.table1506, 0.0, 1.23);
    assert!((n1 - 85.0).abs() < 1e-6);
    let cn1 = eng::climb_thrust_corrected_n1(&c.engine, 35000.0, -54.0);
    assert!(cn1 > 85.0 && cn1 < 105.0, "{cn1}");
    let idle = eng::idle_corrected_n1(&c.engine, 20000.0, 0.6, 36090.0);
    assert!(idle > 10.0 && idle < 60.0, "{idle}");
}

#[test]
fn level_flight_drag_balances_weight_at_cruise() {
    let c = AircraftConfig::a380();
    let delta = atm::delta(36000.0, false);
    let drag = fm::drag(&c.flight, 1_000_000.0, 0.85, delta, false, false, FlapConf::Clean);
    assert!(drag > 50_000.0 && drag < 110_000.0, "{drag}");
}

#[test]
fn takeoff_climb_cruise_profile_is_monotone() {
    let cfg = AircraftConfig::a380();
    let p = params();
    let mut profile = Profile::new(&cfg, Box::new(ConstantWinds::default()));
    profile.max_climb_speed_constraints = vec![];
    build_takeoff_path(&mut profile, &p);
    assert_eq!(profile.checkpoints.len(), 3);
    assert_eq!(profile.checkpoints[2].reason, Reason::AccelerationAltitude);

    let sp = McduSpeedProfile::new(&p, 0.0, vec![], vec![]);
    let strategy = ClimbThrustClimbStrategy { params: &p, atm: p.atmosphere(), config: &cfg };
    ClimbPathBuilder::new(&p).compute_climb_path(&mut profile, &strategy, &sp, p.cruise_altitude, FcuClimbHint::default());

    let toc = *profile.last();
    assert_eq!(toc.reason, Reason::TopOfClimb);
    assert!((toc.altitude - 36000.0).abs() < 11.0, "{}", toc.altitude);
    assert!(toc.distance_from_start > 100.0 && toc.distance_from_start < 450.0, "{}", toc.distance_from_start);
    assert!(toc.seconds_from_present > 900.0 && toc.seconds_from_present < 3600.0, "{}", toc.seconds_from_present);
    let burned = p.fuel_on_board - toc.remaining_fuel_on_board;
    assert!(burned > 10_000.0 && burned < 110_000.0, "{burned}");
    for w in profile.checkpoints.windows(2) {
        assert!(w[1].distance_from_start >= w[0].distance_from_start - 1e-9);
        assert!(w[1].seconds_from_present >= w[0].seconds_from_present - 1e-9);
        assert!(w[1].remaining_fuel_on_board <= w[0].remaining_fuel_on_board + 1e-6);
    }
    let limit = profile.checkpoints.iter().find(|c| c.reason == Reason::CrossingClimbSpeedLimit).unwrap();
    assert!((limit.altitude - 10000.0).abs() < 1.0);
    assert!(limit.speed <= 250.0 + 0.5);

    let cruise = CruisePathBuilder::new(&p).compute_cruise_path(&mut profile, toc, toc.distance_from_start + 1000.0, &sp);
    let end = cruise.last().unwrap();
    assert!((end.distance_from_start - (toc.distance_from_start + 1000.0)).abs() < 1e-6);
    assert!(end.seconds_from_present > toc.seconds_from_present + 3600.0);
    assert!(end.remaining_fuel_on_board < toc.remaining_fuel_on_board);
}

#[test]
fn altitude_constraint_levels_off_the_climb() {
    let cfg = AircraftConfig::a380();
    let p = params();
    let mut profile = Profile::new(&cfg, Box::new(ConstantWinds::default()));
    profile.max_altitude_constraints = vec![MaxAltitudeConstraint { distance_from_start: 60.0, max_altitude: 6000.0 }];
    build_takeoff_path(&mut profile, &p);
    let sp = McduSpeedProfile::new(&p, 0.0, vec![], vec![]);
    let strategy = ClimbThrustClimbStrategy { params: &p, atm: p.atmosphere(), config: &cfg };
    ClimbPathBuilder::new(&p).compute_climb_path(&mut profile, &strategy, &sp, p.cruise_altitude, FcuClimbHint::default());

    let level = profile.checkpoints.iter().find(|c| c.reason == Reason::LevelOffForClimbConstraint).unwrap();
    assert!((level.altitude - 6000.0).abs() < 11.0);
    assert!(level.distance_from_start < 60.0);
    let past = profile.checkpoints.iter().find(|c| c.reason == Reason::ContinueClimb || c.reason == Reason::AltitudeConstraint);
    assert!(past.is_some());
    assert!(profile.interpolate_altitude_at_distance(59.0) <= 6011.0);
}

#[test]
fn speed_constraint_caps_climb_speed() {
    let cfg = AircraftConfig::a380();
    let p = params();
    let mut profile = Profile::new(&cfg, Box::new(ConstantWinds::default()));
    let constraint = MaxSpeedConstraint { distance_from_start: 40.0, max_speed: 220.0 };
    profile.max_climb_speed_constraints = vec![constraint];
    build_takeoff_path(&mut profile, &p);
    let sp = McduSpeedProfile::new(&p, 0.0, vec![constraint], vec![]);
    let strategy = ClimbThrustClimbStrategy { params: &p, atm: p.atmosphere(), config: &cfg };
    ClimbPathBuilder::new(&p).compute_climb_path(&mut profile, &strategy, &sp, p.cruise_altitude, FcuClimbHint::default());
    let at = profile.interpolate_speed_at_distance(39.0);
    assert!(at <= 221.0, "{at}");
    assert!(profile.interpolate_speed_at_distance(profile.last().distance_from_start) > 250.0);
}

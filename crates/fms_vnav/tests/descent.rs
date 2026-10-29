use fms_vnav::config::{AircraftConfig, FlapConf};
use fms_vnav::descent::*;
use fms_vnav::managed::*;
use fms_vnav::profile::*;
use fms_vnav::speed_profile::McduSpeedProfile;

fn params() -> ComputationParameters {
    ComputationParameters {
        present_altitude: 0.0,
        fcu_altitude: 0.0,
        fcu_speed: -1.0,
        managed_climb_speed: 290.0,
        managed_climb_speed_mach: 0.84,
        managed_cruise_speed: 310.0,
        managed_cruise_speed_mach: 0.85,
        managed_descent_speed: 288.0,
        managed_descent_speed_mach: 0.84,
        zero_fuel_weight: 727_525.0,
        fuel_on_board: 100_000.0,
        v2_speed: 150.0,
        tropopause: 36000.0,
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
        approach_speed: 148.0,
        flap_retraction_speed: 177.0,
        slat_retraction_speed: 191.0,
        clean_speed: 211.0,
        gross_weight_valid: true,
    }
}

fn decel_point() -> Checkpoint {
    Checkpoint {
        reason: Reason::Decel,
        distance_from_start: 3651.6,
        seconds_from_present: 27434.0,
        altitude: 1062.6,
        remaining_fuel_on_board: 9912.0,
        speed: 250.0,
        mach: 0.84,
        profile_phase: ProfilePhase::Descent,
        target_speed: None,
    }
}

#[test]
fn managed_speeds_follow_the_a380x_getters() {
    assert_eq!(managed_climb_speed(None), 250.0);
    assert!((managed_climb_speed(Some(0.0)) - 290.0).abs() < 1e-9);
    assert!((managed_climb_speed(Some(30.0)) - 290.036072).abs() < 1e-5);
    assert!((managed_climb_speed(Some(999.0)) - 330.0).abs() < 1e-9);
    assert_eq!(managed_descent_speed(None, Some(30.0)), 288.0);
    assert_eq!(managed_descent_speed(Some(270.0), Some(30.0)), 270.0);
    assert_eq!(managed_descent_speed(None, None), 300.0);
    assert_eq!(managed_descent_mach(None), 0.84);
}

#[test]
fn cost_index_econ_mach_rises_with_cost_index() {
    let cfg = AircraftConfig::a380();
    let weight = 900_000.0;
    let low = cost_index_to_mach(&cfg, 0.0, 360.0, weight, 0.0, 0.0);
    let high = cost_index_to_mach(&cfg, 100.0, 360.0, weight, 0.0, 0.0);
    assert!(low < high && low > 0.7 && high < 0.81, "{low} {high}");
}

#[test]
fn idle_path_builds_backwards_to_the_cruise_altitude() {
    let cfg = AircraftConfig::a380();
    let p = params();
    let profile = Profile::new(&cfg, Box::new(ConstantWinds::default()));
    let sp = McduSpeedProfile::new(&p, 0.0, vec![], vec![]);
    let mut seq = TemporaryCheckpointSequence::new(decel_point());
    IdlePathBuilder::new(&p, &cfg).build_idle_path(&profile, &mut seq, &[], &sp, 36000.0, f64::NEG_INFINITY);

    let tod = seq.last();
    assert_eq!(tod.reason, Reason::TopOfDescent);
    assert!((tod.altitude - 36000.0).abs() < 2.0, "{}", tod.altitude);
    let distance = decel_point().distance_from_start - tod.distance_from_start;
    assert!(distance > 80.0 && distance < 160.0, "{distance}");
    assert!(seq.checkpoints.iter().any(|c| c.reason == Reason::CrossingDescentSpeedLimit));
    for w in seq.checkpoints.windows(2) {
        assert!(w[1].distance_from_start <= w[0].distance_from_start + 1e-9);
        assert!(w[1].seconds_from_present <= w[0].seconds_from_present + 1e-9);
        assert!(w[1].remaining_fuel_on_board >= w[0].remaining_fuel_on_board - 1e-9);
    }
}

#[test]
fn altitude_constraint_adds_a_geometric_path() {
    let cfg = AircraftConfig::a380();
    let p = params();
    let mut profile = Profile::new(&cfg, Box::new(ConstantWinds::default()));
    profile.descent_altitude_constraints = vec![DescentAltitudeConstraint {
        distance_from_start: 3600.0,
        constraint: AltitudeConstraint { descriptor: AltitudeDescriptor::AtOrBelowAlt1, altitude1: 6000.0, altitude2: 0.0 },
        leg: 7,
    }];
    let sp = McduSpeedProfile::new(&p, 0.0, vec![], vec![]);
    let mut seq = TemporaryCheckpointSequence::new(decel_point());
    let mut steep = Vec::new();
    DescentPathBuilder::new(&p, &cfg).compute_managed_descent_path(&mut seq, &profile, &sp, 36000.0, f64::NEG_INFINITY, &mut steep);

    assert!(seq.checkpoints.iter().any(|c| c.reason == Reason::GeometricPathStart));
    assert_eq!(seq.last().reason, Reason::TopOfDescent);
    let at_constraint = seq.checkpoints.iter().find(|c| (c.distance_from_start - 3600.0).abs() < 1e-6).expect("checkpoint at the constraint");
    assert!(at_constraint.altitude <= 6010.0, "{}", at_constraint.altitude);
}

#[test]
fn constraint_evaluation_matches_the_descriptors() {
    let c = AltitudeConstraint { descriptor: AltitudeDescriptor::BetweenAlt1Alt2, altitude1: 10000.0, altitude2: 8000.0 };
    assert_eq!(evaluate_altitude_constraint(&c, 12000.0, 10.0), (false, 10000.0));
    assert_eq!(evaluate_altitude_constraint(&c, 7000.0, 10.0), (false, 8000.0));
    assert_eq!(evaluate_altitude_constraint(&c, 9000.0, 10.0), (true, 9000.0));
    let at = AltitudeConstraint { descriptor: AltitudeDescriptor::AtAlt1, altitude1: 5000.0, altitude2: 0.0 };
    assert_eq!(evaluate_altitude_constraint(&at, 5005.0, 10.0), (true, 5000.0));
}

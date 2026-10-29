use fms_vnav::approach::*;
use fms_vnav::config::{AircraftConfig, FlapConf};
use fms_vnav::constraints::*;
use fms_vnav::coordinator::*;
use fms_vnav::profile::*;
use fms_vnav::speed_profile::McduSpeedProfile;
use fms_vnav::strategy::{ClimbThrustClimbStrategy, VerticalSpeedStrategy};
use fms_vnav::{climb::*, takeoff::*};

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
        fuel_on_board: 209_439.0,
        v2_speed: 150.0,
        tropopause: 36000.0,
        perf_factor: 0.0,
        departure_elevation: 80.0,
        destination_elevation: 13.0,
        acceleration_altitude: 1580.0,
        thrust_reduction_altitude: 1580.0,
        cruise_altitude: 34000.0,
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

fn leg(cumulative: f64, total: f64) -> ConstraintLeg {
    ConstraintLeg {
        cumulative_distance_with_transitions: Some(cumulative),
        cumulative_distance_to_end_with_transitions: Some(total - cumulative),
        constraint_type: ConstraintType::Unknown,
        altitude_constraint: None,
        speed_constraint: None,
        cruise_step: None,
        vertical_angle: None,
        is_final_approach_fix: false,
    }
}

fn plan(total: f64, step: Option<(f64, f64)>, descent_constraint: Option<(f64, f64)>) -> ConstraintPlan {
    let mut elements = vec![Some(leg(0.0, total)), Some(leg(1000.0, total)), Some(leg(total - 30.0, total)), Some(leg(total, total))];
    if let Some((before_termination, to_altitude)) = step {
        elements[1].as_mut().unwrap().cruise_step = Some(CruiseStepEntry { waypoint_index: 1, to_altitude, distance_before_termination: before_termination, is_ignored: false });
    }
    if let Some((altitude, speed)) = descent_constraint {
        let l = elements[2].as_mut().unwrap();
        l.constraint_type = ConstraintType::Descent;
        l.altitude_constraint = Some(AltitudeConstraint { descriptor: AltitudeDescriptor::AtOrAboveAlt1, altitude1: altitude, altitude2: 0.0 });
        l.speed_constraint = Some(speed);
    }
    ConstraintPlan {
        elements,
        active_leg_index: 1,
        first_missed_approach_leg_index: 4,
        destination_leg_index: Some(3),
        missed_approach_point_altitude: None,
        destination_runway_threshold_altitude: None,
        destination_airport_altitude: Some(13.0),
    }
}

fn build<'a>(cfg: &'a AircraftConfig, p: &ComputationParameters, reader: &ConstraintReader) -> Profile<'a> {
    let mut profile = Profile::new(cfg, Box::new(ConstantWinds::default()));
    reader.apply_to_profile(&mut profile);
    build_takeoff_path(&mut profile, p);
    let sp = McduSpeedProfile::new(p, 0.0, reader.climb_speed_constraints.clone(), reader.descent_speed_constraints.clone());
    let climb = ClimbThrustClimbStrategy { params: p, atm: p.atmosphere(), config: cfg };
    ClimbPathBuilder::new(p).compute_climb_path(&mut profile, &climb, &sp, p.cruise_altitude, FcuClimbHint::default());
    let descent = VerticalSpeedStrategy { params: p, atm: p.atmosphere(), vertical_speed: -1000.0, config: cfg };
    let mut coordinator = CruiseToDescentCoordinator::default();
    coordinator.build_cruise_and_descent_path(p, &mut profile, &sp, &climb, &descent, None, &mut Vec::new()).unwrap();
    profile.finalize_profile();
    profile
}

#[test]
fn constraint_reader_collects_steps_constraints_and_final_altitude() {
    let mut reader = ConstraintReader::default();
    reader.update_flight_plan(&plan(2000.0, Some((5.0, 38000.0)), Some((3000.0, 210.0))), 2000.0);
    assert_eq!(reader.total_flight_plan_distance, 2000.0);
    assert_eq!(reader.cruise_steps.len(), 1);
    assert!((reader.cruise_steps[0].distance_from_start - 995.0).abs() < 1e-9);
    assert_eq!(reader.descent_altitude_constraints.len(), 1);
    assert_eq!(reader.descent_speed_constraints[0].max_speed, 210.0);
    assert!((reader.final_altitude - 63.0).abs() < 1e-9);
    assert!((reader.faf_distance_to_end - 1000.0 / 3f64.to_radians().tan() / 6076.12).abs() < 1e-9);
}

#[test]
fn approach_path_ends_in_decel_with_flap_target_speeds() {
    let cfg = AircraftConfig::a380();
    let p = params();
    let mut reader = ConstraintReader::default();
    reader.update_flight_plan(&plan(2000.0, None, None), 2000.0);
    let mut profile = Profile::new(&cfg, Box::new(ConstantWinds::default()));
    reader.apply_to_profile(&mut profile);
    let sp = McduSpeedProfile::new(&p, 0.0, vec![], vec![]);
    let seq = ApproachPathBuilder::new(&p, &cfg).compute_approach_path(&profile, &sp, 9000.0, 20000.0, &mut Vec::new());
    let reasons: Vec<Reason> = seq.checkpoints.iter().map(|c| c.reason).collect();
    assert_eq!(reasons, vec![Reason::Landing, Reason::AtmosphericConditions, Reason::FlapsFull, Reason::Flaps3, Reason::Flaps2, Reason::Flaps1, Reason::Decel]);
    let targets: Vec<Option<f64>> = seq.checkpoints[2..].iter().map(|c| c.target_speed).collect();
    assert_eq!(targets, vec![Some(148.0), Some(177.0), Some(177.0), Some(191.0), Some(211.0)]);
    assert_eq!(seq.last().speed, 250.0);
}

#[test]
fn step_climb_is_flown_and_a_late_step_is_ignored() {
    let cfg = AircraftConfig::a380();
    let p = params();
    let mut reader = ConstraintReader::default();
    reader.update_flight_plan(&plan(3000.0, Some((0.0, 38000.0)), None), 3000.0);
    let profile = build(&cfg, &p, &reader);
    let step = profile.checkpoints.iter().find(|c| c.reason == Reason::StepClimb).expect("step climb");
    let top = profile.checkpoints.iter().find(|c| c.reason == Reason::TopOfStepClimb).expect("top of step climb");
    assert!((step.distance_from_start - 1000.0).abs() < 1e-6);
    assert!((top.altitude - 38000.0).abs() < 1.0);
    let tod = profile.checkpoints.iter().find(|c| c.reason == Reason::TopOfDescent).unwrap();
    assert!((tod.altitude - 38000.0).abs() < 1.0, "descent starts from the step altitude, {}", tod.altitude);
    assert!(profile.ignored_cruise_step_waypoints.is_empty());

    let mut reader = ConstraintReader::default();
    reader.update_flight_plan(&plan(1150.0, Some((0.0, 38000.0)), None), 1150.0);
    let profile = build(&cfg, &p, &reader);
    assert!(profile.checkpoints.iter().all(|c| c.reason != Reason::TopOfStepClimb));
    assert_eq!(profile.ignored_cruise_step_waypoints, vec![1]);
}

#[test]
fn whole_profile_is_ordered_and_burns_fuel() {
    let cfg = AircraftConfig::a380();
    let p = params();
    let mut reader = ConstraintReader::default();
    reader.update_flight_plan(&plan(2000.0, None, Some((3000.0, 210.0))), 2000.0);
    let profile = build(&cfg, &p, &reader);
    assert_eq!(profile.checkpoints.last().unwrap().reason, Reason::Landing);
    assert!((profile.checkpoints.last().unwrap().distance_from_start - 2000.0).abs() < 1e-9);
    // The coordinator stops once the cruise and descent meet the T/D within 100 lb and 1 s.
    for w in profile.checkpoints.windows(2) {
        assert!(w[1].seconds_from_present >= w[0].seconds_from_present - 1.0, "{:?} {:?}", w[0], w[1]);
        assert!(w[1].remaining_fuel_on_board <= w[0].remaining_fuel_on_board + 100.0, "{:?} {:?}", w[0], w[1]);
    }
}

#[test]
fn bisection_finds_a_root_within_tolerance() {
    let x = find_zero(|x| x * x - 2.0, (0.0, 2.0), (-0.01, 0.01), NonTerminationStrategy::LowerAbsoluteErrorResult, 20);
    assert!((x * x - 2.0).abs() <= 0.01);
    assert_eq!(calculate_intercept(&[], &[], 0.0), None);
}

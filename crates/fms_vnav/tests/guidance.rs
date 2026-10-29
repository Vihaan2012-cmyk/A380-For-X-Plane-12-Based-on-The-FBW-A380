use fms_vnav::config::{AircraftConfig, FlapConf};
use fms_vnav::guidance::*;
use fms_vnav::modes::{vertical, FcuModes};
use fms_vnav::profile::*;

fn params() -> ComputationParameters {
    ComputationParameters {
        present_altitude: 20000.0,
        fcu_altitude: 3000.0,
        fcu_speed: 0.0,
        managed_climb_speed: 290.0,
        managed_climb_speed_mach: 0.84,
        managed_cruise_speed: 310.0,
        managed_cruise_speed_mach: 0.85,
        managed_descent_speed: 310.0,
        managed_descent_speed_mach: 0.84,
        zero_fuel_weight: 727_000.0,
        fuel_on_board: 40_000.0,
        v2_speed: 150.0,
        tropopause: 36_090.0,
        perf_factor: 0.0,
        departure_elevation: 0.0,
        destination_elevation: 13.0,
        acceleration_altitude: 1500.0,
        thrust_reduction_altitude: 1500.0,
        cruise_altitude: 36_000.0,
        climb_speed_limit: Some(SpeedLimit { speed: 250.0, under_altitude: 10_000.0 }),
        descent_speed_limit: Some(SpeedLimit { speed: 250.0, under_altitude: 10_000.0 }),
        flight_phase: FlightPhase::Descent,
        preselected_clb_speed: 0.0,
        preselected_cruise_speed: 0.0,
        takeoff_flaps: FlapConf::Conf1,
        takeoff_toga_n1: 0.0,
        isa_deviation: 0.0,
        approach_speed: 140.0,
        flap_retraction_speed: 180.0,
        slat_retraction_speed: 190.0,
        clean_speed: 210.0,
        gross_weight_valid: true,
    }
}

fn cp(reason: Reason, d: f64, alt: f64) -> Checkpoint {
    Checkpoint {
        reason,
        distance_from_start: d,
        seconds_from_present: 0.0,
        altitude: alt,
        remaining_fuel_on_board: 10_000.0,
        speed: 300.0,
        mach: 0.84,
        profile_phase: ProfilePhase::Descent,
        target_speed: None,
    }
}

/// Idle path from the TOD at 100 NM / FL360 to 3000 ft at 200 NM, geometric segment to the runway at 210 NM.
fn profile(cfg: &AircraftConfig) -> Profile<'_> {
    let mut p = Profile::new(cfg, Box::new(ConstantWinds::default()));
    p.total_flight_plan_distance = 210.0;
    p.checkpoints = vec![
        cp(Reason::TopOfDescent, 100.0, 36000.0),
        cp(Reason::IdlePathEnd, 200.0, 3000.0),
        cp(Reason::GeometricPathStart, 200.0, 3000.0),
        cp(Reason::Landing, 210.0, 50.0),
    ];
    p
}

fn mach_to_kias(_: f64) -> f64 {
    400.0
}

fn inputs(modes: FcuModes) -> GuidanceInputs<'static> {
    GuidanceInputs {
        modes,
        speeds: SpeedData { vls: 150.0, vmax: 340.0, green_dot: 200.0, s_speed: 190.0, f_speed: 170.0, flaps_handle_index: 0, mach_to_kias: &mach_to_kias },
        autopilot_airspeed_managed: true,
        manual_hold_active: false,
        inertial_vertical_speed: Some(-2000.0),
        barometric_vertical_speed: None,
        ground_speed: 400.0,
        current_airspeed: 300.0,
        current_pressure_altitude: 20000.0,
        current_altitude: Some(20000.0),
        speeds_managed_pfd: 300.0,
        speeds_managed_athr: 300.0,
        origin_transition_altitude: 6000.0,
        autopilot_active: true,
        fma_lateral_mode: 20,
        pause_at_tod_enabled: false,
        pause_at_tod_distance: 10.0,
    }
}

#[test]
fn relation_tracks_the_aircraft_along_the_profile() {
    let cfg = AircraftConfig::a380();
    let prof = profile(&cfg);
    let mut rel = AircraftToDescentProfileRelation::default();
    rel.update_profile(Some(&prof));
    assert!(rel.is_valid);
    rel.update(60.0);
    assert_eq!(rel.distance_from_start(), 150.0);
    assert!(rel.is_past_top_of_descent());
    assert_eq!(rel.distance_to_top_of_descent(), Some(-50.0));
    assert!(!rel.is_on_geometric_path());
    assert!((rel.current_target_altitude(&prof) - 19500.0).abs() < 1e-9);
    assert!((rel.compute_linear_deviation(&prof, 20000.0) - 500.0).abs() < 1e-9);
    rel.update(f64::INFINITY);
    assert!(!rel.is_valid);
}

#[test]
fn speed_margins_follow_fbw_offsets() {
    let cfg = AircraftConfig::a380();
    let p = params();
    let i = inputs(FcuModes::default());
    let m = SpeedMargin::new(&cfg);
    assert_eq!(m.get_margins(300.0, &p, &i.speeds), (280.0, 305.0));
    assert_eq!(m.get_margins(310.0, &p, &i.speeds), (290.0, 330.0));
    assert_eq!(m.get_target(320.0, 300.0, &p, &i.speeds), 305.0);
}

#[test]
fn des_mode_captures_the_idle_path_then_goes_above_it() {
    let cfg = AircraftConfig::a380();
    let prof = profile(&cfg);
    let mut p = params();
    p.present_altitude = 19500.0;
    let mut rel = AircraftToDescentProfileRelation::default();
    let mut g = DescentGuidance::new(&cfg);
    g.update_profile(&mut rel, Some(&prof));
    let des = FcuModes { vertical: vertical::DES, speed_managed: true, ..Default::default() };
    let i = inputs(des);
    g.update(50.0, 60.0, &mut rel, Some(&prof), &p, &i);
    assert_eq!(g.path_capture_state(), PathCaptureState::InPathCapture);
    assert_eq!(g.des_submode(), RequestedVerticalMode::VpathThrust);
    assert!(g.out.pfd_linear_deviation_active);
    assert!(g.out.pfd_show_speed_margins);
    assert_eq!(g.out.speeds_managed_athr, Some(300.0), "the speed target is set before the submode of this update");
    g.update(50.0, 60.0, &mut rel, Some(&prof), &p, &i);
    assert_eq!(g.path_capture_state(), PathCaptureState::OnPath);
    assert_eq!(g.out.speeds_managed_athr, Some(280.0), "VPATH THRUST targets the lower margin");

    p.present_altitude = 21500.0;
    g.update(50.0, 60.0, &mut rel, Some(&prof), &p, &i);
    assert_eq!(g.path_capture_state(), PathCaptureState::OffPath);
    g.update(50.0, 60.0, &mut rel, Some(&prof), &p, &i);
    assert_eq!(g.des_submode(), RequestedVerticalMode::SpeedThrust);
    assert_eq!(g.out.speeds_managed_athr, Some(305.0), "above the path targets the upper margin");
}

#[test]
fn below_the_path_before_tod_uses_vertical_speed() {
    let cfg = AircraftConfig::a380();
    let prof = profile(&cfg);
    let p = ComputationParameters { present_altitude: 36000.0, ..params() };
    let mut rel = AircraftToDescentProfileRelation::default();
    let mut g = DescentGuidance::new(&cfg);
    g.update_profile(&mut rel, Some(&prof));
    let i = inputs(FcuModes { vertical: vertical::DES, speed_managed: true, ..Default::default() });
    g.update(50.0, 150.0, &mut rel, Some(&prof), &p, &i);
    assert_eq!(g.des_submode(), RequestedVerticalMode::VsSpeed);
    assert_eq!(g.out.fg_target_vertical_speed, -1000.0);
}

#[test]
fn latched_guidance_latches_on_path() {
    let cfg = AircraftConfig::a380();
    let prof = profile(&cfg);
    let p = ComputationParameters { present_altitude: 19600.0, ..params() };
    let mut rel = AircraftToDescentProfileRelation::default();
    let mut g = LatchedDescentGuidance::new(&cfg);
    g.update_profile(&mut rel, Some(&prof));
    let i = inputs(FcuModes { vertical: vertical::DES, speed_managed: true, ..Default::default() });
    g.update(50.0, 60.0, &mut rel, Some(&prof), &p, &i);
    assert_eq!(g.des_submode(), RequestedVerticalMode::VpathSpeed);
    assert!(g.out.pfd_vertical_profile_latched);
    let p = ComputationParameters { present_altitude: 19800.0, ..p };
    g.update(50.0, 60.0, &mut rel, Some(&prof), &p, &i);
    assert_eq!(g.des_submode(), RequestedVerticalMode::SpeedThrust);
    assert!(!g.out.pfd_vertical_profile_latched);
}

#[test]
fn tod_reached_and_pause() {
    let cfg = AircraftConfig::a380();
    let prof = profile(&cfg);
    let p = ComputationParameters { present_altitude: 36000.0, flight_phase: FlightPhase::Cruise, ..params() };
    let mut rel = AircraftToDescentProfileRelation::default();
    let mut g = DescentGuidance::new(&cfg);
    g.update_profile(&mut rel, Some(&prof));
    let mut i = inputs(FcuModes::default());
    i.pause_at_tod_enabled = true;
    g.update(50.0, 115.0, &mut rel, Some(&prof), &p, &i);
    assert!(!g.out.pfd_msg_td_reached);
    assert_eq!(g.out.popups, vec![TodPopup::TopOfDescentPause]);
    g.update(50.0, 105.0, &mut rel, Some(&prof), &p, &i);
    assert!(g.out.pfd_msg_td_reached);
    assert_eq!(g.out.popups.len(), 1, "no second pause while the first is latched");
}

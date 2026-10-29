use wxr_extras::synthetic::{LinearShear, Microburst, SyntheticField};
use wxr_extras::{Aural, Ownship, PwsConfig, PwsInputs, PwsOutput, PwsPhase, PwsScanner, PwsStatus, Severity};

const NM: f64 = 1852.0;

fn approach_inputs() -> PwsInputs {
    PwsInputs {
        own: Ownship { alt_msl_m: 300., ra_ft: 984., heading_true_deg: 90., tas_ms: 75., gs_ms: 75., vs_ms: -3.8, on_ground: false, ..Default::default() },
        phase: PwsPhase::Approach,
        pred_ws_auto: true,
        radar_powered: true,
        radar_failed: false,
        data_valid: true,
        ..Default::default()
    }
}

fn takeoff_roll_inputs(gs_ms: f64) -> PwsInputs {
    PwsInputs {
        own: Ownship { alt_msl_m: 0., ra_ft: 0., heading_true_deg: 90., tas_ms: gs_ms, gs_ms, vs_ms: 0., on_ground: true, ..Default::default() },
        phase: PwsPhase::Takeoff,
        takeoff_power: true,
        pred_ws_auto: true,
        radar_powered: true,
        data_valid: true,
        ..Default::default()
    }
}

fn burst_at(range_nm: f64, azimuth_deg: f64, radius_m: f64, outflow: f64, downdraft: f64) -> SyntheticField {
    let b = (90.0 + azimuth_deg).to_radians();
    let (e, n) = (range_nm * NM * b.sin(), range_nm * NM * b.cos());
    SyntheticField { bursts: vec![Microburst::new(e, n, radius_m, outflow, downdraft)], ..Default::default() }
}

fn run(inputs: PwsInputs, field: &mut SyntheticField, seconds: f64) -> Vec<PwsOutput> {
    let mut scanner = PwsScanner::new(PwsConfig::default());
    run_with(&mut scanner, inputs, field, seconds, 400)
}

fn run_with(scanner: &mut PwsScanner, mut inputs: PwsInputs, field: &mut SyntheticField, seconds: f64, budget: usize) -> Vec<PwsOutput> {
    let mut out = Vec::new();
    let mut t = 0.0;
    while t <= seconds {
        inputs.time_s = t;
        out.push(scanner.step(&inputs, field, budget));
        t += 0.25;
    }
    out
}

fn worst(out: &[PwsOutput]) -> Severity {
    out.iter().map(|o| o.severity).max().unwrap()
}

fn aurals(out: &[PwsOutput]) -> Vec<Aural> {
    out.iter().filter_map(|o| o.aural).collect()
}

#[test]
fn microburst_one_mile_ahead_on_approach_gives_a_warning_and_go_around_aural() {
    let mut field = burst_at(1.0, 0., 1000., 20., 8.);
    let out = run(approach_inputs(), &mut field, 3.);
    let last = out.last().unwrap();
    assert_eq!(last.status, PwsStatus::Active);
    assert_eq!(last.severity, Severity::Warning);
    assert_eq!(last.pfd_ws_ahead, Severity::Warning);
    assert_eq!(aurals(&out), vec![Aural::GoAroundWindshearAhead]);
    let icon = last.icon.unwrap();
    assert!(icon.range_nm <= 1.5 && icon.azimuth_deg.abs() <= 5.);
    assert_eq!(icon.severity, Severity::Warning);
}

#[test]
fn microburst_further_out_gives_only_a_caution_with_monitor_radar_display() {
    let mut field = burst_at(3.0, 0., 1000., 20., 8.);
    let out = run(approach_inputs(), &mut field, 3.);
    let last = out.last().unwrap();
    assert_eq!(last.severity, Severity::Caution, "{last:?}");
    assert_eq!(aurals(&out), vec![Aural::MonitorRadarDisplay]);
    assert_eq!(last.icon.unwrap().severity, Severity::Caution);
}

#[test]
fn closing_on_a_burst_escalates_from_caution_to_warning() {
    let mut scanner = PwsScanner::new(PwsConfig::default());
    let mut field = burst_at(3.0, 0., 1000., 20., 8.);
    let far = run_with(&mut scanner, approach_inputs(), &mut field, 2., 400);
    assert_eq!(far.last().unwrap().severity, Severity::Caution);
    let mut near = approach_inputs();
    near.own.east_m = 1.6 * NM;
    let mut out = Vec::new();
    let mut t = 3.;
    while t < 6. {
        near.time_s = t;
        out.push(scanner.step(&near, &mut field, 400));
        t += 0.25;
    }
    assert_eq!(out.last().unwrap().severity, Severity::Warning);
    assert_eq!(aurals(&out), vec![Aural::GoAroundWindshearAhead]);
}

#[test]
fn takeoff_warning_uses_the_takeoff_aural() {
    let mut field = burst_at(1.0, 0., 1000., 20., 8.);
    let out = run(takeoff_roll_inputs(30.), &mut field, 3.);
    assert_eq!(out.last().unwrap().severity, Severity::Warning);
    assert_eq!(aurals(&out), vec![Aural::WindshearAhead]);
}

#[test]
fn burst_beyond_caution_range_only_shows_the_icon() {
    let mut field = burst_at(4.6, 0., 1000., 20., 8.);
    let out = run(approach_inputs(), &mut field, 3.);
    let last = out.last().unwrap();
    assert_eq!(worst(&out), Severity::None);
    assert!(aurals(&out).is_empty());
    let icon = last.icon.expect("icon within 5 nm");
    assert!(icon.range_nm > 3.);
}

#[test]
fn burst_outside_the_scan_range_is_ignored() {
    let mut field = burst_at(9.0, 0., 1000., 25., 10.);
    let out = run(approach_inputs(), &mut field, 3.);
    assert_eq!(worst(&out), Severity::None);
    assert!(out.iter().all(|o| o.icon.is_none() && o.hazard_cells == 0));
}

#[test]
fn burst_far_off_the_nose_is_ignored_and_one_outside_the_alert_sector_only_draws_an_icon() {
    let mut field = burst_at(1.5, 70., 800., 25., 10.);
    let out = run(approach_inputs(), &mut field, 3.);
    assert_eq!(worst(&out), Severity::None);
    assert!(out.iter().all(|o| o.icon.is_none()));

    let mut field = burst_at(3.0, 37., 500., 25., 10.);
    let out = run(approach_inputs(), &mut field, 3.);
    let last = out.last().unwrap();
    assert_eq!(worst(&out), Severity::None, "{last:?}");
    let icon = last.icon.expect("inside the +-40 deg scan");
    assert!(icon.azimuth_deg > 25.);
}

#[test]
fn weak_shear_stays_quiet() {
    let mut field = burst_at(1.0, 0., 1000., 4., 1.5);
    let out = run(approach_inputs(), &mut field, 3.);
    assert_eq!(worst(&out), Severity::None);
    assert!(out.iter().all(|o| o.icon.is_none()));
}

#[test]
fn a_calm_sky_and_a_steady_wind_stay_quiet() {
    let mut field = SyntheticField { background_wind: (-12., 5.), ..Default::default() };
    assert_eq!(worst(&run(approach_inputs(), &mut field, 3.)), Severity::None);
}

#[test]
fn losing_headwind_is_hazardous_but_gaining_it_is_not() {
    let ramp = |change: f64| SyntheticField {
        linear_shear: Some(LinearShear { bearing_deg: 90., from_m: 0.8 * NM, to_m: 1.8 * NM, change_ms: change, origin_east_m: 0., origin_north_m: 0. }),
        ..Default::default()
    };
    let out = run(approach_inputs(), &mut ramp(30.), 3.);
    assert_eq!(worst(&out), Severity::Warning);
    let out = run(approach_inputs(), &mut ramp(-30.), 3.);
    assert_eq!(worst(&out), Severity::None);
}

#[test]
fn downdraft_alone_contributes_to_the_hazard() {
    let mut field = burst_at(1.0, 0., 1000., 3., 14.);
    assert_ne!(worst(&run(approach_inputs(), &mut field, 3.)), Severity::None);
}

#[test]
fn above_the_ceiling_the_scanner_is_in_standby_and_silent() {
    let mut inputs = approach_inputs();
    inputs.own.ra_ft = 2500.;
    inputs.own.alt_msl_m = 762.;
    let mut field = burst_at(1.0, 0., 1000., 20., 8.);
    let out = run(inputs, &mut field, 3.);
    assert!(out.iter().all(|o| o.status == PwsStatus::Standby && o.severity == Severity::None && o.icon.is_none()));
}

#[test]
fn other_phase_and_ground_without_takeoff_power_are_standby() {
    let mut field = burst_at(1.0, 0., 1000., 20., 8.);
    let mut inputs = approach_inputs();
    inputs.phase = PwsPhase::Other;
    assert_eq!(run(inputs, &mut field, 1.).last().unwrap().status, PwsStatus::Standby);
    let mut inputs = takeoff_roll_inputs(10.);
    inputs.takeoff_power = false;
    assert_eq!(run(inputs, &mut field, 1.).last().unwrap().status, PwsStatus::Standby);
}

#[test]
fn switch_off_and_radar_failure_are_reported_and_silent() {
    let mut field = burst_at(1.0, 0., 1000., 20., 8.);
    let mut inputs = approach_inputs();
    inputs.pred_ws_auto = false;
    let out = run(inputs, &mut field, 1.);
    assert!(out.iter().all(|o| o.status == PwsStatus::Off && o.severity == Severity::None));
    let mut inputs = approach_inputs();
    inputs.radar_failed = true;
    assert_eq!(run(inputs, &mut field, 1.).last().unwrap().status, PwsStatus::Failed);
    let mut inputs = approach_inputs();
    inputs.radar_powered = false;
    assert_eq!(run(inputs, &mut field, 1.).last().unwrap().status, PwsStatus::Failed);
    let mut inputs = approach_inputs();
    inputs.data_valid = false;
    assert_eq!(run(inputs, &mut field, 1.).last().unwrap().status, PwsStatus::Failed);
}

#[test]
fn alerts_are_inhibited_fast_on_the_roll_and_low_in_the_air_but_the_icon_stays() {
    let mut field = burst_at(1.0, 0., 1000., 20., 8.);
    let fast = run(takeoff_roll_inputs(70.), &mut field, 3.);
    let last = fast.last().unwrap();
    assert!(last.alerts_inhibited && last.severity == Severity::None && last.pfd_ws_ahead == Severity::None);
    assert!(last.icon.is_some() && last.hazard == Severity::Warning);
    assert!(aurals(&fast).is_empty());

    let mut lift = takeoff_roll_inputs(80.);
    lift.own.on_ground = false;
    lift.own.ra_ft = 30.;
    lift.own.alt_msl_m = 9.;
    let out = run(lift, &mut field, 3.);
    assert!(out.last().unwrap().alerts_inhibited && aurals(&out).is_empty());
    lift.own.ra_ft = 120.;
    lift.own.alt_msl_m = 36.;
    let out = run(lift, &mut field, 3.);
    assert_eq!(out.last().unwrap().severity, Severity::Warning);

    let mut flare = approach_inputs();
    flare.own.ra_ft = 40.;
    flare.own.alt_msl_m = 12.;
    let out = run(flare, &mut field, 3.);
    assert!(out.last().unwrap().alerts_inhibited && aurals(&out).is_empty());
}

#[test]
fn a_higher_priority_aural_inhibits_the_alert() {
    let mut field = burst_at(1.0, 0., 1000., 20., 8.);
    let mut inputs = approach_inputs();
    inputs.higher_priority_aural = true;
    let out = run(inputs, &mut field, 3.);
    assert!(aurals(&out).is_empty() && out.last().unwrap().alerts_inhibited);
}

#[test]
fn warning_aural_repeats_a_limited_number_of_times() {
    let mut field = burst_at(1.0, 0., 1000., 20., 8.);
    let out = run(approach_inputs(), &mut field, 40.);
    assert_eq!(aurals(&out).len(), 3);
}

#[test]
fn hazard_is_held_for_a_few_seconds_then_clears() {
    let mut scanner = PwsScanner::new(PwsConfig::default());
    let mut field = burst_at(1.0, 0., 1000., 20., 8.);
    let on = run_with(&mut scanner, approach_inputs(), &mut field, 2., 400);
    assert_eq!(on.last().unwrap().severity, Severity::Warning);
    let mut clear = SyntheticField::default();
    let mut inputs = approach_inputs();
    let mut sev = Vec::new();
    let mut t = 2.25;
    while t < 12. {
        inputs.time_s = t;
        sev.push((t, scanner.step(&inputs, &mut clear, 400).severity));
        t += 0.25;
    }
    assert!(sev.iter().any(|(t, s)| *t < 4. && *s == Severity::Warning));
    assert_eq!(sev.last().unwrap().1, Severity::None);
}

#[test]
fn a_time_sliced_scan_finds_the_burst_after_enough_steps() {
    let mut scanner = PwsScanner::new(PwsConfig::default());
    let mut field = burst_at(1.0, 0., 1000., 20., 8.);
    let out = run_with(&mut scanner, approach_inputs(), &mut field, 6., 20);
    assert_eq!(out.last().unwrap().severity, Severity::Warning);
    assert!(out[0].severity == Severity::None);
}

#[test]
fn stale_cells_are_ignored() {
    let mut scanner = PwsScanner::new(PwsConfig::default());
    let mut field = burst_at(1.0, 0., 1000., 20., 8.);
    let mut inputs = approach_inputs();
    inputs.time_s = 0.;
    scanner.step(&inputs, &mut field, 400);
    inputs.time_s = 30.;
    let out = scanner.step(&inputs, &mut field, 0);
    assert_eq!(out.hazard_cells, 0);
}

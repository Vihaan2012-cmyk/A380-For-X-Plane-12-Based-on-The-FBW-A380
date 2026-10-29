use row_rop::model::*;
use row_rop::vars;
use row_rop::*;

const DT: f64 = 0.1;

fn base(lda: f64, cond: RunwayCondition) -> Inputs {
    Inputs {
        runway: Some(Runway { lda_m: lda, slope_pct: 0., condition: cond }),
        aircraft: Aircraft {
            radio_alt_ft: 300.,
            vertical_speed_fpm: -700.,
            ground_speed_kt: 140.,
            tas_kt: 145.,
            headwind_kt: 5.,
            long_accel_ms2: 0.,
            weight_kg: 380_000.,
            dist_from_threshold_m: -(300. * FT_TO_M / 3f64.to_radians().tan()),
            position_valid: true,
            on_ground: false,
            gear_down: true,
            ground_spoilers: false,
            flight_phase: 8,
        },
        braking: Braking {
            pedal_left_pct: 0.,
            pedal_right_pct: 0.,
            autobrake_mode: AutobrakeMode::Off,
            autobrake_active: false,
        },
        reversers: Reversers { tla: [0.; 4], inboard_available: [true, true] },
        row_lost: false,
        rop_lost: false,
        keep_max_reverse_played: false,
    }
}

fn rollout(lda: f64, cond: RunwayCondition, dist: f64, gs_kt: f64, decel: f64) -> Inputs {
    let mut i = base(lda, cond);
    i.aircraft.radio_alt_ft = 0.;
    i.aircraft.on_ground = true;
    i.aircraft.vertical_speed_fpm = 0.;
    i.aircraft.ground_speed_kt = gs_kt;
    i.aircraft.dist_from_threshold_m = dist;
    i.aircraft.long_accel_ms2 = -decel;
    i.aircraft.ground_spoilers = true;
    i.aircraft.flight_phase = 10;
    i
}

fn run(sys: &mut RowRop, input: &Inputs, seconds: f64) -> Outputs {
    let mut out = Outputs::default();
    let mut t = 0.;
    while t < seconds {
        out = sys.update(DT, input);
        t += DT;
    }
    out
}

#[test]
fn dry_long_runway_is_silent_on_approach_and_rollout() {
    let mut sys = RowRop::default();
    let out = run(&mut sys, &base(4000., RunwayCondition::Dry), 5.);
    assert!(!out.rwy_too_short && !out.if_wet_rwy_too_short);
    assert!(out.pfd_messages.is_empty());
    assert_eq!(out.aurals, Aurals::default());
    assert!(out.row_operative);

    let mut i = rollout(4000., RunwayCondition::Dry, 900., 115., 2.5);
    i.braking.autobrake_mode = AutobrakeMode::L3;
    i.braking.autobrake_active = true;
    let out = run(&mut sys, &i, 8.);
    assert!(!out.overrun_detected);
    assert!(!out.auto_max_braking);
    assert_eq!(out.aurals, Aurals::default());
    assert_eq!(out.status_word, 0);
    assert!(out.status_valid);
}

#[test]
fn short_wet_runway_on_approach_raises_row() {
    let mut sys = RowRop::default();
    let out = run(&mut sys, &base(1200., RunwayCondition::Wet), 2.);
    assert!(out.rwy_too_short);
    assert!(!out.if_wet_rwy_too_short);
    assert!(out.aurals.runway_too_short);
    assert_eq!(out.pfd_messages, vec![PfdMessage::RwyTooShort]);
    assert_eq!(PfdMessage::RwyTooShort.level(), Level::Amber);
    assert_ne!(out.status_word & vars::bit(vars::BIT_TOO_SHORT), 0);
}

#[test]
fn dry_runway_only_short_if_wet() {
    let mut sys = RowRop::default();
    let out = run(&mut sys, &base(1600., RunwayCondition::Dry), 2.);
    assert!(!out.rwy_too_short);
    assert!(out.if_wet_rwy_too_short);
    assert!(!out.aurals.runway_too_short);
    assert_eq!(out.pfd_messages, vec![PfdMessage::IfWetRwyTooShort]);
    assert_eq!(out.status_word, vars::bit(vars::BIT_IF_WET_TOO_SHORT));
    assert_eq!(PfdMessage::IfWetRwyTooShort.text(), "IF WET: RWY TOO SHORT");
}

#[test]
fn row_inhibitions() {
    let mut sys = RowRop::default();
    let mut i = base(1200., RunwayCondition::Wet);
    i.aircraft.vertical_speed_fpm = 800.;
    assert!(!run(&mut sys, &i, 1.).rwy_too_short);

    let mut i = base(1200., RunwayCondition::Wet);
    i.aircraft.gear_down = false;
    assert!(!run(&mut sys, &i, 1.).rwy_too_short);

    let mut i = base(1200., RunwayCondition::Wet);
    i.aircraft.radio_alt_ft = 30.;
    assert!(!run(&mut sys, &i, 1.).rwy_too_short);

    let mut i = base(1200., RunwayCondition::Wet);
    i.aircraft.radio_alt_ft = 900.;
    assert!(!run(&mut sys, &i, 1.).rwy_too_short);

    let mut i = base(1200., RunwayCondition::Wet);
    i.row_lost = true;
    let out = run(&mut sys, &i, 1.);
    assert!(!out.rwy_too_short);
    assert!(!out.status_valid);
}

#[test]
fn row_clears_when_runway_becomes_adequate() {
    let mut sys = RowRop::default();
    assert!(run(&mut sys, &base(1200., RunwayCondition::Wet), 1.).rwy_too_short);
    assert!(!run(&mut sys, &base(3500., RunwayCondition::Wet), 1.).rwy_too_short);
}

#[test]
fn slow_deceleration_with_autobrake_commands_max_braking_and_reverse() {
    let mut sys = RowRop::default();
    let mut i = rollout(2200., RunwayCondition::Wet, 1100., 126., 0.8);
    i.braking.autobrake_mode = AutobrakeMode::L2;
    i.braking.autobrake_active = true;
    let out = run(&mut sys, &i, 7.);
    assert!(out.overrun_detected);
    assert!(out.auto_max_braking);
    assert!(out.set_max_reverse_request);
    assert!(out.aurals.set_max_reverse);
    assert!(!out.aurals.max_braking);
    assert_eq!(out.pfd_messages, vec![PfdMessage::SetMaxReverse]);
    assert_ne!(out.status_word & vars::bit(vars::BIT_SET_MAX_REVERSE), 0);
}

#[test]
fn manual_braking_gets_max_braking_then_reverse_then_keep_reverse() {
    let mut sys = RowRop::default();
    let mut i = rollout(2200., RunwayCondition::Wet, 1100., 126., 0.8);
    let out = run(&mut sys, &i, 7.);
    assert!(out.overrun_detected);
    assert!(!out.auto_max_braking);
    assert!(out.aurals.max_braking);
    assert!(!out.aurals.set_max_reverse);
    assert_eq!(out.pfd_messages, vec![PfdMessage::MaxBraking]);
    assert_eq!(PfdMessage::MaxBraking.level(), Level::Red);

    i.braking.pedal_left_pct = 100.;
    i.braking.pedal_right_pct = 100.;
    let out = run(&mut sys, &i, 1.);
    assert!(!out.aurals.max_braking);
    assert!(out.aurals.set_max_reverse);

    i.reversers.tla = [0., -100., -100., 0.];
    i.aircraft.ground_speed_kt = 70.;
    i.aircraft.dist_from_threshold_m = 1500.;
    i.aircraft.long_accel_ms2 = -0.7;
    let out = run(&mut sys, &i, 1.);
    assert!(out.keep_max_reverse_request);
    assert!(out.aurals.keep_max_reverse);
    assert!(!out.aurals.set_max_reverse);

    i.keep_max_reverse_played = true;
    let out = run(&mut sys, &i, 0.1);
    assert!(!out.aurals.keep_max_reverse);
    i.keep_max_reverse_played = false;
    let out = run(&mut sys, &i, 1.);
    assert!(!out.aurals.keep_max_reverse);
}

#[test]
fn no_false_alert_during_the_transition_after_touchdown() {
    let mut sys = RowRop::default();
    let i = rollout(3500., RunwayCondition::Dry, 300., 140., 0.);
    for _ in 0..40 {
        let out = sys.update(DT, &i);
        assert!(!out.overrun_detected);
    }
}

#[test]
fn forward_thrust_inhibits_rop_aurals() {
    let mut sys = RowRop::default();
    let mut i = rollout(2200., RunwayCondition::Wet, 1100., 126., 0.8);
    i.reversers.tla = [30., 30., 30., 30.];
    let out = run(&mut sys, &i, 7.);
    assert_eq!(out.aurals, Aurals::default());
    assert!(!out.auto_max_braking);
}

#[test]
fn rop_lost_and_rto_inhibit() {
    let mut sys = RowRop::default();
    let mut i = rollout(2200., RunwayCondition::Wet, 1100., 126., 0.8);
    i.braking.autobrake_mode = AutobrakeMode::L2;
    i.braking.autobrake_active = true;
    i.rop_lost = true;
    let out = run(&mut sys, &i, 7.);
    assert!(!out.auto_max_braking && !out.rop_operative);

    let mut i = rollout(2200., RunwayCondition::Wet, 1100., 126., 0.8);
    i.braking.autobrake_mode = AutobrakeMode::Rto;
    i.braking.autobrake_active = true;
    let out = run(&mut sys, &i, 7.);
    assert!(!out.auto_max_braking && !out.overrun_detected);
}

#[test]
fn max_braking_stays_latched_then_releases_when_stop_is_assured() {
    let mut sys = RowRop::default();
    let mut i = rollout(2200., RunwayCondition::Wet, 1100., 126., 0.8);
    i.braking.autobrake_mode = AutobrakeMode::High;
    i.braking.autobrake_active = true;
    assert!(run(&mut sys, &i, 7.).auto_max_braking);

    i.aircraft.ground_speed_kt = 60.;
    i.aircraft.dist_from_threshold_m = 1200.;
    i.aircraft.long_accel_ms2 = -2.5;
    let out = run(&mut sys, &i, 1.);
    assert!(!out.auto_max_braking);

    i.aircraft.ground_speed_kt = 126.;
    i.aircraft.dist_from_threshold_m = 1100.;
    i.aircraft.long_accel_ms2 = -0.8;
    assert!(run(&mut sys, &i, 1.).auto_max_braking);
    i.braking.autobrake_active = false;
    assert!(!run(&mut sys, &i, 0.2).auto_max_braking);
}

#[test]
fn max_braking_is_held_for_a_minimum_time() {
    let mut sys = RowRop::default();
    let mut i = rollout(2200., RunwayCondition::Wet, 1100., 126., 0.8);
    i.braking.autobrake_mode = AutobrakeMode::High;
    i.braking.autobrake_active = true;
    run(&mut sys, &i, 5.1);
    let mut latched_at = RowRop::default();
    let mut k = 0;
    while !latched_at.update(DT, &i).auto_max_braking {
        k += 1;
        assert!(k < 200);
    }
    i.aircraft.ground_speed_kt = 60.;
    i.aircraft.dist_from_threshold_m = 1200.;
    i.aircraft.long_accel_ms2 = -2.5;
    assert!(run(&mut latched_at, &i, 2.).auto_max_braking);
    assert!(!run(&mut latched_at, &i, 4.).auto_max_braking);
}

#[test]
fn btv_is_overridden_by_rop() {
    let mut sys = RowRop::default();
    let mut i = rollout(2200., RunwayCondition::Wet, 1100., 126., 0.8);
    i.braking.autobrake_mode = AutobrakeMode::Btv;
    i.braking.autobrake_active = true;
    let out = run(&mut sys, &i, 7.);
    assert!(out.auto_max_braking && out.btv_overridden);
}

#[test]
fn bounce_window_keeps_rop_aurals_for_four_and_a_half_seconds() {
    let mut sys = RowRop::default();
    let mut i = rollout(2200., RunwayCondition::Wet, 1100., 126., 0.8);
    assert!(run(&mut sys, &i, 7.).aurals.max_braking);
    i.aircraft.flight_phase = 9;
    assert!(run(&mut sys, &i, 3.).aurals.max_braking);
    assert!(!run(&mut sys, &i, 3.).aurals.max_braking);
}

#[test]
fn landing_distance_model_ordering() {
    let cfg = Config::default();
    let roll = |cond, slope, rev, lvl| Roll {
        speed_ms: 70.,
        condition: cond,
        slope_pct: slope,
        weight_kg: 380_000.,
        reversers: rev,
        reverse_level: lvl,
        spoilers: true,
    };
    let dry = ground_roll_distance(&cfg, &roll(RunwayCondition::Dry, 0., 0, 0.));
    let wet = ground_roll_distance(&cfg, &roll(RunwayCondition::Wet, 0., 0, 0.));
    let poor = ground_roll_distance(&cfg, &roll(RunwayCondition::Poor, 0., 0, 0.));
    assert!(dry < wet && wet < poor);
    assert!((dry - (70. * 5. + 70. * 70. / 5.6)).abs() < 10.);

    let downhill = ground_roll_distance(&cfg, &roll(RunwayCondition::Wet, -2., 0, 0.));
    let uphill = ground_roll_distance(&cfg, &roll(RunwayCondition::Wet, 2., 0, 0.));
    assert!(uphill < wet && wet < downhill);

    let rev = ground_roll_distance(&cfg, &roll(RunwayCondition::Wet, 0., 2, 1.));
    assert!(rev < wet);

    let heavy = braking_distance(&cfg, &Roll { weight_kg: 500_000., ..roll(RunwayCondition::Wet, 0., 2, 1.) });
    let light = braking_distance(&cfg, &Roll { weight_kg: 300_000., ..roll(RunwayCondition::Wet, 0., 2, 1.) });
    assert!(light < heavy);

    let head = touchdown_ground_speed_ms(&cfg, 145., 20.);
    let tail = touchdown_ground_speed_ms(&cfg, 145., -10.);
    assert!(head < 145. * KT_TO_MS && tail > 145. * KT_TO_MS);
}

#[test]
fn lost_flags_and_word_encoding() {
    let mut a = vars::Availability {
        ir_available: 3,
        adr_available: 3,
        ra_available: 2,
        fws_audio_available: 2,
        ..Default::default()
    };
    assert_eq!(vars::row_rop_lost(&a), (false, false));
    a.oans_ppos_lost = true;
    assert_eq!(vars::row_rop_lost(&a), (false, true));
    a.oans_ppos_lost = false;
    a.green_hyd_lost = true;
    assert_eq!(vars::row_rop_lost(&a), (true, true));
    assert_eq!(vars::word_raw(vars::bit(15), true), 16384. + 3. * 4_294_967_296.);
    assert_eq!(vars::word_raw(0, false), 0.);
}

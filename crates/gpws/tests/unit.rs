use gpws::arinc::{DiscreteWord, Ssm, Word};
use gpws::bridge::{self, nd_mode, BridgeInputs, EfisSideInputs};
use gpws::callouts::{CalloutContext, Callouts, Event, DEFAULT_AUTO_CALL_OUTS};
use gpws::gpws::phase;
use gpws::sound::{SoundManager, PULL_UP, SINK_RATE};
use gpws::{Gpws, Inputs};

fn airborne() -> Inputs {
    Inputs {
        radio_alt_1: Word::normal(800.),
        radio_alt_2: Word::normal(800.),
        baro_alt: Word::normal(1400.),
        taws_selected: 1.,
        flight_phase: phase::APPROACH,
        flaps_handle_index: 4.,
        gear_pct_extended: 100.,
        airspeed_kt: 140.,
        ..Inputs::default()
    }
}

#[test]
fn arinc_words_round_trip_through_the_raw_encoding() {
    let w = Word::new(1234.5, Ssm::FunctionalTest);
    let back = Word::from_raw(w.to_raw());
    assert_eq!(back, w);
    assert_eq!(Word::from_raw(0.).ssm, Ssm::FailureWarning);
    assert_eq!(Word::from_raw(Word::normal(-3.25).to_raw()).value, -3.25);
}

#[test]
fn discrete_words_are_float_encoded_bit_fields() {
    let mut w = DiscreteWord::empty();
    w.ssm = Ssm::NormalOperation;
    w.set_bit(11, true);
    w.set_bit(15, true);
    assert_eq!(w.bits(), (1 << 10) | (1 << 14));
    let read = Word::from_raw(w.raw());
    assert!(read.bit_value(11) && read.bit_value(15) && !read.bit_value(12));
}

#[test]
fn mode_1_sink_rate_and_pull_up_envelopes() {
    let mut g = Gpws::new();
    let mut i = airborne();
    i.vertical_speed_fpm = -1500.;
    i.radio_alt_1 = Word::normal(300.);
    g.gpws(125., &i);
    assert_eq!(g.mode_states()[0], 1);
    i.radio_alt_1 = Word::normal(5000.);
    g.gpws(125., &i);
    assert_eq!(g.mode_states()[0], 0);
    i.radio_alt_1 = Word::normal(100.);
    i.vertical_speed_fpm = -2500.;
    g.gpws(125., &i);
    assert_eq!(g.mode_states()[0], 2);
    assert!(g.outputs().gpws_warning_active);
    i.vertical_speed_fpm = -900.;
    g.gpws(125., &i);
    assert_eq!(g.mode_states()[0], 0);
    assert!(!g.outputs().gpws_warning_active);
}

#[test]
fn pull_up_sets_the_lamp_but_not_discrete_bit_12_like_the_javascript() {
    let mut g = Gpws::new();
    let mut i = airborne();
    i.vertical_speed_fpm = -2500.;
    i.radio_alt_1 = Word::normal(100.);
    g.gpws(125., &i);
    assert_eq!(g.mode_states()[0], 2);
    assert!(!Word::from_raw(g.outputs().alert_word_1).bit_value(12));
    assert!(Word::from_raw(g.outputs().alert_word_2).bit_value(12));
}

#[test]
fn gpws_is_inhibited_outside_ten_to_2450_feet_and_on_failure() {
    let mut i = airborne();
    i.vertical_speed_fpm = -3000.;
    i.radio_alt_1 = Word::normal(9.);
    let mut g = Gpws::new();
    g.gpws(125., &i);
    assert_eq!(g.mode_states(), [0; 5]);

    i.radio_alt_1 = Word::normal(300.);
    i.gpws_failed = [true, false];
    g.gpws(125., &i);
    assert_eq!(g.mode_states(), [0; 5]);
    i.gpws_failed = [false, false];
    i.gpws_sys_off = true;
    g.gpws(125., &i);
    assert_eq!(g.mode_states(), [0; 5]);
    i.gpws_sys_off = false;
    i.taws_selected = 0.;
    g.gpws(125., &i);
    assert_eq!(g.mode_states(), [0; 5]);
    i.taws_selected = 2.;
    g.gpws(125., &i);
    assert_eq!(g.mode_states()[0], 2);
}

#[test]
fn radio_altimeter_two_is_used_when_one_fails() {
    let mut i = airborne();
    i.vertical_speed_fpm = -3000.;
    i.radio_alt_1 = Word::new(0., Ssm::FailureWarning);
    i.radio_alt_2 = Word::normal(100.);
    let mut g = Gpws::new();
    g.gpws(125., &i);
    assert_eq!(g.mode_states()[0], 2);
    i.radio_alt_1 = Word::new(0., Ssm::FunctionalTest);
    g.gpws(125., &i);
    assert_eq!(g.mode_states()[0], 0);
}

#[test]
fn mode_5_glideslope_alert_uses_dots_of_deviation() {
    let mut g = Gpws::new();
    let mut i = airborne();
    i.radio_alt_1 = Word::normal(250.);
    i.gs_valid = true;
    i.gs_deviation = -0.6;
    g.gpws(125., &i);
    assert_eq!(g.mode_states()[4], 1);
    assert!(g.outputs().gs_warning_active);
    i.gs_deviation = -1.;
    g.gpws(125., &i);
    assert_eq!(g.mode_states()[4], 2);
    i.gpws_gs_off = true;
    g.gpws(125., &i);
    assert_eq!(g.mode_states()[4], 0);
    assert!(!g.outputs().gs_warning_active);
}

#[test]
fn mode_3_flags_altitude_loss_after_take_off() {
    let mut g = Gpws::new();
    let mut i = airborne();
    i.flight_phase = phase::TAKEOFF;
    i.radio_alt_1 = Word::normal(300.);
    i.plane_altitude_ft = 1000.;
    g.gpws(125., &i);
    assert_eq!(g.mode_states()[2], 0);
    i.plane_altitude_ft = 960.;
    i.radio_alt_1 = Word::normal(260.);
    g.gpws(125., &i);
    assert_eq!(g.mode_states()[2], 1);
    assert!(Word::from_raw(g.outputs().alert_word_1).bit_value(14));
}

#[test]
fn windshear_needs_flaps_height_and_a_high_f_factor() {
    let mut g = Gpws::new();
    let mut i = airborne();
    i.f_factor = 0.2;
    i.flaps_handle_index = 1.;
    i.radio_alt_1 = Word::normal(400.);
    g.gpws(125., &i);
    assert!(g.windshear_warning());
    assert!(Word::from_raw(g.outputs().alert_word_2).bit_value(15));
    i.radio_alt_1 = Word::normal(1301.);
    g.gpws(125., &i);
    assert!(!g.windshear_warning());
    i.radio_alt_1 = Word::normal(30.);
    i.vertical_speed_fpm = -300.;
    g.gpws(125., &i);
    assert!(!g.windshear_warning());
    i.vertical_speed_fpm = 300.;
    g.gpws(125., &i);
    assert!(g.windshear_warning());
}

#[test]
fn minimums_and_hundred_above_calls() {
    let mut g = Gpws::new();
    let mut i = airborne();
    i.decision_height = 200.;
    i.radio_alt_1 = Word::normal(400.);
    g.gpws(125., &i);
    for (ra, expected) in [(400., vec![]), (290., vec![Event::Enqueue("hundred_above")]), (204., vec![Event::Enqueue("minimums")])] {
        i.radio_alt_1 = Word::normal(ra);
        g.gpws(125., &i);
        let events: Vec<Event> = g.take_events().into_iter().filter(|e| !matches!(e, Event::Enqueue(n) if n.starts_with("alt_"))).collect();
        assert_eq!(events, expected, "ra {ra}");
    }
}

#[test]
fn altitude_callouts_fire_on_the_way_down_with_hysteresis() {
    let mut c = Callouts::new();
    let ctx = CalloutContext { auto_call_out_pins: DEFAULT_AUTO_CALL_OUTS, autopilot_active: true, tla: [25.; 4], go_around_phase: false };
    let mut events = Vec::new();
    for _ in 0..25 {
        c.update(3000., &ctx, &mut events);
    }
    assert!(events.is_empty());
    for ra in [2600., 2531., 2500., 1500., 1021., 1020., 900.] {
        c.update(ra, &ctx, &mut events);
    }
    assert_eq!(events, vec![Event::Enqueue("alt_2500"), Event::Enqueue("alt_1000")]);
}

#[test]
fn retard_plays_below_20_without_autopilot_and_stops_at_idle() {
    let mut c = Callouts::new();
    let mut ctx = CalloutContext { auto_call_out_pins: 0, autopilot_active: false, tla: [25.; 4], go_around_phase: false };
    let mut events = Vec::new();
    c.update(50., &ctx, &mut events);
    c.update(19., &ctx, &mut events);
    assert_eq!(events, vec![Event::Enqueue("retard")]);
    ctx.tla[2] = 2.;
    c.update(10., &ctx, &mut events);
    assert_eq!(events.last(), Some(&Event::Dequeue("retard")));
}

#[test]
fn periodic_sounds_repeat_after_their_period() {
    let mut s = SoundManager::new();
    s.add_periodic_sound(Some(SINK_RATE), 1.1);
    let mut plays = Vec::new();
    for _ in 0..80 {
        s.update(50.);
        plays.extend(s.take_played());
    }
    assert_eq!(plays.len(), 4);
    s.add_periodic_sound(Some(PULL_UP), 0.1);
    assert_eq!(s.periodic_names(), vec!["aural_sink_rate_new", "aural_pullup_new"]);
}

#[test]
fn mode_2_is_translated_but_not_run_by_the_tick() {
    assert!(!Gpws::MODE_2_ENABLED);
    let mut g = Gpws::new();
    for _ in 0..8 {
        g.mode_2(500., 200., false, false);
    }
    assert_eq!(g.mode_states()[1], 0);
}

fn healthy_bridge() -> BridgeInputs {
    let ok = Word::normal(10.);
    BridgeInputs {
        ac_ess_powered: true,
        ac_4_powered: true,
        dc_ess_in_flight_powered: true,
        dc_1_powered: true,
        dc_2_powered: true,
        taws_wxr_selected: 1.,
        ir_heading: [ok; 3],
        ir_track: [ok; 3],
        ir_latitude: [ok; 3],
        ir_maint: [ok; 3],
        efis: [EfisSideInputs { nd_range_index: 3, nd_mode: nd_mode::ARC, overlay: 2, ..Default::default() }; 2],
        ..BridgeInputs::default()
    }
}

#[test]
fn bridge_failure_flags_follow_power_resets_and_extreme_latitude() {
    let mut b = healthy_bridge();
    let o = bridge::update(&b);
    assert_eq!(o.terr_failed, [false, false]);
    assert_eq!(o.gpws_failed, [false, false]);
    assert_eq!(o.wxr_failed, [true, true]);
    assert!(o.efis[0].terr_on_nd && o.efis[0].terr_on_vd && o.efis[1].terr_on_nd);
    assert_eq!(o.efis[0].nd_range, 40);

    b.aesu_reset_pulled[0] = true;
    let o = bridge::update(&b);
    assert_eq!(o.terr_failed, [true, false]);
    assert_eq!(o.gpws_failed, [true, false]);
    assert!(o.terr_failed_selected && !o.efis[0].terr_on_nd);

    b.aesu_reset_pulled[0] = false;
    b.ir_maint[0] = Word::normal((1 << 14) as f32);
    let o = bridge::update(&b);
    assert_eq!(o.terr_failed, [true, true]);
    assert_eq!(o.gpws_failed, [false, false]);

    let mut b = healthy_bridge();
    b.taws_wxr_selected = 0.;
    assert!(bridge::update(&b).terr_failed_selected);
}

#[test]
fn bridge_display_availability_and_track_line() {
    let mut b = healthy_bridge();
    b.att_hdg_switching = 1.;
    b.ir_heading[0] = Word::failed();
    let o = bridge::update(&b);
    assert!(!o.efis[0].terr_on_nd && o.efis[1].terr_on_nd);
    b.att_hdg_switching = 2.;
    let o = bridge::update(&b);
    assert!(o.efis[0].terr_on_nd);

    let mut b = healthy_bridge();
    b.efis[0].nd_mode = nd_mode::PLAN;
    assert!(!bridge::update(&b).efis[0].terr_on_nd);
    b.efis[0].nd_mode = nd_mode::ROSE_NAV;
    let o = bridge::update(&b);
    assert!(o.efis[0].terr_on_vd && !o.efis[0].arc_mode);

    b.active_lateral_mode = bridge::lateral_mode::HDG;
    assert!(bridge::update(&b).manual_azim_enabled);
    b.armed_lateral_bitmask = 1;
    assert!(!bridge::update(&b).manual_azim_enabled);
    b.armed_lateral_bitmask = 0;
    b.active_lateral_mode = 20;
    assert!(!bridge::update(&b).manual_azim_enabled);
}

#[test]
fn flap_inhibit_from_the_mfd_variable_silences_mode_4_flaps() {
    let mut i = airborne();
    i.flaps_handle_index = 2.;
    i.radio_alt_1 = Word::normal(200.);
    let mut g = Gpws::new();
    g.gpws(125., &i);
    assert_eq!(g.mode_states()[3], 2);
    assert!(i.set_var("L:A32NX_GPWS_FLAPS_OFF", 1.));
    let mut g = Gpws::new();
    g.gpws(125., &i);
    assert_eq!(g.mode_states()[3], 0);
}

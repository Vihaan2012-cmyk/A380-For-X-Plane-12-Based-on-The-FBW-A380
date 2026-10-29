use super::*;
use crate::doors::{DoorRefusal, DoorState, SlideState};
use crate::evac::EvacRefusal;
use crate::maint::TestResult;
use crate::view::{PageBody, ViewMode};

fn ground() -> Inputs {
    let mut i = Inputs::parked();
    i.flight.parking_brake = true;
    i
}

fn cruise() -> Inputs {
    let mut i = Inputs::parked();
    i.flight = Flight { on_ground: false, gs_kt: 480., alt_ft: 36000., radio_alt_ft: 36000., n1_pct: [85.; 4], engine_state: [1.; 4], cabin_alt_ft: 6500., ..Default::default() };
    i
}

fn run(c: &mut Cids, i: &Inputs, secs: f64) {
    for _ in 0..(secs / 0.1).round() as usize {
        c.update(i, 0.1);
    }
}

fn booted(i: &Inputs) -> Cids {
    let mut c = Cids::default();
    run(&mut c, i, 12.);
    c
}

fn tap(c: &mut Cids, i: &Inputs, ev: Event) -> Outputs {
    let mut x = i.clone();
    x.crew.events = vec![ev];
    let o = c.update(&x, 0.1).clone();
    c.update(i, 0.1);
    o
}

fn chime(target: Target, kind: ChimeKind) -> Chime {
    Chime { target, kind }
}

#[test]
fn directors_come_up_after_power_and_the_lowest_healthy_one_is_active() {
    let i = ground();
    let mut c = Cids::default();
    c.update(&i, 0.1);
    assert!(!c.outputs().cids_ok, "directors are still starting");
    run(&mut c, &i, 9.);
    assert!(c.outputs().cids_ok);
    assert_eq!(c.outputs().active_director, Some(0));
    let mut f = i.clone();
    f.cabin.director_fault[0] = true;
    run(&mut c, &f, 1.);
    assert_eq!(c.outputs().active_director, Some(1));
    assert!(c.outputs().cids_ok, "switchover keeps the system up");
    assert!(c.outputs().ecam.cids_degraded);
    f.cabin.director_fault[0] = false;
    run(&mut c, &f, 1.);
    assert_eq!(c.outputs().active_director, Some(1), "no switch back");
}

#[test]
fn losing_all_three_directors_blanks_the_functions_but_keeps_hardwired_ones() {
    let mut i = ground();
    let mut c = booted(&i);
    i.cabin.director_fault = [true; 3];
    run(&mut c, &i, 1.);
    let o = c.outputs().clone();
    assert!(!o.cids_ok && o.ecam.cids_fault);
    assert_eq!(c.view().body, PageBody::Fault);
    let o2 = tap(&mut c, &i, Event::Door(0));
    assert_eq!(o2.door_toggle, None);
    assert!(c.view().message.unwrap().contains("CIDS FAULT"));
    i.cockpit.seatbelt = Sw3::On;
    run(&mut c, &i, 0.2);
    assert!(c.outputs().seatbelt_sign, "cockpit switches reach the signs directly");
    i.cockpit.emer_call_on = true;
    let o = c.update(&i, 0.1);
    assert!(o.calls.emer_to_cabin);
}

#[test]
fn the_phase_follows_the_flight() {
    let mut p = phase::PhaseTracker::new();
    let mut f = Flight { on_ground: true, ..Default::default() };
    assert_eq!(p.step(&f), Phase::Parked);
    f.n1_pct = [20.; 4];
    f.engine_state = [1.; 4];
    f.gs_kt = 10.;
    assert_eq!(p.step(&f), Phase::TaxiOut);
    f.gs_kt = 90.;
    assert_eq!(p.step(&f), Phase::TakeoffRoll);
    f.on_ground = false;
    f.radio_alt_ft = 300.;
    f.vs_fpm = 2000.;
    assert_eq!(p.step(&f), Phase::Climb);
    f.radio_alt_ft = 36000.;
    f.vs_fpm = 0.;
    assert_eq!(p.step(&f), Phase::Cruise);
    f.vs_fpm = -1500.;
    assert_eq!(p.step(&f), Phase::Descent);
    f.radio_alt_ft = 2500.;
    assert_eq!(p.step(&f), Phase::Approach);
    f.on_ground = true;
    f.gs_kt = 120.;
    assert_eq!(p.step(&f), Phase::Rollout);
    f.gs_kt = 15.;
    assert_eq!(p.step(&f), Phase::TaxiIn);
    f.gs_kt = 0.;
    f.n1_pct = [0.; 4];
    f.engine_state = [0.; 4];
    assert_eq!(p.step(&f), Phase::Parked);
}

#[test]
fn a_cockpit_call_rings_the_cabin_and_lights_the_stations_until_reset() {
    let i = ground();
    let mut c = booted(&i);
    let mut held = i.clone();
    held.cockpit.calls_fwd = true;
    let o = c.update(&held, 0.1).clone();
    assert!(o.chimes.contains(&chime(Target::Cabin, ChimeKind::PilotCall)));
    assert_eq!(o.calls.cabin_call_lights, [true, false, false, false]);
    let o = c.update(&held, 0.1).clone();
    assert!(o.chimes.is_empty(), "one chime per press");
    run(&mut c, &i, 10.);
    assert!(c.outputs().calls.cabin_call_lights[0], "stays lit after release");
    tap(&mut c, &i, Event::CallReset);
    assert!(!c.outputs().calls.cabin_call_lights[0]);
    let mut all = i.clone();
    all.cockpit.calls_all = true;
    c.update(&all, 0.1);
    assert_eq!(c.outputs().calls.cabin_call_lights, [true; 4]);
}

#[test]
fn the_call_light_times_out() {
    let i = ground();
    let mut c = booted(&i);
    let mut held = i.clone();
    held.cockpit.calls_purs = true;
    c.update(&held, 0.1);
    run(&mut c, &i, 31.);
    assert!(!c.outputs().calls.cabin_call_lights[3]);
}

#[test]
fn cabin_calls_reach_the_cockpit_by_priority_and_latch_until_reset() {
    let i = ground();
    let mut c = booted(&i);
    let mut x = i.clone();
    x.crew.attendant_call[1] = true;
    let o = c.update(&x, 0.1).clone();
    assert!(o.chimes.contains(&chime(Target::Cockpit, ChimeKind::CabinCall)));
    assert!(o.calls.cockpit_att_light);
    x.crew.attendant_call[1] = false;
    x.crew.purser_call = true;
    let o = c.update(&x, 0.1).clone();
    assert!(o.chimes.contains(&chime(Target::Cockpit, ChimeKind::PurserCall)));
    x.crew.purser_call = false;
    x.crew.cabin_emer_call = true;
    let o = c.update(&x, 0.1).clone();
    assert!(o.chimes.contains(&chime(Target::Cockpit, ChimeKind::EmergencyCall)));
    assert!(o.calls.cockpit_emer_light && o.ecam.cabin_emer_call);
    x.crew.cabin_emer_call = false;
    let mut repeats = 0;
    for _ in 0..200 {
        if c.update(&x, 0.1).chimes.contains(&chime(Target::Cockpit, ChimeKind::EmergencyCall)) {
            repeats += 1;
        }
    }
    assert_eq!(repeats, 3, "every 6 s for 20 s");
    tap(&mut c, &x, Event::CallReset);
    assert!(!c.outputs().calls.cockpit_emer_light && !c.outputs().calls.cockpit_att_light);
    assert_eq!(c.outputs().calls.pending, 0);
}

#[test]
fn the_interphone_channel_fault_silences_call_chimes() {
    let mut i = ground();
    let mut c = booted(&i);
    i.cabin.channel_fault[3] = 1.;
    run(&mut c, &i, 0.2);
    i.crew.attendant_call[0] = true;
    let o = c.update(&i, 0.1);
    assert!(o.chimes.is_empty());
}

#[test]
fn the_cockpit_emergency_call_pb_sounds_the_cabin_repeatedly() {
    let mut i = ground();
    let mut c = booted(&i);
    i.cockpit.emer_call_on = true;
    let mut n = 0;
    for _ in 0..130 {
        if c.update(&i, 0.1).chimes.contains(&chime(Target::Cabin, ChimeKind::EmergencyCall)) {
            n += 1;
        }
    }
    assert_eq!(n, 3);
    i.cockpit.emer_call_on = false;
    run(&mut c, &i, 10.);
    assert!(!c.outputs().calls.emer_to_cabin);
}

#[test]
fn the_mech_call_horn_sounds_only_on_the_stopped_ground_and_a_ground_call_lights_the_pb() {
    let mut i = ground();
    let mut c = booted(&i);
    i.cockpit.calls_mech = true;
    assert!(c.update(&i, 0.1).calls.mech_horn);
    i.flight.gs_kt = 8.;
    assert!(!c.update(&i, 0.1).calls.mech_horn);
    let mut g = ground();
    g.crew.ground_mech_call = true;
    let o = c.update(&g, 0.1).clone();
    assert!(o.chimes.contains(&chime(Target::Cockpit, ChimeKind::MechCall)));
    assert!(o.calls.cockpit_mech_light);
    let mut press = ground();
    press.cockpit.calls_mech = true;
    c.update(&press, 0.1);
    assert!(!c.outputs().calls.cockpit_mech_light, "answering clears the light");
}

#[test]
fn evac_from_the_cabin_needs_a_second_press_and_the_purser_selection() {
    let mut i = ground();
    let mut c = booted(&i);
    let o = tap(&mut c, &i, Event::EvacCmd);
    assert!(!o.evac.active && o.evac.guard_open);
    assert!(c.view().message.is_some());
    let o = tap(&mut c, &i, Event::EvacCmd);
    assert!(o.evac.active && o.evac.horn_cockpit && o.evac.horn_cabin);
    assert!(o.evac.cockpit_pb_flash && o.evac.from_cabin);
    assert!(o.ecam.evac && o.emer_lights && o.full_bright);
    assert_eq!(o.cabin_level, Some(1.));
    let o = tap(&mut c, &i, Event::EvacReset);
    assert!(!o.evac.active);

    i.cockpit.evac_capt_only = true;
    run(&mut c, &i, 4.);
    tap(&mut c, &i, Event::EvacCmd);
    let o = tap(&mut c, &i, Event::EvacCmd);
    assert!(!o.evac.active);
    assert_eq!(o.evac.refused, Some(EvacRefusal::CaptOnly));
}

#[test]
fn the_evac_guard_closes_after_its_window() {
    let i = ground();
    let mut c = booted(&i);
    tap(&mut c, &i, Event::EvacCmd);
    run(&mut c, &i, 4.);
    let o = tap(&mut c, &i, Event::EvacCmd);
    assert!(!o.evac.active, "the first press is a fresh guard again");
}

#[test]
fn the_cockpit_evac_command_and_horn_shutoff_work_without_the_fap() {
    let mut i = ground();
    let mut c = booted(&i);
    i.cockpit.evac_cmd = true;
    let o = c.update(&i, 0.1).clone();
    assert!(o.evac.active && o.evac.horn_cockpit && !o.evac.from_cabin);
    i.cockpit.evac_horn_shutoff = true;
    c.update(&i, 0.1);
    i.cockpit.evac_horn_shutoff = false;
    let o = c.update(&i, 0.1).clone();
    assert!(!o.evac.horn_cockpit && o.evac.horn_cabin, "the shut off silences the cockpit only");
    let o = tap(&mut c, &i, Event::EvacReset);
    assert!(o.evac.active);
    assert_eq!(o.evac.refused, Some(EvacRefusal::CockpitCommandOn));
    i.cockpit.evac_cmd = false;
    run(&mut c, &i, 0.2);
    i.cockpit.evac_cmd = true;
    assert!(c.update(&i, 0.1).evac.horn_cockpit, "a new command sounds the horn again");
}

#[test]
fn evac_overrides_music_announcements_and_the_light_scene() {
    let i = ground();
    let mut c = booted(&i);
    tap(&mut c, &i, Event::Announce(4));
    assert_eq!(c.outputs().pa.deck, [PaSource::PreRecorded(4); 3]);
    tap(&mut c, &i, Event::Scene(7));
    let mut e = i.clone();
    e.cockpit.evac_cmd = true;
    let o = c.update(&e, 0.1).clone();
    assert_eq!(o.pa.deck, [PaSource::None; 3]);
    assert!(o.full_bright);
}

#[test]
fn seat_belt_and_no_smoking_signs_follow_their_switches() {
    let mut i = cruise();
    i.cockpit.seatbelt = Sw3::Auto;
    let mut c = booted(&i);
    assert!(!c.outputs().seatbelt_sign);
    i.flight.slats_deg = 20.;
    let o = c.update(&i, 0.1).clone();
    assert!(o.seatbelt_sign);
    assert!(o.chimes.contains(&chime(Target::Cabin, ChimeKind::Sign)));
    assert!(!c.update(&i, 0.1).chimes.contains(&chime(Target::Cabin, ChimeKind::Sign)));
    i.cockpit.seatbelt = Sw3::Off;
    assert!(!c.update(&i, 0.1).seatbelt_sign);
    i.cockpit.no_smoking = Sw3::Auto;
    i.flight.gear_down = true;
    assert!(c.update(&i, 0.1).no_smoking_sign);
}

#[test]
fn excessive_cabin_altitude_forces_the_signs_on() {
    let mut i = cruise();
    i.cockpit.seatbelt = Sw3::Off;
    i.cockpit.no_smoking = Sw3::Off;
    let mut c = booted(&i);
    i.flight.cabin_alt_ft = 12_000.;
    let o = c.update(&i, 0.1).clone();
    assert!(o.seatbelt_sign && o.no_smoking_sign && o.ecam.excess_cabin_alt_signs);
    assert!(o.cautions.iter().any(|x| x.text == "CABIN ALT SIGNS ON"));
}

#[test]
fn emergency_lights_follow_the_arm_switch_power_and_the_fap() {
    let mut i = ground();
    let mut c = booted(&i);
    assert!(!c.outputs().emer_lights);
    tap(&mut c, &i, Event::EmerLight(true));
    assert!(c.outputs().emer_lights);
    tap(&mut c, &i, Event::EmerLight(false));
    assert!(!c.outputs().emer_lights);
    i.power.ac_cabin = false;
    i.power.dc_ess = true;
    assert!(c.update(&i, 0.1).emer_lights, "armed lights come on at power loss");
    assert!(c.outputs().exit_signs);
    i.cockpit.emer_exit = EmerSw::Off;
    assert!(!c.update(&i, 0.1).emer_lights);
    i.cockpit.emer_exit = EmerSw::On;
    i.power.ac_cabin = true;
    assert!(c.update(&i, 0.1).emer_lights);
    let mut j = ground();
    j.cockpit.emer_exit = EmerSw::Off;
    let mut c = booted(&j);
    tap(&mut c, &j, Event::EmerLight(true));
    assert!(!c.outputs().emer_lights, "a cockpit OFF is not overridden by the FAP");
}

#[test]
fn live_pa_beats_announcements_which_beat_music_and_failed_decks_go_silent() {
    let mut i = ground();
    let mut c = booted(&i);
    tap(&mut c, &i, Event::Music(true));
    assert_eq!(c.outputs().pa.deck, [PaSource::Music; 3]);
    assert!(c.outputs().pa.music_volume > 0.);
    tap(&mut c, &i, Event::Announce(5));
    assert_eq!(c.outputs().pa.deck, [PaSource::PreRecorded(5); 3]);
    assert_eq!(c.outputs().pa.music_volume, 0., "music ducks under an announcement");
    i.crew.handset_pa = true;
    assert_eq!(c.update(&i, 0.1).pa.deck, [PaSource::Attendant; 3]);
    i.crew.cockpit_pa = true;
    assert_eq!(c.update(&i, 0.1).pa.deck, [PaSource::Cockpit; 3]);
    i.crew.cockpit_pa = false;
    i.crew.handset_pa = false;
    i.cabin.channel_fault[0] = 1.;
    let o = c.update(&i, 0.1).clone();
    assert_eq!(o.pa.deck[0], PaSource::None);
    assert_eq!(o.pa.deck[1], PaSource::PreRecorded(5));
    assert!(o.ecam.pa_fault && o.pa.degraded);
    assert!(o.cautions.iter().any(|x| x.text == "PA UPPER FAULT"));
}

#[test]
fn a_pre_recorded_announcement_runs_its_length_and_can_be_stopped() {
    let i = ground();
    let mut c = booted(&i);
    tap(&mut c, &i, Event::Announce(3));
    run(&mut c, &i, 20.);
    assert_eq!(c.outputs().pa.announcement, Some(3));
    run(&mut c, &i, 6.);
    assert_eq!(c.outputs().pa.announcement, None);
    tap(&mut c, &i, Event::Announce(9));
    tap(&mut c, &i, Event::Announce(0));
    assert_eq!(c.outputs().pa.deck, [PaSource::None; 3]);
}

#[test]
fn the_automatic_announcement_scheduler_plays_through_the_cids() {
    let mut i = ground();
    let mut c = booted(&i);
    i.flight.auto_announcement = Some(6);
    c.update(&i, 0.1);
    i.flight.auto_announcement = None;
    assert_eq!(c.update(&i, 0.1).pa.announcement, Some(6));
    i.flight.auto_announcement = Some(1);
    c.update(&i, 0.1);
    assert!(c.outputs().pa.music_on, "request 1 is boarding music");
}

#[test]
fn boarding_music_starts_with_the_doors_and_ends_when_they_close() {
    let mut i = ground();
    let mut c = booted(&i);
    assert!(!c.outputs().pa.music_on);
    i.doors[0].open = 1.;
    run(&mut c, &i, 0.3);
    assert!(c.outputs().pa.music_on);
    i.doors[0].open = 0.;
    run(&mut c, &i, 0.3);
    assert!(!c.outputs().pa.music_on);
    i.doors[0].open = 1.;
    run(&mut c, &i, 0.3);
    assert!(c.outputs().pa.music_on);
    tap(&mut c, &i, Event::StopAudio);
    run(&mut c, &i, 2.);
    assert!(!c.outputs().pa.music_on, "a crew stop sticks until the doors close");
    i.doors[0].open = 0.;
    run(&mut c, &i, 0.3);
    i.doors[0].open = 1.;
    run(&mut c, &i, 0.3);
    assert!(c.outputs().pa.music_on);
}

#[test]
fn music_track_and_volume_are_clamped() {
    let i = ground();
    let mut c = booted(&i);
    tap(&mut c, &i, Event::MusicTrack(9));
    tap(&mut c, &i, Event::MusicVolume(200));
    tap(&mut c, &i, Event::Music(true));
    let o = c.outputs();
    assert_eq!(o.pa.music_track, 4);
    assert_eq!(o.pa.music_volume, 1.);
}

#[test]
fn fap_doors_obey_the_ground_interlocks() {
    let mut i = ground();
    let mut c = booted(&i);
    let o = tap(&mut c, &i, Event::Door(2));
    assert_eq!(o.door_toggle, Some(2));
    i.flight.engine_state = [0., 0., 1., 0.];
    let o = tap(&mut c, &i, Event::Door(2));
    assert_eq!(o.door_refused, Some(DoorRefusal::EngineRunning));
    assert!(c.view().message.unwrap().contains("ENGINE"));
    i.flight.engine_state = [0.; 4];
    i.flight.gs_kt = 3.;
    assert_eq!(tap(&mut c, &i, Event::Door(2)).door_refused, Some(DoorRefusal::NotStationary));
    i.flight.gs_kt = 0.;
    i.flight.cabin_dp_psi = 2.;
    assert_eq!(tap(&mut c, &i, Event::Door(2)).door_refused, Some(DoorRefusal::Pressurised));
    i.flight.cabin_dp_psi = 0.;
    i.doors[2].slide_armed = true;
    assert_eq!(tap(&mut c, &i, Event::Door(2)).door_refused, Some(DoorRefusal::SlideArmed));
    i.doors[2].open = 1.;
    assert_eq!(tap(&mut c, &i, Event::Door(2)).door_toggle, Some(2), "closing an armed door is fine");
    assert_eq!(tap(&mut c, &i, Event::Door(40)).door_refused, Some(DoorRefusal::BadDoor));
    assert_eq!(tap(&mut c, &i, Event::Door(16)).door_toggle, Some(16), "cargo doors have no slides");
}

#[test]
fn the_doors_page_shows_door_and_slide_state() {
    let mut i = ground();
    i.doors[0].open = 0.5;
    i.doors[1].slide_armed = true;
    i.doors[3].open = 1.;
    let mut c = booted(&i);
    tap(&mut c, &i, Event::SelectPage(Page::Doors));
    let PageBody::Doors { doors, slides_armed, pax_closed } = c.view().body else { panic!() };
    assert_eq!(doors[0].state, DoorState::Moving);
    assert_eq!(doors[1].slide, SlideState::Armed);
    assert_eq!(doors[3].state, DoorState::Open);
    assert_eq!(doors[16].slide, SlideState::NotFitted);
    assert_eq!(slides_armed, 1);
    assert!(!pax_closed);
}

#[test]
fn opening_a_door_with_the_slide_armed_warns_the_cabin() {
    let mut i = ground();
    i.doors[4].slide_armed = true;
    let mut c = booted(&i);
    i.doors[4].open = 0.3;
    let o = c.update(&i, 0.1).clone();
    assert_eq!(c.doors_out.open_while_armed, Some(4));
    assert!(o.chimes.contains(&chime(Target::Cabin, ChimeKind::DoorWarning)));
    assert!(o.cautions.iter().any(|x| x.text.contains("M3L OPENED, SLIDE ARMED") && x.level == Level::Red));
    run(&mut c, &i, 11.);
    assert!(!c.outputs().cautions.iter().any(|x| x.text.contains("SLIDE ARMED")));
}

#[test]
fn departure_needs_closed_doors_and_armed_slides() {
    let mut i = ground();
    i.flight.engine_state = [1.; 4];
    i.flight.n1_pct = [25.; 4];
    i.flight.gs_kt = 12.;
    let mut c = booted(&i);
    assert!(c.outputs().ecam.slides_not_armed);
    assert!(c.outputs().cautions.iter().any(|x| x.text == "SLIDES NOT ARMED 16" && x.level == Level::Amber));
    for d in 0..PAX_DOORS {
        i.doors[d].slide_armed = true;
    }
    run(&mut c, &i, 0.2);
    assert!(!c.outputs().ecam.slides_not_armed);
    i.doors[5].open = 1.;
    run(&mut c, &i, 0.2);
    assert!(c.outputs().ecam.door_open_in_departure);
    assert_eq!(c.outputs().cautions[0].level, Level::Red);
    i.doors[16].open = 1.;
    run(&mut c, &i, 0.2);
    assert!(c.outputs().cautions.iter().any(|x| x.text == "DOOR CARGO FWD OPEN"));
}

#[test]
fn door_latch_and_slide_pressure_faults_are_cautions() {
    let mut i = ground();
    i.doors[1].latch_fault = true;
    i.doors[7].slide_press_low = true;
    let c = booted(&i);
    let t: Vec<_> = c.outputs().cautions.iter().map(|x| x.text.clone()).collect();
    assert!(t.contains(&"DOOR M1R LATCH".to_string()) && t.contains(&"SLIDE M4R PRESS LO".to_string()));
}

#[test]
fn cabin_smoke_sounds_the_horn_raises_an_emergency_call_and_resets_when_clear() {
    let mut i = ground();
    let mut c = booted(&i);
    i.cabin.smoke[5] = true;
    let o = c.update(&i, 0.1).clone();
    assert!(o.smoke_horn && o.ecam.cabin_smoke);
    assert!(o.calls.cockpit_emer_light);
    assert!(o.chimes.contains(&chime(Target::Cockpit, ChimeKind::EmergencyCall)));
    assert!(o.cautions.iter().any(|x| x.text == "SMOKE GALLEY MID"));
    let o = tap(&mut c, &i, Event::SmokeReset);
    assert!(!o.smoke_horn);
    assert!(c.outputs().ecam.cabin_smoke, "the detection stays while smoke is present");
    i.cabin.smoke[5] = false;
    run(&mut c, &i, 0.2);
    assert!(!c.outputs().ecam.cabin_smoke);
    i.cabin.smoke[6] = true;
    assert!(c.update(&i, 0.1).smoke_horn, "a new detection sounds again");
}

#[test]
fn cargo_smoke_is_shown_but_does_not_call_the_cockpit_as_cabin_smoke_does() {
    let mut i = ground();
    let mut c = booted(&i);
    i.cabin.smoke[10] = true;
    let o = c.update(&i, 0.1).clone();
    assert!(o.ecam.cargo_smoke && !o.ecam.cabin_smoke);
    assert!(!o.calls.cockpit_emer_light);
    tap(&mut c, &i, Event::SelectPage(Page::Smoke));
    let PageBody::Smoke { rows, .. } = c.view().body else { panic!() };
    assert!(rows.iter().any(|(n, on)| n == "CARGO FWD" && *on));
}

#[test]
fn cabin_ready_reports_checks_and_resets_with_the_phase_and_the_doors() {
    let mut i = ground();
    for d in 0..PAX_DOORS {
        i.doors[d].slide_armed = true;
    }
    i.cockpit.seatbelt = Sw3::On;
    let mut c = booted(&i);
    for a in 0..AREAS {
        tap(&mut c, &i, Event::AreaReady(a));
    }
    let o = tap(&mut c, &i, Event::CabinReady);
    assert!(o.cabin_ready && !o.cabin_ready_incomplete);
    assert!(o.chimes.contains(&chime(Target::Cockpit, ChimeKind::CabinReady)));
    assert!(c.outputs().cabin_ready);
    i.doors[3].open = 1.;
    run(&mut c, &i, 0.2);
    assert!(!c.outputs().cabin_ready, "opening a door cancels it");
}

#[test]
fn cabin_ready_with_open_items_is_allowed_by_default_but_flagged() {
    let i = ground();
    let mut c = booted(&i);
    let o = tap(&mut c, &i, Event::CabinReady);
    assert!(o.cabin_ready && o.cabin_ready_incomplete);
    tap(&mut c, &i, Event::SelectPage(Page::Ready));
    let PageBody::Ready { checks, .. } = c.view().body else { panic!() };
    assert!(checks.iter().any(|k| !k.ok));
}

#[test]
fn strict_cabin_ready_refuses_with_open_items() {
    let i = ground();
    let mut c = Cids::new(Config { strict_cabin_ready: true, ..Default::default() });
    run(&mut c, &i, 12.);
    let o = tap(&mut c, &i, Event::CabinReady);
    assert!(!o.cabin_ready);
    assert!(c.view().message.unwrap().contains("CABIN READY"));
}

#[test]
fn cabin_ready_is_cleared_when_the_flight_moves_to_the_next_stage() {
    let i = ground();
    let mut c = booted(&i);
    tap(&mut c, &i, Event::CabinReady);
    assert!(c.outputs().cabin_ready);
    let mut f = cruise();
    f.flight.radio_alt_ft = 5000.;
    f.flight.vs_fpm = 1500.;
    run(&mut c, &f, 0.2);
    assert!(!c.outputs().cabin_ready);
    f.flight.vs_fpm = -1500.;
    run(&mut c, &f, 0.2);
    let o = tap(&mut c, &f, Event::CabinReady);
    assert!(o.cabin_ready, "ready for landing");
}

#[test]
fn temperature_setpoints_snap_to_half_degrees_within_limits() {
    let i = ground();
    let mut c = booted(&i);
    tap(&mut c, &i, Event::TempSet(0, 17.));
    tap(&mut c, &i, Event::TempSet(1, 26.3));
    tap(&mut c, &i, Event::TempStep(2, 40.));
    tap(&mut c, &i, Event::TempStep(3, -0.5));
    let t = &c.outputs().temp;
    assert_eq!(t.setpoints[0], 18.);
    assert_eq!(t.setpoints[1], 26.5);
    assert_eq!(t.setpoints[2], 30.);
    assert_eq!(t.setpoints[3], 23.5);
    assert!((t.purser_sel_c - 24.).abs() < 1.5);
    tap(&mut c, &i, Event::TempSet(99, 20.));
    tap(&mut c, &i, Event::TempSet(0, f64::NAN));
    assert_eq!(c.outputs().temp.setpoints[0], 18.);
}

#[test]
fn a_purser_temperature_selector_fault_is_flagged() {
    let mut i = ground();
    i.cabin.purser_temp_sel_fault = true;
    let c = booted(&i);
    assert!(!c.outputs().temp.purser_active);
    assert!(c.outputs().cautions.iter().any(|x| x.text == "TEMP SEL FAULT"));
}

#[test]
fn maintenance_tests_run_only_parked_with_the_brake_on() {
    let mut i = ground();
    let mut c = booted(&i);
    let o = tap(&mut c, &i, Event::Test(TestKind::Lamp));
    assert!(o.lamp_test);
    run(&mut c, &i, 3.5);
    assert_eq!(c.test_result(), TestResult::Pass(TestKind::Lamp));
    i.cabin.channel_fault[1] = 1.;
    tap(&mut c, &i, Event::Test(TestKind::Pa));
    run(&mut c, &i, 4.5);
    assert_eq!(c.test_result(), TestResult::Fail(TestKind::Pa, "PA MAIN CHANNEL".into()));
    i.cabin.channel_fault[1] = 0.;
    i.cabin.director_fault[2] = true;
    tap(&mut c, &i, Event::Test(TestKind::Directors));
    run(&mut c, &i, 2.5);
    assert_eq!(c.test_result(), TestResult::Fail(TestKind::Directors, "DIRECTOR 3".into()));
    let f = cruise();
    run(&mut c, &f, 0.5);
    tap(&mut c, &f, Event::Test(TestKind::Lamp));
    assert_eq!(c.test_result(), TestResult::NotAllowed(TestKind::Lamp));
}

#[test]
fn the_pa_test_tone_plays_in_the_cabin() {
    let i = ground();
    let mut c = booted(&i);
    let o = tap(&mut c, &i, Event::Test(TestKind::Pa));
    assert!(o.chimes.contains(&chime(Target::Cabin, ChimeKind::TestTone)));
}

#[test]
fn the_fap_boots_after_power_and_pc_reset() {
    let i = ground();
    let mut c = Cids::default();
    c.update(&i, 0.1);
    assert_eq!(c.view().mode, ViewMode::Booting);
    assert!(c.view().keys.is_empty());
    run(&mut c, &i, 10.);
    assert_eq!(c.view().mode, ViewMode::Live);
    tap(&mut c, &i, Event::SelectPage(Page::Doors));
    tap(&mut c, &i, Event::PcReset);
    assert_eq!(c.view().mode, ViewMode::Booting);
    run(&mut c, &i, 10.);
    assert_eq!(c.view().page, Page::Status);
    let mut dark = i.clone();
    dark.power = Power::default();
    c.update(&dark, 0.1);
    assert_eq!(c.view().mode, ViewMode::Dark);
}

#[test]
fn input_while_booting_or_dark_is_ignored() {
    let i = ground();
    let mut c = Cids::default();
    let o = tap(&mut c, &i, Event::EvacCmd);
    assert!(!o.evac.guard_open);
    let o = tap(&mut c, &i, Event::Door(0));
    assert_eq!(o.door_toggle, None);
}

#[test]
fn screen_off_and_lock_block_touch_but_not_the_hard_keys() {
    let i = ground();
    let mut c = booted(&i);
    tap(&mut c, &i, Event::ScreenOff);
    assert_eq!(c.view().mode, ViewMode::ScreenOff);
    let o = tap(&mut c, &i, Event::Door(0));
    assert_eq!(o.door_toggle, None);
    tap(&mut c, &i, Event::ScreenOn);
    assert_eq!(c.view().mode, ViewMode::Live);
    tap(&mut c, &i, Event::LockScreen);
    assert!(c.view().locked);
    let o = tap(&mut c, &i, Event::Door(0));
    assert_eq!(o.door_toggle, None);
    tap(&mut c, &i, Event::EmerLight(true));
    assert!(c.outputs().emer_lights, "hard keys still work");
    run(&mut c, &i, 31.);
    assert!(!c.view().locked);
    assert_eq!(tap(&mut c, &i, Event::Door(0)).door_toggle, Some(0));
}

#[test]
fn page_navigation_wraps() {
    let i = ground();
    let mut c = booted(&i);
    tap(&mut c, &i, Event::PrevPage);
    assert_eq!(c.view().page, Page::Maintenance);
    tap(&mut c, &i, Event::NextPage);
    assert_eq!(c.view().page, Page::Status);
    for p in Page::ALL {
        tap(&mut c, &i, Event::SelectPage(p));
        let v = c.view();
        assert_eq!(v.page, p);
        assert_eq!(v.title, p.title());
        assert_ne!(v.body, PageBody::Fault, "{:?} has a body", p);
    }
}

#[test]
fn the_light_scene_and_level_encode_like_the_plugin_expects() {
    let i = ground();
    let mut c = booted(&i);
    assert_eq!((c.outputs().cabin_scene, c.outputs().cabin_level), (None, None));
    tap(&mut c, &i, Event::Scene(12));
    assert_eq!(c.outputs().cabin_scene, Some(0));
    tap(&mut c, &i, Event::Scene(7));
    assert_eq!(c.outputs().cabin_scene, Some(7));
    tap(&mut c, &i, Event::Level(25));
    assert_eq!(c.outputs().cabin_level, Some(0.25));
    tap(&mut c, &i, Event::LightsMain);
    assert_eq!(c.outputs().cabin_level, Some(0.0));
    tap(&mut c, &i, Event::LightsMain);
    assert_eq!(c.outputs().cabin_level, Some(1.0));
    tap(&mut c, &i, Event::Scene(0));
    tap(&mut c, &i, Event::Level(255));
    assert_eq!((c.outputs().cabin_scene, c.outputs().cabin_level), (None, None));
}

#[test]
fn equipment_switches_cut_the_loads_and_pax_sys_cuts_seat_power() {
    let i = ground();
    let mut c = booted(&i);
    let o = c.outputs();
    assert!(o.seat_power && o.pax_sys && o.water_heaters && o.galley_on == [true; 3]);
    tap(&mut c, &i, Event::Galley(1, false));
    tap(&mut c, &i, Event::WaterHeaters(false));
    tap(&mut c, &i, Event::PaxSys(false));
    let o = c.outputs();
    assert_eq!(o.galley_on, [true, false, true]);
    assert!(!o.water_heaters && !o.pax_sys && !o.seat_power);
    tap(&mut c, &i, Event::PaxSys(true));
    tap(&mut c, &i, Event::SeatPower(false));
    assert!(!c.outputs().seat_power && c.outputs().pax_sys);
}

#[test]
fn equipment_defaults_to_on_when_the_cids_is_down() {
    let mut i = ground();
    let mut c = booted(&i);
    tap(&mut c, &i, Event::PaxSys(false));
    i.cabin.director_fault = [true; 3];
    run(&mut c, &i, 0.5);
    assert!(c.outputs().pax_sys && c.outputs().seat_power);
}

#[test]
fn cautions_are_sorted_red_first_and_the_fap_shows_three() {
    let mut i = ground();
    i.cabin.waste_full = [true; 3];
    i.cabin.water_fault = true;
    i.cabin.smoke[0] = true;
    let c = booted(&i);
    assert_eq!(c.outputs().cautions[0].level, Level::Red);
    assert_eq!(c.view().cautions.len(), 3);
    assert!(c.outputs().cautions.len() > 3);
}

#[test]
fn calls_page_lists_recent_calls_newest_first() {
    let mut i = ground();
    let mut c = booted(&i);
    i.crew.purser_call = true;
    c.update(&i, 0.1);
    i.crew.purser_call = false;
    i.crew.attendant_call[2] = true;
    c.update(&i, 0.1);
    i.crew.attendant_call[2] = false;
    tap(&mut c, &i, Event::SelectPage(Page::Calls));
    let PageBody::Calls { log, pending, .. } = c.view().body else { panic!() };
    assert_eq!(log[0].text, "ATTENDANT CALL AFT");
    assert_eq!(log[1].text, "PURSER CALL");
    assert_eq!(pending, 2);
    tap(&mut c, &i, Event::CallReset);
    let PageBody::Calls { log, .. } = c.view().body else { panic!() };
    assert!(log.iter().all(|l| l.answered));
}

#[test]
fn nothing_goes_non_finite_with_bad_time_steps() {
    let i = ground();
    let mut c = booted(&i);
    for dt in [0., -1., f64::NAN, f64::INFINITY, 100.] {
        let o = c.update(&i, dt).clone();
        assert!(o.pa.music_volume.is_finite() && o.temp.purser_sel_c.is_finite());
    }
}

#[test]
fn the_status_page_summarises_the_cabin() {
    let mut i = ground();
    i.doors[0].open = 1.;
    let c = booted(&i);
    let PageBody::Status(rows) = c.view().body else { panic!() };
    assert_eq!(rows[0].label, "DOORS");
    assert_eq!(rows[0].value, "1 NOT CLOSED");
    assert!(rows.iter().any(|r| r.label == "DIRECTORS" && r.value == "3 OF 3"));
}

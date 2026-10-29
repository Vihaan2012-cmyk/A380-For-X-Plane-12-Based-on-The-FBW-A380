use crate::*;

const DT: f64 = 0.1;

/// The opening lines of FBW's ENG 1 FIRE (id 260800005, fws_data ata26), as a fixture.
fn eng1_fire() -> EcamProcedure {
    let a = |name, sensed, level, label, time| EcamItem { name, kind: ItemKind::Action, sensed, level, label, time };
    EcamProcedure {
        id: 260800005,
        title: "ENG 1 FIRE",
        items: vec![
            a("THR LEVER 1", true, 0, "IDLE", None),
            a("ENG 1 MASTER", true, 0, "OFF", None),
            a("ENG 1 FIRE P/B", true, 0, "PUSH", None),
            a("APU BLEED", true, 0, "OFF", None),
            EcamItem { name: "APU BLEED : DO NOT USE", kind: ItemKind::Plain, sensed: false, level: 0, label: "", time: None },
            a("ENG 1 AGENT 1", true, 0, "DISCH", Some(10)),
            a("ATC", false, 0, "NOTIFY", None),
            EcamItem { name: "STILL FIRE", kind: ItemKind::Condition, sensed: true, level: 0, label: "", time: Some(30) },
            a("ENG 1 AGENT 2", true, 1, "DISCH", None),
        ],
    }
}

fn source(id: u32) -> Option<EcamProcedure> {
    (id == 260800005).then(eng1_fire)
}

fn pm() -> PilotMonitoring {
    PilotMonitoring::new(Duties::default(), source)
}

fn run(pm: &mut PilotMonitoring, s: &Snapshot, cmds: &[PfCommand], seconds: f64) -> Vec<Action> {
    let mut out = pm.step(s, cmds, DT);
    let mut t = DT;
    while t < seconds - 1e-9 {
        out.extend(pm.step(s, &[], DT));
        t += DT;
    }
    out
}

fn says(out: &[Action]) -> Vec<&'static str> {
    out.iter().filter_map(|a| if let Action::Say(k) = a { Some(*k) } else { None }).collect()
}

fn parked() -> Snapshot {
    Snapshot { on_ground: true, alt_ft: 30., gear_handle_down: true, gear_down_locked: true, ..Default::default() }
}

fn ready_to_go(pm: &mut PilotMonitoring) -> (Snapshot, Vec<Action>) {
    let mut s = parked();
    let mut out = run(pm, &s, &[], 1.);
    s.beacon = true;
    out.extend(run(pm, &s, &[], 1.));
    s.engines_running = 4;
    out.extend(run(pm, &s, &[], 30.));
    s.gs_kt = 15.;
    out.extend(run(pm, &s, &[], 5.));
    s.gs_kt = 0.;
    s.landing_lights = true;
    out.extend(run(pm, &s, &[], 1.));
    (s, out)
}

fn take_off(pm: &mut PilotMonitoring, mut s: Snapshot) -> (Snapshot, Vec<Action>) {
    let mut out = Vec::new();
    s.tla_max = 35.;
    s.vr_kt = 150.;
    s.flap_lever = 2;
    for ias in (0..=160).step_by(5) {
        s.ias_kt = ias as f64;
        s.gs_kt = ias as f64;
        out.extend(run(pm, &s, &[], 0.5));
    }
    s.on_ground = false;
    s.fmgc_phase = 1;
    s.gear_handle_down = true;
    s.gear_down_locked = true;
    s.vs_fpm = 1500.;
    for ra in (0..=400).step_by(20) {
        s.ra_ft = ra as f64;
        s.alt_ft = 30. + ra as f64;
        out.extend(run(pm, &s, &[], 0.3));
    }
    (s, out)
}

#[test]
fn takeoff_callouts_come_once_in_order_after_the_ground_checklists() {
    let mut pm = pm();
    let (s, ground) = ready_to_go(&mut pm);
    let cls: Vec<u8> = ground.iter().filter_map(|a| if let Action::Checklist(c) = a { Some(*c) } else { None }).collect();
    assert_eq!(cls, [CL_BEFORE_START, CL_AFTER_START, CL_TAXI, CL_LINE_UP]);
    assert!(ground.contains(&Action::Flow(Flow::AfterStart)) && ground.contains(&Action::Flow(Flow::Taxi)));
    assert!(ground.contains(&Action::Pa(Pa::SeatsForTakeoff)));
    let (_, air) = take_off(&mut pm, s);
    assert_eq!(says(&air), ["thrust_set", "100kt", "rotate", "pos_climb"]);
    assert!(!air.iter().any(|a| matches!(a, Action::GearLever(_))), "no gear up unless commanded or the assist is on");
    assert_eq!(pm.phase(), Phase::Climb);
}

#[test]
fn the_auto_gear_up_assist_raises_the_gear_after_positive_climb() {
    let mut pm = PilotMonitoring::new(Duties { auto_gear_up: true, ..Duties::default() }, source);
    let (s, _) = ready_to_go(&mut pm);
    let (_, air) = take_off(&mut pm, s);
    assert!(air.contains(&Action::GearLever(Lever::Up)));
    assert_eq!(says(&air), ["thrust_set", "100kt", "rotate", "pos_climb", "gear_up"]);
}

#[test]
fn a_rejected_takeoff_stops_the_calls_and_rearms() {
    let mut pm = pm();
    let (mut s, _) = ready_to_go(&mut pm);
    s.tla_max = 35.;
    s.vr_kt = 150.;
    s.ias_kt = 110.;
    s.gs_kt = 110.;
    let out = run(&mut pm, &s, &[], 1.);
    assert_eq!(says(&out), ["thrust_set", "100kt"]);
    s.tla_max = 0.;
    s.ias_kt = 10.;
    s.gs_kt = 10.;
    run(&mut pm, &s, &[], 2.);
    let (_, again) = take_off(&mut pm, s);
    assert_eq!(says(&again), ["thrust_set", "100kt", "rotate", "pos_climb"]);
}

#[test]
fn gear_and_flaps_move_on_the_pfs_command_and_are_called_when_they_settle() {
    let mut pm = pm();
    let (s, _) = ready_to_go(&mut pm);
    let mut on_ground = s.clone();
    on_ground.gear_handle_down = true;
    assert!(run(&mut pm, &on_ground, &[PfCommand::GearUp], 0.1).is_empty(), "never gear up on the ground");
    let (mut s, _) = take_off(&mut pm, s);
    let out = run(&mut pm, &s, &[PfCommand::GearUp], 0.1);
    assert_eq!(out, [Action::GearLever(Lever::Up), Action::Say("gear_up")]);
    s.gear_handle_down = false;
    s.gear_down_locked = false;
    s.ias_kt = 200.;
    let out = run(&mut pm, &s, &[PfCommand::Flaps(1)], 0.1);
    assert_eq!(out, [Action::FlapLever(1)]);
    s.flap_lever = 1;
    assert_eq!(says(&run(&mut pm, &s, &[], 1.5)), ["flaps_1"]);
    s.vfe_kt = [0., 263., 220., 196., 182.];
    s.ias_kt = 240.;
    assert_eq!(run(&mut pm, &s, &[PfCommand::Flaps(2)], 0.1), [Action::Say("speed_check")], "no flaps above VFE");
    s.ias_kt = 210.;
    assert_eq!(run(&mut pm, &s, &[PfCommand::Flaps(2)], 0.1), [Action::FlapLever(2)]);
    let out = run(&mut pm, &s, &[PfCommand::GearDown], 0.1);
    assert_eq!(out, [Action::GearLever(Lever::Down)]);
    s.gear_handle_down = true;
    assert_eq!(says(&run(&mut pm, &s, &[], 0.2)), ["gear_down"]);
}

fn cruise(pm: &mut PilotMonitoring) -> Snapshot {
    let (s, _) = ready_to_go(pm);
    let (mut s, _) = take_off(pm, s);
    s.gear_handle_down = false;
    s.gear_down_locked = false;
    s.flap_lever = 0;
    s.fmgc_phase = 3;
    s.vs_fpm = 0.;
    s.alt_ft = 37_000.;
    s.ra_ft = 0.;
    s.ias_kt = 260.;
    run(pm, &s, &[], 3.);
    s
}

#[test]
fn approach_and_landing_calls_follow_the_flight_down_to_the_stand() {
    let mut pm = pm();
    let mut s = cruise(&mut pm);
    s.fmgc_phase = 4;
    s.vs_fpm = -2000.;
    s.trans_lvl_ft = 6000.;
    let mut out = Vec::new();
    for alt in (5000..=12_000).rev().step_by(500) {
        s.alt_ft = alt as f64;
        out.extend(run(&mut pm, &s, &[], 0.5));
    }
    assert!(says(&out).contains(&"transition_qnh"));
    assert!(out.contains(&Action::Checklist(CL_APPROACH)) && out.contains(&Action::Pa(Pa::PrepareForLanding)));
    assert!(out.contains(&Action::Flow(Flow::Descent10k)));

    s.fmgc_phase = 5;
    s.vs_fpm = -700.;
    s.vapp_kt = 140.;
    s.ias_kt = 140.;
    s.flap_lever = 4;
    s.gear_handle_down = true;
    s.gear_down_locked = true;
    s.tla_max = 20.;
    let mut out = Vec::new();
    for ra in (500..=2000).rev().step_by(50) {
        s.ra_ft = ra as f64;
        s.alt_ft = ra as f64;
        out.extend(run(&mut pm, &s, &[], 0.3));
    }
    assert!(out.contains(&Action::Checklist(CL_LANDING)) && out.contains(&Action::Pa(Pa::SeatsForLanding)));
    let calls = says(&out);
    assert!(calls.contains(&"stabilised") && !calls.contains(&"not_stabilised"));
    s.vs_fpm = -1300.;
    assert_eq!(says(&run(&mut pm, &s, &[], 1.)), ["sink_rate"], "said once while the deviation lasts");
    s.vs_fpm = -700.;
    run(&mut pm, &s, &[], 0.5);

    s.on_ground = true;
    s.ra_ft = 0.;
    s.alt_ft = 30.;
    s.spoilers_extended = true;
    s.tla_max = -5.;
    s.tla_min = -5.;
    s.reversers_deployed = true;
    s.decel_g = 0.2;
    let mut out = Vec::new();
    for kt in (35..=135).rev().step_by(5) {
        s.ias_kt = kt as f64;
        s.gs_kt = kt as f64;
        out.extend(run(&mut pm, &s, &[], 0.3));
    }
    assert_eq!(says(&out), ["spoilers", "reverse_green", "decel", "70kt"]);
    s.tla_max = 0.;
    s.tla_min = 0.;
    s.reversers_deployed = false;
    s.gs_kt = 15.;
    let out = run(&mut pm, &s, &[], 1.);
    assert!(out.contains(&Action::Flow(Flow::AfterLanding)));
    assert_eq!(pm.phase(), Phase::TaxiIn);
    s.gs_kt = 0.;
    s.engines_running = 0;
    let out = run(&mut pm, &s, &[], 1.);
    assert!(out.contains(&Action::Checklist(CL_PARKING)) && out.contains(&Action::Pa(Pa::DisarmDoors)));
    assert_eq!(pm.phase(), Phase::Parked);
}

#[test]
fn the_engine_fire_ecam_actions_are_read_confirmed_timed_and_done() {
    let mut pm = pm();
    let mut s = cruise(&mut pm);
    s.ecam_warning = Some(260800005);
    s.ecam = Some(EcamLive { procedure: 260800005, done: vec![false; 9] });
    let out = run(&mut pm, &s, &[], 0.1);
    assert_eq!(out, [Action::ReadEcamTitle { procedure: 260800005 }]);
    assert!(run(&mut pm, &s, &[], 2.).is_empty(), "the PM waits for the PF's ECAM ACTIONS order");

    let out = run(&mut pm, &s, &[PfCommand::EcamActions], 0.1);
    assert_eq!(out, [Action::ReadEcam { procedure: 260800005, item: 0 }], "thrust lever is the PF's");
    assert!(run(&mut pm, &s, &[], 2.).is_empty());
    let sim = |s: &mut Snapshot, i: usize| s.ecam.as_mut().unwrap().done[i] = true;
    sim(&mut s, 0);
    let out = run(&mut pm, &s, &[], 0.1);
    assert_eq!(out, [Action::ReadEcam { procedure: 260800005, item: 1 }, Action::Say("confirm")]);
    assert!(run(&mut pm, &s, &[], 3.).is_empty(), "no master switch without confirmation");
    let out = run(&mut pm, &s, &[PfCommand::Confirm], 0.1);
    assert_eq!(out, [Action::Switch(Switch::EngMaster { engine: 1, on: false })]);
    sim(&mut s, 1);
    let out = run(&mut pm, &s, &[], 0.1);
    assert_eq!(out, [Action::ReadEcam { procedure: 260800005, item: 2 }, Action::Say("confirm")]);
    let out = run(&mut pm, &s, &[PfCommand::Confirm], 0.1);
    assert_eq!(out, [Action::Switch(Switch::EngFirePb { engine: 1 })]);
    sim(&mut s, 2);
    let out = run(&mut pm, &s, &[], 0.1);
    assert_eq!(out, [Action::ReadEcam { procedure: 260800005, item: 3 }, Action::Switch(Switch::ApuBleed { on: false })]);
    sim(&mut s, 3);
    let out = run(&mut pm, &s, &[], 0.1);
    assert_eq!(out, [Action::ReadEcam { procedure: 260800005, item: 4 }]);
    assert!(run(&mut pm, &s, &[], 9.).is_empty(), "agent 1 waits its 10 s");
    let out = run(&mut pm, &s, &[], 1.2);
    assert_eq!(out, [Action::ReadEcam { procedure: 260800005, item: 5 }, Action::Switch(Switch::EngAgent { engine: 1, agent: 1 })]);
    sim(&mut s, 5);
    let out = run(&mut pm, &s, &[], 0.1);
    assert_eq!(out, [Action::ReadEcam { procedure: 260800005, item: 6 }, Action::AtcNotify, Action::TickEcam { procedure: 260800005, item: 6 }]);
    sim(&mut s, 7);
    assert!(run(&mut pm, &s, &[], 29.).is_empty(), "still fire is checked 30 s after agent 1");
    let out = run(&mut pm, &s, &[], 1.2);
    assert_eq!(
        out,
        [
            Action::ReadEcam { procedure: 260800005, item: 7 },
            Action::ReadEcam { procedure: 260800005, item: 8 },
            Action::Switch(Switch::EngAgent { engine: 1, agent: 2 }),
        ]
    );
    sim(&mut s, 8);
    assert_eq!(says(&run(&mut pm, &s, &[], 0.1)), ["ecam_actions_complete"]);
    assert_eq!(pm.ecam_running(), None);
}

#[test]
fn agent_two_is_skipped_when_the_fire_is_out() {
    let mut pm = pm();
    let mut s = cruise(&mut pm);
    s.ecam_warning = Some(260800005);
    let mut done = vec![true; 9];
    done[5] = false;
    done[7] = false;
    done[8] = false;
    s.ecam = Some(EcamLive { procedure: 260800005, done });
    let mut out = run(&mut pm, &s, &[PfCommand::EcamActions], 12.);
    s.ecam.as_mut().unwrap().done[5] = true;
    out.extend(run(&mut pm, &s, &[], 45.));
    assert!(out.contains(&Action::Switch(Switch::EngAgent { engine: 1, agent: 1 })));
    assert!(!out.contains(&Action::Switch(Switch::EngAgent { engine: 1, agent: 2 })));
    assert!(says(&out).contains(&"ecam_actions_complete"));
}

#[test]
fn an_unknown_procedure_is_reported_not_guessed() {
    let mut pm = pm();
    let mut s = cruise(&mut pm);
    s.ecam_warning = Some(290800001);
    s.ecam = Some(EcamLive { procedure: 290800001, done: vec![] });
    let out = run(&mut pm, &s, &[PfCommand::EcamActions], 0.1);
    assert!(says(&out).contains(&"unable"));
}

#[test]
fn the_pm_preselects_departure_and_changes_frequency_on_handover() {
    let mut pm = pm();
    let (mut s, _) = ready_to_go(&mut pm);
    s.frequencies = vec![
        Station { kind: StationKind::Ground, khz: 121_700 },
        Station { kind: StationKind::Tower, khz: 118_500 },
        Station { kind: StationKind::Departure, khz: 119_725 },
        Station { kind: StationKind::Centre, khz: 133_175 },
    ];
    s.com1_active_khz = 121_700;
    let out = run(&mut pm, &s, &[], 0.2);
    assert_eq!(out, [Action::TuneStandby { com: 1, khz: 118_500 }], "tower ready in standby while taxiing");
    s.com1_standby_khz = 118_500;
    let out = run(&mut pm, &s, &[PfCommand::Contact(StationKind::Tower)], 0.1);
    assert_eq!(out, [Action::ReadFrequency(118_500), Action::SwapCom(1)]);
    s.com1_active_khz = 118_500;
    s.com1_standby_khz = 121_700;
    let (mut s, out) = take_off(&mut pm, s);
    assert!(out.contains(&Action::TuneStandby { com: 1, khz: 119_725 }));
    s.com1_standby_khz = 119_725;
    s.handover = Some(Handover { seq: 1, kind: StationKind::Departure, khz: 119_725 });
    let out: Vec<Action> = run(&mut pm, &s, &[], 0.5).into_iter().filter(|a| !matches!(a, Action::Say(_))).collect();
    assert_eq!(out[..2], [Action::ReadFrequency(119_725), Action::SwapCom(1)]);
    s.com1_active_khz = 119_725;
    s.com1_standby_khz = 118_500;
    s.handover = Some(Handover { seq: 2, kind: StationKind::Centre, khz: 133_175 });
    let out: Vec<Action> = run(&mut pm, &s, &[], 0.5).into_iter().filter(|a| !matches!(a, Action::Say(_))).collect();
    assert_eq!(out, [Action::ReadFrequency(133_175), Action::TuneStandby { com: 1, khz: 133_175 }, Action::SwapCom(1)]);
    s.handover = Some(Handover { seq: 3, kind: StationKind::Centre, khz: 99_000 });
    assert_eq!(says(&run(&mut pm, &s, &[], 0.1)), ["unable"]);
}

#[test]
fn approach_data_goes_into_the_mfd_once_near_the_destination() {
    let mut pm = pm();
    let mut s = cruise(&mut pm);
    let data = ApproachData { qnh_hpa: 1009., temp_c: 14., wind_dir_deg: 250., wind_kt: 12., baro_minimum_ft: Some(450.), radio_minimum_ft: None };
    s.approach_data = Some(data);
    s.dest_dist_nm = Some(400.);
    assert!(run(&mut pm, &s, &[], 1.).is_empty());
    s.dest_dist_nm = Some(180.);
    let out = run(&mut pm, &s, &[], 5.);
    assert_eq!(out, [Action::Mfd(MfdEntry::Approach(data)), Action::Say("approach_data_set")]);
}

#[test]
fn takeoff_data_is_entered_before_departure() {
    let mut pm = pm();
    let mut s = parked();
    let data = TakeoffData { flaps: 2, v1_kt: 140., vr_kt: 148., v2_kt: 155., flex_temp_c: Some(48), ths_deg: 1.2 };
    s.takeoff_data = Some(data);
    let out = run(&mut pm, &s, &[], 1.);
    assert_eq!(out, [Action::Mfd(MfdEntry::Takeoff(data)), Action::Say("takeoff_data_set")]);
}

#[test]
fn duties_the_pilot_keeps_produce_nothing_but_the_state_still_follows() {
    let mut pm = PilotMonitoring::new(Duties { callouts: false, checklists: false, cabin: false, flows: false, radios: false, mfd: false, ecam: false, ground: false, auto_gear_up: false }, source);
    let (s, ground) = ready_to_go(&mut pm);
    assert!(ground.is_empty());
    let (_, air) = take_off(&mut pm, s);
    assert!(air.is_empty());
    pm.duties.callouts = true;
    assert_eq!(pm.phase(), Phase::Climb);
}

#[test]
fn ground_crew_requests_go_to_the_interphone() {
    let mut pm = pm();
    let s = parked();
    let out = run(&mut pm, &s, &[PfCommand::RequestPush, PfCommand::RequestStart], 0.1);
    assert_eq!(out, [Action::Interphone(Interphone::Push), Action::Interphone(Interphone::Start)]);
    assert_eq!(Interphone::Disconnect.request(), "disconnect");
}

#[test]
fn ecam_items_map_to_switches_and_voice_keys() {
    let p = eng1_fire();
    assert_eq!(ecam::doer(&p.items[0]), Doer::Pf);
    assert_eq!(ecam::doer(&p.items[2]), Doer::Pm(Switch::EngFirePb { engine: 1 }));
    assert_eq!(ecam::doer(&p.items[6]), Doer::Atc);
    assert!(needs_confirmation(Switch::EngMaster { engine: 3, on: false }));
    assert!(!needs_confirmation(Switch::EngAgent { engine: 3, agent: 1 }));
    assert_eq!(ecam_voice_key(&p.items[2]), "e_eng_1_fire_p_b_push");
    assert_eq!(ecam_voice_key(&p.items[4]), "e_apu_bleed_do_not_use");
    let keys_ok = |k: &str| k.len() <= 64 && k.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    assert!(p.items.iter().all(|i| keys_ok(&ecam_voice_key(i))));
}

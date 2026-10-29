use fws_ecl::ecl::*;
use fws_ecl::mirror::EclMirror;
use fws_ecl::NORMAL_PROCEDURES;

fn press(ecl: &mut Ecl, s: &SensedInputs, f: impl Fn(&mut EcpPulses)) {
    let mut p = EcpPulses::default();
    f(&mut p);
    ecl.update(&CycleInputs { sensed: *s, pulses: p, ..CycleInputs::default() });
    ecl.update(&CycleInputs { sensed: *s, ..CycleInputs::default() });
}

#[test]
fn sensed_vectors_match_item_counts_and_flags() {
    let s = SensedInputs::default();
    for p in NORMAL_PROCEDURES {
        let v = sensed_items(p.id, &s, RudderTrimSensing::Absolute);
        let n = p.items.len().max(1);
        assert_eq!(v.len(), n, "{}", p.title);
        for (k, it) in p.items.iter().enumerate() {
            assert_eq!(v[k].is_some(), it.sensed, "{} {}", p.title, it.name);
            assert_eq!(p.checks[k].is_some(), it.sensed, "{} {}", p.title, it.name);
        }
    }
}

#[test]
fn sensed_conditions_follow_fbw() {
    let mut s = SensedInputs { rudder_trim_deg: -2.0, emer_exit_light_position: 1, ..Default::default() };
    assert_eq!(sensed_items(1000003, &s, RudderTrimSensing::SignedAsFbw)[2], Some(true));
    assert_eq!(sensed_items(1000003, &s, RudderTrimSensing::Absolute)[2], Some(false));
    s.flaps_handle = 4;
    assert_eq!(sensed_items(1000004, &s, RudderTrimSensing::Absolute)[6], Some(false));
    assert_eq!(sensed_items(1000012, &s, RudderTrimSensing::Absolute)[4], Some(true));
    s.landing_conf3 = true;
    assert_eq!(sensed_items(1000012, &s, RudderTrimSensing::Absolute)[4], Some(false));
    s.flaps_handle = 3;
    assert_eq!(sensed_items(1000012, &s, RudderTrimSensing::Absolute)[4], Some(true));
    assert_eq!(sensed_items(1000004, &s, RudderTrimSensing::Absolute)[6], Some(true));
    s.auto_brake_mode = 6;
    assert_eq!(sensed_items(1000004, &s, RudderTrimSensing::Absolute)[7], Some(true));
    assert_eq!(sensed_items(1000013, &s, RudderTrimSensing::Absolute)[1], Some(true));
    s.engine_master[2] = true;
    assert_eq!(sensed_items(1000013, &s, RudderTrimSensing::Absolute)[1], Some(false));
    assert_eq!(sensed_items(1000014, &s, RudderTrimSensing::Absolute)[1], Some(false));
    s.emer_exit_light_position = 2;
    assert_eq!(sensed_items(1000014, &s, RudderTrimSensing::Absolute)[1], Some(true));
}

#[test]
fn walk_cockpit_preparation_complete_and_reset() {
    let mut ecl = Ecl::new();
    let mut s = SensedInputs::default();
    press(&mut ecl, &s, |p| p.cl = true);
    assert!(ecl.shown);
    assert_eq!((ecl.checklist_id, ecl.selected_line), (0, 0));
    press(&mut ecl, &s, |p| p.check = true);
    assert_eq!((ecl.checklist_id, ecl.selected_line), (1000001, 0));
    press(&mut ecl, &s, |p| p.check = true);
    press(&mut ecl, &s, |p| p.check = true);
    assert_eq!(ecl.selected_line, 2);
    s.seat_belt_switch = true;
    press(&mut ecl, &s, |_| {});
    assert_eq!(ecl.state(1000001).unwrap().items_checked, vec![true, true, true, false]);
    assert_eq!(ecl.selected_line, 3);
    press(&mut ecl, &s, |p| p.check = true);
    assert_eq!(ecl.selected_line, SPECIAL_INDEX_NORMAL_CL_COMPLETE);
    press(&mut ecl, &s, |p| p.check = true);
    assert!(ecl.state(1000001).unwrap().procedure_completed);
    assert!(!ecl.shown);
    assert_eq!(ecl.state(1000004).unwrap().items_checked[4], true);

    press(&mut ecl, &s, |p| p.cl = true);
    assert_eq!((ecl.checklist_id, ecl.selected_line), (0, 1));
    press(&mut ecl, &s, |p| p.up = true);
    press(&mut ecl, &s, |p| p.check = true);
    assert_eq!(ecl.selected_line, SPECIAL_INDEX_NORMAL_RESET);
    press(&mut ecl, &s, |p| p.check = true);
    let st = ecl.state(1000001).unwrap();
    assert!(!st.procedure_completed);
    assert_eq!(st.items_checked, vec![false, false, true, false]);
    assert_eq!(ecl.selected_line, 0);

    let m = EclMirror::from_ecl(&ecl, &s);
    assert_eq!(m.open_efb_page_id.as_deref(), Some("n:0"));
    assert_eq!(m.efb_ticks("n:0"), vec![2]);
    assert_eq!(m.checklists.len(), 14);
    assert!(!m.checklists[6].in_menu);
}

#[test]
fn sensed_items_only_latch_while_shown_and_takeoff_power_resets_after_departure_change() {
    let mut ecl = Ecl::new();
    let s = SensedInputs { beacon_on: true, ..Default::default() };
    press(&mut ecl, &s, |_| {});
    assert!(!ecl.state(1000002).unwrap().items_checked[2]);
    press(&mut ecl, &s, |p| p.cl = true);
    press(&mut ecl, &s, |_| {});
    assert!(ecl.state(1000002).unwrap().items_checked[2]);

    let idx = NORMAL_PROCEDURES.iter().position(|p| p.id == 1000010).unwrap();
    ecl.states[idx].items_checked[0] = true;
    ecl.states[idx].procedure_completed = true;
    ecl.update(&CycleInputs { sensed: s, takeoff_power: true, flight_phase: 3, ..CycleInputs::default() });
    assert!(!ecl.state(1000010).unwrap().procedure_completed);
    assert!(!ecl.state(1000010).unwrap().items_checked[0]);
    assert!(ecl.state(1000006).unwrap().procedure_completed);
}

#[test]
fn deferred_page_activate_tick_complete_recall() {
    let mut ecl = Ecl::new();
    let s = SensedInputs::default();
    ecl.core_add_deferred(221700001, &[true, false], Some(&[true, true]));
    press(&mut ecl, &s, |p| p.cl = true);
    for _ in 0..6 {
        press(&mut ecl, &s, |p| p.down = true);
    }
    assert_eq!(ecl.selected_line, 7);
    press(&mut ecl, &s, |p| p.check = true);
    assert_eq!((ecl.checklist_id, ecl.selected_line), (1000008, -2));
    press(&mut ecl, &s, |p| p.check = true);
    assert!(ecl.deferred_procs[0].state.procedure_activated);
    assert_eq!(ecl.selected_line, 0);
    press(&mut ecl, &s, |p| p.down = true);
    press(&mut ecl, &s, |p| p.check = true);
    assert_eq!(ecl.selected_line, -3);
    press(&mut ecl, &s, |p| p.check = true);
    assert_eq!(ecl.selected_line, -4);
    assert!(ecl.deferred_procs[0].state.procedure_completed && ecl.overview.items_checked[7]);
    press(&mut ecl, &s, |p| p.down = true);
    assert_eq!(ecl.selected_line, SPECIAL_INDEX_DEFERRED_PAGE_CLEAR);
    press(&mut ecl, &s, |p| p.check = true);
    assert_eq!(ecl.checklist_id, 0);
    ecl.core_clear_deferred();
    press(&mut ecl, &s, |p| p.cl = true);
    press(&mut ecl, &s, |p| p.cl = true);
    assert!(!ecl.overview.items_to_show[7]);
}

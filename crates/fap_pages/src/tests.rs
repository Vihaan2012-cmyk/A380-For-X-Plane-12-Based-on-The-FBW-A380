use super::*;
use cids::maint::TestResult;
use cids::view::CallLine;
use cids::{Caution, TestKind};

fn view(page: Page, body: PageBody) -> FapView {
    FapView {
        mode: ViewMode::Live,
        boot_remaining_s: 0.,
        page,
        title: page.title(),
        cautions: vec![Caution { level: Level::Red, text: "SMOKE AFT".into() }],
        message: None,
        keys: vec![HardKey { id: "callreset", label: "CALL RESET", lit: true, alert: false, guard: false }],
        switches: vec![SwitchView { id: "emer", label: "EMER LIGHT", up: false }],
        locked: false,
        body,
    }
}

fn json(page: Page, body: PageBody) -> String {
    fap_root(&view(page, body), &FapSim::default(), &Ui::default()).to_json()
}

fn count(n: &Node, cls: &str) -> usize {
    let me = n.cls.iter().any(|c| c == cls) as usize;
    me + n.kids.iter().map(|k| if let mfd_tree::Kid::Node(c) = k { count(c, cls) } else { 0 }).sum::<usize>()
}

#[test]
fn shell_has_header_cautions_nav_and_hard_keys() {
    let v = view(Page::Status, PageBody::Status(Vec::new()));
    let n = fap_root(&v, &FapSim::default(), &Ui::default());
    let j = n.to_json();
    assert!(j.contains(r#""text":"CABIN STATUS""#));
    assert!(j.contains(r#"{"tag":"span","cls":["red"],"kids":[{"text":"SMOKE AFT"}]}"#));
    assert_eq!(count(&n, "nav"), 1);
    assert_eq!(count(&n, "arrow"), 2);
    assert_eq!(count(&n, "hk"), 1);
    assert!(j.contains(r#"{"text":"CALL"},{"tag":"br","kids":[]},{"text":"RESET"}"#));
    assert_eq!(count(&n, "tsw"), 1);
}

#[test]
fn full_nav_lists_fourteen_pages_and_marks_current() {
    let n = fap_root(&view(Page::Evac, PageBody::Fault), &FapSim::default(), &Ui::default());
    let nav = match &n.kids[0] {
        mfd_tree::Kid::Node(scr) => scr.kids.iter().find_map(|k| match k {
            mfd_tree::Kid::Node(c) if c.cls.iter().any(|x| x == "nav") => Some(c.clone()),
            _ => None,
        }),
        _ => None,
    }
    .unwrap();
    assert_eq!(nav.kids.len(), 16);
    assert_eq!(count(&nav, "on"), 1);
    let legacy = Ui { nav: LEGACY_NAV.to_vec(), ..Ui::default() };
    assert_eq!(count(&fap_root(&view(Page::Status, PageBody::Status(vec![])), &FapSim::default(), &legacy), "key") - 1, 13);
}

#[test]
fn dark_booting_and_screen_off_modes() {
    let mut v = view(Page::Status, PageBody::Fault);
    v.mode = ViewMode::Dark;
    let j = fap_root(&v, &FapSim::default(), &Ui::default()).to_json();
    assert_eq!(j, r#"{"tag":"div","id":"FAP_ROOT","kids":[{"tag":"div","id":"SCR","cls":["dark"],"kids":[]},{"tag":"div","id":"HK","kids":[]}]}"#);
    v.mode = ViewMode::Booting;
    v.boot_remaining_s = 7.;
    assert!(!fap_root(&v, &FapSim::default(), &Ui::default()).to_json().contains("INITIALIZING"));
    v.boot_remaining_s = 3.;
    assert!(fap_root(&v, &FapSim::default(), &Ui::default()).to_json().contains("FAP INITIALIZING..."));
    v.mode = ViewMode::ScreenOff;
    let n = fap_root(&v, &FapSim::default(), &Ui::default());
    assert_eq!(count(&n, "hk"), 1);
    assert!(!n.to_json().contains("CABIN STATUS"));
}

#[test]
fn message_and_lock_lines() {
    let mut v = view(Page::Status, PageBody::Status(vec![]));
    v.message = Some("DOOR OPERATION INHIBITED".into());
    v.locked = true;
    let n = fap_root(&v, &FapSim::default(), &Ui::default());
    assert_eq!(count(&n, "msg"), 2);
}

#[test]
fn fault_body_shows_cids_fault() {
    assert!(json(Page::Lights, PageBody::Fault).contains("CIDS FAULT"));
}

#[test]
fn calls_page_lists_newest_first_with_age() {
    let log = vec![
        CallLine { text: "PURSER CALL".into(), age_s: 125., answered: true },
        CallLine { text: "EMERGENCY CALL".into(), age_s: 4., answered: false },
    ];
    let j = json(Page::Calls, PageBody::Calls { log, pending: 1, emer: true });
    let e = j.find("EMERGENCY CALL\"").unwrap();
    let p = j.find("PURSER CALL").unwrap();
    assert!(e < p);
    assert!(j.contains("00:04 AGO OPEN"));
    assert!(j.contains("02:05 AGO ANSWERED"));
    assert!(j.contains(r#""text":"ACTIVE""#));
    assert!(json(Page::Calls, PageBody::Calls { log: vec![], pending: 0, emer: false }).contains("NO CALLS"));
}

#[test]
fn calls_page_caps_rows() {
    let log = (0..30).map(|i| CallLine { text: format!("ATTENDANT CALL {i}"), age_s: i as f64, answered: false }).collect();
    let n = fap_root(&view(Page::Calls, PageBody::Calls { log, pending: 30, emer: false }), &FapSim::default(), &Ui::default());
    assert_eq!(count(&n, "s"), extra::CALL_ROWS + 2);
}

#[test]
fn evac_page_states() {
    let b = |active, cockpit, cabin, capt_only, guard| PageBody::Evac { active, cockpit_cmd: cockpit, cabin_cmd: cabin, capt_only, guard_open: guard, horn_cockpit: active };
    let j = json(Page::Evac, b(false, false, false, true, true));
    assert!(j.contains("PRESS EVAC CMD AGAIN TO CONFIRM"));
    assert!(j.contains(r#""text":"CAPT""#));
    let j = json(Page::Evac, b(true, false, true, false, false));
    assert!(j.contains(r#""text":"ALERT""#));
    assert!(j.contains("CAPT & PURS"));
    assert!(j.contains(r#""cls":["key","red"]"#));
    let n = fap_root(&view(Page::Evac, b(true, true, false, false, false)), &FapSim::default(), &Ui::default());
    assert_eq!(count(&n, "off"), 1);
}

#[test]
fn maintenance_page_results() {
    let b = |result, allowed| PageBody::Maintenance { result, test_allowed: allowed, lamp_test: false };
    let n = fap_root(&view(Page::Maintenance, b(TestResult::None, false)), &FapSim::default(), &Ui::default());
    assert_eq!(count(&n, "off"), 3);
    assert!(n.to_json().contains("NOT AVAILABLE"));
    let j = json(Page::Maintenance, b(TestResult::Running(TestKind::Pa), true));
    assert!(j.contains("IN PROGRESS"));
    assert!(j.contains(r#""cls":["key","on"]"#));
    assert!(json(Page::Maintenance, b(TestResult::Fail(TestKind::Directors, "DIRECTOR 2".into()), true)).contains("FAILED: DIRECTOR 2"));
    assert!(json(Page::Maintenance, b(TestResult::Pass(TestKind::Lamp), true)).contains("PASSED"));
}

#[test]
fn sim_reads_doors_like_the_js() {
    let s = FapSim::read(|n| match n {
        "A:INTERACTIVE POINT OPEN:0" => Some(0.),
        "A:INTERACTIVE POINT OPEN:1" => Some(99.),
        "A:INTERACTIVE POINT OPEN:2" => Some(40.),
        "A:INTERACTIVE POINT OPEN:3" => Some(0.),
        "A:INTERACTIVE POINT OPEN:16" => Some(0.),
        "L:ANIM_DOOR_M1L_DISARMED" => Some(0.),
        "L:A32NX_ELEC_AC_3_BUS_IS_POWERED" => Some(1.),
        _ => None,
    });
    assert!(s.powered);
    assert_eq!(&s.doors[..4], &[DoorMark::Armed, DoorMark::Open, DoorMark::Moving, DoorMark::Disarmed]);
    assert_eq!(s.doors[16], DoorMark::Disarmed);
    assert_eq!(s.doors[17], DoorMark::Na);
}

#[test]
fn plan_svg_has_silhouette() {
    let n = fap_root(&view(Page::Status, PageBody::Status(vec![])), &FapSim::default(), &Ui::default());
    let j = n.to_json();
    assert!(j.contains(r#""t":"path""#));
    assert_eq!(j.matches(r#""t":"circle""#).count(), 18);
}

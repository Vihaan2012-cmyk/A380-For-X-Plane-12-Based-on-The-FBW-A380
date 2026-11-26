use std::collections::BTreeMap;

use fms::navdb::MemoryNavDb;
use fms::service::{Command as Plan, PlanRef};
use fms_host::{Envelope, FmsCore, HostCommand, Snapshot};

use super::*;
use crate::page::Page;

fn db() -> MemoryNavDb {
    MemoryNavDb::from_fixture_json(include_str!("../../fms/tests/fixtures/navdb.json")).unwrap()
}

fn snapshot() -> Snapshot {
    let db = db();
    let mut core = FmsCore::new();
    let cmd = HostCommand::Plan(Plan::NewCityPair { from: "EGLL".into(), to: "KJFK".into(), alternate: None, plan: PlanRef::Active });
    core.command(&db, Envelope { id: 1, command: cmd });
    core.snapshot()
}

fn acts(m: &Mfd) -> Vec<String> {
    let mut v = Vec::new();
    tree::acts(&m.tree, &mut v);
    v
}

#[test]
fn every_page_builds_from_the_host_snapshot() {
    let s = snapshot();
    let src = Sources::from_snapshot(&s, sources::NO_VARS, &[]);
    let uris = [
        "fms/active/f-pln", "fms/active/init", "fms/active/fuel-load", "fms/active/perf", "fms/active/wind", "fms/active/f-pln-vert-rev", "fms/active/f-pln-hold",
        "fms/active/f-pln-direct-to", "fms/active/f-pln-airways", "fms/active/f-pln-departure", "fms/active/f-pln-arrival", "fms/position/monitor", "fms/sec/index/1",
        "fms/data/status", "atccom/connect", "surv/controls", "fcubkup/afs", "fcubkup/efis",
    ];
    for u in uris {
        let mut m = Mfd::default();
        m.capt.navigate(u);
        assert_ne!(m.capt.page(), Page::NotFound, "{u}");
        m.build(&src);
        assert!(acts(&m).iter().any(|a| a.starts_with("c|")), "{u}");
        assert!(!m.flat.is_empty());
    }
}

#[test]
fn fpln_lines_come_from_the_plan() {
    let s = snapshot();
    let src = Sources::from_snapshot(&s, sources::NO_VARS, &[]);
    let mut ui = UiState::new(true);
    ui.navigate("fms/active/f-pln");
    let m = model::fpln(&ui, &src);
    assert!(m.lines.iter().any(|l| l.ident.starts_with("EGLL")));
    assert!(m.lines.iter().any(|l| l.label == "END OF F-PLN"));
    assert!(m.dest.loaded);
    assert_eq!(m.dest.button_label, "KJFK");
}

#[test]
fn header_menu_navigates_and_init_field_sends_a_command() {
    let s = snapshot();
    let src = Sources::from_snapshot(&s, sources::NO_VARS, &[]);
    let mut m = Mfd::default();
    m.build(&src);
    assert!(m.act(&src, "c|psel:0").is_empty());
    assert_eq!(m.capt.page_selector, Some(0));
    m.act(&src, "c|item:CAPT_MFD_pageSelectorActive_2");
    assert_eq!(m.capt.page(), Page::FuelLoad);
    m.act(&src, "c|field:ZFW");
    assert!(m.keyboard());
    for c in "300.5".chars() {
        m.key(&src, None, Key::Char(c));
    }
    m.build(&src);
    assert!(tree::text_of(&m.tree).contains("300.5"));
    let (used, out) = m.key(&src, None, Key::Enter);
    assert!(used && !m.keyboard());
    match &out[..] {
        [Out::Fms(HostCommand::Perf(fms_data::Command::Enter { plan: 0, entry: fms_data::Entry::ZeroFuelWeightKg(Some(kg)) }))] => assert!((kg - 300_500.).abs() < 1e-6),
        o => panic!("{o:?}"),
    }
}

#[test]
fn escape_releases_the_keyboard_and_city_pair_is_entered() {
    let empty = Snapshot { plans: vec![None; 6], perf: BTreeMap::new(), phase: 0, ..Default::default() };
    let src = Sources::from_snapshot(&empty, sources::NO_VARS, &[]);
    let mut m = Mfd::default();
    m.act(&src, "c|field:FROM");
    m.key(&src, None, Key::Char('x'));
    assert_eq!(m.capt.scratchpad, "X");
    m.key(&src, None, Key::Escape);
    assert!(!m.keyboard());
    m.act(&src, "c|field:FROM");
    for c in "EGLL/KJFK".chars() {
        m.key(&src, None, Key::Char(c));
    }
    let (_, out) = m.key(&src, None, Key::Enter);
    assert!(matches!(&out[..], [Out::Fms(HostCommand::Plan(Plan::NewCityPair { from, to, .. }))] if from == "EGLL" && to == "KJFK"));
}

#[test]
fn hit_test_finds_the_innermost_act() {
    let mut body = mfd_tree::Node::new("body").child(mfd_tree::Node::new("div").attr(tree::ACT, "c|btn:A").child(mfd_tree::Node::new("span")));
    tree::tag(&mut body, "c");
    let flat = hit::flatten(&body);
    let boxes = [[0., 0., 100., 100.], [10., 10., 50., 50.], [12., 12., 5., 5.]];
    assert_eq!(hit::hit(&flat, &boxes, 13., 13.).as_deref(), Some("c|btn:A"));
    assert_eq!(hit::hit(&flat, &boxes, 90., 90.), None);
}

#[test]
fn revisions_menu_opens_on_a_leg_and_deletes() {
    let s = snapshot();
    let src = Sources::from_snapshot(&s, sources::NO_VARS, &[]);
    let mut m = Mfd::default();
    m.capt.navigate("fms/active/f-pln");
    m.build(&src);
    let a = acts(&m);
    let ident = a.iter().find(|x| x.starts_with("c|fpln:ident:")).cloned().unwrap();
    m.act(&src, &ident);
    assert_eq!(m.capt.open.as_deref(), Some("revisions"));
}

#[test]
fn page_selector_click_changes_the_tree() {
    let empty = Snapshot { plans: vec![None; 6], perf: BTreeMap::new(), phase: 0, ..Default::default() };
    let src = Sources::from_snapshot(&empty, sources::NO_VARS, &[]);
    let mut m = Mfd::default();
    let before = m.build(&src).clone();
    m.act(&src, "c|psel:0");
    let after = m.build(&src).clone();
    assert_ne!(before, after);
}

#[test]
fn init_takes_a_flight_number_and_city_pair_on_a_fresh_fms() {
    let db = db();
    let mut core = FmsCore::new();
    core.tick(&db, 0., &Default::default());
    let s = core.snapshot();
    let src = Sources::from_snapshot(&s, sources::NO_VARS, &[]);
    let mut m = Mfd::default();
    m.capt.navigate("fms/active/init");
    m.build(&src);
    let all = acts(&m);
    let mut sent = Vec::new();
    for (field, text) in [("FLT NBR", "EK1"), ("FROM", "EGLL"), ("TO", "KJFK")] {
        let act = all.iter().find(|a| a.ends_with(&format!("c|field:{field}"))).cloned().unwrap();
        sent.extend(m.act(&src, &act));
        for c in text.chars() {
            sent.extend(m.key(&src, Some(&db), Key::Char(c)).1);
        }
        sent.extend(m.key(&src, Some(&db), Key::Enter).1);
        m.build(&src);
    }
    let text = format!("{sent:?}");
    assert!(text.contains("SetFlightNumber { plan: 0, number: \"EK1\" }"), "{text}");
    assert!(text.contains("NewCityPair { from: \"EGLL\", to: \"KJFK\""), "{text}");
}

#[test]
#[ignore]
fn dump_perf_document() {
    let html_ui = std::env::var("MFD_HTML_UI").expect("MFD_HTML_UI");
    let native = mfd_native::Native::load(&html_ui, 1.).unwrap();
    let s = snapshot();
    let src = Sources::from_snapshot(&s, sources::NO_VARS, &[]);
    let mut m = Mfd::default();
    m.build(&src);
    for (name, uri) in [("perf", "fms/active/perf"), ("fpln", "fms/active/f-pln")] {
        m.capt.navigate(uri);
        m.fo.navigate(uri);
        let tree = m.build(&src).clone();
        let t0 = std::time::Instant::now();
        let html = native.document(&tree);
        eprintln!("{name}: {} bytes, document {:.2} ms", html.len(), t0.elapsed().as_secs_f64() * 1000.0);
        std::fs::write(format!("{}/{name}.dom.html", std::env::var("MFD_DUMP_DIR").unwrap_or(".".into())), html).unwrap();
    }
    m.capt.navigate("fms/active/perf");
    m.fo.navigate("fms/active/perf");
    m.build(&src);
    m.act(&src, "c|psel:0");
    let tree = m.build(&src).clone();
    std::fs::write(format!("{}/perf_menu.dom.html", std::env::var("MFD_DUMP_DIR").unwrap_or(".".into())), native.document(&tree)).unwrap();
    m.act(&src, "c|psel:0");
    for (name, tab) in [("perf_clb", "c|tab:1"), ("perf_des", "c|tab:3")] {
        m.build(&src);
        m.act(&src, tab);
        let tree = m.build(&src).clone();
        std::fs::write(format!("{}/{name}.dom.html", std::env::var("MFD_DUMP_DIR").unwrap_or(".".into())), native.document(&tree)).unwrap();
    }
}

#[test]
fn cpny_fpln_request_then_insert() {
    let mut s = snapshot();
    s.plans[0] = None;
    let state = std::cell::Cell::new(0.);
    let vars = |n: &str| if n == model::CPNY_FPLN_STATE { state.get() } else { 0. };
    let mut m = Mfd::default();
    m.capt.navigate("fms/active/init");
    let src = Sources::from_snapshot(&s, &vars, &[]);
    m.build(&src);
    let act = |m: &Mfd, text: &str| m.flat.iter().filter_map(|f| f.act.clone()).find(|a| a.contains(text));
    let request = act(&m, "CPNY F-PLN").expect("CPNY F-PLN REQUEST button");
    assert!(matches!(m.act(&src, &request)[..], [Out::CpnyFplnRequest(0)]), "{request}");
    state.set(2.);
    let src = Sources::from_snapshot(&s, &vars, &[]);
    m.build(&src);
    assert!(model::init(&m.capt, &src).cpny_fpln_avail);
    assert!(m.act(&src, "c|bmenu:CAPT_MFD_fplnreq").is_empty());
    assert!(matches!(m.act(&src, "c|item:CAPT_MFD_fplnreq_0")[..], [Out::CpnyFplnInsert(0)]));
    assert!(matches!(m.act(&src, "c|item:CAPT_MFD_fplnreq_1")[..], [Out::CpnyFplnClear]));
}

#[test]
fn a_kccu_types_only_into_its_own_sides_focused_field() {
    let empty = Snapshot { plans: vec![None; 6], perf: BTreeMap::new(), phase: 0, ..Default::default() };
    let src = Sources::from_snapshot(&empty, sources::NO_VARS, &[]);
    let mut m = Mfd::default();
    m.act(&src, "c|field:FROM");
    assert!(!m.kccu_key(true, &src, None, Key::Char('Q')).0, "the right KCCU leaves the captain's field alone");
    for c in "EGLL/KJFK".chars() {
        m.kccu_key(false, &src, None, Key::Char(c));
    }
    let (_, out) = m.kccu_key(false, &src, None, Key::Enter);
    assert!(matches!(&out[..], [Out::Fms(HostCommand::Plan(Plan::NewCityPair { from, to, .. }))] if from == "EGLL" && to == "KJFK"));
}

#[test]
fn kccu_page_keys_open_fbws_pages_and_arrows_scroll_the_fpln() {
    let s = snapshot();
    let src = Sources::from_snapshot(&s, sources::NO_VARS, &[]);
    let mut m = Mfd::default();
    for (key, page) in [
        ("INIT", Page::Init),
        ("PERF", Page::Perf),
        ("DIR", Page::DirTo),
        ("NAVAID", Page::PosNavaids),
        ("SECINDEX", Page::SecIndex),
        ("SURV", Page::SurvControls),
        ("ATCCOM", Page::AtcConnect),
        ("FPLN", Page::Fpln),
    ] {
        assert!(m.kccu_command(false, &src, key).0, "{key}");
        assert_eq!(m.capt.page(), page, "{key}");
    }
    assert_eq!(m.fo.page(), Mfd::default().fo.page(), "the left KCCU leaves the F/O's MFD alone");
    m.build(&src);
    m.kccu_command(false, &src, "DOWN");
    assert_eq!(m.capt.fpln_scroll, Some(1));
    m.kccu_command(false, &src, "UP");
    assert_eq!(m.capt.fpln_scroll, Some(0));
    m.kccu_command(false, &src, "DEST");
    let (_, start, total) = model::fpln_visible_lines(&m.capt, &src);
    assert_eq!(start, total.saturating_sub(model::FPLN_PAGE_LINES), "DEST shows the end of the plan");
    assert!(!m.kccu_command(false, &src, "MAILBOX").0, "FBW moves the cursor to the SD there");
}

#[test]
fn a_discontinuity_line_opens_its_revision_menu_and_deletes() {
    let s = snapshot();
    let src = Sources::from_snapshot(&s, sources::NO_VARS, &[]);
    let mut m = Mfd::default();
    m.capt.navigate("fms/active/f-pln");
    m.build(&src);
    let (lines, _, _) = model::fpln_visible_lines(&m.capt, &src);
    let k = lines.iter().position(|l| l.label == "DISCONTINUITY").expect("city pair has a disco");
    let act = format!("c|fpln:ident:{k}");
    assert!(acts(&m).contains(&act), "the DISCONTINUITY line is pressable");
    m.act(&src, &act);
    assert_eq!(m.capt.open.as_deref(), Some("revisions"));
    let out = m.act(&src, "c|item:CAPT_MFD_revisionsMenu_2");
    assert!(!out.is_empty(), "DELETE * removes it: {out:?}");
}

#[test]
fn kccu_arrows_and_the_wheel_scroll_an_open_sid_list() {
    let s = snapshot();
    let src = Sources::from_snapshot(&s, sources::NO_VARS, &[]);
    let mut menu = mfd_tree::Node::el("div", "mfd-dropdown-menu");
    for _ in 0..20 {
        menu = menu.child(mfd_tree::Node::el("span", "mfd-dropdown-menu-element"));
    }
    let mut m = Mfd::default();
    m.tree = mfd_tree::Node::new("body")
        .child(mfd_tree::Node::el("div", "mfd-dropdown-container").child(mfd_tree::Node::new("div").attr(tree::ACT, "c|dd:SID")).child(menu));
    m.capt.open = Some("SID".into());
    assert!(m.kccu_command(false, &src, "DOWN").0);
    assert_eq!(m.capt.menu_scroll, 33.);
    assert!(m.wheel(10., 100.));
    assert_eq!(m.capt.menu_scroll, 133.);
    m.kccu_command(false, &src, "UP");
    assert_eq!(m.capt.menu_scroll, 100.);
    assert_eq!(m.fo.menu_scroll, 0., "the left KCCU leaves the F/O's menu alone");
}

#[test]
fn fpln_lines_show_the_vnav_speed_and_altitude_predictions() {
    let mut s = snapshot();
    let p = fms_host::WaypointPrediction { altitude: 12000., speed: 250., seconds_from_present: 600., estimated_fuel_on_board: 200_000., ..Default::default() };
    s.predictions.insert(0, p);
    let vars = |n: &str| if n == "ZULU TIME" { 3600. } else { 0. };
    let src = Sources::from_snapshot(&s, &vars, &[]);
    let mut ui = UiState::new(true);
    ui.navigate("fms/active/f-pln");
    let (lines, _, _) = model::fpln_visible_lines(&ui, &src);
    let l = lines.iter().find(|l| l.original_leg_index == Some(0)).unwrap();
    assert_eq!((l.altitude_prediction, l.speed_prediction), (Some(12000.), Some(250.)));
    assert_eq!(l.eta_ms, Some(4200. * 1000.));
}

#[test]
fn surv_controls_toggle_taws_set_tcas_xpdr_and_squawk() {
    let s = snapshot();
    let vars = |n: &str| match n {
        "TRANSPONDER CODE:1" => 0x2000 as f64,
        "L:A32NX_TCAS_MODE" => 2.,
        _ => 0.,
    };
    let src = Sources::from_snapshot(&s, &vars, &[]);
    let mut m = Mfd::default();
    m.capt.navigate("surv/controls");
    m.build(&src);
    assert!(tree::text_of(&m.tree).contains("2000"), "BCD squawk shown as 2000");
    let a = acts(&m);
    for want in ["c|surv:TERR SYS", "c|surv:GPWS", "c|surv:G/S MODE", "c|surv:FLAP MODE", "c|surv:ALT RPTG", "c|field:SQWK", "c|btn:IDENT"] {
        assert!(a.iter().any(|x| x == want), "{want} in {a:?}");
    }
    let var = |out: &[Out], name: &str| out.iter().find_map(|o| match o { Out::Var(n, v) if n == name => Some(*v), _ => None });
    let out = m.act(&src, "c|surv:GPWS");
    assert_eq!(var(&out, "L:A32NX_GPWS_SYS_OFF"), Some(1.));
    let out = m.act(&src, "c|radio:CAPT_MFD_survControlsTcasTara_1");
    assert_eq!((var(&out, model::TCAS_ALERT_LEVEL), var(&out, model::TCAS_MIRROR)), (Some(1.), Some(1.)), "TA ONLY");
    let out = m.act(&src, "c|radio:CAPT_MFD_survControlsXpdrStatus_1");
    assert_eq!((var(&out, model::XPDR_AUTO), var(&out, model::XPDR_ALT_RPTG)), (Some(0.), Some(1.)), "STBY keeps ALT RPTG");
    assert!(matches!(&m.act(&src, "c|btn:IDENT")[..], [Out::KeyEvent(k, _)] if k == "XPNDR_IDENT_ON"));

    m.act(&src, "c|field:SQWK");
    for c in "7700".chars() {
        m.key(&src, None, Key::Char(c));
    }
    let (_, out) = m.key(&src, None, Key::Enter);
    assert!(matches!(&out[..], [Out::KeyEvent(k, v)] if k == "XPNDR_SET" && *v == 0x7700 as f64), "{out:?}");
    m.act(&src, "c|field:SQWK");
    for c in "7800".chars() {
        m.key(&src, None, Key::Char(c));
    }
    assert!(m.key(&src, None, Key::Enter).1.is_empty(), "8 is not an octal digit");

    // The model shows what the MFD set.
    let set = |n: &str| match n {
        "L:A380X_TCAS_MFD_MIRROR" => 1.,
        "L:A380X_TCAS_MFD_ALERT_LEVEL" => 1.,
        "L:A380X_TCAS_MFD_ALT_SELECT" => 2.,
        _ => 0.,
    };
    let src = Sources::from_snapshot(&s, &set, &[]);
    let c = model::surv_controls(&m.capt, &src);
    assert_eq!((c.tcas_tara_selected, c.tcas_alt_selected, c.xpdr_selected), (Some(1), Some(2), Some(1)), "XPDR unset, transponder off: STBY");

    // Before the MFD sets it, XPDR shows what the transponder does: STBY (1) after a cold and dark spawn.
    for (state, want) in [(1., Some(1)), (5., Some(0))] {
        let tx = move |n: &str| if n == "TRANSPONDER STATE:1" { state } else { 0. };
        let src = Sources::from_snapshot(&s, &tx, &[]);
        assert_eq!(model::surv_controls(&m.capt, &src).xpdr_selected, want, "state {state}");
    }
}

#[test]
fn only_the_active_init_fuel_and_perf_pages_restart_preflight() {
    let mut m = Mfd::default();
    m.fo.navigate("fms/active/f-pln");
    for (u, want) in [("fms/active/init", true), ("fms/active/fuel-load", true), ("fms/active/perf", true), ("fms/sec1/init", false), ("fms/position/navaids", false), ("fms/active/f-pln", false)] {
        m.capt.navigate(u);
        assert_eq!(m.preflight_page_open(), want, "{u}");
    }
}

#[test]
fn navaids_page_tunes_vor_frequency_and_course_through_the_fms() {
    let db = db();
    let mut core = FmsCore::new();
    let mut m = Mfd::default();
    m.capt.navigate("fms/position/navaids");
    let mut enter = |m: &mut Mfd, core: &mut FmsCore, act: &str, text: &str| {
        let s = core.snapshot();
        let src = Sources::from_snapshot(&s, sources::NO_VARS, &[]);
        m.build(&src);
        m.act(&src, act);
        for c in text.chars() {
            m.key(&src, Some(&db), Key::Char(c));
        }
        let (_, out) = m.key(&src, Some(&db), Key::Enter);
        for o in &out {
            if let Out::Fms(c) = o {
                core.command(&db, Envelope { id: 1, command: c.clone() });
            }
        }
        out
    };
    assert!(!enter(&mut m, &mut core, "c|field:VOR1#2", "112.30").is_empty());
    enter(&mut m, &mut core, "c|field:VOR1#3", "90");
    assert!(enter(&mut m, &mut core, "c|field:VOR2#2", "118.00").is_empty(), "above the VOR band");
    assert!(enter(&mut m, &mut core, "c|field:LS", "ZZZZ").is_empty(), "not in the database");
    let s = core.snapshot();
    let src = Sources::from_snapshot(&s, sources::NO_VARS, &[]);
    m.build(&src);
    let text = tree::text_of(&m.tree);
    assert!(text.contains("112.30") && text.contains("090"), "{text}");
    assert!(enter(&mut m, &mut core, "c|field:VOR1#2", "CLR").iter().any(|o| matches!(o, Out::Fms(HostCommand::TuneVor { index: 0, entry: fms_host::NavaidEntry::Clear }))));
    assert_eq!(core.snapshot().navaids.vor[0].frequency, None);
}

#[test]
fn fpln_lists_the_top_of_descent_before_its_leg() {
    let mut s = snapshot();
    let dest = s.plans[0].as_ref().and_then(|p| p.destination_leg_index()).unwrap();
    s.pseudo_waypoints.push(fms_host::PseudoWaypointPrediction { ident: "(T/D)".into(), along_leg_index: Some(dest), displayed_on_mcdu: true, altitude: Some(35000.), ..Default::default() });
    s.pseudo_waypoints.push(fms_host::PseudoWaypointPrediction { ident: "(HIDDEN)".into(), along_leg_index: Some(dest), displayed_on_mcdu: false, ..Default::default() });
    let src = Sources::from_snapshot(&s, sources::NO_VARS, &[]);
    let lines = model::fpln_line_data(src.plan(0), model::preds(0, &src), model::pwps(0, &src));
    let td = lines.iter().position(|l| l.ident == "(T/D)").expect("T/D listed");
    assert!(lines[td].is_pseudo && lines[td].original_leg_index.is_none());
    assert_eq!(lines[td + 1].original_leg_index, Some(dest));
    assert!(!lines.iter().any(|l| l.ident == "(HIDDEN)"));
    let mut ui = UiState::new(true);
    ui.navigate("fms/active/f-pln");
    let (shown, _, _) = model::fpln_visible_lines(&ui, &src);
    let active = s.plans[0].as_ref().unwrap().active_leg_index as usize;
    assert!(shown.iter().filter(|l| l.is_active_leg).all(|l| l.original_leg_index == Some(active)));
}

#[test]
fn fpln_dest_row_shows_the_destination_time_and_efob() {
    let mut s = snapshot();
    let dest = s.plans[0].as_ref().and_then(|p| p.destination_leg_index()).unwrap();
    s.predictions.insert(dest, fms_host::WaypointPrediction { seconds_from_present: 600., estimated_fuel_on_board: 22046.2, ..Default::default() });
    let now = |n: &str| if n == "ZULU TIME" { 3600. } else { 0. };
    let src = Sources::from_snapshot(&s, &now, &[]);
    let mut ui = UiState::new(true);
    ui.navigate("fms/active/f-pln");
    let d = model::fpln(&ui, &src).dest;
    assert_eq!(d.time_ms, Some(4_200_000.));
    assert!((d.efob_kg.unwrap() - 10_000.).abs() < 1.);
}

#[test]
fn perf_activate_appr_asks_then_sends_the_approach_phase() {
    let s = snapshot();
    let src = Sources::from_snapshot(&s, sources::NO_VARS, &[]);
    let mut m = Mfd::default();
    m.capt.navigate("fms/active/perf");
    m.build(&src);
    assert!(m.act(&src, "c|btn:ACTIVATE APPR *").is_empty());
    assert!(model::perf(&m.capt, &src).approach_dialog_visible);
    m.build(&src);
    let out = m.act(&src, "c|btn:CONFIRM *");
    assert!(matches!(&out[..], [Out::Fms(fms_host::HostCommand::ActivateApproach)]), "{out:?}");
    assert!(!m.capt.approach_dialog_open);
}

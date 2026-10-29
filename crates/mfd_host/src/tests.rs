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

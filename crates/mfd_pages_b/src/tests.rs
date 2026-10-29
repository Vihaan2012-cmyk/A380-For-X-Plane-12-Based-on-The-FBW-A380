use crate::fuel::{fuel_load_page, FuelModel};
use crate::init::{init_page, InitModel};
use crate::perf::{perf_page, PerfModel};
use mfd_tree::{Kid, Node};

fn walk<'a>(n: &'a Node, out: &mut Vec<&'a Node>) {
    out.push(n);
    for k in &n.kids {
        if let Kid::Node(c) = k {
            walk(c, out);
        }
    }
}

fn all(n: &Node) -> Vec<&Node> {
    let mut v = Vec::new();
    walk(n, &mut v);
    v
}

fn text_of(n: &Node) -> String {
    let mut s = String::new();
    for k in &n.kids {
        match k {
            Kid::Text { text } => s.push_str(text),
            Kid::Node(c) => s.push_str(&text_of(c)),
        }
    }
    s
}

fn with_class<'a>(n: &'a Node, cls: &str) -> Vec<&'a Node> {
    all(n).into_iter().filter(|x| x.cls.iter().any(|c| c == cls)).collect()
}

fn by_id<'a>(n: &'a Node, id: &str) -> Option<&'a Node> {
    all(n).into_iter().find(|x| x.id.as_deref() == Some(id))
}

fn inputs(n: &Node) -> Vec<String> {
    with_class(n, "mfd-input-field-text-input").into_iter().map(text_of).collect()
}

fn display_of(n: &Node) -> &str {
    n.inline.as_deref().unwrap_or("")
}

#[test]
fn init_shows_entered_airports_and_flight_number() {
    let m = InitModel {
        flight_number: Some("TESTING".to_string()),
        from: Some("YSSY".to_string()),
        to: Some("OMDB".to_string()),
        ..Default::default()
    };
    let t = init_page(&m);
    assert_eq!(t.cls, vec!["mfd-page-container"]);
    let texts = inputs(&t);
    assert!(texts.contains(&"TESTING".to_string()));
    assert!(texts.contains(&"YSSY".to_string()));
    assert!(texts.contains(&"OMDB".to_string()));
}

#[test]
fn init_empty_plan_shows_mandatory_placeholders() {
    let t = init_page(&InitModel::default());
    let texts = inputs(&t);
    assert!(texts.iter().any(|s| s.contains('\u{25AF}')));
    assert!(!texts.contains(&"YSSY".to_string()));
}

#[test]
fn init_open_dropdown_class_changes_only_the_menu() {
    let closed = init_page(&InitModel { cost_index_mode: 1, has_active_fp_with_city_pair: true,
        from: Some("YSSY".to_string()),
        to: Some("OMDB".to_string()),
        ..Default::default()
    });
    let open = init_page(&InitModel {
        cost_index_mode: 1,
        has_active_fp_with_city_pair: true,
        from: Some("YSSY".to_string()),
        to: Some("OMDB".to_string()),
        open_dropdown: Some("costIndexMode".to_string()),
        ..Default::default()
    });
    assert_eq!(all(&closed).len(), all(&open).len());
    let edit = |t: &Node| with_class(t, "editing").len();
    assert_eq!(edit(&closed), 0);
    assert_eq!(edit(&open), 1);
}

#[test]
fn fuel_weights_follow_unit_setting() {
    let m = FuelModel { zero_fuel_weight_kg: Some(300000.0), ..Default::default() };
    let metric = fuel_load_page(&m);
    assert!(inputs(&metric).contains(&"300.0".to_string()));
    let imperial = fuel_load_page(&FuelModel { metric: false, ..m });
    assert!(inputs(&imperial).contains(&"661.4".to_string()));
}

#[test]
fn fuel_block_line_hidden_after_engine_start() {
    let before = fuel_load_page(&FuelModel::default());
    let after = fuel_load_page(&FuelModel { engines_were_started: true, ..Default::default() });
    let vis = |t: &Node| display_of(with_class(t, "mfd-fms-fuel-load-block-line")[0]).to_string();
    assert!(vis(&before).contains("visible"));
    assert!(vis(&after).contains("hidden"));
}

#[test]
fn fuel_cost_index_disabled_for_lrc() {
    let t = fuel_load_page(&FuelModel {
        cost_index_mode: Some(0),
        cost_index: Some(48.0),
        dest_icao: Some("OMDB".to_string()),
        ..Default::default()
    });
    assert!(inputs(&t).contains(&"48".to_string()));
    assert!(!with_class(&t, "disabled").is_empty());
}

#[test]
fn perf_has_six_tabs_and_all_pages_present() {
    let t = perf_page(&PerfModel::default());
    let labels: Vec<String> = with_class(&t, "mfd-top-tab-navigator-bar-element-label").into_iter().map(text_of).collect();
    assert_eq!(labels, vec!["T.O", "CLB", "CRZ", "DES", "APPR", "GA"]);
    assert_eq!(with_class(&t, "mfd-top-tab-navigator-tab-content").len(), 6);
}

#[test]
fn perf_selected_tab_only_changes_classes_and_display() {
    let a = perf_page(&PerfModel { selected_tab: 0, ..Default::default() });
    let b = perf_page(&PerfModel { selected_tab: 3, ..Default::default() });
    assert_eq!(all(&a).len(), all(&b).len());
    let shown = |t: &Node| -> Vec<bool> {
        with_class(t, "mfd-top-tab-navigator-tab-content").into_iter().map(|p| display_of(p).ends_with("display: flex;")).collect()
    };
    assert_eq!(shown(&a), vec![true, false, false, false, false, false]);
    assert_eq!(shown(&b), vec![false, false, false, true, false, false]);
    let active = |t: &Node| with_class(t, "active").len();
    assert_eq!(active(&a), 2);
    assert_eq!(active(&b), 2);
}

#[test]
fn perf_takeoff_speeds_switch_to_confirmation() {
    let m = PerfModel {
        v1_tbc: Some(150.0),
        vr_tbc: Some(155.0),
        v2_tbc: Some(160.0),
        show_confirm_vspeeds: true,
        ..Default::default()
    };
    let t = perf_page(&m);
    let tmpy: Vec<String> = with_class(&t, "tmpy").into_iter().map(text_of).collect();
    assert_eq!(tmpy, vec!["150", "155", "160"]);
    let flap_rows: Vec<&Node> = with_class(&t, "mfd-fms-perf-to-v-speeds")[0]
        .kids
        .iter()
        .filter_map(|k| if let Kid::Node(n) = k { Some(n) } else { None })
        .filter(|n| display_of(n) == "display: none;")
        .collect();
    assert_eq!(flap_rows.len(), 3);
}

#[test]
fn perf_open_dropdown_marks_the_selected_field_editing() {
    let m = PerfModel { open_dropdown: Some("packs".to_string()), takeoff_packs: Some(1), ..Default::default() };
    let t = perf_page(&m);
    let editing = with_class(&t, "editing");
    assert_eq!(editing.len(), 1);
    assert_eq!(text_of(editing[0]), "ON");
    let menu = by_id(&t, "CAPT_MFD_packsDropdown_0").unwrap();
    assert_eq!(text_of(menu), "OFF/APU");
    let all_menus = with_class(&t, "mfd-dropdown-menu");
    let open = all_menus.iter().filter(|n| display_of(n).starts_with("display: block")).count();
    assert_eq!(open, 1);
}

#[test]
fn perf_cost_index_labels_follow_engine_out() {
    let t = perf_page(&PerfModel { eo_active: true, ..Default::default() });
    assert_eq!(text_of(by_id(&t, "CAPT_MFD_clbCostIndexModeDropdown_0").unwrap()), "EO-LRC");
    assert_eq!(text_of(by_id(&t, "CAPT_MFD_desCostIndexModeDropdown_1").unwrap()), "EO-ECON");
    let t = perf_page(&PerfModel { captain: false, ..Default::default() });
    assert!(by_id(&t, "FO_MFD_crzCostIndexModeDropdown_0").is_some());
}

#[test]
fn perf_inactive_takeoff_page_after_takeoff() {
    let m = PerfModel { active_flight_phase: 2, thr_red_alt: Some(1510.0), ..Default::default() };
    let t = perf_page(&m);
    let inactive = with_class(&t, "inactive").len();
    assert!(inactive > 0);
    let pre = perf_page(&PerfModel { thr_red_alt: Some(1510.0), ..Default::default() });
    assert!(with_class(&pre, "inactive").len() < inactive);
}

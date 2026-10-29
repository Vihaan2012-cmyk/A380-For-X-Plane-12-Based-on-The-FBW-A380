use mfd_pages_c::fmt::*;
use mfd_pages_c::fpln_rev::*;
use mfd_pages_c::position::*;
use mfd_pages_c::sec_index::*;
use mfd_pages_c::wind::*;
use std::fs;

const WPTS: &str = "KADOM,PKS,MACLA,APOMA,POLEV,KALUG,AS,ONOXA,NYOMA,SRITI,BLI,JUNHA,IRTAD,AVPIV,SJ,VJB,AROSO,KALIL,BILIK,GOBAS,PUGER,ANSAX,IGEBO,NOPEK,DUBTA,ELSAR,GIRNA,MMV,OPAMO,GULAM,BBM,MADOG,UDULO,OKILA,ERVIS,GUNDI,DARMI,BISET,ADPOP,BOLIS,TOTOX,LOSIM,KAXEM,MUSRU,PAROK,ITURA,ALMOG,VELOD,GEXAN,TAPRA,VAXAS,RUDAT,MIVEK,IMPED";

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "E:/fbw-build/mfd-pages-c/c2".to_string());
    let w = |name: &str, n: mfd_tree::Node| {
        fs::write(format!("{}/{}.json", out, name), n.to_json()).unwrap();
    };
    let nav = |kids: Vec<mfd_tree::Node>| mfd_tree::Node::el("div", "mfd-navigator-container").children(kids);
    let dir_wpts: Vec<String> = format!("{},OMDB", WPTS).split(',').map(String::from).collect();
    let dir = DirectToModel { tmpy_active: true, waypoints: dir_wpts, selected: Some(0), ..Default::default() };
    w("0040", nav(direct_to(&dir)));
    w("0044", nav(direct_to(&DirectToModel { editing: true, ..dir })));
    let hold_model = HoldModel {
        tmpy_active: true,
        hold_type: hold_type_label(true, false).into(),
        waypoint_ident: "KADOM".into(),
        inbound_course: Some(100.0),
        turn: Some(1),
        leg_parameter: Some(0),
        leg_time: Some(1.0),
        last_exit_utc: "--:--".into(),
        ..Default::default()
    };
    w("0065", nav(hold(&hold_model)));
    let awy = AirwaysModel {
        tmpy_active: true,
        revised_fix: "KADOM".into(),
        lines: vec![AirwayLine::default()],
        scroll_down_disabled: true,
        scroll_up_disabled: true,
    };
    w("0072", nav(airways(&awy)));
    let ppos = coordinate_to_string(-33.9461, 151.1772, false);

    let mon = PositionMonitorModel {
        accuracy_high: true,
        epe: 0.04,
        rnp: Some(2.0),
        position1: ppos.clone(),
        position2: ppos.clone(),
        ir_deviation: ["0.0".into(), "0.0".into(), "0.0".into()],
        footer_message: "NAV PRIMARY".into(),
        footer_background: "white".into(),
        ..Default::default()
    };
    w("0231", position_monitor(&mon));

    let vor = NavaidTuned { ident: "CB".into(), freq: "116.70".into(), course: "---".into(), class: "VOR/DME".into(), ..Default::default() };
    let mut nav = PositionNavaidsModel {
        vor: [vor.clone(), vor],
        ls_ident: "----".into(),
        ls_freq: "---.--".into(),
        ls_course: "----".into(),
        ls_slope: "---".into(),
        deselected: Default::default(),
        cap_or_fo: "CAPT".into(),
        ..Default::default()
    };
    w("0233", position_navaids(&nav));
    nav.selected_tab = 1;
    w("0235", position_navaids(&nav));

    let irs = PositionIrsModel {
        alignment_label: irs_alignment_label(true, false).into(),
        alignment_position: ppos.clone(),
        status: ["NAV".into(), "NAV".into(), "NAV".into()],
        show_data_for: 1,
        data_position: ppos.clone(),
        true_track: "335.0".into(),
        true_heading: "335.0".into(),
        ground_speed: "0.0".into(),
        magnetic_heading: "322.0".into(),
        wind_direction: "0.0".into(),
        wind_speed: "/0.0".into(),
        magnetic_variation: "13.0".into(),
        magnetic_variation_unit: "\u{b0}W".into(),
        gpirs_position: ppos.clone(),
        accuracy: "0".into(),
        ..Default::default()
    };
    w("0237", position_irs(&irs));

    let mut sec = SecIndexModel { cap_or_fo: "CAPT".into(), ..Default::default() };
    for i in 0..3 {
        sec.tabs[i] = SecTab { cpny_label: "CPNY F-PLN REQUEST".into(), import_items: import_items(i + 1), ..Default::default() };
    }
    for (name, tab) in [
        ("0243", 0), ("0245", 0), ("0248", 1), ("0251", 1), ("0254", 2), ("0257", 2), ("0261", 1), ("0262", 1), ("0265", 1),
        ("0268", 1), ("0271", 2), ("0273", 2), ("0276", 2), ("0278", 2), ("0279", 1), ("0282", 1), ("0284", 2), ("0287", 2),
    ] {
        sec.selected_tab = tab;
        let open = ["0245", "0251", "0257", "0262", "0268", "0273", "0278", "0282", "0287"].contains(&name);
        sec.tabs[tab].import_open = open;
        w(name, sec_index(&sec));
        sec.tabs[tab].import_open = false;
    }

    let wpts: Vec<String> = WPTS.split(',').map(String::from).collect();
    let base = WindModel::default();
    for n in ["0209", "0212"] {
        w(n, wind(&base));
    }
    let b = WindModel { selected_tab: 2, waypoints: wpts.clone(), selected_waypoint: "KADOM".into(), select_next_disabled: false, ..Default::default() };
    let r = |m: &WindModel, n: &str| WindModel { return_button_visible: n.starts_with("01"), ..m.clone() };
    for n in ["0147", "0155", "0215"] {
        w(n, wind(&r(&b, n)));
    }
    let c = WindModel { dropdown_editing: true, ..b.clone() };
    for n in ["0149", "0157", "0217"] {
        w(n, wind(&r(&c, n)));
    }
    w("0152", wind(&WindModel { selected_tab: 1, return_button_visible: true, ..b.clone() }));
    let e = WindModel { selected_tab: 3, alternate_disabled: true, alternate_primary: true, descent_entries_loaded: true, ..b.clone() };
    for n in ["0160", "0220"] {
        w(n, wind(&r(&e, n)));
    }
}

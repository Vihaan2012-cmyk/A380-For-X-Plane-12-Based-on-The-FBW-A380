use crate::formats::*;
use crate::init::{side_prefix, FP_ACTIVE, FP_TEMPORARY};
use crate::widgets::*;
use mfd_tree::Node;
use serde::Deserialize;

pub const PH_TAKEOFF: u8 = 1;
pub const PH_CLIMB: u8 = 2;
pub const PH_CRUISE: u8 = 3;
pub const PH_DESCENT: u8 = 4;

const ROW_CENTER: &str = "display: flex; flex-direction: row; justify-content: center; align-items: center;";
const CELL: &str = "mfd-fms-perf-speed-table-cell";
const PRESEL_CELL: &str = "mfd-fms-perf-speed-presel-managed-table-cell";

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct PerfModel {
    pub captain: bool,
    pub loaded_fp_index: i32,
    pub active_flight_phase: u8,
    pub is_active_or_copy: bool,
    pub eo_active: bool,
    pub sec_active: bool,
    pub metric: bool,
    pub selected_tab: usize,
    pub open_dropdown: Option<String>,
    pub crz_fl: Option<f64>,
    pub crz_fl_mandatory: bool,
    pub opt_fl: String,
    pub opt_fl_not_avail: bool,
    pub rec_max_fl: String,
    pub rec_max_fl_not_avail: bool,
    pub eo_max_fl: String,
    pub eo_max_fl_not_avail: bool,
    pub cost_index: Option<f64>,
    pub cost_index_mode: Option<usize>,
    pub trans_alt: Option<f64>,
    pub trans_alt_from_database: bool,
    pub trans_fl: Option<f64>,
    pub trans_fl_from_database: bool,
    pub thr_red_alt: Option<f64>,
    pub thr_red_alt_pilot: bool,
    pub accel_alt: Option<f64>,
    pub accel_alt_pilot: bool,
    pub eo_accel_alt: Option<f64>,
    pub eo_accel_alt_pilot: bool,
    pub noise_enabled: bool,
    pub noise_n1: Option<f64>,
    pub noise_speed: Option<f64>,
    pub noise_end_altitude: Option<f64>,
    pub origin_runway_ident: String,
    pub takeoff_shift_disabled: bool,
    pub takeoff_shift_m: Option<f64>,
    pub v1: Option<f64>,
    pub vr: Option<f64>,
    pub v2: Option<f64>,
    pub v1_tbc: Option<f64>,
    pub vr_tbc: Option<f64>,
    pub v2_tbc: Option<f64>,
    pub show_confirm_vspeeds: bool,
    pub to_flap_retraction: Option<f64>,
    pub to_slat_retraction: Option<f64>,
    pub to_green_dot: Option<f64>,
    pub takeoff_flaps_index: Option<usize>,
    pub takeoff_power_setting: Option<usize>,
    pub takeoff_derated: Option<usize>,
    pub takeoff_ths_for: Option<f64>,
    pub takeoff_packs: Option<usize>,
    pub takeoff_anti_ice: Option<usize>,
    pub flex_temp: Option<f64>,
    pub derated_dialog_visible: bool,
    pub derated_dialog_title: String,
    pub climb_derated: Option<usize>,
    pub climb_pred_ref: Option<f64>,
    pub climb_pred_ref_pilot: bool,
    pub climb_presel_speed: Option<f64>,
    pub speed_constraint_speed: Option<f64>,
    pub speed_constraint_altitude: Option<f64>,
    pub clb_noise_table_hidden: bool,
    pub clb_mode1: Option<String>,
    pub clb_spd1: Option<String>,
    pub clb_mach1: Option<String>,
    pub clb_pred1: Option<String>,
    pub clb_mode2: Option<String>,
    pub clb_spd2: Option<String>,
    pub clb_mach2: Option<String>,
    pub clb_pred2: Option<String>,
    pub clb_mode3: Option<String>,
    pub clb_spd3: Option<String>,
    pub clb_mach3: Option<String>,
    pub crz_pred: Option<String>,
    pub crz_pred_waypoint: String,
    pub crz_pred_altitude_target: Option<f64>,
    pub crz_mode1: Option<String>,
    pub crz_spd1: Option<String>,
    pub crz_mach1: Option<String>,
    pub crz_pred1: Option<String>,
    pub crz_mode2: Option<String>,
    pub crz_spd2: Option<String>,
    pub crz_mach2: Option<String>,
    pub crz_pred2: Option<String>,
    pub crz_mode3: Option<String>,
    pub crz_spd3: Option<String>,
    pub crz_mach3: Option<String>,
    pub cruise_presel_speed: Option<f64>,
    pub managed_speed_active: bool,
    pub dest_ident: String,
    pub dest_eta: String,
    pub dest_efob_kg: Option<f64>,
    pub dest_efob_below_min: bool,
    pub descent_cabin_rate: Option<f64>,
    pub des_pred_ref: Option<f64>,
    pub des_managed_spd: Option<f64>,
    pub des_managed_mach: Option<f64>,
    pub des_mode1: Option<String>,
    pub des_spd1: Option<String>,
    pub des_mach1: Option<String>,
    pub des_pred1: Option<String>,
    pub des_mode2: Option<String>,
    pub des_spd2: Option<String>,
    pub des_mach2: Option<String>,
    pub des_pred2: Option<String>,
    pub appr_ident: String,
    pub landing_weight_kg: Option<f64>,
    pub dest_airport_missing: bool,
    pub appr_wind_direction: Option<f64>,
    pub appr_wind_magnitude: Option<f64>,
    pub appr_head_wind: Option<f64>,
    pub appr_cross_wind: Option<f64>,
    pub appr_temperature: Option<f64>,
    pub appr_qnh: Option<f64>,
    pub appr_qnh_hpa: bool,
    pub appr_params_mandatory: bool,
    pub appr_baro_min: Option<f64>,
    pub appr_radio_min: Option<f64>,
    pub precision_approach: bool,
    pub appr_green_dot: Option<f64>,
    pub appr_slat_retraction: Option<f64>,
    pub appr_flap_retraction: Option<f64>,
    pub appr_vref: Option<f64>,
    pub appr_vls: Option<f64>,
    pub appr_vapp: Option<f64>,
    pub appr_vapp_pilot: bool,
    pub appr_flaps3: bool,
    pub appr_vertical_deviation: String,
    pub missed_thr_red_alt: Option<f64>,
    pub missed_thr_red_pilot: bool,
    pub missed_accel_alt: Option<f64>,
    pub missed_accel_pilot: bool,
    pub missed_eo_accel_alt: Option<f64>,
    pub missed_eo_accel_pilot: bool,
    pub approach_dialog_visible: bool,
    pub clear_eo_dialog_visible: bool,
}

impl Default for PerfModel {
    fn default() -> Self {
        PerfModel {
            captain: true,
            loaded_fp_index: FP_ACTIVE,
            active_flight_phase: 0,
            is_active_or_copy: true,
            eo_active: false,
            sec_active: false,
            metric: true,
            selected_tab: 0,
            open_dropdown: None,
            crz_fl: None,
            crz_fl_mandatory: true,
            opt_fl: "---".to_string(),
            opt_fl_not_avail: false,
            rec_max_fl: "---".to_string(),
            rec_max_fl_not_avail: false,
            eo_max_fl: "---".to_string(),
            eo_max_fl_not_avail: false,
            cost_index: None,
            cost_index_mode: None,
            trans_alt: None,
            trans_alt_from_database: false,
            trans_fl: None,
            trans_fl_from_database: true,
            thr_red_alt: None,
            thr_red_alt_pilot: false,
            accel_alt: None,
            accel_alt_pilot: false,
            eo_accel_alt: None,
            eo_accel_alt_pilot: false,
            noise_enabled: false,
            noise_n1: None,
            noise_speed: None,
            noise_end_altitude: None,
            origin_runway_ident: "---".to_string(),
            takeoff_shift_disabled: false,
            takeoff_shift_m: None,
            v1: None,
            vr: None,
            v2: None,
            v1_tbc: None,
            vr_tbc: None,
            v2_tbc: None,
            show_confirm_vspeeds: false,
            to_flap_retraction: None,
            to_slat_retraction: None,
            to_green_dot: None,
            takeoff_flaps_index: None,
            takeoff_power_setting: None,
            takeoff_derated: None,
            takeoff_ths_for: None,
            takeoff_packs: None,
            takeoff_anti_ice: None,
            flex_temp: None,
            derated_dialog_visible: false,
            derated_dialog_title: String::new(),
            climb_derated: None,
            climb_pred_ref: None,
            climb_pred_ref_pilot: false,
            climb_presel_speed: None,
            speed_constraint_speed: None,
            speed_constraint_altitude: None,
            clb_noise_table_hidden: false,
            clb_mode1: Some("PRESEL".to_string()),
            clb_spd1: None,
            clb_mach1: None,
            clb_pred1: Some("--:--   ----".to_string()),
            clb_mode2: Some("MANAGED".to_string()),
            clb_spd2: Some("250".to_string()),
            clb_mach2: None,
            clb_pred2: None,
            clb_mode3: Some("ECON".to_string()),
            clb_spd3: Some("314".to_string()),
            clb_mach3: Some(".82".to_string()),
            crz_pred: None,
            crz_pred_waypoint: String::new(),
            crz_pred_altitude_target: None,
            crz_mode1: Some("PRESEL".to_string()),
            crz_spd1: None,
            crz_mach1: None,
            crz_pred1: None,
            crz_mode2: Some("MANAGED".to_string()),
            crz_spd2: Some("---".to_string()),
            crz_mach2: Some(".82".to_string()),
            crz_pred2: Some("--:--   ----".to_string()),
            crz_mode3: None,
            crz_spd3: None,
            crz_mach3: None,
            cruise_presel_speed: None,
            managed_speed_active: false,
            dest_ident: "----".to_string(),
            dest_eta: "--:--".to_string(),
            dest_efob_kg: None,
            dest_efob_below_min: false,
            descent_cabin_rate: None,
            des_pred_ref: None,
            des_managed_spd: None,
            des_managed_mach: None,
            des_mode1: Some("PRESEL".to_string()),
            des_spd1: None,
            des_mach1: None,
            des_pred1: Some("--:--   ----".to_string()),
            des_mode2: Some("MANAGED".to_string()),
            des_spd2: Some("250".to_string()),
            des_mach2: None,
            des_pred2: None,
            appr_ident: "-------".to_string(),
            landing_weight_kg: None,
            dest_airport_missing: false,
            appr_wind_direction: None,
            appr_wind_magnitude: None,
            appr_head_wind: None,
            appr_cross_wind: None,
            appr_temperature: None,
            appr_qnh: None,
            appr_qnh_hpa: true,
            appr_params_mandatory: false,
            appr_baro_min: None,
            appr_radio_min: None,
            precision_approach: false,
            appr_green_dot: None,
            appr_slat_retraction: None,
            appr_flap_retraction: None,
            appr_vref: None,
            appr_vls: None,
            appr_vapp: None,
            appr_vapp_pilot: false,
            appr_flaps3: false,
            appr_vertical_deviation: "+-----".to_string(),
            missed_thr_red_alt: None,
            missed_thr_red_pilot: false,
            missed_accel_alt: None,
            missed_accel_pilot: false,
            missed_eo_accel_alt: None,
            missed_eo_accel_pilot: false,
            approach_dialog_visible: false,
            clear_eo_dialog_visible: false,
        }
    }
}

struct Ctx<'a> {
    m: &'a PerfModel,
    side: &'static str,
    mandatory_and_active: bool,
    vis: &'static str,
    to_inactive: bool,
    clb_inactive: bool,
    at_or_after_climb: bool,
    crz_inactive: bool,
    des_inactive: bool,
    not_yet_climb: bool,
    not_yet_cruise: bool,
    not_yet_descent: bool,
    not_in_descent: bool,
    in_flight: bool,
    trans: TransAlt,
}

impl<'a> Ctx<'a> {
    fn new(m: &'a PerfModel) -> Ctx<'a> {
        let fp = m.active_flight_phase;
        let act = m.is_active_or_copy;
        let mand = m.loaded_fp_index == FP_ACTIVE || m.loaded_fp_index == FP_TEMPORARY;
        Ctx {
            m,
            side: side_prefix(m.captain),
            mandatory_and_active: mand,
            vis: if mand { "inherit" } else { "hidden" },
            to_inactive: act && fp >= PH_TAKEOFF,
            clb_inactive: act && fp > PH_CLIMB,
            at_or_after_climb: act && fp >= PH_CLIMB,
            crz_inactive: act && fp > PH_CRUISE,
            des_inactive: act && fp > PH_DESCENT,
            not_yet_climb: !act || fp < PH_CLIMB,
            not_yet_cruise: !act || fp < PH_CRUISE,
            not_yet_descent: !act || fp < PH_DESCENT,
            not_in_descent: !act || fp != PH_DESCENT,
            in_flight: fp >= PH_CLIMB && fp <= PH_DESCENT,
            trans: TransAlt { value_ft: m.trans_alt, is_fl: false },
        }
    }

    fn open(&self, key: &str) -> bool {
        self.m.open_dropdown.as_deref() == Some(key)
    }

    fn ci_mode_disabled(&self) -> bool {
        self.m.is_active_or_copy && self.m.active_flight_phase >= PH_DESCENT
    }

    fn ci_disabled(&self) -> bool {
        self.m.cost_index_mode == Some(0) || self.ci_mode_disabled()
    }

    fn ci_labels(&self) -> [&'static str; 2] {
        if self.m.eo_active {
            ["EO-LRC", "EO-ECON"]
        } else {
            ["LRC", "ECON"]
        }
    }
}

fn os(v: &Option<String>) -> &str {
    v.as_deref().unwrap_or("")
}

fn value_span(sec: bool, text: &str) -> Node {
    Node::el("span", "mfd-value").class_if("sec", sec).text(text)
}

fn unit_trailing(text: &str) -> Node {
    span_c("mfd-label-unit mfd-unit-trailing", text)
}

fn label_sr(text: &str) -> Node {
    span_c("mfd-label mfd-spacing-right", text)
}

fn num_text(v: Option<f64>) -> String {
    v.map(js_num).unwrap_or_default()
}

fn kt_value(sec: bool, text: &str, unit: &str) -> Node {
    label_value("").child(value_span(sec, text)).child(unit_trailing(unit))
}

fn kt_after(sec: bool, label: &str, v: Option<f64>) -> Node {
    label_value("")
        .child(label_sr(label))
        .child(value_span(sec, &fmc_format_value(v)))
        .child(unit_trailing("KT"))
}

fn unit_if(text: &Option<String>, unit: &str) -> &'static str {
    let _ = unit;
    if os(text).is_empty() {
        ""
    } else {
        "KT"
    }
}

fn nm_if(text: &Option<String>) -> &'static str {
    if os(text).is_empty() {
        ""
    } else {
        "NM"
    }
}

fn arrow_cell(label: &str, class: &str, style: &str) -> Node {
    div_c(class, style).child(arrow_svg()).child(
        Node::el("span", "mfd-label").style("width: 40px; margin-left: 10px; text-align: right").text(label),
    )
}

fn radio_wrap(values: &[&str], prefix: &str, vspace: i32, selected: Option<usize>, disabled: &[bool]) -> Node {
    radio_group(values, prefix, vspace, selected, disabled)
}

fn tab_bar(titles: &[&str], selected: usize) -> Node {
    let mut bar = Node::el("div", "mfd-top-tab-navigator-bar").style("height: 36px");
    let tw = "20.784609690826528";
    for (i, t) in titles.iter().enumerate() {
        let sel = i == selected;
        let fill = if sel { "fill:#000000;" } else { "fill:#272525;" };
        let mut s1 = vec![
            shape("polygon", &[("points", &format!("0,36 {},0 {},36", tw, tw)), ("style", fill)]),
            shape("line", &[("x1", "0"), ("y1", "36"), ("x2", tw), ("y2", "0"), ("style", "stroke: lightgrey; stroke-width:2")]),
        ];
        let mut s2 = vec![
            shape("polygon", &[("points", &format!("0,0 {},36 0,36", tw)), ("style", fill)]),
            shape("line", &[("x1", "0"), ("y1", "0"), ("x2", tw), ("y2", "36"), ("style", "stroke: lightgrey; stroke-width:2")]),
        ];
        if !sel {
            s1.push(shape("line", &[("x1", "0"), ("y1", "35"), ("x2", tw), ("y2", "35"), ("style", "stroke: lightgrey; stroke-width:2")]));
            s2.push(shape("line", &[("x1", "0"), ("y1", "35"), ("x2", tw), ("y2", "35"), ("style", "stroke: lightgrey; stroke-width:2")]));
        }
        let label = Node::el("span", "mfd-top-tab-navigator-bar-element-label")
            .class_if("active", sel)
            .style(&format!("font-size: 21px;{}", if sel { " color: white;" } else { "" }))
            .text(t);
        bar = bar.child(
            Node::el("div", "mfd-top-tab-navigator-bar-element-outer")
                .class_if("active", sel)
                .style(&format!("margin-right: {}px", if i == titles.len() - 1 { 0 } else { -15 }))
                .child(svg_node(tw, "36", s1))
                .child(label)
                .child(svg_node(tw, "36", s2)),
        );
    }
    bar
}

fn tab_page(visible: bool, container_style: &str, kids: Vec<Node>) -> Node {
    let style = format!("{}display: {};", container_style, if visible { "flex" } else { "none" });
    Node::el("div", "mfd-top-tab-navigator-tab-content").style(&style).children(kids)
}

fn ci_header_mode_dropdown(c: &Ctx, id: &str, key: &str) -> Node {
    let labels = c.ci_labels();
    Dropdown::new(&format!("{}_MFD_{}", c.side, id), &labels, c.m.cost_index_mode)
        .disabled(c.ci_mode_disabled())
        .cstyle("width: 175px; margin-right: 15px;")
        .digits(7)
        .open(c.open(key))
        .node()
}

fn ci_field(c: &Ctx) -> Node {
    field(cost_index(c.m.cost_index)).disabled(c.ci_disabled()).cstyle("width: 75px;").align("center").node()
}

fn top_row(c: &Ctx) -> Node {
    let m = c.m;
    div("margin: 5px; display: grid; grid-template-columns: 1fr 1fr 1fr; justify-content: space-between;")
        .child(
            label_value("padding: 0px 0px 0px 20px; height:37px;")
                .child(label_sr("CRZ"))
                .child(field(flight_level(m.crz_fl)).mandatory(m.crz_fl_mandatory).align("center").key("CRZ FL").node()),
        )
        .child(
            label_value(&format!("padding: 0px; justify-content: center; visibility: {};", c.vis))
                .child(label_sr("OPT"))
                .child(span_c("mfd-label-unit mfd-unit-leading", "FL"))
                .child(Node::el("span", "mfd-value").class_if("white", m.opt_fl_not_avail).text(&m.opt_fl)),
        )
        .child(
            label_value(&format!("padding: 0px 20px 0px 0px; justify-content: flex-end; visibility: {};", c.vis))
                .child(label_sr("REC MAX"))
                .child(span_c("mfd-label-unit mfd-unit-leading", "FL"))
                .child(Node::el("span", "mfd-value").class_if("white", m.rec_max_fl_not_avail).text(&m.rec_max_fl)),
        )
        .child(div(""))
        .child(div(""))
        .child(
            label_value(&format!("padding: 0px 20px 0px 0px; justify-content: flex-end; visibility: {};", c.vis))
                .child(Node::el("span", "mfd-label mfd-spacing-right").class_if("amber", m.eo_active).text("\u{00A0}EO MAX"))
                .child(span_c("mfd-label-unit mfd-unit-leading", "FL"))
                .child(Node::el("span", "mfd-value").class_if("white", m.eo_max_fl_not_avail).text(&m.eo_max_fl)),
        )
}

fn noise_vis(c: &Ctx) -> &'static str {
    if c.m.noise_enabled {
        "visible"
    } else {
        "hidden"
    }
}

fn confirm_button() -> Node {
    button_node(two_line_star_label_spaced("CONFIRM", "T.O SPDs")).style("color: yellow; padding-right: 2px;").node()
}

fn to_tab(c: &Ctx) -> Node {
    let m = c.m;
    let confirm = m.show_confirm_vspeeds;
    let conf_disp = if confirm { "flex" } else { "none" };
    let flap_disp = if confirm { "none" } else { "flex" };
    let vfield = |label: &str, v: Option<f64>| {
        label_value("").child(label_sr(label)).child(
            field(speed_knots(v)).mandatory(c.mandatory_and_active).inactive(c.to_inactive).align("flex-end").node(),
        )
    };
    let tbc = |v: Option<f64>| {
        label_value("").child(
            div(&format!("display: {};", conf_disp))
                .child(Node::el("span", "mfd-value tmpy").text(&fmc_format_value(v)))
                .child(unit_trailing("KT")),
        )
    };
    let sp = |label: Node, v: Option<f64>| {
        label_value(&format!("display: {};", flap_disp))
            .child(label)
            .child(value_span(m.sec_active, &fmc_format_value(v)))
            .child(unit_trailing("KT"))
    };
    let v_speeds = div_c("mfd-fms-perf-to-v-speeds", "")
        .child(vfield("V1", m.v1))
        .child(tbc(m.v1_tbc))
        .child(
            div(&format!(
                "grid-row-start: span 3; display: {}; justify-content: flex-end; align-items: flex-end;",
                conf_disp
            ))
            .child(confirm_button()),
        )
        .child(sp(label_sr("F"), m.to_flap_retraction))
        .child(vfield("VR", m.vr))
        .child(tbc(m.vr_tbc))
        .child(sp(label_sr("S"), m.to_slat_retraction))
        .child(vfield("V2", m.v2))
        .child(tbc(m.v2_tbc))
        .child(sp(
            Node::new("span").style("margin-right: 15px; justify-content: center;").child(circle_svg()),
            m.to_green_dot,
        ));

    let setting = m.takeoff_power_setting.unwrap_or(0);
    let flex_vis = if setting == 1 { "visible" } else { "hidden" };
    let der_vis = if setting == 2 { "visible" } else { "hidden" };
    let derated_vals = ["D01", "D02", "D03", "D04", "D05"];

    let first = div_c("mfd-fms-perf-to-first", "")
        .child(
            label_value("padding: 15px;")
                .child(label_sr("RWY"))
                .child(span_c("mfd-value", &m.origin_runway_ident)),
        )
        .child(
            label_value("")
                .child(label_sr("T.O SHIFT"))
                .child(
                    field(length(m.takeoff_shift_m, m.metric))
                        .disabled(m.takeoff_shift_disabled)
                        .inactive(c.to_inactive)
                        .align("center")
                        .node(),
                ),
        );

    let second = div_c("mfd-fms-perf-to-second", "")
        .child(v_speeds)
        .child(confirmation_dialog(m.derated_dialog_visible, "width: 325px; height: 165px;", false, vec![], &m.derated_dialog_title))
        .child(
            div_c("mfd-fms-perf-to-flex-toga", "width: 200px;").child(
                Node::new("span")
                    .style("width: 175px; display: inline; margin-left: 15px;")
                    .child(radio_wrap(
                        &["TOGA", "FLEX", "DERATED"],
                        &format!("{}_MFD_toThrustSettingRadio", c.side),
                        15,
                        Some(setting),
                        &[m.active_flight_phase >= 1, m.active_flight_phase >= 1, true],
                    )),
            ),
        )
        .child(
            div_c("mfd-fms-perf-to-flex-toga", "width: 125px;")
                .child(
                    label_value(&format!("margin-top: 60px; visibility: {};", flex_vis)).child(
                        field(temperature(m.flex_temp)).inactive(c.to_inactive).align("center").key("FLEX").node(),
                    ),
                )
                .child(
                    div(&format!("margin-top: 0px; visibility: {};", der_vis)).child(
                        Dropdown::new(&format!("{}_MFD_deratedDropdown", c.side), &derated_vals, m.takeoff_derated)
                            .inactive(c.to_inactive)
                            .cstyle("width: 100px;")
                            .digits(3)
                            .align("flex-start")
                            .open(c.open("derated"))
                            .node(),
                    ),
                ),
        );

    let mt15 = "margin-top: 15px;";
    let flaps_grid = div_c("mfd-fms-perf-to-flaps-packs-grid", "")
        .child(div("").child(span_c("mfd-label", "FLAPS")))
        .child(div("").child(span_c("mfd-label", "THS FOR")))
        .child(div("grid-row-start: span 2; border-left: 1px solid lightgrey; margin-right: 10px;"))
        .child(div("").child(span_c("mfd-label", "PACKS")))
        .child(div("").child(span_c("mfd-label", "ANTI ICE")))
        .child(
            div(mt15).child(
                Dropdown::new(&format!("{}_MFD_flapDropdown", c.side), &["1", "2", "3"], m.takeoff_flaps_index)
                    .inactive(c.to_inactive)
                    .cstyle("width: 75px;")
                    .digits(1)
                    .open(c.open("flaps"))
                    .node(),
            ),
        )
        .child(
            div("margin-top: 15px; align-self: center;").child(
                field(percentage(m.takeoff_ths_for))
                    .mandatory(c.mandatory_and_active)
                    .inactive(c.to_inactive)
                    .align("flex-end")
                    .key("THS FOR")
                    .node(),
            ),
        )
        .child(
            div(mt15).child(
                Dropdown::new(&format!("{}_MFD_packsDropdown", c.side), &["OFF/APU", "ON"], m.takeoff_packs)
                    .inactive(c.to_inactive)
                    .cstyle("width: 200px;")
                    .digits(8)
                    .open(c.open("packs"))
                    .node(),
            ),
        )
        .child(
            div(mt15).child(
                Dropdown::new(&format!("{}_MFD_antiIceDropdown", c.side), &["OFF", "ENG ONLY", "ENG + WING"], m.takeoff_anti_ice)
                    .inactive(c.to_inactive)
                    .cstyle("width: 225px;")
                    .digits(10)
                    .open(c.open("antiIce"))
                    .node(),
            ),
        );

    let nv = noise_vis(c);
    let alt_fl = |v: Option<f64>| altitude_or_fl(v, Some(&c.trans));
    let noise_btn_disp = if m.noise_enabled { "none" } else { "flex" };
    let noise_end_disp = if m.noise_enabled { "flex" } else { "none" };
    let cell_style = "margin-right: 15px; width: 125px;";
    let thrred = div_c("mfd-fms-perf-to-thrred-noise-grid", "")
        .child(div_c("mfd-fms-perf-to-thrred-noise-grid-cell", cell_style).child(span_c("mfd-label", "THR RED")))
        .child(
            div_c("mfd-fms-perf-to-thrred-noise-grid-cell-start", "").child(
                field(alt_fl(m.thr_red_alt))
                    .inactive(c.to_inactive)
                    .pilot(m.thr_red_alt_pilot)
                    .cstyle("width: 150px;")
                    .align("flex-end")
                    .node(),
            ),
        )
        .child(div("").child(arrow_cell(
            "N1",
            "mfd-fms-perf-to-thrred-noise-grid-cell",
            &format!("margin-right: 15px; visibility: {};", nv),
        )))
        .child(
            div("").child(
                div_c("mfd-fms-perf-to-thrred-noise-grid-cell-start", &format!("visibility: {};", nv)).child(
                    field(percentage(m.noise_n1)).inactive(c.to_inactive).cstyle("width: 110px;").align("flex-end").node(),
                ),
            ),
        )
        .child(
            div("grid-row-start: span 2;").child(
                div_c("mfd-fms-perf-to-thrred-noise-grid-cell-start", &format!("visibility: {};", nv)).child(conditional(
                    Some(112),
                    Some(62),
                    c.to_inactive,
                    vec![],
                    vec![button("CANCEL<br />NOISE").node()],
                )),
            ),
        )
        .child(div_c("mfd-fms-perf-to-thrred-noise-grid-cell", cell_style).child(span_c("mfd-label", "ACCEL")))
        .child(
            div_c("mfd-fms-perf-to-thrred-noise-grid-cell-start", "").child(
                field(alt_fl(m.accel_alt))
                    .inactive(c.to_inactive)
                    .pilot(m.accel_alt_pilot)
                    .cstyle("width: 150px;")
                    .align("flex-end")
                    .node(),
            ),
        )
        .child(div("").child(arrow_cell(
            "SPD",
            "mfd-fms-perf-to-thrred-noise-grid-cell",
            &format!("margin-right: 15px; visibility: {};", nv),
        )))
        .child(
            div("").child(
                div_c("mfd-fms-perf-to-thrred-noise-grid-cell-start", &format!("visibility: {};", nv)).child(
                    field(speed_knots(m.noise_speed)).inactive(c.to_inactive).cstyle("width: 110px;").align("flex-end").node(),
                ),
            ),
        )
        .child(
            div_c("mfd-fms-perf-to-thrred-noise-grid-cell", cell_style)
                .child(Node::el("span", "mfd-label").style(&format!("display: {};", noise_end_disp)).text("NOISE END")),
        )
        .child(
            div("")
                .child(
                    div_c("mfd-fms-perf-to-thrred-noise-grid-cell-start", &format!("display: {};", noise_btn_disp)).child(
                        conditional(
                            Some(98),
                            Some(40),
                            c.to_inactive,
                            vec![],
                            vec![button("NOISE").disabled(true).node()],
                        ),
                    ),
                )
                .child(
                    div_c("mfd-fms-perf-to-thrred-noise-grid-cell-centered", &format!("display: {};", noise_end_disp)).child(
                        field(alt_fl(m.noise_end_altitude))
                            .inactive(c.to_inactive)
                            .cstyle("width: 150px;")
                            .align("flex-end")
                            .key("END")
                            .node(),
                    ),
                ),
        )
        .child(div(""))
        .child(div(""));

    let bottom = div_c("mfd-fms-perf-to-bottom", "")
        .child(
            label_value("").child(label_sr("TRANS")).child(
                field(altitude(m.trans_alt))
                    .pilot(!m.trans_alt_from_database)
                    .cstyle("width: 150px;")
                    .align("flex-end")
                    .node(),
            ),
        )
        .child(
            label_value("").child(label_sr("EO ACCEL")).child(
                field(alt_fl(m.eo_accel_alt))
                    .inactive(c.to_inactive)
                    .pilot(m.eo_accel_alt_pilot)
                    .cstyle("width: 150px;")
                    .align("flex-end")
                    .node(),
            ),
        )
        .child(div(&format!("visibility: {};", c.vis)).child(conditional(
            Some(176),
            Some(62),
            c.to_inactive,
            vec![],
            vec![button("CPNY T.O<br />REQUEST")
                .disabled(true)
                .style("padding-left: 30px; padding-right: 30px")
                .node()],
        )));

    tab_page(
        m.selected_tab == 0,
        "",
        vec![first, second, flaps_grid, thrred, div("flex-grow: 1;"), bottom],
    )
}

fn clb_noise_grid(c: &Ctx) -> Node {
    let m = c.m;
    let nv = noise_vis(c);
    let alt_fl = |v: Option<f64>| altitude_or_fl(v, Some(&c.trans));
    let mb = "margin-right: 15px; margin-bottom: 15px;";
    let cell = "mfd-fms-perf-to-thrred-noise-grid-cell";
    let spd_lim_disp = if m.noise_enabled { "none" } else { "flex" };
    let noise_end_disp = if m.noise_enabled { "flex" } else { "none" };
    let reason = m
        .speed_constraint_altitude
        .filter(|v| *v != 0.0)
        .map(|v| fixed0(v / 100.0))
        .unwrap_or_default();
    let grid_vis = if m.clb_noise_table_hidden { "hidden" } else { "visible" };
    let ac = c.at_or_after_climb;
    div_c("mfd-fms-perf-to-thrred-noise-grid", &format!("padding-left: 15px; visibility: {};", grid_vis))
        .child(div_c(cell, "margin-right: 15px; margin-bottom: 15px; width: 125px;").child(span_c("mfd-label", "THR RED")))
        .child(div("margin-bottom: 15px;").child(
            field(alt_fl(m.thr_red_alt)).inactive(ac).pilot(m.thr_red_alt_pilot).cstyle("width: 150px;").align("flex-end").node(),
        ))
        .child(div("").child(arrow_cell("N1", cell, &format!("{} visibility: {};", mb, nv))))
        .child(div("").child(
            div(&format!("margin-bottom: 15px; visibility: {};", nv)).child(
                field(percentage(m.noise_n1)).inactive(ac).cstyle("width: 110px;").align("flex-end").node(),
            ),
        ))
        .child(
            div("grid-row-start: span 2; display: flex; justify-content: center; align-items: center;").child(
                div(&format!(" display: flex; justify-content: center; align-items: center; visibility: {};", nv)).child(
                    conditional(
                        Some(98),
                        Some(40),
                        c.to_inactive,
                        vec![],
                        vec![button("CANCEL<br />NOISE").node()],
                    ),
                ),
            ),
        )
        .child(div_c(cell, "margin-right: 15px; margin-bottom: 15px; width: 125px;").child(span_c("mfd-label", "ACCEL")))
        .child(div("margin-bottom: 15px;").child(
            field(alt_fl(m.accel_alt)).inactive(ac).pilot(m.accel_alt_pilot).cstyle("width: 150px;").align("flex-end").node(),
        ))
        .child(div("").child(arrow_cell("SPD", cell, &format!("{} visibility: {};", mb, nv))))
        .child(div("").child(
            div(&format!("margin-bottom: 15px; visibility: {};", nv)).child(
                field(speed_knots(m.noise_speed)).inactive(ac).cstyle("width: 110px;").align("flex-end").node(),
            ),
        ))
        .child(
            div_c(cell, "margin-right: 15px; margin-bottom: 15px; width: 150px;")
                .child(Node::el("span", "mfd-label").style(&format!("display: {};", spd_lim_disp)).text("CLB SPD LIM"))
                .child(Node::el("span", "mfd-label").style(&format!("display: {};", noise_end_disp)).text("NOISE END")),
        )
        .child(
            div("grid-column-start: span 4; width: 300px;")
                .child(
                    div(&format!(
                        "grid-row-start: span 3; display: {}; justify-content: flex-start; align-items: center;",
                        spd_lim_disp
                    ))
                    .child(kt_value(m.sec_active, &num_text(m.speed_constraint_speed), "KT"))
                    .child(value_span(m.sec_active, "/"))
                    .child(
                        label_value("")
                            .child(span_c("mfd-label-unit mfd-unit-leading", "FL"))
                            .child(value_span(m.sec_active, &reason)),
                    ),
                )
                .child(div(&format!("display: {};", noise_end_disp)).child(
                    field(alt_fl(m.noise_end_altitude)).inactive(ac).cstyle("width: 150px;").align("flex-end").key("END").node(),
                )),
        )
}

fn clb_tab(c: &Ctx) -> Node {
    let m = c.m;
    let sec = m.sec_active;
    let fp = m.active_flight_phase;
    let eo = m.eo_active;
    let green1 = (fp >= PH_CLIMB && fp <= PH_DESCENT && !eo) || (fp < PH_CLIMB && m.climb_presel_speed.is_some());
    let amber1 = fp >= PH_CLIMB && fp <= PH_DESCENT && eo;
    let green2 = (fp >= PH_CLIMB && fp <= PH_DESCENT) || (fp < PH_CLIMB && m.climb_presel_speed.is_none());
    let derated = ["NONE", "01", "02", "03", "04", "05"];

    let head = div("display: flex; justify-content: space-between;")
        .child(
            label_value("margin-bottom: 15px;")
                .child(ci_header_mode_dropdown(c, "clbCostIndexModeDropdown", "clbCostIndexMode"))
                .child(label_sr("CI"))
                .child(ci_field(c)),
        )
        .child(
            label_value("").child(label_sr("DERATED CLB")).child(
                Dropdown::new(&format!("{}_MFD_deratedClbDropdown", c.side), &derated, m.climb_derated)
                    .inactive(true)
                    .cstyle("width: 125px;")
                    .digits(4)
                    .open(c.open("clbDerated"))
                    .node(),
            ),
        );

    let spd_unit2 = unit_if(&m.clb_spd2, "KT");
    let spd_unit3 = unit_if(&m.clb_spd3, "KT");
    let pad = "padding: 5px 15px 5px 15px;";
    let grid = div_c("mfd-fms-perf-clb-grid", "")
        .child(div_c(&format!("{} br", CELL), "").child(div_c("mfd-label", "").text("MODE")))
        .child(div_c(CELL, "").child(div_c("mfd-label", "").text("SPD")))
        .child(div_c(CELL, "").child(div_c("mfd-label", "").text("MACH")))
        .child(
            div_c(CELL, "").child(
                div(&format!("visibility: {};", c.vis))
                    .child(div_c("mfd-label", "").text("PRED TO "))
                    .child(
                        field(altitude_or_fl(m.climb_pred_ref, Some(&c.trans)))
                            .inactive(c.clb_inactive)
                            .pilot(m.climb_pred_ref_pilot)
                            .cstyle("width: 150px; margin-left: 15px;")
                            .align("flex-end")
                            .node(),
                    ),
            ),
        )
        .child(
            div_c(PRESEL_CELL, "").child(
                Node::el("div", "mfd-label")
                    .class_if("green", green1)
                    .class_if("amber", amber1)
                    .class_if("biggest", c.in_flight)
                    .text(os(&m.clb_mode1)),
            ),
        )
        .child(div_c(CELL, "").child(conditional(
            None,
            None,
            c.not_yet_climb,
            vec![field(speed_knots(m.climb_presel_speed)).inactive(c.at_or_after_climb).align("flex-end").key("PRESEL").node()],
            vec![kt_value(sec, os(&m.clb_spd1), "KT")],
        )))
        .child(div_c(CELL, "").child(value_span(sec, os(&m.clb_mach1))))
        .child(div_c(CELL, "").child(value_span(sec, os(&m.clb_pred1))))
        .child(
            div_c(PRESEL_CELL, "").child(
                Node::el("div", "mfd-label").class_if("green", green2).text(os(&m.clb_mode2)),
            ),
        )
        .child(div_c(CELL, "").child(kt_value(sec, os(&m.clb_spd2), spd_unit2)))
        .child(div_c(CELL, "").child(value_span(sec, os(&m.clb_mach2))))
        .child(div_c(CELL, "").child(value_span(sec, os(&m.clb_pred2))))
        .child(
            div_c(&format!("{} br", CELL), &format!("justify-content: flex-end; {}", pad))
                .child(div_c("mfd-label", "").text(os(&m.clb_mode3))),
        )
        .child(div_c(CELL, pad).child(kt_value(sec, os(&m.clb_spd3), spd_unit3)))
        .child(div_c(CELL, pad).child(value_span(sec, os(&m.clb_mach3))))
        .child(div_c(CELL, pad))
        .child(div("border-right: 1px solid lightgrey; height: 40px;"))
        .child(div(""))
        .child(div(""))
        .child(div(""));

    let noise_btn_disp = if m.noise_enabled { "none" } else { "flex" };
    let footer = div_c(
        "mfd-fms-perf-to-thrred-noise-grid-cell",
        "margin: 5px 2px 3px 2px;",
    )
    .child(div(&format!("display: {};", noise_btn_disp)).child(conditional(
        Some(98),
        Some(40),
        c.to_inactive,
        vec![],
        vec![button("NOISE").disabled(true).node()],
    )))
    .child(
        label_value("margin-left: 50px;").child(label_sr("TRANS")).child(
            field(altitude(m.trans_alt)).pilot(!m.trans_alt_from_database).cstyle("width: 150px;").align("flex-end").node(),
        ),
    )
    .child(button("SPD CSTR").disabled(c.clb_inactive).node());

    tab_page(
        m.selected_tab == 1,
        "padding-top: 0px; padding-left: 0px; ",
        vec![head, grid, clb_noise_grid(c), div("flex-grow: 1;"), footer],
    )
}

fn dest_block(c: &Ctx, button_row: Node) -> Node {
    let m = c.m;
    div_c("mfd-fms-perf-crz-dest", "")
        .child(span_c("mfd-label bigger", "DEST"))
        .child(Node::el("span", "mfd-label").class_if("green", c.mandatory_and_active).class_if("bigger", true).text(&m.dest_ident))
        .child(Node::el("span", "mfd-label").class_if("green", c.mandatory_and_active).class_if("bigger", true).text(&m.dest_eta))
        .child(
            label_value("")
                .child(
                    Node::el("span", "mfd-value")
                        .class_if("sec", m.sec_active)
                        .class_if("amber", m.dest_efob_below_min && m.loaded_fp_index == FP_ACTIVE)
                        .text(&weight_text(m.dest_efob_kg, m.metric)),
                )
                .child(unit_trailing(weight_unit_text(m.metric))),
        )
        .child(button_row)
}

fn crz_pred_cell(c: &Ctx) -> Node {
    let m = c.m;
    let sec = m.sec_active;
    let (step, td, ahead, drift) = match m.crz_pred.as_deref() {
        Some("td") => ("none", "block", "none", "none"),
        Some("ahead") => ("none", "none", "block", "none"),
        Some("step") => ("block", "none", "none", "none"),
        _ => ("", "", "", ""),
    };
    let st = |d: &str| if d.is_empty() { None } else { Some(format!("display: {};", d)) };
    let wrap = |d: &str, n: Node| {
        let mut w = Node::new("div");
        if let Some(s) = st(d) {
            w = w.style(&s);
        }
        w.child(n)
    };
    let _ = wrap;
    let mk = |d: &str, kids: Vec<Node>| {
        let mut w = Node::new("div");
        if let Some(s) = st(d) {
            w = w.style(&s);
        }
        w.children(kids)
    };
    let target = num_text(m.crz_pred_altitude_target);
    div_c(CELL, "flex-direction: column;")
        .child(mk(
            step,
            vec![
                div(ROW_CENTER)
                    .child(div_c("mfd-label mfd-spacing-right", "").text("AT"))
                    .child(Node::el("div", "mfd-value").class_if("sec", sec).class_if("bigger", true).text(&m.crz_pred_waypoint)),
                div(ROW_CENTER).child(div_c("mfd-label mfd-spacing-right", "").text("STEP TO")).child(
                    label_value("")
                        .child(span_c("mfd-label-unit mfd-unit-leading", "FL"))
                        .child(span_c("mfd-value bigger", &target)),
                ),
            ],
        ))
        .child(mk(
            td,
            vec![div(ROW_CENTER)
                .child(div_c("mfd-label mfd-spacing-right", "").text("PRED TO"))
                .child(label_value("").child(span_c("mfd-value bigger", "T/D")))],
        ))
        .child(mk(
            ahead,
            vec![div(ROW_CENTER).child(div_c("mfd-label mfd-spacing-right green", "").text("STEP AHEAD"))],
        ))
        .child(mk(
            drift,
            vec![
                div(ROW_CENTER).child(div_c("mfd-label mfd-spacing-right", "").text("DRIFT DOWN")),
                div(ROW_CENTER).child(div_c("mfd-label mfd-spacing-right", "").text("TO")).child(
                    label_value("")
                        .child(span_c("mfd-label-unit mfd-unit-leading", "FL"))
                        .child(Node::el("span", "mfd-value").class_if("sec", sec).class_if("bigger", true).text(&target)),
                ),
            ],
        ))
}

fn crz_tab(c: &Ctx) -> Node {
    let m = c.m;
    let sec = m.sec_active;
    let fp = m.active_flight_phase;
    let eo = m.eo_active;
    let presel = m.cruise_presel_speed;
    let g1 = (fp >= PH_CLIMB && fp <= PH_DESCENT && !eo) || (fp < PH_CLIMB && presel.is_some());
    let a1 = fp >= PH_CLIMB && fp <= PH_DESCENT && eo;
    let g2 = fp < PH_CLIMB && m.managed_speed_active && presel.is_none();
    let presel_knots = presel.filter(|v| *v > 1.0);
    let presel_mach = presel.filter(|v| *v < 1.0);
    let pad = "padding: 5px 15px 5px 15px;";
    let nb = "border-bottom: none; padding: 5px;";
    let nbr = "border-bottom: none; justify-content: flex-end; padding: 5px;";

    let head = div("display: flex; justify-content: space-between;").child(
        label_value("")
            .child(ci_header_mode_dropdown(c, "crzCostIndexModeDropdown", "crzCostIndexMode"))
            .child(label_sr("CI"))
            .child(ci_field(c)),
    );

    let grid = div_c("mfd-fms-perf-crz-grid", "")
        .child(div_c(&format!("{} br", CELL), "").child(div_c("mfd-label", "").text("MODE")))
        .child(div_c(CELL, "").child(div_c("mfd-label", "").text("MACH")))
        .child(div_c(CELL, "").child(div_c("mfd-label", "").text("SPD")))
        .child(crz_pred_cell(c))
        .child(
            div_c(PRESEL_CELL, "").child(
                Node::el("div", "mfd-label")
                    .class_if("green", g1)
                    .class_if("amber", a1)
                    .class_if("biggest", c.in_flight)
                    .text(os(&m.crz_mode1)),
            ),
        )
        .child(div_c(CELL, "").child(conditional(
            None,
            None,
            c.not_yet_cruise,
            vec![field(speed_mach(presel_mach)).align("flex-end").key("PRESEL MACH").node()],
            vec![label_value("").child(span_c("mfd-value", os(&m.crz_mach1)))],
        )))
        .child(div_c(CELL, "").child(conditional(
            None,
            None,
            c.not_yet_cruise,
            vec![field(speed_knots(presel_knots)).align("flex-end").key("PRESEL SPD").node()],
            vec![kt_value(sec, os(&m.crz_spd1), "KT")],
        )))
        .child(
            div_c(CELL, "")
                .child(value_span(sec, os(&m.crz_pred1)))
                .child(unit_trailing(nm_if(&m.crz_pred1))),
        )
        .child(div_c(PRESEL_CELL, "").child(Node::el("div", "mfd-label").class_if("green", g2).text(os(&m.crz_mode2))))
        .child(div_c(CELL, "").child(value_span(sec, os(&m.crz_mach2))))
        .child(div_c(CELL, "").child(kt_value(sec, os(&m.crz_spd2), unit_if(&m.crz_spd2, "KT"))))
        .child(
            div_c(CELL, "")
                .child(value_span(sec, os(&m.crz_pred2)))
                .child(unit_trailing(nm_if(&m.crz_pred2))),
        )
        .child(
            div_c(&format!("{} br", CELL), &format!("justify-content: flex-end; {}", pad))
                .child(div_c("mfd-label", "").text(os(&m.crz_mode3))),
        )
        .child(div_c(CELL, pad).child(value_span(sec, os(&m.crz_mach3))))
        .child(div_c(CELL, pad).child(kt_value(sec, os(&m.crz_spd3), unit_if(&m.crz_spd3, "KT"))))
        .child(div_c(CELL, pad))
        .child(div_c(&format!("{} br", CELL), nbr).child(div_c("mfd-label", "").text("LRC")))
        .child(div_c(CELL, nb).child(value_span(sec, ".84")))
        .child(div_c(CELL, nb).child(label_value("").child(value_span(sec, "---")).child(unit_trailing(" "))))
        .child(div(""))
        .child(div_c(&format!("{} br", CELL), nbr).child(div_c("mfd-label", "").text("MAX TURB")))
        .child(div_c(CELL, nb).child(value_span(sec, ".85")))
        .child(div_c(CELL, nb).child(label_value("").child(value_span(sec, "---")).child(unit_trailing(" "))))
        .child(div(""));

    let buttons = div("display: flex; flex-direction: row;")
        .child(button("CMS").disabled(true).style("margin-right: 10px;").node())
        .child(button("STEP ALTs").disabled(c.crz_inactive).node());

    tab_page(
        m.selected_tab == 2,
        "padding-top: 0px; padding-left: 0px; ",
        vec![head, grid, div("flex-grow: 1;"), dest_block(c, buttons)],
    )
}

fn des_tab(c: &Ctx) -> Node {
    let m = c.m;
    let sec = m.sec_active;
    let fp = m.active_flight_phase;
    let g1 = fp >= PH_CLIMB && fp <= PH_DESCENT;
    let g2 = fp < PH_CLIMB && m.managed_speed_active;
    let trans_fl = TransAlt { value_ft: m.trans_fl, is_fl: true };

    let head = div("display: flex; justify-content: space-between;")
        .child(
            label_value("")
                .child(ci_header_mode_dropdown(c, "desCostIndexModeDropdown", "desCostIndexMode"))
                .child(label_sr("CI"))
                .child(
                    field(cost_index(m.cost_index))
                        .disabled(c.ci_disabled())
                        .inactive(c.crz_inactive)
                        .cstyle("width: 75px;")
                        .align("center")
                        .node(),
                ),
        )
        .child(
            label_value(&format!("padding: 15px; visibility: {};", c.vis))
                .child(label_sr("DES CABIN RATE"))
                .child(
                    field(descent_rate(m.descent_cabin_rate))
                        .inactive(c.crz_inactive)
                        .cstyle("width: 175px;")
                        .align("flex-end")
                        .node(),
                ),
        );

    let grid = div_c("mfd-fms-perf-crz-grid", "")
        .child(div_c(&format!("{} br", CELL), "").child(div_c("mfd-label", "").text("MODE")))
        .child(div_c(CELL, "").child(div_c("mfd-label", "").text("MACH")))
        .child(div_c(CELL, "").child(div_c("mfd-label", "").text("SPD")))
        .child(
            div_c(CELL, "").child(
                div(&format!("visibility: {};", c.vis))
                    .child(div_c("mfd-label", "").text("PRED TO "))
                    .child(
                        field(altitude_or_fl(m.des_pred_ref, Some(&trans_fl)))
                            .disabled(c.not_in_descent)
                            .cstyle("width: 150px; margin-left: 15px;")
                            .align("flex-end")
                            .node(),
                    ),
            ),
        )
        .child(
            div_c(PRESEL_CELL, "").child(
                Node::el("div", "mfd-label")
                    .class_if("green", g1)
                    .class_if("biggest", c.in_flight)
                    .text(os(&m.des_mode1)),
            ),
        )
        .child(div_c(CELL, "").child(conditional(
            None,
            None,
            c.not_yet_descent,
            vec![label_value("").child(span_c("mfd-value", os(&m.des_mach1)))],
            vec![field(speed_mach(m.des_managed_mach)).align("flex-end").key("DES MACH").node()],
        )))
        .child(div_c(CELL, "").child(conditional(
            None,
            None,
            c.not_yet_descent,
            vec![field(speed_knots(m.des_managed_spd)).align("flex-end").key("DES SPD").node()],
            vec![label_value("").child(span_c("mfd-value", os(&m.des_spd1))).child(unit_trailing("KT"))],
        )))
        .child(div_c(CELL, "").child(value_span(sec, "--:-- ----")))
        .child(div_c(PRESEL_CELL, "").child(Node::el("div", "mfd-label").class_if("green", g2).text(os(&m.des_mode2))))
        .child(div_c(CELL, "").child(value_span(sec, os(&m.des_mach2))))
        .child(div_c(CELL, "").child(kt_value(sec, os(&m.des_spd2), unit_if(&m.des_spd2, "KT"))))
        .child(
            div_c(CELL, "")
                .child(value_span(sec, os(&m.des_pred2)))
                .child(unit_trailing(nm_if(&m.des_pred2))),
        )
        .child(div_c(&format!("{} br", CELL), "border-bottom: none; justify-content: flex-end; height: 75px;"))
        .child(div_c(CELL, "border-bottom: none; padding: 5px;"))
        .child(div_c(CELL, "border-bottom: none; padding: 5px;"))
        .child(div_c(CELL, "border-bottom: none; padding: 5px;"));

    let buttons = div("display: flex; flex-direction: row;").child(button("SPD CSTR").disabled(c.des_inactive).node());

    tab_page(
        m.selected_tab == 3,
        "padding-top: 0px; padding-left: 0px; ",
        vec![head, grid, div("flex-grow: 1;"), dest_block(c, buttons)],
    )
}

fn appr_tab(c: &Ctx) -> Node {
    let m = c.m;
    let sec = m.sec_active;
    let row = "display: flex; flex-direction: row;";
    let head = div("display: flex; justify-content: space-between; border-bottom: 1px solid lightgrey;")
        .child(label_value("padding: 15px;").child(label_sr("APPR")).child(value_span(sec, &m.appr_ident)))
        .child(label_value("padding: 15px;").child(value_span(sec, &m.dest_ident)))
        .child(
            label_value("padding: 15px;")
                .child(label_sr("LW"))
                .child(value_span(sec, &weight_text(m.landing_weight_kg, m.metric)))
                .child(unit_trailing(weight_unit_text(m.metric))),
        );

    let head_wind_label = if m.appr_head_wind.map(|v| v < 0.0).unwrap_or(false) { "TL" } else { "HD" };
    let wind_text = |v: Option<f64>| match v {
        Some(v) => format!("{:0>3}", fixed0(v.abs())),
        None => "---".to_string(),
    };

    let weather = div("border: 1px solid lightgrey; display: flex; flex-direction: column; margin: 20px 40px 20px 0px; padding: 15px;")
        .child(
            div(row)
                .child(span_c("mfd-label mfd-spacing-right perf-appr-weather", "MAG WIND"))
                .child(
                    div("border: 1px solid lightgrey; display: flex; flex-direction: row; padding: 2px;")
                        .child(
                            field(wind_direction(m.appr_wind_direction))
                                .disabled(m.dest_airport_missing)
                                .align("center")
                                .key("WIND DIR")
                                .node(),
                        )
                        .child(
                            field(wind_speed(m.appr_wind_magnitude))
                                .disabled(m.dest_airport_missing)
                                .cstyle("margin-left: 10px;")
                                .align("center")
                                .key("WIND SPD")
                                .node(),
                        ),
                ),
        )
        .child(
            div("display: flex; flex-direction: row; margin-top: 15px;")
                .child(
                    label_value("padding: 15px;")
                        .child(label_sr(head_wind_label))
                        .child(value_span(sec, &wind_text(m.appr_head_wind)))
                        .child(unit_trailing("KT")),
                )
                .child(
                    label_value("padding: 15px;")
                        .child(label_sr("CROSS"))
                        .child(value_span(sec, &wind_text(m.appr_cross_wind)))
                        .child(unit_trailing("KT")),
                ),
        )
        .child(
            div("display: flex; flex-direction: row; margin-top: 20px;")
                .child(span_c("mfd-label mfd-spacing-right perf-appr-weather", "OAT"))
                .child(
                    field(temperature(m.appr_temperature))
                        .mandatory(m.appr_params_mandatory)
                        .cstyle("width: 125px;")
                        .align("flex-end")
                        .disabled(m.dest_airport_missing)
                        .node(),
                ),
        )
        .child(
            div("display: flex; flex-direction: row; margin-top: 15px;")
                .child(span_c("mfd-label mfd-spacing-right perf-appr-weather", "QNH"))
                .child(
                    field(qnh(m.appr_qnh, m.appr_qnh_hpa))
                        .mandatory(m.appr_params_mandatory)
                        .cstyle("width: 125px;")
                        .align("flex-end")
                        .disabled(m.dest_airport_missing)
                        .node(),
                ),
        );

    let radio_text = if m.precision_approach { "RADIO" } else { "-----" };
    let minimum = div_c("mfd-fms-perf-appr-min-container", "")
        .child(span_c("mfd-label mfd-spacing-right mfd-fms-perf-appr-min-label", "MINIMUM"))
        .child(
            div(row)
                .child(span_c("mfd-label mfd-spacing-right perf-appr-weather", "BARO"))
                .child(field(altitude(m.appr_baro_min)).cstyle("width: 150px;").align("flex-end").node()),
        )
        .child(
            div("display: flex; flex-direction: row; margin-top: 15px;")
                .child(span_c("mfd-label mfd-spacing-right perf-appr-weather", radio_text))
                .child(conditional(
                    None,
                    None,
                    m.precision_approach,
                    vec![field(radio_altitude(m.appr_radio_min)).cstyle("width: 150px;").align("flex-end").node()],
                    vec![],
                )),
        );

    let left = div("flex: 5; display: flex; flex-direction: column;").child(weather).child(minimum);

    let speeds = div("display: flex; flex-direction: column; align-items: center; margin-top: 30px;")
        .child(
            label_value("")
                .child(Node::el("span", "mfd-fms-perf-appr-flap-speeds").child(circle_svg()))
                .child(value_span(sec, &fmc_format_value(m.appr_green_dot)))
                .child(unit_trailing("KT")),
        )
        .child(
            label_value("")
                .child(span_c("mfd-label mfd-spacing-right mfd-fms-perf-appr-flap-speeds", "S"))
                .child(value_span(sec, &fmc_format_value(m.appr_slat_retraction)))
                .child(unit_trailing("KT")),
        )
        .child(
            label_value("")
                .child(span_c("mfd-label mfd-spacing-right mfd-fms-perf-appr-flap-speeds", "F"))
                .child(value_span(sec, &fmc_format_value(m.appr_flap_retraction)))
                .child(unit_trailing("KT")),
        )
        .child(
            label_value(&format!("padding-top: 15px; visibility: {};", c.vis))
                .child(span_c("mfd-label mfd-spacing-right mfd-fms-perf-appr-flap-speeds", "VREF"))
                .child(span_c("mfd-value", &fmc_format_value(m.appr_vref)))
                .child(unit_trailing("KT")),
        );

    let conf = div_c("mfd-fms-perf-appr-conf-box", "")
        .child(radio_wrap(&["CONF 3", "FULL"], &format!("{}_MFD_apprFlapsSettingsRadio", c.side), 15, Some(if m.appr_flaps3 { 0 } else { 1 }), &[]))
        .child(
            label_value("margin-top: 10px;")
                .child(label_sr("VLS"))
                .child(value_span(sec, &fmc_format_value(m.appr_vls)))
                .child(unit_trailing("KT")),
        );

    let vapp = div_c("mfd-fms-perf-appr-vapp-box", "").child(
        div("display: flex; flex-direction: row; justify-content: center; justify-self; center;")
            .child(Node::el("span", "mfd-label mfd-spacing-right").style("text-align: right; align-self: center;").text("VAPP"))
            .child(
                field(speed_knots(m.appr_vapp)).pilot(m.appr_vapp_pilot).align("flex-end").node(),
            ),
    );

    let right = div("flex: 4; display: flex; flex-direction: column;").child(speeds).child(conf).child(vapp);

    let trans = div_c("mfd-fms-perf-appr-trans-vertdev", "")
        .child(
            label_value("")
                .child(
                    Node::el("span", "mfd-label mfd-spacing-right")
                        .style("width: 125px; text-align: right; align-self: center; padding-left: 20px;")
                        .text("TRANS"),
                )
                .child(
                    field(flight_level(m.trans_fl))
                        .pilot(!m.trans_fl_from_database)
                        .cstyle("width: 110px;")
                        .align("flex-start")
                        .node(),
                ),
        )
        .child(
            label_value(&format!("padding: 15px; visibility: {};", c.vis))
                .child(label_sr("VERT DEV"))
                .child(span_c("mfd-value", &m.appr_vertical_deviation)),
        );

    tab_page(
        m.selected_tab == 4,
        "",
        vec![head, div(row).child(left).child(right), div("flex-grow: 1;"), trans],
    )
}

fn ga_tab(c: &Ctx) -> Node {
    let m = c.m;
    let sec = m.sec_active;
    let trans_ft = m.trans_fl.map(|v| v * 100.0);
    let ta = TransAlt { value_ft: trans_ft, is_fl: false };
    let af = |v: Option<f64>| altitude_or_fl(v, Some(&ta));
    let top = div("margin: 60px 0px 100px 200px; display: flex; flex-direction: column;")
        .child(kt_after(sec, "F", m.appr_flap_retraction))
        .child(kt_after(sec, "S", m.appr_slat_retraction))
        .child(
            label_value("")
                .child(Node::new("span").style("margin-right: 15px; text-align: right;").child(circle_svg()))
                .child(value_span(sec, &fmc_format_value(m.appr_green_dot)))
                .child(unit_trailing("KT")),
        );
    let mid = div("display: flex; flex-direction: column;")
        .child(
            div("display: flex; flex-direction: row;")
                .child(div_c("mfd-fms-perf-appr-thrred-accel", "").child(span_c("mfd-label", "THR RED")))
                .child(div("margin-bottom: 15px;").child(
                    field(af(m.missed_thr_red_alt)).pilot(m.missed_thr_red_pilot).cstyle("width: 150px;").align("flex-end").node(),
                )),
        )
        .child(
            div("display: flex; flex-direction: row;")
                .child(div_c("mfd-fms-perf-appr-thrred-accel", "").child(span_c("mfd-label", "ACCEL")))
                .child(div("margin-bottom: 15px;").child(
                    field(af(m.missed_accel_alt)).pilot(m.missed_accel_pilot).cstyle("width: 150px;").align("flex-end").node(),
                ))
                .child(div_c("mfd-fms-perf-appr-eo-accel", "").child(span_c("mfd-label", "EO ACCEL")))
                .child(div("margin-bottom: 15px;").child(
                    field(af(m.missed_eo_accel_alt)).pilot(m.missed_eo_accel_pilot).cstyle("width: 150px;").align("flex-end").node(),
                )),
        );
    let trans = label_value("")
        .child(Node::el("span", "mfd-label mfd-spacing-right").style("width: 150px; text-align: right;").text("TRANS"))
        .child(value_span(sec, &fmc_format_value(trans_ft)))
        .child(unit_trailing("FT"));
    tab_page(m.selected_tab == 5, "", vec![top, mid, div("flex-grow: 1;"), trans])
}

fn footer(c: &Ctx) -> Node {
    let m = c.m;
    let fp = m.active_flight_phase;
    let appr_vis = if c.mandatory_and_active && (fp == PH_CLIMB || fp == PH_CRUISE || fp == PH_DESCENT || fp == 6) {
        "visible"
    } else {
        "hidden"
    };
    let eo_vis = if m.eo_active { "visible" } else { "hidden" };
    let star = |a: &str, b: &str| {
        button_node(two_line_star_label_spaced(a, b)).style("color: #e68000; padding-right: 2px;").node()
    };
    div_c("mfd-fms-perf-appr-footer", "")
        .child(
            div("")
                .child(button("RETURN").style("margin-right: 5px;").node())
                .child(confirmation_dialog(
                    m.approach_dialog_visible,
                    "width: 280px; height: 165px; bottom: -6px; left: -5px;",
                    true,
                    vec![],
                    "ACTIVATE APPR ?",
                ))
                .child(confirmation_dialog(
                    m.clear_eo_dialog_visible,
                    "width: 450px; height: 165px; bottom: -6px; left: -5px; text-align: center;",
                    true,
                    vec![
                        Node::new("br"),
                        Node::new("br"),
                        Node::new("span").style("color: white;").text("(BACK TO ALL ENGs COMPUTATION)"),
                    ],
                    "CLEAR EO ?",
                )),
        )
        .child(div(&format!("margin-right: 5px; visibility: {};", appr_vis)).child(star("ACTIVATE", "APPR")))
        .child(div(&format!("visibility: {};", c.vis)).child(button("POS MONITOR").cstyle("margin-right: 5px;").node()))
        .child(div(&format!("margin-right: 5px; visibility: {};", eo_vis)).child(star("CLEAR", "EO")))
        .child(div("flex: 1"))
}

pub fn perf_page(m: &PerfModel) -> Node {
    let c = Ctx::new(m);
    let titles = ["T.O", "CLB", "CRZ", "DES", "APPR", "GA"];
    Node::el("div", "mfd-page-container")
        .child(top_row(&c))
        .child(
            Node::el("div", "mfd-top-tab-navigator-container")
                .child(tab_bar(&titles, m.selected_tab))
                .child(to_tab(&c))
                .child(clb_tab(&c))
                .child(crz_tab(&c))
                .child(des_tab(&c))
                .child(appr_tab(&c))
                .child(ga_tab(&c)),
        )
        .child(footer(&c))
}

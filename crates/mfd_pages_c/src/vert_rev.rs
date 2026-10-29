use crate::radio::{radio_button_group, RadioColor};
use crate::w::*;
use crate::x::*;
use mfd_pages_a::widgets as aw;
use mfd_tree::Node;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VertRevEditing {
    #[default]
    None,
    SpdWpt,
    AltWpt,
    StepWpt(usize),
}

#[derive(Clone, Debug)]
pub struct StepLine {
    pub visible: bool,
    pub wpt: String,
    pub alt: String,
    pub message: String,
    pub predictions_visible: bool,
    pub dist: String,
    pub time: String,
}

impl Default for StepLine {
    fn default() -> Self {
        StepLine {
            visible: false,
            wpt: "-------".into(),
            alt: "-----".into(),
            message: String::new(),
            predictions_visible: false,
            dist: "---".into(),
            time: "--:--".into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct VertRevModel {
    pub selected_tab: usize,
    pub cap_or_fo: String,
    pub tmpy_active: bool,
    pub sec_active: bool,
    pub waypoints: Vec<String>,
    pub selected_waypoint: String,
    pub editing: VertRevEditing,
    pub ett: String,
    pub constraint_type: String,
    pub clb_des_visible: bool,
    pub speed_message: String,
    pub spd_cstr: String,
    pub spd_cstr_disabled: bool,
    pub spd_clb_des_selected: Option<usize>,
    pub delete_spd_cstr_disabled: bool,
    pub speed_limit_des: bool,
    pub speed_limit_pilot: bool,
    pub speed_limit_speed: String,
    pub speed_limit_alt: String,
    pub delete_spd_limit_disabled: bool,
    pub altitude_message: String,
    pub alt_cstr: String,
    pub alt_cstr_disabled: bool,
    pub alt_option: Option<usize>,
    pub alt_clb_des_selected: Option<usize>,
    pub alt_window_visible: Option<bool>,
    pub alt_window: (String, String, String),
    pub delete_alt_cstr_disabled: bool,
    pub step_disabled_reason: Option<String>,
    pub crz_fl: String,
    pub preflight: bool,
    pub step_lines: [StepLine; 5],
    pub step_not_allowed_at: String,
    pub step_scroll_down_disabled: bool,
    pub step_scroll_up_disabled: bool,
}

impl Default for VertRevModel {
    fn default() -> Self {
        VertRevModel {
            selected_tab: 0,
            cap_or_fo: "CAPT".into(),
            tmpy_active: false,
            sec_active: false,
            waypoints: vec![],
            selected_waypoint: "-------".into(),
            editing: VertRevEditing::None,
            ett: "--:--:--".into(),
            constraint_type: String::new(),
            clb_des_visible: false,
            speed_message: String::new(),
            spd_cstr: "---".into(),
            spd_cstr_disabled: true,
            spd_clb_des_selected: None,
            delete_spd_cstr_disabled: true,
            speed_limit_des: false,
            speed_limit_pilot: false,
            speed_limit_speed: "---".into(),
            speed_limit_alt: "-----".into(),
            delete_spd_limit_disabled: true,
            altitude_message: String::new(),
            alt_cstr: "-----".into(),
            alt_cstr_disabled: true,
            alt_option: None,
            alt_clb_des_selected: None,
            alt_window_visible: None,
            alt_window: (String::new(), "EMPTY".into(), String::new()),
            delete_alt_cstr_disabled: true,
            step_disabled_reason: None,
            crz_fl: "---".into(),
            preflight: true,
            step_lines: Default::default(),
            step_not_allowed_at: String::new(),
            step_scroll_down_disabled: true,
            step_scroll_up_disabled: true,
        }
    }
}

pub fn alt_window(alt1: f64, alt2: f64, alt1_fl: bool, alt2_fl: bool) -> (String, String, String) {
    let f = |a: f64, fl: bool| format!("{:03}", (if fl { a / 100.0 } else { a }).round() as i64);
    (
        if alt1_fl && !alt2_fl { "FT".into() } else { String::new() },
        format!("{}-{}", f(alt2, alt2_fl), f(alt1, alt1_fl)),
        if alt1_fl { "FL".into() } else { "FT".into() },
    )
}

pub struct Fld<'a> {
    pub text: &'a str,
    pub trailing: &'a str,
    pub align: &'a str,
    pub style: &'a str,
    pub disabled: bool,
    pub computed: bool,
    pub tmpy: bool,
    pub root: &'a str,
    pub digits: usize,
}

pub fn fld(f: Fld) -> Node {
    let mut spec = aw::InputSpec::new(f.text);
    spec.trailing_unit = f.trailing.to_string();
    spec.align = f.align.to_string();
    spec.container_style = f.style.to_string();
    spec.root_class = f.root.to_string();
    spec.max_digits = f.digits;
    spec.state.computed_by_fms = f.computed;
    spec.state.tmpy = f.tmpy;
    spec.state.disabled = f.disabled;
    aw::input_field(spec)
}

fn val_cls(m: &VertRevModel, tmpy: bool) -> Node {
    Node::el("span", "mfd-value").class_if("sec", m.sec_active).class_if("tmpy", tmpy && m.tmpy_active)
}

pub fn wpt_dropdown(m: &VertRevModel, prefix: &str, text: &str, editing: bool) -> Node {
    waypoint_dropdown(&format!("{}_MFD_{}", m.cap_or_fo, prefix), &m.waypoints, text, editing)
}

pub fn waypoint_dropdown(id_prefix: &str, waypoints: &[String], text: &str, editing: bool) -> Node {
    let mut input = aw::InputSpec::new(if text.is_empty() { "------" } else { text });
    if editing {
        input.state = aw::InputState::focused();
    }
    aw::dropdown_menu(aw::DropdownSpec {
        id_prefix: id_prefix.to_string(),
        input,
        options: waypoints.to_vec(),
        container_style: "width: 175px;".into(),
        align: "flex-start".into(),
        digits: 7,
        open: editing,
        ..Default::default()
    })
}

fn delete_label(top: &str, bottom: &str, fr: bool) -> Node {
    let outer = if fr {
        Node::el("div", "fr").style("justify-content: space-between;")
    } else {
        Node::new("div").style("display: flex; flex-direction: row; justify-content: space-between;")
    };
    outer
        .child(
            Node::new("span")
                .style("text-align: center; vertical-align: center; margin-right: 10px;")
                .text(top)
                .child(br())
                .text(bottom),
        )
        .child(Node::new("span").style("display: flex; align-items: center; justify-content: center;").text("*"))
}

fn header_row(m: &VertRevModel, kind: &str, prefix: &str, editing: bool) -> Vec<Node> {
    let msg = if kind == "SPD" { &m.speed_message } else { &m.altitude_message };
    vec![
        Node::new("div").child(Node::el("span", "mfd-label biggest amber").text(msg)),
        Node::new("div")
            .style("display: flex; flex-direction: row; justify-content: center; align-items: center; margin-top: 25px;")
            .child(Node::el("span", "mfd-label biggest green mfd-spacing-right").text(&m.constraint_type))
            .child(Node::el("span", "mfd-label bigger mfd-spacing-right").text(&format!("{} CSTR AT ", kind)))
            .child(wpt_dropdown(m, prefix, &m.selected_waypoint, editing)),
    ]
}

fn clb_des(m: &VertRevModel, prefix: &str, selected: Option<usize>) -> Node {
    Node::el("div", "mfd-vert-rev-clbdes")
        .style(&format!("visibility: {};", if m.clb_des_visible { "visible" } else { "hidden" }))
        .child(radio_button_group(&format!("{}_MFD_{}", m.cap_or_fo, prefix), &["CLB CSTR", "DES CSTR"], selected, RadioColor::Amber, &[]))
}

fn rta_page(m: &VertRevModel) -> Node {
    Node::new("div")
        .style("display: flex; flex-direction: column; justify-content: center; align-items: center;")
        .child(Node::el("span", "mfd-label").text("NOT IMPLEMENTED"))
        .child(
            Node::new("div")
                .style("display: flex; flex-direction: row; margin-top: 20px; justify-content: center; align-items: center;")
                .child(Node::el("div", "mfd-label mfd-spacing-right").text("ETT"))
                .child(Node::new("div").child(fld(Fld {
                    text: &m.ett,
                    trailing: "",
                    align: "center",
                    style: "width: 175px;",
                    disabled: false,
                    computed: false,
                    tmpy: m.tmpy_active,
                    root: "",
                    digits: 6,
                }))),
        )
}

fn spd_page(m: &VertRevModel) -> Node {
    let spd_line = Node::el("div", "mfd-vert-rev-spd-cstr-line")
        .child(aw::keyed(
            fld(Fld {
                text: &m.spd_cstr,
                trailing: "KT",
                align: "flex-end",
                style: "",
                disabled: m.spd_cstr_disabled,
                computed: false,
                tmpy: m.tmpy_active,
                root: "",
                digits: 3,
            }),
            "SPD CSTR",
        ))
        .child(clb_des(m, "spdCstrClbDesRadioButtons", m.spd_clb_des_selected))
        .child(btn_node(delete_label("DELETE", "SPD CSTR", false), m.delete_spd_cstr_disabled, false, "adding-right: 2px;"));
    let lim = |text: &str, trailing: &str, digits: usize| {
        fld(Fld { text, trailing, align: "flex-end", style: "", disabled: false, computed: !m.speed_limit_pilot, tmpy: m.tmpy_active, root: "", digits })
    };
    Node::new("div")
        .style("display: flex; flex-direction: column; justify-content: center; align-items: center; margin-top: 15px;")
        .children(header_row(m, "SPD", "clbConstraintWptDropdown", m.editing == VertRevEditing::SpdWpt))
        .child(spd_line)
        .child(
            Node::el("span", "mfd-vert-rev-spd-lim-header mfd-label bigger mfd-spacing-right")
                .text(if m.speed_limit_des { "DES SPD LIMIT" } else { "CLB SPD LIMIT" }),
        )
        .child(
            Node::el("div", "mfd-vert-rev-spd-lim")
                .child(lim(&m.speed_limit_speed, "KT", 3))
                .child(Node::el("span", "mfd-label bigger").text("AT OR BELOW"))
                .child(lim(&m.speed_limit_alt, "FT", 5))
                .child(btn_node(delete_label("DELETE", "SPD LIMIT", false), m.delete_spd_limit_disabled, false, "adding-right: 2px;")),
        )
}

fn cms_page() -> Node {
    Node::new("div")
        .style("display: flex; flex-direction: column; justify-content: center; align-items: center;")
        .child(Node::el("span", "mfd-label").text("NOT IMPLEMENTED"))
}

fn alt_page(m: &VertRevModel) -> Node {
    let win = |n: Node| match m.alt_window_visible {
        Some(v) => n.style(if v { "visibility: visible;" } else { "visibility: hidden;" }),
        None => n,
    };
    let rb = Node::el("div", "mfd-vert-rev-alt-cstr-rb")
        .child(radio_button_group(
            &format!("{}_MFD_altCstrRadioButtons", m.cap_or_fo),
            &["AT", "AT OR ABOVE", "AT OR BELOW"],
            m.alt_option,
            if m.tmpy_active { RadioColor::Yellow } else { RadioColor::Cyan },
            &[m.alt_cstr_disabled; 3],
        ))
        .child(win(Node::el("div", "mfd-label bigger mfd-vert-rev-alt-window-label")).text("WINDOW"));
    let sel = Node::el("div", "mfd-vert-rev-alt-cstr-sel")
        .child(Node::new("div").class_if("invisible", m.alt_option.is_none()).child(aw::keyed(
            fld(Fld {
                text: &m.alt_cstr,
                trailing: "FT",
                align: "flex-end",
                style: "",
                disabled: m.alt_cstr_disabled,
                computed: false,
                tmpy: m.tmpy_active,
                root: "",
                digits: 5,
            }),
            "ALT CSTR",
        )))
        .child(
            win(Node::el("div", "mfd-vert-rev-alt-window-value"))
                .child(Node::el("span", "mfd-label-unit bigger mfd-unit-leading").text(&m.alt_window.0))
                .child(Node::el("span", "mfd-label green bigger").text(&m.alt_window.1))
                .child(Node::el("span", "mfd-label-unit bigger mfd-unit-trailing").text(&m.alt_window.2)),
        );
    let right = Node::el("div", "mfd-vert-rev-alt-right-container")
        .child(btn_node(delete_label("DELETE", "ALT CSTR", true), m.delete_alt_cstr_disabled, false, "adding-right: 2px;"))
        .child(clb_des(m, "altCstrClbDesRadioButtons", m.alt_clb_des_selected));
    Node::new("div")
        .style("display: flex; flex-direction: column; justify-content: center; align-items: center; margin-top: 15px;")
        .children(header_row(m, "ALT", "altConstraintWptDropdown", m.editing == VertRevEditing::AltWpt))
        .child(Node::el("div", "mfd-vert-rev-alt-cstr-line").child(rb).child(sel).child(right))
}

const HDR: &str = "align-self: flex-end; padding-bottom: 10px; border-bottom: 1px solid #777777;";

fn step_line(m: &VertRevModel, li: usize) -> Vec<Node> {
    let l = &m.step_lines[li];
    let vis = |v: bool| format!("visibility: {};", if v { "visible" } else { "hidden" });
    let has_msg = !l.message.is_empty();
    let shown = if has_msg { "flex" } else { "none" };
    let hidden = if has_msg { "none" } else { "flex" };
    vec![
        Node::el("div", "fc aic jcc").child(Node::new("div").style(&vis(l.visible)).child(wpt_dropdown(
            m,
            &format!("stepAltWpt{}", li),
            &l.wpt,
            m.editing == VertRevEditing::StepWpt(li),
        ))),
        Node::el("div", "fc aic jcc").style("border-right: 1px solid #777777;").child(Node::new("div").style(&vis(l.visible)).child(fld(Fld {
            text: &l.alt,
            trailing: "FT",
            align: "center",
            style: "width: 150px;",
            disabled: false,
            computed: false,
            tmpy: m.tmpy_active,
            root: "",
            digits: 5,
        }))),
        Node::el("div", "fr aic jcc")
            .style(&format!("display: {}; grid-column: span 2;", shown))
            .child(Node::el("span", "mfd-label").text(&l.message)),
        Node::el("div", "fr aic jcc").style(&format!("display: {}; justify-content: flex-end;", hidden)).child(
            Node::new("div")
                .style(&vis(l.predictions_visible))
                .child(val_cls(m, true).text(&l.dist))
                .child(Node::el("span", "mfd-label-unit mfd-unit-trailing").text("NM")),
        ),
        Node::el("div", "fr aic jcc")
            .style(&format!("display: {};", hidden))
            .child(val_cls(m, true).style(&vis(l.predictions_visible)).text(&l.time)),
    ]
}

fn step_page(m: &VertRevModel) -> Vec<Node> {
    let reason = m.step_disabled_reason.clone().unwrap_or_default();
    let disabled = m.step_disabled_reason.is_some();
    let mut grid = Node::new("div")
        .style("display: grid; grid-template-columns: 35% 25% 20% 20%; grid-auto-rows: 60px;")
        .child(Node::el("div", "mfd-label").style(&format!("{} padding-left: 60px;", HDR)).text("WPT"))
        .child(
            Node::el("div", "mfd-label")
                .style(&format!("{} padding-left: 25px; border-right: 1px solid #777777;", HDR))
                .text("ALT"),
        )
        .child(
            Node::el("div", "mfd-label")
                .style(&format!("{} padding-left: 30px;", HDR))
                .child(Node::new("span").style("margin-right: 70px;").text("DIST")),
        )
        .child(
            Node::el("div", "mfd-label")
                .style(&format!("{} padding-left: 30px;", HDR))
                .child(Node::new("span").text(if m.preflight { "TIME" } else { "UTC" })),
        );
    for li in 0..5 {
        grid = grid.children(step_line(m, li));
    }
    let table = Node::el("div", "mfd-fms-fpln-labeled-box-container")
        .style("width: 100%;")
        .child(
            Node::el("div", "mfd-fms-fpln-labeled-box-label")
                .style("margin-left: 15px;")
                .child(Node::el("span", "mfd-label mfd-spacing-right").text("STEP ALTs FROM CRZ"))
                .child(Node::el("span", "mfd-label-unit mfd-unit-leading").text("FL"))
                .child(val_cls(m, true).text(&m.crz_fl)),
        )
        .child(
            Node::new("div")
                .style("width: 100%")
                .child(grid)
                .child(Node::new("div").style("flex-grow: 1"))
                .child(
                    Node::el("div", "fr jcc").style("flex: 1;").child(
                        Node::el("span", "mfd-label biggest amber")
                            .style(if m.step_not_allowed_at.is_empty() { "visibility: hidden;" } else { "visibility: visible;" })
                            .text(&m.step_not_allowed_at),
                    ),
                ),
        );
    let scroll = Node::new("div")
        .style("display: flex; flex-direction: row; justify-content: center; align-items: center;")
        .child(icon("double-down", m.step_scroll_down_disabled, "padding: 10px; margin-right: 5px;"))
        .child(icon("double-up", m.step_scroll_up_disabled, "padding: 10px"));
    let opt = Node::el("div", "fc").style("width: 100%; margin-top: 25px;").child(
        Node::el("div", "mfd-fms-fpln-labeled-box-container")
            .style("flex-direction: row; justify-content: flex-start; align-items: flex-start;")
            .child(Node::el("div", "mfd-fms-fpln-labeled-box-label").child(Node::el("span", "mfd-label").text("OPTIMUM STEP POINT")))
            .child(
                Node::el("div", "fr aic")
                    .style("margin: 25px 200px 0px 10px;")
                    .child(Node::el("span", "mfd-label").style("margin-right: 10px;").text("TO"))
                    .child(Node::el("span", "mfd-label-unit mfd-unit-leading").text("FL"))
                    .child(val_cls(m, false).text("---")),
            )
            .child(Node::el("div", "mfd-label").style("margin-top: 60px; margin-bottom: 30px;").text("NO OPTIMUM STEP FOUND")),
    );
    vec![
        Node::el("div", "mfd-page-container")
            .style(if disabled { "display: inherit;" } else { "display: none;" })
            .child(Node::el("div", "mfd-amber-error-message").text(&reason)),
        Node::new("div")
            .style(&format!(
                "flex: 1 1 0%; display: {}; flex-direction: column; justify-content: center; align-items: center;",
                if disabled { "none" } else { "block" }
            ))
            .child(table)
            .child(scroll)
            .child(opt),
    ]
}

pub fn vert_rev(m: &VertRevModel) -> Node {
    let titles = ["RTA", "SPD", "CMS", "ALT", "STEP ALTs"];
    let t = Tabs::new(&titles, m.selected_tab);
    let s = m.selected_tab;
    let fill = s == 4 && m.step_disabled_reason.is_none();
    let wrap = |label: Node, visible: bool| {
        Node::new("div")
            .style(&format!("display: flex; justify-content: flex-end; padding: 2px; visibility: {};", if visible { "visible" } else { "hidden" }))
            .child(label)
    };
    page_container(vec![
        Node::new("div").style("height: 15px;"),
        top_tab_navigator(
            &t,
            vec![
                tab_page(s == 0, None, vec![rta_page(m)]),
                tab_page(s == 1, None, vec![spd_page(m)]),
                tab_page(s == 2, None, vec![cms_page()]),
                tab_page(s == 3, None, vec![alt_page(m)]),
                tab_page(s == 4, None, step_page(m)),
            ],
        ),
        Node::new("div").style(&format!("flex-grow: 1; display: {};", if fill { "inherit" } else { "none" })),
        Node::new("div")
            .style("display: flex; flex-direction: row; justify-content: space-between;")
            .child(wrap(btn("RETURN", false, ""), !m.tmpy_active))
            .child(wrap(btn("TMPY F-PLN", false, "color: yellow"), m.tmpy_active)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_both_feet() {
        assert_eq!(alt_window(9000.0, 7000.0, false, false), ("".into(), "7000-9000".into(), "FT".into()));
    }

    #[test]
    fn window_upper_flight_level() {
        assert_eq!(alt_window(12000.0, 9000.0, true, false), ("FT".into(), "9000-120".into(), "FL".into()));
        assert_eq!(alt_window(25000.0, 15000.0, true, true), ("".into(), "150-250".into(), "FL".into()));
    }

    #[test]
    fn step_tab_hides_disabled_message() {
        let m = VertRevModel { selected_tab: 4, ..Default::default() };
        let n = vert_rev(&m);
        assert!(text_of(&n).contains("STEP ALTs FROM CRZ"));
        let m = VertRevModel { step_disabled_reason: Some("X".into()), ..m };
        assert!(text_of(&vert_rev(&m)).contains('X'));
    }
}

use crate::w::*;
use mfd_tree::Node;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SurvControlsModel {
    pub footer: FooterModel,
    pub xpdr_failed: bool,
    pub squawk_text: String,
    pub alt_rptg_on: bool,
    pub alt_rptg_disabled: bool,
    pub xpdr_radio_color: String,
    pub tcas_failed: bool,
    pub tcas_radio_color: String,
    pub tcas_radio_disabled: bool,
    pub wxr_failed: bool,
    pub wxr_off: bool,
    pub wxr_inhibited: bool,
    pub wxr_auto: bool,
    pub pred_ws_auto: bool,
    pub turb_auto: bool,
    pub gain_auto: bool,
    pub mode_wx: bool,
    pub vd_auto: bool,
    pub elevn_tilt_color: String,
    pub elevn_tilt_disabled: bool,
    pub elevn_unit: String,
    pub tilt_selected: bool,
    pub elevn_ft_text: String,
    pub elevn_fl_text: String,
    pub tilt_text: String,
    pub gain_text: String,
    pub gain_field_shown: bool,
    pub confirm_visible: bool,
    pub confirm_label: String,
    pub all_taws_failed: bool,
    pub terr_on: bool,
    pub terr_failed: bool,
    pub gpws_on: bool,
    pub gs_on: bool,
    pub flap_on: bool,
    pub gpws_failed: bool,
}

fn heading(cls: &str, failed: bool, text: &str) -> Node {
    Node::el("div", &format!("mfd-surv-heading {cls}")).class_if("failed", failed).text(text)
}

fn label_big(style: &str, text: &str) -> Node {
    Node::el("div", "mfd-label bigger").style(style).text(text)
}

fn surv_button(state: bool, disabled: bool, t: &str, f: &str) -> Node {
    toggle_button("mfd-surv-button", state, disabled, t, f, "")
}

fn input_display(shown: bool, node: Node) -> Node {
    Node::new("div").style(if shown { "display: block" } else { "display: none" }).child(node)
}

pub fn controls_body(m: &SurvControlsModel) -> Node {
    let color = |c: &str| if c.is_empty() { "cyan".to_string() } else { c.to_string() };
    let xpdr_left = Node::el("div", "mfd-surv-controls-xpdr-left")
        .child(heading("mfd-surv-xpdr-label", m.xpdr_failed, "XPDR"))
        .child(label_big("margin-top: 30px;", "SQWK"))
        .child(input_field_text(&m.squawk_text, "width: 100px; margin-bottom: 5px;", "center", 4, "", "", ""))
        .child(button(&ButtonOpts::new("IDENT").style("width: 100px;")))
        .child(label_big("margin-top: 20px; margin-bottom: 5px;", "ALT RPTG"))
        .child(surv_button(m.alt_rptg_on, m.alt_rptg_disabled, "ON", "OFF"));
    let xpdr_right = Node::el("div", "mfd-surv-controls-xpdr-right").child(radio_group(
        "CAPT_MFD_survControlsXpdrStatus",
        &["AUTO", "STBY"],
        50,
        &color(&m.xpdr_radio_color),
        false,
    ));
    let xpdr = Node::el("div", "mfd-surv-controls-xpdr-section").child(xpdr_left).child(xpdr_right);
    let tcas = Node::el("div", "mfd-surv-controls-tcas-section")
        .child(heading("mfd-surv-tcas-label", m.tcas_failed, "TCAS"))
        .child(Node::el("div", "mfd-surv-controls-tcas-left").child(radio_group(
            "CAPT_MFD_survControlsTcasTara",
            &["TA/RA", "TA ONLY", "STBY"],
            10,
            &color(&m.tcas_radio_color),
            m.tcas_radio_disabled,
        )))
        .child(Node::el("div", "mfd-surv-controls-tcas-right").child(radio_group(
            "CAPT_MFD_survControlsTcasNormAbvBlw",
            &["NORM", "ABV", "BLW"],
            10,
            &color(&m.tcas_radio_color),
            m.tcas_radio_disabled,
        )));
    let first = Node::el("div", "mfd-surv-controls-first-section").child(xpdr).child(tcas);

    let entry = Node::el("div", "mfd-surv-controls-wxr-entry")
        .child(input_display(
            m.elevn_unit == "FT",
            input_field_text(&m.elevn_ft_text, "width: 130px;", "center", 5, "", "FT", ""),
        ))
        .child(input_display(
            m.elevn_unit == "FL",
            input_field_text(&m.elevn_fl_text, "width: 130px;", "center", 3, "FL", "", ""),
        ))
        .child(input_display(
            m.tilt_selected,
            input_field_text(&m.tilt_text, "width: 130px;", "center", 5, "", "\u{b0}", ""),
        ));
    let wxr_left = Node::el("div", "mfd-surv-controls-wxr-left")
        .child(heading("mfd-surv-wxr-label", m.wxr_failed, "WXR"))
        .child(label_big("margin-top: 80px;", "ELEVN/TILT"))
        .child(radio_group(
            "CAPT_MFD_survControlswxrElevnTilt",
            &["AUTO", "ELEVN", "TILT"],
            10,
            &color(&m.elevn_tilt_color),
            m.elevn_tilt_disabled,
        ))
        .child(entry);

    let cell = |label: &str, btn: Node| {
        Node::el("div", "mfd-surv-controls-wxr-grid-cell")
            .child(Node::el("div", "mfd-surv-label").text(label))
            .child(btn)
    };
    let vis = if m.wxr_off { "visibility: hidden" } else { "visibility: visible" };
    let gain_cell = Node::el("div", "mfd-surv-controls-wxr-grid-cell")
        .style(vis)
        .child(Node::el("div", "mfd-surv-label").text("GAIN"))
        .child(surv_button(m.gain_auto, m.wxr_failed, "AUTO", "MAN"))
        .child(
            Node::el("div", "mfd-surv-controls-wxr-gain-entry")
                .style(if m.gain_field_shown { "display: block" } else { "display: none" })
                .child(input_field_text(&m.gain_text, "width: 90px;", "center", 3, "", "%", "")),
        );
    let mode_cell = Node::el("div", "mfd-surv-controls-wxr-grid-cell")
        .style(vis)
        .child(Node::el("div", "mfd-surv-label").text("MODE"))
        .child(surv_button(m.mode_wx, m.wxr_failed, "WX", "MAP"));
    let grid = Node::el("div", "mfd-surv-controls-wxr-grid")
        .child(cell("WXR", surv_button(m.wxr_auto, m.wxr_failed, "AUTO", "OFF")))
        .child(cell("PRED W/S", surv_button(m.pred_ws_auto, m.wxr_inhibited, "AUTO", "OFF")))
        .child(cell("TURB", surv_button(m.turb_auto, m.wxr_inhibited, "AUTO", "OFF")))
        .child(gain_cell)
        .child(mode_cell)
        .child(cell("WX ON VD", surv_button(m.vd_auto, m.wxr_inhibited, "AUTO", "OFF")));

    let mut cancel = ButtonOpts::new("CANCEL");
    cancel.undefined_style = true;
    let dialog = Node::new("div")
        .style(&format!("position: relative; display: {};", if m.confirm_visible { "block" } else { "none" }))
        .child(
            Node::el("div", "mfd-dialog")
                .style("width: 300px; height: 140px; left: -560px; top: -10px;")
                .child(
                    Node::el("div", "mfd-dialog-title")
                        .child(Node::el("span", "mfd-label").text(if m.confirm_label.is_empty() { " " } else { &m.confirm_label })),
                )
                .child(
                    Node::el("div", "mfd-dialog-buttons")
                        .child(button(&cancel))
                        .child(button(&ButtonOpts::new("CONFIRM *").style("padding-right: 6px;"))),
                ),
        );
    let wxr_right = Node::el("div", "mfd-surv-controls-wxr-right").child(grid).child(dialog);
    let second = Node::el("div", "mfd-surv-controls-second-section").child(wxr_left).child(wxr_right);

    let taws_el = |label: &str, state: bool, failed: bool, style: &str| {
        let mut n = Node::el("div", "mfd-surv-controls-taws-element");
        if !style.is_empty() {
            n = n.style(style);
        }
        n.child(Node::el("div", "mfd-surv-label").text(label)).child(surv_button(state, failed, "ON", "OFF"))
    };
    let taws = Node::el("div", "mfd-surv-controls-taws-section")
        .child(taws_el("TERR SYS", m.terr_on, m.terr_failed, "padding-right: 50px;"))
        .child(taws_el("GPWS", m.gpws_on, m.gpws_failed, ""))
        .child(taws_el("G/S MODE", m.gs_on, m.gpws_failed, ""))
        .child(taws_el("FLAP MODE", m.flap_on, m.gpws_failed, ""));
    let def = Node::el("div", "mfd-surv-controls-def-settings-container").child(
        Node::el("div", "mfd-surv-controls-def-settings")
            .child(Node::el("div", "mfd-surv-label").text("SURV"))
            .child(button(&ButtonOpts::new("DEFAULT SETTINGS").style("width: 140px;"))),
    );
    let third = Node::el("div", "mfd-surv-controls-third-section")
        .child(heading("mfd-surv-taws-label", m.all_taws_failed, "TAWS"))
        .child(taws)
        .child(def);
    Node::el("div", "mfd-page-container").child(first).child(second).child(third)
}

pub fn controls_page(m: &SurvControlsModel) -> Node {
    navigator("CONTROLS", controls_body(m), vec![], footer(&m.footer))
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SurvStatusModel {
    pub footer: FooterModel,
    pub wxr_taws: [SurvStatusSide; 2],
    pub xpdr_tcas: [SurvStatusSide; 2],
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SurvStatusSide {
    pub active: bool,
    pub failed_a: bool,
    pub failed_b: bool,
    pub failed_c: bool,
    pub failed_d: bool,
    pub failed_e: bool,
}

fn status_item(label: &str, sys: &str, active: bool, failed: bool, style: &str) -> Node {
    let text = format!("{} {}", label, if active { sys } else { "OFF" });
    let mut n = Node::el("div", "mfd-surv-status-item").class_if("active", active).class_if("failed", failed);
    if !style.is_empty() {
        n = n.style(style);
    }
    n.text(&text)
}

fn status_button(label: &str, active: bool) -> Node {
    let ind = || Node::el("div", "mfd-surv-status-indicator").class_if("active", active);
    Node::el("div", "mfd-surv-status-button")
        .child(
            Node::new("div")
                .style("padding-left: 7px; padding-right: 7px; height: 50%; background-color: black; display: flex; flex-direction: column; justify-content: space-evenly;")
                .child(ind())
                .child(ind())
                .child(ind()),
        )
        .child(Node::el("p", "mfd-surv-status-button-label").text(label))
}

fn sys_group(active: bool, style: &str, items: Vec<Node>) -> Node {
    let mut n = Node::el("div", "sys-group").class_if("active", active);
    if !style.is_empty() {
        n = n.style(style);
    }
    n.children(items)
}

fn wxr_box(sys: &str, s: &SurvStatusSide) -> Node {
    let a = s.active;
    let mb = "margin-bottom: 10px;";
    Node::el("div", "sys-box")
        .child(status_button(&format!("SYS {sys}"), a))
        .child(sys_group(
            a,
            mb,
            vec![
                status_item("WXR DISPLAY", sys, a, s.failed_a, mb),
                status_item("TURB", sys, a, s.failed_b, mb),
                status_item("PRED W/S", sys, a, s.failed_c, ""),
            ],
        ))
        .child(sys_group(a, "", vec![status_item("TERR SYS", sys, a, s.failed_d, mb), status_item("GPWS", sys, a, s.failed_e, "")]))
}

fn xpdr_box(sys: &str, s: &SurvStatusSide) -> Node {
    let a = s.active;
    Node::el("div", "sys-box")
        .child(status_button(&format!("SYS {sys}"), a))
        .child(sys_group(a, "margin-bottom: 5px;", vec![status_item("XPDR", sys, a, s.failed_a, "")]))
        .child(sys_group(a, "", vec![status_item("TCAS", sys, a, s.failed_b, "")]))
}

pub fn status_body(m: &SurvStatusModel) -> Node {
    let row_style = "width: 100%; display: flex; flex-direction: row; justify-content: space-between; align-items: top; padding: 50px;";
    let upper_mid = Node::new("div")
        .style("text-align: center;")
        .child(label_big("margin-top: 55px;", "WXR"))
        .child(label_big("margin-top: 90px;", "TAWS"));
    let lower_mid = Node::new("div")
        .style("text-align: center;")
        .child(label_big("margin-top: 55px;", "XPDR"))
        .child(label_big("margin-top: 20px;", "TCAS"));
    Node::el("div", "mfd-page-container")
        .child(
            Node::new("div")
                .style(row_style)
                .child(wxr_box("1", &m.wxr_taws[0]))
                .child(upper_mid)
                .child(wxr_box("2", &m.wxr_taws[1])),
        )
        .child(
            Node::new("div")
                .style(row_style)
                .child(xpdr_box("1", &m.xpdr_tcas[0]))
                .child(lower_mid)
                .child(xpdr_box("2", &m.xpdr_tcas[1])),
        )
}

pub fn status_page(m: &SurvStatusModel) -> Node {
    navigator("STATUS & SWITCHING", status_body(m), vec![], footer(&m.footer))
}

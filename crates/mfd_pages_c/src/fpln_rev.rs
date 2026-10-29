use crate::entry::{js_to_fixed, HoldEntry};
use crate::radio::{radio_button_group, RadioColor};
use crate::vert_rev::{fld, waypoint_dropdown, Fld};
use crate::x::*;
use mfd_pages_a::widgets as aw;
use mfd_pages_a::widgets::{ButtonSpec, MenuItem};
use mfd_tree::Node;

fn vis(v: bool) -> &'static str {
    if v {
        "visibility: visible;"
    } else {
        "visibility: hidden;"
    }
}

fn tmpy_color(tmpy: bool) -> RadioColor {
    if tmpy {
        RadioColor::Yellow
    } else {
        RadioColor::Cyan
    }
}

fn return_tmpy_row(tmpy: bool, row_class: &str, row_style: &str, div_class: Option<&str>, tmpy_style: &str) -> Node {
    let div = |label: Node, visible: bool| {
        let d = match div_class {
            Some(c) => Node::el("div", c).style(vis(visible)),
            None => Node::new("div").style(&format!("display: flex; justify-content: flex-end; padding: 2px; {}", vis(visible))),
        };
        d.child(label)
    };
    let mut row = Node::el("div", row_class);
    if !row_style.is_empty() {
        row = row.style(row_style);
    }
    row.child(div(btn("RETURN", false, ""), !tmpy)).child(div(btn("TMPY F-PLN", false, tmpy_style), tmpy))
}

#[derive(Clone, Debug)]
pub struct HoldModel {
    pub cap_or_fo: String,
    pub tmpy_active: bool,
    pub active_or_tmpy: bool,
    pub hold_type: String,
    pub waypoint_ident: String,
    pub inbound_course: Option<f64>,
    pub turn: Option<usize>,
    pub leg_parameter: Option<usize>,
    pub leg_time: Option<f64>,
    pub leg_distance: Option<f64>,
    pub last_exit_utc: String,
    pub last_exit_efob: f64,
    pub metric: bool,
}

impl Default for HoldModel {
    fn default() -> Self {
        HoldModel {
            cap_or_fo: "CAPT".into(),
            tmpy_active: false,
            active_or_tmpy: true,
            hold_type: "MODIFIED HOLD AT".into(),
            waypoint_ident: "WAYPOINT".into(),
            inbound_course: None,
            turn: None,
            leg_parameter: None,
            leg_time: None,
            leg_distance: None,
            last_exit_utc: String::new(),
            last_exit_efob: f64::NAN,
            metric: true,
        }
    }
}

pub fn hold_type_label(computed: bool, pilot: bool) -> &'static str {
    if computed {
        "COMPUTED HOLD AT "
    } else if pilot {
        "MODIFIED HOLD AT "
    } else {
        ""
    }
}

pub fn weight_tonnes(v: f64) -> String {
    if v.is_nan() {
        "---.-".into()
    } else {
        js_to_fixed(v, 1)
    }
}

fn entry_field(m_tmpy: bool, e: HoldEntry, v: Option<f64>) -> Node {
    let f = e.format(v);
    let digits = match e {
        HoldEntry::Heading => 5,
        HoldEntry::InboundCourse | HoldEntry::HoldDist => 4,
        HoldEntry::HoldTime => 3,
    };
    fld(Fld {
        text: &f.value,
        trailing: f.trailing.as_deref().unwrap_or(""),
        align: "center",
        style: "",
        disabled: false,
        computed: false,
        tmpy: m_tmpy,
        root: "",
        digits,
    })
}

pub fn hold(m: &HoldModel) -> Vec<Node> {
    let color = tmpy_color(m.tmpy_active);
    let label_mt = "margin-top: 50px; margin-bottom: 20px;";
    let (time_vis, dist_vis) = match m.leg_parameter {
        Some(0) => (true, false),
        Some(1) => (false, true),
        _ => (false, false),
    };
    let left = Node::el("div", "mfd-fms-fpln-labeled-box-container")
        .style("flex-grow: 1;")
        .child(
            Node::el("span", "mfd-label mfd-spacing-right mfd-fms-fpln-labeled-box-label")
                .text(&m.hold_type)
                .text(" ")
                .child(Node::el("span", "mfd-label").class_if("green", m.active_or_tmpy).class_if("bigger", true).text(&m.waypoint_ident)),
        )
        .child(Node::el("span", "mfd-label").style(label_mt).text("INBOUND CRS"))
        .child(Node::new("div").style("margin-left: 75px;").child(entry_field(m.tmpy_active, HoldEntry::InboundCourse, m.inbound_course)))
        .child(Node::el("span", "mfd-label").style(label_mt).text("TURN"))
        .child(Node::new("div").style("margin-left: 75px;").child(radio_button_group(
            &format!("{}_MFD_holdTurnRadio", m.cap_or_fo),
            &["LEFT", "RIGHT"],
            m.turn,
            color,
            &[],
        )))
        .child(Node::el("span", "mfd-label").style(label_mt).text("LEG DEFINING PARAMETER"))
        .child(
            Node::new("div")
                .style("display: flex; flex-direction: row; margin-left: 75px;")
                .child(radio_button_group(
                    &format!("{}_MFD_holdDefiningParameterRadio", m.cap_or_fo),
                    &["TIME", "DIST"],
                    m.leg_parameter,
                    color,
                    &[],
                ))
                .child(
                    Node::el("div", "mfd-fpln-hold-timedist-box")
                        .child(Node::new("div").style(vis(time_vis)).child(entry_field(m.tmpy_active, HoldEntry::HoldTime, m.leg_time)))
                        .child(Node::new("div").style(vis(dist_vis)).child(entry_field(m.tmpy_active, HoldEntry::HoldDist, m.leg_distance))),
                ),
        )
        .child(Node::el("span", "mfd-label mfd-fpln-hold-last-exit-label").text("LAST EXIT (FOR EXTRA FUEL 0 AT ALTN)"))
        .child(
            Node::el("div", "mfd-fpln-hold-grid")
                .child(Node::el("div", "mfd-label").text("AT"))
                .child(Node::el("div", "mfd-label").style("align-self: center;").text("UTC"))
                .child(Node::el("div", "mfd-label").text("EFOB"))
                .child(Node::new("div"))
                .child(Node::el("div", "mfd-value").class_if("magenta", m.active_or_tmpy).text(&m.last_exit_utc))
                .child(
                    Node::el("div", "mfd-label-value-container")
                        .child(Node::el("span", "mfd-value").class_if("magenta", m.active_or_tmpy).text(&weight_tonnes(m.last_exit_efob)))
                        .child(Node::el("span", "mfd-label-unit mfd-unit-trailing").text(if m.metric { "T" } else { "KLB" })),
                ),
        );
    let right = Node::el("div", "fc")
        .style("margin-top: 40px;")
        .child(btn("DATABASE", true, "padding: 20px; margin: 5px;"))
        .child(btn("COMPUTED", false, "padding: 20px; margin: 5px;"));
    let cls = "mfd-fpln-hold-button-with-creative-class-name-which-is-as-long-as-style-attribute";
    vec![
        Node::el("div", "fr").child(left).child(right),
        Node::new("div").style("flex-grow: 1;"),
        return_tmpy_row(m.tmpy_active, "fr", "justify-content: space-between;", Some(cls), "color: yellow"),
    ]
}

#[derive(Clone, Debug)]
pub struct DirectToModel {
    pub cap_or_fo: String,
    pub tmpy_active: bool,
    pub waypoints: Vec<String>,
    pub selected: Option<usize>,
    pub free_text: String,
    pub utc_eta: String,
    pub dist: String,
    pub option: Option<usize>,
    pub editing: bool,
}

impl Default for DirectToModel {
    fn default() -> Self {
        DirectToModel {
            cap_or_fo: "CAPT".into(),
            tmpy_active: false,
            waypoints: vec![],
            selected: None,
            free_text: String::new(),
            utc_eta: "--:--".into(),
            dist: "---".into(),
            option: Some(0),
            editing: false,
        }
    }
}

fn set_input_tmpy(n: &mut Node) {
    if n.cls.iter().any(|c| c == "mfd-input-field-text-input") {
        n.cls.push("tmpy".into());
        return;
    }
    for k in n.kids.iter_mut() {
        if let mfd_tree::Kid::Node(c) = k {
            set_input_tmpy(c);
        }
    }
}

fn big_value(text: &str, tmpy: bool) -> Node {
    Node::el("span", "mfd-value bigger").class_if("mfd-fms-yellow-text", tmpy).text(text)
}

pub fn direct_to(m: &DirectToModel) -> Vec<Node> {
    let text = match m.selected {
        Some(i) if i < m.waypoints.len() => m.waypoints[i].clone(),
        _ if !m.free_text.is_empty() => m.free_text.clone(),
        _ => "-------".into(),
    };
    let mut dd = waypoint_dropdown(&format!("{}_MFD_directToDropdown", m.cap_or_fo), &m.waypoints, &text, m.editing);
    if m.tmpy_active && !m.editing {
        set_input_tmpy(&mut dd);
    }
    let left = Node::new("div").style("flex: 1").child(
        Node::el("div", "fc")
            .child(
                Node::el("div", "mfd-fms-direct-to-wpt-row")
                    .child(Node::el("span", "mfd-label").text("DIRECT TO"))
                    .child(Node::el("div", "mfd-fms-direct-to-dropdown-div").child(dd)),
            )
            .child(
                Node::el("div", "mfd-fms-direct-to-wpt-info")
                    .child(Node::el("div", "mfd-fms-direct-to-utc-label").child(Node::el("span", "mfd-label").text("UTC")))
                    .child(Node::el("div", "mfd-fms-direct-to-utc-value").child(big_value(&m.utc_eta, m.tmpy_active)))
                    .child(Node::new("div"))
                    .child(Node::el("div", "mfd-fms-direct-to-utc-label").child(Node::el("span", "mfd-label").text("DIST")))
                    .child(Node::el("div", "mfd-fms-direct-to-utc-value").child(big_value(&m.dist, m.tmpy_active)))
                    .child(Node::new("div").child(Node::el("span", "mfd-label-unit mfd-unit-trailing").text("NM"))),
            ),
    );
    let right = Node::new("div").style("flex: 1").child(
        Node::el("div", "mfd-fms-direct-to-options-box")
            .child(Node::el("span", "mfd-label").text("OPTIONS"))
            .child(Node::el("div", "mfd-fms-direct-to-options").child(radio_button_group(
                &format!("{}_MFD_directToOptionsRadio", m.cap_or_fo),
                &["DIRECT", "DIRECT WITH ABEAM", "CRS IN", "CRS OUT"],
                m.option,
                tmpy_color(m.tmpy_active),
                &[false, true, true, true],
            ))),
    );
    let t = m.tmpy_active;
    let cls = "mfd-fms-direct-to-erase-return-btn";
    vec![
        Node::el("div", "fr").child(left).child(right),
        Node::new("div").style("flex-grow: 1;"),
        Node::el("div", "mfd-fms-bottom-button-row")
            .child(Node::el("div", cls).style(if t { "display: block;" } else { "display: none;" }).child(btn_lines(&["ERASE", "DIR TO*"], false, "color: #e68000;")))
            .child(Node::el("div", cls).style(if t { "display: none;" } else { "display: block;" }).child(btn("RETURN", false, "")))
            .child(Node::el("div", cls).style(vis(t)).child(btn_lines(&["INSERT", "DIR TO*"], false, "color: #e68000;"))),
    ]
}

#[derive(Clone, Debug, Default)]
pub struct AirwayLine {
    pub via: Option<String>,
    pub via_disabled: bool,
    pub to: Option<String>,
    pub to_disabled: bool,
}

#[derive(Clone, Debug, Default)]
pub struct AirwaysModel {
    pub tmpy_active: bool,
    pub revised_fix: String,
    pub lines: Vec<AirwayLine>,
    pub scroll_down_disabled: bool,
    pub scroll_up_disabled: bool,
}

pub const AIRWAY_MAX_LINES: usize = 11;

fn airway_line(l: &AirwayLine) -> Node {
    let f = |text: Option<&String>, ph: &str, disabled: bool, digits: usize| {
        fld(Fld {
            text: text.map(String::as_str).filter(|s| !s.is_empty()).unwrap_or(ph),
            trailing: "",
            align: "center",
            style: "",
            disabled,
            computed: false,
            tmpy: false,
            root: "yellow-when-disabled",
            digits,
        })
    };
    Node::el("div", "fr mfd-fms-awy-line-container")
        .child(
            Node::el("div", "fr aic")
                .child(Node::el("div", "mfd-label").style("margin-right: 5px;").text("VIA"))
                .child(f(l.via.as_ref(), "---", l.via_disabled, 5)),
        )
        .child(
            Node::el("div", "fr aic")
                .child(Node::el("div", "mfd-label").style("margin-right: 5px;").text("TO"))
                .child(f(l.to.as_ref(), "-------", l.to_disabled, 7)),
        )
}

pub fn airways(m: &AirwaysModel) -> Vec<Node> {
    vec![
        Node::el("div", "fc")
            .style("margin-top: 15px;")
            .child(
                Node::el("div", "fr aic")
                    .child(Node::el("span", "mfd-label").style("margin-left: 15px;").text("AIRWAYS FROM"))
                    .child(
                        Node::el("span", "mfd-value bigger")
                            .class_if("mfd-fms-yellow-text", m.tmpy_active)
                            .style("margin-left: 20px;")
                            .text(&m.revised_fix),
                    ),
            )
            .child(Node::el("div", "mfd-fms-fpln-awy-awy-container").children(m.lines.iter().take(AIRWAY_MAX_LINES).map(airway_line))),
        Node::el("div", "fr jcc")
            .child(icon("double-down", m.scroll_down_disabled, "width: 60px; height: 60px; margin-right: 20px;"))
            .child(icon("double-up", m.scroll_up_disabled, "width: 60px; height: 60px;")),
        Node::new("div").style("flex-grow: 1"),
        {
            let cls = "mfd-fms-direct-to-erase-return-btn";
            Node::el("div", "mfd-fms-bottom-button-row")
                .child(Node::el("div", cls).style(vis(!m.tmpy_active)).child(btn("RETURN", false, "")))
                .child(Node::el("div", cls).style(vis(m.tmpy_active)).child(btn("TMPY F-PLN", false, "color: #ffff00;")))
        },
    ]
}

#[derive(Clone, Debug, Default)]
pub struct ProcMenu {
    pub disabled: bool,
    pub items: Vec<(String, bool)>,
}

#[derive(Clone, Debug)]
pub struct DepartureModel {
    pub cap_or_fo: String,
    pub tmpy_active: bool,
    pub sec_active: bool,
    pub metric: bool,
    pub from_icao: String,
    pub ls_ident: String,
    pub rwy_ident: String,
    pub rwy_length_m: f64,
    pub rwy_crs: String,
    pub eosid: String,
    pub freq: String,
    pub sid: String,
    pub trans: String,
    pub rwy: ProcMenu,
    pub sid_menu: ProcMenu,
    pub trans_menu: ProcMenu,
}

impl Default for DepartureModel {
    fn default() -> Self {
        DepartureModel {
            cap_or_fo: "CAPT".into(),
            tmpy_active: false,
            sec_active: false,
            metric: true,
            from_icao: String::new(),
            ls_ident: "----".into(),
            rwy_ident: String::new(),
            rwy_length_m: f64::NAN,
            rwy_crs: String::new(),
            eosid: String::new(),
            freq: String::new(),
            sid: String::new(),
            trans: String::new(),
            rwy: ProcMenu::default(),
            sid_menu: ProcMenu::default(),
            trans_menu: ProcMenu::default(),
        }
    }
}

pub fn runway_length(m: f64, metric: bool) -> (String, &'static str) {
    if m.is_nan() {
        return ("----".into(), if metric { "M" } else { "FT" });
    }
    if metric {
        (js_to_fixed(m, 0), "M")
    } else {
        (js_to_fixed(m / 0.3048, 0), "FT")
    }
}

fn pv(text: &str, tmpy: bool, sec: bool) -> Node {
    Node::el("span", "mfd-value").class_if("tmpy", tmpy).class_if("sec", sec).text(text)
}

fn col(label: &str, flex: &str, value: Node, wrap: bool) -> Node {
    let c = Node::el("div", "fc").style(&format!("flex: {};", flex)).child(Node::el("span", "mfd-label mfd-fms-fpln-label-bottom-space").text(label));
    if wrap {
        c.child(Node::new("div").child(value))
    } else {
        c.child(value)
    }
}

fn with_unit(value: Node, unit: &str) -> Node {
    Node::new("div").child(value).child(Node::el("span", "mfd-label-unit mfd-unit-trailing").text(unit))
}

fn proc_button(label: &str, style: &str, prefix: &str, menu: &ProcMenu) -> Node {
    let items: Vec<MenuItem> = menu.items.iter().map(|(l, d)| MenuItem::new(l, *d)).collect();
    aw::button(ButtonSpec::new(label).disabled(menu.disabled).style(style).menu(prefix, items, false))
}

fn length_col(flex: &str, len_m: f64, metric: bool, t: bool, s: bool) -> Node {
    let (v, u) = runway_length(len_m, metric);
    Node::el("div", "fc")
        .style(&format!("flex: {};", flex))
        .child(Node::el("span", "mfd-label mfd-fms-fpln-label-bottom-space").text("LENGTH"))
        .child(with_unit(pv(&v, t, s), u))
}

fn crs_col(flex: &str, crs: &str, t: bool, s: bool) -> Node {
    Node::el("div", "fc")
        .style(&format!("flex: {};", flex))
        .child(Node::el("span", "mfd-label mfd-fms-fpln-label-bottom-space").text("CRS"))
        .child(with_unit(pv(crs, t, s), "\u{b0}"))
}

pub fn departure(m: &DepartureModel) -> Vec<Node> {
    let (t, s) = (m.tmpy_active, m.sec_active);
    let p = |n: &str| format!("{}_MFD_f-pln-dep-{}-btn", m.cap_or_fo, n);
    vec![
        Node::el("div", "mfd-fms-fpln-labeled-box-container")
            .child(Node::el("span", "mfd-label mfd-spacing-right mfd-fms-fpln-labeled-box-label").text("SELECTED DEPARTURE"))
            .child(
                Node::el("div", "mfd-fms-fpln-label-bottom-space fr aic")
                    .child(
                        Node::el("div", "fr aic")
                            .style("flex: 2;")
                            .child(Node::el("span", "mfd-label mfd-spacing-right").text("FROM"))
                            .child(pv(&m.from_icao, t, s)),
                    )
                    .child(col("LS", "1", pv(&m.ls_ident, t, s), false))
                    .child(col("RWY", "1", pv(&m.rwy_ident, t, s), false))
                    .child(length_col("1.3", m.rwy_length_m, m.metric, t, s))
                    .child(crs_col("0.7", &m.rwy_crs, t, s)),
            )
            .child(
                Node::el("div", "fr aic")
                    .child(col("EOSID", "0.25", pv(&m.eosid, t, s), false))
                    .child(col("FREQ/CHAN", "0.3", pv(&m.freq, t, s), false))
                    .child(col("SID", "0.25", pv(&m.sid, t, s), true))
                    .child(col("TRANS", "0.2", pv(&m.trans, t, s), true)),
            ),
        Node::el("div", "fr")
            .style("margin-left: 50px;")
            .child(proc_button("RWY", "width: 250px;", &p("rwy"), &m.rwy))
            .child(Node::new("div").style("width: 100px;"))
            .child(proc_button("SID", "width: 140px;", &p("sid"), &m.sid_menu))
            .child(Node::new("div").style("width: 50px;"))
            .child(proc_button("TRANS", "width: 130px;", &p("trans"), &m.trans_menu)),
        Node::new("div").style("flex-grow: 1;"),
        return_tmpy_row(t, "fr", "justify-content: space-between;", None, "color: yellow"),
    ]
}

#[derive(Clone, Debug)]
pub struct ArrivalModel {
    pub cap_or_fo: String,
    pub tmpy_active: bool,
    pub sec_active: bool,
    pub metric: bool,
    pub to_icao: String,
    pub ls_ident: String,
    pub rwy_ident: String,
    pub rwy_length_m: f64,
    pub rwy_crs: String,
    pub approach: String,
    pub ls_freq: String,
    pub via: String,
    pub star: String,
    pub trans: String,
    pub rwy: ProcMenu,
    pub appr_menu: ProcMenu,
    pub via_menu: ProcMenu,
    pub star_menu: ProcMenu,
    pub trans_menu: ProcMenu,
}

impl Default for ArrivalModel {
    fn default() -> Self {
        ArrivalModel {
            cap_or_fo: "CAPT".into(),
            tmpy_active: false,
            sec_active: false,
            metric: true,
            to_icao: String::new(),
            ls_ident: "----".into(),
            rwy_ident: String::new(),
            rwy_length_m: f64::NAN,
            rwy_crs: String::new(),
            approach: String::new(),
            ls_freq: "---.--".into(),
            via: String::new(),
            star: String::new(),
            trans: String::new(),
            rwy: ProcMenu::default(),
            appr_menu: ProcMenu::default(),
            via_menu: ProcMenu::default(),
            star_menu: ProcMenu::default(),
            trans_menu: ProcMenu::default(),
        }
    }
}

pub fn arrival(m: &ArrivalModel) -> Vec<Node> {
    let (t, s) = (m.tmpy_active, m.sec_active);
    let p = |n: &str| format!("{}_MFD_f-pln-arr-{}-btn", m.cap_or_fo, n);
    vec![
        Node::el("div", "mfd-fms-fpln-labeled-box-container")
            .child(Node::el("span", "mfd-label mfd-spacing-right mfd-fms-fpln-labeled-box-label").text("SELECTED ARRIVAL"))
            .child(
                Node::el("div", "mfd-fms-fpln-label-bottom-space fr aic")
                    .child(
                        Node::el("div", "fr aic")
                            .style("flex: 0.2;")
                            .child(Node::el("span", "mfd-label mfd-spacing-right").text("TO"))
                            .child(pv(&m.to_icao, t, s)),
                    )
                    .child(col("LS", "0.2", pv(&m.ls_ident, t, s), false))
                    .child(col("RWY", "0.2", pv(&m.rwy_ident, t, s), true))
                    .child(length_col("0.2", m.rwy_length_m, m.metric, t, s))
                    .child(crs_col("0.2", &m.rwy_crs, t, s)),
            )
            .child(
                Node::el("div", "fr aic")
                    .child(col("APPR", "0.2", pv(&m.approach, t, s), false))
                    .child(col("FREQ/CHAN", "0.2", pv(&m.ls_freq, t, s), false))
                    .child(col("VIA", "0.2", pv(&m.via, t, s), true))
                    .child(col("STAR", "0.2", pv(&m.star, t, s), true))
                    .child(col("TRANS", "0.2", pv(&m.trans, t, s), true)),
            ),
        Node::el("div", "fr")
            .style("justify-content: space-between;")
            .child(proc_button("RWY", "width: 190px;", &p("rwy"), &m.rwy))
            .child(proc_button("APPR", "width: 160px;", &p("appr"), &m.appr_menu))
            .child(proc_button("VIA", "width: 125px;", &p("via"), &m.via_menu))
            .child(proc_button("STAR", "width: 125px;", &p("star"), &m.star_menu))
            .child(proc_button("TRANS", "width: 125px;", &p("trans"), &m.trans_menu)),
        Node::new("div").style("flex-grow: 1;"),
        return_tmpy_row(t, "fr", "justify-content: space-between;", None, "color: yellow"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::w::text_of;
    use mfd_tree::Kid;

    fn all_text(ns: &[Node]) -> String {
        ns.iter().map(text_of).collect::<Vec<_>>().join("|")
    }

    fn find<'a>(n: &'a Node, pred: &dyn Fn(&Node) -> bool) -> Option<&'a Node> {
        if pred(n) {
            return Some(n);
        }
        n.kids.iter().find_map(|k| if let Kid::Node(c) = k { find(c, pred) } else { None })
    }

    fn by_id<'a>(ns: &'a [Node], id: &str) -> Option<&'a Node> {
        ns.iter().find_map(|n| find(n, &|x| x.id.as_deref() == Some(id)))
    }

    #[test]
    fn hold_defaults_and_selection() {
        let m = HoldModel {
            hold_type: hold_type_label(true, false).into(),
            waypoint_ident: "KADOM".into(),
            inbound_course: Some(275.0),
            turn: Some(1),
            leg_parameter: Some(0),
            leg_time: Some(1.0),
            last_exit_utc: "--:--".into(),
            ..Default::default()
        };
        let n = hold(&m);
        let t = all_text(&n);
        assert!(t.contains("COMPUTED HOLD AT  KADOM"));
        assert!(t.contains("275"));
        assert!(t.contains("1.0"));
        assert!(t.contains("---.-"));
        assert!(t.contains("--.-"));
        let right = by_id(&n, "CAPT_MFD_holdTurnRadio_1").unwrap();
        assert!(right.attrs.contains_key("checked"));
        let time_label = by_id(&n, "CAPT_MFD_holdDefiningParameterRadio_label_0").unwrap();
        assert!(time_label.cls.contains(&"cyan".to_string()));
        assert_eq!(n.len(), 3);
    }

    #[test]
    fn hold_tmpy_is_yellow() {
        let n = hold(&HoldModel { tmpy_active: true, ..Default::default() });
        let l = by_id(&n, "CAPT_MFD_holdTurnRadio_label_0").unwrap();
        assert!(l.cls.contains(&"yellow".to_string()));
    }

    #[test]
    fn direct_to_options_disabled() {
        let m = DirectToModel { waypoints: vec!["KADOM".into(), "PKS".into()], selected: Some(1), ..Default::default() };
        let n = direct_to(&m);
        assert!(all_text(&n).contains("DIRECT TO"));
        assert!(by_id(&n, "CAPT_MFD_directToOptionsRadio_0").unwrap().attrs.contains_key("checked"));
        assert!(by_id(&n, "CAPT_MFD_directToOptionsRadio_3").unwrap().attrs.contains_key("disabled"));
        assert!(by_id(&n, "CAPT_MFD_directToDropdown_1").is_some());
        let input = n.iter().find_map(|x| find(x, &|y| y.cls.iter().any(|c| c == "mfd-input-field-text-input"))).unwrap();
        assert_eq!(text_of(input), "PKS");
    }

    #[test]
    fn airways_lines() {
        let m = AirwaysModel {
            revised_fix: "KADOM".into(),
            lines: vec![AirwayLine { via: Some("Q10".into()), via_disabled: true, ..Default::default() }, AirwayLine::default()],
            scroll_down_disabled: true,
            scroll_up_disabled: true,
            ..Default::default()
        };
        let n = airways(&m);
        let t = all_text(&n);
        assert!(t.contains("AIRWAYS FROMKADOM"));
        assert!(t.contains("Q10"));
        assert!(t.contains("-------"));
        let lines = n[0].kids.iter().filter_map(|k| if let Kid::Node(c) = k { Some(c) } else { None }).last().unwrap();
        assert_eq!(lines.kids.len(), 2);
        let root = find(lines, &|x| x.cls.iter().any(|c| c == "yellow-when-disabled")).unwrap();
        let container = find(root, &|x| x.cls.iter().any(|c| c == "mfd-input-field-container")).unwrap();
        assert!(container.cls.contains(&"disabled".to_string()));
    }

    #[test]
    fn departure_and_arrival_placeholders() {
        let d = departure(&DepartureModel {
            from_icao: "YSSY".into(),
            rwy_length_m: 3962.0,
            sid_menu: ProcMenu { disabled: false, items: vec![("NONE".into(), false), ("ABBEY3".into(), false)] },
            ..Default::default()
        });
        let t = all_text(&d);
        assert!(t.contains("YSSY"));
        assert!(t.contains("3962M"));
        assert!(t.contains("----"));
        assert!(by_id(&d, "CAPT_MFD_f-pln-dep-sid-btn_1").is_some());
        let a = arrival(&ArrivalModel { metric: false, rwy_length_m: 3000.0, ..Default::default() });
        let t = all_text(&a);
        assert!(t.contains("---.--"));
        assert!(t.contains("9843FT"));
    }

    #[test]
    fn runway_length_units() {
        assert_eq!(runway_length(f64::NAN, true), ("----".into(), "M"));
        assert_eq!(runway_length(2500.4, true), ("2500".into(), "M"));
    }
}

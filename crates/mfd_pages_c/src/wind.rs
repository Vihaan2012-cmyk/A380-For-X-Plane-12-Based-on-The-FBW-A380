use crate::w::*;
use crate::x::*;
use mfd_pages_a::widgets as aw;
use mfd_tree::{Kid, Node};

#[derive(Clone, Debug)]
pub struct WindRow {
    pub alt: String,
    pub dir: String,
    pub spd: String,
    pub in_fp: bool,
}

impl WindRow {
    pub fn blank(alt: &str) -> Self {
        WindRow { alt: alt.into(), dir: "---".into(), spd: "---".into(), in_fp: false }
    }

    fn has_alt(&self) -> bool {
        !self.alt.chars().all(|c| c == '-')
    }
}

#[derive(Clone, Debug)]
pub struct HistoryRow {
    pub fl_label: String,
    pub fl: String,
    pub dir: String,
    pub spd: String,
}

#[derive(Clone, Debug)]
pub struct WindModel {
    pub selected_tab: usize,
    pub secondary: bool,
    pub history: Vec<HistoryRow>,
    pub climb: Vec<WindRow>,
    pub cruise: Vec<WindRow>,
    pub descent: Vec<WindRow>,
    pub draft_wind_label_visible: bool,
    pub header_display: bool,
    pub message_display: bool,
    pub waypoints: Vec<String>,
    pub selected_waypoint: String,
    pub dropdown_editing: bool,
    pub select_next_disabled: bool,
    pub select_previous_disabled: bool,
    pub cruise_winds_disabled: bool,
    pub cruise_temperature_fl: String,
    pub cruise_temperature: String,
    pub alternate_fl: String,
    pub alternate_primary: bool,
    pub alternate_dir: String,
    pub alternate_spd: String,
    pub alternate_disabled: bool,
    pub draft_winds_exist: bool,
    pub history_button_disabled: bool,
    pub cpny_top: String,
    pub cpny_bottom: String,
    pub cpny_asterisk_hidden: bool,
    pub cpny_disabled: bool,
    pub cpny_menu: Vec<String>,
    pub cap_or_fo: String,
    pub history_button_visible: bool,
    pub return_button_visible: bool,
    pub disabled_due_to_phase: bool,
    pub descent_entries_loaded: bool,
}

impl Default for WindModel {
    fn default() -> Self {
        WindModel {
            selected_tab: 1,
            secondary: false,
            history: (0..5).map(|_| HistoryRow { fl_label: String::new(), fl: String::new(), dir: "---".into(), spd: "/---".into() }).collect(),
            climb: (0..5).map(|_| WindRow::blank("-----")).collect(),
            cruise: (0..4).map(|_| WindRow::blank("---")).collect(),
            descent: (0..5).map(|_| WindRow::blank("-----")).collect(),
            draft_wind_label_visible: false,
            header_display: true,
            message_display: false,
            waypoints: vec![],
            selected_waypoint: "-------".into(),
            dropdown_editing: false,
            select_next_disabled: true,
            select_previous_disabled: true,
            cruise_winds_disabled: false,
            cruise_temperature_fl: "---".into(),
            cruise_temperature: "---".into(),
            alternate_fl: "---".into(),
            alternate_primary: false,
            alternate_dir: "---".into(),
            alternate_spd: "---".into(),
            alternate_disabled: false,
            draft_winds_exist: false,
            history_button_disabled: false,
            cpny_top: "CPNY WIND".into(),
            cpny_bottom: "REQUEST".into(),
            cpny_asterisk_hidden: false,
            cpny_disabled: false,
            cpny_menu: vec![],
            cap_or_fo: "CAPT".into(),
            history_button_visible: false,
            return_button_visible: false,
            disabled_due_to_phase: false,
            descent_entries_loaded: false,
        }
    }
}

fn vis(on: bool) -> &'static str {
    if on { "visibility: visible;" } else { "visibility: hidden;" }
}

fn disp(on: bool) -> &'static str {
    if on { "display: flex;" } else { "display: none;" }
}

fn table_header(m: &WindModel, history: bool) -> Node {
    Node::new("div")
        .child(
            Node::el("div", "mfd-fms-wind-page-table-row header")
                .class_if("history", history)
                .style(disp(m.header_display))
                .child(Node::el("span", "mfd-label bigger").text("ALT"))
                .child(Node::el("span", "mfd-label bigger").text("T.WIND")),
        )
        .child(
            Node::el("div", "mfd-fms-wind-page-table-message-area mfd-value smaller amber")
                .class_if("history", history)
                .style(disp(m.message_display))
                .text("WIND ENTRY NOT ALLOWED: TMPY F-PLN EXISTING"),
        )
}

fn title(m: &WindModel, text: &str) -> Node {
    Node::el("div", "mfd-fms-wind-page-title-container")
        .child(Node::el("span", "mfd-fms-wind-page-draft-label mfd-label biggest").style(vis(m.draft_wind_label_visible)).text("DRAFT"))
        .child(Node::el("span", "mfd-label bigger").text(text))
}

fn disable(n: Node) -> Node {
    let mut n = n;
    if n.cls.iter().any(|c| c == "mfd-input-field-container" || c == "mfd-input-field-text-input") {
        n.cls.push("disabled".into());
    }
    n.kids = n
        .kids
        .into_iter()
        .map(|k| match k {
            Kid::Node(c) => Kid::Node(disable(c)),
            t => t,
        })
        .collect();
    n
}

fn button_visible(mut n: Node, visible: bool) -> Node {
    if !visible {
        n.inline = Some(n.inline.unwrap_or_default().replace("visibility: inherit;", "visibility: hidden;"));
    }
    n
}

fn dir_spd(dir: &str, spd: &str, disabled: bool) -> Vec<Node> {
    let a = Field::new(dir).units("", "\u{b0}").style("height:42px; width:98px; margin-right:6px;").align("center").min_width(54);
    let b = Field::new(spd).units("", "KT").style("height: 42px; width:115px;").align("center").min_width(54);
    let (mut an, mut bn) = (input_field(&a), input_field(&b));
    if disabled {
        an = disable(an);
        bn = disable(bn);
    }
    vec![an, bn]
}

fn rows(data: &[WindRow], leading: &str, kind: &str, phase_disabled: bool, loaded: bool) -> Vec<Node> {
    data.iter()
        .enumerate()
        .map(|(i, r)| {
            let first = i == 0;
            let prev = if first { None } else { data.get(i - 1) };
            let alt_visible = !loaded || r.has_alt() || (!phase_disabled && (first || prev.map_or(false, |p| p.has_alt() || p.in_fp)));
            let wind_visible = !loaded || (!phase_disabled && (first || (!r.has_alt() && r.in_fp))) || r.has_alt();
            let des = kind == "des";
            let mut row = Node::el("div", "mfd-fms-wind-page-table-row");
            row = if des {
                row.class_if("des", !first).class_if("first", first).class_if("entry", first)
            } else {
                row.class_if("entry", true).class_if("first", first)
            };
            row.child(
                Node::el("div", "mfd-fms-wind-page-entry-row")
                    .child(
                        Node::el("div", "mfd-fms-wind-altitude-entry-container")
                            .class_if("first", first)
                            .class_if("des", des && !first)
                            .style(vis(alt_visible))
                            .child(input_field(&Field::new(&r.alt).units(leading, "").style("width:157px; height:40px;").align("flex-start").min_width(90))),
                    )
                    .child(
                        Node::el("div", "mfd-fms-wind-direction-speed-entry-container")
                            .class_if("first", first)
                            .class_if("des", des && !first)
                            .style(vis(wind_visible))
                            .children(dir_spd(&r.dir, &r.spd, false)),
                    ),
            )
        })
        .collect()
}

pub fn wind(m: &WindModel) -> Node {
    let history_title = if m.secondary { "" } else { "HISTORY" };
    let titles = [history_title, "CLB", "CRZ", "DES"];
    let t = Tabs::new(&titles, m.selected_tab);

    let mut hist = Node::el("div", "mfd-fms-wind-page-container").child(Node::el("div", "mfd-fms-wind-page-title-container history")).child(table_header(m, true));
    for (i, h) in m.history.iter().enumerate() {
        let on = !h.fl.is_empty();
        hist = hist.child(
            Node::el("div", "mfd-fms-wind-page-table-row history entry").class_if("first", i == 0).child(
                Node::new("div")
                    .style(&format!("{} display: flex; flex-direction: row;", vis(on)))
                    .child(
                        Node::el("div", "mfd-fms-wind-history-fl-entry")
                            .child(Node::el("span", "mfd-label bigger mfd-fms-wind-page-history-wind-fl-label").text(&h.fl_label))
                            .child(Node::el("span", "mfd-label-unit bigger mfd-fms-wind-page-history-wind-fl-label").text("FL"))
                            .child(Node::el("span", "mfd-label green biggest mfd-fms-wind-page-history-wind-fl-value").text(&h.fl)),
                    )
                    .child(
                        Node::el("div", "mfd-fms-wind-history-wind-entry")
                            .child(Node::el("span", "mfd-label green biggest").text(&h.dir))
                            .child(Node::el("span", "mfd-label-unit bigger").style(vis(on)).text("\u{b0}\u{a0}\u{a0}"))
                            .child(Node::el("span", "mfd-label green biggest").text(&h.spd))
                            .child(Node::el("span", "mfd-label-unit bigger").style(vis(on)).text("KT")),
                    ),
            ),
        );
    }
    let hist_page = vec![
        hist,
        Node::el("div", "history-wind-button-container").child(button_visible(
            btn_node(
                star_label(&["INSERT", "HISTORY WIND"], "text-align: center; vertical-align: center; margin-right: 5px;"),
                m.history_button_disabled,
                false,
                "",
            ),
            m.history_button_visible,
        )),
    ];

    let clb = Node::el("div", "mfd-fms-wind-page-container")
        .child(title(m, "CLB WIND"))
        .child(table_header(m, false))
        .children(rows(&m.climb, "", "clb", m.disabled_due_to_phase, true));

    let mut dd_input = aw::InputSpec::new(if m.selected_waypoint.is_empty() { "------" } else { &m.selected_waypoint });
    if m.dropdown_editing {
        dd_input.state = aw::InputState::focused();
    }
    let dd = aw::DropdownSpec {
        id_prefix: format!("{}_MFD_CruiseWindWaypointDropdown", m.cap_or_fo),
        input: dd_input,
        options: m.waypoints.clone(),
        disabled: m.cruise_winds_disabled,
        container_style: "width: 192px; margin-right: 19px; ".into(),
        align: "center".into(),
        digits: 7,
        open: m.dropdown_editing,
        ..Default::default()
    };
    let crz = Node::el("div", "mfd-fms-wind-page-container crz")
        .child(
            Node::el("div", "mfd-fms-wind-page-crz-title-buttons-container")
                .child(
                    Node::el("div", "mfd-fms-wind-page-crz-title-container")
                        .child(Node::el("span", "mfd-fms-wind-page-draft-label mfd-label biggest").style(vis(m.draft_wind_label_visible)).text("DRAFT"))
                        .child(Node::el("span", "mfd-label bigger").style("position: relative; bottom: 3px;").text("CRZ WIND AT")),
                )
                .child(Node::el("div", "mfd-fms-wind-page-crz-dropdown-container").child(aw::dropdown_menu(dd)))
                .child(
                    Node::el("div", "mfd-fms-wind-page-buttons-container")
                        .child(aw::icon_button(aw::Icon::DoubleDown, m.select_next_disabled, "width: 66px; height: 62px; margin-right: 6px;"))
                        .child(aw::icon_button(aw::Icon::DoubleUp, m.select_previous_disabled, "width: 66px; height: 62px;")),
                ),
        )
        .child(table_header(m, false))
        .children(rows(&m.cruise, "FL", "crz", m.disabled_due_to_phase, !m.selected_waypoint.chars().all(|c| c == '-')));
    let temp_field = |text: &str, leading: &str, trailing: &str, style: &str| {
        disable(input_field(&Field::new(text).units(leading, trailing).style(style).align("flex-start").min_width(54)))
    };
    let crz_temp = Node::el("div", "mfd-fms-wind-page-crz-temp-table")
        .child(
            Node::el("div", "fr")
                .child(Node::el("div", "mfd-fms-wind-crz-temp-alt-header-container").child(Node::el("span", "mfd-label bigger").text("ALT")))
                .child(Node::el("div", "mfd-fms-wind-crz-temp-sat-header-container").child(Node::el("span", "mfd-label bigger").text("SAT"))),
        )
        .child(
            Node::el("div", "mfd-fms-wind-crz-temp-entry-container")
                .child(temp_field(&m.cruise_temperature_fl, "FL", "", "width:157px; height:40px;"))
                .child(temp_field(&m.cruise_temperature, "", "\u{b0}C", "height:43px; width:113px;margin-left: 76px;")),
        );

    let des = Node::el("div", "mfd-fms-wind-page-container")
        .child(title(m, "DES WIND"))
        .child(table_header(m, false))
        .children(rows(&m.descent, "", "des", m.disabled_due_to_phase, m.descent_entries_loaded))
        .child(
            Node::el("div", "mfd-fms-wind-alternate-row")
                .child(Node::el("span", "mfd-label bigger").text("ALTN TRIP WIND"))
                .child(
                    Node::el("div", "fr")
                        .child(
                            Node::el("div", "mfd-fms-wind-alternate-row-cruise-fl")
                                .child(Node::el("span", "mfd-label-unit bigger").style(vis(!m.alternate_fl.chars().all(|c| c == '-'))).text("FL"))
                                .child(Node::el("span", "mfd-label biggest").class_if("green", m.alternate_primary).text(&m.alternate_fl)),
                        )
                        .child(
                            Node::el("div", "mfd-fms-wind-direction-speed-entry-container no-margin")
                                .children(dir_spd(&m.alternate_dir, &m.alternate_spd, m.alternate_disabled)),
                        ),
                ),
        );

    let cpny = {
        let label = Node::new("div")
            .style("display: flex; flex-direction: row; justify-content: space-between;")
            .child(
                Node::new("span")
                    .style("text-align: center; vertical-align: center; margin-right: 10px;")
                    .text(&m.cpny_top)
                    .child(br())
                    .text(&m.cpny_bottom),
            )
            .child(Node::new("span").class_if("hidden", m.cpny_asterisk_hidden).style("display: flex; align-items: center; justify-content: center;").text("*"));
        let mut b = btn_node(label, m.cpny_disabled, false, "margin-right: 10px; justify-self: flex-end; width: 178px; height: 58px;");
        if let Some(Kid::Node(menu)) = b.kids.get_mut(1) {
            let mut inner = Node::new("div");
            for (i, l) in m.cpny_menu.iter().enumerate() {
                inner = inner.child(Node::el("span", "mfd-dropdown-menu-element").id(&format!("{}_MFD_cpnywind_{}", m.cap_or_fo, i)).text(l));
            }
            menu.kids.push(Kid::Node(inner));
        }
        b
    };

    let bottom = Node::el("div", "mfd-fms-wind-bottom-buttons-container")
        .child(
            Node::el("div", "mfd-fms-wind-return-cancel-wind-container")
                .child(
                    Node::new("div")
                        .style(if m.return_button_visible && !m.draft_winds_exist { "display: inline;" } else { "display: none;" })
                        .child(button_visible(btn("RETURN", false, "width:125px;"), m.return_button_visible)),
                )
                .child(Node::new("div").style(disp(m.draft_winds_exist)).child(button_visible(
                    btn_node(
                        star_label(&["CANCEL", "WIND"], "text-align: center; vertical-align: center; margin-right: 10px;"),
                        false,
                        false,
                        "justify-self: flex-end; height:58px;",
                    ),
                    m.draft_winds_exist,
                ))),
        )
        .child(Node::el("div", "mfd-fms-wind-cpny-button-container").child(cpny))
        .child(Node::el("div", "mfd-fms-wind-insert-button-container").style(disp(m.draft_winds_exist)).child(button_visible(
            btn_node(
                star_label(&["INSERT", "WIND"], "text-align: center; vertical-align: center; margin-right: 10px;"),
                false,
                false,
                "justify-self: flex-end; height:58px;",
            ),
            m.draft_winds_exist,
        )));

    page_container(vec![
        Node::new("div").style("height: 11px;"),
        top_tab_navigator(
            &t,
            vec![
                tab_page(m.selected_tab == 0, Some("padding-bottom:3px;"), hist_page),
                tab_page(m.selected_tab == 1, Some("padding-bottom:3px;"), vec![clb]),
                tab_page(m.selected_tab == 2, None, vec![crz, crz_temp]),
                tab_page(m.selected_tab == 3, None, vec![des]),
            ],
        ),
        bottom,
    ])
}

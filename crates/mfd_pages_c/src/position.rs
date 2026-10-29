use crate::fmt::*;
use crate::w::*;
use crate::x::*;
use mfd_tree::Node;

const NBSP_S: &str = "\u{a0}";

#[derive(Clone, Debug)]
pub struct PositionMonitorModel {
    pub nav_primary_lost: bool,
    pub accuracy_high: bool,
    pub epe: f64,
    pub rnp: Option<f64>,
    pub rnp_entered_by_pilot: bool,
    pub position1: String,
    pub position2: String,
    pub position1_mode: String,
    pub position2_mode: String,
    pub gnss1: String,
    pub gnss2: String,
    pub ir: [String; 3],
    pub ir_deviation: [String; 3],
    pub radio: String,
    pub mix_irs: String,
    pub sensors_visible: bool,
    pub frozen: bool,
    pub frozen_time: String,
    pub on_side_label: String,
    pub gps_deselected: bool,
    pub monitor_waypoint: Option<String>,
    pub bearing: Option<f64>,
    pub distance: Option<f64>,
    pub return_visible: bool,
    pub footer_message: String,
    pub footer_background: String,
}

impl Default for PositionMonitorModel {
    fn default() -> Self {
        let np = NO_POSITION.to_string();
        PositionMonitorModel {
            nav_primary_lost: false,
            accuracy_high: false,
            epe: f64::INFINITY,
            rnp: None,
            rnp_entered_by_pilot: false,
            position1: np.clone(),
            position2: np.clone(),
            position1_mode: String::new(),
            position2_mode: String::new(),
            gnss1: np.clone(),
            gnss2: np.clone(),
            ir: [np.clone(), np.clone(), np.clone()],
            ir_deviation: ["--.-".into(), "--.-".into(), "--.-".into()],
            radio: np.clone(),
            mix_irs: np,
            sensors_visible: false,
            frozen: false,
            frozen_time: String::new(),
            on_side_label: "POS1".into(),
            gps_deselected: false,
            monitor_waypoint: None,
            bearing: None,
            distance: None,
            return_visible: false,
            footer_message: String::new(),
            footer_background: String::new(),
        }
    }
}

fn truthy(v: Option<f64>) -> Option<f64> {
    v.filter(|x| *x != 0.0)
}

pub fn rnp_format(v: Option<f64>) -> String {
    match v {
        None => "--.-".to_string(),
        Some(x) => if x > 10.0 { to_fixed(x, 1) } else { to_fixed(x, 2) },
    }
}

fn vis(on: bool) -> &'static str {
    if on { "visibility: visible;" } else { "visibility: hidden;" }
}

pub fn position_monitor(m: &PositionMonitorModel) -> Node {
    let epe_text = pad_end(&if m.epe.is_infinite() { "-.--".to_string() } else { to_fixed(m.epe, 2) }, 5, '\u{a0}');
    let nav_cls = format!("mfd-value {} bigger mfd-spacing-right", if m.nav_primary_lost { "amber" } else { "" });
    let acc_cls = format!("mfd-value {} bigger mfd-spacing-right", if m.accuracy_high { "" } else { "amber" });
    let dev_vis = |s: &str| s != "--.-";
    let bearing = truthy(m.bearing);
    let bearing_text = bearing.map(|v| pad_start(&to_fixed(v, 0), 3, '0')).unwrap_or_else(|| "---".into());
    let dist = truthy(m.distance);
    let dist_text = dist.map(|v| pad_end(&to_fixed(v, if v > 9999.0 { 0 } else { 1 }), 6, '\u{a0}')).unwrap_or_else(|| "----.-".into());
    let bearing_unit = if bearing.is_some() { "\u{b0}" } else { NBSP_S };
    let dist_unit = if bearing.is_some() { "NM" } else { "\u{a0}\u{a0}" };
    let wpt_entered = m.monitor_waypoint.is_some();
    let wpt_text = m.monitor_waypoint.clone().unwrap_or_else(|| "-------".to_string());

    let rnp_field = {
        let mut f = Field::new(&rnp_format(m.rnp)).units("", "NM").style("width: 155px;").align("center").big_unit(true).min_width(72);
        if !m.rnp_entered_by_pilot {
            f = f.cls(&["computedByFms"]);
        }
        input_field(&f)
    };

    let mut sensors = Node::new("div").style(&format!("visibility:{}", if m.sensors_visible { "visible" } else { "hidden" }));
    sensors = sensors
        .child(
            Node::el("div", "mfd-label-value-container pos-monitor-table-lateral-spacing pos-monitor-table-vertical-spacing")
                .child(Node::el("span", "mfd-label bigger mfd-spacing-right").text("GNSS1"))
                .child(Node::el("span", "mfd-value bigger").style("position: relative; top:5px;").text(&m.gnss1)),
        )
        .child(
            Node::el("div", "mfd-label-value-container pos-monitor-table-lateral-spacing")
                .style("padding-bottom:5px;")
                .child(Node::el("span", "mfd-label bigger mfd-spacing-right").text("GNSS2"))
                .child(Node::el("span", "mfd-value bigger").text(&m.gnss2)),
        )
        .child(Node::el("div", "mfd-pos-monitor-line short"));
    for i in 0..3 {
        let mut row = Node::el("div", "mfd-label-value-container pos-monitor-table-lateral-spacing");
        row = if i < 2 { row.class_if("pos-monitor-table-vertical-spacing", true) } else { row.style("padding-bottom:10px;") };
        sensors = sensors.child(
            row.child(Node::el("span", "mfd-label bigger mfd-spacing-right").text(&format!("{}IRS{}", NBSP_S, i + 1)))
                .child(Node::el("span", "mfd-value bigger").text(&m.ir[i])),
        );
    }
    sensors = sensors
        .child(Node::el("div", "mfd-pos-monitor-line short").text(" "))
        .child(
            Node::el("div", "mfd-label-value-container pos-monitor-table-lateral-spacing")
                .style("padding-bottom:10px;")
                .child(Node::el("span", "mfd-label bigger mfd-spacing-right").text("RADIO"))
                .child(Node::el("span", "mfd-value bigger").text(&m.radio)),
        )
        .child(
            Node::el("div", "mfd-label-value-container")
                .style("padding-left:3px")
                .child(Node::el("span", "mfd-label bigger mfd-spacing-right").text("MIXIRS"))
                .child(Node::el("span", "mfd-value bigger").text(&m.mix_irs)),
        );

    let mut dev = Node::el("div", "mfd-pos-monitor-deviation-container")
        .child(Node::el("span", "mfd-value bigger amber").style(vis(m.gps_deselected)).text("GPS DESELECTED"))
        .child(
            Node::el("div", "fc")
                .style("align-items: flex-end; margin-bottom: 10px; margin-top:17px; margin-right:7px")
                .child(Node::el("span", "mfd-label bigger").text(&format!("DEVIATION FROM {}", m.on_side_label))),
        );
    for i in 0..3 {
        dev = dev.child(
            Node::el("div", "mfd-pos-monitor-irs-deviation-line")
                .child(
                    Node::el("span", "mfd-label bigger mfd-pos-monitor-irs-deviation-title")
                        .style(&format!("visibility:{}", if m.sensors_visible { "hidden" } else { "visible" }))
                        .text(&format!("IRS{}", i + 1)),
                )
                .child(Node::el("span", "mfd-value bigger").text(&m.ir_deviation[i]))
                .child(
                    Node::el("span", "mfd-label-unit bigger mfd-unit-trailing")
                        .style(vis(dev_vis(&m.ir_deviation[i])))
                        .text("NM"),
                ),
        );
    }

    let wpt_field = {
        let mut f = Field::new(&wpt_text).style("\"width:130px;").align("center").min_width(126);
        if !wpt_entered {
            f = f.cls(&["computedByFms"]);
        }
        input_field(&f)
    };

    page_container(vec![
        Node::el("div", "mfd-pos-top-row")
            .child(
                Node::el("div", "mfd-label-value-container")
                    .style("margin-top:13px")
                    .child(Node::el("span", &nav_cls).text(if m.nav_primary_lost { "NAV PRIMARY LOST" } else { "NAV PRIMARY" })),
            )
            .child(
                Node::el("div", "rnp-container")
                    .child(lv(vec![Node::el("span", "mfd-label bigger mfd-spacing-right").text("RNP"), rnp_field])),
            ),
        Node::el("div", "mfd-pos-top-row")
            .child(
                Node::el("div", "mfd-label-value-container")
                    .style(vis(m.nav_primary_lost))
                    .child(Node::el("span", "mfd-label bigger mfd-spacing-right").text("ACCURACY"))
                    .child(Node::el("span", &acc_cls).text(if m.accuracy_high { "HIGH" } else { "LOW" })),
            )
            .child(
                Node::el("div", "mfd-label-value-container")
                    .style("margin-right:95px")
                    .child(Node::el("span", "mfd-label bigger mfd-spacing").style("margin-right: 37px;").text("EPU"))
                    .child(Node::el("span", "mfd-value bigger").text(&epe_text))
                    .child(
                        Node::el("span", "mfd-label-unit bigger mfd-unit-trailing")
                            .style(vis(!m.epe.is_infinite()))
                            .text("NM"),
                    ),
            ),
        pos_line("POS1", &m.position1, &m.position1_mode, "margin-bottom: 14.5px;"),
        pos_line("POS2", &m.position2, &m.position2_mode, "margin-bottom: 3px;"),
        Node::el("div", "mfd-pos-monitor-line big").text(" "),
        Node::el("div", "fr").child(sensors).child(Node::new("div").child(dev)),
        Node::el("div", "fc")
            .child(
                Node::el("div", "mfd-pos-monitor-frozen-pos-time-container")
                    .child(Node::el("span", "mfd-label bigger").text(if m.frozen { "POS DATA FROZEN" } else { "" }))
                    .child(Node::el("span", "mfd-label bigger").text(&if m.frozen { format!("\u{a0}AT\u{a0}{}", m.frozen_time) } else { String::new() })),
            )
            .child(
                Node::el("div", "fr space-between")
                    .style("margin-bottom: 19px;")
                    .child(btn_node(
                        Node::new("div")
                            .style("display: flex; flex-direction: row; justify-content: space-between;")
                            .child(
                                lines_label(&[if m.sensors_visible { "HIDE" } else { "DISPLAY" }, "POS SENSORS"])
                                    .style("text-align: center; vertical-align: center; margin-right: 25px;"),
                            ),
                        false,
                        m.sensors_visible,
                        "width: 219px; margin-left: 95px; height:58px;",
                    ))
                    .child(btn_node(
                        star_label(&[if m.frozen { "UNFREEZE" } else { "FREEZE" }, "POS DATA"], "text-align: center; vertical-align: center; margin-right: 10px;"),
                        false,
                        m.frozen,
                        "width: 212px; margin-right:60px; height:58px",
                    )),
            ),
        Node::el("div", "mfd-pos-monitor-line big"),
        Node::el("div", "fr space-between")
            .child(btn_lines(&["POSITION", "UPDATE"], true, "width: 138px; height:59px; margin-top:18px;"))
            .child(
                Node::new("div")
                    .child(
                        Node::el("div", "mfd-label-value-container")
                            .style("margin-right:16px; padding-top:8px; justify-content:flex-end; padding-bottom:3px;")
                            .child(Node::el("span", "mfd-label mfd-spacing-right-small").style("font-size:22px;").text("BRG / DIST TO"))
                            .child(wpt_field),
                    )
                    .child(
                        Node::el("div", "fr")
                            .style("width:417px; margin-top: 5px;")
                            .child(
                                Node::new("div")
                                    .child(Node::el("span", "mfd-value").text(&bearing_text))
                                    .child(Node::el("span", "mfd-label-unit mfd-unit-trailing").text(&format!("{}{}", NBSP_S, bearing_unit))),
                            )
                            .child(Node::el("span", "mfd-value").style("margin-left:20px;").text("/"))
                            .child(
                                Node::new("div")
                                    .child(Node::el("span", "mfd-value").style("margin-left:14px").text(&dist_text))
                                    .child(Node::el("span", "mfd-label-unit mfd-unit-trailing").text(&format!("{}{}", NBSP_S, dist_unit))),
                            ),
                    ),
            ),
        Node::el("div", "mfd-pos-monitor-line big").style("margin-top:34px;"),
        spacer(),
        Node::el("div", "fr space-between")
            .child(return_button(m.return_visible, "margin-right: 5px; width:150px;"))
            .child(
                Node::el("div", "fr")
                    .child(btn("NAVAIDS", false, "margin-right: 5px; width:149px; height:43px;"))
                    .child(btn("GNSS", true, "margin-right: 5px; width:133px; height:43px;"))
                    .child(btn("IRS", false, "margin-right: 5px; width:136px; height:43px;")),
            ),
        footer_bg(&m.footer_message, &m.footer_background),
    ])
}

fn return_button(visible: bool, style: &str) -> Node {
    let b = btn("RETURN", false, style);
    if visible { b } else { b.style("visibility: hidden;") }
}

fn pos_line(label: &str, pos: &str, mode: &str, style: &str) -> Node {
    Node::el("div", "mfd-pos-monitor-fm-pos-line")
        .style(style)
        .child(lv(vec![
            Node::el("span", "mfd-label bigger mfd-spacing-right").text(&format!("{}{}", NBSP_S, label)),
            Node::el("span", "mfd-value bigger mfd-spacing-right").text(pos),
        ]))
        .child(lv(vec![Node::el("span", "mfd-value bigger").text(mode)]))
}

#[derive(Clone, Debug, Default)]
pub struct PositionIrsModel {
    pub alignment_label: String,
    pub alignment_position: String,
    pub status: [String; 3],
    pub second: [String; 3],
    pub third: [String; 3],
    pub show_data_for: usize,
    pub data_position: String,
    pub true_track: String,
    pub true_heading: String,
    pub ground_speed: String,
    pub magnetic_heading: String,
    pub wind_direction: String,
    pub wind_speed: String,
    pub magnetic_variation: String,
    pub magnetic_variation_unit: String,
    pub gpirs_position: String,
    pub accuracy: String,
}

pub fn irs_alignment_label(aligned: bool, on_ref: bool) -> &'static str {
    match (aligned, on_ref) {
        (true, true) => "IRS ALIGNED ON REF POS:",
        (true, false) => "IRS ALIGNED ON GPS POS:",
        (false, true) => "IRS ALIGNING ON REF POS:",
        (false, false) => "IRS ALIGNING ON GPS POS:",
    }
}

pub fn position_irs(m: &PositionIrsModel) -> Node {
    let row = |i: usize, last: bool, first: bool| {
        let extra = if last { " mfd-position-irs-table-last-row" } else { "" };
        let mut r = Node::el("div", "fr");
        if first {
            r = r.style("margin-top: 40px;");
        }
        r.child(Node::el("div", &format!("mfd-position-irs-table-col1{}", extra)).child(Node::el("span", "mfd-label").text(&format!("IRS {}", i + 1))))
            .child(Node::el("div", &format!("mfd-position-irs-table-col2{}", extra)).child(Node::el("span", "mfd-value bigger").text(&m.status[i])))
            .child(Node::el("div", &format!("mfd-position-irs-table-col3{}", extra)).child(Node::el("span", "mfd-value").text(&m.second[i])))
            .child(Node::el("div", &format!("mfd-position-irs-table-col4{}", extra)).child(Node::el("span", "mfd-value").text(&m.third[i])))
    };
    let cell = |t: &str| {
        Node::new("div")
            .style("flex: 1; display: flex; justify-content: flex-end; align-items: center; padding: 7px;")
            .child(Node::el("span", "mfd-label").text(t))
    };
    let vcell = |flex: &str, kids: Vec<Node>| {
        Node::el("div", "mfd-label-value-container").style(&format!("flex: {}; justify-content: flex-end;", flex)).children(kids)
    };
    let val = |t: &str| Node::el("span", "mfd-value bigger").text(t);
    let data = Node::el("div", "fc")
        .style(&format!("border: 2px outset lightgrey; padding: 2px 15px 15px; visibility: {};", if m.show_data_for == 0 { "hidden" } else { "visible" }))
        .child(
            Node::el("div", "fr")
                .style("justify-content: space-between; margin-bottom: 15px;")
                .child(Node::new("div").style("align-self: flex-start;").child(btn_lines(&["FREEZE", "ALL IRS"], true, "")))
                .child(
                    Node::new("div")
                        .style("align-self: flex-end;")
                        .child(Node::el("span", "mfd-label").style("margin-right: 20px;").text("POSITION"))
                        .child(val(&m.data_position)),
                ),
        )
        .child(
            Node::el("div", "fr")
                .child(cell("T.TRK"))
                .child(vcell("1.5; align-items: center", vec![val(&m.true_track), unit("\u{b0}T", false)]))
                .child(cell("T.HDG"))
                .child(vcell("1", vec![val(&m.true_heading), unit("\u{b0}T", false)])),
        )
        .child(
            Node::el("div", "fr")
                .child(cell("GND SPD"))
                .child(vcell("1.5", vec![val(&m.ground_speed), unit("KT", false)]))
                .child(cell("MAG HDG"))
                .child(vcell("1", vec![val(&m.magnetic_heading), unit("\u{b0}\u{a0}", false)])),
        )
        .child(
            Node::el("div", "fr")
                .style("border-bottom: 2px solid lightgrey; margin-bottom: 15px;")
                .child(cell("T.WIND"))
                .child(vcell("1.5", vec![val(&m.wind_direction), unit("\u{b0}", false), val(&m.wind_speed), unit("KT", false)]))
                .child(cell("MAG VAR"))
                .child(vcell("1", vec![val(&m.magnetic_variation), unit(&m.magnetic_variation_unit, false)])),
        )
        .child(
            Node::el("div", "fc")
                .style("display: flex; align-items: flex-end; padding-right: 15px;")
                .child(lv(vec![
                    Node::el("span", "mfd-label mfd-spacing-right").text("GPIRS POSITION"),
                    Node::el("span", "mfd-value bigger").style("width: 325px;").text(&m.gpirs_position),
                ]))
                .child(lv(vec![
                    Node::el("span", "mfd-label mfd-spacing-right").text("ACCURACY"),
                    Node::el("span", "mfd-value bigger").style("width: 300px; text-align: right;").text(&m.accuracy),
                    Node::el("span", "mfd-label-unit").style("width: 25px;").text("FT"),
                ])),
        );
    page_container(vec![
        Node::el("div", "fr")
            .style("margin: 15px;")
            .child(
                Node::new("div")
                    .style("flex: 1; display: flex; justify-content: center; align-items: center;")
                    .child(Node::el("span", "mfd-label").style("color: #e68000").text(&m.alignment_label)),
            )
            .child(
                Node::new("div")
                    .style("flex: 1 display: flex; justify-content: center; align-items: center;")
                    .child(val(&m.alignment_position)),
            ),
        Node::el("div", "fr")
            .style("padding-bottom: 20px; border-bottom: 2px solid lightgrey;")
            .child(btn_lines(&["ALIGN ON", "OTHER REF"], true, "")),
        row(0, false, true),
        row(1, false, false),
        row(2, true, false),
        Node::el("div", "fr")
            .style("justify-content: flex-end; align-items: center; margin-top: 10px; margin-bottom: 20px; visibility: hidden;")
            .child(Node::el("span", "mfd-label").text("SET HDG"))
            .child(input_field(&Field::new(&mandatory("---.-")).units("", "\u{b0}T").cls(&["mandatory"]).style("width: 150px; margin-left: 10px;").align("flex-end").min_width(90))),
        Node::el("div", "mfd-position-irs-irs-button-row")
            .child(btn_sel("IRS1", m.show_data_for == 1, "width: 150px; margin-right: 3px;"))
            .child(btn_sel("IRS2", m.show_data_for == 2, "width: 150px; margin-right: 3px;"))
            .child(btn_sel("IRS3", m.show_data_for == 3, "width: 150px;")),
        data,
        spacer(),
        Node::new("div").style("width: 150px;").child(btn("RETURN", false, "margin-right: 5px;")),
    ])
}

#[derive(Clone, Debug, Default)]
pub struct NavaidTuned {
    pub ident: String,
    pub ident_entered: bool,
    pub freq: String,
    pub freq_entered: bool,
    pub course: String,
    pub class: String,
}

#[derive(Clone, Debug, Default)]
pub struct SelectedNavaid {
    pub ident: String,
    pub freq_or_chan: String,
    pub class: String,
}

#[derive(Clone, Debug, Default)]
pub struct PositionNavaidsModel {
    pub selected_tab: usize,
    pub vor: [NavaidTuned; 2],
    pub ls_ident: String,
    pub ls_ident_entered: bool,
    pub ls_freq: String,
    pub ls_freq_entered: bool,
    pub ls_course: String,
    pub ls_course_entered: bool,
    pub ls_slope: String,
    pub ls_class: String,
    pub selected: [SelectedNavaid; 3],
    pub radio_nav_mode: String,
    pub radio_nav_position: String,
    pub deselected: [String; 6],
    pub return_visible: bool,
    pub cap_or_fo: String,
    pub footer_message: String,
}

fn row_field(text: &str, entered: Option<bool>, unit_trailing: &str, style: &str, max_digits: u32) -> Node {
    let mut f = Field::new(text).units("", unit_trailing).style(style).align("center").min_width(digits_width(max_digits));
    if entered == Some(false) {
        f = f.cls(&["computedByFms"]);
    }
    input_field(&f)
}

pub fn position_navaids(m: &PositionNavaidsModel) -> Node {
    let r = |kids: Vec<Node>| Node::el("div", "mfd-position-navaids-row").children(kids);
    let lab = |t: &str| Node::el("div", "mfd-label mfd-position-navaids-row").text(t);
    let vor_col = |i: usize, name: &str| {
        let v = &m.vor[i];
        Node::new("div")
            .style(if i == 0 { " flex-direction: column; width: 40%; justify-content: space-between;" } else { "display: flex; flex-direction: column; width: 40%;" })
            .child(lab(name))
            .child(r(vec![row_field(&v.ident, Some(v.ident_entered), "", "width: 125px;", 4)]))
            .child(r(vec![row_field(&v.freq, Some(v.freq_entered), "", "width: 125px;", 6)]))
            .child(r(vec![row_field(&v.course, None, "\u{b0}", "width: 125px;", 4)]))
            .child(r(vec![Node::el("span", "mfd-value").text(&v.class)]))
    };
    let tuned = vec![
        Node::new("div")
            .style("display: flex; flex-direction: row; align-self: center; padding-bottom: 20px;")
            .child(vor_col(0, "VOR1"))
            .child(
                Node::new("div")
                    .style("display: flex; flex-direction: column; width: 20%; margin-left: 40px; margin-right: 40px;")
                    .child(lab(""))
                    .child(lab("IDENT"))
                    .child(lab("FREQ"))
                    .child(lab("CRS"))
                    .child(lab("CLASS")),
            )
            .child(vor_col(1, "VOR2")),
        Node::new("div").style("height: 5px; width: 100%; border-bottom: 2px solid darkgrey;"),
    ];
    let nav_button = |i: usize| {
        let s = &m.selected[i];
        let empty = s.ident.is_empty();
        let b = menu_button_items(&s.ident, true, "min-width: 107px;", &format!("{}_MFD_dataNavaid", m.cap_or_fo), &["DATA NAVAID"]);
        Node::el("div", "mfd-label br").child(Node::new("div").class_if("invisible", empty).child(b))
    };
    let mut table = Node::el("div", "mfd-pos-nav-nav-table")
        .child(Node::el("div", "mfd-label br bb").text("IDENT"))
        .child(Node::el("div", "mfd-label br bb").text("FREQ/CHAN"))
        .child(Node::el("div", "mfd-label bb").text("CLASS"));
    for i in 0..3 {
        table = table
            .child(nav_button(i))
            .child(Node::el("div", "mfd-value br").text(&m.selected[i].freq_or_chan))
            .child(Node::el("div", "mfd-value").text(&m.selected[i].class));
    }
    let dsel = |i: usize| {
        let disabled = i > 0 && m.deselected[i - 1].is_empty();
        let mut f = Field::new(if m.deselected[i].is_empty() { "-" } else { &m.deselected[i] }).align("center").min_width(digits_width(4));
        f.container_inactive = false;
        let mut n = input_field(&f);
        if disabled {
            n = disabled_field(n);
        }
        Node::new("div").child(n)
    };
    let selected = vec![
        table,
        Node::new("div")
            .style("display: grid; grid-template-columns: 190px 320px; margin-left: 80px;")
            .child(Node::el("div", "mfd-label").style("justify-content: right; margin-bottom: 20px;").text("RADIO NAV MODE"))
            .child(Node::el("div", "mfd-value").style("justify-content: left; margin-left: 20px; margin-bottom: 20px;").text(&m.radio_nav_mode))
            .child(Node::el("div", "mfd-label").style("justify-content: right; margin-bottom: 20px;").text("RADIO POSITION"))
            .child(Node::el("div", "mfd-value").style("justify-content: left; margin-left: 20px; margin-bottom: 20px;").text(&m.radio_nav_position)),
        Node::new("div").style("border-bottom: 1px solid lightgrey; width: 100%; height: 3px; margin-bottom: 15px;"),
        Node::el("div", "mfd-label").style("padding-left: 15px; margin-bottom: 10px;").text("LIST OF DESELECTED NAVAIDS"),
        Node::new("div")
            .style("width: 45%; display: flex; justify-content: space-between; margin-bottom: 10px;")
            .child(dsel(0))
            .child(dsel(1))
            .child(dsel(2)),
        Node::new("div").style("width: 45%; display: flex; justify-content: space-between;").child(dsel(3)).child(dsel(4)).child(dsel(5)),
    ];
    let t = Tabs::new(&["TUNED FOR DISPLAY", "SELECTED FOR FMS NAV"], m.selected_tab).angle(25.0);
    let ls_lab = |t: &str| Node::el("div", "mfd-label mfd-position-navaids-row ar").text(t);
    page_container(vec![
        top_tab_navigator(&t, vec![tab_page(m.selected_tab == 0, None, tuned), tab_page(m.selected_tab == 1, None, selected)]),
        Node::new("div")
            .style("display: flex; flex-direction: row; align-self: center;")
            .child(
                Node::new("div")
                    .style("display: flex; flex-direction: column; margin-right: 20px;")
                    .child(lab(""))
                    .child(ls_lab("IDENT"))
                    .child(ls_lab("FREQ/CHAN"))
                    .child(ls_lab("CRS"))
                    .child(ls_lab("SLOPE"))
                    .child(ls_lab("CLASS")),
            )
            .child(
                Node::new("div")
                    .style("display: flex; flex-direction: column;")
                    .child(lab("LS"))
                    .child(r(vec![row_field(&m.ls_ident, Some(m.ls_ident_entered), "", "width: 125px;", 4)]))
                    .child(r(vec![row_field(&m.ls_freq, Some(m.ls_freq_entered), "", "width: 125px;", 6)]))
                    .child(r(vec![row_field(&m.ls_course, Some(m.ls_course_entered), "\u{b0}", "width: 125px;", 4)]))
                    .child(r(vec![lv(vec![Node::el("span", "mfd-value").text(&m.ls_slope), unit("\u{b0}", false)])]))
                    .child(r(vec![Node::el("span", "mfd-value").text(&m.ls_class)])),
            )
            .child(
                Node::new("div")
                    .style("display: flex; flex-direction: column; justify-content: center; align-items: center; margin-left: 15px;")
                    .child(btn_node(star_label(&["DESELECT", "GLIDE"], "text-align: center; vertical-align: center; margin-right: 10px;"), true, false, "width: 225px;")),
            ),
        spacer(),
        Node::new("div").style("width: 150px;").child(visible(btn("RETURN", false, "margin-right: 5px;"), m.return_visible)),
    ])
}

fn digits_width(max_digits: u32) -> u32 {
    (max_digits as f64 * 27.0 / 1.5).round() as u32
}

fn visible(mut n: Node, on: bool) -> Node {
    if !on {
        n.inline = n.inline.as_ref().map(|s| s.replacen("visibility: inherit", "visibility: hidden", 1));
    }
    n
}

fn grey_arrows(mut n: Node) -> Node {
    for s in &mut n.shapes {
        if let Some(st) = s.a.get_mut("style") {
            *st = st.replace("fill: white;", "fill: grey;");
        }
    }
    n.kids = n
        .kids
        .into_iter()
        .map(|k| match k {
            mfd_tree::Kid::Node(c) => mfd_tree::Kid::Node(grey_arrows(c)),
            t => t,
        })
        .collect();
    n
}

fn disabled_field(n: Node) -> Node {
    fn walk(mut n: Node) -> Node {
        if n.cls.iter().any(|c| c == "mfd-input-field-container") {
            n.cls.push("disabled".into());
        }
        if n.cls.iter().any(|c| c == "mfd-input-field-text-input") {
            n.cls.push("disabled".into());
        }
        n.kids = n
            .kids
            .into_iter()
            .map(|k| match k {
                mfd_tree::Kid::Node(c) => mfd_tree::Kid::Node(walk(c)),
                t => t,
            })
            .collect();
        n
    }
    walk(n)
}

fn menu_button_items(text: &str, disabled: bool, style: &str, prefix: &str, items: &[&str]) -> Node {
    let n = btn_menu(text, disabled, style, prefix, items, true);
    if disabled {
        grey_arrows(n)
    } else {
        n
    }
}

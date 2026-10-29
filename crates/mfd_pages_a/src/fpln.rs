use crate::shell::{fms_display_unit, ShellModel};
use crate::widgets::*;
use mfd_tree::Node;
use serde::Deserialize;

const ACTIVE: &str = "#00ff00";
const TEMPORARY: &str = "#ffff00";
const SECONDARY: &str = "#ffffff";
const ALTERNATE: &str = "#00ffff";

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct LineData {
    pub kind: String,
    pub original_leg_index: Option<usize>,
    pub label: String,
    pub ident: String,
    pub overfly: bool,
    pub annotation: String,
    pub eta_ms: Option<f64>,
    pub is_pseudo: bool,
    pub is_altn: bool,
    pub is_missed: bool,
    pub transition_altitude: Option<f64>,
    pub altitude_prediction: Option<f64>,
    pub has_altitude_constraint: bool,
    pub altitude_constraint_altitude1: Option<f64>,
    pub altitude_constraint_altitude2: Option<f64>,
    pub altitude_constraint_kind: String,
    pub altitude_constraint_respected: bool,
    pub speed_prediction: Option<f64>,
    pub has_speed_constraint: bool,
    pub speed_constraint: Option<f64>,
    pub speed_constraint_respected: bool,
    pub efob_prediction_kg: Option<f64>,
    pub wind_dir: Option<String>,
    pub wind_dir_full: Option<String>,
    pub wind_mag: Option<String>,
    pub track: Option<f64>,
    pub dist: Option<f64>,
    pub fpa: Option<f64>,
    pub hold_speed: Option<f64>,
    pub imm_exit: bool,
    pub decel_sequenced: bool,
    pub is_active_hold_leg: bool,
    pub first_line: bool,
    pub before_special: bool,
    pub after_special: bool,
    pub before_active_leg: bool,
    pub is_active_leg: bool,
    pub last_line: bool,
    pub selected: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DestRow {
    pub loaded: bool,
    pub button_label: String,
    pub time_ms: Option<f64>,
    pub efob_kg: Option<f64>,
    pub efob_amber: bool,
    pub distance: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct NewDestBox {
    pub open: bool,
    pub from_ident: String,
    pub text: String,
    pub input: InputState,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct InsertNextBox {
    pub open: bool,
    pub from_ident: String,
    pub coordinates: String,
    pub text: String,
    pub caret: String,
    pub options: Vec<String>,
    pub input: InputState,
    pub dropdown_open: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct FplnModel {
    pub title: String,
    pub line_color: String,
    pub preflight: bool,
    pub display_efob_and_wind: bool,
    pub true_track: bool,
    pub weight_kg: bool,
    pub lines: Vec<LineData>,
    pub dest: DestRow,
    pub tmpy_active: bool,
    pub active_plan: bool,
    pub scroll_down_disabled: bool,
    pub scroll_up_disabled: bool,
    pub spd_alt_open: bool,
    pub info_open: bool,
    pub sec_active: bool,
    pub show_init: bool,
    pub sec_index: usize,
    pub revisions_menu: Vec<MenuItem>,
    pub revisions_menu_open: bool,
    pub revisions_menu_positioned: bool,
    pub tmpy_line: Option<bool>,
    pub new_dest: NewDestBox,
    pub insert_next: InsertNextBox,
}

impl Default for FplnModel {
    fn default() -> Self {
        FplnModel {
            title: "ACTIVE/F-PLN".into(),
            line_color: "active".into(),
            preflight: true,
            display_efob_and_wind: false,
            true_track: false,
            weight_kg: true,
            lines: Vec::new(),
            dest: DestRow::default(),
            tmpy_active: false,
            active_plan: true,
            scroll_down_disabled: true,
            scroll_up_disabled: false,
            spd_alt_open: false,
            info_open: false,
            sec_active: false,
            show_init: false,
            sec_index: 1,
            revisions_menu: Vec::new(),
            revisions_menu_open: false,
            revisions_menu_positioned: false,
            tmpy_line: None,
            new_dest: NewDestBox::default(),
            insert_next: InsertNextBox::default(),
        }
    }
}

fn color_hex(name: &str) -> &'static str {
    match name {
        "temporary" => TEMPORARY,
        "secondary" => SECONDARY,
        "alternate" => ALTERNATE,
        _ => ACTIVE,
    }
}

fn utc_hhmm(ms: f64) -> String {
    let secs = (ms / 1000.0).floor() as i64;
    let day = secs.rem_euclid(86400);
    format!("{:02}:{:02}", day / 3600, (day % 3600) / 60)
}

fn mono(text: &str) -> Node {
    Node::new("span").style("font-family: FBW-Display-EIS-A380-SlashedZero, monospace;").text(text)
}

fn speed_text(v: f64) -> String {
    if v > 2.0 {
        format!("{:.0}", v)
    } else {
        format!(".{}", format!("{:.2}", v).split('.').nth(1).unwrap_or(""))
    }
}

fn js_round(v: f64) -> f64 {
    (v + 0.5).floor()
}

fn format_speed(d: &LineData, prev: Option<&LineData>) -> Node {
    let mut speed = mono("---");
    let repeats = prev.map_or(false, |p| {
        p.kind == "waypoint"
            && match (d.speed_prediction, p.speed_prediction) {
                (Some(s), Some(ps)) if s != 0.0 && ps != 0.0 => {
                    ((s >= 2.0 && (ps - s).abs() < 1.0) || (s < 2.0 && (ps - s).abs() < 0.01)) && !d.has_speed_constraint
                }
                _ => false,
            }
    }) && !(d.after_special || d.first_line);
    if repeats {
        speed = Node::new("span")
            .style("padding-left: 3px; padding-right: 3px; font-family: FBW-Display-EIS-A380-SlashedZero, monospace;")
            .text("\"");
    } else if let Some(s) = d.speed_prediction.filter(|s| s.is_finite() && *s > 0.0) {
        speed = mono(&speed_text(s));
    }
    if d.has_speed_constraint && d.speed_prediction.map_or(false, |s| s != 0.0) {
        let cls = if d.speed_constraint_respected { "mfd-fms-fpln-leg-constraint-respected" } else { "mfd-fms-fpln-leg-constraint-missed" };
        return Node::new("span").child(Node::el("span", cls).text("*")).child(Node::new("span").child(speed));
    }
    if d.has_speed_constraint && d.speed_constraint.map_or(false, |s| s != 0.0) && d.speed_prediction.map_or(true, |s| s == 0.0) {
        return Node::el("span", "mfd-fms-fpln-leg-constraint-respected")
            .style("margin-left: 20px; font-size: 25px;")
            .text(&speed_text(d.speed_constraint.unwrap_or(0.0)));
    }
    Node::new("span").style("margin-left: 20px;").child(speed)
}

fn fl_text(v: f64) -> String {
    format!("FL{:03}", js_round(v / 100.0) as i64)
}

fn format_altitude(d: &LineData, prev: Option<&LineData>) -> Vec<Node> {
    let mut alt = mono("---");
    if let Some(p) = d.altitude_prediction.filter(|p| *p != 0.0) {
        let below_trans = d.transition_altitude.map_or(true, |t| p < t);
        let repeats = prev.map_or(false, |pr| {
            pr.kind == "waypoint" && pr.altitude_prediction.map_or(false, |pa| pa != 0.0 && (pa - p).abs() < 100.0)
        }) && !d.has_altitude_constraint
            && !(d.after_special || d.first_line);
        if repeats {
            alt = mono("\"");
        } else if !below_trans {
            alt = mono(&fl_text(p));
        } else {
            alt = mono(&format!("{:.0}", (p / 10.0).round() * 10.0));
        }
    }
    let pred = d.altitude_prediction.filter(|p| *p != 0.0);
    if d.has_altitude_constraint && pred.is_some() {
        let cls = if d.altitude_constraint_respected { "mfd-fms-fpln-leg-constraint-respected" } else { "mfd-fms-fpln-leg-constraint-missed" };
        return vec![Node::el("span", cls).text("*"), Node::new("span").child(alt)];
    }
    if d.has_altitude_constraint && d.altitude_constraint_altitude1.map_or(false, |a| a != 0.0) && pred.is_none() {
        let a1 = d.altitude_constraint_altitude1.unwrap_or(0.0);
        let below_trans = d.transition_altitude.map_or(true, |t| a1 < t);
        let cstr = if below_trans { format!("{:.0}", a1) } else { format!("FL{}", js_round(a1 / 100.0) as i64) };
        let kind = match d.altitude_constraint_kind.as_str() {
            "above" => "+",
            "below" => "-",
            _ => "",
        };
        let shown = if d.altitude_constraint_altitude2.map_or(false, |a| a != 0.0) { "WINDOW".to_string() } else { format!("{}{}", kind, cstr) };
        return vec![Node::el("span", "mfd-fms-fpln-leg-constraint-respected").style("font-size: 25px;").text(&shown)];
    }
    vec![Node::new("span").style("margin-left: 20px; font-family: FBW-Display-EIS-A380-SlashedZero, monospace;").child(alt)]
}

fn format_wind(d: &LineData, prev: Option<&LineData>) -> Node {
    let force_full = d.after_special || d.first_line;
    let prev_wp = prev.filter(|p| p.kind == "waypoint");
    let dir = match (&d.wind_dir, prev_wp.and_then(|p| p.wind_dir.as_ref())) {
        (Some(c), Some(p)) if c == p && !force_full => " \" ".to_string(),
        (Some(_), _) => d.wind_dir_full.clone().or(d.wind_dir.clone()).unwrap_or_default(),
        _ => "---".to_string(),
    };
    let mag = match (&d.wind_mag, prev_wp.and_then(|p| p.wind_mag.as_ref())) {
        (Some(c), Some(p)) if c == p && !force_full => " \" ".to_string(),
        (Some(c), _) => c.clone(),
        _ => "--".to_string(),
    };
    Node::new("div")
        .style("display: flex; flex-direction: row; justify-self: flex-end; font-family: FBW-Display-EIS-A380-SlashedZero, monospace;")
        .child(Node::new("span").style("width: 45px; text-align: center;").text(&dir))
        .child(Node::new("span").text("/"))
        .child(Node::new("span").style("width: 45px; text-align: center;").text(&mag))
}

fn connector(m: &FplnModel, d: &LineData, color: &str) -> Option<Node> {
    let line = |x1: &str, y1: &str, x2: &str, y2: &str, style: Option<String>, depth: usize| {
        let mut a = vec![("x1", x1), ("y1", y1), ("x2", x2), ("y2", y2)];
        let s;
        if let Some(st) = style {
            s = st;
            a.push(("style", &s));
            shape("line", depth, &a)
        } else {
            shape("line", depth, &a)
        }
    };
    let stroke = |c: &str| format!("stroke:{};stroke-width:2", c);
    let group = |c: &str| shape("g", 0, &[("style", &format!("fill:none;stroke:{};stroke-width:2", c))]);
    let last_or_before = d.last_line || d.before_special;
    let dims = |h: &str| [("height", h.to_string()), ("width", "30".to_string())];
    let mk = |h: &str, shapes: Vec<mfd_tree::Shape>| {
        let [a, b] = dims(h);
        svg("", &[(a.0, &a.1), (b.0, &b.1)], shapes)
    };
    let _ = m;
    if d.first_line && d.before_active_leg {
        return None;
    }
    if d.first_line {
        let active_white = d.is_active_leg && color == ACTIVE;
        let poly_color = if d.is_active_leg && color != TEMPORARY { "#fff" } else { color };
        return Some(mk(
            "40",
            vec![
                line("15", "40", "15", "30", Some(stroke(if d.before_special { "#000" } else { color })), 0),
                line("8", "18", "0", "18", Some(stroke(if active_white { "#fff" } else { color })), 0),
                shape("g", 0, &[("style", &format!("fill:none;stroke:{};stroke-width:2", poly_color))]),
                shape("polyline", 1, &[("points", "15,31 8,19 15,5 22,19 15,31")]),
            ],
        ));
    }
    if d.after_special {
        return Some(mk(
            "72",
            vec![
                line("15", "72", "15", "63", Some(stroke(if last_or_before { "#000" } else { color })), 0),
                group(color),
                shape("polyline", 1, &[("points", "15,63 8,51 15,37 22,51 15,63")]),
                line("8", "50", "0", "50", None, 1),
            ],
        ));
    }
    if d.is_active_leg {
        let gcol = if color == ACTIVE { "#fff" } else { color };
        return Some(mk(
            "72",
            vec![
                line("15", "72", "15", "62", Some(stroke(color)), 0),
                group(gcol),
                line("15", "9", "15", "37", None, 1),
                shape("polyline", 1, &[("points", "15,63 8,51 15,37 22,51 15,63")]),
                line("8", "50", "0", "50", None, 1),
                line("15", "10", "30", "10", None, 1),
            ],
        ));
    }
    if d.is_pseudo {
        return Some(mk(
            "72",
            vec![
                line("15", "72", "15", "50", Some(stroke(if last_or_before { "#000" } else { color })), 0),
                group(color),
                line("15", "0", "15", "50", None, 1),
                line("15", "50", "0", "50", None, 1),
                line("15", "10", "30", "10", None, 1),
            ],
        ));
    }
    Some(mk(
        "72",
        vec![
            line("15", "72", "15", "63", Some(stroke(if last_or_before { "#000" } else { color })), 0),
            group(color),
            line("15", "0", "15", "37", None, 1),
            shape("polyline", 1, &[("points", "15,63 8,51 15,37 22,51 15,63")]),
            line("8", "50", "0", "50", None, 1),
            line("15", "10", "30", "10", None, 1),
        ],
    ))
}

fn hold_connector(color: &str) -> Node {
    svg(
        "",
        &[("height", "72"), ("width", "30")],
        vec![shape("line", 0, &[("x1", "15"), ("y1", "72"), ("x2", "15"), ("y2", "0"), ("style", &format!("stroke:{};stroke-width:2", color))])],
    )
}

fn weight_text(kg: Option<f64>, metric: bool) -> String {
    match kg {
        Some(v) if v.is_finite() => {
            let x = if metric { v } else { v * 2.2046226218 } / 1000.0;
            format!("{:.1}", x)
        }
        _ => "---.-".into(),
    }
}

fn hold_exit_button(d: &LineData) -> Node {
    let top = if d.imm_exit { "RESUME" } else { "IMMEDIATE" };
    let bottom = if d.imm_exit { "HOLD" } else { "EXIT" };
    let label = Node::new("div")
        .style("display: flex; flex-direction: row; justify-content: space-between;")
        .child(
            Node::new("span")
                .style("text-align: center; vertical-align: center; margin-right: 10px;")
                .text(top)
                .child(Node::new("br"))
                .text(bottom),
        )
        .child(Node::new("span").style("display: flex; align-items: center; justify-content: center;").text("*"));
    button(ButtonSpec::node(label).style("color: #e68000; padding-right: 2px; width:200px;"))
}

fn render_line(m: &FplnModel, d: &LineData, prev: Option<&LineData>) -> Node {
    let first = d.first_line;
    let size = if first { "height: 40px; margin-top: 16px;" } else { "height: 72px;" };
    let active_cls = |n: Node| n.class_if("mfd-fms-fpln-leg-active", d.is_active_leg);
    if d.kind == "special" {
        let delim = if d.label.len() > 13 { "- - - - -" } else { "- - - - - - " };
        let line = Node::el("div", "mfd-fms-fpln-line mfd-fms-fpln-line-special")
            .style(&format!("font-size: 30px; {}", size))
            .class_if("mfd-fms-fpln-leg-active", d.is_active_leg)
            .text(delim)
            .child(Node::new("span").style("margin: 0px 15px 0px 15px;").text(&d.label))
            .text(delim);
        return Node::new("div").child(line);
    }
    let is_hold = d.kind == "hold";
    let line_color = if d.is_altn || d.is_missed { "alternate" } else { m.line_color.as_str() };
    let global_active = m.line_color == "active";
    let mut ident_div = active_cls(Node::el("div", "mfd-fms-fpln-line-ident")).class_if("selected", d.selected);
    if d.overfly && !is_hold {
        ident_div = ident_div.child(
            Node::new("span").text(&d.ident).child(Node::new("span").style("font-size: 24px; vertical-align: baseline;").text("@")),
        );
    } else {
        ident_div = ident_div.text(&d.ident);
    }
    let mut col1 = Node::new("div").style("width: 25%; display: flex; flex-direction: column;");
    if !first {
        col1 = col1.child(Node::el("div", "mfd-fms-fpln-line-annotation").text(if is_hold { "" } else { &d.annotation }));
    }
    col1 = col1.child(ident_div);

    let time_clickable = !is_hold;
    let (speed_clickable, alt_clickable) = if is_hold { (false, false) } else if m.display_efob_and_wind { (false, true) } else { (true, true) };
    let time_text = if is_hold {
        d.hold_speed.map(|s| format!("{:.0}", s)).unwrap_or_default()
    } else if global_active {
        d.eta_ms.filter(|e| *e != 0.0).map(utc_hhmm).unwrap_or_else(|| "--:--".into())
    } else {
        "--:--".into()
    };
    let mut col2 = Node::el("div", "mfd-fms-fpln-label-small").class_if("clickable", time_clickable).style("width: 11.5%;");
    if !first {
        col2 = col2.child(Node::el("div", "mfd-fms-fpln-line-annotation").text(if is_hold { "SPD" } else { "" }));
    }
    col2 = col2.child(active_cls(Node::el("div", "mfd-fms-fpln-leg-lower-row")).text(&time_text));

    let mut speed_lower = active_cls(Node::el("div", "mfd-fms-fpln-leg-lower-row"));
    let mut alt_lower = active_cls(Node::el("div", "mfd-fms-fpln-leg-lower-row"));
    if m.display_efob_and_wind && !is_hold {
        speed_lower = speed_lower.style("padding-left: 10px;");
        alt_lower = alt_lower.style("align-self: flex-end; padding-right: 20px;");
    }
    if is_hold {
        if d.decel_sequenced || d.is_active_hold_leg {
            speed_lower = speed_lower.child(hold_exit_button(d));
        }
    } else {
        let speed_node = if m.display_efob_and_wind {
            match d.efob_prediction_kg.filter(|v| v.is_finite()) {
                Some(v) if global_active => Node::new("span").text(&weight_text(Some(v), m.weight_kg)),
                _ => Node::new("span").text("--.-"),
            }
        } else if global_active {
            format_speed(d, prev)
        } else {
            Node::new("span").text("---")
        };
        speed_lower = speed_lower.child(speed_node);
        if m.display_efob_and_wind {
            alt_lower = if global_active {
                alt_lower.child(format_wind(d, prev))
            } else {
                alt_lower.child(Node::new("span").text("---\u{b0}/---"))
            };
        } else if global_active {
            alt_lower = alt_lower.children(format_altitude(d, prev));
        } else {
            alt_lower = alt_lower.child(Node::new("span").text("---"));
        }
    }
    let mut col3 = Node::el("div", "mfd-fms-fpln-label-small")
        .class_if("clickable", speed_clickable)
        .style("width: 15%; align-items: flex-start; padding-left: 40px;");
    let mut col4 = Node::el("div", "mfd-fms-fpln-label-small")
        .class_if("clickable", alt_clickable)
        .style("width: 21%; align-items: flex-start; padding-left: 20px;");
    if !first {
        col3 = col3.child(Node::el("div", "mfd-fms-fpln-leg-upper-row"));
        col4 = col4.child(Node::el("div", "mfd-fms-fpln-leg-upper-row"));
    }
    col3 = col3.child(speed_lower);
    col4 = col4.child(alt_lower);

    let mut col5 = active_cls(Node::el("div", "mfd-fms-fpln-label-small")).style("width: 30px; margin-right: 5px;");
    let hex = color_hex(line_color);
    if is_hold {
        col5 = col5.child(hold_connector(hex));
    } else if let Some(c) = connector(m, d, hex) {
        col5 = col5.child(c);
    }

    let (track, dist, fpa) = if is_hold || d.after_special {
        (String::new(), String::new(), String::new())
    } else {
        (
            d.track
                .filter(|t| *t != 0.0)
                .map(|t| format!("{:03.0}\u{b0}{}", t, if m.true_track { "T" } else { "" }))
                .unwrap_or_default(),
            d.dist.map(|v| format!("{:.0}", v)).unwrap_or_default(),
            d.fpa.filter(|v| *v != 0.0).map(|v| format!("{:.1}", v)).unwrap_or_default(),
        )
    };
    let small = |w: &str, align: &str, upper: &str| {
        let mut n = Node::el("div", "mfd-fms-fpln-label-small").style(&format!("width: {}; align-items: {};", w, align));
        if !first {
            n = n.child(active_cls(Node::el("div", "mfd-fms-fpln-leg-upper-row")).text(upper));
        }
        n.child(active_cls(Node::el("div", "mfd-fms-fpln-leg-lower-row")))
    };
    let line = Node::el("div", "mfd-fms-fpln-line")
        .class_if("mfd-fms-fpln-line-temporary", line_color == "temporary")
        .class_if("mfd-fms-fpln-line-secondary", line_color == "secondary")
        .class_if("mfd-fms-fpln-line-altn", line_color == "alternate")
        .style(&format!("{};", size))
        .child(col1)
        .child(col2)
        .child(col3)
        .child(col4)
        .child(col5)
        .child(small("9%", "flex-start", &track))
        .child(small("6%", "flex-end", &dist))
        .child(small("8%", "flex-end", &fpa));
    Node::new("div").child(line)
}

fn dialog_new_dest(b: &NewDestBox) -> Node {
    Node::new("div").style("position: relative;").child(
        Node::el("div", "mfd-dialog mfd-fms-new-dest-box")
            .child(
                Node::el("div", "mfd-fms-new-dest-box-inner")
                    .child(
                        Node::el("span", "mfd-label")
                            .text("NEW DEST FROM ")
                            .child(Node::el("span", "mfd-value bigger").text(&b.from_ident)),
                    )
                    .child(
                        Node::new("div")
                            .style("align-self: center; margin-top: 50px;")
                            .child(input_field(InputSpec {
                                max_digits: 4,
                                ..InputSpec::new(if b.text.is_empty() { "----" } else { &b.text }).state(b.input.clone())
                            })),
                    ),
            )
            .child(
                Node::el("div", "fr")
                    .style("justify-content: space-between")
                    .child(button(ButtonSpec::new("CANCEL"))),
            ),
    )
    .style(&format!("position: relative; display: {};", if b.open { "block" } else { "none" }))
}

fn dialog_insert_next(side: &str, b: &InsertNextBox) -> Node {
    let text = if b.text.is_empty() && b.caret.is_empty() { "-------".to_string() } else { b.text.clone() };
    let mut input = InputSpec::new(&text).align("flex-start");
    input.caret = b.caret.clone();
    input.state = b.input.clone();
    let dd = dropdown_menu(DropdownSpec {
        id_prefix: format!("{}_MFD_insertNextWptDropdown", side),
        input,
        options: b.options.clone(),
        container_style: "width: 175px;".into(),
        align: "flex-start".into(),
        open: b.dropdown_open,
        digits: 7,
        ..Default::default()
    });
    Node::new("div")
        .style(&format!("position: relative; display: {};", if b.open { "block" } else { "none" }))
        .child(
            Node::el("div", "mfd-dialog mfd-fms-insert-next-wpt-box")
                .child(
                    Node::el("div", "mfd-fms-insert-next-wpt-box-inner")
                        .child(
                            Node::el("span", "mfd-label")
                                .text("INSERT NEXT WPT FROM ")
                                .child(Node::el("span", "mfd-value bigger").text(&b.from_ident)),
                        )
                        .child(
                            Node::new("span")
                                .style("margin-left: 50px; margin-top: 10px;")
                                .child(Node::el("span", "mfd-value bigger").text(&b.coordinates)),
                        )
                        .child(Node::new("div").style("margin-left: 50px; margin-top: 10px;").child(dd)),
                )
                .child(
                    Node::el("div", "fr")
                        .style("justify-content: space-between")
                        .child(button(ButtonSpec::new("CANCEL"))),
                ),
        )
}

fn tmpy_button(top: &str) -> Node {
    let label = Node::new("div")
        .style("display: flex; flex-direction: row; justify-content: space-between;")
        .child(
            Node::new("span")
                .style("text-align: center; vertical-align: center; margin-right: 10px;")
                .text(top)
                .child(Node::new("br"))
                .text("TMPY"),
        )
        .child(Node::new("span").style("display: flex; align-items: center; justify-content: center;").text("*"));
    button(ButtonSpec::node(label).style("color: #e68000; padding-right: 2px;"))
}

pub fn fpln_page(shell: &ShellModel, m: &FplnModel) -> Vec<Node> {
    let side = shell.side.as_str();
    let temporary = m.line_color == "temporary";
    let dest = &m.dest;
    let dest_not_loaded = !dest.loaded;
    let time_not_avail_active = !m.tmpy_active && dest.time_ms.is_none() && m.active_plan;
    let efob_na = dest.efob_kg.map_or(true, |v| !v.is_finite());
    let efob_na_active = !m.tmpy_active && efob_na && m.active_plan;
    let dist_na_active = !m.tmpy_active && dest.distance.is_none() && m.active_plan;

    let spd_alt_label = if m.display_efob_and_wind {
        format!("EFOB{n}{n}{n}{n}T.WIND", n = NBSP)
    } else {
        format!("{n}SPD{n}{n}{n}{n}ALT{n}{n}{n}", n = NBSP)
    };
    let spd_alt_item = if m.display_efob_and_wind {
        format!("SPD{n}{n}{n}{n}{n}ALT", n = NBSP)
    } else {
        format!("EFOB{n}{n}{n}{n}T.WIND", n = NBSP)
    };
    let spd_alt_btn = button(
        ButtonSpec::node(Node::el("div", "mfd-fms-fpln-button-speed-alt").text(&spd_alt_label))
            .style("margin-right: 5px; width: 260px; height: 43px;")
            .menu(&format!("{}_MFD_windbtn", side), vec![MenuItem::new(&spd_alt_item, false)], false)
            .opened(m.spd_alt_open),
    );
    let header = Node::el("div", "mfd-fms-fpln-header")
        .child(Node::el("div", "mfd-fms-fpln-header-from").child(Node::el("span", "mfd-label").text("FROM")))
        .child(
            Node::el("div", "mfd-fms-fpln-header-time")
                .child(Node::el("span", "mfd-label").text(if m.preflight { "TIME" } else { "UTC" })),
        )
        .child(Node::el("div", "mfd-fms-fpln-header-speed-alt").child(spd_alt_btn))
        .child(Node::el("div", "mfd-fms-fpln-header-trk").child(Node::el("span", "mfd-label").text("TRK")))
        .child(Node::el("div", "mfd-fms-fpln-header-dist").child(Node::el("span", "mfd-label").text("DIST")))
        .child(Node::el("div", "mfd-fms-fpln-header-fpa").child(Node::el("span", "mfd-label").text("FPA")));

    let mut lines = Node::new("div");
    for (i, l) in m.lines.iter().enumerate() {
        lines = lines.child(render_line(m, l, if i > 0 { m.lines.get(i - 1) } else { None }));
    }

    let dest_label_cls = |base: &str, green: bool, amber: bool| {
        Node::el("span", base).class_if("mfd-fms-yellow-text", temporary).class_if("mfd-fms-green-text", green).class_if("amber", amber)
    };
    let weight_unit = if m.weight_kg { "T" } else { "KLB" };
    let dest_btn = if dest_not_loaded {
        None
    } else {
        Some(button(ButtonSpec::new(&dest.button_label).style("font-size: 30px; width: 150px; margin-right: 5px;")))
    };
    let dest_btn_disabled = button(ButtonSpec::new(&dest.button_label).style("font-size: 30px; width: 150px; margin-right: 5px;"));
    let dest_first = conditional(
        dest_not_loaded,
        Some(
            Node::el("span", "mfd-label")
                .class_if("mfd-fms-green-text", m.active_plan)
                .style("width:150px; margin-right:5 px")
                .text("--------"),
        ),
        Some(dest_btn.unwrap_or(dest_btn_disabled)),
        None,
    );
    let destination = Node::el("div", "mfd-fms-fpln-line-destination")
        .child(dest_first)
        .child(
            Node::el("span", "mfd-label")
                .class_if("mfd-fms-yellow-text", temporary)
                .class_if("mfd-fms-green-text", time_not_avail_active)
                .text(&dest.time_ms.map(utc_hhmm).unwrap_or_else(|| "--:--".into())),
        )
        .child(
            Node::el("div", "mfd-label-value-container")
                .child(dest_label_cls("mfd-label", efob_na_active, dest.efob_amber).text(&weight_text(dest.efob_kg, m.weight_kg)))
                .child(
                    Node::el("span", "mfd-label-unit mfd-unit-trailing")
                        .style(&format!("visibility: {}", if efob_na { "hidden" } else { "visible" }))
                        .text(weight_unit),
                ),
        )
        .child(
            Node::el("div", "mfd-label-value-container")
                .child(dest_label_cls("mfd-label", dist_na_active, false).text(&dest.distance.map(|d| format!("{:.0}", d)).unwrap_or_else(|| "----".into())))
                .child(
                    Node::el("span", "mfd-label-unit mfd-unit-trailing")
                        .style(&format!("visibility: {}", if dest.distance.is_none() { "hidden" } else { "visible" }))
                        .text("NM"),
                ),
        )
        .child(
            Node::new("div")
                .style("display: flex; flex-direction: row; margin-top: 5px; margin-bottom: 5px;")
                .child(icon_button(Icon::DoubleDown, m.scroll_down_disabled, "width: 60px; height: 60px;"))
                .child(icon_button(Icon::DoubleUp, m.scroll_up_disabled, "width: 60px; height: 60px;"))
                .child(button(
                    ButtonSpec::new("DEST")
                        .disabled(dest_not_loaded)
                        .style("height: 60px; margin-right: 5px; padding: auto 15px auto 15px;"),
                )),
        );

    let return_btn = button(ButtonSpec::new("RETURN").style("width: 125px;"));
    let init_btn = button(ButtonSpec::new("INIT").style("width: 125px;"));
    let left = conditional(
        m.sec_active,
        Some(return_btn),
        Some(conditional(m.show_init, Some(init_btn), None, Some(125))),
        Some(125),
    );
    let info_items = vec![
        MenuItem::new("ALTERNATE", true),
        MenuItem::new("CLOSEST AIRPORTS", true),
        MenuItem::new("EQUI-TIME POINT", true),
        MenuItem::new("FIX INFO", m.sec_active),
        MenuItem::new("LL CROSSING", true),
        MenuItem::new("TIME", true),
        MenuItem::new("CPNY F-PLN REPORT", true),
    ];
    let info = button(
        ButtonSpec::new("F-PLN INFO")
            .menu(&format!("{}_MFD_f-pln-infoBtn", side), info_items, true)
            .opened(m.info_open),
    );
    let right = conditional(
        m.sec_active,
        None,
        Some(button(ButtonSpec::new("DIR TO").style("margin-right: 5px;"))),
        Some(125),
    );
    let footer_row = Node::el("div", "mfd-fms-fpln-footer").child(left).child(info).child(right);

    let mut revisions = context_menu(&format!("{}_MFD_revisionsMenu", side), &m.revisions_menu, m.revisions_menu_open);
    if m.revisions_menu_positioned || m.revisions_menu_open {
        revisions = revisions.style(&format!("display: {}; left: 195px; top: 183px;", if m.revisions_menu_open { "block" } else { "none" }));
    }
    let tmpy_line = m.tmpy_line.unwrap_or(m.tmpy_active);
    let page = Node::el("div", "mfd-page-container")
        .child(revisions)
        .child(dialog_new_dest(&m.new_dest))
        .child(dialog_insert_next(side, &m.insert_next))
        .child(header)
        .child(lines)
        .child(Node::new("div").style("flex-grow: 1"))
        .child(
            Node::el("div", "mfd-fms-fpln-line-erase-temporary")
                .style(&format!("display: {};", if tmpy_line { "flex" } else { "none" }))
                .child(tmpy_button("ERASE"))
                .child(tmpy_button("INSERT")),
        )
        .child(Node::new("div").style("flex-grow: 1;"))
        .child(destination)
        .child(footer_row);

    let mut tb = shell.title(&m.title, "");
    tb.tmpy = m.tmpy_active || tb.tmpy;
    vec![title_bar(&tb), page, footer(&shell.footer())]
}

pub fn fpln_screen(shell: &ShellModel, m: &FplnModel) -> Node {
    fms_display_unit(shell, fpln_page(shell, m))
}

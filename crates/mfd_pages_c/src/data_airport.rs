use crate::fmt::*;
use crate::w::*;
use mfd_tree::Node;

#[derive(Clone, Debug)]
pub struct Runway {
    pub designation: String,
    pub length_m: f64,
    pub ls_ident: Option<String>,
    pub elevation: String,
    pub course: String,
    pub coords: String,
}

#[derive(Clone, Debug, Default)]
pub struct DataAirportModel {
    pub selected_tab: usize,
    pub use_metric: bool,
    pub icao: Option<String>,
    pub airport_name: String,
    pub airport_coords: String,
    pub runways: Vec<Runway>,
    pub selected_runway: Option<usize>,
    pub custom_rwy_button_disabled: bool,
}

fn pad_end(s: &str, n: usize) -> String {
    let len = s.chars().count();
    if len >= n {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(n - len))
    }
}

pub fn format_runway_label(r: &Runway, use_metric: bool, elaborate_ls: bool) -> String {
    let len = if use_metric { r.length_m } else { r.length_m / 0.3048 };
    format!(
        "{} {}{} {}",
        pad_end(&r.designation, 3),
        pad_start(&to_fixed(len, 0), 5, '\u{a0}'),
        if use_metric { "M" } else { "FT" },
        match &r.ls_ident {
            Some(_) => {
                if elaborate_ls {
                    "ILS"
                } else {
                    "LS"
                }
            }
            None => "",
        }
    )
}

pub fn data_airport(m: &DataAirportModel) -> Node {
    let icao_text = match &m.icao {
        Some(s) if !s.is_empty() => s.clone(),
        _ => mfd_placeholder("----"),
    };
    let (info_visible, list_visible, data_visible) = match &m.icao {
        None => (true, false, false),
        Some(s) if s.chars().count() == 4 => (true, m.selected_runway.is_none() && !m.runways.is_empty(), m.selected_runway.is_some()),
        Some(_) => (false, false, false),
    };
    let airport_loaded = m.icao.as_ref().map(|s| s.chars().count() == 4).unwrap_or(false) && !m.runways.is_empty();
    let sel = m.selected_runway.and_then(|i| m.runways.get(i));

    let mut list = Node::el("div", "mfd-data-airport-list mfd-data-airport-border-container");
    if airport_loaded {
        for r in &m.runways {
            list = list.child(button(
                &format_runway_label(r, m.use_metric, false),
                &Btn::new().style("width: 220px; margin-bottom: 5px; margin-top: 5px; height: 40px; margin-left: 10px; white-space: nowrap"),
            ));
        }
    }

    let length_text = match sel {
        Some(r) => {
            let v = if m.use_metric { r.length_m } else { r.length_m / 0.3048 };
            format!(" {}", to_fixed(v, 1).trim_end_matches('0').trim_end_matches('.'))
        }
        None => " -----".to_string(),
    };
    let rwy_field = |inner: Vec<Node>, extra: Vec<Node>| {
        Node::el("div", "mfd-label mfd-data-airport-runway-field")
            .style("min-width: 120px;")
            .child(Node::new("div").children(inner))
            .children(extra)
    };
    let (cur, total) = match m.selected_runway {
        Some(i) => ((i + 1).to_string(), m.runways.len().to_string()),
        None => (String::new(), String::new()),
    };
    let disabled_left = m.selected_runway.map(|i| i == 0).unwrap_or(true);
    let disabled_right = match m.selected_runway {
        Some(i) => i + 1 == m.runways.len(),
        None => false,
    };
    let runway_data = Node::el("div", "mfd-data-airport-border-container")
        .child(
            Node::el("div", "mfd-data-airport-runway")
                .child(rwy_field(vec![value_big(sel.map(|r| pad_end(&r.designation, 3)).as_deref().unwrap_or(""))], vec![]))
                .child(
                    Node::new("div")
                        .style("display: flex; flex-direction: column; justify-content: center; align-items: center; margin-top: -25px; margin-bottom: 20px;")
                        .child(
                            Node::new("div")
                                .child(label(&cur))
                                .child(label("/"))
                                .child(label(&total)),
                        )
                        .child(
                            Node::el("div", "mfd-data-airport-runway-scroll-buttons")
                                .child(icon_button("double-left", disabled_left, "width: 50px; height: 50px; margin-right: 5px;"))
                                .child(icon_button("double-right", disabled_right, "width: 50px; height: 50px;")),
                        ),
                )
                .child(rwy_field(vec![value_big(sel.map(|r| r.coords.as_str()).unwrap_or(""))], vec![]))
                .child(rwy_field(
                    vec![value_big(sel.map(|r| r.elevation.as_str()).unwrap_or("")), Node::el("span", "mfd-label-unit mfd-unit-trailing").text("FT")],
                    vec![],
                ))
                .child(rwy_field(
                    vec![
                        Node::el("span", "mfd-value bigger").text(&length_text),
                        Node::el("span", "mfd-label-unit mfd-unit-trailing").text(if m.use_metric { "M" } else { "FT" }),
                    ],
                    vec![Node::el("div", "mfd-label mfd-data-airport-runway-field-course")
                        .child(value_big(sel.map(|r| r.course.as_str()).unwrap_or("")))
                        .child(Node::el("span", "mfd-label-unit mfd-unit-trailing").text("\u{b0}"))],
                ))
                .child(rwy_field(
                    vec![value_big(match sel {
                        Some(r) => r.ls_ident.as_deref().unwrap_or("\u{a0}"),
                        None => "\u{a0}",
                    })],
                    vec![],
                )),
        )
        .child(button("RWY LIST", &Btn::new().style("width: 220px; margin-bottom: 10px; margin-left: 10px; height: 40px;")));

    let db = vec![Node::el("div", "mfd-data-airport-container")
        .child(
            Node::el("div", "mfd-data-airport-input")
                .child(Node::el("div", "mfd-label").style("position: relative; right: 90px;").text("ARPT IDENT"))
                .child(Node::new("div").child(input_field(
                    &Field::new(&icao_text).cls(if m.icao.is_none() { &["mandatory"] } else { &[] }).style("position: relative; right: 60px;").align("center"),
                ))),
        )
        .child(Node::el("div", "mfd-data-airport-info-container").child(conditional(
            info_visible,
            Node::el("div", "mfd-data-airport-info")
                .child(Node::el("div", "mfd-value bigger").text(&m.airport_name))
                .child(Node::el("div", "mfd-value bigger mfd-data-airport-coords").text(&m.airport_coords)),
            empty(),
            None,
        )))
        .child(conditional(list_visible, list, empty(), Some("")))
        .child(conditional(data_visible, runway_data, empty(), Some("")))];

    let stored = vec![
        Node::el("div", "mfd-label").style("align-self: center; position: relative; top: 45px;").text("NO PILOT STORED NAVAID"),
        button("NEW RWY", &Btn::new().disabled(m.custom_rwy_button_disabled).style("width: 170px; height: 60px; position: absolute; right: 0; top: 630px;")),
    ];
    let t = Tabs::new(&["DATABASE ARPTs", "PILOT STORED RWYs"], m.selected_tab).angle(25.0);
    let mut kids = vec![
        top_tab_navigator(&t, vec![tab_page(m.selected_tab == 0, Some("height: 680px;"), db), tab_page(m.selected_tab == 1, Some("height: 680px;"), stored)]),
        spacer(),
    ];
    kids.push(Node::new("div").style("width: 150px;").child(button(
        "RETURN",
        &Btn::new().style("margin-right: 5px;").container(if m.selected_tab == 0 { "" } else { "visibility: hidden;" }),
    )));
    page_container(kids)
}

fn mfd_placeholder(s: &str) -> String {
    mandatory(s)
}

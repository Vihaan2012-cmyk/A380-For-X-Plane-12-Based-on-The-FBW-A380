use crate::fmt::*;
use crate::w::*;
use mfd_tree::Node;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NavaidType {
    Ls,
    Vor,
    Ndb,
}

#[derive(Clone, Debug, Default)]
pub struct NavaidData {
    pub class: String,
    pub lat_long: String,
    pub elevation: String,
    pub station_declination: String,
    pub declination_cardinal_dir: String,
    pub runway_ident: String,
    pub freq: String,
    pub category: String,
    pub course: String,
    pub fig_of_merit: String,
}

#[derive(Clone, Debug, Default)]
pub struct DataNavaidModel {
    pub selected_tab: usize,
    pub navaid_ident: Option<String>,
    pub navaid_type: Option<NavaidType>,
    pub data: NavaidData,
    pub new_navaid_ident: Option<String>,
    pub new_navaid_class: usize,
    pub new_navaid_cat: usize,
    pub new_navaid_fom: usize,
}

struct Fld {
    label: &'static str,
    value: String,
    class: Option<&'static str>,
    unit: Option<String>,
}

fn navaid_fields(t: NavaidType, d: &NavaidData) -> Node {
    let mut f = vec![
        Fld { label: "CLASS", value: d.class.clone(), class: None, unit: None },
        Fld { label: "LAT/LONG", value: d.lat_long.clone(), class: None, unit: None },
    ];
    match t {
        NavaidType::Ls => {
            f.push(Fld { label: "ELEVATION", value: d.elevation.clone(), class: Some("indent-1"), unit: None });
            f.push(Fld { label: "RWY IDENT", value: d.runway_ident.clone(), class: None, unit: None });
            f.push(Fld { label: "FREQ", value: d.freq.clone(), class: None, unit: None });
            f.push(Fld { label: "CAT", value: d.category.clone(), class: None, unit: None });
            f.push(Fld { label: "CRS", value: d.course.clone(), class: Some("indent-0"), unit: Some("\u{b0}".to_string()) });
            f.push(Fld { label: "FIG OF MERIT", value: d.fig_of_merit.clone(), class: None, unit: None });
        }
        NavaidType::Vor => {
            f.push(Fld {
                label: "STATION DEC",
                value: d.station_declination.clone(),
                class: None,
                unit: Some(format!("\u{b0}{}", d.declination_cardinal_dir)),
            });
            f.push(Fld { label: "FREQ", value: d.freq.clone(), class: None, unit: None });
            f.push(Fld { label: "FIG OF MERIT", value: d.fig_of_merit.clone(), class: None, unit: None });
        }
        NavaidType::Ndb => {
            f.push(Fld { label: "FREQ", value: d.freq.clone(), class: None, unit: None });
        }
    }
    Node::el("div", "mfd-data-navaid-information-row")
        .child(Node::el("div", "mfd-data-navaid-information-row labels").children(f.iter().map(|x| Node::el("div", "mfd-label").text(x.label))))
        .child(Node::el("div", "mfd-data-navaid-information-row values").children(f.iter().map(|x| {
            let mut n = Node::el("div", "mfd-value bigger");
            if let Some(c) = x.class {
                n.cls.push(c.to_string());
            }
            n = n.text(&x.value);
            if let Some(u) = &x.unit {
                n = n.child(Node::el("span", "mfd-label-unit mfd-unit-trailing").text(u));
            }
            n
        })))
}

fn option(cls: &str, lbl: &str, inner: Vec<Node>) -> Node {
    Node::el("div", &format!("option {}", cls)).child(Node::el("div", "mfd-label").text(lbl)).children(inner)
}

pub fn data_navaid(m: &DataNavaidModel) -> Node {
    let ident_text = match &m.navaid_ident {
        Some(s) if !s.is_empty() => s.clone(),
        _ => mandatory("----"),
    };
    let ident_cls: &[&'static str] = if m.navaid_ident.as_deref().map(|s| s.is_empty()).unwrap_or(true) { &["mandatory"] } else { &[] };
    let new_ident_text = match &m.new_navaid_ident {
        Some(s) if !s.is_empty() => s.clone(),
        _ => mandatory("----"),
    };
    let new_ident_cls: &[&'static str] = if m.new_navaid_ident.as_deref().map(|s| s.is_empty()).unwrap_or(true) { &["mandatory"] } else { &[] };

    let db = vec![
        Node::el("div", "mfd-data-navaid-input-container")
            .child(Node::el("div", "mfd-label").style("position:relative; right: 90px").text("NAVAID IDENT"))
            .child(input_field(&Field::new(&ident_text).cls(ident_cls).style("position: relative; right: 60px;").align("center"))),
        Node::el("div", "mfd-data-navaid-information-container")
            .child(conditional(m.navaid_type == Some(NavaidType::Ls), navaid_fields(NavaidType::Ls, &m.data), empty(), None))
            .child(conditional(m.navaid_type == Some(NavaidType::Vor), navaid_fields(NavaidType::Vor, &m.data), empty(), None))
            .child(conditional(m.navaid_type == Some(NavaidType::Ndb), navaid_fields(NavaidType::Ndb, &m.data), empty(), None)),
    ];

    let dd = |text: &str, items: &[&str]| {
        let mut d = Dropdown::new(text);
        d.items = items.iter().map(|s| s.to_string()).collect();
        dropdown(&d)
    };
    let classes = ["VOR", "DME", "LOC", "ILS", "GLS", "NDB", "VOR/DME"];
    let cats = ["1", "2", "3"];
    let foms = ["0 (40NM)", "1 (70NM)", "2 (130NM)", "3 (250NM)"];
    let latfld = || input_field(&Field::new("-------").align("center"));

    let stored = vec![
        Node::el("div", "mfd-label").style("align-self: center; position: relative; top: 45px;").text("NO PILOT STORED NAVAID"),
        button("NEW NAVAID", &Btn::new().disabled(true).style("width: 170px; height: 60px; position: absolute; right: 0; top: 630px;")),
        Node::el("div", "new-navaid-container")
            .style("display: none;")
            .child(
                Node::el("div", "mfd-data-navaid-input-container")
                    .child(Node::el("div", "mfd-label").style("position: relative; right: 90px;").text("NAVAID IDENT"))
                    .child(Node::new("div").child(input_field(&Field::new(&new_ident_text).cls(new_ident_cls).style("position: relative; right: 60px;").align("center")))),
            )
            .child(
                Node::el("div", "mfd-data-navaid-options")
                    .child(option("class", "CLASS", vec![dd(classes.get(m.new_navaid_class).copied().unwrap_or(""), &classes)]))
                    .child(option("lat-long", "LAT/LONG", vec![latfld(), Node::new("span").text("/"), latfld()]))
                    .child(option("elevation", "ELEVATION", vec![input_field(&Field::new("-----").units("", "FT").align("center"))]))
                    .child(option("station-dec", "STATION DEC", vec![input_field(&Field::new("----").units("", "M").align("center"))]))
                    .child(option("cat", "CAT", vec![dd(cats.get(m.new_navaid_cat).copied().unwrap_or(""), &cats)]))
                    .child(option("fom", "FIG OF MERIT", vec![dd(foms.get(m.new_navaid_fom).copied().unwrap_or(""), &foms)])),
            ),
    ];
    let t = Tabs::new(&["DATABASE NAVAIDS", "PILOT STORED NAVAIDS"], m.selected_tab).angle(25.0);
    page_container(vec![
        top_tab_navigator(&t, vec![tab_page(m.selected_tab == 0, Some("height: 680px;"), db), tab_page(m.selected_tab == 1, Some("height: 680px;"), stored)]),
        spacer(),
    ])
}

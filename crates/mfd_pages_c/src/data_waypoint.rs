use crate::fmt::*;
use crate::w::*;
use mfd_tree::Node;

#[derive(Clone, Debug, Default)]
pub struct DataWaypointModel {
    pub selected_tab: usize,
    pub waypoint_ident: Option<String>,
    pub waypoint_coords: String,
    pub pilot_stored: Vec<String>,
    pub pilot_stored_index: Option<usize>,
}

pub fn data_waypoint(m: &DataWaypointModel) -> Node {
    let has_ident = m.waypoint_ident.as_deref().map(|s| !s.is_empty()).unwrap_or(false);
    let ident_text = if has_ident { m.waypoint_ident.clone().unwrap_or_default() } else { mandatory("-------") };
    let ident_cls: &[&'static str] = if has_ident { &[] } else { &["mandatory"] };

    let info = Node::el("div", "mfd-data-waypoint-info")
        .child(
            Node::el("div", "mfd-data-latlong-text")
                .child(Node::el("div", "mfd-label").style("position: relative; right: 75px;").text("LAT"))
                .child(Node::el("div", "mfd-label").style("position: relative; left: 75px;").text("LONG")),
        )
        .child(Node::el("div", "mfd-value bigger mfd-data-airport-coords").text(&m.waypoint_coords));

    let db = vec![
        Node::el("div", "mfd-data-waypoint-input-container")
            .child(Node::el("div", "mfd-label").style("position: relative; right: 65px").text("WPT IDENT"))
            .child(input_field(&Field::new(&ident_text).cls(ident_cls).style("position: relative; right: 60px;").align("center"))),
        Node::el("div", "mfd-data-waypoint-info-container").child(conditional(has_ident, info, empty(), None)),
    ];

    let any = !m.pilot_stored.is_empty();
    let sel = m.pilot_stored_index.and_then(|i| m.pilot_stored.get(i)).cloned().unwrap_or_default();
    let mut dd = Dropdown::new(&sel);
    dd.items = m.pilot_stored.clone();
    dd.id_prefix = "dataWaypointListDropdown".to_string();
    let list = Node::el("div", "mfd-data-waypoint-list-container")
        .child(Node::el("div", "mfd-label").style("align-self:center; position: relative; top: 45px;").text("WPT IDENT"))
        .child(dropdown(&dd))
        .child(
            Node::el("div", "mfd-data-waypoint-stored-list-scroll-buttons")
                .child(icon_button("double-left", true, "width: 50px; height: 50px; margin-right: 5px"))
                .child(icon_button("double-right", false, "width: 50px; height: 50px;")),
        )
        .child(Node::el("div", "mfd-label").text(&pad_start(&m.pilot_stored.len().to_string(), 2, '0')))
        .child(
            Node::el("div", "mfd-data-waypoint-stored-ll-display")
                .child(Node::el("div", "mfd-lable").text("LAT"))
                .child(Node::el("div", "mfd-lable").text("LAT")),
        );
    let none = Node::el("div", "mfd-data-waypoint-stored-container").child(
        Node::el("div", "mfd-data-waypoint-stored-ident-row")
            .child(Node::el("div", "mfd-label").style("position: relative;").text("NO PILOT STORED WPT")),
    );
    let t = Tabs::new(&["DATABASE WPTs", "PILOT STORED WPTs"], m.selected_tab).angle(25.0);
    page_container(vec![
        top_tab_navigator(
            &t,
            vec![
                tab_page(m.selected_tab == 0, Some("height: 680px;"), db),
                tab_page(m.selected_tab == 1, Some("height: 680px;"), vec![conditional(any, list, none, None)]),
            ],
        ),
        spacer(),
    ])
}

use crate::w::*;
use crate::x::*;
use mfd_tree::Node;

pub const NUM_ROUTE_LINES_DISPLAYED: usize = 11;

#[derive(Clone, Debug, Default)]
pub struct SecTab {
    pub exists: bool,
    pub city_pair: String,
    pub created: String,
    pub route: Vec<(String, String)>,
    pub route_start_line: usize,
    pub swap_visible: bool,
    pub cpny_label: String,
    pub cpny_disabled: bool,
    pub cpny_menu: Vec<String>,
    pub import_items: [String; 3],
    pub import_open: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SecIndexModel {
    pub selected_tab: usize,
    pub tabs: [SecTab; 3],
    pub cap_or_fo: String,
    pub footer_message: String,
}

pub fn created_label(exists: bool, time_hhmm: Option<&str>, source: &str) -> String {
    match (exists, time_hhmm) {
        (true, Some(t)) => format!("CREATED\u{a0}\u{a0}{}{}", t, source),
        _ => String::new(),
    }
}

pub fn import_items(sec_index: usize) -> [String; 3] {
    [
        "ACTIVE*".into(),
        if sec_index == 1 { "SEC 2*".into() } else { "SEC 1*".into() },
        if sec_index == 3 { "SEC 2*".into() } else { "SEC 3*".into() },
    ]
}

fn sec_tab(m: &SecIndexModel, i: usize) -> Vec<Node> {
    let t = &m.tabs[i];
    let sec_index = i + 1;
    let fp = 3 + i;
    let pfx = |s: &str| format!("{}_MFD_sec{}index_{}", m.cap_or_fo, fp, s);
    let does_not_exist = !t.exists;
    let mut route = Node::el("div", "mfd-sec-index-table-left route");
    for idx in 0..NUM_ROUTE_LINES_DISPLAYED * 2 {
        let at = t.route_start_line * 2 + idx;
        let (a, b, vis) = match t.route.get(at) {
            Some((a, b)) => (a.as_str(), b.as_str(), true),
            None => ("", "", false),
        };
        let st = if vis { "visibility: inherit;" } else { "visibility: hidden;" };
        route = route
            .child(Node::el("div", "mfd-sec-index-table-lower-button-grid-cell leg-ident").style(st).text(a))
            .child(Node::el("div", "mfd-sec-index-table-lower-button-grid-cell fix-ident").style(st).text(b));
    }
    let items: Vec<&str> = t.import_items.iter().map(|s| s.as_str()).collect();
    let cpny_items: Vec<&str> = t.cpny_menu.iter().map(|s| s.as_str()).collect();
    let nav_btn = |label: &str, key: &str, disabled: bool| {
        btn_menu_plain(label, disabled, "width: 160px; margin-top: 5px;", &pfx(key))
    };
    let up_disabled = t.route_start_line == 0;
    let down_disabled = (t.route.len() + 1) / 2 <= t.route_start_line + NUM_ROUTE_LINES_DISPLAYED;
    let upper = Node::el("div", "mfd-sec-index-table upper").child(route).child(
        Node::el("div", "mfd-sec-index-table-right")
            .child(set_opened(btn_menu("IMPORT", false, "width: 175px;", &pfx("import"), &items, true), t.import_open))
            .child(btn_menu(&t.cpny_label, t.cpny_disabled, "width: 175px; margin-top: 5px;", &format!("{}_MFD_fplnreq_sec{}", m.cap_or_fo, fp), &cpny_items, false))
            .child(spacer())
            .child(nav_btn("F-PLN", "f-pln", false))
            .child(nav_btn("PERF", "perf", does_not_exist))
            .child(nav_btn("WIND", "wind", does_not_exist))
            .child(nav_btn("FUEL&LOAD", "fuel-load", does_not_exist))
            .child(nav_btn("INIT", "init", false))
            .child(nav_btn("WHAT IF", "what-if", true)),
    );
    let lower = Node::el("div", "mfd-sec-index-table lower")
        .child(
            Node::el("div", "mfd-sec-index-table-left").child(
                Node::el("div", "mfd-sec-index-table-lower-button-grid")
                    .child(btn_node(
                        star_label(&["DELETE"], "text-align: center; vertical-align: center; margin-right: 10px;"),
                        does_not_exist,
                        false,
                        "padding-right: 2px;width: 160px; height: 60px;",
                    ))
                    .child(
                        Node::new("div")
                            .style("display: flex; flex-direction: row; justify-content: center; align-items: center;")
                            .child(icon("double-down", down_disabled, "width: 60px; height: 60px; margin-right: 5px;"))
                            .child(icon("double-up", up_disabled, "width: 60px; height: 60px;")),
                    )
                    .child(
                        Node::new("div")
                            .style(&format!(
                                "display: flex; justify-content: flex-end; align-items: center; visibility: {};",
                                if t.swap_visible { "inherit" } else { "hidden" }
                            ))
                            .child(btn_node(
                                star_label(&["SWAP", "ACTIVE"], "text-align: center; vertical-align: center; margin-right: 10px; color: #e68000;"),
                                false,
                                false,
                                "padding-right: 2px; width: 160px; height: 60px;",
                            )),
                    )
                    .child(Node::new("div"))
                    .child(Node::new("div"))
                    .child(
                        Node::new("div")
                            .style("display: flex; justify-content: flex-end; align-items: center;")
                            .child(btn_menu_plain("XFR TO MAILBOX", true, "width: 160px; height: 60px; margin-top: 30px;", &pfx("xfr-to-mailbox"))),
                    ),
            ),
        )
        .child(
            Node::el("div", "mfd-sec-index-table-right").child(spacer()).child(
                Node::new("div")
                    .style("justify-content: flex-end; align-self: center; margin-bottom: 15px; margin-top: 75px;")
                    .child(btn_node(
                        star_label(&["PRINT"], "text-align: center; vertical-align: center; margin-right: 10px;"),
                        true,
                        false,
                        "padding-right: 2px; width: 160px; height: 60px; margin-top: 30px;",
                    )),
            ),
    );
    let _ = sec_index;
    vec![
        Node::el("div", "mfd-sec-index-title-container")
            .child(Node::el("span", "mfd-sec-index-title-line1").text(&t.city_pair))
            .child(Node::el("span", "mfd-sec-index-title-line2").text(&t.created)),
        upper,
        lower,
    ]
}

fn btn_menu_plain(label: &str, disabled: bool, style: &str, _prefix: &str) -> Node {
    btn(label, disabled, style)
}

pub fn sec_index(m: &SecIndexModel) -> Node {
    let t = Tabs::new(&["SEC 1", "SEC 2", "SEC 3"], m.selected_tab).angle(25.0);
    page_container(vec![top_tab_navigator(
        &t,
        (0..3).map(|i| tab_page(m.selected_tab == i, None, sec_tab(m, i))).collect(),
    )])
}

pub fn sec_index_footer(m: &SecIndexModel) -> Node {
    footer(&m.footer_message)
}

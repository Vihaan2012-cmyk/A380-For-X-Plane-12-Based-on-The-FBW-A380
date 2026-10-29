use crate::widgets::*;
use mfd_tree::Node;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct ShellModel {
    pub side: String,
    pub fms_source: String,
    pub callsign: Option<String>,
    pub system: String,
    pub category: String,
    pub sys_dropdown_open: bool,
    pub sys_dropdown_typed: String,
    pub open_page_selector: Option<usize>,
    pub eo: bool,
    pub tmpy: bool,
    pub penalty: bool,
    pub message: Option<String>,
    pub message_background: String,
    pub message_list_open: bool,
    pub message_list: Vec<String>,
    pub duplicates_open: bool,
    pub page_not_avail: bool,
}

impl Default for ShellModel {
    fn default() -> Self {
        ShellModel {
            side: "CAPT".into(),
            fms_source: "FMS 1".into(),
            callsign: None,
            system: "fms".into(),
            category: "active".into(),
            sys_dropdown_open: false,
            sys_dropdown_typed: String::new(),
            open_page_selector: None,
            eo: false,
            tmpy: false,
            penalty: false,
            message: None,
            message_background: String::new(),
            message_list_open: false,
            message_list: Vec::new(),
            duplicates_open: false,
            page_not_avail: false,
        }
    }
}

impl ShellModel {
    pub fn footer(&self) -> FooterModel {
        FooterModel { message: self.message.clone(), message_background: self.message_background.clone() }
    }

    pub fn title(&self, title: &str, offset: &str) -> TitleBarModel {
        TitleBarModel {
            title: title.to_string(),
            offset: offset.to_string(),
            eo: self.eo,
            tmpy: self.tmpy,
            penalty: self.penalty,
            is_fms_page: true,
        }
    }

    pub fn id_prefix(&self) -> String {
        format!("{}_MFD", self.side)
    }
}

pub fn sys_index(system: &str) -> usize {
    match system {
        "atccom" => 1,
        "surv" => 2,
        "fcubkup" => 3,
        _ => 0,
    }
}

pub fn sys_select_row(m: &ShellModel) -> Node {
    let options = vec![m.fms_source.clone(), "ATCCOM".into(), "SURV".into(), "FCU BKUP".into()];
    let idx = sys_index(&m.system);
    let shown = if m.sys_dropdown_typed.is_empty() { options[idx].clone() } else { m.sys_dropdown_typed.clone() };
    let mut input = InputSpec::new(&shown).align("flex-start");
    if m.sys_dropdown_open {
        input.state = InputState::focused();
    }
    let filtered: Vec<String> = options.iter().filter(|o| o.starts_with(&m.sys_dropdown_typed)).cloned().collect();
    let dd = dropdown_menu(DropdownSpec {
        id_prefix: format!("{}_sysSelectorDropdown", m.id_prefix()),
        input,
        options: filtered,
        container_style: "width: 25%;".into(),
        align: "flex-start".into(),
        open: m.sys_dropdown_open,
        ..Default::default()
    });
    Node::el("div", "mfd-header-sys-select-row")
        .child(dd)
        .child(Node::el("span", "mfd-header-callsign").text(m.callsign.as_deref().unwrap_or("----------")))
}

fn items(list: &[(&str, bool)]) -> Vec<MenuItem> {
    list.iter().map(|(l, d)| MenuItem::new(l, *d)).collect()
}

pub fn fms_header(m: &ShellModel) -> Node {
    let cat = m.category.as_str();
    let p = m.id_prefix();
    let active = cat == "active";
    let position = cat == "position";
    let sec = matches!(cat, "sec" | "sec1" | "sec2" | "sec3");
    let data = cat == "data";
    let open = |i: usize| m.open_page_selector == Some(i);
    let row = Node::el("div", "mfd-header-page-select-row")
        .child(page_selector(
            "ACTIVE",
            active,
            &format!("{}_pageSelectorActive", p),
            &items(&[("F-PLN", false), ("PERF", false), ("FUEL&LOAD", false), ("WIND", false), ("INIT", false)]),
            open(0),
        ))
        .child(page_selector(
            "POSITION",
            position,
            &format!("{}_pageSelectorPosition", p),
            &items(&[("MONITOR", false), ("REPORT", true), ("NAVAIDS", false), ("IRS", false), ("GNSS", true), ("TIME", true)]),
            open(1),
        ))
        .child(page_selector(
            "SEC INDEX",
            sec,
            &format!("{}_pageSelectorSecIndex", p),
            &items(&[("SEC 1", false), ("SEC 2", false), ("SEC 3", false)]),
            open(2),
        ))
        .child(page_selector(
            "DATA",
            data,
            &format!("{}_pageSelectorData", p),
            &items(&[("STATUS", false), ("WAYPOINT", false), ("NAVAID", false), ("ROUTE", true), ("AIRPORT", false), ("PRINTER", true)]),
            open(3),
        ));
    Node::new("div").child(sys_select_row(m)).child(row)
}

fn page_not_avail(m: &ShellModel) -> Node {
    Node::el("div", "mfd-fms-fpln-dialog-outer")
        .style(&format!("display: {};", if m.page_not_avail { "block" } else { "none" }))
        .child(
            Node::el("div", "mfd-fms-fpln-dialog-inner")
                .style("top: 34px; height: 818px;")
                .child(Node::el("div", "mfd-page-container").child(Node::el("div", "mfd-amber-error-message").text("FMS PAGE NOT AVAIL"))),
        )
}

fn message_list(m: &ShellModel) -> Node {
    let mut container = Node::el("div", "mfd-msg-list-element-container");
    for msg in m.message_list.iter().take(4) {
        container = container.child(Node::el("div", "mfd-label msg-list-element").text(msg));
    }
    let page = Node::el("div", "mfd-page-container")
        .child(container)
        .child(Node::new("div").style("flex-grow: 1;"))
        .child(Node::new("div").style("display: flex; justify-content: flex-start;").child(button(ButtonSpec::new("CLOSE"))));
    Node::el("div", "mfd-fms-fpln-dialog-outer")
        .style(&format!("display: {};", if m.message_list_open { "block" } else { "none" }))
        .child(
            Node::el("div", "mfd-fms-fpln-dialog-inner")
                .child(title_bar(&TitleBarModel { title: "MESSAGE LIST".into(), eo: m.eo, is_fms_page: true, ..Default::default() }))
                .child(page),
        )
}

fn duplicate_names(m: &ShellModel) -> Node {
    let head = [("IDENT", "20%"), ("DIST(NM)", "20%"), ("LAT/LONG", "30%"), ("FREQ/CHAN", "30%")]
        .iter()
        .fold(Node::el("div", "mfd-fms-fpln-duplicate-table-header"), |n, (h, w)| {
            n.child(Node::new("div").style(&format!("width: {}", w)).child(Node::el("span", "mfd-label").text(h)))
        });
    Node::el("div", "mfd-fms-fpln-dialog-outer")
        .style(&format!("display: {};", if m.duplicates_open { "block" } else { "none" }))
        .child(
            Node::el("div", "mfd-fms-fpln-dialog-inner")
                .child(title_bar(&TitleBarModel { title: "DUPLICATE NAMES".into(), eo: m.eo, is_fms_page: true, ..Default::default() }))
                .child(Node::el("div", "mfd-fms-fpln-duplicate-table").style("margin-top: 100px;").child(head))
                .child(
                    Node::el("div", "mfd-fms-fpln-duplicate-outline mfd-fms-fpln-label-bottom-space")
                        .child(Node::el("div", "mfd-fms-fpln-duplicate-table"))
                        .child(Node::new("div").style("flex-grow: 1;")),
                )
                .child(
                    Node::new("div")
                        .style("display: flex; flex-direction: row; justify-content: center")
                        .child(icon_button(Icon::DoubleDown, true, "width: 60px; height: 60px; margin-right: 20px;"))
                        .child(icon_button(Icon::DoubleUp, true, "width: 60px; height: 60px;")),
                )
                .child(Node::new("div").style("flex-grow: 1;"))
                .child(
                    Node::new("div")
                        .style("display: flex; flex-direction: row; justify-content: space-between;")
                        .child(Node::new("div").style("display: flex; justify-content: flex-end; padding: 2px;").child(button(ButtonSpec::new("RETURN")))),
                ),
        )
}

fn overlays() -> Vec<Node> {
    let hidden = |cls: &str, view: &str, shapes| svg(cls, &[("style", "display:none"), ("viewbox", view)], shapes).style("display:none");
    let bg = || shape("rect", 0, &[("class", "SelfTestBackground"), ("x", "0"), ("y", "0"), ("width", "100%"), ("height", "100%")]);
    let eng = |x: &str, y: &str, end: bool| {
        let mut a = vec![("class", "EngineeringTestModeText"), ("x", x), ("y", y)];
        if end {
            a.push(("text-anchor", "end"));
        }
        shape("text", 0, &a)
    };
    vec![
        hidden(
            "SelfTest",
            "0 0 768 1024",
            vec![
                bg(),
                shape("text", 0, &[("class", "SelfTestText"), ("x", "50%"), ("y", "50%")]),
                shape("text", 0, &[("class", "SelfTestText"), ("x", "50%"), ("y", "54%")]),
            ],
        ),
        hidden(
            "SelfTest",
            "0 0 768 1024",
            vec![
                bg(),
                shape("rect", 0, &[("x", "84"), ("y", "880"), ("width", "600"), ("height", "70"), ("fill", "#ffffff")]),
                shape("rect", 0, &[("x", "89"), ("y", "885"), ("width", "35"), ("height", "60"), ("fill", "#4d4dff")]),
            ],
        ),
        hidden("MaintenanceMode", "0 0 600 600", vec![shape("text", 0, &[("class", "SelfTestText"), ("x", "50%"), ("y", "50%")])]),
        hidden(
            "EngineeringTestMode",
            "0 0 600 600",
            vec![
                eng("9", "250", false),
                eng("10", "270", false),
                eng("10", "344", false),
                eng("10", "366", false),
                eng("90%", "250", true),
                eng("98%", "366", true),
            ],
        ),
    ]
}

pub fn display_unit(m: &ShellModel, header: Node, page: Vec<Node>) -> Node {
    let main = Node::el("div", "mfd-main")
        .child(header)
        .child(page_not_avail(m))
        .child(message_list(m))
        .child(duplicate_names(m))
        .child(Node::el("div", "mfd-navigator-container").children(page))
        .child(mouse_cursor());
    let (id, style) = if m.side == "CAPT" {
        ("MFD_LEFT_PARENT_DIV", "width: 768px; position: relative; margin-right: 110px;")
    } else {
        ("MFD_RIGHT_PARENT_DIV", "width: 768px; position: relative;")
    };
    Node::new("div").id(id).style(style).children(overlays()).child(Node::new("div").style("display: block;").child(main))
}

pub fn fms_display_unit(m: &ShellModel, page: Vec<Node>) -> Node {
    display_unit(m, fms_header(m), page)
}

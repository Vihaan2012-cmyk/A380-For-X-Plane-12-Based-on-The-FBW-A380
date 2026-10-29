use crate::avncs::{render_section, AvncsPage};
use crate::flt_ops::{self, FltOpsPage};
use crate::navigator::{file, folder, nav_button, FolderOpts};
use crate::widgets::{button, icon_button, page_selector, Icon, PageItem};
use mfd_tree::Node;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct OitModel {
    pub capt: bool,
    pub self_test_visible: bool,
    pub oit_visible: bool,
    pub ansu_failed: bool,
    pub flt_ops: FltOps,
    pub avncs: Avncs,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct FltOps {
    pub hidden: bool,
    pub content_hidden: bool,
    pub login_hidden: bool,
    pub loading_hidden: bool,
    pub loading_fill_width: String,
    pub menu_opened: bool,
    pub functions_opened: bool,
    pub page: FltOpsPage,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Avncs {
    pub hidden: bool,
    pub content_hidden: bool,
    pub login_hidden: bool,
    pub loading_hidden: bool,
    pub loading_fill_width: String,
    pub flt_number: String,
    pub back_disabled: bool,
    pub forward_disabled: bool,
    pub connected_text: String,
    pub page: AvncsPage,
}

impl Default for Avncs {
    fn default() -> Self {
        Avncs {
            hidden: false,
            content_hidden: false,
            login_hidden: false,
            loading_hidden: false,
            loading_fill_width: String::new(),
            flt_number: String::new(),
            back_disabled: true,
            forward_disabled: true,
            connected_text: "Connected.".into(),
            page: AvncsPage::Menu,
        }
    }
}

pub fn prefix(capt: bool) -> &'static str {
    if capt { "CAPT" } else { "FO" }
}

pub fn build(m: &OitModel) -> Node {
    let who = prefix(m.capt);
    Node::new("div")
        .id("OIT_CONTENT")
        .child(Node::el("div", "SelfTest").style(if m.self_test_visible { "display: block;" } else { "display: none;" }))
        .child(
            Node::new("div")
                .style(if m.oit_visible { "display: block;" } else { "display: none;" })
                .child(flt_ops_container(&m.flt_ops, who, m.capt))
                .child(avncs_container(&m.avncs, who)),
        )
        .child(ansu_failed(m.ansu_failed))
}

fn ansu_failed(shown: bool) -> Node {
    let mut n = Node::el("svg", "AnsuFailed").style(if shown { "display: block;" } else { "display: none;" });
    n.svgattr.insert("viewbox".into(), "0 0 1333 1000".into());
    n.svgattr.insert("style".into(), if shown { "display: block;" } else { "display: none;" }.into());
    let mut a = std::collections::BTreeMap::new();
    a.insert("class".into(), "AnsuFailedText".into());
    a.insert("x".into(), "50%".into());
    a.insert("y".into(), "50%".into());
    a.insert("_d".into(), "0".into());
    a.insert("_display".into(), "block".into());
    n.shapes.push(mfd_tree::Shape { t: "text".into(), a });
    n
}

fn header_items() -> Vec<PageItem> {
    let it = |l: &str, s: bool| PageItem { label: l.into(), disabled: false, separator: s };
    vec![
        it("FLT OPS MENU", true),
        it("FLT FOLDER", false),
        it("TERML CHART", true),
        it("OPS LIBRARY", true),
        it("T.O PERF", false),
        it("LOADSHEET", false),
        it("LDG PERF", false),
        it("IN-FLT PERF", true),
        it("AOC", true),
        it("FLT OPS STS", false),
        it("LOAD BOX", false),
        it("EXPORT BOX", true),
        it("EXIT SESSION", false),
    ]
}

fn function_items() -> Vec<PageItem> {
    let it = |l: &str, d: bool, s: bool| PageItem { label: l.into(), disabled: d, separator: s };
    vec![
        it("HOME", false, false),
        it("PREVIOUS", false, false),
        it("NEXT", false, true),
        it("PRINT", false, false),
        it("STORE", false, false),
        it("UPDATE", false, true),
        it("UNDO", true, false),
        it("REDO", true, true),
        it("HELP", false, true),
        it("HIDE SWITCHING BAR", false, true),
        it("CLOSE APPLICATION", false, true),
    ]
}

pub fn flt_ops_header(heading: &str, who: &str, menu_opened: bool, functions_opened: bool) -> Node {
    Node::el("div", "oit-header-row")
        .child(page_selector("MENU", &header_items(), &format!("{who}_OIT_menu_menu"), menu_opened, "width: 300px;"))
        .child(Node::el("div", "oit-heading").text(heading))
        .child(Node::new("div").style("flex-grow: 1"))
        .child(page_selector("FUNCTIONS", &function_items(), &format!("{who}_OIT_menu_functions"), functions_opened, "width: 300px;"))
        .child(Node::el("div", "oit-msg-header").text("0 MSG"))
        .child(Node::el("div", "oit-msg-box"))
        .child(icon_button(Icon::FlatSingleDown, true, "width: 60px; height: 60px;"))
        .child(button("CLEAR", true, "", "font-size: 28px; height: 60px;"))
}

pub fn flt_ops_footer() -> Node {
    let b = |l: &str| button(l, false, "", "width: 225px; font-size: 28px; height: 50px;");
    Node::el("div", "oit-header-row")
        .child(icon_button(Icon::SingleLeft, false, "width: 60px; height: 50px; padding: 17.5px;"))
        .child(b("FLT OPS STS"))
        .child(b("CHARTS"))
        .child(b("T.O PERF"))
        .child(b("FLT FOLDER"))
        .child(b("AOC"))
        .child(Node::new("div").style("flex-grow: 1"))
        .child(icon_button(Icon::SingleRight, false, "width: 60px; height: 50px; padding: 17.5px;"))
}

fn flt_ops_login(who_capt: bool, hidden: bool) -> Node {
    Node::el("div", "oit-flt-ops-loading-screen-container")
        .class_if("hidden", hidden)
        .child(Node::el("div", "oit-login-page-heading").text("FLT OPS Domain"))
        .child(Node::el("div", "oit-login-page-sub-heading").text("Login Page"))
        .child(Node::el("div", "oit-login-page-sub-sub-heading").text(if who_capt { "CAPTAIN" } else { "FIRST OFFICER" }))
        .child(
            Node::el("div", "oit-login-page-button-container")
                .child(button("PILOT", false, "", ""))
                .child(button("MAINTAINER", true, "margin-top: 10px;", "")),
        )
        .child(
            Node::el("div", "oit-login-page-bottom-left-button").child(button("SWITCH OFF<br />LAPTOP", false, "", "")),
        )
}

fn loading_bar(hidden: bool, width: &str, text: bool) -> Node {
    let mut n = Node::el("div", "oit-flt-ops-loading-screen-container").class_if("hidden", hidden);
    if text {
        n = n.child(Node::el("div", "LoadingTextTopLeft").text("The system is loading, please wait..."));
    }
    n.child(Node::el("div", "FbwTail"))
        .child(Node::el("div", "LoadingProgressBarBackground"))
        .child(Node::el("div", "LoadingProgressBarFill").style(&format!("width: {width};")))
}

fn flt_ops_container(m: &FltOps, who: &str, capt: bool) -> Node {
    let (heading, page) = flt_ops::render(&m.page, who);
    Node::el("div", "oit-main")
        .class_if("hidden", m.hidden)
        .child(
            Node::el("div", "oit-page-container")
                .class_if("hidden", m.content_hidden)
                .child(flt_ops_header(&heading, who, m.menu_opened, m.functions_opened))
                .child(Node::el("div", "mfd-navigator-container").child(page))
                .child(flt_ops_footer()),
        )
        .child(flt_ops_login(capt, m.login_hidden))
        .child(loading_bar(m.loading_hidden, &m.loading_fill_width, false))
}

fn avncs_header(m: &Avncs) -> Node {
    Node::el("div", "oit-header-row")
        .child(button("NSS AVNCS MENU", false, "", "width: 250px; font-size: 28px; height: 50px;"))
        .child(button("MAXIMIZE", false, "", "width: 125px; font-size: 28px; height: 50px; margin-left: 5px;"))
        .child(Node::el("div", "oit-msg-header").style("padding: 11px;").text("0 MSG"))
        .child(Node::el("div", "oit-msg-box"))
        .child(icon_button(Icon::FlatSingleDown, true, "width: 60px; height: 50px; margin-left: 5px;"))
        .child(button("CLEAR", true, "", "font-size: 28px; height: 50px; margin-left: 15px;"))
        .child(Node::el("div", "oit-avncs-header-flt-nbr").text(&m.flt_number))
}

pub(crate) fn avncs_sub_header(title: &str, m: &Avncs) -> Node {
    let b = |l: &str, d: bool, ml: bool| {
        button(l, d, "", &format!("width: 130px; font-size: 28px; height: 50px;{}", if ml { " margin-left: 10px;" } else { "" }))
    };
    Node::el("div", "oit-header-row")
        .style("padding-top: 10px;")
        .child(Node::el("div", "oit-avncs-sub-header-title").text(title))
        .child(b("HOME", false, false))
        .child(b("BACK", m.back_disabled, true))
        .child(b("FORWARD", m.forward_disabled, true))
        .child(b("PRINT", true, true))
        .child(b("UPDATE", false, true))
        .child(b("HELP", true, true))
        .child(b("CLOSE", true, true))
        .child(Node::new("div").style("flex-grow: 1;"))
}

fn avncs_menu(m: &Avncs) -> Vec<Node> {
    let open = |hidden_icon: bool| FolderOpts { expanded: true, hide_icon: hidden_icon, hide_opener: false, hidden: false };
    let tree = folder(
        "OIS MENU",
        &open(true),
        vec![
            folder(
                "NSS AVNCS MENU",
                &open(true),
                vec![file("COMPANY COM", false, false, true), file("REFUEL", false, true, true), file("MAINTENANCE", false, false, true)],
            ),
            folder("UTILITIES", &open(true), vec![file("A380X SYSTEMS", false, false, true)]),
        ],
    );
    let right = Node::el("div", "it-avncs-navigator-right")
        .child(Node::el("div", "oit-avncs-navigator-right-exit-ois").child(nav_button("EXIT OIS", false, "")))
        .child(
            Node::el("div", "oit-avncs-navigator-right-center-buttons")
                .child(Node::new("span").style("font-weight: bold;").text("NSS AVNCS MENU"))
                .child(nav_button("COMPANY COM", false, "margin-top: 25px; margin-bottom: 25px;"))
                .child(nav_button("REFUEL", true, ""))
                .child(nav_button("MAINTENANCE", false, "margin-top: 25px;")),
        );
    vec![
        avncs_sub_header("NSS AVNCS MENU", m),
        Node::el("div", "oit-page-container").child(
            Node::el("div", "oit-avncs-navigator-container")
                .child(Node::el("div", "oit-avncs-navigator-left").child(tree))
                .child(right),
        ),
    ]
}

fn avncs_login(m: &Avncs) -> Node {
    Node::el("div", "oit-flt-ops-loading-screen-container")
        .class_if("hidden", m.login_hidden)
        .child(Node::el("div", "oit-login-page-heading").text("NSS AVNCS DOMAIN"))
        .child(Node::el("div", "oit-login-page-sub-heading").text("LOGIN PAGE"))
        .child(
            Node::el("div", "oit-login-page-button-container")
                .child(button("PILOT", false, "", ""))
                .child(button("MAINTAINER", true, "margin-top: 10px;", ""))
                .child(button("ADMINISTRATOR", true, "margin-top: 10px;", "")),
        )
        .child(Node::el("div", "oit-login-page-bottom-left-text").text(&m.connected_text))
}

fn avncs_container(m: &Avncs, who: &str) -> Node {
    Node::el("div", "oit-main")
        .class_if("hidden", m.hidden)
        .child(
            Node::el("div", "oit-page-container")
                .class_if("hidden", m.content_hidden)
                .child(avncs_header(m))
                .child(Node::el("div", "mfd-navigator-container").children(render_section(&m.page, m, who).unwrap_or_else(|| avncs_menu(m))))
                .child(
                    Node::el("div", "oit-header-row")
                        .child(button("COMPANY COM", false, "", "width: 225px; font-size: 28px; height: 50px;"))
                        .child(Node::new("div").style("flex-grow: 1")),
                ),
        )
        .child(avncs_login(m))
        .child(loading_bar(m.loading_hidden, &m.loading_fill_width, true))
}

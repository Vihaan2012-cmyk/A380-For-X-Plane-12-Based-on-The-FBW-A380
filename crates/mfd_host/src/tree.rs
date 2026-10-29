use std::collections::HashMap;

use mfd_pages_a::shell::{display_unit, fms_display_unit, sys_select_row};
use mfd_pages_a::widgets::{footer, page_selector, title_bar, MenuItem};
use mfd_pages_a::ShellModel;
use mfd_tree::{Kid, Node};

use crate::model;
use crate::page::Page;
use crate::sources::Sources;
use crate::ui::UiState;

pub const ACT: &str = "data-act";

fn has(n: &Node, c: &str) -> bool {
    n.cls.iter().any(|x| x == c)
}

fn page_container(kids: Vec<Node>) -> Node {
    Node::el("div", "mfd-page-container").children(kids)
}

fn fms_page(shell: &ShellModel, title: &str, page: Node) -> Node {
    let tb = title_bar(&shell.title(title, ""));
    fms_display_unit(shell, vec![tb, page, footer(&shell.footer())])
}

fn navigator_kids(n: Node) -> Vec<Node> {
    if has(&n, "mfd-navigator-container") {
        n.kids.into_iter().filter_map(|k| if let Kid::Node(c) = k { Some(c) } else { None }).collect()
    } else {
        vec![n]
    }
}

/// FBW `AtccomHeader`, `SurvHeader`, `FcuBkupHeader`: the system row and one-item page selectors.
fn sys_header(shell: &ShellModel, ui: &UiState) -> Node {
    let p = shell.id_prefix();
    let one = [MenuItem::new("", false)];
    let sel = |label: &str, active: bool, id: &str, open: bool| page_selector(label, active, &format!("{p}_pageSelector{id}"), &one, open);
    let cat = ui.uri.category.as_str();
    let open = |i: usize| ui.page_selector == Some(i);
    let row = match ui.uri.sys.as_str() {
        "atccom" => Node::el("div", "mfd-header-page-select-row")
            .child(sel("CONNECT", cat == "connect", "Connect", open(0)))
            .child(sel("REQUEST", cat == "request", "Request", open(1)))
            .child(sel("REPORT & NOTIFY", cat == "report-modify", "ReportModify", open(2)))
            .child(sel("MSG RECORD", cat == "msg-record", "MsgRecord", open(3)))
            .child(sel("D-ATIS", cat == "d-atis", "Atis", open(4)))
            .child(sel("EMER", cat == "emer", "Emer", open(5))),
        "surv" => Node::el("div", "mfd-header-page-select-row")
            .child(sel("CONTROLS", cat == "controls", "Controls", open(0)))
            .child(sel("STATUS & SWITCHING", cat == "status-switching", "StatSwitch", open(1))),
        _ => Node::el("div", "mfd-header-page-select-row")
            .style("width: 50%")
            .child(sel("AFS CP", cat == "afs", "Afs", open(0)))
            .child(sel("EFIS CP", cat == "efis", "MsgEFis", open(1))),
    };
    Node::new("div").child(sys_select_row(shell)).child(row)
}

/// One display unit (CAPT or F/O side) for the page the UI state shows.
pub fn side_tree(ui: &UiState, src: &Sources) -> Node {
    let shell = model::shell(ui, src);
    let t = |name: &str| model::page_title(ui, name);
    use mfd_pages_c as c;
    use mfd_pages_d as d;
    let mut n = match ui.page() {
        Page::Fpln => mfd_pages_a::fpln_screen(&shell, &model::fpln(ui, src)),
        Page::Init => fms_page(&shell, &t("INIT"), mfd_pages_b::init::init_page(&model::init(ui, src))),
        Page::FuelLoad => fms_page(&shell, &t("FUEL&LOAD"), mfd_pages_b::fuel::fuel_load_page(&model::fuel(ui, src))),
        Page::Perf => fms_page(&shell, &t("PERF"), mfd_pages_b::perf::perf_page(&model::perf(ui, src))),
        Page::Wind => fms_page(&shell, &t("WIND"), c::wind::wind(&model::wind(ui, src))),
        Page::VertRev => fms_page(&shell, &t("F-PLN/VERT REV"), c::vert_rev::vert_rev(&model::vert_rev(ui, src))),
        Page::Hold => fms_page(&shell, &t("F-PLN/HOLD"), page_container(c::fpln_rev::hold(&model::hold(ui, src)))),
        Page::DirTo => fms_page(&shell, &t("F-PLN/DIRECT-TO"), page_container(c::fpln_rev::direct_to(&model::dir_to(ui, src)))),
        Page::Airways => fms_page(&shell, &t("F-PLN/AIRWAYS"), page_container(c::fpln_rev::airways(&model::airways(ui, src)))),
        Page::Departure => fms_page(&shell, &t("F-PLN/DEPARTURE"), page_container(c::fpln_rev::departure(&model::departure(ui, src)))),
        Page::Arrival => fms_page(&shell, &t("F-PLN/ARRIVAL"), page_container(c::fpln_rev::arrival(&model::arrival(ui, src)))),
        Page::PosMonitor => fms_page(&shell, "POSITION/MONITOR", c::position::position_monitor(&model::position_monitor(ui, src))),
        Page::PosIrs => fms_page(&shell, "POSITION/IRS", c::position::position_irs(&model::position_irs(ui, src))),
        Page::PosNavaids => fms_page(&shell, "POSITION/NAVAIDS", c::position::position_navaids(&model::position_navaids(ui, src))),
        Page::SecIndex => fms_page(&shell, "SEC INDEX", c::sec_index::sec_index(&model::sec_index(ui, src))),
        Page::DataStatus => fms_page(&shell, "DATA/STATUS", c::data_status::data_status(&c::data_status::DataStatusModel::default())),
        Page::DataWaypoint => fms_page(&shell, "DATA/WAYPOINT", c::data_waypoint::data_waypoint(&c::data_waypoint::DataWaypointModel { selected_tab: ui.tab(Page::DataWaypoint, 0), ..Default::default() })),
        Page::DataNavaid => fms_page(&shell, "DATA/NAVAID", c::data_navaid::data_navaid(&c::data_navaid::DataNavaidModel { selected_tab: ui.tab(Page::DataNavaid, 0), ..Default::default() })),
        Page::DataAirport => fms_page(&shell, "DATA/AIRPORT", c::data_airport::data_airport(&c::data_airport::DataAirportModel { selected_tab: ui.tab(Page::DataAirport, 0), use_metric: ui.metric, custom_rwy_button_disabled: true, ..Default::default() })),
        Page::AtcConnect => display_unit(&shell, sys_header(&shell, ui), navigator_kids(d::atccom::connect_page(&Default::default()))),
        Page::AtcRequest => display_unit(&shell, sys_header(&shell, ui), navigator_kids(d::atccom::request_page(&Default::default()))),
        Page::AtcMsgRecord => display_unit(&shell, sys_header(&shell, ui), navigator_kids(d::atccom::msg_record_page(&Default::default()))),
        Page::AtcDatis => display_unit(&shell, sys_header(&shell, ui), navigator_kids(d::atccom::datis_page(&Default::default()))),
        Page::SurvControls => display_unit(&shell, sys_header(&shell, ui), navigator_kids(d::surv::controls_page(&model::surv_controls(ui, src)))),
        Page::SurvStatus => display_unit(&shell, sys_header(&shell, ui), navigator_kids(d::surv::status_page(&model::surv_status(ui, src)))),
        Page::FcuAfs => display_unit(&shell, sys_header(&shell, ui), navigator_kids(d::fcubkup::afs_page(&model::fcu_afs(ui, src)))),
        Page::FcuEfis => display_unit(&shell, sys_header(&shell, ui), navigator_kids(d::fcubkup::efis_page(&model::fcu_efis(ui, src)))),
        Page::NotFound => fms_display_unit(&shell, vec![title_bar(&shell.title("", "")), c::page_not_avail::fms_page_not_avail(), footer(&shell.footer())]),
    };
    tag(&mut n, if ui.captain { "c" } else { "f" });
    overlay(&mut n, ui);
    n
}

/// Both display units under the `a380x-mfd` element, as the instrument page lays them out.
pub fn screen(capt: &UiState, fo: &UiState, src: &Sources) -> Node {
    mfd_native::mfd_screen(side_tree(capt, src), side_tree(fo, src))
}

pub fn text_of(n: &Node) -> String {
    let mut s = String::new();
    fn walk(n: &Node, s: &mut String) {
        for k in &n.kids {
            match k {
                Kid::Text { text } => {
                    s.push_str(text);
                    s.push(' ');
                }
                Kid::Node(c) => walk(c, s),
            }
        }
    }
    walk(n, &mut s);
    s.replace('\u{a0}', " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn own_text(n: &Node) -> String {
    let mut s = String::new();
    for k in &n.kids {
        if let Kid::Text { text } = k {
            s.push_str(text);
        }
    }
    s.replace('\u{a0}', " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn first_menu_id(n: &Node) -> Option<String> {
    if has(n, "mfd-dropdown-menu-element") || has(n, "mfd-context-menu-element") {
        return n.id.clone();
    }
    n.kids.iter().find_map(|k| if let Kid::Node(c) = k { first_menu_id(c) } else { None })
}

fn strip_index(id: &str) -> String {
    match id.rsplit_once('_') {
        Some((p, i)) if i.chars().all(|c| c.is_ascii_digit()) => p.to_string(),
        _ => id.to_string(),
    }
}

fn is_down_icon(n: &Node) -> bool {
    fn any(n: &Node) -> bool {
        n.shapes.iter().any(|s| s.a.get("transform").is_some_and(|t| t.contains("rotate(180")) || s.a.get("points").is_some_and(|p| p == "0,1 15,1 7.5,14"))
            || n.cls.iter().any(|c| c.contains("down"))
            || n.kids.iter().any(|k| matches!(k, Kid::Node(c) if any(c)))
    }
    any(n)
}

#[derive(Default)]
struct Tagger {
    side: String,
    label: String,
    seen: HashMap<String, usize>,
    psel: usize,
    tabs: usize,
    fpln_line: usize,
    fpln_col: usize,
}

impl Tagger {
    fn unique(&mut self, act: String) -> String {
        let n = self.seen.entry(act.clone()).or_insert(0);
        *n += 1;
        let a = if *n == 1 { act } else { format!("{act}#{n}") };
        format!("{}|{}", self.side, a)
    }

    fn set(&mut self, n: &mut Node, act: String) {
        let a = self.unique(act);
        n.attrs.insert(ACT.into(), a);
    }

    fn walk(&mut self, n: &mut Node, in_dropdown: bool) {
        let mut in_dd = in_dropdown;
        let mut descend = true;
        if has(n, "mfd-label") && !has(n, "mfd-label-unit") {
            let t = text_of(n);
            if !t.is_empty() && !t.chars().all(|c| c == '-' || c == ' ') {
                self.label = t;
            }
        }
        if has(n, "mfd-top-tab-navigator-bar") {
            self.tabs = 0;
        }
        if has(n, "mfd-dropdown-container") {
            let menu = first_menu_id(n).map(|id| strip_index(&id));
            let is_button_menu = n.kids.iter().any(|k| matches!(k, Kid::Node(c) if has(c, "mfd-button")));
            for k in n.kids.iter_mut() {
                if let Kid::Node(c) = k {
                    if has(c, "mfd-dropdown-outer") {
                        let key = menu.clone().unwrap_or_else(|| format!("dd:{}", self.label));
                        if has(c, "inactive") {
                            self.unique(format!("dd:{key}"));
                        } else {
                            self.set(c, format!("dd:{key}"));
                        }
                    } else if has(c, "mfd-page-selector-outer") {
                        let i = self.psel;
                        self.psel += 1;
                        self.set(c, format!("psel:{i}"));
                    } else if is_button_menu && has(c, "mfd-button") {
                        match &menu {
                            Some(m) => self.set(c, format!("bmenu:{m}")),
                            None => {
                                let t = text_of(c);
                                self.set(c, format!("btn:{t}"));
                            }
                        }
                    }
                }
            }
            in_dd = true;
        } else if has(n, "mfd-dropdown-menu-element") || has(n, "mfd-context-menu-element") {
            if let Some(id) = n.id.clone() {
                self.set(n, format!("item:{id}"));
            }
            descend = false;
        } else if has(n, "mfd-radio-button") {
            if let Some(id) = n.id.clone() {
                self.set(n, format!("radio:{}", id.replace("_label_", "_")));
            }
            descend = false;
        } else if has(n, "mfd-input-field-root") && !in_dd {
            let l = match n.attrs.get("data-key") {
                Some(k) => k.clone(),
                None if self.label.is_empty() => "?".to_string(),
                None => self.label.clone(),
            };
            let dead = n.kids.iter().any(|k| matches!(k, Kid::Node(c) if has(c, "mfd-input-field-container") && (has(c, "inactive") || has(c, "disabled"))));
            if dead {
                self.unique(format!("field:{l}"));
            } else {
                self.set(n, format!("field:{l}"));
            }
        } else if has(n, "mfd-button") && !n.attrs.contains_key(ACT) && !in_dd {
            let t = text_of(n);
            self.set(n, format!("btn:{t}"));
            descend = false;
        } else if has(n, "mfd-icon-button") {
            self.set(n, if is_down_icon(n) { "icon:down".into() } else { "icon:up".into() });
            descend = false;
        } else if has(n, "mfd-top-tab-navigator-bar-element-outer") {
            let i = self.tabs;
            self.tabs += 1;
            self.set(n, format!("tab:{i}"));
            descend = false;
        } else if has(n, "mfd-fms-fpln-line") {
            self.fpln_line += 1;
            self.fpln_col = 0;
        } else if has(n, "mfd-fms-fpln-line-ident") {
            let k = self.fpln_line.saturating_sub(1);
            self.set(n, format!("fpln:ident:{k}"));
        } else if has(n, "mfd-fms-fpln-label-small") && self.fpln_line > 0 {
            let k = self.fpln_line - 1;
            self.fpln_col += 1;
            if has(n, "clickable") {
                let col = match self.fpln_col {
                    1 => "time",
                    2 => "spd",
                    _ => "alt",
                };
                self.set(n, format!("fpln:{col}:{k}"));
            }
        } else if has(n, "mfd-fms-fpln-line-destination") {
            self.fpln_line = 0;
        }
        if descend {
            for k in n.kids.iter_mut() {
                if let Kid::Node(c) = k {
                    self.walk(c, in_dd);
                }
            }
        }
    }
}

/// Gives every element the pilot can press a `data-act` id: `<side>|<kind>:<key>`,
/// with `#n` on repeats.
pub fn tag(n: &mut Node, side: &str) {
    let mut t = Tagger { side: side.into(), ..Default::default() };
    t.walk(n, false);
}

pub fn act_of(n: &Node) -> Option<&str> {
    n.attrs.get(ACT).map(String::as_str)
}

fn strip_side(act: &str) -> &str {
    act.split_once('|').map(|(_, a)| a).unwrap_or(act)
}

fn set_display(n: &mut Node, display: &str) {
    let st = n.inline.take().unwrap_or_default();
    let kept: Vec<&str> = st.split(';').map(str::trim).filter(|d| !d.is_empty() && !d.starts_with("display")).collect();
    let mut s = format!("display: {display}");
    for d in kept {
        s.push_str("; ");
        s.push_str(d);
    }
    n.inline = Some(s);
}

fn open_menus(n: &mut Node, key: &str, scroll: f64) {
    if has(n, "mfd-dropdown-container") {
        let opener = n.kids.iter().any(|k| matches!(k, Kid::Node(c) if act_of(c).map(strip_side).is_some_and(|a| a == key)));
        if opener {
            for k in n.kids.iter_mut() {
                if let Kid::Node(c) = k {
                    if has(c, "mfd-dropdown-menu") {
                        set_display(c, "block");
                        if scroll > 0. {
                            c.attrs.insert("data-st".into(), format!("{scroll}"));
                        }
                    } else if has(c, "mfd-button") && !has(c, "opened") {
                        c.cls.push("opened".into());
                    }
                }
            }
        }
    }
    for k in n.kids.iter_mut() {
        if let Kid::Node(c) = k {
            open_menus(c, key, scroll);
        }
    }
}

fn count_menu_elements(n: &Node) -> usize {
    usize::from(has(n, "mfd-dropdown-menu-element")) + n.kids.iter().map(|k| if let Kid::Node(c) = k { count_menu_elements(c) } else { 0 }).sum::<usize>()
}

/// How many items the menu opened by `key` (`dd:<id>` or `bmenu:<id>`) holds.
pub fn menu_items(n: &Node, key: &str) -> Option<usize> {
    if has(n, "mfd-dropdown-container") && n.kids.iter().any(|k| matches!(k, Kid::Node(c) if act_of(c).map(strip_side).is_some_and(|a| a == key))) {
        for k in &n.kids {
            if let Kid::Node(c) = k {
                if has(c, "mfd-dropdown-menu") {
                    return Some(count_menu_elements(c));
                }
            }
        }
    }
    n.kids.iter().find_map(|k| if let Kid::Node(c) = k { menu_items(c, key) } else { None })
}

fn edit_field(n: &mut Node, act: &str, text: &str) -> bool {
    if act_of(n) == Some(act) {
        fn apply(n: &mut Node, text: &str) {
            if has(n, "mfd-input-field-text-input") {
                n.cls.retain(|c| !matches!(c.as_str(), "valueSelected" | "editing" | "computedByFms" | "tmpy" | "mandatory"));
                n.cls.push("editing".into());
                if text.is_empty() {
                    n.cls.push("valueSelected".into());
                } else {
                    n.kids = vec![Kid::Text { text: text.to_string() }];
                }
            } else if has(n, "mfd-input-field-caret") && !text.is_empty() {
                set_display(n, "inline");
            } else if has(n, "mfd-input-field-text-input-container") && !text.is_empty() {
                let st = n.inline.take().unwrap_or_default();
                let kept: Vec<&str> = st.split(';').map(str::trim).filter(|d| !d.is_empty() && !d.starts_with("justify-content")).collect();
                let mut s = "justify-content: flex-start".to_string();
                for d in kept {
                    s.push_str("; ");
                    s.push_str(d);
                }
                n.inline = Some(s);
            }
            for k in n.kids.iter_mut() {
                if let Kid::Node(c) = k {
                    apply(c, text);
                }
            }
        }
        apply(n, text);
        return true;
    }
    n.kids.iter_mut().any(|k| match k {
        Kid::Node(c) => edit_field(c, act, text),
        _ => false,
    })
}

/// The UI state the page builders do not carry: the open menu and the field being typed into.
pub fn overlay(n: &mut Node, ui: &UiState) {
    let side = if ui.captain { "c" } else { "f" };
    if let Some(open) = &ui.open {
        for key in [format!("dd:{open}"), format!("bmenu:{open}")] {
            open_menus(n, &key, ui.menu_scroll);
        }
        if ui.focus.is_none() {
            edit_field(n, &format!("{side}|dd:{open}"), "");
        }
    }
    if let Some(f) = &ui.focus {
        edit_field(n, &format!("{side}|{f}"), &ui.scratchpad);
    }
}

pub fn find_act<'a>(n: &'a Node, act: &str) -> Option<&'a Node> {
    if act_of(n) == Some(act) {
        return Some(n);
    }
    n.kids.iter().find_map(|k| if let Kid::Node(c) = k { find_act(c, act) } else { None })
}

pub fn acts(n: &Node, out: &mut Vec<String>) {
    if let Some(a) = act_of(n) {
        out.push(a.to_string());
    }
    for k in &n.kids {
        if let Kid::Node(c) = k {
            acts(c, out);
        }
    }
}

pub fn own(n: &Node) -> String {
    own_text(n)
}

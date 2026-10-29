use crate::navigator::{file, folder, FolderOpts};
use crate::perf_common::*;
use crate::shell::prefix;
use crate::ops_pages::*;
use crate::perf_pages::{in_flt_perf_page, ldg_perf_page, InFltModel, LdgPerfModel};
use crate::to_compute::{compute_results, ToCompute};
use crate::widgets::{button, div, dropdown, input_field, radio_group, Dropdown, Field};
use mfd_tree::Node;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FltOpsPage {
    Menu,
    FltFolder(FltFolderModel),
    Charts(ChartsModel),
    Sts(StsModel),
    ToPerf(Box<ToPerfModel>),
    Aoc(AocModel),
    LdgPerf(Box<LdgPerfModel>),
    InFltPerf(Box<InFltModel>),
    Loadsheet(LoadsheetModel),
    OpsLibrary(Box<OpsLibraryModel>),
    LoadBox(LoadBoxModel),
    ExportBox(ExportBoxModel),
    Other { uri: String },
}

impl Default for FltOpsPage {
    fn default() -> Self {
        FltOpsPage::Menu
    }
}

pub fn heading(page: &FltOpsPage) -> String {
    match page {
        FltOpsPage::Menu => "FLT OPS MENU".into(),
        FltOpsPage::FltFolder(_) => "FLT FOLDER".into(),
        FltOpsPage::Charts(_) => "CHARTS".into(),
        FltOpsPage::Sts(_) => "STATUS PAGE".into(),
        FltOpsPage::ToPerf(_) => "T.O PERF".into(),
        FltOpsPage::Aoc(_) => "AOC".into(),
        FltOpsPage::LdgPerf(_) => "LDG PERF".into(),
        FltOpsPage::InFltPerf(_) => "IN-FLT PERF".into(),
        FltOpsPage::Loadsheet(_) => "LOADSHEET".into(),
        FltOpsPage::OpsLibrary(_) => "OPS LIBRARY".into(),
        FltOpsPage::LoadBox(_) => "LOAD BOX".into(),
        FltOpsPage::ExportBox(_) => "EXPORT BOX".into(),
        FltOpsPage::Other { uri } => match uri.as_str() {
            "flt-ops/ldg-perf" => "LDG PERF".into(),
            "flt-ops/load-box" => "LOAD BOX".into(),
            "flt-ops/export-box" => "EXPORT BOX".into(),
            "flt-ops/loadsheet" => "LOADSHEET".into(),
            "flt-ops/in-flt-perf" => "IN-FLT PERF".into(),
            "flt-ops/ops-library" => "OPS LIBRARY".into(),
            other => other.to_uppercase(),
        },
    }
}

pub fn render(page: &FltOpsPage, who: &str) -> (String, Node) {
    let node = match page {
        FltOpsPage::Menu => menu_page(),
        FltOpsPage::FltFolder(m) => flt_folder_page(m),
        FltOpsPage::Charts(m) => charts_page(m),
        FltOpsPage::Sts(m) => sts_page(m, who),
        FltOpsPage::ToPerf(m) => to_perf_page(m, who),
        FltOpsPage::Aoc(m) => aoc_page(m),
        FltOpsPage::LdgPerf(m) => ldg_perf_page(m, who),
        FltOpsPage::InFltPerf(m) => in_flt_perf_page(m),
        FltOpsPage::Loadsheet(m) => loadsheet_page(m),
        FltOpsPage::OpsLibrary(m) => ops_library_page(m),
        FltOpsPage::LoadBox(m) => load_box_page(m),
        FltOpsPage::ExportBox(m) => export_box_page(m),
        FltOpsPage::Other { .. } => Node::el("div", "oit-page-container"),
    };
    (heading(page), node)
}

pub fn render_page_only(page: &FltOpsPage, who_capt: bool) -> Node {
    render(page, prefix(who_capt)).1
}

fn menu_button(label: &str) -> Node {
    button(label, false, "width: 300px; margin-bottom: 20px", "")
}

fn menu_column(title: &str, labels: &[&str]) -> Node {
    Node::el("div", "oit-flt-ops-menu-column")
        .child(Node::el("div", "oit-flt-ops-menu-column-title").text(title))
        .children(labels.iter().map(|l| menu_button(l)))
}

pub fn menu_page() -> Node {
    Node::el("div", "oit-page-container")
        .child(
            Node::el("div", "fr")
                .child(menu_column("MISSION", &["FLT FOLDER", "CHARTS"]))
                .child(menu_column("DOCUMENTATION", &["OPS LIBRARY"]))
                .child(menu_column("PERFORMANCE", &["T.O PERF", "LOADSHEET", "LDG PERF", "IN-FLT PERF"]))
                .child(menu_column("UTILITIES", &["FLT OPS STS", "LOAD BOX", "EXPORT BOX"])),
        )
        .child(Node::new("div").style("flex-grow: 1"))
        .child(button("EXIT SESSION", false, "width: 300px; margin-bottom: 20px; align-self: flex-end;", ""))
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct OfpSection {
    pub group: String,
    pub title: String,
    pub level: u32,
    pub text: Option<String>,
    pub image: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Ofp {
    pub flight: String,
    pub origin: String,
    pub destination: String,
    pub alternate: String,
    pub generated: String,
    pub sections: Vec<OfpSection>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Message {
    pub text: String,
    pub warn: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct FltFolderModel {
    pub tree_hidden: bool,
    pub ofp: Option<Ofp>,
    pub selected: Option<usize>,
    pub message: Option<Message>,
}

fn has_body(s: &OfpSection) -> bool {
    s.text.as_deref().map_or(false, |t| !t.is_empty()) || s.image.as_deref().map_or(false, |t| !t.is_empty())
}

pub fn flt_folder_page(m: &FltFolderModel) -> Node {
    let mut summary = String::new();
    let mut tree = div("oefb-tree");
    let mut view = div("oefb-view");
    let mut first: Option<usize> = None;
    if let Some(ofp) = &m.ofp {
        let route = if !ofp.origin.is_empty() && !ofp.destination.is_empty() {
            format!("{} - {}", ofp.origin, ofp.destination)
        } else {
            String::new()
        };
        let altn = if ofp.alternate.is_empty() { String::new() } else { format!("ALTN {}", ofp.alternate) };
        summary = [ofp.flight.clone(), route, altn, ofp.generated.clone()]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
            .join("   ");
        let mut group = String::new();
        let selected = m.selected.or_else(|| ofp.sections.iter().position(has_body));
        for (i, s) in ofp.sections.iter().enumerate() {
            if s.group != group {
                group = s.group.clone();
                tree = tree.child(div("oefb-group").text(&group));
            }
            if !has_body(s) {
                tree = tree.child(div("oefb-heading").text(&s.title));
                continue;
            }
            if first.is_none() {
                first = Some(i);
            }
            tree = tree.child(
                div("oefb-item").class_if("l1", s.level > 0).class_if("sel", selected == Some(i)).text(&s.title),
            );
        }
        if let Some(i) = selected.filter(|i| *i < ofp.sections.len()) {
            let s = &ofp.sections[i];
            if s.image.as_deref().map_or(false, |x| !x.is_empty()) {
                view = view.child(div("oefb-view-title").text(&s.title)).child(Node::el("img", "oefb-picture"));
            } else {
                view = view.child(div("oefb-mono").text(s.text.as_deref().unwrap_or("")));
            }
        } else if first.is_none() {
            view = view.child(div("oefb-msg").text("THE OFP HAS NO SECTIONS"));
        }
    }
    if let Some(msg) = &m.message {
        let mut v = div("oefb-view");
        v = v.child(div(if msg.warn { "oefb-msg warn" } else { "oefb-msg" }).text(&msg.text));
        view = v;
        if msg.warn {
            tree = div("oefb-tree");
            summary.clear();
        }
    }
    let tree = if m.tree_hidden { tree.style("display: none;") } else { tree };
    Node::el("div", "oit-page-container oefb")
        .child(
            div("oefb-bar")
                .child(div("oefb-title").text("FLT FOLDER"))
                .child(div("oefb-sub").text(&summary))
                .child(div("oefb-spacer"))
                .child(div("oefb-btn").text(if m.tree_hidden { "SHOW TREE" } else { "HIDE TREE" }))
                .child(div("oefb-btn primary").text("FETCH OFP")),
        )
        .child(div("oefb-body").child(tree).child(view))
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Chip {
    pub label: String,
    pub icao: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ChartGroup {
    pub name: String,
    pub items: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChartList {
    Message { text: String },
    Groups { groups: Vec<ChartGroup>, selected: Option<usize> },
}

impl Default for ChartList {
    fn default() -> Self {
        ChartList::Message { text: String::new() }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChartStage {
    Message { text: String, warn: bool },
    Image {
        #[serde(default)]
        view: SheetView,
    },
    Pdf,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct SheetView {
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
    pub rotation: f64,
    pub night_filter: bool,
}

impl Default for SheetView {
    fn default() -> Self {
        SheetView { pan_x: 0.0, pan_y: 0.0, zoom: 1.0, rotation: 0.0, night_filter: false }
    }
}

pub fn fit_zoom(stage_w: f64, stage_h: f64, natural_w: f64, natural_h: f64, turned: bool) -> f64 {
    let (w, h) = if turned { (natural_h, natural_w) } else { (natural_w, natural_h) };
    if w > 0.0 && h > 0.0 { ((stage_w - 20.0) / w).min((stage_h - 20.0) / h) } else { 1.0 }
}

fn sheet_style(v: &SheetView) -> String {
    let n = |x: f64| format!("{}", x + 0.0);
    format!(
        "transform: translate(-50%, -50%) translate({}px, {}px) scale({}) rotate({}deg); filter: {};",
        n(v.pan_x),
        n(v.pan_y),
        n(v.zoom),
        n(v.rotation),
        if v.night_filter { "invert(0.92) hue-rotate(180deg)" } else { "none" }
    )
}

impl Default for ChartStage {
    fn default() -> Self {
        ChartStage::Message { text: String::new(), warn: false }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct ChartsModel {
    pub enroute: bool,
    pub source: String,
    pub status: String,
    pub chips: Vec<Chip>,
    pub icao: String,
    pub list: ChartList,
    pub stage: ChartStage,
    pub night: bool,
}

impl Default for ChartsModel {
    fn default() -> Self {
        ChartsModel {
            enroute: false,
            source: "navigraph".into(),
            status: String::new(),
            chips: vec![],
            icao: String::new(),
            list: ChartList::default(),
            stage: ChartStage::default(),
            night: false,
        }
    }
}

fn segment(items: &[(&str, &str)], current: &str, classes: &str, hidden: bool) -> Node {
    let mut n = div(classes);
    if hidden {
        n = n.style("display: none;");
    }
    for (value, label) in items {
        n = n.child(div(if *value == current { "on" } else { "" }).text(label));
    }
    n
}

pub fn charts_page(m: &ChartsModel) -> Node {
    let terminal_hidden = m.enroute;
    let mut chips = div("oefb-chips");
    if terminal_hidden {
        chips = chips.style("display: none;");
    }
    for c in &m.chips {
        chips = chips.child(
            div("oefb-chip")
                .child(Node::el("span", "k").text(&c.label))
                .child(Node::new("span").text(&c.icao)),
        );
    }
    let mut input = Node::el("input", "oefb-input").value(&m.icao);
    if terminal_hidden {
        input = input.style("display: none;");
    }
    let mut tree = div("oefb-tree");
    match &m.list {
        ChartList::Message { text } => tree = tree.child(div("oefb-msg small").text(text)),
        ChartList::Groups { groups, selected } => {
            let mut flat = 0usize;
            for g in groups {
                tree = tree.child(div("oefb-group").text(&g.name));
                for it in &g.items {
                    tree = tree.child(div("oefb-item").class_if("sel", *selected == Some(flat)).text(it));
                    flat += 1;
                }
            }
        }
    }
    let stage = match &m.stage {
        ChartStage::Message { text, warn } => div("oefb-stage").child(div(if *warn { "oefb-msg warn" } else { "oefb-msg" }).text(text)),
        ChartStage::Image { view } => div("oefb-stage").child(Node::el("img", "oefb-sheet").style(&sheet_style(view))),
        ChartStage::Pdf => div("oefb-stage").child(Node::el("iframe", "oefb-pdf")),
    };
    Node::el("div", "oit-page-container oefb")
        .child(
            div("oefb-bar")
                .child(div("oefb-title").text("CHARTS"))
                .child(segment(
                    &[("TERMINAL", "TERMINAL"), ("ENROUTE", "ENROUTE")],
                    if m.enroute { "ENROUTE" } else { "TERMINAL" },
                    "oefb-seg",
                    false,
                ))
                .child(div("oefb-sub").text(&m.status))
                .child(div("oefb-spacer"))
                .child(chips)
                .child(input)
                .child(div("oefb-btn primary").text("LOAD")),
        )
        .child(
            div("oefb-body")
                .child(
                    div("oefb-side")
                        .child(segment(
                            &[("navigraph", "NAVIGRAPH"), ("faa", "FAA"), ("local", "MY CHARTS")],
                            &m.source,
                            "oefb-seg full",
                            terminal_hidden,
                        ))
                        .child(tree),
                )
                .child(
                    div("oefb-viewer").child(stage).child(
                        div("oefb-tools")
                            .child(div("oefb-btn").text("-"))
                            .child(div("oefb-btn").text("+"))
                            .child(div("oefb-btn").text("FIT"))
                            .child(div("oefb-btn").text("ROTATE"))
                            .child(div("oefb-btn").text(if m.night { "DAY" } else { "NIGHT" })),
                    ),
                ),
        )
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct StsModel {
    pub registration: Dropdown,
    pub msn: String,
    pub ois_version: String,
    pub flt_number: Field,
    pub from_airport: Field,
    pub to_airport: Field,
}

fn sts_line(left: &str, right: Node) -> Node {
    div("oit-flt-ops-sts-line")
        .child(div("oit-flt-ops-sts-line-left oit-label bigger").text(left))
        .child(right)
}

fn sts_right() -> Node {
    div("oit-flt-ops-sts-line-right oit-label bigger")
}

pub fn sts_page(m: &StsModel, who: &str) -> Node {
    let mut reg = m.registration.clone();
    reg.container_style = "width: 600px;".into();
    reg.digits = 8;
    let mut flt = m.flt_number.clone();
    flt.container_style = "width: 600px; margin-right: 5px;".into();
    flt.max_digits = 10;
    let mut from = m.from_airport.clone();
    from.container_style = "width: 250px; margin-right: 5px;".into();
    from.max_digits = 4;
    let mut to = m.to_airport.clone();
    to.container_style = "width: 250px; margin-right: 5px;".into();
    to.max_digits = 4;
    Node::el("div", "oit-page-container framed")
        .child(
            div("oit-flt-ops-sts-upper")
                .child(sts_line(
                    "ACFT REGISTRATION",
                    sts_right()
                        .child(dropdown(&format!("{who}_OIT_acftRegistration"), &reg))
                        .child(Node::new("div").style("width: 40px;"))
                        .child(button("SYNCHRO<br />AVIONICS", false, "width: 175px;", ""))
                        .child(Node::new("div").style("flex-grow: 1")),
                ))
                .child(sts_line("MSN", Node::el("div", "oit-flt-ops-sts-line-right oit-label bigger cyan").text(&m.msn)))
                .child(sts_line("OIS VERSION", sts_right().text(&m.ois_version))),
        )
        .child(
            div("oit-flt-ops-sts-lower")
                .child(sts_line(
                    "FLT NUMBER",
                    sts_right().child(input_field(&flt)).child(Node::new("div").style("flex-grow: 1")),
                ))
                .child(sts_line(
                    "FROM",
                    sts_right()
                        .child(input_field(&from))
                        .child(Node::el("div", "oit-label bigger").style("width: 100px; padding-left: 40px;").text("TO"))
                        .child(input_field(&to))
                        .child(Node::new("div").style("flex-grow: 1")),
                )),
        )
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Runway {
    pub ident: String,
    pub elevation_ft: f64,
    pub slope_percent: f64,
    pub tora: f64,
    pub asda: f64,
    pub lda: f64,
    pub width: f64,
    pub bearing_true: f64,
}

impl Default for Runway {
    fn default() -> Self {
        Runway { ident: String::new(), elevation_ft: 0.0, slope_percent: 0.0, tora: 0.0, asda: 0.0, lda: 0.0, width: 0.0, bearing_true: 0.0 }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Weather {
    pub oat: Option<f64>,
    pub qnh: Option<f64>,
    pub wind_direction_true: Option<f64>,
    pub wind_kt: Option<f64>,
    pub source: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Weights {
    pub zero_fuel_weight_kg: Option<f64>,
    pub zero_fuel_cg: Option<f64>,
    pub fob_kg: Option<f64>,
    pub gross_weight_kg: Option<f64>,
    pub cg: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResultSource {
    None,
    Simbrief,
    Model,
}

impl Default for ResultSource {
    fn default() -> Self {
        ResultSource::None
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct PerfResults {
    pub source: ResultSource,
    pub runway_text: String,
    pub v1: String,
    pub vr: String,
    pub v2: String,
    pub flaps: String,
    pub flex: String,
    pub ths: String,
    pub mtow: String,
    pub stop_margin: String,
    pub note: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct ToPerfModel {
    pub units: Units,
    pub airport: Field,
    pub runway_dropdown: Dropdown,
    pub database_message: String,
    pub runway: Option<Runway>,
    pub takeoff_shift: Option<f64>,
    pub shift_field: Field,
    pub entry_angle: Dropdown,
    pub weather: Weather,
    pub rwy_cond: Dropdown,
    pub air_cond: Dropdown,
    pub flaps: Dropdown,
    pub anti_ice: Dropdown,
    pub thrust: Dropdown,
    pub weights: Weights,
    pub max_zfw_kg: f64,
    pub max_gw_kg: f64,
    pub thr_red_ft: Option<f64>,
    pub eo_accel_ft: Option<f64>,
    pub results: PerfResults,
    pub compute: Option<ToCompute>,
}

impl Default for ToPerfModel {
    fn default() -> Self {
        ToPerfModel {
            units: Units::default(),
            airport: Field::default(),
            runway_dropdown: Dropdown::default(),
            database_message: String::new(),
            runway: None,
            takeoff_shift: None,
            shift_field: Field::default(),
            entry_angle: Dropdown::default(),
            weather: Weather::default(),
            rwy_cond: Dropdown::default(),
            air_cond: Dropdown::default(),
            flaps: Dropdown::default(),
            anti_ice: Dropdown::default(),
            thrust: Dropdown::default(),
            weights: Weights::default(),
            max_zfw_kg: 373_000.0,
            max_gw_kg: A380_MAX_GW_KG,
            thr_red_ft: None,
            eo_accel_ft: None,
            results: PerfResults::default(),
            compute: None,
        }
    }
}

pub const ENTRY_ANGLES: [&str; 3] = ["0", "90", "180"];
pub const RUNWAY_CONDITIONS: [&str; 2] = ["DRY", "WET"];
pub const ANTI_ICE: [&str; 3] = ["OFF [STD]", "ON [ENG]", "ON [ENG & WING]"];
pub const AIR_COND: [&str; 2] = ["ON [STD]", "OFF"];
pub const FLAPS: [&str; 4] = ["OPTIMUM", "1+F", "2", "3"];
pub const THRUST: [&str; 2] = ["FLEX [STD]", "TOGA"];

fn label(text: &str) -> Node {
    Node::el("div", "oit-label bigger").text(text)
}

fn green(text: &str) -> Node {
    Node::el("div", "oit-label bigger oit-green-text").text(text)
}

fn grid(pairs: Vec<(&str, Node)>) -> Node {
    let mut g = div("oit-perf-grid");
    for (k, v) in pairs {
        g = g.child(label(k)).child(v);
    }
    g
}

fn box_with(title: &str) -> Node {
    Node::el("div", "oit-labeled-box-container").child(Node::el("span", "oit-labeled-box-label").text(title))
}

fn selector(label_text: &str, d: &Dropdown, values: &[&str], id: &str, width: u32, who: &str) -> Node {
    let mut d = d.clone();
    d.values = values.iter().map(|s| s.to_string()).collect();
    d.container_style = format!("width: {width}px;");
    d.digits = values.iter().map(|v| v.chars().count() as u32).max().unwrap_or(0);
    div("oit-perf-field")
        .child(Node::el("div", "oit-label bigger oit-perf-field-key").text(label_text))
        .child(dropdown(&format!("{who}_OIT_{id}"), &d))
}

pub fn to_perf_page(m: &ToPerfModel, who: &str) -> Node {
    let u = &m.units;
    let rwy = m.runway.as_ref();
    let shift = m.takeoff_shift.unwrap_or(0.0);
    let usable_tora = rwy.map(|r| (r.tora - shift).max(0.0));
    let usable_asda = rwy.map(|r| (r.asda - shift).max(0.0));
    let dist = |v: Option<f64>| format!("{} {}", format_distance(v, u), distance_unit_label(u));

    let mut airport = m.airport.clone();
    airport.container_style = "width: 160px; margin-right: 20px;".into();
    airport.max_digits = 4;
    let mut rdd = m.runway_dropdown.clone();
    rdd.container_style = "width: 180px; margin-right: 20px;".into();
    rdd.digits = 4;
    let mut entry = m.entry_angle.clone();
    entry.values = ENTRY_ANGLES.iter().map(|s| s.to_string()).collect();
    entry.container_style = "width: 150px; margin-right: 20px;".into();
    entry.digits = 3;
    let mut shift_field = m.shift_field.clone();
    shift_field.container_style = "width: 190px;".into();
    shift_field.trailing = "M".into();
    shift_field.max_digits = 4;

    let slope = rwy.map_or("----".to_string(), |r| {
        format!("{} {}%", if r.slope_percent >= 0.0 { "UP" } else { "DOWN" }, fixed(r.slope_percent.abs(), 2))
    });
    let departure = box_with("DEPARTURE")
        .child(
            div("fr oit-perf-row")
                .child(Node::el("div", "oit-label bigger oit-perf-key").text("AIRPORT"))
                .child(input_field(&airport))
                .child(Node::el("div", "oit-label bigger oit-perf-field-key").text("RWY"))
                .child(dropdown(&format!("{who}_OIT_toPerfRunway"), &rdd))
                .child(button("SYNC FMS", false, "width: 175px; margin-right: 10px;", ""))
                .child(button("SYNC SIMBRIEF", false, "width: 220px;", "")),
        )
        .child(Node::el("div", "oit-label amber oit-perf-note").text(&m.database_message))
        .child(grid(vec![
            ("ELEVN", green(&rwy.map_or("-----".to_string(), |r| format!("{} FT", fixed(js_round(r.elevation_ft), 0))))),
            ("SLOPE", green(&slope)),
            ("TORA", green(&dist(usable_tora))),
            ("ASDA", green(&dist(usable_asda))),
            ("TODA", Node::el("div", "oit-label bigger amber").text(UNAVAILABLE)),
            ("WIDTH", green(&rwy.map_or("----".to_string(), |r| dist(Some(r.width))))),
        ]))
        .child(
            div("fr oit-perf-row")
                .child(Node::el("div", "oit-label bigger oit-perf-key").text("ENTRY ANGLE"))
                .child(dropdown(&format!("{who}_OIT_toPerfEntryAngle"), &entry))
                .child(Node::el("div", "oit-label bigger oit-perf-key").text("T.O SHIFT"))
                .child(input_field(&shift_field)),
        )
        .child(Node::el("div", "oit-label oit-perf-note").text(
            "INTERSECTION NAMES ARE NOT IN THE NAVIGATION DATABASE; ENTER THE INTERSECTION AS A T.O SHIFT FROM THE RUNWAY THRESHOLD. TODA NEEDS A PUBLISHED CLEARWAY, WHICH THE DATABASE DOES NOT CARRY.",
        ));

    let w = &m.weather;
    let wind_text = match (w.wind_direction_true, w.wind_kt) {
        (Some(d), Some(k)) => format!("{:0>3}°T/{:0>3}KT", fixed(js_round(d), 0), fixed(js_round(k), 0)),
        _ => "---°/---".into(),
    };
    let (headwind, crosswind, press_alt) = match (rwy, w.wind_direction_true, w.wind_kt, w.qnh) {
        (Some(r), Some(d), Some(k), Some(q)) => {
            let off = (d - r.bearing_true).to_radians();
            let hw = k * off.cos();
            let xw = k * off.sin();
            (
                format!("{} {}KT", if hw >= 0.0 { "HEAD" } else { "TAIL" }, fixed(js_round(hw.abs()), 0)),
                format!("{} {}KT", if xw >= 0.0 { "R" } else { "L" }, fixed(js_round(xw.abs()), 0)),
                format!("{} FT", fixed(js_round(r.elevation_ft + (1013.25 - q) * 27.0), 0)),
            )
        }
        _ => ("---".into(), "---".into(), "-----".into()),
    };
    let conditions = box_with("CONDITIONS")
        .child(grid(vec![
            ("OAT", green(&w.oat.map_or("---".to_string(), |v| format!("{}°C", fixed(js_round(v), 0))))),
            (
                "QNH",
                green(&w.qnh.map_or("----".to_string(), |q| format!("{} {}", format_pressure(q, u), pressure_unit_label(u)))),
            ),
            ("WIND", green(&wind_text)),
            ("PRESS ALT", green(&press_alt)),
            ("HEADWIND", green(&headwind)),
            ("CROSSWIND", green(&crosswind)),
        ]))
        .child(
            div("fr oit-perf-row")
                .child(selector("RWY COND", &m.rwy_cond, &RUNWAY_CONDITIONS, "toPerfRwyCond", 130, who))
                .child(selector("AIR COND", &m.air_cond, &AIR_COND, "toPerfAirCond", 200, who))
                .child(selector("FLAPS", &m.flaps, &FLAPS, "toPerfFlaps", 190, who)),
        )
        .child(
            div("fr oit-perf-row")
                .child(selector("ANTI ICE", &m.anti_ice, &ANTI_ICE, "toPerfAntiIce", 310, who))
                .child(selector("THRUST", &m.thrust, &THRUST, "toPerfThrust", 230, who))
                .child(button("SYNC WX", false, "width: 160px;", "")),
        )
        .child(Node::el("div", "oit-label oit-perf-note").text("WEATHER SOURCE: ").text(&w.source));

    let wt = &m.weights;
    let wlabel = weight_unit_label(u);
    let cg = |v: Option<f64>| format!("{} %", v.filter(|x| x.is_finite()).map_or("--.-".to_string(), |x| fixed(x, 1)));
    let mut warnings: Vec<&str> = vec![];
    if wt.zero_fuel_weight_kg.map_or(false, |z| z > m.max_zfw_kg) {
        warnings.push("ZFW ABOVE STRUCTURAL LIMIT");
    }
    if wt.gross_weight_kg.map_or(false, |g| g > m.max_gw_kg) {
        warnings.push("GW ABOVE STRUCTURAL LIMIT");
    }
    let weights = box_with("WEIGHTS")
        .child(grid(vec![
            ("ZFW", green(&format!("{} {}", format_weight(wt.zero_fuel_weight_kg, u), wlabel))),
            ("ZFWCG", green(&cg(wt.zero_fuel_cg))),
            ("FOB", green(&format!("{} {}", format_weight(wt.fob_kg, u), wlabel))),
            ("TOW", green(&format!("{} {}", format_weight(wt.gross_weight_kg, u), wlabel))),
            ("TOCG", green(&cg(wt.cg))),
            ("STRUCT MTOW", green(&format!("{} {}", format_weight(Some(m.max_gw_kg), u), wlabel))),
        ]))
        .child(Node::el("div", "oit-label amber oit-perf-note").text(&warnings.join(" - ")));

    let computed_results;
    let r = match &m.compute {
        Some(c) => {
            computed_results = compute_results(m, c);
            &computed_results
        }
        None => &m.results,
    };
    let result_cell = |t: &str| {
        Node::el("div", "oit-label bigger")
            .class_if("cyan", r.source == ResultSource::Simbrief)
            .class_if("oit-green-text", r.source == ResultSource::Model)
            .class_if("amber", r.source == ResultSource::None)
            .text(t)
    };
    let alt = |v: Option<f64>| v.map_or("-----".to_string(), |x| format!("{} FT", fixed(js_round(x), 0)));
    let results = box_with("RESULTS")
        .child(
            div("fr oit-perf-row")
                .child(button("COMPUTE", false, "width: 200px; margin-right: 20px;", ""))
                .child(button("RECHECK WITH AVNCS", true, "width: 320px;", ""))
                .child(Node::new("div").style("flex-grow: 1"))
                .child(label("RWY"))
                .child(green(&r.runway_text).style("margin-left: 15px;")),
        )
        .child(grid(vec![
            ("V1", result_cell(&r.v1)),
            ("FLAPS", result_cell(&r.flaps)),
            ("VR", result_cell(&r.vr)),
            ("FLEX", result_cell(&r.flex)),
            ("V2", result_cell(&r.v2)),
            ("THS", result_cell(&r.ths)),
            ("MTOW (PERF)", result_cell(&r.mtow)),
            ("STOP MARGIN", result_cell(&r.stop_margin)),
            ("THR RED", green(&alt(m.thr_red_ft))),
            ("EO ACCEL", green(&alt(m.eo_accel_ft))),
        ]))
        .child(Node::el("div", "oit-label amber oit-perf-note").text(&r.note));

    let options = Node::el("div", "oit-labeled-box-container oit-perf-options")
        .child(Node::el("span", "oit-labeled-box-label").text("OPTIONS"))
        .child(
            div("fr")
                .child(
                    div("oit-perf-options-column")
                        .child(Node::el("div", "oit-label").text("WEIGHT"))
                        .child(radio_group("oit-perf-opt-weight", &["T", "KLB"])),
                )
                .child(
                    div("oit-perf-options-column")
                        .child(Node::el("div", "oit-label").text("ALTIMETER"))
                        .child(radio_group("oit-perf-opt-alt", &["HPA", "IN HG"])),
                )
                .child(
                    div("oit-perf-options-column")
                        .child(Node::el("div", "oit-label").text("DISTANCE"))
                        .child(radio_group("oit-perf-opt-dist", &["M", "FT"])),
                ),
        );

    Node::el("div", "oit-page-container oit-perf-page")
        .child(departure)
        .child(conditions)
        .child(weights)
        .child(results)
        .child(options)
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct AocMsg {
    pub title: String,
    pub zulu: String,
    pub is_new: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct AocOpen {
    pub from: String,
    pub zulu: String,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct AocModel {
    pub folder: String,
    pub inboxes_expanded: bool,
    pub messages: Vec<AocMsg>,
    pub open: Option<AocOpen>,
}

impl Default for AocModel {
    fn default() -> Self {
        AocModel { folder: "INBOX".into(), inboxes_expanded: true, messages: vec![], open: None }
    }
}

const COMPOSE_FOLDERS: [&str; 4] = ["REQUESTS", "REPORTS", "ADC DELAY", "DIVERSION"];

fn aoc_entry(label_text: &str, current: &str) -> Node {
    file(label_text, label_text == current, false, false).style("cursor: pointer;")
}

fn big_label(t: &str) -> Node {
    Node::el("div", "oit-label bigger").text(t)
}

fn form_row(label: &str, control: Node) -> Node {
    div("fr").style("align-items: center; margin-bottom: 20px;").child(big_label(label).style("width: 200px;")).child(control)
}

fn compose(folder: &str) -> Node {
    let col = "flex-direction: column; align-items: flex-start; padding: 20px;";
    let root = Node::el("div", "oit-centered").style(col);
    match folder {
        "REQUESTS" => root
            .child(big_label("LOADSHEET"))
            .child(
                div("fr")
                    .style("margin: 10px 0 20px 0;")
                    .child(button("REQUEST PRELIM", false, "width: 260px; margin-right: 20px;", ""))
                    .child(button("REQUEST FINAL", false, "width: 260px;", "")),
            )
            .child(big_label("METAR"))
            .child(
                div("fr")
                    .style("margin: 10px 0 20px 0; align-items: center;")
                    .child(Node::new("input").style("width: 120px; font-size: 24px; margin-right: 20px; text-transform: uppercase;").value(""))
                    .child(button("SEND", false, "width: 150px;", "")),
            )
            .child(big_label("WX"))
            .child(div("fr").style("margin: 10px 0; align-items: center;").child(button("SEND WX REQUEST", false, "width: 260px;", ""))),
        "REPORTS" => root
            .child(big_label("REPORT TEXT"))
            .child(Node::new("textarea").style("width: 100%; font-size: 22px; margin: 10px 0;").value(""))
            .child(button("SEND", false, "width: 150px;", "")),
        "ADC DELAY" => root
            .child(form_row("DELAY CODE", Node::new("select").style("font-size: 24px; width: 120px;")))
            .child(form_row("DELAY MINUTES", Node::new("input").style("font-size: 24px; width: 120px;").value("")))
            .child(button("SEND", false, "width: 150px;", "")),
        _ => root
            .child(form_row("DIVERSION AIRPORT", Node::new("input").style("font-size: 24px; width: 150px; text-transform: uppercase;").value("")))
            .child(form_row("REASON", Node::new("input").style("font-size: 24px; width: 400px;").value("")))
            .child(button("SEND", false, "width: 150px;", "")),
    }
}

pub fn aoc_page(m: &AocModel) -> Node {
    let cur = m.folder.as_str();
    let tree = Node::new("div")
        .child(folder(
            "INBOXES",
            &FolderOpts { expanded: m.inboxes_expanded, hide_icon: false, hide_opener: false, hidden: false },
            vec![aoc_entry("INBOX", cur), aoc_entry("OPS", cur), aoc_entry("METAR", cur)],
        ))
        .child(aoc_entry("SENT", cur))
        .child(aoc_entry("REQUESTS", cur))
        .child(aoc_entry("LOADSHEET", cur))
        .child(aoc_entry("REPORTS", cur))
        .child(aoc_entry("ADC DELAY", cur))
        .child(aoc_entry("DIVERSION", cur));

    let mut list = Node::new("div").style("width: 45%; border-right: 1px solid #5a5a5a; overflow-y: auto;");
    let mut viewer = Node::new("div").style("flex-grow: 1; padding: 10px;");
    if COMPOSE_FOLDERS.contains(&cur) {
        list = list.child(compose(cur));
    } else {
        list = list.child(
            div("fr ass")
                .child(div("oit-ccom-inbox-msg-table-header oit-table-ib f1").text("MSG"))
                .child(div("oit-ccom-inbox-msg-table-header oit-table-ib w15").text("ZULU"))
                .child(div("oit-ccom-inbox-msg-table-header oit-table-ib w5").text("NEW")),
        );
        for msg in &m.messages {
            list = list.child(
                div("fr ass")
                    .style("cursor: pointer;")
                    .child(div("oit-ccom-inbox-msg-table-line oit-green-text f1").text(&msg.title))
                    .child(div("oit-ccom-inbox-msg-table-line oit-green-text w15").text(&msg.zulu))
                    .child(div("oit-ccom-inbox-msg-table-line tc w5").text(if msg.is_new { "*" } else { "" })),
            );
        }
        if let Some(o) = &m.open {
            viewer = viewer
                .child(
                    div("fr ass")
                        .child(Node::el("div", "oit-label").text("FROM"))
                        .child(Node::el("div", "oit-label cyan").style("margin-left: 10px;").text(&o.from))
                        .child(Node::new("div").style("flex-grow: 1"))
                        .child(Node::el("div", "oit-label").text("RECVD"))
                        .child(Node::el("div", "oit-label cyan").style("margin-left: 10px;").text(&o.zulu)),
                )
                .child(
                    div("oit-ccom-inbox-msg-box")
                        .style("white-space: pre-wrap; font-family: monospace;")
                        .text(&o.lines.join("\n")),
                );
        }
    }

    Node::el("div", "oit-page-container").child(
        div("oit-avncs-navigator-container")
            .child(
                div("oit-avncs-navigator-left")
                    .child(div("oit-ccom-headline").text("COMPANY COM"))
                    .child(tree)
                    .child(Node::new("div").style("flex-grow: 1"))
                    .child(button("CLEAR ALL", false, "width: 100%; margin-top: 10px;", "")),
            )
            .child(
                div("oit-avncs-navigator-right")
                    .style("display: flex; flex-direction: row;")
                    .child(list)
                    .child(viewer),
            ),
    )
}

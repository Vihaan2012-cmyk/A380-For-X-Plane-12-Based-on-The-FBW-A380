use crate::perf_common::{fixed, js_round};
use crate::widgets::{button, button_sel, div};
use mfd_tree::Node;
use serde::Deserialize;

const MAX_ZFW_KG: f64 = 373_000.0;
const MAX_GW_KG: f64 = 512_000.0;
const MONTHS: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];

pub fn count_set_bits(value: f64) -> u32 {
    let mut v = js_round(value).max(0.0) as u64;
    let mut count = 0;
    while v > 0 {
        count += (v % 2) as u32;
        v /= 2;
    }
    count
}

fn pad2(n: f64) -> String {
    format!("{:0>2}", fixed(n, 0))
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct LoadsheetInput {
    pub zfw: f64,
    pub gw: f64,
    pub main_door_open: f64,
    pub mac_zfw: f64,
    pub mac_tow: f64,
    pub pax_stations: Vec<f64>,
    pub water_pct: f64,
    pub reg: String,
    pub airline: String,
    pub flight_number: String,
    pub zulu_seconds: f64,
    pub day: f64,
    pub month: f64,
    pub year: f64,
    pub origin: Option<String>,
    pub destination: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Loadsheet {
    pub title: String,
    pub lines: Vec<String>,
}

pub fn build_loadsheet(kind: &str, i: &LoadsheetInput) -> Option<Loadsheet> {
    if !(i.zfw > 0.0) || !(i.gw > 0.0) {
        return None;
    }
    if kind == "FINAL" && i.main_door_open > 0.0 {
        return None;
    }
    let fmt = |n: f64| fixed(js_round(n), 0);
    let tof = (i.gw - i.zfw).max(0.0);
    let underload = (MAX_ZFW_KG - i.zfw).min(MAX_GW_KG - i.gw);
    let pax: u32 = i.pax_stations.iter().map(|v| count_set_bits(*v)).sum();
    let callsign = format!("{}{}", i.airline.to_uppercase(), i.flight_number).trim().to_string();
    let hh = (i.zulu_seconds / 3600.0).floor() % 24.0;
    let mm = (i.zulu_seconds / 60.0).floor() % 60.0;
    let month = MONTHS[((i.month - 1.0).max(0.0).min(11.0)) as usize];
    let year = i.year % 100.0;
    let mut lines: Vec<String> = vec![];
    lines.push(format!("{callsign}/{} {}/{month}/{}", pad2(i.day), pad2(i.day), pad2(year)));
    let route: Vec<&str> = [i.origin.as_deref(), i.destination.as_deref(), Some(i.reg.as_str())]
        .into_iter()
        .flatten()
        .filter(|x| !x.is_empty())
        .collect();
    if !route.is_empty() {
        lines.push(route.join(" "));
    }
    lines.push(format!("ZFW {} MAX {}", fmt(i.zfw), fmt(MAX_ZFW_KG)));
    lines.push(format!("TOF {}", fmt(tof)));
    lines.push(format!("TOW {} MAX {}", fmt(i.gw), fmt(MAX_GW_KG)));
    lines.push(format!("MACZFW {}", fixed(i.mac_zfw, 1)));
    lines.push(format!("MACTOW {}", fixed(i.mac_tow, 1)));
    lines.push(format!("UNDLD {}", fmt(underload)));
    lines.push(format!("PAX TTL {pax}"));
    if i.water_pct > 0.0 {
        lines.push(format!("POTABLE WATER {}PCT", fixed(js_round(i.water_pct), 0)));
    }
    lines.push(format!("END/{callsign}"));
    Some(Loadsheet { title: format!("LOADSHEET {kind} {}{}", pad2(hh), pad2(mm)), lines })
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct LoadsheetModel {
    pub sheet: String,
    pub input: Option<LoadsheetInput>,
}

pub fn loadsheet_page(m: &LoadsheetModel) -> Node {
    let kind = if m.sheet == "FINAL" { "FINAL" } else { "PRELIM" };
    let result = m.input.as_ref().and_then(|i| build_loadsheet(kind, i));
    let content = match result {
        None => Node::new("div").style("overflow-y: auto; flex-grow: 1;").child(
            Node::el("div", "oit-label bigger")
                .text("NO LOADSHEET AVAILABLE")
                .child(Node::new("br"))
                .text(if kind == "FINAL" {
                    "MAIN DOOR MUST BE CLOSED FOR A FINAL LOADSHEET"
                } else {
                    "AIRCRAFT WEIGHT NOT YET COMPUTED"
                }),
        ),
        Some(r) => {
            let mut body = Node::new("div")
                .style("white-space: pre-wrap; font-family: monospace;")
                .child(Node::el("div", "oit-label bigger cyan").text(&r.title));
            for l in &r.lines {
                body = body.child(Node::el("div", "oit-label bigger").text(l));
            }
            Node::new("div").style("overflow-y: auto; flex-grow: 1;").child(body)
        }
    };
    Node::el("div", "oit-page-container framed").style("flex-direction: column;").child(
        div("fr ass")
            .style("margin-bottom: 15px;")
            .child(button_sel("PRELIM", false, kind == "PRELIM", "width: 200px; margin-right: 20px;", ""))
            .child(button_sel("FINAL", false, kind == "FINAL", "width: 200px; margin-right: 20px;", ""))
            .child(Node::new("div").style("flex-grow: 1"))
            .child(button("SEND TO AOC", false, "width: 220px;", "")),
    )
    .child(content)
}

pub const ATA: [(&str, &str); 21] = [
    ("21", "AIR CONDITIONING"),
    ("22", "AUTO FLIGHT"),
    ("23", "COMMUNICATIONS"),
    ("24", "ELECTRICAL POWER"),
    ("25", "EQUIPMENT/FURNISHINGS"),
    ("26", "FIRE PROTECTION"),
    ("27", "FLIGHT CONTROLS"),
    ("28", "FUEL"),
    ("29", "HYDRAULIC POWER"),
    ("30", "ICE AND RAIN PROTECTION"),
    ("31", "INDICATING/RECORDING"),
    ("32", "LANDING GEAR"),
    ("33", "LIGHTS"),
    ("34", "NAVIGATION"),
    ("35", "OXYGEN"),
    ("36", "PNEUMATIC"),
    ("38", "WATER/WASTE"),
    ("46", "INFORMATION SYSTEMS"),
    ("49", "AIRBORNE AUXILIARY POWER"),
    ("52", "DOORS"),
    ("70", "POWER PLANT"),
];

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DocEntry {
    pub name: String,
    pub folder: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MelItem {
    pub id: String,
    pub title: String,
    pub category: Option<String>,
    pub installed: Option<String>,
    pub required: Option<String>,
    pub placard: Option<String>,
    pub conditions: String,
    pub ops_procedure_required: bool,
    pub maintenance_procedure_required: bool,
    pub ops_procedure: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OpsView {
    Empty,
    Loading,
    Message { text: String, amber: bool },
    Markdown { text: String },
    Mel { items: Vec<MelItem> },
    MelEmpty,
}

impl Default for OpsView {
    fn default() -> Self {
        OpsView::Empty
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct OpsLibraryModel {
    pub mel_shelf: bool,
    pub docs_loading: bool,
    pub docs: Option<Vec<DocEntry>>,
    pub title: String,
    pub view: OpsView,
}

impl Default for OpsLibraryModel {
    fn default() -> Self {
        OpsLibraryModel { mel_shelf: false, docs_loading: false, docs: None, title: String::new(), view: OpsView::Empty }
    }
}

fn library_entry(label: &str, sub: &str) -> Node {
    Node::el("div", "oit-avncs-navigator-file")
        .style("cursor: pointer; padding: 8px 4px; border-bottom: 1px solid #333;")
        .child(Node::el("div", "oit-label bigger").text(label))
        .child(if sub.is_empty() { Node::new("div") } else { Node::el("div", "oit-label").style("color: #a0a0a0;").text(sub) })
}

fn strip_markup(line: &str) -> String {
    let mut s = line.replace("**", "").replace("__", "").replace('`', "");
    let trimmed = s.trim_start();
    if let Some(rest) = trimmed.strip_prefix(['-', '*']) {
        if rest.starts_with(char::is_whitespace) {
            let body = rest.trim_start();
            let boxed = body.strip_prefix('[').and_then(|b| {
                let mut ch = b.chars();
                let c = ch.next()?;
                if (c == ' ' || c == 'x' || c == 'X') && ch.next() == Some(']') {
                    Some(ch.as_str().trim_start().to_string())
                } else {
                    None
                }
            });
            s = match boxed {
                Some(b) => format!("□ {b}"),
                None => format!("• {body}"),
            };
        }
    }
    s
}

fn heading_of(line: &str) -> Option<(usize, &str)> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 3 {
        return None;
    }
    let rest = &line[hashes..];
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    Some((hashes, rest.trim_start()))
}

fn markdown_nodes(text: &str) -> Vec<Node> {
    text.split('\n')
        .map(|raw| {
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            if let Some((level, body)) = heading_of(line) {
                return Node::el("div", if level == 1 { "oit-label bigger cyan" } else { "oit-label bigger" })
                    .style("margin-top: 14px; font-weight: bold;")
                    .text(&body.to_uppercase());
            }
            Node::el("div", "oit-label").style("white-space: pre-wrap; min-height: 22px;").text(&strip_markup(line))
        })
        .collect()
}

fn mel_nodes(items: &[MelItem]) -> Vec<Node> {
    let dash = |v: &Option<String>| v.clone().unwrap_or_else(|| "-".into());
    let mut out = vec![];
    for it in items {
        out.push(Node::el("div", "oit-label bigger cyan").style("margin-top: 14px;").text(&format!("{}  {}", it.id, it.title.to_uppercase())));
        out.push(Node::el("div", "oit-label bigger").text(&format!(
            "CAT {}   INSTALLED {}   REQUIRED {}   {}   {} {}",
            dash(&it.category),
            dash(&it.installed),
            dash(&it.required),
            if it.placard.as_deref().map_or(false, |p| !p.is_empty()) { "PLACARD" } else { "" },
            if it.ops_procedure_required { "(O)" } else { "" },
            if it.maintenance_procedure_required { "(M)" } else { "" },
        )));
        out.push(Node::el("div", "oit-label").style("white-space: pre-wrap;").text(&it.conditions));
        out.push(match it.ops_procedure.as_deref().filter(|p| !p.is_empty()) {
            Some(p) => Node::el("div", "oit-label").style("white-space: pre-wrap; color: #a0a0a0;").text(p),
            None => Node::new("div"),
        });
    }
    out
}

pub fn ops_library_page(m: &OpsLibraryModel) -> Node {
    let mut list = Node::new("div").style("width: 420px; overflow-y: auto; border-right: 2px solid #555; padding-right: 10px;");
    if m.mel_shelf {
        let mut inner = Node::new("div");
        for (n, name) in ATA {
            inner = inner.child(library_entry(&format!("ATA {n}"), name));
        }
        list = list.child(inner);
    } else if m.docs_loading {
        list = list.child(Node::el("div", "oit-label bigger").text("LOADING..."));
    } else {
        match &m.docs {
            Some(files) if !files.is_empty() => {
                let mut inner = Node::new("div");
                for d in files {
                    let sub = format!(
                        "{}{}",
                        if d.folder.is_empty() { String::new() } else { format!("{} - ", d.folder.to_uppercase()) },
                        d.kind.to_uppercase()
                    );
                    inner = inner.child(library_entry(&d.name.to_uppercase(), &sub));
                }
                list = list.child(inner);
            }
            other => {
                list = list.child(Node::el("div", "oit-label bigger").text(if other.is_some() {
                    "NO DOCUMENTS. PUT .MD, .TXT, PDF OR PICTURE FILES IN THE AIRCRAFT DOCUMENTS FOLDER."
                } else {
                    "LIBRARY SERVER NOT AVAILABLE"
                }));
            }
        }
    }
    let mut view = Node::new("div").style("flex-grow: 1; overflow-y: auto; padding-left: 20px;");
    let loading = || Node::el("div", "oit-label bigger").text("LOADING...");
    match &m.view {
        OpsView::Empty => {}
        OpsView::Loading => view = view.child(Node::new("div").child(loading())),
        OpsView::Message { text, amber } => {
            view = view.child(Node::new("div").child(
                Node::el("div", if *amber { "oit-label bigger amber" } else { "oit-label bigger" }).text(text),
            ))
        }
        OpsView::Markdown { text } => view = view.child(Node::new("div").children(markdown_nodes(text))),
        OpsView::Mel { items } => view = view.child(Node::new("div").children(mel_nodes(items))),
        OpsView::MelEmpty => {
            view = view.child(
                Node::new("div")
                    .child(Node::el("div", "oit-label bigger").text("NO MEL ITEM IN THIS CHAPTER."))
                    .child(Node::el("div", "oit-label").text(
                        "THE OPERATOR MEL IS NOT SHIPPED. PARSE YOUR OWN COPY WITH TOOLS/PARSE_MEL.PY INTO OUTPUT/PREFERENCES/FBW_A380X_MEL.JSON.",
                    )),
            )
        }
    }
    Node::el("div", "oit-page-container framed").style("flex-direction: column;").child(
        div("fr ass")
            .style("margin-bottom: 15px; align-items: center;")
            .child(button_sel("DOCUMENTS", false, !m.mel_shelf, "width: 220px; margin-right: 20px;", ""))
            .child(button_sel("MEL", false, m.mel_shelf, "width: 220px; margin-right: 20px;", ""))
            .child(Node::el("div", "oit-label bigger cyan").style("flex-grow: 1; text-align: center;").text(&m.title))
            .child(button("PAGE UP", false, "width: 170px; margin-right: 10px;", ""))
            .child(button("PAGE DOWN", false, "width: 170px;", "")),
    )
    .child(div("fr").style("flex-grow: 1; min-height: 0;").child(list).child(view))
}

fn status_row(label: &str, status: &str, action: &str) -> Node {
    div("fr oit-perf-row")
        .style("align-items: center; margin-bottom: 18px;")
        .child(Node::el("div", "oit-label bigger").style("width: 330px;").text(label))
        .child(Node::el("div", "oit-label bigger oit-green-text").style("flex-grow: 1;").text(status))
        .child(button(action, false, "width: 200px;", ""))
}

fn detail_node(lines: &[String]) -> Node {
    let mut d = Node::new("div").style("flex-grow: 1; overflow-y: auto; margin-top: 10px;");
    if !lines.is_empty() {
        let mut inner = Node::new("div");
        for l in lines {
            inner = inner.child(Node::el("div", "oit-label bigger").style("white-space: pre-wrap;").text(l));
        }
        d = d.child(inner);
    }
    d
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct LoadBoxModel {
    pub ofp_status: String,
    pub docs_status: String,
    pub detail: Vec<String>,
}

impl Default for LoadBoxModel {
    fn default() -> Self {
        LoadBoxModel { ofp_status: "NOT LOADED".into(), docs_status: "NOT CHECKED".into(), detail: vec![] }
    }
}

pub fn load_box_page(m: &LoadBoxModel) -> Node {
    Node::el("div", "oit-page-container framed")
        .style("flex-direction: column;")
        .child(
            Node::el("div", "oit-labeled-box-container")
                .child(Node::el("span", "oit-labeled-box-label").text("LOADABLE DATA"))
                .child(status_row("SIMBRIEF OFP", &m.ofp_status, "LOAD"))
                .child(status_row("OPS LIBRARY DOCUMENTS", &m.docs_status, "CHECK"))
                .child(Node::el("div", "oit-label oit-perf-note").text(
                    "THE OFP FILLS FLT NUMBER, FROM AND TO ON THE STATUS PAGE AND IS FILED IN THE AOC OPS FOLDER. DOCUMENTS ARE READ FROM THE AIRCRAFT DOCUMENTS FOLDER.",
                )),
        )
        .child(detail_node(&m.detail))
}

pub const EXPORT_ITEMS: [&str; 5] = ["LOADSHEET PRELIM", "LOADSHEET FINAL", "AOC MESSAGES", "TECHNICAL LOG", "FLIGHT SUMMARY"];

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ExportBoxModel {
    pub statuses: Vec<String>,
}

pub fn export_box_page(m: &ExportBoxModel) -> Node {
    let mut b = Node::el("div", "oit-labeled-box-container").child(Node::el("span", "oit-labeled-box-label").text("EXPORTABLE DATA"));
    for (i, label) in EXPORT_ITEMS.iter().enumerate() {
        let status = m.statuses.get(i).map_or("READY", |s| s.as_str());
        b = b.child(status_row(label, status, "EXPORT"));
    }
    b = b.child(Node::el("div", "oit-label oit-perf-note").text(
        "EXPORTS ARE SAVED IN THE AIRCRAFT DOCUMENTS/EXPORTS FOLDER AND APPEAR ON THE EFB UNDER MY DOCUMENTS.",
    ));
    Node::el("div", "oit-page-container framed").style("flex-direction: column;").child(b)
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct OfpSummary {
    pub callsign: String,
    pub flight_number: String,
    pub origin: String,
    pub destination: String,
    pub alternate: Option<String>,
    pub route: String,
    pub cruise_altitude: f64,
    pub cost_index: String,
    pub distance: String,
    pub units: String,
    pub plan_ramp: String,
    pub plan_take_off: String,
    pub enroute_burn: String,
    pub plan_landing: String,
    pub est_zero_fuel_weight: String,
    pub est_take_off_weight: String,
    pub est_landing_weight: String,
    pub passenger_count: String,
}

pub fn ofp_lines(o: &OfpSummary) -> Vec<String> {
    let name = if o.callsign.is_empty() { &o.flight_number } else { &o.callsign };
    vec![
        format!("FLIGHT {name}  {}-{}  ALTN {}", o.origin, o.destination, o.alternate.as_deref().unwrap_or("----")),
        format!("ROUTE {}", o.route),
        format!("CRZ FL{}  CI {}  DIST {} NM", fixed(js_round(o.cruise_altitude / 100.0), 0), o.cost_index, o.distance),
        format!(
            "UNITS {}  RAMP {}  T/O {}  TRIP {}  LDG {}",
            o.units, o.plan_ramp, o.plan_take_off, o.enroute_burn, o.plan_landing
        ),
        format!(
            "EZFW {}  ETOW {}  ELW {}  PAX {}",
            o.est_zero_fuel_weight, o.est_take_off_weight, o.est_landing_weight, o.passenger_count
        ),
    ]
}

use crate::navigator::{folder, oit_file, FolderOpts};
use crate::perf_common::fixed;
use crate::shell::{avncs_sub_header, Avncs};
use crate::widgets::{button, button_sel, div, dropdown, input_field, radio_group, Dropdown, Field};
use mfd_tree::Node;
use serde::Deserialize;

const MONTHS: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];

struct Civil {
    year: i64,
    month: usize,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
}

fn civil(ms: f64) -> Civil {
    let total = (ms / 1000.0).floor() as i64;
    let days = total.div_euclid(86400);
    let rem = total.rem_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    Civil {
        year: if m <= 2 { y + 1 } else { y },
        month: (m - 1) as usize,
        day: d,
        hour: rem / 3600,
        minute: (rem % 3600) / 60,
        second: rem % 60,
    }
}

pub fn format_date_time(ms: Option<f64>) -> String {
    match ms {
        None => "------ --:--".into(),
        Some(t) => {
            let c = civil(t);
            format!("{:02}-{} {:02}:{:02}", c.day, MONTHS[c.month], c.hour, c.minute)
        }
    }
}

pub fn format_scheduled_date(ms: f64) -> String {
    let c = civil(ms);
    format!("{:02}-{}-{}", c.day, MONTHS[c.month], &format!("{}", c.year)[2..])
}

fn duration_text(ms: Option<f64>) -> String {
    match ms {
        None => "--:--:--".into(),
        Some(t) => {
            let c = civil(t);
            format!("{:02}:{:02}:{:02}", c.day * c.hour, c.minute, c.second)
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct InboxMessage {
    pub title: String,
    pub from: String,
    pub time_ms: Option<f64>,
    pub is_new: bool,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct InboxModel {
    pub flt_number: Option<String>,
    pub messages: Vec<InboxMessage>,
    pub selected: Option<usize>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct FlightLogModel {
    pub flt_number: Option<String>,
    pub origin: Option<String>,
    pub destination: Option<String>,
    pub sim_time_ms: Option<f64>,
    pub out_time: Option<f64>,
    pub off_time: Option<f64>,
    pub on_time: Option<f64>,
    pub in_time: Option<f64>,
    pub out_fob: Option<f64>,
    pub off_fob: Option<f64>,
    pub on_fob: Option<f64>,
    pub in_fob: Option<f64>,
    pub flight_time: Option<f64>,
    pub block_time: Option<f64>,
    pub metric: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CompanyComContent {
    None,
    Inbox(InboxModel),
    FlightLog(Box<FlightLogModel>),
}

impl Default for CompanyComContent {
    fn default() -> Self {
        CompanyComContent::None
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct CompanyComModel {
    pub uri: String,
    pub inbox_count: u32,
    pub outbox_count: u32,
    pub content: CompanyComContent,
}

impl Default for CompanyComModel {
    fn default() -> Self {
        CompanyComModel { uri: "nss-avncs/company-com".into(), inbox_count: 2, outbox_count: 0, content: CompanyComContent::None }
    }
}

fn nav_folder(name: &str, expanded: bool, hide_opener: bool, children: Vec<Node>) -> Node {
    folder(name, &FolderOpts { expanded, hide_icon: false, hide_opener, hidden: false }, children)
}

fn disabled_file(name: &str) -> Node {
    oit_file(name, false, true, true)
}

fn company_com_menu(m: &CompanyComModel) -> Vec<Node> {
    let f = |name: &str, target: &str| oit_file(name, m.uri == target, false, true);
    vec![
        f(&format!("Inbox ({})", m.inbox_count), "nss-avncs/company-com/inbox"),
        disabled_file(&format!("Outbox ({})", m.outbox_count)),
        disabled_file("Sent"),
        nav_folder(
            "Pre-flight",
            true,
            true,
            vec![
                disabled_file("INIT Data"),
                nav_folder("Requests", false, false, vec![disabled_file("Weather"), disabled_file("NOTAM")]),
                nav_folder(
                    "Reports",
                    false,
                    false,
                    vec![
                        disabled_file("Fuel Figures"),
                        disabled_file("Refuelling"),
                        disabled_file("Loadsheet Acceptance"),
                        disabled_file("Departure Delay"),
                        disabled_file("Take Off Delay"),
                        disabled_file("Delay Code"),
                        disabled_file("Free Text"),
                        disabled_file("ASR Notification"),
                    ],
                ),
                f("Flight Log", "nss-avncs/company-com/pre-flight/flight-log"),
            ],
        ),
        nav_folder(
            "In-flight",
            true,
            true,
            vec![
                nav_folder("Requests", false, false, vec![disabled_file("Weather"), disabled_file("Notam"), disabled_file("Ramp Service")]),
                nav_folder("Reports", false, false, vec![disabled_file("Diversion"), disabled_file("ETA"), disabled_file("Free Text")]),
                f("Flight Log", "nss-avncs/company-com/in-flight/flight-log"),
            ],
        ),
        nav_folder(
            "Post-flight",
            true,
            true,
            vec![
                nav_folder("Requests", false, false, vec![]),
                nav_folder(
                    "Reports",
                    false,
                    false,
                    vec![disabled_file("ASR Notification"), disabled_file("Flight Summary"), disabled_file("Free Text"), disabled_file("Gate Delay")],
                ),
                f("Flight Log", "nss-avncs/company-com/post-flight/flight-log"),
            ],
        ),
    ]
}

fn headline(t: &str) -> Node {
    div("oit-ccom-headline").text(t)
}

fn grow() -> Node {
    Node::new("div").style("flex-grow: 1")
}

fn inbox_nodes(m: &InboxModel) -> Vec<Node> {
    let selected = m.selected.filter(|i| *i < m.messages.len()).or(if m.messages.is_empty() { None } else { Some(0) });
    let mut table = div("oit-ccom-inbox-msg-table").child(
        div("fr ass")
            .child(div("oit-ccom-inbox-msg-table-header oit-table-ib w5"))
            .child(div("oit-ccom-inbox-msg-table-header oit-table-ib f1").text("Subject"))
            .child(div("oit-ccom-inbox-msg-table-header oit-table-ib w15").text("Date")),
    );
    let mut rows = Node::new("div");
    for (i, msg) in m.messages.iter().enumerate() {
        let sel = selected == Some(i);
        let cell = |cls: &str, green: bool, t: &str| {
            Node::el("div", "oit-ccom-inbox-msg-table-line")
                .class_if("oit-green-text", green)
                .class_if("selected", sel)
                .class_if("tc", cls.contains("tc"))
                .class_if("w5", cls.contains("w5"))
                .class_if("f1", cls.contains("f1"))
                .class_if("w15", cls.contains("w15"))
                .text(t)
        };
        rows = rows.child(
            div("fr ass")
                .child(cell("tc w5", true, if msg.is_new { "\u{2709}" } else { "" }))
                .child(cell("f1", true, &format!("{}  ({})", msg.title, msg.from)))
                .child(cell("w15", true, &format_date_time(msg.time_ms))),
        );
    }
    table = table.child(rows).child(grow());
    let (read, subject, date, text) = match selected {
        None => (String::new(), "Subject :".to_string(), String::new(), "NO MESSAGE".to_string()),
        Some(i) => {
            let msg = &m.messages[i];
            ("Message is read".to_string(), format!("Subject : {}", msg.title), format_date_time(msg.time_ms), msg.lines.join("\n"))
        }
    };
    let to = match &m.flt_number {
        Some(n) if !n.is_empty() => format!("To : {n}"),
        _ => "To : <unknown>".to_string(),
    };
    vec![
        headline("Inbox"),
        table,
        div("oit-ccom-inbox-msg-details")
            .child(Node::new("div").text(&read))
            .child(Node::new("div"))
            .child(Node::new("div").style("text-align: right;").text(&date))
            .child(Node::new("div").text(&to))
            .child(Node::new("div").text(&subject))
            .child(Node::new("div")),
        div("oit-ccom-inbox-msg-box").style("white-space: pre-wrap; font-family: monospace;").text(&text),
        grow(),
    ]
}

fn flight_log_nodes(m: &FlightLogModel) -> Vec<Node> {
    let fob = |w: Option<f64>| match w {
        None => "----- -".to_string(),
        Some(kg) => {
            let v = if m.metric { kg } else { kg * 2.20462262 };
            format!("{:\u{a0}>5} {}", fixed(v / 1000.0, 1), if m.metric { "T" } else { "KLB" })
        }
    };
    let state = if m.out_time.is_none() {
        "INITIAL"
    } else if m.off_time.is_none() {
        "PRE-FLIGHT"
    } else if m.on_time.is_none() {
        "IN-FLIGHT"
    } else if m.in_time.is_none() {
        "POST-FLIGHT"
    } else {
        "IN"
    };
    let nz = |v: &Option<String>, dflt: &str| match v {
        Some(s) if !s.is_empty() => s.clone(),
        _ => dflt.to_string(),
    };
    let sched = m.sim_time_ms.map_or("----".to_string(), format_scheduled_date);
    let oooi = |label: &str, t: Option<f64>, f: Option<f64>| -> Vec<Node> {
        vec![
            div("oit-table-ib").text(label),
            div("oit-green-text oit-table-ib").text(&format_date_time(t)),
            div("oit-green-text oit-table-ib").text(&fob(f)),
        ]
    };
    let mut table = div("oit-ccom-flight-log-oooi-table")
        .child(div("oit-ccom-flight-log-oooi-table-header oit-table-ib").text("State"))
        .child(div("oit-ccom-flight-log-oooi-table-header oit-table-ib").text("Time (UTC)"))
        .child(div("oit-ccom-flight-log-oooi-table-header oit-table-ib").text("FOB"));
    for (l, t, f) in [("OUT", m.out_time, m.out_fob), ("OFF", m.off_time, m.off_fob), ("ON", m.on_time, m.on_fob), ("IN", m.in_time, m.in_fob)] {
        table = table.children(oooi(l, t, f));
    }
    vec![
        headline("Flight Log"),
        div("oit-labeled-box-container").child(Node::el("span", "oit-labeled-box-label").text("Flight")).child(
            Node::new("div")
                .style("display: flex; flex-direction: row; justify-content: flex-start; break-before: always;")
                .child(radio_group("oit-flight-log-flight", &["Current FLT"])),
        ),
        div("oit-labeled-box-container").child(
            div("oit-ccom-flight-log-grid")
                .child(div("oit-align-right-sp25").text("From"))
                .child(Node::new("div").text(&nz(&m.origin, "----")))
                .child(div("oit-align-right-sp25").text("To"))
                .child(Node::new("div").text(&nz(&m.destination, "----")))
                .child(div("oit-align-right-sp25 mt20").text("Flight Identifier"))
                .child(div("oit-green-text mt20").text(&nz(&m.flt_number, "----------")))
                .child(div("oit-align-right-sp25 mt20").text("Scheduled Date"))
                .child(div("oit-green-text mt20").text(&sched)),
        ),
        div("oit-ccom-flight-log-bottom-structure")
            .child(div("oit-ccom-flight-log-bottom-structure-left").child(table))
            .child(
                div("oit-ccom-flight-log-bottom-structure-right")
                    .child(
                        div("oit-ccom-flight-log-bottom-structure-right1")
                            .child(div("mr15").text("Current State"))
                            .child(div("oit-green-text").text(state)),
                    )
                    .child(
                        div("oit-ccom-flight-log-bottom-structure-right2")
                            .child(div("mr15").text("Flight Time"))
                            .child(Node::new("div").text("Block Time"))
                            .child(div("oit-green-text mr15 tc").text(&duration_text(m.flight_time)))
                            .child(div("oit-green-text tc").text(&duration_text(m.block_time))),
                    ),
            ),
        grow(),
    ]
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MaintEvent {
    pub kind: u32,
    pub id: i64,
    pub zulu_s: f64,
    pub phase: String,
    pub ata: u32,
    pub message: String,
}

pub fn kind_label(kind: u32) -> &'static str {
    match kind {
        1 => "FAULT",
        2 => "CLEARED",
        3 => "SYS FAULT",
        4 => "SYS CLEARED",
        5 => "C/B TRIP",
        6 => "C/B RESET",
        7 => "MEL DEFER",
        8 => "MEL RELEASE",
        _ => "",
    }
}

pub fn is_onset(kind: u32) -> bool {
    matches!(kind, 1 | 3 | 5 | 7)
}

fn fault_key(e: &MaintEvent) -> String {
    match e.kind {
        1 | 2 => format!("F{}", e.id),
        3 | 4 => format!("S{}", e.id),
        5 | 6 => format!("B{}", e.id),
        _ => format!("M{}", e.id),
    }
}

pub fn format_zulu(zulu_s: f64) -> String {
    let s = (zulu_s.max(0.0).floor() as i64) % 86400;
    format!("{:02}{:02}", s / 3600, (s % 3600) / 60)
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ReportModel {
    pub post: bool,
    pub ata: Option<u32>,
    pub ata_name: String,
    pub events: Vec<MaintEvent>,
}

pub fn report_rows(m: &ReportModel) -> Vec<&MaintEvent> {
    let mut rows: Vec<&MaintEvent> = vec![];
    if m.post {
        rows.extend(m.events.iter());
    } else {
        let mut seen: Vec<String> = vec![];
        for e in &m.events {
            let key = fault_key(e);
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            if is_onset(e.kind) {
                rows.push(e);
            }
        }
    }
    if let Some(a) = m.ata {
        rows.retain(|e| e.ata == a);
    }
    rows
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ChapterRow {
    pub name: String,
    pub count: u32,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DeferredRow {
    pub label: String,
    pub covers: Vec<String>,
    pub hours: f64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Breaker {
    pub id: String,
    pub name: String,
    pub label: String,
    pub bus: String,
    pub ata: u32,
    pub rating_a: f64,
    pub state: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct RemoteCbModel {
    pub ctl_on: bool,
    pub show_all: bool,
    pub search: Option<String>,
    pub selected: Option<String>,
    pub page: usize,
    pub breakers: Vec<Breaker>,
}

impl Default for RemoteCbModel {
    fn default() -> Self {
        RemoteCbModel { ctl_on: false, show_all: false, search: None, selected: None, page: 0, breakers: vec![] }
    }
}

const ROWS_PER_PAGE: usize = 12;

fn cb_matches(b: &Breaker, query: &Option<String>, show_all: bool) -> bool {
    match query {
        None => show_all || b.state != 0,
        Some(q) => {
            let hay = format!("{} {} {} ATA {} {}", b.name, b.label, b.bus, b.ata, b.ata).to_uppercase();
            q.split_whitespace().all(|w| hay.contains(w))
        }
    }
}

fn cb_state_label(state: u32) -> String {
    match state {
        0 => "CLOSED".into(),
        1 => "OPEN".into(),
        2 => "TRIPPED THERMAL".into(),
        3 => "TRIPPED MAGNETIC".into(),
        4 => "TRIPPED ARC FAULT".into(),
        5 => "LOCKED OUT".into(),
        n => n.to_string(),
    }
}

fn cb_state_class(state: u32) -> &'static str {
    match state {
        0 => "oit-green-text",
        1 => "oit-cyan-text",
        2..=5 => "oit-amber-text",
        _ => "",
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MaintContent {
    None,
    Report(ReportModel),
    SystemStatus { chapters: Vec<ChapterRow> },
    Logbook { rows: Vec<DeferredRow> },
    RemoteCb(Box<RemoteCbModel>),
}

impl Default for MaintContent {
    fn default() -> Self {
        MaintContent::None
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct MaintenanceModel {
    pub uri: String,
    pub content: MaintContent,
}

impl Default for MaintenanceModel {
    fn default() -> Self {
        MaintenanceModel { uri: "nss-avncs/maintenance".into(), content: MaintContent::None }
    }
}

fn maintenance_menu(m: &MaintenanceModel) -> Vec<Node> {
    let f = |name: &str, target: &str| oit_file(name, m.uri == target, false, true);
    vec![
        nav_folder(
            "Reports",
            true,
            true,
            vec![
                f("Current Flight Report", "nss-avncs/maintenance/current-flight-report"),
                f("Post Flight Report", "nss-avncs/maintenance/post-flight-report"),
            ],
        ),
        nav_folder("System Status", true, true, vec![f("By ATA Chapter", "nss-avncs/maintenance/system-status")]),
        nav_folder("Circuit Breakers", true, true, vec![f("Remote C/B Control", "nss-avncs/maintenance/remote-cb")]),
        nav_folder("Logbook", true, true, vec![f("MEL Deferred Items", "nss-avncs/maintenance/logbook")]),
    ]
}

fn table_header(cells: &[(&str, &str)]) -> Node {
    let mut h = div("fr ass");
    for (cls, t) in cells {
        h = h.child(div(&format!("oit-ccom-inbox-msg-table-header oit-table-ib {cls}")).text(t));
    }
    h
}

fn empty_row(text: &str) -> Node {
    div("oit-maint-empty").text(text)
}

fn line_cell(cls: &str, color: &str, t: &str) -> Node {
    div(&format!("oit-ccom-inbox-msg-table-line {cls} {color}")).text(t)
}

fn maint_nodes(m: &MaintenanceModel, content: &MaintContent) -> Vec<Node> {
    match content {
        MaintContent::None => vec![],
        MaintContent::Report(r) => {
            let base = if r.post { "Post Flight Report" } else { "Current Flight Report" };
            let title = if r.ata.is_some() { format!("{base} - ATA {}", r.ata_name) } else { base.to_string() };
            let rows = report_rows(r);
            let mut body = div("oit-maint-table-body");
            if rows.is_empty() {
                body = body.child(empty_row(if r.post { "NO EVENT RECORDED" } else { "NO FAULT" }));
            }
            for e in rows {
                let color = if is_onset(e.kind) { "oit-amber-text" } else { "oit-green-text" };
                let ata = if e.ata == 0 { "--".to_string() } else { e.ata.to_string() };
                body = body.child(
                    div("oit-maint-row")
                        .child(line_cell("w8 tc", color, &format_zulu(e.zulu_s)))
                        .child(line_cell("w8 tc", color, &e.phase))
                        .child(line_cell("w8 tc", color, &ata))
                        .child(line_cell("f1", color, &e.message))
                        .child(line_cell("w15 tc", color, kind_label(e.kind))),
                );
            }
            vec![
                headline(&title),
                div("oit-ccom-inbox-msg-table")
                    .child(table_header(&[("w8", "UTC"), ("w8", "PH"), ("w8", "ATA"), ("f1", "Message"), ("w15", "Type")]))
                    .child(body),
                grow(),
            ]
        }
        MaintContent::SystemStatus { chapters } => {
            let mut body = div("oit-maint-table-body");
            if chapters.is_empty() {
                body = body.child(empty_row("ALL SYSTEMS NORMAL"));
            }
            for c in chapters {
                body = body.child(
                    div("oit-maint-row")
                        .child(line_cell("f1", "oit-amber-text", &c.name.to_uppercase()))
                        .child(line_cell("w15 tc", "oit-amber-text", &c.count.to_string())),
                );
            }
            vec![
                headline("System Status"),
                div("oit-ccom-inbox-msg-table").child(table_header(&[("f1", "ATA Chapter"), ("w15", "Active Faults")])).child(body),
                grow(),
            ]
        }
        MaintContent::Logbook { rows } => {
            let mut body = div("oit-maint-table-body");
            if rows.is_empty() {
                body = body.child(empty_row("NO DEFERRED ITEM"));
            }
            for r in rows {
                body = body.child(
                    div("oit-maint-row")
                        .child(line_cell("w15", "oit-amber-text", &r.label))
                        .child(line_cell("f1", "oit-amber-text", &r.covers.join(", ").to_uppercase()))
                        .child(line_cell(
                            "w12 tc",
                            "oit-amber-text",
                            &if r.hours > 0.0 { format!("{} H", fixed(r.hours, 1)) } else { "EXPIRED".to_string() },
                        )),
                );
            }
            vec![
                headline("Logbook - MEL Deferred Items"),
                div("oit-ccom-inbox-msg-table")
                    .child(table_header(&[("w15", "MEL Ref"), ("f1", "Placarded"), ("w12", "Remaining")]))
                    .child(body),
                grow(),
            ]
        }
        MaintContent::RemoteCb(r) => remote_cb_nodes(r, m),
    }
}

fn remote_cb_nodes(r: &RemoteCbModel, _m: &MaintenanceModel) -> Vec<Node> {
    let matching: Vec<&Breaker> = r.breakers.iter().filter(|b| cb_matches(b, &r.search, r.show_all)).collect();
    let pages = ((matching.len() as f64 / ROWS_PER_PAGE as f64).ceil() as usize).max(1);
    let page = r.page.min(pages - 1);
    let first = page * ROWS_PER_PAGE;
    let shown: Vec<&&Breaker> = matching.iter().skip(first).take(ROWS_PER_PAGE).collect();
    let selected = r.selected.as_ref().and_then(|id| r.breakers.iter().find(|b| &b.id == id));
    let mut body = div("oit-maint-table-body");
    if shown.is_empty() {
        let text = if r.breakers.is_empty() {
            "C/B DATA NOT AVAILABLE".to_string()
        } else if let Some(q) = &r.search {
            format!("NO C/B MATCHES {q}")
        } else {
            "ALL C/B CLOSED".to_string()
        };
        body = body.child(empty_row(&text));
    }
    for b in shown {
        let cls = cb_state_class(b.state);
        let cell = |c: &str, t: &str| div(&format!("oit-cb-cell {c} {cls}")).text(t);
        body = body.child(
            div("oit-cb-row")
                .class_if("selected", r.selected.as_deref() == Some(b.id.as_str()))
                .child(cell("oit-cb-name", &b.name.to_uppercase()))
                .child(cell("oit-cb-ata", &b.ata.to_string()))
                .child(cell("oit-cb-bus", &b.bus))
                .child(cell("oit-cb-rating", &format!("{} A", fixed(b.rating_a, 0))))
                .child(cell("oit-cb-state", &cb_state_label(b.state))),
        );
    }
    let search_field = Field {
        text: r.search.clone().unwrap_or_else(|| "--------------------".into()),
        container_style: "width: 360px; margin-right: 10px;".into(),
        align: "flex-start".into(),
        max_digits: 20,
        ..Default::default()
    };
    let commands_disabled = !r.ctl_on || selected.is_none();
    vec![
        headline("Remote C/B Control"),
        div("oit-maint-ctl-line").text(if r.ctl_on {
            "REMOTE C/B CTL ON"
        } else {
            "REMOTE C/B CTL OFF - SELECT ELEC REMOTE C/B CTL ON (OVHD MAINT PANEL)"
        }),
        div("fr oit-cb-toolbar")
            .child(div("oit-cb-search-label").text("SEARCH"))
            .child(input_field(&search_field))
            .child(button("CLEAR", r.search.is_none(), "", ""))
            .child(Node::new("div").style("width: 20px"))
            .child(button_sel("ABNORMAL", false, !r.show_all, "", ""))
            .child(button_sel("ALL", false, r.show_all, "", ""))
            .child(grow())
            .child(button("PREV", page == 0, "", ""))
            .child(div("oit-maint-pager").text(&format!("{} C/B  PAGE {}/{}", matching.len(), page + 1, pages)))
            .child(button("NEXT", page + 1 >= pages, "", "")),
        div("oit-cb-table")
            .child(
                div("oit-cb-row oit-cb-header")
                    .child(div("oit-cb-cell oit-cb-name").text("C/B"))
                    .child(div("oit-cb-cell oit-cb-ata").text("ATA"))
                    .child(div("oit-cb-cell oit-cb-bus").text("BUS"))
                    .child(div("oit-cb-cell oit-cb-rating").text("RATING"))
                    .child(div("oit-cb-cell oit-cb-state").text("STATE")),
            )
            .child(body),
        div("fr")
            .style("margin-top: 8px; align-items: center;")
            .child(div("oit-maint-selected").text(&selected.map_or("NO C/B SELECTED".to_string(), |b| b.name.to_uppercase())))
            .child(button("OPEN", commands_disabled, "", ""))
            .child(button("CLOSE / RESET", commands_disabled, "", "")),
        grow(),
    ]
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DebugRow {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DebugModel {
    pub transmitting: bool,
    pub rows: Vec<DebugRow>,
}

fn fws_debug_nodes(m: &DebugModel, who: &str) -> Vec<Node> {
    let dd = Dropdown {
        values: vec!["OFF".into(), "ON".into()],
        text: if m.transmitting { "ON".into() } else { "OFF".into() },
        container_style: "width: 175px; margin-right: 15px;".into(),
        digits: 5,
        ..Default::default()
    };
    let mut rows = Node::new("div");
    for r in &m.rows {
        rows = rows.child(
            div("fr ass")
                .child(div("oit-ccom-inbox-msg-table-line tc f1").text(&r.label))
                .child(div("oit-ccom-inbox-msg-table-line oit-green-text f1").text(&r.value)),
        );
    }
    vec![
        headline("Flight Warning System Debug"),
        div("oit-ccom-headline").child(grow()).child(
            div("fr aic")
                .child(Node::new("span").style("margin-right: 15px;").text("Transmission Status"))
                .child(dropdown(&format!("{who}_OIT_a380x_ois_fws_debug_data_enabled_control"), &dd)),
        ),
        div("oit-ccom-inbox-msg-table").child(table_header(&[("f1", "Label"), ("f1", "Value")])).child(rows),
        grow(),
    ]
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct AppLdgInputs {
    pub ap1: bool,
    pub ap2: bool,
    pub athr_status: f64,
    pub athr_mode: f64,
    pub prim: [bool; 3],
    pub sec: [bool; 3],
    pub sfcc_words: [f64; 2],
    pub rudder_words: [f64; 2],
    pub hyd_green: bool,
    pub hyd_yellow: bool,
    pub fws_dw126: [f64; 2],
    pub fcdc_healthy: [bool; 2],
    pub antiskid: bool,
    pub fcdc_dw4: [f64; 2],
    pub fcdc_dw8: [f64; 2],
    pub adr_aoa: [f64; 3],
    pub engines: [bool; 4],
    pub apu_contactors: [bool; 2],
}

#[derive(Clone, Copy, Debug)]
pub struct Arinc {
    pub ssm: u32,
    pub value: f64,
}

pub fn arinc(raw: f64) -> Arinc {
    let bits = ((raw as i64) & 0xffff_ffff) as u32;
    Arinc { ssm: ((raw / 4294967296.0).trunc() as i64 & 3) as u32, value: f32::from_bits(bits) as f64 }
}

impl Arinc {
    pub fn failure_warning(&self) -> bool {
        self.ssm == 0
    }

    pub fn normal(&self) -> bool {
        self.ssm == 3
    }

    pub fn bit_or(&self, bit: u32, default: bool) -> bool {
        if self.ssm == 3 || self.ssm == 2 {
            ((self.value.trunc() as i64 as i32) >> (bit - 1)) & 1 != 0
        } else {
            default
        }
    }
}

#[derive(Clone, Debug)]
pub struct AppLdgState {
    pub capability: &'static str,
    pub one_ap: bool,
    pub two_ap: bool,
    pub athr: bool,
    pub one_prim: bool,
    pub two_prim: bool,
    pub one_sec: bool,
    pub one_sfcc: bool,
    pub two_sfcc: bool,
    pub rudder_trim: bool,
    pub one_hyd: bool,
    pub two_hyd: bool,
    pub one_fws: bool,
    pub two_fws: bool,
    pub one_fcdc: bool,
    pub two_fcdc: bool,
    pub antiskid: bool,
    pub nws: bool,
    pub two_adr: bool,
    pub three_adr: bool,
    pub one_eng_each_side: bool,
    pub three_eng: bool,
    pub four_eng: bool,
}

pub fn app_ldg_state(i: &AppLdgInputs) -> AppLdgState {
    let sfcc = [arinc(i.sfcc_words[0]), arinc(i.sfcc_words[1])];
    let rud = [arinc(i.rudder_words[0]), arinc(i.rudder_words[1])];
    let fws = [arinc(i.fws_dw126[0]), arinc(i.fws_dw126[1])];
    let dw4 = [arinc(i.fcdc_dw4[0]), arinc(i.fcdc_dw4[1])];
    let dw8 = [arinc(i.fcdc_dw8[0]), arinc(i.fcdc_dw8[1])];
    let adr = [arinc(i.adr_aoa[0]), arinc(i.adr_aoa[1]), arinc(i.adr_aoa[2])];
    let sfcc_ok = !sfcc[0].failure_warning() && !sfcc[1].failure_warning();
    let n_adr = adr.iter().filter(|a| !a.failure_warning()).count();
    let n_eng = i.engines.iter().filter(|e| **e).count();
    let cap = |dw: &Arinc| {
        if dw.bit_or(23, false) {
            3
        } else if dw.bit_or(24, false) {
            4
        } else if dw.bit_or(25, false) {
            5
        } else {
            0
        }
    };
    let capability = match if dw4[0].normal() { cap(&dw4[0]) } else { cap(&dw4[1]) } {
        5 => "LAND3 DUAL",
        4 => "LAND3 SINGLE",
        3 => "LAND2",
        _ => "APPR1",
    };
    AppLdgState {
        capability,
        one_ap: i.ap1 || i.ap2,
        two_ap: i.ap1 && i.ap2,
        athr: i.athr_status == 2.0 || i.athr_mode != 0.0,
        one_prim: i.prim.iter().any(|x| *x),
        two_prim: (i.prim[0] && i.prim[1]) || (i.prim[0] && i.prim[2]),
        one_sec: i.sec.iter().any(|x| *x),
        one_sfcc: sfcc_ok,
        two_sfcc: sfcc_ok,
        rudder_trim: rud[0].bit_or(27, false) || rud[1].bit_or(27, false),
        one_hyd: i.hyd_green || i.hyd_yellow,
        two_hyd: i.hyd_green && i.hyd_yellow,
        one_fws: fws[0].bit_or(16, false) || fws[1].bit_or(16, false),
        two_fws: fws[0].bit_or(16, false) && fws[1].bit_or(16, false),
        one_fcdc: i.fcdc_healthy[0] || i.fcdc_healthy[1],
        two_fcdc: i.fcdc_healthy[0] && i.fcdc_healthy[1],
        antiskid: i.antiskid,
        nws: !dw8[0].bit_or(18, false) && !dw8[1].bit_or(18, false),
        two_adr: n_adr >= 2,
        three_adr: n_adr == 3,
        one_eng_each_side: (i.engines[0] || i.engines[1]) && (i.engines[2] || i.engines[3]),
        three_eng: n_eng >= 3,
        four_eng: n_eng == 4 || (n_eng == 3 && (i.apu_contactors[0] || i.apu_contactors[1])),
    }
}

fn app_ldg_nodes(i: &AppLdgInputs) -> Vec<Node> {
    let s = app_ldg_state(i);
    let td = |t: &str, green: bool| Node::el("div", "oit-a380x-systems-app-ldg-cap-td").class_if("green", green).text(t);
    let label = |t: &str| div("oit-a380x-systems-app-ldg-cap-td transparentbg").text(t);
    let head = |t: &str, style: &str, br: Option<&str>| {
        let mut n = div("oit-a380x-systems-app-ldg-cap-td transparentbg").style(style);
        n = n.text(t);
        if let Some(second) = br {
            n = n.child(Node::new("br")).text(second);
        }
        n
    };
    let mut table = div("oit-a380x-systems-app-ldg-cap-table")
        .child(head("Monitored", "font-weight: bold; width: 200px; height: 75px;", Some("Equipment")))
        .child(head("LAND2", "font-weight: bold; height: 75px; text-align: center;", None))
        .child(head("LAND3", "font-weight: bold; height: 75px; text-align: center;", Some("SGL")))
        .child(head("LAND3", "font-weight: bold; height: 75px; text-align: center;", Some("DUAL")));
    let rows: Vec<(&str, [(&str, bool); 3])> = vec![
        ("AP engaged", [("1", s.one_ap), ("1", s.one_ap), ("2", s.two_ap)]),
        ("A/THR", [("Not reqrd.", true), ("Active", s.athr), ("Active", s.athr)]),
        ("PRIM", [("1", s.one_prim), ("1", s.one_prim), ("2", s.two_prim)]),
        ("SEC", [("1", s.one_sec), ("1", s.one_sec), ("1", s.one_sec)]),
        ("Slats/Flaps Ctl", [("1", s.one_sfcc), ("1", s.one_sfcc), ("2", s.two_sfcc)]),
        ("Rud. Trim", [("Avail.", s.rudder_trim), ("Avail.", s.rudder_trim), ("Avail.", s.rudder_trim)]),
        ("Hydraulics", [("G/Y", s.one_hyd), ("G/Y", s.one_hyd), ("G+Y", s.two_hyd)]),
        ("FWS w/ audio", [("1", s.one_fws), ("1", s.one_fws), ("2", s.two_fws)]),
        ("FCDC", [("1", s.one_fcdc), ("1", s.one_fcdc), ("2", s.two_fcdc)]),
        ("Antiskid", [("Not reqrd.", true), ("Not reqrd.", true), ("Avail.", s.antiskid)]),
        ("NWS", [("Not reqrd.", true), ("Not reqrd.", true), ("Avail.", s.nws)]),
        ("ADR", [("2", s.two_adr), ("2", s.two_adr), ("3", s.three_adr)]),
        ("Engines", [("2", s.one_eng_each_side), ("3", s.three_eng), ("4", s.four_eng)]),
    ];
    for (name, cells) in rows {
        table = table.child(label(name));
        for (t, g) in cells {
            table = table.child(td(t, g));
        }
    }
    vec![
        headline("Approach & Landing Capability"),
        div("fr ass")
            .child(
                div("fc aic").child(
                    div("oit-a380x-systems-app-ldg-cap")
                        .child(Node::new("span").style("text-align: center; font-size: 22px;").text("Capability from FCDC:"))
                        .child(
                            Node::new("span")
                                .style("text-align: center; font-size: 36px; font-weight: bold; margin-top: 5px;")
                                .text(s.capability),
                        ),
                ),
            )
            .child(div("fc aic").style("flex-grow: 1; ").child(table)),
        grow(),
    ]
}

fn fbw_systems_menu(uri: &str) -> Vec<Node> {
    vec![
        nav_folder("AUTO FLT", true, false, vec![oit_file("App & Ldg Capability", uri == "nss-avncs/a380x-systems/app-ldg-cap", false, true)]),
        nav_folder("CDS / FWS", true, false, vec![oit_file("FWS Debug", uri == "nss-avncs/a380x-systems/debug-data", false, true)]),
    ]
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FbwContent {
    None,
    AppLdgCap(Box<AppLdgInputs>),
    FwsDebug(DebugModel),
}

impl Default for FbwContent {
    fn default() -> Self {
        FbwContent::None
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct FbwSystemsModel {
    pub uri: String,
    pub content: FbwContent,
}

impl Default for FbwSystemsModel {
    fn default() -> Self {
        FbwSystemsModel { uri: "nss-avncs/a380x-systems".into(), content: FbwContent::None }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AvncsPage {
    Menu,
    CompanyCom(CompanyComModel),
    Maintenance(MaintenanceModel),
    FbwSystems(FbwSystemsModel),
}

impl Default for AvncsPage {
    fn default() -> Self {
        AvncsPage::Menu
    }
}

fn section(title: &str, m: &Avncs, menu: Vec<Node>, content: Vec<Node>) -> Vec<Node> {
    vec![
        avncs_sub_header(title, m),
        Node::el("div", "oit-page-container").child(
            Node::el("div", "oit-avncs-navigator-container")
                .child(Node::el("div", "oit-avncs-navigator-left").children(menu))
                .child(Node::el("div", "oit-avncs-navigator-right oit-centered").children(content)),
        ),
    ]
}

pub fn render_section(page: &AvncsPage, m: &Avncs, who: &str) -> Option<Vec<Node>> {
    match page {
        AvncsPage::Menu => None,
        AvncsPage::CompanyCom(c) => {
            let content = match &c.content {
                CompanyComContent::None => vec![],
                CompanyComContent::Inbox(i) => inbox_nodes(i),
                CompanyComContent::FlightLog(f) => flight_log_nodes(f),
            };
            Some(section("COMPANY COM", m, company_com_menu(c), content))
        }
        AvncsPage::Maintenance(mm) => Some(section("MAINTENANCE", m, maintenance_menu(mm), maint_nodes(mm, &mm.content))),
        AvncsPage::FbwSystems(f) => {
            let content = match &f.content {
                FbwContent::None => vec![],
                FbwContent::AppLdgCap(i) => app_ldg_nodes(i),
                FbwContent::FwsDebug(d) => fws_debug_nodes(d, who),
            };
            Some(section("A380X SYSTEMS", m, fbw_systems_menu(&f.uri), content))
        }
    }
}

use crate::js::{el, js_fixed, num, round, select};
use crate::{button, label, span, Node};
use serde::Deserialize;
use serde_json::Value;

fn panel(id: &str, active: bool) -> Node {
    el("div", "tab-panel").class_if("active", active).id(&format!("tab-{id}"))
}

fn checkbox(id: Option<&str>, checked: bool, act: &str) -> Node {
    let n = Node::new("input").attr("type", "checkbox").attr_if("checked", checked).act(act);
    match id {
        Some(i) => n.id(i),
        None => n,
    }
}

fn text_input(id: &str, placeholder: &str, value: &str) -> Node {
    Node::new("input").attr("type", "text").id(id).attr("placeholder", placeholder).value(value).act(&format!("focus:{id}"))
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SystemsStatus {
    pub variables: f64,
    #[serde(rename = "lastTickMicros")]
    pub last_tick_micros: f64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DisplaysStatus {
    pub screens: Value,
    pub fps: Value,
}

/// The app's `/api/status`; `known` is false until the first answer.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct AppStatus {
    pub known: bool,
    pub xplane: bool,
    pub systems: Option<SystemsStatus>,
    pub displays: Option<DisplaysStatus>,
    pub update: String,
}

impl AppStatus {
    pub fn link_ok(&self) -> Option<bool> {
        self.known.then(|| self.xplane && self.systems.is_some())
    }
}

/// The Status tab's update line from the installed and latest release versions.
pub fn update_text(checking: bool, installed: Option<&str>, latest: Option<&str>) -> String {
    if checking {
        return "Checking GitHub for updates...".into();
    }
    match (installed, latest) {
        (Some(i), None) => format!("Installed {i}. Could not reach GitHub to check for a newer one."),
        (None, None) => "Could not reach GitHub to check for updates.".into(),
        (Some(i), Some(l)) if i == l => format!("Up to date: {i}."),
        (Some(i), Some(l)) => format!("Version {l} is available (you have {i})."),
        (None, Some(l)) => format!("Version {l} is available. Run the installer once to enable updates from here."),
    }
}

fn dot(id: &str, ok: Option<bool>) -> Node {
    let cls = match ok {
        Some(true) => "dot ok",
        Some(false) => "dot bad",
        None => "dot",
    };
    el("span", cls).id(&format!("dot-{id}"))
}

pub fn status_tab(s: &AppStatus, active: bool) -> Node {
    let k = s.known.then_some(());
    let xp = if s.xplane { "X-Plane: connected" } else { "X-Plane: not connected" };
    let sys = match &s.systems {
        Some(y) => format!("Systems: running, {} variables, {} ms per frame", num(y.variables), js_fixed(y.last_tick_micros / 1000.0, 2)),
        None => "Systems: stopped".into(),
    };
    let disp = match &s.displays {
        Some(d) => format!("Displays: {} screens at {} fps", crate::model::jstr(&d.screens), crate::model::jstr(&d.fps)),
        None => "Displays: stopped".into(),
    };
    let light = |id: &str, ok: bool, text: &str| Node::new("div").child(dot(id, k.map(|_| ok))).child(Node::new("span").id(&format!("st-{id}")).text(text));
    let upd = if s.update.is_empty() { update_text(true, None, None) } else { s.update.clone() };
    panel("status", active).child(
        el("div", "columns")
            .child(
                el("div", "column")
                    .child(Node::new("h2").text("System Status"))
                    .child(
                        el("div", "box")
                            .child(
                                el("div", "status")
                                    .child(button("small grey", "Restart all").attr("type", "button").act("action:restart-all"))
                                    .child(button("small grey", "Restart displays").attr("type", "button").act("action:restart-displays"))
                                    .child(button("small grey", "Restart systems").attr("type", "button").act("action:restart-systems"))
                                    .child(button("small grey", "Reset aircraft state").attr("type", "button").act("plugin:resetAircraftState"))
                                    .child(button("small", "New airframe").attr("type", "button").act("plugin:resetAirframe")),
                            )
                            .child(
                                el("div", "lights")
                                    .child(light("xplane", s.xplane, xp))
                                    .child(light("systems", s.systems.is_some(), &sys))
                                    .child(light("displays", s.displays.is_some(), &disp)),
                            ),
                    )
                    .child(Node::new("h2").text("Updates"))
                    .child(
                        el("div", "box")
                            .child(el("div", "row").child(span("label", &upd).id("upd-state")))
                            .child(
                                el("div", "row reset-row")
                                    .child(button("small grey", "Check again").attr("type", "button").id("upd-check").act("update:check"))
                                    .child(button("small", "Update").attr("type", "button").act("action:run-updater")),
                            ),
                    ),
            )
            .child(el("div", "column")),
    )
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DiagStatus {
    pub answered: bool,
    pub error: String,
    pub active: bool,
    pub dir: String,
    pub elapsed: f64,
    pub marks: f64,
    pub tour: Value,
    #[serde(rename = "enabledNextLaunch")]
    pub enabled_next_launch: bool,
    pub steps: Vec<(String, String)>,
    pub captured: Vec<usize>,
    pub current: Option<usize>,
    pub note: String,
    pub health: Option<String>,
}

pub fn diagnostics_tab(d: &DiagStatus, active: bool) -> Node {
    let state = if !d.answered {
        if d.error.is_empty() { "Asking the plugin...".to_string() } else { format!("The plugin is not answering: {}", d.error) }
    } else if d.active {
        let tour = match &d.tour {
            Value::Null | Value::Bool(false) => String::new(),
            Value::String(s) if s.is_empty() => String::new(),
            Value::Number(n) if n.as_f64() == Some(0.0) => String::new(),
            t => format!(", photo tour {}", crate::model::jstr(t)),
        };
        format!("Capturing to {} ({} s, {} marks{tour})", d.dir, num(round(d.elapsed)), num(d.marks))
    } else {
        format!("Not capturing this session.{}", if d.enabled_next_launch { " The next launch will capture." } else { "" })
    };
    let b = |id: &str, t: &str| button("small", t).attr("type", "button").id(id).attr_if("disabled", d.answered && !d.active).act(&format!("diag:{}", &id[5..]));
    let mut steps = el("div", "box").id("diag-steps");
    for (i, (title, detail)) in d.steps.iter().enumerate() {
        let done = d.captured.contains(&i);
        steps = steps.child(
            el("div", "row")
                .class_if("diag-current", d.current == Some(i))
                .style("gap:10px;align-items:flex-start;margin:8px 0")
                .child(button("small", if done { "Captured" } else { "Capture" }).attr("type", "button").act(&format!("diag:step:{i}")))
                .child(Node::new("div").child(Node::new("div").text(&format!("{}. {title}", i + 1))).child(label("label dim", detail))),
        );
    }
    let mut out = Node::new("pre").id("diag-health-out");
    out.cls = vec!["box".into()];
    let out = match &d.health {
        Some(t) => out.style("white-space:pre-wrap;max-height:420px;overflow:auto;font-size:12px;display:block").text(t),
        None => out.style("display:none;white-space:pre-wrap;max-height:420px;overflow:auto;font-size:12px"),
    };
    panel("diagnostics", active).child(
        el("div", "columns")
            .child(
                el("div", "column").child(Node::new("h2").text("Diagnostics")).child(
                    el("div", "box")
                        .child(label("label", &state).id("diag-state"))
                        .child(el("label", "row").child(checkbox(Some("diag-enable"), d.enabled_next_launch, "diag:enable")).child(span("label", "Capture everything on the next launch")))
                        .child(
                            el("div", "row")
                                .style("gap:8px;margin-top:10px")
                                .child(b("diag-tour", "Photo tour"))
                                .child(b("diag-screens", "Save screens"))
                                .child(b("diag-save", "Save bundle"))
                                .child(b("diag-health", "Health check")),
                        )
                        .child(el("div", "row").style("gap:8px;margin-top:10px").child(text_input("diag-note", "What you see or hear right now", &d.note).style("flex:1")).child(b("diag-mark", "MARK"))),
                ),
            )
            .child(el("div", "column").child(Node::new("h2").text("Test plan")).child(out).child(steps)),
    )
}

const DISCORD: &str = r##"<svg width="42" height="32" viewBox="0 0 71 55" fill="#fff" aria-hidden="true"><path d="M60.1 4.9A58.5 58.5 0 0 0 45.6.4a40 40 0 0 0-1.8 3.7 54 54 0 0 0-16.2 0A38 38 0 0 0 25.8.4 58.4 58.4 0 0 0 11.3 4.9C2.1 18.6-.4 32 .9 45.2a58.9 58.9 0 0 0 17.9 9 43 43 0 0 0 3.8-6.2 38 38 0 0 1-6-2.9l1.5-1.1a42 42 0 0 0 36 0l1.5 1.1a38 38 0 0 1-6 2.9 43 43 0 0 0 3.8 6.2 58.7 58.7 0 0 0 17.9-9C72.6 29.8 68.8 16.5 60.1 4.9zM23.7 37.1c-3.5 0-6.4-3.2-6.4-7.2s2.8-7.2 6.4-7.2c3.6 0 6.5 3.3 6.4 7.2 0 4-2.8 7.2-6.4 7.2zm23.6 0c-3.5 0-6.4-3.2-6.4-7.2s2.8-7.2 6.4-7.2c3.6 0 6.5 3.3 6.4 7.2 0 4-2.8 7.2-6.4 7.2z"/></svg>"##;

pub fn support_tab(active: bool) -> Node {
    let row = |t: &str, b: &str, act: &str| el("div", "support-row").child(Node::new("span").text(t)).child(button("link-button", b).attr("type", "button").act(&format!("action:{act}")));
    let log: Vec<(String, Value)> = serde_json::from_str(include_str!("changelog.json")).expect("changelog");
    let mut cl = el("div", "changelog").id("changelog");
    for (tag, v) in log {
        cl = match v {
            Value::Array(items) => cl.child(Node::new("ul").children(items.iter().map(|x| Node::new("li").text(x.as_str().unwrap_or(""))))),
            v => cl.child(Node::new(&tag).text(v.as_str().unwrap_or(""))),
        };
    }
    panel("support", active).child(
        el("div", "columns").child(
            el("div", "column").child(Node::new("h2").text("Help & Support")).child(
                el("div", "box")
                    .child(row("Found a problem or want to report an issue?", "Report an Issue", "report"))
                    .child(row("Been asked for your logs?", "Open Logs", "open-logs"))
                    .child(row("Need the systems' variable dumps?", "Open State Dumps", "open-dumps"))
                    .child(
                        el("div", "support-row")
                            .style("border:0")
                            .child(Node::new("span").text("Want to give feedback or chat with the team?"))
                            .child(el("button", "discord").attr("type", "button").act("action:discord").child(crate::js::svg(DISCORD)).child(Node::new("span").text("JOIN US ON DISCORD"))),
                    )
                    .child(cl),
            ),
        ),
    )
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Chart {
    pub id: String,
    pub name: Option<String>,
    pub category: Option<String>,
    pub type_code: Option<String>,
    pub url: Option<String>,
    pub image_day_url: Option<String>,
    pub image_night_url: Option<String>,
    pub kind: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct ChartsModel {
    pub source: String,
    pub icao: String,
    pub night: bool,
    pub state: String,
    pub list: Vec<Chart>,
    pub current: Option<usize>,
}

impl Default for ChartsModel {
    fn default() -> Self {
        Self { source: "amdb".into(), icao: String::new(), night: false, state: "Pick a chart source and enter an airport.".into(), list: vec![], current: None }
    }
}

fn nonempty(v: &Option<String>) -> Option<&str> {
    v.as_deref().filter(|s| !s.is_empty())
}

pub fn charts_tab(m: &ChartsModel, active: bool) -> Node {
    let mut list = Node::new("div").id("charts-list").style("width:320px;overflow-y:auto");
    let mut groups: Vec<(String, Vec<(usize, &Chart)>)> = Vec::new();
    for (i, c) in m.list.iter().enumerate() {
        let key = nonempty(&c.category).or(nonempty(&c.type_code)).unwrap_or("Other").to_string();
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, g)) => g.push((i, c)),
            None => groups.push((key, vec![(i, c)])),
        }
    }
    for (key, items) in &groups {
        list = list.child(label("section-title", key));
        for (i, c) in items {
            list = list.child(el("button", "tile").attr("type", "button").style("width:100%;text-align:left").act(&format!("chart:{i}")).child(Node::new("span").text(nonempty(&c.name).unwrap_or(&c.id))));
        }
    }
    let mut view = Node::new("div").id("charts-view").style("flex:1;overflow:auto;text-align:center");
    if let Some(c) = m.current.and_then(|i| m.list.get(i)) {
        let url = nonempty(&c.url).or(if m.night { nonempty(&c.image_night_url) } else { nonempty(&c.image_day_url) }).or(nonempty(&c.image_day_url)).unwrap_or("").to_string();
        let name = nonempty(&c.name).unwrap_or(&c.id);
        let mut head = el("div", "row").style("justify-content:space-between;align-items:center;gap:10px;margin:0 0 8px").child(label("label", name));
        if url.starts_with("https:") {
            head = head.child(button("small", "Open in browser").attr("type", "button").act("chart:open"));
        }
        let filter = if m.night { "invert(0.92) hue-rotate(180deg)" } else { "none" };
        let body = if c.kind.as_deref() == Some("pdf") {
            Node::new("iframe").attr("src", &url).style(&format!("width:100%;height:calc(100vh - 250px);border:0;background:#fff;border-radius:6px;filter:{filter}"))
        } else {
            Node::new("img").attr("src", &url).style(&format!("max-width:100%;background:#fff;filter:{}", if nonempty(&c.image_night_url).is_some() { "none" } else { filter }))
        };
        view = view.child(head).child(body);
    }
    let sources = [("amdb", "AMDB Bridge"), ("local", "My Charts (Documents/Charts)"), ("faa", "FAA d-TPP (US)")];
    panel("charts", active)
        .child(
            el("div", "box row")
                .style("gap:8px;align-items:center")
                .child(select(&sources, "charts:source", &m.source).id("charts-source").style("width:auto"))
                .child(text_input("charts-icao", "ICAO", &m.icao).attr("maxlength", "4").style("width:90px;text-transform:uppercase"))
                .child(button("small", "Load charts").attr("type", "button").id("charts-load").act("charts:load"))
                .child(el("label", "row").style("margin:0").child(checkbox(Some("charts-night"), m.night, "charts:night")).child(span("label", "Night")))
                .child(span("label dim", &m.state).id("charts-state")),
        )
        .child(el("div", "page-main").style("display:flex;gap:12px;min-height:0").child(list).child(view))
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SaveEntry {
    pub id: String,
    pub name: String,
    pub error: Option<String>,
    pub reason: Value,
    pub summary: Value,
    pub created_unix: Option<f64>,
    pub bytes: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SavesModel {
    pub answered: bool,
    pub status: String,
    pub states: Vec<SaveEntry>,
    pub settings: Value,
    pub name: String,
    /// Minutes east of UTC for the "Saved" column (the browser's local time).
    pub tz_offset_min: i64,
}

fn saves_kind(s: &SaveEntry) -> String {
    match &s.reason {
        Value::Null | Value::Bool(false) => "Unreadable".into(),
        Value::String(r) if r.is_empty() => "Unreadable".into(),
        Value::String(r) => match r.as_str() {
            "manual" => "Manual",
            "autosave" => "Autosave",
            "quit" => "On quit",
            _ => "",
        }
        .into(),
        Value::Object(o) if o.get("phase").is_some_and(|p| !p.is_null() && p != "") => {
            let p = crate::model::jstr(&o["phase"]);
            match p.as_str() {
                "pushback" => "Pushback",
                "takeoff" => "Take-off",
                "top_of_descent" => "Top of descent",
                "landing" => "Landing",
                "gate" => "At the gate",
                _ => "Phase",
            }
            .into()
        }
        _ => String::new(),
    }
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64() != Some(0.0),
        _ => true,
    }
}

fn saves_summary(s: &SaveEntry) -> String {
    let m = &s.summary;
    let g = |k: &str| m.get(k);
    let mut parts = Vec::new();
    if truthy(g("origin")) || truthy(g("destination")) {
        let o = if truthy(g("origin")) { crate::model::jstr(&m["origin"]) } else { "----".into() };
        let d = if truthy(g("destination")) { crate::model::jstr(&m["destination"]) } else { "----".into() };
        parts.push(format!("{o}-{d}"));
    }
    let ground = truthy(g("on_ground"));
    if ground {
        parts.push("On ground".into());
    } else if let Some(a) = g("altitude_ft").and_then(Value::as_f64) {
        parts.push(format!("{} ft", num(round(a / 100.0) * 100.0)));
    }
    if let Some(v) = g("ias_kt").and_then(Value::as_f64) {
        if !ground {
            parts.push(format!("{} kt", num(round(v))));
        }
    }
    if truthy(g("flight_phase")) {
        parts.push(crate::model::jstr(&m["flight_phase"]).to_uppercase());
    }
    parts.join(" \u{b7} ")
}

fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// `new Date(unix * 1000).toLocaleString()` in en-US at the given offset.
pub fn locale_datetime(unix: f64, tz_offset_min: i64) -> String {
    let t = (unix.floor() as i64) + tz_offset_min * 60;
    let (days, secs) = (t.div_euclid(86_400), t.rem_euclid(86_400));
    let (y, mo, d) = civil(days);
    let (h, mi, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    format!("{mo}/{d}/{y}, {}:{mi:02}:{s:02} {}", (h + 11) % 12 + 1, if h < 12 { "AM" } else { "PM" })
}

pub fn saves_content(m: &SavesModel) -> Node {
    let root = Node::new("div").id("saves-content");
    let mut root = root;
    root.cls = vec!["page-main".into()];
    if !m.answered {
        return root.child(label("box", "Start X-Plane with the A380X to save and load flights."));
    }
    let toolbar = el("div", "study-toolbar")
        .child(Node::new("input").attr("type", "text").attr("placeholder", "Name this save").attr("maxlength", "64").style("width:280px").value(&m.name).act("focus:saves-name"))
        .child(button("small primary", "Save now").attr("type", "button").act("saves:save"))
        .child(span("label dim", &m.status));
    let rows: Vec<Node> = m
        .states
        .iter()
        .map(|s| {
            let when = s.created_unix.filter(|u| *u != 0.0).map(|u| locale_datetime(u, m.tz_offset_min)).unwrap_or_default();
            let size = s.bytes.filter(|b| *b != 0.0).map(|b| format!("{} kB", num(round(b / 1024.0).max(1.0)))).unwrap_or_default();
            let mut actions = Node::new("td").style("white-space:nowrap;text-align:right");
            if s.error.is_none() {
                actions = actions.child(button("small", "Load").attr("type", "button").act(&format!("saves:load:{}", s.id))).text(" ").child(button("small grey", "Rename").attr("type", "button").act(&format!("saves:rename:{}", s.id))).text(" ");
            }
            actions = actions.child(button("small grey", "Delete").attr("type", "button").act(&format!("saves:delete:{}", s.id)));
            let sub = match &s.error {
                Some(e) => format!("Unreadable: {e}"),
                None => saves_summary(s),
            };
            Node::new("tr")
                .child(Node::new("td").child(Node::new("div").text(if s.name.is_empty() { &s.id } else { &s.name })).child(label("label dim", &sub)))
                .child(Node::new("td").text(&saves_kind(s)))
                .child(Node::new("td").text(&when))
                .child(Node::new("td").text(&size))
                .child(actions)
        })
        .collect();
    let table = if rows.is_empty() {
        label("label dim", "No saved flights yet.")
    } else {
        let th = |t: &str| Node::new("th").text(t);
        Node::new("table").style("width:100%").child(Node::new("tr").child(th("Name")).child(th("Kind")).child(th("Saved")).child(th("Size")).child(Node::new("th"))).children(rows)
    };
    let st = &m.settings;
    let n = |k: &str, d: f64| st.get(k).and_then(Value::as_f64).unwrap_or(d);
    let b = |k: &str| st.get(k).and_then(Value::as_bool).unwrap_or(true);
    let iv: Vec<(String, String)> = [0, 2, 5, 10, 15, 30].iter().map(|m| (m.to_string(), if *m == 0 { "Off".into() } else { format!("Every {m} min") })).collect();
    let sl: Vec<(String, String)> = (1..=10).map(|i| (i.to_string(), i.to_string())).collect();
    let as_ref = |v: &Vec<(String, String)>| v.iter().map(|(a, b)| (a.clone(), b.clone())).collect::<Vec<_>>();
    let sel = |v: Vec<(String, String)>, act: &str, val: &str| {
        let o: Vec<(&str, &str)> = v.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        select(&o, act, val)
    };
    let row = |l: &str, c: Node| el("div", "row").style("gap:12px;align-items:center;margin:8px 0").child(span("label", l).style("width:260px")).child(c);
    let settings = el("div", "box")
        .child(Node::new("h3").text("Autosave"))
        .child(row("Autosave interval", sel(as_ref(&iv), "saves:interval", &num(n("interval_min", 5.0)))))
        .child(row("Rolling autosave slots", sel(as_ref(&sl), "saves:slots", &num(n("rolling_slots", 3.0)))))
        .child(row("Save at pushback, take-off, top of descent, landing and the gate", checkbox(None, b("phase_saves"), "saves:phase")))
        .child(row("Save when X-Plane quits", checkbox(None, b("quit_save"), "saves:quit")));
    root.child(toolbar).child(el("div", "box").child(Node::new("h3").text("Saved flights")).child(table)).child(settings)
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Detent {
    pub center: f64,
    pub band: f64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Calibration {
    pub enabled: bool,
    #[serde(rename = "reverseOnAxis")]
    pub reverse_on_axis: bool,
    pub detents: Vec<Detent>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ThrottleModel {
    pub answered: bool,
    pub names: Vec<String>,
    pub draft: Calibration,
    pub raw: Option<Vec<Option<f64>>>,
    pub angle: Vec<f64>,
    pub said: String,
}

pub fn throttle_content(m: &ThrottleModel) -> Node {
    let mut root = Node::new("div").id("throttle-content");
    root.cls = vec!["page-main".into()];
    if !m.answered {
        return root.child(label("box", "Start X-Plane with the A380X to calibrate the throttle."));
    }
    let d = &m.draft;
    let live = match &m.raw {
        Some(raw) => {
            let axis: Vec<Option<f64>> = raw.iter().map(|r| r.map(|r| r * 2.0 - 1.0)).collect();
            format!(
                "Axis {}     TLA {}",
                axis.iter().enumerate().map(|(i, a)| format!("{}: {}", i + 1, a.map(|a| js_fixed(a, 3)).unwrap_or_else(|| "--".into()))).collect::<Vec<_>>().join("   "),
                m.angle.iter().map(|a| format!("{}\u{b0}", js_fixed(*a, 1))).collect::<Vec<_>>().join(" ")
            )
        }
        None => String::new(),
    };
    let last = m.raw.as_ref().and_then(|r| r.iter().flatten().next().map(|r| r * 2.0 - 1.0));
    let left = last.map(|a| format!("left: {}%;", num((a + 1.0) / 2.0 * 100.0))).unwrap_or_else(|| "left: 0%;".into());
    let mut rows = Node::new("div");
    for (i, name) in m.names.iter().enumerate() {
        if !d.reverse_on_axis && i < 2 {
            rows = rows.child(Node::new("span"));
            continue;
        }
        let det = d.detents.get(i).cloned().unwrap_or_default();
        let numi = |v: f64, min: &str, max: &str, act: String| Node::new("input").attr("type", "number").attr("step", "0.01").attr("min", min).attr("max", max).value(&js_fixed(v, 3)).act(&act);
        rows = rows.child(
            el("div", "ef-row ef-detent")
                .child(span("label", name))
                .child(numi(det.center, "-1", "1", format!("thr:center:{i}")))
                .child(span("ef-unit", "\u{b1}"))
                .child(numi(det.band, "0", "0.5", format!("thr:band:{i}")))
                .child(button("small", "Set from throttle").attr("type", "button").act(&format!("thr:set:{i}"))),
        );
    }
    root.child(
        el("div", "box ef-box")
            .child(label("section-title", "Throttle calibration"))
            .child(label("label dim", "Move the lever to each detent and press Set from throttle, then Save and apply. Values are the axis position from -1 to 1 and the width of the detent either side."))
            .child(el("label", "row").child(checkbox(None, d.enabled, "thr:enabled")).child(span("label", "Use this calibration")))
            .child(el("label", "row").child(checkbox(None, d.reverse_on_axis, "thr:reverse")).child(span("label", "Reverser on axis")))
            .child(label("ef-live", &live))
            .child(el("div", "ef-axis").child(el("div", "ef-axis-mark").style(&left)))
            .child(rows)
            .child(el("div", "row").child(button("", "Save and apply").attr("type", "button").act("thr:save")).child(button("", "Reset to defaults").attr("type", "button").act("thr:defaults")))
            .child(label("label dim ef-said", &m.said)),
    )
}

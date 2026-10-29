use crate::model::{path, EfbModel};
use crate::{button, label, span, Node};
use fws_ecl::mirror::{ChecklistMirror, EclMirror};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

mod pages;
pub use pages::{ewd_shot, ofp_markdown, phase_diagram};

#[derive(Clone, Debug, Deserialize)]
pub struct DocLine(pub String, pub String, pub u8, pub String);

impl DocLine {
    fn has(&self, f: char) -> bool {
        self.3.contains(f)
    }
    fn plain(&self) -> bool {
        !self.has('c') && !self.has('h') && !self.has('b')
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct NormalCl {
    pub id: String,
    pub t: String,
    pub i: Vec<DocLine>,
    pub d: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Proc {
    pub id: String,
    pub t: String,
    pub s: Option<String>,
    pub c: Option<String>,
    pub l: Option<String>,
    pub r: Option<String>,
    pub i: Vec<DocLine>,
    pub f: Option<f64>,
    pub ph: Option<Vec<u32>>,
    pub sd: Option<String>,
    pub x: Option<Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DocsData {
    pub normal: Vec<NormalCl>,
    pub sensed: Vec<Proc>,
    #[serde(rename = "nonSensed")]
    pub non_sensed: Vec<Proc>,
    pub deferred: Vec<NormalCl>,
    pub limitations: Vec<String>,
    pub inop: Vec<String>,
    pub guide: Vec<(String, Vec<String>)>,
    pub maps: Vec<String>,
    pub perf: Value,
}

impl DocsData {
    pub fn bundled() -> Self {
        serde_json::from_str(include_str!("docs_data.json")).expect("docs data")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    NormalAll,
    Normal(usize),
    Deferred(usize),
    Proc { sensed: bool, index: usize },
    Limits,
    Engine,
    WvTable,
    Ratings,
    Perf,
    List(bool),
    SysMap(usize),
    Guide(usize),
    Ofp(Option<&'static str>),
    Mine(usize),
    MineNew,
    Md(String),
    MdImg(String),
    MdPdf(String),
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MyDoc {
    pub t: String,
    pub b: String,
}

/// The app's `/api/documents` tree of the user's Documents folder.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MdEntry {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub kids: Option<Vec<MdEntry>>,
}

#[derive(Clone, Debug)]
pub struct Leaf {
    pub id: String,
    pub t: String,
    pub kind: Kind,
    pub tone: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Folder {
    pub key: String,
    pub t: String,
    pub kids: Vec<Folder>,
    pub leaves: Vec<Leaf>,
}

#[derive(Clone, Debug)]
pub struct DocsState {
    pub open: BTreeSet<String>,
    pub sel: String,
    pub query: String,
    pub zoom: f64,
    pub ticks: BTreeMap<String, BTreeSet<usize>>,
    pub folds: BTreeSet<String>,
    pub pins: Vec<String>,
    pub my_docs: Vec<MyDoc>,
    pub copilot: bool,
    pub md_tree: Vec<MdEntry>,
    pub md_text: BTreeMap<String, String>,
    pub md_checks: BTreeMap<String, BTreeSet<usize>>,
    pub ewd_shots: BTreeSet<String>,
}

impl Default for DocsState {
    fn default() -> Self {
        Self {
            open: ["qrh", "back"].iter().map(|s| s.to_string()).collect(),
            sel: "n:all".into(),
            query: String::new(),
            zoom: 1.0,
            ticks: BTreeMap::new(),
            folds: BTreeSet::new(),
            pins: Vec::new(),
            my_docs: Vec::new(),
            copilot: false,
            md_tree: Vec::new(),
            md_text: BTreeMap::new(),
            md_checks: BTreeMap::new(),
            ewd_shots: BTreeSet::new(),
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct StateJson {
    open: Option<Vec<String>>,
    sel: Option<String>,
    query: String,
    zoom: Option<f64>,
    ticks: BTreeMap<String, Vec<usize>>,
    folds: Vec<String>,
    pins: Vec<String>,
    my_docs: Vec<MyDoc>,
    copilot: bool,
    ecl: Option<EclMirror>,
    md_tree: Vec<MdEntry>,
    md_text: BTreeMap<String, String>,
    md_checks: BTreeMap<String, Vec<usize>>,
    ewd_shots: Vec<String>,
}

impl DocsState {
    pub fn from_json(s: &str) -> Self {
        let j: StateJson = serde_json::from_str(s).expect("docs state json");
        let d = Self::default();
        let mut st = Self {
            open: j.open.map(|v| v.into_iter().collect()).unwrap_or(d.open),
            sel: j.sel.unwrap_or(d.sel),
            query: j.query,
            zoom: j.zoom.unwrap_or(1.0),
            ticks: j.ticks.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect(),
            folds: j.folds.into_iter().collect(),
            pins: j.pins,
            my_docs: j.my_docs,
            copilot: j.copilot,
            md_tree: j.md_tree,
            md_text: j.md_text,
            md_checks: j.md_checks.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect(),
            ewd_shots: j.ewd_shots.into_iter().collect(),
        };
        if let Some(e) = j.ecl {
            st.apply_ecl(&e);
        }
        st
    }

    pub fn apply_ecl(&mut self, ecl: &EclMirror) {
        for c in &ecl.checklists {
            self.ticks.insert(c.efb_page_id.clone(), ecl.efb_ticks(&c.efb_page_id).into_iter().collect());
        }
    }
}

pub fn lines_from_mirror(c: &ChecklistMirror) -> Vec<DocLine> {
    c.items.iter().map(|it| DocLine(it.name.clone(), it.value.clone(), it.level, if it.headline { "h".into() } else { String::new() })).collect()
}

pub fn with_ecl(data: &DocsData, ecl: &EclMirror) -> DocsData {
    let mut out = data.clone();
    for c in &ecl.checklists {
        if let Some(i) = c.efb_page_id.strip_prefix("n:").and_then(|s| s.parse::<usize>().ok()) {
            if let Some(n) = out.normal.get_mut(i) {
                n.i = lines_from_mirror(c);
            }
        }
    }
    out
}

fn proc_leaf(p: &Proc, sensed: bool, index: usize) -> Leaf {
    Leaf { id: format!("p:{}", p.id), t: p.t.clone(), kind: Kind::Proc { sensed, index }, tone: p.l.clone() }
}

fn ofp_leaves(ofp: Option<&Value>) -> Vec<Leaf> {
    let leaf = |id: &str, t: String, part: &'static str| Leaf { id: id.into(), t, kind: Kind::Ofp(Some(part)), tone: None };
    let Some(o) = ofp.filter(|o| !o.is_null()) else {
        return vec![Leaf { id: "f:none".into(), t: "No OFP loaded".into(), kind: Kind::Ofp(None), tone: None }];
    };
    let truthy = |v: &Value| !(v.is_null() || v == &Value::Bool(false) || v.as_str() == Some(""));
    let code = |k: &str| match path(o, &[k, "icao_code"]) {
        Value::String(s) => s.clone(),
        v if truthy(v) => crate::model::jstr(v),
        _ => String::new(),
    };
    let mut out = vec![leaf("f:ofp", format!("OFP {}-{}", code("origin"), code("destination")), "plan")];
    if truthy(path(o, &["weights"])) {
        out.push(leaf("f:load", "LOADSHEET".into(), "load"));
    }
    if truthy(path(o, &["weather"])) {
        out.push(leaf("f:wx", "WEATHER".into(), "wx"));
    }
    if truthy(path(o, &["notams", "notamdrec"])) {
        out.push(leaf("f:notam", "NOTAMS".into(), "notam"));
    }
    if truthy(path(o, &["files", "pdf", "link"])) {
        out.push(leaf("f:pdf", "OFP PDF".into(), "pdf"));
    }
    out
}

pub const LIVE_MAPS: [&str; 11] = [
    "LANDING GEAR AND BRAKES", "APU", "FIRE PROTECTION", "OXYGEN", "DOORS", "ANTI ICE AND RAIN", "NAVIGATION", "SURVEILLANCE", "COMMUNICATIONS", "AUTO FLIGHT", "WARNING AND RECORDING",
];

pub fn doc_tree(data: &DocsData, st: &DocsState, ofp: Option<&Value>) -> Vec<Folder> {
    let mut back = vec![Leaf { id: "n:all".into(), t: "NORMAL CHECKLIST".into(), kind: Kind::NormalAll, tone: Some("white".into()) }];
    let emer = ["EMER EVAC", "EMER DESCENT", "DITCHING", "FORCED LANDING"];
    for (i, p) in data.non_sensed.iter().enumerate() {
        if emer.iter().any(|e| p.t.contains(e)) {
            back.push(proc_leaf(p, false, i));
        }
    }
    let mut ns_cats: Vec<(String, Vec<Leaf>)> = Vec::new();
    for (i, p) in data.non_sensed.iter().enumerate() {
        let c = p.c.clone().filter(|c| !c.is_empty()).unwrap_or_else(|| "GENERAL".into());
        let leaf = proc_leaf(p, false, i);
        match ns_cats.iter_mut().find(|(k, _)| *k == c) {
            Some((_, v)) => v.push(leaf),
            None => ns_cats.push((c, vec![leaf])),
        }
    }
    let mut sys: BTreeMap<String, Vec<Leaf>> = BTreeMap::new();
    for (i, p) in data.sensed.iter().enumerate() {
        let s = p.s.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "OTHER".into());
        sys.entry(s).or_default().push(proc_leaf(p, true, i));
    }
    let leaf = |id: &str, t: &str, kind: Kind| Leaf { id: id.into(), t: t.into(), kind, tone: None };
    let f = |key: &str, t: &str, kids: Vec<Folder>, leaves: Vec<Leaf>| Folder { key: key.into(), t: t.into(), kids, leaves };
    fn md_nodes(list: &[MdEntry]) -> (Vec<Folder>, Vec<Leaf>) {
        let (mut kids, mut leaves) = (Vec::new(), Vec::new());
        for n in list {
            if let Some(k) = &n.kids {
                let (kk, ll) = md_nodes(k);
                kids.push(Folder { key: format!("md:{}", n.path), t: n.name.clone(), kids: kk, leaves: ll });
            } else {
                let kind = match n.kind.as_deref() {
                    Some("image") => Kind::MdImg(n.path.clone()),
                    Some("pdf") => Kind::MdPdf(n.path.clone()),
                    _ => Kind::Md(n.path.clone()),
                };
                leaves.push(Leaf { id: format!("md:{}", n.path), t: n.name.clone(), kind, tone: None });
            }
        }
        (kids, leaves)
    }
    let (md_kids, mut mine) = md_nodes(&st.md_tree);
    mine.extend(st.my_docs.iter().enumerate().map(|(i, d)| leaf(&format!("m:{i}"), if d.t.is_empty() { "Untitled" } else { &d.t }, Kind::Mine(i))));
    mine.push(leaf("m:new", "+ New note", Kind::MineNew));
    vec![
        f(
            "qrh",
            "QRH",
            vec![
                f("back", "BACK COVER", vec![], back),
                f("abn", "ABNORMAL AND EMERGENCY PROCEDURES", ns_cats.into_iter().map(|(c, ps)| f(&format!("ns:{c}"), &c, vec![], ps)).collect(), vec![]),
                f(
                    "normal",
                    "NORMAL PROCEDURES",
                    vec![],
                    data.normal.iter().enumerate().map(|(i, c)| Leaf { id: format!("n:{i}"), t: c.t.clone(), kind: Kind::Normal(i), tone: Some("white".into()) }).collect(),
                ),
                f(
                    "supp",
                    "SUPPLEMENTARY PROCEDURES",
                    vec![],
                    data.deferred
                        .iter()
                        .enumerate()
                        .map(|(i, c)| Leaf { id: format!("d:{i}"), t: if c.t.is_empty() { "DEFERRED PROCEDURE".into() } else { c.t.clone() }, kind: Kind::Deferred(i), tone: Some("white".into()) })
                        .collect(),
                ),
                f(
                    "ops",
                    "OPERATIONAL DATA",
                    vec![],
                    vec![leaf("o:lim", "LIMITATIONS", Kind::Limits), leaf("o:eng", "ENGINE LIMITS", Kind::Engine), leaf("o:wv", "WEIGHT VARIANTS", Kind::WvTable), leaf("o:rating", "ENGINE RATINGS", Kind::Ratings)],
                ),
                f("perf", "IN FLIGHT PERFORMANCE", vec![], vec![leaf("f:perf", "PERFORMANCE CALCULATORS", Kind::Perf)]),
            ],
            vec![],
        ),
        f(
            "ecam",
            "ECAM",
            sys.into_iter().map(|(s, ps)| f(&format!("sys:{s}"), &s, vec![], ps)).collect(),
            vec![leaf("e:lim", "ECAM LIMITATIONS", Kind::List(false)), leaf("e:inop", "INOP SYSTEMS", Kind::List(true))],
        ),
        f("maps", "SYSTEM DIAGRAMS", vec![], data.maps.iter().map(String::as_str).chain(LIVE_MAPS).enumerate().map(|(i, t)| leaf(&format!("sd:{i}"), t, Kind::SysMap(i))).collect()),
        f("guide", "User Guide", vec![], data.guide.iter().enumerate().map(|(i, (t, _))| leaf(&format!("g:{i}"), t, Kind::Guide(i))).collect()),
        f("flight", "Flight Documents", vec![], ofp_leaves(ofp)),
        f("mine", "My Documents", md_kids, mine),
    ]
}

fn locate<'a>(tree: &'a [Folder], id: &str) -> Option<(&'a Leaf, Vec<&'a Folder>)> {
    fn walk<'a>(nodes: &'a [Folder], trail: &mut Vec<&'a Folder>, id: &str) -> Option<(&'a Leaf, Vec<&'a Folder>)> {
        for n in nodes {
            trail.push(n);
            if let Some(l) = n.leaves.iter().find(|l| l.id == id) {
                return Some((l, trail.clone()));
            }
            if let Some(hit) = walk(&n.kids, trail, id) {
                return Some(hit);
            }
            trail.pop();
        }
        None
    }
    walk(tree, &mut Vec::new(), id)
}

fn el(tag: &str, classes: &str) -> Node {
    Node::el(tag, classes)
}

fn tone_cls(base: &str, active: bool, tone: &Option<String>) -> String {
    let mut c = base.to_string();
    if active {
        c.push_str(" active");
    }
    if let Some(t) = tone.as_deref().filter(|t| !t.is_empty()) {
        c.push_str(" tone-");
        c.push_str(t);
    }
    c
}

struct Block<'a> {
    ln: &'a DocLine,
    j: usize,
    kids: Vec<Block<'a>>,
}

fn doc_blocks(lines: &[DocLine]) -> Vec<Block<'_>> {
    fn insert<'a>(list: &mut Vec<Block<'a>>, depth: usize, b: Block<'a>) {
        if depth == 0 {
            list.push(b);
        } else {
            insert(&mut list.last_mut().unwrap().kids, depth - 1, b);
        }
    }
    let mut root = Vec::new();
    let mut stack: Vec<i32> = Vec::new();
    for (j, ln) in lines.iter().enumerate() {
        let level = ln.2 as i32;
        while !stack.is_empty() && level <= *stack.last().unwrap() {
            stack.pop();
        }
        insert(&mut root, stack.len(), Block { ln, j, kids: Vec::new() });
        if ln.has('c') {
            stack.push(level);
        }
    }
    root
}

fn collect_plain(ns: &[Block], out: &mut Vec<usize>) {
    for n in ns {
        if n.ln.plain() {
            out.push(n.j);
        }
        collect_plain(&n.kids, out);
    }
}

fn plain_count(lines: &[DocLine]) -> usize {
    lines.iter().filter(|l| l.plain()).count()
}

pub fn doc_lines(id: &str, lines: &[DocLine], st: &DocsState) -> Vec<Node> {
    let empty = BTreeSet::new();
    let ticks = st.ticks.get(id).unwrap_or(&empty);
    let blocks = doc_blocks(lines);
    let mut order = Vec::new();
    collect_plain(&blocks, &mut order);
    let current = order.iter().copied().find(|j| !ticks.contains(j));
    fn draw(nodes: &[Block], id: &str, ticks: &BTreeSet<usize>, current: Option<usize>, st: &DocsState, into: &mut Vec<Node>) {
        for b in nodes {
            let ln = b.ln;
            if ln.has('h') {
                into.push(label("qd-sub", &ln.0));
                draw(&b.kids, id, ticks, current, st, into);
                continue;
            }
            if ln.has('c') {
                let closed = st.folds.contains(&format!("{id}#{}", b.j));
                let mut actions = Vec::new();
                collect_plain(&b.kids, &mut actions);
                let complete = !actions.is_empty() && actions.iter().all(|a| ticks.contains(a));
                let dot = el("span", "qd-dot").class_if("open", !closed).class_if("done", complete);
                into.push(el("button", "qd-cond").attr("type", "button").child(dot).child(Node::new("span").text(&ln.0)));
                let mut block = el("div", "qd-block").class_if("closed", closed);
                if !closed {
                    let mut kids = Vec::new();
                    draw(&b.kids, id, ticks, current, st, &mut kids);
                    block = block.children(kids);
                }
                into.push(block);
                continue;
            }
            let row = el("div", "qd-line")
                .class_if("done", ticks.contains(&b.j))
                .class_if("current", current == Some(b.j))
                .class_if("note", ln.has('b'))
                .class_if("amber", ln.has('a'))
                .class_if("red", ln.has('r'))
                .child(span("qd-n", &ln.0));
            let row = if ln.1.is_empty() { row } else { row.child(el("span", "qd-dots")).child(span("qd-v", &ln.1)) };
            into.push(row);
            draw(&b.kids, id, ticks, current, st, into);
        }
    }
    let mut out = Vec::new();
    draw(&blocks, id, ticks, current, st, &mut out);
    out
}

fn fo_button(m: &EfbModel, st: &DocsState) -> Node {
    let copilot = st.copilot || m.settings.get("xphfbw.copilotMode").is_some_and(|v| v == "true");
    let mode = if copilot { "solo" } else { m.settings.get("xphfbw.copilotChecklist").map(String::as_str).unwrap_or("") };
    let text = match mode {
        "response" => "F/O RESPONDS",
        "solo" => "F/O RUNS IT",
        _ => "F/O READS",
    };
    button("", text).attr("type", "button")
}

fn doc_title(text: &str, tone: Option<&str>, band: bool) -> Node {
    let mut c = "qd-title".to_string();
    if let Some(t) = tone.filter(|t| !t.is_empty()) {
        c.push_str(" tone-");
        c.push_str(t);
    }
    if band {
        c.push_str(" band");
    }
    label(&c, text)
}

fn doc_page(data: &DocsData, m: &EfbModel, st: &DocsState, leaf: &Leaf) -> Vec<Node> {
    let mut body = Vec::new();
    match &leaf.kind {
        Kind::NormalAll => {
            body.push(label("qd-crumb", "QRH > BACK COVER > NORMAL CHECKLIST"));
            body.push(doc_title("NORMAL CHECKLIST", None, false));
            body.push(el("div", "qd-tools").child(button("", "RESET ALL").attr("type", "button")));
            for (i, c) in data.normal.iter().enumerate() {
                let id = format!("n:{i}");
                let open = st.folds.contains(&format!("card:{id}"));
                let done = st.ticks.get(&id).is_some_and(|t| t.len() >= plain_count(&c.i));
                body.push(
                    el("button", "qd-card")
                        .attr("type", "button")
                        .class_if("open", open)
                        .class_if("done", done)
                        .child(Node::new("span").text(&format!("{} CHECKLIST", c.t)))
                        .child(span("qd-chev", if open { "\u{25b4}" } else { "\u{25be}" })),
                );
                if open {
                    body.push(
                        el("div", "qd-cardbody")
                            .child(el("div", "qd-tools").child(fo_button(m, st)))
                            .children(doc_lines(&id, &c.i, st))
                            .child(button("qd-complete", "COMPLETE").attr("type", "button")),
                    );
                }
            }
        }
        Kind::Normal(i) => {
            let c = &data.normal[*i];
            body.push(doc_title(&format!("{} CHECKLIST", leaf.t), None, false));
            body.push(el("div", "qd-tools").child(fo_button(m, st)));
            body.extend(doc_lines(&leaf.id, &c.i, st));
        }
        Kind::Deferred(i) => {
            body.push(doc_title(&leaf.t, None, false));
            body.extend(doc_lines(&leaf.id, &data.deferred[*i].i, st));
        }
        _ => body.extend(pages::more(data, m, st, leaf)),
    }
    body
}

fn zoom_str(z: f64) -> String {
    if z.fract() == 0.0 { format!("{}", z as i64) } else { format!("{z}") }
}

pub fn docs_page(data: &DocsData, m: &EfbModel, st: &DocsState) -> Node {
    let tree_data = doc_tree(data, st, m.ofp.as_ref());
    let q = st.query.trim().to_uppercase();
    let matches = |l: &Leaf| q.is_empty() || l.t.to_uppercase().contains(&q);
    fn count(n: &Folder, m: &dyn Fn(&Leaf) -> bool) -> usize {
        n.leaves.iter().filter(|l| m(l)).count() + n.kids.iter().map(|k| count(k, m)).sum::<usize>()
    }
    fn draw(n: &Folder, depth: usize, q: &str, st: &DocsState, m: &dyn Fn(&Leaf) -> bool, out: &mut Vec<Node>) {
        if !q.is_empty() && count(n, m) == 0 {
            return;
        }
        let open = !q.is_empty() || st.open.contains(&n.key);
        out.push(
            el("button", "qd-folder")
                .attr("type", "button")
                .class_if("open", open)
                .class_if("top", depth == 0)
                .style(&format!("padding-left:{}px", 10 + depth * 12))
                .child(Node::new("span").text(&n.t))
                .child(span("qd-chev", if open { "\u{2304}" } else { "\u{203a}" })),
        );
        if !open {
            return;
        }
        for k in &n.kids {
            draw(k, depth + 1, q, st, m, out);
        }
        for l in n.leaves.iter().filter(|l| m(l)) {
            out.push(
                Node::el("button", &tone_cls("qd-leaf", l.id == st.sel, &l.tone))
                    .attr("type", "button")
                    .style(&format!("padding-left:{}px", 22 + depth * 12))
                    .text(&l.t),
            );
        }
    }
    let mut tree = Vec::new();
    let pins: Vec<&Leaf> = st.pins.iter().filter_map(|id| locate(&tree_data, id).map(|h| h.0)).collect();
    if !pins.is_empty() && q.is_empty() {
        tree.push(label("qd-pinhead", "PINNED"));
        for l in &pins {
            tree.push(Node::el("button", &tone_cls("qd-leaf", l.id == st.sel, &l.tone)).attr("type", "button").style("padding-left:12px").text(&l.t));
        }
        tree.push(el("div", "qd-sep"));
    }
    for n in &tree_data {
        draw(n, 0, &q, st, &matches, &mut tree);
    }
    let side = el("div", "qd-side")
        .child(label("qd-h1", "Documents"))
        .child(
            el("div", "qd-search")
                .child(Node::new("input").attr("type", "text").attr("placeholder", "Search documents").value(&st.query))
                .child(button("qd-x", "\u{00d7}").attr("type", "button")),
        )
        .child(el("div", "qd-tree").children(tree));
    let mut main = el("div", "qd-main");
    let mut body = el("div", "qd-body").style(&format!("zoom: {}", zoom_str(st.zoom)));
    if let Some((leaf, trail)) = locate(&tree_data, &st.sel).or_else(|| locate(&tree_data, "n:all")) {
        let pinned = st.pins.contains(&leaf.id);
        let crumb: Vec<&str> = trail.iter().map(|n| n.t.as_str()).chain([leaf.t.as_str()]).collect();
        main = main.child(
            el("div", "qd-crumbrow")
                .child(label("qd-crumb", &crumb.join("  >  ")))
                .child(button("qd-pin", if pinned { "PINNED" } else { "PIN" }).class_if("on", pinned).attr("type", "button")),
        );
        body = body.children(doc_page(data, m, st, leaf));
    }
    let zoom_pct = format!("{}%", (st.zoom * 100.0).round() as i64);
    main = main.child(body).child(
        el("div", "qd-foot").child(span("qd-sim", "FOR FLIGHT SIMULATOR USE ONLY")).child(
            el("div", "qd-tools").child(button("qd-btn", "RESET ALL").attr("type", "button")).child(
                el("div", "qd-zoom")
                    .child(button("qd-btn qd-z", "\u{2212}").attr("type", "button"))
                    .child(Node::new("span").text(&zoom_pct))
                    .child(button("qd-btn qd-z", "+").attr("type", "button")),
            ),
        ),
    );
    el("div", "page-main").id("cl-content").child(el("div", "qd-wrap box").child(side).child(main))
}

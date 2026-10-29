use crate::{button, label, span, svg_icon, Node};
use crate::model::EfbModel;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Item {
    pub key: Option<String>,
    pub label: String,
    pub desc: Option<String>,
    pub seg: Option<Vec<(String, String)>>,
    pub sel: Option<Vec<(String, String)>>,
    pub range: Option<[f64; 3]>,
    pub num: Option<[f64; 3]>,
    pub unit: Option<String>,
    pub text: Option<String>,
    pub bool: Option<String>,
    pub show: Option<String>,
    pub secret: bool,
    pub callouts: bool,
    pub zoom: bool,
    pub reset: bool,
    pub action: Option<String>,
    pub button: Option<String>,
    #[serde(rename = "statusId")]
    pub status_id: Option<String>,
    pub id: Option<String>,
    #[serde(rename = "labelId")]
    pub label_id: Option<String>,
    #[serde(rename = "liveryKey")]
    pub livery_key: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Category {
    pub name: String,
    pub blurb: String,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, Default)]
pub struct SettingsTable {
    pub categories: Vec<Category>,
    pub defaults: BTreeMap<String, String>,
}

pub const CALLOUTS: [(&str, u32); 19] = [
    ("TWO THOUSAND FIVE HUNDRED", 1 << 0),
    ("TWENTY FIVE HUNDRED", 1 << 1),
    ("2000", 1 << 2),
    ("1000", 1 << 3),
    ("500", 1 << 4),
    ("400", 1 << 6),
    ("300", 1 << 7),
    ("200", 1 << 8),
    ("100", 1 << 9),
    ("90", 1 << 10),
    ("80", 1 << 11),
    ("70", 1 << 12),
    ("60", 1 << 13),
    ("50", 1 << 14),
    ("40", 1 << 15),
    ("30", 1 << 16),
    ("20", 1 << 17),
    ("10", 1 << 18),
    ("5", 1 << 19),
];

const GLOBAL_CATEGORIES: [&str; 3] = ["Connections", "Displays", "Misc"];

impl SettingsTable {
    pub fn bundled() -> Self {
        Self::from_json(include_str!("settings_table.json")).expect("bundled settings table")
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let mut t = SettingsTable::default();
        for c in v["page"].as_array().ok_or("page")? {
            let items: Vec<Item> = serde_json::from_value(c[2].clone()).map_err(|e| e.to_string())?;
            t.categories.push(Category { name: c[0].as_str().unwrap_or("").to_string(), blurb: c[1].as_str().unwrap_or("").to_string(), items });
        }
        if let Some(d) = v["defaults"].as_object() {
            for (k, val) in d {
                t.defaults.insert(k.clone(), crate::model::jstr(val));
            }
        }
        Ok(t)
    }

    fn is_global(&self, key: &str) -> bool {
        self.categories.iter().filter(|c| GLOBAL_CATEGORIES.contains(&c.name.as_str())).any(|c| c.items.iter().any(|i| i.key.as_deref() == Some(key)))
    }
}

struct Ctx<'a> {
    t: &'a SettingsTable,
    m: &'a EfbModel,
}

impl Ctx<'_> {
    fn raw(&self, key: &str) -> String {
        self.m.settings.get(key).or_else(|| self.t.defaults.get(key)).cloned().unwrap_or_default()
    }

    fn find(&self, key: &str) -> Option<&Item> {
        self.t.categories.iter().flat_map(|c| c.items.iter()).find(|i| i.key.as_deref() == Some(key))
    }

    fn control_value(&self, it: &Item) -> String {
        let v = it.key.as_deref().map(|k| self.raw(k)).unwrap_or_default();
        if let Some(b) = &it.bool {
            let on = b.split(',').next().unwrap_or("");
            return if v == on { b.split(',').next().unwrap_or("").to_string() } else { b.split(',').nth(1).unwrap_or("").to_string() };
        }
        if it.seg.is_some() || it.sel.is_some() {
            let opts = it.seg.as_ref().or(it.sel.as_ref()).unwrap();
            return if opts.iter().any(|(o, _)| *o == v) { v } else { String::new() };
        }
        if it.callouts || it.zoom {
            return v;
        }
        if let Some([lo, hi, step]) = it.range {
            return match v.trim().parse::<f64>() {
                Ok(x) if x.is_finite() => trim_num(clamp_step(x, lo, hi, step)),
                _ => trim_num(clamp_step(lo + (hi - lo) / 2.0, lo, hi, step)),
            };
        }
        if let Some([lo, hi, _]) = it.num {
            return match v.trim().parse::<f64>() {
                Ok(x) if x.is_finite() && (lo..=hi).contains(&x) => v,
                Ok(x) if x.is_finite() => trim_num(x.clamp(lo, hi)),
                _ => String::new(),
            };
        }
        v
    }

    fn current(&self, key: &str) -> String {
        match self.find(key) {
            Some(it) if it.bool.is_none() && it.seg.is_none() && it.sel.is_none() && it.range.is_none() && it.num.is_none() && it.text.is_none() && !it.callouts && !it.zoom && it.key.is_some() => {
                let v = self.raw(key);
                (v == "true").to_string()
            }
            Some(it) => self.control_value(it),
            None => self.raw(key),
        }
    }

    fn visible(&self, it: &Item) -> bool {
        let Some(rule) = &it.show else { return true };
        rule.split('&').all(|r| {
            let (key, want) = r.split_once('=').unwrap_or((r, ""));
            let (key, not) = match key.strip_suffix('!') {
                Some(k) => (k, true),
                None => (key, false),
            };
            (self.current(key) == want) != not
        })
    }

    fn badge(&self, it: &Item) -> (String, String) {
        let key = it.key.as_deref().unwrap_or("");
        let v = self.current(key);
        let changed = v != self.t.defaults.get(key).cloned().unwrap_or_default();
        let kind = if let Some(lk) = &it.livery_key {
            let lv = self.m.settings.get(lk).cloned().unwrap_or_default().trim().to_string();
            if !v.trim().is_empty() {
                "user"
            } else if !lv.is_empty() {
                "livery"
            } else {
                "default"
            }
        } else if self.m.session_keys.iter().any(|k| k == key) {
            "session"
        } else if self.m.livery_values.get(key).map_or(false, |lv| *lv == v) {
            "livery"
        } else if changed {
            if self.t.is_global(key) { "global" } else { "user" }
        } else {
            "default"
        };
        (kind.to_uppercase(), kind.to_string())
    }
}

fn trim_num(x: f64) -> String {
    let x = (x * 1e9).round() / 1e9;
    if x.fract() == 0.0 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}

fn decimals(x: f64) -> i32 {
    let s = format!("{x}");
    s.split_once('.').map_or(0, |(_, f)| f.len() as i32).min(9)
}

pub fn clamp_step(x: f64, lo: f64, hi: f64, step: f64) -> f64 {
    let c = x.clamp(lo, hi);
    if !(step > 0.0) {
        return c;
    }
    let p = 10f64.powi(decimals(lo).max(decimals(step)));
    let snap = |q: f64| ((lo + q * step) * p).round() / p;
    let v = snap(((c - lo) / step).round());
    if v > hi {
        snap(((hi - lo) / step).floor())
    } else {
        v
    }
}

impl Item {
    pub fn normalize(&self, text: &str) -> Option<String> {
        let [lo, hi, step] = self.num.or(self.range)?;
        let x: f64 = text.trim().parse().ok().filter(|x: &f64| x.is_finite())?;
        Some(trim_num(clamp_step(x, lo, hi, step)))
    }
}

fn input(kind: &str, classes: &str, value: Option<&str>) -> Node {
    let mut n = Node::el("input", classes);
    let _ = kind;
    if let Some(v) = value {
        n = n.value(v);
    }
    n
}

fn control(c: &Ctx, it: &Item) -> Node {
    control_inner(c, it)
}

fn with_id(n: Node, it: &Item) -> Node {
    match &it.id {
        Some(id) => n.id(id),
        None => n,
    }
}

fn control_inner(c: &Ctx, it: &Item) -> Node {
    let val = c.control_value(it);
    if it.zoom {
        return Node::el("select", "st-sel").id("efb-zoom");
    }
    if let Some(action) = &it.action {
        let _ = action;
        let mut left = Node::el("span", "label dim");
        if let Some(id) = &it.status_id {
            left = left.id(id);
        }
        return Node::el("div", "st-inline").child(left).child(button("small", it.button.as_deref().unwrap_or("")));
    }
    if it.reset {
        return Node::el("div", "st-inline").child(button("small grey", "Reset state")).child(button("small", "New airframe"));
    }
    if it.callouts {
        let flags: u32 = val.parse().unwrap_or(0);
        let mut grid = Node::el("div", "st-chips");
        for (name, bit) in CALLOUTS {
            grid = grid.child(Node::el("button", "st-chip").class_if("on", flags & bit != 0).text(name));
        }
        return Node::el("div", "st-callouts").child(input("hidden", "", Some(&val))).child(grid).child(button("small grey", "Airbus basic"));
    }
    if it.seg.is_some() || it.sel.as_ref().map_or(false, |s| s.len() <= 3) {
        let opts = it.seg.as_ref().or(it.sel.as_ref()).unwrap();
        let mut seg = Node::el("div", "st-seg");
        for (v, t) in opts {
            seg = seg.child(Node::el("button", "").class_if("on", *v == val).text(t));
        }
        return Node::new("div").child(with_id(Node::el("select", "st-hidden"), it)).child(seg);
    }
    if it.sel.is_some() {
        return with_id(Node::el("select", "st-sel"), it);
    }
    if it.range.is_some() {
        let unit = it.unit.as_deref().unwrap_or("");
        return Node::el("div", "st-inline").child(with_id(input("range", "", Some(&val)), it)).child(span("mono st-val", &format!("{val}{unit}")));
    }
    if it.num.is_some() {
        return Node::el("div", "st-inline").child(with_id(input("number", "st-num", Some(&val)), it)).child(span("label dim", it.unit.as_deref().unwrap_or("")));
    }
    if it.text.is_some() {
        return with_id(input("text", "st-text", Some(&val)), it);
    }
    let checked = match &it.bool {
        Some(b) => val == b.split(',').next().unwrap_or(""),
        None => c.raw(it.key.as_deref().unwrap_or("")) == "true",
    };
    let _ = checked;
    Node::el("label", "st-switch").child(with_id(Node::new("input"), it)).child(Node::new("span"))
}

fn row(c: &Ctx, it: &Item, cat: &str, ci: usize, active: bool, query: &str) -> Node {
    let mut name = Node::el("span", "st-name").text(&it.label);
    if let Some(id) = &it.label_id {
        name = name.id(id);
    }
    let mut lab = Node::el("div", "st-label").child(name);
    if it.key.is_some() {
        let (text, kind) = c.badge(it);
        lab = lab.child(Node::el("span", &format!("st-badge {kind}")).text(&text));
    }
    let mut copy = Node::el("div", "st-copy").child(lab);
    if let Some(d) = &it.desc {
        copy = copy.child(label("st-desc", d));
    }
    let tail = if it.key.is_some() { button("st-kebab", "\u{22ee}") } else { Node::el("span", "st-kebab-gap") };
    let ctl = Node::el("div", "st-ctl").child(control(c, it)).child(tail);
    let _ = ci;
    let off = if query.is_empty() {
        !active
    } else {
        let find = format!("{} {} {}", it.label, it.desc.clone().unwrap_or_default(), cat).to_lowercase();
        !find.contains(query)
    };
    let mut r = Node::el("div", "st-row").class_if("st-wide", it.callouts).class_if("st-off", off).child(copy).child(ctl);
    if !c.visible(it) {
        r = r.style("display: none;");
    }
    r
}

pub fn settings_page(t: &SettingsTable, m: &EfbModel) -> Node {
    let c = Ctx { t, m };
    let q = m.search.trim().to_lowercase();
    let selected = m.category.min(t.categories.len().saturating_sub(1));
    let mut nav = Node::el("div", "st-cats");
    for (ci, cat) in t.categories.iter().enumerate() {
        nav = nav.child(Node::el("button", "st-cat").class_if("active", q.is_empty() && ci == selected).child(Node::new("span").text(&cat.name)));
    }
    let mut list = Node::el("div", "st-list").class_if("compact", m.compact);
    for (ci, cat) in t.categories.iter().enumerate() {
        for it in &cat.items {
            list = list.child(row(&c, it, &cat.name, ci, ci == selected, &q));
        }
    }
    let side = Node::el("div", "st-side")
        .child(label("st-h1", "Settings"))
        .child(nav)
        .child(Node::el("div", "st-spacer"))
        .child(Node::el("div", "st-rule"))
        .child(Node::el("button", "st-sidebtn").child(Node::el("span", "st-ico").child(svg_icon())).child(Node::new("span").text("Restart EFB")))
        .child(Node::el("button", "st-sidebtn").child(Node::el("span", "st-ico").child(svg_icon())).child(Node::new("span").text("Sleep EFB")))
        .child(label("st-ver", "A380 for X-Plane 12"));
    let top = Node::el("div", "st-top")
        .child(
            Node::el("div", "st-searchbox")
                .child(Node::el("span", "st-sico").child(svg_icon()))
                .child(input("text", "st-search", Some(&m.search)))
                .child(button("st-clear", "\u{d7}")),
        )
        .child(
            Node::el("div", "st-views")
                .child(Node::el("button", "st-view").class_if("on", !m.compact).child(Node::el("span", "st-ico").child(svg_icon())))
                .child(Node::el("button", "st-view").class_if("on", m.compact).child(Node::el("span", "st-ico").child(svg_icon()))),
        );
    let main = Node::el("div", "st-main").child(top).child(Node::el("div", "st-head")).child(list);
    Node::el("div", "page-main").id("settings-content").child(Node::el("div", "st-wrap st-fenix").child(side).child(main))
}

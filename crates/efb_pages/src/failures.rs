use crate::js::{el, js_fixed, locale_cmp, round};
use crate::{button, label, span, Node};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Failure {
    pub id: u64,
    pub name: String,
    pub chapter: String,
    pub cause: Option<String>,
    pub active: bool,
    pub magnitude: f64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct FailUnit {
    pub unit: String,
    pub id: u64,
    pub name: String,
    pub active: bool,
    pub magnitude: f64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct FailFamily {
    pub key: String,
    pub chapter: String,
    pub title: String,
    pub modes: Vec<(String, Vec<FailUnit>)>,
}

impl FailFamily {
    pub fn units(&self) -> impl Iterator<Item = &FailUnit> {
        self.modes.iter().flat_map(|(_, us)| us.iter())
    }
    fn mode_mut(&mut self, m: &str) -> &mut Vec<FailUnit> {
        if let Some(i) = self.modes.iter().position(|(k, _)| k == m) {
            return &mut self.modes[i].1;
        }
        self.modes.push((m.to_string(), Vec::new()));
        &mut self.modes.last_mut().unwrap().1
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Pending {
    pub id: u64,
    pub name: String,
    pub when: String,
    pub value: serde_json::Value,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct FailuresModel {
    pub families: Vec<FailFamily>,
    pub list: Vec<Failure>,
    pub armed_total: Option<usize>,
    pub scheduled: usize,
    pub pending: Vec<Pending>,
}

impl FailuresModel {
    /// From the plugin's `/study/failures` list, grouped as the page groups it.
    pub fn from_list(list: Vec<Failure>, pending: Vec<Pending>) -> Self {
        let families = families(&list);
        Self { families, scheduled: pending.len(), list, armed_total: None, pending }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct FailSel {
    pub mode: String,
    pub units: BTreeSet<String>,
    pub sev: f64,
}

impl Default for FailSel {
    fn default() -> Self {
        Self { mode: String::new(), units: BTreeSet::new(), sev: 1.0 }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Schedule {
    pub open: bool,
    pub pick: String,
    pub when: String,
    pub value: String,
}

impl Default for Schedule {
    fn default() -> Self {
        Self { open: false, pick: String::new(), when: "phase".into(), value: "Cruise".into() }
    }
}

/// `scroll_top` is relative to the first row of the list; `viewport` None builds every row (the DOM the JS produced).
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct FailuresView {
    pub chapter: String,
    pub query: String,
    pub armed_only: bool,
    pub scroll_top: f64,
    pub viewport: Option<f64>,
    pub row_h: f64,
    pub open: BTreeSet<String>,
    pub sel: BTreeMap<String, FailSel>,
    pub schedule: Schedule,
}

impl Default for FailuresView {
    fn default() -> Self {
        Self {
            chapter: String::new(),
            query: String::new(),
            armed_only: false,
            scroll_top: 0.0,
            viewport: None,
            row_h: ROW_H,
            open: BTreeSet::new(),
            sel: BTreeMap::new(),
            schedule: Schedule::default(),
        }
    }
}

/// Row pitch of a closed `div.ff` in the reference capture (39 px box plus the sub-pixel border pitch).
pub const ROW_H: f64 = 39.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub first: usize,
    pub end: usize,
    pub n: usize,
}

pub fn window(n: usize, scroll_top: f64, viewport: Option<f64>, h: f64) -> Window {
    let Some(vh) = viewport else {
        return Window { first: 0, end: n, n };
    };
    if !(h > 0.0) {
        return Window { first: 0, end: n, n };
    }
    let max_top = (n as f64 * h - vh).max(0.0);
    let top = scroll_top.clamp(0.0, max_top);
    let first = ((top / h).floor() as usize).min(n);
    let end = (first + (vh / h).ceil() as usize + 2).min(n);
    Window { first, end, n }
}

const FAIL_NAMES: &[(&str, &str)] = &[
    ("21|Avionics", "Avionics bay ventilation"), ("21|Cargo", "Cargo extraction"), ("21|Valves", "Cargo isolation valves"), ("21|Fans", "Cargo extraction fans"),
    ("21|Extract", "Avionics extract valves"), ("21|Extraction", "Avionics extraction fans"), ("21|Pack", "Air conditioning pack"), ("21|Pack FCV", "Pack flow control valve"),
    ("21|CAB FAN", "Cabin fan power"), ("21|Cabin Fan", "Cabin fan"), ("23|HF", "HF radio power"), ("23|VHF", "VHF radio"), ("23|HF transceiver", "HF transceiver"),
    ("24|AC", "AC bus"), ("24|DC", "DC bus"), ("24|DC HOT", "DC hot bus"), ("24|Bus", "Bus short circuit"), ("24|Contactor", "Line contactor"),
    ("24|GEN (VFG)", "Engine generator wear"), ("24|Generator", "Engine generator"), ("24|APU GEN", "APU generator wear"), ("24|APU Generator", "APU generator"),
    ("24|TR", "Transformer rectifier"), ("24|TR MONITORING", "TR monitoring"), ("24|BAT", "Battery"), ("24|ENMU", "Electrical network management unit"),
    ("25|Galley", "Galley equipment"), ("25|UPPER GALLEY", "Upper deck galley power"), ("26|Avionics", "Avionics bay fire"), ("26|CARGO", "Cargo fire protection"),
    ("26|ENG", "Engine fire source"), ("26|Engine", "Engine nacelle fire"), ("26|Engine Loop", "Engine fire detection loop"), ("26|APU Loop", "APU fire detection loop"),
    ("26|LDCR bottle", "Cargo fire bottle"), ("26|APU fire bottle squib failure", "APU fire bottle squib"), ("27|Inboard", "Droop nose drive"), ("27|Outboard", "Slat drive"),
    ("27|Inward", "Inner aileron"), ("27|Middle", "Middle aileron"), ("27|Outward", "Outer aileron"), ("27|Flap", "Flap drive"), ("28|Auxiliary", "Auxiliary refuel valve"),
    ("28|ENG", "Engine fuel feed"), ("28|FEED", "Feed tank structure and gauging"), ("28|Feed tank", "Feed tank"), ("28|Outer", "Outer tank"),
    ("28|Tank", "Inner and mid tank pumps and valves"), ("28|Transfer", "Transfer gallery"), ("28|Valves", "Fuel valves"), ("28|FUEL PUMP", "Fuel pump power"),
    ("28|FUEL VALVE", "Fuel valve power"), ("28|Jettison", "Fuel jettison"), ("28|Trim line", "Trim line valve"), ("28|Trim tank", "Trim tank pump"),
    ("29|Brakes", "Brake hydraulic line"), ("29|Cargo_doors", "Cargo door hydraulic line"), ("29|Circuit", "Hydraulic filter"), ("29|Elec", "Electric pump"),
    ("29|Engine pump", "Engine-driven pump overheat"), ("29|Gear", "Gear hydraulic line"), ("29|Local", "Local electric pump"), ("29|Priority", "Priority valve"),
    ("29|Relief", "Relief valve"), ("29|Return", "Return filter"), ("29|Reversers", "Reverser hydraulic line"), ("29|Steering", "Steering hydraulic line"),
    ("29|System", "Hydraulic system"), ("29|Fuel/hydraulic", "Fuel/hydraulic heat exchanger"), ("29|EDP", "Engine-driven pump"), ("30|ADIRU", "ADIRU probe heater"),
    ("30|ENG", "Engine anti-ice valve"), ("30|Engine", "Nacelle anti-ice"), ("30|WING", "Wing anti-ice"), ("30|WINDSHIELD", "Windshield"),
    ("30|WINDSHIELD HEAT", "Windshield heat power"), ("30|PITOT HEAT", "Pitot heat power"), ("30|AOA HEAT", "AOA heat power"), ("30|Probe", "Probe heat"),
    ("31|ND DU", "ND display unit"), ("31|PFD DU", "PFD display unit"), ("32|Body", "Body gear"), ("32|Body wheel", "Body gear wheel"), ("32|Wing wheel", "Wing gear wheel"),
    ("32|Gear", "Landing gear"), ("32|Brake", "Brake pedal"), ("32|GEAR ACTUATOR", "Gear actuator power"), ("34|ADIRU", "ADIRU air data"), ("34|RA SYS", "Radio altimeter"),
    ("34|FMS", "FMS power"), ("34|TRANSPONDER", "Transponder power"), ("34|Inertial Reference", "Inertial reference"), ("36|Cross", "Crossbleed valve"),
    ("36|Engine", "Engine bleed"), ("36|Pylon", "Pylon bleed duct"), ("36|ODLS loop", "Overheat detection loop"), ("38|Zone", "Lavatory and galley water"),
    ("38|Drain", "Drain mast"), ("42|Babbling", "Babbling module"), ("42|Configuration", "Configuration table"), ("42|Hardware", "Module hardware"),
    ("42|Loss", "Module power"), ("42|Partition", "Partition IO driver"), ("44|Zone", "Cabin seat power"), ("44|Smoke", "Cabin smoke detector"),
    ("49|APU generator", "APU generator bearings"), ("49|APU fire bottle squib failure", "APU extinguisher squib"), ("26|Fire - Engine", "Engine fire"),
    ("54|Engine", "Engine and nacelle separation"), ("55|Elevator", "Elevator separation"), ("55|Rudder", "Rudder separation"), ("55|Horizontal", "Horizontal stabiliser separation"),
    ("57|Aileron", "Aileron separation"), ("57|Outer", "Outer wing separation"), ("57|Spoiler", "Spoiler separation"), ("71|Engine", "Nacelle ventilation"),
    ("72|Engine", "Engine core"), ("73|Engine", "Engine fuel control"), ("73|Engine EEC", "EEC sensors"), ("73|FADEC CHANNEL", "FADEC channel power"),
    ("74|Engine", "Ignition"), ("74|IGNITION EXCITER", "Ignition exciter"), ("75|Engine", "Handling bleed valves"), ("76|Engine", "Thrust lever"),
    ("77|Engine", "Engine indicating"), ("78|Engine", "Thrust reverser"), ("78|Engine reverser", "Reverser locks"), ("79|Engine", "Engine oil"), ("80|Engine", "Engine starter"),
];

pub const FAIL_SEVERITY: [(&str, f64); 4] = [("Minor", 0.25), ("Moderate", 0.5), ("Severe", 0.75), ("Total", 1.0)];
const UNIT_WORDS: &[&str] = &[
    "left", "right", "l", "r", "lh", "rh", "inner", "outer", "inward", "outward", "inboard", "outboard", "mid", "middle", "upper", "lower", "forward", "fwd", "aft", "centre",
    "center", "green", "yellow", "captain", "capt", "fo", "nose", "body", "wing", "ess", "apu", "primary", "secondary", "standby", "main", "norm", "a", "b", "c", "d", "e", "f",
];
const NOUNS: &[&str] = &[
    "valve", "pump", "sensor", "probe", "transmitter", "switch", "computer", "detector", "loop", "heater", "fan", "light", "actuator", "motor", "servo", "channel", "bus",
    "breaker", "contactor", "relay", "controller", "module", "display", "indicator", "duct", "line", "filter", "seal", "door", "tyre", "tire", "brake", "wheel", "bottle",
    "squib", "tank", "generator", "battery", "inverter", "transformer", "antenna", "receiver", "transceiver", "camera",
];
const SEPARATION: [(u64, u64, &str); 11] = [
    (54000, 54003, "Engine"), (54010, 54013, "Inlet and fan cowl"), (57340, 57345, "Flap"), (57300, 57305, "Aileron"), (57310, 57325, "Spoiler"),
    (55310, 55313, "Elevator"), (55320, 55321, "Horizontal stabiliser"), (55300, 55301, "Rudder"), (55330, 55330, "Fin"), (57350, 57355, "Wing"), (57330, 57331, "Outer wing"),
];

fn tokens(n: &str) -> Vec<String> {
    let mut s = String::new();
    for c in n.chars() {
        if matches!(c, ':' | '(' | ')') {
            s.push(' ');
            s.push(c);
            s.push(' ');
        } else {
            s.push(c);
        }
    }
    s.split_whitespace().map(str::to_string).collect()
}

fn unit_token(w: &str) -> bool {
    w.bytes().any(|b| b.is_ascii_digit()) || w.contains('_') || UNIT_WORDS.iter().any(|u| u.eq_ignore_ascii_case(w))
}

fn clean(a: &[String]) -> String {
    let j = a.iter().filter(|x| *x != "*").cloned().collect::<Vec<_>>().join(" ");
    let j = j.replace("( ", "(").replace(" )", ")").replace("()", "");
    j.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn split(tt: &[String]) -> (String, String) {
    let (fam, mode): (Vec<String>, Vec<String>) = if let Some(colon) = tt.iter().position(|t| t == ":") {
        (tt[..colon].to_vec(), tt[colon + 1..].to_vec())
    } else {
        match tt.iter().rposition(|t| t == "*") {
            None => (tt.to_vec(), vec![]),
            Some(last) if last == tt.len() - 1 => (tt.to_vec(), vec![]),
            Some(last) if tt[..=last].iter().all(|x| x == "*") => (vec![tt[last + 1].clone()], tt[last + 2..].to_vec()),
            Some(last) => (tt[..=last].to_vec(), tt[last + 1..].to_vec()),
        }
    };
    let f = clean(&fam);
    (if f.is_empty() { clean(tt) } else { f }, clean(&mode))
}

fn norm_name(n: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in n.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if gap && !out.is_empty() {
                out.push(' ');
            }
            gap = false;
            out.push(c);
        } else {
            gap = true;
        }
    }
    out
}

fn unrepeat(s: &str) -> String {
    let t: Vec<&str> = s.split(' ').collect();
    let mut len = t.len() / 2;
    while len >= 1 {
        let mut i = 0;
        while i + 2 * len <= t.len() {
            if t[i..i + len].join(" ").to_lowercase() == t[i + len..i + 2 * len].join(" ").to_lowercase() {
                let next: Vec<&str> = t[..i + len].iter().chain(t[i + 2 * len..].iter()).copied().collect();
                return unrepeat(&next.join(" "));
            }
            i += 1;
        }
        len -= 1;
    }
    s.to_string()
}

pub fn title_case(s: &str) -> String {
    let s = unrepeat(s);
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => s,
    }
}

pub fn parse_int(s: &str) -> Option<i64> {
    let t = s.trim_start();
    let (neg, t) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let d: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    let v: i64 = d.parse().ok()?;
    Some(if neg { -v } else { v })
}

fn sep_kind(f: &Failure) -> Option<&'static str> {
    SEPARATION.iter().find(|(a, b, _)| f.id >= *a && f.id <= *b).map(|x| x.2)
}

fn strip_word_ci(n: &str, word: &str) -> String {
    let lower = n.to_ascii_lowercase();
    let Some(p) = lower.find(word) else { return n.to_string() };
    let mut start = p;
    while start > 0 && n.as_bytes()[start - 1].is_ascii_whitespace() {
        start -= 1;
    }
    format!("{}{}", &n[..start], &n[p + word.len()..])
}

fn sep_unit(kind: &str, name: &str) -> String {
    let mut n = name.to_string();
    if n.to_ascii_lowercase().ends_with("separation") {
        n.truncate(n.len() - "separation".len());
        n = n.trim_end().to_string();
    }
    match kind {
        "Engine" | "Inlet and fan cowl" => n.chars().find(char::is_ascii_digit).map(|c| c.to_string()).unwrap_or(n),
        "Fin" => "Fin".into(),
        _ => {
            let t = title_case(strip_word_ci(&n, &kind.to_ascii_lowercase()).trim());
            if t.is_empty() { n } else { t }
        }
    }
}

fn unit(f: &Failure, unit: &str) -> FailUnit {
    FailUnit { unit: unit.into(), id: f.id, name: f.name.clone(), active: f.active, magnitude: f.magnitude }
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn noun_matches(name: &str) -> Vec<String> {
    let low = name.to_ascii_lowercase();
    let b = low.as_bytes();
    let boundary = |i: usize| {
        let l = i > 0 && is_word(b[i - 1]);
        let r = i < b.len() && is_word(b[i]);
        l != r
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let mut hit = None;
        for w in NOUNS {
            if low[i..].starts_with(w) {
                let e = i + w.len();
                if e < b.len() && b[e] == b's' && boundary(e + 1) {
                    hit = Some(e + 1);
                } else if boundary(e) {
                    hit = Some(e);
                }
                if hit.is_some() {
                    break;
                }
            }
        }
        match hit {
            Some(e) => {
                out.push(name[i..e].to_string());
                i = e;
            }
            None => i += 1,
        }
    }
    out
}

fn strip_article(s: &str) -> String {
    let t = s.trim_start();
    for a in ["the", "an", "a"] {
        if t.len() > a.len() && t[..a.len()].eq_ignore_ascii_case(a) && t[a.len()..].starts_with(char::is_whitespace) {
            return t[a.len()..].trim_start().to_string();
        }
    }
    s.to_string()
}

/// `failFamilies`: groups the flat failure list into families with modes and units.
pub fn families(list: &[Failure]) -> Vec<FailFamily> {
    let regular: BTreeSet<String> = list.iter().filter(|f| f.id < 1_000_000).map(|f| norm_name(&f.name)).collect();
    let all: Vec<&Failure> = list.iter().filter(|f| !(f.id >= 1_000_000 && regular.contains(&norm_name(&f.name)))).collect();
    let sep: Vec<&Failure> = all.iter().copied().filter(|f| sep_kind(f).is_some()).collect();
    let separation = (!sep.is_empty()).then(|| {
        let mut fam = FailFamily { key: "part-separation".into(), chapter: sep[0].chapter.clone(), title: "Part separation".into(), modes: vec![] };
        for (_, _, kind) in SEPARATION {
            let mut members: Vec<&Failure> = sep.iter().copied().filter(|f| sep_kind(f) == Some(kind)).collect();
            members.sort_by_key(|f| f.id);
            if !members.is_empty() {
                fam.modes.push((kind.into(), members.iter().map(|f| unit(f, &sep_unit(kind, &f.name))).collect()));
            }
        }
        fam
    });
    let items: Vec<&Failure> = all.iter().copied().filter(|f| sep_kind(f).is_none()).collect();
    struct Opt {
        key: String,
        tpl: Vec<String>,
        unit: String,
        n: usize,
    }
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut options: Vec<Vec<Opt>> = Vec::with_capacity(items.len());
    for f in &items {
        let t = tokens(&f.name);
        let slots: Vec<usize> = (0..t.len()).filter(|&i| unit_token(&t[i])).collect();
        let mut masks: Vec<Vec<usize>> = Vec::new();
        for a in 0..slots.len() {
            masks.push(vec![slots[a]]);
            for b in a + 1..slots.len() {
                masks.push(vec![slots[a], slots[b]]);
            }
        }
        let opts: Vec<Opt> = masks
            .iter()
            .map(|m| {
                let mut c = t.clone();
                for &i in m {
                    c[i] = "*".into();
                }
                Opt { key: format!("{}|{}", f.chapter, c.join(" ")), tpl: c, unit: m.iter().map(|&i| t[i].clone()).collect::<Vec<_>>().join(" "), n: m.len() }
            })
            .collect();
        for o in &opts {
            *counts.entry(o.key.clone()).or_default() += 1;
        }
        options.push(opts);
    }
    let mut fams: Vec<FailFamily> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut add = |f: &Failure, fam_name: &str, mode: &str, u: &str| {
        let key = format!("{}|{}", f.chapter, fam_name.to_lowercase());
        let i = *index.entry(key.clone()).or_insert_with(|| {
            let title = title_case(fam_name);
            let look = parse_int(&f.chapter).map(|c| format!("{c}|{title}"));
            let named = look.and_then(|k| FAIL_NAMES.iter().find(|(a, _)| *a == k).map(|(_, b)| b.to_string()));
            fams.push(FailFamily { key, chapter: f.chapter.clone(), title: named.unwrap_or(title), modes: vec![] });
            fams.len() - 1
        });
        let m = if mode.is_empty() { "Failure" } else { mode };
        fams[i].mode_mut(m).push(unit(f, u));
    };
    let mut loose: Vec<&Failure> = Vec::new();
    for (f, opts) in items.iter().zip(&options) {
        let mut best: Option<(&Opt, usize)> = None;
        for o in opts {
            let c = counts[&o.key];
            if c < 2 {
                continue;
            }
            if best.map_or(true, |(bo, bc)| c > bc || (c == bc && o.n < bo.n)) {
                best = Some((o, c));
            }
        }
        match best {
            None => loose.push(f),
            Some((o, _)) => {
                let (fam_name, mode) = split(&o.tpl);
                add(f, &fam_name, &title_case(&mode), &o.unit);
            }
        }
    }
    let mut by_cause: Vec<(String, Vec<(&Failure, String, String)>)> = Vec::new();
    for f in loose {
        let cause = f.cause.clone().unwrap_or_default().replace(&f.name, "{X}").replace(&f.name.to_lowercase(), "{X}");
        let nouns = noun_matches(&f.name);
        let key = (cause.contains("{X}") && !nouns.is_empty()).then(|| format!("{}|{}|{}", f.chapter, nouns.last().unwrap().to_lowercase(), cause.to_lowercase()));
        let Some(key) = key else {
            add(f, &f.name, "", &f.name);
            continue;
        };
        let noun = nouns.last().unwrap().to_lowercase();
        match by_cause.iter_mut().find(|(k, _)| *k == key) {
            Some((_, g)) => g.push((f, cause, noun)),
            None => by_cause.push((key, vec![(f, cause, noun)])),
        }
    }
    for (_, group) in &by_cause {
        if group.len() < 2 {
            for (f, _, _) in group {
                add(f, &f.name, "", &f.name);
            }
            continue;
        }
        for (f, cause, noun) in group {
            let m = strip_article(&cause.replacen("{X}", "", 1));
            let m = m.split([';', ':', '.']).next().unwrap_or("");
            let m = strip_article(m);
            add(f, &format!("{noun}s"), &title_case(m.trim()), &f.name);
        }
    }
    drop(add);
    if let Some(s) = separation {
        fams.push(s);
    }
    fams.sort_by(|a, b| locale_cmp(&a.title, &b.title));
    fams
}

fn chapter_name(chapter: &str) -> String {
    let s = chapter.trim_start_matches(|c: char| c.is_ascii_digit()).trim_start();
    let s = if chapter.starts_with(|c: char| c.is_ascii_digit()) { s } else { chapter };
    if s.len() >= 6 && s[..6].eq_ignore_ascii_case("engine") && s[6..].starts_with(char::is_whitespace) {
        s[6..].trim_start().to_string()
    } else {
        s.to_string()
    }
}

fn chips(lbl: &str, entries: Vec<(String, bool, &str, String)>) -> Node {
    el("div", "ff-line").child(span("ff-label", lbl)).child(
        el("div", "ff-chips").children(entries.into_iter().map(|(text, on, extra, act)| Node::el("button", &format!("ff-chip{extra}")).class_if("on", on).attr("type", "button").act(&act).text(&text))),
    )
}

pub type DiagramFn<'a> = dyn Fn(&FailFamily, &[&FailUnit], &[&FailUnit], f64) -> Option<Node> + 'a;

fn family_body(fam: &FailFamily, sel0: Option<&FailSel>, diagram: &DiagramFn) -> Node {
    let all: Vec<&FailUnit> = fam.units().collect();
    let first_mode = fam.modes.first().map(|m| m.0.clone()).unwrap_or_default();
    let mut sel = sel0.cloned().unwrap_or_else(|| FailSel { mode: first_mode.clone(), ..Default::default() });
    if !fam.modes.iter().any(|(m, _)| *m == sel.mode) {
        sel.mode = first_mode;
    }
    let members: Vec<&FailUnit> = fam.modes.iter().find(|(m, _)| *m == sel.mode).map(|(_, us)| us.iter().collect()).unwrap_or_default();
    if members.len() == 1 {
        sel.units = [members[0].unit.clone()].into_iter().collect();
    }
    let k = &fam.key;
    let mut body = el("div", "ff-body");
    if fam.modes.len() > 1 {
        body = body.child(chips("Mode", fam.modes.iter().map(|(m, _)| (m.clone(), *m == sel.mode, "", format!("fail:mode:{k}:{m}"))).collect()));
    }
    if members.len() > 1 {
        let all_on = members.iter().all(|u| sel.units.contains(&u.unit));
        let mut e = vec![("All".to_string(), all_on, "", format!("fail:units:{k}"))];
        e.extend(members.iter().map(|u| (u.unit.clone(), sel.units.contains(&u.unit), if u.active { " live" } else { "" }, format!("fail:unit:{k}:{}", u.unit))));
        body = body.child(chips("Unit", e));
    }
    body = body.child(chips("Severity", FAIL_SEVERITY.iter().map(|(t, v)| (t.to_string(), sel.sev == *v, "", format!("fail:sev:{k}:{v}"))).collect()));
    let chosen: Vec<&FailUnit> = members.iter().copied().filter(|u| sel.units.contains(&u.unit)).collect();
    body = body.child(
        el("div", "ff-line ff-actions").child(span("ff-label", "")).child(
            el("div", "ff-chips")
                .child(button("small", "Arm").attr("type", "button").attr_if("disabled", chosen.is_empty()).act(&format!("fail:arm:{k}")))
                .child(button("small grey", "Disarm").attr("type", "button").attr_if("disabled", !chosen.iter().any(|u| u.active)).act(&format!("fail:disarm:{k}"))),
        ),
    );
    if let Some(d) = diagram(fam, &members, &chosen, sel.sev) {
        body = body.child(el("div", "ff-diagram").child(d));
    }
    if all.iter().any(|u| u.active) {
        let mut list = el("div", "ff-live");
        for (mode, ms) in &fam.modes {
            for u in ms.iter().filter(|u| u.active) {
                let sev = if u.magnitude > 0.0 && u.magnitude < 1.0 { format!(" \u{b7} {}%", round(u.magnitude * 100.0)) } else { String::new() };
                let parts: Vec<&str> = [if fam.modes.len() > 1 { mode.as_str() } else { "" }, if all.len() > 1 && u.unit != fam.title { u.unit.as_str() } else { "" }].into_iter().filter(|s| !s.is_empty()).collect();
                let l = if parts.is_empty() { fam.title.clone() } else { parts.join(" \u{b7} ") };
                list = list.child(
                    el("div", "ff-live-row")
                        .child(Node::new("span").text(&format!("{l}{sev}")))
                        .child(button("ff-x", "\u{d7}").attr("type", "button").attr("title", "Disarm").act(&format!("fail:off:{}", u.id))),
                );
            }
        }
        body = body.child(list);
    }
    body
}

fn family_row(fam: &FailFamily, dup: &BTreeSet<String>, v: &FailuresView, diagram: &DiagramFn) -> Node {
    let all: Vec<&FailUnit> = fam.units().collect();
    let armed = all.iter().filter(|u| u.active).count();
    let open = v.open.contains(&fam.key);
    let units: BTreeSet<&str> = all.iter().map(|u| u.unit.as_str()).collect();
    let meta = if all.len() == 1 {
        String::new()
    } else {
        let mut parts = Vec::new();
        if fam.modes.len() > 1 {
            parts.push(format!("{} modes", fam.modes.len()));
        }
        if units.len() > 1 {
            parts.push(format!("{} units", units.len()));
        }
        parts.join(" \u{b7} ")
    };
    let title = if dup.contains(&fam.title.to_lowercase()) { format!("{} \u{b7} {}", fam.title, chapter_name(&fam.chapter)) } else { fam.title.clone() };
    let armed_text = if armed > 0 { format!("{armed} armed") } else { String::new() };
    let mut row = Node::el("div", "ff").class_if("open", open).class_if("has-armed", armed > 0).child(
        Node::el("div", "ff-head")
            .act(&format!("fail:open:{}", fam.key))
            .child(span("ff-title", &title))
            .child(span("ff-meta", &meta))
            .child(span("ff-armed", &armed_text))
            .child(span("ff-chev", if open { "\u{2212}" } else { "+" })),
    );
    if open {
        row = row.child(family_body(fam, v.sel.get(&fam.key), diagram));
    }
    row
}

pub fn shown<'a>(m: &'a FailuresModel, v: &FailuresView) -> Vec<&'a FailFamily> {
    let q = v.query.trim().to_lowercase();
    m.families
        .iter()
        .filter(|f| v.chapter.is_empty() || f.chapter == v.chapter)
        .filter(|f| !v.armed_only || f.units().any(|u| u.active))
        .filter(|f| q.is_empty() || f.title.to_lowercase().contains(&q) || f.units().any(|u| format!("{} {}", u.id, u.name).to_lowercase().contains(&q)))
        .collect()
}

pub const FS_WHEN: [(&str, &str); 5] = [("phase", "On flight phase"), ("aboveSpeed", "Above IAS (kt)"), ("aboveAlt", "Climbing through (ft)"), ("belowAlt", "Descending through (ft)"), ("minutes", "After (minutes)")];
pub const FS_PHASES: [(&str, &str); 7] = [("Taxi", "Taxi"), ("Takeoff", "Take-off"), ("Climb", "Climb"), ("Cruise", "Cruise"), ("Descent", "Descent"), ("Approach", "Approach"), ("GoAround", "Go-around")];
pub const FS_SCENARIOS: [(&str, &[(&str, &str, &str)]); 5] = [
    ("Tyre burst on the take-off roll", &[("Left wing gear tyre burst", "aboveSpeed", "110")]),
    ("Bird strike in the initial climb", &[("Engine 2 N1 bird-strike imbalance", "aboveSpeed", "175")]),
    ("Generator 1 lost in the climb", &[("Generator 1", "phase", "Climb")]),
    ("Bleed leak in cruise", &[("Engine 1 bleed duct leak", "phase", "Cruise")]),
    ("Rapid decompression in cruise", &[("Rapid Decompression", "phase", "Cruise")]),
];

fn js_value_num(v: &serde_json::Value) -> f64 {
    match v {
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        serde_json::Value::String(_) => crate::flight::js_number(v),
        serde_json::Value::Bool(b) => f64::from(u8::from(*b)),
        serde_json::Value::Null => 0.0,
        _ => f64::NAN,
    }
}

pub fn fs_label(when: &str, value: &serde_json::Value) -> String {
    let n = js_value_num(value);
    match when {
        "phase" => {
            let s = crate::model::jstr(value);
            let l = FS_PHASES.iter().find(|(k, _)| *k == s).map(|(_, l)| l.to_string()).unwrap_or(s);
            format!("on {}", l.to_lowercase())
        }
        "aboveSpeed" => format!("above {} kt", crate::js::num(round(n))),
        "aboveAlt" => format!("climbing through {} ft", crate::js::num(round(n))),
        "belowAlt" => format!("descending through {} ft", crate::js::num(round(n))),
        _ => format!("at {} airframe hours", if n.is_finite() { js_fixed(n, 2) } else { "NaN".into() }),
    }
}

fn select(options: &[(&str, &str)], act: &str, value: &str) -> Node {
    crate::js::select(options, act, value)
}

fn schedule_panel(m: &FailuresModel, s: &Schedule) -> Node {
    let mut bx = el("div", "box fs-box").child(
        Node::el("button", "fs-head")
            .attr("type", "button")
            .act("fs:toggle")
            .child(span("section-title", &format!("Scheduled failures / {}", m.pending.len().max(m.scheduled))).style("margin:0"))
            .child(span("label dim", if s.open { "hide" } else { "schedule, scenarios" })),
    );
    for t in &m.pending {
        bx = bx.child(
            el("div", "fs-row")
                .child(span("fs-name", &t.name))
                .child(span("label dim", &fs_label(&t.when, &t.value)))
                .child(button("small grey", "Cancel").attr("type", "button").act(&format!("fs:cancel:{}", t.id))),
        );
    }
    if !s.open {
        return bx;
    }
    let names = Node::new("datalist").id("fs-names").children(m.list.iter().map(|f| Node::new("option").attr("value", &format!("{} [{}]", f.name, f.id))));
    let pick = Node::new("input").attr("type", "text").attr("list", "fs-names").attr("placeholder", "Failure (type to search)").value(&s.pick).act("fs:pick");
    let value = if s.when == "phase" {
        select(&FS_PHASES, "fs:value", if s.value.is_empty() { "Cruise" } else { &s.value })
    } else {
        let ph = match s.when.as_str() {
            "minutes" => "20",
            "aboveSpeed" => "120",
            _ => "10000",
        };
        Node::new("input").attr("type", "text").attr("placeholder", ph).value(&s.value).act("fs:value")
    };
    let form = el("div", "fs-form").child(pick).child(select(&FS_WHEN, "fs:when", &s.when)).child(value).child(button("small", "Schedule").attr("type", "button").act("fs:go"));
    let mut sc = el("div", "fs-scen");
    for (title, items) in FS_SCENARIOS {
        let found = items.iter().all(|(n, _, _)| m.list.iter().any(|f| f.name == *n));
        let state = if found {
            items
                .iter()
                .map(|(_, w, v)| fs_label(w, &if *w == "phase" { serde_json::Value::String(v.to_string()) } else { serde_json::json!(v.parse::<f64>().unwrap()) }))
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            "not in this failure list".into()
        };
        let tip = items.iter().map(|(n, _, _)| *n).collect::<Vec<_>>().join(", ");
        sc = sc.child(
            el("button", "tile")
                .attr("type", "button")
                .attr("title", &tip)
                .attr_if("disabled", !found)
                .act(&format!("fs:scenario:{title}"))
                .child(Node::new("span").text(title))
                .child(span("tile-state", &state)),
        );
    }
    bx.child(names)
        .child(form)
        .child(label("label dim", "Training scenarios: each arms its failures once, when its trigger comes. Scheduled failures last until they fire or X-Plane closes; cancel above to take one back.").style("margin:12px 0 6px"))
        .child(sc)
}

pub fn failures_nav(m: &FailuresModel, v: &FailuresView) -> Node {
    let btn = |key: &str, text: &str, fams: Vec<&FailFamily>| {
        let armed: usize = fams.iter().map(|f| f.units().filter(|u| u.active).count()).sum();
        Node::new("button")
            .attr("type", "button")
            .class_if("active", v.chapter == key)
            .act(&format!("fail:chapter:{key}"))
            .text(text)
            .child(Node::el("span", "count").class_if("armed", armed > 0).text(&if armed > 0 { format!("{armed} armed") } else { fams.len().to_string() }))
    };
    let chapters: BTreeSet<&str> = m.families.iter().map(|f| f.chapter.as_str()).collect();
    let mut nav = Node::el("nav", "study-nav").id("failures-nav").child(btn("", "All failures", m.families.iter().collect()));
    nav = nav.child(label("study-nav-group", "ATA chapters"));
    for c in chapters {
        nav = nav.child(btn(c, c, m.families.iter().filter(|f| f.chapter == c).collect()));
    }
    nav
}

pub fn failures_content(m: &FailuresModel, v: &FailuresView) -> Node {
    failures_content_with(m, v, &|_, _, _, _| None)
}

pub fn failures_content_with(m: &FailuresModel, v: &FailuresView, diagram: &DiagramFn) -> Node {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for f in &m.families {
        *seen.entry(f.title.to_lowercase()).or_default() += 1;
    }
    let dup: BTreeSet<String> = seen.into_iter().filter(|(_, n)| *n > 1).map(|(t, _)| t).collect();
    let list = shown(m, v);
    let armed = m.armed_total.unwrap_or_else(|| if m.list.is_empty() { m.families.iter().flat_map(|f| f.units()).filter(|u| u.active).count() } else { m.list.iter().filter(|f| f.active).count() });
    let chapter = if v.chapter.is_empty() { "All failures" } else { v.chapter.as_str() };
    let mut bx = el("div", "box ff-list");
    if list.is_empty() {
        bx = bx.child(span("label dim", "No failures match."));
    }
    let w = window(list.len(), v.scroll_top, v.viewport, v.row_h);
    let spacer = |rows: usize| Node::el("div", "spacer").style(&format!("height:{}px", rows as f64 * v.row_h));
    if v.viewport.is_some() {
        bx = bx.child(spacer(w.first));
    }
    bx = bx.children(list[w.first..w.end].iter().map(|f| family_row(f, &dup, v, diagram)));
    if v.viewport.is_some() {
        bx = bx.child(spacer(w.n - w.end));
    }
    Node::new("div")
        .id("failures-content")
        .child(schedule_panel(m, &v.schedule))
        .child(label("section-title", &format!("{chapter} / {} failures / {armed} armed", list.len())))
        .child(bx)
}

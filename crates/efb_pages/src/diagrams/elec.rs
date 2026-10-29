use super::{ctx, ex_t, ns_box, ns_elbow, ns_svg, ns_wire, B, NS_GREEN, NS_GREY_TEXT, NS_ORANGE, T, TD};
use crate::Node;
use std::collections::{BTreeSet, HashMap};

pub(crate) const ELN: [(&str, &str, f64, f64); 34] = [
    ("GEN1", "GEN 1", 50.0, 20.0), ("GEN2", "GEN 2", 230.0, 20.0), ("GEN3", "GEN 3", 650.0, 20.0), ("GEN4", "GEN 4", 830.0, 20.0),
    ("APUGENA", "APU GEN A", 380.0, 20.0), ("APUGENB", "APU GEN B", 500.0, 20.0),
    ("EXT1", "EXT 1", 50.0, 66.0), ("EXT2", "EXT 2", 230.0, 66.0), ("EXT3", "EXT 3", 650.0, 66.0), ("EXT4", "EXT 4", 830.0, 66.0),
    ("AC1", "AC 1", 50.0, 128.0), ("AC2", "AC 2", 230.0, 128.0), ("AC3", "AC 3", 650.0, 128.0), ("AC4", "AC 4", 830.0, 128.0),
    ("ACEHA", "AC EHA", 60.0, 300.0), ("ACESS", "AC ESS", 440.0, 238.0), ("ACESSSHED", "AC ESS SHED", 440.0, 300.0), ("ACEMER", "AC EMER", 600.0, 300.0), ("ACGND", "AC GND/FLT", 830.0, 238.0),
    ("EMERGEN", "EMER GEN", 600.0, 238.0),
    ("TR1", "TR 1", 230.0, 372.0), ("ESSTR", "ESS TR", 440.0, 372.0), ("TR2", "TR 2", 650.0, 372.0), ("APUTR", "APU TR", 830.0, 372.0), ("STATINV", "STAT INV", 520.0, 420.0),
    ("DC1", "DC 1", 140.0, 480.0), ("DCESS", "DC ESS", 380.0, 480.0), ("DC2", "DC 2", 650.0, 480.0), ("DCAPU", "DC APU", 830.0, 480.0), ("DCEHA", "DC EHA", 380.0, 590.0), ("DCGND", "DC GND/FLT", 520.0, 640.0),
    ("BAT1", "BAT 1", 20.0, 590.0), ("BATESS", "BAT ESS", 200.0, 590.0), ("BAT2", "BAT 2", 650.0, 590.0),
];
const ELN_BATAPU: (&str, &str, f64, f64) = ("BATAPU", "BAT APU", 830.0, 590.0);
const ELN_W: f64 = 120.0;
const ELN_H: f64 = 34.0;

pub(crate) fn eln() -> Vec<(&'static str, &'static str, f64, f64)> {
    let mut v = ELN.to_vec();
    v.push(ELN_BATAPU);
    v
}

fn pos(k: &str) -> Option<(&'static str, f64, f64)> {
    eln().into_iter().find(|e| e.0 == k).map(|e| (e.1, e.2, e.3))
}

const FEEDS: [(&str, &[&str]); 21] = [
    ("AC1", &["GEN1", "TIE"]), ("AC2", &["GEN2", "TIE"]), ("AC3", &["GEN3", "TIE"]), ("AC4", &["GEN4", "TIE"]),
    ("ACESS", &["AC1", "AC4", "EMERGEN"]), ("ACESSSHED", &["ACESS"]), ("ACEHA", &["AC3", "ACESS"]), ("ACEMER", &["ACESS", "STATINV"]), ("ACGND", &["AC3"]),
    ("TR1", &["AC2"]), ("TR2", &["AC3"]), ("ESSTR", &["ACESS"]), ("APUTR", &["AC4"]), ("STATINV", &["BATESS", "DCESS"]),
    ("DC1", &["TR1", "DC2", "BAT1"]), ("DC2", &["TR2", "DC1", "BAT2"]), ("DCESS", &["ESSTR", "DC1", "BATESS"]), ("DCAPU", &["APUTR", "BATAPU"]),
    ("DCEHA", &["DC2", "DCESS"]), ("DCGND", &["TR2"]), ("", &[]),
];

fn feeds() -> impl Iterator<Item = (&'static str, &'static [&'static str])> {
    FEEDS.iter().copied().filter(|(k, _)| !k.is_empty())
}

#[derive(Clone, Debug, Default)]
pub struct ElecT {
    pub dead: BTreeSet<String>,
    pub degraded: BTreeSet<String>,
    pub edge: super::OMap,
}

impl ElecT {
    pub fn is_empty(&self) -> bool {
        self.dead.is_empty() && self.degraded.is_empty() && self.edge.is_empty()
    }
}

pub fn elec_targets(name: &str) -> ElecT {
    let n = format!(" {} ", re!(r"[_-]").replace_all(&name.to_uppercase(), " "));
    let n = n.as_str();
    let mut out = ElecT::default();
    let soft = re!(r"DEGRAD|DRIFT|EFFICIENCY|MONITOR|WELDED|BINDING|BIAS").is_match(n);
    let hit = |out: &mut ElecT, id: String| {
        if soft {
            out.degraded.insert(id);
        } else {
            out.dead.insert(id);
        }
    };
    let es = if soft { "stuck" } else { "open" };
    if let Some(m) = re!(r"CONTACTOR GEN (\d) LINE").captures(n) {
        out.edge.set(&format!("GEN{}>AC{}", &m[1], &m[1]), es);
        return out;
    }
    if let Some(m) = re!(r"CONTACTOR APU GEN (\d) LINE").captures(n) {
        out.edge.set(&format!("APUGEN{}>TIE", if &m[1] == "1" { "A" } else { "B" }), es);
        return out;
    }
    if let Some(m) = re!(r"CONTACTOR TR (\d|ESS|APU) LINE").captures(n) {
        let t = match &m[1] {
            "ESS" => "ESSTR".to_string(),
            "APU" => "APUTR".into(),
            x => format!("TR{x}"),
        };
        let src = feeds().find(|(k, _)| *k == t).map(|(_, f)| f[0]).unwrap_or("undefined");
        out.edge.set(&format!("{src}>{t}"), es);
        return out;
    }
    if let Some(m) = re!(r"APU GEN(?:ERATOR)? ?([AB12])").captures(n) {
        let k = match &m[1] {
            "1" => "A",
            "2" => "B",
            x => x,
        };
        hit(&mut out, format!("APUGEN{k}"));
    } else if let Some(m) = re!(r"\b(?:GEN(?:ERATOR)?|VFG) ?(\d)").captures(n) {
        hit(&mut out, format!("GEN{}", &m[1]));
    }
    if re!(r"EMER GEN|\bRAT\b").is_match(n) {
        hit(&mut out, "EMERGEN".into());
    }
    if let Some(m) = re!(r"EXT(?: PWR)? ?(\d)").captures(n) {
        hit(&mut out, format!("EXT{}", &m[1]));
    }
    if n.contains("AC ESS SHED") {
        hit(&mut out, "ACESSSHED".into());
    } else if re!(r"AC ESS\b").is_match(n) {
        hit(&mut out, "ACESS".into());
    }
    if n.contains("AC 247XP") || n.contains("AC EHA") {
        hit(&mut out, "ACEHA".into());
    }
    if n.contains("AC EMER") {
        hit(&mut out, "ACEMER".into());
    }
    if n.contains("AC GND FLT") {
        hit(&mut out, "ACGND".into());
    }
    if let Some(m) = re!(r"\bAC ?(\d)\b").captures(n) {
        hit(&mut out, format!("AC{}", &m[1]));
    }
    if n.contains("DC ESS") {
        hit(&mut out, "DCESS".into());
    }
    if n.contains("DC 247PP") || n.contains("DC EHA") {
        hit(&mut out, "DCEHA".into());
    }
    if n.contains("DC GND FLT") {
        hit(&mut out, "DCGND".into());
    }
    if n.contains("DC APU") || n.contains("DC 309PP") {
        hit(&mut out, "DCAPU".into());
    }
    if let Some(m) = re!(r"DC HOT ?(\d)").captures(n) {
        hit(&mut out, format!("BAT{}", &m[1]));
    } else if let Some(m) = re!(r"\bDC ?(\d)\b").captures(n) {
        hit(&mut out, format!("DC{}", &m[1]));
    }
    if n.contains("ESS TR") || n.contains("TR ESS") {
        hit(&mut out, "ESSTR".into());
    } else if n.contains("APU TR") || n.contains("TR APU") {
        hit(&mut out, "APUTR".into());
    } else if let Some(m) = re!(r"\bTR ?(\d)\b").captures(n) {
        hit(&mut out, format!("TR{}", &m[1]));
    }
    if n.contains("STATIC INV") || n.contains("STAT INV") {
        hit(&mut out, "STATINV".into());
    }
    if n.contains("ESS BAT") || n.contains("BAT ESS") {
        hit(&mut out, "BATESS".into());
    } else if n.contains("APU BAT") || n.contains("BAT APU") {
        hit(&mut out, "BATAPU".into());
    } else if let Some(m) = re!(r"\bBAT(?:TERY)? ?(\d)").captures(n) {
        hit(&mut out, format!("BAT{}", &m[1]));
    }
    if n.contains("BUS TIE") {
        out.edge.set("TIE", es);
    }
    out
}

pub(crate) struct Solved {
    pub live: HashMap<String, bool>,
    pub via: HashMap<String, String>,
}

pub(crate) fn elec_solve(t: &ElecT) -> Solved {
    let mut live: HashMap<String, bool> = HashMap::new();
    let mut via: HashMap<String, String> = HashMap::new();
    for (k, ..) in eln() {
        if re!(r"^(GEN|EMERGEN|BAT)").is_match(k) && !t.dead.contains(k) {
            live.insert(k.into(), true);
        }
    }
    let gen_ok = |i: u32| !t.dead.contains(&format!("GEN{i}")) && t.edge.get(&format!("GEN{i}>AC{i}")) != Some("open");
    let any_gen = (1..=4).any(gen_ok);
    if any_gen {
        live.remove("EMERGEN");
    }
    let tie_ok = t.edge.get("TIE") != Some("open");
    let is_live = |live: &HashMap<String, bool>, k: &str| live.get(k).copied().unwrap_or(false);
    for _ in 0..6 {
        for (bus, fs) in feeds() {
            if t.dead.contains(bus) {
                live.remove(bus);
                continue;
            }
            let mut fed: Option<&str> = None;
            for src in fs.iter().copied() {
                if t.edge.get(&format!("{src}>{bus}")) == Some("open") {
                    continue;
                }
                if src == "TIE" {
                    let own: u32 = bus[2..].parse().unwrap_or(0);
                    if tie_ok && (1..=4).any(|i| i != own && gen_ok(i)) {
                        fed = Some("TIE");
                        break;
                    }
                    continue;
                }
                if let Some(g) = src.strip_prefix("GEN") {
                    if !gen_ok(g.parse().unwrap_or(0)) {
                        continue;
                    }
                }
                if src == "EMERGEN" && any_gen {
                    continue;
                }
                if src == "STATINV" && ["AC1", "AC2", "AC3", "AC4"].iter().any(|b| is_live(&live, b)) {
                    continue;
                }
                if src.starts_with("BAT") && fs[0] != src && is_live(&live, fs[0]) {
                    continue;
                }
                if is_live(&live, src) {
                    fed = Some(src);
                    break;
                }
            }
            match fed {
                Some(f) => {
                    live.insert(bus.into(), true);
                    via.insert(bus.into(), f.into());
                }
                None => {
                    live.remove(bus);
                    via.remove(bus);
                }
            }
        }
    }
    Solved { live, via }
}

fn link_label(k: &str) -> Option<&'static str> {
    Some(match k {
        "DC2>DC1" => "DC 2 \u{2192} DC 1 INTER-BUS",
        "DC1>DC2" => "DC 1 \u{2192} DC 2 INTER-BUS",
        "DC1>DCESS" => "DC 1 \u{2192} DC ESS INTER-BUS",
        "AC4>ACESS" => "AC ESS ALTN FEED",
        "EMERGEN>ACESS" => "EMER GEN",
        "ACESS>ACEHA" => "EHA BACKUP FEED",
        "STATINV>ACEMER" => "STATIC INVERTER",
        "DCESS>DCEHA" => "DC EHA BACKUP",
        "BAT1>DC1" => "BAT 1",
        "BAT2>DC2" => "BAT 2",
        "BATESS>DCESS" => "BAT ESS",
        "BATAPU>DCAPU" => "BAT APU",
        _ => return None,
    })
}

#[derive(Clone, Debug, Default)]
pub struct ElecOpts {
    pub title: String,
    pub full: bool,
    pub loads: Vec<String>,
}

pub fn elec_view(t: &ElecT, opts: &ElecOpts) -> Node {
    let w = 1000.0;
    let Solved { live, via } = elec_solve(t);
    let base = elec_solve(&ElecT::default());
    let loads = &opts.loads;
    let h = if loads.is_empty() { 690.0 } else { 760.0 };
    let lv = |k: &str| live.get(k).copied().unwrap_or(false);
    let state = |k: &str| -> &'static str {
        if t.dead.contains(k) {
            return "fail";
        }
        if t.degraded.contains(k) {
            return "warn";
        }
        if base.live.get(k).copied().unwrap_or(false) && !lv(k) {
            return "warn";
        }
        if let (Some(a), Some(b)) = (via.get(k), base.via.get(k)) {
            if a != b {
                return "alt";
            }
        }
        if lv(k) {
            "live"
        } else {
            "off"
        }
    };
    let affected = |k: &str| !matches!(state(k), "live" | "off");
    let mut parts = Vec::new();
    let top = |k: &str| {
        let p = pos(k).unwrap();
        [p.1 + ELN_W / 2.0, p.2]
    };
    let bot = |k: &str| {
        let p = pos(k).unwrap();
        [p.1 + ELN_W / 2.0, p.2 + ELN_H]
    };
    let tie_y = 200.0;
    let tie_used = (1..=4).any(|i| via.get(&format!("AC{i}")).map(String::as_str) == Some("TIE"));
    let tie_st = if t.edge.get("TIE") == Some("open") { "fail" } else if tie_used { "alt" } else { "off" };
    parts.push(ns_wire(&[[top("AC1")[0], tie_y], [top("AC4")[0], tie_y]], tie_st, if tie_used { "7 5" } else { "" }, true));
    parts.push(ex_t(top("AC1")[0] - 8.0, tie_y - 6.0, "BUS TIE", T { size: 11.0, fill: if tie_used { NS_ORANGE } else { NS_GREY_TEXT }, anchor: "end", ..TD }));
    for i in 1..=4 {
        let ac = format!("AC{i}");
        let st = if via.get(&ac).map(String::as_str) == Some("TIE") { "alt" } else { "off" };
        parts.push(ns_wire(&[bot(&ac), [bot(&ac)[0], tie_y]], st, "", true));
        parts.push(ns_elbow(bot(&format!("EXT{i}")), top(&ac), "off", ""));
    }
    for s in ["APUGENA", "APUGENB"] {
        parts.push(ns_wire(&[bot(s), [bot(s)[0], tie_y]], if t.edge.get(&format!("{s}>TIE")) == Some("open") { "fail" } else { "off" }, "", true));
    }
    let mut labels = Vec::new();
    let mut active = Vec::new();
    for (bus, fs) in feeds() {
        for src in fs.iter().copied() {
            if src == "TIE" || src.starts_with("EXT") {
                continue;
            }
            let key = format!("{src}>{bus}");
            let broken = t.edge.get(&key) == Some("open");
            let used = via.get(bus).map(String::as_str) == Some(src);
            let normal = base.via.get(bus).map(String::as_str) == Some(src);
            let st = if broken {
                "fail"
            } else if used && !normal {
                "alt"
            } else if normal && !used && (affected(src) || affected(bus)) {
                "warn"
            } else if used {
                "live"
            } else {
                "off"
            };
            let (ps, pb) = (pos(src).unwrap(), pos(bus).unwrap());
            let (a, b) = if ps.2 < pb.2 { (bot(src), top(bus)) } else { (top(src), bot(bus)) };
            let lane = match key.as_str() {
                "DC2>DC1" | "DC1>DC2" => Some(532.0),
                "DC1>DCESS" => Some(546.0),
                _ => None,
            };
            let dash = if broken { "3 4" } else if st == "alt" { "7 5" } else { "" };
            let pts = lane.map(|l| [bot(src), [bot(src)[0], l], [bot(bus)[0], l], bot(bus)]);
            let s = match &pts {
                Some(p) => ns_wire(p, st, dash, true),
                None => ns_elbow(a, b, st, dash),
            };
            if st == "off" {
                parts.push(s);
            } else {
                active.push(s);
            }
            if st == "alt" {
                if let Some(l) = link_label(&key) {
                    let (x, y) = match (&pts, lane) {
                        (Some(p), Some(ln)) => ((p[1][0] + p[2][0]) / 2.0, ln + 15.0),
                        _ => ((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0 + 15.0),
                    };
                    labels.push(ex_t(x, y, l, T { size: 10.0, fill: NS_GREEN, ..TD }));
                }
            }
        }
    }
    parts.extend(active);
    for (k, label, x, y) in eln() {
        let st = state(k);
        let tag = match st {
            "fail" => "FAILED".to_string(),
            "alt" => {
                let v = via.get(k).cloned().unwrap_or_default();
                format!("FED FROM {}", pos(&v).map(|p| p.0.to_string()).unwrap_or(v))
            }
            "warn" => if t.degraded.contains(k) { "DEGRADED" } else { "LOST" }.into(),
            _ => String::new(),
        };
        parts.push(ns_box(x, y, ELN_W, ELN_H, label, st, B { tag: &tag, ..Default::default() }));
    }
    parts.extend(labels);
    if !loads.is_empty() {
        let y0 = 700.0;
        parts.push(ex_t(20.0, y0, "LOADS LOST", T { anchor: "start", size: 12.0, fill: NS_ORANGE, ..TD }));
        for (i, l) in loads.iter().take(6).enumerate() {
            let len = l.encode_utf16().count();
            let t: String = if len > 40 { format!("{}\u{2026}", String::from_utf16_lossy(&l.encode_utf16().take(39).collect::<Vec<_>>())) } else { l.clone() };
            parts.push(ex_t(20.0 + (i % 3) as f64 * 330.0, y0 + 22.0 + (i / 3) as f64 * 20.0, &t, T { anchor: "start", size: 11.0, fill: "#d8d8d8", weight: 500.0, ..TD }));
        }
        if loads.len() > 6 {
            parts.push(ex_t(w - 20.0, y0, &format!("+{} MORE", loads.len() - 6), T { anchor: "end", size: 11.0, fill: NS_GREY_TEXT, ..TD }));
        }
    }
    if !opts.title.is_empty() {
        parts.push(ex_t(w - 16.0, h - 12.0, &opts.title, T { anchor: "end", size: 12.0, fill: NS_GREY_TEXT, weight: 500.0, ..TD }));
    }
    let mut hot = Vec::new();
    for (k, _, x, y) in eln() {
        let st = state(k);
        if st == "live" || st == "off" {
            continue;
        }
        hot.push([x, y - 14.0, ELN_W, ELN_H + 14.0]);
        if let Some(src) = via.get(k).or(base.via.get(k)) {
            if let Some(p) = pos(src) {
                hot.push([p.1, p.2, ELN_W, ELN_H]);
            }
        }
    }
    let focus = if opts.full { None } else { super::ns_bounds(&hot) };
    ns_svg(w, h, &parts, focus.as_deref())
}

pub(crate) fn elec_loads_lost(t: &ElecT) -> Vec<String> {
    ctx(|c| {
        let Some(br) = &c.breakers else { return vec![] };
        let Solved { live, .. } = elec_solve(t);
        let base = elec_solve(&ElecT::default());
        let lost: BTreeSet<&String> = base.live.keys().filter(|k| !live.get(*k).copied().unwrap_or(false)).collect();
        let norm = |b: &str| re!(r"[_\s-]").replace_all(&b.to_uppercase(), "").replacen("247XP", "EHA", 1).replacen("247PP", "EHA", 1).replacen("GNDFLTSVC", "GND", 1);
        let key = |b: &str| -> Option<&'static str> {
            let nn = norm(b);
            eln().into_iter().map(|e| e.0).find(|k| *k == nn || *k == nn.replacen("HOT", "BAT", 1))
        };
        br.iter().filter(|(bus, _)| key(bus).is_some_and(|k| lost.iter().any(|l| l.as_str() == k))).map(|(_, nm)| nm.clone()).collect()
    })
}

pub fn elec_diagram(name: &str, title: &str) -> Option<Node> {
    let t = elec_targets(name);
    if t.is_empty() {
        return None;
    }
    let loads = elec_loads_lost(&t);
    Some(elec_view(&t, &ElecOpts { title: title.into(), full: false, loads }))
}

use super::{ex_t, ns_bounds, ns_box, ns_stroke, ns_svg, ns_valve, ns_wire, wire, wire_d, OMap, B, NS_GREEN, NS_GREY_TEXT, T, TD};
use crate::Node;

const TANKS: [(&str, &str, f64, &str); 10] = [
    ("OUTR_L", "L OUTER", 18.0, "outer"), ("FEED1", "FEED 1", 114.0, "feed"), ("MID_L", "L MID", 206.0, "mid"), ("FEED2", "FEED 2", 302.0, "feed"), ("INR_L", "L INNER", 394.0, "inner"),
    ("INR_R", "R INNER", 518.0, "inner"), ("FEED3", "FEED 3", 610.0, "feed"), ("MID_R", "R MID", 702.0, "mid"), ("FEED4", "FEED 4", 794.0, "feed"), ("OUTR_R", "R OUTER", 890.0, "outer"),
];

fn cap(k: &str) -> &'static str {
    match k {
        "OUTR_L" | "OUTR_R" => "8.9",
        "FEED1" | "FEED4" => "21.6",
        "MID_L" | "MID_R" => "27.9",
        "FEED2" | "FEED3" => "23.2",
        _ => "36.5",
    }
}

fn pumps(kind: &str) -> &'static [&'static str] {
    match kind {
        "feed" => &["MAIN", "STBY"],
        "inner" | "mid" => &["FWD", "AFT"],
        _ => &["XFR"],
    }
}

pub fn fuel_targets(name: &str) -> Option<OMap> {
    let nn = format!(" {} ", re!(r"[_-]").replace_all(&name.to_uppercase(), " "));
    let n = nn.as_str();
    let mut m = OMap::default();
    let side = if re!(r"\bLEFT\b|\bL\b").is_match(n) {
        Some("L")
    } else if re!(r"\bRIGHT\b|\bR\b").is_match(n) {
        Some("R")
    } else {
        None
    };
    let tank: Option<String> = if let Some(f) = re!(r"FEED(?: TANK)? ?(\d)|COLLECTOR CELL (\d)|TANK (\d)\b").captures(n) {
        Some(format!("FEED{}", f.get(1).or(f.get(2)).or(f.get(3)).map(|x| x.as_str()).unwrap_or("")))
    } else if re!(r"OUTER|OUTR").is_match(n) && side.is_some() {
        Some(format!("OUTR_{}", side.unwrap()))
    } else if re!(r"\bMID\b").is_match(n) && side.is_some() {
        Some(format!("MID_{}", side.unwrap()))
    } else if re!(r"INNER|\bINR\b").is_match(n) && side.is_some() {
        Some(format!("INR_{}", side.unwrap()))
    } else if n.contains("TRIM") {
        Some("TRIM".into())
    } else {
        None
    };
    let soft = re!(r"DEGRAD|WEAR|DRIFT|FOULING|CONTAMINATION|BIAS").is_match(n);
    let lp = re!(r"ENG(?:INE)? ?(\d).*(?:LP|SHUTOFF|SHUT OFF)").captures(n).or_else(|| re!(r"LP (?:FUEL )?VALVE ?(\d)").captures(n));
    if let Some(e) = lp {
        m.set(&format!("LP{}", &e[1]), "fail");
    } else if let Some(e) = re!(r"CROSS ?FEED VALVE ?(\d)(?: ?(\d))?|XFEED ?(?:VALVE )?(\d)").captures(n) {
        m.set(&format!("XF{}", e.get(1).or(e.get(3)).map(|x| x.as_str()).unwrap_or("")), "fail");
        if let Some(x) = e.get(2) {
            m.set(&format!("XF{}", x.as_str()), "fail");
        }
    } else if n.contains("JETT") {
        m.set(&format!("JETT_{}", side.unwrap_or("L")), "fail");
    } else if re!(r"REFUEL|DEFUEL").is_match(n) {
        m.set(&format!("REFUEL_{}", side.unwrap_or("L")), "fail");
    } else if n.contains("APU") {
        m.set("APU", "fail");
    } else if n.contains("GALLERY") {
        m.set(if n.contains("AFT") { "GAL_AFT" } else { "GAL_FWD" }, "fail");
    } else if re!(r"TRIM LINE|ISOLATION").is_match(n) {
        m.set(if n.contains("AFT") { "TRIMISO_AFT" } else { "TRIMISO_FWD" }, "fail");
    } else if let (Some(t), true) = (&tank, n.contains("INLET")) {
        m.set(&format!("{t}:{}", if n.contains("AFT") { "IN_AFT" } else { "IN_FWD" }), "fail");
    } else if let (Some(t), true) = (&tank, n.contains("PUMP")) {
        let kind = if t.starts_with("FEED") {
            if re!(r"STBY|STANDBY|\bB\b").is_match(n) { "STBY" } else { "MAIN" }
        } else if t == "TRIM" {
            if side == Some("R") { "R" } else { "L" }
        } else if t.starts_with("OUTR") {
            "XFR"
        } else if n.contains("AFT") {
            "AFT"
        } else {
            "FWD"
        };
        m.set(&format!("{t}:{kind}"), if soft { "warn" } else { "fail" });
    } else if let (Some(t), true) = (&tank, re!(r"PUNCTUR|LEAK").is_match(n)) {
        m.set(t, "fail");
    } else if let Some(t) = &tank {
        m.set(t, "warn");
    }
    if m.is_empty() { None } else { Some(m) }
}

pub fn fuel_view(m: &OMap, title: &str) -> Node {
    let (w, h) = (1000.0, 600.0);
    let mut wires = Vec::new();
    let mut fg = Vec::new();
    let (ty, th, tw) = (214.0, 64.0, 88.0);
    let at = |k: &str| TANKS.iter().find(|t| t.0 == k).map(|t| t.2 + tw / 2.0).unwrap_or(0.0);
    let valve = |fg: &mut Vec<String>, x: f64, y: f64, k: &str, label: &str, open: bool| {
        let s = m.st(k, if open { "live" } else { "off" });
        fg.push(ns_valve(x, y, s, if s == "fail" { false } else { open }, label, false));
    };
    let feed_dead = |k: u32| ["MAIN", "STBY"].iter().all(|p| m.get(&format!("FEED{k}:{p}")) == Some("fail")) || m.get(&format!("FEED{k}")) == Some("fail");
    let xfeed_used = (1..=4).any(feed_dead);
    for k in 1..=4 {
        let cx = at(&format!("FEED{k}"));
        let lp_fail = m.get(&format!("LP{k}")) == Some("fail");
        let eng_st = if lp_fail { "warn" } else if feed_dead(k) { "alt" } else { "live" };
        wires.push(wire(&[[cx, ty], [cx, 160.0]], if feed_dead(k) { "warn" } else if lp_fail { "fail" } else { "live" }));
        wires.push(wire(&[[cx, 160.0], [cx, 74.0]], if lp_fail { "fail" } else { "live" }));
        fg.push(ns_box(cx - 42.0, 40.0, 84.0, 34.0, &format!("ENG {k}"), if eng_st == "alt" { "live" } else { eng_st }, B { tag: if lp_fail { "FUEL CUT" } else if feed_dead(k) { "ON XFEED" } else { "" }, ..Default::default() }));
        valve(&mut fg, cx, 112.0, &format!("LP{k}"), "LP", true);
        wires.push(wire_d(
            &[[cx, 160.0], [cx + 30.0, 160.0], [cx + 30.0, 176.0]],
            if xfeed_used && (feed_dead(k) || m.get(&format!("XF{k}")).is_none()) { "live" } else { "off" },
            if xfeed_used { "7 5" } else { "" },
        ));
        valve(&mut fg, cx + 30.0, 176.0, &format!("XF{k}"), "XFEED", xfeed_used);
    }
    wires.push(ns_wire(&[[at("FEED1") + 30.0, 186.0], [at("FEED4") + 30.0, 186.0]], if xfeed_used { "alt" } else { "off" }, if xfeed_used { "7 5" } else { "" }, false));
    fg.push(ex_t(500.0, 182.0, "CROSSFEED MANIFOLD", T { size: 10.0, fill: if xfeed_used { NS_GREEN } else { NS_GREY_TEXT }, ..TD }));
    let (gf, ga) = (372.0, 404.0);
    for (y, k, label) in [(gf, "GAL_FWD", "FWD TRANSFER GALLERY"), (ga, "GAL_AFT", "AFT TRANSFER GALLERY")] {
        let s = m.st(k, "live");
        wires.push(wire_d(&[[40.0, y], [960.0, y]], s, if s == "fail" { "4 5" } else { "" }));
        fg.push(ex_t(500.0, y - 5.0, label, T { size: 9.0, fill: if s == "live" { NS_GREY_TEXT } else { ns_stroke(s) }, ..TD }));
    }
    for (k, label, x, kind) in TANKS {
        let cx = x + tw / 2.0;
        let ps = pumps(kind);
        let pf = ps.iter().filter(|p| m.get(&format!("{k}:{p}")) == Some("fail")).count();
        let tank_st = m.get(k).unwrap_or(if pf == ps.len() { "warn" } else { "live" });
        let tag = match m.get(k) {
            Some("fail") => "LEAK",
            Some(_) => "FAULT",
            None if pf == ps.len() => {
                if kind == "feed" {
                    "NO PRESS"
                } else {
                    "UNUSABLE"
                }
            }
            None => "",
        };
        fg.push(ns_box(x, ty, tw, th, &format!("{label}\n{} T", cap(k)), tank_st, B { tag, ..Default::default() }));
        for (i, p) in ps.iter().enumerate() {
            let px = if ps.len() == 1 { cx - 20.0 } else { x + i as f64 * 46.0 };
            let pk = format!("{k}:{p}");
            let s = m.get(&pk).unwrap_or(if m.get(k) == Some("fail") { "warn" } else { "live" });
            wires.push(wire(&[[px + 21.0, ty + th], [px + 21.0, 296.0]], if s == "fail" { "fail" } else { s }));
            if kind != "feed" {
                wires.push(wire(&[[px + 21.0, 326.0], [px + 21.0, if i > 0 { ga } else { gf }]], if s == "fail" { "fail" } else { s }));
            }
            fg.push(ns_box(px, 296.0, 42.0, 30.0, p, s, B { size: 10.0, r: 7.0, ..Default::default() }));
        }
        if kind == "feed" {
            for (dir, y, dx) in [("FWD", gf, -16.0), ("AFT", ga, 16.0)] {
                let ik = format!("{k}:IN_{dir}");
                let s = m.st(&ik, "live");
                wires.push(wire(&[[cx + dx, y], [cx + dx, 344.0]], if s == "fail" { "warn" } else { "live" }));
                valve(&mut fg, cx + dx, 352.0, &ik, "", true);
            }
        }
    }
    let trim_st = m.st("TRIM", "live");
    fg.push(ns_box(420.0, 500.0, 160.0, 52.0, "TRIM TANK", trim_st, B { tag: if m.get("TRIM") == Some("fail") { "LEAK" } else { "" }, ..Default::default() }));
    for (k, x, label) in [("TRIM:L", 440.0, "L"), ("TRIM:R", 520.0, "R")] {
        let s = m.st(k, "live");
        wires.push(wire(&[[x + 20.0, 500.0], [x + 20.0, 470.0]], if s == "fail" { "fail" } else { s }));
        fg.push(ns_box(x, 440.0, 40.0, 30.0, label, s, B { size: 10.0, r: 7.0, ..Default::default() }));
    }
    for (k, y, x) in [("TRIMISO_FWD", gf, 470.0), ("TRIMISO_AFT", ga, 530.0)] {
        let s = m.st(k, "live");
        wires.push(wire(&[[x, y], [x, 440.0]], if s == "fail" { "warn" } else { "live" }));
        valve(&mut fg, x, 424.0, k, "", true);
        let fwd = k.ends_with("FWD");
        fg.push(ex_t(if fwd { x - 14.0 } else { x + 14.0 }, 428.0, if fwd { "ISO FWD" } else { "ISO AFT" }, T { size: 9.0, anchor: if fwd { "end" } else { "start" }, fill: NS_GREY_TEXT, ..TD }));
    }
    for (k, x, label) in [("JETT_L", 40.0, "JETT L"), ("JETT_R", 960.0, "JETT R")] {
        wires.push(wire(&[[x, gf], [x, ga + 40.0]], if m.get(k).is_some() { "warn" } else { "off" }));
        valve(&mut fg, x, ga + 30.0, k, label, false);
    }
    for (k, x, label) in [("REFUEL_L", 120.0, "REFUEL L"), ("REFUEL_R", 880.0, "REFUEL R")] {
        wires.push(wire(&[[x, ga], [x, ga + 70.0]], if m.get(k).is_some() { "warn" } else { "off" }));
        valve(&mut fg, x, ga + 54.0, k, label, false);
    }
    let apu = m.st("APU", "live");
    wires.push(wire(&[[at("FEED4") + 30.0, 186.0], [960.0, 186.0], [960.0, 520.0], [900.0, 520.0]], if apu == "fail" { "warn" } else { "live" }));
    fg.push(ns_box(820.0, 504.0, 80.0, 32.0, "APU", if apu == "fail" { "warn" } else { "live" }, B { tag: if apu == "fail" { "NO FEED" } else { "" }, ..Default::default() }));
    if !title.is_empty() {
        fg.push(ex_t(w - 16.0, h - 8.0, title, T { anchor: "end", size: 12.0, fill: NS_GREY_TEXT, weight: 500.0, ..TD }));
    }
    let mut xs: Vec<[f64; 4]> = Vec::new();
    for k in m.keys() {
        let base = k.split(':').next().unwrap_or("");
        if let Some(t) = TANKS.iter().find(|t| t.0 == base) {
            xs.push([t.2, 30.0, tw, 380.0]);
        } else if re!(r"^(LP|XF)(\d)").is_match(k) {
            let d = re!(r"(\d)").captures(k).map(|c| c[1].to_string()).unwrap_or_default();
            xs.push([at(&format!("FEED{d}")) - 60.0, 30.0, 150.0, 300.0]);
        } else if k.starts_with("TRIM") {
            xs.push([400.0, 360.0, 200.0, 200.0]);
        } else if k.starts_with("GAL") {
            xs.push([30.0, 340.0, 940.0, 90.0]);
        } else if re!(r"JETT_L|REFUEL_L").is_match(k) {
            xs.push([10.0, 340.0, 200.0, 160.0]);
        } else if re!(r"JETT_R|REFUEL_R").is_match(k) {
            xs.push([790.0, 340.0, 200.0, 160.0]);
        } else if k == "APU" {
            xs.push([780.0, 160.0, 210.0, 390.0]);
        }
    }
    for k in 1..=4 {
        if feed_dead(k) {
            xs.push([at("FEED1") - 40.0, 20.0, at("FEED4") - at("FEED1") + 120.0, 300.0]);
        }
    }
    wires.extend(fg);
    ns_svg(w, h, &wires, ns_bounds(&xs).as_deref())
}

pub fn fuel_diagram(name: &str, title: &str) -> Option<Node> {
    fuel_targets(name).map(|m| fuel_view(&m, title))
}

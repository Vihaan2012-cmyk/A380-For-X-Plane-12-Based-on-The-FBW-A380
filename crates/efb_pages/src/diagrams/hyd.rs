use super::{ex_t, ns_box, ns_stroke, ns_svg, ns_valve, wire, wire_d, FailIn, OMap, B, NS_GREEN, NS_GREY, NS_GREY_TEXT, NS_RED, T, TD};
use crate::jv::n;
use crate::Node;

/// `{ G, Y }` maps, or the single `{ sys, m }` of `hydTargets` (then `single` names it).
#[derive(Clone, Debug, Default)]
pub struct HydT {
    pub g: OMap,
    pub y: OMap,
    pub single: Option<&'static str>,
}

impl HydT {
    fn of(&self, sys: &str) -> &OMap {
        if sys == "G" { &self.g } else { &self.y }
    }
    fn of_mut(&mut self, sys: &str) -> &mut OMap {
        if sys == "G" { &mut self.g } else { &mut self.y }
    }
}

pub fn hyd_targets(name: &str) -> Option<HydT> {
    let nn = format!(" {} ", re!(r"[_-]").replace_all(&name.to_uppercase(), " "));
    let n = nn.as_str();
    let mut sys = if re!(r"\bGREEN\b|\bG\b|\bGRN\b").is_match(n) {
        Some("G")
    } else if re!(r"\bYELLOW\b|\bY\b|\bYLW\b").is_match(n) {
        Some("Y")
    } else {
        None
    };
    let eng = re!(r"(?:EDP|ENG(?:INE)?(?: PUMP)?|PUMP)\s*(\d)\s*([AB])?").captures(n);
    if sys.is_none() {
        if let Some(e) = &eng {
            sys = Some(if e[1].parse::<u32>().unwrap_or(9) <= 2 { "G" } else { "Y" });
        }
    }
    let sys = sys?;
    let mut t = HydT { single: Some(sys), ..Default::default() };
    let soft = re!(r"DEGRAD|DISPLACEMENT|OVERHEAT|OVHT|CLOG|CONTAMINATION|PRECHARGE|CRACKING|AIR (?:LEAK|INGESTION)|DRIFT|LO PR|LO LEVEL").is_match(n);
    let m = t.of_mut(sys);
    let st = if soft { "warn" } else { "fail" };
    if let (Some(e), true) = (&eng, re!(r"EDP|ENG(?:INE)? PUMP|ENG \d [AB] PUMP|PUMP \d ?[AB]").is_match(n)) {
        m.set(&format!("EDP{}{}", &e[1], e.get(2).map(|x| x.as_str()).unwrap_or("A")), st);
    } else if re!(r"ELEC(?:TRIC)? PUMP|LOCAL ELECTRIC|\bE PUMP").is_match(n) || re!(r"(GREEN|YELLOW) [AB] ELEC").is_match(n) {
        m.set(&format!("EP{}", re!(r"\b([AB])\b").captures(n).map(|c| c[1].to_string()).unwrap_or("A".into())), st);
    } else if re!(r"RESERVOIR|RSVR").is_match(n) {
        m.set("RSVR", if soft && !re!(r"LEAK$|RESERVOIR LEAK |RSVR LEAK").is_match(n) { "warn" } else { "fail" });
    } else if re!(r"ACCUMULATOR|ACCU").is_match(n) {
        m.set("ACC", "warn");
    } else if n.contains("PRIORITY") {
        m.set("PRIO", "fail");
    } else if n.contains("RELIEF") {
        m.set("RELIEF", "warn");
    } else if n.contains("FILTER") {
        m.set("FILTER", "warn");
    } else if re!(r"HX|HEAT EXCH").is_match(n) {
        m.set("HX", "warn");
    } else if re!(r"AIR INGESTION|SYS PRESS|SYSTEM PRESS|\bLO PR\b|OVERPRESS").is_match(n) {
        m.set("MANIFOLD", if re!(r"LO|INGESTION").is_match(n) { "fail" } else { "warn" });
    } else if re!(r"LINE LEAK|LEAK").is_match(n) {
        let user = if n.contains("BRAKE") {
            Some(if sys == "G" { "NORM BRAKES" } else { "ALTN BRAKES" })
        } else if re!(r"GEAR|L/G").is_match(n) {
            Some("L/G")
        } else if n.contains("STEER") {
            Some("STEERING")
        } else if n.contains("CARGO") {
            Some("CARGO DOORS")
        } else if n.contains("REVERS") {
            Some("REVERSER")
        } else {
            None
        };
        if let Some(u) = user {
            m.set(&format!("U:{u}"), "fail");
        }
        m.set("RSVR", "warn");
    } else {
        m.set("MANIFOLD", "warn");
    }
    Some(t)
}

pub fn hyd_view(t: Option<&HydT>, title: &str) -> Node {
    let (w, h) = (1000.0, 860.0);
    let mut parts: Vec<String> = Vec::new();
    let mut fg: Vec<String> = Vec::new();
    let empty = OMap::default();
    for sys in ["G", "Y"] {
        let m = match t {
            Some(t) if t.single.is_none() || t.single == Some(sys) => t.of(sys),
            _ => &empty,
        };
        let x0 = if sys == "G" { 20.0 } else { 520.0 };
        let cx = x0 + 230.0;
        let name = if sys == "G" { "GREEN" } else { "YELLOW" };
        let engs = if sys == "G" { [1, 2] } else { [3, 4] };
        let sys_lost = m.get("RSVR") == Some("fail") || m.get("MANIFOLD") == Some("fail");
        let st = |k: &str| -> String {
            match m.get(k) {
                Some(v) => v.to_string(),
                None if sys_lost && !re!(r"^(RSVR|MANIFOLD)$").is_match(k) && !re!(r"^(EDP|EP)").is_match(k) => "warn".into(),
                None => "live".into(),
            }
        };
        fg.push(ex_t(x0 + 6.0, 20.0, &format!("{name} SYSTEM"), T { anchor: "start", size: 14.0, fill: if sys_lost { NS_RED } else { NS_GREEN }, ..TD }));
        if let Some(note) = m.get("NOTE") {
            for (i, line) in note.split('|').enumerate() {
                fg.push(ex_t(x0 + 470.0, 46.0 + i as f64 * 14.0, line, T { anchor: "end", size: 10.0, fill: NS_RED, ..TD }));
            }
        }
        let tag = match m.get("RSVR") {
            Some("fail") => "FAILED",
            Some(_) => "DEGRADED",
            None => "",
        };
        fg.push(ns_box(cx - 90.0, 30.0, 180.0, 44.0, if m.get("RSVR") == Some("fail") { "RESERVOIR\nQTY LO" } else { "RESERVOIR\n3.5 BAR" }, &st("RSVR"), B { size: 12.0, tag, ..Default::default() }));
        let (pump_y, ep_y) = (132.0, 214.0);
        let mut pumps: Vec<(String, String, f64, f64)> = Vec::new();
        for (i, e) in engs.iter().enumerate() {
            for (j, ab) in ["A", "B"].iter().enumerate() {
                pumps.push((format!("EDP{e}{ab}"), format!("EDP {e}{ab}"), x0 + 8.0 + (i * 2 + j) as f64 * 112.0, pump_y));
            }
        }
        pumps.push(("EPA".into(), "ELEC A".into(), x0 + 120.0, ep_y));
        pumps.push(("EPB".into(), "ELEC B".into(), x0 + 232.0, ep_y));
        if sys == "G" {
            pumps.push(("AUX".into(), "AUX PUMP".into(), x0 + 344.0, ep_y));
        }
        let man_y = 300.0;
        let supply = if sys_lost { "warn" } else { "live" };
        let header = 88.0;
        parts.push(wire(&[[cx, 74.0], [cx, header]], supply));
        parts.push(wire(&[[x0 + 58.0, header], [x0 + 394.0, header]], supply));
        for (i, e) in engs.iter().enumerate() {
            let vx = x0 + 8.0 + i as f64 * 224.0 + 106.0;
            parts.push(wire(&[[vx, header], [vx, 118.0]], supply));
            parts.push(wire(&[[vx - 56.0, 118.0], [vx + 56.0, 118.0]], supply));
            parts.push(wire(&[[vx - 56.0, 118.0], [vx - 56.0, pump_y]], supply));
            parts.push(wire(&[[vx + 56.0, 118.0], [vx + 56.0, pump_y]], supply));
            fg.push(ns_valve(vx, 103.0, if sys_lost { "warn" } else { "live" }, m.get(&format!("FSOV{e}")) != Some("shut"), "", false));
            fg.push(ex_t(vx + 14.0, 99.0, &format!("ENG {e}"), T { size: 9.0, anchor: "start", fill: NS_GREY_TEXT, ..TD }));
            fg.push(ex_t(vx + 14.0, 110.0, "FSOV", T { size: 9.0, anchor: "start", fill: NS_GREY_TEXT, ..TD }));
        }
        let header_y = 258.0;
        let flow = if sys_lost { "warn" } else { "live" };
        for (k, _, x, y) in &pumps {
            let s = m.st(k, "live");
            let out = if s == "fail" { "fail" } else { flow };
            parts.push(wire(&[[x + 50.0, y + 34.0], [x + 50.0, y + 44.0]], out));
            parts.push(wire_d(&[[x + 50.0, y + 44.0], [x + 50.0, header_y]], if s == "fail" { "fail" } else { flow }, if s == "fail" { "5 5" } else { "" }));
            fg.push(format!(
                "<path d=\"M{},{} L{},{} L{},{} Z\" fill=\"{}\"/><path d=\"M{},{} L{},{}\" stroke=\"{}\" stroke-width=\"2\"/>",
                n(x + 44.0),
                n(y + 38.0),
                n(x + 56.0),
                n(y + 38.0),
                n(x + 50.0),
                n(y + 47.0),
                ns_stroke(out),
                n(x + 43.0),
                n(y + 48.0),
                n(x + 57.0),
                n(y + 48.0),
                ns_stroke(out)
            ));
        }
        parts.push(wire(&[[x0 + 40.0, header_y], [x0 + 434.0, header_y]], flow));
        fg.push(ex_t(x0 + 440.0, header_y + 4.0, "HP", T { size: 9.0, anchor: "start", fill: NS_GREY_TEXT, ..TD }));
        for (k, label, x, y) in &pumps {
            let s = m.st(k, if sys_lost { "warn" } else { "live" });
            let tag = match m.get(k) {
                Some("fail") => "FAILED",
                Some(_) => "DEGRADED",
                None => "",
            };
            fg.push(ns_box(*x, *y, 100.0, 34.0, label, s, B { tag, ..Default::default() }));
        }
        for (i, (k, label)) in [("FILTER", "FILTER"), ("HX", "FUEL HX"), ("ACC", "ACCUM"), ("RELIEF", "RELIEF V"), ("PRIO", "PRIORITY V")].iter().enumerate() {
            let x = x0 + 8.0 + i as f64 * 90.0;
            let cs = if st(k) == "fail" { "fail" } else { flow };
            parts.push(wire(&[[x + 40.0, header_y], [x + 40.0, 268.0]], cs));
            parts.push(wire(&[[x + 40.0, 296.0], [x + 40.0, man_y + 8.0]], cs));
            let tag = match m.get(k) {
                Some("fail") => "FAILED",
                Some(_) => "DEGRADED",
                None => "",
            };
            fg.push(ns_box(x, 268.0, 80.0, 28.0, label, &st(k), B { size: 11.0, tag, ..Default::default() }));
        }
        let man_st = m.get("MANIFOLD").unwrap_or(if sys_lost { "warn" } else { "live" });
        parts.push(wire(&[[x0 + 8.0, man_y + 8.0], [x0 + 452.0, man_y + 8.0]], man_st));
        let mt = if sys_lost {
            "0 PSI"
        } else if m.get("MANIFOLD").is_some() || m.get("RELIEF").is_some() {
            "LOW PRESS"
        } else {
            "5000 PSI"
        };
        fg.push(ex_t(x0 + 452.0, man_y + 26.0, mt, T { anchor: "end", size: 11.0, fill: ns_stroke(man_st), ..TD }));
        let branches: [(&str, &str, &[&str]); 4] = if sys == "G" {
            [
                ("PRIMARY FLIGHT CONTROLS", "FLIGHT", &["L OUT AIL", "L IN AIL", "R OUT AIL", "R IN AIL", "L IN ELEV", "L OUT ELEV", "L SPLR 2", "L SPLR 4", "L SPLR 6", "L SPLR 8", "R SPLR 2", "R SPLR 4", "R SPLR 6", "R SPLR 8", "UPPER RUD"]),
                ("SECONDARY FLIGHT CONTROLS", "SECONDARY", &["THS MOTOR", "SLAT PCU", "FLAP PCU"]),
                ("LANDING GEAR \u{b7} BEHIND PRIORITY VALVE", "L/G", &["NLG ACT", "L WLG ACT", "R WLG ACT", "L BLG ACT", "R BLG ACT", "GEAR DOORS"]),
                ("NORMAL BRAKES", "NORM BRAKES", &["L WLG BRK", "R WLG BRK", "L BLG BRK", "R BLG BRK"]),
            ]
        } else {
            [
                ("PRIMARY FLIGHT CONTROLS", "FLIGHT", &["L OUT AIL", "L MID AIL", "R OUT AIL", "R MID AIL", "R IN ELEV", "R OUT ELEV", "L SPLR 1", "L SPLR 3", "L SPLR 5", "L SPLR 7", "R SPLR 1", "R SPLR 3", "R SPLR 5", "R SPLR 7", "LOWER RUD"]),
                ("SECONDARY FLIGHT CONTROLS", "SECONDARY", &["THS MOTOR", "FLAP PCU"]),
                ("STEERING AND CARGO DOORS \u{b7} BEHIND PRIORITY VALVE", "STEERING", &["NOSE STEER", "L BODY STEER", "R BODY STEER", "FWD CARGO DR", "AFT CARGO DR"]),
                ("ALTERNATE BRAKES", "ALTN BRAKES", &["BRK ACCUM", "L WLG BRK", "R WLG BRK", "L BLG BRK", "R BLG BRK"]),
            ]
        };
        let mut by = man_y + 56.0;
        let trunk = x0 + 16.0;
        let trunk_st = if sys_lost { "warn" } else { "live" };
        let mut trunk_end = by;
        for (label, key, items) in branches {
            let prio = label.contains("PRIORITY");
            let leak: Option<String> = m.get(&format!("U:{key}")).map(str::to_string).or_else(|| {
                if key == "STEERING" {
                    m.get("U:STEERING").or(m.get("U:CARGO DOORS")).map(str::to_string)
                } else {
                    None
                }
            });
            let bs = if leak.is_some() {
                "fail"
            } else if sys_lost || (prio && m.get("PRIO") == Some("fail")) {
                "warn"
            } else {
                "live"
            };
            let stb = if leak.is_some() { "warn" } else { bs };
            let rows = items.len().div_ceil(6);
            fg.push(ex_t(x0 + 44.0, by + 8.0, label, T { size: 9.0, anchor: "start", fill: if bs == "live" { NS_GREY_TEXT } else { ns_stroke(bs) }, ..TD }));
            if let Some(l) = &leak {
                fg.push(ex_t(x0 + 470.0, by + 8.0, if l == "fail" { "LINE LEAK" } else { l }, T { size: 9.0, anchor: "end", fill: NS_RED, ..TD }));
            }
            for r in 0..rows {
                let row = &items[r * 6..(r * 6 + 6).min(items.len())];
                let y = by + 26.0 + r as f64 * 46.0;
                let bus_y = y - 10.0;
                let last_x = x0 + 44.0 + (row.len() as f64 - 1.0) * 70.0 + 32.0;
                parts.push(wire(&[[trunk, bus_y], [last_x, bus_y]], stb));
                fg.push(format!("<circle cx=\"{}\" cy=\"{}\" r=\"4\" fill=\"{}\"/>", n(trunk), n(bus_y), ns_stroke(trunk_st)));
                if prio {
                    fg.push(ns_valve(trunk + 16.0, bus_y, if m.get("PRIO") == Some("fail") { "fail" } else { "live" }, m.get("PRIO") != Some("fail"), "", false));
                }
                for (j, u) in row.iter().enumerate() {
                    let x = x0 + 44.0 + j as f64 * 70.0;
                    let torn = m.get(&format!("A:{u}")).is_some();
                    parts.push(wire_d(&[[x + 32.0, bus_y], [x + 32.0, y]], if torn { "fail" } else { stb }, if torn { "4 4" } else { "" }));
                    fg.push(ns_box(x, y, 64.0, 26.0, u, if torn { "fail" } else { stb }, B { size: 8.0, r: 6.0, ..Default::default() }));
                }
                trunk_end = bus_y;
            }
            by += 26.0 + (rows as f64 - 1.0) * 46.0 + 52.0;
        }
        parts.push(wire(&[[trunk, man_y + 8.0], [trunk, trunk_end]], trunk_st));
    }
    parts.push(format!("<path d=\"M500,20 L500,{}\" stroke=\"{NS_GREY}\" stroke-width=\"1\" stroke-dasharray=\"4 6\"/>", n(h - 20.0)));
    parts.extend(fg);
    if !title.is_empty() {
        parts.push(ex_t(w - 16.0, h - 10.0, title, T { anchor: "end", size: 12.0, fill: NS_GREY_TEXT, weight: 500.0, ..TD }));
    }
    let focus = match t {
        Some(t) if t.single.is_some() && !t.of(t.single.unwrap()).is_empty() => Some(vec![if t.single == Some("G") { 6.0 } else { 506.0 }, 6.0, 488.0, h - 50.0, 0.0]),
        _ => None,
    };
    ns_svg(w, h, &parts, focus.as_deref())
}

pub fn hyd_diagram(name: &str, title: &str) -> Option<Node> {
    hyd_targets(name).map(|t| hyd_view(Some(&t), title))
}

pub fn hyd_separation_leaks(list: &[FailIn], fire_pb: &[bool; 4]) -> Option<HydT> {
    let mut t = HydT::default();
    let side = |i: usize| if i > 0 { "R" } else { "L" };
    let tear = |t: &mut HydT, sys: &str, label: Option<String>| {
        let m = t.of_mut(sys);
        m.set("U:FLIGHT", "LINES TORN");
        m.set("RSVR", "warn");
        if let Some(l) = label {
            m.set(&format!("A:{l}"), "fail");
        }
    };
    let ail = ["IN", "MID", "OUT"];
    for f in list {
        let id = f.id as i64;
        if (54000..54004).contains(&id) {
            let k = (id - 53999) as usize;
            let sys = if k <= 2 { "G" } else { "Y" };
            let m = t.of_mut(sys);
            m.set(&format!("EDP{k}A"), "fail");
            m.set(&format!("EDP{k}B"), "fail");
            if fire_pb[k - 1] {
                m.set(&format!("FSOV{k}"), "shut");
                m.set("NOTE", &format!("ENG {k} SEPARATED|FIRE P/B OUT \u{b7} LINES SHUT"));
            } else {
                m.set("RSVR", "warn");
                m.set("NOTE", &format!("ENG {k} SEPARATED|LINES OPEN TILL FIRE P/B"));
            }
        } else if (57300..57306).contains(&id) {
            let s = side(((id - 57300) / 3) as usize);
            let k = ((id - 57300) % 3) as usize;
            tear(&mut t, "G", Some(format!("{s} {} AIL", ail[k])));
            tear(&mut t, "Y", Some(format!("{s} {} AIL", ail[k])));
        } else if (57350..57356).contains(&id) {
            let s = side(((id - 57350) / 3) as usize);
            for k in 0..3 {
                tear(&mut t, "G", Some(format!("{s} {} AIL", ail[k])));
                tear(&mut t, "Y", Some(format!("{s} {} AIL", ail[k])));
            }
            for k in [1, 3, 7][((id - 57350) % 3) as usize]..=8 {
                tear(&mut t, if k % 2 == 1 { "Y" } else { "G" }, Some(format!("{s} SPLR {k}")));
            }
        } else if (57330..57332).contains(&id) {
            let s = side((id - 57330) as usize);
            for k in [1, 2] {
                tear(&mut t, "G", Some(format!("{s} {} AIL", ail[k])));
                tear(&mut t, "Y", Some(format!("{s} {} AIL", ail[k])));
            }
        } else if (55310..55314).contains(&id) {
            let i = ((id - 55310) / 2) as usize;
            tear(&mut t, if i > 0 { "Y" } else { "G" }, Some(format!("{} {} ELEV", side(i), if (id - 55310) % 2 == 1 { "OUT" } else { "IN" })));
        } else if (55320..55322).contains(&id) {
            let i = (id - 55320) as usize;
            for p in ["IN", "OUT"] {
                tear(&mut t, if i > 0 { "Y" } else { "G" }, Some(format!("{} {p} ELEV", side(i))));
            }
        } else if (57310..57326).contains(&id) {
            let k = (id - 57310) % 8 + 1;
            tear(&mut t, if k % 2 == 1 { "Y" } else { "G" }, Some(format!("{} SPLR {k}", side(((id - 57310) / 8) as usize))));
        } else if id == 55330 || id == 55300 || id == 55301 {
            let both = id == 55330;
            if both || id == 55300 {
                tear(&mut t, "G", Some("UPPER RUD".into()));
                tear(&mut t, "Y", None);
            }
            if both || id == 55301 {
                tear(&mut t, "Y", Some("LOWER RUD".into()));
                tear(&mut t, "G", None);
            }
        }
    }
    if t.g.is_empty() && t.y.is_empty() { None } else { Some(t) }
}

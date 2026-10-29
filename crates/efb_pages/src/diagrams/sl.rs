use super::{ctx, ex_side_view, ex_t, ex_units, ns_box, ns_elbow, ns_stroke, ns_svg, ns_wire, OMap, B, NS_GREEN, NS_GREY_TEXT, NS_ORANGE, T, TD};
use super::{air::air_targets, elec::elec_targets, fctl::fctl_targets, fuel::fuel_targets, hyd::hyd_targets};
use crate::js::el;
use crate::jv::n;
use crate::{label, Node};
use regex::Regex;
use std::collections::HashSet;

pub(crate) struct Merged {
    pub names: Vec<String>,
    pub elec: super::ElecT,
    pub hyd: super::HydT,
    pub fuel: OMap,
    pub air: OMap,
    pub fctl: OMap,
}

fn merge(list: &[serde_json::Value]) -> Merged {
    let names: Vec<String> = list
        .iter()
        .filter(|f| crate::jv::b(f, "active"))
        .map(|f| {
            for k in ["name", "title"] {
                if crate::jv::b(f, k) {
                    return crate::jv::sv(f, k, "");
                }
            }
            String::new()
        })
        .collect();
    let mut m = Merged { names: names.clone(), elec: Default::default(), hyd: Default::default(), fuel: OMap::default(), air: OMap::default(), fctl: OMap::default() };
    let is = |nm: &str, r: &Regex| r.is_match(&nm.to_uppercase());
    for nm in &names {
        if is(nm, re!(r"FIRE|SMOKE")) || !is(nm, re!(r"ELEC|\bGEN\b|GENERATOR|\bVFG|\bBUS\b|\bTR\b|\bBAT|INV|CONTACTOR|EXT PWR|\bRAT\b|\bAC \d|\bDC \d|\bAC ESS|\bDC ESS")) {
            continue;
        }
        let e = elec_targets(nm);
        m.elec.dead.extend(e.dead);
        m.elec.degraded.extend(e.degraded);
        for (k, v) in e.edge.0 {
            m.elec.edge.set(&k, &v);
        }
    }
    for nm in &names {
        if is(nm, re!(r"FIRE|SMOKE")) {
            continue;
        }
        if is(nm, re!(r"HYD|EDP|PUMP|RSVR|RESERVOIR|ACCU|PRIORITY|HX|HEAT EXCH")) && !is(nm, re!(r"FUEL|BOOST|XFR")) {
            if let Some(h) = hyd_targets(nm) {
                let sys = h.single.unwrap_or("G");
                let src = if sys == "G" { h.g } else { h.y };
                let dst = if sys == "G" { &mut m.hyd.g } else { &mut m.hyd.y };
                for (k, v) in src.0 {
                    dst.set(&k, &v);
                }
            }
        }
        if is(nm, re!(r"FUEL|TANK|XFR|JETT|BOOST|CROSSFEED|X ?FEED|REFUEL")) {
            if let Some(f) = fuel_targets(nm) {
                for (k, v) in f.0 {
                    m.fuel.set(&k, &v);
                }
            }
        }
        if is(nm, re!(r"AIR|BLEED|PACK|COND|VENT|PRESS|OUTFL|DUCT|PRECOOLER|AVNCS|CAB FAN|TRIM AIR|HP VALVE|IP CHECK|FAV|XBLEED")) {
            if let Some(a) = air_targets(nm) {
                for (k, v) in a.0 {
                    m.air.set(&k, &v);
                }
            }
        }
        if is(nm, re!(r"F/CTL|FCTL|AILERON|\bAIL\b|ELEV|RUDDER|\bRUD\b|SPLR|SPOILER|SLAT|FLAP|\bTHS\b|\bPRIM|\bSEC\b|\bEHA|EBHA|SIDESTICK")) {
            if let Some(c) = fctl_targets(nm) {
                for (k, v) in c.0 {
                    m.fctl.set(&k, &v);
                }
            }
        }
    }
    m
}

fn soft(u: &str) -> bool {
    re!(r"DEGRAD|WEAR|DRIFT|LEAK|LOW|LO |FAULT|HOT|OVERHEAT|DET").is_match(u)
}

fn state(names: &[String], r: &dyn Fn(&str) -> bool) -> &'static str {
    let u: Vec<String> = names.iter().map(|x| x.to_uppercase()).collect();
    if u.iter().any(|x| r(x) && !soft(x)) {
        return "fail";
    }
    if u.iter().any(|x| r(x) && soft(x)) {
        return "warn";
    }
    "live"
}

fn st_re(names: &[String], r: &Regex) -> &'static str {
    state(names, &|x| r.is_match(x))
}

fn hits(names: &[String], r: &dyn Fn(&str) -> bool) -> bool {
    names.iter().any(|x| r(&x.to_uppercase()))
}

fn b(size: f64) -> B<'static> {
    B { size, ..Default::default() }
}

fn gear(t: &Merged) -> Node {
    let nm = &t.names;
    let (w, h) = (1000.0, 760.0);
    let mut parts = Vec::new();
    let mut fg = Vec::new();
    let gears: [(&str, f64, f64, usize, &Regex); 5] = [
        ("NLG", 500.0, 90.0, 2, re!(r"NOSE|NLG")),
        ("WLG L", 330.0, 380.0, 4, re!(r"(WING|WLG).*(L|LEFT)\b|LEFT WING GEAR|WLG ?L")),
        ("WLG R", 670.0, 380.0, 4, re!(r"(WING|WLG).*(R|RIGHT)\b|RIGHT WING GEAR|WLG ?R")),
        ("BLG L", 420.0, 520.0, 6, re!(r"(BODY|BLG).*(L|LEFT)\b|BLG ?L")),
        ("BLG R", 580.0, 520.0, 6, re!(r"(BODY|BLG).*(R|RIGHT)\b|BLG ?R")),
    ];
    let brake = st_re(nm, re!(r"BRAKE"));
    for (lbl, x, y, wheels, r) in gears {
        let st = st_re(nm, r);
        let rows = wheels / 2;
        let tyre_re = Regex::new(&format!("(TIRE|TYRE).*{}", lbl.replacen(' ', ".*", 1))).unwrap();
        let tyre = st_re(nm, &tyre_re);
        for rr in 0..rows {
            for c in 0..2 {
                let wx = x - 40.0 + c as f64 * 50.0;
                let wy = y + rr as f64 * 44.0;
                let braked = lbl != "NLG" && !(lbl.starts_with("BLG") && rr == 1);
                fg.push(ns_box(wx, wy, 30.0, 36.0, "", if tyre == "live" { st } else { tyre }, B { r: 7.0, ..Default::default() }));
                if braked {
                    fg.push(format!(
                        "<rect x=\"{}\" y=\"{}\" width=\"12\" height=\"12\" rx=\"3\" fill=\"{}\" opacity=\"0.85\"/>",
                        n(wx + 9.0),
                        n(wy + 12.0),
                        if brake == "live" { NS_GREEN } else { NS_ORANGE }
                    ));
                }
            }
        }
        fg.push(ex_t(x - 15.0, y - 14.0, lbl, T { size: 13.0, fill: ns_stroke(st), ..TD }));
    }
    let steer = st_re(nm, re!(r"STEER|NWS|BWS"));
    fg.push(ns_box(600.0, 70.0, 150.0, 40.0, "NOSE STEERING", steer, B::default()));
    fg.push(ns_box(720.0, 560.0, 170.0, 40.0, "BODY WHEEL STEER", steer, B::default()));
    fg.push(ns_box(80.0, 200.0, 160.0, 40.0, "GEAR DOORS", st_re(nm, re!(r"GEAR DOOR|L/G DOOR")), B::default()));
    fg.push(ns_box(80.0, 260.0, 160.0, 40.0, "L/G CTL (LGCIU)", st_re(nm, re!(r"LGCIU|L/G CTL|GEAR CTL|PROXIMITY")), B::default()));
    fg.push(ns_box(80.0, 320.0, 160.0, 40.0, "GRAVITY EXT", st_re(nm, re!(r"GRAVITY|FREEFALL")), B::default()));
    let g = if t.hyd.g.is_empty() { "live" } else { "warn" };
    let y = if t.hyd.y.is_empty() { "live" } else { "warn" };
    fg.push(ns_box(80.0, 470.0, 160.0, 40.0, "GREEN HYD", g, B::default()));
    fg.push(ns_box(80.0, 530.0, 160.0, 40.0, "YELLOW HYD", y, B::default()));
    fg.push(ns_box(260.0, 650.0, 150.0, 40.0, "NORMAL BRAKES", st_re(nm, re!(r"NORM(AL)? BRAKE|BRAKE.*NORM|BCS")), B::default()));
    fg.push(ns_box(430.0, 650.0, 150.0, 40.0, "ALTN BRAKES", st_re(nm, re!(r"ALTN BRAKE|ALTERNATE BRAKE")), B::default()));
    fg.push(ns_box(600.0, 650.0, 150.0, 40.0, "BRAKE ACCU", st_re(nm, re!(r"ACCU")), B::default()));
    fg.push(ns_box(770.0, 650.0, 150.0, 40.0, "ANTI SKID", st_re(nm, re!(r"SKID")), B::default()));
    parts.push(ns_elbow([240.0, 490.0], [260.0, 670.0], g, ""));
    parts.push(ns_elbow([240.0, 550.0], [430.0, 670.0], y, ""));
    parts.extend(fg);
    ns_svg(w, h, &parts, None)
}

fn apu_body(u: &str) -> bool {
    let b = u.as_bytes();
    let mut i = 0;
    while let Some(p) = u[i..].find("APU") {
        let at = i + p + 3;
        let rest = &u[at..];
        if ![" GEN", " BLEED", " FIRE", " BAT", " TR"].iter().any(|x| rest.starts_with(x)) {
            return true;
        }
        i = at;
        if i >= b.len() {
            break;
        }
    }
    false
}

fn apu(t: &Merged) -> Node {
    let nm = &t.names;
    let s = |r: &Regex| st_re(nm, r);
    let mut parts = Vec::new();
    let mut fg = Vec::new();
    fg.push(ns_box(390.0, 250.0, 220.0, 110.0, "APU\nPW980", state(nm, &apu_body), b(16.0)));
    fg.push(ns_box(120.0, 120.0, 160.0, 44.0, "AIR INTAKE FLAP", s(re!(r"INTAKE|FLAP")), B::default()));
    fg.push(ns_box(120.0, 200.0, 160.0, 44.0, "ECB", s(re!(r"ECB|APU CTL|FADEC")), B::default()));
    fg.push(ns_box(120.0, 280.0, 160.0, 44.0, "START MOTOR 1", s(re!(r"STARTER ?1|START MOTOR ?1")), B::default()));
    fg.push(ns_box(120.0, 340.0, 160.0, 44.0, "START MOTOR 2", s(re!(r"STARTER ?2|START MOTOR ?2")), B::default()));
    fg.push(ns_box(120.0, 440.0, 160.0, 44.0, "FUEL LP VALVE", s(re!(r"APU.*(LP|FUEL)|FUEL.*APU")), B::default()));
    fg.push(ns_box(120.0, 520.0, 160.0, 44.0, "APU BAT", s(re!(r"APU BAT|BAT APU")), B::default()));
    fg.push(ns_box(720.0, 120.0, 170.0, 44.0, "APU GEN A", s(re!(r"APU GEN ?(A|1)")), B::default()));
    fg.push(ns_box(720.0, 200.0, 170.0, 44.0, "APU GEN B", s(re!(r"APU GEN ?(B|2)")), B::default()));
    fg.push(ns_box(720.0, 300.0, 170.0, 44.0, "BLEED VALVE", s(re!(r"APU BLEED")), B::default()));
    fg.push(ns_box(720.0, 380.0, 170.0, 44.0, "OIL", s(re!(r"APU OIL")), B::default()));
    fg.push(ns_box(720.0, 480.0, 170.0, 44.0, "FIRE LOOP", s(re!(r"APU FIRE|APU.*LOOP")), B::default()));
    fg.push(ns_box(720.0, 560.0, 170.0, 44.0, "FIRE BOTTLE", s(re!(r"APU.*(BOTTLE|AGENT|SQUIB)")), B::default()));
    for y in [142.0, 222.0, 302.0, 362.0, 462.0, 542.0] {
        parts.push(ns_elbow([280.0, y], [390.0, 305.0], "live", ""));
    }
    for y in [142.0, 222.0, 322.0, 402.0, 502.0, 582.0] {
        parts.push(ns_elbow([610.0, 305.0], [720.0, y], "live", ""));
    }
    parts.extend(fg);
    ns_svg(1000.0, 640.0, &parts, None)
}

/// `FIRE` not followed anywhere later by `DET` or `LOOP` (the page's `FIRE(?!.*(DET|LOOP))`).
fn fire_not_det(u: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(p) = u[i..].find("FIRE") {
        let at = i + p;
        let rest = &u[at + 4..];
        if !rest.contains("DET") && !rest.contains("LOOP") {
            out.push(at);
        }
        i = at + 1;
    }
    out
}

fn fire(t: &Merged) -> Node {
    let nm = &t.names;
    let mut fg = Vec::new();
    for e in 1..=4 {
        let x = 40.0 + (e - 1) as f64 * 240.0;
        let eng = Regex::new(&format!(r"ENG(INE)? ?{e}")).unwrap();
        let zone = if hits(nm, &|u| eng.find_iter(u).any(|m| fire_not_det(u).iter().any(|f| *f >= m.end()))) { "fail" } else { "live" };
        fg.push(ns_box(x, 60.0, 200.0, 60.0, &format!("ENG {e}"), zone, b(16.0)));
        fg.push(ns_box(x, 140.0, 95.0, 40.0, "LOOP A", st_re(nm, &Regex::new(&format!(r"ENG(INE)? ?{e}.*LOOP ?A")).unwrap()), B::default()));
        fg.push(ns_box(x + 105.0, 140.0, 95.0, 40.0, "LOOP B", st_re(nm, &Regex::new(&format!(r"ENG(INE)? ?{e}.*LOOP ?B")).unwrap()), B::default()));
        fg.push(ns_box(x, 200.0, 95.0, 40.0, "BTL 1", st_re(nm, &Regex::new(&format!(r"ENG(INE)? ?{e}.*(BTL|BOTTLE|AGENT) ?1")).unwrap()), B::default()));
        fg.push(ns_box(x + 105.0, 200.0, 95.0, 40.0, "BTL 2", st_re(nm, &Regex::new(&format!(r"ENG(INE)? ?{e}.*(BTL|BOTTLE|AGENT) ?2")).unwrap()), B::default()));
    }
    let apu_fire = hits(nm, &|u| u.match_indices("APU FIRE").any(|(i, _)| fire_not_det(u).contains(&(i + 4))));
    fg.push(ns_box(40.0, 300.0, 200.0, 60.0, "APU", if apu_fire { "fail" } else { "live" }, b(16.0)));
    fg.push(ns_box(40.0, 380.0, 95.0, 40.0, "LOOP", st_re(nm, re!(r"APU.*LOOP|APU FIRE DET")), B::default()));
    fg.push(ns_box(145.0, 380.0, 95.0, 40.0, "BOTTLE", st_re(nm, re!(r"APU.*(BTL|BOTTLE|AGENT)")), B::default()));
    for (i, (lab, src)) in [("FWD CARGO", r"FWD CARGO|FWD\+AFT CARGO"), ("AFT CARGO", r"AFT CARGO|FWD\+AFT CARGO"), ("BULK", r"BULK")].iter().enumerate() {
        let smoke = Regex::new(&format!("SMOKE.*({src})|({src}).*SMOKE")).unwrap();
        let det = Regex::new(&format!("({src}).*(DET|SDF)")).unwrap();
        let st = if hits(nm, &|u| smoke.is_match(u)) { "fail" } else { st_re(nm, &det) };
        fg.push(ns_box(280.0 + i as f64 * 170.0, 300.0, 150.0, 60.0, lab, st, B::default()));
    }
    fg.push(ns_box(280.0, 380.0, 490.0, 40.0, "CARGO FIRE BOTTLES (2 DISCHARGE, 3 METERING)", st_re(nm, re!(r"CARGO.*(BTL|BOTTLE|AGENT)")), b(12.0)));
    fg.push(ns_box(800.0, 300.0, 160.0, 60.0, "AVIONICS\nSMOKE", if hits(nm, &|u| re!(r"AVNCS SMOKE|AVIONICS SMOKE").is_match(u)) { "fail" } else { "live" }, B::default()));
    fg.push(ns_box(800.0, 380.0, 160.0, 40.0, "LAV SMOKE", if hits(nm, &|u| re!(r"LAV.*SMOKE").is_match(u)) { "fail" } else { "live" }, B::default()));
    fg.push(ns_box(40.0, 470.0, 920.0, 40.0, "FIRE DETECTION UNITS  FDU 1 / FDU 2  /  SDF CIDS", st_re(nm, re!(r"FDU|FIRE DET(ECTION)? UNIT|SDF")), b(12.5)));
    ns_svg(1000.0, 540.0, &fg, None)
}

fn oxygen(t: &Merged) -> Node {
    let nm = &t.names;
    let s = |r: &Regex| st_re(nm, r);
    let mut parts = Vec::new();
    let mut fg = Vec::new();
    fg.push(ex_t(160.0, 40.0, "CREW", T { size: 15.0, fill: NS_GREY_TEXT, ..TD }));
    fg.push(ex_t(700.0, 40.0, "PASSENGERS", T { size: 15.0, fill: NS_GREY_TEXT, ..TD }));
    for k in 1..=3 {
        let r = Regex::new(&format!("CREW (OXY|O2).*(BTL|BOTTLE|CYL) ?{k}|CREW OXY LO PR")).unwrap();
        fg.push(ns_box(60.0 + (k - 1) as f64 * 110.0, 70.0, 95.0, 50.0, &format!("BTL {k}"), s(&r), B::default()));
    }
    fg.push(ns_box(110.0, 170.0, 200.0, 44.0, "CREW SUPPLY VALVE", s(re!(r"CREW (OXY|O2).*VALVE|CREW SUPPLY")), B::default()));
    for (i, m) in ["CAPT", "F/O", "3RD", "4TH", "5TH"].iter().enumerate() {
        let mm = m.replacen('/', ".", 1);
        let r = Regex::new(&format!("MASK.*{mm}|{mm}.*MASK")).unwrap();
        fg.push(ns_box(30.0 + i as f64 * 62.0, 270.0, 56.0, 40.0, m, s(&r), b(11.0)));
    }
    parts.push(ns_elbow([210.0, 120.0], [210.0, 170.0], "live", ""));
    for i in 0..5 {
        parts.push(ns_elbow([210.0, 214.0], [58.0 + i as f64 * 62.0, 270.0], "live", ""));
    }
    for k in 0..6 {
        fg.push(ns_box(480.0 + k as f64 * 80.0, 70.0, 70.0, 50.0, &format!("CYL {}", k + 1), s(re!(r"PAX (OXY|O2).*(BTL|BOTTLE|CYL|LO PR)")), b(12.0)));
    }
    fg.push(ns_box(560.0, 170.0, 320.0, 44.0, "PAX HIGH / LOW PRESSURE REGULATORS", s(re!(r"PAX (OXY|O2).*(REG|PRESS)")), b(12.0)));
    fg.push(ns_box(480.0, 270.0, 230.0, 50.0, "MAIN DECK MASKS", s(re!(r"MAIN DECK.*MASK|PAX MASK|PAX OXY")), B::default()));
    fg.push(ns_box(730.0, 270.0, 230.0, 50.0, "UPPER DECK MASKS", s(re!(r"UPPER DECK.*MASK|PAX MASK|PAX OXY")), B::default()));
    fg.push(ns_box(560.0, 360.0, 320.0, 40.0, "PAX SYS ON  /  MASK MAN ON", s(re!(r"MASK MAN|PAX SYS")), b(12.0)));
    parts.push(ns_elbow([720.0, 120.0], [720.0, 170.0], "live", ""));
    parts.push(ns_elbow([720.0, 214.0], [595.0, 270.0], "live", ""));
    parts.push(ns_elbow([720.0, 214.0], [845.0, 270.0], "live", ""));
    parts.extend(fg);
    ns_svg(1000.0, 430.0, &parts, None)
}

fn doors(t: &Merged) -> Node {
    let mut hl = HashSet::new();
    for nm in &t.names {
        let u = nm.to_uppercase();
        for m in re!(r"\b([MU]\d[LR])\b").captures_iter(&u) {
            hl.insert(format!("D{}", &m[1]));
        }
        if re!(r"CARGO DOOR|FWD CARGO|AFT CARGO").is_match(&u) {
            hl.insert(if u.contains("AFT") { "CAFT" } else { "CFWD" }.to_string());
        }
        if u.contains("BULK") {
            hl.insert("CBULK".into());
        }
        if u.contains("AVNCS") {
            hl.insert("AVNCS".into());
        }
    }
    ex_side_view(&hl, false)
}

fn anti_ice(t: &Merged) -> Node {
    let nm = &t.names;
    let s = |r: &Regex| st_re(nm, r);
    let mut fg = Vec::new();
    let mut parts = Vec::new();
    for e in 1..=4 {
        let r = Regex::new(&format!(r"ENG(INE)? ?{e}.*(NAI|ANTI ?ICE|A\.ICE)")).unwrap();
        fg.push(ns_box(40.0 + (e - 1) as f64 * 240.0, 60.0, 200.0, 50.0, &format!("ENG {e} NAI"), s(&r), B::default()));
    }
    fg.push(ns_box(120.0, 170.0, 300.0, 50.0, "WING A-ICE L", s(re!(r"WING.*(ANTI ?ICE|A\.ICE).*(L|LEFT)|WING A-ICE")), B::default()));
    fg.push(ns_box(580.0, 170.0, 300.0, 50.0, "WING A-ICE R", s(re!(r"WING.*(ANTI ?ICE|A\.ICE).*(R|RIGHT)|WING A-ICE")), B::default()));
    for (i, sd) in ["CAPT", "F/O", "STBY"].iter().enumerate() {
        let x = sd.replacen('/', ".", 1);
        let r = Regex::new(&format!("{x}.*(PROBE|PITOT|AOA|TAT|STATIC)|PROBE.*{x}|PHC ?{}", i + 1)).unwrap();
        fg.push(ns_box(120.0 + i as f64 * 260.0, 290.0, 240.0, 50.0, &format!("{sd} PROBES HEAT"), s(&r), B::default()));
    }
    fg.push(ns_box(120.0, 380.0, 360.0, 50.0, "WINDSHIELD / WINDOWS HEAT", s(re!(r"WINDSHIELD|WINDOW HEAT|WSHLD|WHC")), B::default()));
    fg.push(ns_box(520.0, 380.0, 360.0, 50.0, "RAIN REPELLENT / WIPERS", s(re!(r"WIPER|RAIN")), B::default()));
    fg.push(ns_box(120.0, 470.0, 760.0, 44.0, "ICE DETECTORS 1 / 2", s(re!(r"ICE DET")), B::default()));
    parts.push(ns_wire(&[[140.0, 85.0], [860.0, 85.0]], "off", "true", true));
    parts.extend(fg);
    ns_svg(1000.0, 540.0, &parts, None)
}

fn engines(t: &Merged) -> Node {
    let mut host = el("div", "sl-engs");
    for e in 1..=4 {
        let list: Vec<super::FailIn> = t.names.iter().enumerate().map(|(i, name)| super::FailIn { id: (i + 1) as f64, name: name.clone(), active: true, magnitude: 1.0 }).collect();
        host = host.child(super::engine_svg(&super::EngOpts { n1: 85.0, n3: 96.0, faults: super::eng_faults(&list, e), title: format!("ENGINE {e}"), ..Default::default() }));
    }
    host
}

fn draw(title: &str, t: &Merged) -> Option<Node> {
    Some(match title {
        "ELECTRICAL" => super::elec_view(&t.elec, &super::elec::ElecOpts { title: "ELECTRICAL NETWORK".into(), full: true, loads: super::elec::elec_loads_lost(&t.elec) }),
        "HYDRAULICS" => super::hyd_view(Some(&t.hyd), "HYDRAULIC SYSTEMS"),
        "FUEL" => super::fuel_view(&t.fuel, "FUEL SYSTEM"),
        "FLIGHT CONTROLS" => super::fctl_plan_view(&t.fctl, "FLIGHT CONTROLS"),
        "BLEED AND PACKS" => super::bleed_view(&t.air, "PNEUMATIC SYSTEM"),
        "CABIN AIR AND VENTILATION" => super::vent_view(&t.air, "VENTILATION"),
        "ENGINE" => engines(t),
        "LANDING GEAR AND BRAKES" => gear(t),
        "APU" => apu(t),
        "FIRE PROTECTION" => fire(t),
        "OXYGEN" => oxygen(t),
        "DOORS" => doors(t),
        "ANTI ICE AND RAIN" => anti_ice(t),
        "NAVIGATION" => ex_units("NAV", &t.names.join(" "))?,
        "SURVEILLANCE" => ex_units("SURV", &t.names.join(" "))?,
        "COMMUNICATIONS" => ex_units("COM", &t.names.join(" "))?,
        "AUTO FLIGHT" => ex_units("AUTO FLT", &t.names.join(" "))?,
        "WARNING AND RECORDING" => ex_units("FWS", &t.names.join(" "))?,
        _ => return None,
    })
}

/// `slLive(draw)`: the map from the active failures plus the footer naming them.
pub(crate) fn sl_live(title: &str) -> Node {
    let list = ctx(|c| c.failures.clone());
    let t = merge(&list);
    let mut host = el("div", "sl-live");
    let Some(d) = draw(title, &t) else { return host };
    let foot = if t.names.is_empty() {
        "LIVE  /  NO ACTIVE FAILURES".to_string()
    } else {
        format!(
            "ACTIVE: {}{}",
            t.names.iter().take(12).cloned().collect::<Vec<_>>().join("  /  "),
            if t.names.len() > 12 { format!("  +{}", t.names.len() - 12) } else { String::new() }
        )
    };
    host = host.child(d).child(label("sl-foot", &foot));
    host
}

use super::{ex_t, ns_bounds, ns_box, ns_stroke, ns_svg, ns_valve, wire, wire_d, OMap, B, NS_GREY_TEXT, NS_ORANGE, NS_RED, NS_GREEN, T, TD};
use crate::js::el;
use crate::jv::{fx, n};
use crate::Node;
use serde_json::{Map, Value};

pub fn air_targets(name: &str) -> Option<OMap> {
    let up = name.to_uppercase();
    let n = format!(" {} ", re!(r"[_-]").replace_all(&up, " ").replace("FOWARD", "FORWARD"));
    let n = n.as_str();
    let mut m = OMap::default();
    let soft = re!(r"DEGRAD|FOULING|DRIFT|SENSOR|INSULATION|BLOCKAGE|CLOG|FAULT$").is_match(n.trim()) && !re!(r"STUCK|LEAK|RUPTURE|FAILURE").is_match(n);
    let st = if soft { "warn" } else { "fail" };
    let eng = re!(r"ENG(?:INE)? ?(\d)").captures(n).map(|c| c[1].to_string());
    let side = if re!(r"\bLEFT\b|\bL\b").is_match(n) {
        Some("L")
    } else if re!(r"\bRIGHT\b|\bR\b").is_match(n) {
        Some("R")
    } else if re!(r"CENT(?:RE|ER)").is_match(n) {
        Some("C")
    } else {
        None
    };
    let pack = re!(r"PACK ?(\d)").captures(n).map(|c| c[1].to_string());
    let has = |s: &str| n.contains(s);
    let k: String = if let (Some(e), true) = (&eng, has("START DUCT")) {
        format!("START{e}")
    } else if eng.is_some() && re!(r"DUCT|RUPTURE|INSULATION").is_match(n) && re!(r"BLEED|PYLON|ENGINE").is_match(n) {
        format!("DUCT{}", eng.as_ref().unwrap())
    } else if eng.is_some() && re!(r"HP (?:BLEED )?VALVE|HP VALVE").is_match(n) {
        format!("HP{}", eng.as_ref().unwrap())
    } else if eng.is_some() && re!(r"IP8|IP CHECK").is_match(n) {
        format!("IPC{}", eng.as_ref().unwrap())
    } else if eng.is_some() && re!(r"FAN AIR VALVE|\bFAV\b").is_match(n) {
        format!("FAV{}", eng.as_ref().unwrap())
    } else if eng.is_some() && has("PRECOOLER") {
        format!("PCLR{}", eng.as_ref().unwrap())
    } else if eng.is_some() && re!(r"NACELLE|ANTI ICE").is_match(n) {
        format!("NAI{}", eng.as_ref().unwrap())
    } else if eng.is_some() && re!(r"PR |SHUTOFF|BLEED VALVE|PRV|BLEED").is_match(n) {
        format!("PRV{}", eng.as_ref().unwrap())
    } else if re!(r"CROSS ?BLEED|XBLEED|CROSS\b").is_match(n) {
        format!("XB{}", side.unwrap_or("C"))
    } else if re!(r"APU BLEED|APU .*BLEED").is_match(n) {
        "APUB".into()
    } else if has("WING") && re!(r"ANTI ICE|A/I").is_match(n) {
        format!("WAI{}", side.unwrap_or("L"))
    } else if let Some(c) = re!(r"(?:PACK )?FCV ?(\d) ?(\d)|PACK FCV (\d) (\d)").captures(n) {
        let a = c.get(1).or(c.get(3)).map(|x| x.as_str()).unwrap_or("");
        let b = c.get(2).or(c.get(4)).map(|x| x.as_str()).unwrap_or("");
        format!("FCV{a}{b}")
    } else if pack.is_some() && has("RAM AIR") {
        format!("RAM{}", pack.as_ref().unwrap())
    } else if let Some(p) = &pack {
        format!("PACK{p}")
    } else if let Some(c) = re!(r"HOT AIR VALVE ?(\d)").captures(n) {
        format!("HAV{}", &c[1])
    } else if has("HOT AIR") {
        "HAV1".into()
    } else if re!(r"ODLS|OVERHEAT DETECTION").is_match(n) {
        format!("ODLS{}", re!(r"LOOP ([AB])").captures(n).map(|c| c[1].to_string()).unwrap_or("A".into()))
    } else if let Some(c) = re!(r"CAB(?:IN)? FAN ?(\d)").captures(n) {
        format!("CF{}", &c[1])
    } else if re!(r"OUTFLOW|\bOFV\b").is_match(n) {
        format!("OFV{}", re!(r"(\d)").captures(n).map(|c| c[1].to_string()).unwrap_or("1".into()))
    } else if re!(r"\bCPC\b|PRESSURE CONTROLLER|CABIN .*PRESS|DECOMPRESSION|OCSM").is_match(n) {
        "CPC".into()
    } else if has("AVIONICS") && re!(r"FAN|EXTRACT").is_match(n) {
        format!("AV{}{}", if has("AFT") { "AFT" } else if re!(r"MAIN|UPPER").is_match(n) { "MAIN" } else { "FWD" }, if has("STANDBY") { "S" } else { "P" })
    } else if re!(r"EXTRACT VALVE|INLET VALVE|SKIN").is_match(n) {
        "AVVALVE".into()
    } else if has("BULK") && has("HEATER") {
        "BULKHEAT".into()
    } else if has("CARGO") && has("ISOLATION") {
        if re!(r"BULK|AFT").is_match(n) { "CISO_AFT" } else { "CISO_FWD" }.into()
    } else if has("CARGO") && re!(r"FAN|EXTRACT").is_match(n) {
        if re!(r"BULK|AFT").is_match(n) { "CFAN_AFT" } else { "CFAN_FWD" }.into()
    } else if re!(r"VCM|TADD|FDAC|TRIM AIR|ZONE|TEMP").is_match(n) {
        "TRIM".into()
    } else if has("MIXER") {
        "MIXER".into()
    } else {
        return None;
    };
    m.set(&k, st);
    Some(m)
}

pub fn bleed_view(m: &OMap, title: &str) -> Node {
    let (w, h) = (1000.0, 640.0);
    let mut wires = Vec::new();
    let mut fg = Vec::new();
    let s = |k: &str| m.st(k, "live").to_string();
    let sd = |k: &str, d: &str| m.st(k, d).to_string();
    let fail = |k: &str| m.get(k) == Some("fail");
    let eng_x = [110.0, 310.0, 660.0, 870.0];
    let man_y = 330.0;
    let xb_open = ["XBL", "XBC", "XBR"].iter().any(|k| fail(k)) || (1..=4).any(|e| fail(&format!("PRV{e}")));
    for (i, x) in eng_x.iter().copied().enumerate() {
        let e = i + 1;
        let dead = ["PRV", "DUCT", "PCLR"].iter().any(|p| fail(&format!("{p}{e}")));
        let flow = if dead { "warn" } else { "live" };
        let k = |p: &str| format!("{p}{e}");
        fg.push(ns_box(x - 50.0, 22.0, 100.0, 34.0, &format!("ENG {e}"), "live", B::default()));
        wires.push(wire(&[[x - 22.0, 56.0], [x - 22.0, 92.0]], if s(&k("IPC")) == "fail" { "fail" } else { "live" }));
        wires.push(wire(&[[x + 22.0, 56.0], [x + 22.0, 92.0]], if s(&k("HP")) == "fail" { "fail" } else { "live" }));
        fg.push(ns_valve(x - 22.0, 78.0, &s(&k("IPC")), true, "IP8", true));
        fg.push(ns_valve(x + 22.0, 78.0, &s(&k("HP")), m.get(&k("HP")) != Some("fail"), "HP6", false));
        wires.push(wire(&[[x - 22.0, 92.0], [x + 22.0, 92.0]], "live"));
        wires.push(wire(&[[x, 92.0], [x, 172.0]], if fail(&k("PRV")) { "fail" } else { "live" }));
        fg.push(ns_valve(x, 126.0, &s(&k("PRV")), !fail(&k("PRV")), "PRV", false));
        let tag = match m.get(&k("PCLR")) {
            Some("fail") => "FAILED",
            Some(_) => "DEGRADED",
            None => "",
        };
        fg.push(ns_box(x - 46.0, 172.0, 92.0, 38.0, "PRECOOLER", &s(&k("PCLR")), B { size: 11.0, tag, ..Default::default() }));
        wires.push(wire(&[[x + 46.0, 191.0], [x + 70.0, 191.0], [x + 70.0, 140.0]], if s(&k("FAV")) == "fail" { "fail" } else { "live" }));
        fg.push(ns_valve(x + 70.0, 152.0, &s(&k("FAV")), !fail(&k("FAV")), "FAV", false));
        wires.push(wire(&[[x - 50.0, 39.0], [x - 70.0, 39.0], [x - 70.0, 110.0]], if s(&k("NAI")) == "fail" { "fail" } else { "live" }));
        fg.push(ns_valve(x - 70.0, 96.0, &s(&k("NAI")), false, "NAI", true));
        let duct = sd(&k("DUCT"), flow);
        wires.push(wire_d(&[[x, 210.0], [x, man_y]], if duct == "fail" { "fail" } else { flow }, if duct == "fail" { "5 5" } else { "" }));
        if m.get(&k("DUCT")).is_some() {
            fg.push(ex_t(x + 8.0, 270.0, "DUCT LEAK", T { size: 10.0, anchor: "start", fill: NS_RED, ..TD }));
        }
        wires.push(wire(&[[x, 250.0], [x + 34.0, 250.0], [x + 34.0, 290.0]], if s(&k("START")) == "fail" { "fail" } else { "off" }));
        fg.push(ns_valve(x + 34.0, 276.0, &sd(&k("START"), "off"), false, "START", false));
    }
    wires.push(wire(&[[30.0, man_y], [970.0, man_y]], "live"));
    fg.push(ex_t(500.0, man_y - 8.0, "CROSSBLEED MANIFOLD", T { size: 10.0, fill: NS_GREY_TEXT, ..TD }));
    for (k, x, label) in [("XBL", 195.0, "XBLEED L"), ("XBC", 500.0, "XBLEED C"), ("XBR", 765.0, "XBLEED R")] {
        fg.push(ns_valve(x, man_y, &sd(k, if xb_open { "alt" } else { "off" }), xb_open && !fail(k), label, false));
    }
    for (k, x, label) in [("WAIL", 30.0, "WING A/I L"), ("WAIR", 970.0, "WING A/I R")] {
        wires.push(wire(&[[x, man_y], [x, man_y + 70.0]], if s(k) == "fail" { "fail" } else { "live" }));
        fg.push(ns_valve(x, man_y + 36.0, &sd(k, "off"), false, "", false));
        fg.push(ns_box(if x == 30.0 { 10.0 } else { 910.0 }, man_y + 70.0, 80.0, 30.0, label, &s(k), B { size: 10.0, ..Default::default() }));
    }
    wires.push(wire(&[[560.0, man_y], [560.0, 460.0]], if s("APUB") == "fail" { "fail" } else { "off" }));
    fg.push(ns_valve(560.0, 400.0, &sd("APUB", "off"), false, "APU BLEED", false));
    fg.push(ns_box(520.0, 460.0, 80.0, 32.0, "APU", "off", B::default()));
    wires.push(wire(&[[440.0, man_y], [440.0, 460.0]], "off"));
    fg.push(ns_valve(440.0, 400.0, "off", false, "GND HP", true));
    fg.push(ns_box(400.0, 460.0, 80.0, 32.0, "GND CART", "off", B { size: 10.0, ..Default::default() }));
    for (p, x) in [(1, 230.0), (2, 730.0)] {
        let pack_st = s(&format!("PACK{p}"));
        for (j, dx) in [(1, -30.0), (2, 30.0)] {
            let k = format!("FCV{p}{j}");
            wires.push(wire(&[[x + dx, man_y], [x + dx, 470.0]], if s(&k) == "fail" { "fail" } else { "live" }));
            fg.push(ns_valve(x + dx, 400.0, &s(&k), !fail(&k), &format!("FCV {p}-{j}"), dx < 0.0));
        }
        fg.push(ns_box(x - 70.0, 470.0, 140.0, 46.0, &format!("PACK {p}\nACM"), &pack_st, B { size: 12.0, tag: if m.get(&format!("PACK{p}")).is_some() { "FAULT" } else { "" }, ..Default::default() }));
        wires.push(wire(&[[x + 70.0, 493.0], [x + 96.0, 493.0]], if s(&format!("RAM{p}")) == "fail" { "fail" } else { "live" }));
        fg.push(ns_valve(x + 96.0, 493.0, &s(&format!("RAM{p}")), true, "RAM AIR", false));
        wires.push(wire(&[[x, 516.0], [x, 560.0], [500.0, 560.0]], if pack_st == "fail" { "warn" } else { "live" }));
    }
    fg.push(ns_box(440.0, 560.0, 120.0, 40.0, "MIXER UNIT", &s("MIXER"), B { size: 12.0, ..Default::default() }));
    for (k, x) in [("HAV1", 350.0), ("HAV2", 650.0)] {
        wires.push(wire(&[[x, man_y], [x, 430.0]], if s(k) == "fail" { "fail" } else { "live" }));
        fg.push(ns_valve(x, 380.0, &s(k), !fail(k), if k == "HAV1" { "HOT AIR 1" } else { "HOT AIR 2" }, false));
        fg.push(ex_t(x, 446.0, "TRIM AIR", T { size: 9.0, fill: NS_GREY_TEXT, ..TD }));
    }
    for (k, y) in [("ODLSA", man_y + 12.0), ("ODLSB", man_y + 18.0)] {
        wires.push(wire_d(&[[40.0, y], [960.0, y]], if m.get(k).is_some() { "warn" } else { "off" }, "2 6"));
    }
    if m.get("ODLSA").is_some() || m.get("ODLSB").is_some() {
        fg.push(ex_t(960.0, man_y + 34.0, "OVERHEAT DETECTION LOOP", T { size: 9.0, anchor: "end", fill: NS_ORANGE, ..TD }));
    }
    if !title.is_empty() {
        fg.push(ex_t(w - 16.0, h - 8.0, title, T { anchor: "end", size: 12.0, fill: NS_GREY_TEXT, weight: 500.0, ..TD }));
    }
    let mut focus = Vec::new();
    for key in m.keys() {
        let e: Option<usize> = re!(r"(\d)$").captures(key).and_then(|c| c[1].parse().ok());
        if re!(r"^(IPC|HP|PRV|FAV|PCLR|NAI|DUCT|START)\d$").is_match(key) {
            focus.push([eng_x[e.unwrap() - 1] - 90.0, 10.0, 200.0, 330.0]);
        } else if key.starts_with("XB") {
            focus.push([150.0, 280.0, 700.0, 120.0]);
        } else if key.starts_with("WAI") {
            focus.push([if key.ends_with('L') { 0.0 } else { 800.0 }, 280.0, 200.0, 150.0]);
        } else if re!(r"^(FCV|PACK|RAM)").is_match(key) {
            let d = re!(r"\d").find(key).map(|m| m.as_str()).unwrap_or("");
            focus.push([if d == "1" { 120.0 } else { 620.0 }, 300.0, 260.0, 230.0]);
        } else {
            focus.push([300.0, 300.0, 400.0, 300.0]);
        }
    }
    wires.extend(fg);
    ns_svg(w, h, &wires, ns_bounds(&focus).as_deref())
}

pub fn vent_view(m: &OMap, title: &str) -> Node {
    let (w, h) = (1000.0, 600.0);
    let mut wires = Vec::new();
    let mut fg = Vec::new();
    let s = |k: &str| m.st(k, "live").to_string();
    let sd = |k: &str, d: &str| m.st(k, d).to_string();
    fg.push(ns_box(440.0, 30.0, 120.0, 40.0, "MIXER UNIT", &s("MIXER"), B { size: 12.0, ..Default::default() }));
    wires.push(wire(&[[500.0, 70.0], [500.0, 110.0]], "live"));
    wires.push(wire(&[[120.0, 110.0], [880.0, 110.0]], "live"));
    let trim = s("TRIM");
    for (z, x) in [("COCKPIT", 60.0), ("MAIN FWD", 190.0), ("MAIN MID", 320.0), ("MAIN AFT", 450.0), ("UPPER FWD", 580.0), ("UPPER MID", 710.0), ("UPPER AFT", 840.0)] {
        wires.push(wire(&[[x + 50.0, 110.0], [x + 50.0, 160.0]], "live"));
        let st = if trim == "fail" { "warn" } else { "live" };
        fg.push(ns_valve(x + 50.0, 136.0, st, true, "", false));
        fg.push(ns_box(x, 160.0, 100.0, 36.0, z, st, B { size: 10.0, ..Default::default() }));
    }
    fg.push(ex_t(500.0, 104.0, "TRIM AIR", T { size: 10.0, fill: NS_GREY_TEXT, ..TD }));
    for f in 1..=4 {
        let x = 160.0 + (f - 1) as f64 * 200.0;
        let k = format!("CF{f}");
        let failed = s(&k) == "fail";
        wires.push(wire_d(&[[x + 40.0, 230.0], [x + 40.0, 110.0]], if failed { "fail" } else { "live" }, if failed { "5 5" } else { "" }));
        fg.push(ns_box(x, 230.0, 80.0, 30.0, &format!("CAB FAN {f}"), &s(&k), B { size: 10.0, tag: if m.get(&k).is_some() { "FAILED" } else { "" }, ..Default::default() }));
    }
    fg.push(ex_t(500.0, 278.0, "RECIRCULATION", T { size: 10.0, fill: NS_GREY_TEXT, ..TD }));
    let cpc = m.get("CPC").is_some();
    wires.push(wire(&[[60.0, 320.0], [940.0, 320.0]], if cpc { "warn" } else { "live" }));
    for o in 1..=4 {
        let x = 120.0 + (o - 1) as f64 * 240.0;
        let k = format!("OFV{o}");
        wires.push(wire(&[[x, 320.0], [x, 360.0]], if s(&k) == "fail" { "fail" } else { "live" }));
        fg.push(ns_valve(x, 340.0, &s(&k), true, &format!("OFV {o}"), false));
    }
    wires.push(wire(&[[880.0, 320.0], [880.0, 376.0]], if cpc { "warn" } else { "live" }));
    fg.push(ns_box(820.0, 376.0, 120.0, 30.0, "CPC 1 / 2", &s("CPC"), B { size: 11.0, tag: if cpc { "FAULT" } else { "" }, ..Default::default() }));
    fg.push(ex_t(60.0, 312.0, "CABIN PRESSURE", T { size: 10.0, anchor: "start", fill: NS_GREY_TEXT, ..TD }));
    fg.push(ex_t(40.0, 410.0, "AVIONICS VENTILATION", T { size: 11.0, anchor: "start", fill: NS_GREY_TEXT, ..TD }));
    for (bay, x) in [("FWD", 60.0), ("MAIN", 300.0), ("AFT", 540.0)] {
        let both = ["P", "S"].iter().all(|k| m.get(&format!("AV{bay}{k}")) == Some("fail"));
        fg.push(ns_box(x, 430.0, 100.0, 36.0, &format!("{bay} BAY"), if both { "warn" } else { "live" }, B { size: 11.0, ..Default::default() }));
        for (j, ps) in ["P", "S"].iter().enumerate() {
            let k = format!("AV{bay}{ps}");
            let jx = j as f64 * 64.0;
            wires.push(wire(&[[x + 100.0, 448.0], [x + 120.0 + jx, 448.0], [x + 120.0 + jx, 480.0]], if s(&k) == "fail" { "fail" } else if *ps == "P" { "live" } else { "off" }));
            fg.push(ns_box(x + 104.0 + jx, 480.0, 60.0, 28.0, if *ps == "P" { "PRIM" } else { "STBY" }, &sd(&k, if *ps == "P" { "live" } else { "off" }), B { size: 9.0, r: 7.0, ..Default::default() }));
        }
    }
    fg.push(ns_valve(820.0, 448.0, &s("AVVALVE"), true, "EXTRACT V", false));
    fg.push(ex_t(40.0, 568.0, "CARGO", T { size: 11.0, anchor: "start", fill: NS_GREY_TEXT, ..TD }));
    for (c, x, iso, fan) in [("FWD", 120.0, "CISO_FWD", "CFAN_FWD"), ("BULK", 420.0, "CISO_AFT", "CFAN_AFT")] {
        fg.push(ns_valve(x, 562.0, &s(iso), m.get(iso) != Some("fail"), &format!("{c} ISOL V"), false));
        fg.push(ns_box(x + 90.0, 548.0, 80.0, 28.0, &format!("{c} FAN"), &s(fan), B { size: 9.0, r: 7.0, ..Default::default() }));
    }
    fg.push(ns_box(720.0, 548.0, 110.0, 28.0, "BULK HEATER", &s("BULKHEAT"), B { size: 9.0, r: 7.0, ..Default::default() }));
    if !title.is_empty() {
        fg.push(ex_t(w - 16.0, h - 8.0, title, T { anchor: "end", size: 12.0, fill: NS_GREY_TEXT, weight: 500.0, ..TD }));
    }
    wires.extend(fg);
    ns_svg(w, h, &wires, None)
}

pub fn air_diagram(name: &str, title: &str) -> Option<Node> {
    let m = air_targets(name)?;
    let vent = m.keys().any(|k| re!(r"^(CF|OFV|CPC|AV|CISO|CFAN|BULKHEAT|TRIM|MIXER)").is_match(k));
    Some(if vent { vent_view(&m, title) } else { bleed_view(&m, title) })
}

const ZONES_MAIN: [&str; 8] = ["MAIN_DECK_1", "MAIN_DECK_2", "MAIN_DECK_3", "MAIN_DECK_4", "MAIN_DECK_5", "MAIN_DECK_6", "MAIN_DECK_7", "MAIN_DECK_8"];
const ZONES_UPPER: [&str; 7] = ["UPPER_DECK_1", "UPPER_DECK_2", "UPPER_DECK_3", "UPPER_DECK_4", "UPPER_DECK_5", "UPPER_DECK_6", "UPPER_DECK_7"];

/// The variables the live airflow page polls.
pub fn airflow_names() -> Vec<String> {
    let mut v = Vec::new();
    for e in 1..=4 {
        for k in ["BLEED_EXTRACTION_FLOW", "PR_VALVE_OPEN", "HP_VALVE_OPEN", "IP_VALVE_OPEN", "REGULATED_TRANSDUCER_PRESSURE", "PRECOOLER_OUTLET_TEMPERATURE"] {
            v.push(format!("A32NX_PNEU_ENG_{e}_{k}"));
        }
    }
    for p in 1..=2 {
        for k in 1..=2 {
            v.push(format!("A32NX_PNEU_PACK_{p}_FLOW_VALVE_{k}_FLOW_RATE"));
            v.push(format!("A32NX_COND_PACK_{p}_FLOW_VALVE_{k}_IS_OPEN"));
        }
        v.push(format!("A32NX_COND_PACK_{p}_OUTLET_TEMPERATURE"));
        v.push(format!("A32NX_COND_PACK_{p}_RAM_AIR_FLOW"));
    }
    for z in std::iter::once("CKPT").chain(ZONES_MAIN).chain(ZONES_UPPER) {
        v.push(format!("A32NX_COND_{z}_TEMP"));
    }
    for o in 1..=4 {
        v.push(format!("A32NX_PRESS_MAN_OUTFLOW_VALVE_{o}_OPEN_PERCENTAGE"));
    }
    for k in ["A32NX_PRESS_CABIN_ALTITUDE_1", "A32NX_PRESS_CABIN_DELTA_PRESSURE_1", "A32NX_PRESS_CABIN_VS_1", "A32NX_VENT_CAB_FANS", "A32NX_PNEU_APU_BLEED_CONTAINER_PRESSURE", "A32NX_PRESS_SAFETY_VALVE_OPEN_PERCENTAGE"] {
        v.push(k.into());
    }
    v
}

pub fn airflow_nominal() -> Map<String, Value> {
    let mut v = Map::new();
    let mut put = |k: String, x: f64| {
        v.insert(k, serde_json::json!(x));
    };
    for e in 1..=4 {
        put(format!("A32NX_PNEU_ENG_{e}_BLEED_EXTRACTION_FLOW"), 1.05);
        put(format!("A32NX_PNEU_ENG_{e}_PR_VALVE_OPEN"), 1.0);
        put(format!("A32NX_PNEU_ENG_{e}_HP_VALVE_OPEN"), 0.0);
        put(format!("A32NX_PNEU_ENG_{e}_IP_VALVE_OPEN"), 1.0);
        put(format!("A32NX_PNEU_ENG_{e}_REGULATED_TRANSDUCER_PRESSURE"), 42.0);
        put(format!("A32NX_PNEU_ENG_{e}_PRECOOLER_OUTLET_TEMPERATURE"), 195.0);
    }
    for p in 1..=2 {
        for k in 1..=2 {
            put(format!("A32NX_PNEU_PACK_{p}_FLOW_VALVE_{k}_FLOW_RATE"), 0.62);
            put(format!("A32NX_COND_PACK_{p}_FLOW_VALVE_{k}_IS_OPEN"), 1.0);
        }
        put(format!("A32NX_COND_PACK_{p}_OUTLET_TEMPERATURE"), 8.0);
        put(format!("A32NX_COND_PACK_{p}_RAM_AIR_FLOW"), 1.4);
    }
    for z in std::iter::once("CKPT").chain(ZONES_MAIN).chain(ZONES_UPPER) {
        put(format!("A32NX_COND_{z}_TEMP"), 23.0);
    }
    for o in 1..=4 {
        put(format!("A32NX_PRESS_MAN_OUTFLOW_VALVE_{o}_OPEN_PERCENTAGE"), 18.0);
    }
    put("A32NX_PRESS_CABIN_ALTITUDE_1".into(), 6500.0);
    put("A32NX_PRESS_CABIN_DELTA_PRESSURE_1".into(), 8.4);
    put("A32NX_PRESS_CABIN_VS_1".into(), 0.0);
    put("A32NX_VENT_CAB_FANS".into(), 1.0);
    v
}

pub fn airflow_view(v: &Map<String, Value>, fail: &OMap, note: &str) -> Node {
    let (w, h) = (1000.0, 780.0);
    let num = |k: &str, d: f64| -> f64 {
        let x = match v.get(k) {
            None => f64::NAN,
            Some(Value::Null) => 0.0,
            Some(Value::Bool(b)) => f64::from(u8::from(*b)),
            Some(Value::Number(x)) => x.as_f64().unwrap_or(f64::NAN),
            Some(Value::String(s)) => s.trim().parse().unwrap_or(if s.trim().is_empty() { 0.0 } else { f64::NAN }),
            Some(_) => f64::NAN,
        };
        if x.is_finite() { x } else { d }
    };
    let mut wires: Vec<String> = Vec::new();
    let mut fg: Vec<String> = Vec::new();
    let flow_state = |f: f64, nominal: f64, key: Option<&str>| -> &'static str {
        if key.is_some_and(|k| fail.get(k) == Some("fail")) {
            "fail"
        } else if f <= 0.02 {
            "off"
        } else if f < nominal * 0.4 {
            "warn"
        } else {
            "live"
        }
    };
    let stream = |wires: &mut Vec<String>, p: &[[f64; 2]], f: f64, nominal: f64, key: Option<&str>| -> &'static str {
        let st = flow_state(f, nominal, key);
        let d = super::d_path(p);
        let sw = if st == "off" { 1.4 } else { 2.0 + (f.max(0.0).sqrt() * 2.6).min(5.0) };
        wires.push(format!(
            "<path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\" opacity=\"{}\"/>",
            ns_stroke(st),
            fx(sw, 1),
            if st == "off" { "0.6" } else { "0.35" }
        ));
        if st != "off" && st != "fail" {
            let dur = (2.6 / (f / nominal).max(0.15)).max(0.35);
            wires.push(format!(
                "<path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-dasharray=\"4 14\" class=\"af-flow\" style=\"animation-duration:{}s\"/>",
                ns_stroke(st),
                fx(sw * 0.55, 1),
                fx(dur, 2)
            ));
        }
        st
    };
    let val = |fg: &mut Vec<String>, x: f64, y: f64, t: &str, st: &str, anchor: &str| {
        fg.push(ex_t(x, y, t, T { size: 9.0, anchor, fill: if st == "live" { NS_GREY_TEXT } else { ns_stroke(st) }, weight: 600.0, ..TD }));
    };
    let fst = |k: &str, d: &'static str| -> String { fail.get(k).map(str::to_string).unwrap_or(d.to_string()) };
    let eng_x = [110.0, 310.0, 690.0, 890.0];
    let man_y = 250.0;
    let mut bleed_total = 0.0;
    for (i, x) in eng_x.iter().copied().enumerate() {
        let e = i + 1;
        let f = num(&format!("A32NX_PNEU_ENG_{e}_BLEED_EXTRACTION_FLOW"), 0.0);
        bleed_total += f;
        let prv = num(&format!("A32NX_PNEU_ENG_{e}_PR_VALVE_OPEN"), 0.0) > 0.5;
        let hp = num(&format!("A32NX_PNEU_ENG_{e}_HP_VALVE_OPEN"), 0.0) > 0.5;
        let ip = num(&format!("A32NX_PNEU_ENG_{e}_IP_VALVE_OPEN"), 0.0) > 0.5;
        fg.push(ns_box(x - 50.0, 24.0, 100.0, 32.0, &format!("ENG {e}"), "live", B { size: 12.0, ..Default::default() }));
        stream(&mut wires, &[[x - 22.0, 56.0], [x - 22.0, 96.0], [x, 96.0]], if ip { f } else { 0.0 }, 1.0, Some(&format!("IPC{e}")));
        stream(&mut wires, &[[x + 22.0, 56.0], [x + 22.0, 96.0], [x, 96.0]], if hp { f } else { 0.0 }, 1.0, Some(&format!("HP{e}")));
        fg.push(ns_valve(x - 22.0, 76.0, &fst(&format!("IPC{e}"), if ip { "live" } else { "off" }), ip, "IP", true));
        fg.push(ns_valve(x + 22.0, 76.0, &fst(&format!("HP{e}"), if hp { "live" } else { "off" }), hp, "HP", false));
        let st = stream(&mut wires, &[[x, 96.0], [x, 150.0]], if prv { f } else { 0.0 }, 1.0, Some(&format!("PRV{e}")));
        fg.push(ns_valve(x, 120.0, &fst(&format!("PRV{e}"), if prv { "live" } else { "off" }), prv, "PRV", false));
        fg.push(ns_box(x - 44.0, 150.0, 88.0, 30.0, "PRECOOLER", &fst(&format!("PCLR{e}"), if st == "off" { "off" } else { "live" }), B { size: 10.0, ..Default::default() }));
        stream(&mut wires, &[[x, 180.0], [x, man_y]], if prv { f } else { 0.0 }, 1.0, Some(&format!("DUCT{e}")));
        val(&mut fg, x + 8.0, 206.0, &format!("{} KG/S", fx(f, 2)), st, "start");
        let t = format!(
            "{} PSI  {}\u{b0}C",
            fx(num(&format!("A32NX_PNEU_ENG_{e}_REGULATED_TRANSDUCER_PRESSURE"), 0.0), 0),
            fx(num(&format!("A32NX_PNEU_ENG_{e}_PRECOOLER_OUTLET_TEMPERATURE"), 0.0), 0)
        );
        val(&mut fg, x + 8.0, 220.0, &t, st, "start");
    }
    stream(&mut wires, &[[40.0, man_y], [960.0, man_y]], bleed_total / 2.0, 1.0, None);
    val(&mut fg, 500.0, man_y - 8.0, &format!("BLEED MANIFOLD  {} KG/S", fx(bleed_total, 2)), "live", "middle");
    let mut pack_total = 0.0;
    for (i, x) in [300.0, 700.0].iter().copied().enumerate() {
        let p = i + 1;
        let mut pf = 0.0;
        for (j, dx) in [-34.0, 34.0].iter().copied().enumerate() {
            let k = j + 1;
            let f = num(&format!("A32NX_PNEU_PACK_{p}_FLOW_VALVE_{k}_FLOW_RATE"), 0.0);
            let open = num(&format!("A32NX_COND_PACK_{p}_FLOW_VALVE_{k}_IS_OPEN"), if f > 0.02 { 1.0 } else { 0.0 }) > 0.5;
            pf += f;
            stream(&mut wires, &[[x + dx, man_y], [x + dx, 340.0]], f, 0.6, Some(&format!("FCV{p}{k}")));
            fg.push(ns_valve(x + dx, 296.0, &fst(&format!("FCV{p}{k}"), if open { "live" } else { "off" }), open, &format!("FCV {p}-{k}"), dx < 0.0));
        }
        pack_total += pf;
        let pst = fst(&format!("PACK{p}"), if pf <= 0.02 { "off" } else { "live" });
        fg.push(ns_box(x - 74.0, 340.0, 148.0, 44.0, &format!("PACK {p}\n{}\u{b0}C  {} KG/S", fx(num(&format!("A32NX_COND_PACK_{p}_OUTLET_TEMPERATURE"), 0.0), 0), fx(pf, 2)), &pst, B { size: 10.0, ..Default::default() }));
        stream(&mut wires, &[[x + 74.0, 362.0], [x + 130.0, 362.0]], num(&format!("A32NX_COND_PACK_{p}_RAM_AIR_FLOW"), 0.0), 1.0, Some(&format!("RAM{p}")));
        val(&mut fg, x + 102.0, 354.0, "RAM AIR", "live", "middle");
        stream(&mut wires, &[[x, 384.0], [x, 420.0], [500.0, 420.0], [500.0, 440.0]], pf, 1.2, Some(&format!("PACK{p}")));
    }
    let fans = num("A32NX_VENT_CAB_FANS", 1.0) > 0.5;
    let recirc = if fans { pack_total * 0.65 } else { 0.0 };
    fg.push(ns_box(430.0, 440.0, 140.0, 36.0, &format!("MIXER UNIT\n{} KG/S", fx(pack_total + recirc, 2)), &fst("MIXER", if pack_total > 0.02 { "live" } else { "off" }), B { size: 10.0, ..Default::default() }));
    let per_zone = (pack_total + recirc) / 16.0;
    stream(&mut wires, &[[500.0, 476.0], [500.0, 500.0], [60.0, 500.0], [60.0, 520.0]], pack_total + recirc, 2.4, Some("TRIM"));
    stream(&mut wires, &[[60.0, 500.0], [940.0, 500.0]], pack_total + recirc, 2.4, Some("TRIM"));
    fg.push(ex_t(500.0, 494.0, "TRIM AIR / ZONE DISTRIBUTION", T { size: 9.0, fill: NS_GREY_TEXT, ..TD }));
    let zone_box = |wires: &mut Vec<String>, fg: &mut Vec<String>, z: &str, label: &str, x: f64, y: f64, bw: f64, bus_y: f64| {
        let t = num(&format!("A32NX_COND_{z}_TEMP"), 23.0);
        stream(wires, &[[x + bw / 2.0, bus_y], [x + bw / 2.0, y]], per_zone, 0.15, Some("TRIM"));
        let st = if t > 30.0 || t < 15.0 { "warn" } else { "live" };
        fg.push(ns_box(x, y, bw, 38.0, &format!("{label}\n{}\u{b0}C", fx(t, 0)), if per_zone <= 0.005 { "warn" } else { st }, B { size: 9.0, r: 7.0, ..Default::default() }));
    };
    zone_box(&mut wires, &mut fg, "CKPT", "CKPT", 20.0, 532.0, 70.0, 500.0);
    for (i, z) in ZONES_UPPER.iter().enumerate() {
        zone_box(&mut wires, &mut fg, z, &format!("UPPER {}", i + 1), 104.0 + i as f64 * 126.0, 532.0, 112.0, 500.0);
    }
    let stream2 = |wires: &mut Vec<String>, p: &[[f64; 2]], f: f64, nominal: f64, key: Option<&str>| {
        let st = flow_state(f, nominal, key);
        let d = super::d_path(p);
        let sw = if st == "off" { 1.4 } else { 2.0 + (f.max(0.0).sqrt() * 2.6).min(5.0) };
        wires.push(format!("<path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\" opacity=\"{}\"/>", ns_stroke(st), fx(sw, 1), if st == "off" { "0.6" } else { "0.35" }));
        if st != "off" && st != "fail" {
            let dur = (2.6 / (f / nominal).max(0.15)).max(0.35);
            wires.push(format!(
                "<path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-dasharray=\"4 14\" class=\"af-flow\" style=\"animation-duration:{}s\"/>",
                ns_stroke(st),
                fx(sw * 0.55, 1),
                fx(dur, 2)
            ));
        }
    };
    stream2(&mut wires, &[[60.0, 500.0], [8.0, 500.0], [8.0, 580.0], [960.0, 580.0]], per_zone * 8.0, 1.2, Some("TRIM"));
    for (i, z) in ZONES_MAIN.iter().enumerate() {
        let t = num(&format!("A32NX_COND_{z}_TEMP"), 23.0);
        let (x, y, bw) = (20.0 + i as f64 * 120.0, 590.0, 108.0);
        stream2(&mut wires, &[[x + bw / 2.0, 580.0], [x + bw / 2.0, y]], per_zone, 0.15, Some("TRIM"));
        let st = if t > 30.0 || t < 15.0 { "warn" } else { "live" };
        fg.push(ns_box(x, y, bw, 38.0, &format!("MAIN {}\n{}\u{b0}C", i + 1, fx(t, 0)), if per_zone <= 0.005 { "warn" } else { st }, B { size: 9.0, r: 7.0, ..Default::default() }));
    }
    if fans {
        stream2(&mut wires, &[[900.0, 628.0], [900.0, 646.0], [980.0, 646.0], [980.0, 458.0], [570.0, 458.0]], recirc, 1.0, None);
    }
    fg.push(ns_box(930.0, 470.0, 60.0, 30.0, "CAB FANS", if fans { "live" } else { "off" }, B { size: 8.0, r: 6.0, ..Default::default() }));
    let cabin_out = pack_total;
    let ofv_sum: f64 = (1..=4).map(|o| num(&format!("A32NX_PRESS_MAN_OUTFLOW_VALVE_{o}_OPEN_PERCENTAGE"), 0.0)).sum();
    for o in 1..=4 {
        let x = 140.0 + (o - 1) as f64 * 200.0;
        let pct = num(&format!("A32NX_PRESS_MAN_OUTFLOW_VALVE_{o}_OPEN_PERCENTAGE"), 0.0);
        let f = if ofv_sum > 0.0 { (cabin_out * pct) / ofv_sum } else { 0.0 };
        stream2(&mut wires, &[[x, 646.0], [x, 720.0]], f, 0.4, Some(&format!("OFV{o}")));
        fg.push(ns_valve(x, 684.0, &fst(&format!("OFV{o}"), if pct > 0.5 { "live" } else { "off" }), pct > 0.5, &format!("OFV {o}  {}%", fx(pct, 0)), false));
    }
    stream2(&mut wires, &[[20.0, 646.0], [980.0, 646.0]], cabin_out, 2.4, Some("CPC"));
    fg.push(ex_t(500.0, 742.0, &format!("OVERBOARD  {} KG/S", fx(cabin_out, 2)), T { size: 9.0, fill: NS_GREY_TEXT, weight: 600.0, ..TD }));
    let alt = num("A32NX_PRESS_CABIN_ALTITUDE_1", 0.0);
    let dp = num("A32NX_PRESS_CABIN_DELTA_PRESSURE_1", 0.0);
    let vs = num("A32NX_PRESS_CABIN_VS_1", 0.0);
    fg.push(ex_t(
        20.0,
        764.0,
        &format!("CABIN ALT {} FT   \u{394}P {} PSI   V/S {} FT/MIN", fx(alt, 0), fx(dp, 1), fx(vs, 0)),
        T { size: 11.0, anchor: "start", fill: if alt > 9550.0 { NS_RED } else { NS_GREEN }, ..TD },
    ));
    if !note.is_empty() {
        fg.push(ex_t(w - 16.0, 764.0, note, T { size: 10.0, anchor: "end", fill: NS_ORANGE, ..TD }));
    }
    let mut all = vec!["<style>.af-flow{animation:afmove linear infinite}@keyframes afmove{to{stroke-dashoffset:-36}}</style>".to_string()];
    all.extend(wires);
    all.extend(fg);
    let _ = n;
    ns_svg(w, h, &all, None)
}

pub fn airflow_failures(list: &[Value]) -> OMap {
    let mut m = OMap::default();
    for f in list {
        if !crate::jv::b(f, "active") {
            continue;
        }
        if let Some(t) = air_targets(crate::jv::s(f, "name")) {
            for (k, v) in t.0 {
                m.set(&k, &v);
            }
        }
    }
    m
}

/// `airflowPage`: offline (or in the fixture) it shows nominal cruise values.
pub fn airflow_page() -> Node {
    let (live, failures) = super::ctx(|c| (c.live, c.failures.clone()));
    let host = el("div", "af-live");
    if !live {
        return host.child(airflow_view(&airflow_nominal(), &OMap::default(), "NOT CONNECTED \u{b7} NOMINAL CRUISE VALUES"));
    }
    host.child(airflow_view(&airflow_nominal(), &airflow_failures(&failures), ""))
}

use super::{dx, ex_path, ex_t, ns_box, ns_stroke, ns_svg, ns_valve, pts, surface, wire, wire_d, OMap, B, EX_LE, EX_TE, NS_FAIL_FILL, NS_FILL, NS_GREEN, NS_GREY, NS_GREY_TEXT, NS_ORANGE, NS_RED, NS_WARN_FILL, T, TD};
use crate::ground::squircle;
use crate::jv::{fx, n};
use crate::Node;

fn power_col(k: &str) -> &'static str {
    match k {
        "G" => "#3fd27f",
        "Y" => "#ffd23f",
        _ => "#4fb7ff",
    }
}

fn fctl_power(k: &str) -> &'static [&'static str] {
    match k {
        "AIL_OUT" => &["G", "Y"],
        "AIL_MID" => &["Y", "E ESS"],
        "AIL_IN" => &["G", "E EHA"],
        "ELEV_L_IN" => &["G", "E ESS"],
        "ELEV_L_OUT" => &["G", "E EHA"],
        "ELEV_R_IN" => &["Y", "E EHA"],
        "ELEV_R_OUT" => &["Y", "E ESS"],
        "RUD_UP" => &["G", "E ESS", "E EHA"],
        "RUD_LO" => &["Y", "E ESS", "E AC1"],
        "THS" => &["G", "Y", "E"],
        "SLATS" => &["G", "E"],
        "FLAPS" => &["G", "Y"],
        _ => &[],
    }
}

fn fctl_ctrl(key: &str) -> Option<&'static [&'static str]> {
    if let Some(c) = re!(r"^SPLR_[LR](\d)$").captures(key) {
        return Some(match &c[1] {
            "1" => &["PRIM 1", "SEC 1"],
            "2" => &["PRIM 2"],
            "3" => &["PRIM 3"],
            "4" => &["PRIM 1"],
            "5" => &["PRIM 2"],
            "6" => &["PRIM 3", "SEC 3"],
            "7" => &["PRIM 1"],
            "8" => &["PRIM 2", "SEC 2"],
            _ => return None,
        });
    }
    Some(match key {
        "AIL_L_IN" | "AIL_R_IN" => &["PRIM 1", "SEC 1"],
        "AIL_L_MID" | "AIL_R_MID" => &["PRIM 2", "SEC 2"],
        "AIL_L_OUT" | "AIL_R_OUT" => &["PRIM 3", "SEC 3"],
        "ELEV_L_IN" | "ELEV_R_IN" => &["PRIM 1", "SEC 1"],
        "ELEV_L_OUT" | "ELEV_R_OUT" => &["PRIM 2", "SEC 2"],
        "RUD_UP" => &["PRIM 1", "SEC 1", "BCM"],
        "RUD_LO" => &["PRIM 2", "SEC 3", "BCM"],
        "THS" => &["PRIM 1", "PRIM 2", "PRIM 3", "SEC 1", "SEC 2"],
        "SLATS" | "FLAPS" => &["SFCC 1", "SFCC 2"],
        _ => return None,
    })
}

fn fctl_control(key: &str, m: &OMap) -> (&'static str, String) {
    let Some(list) = fctl_ctrl(key) else { return ("live", String::new()) };
    let alive: Vec<&str> = list.iter().copied().filter(|c| m.get(c) != Some("fail")).collect();
    if alive.is_empty() {
        return ("warn", "LOST".into());
    }
    if alive[0] != list[0] {
        return ("alt", format!("VIA {}", alive[0]));
    }
    ("live", String::new())
}

const COMPUTERS: [&str; 11] = ["PRIM 1", "PRIM 2", "PRIM 3", "SEC 1", "SEC 2", "SEC 3", "FCDC 1", "FCDC 2", "SFCC 1", "SFCC 2", "BCM"];

pub fn fctl_targets(name: &str) -> Option<OMap> {
    let n = format!(" {} ", re!(r"[_-]").replace_all(&name.to_uppercase(), " "));
    let n = n.as_str();
    let mut m = OMap::default();
    let side = if re!(r"\bLEFT\b|\bL\b").is_match(n) {
        Some("L")
    } else if re!(r"\bRIGHT\b|\bR\b").is_match(n) {
        Some("R")
    } else {
        None
    };
    let soft = re!(r"SUPPLY LOSS|TRANSDUCER|DRIFT|BIAS|WEAR|LEAKAGE|DAMPER|CHANNEL|MONITOR|INTERMITTENT").is_match(n);
    let st = if soft { "warn" } else { "fail" };
    if let Some(k) = re!(r"\b(PRIM|SEC|FCDC|SFCC) ?(\d)").captures(n) {
        m.set(&format!("{} {}", &k[1], &k[2]), st);
        return Some(m);
    }
    if re!(r"BCM|BACKUP CONTROL").is_match(n) {
        m.set("BCM", st);
        return Some(m);
    }
    let sides: Vec<&str> = match side {
        Some(s) => vec![s],
        None => vec!["L", "R"],
    };
    if let Some(k) = re!(r"SPOILER ?(\d)").captures(n) {
        for s in &sides {
            m.set(&format!("SPLR_{s}{}", &k[1]), st);
        }
    } else if re!(r"SPOILERS?|SPLR").is_match(n) {
        for s in &sides {
            for i in 1..=8 {
                m.set(&format!("SPLR_{s}{i}"), st);
            }
        }
    } else if re!(r"AILERON|\bAIL\b").is_match(n) {
        let pos: &[&str] = if re!(r"OUTWARD|OUTER|\bOUTR\b|OUTBOARD").is_match(n) {
            &["OUT"]
        } else if re!(r"MIDDLE|\bMID\b").is_match(n) {
            &["MID"]
        } else if re!(r"INWARD|INNER|\bINR\b|INBOARD").is_match(n) {
            &["IN"]
        } else {
            &["IN", "MID", "OUT"]
        };
        for s in &sides {
            for p in pos {
                m.set(&format!("AIL_{s}_{p}"), st);
            }
        }
    } else if re!(r"ELEVATOR|\bELEV\b").is_match(n) {
        let pos: &[&str] = if re!(r"OUTBOARD|OUTWARD|OUTER|\bOUTR\b").is_match(n) {
            &["OUT"]
        } else if re!(r"INBOARD|INWARD|INNER|\bINR\b").is_match(n) {
            &["IN"]
        } else {
            &["IN", "OUT"]
        };
        for s in &sides {
            for p in pos {
                m.set(&format!("ELEV_{s}_{p}"), st);
            }
        }
    } else if re!(r"RUDDER|\bRUD\b").is_match(n) {
        let r: &[&str] = if n.contains("UPPER") {
            &["RUD_UP"]
        } else if n.contains("LOWER") {
            &["RUD_LO"]
        } else {
            &["RUD_UP", "RUD_LO"]
        };
        for k in r {
            m.set(k, st);
        }
    } else if re!(r"THS|PITCH TRIM|STABILI[SZ]ER|HORIZONTAL").is_match(n) {
        m.set("THS", st);
    } else if re!(r"SLAT|DROOP").is_match(n) {
        m.set("SLATS", st);
    } else if n.contains("FLAP") {
        m.set("FLAPS", st);
    } else {
        return None;
    }
    Some(m)
}

fn fctl_law(m: &OMap) -> Option<&'static str> {
    let prims = (1..=3).filter(|i| m.get(&format!("PRIM {i}")) == Some("fail")).count();
    let secs = (1..=3).filter(|i| m.get(&format!("SEC {i}")) == Some("fail")).count();
    if prims == 3 && secs == 3 {
        Some("BACKUP (BCM)")
    } else if prims == 3 {
        Some("DIRECT LAW")
    } else if prims >= 2 {
        Some("ALTN LAW")
    } else {
        None
    }
}

fn fctl_name(key: &str) -> String {
    match key {
        "SLATS" | "FLAPS" | "THS" => return key.into(),
        "RUD_UP" => return "UPPER RUDDER".into(),
        "RUD_LO" => return "LOWER RUDDER".into(),
        _ => {}
    }
    if let Some(c) = re!(r"^SPLR_([LR])(\d)$").captures(key) {
        return format!("{} SPLR {}", &c[1], &c[2]);
    }
    if let Some(c) = re!(r"^AIL_([LR])_(IN|MID|OUT)$").captures(key) {
        return format!("{} {} AIL", &c[1], &c[2]);
    }
    if let Some(c) = re!(r"^ELEV_([LR])_(IN|OUT)$").captures(key) {
        return format!("{} {} ELEV", &c[1], &c[2]);
    }
    key.into()
}

pub(crate) fn surface_poly(key: &str, side: &str) -> Option<Vec<[f64; 2]>> {
    let b = surface(key)?;
    let k = if side == "R" { 1.0 } else { -1.0 };
    let (xa, xb) = (b[0].abs(), b[1].abs());
    let tail = key.contains("elevator");
    let te = pts(&dx()["model"]["te"]);
    let slope = if tail { 0.36 } else { (te[te.len() - 1][1] - te[0][1]) / (te[te.len() - 1][0] - te[0][0]) };
    let (xi, xo) = (xa.min(xb), xa.max(xb));
    let chord = (b[3] - b[2] - slope * (xo - xi)).max(0.8);
    let aft_in = b[3] - slope * (xo - xi);
    Some(vec![[k * xi, aft_in - chord], [k * xo, aft_in - chord + slope * (xo - xi)], [k * xo, b[3]], [k * xi, aft_in]])
}

fn fill_of(st: &str) -> &'static str {
    match st {
        "fail" => "rgba(255,61,99,0.38)",
        "warn" => "rgba(255,176,0,0.25)",
        _ => "rgba(63,210,127,0.12)",
    }
}

pub fn fctl_plan_view(m: &OMap, title: &str) -> Node {
    let (w, h, s, x0, y0) = (1000.0, 900.0, 9.0, 500.0, 64.0);
    let p = |x: f64, z: f64| [x0 + x * s, y0 + (z + 35.4) * s];
    let mut parts = Vec::new();
    let mut fg = Vec::new();
    for (i, c) in COMPUTERS.iter().enumerate() {
        let st = m.st(c, "live");
        fg.push(ns_box(20.0 + i as f64 * 88.0, 14.0, 80.0, 28.0, c, st, B { size: 10.0, r: 7.0, tag: if m.get(c) == Some("fail") { "FAILED" } else { "" } }));
    }
    if let Some(law) = fctl_law(m) {
        fg.push(ex_t(w - 20.0, 68.0, law, T { size: 15.0, anchor: "end", fill: NS_ORANGE, ..TD }));
    }
    parts.push(ex_path(&pts(&dx()["sil"]["top"]).iter().map(|q| p(q[0], q[1])).collect::<Vec<_>>(), "rgba(255,255,255,0.03)", "#4a4f56", 1.2, false));
    let draw_poly = |pp: &[[f64; 2]], st: &str, proj: &dyn Fn(f64, f64) -> [f64; 2]| {
        format!(
            "<path d=\"M{} Z\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\"/>",
            pp.iter().map(|q| {
                let r = proj(q[0], q[1]);
                format!("{},{}", fx(r[0], 1), fx(r[1], 1))
            }).collect::<Vec<_>>().join(" L"),
            fill_of(st),
            ns_stroke(st),
            if st == "live" { "1.2" } else { "2" }
        )
    };
    let dots = |fg: &mut Vec<String>, pp: &[[f64; 2]], power: &[&str], st: &str, proj: &dyn Fn(f64, f64) -> [f64; 2]| -> [f64; 2] {
        let sp: Vec<[f64; 2]> = pp.iter().map(|q| proj(q[0], q[1])).collect();
        let cnt = sp.len() as f64;
        let cx = sp.iter().fold(0.0, |a, q| a + q[0]) / cnt;
        let cy = sp.iter().fold(0.0, |a, q| a + q[1]) / cnt;
        let ww = sp.iter().map(|q| q[0]).fold(f64::NEG_INFINITY, f64::max) - sp.iter().map(|q| q[0]).fold(f64::INFINITY, f64::min);
        let r = (((ww - 4.0) / (power.len() as f64 * 2.6)).min(3.0)).max(1.6);
        for (j, pw) in power.iter().enumerate() {
            fg.push(format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>",
                fx(cx + (j as f64 - (power.len() as f64 - 1.0) / 2.0) * r * 2.6, 1),
                fx(cy, 1),
                fx(r, 1),
                if st == "fail" { "#fff" } else { power_col(pw) }
            ));
        }
        [cx, cy]
    };
    let mut affected: Vec<(String, String, String)> = Vec::new();
    let pj = |x: f64, z: f64| p(x, z);
    let surf = |fg: &mut Vec<String>, affected: &mut Vec<(String, String, String)>, key: &str, pp: &[[f64; 2]], power: &[&str], label: &str, off: f64| {
        let (cst, ctag) = fctl_control(key, m);
        let st = m.get(key).unwrap_or(cst).to_string();
        fg.push(draw_poly(pp, &st, &pj));
        let [cx, cy] = dots(fg, pp, power, &st, &pj);
        if !label.is_empty() {
            fg.push(ex_t(cx, cy + off, label, T { size: 8.0, fill: NS_GREY_TEXT, weight: 600.0, ..TD }));
        }
        let tag = match m.get(key) {
            Some("fail") => "FAILED".to_string(),
            Some(_) => "DEGRADED".into(),
            None => ctag,
        };
        if !tag.is_empty() {
            affected.push((key.into(), tag, st));
        }
    };
    let te = |x: f64| EX_TE[0][1] + ((x - EX_TE[0][0]) * (EX_TE[1][1] - EX_TE[0][1])) / (EX_TE[1][0] - EX_TE[0][0]);
    let le = |x: f64| EX_LE[0][1] + ((x - EX_LE[0][0]) * (EX_LE[1][1] - EX_LE[0][1])) / (EX_LE[1][0] - EX_LE[0][0]);
    let strip = |k: f64, xa: f64, xb: f64, f: &dyn Fn(f64) -> f64, d0: f64, d1: f64| vec![[k * xa, f(xa) + d0], [k * xb, f(xb) + d0], [k * xb, f(xb) + d1], [k * xa, f(xa) + d1]];
    for side in ["L", "R"] {
        let k = if side == "R" { 1.0 } else { -1.0 };
        surf(&mut fg, &mut affected, "FLAPS", &strip(k, 5.4, 26.4, &te, -2.6, -0.1), &["G", "Y"], "", 0.0);
        surf(&mut fg, &mut affected, "SLATS", &strip(k, 7.0, 38.6, &le, 0.05, 1.3), &["G", "E"], "", 0.0);
        for i in 1..=8 {
            let pp = surface_poly(&format!("l_spoiler{i}_key"), side).unwrap_or_default();
            let pw: &[&str] = if i == 6 { &["G", "E"] } else if i % 2 == 1 { &["Y"] } else { &["G"] };
            surf(&mut fg, &mut affected, &format!("SPLR_{side}{i}"), &pp, pw, &i.to_string(), -9.0);
        }
        for (i, pos) in ["IN", "MID", "OUT"].iter().enumerate() {
            let pp = surface_poly(&format!("l_aileron{}_percent_key", i + 1), side).unwrap_or_default();
            let pw: Vec<&str> = fctl_power(&format!("AIL_{pos}")).iter().map(|q| &q[..1]).collect();
            surf(&mut fg, &mut affected, &format!("AIL_{side}_{pos}"), &pp, &pw, "", 0.0);
        }
        let ths_pts = [[k * 1.0, 23.0], [k * 15.2, 34.5], [k * 13.7, 34.67], [k * 5.6, 31.6], [k * 1.0, 29.73]];
        let ths_st = m.get("THS").unwrap_or(fctl_control("THS", m).0).to_string();
        fg.push(draw_poly(&ths_pts, &ths_st, &pj));
        if side == "L" {
            dots(&mut fg, &[[-4.0, 27.6], [-6.0, 27.6], [-6.0, 28.6], [-4.0, 28.6]], &["G", "Y", "E"], &ths_st, &pj);
            let tt = fctl_control("THS", m).1;
            if !tt.is_empty() || m.get("THS").is_some() {
                affected.push(("THS".into(), if m.get("THS") == Some("fail") { "FAILED".into() } else { tt }, ths_st.clone()));
            }
        }
        for (pos, key) in [("IN", "l_inner_elevator_deflection"), ("OUT", "l_outer_elevator_deflection")] {
            let k2 = format!("ELEV_{side}_{pos}");
            let pp = surface_poly(key, side).unwrap_or_default();
            let pw: Vec<&str> = fctl_power(&k2).iter().map(|q| &q[..1]).collect();
            surf(&mut fg, &mut affected, &k2, &pp, &pw, "", 0.0);
        }
    }
    for (x, z, t) in [(-24.0, le(24.0) - 1.6, "SLATS"), (-16.0, te(16.0) + 1.6, "FLAPS"), (-31.5, te(31.5) + 1.9, "AILERONS"), (-15.5, 0.0, "SPOILERS"), (-9.0, 27.0, "THS"), (-8.0, 36.4, "ELEVATORS")] {
        let [a, b] = p(x, z);
        fg.push(ex_t(a, b, t, T { size: 9.0, fill: NS_GREY_TEXT, weight: 600.0, ..TD }));
    }
    let (fxo, fyo, fs) = (790.0, 680.0, 9.5);
    let fp = |z: f64, y: f64| [fxo + (z - 24.0) * fs, fyo + (21.0 - y) * fs];
    fg.push(format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"10\" fill=\"#1f2226\" stroke=\"#3a3e44\"/>", n(fxo - 10.0), n(fyo - 22.0), n(16.0 * fs), n(20.0 * fs)));
    fg.push(ex_t(fxo + 8.0 * fs - 10.0, fyo - 6.0, "FIN \u{b7} SIDE VIEW", T { size: 9.0, fill: NS_GREY_TEXT, ..TD }));
    fg.push(format!("<clipPath id=\"fctlFin\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"10\"/></clipPath>", n(fxo - 10.0), n(fyo - 22.0), n(16.0 * fs), n(20.0 * fs)));
    fg.push(format!(
        "<g clip-path=\"url(#fctlFin)\">{}</g>",
        ex_path(&pts(&dx()["sil"]["side"]).iter().map(|q| fp(q[0], q[1])).collect::<Vec<_>>(), "rgba(255,255,255,0.03)", "#4a4f56", 1.2, false)
    ));
    let te_z = |y: f64| 33.18 + (y - 10.68) * 0.446;
    for (key, ya, yb, chord) in [("RUD_UP", 10.6, 20.0, 3.4), ("RUD_LO", 5.6, 10.5, 3.3)] {
        let pp = [[te_z(ya) - chord, ya], [te_z(ya), ya], [te_z(yb), yb], [te_z(yb) - chord, yb]];
        let (cst, ctag) = fctl_control(key, m);
        let st = m.get(key).unwrap_or(cst).to_string();
        fg.push(draw_poly(&pp, &st, &fp));
        let pw: Vec<&str> = fctl_power(key).iter().map(|q| &q[..1]).collect();
        dots(&mut fg, &pp, &pw, &st, &fp);
        let tag = if m.get(key) == Some("fail") { "FAILED".to_string() } else { ctag };
        if !tag.is_empty() {
            affected.push((key.into(), tag, st));
        }
    }
    if !affected.is_empty() {
        fg.push(ex_t(24.0, 84.0, "AFFECTED", T { size: 10.0, anchor: "start", fill: NS_GREY_TEXT, ..TD }));
        for (i, (key, tag, st)) in affected.iter().enumerate() {
            let (col, row) = (i / 14, i % 14);
            fg.push(ex_t(24.0 + col as f64 * 150.0, 104.0 + row as f64 * 16.0, &format!("{}  {tag}", fctl_name(key)), T { size: 9.0, anchor: "start", fill: ns_stroke(st), weight: 600.0, ..TD }));
        }
    }
    for (i, (k, label)) in [("G", "GREEN HYD"), ("Y", "YELLOW HYD"), ("E", "ELECTRIC (EHA / EBHA)")].iter().enumerate() {
        fg.push(format!("<circle cx=\"30\" cy=\"{}\" r=\"5\" fill=\"{}\"/>", n(h - 64.0 + i as f64 * 18.0), power_col(k)));
        fg.push(ex_t(44.0, h - 60.0 + i as f64 * 18.0, label, T { size: 10.0, anchor: "start", fill: NS_GREY_TEXT, weight: 600.0, ..TD }));
    }
    if !title.is_empty() {
        fg.push(ex_t(w - 16.0, h - 10.0, title, T { anchor: "end", size: 12.0, fill: NS_GREY_TEXT, weight: 500.0, ..TD }));
    }
    parts.extend(fg);
    ns_svg(w, h, &parts, None)
}

fn detail_mode(name: &str) -> &'static str {
    let n = name.to_uppercase();
    let rules: [(&regex::Regex, &str); 10] = [
        (re!(r"SEPARATION"), "separated"),
        (re!(r"DISCONNECT"), "disconnect"),
        (re!(r"HARDOVER"), "hardover"),
        (re!(r"JAM"), "jam"),
        (re!(r"SUPPLY LOSS|HYDRAULIC/ELECTRICAL"), "supply"),
        (re!(r"FLUTTER|DAMPER"), "damper"),
        (re!(r"SERVO VALVE|INTERNAL LEAK"), "servovalve"),
        (re!(r"SEAL|PISTON"), "seal"),
        (re!(r"TRANSDUCER|LVDT|POSITION|DRIFT"), "lvdt"),
        (re!(r"OPEN CIRCUIT|SHORT|WIRING|INTERMITTENT|CHANNEL|COMMAND"), "wiring"),
    ];
    rules.iter().find(|(r, _)| r.is_match(&n)).map(|x| x.1).unwrap_or("fault")
}

struct Surf {
    bx: Option<String>,
    side: &'static str,
    title: String,
    power: Vec<&'static str>,
}

fn detail_surface(name: &str) -> Option<Surf> {
    let n = format!(" {} ", name.to_uppercase());
    let n = n.as_str();
    let side = if re!(r"\bLEFT\b|\bL\b").is_match(n) {
        "L"
    } else if re!(r"\bRIGHT\b|\bR\b").is_match(n) {
        "R"
    } else {
        "L"
    };
    let side_name = if side == "L" { "LEFT" } else { "RIGHT" };
    if n.contains("ELEVATOR") {
        let out = re!(r"OUTBOARD|OUTWARD|OUTER").is_match(n);
        let key = format!("ELEV_{side}_{}", if out { "OUT" } else { "IN" });
        return Some(Surf {
            bx: Some(if out { "l_outer_elevator_deflection" } else { "l_inner_elevator_deflection" }.into()),
            side,
            title: format!("{side_name} {} ELEVATOR", if out { "OUTBOARD" } else { "INBOARD" }),
            power: fctl_power(&key).to_vec(),
        });
    }
    if n.contains("AILERON") {
        let pos = if re!(r"OUTWARD|OUTER").is_match(n) {
            "OUT"
        } else if re!(r"MIDDLE|\bMID\b").is_match(n) {
            "MID"
        } else {
            "IN"
        };
        let idx = match pos {
            "IN" => 1,
            "MID" => 2,
            _ => 3,
        };
        let pn = match pos {
            "OUT" => "OUTWARD",
            "MID" => "MIDDLE",
            _ => "INWARD",
        };
        return Some(Surf { bx: Some(format!("l_aileron{idx}_percent_key")), side, title: format!("{side_name} {pn} AILERON"), power: fctl_power(&format!("AIL_{pos}")).to_vec() });
    }
    if n.contains("RUDDER") {
        let up = !n.contains("LOWER");
        return Some(Surf { bx: None, side: "C", title: if up { "UPPER RUDDER" } else { "LOWER RUDDER" }.into(), power: fctl_power(if up { "RUD_UP" } else { "RUD_LO" }).to_vec() });
    }
    if let Some(c) = re!(r"SPOILER ?(\d)").captures(n) {
        let d: u32 = c[1].parse().unwrap_or(0);
        return Some(Surf {
            bx: Some(format!("l_spoiler{d}_key")),
            side,
            title: format!("{side_name} SPOILER {d}"),
            power: if d == 6 { vec!["G", "E ESS"] } else if d % 2 == 1 { vec!["Y"] } else { vec!["G"] },
        });
    }
    None
}

fn locator(sf: &Surf, x0: f64, y0: f64, w: f64, h: f64) -> String {
    let s = (w / 82.0).min(h / 74.0);
    let p = |x: f64, z: f64| [x0 + w / 2.0 + x * s, y0 + (z + 35.4) * s];
    let mut out = format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"8\" fill=\"#1f2226\" stroke=\"{NS_GREY}\" stroke-width=\"1\"/>", n(x0), n(y0), n(w), n(h));
    out += &ex_path(&pts(&dx()["sil"]["top"]).iter().map(|q| p(q[0], q[1])).collect::<Vec<_>>(), "rgba(255,255,255,0.03)", NS_GREY, 1.0, false);
    let poly = match &sf.bx {
        Some(b) => surface_poly(b, sf.side).unwrap_or_default(),
        None => vec![[-0.8, 28.0], [0.8, 28.0], [0.8, 37.0], [-0.8, 37.0]],
    };
    out += &format!(
        "<path d=\"M{} Z\" fill=\"{NS_RED}\" stroke=\"#fff\" stroke-width=\"1\"/>",
        poly.iter().map(|q| {
            let r = p(q[0], q[1]);
            format!("{},{}", fx(r[0], 1), fx(r[1], 1))
        }).collect::<Vec<_>>().join(" L")
    );
    let l = poly.len() as f64;
    let c = poly.iter().fold([0.0, 0.0], |a, q| [a[0] + q[0] / l, a[1] + q[1] / l]);
    let [cx, cy] = p(c[0], c[1]);
    out += &format!("<circle cx=\"{}\" cy=\"{}\" r=\"9\" fill=\"none\" stroke=\"{NS_RED}\" stroke-width=\"1.5\"/>", fx(cx, 1), fx(cy, 1));
    out
}

pub fn fctl_detail_view(name: &str, title: &str) -> Option<Node> {
    let sf = detail_surface(name)?;
    let mode = detail_mode(name);
    let (w, h) = (1000.0, 660.0);
    let mut wires = Vec::new();
    let mut fg = Vec::new();
    fg.push(ex_t(24.0, 32.0, &sf.title, T { anchor: "start", size: 17.0, fill: NS_GREEN, ..TD }));
    let mode_text = match mode {
        "separated" => "SURFACE SEPARATION",
        "disconnect" => "SURFACE DISCONNECT",
        "hardover" => "SERVO HARDOVER",
        "jam" => "ACTUATOR JAM",
        "supply" => "POWER SUPPLY LOSS",
        "damper" => "FLUTTER DAMPER LOSS",
        "servovalve" => "SERVO VALVE INTERNAL LEAKAGE",
        "seal" => "PISTON SEAL WEAR",
        "lvdt" => "POSITION TRANSDUCER FAULT",
        "wiring" => "COMMAND / MONITOR WIRING FAULT",
        _ => "ACTUATOR FAULT",
    };
    fg.push(ex_t(24.0, 52.0, mode_text, T { anchor: "start", size: 11.0, fill: NS_RED, ..TD }));
    fg.push(locator(&sf, w - 214.0, 14.0, 196.0, 150.0));
    let (hinge_x, hinge_y) = (800.0, 380.0);
    for (i, p) in sf.power.iter().enumerate() {
        let primary = i == 0;
        let is_hyd = *p == "G" || *p == "Y";
        let y = 120.0 + i as f64 * if sf.power.len() > 2 { 160.0 } else { 230.0 };
        let ctrl = if is_hyd { if *p == "G" { "PRIM 1" } else { "PRIM 2" } } else if i == 1 { "SEC 1" } else { "SEC 2" };
        let bad = |on: bool, worse: &'static str| -> &'static str { if primary && on { worse } else { "live" } };
        fg.push(ex_t(24.0, y - 10.0, ctrl, T { size: 11.0, anchor: "start", fill: NS_GREY_TEXT, ..TD }));
        fg.push(ns_box(24.0, y, 104.0, 34.0, "COM LANE", bad(mode == "wiring", "fail"), B { size: 10.0, r: 7.0, ..Default::default() }));
        fg.push(ns_box(24.0, y + 48.0, 104.0, 34.0, "MON LANE", "live", B { size: 10.0, r: 7.0, ..Default::default() }));
        let (ax, aw, ah) = (176.0, 520.0, 150.0);
        fg.push(format!("<path d=\"{}\" fill=\"none\" stroke=\"{NS_GREY}\" stroke-width=\"1.2\" stroke-dasharray=\"3 4\"/>", squircle(ax, y - 16.0, aw, ah, 14.0)));
        let cap = if is_hyd { format!("HYDRAULIC SERVO CONTROL \u{b7} {}", if *p == "G" { "GREEN" } else { "YELLOW" }) } else { format!("ELECTRO-HYDROSTATIC ACTUATOR \u{b7} {}", p.replacen("E ", "AC ", 1)) };
        fg.push(ex_t(ax + aw, y - 24.0, &cap, T { size: 10.0, anchor: "end", fill: NS_GREY_TEXT, ..TD }));
        let row = y + 4.0;
        let chain: Vec<(&str, &str)> = if is_hyd {
            vec![
                ("SHUTOFF V", bad(mode == "supply", "fail")),
                ("FILTER", "live"),
                ("CHECK V", "live"),
                ("SERVO V", bad(mode == "hardover" || mode == "servovalve", if mode == "hardover" { "fail" } else { "warn" })),
                ("MODE V", bad(mode == "fault", "warn")),
            ]
        } else {
            vec![("C/B", bad(mode == "supply", "fail")), ("POWER ELEC", bad(mode == "fault", "warn")), ("MOTOR", "live"), ("PUMP", bad(mode == "servovalve", "warn")), ("ACCUM", "live")]
        };
        for (j, (label, st)) in chain.iter().enumerate() {
            let x = ax + 12.0 + j as f64 * 74.0;
            if j > 0 {
                wires.push(wire(&[[x - 8.0, row + 15.0], [x, row + 15.0]], if *st == "fail" { "fail" } else { "live" }));
            }
            fg.push(ns_box(x, row, 64.0, 30.0, label, st, B { size: 9.0, r: 7.0, ..Default::default() }));
        }
        let piston = bad(mode == "jam" || mode == "seal", if mode == "jam" { "fail" } else { "warn" });
        let px = ax + 12.0 + 5.0 * 74.0;
        wires.push(wire(&[[px - 8.0, row + 15.0], [px, row + 15.0]], if piston == "fail" { "fail" } else { "live" }));
        fg.push(format!(
            "<path d=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"2.2\"/>",
            squircle(px, row - 6.0, 128.0, 42.0, 8.0),
            match piston {
                "fail" => NS_FAIL_FILL,
                "warn" => NS_WARN_FILL,
                _ => NS_FILL,
            },
            ns_stroke(piston)
        ));
        fg.push(format!("<path d=\"M{},{} L{},{}\" stroke=\"{}\" stroke-width=\"3\"/>", n(px + 64.0), n(row - 2.0), n(px + 64.0), n(row + 32.0), ns_stroke(piston)));
        fg.push(ex_t(px + 32.0, row + 19.0, "EXT", T { size: 9.0, fill: NS_GREY_TEXT, ..TD }));
        fg.push(ex_t(px + 96.0, row + 19.0, "RET", T { size: 9.0, fill: NS_GREY_TEXT, ..TD }));
        fg.push(ex_t(px + 64.0, row - 12.0, "PISTON", T { size: 9.0, fill: ns_stroke(piston), ..TD }));
        let damp = bad(mode == "damper", "warn");
        fg.push(ns_valve(px + 64.0, row + 56.0, damp, true, "DAMPING ORIFICE", false));
        wires.push(wire(&[[px + 64.0, row + 36.0], [px + 64.0, row + 47.0]], damp));
        let lvdt_com = bad(mode == "lvdt", "fail");
        fg.push(ns_box(ax + 12.0, row + 60.0, 90.0, 26.0, "LVDT COM", lvdt_com, B { size: 9.0, r: 6.0, ..Default::default() }));
        fg.push(ns_box(ax + 112.0, row + 60.0, 90.0, 26.0, "LVDT MON", "live", B { size: 9.0, r: 6.0, ..Default::default() }));
        fg.push(ns_box(ax + 212.0, row + 60.0, 90.0, 26.0, "MODE SOL", bad(mode == "wiring", "warn"), B { size: 9.0, r: 6.0, ..Default::default() }));
        let cmd = bad(mode == "wiring", "fail");
        let c3 = ax + 12.0 + 3.0 * 74.0 + 32.0;
        wires.push(wire_d(&[[128.0, y + 17.0], [150.0, y + 17.0], [150.0, row - 30.0], [c3, row - 30.0], [c3, row]], cmd, if cmd == "fail" { "5 5" } else { "" }));
        fg.push(ex_t(160.0, row - 34.0, "SERVO COMMAND", T { size: 8.0, anchor: "start", fill: if cmd == "fail" { NS_RED } else { NS_GREY_TEXT }, ..TD }));
        wires.push(wire_d(&[[128.0, y + 22.0], [158.0, y + 22.0], [158.0, row + 100.0], [ax + 257.0, row + 100.0], [ax + 257.0, row + 86.0]], if cmd == "fail" { "fail" } else { "live" }, if cmd == "fail" { "5 5" } else { "" }));
        wires.push(wire_d(&[[ax + 57.0, row + 86.0], [ax + 57.0, row + 108.0], [140.0, row + 108.0], [140.0, y + 28.0], [128.0, y + 28.0]], if lvdt_com == "fail" { "fail" } else { "live" }, "6 4"));
        wires.push(wire_d(&[[ax + 157.0, row + 86.0], [ax + 157.0, row + 116.0], [136.0, row + 116.0], [136.0, y + 66.0], [128.0, y + 66.0]], "live", "6 4"));
        fg.push(ex_t(ax + 300.0, row + 112.0, "FEEDBACK COM / MON", T { size: 8.0, anchor: "start", fill: if lvdt_com == "fail" { NS_RED } else { NS_GREY_TEXT }, ..TD }));
        let supply = bad(mode == "supply", "fail");
        wires.push(wire_d(&[[ax + 44.0, row + 30.0], [ax + 44.0, row + 46.0], [ax - 30.0, row + 46.0], [ax - 30.0, y + ah + 6.0]], supply, if supply == "fail" { "5 5" } else { "" }));
        let cap2 = if is_hyd { format!("{} SYSTEM 5000 PSI", if *p == "G" { "GREEN" } else { "YELLOW" }) } else { format!("{} BUS 115 VAC", p.replacen("E ", "AC ", 1)) };
        fg.push(ex_t(ax - 24.0, y + ah + 2.0, &cap2, T { size: 9.0, anchor: "start", fill: if supply == "fail" { NS_RED } else { NS_GREY_TEXT }, ..TD }));
        let rod = bad(mode == "disconnect", "fail");
        wires.push(wire_d(&[[px + 128.0, row + 15.0], [hinge_x - 30.0, hinge_y + if i > 0 { 26.0 } else { -26.0 }]], rod, if rod == "fail" { "7 6" } else { "" }));
    }
    let defl = match mode {
        "hardover" => 25.0,
        "disconnect" => -12.0,
        _ => 0.0,
    };
    let surf_st = match mode {
        "separated" | "jam" | "hardover" | "disconnect" => "fail",
        "fault" | "supply" | "damper" => "warn",
        _ => "live",
    };
    let c = ns_stroke(surf_st);
    let poly = match &sf.bx {
        Some(b) => surface_poly(b, "R").unwrap_or_default(),
        None => vec![[0.0, 0.0], [1.6, 0.0], [1.6, 9.0], [0.0, 9.0]],
    };
    let xs: Vec<f64> = poly.iter().map(|q| q[0]).collect();
    let zs: Vec<f64> = poly.iter().map(|q| q[1]).collect();
    let mn = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
    let mx = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let sx = 150.0 / 1f64.max(mx(&xs) - mn(&xs));
    let sz = 160.0 / 1f64.max(mx(&zs) - mn(&zs));
    let sc = sx.min(sz);
    let pp: Vec<String> = poly.iter().map(|q| format!("{},{}", fx(hinge_x + (q[0] - mn(&xs)) * sc, 1), fx(hinge_y - 80.0 + (q[1] - mn(&zs)) * sc, 1))).collect();
    fg.push(format!(
        "<g transform=\"rotate({} {} {})\"><path d=\"M{} Z\" fill=\"{}\" stroke=\"{c}\" stroke-width=\"2.4\"{}/></g>",
        n(defl),
        n(hinge_x),
        n(hinge_y),
        pp.join(" L"),
        if surf_st == "fail" { "rgba(255,61,99,0.15)" } else { NS_FILL },
        if mode == "separated" { " stroke-dasharray=\"7 5\"" } else { "" }
    ));
    fg.push(format!("<path d=\"M{},{} L{},{}\" stroke=\"{NS_GREY}\" stroke-width=\"1.4\" stroke-dasharray=\"8 5\"/>", n(hinge_x - 10.0), n(hinge_y - 100.0), n(hinge_x - 10.0), n(hinge_y + 100.0)));
    fg.push(ex_t(hinge_x - 10.0, hinge_y + 116.0, "HINGE LINE", T { size: 9.0, fill: NS_GREY_TEXT, ..TD }));
    let status = match mode {
        "separated" => "SURFACE SEPARATED",
        "disconnect" => "DISCONNECTED - FLOATING",
        "hardover" => "HARDOVER - FULL DEFLECTION",
        "jam" => "JAMMED - SURFACE FROZEN",
        "supply" => "ACTUATOR IN DAMPING MODE",
        "damper" => "FLUTTER DAMPING LOST",
        "servovalve" => "SERVO VALVE LEAKING",
        "seal" => "PISTON SEAL WEAR",
        "lvdt" => "POSITION FEEDBACK LOST",
        "wiring" => "COMMAND / MONITOR WIRING FAULT",
        _ => "ACTUATOR FAULT - OTHER ACTUATOR ACTIVE",
    };
    fg.push(ex_t(hinge_x + 70.0, hinge_y + 140.0, status, T { size: 11.0, fill: if surf_st == "live" { NS_ORANGE } else { c }, ..TD }));
    if !title.is_empty() {
        fg.push(ex_t(w - 16.0, h - 8.0, title, T { anchor: "end", size: 12.0, fill: NS_GREY_TEXT, weight: 500.0, ..TD }));
    }
    wires.extend(fg);
    Some(ns_svg(w, h, &wires, None))
}

pub fn fctl_view(m: &OMap, title: &str) -> Node {
    let (w, h) = (1000.0, 520.0);
    let mut wires = Vec::new();
    let mut fg = Vec::new();
    let s = |k: &str| m.st(k, "live");
    for (i, c) in COMPUTERS.iter().enumerate() {
        let x = 20.0 + i as f64 * 88.0;
        let failed = s(c) == "fail";
        wires.push(wire_d(&[[x + 40.0, 64.0], [x + 40.0, 92.0]], if failed { "fail" } else { "live" }, if failed { "5 5" } else { "" }));
        fg.push(ns_box(x, 28.0, 80.0, 36.0, c, s(c), B { size: 11.0, tag: match m.get(c) { Some("fail") => "FAILED", Some(_) => "FAULT", None => "" }, ..Default::default() }));
    }
    wires.push(wire(&[[8.0, 92.0], [992.0, 92.0]], "live"));
    fg.push(ex_t(500.0, 86.0, "FLIGHT CONTROL COMMAND BUSES", T { size: 9.0, fill: NS_GREY_TEXT, ..TD }));
    if let Some(law) = fctl_law(m) {
        fg.push(ex_t(980.0, 128.0, law, T { size: 14.0, anchor: "end", fill: NS_ORANGE, ..TD }));
    }
    let surface = |wires: &mut Vec<String>, fg: &mut Vec<String>, key: &str, label: &str, x: f64, y: f64, bw: f64, power: &[&str]| {
        let (cst, ctag) = fctl_control(key, m);
        let st = m.get(key).unwrap_or(cst);
        wires.push(wire(&[[x + bw / 2.0, y - 22.0], [x + bw / 2.0, y]], if st == "fail" { "fail" } else { "live" }));
        let tag = match m.get(key) {
            Some("fail") => "FAILED".to_string(),
            Some(_) => "DEGRADED".into(),
            None => ctag,
        };
        fg.push(ns_box(x, y, bw, 30.0, label, st, B { size: 10.0, r: 7.0, tag: &tag }));
        if !power.is_empty() {
            fg.push(ex_t(
                x + bw / 2.0,
                y + 42.0,
                &power.iter().map(|p| p.replacen("E ", "E\u{b7}", 1)).collect::<Vec<_>>().join(" "),
                T { size: 8.0, fill: if st == "fail" { NS_RED } else { "#8fd9b0" }, weight: 700.0, ..TD },
            ));
        }
    };
    let (lr, rr) = (10.0, 990.0);
    for (y, xa, xb) in [(166.0, lr, 486.0), (256.0, lr, 486.0), (166.0, 514.0, rr), (256.0, 514.0, rr), (380.0, lr, rr)] {
        wires.push(wire(&[[xa, y - 22.0], [xb, y - 22.0]], "live"));
    }
    wires.push(wire(&[[lr, 92.0], [lr, 358.0]], "live"));
    wires.push(wire(&[[rr, 92.0], [rr, 358.0]], "live"));
    for (side, x0) in [("L", 20.0), ("R", 520.0)] {
        fg.push(ex_t(x0 + 6.0, 128.0, if side == "L" { "LEFT WING" } else { "RIGHT WING" }, T { size: 11.0, anchor: "start", fill: NS_GREY_TEXT, ..TD }));
        let row: [(&str, &str, &str); 5] = if side == "L" {
            [("AIL_L_OUT", "OUT AIL", "AIL_OUT"), ("AIL_L_MID", "MID AIL", "AIL_MID"), ("AIL_L_IN", "IN AIL", "AIL_IN"), ("SLATS", "SLATS", "SLATS"), ("FLAPS", "FLAPS", "FLAPS")]
        } else {
            [("FLAPS", "FLAPS", "FLAPS"), ("SLATS", "SLATS", "SLATS"), ("AIL_R_IN", "IN AIL", "AIL_IN"), ("AIL_R_MID", "MID AIL", "AIL_MID"), ("AIL_R_OUT", "OUT AIL", "AIL_OUT")]
        };
        for (i, (k, label, pk)) in row.iter().enumerate() {
            surface(&mut wires, &mut fg, k, label, x0 + i as f64 * 92.0, 166.0, 84.0, fctl_power(pk));
        }
        let spl: Vec<u32> = if side == "L" { (1..=8).rev().collect() } else { (1..=8).collect() };
        for (i, k) in spl.iter().enumerate() {
            let pw: &[&str] = if *k == 6 { &["G", "E ESS"] } else if k % 2 == 1 { &["Y"] } else { &["G"] };
            surface(&mut wires, &mut fg, &format!("SPLR_{side}{k}"), &format!("SPLR {k}"), x0 + i as f64 * 58.0, 256.0, 52.0, pw);
        }
    }
    fg.push(ex_t(26.0, 344.0, "EMPENNAGE", T { size: 11.0, anchor: "start", fill: NS_GREY_TEXT, ..TD }));
    for (i, (k, label)) in [("ELEV_L_OUT", "L OUT ELEV"), ("ELEV_L_IN", "L IN ELEV"), ("RUD_UP", "UPPER RUDDER"), ("RUD_LO", "LOWER RUDDER"), ("THS", "THS"), ("ELEV_R_IN", "R IN ELEV"), ("ELEV_R_OUT", "R OUT ELEV")].iter().enumerate() {
        surface(&mut wires, &mut fg, k, label, 26.0 + i as f64 * 136.0, 380.0, 120.0, fctl_power(k));
    }
    fg.push(ex_t(26.0, 462.0, "POWER", T { size: 10.0, anchor: "start", fill: NS_GREY_TEXT, ..TD }));
    fg.push(ex_t(
        26.0,
        480.0,
        "G / Y  HYDRAULIC SERVO   E\u{b7}ESS  AC ESS EHA/EBHA   E\u{b7}EHA  AC EHA (247XP)   E\u{b7}AC1  AC 1   E  ELECTRIC MOTOR",
        T { size: 9.0, anchor: "start", fill: NS_GREY_TEXT, weight: 500.0, ..TD },
    ));
    if !title.is_empty() {
        fg.push(ex_t(w - 16.0, h - 8.0, title, T { anchor: "end", size: 12.0, fill: NS_GREY_TEXT, weight: 500.0, ..TD }));
    }
    wires.extend(fg);
    ns_svg(w, h, &wires, None)
}

use super::{ex_svg, ex_t, EX_DIM, T, TD};
use crate::jv::{fx, n};
use crate::Node;
use regex::Regex;

const ENG_CASE: &str = "rgba(255,255,255,0.06)";
const ENG_METAL: &str = "rgba(200,210,220,0.18)";
const ENG_BLADE: &str = "#cfd6dd";
const ENG_BYPASS: &str = "#5ad1ff";
const ENG_COLD: &str = "#8fd8ff";
const ENG_WARM: &str = "#ffd36b";
const ENG_HOT: &str = "#ff7a2f";
const ENG_FLAME: &str = "#ff4b1f";
const ENG_BAD: &str = "#ff3b3b";
const ENG_WARN: &str = "#ffb000";
const ENG_OIL: &str = "#c9a227";
const ENG_FUEL: &str = "#ff9de2";
const ENG_BLEED: &str = "#7cf0b0";

const PARTS: [(&str, f64, &str); 43] = [
    ("inlet", 70.0, "INLET"), ("spinner", 120.0, "SPINNER"), ("fan", 168.0, "FAN"), ("fanCase", 250.0, "FAN CASE"), ("vsv", 232.0, "IP VSV"), ("ipc", 285.0, "IP COMPRESSOR"),
    ("hpc", 425.0, "HP COMPRESSOR"), ("combustor", 515.0, "COMBUSTOR"), ("nozzles", 487.0, "FUEL NOZZLES"), ("igniter", 500.0, "IGNITERS"), ("hpt", 575.0, "HP TURBINE"),
    ("ipt", 608.0, "IP TURBINE"), ("lpt", 690.0, "LP TURBINE"), ("exhaust", 840.0, "EXHAUST"), ("bypassDuct", 420.0, "BYPASS DUCT"), ("reverser", 470.0, "THRUST REVERSER"),
    ("cowl", 300.0, "FAN COWL"), ("brgFan", 205.0, "FAN FRONT BRG"), ("brgIp", 228.0, "IP FRONT BRG"), ("brgHp", 562.0, "HP TURBINE BRG"), ("brgIpt", 625.0, "IP TURBINE BRG"),
    ("brgLp", 772.0, "LP TURBINE REAR BRG"), ("gearbox", 350.0, "GEARBOX"), ("starter", 300.0, "STARTER"), ("oilPump", 335.0, "OIL PUMP"), ("oilTank", 405.0, "OIL TANK"),
    ("fuelPump", 370.0, "FUEL PUMPS"), ("fmu", 440.0, "FMU"), ("generator", 270.0, "VFG"), ("edp", 430.0, "EDP A/B"), ("eec", 300.0, "EEC"), ("bleedIp", 330.0, "IP8 BLEED"),
    ("bleedHp", 455.0, "HP6 BLEED"), ("precooler", 470.0, "PRECOOLER"), ("t25", 352.0, "T25"), ("p30", 470.0, "P30"), ("tgt", 600.0, "TGT"), ("vibFan", 215.0, "VIB FAN"),
    ("vibCore", 540.0, "VIB CORE"), ("speed", 190.0, "SPEED PICKUP"), ("fanZone", 330.0, "FAN ZONE"), ("coreZone", 620.0, "CORE ZONE"), ("", 0.0, ""),
];

fn part(k: &str) -> Option<(f64, &'static str)> {
    PARTS.iter().find(|p| p.0 == k && !k.is_empty()).map(|p| (p.1, p.2))
}

#[derive(Clone, Debug)]
pub struct FailIn {
    pub id: f64,
    pub name: String,
    pub active: bool,
    pub magnitude: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fault {
    pub mode: String,
    pub mag: f64,
}

pub type Faults = Vec<(String, Fault)>;

fn put(out: &mut Faults, k: &str, mode: &str, mag: f64) {
    match out.iter_mut().find(|(a, _)| a == k) {
        Some((_, prev)) => {
            if mag > prev.mag {
                *prev = Fault { mode: mode.into(), mag };
            }
        }
        None => out.push((k.into(), Fault { mode: mode.into(), mag })),
    }
}

pub fn eng_faults(failures: &[FailIn], engine: usize) -> Faults {
    let mut out = Faults::new();
    for f in failures {
        if !f.active {
            continue;
        }
        let id = f.id;
        let mag = if f.magnitude > 0.0 { f.magnitude } else { 1.0 };
        let nm = f.name.to_uppercase();
        let en: Option<usize> = re!(r"\bENG(?:INE)?\s*(\d)").captures(&nm).and_then(|c| c[1].parse().ok());
        if id < 100000.0 {
            let base = id - (((id - 72000.0) % 4.0) + 4.0) % 4.0;
            let e = id - base + 1.0;
            let th = (id / 1000.0).floor();
            if th == 72.0 || th == 54.0 {
                if e != engine as f64 && en != Some(engine) {
                    continue;
                }
            } else if en != Some(engine) {
                continue;
            }
            if base == 72000.0 {
                put(&mut out, "brgHp", "wear", mag);
            } else if base == 72004.0 {
                put(&mut out, "ipc", "stall", mag);
                put(&mut out, "hpc", "stall", mag);
            } else if base == 72008.0 {
                put(&mut out, "hpt", "damage", mag);
            } else if base == 72012.0 {
                put(&mut out, "hpc", "destroyed", mag);
            } else if base == 72016.0 {
                put(&mut out, "hpt", "released", mag);
            } else if base == 72020.0 {
                put(&mut out, "brgHp", "seized", mag);
            } else if base == 72024.0 {
                put(&mut out, "inlet", "fod", mag);
                put(&mut out, "hpc", "destroyed", mag);
            } else if (54010.0..54014.0).contains(&id) {
                put(&mut out, "cowl", "separated", 1.0);
            } else {
                keywords(&nm, &mut out, mag);
            }
            continue;
        }
        if en != Some(engine) {
            continue;
        }
        keywords(&nm, &mut out, mag);
    }
    out
}

fn keywords(n: &str, out: &mut Faults, mag: f64) {
    let rules: [(&Regex, &str, &str); 41] = [
        (re!(r"\bFAIL$|SHUT ?DOWN|FLAMEOUT|FLAME OUT|RELIGHT"), "combustor", "out"),
        (re!(r"\bSTART\b|STARTER|IGN START"), "starter", "fault"),
        (re!(r"OVERLIMIT|OVER LIMIT|OVERSPEED"), "tgt", "fault"),
        (re!(r"FAN FRONT BRG"), "brgFan", "wear"),
        (re!(r"IP FRONT BRG"), "brgIp", "wear"),
        (re!(r"HP TURBINE BRG"), "brgHp", "wear"),
        (re!(r"IP TURBINE BRG"), "brgIpt", "wear"),
        (re!(r"LP TURBINE REAR BRG"), "brgLp", "wear"),
        (re!(r"BEARING"), "brgHp", "wear"),
        (re!(r"BLADE LOSS|FAN BLADE|BIRD STRIKE FAN"), "fan", "damage"),
        (re!(r"BIRD STRIKE CORE|INGESTION|FOD"), "inlet", "fod"),
        (re!(r"VSV"), "vsv", "jam"),
        (re!(r"T25"), "t25", "fault"),
        (re!(r"P30"), "p30", "fault"),
        (re!(r"TGT|EGT"), "tgt", "fault"),
        (re!(r"VIBRATION PICKUP \(FAN\)|VIB.*FAN"), "vibFan", "fault"),
        (re!(r"VIBRATION PICKUP|VIB"), "vibCore", "fault"),
        (re!(r"SPEED PICKUP|TACHOMETER|N1 |N2 |N3 "), "speed", "fault"),
        (re!(r"NOZZLE|COKING"), "nozzles", "coking"),
        (re!(r"METERING|FMU"), "fmu", "stuck"),
        (re!(r"FUEL PUMP|LP FUEL|HP FUEL|STRAINER|FUEL FILTER|FUEL FLOW"), "fuelPump", "fault"),
        (re!(r"OIL PUMP|OIL PRESS|SCAVENGE"), "oilPump", "fault"),
        (re!(r"OIL (?:LEAK|QTY|QUANTITY|TANK)|OIL FILTER|CHIP"), "oilTank", "fault"),
        (re!(r"OIL TEMP|OIL COOLER|FOHE"), "oilTank", "hot"),
        (re!(r"STARTER"), "starter", "fault"),
        (re!(r"IGNIT"), "igniter", "fault"),
        (re!(r"FADEC|EEC"), "eec", "fault"),
        (re!(r"REVERSER|REV "), "reverser", "fault"),
        (re!(r"PRECOOLER"), "precooler", "fault"),
        (re!(r"HP BLEED|HP VALVE"), "bleedHp", "stuck"),
        (re!(r"BLEED|IP CHECK"), "bleedIp", "fault"),
        (re!(r"GEN(?:ERATOR)?|VFG|IDG"), "generator", "fault"),
        (re!(r"EDP|ENGINE DRIVEN PUMP|PUMP [AB]"), "edp", "fault"),
        (re!(r"FIRE.*FAN ZONE|FAN ZONE"), "fanZone", "fire"),
        (re!(r"FIRE|LOOP"), "coreZone", "?"),
        (re!(r"GEARBOX|AGB"), "gearbox", "fault"),
        (re!(r"COMPRESSOR STALL|SURGE"), "hpc", "stall"),
        (re!(r"COMPRESSOR EROSION|ASH.*COMPRESSOR"), "hpc", "erosion"),
        (re!(r"NGV|GLASSING"), "hpt", "damage"),
        (re!(r"ICE CRYSTAL|ACCRETION"), "ipc", "ice"),
        (re!(r"FLAMEOUT|ROLL-?BACK"), "combustor", "out"),
    ];
    for (r, k, mode) in rules {
        if r.is_match(n) {
            let mode = if mode == "?" { if re!(r"DETECT|LOOP|OPEN|SHORT|FALSE").is_match(n) { "loop" } else { "fire" } } else { mode };
            put(out, k, mode, mag);
            return;
        }
    }
    if re!(r"CASE|CONTAINMENT").is_match(n) {
        put(out, "fanCase", "damage", mag);
        return;
    }
    put(out, "combustor", "fault", mag);
}

#[derive(Clone, Debug, Default)]
pub struct EngOpts {
    pub n1: f64,
    pub n3: f64,
    pub running: Option<bool>,
    pub faults: Faults,
    pub title: String,
    pub readout: String,
}

pub fn eng_view(o: &EngOpts) -> Vec<String> {
    let (w, h, a) = (1000.0, 490.0, 220.0);
    let faults = &o.faults;
    let f = |k: &str| faults.iter().find(|(x, _)| x == k).map(|(_, v)| v);
    let mode_is = |k: &str, m: &str| f(k).is_some_and(|v| v.mode == m);
    let n1 = o.n1.max(0.0);
    let n3 = o.n3.max(0.0);
    let lit = o.running.unwrap_or(n3 > 50.0) && !mode_is("combustor", "out") && !mode_is("hpc", "destroyed") && !mode_is("brgHp", "seized");
    let hp_stopped = mode_is("brgHp", "seized");
    let surge = mode_is("hpc", "stall") || mode_is("ipc", "stall");
    let shattered = mode_is("hpc", "destroyed");
    let shake = faults
        .iter()
        .filter(|(k, _)| re!(r"^brg|fan|hpt").is_match(k))
        .map(|(_, v)| (if v.mode == "released" || v.mode == "damage" { 3.0 } else if v.mode == "seized" { 0.0 } else { 1.2 }) * v.mag)
        .fold(0.0f64, f64::max);
    let mut parts = Vec::new();
    let sym = |p: &[[f64; 2]]| -> Vec<[f64; 2]> {
        let mut v: Vec<[f64; 2]> = p.iter().map(|q| [q[0], a - q[1]]).collect();
        v.extend(p.iter().rev().map(|q| [q[0], a + q[1]]));
        v
    };
    let poly = |p: &[[f64; 2]], fill: &str, stroke: &str, sw: f64, extra: &str| -> String {
        format!("<path d=\"{}Z\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{}\" stroke-linejoin=\"round\"{extra}/>", super::d_path(p), n(sw))
    };
    let band = |outer: &[[f64; 2]], inner: &[[f64; 2]], fill: &str, stroke: &str| -> String {
        let mut top: Vec<[f64; 2]> = outer.iter().map(|q| [q[0], a - q[1]]).collect();
        top.extend(inner.iter().rev().map(|q| [q[0], a - q[1]]));
        let mut bot: Vec<[f64; 2]> = outer.iter().map(|q| [q[0], a + q[1]]).collect();
        bot.extend(inner.iter().rev().map(|q| [q[0], a + q[1]]));
        poly(&top, fill, stroke, 1.4, "") + &poly(&bot, fill, stroke, 1.4, "")
    };
    let hl = |k: &str| -> Option<&'static str> {
        f(k).map(|v| if v.mode == "fire" || re!(r"destroyed|released|seized|separated").is_match(&v.mode) { ENG_BAD } else { ENG_WARN })
    };
    let glow = |k: &str| if f(k).is_some() { " filter=\"url(#engGlow)\"" } else { "" };
    parts.push(format!(
        "<defs><filter id=\"engGlow\" x=\"-50%\" y=\"-50%\" width=\"200%\" height=\"200%\"><feGaussianBlur stdDeviation=\"4\" result=\"b\"/><feMerge><feMergeNode in=\"b\"/><feMergeNode in=\"SourceGraphic\"/></feMerge></filter>\n<linearGradient id=\"engCore\" x1=\"0\" x2=\"1\" y1=\"0\" y2=\"0\"><stop offset=\"0\" stop-color=\"{ENG_COLD}\"/><stop offset=\"0.42\" stop-color=\"{ENG_WARM}\"/><stop offset=\"0.52\" stop-color=\"{}\"/><stop offset=\"0.75\" stop-color=\"{}\"/><stop offset=\"1\" stop-color=\"{}\"/></linearGradient>\n<style>.engflow{{fill:none;stroke-linecap:round;animation:engdash linear infinite}}.engrev{{animation:engrev 0.9s ease-in-out infinite}}.engshake{{animation:engshake 0.09s linear infinite}}.engpulse{{animation:engpulse 0.8s ease-in-out infinite}}.engspark{{animation:engspark 1.2s ease-out infinite}}\n@keyframes engdash{{to{{stroke-dashoffset:-60}}}}@keyframes engrev{{0%,60%{{stroke-dashoffset:0}}80%{{stroke-dashoffset:40}}100%{{stroke-dashoffset:-60}}}}@keyframes engshake{{0%{{transform:translate(0,0)}}25%{{transform:translate(var(--sx),0)}}50%{{transform:translate(0,var(--sy))}}75%{{transform:translate(calc(var(--sx) * -1),0)}}}}\n@keyframes engpulse{{0%,100%{{opacity:0.25}}50%{{opacity:1}}}}@keyframes engspark{{0%{{opacity:1;transform:translate(0,0)}}100%{{opacity:0;transform:translate(var(--dx),var(--dy))}}}}</style></defs>",
        if lit { ENG_FLAME } else { ENG_WARM },
        if lit { ENG_HOT } else { ENG_WARM },
        if lit { "#ffb46b" } else { ENG_COLD }
    ));
    let nac_outer = [[40.0, 176.0], [60.0, 186.0], [120.0, 190.0], [240.0, 192.0], [420.0, 186.0], [500.0, 178.0], [600.0, 150.0]];
    let nac_inner = [[40.0, 168.0], [70.0, 158.0], [150.0, 156.0], [200.0, 158.0], [420.0, 160.0], [520.0, 152.0], [600.0, 144.0]];
    if f("cowl").is_some() {
        let o2: Vec<[f64; 2]> = nac_outer.iter().copied().filter(|q| q[0] >= 420.0).collect();
        let i2: Vec<[f64; 2]> = nac_inner.iter().copied().filter(|q| q[0] >= 420.0).collect();
        parts.push(band(&o2, &i2, ENG_CASE, hl("cowl").unwrap_or("null")));
    } else {
        parts.push(band(&nac_outer, &nac_inner, ENG_CASE, if f("fanZone").is_some() { ENG_BAD } else { "#9aa6b2" }));
    }
    parts.push(band(
        &[[200.0, 156.0], [260.0, 158.0], [420.0, 158.0]],
        &[[200.0, 150.0], [260.0, 150.0], [420.0, 152.0]],
        if f("fanCase").is_some() { "rgba(255,59,59,0.35)" } else { ENG_METAL },
        hl("fanCase").unwrap_or("#9aa6b2"),
    ));
    let core_cowl = [[200.0, 96.0], [300.0, 104.0], [420.0, 100.0], [560.0, 92.0], [640.0, 100.0], [780.0, 104.0], [800.0, 96.0]];
    let core_case = [[205.0, 90.0], [300.0, 86.0], [370.0, 70.0], [470.0, 62.0], [560.0, 80.0], [640.0, 92.0], [780.0, 100.0]];
    parts.push(band(&core_cowl, &core_case, if f("coreZone").is_some() { "rgba(255,59,59,0.25)" } else { ENG_CASE }, if f("coreZone").is_some() { ENG_BAD } else { "#7f8a95" }));
    parts.push(poly(&sym(&[[780.0, 100.0], [880.0, 80.0], [905.0, 72.0]]), "none", "#7f8a95", 1.4, ""));
    parts.push(poly(&sym(&[[760.0, 46.0], [840.0, 36.0], [930.0, 4.0]]), "rgba(255,255,255,0.08)", "#9aa6b2", 1.4, ""));
    let hub = [[200.0, 44.0], [300.0, 46.0], [370.0, 40.0], [470.0, 40.0], [560.0, 48.0], [640.0, 52.0], [760.0, 46.0]];
    parts.push(band(&core_case, &hub, "rgba(0,0,0,0)", "rgba(255,255,255,0.15)"));
    let flow_w = 0.5 + 1.8 * (n1 / 90.0).min(1.2);
    let bypass_dur: Option<String> = (n1 > 2.0).then(|| fx(2.2 / (n1 / 85.0).max(0.15), 2));
    let core_dur: Option<String> = (n3 > 5.0 && !hp_stopped).then(|| fx(1.6 / (n3 / 95.0).max(0.2), 2));
    let stream = |d: &str, stroke: &str, dur: Option<&str>, w: f64, cls: &str, dash: &str| -> String {
        match dur {
            Some(dur) => format!("<path d=\"{d}\" class=\"engflow {cls}\" fill=\"none\" stroke-linecap=\"round\" stroke=\"{stroke}\" stroke-width=\"{}\" stroke-dasharray=\"{dash}\" style=\"animation-duration:{dur}s\" opacity=\"0.85\"/>", n(w)),
            None => format!("<path d=\"{d}\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"{}\" stroke-dasharray=\"2 14\" opacity=\"0.35\"/>", n(w * 0.6)),
        }
    };
    for s in [-1.0, 1.0] {
        for r in [112.0, 126.0, 140.0] {
            let d = format!(
                "M20,{} L190,{} L420,{} L620,{} L720,{}",
                n(a + s * (r + 30.0)),
                n(a + s * r),
                n(a + s * (r + 2.0)),
                n(a + s * (r - 6.0)),
                n(a + s * (r - 14.0))
            );
            parts.push(stream(&d, ENG_BYPASS, bypass_dur.as_deref(), flow_w, "", "10 20"));
        }
        let turb = shattered || surge;
        for r in [55.0, 68.0, 80.0] {
            let hm = f("hpc").map(|v| v.mag).unwrap_or(1.0);
            let wob = |k: i32| if turb { (if k % 2 != 0 { 9.0 } else { -9.0 }) * hm } else { 0.0 };
            let p = [
                [20.0, r + 6.0], [190.0, r + 8.0], [230.0, r - 4.0 + wob(1)], [330.0, r - 14.0 + wob(2)], [420.0, r - 26.0 + wob(3)], [470.0, r - 22.0 + wob(4)], [520.0, r - 12.0 + wob(5)],
                [580.0, r - 6.0], [700.0, r + 6.0], [800.0, r - 10.0], [960.0, r - 30.0],
            ];
            let d = format!("M{}", p.iter().map(|q| format!("{},{}", n(q[0]), n(a + s * q[1].max(6.0)))).collect::<Vec<_>>().join(" L"));
            if shattered {
                let dur = core_dur.as_ref().map(|c| fx(c.parse::<f64>().unwrap() * 3.0, 2)).unwrap_or("2.5".into());
                parts.push(stream(&d, "url(#engCore)", Some(&dur), flow_w * 0.8, "", "3 9 14 26"));
            } else {
                parts.push(stream(&d, "url(#engCore)", core_dur.as_deref(), flow_w, if surge { "engrev" } else { "" }, "10 20"));
            }
        }
    }
    let shake_style = if shake > 0.0 { format!(" class=\"engshake\" style=\"--sx:{}px;--sy:{}px\"", fx(shake, 1), fx(shake * 0.7, 1)) } else { String::new() };
    parts.push(format!("<g{shake_style}>"));
    parts.push(poly(&sym(&[[90.0, 0.0], [120.0, 26.0], [160.0, 42.0], [170.0, 44.0]]), "rgba(255,255,255,0.12)", hl("spinner").unwrap_or("#cfd6dd"), 1.4, ""));
    let fan_col = hl("fan").unwrap_or(ENG_BLADE);
    for s in [-1.0, 1.0] {
        for k in 0..5 {
            let kf = k as f64;
            let y0 = a + s * 44.0;
            let y1 = a + s * (150.0 - kf * 0.5);
            let broken = f("fan").is_some() && k == 2;
            parts.push(format!(
                "<path d=\"M{},{} Q{},{} {},{}\" fill=\"none\" stroke=\"{fan_col}\" stroke-width=\"{}\" opacity=\"{}\"/>",
                n(162.0 + kf * 4.0),
                n(y0),
                n(176.0 + kf * 4.0),
                n((y0 + y1) / 2.0),
                n(168.0 + kf * 4.0),
                n(if broken { a + s * 95.0 } else { y1 }),
                if k == 2 { "3" } else { "1.4" },
                if k == 2 { "1" } else { "0.55" }
            ));
        }
    }
    let stage = |x0: f64, cnt: usize, gap: f64, ri0: f64, ri1: f64, ro0: f64, ro1: f64, key: &str, w: f64| -> String {
        let c = hl(key).unwrap_or(ENG_BLADE);
        let mut out = String::new();
        for k in 0..cnt {
            let t = if cnt > 1 { k as f64 / (cnt - 1) as f64 } else { 0.0 };
            let x = x0 + k as f64 * gap;
            let ri = ri0 + (ri1 - ri0) * t;
            let ro = ro0 + (ro1 - ro0) * t;
            for s in [-1.0, 1.0] {
                let broken_at = if key == "hpc" && shattered && k % 2 == 0 {
                    ri + (ro - ri) * 0.35
                } else if key == "hpt" && mode_is("hpt", "released") {
                    ri + (ro - ri) * 0.4
                } else {
                    ro
                };
                out += &format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{c}\" opacity=\"0.9\"/>",
                    n(x),
                    n((a + s * ri).min(a + s * broken_at)),
                    n(w),
                    n((broken_at - ri).abs())
                );
                if k + 1 < cnt {
                    out += &format!(
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{c}\" opacity=\"0.35\"/>",
                        n(x + gap / 2.0),
                        n((a + s * (ri + 4.0)).min(a + s * ro)),
                        n(w * 0.6),
                        n((ro - ri - 4.0).abs())
                    );
                }
                if broken_at < ro {
                    out += &format!("<path d=\"M{},{} l3,{} l3,{} l3,{}\" stroke=\"{ENG_BAD}\" stroke-width=\"1.6\" fill=\"none\"/>", n(x - 2.0), n(a + s * broken_at), n(s * 5.0), n(-s * 4.0), n(s * 6.0));
                }
            }
        }
        format!("<g{}>{out}</g>", glow(key))
    };
    parts.push(stage(222.0, 2, 10.0, 50.0, 50.0, 88.0, 86.0, "vsv", 3.0));
    parts.push(stage(250.0, 7, 14.0, 48.0, 46.0, 86.0, 72.0, "ipc", 4.0));
    parts.push(stage(378.0, 6, 15.0, 42.0, 44.0, 66.0, 60.0, "hpc", 4.0));
    parts.push(stage(568.0, 1, 0.0, 50.0, 50.0, 82.0, 82.0, "hpt", 7.0));
    parts.push(stage(602.0, 1, 0.0, 52.0, 52.0, 92.0, 92.0, "ipt", 7.0));
    parts.push(stage(632.0, 5, 26.0, 50.0, 46.0, 98.0, 108.0, "lpt", 6.0));
    parts.push("</g>".into());
    let flame = if lit { ENG_FLAME } else { "rgba(120,120,120,0.4)" };
    for s in [-1.0, 1.0] {
        parts.push(poly(
            &[[470.0, a + s * 48.0], [490.0, a + s * 44.0], [560.0, a + s * 52.0], [560.0, a + s * 80.0], [490.0, a + s * 80.0], [470.0, a + s * 70.0]],
            if f("combustor").is_some() { "rgba(255,59,59,0.2)" } else { "rgba(255,255,255,0.04)" },
            hl("combustor").unwrap_or("#9aa6b2"),
            1.4,
            "",
        ));
        parts.push(format!(
            "<ellipse cx=\"520\" cy=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"{flame}\" opacity=\"{}\"{}/>",
            n(a + s * 64.0),
            if lit { "26" } else { "10" },
            if lit { "9" } else { "4" },
            if lit { "0.75" } else { "0.5" },
            if lit { " filter=\"url(#engGlow)\"" } else { "" }
        ));
        parts.push(format!("<circle cx=\"487\" cy=\"{}\" r=\"5\" fill=\"{}\"{}/>", n(a + s * 63.0), hl("nozzles").unwrap_or(ENG_FUEL), glow("nozzles")));
        parts.push(format!("<path d=\"M500,{} l0,{}\" stroke=\"{}\" stroke-width=\"3\"{}/>", n(a + s * 80.0), n(s * 10.0), hl("igniter").unwrap_or(ENG_WARM), glow("igniter")));
    }
    if shattered || mode_is("hpt", "released") {
        let cx = if shattered { 430.0 } else { 580.0 };
        for k in 0..9 {
            let ang = (k as f64 / 9.0) * std::f64::consts::PI * 2.0;
            parts.push(format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"2.5\" fill=\"{ENG_WARM}\" class=\"engspark\" style=\"--dx:{}px;--dy:{}px;animation-delay:{}s\"/>",
                n(cx),
                n(a - 60.0),
                fx(ang.cos() * 120.0, 0),
                fx(ang.sin() * 120.0, 0),
                fx(k as f64 * 0.13, 2)
            ));
        }
    }
    if surge {
        for s in [-1.0, 1.0] {
            parts.push(format!("<path d=\"M190,{} l-40,{}\" stroke=\"{ENG_FLAME}\" stroke-width=\"5\" class=\"engpulse\" filter=\"url(#engGlow)\"/>", n(a + s * 70.0), n(s * 6.0)));
        }
    }
    if f("inlet").is_some() {
        parts.push(format!("<path d=\"M60,{} l14,-10 l10,12 l-8,14 z\" fill=\"{ENG_WARN}\" stroke=\"{ENG_BAD}\"/>", n(a - 40.0)));
    }
    let shaft = |x0: f64, x1: f64, r: f64, col: &str| format!("<path d=\"M{},{} L{},{} M{},{} L{},{}\" stroke=\"{col}\" stroke-width=\"2\"/>", n(x0), n(a - r), n(x1), n(a - r), n(x0), n(a + r), n(x1), n(a + r));
    parts.push(shaft(165.0, 780.0, 8.0, "#7d8a96") + &shaft(240.0, 620.0, 15.0, "#8f9caa") + &shaft(380.0, 580.0, 22.0, if hp_stopped { ENG_BAD } else { "#a8b4c0" }));
    let brg = |k: &str, r: f64| -> String {
        let c = hl(k);
        let x = part(k).map(|p| p.0).unwrap_or(0.0);
        [-1.0, 1.0]
            .iter()
            .map(|s| {
                format!(
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.6\"{}/>",
                    n(x),
                    n(a + s * r),
                    if c.is_some() { "8" } else { "5" },
                    c.unwrap_or("rgba(0,0,0,0.5)"),
                    c.unwrap_or("#cfd6dd"),
                    if c.is_some() { " class=\"engpulse\" filter=\"url(#engGlow)\"" } else { "" }
                )
            })
            .collect()
    };
    parts.push(brg("brgFan", 8.0) + &brg("brgIp", 15.0) + &brg("brgHp", 22.0) + &brg("brgIpt", 15.0) + &brg("brgLp", 8.0));
    let bx = |k: &str, x: f64, y: f64, w: f64, h: f64| -> String {
        let c = hl(k);
        format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"4\" fill=\"{}\" fill-opacity=\"{}\" stroke=\"{}\" stroke-width=\"1.4\"{}/>",
            n(x),
            n(y),
            n(w),
            n(h),
            c.unwrap_or("rgba(255,255,255,0.05)"),
            if c.is_some() { "0.35" } else { "1" },
            c.unwrap_or("#9aa6b2"),
            glow(k)
        )
    };
    parts.push(format!("<path d=\"M360,{} L340,{}\" stroke=\"{}\" stroke-width=\"3\"/>", n(a + 40.0), n(a + 215.0), if hp_stopped { ENG_BAD } else { "#7f8a95" }));
    parts.push(bx("gearbox", 250.0, a + 205.0, 230.0, 26.0));
    parts.push(
        bx("generator", 255.0, a + 180.0, 34.0, 24.0)
            + &bx("starter", 292.0, a + 180.0, 28.0, 24.0)
            + &bx("oilPump", 323.0, a + 180.0, 26.0, 24.0)
            + &bx("fuelPump", 352.0, a + 180.0, 40.0, 24.0)
            + &bx("oilTank", 395.0, a + 176.0, 28.0, 28.0)
            + &bx("edp", 426.0, a + 180.0, 34.0, 24.0),
    );
    parts.push(bx("fmu", 430.0, a + 120.0, 30.0, 20.0));
    parts.push(format!(
        "<path d=\"M372,{} L372,{} L430,{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"/><path d=\"M460,{} L487,{} L487,{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"/>",
        n(a + 180.0),
        n(a + 130.0),
        n(a + 130.0),
        hl("fuelPump").or(hl("fmu")).unwrap_or(ENG_FUEL),
        n(a + 130.0),
        n(a + 130.0),
        n(a + 70.0),
        hl("fmu").or(hl("nozzles")).unwrap_or(ENG_FUEL)
    ));
    parts.push(format!(
        "<path d=\"M409,{} L409,{} L205,{} M409,{} L562,{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.6\" stroke-dasharray=\"5 4\"/>",
        n(a + 176.0),
        n(a + 30.0),
        n(a + 14.0),
        n(a + 30.0),
        n(a + 26.0),
        hl("oilPump").or(hl("oilTank")).unwrap_or(ENG_OIL)
    ));
    parts.push(bx("eec", 280.0, a - 214.0, 46.0, 22.0));
    parts.push(format!(
        "<path d=\"M330,{} L330,{} L470,{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2.4\"/><path d=\"M455,{} L455,{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2.4\"/>",
        n(a - 90.0),
        n(a - 200.0),
        n(a - 200.0),
        hl("bleedIp").unwrap_or(ENG_BLEED),
        n(a - 66.0),
        n(a - 200.0),
        hl("bleedHp").unwrap_or(ENG_BLEED)
    ));
    parts.push(bx("precooler", 470.0, a - 212.0, 60.0, 24.0));
    if f("reverser").is_some() {
        parts.push(format!(
            "<rect x=\"440\" y=\"{}\" width=\"70\" height=\"12\" fill=\"{ENG_WARN}\" opacity=\"0.6\"/><rect x=\"440\" y=\"{}\" width=\"70\" height=\"12\" fill=\"{ENG_WARN}\" opacity=\"0.6\"/>",
            n(a - 186.0),
            n(a + 174.0)
        ));
    }
    let probe = |k: &str, x: f64, r: f64| -> String {
        let c = hl(k);
        format!(
            "<path d=\"M{},{} l0,-14\" stroke=\"{}\" stroke-width=\"{}\"{}/><circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"{}/>",
            n(x),
            n(a - r),
            c.unwrap_or("#cfd6dd"),
            if c.is_some() { "4" } else { "2" },
            glow(k),
            n(x),
            n(a - r - 16.0),
            if c.is_some() { "5" } else { "3" },
            c.unwrap_or("#cfd6dd"),
            glow(k)
        )
    };
    parts.push(probe("t25", 352.0, 88.0) + &probe("p30", 470.0, 66.0) + &probe("tgt", 600.0, 98.0) + &probe("vibFan", 215.0, 158.0) + &probe("vibCore", 540.0, 94.0) + &probe("speed", 190.0, 152.0));
    let dim = |x: f64, y: f64, t: &str| ex_t(x, y, t, T { size: 10.0, fill: "rgba(230,230,230,0.55)", weight: 600.0, ..TD });
    parts.push(
        [(130.0, a + 70.0, "FAN"), (285.0, a + 37.0, "IPC"), (425.0, a + 37.0, "HPC"), (515.0, a + 37.0, "COMB"), (575.0, a + 37.0, "HPT"), (612.0, a + 37.0, "IPT"), (690.0, a + 37.0, "LPT"), (860.0, a + 37.0, "EXHAUST")]
            .iter()
            .map(|(x, y, t)| dim(*x, *y, t))
            .collect(),
    );
    parts.push(
        [(272.0, a + 196.0, "VFG"), (306.0, a + 196.0, "STRT"), (336.0, a + 196.0, "OIL"), (372.0, a + 196.0, "FUEL"), (409.0, a + 194.0, "TANK"), (443.0, a + 196.0, "EDP"), (445.0, a + 134.0, "FMU"), (365.0, a + 224.0, "GEARBOX")]
            .iter()
            .map(|(x, y, t)| dim(*x, *y, t))
            .collect(),
    );
    parts.push([(303.0, a - 199.0, "EEC"), (500.0, a - 196.0, "PRECOOLER"), (316.0, a - 150.0, "IP8"), (442.0, a - 150.0, "HP6"), (420.0, a - 168.0, "BYPASS")].iter().map(|(x, y, t)| dim(*x, *y, t)).collect());
    if mode_is("fanZone", "fire") {
        parts.push(format!("<path d=\"M260,{} q20,-30 40,0 q20,-26 40,0\" fill=\"{ENG_FLAME}\" class=\"engpulse\" filter=\"url(#engGlow)\"/>", n(a - 162.0)));
    }
    if mode_is("coreZone", "fire") {
        parts.push(format!("<path d=\"M600,{} q20,-30 40,0 q20,-26 40,0\" fill=\"{ENG_FLAME}\" class=\"engpulse\" filter=\"url(#engGlow)\"/>", n(a - 100.0)));
    }
    if mode_is("coreZone", "loop") {
        parts.push(format!("<path d=\"M300,{} L780,{}\" stroke=\"{ENG_WARN}\" stroke-width=\"3\" stroke-dasharray=\"8 6\"/>", n(a - 104.0), n(a - 104.0)));
    }
    let labels: Vec<&str> = faults.iter().map(|(k, _)| k.as_str()).filter(|k| part(k).is_some()).collect();
    for (i, k) in labels.iter().enumerate() {
        let (x, lbl) = part(k).unwrap();
        let c = hl(k).unwrap_or("null");
        parts.push(format!("<path d=\"M{},{} L{},{}\" stroke=\"{c}\" stroke-width=\"1\" stroke-dasharray=\"3 3\"/>", n(x), n(a + 236.0), n(x), n(a + 206.0)));
        parts.push(ex_t((w - 70.0f64).min(70.0f64.max(x)), h - 4.0 - (i % 2) as f64 * 14.0, lbl, T { fill: c, size: 12.0, ..TD }));
    }
    if !o.title.is_empty() {
        parts.push(ex_t(24.0, 24.0, &o.title, T { anchor: "start", size: 15.0, ..TD }));
    }
    if !o.readout.is_empty() {
        parts.push(ex_t(w - 20.0, 24.0, &o.readout, T { anchor: "end", size: 13.0, fill: EX_DIM, ..TD }));
    }
    parts
}

pub fn engine_svg(o: &EngOpts) -> Node {
    ex_svg(1000.0, 490.0, &eng_view(o))
}

pub(crate) fn fail_engine_of(f: &FailIn) -> usize {
    if let Some(m) = re!(r"(?i)\bENG(?:INE)?\s*(\d)").captures(&f.name) {
        return m[1].parse().unwrap_or(0);
    }
    if (54010.0..54014.0).contains(&f.id) {
        return (f.id - 54009.0) as usize;
    }
    if (54000.0..54004.0).contains(&f.id) {
        return (f.id - 53999.0) as usize;
    }
    if (f.id / 1000.0).floor() == 72.0 {
        return (f.id % 4.0) as usize + 1;
    }
    0
}

pub(crate) fn fail_is_engine(ata: i64, f: &FailIn) -> bool {
    if ata == 24 || ata == 29 {
        return false;
    }
    if (70..=80).contains(&ata) || ata == 54 {
        return fail_engine_of(f) > 0;
    }
    (ata == 26 || ata == 36) && fail_engine_of(f) > 0
}

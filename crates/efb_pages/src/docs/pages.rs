use super::{doc_lines, doc_title, el, DocLine, DocsData, DocsState, Kind, Leaf, Proc};
use crate::model::{path, EfbModel};
use crate::{button, label, span, Node};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn more(data: &DocsData, m: &EfbModel, st: &DocsState, leaf: &Leaf) -> Vec<Node> {
    let mut body = Vec::new();
    match &leaf.kind {
        Kind::Proc { sensed, index } => {
            let p = if *sensed { &data.sensed[*index] } else { &data.non_sensed[*index] };
            body.extend(proc_page(p, &leaf.id, st));
        }
        Kind::Limits => body.extend(limits_page(&data.perf)),
        Kind::Engine => {
            body.push(doc_title("ENGINE LIMITS (TRENT 900)", None, false));
            body.push(label("qd-rec", "EASA type certificate data sheet E.012"));
            body.extend(doc_table(&[("N1 TAKEOFF MAXIMUM", "97.2 %"), ("N2 (IP) TAKEOFF MAXIMUM", "98.7 %"), ("N3 (HP) TAKEOFF MAXIMUM", "97.8 %"), ("IP OVERSPEED ALLOWANCE", "99.5 % FOR 20 S")]));
            body.push(label("qd-sub", "GROUND OPERATION"));
            body.extend(doc_table(&[("KEEP OUT ZONE BELOW 60 KT", "N1 64 TO 72 %"), ("BELOW 32.5 KT UNTIL STABILISED", "N1 78 % MAX")]));
        }
        Kind::WvTable | Kind::Ratings => body.extend(ops_table(leaf.kind == Kind::WvTable, m)),
        Kind::Perf => {
            body.push(doc_title("IN FLIGHT PERFORMANCE", None, false));
            body.push(label("qd-p", "Takeoff speeds, flex temperature and landing distance are computed from your flight plan and weights on Flight > Performance."));
            body.push(button("qd-btn", "OPEN PERFORMANCE").attr("type", "button").act("tab:performance"));
        }
        Kind::List(inop) => {
            body.push(doc_title(&leaf.t, None, false));
            for line in if *inop { &data.inop } else { &data.limitations } {
                body.push(el("div", "qd-line note").child(span("qd-n", line)));
            }
        }
        Kind::Guide(i) => {
            body.push(doc_title(&leaf.t.to_uppercase(), None, false));
            for p in &data.guide[*i].1 {
                body.push(Node::el("p", "qd-p").text(p));
            }
        }
        Kind::Mine(_) | Kind::MineNew => {
            let (i, mine) = match leaf.kind {
                Kind::Mine(i) => (i, true),
                _ => (st.my_docs.len(), false),
            };
            let d = st.my_docs.get(i).cloned().unwrap_or_default();
            let title = if !mine {
                "NEW NOTE".to_string()
            } else if d.t.is_empty() {
                "UNTITLED".into()
            } else {
                d.t.to_uppercase()
            };
            body.push(doc_title(&title, None, false));
            body.push(Node::el("input", "st-text qd-in").attr("type", "text").attr("placeholder", "Title").value(&d.t).act(&format!("focus:note-title:{i}")));
            body.push(Node::el("textarea", "qd-note").attr("placeholder", "Notes, briefings, company procedures").value(&d.b).act(&format!("focus:note-body:{i}")));
            let mut tools = el("div", "qd-tools");
            if mine {
                tools = tools.child(button("qd-btn", "DELETE").attr("type", "button").act(&format!("doc:note-delete:{i}")));
            }
            body.push(tools.child(button("qd-btn", "SAVE").attr("type", "button").act(&format!("doc:note-save:{i}"))));
        }
        Kind::Ofp(part) => body.extend(ofp_page(*part, m.ofp.as_ref(), &leaf.id, st)),
        Kind::Md(p) => match st.md_text.get(p) {
            None => body.push(label("qd-empty", "Loading...")),
            Some(text) => body.push(crate::md::show(&crate::md::render(text, p), st.md_checks.get(&leaf.id).unwrap_or(&BTreeSet::new()))),
        },
        Kind::MdImg(p) => body.push(Node::el("img", "md-img md-full").attr("src", &format!("/api/docfile?path={}", crate::md::encode_uri_component(p))).attr("alt", &leaf.t)),
        Kind::MdPdf(p) => body.push(Node::el("iframe", "md-pdf").attr("src", &format!("/api/docfile?path={}", crate::md::encode_uri_component(p)))),
        Kind::SysMap(i) => {
            body.push(doc_title(&leaf.t, None, false));
            body.extend(crate::diagrams::system_map(*i, &leaf.t));
        }
        _ => body.push(doc_title(&leaf.t, None, false)),
    }
    body
}

const SD_NAMES: [(&str, &str); 15] = [
    ("Eng", "ENGINE"), ("Apu", "APU"), ("Bleed", "BLEED"), ("Cond", "COND"), ("Press", "CAB PRESS"), ("ElecAc", "ELEC AC"), ("ElecDc", "ELEC DC"), ("Fuel", "FUEL"),
    ("Wheel", "WHEEL"), ("Hyd", "HYD"), ("Fctl", "F/CTL"), ("Door", "DOOR"), ("Status", "STATUS"), ("Crz", "CRUISE"), ("Video", "VIDEO"),
];

fn shots() -> &'static BTreeMap<String, String> {
    static SHOTS: std::sync::OnceLock<BTreeMap<String, String>> = std::sync::OnceLock::new();
    SHOTS.get_or_init(|| serde_json::from_str(include_str!("../ewd_shots.json")).unwrap_or_default())
}

/// The EWD picture file (under `/ecam/`) for a procedure id.
pub fn ewd_shot(id: &str) -> Option<&'static str> {
    shots().get(id).map(String::as_str).filter(|s| !s.is_empty())
}

fn proc_actions(t: &str) -> Vec<DocLine> {
    let l = |a: &str, b: &str, f: &str| DocLine(a.into(), b.into(), 0, f.into());
    match t.to_uppercase().as_str() {
        "T.O SPEEDS NOT INSERTED" => vec![l("V1 / VR / V2 (FMS ACTIVE/PERF T.O)", "INSERT", ""), l("T.O CONFIG", "TEST", "")],
        "T.O SPEEDS TOO LOW" => vec![l("TOW AND T.O DATA", "CHECK", ""), l("V1 / VR / V2 (FMS ACTIVE/PERF T.O)", "RECOMPUTE / INSERT", ""), l("T.O CONFIG", "TEST", "")],
        "T.O V1/VR/V2 DISAGREE" => vec![l("V1 / VR / V2 (FMS ACTIVE/PERF T.O)", "CHECK", ""), l("V1 \u{2264} VR \u{2264} V2", "", "b")],
        _ => vec![],
    }
}

fn doc_card(id: &str, title: &str, items: &[String], st: &DocsState) -> Vec<Node> {
    let key = format!("{id}#card:{title}");
    let open = st.folds.contains(&key);
    let mut out = vec![el("button", "qd-card")
        .attr("type", "button")
        .class_if("open", open)
        .act(&format!("doc:fold:{key}"))
        .child(Node::new("span").text(title))
        .child(span("qd-chev", if open { "\u{25b4}" } else { "\u{25be}" }))];
    if open {
        out.push(el("div", "qd-cardbody").children(items.iter().map(|t| label("qd-inop", t))));
    }
    out
}

pub fn phase_diagram(inhibited: &[u32]) -> Node {
    use crate::jv::{n, Svg};
    let (nn, w, x0, bar, bh, top) = (12usize, 1000.0, 26.0, 300.0, 34.0, 96.0);
    let (lift, fl, fd, td) = (5usize, 7usize, 8usize, 9usize);
    let ww = (w - 2.0 * x0) / nn as f64;
    let labels = ["ELEC PWR", "1ST ENG STARTED", "2ND ENG TO PWR", "80 kt", "V1", "LIFTOFF", "400 ft", "1500 ft", "800 ft", "TOUCHDOWN", "80 kt", "2ND ENG SHUTDOWN", "5 MIN AFTER"];
    let bx = |i: usize| x0 + i as f64 * ww;
    let prof = |i: usize| -> f64 {
        if i <= lift || i >= td {
            bar
        } else if i >= fl && i <= fd {
            top
        } else if i < fl {
            bar - (bar - top) * (i - lift) as f64 / (fl - lift) as f64
        } else {
            bar - (bar - top) * (td - i) as f64 / (td - fd) as f64
        }
    };
    let s = |v: &str| v.to_string();
    let mut g = Svg::new("qd-phases", &[("viewBox", format!("0 0 {} {}", n(w), n(bar + bh + 14.0)))]);
    g.open("defs", &[]);
    g.open("linearGradient", &[("id", s("qdg")), ("gradientUnits", s("userSpaceOnUse")), ("x1", n(x0)), ("x2", n(bx(lift))), ("y1", s("0")), ("y2", s("0"))]);
    g.add("stop", &[("offset", s("0")), ("stop-color", s("#ff8a1e"))]);
    g.add("stop", &[("offset", s("1")), ("stop-color", s("#ff2f8e"))]);
    g.close().close();
    for i in 0..nn {
        let inh = inhibited.contains(&(i as u32 + 1));
        g.add("rect", &[("x", n(bx(i))), ("y", n(bar)), ("width", n(ww)), ("height", n(bh)), ("fill", s(if inh { "#ff1fe6" } else { "#2a2c30" }))]);
        g.text(
            &[("x", n(bx(i) + ww / 2.0)), ("y", n(bar + 24.0)), ("text-anchor", s("middle")), ("fill", s("#f4f4f4")), ("font-size", s("18")), ("font-weight", s("600")), ("font-family", s("Manrope, Segoe UI, sans-serif"))],
            &(i + 1).to_string(),
        );
        if i > 0 {
            g.add("line", &[("x1", n(bx(i))), ("y1", n(bar + 7.0)), ("x2", n(bx(i))), ("y2", n(bar + bh - 7.0)), ("stroke", s("#f4f4f4")), ("stroke-width", s("2"))]);
        }
    }
    g.add("rect", &[("x", n(x0)), ("y", n(bar)), ("width", n(w - 2.0 * x0)), ("height", n(bh)), ("fill", s("none")), ("stroke", s("#f4f4f4")), ("stroke-width", s("2.5"))]);
    for i in [fl, fd] {
        g.add("line", &[("x1", n(bx(i))), ("y1", n(top + 4.0)), ("x2", n(bx(i))), ("y2", n(bar - 2.0)), ("stroke", s("#f4f4f4")), ("stroke-width", s("1.6")), ("stroke-dasharray", s("7 6"))]);
    }
    let pts: Vec<String> = (0..=nn).map(|i| format!("{},{}", n(bx(i)), n(prof(i)))).collect();
    g.add("polyline", &[("points", pts.join(" ")), ("fill", s("none")), ("stroke", s("url(#qdg)")), ("stroke-width", s("3")), ("stroke-linejoin", s("round"))]);
    g.add("line", &[("x1", n(bx(td))), ("y1", n(bar)), ("x2", n(bx(nn))), ("y2", n(bar)), ("stroke", s("#ff2f8e")), ("stroke-width", s("3"))]);
    for (i, t) in labels.iter().enumerate() {
        let y = prof(i.min(nn)).min(bar) - 10.0;
        let x = if i == 0 {
            bx(0) + 18.0
        } else if i == nn {
            bx(nn) - 4.0
        } else {
            bx(i) + 6.0
        };
        g.text(
            &[
                ("transform", format!("translate({},{}) rotate(-90)", n(x), n(y))),
                ("fill", s("#f2f2f2")),
                ("font-size", s("18")),
                ("font-weight", s("800")),
                ("letter-spacing", s("0.3")),
                ("font-family", s("JetBrains Mono, Cascadia Mono, monospace")),
            ],
            t,
        );
    }
    el("div", "qd-svg qd-phasebox").child(g.done())
}

fn proc_page(p: &Proc, id: &str, st: &DocsState) -> Vec<Node> {
    let mut body = Vec::new();
    let band = p.c.is_none() && p.l.as_deref() != Some("white");
    body.push(doc_title(&p.t, p.l.as_deref(), band));
    if let Some(r) = p.r.as_deref().filter(|r| !r.is_empty()) {
        body.push(label("qd-rec", r));
    }
    let f = p.f.filter(|f| *f != 0.0 && !f.is_nan());
    if f.is_some() || p.ph.is_some() {
        body.push(label("qd-h2", "ANNUNCIATIONS"));
        if let Some(f) = f {
            body.push(label("qd-h3", "Indications:"));
            let sd = match p.sd.as_deref().filter(|s| !s.is_empty()) {
                Some(sd) => SD_NAMES.iter().find(|(k, _)| *k == sd).map(|x| x.1.to_string()).unwrap_or_else(|| sd.to_uppercase()),
                None => "NIL".into(),
            };
            let pick = |a: &str, b: &str| {
                if f >= 3.0 {
                    a.to_string()
                } else if f == 2.0 {
                    b.to_string()
                } else {
                    "NIL".to_string()
                }
            };
            let cells = [("Audio", pick("CRC", "SC")), ("Master Light", pick("MASTER WARN", "MASTER CAUT")), ("SD Page", sd)];
            body.push(el("div", "qd-ind").children(cells.iter().flat_map(|(k, v)| [label("qd-indk", k), label("qd-indv", v)])));
        }
        if let Some(ph) = &p.ph {
            body.push(label("qd-h3", "Flight Phase Inhibition:"));
            body.push(phase_diagram(ph));
        }
    }
    let sysd = crate::diagrams::ecam_diagram(p);
    let has_sysd = sysd.is_some();
    if let Some(d) = sysd {
        body.push(label("qd-h2", "SYSTEM"));
        body.push(d);
    }
    let shot = ewd_shot(&p.id);
    if let Some(file) = shot.filter(|_| st.ewd_shots.contains(&p.id)) {
        body.push(el("div", "qd-ewdbox").child(label("qd-h2", "EWD")).child(Node::el("img", "qd-ewd").attr("src", &format!("/ecam/{file}")).attr("alt", "EWD")));
    }
    if f.is_some() || p.ph.is_some() || has_sysd || shot.is_some() {
        body.push(label("qd-h2", "PROCEDURES"));
    }
    let fallback;
    let lines: &[DocLine] = if !p.i.is_empty() {
        &p.i
    } else {
        fallback = proc_actions(&p.t);
        &fallback
    };
    if lines.is_empty() {
        body.push(el("div", "qd-line note").child(span("qd-n", "CREW AWARENESS")));
    }
    body.extend(doc_lines(id, lines, st));
    let x = p.x.clone().unwrap_or(Value::Null);
    let list = |k: &str| -> Vec<String> { x.get(k).and_then(Value::as_array).map(|a| a.iter().map(crate::model::jstr).collect()).unwrap_or_default() };
    let mut lim = list("limitationsAllPhases");
    lim.extend(list("limitationsApprLdg"));
    lim.extend(list("limitationsPfd"));
    let cards = [("LIMITATIONS", lim), ("INOP SYS", list("inopSysAllPhases")), ("INOP SYS APPR & LDG", list("inopSysApprLdg")), ("REDUND LOSS", list("redundLoss")), ("INFO", list("info"))];
    let shown: Vec<&(&str, Vec<String>)> = cards.iter().filter(|(_, v)| !v.is_empty()).collect();
    if !shown.is_empty() {
        body.push(el("div", "qd-cards"));
    }
    for (t, v) in shown {
        body.extend(doc_card(id, t, v, st));
    }
    body
}

fn doc_table(rows: &[(&str, &str)]) -> Vec<Node> {
    rows.iter().map(|(k, v)| el("div", "qd-line note").child(span("qd-n", k)).child(el("span", "qd-dots")).child(span("qd-v", v))).collect()
}

fn limits_page(p: &Value) -> Vec<Node> {
    let g = |k: &str| p.get(k).filter(|v| !v.is_null());
    let sv = |k: &str| match p.get(k) {
        Some(v) => crate::jv::jstring(v),
        None => "undefined".into(),
    };
    let kt = |k: &str| g(k).map(|v| format!("{} KT", crate::jv::jstring(v))).unwrap_or_default();
    let t = |k: &str| g(k).map(|_| format!("{} T", crate::js::num(crate::js::round(crate::jv::nf(p, k) / 1000.0)))).unwrap_or_default();
    let tbl = |rows: &[(&str, String)]| doc_table(&rows.iter().map(|(a, b)| (*a, b.as_str())).collect::<Vec<_>>());
    let mut b = vec![doc_title("LIMITATIONS", None, false), label("qd-sub", "SPEEDS")];
    b.extend(tbl(&[
        ("VMO / MMO", format!("{} / M {}", kt("Vmo"), sv("Mmo"))),
        ("VFE CONF 1", kt("VfeF1")),
        ("VFE CONF 1+F", kt("VfeF1F")),
        ("VFE CONF 2", kt("VfeF2")),
        ("VFE CONF 3", kt("VfeF3")),
        ("VFE FULL", kt("VfeFF")),
        ("VLO / VLE", format!("{} / {}", kt("Vlo"), kt("Vle"))),
        ("MLO / MLE", format!("M {} / M {}", sv("Mlo"), sv("Mle"))),
        ("GRAVITY GEAR EXTENSION", format!("{} / M {}", kt("VloGravityGearExtension"), sv("MloGravityGearExtension"))),
        ("VMCL / VMCL-2", format!("{} / {}", kt("Vmcl"), kt("Vmcl2"))),
    ]));
    b.push(label("qd-sub", "WEIGHTS AND CG"));
    b.extend(tbl(&[
        ("MAX ZERO FUEL WEIGHT", t("maxZfw")),
        ("MAX GROSS WEIGHT (FMS)", t("maxGw")),
        ("ZFW CG RANGE", format!("{} TO {} % MAC", sv("minZfwCg"), sv("maxZfwCg"))),
        ("MAX BLOCK FUEL", t("maxBlockFuel")),
        ("MAX TAXI FUEL", t("maxTaxiFuel")),
    ]));
    b.push(label("qd-sub", "ALTITUDE"));
    b.extend(tbl(&[("MAX CERTIFIED ALTITUDE", format!("{} FT", sv("maxCertifiedAlt"))), ("DEFAULT TROPOPAUSE", format!("{} FT", sv("defaultTropopauseAlt")))]));
    b
}

const DOC_WV: [(&str, u32, u32, u32, u32); 15] = [
    ("WV000", 562, 560, 386, 361), ("WV001", 512, 510, 394, 372), ("WV002", 571, 569, 391, 366), ("WV003", 512, 510, 395, 373), ("WV004", 562, 560, 391, 366),
    ("WV005", 562, 560, 386, 366), ("WV006", 575, 573, 393, 368), ("WV007", 492, 490, 395, 373), ("WV008", 577, 575, 394, 369), ("WV009", 512, 510, 386, 361),
    ("WV010", 482, 480, 386, 361), ("WV011", 577, 575, 395, 369), ("WV012", 571, 569, 395, 366), ("WV013", 494, 492, 386, 361), ("WV014", 574, 572, 391, 366),
];
const DOC_RATINGS: [(&str, &str, f64); 11] = [
    ("970", "Trent 970-84", 334.29), ("972", "Trent 972-84", 341.41), ("972E", "Trent 972E-84", 341.41), ("970B", "Trent 970B-84", 348.31), ("972B", "Trent 972B-84", 356.81),
    ("977", "Trent 977-84", 359.33), ("977B", "Trent 977B-84", 372.92), ("980", "Trent 980-84", 374.09), ("GP7270", "Engine Alliance GP7270", 332.44),
    ("GP7270E", "Engine Alliance GP7270E (flat rated to 36 \u{b0}C)", 332.44), ("GP7277", "Engine Alliance GP7277", 357.15),
];

fn ops_table(wv: bool, m: &EfbModel) -> Vec<Node> {
    let mut b = vec![doc_title(if wv { "WEIGHT VARIANTS" } else { "ENGINE RATINGS" }, None, false)];
    let setting = |k: &str, d: &str| m.settings.get(k).filter(|v| !v.is_empty()).cloned().unwrap_or_else(|| d.to_string());
    let cur = if wv { setting("xphfbw.weightVariant", "WV003") } else { setting("xphfbw.engineRating", "972B") };
    if !wv {
        b.push(label("qd-p", "Trent 900: EASA TCDS E.012. GP7200: EASA TCDS IM.E.026 (EGT limit 1002 \u{b0}C take-off, 970 \u{b0}C max continuous; N1 100% = 2467 rpm, N2 100% = 10998 rpm)."));
    }
    let head: Vec<&str> = if wv { vec!["VARIANT", "MRW", "MTOW", "MLW", "MZFW"] } else { vec!["RATING", "ENGINE", "TAKEOFF THRUST"] };
    let rows: Vec<Vec<String>> = if wv {
        DOC_WV.iter().map(|(n, a, b2, c, d)| vec![n.to_string(), format!("{a} t"), format!("{b2} t"), format!("{c} t"), format!("{d} t")]).collect()
    } else {
        DOC_RATINGS.iter().map(|(n, e, k)| vec![n.to_string(), e.to_string(), format!("{} kN  ({} lbf)", crate::js::js_fixed(*k, 2), crate::js::locale_int(k * 224.809))]).collect()
    };
    let mut t = el("table", "md-table qd-table").child(Node::new("tr").children(head.iter().map(|h| Node::new("th").text(h))));
    for r in rows {
        let is = r[0] == cur;
        t = t.child(Node::el("tr", if is { "qd-cur" } else { "" }).children(r.iter().enumerate().map(|(i, c)| Node::new("td").text(&if i == 0 && is { format!("{c}  (SELECTED)") } else { c.clone() }))));
    }
    b.push(el("div", "md").child(t));
    b.push(label(
        "qd-p",
        if wv {
            "Select the variant in Settings > Aircraft. The FMS, loadsheet, performance calculators and overweight checks use the selected limits."
        } else {
            "Select the rating in Settings > Aircraft. Takeoff, flex and go-around thrust limits scale to the selected rating."
        },
    ));
    b
}

fn arr_or_one(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(a) => a.iter().collect(),
        Value::Null => vec![],
        v => vec![v],
    }
}

/// `ofpMarkdown`: the OFP part as the Markdown the Documents viewer renders.
pub fn ofp_markdown(part: &str, o: &Value) -> String {
    let code = |k: &str| path(o, &[k, "icao_code"]).as_str().filter(|s| !s.is_empty()).map(str::to_string);
    let (org, dst, alt) = (code("origin"), code("destination"), code("alternate"));
    match part {
        "plan" => {
            let html = path(o, &["text", "plan_html"]).as_str().unwrap_or("");
            crate::md::ofp_plan_markdown(&crate::md::html_text(html), org.as_deref().unwrap_or(""), dst.as_deref().unwrap_or(""))
        }
        "load" => {
            let u = path(o, &["params", "units"]).as_str().filter(|s| !s.is_empty()).unwrap_or("kgs").to_uppercase();
            let rows = |obj: &Value, list: &[(&str, &str)]| -> String {
                let mut out = vec!["| Item | Value |".to_string(), "|---|---|".to_string()];
                for (k, key) in list {
                    let v = obj.get(*key).unwrap_or(&Value::Null);
                    if v.is_null() || v.as_str() == Some("") {
                        continue;
                    }
                    let unit = if *k == "Pax" || *k == "Bags" { "" } else { u.as_str() };
                    out.push(format!("| {k} | {} {unit} |", crate::jv::jstring(v)));
                }
                out.join("\n")
            };
            let wr = rows(
                path(o, &["weights"]),
                &[("Pax", "pax_count"), ("Bags", "bag_count"), ("Payload", "payload"), ("Cargo", "cargo"), ("Est ZFW", "est_zfw"), ("Max ZFW", "max_zfw"), ("Est TOW", "est_tow"), ("Max TOW", "max_tow"), ("Est LDW", "est_ldw"), ("Max LDW", "max_ldw")],
            );
            let fr = rows(
                path(o, &["fuel"]),
                &[("Taxi", "taxi"), ("Trip", "enroute_burn"), ("Contingency", "contingency"), ("Alternate", "alternate_burn"), ("Final reserve", "reserve"), ("Extra", "extra"), ("Block", "plan_ramp"), ("Takeoff", "plan_takeoff"), ("Landing", "plan_landing")],
            );
            format!("# LOADSHEET {} - {}\n## Weights\n{wr}\n## Fuel\n{fr}", org.as_deref().unwrap_or(""), dst.as_deref().unwrap_or(""))
        }
        "wx" => {
            let x = path(o, &["weather"]);
            let sec = |name: String, k: &str| -> Option<String> {
                let v = x.get(k).filter(|v| crate::jv::truthy(v))?;
                let lines: Vec<String> = arr_or_one(v).into_iter().map(crate::jv::jstring).collect();
                Some(format!("## {name}\n```\n{}\n```", lines.join("\n")))
            };
            let o1 = org.clone().unwrap_or("ORIGIN".into());
            let d1 = dst.clone().unwrap_or("DESTINATION".into());
            let a1 = alt.clone().unwrap_or("ALTERNATE".into());
            let mut out = vec!["# WEATHER".to_string()];
            out.extend(
                [sec(format!("{o1} METAR"), "orig_metar"), sec(format!("{o1} TAF"), "orig_taf"), sec(format!("{d1} METAR"), "dest_metar"), sec(format!("{d1} TAF"), "dest_taf"), sec(format!("{a1} METAR"), "altn_metar"), sec(format!("{a1} TAF"), "altn_taf")]
                    .into_iter()
                    .flatten(),
            );
            out.join("\n")
        }
        "notam" => {
            let mut by: Vec<(String, Vec<&Value>)> = Vec::new();
            for n in arr_or_one(path(o, &["notams", "notamdrec"])) {
                let k = ["icao_id", "location_icao"].iter().find_map(|k| n.get(*k).filter(|v| crate::jv::truthy(v)).map(crate::jv::jstring)).unwrap_or("OTHER".into());
                match by.iter_mut().find(|(x, _)| *x == k) {
                    Some((_, l)) => l.push(n),
                    None => by.push((k, vec![n])),
                }
            }
            let mut out = vec!["# NOTAMS".to_string()];
            for (k, ns) in by {
                let body: Vec<String> = ns
                    .iter()
                    .map(|n| {
                        let t = ["notam_raw", "notam_text"].iter().find_map(|k| n.get(*k).filter(|v| crate::jv::truthy(v)).map(crate::jv::jstring)).unwrap_or_default();
                        format!("```\n{t}\n```")
                    })
                    .collect();
                out.push(format!("## {k}\n{}", body.join("\n")));
            }
            out.join("\n")
        }
        _ => String::new(),
    }
}

fn ofp_page(part: Option<&str>, ofp: Option<&Value>, id: &str, st: &DocsState) -> Vec<Node> {
    let Some(part) = part else {
        return vec![doc_title("FLIGHT DOCUMENTS", None, false), label("qd-p", "Load a SimBrief flight plan on Flight > Flight Plan and the OFP, loadsheet, weather and NOTAMs appear here.")];
    };
    let o = ofp.unwrap_or(&Value::Null);
    if part == "pdf" {
        let src = format!("{}{}", crate::model::path_str(o, &["files", "directory"]), crate::model::path_str(o, &["files", "pdf", "link"]));
        return vec![Node::el("iframe", "md-pdf").attr("src", &src)];
    }
    let empty = BTreeSet::new();
    vec![crate::md::show(&crate::md::render(&ofp_markdown(part, o), ""), st.md_checks.get(id).unwrap_or(&empty))]
}

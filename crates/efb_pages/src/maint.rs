use crate::js::{el, js_fixed, num, round};
use crate::jv::{a, b, clamp01, entries, f, fo, fx, nf, locale_datetime, numeric_cmp, s, sv, to_exp, truthy};
use crate::study::preview_banner;
use crate::{button, label, span, Node};
use serde_json::Value;
use std::collections::BTreeMap;

fn tab_panel(id: &str, active: bool) -> Node {
    el("div", "tab-panel").class_if("active", active).id(&format!("tab-{id}"))
}

fn text_input(id: &str, placeholder: &str, value: &str) -> Node {
    Node::new("input").attr("type", "text").id(id).attr("placeholder", placeholder).value(value).act(&format!("focus:{id}"))
}

fn check_label(id: &str, checked: bool, text: &str) -> Node {
    el("label", "row").style("margin:0").child(Node::new("input").attr("type", "checkbox").id(id).attr_if("checked", checked).act(&format!("toggle:{id}"))).child(span("label", text))
}

fn small(cls: &str, text: &str, act: &str) -> Node {
    button(cls, text).attr("type", "button").act(act)
}

fn count_btn(active: bool, text: &str, count: &str, warn: bool, act: &str) -> Node {
    Node::el("button", if active { "active" } else { "" }).attr("type", "button").act(act).text(text).child(span("count", count).class_if("armed", warn))
}

pub fn fmt_num(v: Option<f64>) -> String {
    let Some(v) = v.filter(|v| v.is_finite()) else { return "-".into() };
    let x = v.abs();
    if x != 0.0 && !(0.01..1e5).contains(&x) {
        return to_exp(v, 2);
    }
    num(js_fixed(v, 3).parse::<f64>().unwrap_or(0.0))
}

const ACRONYMS: &str = "AC DC ESS APU IGV SCV ECB FCU HP IP LP HPC IPC LPC HPT IPT LPT FADEC EEC VFG IDG TRU BCRU ADIRU ADR IR PRIM SEC FCDC CPIOM AFDX VHF HF ATC TCAS WXR RA GPS ILS VOR DME ADF MMR TAWS EGPWS CIDS FAP PTU EHA EBHA EDP ACM PRV HPV OPV FAV BMC LGERS LGCIU BSCU WAI NAI OAT TAT SAT AOA CG FQMS FQDC FCMC MCDU KCCU CDS DU OIT ATSU RMP ECAM FWS SDAC BTV OANS ROW ROP LED PSU LH RH NLG MLG WLG BLG RAT GCU CVR DFDR SSFDR ULB ELT IFE EFB NSS ACARS SATCOM CPDLC FMS FMGC AP FD NWS MFD PFD ND EWD SD ISIS CRC TRV GLC BTC BPCU EMP ACMP LGCIS FMV HMU VSV VBV TCC PMA CSD PCU SFCC SFCS";

pub fn acronyms(text: &str) -> String {
    text.split(' ').map(|w| if ACRONYMS.split(' ').any(|x| x == w.to_uppercase()) { w.to_uppercase() } else { w.to_string() }).collect::<Vec<_>>().join(" ")
}

fn cap_first(t: &str) -> String {
    let mut c = t.chars();
    match c.next() {
        Some(x) => x.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

pub fn comp_system(c: &Value) -> String {
    if b(c, "chapter") {
        return sv(c, "chapter", "");
    }
    let comp = s(c, "component");
    let mut parts = comp.split('.');
    let head = parts.next().unwrap_or("");
    if head == "engine" {
        return format!("Engine {}", parts.next().unwrap_or("undefined"));
    }
    head.replace('_', " ")
}

fn comp_count(list: &[&Value]) -> usize {
    list.iter().map(|c| s(c, "component")).collect::<std::collections::HashSet<_>>().len()
}

pub fn comp_label(c: &Value) -> String {
    let parts: Vec<&str> = s(c, "component").split('.').collect();
    let tail = if parts[0] == "engine" { format!("{} / {}", parts.get(2..).map(|p| p.join(" ")).unwrap_or_default(), sv(c, "param", "undefined")) } else { parts[1..].join(" ") };
    cap_first(&acronyms(&tail.replace('_', " ")))
}

fn comp_instance(c: &Value) -> String {
    let parts: Vec<&str> = s(c, "component").split('.').collect();
    if parts[0] == "engine" {
        return format!("Engine {}", parts.get(1).copied().unwrap_or("undefined"));
    }
    if b(c, "componentName") {
        return sv(c, "componentName", "");
    }
    comp_system(c)
}

fn comp_limit(c: &Value) -> f64 {
    let (h, mx, mn) = (nf(c, "healthy"), nf(c, "max"), nf(c, "min"));
    if (mx - h).abs() >= (mn - h).abs() { mx } else { mn }
}

pub fn comp_degradation(c: &Value) -> f64 {
    let span = comp_limit(c) - nf(c, "healthy");
    if span == 0.0 {
        return 0.0;
    }
    clamp01((nf(c, "value") - nf(c, "healthy")) / span)
}

fn bar_colour(v: f64) -> &'static str {
    if v > 0.5 { "var(--bad)" } else if v > 0.1 { "var(--warn)" } else { "var(--ok)" }
}

fn comp_bar(v: f64) -> Node {
    el("div", "comp-bar").child(Node::new("div").style(&format!("width:{}%;background:{}", fx(v * 100.0, 1), bar_colour(v))))
}

fn cb_button(cls: &str, title: &str, act: &str) -> Node {
    Node::el("button", cls).attr("type", "button").attr("title", title).act(act).child(el("span", "cb-collar")).child(el("span", "cb-cap"))
}

fn comp_row(c: &Value, open: bool) -> Node {
    let deg = comp_degradation(c);
    let key = format!("{}|{}", s(c, "component"), sv(c, "param", ""));
    let unit = sv(c, "unit", "undefined");
    let rate = fo(c, "ratePerHour").filter(|r| *r != 0.0 && !r.is_nan());
    let st_cls = if deg > 1e-6 { format!("cb pulled {}", if deg >= 0.999 { "trip-magnetic" } else { "trip-thermal" }) } else { "cb".into() };
    let status = cb_button(&st_cls, if deg > 1e-6 { "Restore to healthy" } else { "Fail completely" }, &format!("comp:{}:{key}", if deg > 1e-6 { "restore" } else { "fail" }));
    let inst = comp_instance(c) + &rate.map(|r| format!(" / worsening {} {unit}/h", fmt_num(Some(r)))).unwrap_or_default();
    let name = Node::new("div").child(label("label", &comp_label(c))).child(label("label dim bus", &inst));
    let val = if unit == "fraction lost" { format!("{}%", fx(deg * 100.0, if deg > 0.0 && deg < 0.1 { 1 } else { 0 })) } else { fmt_num(fo(c, "value")) };
    let mut quick = el("span", "comp-quick");
    for (l, fr) in [("25%", 0.25), ("50%", 0.5), ("Fail", 1.0)] {
        quick = quick.child(small("small grey", l, &format!("comp:set:{key}:{fr}")).class_if("armed", (deg - fr).abs() < 1e-6));
    }
    let row = el("div", "comp-row")
        .child(status)
        .child(name)
        .child(span("fval", &val))
        .child(comp_bar(deg))
        .child(quick)
        .child(span("help cb-info", "i").attr("title", "Exact value and wear rate").act(&format!("comp:info:{key}")));
    let mut detail = el("div", "cb-detail");
    if !open {
        detail = detail.attr("hidden", "");
    }
    if b(c, "description") {
        detail = detail.child(Node::new("div").text(&sv(c, "description", "")));
    }
    let direct = fo(c, "direct");
    let t = format!(
        "Healthy {} / range {} to {} {unit} / now {}{}",
        fmt_num(fo(c, "healthy")),
        fmt_num(fo(c, "min")),
        fmt_num(fo(c, "max")),
        fmt_num(fo(c, "value")),
        direct.map(|d| format!(" / set {}", fmt_num(Some(d)))).unwrap_or_default()
    );
    detail = detail.child(Node::new("div").text(&t));
    let v = direct.or(fo(c, "healthy")).map(num).unwrap_or_else(|| sv(c, "healthy", "undefined"));
    let rv = num(rate.unwrap_or(0.0));
    detail = detail.child(
        el("div", "row")
            .style("gap:8px;margin-top:6px")
            .child(Node::new("span").text("Value"))
            .child(Node::new("input").attr("type", "number").attr("step", "any").attr("min", &sv(c, "min", "undefined")).attr("max", &sv(c, "max", "undefined")).style("width:110px").value(&v))
            .child(Node::new("span").text("per hour"))
            .child(Node::new("input").attr("type", "number").attr("step", "any").style("width:90px").value(&rv))
            .child(small("small", "Apply", &format!("comp:apply:{key}"))),
    );
    Node::new("div").child(row).child(detail)
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct ListView {
    pub section: String,
    pub filter: String,
    pub only: bool,
    pub preview: bool,
    pub open: Vec<String>,
    pub tz_offset_min: i64,
    pub failed: bool,
}

pub fn components_tab(list: Option<&[Value]>, v: &ListView, active: bool) -> Node {
    let mut nav = el("nav", "study-nav").id("comp-nav");
    let mut content = Node::new("div").id("comp-content");
    match list.filter(|l| !l.is_empty()) {
        None => content = content.child(label("box", "Could not load the component list.")),
        Some(list) => {
            let all: Vec<&Value> = list.iter().collect();
            let mut systems: Vec<String> = Vec::new();
            for c in list {
                let sy = comp_system(c);
                if !systems.contains(&sy) {
                    systems.push(sy);
                }
            }
            systems.sort_by(|x, y| numeric_cmp(x, y));
            let nav_btn = |key: &str, text: &str, items: &[&Value]| {
                let bad = items.iter().filter(|c| comp_degradation(c) > 1e-6).count();
                let cnt = if bad > 0 { format!("{bad}/{}", comp_count(items)) } else { comp_count(items).to_string() };
                count_btn(v.section == key, text, &cnt, bad > 0, &format!("comp:system:{key}"))
            };
            nav = nav.child(nav_btn("", "All components", &all)).child(label("study-nav-group", "Systems"));
            for sy in &systems {
                let items: Vec<&Value> = list.iter().filter(|c| comp_system(c) == *sy).collect();
                nav = nav.child(nav_btn(sy, sy, &items));
            }
            let q = v.filter.trim().to_lowercase();
            let shown: Vec<&Value> = list
                .iter()
                .filter(|c| {
                    (v.section.is_empty() || comp_system(c) == v.section)
                        && (!v.only || comp_degradation(c) > 1e-6)
                        && (q.is_empty()
                            || format!("{} {} {} {}", s(c, "component"), sv(c, "param", "undefined"), comp_label(c), if b(c, "description") { sv(c, "description", "") } else { String::new() })
                                .to_lowercase()
                                .contains(&q))
                })
                .collect();
            let bad = list.iter().filter(|c| comp_degradation(c) > 1e-6).count();
            let title = format!("{} / {} shown / {} parameters / {bad} degraded", if v.section.is_empty() { "All components" } else { &v.section }, comp_count(&shown), shown.len());
            let mut bx = el("div", "box");
            if shown.is_empty() {
                bx = bx.child(span("label dim", "No components match."));
            }
            for c in shown {
                let key = format!("{}|{}", s(c, "component"), sv(c, "param", ""));
                bx = bx.child(comp_row(c, v.open.contains(&key)));
            }
            content = content.child(label("section-title", &title)).child(bx);
        }
    }
    tab_panel("components", active).child(preview_banner()).child(
        el("div", "study-layout").child(nav).child(
            el("div", "study-main")
                .child(
                    el("div", "study-toolbar")
                        .child(text_input("comp-filter", "Search by name, system or cause\u{2026}", &v.filter))
                        .child(check_label("comp-degraded", v.only, "Degraded only"))
                        .child(small("small grey", "Restore all", "comp:restore-all").id("comp-restore-all"))
                        .child(small("small grey", "Refresh", "comp:refresh").id("comp-refresh")),
                )
                .child(content),
        ),
    )
}

fn wear_pct(fr: Option<f64>) -> String {
    let v = fr.filter(|x| !x.is_nan()).unwrap_or(0.0);
    format!("{}%", fx(v * 100.0, if v > 0.0 && v < 0.01 { 2 } else { 1 }))
}

fn wear_row(lbl: &str, value: &str, frac: Option<f64>) -> Node {
    let r = el("div", "wear-row").child(span("label dim", lbl)).child(span("fval", value));
    match frac {
        Some(fr) => r.child(el("div", "comp-bar").child(Node::new("div").style(&{
            let v = if fr.is_nan() { 0.0 } else { fr.max(0.0).min(1.0) };
            format!("width:{}%;background:{}", fx(v * 100.0, 1), bar_colour(v))
        }))),
        None => r,
    }
}

fn wear_note(text: &str, bad: bool) -> Node {
    label("label dim bus", text).style(if bad { "color:var(--bad)" } else { "" })
}

fn wear_card(title: &str, rows: Vec<Option<Node>>) -> Node {
    el("div", "box wear-card").child(label("label", title)).children(rows.into_iter().flatten())
}

fn wear_grid(cards: Vec<Node>) -> Node {
    el("div", "wear-engines").children(cards)
}

fn known(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite())
}

fn wear_name(name: &str) -> String {
    let mut n = name;
    let l = n.strip_prefix("L:").unwrap_or(n);
    for p in ["A32NX_", "A380X_", "DEEP_"] {
        if let Some(r) = l.strip_prefix(p) {
            n = r;
            break;
        }
    }
    let mut t = n.to_string();
    if let Some(p) = t.rfind(':') {
        if p + 1 < t.len() && t[p + 1..].bytes().all(|c| c.is_ascii_digit()) {
            t = format!("{} {}", &t[..p], &t[p + 1..]);
        }
    }
    cap_first(&acronyms(&t.replace('_', " ").to_lowercase()))
}

const WEAR_SYSTEMS: [(&str, &[&str]); 9] = [
    ("Fuel", &["FUEL"]),
    ("Electrical", &["ELEC", "_GEN", "BAT", "SSC", "TRU"]),
    ("Hydraulics", &["HYD"]),
    ("Pneumatics and air", &["PNEU", "BLEED", "PACK", "DUCT", "ENV_", "OUTFLOW"]),
    ("Flight controls", &["FCTL", "ACTUATOR", "SERVO", "AILERON", "ELEVATOR", "RUDDER", "SPOILER", "FLAP", "SLAT", "THS"]),
    ("Landing gear", &["GEAR", "BRAKE", "TYRE", "WHEEL", "STEER"]),
    ("Engines", &["ENG", "CREEP", "NACELLE", "THRUST", "REVERSER"]),
    ("APU", &["APU"]),
    ("Sensors and avionics", &["ADIRS", "ADR", "STATIC", "PITOT", "AOA", "GPS", "RADIO", "SENSOR", "PROBE", "ANTENNA"]),
];

pub fn wear_content(d: Option<&Value>) -> Vec<Node> {
    let Some(d) = d.filter(|d| truthy(crate::jv::g(d, "engines"))) else { return vec![label("box", "Could not load the wear values.")] };
    let title = |t: &str| label("section-title", t);
    let mut out = vec![title("Airframe"), wear_grid(vec![wear_card("Airframe", vec![Some(wear_row("Flight hours", &format!("{} h", fmt_num(fo(d, "airframeHours"))), None))])])];
    out.push(title("Engines"));
    out.push(wear_grid(
        a(d, "engines")
            .iter()
            .map(|e| {
                let ex: Vec<(String, String)> = entries(crate::jv::g(e, "exceedances"))
                    .into_iter()
                    .filter(|(_, n)| n.as_f64().is_some_and(|x| x > 0.0))
                    .map(|(k, n)| (k.replace('_', " "), crate::jv::jstring(n)))
                    .collect();
                let creep = fo(e, "creepLife");
                wear_card(
                    &format!("Engine {}", sv(e, "engine", "undefined")),
                    vec![
                        Some(wear_row("Hours", &fmt_num(fo(e, "hours")), None)),
                        Some(wear_row("Start cycles", &sv(e, "cycles", "undefined"), None)),
                        Some(wear_row("Creep life used", &wear_pct(creep), creep)),
                        creep.filter(|c| *c > 0.0).map(|_| {
                            wear_note(&format!("Over-temperature {} / above MCT {} / TOGA {}", wear_pct(fo(e, "creepFromOvertemp")), wear_pct(fo(e, "creepFromMct")), wear_pct(fo(e, "creepFromToga"))), false)
                        }),
                        Some(wear_row("Compressor efficiency loss", &wear_pct(fo(e, "compressorEfficiencyLoss")), fo(e, "compressorEfficiencyLoss"))),
                        Some(wear_row("Oil quantity", &format!("{}%", fmt_num(fo(e, "oilQuantityPct"))), Some(1.0 - fo(e, "oilQuantityPct").unwrap_or(100.0) / 100.0))),
                        b(e, "fodIngested").then(|| wear_note("Foreign object ingested", true)),
                        Some(wear_note(
                            &if ex.is_empty() { "No exceedances".to_string() } else { format!("Exceedances: {}", ex.iter().map(|(k, n)| format!("{k} \u{d7}{n}")).collect::<Vec<_>>().join(", ")) },
                            false,
                        )),
                    ],
                )
            })
            .collect(),
    ));
    let ap = crate::jv::g(d, "apu");
    let store = a(d, "components").iter().find(|c| s(c, "id") == "apu");
    out.push(title("APU"));
    out.push(wear_grid(vec![wear_card(
        "APU",
        vec![
            Some(wear_row("Operating hours", &known(fo(ap, "hours")).map(|h| fmt_num(Some(h))).unwrap_or("-".into()), None)),
            Some(wear_row("Compressor wear", &wear_pct(fo(ap, "compressorWear")), fo(ap, "compressorWear"))),
            Some(wear_row("Turbine wear", &wear_pct(fo(ap, "turbineWear")), fo(ap, "turbineWear"))),
            Some(wear_row("Starter duty heat", &wear_pct(fo(ap, "starterHeat")), fo(ap, "starterHeat"))),
            store.map(|c| wear_row("Hot-section degradation", &wear_pct(fo(c, "degradation")), fo(c, "degradation"))),
            store.map(|c| wear_note(&format!("{} hot hours / {} cycles", fmt_num(fo(c, "hotHours")), sv(c, "cycles", "undefined")), false)),
        ],
    )]));
    let new_tread = fo(d, "newTreadMm").filter(|x| *x != 0.0 && !x.is_nan()).unwrap_or(6.0);
    let gear = a(d, "gear");
    out.push(title("Landing gear"));
    out.push(wear_grid(
        gear.iter()
            .map(|gr| {
                let mut rows = vec![
                    Some(wear_row("Strut life used", &wear_pct(fo(gr, "strutLife")), fo(gr, "strutLife"))),
                    Some(wear_row("Strut gas charge", &known(fo(gr, "strutGas")).map(|x| wear_pct(Some(x))).unwrap_or("-".into()), known(fo(gr, "strutGas")).map(|x| 1.0 - x))),
                    Some(wear_row("Strut oil level", &known(fo(gr, "strutOil")).map(|x| wear_pct(Some(x))).unwrap_or("-".into()), known(fo(gr, "strutOil")).map(|x| 1.0 - x))),
                    b(gr, "collapsed").then(|| wear_note("Strut collapsed", true)),
                ];
                for w in a(gr, "wheels") {
                    let tread = known(fo(w, "treadMm"));
                    let t = match tread {
                        Some(t) => format!("{} mm{}", fmt_num(Some(t)), if f(w, "leaked") > 0.001 { format!(" / {} leaked", wear_pct(fo(w, "leaked"))) } else { String::new() }),
                        None => "-".into(),
                    };
                    rows.push(Some(wear_row(&format!("Tyre {}", sv(w, "name", "undefined")), &t, tread.map(|t| 1.0 - t / new_tread))));
                }
                wear_card(s(gr, "name"), rows)
            })
            .collect(),
    ));
    let braked: Vec<&Value> = gear.iter().filter(|gr| a(gr, "wheels").iter().any(|w| known(fo(w, "brakeWear")).is_some())).collect();
    out.push(title("Brakes"));
    out.push(if braked.is_empty() {
        label("box label dim", "No brake values yet: load the aircraft.")
    } else {
        wear_grid(
            braked
                .iter()
                .map(|gr| {
                    wear_card(
                        &s(gr, "name").replacen(" gear", " brakes", 1),
                        a(gr, "wheels")
                            .iter()
                            .filter(|w| known(fo(w, "brakeWear")).is_some())
                            .map(|w| {
                                let bw = fo(w, "brakeWear");
                                let t = format!("{}{}", wear_pct(bw), known(fo(w, "brakeTempC")).map(|t| format!(" / {} \u{b0}C", num(round(t)))).unwrap_or_default());
                                Some(wear_row(&format!("Brake {}", sv(w, "name", "undefined")), &t, bw))
                            })
                            .collect(),
                    )
                })
                .collect(),
        )
    });
    let mut groups: Vec<(&str, Vec<&Value>)> = Vec::new();
    for lv in a(d, "live") {
        let nm = s(lv, "name");
        let sys = WEAR_SYSTEMS.iter().find(|(_, ks)| ks.iter().any(|k| nm.contains(k))).map(|x| x.0).unwrap_or("Other");
        match groups.iter_mut().find(|(k, _)| *k == sys) {
            Some((_, l)) => l.push(lv),
            None => groups.push((sys, vec![lv])),
        }
    }
    if !groups.is_empty() {
        out.push(title("Systems"));
        out.push(wear_grid(
            groups
                .iter()
                .map(|(sys, list)| {
                    wear_card(
                        sys,
                        list.iter()
                            .map(|lv| {
                                let nm = s(lv, "name");
                                let up = nm.to_uppercase();
                                let val = f(lv, "value");
                                let fr = (["FRACTION", "WEAR", "LIFE", "DEGRADATION"].iter().any(|k| up.contains(k)) && (0.0..=1.0).contains(&val)).then_some(val);
                                Some(wear_row(&wear_name(nm), &fr.map(|x| wear_pct(Some(x))).unwrap_or_else(|| fmt_num(fo(lv, "value"))), fr))
                            })
                            .collect(),
                    )
                })
                .collect(),
        ));
    }
    let mut st: Vec<&Value> = a(d, "components").iter().filter(|c| s(c, "id") != "apu").collect();
    st.sort_by(|x, y| f(y, "degradation").partial_cmp(&f(x, "degradation")).unwrap_or(std::cmp::Ordering::Equal));
    out.push(title("Component wear"));
    out.push(if st.is_empty() {
        label("box label dim", "No component wear recorded yet.")
    } else {
        wear_grid(
            st.iter()
                .map(|c| {
                    wear_card(
                        &sv(c, "id", "undefined"),
                        vec![
                            Some(wear_row("Degradation", &wear_pct(fo(c, "degradation")), fo(c, "degradation"))),
                            Some(wear_note(&format!("{} hot hours / {} cycles / thermal stress {}", fmt_num(fo(c, "hotHours")), sv(c, "cycles", "undefined"), fmt_num(fo(c, "thermalStress"))), false)),
                        ],
                    )
                })
                .collect(),
        )
    });
    let mut params: Vec<&Value> = a(d, "parameters").iter().collect();
    params.sort_by(|x, y| comp_degradation(y).partial_cmp(&comp_degradation(x)).unwrap_or(std::cmp::Ordering::Equal));
    out.push(title("Components away from healthy"));
    out.push(if params.is_empty() {
        label("box label dim", "Every component is at its healthy value.")
    } else {
        wear_grid(
            params
                .iter()
                .map(|c| {
                    let deg = comp_degradation(c);
                    let lbl = comp_label(c);
                    let inst = comp_instance(c);
                    let unit = sv(c, "unit", "undefined");
                    let rate = fo(c, "ratePerHour").filter(|r| *r != 0.0 && !r.is_nan());
                    wear_card(
                        if lbl.is_empty() { s(c, "component") } else { &lbl },
                        vec![
                            Some(wear_row(if inst.is_empty() { s(c, "param") } else { &inst }, &wear_pct(Some(deg)), Some(deg))),
                            Some(wear_note(
                                &format!("{} {unit} (healthy {}){}", fmt_num(fo(c, "value")), fmt_num(fo(c, "healthy")), rate.map(|r| format!(" / worsening {} {unit}/h", fmt_num(Some(r)))).unwrap_or_default()),
                                false,
                            )),
                        ],
                    )
                })
                .collect(),
        )
    });
    out
}

pub fn wear_tab(d: Option<&Value>, active: bool) -> Node {
    tab_panel("wear", active).child(preview_banner()).child(el("div", "study-main").id("wear-content").children(wear_content(d)))
}

fn fmt_hours(h: Option<f64>) -> String {
    h.map(|h| format!("{} h", fx(h, 1))).unwrap_or_default()
}

fn or_dash(v: &Value, k: &str) -> String {
    match v.get(k) {
        None | Some(Value::Null) => "-".into(),
        Some(x) => crate::jv::jstring(x),
    }
}

fn mel_item_box(item: &Value) -> Node {
    let mut bx = el("div", "box").child(label("label", &format!("{} / {}", sv(item, "id", "undefined"), sv(item, "title", "undefined"))));
    let cat = if b(item, "category") {
        format!("Category {}{}", sv(item, "category", ""), if b(item, "intervalDays") { format!(" ({} days)", sv(item, "intervalDays", "")) } else { " (interval as stated)".into() })
    } else {
        "Category unreadable in source".into()
    };
    bx = bx.child(label("label dim", &format!("{cat} / installed {} / required {} / placard {}", or_dash(item, "installed"), or_dash(item, "required"), or_dash(item, "placard"))));
    if b(item, "conditions") {
        bx = bx.child(label("label", &sv(item, "conditions", "")));
    }
    if b(item, "opsProcedure") {
        bx = bx.child(Node::new("details").child(Node::new("summary").text("Operational procedure (o)")).child(Node::new("pre").style("white-space:pre-wrap").text(&sv(item, "opsProcedure", ""))));
    }
    if !a(item, "amm").is_empty() {
        bx = bx.child(label("label dim", &format!("Maintenance procedure (m): AMM {}", a(item, "amm").iter().map(crate::jv::jstring).collect::<Vec<_>>().join(", "))));
    }
    bx
}

fn mel_chapter_name(c: &Value) -> String {
    if b(c, "name") && s(c, "name") != "Other" {
        sv(c, "name", "")
    } else {
        format!("{} ATA {}", sv(c, "ata", "undefined"), sv(c, "ata", "undefined"))
    }
}

fn mel_row(item: &Value, d: &Value, open: bool) -> Node {
    let cat = if b(item, "category") { format!("Category {}", sv(item, "category", "")) } else { "Category -".into() };
    let iv = if b(item, "intervalDays") {
        format!("{} days", sv(item, "intervalDays", ""))
    } else if s(item, "category") == "A" {
        "interval as stated".into()
    } else {
        "no interval".into()
    };
    let name = Node::new("div").child(label("label", &format!("{} / {}", sv(item, "id", "undefined"), sv(item, "title", "undefined")))).child(label("label dim bus", &format!("{cat} / {iv}")));
    let mut units = el("span", "mel-units");
    for u in a(item, "units") {
        let def = a(d, "deferrals").iter().find(|x| x.get("id") == u.get("failureId"));
        let here = def.is_some_and(|x| x.get("melRef") == item.get("id"));
        let title = if here { "INOP under this item: press when repaired".to_string() } else if let Some(x) = def { format!("Deferred under {}", sv(x, "melRef", "undefined")) } else { "Placard INOP".into() };
        units = units.child(small("small grey", &sv(u, "name", "undefined"), &format!("mel:unit:{}:{}", sv(item, "id", ""), sv(u, "failureId", ""))).class_if("armed", def.is_some()).attr("title", &title));
    }
    let row = el("div", "mel-row").child(name).child(units).child(span("help cb-info", "i").attr("title", "Conditions and procedures").act(&format!("mel:info:{}", sv(item, "id", ""))));
    let mut detail = el("div", "cb-detail").child(mel_item_box(item));
    if !open {
        detail = detail.attr("hidden", "");
    }
    Node::new("div").child(row).child(detail)
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct DefectSearch {
    pub results: Vec<Value>,
    pub selected: Option<String>,
    pub query: String,
}

fn draw_defects(d: &Value, search: &BTreeMap<String, DefectSearch>) -> Vec<Node> {
    let mut out = Vec::new();
    let disp = crate::jv::g(d, "dispatch");
    let legal = if disp.is_null() { true } else { b(disp, "legal") };
    let mut st = el("div", "box").child(label("section-title", if legal { "Dispatch: allowed" } else { "Dispatch: NOT allowed" }));
    for x in a(disp, "blocking") {
        st = st.child(label("label", &crate::jv::jstring(x)));
    }
    st = st.child(label("label dim", &format!("Airframe hours {}", fmt_hours(Some(fo(d, "airframeHours").unwrap_or(0.0))))));
    let mel = if b(d, "melLoaded") {
        "Operator MEL loaded.".to_string()
    } else {
        format!(
            "No operator MEL found. Run tools/parse_mel.py on your MEL and place the result at {}; deferrals meanwhile use a generic category.",
            if b(d, "melPath") { sv(d, "melPath", "") } else { "Output/preferences/fbw_a380x_mel.json".into() }
        )
    };
    st = st.child(label("label dim", &mel));
    out.push(st);
    let defects = a(d, "defects");
    out.push(label("section-title", &format!("Defects / {}", defects.len())));
    if defects.is_empty() {
        out.push(label("box label dim", "No active defects."));
    }
    for fl in defects {
        let id = sv(fl, "id", "");
        let mut bx = el("div", "box").child(label("label", &format!("{} / ATA {} / {}%", sv(fl, "name", "undefined"), sv(fl, "ata", "undefined"), num(round(fo(fl, "magnitude").unwrap_or(0.0) * 100.0)))));
        if b(fl, "cause") {
            bx = bx.child(label("label dim", &sv(fl, "cause", "")));
        }
        let mut buttons = el("div", "row");
        let def = crate::jv::g(fl, "deferral");
        if truthy(def) {
            let rem = f(def, "hoursRemaining");
            let mr = if b(def, "melRef") { sv(def, "melRef", "") } else { String::new() };
            let t = if rem > 0.0 {
                format!("Deferred{} / {} remaining", if mr.is_empty() { String::new() } else { format!(" under MEL {mr}") }, fmt_hours(Some(rem)))
            } else {
                format!("Deferral EXPIRED{}", if mr.is_empty() { String::new() } else { format!(" (MEL {mr})") })
            };
            bx = bx.child(label("label", &t));
            if truthy(crate::jv::g(def, "melItem")) {
                bx = bx.child(mel_item_box(crate::jv::g(def, "melItem")));
            }
        } else {
            let chosen = search.get(&id).cloned().unwrap_or_default();
            let options: Vec<&Value> = chosen.results.iter().chain(a(fl, "candidates")).collect();
            let mut opts: Vec<(String, String)> = Vec::new();
            if options.is_empty() {
                opts.push((String::new(), if b(fl, "genericCategory") { format!("Generic category {}", sv(fl, "genericCategory", "")) } else { "No MEL item available".into() }));
            }
            for o in &options {
                opts.push((sv(o, "id", "undefined"), format!("{} / {} / {}", sv(o, "id", "undefined"), sv(o, "title", "undefined"), if b(o, "category") { sv(o, "category", "") } else { "?".into() })));
            }
            let sel = chosen.selected.clone().filter(|x| !x.is_empty() && opts.iter().any(|(k, _)| k == x)).unwrap_or_else(|| opts.first().map(|o| o.0.clone()).unwrap_or_default());
            let o2: Vec<(&str, &str)> = opts.iter().map(|(k, l)| (k.as_str(), l.as_str())).collect();
            let preview = match options.iter().find(|o| sv(o, "id", "") == sel) {
                Some(item) => Node::new("div").child(mel_item_box(item)),
                None => Node::new("div").child(Node::new("span")),
            };
            bx = bx
                .child(Node::new("input").attr("type", "text").attr("placeholder", "Search the MEL (ATA, title, condition)\u{2026}").value(&chosen.query).act(&format!("focus:mel-q:{id}")))
                .child(crate::js::select(&o2, &format!("mel:pick:{id}"), &sel))
                .child(preview);
            buttons = buttons.child(small("small", "Defer", &format!("maint:defer:{id}")).attr_if("disabled", options.is_empty() && !b(fl, "genericCategory")));
        }
        buttons = buttons.child(small("small grey", "Repair", &format!("maint:repair:{id}")));
        out.push(bx.child(buttons));
    }
    out
}

pub fn maintenance_tab(d: Option<&Value>, mel_items: &BTreeMap<String, Vec<Value>>, search: &BTreeMap<String, DefectSearch>, v: &ListView, active: bool) -> Node {
    let mut nav = el("nav", "study-nav").id("maint-nav");
    let mut content = Node::new("div").id("maint-content");
    match d {
        None if v.failed => content = content.child(label("box", "Could not load maintenance data.")),
        None => {}
        Some(d) => {
            let len = |k: &str| a(d, k).len();
            let disp = crate::jv::g(d, "dispatch");
            let legal = if disp.is_null() { true } else { b(disp, "legal") };
            let sec = if v.section.is_empty() { "status" } else { v.section.as_str() };
            nav = nav
                .child(count_btn(sec == "status", if legal { "Dispatch & defects" } else { "Dispatch & defects (NO GO)" }, &len("defects").to_string(), !legal, "maint:section:status"))
                .child(count_btn(sec == "wear", "Component wear", &len("components").to_string(), false, "maint:section:wear"))
                .child(count_btn(sec == "log", "Technical log", &len("techLog").to_string(), false, "maint:section:log"));
            if len("melChapters") > 0 {
                nav = nav.child(label("study-nav-group", "Operator MEL"));
                for c in a(d, "melChapters") {
                    let key = format!("mel:{}", sv(c, "ata", "undefined"));
                    nav = nav.child(count_btn(sec == key, &mel_chapter_name(c), &sv(c, "count", "undefined"), false, &format!("maint:section:{key}")));
                }
            }
            let q = v.filter.trim().to_lowercase();
            match sec {
                "status" => content = content.children(draw_defects(d, search)),
                "wear" => {
                    content = content.child(label("section-title", &format!("Component wear / {}", len("components"))));
                    if len("components") == 0 {
                        content = content.child(label("box label dim", "No component wear recorded yet."));
                    }
                    for c in a(d, "components") {
                        content = content.child(
                            el("div", "box row")
                                .child(span(
                                    "label",
                                    &format!(
                                        "{} / {}% degraded / {} hot / {} cycles",
                                        sv(c, "id", "undefined"),
                                        num(round(fo(c, "degradation").unwrap_or(0.0) * 100.0)),
                                        fmt_hours(fo(c, "hotHours")),
                                        sv(c, "cycles", "undefined")
                                    ),
                                ))
                                .child(small("small grey", "Replace", &format!("maint:replace:{}", sv(c, "id", "")))),
                        );
                    }
                }
                "log" => {
                    let mut log = el("div", "box");
                    if len("techLog") == 0 {
                        log = log.child(label("label dim", "No entries."));
                    }
                    for e in a(d, "techLog") {
                        let when = fo(e, "unixTime").filter(|t| *t != 0.0).map(|t| locale_datetime(t, v.tz_offset_min)).unwrap_or_default();
                        log = log.child(label("label", &format!("{when} / {} / {}", fmt_hours(fo(e, "airframeHours")), sv(e, "text", "undefined"))));
                    }
                    content = content.child(label("section-title", "Technical log")).child(log);
                }
                _ => {
                    let (items, title): (&[Value], String) = if sec == "mel-search" {
                        let it = mel_items.get("search").map(Vec::as_slice).unwrap_or(&[]);
                        (it, format!("MEL search / {} found", it.len()))
                    } else {
                        let ata = &sec[4.min(sec.len())..];
                        let ch = a(d, "melChapters").iter().find(|c| s(c, "ata") == ata);
                        let title = ch.map(mel_chapter_name).unwrap_or_else(|| format!("ATA {ata}"));
                        match mel_items.get(ata) {
                            Some(it) => (it.as_slice(), title),
                            None => {
                                content = content.child(label("section-title", &format!("{title} / loading\u{2026}")));
                                return maint_panel(nav, content, v, active);
                            }
                        }
                    };
                    let shown: Vec<&Value> = items
                        .iter()
                        .filter(|it| {
                            q.is_empty()
                                || format!(
                                    "{} {} {} {}",
                                    sv(it, "id", "undefined"),
                                    sv(it, "title", "undefined"),
                                    if b(it, "conditions") { sv(it, "conditions", "") } else { String::new() },
                                    a(it, "units").iter().map(|u| sv(u, "name", "undefined")).collect::<Vec<_>>().join(" ")
                                )
                                .to_lowercase()
                                .contains(&q)
                        })
                        .collect();
                    let inop = shown.iter().filter(|it| a(d, "deferrals").iter().any(|x| x.get("melRef") == it.get("id"))).count();
                    let mut bx = el("div", "box");
                    if shown.is_empty() {
                        bx = bx.child(span("label dim", "No MEL items match."));
                    }
                    for it in shown.iter() {
                        bx = bx.child(mel_row(it, d, v.open.contains(&sv(it, "id", ""))));
                    }
                    content = content.child(label("section-title", &format!("{title} / {} items / {inop} INOP", shown.len()))).child(bx);
                }
            }
        }
    }
    maint_panel(nav, content, v, active)
}

fn maint_panel(nav: Node, content: Node, v: &ListView, active: bool) -> Node {
    tab_panel("maintenance", active).child(preview_banner()).child(
        el("div", "study-layout").child(nav).child(
            el("div", "study-main")
                .child(
                    el("div", "study-toolbar")
                        .child(text_input("maint-filter", "Filter this list, or search the whole MEL (Enter)\u{2026}", &v.filter))
                        .child(small("small grey", "Refresh", "maint:refresh").id("maint-refresh")),
                )
                .child(content),
        ),
    )
}

fn cb_state_label(c: &Value) -> &'static str {
    if b(c, "closed") {
        return "Closed";
    }
    match s(c, "trip") {
        "thermal" => "Tripped (thermal)",
        "magnetic" => "Tripped (magnetic)",
        _ => "Pulled",
    }
}

fn cb_state_dot(c: &Value) -> &'static str {
    if b(c, "closed") {
        "ok"
    } else if s(c, "trip") != "none" {
        "bad"
    } else {
        "warn"
    }
}

fn cb_class(c: &Value) -> String {
    let mut cls = "cb".to_string();
    if !b(c, "closed") {
        cls.push_str(" pulled");
    }
    match s(c, "trip") {
        "thermal" => cls.push_str(" trip-thermal"),
        "magnetic" => cls.push_str(" trip-magnetic"),
        _ => {}
    }
    cls
}

fn amps(v: Option<f64>, d: usize) -> String {
    v.map(|x| format!("{} A", fx(x, d))).unwrap_or("-".into())
}

fn cb_row(c: &Value, open: bool) -> Node {
    let key = format!("{}:{}", s(c, "source"), sv(c, "id", ""));
    let cb = cb_button(&cb_class(c), if b(c, "closed") { "Pull" } else { "Reset" }, &format!("cb:toggle:{key}"));
    let bus = if b(c, "bus") { format!("Bus: {}", sv(c, "bus", "").replace('_', " ")) } else { "Bus: -".into() };
    let name = Node::new("div").child(label("label", &sv(c, "name", "undefined"))).child(label("label dim bus", &bus));
    let state = el("span", "row").style("margin:0;gap:6px").child(Node::el("span", &format!("dot {}", cb_state_dot(c)))).child(span("label dim", cb_state_label(c)).style("text-transform:none"));
    let consumers = match c.get("consumers") {
        Some(Value::Array(l)) => l.iter().map(crate::jv::jstring).collect::<Vec<_>>().join(", "),
        _ => if b(c, "consumer") { sv(c, "consumer", "") } else { String::new() },
    };
    let detail_on = !consumers.is_empty() || b(c, "basis");
    let info = if detail_on { span("help cb-info", "i").attr("title", "Consumers and rating basis").act(&format!("cb:info:{key}")) } else { Node::new("span") };
    let row = el("div", "cb-row").child(cb).child(name).child(span("fval", &amps(fo(c, "ratingA"), 0))).child(span("fval", &amps(fo(c, "currentA"), 1))).child(state).child(info);
    let mut wrap = Node::new("div").child(row);
    if detail_on {
        let mut detail = el("div", "cb-detail");
        if !open {
            detail = detail.attr("hidden", "");
        }
        if !consumers.is_empty() {
            detail = detail.child(Node::new("div").text(&format!("Consumers: {consumers}")));
        }
        if b(c, "basis") {
            detail = detail.child(Node::new("div").text(&format!("Basis: {}", sv(c, "basis", ""))));
        }
        wrap = wrap.child(detail);
    }
    wrap
}

/// `breakersOnShow`'s list: every breaker but the `systems.cfg` circuits.
pub fn breaker_list(all: &[Value]) -> Vec<Value> {
    all.iter().filter(|c| s(c, "source") != "systemsCfg").cloned().collect()
}

pub fn breakers_tab(list: Option<&[Value]>, v: &ListView, active: bool) -> Node {
    let mut nav = el("nav", "study-nav").id("breakers-nav");
    let mut content = Node::new("div").id("breakers-content");
    match list {
        None if v.failed => content = content.child(label("box", "Could not load the circuit breaker list.")),
        None => {}
        Some(list) => {
            let mut chapters: Vec<String> = Vec::new();
            for c in list {
                let ch = sv(c, "chapter", "undefined");
                if !chapters.contains(&ch) {
                    chapters.push(ch);
                }
            }
            chapters.sort_by(|x, y| x.encode_utf16().cmp(y.encode_utf16()));
            let nav_btn = |key: &str, text: &str, items: &[&Value]| {
                let open = items.iter().filter(|c| !b(c, "closed")).count();
                let cnt = if open > 0 { format!("{open}/{}", items.len()) } else { items.len().to_string() };
                count_btn(v.section == key, text, &cnt, open > 0, &format!("cb:chapter:{key}"))
            };
            let all: Vec<&Value> = list.iter().collect();
            nav = nav.child(nav_btn("", "All breakers", &all)).child(label("study-nav-group", "Chapters"));
            for ch in &chapters {
                let items: Vec<&Value> = list.iter().filter(|c| sv(c, "chapter", "undefined") == *ch).collect();
                nav = nav.child(nav_btn(ch, ch, &items));
            }
            let q = v.filter.trim().to_lowercase();
            let shown: Vec<&Value> = list
                .iter()
                .filter(|c| {
                    (v.section.is_empty() || sv(c, "chapter", "undefined") == v.section)
                        && (!v.only || !b(c, "closed"))
                        && (q.is_empty() || format!("{} {} {}", sv(c, "id", "undefined"), sv(c, "name", "undefined"), if b(c, "bus") { sv(c, "bus", "") } else { String::new() }).to_lowercase().contains(&q))
                })
                .collect();
            let open = list.iter().filter(|c| !b(c, "closed")).count();
            let mut bx = el("div", "box");
            if shown.is_empty() {
                bx = bx.child(span("label dim", "No breakers match."));
            }
            for c in &shown {
                let key = format!("{}:{}", s(c, "source"), sv(c, "id", ""));
                bx = bx.child(cb_row(c, v.open.contains(&key)));
            }
            content = content
                .child(label("section-title", &format!("{} / {} shown / {open} pulled or tripped", if v.section.is_empty() { "All breakers" } else { &v.section }, shown.len())))
                .child(bx);
        }
    }
    tab_panel("breakers", active).child(preview_banner()).child(
        el("div", "study-layout").child(nav).child(
            el("div", "study-main")
                .child(
                    el("div", "study-toolbar")
                        .id("breakers-toolbar")
                        .child(text_input("breakers-search", "Search by ATA number, name or bus\u{2026}", &v.filter))
                        .child(check_label("breakers-pulled", v.only, "Pulled/tripped only"))
                        .child(small("small grey", "Reset all", "cb:reset-all").id("breakers-reset-all")),
                )
                .child(content),
        ),
    )
}

use crate::js::{el, locale_int, num};
use crate::jv::{a, s, sv};
use crate::{button, label, span, Node};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct AtcView {
    pub network: String,
    pub list: Option<Vec<Value>>,
    pub error: String,
    pub filter: String,
    pub search: String,
    pub selected: String,
    pub loaded_at_ms: f64,
    pub active: String,
    pub standby: String,
    pub busy: bool,
    pub fixture: bool,
    pub tz_offset_min: i64,
}

/// `atcBcd`: a COM frequency dataref (BCD in Hz) as `nnn.nnn`.
pub fn atc_bcd(v: f64) -> String {
    let n = crate::js::round(if v.is_nan() { 0.0 } else { v });
    if n == 0.0 {
        return String::new();
    }
    let d = format!("{:07x}", n as i64);
    format!("{}.{}", &d[..3], &d[3..6])
}

const ATC_TYPES: [(&str, &str); 7] = [("", "All"), ("ATIS", "ATIS"), ("DELIVERY", "Delivery"), ("GROUND", "Ground"), ("TOWER", "Tower"), ("APPROACH", "Approach"), ("RADAR", "Centre")];

fn tab(on: bool, text: &str, act: &str) -> Node {
    el("button", "pc-tab").class_if("on", on).attr("type", "button").act(act).text(text)
}

pub fn atc_content(v: &AtcView) -> Node {
    let net = if v.network == "IVAO" { "IVAO" } else { "VATSIM" };
    let mut host = Node::new("div").id("atc-content");
    let radio = |l: &str, f: &str| el("div", "atc-radio").child(span("label dim", l)).child(span("mono atc-f", if f.is_empty() { "---.---" } else { f }));
    let mut nets = el("div", "atc-nets");
    for n in ["VATSIM", "IVAO"] {
        nets = nets.child(tab(net == n, n, &format!("atc:net:{n}")));
    }
    host = host.child(
        el("div", "atc-head")
            .child(radio("COM 1 ACTIVE", &v.active))
            .child(radio("STANDBY", &v.standby))
            .child(nets)
            .child(button("small grey", if v.busy { "Loading\u{2026}" } else { "Refresh" }).attr("type", "button").act("atc:refresh")),
    );
    let mut filters = el("div", "atc-filters");
    for (k, l) in ATC_TYPES {
        filters = filters.child(tab(v.filter == k, l, &format!("atc:filter:{k}")));
    }
    filters = filters.child(Node::new("input").attr("type", "text").attr("placeholder", "Callsign").value(&v.search).act("focus:atc-search"));
    host = host.child(filters);
    if !v.error.is_empty() {
        host = host.child(label("label dim", &v.error));
    }
    let Some(list) = &v.list else {
        return host.child(label("label dim", &if v.fixture { "Online ATC needs X-Plane running.".to_string() } else { format!("Loading {net} controllers near the aircraft\u{2026}") }));
    };
    let q = v.search.trim().to_uppercase();
    let rows: Vec<&Value> = list.iter().filter(|st| (v.filter.is_empty() || s(st, "type") == v.filter) && (q.is_empty() || s(st, "callsign").contains(&q))).collect();
    let mut lst = el("div", "atc-list");
    for st in &rows {
        let f = s(st, "frequency");
        let cs = s(st, "callsign");
        let row = el("div", "atc-row")
            .class_if("tuned", f == v.active)
            .class_if("stby", f != v.active && f == v.standby)
            .class_if("sel", v.selected == cs)
            .act(&format!("atc:select:{cs}"))
            .child(el("div", "atc-cs").child(Node::new("span").text(cs)).child(span("label dim", &format!("{} \u{b7} {} nm", s(st, "type").to_lowercase(), sv(st, "distance", "undefined")))))
            .child(span("mono atc-f", f))
            .child(button("small", "Active").attr("type", "button").act(&format!("atc:tune:active:{f}")))
            .child(button("small grey", "Standby").attr("type", "button").act(&format!("atc:tune:standby:{f}")));
        lst = lst.child(row);
    }
    if rows.is_empty() {
        lst = lst.child(label("label dim", "No station in range matches."));
    }
    let sel = list.iter().find(|st| s(st, "callsign") == v.selected);
    let lines: Vec<String> = sel.map(|st| a(st, "atis").iter().map(crate::jv::jstring).collect()).unwrap_or_default();
    let text = if !lines.is_empty() {
        lines.join("\n")
    } else if sel.is_some() {
        "No information published.".into()
    } else {
        "Tap a station for its ATIS or controller information.".into()
    };
    let info = el("div", "atc-info").child(label("home-sec", sel.map(|st| s(st, "callsign")).unwrap_or("Station information"))).child(label("mono atc-atis", &text));
    host = host.child(el("div", "atc-body").child(lst).child(info));
    let upd = if v.loaded_at_ms != 0.0 { crate::jv::locale_time(v.loaded_at_ms / 1000.0, v.tz_offset_min) } else { "--".into() };
    host.child(label(
        "label dim",
        &format!(
            "{net} stations whose range covers the aircraft, nearest first{}. Updated {upd}.",
            if net == "IVAO" { " (IVAO publishes no range: typical ranges by position)" } else { "" }
        ),
    ))
}

struct Hold {
    key: &'static str,
    name: &'static str,
    max_kg: f64,
}

const HOLDS: [Hold; 3] = [Hold { key: "fwd", name: "FWD hold", max_kg: 28577.0 }, Hold { key: "aft", name: "AFT hold", max_kg: 20310.0 }, Hold { key: "bulk", name: "Bulk hold", max_kg: 2513.0 }];

fn hold(k: &str) -> &'static Hold {
    HOLDS.iter().find(|h| h.key == k).unwrap()
}

struct Item {
    id: &'static str,
    name: &'static str,
    unit: &'static str,
    code: &'static str,
    kg: f64,
    holds: &'static [&'static str],
    dg: &'static str,
    note: &'static str,
    notoc: bool,
    dry_ice: f64,
}

const FA: &[&str] = &["fwd", "aft"];

const ITEMS: [Item; 12] = [
    Item { id: "gen", name: "General cargo", unit: "PMC pallet", code: "GEN", kg: 3000.0, holds: FA, dg: "", note: "", notoc: false, dry_ice: 0.0 },
    Item { id: "xps", name: "Express parcels", unit: "LD3 container", code: "XPS", kg: 900.0, holds: FA, dg: "", note: "", notoc: false, dry_ice: 0.0 },
    Item { id: "mal", name: "Mail", unit: "LD3 container", code: "MAL", kg: 700.0, holds: FA, dg: "", note: "", notoc: false, dry_ice: 0.0 },
    Item {
        id: "pil",
        name: "Medical supplies, 2 to 8 \u{b0}C",
        unit: "active container (LD3 size)",
        code: "PIL ACT",
        kg: 1100.0,
        holds: FA,
        dg: "",
        note: "Temperature controlled; keep the hold heating on.",
        notoc: false,
        dry_ice: 0.0,
    },
    Item { id: "ice", name: "Vaccines on dry ice", unit: "insulated box, 200 kg dry ice", code: "PIL ICE", kg: 450.0, holds: FA, dg: "UN1845 Dry ice, class 9, 200 kg", note: "", notoc: false, dry_ice: 200.0 },
    Item { id: "per", name: "Fresh fish and flowers", unit: "LD3 container", code: "PER", kg: 1200.0, holds: FA, dg: "", note: "", notoc: false, dry_ice: 0.0 },
    Item {
        id: "avi_pet",
        name: "Dogs and cats in kennels",
        unit: "kennel",
        code: "AVI",
        kg: 60.0,
        holds: &["bulk"],
        dg: "",
        note: "Bulk hold only: heated and ventilated. Set BULK heating on.",
        notoc: true,
        dry_ice: 0.0,
    },
    Item { id: "veh_kit", name: "Formula 1 freight (spares, tyres, tools)", unit: "PMC pallet", code: "VEH", kg: 2500.0, holds: FA, dg: "", note: "", notoc: false, dry_ice: 0.0 },
    Item { id: "val", name: "Valuables (banknotes, gold)", unit: "secure LD3", code: "VAL", kg: 500.0, holds: &["fwd"], dg: "", note: "", notoc: true, dry_ice: 0.0 },
    Item { id: "hum", name: "Human remains", unit: "coffin on skid", code: "HUM", kg: 250.0, holds: FA, dg: "", note: "", notoc: true, dry_ice: 0.0 },
    Item { id: "eli", name: "Electronics with lithium batteries", unit: "LD3 container", code: "ELI", kg: 800.0, holds: FA, dg: "UN3481 Lithium ion batteries packed with equipment, class 9, PI 967", note: "", notoc: false, dry_ice: 0.0 },
    Item { id: "mag", name: "Magnetised material", unit: "crate", code: "MAG", kg: 300.0, holds: &["aft"], dg: "UN2807 Magnetized material, class 9", note: "", notoc: false, dry_ice: 0.0 },
];

pub struct CargoPlan {
    pub error: String,
    pub load: BTreeMap<&'static str, f64>,
    lines: Vec<(&'static str, usize)>,
    notoc: Vec<usize>,
    pub warn: String,
}

pub fn cargo_plan(qty: &BTreeMap<String, f64>) -> CargoPlan {
    let mut load: BTreeMap<&'static str, f64> = HOLDS.iter().map(|h| (h.key, 0.0)).collect();
    let mut lines = Vec::new();
    let mut notoc = Vec::new();
    let mut dry = 0.0;
    for (i, it) in ITEMS.iter().enumerate() {
        let q = qty.get(it.id).copied().unwrap_or(0.0);
        if q == 0.0 || q.is_nan() {
            continue;
        }
        let mut left = q;
        for h in it.holds {
            while left > 0.0 && load[h] + it.kg <= hold(h).max_kg {
                *load.get_mut(h).unwrap() += it.kg;
                left -= 1.0;
                lines.push((*h, i));
            }
        }
        if left > 0.0 {
            return CargoPlan {
                error: format!("{}: no room left in {}.", it.name, it.holds.iter().map(|h| hold(h).name).collect::<Vec<_>>().join(" or ")),
                load: BTreeMap::new(),
                lines: vec![],
                notoc: vec![],
                warn: String::new(),
            };
        }
        if !it.dg.is_empty() || it.notoc {
            for _ in 0..q as usize {
                notoc.push(i);
            }
        }
        dry += it.dry_ice * q;
    }
    let warn = if dry > 1000.0 { format!("Dry ice {} kg: above the 1000 kg operator limit for the lower deck.", num(dry)) } else { String::new() };
    CargoPlan { error: String::new(), load, lines, notoc, warn }
}

/// The NOTOC text; `issued` is `YYYY-MM-DD HH:MM` UTC.
pub fn cargo_notoc(plan: &CargoPlan, issued: &str) -> String {
    let mut head = vec!["NOTOC - NOTIFICATION TO CAPTAIN".to_string(), format!("ISSUED {issued}Z"), String::new()];
    if plan.notoc.is_empty() {
        head.push("NO DANGEROUS GOODS OR SPECIAL LOAD ON BOARD".into());
        return head.join("\n");
    }
    head.push("CODE     POSITION   WEIGHT   DESCRIPTION".into());
    for i in &plan.notoc {
        let it = &ITEMS[*i];
        let h = plan.lines.iter().find(|(_, x)| x == i).map(|l| l.0).unwrap_or(it.holds[0]);
        head.push(format!("{:<8} {:<10} {:>5} KG  {}", it.code, hold(h).name, num(it.kg), if it.dg.is_empty() { it.name.to_uppercase() } else { it.dg.to_string() }));
    }
    head.join("\n")
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct CargoView {
    pub qty: BTreeMap<String, f64>,
    pub msg: String,
    pub issued: String,
}

pub fn cargo_content(v: &CargoView) -> Node {
    let plan = cargo_plan(&v.qty);
    let mut list = el("div", "cg-list").child(label("section-title", "Special loads"));
    for it in &ITEMS {
        let q = v.qty.get(it.id).copied().unwrap_or(0.0);
        let unit = {
            let mut c = it.unit.chars();
            c.next().map(|x| x.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        };
        let meta = format!(
            "{unit}, {} kg each, {}{}{}",
            num(it.kg),
            it.holds.iter().map(|h| hold(h).name).collect::<Vec<_>>().join(" / "),
            if it.dg.is_empty() { String::new() } else { format!(" \u{b7} {}", it.dg) },
            if it.note.is_empty() { String::new() } else { format!(" \u{b7} {}", it.note) }
        );
        list = list.child(
            el("div", "cg-row")
                .child(el("div", "cg-main").child(el("div", "cg-name").child(span("cg-code", it.code)).child(Node::new("span").text(it.name))).child(label("cg-meta", &meta)))
                .child(
                    el("div", "cg-qty")
                        .child(button("cg-step", "\u{2212}").attr("type", "button").act(&format!("cargo:minus:{}", it.id)))
                        .child(span("cg-n", &num(q)))
                        .child(button("cg-step", "+").attr("type", "button").act(&format!("cargo:plus:{}", it.id))),
                ),
        );
    }
    let holds = el("div", "cg-holds").children(HOLDS.iter().map(|h| {
        let kg = if plan.error.is_empty() { plan.load[h.key] } else { 0.0 };
        el("div", "cg-hold").child(label("cg-hn", h.name)).child(label("cg-hv", &format!("{} kg", locale_int(kg)))).child(label("cg-hm", &format!("of {} kg", locale_int(h.max_kg))))
    }));
    let notoc = if plan.error.is_empty() { cargo_notoc(&plan, &v.issued) } else { String::new() };
    let msg = [v.msg.as_str(), plan.error.as_str(), plan.warn.as_str()].into_iter().find(|x| !x.is_empty()).unwrap_or("");
    let side = el("div", "cg-side")
        .child(label("section-title", "Holds"))
        .child(holds)
        .child(
            el("div", "cg-actions")
                .child(button("", "Import from SimBrief").attr("type", "button").act("cargo:simbrief"))
                .child(button("primary", "Add to hold targets").attr("type", "button").act("cargo:apply"))
                .child(button("", "Remove special cargo").attr("type", "button").act("cargo:remove")),
        )
        .child(if msg.is_empty() { Node::new("div") } else { label("cg-msg", msg) })
        .child(label("section-title", "NOTOC"))
        .child(Node::el("pre", "cg-notoc").text(&notoc))
        .child(button("", "Save NOTOC to Documents").attr("type", "button").act("cargo:notoc"));
    Node::new("div").id("cargo-content").child(el("div", "cg-wrap").child(list).child(side))
}

use crate::js::{el, num, round};
use crate::jv::{fx, Vars};
use crate::settings::SettingsTable;
use crate::{label, span, EfbModel, Node};
use serde::Deserialize;
use serde_json::Value;

pub const FAP_PAGES: [(&str, &str); 9] = [
    ("status", "CABIN STATUS"), ("audio", "AUDIO"), ("lights", "LIGHTS"), ("doors", "DOORS / SLIDES"), ("temp", "TEMP"), ("water", "WATER / WASTE"), ("smoke", "SMOKE DETECT"), ("seats", "SEAT SETTING"), ("sysinfo", "SYSTEM INFO"),
];
const DOORS: [&str; 18] = ["M1L", "M1R", "M2L", "M2R", "M3L", "M3R", "M4L", "M4R", "M5L", "M5R", "U1L", "U1R", "U2L", "U2R", "U3L", "U3R", "CARGO FWD", "CARGO AFT"];
const ZONES: [&str; 3] = ["FWD", "MID", "AFT"];

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct FapView {
    pub page: String,
    pub ok: bool,
    pub vars: Vars,
    /// `HH:MM UTC`.
    pub utc: String,
}

/// The variables the FAP polls.
pub fn fap_names() -> Vec<String> {
    let mut v: Vec<String> = [
        "A32NX_COND_CKPT_TEMP", "A32NX_COND_CARGO_FWD_TEMP", "A32NX_COND_CARGO_BULK_TEMP", "A32NX_COND_PURS_SEL_TEMPERATURE", "A32NX_CABIN_WATER_QTY_PERCENT", "A32NX_CABIN_WATER_QTY_KG",
        "A32NX_CABIN_WATER_PRESS_PSI", "A32NX_CABIN_WATER_COMPRESSOR_CMD", "A32NX_CABIN_WATER_FLOW_FRACTION", "A32NX_CABIN_WATER_LEAK_L_S", "A32NX_CABIN_WASTE_GENERATOR_RUNNING",
        "A32NX_CABIN_SLIDE_PRESSURE_LOW:1", "A32NX_CABIN_SLIDE_DEPLOYED:1", "A32NX_CABIN_SLIDE_BOTTLE_PA:1", "A32NX_CABIN_DOOR_LATCHED:1", "A32NX_CABIN_CARGO_DOOR_JAMMED:1",
        "A32NX_CARGO_FWD_SMOKE_DETECTED", "A32NX_CARGO_AFT_SMOKE_DETECTED", "A32NX_CABIN_IFE_CONTENT_AVAILABLE", "A32NX_CABIN_SEAT_POWER_CMD", "A32NX_CABIN_GALLEY_TOTAL_POWER_W",
        "A32NX_CABIN_CREW_CALL_NEW_COUNT", "A32NX_CABIN_CREW_CALL_PRIORITY", "A32NX_DEEP_CABIN_PURSER_TEMP_SEL_FAULT", "A32NX_DEEP_CABIN_LAV_GALLEY_EXTRACT_FAULT",
        "A32NX_DEEP_CABIN_SECONDARY_FANS_FAILED_COUNT", "A32NX_DEEP_CABIN_IFE_BAY_ISOL_FAULT", "A32NX_DEEP_CABIN_IFE_BAY_VENT_FAULT", "A32NX_DEEP_COM_CIDS_PA_MAIN_MAGNITUDE",
        "A32NX_DEEP_COM_CIDS_PA_UPPER_MAGNITUDE", "A32NX_DEEP_COM_CIDS_PA_LOWER_MAGNITUDE", "A32NX_DEEP_COM_CIDS_INTERPHONE_MAGNITUDE",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    v.extend((1..=8).map(|i| format!("A32NX_COND_MAIN_DECK_{i}_TEMP")));
    v.extend((1..=7).map(|i| format!("A32NX_COND_UPPER_DECK_{i}_TEMP")));
    v.extend((0..18).map(|i| format!("INTERACTIVE POINT OPEN:{i}")));
    v.extend(DOORS[..16].iter().map(|d| format!("ANIM_DOOR_{d}_DISARMED")));
    for i in 1..=3 {
        for k in [
            format!("A32NX_DEEP_COM_CIDS_{i}_FAULT"), format!("A32NX_CABIN_WASTE_LEVEL_PERCENT:{i}"), format!("A32NX_CABIN_WASTE_TANK_FULL:{i}"), format!("A32NX_CABIN_LAV_INOPERATIVE:{i}"),
            format!("A32NX_CABIN_WATER_HEATER_TEMP_C:{i}"), format!("A32NX_CABIN_IFE_ZONE_SMOKE:{i}"), format!("A32NX_CABIN_IFE_ZONE_OVERHEAT:{i}"), format!("A32NX_CABIN_IFE_ZONE_TRIPPED:{i}"),
            format!("A32NX_CABIN_IFE_ZONE_TEMP_C:{i}"), format!("A32NX_CABIN_GALLEY_OVEN_SMOKE:{i}"), format!("A32NX_CABIN_GALLEY_OVEN_TEMP_C:{i}"), format!("A32NX_CABIN_GALLEY_CHILLER_TEMP_C:{i}"),
            format!("A32NX_CABIN_GALLEY_BOILER_TEMP_C:{i}"), format!("A32NX_CABIN_GALLEY_BUS_FAULT:{i}"),
        ] {
            v.push(k);
        }
    }
    for i in 1..=2 {
        v.extend([format!("A32NX_CABIN_IFE_SERVER_FAIL:{i}"), format!("A32NX_CABIN_MAST_BLOCKED:{i}"), format!("A32NX_CABIN_MAST_ICE_KG:{i}")]);
    }
    v
}

struct F<'a>(&'a Vars);

impl F<'_> {
    fn v(&self, k: &str) -> Option<f64> {
        self.0.get(k).and_then(Value::as_f64).filter(|x| x.is_finite())
    }
    fn on(&self, k: &str) -> bool {
        self.v(k).unwrap_or(0.0) >= 0.5
    }
}

fn ftxt(x: Option<f64>, d: usize, unit: &str) -> String {
    x.map(|x| format!("{}{unit}", fx(x, d))).unwrap_or("--".into())
}

fn row(k: &str, v: &str, cls: &str) -> Node {
    el("div", "fap-row").child(Node::new("span").text(k)).child(Node::el("span", &format!("fap-val {cls}")).text(v))
}

fn bx(title: &str, kids: Vec<Node>) -> Node {
    el("div", "fap-box").child(label("fap-boxh", title)).children(kids)
}

fn btn(text: &str, on: bool, act: &str, cls: &str) -> Node {
    Node::el("button", &format!("fap-btn {cls}")).class_if("on", on).attr("type", "button").act(act).text(text)
}

fn temp_class(t: Option<f64>) -> &'static str {
    match t {
        None => "na",
        Some(t) if t < 20.0 => "cold",
        Some(t) if t > 26.0 => "hot",
        _ => "ok",
    }
}

fn zone(f: &F, l: &str, key: &str) -> Node {
    let t = f.v(key);
    Node::el("div", &format!("fap-zone {}", temp_class(t))).child(span("fap-zl", l)).child(span("fap-zt", &ftxt(t, 1, "\u{b0}")))
}

fn door_state(f: &F, i: usize) -> (String, &'static str) {
    match f.v(&format!("INTERACTIVE POINT OPEN:{i}")) {
        None => ("--".into(), "na"),
        Some(p) if p > 98.0 => ("OPEN".into(), "amber"),
        Some(p) if p > 2.0 => (format!("{} %", num(round(p))), "amber"),
        _ => ("CLOSED".into(), "green"),
    }
}

fn setting(m: &EfbModel, t: &SettingsTable, k: &str) -> Option<String> {
    m.settings.get(k).cloned().or_else(|| t.defaults.get(k).cloned())
}

fn slider_value(raw: Option<String>, lo: f64, hi: f64, step: f64, default: f64) -> f64 {
    let x = raw.and_then(|v| v.trim().parse::<f64>().ok()).filter(|x| x.is_finite()).unwrap_or(default);
    crate::settings::clamp_step(x, lo, hi, step)
}

fn yn<'a>(c: bool, a: &'a str, b: &'a str) -> &'a str {
    if c { a } else { b }
}

fn page(p: &str, f: &F, m: &EfbModel, table: &SettingsTable) -> Vec<Node> {
    match p {
        "status" => {
            let open = (0..DOORS.len()).filter(|i| {
                let s = door_state(f, *i).0;
                s != "CLOSED" && s != "--"
            }).count();
            let temps: Vec<f64> = (1..=8).map(|i| format!("A32NX_COND_MAIN_DECK_{i}_TEMP")).chain((1..=7).map(|i| format!("A32NX_COND_UPPER_DECK_{i}_TEMP"))).filter_map(|k| f.v(&k)).collect();
            let avg = (!temps.is_empty()).then(|| temps.iter().sum::<f64>() / temps.len() as f64);
            let waste = (1..=3).map(|i| f.v(&format!("A32NX_CABIN_WASTE_LEVEL_PERCENT:{i}")).filter(|x| *x != 0.0).unwrap_or(0.0)).fold(f64::NEG_INFINITY, f64::max);
            let lavs = (1..=3).filter(|i| f.on(&format!("A32NX_CABIN_LAV_INOPERATIVE:{i}"))).count();
            let slide = f.on("A32NX_CABIN_SLIDE_PRESSURE_LOW:1");
            let ife = f.on("A32NX_CABIN_IFE_CONTENT_AVAILABLE");
            vec![el("div", "fap-grid")
                .child(bx("DOORS", vec![row("Open doors", &open.to_string(), yn(open > 0, "amber", "green")), row("Slides", yn(slide, "CHECK", "NORMAL"), yn(slide, "amber", "green"))]))
                .child(bx("TEMPERATURE", vec![row("Cabin average", &ftxt(avg, 1, " \u{b0}C"), ""), row("Selected (purser)", &ftxt(f.v("A32NX_COND_PURS_SEL_TEMPERATURE"), 1, " \u{b0}C"), "")]))
                .child(bx(
                    "WATER / WASTE",
                    vec![
                        row("Potable water", &ftxt(f.v("A32NX_CABIN_WATER_QTY_PERCENT"), 0, " %"), ""),
                        row("Waste (highest)", &format!("{} %", num(round(waste))), yn(waste > 90.0, "amber", "")),
                        row("Lavatories inop", &lavs.to_string(), yn(lavs > 0, "amber", "")),
                    ],
                ))
                .child(bx(
                    "CABIN",
                    vec![
                        row("IFE content", yn(ife, "AVAILABLE", "NOT AVAILABLE"), yn(ife, "green", "amber")),
                        row("Seat power", yn(f.on("A32NX_CABIN_SEAT_POWER_CMD"), "ON", "OFF"), ""),
                        row("Galley load", &ftxt(Some(f.v("A32NX_CABIN_GALLEY_TOTAL_POWER_W").unwrap_or(0.0) / 1000.0), 1, " kW"), ""),
                        row("Crew calls", &num(f.v("A32NX_CABIN_CREW_CALL_NEW_COUNT").filter(|x| *x != 0.0).unwrap_or(0.0)), ""),
                    ],
                ))]
        }
        "temp" => {
            let sel = f.v("A32NX_COND_PURS_SEL_TEMPERATURE");
            let deck = |l: &str, zs: Vec<Node>| el("div", "fap-deck").child(label("fap-deckl", l)).child(el("div", "fap-zones").children(zs));
            let upper = deck("UPPER DECK", (1..=7).map(|i| zone(f, &format!("U{i}"), &format!("A32NX_COND_UPPER_DECK_{i}_TEMP"))).collect());
            let mut mz = vec![zone(f, "CKPT", "A32NX_COND_CKPT_TEMP")];
            mz.extend((1..=8).map(|i| zone(f, &format!("M{i}"), &format!("A32NX_COND_MAIN_DECK_{i}_TEMP"))));
            let main = deck("MAIN DECK", mz);
            let cargo = deck("CARGO", vec![zone(f, "FWD", "A32NX_COND_CARGO_FWD_TEMP"), zone(f, "BULK", "A32NX_COND_CARGO_BULK_TEMP")]);
            let mut out = vec![
                el("div", "fap-plane").child(upper).child(main).child(cargo),
                el("div", "fap-sel")
                    .child(Node::new("span").text("CABIN TEMP SELECTED"))
                    .child(btn("\u{25bc}", false, "fap:temp:-0.5", ""))
                    .child(span("fap-selv", &ftxt(sel, 1, " \u{b0}C")))
                    .child(btn("\u{25b2}", false, "fap:temp:0.5", "")),
                label("fap-note", "The cabin zones follow this temperature when the cockpit CABIN temperature selector is set to PURS SEL; otherwise the selector sets them. Range 18 to 30 \u{b0}C."),
            ];
            if f.on("A32NX_DEEP_CABIN_PURSER_TEMP_SEL_FAULT") {
                out.push(label("fap-note amber", "PURSER TEMP SELECTION FAULT: the cockpit CABIN selector controls the cabin."));
            }
            out
        }
        "doors" => {
            let rows: Vec<Node> = DOORS
                .iter()
                .enumerate()
                .map(|(i, d)| {
                    let (t, c) = door_state(f, i);
                    let lever = if i < 16 { f.v(&format!("ANIM_DOOR_{d}_DISARMED")) } else { None };
                    let slide = match lever {
                        None => "",
                        Some(l) if l < 0.5 => " \u{b7} ARMED",
                        _ => " \u{b7} DISARMED",
                    };
                    Node::el("div", &format!("fap-door {c}")).class_if("armed", lever.is_some_and(|l| l < 0.5)).child(Node::new("span").text(d)).child(Node::new("span").text(&format!("{t}{slide}")))
                })
                .collect();
            let psi = f.v("A32NX_CABIN_SLIDE_BOTTLE_PA:1");
            let low = f.on("A32NX_CABIN_SLIDE_PRESSURE_LOW:1");
            let dep = f.on("A32NX_CABIN_SLIDE_DEPLOYED:1");
            let jam = f.on("A32NX_CABIN_CARGO_DOOR_JAMMED:1");
            vec![
                el("div", "fap-doors").children(rows),
                bx(
                    "SLIDES",
                    vec![
                        row("Bottle pressure", &psi.map(|p| format!("{} psi", num(round(p / 6894.757)))).unwrap_or("--".into()), yn(low, "amber", "green")),
                        row("Deployed", yn(dep, "YES", "NO"), yn(dep, "amber", "")),
                        row("Door latch", yn(f.on("A32NX_CABIN_DOOR_LATCHED:1"), "LATCHED", "UNLATCHED"), ""),
                        row("Cargo door", yn(jam, "JAMMED", "NORMAL"), yn(jam, "amber", "")),
                    ],
                ),
            ]
        }
        "water" => {
            let bar = |pct: Option<f64>, cls: &str| {
                let p = pct.filter(|x| *x != 0.0 && !x.is_nan()).unwrap_or(0.0).min(100.0).max(0.0);
                el("div", "fap-bar").child(Node::el("div", &format!("fap-fill {cls}")).style(&format!("height:{}%", num(p))))
            };
            let tank = f.v("A32NX_CABIN_WATER_QTY_PERCENT");
            let leak = f.v("A32NX_CABIN_WATER_LEAK_L_S").unwrap_or(0.0) > 0.001;
            let mut wk = vec![
                el("div", "fap-tanks").child(el("div", "fap-tank").child(bar(tank, "water")).child(Node::new("div").text(&ftxt(tank, 0, " %"))).child(label("fap-small", &ftxt(f.v("A32NX_CABIN_WATER_QTY_KG"), 0, " kg")))),
                row("Pressure", &ftxt(f.v("A32NX_CABIN_WATER_PRESS_PSI"), 1, " psi"), ""),
                row("Compressor", yn(f.on("A32NX_CABIN_WATER_COMPRESSOR_CMD"), "ON", "OFF"), ""),
                row("Leak", yn(leak, "DETECTED", "NONE"), yn(leak, "amber", "")),
            ];
            wk.extend((1..=3).map(|i| row(&format!("Heater {}", ZONES[i - 1]), &ftxt(f.v(&format!("A32NX_CABIN_WATER_HEATER_TEMP_C:{i}")), 0, " \u{b0}C"), "")));
            wk.extend((1..=2).map(|i| {
                let b = f.on(&format!("A32NX_CABIN_MAST_BLOCKED:{i}"));
                row(&format!("Drain mast {i}"), yn(b, "BLOCKED", "CLEAR"), yn(b, "amber", ""))
            }));
            let mut ws = vec![
                el("div", "fap-tanks").children((1..=3).map(|i| {
                    let w = f.v(&format!("A32NX_CABIN_WASTE_LEVEL_PERCENT:{i}"));
                    el("div", "fap-tank").child(bar(w, if w.is_some_and(|w| w > 90.0) { "full" } else { "waste" })).child(Node::new("div").text(&ftxt(w, 0, " %"))).child(label("fap-small", ZONES[i - 1]))
                })),
                row("Vacuum generator", yn(f.on("A32NX_CABIN_WASTE_GENERATOR_RUNNING"), "RUNNING", "OFF"), ""),
            ];
            ws.extend((1..=3).map(|i| {
                let inop = f.on(&format!("A32NX_CABIN_LAV_INOPERATIVE:{i}"));
                row(&format!("Lavatories {}", ZONES[i - 1]), yn(inop, "INOP", "NORMAL"), yn(inop, "amber", "green"))
            }));
            vec![el("div", "fap-grid two").child(bx("POTABLE WATER", wk)).child(bx("WASTE", ws))]
        }
        "smoke" => {
            let mut det: Vec<(String, String)> = vec![("CARGO FWD".into(), "A32NX_CARGO_FWD_SMOKE_DETECTED".into()), ("CARGO AFT".into(), "A32NX_CARGO_AFT_SMOKE_DETECTED".into())];
            det.extend((1..=3).map(|i| (format!("IFE ZONE {}", ZONES[i - 1]), format!("A32NX_CABIN_IFE_ZONE_SMOKE:{i}"))));
            det.extend((1..=3).map(|i| (format!("GALLEY {} OVEN", ZONES[i - 1]), format!("A32NX_CABIN_GALLEY_OVEN_SMOKE:{i}"))));
            vec![el("div", "fap-doors").children(det.iter().map(|(n, k)| {
                let (c, t) = match f.v(k) {
                    None => ("na", "--"),
                    Some(_) if f.on(k) => ("red", "SMOKE"),
                    _ => ("green", "NORMAL"),
                };
                Node::el("div", &format!("fap-door {c}")).child(Node::new("span").text(n)).child(Node::new("span").text(t))
            }))]
        }
        "audio" => {
            let ann = setting(m, table, "SOUND_ANNOUNCEMENTS_ENABLED").as_deref() == Some("1");
            let vol = slider_value(setting(m, table, "SOUND_LEVEL_CABIN"), 0.0, 100.0, 5.0, 100.0);
            let ife = f.on("A32NX_CABIN_IFE_CONTENT_AVAILABLE");
            let mut ifek = vec![row("Content", yn(ife, "AVAILABLE", "NOT AVAILABLE"), yn(ife, "green", "amber"))];
            ifek.extend((1..=2).map(|i| {
                let b = f.on(&format!("A32NX_CABIN_IFE_SERVER_FAIL:{i}"));
                row(&format!("Server {i}"), yn(b, "FAULT", "NORMAL"), yn(b, "amber", "green"))
            }));
            vec![el("div", "fap-grid two")
                .child(bx(
                    "ANNOUNCEMENTS",
                    vec![
                        el("div", "fap-btns").child(btn("PRE-RECORDED ON", ann, "set:SOUND_ANNOUNCEMENTS_ENABLED=1", "")).child(btn("OFF", !ann, "set:SOUND_ANNOUNCEMENTS_ENABLED=0", "")),
                        label("fap-note", "The pre-recorded cabin announcements play by flight phase."),
                    ],
                ))
                .child(bx(
                    "CABIN VOLUME",
                    vec![
                        Node::el("input", "fap-range").attr("type", "range").attr("min", "0").attr("max", "100").attr("step", "5").value(&num(vol)).act("range:SOUND_LEVEL_CABIN"),
                        label("fap-selv", &format!("{} %", num(vol))),
                    ],
                ))
                .child(bx(
                    "PA / INTERPHONE",
                    vec![
                        row("PA main deck", &ftxt(f.v("A32NX_DEEP_COM_CIDS_PA_MAIN_MAGNITUDE"), 2, ""), ""),
                        row("PA upper deck", &ftxt(f.v("A32NX_DEEP_COM_CIDS_PA_UPPER_MAGNITUDE"), 2, ""), ""),
                        row("PA lower deck", &ftxt(f.v("A32NX_DEEP_COM_CIDS_PA_LOWER_MAGNITUDE"), 2, ""), ""),
                        row("Interphone", &ftxt(f.v("A32NX_DEEP_COM_CIDS_INTERPHONE_MAGNITUDE"), 2, ""), ""),
                    ],
                ))
                .child(bx("IFE", ifek))]
        }
        "lights" => {
            let item = table.categories.iter().flat_map(|c| &c.items).find(|it| it.key.as_deref() == Some("xphfbw.cabinScene"));
            let opts: Vec<(String, String)> = item.and_then(|it| it.sel.clone().or(it.seg.clone())).unwrap_or_default();
            let cur = setting(m, table, "xphfbw.cabinScene").unwrap_or("0".into());
            let bright = slider_value(setting(m, table, "xphfbw.cabinBrightness"), 0.0, 1.0, 0.05, 1.0);
            vec![
                bx("CABIN LIGHTING SCENE", vec![el("div", "fap-btns wrap").children(opts.iter().map(|(v, l)| btn(&l.to_uppercase(), *v == cur, &format!("set:xphfbw.cabinScene={v}"), "")))]),
                bx(
                    "DIMMING",
                    vec![
                        Node::el("input", "fap-range").attr("type", "range").attr("min", "0").attr("max", "1").attr("step", "0.05").value(&num(bright)).act("range:xphfbw.cabinBrightness"),
                        label("fap-selv", &format!("{} %", num(round(bright * 100.0)))),
                    ],
                ),
            ]
        }
        "seats" => vec![
            el("div", "fap-grid").children((1..=3).map(|i| {
                let tr = f.on(&format!("A32NX_CABIN_IFE_ZONE_TRIPPED:{i}"));
                let oh = f.on(&format!("A32NX_CABIN_IFE_ZONE_OVERHEAT:{i}"));
                bx(
                    &format!("SEAT / IFE ZONE {}", ZONES[i - 1]),
                    vec![
                        row("In-seat power", if tr { "TRIPPED" } else if f.on("A32NX_CABIN_SEAT_POWER_CMD") { "ON" } else { "OFF" }, yn(tr, "amber", "")),
                        row("Equipment temp", &ftxt(f.v(&format!("A32NX_CABIN_IFE_ZONE_TEMP_C:{i}")), 0, " \u{b0}C"), yn(oh, "amber", "")),
                        row("Overheat", yn(oh, "YES", "NO"), yn(oh, "amber", "")),
                    ],
                )
            })),
            label("fap-note", "In-seat power follows the commercial electrical supply: it is shed with the galleys and when a zone trips."),
        ],
        "sysinfo" => {
            let lgf = f.on("A32NX_DEEP_CABIN_LAV_GALLEY_EXTRACT_FAULT");
            let mut boxes = vec![
                bx(
                    "CIDS DIRECTORS",
                    (1..=3)
                        .map(|i| {
                            let b = f.on(&format!("A32NX_DEEP_COM_CIDS_{i}_FAULT"));
                            row(&format!("Director {i}"), yn(b, "FAULT", "NORMAL"), yn(b, "amber", "green"))
                        })
                        .collect(),
                ),
                bx(
                    "VENTILATION",
                    vec![
                        row("Lav / galley extract", yn(lgf, "FAULT", "NORMAL"), yn(lgf, "amber", "green")),
                        row("Secondary fans failed", &num(f.v("A32NX_DEEP_CABIN_SECONDARY_FANS_FAILED_COUNT").filter(|x| *x != 0.0).unwrap_or(0.0)), ""),
                        row("IFE bay isolation", yn(f.on("A32NX_DEEP_CABIN_IFE_BAY_ISOL_FAULT"), "FAULT", "NORMAL"), ""),
                        row("IFE bay ventilation", yn(f.on("A32NX_DEEP_CABIN_IFE_BAY_VENT_FAULT"), "FAULT", "NORMAL"), ""),
                    ],
                ),
            ];
            boxes.extend((1..=3).map(|i| {
                let b = f.on(&format!("A32NX_CABIN_GALLEY_BUS_FAULT:{i}"));
                bx(
                    &format!("GALLEY {}", ZONES[i - 1]),
                    vec![
                        row("Supply", yn(b, "FAULT", "NORMAL"), yn(b, "amber", "green")),
                        row("Oven", &ftxt(f.v(&format!("A32NX_CABIN_GALLEY_OVEN_TEMP_C:{i}")), 0, " \u{b0}C"), ""),
                        row("Chiller", &ftxt(f.v(&format!("A32NX_CABIN_GALLEY_CHILLER_TEMP_C:{i}")), 1, " \u{b0}C"), ""),
                        row("Boiler", &ftxt(f.v(&format!("A32NX_CABIN_GALLEY_BOILER_TEMP_C:{i}")), 0, " \u{b0}C"), ""),
                    ],
                )
            }));
            vec![el("div", "fap-grid").children(boxes)]
        }
        _ => vec![],
    }
}

fn cautions(f: &F) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    if f.on("A32NX_CARGO_FWD_SMOKE_DETECTED") || f.on("A32NX_CARGO_AFT_SMOKE_DETECTED") {
        out.push(("red", "CARGO SMOKE".to_string()));
    }
    for i in 1..=3 {
        let z = ZONES[i - 1];
        if f.on(&format!("A32NX_CABIN_IFE_ZONE_SMOKE:{i}")) {
            out.push(("red", format!("SMOKE IFE ZONE {z}")));
        }
        if f.on(&format!("A32NX_CABIN_GALLEY_OVEN_SMOKE:{i}")) {
            out.push(("red", format!("SMOKE GALLEY {z}")));
        }
        if f.on(&format!("A32NX_CABIN_WASTE_TANK_FULL:{i}")) {
            out.push(("amber", format!("WASTE TANK {z} FULL")));
        }
        if f.on(&format!("A32NX_DEEP_COM_CIDS_{i}_FAULT")) {
            out.push(("amber", format!("CIDS DIR {i} FAULT")));
        }
    }
    if f.on("A32NX_CABIN_SLIDE_PRESSURE_LOW:1") {
        out.push(("amber", "SLIDE BOTTLE PRESSURE LOW".into()));
    }
    if f.on("A32NX_CABIN_SLIDE_DEPLOYED:1") {
        out.push(("amber", "SLIDE DEPLOYED".into()));
    }
    if f.v("A32NX_CABIN_WATER_QTY_PERCENT").is_some_and(|w| w < 10.0) {
        out.push(("amber", "POTABLE WATER LOW".into()));
    }
    let calls = f.v("A32NX_CABIN_CREW_CALL_NEW_COUNT").filter(|x| *x != 0.0).unwrap_or(0.0);
    if calls > 0.0 {
        out.push(("cyan", format!("{} CREW CALL{}", num(calls), if calls > 1.0 { "S" } else { "" })));
    }
    out
}

pub fn fap_content(v: &FapView, m: &EfbModel, table: &SettingsTable) -> Node {
    let f = F(&v.vars);
    let pg = if v.page.is_empty() { "status" } else { v.page.as_str() };
    let flt = match m.ofp.as_ref().filter(|o| o.get("general").is_some_and(crate::jv::truthy)) {
        Some(o) => {
            let g = &o["general"];
            let t = |x: &Value, k: &str| if crate::jv::b(x, k) { crate::jv::sv(x, k, "") } else { String::new() };
            let code = |k: &str| if crate::jv::b(o, k) { crate::jv::sv(&o[k], "icao_code", "undefined") } else { String::new() };
            format!("{}{}  {}-{}", t(g, "icao_airline"), t(g, "flight_number"), code("origin"), code("destination"))
        }
        None => "NO FLIGHT DATA".into(),
    };
    let title = FAP_PAGES.iter().find(|x| x.0 == pg).unwrap_or(&FAP_PAGES[0]).1;
    let nav = el("div", "fap-nav").children(FAP_PAGES.iter().map(|(k, t)| btn(t, pg == *k, &format!("fap:page:{k}"), "fap-navb")));
    let mut main = el("div", "fap-main");
    if !v.ok {
        main = main.child(label("fap-note", "CIDS NOT CONNECTED: start X-Plane with the A380 to use the FAP."));
    }
    main = main.children(page(pg, &f, m, table));
    let c = cautions(&f);
    let foot = if c.is_empty() { el("div", "fap-foot").child(span("fap-caut none", "NO CIDS CAUTION")) } else { el("div", "fap-foot").children(c.iter().map(|(k, t)| span(&format!("fap-caut {k}"), t))) };
    let screen = el("div", "fap-screen")
        .child(el("div", "fap-top").child(span("fap-title", title)).child(Node::new("span").text(&flt)).child(Node::new("span").text(&v.utc)))
        .child(el("div", "fap-body").child(nav).child(main))
        .child(foot);
    Node::new("div").id("fap-content").child(el("div", "fap-bezel").child(label("fap-brand", "FLIGHT ATTENDANT PANEL")).child(screen))
}

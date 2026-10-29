use crate::flt_ops::{Runway, Weather};
use crate::inflight_model as ifm;
use crate::perf_common::*;
use crate::perf_model::*;
use crate::to_compute::{TlrPhase, TlrRunway};
use crate::widgets::{button, button_sel, div, dropdown, input_field, radio_group, Dropdown, Field};
use mfd_tree::Node;
use serde::Deserialize;

fn label(text: &str) -> Node {
    Node::el("div", "oit-label bigger").text(text)
}

fn green(text: &str) -> Node {
    Node::el("div", "oit-label bigger oit-green-text").text(text)
}

fn grid(pairs: Vec<(&str, Node)>) -> Node {
    let mut g = div("oit-perf-grid");
    for (k, v) in pairs {
        g = g.child(label(k)).child(v);
    }
    g
}

fn box_with(title: &str) -> Node {
    Node::el("div", "oit-labeled-box-container").child(Node::el("span", "oit-labeled-box-label").text(title))
}

fn note(class: &str, text: &str) -> Node {
    Node::el("div", class).text(text)
}

fn options_box() -> Node {
    Node::el("div", "oit-labeled-box-container oit-perf-options")
        .child(Node::el("span", "oit-labeled-box-label").text("OPTIONS"))
        .child(
            div("fr")
                .child(
                    div("oit-perf-options-column")
                        .child(Node::el("div", "oit-label").text("WEIGHT"))
                        .child(radio_group("oit-perf-opt-weight", &["T", "KLB"])),
                )
                .child(
                    div("oit-perf-options-column")
                        .child(Node::el("div", "oit-label").text("ALTIMETER"))
                        .child(radio_group("oit-perf-opt-alt", &["HPA", "IN HG"])),
                )
                .child(
                    div("oit-perf-options-column")
                        .child(Node::el("div", "oit-label").text("DISTANCE"))
                        .child(radio_group("oit-perf-opt-dist", &["M", "FT"])),
                ),
        )
}

fn selector(label_text: &str, d: &Dropdown, values: &[&str], id: &str, width: u32, who: &str) -> Node {
    let mut d = d.clone();
    d.values = values.iter().map(|s| s.to_string()).collect();
    if d.text.is_empty() {
        d.text = values[0].to_string();
    }
    d.container_style = format!("width: {width}px;");
    d.digits = values.iter().map(|v| v.chars().count() as u32).max().unwrap_or(0);
    div("oit-perf-field")
        .child(Node::el("div", "oit-label bigger oit-perf-field-key").text(label_text))
        .child(dropdown(&format!("{who}_OIT_{id}"), &d))
}

fn selected_text<'a>(d: &'a Dropdown, values: &[&'a str]) -> &'a str {
    if d.text.is_empty() { values[0] } else { &d.text }
}

fn wind_text(w: &Weather) -> String {
    match (w.wind_direction_true, w.wind_kt) {
        (Some(d), Some(k)) => format!("{:0>3}°T/{:0>3}KT", fixed(js_round(d), 0), fixed(js_round(k), 0)),
        _ => "---°/---".into(),
    }
}

fn headwind_of(rwy: &Runway, w: &Weather) -> f64 {
    match (w.wind_direction_true, w.wind_kt) {
        (Some(d), Some(k)) => k * ((d - rwy.bearing_true) * std::f64::consts::PI / 180.0).cos(),
        _ => 0.0,
    }
}

fn conditions_grid(rwy: Option<&Runway>, w: &Weather, u: &Units) -> Node {
    let (headwind, crosswind, press_alt) = match (rwy, w.wind_direction_true, w.wind_kt, w.qnh) {
        (Some(r), Some(d), Some(k), Some(q)) => {
            let off = (d - r.bearing_true).to_radians();
            let hw = k * off.cos();
            let xw = k * off.sin();
            (
                format!("{} {}KT", if hw >= 0.0 { "HEAD" } else { "TAIL" }, fixed(js_round(hw.abs()), 0)),
                format!("{} {}KT", if xw >= 0.0 { "R" } else { "L" }, fixed(js_round(xw.abs()), 0)),
                format!("{} FT", fixed(js_round(r.elevation_ft + (1013.25 - q) * 27.0), 0)),
            )
        }
        _ => ("---".into(), "---".into(), "-----".into()),
    };
    grid(vec![
        ("OAT", green(&w.oat.map_or("---".to_string(), |v| format!("{}°C", fixed(js_round(v), 0))))),
        ("QNH", green(&w.qnh.map_or("----".to_string(), |q| format!("{} {}", format_pressure(q, u), pressure_unit_label(u))))),
        ("WIND", green(&wind_text(w))),
        ("PRESS ALT", green(&press_alt)),
        ("HEADWIND", green(&headwind)),
        ("CROSSWIND", green(&crosswind)),
    ])
}

const RUNWAY_CONDITIONS: [&str; 3] = ["DRY", "GOOD", "MEDIUM"];
const LDG_FLAPS: [&str; 2] = ["CONF FULL", "CONF 3"];
const TECHNIQUE: [&str; 2] = ["MANUAL", "AUTOLAND"];
const BRAKE_MODE: [&str; 4] = ["LO", "2", "3", "HI"];
const REVERSE: [&str; 2] = ["YES", "NO"];
const VPILOT: [&str; 5] = ["0", "5", "10", "15", "20"];
const LDG_TLR_NOTE: &str = "SYNC SIMBRIEF FOR THE SIMBRIEF LANDING REPORT (OFP GENERATED WITH RUNWAY ANALYSIS ON)";

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct LdgCompute {
    pub pressed: bool,
    pub tlr: Option<TlrPhase>,
    pub tlr_weight_units: String,
    pub calibration: PerfCalibration,
}

impl Default for LdgCompute {
    fn default() -> Self {
        LdgCompute { pressed: false, tlr: None, tlr_weight_units: "KG".into(), calibration: NO_CALIBRATION }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct LdgPerfModel {
    pub units: Units,
    pub airport: Field,
    pub runway_dropdown: Dropdown,
    pub database_message: String,
    pub runway: Option<Runway>,
    pub landing_weight_kg: Option<f64>,
    pub lw_field: Field,
    pub lw_source: String,
    pub weather: Weather,
    pub cg: Option<f64>,
    pub rwy_cond: Dropdown,
    pub flaps: Dropdown,
    pub vpilot: Dropdown,
    pub technique: Dropdown,
    pub brake: Dropdown,
    pub reverse: Dropdown,
    pub compute: LdgCompute,
}

impl Default for LdgPerfModel {
    fn default() -> Self {
        LdgPerfModel {
            units: Units::default(),
            airport: Field::default(),
            runway_dropdown: Dropdown::default(),
            database_message: String::new(),
            runway: None,
            landing_weight_kg: None,
            lw_field: Field::default(),
            lw_source: "NOT ENTERED".into(),
            weather: Weather::default(),
            cg: None,
            rwy_cond: Dropdown::default(),
            flaps: Dropdown::default(),
            vpilot: Dropdown::default(),
            technique: Dropdown::default(),
            brake: Dropdown::default(),
            reverse: Dropdown::default(),
            compute: LdgCompute::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LdgResults {
    pub runway_text: String,
    pub tlr_vref: String,
    pub tlr_max_dry: String,
    pub tlr_max_wet: String,
    pub tlr_note: String,
    pub from_simbrief: bool,
    pub model_ok: bool,
    pub ld: String,
    pub fld: String,
    pub margin_ld: String,
    pub margin_fld: String,
    pub vapp: String,
    pub xwind: String,
    pub model_note: String,
}

fn ldg_unavailable() -> LdgResults {
    LdgResults {
        runway_text: "----/---".into(),
        tlr_vref: UNAVAILABLE.into(),
        tlr_max_dry: UNAVAILABLE.into(),
        tlr_max_wet: UNAVAILABLE.into(),
        tlr_note: LDG_TLR_NOTE.into(),
        from_simbrief: false,
        model_ok: false,
        ld: UNAVAILABLE.into(),
        fld: UNAVAILABLE.into(),
        margin_ld: UNAVAILABLE.into(),
        margin_fld: UNAVAILABLE.into(),
        vapp: UNAVAILABLE.into(),
        xwind: UNAVAILABLE.into(),
        model_note: "PRESS COMPUTE FOR THE OIS MODEL RESULT".into(),
    }
}

fn num(v: &Option<String>) -> Option<f64> {
    let s = v.as_deref()?.trim();
    if s.is_empty() {
        return None;
    }
    s.parse::<f64>().ok().filter(|n| n.is_finite())
}

fn ldg_show_tlr(m: &LdgPerfModel, mut r: LdgResults) -> LdgResults {
    let tlr = match &m.compute.tlr {
        None => return r,
        Some(t) => t,
    };
    let ident = m.runway.as_ref().map(|x| x.ident.as_str());
    let list: Vec<&TlrRunway> = match &tlr.runway {
        crate::to_compute::TlrRunwayList::Many(v) => v.iter().collect(),
        crate::to_compute::TlrRunwayList::One(x) => vec![x],
    };
    let entry = match ident.and_then(|id| list.iter().find(|e| e.identifier == id)) {
        None => {
            r.tlr_note = if list.is_empty() {
                "THE SIMBRIEF OFP HAS NO LANDING REPORT: GENERATE IT WITH RUNWAY ANALYSIS ON".into()
            } else {
                format!("THE SIMBRIEF LANDING REPORT HAS NO RWY {}", ident.unwrap_or("---"))
            };
            return r;
        }
        Some(e) => e,
    };
    let u = &m.units;
    let to_kg = |v: f64| if m.compute.tlr_weight_units == "LB" { v * 0.45359237 } else { v };
    let weight = |v: &Option<String>| match num(v) {
        None => UNAVAILABLE.to_string(),
        Some(n) => format!("{} {}", format_weight(Some(to_kg(n)), u), weight_unit_label(u)),
    };
    r.tlr_vref = num(&entry.speeds_vref).map_or(UNAVAILABLE.to_string(), |v| format!("{} KT", fixed(js_round(v), 0)));
    r.tlr_max_dry = weight(&entry.max_weight_dry);
    r.tlr_max_wet = weight(&entry.max_weight_wet);
    r.from_simbrief = true;
    let c = &tlr.conditions;
    let mut assumed: Vec<String> = vec![];
    if let Some(f) = c.flap_setting.as_deref().filter(|x| !x.is_empty()) {
        assumed.push(format!("FLAPS {f}"));
    }
    if let Some(t) = c.temperature.as_deref().filter(|x| !x.is_empty()) {
        assumed.push(format!("{t}°C"));
    }
    if let (Some(d), Some(s)) = (c.wind_direction.as_deref().filter(|x| !x.is_empty()), c.wind_speed.as_deref().filter(|x| !x.is_empty())) {
        assumed.push(format!("WIND {:0>3}/{s}KT", d));
    }
    if let Some(sc) = c.surface_condition.as_deref().filter(|x| !x.is_empty()) {
        assumed.push(sc.to_uppercase());
    }
    r.tlr_note = format!(
        "SIMBRIEF LANDING REPORT, RWY {}{}: SIMBRIEF PLANNING DATA, NOT AIRBUS-CERTIFIED A380 PERFORMANCE",
        entry.identifier,
        if assumed.is_empty() { String::new() } else { format!(" (SIMBRIEF ASSUMED {})", assumed.join(", ")) }
    );
    r
}

pub fn ldg_results(m: &LdgPerfModel) -> LdgResults {
    let mut r = ldg_unavailable();
    if !m.compute.pressed {
        return ldg_show_tlr(m, r);
    }
    let rwy = m.runway.as_ref();
    if let Some(rw) = rwy {
        r.runway_text = format!("{}/{}", if m.airport.text.is_empty() { "----" } else { &m.airport.text }, rw.ident);
    }
    let r = ldg_show_tlr(m, r);
    ldg_show_model(m, r)
}

fn ldg_show_model(m: &LdgPerfModel, mut r: LdgResults) -> LdgResults {
    let rwy = match &m.runway {
        None => {
            r.model_note = "SELECT A RUNWAY".into();
            return r;
        }
        Some(x) => x,
    };
    let lw = match m.landing_weight_kg {
        None => {
            r.model_note = "ENTER THE LANDING WEIGHT".into();
            return r;
        }
        Some(x) => x,
    };
    let w = &m.weather;
    let oat = w.oat.unwrap_or(0.0);
    let qnh = w.qnh.unwrap_or(1013.25);
    let hw = headwind_of(rwy, w);
    let mu_name = selected_text(&m.rwy_cond, &RUNWAY_CONDITIONS);
    let mu = ldg_mu(mu_name).unwrap_or(0.38);
    let conf = if selected_text(&m.flaps, &LDG_FLAPS) == "CONF 3" { LdgConf::Three } else { LdgConf::Full };
    let conf_label = if conf == LdgConf::Three { "3" } else { "FULL" };
    let autoland = selected_text(&m.technique, &TECHNIQUE) == "AUTOLAND";
    let mode = selected_text(&m.brake, &BRAKE_MODE);
    let res = landing(
        &LandingInput {
            atm: atmos(rwy.elevation_ft, qnh, oat),
            lw_t: lw / 1000.0,
            cg: m.cg.unwrap_or(33.0),
            conf,
            mu,
            rev: selected_text(&m.reverse, &REVERSE) == "YES",
            autoland,
            slope: rwy.slope_percent,
            hw,
            lda: rwy.lda,
            cal: m.compute.calibration,
        },
        &[mode],
    );
    let row = &res.rows[0];
    let u = &m.units;
    let dist = |v: f64| format!("{} {}", format_distance(Some(v), u), distance_unit_label(u));
    let v_pilot: f64 = selected_text(&m.vpilot, &VPILOT).parse().unwrap_or(0.0);
    r.ld = dist(row.inflight);
    r.fld = dist(row.dispatch);
    r.margin_ld = dist(rwy.lda - row.inflight);
    r.margin_fld = dist(rwy.lda - row.dispatch);
    r.vapp = format!("{} KT (VLS {})", fixed(res.vapp + v_pilot, 0), fixed(res.vls, 0));
    r.xwind = format!("{} KT", fixed(crosswind_limit_kt(mu, autoland), 0));
    r.model_ok = true;
    let short = if row.inflight > rwy.lda {
        " LANDING DISTANCE EXCEEDS LDA."
    } else if row.dispatch > rwy.lda {
        " FACTORED DISTANCE EXCEEDS LDA."
    } else {
        ""
    };
    r.model_note = format!(
        "OIS MODEL, AUTOBRAKE {mode}, CONF {conf_label}: NOT AIRBUS-CERTIFIED.{short} {}",
        calibration_text(&m.compute.calibration)
    );
    r
}

pub fn ldg_perf_page(m: &LdgPerfModel, who: &str) -> Node {
    let u = &m.units;
    let rwy = m.runway.as_ref();
    let r = ldg_results(m);
    let dist_of = |f: &dyn Fn(&Runway) -> f64| rwy.map_or("----".to_string(), |x| format!("{} {}", format_distance(Some(f(x)), u), distance_unit_label(u)));

    let mut airport = m.airport.clone();
    airport.container_style = "width: 160px; margin-right: 20px;".into();
    airport.max_digits = 4;
    let mut rdd = m.runway_dropdown.clone();
    rdd.container_style = "width: 180px; margin-right: 20px;".into();
    rdd.digits = 4;
    let mut lw_field = m.lw_field.clone();
    lw_field.container_style = "width: 220px; margin-right: 20px;".into();
    lw_field.max_digits = 5;

    let slope = rwy.map_or("----".to_string(), |x| {
        format!("{} {}%", if x.slope_percent >= 0.0 { "UP" } else { "DOWN" }, fixed(x.slope_percent.abs(), 2))
    });
    let arrival = box_with("ARRIVAL")
        .child(
            div("fr oit-perf-row")
                .child(label("AIRPORT").class_if("oit-perf-key", true))
                .child(input_field(&airport))
                .child(Node::el("div", "oit-label bigger oit-perf-field-key").text("RWY"))
                .child(dropdown(&format!("{who}_OIT_ldgPerfRunway"), &rdd))
                .child(button("SYNC FMS", false, "width: 175px; margin-right: 20px;", ""))
                .child(button("SYNC SIMBRIEF", false, "width: 230px;", "")),
        )
        .child(note("oit-label amber oit-perf-note", &m.database_message))
        .child(grid(vec![
            ("LDA", green(&dist_of(&|x| x.lda))),
            ("ELEVN", green(&rwy.map_or("-----".to_string(), |x| format!("{} FT", fixed(js_round(x.elevation_ft), 0))))),
            ("SLOPE", green(&slope)),
            ("WIDTH", green(&dist_of(&|x| x.width))),
        ]));

    let w = &m.weather;
    let conditions = box_with("CONDITIONS")
        .child(
            div("fr oit-perf-row")
                .child(Node::el("div", "oit-label bigger oit-perf-key").text("LW"))
                .child(input_field(&lw_field))
                .child(Node::el("div", "oit-label").text(&m.lw_source)),
        )
        .child(conditions_grid(rwy, w, u))
        .child(
            div("fr oit-perf-row")
                .child(selector("RWY COND", &m.rwy_cond, &RUNWAY_CONDITIONS, "ldgPerfRwyCond", 180, who))
                .child(selector("FLAPS", &m.flaps, &LDG_FLAPS, "ldgPerfFlaps", 230, who))
                .child(selector("VPILOT", &m.vpilot, &VPILOT, "ldgPerfVpilot", 110, who)),
        )
        .child(
            div("fr oit-perf-row")
                .child(selector("TECHNIQUE", &m.technique, &TECHNIQUE, "ldgPerfTechnique", 210, who))
                .child(selector("BRAKE", &m.brake, &BRAKE_MODE, "ldgPerfBrake", 110, who))
                .child(selector("REVERSE", &m.reverse, &REVERSE, "ldgPerfReverse", 120, who)),
        );

    let model_cell = |t: &str| {
        Node::el("div", "oit-label bigger").class_if("oit-green-text", r.model_ok).class_if("amber", !r.model_ok).text(t)
    };
    let results = box_with("RESULTS")
        .child(
            div("fr oit-perf-row")
                .child(button("COMPUTE", false, "width: 200px; margin-right: 20px;", ""))
                .child(Node::new("div").style("flex-grow: 1"))
                .child(label("RWY"))
                .child(green(&r.runway_text).style("margin-left: 15px;")),
        )
        .child(grid(vec![
            ("LW USED", green(&format!("{} {}", format_weight(m.landing_weight_kg, u), weight_unit_label(u)))),
            ("LDA", green(&dist_of(&|x| x.lda))),
            ("LD", model_cell(&r.ld)),
            ("FLD", model_cell(&r.fld)),
            ("STOP MARGIN / LD", model_cell(&r.margin_ld)),
            ("STOP MARGIN / FLD", model_cell(&r.margin_fld)),
            ("VAPP", model_cell(&r.vapp)),
            ("MAX CROSSWIND", model_cell(&r.xwind)),
        ]))
        .child(note("oit-label amber oit-perf-note", &r.model_note));

    let tlr_cell = |t: &str| {
        Node::el("div", "oit-label bigger").class_if("cyan", r.from_simbrief).class_if("amber", !r.from_simbrief).text(t)
    };
    let tlr = box_with("SIMBRIEF LANDING REPORT")
        .child(
            div("oit-perf-grid")
                .child(label("VREF"))
                .child(tlr_cell(&r.tlr_vref))
                .child(label("MAX LW DRY"))
                .child(tlr_cell(&r.tlr_max_dry))
                .child(Node::el("div", "oit-label bigger"))
                .child(Node::el("div", "oit-label bigger"))
                .child(label("MAX LW WET"))
                .child(tlr_cell(&r.tlr_max_wet)),
        )
        .child(note("oit-label amber oit-perf-note", &r.tlr_note));

    Node::el("div", "oit-page-container oit-perf-page")
        .child(arrival)
        .child(conditions)
        .child(results)
        .child(tlr)
        .child(options_box())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Module {
    #[serde(rename = "CRZ")]
    Crz,
    #[serde(rename = "HLD")]
    Hld,
    #[serde(rename = "DES")]
    Des,
    #[serde(rename = "PLAN")]
    Plan,
    #[serde(rename = "ATMO")]
    Atmo,
}

impl Module {
    fn label(self) -> &'static str {
        match self {
            Module::Crz => "CRZ",
            Module::Hld => "HLD",
            Module::Des => "DES",
            Module::Plan => "PLAN",
            Module::Atmo => "ATMO",
        }
    }
}

const MODULES: [Module; 5] = [Module::Crz, Module::Hld, Module::Des, Module::Plan, Module::Atmo];

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Live {
    pub weight_kg: Option<f64>,
    pub fob_kg: Option<f64>,
    pub fl: Option<f64>,
    pub mach: Option<f64>,
    pub sat_c: Option<f64>,
    pub tat_c: Option<f64>,
    pub isa_dev: Option<f64>,
    pub ff_kg_h: f64,
    pub gs_kt: f64,
    pub wind_kt: f64,
    pub wind_dir: f64,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default)]
pub struct FlightModelCal {
    pub calibration: f64,
    pub samples: u32,
}

impl Default for FlightModelCal {
    fn default() -> Self {
        FlightModelCal { calibration: 1.0, samples: 0 }
    }
}

pub fn learn_flight_model(cal: &mut FlightModelCal, l: &Live, vs_fpm: f64) {
    let (Some(w), Some(fl), Some(mach), Some(isa)) = (l.weight_kg, l.fl, l.mach, l.isa_dev) else {
        return;
    };
    if fl < 200.0 || mach < 0.7 || vs_fpm.abs() > 300.0 || l.ff_kg_h < 2000.0 {
        return;
    }
    let model = ifm::cruise(w, fl, mach, isa, 1.0, 4.0).fuel_flow_kg_h;
    let ratio = (l.ff_kg_h / model).max(0.6).min(1.6);
    cal.samples += 1;
    let gain = if cal.samples < 20 { 1.0 / cal.samples as f64 } else { 0.05 };
    cal.calibration += (ratio - cal.calibration) * gain;
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct InFltModel {
    pub module: Module,
    pub engines: u32,
    pub live_weight: bool,
    pub live_level: bool,
    pub weight_kg: f64,
    pub level: f64,
    pub mach: f64,
    pub isa_dev: f64,
    pub fuel_factor: f64,
    pub distance_nm: f64,
    pub wind_kt: f64,
    pub elevation_ft: f64,
    pub live: Live,
    pub fm: FlightModelCal,
}

impl Default for InFltModel {
    fn default() -> Self {
        InFltModel {
            module: Module::Crz,
            engines: 4,
            live_weight: true,
            live_level: true,
            weight_kg: 450000.0,
            level: 370.0,
            mach: 0.85,
            isa_dev: 0.0,
            fuel_factor: 1.0,
            distance_nm: 500.0,
            wind_kt: 0.0,
            elevation_ft: 0.0,
            live: Live::default(),
            fm: FlightModelCal::default(),
        }
    }
}

pub fn resolve_inputs(m: &InFltModel) -> InFltModel {
    let mut r = m.clone();
    let l = &m.live;
    if m.live_weight {
        if let Some(w) = l.weight_kg.filter(|w| *w > 100000.0) {
            r.weight_kg = js_round(w / 100.0) * 100.0;
        }
    }
    if m.live_level {
        if let Some(fl) = l.fl {
            r.level = (js_round(fl / 10.0) * 10.0).min(431.0).max(100.0);
            if let Some(mach) = l.mach.filter(|x| *x > 0.5) {
                r.mach = js_round(mach * 100.0) / 100.0;
            }
            if let Some(isa) = l.isa_dev {
                r.isa_dev = js_round(isa);
            }
        }
    }
    r
}

fn tons(kg: f64) -> String {
    fixed(kg / 1000.0, 1)
}

fn hhmm(min: f64) -> String {
    let h = (min / 60.0).floor();
    let rest = fixed(js_round(min % 60.0), 0);
    format!("{}H{:0>2}", fixed(h, 0), rest)
}

fn fl_text(v: f64) -> String {
    format!("FL{:0>3}", fixed(js_round(v), 0))
}

pub fn inflight_rows(m: &InFltModel) -> Vec<(String, String)> {
    let r = resolve_inputs(m);
    let l = &r.live;
    let (w, level, mach, isa, eng) = (r.weight_kg, r.level, r.mach, r.isa_dev, r.engines as f64);
    let factor = r.fuel_factor * r.fm.calibration;
    let s = |a: &str, b: String| (a.to_string(), b);
    let kg_h = |v: f64| format!("{} KG/H", fixed(js_round(v), 0));
    match r.module {
        Module::Crz => {
            let c = ifm::cruise(w, level, mach, isa, factor, eng);
            let rec = if r.engines == 4 { ifm::rec_max_fl(w, mach, isa) } else { ifm::engine_out_ceiling_fl(w, eng, isa) };
            vec![
                s("FUEL FLOW", kg_h(c.fuel_flow_kg_h)),
                s("SPEC RANGE", format!("{} NM/T", fixed(c.specific_range_nm_per_t, 1))),
                s("TAS", format!("{} KT", fixed(js_round(c.tas_kt), 0))),
                s("THRUST/ENG", format!("{} KN", fixed(c.thrust_per_engine_n / 1000.0, 0))),
                s(if r.engines == 4 { "REC MAX" } else { "EO CEILING" }, fl_text(rec)),
                s("OPT FL", if r.engines == 4 { fl_text(ifm::optimum_fl(w, mach, isa)) } else { "---".into() }),
                s("1 ENG OUT CEILING", fl_text(ifm::engine_out_ceiling_fl(w, 3.0, isa))),
                s("2 ENG OUT CEILING", fl_text(ifm::engine_out_ceiling_fl(w, 2.0, isa))),
                s("LIVE FUEL FLOW", kg_h(l.ff_kg_h)),
                s(
                    "LIVE ENDURANCE",
                    match l.fob_kg {
                        Some(fob) if l.ff_kg_h > 500.0 => hhmm((fob / l.ff_kg_h) * 60.0),
                        _ => "---".into(),
                    },
                ),
            ]
        }
        Module::Hld => {
            let h = ifm::holding(w, level, isa, eng);
            let fob = l.fob_kg.unwrap_or(0.0);
            vec![
                s("HOLD SPEED (GD)", format!("{} KT", fixed(js_round(h.kcas), 0))),
                s("FUEL FLOW", kg_h(h.fuel_flow_kg_h * factor)),
                s("FUEL PER 30 MIN", format!("{} T", tons((h.fuel_flow_kg_h * factor) / 2.0))),
                s("MAX HOLD TIME", if fob > 0.0 { hhmm((fob / (h.fuel_flow_kg_h * factor)) * 60.0) } else { "---".into() }),
                s("GREEN DOT FL200", format!("{} KT", fixed(js_round(ifm::green_dot_kcas(w, 200.0)), 0))),
                s("GREEN DOT FL100", format!("{} KT", fixed(js_round(ifm::green_dot_kcas(w, 100.0)), 0))),
            ]
        }
        Module::Des => {
            let d = ifm::descent(level, r.elevation_ft, w, r.wind_kt);
            vec![
                s("DISTANCE", format!("{} NM", fixed(js_round(d.distance_nm), 0))),
                s("TIME", hhmm(d.time_min)),
                s("FUEL", format!("{} T", tons(d.fuel_kg * factor))),
                s(
                    "GRADIENT",
                    format!("{} FT/1000 PER NM", fixed(((level * 100.0 - r.elevation_ft) / d.distance_nm.max(1.0)) / 1000.0, 1)),
                ),
            ]
        }
        Module::Plan => {
            let fob = l.fob_kg.unwrap_or(0.0);
            let p = ifm::plan(w, fob, r.distance_nm, level, mach, isa, r.wind_kt, factor);
            let d_fuel = ifm::descent(level, r.elevation_ft, w - p.trip_fuel_kg, r.wind_kt).fuel_kg * factor;
            vec![
                s("TRIP FUEL", format!("{} T", tons(p.trip_fuel_kg + d_fuel))),
                s("TRIP TIME", hhmm(p.time_min)),
                s("FOB NOW", if fob > 0.0 { format!("{} T", tons(fob)) } else { "---".into() }),
                s("EFOB AT DEST", if fob > 0.0 { format!("{} T", tons(p.efob_kg - d_fuel)) } else { "---".into() }),
                s("LANDING WEIGHT", format!("{} T", tons(w - p.trip_fuel_kg - d_fuel))),
            ]
        }
        Module::Atmo => {
            let atm = ifm::atmosphere(level * 100.0, isa);
            let sgn = |v: f64| if v >= 0.0 { "+" } else { "" };
            vec![
                s("PRESS ALT", l.fl.map_or("---".into(), |f| format!("{} FT", fixed(f * 100.0, 0)))),
                s("SAT", l.sat_c.map_or("---".into(), |v| format!("{}°C", fixed(js_round(v), 0)))),
                s("TAT", l.tat_c.map_or("---".into(), |v| format!("{}°C", fixed(js_round(v), 0)))),
                s("ISA DEV", l.isa_dev.map_or("---".into(), |v| format!("{}{}°C", sgn(v), fixed(js_round(v), 0)))),
                s("WIND", format!("{:0>3}°/{} KT", fixed(js_round(l.wind_dir), 0), fixed(js_round(l.wind_kt), 0))),
                s("GROUND SPEED", format!("{} KT", fixed(js_round(l.gs_kt), 0))),
                s("DENSITY RATIO", fixed(atm.sigma, 3)),
                s("SPEED OF SOUND", format!("{} KT", fixed(js_round(atm.sound_ms / 0.514444), 0))),
            ]
        }
    }
}

fn stepper(label_text: &str, value: &str, live: Option<bool>) -> Node {
    let mut row = div("fr oit-perf-row")
        .style("align-items: center;")
        .child(Node::el("div", "oit-label bigger oit-perf-key").style("width: 230px;").text(label_text))
        .child(button("−", false, "width: 70px; margin-right: 10px;", ""))
        .child(Node::el("div", "oit-label bigger oit-green-text").style("width: 220px; text-align: center;").text(value))
        .child(button("+", false, "width: 70px; margin-right: 20px;", ""));
    row = match live {
        Some(on) => row.child(button_sel("LIVE", false, on, "width: 110px;", "")),
        None => row.child(Node::new("div")),
    };
    row
}

pub fn in_flt_perf_page(m: &InFltModel) -> Node {
    let r = resolve_inputs(m);
    let mut head = div("fr oit-perf-row");
    for md in MODULES {
        head = head.child(button_sel(md.label(), false, md == r.module, "width: 150px; margin-right: 15px;", ""));
    }
    head = head.child(Node::new("div").style("flex-grow: 1"));
    for n in [4u32, 3, 2] {
        head = head.child(button_sel(
            if n == 4 { "AEO" } else if n == 3 { "OEI" } else { "TEO" },
            false,
            r.engines == n,
            "width: 110px; margin-left: 10px;",
            "",
        ));
    }
    let isa_text = format!("ISA {}{}", if r.isa_dev >= 0.0 { "+" } else { "" }, fixed(r.isa_dev, 0));
    let wind_text = format!("{} {} KT", if r.wind_kt >= 0.0 { "TL" } else { "HD" }, fixed(r.wind_kt.abs(), 0));
    let input = box_with("INPUT")
        .child(stepper("WEIGHT", &format!("{} T", tons(r.weight_kg)), Some(r.live_weight)))
        .child(stepper("FLIGHT LEVEL", &fl_text(r.level), Some(r.live_level)))
        .child(stepper("MACH", &format!("M {}", fixed(r.mach, 2)), None))
        .child(stepper("TEMPERATURE", &isa_text, None))
        .child(stepper("FUEL FACTOR", &fixed(r.fuel_factor, 2), None))
        .child(stepper("DISTANCE", &format!("{} NM", fixed(r.distance_nm, 0)), None))
        .child(stepper("WIND", &wind_text, None))
        .child(stepper("DEST ELEVATION", &format!("{} FT", fixed(r.elevation_ft, 0)), None));
    let input_text = format!(
        "{}, FUEL FACTOR {}, X-PLANE FM CAL {}",
        match r.engines {
            4 => "ALL ENGINES",
            3 => "ONE ENGINE INOPERATIVE",
            _ => "TWO ENGINES INOPERATIVE",
        },
        fixed(r.fuel_factor, 2),
        if r.fm.samples > 0 { fixed(r.fm.calibration, 3) } else { "PENDING (STABLE CRUISE NEEDED)".into() }
    );
    let mut rows = div("oit-perf-grid");
    for (k, v) in inflight_rows(m) {
        rows = rows.child(label(&k)).child(green(&v));
    }
    let results = box_with(&format!("{} RESULTS", r.module.label()))
        .child(note("oit-label oit-perf-note", &input_text))
        .child(Node::new("div").child(rows));
    Node::el("div", "oit-page-container oit-perf-page")
        .child(head)
        .child(input)
        .child(results)
        .child(note(
            "oit-label amber oit-perf-note",
            "OIS PERFORMANCE MODEL: DRAG POLAR, THRUST LAPSE AND FUEL FLOW CALIBRATED TO TYPICAL A380 VALUES, FUEL RESULTS SCALED BY THE X-PLANE FM CAL LEARNT IN STABLE CRUISE. NOT AIRBUS-CERTIFIED; FMS PREDICTIONS AND THE QRH PREVAIL.",
        ))
}

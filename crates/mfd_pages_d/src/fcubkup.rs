use crate::w::*;
use mfd_tree::Node;
use serde::Deserialize;

fn polyline(points: &str) -> (&'static str, Vec<(String, String)>) {
    ("polyline", vec![("points".to_string(), points.to_string())])
}

fn rect(x: u32, y: u32, w: u32, h: u32) -> (&'static str, Vec<(String, String)>) {
    (
        "rect",
        vec![
            ("x".to_string(), x.to_string()),
            ("y".to_string(), y.to_string()),
            ("width".to_string(), w.to_string()),
            ("height".to_string(), h.to_string()),
        ],
    )
}

fn lines(shapes: Vec<(&'static str, Vec<(String, String)>)>) -> Node {
    let mut n = Node::new("svg");
    n.cls.push("mfd-fcubkup-lines".into());
    n.svgattr.insert("width".into(), "768".into());
    n.svgattr.insert("height".into(), "780".into());
    for (t, attrs) in shapes {
        let mut a = std::collections::BTreeMap::new();
        for (k, v) in attrs {
            a.insert(k, v);
        }
        a.insert("_d".into(), "0".into());
        a.insert("_display".into(), "inline".into());
        n.shapes.push(mfd_tree::Shape { t: t.to_string(), a });
    }
    n
}

fn light(label: &str, on: bool, label_below: bool) -> Node {
    let bars = Node::el("div", "mfd-fcubkup-bars")
        .class_if("on", on)
        .child(Node::new("div"))
        .child(Node::new("div"))
        .child(Node::new("div"));
    let mut lab = Node::el("div", "mfd-fcubkup-light-label").text(label);
    if label_below {
        lab = lab.style("display: none");
    }
    let mut cap = Node::el("div", "mfd-fcubkup-light-caption").text(label);
    if !label_below {
        cap = cap.style("display: none");
    }
    Node::el("div", "mfd-fcubkup-light-group")
        .class_if("label-below", label_below)
        .child(Node::el("div", "mfd-fcubkup-light").class_if("wide", label_below).child(bars).child(lab))
        .child(cap)
}

fn item(left: u32, top: u32, inner: Node) -> Node {
    Node::el("div", "mfd-fcubkup-item").style(&format!("left: {left}px; top: {top}px;")).child(inner)
}

fn label_at(left: u32, top: u32, width: u32, text: &str) -> Node {
    Node::el("div", "mfd-fcubkup-label").style(&format!("left: {left}px; top: {top}px; width: {width}px;")).text(text)
}

fn window(left: u32, top: u32, width: u32, text: &str, unit: &str) -> Node {
    Node::el("div", "mfd-fcubkup-window")
        .style(&format!("left: {left}px; top: {top}px; width: {width}px;"))
        .child(Node::el("span", "mfd-fcubkup-window-value").text(text))
        .child(Node::el("span", "mfd-fcubkup-window-unit").text(unit))
}

fn pick<'a>(on: bool, when_off: &'a str, when_on: &'a str) -> &'a str {
    if on {
        when_on
    } else {
        when_off
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct FcuBkupAfsModel {
    pub footer: FooterModel,
    pub backup_on: bool,
    pub trk_fpa_mode: bool,
    pub metric: bool,
    pub mach_mode: bool,
    pub true_mode: bool,
    pub spd_text: String,
    pub hdg_text: String,
    pub alt_text: String,
    pub vs_text: String,
    pub loc_light: bool,
    pub alt_light: bool,
    pub appr_light: bool,
    pub fd_light: bool,
    pub ap1_light: bool,
    pub ap2_light: bool,
    pub athr_light: bool,
}

pub fn afs_body(m: &FcuBkupAfsModel) -> Node {
    let b = |label: &str, w: u32| button(&ButtonOpts::new(label).style(&format!("width: {w}px;")));
    let stack = |left: u32, labels: &[&str]| {
        let mut n = Node::el("div", "mfd-fcubkup-stack").style(&format!("left: {left}px;"));
        for l in labels {
            n = n.child(b(l, 150));
        }
        n
    };
    let hdg_label = format!("{} {}", if m.true_mode { "TRUE" } else { "MAG" }, if m.trk_fpa_mode { "TRK" } else { "HDG" });
    let canvas = Node::el("div", "mfd-fcubkup-canvas")
        .child(lines(vec![
            polyline("120,225 120,262"),
            polyline("300,225 300,250 230,250 230,262"),
            polyline("470,80 470,140 300,140 300,175"),
            polyline("620,80 620,262"),
            polyline("520,138 520,262"),
            rect(20, 170, 728, 390),
            rect(190, 590, 390, 185),
        ]))
        .child(item(150, 5, light("AFS CP BACKUP", m.backup_on, true)))
        .child(item(360, 40, b(pick(m.trk_fpa_mode, "HDG-V/S \u{2192} TRK-FPA", "TRK-FPA \u{2192} HDG-V/S"), 230)))
        .child(item(
            470,
            98,
            button(&ButtonOpts::new("METER").style("width: 100px;").selected(m.metric)),
        ))
        .child(item(40, 182, b(pick(m.mach_mode, "SPD \u{2192} MACH", "MACH \u{2192} SPD"), 160)))
        .child(item(220, 182, b(pick(m.true_mode, "MAG \u{2192} TRUE", "TRUE \u{2192} MAG"), 160)))
        .child(label_at(45, 266, 150, pick(m.mach_mode, "SPD", "MACH")))
        .child(label_at(225, 266, 150, &hdg_label))
        .child(label_at(405, 266, 150, "ALT"))
        .child(label_at(575, 266, 150, pick(m.trk_fpa_mode, "V/S", "FPA")))
        .child(window(45, 305, 150, &m.spd_text, pick(m.mach_mode, "KT", "")))
        .child(window(225, 305, 150, &m.hdg_text, "\u{b0}"))
        .child(window(405, 305, 150, &m.alt_text, "FT"))
        .child(window(575, 305, 150, &m.vs_text, pick(m.trk_fpa_mode, "FT/MN", "\u{b0}")))
        .child(stack(45, &["SELECTED", "MANAGED"]))
        .child(stack(225, &["SELECTED", "MANAGED"]))
        .child(stack(405, &["OPEN", "MANAGED"]))
        .child(stack(575, &["SELECTED"]))
        .child(item(260, 470, light("LOC", m.loc_light, false)))
        .child(item(440, 470, light("ALT", m.alt_light, false)))
        .child(item(610, 470, light("APPR", m.appr_light, false)))
        .child(item(345, 600, light("FD", m.fd_light, false)))
        .child(item(465, 612, light("AP2", m.ap2_light, false)))
        .child(item(225, 680, light("AP1", m.ap1_light, false)))
        .child(item(345, 685, light("A/THR", m.athr_light, false)));
    Node::el("div", "mfd-page-container").child(canvas)
}

pub fn afs_page(m: &FcuBkupAfsModel) -> Node {
    navigator("AFS CONTROL PANEL", afs_body(m), vec![], footer(&m.footer))
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct FcuBkupEfisModel {
    pub footer: FooterModel,
    pub captain_side: bool,
    pub backup_on: bool,
    pub baro_text: String,
    pub baro_unit: String,
    pub baro_std: bool,
    pub ls: bool,
    pub vv: bool,
    pub cstr: bool,
    pub filter: u8,
    pub arpt: bool,
    pub overlay: u8,
    pub traf: bool,
}

pub fn efis_body(m: &FcuBkupEfisModel) -> Node {
    let prefix = "CAPT_MFD_fcuBkupEfis";
    let b = |label: &str| button(&ButtonOpts::new(label).style("width: 120px;"));
    let lt = |label: &str, on: bool, left: u32, top: u32| item(left, top, light(label, on, false));
    let canvas = Node::el("div", "mfd-fcubkup-canvas")
        .child(lines(vec![rect(20, 120, 300, 250), rect(340, 120, 408, 250), rect(20, 390, 728, 200), rect(20, 610, 728, 160)]))
        .child(item(40, 5, light("EFIS CP BACKUP", m.backup_on, true)))
        .child(label_at(95, 132, 150, "BARO REF"))
        .child(window(70, 172, 200, &m.baro_text, &m.baro_unit))
        .child(item(45, 245, b("QNH")))
        .child(item(175, 245, button(&ButtonOpts::new("STD").style("width: 120px;").selected(m.baro_std))))
        .child(label_at(360, 132, 120, "ND MODE"))
        .child(item(370, 165, radio_group(&format!("{prefix}Mode"), &["LS", "VOR", "NAV", "ARC", "PLAN"], 0, "green", false)))
        .child(label_at(520, 132, 120, "ND RANGE"))
        .child(
            Node::el("div", "mfd-fcubkup-item mfd-fcubkup-range").style("left: 530px; top: 165px;").child(radio_group(
                &format!("{prefix}Range"),
                &["10", "20", "40", "80", "160", "320", "640"],
                0,
                "green",
                false,
            )),
        )
        .child(lt("LS", m.ls, 60, 410))
        .child(lt("VV", m.vv, 160, 410))
        .child(lt("CSTR", m.cstr, 300, 410))
        .child(lt("WPT", m.filter == 1, 390, 410))
        .child(lt("VOR.D", m.filter == 2, 480, 410))
        .child(lt("NDB", m.filter == 3, 570, 410))
        .child(lt("ARPT", m.arpt, 660, 410))
        .child(lt("WX", m.overlay == 1, 300, 500))
        .child(lt("TERR", m.overlay == 2, 390, 500))
        .child(lt("TRAF", m.traf, 480, 500))
        .child(label_at(60, 625, 200, "ADF/VOR 1"))
        .child(item(90, 660, radio_group(&format!("{prefix}Nav1"), &["VOR", "OFF", "ADF"], 0, "green", false)))
        .child(label_at(480, 625, 200, "ADF/VOR 2"))
        .child(item(510, 660, radio_group(&format!("{prefix}Nav2"), &["VOR", "OFF", "ADF"], 0, "green", false)));
    Node::el("div", "mfd-page-container").child(canvas)
}

pub fn efis_page(m: &FcuBkupEfisModel) -> Node {
    let side = if m.captain_side { "CAPT" } else { "F/O" };
    navigator(&format!("{side} EFIS CONTROL PANEL"), efis_body(m), vec![], footer(&m.footer))
}

use crate::docs::Proc;
use crate::js::el;
use crate::jv::{fx, n};
use crate::Node;
use regex::Regex;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::OnceLock;

macro_rules! re {
    ($s:expr) => {{
        static R: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        R.get_or_init(|| regex::Regex::new($s).unwrap())
    }};
}

mod air;
mod elec;
mod eng;
mod fctl;
mod fuel;
mod hyd;
mod sl;

pub use air::{air_diagram, airflow_names, airflow_nominal, airflow_view, bleed_view, vent_view};
pub use elec::{elec_diagram, elec_view, ElecT};
pub use eng::{eng_faults, engine_svg, EngOpts, FailIn};
pub use fctl::{fctl_detail_view, fctl_plan_view, fctl_targets, fctl_view};
pub use fuel::{fuel_diagram, fuel_view};
pub use hyd::{hyd_diagram, hyd_separation_leaks, hyd_view, HydT};

pub const EX_INK: &str = "#e6e6e6";
pub const EX_DIM: &str = "rgba(230,230,230,0.45)";
pub const EX_HOT: &str = "#e040fb";
pub const EX_FIRE: &str = "#ff4b4b";
pub const EX_GREEN: &str = "#3fd27f";
pub const EX_YELLOW: &str = "#ffd23f";
pub const EX_MONO: &str = "'JetBrains Mono','Cascadia Mono','Consolas',monospace";
pub const EX_NOSE: f64 = -35.21;
pub const EX_LE: [[f64; 2]; 2] = [[4.5, -14.99], [38.8, 10.98]];
pub const EX_TE: [[f64; 2]; 2] = [[4.0, 2.0], [39.5, 15.4]];

pub const NS_BG: &str = "#25272b";
pub const NS_GREEN: &str = "#3fd27f";
pub const NS_ORANGE: &str = "#ffb000";
pub const NS_RED: &str = "#ff3d63";
pub const NS_GREY: &str = "#61656c";
pub const NS_GREY_TEXT: &str = "#8b9097";
pub const NS_FILL: &str = "#1f2226";
pub const NS_WARN_FILL: &str = "#3a2f14";
pub const NS_FAIL_FILL: &str = "#ff3d63";

/// What the diagrams read besides their own input: the breaker list (bus, name), the study failure list, the fire P/B states.
#[derive(Clone, Debug, Default)]
pub struct Ctx {
    pub breakers: Option<Vec<(String, String)>>,
    pub failures: Vec<Value>,
    pub fire_pb: [bool; 4],
    pub live: bool,
}

thread_local! {
    static CTX: RefCell<Ctx> = RefCell::new(Ctx::default());
}

pub fn set_ctx(c: Ctx) {
    CTX.with(|x| *x.borrow_mut() = c);
}

pub(crate) fn ctx<R>(f: impl FnOnce(&Ctx) -> R) -> R {
    CTX.with(|x| f(&x.borrow()))
}

/// An ordered `Map` (JS `Map` iteration order).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OMap(pub Vec<(String, String)>);

impl OMap {
    pub fn get(&self, k: &str) -> Option<&str> {
        self.0.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
    }
    pub fn set(&mut self, k: &str, v: &str) {
        match self.0.iter_mut().find(|(a, _)| a == k) {
            Some(e) => e.1 = v.into(),
            None => self.0.push((k.into(), v.into())),
        }
    }
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(k, _)| k.as_str())
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn st<'a>(&'a self, k: &str, d: &'a str) -> &'a str {
        self.get(k).unwrap_or(d)
    }
}

pub(crate) fn dx() -> &'static Value {
    static D: OnceLock<Value> = OnceLock::new();
    D.get_or_init(|| crate::jv::parse_ordered(include_str!("../dx_data.json")))
}

pub(crate) fn pts(v: &Value) -> Vec<[f64; 2]> {
    v.as_array().map(|a| a.iter().map(|p| [p[0].as_f64().unwrap_or(0.0), p[1].as_f64().unwrap_or(0.0)]).collect()).unwrap_or_default()
}

pub(crate) fn surface(key: &str) -> Option<[f64; 4]> {
    let b = dx()["model"]["surfaces"].get(key)?.as_array()?;
    Some([b[0].as_f64()?, b[1].as_f64()?, b[2].as_f64()?, b[3].as_f64()?])
}

pub(crate) fn ex_part(name: &str) -> Option<[[f64; 3]; 2]> {
    let parts = &dx()["model"]["parts"];
    let (_, v) = crate::jv::entries(parts).into_iter().find(|(k, _)| k.contains(name))?;
    let a = v.as_array()?;
    let row = |r: &Value| -> Option<[f64; 3]> { Some([r[0].as_f64()?, r[1].as_f64()?, r[2].as_f64()?]) };
    Some([row(&a[0])?, row(&a[1])?])
}

pub(crate) fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;")
}

#[derive(Clone, Copy)]
pub(crate) struct T<'a> {
    pub size: f64,
    pub fill: &'a str,
    pub anchor: &'a str,
    pub weight: f64,
    pub rot: f64,
}

pub(crate) const TD: T<'static> = T { size: 13.0, fill: EX_INK, anchor: "middle", weight: 700.0, rot: 0.0 };

pub(crate) fn ex_t(x: f64, y: f64, s: &str, o: T) -> String {
    let rot = if o.rot != 0.0 { format!(" transform=\"rotate({} {} {})\"", n(o.rot), n(x), n(y)) } else { String::new() };
    format!(
        "<text x=\"{}\" y=\"{}\" fill=\"{}\" font-size=\"{}\" font-weight=\"{}\" text-anchor=\"{}\" font-family=\"{}\"{rot}>{}</text>",
        n(x),
        n(y),
        o.fill,
        n(o.size),
        n(o.weight),
        o.anchor,
        EX_MONO,
        esc(s)
    )
}

pub(crate) fn d_path(p: &[[f64; 2]]) -> String {
    p.iter().enumerate().map(|(i, q)| format!("{}{},{}", if i > 0 { "L" } else { "M" }, fx(q[0], 1), fx(q[1], 1))).collect()
}

pub(crate) fn ex_path(p: &[[f64; 2]], fill: &str, stroke: &str, sw: f64, open: bool) -> String {
    format!(
        "<path d=\"{}{}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{}\" stroke-linejoin=\"round\" stroke-linecap=\"round\"/>",
        d_path(p),
        if open { "" } else { "Z" },
        n(sw)
    )
}

pub(crate) fn ex_box(x: f64, y: f64, w: f64, h: f64, label: &str, on: bool, size: f64) -> String {
    let c = if on { EX_HOT } else { "rgba(255,255,255,0.05)" };
    let mut s = format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{c}\" stroke=\"{EX_INK}\" stroke-width=\"1.6\"/>", n(x), n(y), n(w), n(h));
    if !label.is_empty() {
        s += &ex_t(x + w / 2.0, y + h / 2.0 + 5.0, label, T { size, fill: if on { "#fff" } else { EX_INK }, ..TD });
    }
    s
}

pub(crate) fn ex_line(p: &[[f64; 2]], on: bool, stroke: &str, sw: f64) -> String {
    ex_path(p, "none", if on { EX_HOT } else { stroke }, if on { 4.0 } else { sw }, true)
}

pub(crate) fn ex_pump(x: f64, y: f64, on: bool, label: &str) -> String {
    let mut s = format!(
        "<circle cx=\"{}\" cy=\"{}\" r=\"15\" fill=\"{}\" stroke=\"{EX_INK}\" stroke-width=\"1.6\"/><path d=\"M{},{} L{},{} L{},{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.6\"/>",
        n(x),
        n(y),
        if on { EX_HOT } else { "rgba(255,255,255,0.05)" },
        n(x - 8.0),
        n(y + 6.0),
        n(x),
        n(y - 9.0),
        n(x + 8.0),
        n(y + 6.0),
        if on { "#fff" } else { EX_INK }
    );
    if !label.is_empty() {
        s += &ex_t(x, y + 32.0, label, T { size: 11.5, fill: if on { EX_HOT } else { EX_INK }, ..TD });
    }
    s
}

pub(crate) fn ex_valve(x: f64, y: f64, on: bool, label: &str, vertical: bool) -> String {
    let bar = if vertical { format!("M{},{} L{},{}", n(x), n(y - 11.0), n(x), n(y + 11.0)) } else { format!("M{},{} L{},{}", n(x - 11.0), n(y), n(x + 11.0), n(y)) };
    let mut s = format!(
        "<circle cx=\"{}\" cy=\"{}\" r=\"13\" fill=\"{}\" stroke=\"{EX_INK}\" stroke-width=\"1.6\"/><path d=\"{bar}\" stroke=\"{}\" stroke-width=\"2\"/>",
        n(x),
        n(y),
        if on { EX_HOT } else { "#1a1d21" },
        if on { "#fff" } else { EX_INK }
    );
    if !label.is_empty() {
        s += &ex_t(x, y + 30.0, label, T { size: 11.5, fill: if on { EX_HOT } else { EX_INK }, ..TD });
    }
    s
}

pub(crate) fn ex_svg(w: f64, h: f64, parts: &[String]) -> Node {
    crate::js::svg_div("qd-svg", &format!("<svg viewBox=\"0 0 {} {}\" class=\"qd-phases\" font-family=\"{EX_MONO}\">{}</svg>", n(w), n(h), parts.concat()))
}

pub(crate) fn ns_stroke(st: &str) -> &'static str {
    match st {
        "fail" => NS_RED,
        "warn" => NS_ORANGE,
        "live" | "alt" => NS_GREEN,
        _ => NS_GREY,
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct B<'a> {
    pub size: f64,
    pub r: f64,
    pub tag: &'a str,
}

pub(crate) fn ns_box(x: f64, y: f64, w: f64, h: f64, label: &str, st: &str, o: B) -> String {
    let c = ns_stroke(st);
    let fill = match st {
        "fail" => NS_FAIL_FILL,
        "warn" => NS_WARN_FILL,
        _ => NS_FILL,
    };
    let txt = match st {
        "fail" => "#fff",
        "off" => NS_GREY_TEXT,
        _ => c,
    };
    let lines: Vec<&str> = label.split('\n').collect();
    let size = if o.size != 0.0 { o.size } else { 13.0 };
    let ty = y + h / 2.0 + size * 0.36 - ((lines.len() as f64 - 1.0) * (size + 2.0)) / 2.0;
    let mut s = format!(
        "<path d=\"{}\" fill=\"{fill}\" stroke=\"{c}\" stroke-width=\"{}\"/>",
        crate::ground::squircle(x, y, w, h, if o.r != 0.0 { o.r } else { 9.0 }),
        if st == "off" { "1.4" } else { "2.2" }
    );
    for (i, l) in lines.iter().enumerate() {
        s += &ex_t(x + w / 2.0, ty + i as f64 * (size + 2.0), l, T { size, fill: txt, ..TD });
    }
    if !o.tag.is_empty() {
        s += &ex_t(x + w / 2.0, y - 5.0, o.tag, T { size: 10.0, fill: c, ..TD });
    }
    s
}

pub(crate) fn ns_wire(p: &[[f64; 2]], st: &str, dash: &str, arrow: bool) -> String {
    format!(
        "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\"{}{}/>",
        d_path(p),
        ns_stroke(st),
        if st == "off" { "1.4" } else { "2.6" },
        if dash.is_empty() { String::new() } else { format!(" stroke-dasharray=\"{dash}\"") },
        if st == "alt" && arrow { " marker-end=\"url(#nsArrow)\"" } else { "" }
    )
}

pub(crate) fn wire(p: &[[f64; 2]], st: &str) -> String {
    ns_wire(p, st, "", true)
}

pub(crate) fn wire_d(p: &[[f64; 2]], st: &str, dash: &str) -> String {
    ns_wire(p, st, dash, true)
}

pub(crate) fn ns_elbow(a: [f64; 2], b: [f64; 2], st: &str, dash: &str) -> String {
    let ym = (a[1] + b[1]) / 2.0;
    ns_wire(&[a, [a[0], ym], [b[0], ym], b], st, dash, true)
}

pub(crate) fn ns_svg(w: f64, h: f64, parts: &[String], focus: Option<&[f64]>) -> Node {
    let defs = format!(
        "<defs><marker id=\"nsArrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\"><path d=\"M0,0 L10,5 L0,10 z\" fill=\"{NS_GREEN}\"/></marker></defs>"
    );
    let mut all = vec![defs, format!("<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" rx=\"10\" fill=\"{NS_BG}\"/>", n(w), n(h))];
    all.extend(parts.iter().cloned());
    let mut d = ex_svg(w, h, &all);
    let Some(f) = focus else { return d };
    let pad = if f.len() > 4 { f[4] } else { 40.0 };
    let (mut x, mut y, mut fw, mut fh) = (f[0] - pad, f[1] - pad, f[2] + pad * 2.0, f[3] + pad * 2.0);
    let min_w = if f.len() > 4 { 520f64.min(f[2] + pad * 2.0) } else { w.min(720.0) };
    let min_h = h.min(380.0);
    if fw < min_w {
        x -= (min_w - fw) / 2.0;
        fw = min_w;
    }
    if fh < min_h {
        y -= (min_h - fh) / 2.0;
        fh = min_h;
    }
    x = x.min(w - fw).max(0.0);
    y = y.min(h - fh).max(0.0);
    let crop = format!("{} {} {} {}", fx(x, 0), fx(y, 0), fx(fw.min(w), 0), fx(fh.min(h), 0));
    if let Some(mfd_tree::Kid::Node(svg)) = d.kids.first_mut() {
        svg.svgattr.insert("viewBox".into(), crop.clone());
        svg.data.insert("full".into(), format!("0 0 {} {}", n(w), n(h)));
    }
    d.cls.push("ns-crop".into());
    d.attr("title", "Tap for the full diagram").act(&format!("svg:crop:{crop}"))
}

pub(crate) fn ns_bounds(rects: &[[f64; 4]]) -> Option<Vec<f64>> {
    if rects.is_empty() {
        return None;
    }
    let x0 = rects.iter().map(|r| r[0]).fold(f64::INFINITY, f64::min);
    let y0 = rects.iter().map(|r| r[1]).fold(f64::INFINITY, f64::min);
    let x1 = rects.iter().map(|r| r[0] + r[2]).fold(f64::NEG_INFINITY, f64::max);
    let y1 = rects.iter().map(|r| r[1] + r[3]).fold(f64::NEG_INFINITY, f64::max);
    Some(vec![x0, y0, x1 - x0, y1 - y0])
}

pub(crate) fn ns_valve(x: f64, y: f64, st: &str, open: bool, label: &str, left: bool) -> String {
    let c = ns_stroke(st);
    let bar = if open { format!("M{},{} L{},{}", n(x), n(y - 7.0), n(x), n(y + 7.0)) } else { format!("M{},{} L{},{}", n(x - 7.0), n(y), n(x + 7.0), n(y)) };
    let mut s = format!(
        "<circle cx=\"{}\" cy=\"{}\" r=\"9\" fill=\"{}\" stroke=\"{c}\" stroke-width=\"2\"/><path d=\"{bar}\" stroke=\"{}\" stroke-width=\"2.2\"/>",
        n(x),
        n(y),
        if st == "fail" { NS_RED } else { NS_FILL },
        if st == "fail" { "#fff" } else { c }
    );
    if !label.is_empty() {
        s += &ex_t(
            if left { x - 13.0 } else { x + 13.0 },
            y + 4.0,
            label,
            T { size: 9.0, anchor: if left { "end" } else { "start" }, fill: if st == "live" || st == "off" { NS_GREY_TEXT } else { c }, ..TD },
        );
    }
    s
}

fn plan_p(x: f64, z: f64) -> [f64; 2] {
    [490.0 + x * 7.9, 34.0 + (z + 35.4) * 7.9]
}

pub fn ex_plan_view(hl: &HashSet<String>, fire: bool) -> Node {
    let (w, h) = (980.0, 640.0);
    let p = plan_p;
    let hot = if fire { EX_FIRE } else { EX_HOT };
    let has = |k: &str| hl.contains(k);
    let mut parts = vec![ex_path(&pts(&dx()["sil"]["top"]).iter().map(|q| p(q[0], q[1])).collect::<Vec<_>>(), "rgba(255,255,255,0.03)", EX_INK, 1.6, false)];
    let mut engs: Vec<(Vec<[f64; 2]>, f64)> = dx()["parts"]["top_engines"]
        .as_array()
        .map(|a| a.iter().map(|l| {
            let lp = pts(l);
            let cx = lp.iter().fold(0.0, |a, q| a + q[0]) / lp.len() as f64;
            (lp, cx)
        }).collect())
        .unwrap_or_default();
    engs.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    for (i, (lp, cx)) in engs.iter().enumerate() {
        let on = has(&format!("ENG{}", i + 1));
        parts.push(ex_path(&lp.iter().map(|q| p(q[0], q[1])).collect::<Vec<_>>(), if on { hot } else { "rgba(255,255,255,0.04)" }, EX_INK, 1.6, false));
        let [lx, ly] = p(*cx, -16.0);
        parts.push(ex_t(lx, ly, &(i + 1).to_string(), T { size: 15.0, fill: if on { hot } else { EX_INK }, ..TD }));
    }
    let rect = |key: &str, side: &str, on: bool| -> String {
        let Some(b) = surface(key) else { return String::new() };
        let k = if side == "R" { 1.0 } else { -1.0 };
        let [ax, ay] = p(k * b[0].abs(), b[2]);
        let [bx, by] = p(k * b[1].abs(), b[3]);
        format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.2\"/>",
            n(ax.min(bx)),
            n(ay.min(by)),
            n((bx - ax).abs()),
            n((by - ay).abs()),
            if on { hot } else { "rgba(255,255,255,0.04)" },
            if on { hot } else { EX_DIM }
        )
    };
    for side in ["L", "R"] {
        for k in 1..=8 {
            parts.push(rect(&format!("l_spoiler{k}_key"), side, has(&format!("SPLR{side}{k}")) || has(&format!("SPLR{k}")) || has("SPLRALL")));
        }
        for (i, part) in ["INR", "MID", "OUTR"].iter().enumerate() {
            parts.push(rect(&format!("l_aileron{}_percent_key", i + 1), side, has(&format!("AIL{side}{part}")) || has(&format!("AIL{part}")) || has("AILALL") || has(&format!("AIL{side}"))));
        }
        parts.push(rect("l_inner_elevator_deflection", side, has(&format!("ELEV{side}INR")) || has("ELEVALL") || has(&format!("ELEV{side}"))));
        parts.push(rect("l_outer_elevator_deflection", side, has(&format!("ELEV{side}OUTR")) || has("ELEVALL") || has(&format!("ELEV{side}"))));
        let k = if side == "R" { 1.0 } else { -1.0 };
        let le: Vec<[f64; 2]> = EX_LE.iter().map(|q| p(k * q[0], q[1])).collect();
        parts.push(ex_line(&le, has("SLAT") || has(&format!("SLAT{side}")), "rgba(0,0,0,0)", 2.0));
        let te: Vec<[f64; 2]> = [[4.0, 2.0], [26.0, 9.3]].iter().map(|q: &[f64; 2]| p(k * q[0], q[1] + 0.6)).collect();
        parts.push(ex_line(&te, has("FLAP") || has(&format!("FLAP{side}")), "rgba(0,0,0,0)", 2.0));
    }
    let [rx, ry] = p(0.0, 30.2);
    let [_, ry2] = p(0.0, 37.8);
    let rud = has("RUDUPPER") || has("RUDLOWER") || has("RUD");
    parts.push(format!(
        "<rect x=\"{}\" y=\"{}\" width=\"7\" height=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.2\"/>",
        n(rx - 3.5),
        n(ry),
        n(ry2 - ry),
        if rud { hot } else { "rgba(255,255,255,0.1)" },
        if rud { hot } else { EX_DIM }
    ));
    if has("THS") {
        let [a, b] = p(-15.0, 32.0);
        let [c, d] = p(15.0, 34.5);
        parts.push(format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"{hot}\" stroke-width=\"3\"/>", n(a), n(b), n(c - a), n(d - b)));
    }
    let apu = has("APU");
    let [ax, ay] = p(0.0, 36.6);
    parts.push(format!(
        "<rect x=\"{}\" y=\"{}\" width=\"18\" height=\"16\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.2\"/>",
        n(ax - 9.0),
        n(ay - 12.0),
        if apu { hot } else { "rgba(255,255,255,0.06)" },
        if apu { hot } else { EX_DIM }
    ));
    if apu {
        parts.push(ex_t(ax + 40.0, ay, "APU", T { fill: hot, ..TD }));
    }
    let gear: [(&str, f64, f64, usize, usize); 5] = [("NLG", 0.0, -29.4, 1, 2), ("WLGL", -5.95, -1.1, 2, 2), ("WLGR", 5.95, -1.1, 2, 2), ("BLGL", -2.85, 2.6, 2, 3), ("BLGR", 2.85, 2.6, 2, 3)];
    for (id, x, z, cols, rows) in gear {
        let on = has(id) || has(&id[..3]) || has("GEAR") || (!id.starts_with('N') && has("MLG"));
        for c in 0..cols {
            for r in 0..rows {
                let [gx, gy] = p(x + (c as f64 - (cols as f64 - 1.0) / 2.0) * 0.9, z + (r as f64 - (rows as f64 - 1.0) / 2.0) * 1.7);
                parts.push(format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"6\" height=\"11\" rx=\"1.5\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1\"/>",
                    n(gx - 3.0),
                    n(gy - 5.5),
                    if on { hot } else { "rgba(255,255,255,0.15)" },
                    if on { hot } else { EX_DIM }
                ));
            }
        }
    }
    if has("LGL") || has("LGR") {
        for side in ["L", "R"] {
            if has(&format!("LG{side}")) {
                for g in gear.iter().filter(|g| g.0.ends_with(side)) {
                    let [gx, gy] = p(g.1, g.2);
                    parts.push(format!("<circle cx=\"{}\" cy=\"{}\" r=\"18\" fill=\"none\" stroke=\"{hot}\" stroke-width=\"2.5\"/>", n(gx), n(gy)));
                }
            }
        }
    }
    parts.push(ex_t(14.0, 22.0, "L", T { anchor: "start", size: 15.0, ..TD }));
    parts.push(ex_t(w - 14.0, 22.0, "R", T { anchor: "end", size: 15.0, ..TD }));
    ex_svg(w, h, &parts)
}

const EX_DOORS: [(&str, &str); 8] = [("M1", "ANIM_DOOR_M1L"), ("M2", "ANIM_DOOR_M2L"), ("M3", "ANIM_DOOR_M3L"), ("M4", "ANIM_DOOR_M4L"), ("M5", "ANIM_DOOR_M5L"), ("U1", "U1L"), ("U2", "U2L"), ("U3", "U3L")];

fn side_p(z: f64, y: f64) -> [f64; 2] {
    [26.0 + (z + 35.4) * 12.6, 40.0 + (20.4 - y) * 12.6]
}

pub fn ex_side_view(hl: &HashSet<String>, smoke: bool) -> Node {
    let (w, h) = (980.0, 380.0);
    let p = side_p;
    let hot = if smoke { EX_FIRE } else { EX_HOT };
    let has = |k: &str| hl.contains(k);
    let mut parts = vec![ex_path(&pts(&dx()["sil"]["side"]).iter().map(|q| p(q[0], q[1])).collect::<Vec<_>>(), "rgba(255,255,255,0.03)", EX_INK, 1.6, false)];
    for (nm, key) in EX_DOORS {
        let Some(b) = ex_part(key) else { continue };
        let on = has(&format!("D{nm}")) || has("DALL") || (nm.starts_with('M') && has("DMAIN")) || (nm.starts_with('U') && has("DUPPER"));
        let [ax, ay] = p(b[0][2], b[1][1]);
        let [bx, by] = p(b[1][2], b[0][1]);
        parts.push(format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"5\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.6\"/>",
            n(ax),
            n(ay),
            n(bx - ax),
            n(by - ay),
            if on { hot } else { "rgba(255,255,255,0.04)" },
            if on { hot } else { EX_INK }
        ));
        parts.push(ex_t((ax + bx) / 2.0, if nm.starts_with('U') { ay - 10.0 } else { by + 22.0 }, nm, T { size: 13.0, fill: if on { hot } else { EX_INK }, ..TD }));
    }
    let mut zone = |id: &str, z0: f64, z1: f64, label: &str| {
        let on = has(id);
        let [a, t] = p(EX_NOSE + z0, 0.6);
        let [b, bt] = p(EX_NOSE + z1, -2.4);
        parts.push(format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" fill-opacity=\"{}\" stroke=\"{}\" stroke-width=\"1.4\" stroke-dasharray=\"{}\"/>",
            n(a),
            n(t),
            n(b - a),
            n(bt - t),
            if on { hot } else { "none" },
            if on { "0.85" } else { "0" },
            if on { hot } else { EX_DIM },
            if on { "" } else { "6 4" }
        ));
        parts.push(ex_t((a + b) / 2.0, bt + 40.0, label, T { size: 12.0, fill: if on { hot } else { EX_DIM }, ..TD }));
    };
    zone("CFWD", 10.5, 27.9, "FWD CARGO");
    zone("CAFT", 37.0, 46.7, "AFT CARGO");
    zone("CBULK", 47.3, 50.1, "BULK");
    let av = has("AVNCS");
    let [a1, t1] = p(EX_NOSE + 4.0, 0.8);
    let [b1, bt1] = p(EX_NOSE + 9.5, -2.2);
    parts.push(format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.4\" stroke-dasharray=\"{}\"/>",
        n(a1),
        n(t1),
        n(b1 - a1),
        n(bt1 - t1),
        if av { hot } else { "none" },
        if av { hot } else { EX_DIM },
        if av { "" } else { "6 4" }
    ));
    parts.push(ex_t((a1 + b1) / 2.0, bt1 + 40.0, "AVNCS", T { size: 12.0, fill: if av { hot } else { EX_DIM }, ..TD }));
    ex_svg(w, h, &parts)
}

fn ex_hyd(hl: &HashSet<String>) -> Node {
    let mut parts = Vec::new();
    let has = |k: &str| hl.contains(k);
    for (x, c, name, engs) in [(40.0, EX_GREEN, "G", [1, 2]), (530.0, EX_YELLOW, "Y", [3, 4])] {
        let on = has(name);
        parts.push(ex_t(x + 200.0, 34.0, if name == "G" { "GREEN" } else { "YELLOW" }, T { size: 17.0, fill: c, ..TD }));
        for (i, e) in engs.iter().enumerate() {
            for (j, ab) in ["A", "B"].iter().enumerate() {
                let px = x + 40.0 + i as f64 * 180.0 + j as f64 * 70.0;
                parts.push(ex_pump(px, 110.0, has(&format!("EDP{e}{ab}")), &format!("{e}{ab}")));
                parts.push(ex_line(&[[px, 125.0], [px, 230.0]], false, c, 2.0));
            }
            parts.push(ex_t(x + 75.0 + i as f64 * 180.0, 64.0, &format!("ENG {e}"), T { size: 13.0, ..TD }));
        }
        for (j, ab) in ["A", "B"].iter().enumerate() {
            let px = x + 320.0 + j as f64 * 60.0;
            parts.push(ex_pump(px, 300.0, has(&format!("EP{name}{ab}")), &format!("ELEC {ab}")));
            parts.push(ex_line(&[[px, 285.0], [px, 230.0]], false, c, 2.0));
        }
        parts.push(ex_line(&[[x + 20.0, 230.0], [x + 400.0, 230.0]], on, c, 5.0));
        parts.push(ex_box(x + 40.0, 270.0, 120.0, 46.0, "RSVR", has(&format!("RSVR{name}")), 13.0));
        parts.push(ex_t(x + 210.0, 390.0, "5000 PSI", T { size: 13.0, fill: c, ..TD }));
    }
    ex_svg(980.0, 430.0, &parts)
}

fn ex_elec(hl: &HashSet<String>) -> Node {
    let mut parts = Vec::new();
    let has = |k: &str| hl.contains(k);
    for i in 1..=4 {
        let x = 30.0 + (i - 1) as f64 * 240.0;
        parts.push(ex_box(x, 30.0, 100.0, 44.0, &format!("GEN {i}"), has(&format!("GEN{i}")), 13.0));
        parts.push(ex_box(x + 112.0, 30.0, 92.0, 44.0, &format!("EXT {i}"), has(&format!("EXT{i}")), 13.0));
        parts.push(ex_line(&[[x + 50.0, 74.0], [x + 50.0, 130.0]], false, EX_INK, 2.0));
        parts.push(ex_line(&[[x + 158.0, 74.0], [x + 158.0, 110.0], [x + 50.0, 110.0]], false, EX_INK, 2.0));
        parts.push(ex_box(x, 130.0, 204.0, 44.0, &format!("AC {i}"), has(&format!("AC{i}")), 13.0));
    }
    parts.push(ex_box(300.0, 210.0, 140.0, 44.0, "APU GEN A", has("APUGENA"), 13.0));
    parts.push(ex_box(540.0, 210.0, 140.0, 44.0, "APU GEN B", has("APUGENB"), 13.0));
    parts.push(ex_line(&[[30.0, 190.0], [950.0, 190.0]], has("BUSTIE"), EX_INK, 2.0));
    parts.push(ex_box(60.0, 280.0, 180.0, 44.0, "AC ESS", has("ACESS"), 13.0));
    parts.push(ex_box(400.0, 280.0, 180.0, 44.0, "AC EHA", has("ACEHA"), 13.0));
    parts.push(ex_box(740.0, 280.0, 180.0, 44.0, "EMER GEN", has("RAT"), 13.0));
    let rows = [("TR 1", "DC 1", "TR1", "DC1"), ("TR 2", "DC 2", "TR2", "DC2"), ("ESS TR", "DC ESS", "TRESS", "DCESS"), ("APU TR", "DC APU", "TRAPU", "DCAPU")];
    for (i, (tr, dc, a, b)) in rows.iter().enumerate() {
        let x = 30.0 + i as f64 * 240.0;
        parts.push(ex_box(x, 350.0, 96.0, 40.0, tr, has(a), 13.0));
        parts.push(ex_box(x + 108.0, 350.0, 96.0, 40.0, dc, has(b), 13.0));
        parts.push(ex_line(&[[x + 96.0, 370.0], [x + 108.0, 370.0]], false, EX_INK, 2.0));
        let bat = ["BAT 1", "BAT 2", "ESS BAT", "APU BAT"][i];
        let bid = ["BAT1", "BAT2", "BATESS", "BATAPU"][i];
        parts.push(ex_box(x + 108.0, 410.0, 96.0, 36.0, bat, has(bid), 12.0));
    }
    ex_svg(980.0, 470.0, &parts)
}

fn ex_fuel(hl: &HashSet<String>) -> Node {
    let mut parts = Vec::new();
    let has = |k: &str| hl.contains(k);
    let any = |id: &str| has(id) || has(id.strip_suffix('L').or(id.strip_suffix('R')).unwrap_or(id));
    let tanks = [("OUTR", "OUTRL"), ("FEED 1", "FEED1"), ("MID", "MIDL"), ("INR", "INRL"), ("FEED 2", "FEED2"), ("FEED 3", "FEED3"), ("INR", "INRR"), ("MID", "MIDR"), ("FEED 4", "FEED4"), ("OUTR", "OUTRR")];
    for (i, (name, id)) in tanks.iter().enumerate() {
        let x = 18.0 + i as f64 * 95.0 + if i >= 5 { 14.0 } else { 0.0 };
        let y = 170.0 - (i as f64 - 4.5).abs() * 14.0;
        parts.push(ex_box(x, y, 84.0, 92.0, "", any(id), 13.0));
        parts.push(ex_t(x + 42.0, y + 38.0, name, T { size: 12.5, fill: if any(id) { "#fff" } else { EX_INK }, ..TD }));
        if let Some(nn) = name.strip_prefix("FEED ") {
            parts.push(ex_pump(x + 26.0, y + 70.0, has(&format!("PUMPMAIN{nn}")), ""));
            parts.push(ex_pump(x + 60.0, y + 70.0, has(&format!("PUMPSTBY{nn}")), ""));
            parts.push(ex_line(&[[x + 42.0, y], [x + 42.0, 60.0]], has(&format!("ENGFEED{nn}")), EX_INK, 2.0));
            parts.push(ex_box(x + 6.0, 24.0, 72.0, 36.0, &format!("ENG {nn}"), has(&format!("ENGFEED{nn}")), 12.5));
            parts.push(ex_valve(x + 42.0, y - 36.0, has(&format!("XFEED{nn}")), "", true));
        }
    }
    parts.push(ex_line(&[[60.0, 300.0], [920.0, 300.0]], has("XFR"), EX_INK, 2.0));
    parts.push(ex_box(410.0, 340.0, 160.0, 70.0, "TRIM", any("TRIM"), 13.0));
    parts.push(ex_line(&[[490.0, 300.0], [490.0, 340.0]], has("TRIM"), EX_INK, 2.0));
    parts.push(ex_box(780.0, 360.0, 120.0, 44.0, "APU", has("APUFEED"), 12.5));
    parts.push(ex_box(70.0, 360.0, 150.0, 44.0, "JETTISON", has("JETT"), 12.5));
    ex_svg(980.0, 460.0, &parts)
}

fn ex_bleed(hl: &HashSet<String>) -> Node {
    let mut parts = Vec::new();
    let has = |k: &str| hl.contains(k);
    for i in 1..=4 {
        let x = 80.0 + (i - 1) as f64 * 230.0;
        let b = has(&format!("BLEED{i}"));
        parts.push(ex_box(x - 50.0, 24.0, 100.0, 46.0, &format!("ENG {i}"), has(&format!("ENG{i}")) && !b, 13.0));
        parts.push(ex_valve(x, 120.0, b, "", true));
        parts.push(ex_line(&[[x, 70.0], [x, 107.0]], b, EX_INK, 2.0));
        parts.push(ex_line(&[[x, 133.0], [x, 200.0]], b, EX_INK, 2.0));
        parts.push(ex_t(x + 30.0, 125.0, "BLEED", T { size: 11.5, anchor: "start", fill: if b { EX_HOT } else { EX_DIM }, ..TD }));
    }
    parts.push(ex_line(&[[80.0, 200.0], [770.0, 200.0]], has("DUCT"), EX_INK, 2.4));
    for (x, nm, id) in [(195.0, "XBLEED L", "XBLEEDL"), (425.0, "XBLEED C", "XBLEEDC"), (655.0, "XBLEED R", "XBLEEDR")] {
        parts.push(ex_valve(x, 200.0, has(id), nm, false));
    }
    parts.push(ex_box(375.0, 260.0, 100.0, 44.0, "APU", has("APUBLEED"), 13.0));
    parts.push(ex_line(&[[425.0, 260.0], [425.0, 213.0]], has("APUBLEED"), EX_INK, 2.0));
    for (x, nm, id) in [(160.0, "PACK 1", "PACK1"), (690.0, "PACK 2", "PACK2")] {
        parts.push(ex_line(&[[x + 40.0, 200.0], [x + 40.0, 330.0]], has(id), EX_INK, 2.0));
        parts.push(ex_box(x, 330.0, 80.0, 44.0, nm, has(id), 12.5));
    }
    parts.push(ex_box(860.0, 300.0, 100.0, 44.0, "HOT AIR", has("HOTAIR"), 12.0));
    ex_svg(980.0, 440.0, &parts)
}

fn all<'a>(r: &Regex, t: &'a str) -> Vec<regex::Captures<'a>> {
    r.captures_iter(t).collect()
}

fn g<'a>(c: &'a regex::Captures, i: usize) -> &'a str {
    c.get(i).map(|m| m.as_str()).unwrap_or("")
}

pub fn ex_highlights(title: &str, sys: &str) -> HashSet<String> {
    let up = title.to_uppercase();
    let t = format!(" {} ", re!(r"[^A-Z0-9/+ ]").replace_all(&up, " "));
    let t = t.as_str();
    let s = sys.to_uppercase();
    let mut hl: HashSet<String> = HashSet::new();
    let test = |r: &Regex| r.is_match(t);
    for m in all(re!(r"ENG (\d)(?:\s*\+\s*(\d))?(?:\s*\+\s*(\d))?(?:\s*\+\s*(\d))?"), t) {
        for i in 1..=4 {
            if let Some(x) = m.get(i) {
                hl.insert(format!("ENG{}", x.as_str()));
            }
        }
    }
    if test(re!(r" ENG 1\+2\+3\+4 | ALL ENG")) {
        for k in 1..=4 {
            hl.insert(format!("ENG{k}"));
        }
    }
    if test(re!(r"\bAPU\b")) {
        hl.insert("APU".into());
    }
    for m in all(re!(r"(?:EDP|ENG)\s*(\d)\s*PUMP\s*([AB])"), t) {
        hl.insert(format!("EDP{}{}", g(&m, 1), g(&m, 2)));
    }
    for m in all(re!(r"ENG (\d) PUMP ([AB])"), t) {
        hl.insert(format!("EDP{}{}", g(&m, 1), g(&m, 2)));
    }
    for m in all(re!(r"\b(G|Y|GREEN|YELLOW)\b"), t) {
        hl.insert(g(&m, 1)[..1].to_string());
    }
    for m in all(re!(r"ELEC PUMP ([AB])"), t) {
        if hl.contains("G") {
            hl.insert(format!("EPG{}", g(&m, 1)));
        }
        if hl.contains("Y") {
            hl.insert(format!("EPY{}", g(&m, 1)));
        }
    }
    if t.contains("RSVR") {
        if hl.contains("G") {
            hl.insert("RSVRG".into());
        }
        if hl.contains("Y") {
            hl.insert("RSVRY".into());
        }
    }
    let add = |r: &Regex, p: &str, hl: &mut HashSet<String>| {
        for m in all(r, t) {
            hl.insert(format!("{p}{}", g(&m, 1)));
        }
    };
    add(re!(r"(?:GEN|VFG)\s*(\d)"), "GEN", &mut hl);
    add(re!(r"APU GEN ([AB])"), "APUGEN", &mut hl);
    add(re!(r"EXT (?:PWR )?(\d)"), "EXT", &mut hl);
    add(re!(r"AC (?:BUS )?(\d)"), "AC", &mut hl);
    let flag = |cond: bool, k: &str, hl: &mut HashSet<String>| {
        if cond {
            hl.insert(k.into());
        }
    };
    flag(t.contains("AC ESS"), "ACESS", &mut hl);
    flag(t.contains("AC EHA") || t.contains("247XP"), "ACEHA", &mut hl);
    add(re!(r"DC (?:BUS )?(\d)"), "DC", &mut hl);
    flag(t.contains("DC ESS"), "DCESS", &mut hl);
    flag(t.contains("DC APU"), "DCAPU", &mut hl);
    add(re!(r"TR (\d)"), "TR", &mut hl);
    flag(t.contains("ESS TR"), "TRESS", &mut hl);
    flag(t.contains("APU TR"), "TRAPU", &mut hl);
    add(re!(r"BAT (\d)"), "BAT", &mut hl);
    flag(t.contains("ESS BAT"), "BATESS", &mut hl);
    flag(t.contains("APU BAT"), "BATAPU", &mut hl);
    flag(t.contains("RAT") || t.contains("EMER GEN") || t.contains("EMER CONFIG"), "RAT", &mut hl);
    flag(t.contains("BUS TIE"), "BUSTIE", &mut hl);
    add(re!(r"FEED (?:TK )?(\d)"), "FEED", &mut hl);
    add(re!(r"COLLECTOR CELL (\d)"), "FEED", &mut hl);
    for m in all(re!(r"(?:L|LEFT) (OUTR|MID|INR)"), t) {
        hl.insert(format!("{}L", g(&m, 1)));
    }
    for m in all(re!(r"(?:R|RIGHT) (OUTR|MID|INR)"), t) {
        hl.insert(format!("{}R", g(&m, 1)));
    }
    for m in all(re!(r" (OUTR|MID|INR) TK"), t) {
        hl.insert(format!("{}L", g(&m, 1)));
        hl.insert(format!("{}R", g(&m, 1)));
    }
    flag(t.contains("TRIM TK") || t.contains("TRIM TANK"), "TRIM", &mut hl);
    for m in all(re!(r"(MAIN|STBY) PUMP"), t) {
        for k in 1..=4 {
            if hl.contains(&format!("FEED{k}")) {
                hl.insert(format!("PUMP{}{k}", g(&m, 1)));
            }
        }
    }
    add(re!(r"XFEED (\d)"), "XFEED", &mut hl);
    flag(t.contains("XFR"), "XFR", &mut hl);
    flag(t.contains("JETT"), "JETT", &mut hl);
    flag(re!(r"APU .*FEED|APU FUEL").is_match(t), "APUFEED", &mut hl);
    add(re!(r"BLEED (\d)"), "BLEED", &mut hl);
    add(re!(r"ENG (\d) BLEED"), "BLEED", &mut hl);
    for m in all(re!(r"PACK (\d)(?:\+(\d))?"), t) {
        hl.insert(format!("PACK{}", g(&m, 1)));
        if m.get(2).is_some() {
            hl.insert(format!("PACK{}", g(&m, 2)));
        }
    }
    if re!(r"XBLEED|X BLEED|CROSSBLEED").is_match(t) {
        for x in ["XBLEEDL", "XBLEEDC", "XBLEEDR"] {
            hl.insert(x.into());
        }
    }
    flag(t.contains("APU BLEED"), "APUBLEED", &mut hl);
    flag(t.contains("HOT AIR"), "HOTAIR", &mut hl);
    flag(t.contains("DUCT") || t.contains("LEAK"), "DUCT", &mut hl);
    add(re!(r"(?:SPLR|SPOILER)S? (\d)"), "SPLR", &mut hl);
    flag(re!(r"SPLRS|SPOILERS|SPD BRK|GND SPLR").is_match(t), "SPLRALL", &mut hl);
    for m in all(re!(r"\b([LR]) (INR|MID|OUTR) AIL"), t) {
        hl.insert(format!("AIL{}{}", g(&m, 1), g(&m, 2)));
    }
    add(re!(r"\b([LR]) AIL"), "AIL", &mut hl);
    if re!(r"AILERONS?|AIL ").is_match(t) && !hl.iter().any(|x| x.starts_with("AIL")) {
        hl.insert("AILALL".into());
    }
    for m in all(re!(r"\b([LR]) (INR|OUTR) ELEV"), t) {
        hl.insert(format!("ELEV{}{}", g(&m, 1), g(&m, 2)));
    }
    add(re!(r"\b([LR]) ELEV"), "ELEV", &mut hl);
    if t.contains("ELEV") && !hl.iter().any(|x| x.starts_with("ELEV")) {
        hl.insert("ELEVALL".into());
    }
    flag(t.contains("UPPER RUD") || t.contains("UPR RUD"), "RUDUPPER", &mut hl);
    flag(t.contains("LOWER RUD") || t.contains("LWR RUD"), "RUDLOWER", &mut hl);
    flag(t.contains("RUD"), "RUD", &mut hl);
    flag(t.contains("SLAT"), "SLAT", &mut hl);
    flag(t.contains("FLAP"), "FLAP", &mut hl);
    flag(re!(r"PITCH TRIM|THS|STAB").is_match(t), "THS", &mut hl);
    flag(re!(r"N/W|NOSE (?:L/G|GEAR|WHEEL)|NLG").is_match(t), "NLG", &mut hl);
    let tg = t.replace("L/G", "GEAR");
    let side_of = |a: &str, b: &str| -> Vec<String> {
        if re!(r"RIGHT|\bR\b|\bRH\b").is_match(&tg) {
            vec![b.into()]
        } else if re!(r"LEFT|\bL\b|\bLH\b").is_match(&tg) {
            vec![a.into()]
        } else {
            vec![a.into(), b.into()]
        }
    };
    if re!(r"BODY GEAR|BLG").is_match(&tg) {
        hl.extend(side_of("BLGL", "BLGR"));
    }
    if re!(r"WING GEAR|WLG").is_match(&tg) {
        hl.extend(side_of("WLGL", "WLGR"));
    }
    if re!(r"L/G|GEAR|BRAKES|WHEEL|STEER|MLG").is_match(t) && !hl.iter().any(|x| x.contains("LG")) {
        hl.insert(if s == "STEER" { "NLG" } else { "GEAR" }.into());
    }
    for m in all(re!(r"(?:DOOR )?(MAIN|UPPER|UPR) (\d)([LR])?"), t) {
        hl.insert(format!("D{}{}", if g(&m, 1) == "MAIN" { "M" } else { "U" }, g(&m, 2)));
    }
    flag(t.contains("FWD CARGO"), "CFWD", &mut hl);
    flag(t.contains("AFT CARGO"), "CAFT", &mut hl);
    flag(t.contains("BULK"), "CBULK", &mut hl);
    flag(t.contains("AVNCS"), "AVNCS", &mut hl);
    if re!(r"DOORS?\b").is_match(t) && !hl.iter().any(|x| x.starts_with('D')) {
        hl.insert("DALL".into());
    }
    hl
}

fn ecam_base(title: &str, sys: &str) -> Option<Node> {
    let s = sys.to_uppercase();
    let t = title.to_uppercase();
    let hl = ex_highlights(title, sys);
    let fire = t.contains("FIRE") || s.contains("FIRE");
    if s.starts_with("HYD") || t.starts_with("HYD") {
        return Some(ex_hyd(&hl));
    }
    if s.starts_with("ELEC") || re!(r"^\*?ELEC").is_match(&t) {
        return Some(ex_elec(&hl));
    }
    if s.starts_with("FUEL") {
        return Some(ex_fuel(&hl));
    }
    if re!(r"^(AIR|COND|VENT)").is_match(&s) && re!(r"BLEED|PACK|XBLEED|HOT AIR|DUCT|LEAK|ENG \d").is_match(&t) {
        return Some(ex_bleed(&hl));
    }
    if s.starts_with("DOOR") || (s.starts_with("SMOKE") && re!(r"CARGO|AVNCS|BULK").is_match(&t)) {
        return Some(ex_side_view(&hl, s.starts_with("SMOKE")));
    }
    if re!(r"^(ENG|F/CTL|BRAKES|L/G|STEER|MLG BAY|WHEEL|APU)").is_match(&s) || s.contains("FIRE") || re!(r"^ENG \d").is_match(&s) {
        return Some(ex_plan_view(&hl, fire));
    }
    None
}

type Units = &'static [(&'static str, &'static [&'static str])];

fn ex_units_rows(sys: &str) -> Option<Units> {
    Some(match sys {
        "NAV" => &[("ADR", &["1", "2", "3"]), ("IR", &["1", "2", "3"]), ("AOA", &["1", "2", "3"]), ("GPS", &["1", "2"]), ("ILS", &["1", "2"]), ("FLS", &["1", "2"]), ("GLS", &["1", "2"]), ("HUD", &[""])],
        "SURV" => &[("TCAS", &["1", "2"]), ("TAWS", &["1", "2"]), ("GPWS", &["1", "2"]), ("TERR SYS", &["1", "2"]), ("XPDR", &["1", "2"]), ("ROW/ROP", &[""])],
        "COM" => &[("VHF", &["1", "2", "3"]), ("HF", &["1", "2"]), ("SATCOM", &[""]), ("RMP", &["1", "2", "3"]), ("PTT", &["CAPT", "F/O", "3RD"]), ("DATALINK", &[""])],
        "AUTO FLT" => &[("AP", &[""]), ("A/THR", &["1", "2", "3", "4"]), ("FMS", &["1", "2"]), ("FMC", &["A", "B", "C"]), ("AFS CTL PNL", &["", "CAPT BKUP", "F/O BKUP"]), ("TCAS MODE", &[""])],
        "FWS" => &[("FWS", &["1", "2"]), ("FCDC", &["1", "2"]), ("ECP", &[""]), ("AUDIO", &[""])],
        "RECORDER" => &[("CVR", &[""]), ("DFDR", &[""]), ("ACCELMTR", &[""])],
        "CAB COM" => &[("CIDS", &["1", "2", "3"]), ("CABIN COM", &[""])],
        _ => return None,
    })
}

/// Ends of `[0-9A-C](?:\+|\s*&\s*)?` repeated, greediest first, from `i` (the regex's backtracking order).
fn token_ends(b: &[u8], i: usize) -> Vec<usize> {
    let mut out = Vec::new();
    if i < b.len() && (b[i].is_ascii_digit() || (b'A'..=b'C').contains(&b[i])) {
        let j = i + 1;
        let mut seps = Vec::new();
        if j < b.len() && b[j] == b'+' {
            seps.push(j + 1);
        } else {
            let mut k = j;
            while k < b.len() && (b[k] as char).is_whitespace() {
                k += 1;
            }
            if k < b.len() && b[k] == b'&' {
                let mut e = k + 1;
                while e < b.len() && (b[e] as char).is_whitespace() {
                    e += 1;
                }
                let mut alt = vec![e];
                let mut e2 = e;
                while e2 > k + 1 {
                    e2 -= 1;
                    alt.push(e2);
                }
                seps.extend(alt);
            }
        }
        seps.push(j);
        for s in seps {
            for e in token_ends(b, s) {
                out.push(e);
            }
            out.push(s);
        }
    }
    out
}

fn unit_hits(t: &str, name: &str, ids: &[&str]) -> HashSet<String> {
    let mut hits: HashSet<String> = HashSet::new();
    let b = t.as_bytes();
    let nb = name.as_bytes();
    let mut i = 0;
    while i + nb.len() <= b.len() {
        if &b[i..i + nb.len()] != nb {
            i += 1;
            continue;
        }
        let after = i + nb.len();
        let mut starts = Vec::new();
        if after < b.len() && (b[after] == b' ' || b[after] == b'-') {
            starts.push(after + 1);
        }
        starts.push(after);
        let mut found: Option<(usize, Option<(usize, usize)>)> = None;
        'outer: for s in starts {
            for e in token_ends(b, s).into_iter().map(Some).chain([None]) {
                let end = e.unwrap_or(s);
                if end < b.len() && b[end].is_ascii_uppercase() {
                    continue;
                }
                found = Some((end, e.map(|e| (s, e))));
                break 'outer;
            }
        }
        match found {
            Some((end, grp)) => {
                match grp {
                    None => hits.extend(ids.iter().map(|x| x.to_string())),
                    Some((s, e)) => {
                        for x in t[s..e].split(['+', '&']).map(str::trim).filter(|x| !x.is_empty()) {
                            hits.insert(x.into());
                        }
                    }
                }
                i = end.max(i + 1);
            }
            None => i += 1,
        }
    }
    if name == "A/THR" {
        for m in all(re!(r"ENG (\d) A/THR"), t) {
            hits.insert(g(&m, 1).into());
        }
    }
    if name == "AFS CTL PNL" {
        hits.clear();
        if t.contains("AFS CTL PNL") {
            hits.insert(String::new());
        }
        if t.contains("CAPT BKUP") {
            hits.insert("CAPT BKUP".into());
        }
        if t.contains("F/O BKUP") {
            hits.insert("F/O BKUP".into());
        }
    }
    if name == "PTT" {
        hits.clear();
        if t.contains("CAPT PTT") {
            hits.insert("CAPT".into());
        }
        if t.contains("F/O PTT") {
            hits.insert("F/O".into());
        }
        if t.contains("THIRD") {
            hits.insert("3RD".into());
        }
    }
    hits
}

pub fn ex_units(sys: &str, title: &str) -> Option<Node> {
    let rows = ex_units_rows(sys)?;
    let t = format!(" {} ", re!(r"^\S+(?:\s+FLT)?\s").replace(&title.to_uppercase(), ""));
    let (w, row_h) = (980.0, 52.0);
    let h = 30.0 + rows.len() as f64 * row_h;
    let mut parts = Vec::new();
    for (r, (name, ids)) in rows.iter().enumerate() {
        let y = 16.0 + r as f64 * row_h;
        let mut hits = unit_hits(&t, name, ids);
        let word = Regex::new(&format!("(^|[^A-Z/]){}([^A-Z]|$)", regex::escape(name))).unwrap();
        if !word.is_match(&t) && !["A/THR", "AFS CTL PNL", "PTT"].contains(name) {
            hits.clear();
        }
        if *name == "A/THR" && !t.contains("A/THR") {
            hits.clear();
        }
        let lit = !hits.is_empty();
        parts.push(ex_t(250.0, y + 28.0, name, T { anchor: "end", size: 14.0, fill: if lit { EX_HOT } else { EX_INK }, ..TD }));
        let bw = if ids.len() > 3 { 120.0 } else { 150.0 };
        for (i, id) in ids.iter().enumerate() {
            let on = hits.contains(*id);
            parts.push(ex_box(280.0 + i as f64 * (bw + 16.0), y + 6.0, bw, 36.0, if id.is_empty() { "SYS" } else { id }, on, 13.0));
        }
    }
    Some(ex_svg(w, h, &parts))
}

fn ex_cds(title: &str) -> Node {
    let t = title.to_uppercase();
    let mut parts = Vec::new();
    let both = re!(r"CAPT\+F/O|CAPT F/O").is_match(&t);
    let side = |k: &str| both || if k == "CAPT" { t.contains("CAPT") } else { t.contains("F/O") };
    let du = [("CAPT", "PFD", 20.0), ("CAPT", "ND", 150.0), ("", "EWD", 300.0), ("", "SD", 430.0), ("", "MFD", 560.0), ("F/O", "ND", 690.0), ("F/O", "PFD", 820.0)];
    for (who, nm, x) in du {
        let named = Regex::new(&format!(r"\b{nm}\b")).unwrap().is_match(&t);
        let on = if t.contains("DISPLAY DISAGREE") { nm == "PFD" || nm == "ND" } else { t.contains("DU") && named && (who.is_empty() || side(who)) };
        parts.push(ex_box(x, 24.0, 120.0, 120.0, "", on, 13.0));
        parts.push(ex_t(x + 60.0, 80.0, nm, T { size: 15.0, fill: if on { "#fff" } else { EX_INK }, ..TD }));
        parts.push(ex_t(x + 60.0, 104.0, who, T { size: 11.5, fill: if on { "#fff" } else { EX_DIM }, ..TD }));
    }
    for (who, x) in [("CAPT", 40.0), ("F/O", 740.0)] {
        let s = side(who);
        parts.push(ex_t(x + 100.0, 172.0, who, T { size: 12.0, fill: EX_DIM, ..TD }));
        parts.push(ex_box(x, 182.0, 200.0, 34.0, "EFIS CTL PNL", s && t.contains("EFIS CTL PNL"), 12.0));
        parts.push(ex_box(x, 224.0, 200.0, 34.0, "EFIS BKUP CTL", s && t.contains("EFIS BKUP"), 12.0));
        parts.push(ex_box(x, 266.0, 96.0, 34.0, "KCCU", s && t.contains("CURSOR"), 12.0));
        parts.push(ex_box(x + 104.0, 266.0, 96.0, 34.0, "KEYBD", s && t.contains("KEYBOARD"), 12.0));
    }
    if t.contains("MAILBOX") {
        parts.push(ex_box(410.0, 230.0, 160.0, 40.0, "MAILBOX", true, 12.0));
    }
    ex_svg(980.0, 320.0, &parts)
}

fn ex_cabin(title: &str) -> Node {
    let t = title.to_uppercase();
    let fire = re!(r"SMOKE|FIRE").is_match(&t) && !re!(r"DET FAULT|BOTTLES|BTL|FAULT$").is_match(&t);
    let hotc = if fire { EX_FIRE } else { EX_HOT };
    let s = 12.6;
    let p2 = side_p;
    let mut parts = vec![ex_path(&pts(&dx()["sil"]["side"]).iter().map(|q| p2(q[0], q[1])).collect::<Vec<_>>(), "rgba(255,255,255,0.03)", EX_INK, 1.6, false)];
    let zones: [(&str, f64, f64, f64, f64, &Regex); 16] = [
        ("MAIN AVNCS", 3.5, 9.5, -2.2, 0.8, re!(r"MAIN AVNCS|AVNCS VENT|AVNCS BLOWING|AVNCS EXTRACT|AVNCS OVBD|COOLG")),
        ("UPPER AVNCS", 3.5, 8.0, 4.2, 6.6, re!(r"UPPER AVNCS")),
        ("AFT AVNCS", 44.0, 47.0, 4.2, 6.6, re!(r"AFT AVNCS")),
        ("FWD LWR REST", 27.9, 33.5, -2.2, 0.8, re!(r"FWD LWR CAB REST")),
        ("LWR LAV", 33.5, 36.5, -2.2, 0.8, re!(r"LOWER DECK LAV")),
        ("IFE BAY", 30.0, 33.5, 4.2, 6.6, re!(r"IFE BAY")),
        ("FWD CARGO", 10.5, 27.9, -2.2, 0.8, re!(r"FWD CARGO|FWD\+AFT CARGO")),
        ("AFT CARGO", 37.0, 46.7, -2.2, 0.8, re!(r"AFT CARGO|FWD\+AFT CARGO")),
        ("BULK", 47.3, 50.1, -2.2, 0.8, re!(r"BULK")),
        ("PACK BAY 1", 22.0, 30.0, -2.6, -1.2, re!(r"PACK BAY 1")),
        ("PACK BAY 2", 22.0, 30.0, -1.2, 0.2, re!(r"PACK BAY 2|PACK BAY 1\+2")),
        ("MAIN DECK FWD", 6.0, 30.0, 1.2, 3.6, re!(r"FWD VENT|PURSER|PRIMARY CABIN FAN|SECONDARY CABIN FAN|TEMP CTL|MIXER")),
        ("MAIN DECK AFT", 30.0, 56.0, 1.2, 3.6, re!(r"AFT VENT|TEMP CTL|CABIN AIR EXTRACT|SECONDARY CABIN FAN|PRIMARY CABIN FAN")),
        ("UPPER DECK", 12.0, 52.0, 4.2, 6.6, re!(r"TEMP CTL|PURSER|LAV & GALLEYS")),
        ("THS BAY", 58.0, 64.0, 2.0, 4.6, re!(r"THS BAY")),
        ("MAIN 5L REST", 54.0, 57.5, 1.2, 3.6, re!(r"MAIN 5L CAB REST")),
    ];
    for (nm, a, b, ylo, yhi, r) in zones {
        let on = r.is_match(&t);
        if !on && re!(r"^(PACK BAY|MAIN DECK|UPPER DECK|THS BAY|MAIN 5L)").is_match(nm) {
            continue;
        }
        let [ax, ay] = p2(EX_NOSE + a, yhi);
        let [bx, by] = p2(EX_NOSE + b, ylo);
        parts.push(format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" fill-opacity=\"{}\" stroke=\"{}\" stroke-width=\"1.3\" stroke-dasharray=\"{}\"/>",
            n(ax),
            n(ay),
            n(bx - ax),
            n(by - ay),
            if on { hotc } else { "none" },
            if on { "0.85" } else { "0" },
            if on { hotc } else { EX_DIM },
            if on { "" } else { "5 4" }
        ));
        if on {
            parts.push(ex_t((ax + bx) / 2.0, ay - 10.0, nm, T { size: 12.5, fill: hotc, ..TD }));
        }
    }
    for m in all(re!(r"MAIN (\d)([LR]) (CWS|RCC)"), &t) {
        let Some(b) = ex_part(&format!("ANIM_DOOR_M{}{}", g(&m, 1), g(&m, 2))) else { continue };
        let [cx, cy] = p2(b[1][2] + 2.0, 2.4);
        parts.push(format!("<circle cx=\"{}\" cy=\"{}\" r=\"14\" fill=\"{hotc}\"/>", n(cx), n(cy)));
        parts.push(ex_t(cx, cy - 22.0, &format!("{}{} {}", g(&m, 1), g(&m, 2), g(&m, 3)), T { size: 12.5, fill: hotc, ..TD }));
    }
    for (k, z) in [(1, 6.0), (2, 9.0), (3, 52.0), (4, 55.0)] {
        let on = t.contains("OUTFLW") && (Regex::new(&format!(r"CTL {k}\b")).unwrap().is_match(&t) || !re!(r"CTL \d").is_match(&t));
        let [cx, cy] = p2(EX_NOSE + z, if z < 20.0 { -2.6 } else { -1.4 });
        parts.push(format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"9\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.4\"/>",
            n(cx),
            n(cy),
            if on { EX_HOT } else { "#1a1d21" },
            if on { EX_HOT } else { EX_DIM }
        ));
        parts.push(ex_t(cx, cy + 26.0, &format!("OFV {k}"), T { size: 11.0, fill: if on { EX_HOT } else { EX_DIM }, ..TD }));
    }
    if t.contains("CAB PRESS") && !t.contains("OUTFLW") {
        let [ax, ay] = p2(EX_NOSE + 2.0, 7.2);
        let [bx, by] = p2(EX_NOSE + 60.0, -3.0);
        parts.push(format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"40\" fill=\"none\" stroke=\"{EX_HOT}\" stroke-width=\"3\"/>", n(ax), n(ay), n(bx - ax), n(by - ay)));
    }
    if t.contains("EMER EXIT") {
        for (_, key) in EX_DOORS {
            let Some(b) = ex_part(key) else { continue };
            let [ax, ay] = p2(b[0][2], b[1][1]);
            parts.push(format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"5\" fill=\"{EX_HOT}\"/>",
                n(ax),
                n(ay),
                n((b[1][2] - b[0][2]) * s),
                n((b[1][1] - b[0][1]) * s)
            ));
        }
    }
    ex_svg(980.0, 400.0, &parts)
}

fn ecam_ex(title: &str, sys: &str) -> Option<Node> {
    let s = sys.to_uppercase();
    let s = s.strip_prefix('*').unwrap_or(&s);
    let t = title.to_uppercase();
    if ex_units_rows(s).is_some() {
        return ex_units(s, title);
    }
    if s == "CDS" || s == "CDS & AUTO FLT" {
        return Some(ex_cds(title));
    }
    if re!(r"^(SMOKE|VENT|CAB PRESS|CABIN)$").is_match(s) || (s == "COND" && !re!(r"HOT AIR|RAM AIR").is_match(&t)) {
        return Some(ex_cabin(title));
    }
    if s == "COND" {
        return Some(ex_bleed(&ex_highlights(title, sys)));
    }
    if s == "MLG BAY" {
        return Some(ex_plan_view(&["MLG".to_string()].into_iter().collect(), t.ends_with("FIRE")));
    }
    if s == "CONFIG" {
        let mut hl = HashSet::new();
        for (k, v) in [("FLAPS", "FLAP"), ("SLATS", "SLAT"), ("PITCH TRIM", "THS"), ("RUDDER TRIM", "RUD"), ("SPD BRK", "SPLRALL"), ("PARK BRK", "GEAR")] {
            if t.contains(k) {
                hl.insert(v.to_string());
            }
        }
        return if hl.is_empty() { None } else { Some(ex_plan_view(&hl, false)) };
    }
    ecam_base(title, sys)
}

/// `ecamDiagram`: the SYSTEM picture of an ECAM procedure.
pub fn ecam_diagram_for(title: &str, sys: &str) -> Option<Node> {
    let su = sys.to_uppercase();
    let s = su.strip_prefix('*').unwrap_or(&su);
    let t = title.to_uppercase();
    if let Some(m) = re!(r"^ENG (\d)\b").captures(&t) {
        if (s.starts_with("ENG") || s.contains("FIRE")) && !re!(r"BLEED|GEN|EDP|HYD").is_match(&t) {
            let k: usize = g(&m, 1).parse().unwrap_or(1);
            let faults = eng_faults(&[FailIn { id: 1e7, name: t.clone(), active: true, magnitude: 1.0 }], k);
            let down = re!(r"FAIL|SHUT ?DOWN|FIRE").is_match(&t);
            return Some(engine_svg(&EngOpts { n1: if down { 20.0 } else { 85.0 }, n3: if down { 30.0 } else { 96.0 }, faults, title: format!("ENGINE {k}"), ..Default::default() }));
        }
    }
    if re!(r"^(AIR|COND|VENT|CAB PRESS|A-ICE|ANTI ICE)").is_match(s) {
        if let Some(d) = air_diagram(&t, s) {
            return Some(d);
        }
    }
    if s.starts_with("F/CTL") {
        if let Some(tm) = fctl_targets(&t) {
            return Some(fctl_plan_view(&tm, s));
        }
    }
    if s.starts_with("FUEL") {
        if let Some(d) = fuel_diagram(&t, s) {
            return Some(d);
        }
    }
    if s.starts_with("HYD") {
        if let Some(d) = hyd_diagram(&t, s) {
            return Some(d);
        }
    }
    if s.starts_with("ELEC") {
        if let Some(d) = elec_diagram(&t, s) {
            return Some(d);
        }
    }
    ecam_ex(title, sys)
}

pub fn ecam_diagram(p: &Proc) -> Option<Node> {
    ecam_diagram_outer(&p.t, p.s.as_deref().unwrap_or(""))
}

fn set(keys: &[&str]) -> HashSet<String> {
    keys.iter().map(|k| k.to_string()).collect()
}

/// The last `ecamDiagram` wrapper: pictures for titles the system routes leave empty.
pub fn ecam_diagram_outer(title: &str, sys: &str) -> Option<Node> {
    if let Some(d) = ecam_diagram_for(title, sys) {
        return Some(d);
    }
    let t = title.to_uppercase();
    if let Some(c) = re!(r"^\*([A-Z/]+)").captures(&t) {
        let st = &c[1];
        return ecam_diagram_for(&format!("{st} SYS"), if st == "BLEED" { "AIR" } else { st });
    }
    if t.starts_with("APU FIRE") {
        return Some(ex_plan_view(&set(&["APU"]), true));
    }
    if t.contains("AUTOLAND") {
        return ex_units("AUTO FLT", &format!("AP SYS FMS 1 FMS 2 {title}"));
    }
    if re!(r"^T\.O SPEEDS|^T\.O V1").is_match(&t) {
        return ex_units("AUTO FLT", &format!("FMS 1 FMS 2 FMC A FMC B FMC C {title}"));
    }
    if t.starts_with("T.O ACCELERATION") || t.contains("VOLCANIC ASH") {
        return Some(ex_plan_view(&set(&["ENG1", "ENG2", "ENG3", "ENG4"]), false));
    }
    if t.contains("SIDESTICK") {
        return ecam_diagram_for(title, "F/CTL").or_else(|| Some(ex_plan_view(&set(&["AILALL", "ELEVALL"]), false)));
    }
    if re!(r"VIDEO|OIS").is_match(&t) {
        return Some(ex_cds(title));
    }
    if t.contains("EMER EVAC") {
        return Some(ex_side_view(&set(&["DALL"]), false));
    }
    if t.contains("OVERWEIGHT LDG") {
        return Some(ex_plan_view(&set(&["GEAR", "LGL", "LGR", "MLG"]), false));
    }
    None
}

const MAP_TITLES: [&str; 19] = [
    "ELECTRICAL", "HYDRAULICS", "FUEL", "FLIGHT CONTROLS", "LIVE AIRFLOW", "BLEED AND PACKS", "CABIN AIR AND VENTILATION", "ENGINE", "LANDING GEAR AND BRAKES", "APU", "FIRE PROTECTION",
    "OXYGEN", "DOORS", "ANTI ICE AND RAIN", "NAVIGATION", "SURVEILLANCE", "COMMUNICATIONS", "AUTO FLIGHT", "WARNING AND RECORDING",
];

/// The Documents SYSTEM DIAGRAMS page `i` (`SYSTEM_MAPS[i][1]()`, or the airflow page).
pub fn system_map(i: usize, _title: &str) -> Vec<Node> {
    match MAP_TITLES.get(i).copied() {
        Some("LIVE AIRFLOW") => vec![air::airflow_page()],
        Some(t) => vec![sl::sl_live(t)],
        None => vec![],
    }
}

/// `failDiagram`: the picture under an open failure family.
pub fn fail_diagram(fam: &crate::failures::FailFamily, members: &[&crate::failures::FailUnit], chosen: &[&crate::failures::FailUnit], severity: f64) -> Option<Node> {
    let pick: Vec<&crate::failures::FailUnit> = if chosen.is_empty() { members.iter().take(1).copied().collect() } else { chosen.to_vec() };
    if pick.is_empty() {
        return None;
    }
    let sev = if severity != 0.0 && !severity.is_nan() { severity } else { 1.0 };
    let list: Vec<FailIn> = pick.iter().map(|u| FailIn { id: u.id as f64, name: u.name.clone(), active: true, magnitude: sev }).collect();
    let ata = if fam.key == "part-separation" { (list[0].id / 1000.0).floor() as i64 } else { crate::failures::parse_int(&fam.chapter).unwrap_or(-1) };
    let names = list.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(" ");
    let ftitle = fam.title.to_uppercase();
    if fam.key == "part-separation" {
        let title = if list.len() > 1 { "PART SEPARATION".to_string() } else { list[0].name.to_uppercase() };
        let mut engines: Vec<usize> = Vec::new();
        for f in &list {
            if f.id >= 54000.0 && f.id < 54004.0 {
                let e = (f.id - 53999.0) as usize;
                if !engines.contains(&e) {
                    engines.push(e);
                }
            }
        }
        let fire = ctx(|c| c.fire_pb);
        let mut views = Vec::new();
        if let Some(leaks) = hyd_separation_leaks(&list, &fire) {
            views.push(hyd_view(Some(&leaks), &title));
        }
        if !engines.is_empty() {
            let mut e = ElecT::default();
            for k in &engines {
                e.dead.insert(format!("GEN{k}"));
            }
            let loads = elec::elec_loads_lost(&e);
            views.push(elec_view(&e, &elec::ElecOpts { title: title.clone(), full: false, loads }));
            let mut b = OMap::default();
            for k in &engines {
                for p in ["HP", "IPC", "PRV", "PCLR", "FAV", "NAI", "START"] {
                    b.set(&format!("{p}{k}"), "fail");
                }
            }
            views.push(bleed_view(&b, &title));
        }
        if !views.is_empty() {
            return Some(if views.len() == 1 { views.pop().unwrap() } else { el("div", "ff-diagram-stack").children(views) });
        }
    }
    if ata == 24 || re!(r"(?i)\b(?:AC|DC)[ _]?(?:\d|ESS|EHA|EMER)|\bTR \d|STATIC INV|BUS TIE|CONTACTOR").is_match(&list[0].name) {
        if let Some(d) = elec_diagram(&names, &ftitle) {
            return Some(d);
        }
    }
    if ata == 21 || ata == 36 || (ata == 30 && re!(r"(?i)ANTI.?ICE|WING|NACELLE").is_match(&list[0].name)) {
        if let Some(d) = air_diagram(&names, &ftitle) {
            return Some(d);
        }
    }
    if ata == 27 || ata == 55 || ata == 57 {
        if let Some(d) = fctl_detail_view(&list[0].name, &ftitle) {
            return Some(d);
        }
        if let Some(tm) = fctl_targets(&names) {
            return Some(fctl_plan_view(&tm, &ftitle));
        }
    }
    if ata == 28 {
        if let Some(d) = fuel_diagram(&names, &ftitle) {
            return Some(d);
        }
    }
    if ata == 29 {
        if let Some(d) = hyd_diagram(&names, &ftitle) {
            return Some(d);
        }
    }
    if eng::fail_is_engine(ata, &list[0]) {
        if list.iter().any(|f| f.id >= 54000.0 && f.id < 54004.0) {
            return None;
        }
        let k = eng::fail_engine_of(&list[0]);
        return Some(engine_svg(&EngOpts { n1: 85.0, n3: 96.0, faults: eng_faults(&list, k), title: format!("ENGINE {k}"), readout: "CRUISE, N1 85 %".into(), ..Default::default() }));
    }
    None
}

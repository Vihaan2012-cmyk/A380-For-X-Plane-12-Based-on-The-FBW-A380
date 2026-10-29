#![allow(clippy::needless_range_loop)]

pub mod extra;
pub mod pages;
pub mod sim;

use cids::view::{FapView, HardKey, PageBody, SwitchView, ViewMode};
use cids::{Level, Page};
use mfd_tree::Node;
pub use sim::{DoorMark, FapSim};

pub const FAP_CSS: &str = include_str!("../../../html_ui/Pages/VCockpit/Instruments/A380X/FAP/fap.css");

pub const LEGACY_NAV: [Page; 11] =
    [Page::Status, Page::Music, Page::PreAnn, Page::Lights, Page::Doors, Page::Temp, Page::Water, Page::Smoke, Page::Seats, Page::Ready, Page::SysInfo];

#[derive(Clone, Debug, PartialEq)]
pub struct Ui {
    pub show_main: bool,
    pub show_upper: bool,
    pub nav: Vec<Page>,
}

impl Default for Ui {
    fn default() -> Self {
        Ui { show_main: true, show_upper: true, nav: Page::ALL.to_vec() }
    }
}

pub struct Ctx<'a> {
    pub view: &'a FapView,
    pub sim: &'a FapSim,
    pub ui: &'a Ui,
}

pub fn page_id(p: Page) -> &'static str {
    match p {
        Page::Status => "status",
        Page::Music => "music",
        Page::PreAnn => "preann",
        Page::Lights => "lights",
        Page::Doors => "doors",
        Page::Temp => "temp",
        Page::Water => "water",
        Page::Smoke => "smoke",
        Page::Seats => "seats",
        Page::Ready => "ready",
        Page::SysInfo => "sysinfo",
        Page::Calls => "calls",
        Page::Evac => "evac",
        Page::Maintenance => "maint",
    }
}

pub fn page_from_id(id: &str) -> Option<Page> {
    Page::ALL.iter().copied().find(|p| page_id(*p) == id)
}

fn nav_label(p: Page) -> &'static str {
    match p {
        Page::Status => "Cabin<br>Status",
        Page::Music => "Music",
        Page::PreAnn => "Pre.<br>Ann.",
        Page::Lights => "Lights",
        Page::Doors => "Doors/<br>Slides",
        Page::Temp => "Temp.",
        Page::Water => "Water/<br>Waste",
        Page::Smoke => "Smoke<br>Detect",
        Page::Seats => "Seat<br>Setting",
        Page::Ready => "Cabin<br>Ready",
        Page::SysInfo => "System<br>Info",
        Page::Calls => "Calls",
        Page::Evac => "Evac",
        Page::Maintenance => "Maint.",
    }
}

fn hard_key_label(k: &HardKey) -> String {
    match k.id {
        "ped" => "PED<br>POWER".into(),
        "lights" => "LIGHTS MAIN<br>ON/OFF".into(),
        "lock" => "SCREEN<br>30 sec LOCK".into(),
        "pcreset" => "FAP - PC<br>RESET".into(),
        _ => match k.label.rsplit_once(' ') {
            Some((a, b)) => format!("{a}<br>{b}"),
            None => k.label.to_string(),
        },
    }
}

pub(crate) fn lines(mut n: Node, label: &str) -> Node {
    for (i, part) in label.split("<br>").enumerate() {
        if i > 0 {
            n = n.child(Node::new("br"));
        }
        n = n.text(part);
    }
    n
}

pub(crate) fn bx(x: i32, y: i32, w: i32, h: i32, cls: &str) -> Node {
    Node::el("div", cls).style(&format!("left:{x}px;top:{y}px;width:{w}px;height:{h}px;"))
}

pub(crate) fn key(x: i32, y: i32, w: i32, h: i32, _act: &str, label: &str, cls: &str) -> Node {
    let n = Node::el("div", &format!("key {cls}")).style(&format!("position:absolute;left:{x}px;top:{y}px;width:{w}px;height:{h}px;"));
    lines(n, label)
}

pub(crate) fn ttl(t: &str) -> Node {
    Node::el("div", "ttl").text(t)
}

pub(crate) fn fmt(x: Option<f64>, d: usize, u: &str) -> String {
    match x {
        Some(v) => format!("{v:.d$}{u}"),
        None => "--".to_string(),
    }
}

pub(crate) fn row(n: Node, k: &str, v: &str, colour: &str) -> Node {
    let s = Node::el("div", "s").style(&if colour.is_empty() { String::new() } else { format!("color:{colour}") }).text(v);
    n.text(k).child(s)
}

fn arrow(prev: bool) -> Node {
    let mut svg = Node::new("svg");
    svg.svgattr.insert("width".into(), "16".into());
    svg.svgattr.insert("height".into(), "18".into());
    svg.svgattr.insert("viewBox".into(), "0 0 16 18".into());
    let pts = if prev { "15,1 15,17 1,9" } else { "1,1 1,17 15,9" };
    svg.shapes.push(sim::shape("polygon", &[("points", pts), ("fill", "#1d2125")], 0));
    Node::el("div", "key arrow").style("width:40px;height:34px;").child(svg)
}

fn nav(ui: &Ui, current: Page) -> Node {
    let narrow = ui.nav.len() > LEGACY_NAV.len();
    let mut n = Node::el("div", "nav").child(arrow(true));
    for p in &ui.nav {
        let w = if narrow { 46 } else { 52 };
        let k = Node::el("div", &format!("key {}", if *p == current { "on" } else { "" })).style(&format!("width:{w}px;height:34px;"));
        n = n.child(lines(k, nav_label(*p)));
    }
    n.child(arrow(false))
}

fn level_class(l: Level) -> &'static str {
    match l {
        Level::Red => "red",
        Level::Amber => "amber",
        Level::Cyan => "cyan",
        Level::White => "",
    }
}

fn hard_keys(keys: &[HardKey], switches: &[SwitchView]) -> Node {
    let mut hk = Node::new("div").id("HK");
    for (i, k) in keys.iter().enumerate() {
        let mut cls = format!("hk {}", if k.id == "evac" { "evac" } else { "" });
        if k.guard {
            cls.push_str(" guard");
        }
        if k.alert {
            cls.push_str(" alert");
        } else if k.lit {
            cls.push_str(" lit");
        }
        let n = Node::el("div", &cls).style(&format!("left:{}px", 18 + i * 92));
        hk = hk.child(lines(n, &hard_key_label(k)));
    }
    for (i, s) in switches.iter().enumerate() {
        let (up_t, down_t) = if s.id == "emer" { ("ON", "ARMED") } else { ("ON", "OFF") };
        let t = Node::el("div", "tsw")
            .style(&format!("left:{}px", 768 + i * 92))
            .child(Node::new("div").text(s.label))
            .child(Node::el("div", "base").child(Node::el("div", &format!("bat {}", if s.up { "up" } else { "down" }))))
            .child(Node::new("div").style("font-weight:400").text(if s.up { up_t } else { down_t }));
        hk = hk.child(t);
    }
    hk
}

pub fn body_node(c: &Ctx) -> Vec<Node> {
    match (&c.view.body, c.view.page) {
        (PageBody::Fault, _) => extra::fault(),
        (_, Page::Status) => pages::status(c),
        (_, Page::Music) => pages::music(c),
        (_, Page::PreAnn) => pages::preann(c),
        (_, Page::Lights) => pages::lights(c),
        (_, Page::Doors) => pages::doors(c),
        (_, Page::Temp) => pages::temp(c),
        (_, Page::Water) => pages::water(c),
        (_, Page::Smoke) => pages::smoke(c),
        (_, Page::Seats) => pages::seats(c),
        (_, Page::Ready) => pages::ready(c),
        (_, Page::SysInfo) => pages::sysinfo(c),
        (_, Page::Calls) => extra::calls(c),
        (_, Page::Evac) => extra::evac(c),
        (_, Page::Maintenance) => extra::maintenance(c),
    }
}

pub fn fap_root(view: &FapView, sim: &FapSim, ui: &Ui) -> Node {
    let c = Ctx { view, sim, ui };
    let mut scr = Node::new("div").id("SCR");
    let keys = if view.mode == ViewMode::Dark { Vec::new() } else { view.keys.clone() };
    let hk = if view.mode == ViewMode::Dark { Node::new("div").id("HK") } else { hard_keys(&keys, &view.switches) };
    match view.mode {
        ViewMode::Dark | ViewMode::ScreenOff => scr = scr.class_if("dark", true),
        ViewMode::Booting => {
            scr = scr.class_if("dark", true);
            if view.boot_remaining_s < 5. {
                scr = scr.child(
                    Node::new("div")
                        .style("position:absolute;left:0;right:0;top:280px;text-align:center;color:#c9ccd0;font:700 14px B612")
                        .text("FAP INITIALIZING..."),
                );
            }
        }
        ViewMode::Live => {
            scr = scr.child(Node::el("div", "hdr").text(view.title));
            scr = scr.child(key(8, 6, 46, 28, "screenoff", "Screen<br>Off", "screenoff"));
            let mut caut = Node::el("div", "caut");
            for ct in &view.cautions {
                caut = caut.child(Node::el("span", level_class(ct.level)).text(&ct.text));
            }
            scr = scr.child(caut).children(body_node(&c)).child(nav(ui, view.page));
            if let Some(m) = &view.message {
                scr = scr.child(Node::el("div", "msg").text(m));
            }
            if view.locked {
                scr = scr.child(Node::el("div", "msg").text("SCREEN LOCKED FOR CLEANING"));
            }
        }
    }
    Node::new("div").id("FAP_ROOT").child(scr).child(hk)
}

#[cfg(test)]
mod tests;

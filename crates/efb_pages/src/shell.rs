use crate::bg::{lock_bg, Backgrounds};
use crate::js::{el, svg};
use crate::{span, Node};

const ICONS: [(&str, &str); 12] = [
    ("home", r#"<path d="M4 11.5 12 5l8 6.5"/><path d="M6.5 10v9h11v-9"/>"#),
    ("flight", r#"<path d="M21 14v-2l-8-5V3.5a1.3 1.3 0 0 0-2.6 0V7l-8 5v2l8-2.5V16l-2 1.4v1.6l3.3-1 3.3 1v-1.6L13 16v-4.5z"/>"#),
    ("cabin", r#"<path d="M4 18V9a3 3 0 0 1 3-3h10a3 3 0 0 1 3 3v9"/><path d="M4 18h16"/><path d="M8 6v12M12 6v12M16 6v12"/>"#),
    ("ground", r#"<rect x="3" y="10.5" width="12" height="6" rx="1.3"/><path d="M15 12.5h3l2.5 2.5v1.5H15"/><circle cx="7" cy="18" r="1.6"/><circle cx="17" cy="18" r="1.6"/>"#),
    ("aircraft", r#"<circle cx="12" cy="12" r="8.4"/><path d="M12 7v5l3.4 2"/>"#),
    ("failures", r#"<path d="M12 3 2.5 20h19z"/><path d="M12 10v4.5"/><path d="M12 17.4v.1"/>"#),
    ("checklists", r#"<path d="M4 5.5A2.5 2.5 0 0 1 6.5 3H20v15H6.5A2.5 2.5 0 0 0 4 20.5z"/><path d="M4 20.5A2.5 2.5 0 0 0 6.5 23H20v-5"/><path d="M8 7.5h8M8 11h6"/>"#),
    ("components", r#"<rect x="6.5" y="6.5" width="11" height="11" rx="1.5"/><path d="M9.5 3v3.5M14.5 3v3.5M9.5 17.5V21M14.5 17.5V21M3 9.5h3.5M3 14.5h3.5M17.5 9.5H21M17.5 14.5H21"/>"#),
    ("wear", r#"<path d="M4.5 16.5a8 8 0 1 1 15 0"/><path d="M12 13.5l4-4.5"/><circle cx="12" cy="13.5" r="1.3"/>"#),
    ("breakers", r#"<rect x="5" y="3.5" width="14" height="17" rx="2"/><path d="M9 8h6M9 12h6M9 16h6"/><circle cx="7.6" cy="8" r=".4"/><circle cx="7.6" cy="12" r=".4"/><circle cx="7.6" cy="16" r=".4"/>"#),
    ("maint", r#"<path d="M14.7 6.3a4 4 0 0 0-5.4 5.2L4 16.8V20h3.2l5.3-5.3a4 4 0 0 0 5.2-5.4l-2.6 2.6-2.4-.6-.6-2.4z"/>"#),
    ("settings", r#"<circle cx="12" cy="12" r="3"/><path d="M12 2.8v2.4M12 18.8v2.4M2.8 12h2.4M18.8 12h2.4M5.3 5.3l1.7 1.7M17 17l1.7 1.7M5.3 18.7l1.7-1.7M17 7l1.7-1.7"/>"#),
];

pub struct Section {
    pub key: &'static str,
    pub title: &'static str,
    pub subs: &'static [(&'static str, &'static str)],
}

pub const SECTIONS: [Section; 12] = [
    Section { key: "home", title: "Home", subs: &[("Home", "home")] },
    Section { key: "flight", title: "Flight", subs: &[("Flight Plan", "flightplan"), ("Payload & Fuel", "loadsheet"), ("Cargo", "cargo"), ("Performance", "performance"), ("Charts", "charts"), ("ATC", "atc")] },
    Section { key: "checklists", title: "Documents", subs: &[("Documents", "checklists")] },
    Section { key: "ground", title: "Ground", subs: &[("Ground Services", "ground"), ("Pushback", "pushback")] },
    Section { key: "cabin", title: "Cabin", subs: &[("Flight Attendant Panel", "fap")] },
    Section {
        key: "aircraft",
        title: "Systems",
        subs: &[("Systems", "study:"), ("Engines", "study:Engines"), ("Air", "study:Environmental"), ("Hydraulics", "study:Hydraulics"), ("Landing Gear", "study:Landing Gear"), ("Fire", "study:Fire")],
    },
    Section { key: "components", title: "Components", subs: &[("Components", "components")] },
    Section { key: "wear", title: "Wear & Tear", subs: &[("Wear & Tear", "wear")] },
    Section { key: "maint", title: "Maintenance", subs: &[("MEL & Maintenance", "maintenance")] },
    Section { key: "breakers", title: "Breakers", subs: &[("Circuit Breakers", "breakers")] },
    Section { key: "failures", title: "Failures", subs: &[("Failures", "failures")] },
    Section { key: "settings", title: "Settings", subs: &[("Settings", "settings"), ("Throttle", "throttle"), ("Status", "status"), ("Diagnostics", "diagnostics"), ("Support", "support")] },
];

/// `tab` is the active tab id; the Systems tab is `study:<group>`.
pub fn section_of(tab: &str) -> &'static Section {
    SECTIONS.iter().find(|s| s.subs.iter().any(|(_, t)| *t == tab)).unwrap_or(&SECTIONS[0])
}

fn icon(key: &str) -> Node {
    let body = ICONS.iter().find(|(k, _)| *k == key).map(|(_, b)| *b).unwrap_or("");
    svg(&format!(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">{body}</svg>"#))
}

pub fn rail(tab: &str) -> Node {
    let on = section_of(tab).key;
    let mut n = el("nav", "efb-rail").child(el("div", "logo").text("A380"));
    for s in &SECTIONS {
        n = n.child(Node::new("a").class_if("on", s.key == on).act(&format!("sec:{}", s.key)).child(icon(s.key)).child(Node::new("span").text(s.title)));
    }
    n.child(el("div", "spacer"))
}

#[derive(Clone, Debug, Default)]
pub struct Clock {
    pub utc: String,
    pub local: String,
    pub chrono: String,
    pub chrono_running: bool,
    /// Origin, destination and flown fraction in percent for the SimBrief progress bar; None hides it.
    pub progress: Option<(String, String, Option<f64>)>,
}

fn progress(p: &Option<(String, String, Option<f64>)>) -> Node {
    let (org, dst, pct) = p.clone().unwrap_or_default();
    let (fill, dot) = match pct {
        Some(v) => (Node::new("i").style(&format!("width: {}%;", crate::js::num(v))), Node::new("b").style(&format!("left: {}%;", crate::js::num(v)))),
        None => (Node::new("i"), Node::new("b")),
    };
    el("div", "sb-prog").attr_if("hidden", p.is_none()).child(span("sb-org", &org)).child(el("div", "sb-track").child(fill).child(dot)).child(span("sb-dst", &dst))
}

pub fn topbar(tab: &str, clock: &Clock) -> Node {
    let s = section_of(tab);
    let mut title = s.title;
    let mut subs = el("div", "subs");
    if s.subs.len() > 1 {
        for (label, t) in s.subs {
            if *t == tab {
                title = label;
            }
            subs = subs.child(Node::new("a").class_if("on", *t == tab).act(&format!("tab:{t}")).text(label));
        }
    }
    el("div", "efb-topbar").child(Node::new("h1").text(title)).child(subs).child(
        el("div", "st").child(progress(&clock.progress)).child(
            el("div", "efb-clock")
                .child(Node::new("span").id("clock-utc").text(&clock.utc))
                .child(Node::new("span").id("clock-local").text(&clock.local))
                .child(el("button", "chrono").class_if("running", clock.chrono_running).id("chrono").attr("type", "button").text(&clock.chrono)),
        ),
    )
}

/// `link` is the systems link light (None before the first status answer).
pub fn footer(link: Option<bool>) -> Node {
    Node::new("footer")
        .child(el("div", "brand").child(Node::new("img")).child(Node::new("span").text("A380 for X-Plane 12")))
        .child(
            el("div", "legal")
                .text("\u{a9} 2026 A380 for X-Plane 12, ported from FBW A380 (GPL-3.0)")
                .child(Node::new("br"))
                .text("Airport maps \u{a9} OpenStreetMap contributors (ODbL)")
                .child(Node::new("br"))
                .child(Node::new("a").act("action:licenses").text("LICENSES")),
        )
        .child(el("div", "server").text("Systems link: ").child(el("span", match link { Some(true) => "dot ok", Some(false) => "dot bad", None => "dot" }).id("dot-link").style("margin:0 0 0 4px")))
}

#[derive(Clone, Debug, Default)]
pub struct LockClock {
    pub date: String,
    pub clock: String,
    pub utc: String,
}

pub fn lock(bgs: &Backgrounds, c: &LockClock) -> Node {
    el("div", "")
        .id("efb-lock")
        .act("unlock")
        .child(lock_bg(bgs))
        .child(el("div", "lock-scrim"))
        .child(
            el("div", "lock-inner")
                .child(el("div", "lock-top").child(span("mono", "A380 / EFB 1")).child(span("mono", "100%").id("lock-pct")))
                .child(
                    el("div", "lock-mid")
                        .child(el("div", "mono lock-date").id("lock-date").text(&c.date))
                        .child(el("div", "lock-clock").id("lock-clock").text(&c.clock))
                        .child(el("div", "mono lock-sub").child(Node::new("span").text("UTC ").child(Node::new("b").id("lock-utc").text(&c.utc)))),
                )
                .child(el("div", "lock-foot").child(span("mono", "KJFK -> EGLL / A380-842")).child(span("lock-hint", "Tap to unlock"))),
        )
}

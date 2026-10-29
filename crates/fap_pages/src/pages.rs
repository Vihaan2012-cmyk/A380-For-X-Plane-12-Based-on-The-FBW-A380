use crate::sim::{plan, shape, DoorMark, FapSim, DOORS, ZONES};
use crate::{bx, fmt, key, lines, row, ttl, Ctx};
use cids::view::PageBody;
use mfd_tree::Node;

const ANNOUNCEMENTS: [(u8, &str); 10] = [
    (2, "Boarding"),
    (3, "Taxi /<br>Safety"),
    (4, "Takeoff"),
    (5, "After<br>Takeoff"),
    (6, "Cruise"),
    (7, "Descent"),
    (8, "Approach"),
    (9, "After<br>Landing"),
    (10, "Gate<br>Arrival"),
    (11, "Deboard-<br>ing"),
];
const SCENES: [(u8, &str, Option<&str>); 12] = [
    (12, "Auto", None),
    (1, "Boarding", Some("#ffb86b")),
    (2, "Taxi", Some("#5280ff")),
    (3, "Takeoff", Some("#5280ff")),
    (4, "Climb", Some("#ffb86b")),
    (5, "Cruise", Some("#fff2e0")),
    (6, "Meal", Some("#ffb86b")),
    (7, "Night", Some("#1f2973")),
    (8, "Sunrise", Some("#ff8026")),
    (9, "Descent", Some("#ffb86b")),
    (10, "Landing", Some("#5280ff")),
    (11, "Arrival", Some("#fff2e0")),
];
pub const AREAS: [(&str, &str); 6] = [
    ("MD FWD", "MAIN DECK FWD"),
    ("MD MID", "MAIN DECK MID"),
    ("MD AFT", "MAIN DECK AFT"),
    ("UD FWD", "UPPER DECK FWD"),
    ("UD MID", "UPPER DECK MID"),
    ("UD AFT", "UPPER DECK AFT"),
];
const AMBER: &str = "#ffa21f";
const GREEN: &str = "#7cf04a";

fn f1(v: f64) -> String {
    format!("{v:.1}")
}

fn panel(x: i32, y: i32, w: i32, h: i32, title: &str) -> Node {
    bx(x, y, w, h, "panel").child(ttl(title))
}

fn stat() -> Node {
    Node::el("div", "status")
}

fn otitle(t: &str) -> Node {
    Node::el("div", "otitle").style("margin-top:6px").text(t)
}

fn doors_of(c: &Ctx) -> [DoorMark; 18] {
    let mut d = c.sim.doors;
    if let PageBody::Doors { doors, .. } = &c.view.body {
        for (i, v) in doors.iter().enumerate().take(18) {
            d[i] = DoorMark::from_view(v);
        }
    }
    d
}

pub fn status(c: &Ctx) -> Vec<Node> {
    let s = c.sim;
    let doors = doors_of(c);
    let open = doors.iter().filter(|d| matches!(d, DoorMark::Open | DoorMark::Moving)).count();
    let armed = doors[..16].iter().filter(|d| **d == DoorMark::Armed).count();
    let known: Vec<f64> = s.main_temp.iter().chain(s.upper_temp.iter()).flatten().copied().collect();
    let avg = if known.is_empty() { None } else { Some(known.iter().sum::<f64>() / known.len() as f64) };
    let waste = s.waste_pct.iter().map(|w| w.unwrap_or(0.)).fold(f64::MIN, f64::max);
    let calls = s.calls;
    let mut v = vec![panel(20, 44, 370, 230, "DOORS / SLIDES")];
    let mut st = stat();
    st = row(st, "DOORS OPEN", &open.to_string(), if open > 0 { AMBER } else { GREEN });
    st = row(st, "SLIDES ARMED", &format!("{armed} OF 16"), "");
    st = row(st, "SLIDE BOTTLES", if s.slide_low { "PRESSURE LOW" } else { "NORMAL" }, if s.slide_low { AMBER } else { "" });
    v.push(bx(30, 68, 350, 196, "inset").child(st));
    v.push(panel(410, 44, 370, 230, "TEMPERATURE / WATER / WASTE"));
    let mut st = stat();
    st = row(st, "CABIN AVERAGE", &fmt(avg, 1, " °C"), "");
    st = row(st, "SELECTED", &fmt(s.purs_sel, 1, " °C"), "");
    st = row(st, "POTABLE WATER", &fmt(s.water_pct, 0, " %"), "");
    st = row(st, "WASTE (HIGHEST TANK)", &format!("{} %", waste.round()), if waste > 90. { AMBER } else { "" });
    v.push(bx(420, 68, 350, 196, "inset").child(st));
    v.push(panel(20, 290, 370, 228, "CABIN"));
    let mut st = stat();
    st = row(st, "CABIN READY", if s.cabin_ready { "SENT TO COCKPIT" } else { "NOT SENT" }, if s.cabin_ready { GREEN } else { "" });
    st = row(st, "CREW CALLS", &calls.to_string(), if calls != 0. { "#6fe3ff" } else { "" });
    st = row(st, "IN-SEAT POWER", if s.seat_power_cmd { "ON" } else { "OFF" }, "");
    st = row(st, "EVACUATION", if s.evac { "ALERT" } else { "NORMAL" }, if s.evac { "#ff5a4a" } else { "" });
    v.push(bx(30, 314, 350, 194, "inset").child(st));
    let svg = plan(370., 228., |p| {
        DOORS
            .iter()
            .zip(doors.iter())
            .map(|(d, m)| {
                let (x, y) = p.p(d.1, d.2);
                shape("circle", &[("cx", &f1(x)), ("cy", &f1(y)), ("r", "3"), ("fill", m.colour())], 0)
            })
            .collect()
    });
    v.push(bx(410, 290, 370, 228, "inset").child(svg));
    v
}

pub fn ready(c: &Ctx) -> Vec<Node> {
    let (areas, sent) = match &c.view.body {
        PageBody::Ready { areas, ready, .. } => (*areas, *ready),
        _ => ([false; 6], c.sim.cabin_ready),
    };
    let all = areas.iter().all(|a| *a);
    let mut v = vec![panel(20, 44, 480, 474, "AREA STATUS")];
    for (k, (_, label)) in AREAS.iter().enumerate() {
        let ok = areas[k];
        let y = 78 + k as i32 * 62;
        v.push(
            Node::new("div")
                .style(&format!("position:absolute;left:36px;top:{y}px;width:300px;height:46px;display:flex;align-items:center;font:700 13px B612;color:#2a2e33"))
                .text(label),
        );
        v.push(key(340, 74 + k as i32 * 62, 140, 50, "area", if ok { "AREA READY" } else { "NOT READY" }, if ok { "on" } else { "" }));
    }
    v.push(
        panel(520, 44, 260, 474, "CABIN READY").child(
            Node::el("div", "note")
                .style("padding:6px 14px")
                .text("When every area reports ready, the purser sends CABIN READY to the cockpit (ECAM)."),
        ),
    );
    v.push(key(560, 160, 180, 70, "cabinready", "CABIN<br>READY", if sent { "on" } else if all { "" } else { "off" }));
    let msg = if sent {
        "CABIN READY SENT"
    } else if all {
        "ALL AREAS READY"
    } else {
        "WAITING FOR AREAS"
    };
    v.push(bx(540, 260, 220, 80, "inset").child(stat().text(msg)));
    v
}

pub fn music(c: &Ctx) -> Vec<Node> {
    let playing = matches!(c.view.body, PageBody::Music { playing: true, .. });
    let mut v = vec![panel(20, 48, 330, 250, "BOARDING MUSIC")
        .child(Node::el("div", "note").style("padding:4px 14px").text("Cabin boarding music on the passenger address system, on every deck."))];
    v.push(key(48, 140, 120, 56, "music:1", "PLAY", if playing { "on" } else { "" }));
    v.push(key(200, 140, 120, 56, "music:0", "STOP", "red"));
    let mut st = stat().text("CHANNEL").child(Node::el("div", "s").text("BOARDING"));
    st = st.text("STATE").child(Node::el("div", "s").text(if playing { "PLAYING" } else { "STOPPED" }));
    v.push(bx(20, 318, 330, 150, "inset").child(otitle("MUSIC STATUS")).child(st));
    let fill = if playing { "#7cf04a" } else { "#6a6d71" };
    let svg = plan(400., 470., |p| {
        DOORS
            .iter()
            .filter(|d| d.3 != 'C')
            .map(|d| {
                let (x, y) = p.p(d.1 * 0.75, d.2);
                shape("circle", &[("cx", &f1(x)), ("cy", &f1(y)), ("r", "4"), ("fill", fill)], 0)
            })
            .collect()
    });
    v.push(bx(380, 48, 400, 470, "inset").child(svg));
    v
}

pub fn preann(c: &Ctx) -> Vec<Node> {
    let (sel, auto) = match &c.view.body {
        PageBody::PreAnn { selected, auto_enabled, .. } => (selected.unwrap_or(0), *auto_enabled),
        _ => (0, c.sim.ann_enabled),
    };
    let mut v = vec![panel(20, 48, 470, 330, "SELECT ANNOUNCEMENT")];
    for (k, (id, label)) in ANNOUNCEMENTS.iter().enumerate() {
        let k = k as i32;
        v.push(key(40 + (k % 5) * 88, 82 + (k / 5) * 72, 78, 56, "pa", label, if sel == *id { "on" } else { "" }));
    }
    v.push(key(40, 250, 166, 46, "pa:0", "STOP ANNOUNCEMENT", "red"));
    let name = ANNOUNCEMENTS.iter().find(|a| a.0 == sel).map(|a| a.1.replacen("<br>", " ", 1).to_uppercase()).unwrap_or_else(|| "NONE".into());
    let st = stat()
        .text("SELECTED")
        .child(Node::el("div", "s").text(&name))
        .text("AUTOMATIC BY FLIGHT PHASE")
        .child(Node::el("div", "s").text(if auto { "ON" } else { "OFF" }));
    v.push(bx(20, 398, 470, 120, "inset").child(otitle("ANNOUNCEMENT STATUS")).child(st));
    let svg = plan(270., 470., |p| {
        let (x0, y0) = p.p(-2., -30.);
        let (_, y1) = p.p(0., 30.);
        let w = p.p(2., 0.).0 - x0;
        let fill = if sel > 1 { "rgba(124,240,74,0.35)" } else { "none" };
        vec![shape("rect", &[("x", &f1(x0)), ("y", &f1(y0)), ("width", &f1(w)), ("height", &f1(y1 - y0)), ("fill", fill)], 0)]
    });
    v.push(bx(510, 48, 270, 470, "inset").child(svg));
    v
}

pub fn lights(c: &Ctx) -> Vec<Node> {
    let (scene, level_raw) = match &c.view.body {
        PageBody::Lights { scene, level_pct, .. } => (scene.unwrap_or(0), level_pct.map(|p| p as i32 + 1).unwrap_or(0)),
        _ => (c.sim.scene_raw as u8, c.sim.level_raw as i32),
    };
    let pct = if level_raw != 0 { level_raw - 1 } else { 100 };
    let cur = SCENES.iter().find(|s| s.0 == scene);
    let mut v = vec![bx(150, 40, 250, 220, "inset").child(Node::el("div", "otitle").style("margin-top:5px").text("SELECT LIGHT SCENE"))];
    for (k, (id, label, colour)) in SCENES.iter().enumerate() {
        let k = k as i32;
        let mut n = key(162 + (k % 4) * 58, 64 + (k / 4) * 62, 52, 54, "scene", "", if scene == *id { "on" } else { "" });
        if let Some(col) = colour {
            n = n.child(Node::new("div").style(&format!("position:absolute;left:4px;right:4px;top:4px;height:9px;border-radius:2px;background:{col}")));
        }
        v.push(n.child(Node::new("span").style("margin-top:12px").text(label)));
    }
    v.push(bx(420, 40, 170, 220, "inset").child(Node::el("div", "otitle").style("margin-top:5px").text("BRIGHTNESS")));
    v.push(key(432, 66, 54, 40, "level:100", "BRT<br>100%", if pct == 100 && level_raw != 0 { "on" } else { "" }));
    v.push(key(432, 116, 54, 40, "level:25", "DIM<br>25%", if pct == 25 { "on" } else { "" }));
    v.push(key(432, 166, 54, 40, "level:0", "OFF<br>0%", if pct == 0 && level_raw != 0 { "on" } else { "" }));
    let mut wedge = Node::new("svg").style("position:absolute;left:500px;top:64px");
    wedge.svgattr.insert("width".into(), "30".into());
    wedge.svgattr.insert("height".into(), "150".into());
    wedge.shapes.push(shape("polygon", &[("points", "2,0 28,0 15,150"), ("fill", "#ff9a3a")], 0));
    v.push(wedge);
    let thumb_top = (1. - pct as f64 / 100.) * 140.;
    v.push(
        Node::new("div")
            .style("position:absolute;left:540px;top:64px;width:26px;height:150px;border-radius:4px;background:linear-gradient(90deg,#1e2022,#4b4e52,#1e2022);border:1px solid #111")
            .child(Node::new("div").style(&format!(
                "position:absolute;left:-4px;right:-4px;top:{thumb_top}px;height:10px;border-radius:3px;background:linear-gradient(#b8f58a,#46a81b);border:1px solid #22420f"
            ))),
    );
    v.push(bx(500, 222, 70, 26, "inset").child(Node::new("div").style("text-align:center;font:700 14px 'B612';line-height:22px").text(&format!("{pct} %"))));
    v.push(bx(150, 280, 440, 150, "panel").child(Node::el("div", "ttl").style("text-align:left;padding-left:10px").text("LIGHT STATUS")));
    let scene_name = cur.map(|s| s.1.to_uppercase()).unwrap_or_else(|| "SET BY EFB".into());
    let level_txt = if level_raw != 0 { format!("{pct}%") } else { "SET BY EFB".into() };
    let mut st = stat().text("SELECTED SCENE").child(Node::el("div", "s").text(&scene_name));
    st = st.text("BRIGHTNESS LEVEL").child(Node::el("div", "s").text(&level_txt));
    if cur.map(|s| s.0) == Some(12) {
        st = st.child(Node::el("div", "w").text("SCENE FOLLOWS THE FLIGHT PHASE"));
    }
    v.push(bx(160, 302, 420, 116, "inset").child(st));
    v.push(key(150, 446, 120, 34, "lightsreset", "RESET TO EFB", ""));
    let glow = cur.and_then(|s| s.2).unwrap_or("#fff2e0");
    let opacity = format!("{}", 0.15 + 0.6 * pct as f64 / 100.);
    let svg = plan(160., 470., |p| {
        let (x0, y0) = p.p(-2.6, -31.);
        let (x1, y1) = p.p(2.6, 30.);
        vec![shape(
            "rect",
            &[("x", &f1(x0)), ("y", &f1(y0)), ("width", &f1(x1 - x0)), ("height", &f1(y1 - y0)), ("rx", "6"), ("fill", glow), ("opacity", &opacity)],
            0,
        )]
    });
    v.push(bx(620, 40, 160, 470, "inset").child(svg));
    v
}

fn legend_row(colour: &str, text: &str) -> Node {
    let sw = Node::el("div", "sw").style(&format!("background:{colour}"));
    Node::el("div", "row").child(sw).child(lines(Node::new("div"), text))
}

pub fn doors(c: &Ctx) -> Vec<Node> {
    let marks = doors_of(c);
    let deck = |on: bool, text: &str| {
        Node::el("div", "row")
            .style("cursor:pointer")
            .child(Node::el("div", "sw").style(&format!("width:10px;height:10px;background:{}", if on { "#2a2e33" } else { "#fff" })))
            .text(text)
    };
    let legend = Node::el("div", "legend")
        .child(legend_row(DoorMark::Armed.colour(), "Door locked<br>Slide armed"))
        .child(legend_row(DoorMark::Disarmed.colour(), "Door locked<br>Slide disarmed"))
        .child(legend_row(DoorMark::Moving.colour(), "Door moving"))
        .child(legend_row(DoorMark::Open.colour(), "Door open"))
        .child(Node::new("div").style("border-top:1px solid #8d9196;margin:10px 8px"))
        .child(deck(c.ui.show_main, "Middle deck M1 - M5"))
        .child(deck(c.ui.show_upper, "Upper deck U1 - U3"));
    let note = Node::el("div", "note")
        .style("position:absolute;left:10px;right:10px;bottom:10px")
        .text("Tap a door to open or close it. Only available with the aircraft stationary and all engines shut down.");
    let mut v = vec![panel(20, 44, 160, 474, "LEGEND").child(legend).child(note)];
    v.push(bx(196, 44, 584, 474, "panel").child(Node::el("div", "ttl").style("text-align:left;padding-left:8px").text("MIDDLE & UPPER DECK DOORS")));
    let (show_main, show_upper) = (c.ui.show_main, c.ui.show_upper);
    let svg = plan(570., 442., |p| {
        let mut out = Vec::new();
        for (i, d) in DOORS.iter().enumerate() {
            if (d.3 == 'M' || d.3 == 'C') && !show_main || d.3 == 'U' && !show_upper {
                continue;
            }
            let col = marks[i].colour();
            let left = d.1 < 0.;
            let inboard = if d.3 == 'U' { 0.55 } else { 0. };
            let (x, y) = p.p(d.1 + if left { inboard } else { -inboard }, d.2);
            out.push(shape("g", &[], 0));
            if d.3 == 'C' {
                let lx = x + 62.;
                out.push(shape("line", &[("x1", &f1(x + 5.)), ("y1", &f1(y)), ("x2", &f1(lx - 3.)), ("y2", &f1(y)), ("stroke", "#9a9ea3"), ("stroke-width", "0.8")], 1));
            }
            if d.3 == 'U' {
                out.push(shape("circle", &[("cx", &f1(x)), ("cy", &f1(y)), ("r", "5"), ("fill", col)], 1));
            } else {
                out.push(shape(
                    "rect",
                    &[("x", &f1(x - 4.)), ("y", &f1(y - 7.)), ("width", "8"), ("height", "14"), ("fill", col), ("stroke", "#111"), ("stroke-width", "0.8")],
                    1,
                ));
            }
        }
        out
    });
    v.push(bx(203, 66, 570, 442, "inset").child(svg));
    v
}

fn chip_colour(t: Option<f64>) -> &'static str {
    match t {
        None => "#5b5e62",
        Some(t) if t < 20. => "#3c6fb5",
        Some(t) if t > 26. => "#b54a3c",
        _ => "#3f8f55",
    }
}

pub fn temp(c: &Ctx) -> Vec<Node> {
    let s = c.sim;
    let mut v = vec![bx(20, 44, 520, 474, "panel").child(Node::el("div", "ttl").style("text-align:left;padding-left:8px").text("ZONE TEMPERATURES (°C)"))];
    let svg = plan(506., 442., |p| {
        let mut out = Vec::new();
        let mut chip = |(x, y): (f64, f64), t: Option<f64>| {
            out.push(shape(
                "rect",
                &[("x", &f1(x - 23.)), ("y", &f1(y - 13.)), ("width", "46"), ("height", "26"), ("rx", "3"), ("fill", chip_colour(t)), ("stroke", "#111")],
                0,
            ));
        };
        chip(p.p(0., -31.5), s.ckpt_temp);
        for (i, t) in s.main_temp.iter().enumerate() {
            chip(p.p(-9.5, -27. + i as f64 * 7.4), *t);
        }
        for (i, t) in s.upper_temp.iter().enumerate() {
            chip(p.p(9.5, -17. + i as f64 * 6.4), *t);
        }
        chip(p.p(-17., -21.), s.cargo_fwd_temp);
        chip(p.p(17., 17.), s.cargo_bulk_temp);
        out
    });
    v.push(bx(27, 66, 506, 442, "inset").child(svg));
    v.push(panel(556, 44, 224, 250, "CABIN TEMPERATURE SELECTION"));
    v.push(key(574, 90, 56, 48, "temp:0.5", "\u{25b2}", ""));
    v.push(key(574, 150, 56, 48, "temp:-0.5", "\u{25bc}", ""));
    v.push(bx(642, 112, 120, 50, "inset").child(Node::new("div").style("text-align:center;font:700 22px 'B612';line-height:46px").text(&fmt(s.purs_sel, 1, " °C"))));
    v.push(
        bx(566, 210, 204, 76, "")
            .style("left:566px;top:210px;width:204px;height:76px;position:absolute")
            .child(Node::el("div", "note").text("The zones follow this selection when the cockpit CABIN selector is at PURS SEL. Range 18 to 30 °C.")),
    );
    if s.purser_sel_fault {
        v.push(bx(556, 310, 224, 60, "inset").child(lines(Node::el("div", "status w"), "PURSER TEMP SEL FAULT<br>COCKPIT CONTROL ONLY")));
    }
    v
}

fn bar(x: i32, label: &str, pct: Option<f64>, litres: Option<f64>, colour: &str) -> Vec<Node> {
    let p = pct.unwrap_or(0.).clamp(0., 100.);
    let l = match litres {
        Some(l) => format!("{l:.0} L / {:.0} US GAL", l / 3.785),
        None => "--".into(),
    };
    vec![
        Node::el("div", "scale").style(&format!("position:absolute;left:{x}px;top:76px;width:70px;text-align:center")).text(label),
        Node::new("div")
            .style(&format!(
                "position:absolute;left:{}px;top:96px;width:34px;height:200px;border-radius:5px;border:1px solid #555;background:linear-gradient(90deg,#d7d9db,#f6f7f8,#c9ccd0);overflow:hidden",
                x + 18
            ))
            .child(Node::new("div").style(&format!("position:absolute;left:0;right:0;bottom:0;height:{}px;background:{colour}", p * 2.))),
        Node::new("div").style(&format!("position:absolute;left:{x}px;top:302px;width:70px;text-align:center;font:700 13px 'B612'")).text(&fmt(pct, 0, " %")),
        Node::el("div", "scale").style(&format!("position:absolute;left:{x}px;top:320px;width:70px;text-align:center")).text(&l),
    ]
}

pub fn water(c: &Ctx) -> Vec<Node> {
    let s = c.sim;
    let mut v = vec![bx(20, 44, 470, 330, "panel").child(
        Node::new("div")
            .style("display:flex")
            .child(Node::el("div", "ttl").style("flex:3").text("WASTE QUANTITY"))
            .child(Node::el("div", "ttl").style("flex:1.3").text("WATER QUANTITY")),
    )];
    for i in 0..3 {
        let w = s.waste_pct[i];
        let colour = if w.unwrap_or(0.) > 90. { "#c96a1b" } else { "#7d6c4e" };
        v.extend(bar(20 + i as i32 * 90, &format!("TANK {}", ZONES[i]), w, w.map(|w| w * 2.), colour));
    }
    v.extend(bar(340, "POTABLE", s.water_pct, s.water_kg, "#2f7fd0"));
    let lav: Vec<&str> = (0..3).filter(|i| s.lav_inop[*i]).map(|i| ZONES[i]).collect();
    let lav = if lav.is_empty() { "NONE".to_string() } else { lav.join(" ") };
    let mut st = Node::el("div", "status").style("columns:2");
    st = st.text("PRESSURE").child(Node::el("div", "s").text(&fmt(s.water_press, 1, " PSI")));
    st = st.text("COMPRESSOR").child(Node::el("div", "s").text(if s.compressor { "ON" } else { "OFF" }));
    st = st.text("LEAK").child(Node::el("div", "s").text(if s.leak { "DETECTED" } else { "NONE" }));
    st = st.text("VACUUM GENERATOR").child(Node::el("div", "s").text(if s.generator { "RUNNING" } else { "OFF" }));
    st = st.text("DRAIN MASTS").child(Node::el("div", "s").text(if s.mast_blocked { "BLOCKED" } else { "CLEAR" }));
    st = st.text("LAVATORIES INOP").child(Node::el("div", "s").text(&lav));
    v.push(bx(20, 390, 470, 128, "inset").child(otitle("SYSTEM STATUS")).child(st));
    v.push(panel(506, 44, 274, 230, "WATER HEATERS"));
    for i in 0..3 {
        v.push(
            Node::el("div", "row")
                .style(&format!(
                    "position:absolute;left:520px;top:{}px;width:240px;display:flex;justify-content:space-between;font:700 12px 'B612';color:#2a2e33",
                    70 + i * 22
                ))
                .child(Node::new("span").text(&format!("HEATER {}", ZONES[i])))
                .child(Node::new("span").text(&fmt(s.heater_temp[i], 0, " °C"))),
        );
    }
    let off = s.heaters_off;
    v.push(key(560, 150, 160, 50, "heaters", if off { "WATER HEATERS<br>OFF" } else { "WATER HEATERS<br>ON" }, if off { "" } else { "on" }));
    v
}

fn detectors(s: &FapSim) -> Vec<(String, Option<f64>, f64, f64)> {
    let mut d = vec![("FWD CARGO".to_string(), s.cargo_smoke[0], 1.6, -23.4), ("AFT CARGO".to_string(), s.cargo_smoke[1], 1.6, 13.)];
    for i in 0..3 {
        d.push((format!("IFE {}", ZONES[i]), s.ife_smoke[i], -1.6, -20. + i as f64 * 18.));
    }
    for i in 0..3 {
        d.push((format!("GALLEY {}", ZONES[i]), s.galley_smoke[i], 0., -26. + i as f64 * 22.));
    }
    d
}

pub fn smoke(c: &Ctx) -> Vec<Node> {
    let det = detectors(c.sim);
    let mut v = vec![panel(20, 44, 300, 474, "DETECTORS")];
    for (k, (label, s, _, _)) in det.iter().enumerate() {
        let (col, txt) = match s {
            None => ("#777", "--"),
            Some(x) if *x >= 0.5 => ("#d8473a", "SMOKE"),
            _ => ("#25cf3a", "NORMAL"),
        };
        v.push(
            Node::new("div")
                .style(&format!(
                    "position:absolute;left:34px;top:{}px;width:272px;height:26px;display:flex;align-items:center;justify-content:space-between;background:#eceef0;border:1px solid #8d9196;border-radius:3px;padding:0 8px;box-sizing:border-box;font:700 12px 'B612';color:#2a2e33",
                    74 + k * 34
                ))
                .child(Node::new("span").text(label))
                .child(Node::new("span").style(&format!("color:{col}")).text(txt)),
        );
    }
    let svg = plan(444., 474., |p| {
        det.iter()
            .map(|(_, s, x, z)| {
                let (px, py) = p.p(*x, *z);
                let fill = match s {
                    None => "#777",
                    Some(v) if *v >= 0.5 => "#ff4a3a",
                    _ => "#25cf3a",
                };
                shape("circle", &[("cx", &f1(px)), ("cy", &f1(py)), ("r", "7"), ("fill", fill), ("stroke", "#111")], 0)
            })
            .collect()
    });
    v.push(bx(336, 44, 444, 474, "inset").child(svg));
    v
}

pub fn seats(c: &Ctx) -> Vec<Node> {
    let s = c.sim;
    let off = match &c.view.body {
        PageBody::Seats { seat_power, .. } => !seat_power,
        _ => s.seat_power_off,
    };
    let mut v = Vec::new();
    for i in 0..3 {
        let x = 20 + i as i32 * 256;
        v.push(panel(x, 44, 244, 200, &format!("ZONE {}", ZONES[i])));
        let power = if s.ife_tripped[i] {
            "TRIPPED"
        } else if s.seat_power_cmd {
            "ON"
        } else {
            "OFF"
        };
        let mut st = stat().text("IN-SEAT POWER").child(Node::el("div", "s").text(power));
        st = st.text("EQUIPMENT TEMP").child(Node::el("div", "s").text(&fmt(s.ife_temp[i], 0, " °C")));
        if s.ife_hot[i] {
            st = st.child(Node::el("div", "w").text("OVERHEAT"));
        }
        v.push(bx(x + 10, 70, 224, 160, "inset").child(st));
    }
    v.push(
        panel(20, 262, 500, 140, "IN-SEAT POWER SUPPLY").child(
            Node::el("div", "note")
                .style("padding:4px 14px")
                .text("Seat power is shed with the galleys when commercial power is lost, and per zone when a zone trips."),
        ),
    );
    v.push(key(60, 330, 180, 54, "seatpower", if off { "SEAT POWER<br>OFF" } else { "SEAT POWER<br>ON" }, if off { "" } else { "on" }));
    v.push(panel(540, 262, 240, 140, "PASSENGER CALLS"));
    v.push(
        bx(550, 286, 220, 40, "inset")
            .child(Node::el("div", "status").style("padding:10px 14px").text("ACTIVE CALLS").child(Node::el("span", "s").text(&s.calls.to_string()))),
    );
    v.push(key(580, 340, 160, 50, "callreset", "CALL RESET", "blue"));
    v
}

pub fn sysinfo(c: &Ctx) -> Vec<Node> {
    let s = c.sim;
    let faults = match &c.view.body {
        PageBody::SysInfo { directors, .. } => [!directors[0], !directors[1], !directors[2]],
        _ => s.cids_fault,
    };
    let st_txt = |bad: bool| if bad { "FAULT" } else { "NORMAL" };
    let mut v = vec![panel(20, 44, 250, 230, "CIDS / IFE")];
    let mut st = stat();
    for (i, f) in faults.iter().enumerate() {
        st = st.text(&format!("DIRECTOR {}", i + 1)).child(Node::el("div", "s").text(st_txt(*f)));
    }
    for (i, f) in s.ife_server_fail.iter().enumerate() {
        st = st.text(&format!("IFE SERVER {}", i + 1)).child(Node::el("div", "s").text(st_txt(*f)));
    }
    v.push(bx(30, 68, 230, 196, "inset").child(st));
    v.push(panel(20, 288, 250, 230, "VENTILATION"));
    let mut st = stat();
    st = st.text("LAV / GALLEY EXTRACT").child(Node::el("div", "s").text(st_txt(s.extract_fault)));
    st = st.text("SECONDARY FANS FAILED").child(Node::el("div", "s").text(&s.fans_failed.to_string()));
    st = st.text("IFE BAY").child(Node::el("div", "s").text(st_txt(s.ife_bay_fault)));
    v.push(bx(30, 312, 230, 196, "inset").child(st));
    for i in 0..3 {
        let y = 44 + i as i32 * 160;
        let off = s.galley_off[i];
        v.push(bx(286, y, 494, 150, "panel").child(Node::el("div", "ttl").style("text-align:left;padding-left:8px").text(&format!("GALLEY {}", ZONES[i]))));
        let temps = format!("{} / {} / {} °C", fmt(s.oven[i], 0, ""), fmt(s.chiller[i], 1, ""), fmt(s.boiler[i], 0, ""));
        let st = Node::el("div", "status")
            .style("padding:6px 12px")
            .text("SUPPLY")
            .child(Node::el("div", "s").text(st_txt(s.galley_bus_fault[i])))
            .text("OVEN / CHILLER / BOILER")
            .child(Node::el("div", "s").text(&temps));
        v.push(bx(296, y + 24, 300, 116, "inset").child(st));
        v.push(key(620, y + 44, 140, 64, "galley", if off { "GALLEY POWER<br>OFF" } else { "GALLEY POWER<br>ON" }, if off { "" } else { "on" }));
    }
    v
}

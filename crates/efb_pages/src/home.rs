use crate::fmt::{hhmm, js_round, num, utc_hm, zulu_hm};
use crate::model::{path, path_f64, path_str, EfbModel, Landing};
use crate::{button, label, span, Node};

fn el(tag: &str, classes: &str) -> Node {
    Node::el(tag, classes)
}

fn home_stat(k: &str, big: &str, sub: &str) -> Node {
    el("div", "home-stat")
        .child(label("k", k))
        .child(label("big", big))
        .child(label("sub", sub))
}

fn kv(k: &str, v: &str) -> Node {
    el("div", "home-kv").child(Node::new("span").text(k)).child(span("v", v))
}

fn status_box(m: &EfbModel) -> Node {
    let mut b = el("div", "box").child(label("home-sec", "Aircraft"));
    for line in [&m.status.xplane, &m.status.systems, &m.status.displays] {
        let cls = if line.class.is_empty() { "dot".to_string() } else { format!("dot {}", line.class) };
        b = b.child(el("div", "home-strow").child(el("span", &cls)).child(Node::new("span").text(&line.text)));
    }
    b
}

fn rem_row(classes: &str, title: &str, count: Option<&str>) -> Node {
    let mut r = el("button", classes).child(span("home-rem-dot", "")).child(span("home-rem-t", title));
    if let Some(c) = count {
        r = r.child(span("mono dim", c));
    }
    r
}

fn reminders(m: &EfbModel) -> Node {
    let next = m.checklists.iter().position(|r| r.done < r.total);
    let mut cl = el("div", "home-rem-col").child(label("home-sec", "Checklists"));
    for (i, r) in m.checklists.iter().enumerate() {
        let state = if r.done == r.total {
            "done"
        } else if Some(i) == next {
            "next"
        } else {
            ""
        };
        let classes = format!("home-rem-row {state}");
        cl = cl.child(rem_row(&classes, &r.title, Some(&format!("{}/{}", r.done, r.total))));
    }
    let mut mx = el("div", "home-rem-col").child(label("home-sec", "Maintenance"));
    if !m.pins.is_empty() {
        mx = mx.child(label("home-sec home-rem-sub", "Pinned documents"));
        for t in m.pins.iter().take(8) {
            mx = mx.child(rem_row("home-rem-row pin", t, None));
        }
    }
    match (&m.maintenance, m.offline) {
        (Some(d), _) => {
            let legal = d.legal;
            let (cls, text) = if legal { ("home-rem-row done", "Dispatch: GO") } else { ("home-rem-row nogo", "Dispatch: NO GO") };
            mx = mx.child(rem_row(cls, text, None));
            for b in &d.blocking {
                mx = mx.child(label("label home-rem-note", b));
            }
            for (name, n) in [("Open defects", d.defects), ("MEL deferrals", d.deferrals), ("Tech log entries", d.tech_log)] {
                let cls = if n > 0 && name == "Open defects" { "home-rem-row next" } else { "home-rem-row" };
                mx = mx.child(rem_row(cls, name, Some(&n.to_string())));
            }
            if let Some(h) = d.airframe_hours {
                mx = mx.child(label("label dim home-rem-note", &format!("Airframe {} h", num(Some(h.round()), 0))));
            }
        }
        (None, true) => mx = mx.child(label("label dim", "Maintenance status needs X-Plane running.")),
        (None, false) => mx = mx.child(label("label dim", "Reading the tech log\u{2026}")),
    }
    el("div", "box home-rem").child(cl).child(mx)
}

fn grade_class(l: &Landing) -> &'static str {
    if l.grade.starts_with("Hard") {
        "hard"
    } else if l.grade == "Firm" {
        "firm"
    } else {
        "ok"
    }
}

fn landings(m: &EfbModel) -> Node {
    let mut box_ = el("div", "box lr-box").child(label("home-sec", "Landings"));
    let Some(l) = m.landings.first() else {
        let t = if m.offline {
            "Touchdowns appear here once X-Plane is running."
        } else {
            "No landing recorded yet. Each touchdown is graded against the A380 limit descent velocity."
        };
        return box_.child(label("label dim", t));
    };
    let cls = grade_class(l);
    box_ = box_.child(
        el("div", &format!("lr-big {cls}"))
            .child(span("mono", &format!("{} fpm", js_round(l.vs_fpm))))
            .child(span("mono dim", &format!("{:.2} g", l.g))),
    );
    let airport = l.airport.clone().unwrap_or_default();
    let line: Vec<String> = [airport.clone(), zulu_hm(l.zulu_s), l.grade.clone()].into_iter().filter(|s| !s.is_empty()).collect();
    box_ = box_.child(label(&format!("label lr-grade {cls}"), &line.join("  \u{b7}  ")));
    box_ = box_
        .child(kv("Speed", &format!("{} kt IAS / {} kt GS", js_round(l.ias_kt), js_round(l.gs_kt))))
        .child(kv("Pitch / bank", &format!("{:.1}\u{b0} / {:.1}\u{b0}", l.pitch_deg, l.roll_deg)))
        .child(kv("Weight", &format!("{:.1} t (limit {} fpm)", l.weight_kg / 1000.0, js_round(l.limit_fpm))));
    if l.bounces > 0 {
        box_ = box_.child(kv("Bounces", &l.bounces.to_string()));
    }
    if m.landings.len() > 1 {
        box_ = box_.child(el("div", "home-hr"));
        for p in m.landings.iter().skip(1).take(5) {
            let when = [zulu_hm(p.zulu_s), p.airport.clone().unwrap_or_default()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("  ");
            let c = grade_class(p);
            box_ = box_.child(
                el("div", "home-kv lr-row")
                    .child(Node::new("span").text(&when))
                    .child(span(&format!("v lr-{c}"), &format!("{} fpm  {:.2} g", js_round(p.vs_fpm), p.g))),
            );
        }
    }
    box_
}

fn route_box(ofp: &serde_json::Value) -> Node {
    let org = |k: &str| path_str(ofp, &["origin", k]);
    let dst = |k: &str| path_str(ofp, &["destination", k]);
    let route = path_str(ofp, &["general", "route"]);
    let mut b = el("div", "box").style("flex:1");
    b = b.child(label("home-sec", &format!("Route / {} NM", num(path_f64(ofp, &["general", "route_distance"]), 0))));
    b = b.child(label(
        "home-route",
        &format!("{}/{} {} {}/{}", org("icao_code"), org("plan_rwy"), route, dst("icao_code"), dst("plan_rwy")),
    ));
    b = b.child(el("div", "home-hr"));
    let unit = {
        let u = path_str(ofp, &["params", "units"]);
        if u.is_empty() { "kgs".to_string() } else { u }
    };
    let mut wcol = el("div", "").child(label("home-sec", &format!("Weights ({unit})")));
    for (k, v) in [
        ("Zero fuel", path_f64(ofp, &["weights", "est_zfw"])),
        ("Block fuel", path_f64(ofp, &["fuel", "plan_ramp"])),
        ("Take off", path_f64(ofp, &["weights", "est_tow"])),
        ("Landing", path_f64(ofp, &["weights", "est_ldw"])),
    ] {
        wcol = wcol.child(kv(k, &num(v, 0)));
    }
    let xcol = el("div", "home-metar").child(label("home-sec", "Weather"));
    b.child(el("div", "home-split").child(wcol).child(xcol))
}

fn hero(ofp: &serde_json::Value) -> Node {
    let t = |k: &str| path_f64(ofp, &["times", k]);
    let rwy = |side: &str| {
        let icao = path_str(ofp, &[side, "icao_code"]);
        let r = path_str(ofp, &[side, "plan_rwy"]);
        format!("{}{}", icao, if r.is_empty() { String::new() } else { format!(" RWY {r}") }).trim().to_string()
    };
    let alt = path_f64(ofp, &["general", "initial_altitude"]).map(|a| (a / 100.0).round() as i64).filter(|a| *a != 0);
    let ci = path(ofp, &["general", "costindex"]);
    let ci_text = if ci.is_null() { "--".to_string() } else { crate::model::jstr(ci) };
    el("div", "box home-hero")
        .child(home_stat("Off block", &format!("{}Z", utc_hm(t("est_out"))), &rwy("origin")))
        .child(home_stat("Block time", &hhmm(t("est_block")), &format!("air {}", hhmm(t("est_time_enroute")))))
        .child(home_stat("Cruise", &format!("FL{}", alt.map_or("---".to_string(), |a| a.to_string())), &format!("CI {ci_text}")))
        .child(home_stat("On block", &format!("{}Z", utc_hm(t("est_in"))), &rwy("destination")))
}

pub fn home_page(m: &EfbModel) -> Node {
    let mut left = el("div", "home-col").style("flex:1");
    let mut right = el("div", "home-col").style("width:380px");
    match &m.ofp {
        Some(ofp) => {
            left = left.child(hero(ofp)).child(route_box(ofp));
        }
        None => {
            left = left.child(
                el("div", "box home-empty")
                    .style("flex:1")
                    .child(label("home-empty-big", "No flight plan loaded"))
                    .child(label("label dim", "Fetch your SimBrief OFP to see the route, times, weights and weather here."))
                    .child(el("div", "row").style("margin-top:14px").child(button("", "Open flight plan"))),
            );
        }
    }
    left = left.child(reminders(m));
    right = right.child(status_box(m)).child(landings(m));
    if let Some(ofp) = &m.ofp {
        let org = path_str(ofp, &["origin", "name"]);
        let dst = path_str(ofp, &["destination", "name"]);
        let flt = format!("{}{}", path_str(ofp, &["general", "icao_airline"]), path_str(ofp, &["general", "flight_number"]));
        right = right.child(
            el("div", "box")
                .child(label("home-sec", "Flight"))
                .child(label("home-flt", &flt))
                .child(label("label dim", &org).style("margin-top:4px"))
                .child(label("label dim", &if dst.is_empty() { String::new() } else { format!("to {dst}") })),
        );
    }
    Node::el("div", "home-wrap").id("home-content").child(left).child(right)
}

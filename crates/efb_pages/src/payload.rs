use crate::js::{el, locale_int, num, round};
use crate::jv::{a, f, fx, n, s, sv, Svg, Vars};
use crate::{button, label, span, Node};
use serde_json::{Map, Value};

#[derive(Clone, Debug, PartialEq)]
pub enum LoadsheetSource {
    LoadFailed,
    NoPage { preview: bool },
}

pub fn loadsheet_content(src: &LoadsheetSource) -> Node {
    let text = match src {
        LoadsheetSource::LoadFailed => "Could not load the loadsheet.",
        LoadsheetSource::NoPage { preview: true } => "Payload and fuel load once X-Plane is running with the A380.",
        LoadsheetSource::NoPage { preview: false } => "The plugin did not offer a loadsheet page.",
    };
    Node::el("div", "page-main").id("loadsheet-content").child(Node::el("div", "box").text(text))
}

fn fill_colour(frac: f64) -> &'static str {
    if frac >= 0.9 {
        "#d8a23a"
    } else if frac > 0.0 {
        "#4f8fd0"
    } else {
        "#2a2f38"
    }
}

const FUS_LEN: f64 = 72.7;
const FUS_W: f64 = 7.1;
const NOSE_LEN: f64 = 9.0;
const TAIL_LEN: f64 = 13.0;
const SPAN: f64 = 79.75;
const ROOT_CHORD: f64 = 17.7;
const TIP_CHORD: f64 = 4.0;
const SWEEP: f64 = 33.5;
const ROOT_LE_Y: f64 = 27.0;
const HTP_SPAN: f64 = 30.4;
const HTP_ROOT_S: f64 = 2.0;
const HTP_ROOT_CHORD: f64 = 7.0;
const HTP_TIP_CHORD: f64 = 2.2;
const HTP_SWEEP: f64 = 34.0;
const HTP_ROOT_LE_Y: f64 = 63.0;
const ENG_IN: f64 = 14.0;
const ENG_OUT: f64 = 26.0;
const ENG_LEN: f64 = 6.0;
const ENG_W: f64 = 3.2;

fn half_fus() -> f64 {
    FUS_W / 2.0
}
fn half_span() -> f64 {
    SPAN / 2.0
}
fn half_htp() -> f64 {
    HTP_SPAN / 2.0
}
fn wing_le(s: f64) -> f64 {
    ROOT_LE_Y + (s.abs() - half_fus()) * (SWEEP * std::f64::consts::PI / 180.0).tan()
}
fn wing_chord(s: f64) -> f64 {
    ROOT_CHORD + ((TIP_CHORD - ROOT_CHORD) * (s.abs() - half_fus())) / (half_span() - half_fus())
}
fn wing_te(s: f64) -> f64 {
    wing_le(s) + wing_chord(s)
}
fn htp_le(s: f64) -> f64 {
    HTP_ROOT_LE_Y + (s.abs() - HTP_ROOT_S) * (HTP_SWEEP * std::f64::consts::PI / 180.0).tan()
}
fn htp_chord(s: f64) -> f64 {
    HTP_ROOT_CHORD + ((HTP_TIP_CHORD - HTP_ROOT_CHORD) * (s.abs() - HTP_ROOT_S)) / (half_htp() - HTP_ROOT_S)
}
fn htp_te(s: f64) -> f64 {
    htp_le(s) + htp_chord(s)
}

fn pts_attr(p: &[[f64; 2]]) -> String {
    p.iter().map(|q| format!("{},{}", n(q[0]), n(q[1]))).collect::<Vec<_>>().join(" ")
}

fn fuselage_d() -> String {
    let r = half_fus();
    let (l, cyl_b, cyl_e) = (FUS_LEN, NOSE_LEN, FUS_LEN - TAIL_LEN);
    [
        "M 0 0".to_string(),
        format!("Q {} {} {} {}", fx(r * 1.05, 2), fx(NOSE_LEN * 0.15, 2), n(r), n(cyl_b)),
        format!("L {} {}", n(r), n(cyl_e)),
        format!("Q {} {} {} {}", n(r), fx(l - TAIL_LEN * 0.35, 2), fx(r * 0.18, 2), n(l)),
        format!("L {} {}", fx(-r * 0.18, 2), n(l)),
        format!("Q {} {} {} {}", n(-r), fx(l - TAIL_LEN * 0.35, 2), n(-r), n(cyl_e)),
        format!("L {} {}", n(-r), n(cyl_b)),
        format!("Q {} {} 0 0", fx(-r * 1.05, 2), fx(NOSE_LEN * 0.15, 2)),
        "Z".into(),
    ]
    .join(" ")
}

#[allow(clippy::too_many_arguments)]
fn load_block(g: &mut Svg, pts: [[f64; 2]; 4], frac: f64, from: [f64; 2], to: [f64; 2], lbl: &str, value: &str, dir: f64) {
    g.open("g", &[]);
    g.add("polygon", &[("points", pts_attr(&pts)), ("fill", "#20242c".into()), ("stroke", "#3a4150".into()), ("stroke-width", "0.3".into())]);
    if frac > 0.0 {
        let ff = frac.min(1.0).max(0.0);
        let mid = |p: [f64; 2], q: [f64; 2], t: f64| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
        let fp = [pts[0], pts[1], mid(pts[1], pts[2], ff), mid(pts[0], pts[3], ff)];
        g.add("polygon", &[("points", pts_attr(&fp)), ("fill", fill_colour(frac).into())]);
    }
    g.add("line", &[("x1", n(from[0])), ("y1", n(from[1])), ("x2", n(to[0])), ("y2", n(to[1])), ("stroke", "#5a6272".into()), ("stroke-width", "0.22".into())]);
    if !lbl.is_empty() {
        g.text(&[("x", n(from[0])), ("y", n(from[1] + dir * 2.5)), ("text-anchor", "middle".into()), ("fill", "#8b94a6".into()), ("font-size", "1.7".into())], lbl);
    }
    g.text(&[("x", n(from[0])), ("y", n(from[1] + dir * 0.5)), ("text-anchor", "middle".into()), ("fill", "#dfe4ee".into()), ("font-size", "1.85".into())], value);
    g.close();
}

fn wing_tank_spans(tanks: &[Value]) -> Vec<(String, [f64; 2])> {
    let m_per_unit = SPAN / 960.0;
    let mid = |t: &Value| f(t, "x") + f(t, "w") / 2.0;
    let mut left: Vec<&Value> = tanks.iter().filter(|t| s(t, "label") != "TRIM" && mid(t) < 480.0).collect();
    left.sort_by(|p, q| f(p, "x").partial_cmp(&f(q, "x")).unwrap_or(std::cmp::Ordering::Equal));
    let mut right: Vec<&Value> = tanks.iter().filter(|t| s(t, "label") != "TRIM" && mid(t) >= 480.0).collect();
    right.sort_by(|p, q| f(q, "x").partial_cmp(&f(p, "x")).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = Vec::new();
    for (side, list) in [(-1.0, left), (1.0, right)] {
        let mut edge = side * half_span();
        for (i, t) in list.iter().enumerate() {
            let inner = if i == list.len() - 1 { side * half_fus() } else { edge - side * f(t, "w") * m_per_unit };
            out.push((sv(t, "name", "undefined"), [edge, inner]));
            edge = inner;
        }
    }
    out
}

fn var_of(vars: &Vars, k: &str) -> Option<f64> {
    vars.get(k).map(|v| v.as_f64().unwrap_or(f64::NAN))
}

fn draw_aircraft(page: &Value, vars: &Vars) -> Node {
    let margin = 12.0;
    let mut g = Svg::new("", &[("viewBox", format!("{} -4 {} {}", n(-(half_span() + margin)), n(2.0 * (half_span() + margin)), n(FUS_LEN + 8.0)))]);
    for sign in [-1.0, 1.0] {
        let wing = [[sign * half_fus(), wing_le(half_fus())], [sign * half_span(), wing_le(half_span())], [sign * half_span(), wing_te(half_span())], [sign * half_fus(), wing_te(half_fus())]];
        let htp = [[sign * HTP_ROOT_S, htp_le(HTP_ROOT_S)], [sign * half_htp(), htp_le(half_htp())], [sign * half_htp(), htp_te(half_htp())], [sign * HTP_ROOT_S, htp_te(HTP_ROOT_S)]];
        for p in [wing, htp] {
            g.add("polygon", &[("points", pts_attr(&p)), ("fill", "#1b1f27".into()), ("stroke", "#39404e".into()), ("stroke-width", "0.3".into())]);
        }
    }
    g.add("path", &[("d", fuselage_d()), ("fill", "#252a33".into()), ("stroke", "#454d5e".into()), ("stroke-width", "0.3".into())]);
    for sign in [-1.0, 1.0] {
        for s0 in [ENG_IN, ENG_OUT] {
            let y_aft = wing_le(s0) - 1.0;
            g.add(
                "rect",
                &[
                    ("x", n(sign * s0 - ENG_W / 2.0)),
                    ("y", n(y_aft - ENG_LEN)),
                    ("width", n(ENG_W)),
                    ("height", n(ENG_LEN)),
                    ("rx", n(ENG_W / 2.4)),
                    ("fill", "#2a2f38".into()),
                    ("stroke", "#4a5060".into()),
                    ("stroke-width", "0.25".into()),
                ],
            );
        }
    }
    let tanks = a(page, "tanks");
    let spans = wing_tank_spans(tanks);
    for t in tanks {
        let name = sv(t, "name", "undefined");
        let Some((_, [s_out, s_in])) = spans.iter().find(|(k, _)| *k == name).filter(|_| s(t, "label") != "TRIM") else { continue };
        let q = var_of(vars, &name);
        let frac = q.map(|q| q / f(t, "capacityGal")).unwrap_or(0.0);
        let kg = q.map(|q| num(round(q * 3.03907))).unwrap_or("--".into());
        let pts = [[*s_in, wing_le(*s_in)], [*s_in, wing_te(*s_in)], [*s_out, wing_te(*s_out)], [*s_out, wing_le(*s_out)]];
        let sc = (s_in + s_out) / 2.0;
        let near: Vec<f64> = [ENG_IN, ENG_OUT].into_iter().filter(|es| (sc.abs() - es).abs() < 3.2).collect();
        let natural = wing_le(sc) - 3.4;
        let clear = if near.is_empty() { natural } else { near.iter().map(|es| wing_le(*es) - 1.0 - ENG_LEN - 1.0).fold(f64::INFINITY, f64::min) };
        load_block(&mut g, pts, frac, [sc, natural.min(clear)], [sc, wing_le(sc)], s(t, "label"), &kg, -1.0);
    }
    if let Some(t) = tanks.iter().find(|t| s(t, "label") == "TRIM") {
        let q = var_of(vars, &sv(t, "name", "undefined"));
        let frac = q.map(|q| q / f(t, "capacityGal")).unwrap_or(0.0);
        let kg = q.map(|q| format!("{} kg", num(round(q * 3.03907)))).unwrap_or("--".into());
        let (x0, x1) = (-3.5, 3.5);
        let y0 = htp_le(HTP_ROOT_S) + 1.0;
        let y1 = y0 + 5.2;
        load_block(&mut g, [[x0, y0], [x0, y1], [x1, y1], [x1, y0]], frac, [0.0, y0 - 3.4], [0.0, y0], "TRIM", &kg, -1.0);
    }
    g.done()
}

fn gauge(cls: &str, frac: f64, lbl: &str, value: &str, flex: Option<f64>) -> Node {
    let ff = if frac.is_nan() { frac } else { frac.min(1.0).max(0.0) };
    el("div", cls)
        .style(&flex.map(|b| format!("flex:{} 1 0", num(b))).unwrap_or_default())
        .child(label("lz-label", lbl))
        .child(el("div", "lz-bar").child(el("div", "lz-fill").style(&format!("width:{}%;background:{}", num(ff * 100.0), fill_colour(frac)))))
        .child(label("lz-val", value))
}

fn deck(title: &str, zones: &[&Value], vars: &Vars) -> Node {
    let mut row = el("div", "loadsheet-deck-row");
    for z in zones {
        let pax = var_of(vars, &sv(z, "name", "undefined"));
        let seats = f(z, "seats");
        let frac = pax.map(|p| p / seats).unwrap_or(0.0);
        row = row.child(gauge("loadsheet-zone", frac, s(z, "label"), &pax.map(|p| format!("{}/{}", num(round(p)), num(seats))).unwrap_or("--".into()), Some(seats)));
    }
    el("div", "loadsheet-deck").child(label("loadsheet-deck-title", title)).child(row)
}

fn holds(page: &Value, vars: &Vars) -> Node {
    let mut row = el("div", "loadsheet-holds-row");
    for h in a(page, "holds") {
        let q = var_of(vars, &sv(h, "name", "undefined"));
        let frac = q.map(|q| q / f(h, "maxKg")).unwrap_or(0.0);
        row = row.child(gauge("loadsheet-hold", frac, s(h, "label"), &q.map(|q| format!("{} kg", num(round(q)))).unwrap_or("--".into()), None));
    }
    el("div", "loadsheet-deck").child(label("loadsheet-deck-title", "Cargo")).child(row)
}

fn legend() -> Node {
    let item = |c: &str, t: &str| el("span", "ll-item").child(el("span", "ll-swatch").style(&format!("background:{c}"))).text(t);
    el("div", "loadsheet-legend").child(item("#2a2f38", "Empty")).child(item("#4f8fd0", "Partial")).child(item("#d8a23a", "Past 90%")).child(span("ll-note", "Wing tank figures are kg"))
}

fn diagram(page: &Value, vars: &Vars) -> Node {
    let zones = a(page, "zones");
    let upper: Vec<&Value> = zones.iter().filter(|z| f(z, "deck") == 1.0).collect();
    let main: Vec<&Value> = zones.iter().filter(|z| f(z, "deck") == 0.0).collect();
    el("div", "loadsheet-diagram")
        .child(el("div", "loadsheet-plane-wrap").child(draw_aircraft(page, vars)).child(legend()))
        .child(el("div", "loadsheet-cabin").child(deck("Upper deck", &upper, vars)).child(deck("Main deck", &main, vars)).child(holds(page, vars)))
}

fn seat_order(name: &str, seats: usize, abreast: usize, gaps: &[usize]) -> Vec<(usize, usize)> {
    let mut h: i32 = 2166136261u32 as i32;
    for ch in name.chars() {
        let mut buf = [0u16; 2];
        let unit = ch.encode_utf16(&mut buf)[0] as i32;
        h = (h ^ unit).wrapping_mul(16777619);
    }
    let rnd = |k: usize| -> f64 {
        let mut x = (h ^ (k as i32 + 1)).wrapping_mul(2654435761u32 as i32) as u32;
        x = ((x ^ (x >> 15)) as i32).wrapping_mul(2246822519u32 as i32) as u32;
        let r = (x ^ (x >> 13)) as i32;
        r as f64 / 4294967296.0
    };
    let tier = |row: usize| -> f64 {
        if row == 0 || row == abreast - 1 {
            0.0
        } else if gaps.contains(&row) || (row > 0 && gaps.contains(&(row - 1))) {
            0.45
        } else {
            1.0
        }
    };
    let cols = seats.div_ceil(abreast);
    let mut v: Vec<(usize, f64)> = (0..seats).map(|k| (k, tier(k % abreast) + 0.3 * ((k / abreast) as f64 / cols.max(1) as f64) + rnd(k))).collect();
    v.sort_by(|p, q| p.1.partial_cmp(&q.1).unwrap_or(std::cmp::Ordering::Equal));
    v.into_iter().enumerate().map(|(i, (k, _))| (k, i)).collect()
}

fn seat_map(page: &Value, vars: &Vars) -> Vec<Node> {
    let val = |k: &str| -> f64 {
        match vars.get(k) {
            None => 0.0,
            Some(v) => crate::jv::nf(&serde_json::json!({ "v": v }), "v"),
        }
    };
    let zones = a(page, "zones");
    let (w, padx, aisle, deck_gap) = (1000.0, 34.0, 6.0, 40.0);
    let mut g = Svg::new("seatmap-svg", &[("role", "img".into()), ("aria-label", "Seat map, both decks".into())]);
    let mut y = 22.0;
    let (mut total, mut filled) = (0.0, 0.0);
    for (dk, title, abreast, gaps, seat_h) in [(1.0, "Upper deck", 4usize, [0usize, 2], 13.0), (0.0, "Main deck", 10, [2, 6], 10.0)] {
        let dz: Vec<&Value> = zones.iter().filter(|z| f(z, "deck") == dk).collect();
        let cols: Vec<f64> = dz.iter().map(|z| (f(z, "seats") / abreast as f64).ceil()).collect();
        let sum: f64 = cols.iter().sum();
        let cw = (w - 2.0 * padx) / if sum == 0.0 { 1.0 } else { sum };
        let h = abreast as f64 * seat_h + gaps.len() as f64 * aisle;
        g.text(&[("x", n(padx)), ("y", n(y - 9.0)), ("class", "sm-title".into())], &title.to_uppercase());
        g.add("rect", &[("x", n(padx - 14.0)), ("y", n(y - 6.0)), ("width", n(w - 2.0 * padx + 28.0)), ("height", n(h + 12.0)), ("rx", "16".into()), ("class", "sm-hull".into())]);
        let mut x = padx;
        for (i, z) in dz.iter().enumerate() {
            let seats = f(z, "seats");
            let v = val(&sv(z, "name", "undefined"));
            let pax = round(if v.is_nan() || v == 0.0 { 0.0 } else { v }).min(seats).max(0.0);
            total += seats;
            filled += pax;
            let rank: std::collections::HashMap<usize, usize> = seat_order(&sv(z, "name", "undefined"), seats as usize, abreast, &gaps).into_iter().collect();
            for k in 0..seats as usize {
                let (col, row) = (k / abreast, k % abreast);
                let gg = gaps.iter().filter(|r| row > **r).count();
                g.add(
                    "rect",
                    &[
                        ("x", fx(x + col as f64 * cw + 0.7, 1)),
                        ("y", fx(y + row as f64 * seat_h + gg as f64 * aisle, 1)),
                        ("width", fx((cw - 1.4).max(1.0), 1)),
                        ("height", fx(seat_h - 1.3, 1)),
                        ("rx", "1".into()),
                        ("class", if (rank[&k] as f64) < pax { "sm-seat on" } else { "sm-seat" }.into()),
                    ],
                );
            }
            x += cols[i] * cw;
            if i + 1 < dz.len() {
                g.add("line", &[("x1", fx(x, 1)), ("x2", fx(x, 1)), ("y1", n(y - 3.0)), ("y2", n(y + h + 3.0)), ("class", "sm-div".into())]);
            }
        }
        y += h + deck_gap;
    }
    let mut svg = g.done();
    svg.svgattr.insert("viewBox".into(), format!("0 0 {} {}", n(w), n(y - deck_gap + 10.0)));
    let head = el("div", "seatmap-head").child(label("home-sec", "Loading / A380-842 / 2 decks").style("margin:0")).child(label("mono dim", &format!("{} / {} PAX", num(filled), num(total))));
    vec![head, svg]
}

fn minutes(sec: f64) -> String {
    if sec > 0.0 {
        if sec >= 60.0 {
            format!("{} min", num(round(sec / 60.0)))
        } else {
            format!("{} s", num(round(sec)))
        }
    } else {
        String::new()
    }
}

fn ef_row(lbl: &str, unit: &str, act: &str, target: f64, current: f64) -> Node {
    let v = if target.is_finite() { fx(target, 0) } else { String::new() };
    let now = if current.is_finite() { format!("now {}", locale_int(current)) } else { String::new() };
    el("div", "ef-row")
        .child(span("label", lbl))
        .child(Node::new("input").attr("type", "number").value(&v).act(&format!("focus:{act}")))
        .child(span("ef-unit", unit))
        .child(span("ef-now", &now))
}

fn choice(opts: &[(f64, &str)], act: &str, current: f64) -> Node {
    el("div", "ef-choice").children(opts.iter().map(|(v, t)| button("small", t).class_if("on", round(current) == *v).attr("type", "button").act(&format!("{act}:{}", num(*v)))))
}

/// The Payload & Fuel tab with the plugin's loadsheet page, its variables and the EFB state values.
pub fn loadsheet_page(page: &Value, vars: Option<&Vars>, state: &Map<String, Value>, said: &str) -> Node {
    let v = |k: &str| state.get(k).map(|x| x.as_f64().unwrap_or(f64::NAN)).unwrap_or(f64::NAN);
    let boarding = v("payload/boarding_started") == 1.0;
    let eta = minutes(v("payload/boarding_eta_s"));
    let board_eta = if boarding {
        if eta.is_empty() { "Boarding".to_string() } else { format!("About {eta} left") }
    } else if eta.is_empty() {
        String::new()
    } else {
        format!("Boarding takes about {eta}")
    };
    let pct = |x: f64| if x.is_finite() { format!("{} %MAC", fx(x, 1)) } else { "--".into() };
    let cg = format!(
        "ZFW CG {} (now {})   GW CG {} (now {})",
        pct(v("wb/zfw_cg_mac_desired")),
        pct(v("wb/zfw_cg_mac")),
        pct(v("wb/gw_cg_mac_desired")),
        pct(v("wb/gw_cg_mac"))
    );
    let payload = el("div", "box ef-box")
        .child(label("section-title", "Payload"))
        .child(ef_row("Passengers", "pax", "payload/pax_target", v("payload/pax_desired"), v("payload/pax")))
        .child(ef_row("Cargo", "kg", "payload/cargo_target_kg", v("payload/cargo_desired_kg"), v("payload/cargo_kg")))
        .child(ef_row("ZFW", "kg", "payload/zfw_target_kg", v("wb/zfw_desired_kg"), v("wb/zfw_kg")))
        .child(ef_row("GW", "kg", "payload/gw_target_kg", v("wb/gw_desired_kg"), v("wb/gw_kg")))
        .child(label("ef-cg", &cg))
        .child(el("div", "ef-row").child(span("label", "Boarding rate")).child(choice(&[(0.0, "Instant"), (1.0, "Fast"), (2.0, "Real")], "efb:payload/boarding_rate", v("payload/boarding_rate"))))
        .child(
            el("div", "row")
                .child(button("", if boarding { "Stop boarding" } else { "Board" }).class_if("on", boarding).attr("type", "button").act("efb:board"))
                .child(button("", "Deboard").attr("type", "button").act("efb:deboard"))
                .child(button("", "Import SimBrief").attr("type", "button").act("efb:simbrief"))
                .child(span("label dim", &board_eta)),
        );
    let mut detail = el("div", "box ef-box")
        .child(label("section-title", "Cabin and holds"))
        .child(ef_row("Weight per passenger", "kg", "payload/pax_weight_kg", v("payload/pax_weight_kg"), f64::NAN))
        .child(ef_row("Weight per bag", "kg", "payload/bag_weight_kg", v("payload/bag_weight_kg"), f64::NAN));
    for (i, h) in a(page, "holds").iter().enumerate() {
        let key = ["fwd", "aft", "bulk"].get(i).copied().unwrap_or("undefined");
        let l = s(h, "label");
        let lbl = if l.to_lowercase().contains("hold") { l.to_string() } else { format!("{l} hold") };
        detail = detail.child(ef_row(&lbl, "kg", &format!("payload/cargo/{key}/kg_desired"), v(&format!("payload/cargo/{key}/kg_desired")), v(&format!("payload/cargo/{key}/kg"))));
    }
    let mut stations = el("div", "ef-stations");
    if !state.is_empty() || vars.is_some() {
        for (i, z) in a(page, "zones").iter().enumerate() {
            let a1 = round(v(&format!("payload/station/{}/pax", i + 1)));
            let a2 = round(v(&format!("payload/station/{}/pax_desired", i + 1)));
            let z0 = |x: f64| if x.is_nan() || x == 0.0 { "0".to_string() } else { num(x) };
            stations = stations.child(el("div", "ef-station").child(Node::new("span").text(s(z, "label"))).child(Node::new("span").text(&format!("{} / {}", z0(a1), z0(a2)))));
        }
    }
    detail = detail.child(stations);
    let refuelling = v("refuel/started") == 1.0;
    let disabled = !refuelling && v("refuel/allowed") == 0.0;
    let fuel_info = if disabled {
        "Engines running or airborne: only Instant refuels".to_string()
    } else if refuelling && !minutes(v("refuel/eta_s")).is_empty() {
        format!("About {} left", minutes(v("refuel/eta_s")))
    } else {
        String::new()
    };
    let slider = if v("refuel/target_percent").is_finite() { num(round(v("refuel/target_percent")).clamp(0.0, 100.0)) } else { "50".into() };
    let fuel = el("div", "box ef-box")
        .child(label("section-title", "Fuel"))
        .child(ef_row("Fuel target", "kg", "refuel/target_kg", v("refuel/target_kg"), v("refuel/total_kg")))
        .child(el("div", "ef-row").child(span("label", "Target")).child(Node::new("input").attr("type", "range").attr("min", "0").attr("max", "100").attr("step", "1").value(&slider).act("range:refuel/target_percent")))
        .child(el("div", "ef-row").child(span("label", "Refuel rate")).child(choice(&[(0.0, "Real"), (1.0, "Fast"), (2.0, "Instant")], "efb:refuel/rate", v("refuel/rate"))))
        .child(
            el("div", "row")
                .child(button("", if refuelling { "Stop refuel" } else { "Start refuel" }).class_if("on", refuelling).attr("type", "button").attr_if("disabled", disabled).act("efb:refuel"))
                .child(span("label dim", &fuel_info)),
        );
    let empty = Vars::new();
    let sm_vars = vars.unwrap_or(&empty);
    let mut plan = el("div", "box");
    if let Some(vs) = vars {
        plan = plan.child(diagram(page, vs));
    }
    Node::el("div", "page-main")
        .id("loadsheet-content")
        .child(el("div", "box seatmap").children(seat_map(page, sm_vars)))
        .child(el("div", "ef-grid").child(Node::new("div").child(payload).child(fuel)).child(Node::new("div").child(detail)))
        .child(label("label dim ef-said", said))
        .child(plan)
}

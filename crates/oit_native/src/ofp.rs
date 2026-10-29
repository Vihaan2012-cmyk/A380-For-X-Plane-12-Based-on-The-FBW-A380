use oit_pages::flt_ops::{Ofp, OfpSection};
use oit_pages::ops_pages::OfpSummary;
use oit_pages::to_compute::TlrPhase;
use serde_json::Value;

/// What the OIT takes from one SimBrief OFP (`json=1`).
#[derive(Clone, Debug, Default)]
pub struct Loaded {
    pub ofp: Ofp,
    pub summary: OfpSummary,
    pub origin_runway: String,
    pub dest_runway: String,
    pub planned_tow: Option<f64>,
    pub planned_ldw: Option<f64>,
    pub lbs: bool,
    pub tlr_takeoff: Option<TlrPhase>,
    pub tlr_landing: Option<TlrPhase>,
}

fn arr(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(a) => a.iter().collect(),
        Value::Null => vec![],
        other => vec![other],
    }
}

fn s(v: &Value, path: &[&str]) -> String {
    let mut cur = v;
    for p in path {
        cur = &cur[*p];
    }
    match cur {
        Value::String(x) => x.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn num(v: &Value, path: &[&str]) -> Option<f64> {
    let t = s(v, path);
    t.trim().parse::<f64>().ok().filter(|x| x.is_finite())
}

fn decode_entities(s: &str) -> String {
    css_cascade::dom::decode_entities(s)
}

/// `textContent` of an HTML fragment, `<br>` as newlines.
pub fn html_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        let after = &rest[lt..];
        let end = after.find('>').map(|e| e + 1).unwrap_or(after.len());
        let tag = after[..end].to_ascii_lowercase();
        if tag.starts_with("<br") {
            out.push('\n');
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    let text = decode_entities(&out).replace('\r', "");
    text.trim_start_matches('\n').trim_end().to_string()
}

pub fn plan_sections(html: &str) -> Vec<OfpSection> {
    let mut body = html;
    if body.starts_with("<div ") {
        if let Some(e) = body.find('>') {
            body = &body[e + 1..];
        }
    }
    let body = body.strip_suffix("</div>").unwrap_or(body);
    let mut marks: Vec<(String, u32, usize, usize)> = Vec::new();
    let mut from = 0;
    while let Some(i) = body[from..].find("<!--BKMK///") {
        let at = from + i;
        let tail = &body[at + 11..];
        let Some(sep) = tail.find("///") else { break };
        let title = &tail[..sep];
        let after = &tail[sep + 3..];
        let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() || !after[digits.len()..].starts_with("-->") {
            from = at + 11;
            continue;
        }
        let end = at + 11 + sep + 3 + digits.len() + 3;
        marks.push((title.to_string(), digits.parse().unwrap_or(0), at, end));
        from = end;
    }
    if marks.is_empty() {
        return vec![OfpSection { group: "OFP".into(), title: "OPERATIONAL FLIGHT PLAN".into(), level: 0, text: Some(html_text(body)), image: None }];
    }
    let mut out = Vec::new();
    let head = html_text(&body[..marks[0].2]);
    if !head.trim().is_empty() {
        out.push(OfpSection { group: "OFP".into(), title: "HEADER".into(), level: 0, text: Some(head), image: None });
    }
    for (i, m) in marks.iter().enumerate() {
        let stop = marks.get(i + 1).map_or(body.len(), |n| n.2);
        let text = html_text(&body[m.3..stop]);
        let deeper = marks.get(i + 1).is_some_and(|n| n.1 > m.1);
        if !text.trim().is_empty() || deeper {
            out.push(OfpSection {
                group: "OFP".into(),
                title: m.0.to_uppercase(),
                level: m.1.min(1),
                text: if text.trim().is_empty() { None } else { Some(text) },
                image: None,
            });
        }
    }
    out
}

fn weather_text(w: &Value, origin: &str, dest: &str, altn: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut add = |label: String, v: &Value| {
        let t: Vec<&str> = arr(v).into_iter().filter_map(|x| x.as_str()).filter(|x| !x.trim().is_empty()).collect();
        if !t.is_empty() {
            lines.push(label);
            lines.push(t.join("\n"));
            lines.push(String::new());
        }
    };
    add(format!("DEPARTURE METAR {origin}"), &w["orig_metar"]);
    add(format!("DEPARTURE TAF {origin}"), &w["orig_taf"]);
    add(format!("DESTINATION METAR {dest}"), &w["dest_metar"]);
    add(format!("DESTINATION TAF {dest}"), &w["dest_taf"]);
    add(format!("ALTERNATE METAR {altn}"), &w["altn_metar"]);
    add(format!("ALTERNATE TAF {altn}"), &w["altn_taf"]);
    lines.join("\n").trim().to_string()
}

pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn iso_minutes(secs: f64) -> String {
    let t = secs.floor() as i64;
    let (y, m, d) = civil_from_days(t.div_euclid(86400));
    let sod = t.rem_euclid(86400);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}Z", sod / 3600, (sod % 3600) / 60)
}

fn tlr(v: &Value) -> Option<TlrPhase> {
    if v.is_null() {
        return None;
    }
    serde_json::from_value(v.clone()).ok()
}

/// The OFP or the message SimBrief answered with.
pub fn parse(text: &str) -> Result<Loaded, String> {
    let raw: Value = serde_json::from_str(text).map_err(|_| "SIMBRIEF NOT REACHABLE".to_string())?;
    let status = s(&raw, &["fetch", "status"]);
    if !status.is_empty() && status != "Success" {
        return Err(format!("SIMBRIEF: {}", status.to_uppercase()));
    }
    let g = &raw["general"];
    let origin = s(&raw, &["origin", "icao_code"]);
    let destination = s(&raw, &["destination", "icao_code"]);
    let alternate = arr(&raw["alternate"]).first().map(|a| s(a, &["icao_code"])).unwrap_or_default();
    let mut sections = plan_sections(&s(&raw, &["text", "plan_html"]));
    let fpl = s(&raw, &["atc", "flightplan_text"]).trim().to_string();
    if !fpl.is_empty() {
        sections.push(OfpSection { group: "ATC".into(), title: "ATC FLIGHT PLAN".into(), level: 0, text: Some(fpl), image: None });
    }
    let wx = weather_text(&raw["weather"], &origin, &destination, &alternate);
    if !wx.is_empty() {
        sections.push(OfpSection { group: "WEATHER".into(), title: "METAR / TAF".into(), level: 0, text: Some(wx), image: None });
    }
    let dir = s(&raw, &["images", "directory"]);
    for m in arr(&raw["images"]["map"]) {
        let link = s(m, &["link"]);
        if !link.is_empty() {
            let name = s(m, &["name"]);
            sections.push(OfpSection {
                group: "GRAPHICAL ANALYSIS".into(),
                title: if name.is_empty() { link.to_uppercase() } else { name.to_uppercase() },
                level: 0,
                text: None,
                image: Some(format!("{dir}{link}")),
            });
        }
    }
    let generated = num(&raw, &["params", "time_generated"]).filter(|t| *t > 0.0).map(iso_minutes).unwrap_or_default();
    let flight = format!("{}{}", s(g, &["icao_airline"]), s(g, &["flight_number"]));
    let w = &raw["weights"];
    let f = &raw["fuel"];
    let summary = OfpSummary {
        callsign: s(&raw, &["atc", "callsign"]),
        flight_number: flight.clone(),
        origin: origin.clone(),
        destination: destination.clone(),
        alternate: if alternate.is_empty() { None } else { Some(alternate.clone()) },
        route: s(g, &["route"]),
        cruise_altitude: num(g, &["initial_altitude"]).unwrap_or(0.0),
        cost_index: s(g, &["costindex"]),
        distance: s(g, &["air_distance"]),
        units: s(&raw, &["params", "units"]),
        plan_ramp: s(f, &["plan_ramp"]),
        plan_take_off: s(f, &["plan_takeoff"]),
        enroute_burn: s(f, &["enroute_burn"]),
        plan_landing: s(f, &["plan_landing"]),
        est_zero_fuel_weight: s(w, &["est_zfw"]),
        est_take_off_weight: s(w, &["est_tow"]),
        est_landing_weight: s(w, &["est_ldw"]),
        passenger_count: s(w, &["pax_count"]),
    };
    Ok(Loaded {
        ofp: Ofp { flight, origin: origin.clone(), destination: destination.clone(), alternate, generated, sections },
        summary,
        origin_runway: s(&raw, &["origin", "plan_rwy"]),
        dest_runway: s(&raw, &["destination", "plan_rwy"]),
        planned_tow: num(w, &["est_tow"]),
        planned_ldw: num(w, &["est_ldw"]),
        lbs: s(&raw, &["params", "units"]) == "lbs",
        tlr_takeoff: tlr(&raw["tlr"]["takeoff"]),
        tlr_landing: tlr(&raw["tlr"]["landing"]),
    })
}

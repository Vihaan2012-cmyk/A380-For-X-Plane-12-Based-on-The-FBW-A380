use serde_json::Value;

use crate::acars::{form_urlencode, HttpClient};
use crate::online::SIMBRIEF_FETCH_URL;
use crate::status::AtsuStatusCodes;

#[derive(Clone, Debug, PartialEq)]
pub enum WindFix {
    Ident(String),
    LatLon { lat: f64, long: f64 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct CruiseWindRequest {
    pub flight_levels: Vec<f64>,
    pub waypoints: Vec<WindFix>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AlternateWindRequest {
    pub destination_icao: String,
    pub alternate_icao: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindRequestMessage {
    pub climb_wind_level: Option<Option<f64>>,
    pub cruise_winds: Option<CruiseWindRequest>,
    pub descent_wind_level: Option<Option<f64>>,
    pub alternate_wind: Option<AlternateWindRequest>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UplinkedWindLevel {
    pub flight_level: f64,
    pub true_degrees: f64,
    pub magnitude: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UplinkedCruiseWindEntry {
    pub fix: WindFix,
    pub flight_level: Option<f64>,
    pub temperature: Option<f64>,
    pub true_degrees: f64,
    pub magnitude: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UplinkedCruiseWindSet {
    pub flight_level: f64,
    pub fixes: Vec<UplinkedCruiseWindEntry>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindUplinkMessage {
    pub climb_winds: Option<Vec<UplinkedWindLevel>>,
    pub cruise_winds: Option<Vec<UplinkedCruiseWindSet>>,
    pub descent_winds: Option<Vec<UplinkedWindLevel>>,
    pub alternate_wind: Option<UplinkedWindLevel>,
}

fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

fn fixed0(x: f64) -> String {
    format!("{}", x.round() as i64)
}

fn pad3(x: f64) -> String {
    format!("{:0>3}", fixed0(x))
}

fn coordinate_ident(lat: f64, long: f64) -> String {
    format!(
        "{}{:0>2}{}{:0>3}",
        if lat >= 0.0 { 'N' } else { 'S' },
        lat.trunc().abs() as i64,
        if long >= 0.0 { 'E' } else { 'W' },
        long.trunc().abs() as i64
    )
}

fn fix_ident(fix: &WindFix) -> String {
    match fix {
        WindFix::Ident(s) => s.clone(),
        WindFix::LatLon { lat, long } => coordinate_ident(*lat, *long),
    }
}

fn level_triplet(w: &UplinkedWindLevel) -> String {
    format!("{}{}{}", pad3(w.flight_level), pad3(w.true_degrees), pad3(w.magnitude))
}

pub fn format_wind_downlink_message(m: &WindRequestMessage) -> String {
    let climb = match m.climb_wind_level {
        None => String::new(),
        Some(None) => "/CQ".into(),
        Some(Some(l)) => format!("/CQ{}", fixed0(js_round(l))),
    };
    let cruise = match &m.cruise_winds {
        Some(c) if !c.waypoints.is_empty() => format!(
            "/WQ{}:{}",
            c.flight_levels.iter().map(|l| fixed0(js_round(*l))).collect::<Vec<_>>().join("."),
            c.waypoints.iter().map(fix_ident).collect::<Vec<_>>().join(",")
        ),
        _ => String::new(),
    };
    let descent = match m.descent_wind_level {
        None => String::new(),
        Some(None) => "/DQ".into(),
        Some(Some(l)) => format!("/DQ{}", js_round(l)),
    };
    let alternate = match &m.alternate_wind {
        None => String::new(),
        Some(a) => format!("/WR{}.{}", a.destination_icao, a.alternate_icao),
    };
    format!("REQPWI{climb}{cruise}{descent}{alternate}")
}

pub fn format_wind_uplink_message(m: &WindUplinkMessage) -> String {
    let climb = m
        .climb_winds
        .as_ref()
        .map(|c| format!("/CB{}", c.iter().map(level_triplet).collect::<Vec<_>>().join(".")))
        .unwrap_or_default();
    let cruise = m
        .cruise_winds
        .as_ref()
        .map(|sets| {
            sets.iter()
                .map(|set| {
                    let fixes = set
                        .fixes
                        .iter()
                        .map(|fix| {
                            let temp = match (fix.temperature, fix.flight_level) {
                                (Some(t), Some(fl)) => {
                                    let r = js_round(t);
                                    format!(",{}{}{}", pad3(fl), if r >= 0.0 { 'P' } else { 'M' }, r.abs())
                                }
                                _ => String::new(),
                            };
                            format!("{},{}{}{}", fix_ident(&fix.fix), pad3(fix.true_degrees), pad3(fix.magnitude), temp)
                        })
                        .collect::<Vec<_>>()
                        .join(".");
                    format!("/WD{},{}", pad3(set.flight_level), fixes)
                })
                .collect::<String>()
        })
        .unwrap_or_default();
    let descent = m
        .descent_winds
        .as_ref()
        .map(|d| format!("/DD{}", d.iter().map(level_triplet).collect::<Vec<_>>().join(".")))
        .unwrap_or_default();
    let alternate = m.alternate_wind.as_ref().map(|a| format!("/AW{}", level_triplet(a))).unwrap_or_default();
    format!("PWI{climb}{cruise}{descent}{alternate}")
}

fn parse_int(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().map(f64::trunc).unwrap_or(f64::NAN),
        Some(Value::String(s)) => {
            let t = s.trim_start();
            let end = t
                .char_indices()
                .take_while(|(i, c)| c.is_ascii_digit() || (*i == 0 && (*c == '-' || *c == '+')))
                .map(|(i, c)| i + c.len_utf8())
                .last()
                .unwrap_or(0);
            t[..end].parse::<f64>().unwrap_or(f64::NAN)
        }
        _ => f64::NAN,
    }
}

fn parse_float(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
}

fn str_field<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn levels(fix: &Value) -> Vec<Value> {
    match fix.pointer("/wind_data/level") {
        Some(Value::Array(a)) => a.clone(),
        Some(v @ Value::Object(_)) => vec![v.clone()],
        _ => Vec::new(),
    }
}

fn wind_level(l: &Value) -> UplinkedWindLevel {
    UplinkedWindLevel {
        true_degrees: parse_int(l.get("wind_dir")),
        magnitude: parse_int(l.get("wind_spd")),
        flight_level: js_round(parse_int(l.get("altitude")) / 100.0),
    }
}

fn level_alt(l: &[Value], i: usize) -> f64 {
    parse_int(l[i].get("altitude"))
}

fn parse_climb_winds(navlog: &[Value]) -> Vec<UplinkedWindLevel> {
    let mut result = Vec::new();
    let mut last = 0.0;
    for (wpt_idx, wpt) in navlog.iter().enumerate() {
        if str_field(wpt, "stage") != "CLB" {
            continue;
        }
        let wpt_alt = parse_int(wpt.get("altitude_feet"));
        let lv = levels(wpt);
        if wpt_idx == 0 {
            let mut alt_idx = 0;
            while last < wpt_alt && alt_idx < lv.len() {
                let w = wind_level(&lv[alt_idx]);
                result.push(w);
                last = w.flight_level * 100.0;
                alt_idx += 1;
            }
        }
        for level_idx in 0..lv.len() {
            let altitude = level_alt(&lv, level_idx);
            let (mut d_prev, mut d_this) = (0.0, 0.0);
            if level_idx > 0 && level_idx < lv.len() - 1 {
                d_prev = (wpt_alt - level_alt(&lv, level_idx - 1)).abs();
                d_this = (wpt_alt - altitude).abs();
            }
            if altitude > last && last <= wpt_alt {
                let idx = if d_prev > d_this { level_idx as isize } else { level_idx as isize - 1 };
                if idx < 0 {
                    continue;
                }
                let idx = idx as usize;
                let idx_alt = level_alt(&lv, idx);
                if idx_alt > last {
                    result.push(UplinkedWindLevel {
                        true_degrees: parse_int(lv[idx].get("wind_dir")),
                        magnitude: parse_int(lv[idx].get("wind_spd")),
                        flight_level: js_round(idx_alt / 100.0),
                    });
                    last = idx_alt;
                }
            }
        }
    }
    result.sort_by(|a, b| b.flight_level.partial_cmp(&a.flight_level).unwrap_or(std::cmp::Ordering::Equal));
    result
}

fn parse_descent_winds(navlog: &[Value]) -> Vec<UplinkedWindLevel> {
    let mut result = Vec::new();
    let mut last = 45000.0;
    for (wpt_idx, wpt) in navlog.iter().enumerate() {
        if str_field(wpt, "ident") != "TOD" && str_field(wpt, "stage") != "DSC" {
            continue;
        }
        let wpt_alt = parse_int(wpt.get("altitude_feet"));
        let lv = levels(wpt);
        if wpt_idx == 0 {
            let mut alt_idx = lv.len() as isize - 1;
            while last > wpt_alt && alt_idx >= 0 {
                let w = wind_level(&lv[alt_idx as usize]);
                result.push(w);
                last = w.flight_level * 100.0;
                alt_idx -= 1;
            }
        }
        for level_idx in (0..lv.len()).rev() {
            let altitude = level_alt(&lv, level_idx);
            let (mut d_next, mut d_this) = (0.0, 0.0);
            if level_idx > 0 {
                d_next = (wpt_alt - level_alt(&lv, level_idx - 1)).abs();
                d_this = (wpt_alt - altitude).abs();
            }
            if altitude >= last && last > wpt_alt {
                let idx = if d_next > d_this { level_idx as isize } else { level_idx as isize - 1 };
                if idx < 0 {
                    continue;
                }
                let idx = idx as usize;
                let idx_alt = level_alt(&lv, idx);
                if idx_alt < last {
                    result.push(UplinkedWindLevel {
                        true_degrees: parse_int(lv[idx].get("wind_dir")),
                        magnitude: parse_int(lv[idx].get("wind_spd")),
                        flight_level: js_round(idx_alt / 100.0),
                    });
                    last = idx_alt;
                }
            }
        }
    }
    result
}

fn interpolate(x: f64, x0: f64, x1: f64, y0: f64, y1: f64) -> f64 {
    (y0 * (x1 - x) + y1 * (x - x0)) / (x1 - x0)
}

fn parse_cruise_winds(navlog: &[Value], req: &CruiseWindRequest) -> Vec<UplinkedCruiseWindSet> {
    let about = |a: f64, b: f64| (a - b).abs() < 1e-4;
    let wpts: Vec<&Value> = navlog
        .iter()
        .filter(|w| {
            let ident = str_field(w, "ident");
            ident != "TOC"
                && ident != "TOD"
                && req.waypoints.iter().any(|r| match r {
                    WindFix::Ident(s) => str_field(w, "type") == "wpt" && s == ident,
                    WindFix::LatLon { lat, long } => {
                        str_field(w, "type") == "ltlg"
                            && about(*lat, parse_float(w.get("pos_lat")))
                            && about(*long, parse_float(w.get("pos_long")))
                    }
                })
        })
        .collect();

    let mut result = Vec::new();
    for &level in &req.flight_levels {
        let mut set = UplinkedCruiseWindSet { flight_level: level, fixes: Vec::new() };
        for fix in &wpts {
            let lv = levels(fix);
            if lv.is_empty() {
                continue;
            }
            let id = if str_field(fix, "type") == "ltlg" {
                WindFix::LatLon { lat: parse_float(fix.get("pos_lat")), long: parse_float(fix.get("pos_long")) }
            } else {
                WindFix::Ident(str_field(fix, "ident").to_string())
            };
            let mut entry = UplinkedCruiseWindEntry {
                fix: id,
                flight_level: Some(level),
                temperature: None,
                true_degrees: 0.0,
                magnitude: 0.0,
            };
            let last = lv.len() - 1;
            let lowest = js_round(level_alt(&lv, 0) / 100.0);
            let highest = js_round(level_alt(&lv, last) / 100.0);
            let take = |e: &mut UplinkedCruiseWindEntry, l: &Value| {
                e.true_degrees = parse_int(l.get("wind_dir"));
                e.magnitude = parse_int(l.get("wind_spd"));
                e.temperature = Some(parse_int(l.get("oat")));
            };
            if level <= lowest {
                take(&mut entry, &lv[0]);
            } else if level >= highest {
                take(&mut entry, &lv[last]);
            }
            for i in 0..last {
                let (lo, hi) = (&lv[i], &lv[i + 1]);
                let lower_level = js_round(parse_int(lo.get("altitude")) / 100.0);
                let upper_level = js_round(parse_int(hi.get("altitude")) / 100.0);
                if level >= lower_level && level <= upper_level {
                    let polar = |l: &Value| {
                        let spd = parse_int(l.get("wind_spd"));
                        let dir = parse_int(l.get("wind_dir")).to_radians();
                        (spd * dir.cos(), spd * dir.sin())
                    };
                    let (x0, y0) = polar(lo);
                    let (x1, y1) = polar(hi);
                    let x = interpolate(level, lower_level, upper_level, x0, x1);
                    let y = interpolate(level, lower_level, upper_level, y0, y1);
                    let deg = y.atan2(x).to_degrees() % 360.0;
                    entry.true_degrees = js_round((deg + 360.0) % 360.0);
                    entry.magnitude = js_round(x.hypot(y));
                    entry.temperature = Some(interpolate(
                        level,
                        lower_level,
                        upper_level,
                        parse_int(lo.get("oat")),
                        parse_int(hi.get("oat")),
                    ));
                    break;
                }
            }
            set.fixes.push(entry);
        }
        result.push(set);
    }
    result
}

pub fn parse_simbrief_winds(ofp: &Value, request: &WindRequestMessage) -> WindUplinkMessage {
    let navlog: Vec<Value> = match ofp.pointer("/navlog/fix") {
        Some(Value::Array(a)) => a.clone(),
        Some(v @ Value::Object(_)) => vec![v.clone()],
        _ => Vec::new(),
    };
    let alternate = ofp.get("alternate").filter(|a| a.is_object());
    WindUplinkMessage {
        climb_winds: request.climb_wind_level.map(|_| parse_climb_winds(&navlog)),
        cruise_winds: request
            .cruise_winds
            .as_ref()
            .filter(|c| !c.flight_levels.is_empty())
            .map(|c| parse_cruise_winds(&navlog, c)),
        descent_winds: request.descent_wind_level.map(|_| parse_descent_winds(&navlog)),
        alternate_wind: match (alternate, &request.alternate_wind) {
            (Some(a), Some(_)) => Some(UplinkedWindLevel {
                flight_level: js_round(parse_int(a.get("cruise_altitude")) / 100.0),
                true_degrees: parse_int(a.get("avg_wind_dir")),
                magnitude: parse_int(a.get("avg_wind_spd")),
            }),
            _ => None,
        },
    }
}

pub fn simbrief_url(username: &str, user_id: &str) -> Option<String> {
    if !user_id.is_empty() {
        Some(format!("{}?userid={}&json=1", SIMBRIEF_FETCH_URL, form_urlencode(user_id)))
    } else if !username.is_empty() {
        Some(format!("{}?username={}&json=1", SIMBRIEF_FETCH_URL, form_urlencode(username)))
    } else {
        None
    }
}

pub fn receive_simbrief_winds(
    client: &mut dyn HttpClient,
    username: &str,
    user_id: &str,
    request: &WindRequestMessage,
) -> (AtsuStatusCodes, Option<WindUplinkMessage>) {
    let Some(url) = simbrief_url(username, user_id) else {
        return (AtsuStatusCodes::ComFailed, None);
    };
    let ofp = match client.get(&url, "application/json") {
        Ok(r) if r.ok() => serde_json::from_str::<Value>(&r.body).ok(),
        _ => None,
    };
    match ofp {
        Some(ofp) => (AtsuStatusCodes::Ok, Some(parse_simbrief_winds(&ofp, request))),
        None => (AtsuStatusCodes::ComFailed, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nxapi::tests::{ok, Recorder};
    use serde_json::json;

    fn lvl(alt: i32, dir: i32, spd: i32, oat: i32) -> Value {
        json!({"altitude": alt.to_string(), "wind_dir": format!("{dir:03}"), "wind_spd": format!("{spd:03}"), "oat": oat.to_string()})
    }

    fn levels_set() -> Vec<Value> {
        vec![lvl(1000, 200, 10, 10), lvl(10000, 230, 30, -5), lvl(20000, 250, 50, -25), lvl(30000, 270, 80, -45), lvl(40000, 270, 60, -56), lvl(45000, 280, 40, -57)]
    }

    fn ofp() -> Value {
        json!({
            "navlog": {"fix": [
                {"ident": "DEP1", "type": "wpt", "stage": "CLB", "altitude_feet": "4000", "pos_lat": "51.1", "pos_long": "-0.5", "wind_data": {"level": levels_set()}},
                {"ident": "CLB2", "type": "wpt", "stage": "CLB", "altitude_feet": "21000", "pos_lat": "51.5", "pos_long": "1.0", "wind_data": {"level": levels_set()}},
                {"ident": "TOC", "type": "ltlg", "stage": "CLB", "altitude_feet": "35000", "pos_lat": "52.0", "pos_long": "3.0", "wind_data": {"level": levels_set()}},
                {"ident": "CRZA", "type": "wpt", "stage": "CRZ", "altitude_feet": "35000", "pos_lat": "52.0", "pos_long": "5.0", "wind_data": {"level": levels_set()}},
                {"ident": "N52E010", "type": "ltlg", "stage": "CRZ", "altitude_feet": "35000", "pos_lat": "52.000000", "pos_long": "10.000000", "wind_data": {"level": levels_set()}},
                {"ident": "TOD", "type": "ltlg", "stage": "CRZ", "altitude_feet": "35000", "pos_lat": "52.0", "pos_long": "11.0", "wind_data": {"level": levels_set()}},
                {"ident": "DSC1", "type": "wpt", "stage": "DSC", "altitude_feet": "12000", "pos_lat": "52.0", "pos_long": "12.0", "wind_data": {"level": levels_set()}}
            ]},
            "alternate": {"icao_code": "EDDH", "avg_wind_dir": "250", "avg_wind_spd": "21", "cruise_altitude": "24000"}
        })
    }

    fn request() -> WindRequestMessage {
        WindRequestMessage {
            climb_wind_level: Some(Some(350.0)),
            cruise_winds: Some(CruiseWindRequest {
                flight_levels: vec![350.0, 450.0],
                waypoints: vec![WindFix::Ident("CRZA".into()), WindFix::LatLon { lat: 52.0, long: 10.0 }, WindFix::Ident("TOD".into())],
            }),
            descent_wind_level: Some(Some(350.0)),
            alternate_wind: Some(AlternateWindRequest { destination_icao: "EDDB".into(), alternate_icao: "EDDH".into() }),
        }
    }

    #[test]
    fn downlink_format() {
        assert_eq!(
            format_wind_downlink_message(&request()),
            "REQPWI/CQ350/WQ350.450:CRZA,N52E010,TOD/DQ350/WREDDB.EDDH"
        );
        let empty = WindRequestMessage { climb_wind_level: Some(None), descent_wind_level: Some(None), ..Default::default() };
        assert_eq!(format_wind_downlink_message(&empty), "REQPWI/CQ/DQ");
        let south = WindFix::LatLon { lat: -5.5, long: -45.2 };
        assert_eq!(fix_ident(&south), "S05W045");
    }

    #[test]
    fn simbrief_winds_parse() {
        let up = parse_simbrief_winds(&ofp(), &request());
        let climb = up.climb_winds.clone().unwrap();
        assert_eq!(climb.first().unwrap().flight_level, 400.0);
        assert!(climb.windows(2).all(|w| w[0].flight_level >= w[1].flight_level));
        assert!(climb.iter().any(|w| w.flight_level == 10.0));

        let cruise = up.cruise_winds.clone().unwrap();
        assert_eq!(cruise.len(), 2);
        assert_eq!(cruise[0].fixes.len(), 2);
        let f = &cruise[0].fixes[0];
        assert_eq!(f.fix, WindFix::Ident("CRZA".into()));
        assert_eq!((f.true_degrees, f.magnitude), (270.0, 70.0));
        assert!((f.temperature.unwrap() - -50.5).abs() < 1e-9);
        assert_eq!((cruise[1].fixes[0].true_degrees, cruise[1].fixes[0].magnitude), (280.0, 40.0));

        let descent = up.descent_winds.clone().unwrap();
        assert_eq!(descent.iter().map(|w| w.flight_level).collect::<Vec<_>>(), vec![400.0, 300.0, 200.0, 100.0]);
        assert!(descent.windows(2).all(|w| w[0].flight_level > w[1].flight_level));

        assert_eq!(up.alternate_wind, Some(UplinkedWindLevel { flight_level: 240.0, true_degrees: 250.0, magnitude: 21.0 }));

        let text = format_wind_uplink_message(&up);
        assert!(text.starts_with("PWI/CB400270060."), "{text}");
        assert!(text.contains("/WD350,CRZA,270070,350M50."), "{text}");
        assert!(text.contains(".N52E010,270070,350M50/WD450,"), "{text}");
        assert!(text.ends_with("/AW240250021"));
    }

    #[test]
    fn simbrief_fetch_paths() {
        let mut c = Recorder::new(vec![ok(&ofp().to_string()), Err("timeout".into())]);
        let (code, up) = receive_simbrief_winds(&mut c, "", "123456", &WindRequestMessage::default());
        assert_eq!(code, AtsuStatusCodes::Ok);
        assert_eq!(up.unwrap(), WindUplinkMessage::default());
        assert_eq!(c.calls[0], format!("GET {SIMBRIEF_FETCH_URL}?userid=123456&json=1"));
        assert_eq!(receive_simbrief_winds(&mut c, "pilot", "", &request()).0, AtsuStatusCodes::ComFailed);
        assert_eq!(receive_simbrief_winds(&mut c, "", "", &request()).0, AtsuStatusCodes::ComFailed);
    }
}

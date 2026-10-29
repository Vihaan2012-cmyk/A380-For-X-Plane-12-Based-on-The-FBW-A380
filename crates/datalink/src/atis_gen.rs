use crate::online::{AtcKind, OnlineAtc};
use crate::weather::AtisType;

#[derive(Clone, Debug, PartialEq)]
pub struct AtisRunway {
    pub ident: String,
    pub heading_true_deg: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AtisAirport {
    pub icao: String,
    pub runways: Vec<AtisRunway>,
    pub transition_level_ft: Option<i32>,
    pub transition_alt_ft: Option<i32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wind {
    pub direction_true: Option<u16>,
    pub speed_kt: u16,
    pub gust_kt: Option<u16>,
    pub variable_from_to: Option<(u16, u16)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParsedMetar {
    pub station: String,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub wind: Option<Wind>,
    pub cavok: bool,
    pub visibility_m: Option<u32>,
    pub visibility_sm: Option<String>,
    pub weather: Vec<String>,
    pub clouds: Vec<(String, u32, String)>,
    pub vertical_visibility_ft: Option<u32>,
    pub temperature_c: Option<i32>,
    pub dewpoint_c: Option<i32>,
    pub qnh_hpa: Option<u16>,
    pub altimeter_inhg: Option<u16>,
}

fn temp(s: &str) -> Option<i32> {
    match s.strip_prefix('M') {
        Some(rest) => rest.parse::<i32>().ok().map(|v| -v),
        None => s.parse().ok(),
    }
}

fn parse_wind(tok: &str) -> Option<Wind> {
    let (body, factor) = if let Some(b) = tok.strip_suffix("KT") {
        (b, 1.0)
    } else if let Some(b) = tok.strip_suffix("MPS") {
        (b, 1.943_844)
    } else {
        return None;
    };
    if body.len() < 5 {
        return None;
    }
    let (dir, rest) = body.split_at(3);
    let direction_true = if dir == "VRB" { None } else { Some(dir.parse::<u16>().ok()?) };
    let (spd, gust) = match rest.split_once('G') {
        Some((s, g)) => (s, Some(g)),
        None => (rest, None),
    };
    let conv = |v: &str| v.parse::<f64>().ok().map(|n| (n * factor).round() as u16);
    Some(Wind { direction_true, speed_kt: conv(spd)?, gust_kt: gust.and_then(conv), variable_from_to: None })
}

fn is_weather(tok: &str) -> bool {
    const CODES: [&str; 26] = [
        "DZ", "RA", "SN", "SG", "IC", "PL", "GR", "GS", "UP", "BR", "FG", "FU", "VA", "DU", "SA", "HZ", "PY", "PO", "SQ",
        "FC", "SS", "DS", "TS", "SH", "FZ", "BL",
    ];
    let t = tok.trim_start_matches(['+', '-']).trim_start_matches("VC");
    !t.is_empty() && t.len() % 2 == 0 && t.as_bytes().chunks(2).all(|c| CODES.contains(&std::str::from_utf8(c).unwrap_or("")))
}

pub fn parse_metar(text: &str) -> Option<ParsedMetar> {
    let mut m = ParsedMetar::default();
    let mut toks = text.split_whitespace().peekable();
    while let Some(&t) = toks.peek() {
        if t == "METAR" || t == "SPECI" {
            toks.next();
        } else {
            break;
        }
    }
    m.station = toks.next()?.to_string();
    let time = toks.next()?;
    let digits = time.strip_suffix('Z')?;
    if digits.len() != 6 {
        return None;
    }
    m.day = digits[0..2].parse().ok()?;
    m.hour = digits[2..4].parse().ok()?;
    m.minute = digits[4..6].parse().ok()?;
    let mut sm_whole: Option<String> = None;
    for tok in toks {
        if tok == "RMK" || tok == "TEMPO" || tok == "BECMG" || tok == "NOSIG" {
            break;
        }
        if tok == "AUTO" || tok == "COR" {
            continue;
        }
        if m.wind.is_none() {
            if let Some(w) = parse_wind(tok) {
                m.wind = Some(w);
                continue;
            }
        }
        if tok.len() == 7 && tok.as_bytes()[3] == b'V' {
            if let (Ok(a), Ok(b)) = (tok[0..3].parse::<u16>(), tok[4..7].parse::<u16>()) {
                if let Some(w) = m.wind.as_mut() {
                    w.variable_from_to = Some((a, b));
                }
                continue;
            }
        }
        if tok == "CAVOK" {
            m.cavok = true;
            continue;
        }
        if tok.len() == 4 && tok.bytes().all(|c| c.is_ascii_digit()) && m.visibility_m.is_none() {
            m.visibility_m = tok.parse().ok();
            continue;
        }
        if let Some(sm) = tok.strip_suffix("SM") {
            m.visibility_sm = Some(match sm_whole.take() {
                Some(w) => format!("{w} {sm}"),
                None => sm.to_string(),
            });
            continue;
        }
        if tok.len() == 1 && tok.bytes().all(|c| c.is_ascii_digit()) {
            sm_whole = Some(tok.to_string());
            continue;
        }
        if let Some(vv) = tok.strip_prefix("VV") {
            m.vertical_visibility_ft = vv.parse::<u32>().ok().map(|h| h * 100);
            continue;
        }
        if tok.len() >= 6 && ["FEW", "SCT", "BKN", "OVC"].contains(&&tok[0..3]) {
            if let Ok(h) = tok[3..6].parse::<u32>() {
                m.clouds.push((tok[0..3].to_string(), h * 100, tok[6..].to_string()));
                continue;
            }
        }
        if tok == "NSC" || tok == "SKC" || tok == "CLR" || tok == "NCD" {
            continue;
        }
        if let Some((t, d)) = tok.split_once('/') {
            if let (Some(t), Some(d)) = (temp(t), temp(d)) {
                m.temperature_c = Some(t);
                m.dewpoint_c = Some(d);
                continue;
            }
        }
        if let Some(q) = tok.strip_prefix('Q') {
            if let Ok(v) = q.parse() {
                m.qnh_hpa = Some(v);
                continue;
            }
        }
        if let Some(a) = tok.strip_prefix('A') {
            if a.len() == 4 {
                if let Ok(v) = a.parse() {
                    m.altimeter_inhg = Some(v);
                    continue;
                }
            }
        }
        if is_weather(tok) {
            m.weather.push(tok.to_string());
        }
    }
    Some(m)
}

pub fn atis_letter(day: u8, hour: u8) -> char {
    let n = (day as u32 * 24 + hour as u32) % 26;
    (b'A' + n as u8) as char
}

pub fn runway_in_use<'a>(wind: Option<&Wind>, runways: &'a [AtisRunway]) -> Option<&'a AtisRunway> {
    let dir = wind.and_then(|w| if w.speed_kt == 0 { None } else { w.direction_true });
    let Some(dir) = dir else {
        return runways.first();
    };
    let headwind = |r: &AtisRunway| (((dir as f64) - r.heading_true_deg).to_radians()).cos();
    let mut best: Option<&AtisRunway> = None;
    for r in runways {
        if best.map_or(true, |b| headwind(r) > headwind(b) + 1e-9) {
            best = Some(r);
        }
    }
    best
}

fn signed(v: i32) -> String {
    if v < 0 {
        format!("M{:02}", -v)
    } else {
        format!("{v:02}")
    }
}

pub fn generate_atis(metar_text: &str, airport: Option<&AtisAirport>, kind: AtisType) -> Option<String> {
    let m = parse_metar(metar_text)?;
    let letter = atis_letter(m.day, m.hour);
    let icao = airport.map_or(m.station.as_str(), |a| a.icao.as_str());
    let label = match kind {
        AtisType::Departure => "DEP ATIS",
        AtisType::Arrival => "ARR ATIS",
        AtisType::Enroute => "ATIS",
    };
    let mut parts = vec![format!("{icao} {label} INFO {letter} {:02}{:02}Z", m.hour, m.minute)];
    if let Some(rwy) = airport.and_then(|a| runway_in_use(m.wind.as_ref(), &a.runways)) {
        parts.push(format!("RWY IN USE {}", rwy.ident));
    }
    if let Some(w) = &m.wind {
        let mut s = if w.speed_kt == 0 {
            "WIND CALM".to_string()
        } else {
            match w.direction_true {
                Some(d) => format!("WIND {d:03}/{}KT", w.speed_kt),
                None => format!("WIND VRB/{}KT", w.speed_kt),
            }
        };
        if let Some(g) = w.gust_kt {
            s.push_str(&format!(" GUST {g}KT"));
        }
        if let Some((a, b)) = w.variable_from_to {
            s.push_str(&format!(" VRB BTN {a:03} AND {b:03}"));
        }
        parts.push(s);
    }
    if m.cavok {
        parts.push("CAVOK".into());
    } else {
        if let Some(v) = m.visibility_m {
            parts.push(if v >= 9999 { "VIS 10KM OR MORE".into() } else if v >= 5000 { format!("VIS {}KM", v / 1000) } else { format!("VIS {v}M") });
        } else if let Some(v) = &m.visibility_sm {
            parts.push(format!("VIS {v}SM"));
        }
        if !m.weather.is_empty() {
            parts.push(m.weather.join(" "));
        }
        if let Some(vv) = m.vertical_visibility_ft {
            parts.push(format!("VERT VIS {vv}FT"));
        }
        if !m.clouds.is_empty() {
            let layers: Vec<String> = m.clouds.iter().map(|(c, h, t)| format!("{c} {h}FT{}", if t.is_empty() { String::new() } else { format!(" {t}") })).collect();
            parts.push(format!("CLD {}", layers.join(" ")));
        } else if m.vertical_visibility_ft.is_none() {
            parts.push("NO SIG CLOUD".into());
        }
    }
    if let (Some(t), Some(d)) = (m.temperature_c, m.dewpoint_c) {
        parts.push(format!("T {} DP {}", signed(t), signed(d)));
    }
    if let Some(q) = m.qnh_hpa {
        parts.push(format!("QNH {q}"));
    }
    if let Some(a) = m.altimeter_inhg {
        parts.push(format!("ALTIMETER {a:04}"));
    }
    if let Some(ta) = airport.and_then(|a| a.transition_alt_ft) {
        parts.push(format!("TRANS ALT {ta}FT"));
    }
    if let Some(tl) = airport.and_then(|a| a.transition_level_ft) {
        parts.push(format!("TRANS LEVEL FL{}", tl / 100));
    }
    parts.push(format!("ACK INFO {letter} ON FIRST CONTACT"));
    Some(format!("{}.", parts.join(". ")))
}

pub fn online_atis(stations: &[OnlineAtc], icao: &str, kind: AtisType) -> Option<String> {
    let icao = icao.to_ascii_uppercase();
    let wanted = match kind {
        AtisType::Departure => Some("_D_ATIS"),
        AtisType::Arrival => Some("_A_ATIS"),
        AtisType::Enroute => None,
    };
    let mut candidates: Vec<&OnlineAtc> = stations
        .iter()
        .filter(|s| s.kind == AtcKind::Atis && s.callsign.to_ascii_uppercase().starts_with(&format!("{icao}_")))
        .filter(|s| s.atis_lines.iter().any(|l| !l.trim().is_empty()))
        .collect();
    candidates.sort_by_key(|s| match wanted {
        Some(suffix) if s.callsign.to_ascii_uppercase().ends_with(suffix) => 0,
        _ if s.callsign.to_ascii_uppercase().ends_with("_ATIS") && !s.callsign.contains("_D_") && !s.callsign.contains("_A_") => 1,
        _ => 2,
    });
    let s = candidates.first()?;
    let text = s.atis_lines.iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
    Some(match &s.atis_code {
        Some(code) if !code.is_empty() && !text.contains(&format!("INFO {code}")) => format!("INFO {code} {text}"),
        _ => text,
    })
}

pub fn choose_atis(
    connected: bool,
    stations: &[OnlineAtc],
    icao: &str,
    kind: AtisType,
    metar: Option<&str>,
    airport: Option<&AtisAirport>,
) -> Option<String> {
    if connected {
        if let Some(text) = online_atis(stations, icao, kind) {
            return Some(text);
        }
    }
    generate_atis(metar?, airport, kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::online::OnlineNetwork;

    fn egll() -> AtisAirport {
        AtisAirport {
            icao: "EGLL".into(),
            runways: vec![
                AtisRunway { ident: "09L".into(), heading_true_deg: 89.7 },
                AtisRunway { ident: "27R".into(), heading_true_deg: 269.7 },
                AtisRunway { ident: "09R".into(), heading_true_deg: 89.7 },
                AtisRunway { ident: "27L".into(), heading_true_deg: 269.7 },
            ],
            transition_level_ft: Some(7000),
            transition_alt_ft: Some(6000),
        }
    }

    #[test]
    fn parses_an_icao_metar() {
        let m = parse_metar("EGLL 221250Z AUTO 24012G22KT 210V280 9999 -RA FEW025 BKN040CB 15/09 Q1013 NOSIG").unwrap();
        assert_eq!((m.day, m.hour, m.minute), (22, 12, 50));
        let w = m.wind.unwrap();
        assert_eq!((w.direction_true, w.speed_kt, w.gust_kt, w.variable_from_to), (Some(240), 12, Some(22), Some((210, 280))));
        assert_eq!(m.visibility_m, Some(9999));
        assert_eq!(m.weather, vec!["-RA".to_string()]);
        assert_eq!(m.clouds, vec![("FEW".into(), 2500, String::new()), ("BKN".into(), 4000, "CB".into())]);
        assert_eq!((m.temperature_c, m.dewpoint_c, m.qnh_hpa), (Some(15), Some(9), Some(1013)));
    }

    #[test]
    fn parses_a_us_metar_with_fractional_miles_and_minus_temps() {
        let m = parse_metar("METAR KJFK 220151Z 31008KT 1 1/2SM BR OVC004 M02/M03 A2992 RMK AO2").unwrap();
        assert_eq!(m.visibility_sm.as_deref(), Some("1 1/2"));
        assert_eq!(m.weather, vec!["BR".to_string()]);
        assert_eq!((m.temperature_c, m.dewpoint_c, m.altimeter_inhg), (Some(-2), Some(-3), Some(2992)));
        let mps = parse_metar("UUEE 220130Z 18005MPS CAVOK 03/M01 Q1021").unwrap();
        assert_eq!(mps.wind.unwrap().speed_kt, 10);
        assert!(mps.cavok);
    }

    #[test]
    fn runway_follows_the_wind_and_calm_keeps_the_first() {
        let a = egll();
        let west = Wind { direction_true: Some(240), speed_kt: 12, ..Default::default() };
        assert_eq!(runway_in_use(Some(&west), &a.runways).unwrap().ident, "27R");
        let east = Wind { direction_true: Some(100), speed_kt: 6, ..Default::default() };
        assert_eq!(runway_in_use(Some(&east), &a.runways).unwrap().ident, "09L");
        let calm = Wind { direction_true: Some(0), speed_kt: 0, ..Default::default() };
        assert_eq!(runway_in_use(Some(&calm), &a.runways).unwrap().ident, "09L");
        assert!(runway_in_use(None, &[]).is_none());
    }

    #[test]
    fn letter_changes_every_hour_and_wraps() {
        assert_eq!(atis_letter(0, 0), 'A');
        assert_eq!(atis_letter(0, 1), 'B');
        assert_eq!(atis_letter(1, 2), 'A');
        assert_ne!(atis_letter(22, 12), atis_letter(22, 13));
    }

    #[test]
    fn generated_atis_reads_like_a_d_atis() {
        let a = egll();
        let text = generate_atis("EGLL 221250Z 24012G22KT 9999 -RA FEW025 BKN040CB 15/09 Q1013", Some(&a), AtisType::Departure).unwrap();
        let l = atis_letter(22, 12);
        assert_eq!(
            text,
            format!(
                "EGLL DEP ATIS INFO {l} 1250Z. RWY IN USE 27R. WIND 240/12KT GUST 22KT. VIS 10KM OR MORE. -RA. CLD FEW 2500FT BKN 4000FT CB. T 15 DP 09. QNH 1013. TRANS ALT 6000FT. TRANS LEVEL FL70. ACK INFO {l} ON FIRST CONTACT."
            )
        );
        let bare = generate_atis("KXYZ 220151Z 00000KT 3/4SM FG VV002 M02/M03 A2992", None, AtisType::Arrival).unwrap();
        assert!(bare.starts_with("KXYZ ARR ATIS INFO"), "{bare}");
        assert!(bare.contains("WIND CALM. VIS 3/4SM. FG. VERT VIS 200FT. T M02 DP M03. ALTIMETER 2992."), "{bare}");
        assert!(!bare.contains("RWY IN USE"));
        assert!(generate_atis("garbage", None, AtisType::Arrival).is_none());
    }

    fn station(callsign: &str, lines: &[&str], code: Option<&str>) -> OnlineAtc {
        OnlineAtc {
            network: OnlineNetwork::Vatsim,
            callsign: callsign.into(),
            frequency_mhz: 128.075,
            kind: AtcKind::Atis,
            rating: 1,
            latitude: None,
            longitude: None,
            distance_nm: None,
            visual_range_nm: None,
            atis_code: code.map(Into::into),
            atis_lines: lines.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn online_atis_wins_only_when_connected() {
        let list = vec![
            station("KJFK_A_ATIS", &["KJFK ARR INFO C", "RWY 22L"], Some("C")),
            station("KJFK_D_ATIS", &["KJFK DEP INFO D", "RWY 31L"], Some("D")),
            station("EGLL_ATIS", &["", "  "], Some("K")),
        ];
        assert_eq!(online_atis(&list, "kjfk", AtisType::Departure).unwrap(), "KJFK DEP INFO D RWY 31L");
        assert_eq!(online_atis(&list, "KJFK", AtisType::Arrival).unwrap(), "KJFK ARR INFO C RWY 22L");
        assert!(online_atis(&list, "EGLL", AtisType::Departure).is_none());
        let metar = "KJFK 221251Z 31010KT 10SM FEW250 12/01 A3001";
        let online = choose_atis(true, &list, "KJFK", AtisType::Departure, Some(metar), None).unwrap();
        assert!(online.contains("RWY 31L"));
        let offline = choose_atis(false, &list, "KJFK", AtisType::Departure, Some(metar), None).unwrap();
        assert!(offline.starts_with("KJFK DEP ATIS INFO"), "{offline}");
        assert!(choose_atis(true, &list, "EGLL", AtisType::Departure, None, None).is_none());
    }
}

//! Online ATC for the EFB's ATC page, as FlyByWire's flyPad lists it
//! (fbw-common EFB/ATC/ATC.tsx): the controllers and ATIS stations in range
//! of the aircraft, nearest first, at most 26, plus UNICOM. FlyByWire takes
//! them from its own API; this reads the networks' public feeds directly,
//! refreshed at most once a minute: VATSIM's data.vatsim.net v3
//! (`controllers`, `atis`, and `transceivers-data.json` for where each
//! station is) and IVAO's tracker whazzup v2 (`clients.atcs`), which gives
//! no range, so a station is heard out to a typical range for its position.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const DATA_URL: &str = "https://data.vatsim.net/v3/vatsim-data.json";
const IVAO_URL: &str = "https://api.ivao.aero/v2/tracker/whazzup";
const TRANSCEIVERS_URL: &str = "https://data.vatsim.net/v3/transceivers-data.json";
const RECEIVE_MS: u32 = 20_000;
const REFRESH: Duration = Duration::from_secs(60);
const MAX_STATIONS: usize = 26;
const ATIS_RANGE_NM: f64 = 100.;
const HIGHEST_VHF_MHZ: f64 = 136.975;

#[derive(Clone, Debug, PartialEq)]
pub struct Station {
    pub callsign: String,
    pub frequency: String,
    pub kind: &'static str,
    pub range_nm: f64,
    pub lat: f64,
    pub lon: f64,
    pub atis: Vec<String>,
}

static CACHE: Mutex<[Option<(Instant, Vec<Station>)>; 2]> = Mutex::new([None, None]);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Network {
    Vatsim,
    Ivao,
}

impl Network {
    fn name(self) -> &'static str {
        match self {
            Network::Vatsim => "VATSIM",
            Network::Ivao => "IVAO",
        }
    }
}

pub fn ivao_range_nm(kind: &str) -> f64 {
    match kind {
        "DELIVERY" | "GROUND" => 20.,
        "TOWER" => 30.,
        "ATIS" | "APPROACH" => 100.,
        "RADAR" => 400.,
        _ => 50.,
    }
}

pub fn parse_ivao(whazzup: &str) -> Vec<Station> {
    let Ok(v) = serde_json::from_str::<Value>(whazzup) else { return Vec::new() };
    let mut out = Vec::new();
    for a in v.pointer("/clients/atcs").and_then(Value::as_array).into_iter().flatten() {
        let (Some(callsign), Some(mhz)) = (a.get("callsign").and_then(Value::as_str), a.pointer("/atcSession/frequency").and_then(Value::as_f64)) else {
            continue;
        };
        let (Some(lat), Some(lon)) = (a.pointer("/lastTrack/latitude").and_then(Value::as_f64), a.pointer("/lastTrack/longitude").and_then(Value::as_f64)) else {
            continue;
        };
        if callsign.contains("_OBS") || !(118.0..=HIGHEST_VHF_MHZ).contains(&mhz) {
            continue;
        }
        let kind = kind_of(callsign);
        out.push(Station {
            callsign: callsign.to_owned(),
            frequency: format!("{mhz:.3}"),
            kind,
            range_nm: ivao_range_nm(kind),
            lat,
            lon,
            atis: a.pointer("/atis/lines").and_then(Value::as_array).map(|l| l.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
        });
    }
    out
}

pub fn kind_of(callsign: &str) -> &'static str {
    match callsign.rsplit('_').next().unwrap_or("") {
        "ATIS" => "ATIS",
        "DEL" => "DELIVERY",
        "GND" => "GROUND",
        "TWR" => "TOWER",
        "APP" | "DEP" => "APPROACH",
        "CTR" | "FSS" => "RADAR",
        _ => "OTHER",
    }
}

pub fn parse(data: &str, transceivers: &str) -> Vec<Station> {
    let positions: HashMap<String, (f64, f64)> = serde_json::from_str::<Value>(transceivers)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|s| {
            let t = s.get("transceivers")?.as_array()?.first()?;
            Some((s.get("callsign")?.as_str()?.to_owned(), (t.get("latDeg")?.as_f64()?, t.get("lonDeg")?.as_f64()?)))
        })
        .collect();
    let Ok(data) = serde_json::from_str::<Value>(data) else { return Vec::new() };
    let mut out = Vec::new();
    for list in ["controllers", "atis"] {
        for c in data.get(list).and_then(Value::as_array).into_iter().flatten() {
            let (Some(callsign), Some(frequency)) = (c.get("callsign").and_then(Value::as_str), c.get("frequency").and_then(Value::as_str)) else {
                continue;
            };
            let mhz: f64 = frequency.parse().unwrap_or(0.);
            if callsign.contains("_OBS") || !(118.0..=HIGHEST_VHF_MHZ).contains(&mhz) {
                continue;
            }
            let Some(&(lat, lon)) = positions.get(callsign) else { continue };
            let kind = kind_of(callsign);
            let range = c.get("visual_range").and_then(Value::as_f64).unwrap_or(0.);
            out.push(Station {
                callsign: callsign.to_owned(),
                frequency: format!("{mhz:.3}"),
                kind,
                range_nm: if range == 0. && kind == "ATIS" { ATIS_RANGE_NM } else { range },
                lat,
                lon,
                atis: c.get("text_atis").and_then(Value::as_array).map(|l| l.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
            });
        }
    }
    out
}

pub fn distance_nm(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let (dp, dl) = ((lat2 - lat1).to_radians(), (lon2 - lon1).to_radians());
    let a = (dp / 2.).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.).sin().powi(2);
    2. * a.sqrt().atan2((1. - a).sqrt()) * 3440.065
}

pub fn in_range(stations: &[Station], lat: f64, lon: f64) -> Vec<(Station, f64)> {
    let mut near: Vec<(Station, f64)> = stations.iter().map(|s| (s.clone(), distance_nm(s.lat, s.lon, lat, lon))).filter(|(s, d)| *d <= s.range_nm).collect();
    near.sort_by(|a, b| a.1.total_cmp(&b.1));
    near.truncate(MAX_STATIONS);
    near
}

fn stations(network: Network) -> Result<Vec<Station>, String> {
    let slot = network as usize;
    if let Ok(cache) = CACHE.lock() {
        if let Some((at, list)) = cache[slot].as_ref() {
            if at.elapsed() < REFRESH {
                return Ok(list.clone());
            }
        }
    }
    let get = |url: &str| crate::net::request_with("GET", url, "", RECEIVE_MS).filter(|(status, _)| *status == 200).map(|(_, body)| body);
    let list = match network {
        Network::Vatsim => {
            let data = get(DATA_URL).ok_or("VATSIM data feed unreachable")?;
            let transceivers = get(TRANSCEIVERS_URL).ok_or("VATSIM transceiver feed unreachable")?;
            parse(&data, &transceivers)
        }
        Network::Ivao => parse_ivao(&get(IVAO_URL).ok_or("IVAO tracker unreachable")?),
    };
    if let Ok(mut cache) = CACHE.lock() {
        cache[slot] = Some((Instant::now(), list.clone()));
    }
    Ok(list)
}

pub fn answer(query: &str) -> (u16, String) {
    let arg = |k: &str| query.split('&').find_map(|kv| kv.strip_prefix(k)?.strip_prefix('=')?.parse::<f64>().ok());
    let (Some(lat), Some(lon)) = (arg("lat"), arg("lon")) else {
        return (400, json!({ "error": "lat and lon are needed" }).to_string());
    };
    let network = if query.split('&').any(|kv| kv.eq_ignore_ascii_case("source=ivao")) { Network::Ivao } else { Network::Vatsim };
    match stations(network) {
        Err(e) => (502, json!({ "error": e }).to_string()),
        Ok(list) => {
            let mut rows: Vec<Value> = in_range(&list, lat, lon)
                .into_iter()
                .map(|(s, d)| json!({ "callsign": s.callsign, "frequency": s.frequency, "type": s.kind, "distance": (d * 10.).round() / 10., "atis": s.atis }))
                .collect();
            rows.push(json!({ "callsign": "UNICOM", "frequency": "122.800", "type": "RADAR", "distance": 0, "atis": [] }));
            (200, json!({ "source": network.name(), "stations": rows }).to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = r#"{"controllers":[
        {"callsign":"EGLL_N_TWR","frequency":"118.700","visual_range":50,"text_atis":["Heathrow Tower"]},
        {"callsign":"EGLL_OBS","frequency":"199.998","visual_range":0},
        {"callsign":"LON_S_CTR","frequency":"129.425","visual_range":400,"text_atis":null},
        {"callsign":"KJFK_TWR","frequency":"119.100","visual_range":50}],
      "atis":[{"callsign":"EGLL_ATIS","frequency":"128.075","visual_range":0,"text_atis":["INFO A","RWY 27R"]}]}"#;
    const TRANSCEIVERS: &str = r#"[
        {"callsign":"EGLL_N_TWR","transceivers":[{"latDeg":51.47,"lonDeg":-0.45}]},
        {"callsign":"LON_S_CTR","transceivers":[{"latDeg":51.0,"lonDeg":-0.2}]},
        {"callsign":"KJFK_TWR","transceivers":[{"latDeg":40.64,"lonDeg":-73.78}]},
        {"callsign":"EGLL_ATIS","transceivers":[{"latDeg":51.47,"lonDeg":-0.46}]}]"#;

    #[test]
    fn stations_in_range_nearest_first_without_observers() {
        let all = parse(DATA, TRANSCEIVERS);
        assert_eq!(all.len(), 4, "the observer on 199.998 is dropped");
        let near = in_range(&all, 51.48, -0.40);
        let calls: Vec<&str> = near.iter().map(|(s, _)| s.callsign.as_str()).collect();
        assert_eq!(calls, ["EGLL_N_TWR", "EGLL_ATIS", "LON_S_CTR"]);
        assert_eq!(near[1].0.atis, ["INFO A", "RWY 27R"]);
        assert_eq!(near[1].0.range_nm, 100., "an ATIS with no range is heard 100 nm out");
        assert_eq!(kind_of("LON_S_CTR"), "RADAR");
        assert_eq!(kind_of("EGLL_DEL"), "DELIVERY");
    }

    #[test]
    fn ivao_controllers_come_with_their_last_position_and_a_typical_range() {
        let w = r#"{"clients":{"atcs":[
            {"callsign":"EDDL_TWR","atcSession":{"frequency":118.305,"position":"TWR"},"lastTrack":{"latitude":51.28,"longitude":6.757},"atis":{"lines":["EDDL INFO P"]}},
            {"callsign":"EDDL_OBS","atcSession":{"frequency":199.998},"lastTrack":{"latitude":51.28,"longitude":6.757}},
            {"callsign":"EDGG_CTR","atcSession":{"frequency":127.725,"position":"CTR"},"lastTrack":null}]}}"#;
        let all = parse_ivao(w);
        assert_eq!(all.len(), 1, "observers and stations without a position are dropped");
        assert_eq!((all[0].frequency.as_str(), all[0].kind, all[0].range_nm), ("118.305", "TOWER", 30.));
        assert_eq!(all[0].atis, ["EDDL INFO P"]);
    }

    #[test]
    fn heathrow_to_jfk_is_about_2990_nm() {
        let d = distance_nm(51.47, -0.45, 40.64, -73.78);
        assert!((d - 2991.).abs() < 15., "{d}");
    }
}

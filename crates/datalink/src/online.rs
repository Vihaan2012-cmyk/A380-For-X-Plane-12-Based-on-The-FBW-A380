use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::acars::{AcarsConnector, AcarsSettings, HttpClient};
use crate::message::DclMessage;
use crate::status::AtsuStatusCodes;

pub const VATSIM_DATA_FEED_URL: &str = "https://data.vatsim.net/v3/vatsim-data.json";
pub const IVAO_WHAZZUP_URL: &str = "https://api.ivao.aero/v2/tracker/whazzup";
pub const SIMBRIEF_FETCH_URL: &str = "https://www.simbrief.com/api/xml.fetcher.php";
pub const VATSIM_PREFILE_URL: &str = "https://my.vatsim.net/api/v2/flightplan";
pub const IVAO_PREFILE_URL: &str = "https://api.ivao.aero/v2/flight-plans";

#[derive(Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: &str) -> Secret {
        Secret(value.to_string())
    }

    pub fn reveal(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn masked(&self) -> String {
        let chars: Vec<char> = self.0.chars().collect();
        match chars.len() {
            0 => String::new(),
            1..=4 => "*".repeat(chars.len()),
            n => format!("{}{}{}", chars[0], "*".repeat(n - 2), chars[n - 1]),
        }
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secret({})", self.masked())
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.masked())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnlineNetwork {
    Vatsim,
    Ivao,
}

impl OnlineNetwork {
    pub fn parse(setting: &str) -> Option<OnlineNetwork> {
        match setting.to_ascii_lowercase().as_str() {
            "vatsim" => Some(OnlineNetwork::Vatsim),
            "ivao" => Some(OnlineNetwork::Ivao),
            _ => None,
        }
    }

    pub fn feed_url(self) -> &'static str {
        match self {
            OnlineNetwork::Vatsim => VATSIM_DATA_FEED_URL,
            OnlineNetwork::Ivao => IVAO_WHAZZUP_URL,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OnlineSettings {
    pub vatsim_cid: String,
    pub vatsim_token: Secret,
    pub vatsim_prefile_url: Option<String>,
    pub ivao_vid: String,
    pub ivao_token: Secret,
    pub ivao_prefile_url: Option<String>,
    pub simbrief_username: String,
}

impl OnlineSettings {
    pub fn credentials_present(&self, network: OnlineNetwork) -> bool {
        match network {
            OnlineNetwork::Vatsim => !self.vatsim_cid.is_empty() && !self.vatsim_token.is_empty(),
            OnlineNetwork::Ivao => !self.ivao_vid.is_empty() && !self.ivao_token.is_empty(),
        }
    }

    pub fn display_summary(&self) -> String {
        format!(
            "VATSIM {} token {}; IVAO {} token {}",
            self.vatsim_cid,
            self.vatsim_token.masked(),
            self.ivao_vid,
            self.ivao_token.masked()
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtcKind {
    Observer,
    Delivery,
    Ground,
    Tower,
    Approach,
    Departure,
    Centre,
    Fss,
    Atis,
    Other,
}

impl AtcKind {
    fn from_suffix(callsign: &str) -> AtcKind {
        let suffix = callsign.rsplit('_').next().unwrap_or("");
        match suffix {
            "OBS" => AtcKind::Observer,
            "DEL" | "CLR" | "CLD" => AtcKind::Delivery,
            "GND" | "RMP" | "APR" => AtcKind::Ground,
            "TWR" => AtcKind::Tower,
            "APP" => AtcKind::Approach,
            "DEP" => AtcKind::Departure,
            "CTR" => AtcKind::Centre,
            "FSS" => AtcKind::Fss,
            "ATIS" => AtcKind::Atis,
            _ => AtcKind::Other,
        }
    }

    fn from_vatsim_facility(facility: i64, callsign: &str) -> AtcKind {
        match facility {
            0 => AtcKind::Observer,
            1 => AtcKind::Fss,
            2 => AtcKind::Delivery,
            3 => AtcKind::Ground,
            4 => AtcKind::Tower,
            5 => {
                if callsign.ends_with("_DEP") {
                    AtcKind::Departure
                } else {
                    AtcKind::Approach
                }
            }
            6 => AtcKind::Centre,
            _ => AtcKind::Other,
        }
    }

    pub fn default_range_nm(self) -> Option<f64> {
        match self {
            AtcKind::Delivery | AtcKind::Ground => Some(30.0),
            AtcKind::Tower | AtcKind::Atis => Some(60.0),
            AtcKind::Approach | AtcKind::Departure => Some(150.0),
            AtcKind::Centre | AtcKind::Fss => None,
            AtcKind::Observer | AtcKind::Other => Some(0.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OnlineAtc {
    pub network: OnlineNetwork,
    pub callsign: String,
    pub frequency_mhz: f64,
    pub kind: AtcKind,
    pub rating: i64,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub distance_nm: Option<f64>,
    pub visual_range_nm: Option<f64>,
    pub atis_code: Option<String>,
    pub atis_lines: Vec<String>,
}

pub trait PositionLookup {
    fn airport_position(&self, icao: &str) -> Option<(f64, f64)>;
}

pub struct NoLookup;

impl PositionLookup for NoLookup {
    fn airport_position(&self, _icao: &str) -> Option<(f64, f64)> {
        None
    }
}

pub fn great_circle_nm(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dphi = (lat2 - lat1).to_radians();
    let dlambda = (lon2 - lon1).to_radians();
    let a = (dphi / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dlambda / 2.0).sin().powi(2);
    3440.065 * 2.0 * a.sqrt().atan2((1.0 - a).sqrt())
}

fn text_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn lookup_callsign(lookup: &dyn PositionLookup, callsign: &str) -> Option<(f64, f64)> {
    let prefix = callsign.split('_').next().unwrap_or("");
    if prefix.len() == 4 {
        return lookup.airport_position(prefix);
    }
    if prefix.len() == 3 {
        for lead in ['K', 'C', 'P'] {
            if let Some(position) = lookup.airport_position(&format!("{lead}{prefix}")) {
                return Some(position);
            }
        }
    }
    None
}

pub fn parse_vatsim_feed(feed: &Value, lookup: &dyn PositionLookup) -> Vec<OnlineAtc> {
    let mut result = Vec::new();
    let mut take = |entries: Option<&Vec<Value>>, atis: bool| {
        for entry in entries.into_iter().flatten() {
            let callsign = entry.get("callsign").and_then(Value::as_str).unwrap_or("").to_string();
            if callsign.is_empty() || callsign.contains("_OBS") {
                continue;
            }
            let facility = entry.get("facility").and_then(Value::as_i64).unwrap_or(-1);
            let kind = if atis { AtcKind::Atis } else { AtcKind::from_vatsim_facility(facility, &callsign) };
            if kind == AtcKind::Observer {
                continue;
            }
            let frequency = entry.get("frequency").and_then(text_f64).unwrap_or(0.0);
            if frequency >= 199.0 {
                continue;
            }
            let position = lookup_callsign(lookup, &callsign);
            let lines = entry
                .get("text_atis")
                .and_then(Value::as_array)
                .map(|l| l.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            result.push(OnlineAtc {
                network: OnlineNetwork::Vatsim,
                callsign,
                frequency_mhz: frequency,
                kind,
                rating: entry.get("rating").and_then(Value::as_i64).unwrap_or(0),
                latitude: position.map(|p| p.0),
                longitude: position.map(|p| p.1),
                distance_nm: None,
                visual_range_nm: entry.get("visual_range").and_then(text_f64),
                atis_code: entry.get("atis_code").and_then(Value::as_str).map(String::from),
                atis_lines: lines,
            });
        }
    };
    take(feed.get("controllers").and_then(Value::as_array), false);
    take(feed.get("atis").and_then(Value::as_array), true);
    result
}

pub fn parse_ivao_whazzup(feed: &Value) -> Vec<OnlineAtc> {
    let mut result = Vec::new();
    let atcs = feed
        .get("clients")
        .and_then(|c| c.get("atcs"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for entry in atcs {
        let callsign = entry.get("callsign").and_then(Value::as_str).unwrap_or("").to_string();
        if callsign.is_empty() {
            continue;
        }
        let session = entry.get("atcSession").cloned().unwrap_or(Value::Null);
        let position_name = session.get("position").and_then(Value::as_str).unwrap_or("");
        let kind = match AtcKind::from_suffix(&callsign) {
            AtcKind::Other => AtcKind::from_suffix(&format!("_{position_name}")),
            other => other,
        };
        if kind == AtcKind::Observer {
            continue;
        }
        let track = entry.get("lastTrack").cloned().unwrap_or(Value::Null);
        let atis = entry.get("atis").cloned().unwrap_or(Value::Null);
        let lines: Vec<String> = atis
            .get("lines")
            .and_then(Value::as_array)
            .map(|l| l.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        result.push(OnlineAtc {
            network: OnlineNetwork::Ivao,
            callsign,
            frequency_mhz: session.get("frequency").and_then(text_f64).unwrap_or(0.0),
            kind,
            rating: entry.get("rating").and_then(Value::as_i64).unwrap_or(0),
            latitude: track.get("latitude").and_then(text_f64),
            longitude: track.get("longitude").and_then(text_f64),
            distance_nm: None,
            visual_range_nm: None,
            atis_code: atis.get("revision").and_then(Value::as_str).map(String::from),
            atis_lines: lines,
        });
    }
    result
}

pub fn nearby(list: Vec<OnlineAtc>, lat: f64, lon: f64, max_range_nm: f64) -> Vec<OnlineAtc> {
    let mut known: Vec<OnlineAtc> = Vec::new();
    let mut unknown: Vec<OnlineAtc> = Vec::new();
    for mut atc in list {
        match (atc.latitude, atc.longitude) {
            (Some(a), Some(b)) => {
                let distance = great_circle_nm(lat, lon, a, b);
                atc.distance_nm = Some(distance);
                let kind_range = atc.kind.default_range_nm().unwrap_or(max_range_nm);
                let limit = kind_range.max(atc.visual_range_nm.unwrap_or(0.0)).min(max_range_nm.max(kind_range));
                if distance <= limit.min(max_range_nm) {
                    known.push(atc);
                }
            }
            _ => {
                if matches!(atc.kind, AtcKind::Centre | AtcKind::Fss) {
                    unknown.push(atc);
                }
            }
        }
    }
    known.sort_by(|a, b| a.distance_nm.partial_cmp(&b.distance_nm).unwrap());
    unknown.sort_by(|a, b| a.callsign.cmp(&b.callsign));
    known.extend(unknown);
    known
}

pub fn fetch_online_atc(
    client: &mut dyn HttpClient,
    network: OnlineNetwork,
    lookup: &dyn PositionLookup,
    lat: f64,
    lon: f64,
    max_range_nm: f64,
) -> Result<Vec<OnlineAtc>, String> {
    let response = client.get(network.feed_url(), "application/json")?;
    if !response.ok() {
        return Err(format!("{} feed returned HTTP {}", network.feed_url(), response.status));
    }
    let feed: Value = serde_json::from_str(&response.body).map_err(|e| format!("feed is not JSON: {e}"))?;
    let list = match network {
        OnlineNetwork::Vatsim => parse_vatsim_feed(&feed, lookup),
        OnlineNetwork::Ivao => parse_ivao_whazzup(&feed),
    };
    Ok(nearby(list, lat, lon, max_range_nm))
}

pub const VATSIM_PILOT_UPDATE_S: f64 = 15.0;
pub const IVAO_PILOT_UPDATE_S: f64 = 15.0;
pub const PILOT_RANGE_NM: f64 = 80.0;
pub const PILOT_RETRY_MAX_S: f64 = 120.0;

#[derive(Clone, Debug, PartialEq)]
pub struct OnlinePilot {
    pub network: OnlineNetwork,
    pub callsign: String,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_ft: f64,
    pub groundspeed_kt: f64,
    pub heading_deg: f64,
    pub transponder: Option<String>,
    pub aircraft_type: String,
    pub on_ground: Option<bool>,
    pub last_update_unix_s: Option<f64>,
    pub distance_nm: Option<f64>,
}

impl OnlinePilot {
    pub fn age_s(&self, now_unix_s: f64) -> Option<f64> {
        self.last_update_unix_s.map(|t| (now_unix_s - t).max(0.0))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PilotSnapshot {
    pub network: OnlineNetwork,
    pub feed_time_unix_s: Option<f64>,
    pub pilots: Vec<OnlinePilot>,
}

pub fn parse_iso_utc(text: &str) -> Option<f64> {
    let t = text.trim().trim_end_matches('Z');
    let (date, time) = t.split_once('T')?;
    let mut d = date.split('-');
    let y: i64 = d.next()?.parse().ok()?;
    let m: i64 = d.next()?.parse().ok()?;
    let day: i64 = d.next()?.parse().ok()?;
    let mut c = time.split(':');
    let hh: f64 = c.next()?.parse().ok()?;
    let mm: f64 = c.next()?.parse().ok()?;
    let ss: f64 = c.next().unwrap_or("0").parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) {
        return None;
    }
    let yy = if m <= 2 { y - 1 } else { y };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(days as f64 * 86400.0 + hh * 3600.0 + mm * 60.0 + ss)
}

fn squawk(value: Option<&Value>) -> Option<String> {
    let code = match value? {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => format!("{:04}", n.as_i64()?),
        _ => return None,
    };
    (code.len() == 4 && code.chars().all(|c| ('0'..='7').contains(&c))).then_some(code)
}

pub fn parse_vatsim_pilots(feed: &Value) -> Vec<OnlinePilot> {
    let mut out = Vec::new();
    for p in feed.get("pilots").and_then(Value::as_array).into_iter().flatten() {
        let callsign = p.get("callsign").and_then(Value::as_str).unwrap_or("").trim().to_string();
        let (Some(lat), Some(lon)) = (p.get("latitude").and_then(text_f64), p.get("longitude").and_then(text_f64)) else { continue };
        if callsign.is_empty() {
            continue;
        }
        let plan = p.get("flight_plan").cloned().unwrap_or(Value::Null);
        out.push(OnlinePilot {
            network: OnlineNetwork::Vatsim,
            callsign,
            latitude: lat,
            longitude: lon,
            altitude_ft: p.get("altitude").and_then(text_f64).unwrap_or(0.0),
            groundspeed_kt: p.get("groundspeed").and_then(text_f64).unwrap_or(0.0),
            heading_deg: p.get("heading").and_then(text_f64).unwrap_or(0.0).rem_euclid(360.0),
            transponder: squawk(p.get("transponder")),
            aircraft_type: plan.get("aircraft_short").and_then(Value::as_str).unwrap_or("").to_string(),
            on_ground: None,
            last_update_unix_s: p.get("last_updated").and_then(Value::as_str).and_then(parse_iso_utc),
            distance_nm: None,
        });
    }
    out
}

pub fn parse_ivao_pilots(feed: &Value) -> Vec<OnlinePilot> {
    let mut out = Vec::new();
    let pilots = feed.get("clients").and_then(|c| c.get("pilots")).and_then(Value::as_array);
    for p in pilots.into_iter().flatten() {
        let callsign = p.get("callsign").and_then(Value::as_str).unwrap_or("").trim().to_string();
        let track = p.get("lastTrack").cloned().unwrap_or(Value::Null);
        let (Some(lat), Some(lon)) = (track.get("latitude").and_then(text_f64), track.get("longitude").and_then(text_f64)) else { continue };
        if callsign.is_empty() {
            continue;
        }
        let plan = p.get("flightPlan").cloned().unwrap_or(Value::Null);
        let aircraft = plan
            .get("aircraft")
            .and_then(|a| a.get("icaoCode"))
            .and_then(Value::as_str)
            .or_else(|| plan.get("aircraftId").and_then(Value::as_str))
            .unwrap_or("");
        out.push(OnlinePilot {
            network: OnlineNetwork::Ivao,
            callsign,
            latitude: lat,
            longitude: lon,
            altitude_ft: track.get("altitude").and_then(text_f64).unwrap_or(0.0),
            groundspeed_kt: track.get("groundSpeed").and_then(text_f64).unwrap_or(0.0),
            heading_deg: track.get("heading").and_then(text_f64).unwrap_or(0.0).rem_euclid(360.0),
            transponder: squawk(track.get("transponder")),
            aircraft_type: aircraft.to_string(),
            on_ground: track.get("onGround").and_then(Value::as_bool),
            last_update_unix_s: track.get("timestamp").and_then(Value::as_str).and_then(parse_iso_utc),
            distance_nm: None,
        });
    }
    out
}

pub fn feed_time_unix_s(network: OnlineNetwork, feed: &Value) -> Option<f64> {
    let stamp = match network {
        OnlineNetwork::Vatsim => feed.get("general").and_then(|g| g.get("update_timestamp")),
        OnlineNetwork::Ivao => feed.get("updatedAt"),
    };
    stamp.and_then(Value::as_str).and_then(parse_iso_utc)
}

pub fn nearby_pilots(list: Vec<OnlinePilot>, lat: f64, lon: f64, range_nm: f64, own_callsign: Option<&str>) -> Vec<OnlinePilot> {
    let own = own_callsign.map(|c| c.trim().to_ascii_uppercase()).filter(|c| !c.is_empty());
    let mut out: Vec<OnlinePilot> = list
        .into_iter()
        .filter(|p| own.as_deref() != Some(p.callsign.to_ascii_uppercase().as_str()))
        .filter_map(|mut p| {
            let d = great_circle_nm(lat, lon, p.latitude, p.longitude);
            p.distance_nm = Some(d);
            (d <= range_nm).then_some(p)
        })
        .collect();
    out.sort_by(|a, b| a.distance_nm.partial_cmp(&b.distance_nm).unwrap_or(std::cmp::Ordering::Equal));
    out
}

pub fn parse_pilot_feed(network: OnlineNetwork, feed: &Value) -> PilotSnapshot {
    let pilots = match network {
        OnlineNetwork::Vatsim => parse_vatsim_pilots(feed),
        OnlineNetwork::Ivao => parse_ivao_pilots(feed),
    };
    PilotSnapshot { network, feed_time_unix_s: feed_time_unix_s(network, feed), pilots }
}

pub fn fetch_online_pilots(
    client: &mut dyn HttpClient,
    network: OnlineNetwork,
    lat: f64,
    lon: f64,
    range_nm: f64,
    own_callsign: Option<&str>,
) -> Result<PilotSnapshot, String> {
    let response = client.get(network.feed_url(), "application/json")?;
    if !response.ok() {
        return Err(format!("{} feed returned HTTP {}", network.feed_url(), response.status));
    }
    let feed: Value = serde_json::from_str(&response.body).map_err(|e| format!("feed is not JSON: {e}"))?;
    let mut snap = parse_pilot_feed(network, &feed);
    snap.pilots = nearby_pilots(snap.pilots, lat, lon, range_nm, own_callsign);
    Ok(snap)
}

#[derive(Clone, Debug, PartialEq)]
pub struct PilotPoller {
    pub network: OnlineNetwork,
    pub interval_s: f64,
    next_due_s: f64,
    failures: u32,
}

impl PilotPoller {
    pub fn new(network: OnlineNetwork) -> PilotPoller {
        let interval_s = match network {
            OnlineNetwork::Vatsim => VATSIM_PILOT_UPDATE_S,
            OnlineNetwork::Ivao => IVAO_PILOT_UPDATE_S,
        };
        PilotPoller { network, interval_s, next_due_s: f64::NEG_INFINITY, failures: 0 }
    }

    pub fn due(&self, now_s: f64) -> bool {
        now_s >= self.next_due_s
    }

    pub fn record(&mut self, now_s: f64, ok: bool) {
        if ok {
            self.failures = 0;
            self.next_due_s = now_s + self.interval_s;
        } else {
            self.failures = self.failures.saturating_add(1);
            let wait = (self.interval_s * 2f64.powi(self.failures.min(8) as i32)).min(PILOT_RETRY_MAX_S);
            self.next_due_s = now_s + wait;
        }
    }

    pub fn poll(
        &mut self,
        client: &mut dyn HttpClient,
        now_s: f64,
        lat: f64,
        lon: f64,
        range_nm: f64,
        own_callsign: Option<&str>,
    ) -> Option<Result<PilotSnapshot, String>> {
        if !self.due(now_s) {
            return None;
        }
        let result = fetch_online_pilots(client, self.network, lat, lon, range_nm, own_callsign);
        self.record(now_s, result.is_ok());
        Some(result)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FlightPlanFields {
    pub callsign: String,
    pub flight_rules: String,
    pub flight_type: String,
    pub aircraft_type: String,
    pub wake: String,
    pub equipment: String,
    pub transponder: String,
    pub departure: String,
    pub destination: String,
    pub alternate: String,
    pub cruise_speed: String,
    pub cruise_level: String,
    pub route: String,
    pub departure_time: String,
    pub enroute_seconds: i64,
    pub endurance_seconds: i64,
    pub persons_on_board: i64,
    pub remarks: String,
}

fn string_at(value: &Value, path: &[&str]) -> String {
    let mut current = value;
    for key in path {
        match current.get(key) {
            Some(next) => current = next,
            None => return String::new(),
        }
    }
    match current {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn number_at(value: &Value, path: &[&str]) -> i64 {
    string_at(value, path).parse::<f64>().map(|v| v as i64).unwrap_or(0)
}

fn clock_from_epoch(epoch: i64) -> String {
    let seconds = epoch.rem_euclid(86400);
    format!("{:02}{:02}", seconds / 3600, (seconds % 3600) / 60)
}

impl FlightPlanFields {
    pub fn from_simbrief(ofp: &Value) -> FlightPlanFields {
        let callsign = {
            let atc = string_at(ofp, &["atc", "callsign"]);
            if atc.is_empty() {
                format!("{}{}", string_at(ofp, &["general", "icao_airline"]), string_at(ofp, &["general", "flight_number"]))
            } else {
                atc
            }
        };
        let level_feet = number_at(ofp, &["general", "initial_altitude"]);
        let speed = number_at(ofp, &["general", "cruise_tas"]);
        let mut remarks = string_at(ofp, &["atc", "flightplan_text"]);
        remarks.clear();
        let reg = string_at(ofp, &["aircraft", "reg"]);
        if !reg.is_empty() {
            remarks = format!("REG/{reg}");
        }
        let people = number_at(ofp, &["weights", "pax_count"]) + number_at(ofp, &["crew", "pilot_count"]) + number_at(ofp, &["crew", "cabin_crew"]);
        FlightPlanFields {
            callsign,
            flight_rules: "I".into(),
            flight_type: "S".into(),
            aircraft_type: string_at(ofp, &["aircraft", "icaocode"]),
            wake: "J".into(),
            equipment: string_at(ofp, &["aircraft", "equip"]),
            transponder: string_at(ofp, &["aircraft", "transponder"]),
            departure: string_at(ofp, &["origin", "icao_code"]),
            destination: string_at(ofp, &["destination", "icao_code"]),
            alternate: string_at(ofp, &["alternate", "icao_code"]),
            cruise_speed: format!("N{:04}", speed),
            cruise_level: format!("F{:03}", level_feet / 100),
            route: string_at(ofp, &["general", "route"]),
            departure_time: clock_from_epoch(number_at(ofp, &["times", "sched_out"])),
            enroute_seconds: number_at(ofp, &["times", "est_time_enroute"]),
            endurance_seconds: number_at(ofp, &["times", "endurance"]),
            persons_on_board: people,
            remarks,
        }
    }

    pub fn enroute_hhmm(&self) -> String {
        format!("{:02}{:02}", self.enroute_seconds / 3600, (self.enroute_seconds % 3600) / 60)
    }

    pub fn endurance_hhmm(&self) -> String {
        format!("{:02}{:02}", self.endurance_seconds / 3600, (self.endurance_seconds % 3600) / 60)
    }
}

pub fn fetch_simbrief_plan(client: &mut dyn HttpClient, settings: &OnlineSettings) -> Result<FlightPlanFields, String> {
    if settings.simbrief_username.is_empty() {
        return Err("SimBrief username missing".into());
    }
    let url = format!("{}?username={}&json=1", SIMBRIEF_FETCH_URL, crate::acars::form_urlencode(&settings.simbrief_username));
    let response = client.get(&url, "application/json")?;
    if !response.ok() {
        return Err(format!("SimBrief returned HTTP {}", response.status));
    }
    let ofp: Value = serde_json::from_str(&response.body).map_err(|e| format!("SimBrief reply is not JSON: {e}"))?;
    let fields = FlightPlanFields::from_simbrief(&ofp);
    if fields.departure.is_empty() || fields.destination.is_empty() {
        return Err("SimBrief reply carries no flight plan".into());
    }
    Ok(fields)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrefileOutcome {
    Filed,
    MissingCredentials,
    Rejected(u16),
    TransportError(String),
}

pub fn prefile_body(network: OnlineNetwork, plan: &FlightPlanFields) -> Value {
    match network {
        OnlineNetwork::Vatsim => json!({
            "callsign": plan.callsign,
            "flight_rules": plan.flight_rules,
            "aircraft_short": plan.aircraft_type,
            "aircraft_faa": format!("{}/{}", plan.wake, plan.aircraft_type),
            "departure": plan.departure,
            "arrival": plan.destination,
            "alternate": plan.alternate,
            "cruise_tas": plan.cruise_speed,
            "altitude": plan.cruise_level,
            "deptime": plan.departure_time,
            "enroute_time": plan.enroute_hhmm(),
            "fuel_time": plan.endurance_hhmm(),
            "route": plan.route,
            "remarks": plan.remarks,
        }),
        OnlineNetwork::Ivao => json!({
            "aircraftId": plan.aircraft_type,
            "aircraftNumber": 1,
            "departureId": plan.departure,
            "arrivalId": plan.destination,
            "alternativeId": plan.alternate,
            "route": plan.route,
            "remarks": plan.remarks,
            "speed": plan.cruise_speed,
            "level": plan.cruise_level,
            "flightRules": plan.flight_rules,
            "flightType": plan.flight_type,
            "eet": plan.enroute_seconds,
            "endurance": plan.endurance_seconds,
            "departureTime": plan.departure_time.parse::<i64>().map(|hhmm| (hhmm / 100) * 3600 + (hhmm % 100) * 60).unwrap_or(0),
            "peopleOnBoard": plan.persons_on_board,
            "aircraftEquipments": plan.equipment,
            "aircraftTransponderTypes": plan.transponder,
        }),
    }
}

pub fn prefile(
    client: &mut dyn HttpClient,
    settings: &OnlineSettings,
    network: OnlineNetwork,
    plan: &FlightPlanFields,
) -> PrefileOutcome {
    if !settings.credentials_present(network) {
        return PrefileOutcome::MissingCredentials;
    }
    let (url, token) = match network {
        OnlineNetwork::Vatsim => (
            settings.vatsim_prefile_url.clone().unwrap_or_else(|| VATSIM_PREFILE_URL.to_string()),
            settings.vatsim_token.reveal(),
        ),
        OnlineNetwork::Ivao => (
            settings.ivao_prefile_url.clone().unwrap_or_else(|| IVAO_PREFILE_URL.to_string()),
            settings.ivao_token.reveal(),
        ),
    };
    let body = prefile_body(network, plan).to_string();
    let authorization = format!("Bearer {token}");
    match client.post(&url, "application/json", Some(&authorization), &body) {
        Ok(response) if response.ok() => PrefileOutcome::Filed,
        Ok(response) => PrefileOutcome::Rejected(response.status),
        Err(error) => PrefileOutcome::TransportError(error.replace(token, "****")),
    }
}

pub fn build_pdc_request(plan: &FlightPlanFields, atis_letter: &str, gate: &str, station: &str) -> DclMessage {
    let mut message = DclMessage::new();
    message.cpdlc.header.station = station.to_string();
    message.callsign = plan.callsign.clone();
    message.origin = plan.departure.clone();
    message.destination = plan.destination.clone();
    message.ac_type = plan.aircraft_type.clone();
    message.atis = atis_letter.to_string();
    message.gate = gate.to_string();
    message
}

pub fn datalink_station_for(atc: &OnlineAtc, origin: &str) -> Option<String> {
    let prefix = atc.callsign.split('_').next()?;
    if matches!(atc.kind, AtcKind::Delivery | AtcKind::Ground | AtcKind::Tower | AtcKind::Approach)
        && (prefix == origin || (prefix.len() == 3 && origin.ends_with(prefix)))
    {
        Some(prefix.to_string())
    } else {
        None
    }
}

pub fn request_pdc(
    connector: &AcarsConnector,
    client: &mut dyn HttpClient,
    acars: &AcarsSettings,
    station: &str,
    plan: &FlightPlanFields,
    atis_letter: &str,
    gate: &str,
) -> AtsuStatusCodes {
    let available = connector.is_station_available(client, acars, station);
    if available != AtsuStatusCodes::Ok {
        return available;
    }
    let message = build_pdc_request(plan, atis_letter, gate, station);
    connector.send_dcl_message(client, acars, &message, false)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OooiEvent {
    Out,
    Off,
    On,
    In,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OooiSample {
    pub time_s: f64,
    pub on_ground: bool,
    pub parking_brake: bool,
    pub ground_speed_kt: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OooiTimes {
    pub out: Option<f64>,
    pub off: Option<f64>,
    pub on: Option<f64>,
    pub in_: Option<f64>,
}

#[derive(Default)]
pub struct OooiTracker {
    times: OooiTimes,
    was_on_ground: Option<bool>,
}

impl OooiTracker {
    pub fn new() -> OooiTracker {
        OooiTracker::default()
    }

    pub fn times(&self) -> &OooiTimes {
        &self.times
    }

    pub fn update(&mut self, sample: &OooiSample) -> Vec<OooiEvent> {
        let mut events = Vec::new();
        if self.times.out.is_none() && sample.on_ground && !sample.parking_brake && sample.ground_speed_kt > 0.5 {
            self.times.out = Some(sample.time_s);
            events.push(OooiEvent::Out);
        }
        if let Some(was) = self.was_on_ground {
            if self.times.out.is_some() && self.times.off.is_none() && was && !sample.on_ground {
                self.times.off = Some(sample.time_s);
                events.push(OooiEvent::Off);
            } else if self.times.off.is_some() && self.times.on.is_none() && !was && sample.on_ground {
                self.times.on = Some(sample.time_s);
                events.push(OooiEvent::On);
            }
        }
        if self.times.on.is_some()
            && self.times.in_.is_none()
            && sample.on_ground
            && sample.parking_brake
            && sample.ground_speed_kt < 0.5
        {
            self.times.in_ = Some(sample.time_s);
            events.push(OooiEvent::In);
        }
        self.was_on_ground = Some(sample.on_ground);
        events
    }
}

fn hhmm(seconds: f64) -> String {
    let s = seconds.rem_euclid(86400.0) as i64;
    format!("{:02}{:02}", s / 3600, (s % 3600) / 60)
}

pub fn format_oooi_report(plan: &FlightPlanFields, times: &OooiTimes) -> String {
    let field = |label: &str, value: Option<f64>| match value {
        Some(v) => format!("{label} {}Z", hhmm(v)),
        None => format!("{label} ----"),
    };
    format!(
        "{} {}-{}\n{}\n{}\n{}\n{}",
        plan.callsign,
        plan.departure,
        plan.destination,
        field("OUT", times.out),
        field("OFF", times.off),
        field("ON", times.on),
        field("IN", times.in_)
    )
}

pub fn send_oooi_telex(
    connector: &AcarsConnector,
    client: &mut dyn HttpClient,
    acars: &AcarsSettings,
    company_station: &str,
    plan: &FlightPlanFields,
    times: &OooiTimes,
) -> AtsuStatusCodes {
    let mut message = crate::message::FreetextMessage::new();
    message.header.station = company_station.to_string();
    message.header.message = format_oooi_report(plan, times);
    connector.send_telex_message(client, acars, &message, false)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompanyMessageKind {
    Oooi,
    Metar,
    Taf,
    Notam,
    Freetext,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompanyMessage {
    pub kind: CompanyMessageKind,
    pub station: String,
    pub text: String,
}

pub trait NotamSource {
    fn notams(&mut self, icao: &str) -> Result<Vec<String>, String>;
}

pub trait WeatherSource {
    fn metar(&mut self, icao: &str) -> Option<String>;
    fn taf(&mut self, icao: &str) -> Option<String>;
    fn atis(&mut self, _icao: &str, _kind: crate::weather::AtisType) -> Option<String> {
        None
    }
}

pub fn company_weather_uplinks(
    weather: &mut dyn WeatherSource,
    notams: &mut dyn NotamSource,
    plan: &FlightPlanFields,
) -> Vec<CompanyMessage> {
    let mut out = Vec::new();
    let mut stations: Vec<&str> = vec![plan.departure.as_str(), plan.destination.as_str()];
    if !plan.alternate.is_empty() {
        stations.push(plan.alternate.as_str());
    }
    for icao in stations {
        if icao.is_empty() {
            continue;
        }
        if let Some(metar) = weather.metar(icao) {
            out.push(CompanyMessage { kind: CompanyMessageKind::Metar, station: icao.into(), text: metar });
        }
        if let Some(taf) = weather.taf(icao) {
            out.push(CompanyMessage { kind: CompanyMessageKind::Taf, station: icao.into(), text: taf });
        }
        if let Ok(list) = notams.notams(icao) {
            for text in list {
                out.push(CompanyMessage { kind: CompanyMessageKind::Notam, station: icao.into(), text });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acars::{AcarsProvider, HttpResponse};
    use std::collections::VecDeque;

    pub struct Scripted {
        pub log: Vec<(String, String, String)>,
        pub replies: VecDeque<Result<HttpResponse, String>>,
    }

    impl Scripted {
        fn new(replies: Vec<Result<HttpResponse, String>>) -> Scripted {
            Scripted { log: Vec::new(), replies: replies.into() }
        }
    }

    impl HttpClient for Scripted {
        fn get(&mut self, url: &str, _accept: &str) -> Result<HttpResponse, String> {
            self.log.push(("GET".into(), url.into(), String::new()));
            self.replies.pop_front().unwrap_or_else(|| Err("no reply".into()))
        }

        fn post(&mut self, url: &str, content_type: &str, authorization: Option<&str>, body: &str) -> Result<HttpResponse, String> {
            self.log.push(("POST".into(), format!("{url}|{content_type}|{}", authorization.unwrap_or("")), body.into()));
            self.replies.pop_front().unwrap_or_else(|| Err("no reply".into()))
        }
    }

    fn ok(body: &str) -> Result<HttpResponse, String> {
        Ok(HttpResponse { status: 200, body: body.into() })
    }

    struct Airports;

    impl PositionLookup for Airports {
        fn airport_position(&self, icao: &str) -> Option<(f64, f64)> {
            match icao {
                "EDDF" => Some((50.033, 8.570)),
                "KJFK" => Some((40.639, -73.779)),
                "EDDM" => Some((48.354, 11.786)),
                _ => None,
            }
        }
    }

    const VATSIM: &str = include_str!("../tests/fixtures/vatsim_sample.json");
    const IVAO: &str = include_str!("../tests/fixtures/ivao_sample.json");

    #[test]
    fn secrets_never_print() {
        let s = Secret::new("abcdef123456");
        assert_eq!(s.masked(), "a**********6");
        assert!(!format!("{:?}", s).contains("cdef"));
        let settings = OnlineSettings { vatsim_cid: "1000000".into(), vatsim_token: s, ..Default::default() };
        assert!(!settings.display_summary().contains("abcdef"));
        assert!(!format!("{:?}", settings).contains("abcdef123456"));
        assert_eq!(Secret::new("ab").masked(), "**");
    }

    #[test]
    fn vatsim_feed_is_parsed_and_filtered_by_range() {
        let mut client = Scripted::new(vec![ok(VATSIM)]);
        let list = fetch_online_atc(&mut client, OnlineNetwork::Vatsim, &Airports, 50.05, 8.55, 120.0).unwrap();
        assert_eq!(client.log[0].1, VATSIM_DATA_FEED_URL);
        let callsigns: Vec<&str> = list.iter().map(|a| a.callsign.as_str()).collect();
        assert_eq!(callsigns, vec!["EDDF_TWR", "EDDF_ATIS", "EDGG_CTR"]);
        assert_eq!(list[0].kind, AtcKind::Tower);
        assert!((list[0].frequency_mhz - 118.7).abs() < 1e-6);
        assert!(list[0].distance_nm.unwrap() < 5.0);
        assert_eq!(list[1].atis_code.as_deref(), Some("B"));
        assert!(list[2].distance_nm.is_none());
        assert!(!callsigns.contains(&"KJFK_GND"));
        assert!(!callsigns.contains(&"EDDM_TWR"));
    }

    #[test]
    fn three_letter_us_callsigns_resolve_through_the_lookup() {
        let mut client = Scripted::new(vec![ok(VATSIM)]);
        let list = fetch_online_atc(&mut client, OnlineNetwork::Vatsim, &Airports, 40.7, -73.8, 60.0).unwrap();
        assert_eq!(list.iter().map(|a| a.callsign.as_str()).collect::<Vec<_>>(), vec!["JFK_GND", "EDGG_CTR"]);
    }

    #[test]
    fn ivao_whazzup_uses_track_positions() {
        let mut client = Scripted::new(vec![ok(IVAO)]);
        let list = fetch_online_atc(&mut client, OnlineNetwork::Ivao, &NoLookup, 43.4, 5.2, 100.0).unwrap();
        assert_eq!(client.log[0].1, IVAO_WHAZZUP_URL);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].callsign, "LFML_TWR");
        assert_eq!(list[0].kind, AtcKind::Tower);
        assert!((list[0].frequency_mhz - 133.1).abs() < 1e-6);
        assert_eq!(list[0].atis_code.as_deref(), Some("K"));
    }

    #[test]
    fn feed_errors_are_reported() {
        let mut client = Scripted::new(vec![Ok(HttpResponse { status: 503, body: String::new() })]);
        assert!(fetch_online_atc(&mut client, OnlineNetwork::Vatsim, &NoLookup, 0.0, 0.0, 50.0).is_err());
        let mut client = Scripted::new(vec![ok("not json")]);
        assert!(fetch_online_atc(&mut client, OnlineNetwork::Ivao, &NoLookup, 0.0, 0.0, 50.0).is_err());
    }

    fn ofp() -> Value {
        json!({
            "general": {"icao_airline": "DLH", "flight_number": "400", "initial_altitude": "36000", "cruise_tas": "487", "route": "ANEKI Z74 SULUS"},
            "atc": {"callsign": "DLH400"},
            "aircraft": {"icaocode": "A388", "equip": "SDE3FGHIRWXY/LB1D1", "transponder": "LB1D1", "reg": "DAIMA"},
            "origin": {"icao_code": "EDDF"},
            "destination": {"icao_code": "KJFK"},
            "alternate": {"icao_code": "KEWR"},
            "times": {"sched_out": "1792670400", "est_time_enroute": "30300", "endurance": "37800"},
            "weights": {"pax_count": "400"},
            "crew": {"pilot_count": "3", "cabin_crew": "18"}
        })
    }

    #[test]
    fn simbrief_ofp_maps_to_icao_fields() {
        let plan = FlightPlanFields::from_simbrief(&ofp());
        assert_eq!(plan.callsign, "DLH400");
        assert_eq!(plan.cruise_level, "F360");
        assert_eq!(plan.cruise_speed, "N0487");
        assert_eq!(plan.enroute_hhmm(), "0825");
        assert_eq!(plan.endurance_hhmm(), "1030");
        assert_eq!(plan.persons_on_board, 421);
        assert_eq!(plan.remarks, "REG/DAIMA");
        assert_eq!(plan.departure_time.len(), 4);
    }

    #[test]
    fn simbrief_fetch_builds_the_documented_url() {
        let mut client = Scripted::new(vec![ok(&ofp().to_string())]);
        let settings = OnlineSettings { simbrief_username: "some pilot".into(), ..Default::default() };
        let plan = fetch_simbrief_plan(&mut client, &settings).unwrap();
        assert_eq!(client.log[0].1, "https://www.simbrief.com/api/xml.fetcher.php?username=some+pilot&json=1");
        assert_eq!(plan.destination, "KJFK");
        let mut client = Scripted::new(vec![ok("{}")]);
        assert!(fetch_simbrief_plan(&mut client, &settings).is_err());
    }

    #[test]
    fn prefile_posts_the_plan_and_hides_the_token() {
        let plan = FlightPlanFields::from_simbrief(&ofp());
        let settings = OnlineSettings {
            vatsim_cid: "1000000".into(),
            vatsim_token: Secret::new("tok-secret-1"),
            ivao_vid: "200000".into(),
            ivao_token: Secret::new("ivao-secret-2"),
            ..Default::default()
        };
        let mut client = Scripted::new(vec![ok("{}")]);
        assert_eq!(prefile(&mut client, &settings, OnlineNetwork::Vatsim, &plan), PrefileOutcome::Filed);
        assert_eq!(client.log[0].0, "POST");
        assert!(client.log[0].1.starts_with(VATSIM_PREFILE_URL));
        assert!(client.log[0].1.ends_with("|application/json|Bearer tok-secret-1"));
        let body: Value = serde_json::from_str(&client.log[0].2).unwrap();
        assert_eq!(body["departure"], "EDDF");
        assert_eq!(body["arrival"], "KJFK");
        assert_eq!(body["altitude"], "F360");
        assert_eq!(body["enroute_time"], "0825");

        let mut client = Scripted::new(vec![Ok(HttpResponse { status: 401, body: String::new() })]);
        assert_eq!(prefile(&mut client, &settings, OnlineNetwork::Ivao, &plan), PrefileOutcome::Rejected(401));
        let body: Value = serde_json::from_str(&client.log[0].2).unwrap();
        assert_eq!(body["arrivalId"], "KJFK");
        assert_eq!(body["eet"], 30300);

        let mut client = Scripted::new(vec![Err("connect failed for ivao-secret-2".into())]);
        match prefile(&mut client, &settings, OnlineNetwork::Ivao, &plan) {
            PrefileOutcome::TransportError(text) => assert!(!text.contains("ivao-secret-2")),
            other => panic!("{other:?}"),
        }

        let mut client = Scripted::new(vec![]);
        assert_eq!(
            prefile(&mut client, &OnlineSettings::default(), OnlineNetwork::Vatsim, &plan),
            PrefileOutcome::MissingCredentials
        );
        assert!(client.log.is_empty());

        let custom = OnlineSettings { vatsim_prefile_url: Some("https://example.invalid/fp".into()), ..settings.clone() };
        let mut client = Scripted::new(vec![ok("{}")]);
        prefile(&mut client, &custom, OnlineNetwork::Vatsim, &plan);
        assert!(client.log[0].1.starts_with("https://example.invalid/fp"));
    }

    fn acars() -> AcarsSettings {
        AcarsSettings { provider: AcarsProvider::Hoppie, hoppie_user_id: "ABC123".into(), sai_logon_key: String::new() }
    }

    #[test]
    fn pdc_goes_out_as_hoppie_telex_after_a_ping() {
        let plan = FlightPlanFields::from_simbrief(&ofp());
        let mut connector = AcarsConnector::new();
        connector.acars_active = true;
        connector.flight_number = "DLH400".into();
        let mut client = Scripted::new(vec![ok("ok {EDDF-DEL}"), ok("ok")]);
        let code = request_pdc(&connector, &mut client, &acars(), "EDDF", &plan, "B", "A26");
        assert_eq!(code, AtsuStatusCodes::Ok);
        assert!(client.log[0].1.contains("type=ping"));
        assert!(client.log[1].1.contains("type=telex"));
        assert!(client.log[1].1.contains("to=EDDF"));
        assert!(client.log[1].1.contains("REQUEST+PREDEP+CLEARANCE"));
        assert!(client.log[1].1.contains("STAND+A26"));

        let mut client = Scripted::new(vec![ok("error {no such station}")]);
        assert_ne!(request_pdc(&connector, &mut client, &acars(), "EDDF", &plan, "B", ""), AtsuStatusCodes::Ok);
        assert_eq!(client.log.len(), 1);
    }

    #[test]
    fn pdc_station_choice_follows_the_origin() {
        let list = parse_vatsim_feed(&serde_json::from_str(VATSIM).unwrap(), &Airports);
        let del: Vec<_> = list.iter().filter_map(|a| datalink_station_for(a, "EDDF")).collect();
        assert_eq!(del, vec!["EDDF".to_string()]);
    }

    #[test]
    fn oooi_tracker_walks_out_off_on_in() {
        let mut tracker = OooiTracker::new();
        let sample = |t: f64, ground: bool, brake: bool, gs: f64| OooiSample { time_s: t, on_ground: ground, parking_brake: brake, ground_speed_kt: gs };
        assert!(tracker.update(&sample(36000.0, true, true, 0.0)).is_empty());
        assert_eq!(tracker.update(&sample(36300.0, true, false, 3.0)), vec![OooiEvent::Out]);
        assert!(tracker.update(&sample(37000.0, true, false, 120.0)).is_empty());
        assert_eq!(tracker.update(&sample(37100.0, false, false, 160.0)), vec![OooiEvent::Off]);
        assert_eq!(tracker.update(&sample(70000.0, true, false, 130.0)), vec![OooiEvent::On]);
        assert!(tracker.update(&sample(70600.0, true, false, 10.0)).is_empty());
        assert_eq!(tracker.update(&sample(70900.0, true, true, 0.0)), vec![OooiEvent::In]);
        assert!(tracker.update(&sample(71000.0, true, true, 0.0)).is_empty());

        let plan = FlightPlanFields::from_simbrief(&ofp());
        let report = format_oooi_report(&plan, tracker.times());
        assert_eq!(report, "DLH400 EDDF-KJFK\nOUT 1005Z\nOFF 1018Z\nON 1926Z\nIN 1941Z");
    }

    #[test]
    fn oooi_report_is_sent_as_telex() {
        let plan = FlightPlanFields::from_simbrief(&ofp());
        let mut connector = AcarsConnector::new();
        connector.acars_active = true;
        connector.flight_number = "DLH400".into();
        let mut client = Scripted::new(vec![ok("ok")]);
        let times = OooiTimes { out: Some(36300.0), ..Default::default() };
        let code = send_oooi_telex(&connector, &mut client, &acars(), "DLHOPS", &plan, &times);
        assert_eq!(code, AtsuStatusCodes::Ok);
        assert!(client.log[0].1.contains("to=DLHOPS"));
        assert!(client.log[0].1.contains("OUT+1005Z"));
        assert!(client.log[0].1.contains("OFF+----"));
    }

    struct Wx;
    impl WeatherSource for Wx {
        fn metar(&mut self, icao: &str) -> Option<String> {
            Some(format!("{icao} 221050Z 25006KT CAVOK"))
        }
        fn taf(&mut self, icao: &str) -> Option<String> {
            if icao == "EDDF" { Some("TAF EDDF".into()) } else { None }
        }
    }
    struct Notams;
    impl NotamSource for Notams {
        fn notams(&mut self, icao: &str) -> Result<Vec<String>, String> {
            if icao == "KJFK" { Ok(vec!["RWY 04L/22R CLSD".into()]) } else { Err("no feed".into()) }
        }
    }

    #[test]
    fn company_uplinks_cover_all_planned_airports() {
        let plan = FlightPlanFields::from_simbrief(&ofp());
        let list = company_weather_uplinks(&mut Wx, &mut Notams, &plan);
        let kinds: Vec<(CompanyMessageKind, &str)> = list.iter().map(|m| (m.kind, m.station.as_str())).collect();
        assert_eq!(
            kinds,
            vec![
                (CompanyMessageKind::Metar, "EDDF"),
                (CompanyMessageKind::Taf, "EDDF"),
                (CompanyMessageKind::Metar, "KJFK"),
                (CompanyMessageKind::Notam, "KJFK"),
                (CompanyMessageKind::Metar, "KEWR"),
            ]
        );
    }

    const VATSIM_PILOTS: &str = include_str!("../tests/fixtures/vatsim_pilots.json");
    const IVAO_PILOTS: &str = include_str!("../tests/fixtures/ivao_pilots.json");

    #[test]
    fn iso_timestamps_become_unix_seconds() {
        assert_eq!(parse_iso_utc("2026-10-22T12:00:00Z"), Some(1792670400.0));
        assert_eq!(parse_iso_utc("2000-02-29T00:00:00.000Z"), Some(951782400.0));
        let frac = parse_iso_utc("2026-10-22T11:59:22.7058857Z").unwrap();
        assert!((frac - (1792670400.0 - 37.2941143)).abs() < 1e-6);
        assert_eq!(parse_iso_utc("2026-13-01T00:00:00Z"), None);
        assert_eq!(parse_iso_utc("garbage"), None);
    }

    #[test]
    fn vatsim_pilots_are_parsed() {
        let snap = parse_pilot_feed(OnlineNetwork::Vatsim, &serde_json::from_str(VATSIM_PILOTS).unwrap());
        assert_eq!(snap.feed_time_unix_s, Some(1792670400.0));
        let names: Vec<&str> = snap.pilots.iter().map(|p| p.callsign.as_str()).collect();
        assert_eq!(names, vec!["TST101", "TST202", "OWN380", "TST404"]);
        let p = &snap.pilots[0];
        assert_eq!((p.latitude, p.longitude, p.altitude_ft, p.groundspeed_kt, p.heading_deg), (50.1, 8.6, 12000.0, 280.0, 90.0));
        assert_eq!(p.transponder.as_deref(), Some("2000"));
        assert_eq!(p.aircraft_type, "B738");
        assert_eq!(p.on_ground, None);
        assert!((p.age_s(1792670400.0).unwrap() - 4.5).abs() < 1e-6);
        assert_eq!(snap.pilots[1].aircraft_type, "");
        assert_eq!(snap.pilots[3].transponder, None);
        assert_eq!(snap.pilots[3].heading_deg, 350.0);
    }

    #[test]
    fn ivao_pilots_are_parsed() {
        let snap = parse_pilot_feed(OnlineNetwork::Ivao, &serde_json::from_str(IVAO_PILOTS).unwrap());
        assert_eq!(snap.feed_time_unix_s, Some(1792670400.0));
        assert_eq!(snap.pilots.len(), 2);
        let p = &snap.pilots[0];
        assert_eq!(p.callsign, "TST510");
        assert_eq!((p.altitude_ft, p.groundspeed_kt, p.heading_deg), (8000.0, 250.0, 180.0));
        assert_eq!(p.transponder.as_deref(), Some("2000"));
        assert_eq!(p.aircraft_type, "A20N");
        assert_eq!(p.on_ground, Some(false));
        assert_eq!(p.last_update_unix_s, Some(1792670390.0));
        assert_eq!(snap.pilots[1].on_ground, Some(true));
        assert_eq!(snap.pilots[1].transponder.as_deref(), Some("7000"));
    }

    #[test]
    fn nearby_pilots_drop_far_and_own_callsign() {
        let mut client = Scripted::new(vec![ok(VATSIM_PILOTS)]);
        let snap = fetch_online_pilots(&mut client, OnlineNetwork::Vatsim, 50.05, 8.55, PILOT_RANGE_NM, Some("own380")).unwrap();
        assert_eq!(client.log[0].1, VATSIM_DATA_FEED_URL);
        let names: Vec<&str> = snap.pilots.iter().map(|p| p.callsign.as_str()).collect();
        assert_eq!(names, vec!["TST101", "TST404"]);
        assert!(snap.pilots[0].distance_nm.unwrap() < snap.pilots[1].distance_nm.unwrap());
        let mut client = Scripted::new(vec![ok(IVAO_PILOTS)]);
        let snap = fetch_online_pilots(&mut client, OnlineNetwork::Ivao, 43.44, 5.22, 5.0, None).unwrap();
        assert_eq!(snap.pilots.iter().map(|p| p.callsign.as_str()).collect::<Vec<_>>(), vec!["TST620", "TST510"]);
    }

    #[test]
    fn pilot_poller_waits_the_interval_and_backs_off() {
        let mut poller = PilotPoller::new(OnlineNetwork::Vatsim);
        let mut client = Scripted::new(vec![ok(VATSIM_PILOTS), Err("down".into()), Err("down".into())]);
        assert!(poller.poll(&mut client, 100.0, 50.0, 8.5, 80.0, None).unwrap().is_ok());
        assert!(poller.poll(&mut client, 110.0, 50.0, 8.5, 80.0, None).is_none());
        assert!(poller.poll(&mut client, 115.0, 50.0, 8.5, 80.0, None).unwrap().is_err());
        assert!(!poller.due(140.0));
        assert!(poller.due(145.0));
        assert!(poller.poll(&mut client, 145.0, 50.0, 8.5, 80.0, None).unwrap().is_err());
        assert!(!poller.due(204.0));
        assert!(poller.due(205.0));
        assert_eq!(client.log.len(), 3);
    }
}

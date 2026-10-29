use serde::Deserialize;
use serde_json::Value;

use crate::acars::{form_urlencode, AcarsProvider, AcarsSettings, HttpClient};
use crate::online::Secret;

pub const SAPI_BASE_URL: &str = "https://apipri.sayintentions.ai/sapi/";
pub const FLIGHT_JSON_URL: &str = "http://localhost:43117/flightJSON";
pub const DEVELOPER_KEY_HEADER: &str = "X-Developer-Key";

#[derive(Clone, Debug, Default)]
pub struct SaiConfig {
    pub api_key: Secret,
    pub developer_key: Option<Secret>,
}

impl SaiConfig {
    pub fn configured(&self) -> bool {
        !self.api_key.is_empty()
    }
}

pub fn acars_settings(logon_key: &str) -> AcarsSettings {
    AcarsSettings { provider: AcarsProvider::Sai, hoppie_user_id: String::new(), sai_logon_key: logon_key.to_string() }
}

pub fn sapi_url(endpoint: &str, api_key: &str, params: &[(&str, String)]) -> String {
    let mut url = format!("{SAPI_BASE_URL}{endpoint}?api_key={}", form_urlencode(api_key));
    for (k, v) in params {
        url.push('&');
        url.push_str(k);
        url.push('=');
        url.push_str(&form_urlencode(v));
    }
    url
}

fn sapi_get(client: &mut dyn HttpClient, cfg: &SaiConfig, endpoint: &str, params: &[(&str, String)]) -> Result<Value, String> {
    if !cfg.configured() {
        return Err("SayIntentions API key missing".into());
    }
    let url = sapi_url(endpoint, cfg.api_key.reveal(), params);
    let response = match &cfg.developer_key {
        Some(key) if !key.is_empty() => client.get_with_header(&url, DEVELOPER_KEY_HEADER, key.reveal()),
        _ => client.get(&url, "application/json"),
    }
    .map_err(|e| e.replace(cfg.api_key.reveal(), &cfg.api_key.masked()))?;
    let body: Value = serde_json::from_str(&response.body).unwrap_or(Value::Null);
    if !response.ok() {
        let reason = body.get("error").and_then(Value::as_str).unwrap_or("request failed");
        return Err(format!("SayIntentions {endpoint}: HTTP {} {reason}", response.status));
    }
    if let Some(e) = body.get("error").and_then(Value::as_str) {
        return Err(format!("SayIntentions {endpoint}: {e}"));
    }
    Ok(body)
}

fn num_or_text(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct CommsEntry {
    pub id: i64,
    #[serde(default)]
    pub frequency: Value,
    #[serde(default)]
    pub is_acars: Value,
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub stamp_zulu: String,
    #[serde(default)]
    pub station_name: String,
    #[serde(default)]
    pub incoming_message: String,
    #[serde(default)]
    pub outgoing_message: String,
}

impl CommsEntry {
    pub fn frequency_mhz(&self) -> Option<f64> {
        num_or_text(&self.frequency).filter(|f| *f > 100.0)
    }

    pub fn acars(&self) -> bool {
        num_or_text(&self.is_acars).is_some_and(|v| v != 0.0) || self.is_acars == Value::Bool(true)
    }
}

pub fn parse_comms_history(body: &Value) -> Vec<CommsEntry> {
    body.get("comm_history")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect())
        .unwrap_or_default()
}

pub fn comms_history(client: &mut dyn HttpClient, cfg: &SaiConfig, since_id: i64) -> Result<Vec<CommsEntry>, String> {
    sapi_get(client, cfg, "getCommsHistory", &[("since_id", since_id.to_string())]).map(|b| parse_comms_history(&b))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Handoff {
    pub accepted: bool,
    pub station: String,
    pub frequency_mhz: Option<f64>,
    pub reason: Option<String>,
}

pub fn accept_last_handoff(client: &mut dyn HttpClient, cfg: &SaiConfig) -> Result<Handoff, String> {
    let b = sapi_get(client, cfg, "acceptLastHandoff", &[])?;
    Ok(Handoff {
        accepted: b.get("accepted").and_then(Value::as_bool).unwrap_or(false),
        station: b.get("station").and_then(Value::as_str).unwrap_or_default().to_string(),
        frequency_mhz: b.get("frequency").and_then(num_or_text),
        reason: b.get("reason").and_then(Value::as_str).map(str::to_string),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComSlot {
    Com1Active,
    Com1Standby,
    Com2Active,
    Com2Standby,
}

pub fn set_freq(client: &mut dyn HttpClient, cfg: &SaiConfig, mhz: f64, slot: ComSlot) -> Result<(), String> {
    let (com, mode) = match slot {
        ComSlot::Com1Active => ("1", "active"),
        ComSlot::Com1Standby => ("1", "standby"),
        ComSlot::Com2Active => ("2", "active"),
        ComSlot::Com2Standby => ("2", "standby"),
    };
    sapi_get(client, cfg, "setFreq", &[("freq", format!("{mhz:.3}")), ("com", com.into()), ("mode", mode.into())]).map(|_| ())
}

#[derive(Clone, Debug, PartialEq)]
pub struct AirspaceFrequency {
    pub kind: String,
    pub callsign: String,
    pub frequency_mhz: f64,
    pub airport: Option<String>,
    pub distance_nm: Option<f64>,
}

pub fn airspace_frequencies(client: &mut dyn HttpClient, cfg: &SaiConfig, lat: f64, lon: f64, ifr: bool) -> Result<Vec<AirspaceFrequency>, String> {
    let rules = if ifr { "IFR" } else { "VFR" };
    let b = sapi_get(client, cfg, "getAirSpaceFrequencies", &[("lat", format!("{lat:.5}")), ("lon", format!("{lon:.5}")), ("flight_rules", rules.into())])?;
    Ok(b.get("frequencies")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|f| {
                    let mhz = f.get("frequency").and_then(num_or_text).or_else(|| f.get("frequency_khz").and_then(num_or_text).map(|k| k / 1000.0))?;
                    Some(AirspaceFrequency {
                        kind: f.get("type").and_then(Value::as_str).unwrap_or_default().to_string(),
                        callsign: f.get("callsign").and_then(Value::as_str).unwrap_or_default().to_string(),
                        frequency_mhz: mhz,
                        airport: f.get("airport").and_then(Value::as_str).map(str::to_string),
                        distance_nm: f.get("distance_nm").and_then(num_or_text),
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

pub fn assign_gate(client: &mut dyn HttpClient, cfg: &SaiConfig, airport: &str, gate: &str) -> Result<String, String> {
    let gate: String = gate.chars().filter(char::is_ascii_alphanumeric).take(30).collect();
    let b = sapi_get(client, cfg, "assignGate", &[("gate", gate), ("airport", airport.to_ascii_uppercase())])?;
    Ok(b.get("assigned_gate_name").and_then(Value::as_str).unwrap_or_default().to_string())
}

#[derive(Clone, Debug, PartialEq)]
pub enum SimApiCommand {
    Tune { slot: ComSlot, hz: u64 },
    Transponder(u16),
    Other { var: String, value: String },
}

pub fn parse_simapi_output(text: &str) -> Vec<SimApiCommand> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line.trim()) else { continue };
        let Some(var) = v.get("setvar").and_then(Value::as_str) else { continue };
        let value = match v.get("value") {
            Some(Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => continue,
        };
        let slot = match var {
            "COM_RADIO_SET_HZ" | "COM1_RADIO_SET_HZ" => Some(ComSlot::Com1Active),
            "COM_STBY_RADIO_SET_HZ" | "COM1_STBY_RADIO_SET_HZ" => Some(ComSlot::Com1Standby),
            "COM2_RADIO_SET_HZ" => Some(ComSlot::Com2Active),
            "COM2_STBY_RADIO_SET_HZ" => Some(ComSlot::Com2Standby),
            _ => None,
        };
        match (slot, var) {
            (Some(slot), _) => {
                if let Ok(hz) = value.trim().parse::<f64>() {
                    out.push(SimApiCommand::Tune { slot, hz: hz.round() as u64 });
                }
            }
            (None, "TRANSPONDER_CODE_SET") => {
                if let Ok(code) = value.trim().parse() {
                    out.push(SimApiCommand::Transponder(code));
                }
            }
            _ => out.push(SimApiCommand::Other { var: var.to_string(), value }),
        }
    }
    out
}

pub fn assigned_frequency(message: &str) -> Option<f64> {
    let upper = message.to_ascii_uppercase();
    let mut found = None;
    for word in upper.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
        if let Some((a, b)) = word.split_once('.') {
            if a.len() == 3 && !b.is_empty() && b.len() <= 3 {
                if let Ok(f) = word.parse::<f64>() {
                    if (118.0..=136.991).contains(&f) {
                        found = Some(f);
                    }
                }
            }
        }
    }
    found
}

#[derive(Debug, Default)]
pub struct AutoTuner {
    last_id: i64,
    last_tuned: Option<u64>,
}

impl AutoTuner {
    pub fn last_id(&self) -> i64 {
        self.last_id
    }

    pub fn feed(&mut self, entries: &[CommsEntry]) -> Option<SimApiCommand> {
        let mut tune = None;
        for e in entries {
            if e.id <= self.last_id {
                continue;
            }
            self.last_id = e.id;
            if e.acars() {
                continue;
            }
            if let Some(f) = assigned_frequency(&e.incoming_message) {
                tune = Some((f * 1000.0).round() as u64 * 1000);
            }
        }
        let hz = tune?;
        if self.last_tuned == Some(hz) {
            return None;
        }
        self.last_tuned = Some(hz);
        Some(SimApiCommand::Tune { slot: ComSlot::Com1Standby, hz })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SaiFlight {
    pub callsign: String,
    pub origin: String,
    pub destination: String,
    pub route: String,
    pub sid: String,
    pub star: String,
    pub departure_runway: String,
    pub arrival_runway: String,
    pub assigned_gate: String,
}

pub fn parse_flight_json(body: &Value) -> Option<SaiFlight> {
    let d = body.get("flight_details")?;
    let c = d.get("current_flight").cloned().unwrap_or(Value::Null);
    let t = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().trim().to_string();
    Some(SaiFlight {
        callsign: t(d, "callsign"),
        origin: t(&c, "flight_origin"),
        destination: t(&c, "flight_destination"),
        route: t(&c, "flight_plan_route"),
        sid: t(&c, "flight_plan_sid"),
        star: t(&c, "flight_plan_star"),
        departure_runway: t(&c, "flight_plan_departing_runway"),
        arrival_runway: t(&c, "flight_plan_arriving_runway"),
        assigned_gate: t(&c, "assigned_gate"),
    })
}

pub fn fetch_flight(client: &mut dyn HttpClient) -> Result<SaiFlight, String> {
    let r = client.get(FLIGHT_JSON_URL, "application/json")?;
    if !r.ok() {
        return Err(format!("SayIntentions flight.json: HTTP {}", r.status));
    }
    let body: Value = serde_json::from_str(&r.body).map_err(|e| format!("flight.json is not JSON: {e}"))?;
    parse_flight_json(&body).ok_or_else(|| "flight.json has no flight_details".into())
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FmsPlan {
    pub origin: String,
    pub destination: String,
    pub departure_runway: String,
    pub sid: String,
    pub route: String,
}

pub fn plan_differences(fms: &FmsPlan, sai: &SaiFlight) -> Vec<&'static str> {
    let differs = |a: &str, b: &str| !a.is_empty() && !b.is_empty() && !a.eq_ignore_ascii_case(b);
    let route_words = |r: &str| r.split_whitespace().filter(|w| *w != "DCT").map(str::to_ascii_uppercase).collect::<Vec<_>>();
    let mut out = Vec::new();
    if differs(&fms.origin, &sai.origin) {
        out.push("ORIGIN");
    }
    if differs(&fms.destination, &sai.destination) {
        out.push("DESTINATION");
    }
    if differs(fms.departure_runway.trim_start_matches("RW"), &sai.departure_runway) {
        out.push("DEPARTURE RUNWAY");
    }
    if differs(&fms.sid, &sai.sid) {
        out.push("SID");
    }
    if !fms.route.is_empty() && !sai.route.is_empty() && route_words(&fms.route) != route_words(&sai.route) {
        out.push("ROUTE");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acars::tests::{body, ScriptedClient};
    use crate::acars::HttpResponse;

    const COMMS: &str = include_str!("../tests/fixtures/sayintentions_comms.json");
    const FLIGHT: &str = include_str!("../tests/fixtures/sayintentions_flight.json");
    const SIMAPI: &str = include_str!("../tests/fixtures/sayintentions_simapi_output.jsonl");

    fn cfg() -> SaiConfig {
        SaiConfig { api_key: Secret::new("abcdef123456"), developer_key: None }
    }

    #[test]
    fn urls_follow_the_sapi_layout() {
        assert_eq!(
            sapi_url("setFreq", "k y", &[("freq", "118.500".into()), ("com", "1".into())]),
            "https://apipri.sayintentions.ai/sapi/setFreq?api_key=k+y&freq=118.500&com=1"
        );
        let a = acars_settings("LOGON");
        assert_eq!((a.provider, a.sai_logon_key.as_str()), (AcarsProvider::Sai, "LOGON"));
        assert_eq!(AcarsProvider::Sai.endpoint(), Some("https://acars.sayintentions.ai/acars/system/connect.html"));
    }

    #[test]
    fn comms_history_parses_the_fixture_and_feeds_the_auto_tuner() {
        let mut client = ScriptedClient::new(vec![body(COMMS)]);
        let list = comms_history(&mut client, &cfg(), 40).unwrap();
        assert_eq!(client.requests[0], "https://apipri.sayintentions.ai/sapi/getCommsHistory?api_key=abcdef123456&since_id=40");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].frequency_mhz(), Some(121.9));
        assert!(!list[0].acars());
        assert!(list[1].acars());
        let mut tuner = AutoTuner::default();
        assert_eq!(tuner.feed(&list), Some(SimApiCommand::Tune { slot: ComSlot::Com1Standby, hz: 118_500_000 }));
        assert_eq!(tuner.last_id(), 42);
        assert_eq!(tuner.feed(&list), None);
    }

    #[test]
    fn frequencies_are_found_in_atc_text() {
        assert_eq!(assigned_frequency("contact London Control 133.175 goodbye"), Some(133.175));
        assert_eq!(assigned_frequency("climb FL350, squawk 4521"), None);
        assert_eq!(assigned_frequency("QNH 1013.2"), None);
    }

    #[test]
    fn simapi_output_lines_become_tune_and_squawk_commands() {
        let cmds = parse_simapi_output(SIMAPI);
        assert_eq!(
            cmds,
            vec![
                SimApiCommand::Tune { slot: ComSlot::Com1Standby, hz: 118_500_000 },
                SimApiCommand::Tune { slot: ComSlot::Com1Active, hz: 121_900_000 },
                SimApiCommand::Tune { slot: ComSlot::Com2Standby, hz: 128_075_000 },
                SimApiCommand::Transponder(4521),
            ]
        );
    }

    #[test]
    fn handoff_set_freq_and_errors() {
        let mut client = ScriptedClient::new(vec![
            body(r#"{"status":"ok","accepted":true,"station":"London Control","frequency":"133.175"}"#),
            body(r#"{"status":"ok","accepted":false,"reason":"no_handoff","message":"none"}"#),
            body("{}"),
            Ok(HttpResponse { status: 401, body: r#"{"error":"bad key"}"#.into() }),
        ]);
        let h = accept_last_handoff(&mut client, &cfg()).unwrap();
        assert_eq!((h.accepted, h.station.as_str(), h.frequency_mhz), (true, "London Control", Some(133.175)));
        let h = accept_last_handoff(&mut client, &cfg()).unwrap();
        assert_eq!((h.accepted, h.reason.as_deref()), (false, Some("no_handoff")));
        set_freq(&mut client, &cfg(), 118.5, ComSlot::Com2Standby).unwrap();
        assert!(client.requests[2].ends_with("setFreq?api_key=abcdef123456&freq=118.500&com=2&mode=standby"));
        let e = set_freq(&mut client, &cfg(), 118.5, ComSlot::Com1Active).unwrap_err();
        assert!(e.contains("401") && e.contains("bad key"), "{e}");
        let mut none = ScriptedClient::new(Vec::new());
        assert!(comms_history(&mut none, &SaiConfig::default(), 0).is_err());
        assert!(none.requests.is_empty());
    }

    #[test]
    fn airspace_frequencies_and_gate() {
        let mut client = ScriptedClient::new(vec![
            body(r#"{"frequencies":[{"type":"CTR","callsign":"LONDON CONTROL","frequency":"133.175","distance_nm":12.5},{"type":"APP","callsign":"HEATHROW DIRECTOR","frequency_khz":119725,"airport":"EGLL"}]}"#),
            body(r#"{"assigned_gate_name":"511"}"#),
        ]);
        let f = airspace_frequencies(&mut client, &cfg(), 51.4, -0.4, true).unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!((f[1].frequency_mhz, f[1].airport.as_deref()), (119.725, Some("EGLL")));
        assert!(client.requests[0].contains("flight_rules=IFR"));
        assert_eq!(assign_gate(&mut client, &cfg(), "egll", "5-11").unwrap(), "511");
        assert!(client.requests[1].ends_with("gate=511&airport=EGLL"));
    }

    #[test]
    fn flight_json_is_compared_with_the_fms_plan() {
        let flight = parse_flight_json(&serde_json::from_str(FLIGHT).unwrap()).unwrap();
        assert_eq!((flight.origin.as_str(), flight.destination.as_str(), flight.assigned_gate.as_str()), ("EGLL", "KJFK", "511"));
        let same = FmsPlan { origin: "EGLL".into(), destination: "KJFK".into(), departure_runway: "RW27R".into(), sid: "CPT5J".into(), route: "CPT5J CPT DCT UL9 KENET".into() };
        assert!(plan_differences(&same, &flight).is_empty());
        let other = FmsPlan { destination: "KEWR".into(), departure_runway: "RW09L".into(), ..same.clone() };
        assert_eq!(plan_differences(&other, &flight), vec!["DESTINATION", "DEPARTURE RUNWAY"]);
        let mut client = ScriptedClient::new(vec![body(FLIGHT)]);
        assert_eq!(fetch_flight(&mut client).unwrap(), flight);
        assert_eq!(client.requests[0], FLIGHT_JSON_URL);
    }

    #[test]
    fn transport_errors_never_carry_the_key() {
        struct Failing;
        impl HttpClient for Failing {
            fn get(&mut self, url: &str, _accept: &str) -> Result<HttpResponse, String> {
                Err(format!("timeout fetching {url}"))
            }
        }
        let e = comms_history(&mut Failing, &cfg(), 0).unwrap_err();
        assert!(!e.contains("abcdef123456") && e.contains("a**********6"), "{e}");
    }
}

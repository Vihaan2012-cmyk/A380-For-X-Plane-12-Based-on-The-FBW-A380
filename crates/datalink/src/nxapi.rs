use serde_json::{json, Value};

use crate::acars::{form_urlencode, HttpClient};
use crate::message::{ComStatus, FreetextMessage, MessageDirection, MessageNetwork};
use crate::status::AtsuStatusCodes;
use crate::weather::{AtisType, Report, WeatherMessage};
use crate::weather_sources::{BeyondAtc, SayIntentions};

pub const NXAPI_URL: &str = "https://api.flybywiresim.com";
pub const NO_METAR: &str = "NO METAR AVAILABLE";
pub const NO_TAF: &str = "NO TAF AVAILABLE";
pub const NO_ATIS: &str = "D-ATIS NOT AVAILABLE";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AircraftStatus {
    pub latitude: f64,
    pub longitude: f64,
    pub true_altitude: f64,
    pub heading: f64,
    pub origin: String,
    pub destination: String,
    pub freetext_enabled: bool,
    pub aircraft_type: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NxApiSettings {
    pub base_url: String,
    pub online_features_enabled: bool,
    pub metar_source: String,
    pub taf_source: String,
    pub atis_source: String,
    pub sai_api_key: String,
}

impl Default for NxApiSettings {
    fn default() -> Self {
        NxApiSettings {
            base_url: NXAPI_URL.into(),
            online_features_enabled: false,
            metar_source: "vatsim".into(),
            taf_source: "aviationweather".into(),
            atis_source: "faa".into(),
            sai_api_key: String::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct NxApiConnector {
    flight_number: String,
    connected: bool,
    access_token: String,
}

fn source_url(base: &str, kind: &str, icao: &str, source: &str) -> String {
    let mut url = format!("{}/{}/{}", base.trim_end_matches('/'), kind, icao);
    if !source.is_empty() {
        url.push_str("?source=");
        url.push_str(&form_urlencode(source));
    }
    url
}

fn get_json(client: &mut dyn HttpClient, url: &str) -> Option<Value> {
    let reply = client.get(url, "application/json").ok()?;
    if !reply.ok() {
        return None;
    }
    serde_json::from_str(&reply.body).ok()
}

fn text_field(data: &Value, key: &str) -> Option<String> {
    data.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

pub fn pick_atis(data: &Value, kind: AtisType) -> Option<String> {
    let has = |k: &str| data.get(k).is_some();
    let field = match kind {
        AtisType::Arrival => if has("arr") { "arr" } else { "combined" },
        AtisType::Departure => if has("dep") { "dep" } else { "combined" },
        AtisType::Enroute => {
            if has("combined") {
                "combined"
            } else if has("arr") {
                "arr"
            } else {
                return None;
            }
        }
    };
    text_field(data, field)
}

impl NxApiConnector {
    pub fn new() -> NxApiConnector {
        NxApiConnector::default()
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn flight_number(&self) -> &str {
        &self.flight_number
    }

    fn token(&self) -> String {
        format!("Bearer {}", self.access_token)
    }

    fn status_body(&self, status: &AircraftStatus) -> String {
        json!({
            "location": { "x": status.longitude, "y": status.latitude },
            "trueAltitude": status.true_altitude,
            "heading": status.heading,
            "origin": status.origin,
            "destination": status.destination,
            "freetextEnabled": status.freetext_enabled,
            "flight": self.flight_number,
            "aircraftType": status.aircraft_type,
        })
        .to_string()
    }

    fn url(settings: &NxApiSettings, path: &str) -> String {
        format!("{}{}", settings.base_url.trim_end_matches('/'), path)
    }

    pub fn connect(
        &mut self,
        client: &mut dyn HttpClient,
        settings: &NxApiSettings,
        flight_no: &str,
        status: &AircraftStatus,
    ) -> AtsuStatusCodes {
        if !settings.online_features_enabled {
            return AtsuStatusCodes::TelexDisabled;
        }
        self.disconnect(client, settings);
        self.flight_number = flight_no.to_string();
        let body = self.status_body(status);
        let reply = client.post(&Self::url(settings, "/txcxn"), "application/json", None, &body);
        match reply {
            Ok(r) if r.ok() => {
                let token = serde_json::from_str::<Value>(&r.body)
                    .ok()
                    .and_then(|v| v.get("accessToken").and_then(Value::as_str).map(str::to_string))
                    .unwrap_or_default();
                if token.is_empty() {
                    AtsuStatusCodes::NoTelexConnection
                } else {
                    self.access_token = token;
                    self.connected = true;
                    AtsuStatusCodes::Ok
                }
            }
            _ => AtsuStatusCodes::CallsignInUse,
        }
    }

    pub fn disconnect(&mut self, client: &mut dyn HttpClient, settings: &NxApiSettings) -> AtsuStatusCodes {
        if !settings.online_features_enabled {
            return AtsuStatusCodes::TelexDisabled;
        }
        if !self.connected {
            return AtsuStatusCodes::NoTelexConnection;
        }
        match client.delete(&Self::url(settings, "/txcxn"), Some(&self.token())) {
            Ok(r) if r.ok() => {
                self.access_token.clear();
                self.connected = false;
                self.flight_number.clear();
                AtsuStatusCodes::Ok
            }
            _ => AtsuStatusCodes::ProxyError,
        }
    }

    pub fn send_telex_message(
        &mut self,
        client: &mut dyn HttpClient,
        settings: &NxApiSettings,
        message: &mut FreetextMessage,
    ) -> AtsuStatusCodes {
        if !self.connected {
            return AtsuStatusCodes::NoTelexConnection;
        }
        let content = message.header.message.replacen('\n', ";", 1);
        let body = json!({ "to": message.header.station, "message": content }).to_string();
        match client.post(&Self::url(settings, "/txmsg"), "application/json", Some(&self.token()), &body) {
            Ok(r) if r.ok() => {
                message.header.com_status = ComStatus::Sent;
                AtsuStatusCodes::Ok
            }
            _ => {
                message.header.com_status = ComStatus::Failed;
                AtsuStatusCodes::ComFailed
            }
        }
    }

    pub fn poll(
        &mut self,
        client: &mut dyn HttpClient,
        settings: &NxApiSettings,
        status: &AircraftStatus,
    ) -> (AtsuStatusCodes, Vec<FreetextMessage>) {
        let mut out = Vec::new();
        if !self.connected {
            return (AtsuStatusCodes::Ok, out);
        }
        let body = self.status_body(status);
        match client.put(&Self::url(settings, "/txcxn"), "application/json", Some(&self.token()), &body) {
            Ok(r) if r.ok() => {}
            _ => return (AtsuStatusCodes::ComFailed, out),
        }
        let data = match client.get_authorized(&Self::url(settings, "/txmsg"), &self.token()) {
            Ok(r) if r.ok() => serde_json::from_str::<Value>(&r.body).ok(),
            _ => None,
        };
        let Some(Value::Array(items)) = data else {
            return (AtsuStatusCodes::ComFailed, out);
        };
        for item in items {
            let mut message = FreetextMessage::new();
            message.header.network = MessageNetwork::Fbw;
            message.header.direction = MessageDirection::Uplink;
            message.header.station = item
                .pointer("/from/flight")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            message.header.message = item
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .replacen(';', " ", 1);
            out.push(message);
        }
        (AtsuStatusCodes::Ok, out)
    }

    pub fn poll_interval_ms(unit: f64) -> f64 {
        unit * 30_000.0 + 45_000.0
    }
}

pub fn receive_metar(
    client: &mut dyn HttpClient,
    settings: &NxApiSettings,
    icao: &str,
    message: &mut WeatherMessage,
) -> AtsuStatusCodes {
    match settings.metar_source.as_str() {
        "BEYONDATC" => return BeyondAtc { client }.receive_metar(icao, message),
        "SAI" => {
            return SayIntentions { client, api_key: settings.sai_api_key.clone() }.receive_metar(icao, message)
        }
        _ => {}
    }
    let url = source_url(&settings.base_url, "metar", icao, &settings.metar_source);
    let report = get_json(client, &url)
        .and_then(|v| text_field(&v, "metar"))
        .unwrap_or_else(|| NO_METAR.to_string());
    message.reports.push(Report { airport: icao.to_string(), report });
    AtsuStatusCodes::Ok
}

pub fn receive_taf(
    client: &mut dyn HttpClient,
    settings: &NxApiSettings,
    icao: &str,
    message: &mut WeatherMessage,
) -> AtsuStatusCodes {
    let url = source_url(&settings.base_url, "taf", icao, &settings.taf_source);
    let report = get_json(client, &url)
        .and_then(|v| text_field(&v, "taf"))
        .unwrap_or_else(|| NO_TAF.to_string());
    message.reports.push(Report { airport: icao.to_string(), report });
    AtsuStatusCodes::Ok
}

pub fn receive_atis(
    client: &mut dyn HttpClient,
    settings: &NxApiSettings,
    icao: &str,
    kind: AtisType,
    message: &mut WeatherMessage,
) -> AtsuStatusCodes {
    match settings.atis_source.as_str() {
        "BEYONDATC" => return BeyondAtc { client }.receive_atis(icao, message),
        "SAI" => {
            return SayIntentions { client, api_key: settings.sai_api_key.clone() }.receive_atis(icao, message)
        }
        _ => {}
    }
    let url = source_url(&settings.base_url, "atis", icao, &settings.atis_source);
    let report = get_json(client, &url)
        .and_then(|v| pick_atis(&v, kind))
        .unwrap_or_else(|| NO_ATIS.to_string());
    message.reports.push(Report { airport: icao.to_string(), report });
    AtsuStatusCodes::Ok
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::acars::HttpResponse;
    use crate::message::MessageType;
    use std::collections::VecDeque;

    #[derive(Default)]
    pub struct Recorder {
        pub calls: Vec<String>,
        pub replies: VecDeque<Result<HttpResponse, String>>,
    }

    impl Recorder {
        pub fn new(replies: Vec<Result<HttpResponse, String>>) -> Recorder {
            Recorder { calls: Vec::new(), replies: replies.into() }
        }

        fn next(&mut self, call: String) -> Result<HttpResponse, String> {
            self.calls.push(call);
            self.replies.pop_front().unwrap_or_else(|| Err("no reply".into()))
        }
    }

    impl HttpClient for Recorder {
        fn get(&mut self, url: &str, _accept: &str) -> Result<HttpResponse, String> {
            self.next(format!("GET {url}"))
        }
        fn post(&mut self, url: &str, _ct: &str, auth: Option<&str>, body: &str) -> Result<HttpResponse, String> {
            self.next(format!("POST {url} {} {body}", auth.unwrap_or("-")))
        }
        fn put(&mut self, url: &str, _ct: &str, auth: Option<&str>, body: &str) -> Result<HttpResponse, String> {
            self.next(format!("PUT {url} {} {body}", auth.unwrap_or("-")))
        }
        fn delete(&mut self, url: &str, auth: Option<&str>) -> Result<HttpResponse, String> {
            self.next(format!("DELETE {url} {}", auth.unwrap_or("-")))
        }
        fn get_authorized(&mut self, url: &str, auth: &str) -> Result<HttpResponse, String> {
            self.next(format!("GETA {url} {auth}"))
        }
    }

    pub fn ok(text: &str) -> Result<HttpResponse, String> {
        Ok(HttpResponse { status: 200, body: text.into() })
    }

    fn enabled() -> NxApiSettings {
        NxApiSettings { online_features_enabled: true, ..NxApiSettings::default() }
    }

    #[test]
    fn telex_disabled_short_circuits() {
        let mut c = Recorder::new(vec![]);
        let mut nx = NxApiConnector::new();
        let s = NxApiSettings::default();
        assert_eq!(nx.connect(&mut c, &s, "TST123", &AircraftStatus::default()), AtsuStatusCodes::TelexDisabled);
        assert!(c.calls.is_empty());
    }

    #[test]
    fn connect_send_poll_disconnect() {
        let mut c = Recorder::new(vec![
            ok(r#"{"accessToken":"tok1","flight":"TST123"}"#),
            ok("{}"),
            ok("{}"),
            ok(r#"[{"from":{"flight":"OPS1"},"message":"GATE CHANGE;B12;STAND 4"}]"#),
            ok(""),
        ]);
        let mut nx = NxApiConnector::new();
        let s = enabled();
        let st = AircraftStatus { latitude: 1.0, longitude: 2.0, origin: "EGLL".into(), ..Default::default() };
        assert_eq!(nx.connect(&mut c, &s, "TST123", &st), AtsuStatusCodes::Ok);
        assert!(c.calls[0].starts_with("POST https://api.flybywiresim.com/txcxn - "));
        assert!(c.calls[0].contains(r#""location":{"x":2.0,"y":1.0}"#));
        assert!(c.calls[0].contains(r#""flight":"TST123""#));

        let mut msg = FreetextMessage::new();
        msg.header.station = "OPS1".into();
        msg.header.message = "LINE1\nLINE2\nLINE3".into();
        assert_eq!(nx.send_telex_message(&mut c, &s, &mut msg), AtsuStatusCodes::Ok);
        assert_eq!(msg.header.com_status, ComStatus::Sent);
        assert!(c.calls[1].contains("Bearer tok1"));
        assert!(c.calls[1].contains(r#""message":"LINE1;LINE2\nLINE3""#));

        let (code, msgs) = nx.poll(&mut c, &s, &st);
        assert_eq!(code, AtsuStatusCodes::Ok);
        assert!(c.calls[2].starts_with("PUT https://api.flybywiresim.com/txcxn Bearer tok1"));
        assert_eq!(c.calls[3], "GETA https://api.flybywiresim.com/txmsg Bearer tok1");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].header.station, "OPS1");
        assert_eq!(msgs[0].header.message, "GATE CHANGE B12;STAND 4");
        assert_eq!(msgs[0].header.network, MessageNetwork::Fbw);

        assert_eq!(nx.disconnect(&mut c, &s), AtsuStatusCodes::Ok);
        assert_eq!(c.calls[4], "DELETE https://api.flybywiresim.com/txcxn Bearer tok1");
        assert!(!nx.is_connected());
    }

    #[test]
    fn connect_failures() {
        let mut c = Recorder::new(vec![ok(r#"{"accessToken":""}"#), Err("409".into())]);
        let mut nx = NxApiConnector::new();
        let s = enabled();
        assert_eq!(nx.connect(&mut c, &s, "A", &AircraftStatus::default()), AtsuStatusCodes::NoTelexConnection);
        assert_eq!(nx.connect(&mut c, &s, "A", &AircraftStatus::default()), AtsuStatusCodes::CallsignInUse);
    }

    #[test]
    fn send_fails_without_connection_or_on_error() {
        let mut c = Recorder::new(vec![ok(r#"{"accessToken":"t"}"#), Err("500".into()), Err("x".into())]);
        let mut nx = NxApiConnector::new();
        let s = enabled();
        let mut msg = FreetextMessage::new();
        assert_eq!(nx.send_telex_message(&mut c, &s, &mut msg), AtsuStatusCodes::NoTelexConnection);
        nx.connect(&mut c, &s, "A", &AircraftStatus::default());
        assert_eq!(nx.send_telex_message(&mut c, &s, &mut msg), AtsuStatusCodes::ComFailed);
        assert_eq!(msg.header.com_status, ComStatus::Failed);
        assert_eq!(nx.poll(&mut c, &s, &AircraftStatus::default()).0, AtsuStatusCodes::ComFailed);
    }

    #[test]
    fn weather_requests_fill_reports() {
        let mut c = Recorder::new(vec![
            ok(r#"{"icao":"EGLL","metar":"EGLL 221250Z 24012KT 9999 FEW030 15/08 Q1015"}"#),
            Err("404".into()),
            ok(r#"{"icao":"EGLL","taf":""}"#),
            ok(r#"{"icao":"KJFK","combined":"JFK ATIS INFO A"}"#),
            ok(r#"{"icao":"KJFK","arr":"ARR INFO B","dep":"DEP INFO C"}"#),
        ]);
        let s = NxApiSettings::default();
        let mut m = WeatherMessage::new(MessageType::Metar, "");
        receive_metar(&mut c, &s, "EGLL", &mut m);
        receive_metar(&mut c, &s, "LFPG", &mut m);
        assert_eq!(c.calls[0], "GET https://api.flybywiresim.com/metar/EGLL?source=vatsim");
        assert!(m.reports[0].report.starts_with("EGLL 221250Z"));
        assert_eq!(m.reports[1].report, NO_METAR);

        let mut t = WeatherMessage::new(MessageType::Taf, "");
        receive_taf(&mut c, &s, "EGLL", &mut t);
        assert_eq!(t.reports[0].report, NO_TAF);

        let mut a = WeatherMessage::new(MessageType::Atis, "");
        receive_atis(&mut c, &s, "KJFK", AtisType::Departure, &mut a);
        receive_atis(&mut c, &s, "KJFK", AtisType::Enroute, &mut a);
        assert_eq!(c.calls[3], "GET https://api.flybywiresim.com/atis/KJFK?source=faa");
        assert_eq!(a.reports[0].report, "JFK ATIS INFO A");
        assert_eq!(a.reports[1].report, "ARR INFO B");
    }
}

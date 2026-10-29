use crate::elements::{uplink, ExpectedResponse, MessageElement, ElementDef};
use crate::catalogue::UPLINK;
use crate::fans::FansMode;
use crate::message::{
    AnyMessage, ComStatus, CpdlcMessage, FreetextMessage, MessageDirection, MessageNetwork, SerializationFormat,
};
use crate::elements::parse_transmission_id;
use crate::status::AtsuStatusCodes;

pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

impl HttpResponse {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

pub trait HttpClient {
    fn get(&mut self, url: &str, accept: &str) -> Result<HttpResponse, String>;

    fn post(&mut self, _url: &str, _content_type: &str, _authorization: Option<&str>, _body: &str) -> Result<HttpResponse, String> {
        Err("post not supported".into())
    }

    fn put(&mut self, _url: &str, _content_type: &str, _authorization: Option<&str>, _body: &str) -> Result<HttpResponse, String> {
        Err("put not supported".into())
    }

    fn delete(&mut self, _url: &str, _authorization: Option<&str>) -> Result<HttpResponse, String> {
        Err("delete not supported".into())
    }

    fn get_authorized(&mut self, _url: &str, _authorization: &str) -> Result<HttpResponse, String> {
        Err("authorized get not supported".into())
    }

    fn get_with_header(&mut self, url: &str, _name: &str, _value: &str) -> Result<HttpResponse, String> {
        self.get(url, "application/json")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcarsProvider {
    None,
    Hoppie,
    Batc,
    Sai,
}

impl AcarsProvider {
    pub fn parse(setting: &str) -> AcarsProvider {
        match setting {
            "HOPPIE" => AcarsProvider::Hoppie,
            "BATC" => AcarsProvider::Batc,
            "SAI" => AcarsProvider::Sai,
            _ => AcarsProvider::None,
        }
    }

    pub fn endpoint(self) -> Option<&'static str> {
        match self {
            AcarsProvider::Hoppie => Some("https://www.hoppie.nl/acars/system/connect.html"),
            AcarsProvider::Batc => Some("http://localhost:57698/acars/system/connect.html"),
            AcarsProvider::Sai => Some("https://acars.sayintentions.ai/acars/system/connect.html"),
            AcarsProvider::None => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcarsSettings {
    pub provider: AcarsProvider,
    pub hoppie_user_id: String,
    pub sai_logon_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CpdlcMessageDto {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub packet: Option<String>,
}

pub fn form_urlencode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        match b {
            b'*' | b'-' | b'.' | b'_' | b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

pub fn build_request_url(settings: &AcarsSettings, dto: &CpdlcMessageDto) -> Result<String, String> {
    let base = settings
        .provider
        .endpoint()
        .ok_or_else(|| "error undefined".to_string())?;
    let mut params: Vec<(&str, &str)> = Vec::new();

    match settings.provider {
        AcarsProvider::Sai => {
            if settings.sai_logon_key.is_empty() {
                return Err("error Missing SAI logon key".to_string());
            }
            params.push(("logon", &settings.sai_logon_key));
        }
        AcarsProvider::Hoppie => {
            if settings.hoppie_user_id.is_empty() {
                return Err("error Missing Hoppie user ID".to_string());
            }
            params.push(("logon", &settings.hoppie_user_id));
        }
        _ => {}
    }

    params.push(("from", &dto.from));
    params.push(("to", &dto.to));
    params.push(("type", &dto.kind));
    if let Some(packet) = &dto.packet {
        if !packet.is_empty() {
            params.push(("packet", packet));
        }
    }

    let query: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{}={}", form_urlencode(k), form_urlencode(v)))
        .collect();
    Ok(format!("{}?{}", base, query.join("&")))
}

pub fn get_data(client: &mut dyn HttpClient, settings: &AcarsSettings, dto: &CpdlcMessageDto) -> Result<String, String> {
    let url = build_request_url(settings, dto)?;
    match client.get(&url, "text/plain") {
        Ok(response) => {
            if response.body.is_empty() {
                Err("error Empty response".to_string())
            } else {
                Ok(response.body)
            }
        }
        Err(e) => Err(format!("error {e}")),
    }
}

fn levenshtein_text(template: &str, message: &str) -> usize {
    let t: Vec<char> = template.chars().collect();
    let m: Vec<char> = message.chars().collect();
    let mut previous: Vec<usize> = (0..=t.len()).collect();
    for j in 1..=m.len() {
        let mut current = vec![j; t.len() + 1];
        for i in 1..=t.len() {
            let indicator = usize::from(t[i - 1] != m[j - 1]);
            current[i] = (current[i - 1] + 1)
                .min(previous[i] + 1)
                .min(previous[i - 1] + indicator);
        }
        previous = current;
    }
    previous[t.len()]
}

const NO_MATCH: usize = 100000;

fn levenshtein_distance(template: &str, message: &str, element: &MessageElement, fans: FansMode) -> usize {
    let mut elements: Vec<String> = message.replace('\n', " ").split(' ').map(String::from).collect();
    let mut content = element.content.clone();
    for entry in content.iter_mut() {
        if !entry.validate_and_replace(&mut elements, fans) {
            return NO_MATCH;
        }
    }
    levenshtein_text(template, &elements.join(" "))
}

/// Port of AcarsConnector.cpdlcMessageClassification: finds the uplink element whose
/// template is closest to the received text and fills its content slots.
pub fn classify_uplink(message: &str, fans: FansMode) -> Option<MessageElement> {
    let cleared = message.replace('@', "").replace('_', " ");
    let mut scores: Vec<(usize, &'static ElementDef)> = Vec::new();
    let mut min_score = NO_MATCH;

    for def in UPLINK {
        if fans == FansMode::FansNone || def.supports(fans) {
            let element = def.to_element();
            let mut min_distance = NO_MATCH;
            for template in def.templates {
                let distance = levenshtein_distance(template, &cleared, &element, fans);
                if min_distance > distance {
                    min_distance = distance;
                }
            }
            scores.push((min_distance, def));
            if min_score > min_distance {
                min_score = min_distance;
            }
        }
    }

    let mut matches: Vec<&'static ElementDef> = scores
        .iter()
        .filter(|(score, _)| *score == min_score)
        .map(|(_, def)| *def)
        .collect();
    if matches.is_empty() {
        return None;
    }

    if matches.len() > 1 && min_score != 0 {
        let non_empty: Vec<_> = matches.iter().copied().filter(|d| !d.content.is_empty()).collect();
        if !non_empty.is_empty() && matches.len() != non_empty.len() {
            matches = non_empty;
        }
    }

    if matches.len() > 1 {
        let non_freetext: Vec<_> = matches
            .iter()
            .copied()
            .filter(|d| d.type_id != "UM169" && d.type_id != "UM183")
            .collect();
        if !non_freetext.is_empty() && matches.len() != non_freetext.len() {
            matches = non_freetext;
        }
    }

    if matches.len() > 1 && fans != FansMode::FansNone {
        let valid: Vec<_> = matches.iter().copied().filter(|d| d.supports(fans)).collect();
        if !valid.is_empty() && matches.len() != valid.len() {
            matches = valid;
        }
    }

    let mut result = matches[0].to_element();
    let mut elements: Vec<String> = if result.is_freetext_element() {
        message.replace('_', " ").split(' ').map(String::from).collect()
    } else {
        cleared.split(' ').map(String::from).collect()
    };
    for entry in result.content.iter_mut() {
        entry.validate_and_replace(&mut elements, fans);
    }
    Some(result)
}

/// Port of JavaScript's `text.split(/({[\s\S\n]*?})/gm)`.
pub fn split_braces(text: &str) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut last = 0;
    let mut search_from = 0;
    while let Some(open_rel) = text[search_from..].find('{') {
        let open = search_from + open_rel;
        match text[open..].find('}') {
            Some(close_rel) => {
                let close = open + close_rel;
                pieces.push(text[last..open].to_string());
                pieces.push(text[open..=close].to_string());
                last = close + 1;
                search_from = last;
            }
            None => break,
        }
    }
    pieces.push(text[last..].to_string());
    pieces
}

fn parse_poll_reply(text: &str, fans: FansMode) -> Result<Vec<AnyMessage>, ()> {
    let mut result = Vec::new();
    let trimmed = text.trim();
    let messages: Vec<String> = split_braces(trimmed)
        .into_iter()
        .filter(|e| !matches!(e.as_str(), "ok" | "ok " | "} " | "}\n" | "}" | ""))
        .collect();

    for element in messages {
        let body = element.get(1..).ok_or(())?;
        let entries = split_braces(body);
        let head = entries.first().ok_or(())?;
        let payload = entries.get(1).ok_or(())?;
        let metadata: Vec<&str> = head.split(' ').collect();
        let sender = metadata.first().ok_or(())?.to_uppercase();
        let kind = metadata.get(1).ok_or(())?.to_lowercase();
        let content = payload.replacen('{', "", 1).replacen('}', "", 1).to_uppercase();

        match kind.as_str() {
            "telex" => {
                let mut freetext = FreetextMessage::new();
                freetext.header.network = MessageNetwork::Hoppie;
                freetext.header.station = sender;
                freetext.header.direction = MessageDirection::Uplink;
                freetext.header.com_status = ComStatus::Received;
                freetext.header.message = content;
                result.push(AnyMessage::Freetext(freetext));
            }
            "cpdlc" => {
                let mut cpdlc = CpdlcMessage::new();
                cpdlc.header.station = sender;
                cpdlc.header.direction = MessageDirection::Uplink;
                cpdlc.header.com_status = ComStatus::Received;

                let parts: Vec<&str> = content.split('/').collect();
                cpdlc.current_transmission_id = parse_transmission_id(parts.get(2).ok_or(())?);
                if *parts.get(3).ok_or(())? != "" {
                    cpdlc.previous_transmission_id = parse_transmission_id(parts[3]);
                }
                cpdlc.header.message = parts.get(5).ok_or(())?.to_string();
                let mut classified = classify_uplink(&cpdlc.header.message, fans).ok_or(())?;
                let wire_response = parts.get(4).ok_or(())?;
                if let Some(code) = ExpectedResponse::from_code(wire_response) {
                    classified.expected_response = code;
                }
                cpdlc.content.push(classified);
                result.push(AnyMessage::Cpdlc(cpdlc));
            }
            _ => {}
        }
    }
    Ok(result)
}

pub const MAX_CONNECTION_ATTEMPTS: u32 = 60;
pub const RETRY_DELAY_MS: u64 = 5_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivationOutcome {
    Connected,
    InvalidLogon,
    Retry,
    GaveUp,
}

pub struct AcarsConnector {
    pub flight_number: String,
    pub fans_mode: FansMode,
    pub acars_active: bool,
    connection_attempts: u32,
    connected: bool,
    activation_requested: bool,
}

impl Default for AcarsConnector {
    fn default() -> Self {
        AcarsConnector::new()
    }
}

impl AcarsConnector {
    pub fn new() -> AcarsConnector {
        AcarsConnector {
            flight_number: String::new(),
            fans_mode: FansMode::FansNone,
            acars_active: false,
            connection_attempts: 0,
            connected: false,
            activation_requested: false,
        }
    }

    fn stop_activation(&mut self) {
        self.activation_requested = false;
        self.connection_attempts = 0;
        self.connected = false;
    }

    pub fn activate_acars(&mut self, settings: &AcarsSettings) {
        self.stop_activation();
        self.acars_active = false;
        if settings.provider == AcarsProvider::None {
            return;
        }
        self.activation_requested = true;
    }

    pub fn deactivate_acars(&mut self) {
        self.stop_activation();
        self.acars_active = false;
    }

    pub fn activation_pending(&self) -> bool {
        self.activation_requested && !self.connected
    }

    /// One iteration of the activation loop; the caller waits RETRY_DELAY_MS between Retry results.
    pub fn attempt_activation(&mut self, client: &mut dyn HttpClient, settings: &AcarsSettings) -> ActivationOutcome {
        if !self.activation_pending() {
            return ActivationOutcome::Connected;
        }
        self.connection_attempts += 1;
        let dto = CpdlcMessageDto {
            from: "FBWA32NX".into(),
            to: "SERVER".into(),
            kind: "ping".into(),
            packet: Some(String::new()),
        };
        match get_data(client, settings, &dto) {
            Ok(response) => {
                if response != "error {invalid logon code}" {
                    self.acars_active = true;
                    self.connected = true;
                    return ActivationOutcome::Connected;
                }
                self.activation_requested = false;
                return ActivationOutcome::InvalidLogon;
            }
            Err(_) => {}
        }
        if self.connection_attempts >= MAX_CONNECTION_ATTEMPTS {
            self.stop_activation();
            return ActivationOutcome::GaveUp;
        }
        ActivationOutcome::Retry
    }

    pub fn connect(
        &mut self,
        client: &mut dyn HttpClient,
        settings: &AcarsSettings,
        flight_no: &str,
    ) -> AtsuStatusCodes {
        if !self.acars_active {
            self.flight_number = flight_no.to_string();
            return AtsuStatusCodes::NoAcarsConnection;
        }
        let code = self.is_callsign_in_use(client, settings, flight_no);
        if code == AtsuStatusCodes::Ok {
            self.flight_number = flight_no.to_string();
            self.poll(client, settings);
        }
        code
    }

    pub fn disconnect(&mut self) -> AtsuStatusCodes {
        self.flight_number.clear();
        self.activation_requested = false;
        self.connected = false;
        AtsuStatusCodes::Ok
    }

    fn ping(&self, client: &mut dyn HttpClient, settings: &AcarsSettings, from: &str, packet: &str) -> String {
        let dto = CpdlcMessageDto {
            from: from.to_string(),
            to: "SERVER".into(),
            kind: "ping".into(),
            packet: Some(packet.to_string()),
        };
        get_data(client, settings, &dto).unwrap_or_else(|e| e)
    }

    pub fn is_callsign_in_use(
        &self,
        client: &mut dyn HttpClient,
        settings: &AcarsSettings,
        station: &str,
    ) -> AtsuStatusCodes {
        if !self.acars_active {
            return AtsuStatusCodes::NoAcarsConnection;
        }
        let text = self.ping(client, settings, station, station);
        if text == "error {callsign already in use}" {
            return AtsuStatusCodes::CallsignInUse;
        }
        if text.contains("error") {
            return AtsuStatusCodes::ProxyError;
        }
        if !text.starts_with("ok") {
            return AtsuStatusCodes::ComFailed;
        }
        AtsuStatusCodes::Ok
    }

    pub fn is_station_available(
        &self,
        client: &mut dyn HttpClient,
        settings: &AcarsSettings,
        station: &str,
    ) -> AtsuStatusCodes {
        if !self.acars_active || self.flight_number.is_empty() {
            return AtsuStatusCodes::NoAcarsConnection;
        }
        if station == self.flight_number {
            return AtsuStatusCodes::OwnCallsign;
        }
        let text = self.ping(client, settings, &self.flight_number.clone(), station);
        if text == "error {callsign already in use}" {
            return AtsuStatusCodes::CallsignInUse;
        }
        if text.contains("error") {
            return AtsuStatusCodes::ProxyError;
        }
        if !text.starts_with("ok") {
            return AtsuStatusCodes::ComFailed;
        }
        if !text.contains(station) {
            return AtsuStatusCodes::NoAtc;
        }
        AtsuStatusCodes::Ok
    }

    fn send_message(
        &self,
        client: &mut dyn HttpClient,
        settings: &AcarsSettings,
        station: &str,
        packet: String,
        kind: &str,
    ) -> AtsuStatusCodes {
        if !self.acars_active || self.flight_number.is_empty() {
            return AtsuStatusCodes::NoAcarsConnection;
        }
        let dto = CpdlcMessageDto {
            from: self.flight_number.clone(),
            to: station.to_string(),
            kind: kind.to_string(),
            packet: Some(packet),
        };
        let text = match get_data(client, settings, &dto) {
            Ok(t) => t,
            Err(_) => return AtsuStatusCodes::ProxyError,
        };
        if text == "error {callsign already in use}" {
            return AtsuStatusCodes::CallsignInUse;
        }
        if text != "ok" {
            return AtsuStatusCodes::ComFailed;
        }
        AtsuStatusCodes::Ok
    }

    pub fn send_telex_message(
        &self,
        client: &mut dyn HttpClient,
        settings: &AcarsSettings,
        message: &FreetextMessage,
        force: bool,
    ) -> AtsuStatusCodes {
        if !self.flight_number.is_empty() && (force || self.acars_active) {
            return self.send_message(
                client,
                settings,
                &message.header.station,
                message.render(SerializationFormat::Network),
                "telex",
            );
        }
        AtsuStatusCodes::NoAcarsConnection
    }

    pub fn send_dcl_message(
        &self,
        client: &mut dyn HttpClient,
        settings: &AcarsSettings,
        message: &crate::message::DclMessage,
        force: bool,
    ) -> AtsuStatusCodes {
        if !self.flight_number.is_empty() && (force || self.acars_active) {
            return self.send_message(
                client,
                settings,
                &message.cpdlc.header.station,
                message.render(SerializationFormat::Network),
                "telex",
            );
        }
        AtsuStatusCodes::NoAcarsConnection
    }

    pub fn send_cpdlc_message(
        &self,
        client: &mut dyn HttpClient,
        settings: &AcarsSettings,
        message: &CpdlcMessage,
        force: bool,
    ) -> AtsuStatusCodes {
        if !self.flight_number.is_empty() && (force || self.acars_active) {
            return self.send_message(
                client,
                settings,
                &message.header.station,
                message.render(SerializationFormat::Network),
                "cpdlc",
            );
        }
        AtsuStatusCodes::NoAcarsConnection
    }

    pub fn poll(&mut self, client: &mut dyn HttpClient, settings: &AcarsSettings) -> (AtsuStatusCodes, Vec<AnyMessage>) {
        if !self.acars_active || self.flight_number.is_empty() {
            return (AtsuStatusCodes::NoAcarsConnection, Vec::new());
        }
        let dto = CpdlcMessageDto {
            from: self.flight_number.clone(),
            to: self.flight_number.clone(),
            kind: "poll".into(),
            packet: None,
        };
        let text = match get_data(client, settings, &dto) {
            Ok(t) => t,
            Err(_) => return (AtsuStatusCodes::ProxyError, Vec::new()),
        };
        if text == "error {callsign already in use}" {
            return (AtsuStatusCodes::CallsignInUse, Vec::new());
        }
        if !text.starts_with("ok") {
            return (AtsuStatusCodes::ComFailed, Vec::new());
        }
        match parse_poll_reply(&text, self.fans_mode) {
            Ok(messages) => (AtsuStatusCodes::Ok, messages),
            Err(()) => (AtsuStatusCodes::NoAcarsConnection, Vec::new()),
        }
    }

    /// `unit` is a uniform random number in [0, 1).
    pub fn poll_interval_ms(unit: f64) -> f64 {
        unit * 30_000.0 + 45_000.0
    }
}

pub fn lookup_uplink(type_id: &str) -> Option<&'static ElementDef> {
    uplink(type_id)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::VecDeque;

    pub struct ScriptedClient {
        pub requests: Vec<String>,
        pub replies: VecDeque<Result<HttpResponse, String>>,
    }

    impl ScriptedClient {
        pub fn new(replies: Vec<Result<HttpResponse, String>>) -> ScriptedClient {
            ScriptedClient {
                requests: Vec::new(),
                replies: replies.into(),
            }
        }
    }

    impl HttpClient for ScriptedClient {
        fn get(&mut self, url: &str, _accept: &str) -> Result<HttpResponse, String> {
            self.requests.push(url.to_string());
            self.replies.pop_front().unwrap_or_else(|| Err("no reply".into()))
        }
    }

    pub fn body(text: &str) -> Result<HttpResponse, String> {
        Ok(HttpResponse {
            status: 200,
            body: text.to_string(),
        })
    }

    fn hoppie() -> AcarsSettings {
        AcarsSettings {
            provider: AcarsProvider::Hoppie,
            hoppie_user_id: "ABC123".into(),
            sai_logon_key: String::new(),
        }
    }

    #[test]
    fn url_matches_the_connect_html_layout() {
        let dto = CpdlcMessageDto {
            from: "FBW001".into(),
            to: "EDMM".into(),
            kind: "cpdlc".into(),
            packet: Some("/data2/1//Y/REQUEST FL350".into()),
        };
        assert_eq!(
            build_request_url(&hoppie(), &dto).unwrap(),
            "https://www.hoppie.nl/acars/system/connect.html?logon=ABC123&from=FBW001&to=EDMM&type=cpdlc&packet=%2Fdata2%2F1%2F%2FY%2FREQUEST+FL350"
        );
    }

    #[test]
    fn missing_logon_is_an_error() {
        let mut settings = hoppie();
        settings.hoppie_user_id.clear();
        let dto = CpdlcMessageDto {
            from: "A".into(),
            to: "B".into(),
            kind: "ping".into(),
            packet: None,
        };
        assert_eq!(build_request_url(&settings, &dto).unwrap_err(), "error Missing Hoppie user ID");
        settings.provider = AcarsProvider::Batc;
        assert_eq!(
            build_request_url(&settings, &dto).unwrap(),
            "http://localhost:57698/acars/system/connect.html?from=A&to=B&type=ping"
        );
    }

    #[test]
    fn split_braces_behaves_like_js_split() {
        assert_eq!(
            split_braces("ok {A telex {X}} {B telex {Y}}"),
            vec!["ok ", "{A telex {X}", "} ", "{B telex {Y}", "}"]
        );
        assert_eq!(split_braces("plain"), vec!["plain"]);
    }

    #[test]
    fn activation_pings_and_sets_the_active_flag() {
        let mut conn = AcarsConnector::new();
        let settings = hoppie();
        conn.activate_acars(&settings);
        let mut client = ScriptedClient::new(vec![Err("offline".into()), body("ok")]);
        assert_eq!(conn.attempt_activation(&mut client, &settings), ActivationOutcome::Retry);
        assert!(!conn.acars_active);
        assert_eq!(conn.attempt_activation(&mut client, &settings), ActivationOutcome::Connected);
        assert!(conn.acars_active);
        assert!(client.requests[0].contains("from=FBWA32NX&to=SERVER&type=ping"));
    }

    #[test]
    fn activation_stops_on_invalid_logon_and_gives_up_after_sixty() {
        let settings = hoppie();
        let mut conn = AcarsConnector::new();
        conn.activate_acars(&settings);
        let mut client = ScriptedClient::new(vec![body("error {invalid logon code}")]);
        assert_eq!(conn.attempt_activation(&mut client, &settings), ActivationOutcome::InvalidLogon);
        assert!(!conn.activation_pending());

        conn.activate_acars(&settings);
        let mut client = ScriptedClient::new(vec![]);
        let mut last = ActivationOutcome::Retry;
        for _ in 0..MAX_CONNECTION_ATTEMPTS {
            last = conn.attempt_activation(&mut client, &settings);
        }
        assert_eq!(last, ActivationOutcome::GaveUp);
    }

    #[test]
    fn connect_checks_the_callsign_then_polls() {
        let settings = hoppie();
        let mut conn = AcarsConnector::new();
        conn.acars_active = true;
        let mut client = ScriptedClient::new(vec![body("ok"), body("ok")]);
        assert_eq!(conn.connect(&mut client, &settings, "FBW001"), AtsuStatusCodes::Ok);
        assert_eq!(conn.flight_number, "FBW001");
        assert_eq!(client.requests.len(), 2);
        assert!(client.requests[1].contains("type=poll"));

        let mut busy = AcarsConnector::new();
        busy.acars_active = true;
        let mut client = ScriptedClient::new(vec![body("error {callsign already in use}")]);
        assert_eq!(busy.connect(&mut client, &settings, "FBW001"), AtsuStatusCodes::CallsignInUse);
        assert_eq!(busy.flight_number, "");
    }

    #[test]
    fn station_availability() {
        let settings = hoppie();
        let mut conn = AcarsConnector::new();
        conn.acars_active = true;
        conn.flight_number = "FBW001".into();
        let mut client = ScriptedClient::new(vec![body("ok EDMM"), body("ok"), body("error {boom}")]);
        assert_eq!(conn.is_station_available(&mut client, &settings, "FBW001"), AtsuStatusCodes::OwnCallsign);
        assert_eq!(conn.is_station_available(&mut client, &settings, "EDMM"), AtsuStatusCodes::Ok);
        assert_eq!(conn.is_station_available(&mut client, &settings, "EDMM"), AtsuStatusCodes::NoAtc);
        assert_eq!(conn.is_station_available(&mut client, &settings, "EDMM"), AtsuStatusCodes::ProxyError);
    }

    #[test]
    fn telex_and_cpdlc_are_sent_in_network_format() {
        let settings = hoppie();
        let mut conn = AcarsConnector::new();
        conn.acars_active = true;
        conn.flight_number = "FBW001".into();

        let mut telex = FreetextMessage::new();
        telex.header.station = "EGLL".into();
        telex.header.message = "HELLO".into();
        let mut client = ScriptedClient::new(vec![body("ok")]);
        assert_eq!(conn.send_telex_message(&mut client, &settings, &telex, false), AtsuStatusCodes::Ok);
        assert!(client.requests[0].ends_with("from=FBW001&to=EGLL&type=telex&packet=HELLO"));

        let mut msg = CpdlcMessage::new();
        msg.header.station = "EDMM".into();
        msg.current_transmission_id = 2;
        msg.content.push(crate::elements::downlink_element("DM0").unwrap());
        let mut client = ScriptedClient::new(vec![body("ok")]);
        assert_eq!(conn.send_cpdlc_message(&mut client, &settings, &msg, false), AtsuStatusCodes::Ok);
        assert!(client.requests[0].ends_with("type=cpdlc&packet=%2Fdata2%2F2%2F%2FN%2FWILCO"));

        let mut client = ScriptedClient::new(vec![body("ok something")]);
        assert_eq!(conn.send_cpdlc_message(&mut client, &settings, &msg, false), AtsuStatusCodes::ComFailed);
        let mut client = ScriptedClient::new(vec![Err("down".into())]);
        assert_eq!(conn.send_cpdlc_message(&mut client, &settings, &msg, false), AtsuStatusCodes::ProxyError);

        conn.acars_active = false;
        assert_eq!(
            conn.send_cpdlc_message(&mut client, &settings, &msg, false),
            AtsuStatusCodes::NoAcarsConnection
        );
    }

    #[test]
    fn poll_decodes_telex_and_cpdlc() {
        let settings = hoppie();
        let mut conn = AcarsConnector::new();
        conn.acars_active = true;
        conn.flight_number = "FBW001".into();
        let reply = "ok {EGLL telex {Hello world!}} {EDMM cpdlc {/data2/7//WU/CLIMB TO FL350}}";
        let mut client = ScriptedClient::new(vec![body(reply)]);
        let (code, messages) = conn.poll(&mut client, &settings);
        assert_eq!(code, AtsuStatusCodes::Ok);
        assert_eq!(messages.len(), 2);
        match &messages[0] {
            AnyMessage::Freetext(m) => {
                assert_eq!(m.header.station, "EGLL");
                assert_eq!(m.header.message, "HELLO WORLD!");
                assert_eq!(m.header.com_status, ComStatus::Received);
                assert_eq!(m.header.direction, MessageDirection::Uplink);
            }
            other => panic!("unexpected {other:?}"),
        }
        match &messages[1] {
            AnyMessage::Cpdlc(m) => {
                assert_eq!(m.header.station, "EDMM");
                assert_eq!(m.current_transmission_id, 7);
                assert_eq!(m.previous_transmission_id, -1);
                assert_eq!(m.content[0].type_id, "UM20");
                assert_eq!(m.content[0].content[0].value, "FL350");
                assert_eq!(m.content[0].expected_response, ExpectedResponse::WilcoUnable);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn poll_error_paths() {
        let settings = hoppie();
        let mut conn = AcarsConnector::new();
        let mut client = ScriptedClient::new(vec![]);
        assert_eq!(conn.poll(&mut client, &settings).0, AtsuStatusCodes::NoAcarsConnection);
        conn.acars_active = true;
        conn.flight_number = "FBW001".into();
        let mut client = ScriptedClient::new(vec![Err("x".into())]);
        assert_eq!(conn.poll(&mut client, &settings).0, AtsuStatusCodes::ProxyError);
        let mut client = ScriptedClient::new(vec![body("error {callsign already in use}")]);
        assert_eq!(conn.poll(&mut client, &settings).0, AtsuStatusCodes::CallsignInUse);
        let mut client = ScriptedClient::new(vec![body("error {other}")]);
        assert_eq!(conn.poll(&mut client, &settings).0, AtsuStatusCodes::ComFailed);
        let mut client = ScriptedClient::new(vec![body("ok")]);
        let (code, list) = conn.poll(&mut client, &settings);
        assert_eq!((code, list.len()), (AtsuStatusCodes::Ok, 0));
    }

    #[test]
    fn classification_picks_the_closest_template() {
        let fans = FansMode::FansA;
        let e = classify_uplink("CLIMB TO FL350", fans).unwrap();
        assert_eq!(e.type_id, "UM20");
        assert_eq!(e.content[0].value, "FL350");

        let e = classify_uplink("CONTACT EDMM CTR 132.150", fans).map(|e| e.type_id);
        assert!(e.is_some());

        let e = classify_uplink("UNABLE", fans).unwrap();
        assert_eq!(e.type_id, "UM0");
        assert!(e.content.is_empty());
    }

    #[test]
    fn poll_interval_is_between_45_and_75_seconds() {
        assert_eq!(AcarsConnector::poll_interval_ms(0.0), 45_000.0);
        assert_eq!(AcarsConnector::poll_interval_ms(0.5), 60_000.0);
    }
}

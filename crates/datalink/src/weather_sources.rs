use serde::Deserialize;

use crate::acars::HttpClient;
use crate::status::AtsuStatusCodes;
use crate::weather::{Report, WeatherMessage};

const SAI_BASE_URL: &str = "https://apipri.sayintentions.ai/sapi/getWX";
const BATC_METAR_URL: &str = "http://localhost:57698/acars/metar";
const BATC_ATIS_URL: &str = "http://localhost:57698/acars/atis";

#[derive(Debug, Default, Deserialize)]
pub struct SayIntentionsAirport {
    pub metar: Option<String>,
    pub atis: Option<String>,
    pub atis_cpdlc: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SayIntentionsResponse {
    airports: Option<Vec<SayIntentionsAirport>>,
}

pub struct SayIntentions<'a> {
    pub client: &'a mut dyn HttpClient,
    pub api_key: String,
}

impl SayIntentions<'_> {
    pub fn build_url(api_key: &str, icao: &str) -> String {
        format!(
            "{}?api_key={}&icao={}",
            SAI_BASE_URL,
            crate::acars::form_urlencode(api_key),
            crate::acars::form_urlencode(&icao.to_uppercase())
        )
    }

    fn fetch_airport(&mut self, icao: &str) -> Result<SayIntentionsAirport, String> {
        if self.api_key.is_empty() {
            return Err("Missing SAI API key".into());
        }
        let url = SayIntentions::build_url(&self.api_key, icao);
        let response = self.client.get(&url, "*/*")?;
        if !response.ok() {
            return Err(format!("SAI request failed ({})", response.status));
        }
        let data: SayIntentionsResponse = serde_json::from_str(&response.body).map_err(|e| e.to_string())?;
        data.airports
            .and_then(|a| a.into_iter().next())
            .ok_or_else(|| "SAI weather not available".to_string())
    }

    fn select_metar(airport: &SayIntentionsAirport) -> Option<String> {
        let metar = airport.metar.as_deref()?.trim();
        if metar.is_empty() {
            None
        } else {
            Some(metar.to_string())
        }
    }

    pub fn receive_metar(&mut self, icao: &str, message: &mut WeatherMessage) -> AtsuStatusCodes {
        let report = match self.fetch_airport(icao) {
            Ok(airport) => SayIntentions::select_metar(&airport).unwrap_or_else(|| "SAI METAR NOT AVAILABLE".into()),
            Err(_) => "SAI METAR NOT AVAILABLE".into(),
        };
        message.reports.push(Report {
            airport: icao.to_string(),
            report,
        });
        AtsuStatusCodes::Ok
    }

    pub fn receive_atis(&mut self, icao: &str, message: &mut WeatherMessage) -> AtsuStatusCodes {
        let report = match self.fetch_airport(icao) {
            Ok(airport) => airport
                .atis_cpdlc
                .or(airport.atis)
                .unwrap_or_else(|| "SAI D-ATIS NOT AVAILABLE".into()),
            Err(_) => "SAI D-ATIS NOT AVAILABLE".into(),
        };
        message.reports.push(Report {
            airport: icao.to_string(),
            report,
        });
        AtsuStatusCodes::Ok
    }

    pub fn get_metar(&mut self, icao: &str) -> Option<String> {
        self.fetch_airport(icao).ok().and_then(|a| SayIntentions::select_metar(&a))
    }

    pub fn get_atis(&mut self, icao: &str) -> Option<String> {
        self.fetch_airport(icao)
            .ok()
            .map(|a| a.atis_cpdlc.unwrap_or_else(|| "SAI D-ATIS NOT AVAILABLE".into()))
    }
}

pub struct BeyondAtc<'a> {
    pub client: &'a mut dyn HttpClient,
}

impl BeyondAtc<'_> {
    pub fn metar_url(icao: &str) -> String {
        format!("{}/{}", BATC_METAR_URL, icao.to_uppercase())
    }

    pub fn atis_url(icao: &str) -> String {
        format!("{}/{}", BATC_ATIS_URL, icao.to_uppercase())
    }

    fn fetch(&mut self, url: &str) -> Option<String> {
        match self.client.get(url, "*/*") {
            Ok(response) if response.ok() => Some(response.body),
            _ => None,
        }
    }

    fn receive(&mut self, url: &str, icao: &str, missing: &str, message: &mut WeatherMessage) -> AtsuStatusCodes {
        let report = match self.fetch(url) {
            Some(text) if !text.is_empty() => text,
            _ => missing.to_string(),
        };
        message.reports.push(Report {
            airport: icao.to_string(),
            report,
        });
        AtsuStatusCodes::Ok
    }

    pub fn receive_metar(&mut self, icao: &str, message: &mut WeatherMessage) -> AtsuStatusCodes {
        let url = BeyondAtc::metar_url(icao);
        self.receive(&url, icao, "BEYONDATC METAR NOT AVAILABLE", message)
    }

    pub fn receive_atis(&mut self, icao: &str, message: &mut WeatherMessage) -> AtsuStatusCodes {
        let url = BeyondAtc::atis_url(icao);
        self.receive(&url, icao, "BEYONDATC D-ATIS NOT AVAILABLE", message)
    }

    pub fn get_metar(&mut self, icao: &str) -> Option<String> {
        let url = BeyondAtc::metar_url(icao);
        self.fetch(&url)
    }

    pub fn get_atis(&mut self, icao: &str) -> Option<String> {
        let url = BeyondAtc::atis_url(icao);
        self.fetch(&url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acars::tests::{body, ScriptedClient};
    use crate::acars::HttpResponse;
    use crate::message::MessageType;

    fn wx() -> WeatherMessage {
        WeatherMessage::new(MessageType::Metar, "SAI")
    }

    #[test]
    fn sai_metar_and_atis() {
        let json = r#"{"airports":[{"icao":"KJFK","metar":"  KJFK 221651Z 28012KT 10SM FEW250 15/05 A3012  ","atis":"ATIS PLAIN","atis_cpdlc":"KJFK ATIS INFO H"}]}"#;
        let mut client = ScriptedClient::new(vec![body(json), body(json)]);
        let mut sai = SayIntentions { client: &mut client, api_key: "KEY".into() };
        let mut message = wx();
        assert_eq!(sai.receive_metar("kjfk", &mut message), AtsuStatusCodes::Ok);
        assert_eq!(message.reports[0].report, "KJFK 221651Z 28012KT 10SM FEW250 15/05 A3012");
        sai.receive_atis("kjfk", &mut message);
        assert_eq!(message.reports[1].report, "KJFK ATIS INFO H");
        assert_eq!(
            client.requests[0],
            "https://apipri.sayintentions.ai/sapi/getWX?api_key=KEY&icao=KJFK"
        );
    }

    #[test]
    fn sai_failures_produce_the_placeholder_text() {
        let mut client = ScriptedClient::new(vec![
            Ok(HttpResponse { status: 500, body: String::new() }),
            body(r#"{"airports":[]}"#),
            body("not json"),
        ]);
        let mut sai = SayIntentions { client: &mut client, api_key: "KEY".into() };
        let mut message = wx();
        sai.receive_metar("KJFK", &mut message);
        sai.receive_atis("KJFK", &mut message);
        sai.receive_metar("KJFK", &mut message);
        assert_eq!(message.reports[0].report, "SAI METAR NOT AVAILABLE");
        assert_eq!(message.reports[1].report, "SAI D-ATIS NOT AVAILABLE");
        assert_eq!(message.reports[2].report, "SAI METAR NOT AVAILABLE");

        let mut client = ScriptedClient::new(vec![]);
        let mut keyless = SayIntentions { client: &mut client, api_key: String::new() };
        assert_eq!(keyless.get_metar("KJFK"), None);
        assert!(client.requests.is_empty());
    }

    #[test]
    fn sai_atis_without_cpdlc_text_falls_back_to_the_plain_atis() {
        let json = r#"{"airports":[{"atis":"PLAIN ATIS"}]}"#;
        let mut client = ScriptedClient::new(vec![body(json), body(json)]);
        let mut sai = SayIntentions { client: &mut client, api_key: "K".into() };
        let mut message = wx();
        sai.receive_atis("EGLL", &mut message);
        assert_eq!(message.reports[0].report, "PLAIN ATIS");
        assert_eq!(sai.get_atis("EGLL").as_deref(), Some("SAI D-ATIS NOT AVAILABLE"));
    }

    #[test]
    fn beyondatc_urls_and_placeholders() {
        let mut client = ScriptedClient::new(vec![
            body("EGLL 221650Z 24010KT"),
            body(""),
            Ok(HttpResponse { status: 404, body: "x".into() }),
            Err("refused".into()),
        ]);
        let mut batc = BeyondAtc { client: &mut client };
        let mut message = wx();
        batc.receive_metar("egll", &mut message);
        batc.receive_metar("egll", &mut message);
        batc.receive_atis("egll", &mut message);
        batc.receive_atis("egll", &mut message);
        assert_eq!(message.reports[0].report, "EGLL 221650Z 24010KT");
        assert_eq!(message.reports[1].report, "BEYONDATC METAR NOT AVAILABLE");
        assert_eq!(message.reports[2].report, "BEYONDATC D-ATIS NOT AVAILABLE");
        assert_eq!(message.reports[3].report, "BEYONDATC D-ATIS NOT AVAILABLE");
        assert_eq!(client.requests[0], "http://localhost:57698/acars/metar/EGLL");
        assert_eq!(client.requests[2], "http://localhost:57698/acars/atis/EGLL");
    }
}

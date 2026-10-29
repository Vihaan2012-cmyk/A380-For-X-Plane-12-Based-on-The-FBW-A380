use serde::{Deserialize, Serialize};

use crate::convert::word_wrap;
use crate::message::{Header, MessageDirection, MessageType, SerializationFormat};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub airport: String,
    pub report: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtisType {
    Departure,
    Arrival,
    Enroute,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WeatherMessage {
    #[serde(flatten)]
    pub header: Header,
    #[serde(rename = "Reports")]
    pub reports: Vec<Report>,
}

impl WeatherMessage {
    pub fn new(kind: MessageType, station: &str) -> WeatherMessage {
        let mut header = Header::new(kind, MessageDirection::Uplink);
        header.station = station.to_string();
        WeatherMessage {
            header,
            reports: Vec::new(),
        }
    }

    pub fn render(&self, format: SerializationFormat) -> String {
        let kind = match self.header.kind {
            MessageType::Metar => "METAR",
            MessageType::Taf => "TAF",
            _ => "ATIS",
        };
        let mut message = String::new();

        if matches!(
            format,
            SerializationFormat::FmsDisplay | SerializationFormat::FmsDisplayMonitored
        ) {
            for report in &self.reports {
                message.push_str(&format!("{{cyan}}{} {}{{end}}\n", kind, report.airport));
                for line in word_wrap(&report.report, 25) {
                    if line.starts_with("D-ATIS") || line == "NO METAR AVAILABLE" || line == "NO TAF AVAILABLE" {
                        message.push_str(&format!("{{amber}}{line}{{end}}\n"));
                    } else {
                        message.push_str(&format!("{{green}}{line}{{end}}\n"));
                    }
                }
                message.push_str("{white}------------------------{end}\n");
            }
        } else {
            for report in &self.reports {
                message.push_str(&format!("{} {}\n", kind, report.airport));
                message.push_str(&format!("{}\n", report.report));
                message.push_str("------------------------\n");
            }
        }
        message
    }
}

macro_rules! weather_kind {
    ($name:ident, $kind:expr, $upper:expr) => {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct $name {
            #[serde(flatten)]
            pub weather: WeatherMessage,
        }

        impl $name {
            pub fn new(station: &str) -> $name {
                $name {
                    weather: WeatherMessage::new(
                        $kind,
                        &(if $upper { station.to_uppercase() } else { station.to_string() }),
                    ),
                }
            }
        }
    };
}

weather_kind!(MetarMessage, MessageType::Metar, true);
weather_kind!(TafMessage, MessageType::Taf, false);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AtisMessage {
    #[serde(flatten)]
    pub weather: WeatherMessage,
    #[serde(rename = "Information")]
    pub information: String,
}

impl AtisMessage {
    pub fn new(station: &str) -> AtisMessage {
        AtisMessage {
            weather: WeatherMessage::new(MessageType::Atis, &station.to_uppercase()),
            information: String::new(),
        }
    }

    pub fn parse_information(&mut self) {
        let mut found_info = false;
        for report in &self.weather.reports {
            for word in report.report.split(' ') {
                if !found_info {
                    let upper = word.to_uppercase();
                    if upper == "INFORMATION" || upper == "INFO" || upper == report.airport.to_uppercase() {
                        found_info = true;
                    }
                } else {
                    self.information = word.to_string();
                    if self.information.chars().count() > 1 {
                        self.information = self.information.chars().take(1).collect();
                    }
                    found_info = false;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atis_information_letter() {
        let mut atis = AtisMessage::new("faa");
        atis.weather.reports.push(Report {
            airport: "KJFK".into(),
            report: "KJFK ATIS INFORMATION HOTEL 1651Z".into(),
        });
        atis.parse_information();
        assert_eq!(atis.information, "H");
        assert_eq!(atis.weather.header.station, "FAA");
    }

    #[test]
    fn atis_information_by_airport_ident() {
        let mut atis = AtisMessage::new("faa");
        atis.weather.reports.push(Report {
            airport: "YBBN".into(),
            report: "YBBN K AUTO".into(),
        });
        atis.parse_information();
        assert_eq!(atis.information, "K");
    }

    #[test]
    fn fms_weather_colours() {
        let mut metar = MetarMessage::new("ms");
        metar.weather.reports.push(Report {
            airport: "EGLL".into(),
            report: "NO METAR AVAILABLE".into(),
        });
        let text = metar.weather.render(SerializationFormat::FmsDisplay);
        assert_eq!(
            text,
            "{cyan}METAR EGLL{end}\n{amber}NO METAR AVAILABLE{end}\n{white}------------------------{end}\n"
        );
        let plain = metar.weather.render(SerializationFormat::Printer);
        assert_eq!(plain, "METAR EGLL\nNO METAR AVAILABLE\n------------------------\n");
    }
}

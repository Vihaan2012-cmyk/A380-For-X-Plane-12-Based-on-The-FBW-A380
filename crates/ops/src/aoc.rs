//! A simulated airline operations centre for flights flown off-network. It
//! turns flight events into company uplinks (gate, final loadsheet, delays,
//! OOOI acknowledgements); the plugin hands each one to the datalink AOC as
//! a freetext uplink. Reply delays and the delay threshold are GENERIC.

use serde::{Deserialize, Serialize};

use crate::loadsheet::Loadsheet;
use crate::settings::Units;

pub const REPLY_DELAY_S: f64 = 30.0;
pub const LOADSHEET_DELAY_S: f64 = 90.0;
pub const OOOI_ACK_DELAY_S: f64 = 15.0;
pub const DELAY_REPORT_MIN: f64 = 15.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Flight {
    pub airline_icao: String,
    pub flight_number: String,
    pub origin: String,
    pub destination: String,
    /// Scheduled off-block, seconds since midnight UTC.
    pub std_s: Option<f64>,
}

impl Flight {
    pub fn callsign(&self) -> String {
        format!("{}{}", self.airline_icao.trim(), self.flight_number.trim())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Oooi {
    Out,
    Off,
    On,
    In,
}

impl Oooi {
    fn label(self) -> &'static str {
        match self {
            Oooi::Out => "OUT",
            Oooi::Off => "OFF",
            Oooi::On => "ON",
            Oooi::In => "IN",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    FlightInitialised { flight: Flight, parking_spot: Option<String> },
    ArrivalGate { parking_spot: String },
    BoardingStarted,
    BoardingComplete { loadsheet: Loadsheet, units: Units },
    Oooi { kind: Oooi },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Gate,
    Loadsheet,
    Delay,
    OooiAck,
    Info,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Uplink {
    pub due_s: f64,
    pub station: String,
    pub kind: Kind,
    pub text: String,
}

/// Turns a scenery parking spot name into the gate an airline would quote:
/// "Gate A12" -> "A12", "Stand 305 Heavy" -> "305", "Ramp 4" -> "R4".
pub fn gate_from_spot(name: &str) -> Option<String> {
    let words: Vec<&str> = name.split(|c: char| c.is_whitespace() || c == '-' || c == '_').filter(|w| !w.is_empty()).collect();
    let is_id = |w: &str| w.chars().any(|c| c.is_ascii_digit()) && w.chars().all(|c| c.is_ascii_alphanumeric());
    let lead = words.first()?.to_ascii_uppercase();
    let id = words.iter().find(|w| is_id(w))?.to_ascii_uppercase();
    if lead.starts_with("RAMP") || lead.starts_with("APRON") || lead.starts_with("PARKING") {
        Some(format!("R{id}"))
    } else {
        Some(id)
    }
}

fn hhmm(t_s: f64) -> String {
    let m = (t_s.rem_euclid(86_400.0) / 60.0).floor() as u32;
    format!("{:02}{:02}", m / 60, m % 60)
}

fn weight(kg: f64, units: Units) -> String {
    format!("{:.1}", units.from_kg(kg) / 1000.0)
}

pub fn loadsheet_text(flight: &Flight, ls: &Loadsheet, units: Units) -> String {
    let u = units.label();
    let mut lines = vec![
        format!("FINAL LOADSHEET {}", flight.callsign()),
        format!("{}-{}", flight.origin, flight.destination),
        format!("PAX {} (MAIN {} UPPER {})", ls.pax, ls.pax_main, ls.pax_upper),
        format!("CARGO {} T{u}", weight(ls.cargo_kg, units)),
        format!("ZFW {} T{u} ZFWCG {:.1}", weight(ls.zfw_kg, units), ls.zfw_cg),
        format!("TOF {} T{u}", weight(ls.fuel_kg, units)),
        format!("TOW {} T{u} TOWCG {:.1}", weight(ls.tow_kg, units), ls.tow_cg),
        format!("LAW {} T{u}", weight(ls.lw_kg, units)),
        format!("THS {:.1} {}", ls.ths_deg.abs(), if ls.ths_deg < 0.0 { "DN" } else { "UP" }),
    ];
    if !ls.within_limits() {
        lines.push("LIMITS EXCEEDED - CONTACT LOAD CONTROL".into());
    }
    lines.join("\n")
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AocSim {
    flight: Option<Flight>,
    queue: Vec<Uplink>,
    delay_sent: bool,
    out_at_s: Option<f64>,
}

impl AocSim {
    pub fn new() -> Self {
        Self::default()
    }

    fn station(&self) -> String {
        self.flight.as_ref().map(|f| format!("{} OPS", f.airline_icao.trim())).unwrap_or_else(|| "OPS".into())
    }

    fn push(&mut self, due_s: f64, kind: Kind, text: String) {
        let station = self.station();
        self.queue.push(Uplink { due_s, station, kind, text });
    }

    pub fn on_event(&mut self, now_s: f64, event: Event) {
        match event {
            Event::FlightInitialised { flight, parking_spot } => {
                self.flight = Some(flight.clone());
                self.delay_sent = false;
                self.out_at_s = None;
                let gate = parking_spot.as_deref().and_then(gate_from_spot);
                let text = match gate {
                    Some(g) => format!("{} {}-{} DEP GATE {g}", flight.callsign(), flight.origin, flight.destination),
                    None => format!("{} {}-{} DEP STAND TBA", flight.callsign(), flight.origin, flight.destination),
                };
                self.push(now_s + REPLY_DELAY_S, Kind::Gate, text);
            }
            Event::ArrivalGate { parking_spot } => {
                if let (Some(f), Some(g)) = (self.flight.clone(), gate_from_spot(&parking_spot)) {
                    self.push(now_s + REPLY_DELAY_S, Kind::Gate, format!("{} ARR {} GATE {g}", f.callsign(), f.destination));
                }
            }
            Event::BoardingStarted => {
                if let Some(f) = self.flight.clone() {
                    self.push(now_s + REPLY_DELAY_S, Kind::Info, format!("{} BOARDING STARTED", f.callsign()));
                }
            }
            Event::BoardingComplete { loadsheet, units } => {
                if let Some(f) = self.flight.clone() {
                    self.push(now_s + LOADSHEET_DELAY_S, Kind::Loadsheet, loadsheet_text(&f, &loadsheet, units));
                }
            }
            Event::Oooi { kind } => {
                if kind == Oooi::Out {
                    self.out_at_s = Some(now_s);
                }
                let call = self.flight.as_ref().map(Flight::callsign).unwrap_or_default();
                self.push(now_s + OOOI_ACK_DELAY_S, Kind::OooiAck, format!("{call} {} {} RCVD", kind.label(), hhmm(now_s)));
            }
        }
    }

    /// Checks the schedule; once off-block is late by the threshold a delay
    /// message goes out, once per flight.
    pub fn check_delay(&mut self, now_s: f64) {
        let Some(f) = self.flight.clone() else { return };
        let Some(std_s) = f.std_s else { return };
        if self.delay_sent || self.out_at_s.is_some() {
            return;
        }
        let late_min = (now_s - std_s) / 60.0;
        if late_min >= DELAY_REPORT_MIN {
            self.delay_sent = true;
            let etd = std_s + (late_min / 5.0).ceil() * 5.0 * 60.0 + 10.0 * 60.0;
            self.push(
                now_s + REPLY_DELAY_S,
                Kind::Delay,
                format!("{} DELAYED STD {} NEW ETD {} ADVISE READY", f.callsign(), hhmm(std_s), hhmm(etd)),
            );
        }
    }

    /// Uplinks now due, oldest first.
    pub fn poll(&mut self, now_s: f64) -> Vec<Uplink> {
        self.check_delay(now_s);
        let (due, rest): (Vec<_>, Vec<_>) = self.queue.drain(..).partition(|u| u.due_s <= now_s);
        self.queue = rest;
        let mut due = due;
        due.sort_by(|a, b| a.due_s.total_cmp(&b.due_s));
        due
    }

    pub fn pending(&self) -> usize {
        self.queue.len()
    }
}

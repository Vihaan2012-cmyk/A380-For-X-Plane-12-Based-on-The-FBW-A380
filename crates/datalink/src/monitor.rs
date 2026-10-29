use crate::message::{ComStatus, CpdlcMessage};
use crate::status::{Clock, Waypoint};

const POSITION_IDS: [&str; 10] = [
    "UM22", "UM25", "UM65", "UM77", "UM83", "UM84", "UM97", "UM118", "UM121", "UM130",
];
const TIME_IDS: [&str; 7] = ["UM21", "UM24", "UM66", "UM76", "UM119", "UM122", "UM184"];
const LEVEL_IDS: [&str; 6] = ["UM78", "UM128", "UM129", "UM130", "UM175", "UM180"];
const DEFERRED_TIME_IDS: [&str; 4] = ["UM66", "UM69", "UM119", "UM122"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UplinkMonitorType {
    Unknown,
    Time,
    Altitude,
    Position,
}

pub enum Condition<'a> {
    Time(&'a Clock),
    Altitude(f64),
    Position(&'a Waypoint),
}

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Position { position: String },
    Time { offset: f64, target: f64 },
    Level(LevelMonitor),
}

#[derive(Clone, Debug, PartialEq)]
struct LevelMonitor {
    lower: f64,
    upper: f64,
    reaching: bool,
    leaving: bool,
    reached: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UplinkMonitor {
    pub message_id: i64,
    kind: Kind,
}

fn first_type_id(message: &CpdlcMessage) -> Option<&str> {
    message.content.first().map(|c| c.type_id.as_str())
}

fn slot_value(message: &CpdlcMessage, index: usize) -> &str {
    message
        .content
        .first()
        .and_then(|c| c.content.get(index))
        .map(|c| c.value.as_str())
        .unwrap_or("")
}

fn extract_seconds(value: &str) -> f64 {
    let digits: Vec<u8> = value.bytes().filter(|b| b.is_ascii_digit()).collect();
    let pair = |i: usize| -> f64 {
        if digits.len() >= i + 2 {
            ((digits[i] - b'0') as f64) * 10.0 + (digits[i + 1] - b'0') as f64
        } else {
            0.0
        }
    };
    (pair(0) * 60.0 + pair(2)) * 60.0
}

fn extract_altitude(value: &str) -> f64 {
    let run: String = value
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let mut altitude: f64 = run.parse().unwrap_or(0.0);
    if value.starts_with("FL") {
        altitude *= 100.0;
    } else if value.ends_with('M') {
        altitude *= 3.28084;
    }
    altitude
}

impl UplinkMonitor {
    pub fn relevant_message(message: &CpdlcMessage) -> bool {
        match first_type_id(message) {
            Some(id) => POSITION_IDS.contains(&id) || TIME_IDS.contains(&id) || LEVEL_IDS.contains(&id),
            None => false,
        }
    }

    pub fn create_message_monitor(message: &CpdlcMessage) -> Option<UplinkMonitor> {
        let id = first_type_id(message)?;
        let kind = if POSITION_IDS.contains(&id) {
            Kind::Position { position: slot_value(message, 0).to_string() }
        } else if TIME_IDS.contains(&id) {
            Kind::Time {
                offset: if DEFERRED_TIME_IDS.contains(&id) { 30.0 } else { 0.0 },
                target: extract_seconds(slot_value(message, 0)),
            }
        } else if LEVEL_IDS.contains(&id) {
            let lower = extract_altitude(slot_value(message, 0));
            let mut level = LevelMonitor { lower, upper: -1.0, reaching: false, leaving: false, reached: false };
            match id {
                "UM180" => {
                    level.upper = extract_altitude(slot_value(message, 1));
                    level.reaching = true;
                }
                "UM78" | "UM129" | "UM175" => level.reaching = true,
                "UM128" => level.reaching = false,
                "UM130" => {
                    level.reaching = true;
                    level.leaving = true;
                }
                _ => {}
            }
            Kind::Level(level)
        } else {
            return None;
        };
        Some(UplinkMonitor { message_id: message.header.unique_message_id, kind })
    }

    pub fn monitor_type(&self) -> UplinkMonitorType {
        match self.kind {
            Kind::Position { .. } => UplinkMonitorType::Position,
            Kind::Time { .. } => UplinkMonitorType::Time,
            Kind::Level(_) => UplinkMonitorType::Altitude,
        }
    }

    pub fn conditions_met(&mut self, condition: Condition) -> bool {
        match (&mut self.kind, condition) {
            (Kind::Position { position }, Condition::Position(waypoint)) => *position == waypoint.ident,
            (Kind::Time { offset, target }, Condition::Time(clock)) => {
                if clock.seconds_of_day + *offset >= *target {
                    return clock.seconds_of_day - *target < 30.0;
                }
                false
            }
            (Kind::Level(level), Condition::Altitude(altitude)) => {
                if level.reaching && level.leaving {
                    if !level.reached {
                        level.reached = (altitude - level.lower).abs() <= 100.0;
                    } else {
                        return (altitude - level.lower).abs() > 100.0;
                    }
                }
                if !level.reaching {
                    return (altitude - level.lower).abs() > 100.0;
                }
                if level.upper > -1.0 {
                    return level.lower <= altitude && level.upper >= altitude;
                }
                (altitude - level.lower).abs() <= 100.0
            }
            _ => false,
        }
    }
}

#[derive(Default)]
pub struct UplinkMessageMonitoring {
    monitored: Vec<UplinkMonitor>,
}

impl UplinkMessageMonitoring {
    pub fn new() -> UplinkMessageMonitoring {
        UplinkMessageMonitoring::default()
    }

    pub fn monitor_message(&mut self, message: &CpdlcMessage) -> bool {
        if UplinkMonitor::relevant_message(message) {
            if let Some(monitor) = UplinkMonitor::create_message_monitor(message) {
                self.monitored.push(monitor);
                return true;
            }
        }
        false
    }

    pub fn remove_message(&mut self, uid: i64) -> bool {
        match self.monitored.iter().position(|m| m.message_id == uid) {
            Some(index) => {
                self.monitored.remove(index);
                true
            }
            None => false,
        }
    }

    pub fn monitored_message_ids(&self) -> Vec<i64> {
        self.monitored.iter().map(|m| m.message_id).collect()
    }

    pub fn check_message_conditions(
        &mut self,
        messages: &[CpdlcMessage],
        clock: &Clock,
        altitude: f64,
        last_waypoint: Option<&Waypoint>,
    ) -> Vec<i64> {
        let mut ids = Vec::new();
        let mut index = self.monitored.len();
        while index > 0 {
            index -= 1;
            let monitor = &mut self.monitored[index];
            let mut met = monitor.conditions_met(Condition::Time(clock));
            met = met || monitor.conditions_met(Condition::Altitude(altitude));
            if let Some(waypoint) = last_waypoint {
                met = met || monitor.conditions_met(Condition::Position(waypoint));
            }
            if met {
                let id = monitor.message_id;
                let sent = messages
                    .iter()
                    .find(|m| m.header.unique_message_id == id)
                    .and_then(|m| m.response.as_ref())
                    .is_some_and(|r| r.header.com_status == ComStatus::Sent);
                if sent {
                    ids.push(id);
                    self.monitored.remove(index);
                }
            }
        }
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::uplink;

    fn clock(seconds: f64) -> Clock {
        Clock { year: 2026, month: 10, day_of_month: 22, hour: 0, minute: 0, second: 0, seconds_of_day: seconds }
    }

    fn message(id: &str, values: &[&str], uid: i64) -> CpdlcMessage {
        let mut m = CpdlcMessage::new();
        m.header.unique_message_id = uid;
        let mut element = uplink(id).unwrap().to_element();
        for (slot, value) in element.content.iter_mut().zip(values) {
            slot.value = value.to_string();
        }
        m.content.push(element);
        m
    }

    #[test]
    fn relevance_follows_the_id_tables() {
        assert!(UplinkMonitor::relevant_message(&message("UM21", &["1200Z"], 1)));
        assert!(UplinkMonitor::relevant_message(&message("UM22", &["ABC"], 1)));
        assert!(UplinkMonitor::relevant_message(&message("UM78", &["FL100"], 1)));
        assert!(!UplinkMonitor::relevant_message(&message("UM20", &["FL100"], 1)));
        assert!(!UplinkMonitor::relevant_message(&CpdlcMessage::new()));
    }

    #[test]
    fn um130_is_a_position_monitor_first() {
        let m = UplinkMonitor::create_message_monitor(&message("UM130", &["ABC"], 1)).unwrap();
        assert_eq!(m.monitor_type(), UplinkMonitorType::Position);
    }

    #[test]
    fn time_monitor_with_day_change_guard() {
        let mut t = UplinkMonitor::create_message_monitor(&message("UM21", &["1200Z"], 1)).unwrap();
        assert_eq!(t.monitor_type(), UplinkMonitorType::Time);
        assert!(!t.conditions_met(Condition::Time(&clock(43199.0))));
        assert!(t.conditions_met(Condition::Time(&clock(43200.0))));
        assert!(t.conditions_met(Condition::Time(&clock(43229.0))));
        assert!(!t.conditions_met(Condition::Time(&clock(43300.0))));
        let mut deferred = UplinkMonitor::create_message_monitor(&message("UM66", &["1200Z"], 1)).unwrap();
        assert!(deferred.conditions_met(Condition::Time(&clock(43175.0))));
    }

    #[test]
    fn level_monitors() {
        let mut reach = UplinkMonitor::create_message_monitor(&message("UM78", &["FL100"], 1)).unwrap();
        assert!(!reach.conditions_met(Condition::Altitude(9000.0)));
        assert!(reach.conditions_met(Condition::Altitude(9950.0)));
        let mut leave = UplinkMonitor::create_message_monitor(&message("UM128", &["FL100"], 1)).unwrap();
        assert!(!leave.conditions_met(Condition::Altitude(10050.0)));
        assert!(leave.conditions_met(Condition::Altitude(10300.0)));
        assert!(!reach.conditions_met(Condition::Time(&clock(0.0))));
    }

    #[test]
    fn position_monitor_matches_last_waypoint() {
        let mut p = UplinkMonitor::create_message_monitor(&message("UM22", &["ABC"], 1)).unwrap();
        let wp = Waypoint { ident: "ABC".into(), altitude: 0.0, utc: 0.0 };
        let other = Waypoint { ident: "XYZ".into(), altitude: 0.0, utc: 0.0 };
        assert!(p.conditions_met(Condition::Position(&wp)));
        assert!(!p.conditions_met(Condition::Position(&other)));
    }

    #[test]
    fn collection_only_releases_messages_with_a_sent_response() {
        let mut monitoring = UplinkMessageMonitoring::new();
        let mut m = message("UM22", &["ABC"], 7);
        assert!(monitoring.monitor_message(&m));
        assert_eq!(monitoring.monitored_message_ids(), vec![7]);
        let wp = Waypoint { ident: "ABC".into(), altitude: 0.0, utc: 0.0 };
        assert!(monitoring.check_message_conditions(&[m.clone()], &clock(0.0), 0.0, Some(&wp)).is_empty());
        let mut response = CpdlcMessage::new();
        response.header.com_status = ComStatus::Sent;
        m.response = Some(Box::new(response));
        assert_eq!(monitoring.check_message_conditions(&[m], &clock(0.0), 0.0, Some(&wp)), vec![7]);
        assert!(monitoring.monitored_message_ids().is_empty());
        assert!(!monitoring.remove_message(7));
    }
}

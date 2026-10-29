use crate::message::{AnyMessage, MessageDirection, Timestamp};
use crate::status::{AtsuStatusCodes, Clock};

#[derive(Clone, Debug, PartialEq)]
pub enum AocEvent {
    CompanyMessageCount(usize),
    Resynchronize(AnyMessage),
    Delete(i64),
    Transmit { request_id: u32, message: AnyMessage },
    TransmissionResponse { request_id: u32, code: AtsuStatusCodes },
    PowerDown,
}

#[derive(Debug, Default)]
pub struct Aoc {
    powered_up: bool,
    message_counter: i64,
    uplink: Vec<AnyMessage>,
    downlink: Vec<AnyMessage>,
    blacklisted: Vec<i64>,
    pending_sends: Vec<(u32, AnyMessage)>,
    events: Vec<AocEvent>,
    pub utc_clock: Option<Clock>,
}

impl Aoc {
    pub fn new() -> Aoc {
        Aoc::default()
    }

    pub fn powered_up(&self) -> bool {
        self.powered_up
    }

    pub fn take_events(&mut self) -> Vec<AocEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn uplink_messages(&self) -> &[AnyMessage] {
        &self.uplink
    }

    pub fn downlink_messages(&self) -> &[AnyMessage] {
        &self.downlink
    }

    pub fn power_up(&mut self) {
        self.powered_up = true;
    }

    pub fn power_down(&mut self) {
        self.events.push(AocEvent::PowerDown);
        self.uplink.clear();
        self.downlink.clear();
        self.pending_sends.clear();
        self.powered_up = false;
    }

    fn update_message_count(&mut self) {
        let count = self.uplink.iter().filter(|m| !m.header().confirmed).count();
        self.events.push(AocEvent::CompanyMessageCount(count));
    }

    pub fn send_message(&mut self, request_id: u32, message: AnyMessage) {
        if !self.powered_up {
            self.events.push(AocEvent::TransmissionResponse { request_id, code: AtsuStatusCodes::ComFailed });
            return;
        }
        self.pending_sends.push((request_id, message.clone()));
        self.events.push(AocEvent::Transmit { request_id, message });
    }

    pub fn on_send_result(&mut self, request_id: u32, code: AtsuStatusCodes) {
        let Some(idx) = self.pending_sends.iter().position(|(id, _)| *id == request_id) else {
            return;
        };
        let (_, message) = self.pending_sends.remove(idx);
        if code == AtsuStatusCodes::Ok {
            self.insert_messages(vec![message]);
        }
        self.events.push(AocEvent::TransmissionResponse { request_id, code });
    }

    pub fn weather_allowed(&self) -> Result<(), AtsuStatusCodes> {
        if self.powered_up {
            Ok(())
        } else {
            Err(AtsuStatusCodes::ComFailed)
        }
    }

    pub fn remove_message(&mut self, uid: i64) {
        if let Some(i) = self.uplink.iter().position(|m| m.header().unique_message_id == uid) {
            let unread = !self.uplink[i].header().confirmed;
            self.uplink.remove(i);
            self.events.push(AocEvent::Delete(uid));
            if unread {
                self.update_message_count();
            }
        } else if let Some(i) = self.downlink.iter().position(|m| m.header().unique_message_id == uid) {
            self.downlink.remove(i);
            self.events.push(AocEvent::Delete(uid));
        }
    }

    pub fn message_read(&mut self, uid: i64) {
        if let Some(i) = self.uplink.iter().position(|m| m.header().unique_message_id == uid) {
            let unread = !self.uplink[i].header().confirmed;
            self.uplink[i].header_mut().confirmed = true;
            self.events.push(AocEvent::Resynchronize(self.uplink[i].clone()));
            if unread {
                self.update_message_count();
            }
        }
    }

    pub fn ignore_incoming_message(&mut self, uid: i64) {
        if self.uplink.iter().any(|m| m.header().unique_message_id == uid) {
            self.remove_message(uid);
        } else {
            self.blacklisted.push(uid);
        }
    }

    pub fn insert_messages(&mut self, messages: Vec<AnyMessage>) {
        for mut message in messages {
            self.message_counter += 1;
            let header = message.header_mut();
            header.unique_message_id = self.message_counter;
            if let Some(clock) = &self.utc_clock {
                header.timestamp = Timestamp::from_clock(clock);
            }
            if header.direction == MessageDirection::Uplink {
                if let Some(i) = self.blacklisted.iter().position(|uid| *uid == self.message_counter) {
                    self.blacklisted.remove(i);
                } else {
                    self.uplink.insert(0, message.clone());
                    self.update_message_count();
                }
            } else {
                self.downlink.insert(0, message.clone());
            }
            self.events.push(AocEvent::Resynchronize(message));
        }
    }

    pub fn received_freetext(&mut self, message: AnyMessage) {
        self.insert_messages(vec![message]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::FreetextMessage;

    fn uplink(text: &str) -> AnyMessage {
        let mut m = FreetextMessage::new();
        m.header.direction = MessageDirection::Uplink;
        m.header.message = text.into();
        AnyMessage::Freetext(m)
    }

    #[test]
    fn insert_read_remove_updates_count() {
        let mut aoc = Aoc::new();
        aoc.power_up();
        aoc.insert_messages(vec![uplink("A"), uplink("B")]);
        assert_eq!(aoc.uplink_messages()[0].header().message, "B");
        assert_eq!(aoc.uplink_messages()[0].header().unique_message_id, 2);
        let ev = aoc.take_events();
        assert!(ev.contains(&AocEvent::CompanyMessageCount(2)));
        aoc.message_read(1);
        assert!(aoc.take_events().contains(&AocEvent::CompanyMessageCount(1)));
        aoc.message_read(1);
        assert!(!aoc.take_events().iter().any(|e| matches!(e, AocEvent::CompanyMessageCount(_))));
        aoc.remove_message(2);
        let ev = aoc.take_events();
        assert!(ev.contains(&AocEvent::Delete(2)));
        assert!(ev.contains(&AocEvent::CompanyMessageCount(0)));
    }

    #[test]
    fn blacklisted_id_is_dropped_once() {
        let mut aoc = Aoc::new();
        aoc.ignore_incoming_message(1);
        aoc.insert_messages(vec![uplink("A")]);
        assert!(aoc.uplink_messages().is_empty());
        aoc.insert_messages(vec![uplink("B")]);
        assert_eq!(aoc.uplink_messages().len(), 1);
    }

    #[test]
    fn send_inserts_only_on_success() {
        let mut aoc = Aoc::new();
        aoc.send_message(1, uplink("X"));
        assert_eq!(aoc.take_events(), vec![AocEvent::TransmissionResponse { request_id: 1, code: AtsuStatusCodes::ComFailed }]);
        aoc.power_up();
        let mut down = FreetextMessage::new();
        down.header.direction = MessageDirection::Downlink;
        aoc.send_message(2, AnyMessage::Freetext(down.clone()));
        aoc.send_message(3, AnyMessage::Freetext(down));
        aoc.on_send_result(2, AtsuStatusCodes::Ok);
        aoc.on_send_result(3, AtsuStatusCodes::ComFailed);
        assert_eq!(aoc.downlink_messages().len(), 1);
        aoc.power_down();
        assert!(aoc.downlink_messages().is_empty());
        assert_eq!(aoc.weather_allowed(), Err(AtsuStatusCodes::ComFailed));
    }
}

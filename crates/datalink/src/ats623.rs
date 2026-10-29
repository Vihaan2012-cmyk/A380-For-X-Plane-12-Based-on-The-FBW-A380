use crate::atc::{Atc, AtcEvent};
use crate::elements::{uplink_element, ExpectedResponse};
use crate::fans::FansMode;
use crate::message::{AnyMessage, CpdlcMessage, MessageType};

#[derive(Default)]
pub struct Ats623 {
    clearance_request: Option<CpdlcMessage>,
}

fn cleared_to(text: &str, destination: &str, needs_squawk: bool) -> bool {
    let mut search_from = 0;
    let marker = "TO ";
    while let Some(found) = text[search_from..].find(marker) {
        let after = search_from + found + marker.len();
        let mut rest = &text[after..];
        if let Some(stripped) = rest.strip_prefix('@') {
            rest = stripped;
        }
        if let Some(tail) = rest.strip_prefix(destination) {
            if !needs_squawk {
                return true;
            }
            let tail = tail.strip_prefix('@').unwrap_or(tail);
            if tail.contains("SQWK") || tail.contains("SQUAWK") {
                return true;
            }
        }
        search_from = after;
    }
    false
}

impl Ats623 {
    pub fn new() -> Ats623 {
        Ats623::default()
    }

    pub fn clearance_request(&self) -> Option<&CpdlcMessage> {
        self.clearance_request.as_ref()
    }

    pub fn is_relevant_message(&self, message: &AnyMessage) -> bool {
        if matches!(message, AnyMessage::Dcl(_) | AnyMessage::Ocl(_)) {
            return true;
        }
        let station = self.clearance_request.as_ref().map(|c| c.header.station.as_str());
        if Some(message.header().station.as_str()) != station {
            return false;
        }
        if let AnyMessage::Cpdlc(cpdlc) = message {
            let first = cpdlc.content.first().map(|c| c.type_id.as_str());
            return first == Some("UM183") || first == Some("UM169");
        }
        true
    }

    pub fn insert_messages(&mut self, atc: &mut Atc, messages: Vec<AnyMessage>) -> Vec<AtcEvent> {
        let mut handled: Vec<AnyMessage> = Vec::new();
        let mut aoc_ignore: Vec<i64> = Vec::new();

        for message in messages {
            let is_clearance_request = matches!(message, AnyMessage::Dcl(_) | AnyMessage::Ocl(_));
            let kind = message.header().kind;
            let uid = message.header().unique_message_id;

            let mut processed: AnyMessage = match &message {
                AnyMessage::Cpdlc(_) | AnyMessage::Dcl(_) | AnyMessage::Ocl(_) => message.clone(),
                other => {
                    let header = other.header();
                    let mut converted = CpdlcMessage::new();
                    converted.header.unique_message_id = header.unique_message_id;
                    converted.header.com_status = header.com_status;
                    converted.header.confirmed = header.confirmed;
                    converted.header.direction = header.direction;
                    converted.header.timestamp = header.timestamp.clone();
                    converted.header.network = header.network;
                    converted.header.station = header.station.clone();
                    converted.header.message = header.message.clone();
                    converted.mailbox_relevant_message = true;
                    converted.previous_transmission_id = self
                        .clearance_request
                        .as_ref()
                        .map(|c| c.current_transmission_id)
                        .unwrap_or(-1);
                    let id = if atc.fans_mode() == FansMode::FansA { "UM169" } else { "UM183" };
                    let mut element = uplink_element(id).unwrap();
                    element.content[0].value = header.message.clone();
                    element.expected_response = ExpectedResponse::No;
                    converted.content = vec![element];
                    AnyMessage::Cpdlc(converted)
                }
            };

            let destination = atc.inputs.route.destination.as_ref().map(|d| d.ident.clone());
            if is_clearance_request {
                if let AnyMessage::Dcl(m) = &message {
                    self.clearance_request = Some(m.cpdlc.clone());
                } else if let AnyMessage::Ocl(m) = &message {
                    self.clearance_request = Some(m.cpdlc.clone());
                }
            } else if let Some(destination) = destination {
                let cpdlc = match &mut processed {
                    AnyMessage::Cpdlc(m) => m,
                    _ => unreachable!(),
                };
                cpdlc.close_automatically = false;
                let text = cpdlc.header.message.clone();
                let departure_clearance = self.clearance_request.as_ref().is_some_and(|c| c.header.kind == MessageType::Dcl);
                if departure_clearance {
                    if cleared_to(&text, &destination, true) {
                        if cpdlc.content[0].expected_response == ExpectedResponse::No {
                            cpdlc.content[0].expected_response = ExpectedResponse::Roger;
                        }
                        self.clearance_request = None;
                    } else if text.contains("VIA TELEX") {
                        cpdlc.mailbox_relevant_message = false;
                    }
                } else if cleared_to(&text, &destination, false) {
                    if cpdlc.content[0].expected_response == ExpectedResponse::No {
                        cpdlc.content[0].expected_response = ExpectedResponse::Roger;
                    }
                    self.clearance_request = None;
                }
            }

            handled.push(processed);
            if (kind as u8) <= (MessageType::Aoc as u8) {
                aoc_ignore.push(uid);
            }
        }

        for id in aoc_ignore {
            atc.push_event(AtcEvent::AocIgnoreMessageId(id));
        }
        atc.insert_messages(handled);
        atc.take_events()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inputs::AtcInputs;
    use crate::message::{DclMessage, FreetextMessage, MessageDirection};
    use crate::status::{Clock, Waypoint};

    fn atc() -> Atc {
        let clock = Clock { year: 2026, month: 10, day_of_month: 22, hour: 12, minute: 0, second: 0, seconds_of_day: 43200.0 };
        let mut atc = Atc::new(AtcInputs::new(clock));
        atc.power_up(0.0);
        atc.inputs.route.destination = Some(Waypoint { ident: "EGLL".into(), altitude: 0.0, utc: 0.0 });
        atc
    }

    fn freetext(text: &str, station: &str) -> AnyMessage {
        let mut m = FreetextMessage::new();
        m.header.station = station.into();
        m.header.message = text.into();
        m.header.direction = MessageDirection::Uplink;
        AnyMessage::Freetext(m)
    }

    #[test]
    fn dcl_run_ends_with_destination_and_squawk() {
        let mut atc = atc();
        let mut ats = Ats623::new();
        let mut dcl = DclMessage::new();
        dcl.cpdlc.header.station = "EDDF".into();
        dcl.cpdlc.current_transmission_id = 4;
        assert!(ats.is_relevant_message(&AnyMessage::Dcl(dcl.clone())));
        atc.take_events();
        ats.insert_messages(&mut atc, vec![AnyMessage::Dcl(dcl)]);
        assert!(ats.clearance_request().is_some());

        assert!(ats.is_relevant_message(&freetext("CLRD TO EGLL", "EDDF")));
        assert!(!ats.is_relevant_message(&freetext("HELLO", "LFPG")));

        ats.insert_messages(&mut atc, vec![freetext("CLEARANCE DELIVERED VIA TELEX", "EDDF")]);
        assert!(ats.clearance_request().is_some());
        let queued = atc.messages().len();
        assert_eq!(queued, 2);

        ats.insert_messages(&mut atc, vec![freetext("CLRD TO @EGLL@ OFF 25 SQWK 1234", "EDDF")]);
        assert!(ats.clearance_request().is_none());
        match &atc.messages()[0] {
            AnyMessage::Cpdlc(m) => {
                assert_eq!(m.content[0].expected_response, ExpectedResponse::Roger);
                assert_eq!(m.content[0].type_id, "UM183");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn telex_is_hidden_from_the_mailbox() {
        let mut atc = atc();
        let mut ats = Ats623::new();
        let mut dcl = DclMessage::new();
        dcl.cpdlc.header.station = "EDDF".into();
        ats.insert_messages(&mut atc, vec![AnyMessage::Dcl(dcl)]);
        atc.take_events();
        ats.insert_messages(&mut atc, vec![freetext("CLEARANCE DELIVERED VIA TELEX", "EDDF")]);
        match &atc.messages()[0] {
            AnyMessage::Cpdlc(m) => assert!(!m.mailbox_relevant_message),
            _ => panic!(),
        }
    }
}

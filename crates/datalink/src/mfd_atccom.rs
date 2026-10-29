use std::collections::BTreeMap;

use crate::message::{AnyMessage, CpdlcMessage};
use crate::router::LinkTriple;
use crate::status::{AtsuStatusCodes, DatalinkModeCode, DatalinkStatusCode};
use crate::weather::{AtisMessage, AtisType};

pub const SENDING: &str = "SENDING";
pub const SENT: &str = "SENT";
pub const USE_VOICE: &str = "USE<br/>VOICE";
pub const SEND_FAILED: &str = "SEND<br/>FAILED";
pub const NO_REPLY: &str = "NO REPLY";
pub const NO_AUTO_UPDATE: &str = "NO AUTO<br/>UPDATE";
pub const END_OF_UPDATE: &str = "END OF<br/>UPDATE";
pub const ATIS_X_REJECTED: &str = "ATIS X<br/>REJECTED";
pub const GND_SYS_MSG: &str = "GND SYS<br/>MSG >>>";
pub const DATIS_NO_REPLY: &str = "D-ATIS NO REPLY";

#[derive(Clone, Debug, PartialEq)]
pub struct AirportAtis {
    pub icao: String,
    pub kind: AtisType,
    pub requested: bool,
    pub autoupdate: bool,
    pub last_read_atis: Option<String>,
    pub status: Option<String>,
}

impl AirportAtis {
    fn empty(kind: AtisType) -> AirportAtis {
        AirportAtis { icao: String::new(), kind, requested: false, autoupdate: false, last_read_atis: None, status: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtcErrorType {
    EntryOutOfRange,
    FormatError,
    NotInDatabase,
    NotYetImplemented,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AtcErrorMessage {
    pub text: String,
    pub message_text: String,
    pub amber: bool,
    pub type_two: bool,
    pub cleared: bool,
}

#[derive(Debug, Default)]
pub struct MessageStorage {
    pub atis_reports: BTreeMap<String, Vec<AtisMessage>>,
    pub atc_messages_buffer: Vec<AnyMessage>,
    pub atc_monitored_messages: Vec<CpdlcMessage>,
}

impl MessageStorage {
    pub fn set_atis_reports(&mut self, reports: Vec<AtisMessage>) {
        self.atis_reports.clear();
        for m in reports {
            let Some(airport) = m.weather.reports.first().map(|r| r.airport.clone()) else {
                continue;
            };
            self.atis_reports.entry(airport).or_default().push(m);
        }
    }

    pub fn set_monitored_messages(&mut self, messages: Vec<CpdlcMessage>) {
        self.atc_monitored_messages = messages;
    }

    pub fn resynchronize_atc_message(&mut self, message: AnyMessage) {
        let uid = message.header().unique_message_id;
        match self.atc_messages_buffer.iter().position(|m| m.header().unique_message_id == uid) {
            Some(i) => self.atc_messages_buffer[i] = message,
            None => self.atc_messages_buffer.insert(0, message),
        }
    }

    pub fn delete_message(&mut self, uid: i64) -> bool {
        match self.atc_messages_buffer.iter().position(|m| m.header().unique_message_id == uid) {
            Some(i) => {
                self.atc_messages_buffer.remove(i);
                true
            }
            None => false,
        }
    }

    pub fn reset_atc_data(&mut self) {
        self.atis_reports.clear();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AtcDatalinkEvent {
    AtcAtis { index: usize, data: AirportAtis },
    RequestAtis { icao: String, kind: AtisType, request_id: u32 },
    ActivateAtisAutoUpdate { icao: String, kind: AtisType, request_id: u32 },
    DeactivateAtisAutoUpdate { icao: String, request_id: u32 },
    RegisterCpdlcMessages(Vec<AnyMessage>),
    RegisterDclMessages(Vec<AnyMessage>),
    RegisterOclMessages(Vec<AnyMessage>),
    ResetAtisAutoUpdate,
    AtisRequestDone { index: usize, code: AtsuStatusCodes },
    AutoUpdateDone { index: usize, request_id: u32 },
}

const NOT_INSTALLED: LinkTriple<DatalinkStatusCode> = LinkTriple {
    vhf: DatalinkStatusCode::NotInstalled,
    satellite: DatalinkStatusCode::NotInstalled,
    hf: DatalinkStatusCode::NotInstalled,
};
const NO_MODE: LinkTriple<DatalinkModeCode> =
    LinkTriple { vhf: DatalinkModeCode::None, satellite: DatalinkModeCode::None, hf: DatalinkModeCode::None };

#[derive(Debug)]
pub struct AtcDatalinkSystem {
    pub storage: MessageStorage,
    request_id: u32,
    pending_atis: Vec<(u32, usize)>,
    pending_generic: Vec<(u32, usize)>,
    atis_auto_updates: Vec<String>,
    pub atis_reports_print_active: bool,
    pub errors: Vec<AtcErrorMessage>,
    atis_airports: [AirportAtis; 3],
    datalink_status: LinkTriple<DatalinkStatusCode>,
    datalink_mode: LinkTriple<DatalinkModeCode>,
    events: Vec<AtcDatalinkEvent>,
}

impl Default for AtcDatalinkSystem {
    fn default() -> Self {
        AtcDatalinkSystem {
            storage: MessageStorage::default(),
            request_id: 0,
            pending_atis: Vec::new(),
            pending_generic: Vec::new(),
            atis_auto_updates: Vec::new(),
            atis_reports_print_active: false,
            errors: Vec::new(),
            atis_airports: [
                AirportAtis::empty(AtisType::Departure),
                AirportAtis::empty(AtisType::Arrival),
                AirportAtis::empty(AtisType::Arrival),
            ],
            datalink_status: NOT_INSTALLED,
            datalink_mode: NO_MODE,
            events: Vec::new(),
        }
    }
}

impl AtcDatalinkSystem {
    pub fn new() -> AtcDatalinkSystem {
        AtcDatalinkSystem::default()
    }

    pub fn take_events(&mut self) -> Vec<AtcDatalinkEvent> {
        std::mem::take(&mut self.events)
    }

    fn publish_atis(&mut self, index: usize) {
        self.events.push(AtcDatalinkEvent::AtcAtis { index, data: self.atis_airports[index].clone() });
    }

    pub fn on_reset_data(&mut self) {
        self.storage.reset_atc_data();
        self.atis_auto_updates.clear();
        self.atis_reports_print_active = false;
        self.datalink_status = NOT_INSTALLED;
        self.datalink_mode = NO_MODE;
    }

    pub fn on_datalink_status(&mut self, status: LinkTriple<DatalinkStatusCode>) {
        self.datalink_status = status;
    }

    pub fn on_datalink_mode(&mut self, mode: LinkTriple<DatalinkModeCode>) {
        self.datalink_mode = mode;
    }

    pub fn on_system_status(&mut self, code: AtsuStatusCodes) {
        if code == AtsuStatusCodes::NewAtisReceived {
            for i in 0..self.atis_airports.len() {
                self.publish_atis(i);
            }
        }
    }

    pub fn on_active_atis_auto_updates(&mut self, airports: Vec<String>) {
        for i in 0..self.atis_airports.len() {
            let a = &self.atis_airports[i];
            let listed = airports.contains(&a.icao);
            if listed && !a.autoupdate && a.kind != AtisType::Departure {
                self.atis_airports[i].autoupdate = true;
                self.publish_atis(i);
            } else if !listed && a.autoupdate {
                self.atis_airports[i].autoupdate = false;
                self.publish_atis(i);
            }
        }
        self.atis_auto_updates = airports;
    }

    pub fn on_request_atsu_status_code(&mut self, request_id: u32, code: AtsuStatusCodes) {
        let Some(pos) = self.pending_atis.iter().position(|(id, _)| *id == request_id) else {
            return;
        };
        let (_, index) = self.pending_atis.remove(pos);
        match code {
            AtsuStatusCodes::ComFailed => self.atis_airports[index].status = Some(SEND_FAILED.into()),
            AtsuStatusCodes::NoAtisReceived => {
                self.atis_airports[index].status = Some(NO_REPLY.into());
                self.add_message_to_queue(DATIS_NO_REPLY, None, false, false);
            }
            AtsuStatusCodes::NewAtisReceived => self.atis_airports[index].status = Some(String::new()),
            _ => {}
        }
        self.publish_atis(index);
        self.atis_airports[index].requested = false;
        self.events.push(AtcDatalinkEvent::AtisRequestDone { index, code });
    }

    pub fn on_generic_request_response(&mut self, request_id: u32) {
        if let Some(pos) = self.pending_generic.iter().position(|(id, _)| *id == request_id) {
            let (_, index) = self.pending_generic.remove(pos);
            self.events.push(AtcDatalinkEvent::AutoUpdateDone { index, request_id });
        }
    }

    pub fn on_fms_origin(&mut self, icao: Option<&str>) {
        self.init_atis(0, icao);
    }

    pub fn on_fms_destination(&mut self, icao: Option<&str>) {
        self.init_atis(1, icao);
    }

    pub fn on_fms_alternate(&mut self, icao: Option<&str>) {
        self.init_atis(2, icao);
    }

    pub fn on_reset_auto_update(&mut self) {
        self.events.push(AtcDatalinkEvent::ResetAtisAutoUpdate);
    }

    fn init_atis(&mut self, index: usize, icao: Option<&str>) {
        let kind = if index == 0 { AtisType::Departure } else { AtisType::Arrival };
        self.atis_airports[index] = AirportAtis {
            icao: icao.unwrap_or_default().to_string(),
            kind,
            requested: false,
            autoupdate: false,
            last_read_atis: Some(String::new()),
            status: Some(String::new()),
        };
        self.publish_atis(index);
    }

    pub fn show_atc_error_message(&mut self, kind: AtcErrorType, details: Option<&str>) {
        match kind {
            AtcErrorType::EntryOutOfRange => self.add_message_to_queue("ENTRY OUT OF RANGE", details, false, false),
            AtcErrorType::FormatError => self.add_message_to_queue("FORMAT ERROR", details, false, false),
            AtcErrorType::NotInDatabase => self.add_message_to_queue("NOT IN DATABASE", None, false, false),
            AtcErrorType::NotYetImplemented => self.add_message_to_queue("NOT YET IMPLEMENTED", None, false, false),
        }
    }

    pub fn clear_latest_atc_error_message(&mut self) {
        if let Some(i) = self.errors.iter().position(|e| !e.cleared) {
            if self.errors[i].type_two {
                self.errors[i].cleared = true;
            } else {
                self.errors.remove(i);
            }
        }
    }

    pub fn add_message_to_queue(&mut self, text: &str, details: Option<&str>, amber: bool, type_two: bool) {
        let message_text = match details {
            Some(d) if !d.is_empty() => format!("{text}\n{d}"),
            _ => text.to_string(),
        };
        if let Some(i) = self.errors.iter().position(|e| e.message_text == message_text && e.cleared) {
            self.errors.remove(i);
        }
        self.errors.insert(
            0,
            AtcErrorMessage { text: text.to_string(), message_text, amber, type_two, cleared: false },
        );
    }

    fn next_request_id(&mut self) -> u32 {
        let id = self.request_id;
        self.request_id += 1;
        id
    }

    pub fn atis_airports(&self) -> &[AirportAtis; 3] {
        &self.atis_airports
    }

    pub fn set_atis_airport(&mut self, data: AirportAtis, index: usize) {
        self.atis_airports[index] = data;
    }

    pub fn atis_reports(&self, icao: &str) -> &[AtisMessage] {
        self.storage.atis_reports.get(icao).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn request_atis(&mut self, index: usize) {
        if self.atis_airports[index].requested {
            return;
        }
        self.atis_airports[index].requested = true;
        self.atis_airports[index].status = Some(SENDING.into());
        self.publish_atis(index);
        let request_id = self.next_request_id();
        self.pending_atis.push((request_id, index));
        let a = &self.atis_airports[index];
        self.events.push(AtcDatalinkEvent::RequestAtis { icao: a.icao.clone(), kind: a.kind, request_id });
    }

    pub fn update_all_atis(&mut self) {
        for i in 0..self.atis_airports.len() {
            self.request_atis(i);
        }
    }

    pub fn atis_auto_update_active(&self, icao: &str) -> bool {
        self.atis_auto_updates.iter().any(|a| a == icao)
    }

    pub fn activate_atis_auto_update(&mut self, index: usize) -> u32 {
        let request_id = self.next_request_id();
        self.pending_generic.push((request_id, index));
        let a = &self.atis_airports[index];
        self.events.push(AtcDatalinkEvent::ActivateAtisAutoUpdate { icao: a.icao.clone(), kind: a.kind, request_id });
        request_id
    }

    pub fn deactivate_atis_auto_update(&mut self, index: usize) -> u32 {
        let request_id = self.next_request_id();
        self.pending_generic.push((request_id, index));
        let icao = self.atis_airports[index].icao.clone();
        self.events.push(AtcDatalinkEvent::DeactivateAtisAutoUpdate { icao, request_id });
        request_id
    }

    pub fn register_messages(&mut self, messages: Vec<AnyMessage>) {
        let event = match messages.first() {
            Some(AnyMessage::Cpdlc(_)) => AtcDatalinkEvent::RegisterCpdlcMessages(messages),
            Some(AnyMessage::Dcl(_)) => AtcDatalinkEvent::RegisterDclMessages(messages),
            Some(AnyMessage::Ocl(_)) => AtcDatalinkEvent::RegisterOclMessages(messages),
            _ => return,
        };
        self.events.push(event);
    }

    pub fn datalink_status(&self) -> LinkTriple<DatalinkStatusCode> {
        self.datalink_status
    }

    pub fn datalink_mode(&self) -> LinkTriple<DatalinkModeCode> {
        self.datalink_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{DclMessage, MessageType};
    use crate::weather::Report;

    #[test]
    fn atis_request_round_trip() {
        let mut s = AtcDatalinkSystem::new();
        s.on_fms_origin(Some("EGLL"));
        s.on_fms_destination(Some("KJFK"));
        s.take_events();
        s.request_atis(1);
        s.request_atis(1);
        let ev = s.take_events();
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[1], AtcDatalinkEvent::RequestAtis { icao: "KJFK".into(), kind: AtisType::Arrival, request_id: 0 });
        assert_eq!(s.atis_airports()[1].status.as_deref(), Some(SENDING));
        s.on_request_atsu_status_code(0, AtsuStatusCodes::NoAtisReceived);
        assert_eq!(s.atis_airports()[1].status.as_deref(), Some(NO_REPLY));
        assert!(!s.atis_airports()[1].requested);
        assert_eq!(s.errors[0].message_text, DATIS_NO_REPLY);
        s.clear_latest_atc_error_message();
        assert!(s.errors.is_empty());
    }

    #[test]
    fn auto_update_list_flags_arrival_airports_only() {
        let mut s = AtcDatalinkSystem::new();
        s.on_fms_origin(Some("EGLL"));
        s.on_fms_destination(Some("KJFK"));
        s.take_events();
        s.on_active_atis_auto_updates(vec!["EGLL".into(), "KJFK".into()]);
        assert!(!s.atis_airports()[0].autoupdate);
        assert!(s.atis_airports()[1].autoupdate);
        assert!(s.atis_auto_update_active("EGLL"));
        s.on_active_atis_auto_updates(vec![]);
        assert!(!s.atis_airports()[1].autoupdate);
        assert_eq!(s.take_events().len(), 2);
        let id = s.activate_atis_auto_update(1);
        s.on_generic_request_response(id);
        assert!(s.take_events().contains(&AtcDatalinkEvent::AutoUpdateDone { index: 1, request_id: id }));
    }

    #[test]
    fn storage_groups_atis_and_resyncs() {
        let mut st = MessageStorage::default();
        let mut a = AtisMessage::new("EGLL");
        a.weather.reports.push(Report { airport: "EGLL".into(), report: "INFO A".into() });
        let mut b = a.clone();
        b.weather.reports[0].report = "INFO B".into();
        st.set_atis_reports(vec![a, b]);
        assert_eq!(st.atis_reports["EGLL"].len(), 2);

        let mut m = DclMessage::new();
        m.cpdlc.header.unique_message_id = 5;
        st.resynchronize_atc_message(AnyMessage::Dcl(m.clone()));
        m.cpdlc.header.message = "X".into();
        st.resynchronize_atc_message(AnyMessage::Dcl(m));
        assert_eq!(st.atc_messages_buffer.len(), 1);
        assert_eq!(st.atc_messages_buffer[0].header().message, "X");
        assert_eq!(st.atc_messages_buffer[0].header().kind, MessageType::Dcl);
        assert!(st.delete_message(5));
        assert!(!st.delete_message(5));
    }

    #[test]
    fn errors_dedupe_cleared_entries() {
        let mut s = AtcDatalinkSystem::new();
        s.add_message_to_queue("TEST", Some("A"), false, true);
        s.clear_latest_atc_error_message();
        assert!(s.errors[0].cleared);
        s.add_message_to_queue("TEST", Some("A"), false, true);
        assert_eq!(s.errors.len(), 1);
        s.show_atc_error_message(AtcErrorType::FormatError, Some("FL999"));
        assert_eq!(s.errors[0].message_text, "FORMAT ERROR\nFL999");
    }
}

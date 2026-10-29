use std::collections::HashMap;

use crate::convert::{coordinate_to_string, timestamp_to_string};
use crate::elements::{downlink_element, ExpectedResponse, MessageElement};
use crate::fans::{current_fans_mode, FansMode};
use crate::input_checks::{format_scratchpad_vertical_speed, format_scratchpad_wind};
use crate::inputs::{js_number, js_round, AtcInputs};
use crate::interpretation::{
    message_remains_on_mailbox, semantic_answer, semantic_answer_required, REQUEST_MESSAGES,
};
use crate::message::{
    AnyMessage, ComStatus, CpdlcMessage, MessageDirection, MessageNetwork, MessageType, MonitoringState, Timestamp,
};
use crate::monitor::{UplinkMessageMonitoring, UplinkMonitor};
use crate::status::{AtsuStatusCodes, Clock};
use crate::validation::{format_scratchpad_altitude, format_scratchpad_speed};
use crate::weather::{AtisMessage, AtisType};

pub const MAX_MAILBOX_FILE_SIZE: usize = 5;
pub const LOGON_TIMEOUT_S: f64 = 300.0;
pub const WATCHDOG_MS: f64 = 5000.0;
pub const SENT_STATUS_RESET_MS: f64 = 5000.0;
pub const PRINT_STATUS_RESET_MS: f64 = 4500.0;
pub const RECALL_TIMEOUT_MS: f64 = 300_000.0;
pub const HANDOVER_POLL_MS: f64 = 1000.0;
pub const HANDOVER_SETTLE_MS: f64 = 15_000.0;
pub const ATIS_UPDATE_MS: f64 = 60_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i8)]
pub enum MailboxStatusMessage {
    NoMessage = -1,
    AnswerRequired = 0,
    CommunicationFault,
    CommunicationNotAvailable,
    CommunicationNotInitialized,
    MaximumDownlinkMessages,
    LinkLost,
    FlightplanLoadFailed,
    FlightplanLoadPartial,
    FlightplanLoadingUnavailable,
    MonitoringFailed,
    MonitoringLost,
    MonitoringUnavailable,
    NoAtcReply,
    OverflowClosed,
    PrintFailed,
    PriorityMessage,
    SendFailed = 16,
    FlightplanLoadSecondary,
    FlightplanLoadingSecondary,
    FmsDisplayForText,
    FmsDisplayForModification,
    MonitoringCancelled,
    Monitoring,
    NoFmData,
    NoMoreMessages,
    NoMorePages,
    PartialFmgsData,
    Printing,
    RecallMode,
    RecallEmpty,
    Reminder,
    Sending,
    Sent,
    WaitFmData,
}

pub trait AtcLink {
    fn send_message(&mut self, message: &AnyMessage, force: bool) -> AtsuStatusCodes;
    fn receive_atis(&mut self, icao: &str, kind: AtisType) -> (AtsuStatusCodes, Option<AtisMessage>);
}

#[derive(Clone, Debug, PartialEq)]
pub struct StationStatus {
    pub current: String,
    pub next: String,
    pub notification_time: f64,
    pub mode: FansMode,
    pub logon_in_progress: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AtcEvent {
    ResetSystem,
    CpdlcMessages(Vec<AnyMessage>),
    LogonMessage(String),
    SystemStatus(MailboxStatusMessage),
    MessageStatus { uid: i64, status: MailboxStatusMessage },
    MailboxDelete(i64),
    SystemStatusCode(AtsuStatusCodes),
    RmpFrequency(Option<f64>),
    ActivateButton,
    ResetButton,
    AtcRing,
    StationStatus(StationStatus),
    MaxUplinkDelay(i64),
    ActiveAtisAutoUpdates(Vec<String>),
    AtisReports(Vec<AtisMessage>),
    MonitoredMessages(Vec<AnyMessage>),
    AutomaticPositionReport(bool),
    PrintAtisReports(bool),
    PrintMessage(AnyMessage),
    ResynchronizeMessage(AnyMessage),
    DeleteMessage(i64),
    MessageModify(AnyMessage),
    AocIgnoreMessageId(i64),
    HandoverResult(AtsuStatusCodes),
}

#[derive(Clone, Debug, PartialEq)]
struct MailboxMessage {
    id: i64,
    station: String,
    sent: bool,
    read: bool,
    priority: bool,
    status: MailboxStatusMessage,
    direction: MessageDirection,
}

type Block = Vec<MailboxMessage>;

#[derive(Clone, Debug, PartialEq)]
enum Timer {
    ClearSent(i64),
    ClearPrinting(i64),
    LogonTimeout,
    Watchdog,
    Ring,
    HandoverPoll,
    HandoverFinish,
    AtisUpdate(String),
}

#[derive(Clone, Debug)]
struct Scheduled {
    at: f64,
    period: Option<f64>,
    timer: Timer,
}

fn cp(message: &AnyMessage) -> &CpdlcMessage {
    match message {
        AnyMessage::Cpdlc(m) => m,
        AnyMessage::Dcl(m) => &m.cpdlc,
        AnyMessage::Ocl(m) => &m.cpdlc,
        _ => unreachable!("only CPDLC family messages are queued"),
    }
}

fn pack_vhf_com_frequency_to_arinc(hz: f64) -> f64 {
    let f = js_round(hz / 1000.0) as i64;
    let bcd = ((f % 10) << 4)
        + (((f / 10) % 10) << 8)
        + (((f / 100) % 10) << 12)
        + (((f / 1000) % 10) << 16)
        + (((f / 10000) % 10) << 20)
        + (((f / 100000) % 10) << 24);
    ((bcd >> 4) & 0x7ffff) as f64
}

fn cp_mut(message: &mut AnyMessage) -> &mut CpdlcMessage {
    match message {
        AnyMessage::Cpdlc(m) => m,
        AnyMessage::Dcl(m) => &mut m.cpdlc,
        AnyMessage::Ocl(m) => &mut m.cpdlc,
        _ => unreachable!("only CPDLC family messages are queued"),
    }
}

fn is_cpdlc_family(message: &AnyMessage) -> bool {
    matches!(message, AnyMessage::Cpdlc(_) | AnyMessage::Dcl(_) | AnyMessage::Ocl(_))
}

fn find_link_mut(message: &mut CpdlcMessage, transmission_id: i64) -> Option<&mut CpdlcMessage> {
    if message.current_transmission_id == transmission_id {
        return Some(message);
    }
    match message.response.as_mut() {
        Some(next) => find_link_mut(next, transmission_id),
        None => None,
    }
}

fn first_type(message: &CpdlcMessage) -> &str {
    message.content.first().map(|c| c.type_id.as_str()).unwrap_or("")
}

fn slot(message: &mut CpdlcMessage, element: usize, index: usize, value: String) {
    if let Some(entry) = message.content.get_mut(element).and_then(|c| c.content.get_mut(index)) {
        entry.value = value;
    }
}

fn downlink_response(id: &str) -> Option<MessageElement> {
    downlink_element(id)
}

pub struct Atc {
    pub inputs: AtcInputs,
    message_counter: i64,
    cpdlc_message_id: i64,
    powered_up: bool,
    queue: Vec<AnyMessage>,
    monitoring: Option<UplinkMessageMonitoring>,
    current_atc: String,
    next_atc: String,
    notification_time: f64,
    handover_ongoing: bool,
    handover_station: Option<String>,
    print_atis_report: bool,
    atis_auto_update: Vec<(String, AtisType)>,
    atis_messages: HashMap<String, (f64, Vec<AtisMessage>)>,
    atis_order: Vec<String>,
    max_uplink_delay: i64,
    fans_mode: FansMode,
    automatic_position_report: bool,
    events: Vec<AtcEvent>,
    timers: Vec<Scheduled>,
    now_ms: f64,
    downlink_blocks: Vec<Block>,
    uplink_blocks: Vec<Block>,
    buffered_downlink: Vec<Block>,
    buffered_uplink: Vec<Block>,
    last_closed: Option<(Block, f64)>,
    recall_message_id: Option<i64>,
    ring_active: bool,
}

impl Atc {
    pub fn new(inputs: AtcInputs) -> Atc {
        Atc {
            inputs,
            message_counter: 0,
            cpdlc_message_id: 0,
            powered_up: false,
            queue: Vec::new(),
            monitoring: None,
            current_atc: String::new(),
            next_atc: String::new(),
            notification_time: 0.0,
            handover_ongoing: false,
            handover_station: None,
            print_atis_report: false,
            atis_auto_update: Vec::new(),
            atis_messages: HashMap::new(),
            atis_order: Vec::new(),
            max_uplink_delay: -1,
            fans_mode: FansMode::FansNone,
            automatic_position_report: false,
            events: Vec::new(),
            timers: Vec::new(),
            now_ms: 0.0,
            downlink_blocks: Vec::new(),
            uplink_blocks: Vec::new(),
            buffered_downlink: Vec::new(),
            buffered_uplink: Vec::new(),
            last_closed: None,
            recall_message_id: None,
            ring_active: false,
        }
    }

    pub fn push_event(&mut self, event: AtcEvent) {
        self.events.push(event);
    }

    pub fn take_events(&mut self) -> Vec<AtcEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn messages(&self) -> &[AnyMessage] {
        &self.queue
    }

    pub fn current_station(&self) -> &str {
        &self.current_atc
    }

    pub fn fans_mode(&self) -> FansMode {
        self.fans_mode
    }

    pub fn is_powered(&self) -> bool {
        self.powered_up
    }

    pub fn automatic_position_report_active(&self) -> bool {
        self.automatic_position_report
    }

    pub fn print_atis_report_active(&self) -> bool {
        self.print_atis_report
    }

    fn schedule(&mut self, delay: f64, period: Option<f64>, timer: Timer) {
        self.timers.push(Scheduled { at: self.now_ms + delay, period, timer });
    }

    fn cancel(&mut self, matcher: impl Fn(&Timer) -> bool) {
        self.timers.retain(|t| !matcher(&t.timer));
    }

    pub fn is_relevant_message(message: &AnyMessage) -> bool {
        let kind = message.header().kind as u8;
        kind > MessageType::Aoc as u8 && kind < MessageType::Atc as u8
    }

    pub fn create_position_report(&self) -> CpdlcMessage {
        let inputs = &self.inputs;
        let mut message = CpdlcMessage::new();
        message.header.station = self.current_atc.clone();
        message.content.push(downlink_element("DM48").unwrap());

        let altitude = inputs.altitude.unwrap_or(0.0);
        let last = inputs.route.last_waypoint.as_ref();
        let ap_on = inputs.ap_active == Some(true);
        let mut target_altitude = String::new();
        let passed_altitude;
        let current_altitude;
        if inputs.baro_std {
            passed_altitude = match last {
                Some(w) => format_scratchpad_altitude(&format!("FL{}", js_number(js_round(w.altitude / 100.0)))),
                None => format_scratchpad_altitude(&format!("FL{}", js_number(js_round(altitude / 100.0)))),
            };
            current_altitude = format_scratchpad_altitude(&format!("FL{}", js_number(js_round(altitude / 100.0))));
            if ap_on {
                if inputs.selected_altitude != altitude {
                    target_altitude = format_scratchpad_altitude(&format!(
                        "FL{}",
                        js_number(js_round(inputs.selected_altitude / 100.0))
                    ));
                } else {
                    target_altitude = current_altitude.clone();
                }
            }
        } else {
            passed_altitude = match last {
                Some(w) => format_scratchpad_altitude(&js_number(w.altitude)),
                None => format_scratchpad_altitude(&js_number(altitude)),
            };
            current_altitude = format_scratchpad_altitude(&js_number(altitude));
            if ap_on {
                if inputs.selected_altitude != 0.0 {
                    target_altitude = format_scratchpad_altitude(&js_number(inputs.selected_altitude));
                } else {
                    target_altitude = current_altitude.clone();
                }
            }
        }

        let extension = |text: String| -> MessageElement {
            let mut element = downlink_element("DM67").unwrap();
            element.content[0].value = text;
            element
        };

        if let Some(w) = last {
            message.content.push(extension(format!("OVHD: {}", w.ident)));
            message
                .content
                .push(extension(format!("AT {}Z/{}", timestamp_to_string(w.utc), passed_altitude)));
        }

        message.content.push(extension(format!(
            "PPOS: {}",
            coordinate_to_string(inputs.latitude.unwrap_or(0.0), inputs.longitude.unwrap_or(0.0), false)
        )));
        message.content.push(extension(format!(
            "AT {}Z/{}",
            timestamp_to_string(inputs.clock.seconds_of_day),
            current_altitude
        )));

        if let Some(w) = inputs.route.active_waypoint.as_ref() {
            message
                .content
                .push(extension(format!("TO: {} AT {}Z", w.ident, timestamp_to_string(w.utc))));
        }
        if let Some(w) = inputs.route.next_waypoint.as_ref() {
            message.content.push(extension(format!("NEXT: {}", w.ident)));
        }

        if let (Some(direction), Some(speed), Some(sat)) =
            (inputs.wind_direction, inputs.wind_speed, inputs.static_air_temperature)
        {
            let wind = format_scratchpad_wind(&format!("{}/{}KT", js_number(direction), js_number(speed)));
            message.content.push(extension(format!("WIND: {} SAT: {}C", wind, js_number(sat))));
        }

        if let Some(w) = inputs.route.destination.as_ref() {
            message
                .content
                .push(extension(format!("DEST ETA: {}Z", timestamp_to_string(w.utc))));
        }

        if ap_on && (inputs.selected_altitude - altitude).abs() >= 500.0 {
            if inputs.selected_altitude > altitude {
                message.content.push(extension(format!("CLIMBING TO: {}", target_altitude)));
            } else {
                message.content.push(extension(format!("DESCENDING TO: {}", target_altitude)));
            }
            let vs = inputs.vertical_speed.unwrap_or(0.0);
            message.content.push(extension(format!(
                "VS: {}",
                format_scratchpad_vertical_speed(&format!("{}FTM", js_number(vs)))
            )));
        }

        let ias = format_scratchpad_speed(&js_number(inputs.computed_airspeed.unwrap_or(0.0)));
        let gs = format_scratchpad_speed(&js_number(inputs.ground_speed.unwrap_or(0.0)));
        message.content.push(extension(format!("SPD: {} GS: {}", ias, gs)));
        message.content.push(extension(format!(
            "HDG: {}\u{b0}TRUE",
            js_number(inputs.heading.unwrap_or(0.0))
        )));
        message
            .content
            .push(extension(format!("TRK: {}\u{b0}", js_number(inputs.track.unwrap_or(0.0)))));
        message
    }

    pub fn power_up(&mut self, now_ms: f64) {
        if self.powered_up {
            return;
        }
        self.now_ms = now_ms;
        self.monitoring = Some(UplinkMessageMonitoring::new());
        self.mailbox_reset();
        self.schedule(WATCHDOG_MS, Some(WATCHDOG_MS), Timer::Watchdog);
        self.recall_message_id = None;
        self.powered_up = true;
        self.events.push(AtcEvent::RmpFrequency(None));
    }

    pub fn power_down(&mut self) {
        if !self.powered_up {
            return;
        }
        self.events.push(AtcEvent::RmpFrequency(None));
        self.timers.clear();
        self.atis_auto_update.clear();
        self.handover_station = None;
        self.handover_ongoing = false;
        self.current_atc.clear();
        self.next_atc.clear();
        self.notification_time = 0.0;
        self.queue.clear();
        self.print_atis_report = false;
        self.atis_messages.clear();
        self.atis_order.clear();
        self.max_uplink_delay = -1;
        self.fans_mode = FansMode::FansNone;
        self.automatic_position_report = false;
        self.monitoring = None;
        self.recall_message_id = None;
        self.ring_active = false;
        self.powered_up = false;
    }

    pub fn new_route_received(&mut self, link: &mut dyn AtcLink, route: crate::inputs::RouteData) {
        let previous = self.inputs.route.last_waypoint.as_ref().map(|w| w.ident.clone());
        let passed = match (&route.last_waypoint, &previous) {
            (Some(new), Some(old)) => &new.ident != old,
            (Some(_), None) => true,
            _ => false,
        };
        let report_due = self.automatic_position_report && !self.current_atc.is_empty() && passed;
        if report_due {
            let mut message = self.create_position_report();
            message.mailbox_relevant_message = false;
            self.inputs.route = route;
            self.send_message(link, AnyMessage::Cpdlc(message));
        } else {
            self.inputs.route = route;
        }
    }

    fn send_station_status(&mut self) {
        let status = StationStatus {
            current: self.current_atc.clone(),
            next: self.next_atc.clone(),
            notification_time: self.notification_time,
            mode: self.fans_mode,
            logon_in_progress: self.logon_in_progress(),
        };
        self.events.push(AtcEvent::StationStatus(status));
    }

    fn logon_in_progress(&self) -> bool {
        !self.next_atc.is_empty()
    }

    pub fn update_shown_message_in_mailbox(&mut self, uid: i64, recall_message: bool) {
        if uid == -1 || recall_message {
            self.events.push(AtcEvent::RmpFrequency(None));
            return;
        }
        let frequency = self
            .queue
            .iter()
            .map(cp)
            .find(|m| m.header.unique_message_id == uid && m.header.direction == MessageDirection::Uplink)
            .and_then(|m| {
                let index = match first_type(m) {
                    "UM117" | "UM120" => 1,
                    "UM118" | "UM119" | "UM121" | "UM122" => 2,
                    _ => return None,
                };
                m.content[0].content.get(index)?.value.parse::<f64>().ok().map(|f| pack_vhf_com_frequency_to_arinc(f * 1_000_000.0))
            });
        self.events.push(AtcEvent::RmpFrequency(frequency));
    }

    pub fn reset_logon(&mut self) {
        self.current_atc.clear();
        self.next_atc.clear();
        self.notification_time = 0.0;
        self.fans_mode = FansMode::FansNone;
        self.events.push(AtcEvent::LogonMessage(String::new()));
        self.send_station_status();
    }

    fn next_cpdlc_id(&mut self) -> i64 {
        self.cpdlc_message_id += 1;
        self.cpdlc_message_id
    }

    pub fn logon(&mut self, link: &mut dyn AtcLink, station: &str) -> AtsuStatusCodes {
        if !self.next_atc.is_empty() && station != self.next_atc {
            return AtsuStatusCodes::SystemBusy;
        }
        if !self.handover_ongoing && !self.current_atc.is_empty() {
            let result = self.logoff(link);
            if result != AtsuStatusCodes::Ok {
                return result;
            }
        }
        self.handover_ongoing = false;

        let mut message = CpdlcMessage::new();
        message.header.station = station.to_string();
        message.current_transmission_id = self.next_cpdlc_id();
        message.header.direction = MessageDirection::Downlink;
        message.content.push(downlink_response("DM9998").unwrap());
        message.header.com_status = ComStatus::Sending;
        message.header.message = "REQUEST LOGON".to_string();
        message.mailbox_relevant_message = false;

        self.next_atc = station.to_string();
        self.insert_messages(vec![AnyMessage::Cpdlc(message)]);
        self.events.push(AtcEvent::LogonMessage(format!("NEXT ATC: {}", station)));
        self.notification_time = self.inputs.clock.seconds_of_day;
        self.send_station_status();
        self.schedule(LOGON_TIMEOUT_S * 1000.0, None, Timer::LogonTimeout);

        let stored = self
            .queue
            .iter()
            .find(|m| cp(m).header.station == station && first_type(cp(m)) == "DM9998")
            .cloned();
        match stored {
            Some(m) => link.send_message(&m, false),
            None => AtsuStatusCodes::Ok,
        }
    }

    fn start_handover(&mut self, station: &str) -> Option<AtsuStatusCodes> {
        if !self.next_atc.is_empty() && station != self.next_atc {
            return Some(AtsuStatusCodes::SystemBusy);
        }
        self.cancel(|t| matches!(t, Timer::HandoverPoll | Timer::HandoverFinish));
        self.handover_station = Some(station.to_string());
        self.schedule(HANDOVER_POLL_MS, Some(HANDOVER_POLL_MS), Timer::HandoverPoll);
        None
    }

    fn logoff_without_reset(&mut self, link: &mut dyn AtcLink) -> AtsuStatusCodes {
        if self.current_atc.is_empty() {
            return AtsuStatusCodes::NoAtc;
        }
        let mut message = CpdlcMessage::new();
        message.header.station = self.current_atc.clone();
        message.current_transmission_id = self.next_cpdlc_id();
        message.header.direction = MessageDirection::Downlink;
        message.content.push(downlink_response("DM9999").unwrap());
        message.header.com_status = ComStatus::Sending;
        message.mailbox_relevant_message = false;

        self.max_uplink_delay = -1;
        self.events.push(AtcEvent::MaxUplinkDelay(self.max_uplink_delay));
        self.insert_messages(vec![AnyMessage::Cpdlc(message.clone())]);
        let stored = self
            .queue
            .iter()
            .find(|m| cp(m).current_transmission_id == message.current_transmission_id)
            .cloned()
            .unwrap_or(AnyMessage::Cpdlc(message));
        link.send_message(&stored, true)
    }

    pub fn logoff(&mut self, link: &mut dyn AtcLink) -> AtsuStatusCodes {
        self.cancel(|t| matches!(t, Timer::HandoverPoll));
        self.handover_station = None;
        let result = self.logoff_without_reset(link);
        self.reset_logon();
        result
    }

    pub fn disconnect(&mut self, link: &mut dyn AtcLink) {
        if !self.current_atc.is_empty() {
            self.logoff(link);
        }
        if !self.next_atc.is_empty() {
            self.reset_logon();
        }
    }

    fn create_cpdlc_response(&mut self, request: &CpdlcMessage, response: i64) -> Option<CpdlcMessage> {
        let element = downlink_response(&format!("DM{}", response))?;
        let mut message = CpdlcMessage::new();
        message.header.direction = MessageDirection::Downlink;
        message.current_transmission_id = self.next_cpdlc_id();
        message.previous_transmission_id = request.current_transmission_id;
        message.header.station = request.header.station.clone();
        message.content.push(element);
        Some(message)
    }

    fn queue_index(&self, uid: i64) -> Option<usize> {
        self.queue.iter().position(|m| cp(m).header.unique_message_id == uid)
    }

    fn schedule_sent_reset(&mut self, uid: i64) {
        self.schedule(SENT_STATUS_RESET_MS, None, Timer::ClearSent(uid));
    }

    pub fn send_response(&mut self, link: &mut dyn AtcLink, uid: i64, response: i64) {
        if !self.powered_up {
            return;
        }
        let index = match self.queue_index(uid) {
            Some(i) => i,
            None => return,
        };
        let request = cp(&self.queue[index]).clone();
        let mut response_message = match self.create_cpdlc_response(&request, response) {
            Some(m) => m,
            None => return,
        };

        if let Some(existing) = request.response.as_ref() {
            if existing.content.first().map(|c| c.type_id.as_str()) == response_message.content.first().map(|c| c.type_id.as_str())
                && matches!(existing.header.com_status, ComStatus::Sending | ComStatus::Sent)
            {
                return;
            }
        }

        response_message.header.com_status = ComStatus::Sending;
        cp_mut(&mut self.queue[index]).response = Some(Box::new(response_message.clone()));
        self.update_message_status(uid, MailboxStatusMessage::Sending);
        self.mailbox_update(uid, false);

        let outgoing = AnyMessage::Cpdlc(response_message);
        let code = link.send_message(&outgoing, false);
        self.finish_response_send(uid, code);
    }

    fn finish_response_send(&mut self, uid: i64, code: AtsuStatusCodes) {
        let index = match self.queue_index(uid) {
            Some(i) => i,
            None => return,
        };
        if code == AtsuStatusCodes::Ok {
            if let Some(r) = cp_mut(&mut self.queue[index]).response.as_mut() {
                r.header.com_status = ComStatus::Sent;
            }
            self.update_message_status(uid, MailboxStatusMessage::Sent);
            self.schedule_sent_reset(uid);
        } else {
            if let Some(r) = cp_mut(&mut self.queue[index]).response.as_mut() {
                r.header.com_status = ComStatus::Failed;
            }
            self.update_message_status(uid, MailboxStatusMessage::SendFailed);
        }
        self.mailbox_update(uid, false);
        let resync = self.queue[index].clone();
        self.events.push(AtcEvent::ResynchronizeMessage(resync));
    }

    pub fn send_existing_response(&mut self, link: &mut dyn AtcLink, uid: i64) {
        if !self.powered_up {
            return;
        }
        let index = match self.queue_index(uid) {
            Some(i) => i,
            None => return,
        };
        let response_status = match cp(&self.queue[index]).response.as_ref() {
            Some(r) => r.header.com_status,
            None => return,
        };
        if matches!(response_status, ComStatus::Sending | ComStatus::Sent) {
            return;
        }
        if cp(&self.queue[index]).response.as_ref().unwrap().current_transmission_id < 0 {
            let id = self.next_cpdlc_id();
            cp_mut(&mut self.queue[index]).response.as_mut().unwrap().current_transmission_id = id;
        }
        cp_mut(&mut self.queue[index]).response.as_mut().unwrap().header.com_status = ComStatus::Sending;
        self.update_message_status(uid, MailboxStatusMessage::Sending);
        self.mailbox_update(uid, false);

        let outgoing = AnyMessage::Cpdlc((**cp(&self.queue[index]).response.as_ref().unwrap()).clone());
        let code = link.send_message(&outgoing, false);
        self.finish_response_send(uid, code);
    }

    pub fn send_message(&mut self, link: &mut dyn AtcLink, message: AnyMessage) -> AtsuStatusCodes {
        if !self.powered_up {
            return AtsuStatusCodes::ComFailed;
        }
        let mut message = message;
        let uid = message.header().unique_message_id;
        let status = message.header().com_status;
        if matches!(status, ComStatus::Sending | ComStatus::Sent) {
            return AtsuStatusCodes::Ok;
        }
        if message.header().station.is_empty() {
            if self.current_atc.is_empty() {
                return AtsuStatusCodes::NoAtc;
            }
            set_station(&mut message, &self.current_atc);
        }
        set_com_status(&mut message, ComStatus::Sending);
        self.sync_queue_entry(&message);
        let relevant = is_cpdlc_family(&message) && cp(&message).mailbox_relevant_message;
        if relevant {
            self.update_message_status(uid, MailboxStatusMessage::Sending);
            self.mailbox_update_with(&message, false);
        }

        let code = link.send_message(&message, false);
        set_com_status(
            &mut message,
            if code == AtsuStatusCodes::Ok { ComStatus::Sent } else { ComStatus::Failed },
        );
        self.sync_queue_entry(&message);

        if relevant {
            self.mailbox_update_with(&message, false);
            self.update_message_status(
                uid,
                if code == AtsuStatusCodes::Ok { MailboxStatusMessage::Sent } else { MailboxStatusMessage::SendFailed },
            );
            if code == AtsuStatusCodes::Ok {
                self.schedule_sent_reset(uid);
            }
        }
        if self.queue_index(uid).is_some() {
            self.events.push(AtcEvent::ResynchronizeMessage(message));
        }
        code
    }

    fn sync_queue_entry(&mut self, message: &AnyMessage) {
        let uid = message.header().unique_message_id;
        if let Some(index) = self.queue_index(uid) {
            let station = message.header().station.clone();
            let status = message.header().com_status;
            let entry = cp_mut(&mut self.queue[index]);
            entry.header.station = station;
            entry.header.com_status = status;
        }
    }

    pub fn monitored_messages(&self) -> Vec<AnyMessage> {
        let ids = self.monitoring.as_ref().map(|m| m.monitored_message_ids()).unwrap_or_default();
        ids.iter()
            .filter_map(|id| self.queue.iter().find(|m| cp(m).header.unique_message_id == *id).cloned())
            .collect()
    }

    fn emit_monitored(&mut self) {
        let list = self.monitored_messages();
        self.events.push(AtcEvent::MonitoredMessages(list));
    }

    pub fn remove_message(&mut self, uid: i64) -> bool {
        match self.queue_index(uid) {
            Some(index) => {
                self.queue.remove(index);
                self.dequeue(uid);
                self.events.push(AtcEvent::DeleteMessage(uid));
                self.dequeue(uid);
                true
            }
            None => false,
        }
    }

    fn send_atis_reports(&mut self) {
        let mut reports = Vec::new();
        for icao in &self.atis_order {
            if let Some((_, list)) = self.atis_messages.get(icao) {
                reports.extend(list.iter().cloned());
            }
        }
        self.events.push(AtcEvent::AtisReports(reports));
    }

    pub fn cleanup_messages(&mut self) {
        let ids: Vec<i64> = self.queue.iter().map(|m| cp(m).header.unique_message_id).collect();
        for id in ids {
            self.events.push(AtcEvent::DeleteMessage(id));
        }
        self.queue.clear();
        self.mailbox_reset();
        self.atis_messages.clear();
        self.atis_order.clear();
        self.send_atis_reports();
    }

    fn analyze_message(&mut self, request: &mut CpdlcMessage, response: Option<&mut CpdlcMessage>) -> bool {
        if request.content.first().map(|c| c.expected_response) == Some(ExpectedResponse::NotRequired) && response.is_none() {
            let id = first_type(request).to_string();
            if id == "UM9999" {
                request.mailbox_relevant_message = false;
                if !self.current_atc.is_empty() {
                    self.events.push(AtcEvent::LogonMessage(request.header.message.clone()));
                }
                return true;
            }
            if id == "UM9995" || request.header.message.contains("TERMINATED") {
                request.mailbox_relevant_message = false;
                self.reset_logon();
                return true;
            }
            if id == "UM9998" {
                let entries: Vec<&str> = request.header.message.split(' ').collect();
                if entries.len() >= 2 {
                    request.mailbox_relevant_message = false;
                    let station = entries[1].replace('@', "");
                    if let Some(code) = self.start_handover(&station) {
                        self.events.push(AtcEvent::HandoverResult(code));
                    }
                    return true;
                }
            }
        }

        if !self.next_atc.is_empty() {
            if let Some(response) = response {
                if first_type(request) == "DM9998" {
                    let response_id = first_type(response).to_string();
                    if response_id == "UM9997" {
                        response.mailbox_relevant_message = false;
                        self.events.push(AtcEvent::LogonMessage(format!("CURRENT ATC UNIT @{}@ CTL", self.next_atc)));
                        self.fans_mode = current_fans_mode(&self.next_atc);
                        self.current_atc = std::mem::take(&mut self.next_atc);
                        self.send_station_status();
                        return true;
                    }
                    if response_id == "UM9996" || response_id == "UM0" {
                        response.mailbox_relevant_message = false;
                        self.reset_logon();
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn insert_messages(&mut self, messages: Vec<AnyMessage>) {
        if !self.powered_up {
            return;
        }
        let mut messages: Vec<AnyMessage> = messages.into_iter().filter(is_cpdlc_family).collect();
        let count = messages.len() as i64;
        if count == 0 {
            return;
        }
        self.message_counter += count * (count - 1);
        let stamp = Timestamp::from_clock(&self.inputs.clock);
        for message in messages.iter_mut() {
            self.message_counter += 1;
            let header = &mut cp_mut(message).header;
            header.unique_message_id = self.message_counter;
            header.timestamp = stamp.clone();
        }

        for index in 0..messages.len() {
            let mut message = cp(&messages[index]).clone();

            let mut concat_messages = true;
            if message.header.direction == MessageDirection::Uplink && !message.content.is_empty() {
                concat_messages = matches!(
                    first_type(&message),
                    "UM0" | "UM1" | "UM3" | "UM4" | "UM5" | "UM9995" | "UM9996" | "UM9997"
                );
            }

            if message.header.direction == MessageDirection::Downlink && message.current_transmission_id == -1 {
                message.current_transmission_id = self.next_cpdlc_id();
            }

            if message.header.direction == MessageDirection::Uplink {
                self.state_initialize(&mut message);
            }

            if concat_messages && message.previous_transmission_id != -1 {
                for q in 0..self.queue.len() {
                    if cp(&self.queue[q]).header.station != message.header.station {
                        continue;
                    }
                    let mut top = cp(&self.queue[q]).clone();
                    let previous = message.previous_transmission_id;
                    let mut matched = false;
                    let mut working = message.clone();
                    if let Some(link) = find_link_mut(&mut top, previous) {
                        let mut request = link.clone();
                        self.analyze_message(&mut request, Some(&mut working));
                        request.response = Some(Box::new(working.clone()));
                        *link = request;
                        matched = true;
                    }
                    if matched {
                        message = working;
                        *cp_mut(&mut self.queue[q]) = top;
                        let resync = self.queue[q].clone();
                        self.events.push(AtcEvent::ResynchronizeMessage(resync));
                    }
                }
            } else {
                self.analyze_message(&mut message, None);
                let mut stored = messages[index].clone();
                *cp_mut(&mut stored) = message.clone();
                self.queue.insert(0, stored.clone());
                self.events.push(AtcEvent::ResynchronizeMessage(stored));
            }
            *cp_mut(&mut messages[index]) = message;
        }

        if cp(&messages[0]).mailbox_relevant_message {
            self.enqueue(messages);
        }
    }

    pub fn update_message(&mut self, message: AnyMessage) {
        let uid = message.header().unique_message_id;
        if let Some(index) = self.queue_index(uid) {
            self.queue[index] = message.clone();
            self.mailbox_update_with(&message, false);
        }
    }

    pub fn message_read(&mut self, uid: i64) -> bool {
        match self.queue_index(uid) {
            Some(index) => {
                if cp(&self.queue[index]).header.direction == MessageDirection::Uplink {
                    cp_mut(&mut self.queue[index]).header.confirmed = true;
                    let resync = self.queue[index].clone();
                    self.events.push(AtcEvent::ResynchronizeMessage(resync));
                }
                true
            }
            None => false,
        }
    }

    fn update_atis(&mut self, link: &mut dyn AtcLink, icao: &str, kind: AtisType, overwrite: bool) -> AtsuStatusCodes {
        let (code, atis) = link.receive_atis(icao, kind);
        if code != AtsuStatusCodes::Ok {
            return code;
        }
        let mut atis = match atis {
            Some(a) => a,
            None => return AtsuStatusCodes::NoAtisReceived,
        };
        atis.weather.header.timestamp = Timestamp::from_clock(&self.inputs.clock);
        atis.parse_information();
        if atis.information.is_empty() {
            return AtsuStatusCodes::NoAtisReceived;
        }

        let mut result = AtsuStatusCodes::Ok;
        let mut printable = false;
        let mut resynchronize = false;
        let seconds = atis.weather.header.timestamp.seconds;

        match self.atis_messages.get_mut(icao) {
            Some((stamp, list)) => {
                if list[0].information != atis.information {
                    list.insert(0, atis.clone());
                    result = AtsuStatusCodes::NewAtisReceived;
                    resynchronize = true;
                    printable = true;
                } else if overwrite {
                    list[0] = atis.clone();
                    result = AtsuStatusCodes::NewAtisReceived;
                    resynchronize = true;
                }
                *stamp = seconds;
            }
            None => {
                self.atis_messages.insert(icao.to_string(), (seconds, vec![atis.clone()]));
                self.atis_order.push(icao.to_string());
                result = AtsuStatusCodes::NewAtisReceived;
                resynchronize = true;
                printable = true;
            }
        }

        if resynchronize {
            self.send_atis_reports();
        }
        if self.print_atis_report && printable {
            self.events.push(AtcEvent::PrintMessage(AnyMessage::Atis(atis)));
        }
        result
    }

    pub fn toggle_print_atis_reports(&mut self) {
        self.print_atis_report = !self.print_atis_report;
        self.events.push(AtcEvent::PrintAtisReports(self.print_atis_report));
    }

    pub fn receive_atis(&mut self, link: &mut dyn AtcLink, icao: &str, kind: AtisType) -> AtsuStatusCodes {
        self.update_atis(link, icao, kind, true)
    }

    pub fn reset_atis_auto_update(&mut self) {
        self.cancel(|t| matches!(t, Timer::AtisUpdate(_)));
        self.atis_auto_update.clear();
        self.events.push(AtcEvent::ActiveAtisAutoUpdates(Vec::new()));
    }

    pub fn atis_auto_update_active(&self, icao: &str) -> bool {
        self.atis_auto_update.iter().any(|(i, _)| i == icao)
    }

    pub fn activate_atis_auto_update(&mut self, icao: &str, kind: AtisType) {
        if !self.atis_auto_update_active(icao) {
            self.atis_auto_update.push((icao.to_string(), kind));
            self.schedule(ATIS_UPDATE_MS, Some(ATIS_UPDATE_MS), Timer::AtisUpdate(icao.to_string()));
            let icaos = self.atis_auto_update.iter().map(|(i, _)| i.clone()).collect();
            self.events.push(AtcEvent::ActiveAtisAutoUpdates(icaos));
        }
    }

    pub fn deactivate_atis_auto_update(&mut self, icao: &str) {
        if let Some(index) = self.atis_auto_update.iter().position(|(i, _)| i == icao) {
            self.atis_auto_update.remove(index);
            let target = icao.to_string();
            self.cancel(|t| matches!(t, Timer::AtisUpdate(i) if *i == target));
            let icaos = self.atis_auto_update.iter().map(|(i, _)| i.clone()).collect();
            self.events.push(AtcEvent::ActiveAtisAutoUpdates(icaos));
        }
    }

    pub fn toggle_automatic_position_report(&mut self) {
        self.automatic_position_report = !self.automatic_position_report;
        self.events.push(AtcEvent::AutomaticPositionReport(self.automatic_position_report));
    }

    pub fn set_max_uplink_delay(&mut self, delay: i64) {
        self.max_uplink_delay = delay;
        self.events.push(AtcEvent::MaxUplinkDelay(delay));
    }

    pub fn max_uplink_delay(&self) -> i64 {
        self.max_uplink_delay
    }

    pub fn tick(&mut self, now_ms: f64, link: &mut dyn AtcLink) {
        self.now_ms = now_ms;
        loop {
            let due = self
                .timers
                .iter()
                .enumerate()
                .filter(|(_, t)| t.at <= now_ms)
                .min_by(|a, b| a.1.at.partial_cmp(&b.1.at).unwrap())
                .map(|(i, _)| i);
            let index = match due {
                Some(i) => i,
                None => break,
            };
            let scheduled = self.timers[index].clone();
            match scheduled.period {
                Some(period) => self.timers[index].at += period,
                None => {
                    self.timers.remove(index);
                }
            }
            self.fire(scheduled.timer, link);
        }
    }

    fn fire(&mut self, timer: Timer, link: &mut dyn AtcLink) {
        match timer {
            Timer::ClearSent(uid) => {
                if self.current_message_status(uid) == MailboxStatusMessage::Sent {
                    self.update_message_status(uid, MailboxStatusMessage::NoMessage);
                }
            }
            Timer::ClearPrinting(uid) => {
                if self.current_message_status(uid) == MailboxStatusMessage::Printing {
                    self.events.push(AtcEvent::SystemStatus(MailboxStatusMessage::NoMessage));
                }
            }
            Timer::LogonTimeout => {
                if self.logon_in_progress() {
                    let delta = self.inputs.clock.seconds_of_day - self.notification_time;
                    if delta >= LOGON_TIMEOUT_S {
                        self.reset_logon();
                    }
                }
            }
            Timer::Watchdog => self.watchdog(),
            Timer::Ring => self.events.push(AtcEvent::AtcRing),
            Timer::HandoverPoll => {
                let station = self.current_atc.clone();
                if !self.open_messages_for_station(&station) {
                    self.cancel(|t| matches!(t, Timer::HandoverPoll));
                    self.schedule(HANDOVER_SETTLE_MS, None, Timer::HandoverFinish);
                }
            }
            Timer::HandoverFinish => {
                let station = match self.handover_station.take() {
                    Some(s) => s,
                    None => return,
                };
                if !self.current_atc.is_empty() {
                    let code = self.logoff_without_reset(link);
                    if code != AtsuStatusCodes::Ok {
                        self.events.push(AtcEvent::HandoverResult(code));
                    }
                }
                self.handover_ongoing = true;
                let code = self.logon(link, &station);
                self.events.push(AtcEvent::HandoverResult(code));
            }
            Timer::AtisUpdate(icao) => {
                let kind = self
                    .atis_auto_update
                    .iter()
                    .find(|(i, _)| *i == icao)
                    .map(|(_, k)| *k)
                    .unwrap_or(AtisType::Arrival);
                let had = self.atis_messages.contains_key(&icao);
                let code = self.update_atis(link, &icao, kind, false);
                if code == AtsuStatusCodes::Ok {
                    if had {
                        let now = self.inputs.clock.seconds_of_day;
                        if let Some(entry) = self.atis_messages.get_mut(&icao) {
                            entry.0 = now;
                        }
                    }
                } else {
                    self.events.push(AtcEvent::SystemStatusCode(code));
                }
            }
        }
    }

    fn watchdog(&mut self) {
        let ids = match self.monitoring.as_mut() {
            Some(monitoring) => {
                let messages: Vec<CpdlcMessage> = self.queue.iter().map(|m| cp(m).clone()).collect();
                let altitude = self.inputs.altitude.unwrap_or(0.0);
                monitoring.check_message_conditions(
                    &messages,
                    &self.inputs.clock,
                    altitude,
                    self.inputs.route.last_waypoint.as_ref(),
                )
            }
            None => return,
        };
        for id in ids {
            if let Some(index) = self.queue_index(id) {
                self.state_update_at(index, false, true);
                let message = self.queue[index].clone();
                self.mailbox_update_with(&message, true);
            }
        }
    }

    pub fn state_initialize(&self, message: &mut CpdlcMessage) {
        message.close_automatically = !message_remains_on_mailbox(message);
        if UplinkMonitor::relevant_message(message) {
            message.message_monitoring = MonitoringState::Required;
            message.semantic_response_required = false;
        } else {
            message.message_monitoring = MonitoringState::Ignored;
            message.semantic_response_required = semantic_answer_required(message);
            if message.semantic_response_required {
                self.append_semantic_answer(true, message);
            }
        }
    }

    pub fn state_update_at(&mut self, index: usize, ui_event: bool, positive: bool) {
        let mut message = cp(&self.queue[index]).clone();
        let mut monitored_changed = false;
        if positive {
            if message.message_monitoring == MonitoringState::Required {
                message.message_monitoring = MonitoringState::Monitoring;
                if let Some(m) = self.monitoring.as_mut() {
                    m.monitor_message(&message);
                }
                monitored_changed = true;
            } else if !ui_event && message.message_monitoring == MonitoringState::Monitoring {
                message.message_monitoring = MonitoringState::Finished;
                message.reminder_timestamp = Some(Timestamp::from_clock(&self.inputs.clock));
                message.semantic_response_required = semantic_answer_required(&message);
            }
        } else if message.message_monitoring == MonitoringState::Monitoring {
            let sending_or_sent = message
                .response
                .as_ref()
                .is_some_and(|r| matches!(r.header.com_status, ComStatus::Sending | ComStatus::Sent));
            message.message_monitoring =
                if sending_or_sent { MonitoringState::Cancelled } else { MonitoringState::Required };
            if let Some(m) = self.monitoring.as_mut() {
                m.remove_message(message.header.unique_message_id);
            }
            monitored_changed = true;
        }

        if message.semantic_response_required {
            self.append_semantic_answer(positive, &mut message);
        }
        *cp_mut(&mut self.queue[index]) = message;
        if monitored_changed {
            self.emit_monitored();
        }
    }

    pub fn append_semantic_answer(&self, positive: bool, message: &mut CpdlcMessage) -> bool {
        let id = first_type(message).to_string();
        if id == "UM143" {
            for queued in &self.queue {
                let request = cp(queued);
                if REQUEST_MESSAGES.contains(&first_type(request)) {
                    let mut response = CpdlcMessage::new();
                    response.header.station = request.header.station.clone();
                    response.previous_transmission_id = message.current_transmission_id;
                    response.content = request.content.clone();
                    message.response = Some(Box::new(response));
                    return true;
                }
            }
            if message.response.is_none() {
                let mut response = CpdlcMessage::new();
                response.header.station = message.header.station.clone();
                response.previous_transmission_id = message.current_transmission_id;
                let mut element = downlink_element("DM67").unwrap();
                element.content[0].value = "NO REQUEST TRANSMITTED".to_string();
                response.content.push(element);
                message.response = Some(Box::new(response));
            }
        } else if let Some(entry) = semantic_answer(&id) {
            if entry.positive_or_negative {
                let mut response = CpdlcMessage::new();
                response.header.station = message.header.station.clone();
                response.previous_transmission_id = message.current_transmission_id;
                let chosen = if positive { entry.messages[0] } else { entry.messages[1] };
                response.content.push(downlink_element(chosen).unwrap());
                message.response = Some(Box::new(response));
            } else if let Some(element) = downlink_element(entry.messages[0]) {
                let mut response = CpdlcMessage::new();
                response.header.station = message.header.station.clone();
                response.previous_transmission_id = message.current_transmission_id;
                response.content.push(element);
                message.response = Some(Box::new(response));
            }
        }

        if !self.fill_present_data(message) && !self.fill_assigned_data(message) {
            if !self.fill_position_report_related_data(message) {
                fill_reporting_related_data(message);
            }
        }
        false
    }

    fn fill_present_data(&self, message: &mut CpdlcMessage) -> bool {
        let inputs = &self.inputs;
        let id = first_type(message).to_string();
        let value: Option<String> = match id.as_str() {
            "UM132" => match (inputs.latitude, inputs.longitude) {
                (Some(lat), Some(lon)) => Some(coordinate_to_string(lat, lon, false)),
                _ => None,
            },
            "UM133" => inputs
                .altitude
                .map(|a| format_scratchpad_altitude(&js_number(js_round(a / 100.0)))),
            "UM134" => inputs
                .computed_airspeed
                .map(|a| format_scratchpad_speed(&js_number(js_round(a)))),
            "UM144" => Some(format!("{:0>4}", inputs.transponder_code)),
            "UM145" => inputs.heading.map(|a| format_scratchpad_speed(&js_number(js_round(a)))),
            "UM146" => inputs.track.map(|a| format_scratchpad_speed(&js_number(js_round(a)))),
            "UM228" => inputs
                .route
                .destination
                .as_ref()
                .map(|d| format!("{}Z", timestamp_to_string(d.utc))),
            _ => return false,
        };
        match value {
            Some(v) => {
                set_response_slot(message, 0, v);
                true
            }
            None => false,
        }
    }

    fn fill_assigned_data(&self, message: &mut CpdlcMessage) -> bool {
        let inputs = &self.inputs;
        match first_type(message) {
            "UM135" => {
                let value = format_scratchpad_altitude(&js_number(js_round(js_round(inputs.selected_altitude) / 100.0)));
                set_response_slot(message, 0, value);
                true
            }
            "UM136" => {
                if inputs.autothrust_active != Some(true) {
                    return false;
                }
                let content = if inputs.mach_mode {
                    format_scratchpad_altitude(&js_number(inputs.selected_mach.unwrap_or(0.0)))
                } else {
                    format_scratchpad_altitude(&js_number(js_round(inputs.selected_speed.unwrap_or(0.0))))
                };
                set_response_slot(message, 0, content);
                true
            }
            _ => false,
        }
    }

    fn fill_position_report_related_data(&self, message: &mut CpdlcMessage) -> bool {
        let route = &self.inputs.route;
        let id = first_type(message).to_string();
        match id.as_str() {
            "UM138" => {
                if let Some(w) = route.last_waypoint.as_ref() {
                    set_response_slot(message, 0, format!("{}Z", timestamp_to_string(w.utc)));
                }
                true
            }
            "UM139" => {
                if let Some(w) = route.last_waypoint.as_ref() {
                    set_response_slot(message, 0, w.ident.clone());
                }
                true
            }
            "UM140" => {
                if let Some(w) = route.active_waypoint.as_ref() {
                    set_response_slot(message, 0, w.ident.clone());
                }
                true
            }
            "UM141" => {
                if let Some(w) = route.active_waypoint.as_ref() {
                    set_response_slot(message, 0, format!("{}Z", timestamp_to_string(w.utc)));
                }
                true
            }
            "UM142" => {
                if let Some(w) = route.next_waypoint.as_ref() {
                    set_response_slot(message, 0, w.ident.clone());
                }
                true
            }
            "UM147" => {
                message.response = Some(Box::new(self.create_position_report()));
                true
            }
            "UM148" | "UM151" => {
                let value = message.content[0].content[0].value.clone();
                set_response_slot(message, 0, value);
                true
            }
            "UM152" => {
                let first = message.content[0].content[0].value.clone();
                let second = message.content[0].content[1].value.clone();
                set_response_slot(message, 0, first);
                set_response_slot(message, 1, second);
                true
            }
            "UM228" => {
                if let Some(d) = route.destination.as_ref() {
                    set_response_slot(message, 0, d.ident.clone());
                    set_response_slot(message, 1, format!("{}Z", timestamp_to_string(d.utc)));
                }
                true
            }
            _ => false,
        }
    }
}

fn set_response_slot(message: &mut CpdlcMessage, index: usize, value: String) {
    if let Some(response) = message.response.as_mut() {
        slot(response, 0, index, value);
    }
}

fn fill_reporting_related_data(message: &mut CpdlcMessage) -> bool {
    match first_type(message) {
        "UM128" | "UM129" | "UM130" | "UM175" => {
            let value = message.content[0].content[0].value.clone();
            set_response_slot(message, 0, value);
            true
        }
        "UM180" => {
            let values: Vec<String> = message.content[0].content.iter().map(|c| c.value.clone()).collect();
            let count = message
                .response
                .as_ref()
                .and_then(|r| r.content.first())
                .map(|c| c.content.len())
                .unwrap_or(0);
            for i in 0..count.min(values.len()) {
                set_response_slot(message, i, values[i].clone());
            }
            true
        }
        _ => false,
    }
}

fn set_station(message: &mut AnyMessage, station: &str) {
    match message {
        AnyMessage::Cpdlc(m) => m.header.station = station.to_string(),
        AnyMessage::Dcl(m) => m.cpdlc.header.station = station.to_string(),
        AnyMessage::Ocl(m) => m.cpdlc.header.station = station.to_string(),
        _ => {}
    }
}

fn set_com_status(message: &mut AnyMessage, status: ComStatus) {
    match message {
        AnyMessage::Cpdlc(m) => m.header.com_status = status,
        AnyMessage::Dcl(m) => m.cpdlc.header.com_status = status,
        AnyMessage::Ocl(m) => m.cpdlc.header.com_status = status,
        _ => {}
    }
}

impl Atc {
    fn mailbox_reset(&mut self) {
        self.recall_message_id = None;
        self.downlink_blocks.clear();
        self.uplink_blocks.clear();
        self.buffered_downlink.clear();
        self.buffered_uplink.clear();
        self.last_closed = None;
        self.events.push(AtcEvent::ResetSystem);
    }

    fn upload(&mut self, messages: Vec<AnyMessage>) {
        if !messages.is_empty() {
            self.events.push(AtcEvent::CpdlcMessages(messages));
        }
    }

    fn block_ids(block: &Block) -> Vec<i64> {
        block.iter().map(|m| m.id).collect()
    }

    fn find_block(blocks: &[Block], uid: i64) -> Option<usize> {
        blocks.iter().position(|b| b[0].id == uid)
    }

    pub fn estimate_ring_interval(&self) -> f64 {
        let mut interval: f64 = 15000.0;
        for block in &self.uplink_blocks {
            if !block[0].read {
                interval = interval.min(if block[0].priority { 5000.0 } else { 15000.0 });
            }
        }
        interval
    }

    fn cleanup_notifications(&mut self) {
        self.events.push(AtcEvent::ResetButton);
        self.cancel(|t| matches!(t, Timer::Ring));
        self.ring_active = false;
    }

    pub fn atc_message_button_pressed(&mut self) {
        self.cleanup_notifications();
    }

    fn validate_notification_condition(&mut self) {
        if !self.uplink_blocks.iter().any(|b| !b[0].read) {
            self.cleanup_notifications();
        }
    }

    fn setup_intervals(&mut self) {
        self.cancel(|t| matches!(t, Timer::Ring));
        self.events.push(AtcEvent::AtcRing);
        let interval = self.estimate_ring_interval();
        self.schedule(interval, Some(interval), Timer::Ring);
        self.ring_active = true;
    }

    pub fn enqueue(&mut self, messages: Vec<AnyMessage>) {
        if messages.is_empty() {
            return;
        }
        let block: Block = messages
            .iter()
            .map(|m| {
                let c = cp(m);
                MailboxMessage {
                    id: c.header.unique_message_id,
                    station: c.header.station.clone(),
                    sent: false,
                    read: c.header.direction == MessageDirection::Downlink,
                    priority: c.content.first().map(|e| e.urgent).unwrap_or(false),
                    status: MailboxStatusMessage::NoMessage,
                    direction: c.header.direction,
                }
            })
            .collect();

        if block[0].direction == MessageDirection::Downlink && self.downlink_blocks.len() < MAX_MAILBOX_FILE_SIZE {
            self.downlink_blocks.push(block);
        } else if block[0].direction == MessageDirection::Uplink && self.uplink_blocks.len() < MAX_MAILBOX_FILE_SIZE {
            self.uplink_blocks.push(block);
            self.events.push(AtcEvent::ActivateButton);
            self.setup_intervals();
        } else {
            if block[0].direction == MessageDirection::Downlink {
                self.buffered_downlink.push(block);
                self.events.push(AtcEvent::SystemStatus(MailboxStatusMessage::MaximumDownlinkMessages));
                self.events.push(AtcEvent::SystemStatusCode(AtsuStatusCodes::MailboxFull));
            } else {
                self.buffered_uplink.push(block);
                self.events.push(AtcEvent::SystemStatus(MailboxStatusMessage::AnswerRequired));
            }
            return;
        }
        self.upload(messages);
    }

    fn mailbox_update(&mut self, uid: i64, insert_if_needed: bool) {
        if let Some(index) = self.queue_index(uid) {
            let message = self.queue[index].clone();
            self.mailbox_update_with(&message, insert_if_needed);
        }
    }

    fn mailbox_update_with(&mut self, message: &AnyMessage, insert_if_needed: bool) {
        let uid = message.header().unique_message_id;
        let block_ids = if let Some(i) = Self::find_block(&self.uplink_blocks, uid) {
            Some(Self::block_ids(&self.uplink_blocks[i]))
        } else {
            Self::find_block(&self.downlink_blocks, uid).map(|i| Self::block_ids(&self.downlink_blocks[i]))
        };
        match block_ids {
            Some(ids) => {
                let mut list = Vec::new();
                for id in ids {
                    if id == uid {
                        list.push(message.clone());
                    } else if let Some(i) = self.queue_index(id) {
                        list.push(self.queue[i].clone());
                    }
                }
                self.upload(list);
            }
            None => {
                if insert_if_needed {
                    self.enqueue(vec![message.clone()]);
                }
            }
        }
    }

    fn dequeue(&mut self, uid: i64) {
        let ids = if let Some(i) = Self::find_block(&self.uplink_blocks, uid) {
            Some(Self::block_ids(&self.uplink_blocks[i]))
        } else {
            Self::find_block(&self.downlink_blocks, uid).map(|i| Self::block_ids(&self.downlink_blocks[i]))
        };
        if let Some(ids) = ids {
            self.events.push(AtcEvent::MailboxDelete(uid));
            for id in ids {
                self.remove_message(id);
            }
        }
    }

    pub fn update_message_status(&mut self, uid: i64, status: MailboxStatusMessage) {
        if let Some(i) = Self::find_block(&self.uplink_blocks, uid) {
            self.uplink_blocks[i][0].status = status;
            self.events.push(AtcEvent::MessageStatus { uid, status });
            return;
        }
        if let Some(i) = Self::find_block(&self.downlink_blocks, uid) {
            self.downlink_blocks[i][0].status = status;
            self.events.push(AtcEvent::MessageStatus { uid, status });
        }
    }

    pub fn current_message_status(&self, uid: i64) -> MailboxStatusMessage {
        if let Some(i) = Self::find_block(&self.uplink_blocks, uid) {
            return self.uplink_blocks[i][0].status;
        }
        if let Some(i) = Self::find_block(&self.downlink_blocks, uid) {
            return self.downlink_blocks[i][0].status;
        }
        MailboxStatusMessage::NoMessage
    }

    pub fn open_messages_for_station(&self, station: &str) -> bool {
        [&self.uplink_blocks, &self.downlink_blocks, &self.buffered_uplink, &self.buffered_downlink]
            .iter()
            .any(|blocks| blocks.iter().any(|b| !b[0].sent && b[0].station == station))
    }

    pub fn mailbox_delete_message(&mut self, uid: i64) {
        let ids = if let Some(i) = Self::find_block(&self.uplink_blocks, uid) {
            Some(Self::block_ids(&self.uplink_blocks[i]))
        } else {
            Self::find_block(&self.downlink_blocks, uid).map(|i| Self::block_ids(&self.downlink_blocks[i]))
        };
        if let Some(ids) = ids {
            for id in ids {
                self.remove_message(id);
            }
        }
    }

    pub fn mailbox_uplink_response(&mut self, link: &mut dyn AtcLink, uid: i64, response_id: i64) {
        if !self.powered_up {
            return;
        }
        if let Some(i) = Self::find_block(&self.uplink_blocks, uid) {
            let ids = Self::block_ids(&self.uplink_blocks[i]);
            for id in ids.into_iter().rev() {
                self.send_response(link, id, response_id);
            }
        }
    }

    pub fn mailbox_downlink_transmit(&mut self, link: &mut dyn AtcLink, uid: i64) {
        if !self.powered_up {
            return;
        }
        if let Some(i) = Self::find_block(&self.downlink_blocks, uid) {
            let ids = Self::block_ids(&self.downlink_blocks[i]);
            for id in ids.into_iter().rev() {
                if let Some(index) = self.queue_index(id) {
                    let message = self.queue[index].clone();
                    if cp(&message).header.direction == MessageDirection::Downlink {
                        let code = self.send_message(link, message);
                        if code != AtsuStatusCodes::Ok {
                            self.events.push(AtcEvent::SystemStatusCode(code));
                        }
                    }
                }
            }
            return;
        }
        if Self::find_block(&self.uplink_blocks, uid).is_some() {
            if let Some(index) = self.queue_index(uid) {
                let message = cp(&self.queue[index]);
                if message.response.is_some() && message.semantic_response_required {
                    self.send_existing_response(link, uid);
                }
            }
        }
    }

    pub fn mailbox_modify_message(&mut self, uid: i64) {
        if !self.powered_up || Self::find_block(&self.uplink_blocks, uid).is_none() {
            return;
        }
        if let Some(index) = self.queue_index(uid) {
            let message = self.queue[index].clone();
            self.events.push(AtcEvent::MessageModify(message));
        }
    }

    pub fn mailbox_print_message(&mut self, uid: i64) {
        if !self.powered_up {
            return;
        }
        if let Some(index) = self.queue_index(uid) {
            let message = self.queue[index].clone();
            self.events.push(AtcEvent::SystemStatus(MailboxStatusMessage::Printing));
            self.events.push(AtcEvent::PrintMessage(message));
            self.schedule(PRINT_STATUS_RESET_MS, None, Timer::ClearPrinting(uid));
        }
    }

    fn close_block(&mut self, uplink: bool, uid: i64) -> bool {
        let index = match Self::find_block(if uplink { &self.uplink_blocks } else { &self.downlink_blocks }, uid) {
            Some(i) => i,
            None => return false,
        };
        let block = if uplink { self.uplink_blocks.remove(index) } else { self.downlink_blocks.remove(index) };
        let exists = self.queue_index(uid).is_some();
        let already = self.last_closed.as_ref().is_some_and(|(b, _)| b[0].id == uid);
        if !already && exists {
            self.last_closed = Some((block, self.now_ms));
        }
        if uplink {
            self.validate_notification_condition();
        }

        loop {
            let (len, backlog_len) = if uplink {
                (self.uplink_blocks.len(), self.buffered_uplink.len())
            } else {
                (self.downlink_blocks.len(), self.buffered_downlink.len())
            };
            if backlog_len == 0 || len == MAX_MAILBOX_FILE_SIZE {
                break;
            }
            let buffered = if uplink { self.buffered_uplink.remove(0) } else { self.buffered_downlink.remove(0) };
            let mut kept: Block = Vec::new();
            let mut mailbox_messages = Vec::new();
            for data in buffered {
                if let Some(qi) = self.queue_index(data.id) {
                    let unread = !data.read;
                    kept.push(data);
                    if unread {
                        self.setup_intervals();
                    }
                    if cp(&self.queue[qi]).mailbox_relevant_message {
                        mailbox_messages.push(self.queue[qi].clone());
                    }
                }
            }
            if uplink {
                self.uplink_blocks.push(kept);
            } else {
                self.downlink_blocks.push(kept);
            }
            self.upload(mailbox_messages);
        }
        true
    }

    pub fn mailbox_close_message(&mut self, uid: i64) {
        if !self.close_block(true, uid) {
            self.close_block(false, uid);
        }
    }

    pub fn mailbox_update_message_monitoring(&mut self, uid: i64) {
        if !self.powered_up {
            return;
        }
        if let Some(index) = self.queue_index(uid) {
            self.state_update_at(index, true, true);
            self.mailbox_update(uid, false);
        }
    }

    pub fn mailbox_stop_message_monitoring(&mut self, uid: i64) {
        if let Some(index) = self.queue_index(uid) {
            self.state_update_at(index, true, false);
            self.mailbox_update(uid, false);
        }
    }

    pub fn mailbox_invert_semantic_response(&mut self, uid: i64) {
        if !self.powered_up {
            return;
        }
        if let Some(index) = self.queue_index(uid) {
            self.state_update_at(index, true, false);
            let message = self.queue[index].clone();
            self.upload(vec![message]);
        }
    }

    pub fn mailbox_recall_message(&mut self) {
        if !self.powered_up {
            return;
        }
        match self.last_closed.take() {
            None => self.events.push(AtcEvent::SystemStatus(MailboxStatusMessage::RecallEmpty)),
            Some((block, stamp)) => {
                if self.now_ms - stamp > RECALL_TIMEOUT_MS {
                    self.events.push(AtcEvent::SystemStatus(MailboxStatusMessage::RecallEmpty));
                    return;
                }
                let mut messages = Vec::new();
                for entry in &block {
                    if let Some(i) = self.queue_index(entry.id) {
                        messages.push(self.queue[i].clone());
                    }
                }
                if messages.is_empty() {
                    self.last_closed = Some((block, stamp));
                    return;
                }
                cp_mut(&mut messages[0]).close_automatically = false;
                let first = cp(&messages[0]).header.unique_message_id;
                if let Some(i) = self.queue_index(first) {
                    cp_mut(&mut self.queue[i]).close_automatically = false;
                }
                self.recall_message_id = Some(first);
                self.upload(messages);
                if block[0].direction == MessageDirection::Downlink {
                    self.downlink_blocks.push(block.clone());
                } else {
                    self.uplink_blocks.push(block.clone());
                }
                self.last_closed = Some((block, stamp));
                self.update_message_status(first, MailboxStatusMessage::RecallMode);
            }
        }
    }

    pub fn mailbox_read_message(&mut self, uid: i64) {
        if !self.powered_up {
            return;
        }
        if let Some(i) = Self::find_block(&self.uplink_blocks, uid) {
            self.uplink_blocks[i][0].read = true;
            self.validate_notification_condition();
        }
    }

    pub fn mailbox_visible_message(&mut self, uid: i64) {
        if !self.powered_up {
            return;
        }
        let recall = Some(uid) == self.recall_message_id;
        self.update_shown_message_in_mailbox(uid, recall);
    }

    pub fn clock(&self) -> &Clock {
        &self.inputs.clock
    }

    pub fn network_of(message: &AnyMessage) -> MessageNetwork {
        message.header().network
    }
}

use datalink::atc::{Atc, AtcEvent, AtcLink, MailboxStatusMessage};
use datalink::elements::{downlink_element, uplink_element};
use datalink::fans::FansMode;
use datalink::inputs::AtcInputs;
use datalink::message::{AnyMessage, ComStatus, CpdlcMessage, MessageDirection};
use datalink::status::{AtsuStatusCodes, Clock, Waypoint};
use datalink::weather::{AtisMessage, AtisType, Report};

struct Link {
    sent: Vec<AnyMessage>,
    code: AtsuStatusCodes,
    atis: Vec<String>,
}

impl Link {
    fn new() -> Link {
        Link { sent: Vec::new(), code: AtsuStatusCodes::Ok, atis: Vec::new() }
    }
}

impl AtcLink for Link {
    fn send_message(&mut self, message: &AnyMessage, _force: bool) -> AtsuStatusCodes {
        self.sent.push(message.clone());
        self.code
    }

    fn receive_atis(&mut self, icao: &str, _kind: AtisType) -> (AtsuStatusCodes, Option<AtisMessage>) {
        if self.atis.is_empty() {
            return (AtsuStatusCodes::ComFailed, None);
        }
        let text = self.atis.remove(0);
        let mut atis = AtisMessage::new(icao);
        atis.weather.reports.push(Report { airport: icao.into(), report: text });
        (AtsuStatusCodes::Ok, Some(atis))
    }
}

fn clock(seconds: f64) -> Clock {
    Clock { year: 2026, month: 10, day_of_month: 22, hour: 0, minute: 0, second: 0, seconds_of_day: seconds }
}

fn powered() -> Atc {
    let mut atc = Atc::new(AtcInputs::new(clock(43200.0)));
    atc.power_up(0.0);
    atc.take_events();
    atc
}

fn uplink_message(station: &str, id: &str, values: &[&str], transmission: i64, previous: i64) -> AnyMessage {
    let mut m = CpdlcMessage::new();
    m.header.direction = MessageDirection::Uplink;
    m.header.station = station.into();
    m.current_transmission_id = transmission;
    m.previous_transmission_id = previous;
    let mut element = uplink_element(id).unwrap();
    for (slot, value) in element.content.iter_mut().zip(values) {
        slot.value = value.to_string();
    }
    m.content.push(element);
    AnyMessage::Cpdlc(m)
}

fn logged_on(atc: &mut Atc, link: &mut Link, station: &str) {
    assert_eq!(atc.logon(link, station), AtsuStatusCodes::Ok);
    let request_id = atc.messages().iter().find_map(|m| match m {
        AnyMessage::Cpdlc(c) if c.content[0].type_id == "DM9998" => Some(c.current_transmission_id),
        _ => None,
    });
    atc.insert_messages(vec![uplink_message(station, "UM9997", &[], 90, request_id.unwrap())]);
    assert_eq!(atc.current_station(), station);
}

#[test]
fn logon_sends_the_request_and_accepts_the_ack() {
    let mut atc = powered();
    let mut link = Link::new();
    logged_on(&mut atc, &mut link, "EDMM");
    assert_eq!(atc.fans_mode(), FansMode::FansB);
    match &link.sent[0] {
        AnyMessage::Cpdlc(m) => {
            assert_eq!(m.content[0].type_id, "DM9998");
            assert_eq!(m.header.station, "EDMM");
        }
        other => panic!("{other:?}"),
    }
    let events = atc.take_events();
    assert!(events.iter().any(|e| matches!(e, AtcEvent::LogonMessage(t) if t == "CURRENT ATC UNIT @EDMM@ CTL")));
    assert!(events
        .iter()
        .any(|e| matches!(e, AtcEvent::StationStatus(s) if s.current == "EDMM" && s.next.is_empty())));
}

#[test]
fn logon_denied_resets_the_state() {
    let mut atc = powered();
    let mut link = Link::new();
    atc.logon(&mut link, "EDMM");
    let id = match &atc.messages()[0] {
        AnyMessage::Cpdlc(c) => c.current_transmission_id,
        _ => panic!(),
    };
    atc.insert_messages(vec![uplink_message("EDMM", "UM9996", &[], 91, id)]);
    assert_eq!(atc.current_station(), "");
}

#[test]
fn second_logon_while_one_is_pending_is_busy() {
    let mut atc = powered();
    let mut link = Link::new();
    atc.logon(&mut link, "EDMM");
    assert_eq!(atc.logon(&mut link, "LOWW"), AtsuStatusCodes::SystemBusy);
}

#[test]
fn logon_times_out_after_five_minutes() {
    let mut atc = powered();
    let mut link = Link::new();
    atc.logon(&mut link, "EDMM");
    atc.inputs.clock = clock(43200.0 + 301.0);
    atc.tick(301_000.0, &mut link);
    let events = atc.take_events();
    assert!(events
        .iter()
        .any(|e| matches!(e, AtcEvent::StationStatus(s) if s.next.is_empty() && s.current.is_empty())));
}

#[test]
fn uplink_enters_the_mailbox_and_rings() {
    let mut atc = powered();
    let mut link = Link::new();
    logged_on(&mut atc, &mut link, "EDMM");
    atc.take_events();
    atc.insert_messages(vec![uplink_message("EDMM", "UM20", &["FL350"], 5, -1)]);
    let events = atc.take_events();
    assert!(events.iter().any(|e| matches!(e, AtcEvent::ActivateButton)));
    assert!(events.iter().any(|e| matches!(e, AtcEvent::AtcRing)));
    assert!(events.iter().any(|e| matches!(e, AtcEvent::CpdlcMessages(m) if m.len() == 1)));
    assert!(atc.estimate_ring_interval() <= 15000.0);

    let uid = atc.messages()[0].header().unique_message_id;
    atc.mailbox_read_message(uid);
    assert!(atc.take_events().iter().any(|e| matches!(e, AtcEvent::ResetButton)));
}

#[test]
fn response_is_sent_and_status_cycles() {
    let mut atc = powered();
    let mut link = Link::new();
    logged_on(&mut atc, &mut link, "EDMM");
    atc.insert_messages(vec![uplink_message("EDMM", "UM20", &["FL350"], 5, -1)]);
    let uid = atc.messages()[0].header().unique_message_id;
    atc.take_events();
    link.sent.clear();

    atc.mailbox_uplink_response(&mut link, uid, 0);
    assert_eq!(link.sent.len(), 1);
    match &link.sent[0] {
        AnyMessage::Cpdlc(m) => {
            assert_eq!(m.content[0].type_id, "DM0");
            assert_eq!(m.previous_transmission_id, 5);
        }
        _ => panic!(),
    }
    assert_eq!(atc.current_message_status(uid), MailboxStatusMessage::Sent);
    atc.tick(5_000.0, &mut link);
    assert_eq!(atc.current_message_status(uid), MailboxStatusMessage::NoMessage);

    link.sent.clear();
    atc.mailbox_uplink_response(&mut link, uid, 0);
    assert!(link.sent.is_empty());
}

#[test]
fn failed_send_is_flagged() {
    let mut atc = powered();
    let mut link = Link::new();
    logged_on(&mut atc, &mut link, "EDMM");
    atc.insert_messages(vec![uplink_message("EDMM", "UM20", &["FL350"], 5, -1)]);
    let uid = atc.messages()[0].header().unique_message_id;
    link.code = AtsuStatusCodes::ComFailed;
    atc.mailbox_uplink_response(&mut link, uid, 0);
    assert_eq!(atc.current_message_status(uid), MailboxStatusMessage::SendFailed);
}

#[test]
fn downlink_without_station_needs_a_logon() {
    let mut atc = powered();
    let mut link = Link::new();
    let mut m = CpdlcMessage::new();
    m.content.push(downlink_element("DM6").unwrap());
    assert_eq!(atc.send_message(&mut link, AnyMessage::Cpdlc(m)), AtsuStatusCodes::NoAtc);
}

#[test]
fn semantic_answer_is_filled_from_the_aircraft_state() {
    let mut atc = powered();
    let mut link = Link::new();
    logged_on(&mut atc, &mut link, "EDMM");
    atc.inputs.altitude = Some(35000.0);
    atc.insert_messages(vec![uplink_message("EDMM", "UM133", &[], 6, -1)]);
    match &atc.messages()[0] {
        AnyMessage::Cpdlc(m) => {
            let response = m.response.as_ref().expect("semantic answer prepared");
            assert_eq!(response.content[0].type_id, "DM32");
            assert_eq!(response.content[0].content[0].value, "FL350");
        }
        _ => panic!(),
    }
}

#[test]
fn time_monitored_uplink_runs_through_the_state_machine() {
    let mut atc = powered();
    let mut link = Link::new();
    logged_on(&mut atc, &mut link, "EDMM");
    atc.insert_messages(vec![uplink_message("EDMM", "UM21", &["1201Z"], 7, -1)]);
    let uid = atc.messages()[0].header().unique_message_id;
    atc.mailbox_uplink_response(&mut link, uid, 0);
    atc.mailbox_update_message_monitoring(uid);
    assert_eq!(atc.monitored_messages().len(), 1);

    atc.inputs.clock = clock(43260.0);
    atc.tick(5_000.0, &mut link);
    assert!(atc.monitored_messages().is_empty());
    match &atc.messages()[0] {
        AnyMessage::Cpdlc(m) => assert_eq!(m.message_monitoring as u8, datalink::message::MonitoringState::Finished as u8),
        _ => panic!(),
    }
}

#[test]
fn handover_waits_for_open_messages_then_logs_on_again() {
    let mut atc = powered();
    let mut link = Link::new();
    logged_on(&mut atc, &mut link, "EDMM");
    link.sent.clear();
    atc.take_events();

    let mut handover = match uplink_message("EDMM", "UM9998", &[], 8, -1) {
        AnyMessage::Cpdlc(m) => m,
        _ => panic!(),
    };
    handover.header.message = "NEXT @LOWW@".into();
    atc.insert_messages(vec![AnyMessage::Cpdlc(handover)]);
    atc.tick(1_000.0, &mut link);
    assert!(link.sent.is_empty());
    atc.tick(16_000.0, &mut link);
    let types: Vec<String> = link
        .sent
        .iter()
        .map(|m| match m {
            AnyMessage::Cpdlc(c) => format!("{}:{}", c.content[0].type_id, c.header.station),
            _ => String::new(),
        })
        .collect();
    assert_eq!(types, vec!["DM9999:EDMM".to_string(), "DM9998:LOWW".to_string()]);
    assert!(atc.take_events().iter().any(|e| matches!(e, AtcEvent::HandoverResult(AtsuStatusCodes::Ok))));
}

#[test]
fn position_report_has_the_documented_lines() {
    let mut atc = powered();
    let mut link = Link::new();
    logged_on(&mut atc, &mut link, "EDMM");
    atc.inputs.latitude = Some(50.0);
    atc.inputs.longitude = Some(8.0);
    atc.inputs.altitude = Some(35000.0);
    atc.inputs.heading = Some(90.0);
    atc.inputs.track = Some(91.0);
    atc.inputs.computed_airspeed = Some(280.0);
    atc.inputs.ground_speed = Some(450.0);
    atc.inputs.route.last_waypoint = Some(Waypoint { ident: "ABC".into(), altitude: 34000.0, utc: 43000.0 });
    atc.inputs.route.destination = Some(Waypoint { ident: "EGLL".into(), altitude: 0.0, utc: 50000.0 });
    let report = atc.create_position_report();
    let lines: Vec<&str> = report.content.iter().map(|c| c.content.first().map(|s| s.value.as_str()).unwrap_or("")).collect();
    assert!(lines.contains(&"OVHD: ABC"));
    assert!(lines.contains(&"AT 1156Z/FL340"));
    assert!(lines.iter().any(|l| l.starts_with("PPOS: ")));
    assert!(lines.contains(&"DEST ETA: 1353Z"));
    assert!(lines.contains(&"SPD: 280 GS: 450"));
    assert!(lines.contains(&"HDG: 90\u{b0}TRUE"));
    assert!(lines.contains(&"TRK: 91\u{b0}"));
    assert_eq!(report.content[0].type_id, "DM48");
    assert_eq!(report.header.station, "EDMM");
}

#[test]
fn atis_auto_update_stores_reports_and_dedups() {
    let mut atc = powered();
    let mut link = Link::new();
    link.atis = vec!["EDDF ATIS INFORMATION A".into(), "EDDF ATIS INFORMATION A".into(), "EDDF ATIS INFORMATION B".into()];
    atc.activate_atis_auto_update("EDDF", AtisType::Arrival);
    atc.take_events();
    atc.tick(60_000.0, &mut link);
    let first = atc.take_events();
    assert!(first.iter().any(|e| matches!(e, AtcEvent::AtisReports(r) if r.len() == 1)));
    atc.tick(120_000.0, &mut link);
    assert!(!atc.take_events().iter().any(|e| matches!(e, AtcEvent::AtisReports(_))));
    atc.tick(180_000.0, &mut link);
    assert!(atc.take_events().iter().any(|e| matches!(e, AtcEvent::AtisReports(r) if r.len() == 2)));
    atc.deactivate_atis_auto_update("EDDF");
    assert!(!atc.atis_auto_update_active("EDDF"));
}

#[test]
fn removing_a_message_closes_its_mailbox_block() {
    let mut atc = powered();
    let mut link = Link::new();
    logged_on(&mut atc, &mut link, "EDMM");
    atc.insert_messages(vec![uplink_message("EDMM", "UM20", &["FL350"], 5, -1)]);
    let uid = atc.messages()[0].header().unique_message_id;
    atc.take_events();
    assert!(atc.remove_message(uid));
    let events = atc.take_events();
    assert!(events.iter().any(|e| matches!(e, AtcEvent::MailboxDelete(id) if *id == uid)));
    assert!(atc.messages().iter().all(|m| m.header().unique_message_id != uid));
    atc.mailbox_close_message(uid);
    atc.mailbox_recall_message();
    let recall = atc.take_events();
    assert!(recall.iter().any(|e| matches!(e, AtcEvent::SystemStatus(MailboxStatusMessage::RecallEmpty))));
    let _ = ComStatus::Open;
}

#[test]
fn contact_frequency_goes_to_the_rmp_as_packed_arinc_bcd() {
    let mut atc = powered();
    atc.insert_messages(vec![uplink_message("EDDF", "UM117", &["EDDF", "121.500"], 5, -1)]);
    let uid = atc.messages()[0].header().unique_message_id;
    atc.take_events();
    atc.update_shown_message_in_mailbox(uid, false);
    assert_eq!(atc.take_events(), vec![AtcEvent::RmpFrequency(Some(0x21500 as f64))]);
    atc.update_shown_message_in_mailbox(uid, true);
    assert_eq!(atc.take_events(), vec![AtcEvent::RmpFrequency(None)]);
}

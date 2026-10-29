use datalink::catalogue::{DOWNLINK, UPLINK};
use datalink::elements::{downlink, downlink_element, uplink, uplink_element, ContentType, ExpectedResponse};
use datalink::fans::{current_fans_mode, FansMode};
use datalink::message::{
    AnyMessage, ComStatus, CpdlcMessage, DclMessage, FreetextMessage, MessageDirection, MessageType, MonitoringState,
    OclMessage, SerializationFormat, Timestamp,
};
use datalink::topics;
use serde_json::json;

fn stamped(mut m: CpdlcMessage, station: &str) -> CpdlcMessage {
    m.header.station = station.into();
    m.header.timestamp = Timestamp {
        year: 2026,
        month: 10,
        day: 22,
        seconds: 45296.0,
    };
    m
}

fn dm6(level: &str) -> CpdlcMessage {
    let mut m = CpdlcMessage::new();
    let mut e = downlink_element("DM6").unwrap();
    e.content[0].value = level.into();
    m.content.push(e);
    m.current_transmission_id = 3;
    stamped(m, "EDMM")
}

#[test]
fn catalogue_sizes_and_unique_ids() {
    assert_eq!(DOWNLINK.len(), 96);
    assert_eq!(UPLINK.len(), 178);
    for table in [DOWNLINK, UPLINK] {
        for (i, a) in table.iter().enumerate() {
            assert!(table[i + 1..].iter().all(|b| b.type_id != a.type_id), "{}", a.type_id);
            assert!(!a.templates.is_empty(), "{}", a.type_id);
        }
    }
    assert!(DOWNLINK.iter().all(|e| e.type_id.starts_with("DM")));
    assert!(UPLINK.iter().all(|e| e.type_id.starts_with("UM")));
}

#[test]
fn placeholders_match_content_slots() {
    for def in DOWNLINK.iter().chain(UPLINK.iter()) {
        for template in def.templates {
            let slots = template.matches("%s").count();
            assert_eq!(slots, def.content.len(), "{} {}", def.type_id, template);
        }
    }
}

#[test]
fn catalogue_spot_checks_against_the_source_table() {
    let dm0 = downlink("DM0").unwrap();
    assert_eq!(dm0.templates, &["WILCO"]);
    assert_eq!(dm0.fans, &[FansMode::FansA, FansMode::FansB]);
    assert_eq!(dm0.response, ExpectedResponse::No);

    let dm6 = downlink("DM6").unwrap();
    assert_eq!(dm6.templates, &["REQUEST %s"]);
    assert_eq!(dm6.response, ExpectedResponse::Yes);
    assert_eq!(dm6.content[0].kind, ContentType::Level);
    assert_eq!(dm6.content[0].index_start, 1);

    let dm7 = downlink("DM7").unwrap();
    assert_eq!(dm7.fans, &[FansMode::FansA]);
    assert_eq!((dm7.content[0].index_start, dm7.content[1].index_start), (2, 4));

    let um184 = uplink("UM184").unwrap();
    assert_eq!(um184.templates.len(), 2);
    assert!(um184.content[0].monitoring);
    assert_eq!(um184.content[0].kind, ContentType::Time);

    let um183 = uplink("UM183").unwrap();
    assert_eq!(um183.content[0].kind, ContentType::Freetext);
    assert_eq!(um183.content[0].index_end, -1);
    assert_eq!(um183.fans, &[FansMode::FansB]);

    let um9995 = uplink("UM9995").unwrap();
    assert_eq!(um9995.templates, &["LOGOFF"]);
    assert_eq!(um9995.response, ExpectedResponse::NotRequired);
}

#[test]
fn urgent_flags_come_through() {
    let urgent = DOWNLINK.iter().chain(UPLINK.iter()).filter(|e| e.urgent).count();
    assert_eq!(urgent, 13);
}

#[test]
fn network_format_is_hoppie_data2() {
    let m = dm6("FL350");
    assert_eq!(m.render(SerializationFormat::Network), "/data2/3//Y/REQUEST FL350");

    let mut with_previous = dm6("FL350");
    with_previous.previous_transmission_id = 12;
    assert_eq!(
        with_previous.render(SerializationFormat::Network),
        "/data2/3/12/Y/REQUEST FL350"
    );

    let mut wilco = CpdlcMessage::new();
    wilco.current_transmission_id = 4;
    wilco.previous_transmission_id = 9;
    wilco.content.push(downlink_element("DM0").unwrap());
    assert_eq!(wilco.render(SerializationFormat::Network), "/data2/4/9/N/WILCO");
}

#[test]
fn mailbox_and_display_formats() {
    let m = dm6("FL350");
    assert_eq!(m.render(SerializationFormat::Mailbox), "REQUEST @FL350@");
    assert_eq!(
        m.render(SerializationFormat::FmsDisplay),
        "{cyan}1234Z TO EDMM{end}\n{green}REQUEST FL350{end}\n{white}------------------------{end}\n"
    );
    assert_eq!(
        m.render(SerializationFormat::Printer),
        "1234Z TO EDMM}\nREQUEST FL350\n------------------------\n"
    );

    let blank = dm6("");
    assert_eq!(blank.render(SerializationFormat::Mailbox), "REQUEST [      ]");
    assert_eq!(blank.render(SerializationFormat::Network), "/data2/3//Y/REQUEST");
}

#[test]
fn uplink_display_uses_from_and_uplink_templates() {
    let mut m = CpdlcMessage::new();
    m.header.direction = MessageDirection::Uplink;
    let mut e = uplink_element("UM20").unwrap();
    e.content[0].value = "FL350".into();
    m.content.push(e);
    let m = stamped(m, "EDMM");
    assert_eq!(
        m.render(SerializationFormat::FmsDisplay),
        "{cyan}1234Z FROM EDMM{end}\n{green}CLIMB TO FL350{end}\n{white}------------------------{end}\n"
    );
}

#[test]
fn monitored_display_keeps_the_original_colour_quirk() {
    let mut m = dm6("FL350");
    m.message_monitoring = MonitoringState::Monitoring;
    assert_eq!(
        m.render(SerializationFormat::FmsDisplayMonitored),
        "{cyan}1234Z TO EDMM{end}\nREQUEST MAGENTA FL350END{white}------------------------{end}\n"
    );
}

#[test]
fn freetext_elements_replace_the_whole_line() {
    let mut m = CpdlcMessage::new();
    m.header.direction = MessageDirection::Uplink;
    let mut e = uplink_element("UM183").unwrap();
    e.content[0].value = "EXPECT RADAR VECTORS".into();
    m.content.push(e);
    assert_eq!(m.render(SerializationFormat::Mailbox), "EXPECT RADAR VECTORS");
}

#[test]
fn messages_without_content_split_on_underscores() {
    let mut m = CpdlcMessage::new();
    m.header.message = "LINE ONE_LINE TWO".into();
    assert_eq!(m.render(SerializationFormat::Mailbox), "LINE ONE\nLINE TWO");
}

#[test]
fn non_standard_responses_are_appended() {
    let mut m = CpdlcMessage::new();
    m.header.direction = MessageDirection::Uplink;
    m.content.push(uplink_element("UM20").unwrap());
    let mut reply = dm6("FL370");
    reply.header.station = "EDMM".into();
    m.response = Some(Box::new(reply));
    let text = m.render(SerializationFormat::Printer);
    assert!(text.ends_with("1234Z TO EDMM}\nREQUEST FL370\n------------------------\n"), "{text}");

    let mut standard = CpdlcMessage::new();
    standard.header.direction = MessageDirection::Uplink;
    standard.content.push(uplink_element("UM20").unwrap());
    let mut wilco = CpdlcMessage::new();
    wilco.content.push(downlink_element("DM0").unwrap());
    standard.response = Some(Box::new(wilco));
    assert_eq!(standard.render(SerializationFormat::Printer).matches("TO").count(), 1);
}

#[test]
fn dcl_request_formats() {
    let mut dcl = DclMessage::new();
    dcl.callsign = "FBW001".into();
    dcl.origin = "EDDM".into();
    dcl.destination = "EGLL".into();
    dcl.ac_type = "A388".into();
    dcl.atis = "H".into();
    dcl.gate = "G12".into();
    dcl.freetext = vec!["REQ PUSH".into(), "".into(), "  ".into(), "THANKS".into()];
    assert_eq!(dcl.cpdlc.header.kind, MessageType::Dcl);
    assert!(!dcl.cpdlc.close_automatically);

    assert_eq!(
        dcl.render(SerializationFormat::Network),
        "REQUEST PREDEP CLEARANCE \nFBW001 A388 TO EGLL \nAT EDDM STAND G12 \nATIS H"
    );
    assert_eq!(
        dcl.render(SerializationFormat::Mailbox),
        "DEPART REQUEST\nFBW001\nFROM:EDDM GATE:G12\nTO:EGLL ATIS:H\nA/C TYPE:A388\nREQ PUSH\nTHANKS"
    );
    dcl.cpdlc.header.station = "EDDMDEP".into();
    dcl.cpdlc.header.timestamp.seconds = 3600.0;
    dcl.gate.clear();
    dcl.freetext.clear();
    assert_eq!(
        dcl.render(SerializationFormat::FmsDisplay),
        "0100Z TO EDDMDEP\nDEPART REQUEST\nFBW001\nFROM:EDDM\nTO:EGLL ATIS:H\nA/C TYPE:A388"
    );
}

#[test]
fn ocl_request_formats() {
    let mut ocl = OclMessage::new();
    ocl.callsign = "FBW001".into();
    ocl.entry_point = "RESNO".into();
    ocl.entry_time = "1234Z".into();
    ocl.requested_mach = "M.84".into();
    ocl.requested_flightlevel = "FL350".into();
    ocl.cpdlc.current_transmission_id = 5;
    assert_eq!(
        ocl.render(SerializationFormat::Network),
        "/data2/5//N/OCEANIC REQUEST\nFBW001 \nENTRY POINT:RESNO\nAT:1234Z \nREQ:M.84 FL350"
    );
    assert_eq!(
        ocl.render(SerializationFormat::Mailbox),
        "OCEANIC REQUEST\nFBW001 \nENTRY POINT:RESNO\nAT:1234Z \nREQ:M.84 FL350"
    );
}

#[test]
fn freetext_display() {
    let mut m = FreetextMessage::new();
    m.header.message = "see you at the gate".into();
    assert_eq!(m.render(SerializationFormat::Network), "see you at the gate");
    assert_eq!(
        m.render(SerializationFormat::FmsDisplay),
        "{green}SEE YOU AT THE GATE{end}\n{white}------------------------{end}\n"
    );
}

#[test]
fn json_round_trip_and_conversion() {
    let mut m = dm6("FL350");
    m.header.unique_message_id = 17;
    m.header.com_status = ComStatus::Sent;
    m.response = Some(Box::new(stamped(CpdlcMessage::new(), "EDMM")));
    m.reminder_timestamp = Some(Timestamp {
        year: 2026,
        month: 10,
        day: 22,
        seconds: 100.0,
    });
    let value = AnyMessage::Cpdlc(m.clone()).to_json();
    assert_eq!(value["Type"], 5);
    assert_eq!(value["UniqueMessageID"], 17);
    assert_eq!(value["Content"][0]["TypeId"], "DM6");
    assert_eq!(value["Content"][0]["ExpectedResponse"], "Y");
    assert_eq!(value["Content"][0]["Content"][0]["Value"], "FL350");
    assert_eq!(value["Content"][0]["Content"][0]["Type"], 1);
    assert_eq!(value["Content"][0]["FansModes"], json!([1, 2]));
    assert_eq!(AnyMessage::from_json(&value), Some(AnyMessage::Cpdlc(m)));
}

#[test]
fn conversion_follows_the_type_field() {
    let dcl = DclMessage::new();
    let value = AnyMessage::Dcl(dcl.clone()).to_json();
    assert_eq!(value["Type"], 6);
    assert!(matches!(AnyMessage::from_json(&value), Some(AnyMessage::Dcl(d)) if d == dcl));

    let ocl = OclMessage::new();
    let value = AnyMessage::Ocl(ocl.clone()).to_json();
    assert!(matches!(AnyMessage::from_json(&value), Some(AnyMessage::Ocl(o)) if o == ocl));

    let free = FreetextMessage::new();
    let value = AnyMessage::Freetext(free.clone()).to_json();
    assert!(matches!(AnyMessage::from_json(&value), Some(AnyMessage::Freetext(f)) if f == free));

    assert_eq!(AnyMessage::from_json(&json!({"Type": 4})), None);
    assert_eq!(AnyMessage::from_json(&json!({"Type": 8})), None);
}

#[test]
fn ts_shaped_json_is_accepted() {
    let value = json!({
        "Network": 0, "UniqueMessageID": 2, "Timestamp": {"Year": 2026, "Month": 10, "Day": 22, "Seconds": 3600},
        "Station": "EDMM", "ComStatus": 3, "Type": 5, "Direction": 0, "Confirmed": false, "Message": "CLIMB TO FL350",
        "Content": [{
            "TypeId": "UM20", "FansModes": [1, 2], "Urgent": false, "ExpectedResponse": "WU",
            "Content": [{"Type": 1, "IndexStart": 2, "IndexEnd": 4, "Value": "FL350", "Monitoring": false}]
        }],
        "Response": null, "CurrentTransmissionId": 7, "PreviousTransmissionId": -1,
        "MailboxRelevantMessage": true, "CloseAutomatically": true, "MessageMonitoring": 0,
        "SemanticResponseRequired": false, "ReminderTimestamp": null
    });
    match AnyMessage::from_json(&value) {
        Some(AnyMessage::Cpdlc(m)) => {
            assert_eq!(m.header.station, "EDMM");
            assert_eq!(m.header.direction, MessageDirection::Uplink);
            assert_eq!(m.header.com_status, ComStatus::Received);
            assert_eq!(m.render(SerializationFormat::Mailbox), "CLIMB TO @FL350@");
            assert_eq!(m.header.timestamp.mailbox_timestamp(), "0100Z");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn fans_areas() {
    assert_eq!(current_fans_mode("EDMM"), FansMode::FansB);
    assert_eq!(current_fans_mode("LFXX"), FansMode::FansB);
    assert_eq!(current_fans_mode("KZNY"), FansMode::FansA);
    assert_eq!(current_fans_mode("edmm"), FansMode::FansNone);
    assert_eq!(current_fans_mode("ABC"), FansMode::FansNone);
}

#[test]
fn topic_tables() {
    let all = topics::all();
    assert_eq!(all.len(), 95);
    for name in [
        "atcLogon",
        "atcStationStatus",
        "atcAtisReports",
        "routerDatalinkStatus",
        "routerConnect",
        "aocRequestWeather",
        "cpdlcMessages",
        "dclMessages",
        "atcMessageButtonActive",
        "routerReceivedCpdlcMessage",
    ] {
        assert!(all.contains(&name), "{name}");
    }
    assert!(topics::ATC_TO_FMS.contains(&"atcSystemStatus"));
    assert!(topics::FMS_TO_ATC.contains(&"atcRouteData"));
    assert_eq!(topics::FANS_B_AREAS.len(), 217);
}

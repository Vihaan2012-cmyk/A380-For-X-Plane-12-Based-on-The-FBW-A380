use serde_json::{json, Value};

use crate::aoc::AocEvent;
use crate::message::AnyMessage;
use crate::mfd_atccom::AtcDatalinkEvent;
use crate::router::{LinkTriple, RouterEvent};
use crate::weather::AtisType;

pub const ACARS_ACTIVE_VAR: &str = "L:A32NX_ACARS_ACTIVE";
pub const RMP_FREQUENCY_VAR: &str = "L:A32NX_ATSU_RMP_FREQUENCY";
pub const COMPANY_MESSAGE_COUNT_VAR: &str = "L:A32NX_COMPANY_MSG_COUNT";

#[derive(Clone, Debug, PartialEq)]
pub enum BusOut {
    Topic { name: String, payload: Value },
    Var { name: &'static str, value: f64 },
}

fn topic(name: impl Into<String>, payload: Value) -> BusOut {
    BusOut::Topic { name: name.into(), payload }
}

fn triple<T: Copy>(t: &LinkTriple<T>, f: impl Fn(T) -> i32) -> Value {
    json!({ "vhf": f(t.vhf), "satellite": f(t.satellite), "hf": f(t.hf) })
}

fn atis_type(kind: AtisType) -> u8 {
    match kind {
        AtisType::Departure => 0,
        AtisType::Arrival => 1,
        AtisType::Enroute => 2,
    }
}

pub fn router_event(event: &RouterEvent) -> Vec<BusOut> {
    match event {
        RouterEvent::DatalinkStatus(t) => vec![topic("routerDatalinkStatus", triple(t, |c| c as i32))],
        RouterEvent::DatalinkMode(t) => vec![topic("routerDatalinkMode", triple(t, |c| c as i32))],
        RouterEvent::Received(message) => {
            let name = match message.as_ref() {
                AnyMessage::Freetext(_) => "routerReceivedFreetextMessage",
                AnyMessage::Cpdlc(_) | AnyMessage::Dcl(_) | AnyMessage::Ocl(_) => "routerReceivedCpdlcMessage",
                AnyMessage::Metar(_) | AnyMessage::Taf(_) | AnyMessage::Atis(_) => "aocResynchronizeWeatherMessage",
            };
            vec![topic(name, message.to_json())]
        }
        RouterEvent::SendResult { request_id, code } => {
            vec![topic("routerSendMessageResponse", json!({ "requestId": request_id, "status": code.code() }))]
        }
        RouterEvent::RequestSent { request_id } => vec![topic("routerRequestSent", json!(request_id))],
        RouterEvent::Weather { request_id, code, message } => {
            let body = message.as_ref().map_or(Value::Null, |m| serde_json::to_value(m).unwrap_or(Value::Null));
            vec![topic("routerReceivedWeather", json!({ "requestId": request_id, "response": [code.code(), body] }))]
        }
    }
}

pub fn aoc_event(event: &AocEvent) -> Vec<BusOut> {
    match event {
        AocEvent::CompanyMessageCount(n) => vec![BusOut::Var { name: COMPANY_MESSAGE_COUNT_VAR, value: *n as f64 }],
        AocEvent::Resynchronize(message) => {
            let name = match message {
                AnyMessage::Freetext(_) => "aocResynchronizeFreetextMessage",
                _ => "aocResynchronizeWeatherMessage",
            };
            vec![topic(name, message.to_json())]
        }
        AocEvent::Delete(uid) => vec![topic("aocDeleteMessage", json!(uid))],
        AocEvent::Transmit { .. } => Vec::new(),
        AocEvent::TransmissionResponse { request_id, code } => {
            vec![topic("aocTransmissionResponse", json!({ "requestId": request_id, "status": code.code() }))]
        }
        AocEvent::PowerDown => vec![topic("aocResetData", json!(true))],
    }
}

pub fn atc_datalink_event(event: &AtcDatalinkEvent) -> Vec<BusOut> {
    let list = |messages: &[AnyMessage]| Value::Array(messages.iter().map(AnyMessage::to_json).collect());
    match event {
        AtcDatalinkEvent::AtcAtis { index, data } => vec![topic(
            format!("atcAtis_{index}"),
            json!({
                "icao": data.icao,
                "type": atis_type(data.kind),
                "requested": data.requested,
                "autoupdate": data.autoupdate,
                "lastReadAtis": data.last_read_atis,
                "status": data.status,
            }),
        )],
        AtcDatalinkEvent::RequestAtis { icao, kind, request_id } => {
            vec![topic("atcRequestAtis", json!({ "icao": icao, "type": atis_type(*kind), "requestId": request_id }))]
        }
        AtcDatalinkEvent::ActivateAtisAutoUpdate { icao, kind, request_id } => {
            vec![topic("atcActivateAtisAutoUpdate", json!({ "icao": icao, "type": atis_type(*kind), "requestId": request_id }))]
        }
        AtcDatalinkEvent::DeactivateAtisAutoUpdate { icao, request_id } => {
            vec![topic("atcDeactivateAtisAutoUpdate", json!({ "icao": icao, "requestId": request_id }))]
        }
        AtcDatalinkEvent::RegisterCpdlcMessages(m) => vec![topic("atcRegisterCpdlcMessages", list(m))],
        AtcDatalinkEvent::RegisterDclMessages(m) => vec![topic("atcRegisterDclMessages", list(m))],
        AtcDatalinkEvent::RegisterOclMessages(m) => vec![topic("atcRegisterOclMessages", list(m))],
        AtcDatalinkEvent::ResetAtisAutoUpdate => vec![topic("atcResetAtisAutoUpdate", json!(true))],
        AtcDatalinkEvent::AtisRequestDone { .. } | AtcDatalinkEvent::AutoUpdateDone { .. } => Vec::new(),
    }
}

pub fn acars_active(active: bool) -> BusOut {
    BusOut::Var { name: ACARS_ACTIVE_VAR, value: if active { 1.0 } else { 0.0 } }
}

pub fn rmp_frequency(hz: f64) -> BusOut {
    BusOut::Var { name: RMP_FREQUENCY_VAR, value: hz }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mfd_atccom::AirportAtis;
    use crate::status::{AtsuStatusCodes, DatalinkModeCode, DatalinkStatusCode};
    use crate::tables;

    fn names(out: &[BusOut]) -> Vec<String> {
        out.iter()
            .filter_map(|o| match o {
                BusOut::Topic { name, .. } => Some(name.clone()),
                BusOut::Var { .. } => None,
            })
            .collect()
    }

    fn known(name: &str) -> bool {
        let base = name.split('_').next().unwrap_or(name);
        tables::ALL_GROUPS.iter().any(|(_, g)| g.contains(&base)) || base == "atcAtis"
    }

    #[test]
    fn router_events_use_the_fbw_topic_names_and_payloads() {
        let status = LinkTriple { vhf: DatalinkStatusCode::DlkAvail, satellite: DatalinkStatusCode::NotInstalled, hf: DatalinkStatusCode::NotInstalled };
        let out = router_event(&RouterEvent::DatalinkStatus(status));
        assert_eq!(names(&out), vec!["routerDatalinkStatus"]);
        let BusOut::Topic { payload, .. } = &out[0] else { panic!() };
        assert_eq!(payload["vhf"], 2);
        let mode = LinkTriple { vhf: DatalinkModeCode::AtcAoc, satellite: DatalinkModeCode::None, hf: DatalinkModeCode::None };
        assert_eq!(names(&router_event(&RouterEvent::DatalinkMode(mode))), vec!["routerDatalinkMode"]);
        let out = router_event(&RouterEvent::SendResult { request_id: 7, code: AtsuStatusCodes::ComFailed });
        let BusOut::Topic { name, payload } = &out[0] else { panic!() };
        assert_eq!(name, "routerSendMessageResponse");
        assert_eq!(payload, &json!({ "requestId": 7, "status": AtsuStatusCodes::ComFailed.code() }));
        let out = router_event(&RouterEvent::RequestSent { request_id: 3 });
        assert_eq!(out, vec![BusOut::Topic { name: "routerRequestSent".into(), payload: json!(3) }]);
        let out = router_event(&RouterEvent::Weather { request_id: 4, code: AtsuStatusCodes::Ok, message: None });
        let BusOut::Topic { name, payload } = &out[0] else { panic!() };
        assert_eq!(name, "routerReceivedWeather");
        assert_eq!(payload["requestId"], 4);
        assert_eq!(payload["response"][0], 0);
        assert!(payload["response"][1].is_null());
        for o in [out, router_event(&RouterEvent::DatalinkStatus(status))] {
            for n in names(&o) {
                assert!(known(&n), "{n} is not a FBW datalink topic");
            }
        }
    }

    #[test]
    fn aoc_events_map_to_aoc_topics_and_the_company_count_var() {
        assert_eq!(aoc_event(&AocEvent::CompanyMessageCount(3)), vec![BusOut::Var { name: COMPANY_MESSAGE_COUNT_VAR, value: 3.0 }]);
        assert_eq!(names(&aoc_event(&AocEvent::Delete(12))), vec!["aocDeleteMessage"]);
        assert_eq!(names(&aoc_event(&AocEvent::PowerDown)), vec!["aocResetData"]);
        let out = aoc_event(&AocEvent::TransmissionResponse { request_id: 9, code: AtsuStatusCodes::Ok });
        let BusOut::Topic { name, payload } = &out[0] else { panic!() };
        assert_eq!((name.as_str(), payload["requestId"].as_u64(), payload["status"].as_u64()), ("aocTransmissionResponse", Some(9), Some(0)));
        for n in names(&out) {
            assert!(known(&n));
        }
    }

    #[test]
    fn atc_datalink_events_map_to_atc_topics() {
        let data = AirportAtis { icao: "EGLL".into(), kind: AtisType::Arrival, requested: true, autoupdate: false, last_read_atis: None, status: None };
        let out = atc_datalink_event(&AtcDatalinkEvent::AtcAtis { index: 1, data });
        let BusOut::Topic { name, payload } = &out[0] else { panic!() };
        assert_eq!(name, "atcAtis_1");
        assert_eq!((payload["icao"].as_str(), payload["type"].as_u64()), (Some("EGLL"), Some(1)));
        let out = atc_datalink_event(&AtcDatalinkEvent::RequestAtis { icao: "KJFK".into(), kind: AtisType::Departure, request_id: 2 });
        assert_eq!(names(&out), vec!["atcRequestAtis"]);
        assert_eq!(names(&atc_datalink_event(&AtcDatalinkEvent::ResetAtisAutoUpdate)), vec!["atcResetAtisAutoUpdate"]);
        assert_eq!(names(&atc_datalink_event(&AtcDatalinkEvent::RegisterCpdlcMessages(Vec::new()))), vec!["atcRegisterCpdlcMessages"]);
        assert!(atc_datalink_event(&AtcDatalinkEvent::AtisRequestDone { index: 0, code: AtsuStatusCodes::Ok }).is_empty());
        for e in [
            AtcDatalinkEvent::ActivateAtisAutoUpdate { icao: "A".into(), kind: AtisType::Arrival, request_id: 1 },
            AtcDatalinkEvent::DeactivateAtisAutoUpdate { icao: "A".into(), request_id: 1 },
            AtcDatalinkEvent::RegisterDclMessages(Vec::new()),
            AtcDatalinkEvent::RegisterOclMessages(Vec::new()),
        ] {
            for n in names(&atc_datalink_event(&e)) {
                assert!(known(&n), "{n}");
            }
        }
    }

    #[test]
    fn lvars_carry_the_fbw_names() {
        assert_eq!(acars_active(true), BusOut::Var { name: "L:A32NX_ACARS_ACTIVE", value: 1.0 });
        assert_eq!(rmp_frequency(131_725_000.0), BusOut::Var { name: "L:A32NX_ATSU_RMP_FREQUENCY", value: 131_725_000.0 });
    }
}

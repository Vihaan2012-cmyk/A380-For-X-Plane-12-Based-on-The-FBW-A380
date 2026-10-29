use super::*;
use crate::acars::HttpResponse;
use crate::message::{FreetextMessage, MessageDirection};

struct Offline(usize);

impl HttpClient for Offline {
    fn get(&mut self, _url: &str, _accept: &str) -> Result<HttpResponse, String> {
        self.0 += 1;
        Err("offline".into())
    }
}

struct Wx;

impl WeatherSource for Wx {
    fn metar(&mut self, icao: &str) -> Option<String> {
        Some(format!("{icao} 221250Z 24012KT CAVOK 15/08 Q1015"))
    }
    fn taf(&mut self, _icao: &str) -> Option<String> {
        None
    }
    fn atis(&mut self, icao: &str, _kind: AtisType) -> Option<String> {
        Some(format!("{icao} INFORMATION K 1250Z RWY 27L"))
    }
}

fn powered() -> SimInputs {
    SimInputs {
        utc_year: 2026.0,
        utc_month: 10.0,
        utc_day: 22.0,
        utc_seconds: 45_000.0,
        latitude: arinc_raw(51.47, 3),
        longitude: arinc_raw(-0.45, 3),
        vhf3_powered: true,
        rmp_mode_active_3: 1.0,
        cpiom_d1_available: true,
        cpiom_d3_available: true,
        ac_bus_1: true,
        transponder_code: 2000.0,
        ..SimInputs::default()
    }
}

struct Run {
    sys: DatalinkSystem,
    now: f64,
    client: Offline,
}

impl Run {
    fn new() -> Run {
        let mut r = Run { sys: DatalinkSystem::new(), now: 0.0, client: Offline(0) };
        r.step(&powered(), vec![]);
        r
    }

    fn step(&mut self, sim: &SimInputs, inputs: Vec<Input>) -> StepOut {
        let out = self.sys.step(self.now, sim, inputs, &mut self.client, &mut Wx, None);
        self.now += 100.0;
        out
    }

    fn run(&mut self, inputs: Vec<Input>, ms: f64) -> StepOut {
        let mut all = self.step(&powered(), inputs);
        let end = self.now + ms;
        while self.now < end {
            let o = self.step(&powered(), vec![]);
            all.bus.extend(o.bus);
            all.mfd.extend(o.mfd);
            all.vars.extend(o.vars);
            all.winds.extend(o.winds);
        }
        all
    }
}

fn bus(topic: &str, payload: Value) -> Input {
    Input::Bus(topic.into(), payload)
}

fn find<'a>(out: &'a StepOut, topic: &str) -> Vec<&'a Value> {
    out.bus.iter().filter(|(t, _)| t == topic).map(|(_, v)| v).collect()
}

#[test]
fn arinc_words_decode_like_fbw_sdk() {
    assert_eq!(arinc(arinc_raw(51.5, 3)), (3, 51.5));
    assert_eq!(arinc(arinc_raw(-12.25, 1)), (1, -12.25));
    let (ssm, value) = arinc(3.0);
    assert_eq!(ssm, 0);
    assert!(value > 0.0 && value < 1e-40);
    assert_eq!(flight_phase(3.0), AtsuFlightPhase::Preflight);
    assert_eq!(flight_phase(arinc_raw(3.0, 3)), AtsuFlightPhase::Cruise);
}

#[test]
fn clock_keeps_simvarhandlings_split() {
    let c = clock(&SimInputs { utc_seconds: 3725.0, ..SimInputs::default() });
    assert_eq!((c.hour, c.minute, c.second), (1, 2, 3725 - 3600 + 120));
}

#[test]
fn router_status_follows_power_and_data_mode() {
    let mut r = Run::new();
    let out = r.run(vec![], 1100.0);
    let status = find(&out, "routerDatalinkStatus");
    assert_eq!(status.last().unwrap()["vhf"], 2);
    assert_eq!(r.sys.mfd.datalink_status().vhf, DatalinkStatusCode::DlkAvail);
    let mut voice = powered();
    voice.rmp_mode_active_3 = 0.0;
    let mut out = StepOut::default();
    for _ in 0..12 {
        out.bus.extend(r.step(&voice, vec![]).bus);
    }
    assert_eq!(find(&out, "routerDatalinkStatus").last().unwrap()["vhf"], 1);
    let mut off = powered();
    off.cpiom_d3_available = false;
    let mut out = StepOut::default();
    for _ in 0..12 {
        out.bus.extend(r.step(&off, vec![]).bus);
    }
    assert_eq!(find(&out, "routerDatalinkStatus").last().unwrap()["vhf"], -1);
}

#[test]
fn atc_requests_answer_with_the_fbw_request_ids() {
    let mut r = Run::new();
    r.run(vec![], 1100.0);
    let out = r.run(vec![bus("atcLogon", json!({ "station": "EDDM", "requestId": 7 }))], 0.0);
    let reply = find(&out, "atcRequestAtsuStatusCode");
    assert_eq!(reply[0]["requestId"], 7);
    let out = r.run(vec![bus("atcActivateAtisAutoUpdate", json!({ "icao": "EGLL", "type": 1, "requestId": 8 }))], 0.0);
    assert_eq!(find(&out, "atcGenericRequestResponse"), vec![&json!(8)]);
    assert_eq!(find(&out, "atcActiveAtisAutoUpdates"), vec![&json!(["EGLL"])]);
    let out = r.run(vec![bus("atcRequestPositionReport", json!(9))], 0.0);
    let report = find(&out, "atcPositionReport");
    assert_eq!(report[0]["requestId"], 9);
    assert!((report[0]["data"]["flightState"]["lat"].as_f64().unwrap() - 51.47).abs() < 1e-4);
}

#[test]
fn the_native_mfd_requests_atis_without_leaking_its_request_ids() {
    let mut r = Run::new();
    r.run(vec![], 1100.0);
    let fms = MfdCommand::FmsAirports { origin: Some("EGLL".into()), destination: Some("LFPG".into()), alternate: None };
    let out = r.run(vec![Input::Mfd(fms), Input::Mfd(MfdCommand::RequestAtis(0))], 0.0);
    assert!(out.mfd.iter().any(|(t, v)| t == "atcAtis_0" && v["icao"] == "EGLL"));
    assert!(find(&out, "atcRequestAtsuStatusCode").is_empty());
    assert_eq!(r.sys.mfd.atis_reports("EGLL").len(), 1);
    assert!(!r.sys.mfd.atis_airports()[0].requested);
}

#[test]
fn aoc_weather_goes_through_the_router_queue() {
    let mut r = Run::new();
    r.run(vec![], 1100.0);
    let out = r.run(vec![bus("aocRequestWeather", json!({ "icaos": ["EGLL"], "requestMetar": true, "requestId": 3 }))], 15_000.0);
    assert_eq!(find(&out, "aocRequestSentToGround"), vec![&json!(3)]);
    let response = find(&out, "aocWeatherResponse");
    assert_eq!(response[0]["requestId"], 3);
    assert_eq!(response[0]["data"][0], 0);
    assert!(response[0]["data"][1]["Reports"][0]["report"].as_str().unwrap().starts_with("EGLL"));
}

#[test]
fn aoc_send_reports_com_failed_when_offline() {
    let mut r = Run::new();
    let mut m = FreetextMessage::new();
    m.header.direction = MessageDirection::Downlink;
    m.header.message = "HELLO".into();
    let mut sim = powered();
    sim.vhf3_powered = false;
    for _ in 0..12 {
        r.step(&sim, vec![]);
    }
    let out = r.step(&sim, vec![bus("aocSendFreetextMessage", json!({ "message": AnyMessage::Freetext(m).to_json(), "requestId": 5 }))]);
    let reply = find(&out, "aocTransmissionResponse");
    assert_eq!(reply[0], &json!({ "requestId": 5, "status": AtsuStatusCodes::ComFailed.code() }));
}

#[test]
fn fms_wind_requests_round_trip_through_the_main_thread() {
    let mut r = Run::new();
    r.run(vec![], 1100.0);
    let request = json!({ "flightPlan": 2, "message": { "climbWindLevel": null, "cruiseWinds": { "flightLevels": [350], "waypoints": ["DET", { "lat": 50.2, "long": -10.0 }] } } });
    let out = r.run(vec![bus("wind_uplink_request", request)], 0.0);
    let (id, req) = out.winds[0].clone();
    assert!(id >= INTERNAL_REQUEST_BASE);
    assert_eq!(req.climb_wind_level, Some(None));
    assert_eq!(req.descent_wind_level, None);
    assert_eq!(req.cruise_winds.unwrap().waypoints[1], WindFix::LatLon { lat: 50.2, long: -10.0 });
    let uplink = WindUplinkMessage { climb_winds: Some(vec![UplinkedWindLevel { flight_level: 50.0, true_degrees: 270.0, magnitude: 20.0 }]), ..Default::default() };
    let out = r.run(vec![Input::Winds { request_id: id, message: Some(uplink) }], 0.0);
    let resp = find(&out, "wind_uplink_response");
    assert_eq!(resp[0]["flightPlan"], 2);
    assert_eq!(resp[0]["status"], 0);
    assert_eq!(resp[0]["message"]["climbWinds"][0]["trueDegrees"], 270.0);
    assert!(resp[0]["message"].get("cruiseWinds").is_none());
    assert!(find(&out, "aocWindsResponse").is_empty());
}

#[test]
fn acars_state_and_atc_power_down_reach_the_bus() {
    let mut r = Run { sys: DatalinkSystem::new(), now: 0.0, client: Offline(0) };
    let out = r.step(&powered(), vec![]);
    assert!(out.vars.contains(&("L:A32NX_ACARS_ACTIVE".to_string(), 0.0)));
    assert!(out.vars.iter().any(|(n, _)| n == "L:A32NX_ATSU_RMP_FREQUENCY"));
    let mut off = powered();
    off.cpiom_d1_available = false;
    let out = r.step(&off, vec![]);
    assert_eq!(find(&out, "atcResetData"), vec![&json!(true)]);
    r.sys.set_settings(acars_settings("ABC123", ""), NxApiSettings::default());
    r.step(&powered(), vec![]);
    assert!(r.client.0 >= 1);
    assert!(!r.sys.acars_active());
}

use crate::acars::{AcarsConnector, AcarsSettings, HttpClient};
use crate::message::{AnyMessage, MessageNetwork, MessageType, SerializationFormat};
use crate::nxapi::{self, AircraftStatus, NxApiConnector, NxApiSettings, NO_METAR, NO_TAF};
use crate::online::WeatherSource;
use crate::status::{AtsuFlightPhase, AtsuStatusCodes, DatalinkModeCode, DatalinkStatusCode};
use crate::vhf::{Vdl, VdlInputs};
use crate::weather::{AtisType, Report, WeatherMessage};
use crate::winds::{WindRequestMessage, WindUplinkMessage};
use crate::xp_weather::{xplane_winds, WindRoute, XPlaneWeatherHost};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveCommunicationInterface {
    None,
    Vhf,
    Hf,
    Satcom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RouterInputs {
    pub vhf3_powered: bool,
    pub vhf3_data_mode: bool,
    pub acars_active: bool,
    pub satcom_enabled: bool,
    pub flight_phase: Option<AtsuFlightPhase>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkTriple<T> {
    pub vhf: T,
    pub satellite: T,
    pub hf: T,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RouterEvent {
    DatalinkStatus(LinkTriple<DatalinkStatusCode>),
    DatalinkMode(LinkTriple<DatalinkModeCode>),
    Received(Box<AnyMessage>),
    SendResult { request_id: u32, code: AtsuStatusCodes },
    RequestSent { request_id: u32 },
    Weather { request_id: u32, code: AtsuStatusCodes, message: Option<WeatherMessage> },
}

pub struct RouterNet<'a> {
    pub client: &'a mut dyn HttpClient,
    pub acars: &'a mut AcarsConnector,
    pub acars_settings: &'a AcarsSettings,
    pub nxapi: &'a mut NxApiConnector,
    pub nxapi_settings: &'a NxApiSettings,
    pub weather: &'a mut dyn WeatherSource,
    pub status: AircraftStatus,
}

#[derive(Clone, Debug)]
enum Job {
    Inbound { message: AnyMessage, delay: f64 },
    Outbound { request_id: u32, message: AnyMessage, force: bool },
    WeatherUp { request_id: u32, code: AtsuStatusCodes, message: WeatherMessage, delay: f64 },
    WeatherProcessing { request_id: u32, code: AtsuStatusCodes, message: WeatherMessage },
    WeatherDown { request_id: u32, code: AtsuStatusCodes, message: WeatherMessage, delay: f64 },
}

pub struct Router {
    pub inputs: RouterInputs,
    pub vdl: Vdl,
    interface: ActiveCommunicationInterface,
    powered_up: bool,
    waited_com_update: f64,
    waited_hoppie: f64,
    waited_nxapi: f64,
    hoppie_interval: f64,
    nxapi_interval: f64,
    first_poll_hoppie: bool,
    last_update: f64,
    now: f64,
    jobs: Vec<(f64, Job)>,
    events: Vec<RouterEvent>,
    rng: u64,
}

impl Default for Router {
    fn default() -> Self {
        Router::new(0x2545_f491_4f6c_dd1d)
    }
}

impl Router {
    pub fn new(seed: u64) -> Router {
        let mut r = Router {
            inputs: RouterInputs::default(),
            vdl: Vdl::default(),
            interface: ActiveCommunicationInterface::None,
            powered_up: false,
            waited_com_update: 0.0,
            waited_hoppie: 0.0,
            waited_nxapi: 0.0,
            hoppie_interval: 0.0,
            nxapi_interval: 0.0,
            first_poll_hoppie: true,
            last_update: -1.0,
            now: 0.0,
            jobs: Vec::new(),
            events: Vec::new(),
            rng: seed.max(1),
        };
        r.hoppie_interval = AcarsConnector::poll_interval_ms(r.random());
        r.nxapi_interval = NxApiConnector::poll_interval_ms(r.random());
        r
    }

    fn random(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn interface(&self) -> ActiveCommunicationInterface {
        self.interface
    }

    pub fn take_events(&mut self) -> Vec<RouterEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn power_up(&mut self) {
        self.powered_up = true;
    }

    pub fn power_down(&mut self) {
        self.powered_up = false;
        self.jobs.clear();
        self.vdl.reinitialize();
    }

    pub fn connect(&mut self, net: &mut RouterNet, flight_no: &str) -> AtsuStatusCodes {
        let mut code = net.nxapi.connect(net.client, net.nxapi_settings, flight_no, &net.status);
        if code == AtsuStatusCodes::TelexDisabled {
            code = AtsuStatusCodes::Ok;
        }
        if code != AtsuStatusCodes::Ok {
            return code;
        }
        match net.acars.connect(net.client, net.acars_settings, flight_no) {
            AtsuStatusCodes::NoAcarsConnection => AtsuStatusCodes::Ok,
            c => c,
        }
    }

    pub fn disconnect(&mut self, net: &mut RouterNet) -> AtsuStatusCodes {
        let mut nx = net.nxapi.disconnect(net.client, net.nxapi_settings);
        if nx == AtsuStatusCodes::TelexDisabled {
            nx = AtsuStatusCodes::Ok;
        }
        let mut hoppie = net.acars.disconnect();
        if hoppie == AtsuStatusCodes::NoAcarsConnection {
            hoppie = AtsuStatusCodes::Ok;
        }
        if nx != AtsuStatusCodes::Ok {
            nx
        } else {
            hoppie
        }
    }

    pub fn is_station_available(&mut self, net: &mut RouterNet, callsign: &str) -> AtsuStatusCodes {
        net.acars.is_station_available(net.client, net.acars_settings, callsign)
    }

    fn offline(&self) -> bool {
        self.interface == ActiveCommunicationInterface::None || !self.powered_up
    }

    pub fn online(&self) -> bool {
        !self.offline()
    }

    /// Synchronous send for the ATC link (the VDL queue is entered and left at once).
    pub fn transmit_now(&mut self, net: &mut RouterNet, message: &AnyMessage, force: bool) -> AtsuStatusCodes {
        if self.offline() {
            return AtsuStatusCodes::ComFailed;
        }
        let len = message.render(SerializationFormat::Network).len();
        self.vdl.enqueue_outbound_message(self.now, len);
        self.vdl.dequeue_outbound_message(self.now, 0.0);
        let mut message = message.clone();
        self.transmit(net, &mut message, force)
    }

    fn atis_message(net: &mut RouterNet, icao: &str, kind: AtisType) -> WeatherMessage {
        let mut message = WeatherMessage::new(MessageType::Atis, "");
        match net.weather.atis(icao, kind) {
            Some(report) => message.reports.push(Report { airport: icao.to_string(), report }),
            None if net.nxapi_settings.online_features_enabled => {
                nxapi::receive_atis(net.client, net.nxapi_settings, icao, kind, &mut message);
            }
            None => message.reports.push(Report { airport: icao.to_string(), report: nxapi::NO_ATIS.to_string() }),
        }
        message
    }

    /// Synchronous ATIS for the ATC link.
    pub fn atis_now(&mut self, net: &mut RouterNet, icao: &str, kind: AtisType) -> (AtsuStatusCodes, Option<WeatherMessage>) {
        if self.offline() {
            return (AtsuStatusCodes::ComFailed, None);
        }
        (AtsuStatusCodes::Ok, Some(Self::atis_message(net, icao, kind)))
    }

    pub fn send_message(&mut self, request_id: u32, message: AnyMessage, force: bool) -> Option<AtsuStatusCodes> {
        if self.offline() {
            return Some(AtsuStatusCodes::ComFailed);
        }
        let len = message.render(SerializationFormat::Network).len();
        self.vdl.enqueue_outbound_message(self.now, len);
        self.jobs.push((self.now, Job::Outbound { request_id, message, force }));
        None
    }

    fn start_weather_response(&mut self, request_id: u32, code: AtsuStatusCodes, message: WeatherMessage) {
        let delay = self.vdl.enqueue_outbound_packet(self.now);
        self.jobs.push((self.now + delay, Job::WeatherUp { request_id, code, message, delay }));
    }

    pub fn request_atis(&mut self, net: &mut RouterNet, request_id: u32, icao: &str, kind: AtisType) -> Option<AtsuStatusCodes> {
        if self.offline() {
            return Some(AtsuStatusCodes::ComFailed);
        }
        let message = Self::atis_message(net, icao, kind);
        self.start_weather_response(request_id, AtsuStatusCodes::Ok, message);
        None
    }

    pub fn request_weather(&mut self, net: &mut RouterNet, request_id: u32, icaos: &[String], metar: bool) -> Option<AtsuStatusCodes> {
        if self.offline() {
            return Some(AtsuStatusCodes::ComFailed);
        }
        let kind = if metar { MessageType::Metar } else { MessageType::Taf };
        let mut message = WeatherMessage::new(kind, "");
        for icao in icaos {
            let report = if metar {
                net.weather.metar(icao).unwrap_or_else(|| NO_METAR.to_string())
            } else {
                net.weather.taf(icao).unwrap_or_else(|| NO_TAF.to_string())
            };
            message.reports.push(Report { airport: icao.clone(), report });
        }
        self.start_weather_response(request_id, AtsuStatusCodes::Ok, message);
        None
    }

    pub fn request_winds(
        &mut self,
        request_id: u32,
        host: &mut dyn XPlaneWeatherHost,
        route: &WindRoute,
        request: &WindRequestMessage,
    ) -> (AtsuStatusCodes, Option<WindUplinkMessage>) {
        if self.offline() {
            return (AtsuStatusCodes::ComFailed, None);
        }
        self.events.push(RouterEvent::RequestSent { request_id });
        (AtsuStatusCodes::Ok, Some(xplane_winds(host, route, request)))
    }

    fn deliver(&mut self, message: AnyMessage) {
        match message {
            AnyMessage::Freetext(_) | AnyMessage::Cpdlc(_) => self.events.push(RouterEvent::Received(Box::new(message))),
            _ => {}
        }
    }

    fn transmit(&mut self, net: &mut RouterNet, message: &mut AnyMessage, force: bool) -> AtsuStatusCodes {
        match message {
            AnyMessage::Freetext(m) if m.header.network == MessageNetwork::Fbw => {
                net.nxapi.send_telex_message(net.client, net.nxapi_settings, m)
            }
            AnyMessage::Freetext(m) => net.acars.send_telex_message(net.client, net.acars_settings, m, force),
            AnyMessage::Dcl(m) => net.acars.send_dcl_message(net.client, net.acars_settings, m, force),
            AnyMessage::Cpdlc(m) => net.acars.send_cpdlc_message(net.client, net.acars_settings, m, force),
            AnyMessage::Ocl(m) => net.acars.send_cpdlc_message(net.client, net.acars_settings, &m.cpdlc, force),
            _ => AtsuStatusCodes::UnknownMessage,
        }
    }

    fn run_jobs(&mut self, net: &mut RouterNet) {
        while let Some(idx) = self.jobs.iter().position(|(due, _)| *due <= self.now) {
            let (_, job) = self.jobs.remove(idx);
            match job {
                Job::Inbound { message, delay } => {
                    self.vdl.dequeue_inbound_message(self.now, delay);
                    if self.powered_up {
                        self.deliver(message);
                    }
                }
                Job::Outbound { request_id, mut message, force } => {
                    self.vdl.dequeue_outbound_message(self.now, 0.0);
                    let code = self.transmit(net, &mut message, force);
                    self.events.push(RouterEvent::SendResult { request_id, code });
                }
                Job::WeatherUp { request_id, code, message, delay } => {
                    self.vdl.dequeue_outbound_message(self.now, delay);
                    if !self.powered_up {
                        continue;
                    }
                    self.events.push(RouterEvent::RequestSent { request_id });
                    let processing = 300.0 + (self.random() * 500.0).floor();
                    self.jobs.push((self.now + processing, Job::WeatherProcessing { request_id, code, message }));
                }
                Job::WeatherProcessing { request_id, code, message } => {
                    let len = message.render(SerializationFormat::Network).len();
                    let delay = self.vdl.enqueue_inbound_message(self.now, len);
                    self.jobs.push((self.now + delay, Job::WeatherDown { request_id, code, message, delay }));
                }
                Job::WeatherDown { request_id, code, message, delay } => {
                    self.vdl.dequeue_inbound_message(self.now, delay);
                    if self.powered_up {
                        self.events.push(RouterEvent::Weather { request_id, code, message: Some(message) });
                    }
                }
            }
        }
    }

    fn enqueue_received(&mut self, messages: Vec<AnyMessage>) {
        if self.interface != ActiveCommunicationInterface::Vhf {
            return;
        }
        for message in messages {
            if message.header().message.is_empty() {
                continue;
            }
            let len = message.render(SerializationFormat::Network).len();
            let delay = self.vdl.enqueue_inbound_message(self.now, len);
            self.jobs.push((self.now + delay, Job::Inbound { message, delay }));
        }
    }

    pub fn update(&mut self, now_ms: f64, net: &mut RouterNet, vdl_inputs: Option<&VdlInputs>) {
        self.now = now_ms;
        self.run_jobs(net);
        let elapsed = if self.last_update >= 0.0 { now_ms - self.last_update } else { 0.0 };

        self.vdl.vhf3.update_datalink_states(self.inputs.vhf3_powered, self.inputs.vhf3_data_mode, self.inputs.acars_active);
        self.interface = if self.vdl.vhf3.datalink_status == DatalinkStatusCode::DlkAvail && self.powered_up {
            ActiveCommunicationInterface::Vhf
        } else {
            ActiveCommunicationInterface::None
        };

        if self.waited_com_update >= 30000.0 {
            if let Some(inputs) = vdl_inputs {
                self.vdl.simulate_transmission_times(inputs);
            }
            self.waited_com_update = 0.0;
        } else {
            self.waited_com_update += elapsed;
        }

        let status = LinkTriple {
            vhf: if self.powered_up { self.vdl.vhf3.datalink_status } else { DatalinkStatusCode::Inop },
            satellite: if self.inputs.satcom_enabled { DatalinkStatusCode::DlkNotAvail } else { DatalinkStatusCode::NotInstalled },
            hf: DatalinkStatusCode::NotInstalled,
        };
        let mode = LinkTriple {
            vhf: if self.powered_up { self.vdl.vhf3.datalink_mode } else { DatalinkModeCode::None },
            satellite: DatalinkModeCode::None,
            hf: DatalinkModeCode::None,
        };
        self.events.push(RouterEvent::DatalinkStatus(status));
        self.events.push(RouterEvent::DatalinkMode(mode));

        if self.hoppie_interval <= self.waited_hoppie {
            let (code, messages) = net.acars.poll(net.client, net.acars_settings);
            if code == AtsuStatusCodes::Ok {
                if !self.first_poll_hoppie && self.powered_up {
                    self.enqueue_received(messages);
                }
                self.first_poll_hoppie = false;
            }
            self.waited_hoppie = 0.0;
            self.hoppie_interval = AcarsConnector::poll_interval_ms(self.random());
        } else {
            self.waited_hoppie += elapsed;
        }

        if self.nxapi_interval <= self.waited_nxapi {
            let (code, messages) = net.nxapi.poll(net.client, net.nxapi_settings, &net.status);
            if code == AtsuStatusCodes::Ok && self.powered_up {
                self.enqueue_received(messages.into_iter().map(AnyMessage::Freetext).collect());
            }
            self.waited_nxapi = 0.0;
            self.nxapi_interval = NxApiConnector::poll_interval_ms(self.random());
        } else {
            self.waited_nxapi += elapsed;
        }

        self.last_update = now_ms;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acars::AcarsProvider;
    use crate::message::FreetextMessage;
    use crate::nxapi::tests::{ok, Recorder};

    struct SimWx;

    impl WeatherSource for SimWx {
        fn metar(&mut self, icao: &str) -> Option<String> {
            (icao != "ZZZZ").then(|| format!("{icao} 221250Z 24012KT CAVOK 15/08 Q1015"))
        }
        fn taf(&mut self, _icao: &str) -> Option<String> {
            None
        }
    }

    fn settings() -> (AcarsSettings, NxApiSettings) {
        (
            AcarsSettings { provider: AcarsProvider::Hoppie, hoppie_user_id: "ABC123".into(), sai_logon_key: String::new() },
            NxApiSettings { online_features_enabled: true, ..NxApiSettings::default() },
        )
    }

    fn powered() -> Router {
        let mut r = Router::new(7);
        r.inputs = RouterInputs { vhf3_powered: true, vhf3_data_mode: true, ..Default::default() };
        r.power_up();
        r
    }

    #[test]
    fn status_follows_power_and_data_mode() {
        let (a, n) = settings();
        let mut client = Recorder::new(vec![]);
        let (mut acars, mut nx) = (AcarsConnector::new(), NxApiConnector::new());
        let mut wx = SimWx;
        let mut net = RouterNet { client: &mut client, acars: &mut acars, acars_settings: &a, nxapi: &mut nx, nxapi_settings: &n, weather: &mut wx, status: AircraftStatus::default() };
        let mut r = Router::new(3);
        r.update(0.0, &mut net, None);
        let ev = r.take_events();
        assert!(matches!(ev[0], RouterEvent::DatalinkStatus(LinkTriple { vhf: DatalinkStatusCode::Inop, .. })));
        assert_eq!(r.interface(), ActiveCommunicationInterface::None);
        assert_eq!(r.send_message(1, AnyMessage::Freetext(FreetextMessage::new()), false), Some(AtsuStatusCodes::ComFailed));

        r.inputs = RouterInputs { vhf3_powered: true, vhf3_data_mode: true, ..Default::default() };
        r.power_up();
        r.update(1000.0, &mut net, None);
        let ev = r.take_events();
        assert!(matches!(ev[0], RouterEvent::DatalinkStatus(LinkTriple { vhf: DatalinkStatusCode::DlkAvail, .. })));
        assert!(matches!(ev[1], RouterEvent::DatalinkMode(LinkTriple { vhf: DatalinkModeCode::Aoc, .. })));
        assert_eq!(r.interface(), ActiveCommunicationInterface::Vhf);
    }

    #[test]
    fn fbw_telex_goes_through_nxapi_on_next_tick() {
        let (a, n) = settings();
        let mut client = Recorder::new(vec![ok(r#"{"accessToken":"t"}"#), ok("{}")]);
        let (mut acars, mut nx) = (AcarsConnector::new(), NxApiConnector::new());
        let mut wx = SimWx;
        let mut net = RouterNet { client: &mut client, acars: &mut acars, acars_settings: &a, nxapi: &mut nx, nxapi_settings: &n, weather: &mut wx, status: AircraftStatus::default() };
        let mut r = powered();
        assert_eq!(r.connect(&mut net, "TST1"), AtsuStatusCodes::Ok);
        r.update(0.0, &mut net, None);
        let mut msg = FreetextMessage::new();
        msg.header.network = MessageNetwork::Fbw;
        msg.header.station = "OPS".into();
        msg.header.message = "HELLO".into();
        assert_eq!(r.send_message(9, AnyMessage::Freetext(msg), false), None);
        r.update(100.0, &mut net, None);
        let ev = r.take_events();
        assert!(ev.contains(&RouterEvent::SendResult { request_id: 9, code: AtsuStatusCodes::Ok }));
        drop(net);
        assert!(client.calls[1].starts_with("POST https://api.flybywiresim.com/txmsg"));
    }

    #[test]
    fn metar_request_reports_sent_then_answer() {
        let (a, n) = settings();
        let mut client = Recorder::new(vec![]);
        let (mut acars, mut nx) = (AcarsConnector::new(), NxApiConnector::new());
        let mut wx = SimWx;
        let mut net = RouterNet { client: &mut client, acars: &mut acars, acars_settings: &a, nxapi: &mut nx, nxapi_settings: &n, weather: &mut wx, status: AircraftStatus::default() };
        let mut r = powered();
        r.update(0.0, &mut net, None);
        r.take_events();
        assert_eq!(r.request_weather(&mut net, 4, &["EGLL".into(), "EGKK".into()], true), None);
        let mut sent = false;
        let mut answer = None;
        let mut t = 0.0;
        while t < 20_000.0 && answer.is_none() {
            t += 50.0;
            r.update(t, &mut net, None);
            for e in r.take_events() {
                match e {
                    RouterEvent::RequestSent { request_id: 4 } => sent = true,
                    RouterEvent::Weather { request_id: 4, code, message } => answer = Some((code, message.unwrap())),
                    _ => {}
                }
            }
        }
        assert!(sent);
        let (code, message) = answer.expect("weather answer");
        assert_eq!(code, AtsuStatusCodes::Ok);
        assert_eq!(message.reports.len(), 2);
        assert_eq!(message.reports[1].airport, "EGKK");
        drop(net);
        assert!(client.calls.is_empty());
    }

    #[test]
    fn power_down_drops_pending_work() {
        let (a, n) = settings();
        let mut client = Recorder::new(vec![]);
        let (mut acars, mut nx) = (AcarsConnector::new(), NxApiConnector::new());
        let mut wx = SimWx;
        let mut net = RouterNet { client: &mut client, acars: &mut acars, acars_settings: &a, nxapi: &mut nx, nxapi_settings: &n, weather: &mut wx, status: AircraftStatus::default() };
        let mut r = powered();
        r.update(0.0, &mut net, None);
        r.request_weather(&mut net, 1, &["EGLL".into()], true);
        r.power_down();
        for i in 1..100 {
            r.update(i as f64 * 200.0, &mut net, None);
        }
        assert!(!r.take_events().iter().any(|e| matches!(e, RouterEvent::Weather { .. } | RouterEvent::RequestSent { .. })));
    }
}

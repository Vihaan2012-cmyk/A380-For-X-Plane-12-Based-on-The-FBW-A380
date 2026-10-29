//! The A380X's SystemsHost datalink in one place: Acr (Router), AtcDatalink
//! (Atc + ATS623), AtsuSystem (Aoc + AtsuFmsClient) and the inputs
//! SimVarHandling publishes, driven by FBW's bus topics, plus the MFD's
//! AtcDatalinkSystem fed from their outputs.

use serde_json::{json, Value};

use crate::acars::{AcarsConnector, AcarsProvider, AcarsSettings, ActivationOutcome, HttpClient, RETRY_DELAY_MS};
use crate::aoc::{Aoc, AocEvent};
use crate::atc::{Atc, AtcEvent, AtcLink};
use crate::ats623::Ats623;
use crate::host_bridge::{self, BusOut};
use crate::inputs::{AtcInputs, RouteData};
use crate::message::{AnyMessage, SerializationFormat};
use crate::mfd_atccom::{AtcDatalinkEvent, AtcDatalinkSystem};
use crate::nxapi::{AircraftStatus, NxApiConnector, NxApiSettings};
use crate::online::WeatherSource;
use crate::router::{LinkTriple, Router, RouterEvent, RouterNet};
use crate::status::{AtsuFlightPhase, AtsuStatusCodes, Clock, DatalinkModeCode, DatalinkStatusCode, Waypoint};
use crate::vhf::VdlInputs;
use crate::weather::{AtisMessage, AtisType};
use crate::winds::{
    AlternateWindRequest, CruiseWindRequest, UplinkedCruiseWindEntry, UplinkedWindLevel, WindFix, WindRequestMessage,
    WindUplinkMessage,
};

pub const ATC_MSG_WAITING_VAR: &str = "L:A32NX_DCDU_ATC_MSG_WAITING";
pub const ATC_MSG_ACK_VAR: &str = "L:A32NX_DCDU_ATC_MSG_ACK";
/// Request ids the native MFD uses; their responses stay native.
pub const INTERNAL_REQUEST_BASE: u32 = 1 << 24;
const ROUTER_UPDATE_MS: f64 = 1000.0;
const NORMAL_OPERATION: u8 = 3;
const FREQUENCY_MODE_DATA: f64 = 1.0;

/// Raw values of the sim variables SimVarHandling, Acr, AtcDatalink and
/// AtsuSystem read (the ADIRS/flight-phase ones as raw ARINC words).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SimInputs {
    pub utc_year: f64,
    pub utc_month: f64,
    pub utc_day: f64,
    pub utc_seconds: f64,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
    pub heading: f64,
    pub track: f64,
    pub computed_airspeed: f64,
    pub mach: f64,
    pub ground_speed: f64,
    pub vertical_speed: f64,
    pub autopilot_active: f64,
    pub autothrust_mode: f64,
    pub selected_mach: f64,
    pub selected_knots: f64,
    pub wind_direction: f64,
    pub wind_speed: f64,
    pub static_air_temperature: f64,
    pub flight_phase: f64,
    pub vhf3_powered: bool,
    pub transponder_code: f64,
    pub selected_altitude: f64,
    pub rmp_mode_active_3: f64,
    pub cpiom_d1_available: bool,
    pub cpiom_d3_available: bool,
    pub ac_bus_1: bool,
    pub atc_button_pressed: bool,
    pub altitude_above_ground: f64,
    pub true_altitude: f64,
}

/// fbw-sdk `Arinc429Word`: (ssm, value).
pub fn arinc(raw: f64) -> (u8, f64) {
    let ssm = ((raw / 4_294_967_296.0).trunc() as i64 & 3) as u8;
    let low = (raw.trunc() as i64).rem_euclid(1 << 32) as u32;
    (ssm, f32::from_bits(low) as f64)
}

pub fn arinc_raw(value: f64, ssm: u8) -> f64 {
    (value as f32).to_bits() as f64 + ssm as f64 * 4_294_967_296.0
}

fn normal(raw: f64) -> Option<f64> {
    let (ssm, value) = arinc(raw);
    (ssm == NORMAL_OPERATION).then_some(value)
}

/// SimVarHandling's split of ZULU TIME, its seconds-of-minute term included.
pub fn clock(i: &SimInputs) -> Clock {
    let s = i.utc_seconds;
    let hours = (s / 3600.0).floor();
    let minutes = (s / 60.0).floor() % 60.0;
    Clock {
        year: i.utc_year as i32,
        month: i.utc_month as i32,
        day_of_month: i.utc_day as i32,
        hour: hours as i32,
        minute: minutes as i32,
        second: (s.floor() - hours * 3600.0 + minutes * 60.0) as i32,
        seconds_of_day: s,
    }
}

fn flight_phase(raw: f64) -> AtsuFlightPhase {
    use AtsuFlightPhase::*;
    match normal(raw).map(|v| v as i64) {
        Some(1) => Takeoff,
        Some(2) => Climb,
        Some(3) => Cruise,
        Some(4) => Descent,
        Some(5) => Approach,
        Some(6) => GoAround,
        Some(7) => Done,
        _ => Preflight,
    }
}

pub fn atc_inputs(i: &SimInputs, route: RouteData) -> AtcInputs {
    let (at_ssm, at_mode) = arinc(i.autothrust_mode);
    let mut a = AtcInputs::new(clock(i));
    a.latitude = normal(i.latitude);
    a.longitude = normal(i.longitude);
    a.altitude = normal(i.altitude);
    a.heading = normal(i.heading);
    a.track = normal(i.track);
    a.computed_airspeed = normal(i.computed_airspeed);
    a.mach = normal(i.mach);
    a.ground_speed = normal(i.ground_speed);
    a.vertical_speed = normal(i.vertical_speed);
    a.ap_active = normal(i.autopilot_active).map(|v| v != 0.0);
    a.autothrust_active = (at_ssm == NORMAL_OPERATION).then_some(at_mode != 0.0);
    a.selected_altitude = i.selected_altitude;
    a.selected_speed = Some(i.selected_knots);
    a.selected_mach = Some(i.selected_mach);
    a.mach_mode = at_ssm == NORMAL_OPERATION && at_mode == 8.0;
    a.wind_direction = normal(i.wind_direction);
    a.wind_speed = normal(i.wind_speed);
    a.static_air_temperature = normal(i.static_air_temperature);
    a.transponder_code = i.transponder_code.max(0.0) as u32;
    a.route = route;
    a
}

pub fn atis_type_from(v: &Value) -> AtisType {
    match v.as_u64() {
        Some(0) => AtisType::Departure,
        Some(2) => AtisType::Enroute,
        _ => AtisType::Arrival,
    }
}

fn opt_level(v: &Value, key: &str) -> Option<Option<f64>> {
    match v.get(key) {
        None => None,
        Some(Value::Null) => Some(None),
        Some(x) => Some(x.as_f64()),
    }
}

pub fn wind_request_from_json(v: &Value) -> WindRequestMessage {
    let cruise_winds = v.get("cruiseWinds").filter(|c| c.is_object()).map(|c| CruiseWindRequest {
        flight_levels: c["flightLevels"].as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default(),
        waypoints: c["waypoints"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|w| match w {
                        Value::String(s) => Some(WindFix::Ident(s.clone())),
                        _ => Some(WindFix::LatLon { lat: w.get("lat")?.as_f64()?, long: w.get("long")?.as_f64()? }),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    });
    let alternate_wind = v.get("alternateWind").filter(|a| a.is_object()).map(|a| AlternateWindRequest {
        destination_icao: a["destinationIcao"].as_str().unwrap_or_default().to_string(),
        alternate_icao: a["alternateIcao"].as_str().unwrap_or_default().to_string(),
    });
    WindRequestMessage {
        climb_wind_level: opt_level(v, "climbWindLevel"),
        cruise_winds,
        descent_wind_level: opt_level(v, "descentWindLevel"),
        alternate_wind,
    }
}

fn level_json(l: &UplinkedWindLevel) -> Value {
    json!({ "flightLevel": l.flight_level, "trueDegrees": l.true_degrees, "magnitude": l.magnitude })
}

fn cruise_entry_json(e: &UplinkedCruiseWindEntry) -> Value {
    let mut o = match &e.fix {
        WindFix::Ident(s) => json!({ "type": "waypoint", "fixIdent": s }),
        WindFix::LatLon { lat, long } => json!({ "type": "latlon", "lat": lat, "long": long }),
    };
    o["trueDegrees"] = json!(e.true_degrees);
    o["magnitude"] = json!(e.magnitude);
    if let Some(fl) = e.flight_level {
        o["flightLevel"] = json!(fl);
    }
    if let Some(t) = e.temperature {
        o["temperature"] = json!(t);
    }
    o
}

pub fn wind_uplink_to_json(m: &WindUplinkMessage) -> Value {
    let mut o = serde_json::Map::new();
    if let Some(c) = &m.climb_winds {
        o.insert("climbWinds".into(), c.iter().map(level_json).collect());
    }
    if let Some(c) = &m.cruise_winds {
        o.insert(
            "cruiseWinds".into(),
            c.iter().map(|s| json!({ "flightLevel": s.flight_level, "fixes": s.fixes.iter().map(cruise_entry_json).collect::<Vec<_>>() })).collect(),
        );
    }
    if let Some(d) = &m.descent_winds {
        o.insert("descentWinds".into(), d.iter().map(level_json).collect());
    }
    if let Some(a) = &m.alternate_wind {
        o.insert("alternateWind".into(), level_json(a));
    }
    Value::Object(o)
}

fn waypoint(v: &Value) -> Option<Waypoint> {
    serde_json::from_value(v.clone()).ok()
}

fn route_from_json(v: &Value) -> RouteData {
    RouteData {
        last_waypoint: waypoint(&v["lastWaypoint"]),
        active_waypoint: waypoint(&v["activeWaypoint"]),
        next_waypoint: waypoint(&v["nextWaypoint"]),
        destination: waypoint(&v["destination"]),
    }
}

fn messages(v: &Value) -> Vec<AnyMessage> {
    v.as_array().map(|a| a.iter().filter_map(AnyMessage::from_json).collect()).unwrap_or_default()
}

fn req(v: &Value) -> u32 {
    v.get("requestId").and_then(Value::as_u64).or_else(|| v.as_u64()).unwrap_or(0) as u32
}

fn uid(v: &Value) -> i64 {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).unwrap_or(-1)
}

fn status_code_from(v: &Value) -> AtsuStatusCodes {
    use AtsuStatusCodes::*;
    const ALL: [AtsuStatusCodes; 18] = [
        Ok, CallsignInUse, OwnCallsign, NoAcarsConnection, NoTelexConnection, TelexDisabled, ComFailed, NoAtc,
        MailboxFull, UnknownMessage, ProxyError, NewAtisReceived, NoAtisReceived, SystemBusy, EntryOutOfRange,
        FormatError, NotInDatabase, RequestTimeout,
    ];
    v.as_u64().and_then(|c| ALL.get(c as usize).copied()).unwrap_or(Ok)
}

fn link_status(v: &Value) -> LinkTriple<DatalinkStatusCode> {
    let c = |k: &str| match v[k].as_i64() {
        Some(2) => DatalinkStatusCode::DlkAvail,
        Some(1) => DatalinkStatusCode::DlkNotAvail,
        Some(-1) => DatalinkStatusCode::Inop,
        _ => DatalinkStatusCode::NotInstalled,
    };
    LinkTriple { vhf: c("vhf"), satellite: c("satellite"), hf: c("hf") }
}

fn link_mode(v: &Value) -> LinkTriple<DatalinkModeCode> {
    let c = |k: &str| match v[k].as_i64() {
        Some(1) => DatalinkModeCode::AtcAoc,
        Some(2) => DatalinkModeCode::Aoc,
        Some(3) => DatalinkModeCode::Atc,
        _ => DatalinkModeCode::None,
    };
    LinkTriple { vhf: c("vhf"), satellite: c("satellite"), hf: c("hf") }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settings {
    pub acars: Option<AcarsSettings>,
    pub nxapi: NxApiSettings,
}

pub fn acars_settings(hoppie_logon: &str, sayintentions_key: &str) -> AcarsSettings {
    let provider = if !hoppie_logon.is_empty() {
        AcarsProvider::Hoppie
    } else if !sayintentions_key.is_empty() {
        AcarsProvider::Sai
    } else {
        AcarsProvider::None
    };
    AcarsSettings { provider, hoppie_user_id: hoppie_logon.to_string(), sai_logon_key: sayintentions_key.to_string() }
}

/// Commands from the native MFD ATCCOM pages to their AtcDatalinkSystem.
#[derive(Clone, Debug, PartialEq)]
pub enum MfdCommand {
    FmsAirports { origin: Option<String>, destination: Option<String>, alternate: Option<String> },
    RequestAtis(usize),
    UpdateAllAtis,
    ActivateAtisAutoUpdate(usize),
    DeactivateAtisAutoUpdate(usize),
    RegisterMessages(Vec<AnyMessage>),
    ClearLatestError,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    /// A FBW bus topic from the FMS/MFD/OIT side (FMS_TO_*, mailbox, wind_uplink_request).
    Bus(String, Value),
    /// Main-thread result of a wind request (`StepOut::winds`).
    Winds { request_id: u32, message: Option<WindUplinkMessage> },
    Mfd(MfdCommand),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StepOut {
    /// FBW system bus topics (what the JS hosts published).
    pub bus: Vec<(String, Value)>,
    /// MFD-local topics (`atcAtis_N`) from the native AtcDatalinkSystem.
    pub mfd: Vec<(String, Value)>,
    pub vars: Vec<(String, f64)>,
    /// Instrument sounds (`cpdlc_ring`).
    pub sounds: Vec<String>,
    /// Wind requests for the main thread (X-Plane weather).
    pub winds: Vec<(u32, WindRequestMessage)>,
}

impl StepOut {
    fn topic(&mut self, name: &str, payload: Value) {
        self.bus.push((name.to_string(), payload));
    }

    fn outs(&mut self, outs: Vec<BusOut>) {
        for o in outs {
            match o {
                BusOut::Topic { name, payload } => self.bus.push((name, payload)),
                BusOut::Var { name, value } => self.vars.push((name.to_string(), value)),
            }
        }
    }
}

enum Owner {
    AocSend(u32),
    AocWeather(u32),
}

struct Link<'a, 'b> {
    router: &'a mut Router,
    net: &'a mut RouterNet<'b>,
}

impl AtcLink for Link<'_, '_> {
    fn send_message(&mut self, message: &AnyMessage, force: bool) -> AtsuStatusCodes {
        self.router.transmit_now(self.net, message, force)
    }

    fn receive_atis(&mut self, icao: &str, kind: AtisType) -> (AtsuStatusCodes, Option<AtisMessage>) {
        let (code, message) = self.router.atis_now(self.net, icao, kind);
        (code, message.map(|weather| AtisMessage { weather, information: String::new() }))
    }
}

impl Default for NetState {
    fn default() -> Self {
        NetState {
            acars: AcarsConnector::new(),
            nxapi: NxApiConnector::new(),
            acars_settings: acars_settings("", ""),
            nxapi_settings: NxApiSettings::default(),
            activation_retry_at: 0.0,
        }
    }
}

struct NetState {
    acars: AcarsConnector,
    nxapi: NxApiConnector,
    acars_settings: AcarsSettings,
    nxapi_settings: NxApiSettings,
    activation_retry_at: f64,
}

pub struct DatalinkSystem {
    pub router: Router,
    pub atc: Atc,
    pub aoc: Aoc,
    pub ats623: Ats623,
    pub mfd: AtcDatalinkSystem,
    net: NetState,
    inputs: SimInputs,
    route: RouteData,
    router_powered: Option<bool>,
    atc_powered: Option<bool>,
    aoc_powered: Option<bool>,
    button_active: bool,
    rmp_frequency: f64,
    last_router_update: f64,
    next_router_id: u32,
    owners: Vec<(u32, Owner)>,
    wind_requests: Vec<(u32, u32)>,
    next_wind_id: u32,
    acars_active_sent: Option<bool>,
}

impl Default for DatalinkSystem {
    fn default() -> Self {
        DatalinkSystem::new()
    }
}

impl DatalinkSystem {
    pub fn new() -> DatalinkSystem {
        let clock = Clock { year: 0, month: 0, day_of_month: 0, hour: 0, minute: 0, second: 0, seconds_of_day: 0.0 };
        DatalinkSystem {
            router: Router::default(),
            atc: Atc::new(AtcInputs::new(clock)),
            aoc: Aoc::new(),
            ats623: Ats623::new(),
            mfd: AtcDatalinkSystem::new(),
            net: NetState::default(),
            inputs: SimInputs::default(),
            route: RouteData::empty(),
            router_powered: None,
            atc_powered: None,
            aoc_powered: None,
            button_active: false,
            rmp_frequency: 0.0,
            last_router_update: f64::NEG_INFINITY,
            next_router_id: 0,
            owners: Vec::new(),
            wind_requests: Vec::new(),
            next_wind_id: 0,
            acars_active_sent: None,
        }
    }

    /// AcarsConnector.activateAcars on a provider/logon change.
    pub fn set_settings(&mut self, acars: AcarsSettings, nxapi: NxApiSettings) {
        if acars != self.net.acars_settings {
            self.net.acars.activate_acars(&acars);
            self.net.acars_settings = acars;
            self.net.activation_retry_at = 0.0;
        }
        self.net.nxapi_settings = nxapi;
    }

    pub fn acars_active(&self) -> bool {
        self.net.acars.acars_active
    }

    fn router_id(&mut self, owner: Owner) -> u32 {
        let id = self.next_router_id;
        self.next_router_id = self.next_router_id.wrapping_add(1);
        self.owners.push((id, owner));
        id
    }

    fn take_owner(&mut self, id: u32) -> Option<Owner> {
        let i = self.owners.iter().position(|(o, _)| *o == id)?;
        Some(self.owners.remove(i).1)
    }

    pub fn step(
        &mut self,
        now_ms: f64,
        sim: &SimInputs,
        inputs: Vec<Input>,
        client: &mut dyn HttpClient,
        weather: &mut dyn WeatherSource,
        vdl: Option<&VdlInputs>,
    ) -> StepOut {
        let mut out = StepOut::default();
        self.inputs = sim.clone();
        let pressed = sim.atc_button_pressed;

        let mut net_state = std::mem::take(&mut self.net);
        let net = &mut net_state;
        if net.acars.activation_pending() && now_ms >= net.activation_retry_at {
            if net.acars.attempt_activation(client, &net.acars_settings) == ActivationOutcome::Retry {
                net.activation_retry_at = now_ms + RETRY_DELAY_MS as f64;
            }
        }
        let acars_active = net.acars.acars_active;
        if self.acars_active_sent != Some(acars_active) {
            self.acars_active_sent = Some(acars_active);
            out.outs(vec![host_bridge::acars_active(acars_active)]);
        }
        let status = AircraftStatus {
            latitude: arinc(sim.latitude).1,
            longitude: arinc(sim.longitude).1,
            true_altitude: sim.true_altitude,
            heading: arinc(sim.heading).1,
            origin: String::new(),
            destination: self.route.destination.as_ref().map(|w| w.ident.clone()).unwrap_or_default(),
            freetext_enabled: false,
            aircraft_type: "A380".into(),
        };
        let mut rnet = RouterNet {
            client,
            acars: &mut net.acars,
            acars_settings: &net.acars_settings,
            nxapi: &mut net.nxapi,
            nxapi_settings: &net.nxapi_settings,
            weather,
            status,
        };

        self.router.inputs.vhf3_powered = sim.vhf3_powered;
        self.router.inputs.vhf3_data_mode = sim.rmp_mode_active_3 == FREQUENCY_MODE_DATA;
        self.router.inputs.acars_active = acars_active;
        self.router.inputs.flight_phase = Some(flight_phase(sim.flight_phase));
        if self.router_powered != Some(sim.cpiom_d3_available) {
            self.router_powered = Some(sim.cpiom_d3_available);
            if sim.cpiom_d3_available {
                self.router.power_up();
            } else {
                self.router.power_down();
            }
        }
        if self.atc_powered != Some(sim.cpiom_d1_available) {
            self.atc_powered = Some(sim.cpiom_d1_available);
            if sim.cpiom_d1_available {
                self.atc.power_up(now_ms);
            } else {
                self.atc.power_down();
                out.topic("atcResetData", json!(true));
                self.button_active = false;
            }
        }
        if self.aoc_powered != Some(sim.ac_bus_1) {
            self.aoc_powered = Some(sim.ac_bus_1);
            if sim.ac_bus_1 {
                self.aoc.power_up();
            } else {
                self.aoc.power_down();
            }
        }
        if self.atc.is_powered() {
            self.atc.inputs = atc_inputs(sim, self.route.clone());
        }
        if self.aoc.powered_up() {
            self.aoc.utc_clock = Some(clock(sim));
        }

        for input in inputs {
            match input {
                Input::Bus(topic, payload) => self.bus_input(&topic, &payload, &mut rnet, &mut out),
                Input::Winds { request_id, message } => self.winds_done(request_id, message, &mut out),
                Input::Mfd(c) => self.mfd_command(c),
            }
        }
        if pressed {
            if self.atc.is_powered() && self.button_active {
                self.atc.atc_message_button_pressed();
            }
            out.vars.push((ATC_MSG_ACK_VAR.to_string(), 0.0));
        }

        if now_ms - self.last_router_update >= ROUTER_UPDATE_MS {
            self.last_router_update = now_ms;
            self.router.update(now_ms, &mut rnet, vdl);
        }
        {
            let mut link = Link { router: &mut self.router, net: &mut rnet };
            self.atc.tick(now_ms, &mut link);
        }
        self.drain(&mut rnet, &mut out);
        drop(rnet);
        self.net = net_state;
        out
    }

    fn drain(&mut self, net: &mut RouterNet, out: &mut StepOut) {
        for _ in 0..8 {
            let router = self.router.take_events();
            let atc = self.atc.take_events();
            let aoc = self.aoc.take_events();
            let mfd = self.mfd.take_events();
            if router.is_empty() && atc.is_empty() && aoc.is_empty() && mfd.is_empty() {
                return;
            }
            for e in router {
                self.router_event(e, out);
            }
            for e in atc {
                self.atc_event(e, out);
            }
            for e in aoc {
                self.aoc_event(e, net, out);
            }
            for e in mfd {
                self.mfd_event(e, net, out);
            }
        }
    }

    fn feed_mfd(&mut self, topic: &str, payload: &Value) {
        let m = &mut self.mfd;
        match topic {
            "atcResetData" => m.on_reset_data(),
            "routerDatalinkStatus" => m.on_datalink_status(link_status(payload)),
            "routerDatalinkMode" => m.on_datalink_mode(link_mode(payload)),
            "atcSystemStatus" => m.on_system_status(status_code_from(payload)),
            "atcActiveAtisAutoUpdates" => {
                m.on_active_atis_auto_updates(payload.as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default())
            }
            "atcRequestAtsuStatusCode" if req(payload) >= INTERNAL_REQUEST_BASE => {
                m.on_request_atsu_status_code(req(payload) - INTERNAL_REQUEST_BASE, status_code_from(&payload["code"]))
            }
            "atcGenericRequestResponse" if req(payload) >= INTERNAL_REQUEST_BASE => m.on_generic_request_response(req(payload) - INTERNAL_REQUEST_BASE),
            "atcAtisReports" => {
                let reports = payload.as_array().map(|a| a.iter().filter_map(|r| serde_json::from_value(r.clone()).ok()).collect()).unwrap_or_default();
                m.storage.set_atis_reports(reports);
            }
            "atcPrintAtisReportsPrint" => m.atis_reports_print_active = payload.as_bool().unwrap_or(false),
            "atcResynchronizeCpdlcMessage" | "atcResynchronizeDclMessage" | "atcResynchronizeOclMessage" => {
                if let Some(msg) = AnyMessage::from_json(payload) {
                    m.storage.resynchronize_atc_message(msg);
                }
            }
            "atcDeleteMessage" => {
                m.storage.delete_message(uid(payload));
            }
            "atcMonitoredMessages" => {
                let list = messages(payload)
                    .into_iter()
                    .filter_map(|x| match x {
                        AnyMessage::Cpdlc(c) => Some(c),
                        _ => None,
                    })
                    .collect();
                m.storage.set_monitored_messages(list);
            }
            _ => {}
        }
    }

    fn emit(&mut self, out: &mut StepOut, topic: &str, payload: Value) {
        self.feed_mfd(topic, &payload);
        let internal = matches!(topic, "atcRequestAtsuStatusCode" | "atcGenericRequestResponse") && req(&payload) >= INTERNAL_REQUEST_BASE;
        if !internal {
            out.topic(topic, payload);
        }
    }

    fn router_event(&mut self, e: RouterEvent, out: &mut StepOut) {
        match &e {
            RouterEvent::Received(m) => match m.as_ref() {
                AnyMessage::Freetext(_) => {
                    self.aoc.received_freetext((**m).clone());
                    if self.ats623.is_relevant_message(m) {
                        for ev in self.ats623.insert_messages(&mut self.atc, vec![(**m).clone()]) {
                            self.atc_event(ev, out);
                        }
                    }
                }
                AnyMessage::Cpdlc(_) => self.atc.insert_messages(vec![(**m).clone()]),
                _ => {}
            },
            RouterEvent::SendResult { request_id, code } => match self.take_owner(*request_id) {
                Some(Owner::AocSend(fms)) => self.aoc.on_send_result(fms, *code),
                Some(Owner::AocWeather(fms)) => self.emit(out, "aocWeatherResponse", json!({ "requestId": fms, "data": [code.code(), Value::Null] })),
                None => {}
            },
            RouterEvent::RequestSent { request_id } => {
                let fms = self.owners.iter().find_map(|(id, o)| match o {
                    Owner::AocWeather(f) if id == request_id => Some(*f),
                    _ => None,
                });
                if let Some(fms) = fms {
                    self.emit(out, "aocRequestSentToGround", json!(fms));
                }
            }
            RouterEvent::Weather { request_id, code, message } => {
                if let Some(Owner::AocWeather(fms)) = self.take_owner(*request_id) {
                    let body = message.as_ref().map_or(Value::Null, |m| serde_json::to_value(m).unwrap_or(Value::Null));
                    self.emit(out, "aocWeatherResponse", json!({ "requestId": fms, "data": [code.code(), body] }));
                }
            }
            RouterEvent::DatalinkStatus(_) | RouterEvent::DatalinkMode(_) => {
                for o in host_bridge::router_event(&e) {
                    if let BusOut::Topic { name, payload } = o {
                        self.emit(out, &name, payload);
                    }
                }
            }
        }
    }

    fn atc_event(&mut self, e: AtcEvent, out: &mut StepOut) {
        let list = |m: &[AnyMessage]| Value::Array(m.iter().map(AnyMessage::to_json).collect());
        match e {
            AtcEvent::ResetSystem => self.emit(out, "resetSystem", json!(true)),
            AtcEvent::CpdlcMessages(m) => {
                let name = match m.first() {
                    Some(AnyMessage::Dcl(_)) => "dclMessages",
                    Some(AnyMessage::Ocl(_)) => "oclMessages",
                    _ => "cpdlcMessages",
                };
                self.emit(out, name, list(&m));
            }
            AtcEvent::LogonMessage(s) => self.emit(out, "logonMessage", json!(s)),
            AtcEvent::SystemStatus(s) => self.emit(out, "systemStatus", json!(s as i8)),
            AtcEvent::MessageStatus { uid, status } => self.emit(out, "messageStatus", json!({ "uid": uid, "status": status as i8 })),
            AtcEvent::MailboxDelete(uid) => self.emit(out, "deleteMessage", json!(uid)),
            AtcEvent::SystemStatusCode(c) => self.emit(out, "atcSystemStatus", json!(c.code())),
            AtcEvent::RmpFrequency(f) => {
                let raw = match f {
                    Some(hz) => {
                        self.rmp_frequency = hz;
                        arinc_raw(hz, NORMAL_OPERATION)
                    }
                    None => arinc_raw(self.rmp_frequency, 1),
                };
                out.outs(vec![host_bridge::rmp_frequency(raw)]);
            }
            AtcEvent::ActivateButton => {
                self.button_active = true;
                out.vars.push((ATC_MSG_WAITING_VAR.to_string(), 1.0));
            }
            AtcEvent::ResetButton => {
                self.button_active = false;
                out.vars.push((ATC_MSG_WAITING_VAR.to_string(), 0.0));
                out.vars.push((ATC_MSG_ACK_VAR.to_string(), 0.0));
            }
            AtcEvent::AtcRing => out.sounds.push("cpdlc_ring".into()),
            AtcEvent::StationStatus(s) => self.emit(
                out,
                "atcStationStatus",
                json!({ "current": s.current, "next": s.next, "notificationTime": s.notification_time, "mode": s.mode.code(), "logonInProgress": s.logon_in_progress }),
            ),
            AtcEvent::MaxUplinkDelay(d) => self.emit(out, "atcMaxUplinkDelay", json!(d)),
            AtcEvent::ActiveAtisAutoUpdates(a) => self.emit(out, "atcActiveAtisAutoUpdates", json!(a)),
            AtcEvent::AtisReports(r) => self.emit(out, "atcAtisReports", serde_json::to_value(&r).unwrap_or(Value::Null)),
            AtcEvent::MonitoredMessages(m) => self.emit(out, "atcMonitoredMessages", list(&m)),
            AtcEvent::AutomaticPositionReport(b) => self.emit(out, "atcAutomaticPositionReportActive", json!(b)),
            AtcEvent::PrintAtisReports(b) => self.emit(out, "atcPrintAtisReportsPrint", json!(b)),
            AtcEvent::PrintMessage(m) => self.emit(out, "atcPrintMessage", json!(m.render(SerializationFormat::Printer))),
            AtcEvent::ResynchronizeMessage(m) => {
                let name = match m {
                    AnyMessage::Dcl(_) => "atcResynchronizeDclMessage",
                    AnyMessage::Ocl(_) => "atcResynchronizeOclMessage",
                    _ => "atcResynchronizeCpdlcMessage",
                };
                self.emit(out, name, m.to_json());
            }
            AtcEvent::DeleteMessage(uid) => self.emit(out, "atcDeleteMessage", json!(uid)),
            AtcEvent::MessageModify(m) => self.emit(out, "atcMessageModify", m.to_json()),
            AtcEvent::AocIgnoreMessageId(uid) => self.aoc.ignore_incoming_message(uid),
            AtcEvent::HandoverResult(_) => {}
        }
    }

    fn aoc_event(&mut self, e: AocEvent, _net: &mut RouterNet, out: &mut StepOut) {
        match e {
            AocEvent::Transmit { request_id, message } => {
                let id = self.router_id(Owner::AocSend(request_id));
                if let Some(code) = self.router.send_message(id, message, false) {
                    self.take_owner(id);
                    self.aoc.on_send_result(request_id, code);
                }
            }
            AocEvent::Resynchronize(m) => {
                let name = match m {
                    AnyMessage::Freetext(_) => "aocResynchronizeFreetextMessage",
                    _ => "aocResynchronizeWeatherMessage",
                };
                self.emit(out, name, m.to_json());
            }
            other => {
                for o in host_bridge::aoc_event(&other) {
                    match o {
                        BusOut::Topic { name, payload } => self.emit(out, &name, payload),
                        BusOut::Var { name, value } => out.vars.push((name.to_string(), value)),
                    }
                }
            }
        }
    }

    fn mfd_event(&mut self, e: AtcDatalinkEvent, net: &mut RouterNet, out: &mut StepOut) {
        match &e {
            AtcDatalinkEvent::AtcAtis { .. } => {
                for o in host_bridge::atc_datalink_event(&e) {
                    if let BusOut::Topic { name, payload } = o {
                        out.mfd.push((name, payload));
                    }
                }
            }
            AtcDatalinkEvent::AtisRequestDone { .. } | AtcDatalinkEvent::AutoUpdateDone { .. } => {}
            _ => {
                for o in host_bridge::atc_datalink_event(&e) {
                    if let BusOut::Topic { name, mut payload } = o {
                        if let Some(id) = payload.get("requestId").and_then(Value::as_u64) {
                            payload["requestId"] = json!(id as u32 + INTERNAL_REQUEST_BASE);
                        }
                        self.bus_input(&name, &payload, net, out);
                    }
                }
            }
        }
    }

    fn mfd_command(&mut self, c: MfdCommand) {
        let m = &mut self.mfd;
        match c {
            MfdCommand::FmsAirports { origin, destination, alternate } => {
                m.on_fms_origin(origin.as_deref());
                m.on_fms_destination(destination.as_deref());
                m.on_fms_alternate(alternate.as_deref());
            }
            MfdCommand::RequestAtis(i) if i < 3 => m.request_atis(i),
            MfdCommand::UpdateAllAtis => m.update_all_atis(),
            MfdCommand::ActivateAtisAutoUpdate(i) if i < 3 => {
                m.activate_atis_auto_update(i);
            }
            MfdCommand::DeactivateAtisAutoUpdate(i) if i < 3 => {
                m.deactivate_atis_auto_update(i);
            }
            MfdCommand::RegisterMessages(msgs) => m.register_messages(msgs),
            MfdCommand::ClearLatestError => m.clear_latest_atc_error_message(),
            _ => {}
        }
    }

    fn position_report(&self) -> Value {
        let i = &self.atc.inputs;
        let v = |o: Option<f64>| o.unwrap_or(0.0);
        let mach_mode = i.mach_mode;
        json!({
            "flightState": {
                "lat": v(i.latitude), "lon": v(i.longitude),
                "altitude": v(i.altitude).round(), "heading": v(i.heading).round(), "track": v(i.track).round(),
                "indicatedAirspeed": if mach_mode { v(i.mach) } else { v(i.computed_airspeed).round() },
                "groundSpeed": v(i.ground_speed).round(), "verticalSpeed": v(i.vertical_speed).round(),
            },
            "autopilot": {
                "apActive": i.ap_active.unwrap_or(false),
                "speed": if mach_mode { v(i.selected_mach) } else { v(i.selected_speed) },
                "machMode": mach_mode,
                "altitude": i.selected_altitude,
            },
            "environment": { "windDirection": v(i.wind_direction), "windSpeed": v(i.wind_speed), "temperature": v(i.static_air_temperature) },
            "lastWaypoint": i.route.last_waypoint, "activeWaypoint": i.route.active_waypoint,
            "nextWaypoint": i.route.next_waypoint, "destination": i.route.destination,
        })
    }

    pub fn bus_input(&mut self, topic: &str, p: &Value, net: &mut RouterNet, out: &mut StepOut) {
        let atc_on = self.atc.is_powered();
        macro_rules! link {
            () => {
                &mut Link { router: &mut self.router, net }
            };
        }
        match topic {
            "atcRouteData" if atc_on => {
                let route = route_from_json(p);
                self.route = route.clone();
                self.atc.new_route_received(link!(), route);
            }
            "atcLogon" => {
                let code = self.atc.logon(link!(), p["station"].as_str().unwrap_or_default());
                self.emit(out, "atcRequestAtsuStatusCode", json!({ "requestId": req(p), "code": code.code() }));
            }
            "atcLogoff" => {
                let code = self.atc.logoff(link!());
                self.emit(out, "atcRequestAtsuStatusCode", json!({ "requestId": req(p), "code": code.code() }));
            }
            "atcRequestAtis" => {
                let code = self.atc.receive_atis(link!(), p["icao"].as_str().unwrap_or_default(), atis_type_from(&p["type"]));
                self.emit(out, "atcRequestAtsuStatusCode", json!({ "code": code.code(), "requestId": req(p) }));
            }
            "atcActivateAtisAutoUpdate" => {
                self.atc.activate_atis_auto_update(p["icao"].as_str().unwrap_or_default(), atis_type_from(&p["type"]));
                self.emit(out, "atcGenericRequestResponse", json!(req(p)));
            }
            "atcDeactivateAtisAutoUpdate" => {
                self.atc.deactivate_atis_auto_update(p["icao"].as_str().unwrap_or_default());
                self.emit(out, "atcGenericRequestResponse", json!(req(p)));
            }
            "atcTogglePrintAtisReportsPrint" => {
                self.atc.toggle_print_atis_reports();
                self.emit(out, "atcGenericRequestResponse", json!(req(p)));
            }
            "atcSetMaxUplinkDelay" => {
                self.atc.set_max_uplink_delay(p["delay"].as_f64().unwrap_or(-1.0) as i64);
                self.emit(out, "atcGenericRequestResponse", json!(req(p)));
            }
            "atcToggleAutomaticPositionReport" => {
                self.atc.toggle_automatic_position_report();
                self.emit(out, "atcGenericRequestResponse", json!(req(p)));
            }
            "atcRequestPositionReport" => {
                let data = self.position_report();
                self.emit(out, "atcPositionReport", json!({ "requestId": req(p), "data": data }));
            }
            "atcRegisterCpdlcMessages" | "atcRegisterDclMessages" | "atcRegisterOclMessages" => self.atc.insert_messages(messages(p)),
            "atcUpdateMessage" => {
                if let Some(m) = AnyMessage::from_json(p) {
                    self.atc.update_message(m);
                }
            }
            "atcMessageRead" => {
                self.atc.message_read(uid(p));
            }
            "atcRemoveMessage" => {
                self.atc.remove_message(uid(p));
            }
            "atcCleanupMessages" => self.atc.cleanup_messages(),
            "atcResetAtisAutoUpdate" => self.atc.reset_atis_auto_update(),
            "uplinkResponse" if atc_on => self.atc.mailbox_uplink_response(link!(), uid(&p["uid"]), uid(&p["responseId"])),
            "downlinkTransmit" if atc_on => self.atc.mailbox_downlink_transmit(link!(), uid(p)),
            "modifyMessage" if atc_on => self.atc.mailbox_modify_message(uid(p)),
            "printMessage" if atc_on => self.atc.mailbox_print_message(uid(p)),
            "closeMessage" => self.atc.mailbox_close_message(uid(p)),
            "updateMessageMonitoring" if atc_on => self.atc.mailbox_update_message_monitoring(uid(p)),
            "stopMessageMonitoring" => self.atc.mailbox_stop_message_monitoring(uid(p)),
            "recallMessage" if atc_on => self.atc.mailbox_recall_message(),
            "readMessage" if atc_on => self.atc.mailbox_read_message(uid(p)),
            "invertSemanticResponse" if atc_on => self.atc.mailbox_invert_semantic_response(uid(p)),
            "visibleMessage" if atc_on => self.atc.mailbox_visible_message(uid(p)),
            "aocSendFreetextMessage" => {
                if let Some(m) = AnyMessage::from_json(&p["message"]) {
                    self.aoc.send_message(req(p), m);
                }
            }
            "aocRequestAtis" | "aocRequestWeather" => {
                let fms = req(p);
                if !self.aoc.powered_up() {
                    self.emit(out, "aocWeatherResponse", json!({ "requestId": fms, "data": [AtsuStatusCodes::ComFailed.code(), Value::Null] }));
                    return;
                }
                let id = self.router_id(Owner::AocWeather(fms));
                let immediate = if topic == "aocRequestAtis" {
                    self.router.request_atis(net, id, p["icao"].as_str().unwrap_or_default(), atis_type_from(&p["type"]))
                } else {
                    let icaos: Vec<String> = p["icaos"].as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default();
                    self.router.request_weather(net, id, &icaos, p["requestMetar"].as_bool().unwrap_or(true))
                };
                if let Some(code) = immediate {
                    self.take_owner(id);
                    self.emit(out, "aocWeatherResponse", json!({ "requestId": fms, "data": [code.code(), Value::Null] }));
                }
            }
            "aocRegisterWeatherMessages" | "aocRegisterFreetextMessages" => self.aoc.insert_messages(messages(p)),
            "aocMessageRead" => self.aoc.message_read(uid(p)),
            "aocRemoveMessage" => self.aoc.remove_message(uid(p)),
            "aocRequestWinds" => {
                let fms = req(p);
                if !self.aoc.powered_up() || !self.router.online() {
                    self.winds_done(fms, None, out);
                } else {
                    out.winds.push((fms, wind_request_from_json(p)));
                }
            }
            "wind_uplink_request" => {
                let id = INTERNAL_REQUEST_BASE + self.next_wind_id;
                self.next_wind_id = (self.next_wind_id + 1) % INTERNAL_REQUEST_BASE;
                self.wind_requests.push((id, p["flightPlan"].as_u64().unwrap_or(0) as u32));
                let mut request = p["message"].clone();
                if !request.is_object() {
                    request = json!({});
                }
                request["requestId"] = json!(id);
                self.bus_input("aocRequestWinds", &request, net, out);
            }
            "routerConnect" => {
                let code = self.router.connect(net, p["callsign"].as_str().unwrap_or_default());
                self.emit(out, "routerManagementResponse", json!({ "requestId": req(p), "status": code.code() }));
            }
            "routerDisconnect" => {
                let code = self.router.disconnect(net);
                self.emit(out, "routerManagementResponse", json!({ "requestId": req(p), "status": code.code() }));
            }
            "routerRequestStationAvailable" => {
                let code = self.router.is_station_available(net, p["callsign"].as_str().unwrap_or_default());
                self.emit(out, "routerManagementResponse", json!({ "requestId": req(p), "status": code.code() }));
            }
            _ => {}
        }
        self.drain(net, out);
    }

    fn winds_done(&mut self, fms: u32, message: Option<WindUplinkMessage>, out: &mut StepOut) {
        let code = if message.is_some() { AtsuStatusCodes::Ok } else { AtsuStatusCodes::ComFailed };
        let body = message.as_ref().map_or(Value::Null, wind_uplink_to_json);
        if let Some(i) = self.wind_requests.iter().position(|(id, _)| *id == fms) {
            let (_, flight_plan) = self.wind_requests.remove(i);
            out.topic("wind_uplink_response", json!({ "status": code.code(), "message": body, "flightPlan": flight_plan }));
        } else {
            self.emit(out, "aocWindsResponse", json!({ "requestId": fms, "data": [code.code(), body] }));
        }
    }
}

#[cfg(test)]
#[path = "system_tests.rs"]
mod tests;

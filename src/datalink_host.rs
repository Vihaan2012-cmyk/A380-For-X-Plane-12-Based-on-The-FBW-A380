//! X-Plane side of crates/datalink: HTTP on net.rs, X-Plane weather, the
//! ATIS source, the online-ATC poller and the bus the displays read.

use std::collections::{HashMap, HashSet};
use std::ffi::{c_int, c_void};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use datalink::acars::{HttpClient, HttpResponse};
use datalink::atis_gen::{choose_atis, AtisAirport, AtisRunway};
use datalink::host_bridge::{self, BusOut};
use datalink::nxapi::NxApiSettings;
use datalink::online::{fetch_online_atc, NoLookup, OnlineAtc, OnlineNetwork, Secret, WeatherSource};
use datalink::system::{acars_settings, DatalinkSystem, Input, MfdCommand, SimInputs, StepOut};
use datalink::weather::AtisType;
use datalink::xp_weather::{WindSample, XPlaneWeatherHost};
use serde_json::Value;

const RECEIVE_MS: u32 = 20_000;
const ONLINE_REFRESH: Duration = Duration::from_secs(60);
const ONLINE_RANGE_NM: f64 = 400.0;
const MS_TO_KT: f64 = 1.943_844;
const TOPIC_QUEUE_MAX: usize = 512;
const WORKER_PERIOD: Duration = Duration::from_millis(100);
const CREDENTIALS_REFRESH: Duration = Duration::from_secs(5);
const INPUT_QUEUE_MAX: usize = 256;

pub struct PluginHttp;

impl PluginHttp {
    fn send(method: &str, url: &str, body: &str, headers: &str) -> Result<HttpResponse, String> {
        crate::net::request_bytes(method, url, body.as_bytes(), headers, RECEIVE_MS)
            .map(|(status, _, bytes)| HttpResponse { status, body: String::from_utf8_lossy(&bytes).into_owned() })
            .ok_or_else(|| format!("{method} {} failed", url.split('?').next().unwrap_or(url)))
    }

    fn headers(content_type: Option<&str>, authorization: Option<&str>) -> String {
        let mut h = String::new();
        if let Some(c) = content_type.filter(|c| !c.is_empty()) {
            h.push_str(&format!("Content-Type: {c}\r\n"));
        }
        if let Some(a) = authorization.filter(|a| !a.is_empty()) {
            h.push_str(&format!("Authorization: {a}\r\n"));
        }
        h
    }
}

impl HttpClient for PluginHttp {
    fn get(&mut self, url: &str, accept: &str) -> Result<HttpResponse, String> {
        Self::send("GET", url, "", &format!("Accept: {accept}\r\n"))
    }

    fn post(&mut self, url: &str, content_type: &str, authorization: Option<&str>, body: &str) -> Result<HttpResponse, String> {
        Self::send("POST", url, body, &Self::headers(Some(content_type), authorization))
    }

    fn put(&mut self, url: &str, content_type: &str, authorization: Option<&str>, body: &str) -> Result<HttpResponse, String> {
        Self::send("PUT", url, body, &Self::headers(Some(content_type), authorization))
    }

    fn delete(&mut self, url: &str, authorization: Option<&str>) -> Result<HttpResponse, String> {
        Self::send("DELETE", url, "", &Self::headers(None, authorization))
    }

    fn get_authorized(&mut self, url: &str, authorization: &str) -> Result<HttpResponse, String> {
        Self::send("GET", url, "", &Self::headers(None, Some(authorization)))
    }

    fn get_with_header(&mut self, url: &str, name: &str, value: &str) -> Result<HttpResponse, String> {
        if name.contains(['\r', '\n', ':']) || value.contains(['\r', '\n']) {
            return Err("bad header".into());
        }
        Self::send("GET", url, "", &format!("Accept: application/json\r\n{name}: {value}\r\n"))
    }
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
struct Winds {
    alt_msl: f32,
    speed: f32,
    direction: f32,
    gust_speed: f32,
    shear: f32,
    turbulence: f32,
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
struct Clouds {
    cloud_type: f32,
    coverage: f32,
    alt_top: f32,
    alt_base: f32,
}

#[repr(C)]
struct WeatherInfo {
    struct_size: c_int,
    temperature_alt: f32,
    dewpoint_alt: f32,
    pressure_alt: f32,
    precip_rate_alt: f32,
    wind_dir_alt: f32,
    wind_spd_alt: f32,
    turbulence_alt: f32,
    wave_height: f32,
    wave_length: f32,
    wave_dir: c_int,
    wave_speed: f32,
    visibility: f32,
    precip_rate: f32,
    thermal_climb: f32,
    pressure_sl: f32,
    wind_layers: [Winds; 13],
    cloud_layers: [Clouds; 3],
}

type GetWeatherFn = unsafe extern "C" fn(f64, f64, f64, *mut WeatherInfo) -> c_int;

fn weather_fn() -> Option<GetWeatherFn> {
    static F: OnceLock<Option<usize>> = OnceLock::new();
    let p = (*F.get_or_init(|| crate::xp::Xplm::xplm_symbol("XPLMGetWeatherAtLocation").map(|p: *mut c_void| p as usize)))?;
    Some(unsafe { std::mem::transmute::<usize, GetWeatherFn>(p) })
}

/// Main thread only (XPLMWeather.h).
pub struct XpWeather;

impl XPlaneWeatherHost for XpWeather {
    fn metar_for_airport(&mut self, icao: &str) -> Option<String> {
        crate::xp::metar_for_airport(icao)
    }

    fn weather_at(&mut self, latitude: f64, longitude: f64, altitude_msl_m: f64) -> Option<WindSample> {
        if !crate::xp::on_main_thread() {
            return None;
        }
        let f = weather_fn()?;
        let mut info: WeatherInfo = unsafe { std::mem::zeroed() };
        info.struct_size = std::mem::size_of::<WeatherInfo>() as c_int;
        unsafe { f(latitude, longitude, altitude_msl_m, &mut info) };
        let ok = info.wind_dir_alt.is_finite() && info.wind_spd_alt.is_finite() && info.temperature_alt.is_finite();
        ok.then(|| WindSample {
            direction_true_deg: (info.wind_dir_alt as f64).rem_euclid(360.0),
            speed_kt: info.wind_spd_alt as f64 * MS_TO_KT,
            temperature_c: info.temperature_alt as f64,
        })
    }
}

#[derive(Default)]
struct Shared {
    metars: HashMap<String, (Instant, Option<String>)>,
    wanted_metars: HashSet<String>,
    online: Vec<OnlineAtc>,
    online_at: Option<Instant>,
    position: Option<(f64, f64)>,
    topics: Vec<(String, Value)>,
    latest: HashMap<String, String>,
    vars: HashMap<&'static str, f64>,
    last_error: Option<String>,
}

fn shared() -> std::sync::MutexGuard<'static, Shared> {
    static S: OnceLock<Mutex<Shared>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Shared::default())).lock().unwrap_or_else(|e| e.into_inner())
}

type AirportLookup = Box<dyn Fn(&str) -> Option<AtisAirport> + Send + Sync>;

fn airport_lookup() -> &'static Mutex<Option<AirportLookup>> {
    static L: OnceLock<Mutex<Option<AirportLookup>>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(None))
}

/// Runways and transition levels for the ATIS, from navdata.
pub fn set_airport_lookup(lookup: AirportLookup) {
    *airport_lookup().lock().unwrap_or_else(|e| e.into_inner()) = Some(lookup);
}

#[cfg(not(feature = "js"))]
pub fn navdata_airport(_icao: &str) -> Option<AtisAirport> {
    None
}

#[cfg(feature = "js")]
pub fn navdata_airport(icao: &str) -> Option<AtisAirport> {
    let a = crate::navdata::shared::airport(icao)?;
    Some(AtisAirport {
        icao: a.icao,
        runways: a.runway_ends.into_iter().map(|e| AtisRunway { ident: e.ident, heading_true_deg: e.heading_true_deg }).collect(),
        transition_level_ft: a.transition_level_ft,
        transition_alt_ft: a.transition_alt_ft,
    })
}

pub fn install_navdata_lookup() {
    let mut l = airport_lookup().lock().unwrap_or_else(|e| e.into_inner());
    if l.is_none() {
        *l = Some(Box::new(navdata_airport));
    }
}

pub fn publish(outs: Vec<BusOut>) {
    let mut s = shared();
    for out in outs {
        match out {
            BusOut::Topic { name, payload } => {
                s.latest.insert(name.clone(), payload.to_string());
                if s.topics.len() >= TOPIC_QUEUE_MAX {
                    s.topics.remove(0);
                }
                s.topics.push((name, payload));
            }
            BusOut::Var { name, value } => {
                s.vars.insert(name, value);
            }
        }
    }
}

pub fn take_topics() -> Vec<(String, Value)> {
    std::mem::take(&mut shared().topics)
}

pub fn take_last_error() -> Option<String> {
    shared().last_error.take()
}

pub fn string_var(name: &str) -> Option<String> {
    shared().latest.get(name).cloned()
}

pub fn var(name: &str) -> Option<f64> {
    let name = if name.starts_with("L:") { name.to_string() } else { format!("L:{name}") };
    shared().vars.iter().find(|(k, _)| **k == name).map(|(_, v)| *v)
}

pub fn publish_router(events: &[datalink::router::RouterEvent]) {
    publish(events.iter().flat_map(host_bridge::router_event).collect());
}

pub fn publish_aoc(events: &[datalink::aoc::AocEvent]) {
    publish(events.iter().flat_map(host_bridge::aoc_event).collect());
}

pub fn publish_atc_datalink(events: &[datalink::mfd_atccom::AtcDatalinkEvent]) {
    publish(events.iter().flat_map(host_bridge::atc_datalink_event).collect());
}

#[derive(Clone, Debug, Default)]
pub struct Credentials {
    pub hoppie_logon: Secret,
    pub vatsim_token: Secret,
    pub ivao_token: Secret,
    pub sayintentions_key: Secret,
}

pub fn credentials() -> Credentials {
    let s = crate::app_settings::current();
    Credentials {
        hoppie_logon: Secret::new(s.hoppie_logon.reveal()),
        vatsim_token: Secret::new(s.vatsim_token.reveal()),
        ivao_token: Secret::new(s.ivao_token.reveal()),
        sayintentions_key: Secret::new(s.sayintentions_key.reveal()),
    }
}

fn connected_network(c: &Credentials) -> Option<OnlineNetwork> {
    if !c.vatsim_token.is_empty() {
        Some(OnlineNetwork::Vatsim)
    } else if !c.ivao_token.is_empty() {
        Some(OnlineNetwork::Ivao)
    } else {
        None
    }
}

/// Call from the flight loop (main thread): position for the poller and
/// X-Plane METARs for the airports the datalink asked for.
pub fn main_thread_tick(lat: f64, lon: f64) {
    let wanted: Vec<String> = {
        let mut s = shared();
        s.position = Some((lat, lon));
        s.wanted_metars.drain().collect()
    };
    for icao in wanted {
        let metar = crate::xp::metar_for_airport(&icao);
        shared().metars.insert(icao, (Instant::now(), metar));
    }
}

/// Worker thread: refreshes the online ATC list once a minute while the
/// pilot has network credentials.
pub fn poll_online(client: &mut dyn HttpClient) {
    let Some(network) = connected_network(&credentials()) else {
        let mut s = shared();
        s.online.clear();
        s.online_at = None;
        return;
    };
    let (due, pos) = {
        let s = shared();
        (s.online_at.map_or(true, |t| t.elapsed() >= ONLINE_REFRESH), s.position)
    };
    let (true, Some((lat, lon))) = (due, pos) else { return };
    let result = fetch_online_atc(client, network, &NoLookup, lat, lon, ONLINE_RANGE_NM);
    let mut s = shared();
    s.online_at = Some(Instant::now());
    match result {
        Ok(list) => s.online = list,
        Err(e) => s.last_error = Some(format!("datalink: online ATC: {e}")),
    }
}

pub fn spawn_online_poller() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        install_navdata_lookup();
        let _ = std::thread::Builder::new().name("fbw-datalink-online".into()).spawn(|| loop {
            poll_online(&mut PluginHttp);
            std::thread::sleep(Duration::from_secs(5));
        });
    });
}

/// `WeatherSource` for the router: X-Plane METARs (cached from the main
/// thread), no TAF, and the network ATIS or one built from the METAR.
pub struct HostWeather;

impl HostWeather {
    fn cached_metar(icao: &str) -> Option<String> {
        let icao = icao.to_ascii_uppercase();
        let mut s = shared();
        match s.metars.get(&icao) {
            Some((at, m)) if at.elapsed() < Duration::from_secs(300) => m.clone(),
            _ => {
                s.wanted_metars.insert(icao);
                None
            }
        }
    }
}

impl WeatherSource for HostWeather {
    fn metar(&mut self, icao: &str) -> Option<String> {
        Self::cached_metar(icao)
    }

    fn taf(&mut self, _icao: &str) -> Option<String> {
        None
    }

    fn atis(&mut self, icao: &str, kind: AtisType) -> Option<String> {
        let connected = connected_network(&credentials()).is_some();
        let online = shared().online.clone();
        let metar = Self::cached_metar(icao);
        let airport = airport_lookup().lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|f| f(&icao.to_ascii_uppercase()));
        choose_atis(connected, &online, icao, kind, metar.as_deref(), airport.as_ref())
    }
}

/// The main thread's view of the sim for one DatalinkSystem step.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub now_ms: f64,
    pub sim: SimInputs,
    pub authoritative: bool,
}

/// Shadow runs make no network calls: the JS datalink owns the connection.
pub struct Offline;

impl HttpClient for Offline {
    fn get(&mut self, _url: &str, _accept: &str) -> Result<HttpResponse, String> {
        Err("datalink shadow: no network".into())
    }
}

pub struct Worker {
    pub system: DatalinkSystem,
}

impl Default for Worker {
    fn default() -> Self {
        Worker::new()
    }
}

impl Worker {
    pub fn new() -> Worker {
        Worker { system: DatalinkSystem::new() }
    }

    pub fn step(&mut self, snap: &Snapshot, inputs: Vec<Input>, creds: &Credentials, net: &mut dyn HttpClient, weather: &mut dyn WeatherSource) -> StepOut {
        self.system.set_settings(acars_settings(creds.hoppie_logon.reveal(), creds.sayintentions_key.reveal()), NxApiSettings::default());
        let mut offline = Offline;
        let client: &mut dyn HttpClient = if snap.authoritative { net } else { &mut offline };
        self.system.step(snap.now_ms, &snap.sim, inputs, client, weather, None)
    }
}

#[derive(Default)]
struct Mailbox {
    snapshot: Option<Snapshot>,
    inputs: Vec<Input>,
    out: StepOut,
}

fn mailbox() -> &'static (Mutex<Mailbox>, Condvar) {
    static M: OnceLock<(Mutex<Mailbox>, Condvar)> = OnceLock::new();
    M.get_or_init(|| (Mutex::new(Mailbox::default()), Condvar::new()))
}

fn lock_mailbox() -> std::sync::MutexGuard<'static, Mailbox> {
    mailbox().0.lock().unwrap_or_else(|e| e.into_inner())
}

fn capped<T>(v: &mut Vec<T>, max: usize) {
    let excess = v.len().saturating_sub(max);
    v.drain(..excess);
}

pub fn merge(into: &mut StepOut, out: StepOut) {
    into.bus.extend(out.bus);
    into.mfd.extend(out.mfd);
    into.vars.extend(out.vars);
    into.sounds.extend(out.sounds);
    into.winds.extend(out.winds);
    capped(&mut into.bus, TOPIC_QUEUE_MAX);
    capped(&mut into.mfd, TOPIC_QUEUE_MAX);
    capped(&mut into.vars, TOPIC_QUEUE_MAX);
    capped(&mut into.sounds, INPUT_QUEUE_MAX);
    capped(&mut into.winds, INPUT_QUEUE_MAX);
}

/// Main thread, every frame: the latest sim values for the worker.
pub fn submit(snapshot: Snapshot) {
    lock_mailbox().snapshot = Some(snapshot);
}

pub fn push_input(input: Input) {
    let mut m = lock_mailbox();
    m.inputs.push(input);
    capped(&mut m.inputs, INPUT_QUEUE_MAX);
    drop(m);
    mailbox().1.notify_one();
}

/// A FBW bus topic to the datalink (JS forwarder or the native FMS/MFD).
pub fn push_bus_input(topic: &str, payload: Value) {
    push_input(Input::Bus(topic.to_string(), payload));
}

/// Native MFD ATCCOM pages.
pub fn push_mfd_command(c: MfdCommand) {
    push_input(Input::Mfd(c));
}

/// Everything the worker produced since the last call.
pub fn take_output() -> StepOut {
    std::mem::take(&mut lock_mailbox().out)
}

fn wait_for_work(timeout: Duration) -> Option<(Snapshot, Vec<Input>)> {
    let (lock, cv) = mailbox();
    let mut m = lock.lock().unwrap_or_else(|e| e.into_inner());
    if m.inputs.is_empty() {
        m = cv.wait_timeout(m, timeout).map(|(g, _)| g).unwrap_or_else(|e| e.into_inner().0);
    }
    let snap = m.snapshot.clone()?;
    Some((snap, std::mem::take(&mut m.inputs)))
}

/// Runs the DatalinkSystem on its own thread: HTTP here never blocks the sim.
pub fn spawn_worker() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        install_navdata_lookup();
        let _ = std::thread::Builder::new().name("fbw-datalink".into()).spawn(|| {
            let mut worker = Worker::new();
            let mut creds = (credentials(), Instant::now());
            loop {
                let Some((snap, inputs)) = wait_for_work(WORKER_PERIOD) else { continue };
                if creds.1.elapsed() >= CREDENTIALS_REFRESH {
                    creds = (credentials(), Instant::now());
                }
                let step = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| worker.step(&snap, inputs, &creds.0, &mut PluginHttp, &mut HostWeather)));
                match step {
                    Ok(out) => merge(&mut lock_mailbox().out, out),
                    Err(_) => {
                        worker = Worker::new();
                        shared().last_error = Some("datalink: worker step panicked; system reset".into());
                    }
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use datalink::router::{LinkTriple, RouterEvent};
    use datalink::status::DatalinkStatusCode;

    #[test]
    fn the_weather_struct_matches_the_sdk_v1_layout() {
        assert_eq!(std::mem::size_of::<Winds>(), 24);
        assert_eq!(std::mem::size_of::<Clouds>(), 16);
        assert_eq!(std::mem::offset_of!(WeatherInfo, wind_dir_alt), 20);
        assert_eq!(std::mem::offset_of!(WeatherInfo, wind_layers), 64);
    }

    #[test]
    fn published_events_become_strings_and_vars() {
        let t = LinkTriple { vhf: DatalinkStatusCode::DlkAvail, satellite: DatalinkStatusCode::NotInstalled, hf: DatalinkStatusCode::NotInstalled };
        publish_router(&[RouterEvent::DatalinkStatus(t)]);
        publish(vec![host_bridge::acars_active(true)]);
        assert!(string_var("routerDatalinkStatus").unwrap().contains("\"vhf\":2"));
        assert_eq!(var("A32NX_ACARS_ACTIVE"), Some(1.0));
        assert!(take_topics().iter().any(|(n, _)| n == "routerDatalinkStatus"));
    }

    #[test]
    fn the_navdata_lookup_is_installed_once() {
        install_navdata_lookup();
        assert!(airport_lookup().lock().unwrap().is_some());
        assert!(navdata_airport("ZZZZZZZ").is_none());
    }

    #[test]
    fn header_injection_is_refused() {
        assert!(PluginHttp.get_with_header("https://x.invalid/", "X-Developer-Key", "a\r\nHost: evil").is_err());
    }

    struct Counting(usize);

    impl HttpClient for Counting {
        fn get(&mut self, _url: &str, _accept: &str) -> Result<HttpResponse, String> {
            self.0 += 1;
            Ok(HttpResponse { status: 200, body: "ok".into() })
        }
    }

    struct NoWx;

    impl WeatherSource for NoWx {
        fn metar(&mut self, _icao: &str) -> Option<String> {
            None
        }
        fn taf(&mut self, _icao: &str) -> Option<String> {
            None
        }
    }

    fn snap(authoritative: bool, now_ms: f64) -> Snapshot {
        let sim = SimInputs { cpiom_d1_available: true, cpiom_d3_available: true, ac_bus_1: true, vhf3_powered: true, rmp_mode_active_3: 1.0, ..SimInputs::default() };
        Snapshot { now_ms, sim, authoritative }
    }

    fn hoppie() -> Credentials {
        Credentials { hoppie_logon: Secret::new("abc123"), ..Credentials::default() }
    }

    #[test]
    fn only_the_authoritative_worker_touches_the_network() {
        let mut net = Counting(0);
        let mut shadow = Worker::new();
        for i in 0..20 {
            shadow.step(&snap(false, i as f64 * 1000.0), vec![], &hoppie(), &mut net, &mut NoWx);
        }
        assert_eq!(net.0, 0);
        assert!(!shadow.system.acars_active());

        let mut live = Worker::new();
        let out = live.step(&snap(true, 0.0), vec![], &hoppie(), &mut net, &mut NoWx);
        assert!(net.0 >= 1);
        assert!(live.system.acars_active());
        assert!(out.vars.contains(&("L:A32NX_ACARS_ACTIVE".to_string(), 1.0)));
    }

    #[test]
    fn worker_answers_bus_inputs_with_fbw_topics() {
        let mut w = Worker::new();
        w.step(&snap(true, 0.0), vec![], &Credentials::default(), &mut Offline, &mut NoWx);
        let out = w.step(&snap(true, 100.0), vec![Input::Bus("routerRequestStationAvailable".into(), serde_json::json!({ "requestId": 7, "callsign": "EGLL" }))], &Credentials::default(), &mut Offline, &mut NoWx);
        assert!(out.bus.iter().any(|(t, p)| t == "routerManagementResponse" && p["requestId"] == 7), "{:?}", out.bus);
    }

    #[test]
    fn merged_output_is_capped_oldest_first() {
        let mut all = StepOut::default();
        for i in 0..(TOPIC_QUEUE_MAX + 10) {
            merge(&mut all, StepOut { bus: vec![(format!("t{i}"), Value::Null)], ..StepOut::default() });
        }
        assert_eq!(all.bus.len(), TOPIC_QUEUE_MAX);
        assert_eq!(all.bus[0].0, "t10");
    }

    #[test]
    fn the_mailbox_hands_inputs_and_the_latest_snapshot_to_the_worker() {
        submit(snap(true, 5.0));
        push_bus_input("atcLogoff", serde_json::json!({ "requestId": 1 }));
        let (_, inputs) = wait_for_work(Duration::from_millis(1)).unwrap();
        assert!(inputs.iter().any(|i| matches!(i, Input::Bus(t, _) if t == "atcLogoff")));
    }
}

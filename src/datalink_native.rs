//! Main-thread side of the native datalink (`datalink_host` worker): reads
//! the sim inputs SimVarHandling read, and either compares the worker's L vars
//! with what the JS datalink wrote (shadow) or, when `datalink` is named in
//! `nativeSystems`, writes them and hands the bus topics to the JS with the
//! JS datalink stood down through `L:A380X_NATIVE_DATALINK_MASK`.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use datalink::host_bridge::{ACARS_ACTIVE_VAR, COMPANY_MESSAGE_COUNT_VAR, RMP_FREQUENCY_VAR};
use datalink::system::{Input, SimInputs, StepOut, ATC_MSG_WAITING_VAR};
use datalink::winds::{WindRequestMessage, WindUplinkMessage};
use datalink::xp_weather::WindRoute;
use serde_json::{json, Value};

use crate::datalink_host::{self, Snapshot};
use crate::datalink_native_patches::{INPUT_CALL, MASK_VAR, TAKE_CALL, BIT_DATALINK};
use crate::native_shadow::{Comparator, Compared};

pub const NAME: &str = "datalink";
const SETTINGS_PERIOD_S: f64 = 10.;
const MAX_QUEUED: usize = 512;

/// Vars both datalinks write that do not depend on a network answer.
const COMPARED: [&str; 3] = [ATC_MSG_WAITING_VAR, RMP_FREQUENCY_VAR, COMPANY_MESSAGE_COUNT_VAR];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Datalink;

impl Compared for Datalink {
    fn label(self) -> &'static str {
        NAME
    }

    fn debounce(self) -> f64 {
        2.
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub off: bool,
    pub authoritative: bool,
}

impl Config {
    pub fn parse(raw: &str, native_all: bool) -> Config {
        let mut c = Config { off: false, authoritative: native_all };
        for token in raw.split(|ch: char| ch == ',' || ch == ';' || ch.is_whitespace()).filter(|t| !t.is_empty()) {
            match token.to_ascii_lowercase().as_str() {
                "off" | "none" => c.off = true,
                NAME => c.authoritative = true,
                _ => {}
            }
        }
        if c.off {
            c.authoritative = false;
        }
        c
    }

    pub fn mask(&self) -> f64 {
        if self.authoritative {
            BIT_DATALINK as f64
        } else {
            0.
        }
    }
}

pub trait Io {
    fn get(&mut self, name: &str, unit: &str) -> f64;
    fn set(&mut self, name: &str, value: f64);
    fn play_sound(&mut self, name: &str);
}

fn on(io: &mut dyn Io, name: &str) -> bool {
    io.get(name, "number") != 0.
}

/// SimVarHandling's sources, Acr/AtcDatalink/AtsuSystem's power, the RMP data mode.
pub fn read_inputs(io: &mut dyn Io) -> SimInputs {
    let n = |io: &mut dyn Io, name: &str| io.get(name, "number");
    SimInputs {
        utc_year: n(io, "E:ZULU YEAR"),
        utc_month: n(io, "E:ZULU MONTH OF YEAR"),
        utc_day: n(io, "E:ZULU DAY OF MONTH"),
        utc_seconds: n(io, "E:ZULU TIME"),
        latitude: n(io, "L:A32NX_ADIRS_IR_1_LATITUDE"),
        longitude: n(io, "L:A32NX_ADIRS_IR_1_LONGITUDE"),
        altitude: n(io, "L:A32NX_ADIRS_ADR_1_ALTITUDE"),
        heading: n(io, "L:A32NX_ADIRS_IR_1_HEADING"),
        track: n(io, "L:A32NX_ADIRS_IR_1_TRACK"),
        computed_airspeed: n(io, "L:A32NX_ADIRS_ADR_1_COMPUTED_AIRSPEED"),
        mach: n(io, "L:A32NX_ADIRS_ADR_1_MACH"),
        ground_speed: n(io, "L:A32NX_ADIRS_IR_1_GROUND_SPEED"),
        vertical_speed: n(io, "L:A32NX_ADIRS_IR_1_VERTICAL_SPEED"),
        autopilot_active: n(io, "L:A32NX_AUTOPILOT_ACTIVE"),
        autothrust_mode: n(io, "L:A32NX_AUTOTHRUST_MODE"),
        selected_mach: n(io, "L:A32NX_MachPreselVal"),
        selected_knots: io.get("L:A32NX_SpeedPreselVal", "knots"),
        wind_direction: n(io, "L:A32NX_ADIRS_IR_1_WIND_DIRECTION"),
        wind_speed: n(io, "L:A32NX_ADIRS_IR_1_WIND_SPEED"),
        static_air_temperature: n(io, "L:A32NX_ADIRS_ADR_1_STATIC_AIR_TEMPERATURE"),
        flight_phase: n(io, "L:A32NX_FMGC_FLIGHT_PHASE"),
        vhf3_powered: on(io, "L:A32NX_ELEC_DC_1_BUS_IS_POWERED"),
        transponder_code: n(io, "TRANSPONDER CODE:1"),
        selected_altitude: io.get("AUTOPILOT ALTITUDE LOCK VAR:3", "feet"),
        rmp_mode_active_3: io.get("L:FBW_RMP_MODE_ACTIVE_3", "enum"),
        cpiom_d1_available: io.get("L:A32NX_CPIOM_D1_AVAIL", "bool") != 0.,
        cpiom_d3_available: io.get("L:A32NX_CPIOM_D3_AVAIL", "bool") != 0.,
        ac_bus_1: on(io, "L:A32NX_ELEC_AC_1_BUS_IS_POWERED"),
        atc_button_pressed: on(io, "L:A32NX_DCDU_ATC_MSG_ACK"),
        altitude_above_ground: io.get("PLANE ALT ABOVE GROUND", "feet"),
        true_altitude: io.get("PLANE ALTITUDE", "feet"),
    }
}

/// What one frame hands on: the worker snapshot, bus topics for the JS,
/// and wind requests for X-Plane's weather (main thread only).
#[derive(Default)]
pub struct Frame {
    pub snapshot: Option<Snapshot>,
    pub to_js: Vec<(String, Value)>,
    pub winds: Vec<(u32, WindRequestMessage)>,
    pub unanswered_winds: Vec<u32>,
}

pub struct Native {
    config: Config,
    config_read_at: Option<f64>,
    config_fixed: bool,
    announced: bool,
    mask_written: f64,
    latest: BTreeMap<String, f64>,
    comparator: Comparator<Datalink>,
}

impl Default for Native {
    fn default() -> Self {
        Self::new()
    }
}

impl Native {
    pub fn new() -> Self {
        Native {
            config: Config::default(),
            config_read_at: None,
            config_fixed: false,
            announced: false,
            mask_written: -1.,
            latest: BTreeMap::new(),
            comparator: Comparator::new(),
        }
    }

    pub fn set_config(&mut self, config: Config) {
        self.config = config;
        self.config_fixed = true;
    }

    pub fn config(&self) -> Config {
        self.config
    }

    fn refresh_config(&mut self, time_s: f64, lines: &mut Vec<String>) {
        if self.config_fixed || self.config_read_at.is_some_and(|t| time_s - t < SETTINGS_PERIOD_S && time_s >= t) {
            return;
        }
        self.config_read_at = Some(time_s);
        let config = Config::parse(&crate::native_shadow::settings_text(), crate::app_settings::native_mode::native_all());
        if config != self.config || !self.announced {
            self.announced = true;
            lines.push(format!("native datalink: {}", if config.off { "off" } else if config.authoritative { "authoritative" } else { "shadow" }));
        }
        self.config = config;
    }

    /// `out` is what the worker produced since the last frame.
    pub fn frame(&mut self, io: &mut dyn Io, time_s: f64, out: StepOut, lines: &mut Vec<String>) -> Frame {
        self.refresh_config(time_s, lines);
        let mut frame = Frame::default();
        if self.config.off {
            return frame;
        }
        let mask = self.config.mask();
        if mask != self.mask_written {
            self.mask_written = mask;
            io.set(MASK_VAR, mask);
        }
        let authoritative = self.config.authoritative;
        if authoritative {
            for (n, v) in &out.vars {
                io.set(n, *v);
            }
            for s in &out.sounds {
                io.play_sound(s);
            }
            datalink_host::publish(
                out.bus.iter().chain(&out.mfd).map(|(name, payload)| datalink::host_bridge::BusOut::Topic { name: name.clone(), payload: payload.clone() }).collect(),
            );
            frame.to_js = out.bus;
            frame.winds = out.winds;
        } else {
            self.latest.extend(out.vars);
            frame.unanswered_winds = out.winds.into_iter().map(|(id, _)| id).collect();
            if !on(io, ACARS_ACTIVE_VAR) {
                for name in COMPARED {
                    if let Some(native) = self.latest.get(name).copied() {
                        let fbw = io.get(name, "number");
                        lines.extend(self.comparator.observe(Datalink, name, native, fbw, time_s));
                    }
                }
            }
            lines.extend(self.comparator.summary(time_s));
        }
        frame.snapshot = Some(Snapshot { now_ms: time_s * 1000., sim: read_inputs(io), authoritative });
        frame
    }
}

static TO_JS: Mutex<Vec<(String, Value)>> = Mutex::new(Vec::new());

fn push_to_js(items: Vec<(String, Value)>) {
    let mut q = TO_JS.lock().unwrap_or_else(|e| e.into_inner());
    q.extend(items);
    let excess = q.len().saturating_sub(MAX_QUEUED);
    q.drain(..excess);
}

/// Answers the patched SystemsHost's two Coherent calls; any thread.
pub fn coherent_call(name: &str, args_json: &str) -> Option<String> {
    match name {
        TAKE_CALL => {
            let taken = std::mem::take(&mut *TO_JS.lock().unwrap_or_else(|e| e.into_inner()));
            Some(json!({ "bus": taken.into_iter().map(|(t, d)| json!([t, d])).collect::<Vec<_>>() }).to_string())
        }
        INPUT_CALL => {
            let args: Vec<Value> = serde_json::from_str(args_json).unwrap_or_default();
            if let Some(topic) = args.first().and_then(Value::as_str) {
                let payload = match args.get(1) {
                    Some(Value::String(s)) => serde_json::from_str(s).unwrap_or(Value::Null),
                    Some(v) => v.clone(),
                    None => Value::Null,
                };
                datalink_host::push_bus_input(topic, payload);
            }
            Some("true".to_string())
        }
        _ => None,
    }
}

type RouteProvider = Box<dyn Fn() -> Option<WindRoute> + Send + Sync>;

static WIND_ROUTE: Mutex<Option<RouteProvider>> = Mutex::new(None);

/// The FMS route the wind uplink samples X-Plane's weather along.
pub fn set_wind_route_provider(provider: RouteProvider) {
    *WIND_ROUTE.lock().unwrap_or_else(|e| e.into_inner()) = Some(provider);
}

fn winds_for(request: &WindRequestMessage) -> Option<WindUplinkMessage> {
    let route = WIND_ROUTE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|f| f())?;
    Some(datalink::xp_weather::xplane_winds(&mut datalink_host::XpWeather, &route, request))
}

/// Hands a frame on: snapshot to the worker, topics to the JS, winds answered.
pub fn dispatch(frame: Frame, winds: impl Fn(&WindRequestMessage) -> Option<WindUplinkMessage>) {
    for (request_id, request) in &frame.winds {
        datalink_host::push_input(Input::Winds { request_id: *request_id, message: winds(request) });
    }
    for request_id in frame.unanswered_winds {
        datalink_host::push_input(Input::Winds { request_id, message: None });
    }
    push_to_js(frame.to_js);
    if let Some(s) = frame.snapshot {
        datalink_host::submit(s);
    }
}

struct LiveIo<'a, 'b> {
    host: &'a mut crate::js_bridge::VarsHost<'b>,
    resolved: &'a mut HashMap<(String, String), crate::js_bridge::Resolved>,
}

impl Io for LiveIo<'_, '_> {
    fn get(&mut self, name: &str, unit: &str) -> f64 {
        let key = (name.to_string(), unit.to_string());
        let r = *self.resolved.entry(key).or_insert_with(|| self.host.resolve(name, unit));
        self.host.read(name, r)
    }

    fn set(&mut self, name: &str, value: f64) {
        let key = (name.to_string(), "number".to_string());
        let r = *self.resolved.entry(key).or_insert_with(|| self.host.resolve(name, "number"));
        self.host.write(name, r, value);
    }

    fn play_sound(&mut self, name: &str) {
        crate::sound::play_instrument_sound(name);
    }
}

struct Running {
    native: Native,
    state: crate::js_bridge::HostState,
    env: crate::js_bridge::EnvRefs,
    resolved: HashMap<(String, String), crate::js_bridge::Resolved>,
}

thread_local! {
    static RUNNING: RefCell<Option<Running>> = const { RefCell::new(None) };
}

/// Flight loop, main thread.
pub fn tick(vars: &mut crate::Vars, time: f64) {
    let mut lines = Vec::new();
    RUNNING.with(|cell| {
        let mut guard = cell.borrow_mut();
        let running = guard.get_or_insert_with(|| {
            datalink_host::spawn_worker();
            Running {
                native: Native::new(),
                state: crate::js_bridge::HostState::default(),
                env: crate::js_bridge::EnvRefs::new(vars.xplm),
                resolved: HashMap::new(),
            }
        });
        let out = datalink_host::take_output();
        let mut host = crate::js_bridge::VarsHost::new(vars, &mut running.state, &running.env, time);
        let mut io = LiveIo { host: &mut host, resolved: &mut running.resolved };
        let frame = running.native.frame(&mut io, time, out, &mut lines);
        dispatch(frame, winds_for);
    });
    for line in lines {
        crate::log(&line);
    }
}

#[cfg(test)]
#[path = "datalink_native_tests.rs"]
mod tests;

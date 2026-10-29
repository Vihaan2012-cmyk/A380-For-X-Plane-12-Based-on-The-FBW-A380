use oit_pages::avncs::arinc;
use oit_pages::flt_ops::Runway;
use serde_json::Value;

/// What the OIT reads from and asks of the plugin.
pub trait Backend {
    /// A simulation or L: variable (`L:` prefix for L: variables).
    fn var(&mut self, name: &str, unit: &str) -> f64;
    /// A string simulation variable (`ATC ID`).
    fn text(&mut self, name: &str) -> String;
    fn set_var(&mut self, name: &str, value: f64);
    /// A plugin route (`/oit/docs`, `/perf/cal`, ...), polled until it answers.
    fn request(&mut self, key: &str, method: &str, path: &str, query: &str, body: &str) -> Option<(u16, String)>;
    /// An internet GET, polled until it answers.
    fn fetch(&mut self, key: &str, url: &str) -> Option<(u16, String)>;
    /// A FlyByWire setting (`CONFIG_SELF_TEST_TIME`).
    fn setting(&mut self, key: &str) -> Option<String>;
    /// The facility JSON MSFS's `LOAD_AIRPORT` answers with; `None` while
    /// pending, `Some(None)` for an unknown airport.
    fn airport(&mut self, icao: &str) -> Option<Option<String>>;
    /// The latest payload of a datalink bus topic.
    fn topic(&mut self, name: &str) -> Option<String>;
    fn fms(&mut self) -> FmsInfo;
    /// `oit_pages::avncs::MaintenanceModel` JSON for a maintenance uri.
    fn maintenance(&mut self, uri: &str, ata: Option<u32>) -> Value;
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FmsInfo {
    pub flight_number: Option<String>,
    pub origin: Option<String>,
    pub departure_runway: Option<String>,
    pub destination: Option<String>,
    pub arrival_runway: Option<String>,
}

pub fn arinc_value(b: &mut dyn Backend, name: &str) -> Option<f64> {
    let w = arinc(b.var(name, "number"));
    if w.normal() { Some(w.value) } else { None }
}

pub fn flag(b: &mut dyn Backend, name: &str) -> bool {
    b.var(name, "bool") != 0.0
}

fn designator(d: i64) -> &'static str {
    match d {
        1 => "L",
        2 => "R",
        3 => "C",
        4 => "W",
        5 => "A",
        6 => "B",
        _ => "",
    }
}

/// Both ends of every runway of an MSFS airport facility, sorted by ident.
pub fn runway_ends(airport: &Value) -> Vec<Runway> {
    let f = |v: &Value, k: &str| v.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    let mut out = Vec::new();
    for rwy in airport.get("runways").and_then(Value::as_array).into_iter().flatten() {
        let names: Vec<&str> = rwy.get("designation").and_then(Value::as_str).unwrap_or("").split('-').collect();
        for i in 0..2 {
            let primary = i == 0;
            let Some(number) = names.get(i).filter(|n| !n.is_empty()) else { continue };
            let d = rwy.get(if primary { "designatorCharPrimary" } else { "designatorCharSecondary" }).and_then(Value::as_i64).unwrap_or(0);
            let ident = format!("{:0>2}{}", number, designator(d));
            let this_threshold = f(rwy, if primary { "primaryThresholdLength" } else { "secondaryThresholdLength" });
            let far_overrun = f(rwy, if primary { "secondaryOverrunLength" } else { "primaryOverrunLength" });
            let this_elev = f(rwy, if primary { "primaryElevation" } else { "secondaryElevation" });
            let far_elev = f(rwy, if primary { "secondaryElevation" } else { "primaryElevation" });
            let length = f(rwy, "length");
            let between = (length - f(rwy, "primaryThresholdLength") - f(rwy, "secondaryThresholdLength")).max(1.0);
            let dir = f(rwy, "direction");
            out.push(Runway {
                ident,
                bearing_true: if primary { dir } else { (dir + 180.0) % 360.0 },
                tora: length,
                asda: length + far_overrun,
                lda: (length - this_threshold).max(0.0),
                elevation_ft: this_elev / 0.3048,
                slope_percent: (far_elev - this_elev) / between * 100.0,
                width: f(rwy, "width"),
            });
        }
    }
    out.sort_by(|a, b| a.ident.cmp(&b.ident));
    out
}

/// A confirm node: true once its input has been true for `time` seconds.
#[derive(Clone, Debug, Default)]
pub struct Confirm {
    time: f64,
    held: f64,
    out: bool,
}

impl Confirm {
    pub fn new(time: f64) -> Confirm {
        Confirm { time, held: 0.0, out: false }
    }

    pub fn write(&mut self, v: bool, dt: f64) {
        if v {
            self.held += dt;
            self.out = self.held >= self.time;
        } else {
            self.held = 0.0;
            self.out = false;
        }
    }

    pub fn read(&self) -> bool {
        self.out
    }

    pub fn reset(&mut self) {
        self.held = 0.0;
        self.out = false;
    }
}

/// Out/off/on/in times and fuel (the NSS ANSU's flight log), ms since the
/// Unix epoch and kg.
#[derive(Clone, Debug)]
pub struct Oooi {
    pb_off: (Option<f64>, Option<f64>),
    takeoff: (Option<f64>, Option<f64>),
    touchdown: (Option<f64>, Option<f64>),
    pb_on: (Option<f64>, Option<f64>),
    pub out: (Option<f64>, Option<f64>),
    pub off: (Option<f64>, Option<f64>),
    pub on: (Option<f64>, Option<f64>),
    pub inb: (Option<f64>, Option<f64>),
    pub block_ms: Option<f64>,
    pub flight_ms: Option<f64>,
    last_pb: Option<bool>,
    last_ground: Option<bool>,
    last_ms: Option<f64>,
    since: f64,
    off_conf: Confirm,
    on_conf: Confirm,
    turn_conf: Confirm,
}

impl Default for Oooi {
    fn default() -> Self {
        Oooi {
            pb_off: (None, None),
            takeoff: (None, None),
            touchdown: (None, None),
            pb_on: (None, None),
            out: (None, None),
            off: (None, None),
            on: (None, None),
            inb: (None, None),
            block_ms: None,
            flight_ms: None,
            last_pb: None,
            last_ground: None,
            last_ms: None,
            since: 0.0,
            off_conf: Confirm::new(10.0),
            on_conf: Confirm::new(10.0),
            turn_conf: Confirm::new(15.0 * 60.0),
        }
    }
}

pub struct OooiInputs {
    pub zulu_ms: f64,
    pub park_brake: bool,
    pub on_ground: bool,
    pub fob_kg: Option<f64>,
    pub door_open: f64,
    pub airspeed: f64,
    pub hyd_pressurised: bool,
}

impl Oooi {
    /// AnsuOps.reset(): the NSS ANSU lost its health, so the log is wiped (last_pb/last_ground stay, as FBW's subscriptions do).
    pub fn reset(&mut self) {
        for t in [&mut self.pb_off, &mut self.takeoff, &mut self.touchdown, &mut self.pb_on, &mut self.out, &mut self.off, &mut self.on, &mut self.inb] {
            *t = (None, None);
        }
        self.block_ms = None;
        self.flight_ms = None;
        self.last_ms = None;
        self.since = 0.0;
        self.off_conf.reset();
        self.on_conf.reset();
        self.turn_conf.reset();
    }

    pub fn update(&mut self, i: &OooiInputs) {
        if self.last_pb != Some(i.park_brake) {
            if i.park_brake {
                self.pb_on = (Some(i.zulu_ms), i.fob_kg);
            } else {
                self.pb_off = (Some(i.zulu_ms), i.fob_kg);
            }
            self.last_pb = Some(i.park_brake);
        }
        if self.last_ground != Some(i.on_ground) {
            if i.on_ground {
                self.touchdown = (Some(i.zulu_ms), i.fob_kg);
            } else {
                self.takeoff = (Some(i.zulu_ms), i.fob_kg);
            }
            self.last_ground = Some(i.on_ground);
        }
        let dt_ms = self.last_ms.map_or(1000.0, |l| i.zulu_ms - l);
        self.last_ms = Some(i.zulu_ms);
        self.since += dt_ms;
        if self.since < 500.0 || dt_ms == 0.0 {
            return;
        }
        let dt = self.since / 1000.0;
        self.since = 0.0;
        let all = self.out.0.is_some() && self.off.0.is_some() && self.on.0.is_some() && self.inb.0.is_some();
        self.turn_conf.write(all, dt);
        if (self.turn_conf.read() || self.out.0.is_none()) && !i.park_brake && i.door_open < 0.25 {
            if self.turn_conf.read() {
                self.off = (None, None);
                self.on = (None, None);
                self.inb = (None, None);
            }
            self.out = self.pb_off;
        }
        self.off_conf.write(self.out.0.is_some() && !i.on_ground, dt);
        if self.off.0.is_none() && self.out.0.is_some() && self.off_conf.read() {
            self.off = self.takeoff;
        }
        self.on_conf.write(i.on_ground && i.airspeed < 30.0, dt);
        if self.on.0.is_none() && self.off.0.is_some() && self.on_conf.read() {
            self.on = self.touchdown;
        }
        if self.inb.0.is_none() && self.on.0.is_some() && i.park_brake && i.door_open > 0.25 && !i.hyd_pressurised {
            self.inb = self.pb_on;
        }
        match self.out.0.filter(|t| *t != 0.0) {
            None => {
                self.block_ms = None;
                self.flight_ms = None;
            }
            Some(out) => {
                match self.inb.0.filter(|t| *t != 0.0) {
                    None => self.block_ms = Some(i.zulu_ms - out),
                    Some(inb) => self.block_ms = Some(inb - out),
                }
                if let Some(off) = self.off.0.filter(|t| *t != 0.0) {
                    match self.on.0.filter(|t| *t != 0.0) {
                        None => self.flight_ms = Some(i.zulu_ms - off),
                        Some(on) => self.flight_ms = Some(on - off),
                    }
                }
            }
        }
    }
}

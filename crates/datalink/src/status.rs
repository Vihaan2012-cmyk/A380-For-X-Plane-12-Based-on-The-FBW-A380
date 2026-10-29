use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtsuStatusCodes {
    Ok,
    CallsignInUse,
    OwnCallsign,
    NoAcarsConnection,
    NoTelexConnection,
    TelexDisabled,
    ComFailed,
    NoAtc,
    MailboxFull,
    UnknownMessage,
    ProxyError,
    NewAtisReceived,
    NoAtisReceived,
    SystemBusy,
    EntryOutOfRange,
    FormatError,
    NotInDatabase,
    RequestTimeout,
}

impl AtsuStatusCodes {
    pub fn code(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatalinkStatusCode {
    DlkAvail = 2,
    DlkNotAvail = 1,
    Inop = -1,
    NotInstalled = 0,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatalinkModeCode {
    AtcAoc = 1,
    Aoc = 2,
    Atc = 3,
    None = 0,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtsuFlightPhase {
    Preflight,
    Takeoff,
    Climb,
    Cruise,
    Descent,
    Approach,
    GoAround,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clock {
    pub year: i32,
    pub month: i32,
    pub day_of_month: i32,
    pub hour: i32,
    pub minute: i32,
    pub second: i32,
    pub seconds_of_day: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Waypoint {
    pub ident: String,
    pub altitude: f64,
    pub utc: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightStateData {
    pub lat: f64,
    pub lon: f64,
    pub altitude: f64,
    pub heading: f64,
    pub track: f64,
    pub indicated_airspeed: f64,
    pub ground_speed: f64,
    pub vertical_speed: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutopilotData {
    pub ap_active: bool,
    pub speed: f64,
    pub mach_mode: bool,
    pub altitude: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvironmentData {
    pub wind_direction: f64,
    pub wind_speed: f64,
    pub temperature: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PositionReportData {
    pub flight_state: FlightStateData,
    pub autopilot: AutopilotData,
    pub environment: EnvironmentData,
    pub last_waypoint: Option<Waypoint>,
    pub active_waypoint: Option<Waypoint>,
    pub next_waypoint: Option<Waypoint>,
    pub destination: Option<Waypoint>,
}

pub mod ecam;
pub mod phase;
pub mod pm;
pub mod radio;

pub use ecam::{ecam_voice_key, needs_confirmation, Doer, EcamItem, EcamProcedure, ItemKind, Switch};
pub use phase::{Phase, PhaseTracker};
pub use pm::PilotMonitoring;
pub use radio::{Station, StationKind};

pub type CalloutId = &'static str;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lever {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    AfterStart,
    Taxi,
    Climb10k,
    Descent10k,
    AfterLanding,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pa {
    SeatsForTakeoff,
    PrepareForLanding,
    SeatsForLanding,
    DisarmDoors,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interphone {
    Call,
    Push,
    Start,
    Stop,
    Disconnect,
    Chocks,
    Covers,
    Gpu,
    Asu,
    Pca,
}

impl Interphone {
    /// The `fbw/interphone/<request>` name in the plugin's interphone module.
    pub const fn request(self) -> &'static str {
        match self {
            Interphone::Call => "call",
            Interphone::Push => "push",
            Interphone::Start => "start",
            Interphone::Stop => "stop",
            Interphone::Disconnect => "disconnect",
            Interphone::Chocks => "chocks",
            Interphone::Covers => "covers",
            Interphone::Gpu => "gpu",
            Interphone::Asu => "asu",
            Interphone::Pca => "pca",
        }
    }
}

pub const CL_BEFORE_START: u8 = 1;
pub const CL_AFTER_START: u8 = 2;
pub const CL_TAXI: u8 = 3;
pub const CL_LINE_UP: u8 = 4;
pub const CL_APPROACH: u8 = 5;
pub const CL_LANDING: u8 = 6;
pub const CL_PARKING: u8 = 7;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TakeoffData {
    pub flaps: u8,
    pub v1_kt: f64,
    pub vr_kt: f64,
    pub v2_kt: f64,
    pub flex_temp_c: Option<i32>,
    pub ths_deg: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ApproachData {
    pub qnh_hpa: f64,
    pub temp_c: f64,
    pub wind_dir_deg: f64,
    pub wind_kt: f64,
    pub baro_minimum_ft: Option<f64>,
    pub radio_minimum_ft: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MfdEntry {
    Takeoff(TakeoffData),
    Approach(ApproachData),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Say(CalloutId),
    ReadEcamTitle { procedure: u32 },
    ReadEcam { procedure: u32, item: usize },
    TickEcam { procedure: u32, item: usize },
    Switch(Switch),
    GearLever(Lever),
    FlapLever(u8),
    Checklist(u8),
    Flow(Flow),
    Pa(Pa),
    Interphone(Interphone),
    AtcNotify,
    ReadFrequency(u32),
    TuneStandby { com: u8, khz: u32 },
    SwapCom(u8),
    Mfd(MfdEntry),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PfCommand {
    GearUp,
    GearDown,
    Flaps(u8),
    GoAroundFlaps,
    EcamActions,
    Confirm,
    Checklist(u8),
    Contact(StationKind),
    RequestPush,
    RequestStart,
    Disconnect,
    EnterTakeoffData,
    EnterApproachData,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handover {
    pub seq: u32,
    pub kind: StationKind,
    pub khz: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EcamLive {
    pub procedure: u32,
    /// Per procedure item: action completed, or condition true, as the FWS senses it.
    pub done: Vec<bool>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub on_ground: bool,
    pub ias_kt: f64,
    pub gs_kt: f64,
    pub ra_ft: f64,
    pub alt_ft: f64,
    pub vs_fpm: f64,
    pub pitch_deg: f64,
    pub bank_deg: f64,
    pub tla_max: f64,
    pub tla_min: f64,
    pub vr_kt: f64,
    pub vapp_kt: f64,
    /// VFE per flap lever position 0..=4; 0 when unknown.
    pub vfe_kt: [f64; 5],
    pub gear_handle_down: bool,
    pub gear_down_locked: bool,
    pub flap_lever: u8,
    pub spoilers_extended: bool,
    pub reversers_deployed: bool,
    pub decel_g: f64,
    pub autobrake_armed: bool,
    pub fcu_alt_ft: f64,
    pub trans_alt_ft: f64,
    pub trans_lvl_ft: f64,
    pub engines_running: u8,
    pub beacon: bool,
    pub landing_lights: bool,
    /// FBW FmgcFlightPhase: 0 preflight, 1 takeoff, 2 climb, 3 cruise, 4 descent, 5 approach, 6 go-around, 7 done.
    pub fmgc_phase: u8,
    pub com1_active_khz: u32,
    pub com1_standby_khz: u32,
    pub frequencies: Vec<Station>,
    pub handover: Option<Handover>,
    pub ecam_warning: Option<u32>,
    pub ecam: Option<EcamLive>,
    pub dest_dist_nm: Option<f64>,
    pub takeoff_data: Option<TakeoffData>,
    pub approach_data: Option<ApproachData>,
    pub mfd_takeoff_entered: bool,
    pub mfd_approach_entered: bool,
}

pub const ENGINES: u8 = 4;

/// Which jobs the F/O takes; each is an assist the pilot may keep for themselves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Duties {
    pub callouts: bool,
    pub flows: bool,
    pub checklists: bool,
    pub ecam: bool,
    pub radios: bool,
    pub cabin: bool,
    pub ground: bool,
    pub mfd: bool,
    pub auto_gear_up: bool,
}

impl Default for Duties {
    fn default() -> Self {
        Duties { callouts: true, flows: true, checklists: true, ecam: true, radios: true, cabin: true, ground: true, mfd: true, auto_gear_up: false }
    }
}

#[cfg(test)]
mod tests;

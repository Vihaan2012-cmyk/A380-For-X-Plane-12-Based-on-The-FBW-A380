pub const DOORS: [&str; 18] = [
    "M1L", "M1R", "M2L", "M2R", "M3L", "M3R", "M4L", "M4R", "M5L", "M5R", "U1L", "U1R", "U2L", "U2R", "U3L", "U3R", "CARGO_FWD", "CARGO_AFT",
];
pub const PAX_DOORS: usize = 16;
pub const ZONES: usize = 3;
pub const AREAS: usize = 6;
pub const AREA_NAMES: [&str; AREAS] = ["MD FWD", "MD MID", "MD AFT", "UD FWD", "UD MID", "UD AFT"];
pub const SMOKE_LOCS: usize = 14;
pub const SMOKE_NAMES: [&str; SMOKE_LOCS] = [
    "LAV MD FWD", "LAV MD MID", "LAV MD AFT", "LAV UD", "GALLEY FWD", "GALLEY MID", "GALLEY AFT", "IFE FWD", "IFE MID", "IFE AFT", "CARGO FWD",
    "CARGO AFT", "CARGO BULK", "CREW REST",
];
pub const SMOKE_CARGO: [usize; 3] = [10, 11, 12];
pub const ANNOUNCEMENTS: usize = 12;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sw3 {
    Off,
    #[default]
    Auto,
    On,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EmerSw {
    Off,
    #[default]
    Armed,
    On,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    White,
    Cyan,
    Amber,
    Red,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Caution {
    pub level: Level,
    pub text: String,
}

impl Caution {
    pub fn new(level: Level, text: impl Into<String>) -> Self {
        Self { level, text: text.into() }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Parked,
    TaxiOut,
    TakeoffRoll,
    Climb,
    Cruise,
    Descent,
    Approach,
    Rollout,
    TaxiIn,
}

impl Phase {
    pub fn departing(self) -> bool {
        matches!(self, Phase::TaxiOut | Phase::TakeoffRoll)
    }

    pub fn airborne(self) -> bool {
        matches!(self, Phase::Climb | Phase::Cruise | Phase::Descent | Phase::Approach)
    }

    pub fn ready_group(self) -> u8 {
        match self {
            Phase::Parked | Phase::TaxiOut | Phase::TakeoffRoll => 0,
            Phase::Climb | Phase::Cruise => 1,
            Phase::Descent | Phase::Approach | Phase::Rollout => 2,
            Phase::TaxiIn => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Status,
    Music,
    PreAnn,
    Lights,
    Doors,
    Temp,
    Water,
    Smoke,
    Seats,
    Ready,
    SysInfo,
    Calls,
    Evac,
    Maintenance,
}

impl Page {
    pub const ALL: [Page; 14] = [
        Page::Status,
        Page::Music,
        Page::PreAnn,
        Page::Lights,
        Page::Doors,
        Page::Temp,
        Page::Water,
        Page::Smoke,
        Page::Seats,
        Page::Ready,
        Page::SysInfo,
        Page::Calls,
        Page::Evac,
        Page::Maintenance,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Page::Status => "CABIN STATUS",
            Page::Music => "MUSIC",
            Page::PreAnn => "PRE ANNOUNCEMENT",
            Page::Lights => "CABIN LIGHTING",
            Page::Doors => "DOORS / SLIDES",
            Page::Temp => "CABIN TEMPERATURE",
            Page::Water => "WATER / WASTE",
            Page::Smoke => "SMOKE DETECTION",
            Page::Seats => "SEAT SETTING",
            Page::Ready => "CABIN READY",
            Page::SysInfo => "SYSTEM INFO",
            Page::Calls => "CALLS",
            Page::Evac => "EVACUATION",
            Page::Maintenance => "MAINTENANCE",
        }
    }

    pub fn index(self) -> usize {
        Page::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestKind {
    Lamp,
    Pa,
    Directors,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    SelectPage(Page),
    NextPage,
    PrevPage,
    ScreenOff,
    ScreenOn,
    PcReset,
    LockScreen,
    Door(usize),
    Announce(u8),
    Music(bool),
    MusicTrack(u8),
    MusicVolume(u8),
    StopAudio,
    Scene(u8),
    Level(u8),
    LightsMain,
    CallReset,
    CabinReady,
    AreaReady(usize),
    EvacCmd,
    EvacReset,
    SmokeReset,
    EmerLight(bool),
    PaxSys(bool),
    SeatPower(bool),
    WaterHeaters(bool),
    Galley(usize, bool),
    TempSet(usize, f64),
    TempStep(usize, f64),
    Test(TestKind),
    LavMaint,
}

impl Event {
    pub fn is_hard_key(&self) -> bool {
        matches!(
            self,
            Event::PcReset
                | Event::LockScreen
                | Event::EvacCmd
                | Event::EvacReset
                | Event::SmokeReset
                | Event::CallReset
                | Event::EmerLight(_)
                | Event::PaxSys(_)
                | Event::SeatPower(_)
                | Event::LightsMain
                | Event::LavMaint
                | Event::ScreenOn
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct Cockpit {
    pub calls_mech: bool,
    pub calls_all: bool,
    pub calls_fwd: bool,
    pub calls_aft: bool,
    pub calls_purs: bool,
    pub emer_call_on: bool,
    pub evac_cmd: bool,
    pub evac_horn_shutoff: bool,
    pub evac_capt_only: bool,
    pub seatbelt: Sw3,
    pub no_smoking: Sw3,
    pub emer_exit: EmerSw,
}

#[derive(Clone, Debug, Default)]
pub struct Power {
    pub ac_cabin: bool,
    pub ac_ess_shed: bool,
    pub dc_ess: bool,
    pub cids_bus: [bool; 3],
}

impl Power {
    pub fn normal() -> Self {
        Self { ac_cabin: true, ac_ess_shed: true, dc_ess: true, cids_bus: [true; 3] }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Flight {
    pub on_ground: bool,
    pub gs_kt: f64,
    pub engine_state: [f64; 4],
    pub n1_pct: [f64; 4],
    pub slats_deg: f64,
    pub gear_down: bool,
    pub radio_alt_ft: f64,
    pub vs_fpm: f64,
    pub alt_ft: f64,
    pub cabin_alt_ft: f64,
    pub cabin_dp_psi: f64,
    pub parking_brake: bool,
    pub beacon: bool,
    pub auto_announcement: Option<u8>,
}

impl Flight {
    pub fn engines_running(&self) -> bool {
        self.n1_pct.iter().any(|n| *n > 5.) || self.engine_state.iter().any(|s| *s != 0.)
    }

    pub fn engines_off(&self) -> bool {
        self.engine_state.iter().all(|s| *s == 0.)
    }

    pub fn stationary(&self) -> bool {
        self.on_ground && self.gs_kt < 1.
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DoorIn {
    pub open: f64,
    pub slide_armed: bool,
    pub latch_fault: bool,
    pub slide_press_low: bool,
}

#[derive(Clone, Debug)]
pub struct CabinIn {
    pub smoke: [bool; SMOKE_LOCS],
    pub waste_level_pct: [f64; ZONES],
    pub waste_full: [bool; ZONES],
    pub lav_inop: [bool; ZONES],
    pub water_qty_pct: f64,
    pub water_fault: bool,
    pub channel_fault: [f64; 4],
    pub director_fault: [bool; 3],
    pub purser_temp_sel_fault: bool,
    pub zone_temp_c: [f64; AREAS],
    pub ckpt_temp_c: f64,
}

impl Default for CabinIn {
    fn default() -> Self {
        Self {
            smoke: [false; SMOKE_LOCS],
            waste_level_pct: [0.; ZONES],
            waste_full: [false; ZONES],
            lav_inop: [false; ZONES],
            water_qty_pct: 100.,
            water_fault: false,
            channel_fault: [0.; 4],
            director_fault: [false; 3],
            purser_temp_sel_fault: false,
            zone_temp_c: [24.; AREAS],
            ckpt_temp_c: 24.,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Crew {
    pub events: Vec<Event>,
    pub attendant_call: [bool; ZONES],
    pub purser_call: bool,
    pub cabin_emer_call: bool,
    pub ground_mech_call: bool,
    pub handset_pa: bool,
    pub cockpit_pa: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Inputs {
    pub cockpit: Cockpit,
    pub power: Power,
    pub flight: Flight,
    pub doors: [DoorIn; 18],
    pub cabin: CabinIn,
    pub crew: Crew,
}

impl Inputs {
    pub fn parked() -> Self {
        Self {
            flight: Flight { on_ground: true, cabin_alt_ft: 0., ..Default::default() },
            power: Power::normal(),
            ..Default::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Cockpit,
    Cabin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChimeKind {
    CabinCall,
    PurserCall,
    EmergencyCall,
    PilotCall,
    MechCall,
    Sign,
    CabinReady,
    DoorWarning,
    TestTone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chime {
    pub target: Target,
    pub kind: ChimeKind,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PaSource {
    #[default]
    None,
    Music,
    PreRecorded(u8),
    Attendant,
    Cockpit,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub announcement_len_s: f64,
    pub boarding_music_auto: bool,
    pub strict_cabin_ready: bool,
    pub evac_guard_s: f64,
    pub call_light_s: f64,
    pub emer_repeat_s: f64,
    pub boot_s: f64,
    pub lock_s: f64,
    pub msg_s: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            announcement_len_s: 25.,
            boarding_music_auto: true,
            strict_cabin_ready: false,
            evac_guard_s: 3.,
            call_light_s: 30.,
            emer_repeat_s: 6.,
            boot_s: 9.,
            lock_s: 30.,
            msg_s: 4.,
        }
    }
}

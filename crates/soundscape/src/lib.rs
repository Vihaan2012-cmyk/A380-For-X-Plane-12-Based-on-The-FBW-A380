use std::collections::BTreeMap;

pub mod limits;

use limits::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sound {
    Contactor,
    AcBusUp,
    TrHum,
    HydEmp,
    HydEdp,
    FuelPumps,
    AvionicsVent,
    CockpitAir,
    PackAcm,
    RecircFans,
    BleedValve,
    StartValve,
    ApuStart,
    ApuRun,
    ApuShutdown,
    ApuFlap,
    GearMotion,
    GearUplock,
    GearDownlock,
    GearDoors,
    CargoDoor,
    CargoDoorLatch,
    BrakeSqueal,
    FlapMotor,
    FlapStop,
    SlatMotor,
    SlatStop,
    Spoilers,
    WiperSlow,
    WiperFast,
    WiperPark,
    Rain,
    Hail,
    CockpitDoorOpen,
    CockpitDoorClose,
    SeatMotor,
    SeatStop,
    OxyMaskTest,
    CvrTest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id {
    pub sound: Sound,
    pub index: u8,
}

const fn id(sound: Sound, index: u8) -> Id {
    Id { sound, index }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location {
    Overhead,
    Pedestal,
    FlightDeckAir,
    AvionicsBay,
    CockpitDoor,
    Seat(Side),
    Windshield(Side),
    NoseGearBay,
    MainGearBay,
    PackBay(Side),
    WingRoot,
    Wing(Side),
    Engine(u8),
    ApuTail,
    CargoDoor(u8),
}

impl Location {
    pub fn node(self) -> &'static str {
        match self {
            Location::Overhead | Location::AvionicsBay => "SOUND_SPEAKER_OHVD",
            Location::Pedestal => "PEDALS_LEFT",
            Location::FlightDeckAir | Location::Windshield(Side::Left) => "SOUND_WIPER_L",
            Location::Windshield(Side::Right) => "SOUND_WIPER_R",
            Location::CockpitDoor => "SOUND_FWD_GALLEY",
            Location::Seat(Side::Left) => "SEAT_CPT_MOVE_FWD_AFT",
            Location::Seat(Side::Right) => "SEAT_FO_MOVE_FWD_AFT",
            Location::NoseGearBay => "WHEELS_C",
            Location::MainGearBay | Location::WingRoot => "WHEELS_L",
            Location::PackBay(Side::Left) => "SOUND_PACK_OUTFLOW_LEFT",
            Location::PackBay(Side::Right) => "SOUND_PACK_OUTFLOW_RIGHT",
            Location::Wing(Side::Left) => "WHEELS_L",
            Location::Wing(Side::Right) => "WHEELS_R",
            Location::Engine(1) => "SOUND_ENGINE_1",
            Location::Engine(2) => "SOUND_ENGINE_2",
            Location::Engine(3) => "SOUND_ENGINE_3",
            Location::Engine(_) => "SOUND_ENGINE_4",
            Location::ApuTail => "FX_EXHAUST_APU",
            Location::CargoDoor(_) => "WHEELS_C",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Start { id: Id, looped: bool, gain: f32, pitch: f32, at: Location },
    Set { id: Id, gain: f32, pitch: f32 },
    Stop { id: Id, fade_s: f32 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WiperMode {
    #[default]
    Off,
    Slow,
    Fast,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub paused: bool,
    pub ac_buses: [bool; 4],
    pub dc_ess: bool,
    pub contactors: Vec<bool>,
    pub emp_rpm: [f64; 4],
    pub edp_pressurising: [bool; 8],
    pub fuel_pumps: Vec<bool>,
    pub avionics_vent: bool,
    pub pack_flow_kg_s: [f64; 2],
    pub recirc_fans: bool,
    pub bleed_valves: Vec<bool>,
    pub start_valves: [bool; 4],
    pub apu_n: f64,
    pub apu_flap: f64,
    pub gear: [f64; 5],
    pub gear_doors: [f64; 3],
    pub cargo_doors: [f64; 3],
    pub on_ground: bool,
    pub ground_speed_kt: f64,
    pub brake_demand: f64,
    pub flaps: f64,
    pub slats: f64,
    pub spoilers: f64,
    pub wipers: [WiperMode; 2],
    pub precip_mm: f64,
    pub hail: f64,
    pub tas_kt: f64,
    pub cockpit_door_locked: bool,
    pub seats: [f64; 4],
    pub oxy_mask_test: [bool; 2],
    pub cvr_test: bool,
    pub cvr_ground_control: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Latch {
    on: f64,
    off: f64,
    pub state: bool,
}

impl Latch {
    pub const fn new(on: f64, off: f64) -> Self {
        Self { on, off, state: false }
    }

    pub fn step(&mut self, x: f64) -> bool {
        if self.state {
            self.state = x >= self.off;
        } else {
            self.state = x >= self.on;
        }
        self.state
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Debounce {
    hold_s: f64,
    min_gap_s: f64,
    settled: Option<bool>,
    announced: bool,
    candidate: bool,
    held_s: f64,
    since_fire_s: f64,
}

impl Debounce {
    pub const fn new(hold_s: f64, min_gap_s: f64) -> Self {
        Self { hold_s, min_gap_s, settled: None, announced: false, candidate: false, held_s: 0., since_fire_s: f64::INFINITY }
    }

    pub fn step(&mut self, value: bool, dt: f64) -> Option<bool> {
        self.since_fire_s += dt;
        let Some(settled) = self.settled else {
            self.settled = Some(value);
            self.announced = value;
            self.candidate = value;
            return None;
        };
        if value != self.candidate {
            self.candidate = value;
            self.held_s = 0.;
        } else {
            self.held_s += dt;
        }
        if self.candidate != settled && self.held_s >= self.hold_s {
            self.settled = Some(self.candidate);
        }
        let settled = self.settled.unwrap_or(self.announced);
        if settled == self.announced || self.since_fire_s < self.min_gap_s {
            return None;
        }
        self.announced = settled;
        self.since_fire_s = 0.;
        Some(settled)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Mover {
    rate_eps: f64,
    still_s: f64,
    last: Option<f64>,
    still_for_s: f64,
    pub moving: bool,
    pub rate: f64,
}

impl Mover {
    pub const fn new(rate_eps: f64, still_s: f64) -> Self {
        Self { rate_eps, still_s, last: None, still_for_s: 0., moving: false, rate: 0. }
    }

    pub fn step(&mut self, x: f64, dt: f64) -> bool {
        let Some(last) = self.last.replace(x) else { return false };
        if dt <= 0. {
            return false;
        }
        self.rate = (x - last) / dt;
        if self.rate.abs() > self.rate_eps {
            self.moving = true;
            self.still_for_s = 0.;
            return false;
        }
        if !self.moving {
            return false;
        }
        self.still_for_s += dt;
        if self.still_for_s >= self.still_s {
            self.moving = false;
            return true;
        }
        false
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Spool {
    pub speed: f64,
}

impl Spool {
    pub fn step(&mut self, driven: bool, up_s: f64, down_s: f64, dt: f64) -> f64 {
        let (target, tau) = if driven { (1., up_s) } else { (0., down_s) };
        self.speed += (target - self.speed) * (1. - (-dt / tau.max(1e-6)).exp());
        if !driven && self.speed < 1e-3 {
            self.speed = 0.;
        }
        self.speed
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Travel {
    Unknown,
    Up,
    Transit,
    Down,
}

#[derive(Clone, Copy, Debug)]
struct Ends {
    travel: Travel,
}

impl Ends {
    const fn new() -> Self {
        Self { travel: Travel::Unknown }
    }

    fn step(&mut self, x: f64) -> Option<bool> {
        let now = if x <= END_LOW_IN {
            Travel::Up
        } else if x >= END_HIGH_IN {
            Travel::Down
        } else if x >= END_LOW_OUT && x <= END_HIGH_OUT {
            Travel::Transit
        } else {
            return None;
        };
        let was = std::mem::replace(&mut self.travel, now);
        match (was, now) {
            (Travel::Transit, Travel::Up) => Some(false),
            (Travel::Transit, Travel::Down) => Some(true),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    gain: f32,
    pitch: f32,
}

pub struct Soundscape {
    playing: BTreeMap<Id, Slot>,
    out: Vec<Command>,
    contactors: Vec<Debounce>,
    ac_buses: [Debounce; 4],
    emp: [Latch; 4],
    cockpit_air: Latch,
    pack_acm: [Latch; 2],
    avionics_vent: Spool,
    recirc: Spool,
    bleed_valves: Vec<Debounce>,
    start_valves: [Debounce; 4],
    apu_lit: Latch,
    apu_up: Latch,
    apu_last_n: Option<f64>,
    apu_flap: Mover,
    gear: Mover,
    gear_ends: [Ends; 5],
    gear_doors: [Mover; 3],
    cargo: [Mover; 3],
    cargo_ends: [Ends; 3],
    squeal_held_s: f64,
    squeal_on: bool,
    flaps: Mover,
    slats: Mover,
    spoilers: Mover,
    wipers: [WiperMode; 2],
    rain: Latch,
    hail: Latch,
    door: Debounce,
    seats: [Mover; 4],
    first: bool,
}

impl Default for Soundscape {
    fn default() -> Self {
        Self::new()
    }
}

impl Soundscape {
    pub fn new() -> Self {
        Self {
            playing: BTreeMap::new(),
            out: Vec::new(),
            contactors: Vec::new(),
            ac_buses: [Debounce::new(CONTACTOR_HOLD_S, AC_BUS_GAP_S); 4],
            emp: [Latch::new(EMP_ON_RPM, EMP_OFF_RPM); 4],
            cockpit_air: Latch::new(COCKPIT_AIR_ON_KG_S, COCKPIT_AIR_OFF_KG_S),
            pack_acm: [Latch::new(PACK_ACM_ON_KG_S, PACK_ACM_OFF_KG_S); 2],
            avionics_vent: Spool::default(),
            recirc: Spool::default(),
            bleed_valves: Vec::new(),
            start_valves: [Debounce::new(CONTACTOR_HOLD_S, CONTACTOR_GAP_S); 4],
            apu_lit: Latch::new(APU_AUDIBLE_ON_N, APU_AUDIBLE_OFF_N),
            apu_up: Latch::new(APU_RUNNING_ON_N, APU_RUNNING_OFF_N),
            apu_last_n: None,
            apu_flap: Mover::new(APU_FLAP_RATE_EPS, MOTOR_STILL_S),
            gear: Mover::new(GEAR_RATE_EPS, MOTOR_STILL_S),
            gear_ends: [Ends::new(); 5],
            gear_doors: [Mover::new(GEAR_RATE_EPS, MOTOR_STILL_S); 3],
            cargo: [Mover::new(CARGO_RATE_EPS, MOTOR_STILL_S); 3],
            cargo_ends: [Ends::new(); 3],
            squeal_held_s: 0.,
            squeal_on: false,
            flaps: Mover::new(SURFACE_RATE_EPS, FLAPS_STILL_S),
            slats: Mover::new(SURFACE_RATE_EPS, FLAPS_STILL_S),
            spoilers: Mover::new(SPOILER_RATE_EPS, MOTOR_STILL_S),
            wipers: [WiperMode::Off; 2],
            rain: Latch::new(RAIN_ON_MM, RAIN_OFF_MM),
            hail: Latch::new(HAIL_ON, HAIL_OFF),
            door: Debounce::new(CONTACTOR_HOLD_S, DOOR_GAP_S),
            seats: [Mover::new(SEAT_RATE_EPS, SEAT_STILL_S); 4],
            first: true,
        }
    }

    pub fn playing(&self, id: Id) -> bool {
        self.playing.contains_key(&id)
    }

    pub fn step(&mut self, s: &Snapshot, dt: f64) -> Vec<Command> {
        self.out.clear();
        if s.paused {
            let ids: Vec<Id> = self.playing.keys().copied().collect();
            for id in ids {
                self.stop(id, 0.);
            }
            return std::mem::take(&mut self.out);
        }
        self.electrical(s, dt);
        self.hydraulics(s);
        self.fuel(s);
        self.air(s, dt);
        self.apu(s, dt);
        self.gear(s, dt);
        self.doors(s, dt);
        self.brakes(s, dt);
        self.surfaces(s, dt);
        self.windshield(s);
        self.cabin(s, dt);
        self.first = false;
        std::mem::take(&mut self.out)
    }

    fn shot(&mut self, id: Id, gain: f64, pitch: f64, at: Location) {
        if self.first {
            return;
        }
        self.out.push(Command::Start { id, looped: false, gain: gain as f32, pitch: pitch as f32, at });
    }

    fn stop(&mut self, id: Id, fade_s: f32) {
        if self.playing.remove(&id).is_some() {
            self.out.push(Command::Stop { id, fade_s });
        }
    }

    fn drive(&mut self, id: Id, on: bool, gain: f64, pitch: f64, at: Location, fade_s: f32) {
        if !on {
            self.stop(id, fade_s);
            return;
        }
        let (gain, pitch) = (gain as f32, pitch as f32);
        match self.playing.get_mut(&id) {
            None => {
                self.playing.insert(id, Slot { gain, pitch });
                self.out.push(Command::Start { id, looped: true, gain, pitch, at });
            }
            Some(slot) => {
                if (slot.gain - gain).abs() > SET_GAIN_EPS || (slot.pitch - pitch).abs() > SET_PITCH_EPS {
                    *slot = Slot { gain, pitch };
                    self.out.push(Command::Set { id, gain, pitch });
                }
            }
        }
    }

    fn electrical(&mut self, s: &Snapshot, dt: f64) {
        if self.contactors.len() != s.contactors.len() {
            self.contactors = vec![Debounce::new(CONTACTOR_HOLD_S, CONTACTOR_GAP_S); s.contactors.len()];
            for (d, &c) in self.contactors.iter_mut().zip(&s.contactors) {
                d.step(c, 0.);
            }
        }
        for k in 0..s.contactors.len() {
            if let Some(closed) = self.contactors[k].step(s.contactors[k], dt) {
                let pitch = if closed { 1. } else { CONTACTOR_OPEN_PITCH };
                self.shot(id(Sound::Contactor, k as u8), CONTACTOR_GAIN, pitch, Location::AvionicsBay);
            }
        }
        for k in 0..4 {
            if self.ac_buses[k].step(s.ac_buses[k], dt) == Some(true) {
                self.shot(id(Sound::AcBusUp, k as u8), AC_BUS_GAIN, 1., Location::Overhead);
            }
        }
        let ac = s.ac_buses.iter().any(|b| *b);
        self.drive(id(Sound::TrHum, 0), ac && s.dc_ess, TR_HUM_GAIN, 1., Location::Overhead, 1.);
    }

    fn hydraulics(&mut self, s: &Snapshot) {
        for k in 0..4 {
            let rpm = s.emp_rpm[k];
            let on = self.emp[k].step(rpm);
            let ratio = (rpm / EMP_NOMINAL_RPM).clamp(0., 1.2);
            self.drive(id(Sound::HydEmp, k as u8), on, EMP_GAIN_MIN + EMP_GAIN_SPAN * ratio.min(1.), ratio.clamp(EMP_PITCH_MIN, EMP_PITCH_MAX), Location::MainGearBay, EMP_FADE_S);
        }
        for k in 0..8 {
            self.drive(id(Sound::HydEdp, k as u8), s.edp_pressurising[k], EDP_GAIN, 1., Location::Engine(k as u8 / 2 + 1), 1.);
        }
    }

    fn fuel(&mut self, s: &Snapshot) {
        let total = s.fuel_pumps.len().max(1) as f64;
        let n = s.fuel_pumps.iter().filter(|p| **p).count() as f64;
        let gain = FUEL_GAIN_MIN + FUEL_GAIN_SPAN * (n / total).sqrt();
        self.drive(id(Sound::FuelPumps, 0), n > 0., gain, 1., Location::WingRoot, 1.);
    }

    fn air(&mut self, s: &Snapshot, dt: f64) {
        let ac = s.ac_buses.iter().any(|b| *b);
        let vent = self.avionics_vent.step(s.avionics_vent && ac, FAN_UP_S, FAN_DOWN_S, dt);
        self.drive(id(Sound::AvionicsVent, 0), vent > FAN_AUDIBLE, FAN_GAIN * vent, FAN_PITCH_MIN + (1. - FAN_PITCH_MIN) * vent, Location::AvionicsBay, 0.5);
        let recirc = self.recirc.step(s.recirc_fans && ac, FAN_UP_S, FAN_DOWN_S, dt);
        self.drive(id(Sound::RecircFans, 0), recirc > FAN_AUDIBLE, RECIRC_GAIN * recirc, FAN_PITCH_MIN + (1. - FAN_PITCH_MIN) * recirc, Location::CockpitDoor, 0.5);
        let flow: f64 = s.pack_flow_kg_s.iter().sum();
        let on = self.cockpit_air.step(flow);
        let ratio = (flow / (2. * PACK_NOMINAL_KG_S)).clamp(0., 1.2);
        self.drive(id(Sound::CockpitAir, 0), on, COCKPIT_AIR_GAIN * ratio, 0.9 + 0.1 * ratio, Location::FlightDeckAir, COCKPIT_AIR_FADE_S);
        for k in 0..2 {
            let f = s.pack_flow_kg_s[k];
            let on = self.pack_acm[k].step(f);
            let ratio = (f / PACK_NOMINAL_KG_S).clamp(0., 1.2);
            let side = if k == 0 { Side::Left } else { Side::Right };
            self.drive(id(Sound::PackAcm, k as u8), on, PACK_ACM_GAIN * ratio, 0.8 + 0.2 * ratio, Location::PackBay(side), 1.);
        }
        if self.bleed_valves.len() != s.bleed_valves.len() {
            self.bleed_valves = vec![Debounce::new(CONTACTOR_HOLD_S, VALVE_GAP_S); s.bleed_valves.len()];
            for (d, &v) in self.bleed_valves.iter_mut().zip(&s.bleed_valves) {
                d.step(v, 0.);
            }
        }
        for k in 0..s.bleed_valves.len() {
            if let Some(open) = self.bleed_valves[k].step(s.bleed_valves[k], dt) {
                self.shot(id(Sound::BleedValve, k as u8), VALVE_GAIN, if open { 1. } else { VALVE_CLOSE_PITCH }, Location::WingRoot);
            }
        }
        for k in 0..4 {
            if let Some(open) = self.start_valves[k].step(s.start_valves[k], dt) {
                self.shot(id(Sound::StartValve, k as u8), VALVE_GAIN, if open { 1. } else { VALVE_CLOSE_PITCH }, Location::Overhead);
            }
        }
    }

    fn apu(&mut self, s: &Snapshot, dt: f64) {
        let n = s.apu_n.max(0.);
        let last = self.apu_last_n.replace(n).unwrap_or(n);
        let was_lit = self.apu_lit.state;
        let lit = self.apu_lit.step(n);
        let was_up = self.apu_up.state;
        let up = self.apu_up.step(n);
        if lit && !was_lit && n >= last && !self.first {
            self.shot(id(Sound::ApuStart, 0), APU_START_GAIN, 1., Location::ApuTail);
        }
        if was_up && !up && n < last {
            self.shot(id(Sound::ApuShutdown, 0), APU_SHUTDOWN_GAIN, 1., Location::ApuTail);
        }
        let ratio = (n / 100.).clamp(0., 1.05);
        self.drive(id(Sound::ApuRun, 0), lit, APU_GAIN_MIN + APU_GAIN_SPAN * ratio, APU_PITCH_MIN + (1. - APU_PITCH_MIN) * ratio, Location::ApuTail, APU_FADE_S);
        self.apu_flap.step(s.apu_flap, dt);
        self.drive(id(Sound::ApuFlap, 0), self.apu_flap.moving, APU_FLAP_GAIN, 1., Location::ApuTail, 0.3);
    }

    fn gear(&mut self, s: &Snapshot, dt: f64) {
        let mean = s.gear.iter().sum::<f64>() / s.gear.len() as f64;
        self.gear.step(mean, dt);
        self.drive(id(Sound::GearMotion, 0), self.gear.moving, GEAR_MOTION_GAIN, 1., Location::MainGearBay, 0.5);
        for k in 0..5 {
            if let Some(down) = self.gear_ends[k].step(s.gear[k]) {
                let at = if k == 0 { Location::NoseGearBay } else { Location::MainGearBay };
                let sound = if down { Sound::GearDownlock } else { Sound::GearUplock };
                self.shot(id(sound, k as u8), GEAR_LOCK_GAIN, 1., at);
            }
        }
        for k in 0..3 {
            self.gear_doors[k].step(s.gear_doors[k], dt);
            let at = if k == 0 { Location::NoseGearBay } else { Location::MainGearBay };
            let moving = self.gear_doors[k].moving;
            self.drive(id(Sound::GearDoors, k as u8), moving, GEAR_DOOR_GAIN, 1., at, 0.3);
        }
    }

    fn doors(&mut self, s: &Snapshot, dt: f64) {
        for k in 0..3 {
            self.cargo[k].step(s.cargo_doors[k], dt);
            let moving = self.cargo[k].moving;
            self.drive(id(Sound::CargoDoor, k as u8), moving, CARGO_GAIN, 1., Location::CargoDoor(k as u8), 0.3);
            if self.cargo_ends[k].step(s.cargo_doors[k]).is_some() {
                self.shot(id(Sound::CargoDoorLatch, k as u8), CARGO_LATCH_GAIN, 1., Location::CargoDoor(k as u8));
            }
        }
        if let Some(locked) = self.door.step(s.cockpit_door_locked, dt) {
            let sound = if locked { Sound::CockpitDoorClose } else { Sound::CockpitDoorOpen };
            self.shot(id(sound, 0), DOOR_GAIN, 1., Location::CockpitDoor);
        }
    }

    fn brakes(&mut self, s: &Snapshot, dt: f64) {
        let window = s.on_ground && s.ground_speed_kt > SQUEAL_MIN_KT && s.ground_speed_kt < SQUEAL_MAX_KT;
        let want = if self.squeal_on { window && s.brake_demand >= SQUEAL_OFF_DEMAND } else { window && s.brake_demand >= SQUEAL_ON_DEMAND };
        self.squeal_held_s = if want { self.squeal_held_s + dt } else { 0. };
        self.squeal_on = want && self.squeal_held_s >= SQUEAL_HOLD_S;
        let slow = 1. - (s.ground_speed_kt / SQUEAL_MAX_KT).clamp(0., 1.);
        let gain = SQUEAL_GAIN * s.brake_demand.clamp(0., 1.) * (0.4 + 0.6 * slow);
        self.drive(id(Sound::BrakeSqueal, 0), self.squeal_on, gain, 1. + 0.1 * (1. - slow), Location::MainGearBay, 0.2);
    }

    fn surfaces(&mut self, s: &Snapshot, dt: f64) {
        if self.flaps.step(s.flaps, dt) {
            self.shot(id(Sound::FlapStop, 0), FLAP_STOP_GAIN, 1., Location::WingRoot);
        }
        self.drive(id(Sound::FlapMotor, 0), self.flaps.moving, FLAP_MOTOR_GAIN, 1., Location::WingRoot, 0.3);
        if self.slats.step(s.slats, dt) {
            self.shot(id(Sound::SlatStop, 0), FLAP_STOP_GAIN, 1., Location::WingRoot);
        }
        self.drive(id(Sound::SlatMotor, 0), self.slats.moving, FLAP_MOTOR_GAIN, 1., Location::WingRoot, 0.3);
        self.spoilers.step(s.spoilers, dt);
        let rate = (self.spoilers.rate.abs() / SPOILER_FULL_RATE).clamp(0., 1.);
        let moving = self.spoilers.moving;
        self.drive(id(Sound::Spoilers, 0), moving, SPOILER_GAIN * (0.3 + 0.7 * rate), 1., Location::Wing(Side::Left), 0.3);
    }

    fn windshield(&mut self, s: &Snapshot) {
        for k in 0..2 {
            let side = if k == 0 { Side::Left } else { Side::Right };
            let mode = s.wipers[k];
            let at = Location::Windshield(side);
            self.drive(id(Sound::WiperSlow, k as u8), mode == WiperMode::Slow, WIPER_GAIN, 1., at, 0.2);
            self.drive(id(Sound::WiperFast, k as u8), mode == WiperMode::Fast, WIPER_GAIN, 1., at, 0.2);
            if mode == WiperMode::Off && self.wipers[k] != WiperMode::Off {
                self.shot(id(Sound::WiperPark, k as u8), WIPER_GAIN, 1., at);
            }
            self.wipers[k] = mode;
        }
        let speed = (s.tas_kt / RAIN_FULL_TAS_KT).clamp(0., 1.);
        let on = self.rain.step(s.precip_mm);
        let gain = RAIN_GAIN * (s.precip_mm / RAIN_FULL_MM).clamp(0., 1.).sqrt() * (0.6 + 0.4 * speed);
        self.drive(id(Sound::Rain, 0), on, gain, 1., Location::FlightDeckAir, 1.);
        let on = self.hail.step(s.hail);
        let gain = HAIL_GAIN * s.hail.clamp(0., 1.) * (0.4 + 0.6 * speed);
        self.drive(id(Sound::Hail, 0), on, gain, 1., Location::FlightDeckAir, 1.5);
    }

    fn cabin(&mut self, s: &Snapshot, dt: f64) {
        for k in 0..4 {
            let side = if k < 2 { Side::Left } else { Side::Right };
            if self.seats[k].step(s.seats[k], dt) {
                self.shot(id(Sound::SeatStop, k as u8), SEAT_GAIN, 1., Location::Seat(side));
            }
            let moving = self.seats[k].moving;
            self.drive(id(Sound::SeatMotor, k as u8), moving, SEAT_GAIN, 1., Location::Seat(side), 0.1);
        }
        for k in 0..2 {
            let side = if k == 0 { Side::Left } else { Side::Right };
            self.drive(id(Sound::OxyMaskTest, k as u8), s.oxy_mask_test[k], OXY_GAIN, 1., Location::Seat(side), 0.2);
        }
        self.drive(id(Sound::CvrTest, 0), s.cvr_test && s.cvr_ground_control, CVR_GAIN, 1., Location::Pedestal, 0.1);
    }
}

#[cfg(test)]
mod tests;

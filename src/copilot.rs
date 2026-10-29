//! The AI first officer: Airbus PM callouts during takeoff, approach and
//! rollout, the optional "gear up" on positive climb, and a speech queue for
//! the EFB's checklist reading (`/study/action` kind `copilotSay`). The
//! recordings are `sound/copilot/<voice>/fo_<key>.wav`, made by
//! `tools/copilot_voices.py`.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::xp::{DataRef, Xplm};
use crate::Vars;

pub const VOICES: [&str; 3] = ["northern", "joe", "cori"];
const GAP_S: f64 = 0.25;
const QUEUE_MAX: usize = 24;

static QUEUE: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

pub fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 64 && key.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

pub fn say(key: &str) -> bool {
    if !valid_key(key) {
        return false;
    }
    let Ok(mut q) = QUEUE.lock() else { return false };
    if q.len() >= QUEUE_MAX {
        return false;
    }
    q.push_back(key.to_owned());
    true
}

pub fn hush() {
    if let Ok(mut q) = QUEUE.lock() {
        q.clear();
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Inputs {
    pub on_ground: bool,
    pub ias_kt: f64,
    pub gs_kt: f64,
    pub ra_ft: f64,
    pub vs_fpm: f64,
    pub tla_max: f64,
    pub tla_min: f64,
    pub vr_kt: f64,
    pub gear_down: f64,
    pub gear_handle_down: bool,
    pub flap_index: f64,
    pub spoilers: f64,
    pub reverse: f64,
    pub decel_g: f64,
    pub autobrake_mode: f64,
    pub alt_ft: f64,
    pub fcu_alt_ft: f64,
    pub trans_alt_ft: f64,
    pub trans_lvl_ft: f64,
    pub engines_running: Option<bool>,
    pub engines_off: Option<bool>,
    pub beacon: bool,
    pub landing_lights: bool,
    pub fmgc_phase: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Out {
    Say(&'static str),
    GearUp,
    AfterLanding,
    AfterStart,
    TaxiFlow,
    Checklist(u8),
}

static TAKEOFF_FLAPS: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// The take-off flap lever position (1 for 1+F, 2, 3) the EFB computed, for
/// the first officer's after-start flow.
pub fn set_takeoff_flaps(index: u8) {
    TAKEOFF_FLAPS.store(index, std::sync::atomic::Ordering::Relaxed);
}

pub fn takeoff_flaps() -> Option<u8> {
    match TAKEOFF_FLAPS.load(std::sync::atomic::Ordering::Relaxed) {
        0 => None,
        f => Some(f),
    }
}

pub const CL_BEFORE_START: u8 = 1;
pub const CL_AFTER_START: u8 = 2;
pub const CL_TAXI: u8 = 3;
pub const CL_LINE_UP: u8 = 4;
pub const CL_APPROACH: u8 = 5;
pub const CL_LANDING: u8 = 6;
pub const CL_PARKING: u8 = 7;
const AFTER_START_CHECKLIST_S: f64 = 25.0;
const FMGC_APPROACH: f64 = 5.0;

#[derive(Default)]
pub struct Callouts {
    takeoff: [bool; 4],
    gear_at: Option<f64>,
    approach_armed: bool,
    airborne: bool,
    touchdown_at: Option<f64>,
    landing: [bool; 4],
    reverse_selected_at: Option<f64>,
    t: f64,
    last_alt: Option<f64>,
    thousand_armed: bool,
    taxi_warned: bool,
    engines_were_running: Option<bool>,
    after_start_due: Option<f64>,
    flap_last: Option<f64>,
    flap_call_at: Option<f64>,
    gear_handle_last: Option<bool>,
    cl_fired: [bool; 8],
    cl_after_start_due: Option<f64>,
    landed: bool,
    beacon_last: Option<bool>,
    landing_lights_last: Option<bool>,
    phase_last: f64,
    was_high: bool,
}

const FLAP_SETTLE_S: f64 = 1.0;
const FLAP_CALLS: [&str; 5] = ["flaps_0", "flaps_1", "flaps_2", "flaps_3", "flaps_full"];

const AFTER_START_DELAY_S: f64 = 8.0;

const FLX_TLA: f64 = 34.0;
const REVERSE_TLA: f64 = -1.0;

impl Callouts {
    pub fn step(&mut self, i: &Inputs, dt: f64, gear_up: bool) -> Vec<Out> {
        self.step_with(i, dt, gear_up, false)
    }

    pub fn step_with(&mut self, i: &Inputs, dt: f64, gear_up: bool, sop: bool) -> Vec<Out> {
        let mut out = Vec::new();
        if sop {
            self.sop_calls(i, &mut out);
        }
        self.last_alt = Some(i.alt_ft);
        out.extend(self.core(i, dt, gear_up));
        self.after_start(i, &mut out);
        self.checklists(i, &mut out);
        out
    }

    fn checklist(&mut self, code: u8, out: &mut Vec<Out>) {
        if !self.cl_fired[code as usize] {
            self.cl_fired[code as usize] = true;
            out.push(Out::Checklist(code));
        }
    }

    fn checklists(&mut self, i: &Inputs, out: &mut Vec<Out>) {
        let t = self.t;
        let running = i.engines_running == Some(true);
        let off = i.engines_off == Some(true);
        let beacon_on = self.beacon_last == Some(false) && i.beacon;
        let lights_on = self.landing_lights_last == Some(false) && i.landing_lights;
        self.beacon_last = Some(i.beacon);
        self.landing_lights_last = Some(i.landing_lights);
        if i.on_ground {
            if beacon_on && off && !self.landed {
                self.checklist(CL_BEFORE_START, out);
            }
            if self.cl_after_start_due.is_some_and(|due| t >= due) {
                self.cl_after_start_due = None;
                if running {
                    self.checklist(CL_AFTER_START, out);
                }
            }
            if running && !self.landed && i.gs_kt > 3. && i.gs_kt < 40. && !self.cl_fired[CL_TAXI as usize] {
                out.push(Out::TaxiFlow);
                self.checklist(CL_TAXI, out);
            }
            if running && !self.landed && lights_on {
                self.checklist(CL_LINE_UP, out);
            }
            if self.landed && off {
                self.checklist(CL_PARKING, out);
                self.cl_fired = [false; 8];
                self.cl_fired[CL_PARKING as usize] = true;
                self.landed = false;
            }
            if running && self.cl_fired[CL_PARKING as usize] && !self.landed {
                self.cl_fired[CL_PARKING as usize] = false;
            }
        } else {
            self.cl_after_start_due = None;
            if i.alt_ft > 12_000. {
                self.was_high = true;
            }
            let approach = (i.fmgc_phase.round() == FMGC_APPROACH && self.phase_last.round() != FMGC_APPROACH)
                || (self.was_high && i.alt_ft < 10_000. && i.vs_fpm < -300.);
            if approach {
                self.checklist(CL_APPROACH, out);
            }
            if i.gear_handle_down && i.flap_index >= 3. && i.ra_ft > 0. && i.ra_ft < 3000. && i.vs_fpm < 0. {
                self.checklist(CL_LANDING, out);
            }
        }
        self.phase_last = i.fmgc_phase;
    }

    fn after_start(&mut self, i: &Inputs, out: &mut Vec<Out>) {
        let Some(running) = i.engines_running else { return };
        let was = self.engines_were_running.replace(running);
        let fresh_start = was == Some(false) && running && i.on_ground && i.gs_kt < 5. && !self.airborne && self.touchdown_at.is_none();
        if fresh_start {
            self.after_start_due = Some(self.t + AFTER_START_DELAY_S);
            self.cl_after_start_due = Some(self.t + AFTER_START_CHECKLIST_S);
        }
        if !running || !i.on_ground {
            self.after_start_due = None;
        }
        if self.after_start_due.is_some_and(|due| self.t >= due) {
            self.after_start_due = None;
            out.push(Out::AfterStart);
        }
    }

    fn sop_calls(&mut self, i: &Inputs, out: &mut Vec<Out>) {
        if i.on_ground {
            self.thousand_armed = false;
            if i.gs_kt > 30. && i.tla_max < FLX_TLA && !self.airborne && self.touchdown_at.is_none() && !self.taxi_warned {
                self.taxi_warned = true;
                out.push(Out::Say("taxi_speed"));
            } else if i.gs_kt < 20. {
                self.taxi_warned = false;
            }
            return;
        }
        let Some(prev) = self.last_alt else { return };
        let alt = i.alt_ft;
        if i.trans_alt_ft > 0. && i.vs_fpm > 0. && prev < i.trans_alt_ft && alt >= i.trans_alt_ft {
            out.push(Out::Say("transition_std"));
        }
        if i.trans_lvl_ft > 0. && i.vs_fpm < 0. && prev > i.trans_lvl_ft && alt <= i.trans_lvl_ft {
            out.push(Out::Say("transition_qnh"));
        }
        if i.fcu_alt_ft > 0. {
            let diff = (i.fcu_alt_ft - alt).abs();
            if diff > 1500. {
                self.thousand_armed = true;
            }
            let towards = (i.fcu_alt_ft - alt).signum() == i.vs_fpm.signum() && i.vs_fpm.abs() > 300.;
            if self.thousand_armed && towards && diff <= 1000. && (i.fcu_alt_ft - prev).abs() > 1000. {
                self.thousand_armed = false;
                out.push(Out::Say("1000_to_go"));
            }
        }
    }

    fn core(&mut self, i: &Inputs, dt: f64, gear_up: bool) -> Vec<Out> {
        let mut out = Vec::new();
        self.t += dt.max(0.);
        let t = self.t;
        if i.on_ground {
            if i.gs_kt < 30. && i.tla_max < 20. {
                self.takeoff = [false; 4];
                self.gear_at = None;
            }
            if self.airborne {
                self.airborne = false;
                self.landed = true;
                self.was_high = false;
                self.touchdown_at = Some(t);
                self.landing = [false; 4];
                self.reverse_selected_at = None;
            }
            match self.touchdown_at {
                Some(td) => self.rollout(i, t - td, t, &mut out),
                None => self.takeoff_roll(i, &mut out),
            }
            return out;
        }
        if i.ra_ft > 50. {
            self.airborne = true;
            self.touchdown_at = None;
        }
        self.selections(i, t, &mut out);
        if self.takeoff[0] && !self.takeoff[3] && i.vs_fpm > 300. && i.ra_ft > 20. {
            self.takeoff[3] = true;
            out.push(Out::Say("pos_climb"));
            if gear_up && i.gear_handle_down {
                self.gear_at = Some(t + 1.2);
            }
        }
        if let Some(at) = self.gear_at {
            if t >= at {
                self.gear_at = None;
                if i.gear_handle_down {
                    out.push(Out::GearUp);
                    out.push(Out::Say("gear_up"));
                }
            }
        }
        if i.ra_ft > 1500. {
            self.approach_armed = true;
        }
        if self.approach_armed && i.ra_ft <= 1000. && i.ra_ft > 300. && i.vs_fpm < -100. {
            self.approach_armed = false;
            let stable = i.gear_down >= 0.98 && i.flap_index >= 3. && i.vs_fpm > -1500. && i.tla_max < FLX_TLA;
            out.push(Out::Say(if stable { "stabilised" } else { "not_stabilised" }));
        }
        out
    }

    fn selections(&mut self, i: &Inputs, t: f64, out: &mut Vec<Out>) {
        let flap = i.flap_index.round();
        if self.flap_last.is_some_and(|f| f != flap) && self.airborne {
            self.flap_call_at = Some(t + FLAP_SETTLE_S);
        }
        self.flap_last = Some(flap);
        if self.flap_call_at.is_some_and(|at| t >= at) {
            self.flap_call_at = None;
            if let Some(call) = FLAP_CALLS.get(flap.clamp(0., 4.) as usize) {
                out.push(Out::Say(call));
            }
        }
        if self.gear_handle_last == Some(false) && i.gear_handle_down && self.airborne {
            out.push(Out::Say("gear_down"));
        }
        self.gear_handle_last = Some(i.gear_handle_down);
    }

    fn takeoff_roll(&mut self, i: &Inputs, out: &mut Vec<Out>) {
        if i.tla_max < FLX_TLA {
            return;
        }
        if !self.takeoff[0] && i.ias_kt >= 40. {
            self.takeoff[0] = true;
            out.push(Out::Say("thrust_set"));
        }
        if self.takeoff[0] && !self.takeoff[1] && i.ias_kt >= 100. {
            self.takeoff[1] = true;
            out.push(Out::Say("100kt"));
        }
        if self.takeoff[1] && !self.takeoff[2] && i.vr_kt >= 100. && i.ias_kt >= i.vr_kt {
            self.takeoff[2] = true;
            out.push(Out::Say("rotate"));
        }
    }

    fn rollout(&mut self, i: &Inputs, since: f64, t: f64, out: &mut Vec<Out>) {
        if i.gs_kt < 30. {
            if i.tla_min > REVERSE_TLA {
                self.touchdown_at = None;
                out.push(Out::AfterLanding);
            }
            return;
        }
        if !self.landing[0] {
            if i.spoilers >= 0.5 {
                self.landing[0] = true;
                out.push(Out::Say("spoilers"));
            } else if since >= 3. {
                self.landing[0] = true;
                out.push(Out::Say("no_spoilers"));
            }
        }
        if i.tla_min <= REVERSE_TLA {
            let at = *self.reverse_selected_at.get_or_insert(t);
            if !self.landing[1] {
                if i.reverse >= 0.9 {
                    self.landing[1] = true;
                    out.push(Out::Say("reverse_green"));
                } else if t - at >= 4. {
                    self.landing[1] = true;
                    out.push(Out::Say("no_reverse"));
                }
            }
        }
        if !self.landing[2] {
            if i.decel_g >= 0.1 {
                self.landing[2] = true;
                out.push(Out::Say("decel"));
            } else if since >= 6. && i.autobrake_mode > 0. {
                self.landing[2] = true;
                out.push(Out::Say("no_decel"));
            }
        }
        if !self.landing[3] && i.ias_kt <= 70. {
            self.landing[3] = true;
            out.push(Out::Say("70kt"));
        }
    }
}

struct Ids {
    tla: [VariableIdentifier; 4],
    vr: VariableIdentifier,
    flap_index: VariableIdentifier,
    spoilers: VariableIdentifier,
    autobrake: VariableIdentifier,
    fcu_alt: VariableIdentifier,
    trans_alt: VariableIdentifier,
    trans_lvl: VariableIdentifier,
    engine_state: [VariableIdentifier; 4],
    fmgc_phase: VariableIdentifier,
    cl_code: VariableIdentifier,
    cl_seq: VariableIdentifier,
}

fn arinc(raw: f64) -> Option<f64> {
    let bits = raw as u64;
    ((bits >> 32) & 0b11 == 0b11).then(|| f32::from_bits(bits as u32) as f64).filter(|v| v.is_finite())
}

struct Refs {
    on_ground: Option<DataRef>,
    ias: Option<DataRef>,
    gs: Option<DataRef>,
    ra: Option<DataRef>,
    vs: Option<DataRef>,
    alt: Option<DataRef>,
    gear: Option<DataRef>,
    gear_handle: Option<DataRef>,
    speedbrake: Option<DataRef>,
    reverser: Option<DataRef>,
    beacon: Option<DataRef>,
    landing_lights: Option<DataRef>,
}

pub struct Copilot {
    ids: Ids,
    refs: Refs,
    callouts: Callouts,
    voice: Option<String>,
    durations: HashMap<String, f64>,
    busy_until: Option<Instant>,
    last_gs: f64,
    decel_g: f64,
}

fn start_flow(vars: &mut Vars, preset: i64, name: &str) {
    let load = vars.get("AIRCRAFT_PRESET_LOAD".to_owned());
    if vars.read(&load) == 0. {
        vars.write(&load, preset as f64);
        crate::log(&format!("copilot: {name} flow"));
    }
}

pub fn voice_dir(voice: &str) -> Option<std::path::PathBuf> {
    crate::sound_pack::plugin_dir().map(|d| d.join("sound").join("copilot").join(voice))
}

fn durations(dir: &std::path::Path) -> HashMap<String, f64> {
    let mut out = HashMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return out };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(key) = name.strip_prefix("fo_").and_then(|n| n.strip_suffix(".wav")) else { continue };
        let Some(pcm) = std::fs::read(e.path()).ok().and_then(|b| crate::sound::voice_pack::parse_wav(&b)) else { continue };
        if pcm.sample_rate > 0 && pcm.channels > 0 {
            out.insert(key.to_owned(), pcm.samples.len() as f64 / pcm.channels as f64 / pcm.sample_rate as f64);
        }
    }
    out
}

impl Copilot {
    pub fn new(vars: &mut Vars, xplm: &Xplm) -> Self {
        let f = |n: &str| xplm.find(n);
        Self {
            ids: Ids {
                tla: [1, 2, 3, 4].map(|n| vars.get(format!("AUTOTHRUST_TLA:{n}"))),
                vr: vars.get_unprefixed("AIRLINER_VR_SPEED".into()),
                flap_index: vars.get("FLAPS_HANDLE_INDEX".into()),
                spoilers: vars.get("SPOILERS_HANDLE_POSITION".into()),
                autobrake: vars.get("AUTOBRAKES_ARMED_MODE".into()),
                fcu_alt: vars.get("FCU_AFS_DISPLAY_ALT_VALUE".into()),
                trans_alt: vars.get("FM1_TRANS_ALT".into()),
                trans_lvl: vars.get("FM1_TRANS_LVL".into()),
                engine_state: std::array::from_fn(|n| vars.get(format!("ENGINE_STATE:{}", n + 1))),
                fmgc_phase: vars.get("FMGC_FLIGHT_PHASE".into()),
                cl_code: vars.get("COPILOT_CL_CODE".into()),
                cl_seq: vars.get("COPILOT_CL_SEQ".into()),
            },
            refs: Refs {
                on_ground: f("sim/flightmodel/failures/onground_any"),
                ias: f("sim/flightmodel/position/indicated_airspeed"),
                gs: f("sim/flightmodel/position/groundspeed"),
                ra: f("sim/cockpit2/gauges/indicators/radio_altimeter_height_ft_pilot"),
                vs: f("sim/flightmodel/position/vh_ind_fpm"),
                alt: f("sim/cockpit2/gauges/indicators/altitude_ft_pilot"),
                gear: f("sim/flightmodel2/gear/deploy_ratio"),
                gear_handle: f("sim/cockpit2/controls/gear_handle_down"),
                speedbrake: f("sim/flightmodel2/controls/speedbrake_ratio"),
                reverser: f("sim/flightmodel2/engines/thrust_reverser_deploy_ratio"),
                beacon: f("sim/cockpit2/switches/beacon_on"),
                landing_lights: f("sim/cockpit2/switches/landing_lights_switch"),
            },
            callouts: Callouts::default(),
            voice: None,
            durations: HashMap::new(),
            busy_until: None,
            last_gs: 0.,
            decel_g: 0.,
        }
    }

    fn inputs(&mut self, vars: &mut Vars, xplm: &Xplm, delta: f64) -> Inputs {
        let r = |d: Option<DataRef>| d.map_or(0., |d| xplm.get_f(d) as f64);
        let mut gear = [0f32; 1];
        if let Some(d) = self.refs.gear {
            xplm.get_vf(d, &mut gear);
        }
        let mut rev = [0f32; 4];
        if let Some(d) = self.refs.reverser {
            xplm.get_vf(d, &mut rev);
        }
        let tla: Vec<f64> = self.ids.tla.iter().map(|id| vars.read(id)).collect();
        let gs = r(self.refs.gs);
        if delta > 0. {
            let decel = ((self.last_gs - gs) / delta / 9.80665).clamp(-1., 1.);
            self.decel_g += (decel - self.decel_g) * (1. - (-delta / 0.5).exp());
        }
        self.last_gs = gs;
        Inputs {
            on_ground: self.refs.on_ground.is_some_and(|d| xplm.get_i(d) != 0),
            ias_kt: r(self.refs.ias),
            gs_kt: gs * 1.943_844,
            ra_ft: r(self.refs.ra),
            vs_fpm: r(self.refs.vs),
            tla_max: tla.iter().copied().fold(f64::MIN, f64::max),
            tla_min: tla.iter().copied().fold(f64::MAX, f64::min),
            vr_kt: vars.read(&self.ids.vr),
            gear_down: gear[0] as f64,
            gear_handle_down: self.refs.gear_handle.is_some_and(|d| xplm.get_i(d) != 0),
            flap_index: vars.read(&self.ids.flap_index),
            spoilers: vars.read(&self.ids.spoilers).max(r(self.refs.speedbrake)),
            reverse: rev[1].max(rev[2]) as f64,
            decel_g: self.decel_g,
            autobrake_mode: vars.read(&self.ids.autobrake),
            alt_ft: r(self.refs.alt),
            fcu_alt_ft: vars.read(&self.ids.fcu_alt),
            trans_alt_ft: arinc(vars.read(&self.ids.trans_alt)).unwrap_or(0.),
            trans_lvl_ft: arinc(vars.read(&self.ids.trans_lvl)).map_or(0., |fl| fl * 100.),
            engines_running: Some(self.ids.engine_state.iter().all(|id| vars.read(id) == 1.)),
            engines_off: Some(self.ids.engine_state.iter().all(|id| vars.read(id) == 0.)),
            beacon: self.refs.beacon.is_some_and(|d| xplm.get_i(d) != 0),
            landing_lights: {
                let mut l = [0f32; 6];
                if let Some(d) = self.refs.landing_lights {
                    xplm.get_vf(d, &mut l);
                }
                l[2..].iter().any(|v| *v > 0.5)
            },
            fmgc_phase: vars.read(&self.ids.fmgc_phase),
        }
    }

    pub fn update(&mut self, vars: &mut Vars, xplm: &Xplm, delta: f64) {
        let settings = crate::app_settings::current();
        let voice = VOICES[(settings.copilot_voice as usize).min(VOICES.len() - 1)];
        if self.voice.as_deref() != Some(voice) {
            self.voice = Some(voice.to_owned());
            self.durations = voice_dir(voice).map(|d| durations(&d)).unwrap_or_default();
            crate::sound::set_copilot_voice(voice);
        }
        let inputs = self.inputs(vars, xplm, delta);
        let outs = if settings.copilot_callouts {
            self.callouts.step_with(&inputs, delta, settings.copilot_gear_up, settings.copilot_sop)
        } else {
            self.callouts.step(&inputs, delta, false)
        };
        for o in outs {
            match o {
                Out::Say(key) if settings.copilot_callouts => {
                    say(key);
                }
                Out::GearUp => {
                    crate::xp::command_once("sim/flight_controls/landing_gear_up");
                }
                Out::AfterLanding if settings.copilot_after_landing => {
                    start_flow(vars, crate::extra_backend::procedures::AFTER_LANDING_PRESET, "after landing");
                }
                Out::AfterStart if settings.copilot_after_start => {
                    start_flow(vars, crate::extra_backend::procedures::AFTER_START_PRESET, "after start");
                }
                Out::TaxiFlow if settings.copilot_after_start => {
                    start_flow(vars, crate::extra_backend::procedures::TAXI_FLOW_PRESET, "taxi");
                }
                Out::Checklist(code) if settings.copilot_auto_checklists => {
                    let seq = vars.read(&self.ids.cl_seq);
                    vars.write(&self.ids.cl_code, code as f64);
                    vars.write(&self.ids.cl_seq, seq + 1.);
                    crate::log(&format!("copilot: checklist {code} due"));
                }
                _ => {}
            }
        }
        let now = Instant::now();
        if self.busy_until.is_some_and(|b| now < b) {
            return;
        }
        let next = QUEUE.lock().ok().and_then(|mut q| q.pop_front());
        if let Some(mut key) = next {
            if !self.durations.contains_key(&key) && key.starts_with("r_") {
                key = "checked".into();
            }
            if let Some(length) = self.durations.get(&key).copied() {
                crate::sound::play_instrument_sound(&format!("fo_{key}"));
                self.busy_until = Some(now + Duration::from_secs_f64(length + GAP_S));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(c: &mut Callouts, i: &Inputs, seconds: f64, gear_up: bool) -> Vec<Out> {
        let mut out = Vec::new();
        let mut t = 0.;
        while t < seconds {
            out.extend(c.step(i, 0.1, gear_up));
            t += 0.1;
        }
        out
    }

    fn says(out: &[Out]) -> Vec<&'static str> {
        out.iter().filter_map(|o| if let Out::Say(k) = o { Some(*k) } else { None }).collect()
    }

    #[test]
    fn the_takeoff_calls_come_once_in_order() {
        let mut c = Callouts::default();
        let mut i = Inputs { on_ground: true, tla_max: 35., tla_min: 35., vr_kt: 150., gear_down: 1., gear_handle_down: true, ..Default::default() };
        let mut all = Vec::new();
        for ias in (0..=160).step_by(5) {
            i.ias_kt = ias as f64;
            i.gs_kt = ias as f64;
            all.extend(run(&mut c, &i, 0.3, true));
        }
        i.on_ground = false;
        i.vs_fpm = 1500.;
        i.ra_ft = 40.;
        all.extend(run(&mut c, &i, 2.0, true));
        assert_eq!(says(&all), ["thrust_set", "100kt", "rotate", "pos_climb", "gear_up"]);
        assert!(all.contains(&Out::GearUp));
    }

    #[test]
    fn no_gear_up_unless_asked_and_no_rotate_without_vr() {
        let mut c = Callouts::default();
        let mut i = Inputs { on_ground: true, tla_max: 45., ias_kt: 160., gs_kt: 160., gear_handle_down: true, ..Default::default() };
        let out = run(&mut c, &i, 1.0, false);
        assert_eq!(says(&out), ["thrust_set", "100kt"]);
        i.on_ground = false;
        i.vs_fpm = 1000.;
        i.ra_ft = 60.;
        let out = run(&mut c, &i, 3.0, false);
        assert_eq!(says(&out), ["pos_climb"]);
        assert!(!out.contains(&Out::GearUp));
    }

    #[test]
    fn a_rejected_takeoff_stops_the_calls_and_rearms() {
        let mut c = Callouts::default();
        let mut i = Inputs { on_ground: true, tla_max: 35., ias_kt: 60., gs_kt: 60., vr_kt: 150., ..Default::default() };
        assert_eq!(says(&run(&mut c, &i, 0.5, false)), ["thrust_set"]);
        i.tla_max = 0.;
        i.ias_kt = 110.;
        assert!(run(&mut c, &i, 0.5, false).is_empty());
        i.ias_kt = 0.;
        i.gs_kt = 0.;
        run(&mut c, &i, 0.5, false);
        i.tla_max = 35.;
        i.ias_kt = 50.;
        i.gs_kt = 50.;
        assert_eq!(says(&run(&mut c, &i, 0.5, false)), ["thrust_set"]);
    }

    #[test]
    fn a_thousand_feet_is_stabilised_only_when_configured() {
        let mut c = Callouts::default();
        let mut i = Inputs { ra_ft: 2000., vs_fpm: -700., gear_down: 1., flap_index: 4., ..Default::default() };
        run(&mut c, &i, 0.5, false);
        i.ra_ft = 990.;
        assert_eq!(says(&run(&mut c, &i, 0.5, false)), ["stabilised"]);
        i.ra_ft = 2000.;
        run(&mut c, &i, 0.5, false);
        i.ra_ft = 990.;
        i.gear_down = 0.;
        assert_eq!(says(&run(&mut c, &i, 0.5, false)), ["not_stabilised"]);
    }

    #[test]
    fn the_rollout_calls_spoilers_reverse_decel_and_seventy() {
        let mut c = Callouts::default();
        let mut i = Inputs { ra_ft: 100., vs_fpm: -700., ias_kt: 140., gs_kt: 140., ..Default::default() };
        run(&mut c, &i, 0.5, false);
        i.on_ground = true;
        i.ra_ft = 0.;
        i.spoilers = 1.;
        i.tla_min = -20.;
        i.reverse = 1.;
        i.decel_g = 0.2;
        let first = says(&run(&mut c, &i, 0.5, false));
        assert_eq!(first, ["spoilers", "reverse_green", "decel"]);
        i.ias_kt = 65.;
        i.gs_kt = 65.;
        assert_eq!(says(&run(&mut c, &i, 0.5, false)), ["70kt"]);
    }

    #[test]
    fn the_after_landing_flow_starts_once_below_taxi_speed_with_reversers_stowed() {
        let mut c = Callouts::default();
        let mut i = Inputs { ra_ft: 100., ias_kt: 140., gs_kt: 140., ..Default::default() };
        run(&mut c, &i, 0.5, false);
        i.on_ground = true;
        i.tla_min = -20.;
        let flows = |out: &[Out]| out.iter().filter(|o| **o == Out::AfterLanding).count();
        assert_eq!(flows(&run(&mut c, &i, 2.0, false)), 0);
        i.gs_kt = 25.;
        assert_eq!(flows(&run(&mut c, &i, 2.0, false)), 0, "still in reverse");
        i.tla_min = 0.;
        assert_eq!(flows(&run(&mut c, &i, 2.0, false)), 1);
        assert_eq!(flows(&run(&mut c, &i, 5.0, false)), 0, "only once");
        let mut rto = Callouts::default();
        let r = Inputs { on_ground: true, gs_kt: 20., ..Default::default() };
        assert_eq!(flows(&run(&mut rto, &r, 3.0, false)), 0, "never without a landing");
    }

    #[test]
    fn the_after_start_flow_follows_the_fourth_engine_once_and_only_on_a_fresh_start() {
        let flows = |out: &[Out]| out.iter().filter(|o| **o == Out::AfterStart).count();
        let mut c = Callouts::default();
        let mut i = Inputs { on_ground: true, engines_running: Some(false), ..Default::default() };
        assert_eq!(flows(&run(&mut c, &i, 1.0, false)), 0);
        i.engines_running = Some(true);
        assert_eq!(flows(&run(&mut c, &i, 5.0, false)), 0, "waits a few seconds");
        assert_eq!(flows(&run(&mut c, &i, 5.0, false)), 1);
        assert_eq!(flows(&run(&mut c, &i, 20.0, false)), 0, "only once");
        let mut warm = Callouts::default();
        let w = Inputs { on_ground: true, engines_running: Some(true), ..Default::default() };
        assert_eq!(flows(&run(&mut warm, &w, 20.0, false)), 0, "not when the sim starts with engines running");
    }

    #[test]
    fn flap_and_gear_selections_in_flight_are_confirmed_once_the_lever_settles() {
        let mut c = Callouts::default();
        let mut i = Inputs { ra_ft: 3000., flap_index: 0., ..Default::default() };
        run(&mut c, &i, 1.0, false);
        i.flap_index = 1.;
        run(&mut c, &i, 0.3, false);
        i.flap_index = 2.;
        assert_eq!(says(&run(&mut c, &i, 2.0, false)), ["flaps_2"], "one call for a quick double step");
        i.gear_handle_down = true;
        assert_eq!(says(&run(&mut c, &i, 0.5, false)), ["gear_down"]);
        let mut ground = Callouts::default();
        let mut g = Inputs { on_ground: true, flap_index: 0., ..Default::default() };
        run(&mut ground, &g, 1.0, false);
        g.flap_index = 1.;
        assert!(says(&run(&mut ground, &g, 3.0, false)).is_empty(), "silent on the ground");
    }

    fn checklists_of(out: &[Out]) -> Vec<u8> {
        out.iter().filter_map(|o| if let Out::Checklist(c) = o { Some(*c) } else { None }).collect()
    }

    #[test]
    fn the_first_officer_starts_each_checklist_once_at_its_point_in_the_flight() {
        let mut c = Callouts::default();
        let mut all = Vec::new();
        let mut i = Inputs { on_ground: true, engines_off: Some(true), engines_running: Some(false), ..Default::default() };
        all.extend(checklists_of(&run(&mut c, &i, 2.0, false)));
        i.beacon = true;
        all.extend(checklists_of(&run(&mut c, &i, 2.0, false)));
        i.engines_off = Some(false);
        all.extend(checklists_of(&run(&mut c, &i, 5.0, false)));
        i.engines_running = Some(true);
        all.extend(checklists_of(&run(&mut c, &i, 30.0, false)));
        i.gs_kt = 12.;
        all.extend(checklists_of(&run(&mut c, &i, 5.0, false)));
        i.gs_kt = 0.;
        i.landing_lights = true;
        all.extend(checklists_of(&run(&mut c, &i, 2.0, false)));
        assert_eq!(all, [CL_BEFORE_START, CL_AFTER_START, CL_TAXI, CL_LINE_UP]);
        i.on_ground = false;
        i.ra_ft = 2000.;
        i.alt_ft = 30_000.;
        all.clear();
        all.extend(checklists_of(&run(&mut c, &i, 2.0, false)));
        i.alt_ft = 9_000.;
        i.vs_fpm = -1200.;
        all.extend(checklists_of(&run(&mut c, &i, 2.0, false)));
        i.gear_handle_down = true;
        i.flap_index = 4.;
        all.extend(checklists_of(&run(&mut c, &i, 2.0, false)));
        i.on_ground = true;
        i.ra_ft = 0.;
        i.gs_kt = 20.;
        all.extend(checklists_of(&run(&mut c, &i, 5.0, false)));
        i.gs_kt = 0.;
        i.engines_running = Some(false);
        i.engines_off = Some(true);
        all.extend(checklists_of(&run(&mut c, &i, 2.0, false)));
        assert_eq!(all, [CL_APPROACH, CL_LANDING, CL_PARKING], "no taxi checklist after landing");
        i.beacon = false;
        run(&mut c, &i, 1.0, false);
        i.beacon = true;
        assert_eq!(checklists_of(&run(&mut c, &i, 1.0, false)), [CL_BEFORE_START], "the next departure starts over");
    }

    #[test]
    fn missing_spoilers_and_reverse_are_called_out() {
        let mut c = Callouts::default();
        let mut i = Inputs { ra_ft: 100., ias_kt: 140., gs_kt: 140., autobrake_mode: 2., ..Default::default() };
        run(&mut c, &i, 0.5, false);
        i.on_ground = true;
        i.tla_min = -20.;
        let out = says(&run(&mut c, &i, 7.0, false));
        assert_eq!(out, ["no_spoilers", "no_reverse", "no_decel"]);
    }

    fn sop(c: &mut Callouts, i: &Inputs) -> Vec<&'static str> {
        says(&c.step_with(i, 0.1, false, true))
    }

    #[test]
    fn sop_calls_transition_and_a_thousand_to_go_once() {
        let mut c = Callouts::default();
        let mut i = Inputs { ra_ft: 3000., alt_ft: 3000., vs_fpm: 2000., fcu_alt_ft: 10000., trans_alt_ft: 5000., trans_lvl_ft: 6000., ..Default::default() };
        let mut all = Vec::new();
        for a in (3000..=10000).step_by(100) {
            i.alt_ft = a as f64;
            all.extend(sop(&mut c, &i));
        }
        assert_eq!(all, ["transition_std", "1000_to_go"]);
        i.vs_fpm = -1500.;
        i.fcu_alt_ft = 4000.;
        let mut down = Vec::new();
        for a in (4000..=10000).rev().step_by(100) {
            i.alt_ft = a as f64;
            down.extend(sop(&mut c, &i));
        }
        assert_eq!(down, ["transition_qnh", "1000_to_go"]);
    }

    #[test]
    fn taxi_speed_is_called_on_taxi_but_not_on_the_rollout() {
        let mut c = Callouts::default();
        let mut i = Inputs { on_ground: true, gs_kt: 10., ..Default::default() };
        assert!(sop(&mut c, &i).is_empty());
        i.gs_kt = 35.;
        assert_eq!(sop(&mut c, &i), ["taxi_speed"]);
        assert!(sop(&mut c, &i).is_empty());
        let mut l = Callouts::default();
        let mut a = Inputs { ra_ft: 100., alt_ft: 100., ias_kt: 140., gs_kt: 140., ..Default::default() };
        sop(&mut l, &a);
        a.on_ground = true;
        a.ra_ft = 0.;
        let calls: Vec<&str> = (0..20).flat_map(|_| sop(&mut l, &a)).collect();
        assert!(!calls.contains(&"taxi_speed"), "{calls:?}");
    }

    #[test]
    fn arinc_words_decode_only_in_normal_operation() {
        let word = |v: f32, ssm: u64| ((ssm << 32) | v.to_bits() as u64) as f64;
        assert_eq!(arinc(word(5000., 3)), Some(5000.));
        assert_eq!(arinc(word(5000., 0)), None);
    }

    #[test]
    fn only_slug_keys_are_queued() {
        assert!(valid_key("c_parking_brake"));
        assert!(!valid_key("../x"));
        assert!(!valid_key("A"));
        assert!(!valid_key(""));
    }
}

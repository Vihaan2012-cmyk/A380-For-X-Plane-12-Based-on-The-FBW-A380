//! State that FlyByWire's instrument pages kept for the cockpit and wrote
//! into L: variables, for the pages XPHFBW no longer loads when the native
//! displays draw their screen (`xphfbw_bridge_views::PAGE_STOPS_WHEN_NATIVE`).
//! Each piece below does what the named page source does with the same inputs;
//! it runs only while that page is stopped, so a loaded page never has two
//! writers.
//!
//! Clock: fbw-common `Clock/Components/Chrono.tsx`, `ElapsedTime.tsx`,
//! `shared/Utils.tsx`. ISIS: fbw-a380x `ISISlegacy/index.tsx`,
//! `AutoBrightness.tsx`, `Bug.ts`. SND: the plugin's `SND/snd.js`
//! (`updateBrightness`, `onInteractionEvent`). SD/SDv2: fbw-a380x
//! `SDv2/Pages/Status/StatusPage.tsx`. ND: fbw-a380x
//! `ND/VerticalDisplay/VerticalDisplay.tsx` (`verticalRange`,
//! `minMaxVerticalRange`) with `MsfsAvionicsCommon/AdirsValueProvider.tsx`.
//! BAT, RTPI, IFE, FCU keep no state; PFD and EWD write only from clicks.

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;

use crate::js_bridge::{EnvRefs, HostState, Resolved, VarsHost};
use crate::xphfbw_bridge_views::{page_stop_mask, page_stopped};
use crate::Vars;

const CLOCK: &str = "Clock";
const ISIS: &str = "SCREEN_ISIS_1";
const SND: &str = "SCREEN_ISIS_2";
const SD: &str = "SCREEN_DU_SD";
const ND: [&str; 2] = ["SCREEN_DU_NDL", "SCREEN_DU_NDR"];

const CHRONO_PERIOD: f64 = 0.2;
const MAX_STEP_GAP: f64 = 60.;
const BRIGHTNESS_STEP: f64 = 0.025;
const BRIGHTNESS_MIN: f64 = 0.03;
const BRIGHTNESS_MAX: f64 = 0.99;
const BRIGHTNESS_REPEAT: f64 = 0.15;
const BUGS_TIMEOUT: f64 = 15.;

const ET_ELAPSED: &str = "L:A32NX_CHRONO_ET_ELAPSED_TIME";
const CHRONO_ELAPSED: &str = "L:A32NX_CHRONO_ELAPSED_TIME";
const ET_SWITCH: &str = "L:A32NX_CHRONO_ET_SWITCH_POS";
const DATE_MODE: &str = "L:A32NX_CLOCK_DATE_MODE";
const DC_ESS: &str = "L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED";
const DC_HOT_1: &str = "L:A32NX_ELEC_DC_HOT_1_BUS_IS_POWERED";
const AUTO_BRIGHTNESS: &str = "GLASSCOCKPIT AUTOMATIC BRIGHTNESS";
const ISIS_OFFSET: &str = "L:A32NX_ISIS_MANUAL_BRIGHTNESS_OFFSET";
const ISIS_BUGS_ACTIVE: &str = "L:A32NX_ISIS_BUGS_ACTIVE";
const SND_OFFSET: &str = "L:A380X_SND_MANUAL_BRIGHTNESS_OFFSET";

pub trait Io {
    fn get(&mut self, name: &str) -> f64;
    fn set(&mut self, name: &str, value: f64);
}

fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    v.max(lo).min(hi)
}

fn event_name(raw: &str) -> &str {
    raw.strip_prefix("H:").unwrap_or(raw)
}

#[derive(Default)]
struct Chrono {
    elapsed: Option<f64>,
    running: bool,
    prev_time: Option<f64>,
    et: Option<f64>,
    et_prev_time: Option<f64>,
    hot1: Option<bool>,
    pulse_at: Option<f64>,
    date_mode: bool,
}

impl Chrono {
    fn set_elapsed(&mut self, io: &mut dyn Io, v: Option<f64>) {
        if self.elapsed != v {
            self.elapsed = v;
            io.set(CHRONO_ELAPSED, v.unwrap_or(-1.));
        }
    }

    fn set_et(&mut self, io: &mut dyn Io, v: Option<f64>) {
        if self.et != v {
            self.et = v;
            io.set(ET_ELAPSED, v.unwrap_or(-1.));
        }
    }

    fn event(&mut self, io: &mut dyn Io, name: &str) {
        let ess = io.get(DC_ESS) != 0.;
        match name {
            "A32NX_CHRONO_RST" if ess => {
                let v = if self.running { Some(0.) } else { None };
                self.set_elapsed(io, v);
            }
            "A32NX_CHRONO_TOGGLE" if ess => self.running = !self.running,
            "A32NX_CHRONO_DATE" => {
                self.date_mode = !self.date_mode;
                io.set(DATE_MODE, self.date_mode as u8 as f64);
            }
            _ => {}
        }
    }

    fn delta(now: f64, prev: Option<f64>) -> f64 {
        let diff = prev.map_or(f64::NAN, |p| (now - p).max(0.));
        if diff < MAX_STEP_GAP {
            diff
        } else {
            0.
        }
    }

    fn tick(&mut self, io: &mut dyn Io, now: f64) {
        let hot1 = io.get(DC_HOT_1) != 0.;
        if self.hot1 != Some(hot1) {
            self.hot1 = Some(hot1);
            if !hot1 {
                self.running = false;
                self.set_elapsed(io, None);
                self.set_et(io, None);
            }
        }
        if self.pulse_at.is_some_and(|t| now - t < CHRONO_PERIOD - 1e-9) {
            return;
        }
        self.pulse_at = Some(now);
        let ess = io.get(DC_ESS) != 0.;
        if self.running {
            let v = self.elapsed.unwrap_or(0.) + Self::delta(now, self.prev_time);
            self.set_elapsed(io, Some(v));
        }
        self.prev_time = Some(now);
        if ess {
            let knob = io.get(ET_SWITCH);
            if knob == 0. {
                let v = self.et.unwrap_or(0.) + Self::delta(now, self.et_prev_time);
                self.set_et(io, Some(v));
            } else if knob == 2. {
                self.set_et(io, None);
            }
            self.et_prev_time = Some(now);
        }
    }
}

#[derive(Clone, Copy)]
struct BugLimits {
    min: f64,
    max: f64,
    increment: f64,
}

const ALT_BUG: BugLimits = BugLimits { min: 100., max: 50000., increment: 100. };
const SPD_BUG: BugLimits = BugLimits { min: 30., max: 660., increment: 1. };
const BUG_LIMITS: [BugLimits; 6] = [ALT_BUG, ALT_BUG, SPD_BUG, SPD_BUG, SPD_BUG, SPD_BUG];
const FIRST_SELECTED_BUG: usize = 5;

fn bug_names(index: usize) -> (String, String) {
    let (kind, n) = if index < 2 { ("ALT", index) } else { ("SPD", index - 2) };
    (format!("L:A32NX_ISIS_BUGS_{kind}_VALUE:{n}"), format!("L:A32NX_ISIS_BUGS_{kind}_ACTIVE:{n}"))
}

struct Isis {
    selected: usize,
    last_input: f64,
    bugs_were_active: bool,
    plus_held: bool,
    minus_held: bool,
    last_step: Option<f64>,
    last_raw: Option<f64>,
}

impl Default for Isis {
    fn default() -> Self {
        Self { selected: FIRST_SELECTED_BUG, last_input: 0., bugs_were_active: false, plus_held: false, minus_held: false, last_step: None, last_raw: None }
    }
}

impl Isis {
    fn auto_brightness(io: &mut dyn Io) -> (f64, f64) {
        let raw = io.get(AUTO_BRIGHTNESS);
        let scaled = if raw > 1. { raw / 100. } else { raw };
        (raw, 0.2 + (0.99 - 0.2) * clamp(scaled, 0., 1.))
    }

    fn event(&mut self, io: &mut dyn Io, name: &str, now: f64) {
        let bugs_active = io.get(ISIS_BUGS_ACTIVE) != 0.;
        let (value_name, active_name) = bug_names(self.selected);
        let limits = BUG_LIMITS[self.selected];
        match name {
            "A32NX_ISIS_KNOB_PRESSED" => {
                self.last_input = now;
                if bugs_active {
                    let active = io.get(&active_name) != 0.;
                    io.set(&active_name, (!active) as u8 as f64);
                    let value = io.get(&value_name);
                    io.set(&value_name, value);
                }
            }
            "A32NX_ISIS_KNOB_CLOCKWISE" | "A32NX_ISIS_KNOB_ANTI_CLOCKWISE" => {
                self.last_input = now;
                let (active, value) = (io.get(&active_name) != 0., io.get(&value_name));
                let up = name == "A32NX_ISIS_KNOB_CLOCKWISE";
                if !active || (up && value >= limits.max) || (!up && value <= limits.min) {
                    return;
                }
                if bugs_active {
                    let moved = if up { value + limits.increment } else { value - limits.increment };
                    io.set(&value_name, clamp(moved, limits.min, limits.max));
                    io.set(&active_name, active as u8 as f64);
                }
            }
            "A32NX_ISIS_PLUS_PRESSED" => {
                self.last_input = now;
                if bugs_active {
                    self.selected = (7 + self.selected) % 6;
                } else {
                    self.plus_held = true;
                }
            }
            "A32NX_ISIS_MINUS_PRESSED" => {
                self.last_input = now;
                if bugs_active {
                    self.selected = (5 + self.selected) % 6;
                } else {
                    self.minus_held = true;
                }
            }
            "A32NX_ISIS_PLUS_RELEASED" => self.plus_held = false,
            "A32NX_ISIS_MINUS_RELEASED" => self.minus_held = false,
            _ => {}
        }
    }

    fn tick(&mut self, io: &mut dyn Io, now: f64) {
        let bugs_active = io.get(ISIS_BUGS_ACTIVE) != 0.;
        if bugs_active && !self.bugs_were_active {
            self.last_input = now;
        }
        self.bugs_were_active = bugs_active;
        if bugs_active && now - self.last_input > BUGS_TIMEOUT {
            io.set(ISIS_BUGS_ACTIVE, 0.);
            self.bugs_were_active = false;
        }
        let (raw, auto) = Self::auto_brightness(io);
        let (lo, hi) = (BRIGHTNESS_MIN - auto, BRIGHTNESS_MAX - auto);
        let mut offset = io.get(ISIS_OFFSET);
        if self.last_raw != Some(raw) {
            self.last_raw = Some(raw);
            let clamped = clamp(offset, lo, hi);
            io.set(ISIS_OFFSET, clamped);
            offset = clamped;
        }
        if !(self.plus_held || self.minus_held) {
            self.last_step = None;
            return;
        }
        if self.last_step.is_some_and(|t| now - t < BRIGHTNESS_REPEAT) {
            return;
        }
        self.last_step = Some(now);
        let still_active = io.get(ISIS_BUGS_ACTIVE) != 0.;
        let direction = if still_active { 0. } else if self.plus_held { 1. } else { -1. };
        if direction != 0. {
            io.set(ISIS_OFFSET, clamp(offset + direction * BRIGHTNESS_STEP, lo, hi));
        }
    }
}

#[derive(Default)]
struct Snd {
    offset: Option<f64>,
    held: f64,
    held_at: f64,
}

impl Snd {
    fn event(&mut self, name: &str) {
        match name {
            "A380X_SND_PLUS_PRESSED" => (self.held, self.held_at) = (1., f64::NEG_INFINITY),
            "A380X_SND_MINUS_PRESSED" => (self.held, self.held_at) = (-1., f64::NEG_INFINITY),
            "A380X_SND_PLUS_RELEASED" | "A380X_SND_MINUS_RELEASED" => self.held = 0.,
            _ => {}
        }
    }

    fn tick(&mut self, io: &mut dyn Io, now: f64) {
        let offset = *self.offset.get_or_insert_with(|| io.get(SND_OFFSET));
        if self.held == 0. || now - self.held_at < BRIGHTNESS_REPEAT {
            return;
        }
        self.held_at = now;
        let auto = 0.2 + 0.79 * clamp(io.get(AUTO_BRIGHTNESS) / 100., 0., 1.);
        let moved = clamp(offset + BRIGHTNESS_STEP * self.held, BRIGHTNESS_MIN - auto, BRIGHTNESS_MAX - auto);
        self.offset = Some(moved);
        io.set(SND_OFFSET, moved);
    }
}

const LIMITATIONS_ALL: &str = "fws_limitations_all_phases";
const LIMITATIONS_APPR: &str = "fws_limitations_appr_ldg";
const DEFERRED: &str = "fws_deferred_procedures";
const INFORMATION: &str = "fws_information";
const INOP_ALL: &str = "fws_inop_sys_all_phases";
const INOP_APPR: &str = "fws_inop_sys_appr_ldg";
const INOP_REDUND: &str = "fws_inop_sys_redundancy_loss";
const VD_TARGET: &str = "a32nx_fms_vertical_target_profile";
const VD_ACTUAL: &str = "a32nx_fms_vertical_actual_profile";

/// The bus topics the stopped pages read.
pub const BUS_TOPICS: &[&str] = &[LIMITATIONS_ALL, LIMITATIONS_APPR, DEFERRED, INFORMATION, INOP_ALL, INOP_APPR, INOP_REDUND, VD_TARGET, VD_ACTUAL];

#[derive(Default)]
pub struct Bus {
    values: HashMap<&'static str, Value>,
}

impl Bus {
    pub fn set(&mut self, topic: &str, json: &str) {
        let Some(t) = BUS_TOPICS.iter().find(|t| **t == topic) else { return };
        if let Ok(v) = serde_json::from_str(json) {
            self.values.insert(*t, v);
        }
    }

    fn array(&self, topic: &str) -> &[Value] {
        self.values.get(topic).and_then(Value::as_array).map_or(&[], Vec::as_slice)
    }
}

const STS_LINES_PER_PAGE_USABLE: usize = 13;
const STS_NUMBER_OF_PAGES: &str = "L:A32NX_ECAM_SD_STS_NUMBER_OF_PAGES";
const STS_MORE_AVAILABLE: &str = "L:A32NX_ECAM_SD_STS_MORE_AVAILABLE";
const SD_MORE_SHOWN: &str = "L:A32NX_ECAM_SD_MORE_SHOWN";

/// `EcamDeferredProcedures` ids and their `DeferredProcedureType`.
const DEFERRED_PROCEDURE_TYPES: [(&str, usize); 8] = [
    ("210700001", 0),
    ("210700002", 0),
    ("221700001", 1),
    ("270700001", 2),
    ("320700001", 3),
    ("340700001", 1),
    ("340700003", 1),
    ("340700002", 1),
];

fn deferred_procedure_lines(procs: &[Value]) -> usize {
    let types: Vec<usize> = procs
        .iter()
        .filter_map(|p| {
            let id = match p.get("id")? {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => return None,
            };
            DEFERRED_PROCEDURE_TYPES.iter().find(|(i, _)| *i == id).map(|(_, t)| *t)
        })
        .collect();
    (0..4)
        .map(|t| match types.iter().filter(|&&x| x == t).count() {
            0 => 0,
            n => n + 1,
        })
        .sum()
}

fn sts_number_of_pages(bus: &Bus, more_active: bool) -> usize {
    let n = |topic| bus.array(topic).len();
    if more_active {
        return match n(INOP_REDUND) {
            0 => 1,
            r => r.div_ceil(STS_LINES_PER_PAGE_USABLE),
        };
    }
    let sections = [deferred_procedure_lines(bus.array(DEFERRED)) + 1, n(INFORMATION) + 1, n(INOP_ALL).max(n(INOP_APPR)) + 2, 0];
    let mut page = 0;
    let mut used = n(LIMITATIONS_ALL).max(n(LIMITATIONS_APPR)) + 2;
    for lines in sections {
        if used + lines > STS_LINES_PER_PAGE_USABLE {
            page += 1;
            used = 0;
        }
        used += lines;
    }
    page + 1
}

#[derive(Default)]
struct SdStatus {
    pages: Option<f64>,
    more: Option<f64>,
}

impl SdStatus {
    fn tick(&mut self, io: &mut dyn Io, bus: &Bus) {
        let pages = sts_number_of_pages(bus, io.get(SD_MORE_SHOWN) != 0.) as f64;
        if self.pages != Some(pages) {
            self.pages = Some(pages);
            io.set(STS_NUMBER_OF_PAGES, pages);
        }
        let more = if bus.array(INOP_REDUND).is_empty() { 0. } else { 1. };
        if self.more != Some(more) {
            self.more = Some(more);
            io.set(STS_MORE_AVAILABLE, more);
        }
    }
}

const ND_RANGES: [f64; 8] = [-1., 10., 20., 40., 80., 160., 320., 640.];
const ND_MODE_ARC: f64 = 3.;
const VD_MIN_ALTITUDE: f64 = -500.;
const VD_MAX_ALTITUDE: f64 = 70000.;
const NM_IN_FEET: f64 = 1852. / 0.3048;
const AIR_DATA_KNOB: &str = "L:A32NX_AIR_DATA_SWITCHING_KNOB";

fn arinc_value_or(raw: f64, default: f64) -> f64 {
    let ssm = ((raw / 4294967296.).trunc() as i64) & 0b11;
    if ssm == 0b11 || ssm == 0b10 {
        f32::from_bits((raw as i64 as u64 & 0xffff_ffff) as u32) as f64
    } else {
        default
    }
}

fn vd_range(nd_mode: f64, nd_range: f64) -> f64 {
    if nd_mode == ND_MODE_ARC {
        10f64.max(nd_range.min(160.))
    } else {
        5f64.max((nd_range / 2.).min(160.))
    }
}

fn min_max_vertical_range(vd_range: f64, ego: f64, path: Option<(f64, f64)>) -> (f64, f64) {
    let extent = vd_range * NM_IN_FEET * 4f64.to_radians().sin();
    let Some((first, last)) = path else {
        let lower = VD_MIN_ALTITUDE.max((ego - 0.5 * extent).min(VD_MAX_ALTITUDE - extent));
        return (lower, lower + extent);
    };
    let (highest, lowest) = (first.max(last), first.min(last));
    let mut lower = VD_MIN_ALTITUDE.max((lowest + highest - extent) / 2.);
    let mut upper = lower + extent;
    if ego < lower {
        lower = VD_MIN_ALTITUDE.max(ego - 0.15 * extent);
        upper = lower + extent;
    } else if ego > upper {
        upper = VD_MAX_ALTITUDE.min(ego + 0.15 * extent);
        lower = upper - extent;
    }
    (lower, upper)
}

fn displayed_path_ends(bus: &Bus, vd_range: f64) -> Option<(f64, f64)> {
    let actual = bus.array(VD_ACTUAL);
    let path = if actual.len() > 1 { actual } else { bus.array(VD_TARGET) };
    let mut shown = 0;
    for p in path {
        shown += 1;
        if p.get("distanceFromAircraft").and_then(Value::as_f64).is_some_and(|d| d > vd_range * 1.2) {
            break;
        }
    }
    let altitude = |p: &Value| p.get("altitude").and_then(Value::as_f64);
    Some((altitude(path.first()?)?, altitude(&path[shown - 1])?))
}

struct Vd {
    side: char,
    last: Option<(f64, f64)>,
}

impl Vd {
    fn new(side: char) -> Self {
        Self { side, last: None }
    }

    fn tick(&mut self, io: &mut dyn Io, bus: &Bus) {
        let side = self.side;
        let index = io.get(&format!("L:A32NX_EFIS_{side}_ND_RANGE"));
        let Some(&nd_range) = (index >= 0.).then(|| ND_RANGES.get(index as usize)).flatten() else { return };
        let range = vd_range(io.get(&format!("L:A32NX_EFIS_{side}_ND_MODE")), nd_range);
        let knob = io.get(AIR_DATA_KNOB);
        let (display, source) = if side == 'L' { (1, if knob == 0. { 3 } else { 1 }) } else { (2, if knob == 2. { 3 } else { 2 }) };
        let ego = arinc_value_or(io.get(&format!("L:A32NX_ADIRS_ADR_{source}_BARO_CORRECTED_ALTITUDE_{display}")), 0.);
        let vr = min_max_vertical_range(range, ego, displayed_path_ends(bus, range));
        if self.last != Some(vr) {
            self.last = Some(vr);
            io.set(&format!("L:A32NX_VD_{display}_RANGE_LOWER"), vr.0);
            io.set(&format!("L:A32NX_VD_{display}_RANGE_UPPER"), vr.1);
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Stopped {
    clock: bool,
    isis: bool,
    snd: bool,
    sd: bool,
    nd: [bool; 2],
}

impl Stopped {
    fn from_mask(mask: u64) -> Self {
        let s = |screen| page_stopped(screen, mask);
        Self { clock: s(CLOCK), isis: s(ISIS), snd: s(SND), sd: s(SD), nd: ND.map(s) }
    }
}

pub struct PageState {
    mask: u64,
    stopped: Stopped,
    pub bus: Bus,
    chrono: Chrono,
    isis: Isis,
    snd: Snd,
    sts: SdStatus,
    vd: [Vd; 2],
}

impl PageState {
    pub fn new(mask: u64) -> Self {
        Self {
            mask,
            stopped: Stopped::from_mask(mask),
            bus: Bus::default(),
            chrono: Chrono::default(),
            isis: Isis::default(),
            snd: Snd::default(),
            sts: SdStatus::default(),
            vd: [Vd::new('L'), Vd::new('R')],
        }
    }

    pub fn tick(&mut self, io: &mut dyn Io, now: f64, h_events: &[String]) {
        let Stopped { clock, isis, snd, sd, nd } = self.stopped;
        if clock {
            self.chrono.tick(io, now);
        }
        for raw in h_events {
            let name = event_name(raw);
            if clock {
                self.chrono.event(io, name);
            }
            if isis {
                self.isis.event(io, name, now);
            }
            if snd {
                self.snd.event(name);
            }
        }
        if isis {
            self.isis.tick(io, now);
        }
        if snd {
            self.snd.tick(io, now);
        }
        if sd {
            self.sts.tick(io, &self.bus);
        }
        for (vd, stopped) in self.vd.iter_mut().zip(nd) {
            if stopped {
                vd.tick(io, &self.bus);
            }
        }
    }
}

#[cfg(feature = "native-displays")]
pub fn stopped_pages_mask() -> u64 {
    page_stop_mask(crate::native_displays::screen::selection().targets.iter().map(|t| t.screen))
}

#[cfg(not(feature = "native-displays"))]
pub fn stopped_pages_mask() -> u64 {
    page_stop_mask([])
}

struct HostIo<'a, 'b> {
    host: &'a mut VarsHost<'b>,
    resolved: &'a mut HashMap<String, Resolved>,
}

impl Io for HostIo<'_, '_> {
    fn get(&mut self, name: &str) -> f64 {
        let r = *self.resolved.entry(name.to_string()).or_insert_with(|| self.host.resolve(name, "number"));
        self.host.read(name, r)
    }

    fn set(&mut self, name: &str, value: f64) {
        let r = *self.resolved.entry(name.to_string()).or_insert_with(|| self.host.resolve(name, "number"));
        self.host.write(name, r, value);
    }
}

struct Running {
    state: PageState,
    resolved: HashMap<String, Resolved>,
    #[cfg(feature = "native-displays")]
    feed: crate::native_displays::inputs::LiveInputs,
}

#[cfg(feature = "native-displays")]
mod feed {
    use crate::native_displays::inputs::{InputSink, LiveInputs, ReadSet, VarSource};

    pub fn new() -> LiveInputs {
        let mut live = LiveInputs::with_read_set(ReadSet::default());
        live.skip_mfd_dom(true);
        live
    }

    struct NoVars;

    impl VarSource for NoVars {
        fn read_var(&mut self, _: &str, _: &str) -> Option<f64> {
            None
        }

        fn read_string(&mut self, _: &str) -> Option<String> {
            None
        }
    }

    struct BusSink<'a>(&'a mut super::Bus);

    impl InputSink for BusSink<'_> {
        fn set_var(&mut self, _: &str, _: &str, _: f64) {}
        fn set_string(&mut self, _: &str, _: &str) {}
        fn set_bus(&mut self, topic: &str, json: &str) {
            self.0.set(topic, json);
        }
        fn set_bus_string(&mut self, topic: &str, text: &str) {
            self.0.set(topic, text);
        }
        fn set_generic(&mut self, _: &str, _: &str) {}
        fn set_fetched(&mut self, _: &str, _: &[u8]) {}
        fn set_env_number(&mut self, _: &str, _: f64) {}
        fn set_now(&mut self, _: f64) {}
        fn set_dpr(&mut self, _: f64) {}
    }

    pub fn pull(live: &mut LiveInputs, bus: &mut super::Bus, time: f64) {
        live.update(&mut NoVars, &mut BusSink(bus), time * 1000., 1.);
    }
}

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

pub fn tick(vars: &mut Vars, state: &mut HostState, env: &EnvRefs, time: f64, h_events: &[String]) {
    let Ok(mut guard) = RUNNING.lock() else { return };
    let running = guard.get_or_insert_with(|| Running {
        state: PageState::new(stopped_pages_mask()),
        resolved: HashMap::new(),
        #[cfg(feature = "native-displays")]
        feed: feed::new(),
    });
    if running.state.mask == 0 {
        return;
    }
    #[cfg(feature = "native-displays")]
    feed::pull(&mut running.feed, &mut running.state.bus, time);
    let mut host = VarsHost::new(vars, state, env, time);
    let mut io = HostIo { host: &mut host, resolved: &mut running.resolved };
    running.state.tick(&mut io, time, h_events);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xphfbw_bridge_views::PAGE_STOPS_WHEN_NATIVE;

    #[derive(Default)]
    struct Fake {
        vars: HashMap<String, f64>,
        writes: Vec<(String, f64)>,
    }

    impl Fake {
        fn with(pairs: &[(&str, f64)]) -> Self {
            Self { vars: pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect(), writes: Vec::new() }
        }

        fn at(&self, name: &str) -> f64 {
            self.vars.get(name).copied().unwrap_or(0.)
        }

        fn written(&self, name: &str) -> Vec<f64> {
            self.writes.iter().filter(|(n, _)| n == name).map(|(_, v)| *v).collect()
        }
    }

    impl Io for Fake {
        fn get(&mut self, name: &str) -> f64 {
            self.at(name)
        }

        fn set(&mut self, name: &str, value: f64) {
            self.vars.insert(name.to_string(), value);
            self.writes.push((name.to_string(), value));
        }
    }

    fn events(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn all() -> PageState {
        PageState::new(page_stop_mask(PAGE_STOPS_WHEN_NATIVE.iter().copied()))
    }

    const POWERED: [(&str, f64); 3] = [(DC_ESS, 1.), (DC_HOT_1, 1.), (ET_SWITCH, 1.)];

    #[test]
    fn every_stopped_page_is_either_stateless_or_has_state_here() {
        let stateless = ["FCU", "RTPI", "BAT", "SCREEN_IFE", "SCREEN_DU_PFDL", "SCREEN_DU_PFDR", "SCREEN_DU_EWD"];
        let stateful = [CLOCK, ISIS, SND, SD];
        let systems_host = ["SCREEN_DU_RMP_1"];
        for page in PAGE_STOPS_WHEN_NATIVE {
            assert!(stateless.contains(page) || stateful.contains(page) || systems_host.contains(page), "{page} has no state coverage");
        }
        assert_eq!(PAGE_STOPS_WHEN_NATIVE.len(), stateless.len() + stateful.len() + systems_host.len());
        assert!(ND.iter().all(|nd| !PAGE_STOPS_WHEN_NATIVE.contains(nd)), "the ND page also runs OANC and BTV");
    }

    fn bus(pairs: &[(&str, Value)]) -> Bus {
        let mut b = Bus::default();
        for (topic, v) in pairs {
            b.set(topic, &v.to_string());
        }
        b
    }

    fn ids(n: usize) -> Value {
        Value::Array((0..n).map(|i| Value::String(format!("{i}"))).collect())
    }

    fn deferred(ids: &[&str]) -> Value {
        Value::Array(ids.iter().map(|id| serde_json::json!({ "id": id, "procedureCompleted": false })).collect())
    }

    #[test]
    fn status_page_count_follows_the_section_lines() {
        assert_eq!(sts_number_of_pages(&Bus::default(), false), 1);
        let b = bus(&[(LIMITATIONS_ALL, ids(5)), (LIMITATIONS_APPR, ids(8)), (DEFERRED, deferred(&["210700001", "210700002"])), (INFORMATION, ids(10)), (INOP_ALL, ids(19)), (INOP_APPR, ids(8))]);
        assert_eq!(sts_number_of_pages(&b, false), 5, "an over-full INOP SYS still pushes the empty last section on");
        let b = bus(&[(LIMITATIONS_ALL, ids(3)), (INFORMATION, ids(2))]);
        assert_eq!(sts_number_of_pages(&b, false), 1, "5 + 1 + 3 + 2 lines fit");
        let b = bus(&[(LIMITATIONS_ALL, ids(9)), (INFORMATION, ids(2))]);
        assert_eq!(sts_number_of_pages(&b, false), 2, "11 + 1 + 3 do not");
        assert_eq!(deferred_procedure_lines(deferred(&["221700001", "340700002", "320700001", "999999999"]).as_array().unwrap()), 5, "one heading per type, unknown ids dropped");
    }

    #[test]
    fn more_pages_count_redundancy_losses_thirteen_a_page() {
        assert_eq!(sts_number_of_pages(&Bus::default(), true), 1);
        assert_eq!(sts_number_of_pages(&bus(&[(INOP_REDUND, ids(13))]), true), 1);
        assert_eq!(sts_number_of_pages(&bus(&[(INOP_REDUND, ids(14)), (LIMITATIONS_ALL, ids(30))]), true), 2);
    }

    #[test]
    fn the_status_vars_are_written_at_start_and_on_change_only() {
        let mut io = Fake::default();
        let mut s = all();
        s.tick(&mut io, 1., &[]);
        assert_eq!((io.written(STS_NUMBER_OF_PAGES), io.written(STS_MORE_AVAILABLE)), (vec![1.], vec![0.]));
        let writes = io.writes.len();
        s.tick(&mut io, 2., &[]);
        assert_eq!(io.writes.len(), writes);
        s.bus = bus(&[(INOP_REDUND, ids(20))]);
        s.tick(&mut io, 3., &[]);
        assert_eq!((io.at(STS_NUMBER_OF_PAGES), io.at(STS_MORE_AVAILABLE)), (1., 1.));
        io.vars.insert(SD_MORE_SHOWN.into(), 1.);
        s.tick(&mut io, 4., &[]);
        assert_eq!(io.written(STS_NUMBER_OF_PAGES), vec![1., 2.]);
    }

    fn arinc(value: f32, ssm: u64) -> f64 {
        (ssm * 4294967296 + u64::from(value.to_bits())) as f64
    }

    fn with_nd(mut s: PageState) -> PageState {
        s.stopped.nd = [true, true];
        s
    }

    #[test]
    fn vertical_display_range_centres_the_aircraft_without_a_path() {
        let mut io = Fake::with(&[("L:A32NX_EFIS_L_ND_MODE", 2.), ("L:A32NX_EFIS_L_ND_RANGE", 1.), (AIR_DATA_KNOB, 1.), ("L:A32NX_ADIRS_ADR_1_BARO_CORRECTED_ALTITUDE_1", arinc(15000., 3))]);
        let mut s = with_nd(PageState::new(0));
        s.tick(&mut io, 1., &[]);
        let extent = 5. * NM_IN_FEET * 4f64.to_radians().sin();
        assert!((io.at("L:A32NX_VD_1_RANGE_LOWER") - (15000. - extent / 2.)).abs() < 1e-9);
        assert!((io.at("L:A32NX_VD_1_RANGE_UPPER") - (15000. + extent / 2.)).abs() < 1e-9);
        assert_eq!(io.at("L:A32NX_VD_2_RANGE_LOWER"), -500., "no altitude on side 2 sits at the floor");
        let writes = io.writes.len();
        s.tick(&mut io, 2., &[]);
        assert_eq!(io.writes.len(), writes);
        io.vars.insert("L:A32NX_ADIRS_ADR_1_BARO_CORRECTED_ALTITUDE_1".into(), arinc(15000., 1));
        s.tick(&mut io, 3., &[]);
        assert_eq!(io.at("L:A32NX_VD_1_RANGE_LOWER"), -500., "no computed data reads as zero");
    }

    #[test]
    fn vertical_display_range_takes_the_air_data_source_and_arc_range() {
        let mut io = Fake::with(&[("L:A32NX_EFIS_R_ND_MODE", 3.), ("L:A32NX_EFIS_R_ND_RANGE", 7.), (AIR_DATA_KNOB, 2.), ("L:A32NX_ADIRS_ADR_3_BARO_CORRECTED_ALTITUDE_2", arinc(69000., 3))]);
        let mut s = with_nd(PageState::new(0));
        s.tick(&mut io, 1., &[]);
        let extent = 160. * NM_IN_FEET * 4f64.to_radians().sin();
        assert!((io.at("L:A32NX_VD_2_RANGE_UPPER") - 70000.).abs() < 1e-6, "capped at the ceiling");
        assert!((io.at("L:A32NX_VD_2_RANGE_LOWER") - (70000. - extent)).abs() < 1e-6);
    }

    #[test]
    fn vertical_display_range_fits_the_displayed_path() {
        let profile = serde_json::json!([
            { "distanceFromAircraft": 0, "altitude": 5000 },
            { "distanceFromAircraft": 10, "altitude": 9000 },
            { "distanceFromAircraft": 13, "altitude": 12000 },
            { "distanceFromAircraft": 40, "altitude": 30000 }
        ]);
        let mut io = Fake::with(&[("L:A32NX_EFIS_L_ND_MODE", 3.), ("L:A32NX_EFIS_L_ND_RANGE", 1.), (AIR_DATA_KNOB, 1.), ("L:A32NX_ADIRS_ADR_1_BARO_CORRECTED_ALTITUDE_1", arinc(8000., 3))]);
        let mut s = with_nd(PageState::new(0));
        s.bus = bus(&[(VD_TARGET, profile.clone()), (VD_ACTUAL, serde_json::json!([{ "distanceFromAircraft": 0, "altitude": 1 }]))]);
        s.tick(&mut io, 1., &[]);
        let extent = 10. * NM_IN_FEET * 4f64.to_radians().sin();
        let lower = (5000. + 12000. - extent) / 2.;
        assert!((io.at("L:A32NX_VD_1_RANGE_LOWER") - lower).abs() < 1e-6, "the path stops after the first point past 12 NM");
        io.vars.insert("L:A32NX_ADIRS_ADR_1_BARO_CORRECTED_ALTITUDE_1".into(), arinc(2000., 3));
        s.tick(&mut io, 2., &[]);
        assert!((io.at("L:A32NX_VD_1_RANGE_LOWER") - (2000. - 0.15 * extent)).abs() < 1e-6, "an aircraft below the window pulls it down");
        s.bus = bus(&[(VD_TARGET, serde_json::json!([])), (VD_ACTUAL, profile)]);
        io.vars.insert("L:A32NX_EFIS_L_ND_RANGE".into(), 9.);
        let writes = io.writes.len();
        s.tick(&mut io, 3., &[]);
        assert_eq!(io.writes.len(), writes, "an unknown range index writes nothing");
    }

    fn golden_states(page: &str) -> Vec<std::path::PathBuf> {
        fn walk(dir: &std::path::Path, page: &str, out: &mut Vec<std::path::PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else { return };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, page, out);
                } else if p.file_name().is_some_and(|n| n.to_string_lossy() == format!("{page}.state.json")) {
                    out.push(p);
                }
            }
        }
        let mut out = Vec::new();
        for dir in std::env::var("PAGE_STATE_GOLDENS").unwrap_or_default().split(';').filter(|d| !d.is_empty()) {
            walk(std::path::Path::new(dir), page, &mut out);
        }
        out
    }

    fn replay_golden(path: &std::path::Path, stopped: impl FnOnce(&mut Stopped)) -> Option<(Fake, HashMap<String, f64>)> {
        let state: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
        let mut io = Fake::default();
        for (k, v) in state.get("vars")?.as_object()? {
            if let Some(v) = v.as_f64() {
                io.vars.insert(k.split('|').next().unwrap_or(k).to_string(), v);
            }
        }
        let mut written = HashMap::new();
        for w in state.get("writes")?.as_array()? {
            let a = w.get("a").and_then(Value::as_array);
            if let (Some("setVar"), Some([name, _, value, ..])) = (w.get("k").and_then(Value::as_str), a.map(Vec::as_slice)) {
                if let (Some(name), Some(value)) = (name.as_str(), value.as_f64()) {
                    written.insert(name.to_string(), value);
                }
            }
        }
        let mut s = PageState::new(0);
        stopped(&mut s.stopped);
        for (topic, entry) in state.get("bus")?.as_object()? {
            if let Some(data) = entry.get("data") {
                s.bus.set(topic, &data.to_string());
            }
        }
        s.tick(&mut io, 1., &[]);
        Some((io, written))
    }

    #[test]
    #[ignore = "needs PAGE_STATE_GOLDENS=<golden-tour run dirs separated by ;>"]
    fn matches_the_golden_tour_writes() {
        let (mut checked, mut failed) = (0, Vec::new());
        for path in golden_states("sdv2") {
            let Some((io, written)) = replay_golden(&path, |s| s.sd = true) else { continue };
            for name in [STS_NUMBER_OF_PAGES, STS_MORE_AVAILABLE] {
                if let Some(want) = written.get(name) {
                    checked += 1;
                    if io.at(name) != *want {
                        failed.push(format!("{}: {name} {} want {want}", path.display(), io.at(name)));
                    }
                }
            }
        }
        for (page, n) in [("nd-1", 1), ("nd-2", 2)] {
            for path in golden_states(page) {
                let Some((io, written)) = replay_golden(&path, |s| s.nd = [n == 1, n == 2]) else { continue };
                for end in ["LOWER", "UPPER"] {
                    let name = format!("L:A32NX_VD_{n}_RANGE_{end}");
                    if let Some(want) = written.get(&name) {
                        checked += 1;
                        if (io.at(&name) - want).abs() > 1e-6 {
                            failed.push(format!("{}: {name} {} want {want}", path.display(), io.at(&name)));
                        }
                    }
                }
            }
        }
        eprintln!("golden page-state writes: {checked} checked, {} differ", failed.len());
        for f in failed.iter().take(20) {
            eprintln!("  {f}");
        }
        assert!(checked > 0 && failed.is_empty());
    }

    #[test]
    fn nothing_runs_for_a_page_that_is_still_loaded() {
        let mut io = Fake::with(&POWERED);
        let mut s = PageState::new(0);
        s.tick(&mut io, 1., &events(&["A32NX_CHRONO_TOGGLE", "A32NX_ISIS_PLUS_PRESSED", "A380X_SND_PLUS_PRESSED"]));
        s.tick(&mut io, 3., &[]);
        assert!(io.writes.is_empty());
    }

    #[test]
    fn the_chronometer_counts_only_while_running_and_resets_by_state() {
        let mut io = Fake::with(&POWERED);
        let mut s = all();
        s.tick(&mut io, 100., &[]);
        assert!(io.written(CHRONO_ELAPSED).is_empty(), "nothing written while idle");
        s.tick(&mut io, 101., &events(&["A32NX_CHRONO_TOGGLE"]));
        assert!(io.written(CHRONO_ELAPSED).is_empty(), "started, not yet counted");
        s.tick(&mut io, 102.5, &[]);
        assert_eq!(io.at(CHRONO_ELAPSED), 1.5);
        s.tick(&mut io, 102.6, &[]);
        assert_eq!(io.at(CHRONO_ELAPSED), 1.5, "five pulses a second at most");
        s.tick(&mut io, 104., &events(&["H:A32NX_CHRONO_TOGGLE"]));
        assert_eq!(io.at(CHRONO_ELAPSED), 3.);
        s.tick(&mut io, 110., &[]);
        assert_eq!(io.at(CHRONO_ELAPSED), 3., "stopped");
        s.tick(&mut io, 111., &events(&["A32NX_CHRONO_RST"]));
        assert_eq!(io.at(CHRONO_ELAPSED), -1., "reset while stopped blanks it");
        s.tick(&mut io, 112., &events(&["A32NX_CHRONO_TOGGLE"]));
        s.tick(&mut io, 113., &events(&["A32NX_CHRONO_RST"]));
        assert_eq!(io.at(CHRONO_ELAPSED), 0., "reset while running restarts at zero");
    }

    #[test]
    fn a_time_jump_of_a_minute_or_more_adds_nothing() {
        let mut io = Fake::with(&POWERED);
        let mut s = all();
        s.tick(&mut io, 0., &events(&["A32NX_CHRONO_TOGGLE"]));
        s.tick(&mut io, 1., &[]);
        s.tick(&mut io, 500., &[]);
        assert_eq!(io.at(CHRONO_ELAPSED), 1.);
        s.tick(&mut io, 501., &[]);
        assert_eq!(io.at(CHRONO_ELAPSED), 2.);
    }

    #[test]
    fn chrono_buttons_need_dc_ess() {
        let mut io = Fake::with(&[(DC_ESS, 0.), (DC_HOT_1, 1.)]);
        let mut s = all();
        s.tick(&mut io, 1., &events(&["A32NX_CHRONO_TOGGLE"]));
        s.tick(&mut io, 2., &[]);
        assert!(io.written(CHRONO_ELAPSED).is_empty());
    }

    #[test]
    fn the_elapsed_time_follows_its_switch_and_both_clear_when_hot_bus_one_drops() {
        let mut io = Fake::with(&[(DC_ESS, 1.), (DC_HOT_1, 1.), (ET_SWITCH, 0.)]);
        let mut s = all();
        s.tick(&mut io, 10., &[]);
        assert_eq!(io.at(ET_ELAPSED), 0.);
        s.tick(&mut io, 12., &[]);
        assert_eq!(io.at(ET_ELAPSED), 2.);
        io.vars.insert(ET_SWITCH.into(), 1.);
        s.tick(&mut io, 20., &[]);
        assert_eq!(io.at(ET_ELAPSED), 2., "held at STP");
        io.vars.insert(ET_SWITCH.into(), 2.);
        s.tick(&mut io, 21., &[]);
        assert_eq!(io.at(ET_ELAPSED), -1., "RST blanks it");
        io.vars.insert(ET_SWITCH.into(), 0.);
        s.tick(&mut io, 22., &[]);
        s.tick(&mut io, 23., &events(&["A32NX_CHRONO_TOGGLE"]));
        s.tick(&mut io, 24., &[]);
        assert!(io.at(ET_ELAPSED) > 0. && io.at(CHRONO_ELAPSED) > 0.);
        io.vars.insert(DC_HOT_1.into(), 0.);
        io.vars.insert(DC_ESS.into(), 0.);
        s.tick(&mut io, 25., &[]);
        assert_eq!((io.at(ET_ELAPSED), io.at(CHRONO_ELAPSED)), (-1., -1.));
        s.tick(&mut io, 26., &[]);
        assert_eq!(io.at(CHRONO_ELAPSED), -1., "no longer running");
    }

    const BRIGHT: [(&str, f64); 1] = [(AUTO_BRIGHTNESS, 50.)];

    #[test]
    fn isis_brightness_steps_while_held_and_stays_inside_the_limits() {
        let auto = 0.2 + 0.79 * 0.5;
        let mut io = Fake::with(&BRIGHT);
        let mut s = all();
        s.tick(&mut io, 1., &events(&["A32NX_ISIS_PLUS_PRESSED"]));
        assert!((io.at(ISIS_OFFSET) - 0.025).abs() < 1e-12, "first step is immediate");
        s.tick(&mut io, 1.1, &[]);
        assert!((io.at(ISIS_OFFSET) - 0.025).abs() < 1e-12);
        s.tick(&mut io, 1.16, &[]);
        assert!((io.at(ISIS_OFFSET) - 0.05).abs() < 1e-12);
        s.tick(&mut io, 1.2, &events(&["A32NX_ISIS_PLUS_RELEASED"]));
        s.tick(&mut io, 5., &[]);
        assert!((io.at(ISIS_OFFSET) - 0.05).abs() < 1e-12);
        io.vars.insert(ISIS_OFFSET.into(), 5.);
        s.tick(&mut io, 6., &events(&["A32NX_ISIS_MINUS_PRESSED"]));
        assert!((io.at(ISIS_OFFSET) - (0.99 - auto)).abs() < 1e-12, "an out-of-range offset is clamped by the first step");
        let mut t = 6.;
        for _ in 0..200 {
            t += 0.16;
            s.tick(&mut io, t, &[]);
        }
        assert!((io.at(ISIS_OFFSET) - (0.03 - auto)).abs() < 1e-9, "stops at the dimmest");
    }

    #[test]
    fn isis_offset_is_pulled_back_in_range_when_the_ambient_brightness_changes() {
        let mut io = Fake::with(&[(AUTO_BRIGHTNESS, 100.), (ISIS_OFFSET, 0.5)]);
        let mut s = all();
        s.tick(&mut io, 1., &[]);
        assert!((io.at(ISIS_OFFSET) - 0.).abs() < 1e-12, "0.99 - 0.99");
        io.vars.insert(AUTO_BRIGHTNESS.into(), 0.);
        io.vars.insert(ISIS_OFFSET.into(), -0.5);
        s.tick(&mut io, 2., &[]);
        assert!((io.at(ISIS_OFFSET) - (0.03 - 0.2)).abs() < 1e-12);
        let writes = io.writes.len();
        s.tick(&mut io, 3., &[]);
        assert_eq!(io.writes.len(), writes, "unchanged ambient light writes nothing");
    }

    #[test]
    fn isis_brightness_keys_do_nothing_while_the_bugs_page_is_up() {
        let mut io = Fake::with(&[(AUTO_BRIGHTNESS, 50.), (ISIS_BUGS_ACTIVE, 1.)]);
        let mut s = all();
        s.tick(&mut io, 1., &[]);
        let before = io.at(ISIS_OFFSET);
        s.tick(&mut io, 2., &events(&["A32NX_ISIS_PLUS_PRESSED"]));
        s.tick(&mut io, 3., &[]);
        assert_eq!(io.at(ISIS_OFFSET), before);
    }

    #[test]
    fn isis_bugs_page_selects_toggles_and_adjusts_bugs() {
        let mut io = Fake::with(&[(AUTO_BRIGHTNESS, 50.), (ISIS_BUGS_ACTIVE, 1.)]);
        let mut s = all();
        s.tick(&mut io, 1., &[]);
        assert_eq!(s.isis.selected, 5);
        s.tick(&mut io, 2., &events(&["A32NX_ISIS_PLUS_PRESSED"]));
        assert_eq!(s.isis.selected, 0, "wraps forward");
        s.tick(&mut io, 3., &events(&["A32NX_ISIS_MINUS_PRESSED"]));
        assert_eq!(s.isis.selected, 5, "wraps back");
        s.tick(&mut io, 4., &events(&["A32NX_ISIS_MINUS_PRESSED"]));
        assert_eq!(s.isis.selected, 4);
        let (v, a) = ("L:A32NX_ISIS_BUGS_SPD_VALUE:2", "L:A32NX_ISIS_BUGS_SPD_ACTIVE:2");
        s.tick(&mut io, 5., &events(&["A32NX_ISIS_KNOB_CLOCKWISE"]));
        assert_eq!(io.at(v), 0., "an inactive bug does not move");
        s.tick(&mut io, 6., &events(&["A32NX_ISIS_KNOB_PRESSED"]));
        assert_eq!(io.at(a), 1.);
        s.tick(&mut io, 7., &events(&["A32NX_ISIS_KNOB_CLOCKWISE"]));
        assert_eq!(io.at(v), 30., "clamped up to the minimum");
        s.tick(&mut io, 8., &events(&["A32NX_ISIS_KNOB_CLOCKWISE", "A32NX_ISIS_KNOB_CLOCKWISE"]));
        assert_eq!(io.at(v), 32.);
        s.tick(&mut io, 9., &events(&["A32NX_ISIS_KNOB_ANTI_CLOCKWISE"]));
        assert_eq!(io.at(v), 31.);
        io.vars.insert(v.into(), 660.);
        s.tick(&mut io, 10., &events(&["A32NX_ISIS_KNOB_CLOCKWISE"]));
        assert_eq!(io.at(v), 660.);
        io.vars.insert(v.into(), 30.);
        s.tick(&mut io, 11., &events(&["A32NX_ISIS_KNOB_ANTI_CLOCKWISE"]));
        assert_eq!(io.at(v), 30.);
        s.tick(&mut io, 12., &events(&["A32NX_ISIS_KNOB_PRESSED"]));
        assert_eq!(io.at(a), 0.);
    }

    #[test]
    fn isis_altitude_bugs_move_by_hundreds_between_100_and_50000() {
        let mut io = Fake::with(&[(AUTO_BRIGHTNESS, 50.), (ISIS_BUGS_ACTIVE, 1.), ("L:A32NX_ISIS_BUGS_ALT_ACTIVE:1", 1.), ("L:A32NX_ISIS_BUGS_ALT_VALUE:1", 1000.)]);
        let mut s = all();
        s.tick(&mut io, 1., &[]);
        for _ in 0..2 {
            s.tick(&mut io, 2., &events(&["A32NX_ISIS_PLUS_PRESSED"]));
        }
        assert_eq!(s.isis.selected, 1);
        s.tick(&mut io, 3., &events(&["A32NX_ISIS_KNOB_CLOCKWISE"]));
        assert_eq!(io.at("L:A32NX_ISIS_BUGS_ALT_VALUE:1"), 1100.);
        io.vars.insert("L:A32NX_ISIS_BUGS_ALT_VALUE:1".into(), 49950.);
        s.tick(&mut io, 4., &events(&["A32NX_ISIS_KNOB_CLOCKWISE"]));
        assert_eq!(io.at("L:A32NX_ISIS_BUGS_ALT_VALUE:1"), 50000.);
    }

    #[test]
    fn isis_bugs_page_closes_after_fifteen_seconds_without_input() {
        let mut io = Fake::with(&[(AUTO_BRIGHTNESS, 50.), (ISIS_BUGS_ACTIVE, 1.)]);
        let mut s = all();
        s.tick(&mut io, 100., &[]);
        s.tick(&mut io, 114., &[]);
        assert_eq!(io.at(ISIS_BUGS_ACTIVE), 1.);
        s.tick(&mut io, 114.5, &events(&["A32NX_ISIS_KNOB_PRESSED"]));
        s.tick(&mut io, 128., &[]);
        assert_eq!(io.at(ISIS_BUGS_ACTIVE), 1.);
        s.tick(&mut io, 130., &[]);
        assert_eq!(io.at(ISIS_BUGS_ACTIVE), 0.);
    }

    #[test]
    fn snd_brightness_steps_while_held_from_the_saved_offset() {
        let auto = 0.2 + 0.79 * 0.5;
        let mut io = Fake::with(&[(AUTO_BRIGHTNESS, 50.), (SND_OFFSET, -0.1)]);
        let mut s = all();
        s.tick(&mut io, 1., &[]);
        assert!(io.written(SND_OFFSET).is_empty());
        s.tick(&mut io, 2., &events(&["A380X_SND_MINUS_PRESSED"]));
        assert!((io.at(SND_OFFSET) - (-0.125)).abs() < 1e-12);
        s.tick(&mut io, 2.1, &[]);
        assert_eq!(io.written(SND_OFFSET).len(), 1);
        s.tick(&mut io, 2.2, &[]);
        assert!((io.at(SND_OFFSET) - (-0.15)).abs() < 1e-12);
        s.tick(&mut io, 2.3, &events(&["A380X_SND_MINUS_RELEASED"]));
        s.tick(&mut io, 4., &[]);
        assert!((io.at(SND_OFFSET) - (-0.15)).abs() < 1e-12);
        s.tick(&mut io, 5., &events(&["A380X_SND_PLUS_PRESSED"]));
        assert!((io.at(SND_OFFSET) - (-0.125)).abs() < 1e-12);
        s.snd.offset = Some(10.);
        s.tick(&mut io, 6., &[]);
        assert!((io.at(SND_OFFSET) - (0.99 - auto)).abs() < 1e-12);
    }

    #[cfg(feature = "native-displays")]
    #[test]
    fn with_every_screen_native_the_selection_stops_exactly_the_listed_pages() {
        let selection = crate::native_displays::screen::parse("all");
        let mask = page_stop_mask(selection.targets.iter().map(|t| t.screen));
        let want: Vec<&str> = PAGE_STOPS_WHEN_NATIVE.to_vec();
        let got: Vec<&str> = selection.targets.iter().map(|t| t.screen).filter(|s| page_stopped(s, mask)).collect();
        let mut got_sorted = got.clone();
        got_sorted.sort_unstable();
        let mut want_sorted = want.clone();
        want_sorted.sort_unstable();
        assert_eq!(got_sorted, want_sorted);
        assert_eq!(page_stop_mask(crate::native_displays::screen::parse("off").targets.iter().map(|t| t.screen)), 0);
    }
}

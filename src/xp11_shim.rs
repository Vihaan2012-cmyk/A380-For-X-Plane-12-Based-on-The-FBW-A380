//! X-Plane 11 support: detects X-Plane 11 at plugin start and publishes, under their X-Plane 12 names,
//! the datarefs and commands the aircraft uses that X-Plane 11 does not have, backed by its own.

use std::ffi::{c_char, c_int, c_void, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static XP11: AtomicBool = AtomicBool::new(false);
static INSTALLED: AtomicBool = AtomicBool::new(false);

pub fn is_xp11() -> bool {
    XP11.load(Ordering::Relaxed)
}

/// X-Plane 11.50 and later report XPLM 303; X-Plane 12 reports 400 and up.
pub fn is_xp11_xplm(xplm_version: c_int) -> bool {
    xplm_version > 0 && xplm_version < 400
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Src {
    Scale(&'static str, f64),
    Mem,
    Brakes,
    Arr(&'static str, usize),
    Layers(&'static str, f64),
    MemArr(usize),
    Doors,
    Wiper,
}

pub const ALIASES: &[(&str, Src)] = &[
    ("sim/weather/aircraft/barometer_current_pas", Src::Scale("sim/weather/barometer_current_inhg", 3386.389)),
    ("sim/weather/aircraft/cloud_base_msl_m", Src::Layers("sim/weather/cloud_base_msl_m", 1.0)),
    ("sim/weather/aircraft/cloud_tops_msl_m", Src::Layers("sim/weather/cloud_tops_msl_m", 1.0)),
    ("sim/weather/aircraft/cloud_coverage_percent", Src::Layers("sim/weather/cloud_coverage", 1.0 / 6.0)),
    ("sim/weather/aircraft/precipitation_on_aircraft_ratio", Src::Scale("sim/weather/precipitation_on_aircraft_ratio", 1.0)),
    ("sim/weather/aircraft/temperature_ambient_deg_c", Src::Scale("sim/weather/temperature_ambient_c", 1.0)),
    ("sim/weather/aircraft/temperature_leadingedge_deg_c", Src::Scale("sim/weather/temperature_le_c", 1.0)),
    ("sim/weather/aircraft/wind_now_direction_degt", Src::Scale("sim/weather/wind_direction_degt", 1.0)),
    ("sim/weather/aircraft/wind_now_speed_msc", Src::Scale("sim/weather/wind_speed_kt", 1.0)),
    ("sim/weather/aircraft/wind_now_x_msc", Src::Scale("sim/weather/wind_now_x_msc", 1.0)),
    ("sim/weather/aircraft/wind_now_y_msc", Src::Scale("sim/weather/wind_now_y_msc", 1.0)),
    ("sim/weather/aircraft/wind_now_z_msc", Src::Scale("sim/weather/wind_now_z_msc", 1.0)),
    ("sim/weather/region/rain_percent", Src::Scale("sim/weather/rain_percent", 1.0)),
    ("sim/weather/region/sealevel_pressure_pas", Src::Scale("sim/weather/barometer_sealevel_inhg", 3386.389)),
    ("sim/weather/region/weather_preset", Src::Mem),
    ("sim/cockpit2/engine/indicators/EGT_deg_cel", Src::Arr("sim/cockpit2/engine/indicators/EGT_deg_C", 16)),
    ("sim/flightmodel2/engines/EGT_deg_cel", Src::Arr("sim/flightmodel/engine/ENGN_EGT_c", 16)),
    ("sim/flightmodel2/engines/ITT_deg_cel", Src::Arr("sim/flightmodel/engine/ENGN_ITT_c", 16)),
    ("sim/cockpit2/controls/flap_handle_request_ratio", Src::Scale("sim/cockpit2/controls/flap_ratio", 1.0)),
    ("sim/cockpit2/controls/wheel_brake_ratio", Src::Brakes),
    ("sim/cockpit2/electrical/GPU_generator_on", Src::Scale("sim/cockpit/electrical/gpu_on", 1.0)),
    ("sim/cockpit2/gauges/actuators/barometer_setting_is_std_pilot", Src::Mem),
    ("sim/cockpit2/gauges/actuators/barometer_setting_is_std_copilot", Src::Mem),
    ("sim/cockpit2/ice/ice_AOA_heat_on_stby", Src::Mem),
    ("sim/cockpit2/ice/ice_TAT_heat_on", Src::Mem),
    ("sim/cockpit2/ice/ice_TAT_heat_on_copilot", Src::Mem),
    ("sim/cockpit2/ice/ice_TAT_heat_on_stby", Src::Mem),
    ("sim/cockpit2/ice/ice_pitot_heat_on_standby", Src::Mem),
    ("sim/cockpit2/ice/ice_static_heat_on_standby", Src::Mem),
    ("sim/cockpit2/switches/door_open_ratio", Src::Doors),
    ("sim/cockpit2/switches/rain_repellent_switch", Src::MemArr(2)),
    ("sim/cockpit2/switches/wiper_speed_switch", Src::Wiper),
    ("sim/cockpit2/EFIS/EFIS_weather_sector_width", Src::Mem),
    ("sim/aircraft/overflow/pushback_attached", Src::Mem),
    ("sim/aircraft/weight/acf_m_station_max", Src::MemArr(9)),
    ("sim/flightmodel/weight/m_stations", Src::MemArr(9)),
    ("sim/flightmodel2/gear/tire_part_brake", Src::MemArr(10)),
    ("sim/flightmodel2/misc/cg_offset_z", Src::Scale("sim/flightmodel/misc/cgz_ref_to_default", 1.0)),
    ("sim/flightmodel2/misc/contact/active", Src::MemArr(10)),
    ("sim/flightmodel2/misc/door_cycle_time", Src::MemArr(20)),
    ("sim/flightmodel2/misc/zfw_cg_offset_z", Src::Mem),
    ("sim/operation/override/override_autotune", Src::Mem),
];

pub const COMMANDS: &[&str] = &[
    "sim/autopilot/ap_disc_trim_interrupt",
    "sim/autopilot/autothrottle_arm",
    "sim/autopilot/autothrottle_hard_off",
    "sim/flight_controls/flaps_detent_1",
    "sim/flight_controls/flaps_detent_2",
    "sim/flight_controls/flaps_detent_3",
    "sim/flight_controls/flaps_detent_4",
    "sim/flight_controls/flaps_detent_5",
    "sim/flight_controls/flaps_detent_6",
    "sim/flight_controls/flaps_detent_7",
    "sim/flight_controls/flaps_detent_8",
    "sim/flight_controls/flaps_down_full",
    "sim/flight_controls/flaps_up_full",
    "sim/flight_controls/landing_gear_off",
    "sim/flight_controls/park_brake_release",
    "sim/flight_controls/park_brake_set",
    "sim/flight_controls/park_brake_toggle",
    "sim/ground_ops/jetway",
    "sim/instruments/EFIS_copilot_apt",
    "sim/instruments/EFIS_copilot_fix",
    "sim/instruments/EFIS_copilot_mode_dn",
    "sim/instruments/EFIS_copilot_mode_up",
    "sim/instruments/EFIS_copilot_ndb",
    "sim/instruments/EFIS_copilot_tcas",
    "sim/instruments/EFIS_copilot_terr",
    "sim/instruments/EFIS_copilot_vor",
    "sim/instruments/EFIS_copilot_wxr",
    "sim/instruments/EFIS_terr",
    "sim/instruments/barometer_copilot_std",
    "sim/instruments/barometer_std",
    "sim/operation/pause_off",
    "sim/operation/pause_on",
    "sim/starters/eng_mode_crank",
    "sim/starters/eng_mode_norm",
    "sim/starters/eng_mode_start",
];

pub fn array_len(src: Src) -> Option<usize> {
    match src {
        Src::Arr(_, n) | Src::MemArr(n) => Some(n),
        Src::Layers(..) => Some(3),
        Src::Doors => Some(20),
        Src::Wiper => Some(2),
        _ => None,
    }
}

type Ref = *mut c_void;
type GetI = unsafe extern "C" fn(*mut c_void) -> c_int;
type SetI = unsafe extern "C" fn(*mut c_void, c_int);
type GetF = unsafe extern "C" fn(*mut c_void) -> f32;
type SetF = unsafe extern "C" fn(*mut c_void, f32);
type GetD = unsafe extern "C" fn(*mut c_void) -> f64;
type SetD = unsafe extern "C" fn(*mut c_void, f64);
type GetVi = unsafe extern "C" fn(*mut c_void, *mut c_int, c_int, c_int) -> c_int;
type SetVi = unsafe extern "C" fn(*mut c_void, *mut c_int, c_int, c_int);
type GetVf = unsafe extern "C" fn(*mut c_void, *mut f32, c_int, c_int) -> c_int;
type SetVf = unsafe extern "C" fn(*mut c_void, *mut f32, c_int, c_int);
type GetB = unsafe extern "C" fn(*mut c_void, *mut c_void, c_int, c_int) -> c_int;
type SetB = unsafe extern "C" fn(*mut c_void, *mut c_void, c_int, c_int);
#[allow(clippy::type_complexity)]
type RegisterFn = unsafe extern "C" fn(
    *const c_char,
    c_int,
    c_int,
    Option<GetI>,
    Option<SetI>,
    Option<GetF>,
    Option<SetF>,
    Option<GetD>,
    Option<SetD>,
    Option<GetVi>,
    Option<SetVi>,
    Option<GetVf>,
    Option<SetVf>,
    Option<GetB>,
    Option<SetB>,
    *mut c_void,
    *mut c_void,
) -> Ref;
type CommandHandler = unsafe extern "C" fn(*mut c_void, c_int, *mut c_void) -> c_int;

struct Api {
    find: unsafe extern "C" fn(*const c_char) -> Ref,
    types: unsafe extern "C" fn(Ref) -> c_int,
    geti: unsafe extern "C" fn(Ref) -> c_int,
    seti: unsafe extern "C" fn(Ref, c_int),
    getf: unsafe extern "C" fn(Ref) -> f32,
    setf: unsafe extern "C" fn(Ref, f32),
    getd: unsafe extern "C" fn(Ref) -> f64,
    setd: unsafe extern "C" fn(Ref, f64),
    getvf: unsafe extern "C" fn(Ref, *mut f32, c_int, c_int) -> c_int,
    setvf: unsafe extern "C" fn(Ref, *mut f32, c_int, c_int),
    getvi: unsafe extern "C" fn(Ref, *mut c_int, c_int, c_int) -> c_int,
    setvi: unsafe extern "C" fn(Ref, *mut c_int, c_int, c_int),
}

static API: std::sync::OnceLock<Option<Api>> = std::sync::OnceLock::new();

fn api() -> Option<&'static Api> {
    API.get_or_init(|| {
        let s = crate::xp::Xplm::xplm_symbol;
        unsafe {
            Some(Api {
                find: std::mem::transmute::<*mut c_void, _>(s("XPLMFindDataRef")?),
                types: std::mem::transmute::<*mut c_void, _>(s("XPLMGetDataRefTypes")?),
                geti: std::mem::transmute::<*mut c_void, _>(s("XPLMGetDatai")?),
                seti: std::mem::transmute::<*mut c_void, _>(s("XPLMSetDatai")?),
                getf: std::mem::transmute::<*mut c_void, _>(s("XPLMGetDataf")?),
                setf: std::mem::transmute::<*mut c_void, _>(s("XPLMSetDataf")?),
                getd: std::mem::transmute::<*mut c_void, _>(s("XPLMGetDatad")?),
                setd: std::mem::transmute::<*mut c_void, _>(s("XPLMSetDatad")?),
                getvf: std::mem::transmute::<*mut c_void, _>(s("XPLMGetDatavf")?),
                setvf: std::mem::transmute::<*mut c_void, _>(s("XPLMSetDatavf")?),
                getvi: std::mem::transmute::<*mut c_void, _>(s("XPLMGetDatavi")?),
                setvi: std::mem::transmute::<*mut c_void, _>(s("XPLMSetDatavi")?),
            })
        }
    })
    .as_ref()
}

fn find(name: &str) -> Option<Ref> {
    let api = api()?;
    let c = CString::new(name).ok()?;
    let r = unsafe { (api.find)(c.as_ptr()) };
    (!r.is_null()).then_some(r)
}

fn read(r: Ref) -> f64 {
    let Some(api) = api() else { return 0. };
    unsafe {
        let t = (api.types)(r);
        if t & 2 != 0 {
            (api.getf)(r) as f64
        } else if t & 4 != 0 {
            (api.getd)(r)
        } else if t & 1 != 0 {
            (api.geti)(r) as f64
        } else {
            0.
        }
    }
}

fn write(r: Ref, v: f64) {
    let Some(api) = api() else { return };
    unsafe {
        let t = (api.types)(r);
        if t & 2 != 0 {
            (api.setf)(r, v as f32)
        } else if t & 4 != 0 {
            (api.setd)(r, v)
        } else if t & 1 != 0 {
            (api.seti)(r, v.round() as c_int)
        }
    }
}

struct Alias {
    src: Src,
    refs: Vec<Option<Ref>>,
    mem: Mutex<Vec<f32>>,
}

unsafe impl Send for Alias {}
unsafe impl Sync for Alias {}

impl Alias {
    fn new(src: Src) -> Self {
        let refs = match src {
            Src::Scale(n, _) | Src::Arr(n, _) => vec![find(n)],
            Src::Layers(n, _) => (0..3).map(|k| find(&format!("{n}[{k}]"))).collect(),
            Src::Brakes => vec![find("sim/cockpit2/controls/left_brake_ratio"), find("sim/cockpit2/controls/right_brake_ratio")],
            Src::Doors => vec![find("sim/cockpit2/switches/door_open")],
            Src::Wiper => vec![find("sim/cockpit2/switches/wiper_speed")],
            Src::Mem | Src::MemArr(_) => vec![],
        };
        let n = array_len(src).unwrap_or(1);
        Alias { src, refs, mem: Mutex::new(vec![0.; n]) }
    }

    fn get(&self) -> f64 {
        match self.src {
            Src::Scale(_, k) => self.refs[0].map_or(0., |r| read(r) * k),
            Src::Brakes => self.refs[0].map_or(0., read),
            _ => self.mem.lock().map_or(0., |m| m[0] as f64),
        }
    }

    fn set(&self, v: f64) {
        match self.src {
            Src::Scale(_, k) => {
                if let Some(r) = self.refs[0] {
                    write(r, v / k)
                }
            }
            Src::Brakes => self.refs.iter().flatten().for_each(|&r| write(r, v)),
            _ => {
                if let Ok(mut m) = self.mem.lock() {
                    m[0] = v as f32;
                }
            }
        }
    }

    fn get_at(&self, i: usize) -> f32 {
        let Some(api) = api() else { return 0. };
        match self.src {
            Src::Arr(_, _) => self.refs[0].map_or(0., |r| {
                let mut v = 0f32;
                unsafe { (api.getvf)(r, &mut v, i as c_int, 1) };
                v
            }),
            Src::Layers(_, k) => self.refs.get(i).copied().flatten().map_or(0., |r| (read(r) * k) as f32),
            Src::Doors => self.refs[0].map_or(0., |r| {
                let mut v: c_int = 0;
                if i < 10 {
                    unsafe { (api.getvi)(r, &mut v, i as c_int, 1) };
                }
                v as f32
            }),
            Src::Wiper => self.refs[0].map_or(0., |r| read(r) as f32),
            _ => self.mem.lock().map_or(0., |m| m.get(i).copied().unwrap_or(0.)),
        }
    }

    fn set_at(&self, i: usize, v: f32) {
        let Some(api) = api() else { return };
        match self.src {
            Src::Arr(_, _) => {
                if let Some(r) = self.refs[0] {
                    let mut v = v;
                    unsafe { (api.setvf)(r, &mut v, i as c_int, 1) };
                }
            }
            Src::Layers(..) => {}
            Src::Doors => {
                if let (Some(r), true) = (self.refs[0], i < 10) {
                    let mut b: c_int = (v > 0.5) as c_int;
                    unsafe { (api.setvi)(r, &mut b, i as c_int, 1) };
                }
            }
            Src::Wiper => {
                if let (Some(r), 0) = (self.refs[0], i) {
                    write(r, v as f64);
                }
            }
            _ => {
                if let Ok(mut m) = self.mem.lock() {
                    if let Some(slot) = m.get_mut(i) {
                        *slot = v;
                    }
                }
            }
        }
    }
}

fn alias<'a>(refcon: *mut c_void) -> &'a Alias {
    unsafe { &*(refcon as *const Alias) }
}

unsafe extern "C" fn a_geti(r: *mut c_void) -> c_int {
    alias(r).get().round() as c_int
}
unsafe extern "C" fn a_seti(r: *mut c_void, v: c_int) {
    alias(r).set(v as f64)
}
unsafe extern "C" fn a_getf(r: *mut c_void) -> f32 {
    alias(r).get() as f32
}
unsafe extern "C" fn a_setf(r: *mut c_void, v: f32) {
    alias(r).set(v as f64)
}
unsafe extern "C" fn a_getd(r: *mut c_void) -> f64 {
    alias(r).get()
}
unsafe extern "C" fn a_setd(r: *mut c_void, v: f64) {
    alias(r).set(v)
}

fn span(a: &Alias, offset: c_int, max: c_int) -> std::ops::Range<usize> {
    let n = array_len(a.src).unwrap_or(1);
    let start = (offset.max(0) as usize).min(n);
    start..(start + max.max(0) as usize).min(n)
}

unsafe extern "C" fn a_getvf(r: *mut c_void, out: *mut f32, offset: c_int, max: c_int) -> c_int {
    let a = alias(r);
    if out.is_null() {
        return array_len(a.src).unwrap_or(1) as c_int;
    }
    let range = span(a, offset, max);
    for (k, i) in range.clone().enumerate() {
        *out.add(k) = a.get_at(i);
    }
    range.len() as c_int
}
unsafe extern "C" fn a_setvf(r: *mut c_void, values: *mut f32, offset: c_int, count: c_int) {
    let a = alias(r);
    if values.is_null() {
        return;
    }
    for (k, i) in span(a, offset, count).enumerate() {
        a.set_at(i, *values.add(k));
    }
}
unsafe extern "C" fn a_getvi(r: *mut c_void, out: *mut c_int, offset: c_int, max: c_int) -> c_int {
    let a = alias(r);
    if out.is_null() {
        return array_len(a.src).unwrap_or(1) as c_int;
    }
    let range = span(a, offset, max);
    for (k, i) in range.clone().enumerate() {
        *out.add(k) = a.get_at(i).round() as c_int;
    }
    range.len() as c_int
}
unsafe extern "C" fn a_setvi(r: *mut c_void, values: *mut c_int, offset: c_int, count: c_int) {
    let a = alias(r);
    if values.is_null() {
        return;
    }
    for (k, i) in span(a, offset, count).enumerate() {
        a.set_at(i, *values.add(k) as f32);
    }
}

/// X-Plane 11's master volume, nothing while paused or with sound off: what the plugin's own output plays at.
pub fn master_volume() -> f32 {
    static REFS: std::sync::OnceLock<[usize; 3]> = std::sync::OnceLock::new();
    let refs = REFS.get_or_init(|| {
        ["sim/operation/sound/master_volume_ratio", "sim/time/paused", "sim/operation/sound/sound_on"].map(|n| find(n).map_or(0, |r| r as usize))
    });
    let value = |i: usize, default: f64| if refs[i] == 0 { default } else { read(refs[i] as Ref) };
    if value(1, 0.) != 0. || value(2, 1.) == 0. {
        return 0.;
    }
    value(0, 1.) as f32
}

unsafe extern "C" fn pause_handler(_cmd: *mut c_void, phase: c_int, refcon: *mut c_void) -> c_int {
    if phase == 0 {
        let want = !refcon.is_null();
        let paused = find("sim/time/paused").map(read).unwrap_or(0.) != 0.;
        if paused != want {
            crate::xp::command_once("sim/operation/pause_toggle");
        }
    }
    1
}

/// X-Plane 11 refuses plugin commands under sim/, so an X-Plane 12 command it lacks is made as fbw/xp11/...
pub fn alias_name(name: &str) -> String {
    format!("fbw/xp11/{}", name.strip_prefix("sim/").unwrap_or(name))
}

static COMMAND_ALIASES: std::sync::OnceLock<std::collections::HashMap<&'static str, usize>> = std::sync::OnceLock::new();

/// The command standing in for an X-Plane 12 command X-Plane 11 lacks.
pub fn alias_command(name: &str) -> Option<*mut c_void> {
    COMMAND_ALIASES.get()?.get(name).map(|&c| c as *mut c_void)
}

/// On X-Plane 11, registers every alias and command X-Plane does not already have; on X-Plane 12, nothing.
/// Called first in XPluginStart, before any module looks a dataref or command up.
pub fn install() {
    if INSTALLED.swap(true, Ordering::Relaxed) {
        return;
    }
    let Some(get_versions) = crate::xp::Xplm::xplm_symbol("XPLMGetVersions") else { return };
    let get_versions = unsafe { std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*mut c_int, *mut c_int, *mut c_int)>(get_versions) };
    let (mut xp, mut xplm, mut host) = (0, 0, 0);
    unsafe { get_versions(&mut xp, &mut xplm, &mut host) };
    if !is_xp11_xplm(xplm) {
        return;
    }
    XP11.store(true, Ordering::Relaxed);
    let Some(register) = crate::xp::Xplm::xplm_symbol("XPLMRegisterDataAccessor") else { return };
    let register = unsafe { std::mem::transmute::<*mut c_void, RegisterFn>(register) };
    let mut datarefs = 0;
    for &(name, src) in ALIASES {
        if find(name).is_some() {
            continue;
        }
        let a: &'static Alias = Box::leak(Box::new(Alias::new(src)));
        let refcon = a as *const Alias as *mut c_void;
        let cname: &'static CString = Box::leak(Box::new(CString::new(name).expect("no nul")));
        let made = unsafe {
            if array_len(src).is_some() {
                register(cname.as_ptr(), 8 | 16, 1, None, None, None, None, None, None, Some(a_getvi), Some(a_setvi), Some(a_getvf), Some(a_setvf), None, None, refcon, refcon)
            } else {
                register(cname.as_ptr(), 1 | 2 | 4, 1, Some(a_geti), Some(a_seti), Some(a_getf), Some(a_setf), Some(a_getd), Some(a_setd), None, None, None, None, None, None, refcon, refcon)
            }
        };
        datarefs += (!made.is_null()) as u32;
    }
    let mut commands = 0;
    if let (Some(create), Some(find_cmd), Some(handler)) = (
        crate::xp::Xplm::xplm_symbol("XPLMCreateCommand"),
        crate::xp::Xplm::xplm_symbol("XPLMFindCommand"),
        crate::xp::Xplm::xplm_symbol("XPLMRegisterCommandHandler"),
    ) {
        let create = unsafe { std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_void>(create) };
        let find_cmd = unsafe { std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*const c_char) -> *mut c_void>(find_cmd) };
        let handler = unsafe { std::mem::transmute::<*mut c_void, unsafe extern "C" fn(*mut c_void, CommandHandler, c_int, *mut c_void)>(handler) };
        let mut made = std::collections::HashMap::new();
        for &name in COMMANDS {
            let c = CString::new(name).expect("no nul");
            if !unsafe { find_cmd(c.as_ptr()) }.is_null() {
                continue;
            }
            let alias = CString::new(alias_name(name)).expect("no nul");
            let cmd = unsafe { create(alias.as_ptr(), c.as_ptr()) };
            if cmd.is_null() {
                continue;
            }
            commands += 1;
            made.insert(name, cmd as usize);
            if name == "sim/operation/pause_on" || name == "sim/operation/pause_off" {
                let want = (name == "sim/operation/pause_on") as usize as *mut c_void;
                unsafe { handler(cmd, pause_handler, 0, want) };
            }
        }
        let _ = COMMAND_ALIASES.set(made);
    }
    crate::log(&format!("xp11: X-Plane {xp} (XPLM {xplm}): {datarefs} X-Plane 12 datarefs and {commands} commands published from X-Plane 11's own"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xplm_versions_tell_the_two_simulators_apart() {
        assert!(is_xp11_xplm(303));
        assert!(is_xp11_xplm(301));
        assert!(!is_xp11_xplm(400));
        assert!(!is_xp11_xplm(411));
        assert!(!is_xp11_xplm(0));
    }

    #[test]
    fn every_alias_is_listed_once_and_arrays_have_their_x_plane_12_sizes() {
        let mut names: Vec<&str> = ALIASES.iter().map(|a| a.0).collect();
        let n = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), n);
        let size = |name: &str| ALIASES.iter().find(|a| a.0 == name).and_then(|a| array_len(a.1));
        assert_eq!(size("sim/weather/aircraft/cloud_base_msl_m"), Some(3));
        assert_eq!(size("sim/cockpit2/switches/door_open_ratio"), Some(20));
        assert_eq!(size("sim/flightmodel/weight/m_stations"), Some(9));
        assert_eq!(size("sim/cockpit2/switches/wiper_speed_switch"), Some(2));
        assert_eq!(size("sim/weather/aircraft/barometer_current_pas"), None);
    }

    #[test]
    fn every_command_is_listed_once() {
        let mut c = COMMANDS.to_vec();
        let n = c.len();
        c.sort();
        c.dedup();
        assert_eq!(c.len(), n);
    }
}

//! Binaural sound: every positional sound of the aircraft's sound pack is
//! rendered as it reaches each ear (HRTF), through Valve's Steam Audio
//! spatializer (Apache-2.0, `steamaudio/phonon.dll` + `phonon_fmod.dll`
//! beside the plugin) loaded into X-Plane's own FMOD. Each event's FMOD Pan
//! is bypassed and the spatializer takes its place; the spatializer reads
//! the listener from FMOD itself, so only the event's world position is fed
//! to it, and the event's own distance range and rolloff are applied as the
//! spatializer's output gain, as the bypassed pan did. Events without an
//! FMOD Pan (2D sounds) are left alone.

use std::collections::HashMap;
use std::ffi::c_void;

use crate::sound_levels::{events, Guid};

const PAN_DSP_TYPE: i32 = 27;
const PAN_ROLLOFF_PARAM: i32 = 11;
const STEAM_AUDIO_VERSION: u32 = (4 << 16) | (8 << 8) | 1;
const SIMD_AVX2: i32 = 3;
const SIMULATION_DIRECT: i32 = 1;
const MAX_INSTANCES: usize = 64;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
struct Attributes3d {
    position: Vec3,
    velocity: Vec3,
    forward: Vec3,
    up: Vec3,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct DspPosition {
    relative: Attributes3d,
    absolute: Attributes3d,
}

#[repr(C)]
struct ContextSettings {
    version: u32,
    log: *mut c_void,
    allocate: *mut c_void,
    free: *mut c_void,
    simd_level: i32,
    flags: i32,
}

#[repr(C)]
struct AudioSettings {
    sampling_rate: i32,
    frame_size: i32,
}

#[repr(C)]
struct HrtfSettings {
    kind: i32,
    sofa_file_name: *const i8,
    sofa_data: *const u8,
    sofa_data_size: i32,
    volume: f32,
    norm_type: i32,
}

#[repr(C)]
#[derive(Default)]
struct SimulationSettings {
    flags: i32,
    scene_type: i32,
    reflection_type: i32,
    max_num_occlusion_samples: i32,
    max_num_rays: i32,
    num_diffuse_samples: i32,
    max_duration: f32,
    max_order: i32,
    max_num_sources: i32,
    num_threads: i32,
    ray_batch_size: i32,
    num_vis_samples: i32,
    sampling_rate: i32,
    frame_size: i32,
    open_cl_device: usize,
    radeon_rays_device: usize,
    tan_device: usize,
}

#[repr(C)]
struct ParamDesc {
    kind: i32,
    name: [u8; 16],
}

/// FMOD's own rolloff for a distance between the event's min and max
/// (FMOD_DSP_PAN_3D_ROLLOFF_TYPE: 0 linear squared, 1 linear, 2 inverse,
/// 3 inverse tapered).
pub fn rolloff_gain(kind: i32, min: f32, max: f32, d: f32) -> f32 {
    if max <= 0. || d <= min {
        return 1.;
    }
    if d >= max && kind != 2 {
        return 0.;
    }
    let linear = ((max - d) / (max - min).max(1e-3)).clamp(0., 1.);
    let inverse = (min.max(1e-3) / d).clamp(0., 1.);
    match kind {
        0 => linear * linear,
        1 => linear,
        2 => inverse,
        _ => inverse.min(linear),
    }
}

macro_rules! api {
    ($($name:ident : $sym:literal => fn($($arg:ty),*) $(-> $ret:ty)?;)*) => {
        #[allow(non_snake_case)]
        struct Api { $($name: unsafe extern "C" fn($($arg),*) $(-> $ret)?,)* }
        impl Api {
            #[cfg(windows)]
            unsafe fn load(studio_dll: *mut c_void, core_dll: *mut c_void, phonon: *mut c_void, phonon_fmod: *mut c_void) -> Option<Api> {
                let pick = |module: *mut c_void, sym: &str| -> Option<*mut c_void> {
                    let p = GetProcAddress(module, format!("{sym}\0").as_ptr().cast());
                    (!p.is_null()).then_some(p)
                };
                Some(Api { $($name: std::mem::transmute::<*mut c_void, unsafe extern "C" fn($($arg),*) $(-> $ret)?>(
                    pick(module_for($sym, studio_dll, core_dll, phonon, phonon_fmod), $sym)?
                ),)* })
            }
        }
    };
}

fn module_for(sym: &str, studio: *mut c_void, core: *mut c_void, phonon: *mut c_void, phonon_fmod: *mut c_void) -> *mut c_void {
    if sym.starts_with("FMOD_Studio_") {
        studio
    } else if sym.starts_with("FMOD_") {
        core
    } else if sym.starts_with("iplFMOD") {
        phonon_fmod
    } else {
        phonon
    }
}

api! {
    studio_core: "FMOD_Studio_System_GetCoreSystem" => fn(*mut c_void, *mut *mut c_void) -> i32;
    event_by_id: "FMOD_Studio_System_GetEventByID" => fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32;
    instance_list: "FMOD_Studio_EventDescription_GetInstanceList" => fn(*mut c_void, *mut *mut c_void, i32, *mut i32) -> i32;
    min_max: "FMOD_Studio_EventDescription_GetMinMaxDistance" => fn(*mut c_void, *mut f32, *mut f32) -> i32;
    instance_valid: "FMOD_Studio_EventInstance_IsValid" => fn(*mut c_void) -> i32;
    instance_group: "FMOD_Studio_EventInstance_GetChannelGroup" => fn(*mut c_void, *mut *mut c_void) -> i32;
    instance_3d: "FMOD_Studio_EventInstance_Get3DAttributes" => fn(*mut c_void, *mut Attributes3d) -> i32;
    listener_3d: "FMOD_Studio_System_GetListenerAttributes" => fn(*mut c_void, i32, *mut Attributes3d, *mut Vec3) -> i32;
    load_plugin: "FMOD_System_LoadPlugin" => fn(*mut c_void, *const u8, *mut u32, u32) -> i32;
    nested_count: "FMOD_System_GetNumNestedPlugins" => fn(*mut c_void, u32, *mut i32) -> i32;
    nested: "FMOD_System_GetNestedPlugin" => fn(*mut c_void, u32, i32, *mut u32) -> i32;
    plugin_info: "FMOD_System_GetDSPInfoByPlugin" => fn(*mut c_void, u32, *mut *const u8) -> i32;
    create_dsp: "FMOD_System_CreateDSPByPlugin" => fn(*mut c_void, u32, *mut *mut c_void) -> i32;
    software_format: "FMOD_System_GetSoftwareFormat" => fn(*mut c_void, *mut i32, *mut i32, *mut i32) -> i32;
    buffer_size: "FMOD_System_GetDSPBufferSize" => fn(*mut c_void, *mut u32, *mut i32) -> i32;
    group_dsp_count: "FMOD_ChannelGroup_GetNumDSPs" => fn(*mut c_void, *mut i32) -> i32;
    group_dsp: "FMOD_ChannelGroup_GetDSP" => fn(*mut c_void, i32, *mut *mut c_void) -> i32;
    group_add: "FMOD_ChannelGroup_AddDSP" => fn(*mut c_void, i32, *mut c_void) -> i32;
    group_remove: "FMOD_ChannelGroup_RemoveDSP" => fn(*mut c_void, *mut c_void) -> i32;
    dsp_type: "FMOD_DSP_GetType" => fn(*mut c_void, *mut i32) -> i32;
    dsp_bypass: "FMOD_DSP_SetBypass" => fn(*mut c_void, i32) -> i32;
    dsp_release: "FMOD_DSP_Release" => fn(*mut c_void) -> i32;
    dsp_param_count: "FMOD_DSP_GetNumParameters" => fn(*mut c_void, *mut i32) -> i32;
    dsp_param_info: "FMOD_DSP_GetParameterInfo" => fn(*mut c_void, i32, *mut *const ParamDesc) -> i32;
    dsp_get_int: "FMOD_DSP_GetParameterInt" => fn(*mut c_void, i32, *mut i32, *mut u8, i32) -> i32;
    dsp_set_int: "FMOD_DSP_SetParameterInt" => fn(*mut c_void, i32, i32) -> i32;
    dsp_set_bool: "FMOD_DSP_SetParameterBool" => fn(*mut c_void, i32, i32) -> i32;
    dsp_set_float: "FMOD_DSP_SetParameterFloat" => fn(*mut c_void, i32, f32) -> i32;
    dsp_set_data: "FMOD_DSP_SetParameterData" => fn(*mut c_void, i32, *const c_void, u32) -> i32;
    dsp_wet_dry: "FMOD_DSP_SetWetDryMix" => fn(*mut c_void, f32, f32, f32) -> i32;
    context_create: "iplContextCreate" => fn(*mut ContextSettings, *mut *mut c_void) -> i32;
    hrtf_create: "iplHRTFCreate" => fn(*mut c_void, *mut AudioSettings, *mut HrtfSettings, *mut *mut c_void) -> i32;
    steam_init: "iplFMODInitialize" => fn(*mut c_void);
    steam_hrtf: "iplFMODSetHRTF" => fn(*mut c_void);
    steam_simulation: "iplFMODSetSimulationSettings" => fn(SimulationSettings);
}

#[cfg(windows)]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const std::ffi::c_char) -> *mut c_void;
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

struct Tracked {
    instance: *mut c_void,
    group: *mut c_void,
    pan: *mut c_void,
    dsp: *mut c_void,
    rolloff: i32,
    min: f32,
    max: f32,
}

struct Params {
    position: i32,
}

pub struct Binaural {
    studio: *mut c_void,
    core: *mut c_void,
    api: Option<Api>,
    plugin: u32,
    params: Option<Params>,
    descriptions: Vec<(*mut c_void, f32, f32)>,
    guids: Vec<Guid>,
    tracked: HashMap<usize, Option<Tracked>>,
    failed: bool,
}

impl Binaural {
    pub fn new() -> Self {
        let guids = crate::sound_pack::plugin_dir()
            .and_then(|d| Some(d.parent()?.parent()?.join("fmod").join("GUIDs.txt")))
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|t| events(&t).into_iter().map(|(g, _)| g).collect())
            .unwrap_or_default();
        Self { studio: std::ptr::null_mut(), core: std::ptr::null_mut(), api: None, plugin: 0, params: None, descriptions: Vec::new(), guids, tracked: HashMap::new(), failed: false }
    }

    #[cfg(windows)]
    fn start(&mut self) -> Result<(), String> {
        let studio_fn = crate::xp::Xplm::xplm_symbol("XPLMGetFMODStudio").ok_or("no XPLMGetFMODStudio")?;
        let studio = unsafe { std::mem::transmute::<*mut c_void, unsafe extern "C" fn() -> *mut c_void>(studio_fn)() };
        if studio.is_null() {
            return Err("FMOD Studio not up yet".into());
        }
        let dir = crate::sound_pack::plugin_dir().ok_or("no plugin folder")?.join("steamaudio");
        let phonon_path = dir.join("phonon.dll");
        let fmod_plugin_path = dir.join("phonon_fmod.dll");
        if !phonon_path.exists() || !fmod_plugin_path.exists() {
            return Err(format!("Steam Audio missing in {}", dir.display()));
        }
        let (studio_dll, core_dll, phonon, phonon_fmod) = unsafe {
            (
                GetModuleHandleW(wide("fmodstudio.dll").as_ptr()),
                GetModuleHandleW(wide("fmod.dll").as_ptr()),
                LoadLibraryW(wide(&phonon_path.to_string_lossy()).as_ptr()),
                LoadLibraryW(wide(&fmod_plugin_path.to_string_lossy()).as_ptr()),
            )
        };
        if [studio_dll, core_dll, phonon, phonon_fmod].iter().any(|m| m.is_null()) {
            return Err("an FMOD or Steam Audio library did not load".into());
        }
        let api = unsafe { Api::load(studio_dll, core_dll, phonon, phonon_fmod) }.ok_or("a Steam Audio or FMOD entry point is missing")?;
        let mut core = std::ptr::null_mut();
        if unsafe { (api.studio_core)(studio, &mut core) } != 0 || core.is_null() {
            return Err("no FMOD core system".into());
        }
        let (mut rate, mut speakers, mut raw, mut frame, mut buffers) = (48000, 0, 0, 1024u32, 4);
        unsafe {
            (api.software_format)(core, &mut rate, &mut speakers, &mut raw);
            (api.buffer_size)(core, &mut frame, &mut buffers);
        }
        let mut context = std::ptr::null_mut();
        let mut cs = ContextSettings { version: STEAM_AUDIO_VERSION, log: std::ptr::null_mut(), allocate: std::ptr::null_mut(), free: std::ptr::null_mut(), simd_level: SIMD_AVX2, flags: 0 };
        if unsafe { (api.context_create)(&mut cs, &mut context) } != 0 {
            return Err("iplContextCreate failed".into());
        }
        let mut audio = AudioSettings { sampling_rate: rate, frame_size: frame as i32 };
        let mut hs = HrtfSettings { kind: 0, sofa_file_name: std::ptr::null(), sofa_data: std::ptr::null(), sofa_data_size: 0, volume: 1., norm_type: 0 };
        let mut hrtf = std::ptr::null_mut();
        if unsafe { (api.hrtf_create)(context, &mut audio, &mut hs, &mut hrtf) } != 0 {
            return Err("iplHRTFCreate failed".into());
        }
        unsafe {
            (api.steam_init)(context);
            (api.steam_hrtf)(hrtf);
            (api.steam_simulation)(SimulationSettings { flags: SIMULATION_DIRECT, sampling_rate: rate, frame_size: frame as i32, max_num_sources: MAX_INSTANCES as i32, num_threads: 1, ..Default::default() });
        }
        let path = format!("{}\0", fmod_plugin_path.to_string_lossy());
        let mut list = 0u32;
        if unsafe { (api.load_plugin)(core, path.as_ptr(), &mut list, 0) } != 0 {
            return Err("FMOD could not load phonon_fmod.dll".into());
        }
        let mut count = 0;
        unsafe { (api.nested_count)(core, list, &mut count) };
        let mut spatializer = None;
        for i in 0..count {
            let mut h = 0u32;
            let mut desc: *const u8 = std::ptr::null();
            unsafe {
                (api.nested)(core, list, i, &mut h);
                (api.plugin_info)(core, h, &mut desc);
            }
            if !desc.is_null() {
                let name = unsafe { std::slice::from_raw_parts(desc.add(4), 32) };
                if String::from_utf8_lossy(name).starts_with("Steam Audio Spatializer") {
                    spatializer = Some(h);
                }
            }
        }
        self.plugin = spatializer.ok_or("no Steam Audio Spatializer in phonon_fmod.dll")?;
        self.studio = studio;
        self.core = core;
        self.api = Some(api);
        crate::log(&format!("binaural: Steam Audio HRTF on X-Plane's FMOD ({rate} Hz, {frame}-sample frames)"));
        Ok(())
    }

    #[cfg(not(windows))]
    fn start(&mut self) -> Result<(), String> {
        Err("binaural audio is Windows-only".into())
    }

    fn param_indices(api: &Api, dsp: *mut c_void) -> HashMap<String, i32> {
        let mut n = 0;
        unsafe { (api.dsp_param_count)(dsp, &mut n) };
        let mut out = HashMap::new();
        for i in 0..n {
            let mut d: *const ParamDesc = std::ptr::null();
            if unsafe { (api.dsp_param_info)(dsp, i, &mut d) } == 0 && !d.is_null() {
                let raw = unsafe { (*d).name };
                let name = String::from_utf8_lossy(&raw).trim_end_matches('\0').to_string();
                out.insert(name, i);
            }
        }
        out
    }

    fn attach(&mut self, instance: *mut c_void, min: f32, max: f32) -> Option<Option<Tracked>> {
        let api = self.api.as_ref()?;
        let mut group = std::ptr::null_mut();
        if unsafe { (api.instance_group)(instance, &mut group) } != 0 || group.is_null() {
            return None;
        }
        Some(self.spatialize(instance, group, min, max))
    }

    fn spatialize(&mut self, instance: *mut c_void, group: *mut c_void, min: f32, max: f32) -> Option<Tracked> {
        let api = self.api.as_ref()?;
        let mut n = 0;
        unsafe { (api.group_dsp_count)(group, &mut n) };
        let (index, pan) = (0..n).find_map(|i| {
            let mut d = std::ptr::null_mut();
            let mut t = 0;
            unsafe {
                (api.group_dsp)(group, i, &mut d);
                (api.dsp_type)(d, &mut t);
            }
            (t == PAN_DSP_TYPE).then_some((i, d))
        })?;
        let mut dsp = std::ptr::null_mut();
        if unsafe { (api.create_dsp)(self.core, self.plugin, &mut dsp) } != 0 || dsp.is_null() {
            return None;
        }
        let names = Self::param_indices(api, dsp);
        let set_int = |name: &str, v: i32| {
            if let Some(&i) = names.get(name) {
                unsafe { (api.dsp_set_int)(dsp, i, v) };
            }
        };
        set_int("ApplyDA", 0);
        set_int("ApplyAA", 1);
        set_int("ApplyDir", 0);
        set_int("ApplyOccl", 0);
        set_int("ApplyTrans", 0);
        set_int("Interpolation", 1);
        if let Some(&i) = names.get("DirectBinaural") {
            unsafe { (api.dsp_set_bool)(dsp, i, 1) };
        }
        if let Some(&i) = names.get("DirMixLevel") {
            unsafe { (api.dsp_set_float)(dsp, i, 1.) };
        }
        if self.params.is_none() {
            self.params = Some(Params { position: *names.get("SourcePos")? });
        }
        let mut rolloff = 1;
        unsafe { (api.dsp_get_int)(pan, PAN_ROLLOFF_PARAM, &mut rolloff, std::ptr::null_mut(), 0) };
        if unsafe { (api.group_add)(group, index, dsp) } != 0 {
            unsafe { (api.dsp_release)(dsp) };
            return None;
        }
        unsafe { (api.dsp_bypass)(pan, 1) };
        Some(Tracked { instance, group, pan, dsp, rolloff, min, max })
    }

    fn detach(api: &Api, t: &Tracked, instance_alive: bool) {
        unsafe {
            if instance_alive {
                (api.group_remove)(t.group, t.dsp);
                (api.dsp_bypass)(t.pan, 0);
            }
            (api.dsp_release)(t.dsp);
        }
    }

    pub fn update(&mut self, enabled: bool) {
        if !enabled {
            self.release();
            return;
        }
        if self.failed || self.guids.is_empty() {
            return;
        }
        if self.api.is_none() {
            if let Err(e) = self.start() {
                if !e.contains("not up yet") {
                    crate::log(&format!("binaural: off, {e}"));
                    self.failed = true;
                }
                return;
            }
        }
        let Some(api) = self.api.as_ref() else { return };
        if self.descriptions.is_empty() {
            for g in &self.guids {
                let mut d = std::ptr::null_mut();
                if unsafe { (api.event_by_id)(self.studio, g, &mut d) } == 0 && !d.is_null() {
                    let (mut min, mut max) = (0f32, 0f32);
                    unsafe { (api.min_max)(d, &mut min, &mut max) };
                    self.descriptions.push((d, min, max));
                }
            }
            if self.descriptions.is_empty() {
                return;
            }
        }
        let mut live: Vec<(*mut c_void, f32, f32)> = Vec::new();
        let mut list = [std::ptr::null_mut(); MAX_INSTANCES];
        for &(d, min, max) in &self.descriptions {
            let mut n = 0;
            if unsafe { (api.instance_list)(d, list.as_mut_ptr(), MAX_INSTANCES as i32, &mut n) } == 0 {
                live.extend(list[..n.clamp(0, MAX_INSTANCES as i32) as usize].iter().map(|&i| (i, min, max)));
            }
        }
        let alive: std::collections::HashSet<usize> = live.iter().map(|(i, _, _)| *i as usize).collect();
        let gone: Vec<usize> = self.tracked.keys().filter(|k| !alive.contains(k)).copied().collect();
        for k in gone {
            if let Some(Some(t)) = self.tracked.remove(&k) {
                let valid = unsafe { (api.instance_valid)(t.instance) } != 0;
                Self::detach(api, &t, valid);
            }
        }
        for (instance, min, max) in live {
            if !self.tracked.contains_key(&(instance as usize)) {
                if let Some(t) = self.attach(instance, min, max) {
                    self.tracked.insert(instance as usize, t);
                }
            }
        }
        let Some(api) = self.api.as_ref() else { return };
        let Some(position) = self.params.as_ref().map(|p| p.position) else { return };
        let mut listener = Attributes3d::default();
        if unsafe { (api.listener_3d)(self.studio, 0, &mut listener, std::ptr::null_mut()) } != 0 {
            return;
        }
        for t in self.tracked.values().flatten() {
            let mut at = Attributes3d::default();
            if unsafe { (api.instance_3d)(t.instance, &mut at) } != 0 {
                continue;
            }
            let rel = Vec3 { x: at.position.x - listener.position.x, y: at.position.y - listener.position.y, z: at.position.z - listener.position.z };
            let data = DspPosition { relative: Attributes3d { position: rel, ..at }, absolute: at };
            let d = (rel.x * rel.x + rel.y * rel.y + rel.z * rel.z).sqrt();
            unsafe {
                (api.dsp_set_data)(t.dsp, position, (&data as *const DspPosition).cast(), std::mem::size_of::<DspPosition>() as u32);
                (api.dsp_wet_dry)(t.dsp, 1., rolloff_gain(t.rolloff, t.min, t.max, d), 0.);
            }
        }
    }

    pub fn release(&mut self) {
        if let Some(api) = self.api.as_ref() {
            for (_, t) in self.tracked.drain() {
                if let Some(t) = t {
                    let valid = unsafe { (api.instance_valid)(t.instance) } != 0;
                    Self::detach(api, &t, valid);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fallback_follows_fmods_own_rolloff_curves() {
        assert_eq!(rolloff_gain(1, 8.2, 25., 4.), 1., "inside the minimum distance");
        assert!((rolloff_gain(1, 8.2, 25., 15.) - 0.595).abs() < 0.01, "linear, measured 0.60 offline");
        assert_eq!(rolloff_gain(1, 8.2, 25., 30.), 0., "silent past the maximum");
        assert!((rolloff_gain(0, 8.2, 25., 15.) - 0.354).abs() < 0.01, "linear squared");
        assert!((rolloff_gain(2, 5., 100., 20.) - 0.25).abs() < 1e-6, "inverse");
        assert_eq!(rolloff_gain(1, 0., 0., 500.), 1., "no range: the event does its own attenuation");
    }

    #[test]
    fn the_structures_match_the_c_layouts() {
        assert_eq!(std::mem::size_of::<Attributes3d>(), 48);
        assert_eq!(std::mem::size_of::<DspPosition>(), 96);
        assert_eq!(std::mem::size_of::<SimulationSettings>(), 13 * 4 + 4 + 24);
    }
}

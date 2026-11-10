//! X-Plane 11's cockpit sounds: X-Plane 11 has no XPLMPlayPCMOnBus, so the plugin mixes its own PCM voices
//! and plays them through Windows' waveOut. Voices take the same calls X-Plane 12's channels do: play,
//! stop, volume and pitch (in Hz, the rate the samples play at); 3D position is not modelled.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

pub const RATE: u32 = 48_000;
const FRAMES: usize = 1024;
const BUFFERS: usize = 4;

struct Voice {
    samples: Arc<Vec<i16>>,
    channels: usize,
    pos: f64,
    hz: f64,
    looped: bool,
    volume: f32,
}

#[derive(Default)]
pub struct Mixer {
    voices: HashMap<u64, Voice>,
}

impl Mixer {
    pub fn add(&mut self, id: u64, samples: &[i16], channels: u16, rate: u32, looped: bool) {
        let channels = channels.clamp(1, 2) as usize;
        self.voices.insert(id, Voice { samples: Arc::new(samples.to_vec()), channels, pos: 0., hz: rate as f64, looped, volume: 1. });
    }

    pub fn stop(&mut self, id: u64) {
        self.voices.remove(&id);
    }

    pub fn set_volume(&mut self, id: u64, v: f32) {
        if let Some(voice) = self.voices.get_mut(&id) {
            voice.volume = v.max(0.);
        }
    }

    pub fn set_pitch(&mut self, id: u64, hz: f32) {
        if let Some(voice) = self.voices.get_mut(&id) {
            if hz > 0. {
                voice.hz = hz as f64;
            }
        }
    }

    pub fn playing(&self, id: u64) -> bool {
        self.voices.contains_key(&id)
    }

    /// Mixes `out.len() / 2` stereo frames at `RATE`, times `master`; finished one-shot voices are dropped.
    pub fn fill(&mut self, out: &mut [f32], master: f32) {
        out.iter_mut().for_each(|s| *s = 0.);
        let mut done = Vec::new();
        for (&id, v) in self.voices.iter_mut() {
            let frames = v.samples.len() / v.channels;
            if frames == 0 {
                done.push(id);
                continue;
            }
            let step = v.hz / RATE as f64;
            let gain = v.volume * master / 32768.;
            for frame in out.chunks_exact_mut(2) {
                if v.pos >= frames as f64 {
                    if v.looped {
                        v.pos %= frames as f64;
                    } else {
                        done.push(id);
                        break;
                    }
                }
                let i = v.pos as usize;
                let j = if i + 1 < frames { i + 1 } else if v.looped { 0 } else { i };
                let t = (v.pos - i as f64) as f32;
                let at = |k: usize, c: usize| v.samples[k * v.channels + c.min(v.channels - 1)] as f32;
                for c in 0..2 {
                    frame[c] += (at(i, c) * (1. - t) + at(j, c) * t) * gain;
                }
                v.pos += step;
            }
        }
        for id in done {
            self.voices.remove(&id);
        }
    }
}

static MIXER: OnceLock<Arc<Mutex<Mixer>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
static MASTER: AtomicU32 = AtomicU32::new(0x3f80_0000);
static STARTED: AtomicBool = AtomicBool::new(false);
static OUTPUT_OK: AtomicBool = AtomicBool::new(false);

fn mixer() -> &'static Arc<Mutex<Mixer>> {
    MIXER.get_or_init(|| Arc::new(Mutex::new(Mixer::default())))
}

/// Whether X-Plane 11's own output is up (started on first use).
pub fn available() -> bool {
    start();
    OUTPUT_OK.load(Ordering::Relaxed)
}

/// The master volume (X-Plane 11's master volume, zero while paused), set from the main thread.
pub fn set_master(v: f32) {
    MASTER.store(v.clamp(0., 1.).to_bits(), Ordering::Relaxed);
}

/// A voice handle as X-Plane 12's FMOD channels are: an opaque non-null pointer.
pub fn play(samples: &[i16], rate: u32, channels: u16, looped: bool) -> Option<*mut c_void> {
    if !available() {
        return None;
    }
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    mixer().lock().ok()?.add(id, samples, channels, rate, looped);
    Some(id as usize as *mut c_void)
}

pub fn stop(handle: *mut c_void) {
    if let Ok(mut m) = mixer().lock() {
        m.stop(handle as usize as u64);
    }
}

pub fn set_volume(handle: *mut c_void, v: f32) {
    if let Ok(mut m) = mixer().lock() {
        m.set_volume(handle as usize as u64, v);
    }
}

pub fn set_pitch(handle: *mut c_void, hz: f32) {
    if let Ok(mut m) = mixer().lock() {
        m.set_pitch(handle as usize as u64, hz);
    }
}

fn start() {
    if STARTED.swap(true, Ordering::Relaxed) {
        return;
    }
    #[cfg(windows)]
    {
        let ok = std::thread::Builder::new().name("fbw-xp11-audio".into()).spawn(wave::run).is_ok();
        if !ok {
            crate::log("xp11: no audio thread; cockpit sounds are off");
        }
    }
    #[cfg(not(windows))]
    crate::log("xp11: cockpit sounds need Windows on X-Plane 11");
}

#[cfg(windows)]
mod wave {
    use super::*;
    use std::ffi::{c_char, CString};

    #[repr(C, packed(2))]
    struct WaveFormatEx {
        format_tag: u16,
        channels: u16,
        samples_per_sec: u32,
        avg_bytes_per_sec: u32,
        block_align: u16,
        bits_per_sample: u16,
        cb_size: u16,
    }

    #[repr(C)]
    struct WaveHdr {
        data: *mut i16,
        buffer_length: u32,
        bytes_recorded: u32,
        user: usize,
        flags: u32,
        loops: u32,
        next: *mut WaveHdr,
        reserved: usize,
    }

    const WHDR_DONE: u32 = 1;
    const WAVE_MAPPER: u32 = 0xFFFF_FFFF;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryA(name: *const c_char) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    }

    type Open = unsafe extern "system" fn(*mut *mut c_void, u32, *const WaveFormatEx, usize, usize, u32) -> u32;
    type Hdr = unsafe extern "system" fn(*mut c_void, *mut WaveHdr, u32) -> u32;

    pub(super) fn run() {
        unsafe {
            let dll = CString::new("winmm.dll").expect("no nul");
            let lib = LoadLibraryA(dll.as_ptr());
            if lib.is_null() {
                crate::log("xp11: no winmm.dll; cockpit sounds are off");
                return;
            }
            let sym = |n: &str| {
                let c = CString::new(n).expect("no nul");
                GetProcAddress(lib, c.as_ptr())
            };
            let (open, prepare, write) = (sym("waveOutOpen"), sym("waveOutPrepareHeader"), sym("waveOutWrite"));
            if open.is_null() || prepare.is_null() || write.is_null() {
                crate::log("xp11: winmm has no waveOut; cockpit sounds are off");
                return;
            }
            let open: Open = std::mem::transmute(open);
            let prepare: Hdr = std::mem::transmute(prepare);
            let write: Hdr = std::mem::transmute(write);
            let format = WaveFormatEx { format_tag: 1, channels: 2, samples_per_sec: RATE, avg_bytes_per_sec: RATE * 4, block_align: 4, bits_per_sample: 16, cb_size: 0 };
            let mut device: *mut c_void = std::ptr::null_mut();
            let r = open(&mut device, WAVE_MAPPER, &format, 0, 0, 0);
            if r != 0 || device.is_null() {
                crate::log(&format!("xp11: waveOutOpen failed ({r}); cockpit sounds are off"));
                return;
            }
            let mut data: Vec<Vec<i16>> = (0..BUFFERS).map(|_| vec![0i16; FRAMES * 2]).collect();
            let mut headers: Vec<Box<WaveHdr>> = data
                .iter_mut()
                .map(|d| Box::new(WaveHdr { data: d.as_mut_ptr(), buffer_length: (FRAMES * 4) as u32, bytes_recorded: 0, user: 0, flags: 0, loops: 0, next: std::ptr::null_mut(), reserved: 0 }))
                .collect();
            for h in headers.iter_mut() {
                prepare(device, &mut **h, std::mem::size_of::<WaveHdr>() as u32);
                h.flags |= WHDR_DONE;
            }
            OUTPUT_OK.store(true, Ordering::Relaxed);
            crate::log("xp11: cockpit sounds play through Windows' waveOut at 48 kHz");
            let mut mix = vec![0f32; FRAMES * 2];
            loop {
                let mut wrote = false;
                for (h, d) in headers.iter_mut().zip(data.iter_mut()) {
                    let flags = std::ptr::read_volatile(&h.flags);
                    if flags & WHDR_DONE == 0 {
                        continue;
                    }
                    let master = f32::from_bits(MASTER.load(Ordering::Relaxed));
                    if let Ok(mut m) = mixer().lock() {
                        m.fill(&mut mix, master);
                    }
                    for (o, s) in d.iter_mut().zip(mix.iter()) {
                        *o = (s * 32767.).clamp(-32768., 32767.) as i16;
                    }
                    std::ptr::write_volatile(&mut h.flags, flags & !WHDR_DONE);
                    write(device, &mut **h, std::mem::size_of::<WaveHdr>() as u32);
                    wrote = true;
                }
                if !wrote {
                    std::thread::sleep(std::time::Duration::from_millis(3));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mixer_resamples_and_sums_two_voices() {
        let mut m = Mixer::default();
        m.add(1, &[1000i16; 22_050], 1, 22_050, false);
        m.add(2, &[1000i16; 48_000], 1, 48_000, false);
        m.set_volume(2, 0.5);
        let mut out = vec![0f32; 960];
        m.fill(&mut out, 1.);
        assert!((out[200] - 1500. / 32768.).abs() < 1e-4, "{}", out[200]);
        assert!((out[201] - 1500. / 32768.).abs() < 1e-4, "mono plays on both sides");
    }

    #[test]
    fn a_one_shot_voice_ends_and_a_looped_one_wraps() {
        let mut m = Mixer::default();
        m.add(1, &[100i16; 10], 1, RATE, false);
        m.add(2, &[100i16; 10], 1, RATE, true);
        let mut out = vec![0f32; 64];
        m.fill(&mut out, 1.);
        assert!(!m.playing(1));
        assert!(m.playing(2));
    }

    #[test]
    fn pitch_is_the_rate_the_samples_play_at() {
        let mut m = Mixer::default();
        m.add(1, &(0..100).map(|i| i as i16 * 100).collect::<Vec<_>>(), 1, RATE, false);
        m.set_pitch(1, (RATE * 2) as f32);
        let mut out = vec![0f32; 20];
        m.fill(&mut out, 1.);
        assert!((out[2] * 32768. - 200.).abs() < 1., "second frame reads two samples on: {}", out[2] * 32768.);
    }
}

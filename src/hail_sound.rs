//! Hail on the airframe: the sound pack has rain but no hail, so this plays
//! a synthesised loop of stones striking the skin and windshield while
//! `deep::environment` reports a hail shower (`ENV_HAIL_SHOWER`, its hail
//! intensity times X-Plane's precipitation on the aircraft). Louder with
//! airspeed, scaled by the Weather sound level, silent while paused.

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::xp::{self, DataRef, Xplm};
use crate::Vars;

const RATE: u32 = 44_100;
const LOOP_S: f32 = 6.0;
const START: f32 = 0.03;
const FADE_S: f32 = 1.5;

pub fn synthesise(seed: u64) -> Vec<i16> {
    let frames = (RATE as f32 * LOOP_S) as usize;
    let mut mix = vec![0f32; frames * 2];
    let mut rng = seed | 1;
    let mut rand = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng >> 11) as f32 / (1u64 << 53) as f32
    };
    let strikes = (LOOP_S * 90.0) as usize;
    for k in 0..strikes {
        let at = (rand() * frames as f32) as usize;
        let heavy = k % 9 == 0;
        let amp = if heavy { 0.55 + 0.45 * rand() } else { 0.08 + 0.35 * rand() * rand() };
        let decay_s = if heavy { 0.018 + 0.02 * rand() } else { 0.003 + 0.006 * rand() };
        let len = (decay_s * 6.0 * RATE as f32) as usize;
        let pan = rand();
        let (gl, gr) = ((1.0 - pan).sqrt(), pan.sqrt());
        let smooth = if heavy { 0.55 } else { 0.08 };
        let mut prev = 0f32;
        for i in 0..len {
            let t = i as f32 / RATE as f32;
            let noise = rand() * 2.0 - 1.0;
            let shaped = prev + (noise - prev) * (1.0 - smooth);
            let click = if heavy { shaped } else { noise - prev };
            prev = shaped;
            let v = click * amp * (-t / decay_s).exp();
            let f = (at + i) % frames;
            mix[f * 2] += v * gl;
            mix[f * 2 + 1] += v * gr;
        }
    }
    let peak = mix.iter().fold(0f32, |m, v| m.max(v.abs())).max(1e-6);
    mix.iter().map(|v| (v / peak * 0.85 * i16::MAX as f32) as i16).collect()
}

pub fn target_level(shower: f64, tas_ms: f64) -> f32 {
    if shower < START as f64 {
        return 0.0;
    }
    let speed = (tas_ms / 150.0).clamp(0.0, 1.0);
    ((0.25 + 0.75 * shower.min(1.0)) * (0.45 + 0.55 * speed)) as f32
}

pub struct HailSound {
    shower: VariableIdentifier,
    weather_level: VariableIdentifier,
    tas: Option<DataRef>,
    paused: Option<DataRef>,
    external: Option<DataRef>,
    pcm: Vec<i16>,
    channel: Option<(xp::FmodChannel, bool)>,
    level: f32,
}

impl HailSound {
    pub fn new(vars: &mut Vars, xplm: &Xplm) -> Self {
        Self {
            shower: vars.get("ENV_HAIL_SHOWER".into()),
            weather_level: vars.get("SOUND_LEVEL_WEATHER".into()),
            tas: xplm.find("sim/flightmodel/position/true_airspeed"),
            paused: xplm.find("sim/time/paused"),
            external: xplm.find("sim/graphics/view/view_is_external"),
            pcm: Vec::new(),
            channel: None,
            level: 0.0,
        }
    }

    pub fn update(&mut self, vars: &mut Vars, xplm: &Xplm, dt_s: f64) {
        let paused = self.paused.is_some_and(|d| xplm.get_i(d) != 0);
        let tas = self.tas.map_or(0.0, |d| xplm.get_f(d) as f64);
        let want = if paused { 0.0 } else { target_level(vars.read(&self.shower), tas) };
        let step = (dt_s as f32 / FADE_S).clamp(0.0, 1.0);
        self.level += (want - self.level).clamp(-step, step);
        if paused || self.level < 0.005 {
            self.stop();
            return;
        }
        if !xp::has_pcm_audio() {
            return;
        }
        let outside = self.external.is_some_and(|d| xplm.get_i(d) != 0);
        if self.channel.is_some_and(|(_, o)| o != outside) {
            self.stop();
        }
        if self.channel.is_none() {
            if self.pcm.is_empty() {
                self.pcm = synthesise(0x5eed_4a11);
            }
            let bus = if outside { xp::AUDIO_EXTERIOR_AIRCRAFT } else { xp::AUDIO_INTERIOR };
            self.channel = xp::play_pcm16_on_bus(&self.pcm, RATE, 2, true, bus, None, std::ptr::null_mut()).map(|c| (c, outside));
        }
        if let Some((c, outside)) = self.channel {
            let weather = (vars.read(&self.weather_level) / 100.).clamp(0., 1.) as f32;
            xp::set_audio_volume(c, self.level * weather * if outside { 0.7 } else { 1.0 });
        }
    }

    fn stop(&mut self) {
        if let Some((c, _)) = self.channel.take() {
            xp::stop_audio(c);
        }
    }

    pub fn release(&mut self) {
        self.stop();
        self.level = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_loop_is_stereo_full_scale_and_repeatable() {
        let a = synthesise(7);
        assert_eq!(a.len(), (RATE as f32 * LOOP_S) as usize * 2);
        let peak = a.iter().map(|v| v.unsigned_abs()).max().unwrap();
        assert!(peak > 27_000 && peak <= i16::MAX as u16);
        assert_eq!(a, synthesise(7));
    }

    #[test]
    fn silent_without_a_shower_and_louder_with_speed() {
        assert_eq!(target_level(0.0, 200.0), 0.0);
        assert_eq!(target_level(0.02, 200.0), 0.0);
        let parked = target_level(0.8, 0.0);
        let flying = target_level(0.8, 150.0);
        assert!(parked > 0.0 && flying > parked && flying <= 1.0);
    }
}

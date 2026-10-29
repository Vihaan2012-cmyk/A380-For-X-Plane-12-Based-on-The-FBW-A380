use crate::throttle::Throttler;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Sound {
    pub name: &'static str,
    pub length: f64,
}

pub const PULL_UP: Sound = Sound { name: "aural_pullup_new", length: 0.9 };
pub const SINK_RATE: Sound = Sound { name: "aural_sink_rate_new", length: 0.9 };
pub const DONT_SINK: Sound = Sound { name: "aural_dontsink_new", length: 0.9 };
pub const TOO_LOW_GEAR: Sound = Sound { name: "aural_too_low_gear", length: 0.8 };
pub const TOO_LOW_FLAPS: Sound = Sound { name: "aural_too_low_flaps", length: 0.8 };
pub const TOO_LOW_TERRAIN: Sound = Sound { name: "aural_too_low_terrain", length: 0.9 };
pub const TERRAIN_AHEAD: Sound = Sound { name: "aural_terrain_ahead", length: 1.1 };
pub const TERRAIN_AHEAD_PULL_UP: Sound = Sound { name: "aural_terrain_ahead_pull_up", length: 1.8 };
pub const OBSTACLE_AHEAD: Sound = Sound { name: "aural_obstacle_ahead", length: 1.2 };
pub const OBSTACLE_AHEAD_PULL_UP: Sound = Sound { name: "aural_obstacle_ahead_pull_up", length: 1.9 };

#[derive(Clone, Debug)]
struct Periodic {
    sound: Sound,
    period: f64,
    time_since_last_played: f64,
}

#[derive(Clone, Debug)]
pub struct SoundManager {
    throttler: Throttler,
    periodic: Vec<Periodic>,
    playing: Option<Sound>,
    playing_remaining: f64,
    played: Vec<&'static str>,
}

impl SoundManager {
    pub fn new() -> SoundManager {
        SoundManager::with_throttle_offset(0.)
    }

    pub fn with_throttle_offset(offset_ms: f64) -> SoundManager {
        SoundManager {
            throttler: Throttler::new(50., offset_ms),
            periodic: Vec::new(),
            playing: None,
            playing_remaining: f64::NAN,
            played: Vec::new(),
        }
    }

    pub fn add_periodic_sound(&mut self, sound: Option<Sound>, period: f64) {
        let Some(sound) = sound else { return };
        let use_length_for_period = period < sound.length;
        if !self.periodic.iter().any(|p| p.sound.name == sound.name) {
            self.periodic.push(Periodic {
                sound,
                period: if use_length_for_period { sound.length } else { period },
                time_since_last_played: f64::NAN,
            });
        }
    }

    pub fn remove_periodic_sound(&mut self, sound: Option<Sound>) {
        let Some(sound) = sound else { return };
        self.periodic.retain(|p| p.sound.name != sound.name);
    }

    pub fn try_play_sound(&mut self, sound: Sound) -> bool {
        if self.playing.is_none() {
            self.playing = Some(sound);
            self.playing_remaining = sound.length;
            self.played.push(sound.name);
            return true;
        }
        false
    }

    pub fn update(&mut self, delta_ms: f64) {
        let throttled = self.throttler.can_update(delta_ms);
        if throttled <= 0. {
            return;
        }

        if self.playing_remaining <= 0. {
            self.playing = None;
            self.playing_remaining = f64::NAN;
        } else if self.playing_remaining > 0. {
            self.playing_remaining -= throttled / 1000.;
        }

        for i in 0..self.periodic.len() {
            let due = {
                let e = &self.periodic[i];
                e.time_since_last_played.is_nan() || e.time_since_last_played >= e.period
            };
            if due {
                let sound = self.periodic[i].sound;
                if self.try_play_sound(sound) {
                    self.periodic[i].time_since_last_played = 0.;
                }
            } else {
                self.periodic[i].time_since_last_played += throttled / 1000.;
            }
        }
    }

    pub fn take_played(&mut self) -> Vec<&'static str> {
        std::mem::take(&mut self.played)
    }

    pub fn periodic_names(&self) -> Vec<&'static str> {
        self.periodic.iter().map(|p| p.sound.name).collect()
    }
}

impl Default for SoundManager {
    fn default() -> Self {
        SoundManager::new()
    }
}

use crate::types::{Config, PaSource, ANNOUNCEMENTS};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaOut {
    pub deck: [PaSource; 3],
    pub announcement: Option<u8>,
    pub announcement_left_s: f64,
    pub music_on: bool,
    pub music_track: u8,
    pub music_volume: f64,
    pub deck_ok: [bool; 3],
    pub degraded: bool,
}

pub struct PaInputs {
    pub announce: Option<u8>,
    pub music: Option<bool>,
    pub track: Option<u8>,
    pub volume: Option<u8>,
    pub stop_all: bool,
    pub auto_announcement: Option<u8>,
    pub handset: bool,
    pub cockpit: bool,
    pub evac_active: bool,
    pub boarding: bool,
    pub deck_ok: [bool; 3],
    pub degraded: bool,
}

pub struct Pa {
    music_on: bool,
    track: u8,
    volume: u8,
    ann: Option<(u8, f64)>,
    suppress_boarding: bool,
    boarding_started: bool,
}

impl Pa {
    pub fn new() -> Self {
        Self { music_on: false, track: 1, volume: 60, ann: None, suppress_boarding: false, boarding_started: false }
    }

    pub fn volume_pct(&self) -> u8 {
        self.volume
    }

    pub fn step(&mut self, i: &PaInputs, cfg: &Config, dt: f64) -> PaOut {
        if let Some(t) = i.track {
            self.track = t.clamp(1, 4);
        }
        if let Some(v) = i.volume {
            self.volume = v.min(100);
        }
        if i.stop_all {
            self.ann = None;
            if self.music_on {
                self.suppress_boarding = true;
            }
            self.music_on = false;
        }
        if let Some(m) = i.music {
            self.music_on = m;
            self.suppress_boarding = !m;
        }
        if i.boarding {
            if cfg.boarding_music_auto && !self.suppress_boarding && !self.boarding_started {
                self.music_on = true;
                self.boarding_started = true;
            }
        } else {
            if self.boarding_started {
                self.music_on = false;
            }
            self.boarding_started = false;
            self.suppress_boarding = false;
        }
        let request = i.announce.or(i.auto_announcement);
        match request {
            Some(1) => self.music_on = true,
            Some(n) if (2..ANNOUNCEMENTS as u8).contains(&n) => self.ann = Some((n, cfg.announcement_len_s)),
            _ => {}
        }
        if i.evac_active {
            self.ann = None;
            self.music_on = false;
        }
        if let Some((n, left)) = self.ann {
            let left = left - dt;
            self.ann = if left > 0. { Some((n, left)) } else { None };
        }

        let live = if i.cockpit {
            Some(PaSource::Cockpit)
        } else if i.handset {
            Some(PaSource::Attendant)
        } else {
            None
        };
        let base = if let Some(l) = live {
            l
        } else if let Some((n, _)) = self.ann {
            PaSource::PreRecorded(n)
        } else if self.music_on {
            PaSource::Music
        } else {
            PaSource::None
        };
        let deck = std::array::from_fn(|d| if i.deck_ok[d] { base } else { PaSource::None });
        PaOut {
            deck,
            announcement: self.ann.map(|a| a.0),
            announcement_left_s: self.ann.map_or(0., |a| a.1),
            music_on: self.music_on,
            music_track: self.track,
            music_volume: if base == PaSource::Music { self.volume as f64 / 100. } else { 0. },
            deck_ok: i.deck_ok,
            degraded: i.degraded,
        }
    }
}

impl Default for Pa {
    fn default() -> Self {
        Self::new()
    }
}

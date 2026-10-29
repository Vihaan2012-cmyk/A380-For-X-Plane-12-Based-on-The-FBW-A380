//! FwsSoundManager.ts.

use crate::fwc::Shared;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    SingleChime,
    AuralWarning,
    SyntheticVoice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Volume {
    Full,
    Attenuated,
    Silent,
}

struct Aural {
    key: &'static str,
    var: Option<&'static str>,
    wwise: Option<&'static str>,
    priority: i32,
    kind: Kind,
    length: Option<f64>,
    periodic: bool,
    continuous: bool,
}

const fn lv(key: &'static str, var: &'static str, length: Option<f64>, priority: i32, kind: Kind, continuous: bool) -> Aural {
    Aural { key, var: Some(var), wwise: None, priority, kind, length, periodic: false, continuous }
}

const fn ww(key: &'static str, event: &'static str, length: f64, priority: i32) -> Aural {
    Aural { key, var: None, wwise: Some(event), priority, kind: Kind::SyntheticVoice, length: Some(length), periodic: false, continuous: false }
}

static AURALS: &[Aural] = &[
    lv("continuousRepetitiveChime", "A32NX_FWC_CRC", None, 5, Kind::AuralWarning, true),
    lv("singleChime", "A32NX_FWC_SC", Some(0.54), 0, Kind::SingleChime, false),
    lv("cavalryChargeOnce", "A32NX_FWC_CAVALRY_CHARGE", Some(0.9), 4, Kind::AuralWarning, false),
    lv("cavalryChargeCont", "A32NX_FWC_CAVALRY_CHARGE", None, 4, Kind::AuralWarning, true),
    lv("tripleClick", "A32NX_FMA_TRIPLE_CLICK", Some(0.62), 3, Kind::AuralWarning, false),
    lv("v1", "A32NX_AUDIO_V1_CALLOUT", Some(1.3), 1, Kind::SyntheticVoice, false),
    lv("runwayTooShort", "A32NX_AUDIO_ROW_RWY_TOO_SHORT", Some(1.6), 4, Kind::SyntheticVoice, true),
    lv("keepMaxReverse", KEEP_MAX_REVERSE, Some(1.4), 18, Kind::SyntheticVoice, false),
    lv("autoBrakeOff", "A32NX_AUDIO_AUTOBRAKE_OFF", Some(1.5), 17, Kind::SyntheticVoice, false),
    lv("setMaxReverse", "A32NX_AUDIO_ROW_SET_MAX_REVERSE", Some(1.62), 19, Kind::SyntheticVoice, false),
    lv("brakeMaxBraking", "A32NX_AUDIO_ROP_MAX_BRAKING", Some(3.1), 20, Kind::SyntheticVoice, true),
    lv("stall", "A32NX_AUDIO_ROP_MAX_BRAKING", Some(3.0), 5, Kind::SyntheticVoice, true),
    lv("cChordOnce", "A32NX_ALT_DEVIATION", Some(1.0), 3, Kind::AuralWarning, false),
    lv("cChordCont", "A32NX_ALT_DEVIATION", None, 3, Kind::AuralWarning, true),
    ww("minimums", "aural_minimumnew", 0.67, 15),
    ww("hundred_above", "aural_100above", 0.72, 16),
    Aural { key: "retard", var: None, wwise: Some("new_retard"), priority: 22, kind: Kind::SyntheticVoice, length: Some(0.9), periodic: true, continuous: false },
    ww("alt_2500", "new_2500", 1.1, 0),
    ww("alt_2500b", "new_2_500", 1.047, 1),
    ww("alt_2000", "new_2000", 0.72, 2),
    ww("alt_1000", "new_1000", 0.9, 3),
    ww("alt_500", "new_500", 0.6, 4),
    ww("alt_400", "new_400", 0.6, 5),
    ww("alt_300", "new_300", 0.6, 6),
    ww("alt_200", "new_200", 0.6, 7),
    ww("alt_100", "new_100", 0.6, 8),
    ww("alt_90", "90_380", 0.4, 8),
    ww("alt_80", "80_380", 0.4, 8),
    ww("alt_70", "70_380", 0.4, 8),
    ww("alt_60", "60_380", 0.4, 8),
    ww("alt_50", "50_380", 0.4, 9),
    ww("alt_40", "40_380", 0.4, 10),
    ww("alt_30", "30_380", 0.4, 11),
    ww("alt_20", "20_380", 0.4, 12),
    ww("alt_10", "10_380", 0.3, 13),
    ww("alt_5", "5_380", 0.3, 14),
];

const KEEP_MAX_REVERSE: &str = "A32NX_AUDIO_ROP_KEEP_MAX_REVERSE";

/// The sound key whose Wwise event is `event` (FBW's `FwsAuralsList`).
pub fn key_for_event(event: &str) -> Option<&'static str> {
    AURALS.iter().find(|a| a.wwise == Some(event)).map(|a| a.key)
}

fn aural(key: &str) -> Option<&'static Aural> {
    AURALS.iter().find(|a| a.key == key)
}

pub struct Sound {
    io: Shared,
    queue: Vec<&'static str>,
    single_chimes_pending: u32,
    current: Option<&'static str>,
    remaining: f64,
    keep_max_reverse: bool,
}

impl Sound {
    pub fn new(io: Shared) -> Self {
        let s = Self { io, queue: Vec::new(), single_chimes_pending: 0, current: None, remaining: 0., keep_max_reverse: false };
        for a in AURALS {
            if let Some(var) = a.var {
                s.var(var, false);
            }
        }
        s
    }

    fn var(&self, name: &str, on: bool) {
        self.io.borrow_mut().var(&format!("L:{name}"), on as u8 as f64);
    }

    pub fn enqueue_sound(&mut self, key: &str) {
        let Some(s) = aural(key) else { return };
        if self.current == Some(s.key) {
            return;
        }
        if s.kind == Kind::SingleChime {
            self.single_chimes_pending += 1;
        } else if !self.queue.contains(&s.key) {
            self.queue.push(s.key);
        }
    }

    pub fn dequeue_sound(&mut self, key: &str) {
        if self.current.is_some_and(|c| c == key && aural(c).is_some_and(|a| a.continuous)) {
            self.stop_current_sound();
        }
        self.queue.retain(|k| *k != key);
    }

    fn stop_current_sound(&mut self) {
        let Some(cur) = self.current else { return };
        let s = aural(cur).expect("aural");
        let Some(var) = s.var.filter(|_| s.continuous) else { return };
        self.var(var, false);
        self.current = None;
        self.remaining = 0.;
        self.set_output(var, false);
    }

    pub fn handle_sound_condition(&mut self, key: &str, condition: bool) {
        if condition && self.current.is_none_or(|c| c != key) {
            self.enqueue_sound(key);
        } else if !condition {
            self.dequeue_sound(key);
        }
    }

    pub fn set_volume(&mut self, v: Volume) {
        self.io.borrow_mut().var("L:A32NX_FWS_AUDIO_VOLUME", v as u8 as f64);
    }

    fn play_sound(&mut self, key: &'static str) {
        let Some(s) = aural(key) else { return };
        if let Some(var) = s.var {
            self.var(var, true);
            self.set_output(var, true);
        } else if let Some(ev) = s.wwise {
            self.io.borrow_mut().call("PLAY_INSTRUMENT_SOUND", serde_json::json!([ev]).to_string());
        }
        self.current = Some(key);
        self.remaining = if s.continuous { f64::INFINITY } else { s.length.unwrap_or(0.) };
        self.queue.retain(|k| *k != key);
        let _ = s.periodic;
    }

    fn select_and_play(&mut self, startup_completed: bool, audio_lost: bool) {
        if !startup_completed || audio_lost {
            return;
        }
        let mut selected: Option<&'static Aural> = None;
        for k in &self.queue {
            let s = aural(k).expect("aural");
            if selected.is_none_or(|sel| s.kind > sel.kind || (s.kind == sel.kind && s.priority > sel.priority)) {
                selected = Some(s);
            }
        }
        if let Some(s) = selected {
            self.play_sound(s.key);
            return;
        }
        if self.single_chimes_pending > 0 {
            self.play_sound("singleChime");
            self.single_chimes_pending -= 1;
        }
    }

    pub fn on_update(&mut self, dt_ms: f64, startup_completed: bool, audio_lost: bool) {
        if audio_lost {
            while self.current.is_some() {
                let before = self.current;
                self.stop_current_sound();
                if self.current == before {
                    break;
                }
            }
            return;
        }
        if self.current.is_some() && self.remaining > 0. {
            if self.remaining - dt_ms / 1000. > 0. {
                self.remaining -= dt_ms / 1000.;
            } else {
                let s = aural(self.current.unwrap()).expect("aural");
                if let Some(var) = s.var {
                    self.var(var, false);
                    self.set_output(var, false);
                }
                self.current = None;
                self.remaining = 0.;
            }
            let cur_continuous = self.current.and_then(aural).filter(|c| c.continuous);
            let interrupt = cur_continuous.is_some_and(|c| self.queue.iter().filter_map(|k| aural(k)).any(|s| s.kind > c.kind));
            if interrupt {
                if let Some(c) = cur_continuous {
                    self.stop_current_sound();
                    self.enqueue_sound(c.key);
                }
            }
        } else {
            self.select_and_play(startup_completed, audio_lost);
        }
    }

    fn set_output(&mut self, var: &str, value: bool) {
        if var == KEEP_MAX_REVERSE {
            self.keep_max_reverse = value;
        }
    }

    pub fn keep_max_reverse_played(&self) -> bool {
        self.keep_max_reverse
    }
}

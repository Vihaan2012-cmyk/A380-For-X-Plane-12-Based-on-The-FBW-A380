use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseEvent {
    Pushback,
    Takeoff,
    TopOfDescent,
    Landing,
    Gate,
}

impl PhaseEvent {
    pub const ALL: [PhaseEvent; 5] = [PhaseEvent::Pushback, PhaseEvent::Takeoff, PhaseEvent::TopOfDescent, PhaseEvent::Landing, PhaseEvent::Gate];

    pub fn slot_id(self) -> &'static str {
        match self {
            PhaseEvent::Pushback => "phase-pushback",
            PhaseEvent::Takeoff => "phase-takeoff",
            PhaseEvent::TopOfDescent => "phase-tod",
            PhaseEvent::Landing => "phase-landing",
            PhaseEvent::Gate => "phase-gate",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PhaseEvent::Pushback => "Pushback",
            PhaseEvent::Takeoff => "Take-off",
            PhaseEvent::TopOfDescent => "Top of descent",
            PhaseEvent::Landing => "Landing",
            PhaseEvent::Gate => "At the gate",
        }
    }
}

/// FlyByWire's `A32NX_FMGC_FLIGHT_PHASE` values.
pub mod fmgc {
    pub const PREFLIGHT: u8 = 0;
    pub const TAKEOFF: u8 = 1;
    pub const CLIMB: u8 = 2;
    pub const CRUISE: u8 = 3;
    pub const DESCENT: u8 = 4;
    pub const APPROACH: u8 = 5;
    pub const GOAROUND: u8 = 6;
    pub const DONE: u8 = 7;
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sample {
    pub fmgc_phase: u8,
    pub on_ground: bool,
    pub ground_speed_kt: f64,
    pub pushback_active: bool,
    pub any_engine_running: bool,
    pub parking_brake_set: bool,
}

/// Each event fires at most once per flight; arriving at the gate re-arms them.
#[derive(Clone, Debug, Default)]
pub struct PhaseWatch {
    last: Option<Sample>,
    airborne_seen: bool,
    fired: Vec<PhaseEvent>,
}

const GATE_GROUND_SPEED_KT: f64 = 1.0;

impl PhaseWatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// The first sample after a load only primes the watch, so a load never
    /// immediately overwrites the slot it came from.
    pub fn prime(&mut self, s: Sample, airborne_seen: bool) {
        self.last = Some(s);
        self.airborne_seen = airborne_seen || !s.on_ground;
    }

    pub fn update(&mut self, s: Sample) -> Option<PhaseEvent> {
        let Some(prev) = self.last.replace(s) else {
            self.airborne_seen = !s.on_ground;
            return None;
        };
        if !s.on_ground {
            self.airborne_seen = true;
        }
        let event = if s.pushback_active && !prev.pushback_active {
            Some(PhaseEvent::Pushback)
        } else if s.fmgc_phase == fmgc::TAKEOFF && prev.fmgc_phase != fmgc::TAKEOFF && s.on_ground {
            Some(PhaseEvent::Takeoff)
        } else if s.fmgc_phase == fmgc::DESCENT && prev.fmgc_phase == fmgc::CRUISE {
            Some(PhaseEvent::TopOfDescent)
        } else if s.on_ground && !prev.on_ground && self.airborne_seen {
            Some(PhaseEvent::Landing)
        } else if self.airborne_seen
            && s.on_ground
            && !s.any_engine_running
            && s.parking_brake_set
            && s.ground_speed_kt < GATE_GROUND_SPEED_KT
            && !(prev.on_ground && !prev.any_engine_running && prev.parking_brake_set && prev.ground_speed_kt < GATE_GROUND_SPEED_KT)
        {
            Some(PhaseEvent::Gate)
        } else {
            None
        };
        let event = event.filter(|e| !self.fired.contains(e))?;
        self.fired.push(event);
        if event == PhaseEvent::Gate {
            self.fired.clear();
            self.airborne_seen = false;
        }
        Some(event)
    }
}

/// Real seconds, counted only while the sim runs and not during a load.
#[derive(Clone, Debug)]
pub struct AutosaveClock {
    pub interval_s: f64,
    since_s: f64,
}

pub const DEFAULT_AUTOSAVE_INTERVAL_S: f64 = 300.0;

impl Default for AutosaveClock {
    fn default() -> Self {
        Self { interval_s: DEFAULT_AUTOSAVE_INTERVAL_S, since_s: 0.0 }
    }
}

impl AutosaveClock {
    pub fn from_settings(settings: &crate::slots::AutosaveSettings) -> Self {
        Self { interval_s: settings.interval_s(), since_s: 0.0 }
    }

    pub fn tick(&mut self, delta_s: f64, sim_running: bool) -> bool {
        if !sim_running || !delta_s.is_finite() || delta_s <= 0.0 || self.interval_s <= 0.0 {
            return false;
        }
        self.since_s += delta_s;
        if self.since_s >= self.interval_s {
            self.since_s = 0.0;
            return true;
        }
        false
    }

    pub fn reset(&mut self) {
        self.since_s = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate() -> Sample {
        Sample { fmgc_phase: fmgc::PREFLIGHT, on_ground: true, ground_speed_kt: 0.0, pushback_active: false, any_engine_running: false, parking_brake_set: true }
    }

    #[test]
    fn a_whole_flight_fires_each_event_once_in_order() {
        let mut w = PhaseWatch::new();
        let mut seen = Vec::new();
        let mut feed = |s: Sample, w: &mut PhaseWatch| {
            if let Some(e) = w.update(s) {
                seen.push(e);
            }
        };
        let g = gate();
        feed(g, &mut w);
        let push = Sample { pushback_active: true, parking_brake_set: false, ground_speed_kt: 2.0, ..g };
        feed(push, &mut w);
        feed(Sample { pushback_active: false, any_engine_running: true, ..push }, &mut w);
        feed(Sample { pushback_active: true, any_engine_running: true, ..push }, &mut w);
        let roll = Sample { fmgc_phase: fmgc::TAKEOFF, any_engine_running: true, ground_speed_kt: 30.0, parking_brake_set: false, ..g };
        feed(roll, &mut w);
        let air = Sample { on_ground: false, ground_speed_kt: 200.0, ..roll };
        feed(air, &mut w);
        feed(Sample { fmgc_phase: fmgc::CRUISE, ..air }, &mut w);
        feed(Sample { fmgc_phase: fmgc::DESCENT, ..air }, &mut w);
        feed(Sample { fmgc_phase: fmgc::APPROACH, ..air }, &mut w);
        let touch = Sample { fmgc_phase: fmgc::APPROACH, on_ground: true, ground_speed_kt: 140.0, ..air };
        feed(touch, &mut w);
        feed(Sample { ground_speed_kt: 10.0, ..touch }, &mut w);
        feed(Sample { fmgc_phase: fmgc::DONE, ..gate() }, &mut w);
        feed(gate(), &mut w);
        assert_eq!(seen, [PhaseEvent::Pushback, PhaseEvent::Takeoff, PhaseEvent::TopOfDescent, PhaseEvent::Landing, PhaseEvent::Gate]);
        let mut next = Vec::new();
        if let Some(e) = w.update(push) {
            next.push(e);
        }
        assert_eq!(next, [PhaseEvent::Pushback]);
    }

    #[test]
    fn a_cold_spawn_at_the_gate_is_not_an_arrival() {
        let mut w = PhaseWatch::new();
        assert_eq!(w.update(Sample { parking_brake_set: false, ..gate() }), None);
        assert_eq!(w.update(gate()), None);
    }

    #[test]
    fn a_primed_watch_does_not_fire_on_the_state_it_was_loaded_into() {
        let mut w = PhaseWatch::new();
        let air = Sample { fmgc_phase: fmgc::DESCENT, on_ground: false, any_engine_running: true, ground_speed_kt: 300.0, ..gate() };
        w.prime(air, true);
        assert_eq!(w.update(air), None);
        assert_eq!(w.update(Sample { on_ground: true, ..air }), Some(PhaseEvent::Landing));
    }

    #[test]
    fn the_autosave_clock_counts_only_running_time() {
        let mut c = AutosaveClock::default();
        assert!(!c.tick(299.0, true));
        assert!(!c.tick(100.0, false));
        assert!(c.tick(1.0, true));
        assert!(!c.tick(1.0, true));
        assert!(!c.tick(f64::NAN, true));
        let mut off = AutosaveClock::from_settings(&crate::slots::AutosaveSettings { interval_min: 0, ..Default::default() });
        assert!(!off.tick(1.0e6, true));
    }
}

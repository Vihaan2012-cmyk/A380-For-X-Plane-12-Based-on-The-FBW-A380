use crate::types::{Flight, Phase};

pub struct PhaseTracker {
    pub phase: Phase,
    airborne_seen: bool,
}

impl PhaseTracker {
    pub fn new() -> Self {
        Self { phase: Phase::Parked, airborne_seen: false }
    }

    pub fn step(&mut self, f: &Flight) -> Phase {
        self.phase = if f.on_ground {
            let moving = f.gs_kt >= 1. || f.engines_running();
            if self.airborne_seen {
                if f.gs_kt > 40. {
                    Phase::Rollout
                } else if moving {
                    Phase::TaxiIn
                } else {
                    self.airborne_seen = false;
                    Phase::Parked
                }
            } else if f.gs_kt > 40. && f.engines_running() {
                Phase::TakeoffRoll
            } else if moving {
                Phase::TaxiOut
            } else {
                Phase::Parked
            }
        } else {
            self.airborne_seen = true;
            if f.gear_down || (f.vs_fpm < -300. && f.radio_alt_ft < 3000.) || (f.slats_deg >= 19. && f.radio_alt_ft < 3000.) {
                Phase::Approach
            } else if f.vs_fpm < -300. {
                Phase::Descent
            } else if f.vs_fpm > 300. || f.radio_alt_ft < 1500. {
                Phase::Climb
            } else {
                Phase::Cruise
            }
        };
        self.phase
    }
}

impl Default for PhaseTracker {
    fn default() -> Self {
        Self::new()
    }
}

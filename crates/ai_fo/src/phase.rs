use crate::Snapshot;

pub const FLX_TLA: f64 = 34.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Preflight,
    TaxiOut,
    TakeoffRoll,
    Climb,
    Cruise,
    Descent,
    Approach,
    GoAround,
    Rollout,
    TaxiIn,
    Parked,
}

#[derive(Clone, Debug)]
pub struct PhaseTracker {
    phase: Phase,
    flown: bool,
}

impl Default for PhaseTracker {
    fn default() -> Self {
        PhaseTracker { phase: Phase::Preflight, flown: false }
    }
}

impl PhaseTracker {
    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn has_flown(&self) -> bool {
        self.flown
    }

    pub fn step(&mut self, s: &Snapshot) -> Phase {
        self.phase = self.next(s);
        self.phase
    }

    fn next(&mut self, s: &Snapshot) -> Phase {
        use Phase::*;
        let p = self.phase;
        if s.on_ground {
            if self.flown {
                return match p {
                    Climb | Cruise | Descent | Approach | GoAround => Rollout,
                    Rollout if s.gs_kt < 30. && !s.reversers_deployed => TaxiIn,
                    TaxiIn if s.engines_running == 0 => {
                        self.flown = false;
                        Parked
                    }
                    other => other,
                };
            }
            if s.engines_running == 0 {
                return if p == Parked { Parked } else { Preflight };
            }
            if s.tla_max >= FLX_TLA && s.gs_kt > 3. {
                return TakeoffRoll;
            }
            if p == TakeoffRoll && s.gs_kt > 30. {
                return TakeoffRoll;
            }
            return TaxiOut;
        }
        if s.ra_ft > 50. || p == Climb {
            self.flown = true;
        }
        if !self.flown {
            return p;
        }
        match s.fmgc_phase {
            1 | 2 => Climb,
            3 => Cruise,
            4 => Descent,
            5 => Approach,
            6 => GoAround,
            _ => match p {
                TakeoffRoll | TaxiOut | Preflight => Climb,
                Climb if s.vs_fpm < -500. => Descent,
                Cruise | Climb | Descent if s.ra_ft > 0. && s.ra_ft < 2500. && s.vs_fpm < 0. => Approach,
                other => other,
            },
        }
    }
}

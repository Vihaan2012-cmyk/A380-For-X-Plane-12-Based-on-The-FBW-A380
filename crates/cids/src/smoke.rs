use crate::types::{Caution, Level, SMOKE_CARGO, SMOKE_LOCS, SMOKE_NAMES};

#[derive(Clone, Debug, Default)]
pub struct SmokeOut {
    pub active: [bool; SMOKE_LOCS],
    pub any: bool,
    pub cabin_any: bool,
    pub cargo_any: bool,
    pub horn: bool,
    pub new_cabin_event: bool,
    pub cautions: Vec<Caution>,
}

pub struct Smoke {
    prev: [bool; SMOKE_LOCS],
    silenced: bool,
}

impl Smoke {
    pub fn new() -> Self {
        Self { prev: [false; SMOKE_LOCS], silenced: false }
    }

    pub fn step(&mut self, input: &[bool; SMOKE_LOCS], reset: bool, cids_ok: bool) -> SmokeOut {
        let mut out = SmokeOut { active: *input, ..Default::default() };
        let mut rising = false;
        let mut rising_cabin = false;
        for i in 0..SMOKE_LOCS {
            let cargo = SMOKE_CARGO.contains(&i);
            if input[i] {
                out.any = true;
                if cargo {
                    out.cargo_any = true;
                } else {
                    out.cabin_any = true;
                }
                out.cautions.push(Caution::new(Level::Red, format!("SMOKE {}", SMOKE_NAMES[i])));
            }
            if input[i] && !self.prev[i] {
                rising = true;
                rising_cabin |= !cargo;
            }
        }
        self.prev = *input;
        if rising {
            self.silenced = false;
        }
        if reset {
            self.silenced = true;
        }
        if !out.any {
            self.silenced = false;
        }
        out.horn = out.any && !self.silenced && cids_ok;
        out.new_cabin_event = rising_cabin;
        out
    }
}

impl Default for Smoke {
    fn default() -> Self {
        Self::new()
    }
}

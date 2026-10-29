use crate::types::{Chime, ChimeKind, Config, Phase, Target, AREAS};

#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub label: &'static str,
    pub ok: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ReadyOut {
    pub ready: bool,
    pub areas: [bool; AREAS],
    pub checks: Vec<Check>,
    pub incomplete: bool,
    pub refused: bool,
    pub chime: Option<Chime>,
}

pub struct ReadyInputs<'a> {
    pub phase: Phase,
    pub pax_closed: bool,
    pub slides_armed: bool,
    pub seatbelt_sign: bool,
    pub area_toggle: Option<usize>,
    pub press: bool,
    pub cfg: &'a Config,
}

pub struct CabinReady {
    ready: bool,
    areas: [bool; AREAS],
    group: u8,
    prev_pax_closed: bool,
}

impl CabinReady {
    pub fn new() -> Self {
        Self { ready: false, areas: [false; AREAS], group: 0, prev_pax_closed: true }
    }

    pub fn step(&mut self, i: &ReadyInputs) -> ReadyOut {
        let group = i.phase.ready_group();
        if group != self.group {
            self.group = group;
            self.ready = false;
            self.areas = [false; AREAS];
        }
        if self.prev_pax_closed && !i.pax_closed {
            self.ready = false;
            self.areas = [false; AREAS];
        }
        self.prev_pax_closed = i.pax_closed;
        if let Some(a) = i.area_toggle {
            if a < AREAS {
                self.areas[a] = !self.areas[a];
            }
        }
        let landing = i.phase.ready_group() == 2;
        let mut checks = vec![Check { label: "ALL AREAS READY", ok: self.areas.iter().all(|a| *a) }];
        if !landing {
            checks.push(Check { label: "DOORS CLOSED", ok: i.pax_closed });
            checks.push(Check { label: "SLIDES ARMED", ok: i.slides_armed });
        }
        checks.push(Check { label: "SEAT BELT SIGN ON", ok: i.seatbelt_sign });
        let complete = checks.iter().all(|c| c.ok);
        let mut out = ReadyOut { areas: self.areas, ..Default::default() };
        if i.press {
            if complete || !i.cfg.strict_cabin_ready {
                if !self.ready {
                    out.chime = Some(Chime { target: Target::Cockpit, kind: ChimeKind::CabinReady });
                }
                self.ready = true;
                out.incomplete = !complete;
            } else {
                out.refused = true;
            }
        }
        out.ready = self.ready;
        out.checks = checks;
        out
    }
}

impl Default for CabinReady {
    fn default() -> Self {
        Self::new()
    }
}

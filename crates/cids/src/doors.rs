use crate::types::{Caution, Chime, ChimeKind, DoorIn, Flight, Level, Phase, Target, DOORS, PAX_DOORS};

pub const RESIDUAL_PSI: f64 = 0.8;
pub const CLOSED_BELOW: f64 = 0.02;
pub const OPEN_ABOVE: f64 = 0.98;
const WARN_HOLD_S: f64 = 10.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorState {
    Closed,
    Moving,
    Open,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlideState {
    Armed,
    Disarmed,
    NotFitted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorRefusal {
    CidsFault,
    NotStationary,
    EngineRunning,
    Pressurised,
    SlideArmed,
    BadDoor,
}

impl DoorRefusal {
    pub fn text(self) -> &'static str {
        match self {
            DoorRefusal::CidsFault => "DOOR OPERATION UNAVAILABLE: CIDS FAULT",
            DoorRefusal::NotStationary => "DOOR OPERATION INHIBITED: AIRCRAFT MOVING",
            DoorRefusal::EngineRunning => "DOOR OPERATION INHIBITED: ENGINE RUNNING",
            DoorRefusal::Pressurised => "DOOR OPERATION INHIBITED: CABIN PRESSURISED",
            DoorRefusal::SlideArmed => "DISARM SLIDE BEFORE OPENING DOOR",
            DoorRefusal::BadDoor => "NO SUCH DOOR",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DoorView {
    pub name: &'static str,
    pub state: DoorState,
    pub slide: SlideState,
    pub fault: bool,
}

#[derive(Clone, Debug, Default)]
pub struct DoorsOut {
    pub toggle: Option<usize>,
    pub refused: Option<DoorRefusal>,
    pub views: Vec<DoorView>,
    pub cautions: Vec<Caution>,
    pub chime: Option<Chime>,
    pub pax_closed: bool,
    pub slides_armed: usize,
    pub open_while_armed: Option<usize>,
}

pub fn state_of(open: f64) -> DoorState {
    if open <= CLOSED_BELOW {
        DoorState::Closed
    } else if open >= OPEN_ABOVE {
        DoorState::Open
    } else {
        DoorState::Moving
    }
}

pub fn check_command(index: usize, doors: &[DoorIn; 18], f: &Flight, cids_ok: bool) -> Result<(), DoorRefusal> {
    if index >= DOORS.len() {
        return Err(DoorRefusal::BadDoor);
    }
    if !cids_ok {
        return Err(DoorRefusal::CidsFault);
    }
    if !f.stationary() {
        return Err(DoorRefusal::NotStationary);
    }
    if !f.engines_off() {
        return Err(DoorRefusal::EngineRunning);
    }
    let closed = state_of(doors[index].open) == DoorState::Closed;
    if index < PAX_DOORS && closed && f.cabin_dp_psi >= RESIDUAL_PSI {
        return Err(DoorRefusal::Pressurised);
    }
    if index < PAX_DOORS && closed && doors[index].slide_armed {
        return Err(DoorRefusal::SlideArmed);
    }
    Ok(())
}

pub struct DoorsSlides {
    prev_closed: [bool; 18],
    warn_s: f64,
    warn_door: usize,
}

impl DoorsSlides {
    pub fn new() -> Self {
        Self { prev_closed: [true; 18], warn_s: 0., warn_door: 0 }
    }

    pub fn step(&mut self, doors: &[DoorIn; 18], f: &Flight, phase: Phase, command: Option<usize>, cids_ok: bool, dt: f64) -> DoorsOut {
        let mut out = DoorsOut::default();
        if let Some(i) = command {
            match check_command(i, doors, f, cids_ok) {
                Ok(()) => out.toggle = Some(i),
                Err(r) => out.refused = Some(r),
            }
        }
        self.warn_s = (self.warn_s - dt).max(0.);
        for (i, d) in doors.iter().enumerate() {
            let st = state_of(d.open);
            let was_closed = self.prev_closed[i];
            if i < PAX_DOORS && was_closed && st != DoorState::Closed && d.slide_armed && f.on_ground {
                out.open_while_armed = Some(i);
                self.warn_s = WARN_HOLD_S;
                self.warn_door = i;
                out.chime = Some(Chime { target: Target::Cabin, kind: ChimeKind::DoorWarning });
            }
            self.prev_closed[i] = st == DoorState::Closed;
            let pax = i < PAX_DOORS;
            out.views.push(DoorView {
                name: DOORS[i],
                state: st,
                slide: if !pax {
                    SlideState::NotFitted
                } else if d.slide_armed {
                    SlideState::Armed
                } else {
                    SlideState::Disarmed
                },
                fault: d.latch_fault || (pax && d.slide_press_low),
            });
        }
        out.pax_closed = doors[..PAX_DOORS].iter().all(|d| state_of(d.open) == DoorState::Closed);
        out.slides_armed = doors[..PAX_DOORS].iter().filter(|d| d.slide_armed).count();

        let moving_out = phase == Phase::TakeoffRoll || (phase == Phase::TaxiOut && f.gs_kt > 2.);
        let must_be_closed = moving_out || phase.airborne() || phase == Phase::Rollout;
        if must_be_closed {
            for (i, d) in doors.iter().enumerate() {
                if state_of(d.open) != DoorState::Closed {
                    out.cautions.push(Caution::new(Level::Red, format!("DOOR {} OPEN", DOORS[i].replace('_', " "))));
                }
            }
        }
        let slides_needed = moving_out || matches!(phase, Phase::Approach | Phase::Rollout);
        if slides_needed && out.pax_closed && out.slides_armed < PAX_DOORS {
            out.cautions.push(Caution::new(Level::Amber, format!("SLIDES NOT ARMED {}", PAX_DOORS - out.slides_armed)));
        }
        if self.warn_s > 0. {
            out.cautions.push(Caution::new(Level::Red, format!("DOOR {} OPENED, SLIDE ARMED", DOORS[self.warn_door])));
        }
        for (i, d) in doors.iter().enumerate() {
            if d.latch_fault {
                out.cautions.push(Caution::new(Level::Amber, format!("DOOR {} LATCH", DOORS[i].replace('_', " "))));
            }
            if i < PAX_DOORS && d.slide_press_low {
                out.cautions.push(Caution::new(Level::Amber, format!("SLIDE {} PRESS LO", DOORS[i])));
            }
        }
        out
    }
}

impl Default for DoorsSlides {
    fn default() -> Self {
        Self::new()
    }
}

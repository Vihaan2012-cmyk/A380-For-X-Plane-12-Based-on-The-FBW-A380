use crate::types::{Chime, ChimeKind, Cockpit, Config, Crew, Target, ZONES};

pub const FWD: usize = 0;
pub const AFT: usize = 2;
pub const PURSER: usize = 3;
pub const LOG_LEN: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallKind {
    Attendant(usize),
    Purser,
    Emergency,
    Pilot(usize),
    Mech,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CallRecord {
    pub kind: CallKind,
    pub age_s: f64,
    pub answered: bool,
}

#[derive(Clone, Debug, Default)]
pub struct CallsOut {
    pub chimes: Vec<Chime>,
    pub cabin_call_lights: [bool; 4],
    pub cockpit_att_light: bool,
    pub cockpit_emer_light: bool,
    pub cockpit_mech_light: bool,
    pub mech_horn: bool,
    pub emer_to_cabin: bool,
    pub cabin_emer_active: bool,
    pub pending: usize,
}

pub struct Calls {
    prev_cockpit: [bool; 5],
    prev_att: [bool; ZONES],
    prev_purs: bool,
    prev_cab_emer: bool,
    prev_pilot_emer: bool,
    prev_ground: bool,
    lit_s: [f64; 4],
    att: [bool; ZONES],
    purs: bool,
    cab_emer: bool,
    mech: f64,
    emer_clock_cab: f64,
    emer_clock_pilot: f64,
    pub log: Vec<CallRecord>,
}

impl Calls {
    pub fn new() -> Self {
        Self {
            prev_cockpit: [false; 5],
            prev_att: [false; ZONES],
            prev_purs: false,
            prev_cab_emer: false,
            prev_pilot_emer: false,
            prev_ground: false,
            lit_s: [0.; 4],
            att: [false; ZONES],
            purs: false,
            cab_emer: false,
            mech: 0.,
            emer_clock_cab: 0.,
            emer_clock_pilot: 0.,
            log: Vec::new(),
        }
    }

    fn record(&mut self, kind: CallKind) {
        self.log.insert(0, CallRecord { kind, age_s: 0., answered: false });
        self.log.truncate(LOG_LEN);
    }

    pub fn reset(&mut self) {
        self.att = [false; ZONES];
        self.purs = false;
        self.cab_emer = false;
        self.lit_s = [0.; 4];
        for r in &mut self.log {
            r.answered = true;
        }
    }

    pub fn step(
        &mut self,
        cockpit: &Cockpit,
        crew: &Crew,
        on_ground: bool,
        stationary: bool,
        auto_emergency: bool,
        call_reset: bool,
        link_ok: bool,
        cfg: &Config,
        dt: f64,
    ) -> CallsOut {
        let mut out = CallsOut::default();
        for r in &mut self.log {
            r.age_s += dt;
        }
        if call_reset {
            self.reset();
            self.mech = 0.;
        }

        let pb = [cockpit.calls_all, cockpit.calls_fwd, cockpit.calls_aft, cockpit.calls_purs, cockpit.calls_mech];
        for (i, held) in pb.iter().enumerate() {
            let edge = *held && !self.prev_cockpit[i];
            self.prev_cockpit[i] = *held;
            if i == 4 {
                if edge {
                    self.mech = 0.;
                }
                continue;
            }
            let targets: &[usize] = match i {
                0 => &[0, 1, 2, 3],
                1 => &[FWD],
                2 => &[AFT],
                _ => &[PURSER],
            };
            for t in targets {
                if *held {
                    self.lit_s[*t] = cfg.call_light_s;
                }
            }
            if edge && link_ok {
                out.chimes.push(Chime { target: Target::Cabin, kind: ChimeKind::PilotCall });
                self.record(CallKind::Pilot(i));
            }
        }

        for z in 0..ZONES {
            let edge = crew.attendant_call[z] && !self.prev_att[z];
            self.prev_att[z] = crew.attendant_call[z];
            if edge && link_ok {
                self.att[z] = true;
                out.chimes.push(Chime { target: Target::Cockpit, kind: ChimeKind::CabinCall });
                self.record(CallKind::Attendant(z));
            }
        }
        let edge = crew.purser_call && !self.prev_purs;
        self.prev_purs = crew.purser_call;
        if edge && link_ok {
            self.purs = true;
            out.chimes.push(Chime { target: Target::Cockpit, kind: ChimeKind::PurserCall });
            self.record(CallKind::Purser);
        }

        let cab_emer_in = crew.cabin_emer_call || auto_emergency;
        let edge = cab_emer_in && !self.prev_cab_emer;
        self.prev_cab_emer = cab_emer_in;
        if edge {
            self.cab_emer = true;
            self.emer_clock_cab = 0.;
            out.chimes.push(Chime { target: Target::Cockpit, kind: ChimeKind::EmergencyCall });
            self.record(CallKind::Emergency);
        } else if self.cab_emer {
            self.emer_clock_cab += dt;
            if self.emer_clock_cab >= cfg.emer_repeat_s {
                self.emer_clock_cab = 0.;
                out.chimes.push(Chime { target: Target::Cockpit, kind: ChimeKind::EmergencyCall });
            }
        }

        let edge = cockpit.emer_call_on && !self.prev_pilot_emer;
        self.prev_pilot_emer = cockpit.emer_call_on;
        if edge {
            self.emer_clock_pilot = 0.;
            out.chimes.push(Chime { target: Target::Cabin, kind: ChimeKind::EmergencyCall });
            self.record(CallKind::Emergency);
        } else if cockpit.emer_call_on {
            self.emer_clock_pilot += dt;
            if self.emer_clock_pilot >= cfg.emer_repeat_s {
                self.emer_clock_pilot = 0.;
                out.chimes.push(Chime { target: Target::Cabin, kind: ChimeKind::EmergencyCall });
            }
        }

        let edge = crew.ground_mech_call && !self.prev_ground;
        self.prev_ground = crew.ground_mech_call;
        if edge && on_ground {
            self.mech = cfg.call_light_s;
            out.chimes.push(Chime { target: Target::Cockpit, kind: ChimeKind::MechCall });
            self.record(CallKind::Mech);
        }
        self.mech = (self.mech - dt).max(0.);

        for t in &mut self.lit_s {
            *t = (*t - dt).max(0.);
        }
        out.cabin_call_lights = [self.lit_s[0] > 0., self.lit_s[1] > 0., self.lit_s[2] > 0., self.lit_s[3] > 0.];
        out.cockpit_att_light = self.att.iter().any(|a| *a) || self.purs;
        out.cockpit_emer_light = self.cab_emer;
        out.cabin_emer_active = self.cab_emer;
        out.cockpit_mech_light = self.mech > 0.;
        out.mech_horn = cockpit.calls_mech && on_ground && stationary;
        out.emer_to_cabin = cockpit.emer_call_on;
        out.pending = self.att.iter().filter(|a| **a).count() + self.purs as usize + self.cab_emer as usize;
        out
    }
}

impl Default for Calls {
    fn default() -> Self {
        Self::new()
    }
}

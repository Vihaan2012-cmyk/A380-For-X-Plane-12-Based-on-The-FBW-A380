use crate::types::{Cockpit, Config};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvacRefusal {
    CaptOnly,
    CidsFault,
    CockpitCommandOn,
}

#[derive(Clone, Debug, Default)]
pub struct EvacOut {
    pub active: bool,
    pub horn_cockpit: bool,
    pub horn_cabin: bool,
    pub fap_flash: bool,
    pub cockpit_pb_flash: bool,
    pub guard_open: bool,
    pub from_cabin: bool,
    pub refused: Option<EvacRefusal>,
}

pub struct Evac {
    fap_cmd: bool,
    guard_s: f64,
    silenced: bool,
}

impl Evac {
    pub fn new() -> Self {
        Self { fap_cmd: false, guard_s: 0., silenced: false }
    }

    pub fn step(&mut self, cockpit: &Cockpit, cids_ok: bool, press_cmd: bool, press_reset: bool, cfg: &Config, dt: f64) -> EvacOut {
        let mut refused = None;
        self.guard_s = (self.guard_s - dt).max(0.);
        if press_cmd {
            if cockpit.evac_capt_only {
                refused = Some(EvacRefusal::CaptOnly);
            } else if !cids_ok {
                refused = Some(EvacRefusal::CidsFault);
            } else if self.guard_s > 0. {
                self.fap_cmd = true;
                self.guard_s = 0.;
            } else {
                self.guard_s = cfg.evac_guard_s;
            }
        }
        if press_reset {
            if cockpit.evac_cmd {
                refused = Some(EvacRefusal::CockpitCommandOn);
            } else {
                self.fap_cmd = false;
            }
        }
        let active = cockpit.evac_cmd || self.fap_cmd;
        if !active {
            self.silenced = false;
        } else if cockpit.evac_horn_shutoff {
            self.silenced = true;
        }
        EvacOut {
            active,
            horn_cockpit: active && !self.silenced,
            horn_cabin: active,
            fap_flash: active,
            cockpit_pb_flash: self.fap_cmd && !cockpit.evac_cmd,
            guard_open: self.guard_s > 0.,
            from_cabin: self.fap_cmd,
            refused,
        }
    }
}

impl Default for Evac {
    fn default() -> Self {
        Self::new()
    }
}

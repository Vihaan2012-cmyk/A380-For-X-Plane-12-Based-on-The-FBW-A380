use crate::types::{Chime, ChimeKind, Cockpit, EmerSw, Flight, Power, Sw3, Target};

pub const EXCESS_CABIN_ALT_FT: f64 = 11_300.;

#[derive(Clone, Debug, Default)]
pub struct SignsOut {
    pub seatbelt: bool,
    pub no_smoking: bool,
    pub exit_signs: bool,
    pub emer_lights: bool,
    pub cabin_alt_forced: bool,
    pub chime: Option<Chime>,
}

pub fn auto_seatbelt(f: &Flight) -> bool {
    f.n1_pct.iter().any(|n| *n > 10.) && (f.slats_deg >= 19. || f.gear_down)
}

pub struct Signs {
    prev_seatbelt: Option<bool>,
    prev_no_smoking: Option<bool>,
}

impl Signs {
    pub fn new() -> Self {
        Self { prev_seatbelt: None, prev_no_smoking: None }
    }

    pub fn step(&mut self, cockpit: &Cockpit, flight: &Flight, power: &Power, fap_emer_light: bool, evac_active: bool, cids_ok: bool) -> SignsOut {
        let forced = flight.cabin_alt_ft > EXCESS_CABIN_ALT_FT;
        let seatbelt = forced
            || match cockpit.seatbelt {
                Sw3::On => true,
                Sw3::Off => false,
                Sw3::Auto => auto_seatbelt(flight),
            };
        let no_smoking = forced
            || match cockpit.no_smoking {
                Sw3::On => true,
                Sw3::Off => false,
                Sw3::Auto => flight.gear_down,
            };
        let power_loss = !power.ac_cabin;
        let emer_lights = match cockpit.emer_exit {
            EmerSw::On => true,
            EmerSw::Off => false,
            EmerSw::Armed => power_loss || evac_active || (fap_emer_light && cids_ok),
        };
        let mut chime = None;
        let changed = self.prev_seatbelt.is_some_and(|p| p != seatbelt) || self.prev_no_smoking.is_some_and(|p| p != no_smoking);
        if changed && power.ac_cabin {
            chime = Some(Chime { target: Target::Cabin, kind: ChimeKind::Sign });
        }
        self.prev_seatbelt = Some(seatbelt);
        self.prev_no_smoking = Some(no_smoking);
        SignsOut {
            seatbelt: seatbelt && power.ac_cabin,
            no_smoking: no_smoking && power.ac_cabin,
            exit_signs: power.ac_cabin || emer_lights,
            emer_lights,
            cabin_alt_forced: forced,
            chime,
        }
    }
}

impl Default for Signs {
    fn default() -> Self {
        Self::new()
    }
}

pub mod calls;
pub mod directors;
pub mod doors;
pub mod evac;
pub mod fap;
pub mod maint;
pub mod pa;
pub mod phase;
pub mod ready;
pub mod signs;
pub mod smoke;
pub mod temp;
pub mod types;
pub mod view;

pub use types::*;

use calls::{Calls, CallsOut};
use directors::Directors;
use doors::{DoorRefusal, DoorsOut, DoorsSlides};
use evac::{Evac, EvacOut, EvacRefusal};
use fap::FapUnit;
use maint::{MaintOut, Maintenance, TestResult};
use pa::{Pa, PaInputs, PaOut};
use phase::PhaseTracker;
use ready::{CabinReady, ReadyInputs, ReadyOut};
use signs::{Signs, SignsOut};
use smoke::{Smoke, SmokeOut};
use temp::{Temp, TempOut};

#[derive(Clone, Debug, Default)]
pub struct EcamFlags {
    pub evac: bool,
    pub cabin_emer_call: bool,
    pub cabin_smoke: bool,
    pub cargo_smoke: bool,
    pub slides_not_armed: bool,
    pub door_open_in_departure: bool,
    pub cids_fault: bool,
    pub cids_degraded: bool,
    pub pa_fault: bool,
    pub excess_cabin_alt_signs: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Outputs {
    pub phase: Phase,
    pub cids_ok: bool,
    pub active_director: Option<usize>,
    pub fap_live: bool,
    pub cabin_ready: bool,
    pub cabin_ready_incomplete: bool,
    pub seatbelt_sign: bool,
    pub no_smoking_sign: bool,
    pub exit_signs: bool,
    pub emer_lights: bool,
    pub full_bright: bool,
    pub cabin_scene: Option<u8>,
    pub cabin_level: Option<f64>,
    pub chimes: Vec<Chime>,
    pub calls: CallsOut,
    pub evac: EvacOut,
    pub pa: PaOut,
    pub door_toggle: Option<usize>,
    pub door_refused: Option<DoorRefusal>,
    pub smoke_horn: bool,
    pub seat_power: bool,
    pub pax_sys: bool,
    pub water_heaters: bool,
    pub galley_on: [bool; ZONES],
    pub temp: TempOut,
    pub lamp_test: bool,
    pub cautions: Vec<Caution>,
    pub ecam: EcamFlags,
}

pub struct Cids {
    pub cfg: Config,
    pub(crate) phase: PhaseTracker,
    pub(crate) directors: Directors,
    pub(crate) calls: Calls,
    pub(crate) evac: Evac,
    pub(crate) signs: Signs,
    pub(crate) pa: Pa,
    pub(crate) doors: DoorsSlides,
    pub(crate) smoke: Smoke,
    pub(crate) ready: CabinReady,
    pub(crate) temp: Temp,
    pub(crate) maint: Maintenance,
    pub(crate) fap: FapUnit,
    pub(crate) last: Outputs,
    pub(crate) last_in: Inputs,
    pub(crate) doors_out: DoorsOut,
    pub(crate) smoke_out: SmokeOut,
    pub(crate) ready_out: ReadyOut,
    pub(crate) maint_out: Option<MaintOut>,
    pub(crate) signs_out: SignsOut,
}

#[derive(Default)]
struct Requests {
    door: Option<usize>,
    announce: Option<u8>,
    music: Option<bool>,
    track: Option<u8>,
    volume: Option<u8>,
    stop_audio: bool,
    call_reset: bool,
    ready_press: bool,
    area: Option<usize>,
    evac_cmd: bool,
    evac_reset: bool,
    smoke_reset: bool,
    test: Option<TestKind>,
}

impl Cids {
    pub fn new(cfg: Config) -> Self {
        Self {
            cfg,
            phase: PhaseTracker::new(),
            directors: Directors::new(),
            calls: Calls::new(),
            evac: Evac::new(),
            signs: Signs::new(),
            pa: Pa::new(),
            doors: DoorsSlides::new(),
            smoke: Smoke::new(),
            ready: CabinReady::new(),
            temp: Temp::new(),
            maint: Maintenance::new(),
            fap: FapUnit::new(),
            last: Outputs::default(),
            last_in: Inputs::default(),
            doors_out: DoorsOut::default(),
            smoke_out: SmokeOut::default(),
            ready_out: ReadyOut::default(),
            maint_out: None,
            signs_out: SignsOut::default(),
        }
    }

    pub fn outputs(&self) -> &Outputs {
        &self.last
    }

    pub fn update(&mut self, i: &Inputs, dt: f64) -> &Outputs {
        let dt = if dt.is_finite() { dt.clamp(0., 0.5) } else { 0. };
        let cfg = self.cfg.clone();
        let phase = self.phase.step(&i.flight);
        self.directors.step(&i.power, &i.cabin, dt);
        let cids_ok = self.directors.operating();
        let powered = i.power.ac_cabin || i.power.dc_ess;
        self.fap.tick(powered, &cfg, dt);
        let live = self.fap.live(powered);

        let mut rq = Requests::default();
        for ev in &i.crew.events {
            if !live || !self.fap.accepts(ev) {
                continue;
            }
            self.fap.navigate(ev);
            let needs_cids = !matches!(
                ev,
                Event::SelectPage(_) | Event::NextPage | Event::PrevPage | Event::ScreenOff | Event::ScreenOn | Event::PcReset | Event::LockScreen | Event::LavMaint | Event::EvacCmd | Event::EvacReset
            );
            if needs_cids && !cids_ok {
                self.fap.say("CIDS FAULT: FUNCTION UNAVAILABLE", &cfg);
                continue;
            }
            match ev {
                Event::ScreenOff => self.fap.off = true,
                Event::ScreenOn => self.fap.off = false,
                Event::PcReset => {
                    self.fap.boot_s = cfg.boot_s;
                    self.fap.page = Page::Status;
                    self.fap.off = false;
                }
                Event::LockScreen => self.fap.lock_s = cfg.lock_s,
                Event::LavMaint => self.fap.lav_maint = !self.fap.lav_maint,
                Event::Door(n) => rq.door = Some(*n),
                Event::Announce(n) => match n {
                    0 => rq.stop_audio = true,
                    1 => rq.music = Some(true),
                    n => rq.announce = Some(*n),
                },
                Event::Music(on) => rq.music = Some(*on),
                Event::MusicTrack(t) => rq.track = Some(*t),
                Event::MusicVolume(v) => rq.volume = Some(*v),
                Event::StopAudio => rq.stop_audio = true,
                Event::Scene(_) | Event::Level(_) | Event::LightsMain => self.fap.apply_light_event(ev),
                Event::CallReset => rq.call_reset = true,
                Event::CabinReady => rq.ready_press = true,
                Event::AreaReady(a) => rq.area = Some(*a),
                Event::EvacCmd => rq.evac_cmd = true,
                Event::EvacReset => rq.evac_reset = true,
                Event::SmokeReset => rq.smoke_reset = true,
                Event::EmerLight(on) => self.fap.emer_light = *on,
                Event::PaxSys(on) => self.fap.pax_sys_off = !*on,
                Event::SeatPower(on) => self.fap.seat_power_off = !*on,
                Event::WaterHeaters(on) => self.fap.heaters_off = !*on,
                Event::Galley(z, on) => {
                    if *z < ZONES {
                        self.fap.galley_off[*z] = !*on;
                    }
                }
                Event::TempSet(a, c) => self.temp.set(*a, *c),
                Event::TempStep(a, d) => self.temp.step_by(*a, *d),
                Event::Test(k) => rq.test = Some(*k),
                Event::SelectPage(_) | Event::NextPage | Event::PrevPage => {}
            }
        }

        let smoke = self.smoke.step(&i.cabin.smoke, rq.smoke_reset, cids_ok);
        let link_ok = cids_ok && i.cabin.channel_fault[3] < 1.;
        let calls = self.calls.step(
            &i.cockpit,
            &i.crew,
            i.flight.on_ground,
            i.flight.stationary(),
            smoke.new_cabin_event,
            rq.call_reset,
            link_ok,
            &cfg,
            dt,
        );
        let evac = self.evac.step(&i.cockpit, cids_ok, rq.evac_cmd, rq.evac_reset, &cfg, dt);
        if let Some(r) = evac.refused {
            let text = match r {
                EvacRefusal::CaptOnly => "EVAC SELECTED TO CAPT ONLY",
                EvacRefusal::CidsFault => "CIDS FAULT: USE COCKPIT EVAC",
                EvacRefusal::CockpitCommandOn => "EVAC COMMAND ON IN COCKPIT",
            };
            self.fap.say(text, &cfg);
        } else if rq.evac_cmd && evac.guard_open {
            self.fap.say("PRESS EVAC CMD AGAIN TO CONFIRM", &cfg);
        }

        let doors = self.doors.step(&i.doors, &i.flight, phase, rq.door, cids_ok, dt);
        if let Some(r) = doors.refused {
            self.fap.say(r.text(), &cfg);
        }

        let signs = self.signs.step(&i.cockpit, &i.flight, &i.power, self.fap.emer_light, evac.active, cids_ok);

        let ready = self.ready.step(&ReadyInputs {
            phase,
            pax_closed: doors.pax_closed,
            slides_armed: doors.slides_armed == PAX_DOORS,
            seatbelt_sign: signs.seatbelt,
            area_toggle: rq.area,
            press: rq.ready_press,
            cfg: &cfg,
        });
        if ready.refused {
            self.fap.say("CABIN READY REFUSED: CHECK ITEMS", &cfg);
        }

        let deck_ok: [bool; 3] = std::array::from_fn(|d| cids_ok && i.power.ac_cabin && i.cabin.channel_fault[d] < 1.);
        let degraded = self.directors.degraded() || i.cabin.channel_fault[..3].iter().any(|c| *c > 0.);
        let boarding = phase == Phase::Parked && i.flight.engines_off() && !doors.pax_closed;
        let pa = self.pa.step(
            &PaInputs {
                announce: rq.announce,
                music: rq.music,
                track: rq.track,
                volume: rq.volume,
                stop_all: rq.stop_audio,
                auto_announcement: if cids_ok { i.flight.auto_announcement } else { None },
                handset: i.crew.handset_pa && cids_ok,
                cockpit: i.crew.cockpit_pa && cids_ok,
                evac_active: evac.active,
                boarding,
                deck_ok,
                degraded,
            },
            &cfg,
            dt,
        );

        let temp = self.temp.step(i.cabin.purser_temp_sel_fault);

        let test_allowed = phase == Phase::Parked && i.flight.parking_brake && i.flight.engines_off() && cids_ok;
        let maint = self.maint.step(rq.test, test_allowed, self.directors.healthy, i.cabin.channel_fault, dt);

        let mut chimes: Vec<Chime> = calls.chimes.clone();
        chimes.extend(signs.chime);
        chimes.extend(ready.chime);
        chimes.extend(doors.chime);
        chimes.extend(maint.chime);

        let mut cautions: Vec<Caution> = Vec::new();
        if evac.active {
            cautions.push(Caution::new(Level::Red, "EVAC ACTIVE"));
        }
        cautions.extend(smoke.cautions.iter().cloned());
        cautions.extend(doors.cautions.iter().cloned());
        if calls.cockpit_emer_light {
            cautions.push(Caution::new(Level::Red, "EMER CALL"));
        }
        if !cids_ok {
            cautions.push(Caution::new(Level::Amber, "CIDS FAULT"));
        } else if degraded {
            cautions.push(Caution::new(Level::Amber, "CIDS DEGRADED"));
        }
        for (d, name) in ["UPPER", "MAIN", "LOWER"].iter().enumerate() {
            if i.cabin.channel_fault[d] >= 1. {
                cautions.push(Caution::new(Level::Amber, format!("PA {name} FAULT")));
            }
        }
        if signs.cabin_alt_forced {
            cautions.push(Caution::new(Level::Amber, "CABIN ALT SIGNS ON"));
        }
        for z in 0..ZONES {
            if i.cabin.waste_full[z] {
                cautions.push(Caution::new(Level::Amber, format!("WASTE {} FULL", ["FWD", "MID", "AFT"][z])));
            }
        }
        if i.cabin.water_fault {
            cautions.push(Caution::new(Level::Amber, "WATER SYSTEM FAULT"));
        }
        if i.cabin.purser_temp_sel_fault {
            cautions.push(Caution::new(Level::Amber, "TEMP SEL FAULT"));
        }
        if calls.pending > 0 && !calls.cockpit_emer_light {
            cautions.push(Caution::new(Level::Cyan, format!("CALL {}", calls.pending)));
        }
        cautions.sort_by_key(|c| std::cmp::Reverse(c.level));

        let slides_not_armed = doors.cautions.iter().any(|c| c.text.starts_with("SLIDES NOT ARMED"));
        let door_open_dep = doors.cautions.iter().any(|c| c.level == Level::Red && c.text.ends_with("OPEN"));
        let full_bright = evac.active;
        let out = Outputs {
            phase,
            cids_ok,
            active_director: self.directors.active,
            fap_live: live,
            cabin_ready: ready.ready,
            cabin_ready_incomplete: ready.incomplete,
            seatbelt_sign: signs.seatbelt,
            no_smoking_sign: signs.no_smoking,
            exit_signs: signs.exit_signs,
            emer_lights: signs.emer_lights,
            full_bright,
            cabin_scene: if cids_ok { self.fap.scene } else { None },
            cabin_level: if full_bright { Some(1.) } else if cids_ok { self.fap.level_pct.map(|p| p as f64 / 100.) } else { None },
            chimes,
            calls: calls.clone(),
            evac: evac.clone(),
            pa,
            door_toggle: doors.toggle,
            door_refused: doors.refused,
            smoke_horn: smoke.horn,
            seat_power: !(self.fap.seat_power_off || self.fap.pax_sys_off) || !cids_ok,
            pax_sys: !self.fap.pax_sys_off || !cids_ok,
            water_heaters: !self.fap.heaters_off || !cids_ok,
            galley_on: std::array::from_fn(|z| !self.fap.galley_off[z] || !cids_ok),
            temp,
            lamp_test: maint.lamp_test,
            cautions,
            ecam: EcamFlags {
                evac: evac.active,
                cabin_emer_call: calls.cockpit_emer_light,
                cabin_smoke: smoke.cabin_any,
                cargo_smoke: smoke.cargo_any,
                slides_not_armed,
                door_open_in_departure: door_open_dep,
                cids_fault: !cids_ok,
                cids_degraded: cids_ok && degraded,
                pa_fault: i.cabin.channel_fault[..3].iter().any(|c| *c >= 1.),
                excess_cabin_alt_signs: signs.cabin_alt_forced,
            },
        };
        self.last = out;
        self.last_in = Inputs { crew: Crew { events: Vec::new(), ..i.crew.clone() }, ..i.clone() };
        self.doors_out = doors;
        self.smoke_out = smoke;
        self.ready_out = ready;
        self.signs_out = signs;
        self.maint_out = Some(maint);
        &self.last
    }

    pub fn test_result(&self) -> TestResult {
        self.maint_out.as_ref().map_or(TestResult::None, |m| m.result.clone())
    }
}

impl Default for Cids {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

#[cfg(test)]
mod tests;

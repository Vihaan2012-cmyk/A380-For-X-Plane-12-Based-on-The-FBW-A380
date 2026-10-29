use crate::ecam::EcamRunner;
use crate::phase::FLX_TLA;
use crate::radio::{self, StationKind};
use crate::*;

const REVERSE_TLA: f64 = -1.0;
const FLAP_SETTLE_S: f64 = 1.0;
const AFTER_START_DELAY_S: f64 = 8.0;
const AFTER_START_CHECKLIST_S: f64 = 25.0;
const AUTO_GEAR_UP_S: f64 = 1.2;
const APPROACH_DATA_NM: f64 = 200.0;
const TEN_THOUSAND_FT: f64 = 10_000.0;
const DEV_SPEED_LOW_KT: f64 = 5.0;
const DEV_SPEED_HIGH_KT: f64 = 10.0;
const DEV_SINK_FPM: f64 = -1000.0;
const DEV_BANK_DEG: f64 = 7.0;
const DEV_PITCH_UP_DEG: f64 = 10.0;
const DEV_PITCH_DOWN_DEG: f64 = -2.5;
const FLAP_CALLS: [&str; 5] = ["flaps_0", "flaps_1", "flaps_2", "flaps_3", "flaps_full"];
const DEVIATIONS: [&str; 4] = ["speed", "sink_rate", "bank", "pitch"];

#[derive(Clone, Copy, Debug, Default)]
struct Prev {
    alt_ft: f64,
    flap: u8,
    gear_handle_down: bool,
    beacon: bool,
    landing_lights: bool,
    engines_running: u8,
    fmgc_phase: u8,
}

pub struct PilotMonitoring {
    pub duties: Duties,
    source: fn(u32) -> Option<EcamProcedure>,
    tracker: PhaseTracker,
    t: f64,
    prev: Option<Prev>,
    takeoff: [bool; 4],
    landing: [bool; 4],
    touchdown_at: Option<f64>,
    reverse_selected_at: Option<f64>,
    approach_armed: bool,
    thousand_armed: bool,
    taxi_warned: bool,
    ga_climb_armed: bool,
    gear_up_at: Option<f64>,
    flap_call_at: Option<f64>,
    after_start_due: Option<f64>,
    cl_after_start_due: Option<f64>,
    cl_fired: [bool; 8],
    was_high: bool,
    deviation_latched: [bool; 4],
    ecam: Option<EcamRunner>,
    announced_warning: Option<u32>,
    last_handover: Option<u32>,
    preselected_for: Option<Phase>,
    station: Option<StationKind>,
    takeoff_sent: bool,
    approach_sent: bool,
}

impl PilotMonitoring {
    pub fn new(duties: Duties, source: fn(u32) -> Option<EcamProcedure>) -> Self {
        PilotMonitoring {
            duties,
            source,
            tracker: PhaseTracker::default(),
            t: 0.,
            prev: None,
            takeoff: [false; 4],
            landing: [false; 4],
            touchdown_at: None,
            reverse_selected_at: None,
            approach_armed: false,
            thousand_armed: false,
            taxi_warned: false,
            ga_climb_armed: false,
            gear_up_at: None,
            flap_call_at: None,
            after_start_due: None,
            cl_after_start_due: None,
            cl_fired: [false; 8],
            was_high: false,
            deviation_latched: [false; 4],
            ecam: None,
            announced_warning: None,
            last_handover: None,
            preselected_for: None,
            station: None,
            takeoff_sent: false,
            approach_sent: false,
        }
    }

    pub fn phase(&self) -> Phase {
        self.tracker.phase()
    }

    pub fn ecam_running(&self) -> Option<u32> {
        self.ecam.as_ref().map(|r| r.procedure())
    }

    pub fn step(&mut self, s: &Snapshot, cmds: &[PfCommand], dt: f64) -> Vec<Action> {
        self.t += dt.max(0.);
        let before = self.tracker.phase();
        let phase = self.tracker.step(s);
        let prev = self.prev.unwrap_or(Prev {
            alt_ft: s.alt_ft,
            flap: s.flap_lever,
            gear_handle_down: s.gear_handle_down,
            beacon: s.beacon,
            landing_lights: s.landing_lights,
            engines_running: s.engines_running,
            fmgc_phase: s.fmgc_phase,
        });
        let mut out = Vec::new();
        for c in cmds {
            self.command(*c, s, &mut out);
        }

        let mut calls = Vec::new();
        self.callouts(s, phase, before, &prev, &mut calls);
        let mut gear = Vec::new();
        self.auto_gear(s, &mut gear);
        let fired = self.checklists(s, phase, before, &prev);
        let mut flows = Vec::new();
        self.flows(s, phase, before, &prev, &fired, &mut flows);
        let mut radios = Vec::new();
        self.radios(s, phase, &mut radios);
        let mut mfd = Vec::new();
        self.mfd(s, phase, &mut mfd);

        if self.duties.callouts {
            out.extend(calls);
        }
        if self.duties.auto_gear_up {
            out.extend(gear);
        }
        if self.duties.checklists {
            out.extend(fired.iter().map(|c| Action::Checklist(*c)));
        }
        if self.duties.cabin {
            out.extend(fired.iter().filter_map(|c| pa_for(*c)).map(Action::Pa));
        }
        if self.duties.flows {
            out.extend(flows);
        }
        if self.duties.radios {
            out.extend(radios);
        }
        if self.duties.mfd {
            out.extend(mfd);
        }
        if self.duties.ecam {
            self.ecam_step(s, cmds.contains(&PfCommand::Confirm), &mut out);
        }

        self.prev = Some(Prev {
            alt_ft: s.alt_ft,
            flap: s.flap_lever,
            gear_handle_down: s.gear_handle_down,
            beacon: s.beacon,
            landing_lights: s.landing_lights,
            engines_running: s.engines_running,
            fmgc_phase: s.fmgc_phase,
        });
        out
    }

    fn command(&mut self, c: PfCommand, s: &Snapshot, out: &mut Vec<Action>) {
        match c {
            PfCommand::GearUp => {
                if !s.on_ground && s.vs_fpm > 0. && s.gear_handle_down {
                    self.gear_up_at = None;
                    out.push(Action::GearLever(Lever::Up));
                    out.push(Action::Say("gear_up"));
                }
            }
            PfCommand::GearDown => {
                if !s.on_ground && !s.gear_handle_down {
                    out.push(Action::GearLever(Lever::Down));
                }
            }
            PfCommand::Flaps(n) => {
                let n = n.min(4);
                if n == s.flap_lever {
                    return;
                }
                let vfe = s.vfe_kt[n as usize];
                if n > s.flap_lever && vfe > 0. && s.ias_kt > vfe {
                    out.push(Action::Say("speed_check"));
                } else {
                    out.push(Action::FlapLever(n));
                }
            }
            PfCommand::GoAroundFlaps => {
                if s.flap_lever > 0 {
                    out.push(Action::FlapLever(s.flap_lever - 1));
                }
                self.ga_climb_armed = true;
            }
            PfCommand::EcamActions => {
                if !self.duties.ecam || self.ecam.is_some() {
                    return;
                }
                let id = s.ecam.as_ref().map(|l| l.procedure).or(s.ecam_warning);
                match id.and_then(self.source) {
                    Some(p) => self.ecam = Some(EcamRunner::new(p, self.t)),
                    None => out.push(Action::Say("unable")),
                }
            }
            PfCommand::Confirm => {}
            PfCommand::Checklist(code) => out.push(Action::Checklist(code)),
            PfCommand::Contact(kind) => match radio::find(&s.frequencies, kind) {
                Some(st) => self.handover(s, st.kind, st.khz, out),
                None => out.push(Action::Say("unable")),
            },
            PfCommand::RequestPush => out.push(Action::Interphone(Interphone::Push)),
            PfCommand::RequestStart => out.push(Action::Interphone(Interphone::Start)),
            PfCommand::Disconnect => out.push(Action::Interphone(Interphone::Disconnect)),
            PfCommand::EnterTakeoffData => match s.takeoff_data {
                Some(d) => {
                    self.takeoff_sent = true;
                    out.push(Action::Mfd(MfdEntry::Takeoff(d)));
                    out.push(Action::Say("takeoff_data_set"));
                }
                None => out.push(Action::Say("unable")),
            },
            PfCommand::EnterApproachData => match s.approach_data {
                Some(d) => {
                    self.approach_sent = true;
                    out.push(Action::Mfd(MfdEntry::Approach(d)));
                    out.push(Action::Say("approach_data_set"));
                }
                None => out.push(Action::Say("unable")),
            },
        }
    }

    fn callouts(&mut self, s: &Snapshot, phase: Phase, before: Phase, prev: &Prev, out: &mut Vec<Action>) {
        let t = self.t;
        if s.on_ground {
            self.thousand_armed = false;
            self.deviation_latched = [false; 4];
            if s.gs_kt < 30. && s.tla_max < 20. {
                self.takeoff = [false; 4];
                self.gear_up_at = None;
            }
            if matches!(phase, Phase::TaxiOut | Phase::TaxiIn) {
                if s.gs_kt > 30. && s.tla_max < FLX_TLA && !self.taxi_warned {
                    self.taxi_warned = true;
                    out.push(Action::Say("taxi_speed"));
                } else if s.gs_kt < 20. {
                    self.taxi_warned = false;
                }
            }
            if phase == Phase::Rollout && before != Phase::Rollout {
                self.touchdown_at = Some(t);
                self.landing = [false; 4];
                self.reverse_selected_at = None;
                self.was_high = false;
            }
            if phase == Phase::Rollout {
                self.rollout(s, out);
            } else if phase == Phase::TakeoffRoll {
                self.takeoff_roll(s, out);
            }
            return;
        }
        self.touchdown_at = None;
        if s.alt_ft > 12_000. {
            self.was_high = true;
        }
        self.selections(s, prev, out);
        if self.takeoff[0] && !self.takeoff[3] && s.vs_fpm > 300. && s.ra_ft > 20. {
            self.takeoff[3] = true;
            out.push(Action::Say("pos_climb"));
            if s.gear_handle_down {
                self.gear_up_at = Some(t + AUTO_GEAR_UP_S);
            }
        }
        if self.ga_climb_armed && phase == Phase::GoAround && s.vs_fpm > 300. {
            self.ga_climb_armed = false;
            out.push(Action::Say("pos_climb"));
        }
        if s.ra_ft > 1500. {
            self.approach_armed = true;
        }
        if self.approach_armed && s.ra_ft <= 1000. && s.ra_ft > 300. && s.vs_fpm < -100. {
            self.approach_armed = false;
            let stable = s.gear_down_locked && s.flap_lever >= 3 && s.vs_fpm > -1500. && s.tla_max < FLX_TLA;
            out.push(Action::Say(if stable { "stabilised" } else { "not_stabilised" }));
        }
        if phase == Phase::Approach && s.ra_ft > 50. && s.ra_ft <= 1000. {
            let dev = [
                s.vapp_kt > 0. && (s.ias_kt < s.vapp_kt - DEV_SPEED_LOW_KT || s.ias_kt > s.vapp_kt + DEV_SPEED_HIGH_KT),
                s.vs_fpm < DEV_SINK_FPM,
                s.bank_deg.abs() > DEV_BANK_DEG,
                s.pitch_deg > DEV_PITCH_UP_DEG || s.pitch_deg < DEV_PITCH_DOWN_DEG,
            ];
            for (n, on) in dev.into_iter().enumerate() {
                if on && !self.deviation_latched[n] {
                    out.push(Action::Say(DEVIATIONS[n]));
                }
                self.deviation_latched[n] = on;
            }
        }
        let alt = s.alt_ft;
        if s.trans_alt_ft > 0. && s.vs_fpm > 0. && prev.alt_ft < s.trans_alt_ft && alt >= s.trans_alt_ft {
            out.push(Action::Say("transition_std"));
        }
        if s.trans_lvl_ft > 0. && s.vs_fpm < 0. && prev.alt_ft > s.trans_lvl_ft && alt <= s.trans_lvl_ft {
            out.push(Action::Say("transition_qnh"));
        }
        if s.fcu_alt_ft > 0. {
            let diff = (s.fcu_alt_ft - alt).abs();
            if diff > 1500. {
                self.thousand_armed = true;
            }
            let towards = (s.fcu_alt_ft - alt).signum() == s.vs_fpm.signum() && s.vs_fpm.abs() > 300.;
            if self.thousand_armed && towards && diff <= 1000. && (s.fcu_alt_ft - prev.alt_ft).abs() > 1000. {
                self.thousand_armed = false;
                out.push(Action::Say("1000_to_go"));
            }
        }
    }

    fn selections(&mut self, s: &Snapshot, prev: &Prev, out: &mut Vec<Action>) {
        let t = self.t;
        if prev.flap != s.flap_lever {
            self.flap_call_at = Some(t + FLAP_SETTLE_S);
        }
        if self.flap_call_at.is_some_and(|at| t >= at) {
            self.flap_call_at = None;
            out.push(Action::Say(FLAP_CALLS[s.flap_lever.min(4) as usize]));
        }
        if !prev.gear_handle_down && s.gear_handle_down {
            out.push(Action::Say("gear_down"));
        }
    }

    fn takeoff_roll(&mut self, s: &Snapshot, out: &mut Vec<Action>) {
        if s.tla_max < FLX_TLA {
            return;
        }
        if !self.takeoff[0] && s.ias_kt >= 40. {
            self.takeoff[0] = true;
            out.push(Action::Say("thrust_set"));
        }
        if self.takeoff[0] && !self.takeoff[1] && s.ias_kt >= 100. {
            self.takeoff[1] = true;
            out.push(Action::Say("100kt"));
        }
        if self.takeoff[1] && !self.takeoff[2] && s.vr_kt >= 100. && s.ias_kt >= s.vr_kt {
            self.takeoff[2] = true;
            out.push(Action::Say("rotate"));
        }
    }

    fn rollout(&mut self, s: &Snapshot, out: &mut Vec<Action>) {
        let t = self.t;
        let since = self.touchdown_at.map_or(0., |td| t - td);
        if s.gs_kt < 30. {
            return;
        }
        if !self.landing[0] {
            if s.spoilers_extended {
                self.landing[0] = true;
                out.push(Action::Say("spoilers"));
            } else if since >= 3. {
                self.landing[0] = true;
                out.push(Action::Say("no_spoilers"));
            }
        }
        if s.tla_min <= REVERSE_TLA {
            let at = *self.reverse_selected_at.get_or_insert(t);
            if !self.landing[1] {
                if s.reversers_deployed {
                    self.landing[1] = true;
                    out.push(Action::Say("reverse_green"));
                } else if t - at >= 4. {
                    self.landing[1] = true;
                    out.push(Action::Say("no_reverse"));
                }
            }
        }
        if !self.landing[2] {
            if s.decel_g >= 0.1 {
                self.landing[2] = true;
                out.push(Action::Say("decel"));
            } else if since >= 6. && s.autobrake_armed {
                self.landing[2] = true;
                out.push(Action::Say("no_decel"));
            }
        }
        if !self.landing[3] && s.ias_kt <= 70. {
            self.landing[3] = true;
            out.push(Action::Say("70kt"));
        }
    }

    fn auto_gear(&mut self, s: &Snapshot, out: &mut Vec<Action>) {
        if let Some(at) = self.gear_up_at {
            if s.on_ground {
                self.gear_up_at = None;
            } else if self.t >= at {
                self.gear_up_at = None;
                if s.gear_handle_down {
                    out.push(Action::GearLever(Lever::Up));
                    out.push(Action::Say("gear_up"));
                }
            }
        }
    }

    fn fire(&mut self, code: u8, fired: &mut Vec<u8>) {
        if !self.cl_fired[code as usize] {
            self.cl_fired[code as usize] = true;
            fired.push(code);
        }
    }

    fn checklists(&mut self, s: &Snapshot, phase: Phase, before: Phase, prev: &Prev) -> Vec<u8> {
        let mut fired = Vec::new();
        let t = self.t;
        let all_running = s.engines_running == ENGINES;
        let fresh_start = prev.engines_running < ENGINES && all_running && s.on_ground && s.gs_kt < 5. && matches!(phase, Phase::Preflight | Phase::TaxiOut);
        if fresh_start {
            self.after_start_due = Some(t + AFTER_START_DELAY_S);
            self.cl_after_start_due = Some(t + AFTER_START_CHECKLIST_S);
        }
        if !s.on_ground || !all_running {
            self.after_start_due = None;
            self.cl_after_start_due = None;
        }
        let beacon_on = !prev.beacon && s.beacon;
        let lights_on = !prev.landing_lights && s.landing_lights;
        match phase {
            Phase::Preflight | Phase::Parked if beacon_on && s.engines_running == 0 => {
                if phase == Phase::Parked || self.cl_fired[CL_PARKING as usize] {
                    self.cl_fired = [false; 8];
                }
                self.fire(CL_BEFORE_START, &mut fired);
            }
            Phase::TaxiOut | Phase::Preflight => {
                if self.cl_after_start_due.is_some_and(|due| t >= due) {
                    self.cl_after_start_due = None;
                    self.fire(CL_AFTER_START, &mut fired);
                }
                if phase == Phase::TaxiOut && s.gs_kt > 3. && s.gs_kt < 40. {
                    self.fire(CL_TAXI, &mut fired);
                }
                if phase == Phase::TaxiOut && lights_on {
                    self.fire(CL_LINE_UP, &mut fired);
                }
            }
            Phase::Climb | Phase::Cruise | Phase::Descent | Phase::Approach | Phase::GoAround => {
                let entered_approach = s.fmgc_phase == 5 && prev.fmgc_phase != 5;
                if entered_approach || (self.was_high && s.alt_ft < TEN_THOUSAND_FT && s.vs_fpm < -300.) {
                    self.fire(CL_APPROACH, &mut fired);
                }
                if s.gear_handle_down && s.flap_lever >= 3 && s.ra_ft > 0. && s.ra_ft < 3000. && s.vs_fpm < 0. {
                    self.fire(CL_LANDING, &mut fired);
                }
            }
            Phase::Parked if before == Phase::TaxiIn => {
                self.fire(CL_PARKING, &mut fired);
                self.cl_fired = [false; 8];
                self.cl_fired[CL_PARKING as usize] = true;
                self.takeoff_sent = false;
                self.approach_sent = false;
            }
            _ => {}
        }
        fired
    }

    fn flows(&mut self, s: &Snapshot, phase: Phase, before: Phase, prev: &Prev, fired: &[u8], out: &mut Vec<Action>) {
        if self.after_start_due.is_some_and(|due| self.t >= due) {
            self.after_start_due = None;
            out.push(Action::Flow(Flow::AfterStart));
        }
        if fired.contains(&CL_TAXI) {
            out.push(Action::Flow(Flow::Taxi));
        }
        if !s.on_ground && s.vs_fpm > 0. && prev.alt_ft < TEN_THOUSAND_FT && s.alt_ft >= TEN_THOUSAND_FT {
            out.push(Action::Flow(Flow::Climb10k));
        }
        if !s.on_ground && s.vs_fpm < 0. && prev.alt_ft > TEN_THOUSAND_FT && s.alt_ft <= TEN_THOUSAND_FT {
            out.push(Action::Flow(Flow::Descent10k));
        }
        if phase == Phase::TaxiIn && before == Phase::Rollout && s.tla_min > REVERSE_TLA {
            out.push(Action::Flow(Flow::AfterLanding));
        }
    }

    fn handover(&mut self, s: &Snapshot, kind: StationKind, khz: u32, out: &mut Vec<Action>) {
        if !radio::valid_vhf_khz(khz) {
            out.push(Action::Say("unable"));
            return;
        }
        out.push(Action::ReadFrequency(khz));
        self.station = Some(kind);
        if s.com1_active_khz == khz {
            return;
        }
        if s.com1_standby_khz != khz {
            out.push(Action::TuneStandby { com: 1, khz });
        }
        out.push(Action::SwapCom(1));
    }

    fn radios(&mut self, s: &Snapshot, phase: Phase, out: &mut Vec<Action>) {
        if let Some(h) = s.handover {
            if self.last_handover != Some(h.seq) {
                self.last_handover = Some(h.seq);
                self.handover(s, h.kind, h.khz, out);
                self.preselected_for = None;
            }
        }
        if self.preselected_for == Some(phase) {
            return;
        }
        let current = self.station.or_else(|| radio::kind_of(&s.frequencies, s.com1_active_khz));
        let Some(target) = radio::next_kind(phase, current).and_then(|k| radio::find(&s.frequencies, k)) else { return };
        self.preselected_for = Some(phase);
        if s.com1_standby_khz != target.khz && s.com1_active_khz != target.khz {
            out.push(Action::TuneStandby { com: 1, khz: target.khz });
        }
    }

    fn mfd(&mut self, s: &Snapshot, phase: Phase, out: &mut Vec<Action>) {
        if !self.takeoff_sent && !s.mfd_takeoff_entered && matches!(phase, Phase::Preflight | Phase::TaxiOut) {
            if let Some(d) = s.takeoff_data {
                self.takeoff_sent = true;
                out.push(Action::Mfd(MfdEntry::Takeoff(d)));
                out.push(Action::Say("takeoff_data_set"));
            }
        }
        let near = s.dest_dist_nm.is_some_and(|d| d <= APPROACH_DATA_NM);
        if !self.approach_sent && !s.mfd_approach_entered && near && matches!(phase, Phase::Cruise | Phase::Descent) {
            if let Some(d) = s.approach_data {
                self.approach_sent = true;
                out.push(Action::Mfd(MfdEntry::Approach(d)));
                out.push(Action::Say("approach_data_set"));
            }
        }
    }

    fn ecam_step(&mut self, s: &Snapshot, confirmed: bool, out: &mut Vec<Action>) {
        if s.ecam_warning != self.announced_warning {
            self.announced_warning = s.ecam_warning;
            if let Some(id) = s.ecam_warning {
                out.push(Action::ReadEcamTitle { procedure: id });
            }
        }
        if s.ecam.is_none() && s.ecam_warning.is_none() {
            self.ecam = None;
            return;
        }
        if let Some(r) = self.ecam.as_mut() {
            r.step(self.t, s.ecam.as_ref(), confirmed, out);
            if r.finished() {
                self.ecam = None;
            }
        }
    }
}

fn pa_for(checklist: u8) -> Option<Pa> {
    match checklist {
        CL_LINE_UP => Some(Pa::SeatsForTakeoff),
        CL_APPROACH => Some(Pa::PrepareForLanding),
        CL_LANDING => Some(Pa::SeatsForLanding),
        CL_PARKING => Some(Pa::DisarmDoors),
        _ => None,
    }
}

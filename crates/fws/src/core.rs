use std::collections::HashMap;
use std::rc::Rc;

use crate::fwc::{b, send_failure_warning, Core, Emit, Overlay, Shared};
use crate::vars::{Event, Out, Vars};

pub const START_MS: f64 = 1000.;

/// SystemsHost's FWS part: CPIOM C power and reset gate, the core it builds and destroys, health outputs.
pub struct Fws {
    refresh_offset_ms: f64,
    last_tick: Option<f64>,
    core: Option<Core>,
    io: Shared,
    cpiom_c_available: [bool; 2],
    self_test_time_s: f64,
    last: HashMap<String, u64>,
    last_str: HashMap<String, String>,
    last_topic: HashMap<String, String>,
    out: Vec<Event>,
    extra: Vec<(f64, Emit)>,
    now: f64,
    pending_h_events: Vec<String>,
    pending_sounds: Vec<String>,
    held: Vec<Emit>,
    dc_ess_prev: bool,
}

impl Fws {
    pub fn new(refresh_offset_ms: f64) -> Self {
        Self {
            refresh_offset_ms,
            last_tick: None,
            core: None,
            io: Rc::new(Default::default()),
            cpiom_c_available: [true, true],
            self_test_time_s: 12.,
            last: HashMap::new(),
            last_str: HashMap::new(),
            last_topic: HashMap::new(),
            out: Vec::new(),
            extra: Vec::new(),
            now: 0.,
            pending_h_events: Vec::new(),
            pending_sounds: Vec::new(),
            held: Vec::new(),
            dc_ess_prev: false,
        }
    }

    pub fn flight_phase(&self) -> u8 {
        self.core.as_ref().map_or(0, |c| c.s.flight_phase as u8)
    }

    pub fn current_procedures(&self) -> Vec<u32> {
        self.core.as_ref().map_or(Vec::new(), |c| c.l.presented.iter().map(|p| p.id).collect())
    }

    pub fn startup_completed(&self) -> bool {
        self.core.as_ref().is_some_and(|c| c.s.startup_completed)
    }

    pub fn set_self_test_time_s(&mut self, s: f64) {
        self.self_test_time_s = s;
    }

    /// An `H:` event (ECAM CP STS next page / MORE), handled on the next tick.
    pub fn h_event(&mut self, name: &str) {
        self.pending_h_events.push(name.to_string());
    }

    /// `enqueueSound` from another system (GPWS callouts), handled on the next tick.
    pub fn enqueue_sound(&mut self, key: &str) {
        self.pending_sounds.push(key.to_string());
    }

    /// Bus topics and Coherent calls since the last call: `(t, Emit::Topic | Emit::Call)`.
    pub fn take_extra(&mut self) -> Vec<(f64, Emit)> {
        std::mem::take(&mut self.extra)
    }

    pub fn tick(&mut self, v: &dyn Vars, now_ms: f64) -> Vec<Event> {
        self.now = now_ms;
        self.out.clear();
        if now_ms < START_MS {
            return Vec::new();
        }
        let first = self.last_tick.is_none();
        let dt = self.last_tick.map_or(0., |l| now_ms - l);
        if first {
            self.host_var("L:A32NX_FWS1_IS_HEALTHY", 0.);
            self.host_var("L:A32NX_FWS2_IS_HEALTHY", 0.);
            self.host_var("L:A32NX_FWS_ECP_FAILED", 0.);
            self.create_core(v);
            let mut io = self.io.borrow_mut();
            let (topics, rest): (Vec<Emit>, Vec<Emit>) = std::mem::take(&mut io.out).into_iter().partition(|e| matches!(e, Emit::Topic(..)));
            io.out = rest;
            self.held = topics;
        } else if !self.held.is_empty() {
            let held = std::mem::take(&mut self.held);
            self.io.borrow_mut().out.splice(0..0, held);
        }
        self.last_tick = Some(now_ms);
        let failed = [!self.cpiom_c_available[0], !self.cpiom_c_available[1]];
        if let Some(c) = self.core.as_mut() {
            c.fws1_failed = failed[0];
            c.fws2_failed = failed[1];
            c.run_startup_timer(now_ms);
            if c.l.sc_inhibit_until.is_some_and(|t| now_ms >= t) {
                c.l.sc_inhibit_until = None;
                c.s.aural_single_chime_pending = false;
            }
            crate::sd::run_timers(c, now_ms);
            let ov = Overlay { inner: v, io: self.io.clone() };
            for h in std::mem::take(&mut self.pending_h_events) {
                crate::sd::h_event(c, &ov, &h);
            }
            for s in std::mem::take(&mut self.pending_sounds) {
                c.sound.enqueue_sound(&s);
            }
            c.update(&ov, dt, now_ms);
        }
        self.host_update(v, first);
        self.cpiom_c_available = [b(v.get("L:A32NX_CPIOM_C1_AVAIL")), b(v.get("L:A32NX_CPIOM_C2_AVAIL"))];
        self.drain();
        std::mem::take(&mut self.out)
    }

    fn create_core(&mut self, v: &dyn Vars) {
        let ov = Overlay { inner: v, io: self.io.clone() };
        self.core = Some(Core::new(self.io.clone(), self.refresh_offset_ms, self.self_test_time_s, &ov, self.now));
    }

    fn host_var(&mut self, name: &str, value: f64) {
        self.io.borrow_mut().var(name, value);
    }

    fn host_update(&mut self, v: &dyn Vars, first: bool) {
        let failed = [!self.cpiom_c_available[0], !self.cpiom_c_available[1]];
        let all_failed = failed[0] && failed[1];
        if all_failed && self.core.is_some() {
            self.core = None;
            send_failure_warning(&self.io);
        } else if !all_failed && self.core.is_none() {
            self.create_core(v);
        }
        if let Some(c) = self.core.as_mut() {
            c.fws1_failed = failed[0];
            c.fws2_failed = failed[1];
            c.write_out126();
        }
        let started = self.startup_completed();
        self.host_var("L:A32NX_FWS1_IS_HEALTHY", (!failed[0] && started) as u8 as f64);
        self.host_var("L:A32NX_FWS2_IS_HEALTHY", (!failed[1] && started) as u8 as f64);
        if !first {
            let ecp_not_reachable = !b(v.get("L:A32NX_AFDX_3_3_REACHABLE"))
                && !b(v.get("L:A32NX_AFDX_13_13_REACHABLE"))
                && !b(v.get("L:A32NX_AFDX_4_4_REACHABLE"))
                && !b(v.get("L:A32NX_AFDX_14_14_REACHABLE"));
            self.host_var("L:A32NX_FWS_ECP_FAILED", (!self.dc_ess_prev || ecp_not_reachable) as u8 as f64);
        }
        self.dc_ess_prev = b(v.get("L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED"));
    }

    fn drain(&mut self) {
        let emitted = std::mem::take(&mut self.io.borrow_mut().out);
        for e in emitted {
            match e {
                Emit::Var(name, value) => {
                    if self.last.insert(name.clone(), value.to_bits()) != Some(value.to_bits()) {
                        self.out.push(Event { t: self.now, out: Out::Var(name, value) });
                    }
                }
                Emit::Str(name, value) => {
                    if self.last_str.insert(name.clone(), value.clone()).as_deref() != Some(value.as_str()) {
                        self.out.push(Event { t: self.now, out: Out::Str(name, value) });
                    }
                }
                Emit::Topic(name, value) => {
                    let text = value.to_string();
                    if self.last_topic.insert(name.clone(), text.clone()).as_deref() != Some(text.as_str()) {
                        self.extra.push((self.now, Emit::Topic(name, value)));
                    }
                }
                Emit::Call(name, args) => self.extra.push((self.now, Emit::Call(name, args))),
            }
        }
    }
}


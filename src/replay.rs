use std::collections::VecDeque;

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::xp::{DataRef, Xplm};

const RATE_HZ: f64 = 4.0;
const DEFAULT_KEEP_S: f64 = 1800.0;
const KEY_S: f64 = 30.0;
const DAY_S: f64 = 86_400.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub t: f64,
    pub values: Vec<f32>,
}

#[derive(Clone, Debug)]
struct Stored {
    t: f64,
    key: Option<Vec<f32>>,
    delta: Vec<(u32, f32)>,
}

pub struct Tape {
    frames: VecDeque<Stored>,
    last: Vec<f32>,
    last_key_t: f64,
    keep_s: f64,
}

impl Default for Tape {
    fn default() -> Self {
        Self { frames: VecDeque::new(), last: Vec::new(), last_key_t: f64::NEG_INFINITY, keep_s: DEFAULT_KEEP_S }
    }
}

fn same(a: f32, b: f32) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

impl Tape {
    pub fn set_keep(&mut self, seconds: f64) {
        self.keep_s = seconds.max(60.0);
    }

    pub fn push(&mut self, t: f64, values: Vec<f32>) {
        if let Some(last) = self.frames.back() {
            if t < last.t - 1.0 {
                self.frames.clear();
                self.last.clear();
            } else if t <= last.t {
                return;
            }
        }
        let keyframe = self.frames.is_empty() || values.len() != self.last.len() || t - self.last_key_t >= KEY_S;
        if keyframe {
            self.frames.push_back(Stored { t, key: Some(values.clone()), delta: Vec::new() });
            self.last_key_t = t;
        } else {
            let delta = values.iter().zip(&self.last).enumerate().filter(|(_, (a, b))| !same(**a, **b)).map(|(i, (a, _))| (i as u32, *a)).collect();
            self.frames.push_back(Stored { t, key: None, delta });
        }
        self.last = values;
        let cutoff = t - self.keep_s;
        while self.frames.front().is_some_and(|f| f.t < cutoff) {
            let next_key = self.frames.iter().skip(1).position(|f| f.key.is_some()).map(|k| k + 1);
            match next_key {
                Some(k) if self.frames[k].t <= cutoff => {
                    self.frames.drain(..k);
                }
                _ => break,
            }
        }
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn last_t(&self) -> Option<f64> {
        self.frames.back().map(|f| f.t)
    }

    pub fn first_t(&self) -> Option<f64> {
        self.frames.front().map(|f| f.t)
    }

    fn state_at_index(&self, k: usize) -> Vec<f32> {
        let start = (0..=k).rev().find(|&i| self.frames[i].key.is_some()).unwrap_or(0);
        let mut state = self.frames[start].key.clone().unwrap_or_default();
        for f in self.frames.range(start + 1..=k) {
            if let Some(key) = &f.key {
                state = key.clone();
            } else {
                for (i, v) in &f.delta {
                    if let Some(slot) = state.get_mut(*i as usize) {
                        *slot = *v;
                    }
                }
            }
        }
        state
    }

    pub fn at(&self, t: f64) -> Option<Frame> {
        let first = self.frames.front()?;
        let last = self.frames.back()?;
        if t <= first.t {
            return Some(Frame { t, values: self.state_at_index(0) });
        }
        if t >= last.t {
            return Some(Frame { t, values: self.last.clone() });
        }
        let k = self.frames.partition_point(|f| f.t <= t) - 1;
        let a = self.state_at_index(k);
        let b = self.state_at_index(k + 1);
        let w = ((t - self.frames[k].t) / (self.frames[k + 1].t - self.frames[k].t)) as f32;
        let values = a.iter().zip(&b).map(|(p, q)| if p.is_finite() && q.is_finite() { p + (q - p) * w } else { *p }).collect();
        Some(Frame { t, values })
    }

    pub fn held(&self, t: f64) -> Option<Vec<f32>> {
        self.frames.front()?;
        let k = self.frames.partition_point(|f| f.t <= t).max(1) - 1;
        Some(self.state_at_index(k))
    }

    pub fn stored_values(&self) -> usize {
        self.frames.iter().map(|f| f.key.as_ref().map_or(0, Vec::len) + f.delta.len()).sum()
    }
}

pub fn rig_inputs(main_lua: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in main_lua.split("rd(\"fbw/").skip(1) {
        let Some(end) = part.find('"') else { continue };
        let name = &part[..end];
        if name.starts_with("A32NX_") && !out.iter().any(|n| n == name) {
            out.push(name.to_string());
        }
    }
    out
}

pub struct Replay {
    in_replay: Option<DataRef>,
    zulu: Option<DataRef>,
    date: Option<DataRef>,
    names: Vec<String>,
    ids: Option<Vec<VariableIdentifier>>,
    tape: Tape,
    systems: Tape,
    live: Option<Vec<f32>>,
    next_at: f64,
    playing: bool,
}

impl Replay {
    pub fn new(xplm: &Xplm) -> Self {
        let lua = crate::sound_pack::plugin_dir()
            .and_then(|d| Some(d.parent()?.join("sasl").join("data").join("modules").join("main.lua")))
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        let names = rig_inputs(&lua);
        crate::log(&format!("replay: recording {} rig inputs and the plugin's own animation datarefs", names.len()));
        Self {
            in_replay: xplm.find("sim/time/is_in_replay"),
            zulu: xplm.find("sim/time/zulu_time_sec"),
            date: xplm.find("sim/time/local_date_days"),
            names,
            ids: None,
            tape: Tape::default(),
            systems: Tape::default(),
            live: None,
            next_at: 0.,
            playing: false,
        }
    }

    fn clock(&self, xplm: &Xplm) -> f64 {
        let day = self.date.map_or(0., |d| xplm.get_i(d) as f64);
        let zulu = self.zulu.map_or(0., |d| xplm.get_f(d) as f64);
        day * DAY_S + zulu
    }

    pub fn active(&self, xplm: &Xplm) -> bool {
        self.in_replay.is_some_and(|d| xplm.get_i(d) != 0)
    }

    fn ids<V: VariableRegistry>(&mut self, vars: &mut V) -> &Vec<VariableIdentifier> {
        let names = &self.names;
        self.ids.get_or_insert_with(|| names.iter().map(|n| vars.get(n.clone())).collect())
    }

    pub fn record(&mut self, vars: &mut crate::Vars, xplm: &Xplm) {
        if self.playing {
            self.playing = false;
            if let Some(live) = self.live.take() {
                vars.restore_computed_values(&live);
            }
            crate::log("replay: back in control, recording again");
        }
        let t = self.clock(xplm);
        if t < self.next_at && t > self.next_at - 2.0 / RATE_HZ - 1.0 {
            return;
        }
        self.next_at = t + 1.0 / RATE_HZ;
        let minutes = crate::app_settings::current().replay_minutes;
        self.tape.set_keep(minutes * 60.0);
        self.systems.set_keep(minutes * 60.0);
        self.systems.push(t, vars.computed_values());
        let ids = self.ids(vars).clone();
        let mut values: Vec<f32> = ids.iter().map(|id| vars.read(id) as f32).collect();
        values.extend(crate::published::snapshot_numbers().into_iter().map(|v| v as f32));
        self.tape.push(t, values);
    }

    pub fn play(&mut self, vars: &mut crate::Vars, xplm: &Xplm) {
        if !self.playing {
            self.playing = true;
            self.live = Some(vars.computed_values());
            let span = self.tape.last_t().zip(self.tape.first_t()).map_or(0., |(b, a)| b - a);
            crate::log(&format!("replay: playing back {} frames, {:.0} s, {} stored values", self.tape.len(), span, self.tape.stored_values()));
        }
        let t = self.clock(xplm);
        if let Some(state) = self.systems.held(t) {
            vars.restore_computed_values(&state);
        }
        let Some(frame) = self.tape.at(t) else { return };
        let ids = self.ids(vars).clone();
        let (rig, published) = frame.values.split_at(ids.len().min(frame.values.len()));
        for (id, v) in ids.iter().zip(rig) {
            if v.is_finite() {
                vars.write(id, *v as f64);
            }
        }
        crate::published::restore_numbers(&published.iter().map(|v| *v as f64).collect::<Vec<_>>());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_interpolates_between_recorded_frames() {
        let mut tape = Tape::default();
        tape.push(100., vec![0., 0.]);
        tape.push(101., vec![10., 20.]);
        let f = tape.at(100.25).unwrap();
        assert!((f.values[0] - 2.5).abs() < 1e-5);
        assert!((f.values[1] - 5.).abs() < 1e-5);
        assert_eq!(tape.at(50.).unwrap().values[0], 0.);
        assert_eq!(tape.at(500.).unwrap().values[0], 10.);
    }

    #[test]
    fn held_state_steps_without_blending_packed_words() {
        let mut tape = Tape::default();
        tape.push(100., vec![0x2000 as f32, 1.]);
        tape.push(101., vec![0x8000 as f32, 0.]);
        assert_eq!(tape.held(100.5).unwrap(), vec![0x2000 as f32, 1.]);
        assert_eq!(tape.held(101.).unwrap(), vec![0x8000 as f32, 0.]);
        assert_eq!(tape.held(50.).unwrap(), vec![0x2000 as f32, 1.]);
        assert!(Tape::default().held(1.).is_none());
    }

    #[test]
    fn deltas_rebuild_the_exact_state_between_keyframes() {
        let mut tape = Tape::default();
        for k in 0..400 {
            let t = k as f64 / 4.;
            tape.push(t, vec![(k % 7) as f32, 5., if k > 200 { 1. } else { 0. }]);
        }
        let f = tape.at(60.0).unwrap();
        assert_eq!(f.values, vec![(240 % 7) as f32, 5., 1.]);
        assert!(tape.stored_values() < 400 * 3, "unchanged values are not stored every frame");
    }

    #[test]
    fn the_tape_keeps_the_configured_length_and_restarts_when_time_jumps_back() {
        let mut tape = Tape::default();
        tape.set_keep(600.);
        for k in 0..(4 * 2000) {
            tape.push(k as f64 / 4., vec![k as f32]);
        }
        let span = tape.last_t().unwrap() - tape.first_t().unwrap();
        assert!(span <= 600. + KEY_S + 0.5 && span >= 600., "{span}");
        let t0 = tape.first_t().unwrap();
        let f = tape.at(t0 + 10.).unwrap();
        assert!((f.values[0] - ((t0 + 10.) * 4.) as f32).abs() < 1e-3);
        tape.push(10., vec![1.]);
        assert_eq!(tape.len(), 1, "a sim time far in the past starts a new recording");
    }

    #[test]
    fn the_rig_inputs_come_from_main_lua() {
        let lua = r#"set(D["fbw/anim/a"], rd("fbw/A32NX_LEFT_FLAPS_ANIMATION_POSITION") / 100) x = rd("fbw/A32NX_RAT_RPM") y = rd("fbw/A32NX_RAT_RPM") z = rd("fbw/anim/other")"#;
        assert_eq!(rig_inputs(lua), vec!["A32NX_LEFT_FLAPS_ANIMATION_POSITION".to_string(), "A32NX_RAT_RPM".to_string()]);
    }
}

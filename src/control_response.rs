//! Control response recorder: every sharp sidestick movement writes one CSV of
//! the half second before it and the four seconds after, frame by frame -- the
//! stick, the surface deflections X-Plane was given, and the aircraft's body
//! rates -- to `Output/fbw-a380x/control_response/`, so the delay from stick
//! to surface to motion can be read off a real flight.

use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;

use crate::flight_controls::Surfaces;
use crate::xp::{DataRef, Xplm};

const BEFORE_S: f64 = 0.5;
const AFTER_S: f64 = 4.0;
const STEP: f64 = 0.25;
const STEP_WINDOW_S: f64 = 0.25;
const MAX_FILES: usize = 12;

#[derive(Clone, Copy)]
struct Row {
    time: f64,
    delta: f64,
    stick: [f64; 3],
    elevators: [f64; 2],
    ailerons: [f64; 2],
    rudder: f64,
    rates: [f32; 3],
    attitude: [f32; 2],
}

#[derive(Default)]
pub struct ControlResponse {
    refs: Option<[Option<DataRef>; 5]>,
    history: VecDeque<Row>,
    recording: Option<(f64, Vec<Row>)>,
    written: usize,
    dir: Option<PathBuf>,
}

/// A step: the stick moved by at least STEP within STEP_WINDOW_S on any axis.
fn stepped(history: &VecDeque<Row>, now: &Row) -> bool {
    history.iter().rev().take_while(|r| now.time - r.time <= STEP_WINDOW_S).any(|r| (0..3).any(|a| (now.stick[a] - r.stick[a]).abs() >= STEP))
}

impl ControlResponse {
    pub fn update(&mut self, xplm: &Xplm, time: f64, delta: f64, stick: [f64; 3], s: &Surfaces) {
        if self.written >= MAX_FILES {
            return;
        }
        let refs = *self.refs.get_or_insert_with(|| {
            ["sim/flightmodel/position/P", "sim/flightmodel/position/Q", "sim/flightmodel/position/R", "sim/flightmodel/position/theta", "sim/flightmodel/position/phi"].map(|n| xplm.find(n))
        });
        let get = |i: usize| refs[i].map_or(f32::NAN, |d| xplm.get_f(d));
        let row = Row {
            time,
            delta,
            stick,
            elevators: s.elevators_deg,
            ailerons: [s.ailerons_deg[0][0], s.ailerons_deg[1][0]],
            rudder: s.rudder_deg,
            rates: [get(0), get(1), get(2)],
            attitude: [get(3), get(4)],
        };
        if let Some((start, rows)) = self.recording.as_mut() {
            rows.push(row);
            if time - *start >= AFTER_S {
                let (start, rows) = self.recording.take().unwrap_or_default();
                self.write(start, &rows);
            }
        } else if stepped(&self.history, &row) {
            let mut rows: Vec<Row> = self.history.iter().copied().collect();
            rows.push(row);
            self.recording = Some((time, rows));
        }
        self.history.push_back(row);
        while self.history.front().is_some_and(|r| time - r.time > BEFORE_S) {
            self.history.pop_front();
        }
    }

    fn write(&mut self, start: f64, rows: &[Row]) {
        let dir = self.dir.get_or_insert_with(|| crate::xp::xplane_root().unwrap_or_default().join("Output").join("fbw-a380x").join("control_response")).clone();
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("step_{:03}_{:.0}s.csv", self.written + 1, start));
        let Ok(mut f) = std::fs::File::create(&path) else { return };
        let _ = writeln!(f, "t_rel_s,frame_s,stick_pitch,stick_roll,stick_yaw,elev_l_deg,elev_r_deg,ail_l_deg,ail_r_deg,rudder_deg,p_deg_s,q_deg_s,r_deg_s,theta_deg,phi_deg");
        for r in rows {
            let _ = writeln!(
                f,
                "{:.4},{:.4},{:.4},{:.4},{:.4},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3}",
                r.time - start, r.delta, r.stick[0], r.stick[1], r.stick[2], r.elevators[0], r.elevators[1], r.ailerons[0], r.ailerons[1], r.rudder, r.rates[0], r.rates[1], r.rates[2], r.attitude[0], r.attitude[1]
            );
        }
        self.written += 1;
        crate::log(&format!("control response: {} rows -> {}", rows.len(), path.display()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(time: f64, pitch: f64) -> Row {
        Row { time, delta: 0.03, stick: [pitch, 0., 0.], elevators: [0.; 2], ailerons: [0.; 2], rudder: 0., rates: [0.; 3], attitude: [0.; 2] }
    }

    #[test]
    fn only_a_sharp_stick_movement_starts_a_recording() {
        let mut h = VecDeque::new();
        h.push_back(row(0.0, 0.0));
        h.push_back(row(0.1, 0.05));
        assert!(!stepped(&h, &row(0.2, 0.1)), "slow drift");
        assert!(stepped(&h, &row(0.2, 0.3)), "0.3 of travel in 0.2 s");
        assert!(!stepped(&h, &row(0.6, 0.3)), "the old sample is outside the window");
    }
}

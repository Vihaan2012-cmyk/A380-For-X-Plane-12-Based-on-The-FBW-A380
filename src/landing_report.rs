//! Touchdown report for the EFB: sink rate, load factor, speed, attitude
//! and bounces of each landing, graded against the A380's own limit descent
//! velocities (FAA special conditions, Docket NM341, cond. A.2: 10 ft/s at
//! the design landing weight, 6 ft/s at the design take-off weight, the
//! figures `deep::gear_structure::strut` is built on), interpolated by the
//! landing weight between the weight variant's MLW and MTOW. Kept in
//! `Output/preferences/fbw_a380x_landings.json`, newest first.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::xp::{DataRef, Xplm};
use crate::Vars;

const MIN_FLIGHT_S: f64 = 30.0;
const BOUNCE_WINDOW_S: f64 = 4.0;
const SETTLE_S: f64 = 5.0;
const KEEP: usize = 50;
const FPM_PER_FPS: f64 = 60.0;
const LIMIT_MLW_FPS: f64 = 10.0;
const LIMIT_MTOW_FPS: f64 = 6.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    pub on_ground: bool,
    pub vs_fpm: f64,
    pub g: f64,
    pub ias_kt: f64,
    pub gs_kt: f64,
    pub pitch_deg: f64,
    pub roll_deg: f64,
    pub weight_kg: f64,
    pub mlw_kg: f64,
    pub mtow_kg: f64,
    pub lat: f64,
    pub lon: f64,
    pub zulu_s: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Landing {
    pub zulu_s: f64,
    pub vs_fpm: f64,
    pub g: f64,
    pub ias_kt: f64,
    pub gs_kt: f64,
    pub pitch_deg: f64,
    pub roll_deg: f64,
    pub bounces: u32,
    pub weight_kg: f64,
    pub limit_fpm: f64,
    pub lat: f64,
    pub lon: f64,
    pub grade: String,
}

pub fn limit_fpm(weight_kg: f64, mlw_kg: f64, mtow_kg: f64) -> f64 {
    if mlw_kg <= 0. || mtow_kg <= mlw_kg {
        return LIMIT_MLW_FPS * FPM_PER_FPS;
    }
    let f = ((weight_kg - mlw_kg) / (mtow_kg - mlw_kg)).clamp(0., 1.);
    (LIMIT_MLW_FPS + (LIMIT_MTOW_FPS - LIMIT_MLW_FPS) * f) * FPM_PER_FPS
}

pub fn grade(vs_fpm: f64, limit: f64) -> &'static str {
    let sink = vs_fpm.abs();
    if sink > limit {
        "Hard landing: above the limit descent velocity, report it for inspection"
    } else if sink > 0.6 * limit {
        "Firm"
    } else if sink > 120. {
        "Normal"
    } else {
        "Smooth"
    }
}

#[derive(Default)]
pub struct Detector {
    airborne_s: f64,
    last_air: Option<Frame>,
    pending: Option<(Landing, f64, bool)>,
}

impl Detector {
    pub fn step(&mut self, f: &Frame, dt: f64) -> Option<Landing> {
        if let Some((landing, since, on_ground_now)) = self.pending.as_mut() {
            *since += dt;
            if f.on_ground {
                landing.g = landing.g.max(f.g);
                if !*on_ground_now {
                    *on_ground_now = true;
                }
            } else if *on_ground_now && *since <= BOUNCE_WINDOW_S {
                landing.bounces += 1;
                *on_ground_now = false;
            }
            let settled = f.on_ground && *since >= SETTLE_S;
            let went_around = !f.on_ground && *since > BOUNCE_WINDOW_S;
            if settled {
                return self.pending.take().map(|(l, _, _)| l);
            }
            if went_around {
                self.pending = None;
                self.airborne_s = 0.;
            }
            return None;
        }
        if !f.on_ground {
            self.airborne_s += dt;
            self.last_air = Some(*f);
            return None;
        }
        if self.airborne_s >= MIN_FLIGHT_S {
            let air = self.last_air.unwrap_or(*f);
            let limit = limit_fpm(f.weight_kg, f.mlw_kg, f.mtow_kg);
            self.pending = Some((
                Landing {
                    zulu_s: f.zulu_s,
                    vs_fpm: air.vs_fpm.min(0.).round(),
                    g: f.g.max(air.g),
                    ias_kt: air.ias_kt.round(),
                    gs_kt: air.gs_kt.round(),
                    pitch_deg: (air.pitch_deg * 10.).round() / 10.,
                    roll_deg: (air.roll_deg * 10.).round() / 10.,
                    bounces: 0,
                    weight_kg: f.weight_kg.round(),
                    limit_fpm: limit.round(),
                    lat: f.lat,
                    lon: f.lon,
                    grade: String::new(),
                },
                0.,
                true,
            ));
        }
        self.airborne_s = 0.;
        None
    }
}

static HISTORY: Mutex<Vec<Landing>> = Mutex::new(Vec::new());

const FILE: &str = "Output/preferences/fbw_a380x_landings.json";

fn path() -> Option<PathBuf> {
    Some(PathBuf::from(FILE))
}

pub fn history_json() -> String {
    let list = HISTORY.lock().map(|h| h.clone()).unwrap_or_default();
    serde_json::to_string(&list).unwrap_or_else(|_| "[]".into())
}

pub struct LandingReport {
    detector: Detector,
    weight: Option<DataRef>,
    refs: [Option<DataRef>; 10],
    mlw: VariableIdentifier,
    mtow: VariableIdentifier,
}

impl LandingReport {
    pub fn new(vars: &mut Vars, xplm: &Xplm) -> Self {
        if let Some(p) = path() {
            if let Ok(text) = std::fs::read_to_string(&p) {
                if let (Ok(list), Ok(mut h)) = (serde_json::from_str::<Vec<Landing>>(&text), HISTORY.lock()) {
                    *h = list;
                }
            }
        }
        Self {
            detector: Detector::default(),
            weight: xplm.find("sim/flightmodel/weight/m_total"),
            refs: [
                "sim/flightmodel/failures/onground_any",
                "sim/flightmodel/position/vh_ind_fpm",
                "sim/flightmodel/forces/g_nrml",
                "sim/flightmodel/position/indicated_airspeed",
                "sim/flightmodel/position/groundspeed",
                "sim/flightmodel/position/theta",
                "sim/flightmodel/position/phi",
                "sim/flightmodel/position/latitude",
                "sim/flightmodel/position/longitude",
                "sim/time/zulu_time_sec",
            ]
            .map(|n| xplm.find(n)),
            mlw: vars.get("A380X_WV_MLW_KG".into()),
            mtow: vars.get("A380X_WV_MTOW_KG".into()),
        }
    }

    pub fn update(&mut self, vars: &mut Vars, xplm: &Xplm, dt: f64) {
        if dt <= 0. {
            return;
        }
        let r = |i: usize| self.refs[i].map_or(0., |d| if i == 7 || i == 8 { xplm.get_d(d) } else { xplm.get_f(d) as f64 });
        let frame = Frame {
            on_ground: self.refs[0].is_some_and(|d| xplm.get_i(d) != 0),
            vs_fpm: r(1),
            g: r(2),
            ias_kt: r(3),
            gs_kt: r(4) * 1.943_844,
            pitch_deg: r(5),
            roll_deg: r(6),
            weight_kg: self.weight.map_or(0., |d| xplm.get_f(d) as f64),
            mlw_kg: vars.read(&self.mlw),
            mtow_kg: vars.read(&self.mtow),
            lat: r(7),
            lon: r(8),
            zulu_s: r(9),
        };
        let Some(mut landing) = self.detector.step(&frame, dt) else { return };
        landing.grade = grade(landing.vs_fpm, landing.limit_fpm).to_owned();
        crate::log(&format!(
            "landing: {:.0} fpm, {:.2} g, {:.0} kt, pitch {:.1}, bank {:.1}, {} bounce(s): {}",
            landing.vs_fpm, landing.g, landing.ias_kt, landing.pitch_deg, landing.roll_deg, landing.bounces, landing.grade
        ));
        let list = match HISTORY.lock() {
            Ok(mut h) => {
                h.insert(0, landing);
                h.truncate(KEEP);
                h.clone()
            }
            Err(_) => return,
        };
        if let (Some(p), Ok(text)) = (path(), serde_json::to_string_pretty(&list)) {
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn air(vs: f64) -> Frame {
        Frame { vs_fpm: vs, g: 1.0, ias_kt: 140., gs_kt: 138., pitch_deg: 3.4, weight_kg: 380_000., mlw_kg: 395_000., mtow_kg: 510_000., ..Default::default() }
    }

    fn ground(g: f64) -> Frame {
        Frame { on_ground: true, g, ..air(0.) }
    }

    fn fly(d: &mut Detector, frames: &[(Frame, f64)]) -> Vec<Landing> {
        let mut out = Vec::new();
        for (f, seconds) in frames {
            let mut t = 0.;
            while t < *seconds {
                out.extend(d.step(f, 0.1));
                t += 0.1;
            }
        }
        out
    }

    #[test]
    fn a_landing_reports_the_last_airborne_sink_and_the_peak_load() {
        let mut d = Detector::default();
        let out = fly(&mut d, &[(air(-700.), 60.), (air(-180.), 0.2), (ground(1.31), 0.3), (ground(1.0), 6.)]);
        assert_eq!(out.len(), 1);
        let l = &out[0];
        assert_eq!(l.vs_fpm, -180.);
        assert!((l.g - 1.31).abs() < 1e-9);
        assert_eq!(l.bounces, 0);
        assert_eq!(l.limit_fpm, 600., "below MLW the limit is 10 ft/s");
        assert_eq!(grade(l.vs_fpm, l.limit_fpm), "Normal");
    }

    #[test]
    fn a_bounce_is_counted_and_a_touch_and_go_is_not_a_landing() {
        let mut d = Detector::default();
        let out = fly(&mut d, &[(air(-400.), 60.), (ground(1.5), 0.5), (air(100.), 1.0), (ground(1.2), 6.)]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].bounces, 1);
        let mut tg = Detector::default();
        assert!(fly(&mut tg, &[(air(-300.), 60.), (ground(1.2), 2.0), (air(800.), 30.)]).is_empty());
        assert!(fly(&mut Detector::default(), &[(ground(1.0), 10.), (air(500.), 5.), (ground(1.0), 10.)]).is_empty(), "a hop is not a flight");
    }

    #[test]
    fn the_limit_falls_to_six_feet_per_second_at_mtow() {
        assert_eq!(limit_fpm(395_000., 395_000., 510_000.), 600.);
        assert_eq!(limit_fpm(510_000., 395_000., 510_000.), 360.);
        assert_eq!(limit_fpm(452_500., 395_000., 510_000.), 480.);
        assert!(grade(-650., 600.).starts_with("Hard"));
        assert_eq!(grade(-90., 600.), "Smooth");
        assert_eq!(grade(-400., 600.), "Firm");
    }
}

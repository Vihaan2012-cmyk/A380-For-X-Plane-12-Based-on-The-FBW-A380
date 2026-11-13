//! The engines' thrust on the airframe, from `physics::engine` instead of X-Plane's generic turbine.
//!
//! `override_engine_forces` hands the plugin the summed propulsive force and moment
//! (`fside/fnrml/faxil_prop`, `L/M/N_prop`). Each engine's thrust acts along its axis (Plane
//! Maker's vertical and side cant) at its mount point (`POINT_XYZ`), in X-Plane's ACF axes:
//! X right, Y up, Z aft; L right roll, M pitch up, N yaw right (DataRefs.txt).
//!
//! The conventions are checked against X-Plane before the override is taken: while X-Plane
//! still flies its own engines, the same sums are made from its own `POINT_thrust` and compared
//! with its own `_prop` forces. Only after they agree for `VERIFY_S` with real thrust does the
//! plugin take over, and it hands back at once if its numbers stop being finite or bounded.

use crate::xp::{DataRef, Xplm};

pub const ENGINES: usize = 4;
const VERIFY_S: f64 = 3.0;
/// The share of the four engines' rated thrust needed before a comparison means anything.
const VERIFY_MIN_SHARE: f64 = 0.05;
const FORCE_TOLERANCE: f64 = 0.03;
const MOMENT_TOLERANCE: f64 = 0.10;
/// Moment slack as thrust times this arm, metres, for the near-zero roll and yaw of
/// symmetric thrust.
const MOMENT_SLACK_ARM_M: f64 = 1.0;

/// `[fside, fnrml, faxil, L, M, N]`.
pub type Prop = [f64; 6];

pub fn prop_forces(thrust_n: &[f64; ENGINES], pos_m: &[[f64; 3]; ENGINES], vert_deg: &[f64; ENGINES], side_deg: &[f64; ENGINES]) -> Prop {
    let mut out = [0.0; 6];
    for i in 0..ENGINES {
        let (v, s) = (vert_deg[i].to_radians(), side_deg[i].to_radians());
        let f = [thrust_n[i] * s.sin() * v.cos(), thrust_n[i] * v.sin(), -thrust_n[i] * v.cos() * s.cos()];
        let r = pos_m[i];
        let tau = [r[1] * f[2] - r[2] * f[1], r[2] * f[0] - r[0] * f[2], r[0] * f[1] - r[1] * f[0]];
        out[0] += f[0];
        out[1] += f[1];
        out[2] += f[2];
        out[3] -= tau[2];
        out[4] += tau[0];
        out[5] -= tau[1];
    }
    out
}

/// Whether `ours` reproduces X-Plane's own `theirs` for total thrust `total_n`.
pub fn agrees(ours: &Prop, theirs: &Prop, total_n: f64) -> bool {
    let slack = total_n * MOMENT_SLACK_ARM_M * 0.05;
    let force = (ours[2] - theirs[2]).abs() <= FORCE_TOLERANCE * ours[2].abs().max(1.0) + 500.0;
    let moments = (3..6).all(|k| (ours[k] - theirs[k]).abs() <= MOMENT_TOLERANCE * ours[k].abs() + slack);
    force && moments
}

#[derive(Default)]
pub struct Verifier {
    agreed_s: f64,
    force_s: f64,
    pub verified: bool,
}

/// Thrust alone matching this long is enough: the moment arms are checked, but a bad arm reading must not keep
/// X-Plane's generic engines flying the aircraft.
const FORCE_ONLY_S: f64 = 20.0;

pub fn force_agrees(ours: &Prop, theirs: &Prop) -> bool {
    (ours[2] - theirs[2]).abs() <= FORCE_TOLERANCE * ours[2].abs().max(1.0) + 500.0
}

impl Verifier {
    pub fn step(&mut self, ours: &Prop, theirs: &Prop, total_n: f64, rated_total_n: f64, dt: f64) -> bool {
        if self.verified {
            return true;
        }
        let thrust = total_n >= VERIFY_MIN_SHARE * rated_total_n;
        self.agreed_s = if thrust && agrees(ours, theirs, total_n) { self.agreed_s + dt } else { 0.0 };
        self.force_s = if thrust && force_agrees(ours, theirs) { self.force_s + dt } else { 0.0 };
        self.verified = self.agreed_s >= VERIFY_S || self.force_s >= FORCE_ONLY_S;
        self.verified
    }
}

struct Refs {
    override_forces: Option<DataRef>,
    thrust: Option<DataRef>,
    xyz: Option<DataRef>,
    location: [Option<DataRef>; 3],
    vert: Option<DataRef>,
    side: Option<DataRef>,
    prop: [Option<DataRef>; 6],
}

pub struct EngineForces {
    refs: Refs,
    verifier: Verifier,
    applying: bool,
    log_s: f64,
    location_logged: bool,
    applied_thrust_n: Option<f64>,
}

fn finite_and_bounded(p: &Prop, rated_total_n: f64) -> bool {
    p.iter().all(|v| v.is_finite()) && p[..3].iter().all(|v| v.abs() <= 2.0 * rated_total_n)
}

impl EngineForces {
    pub fn new(xplm: &Xplm) -> Self {
        let f = |n: &str| xplm.find(n);
        Self {
            refs: Refs {
                override_forces: f("sim/operation/override/override_engine_forces"),
                thrust: f("sim/flightmodel/engine/POINT_thrust"),
                xyz: f("sim/flightmodel/engine/POINT_XYZ"),
                location: ["x", "y", "z"].map(|a| f(&format!("sim/flightmodel2/engines/location_{a}_mtr"))),
                vert: f("sim/aircraft/prop/acf_vertcant"),
                side: f("sim/aircraft/prop/acf_sidecant"),
                prop: [
                    f("sim/flightmodel/forces/fside_prop"),
                    f("sim/flightmodel/forces/fnrml_prop"),
                    f("sim/flightmodel/forces/faxil_prop"),
                    f("sim/flightmodel/forces/L_prop"),
                    f("sim/flightmodel/forces/M_prop"),
                    f("sim/flightmodel/forces/N_prop"),
                ],
            },
            verifier: Verifier::default(),
            applying: false,
            log_s: 0.0,
            location_logged: false,
            applied_thrust_n: None,
        }
    }

    fn array(xplm: &Xplm, d: Option<DataRef>) -> [f64; ENGINES] {
        let mut v = [0f32; ENGINES];
        if let Some(d) = d {
            xplm.get_vf(d, &mut v);
        }
        v.map(f64::from)
    }

    /// `ours_n`: each engine's thrust along its axis from `physics::engine`, zero while its
    /// reverser takes the force (`reversers.rs`).
    pub fn update(&mut self, xplm: &Xplm, ours_n: &[f64; ENGINES], rated_n: f64, dt: f64) {
        let mut xyz = [0f32; ENGINES * 3];
        if let Some(d) = self.refs.xyz {
            xplm.get_vf(d, &mut xyz);
        }
        let mut pos = [0, 1, 2, 3].map(|i| [xyz[i * 3], xyz[i * 3 + 1], xyz[i * 3 + 2]].map(f64::from));
        // X-Plane 12 leaves POINT_XYZ at zero, so the mount points come from the engine locations.
        if pos.iter().flatten().all(|v| v.abs() < 1e-3) {
            let axes = self.refs.location.map(|d| Self::array(xplm, d));
            pos = [0, 1, 2, 3].map(|i| [axes[0][i], axes[1][i], axes[2][i]]);
            if !self.location_logged {
                self.location_logged = true;
                crate::log(&format!("engine forces: POINT_XYZ is empty, mount points from the engine locations {pos:.1?} m"));
            }
        }
        let (vert, side) = (Self::array(xplm, self.refs.vert), Self::array(xplm, self.refs.side));
        let rated_total = rated_n * ENGINES as f64;
        let complete = self.refs.override_forces.is_some() && self.refs.prop.iter().all(Option::is_some) && self.refs.xyz.is_some();
        if !complete {
            return;
        }
        if !self.applying {
            let theirs_t = Self::array(xplm, self.refs.thrust);
            let predicted = prop_forces(&theirs_t, &pos, &vert, &side);
            let actual = self.refs.prop.map(|d| d.map_or(0.0, |d| f64::from(xplm.get_f(d))));
            let total: f64 = theirs_t.iter().sum();
            if self.verifier.step(&predicted, &actual, total, rated_total, dt) {
                crate::log(&format!(
                    "engine forces: X-Plane's own thrust sums matched for {VERIFY_S:.0} s (faxil {:.0}/{:.0} kN, M {:.0}/{:.0} kNm, N {:.0}/{:.0} kNm); physics engine thrust takes over",
                    predicted[2] / 1e3, actual[2] / 1e3, predicted[4] / 1e3, actual[4] / 1e3, predicted[5] / 1e3, actual[5] / 1e3
                ));
                self.applying = true;
            } else {
                self.log_s += dt;
                if self.log_s >= 30.0 && total >= VERIFY_MIN_SHARE * rated_total {
                    self.log_s = 0.0;
                    crate::log(&format!(
                        "engine forces: still on X-Plane's engines, sums differ (faxil {:.0}/{:.0} kN, L {:.0}/{:.0}, M {:.0}/{:.0}, N {:.0}/{:.0} kNm)",
                        predicted[2] / 1e3, actual[2] / 1e3, predicted[3] / 1e3, actual[3] / 1e3, predicted[4] / 1e3, actual[4] / 1e3, predicted[5] / 1e3, actual[5] / 1e3
                    ));
                }
                return;
            }
        }
        let ours = prop_forces(ours_n, &pos, &vert, &side);
        if !finite_and_bounded(&ours, rated_total) {
            crate::log(&format!("engine forces: physics thrust {ours_n:?} N not usable, back on X-Plane's engines"));
            self.release(xplm);
            self.verifier = Verifier::default();
            return;
        }
        if let Some(d) = self.refs.override_forces {
            xplm.set_i(d, 1);
        }
        self.applied_thrust_n = Some(-ours[2]);
        for (d, v) in self.refs.prop.iter().zip(ours) {
            if let Some(d) = d {
                xplm.set_f(*d, v as f32);
            }
        }
    }

    /// The thrust along the body axis this plugin applied last, while it flies the engines.
    pub fn applied_thrust_n(&self) -> Option<f64> {
        self.applying.then_some(self.applied_thrust_n).flatten()
    }

    pub fn release(&mut self, xplm: &Xplm) {
        if let Some(d) = self.refs.override_forces {
            xplm.set_i(d, 0);
        }
        self.applying = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POS: [[f64; 3]; 4] = [[-25.7, -2.0, -3.0], [-15.4, -2.5, 2.0], [15.4, -2.5, 2.0], [25.7, -2.0, -3.0]];

    #[test]
    fn forward_thrust_is_negative_faxil_and_engines_below_the_cg_pitch_up() {
        let p = prop_forces(&[100e3; 4], &POS, &[0.0; 4], &[0.0; 4]);
        assert!((p[2] + 400e3).abs() < 1e-6);
        assert!(p[4] > 0.0, "M {}", p[4]);
        assert!(p[3].abs() < 1e-6 && p[5].abs() < 1e-6, "symmetric thrust: L {} N {}", p[3], p[5]);
    }

    #[test]
    fn a_failed_left_engine_yaws_the_nose_left() {
        let p = prop_forces(&[0.0, 100e3, 100e3, 100e3], &POS, &[0.0; 4], &[0.0; 4]);
        assert!(p[5] < 0.0, "N {}", p[5]);
        let q = prop_forces(&[100e3, 100e3, 100e3, 0.0], &POS, &[0.0; 4], &[0.0; 4]);
        assert!(q[5] > 0.0, "N {}", q[5]);
    }

    #[test]
    fn up_cant_tilts_the_thrust_up() {
        let p = prop_forces(&[100e3; 4], &[[0.0; 3]; 4], &[3.0; 4], &[0.0; 4]);
        assert!(p[1] > 0.0 && p[2] < 0.0);
    }

    #[test]
    fn it_takes_over_only_after_agreeing_with_real_thrust_for_long_enough() {
        let ours = prop_forces(&[60e3; 4], &POS, &[0.0; 4], &[0.0; 4]);
        let mut v = Verifier::default();
        for _ in 0..20 {
            assert!(!v.step(&ours, &ours, 240e3, 1.4e6, 0.1));
        }
        assert!((0..20).any(|_| v.step(&ours, &ours, 240e3, 1.4e6, 0.1)));
        let mut idle = Verifier::default();
        assert!(!(0..100).any(|_| idle.step(&ours, &ours, 10e3, 1.4e6, 0.1)), "too little thrust to compare");
    }

    #[test]
    fn matching_thrust_takes_over_even_when_the_moment_arms_read_wrong() {
        let theirs = prop_forces(&[230e3; 4], &POS, &[0.0; 4], &[0.0; 4]);
        let ours = prop_forces(&[230e3; 4], &[[0.0; 3]; 4], &[0.0; 4], &[0.0; 4]);
        let mut v = Verifier::default();
        assert!(!(0..150).any(|_| v.step(&ours, &theirs, 920e3, 1.4e6, 0.1)), "not on the moments' word alone");
        assert!((0..60).any(|_| v.step(&ours, &theirs, 920e3, 1.4e6, 0.1)), "the thrust matched for 20 s");
    }

    #[test]
    fn a_sign_disagreement_never_verifies() {
        let ours = prop_forces(&[0.0, 100e3, 100e3, 100e3], &POS, &[0.0; 4], &[0.0; 4]);
        let mut flipped = ours;
        flipped[5] = -flipped[5];
        let mut v = Verifier::default();
        assert!(!(0..100).any(|_| v.step(&ours, &flipped, 300e3, 1.4e6, 0.1)));
        let mut aft = ours;
        aft[2] = -aft[2];
        let mut w = Verifier::default();
        assert!(!(0..100).any(|_| w.step(&ours, &aft, 300e3, 1.4e6, 0.1)));
    }
}

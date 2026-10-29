//! A collapsed gear leg (failures 32_130..32_134, picked by the crew or
//! armed by `deep::gear_structure` past the strut's ultimate load) folds
//! all the way up along its own retraction hinge, as a leg does when its
//! downlock or side stay lets go, while X-Plane's `rel_collapse` takes the
//! leg's support away.

/// X-Plane gear order (.acf `_gear/0..4`, `rel_collapse1..5`): nose, left
/// body, right body, left wing, right wing.
pub const FOLDED_DEPLOY: [f64; 5] = [0.0; 5];
pub const FIRST_FAILURE: u64 = 32_130;
const BELLY_SKIDS: std::ops::Range<usize> = 5..9;
const COLLAPSE_S: f64 = 0.8;
const RECOVER_S: f64 = 2.0;

const WATCHED: [&str; 6] = [
    "sim/aircraft/parts/acf_gear_deploy",
    "sim/flightmodel2/gear/deploy_ratio",
    "sim/flightmodel2/gear/tire_vertical_deflection_mtr",
    "sim/flightmodel/parts/tire_x_no_deflection",
    "sim/flightmodel/parts/tire_y_no_deflection",
    "sim/flightmodel/parts/tire_z_no_deflection",
];
const WATCHED_SCALARS: [&str; 3] = ["sim/flightmodel/position/local_vy", "sim/flightmodel/position/theta", "sim/flightmodel/position/y_agl"];
const WATCHED_ANIMS: [&str; 6] = ["fbw/anim/c_gear", "fbw/anim/c_gear_comp", "fbw/anim/l_b_gear", "fbw/anim/r_b_gear", "fbw/anim/l_w_gear", "fbw/anim/r_w_gear"];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GearCollapse {
    fold: [f64; 5],
    window_s: f64,
    range: Vec<(f32, f32)>,
}

impl GearCollapse {
    /// The deploy ratio each leg is held at this tick, `None` for a leg
    /// that is not collapsing or recovering.
    pub fn step(&mut self, collapsed: [bool; 5], dt: f64) -> [Option<f64>; 5] {
        std::array::from_fn(|k| {
            let f = &mut self.fold[k];
            if collapsed[k] {
                *f = (*f + dt / COLLAPSE_S).min(1.);
            } else {
                *f = (*f - dt / RECOVER_S).max(0.);
            }
            (*f > 0.).then(|| 1. - *f * (1. - FOLDED_DEPLOY[k]))
        })
    }

    pub fn update(&mut self, sink: &dyn crate::deep::integration::xp_consequences::ForceSink, dt: f64) {
        let collapsed = std::array::from_fn(|k| crate::failures::is_active(FIRST_FAILURE + k as u64));
        for (k, ratio) in self.step(collapsed, dt).into_iter().enumerate() {
            if let Some(r) = ratio {
                sink.set_gear_deploy_ratio(k, r as f32);
            }
        }
        let skids = self.skids_deployed();
        for k in BELLY_SKIDS {
            sink.set_gear_deploy_ratio(k, if skids { 1. } else { 0. });
        }
    }

    pub fn skids_deployed(&self) -> bool {
        self.fold.iter().any(|f| *f > 0.)
    }

    pub fn damp(&self, xplm: &crate::xp::Xplm, sink: &dyn crate::deep::integration::xp_consequences::ForceSink) {
        use crate::deep::integration::xp_consequences::PlugForceAxis;
        if self.fold.iter().all(|f| *f < 0.5) {
            return;
        }
        let f = |n: &str| xplm.find(n).map_or(0., |d| xplm.get_f(d) as f64);
        if xplm.find("sim/flightmodel/failures/onground_any").map_or(0, |d| xplm.get_i(d)) == 0 {
            return;
        }
        let mass = f("sim/flightmodel/weight/m_total");
        let d = resting_damping(
            mass,
            f("sim/aircraft/weight/acf_Jxx_unitmass") * mass,
            f("sim/aircraft/weight/acf_Jzz_unitmass") * mass,
            f("sim/flightmodel/position/local_vy"),
            f("sim/flightmodel/position/Q").to_radians(),
            f("sim/flightmodel/position/P").to_radians(),
        );
        sink.add_plug_force(PlugForceAxis::Fnrml, d[0]);
        sink.add_plug_force(PlugForceAxis::Pitch, d[1]);
        sink.add_plug_force(PlugForceAxis::Roll, d[2]);
    }

    pub fn report(&mut self, xplm: &crate::xp::Xplm, dt: f64) {
        if self.fold.iter().all(|f| *f == 0.) {
            self.window_s = 0.;
            self.range.clear();
            return;
        }
        let mut now = Vec::with_capacity(WATCHED.len() * 9 + WATCHED_ANIMS.len() + WATCHED_SCALARS.len());
        for name in WATCHED {
            let mut v = [0f32; 10];
            xplm.find(name).map(|d| xplm.get_vf(d, &mut v));
            now.extend_from_slice(&v[..9]);
        }
        for name in WATCHED_ANIMS.iter().chain(WATCHED_SCALARS.iter()) {
            now.push(xplm.find(name).map_or(f32::NAN, |d| xplm.get_f(d)));
        }
        if self.range.len() != now.len() {
            self.range = now.iter().map(|&v| (v, v)).collect();
        }
        for (r, v) in self.range.iter_mut().zip(&now) {
            *r = (r.0.min(*v), r.1.max(*v));
        }
        self.window_s += dt;
        if self.window_s < 1. {
            return;
        }
        let span = |from: usize, n: usize| self.range[from..from + n].iter().map(|(a, b)| format!("{a:.2}..{b:.2}")).collect::<Vec<_>>().join(" ");
        crate::log(&format!(
            "gear collapse: fold {:?} | acf_gear_deploy {} | deploy_ratio {} | deflection {} | tire x {} | tire y {} | tire z {} | anim c/c_comp/lb/rb/lw/rw {} | vy/pitch/agl {}",
            self.fold.map(|f| (f * 100.).round() / 100.),
            span(0, 9),
            span(9, 9),
            span(18, 9),
            span(27, 9),
            span(36, 9),
            span(45, 9),
            span(54, WATCHED_ANIMS.len()),
            span(54 + WATCHED_ANIMS.len(), WATCHED_SCALARS.len())
        ));
        self.window_s = 0.;
        self.range.clear();
    }
}

const RESTING_DECAY_PER_S: f64 = 1.5;

pub fn resting_damping(mass_kg: f64, pitch_inertia: f64, roll_inertia: f64, vy_m_s: f64, q_rad_s: f64, p_rad_s: f64) -> [f64; 3] {
    let k = 2. * RESTING_DECAY_PER_S;
    let limit = 2. * mass_kg * 9.81;
    [(-k * mass_kg * vy_m_s).clamp(-limit, limit), -k * pitch_inertia * q_rad_s, -k * roll_inertia * p_rad_s]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_belly_skids_come_down_with_the_first_folding_leg_and_go_up_once_all_are_repaired() {
        let mut g = GearCollapse::default();
        assert!(!g.skids_deployed());
        let mut c = [false; 5];
        c[2] = true;
        g.step(c, 0.1);
        assert!(g.skids_deployed());
        c[2] = false;
        g.step(c, 5.0);
        assert!(!g.skids_deployed());
    }

    #[test]
    fn a_belly_resting_airframe_is_damped_against_every_motion_and_never_pushed() {
        let [up, pitch, roll] = resting_damping(460_000., 7.0e7, 4.0e7, 2.0, 0.1, -0.05);
        assert!(up < 0. && pitch < 0. && roll > 0.);
        assert_eq!(resting_damping(460_000., 7.0e7, 4.0e7, 0., 0., 0.), [0., 0., 0.]);
        let [hard, _, _] = resting_damping(460_000., 7.0e7, 4.0e7, -50., 0., 0.);
        assert!(hard <= 2. * 460_000. * 9.81 + 1e-6, "clamped to twice the weight");
    }

    #[test]
    fn a_collapsed_leg_folds_to_its_measured_pose_within_a_second() {
        let mut g = GearCollapse::default();
        let mut c = [false; 5];
        assert_eq!(g.step(c, 0.1), [None; 5]);
        c[3] = true;
        let half = g.step(c, 0.4)[3].unwrap();
        assert!(half < 1. && half > FOLDED_DEPLOY[3]);
        let full = g.step(c, 1.0)[3].unwrap();
        assert!((full - FOLDED_DEPLOY[3]).abs() < 1e-12);
        assert_eq!(g.step(c, 0.1)[1], None, "the other legs are untouched");
    }

    #[test]
    fn a_repaired_leg_comes_back_down_and_lets_go() {
        let mut g = GearCollapse::default();
        let mut c = [false; 5];
        c[0] = true;
        g.step(c, 2.0);
        c[0] = false;
        assert!(g.step(c, 1.0)[0].is_some());
        assert_eq!(g.step(c, 2.0)[0], None);
    }
}

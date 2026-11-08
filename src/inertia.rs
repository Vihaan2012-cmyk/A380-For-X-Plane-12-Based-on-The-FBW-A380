//! X-Plane's moments of inertia built the way MSFS builds FlyByWire's: the empty aircraft's MOI from
//! flight_model.cfg plus every payload station and fuel tank as a point mass, about the loaded CG.
//! Off unless `xphfbw.fbwInertia` is on: the .acf's control power was matched to X-Plane's own figures.

use crate::mass_balance::Mass;
use crate::xp::{DataRef, Xplm};

const SLUG_FT2_KGM2: f64 = 14.593_902_9 * 0.092_903_04;
const LB_KG: f64 = 0.453_592_37;
const FT_M: f64 = 0.3048;

/// The cfg's empty pitch, yaw and roll MOI, slug ft^2.
pub fn empty_moi(cfg: &str) -> Option<[f64; 3]> {
    let value = |key: &str| {
        cfg.lines().find_map(|l| {
            let l = l.split(';').next()?.trim();
            let (k, v) = l.split_once('=')?;
            (k.trim() == key).then(|| v.trim().parse::<f64>().ok()).flatten()
        })
    };
    Some([value("empty_weight_pitch_MOI")?, value("empty_weight_yaw_MOI")?, value("empty_weight_roll_MOI")?])
}

/// Pitch, yaw and roll inertia (kg m^2) about the loaded CG, and the total mass (kg). Positions are the cfg's
/// feet z (longitudinal), x (lateral), y (vertical).
pub fn loaded(empty_moi_slug_ft2: [f64; 3], empty: Mass, stations: &[Mass], tanks: &[Mass]) -> ([f64; 3], f64) {
    let all: Vec<Mass> = std::iter::once(empty).chain(stations.iter().copied()).chain(tanks.iter().copied()).collect();
    let (total_lb, cg) = crate::mass_balance::centre_of_gravity(all.iter().copied());
    let mut inertia = empty_moi_slug_ft2.map(|i| i * SLUG_FT2_KGM2);
    for m in &all {
        let kg = m.pounds.max(0.) * LB_KG;
        let [dz, dx, dy] = [0, 1, 2].map(|k| (m.position[k] - cg[k]) * FT_M);
        inertia[0] += kg * (dz * dz + dy * dy);
        inertia[1] += kg * (dz * dz + dx * dx);
        inertia[2] += kg * (dx * dx + dy * dy);
    }
    (inertia, total_lb * LB_KG)
}

pub struct Inertia {
    refs: [Option<DataRef>; 3],
    moi: Option<[f64; 3]>,
    logged: bool,
    since_s: f64,
}

impl Inertia {
    pub fn new(xplm: &Xplm) -> Self {
        Inertia {
            refs: ["Jxx", "Jyy", "Jzz"].map(|j| xplm.find(&format!("sim/aircraft/weight/acf_{j}_unitmass"))),
            moi: empty_moi(crate::weight_balance::FLIGHT_MODEL_CFG),
            logged: false,
            since_s: 1.,
        }
    }

    /// Once a second, with this tick's masses: X-Plane recomputes its own figures when the aircraft reloads.
    pub fn update(&mut self, xplm: &Xplm, delta: f64, masses: Option<&(Mass, Vec<Mass>, Vec<Mass>)>) {
        self.since_s += delta;
        if self.since_s < 1. {
            return;
        }
        self.since_s = 0.;
        let (Some(moi), Some((empty, stations, tanks))) = (self.moi, masses) else { return };
        let (inertia, kg) = loaded(moi, *empty, stations, tanks);
        if kg <= 0. {
            return;
        }
        let wanted = inertia.map(|i| i / kg);
        let had: Vec<f64> = self.refs.iter().map(|r| r.map_or(f64::NAN, |d| xplm.get_f(d) as f64)).collect();
        let on = crate::app_settings::fbw_inertia();
        if !self.logged {
            self.logged = true;
            crate::log(&format!(
                "inertia: X-Plane's pitch {:.0} yaw {:.0} roll {:.0} m^2 per kg; FlyByWire's loaded {:.0} {:.0} {:.0} at {:.0} t ({})",
                had[0], had[1], had[2], wanted[0], wanted[1], wanted[2], kg / 1000., if on { "applied" } else { "xphfbw.fbwInertia off" }
            ));
        }
        if !on {
            return;
        }
        for (r, (w, h)) in self.refs.iter().zip(wanted.iter().zip(&had)) {
            if let Some(d) = r {
                if (w - h).abs() > w * 0.01 {
                    xplm.set_f(*d, *w as f32);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuel_in_the_wings_raises_roll_inertia_and_a_point_at_the_cg_adds_nothing() {
        let moi = empty_moi(crate::weight_balance::FLIGHT_MODEL_CFG).expect("the cfg has all three");
        let empty = Mass { pounds: 661_403., position: [0., 0., 0.] };
        let (bare, _) = loaded(moi, empty, &[], &[]);
        let (centre, _) = loaded(moi, empty, &[Mass { pounds: 10_000., position: [0., 0., 0.] }], &[]);
        assert!((bare[2] - centre[2]).abs() < 1.);
        let wings = [Mass { pounds: 50_000., position: [0., -40., 0.] }, Mass { pounds: 50_000., position: [0., 40., 0.] }];
        let (fuelled, _) = loaded(moi, empty, &[], &wings);
        assert!(fuelled[2] > bare[2] * 1.05, "{} vs {}", fuelled[2], bare[2]);
        assert!((fuelled[0] - bare[0]).abs() < bare[0] * 0.01, "wing fuel on the lateral axis leaves pitch alone");
    }
}

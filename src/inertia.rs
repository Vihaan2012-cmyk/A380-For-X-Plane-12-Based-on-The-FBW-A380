//! X-Plane's moments of inertia: the empty aircraft's from a component build-up (below), plus every payload
//! station and fuel tank as a point mass, about the loaded CG. On unless `xphfbw.fbwInertia` is false.

use crate::mass_balance::Mass;
use crate::xp::{DataRef, Xplm};

const LB_KG: f64 = 0.453_592_37;
const FT_M: f64 = 0.3048;

/// One structural group: tonnes, centroid (cfg feet: z long, x lat, y vert) and its own rms extent (m: long, lat, vert).
struct Part(f64, [f64; 3], [f64; 3]);

/// The A380-800's 300 t empty weight by group: engine, gear and tail positions from FlyByWire's cfgs, masses estimated.
const PARTS: [Part; 11] = [
    Part(154.4, [3., 0., 4.], [17.6, 2.5, 2.9]),
    Part(72.0, [-8., 0., -4.], [5.0, 17.6, 0.8]),
    Part(9.4, [4., -84., -4.], [1.6, 0.8, 0.8]),
    Part(9.4, [28., -48.6, -6.], [1.6, 0.8, 0.8]),
    Part(9.4, [28., 48.6, -6.], [1.6, 0.8, 0.8]),
    Part(9.4, [4., 84., -4.], [1.6, 0.8, 0.8]),
    Part(8.5, [-108., 0., 17.], [2.5, 6.45, 0.5]),
    Part(5.5, [-97., 0., 35.], [2.5, 0.3, 4.0]),
    Part(2.5, [99., 0., -9.], [0.5, 0.5, 1.0]),
    Part(9.8, [-5.7, 0., -10.], [1.0, 3.6, 1.0]),
    Part(9.8, [5.7, 0., -10.], [1.0, 7.0, 1.0]),
];

/// Empty pitch, yaw and roll inertia, kg m^2, about the parts' own CG.
pub fn empty_moi() -> [f64; 3] {
    let total: f64 = PARTS.iter().map(|p| p.0).sum();
    let cg: [f64; 3] = [0, 1, 2].map(|k| PARTS.iter().map(|p| p.0 * p.1[k]).sum::<f64>() / total * FT_M);
    let mut i = [0.; 3];
    for Part(t, pos, rms) in &PARTS {
        let kg = t * 1000.;
        let [l, a, v] = [0, 1, 2].map(|k| (pos[k] * FT_M - cg[k]).powi(2) + rms[k] * rms[k]);
        i[0] += kg * (l + v);
        i[1] += kg * (l + a);
        i[2] += kg * (a + v);
    }
    i
}

/// Pitch, yaw and roll inertia (kg m^2) about the loaded CG, and the total mass (kg). Positions are the cfg's
/// feet z (longitudinal), x (lateral), y (vertical).
pub fn loaded(empty_moi_kg_m2: [f64; 3], empty: Mass, stations: &[Mass], tanks: &[Mass]) -> ([f64; 3], f64) {
    let all: Vec<Mass> = std::iter::once(empty).chain(stations.iter().copied()).chain(tanks.iter().copied()).collect();
    let (total_lb, cg) = crate::mass_balance::centre_of_gravity(all.iter().copied());
    let mut inertia = empty_moi_kg_m2;
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
            moi: Some(empty_moi()),
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
                "inertia: X-Plane's pitch {:.0} yaw {:.0} roll {:.0} m^2 per kg; build-up loaded {:.0} {:.0} {:.0} at {:.0} t ({})",
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
        let moi = empty_moi();
        let empty = Mass { pounds: 661_403., position: [0., 0., 0.] };
        let (bare, _) = loaded(moi, empty, &[], &[]);
        let (centre, _) = loaded(moi, empty, &[Mass { pounds: 10_000., position: [0., 0., 0.] }], &[]);
        assert!((bare[2] - centre[2]).abs() < 1.);
        let wings = [Mass { pounds: 50_000., position: [0., -40., 0.] }, Mass { pounds: 50_000., position: [0., 40., 0.] }];
        let (fuelled, _) = loaded(moi, empty, &[], &wings);
        assert!(fuelled[2] > bare[2] * 1.05, "{} vs {}", fuelled[2], bare[2]);
        assert!((fuelled[0] - bare[0]).abs() < bare[0] * 0.01, "wing fuel on the lateral axis leaves pitch alone");
    }

    /// Per kg at the cfg's 300 t empty weight: an airliner's yaw is a little under pitch plus roll, and roll sits well under pitch.
    #[test]
    fn the_build_up_weighs_the_empty_aircraft_and_has_airliner_proportions() {
        let total: f64 = PARTS.iter().map(|p| p.0).sum();
        assert!((total * 1000. / LB_KG - 661_403.).abs() < 661_403. * 0.01, "{total} t");
        let [pitch, yaw, roll] = empty_moi().map(|i| i / (total * 1000.));
        assert!((200. ..270.).contains(&pitch) && (120. ..170.).contains(&roll) && (0.9..1.0).contains(&(yaw / (pitch + roll))), "{pitch:.0} {yaw:.0} {roll:.0}");
    }
}

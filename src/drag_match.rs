//! Matches X-Plane's A380 drag to the real aircraft's polar. X-Plane's
//! blade-element model gives the converted A380 too little drag (flown
//! samples: L/D near 27 at 215 kt in the circuit, where the aircraft is near
//! 18 clean). Each frame X-Plane's own aerodynamic drag is measured from the
//! propulsive force actually applied (`faxil_prop`, X-Plane's engines or
//! `engine_forces`) and the specific force along the body axis. The target
//! drag is OpenAP's A380-800 drag polar for the same lift coefficient, flap,
//! gear and Mach, and the shortfall is added as an aft plug force, filtered
//! and capped. Lift, handling and stability stay X-Plane's own; drag already
//! added this frame by damage (hail, bird strike, separation) is left on top.
//!
//! OpenAP (Sun, Hoekstra and Ellerbroek, TU Delft; `openap/data/dragpolar/
//! a388.yml` and `openap/drag.py`): the clean polar was estimated from ADS-B
//! flight data; the flap term is McCormick (1994) eq. 3.45-3.46, the gear term
//! Mair and Birdsall (1996) eq. 6.1, the wave drag Gur, Mason and Schetz
//! (2010) eq. 15-18.

use crate::xp::{DataRef, Xplm};

pub const WING_AREA_M2: f64 = 845.0;
const SPAN_M: f64 = 79.75;
/// Quarter-chord sweep, Airbus A380 Aircraft Characteristics.
const SWEEP_DEG: f64 = 33.5;
/// OpenAP's default thickness ratio when an aircraft file gives none (Obert 2009).
const THICKNESS_RATIO: f64 = 0.12;
/// Full flap deflection, CONF FULL (`tools/acf_patch.py`).
const FLAP_FULL_DEG: f64 = 33.0;
const CD0_CLEAN: f64 = 0.016;
const K_CLEAN: f64 = 0.050;
const LAMBDA_FLAP: f64 = 0.9;
const FLAP_CHORD_RATIO: f64 = 0.15;
const FLAP_AREA_RATIO: f64 = 0.15;
const CD_GEAR: f64 = 0.012;
const G: f64 = 9.80665;
const FILTER_S: f64 = 2.0;
const CAP_OF_WEIGHT: f64 = 0.05;
/// Below this share of the target the inferred X-Plane drag is not a drag
/// shortfall but a bad sample (an A380 never flies on a tenth of its drag).
const PLAUSIBLE_OF_TARGET: f64 = 0.1;

pub fn target_cd(cl: f64, mach: f64, flap: f64, gear: f64) -> f64 {
    let flap_deg = flap.clamp(0.0, 1.0) * FLAP_FULL_DEG;
    let cd_flap = LAMBDA_FLAP * FLAP_CHORD_RATIO.powf(1.38) * FLAP_AREA_RATIO * flap_deg.to_radians().sin().powi(2);
    let aspect = SPAN_M * SPAN_M / WING_AREA_M2;
    let k = 1.0 / (1.0 / K_CLEAN + std::f64::consts::PI * aspect * 0.0026 * flap_deg);
    let cos = SWEEP_DEG.to_radians().cos();
    let m_crit = 0.95 / cos - THICKNESS_RATIO / (cos * cos) - 0.1 * cl / cos.powi(3) - 0.108;
    CD0_CLEAN + cd_flap + CD_GEAR * gear.clamp(0.0, 1.0) + k * cl * cl + 20.0 * (mach - m_crit).max(0.0).powi(4)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sample {
    pub airborne: bool,
    pub mass_kg: f64,
    pub thrust_n: f64,
    pub nx: f64,
    pub nz: f64,
    pub q_pa: f64,
    pub mach: f64,
    pub flap: f64,
    pub slat: f64,
    pub gear: f64,
    pub other_plug_drag_n: f64,
}

#[derive(Default)]
pub struct Controller {
    pub applied_n: f64,
    pub last: Option<(f64, f64, f64)>,
}

impl Controller {
    pub fn step(&mut self, s: &Sample, dt: f64) -> f64 {
        let usable = s.airborne && s.mass_kg > 100_000.0 && s.q_pa > 1500.0 && (0.5..=2.0).contains(&s.nz);
        let wanted = if usable {
            let weight = s.mass_kg * G;
            let measured = s.thrust_n + s.nx * weight - self.applied_n - s.other_plug_drag_n;
            let cl = s.nz * weight / (s.q_pa * WING_AREA_M2);
            let target = s.q_pa * WING_AREA_M2 * target_cd(cl, s.mach, s.flap, s.gear);
            self.last = Some((s.nz * weight, measured, target));
            if measured < PLAUSIBLE_OF_TARGET * target {
                0.0
            } else {
                (target - measured).clamp(0.0, CAP_OF_WEIGHT * weight)
            }
        } else {
            0.0
        };
        let a = 1.0 - (-dt.max(0.0) / FILTER_S).exp();
        self.applied_n += (wanted - self.applied_n) * a;
        if !s.airborne {
            self.applied_n = 0.0;
        }
        self.applied_n
    }
}

struct Refs {
    on_ground: Option<DataRef>,
    mass: Option<DataRef>,
    prop: Option<DataRef>,
    nx: Option<DataRef>,
    nz: Option<DataRef>,
    tas: Option<DataRef>,
    rho: Option<DataRef>,
    mach: Option<DataRef>,
    flap: Option<DataRef>,
    slat: Option<DataRef>,
    gear: Option<DataRef>,
    faxil: Option<DataRef>,
}

pub struct DragMatch {
    refs: Refs,
    controller: Controller,
    log_s: f64,
}

impl DragMatch {
    pub fn new(xplm: &Xplm) -> Self {
        let f = |n: &str| xplm.find(n);
        Self {
            refs: Refs {
                on_ground: f("sim/flightmodel/failures/onground_any"),
                mass: f("sim/flightmodel/weight/m_total"),
                prop: f("sim/flightmodel/forces/faxil_prop"),
                nx: f("sim/flightmodel/forces/g_axil"),
                nz: f("sim/flightmodel/forces/g_nrml"),
                tas: f("sim/flightmodel/position/true_airspeed"),
                rho: f("sim/weather/rho"),
                mach: f("sim/flightmodel/misc/machno"),
                flap: f("sim/flightmodel2/controls/flap1_deploy_ratio"),
                slat: f("sim/flightmodel2/controls/slat1_deploy_ratio"),
                gear: f("sim/flightmodel2/gear/deploy_ratio"),
                faxil: f("sim/flightmodel/forces/faxil_plug_acf"),
            },
            controller: Controller::default(),
            log_s: 0.0,
        }
    }

    /// `our_thrust_n`: the thrust `engine_forces` applied, since X-Plane's `faxil_prop` reads back 0 once overridden.
    pub fn update(&mut self, xplm: &Xplm, dt: f64, enabled: bool, our_thrust_n: Option<f64>) {
        let r = |d: Option<DataRef>| d.map_or(0.0, |d| xplm.get_f(d) as f64);
        let mut gear = [0f32; 5];
        if let Some(d) = self.refs.gear {
            xplm.get_vf(d, &mut gear);
        }
        let tas = r(self.refs.tas);
        let s = Sample {
            airborne: enabled && !self.refs.on_ground.is_some_and(|d| xplm.get_i(d) != 0),
            mass_kg: r(self.refs.mass),
            thrust_n: our_thrust_n.unwrap_or_else(|| -r(self.refs.prop)),
            nx: r(self.refs.nx),
            nz: r(self.refs.nz),
            q_pa: 0.5 * r(self.refs.rho) * tas * tas,
            mach: r(self.refs.mach),
            flap: r(self.refs.flap),
            slat: r(self.refs.slat),
            gear: gear.iter().map(|g| *g as f64).sum::<f64>() / 5.0,
            other_plug_drag_n: r(self.refs.faxil),
        };
        let drag = self.controller.step(&s, dt);
        self.log_s += dt;
        if self.log_s >= 10.0 {
            self.log_s = 0.0;
            if let (true, Some((lift, measured, target))) = (s.airborne, self.controller.last.take()) {
                crate::log(&format!(
                    "drag match: L/D X-Plane {:.1}, flown {:.1}, target {:.1} (flap {:.2} slat {:.2} gear {:.2} M{:.2}), adding {:.0} kN",
                    lift / measured.max(1.0),
                    lift / (measured + drag).max(1.0),
                    lift / target.max(1.0),
                    s.flap,
                    s.slat,
                    s.gear,
                    s.mach,
                    drag / 1000.0
                ));
            }
        }
        if drag > 0.0 {
            if let Some(d) = self.refs.faxil {
                xplm.set_f(d, (r(Some(d)) + drag) as f32);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_polar_is_openaps_a380() {
        let ld = |cl: f64, m: f64, flap: f64, gear: f64| cl / target_cd(cl, m, flap, gear);
        assert!((target_cd(0.0, 0.3, 0.0, 0.0) - CD0_CLEAN).abs() < 1e-12);
        assert!((target_cd(0.0, 0.3, 0.0, 1.0) - CD0_CLEAN - CD_GEAR).abs() < 1e-12);
        let max_ld = 1.0 / (2.0 * (CD0_CLEAN * K_CLEAN).sqrt());
        let best = ld((CD0_CLEAN / K_CLEAN).sqrt(), 0.5, 0.0, 0.0);
        assert!((best - max_ld).abs() < 0.05, "best L/D {best:.2} vs {max_ld:.2}");
        let landing = ld(1.69, 0.21, 1.0, 1.0);
        assert!(landing < best * 0.7, "CONF FULL gear down L/D {landing:.1}");
        assert!(target_cd(0.5, 0.92, 0.0, 0.0) > target_cd(0.5, 0.85, 0.0, 0.0) * 1.2, "wave drag past Mdd");
    }

    const MASS: f64 = 458_000.0;
    const THRUST: f64 = 4.0 * 60_000.0;

    fn circuit() -> Sample {
        Sample { airborne: true, mass_kg: MASS, thrust_n: THRUST, nx: 0.0, nz: 1.0, q_pa: 7330.0, mach: 0.33, ..Default::default() }
    }

    fn fly(c: &mut Controller, xplane_drag: f64, other: f64, steps: usize) -> f64 {
        let mut plug = 0.0;
        for _ in 0..steps {
            let nx = (xplane_drag + plug + other - THRUST) / (MASS * G);
            plug = c.step(&Sample { nx, other_plug_drag_n: other, ..circuit() }, 0.05);
        }
        plug
    }

    #[test]
    fn it_adds_only_the_shortfall_and_settles() {
        let weight = MASS * G;
        let cl = weight / (7330.0 * WING_AREA_M2);
        let target = 7330.0 * WING_AREA_M2 * target_cd(cl, 0.33, 0.0, 0.0);
        let applied = fly(&mut Controller::default(), weight / 27.0, 0.0, 600);
        let shortfall = target - weight / 27.0;
        assert!((applied - shortfall).abs() / shortfall < 0.05, "applied {applied:.0} shortfall {shortfall:.0}");
    }

    #[test]
    fn nothing_on_the_ground_or_when_xplane_already_has_enough() {
        let mut c = Controller::default();
        assert_eq!(c.step(&Sample { airborne: false, ..circuit() }, 0.1), 0.0);
        assert!(fly(&mut Controller::default(), MASS * G * 0.2, 0.0, 200) < 1.0);
    }

    #[test]
    fn an_impossible_drag_reading_adds_nothing() {
        let applied = fly(&mut Controller::default(), -200_000.0, 0.0, 200);
        assert!(applied < 1.0, "{applied:.0} N added for a negative X-Plane drag");
    }

    #[test]
    fn damage_drag_is_not_cancelled() {
        let clean = fly(&mut Controller::default(), MASS * G / 27.0, 0.0, 600);
        let damaged = fly(&mut Controller::default(), MASS * G / 27.0, 30_000.0, 600);
        assert!((clean - damaged).abs() < 500.0, "{clean} vs {damaged}");
    }
}

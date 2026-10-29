//! Matches X-Plane's A380 drag to the real aircraft's polar. X-Plane's
//! blade-element model gives the converted A380 too little drag (flown
//! samples: L/D near 27 at 215 kt in the circuit, where the aircraft is near
//! 19 clean and well below that with flap). Each frame X-Plane's own
//! aerodynamic drag is measured from thrust and the specific force along the
//! body axis, the target drag comes from an A380-class polar for the same
//! lift coefficient, flap/slat/gear deployment and Mach, and the shortfall
//! is added as an aft plug force, filtered and capped. Lift, handling and
//! stability stay X-Plane's own; drag already added this frame by damage
//! (hail, bird strike, separation) is left on top.

use crate::xp::{DataRef, Xplm};

pub const WING_AREA_M2: f64 = 845.0;
const G: f64 = 9.80665;
const FILTER_S: f64 = 2.0;
const CAP_OF_WEIGHT: f64 = 0.05;
/// Below this share of the target the inferred X-Plane drag is not a drag
/// shortfall but a bad sample (an A380 never flies on a tenth of its drag).
const PLAUSIBLE_OF_TARGET: f64 = 0.1;

pub fn target_cd(cl: f64, mach: f64, flap: f64, slat: f64, gear: f64) -> f64 {
    let flap = flap.clamp(0.0, 1.0);
    let cd0 = 0.0145 + 0.008 * slat.clamp(0.0, 1.0) + 0.067 * flap.powf(1.3) + 0.018 * gear.clamp(0.0, 1.0);
    let k = 0.045 + 0.013 * flap;
    let m_crit = 0.875 - 0.08 * (cl - 0.45) - 0.108;
    cd0 + k * cl * cl + 20.0 * (mach - m_crit).max(0.0).powi(4)
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
            let target = s.q_pa * WING_AREA_M2 * target_cd(cl, s.mach, s.flap, s.slat, s.gear);
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
    thrust: Option<DataRef>,
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
                thrust: f("sim/flightmodel/engine/POINT_thrust"),
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

    pub fn update(&mut self, xplm: &Xplm, dt: f64, enabled: bool) {
        let r = |d: Option<DataRef>| d.map_or(0.0, |d| xplm.get_f(d) as f64);
        let mut thrust = [0f32; 4];
        if let Some(d) = self.refs.thrust {
            xplm.get_vf(d, &mut thrust);
        }
        let mut gear = [0f32; 5];
        if let Some(d) = self.refs.gear {
            xplm.get_vf(d, &mut gear);
        }
        let tas = r(self.refs.tas);
        let s = Sample {
            airborne: enabled && !self.refs.on_ground.is_some_and(|d| xplm.get_i(d) != 0),
            mass_kg: r(self.refs.mass),
            thrust_n: thrust.iter().map(|t| *t as f64).sum(),
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
    fn the_polar_gives_a380_lift_to_drag_ratios() {
        let ld = |cl: f64, m: f64, flap: f64, slat: f64, gear: f64| cl / target_cd(cl, m, flap, slat, gear);
        let clean = ld(0.52, 0.5, 0.0, 0.0, 0.0);
        assert!((18.5..20.5).contains(&clean), "clean L/D {clean:.1}");
        let landing = ld(1.69, 0.21, 1.0, 1.0, 1.0);
        assert!((5.5..7.0).contains(&landing), "CONF FULL gear down L/D {landing:.1}");
        assert!(target_cd(0.5, 0.92, 0.0, 0.0, 0.0) > target_cd(0.5, 0.85, 0.0, 0.0, 0.0) * 1.2, "wave drag past Mdd");
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
        let target = 7330.0 * WING_AREA_M2 * target_cd(cl, 0.33, 0.0, 0.0, 0.0);
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

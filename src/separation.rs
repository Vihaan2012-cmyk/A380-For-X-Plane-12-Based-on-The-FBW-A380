use crate::failures::extra::{AILERON_SEPARATION, COWL_SEPARATION, ELEVATOR_SEPARATION, FIN_SEPARATION, FLAP_SEPARATION, HSTAB_SEPARATION, RUDDER_SEPARATION, SPOILER_SEPARATION, WINGTIP_SEPARATION, WING_SEPARATION};
use crate::flight_controls::Missing;
use crate::published::{self, Published, Value};

const SIDES: [&str; 2] = ["l", "r"];
const FLAP_PANELS: [&str; 3] = ["inner", "mid", "outer"];
/// Each flap panel's area-weighted span centre (metres from the centreline)
/// and its share of one wing's flapped planform, measured on the panels'
/// own meshes (28.7, 33.0 and 25.1 m2).
pub const FLAP_PANEL_CENTRE_M: [f64; 3] = [8.3, 15.55, 23.12];
pub const FLAP_PANEL_SHARE: [f64; 3] = [0.331, 0.38, 0.289];
/// Lift coefficient the flaps add on the wing reference area at full flap.
pub const FULL_FLAP_DELTA_CL: f64 = 0.9;
/// The first spoiler, aileron and flap (0-based) a wing break at each
/// station takes: on the OBJs, spoiler 3 and the mid flap start at 11.5 m,
/// spoiler 7 and the outer flap at 19.5 m.
const WING_TAKES: [(usize, usize, usize); 4] = [(0, 0, 0), (2, 0, 1), (6, 0, 2), (8, 1, 3)];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Separation {
    pub missing: Missing,
    pub hstab: [bool; 2],
    pub fin: bool,
    pub wingtip: [bool; 2],
    /// The innermost station each wing is broken at (`physics::structure`).
    pub wing: [Option<usize>; 2],
    pub cowl: [bool; 4],
    pub flaps: [[bool; 3]; 2],
    /// Flaps torn off a wing that is still there.
    pub flap_torn: [[bool; 3]; 2],
}

impl Separation {
    pub fn from(active: impl Fn(u64) -> bool) -> Self {
        let mut s = Separation { fin: active(FIN_SEPARATION), ..Default::default() };
        let m = &mut s.missing;
        for side in 0..2 {
            s.hstab[side] = active(HSTAB_SEPARATION + side as u64);
            s.wing[side] = (0..3).find(|k| active(WING_SEPARATION + (side * 3 + k) as u64)).or(active(WINGTIP_SEPARATION + side as u64).then_some(3));
            s.wingtip[side] = s.wing[side].is_some();
            let (spoilers_from, ailerons_from, flaps_from) = s.wing[side].map_or((8, 3, 3), |k| WING_TAKES[k]);
            for k in 0..3 {
                s.flap_torn[side][k] = active(FLAP_SEPARATION + (side * 3 + k) as u64) && k < flaps_from;
                s.flaps[side][k] = s.flap_torn[side][k] || k >= flaps_from;
            }
            for k in 0..3 {
                m.ailerons[side][k] = active(AILERON_SEPARATION + (side * 3 + k) as u64) || k >= ailerons_from;
            }
            for k in 0..2 {
                m.elevators[side][k] = active(ELEVATOR_SEPARATION + (side * 2 + k) as u64) || s.hstab[side];
            }
            for k in 0..8 {
                m.spoilers[side][k] = active(SPOILER_SEPARATION + (side * 8 + k) as u64) || k >= spoilers_from;
            }
        }
        for k in 0..2 {
            m.rudders[k] = active(RUDDER_SEPARATION + k as u64) || s.fin;
        }
        for e in 0..4 {
            s.cowl[e] = active(COWL_SEPARATION + e as u64);
        }
        s
    }

    pub fn current() -> Self {
        Self::from(crate::failures::is_active)
    }

    pub fn parts(&self) -> Vec<(String, bool)> {
        let mut v: Vec<(String, bool)> = self.flags().into_iter().filter(|(name, _)| !name.starts_with("cowl_")).collect();
        v.push(("fin".to_string(), self.fin));
        for (side, s) in SIDES.iter().enumerate() {
            v.push((format!("wingtip_{s}"), self.wingtip[side]));
            for k in 0..3 {
                v.push((format!("wing{}_{s}", k + 1), self.wing[side].is_some_and(|w| w <= k)));
            }
        }
        v
    }

    fn flags(&self) -> Vec<(String, bool)> {
        let m = &self.missing;
        let mut v = Vec::new();
        for (side, s) in SIDES.iter().enumerate() {
            for k in 0..3 {
                v.push((format!("{s}_aileron{}", k + 1), m.ailerons[side][k]));
            }
            for (k, part) in ["inner", "outer"].iter().enumerate() {
                v.push((format!("{s}_elevator_{part}"), m.elevators[side][k]));
            }
            for k in 0..8 {
                v.push((format!("{s}_spoiler{}", k + 1), m.spoilers[side][k]));
            }
            v.push((format!("hstab_{s}"), self.hstab[side]));
            for (k, part) in FLAP_PANELS.iter().enumerate() {
                v.push((format!("{s}_flap_{part}"), self.flaps[side][k]));
            }
        }
        for (k, which) in ["upper", "lower"].iter().enumerate() {
            v.push((format!("rudder_{which}"), m.rudders[k]));
        }
        for e in 0..4 {
            v.push((format!("cowl_{}", e + 1), self.cowl[e]));
        }
        v
    }
}

pub struct SeparationAnims {
    _published: Published,
    values: Vec<Value>,
    last: Separation,
}

impl SeparationAnims {
    pub fn new() -> Self {
        let mut p = Published::default();
        let values = Separation::default().flags().iter().map(|(name, _)| p.number(&format!("fbw/fail/sep/{name}"), 0., false)).collect();
        Self { _published: p, values, last: Separation::default() }
    }

    pub fn update(&mut self) -> Separation {
        let now = Separation::current();
        if now != self.last {
            for (value, (_, on)) in self.values.iter().zip(now.flags()) {
                published::set(*value, on as i32 as f64);
            }
            self.last = now;
        }
        now
    }
}

/// One torn actuator or pump line; one lost surface empties its circuit in about 90 s.
pub const SEVERED_LINE_M2: f64 = 1.5e-6;

/// [green, yellow] leak area, actuator supply as in FlyByWire's A380 model.
pub fn severed_line_leak_m2(level: impl Fn(u64) -> f64, fire_pb_released: [bool; 4]) -> [f64; 2] {
    let mut lines = [0.0_f64; 2];
    for side in 0..2 {
        let broken = |takes: &dyn Fn((usize, usize, usize)) -> bool| {
            (0..4).filter(|&k| takes(WING_TAKES[k])).map(|k| level(crate::physics::structure::wing_break_failure(side, k))).fold(0.0, f64::max)
        };
        for k in 0..3 {
            let l = level(AILERON_SEPARATION + (side * 3 + k) as u64).max(broken(&|t| k >= t.1));
            lines[0] += l;
            lines[1] += l;
        }
        for k in 0..2 {
            let l = level(ELEVATOR_SEPARATION + (side * 2 + k) as u64).max(level(HSTAB_SEPARATION + side as u64));
            lines[side] += 2. * l;
        }
        for k in 0..8 {
            lines[if k % 2 == 0 { 1 } else { 0 }] += level(SPOILER_SEPARATION + (side * 8 + k) as u64).max(broken(&|t| k >= t.0));
        }
    }
    for k in 0..2 {
        let l = level(RUDDER_SEPARATION + k as u64).max(level(FIN_SEPARATION));
        lines[0] += l;
        lines[1] += l;
    }
    for (e, released) in fire_pb_released.iter().enumerate() {
        if !released {
            lines[if e < 2 { 0 } else { 1 }] += 2. * level(crate::failures::extra::ENGINE_SEPARATION + e as u64);
        }
    }
    lines.map(|n| n * SEVERED_LINE_M2)
}

/// Every failure `severed_line_leak_m2` reads.
pub fn leaking_failure_ids() -> Vec<u64> {
    let mut ids = Vec::new();
    for base_count in [(AILERON_SEPARATION, 6), (ELEVATOR_SEPARATION, 4), (SPOILER_SEPARATION, 16), (HSTAB_SEPARATION, 2), (WINGTIP_SEPARATION, 2), (WING_SEPARATION, 6), (RUDDER_SEPARATION, 2), (FIN_SEPARATION, 1), (crate::failures::extra::ENGINE_SEPARATION, 4)] {
        ids.extend((0..base_count.1).map(|k| base_count.0 + k));
    }
    ids
}

/// Every part-separation failure. None is restored from a saved airframe: a
/// new session starts with the parts refitted.
pub fn failure_ids() -> Vec<u64> {
    let mut ids = leaking_failure_ids();
    ids.extend((0..4).map(|k| COWL_SEPARATION + k));
    ids.extend((0..6).map(|k| FLAP_SEPARATION + k));
    ids
}

pub fn flap_forces(sink: &dyn crate::deep::integration::xp_consequences::ForceSink, flaps: &[[bool; 3]; 2], flap_ratio: [f64; 2], dynamic_pressure_pa: f64) {
    use crate::deep::integration::xp_consequences::{PlugForceAxis, A380_WING_REFERENCE_AREA_M2};
    for (side, panels) in flaps.iter().enumerate() {
        let outward = if side == 0 { -1. } else { 1. };
        for (k, gone) in panels.iter().enumerate() {
            if !gone {
                continue;
            }
            let lost = dynamic_pressure_pa * A380_WING_REFERENCE_AREA_M2 * FULL_FLAP_DELTA_CL * flap_ratio[side].clamp(0., 1.) * FLAP_PANEL_SHARE[k] / 2.;
            sink.add_plug_force(PlugForceAxis::Fnrml, -lost);
            sink.add_plug_force(PlugForceAxis::Roll, lost * outward * FLAP_PANEL_CENTRE_M[k]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn torn_surfaces_leak_the_circuits_that_drive_them() {
        let only = |ids: &'static [(u64, f64)]| move |id: u64| ids.iter().find(|(k, _)| *k == id).map_or(0., |(_, v)| *v);
        assert_eq!(severed_line_leak_m2(only(&[(ELEVATOR_SEPARATION, 1.)]), [false; 4]), [2. * SEVERED_LINE_M2, 0.]);
        assert_eq!(severed_line_leak_m2(only(&[(HSTAB_SEPARATION + 1, 1.)]), [false; 4]), [0., 4. * SEVERED_LINE_M2]);
        assert_eq!(severed_line_leak_m2(only(&[(AILERON_SEPARATION, 0.5)]), [false; 4]), [0.5 * SEVERED_LINE_M2, 0.5 * SEVERED_LINE_M2]);
        assert_eq!(severed_line_leak_m2(only(&[(SPOILER_SEPARATION, 1.), (SPOILER_SEPARATION + 1, 1.)]), [false; 4]), [SEVERED_LINE_M2, SEVERED_LINE_M2]);
        assert_eq!(severed_line_leak_m2(only(&[(FLAP_SEPARATION, 1.)]), [false; 4]), [0., 0.]);
        let engine3 = only(&[(crate::failures::extra::ENGINE_SEPARATION + 2, 1.)]);
        assert_eq!(severed_line_leak_m2(&engine3, [false; 4]), [0., 2. * SEVERED_LINE_M2]);
        assert_eq!(severed_line_leak_m2(&engine3, [false, false, true, false]), [0., 0.]);
    }

    #[test]
    fn every_separation_the_efb_offers_is_left_behind_on_reload() {
        let ids: std::collections::BTreeSet<u64> = failure_ids().into_iter().collect();
        assert_eq!(ids.len(), 53);
        for id in [54_000, 54_003, 54_010, 54_013, 55_300, 55_301, 55_310, 55_321, 55_330, 57_300, 57_325, 57_331, 57_340, 57_345, 57_350, 57_355] {
            assert!(ids.contains(&id), "{id}");
        }
    }

    #[test]
    fn a_lost_left_flap_drops_lift_and_rolls_left_only_with_the_flaps_out() {
        use crate::deep::integration::xp_consequences::{PlugForceAxis, RecordingForceSink};
        let total = |sink: &RecordingForceSink, axis: PlugForceAxis| sink.plug_forces.borrow().iter().filter(|(a, _)| *a == axis).map(|(_, v)| v).sum::<f64>();
        let mut flaps = [[false; 3]; 2];
        flaps[0][0] = true;
        let up = RecordingForceSink::default();
        flap_forces(&up, &flaps, [0., 0.], 6_000.);
        assert_eq!(total(&up, PlugForceAxis::Fnrml), 0.0);
        let out = RecordingForceSink::default();
        flap_forces(&out, &flaps, [1., 1.], 6_000.);
        assert!(total(&out, PlugForceAxis::Fnrml) < -50_000.0);
        assert!(total(&out, PlugForceAxis::Roll) < 0.0);
    }

    #[test]
    fn a_lost_stabiliser_takes_its_elevators_and_a_lost_fin_both_rudders() {
        let s = Separation::from(|id| id == HSTAB_SEPARATION + 1 || id == FIN_SEPARATION);
        assert_eq!(s.missing.elevators, [[false, false], [true, true]]);
        assert_eq!(s.missing.rudders, [true, true]);
        assert_eq!(s.missing.ailerons, [[false; 3]; 2]);
    }

    #[test]
    fn a_lost_wingtip_takes_the_middle_and_outer_ailerons_only() {
        let s = Separation::from(|id| id == WINGTIP_SEPARATION);
        assert_eq!(s.missing.ailerons[0], [false, true, true]);
        assert_eq!(s.missing.ailerons[1], [false; 3]);
    }

    #[test]
    fn a_wing_broken_between_the_engines_takes_everything_outboard() {
        let s = Separation::from(|id| id == WING_SEPARATION + 5);
        assert_eq!(s.wing, [None, Some(2)]);
        assert!(s.wingtip[1] && !s.wingtip[0]);
        assert_eq!(s.missing.ailerons[1], [true; 3]);
        assert_eq!(s.missing.spoilers[1], [false, false, false, false, false, false, true, true]);
        assert_eq!(s.flaps[1], [false, false, true]);
        assert_eq!(s.flap_torn, [[false; 3]; 2], "a flap gone with its wing loses no lift X-Plane has not already lost");
        let parts = s.parts();
        let on = |n: &str| parts.iter().any(|(k, v)| k == n && *v);
        assert!(on("wing3_r") && on("wingtip_r") && !on("wing2_r") && !on("wing3_l"));
    }

    #[test]
    fn a_wing_root_break_severs_every_line_out_to_the_tip() {
        let root = |id: u64| if id == WING_SEPARATION { 1.0 } else { 0.0 };
        assert_eq!(severed_line_leak_m2(root, [false; 4]), [7. * SEVERED_LINE_M2, 7. * SEVERED_LINE_M2]);
    }

    #[test]
    fn every_surface_has_a_dataref_the_model_hides_on() {
        let names: Vec<String> = Separation::default().flags().into_iter().map(|(n, _)| n).collect();
        for n in ["l_aileron1", "r_aileron3", "l_elevator_inner", "r_elevator_outer", "l_spoiler1", "r_spoiler8", "hstab_l", "rudder_upper", "rudder_lower", "l_flap_inner", "r_flap_outer"] {
            assert!(names.iter().any(|x| x == n), "{n} missing");
        }
        assert!(names.iter().any(|x| x == "cowl_4"));
        assert_eq!(names.len(), 2 * (3 + 2 + 8 + 1 + 3) + 2 + 4);
    }
}

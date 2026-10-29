use fms_lnav::common::PathVector;
use fms_lnav::geometry::{LegCalc, Transition};
use fms_lnav::{Coord, Geometry};
use fms_vnav::modes::FcuModes;
use fms_vnav::profile::{Checkpoint, Profile, Reason};

use crate::outputs::Sink;
use crate::symbols::{flags, pwp_flags};

const IDENT_TOC: &str = "(T/C)";
const IDENT_STEP_CLIMB: &str = "(S/C)";
const IDENT_STEP_DESCENT: &str = "(S/D)";
const IDENT_TOD: &str = "(T/D)";
const IDENT_DECEL: &str = "(DECEL)";
const IDENT_FLAP1: &str = "(FLAP1)";
const IDENT_FLAP2: &str = "(FLAP2)";
const IDENT_SPD_LIM: &str = "(SPDLIM)";
const APPROACH_PHASE_PULSE_MS: f64 = 5_000.;

const MCDU_REASONS: [Reason; 9] = [Reason::TopOfClimb, Reason::CrossingClimbSpeedLimit, Reason::StepClimb, Reason::StepDescent, Reason::TopOfDescent, Reason::CrossingDescentSpeedLimit, Reason::Decel, Reason::Flaps1, Reason::Flaps2];
const ND_REASONS: [Reason; 14] = [
    Reason::TopOfClimb,
    Reason::LevelOffForClimbConstraint,
    Reason::ContinueClimb,
    Reason::CrossingFcuAltitudeClimb,
    Reason::TopOfDescent,
    Reason::CrossingFcuAltitudeDescent,
    Reason::ContinueDescent,
    Reason::ContinueDescentArmed,
    Reason::LevelOffForDescentConstraint,
    Reason::InterceptDescentProfileManaged,
    Reason::InterceptDescentProfileSelected,
    Reason::Decel,
    Reason::Flaps1,
    Reason::Flaps2,
];
const BEFORE_FCU_ALT: [Reason; 4] = [Reason::LevelOffForClimbConstraint, Reason::ContinueClimb, Reason::CrossingClimbSpeedLimit, Reason::CrossingFcuAltitudeClimb];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sequencing {
    TodReached,
    ApproachPhaseAutoEngage,
}

/// FBW `PseudoWaypoint`.
#[derive(Clone, Debug)]
pub struct PseudoWaypoint {
    pub ident: String,
    pub sequencing: Option<Sequencing>,
    pub along_leg_index: Option<usize>,
    pub distance_from_leg_termination: Option<f64>,
    pub efis_symbol_flag: u32,
    pub efis_pwp_symbol_flag: Option<u32>,
    pub efis_symbol_lla: Option<Coord>,
    pub distance_from_start: f64,
    pub displayed_on_mcdu: bool,
    pub mcdu_ident: Option<String>,
    pub mcdu_header: Option<String>,
    pub predicted_altitude: Option<f64>,
    pub seconds_from_present: Option<f64>,
    pub displayed_on_nd: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct EfisState {
    pub mode: i32,
    pub range: f64,
}

pub struct PwpContext<'a> {
    pub geometry: Option<&'a Geometry>,
    pub calc: Option<&'a [Option<LegCalc>]>,
    pub has_active: bool,
    pub first_missed: usize,
    pub active_leg_index: usize,
    pub mcdu: Option<&'a Profile<'static>>,
    pub nd: Option<&'a Profile<'static>>,
    pub modes: FcuModes,
    pub next_speed_change: Option<f64>,
    pub airspeed_managed: bool,
    pub phase_preflight: bool,
    pub phase_descent: bool,
    pub efis: [EfisState; 2],
}

fn point_from_end(vectors: &[PathVector], distance_from_end: f64) -> Option<Coord> {
    let mut acc = 0.;
    for v in vectors.iter().rev() {
        let len = v.length();
        if distance_from_end <= acc + len || std::ptr::eq(v, &vectors[0]) {
            return Some(v.point((distance_from_end - acc).clamp(0., len)));
        }
        acc += len;
    }
    None
}

fn trim_from_start(vectors: Vec<PathVector>, keep_from_end: f64) -> Vec<PathVector> {
    let total: f64 = vectors.iter().map(|v| v.length()).sum();
    let mut skip = total - keep_from_end;
    let mut out = Vec::new();
    for v in vectors {
        if skip >= v.length() {
            skip -= v.length();
            continue;
        }
        out.push(v);
    }
    out
}

fn trim_from_end(vectors: Vec<PathVector>, keep_from_start: f64) -> Vec<PathVector> {
    let mut acc = 0.;
    let mut out = Vec::new();
    for v in vectors {
        if acc >= keep_from_start {
            break;
        }
        acc += v.length();
        out.push(v);
    }
    out
}

fn transition_path(t: Option<&Transition>) -> Vec<PathVector> {
    t.filter(|t| !t.is_null()).map(|t| t.path()).unwrap_or_default()
}

/// Location `distance_from_end_of_leg` before the end of leg `i`'s complete path (inbound
/// transition part, leg, outbound transition part), as FBW's `getPseudoWaypointLocation` calls do.
pub fn complete_leg_point(g: &Geometry, i: usize, distance_from_end_of_leg: f64) -> Option<Coord> {
    let [inbound, leg, outbound] = g.leg_lengths(i);
    let mut v = Vec::new();
    if i > 0 && inbound > 0. {
        v.extend(trim_from_start(transition_path(g.transition(i - 1)), inbound));
    }
    v.extend(g.leg_path(i).iter().copied());
    if outbound > 0. {
        v.extend(trim_from_end(transition_path(g.transition(i)), outbound));
    }
    if v.is_empty() {
        return None;
    }
    let _ = leg;
    point_from_end(&v, distance_from_end_of_leg)
}

/// FBW `Geometry.getLegOrientationAtDistanceFromEnd` over leg `i`'s complete path.
pub fn complete_leg_orientation(g: &Geometry, i: usize, distance_from_end_of_leg: f64) -> Option<f64> {
    let mut v = Vec::new();
    if i > 0 {
        v.extend(transition_path(g.transition(i - 1)));
    }
    v.extend(g.leg_path(i).iter().copied());
    v.extend(transition_path(g.transition(i)));
    let mut left = distance_from_end_of_leg;
    for p in v.iter().rev() {
        let len = p.length();
        if left > len {
            left -= len;
            continue;
        }
        return Some(fms_lnav::geo::bearing_to(p.point(left), p.point(left - 0.02)));
    }
    None
}

/// FBW `PseudoWaypoints`.
#[derive(Default)]
pub struct PseudoWaypoints {
    pub all: Vec<PseudoWaypoint>,
    pub current: Vec<PseudoWaypoint>,
    approach_pulse_ms: Option<f64>,
}

impl PseudoWaypoints {
    fn point_from_end_of_path(c: &PwpContext, g: &Geometry, calc: &[Option<LegCalc>], wpt_count: usize, distance_from_end: f64) -> Option<(Coord, f64, usize)> {
        if !(distance_from_end > 0.) {
            return None;
        }
        let start = c.active_leg_index.saturating_sub(1);
        for i in start..wpt_count.min(g.leg_count()) {
            if g.leg_path(i).is_empty() {
                continue;
            }
            let Some(lc) = calc.get(i).and_then(|c| c.as_ref()) else { continue };
            let acc = lc.cumulative_distance_to_end_with_transitions;
            if acc < distance_from_end {
                let [inb, leg, out] = g.leg_lengths(i);
                let d = (distance_from_end - acc).min(inb + leg + out);
                return complete_leg_point(g, i, d).map(|p| (p, d, i));
            }
        }
        None
    }

    fn from_checkpoint(c: &PwpContext, g: &Geometry, calc: &[Option<LegCalc>], wpt_count: usize, m: &Profile, cp: &Checkpoint) -> Option<PseudoWaypoint> {
        let lat_auto = c.modes.is_lat_auto_control_active() || c.modes.is_lat_auto_control_armed_with_intercept();
        let mut placed = (None, None, None);
        if lat_auto || MCDU_REASONS.contains(&cp.reason) {
            let (lla, d, i) = Self::point_from_end_of_path(c, g, calc, wpt_count, m.total_flight_plan_distance - cp.distance_from_start)?;
            placed = (Some(lla), Some(d), Some(i));
        }
        let pwp = |ident: &str, sym: u32, pwp_flag: Option<u32>, mcdu: bool, nd: bool, info: bool| PseudoWaypoint {
            ident: ident.to_string(),
            sequencing: None,
            along_leg_index: placed.2,
            distance_from_leg_termination: placed.1,
            efis_symbol_flag: sym,
            efis_pwp_symbol_flag: pwp_flag,
            efis_symbol_lla: placed.0,
            distance_from_start: cp.distance_from_start,
            displayed_on_mcdu: mcdu,
            mcdu_ident: None,
            mcdu_header: None,
            predicted_altitude: info.then_some(cp.altitude),
            seconds_from_present: info.then_some(cp.seconds_from_present),
            displayed_on_nd: nd,
        };
        let lat_active = c.modes.is_lat_auto_control_active();
        use pwp_flags::*;
        Some(match cp.reason {
            Reason::LevelOffForClimbConstraint => pwp("Level 1", flags::MAGENTA, Some(CLIMB_LEVEL_OFF), false, true, false),
            Reason::ContinueClimb => pwp("Start of climb 1", flags::CYAN, Some(START_OF_CLIMB), false, true, false),
            Reason::CrossingClimbSpeedLimit | Reason::CrossingDescentSpeedLimit => pwp(IDENT_SPD_LIM, 0, None, true, false, true),
            Reason::CrossingFcuAltitudeClimb => pwp("Level 2", flags::CYAN, Some(CLIMB_LEVEL_OFF), false, true, false),
            Reason::TopOfClimb => pwp(IDENT_TOC, 0, None, true, false, true),
            Reason::StepClimb => pwp(IDENT_STEP_CLIMB, 0, Some(START_OF_CLIMB), true, lat_active, true),
            Reason::StepDescent => PseudoWaypoint { mcdu_ident: Some(IDENT_STEP_DESCENT.into()), ..pwp(IDENT_TOD, 0, Some(TOP_OF_DESCENT), true, lat_active, true) },
            Reason::ContinueDescent => pwp(IDENT_TOD, 0, Some(TOP_OF_DESCENT), false, true, false),
            Reason::ContinueDescentArmed => pwp(IDENT_TOD, flags::CYAN, Some(TOP_OF_DESCENT), false, true, false),
            Reason::TopOfDescent => PseudoWaypoint { sequencing: Some(Sequencing::TodReached), ..pwp(IDENT_TOD, 0, Some(TOP_OF_DESCENT), true, lat_active || c.phase_preflight, true) },
            Reason::CrossingFcuAltitudeDescent => pwp("Level 2", flags::CYAN, Some(DESCENT_LEVEL_OFF), false, true, false),
            Reason::LevelOffForDescentConstraint => pwp("Level 1", flags::MAGENTA, Some(DESCENT_LEVEL_OFF), false, true, false),
            Reason::InterceptDescentProfileSelected => pwp("Intercept point 1", 0, Some(INTERCEPT_PROFILE), false, true, false),
            Reason::InterceptDescentProfileManaged => pwp("Intercept point 2", flags::CYAN, Some(INTERCEPT_PROFILE), false, true, false),
            Reason::Decel => PseudoWaypoint {
                sequencing: Some(Sequencing::ApproachPhaseAutoEngage),
                ..pwp(IDENT_DECEL, if c.airspeed_managed && lat_auto { flags::MAGENTA } else { 0 }, Some(DECEL), true, true, true)
            },
            Reason::Flaps1 => pwp(IDENT_FLAP1, 0, Some(CDA_FLAP1), true, true, true),
            Reason::Flaps2 => pwp(IDENT_FLAP2, 0, Some(CDA_FLAP2), true, true, true),
            _ => return None,
        })
    }

    fn end_of_vd_marker(c: &PwpContext, g: &Geometry, calc: &[Option<LegCalc>], side: usize) -> Option<PseudoWaypoint> {
        let e = c.efis[side];
        let active_calc = calc.get(c.active_leg_index).and_then(|c| c.as_ref())?;
        let range = match e.mode {
            2 => 10f64.max(e.range.min(160.)),
            0 => 5f64.max((e.range / 2.).min(160.)),
            _ => return None,
        };
        let distance_from_start = active_calc.cumulative_distance_with_transitions + range;
        let distance_from_end = active_calc.cumulative_distance_to_end_with_transitions - range;
        let total: f64 = g.all_path_vectors(None, false).iter().chain(g.all_path_vectors(None, true).iter()).map(|v| v.length()).sum();
        let last = calc.iter().rev().find_map(|c| c.as_ref()).filter(|_| calc.last().is_some_and(|l| l.is_some())).map_or(f64::INFINITY, |c| c.cumulative_distance_to_end_with_transitions);
        if distance_from_end > total || last > distance_from_end {
            return None;
        }
        let (lla, d, i) = Self::point_from_end_of_path(c, g, calc, g.leg_count(), distance_from_end)?;
        let side_flag = if side == 0 { flags::LEFT_SIDE_ONLY } else { flags::RIGHT_SIDE_ONLY };
        Some(PseudoWaypoint {
            ident: format!("END_OF_VD_{}", if side == 0 { "L" } else { "R" }),
            sequencing: None,
            along_leg_index: Some(i),
            distance_from_leg_termination: Some(d),
            efis_symbol_flag: flags::CYAN | side_flag,
            efis_pwp_symbol_flag: Some(pwp_flags::END_OF_VD_MARKER),
            efis_symbol_lla: Some(lla),
            distance_from_start,
            displayed_on_mcdu: false,
            mcdu_ident: None,
            mcdu_header: None,
            predicted_altitude: None,
            seconds_from_present: None,
            displayed_on_nd: true,
        })
    }

    /// FBW `PseudoWaypoints.recompute`.
    pub fn recompute(&mut self, c: &PwpContext) {
        self.all.clear();
        let (Some(g), Some(calc), Some(m)) = (c.geometry, c.calc, c.mcdu.filter(|m| m.is_ready_to_display)) else { return };
        if g.leg_count() < 1 || !c.has_active {
            return;
        }
        let wpt_count = c.first_missed;
        let nd_candidates: Vec<&Checkpoint> = c.nd.filter(|n| n.is_ready_to_display).map(|n| n.checkpoints.iter().filter(|cp| ND_REASONS.contains(&cp.reason)).collect()).unwrap_or_default();
        let mut left: Vec<Reason> = MCDU_REASONS.iter().chain(ND_REASONS.iter()).copied().collect();
        for cp in m.checkpoints.iter().filter(|cp| MCDU_REASONS.contains(&cp.reason)).chain(nd_candidates) {
            if !left.contains(&cp.reason) || matches!(cp.reason, Reason::Flaps1 | Reason::Flaps2) {
                continue;
            }
            if !left.contains(&Reason::CrossingFcuAltitudeClimb) && BEFORE_FCU_ALT.contains(&cp.reason) {
                continue;
            }
            left.retain(|r| *r != cp.reason);
            if let Some(p) = Self::from_checkpoint(c, g, calc, wpt_count, m, cp) {
                self.all.push(p);
            }
        }
        if let Some(first) = c.next_speed_change.filter(|d| d.is_finite()) {
            let placed = if c.modes.is_lat_auto_control_active() { Self::point_from_end_of_path(c, g, calc, wpt_count, m.total_flight_plan_distance - first) } else { None };
            self.all.push(PseudoWaypoint {
                ident: "Speed change".into(),
                sequencing: None,
                along_leg_index: placed.map(|p| p.2),
                distance_from_leg_termination: placed.map(|p| p.1),
                efis_symbol_flag: flags::MAGENTA,
                efis_pwp_symbol_flag: Some(pwp_flags::SPEED_CHANGE),
                efis_symbol_lla: placed.map(|p| p.0),
                distance_from_start: first,
                displayed_on_mcdu: false,
                mcdu_ident: None,
                mcdu_header: None,
                predicted_altitude: None,
                seconds_from_present: None,
                displayed_on_nd: true,
            });
        }
        for side in 0..2 {
            if let Some(p) = Self::end_of_vd_marker(c, g, calc, side) {
                self.all.push(p);
            }
        }
    }

    /// FBW `PseudoWaypoints.update`; `complete_dtg` is the active leg's complete leg path distance to go.
    pub fn update(&mut self, dt_ms: f64, c: &PwpContext, complete_dtg: Option<f64>, sink: &mut Sink) {
        if let Some(dtg) = complete_dtg {
            let due: Vec<Sequencing> = self
                .current
                .iter()
                .filter(|p| p.along_leg_index == Some(c.active_leg_index) || (c.active_leg_index > 0 && p.along_leg_index == Some(c.active_leg_index - 1)))
                .filter(|p| p.distance_from_leg_termination.is_some_and(|d| d >= dtg))
                .filter_map(|p| p.sequencing)
                .collect();
            for s in due {
                if s == Sequencing::ApproachPhaseAutoEngage && matches!(c.modes.lateral, 20 | 30 | 31) && self.approach_pulse_ms.is_none() && c.phase_descent {
                    sink.set_bool("A32NX_FM_ENABLE_APPROACH_PHASE", true);
                    self.approach_pulse_ms = Some(APPROACH_PHASE_PULSE_MS);
                }
            }
        }
        self.current.clear();
        let in_selected_hdg = !c.modes.is_lat_auto_control_active();
        for p in &self.all {
            let along = p.along_leg_index;
            let on_active = along == Some(c.active_leg_index);
            let after = along.is_some_and(|a| a > c.active_leg_index);
            let on_active_ok = on_active && matches!((complete_dtg, p.distance_from_leg_termination), (Some(dtg), Some(d)) if dtg > d);
            if in_selected_hdg || after || on_active_ok {
                self.current.push(p.clone());
            }
        }
        if let Some(t) = self.approach_pulse_ms.as_mut() {
            *t -= dt_ms;
            if *t <= 0. {
                self.approach_pulse_ms = None;
                sink.set_bool("A32NX_FM_ENABLE_APPROACH_PHASE", false);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(active_leg_index: usize) -> PwpContext<'static> {
        PwpContext {
            geometry: None,
            calc: None,
            has_active: true,
            first_missed: 99,
            active_leg_index,
            mcdu: None,
            nd: None,
            modes: FcuModes { lateral: 20, ..FcuModes::default() },
            next_speed_change: None,
            airspeed_managed: true,
            phase_preflight: false,
            phase_descent: true,
            efis: [EfisState { mode: 0, range: 10. }; 2],
        }
    }

    #[test]
    fn passing_decel_requests_the_approach_phase() {
        let decel = PseudoWaypoint {
            ident: IDENT_DECEL.into(),
            sequencing: Some(Sequencing::ApproachPhaseAutoEngage),
            along_leg_index: Some(5),
            distance_from_leg_termination: Some(10.),
            efis_symbol_flag: 0,
            efis_pwp_symbol_flag: None,
            efis_symbol_lla: None,
            distance_from_start: 0.,
            displayed_on_mcdu: true,
            mcdu_ident: None,
            mcdu_header: None,
            predicted_altitude: None,
            seconds_from_present: None,
            displayed_on_nd: true,
        };
        let mut pwps = PseudoWaypoints { all: vec![decel], ..Default::default() };
        let mut sink = Sink::default();
        pwps.update(50., &context(5), Some(10.2), &mut sink);
        assert_eq!(sink.written.get("A32NX_FM_ENABLE_APPROACH_PHASE"), None);
        pwps.update(50., &context(5), Some(9.9), &mut sink);
        assert_eq!(sink.written.get("A32NX_FM_ENABLE_APPROACH_PHASE"), Some(&1.));
    }

    #[test]
    fn passing_decel_outside_descent_does_not_pulse() {
        let decel = PseudoWaypoint {
            ident: IDENT_DECEL.into(),
            sequencing: Some(Sequencing::ApproachPhaseAutoEngage),
            along_leg_index: Some(5),
            distance_from_leg_termination: Some(10.),
            efis_symbol_flag: 0,
            efis_pwp_symbol_flag: None,
            efis_symbol_lla: None,
            distance_from_start: 0.,
            displayed_on_mcdu: true,
            mcdu_ident: None,
            mcdu_header: None,
            predicted_altitude: None,
            seconds_from_present: None,
            displayed_on_nd: true,
        };
        let mut pwps = PseudoWaypoints { all: vec![decel], ..Default::default() };
        let mut sink = Sink::default();
        let ctx = PwpContext { phase_descent: false, ..context(5) };
        pwps.update(50., &ctx, Some(10.2), &mut sink);
        pwps.update(50., &ctx, Some(9.9), &mut sink);
        assert_eq!(sink.written.get("A32NX_FM_ENABLE_APPROACH_PHASE"), None);
    }
}

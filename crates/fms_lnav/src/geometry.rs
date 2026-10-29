use crate::common::*;
use crate::geo::*;
use crate::hold::{EntryState, Environment, Hold, HoldEntry};
use crate::input::*;
use crate::picker::{pick, LegKind, TransitionKind};
use crate::pi::{Pi, PiPhase};
use crate::transitions::{ArcLeg, DirectToFix, DmeArc};

pub const DEFAULT_MIN_PREDICTED_TAS: f64 = 160.;
pub const TURN_RADIUS_FACTOR: f64 = 1.;
const G_RADIUS: f64 = 6997.84;
pub const VM_LEG_SIZE: f64 = 512.;

#[derive(Clone, Debug)]
struct LegState {
    input: LegInput,
    df_start: Option<Coord>,
    path: Vec<PathVector>,
    computed: bool,
    intercept: Option<Coord>,
    is_null: bool,
    ca_start: Option<Coord>,
    ca_end: Option<Coord>,
    moved_by_ppos: bool,
    hold: Option<Hold>,
    pi: Option<Pi>,
    af_sweep: f64,
}

impl LegState {
    fn new(input: &LegInput, next: Option<&LegInput>, env: &Environment) -> Self {
        let hold = match input.spec {
            LegSpec::Hold { kind, course, length, length_time, altitude } => Some(Hold::new(kind, input.fix, course, input.turn, length, length_time, altitude, env)),
            _ => None,
        };
        let pi = match (&input.spec, next.map(|n| (&n.spec, n.fix))) {
            (LegSpec::Pi { course }, Some((LegSpec::Cf { course: next_course, .. }, next_fix))) => Some(Pi::new(input.fix, *course, input.turn, *next_course, next_fix)),
            _ => None,
        };
        let intercept = match input.spec {
            LegSpec::Fd { course, distance, dme } => first_small_circle_intersection(dme, distance, input.fix, course),
            _ => None,
        };
        Self { input: input.clone(), df_start: None, path: Vec::new(), computed: matches!(input.spec, LegSpec::If) && input.airport_or_runway, intercept, is_null: input.is_null, ca_start: None, ca_end: None, moved_by_ppos: false, hold, pi, af_sweep: 0. }
    }
}

#[derive(Clone, Debug)]
pub struct PathCapture {
    pub itp: Coord,
    pub ftp: Coord,
    pub tad: f64,
    pub path: Vec<PathVector>,
    pub is_null: bool,
    pub distance: f64,
    pub computed: bool,
    pub computed_turn: TurnDirection,
    pub target_track: f64,
    pub forced_turn_complete: bool,
    pub start_with_tad: bool,
    pub frozen: bool,
}

impl PathCapture {
    fn new() -> Self {
        let z = Coord::new(0., 0.);
        Self {
            itp: z,
            ftp: z,
            tad: 0.,
            path: Vec::new(),
            is_null: false,
            distance: 0.,
            computed: false,
            computed_turn: TurnDirection::Either,
            target_track: 0.,
            forced_turn_complete: false,
            start_with_tad: false,
            frozen: false,
        }
    }

    fn end(&self, prev_fix: Coord) -> Coord {
        if self.is_null {
            prev_fix
        } else {
            self.path.last().map(|v| v.end()).unwrap_or(prev_fix)
        }
    }
}

#[derive(Clone, Debug)]
pub struct FixedRadius {
    pub radius: f64,
    pub tad: f64,
    pub clockwise: bool,
    pub sweep: f64,
    pub centre: Coord,
    pub turning_points: [Coord; 2],
    pub revert: Option<PathCapture>,
    pub computed: bool,
    pub frozen: bool,
}

impl FixedRadius {
    fn new() -> Self {
        let z = Coord::new(0., 0.);
        Self { radius: 0., tad: 0., clockwise: true, sweep: 0., centre: z, turning_points: [z, z], revert: None, computed: false, frozen: false }
    }

    pub fn is_reverted(&self) -> bool {
        self.revert.is_some()
    }

    fn start(&self) -> Coord {
        match &self.revert {
            Some(pc) => pc.itp,
            None => self.turning_points[0],
        }
    }

    fn end(&self, prev_fix: Coord) -> Coord {
        match &self.revert {
            Some(pc) => pc.end(prev_fix),
            None => self.turning_points[1],
        }
    }

    pub fn distance(&self) -> f64 {
        match &self.revert {
            Some(pc) => pc.distance,
            None => arc_length(self.radius, self.sweep),
        }
    }

    fn path(&self) -> Vec<PathVector> {
        match &self.revert {
            Some(pc) => pc.path.clone(),
            None => vec![PathVector::Arc { start: self.turning_points[0], centre: self.centre, end: self.turning_points[1], sweep: self.sweep }],
        }
    }
}

#[derive(Clone, Debug)]
pub struct CourseCapture {
    pub start: Coord,
    pub end: Coord,
    pub centre: Coord,
    pub sweep: f64,
    pub radius: f64,
    pub clockwise: bool,
    pub is_null: bool,
    pub computed: bool,
    pub computed_turn: TurnDirection,
    pub forced_turn_complete: bool,
}

impl CourseCapture {
    fn new() -> Self {
        let z = Coord::new(0., 0.);
        Self { start: z, end: z, centre: z, sweep: 0., radius: 0., clockwise: true, is_null: false, computed: false, computed_turn: TurnDirection::Either, forced_turn_complete: false }
    }

    fn end(&self, prev_fix: Coord) -> Coord {
        if self.is_null {
            prev_fix
        } else {
            self.end
        }
    }

    fn path(&self) -> Vec<PathVector> {
        if self.is_null {
            vec![PathVector::Line { start: self.start, end: self.end }]
        } else {
            vec![PathVector::Arc { start: self.start, centre: self.centre, end: self.end, sweep: self.sweep }]
        }
    }

    fn distance(&self) -> f64 {
        if self.is_null {
            0.
        } else {
            arc_length(self.radius, self.sweep)
        }
    }
}

fn recompute_course_capture(cc: &mut CourseCapture, prev: &LegCtx, next: &LegCtx, prev_end: Coord, tas: f64, gs: f64) {
    cc.computed_turn = TurnDirection::Either;
    let delta_track = (diff_angle(prev.outbound_course, next.inbound_course) * 10.).round() / 10.;
    let course_change = diff_angle_dir(0., delta_track, next.turn);
    let radius = gs * gs / (G * max_bank(tas, false).abs().to_radians().tan()) / G_RADIUS * TURN_RADIUS_FACTOR;
    let centre = place_bearing_distance(prev_end, prev.outbound_course + 90. * sign(course_change), radius);
    let end = place_bearing_distance(centre, prev.outbound_course - 90. * sign(course_change) + course_change, radius);
    cc.radius = radius;
    cc.clockwise = course_change >= 0.;
    if course_change == 0. {
        cc.start = prev_end;
        cc.end = prev_end;
        cc.is_null = true;
        cc.computed = true;
        return;
    }
    cc.computed_turn = if cc.clockwise { TurnDirection::Right } else { TurnDirection::Left };
    cc.is_null = false;
    cc.start = prev_end;
    cc.centre = centre;
    cc.end = end;
    cc.sweep = course_change;
    cc.computed = true;
}

#[derive(Clone, Debug)]
pub enum Transition {
    FixedRadius(FixedRadius),
    PathCapture(PathCapture),
    CourseCapture(CourseCapture),
    DirectToFix(DirectToFix),
    DmeArc(DmeArc),
    HoldEntry(HoldEntry),
}

impl Transition {
    fn computed(&self) -> bool {
        match self {
            Transition::FixedRadius(f) => f.computed,
            Transition::PathCapture(p) => p.computed,
            Transition::CourseCapture(c) => c.computed,
            Transition::DirectToFix(d) => d.computed,
            Transition::DmeArc(d) => d.computed,
            Transition::HoldEntry(h) => h.computed,
        }
    }

    pub fn is_null(&self) -> bool {
        match self {
            Transition::FixedRadius(_) | Transition::DmeArc(_) => false,
            Transition::PathCapture(p) => p.is_null,
            Transition::CourseCapture(c) => c.is_null,
            Transition::DirectToFix(d) => d.is_null,
            Transition::HoldEntry(h) => h.is_null,
        }
    }

    pub fn path(&self) -> Vec<PathVector> {
        match self {
            Transition::FixedRadius(f) => f.path(),
            Transition::PathCapture(p) => p.path.clone(),
            Transition::CourseCapture(c) => c.path(),
            Transition::DirectToFix(d) => d.path.clone(),
            Transition::DmeArc(d) => d.path(),
            Transition::HoldEntry(h) => h.path.clone(),
        }
    }

    pub fn distance(&self) -> f64 {
        match self {
            Transition::FixedRadius(f) => f.distance(),
            Transition::PathCapture(p) => p.distance,
            Transition::CourseCapture(c) => c.distance(),
            Transition::DirectToFix(d) => d.distance(),
            Transition::DmeArc(d) => d.distance(),
            Transition::HoldEntry(_) => 0.,
        }
    }
}

#[derive(Clone, Debug)]
struct LegCtx {
    fix: Coord,
    from: Option<Coord>,
    tf: bool,
    inbound_course: f64,
    outbound_course: f64,
    turn: TurnDirection,
    enroute: bool,
    overfly: bool,
    distance_to_termination: f64,
    overshot: bool,
    path_end: Coord,
    path_start: Option<Coord>,
    dtg: f64,
    course_reversal: bool,
}

fn distance_to_leg(from: Coord, next: &LegCtx) -> f64 {
    let oc = next.outbound_course;
    let i1 = place_bearing_intersection(from, normalise360(oc - 90.), next.path_end, normalise360(oc - 180.));
    let (d1, d2) = (distance_to(from, i1[0]), distance_to(from, i1[1]));
    let reference = if next.tf { next.from } else { next.path_start };
    let Some(reference) = reference else { return d1.min(d2) };
    let i2 = place_bearing_intersection(from, normalise360(oc - 90.), reference, normalise360(oc - 180.));
    d1.min(d2).min(distance_to(from, i2[0])).min(distance_to(from, i2[1]))
}

fn leg_intercept(from: Coord, bearing: f64, next: &LegCtx) -> Coord {
    let oc = next.outbound_course;
    let i1 = place_bearing_intersection(from, normalise360(bearing), next.fix, normalise360(oc - 180.));
    let (d1, d2) = (distance_to(from, i1[0]), distance_to(from, i1[1]));
    let Some(start) = next.path_start else { return if d1 > d2 { i1[1] } else { i1[0] } };
    let i2 = place_bearing_intersection(from, normalise360(bearing), start, normalise360(oc - 180.));
    let (d3, d4) = (distance_to(from, i2[0]), distance_to(from, i2[1]));
    let smallest = d1.min(d2).min(d3).min(d4);
    if smallest == d1 {
        i1[0]
    } else if smallest == d2 {
        i1[1]
    } else if smallest == d3 {
        i2[0]
    } else {
        i2[1]
    }
}

fn sign(x: f64) -> f64 {
    if x > 0. {
        1.
    } else if x < 0. {
        -1.
    } else {
        0.
    }
}

fn compare_turn_directions(sign: f64, data: TurnDirection) -> bool {
    if data != TurnDirection::Either && (sign == -1. || sign == 1.) {
        return (data == TurnDirection::Left && sign == -1.) || (data == TurnDirection::Right && sign == 1.);
    }
    true
}

fn direct_intercept(pc: &mut PathCapture, prev: &LegCtx, next: &LegCtx) {
    let prev_end = prev.fix;
    let intercept = leg_intercept(prev_end, prev.outbound_course, next);
    pc.itp = prev_end;
    pc.ftp = intercept;
    pc.path.push(PathVector::Line { start: prev_end, end: intercept });
    pc.distance = distance_to(prev_end, intercept);
}

fn recompute_path_capture(pc: &mut PathCapture, prev: &LegCtx, next: &LegCtx, tas: f64, gs: f64) {
    if pc.frozen {
        return;
    }
    let target_track = prev.outbound_course;
    let natural_sign = sign(diff_angle(target_track, next.inbound_course));
    pc.computed_turn = TurnDirection::Either;
    pc.target_track = next.inbound_course;

    let initial_turning_point;
    if pc.start_with_tad {
        pc.tad = max_tad(Some(tas)).min(prev.distance_to_termination - 0.05);
        initial_turning_point = match (prev.tf, prev.from) {
            (true, Some(start)) => {
                let length = distance_to(start, prev.fix);
                get_intermediate_point(start, prev.fix, (length - pc.tad) / length)
            }
            _ => place_bearing_distance(prev.fix, reciprocal(prev.outbound_course), pc.tad),
        };
    } else {
        pc.tad = 0.;
        initial_turning_point = prev.fix;
    }

    let distance_from_itp = distance_to_leg(initial_turning_point, next);
    let desired_turn = if next.course_reversal { TurnDirection::Either } else { next.turn };
    let delta_track = diff_angle_dir(target_track, next.inbound_course, desired_turn);
    pc.path.clear();

    if delta_track.abs() < 3. && distance_from_itp < 0.1 {
        pc.itp = prev.fix;
        pc.ftp = prev.fix;
        pc.path.push(PathVector::Line { start: prev.fix, end: prev.fix });
        pc.is_null = true;
        pc.distance = 0.;
        pc.computed = true;
        return;
    }
    pc.is_null = false;

    if delta_track.abs() > 42. && delta_track.abs() < 48. && distance_from_itp > 0.01 {
        direct_intercept(pc, prev, next);
        pc.computed = true;
        return;
    }

    let mut turn_direction = sign(delta_track);
    let radius = gs * gs / (G * max_bank(tas, true).to_radians().tan() * G_RADIUS) * TURN_RADIUS_FACTOR;
    let distance_limit = radius * 48f64.to_radians().cos();
    let tcd = |centre: Coord| sign(diff_angle(bearing_to(centre, next.path_end), next.outbound_course)) * distance_to_leg(centre, next);

    let mut turn_center = place_bearing_distance(initial_turning_point, target_track + turn_direction * 90., radius);
    let mut turn_center_distance = tcd(turn_center);

    if delta_track.abs() < 45. {
        if (delta_track > 0. && turn_center_distance >= radius) || (delta_track < 0. && turn_center_distance <= -radius) {
            turn_center = place_bearing_distance(initial_turning_point, target_track - turn_direction * 90., radius);
            turn_direction = -turn_direction;
            turn_center_distance = tcd(turn_center);
        }
    } else if !compare_turn_directions(turn_direction, next.turn) {
        turn_center = place_bearing_distance(initial_turning_point, target_track - turn_direction * 90., radius);
        turn_direction = -turn_direction;
        turn_center_distance = tcd(turn_center);
    }

    if distance_limit <= turn_center_distance.abs() && turn_center_distance.abs() < radius {
        let radius_to_leg = radius - turn_center_distance.abs();
        let intercept = if next.tf {
            small_circle_great_circle_intersection(turn_center, radius, next.from.unwrap_or(next.fix), next.outbound_course)
                .map(|[one, two]| if distance_to(initial_turning_point, one) > distance_to(initial_turning_point, two) { one } else { two })
        } else {
            first_small_circle_intersection(turn_center, radius, next.path_end, reciprocal(next.outbound_course))
        };
        if intercept.is_none() && radius_to_leg < 0.1 {
            direct_intercept(pc, prev, next);
            pc.computed = true;
            return;
        }
        if let Some(intercept) = intercept.filter(|c| c.is_finite()) {
            let bearing_tc_ftp = bearing_to(turn_center, intercept);
            let angle_to_leg = diff_angle(normalise360(bearing_tc_ftp - if turn_direction > 0. { -90. } else { 90. }), next.outbound_course);
            if angle_to_leg.abs() <= 48. {
                pc.itp = initial_turning_point;
                pc.ftp = intercept;
                pc.path.push(PathVector::Arc {
                    start: initial_turning_point,
                    end: intercept,
                    centre: turn_center,
                    sweep: delta_track.abs() * turn_direction,
                });
                pc.distance = arc_length(radius, delta_track.abs() * turn_direction);
                pc.computed = true;
                return;
            }
        }
    }

    let course_change;
    if delta_track.abs() < 45. {
        if (delta_track > 0. && turn_center_distance >= radius) || (delta_track < 0. && turn_center_distance <= -radius) {
            course_change = course_change::acute_far(turn_direction, turn_center_distance, delta_track);
        } else {
            course_change = course_change::acute_near(turn_direction, turn_center_distance, delta_track);
        }
    } else {
        let is_reverse = !compare_turn_directions(natural_sign, next.turn);
        if is_reverse {
            course_change = course_change::reverse(turn_direction, turn_center_distance, delta_track, radius);
            pc.computed_turn = next.turn;
        } else {
            course_change = course_change::normal(turn_direction, turn_center_distance, delta_track, radius);
            pc.computed_turn = if turn_direction < 0. { TurnDirection::Left } else { TurnDirection::Right };
        }
    }

    pc.target_track = (360. + prev.outbound_course + course_change) % 360.;
    let final_turning_point = place_bearing_distance(turn_center, target_track + course_change - 90. * turn_direction, radius);
    let course = normalise360(target_track + course_change);
    let intercept = if next.tf {
        let [one, two] = place_bearing_intersection(final_turning_point, course, next.from.unwrap_or(next.fix), next.outbound_course);
        if distance_to(final_turning_point, one) < distance_to(final_turning_point, two) {
            one
        } else {
            two
        }
    } else {
        leg_intercept(final_turning_point, target_track + course_change, next)
    };
    let overshot = side_of_point_on_course_to_fix(final_turning_point, target_track + course_change, intercept) == PointSide::Before;
    pc.itp = initial_turning_point;
    pc.ftp = final_turning_point;
    pc.computed = true;
    pc.path.push(PathVector::Arc { start: initial_turning_point, end: final_turning_point, centre: turn_center, sweep: course_change });
    if !overshot {
        pc.path.push(PathVector::Line { start: final_turning_point, end: intercept });
    }
    pc.distance = arc_length(radius, course_change) + if overshot { 0. } else { distance_to(final_turning_point, intercept) };
}

fn recompute_fixed_radius(fx: &mut FixedRadius, prev: &LegCtx, prev_overshot: bool, next: &LegCtx, next_distance: Option<f64>, is_active: bool, tas: f64, gs: f64) {
    if fx.frozen {
        return;
    }
    fx.sweep = diff_angle(prev.outbound_course, next.inbound_course);
    let bank = fx.sweep.abs() / 2.;
    let final_bank = bank.min(max_bank(tas, true)).max(min_bank(next.enroute));
    fx.radius = tas * tas / (9.81 * final_bank.to_radians().tan()) / G_RADIUS * TURN_RADIUS_FACTOR;
    fx.tad = fx.radius * (fx.sweep / 2.).abs().to_radians().tan();

    let prev_term_distance_to_next = distance_to_leg(prev.fix, next);
    let default_direction = if fx.sweep >= 0. { TurnDirection::Right } else { TurnDirection::Left };
    let forced_turn = next.turn != TurnDirection::Either && default_direction != next.turn;
    let too_big_for_previous = prev.distance_to_termination < fx.tad + 0.1;
    let too_big_for_next = next_distance.is_some_and(|d| d < fx.tad + 0.1);
    let not_lined_up = prev_term_distance_to_next.abs() >= 0.25;
    let should_revert = fx.sweep.abs() <= 3. || fx.sweep.abs() > 175. || prev.overfly || forced_turn || too_big_for_previous || too_big_for_next || not_lined_up;

    if should_revert && !prev_overshot {
        let should_have_tad = !prev.overfly && !not_lined_up && (too_big_for_previous || too_big_for_next);
        match fx.revert.as_mut() {
            None => {
                let mut reverted = PathCapture::new();
                reverted.start_with_tad = should_have_tad;
                recompute_path_capture(&mut reverted, prev, next, tas, gs);
                let fix_dtg = prev.dtg + fx.tad;
                if fix_dtg > reverted.tad {
                    fx.revert = Some(reverted);
                    fx.computed = true;
                    return;
                }
            }
            Some(pc) => {
                pc.start_with_tad = should_have_tad;
                recompute_path_capture(pc, prev, next, tas, gs);
                fx.computed = pc.computed;
                return;
            }
        }
    }

    if let Some(pc) = &fx.revert {
        let fix_dtg = prev.dtg + pc.tad;
        if fix_dtg > fx.tad + 0.05 || !is_active {
            fx.revert = None;
        }
    }

    fx.clockwise = fx.sweep >= 0.;
    let inbound = place_bearing_distance(prev.fix, reciprocal(prev.outbound_course), fx.tad);
    let outbound = place_bearing_distance(prev.fix, next.inbound_course, fx.tad);
    fx.centre = place_bearing_distance(inbound, normalise360(prev.outbound_course + if fx.clockwise { 90. } else { -90. }), fx.radius);
    fx.turning_points = [inbound, outbound];
    fx.computed = true;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guidable {
    Leg(usize),
    Transition(usize),
}

#[derive(Clone, Debug, Default)]
pub struct LegCalc {
    pub distance: f64,
    pub distance_with_transitions: f64,
    pub cumulative_distance: f64,
    pub cumulative_distance_with_transitions: f64,
    pub cumulative_distance_to_end: f64,
    pub cumulative_distance_to_end_with_transitions: f64,
}

pub struct Geometry {
    legs: Vec<Option<LegState>>,
    trans: Vec<Option<Transition>>,
    bridge: Vec<Option<Transition>>,
    bridge_out: Vec<Option<Transition>>,
    slots: Vec<bool>,
    kinds: Vec<Option<TransitionKind>>,
    ends: Vec<(usize, usize)>,
    prev_of: Vec<Option<usize>>,
    next_of: Vec<Option<usize>>,
    out_of: Vec<Option<usize>>,
    env: Environment,
    pub version: u32,
}

fn is_xi(spec: &LegSpec) -> bool {
    matches!(spec, LegSpec::Ci { .. } | LegSpec::Pi { .. })
}

fn is_xf(spec: &LegSpec) -> bool {
    matches!(spec, LegSpec::Af { .. } | LegSpec::Cf { .. } | LegSpec::If | LegSpec::Df | LegSpec::Rf { .. } | LegSpec::Tf { .. } | LegSpec::Hold { kind: HoldKind::Hf, .. })
}

fn is_xf_leg(spec: &LegSpec) -> bool {
    matches!(spec, LegSpec::Af { .. } | LegSpec::Cf { .. } | LegSpec::If | LegSpec::Df | LegSpec::Rf { .. } | LegSpec::Tf { .. } | LegSpec::Hold { .. })
}

fn is_course_reversal(spec: &LegSpec) -> bool {
    matches!(spec, LegSpec::Hold { .. } | LegSpec::Pi { .. })
}

fn af_parts(spec: &LegSpec, fix: Coord, turn: TurnDirection) -> Option<(ArcLeg, Coord, Coord, f64, f64)> {
    let LegSpec::Af { centre, boundary_radial, boundary_radial_coded, termination_radial } = *spec else { return None };
    let radius = distance_to(centre, fix);
    let turn_sign = if turn == TurnDirection::Left { -1. } else { 1. };
    let arc_start = place_bearing_distance(centre, boundary_radial, radius);
    let arc_end = place_bearing_distance(centre, termination_radial, radius);
    Some((ArcLeg { centre, radius, turn_sign, boundary_radial_coded }, arc_start, arc_end, normalise360(boundary_radial + 90. * turn_sign), normalise360(termination_radial + 90. * turn_sign)))
}

impl Geometry {
    pub fn new(elements: &[PlanElement]) -> Self {
        Self::build(elements, true, Environment::default())
    }

    pub fn with_environment(elements: &[PlanElement], env: Environment) -> Self {
        Self::build(elements, true, env)
    }

    pub fn engine_out(elements: &[PlanElement]) -> Self {
        Self::build(elements, false, Environment::default())
    }

    fn build(elements: &[PlanElement], transitions: bool, env: Environment) -> Self {
        let inputs: Vec<Option<LegInput>> = elements
            .iter()
            .map(|e| match e {
                PlanElement::Leg(l) => Some(l.clone()),
                PlanElement::Discontinuity => None,
            })
            .collect();
        let n = inputs.len();
        let spec = |i: isize| if i < 0 { None } else { inputs.get(i as usize).and_then(|l| l.as_ref()).map(|l| &l.spec) };
        let xi_if_xf = |i: isize| spec(i).is_some_and(is_xi) && matches!(spec(i + 1), Some(LegSpec::If)) && spec(i + 2).is_some_and(is_xf);
        let ci_if_xf = |i: isize| matches!(spec(i), Some(LegSpec::Ci { .. })) && matches!(spec(i + 1), Some(LegSpec::If)) && spec(i + 2).is_some_and(is_xf_leg);
        let legs: Vec<Option<LegState>> = (0..n)
            .map(|i| {
                let l = inputs[i].as_ref()?;
                let next = if xi_if_xf(i as isize) { inputs.get(i + 2) } else { inputs.get(i + 1) }.and_then(|n| n.as_ref());
                let mut s = LegState::new(l, next, &env);
                if xi_if_xf(i as isize - 1) {
                    s.is_null = true;
                }
                Some(s)
            })
            .collect();
        let mut prev_of = vec![None; n];
        let mut next_of = vec![None; n];
        let mut out_of = vec![None; n];
        for i in 0..n {
            if legs[i].is_none() {
                continue;
            }
            let ii = i as isize;
            prev_of[i] = if ci_if_xf(ii - 2) { Some(i - 2) } else if i > 0 && legs[i - 1].is_some() { Some(i - 1) } else { None };
            next_of[i] = if ci_if_xf(ii) { Some(i + 2) } else if i + 1 < n && legs[i + 1].is_some() { Some(i + 1) } else { None };
            out_of[i] = if ci_if_xf(ii) { Some(i + 1) } else if ci_if_xf(ii - 1) { None } else { Some(i) };
        }
        let mut slots = vec![false; n + 1];
        let mut kinds = vec![None; n + 1];
        let mut ends = (0..=n).map(|t| (t, t + 1)).collect::<Vec<_>>();
        let first_missed = legs.iter().position(|l| l.as_ref().is_some_and(|l| l.input.missed_approach)).unwrap_or(usize::MAX);
        if transitions {
            for i in 1..n {
                let Some(next) = &legs[i] else { continue };
                let p = if xi_if_xf(i as isize - 2) { i - 2 } else { i - 1 };
                let Some(prev) = &legs[p] else { continue };
                if next.input.missed_approach && i > first_missed {
                    continue;
                }
                let same_waypoint = prev.input.waypoint_id == next.input.waypoint_id;
                kinds[i - 1] = pick(prev.input.spec.kind(), next.input.spec.kind(), same_waypoint);
                slots[i - 1] = kinds[i - 1].is_some();
                ends[i - 1] = (p, i);
            }
        }
        Self { trans: vec![None; n + 1], bridge: vec![None; n], bridge_out: vec![None; n], legs, slots, kinds, ends, prev_of, next_of, out_of, env, version: 0 }
    }

    pub fn update_from_plan(&mut self, elements: &[PlanElement], active: usize) {
        let mut next = Self::build(elements, true, self.env);
        let repr = |l: &LegInput| format!("{}|{:?}|{:?}", l.ident, l.spec, l.fix);
        let keep: Vec<bool> = (0..next.legs.len())
            .map(|i| match (self.legs.get(i).and_then(|l| l.as_ref()), next.legs[i].as_ref()) {
                (Some(old), Some(new)) => i + 1 < active || repr(&old.input) == repr(&new.input),
                _ => false,
            })
            .collect();
        for i in 0..next.legs.len() {
            if !keep[i] {
                continue;
            }
            let mut old = self.legs[i].take().unwrap();
            let new = next.legs[i].as_ref().unwrap();
            old.input.missed_approach = new.input.missed_approach;
            old.input.turn = new.input.turn;
            old.input.overfly = new.input.overfly;
            old.input.enroute = new.input.enroute;
            old.is_null = old.is_null || new.is_null;
            next.legs[i] = Some(old);
        }
        for t in 0..next.legs.len().min(self.legs.len()) {
            let (p, n) = next.ends[t];
            let same = next.kinds[t].is_some() && next.kinds[t] == self.kinds[t] && self.ends[t] == (p, n);
            if same && keep[p] && n < keep.len() && keep[n] {
                next.trans[t] = self.trans[t].take();
            }
        }
        next.version = self.version + 1;
        *self = next;
    }

    pub fn set_environment(&mut self, env: Environment) {
        self.env = env;
    }

    pub fn set_predictions(&mut self, i: usize, tas: Option<f64>, gs: Option<f64>) {
        if let Some(Some(l)) = self.legs.get_mut(i) {
            l.input.predicted_tas = tas;
            l.input.predicted_gs = gs;
        }
    }

    pub fn set_hold_speed(&mut self, active: usize, tas: f64) {
        for i in [active, active + 1] {
            if let Some(Some(LegState { hold: Some(h), .. })) = self.legs.get_mut(i) {
                if h.kind == HoldKind::Hm {
                    h.next_tas = tas;
                    return;
                }
            }
        }
    }

    pub fn set_hold_immediate_exit(&mut self, i: usize, exit: bool, ppos: Coord, tas: f64) {
        let env = self.env;
        if let Some(Some(LegState { hold: Some(h), .. })) = self.legs.get_mut(i) {
            if h.kind == HoldKind::Hm {
                h.set_immediate_exit(exit, ppos, tas, &env);
            }
        }
    }

    pub fn leg_count(&self) -> usize {
        self.legs.len()
    }

    pub fn transition(&self, i: usize) -> Option<&Transition> {
        self.trans.get(i).and_then(|t| t.as_ref())
    }

    pub fn is_null(&self, i: usize) -> bool {
        self.legs.get(i).and_then(|l| l.as_ref()).is_some_and(|l| l.is_null || l.hold.as_ref().is_some_and(|h| h.is_null))
    }

    pub fn disable_automatic_sequencing(&self, i: usize) -> bool {
        match self.legs.get(i).and_then(|l| l.as_ref()) {
            Some(l) => match (&l.input.spec, &l.hold) {
                (LegSpec::Vm { .. }, _) => true,
                (_, Some(h)) => h.disable_automatic_sequencing(),
                _ => false,
            },
            None => false,
        }
    }

    fn leg(&self, i: isize) -> Option<&LegState> {
        if i < 0 {
            return None;
        }
        self.legs.get(i as usize).and_then(|l| l.as_ref())
    }

    fn spec(&self, i: usize) -> Option<&LegSpec> {
        self.legs.get(i).and_then(|l| l.as_ref()).map(|l| &l.input.spec)
    }

    fn out_slot(&self, i: usize) -> Option<usize> {
        self.out_of.get(i).copied().flatten()
    }

    fn inbound_trans(&self, i: usize) -> Option<&Transition> {
        if i == 0 {
            None
        } else if let Some(b) = self.active_bridge(i) {
            Some(b).filter(|t| t.computed())
        } else {
            self.trans[i - 1].as_ref().filter(|t| t.computed())
        }
    }

    fn active_bridge(&self, n: usize) -> Option<&Transition> {
        if n == 0 || !self.is_null(n - 1) {
            return None;
        }
        self.bridge.get(n).and_then(|b| b.as_ref())
    }

    fn outbound_trans(&self, i: usize) -> Option<&Transition> {
        let stored = self.out_slot(i).and_then(|t| self.trans[t].as_ref());
        let bridged = if stored.is_none() && i > 0 && self.is_null(i - 1) { self.bridge_out.get(i).and_then(|b| b.as_ref()) } else { None };
        stored.or(bridged).filter(|t| t.computed())
    }

    fn vm_start(&self, i: usize) -> Option<Coord> {
        self.inbound_end(i)
    }

    fn term_fix(&self, i: usize) -> Coord {
        match self.legs[i].as_ref() {
            Some(l) => match &l.input.spec {
                LegSpec::Ci { .. } => l.intercept.unwrap_or(l.input.fix),
                LegSpec::Fa { course, altitude, start_altitude } => place_bearing_distance(l.input.fix, *course, ((altitude - start_altitude).max(0.) / 500.).max(0.1)),
                LegSpec::Fc { course, length } => place_bearing_distance(l.input.fix, *course, *length),
                LegSpec::Fm { course } => place_bearing_distance(l.input.fix, *course, 512.),
                LegSpec::Ca { .. } => l.ca_end.unwrap_or(l.input.fix),
                LegSpec::Cr { .. } | LegSpec::Cd { .. } | LegSpec::Fd { .. } => l.intercept.unwrap_or(l.input.fix),
                LegSpec::Vm { heading } => self.vm_start(i).map(|s| place_bearing_distance(s, *heading, VM_LEG_SIZE)).unwrap_or(Coord::new(f64::NAN, f64::NAN)),
                LegSpec::Af { .. } => af_parts(&l.input.spec, l.input.fix, l.input.turn).map(|p| p.2).unwrap_or(l.input.fix),
                LegSpec::Pi { .. } => l.pi.as_ref().map(|p| p.intercept.ftp).unwrap_or(l.input.fix),
                _ => l.input.fix,
            },
            None => Coord::new(0., 0.),
        }
    }

    fn trans_end(&self, t: usize) -> Coord {
        let (p, n, tr) = match self.active_bridge(t + 1) {
            Some(b) if t + 1 < self.legs.len() => (self.prev_of[t].unwrap_or(t), t + 1, Some(b)),
            _ => (self.ends[t].0, self.ends[t].1, self.trans[t].as_ref()),
        };
        let prev_fix = self.term_fix(p);
        match tr {
            Some(Transition::FixedRadius(f)) => f.end(prev_fix),
            Some(Transition::PathCapture(pc)) => pc.end(prev_fix),
            Some(Transition::CourseCapture(c)) => c.end(prev_fix),
            Some(Transition::DirectToFix(d)) => d.end(self.leg_end(p)),
            Some(Transition::DmeArc(d)) => d.ftp,
            Some(Transition::HoldEntry(h)) => match self.legs[n].as_ref().and_then(|l| l.hold.as_ref()) {
                Some(hold) => h.turning_points(hold)[1],
                None => prev_fix,
            },
            None => prev_fix,
        }
    }

    fn inbound_end(&self, i: usize) -> Option<Coord> {
        if i == 0 {
            return None;
        }
        if self.inbound_trans(i).is_some() {
            return Some(self.trans_end(i - 1));
        }
        if self.slots[i - 1] {
            return None;
        }
        let p = self.prev_of[i]?;
        let prev = self.leg(p as isize)?;
        if prev.computed {
            Some(self.leg_end(p))
        } else {
            None
        }
    }

    pub fn leg_end(&self, i: usize) -> Coord {
        let l = self.legs[i].as_ref().unwrap();
        match (&l.input.spec, self.outbound_trans(i)) {
            (LegSpec::Rf { .. }, _) | (LegSpec::Hold { .. }, _) => l.input.fix,
            (spec, Some(Transition::DmeArc(d))) if is_xf_leg(spec) => d.itp,
            (_, Some(Transition::FixedRadius(f))) => f.start(),
            _ => self.term_fix(i),
        }
    }

    fn estimate_df_start(&self, i: usize) -> Coord {
        let next_course = self.next_of[i].and_then(|n| self.inbound_course(n));
        let bearing = normalise360(next_course.map(|c| c + 180.).unwrap_or(180.));
        place_bearing_distance(self.legs[i].as_ref().unwrap().input.fix, bearing, 2.)
    }

    fn estimate_cf_start(&self, i: usize) -> Coord {
        let l = self.legs[i].as_ref().unwrap();
        let LegSpec::Cf { course, length } = l.input.spec else { unreachable!() };
        let inverse = normalise360(course + 180.);
        if let Some(p) = self.prev_of[i] {
            if self.leg(p as isize).is_some_and(|prev| prev.computed) && !self.slots[i - 1] {
                let prev_term = self.leg_end(p);
                return place_bearing_intersection(self.leg_end(i), inverse, prev_term, normalise360(inverse + 90.))[0];
            }
        }
        place_bearing_distance(l.input.fix, inverse, length)
    }

    pub fn leg_start(&self, i: usize) -> Option<Coord> {
        let l = self.legs[i].as_ref()?;
        match &l.input.spec {
            LegSpec::If | LegSpec::Hold { .. } => Some(l.input.fix),
            LegSpec::Tf { from } => Some(self.inbound_end(i).unwrap_or(*from)),
            LegSpec::Rf { from, .. } => Some(*from),
            LegSpec::Df => Some(self.inbound_end(i).unwrap_or_else(|| self.estimate_df_start(i))),
            LegSpec::Cf { .. } => match (self.inbound_trans(i), self.outbound_trans(i)) {
                (Some(_), _) => Some(self.trans_end(i - 1)),
                (None, Some(Transition::DmeArc(d))) => Some(d.itp),
                _ => Some(self.estimate_cf_start(i)),
            },
            LegSpec::Ci { .. } | LegSpec::Ca { .. } | LegSpec::Cr { .. } | LegSpec::Vm { .. } => self.inbound_end(i),
            LegSpec::Fa { .. } | LegSpec::Fc { .. } | LegSpec::Fm { .. } | LegSpec::Fd { .. } | LegSpec::Pi { .. } => Some(self.inbound_end(i).unwrap_or(l.input.fix)),
            LegSpec::Cd { dme, .. } => Some(self.inbound_end(i).unwrap_or(*dme)),
            LegSpec::Af { .. } => match self.trans.get(i.wrapping_sub(1)).and_then(|t| t.as_ref()) {
                Some(Transition::DmeArc(d)) if i > 0 => Some(d.ftp),
                _ => af_parts(&l.input.spec, l.input.fix, l.input.turn).map(|p| p.1),
            },
        }
    }

    fn rf_params(&self, i: usize) -> Option<(Coord, f64, bool, f64)> {
        let l = self.legs[i].as_ref()?;
        let LegSpec::Rf { from, center } = l.input.spec else { return None };
        let radius = distance_to(center, l.input.fix);
        let from_b = bearing_to(center, from);
        let to_b = bearing_to(center, l.input.fix);
        let (clockwise, angle) = match l.input.turn {
            TurnDirection::Left => (false, normalise360(from_b - to_b)),
            TurnDirection::Right => (true, normalise360(to_b - from_b)),
            TurnDirection::Either => {
                let a = diff_angle(to_b, from_b);
                (a > 0., a.abs())
            }
        };
        Some((center, radius, clockwise, angle))
    }

    pub fn inbound_course(&self, i: usize) -> Option<f64> {
        let l = self.legs[i].as_ref()?;
        match &l.input.spec {
            LegSpec::If => None,
            LegSpec::Tf { from } => Some(bearing_to(*from, l.input.fix)),
            LegSpec::Cf { course, .. } | LegSpec::Ci { course } | LegSpec::Fa { course, .. } | LegSpec::Fc { course, .. } | LegSpec::Fm { course } | LegSpec::Ca { course, .. } | LegSpec::Cr { course, .. } => Some(*course),
            LegSpec::Cd { course, .. } | LegSpec::Fd { course, .. } | LegSpec::Hold { course, .. } => Some(*course),
            LegSpec::Vm { heading } => Some(*heading),
            LegSpec::Df => l.df_start.map(|s| bearing_to(s, l.input.fix)),
            LegSpec::Rf { from, center } => {
                let (_, _, cw, _) = self.rf_params(i)?;
                Some(normalise360(bearing_to(*center, *from) + if cw { 90. } else { -90. }))
            }
            LegSpec::Af { .. } => af_parts(&l.input.spec, l.input.fix, l.input.turn).map(|p| p.3),
            LegSpec::Pi { .. } => Some(l.pi.as_ref().map(|p| p.straight.course).unwrap_or(0.)),
        }
    }

    pub fn outbound_course(&self, i: usize) -> Option<f64> {
        let l = self.legs[i].as_ref()?;
        match &l.input.spec {
            LegSpec::Rf { center, .. } => {
                let (_, _, cw, _) = self.rf_params(i)?;
                Some(normalise360(bearing_to(*center, l.input.fix) + if cw { 90. } else { -90. }))
            }
            LegSpec::Af { .. } => af_parts(&l.input.spec, l.input.fix, l.input.turn).map(|p| p.4),
            LegSpec::Pi { .. } => Some(match self.next_of[i].and_then(|n| self.spec(n)) {
                Some(LegSpec::Cf { course, .. }) => *course,
                _ => 0.,
            }),
            _ => self.inbound_course(i),
        }
    }

    pub fn overshot(&self, i: usize) -> bool {
        let Some(l) = self.legs[i].as_ref() else { return false };
        if !matches!(l.input.spec, LegSpec::Tf { .. } | LegSpec::Cf { .. } | LegSpec::Df | LegSpec::Af { .. }) {
            return false;
        }
        let (Some(oc), Some(start)) = (self.outbound_course(i), self.leg_start(i)) else { return false };
        side_of_point_on_course_to_fix(l.input.fix, oc, start) == PointSide::After
    }

    pub fn leg_distance(&self, i: usize) -> f64 {
        let Some(l) = self.legs[i].as_ref() else { return 0. };
        match l.input.spec {
            LegSpec::If | LegSpec::Hold { .. } | LegSpec::Vm { .. } => 0.,
            LegSpec::Ci { .. } | LegSpec::Cr { .. } if l.is_null => 0.,
            LegSpec::Fm { .. } => 0.,
            LegSpec::Rf { .. } => self.rf_params(i).map(|(_, r, _, a)| arc_length(r, a)).unwrap_or(0.),
            LegSpec::Pi { .. } => l.pi.as_ref().map(|p| p.distance()).unwrap_or(0.),
            _ => {
                if self.overshot(i) {
                    0.
                } else {
                    self.leg_start(i).map(|s| distance_to(s, self.leg_end(i))).filter(|d| d.is_finite()).unwrap_or(0.)
                }
            }
        }
    }

    fn distance_to_termination(&self, i: usize) -> f64 {
        let Some(l) = self.legs[i].as_ref() else { return 0. };
        match l.input.spec {
            LegSpec::If | LegSpec::Hold { .. } => 0.,
            LegSpec::Fm { .. } | LegSpec::Vm { .. } => 1.,
            LegSpec::Rf { .. } => self.rf_params(i).map(|(_, r, _, a)| arc_length(r, a)).unwrap_or(0.),
            LegSpec::Pi { .. } => l.pi.as_ref().map(|p| p.straight.length).unwrap_or(0.),
            LegSpec::Af { .. } => {
                if self.overshot(i) {
                    0.
                } else {
                    self.leg_start(i).map(|s| distance_to(s, l.input.fix)).unwrap_or(0.)
                }
            }
            _ => {
                if self.overshot(i) {
                    0.
                } else {
                    self.leg_start(i).map(|s| distance_to(s, self.term_fix(i))).unwrap_or(0.)
                }
            }
        }
    }

    fn hold_dtg(&self, i: usize, ppos: Coord) -> f64 {
        let l = self.legs[i].as_ref().unwrap();
        let h = l.hold.as_ref().unwrap();
        match h.kind {
            HoldKind::Hm => h.distance_to_go_this_orbit(ppos, &self.env),
            HoldKind::Ha => {
                if h.is_null {
                    0.
                } else if h.term_met {
                    h.distance_to_go_this_orbit(ppos, &self.env)
                } else {
                    h.geometry.leg_length * 2. + h.geometry.radius * std::f64::consts::PI * 2.
                }
            }
            HoldKind::Hf => match self.trans.get(i.wrapping_sub(1)).and_then(|t| t.as_ref()) {
                Some(Transition::HoldEntry(e)) if i > 0 => e.distance_to_go(),
                _ => 0.,
            },
        }
    }

    pub fn leg_distance_to_go(&self, i: usize, ppos: Coord) -> Option<f64> {
        let l = self.legs[i].as_ref()?;
        match &l.input.spec {
            LegSpec::If | LegSpec::Vm { .. } => None,
            LegSpec::Rf { from, .. } => {
                let (center, _, cw, angle) = self.rf_params(i)?;
                Some(arc_distance_to_go(ppos, *from, center, if cw { angle } else { -angle }))
            }
            LegSpec::Ci { .. } if l.is_null => Some(0.),
            LegSpec::Fm { .. } => None,
            LegSpec::Ca { course, .. } => Some(course_to_fix_distance_to_go(ppos, *course, l.ca_end?)),
            LegSpec::Cr { course, .. } | LegSpec::Cd { course, .. } | LegSpec::Fd { course, .. } => Some(course_to_fix_distance_to_go(ppos, *course, l.intercept?)),
            LegSpec::Af { centre, .. } => Some(arc_distance_to_go(ppos, self.leg_start(i)?, *centre, l.af_sweep)),
            LegSpec::Pi { .. } => Some(l.pi.as_ref()?.distance_to_go(ppos)),
            LegSpec::Hold { .. } => Some(self.hold_dtg(i, ppos)),
            _ => Some(course_to_fix_distance_to_go(ppos, self.outbound_course(i)?, self.leg_end(i))),
        }
    }

    fn ctx(&self, i: usize, ppos: Coord) -> LegCtx {
        let l = self.legs[i].as_ref().unwrap();
        LegCtx {
            fix: self.term_fix(i),
            from: match l.input.spec {
                LegSpec::Tf { from } | LegSpec::Rf { from, .. } => Some(from),
                _ => None,
            },
            tf: matches!(l.input.spec, LegSpec::Tf { .. }),
            inbound_course: self.inbound_course(i).unwrap_or(f64::NAN),
            outbound_course: self.outbound_course(i).unwrap_or(f64::NAN),
            turn: l.input.turn,
            enroute: l.input.enroute,
            overfly: l.input.overfly,
            distance_to_termination: self.distance_to_termination(i),
            overshot: self.overshot(i),
            path_end: self.leg_end(i),
            path_start: self.leg_start(i),
            dtg: self.leg_distance_to_go(i, ppos).unwrap_or(f64::INFINITY),
            course_reversal: is_course_reversal(&l.input.spec),
        }
    }

    fn standalone_ctx(&self, i: usize, ppos: Coord) -> LegCtx {
        let mut c = self.ctx(i, ppos);
        let l = self.legs[i].as_ref().unwrap();
        c.path_end = c.fix;
        c.path_start = match &l.input.spec {
            LegSpec::Cf { course, length } => Some(place_bearing_distance(l.input.fix, normalise360(course + 180.), *length)),
            LegSpec::Tf { from } | LegSpec::Rf { from, .. } => Some(*from),
            LegSpec::If | LegSpec::Fa { .. } | LegSpec::Fc { .. } | LegSpec::Fm { .. } | LegSpec::Fd { .. } => Some(l.input.fix),
            _ => None,
        };
        c
    }

    fn leg_tas(&self, i: usize, current: f64) -> f64 {
        self.legs[i].as_ref().and_then(|l| l.input.predicted_tas).unwrap_or(current).max(DEFAULT_MIN_PREDICTED_TAS)
    }

    fn leg_gs(&self, i: usize, current: f64) -> f64 {
        self.legs[i].as_ref().and_then(|l| l.input.predicted_gs).unwrap_or(current).max(DEFAULT_MIN_PREDICTED_TAS)
    }

    fn hold_entry_info(&self, i: usize) -> Option<(bool, EntryState)> {
        match self.trans.get(i.wrapping_sub(1)).and_then(|t| t.as_ref()) {
            Some(Transition::HoldEntry(e)) if i > 0 => Some((e.is_null, e.state)),
            _ => None,
        }
    }

    fn recompute_leg_path(&mut self, i: usize, is_active: bool, ppos: Coord, tas: f64, gs: f64) {
        self.recompute_ca(i, is_active, ppos);
        let start = self.leg_start(i);
        let end = self.leg_end(i);
        let overshot = self.overshot(i);
        let df_start = match self.legs[i].as_ref().map(|l| &l.input.spec) {
            Some(LegSpec::Df) => Some(self.inbound_end(i).unwrap_or_else(|| self.estimate_df_start(i))),
            _ => None,
        };
        let rf = self.rf_params(i);
        let ci = self.compute_ci_intercept(i, start);
        let entry = self.hold_entry_info(i);
        let pi_next = self.next_of[i].and_then(|n| self.legs[n].as_ref()).and_then(|n| match n.input.spec {
            LegSpec::Cf { course, .. } => Some((course, n.input.fix)),
            _ => None,
        });
        let af_path_start = match self.trans.get(i.wrapping_sub(1)).and_then(|t| t.as_ref()) {
            Some(Transition::PathCapture(_)) if i > 0 => Some(self.trans_end(i - 1)),
            _ => start,
        };
        let pi_start = self.inbound_end(i);
        let env = self.env;
        let l = self.legs[i].as_mut().unwrap();
        if let Some((intercept, is_null)) = ci {
            l.intercept = intercept;
            l.is_null = is_null;
        }
        l.path = match &l.input.spec {
            LegSpec::If => Vec::new(),
            LegSpec::Tf { .. } | LegSpec::Cf { .. } => {
                let s = start.unwrap();
                vec![PathVector::Line { start: if overshot { end } else { s }, end }]
            }
            LegSpec::Df => {
                l.df_start = df_start;
                vec![PathVector::Line { start: df_start.unwrap(), end }]
            }
            LegSpec::Rf { from, .. } => {
                let (center, _, cw, angle) = rf.unwrap();
                vec![PathVector::Arc { start: *from, centre: center, end: l.input.fix, sweep: if cw { angle } else { -angle } }]
            }
            LegSpec::Ci { .. } => Vec::new(),
            LegSpec::Fa { .. } | LegSpec::Fc { .. } | LegSpec::Fm { .. } => vec![PathVector::Line { start: start.unwrap_or(l.input.fix), end }],
            LegSpec::Cr { course, navaid, theta } => {
                let start = start.filter(|s| s.is_finite());
                let intercept = start.map(|s| place_bearing_intersection(s, *course, *navaid, *theta)[0]);
                l.intercept = intercept;
                match (start, intercept) {
                    (Some(s), Some(x)) if distance_to(s, x) < 5000. => {
                        l.is_null = false;
                        vec![PathVector::Line { start: s, end: x }]
                    }
                    _ => {
                        l.is_null = true;
                        Vec::new()
                    }
                }
            }
            LegSpec::Ca { .. } => match (l.ca_start, l.ca_end) {
                (Some(s), Some(e)) => vec![PathVector::Line { start: s, end: e }],
                _ => Vec::new(),
            },
            LegSpec::Vm { heading } => match start {
                Some(s) => vec![PathVector::Line { start: s, end: place_bearing_distance(s, *heading, VM_LEG_SIZE) }],
                None => Vec::new(),
            },
            LegSpec::Cd { course, distance, dme } => {
                let s = start.unwrap_or(*dme);
                l.intercept = first_small_circle_intersection(*dme, *distance, s, *course);
                match l.intercept {
                    Some(x) => vec![PathVector::Line { start: s, end: x }],
                    None => Vec::new(),
                }
            }
            LegSpec::Fd { .. } => match l.intercept {
                Some(x) => vec![PathVector::Line { start: start.unwrap_or(l.input.fix), end: x }],
                None => Vec::new(),
            },
            LegSpec::Af { centre, .. } => {
                let s = start.unwrap_or(l.input.fix);
                l.af_sweep = diff_angle(bearing_to(*centre, s), bearing_to(*centre, end));
                vec![PathVector::Arc { start: af_path_start.unwrap_or(s), centre: *centre, end, sweep: l.af_sweep }]
            }
            LegSpec::Pi { .. } => match (l.pi.as_mut(), pi_next) {
                (Some(p), Some((course, fix))) => {
                    p.recompute(is_active, tas, gs, course, fix);
                    p.path(pi_start.unwrap_or(l.input.fix))
                }
                _ => Vec::new(),
            },
            LegSpec::Hold { .. } => match l.hold.as_mut() {
                Some(h) => {
                    h.recompute(is_active, tas, entry, &env);
                    h.predicted_path()
                }
                None => Vec::new(),
            },
        };
        let is_ci = matches!(l.input.spec, LegSpec::Ci { .. });
        let ca_pending = matches!(l.input.spec, LegSpec::Ca { .. }) && l.ca_end.is_none();
        let cr_pending = matches!(l.input.spec, LegSpec::Cr { .. }) && start.is_none();
        l.computed = (!is_ci || ci.is_some()) && !ca_pending && !cr_pending;
        if is_ci && ci.is_some() && !l.is_null {
            let (s, e) = (start.unwrap(), self.leg_end(i));
            self.legs[i].as_mut().unwrap().path = vec![PathVector::Line { start: s, end: e }];
        }
    }

    fn recompute_ca(&mut self, i: usize, is_active: bool, ppos: Coord) {
        let inbound = self.inbound_end(i);
        let Some(l) = self.legs[i].as_mut() else { return };
        let LegSpec::Ca { course, altitude, after_runway } = l.input.spec else { return };
        let estimate = |start: Coord| place_bearing_distance(start, course, altitude / 2000. / 60. * 175.);
        if is_active && !after_runway {
            l.moved_by_ppos = true;
            l.ca_start = Some(ppos);
            if l.ca_end.is_none() {
                l.ca_end = Some(estimate(ppos));
            }
        } else if !l.moved_by_ppos {
            if let Some(s) = inbound {
                l.ca_start = Some(s);
            }
            l.ca_end = l.ca_start.map(estimate);
        }
    }

    fn compute_ci_intercept(&self, i: usize, start: Option<Coord>) -> Option<(Option<Coord>, bool)> {
        let LegSpec::Ci { course } = self.legs[i].as_ref()?.input.spec else { return None };
        let start = start?;
        let Some(n) = self.next_of[i] else { return Some((None, true)) };
        let next = self.standalone_ctx(n, start);
        if !next.outbound_course.is_finite() || !next.path_end.is_finite() {
            return Some((None, true));
        }
        let mut intercept = leg_intercept(start, course, &next);
        let flipped = side_of_point_on_course_to_fix(intercept, course, start) == PointSide::After;
        if !intercept.is_finite() || flipped {
            return Some((None, true));
        }
        if side_of_point_on_course_to_fix(next.path_end, next.outbound_course, intercept) == PointSide::After {
            let [one, two] = place_bearing_intersection(intercept, reciprocal(course), next.path_end, normalise360(next.outbound_course + 90.));
            intercept = if distance_to(intercept, one) < distance_to(intercept, two) { one } else { two };
        }
        Some((Some(intercept), false))
    }

    fn recompute_transition(&mut self, t: usize, is_active: bool, tas: f64, gs: f64, ppos: Coord) {
        if !self.slots[t] {
            return;
        }
        let (p, n) = self.ends[t];
        let prev = self.ctx(p, ppos);
        let next = self.ctx(n, ppos);
        let prev_overshot = prev.overshot;
        let next_distance = match self.legs[n].as_ref().map(|l| &l.input.spec) {
            Some(LegSpec::Tf { from }) => Some(distance_to(*from, next.fix)),
            _ => None,
        };
        match self.kinds[t] {
            Some(TransitionKind::CourseCapture) => {
                let mut cc = match self.trans[t].take() {
                    Some(Transition::CourseCapture(c)) => c,
                    _ => CourseCapture::new(),
                };
                let prev_end = self.leg_end(p);
                recompute_course_capture(&mut cc, &prev, &next, prev_end, tas, gs);
                self.trans[t] = Some(Transition::CourseCapture(cc));
            }
            Some(TransitionKind::PathCapture) => {
                let mut pc = match self.trans[t].take() {
                    Some(Transition::PathCapture(p)) => p,
                    _ => PathCapture::new(),
                };
                recompute_path_capture(&mut pc, &prev, &next, tas, gs);
                self.trans[t] = Some(Transition::PathCapture(pc));
            }
            Some(TransitionKind::DirectToFix) => {
                let prev_end = self.leg_end(p);
                let next_input = self.legs[n].as_ref().unwrap().input.clone();
                let bank = self.env.bank;
                let mut d = match self.trans[t].take() {
                    Some(Transition::DirectToFix(d)) => d,
                    _ => DirectToFix::new(),
                };
                d.recompute(is_active, tas, gs, prev_end, prev.outbound_course, next_input.fix, next_input.turn, bank);
                self.trans[t] = Some(Transition::DirectToFix(d));
            }
            Some(TransitionKind::DmeArc) => {
                let mut d = match self.trans[t].take() {
                    Some(Transition::DmeArc(d)) => d,
                    _ => DmeArc::new(),
                };
                let pl = self.legs[p].as_ref().unwrap();
                let nl = self.legs[n].as_ref().unwrap();
                if let Some((arc, ..)) = af_parts(&pl.input.spec, pl.input.fix, pl.input.turn) {
                    let next_start = self.leg_start(n).unwrap_or(nl.input.fix);
                    d.from_arc(tas, gs, &arc, prev.outbound_course, next.inbound_course, next_start);
                } else if let Some((arc, ..)) = af_parts(&nl.input.spec, nl.input.fix, nl.input.turn) {
                    let prev_is_xf = is_xf_leg(&pl.input.spec);
                    d.to_arc(tas, gs, self.leg_end(p), prev.outbound_course, prev_is_xf, &arc, next.inbound_course);
                }
                self.trans[t] = Some(Transition::DmeArc(d));
            }
            Some(TransitionKind::HoldEntry) => {
                let mut e = match self.trans[t].take() {
                    Some(Transition::HoldEntry(e)) => e,
                    _ => HoldEntry::new(),
                };
                if let Some(h) = self.legs[n].as_mut().and_then(|l| l.hold.as_mut()) {
                    e.recompute(is_active, true, prev.outbound_course, h);
                }
                self.trans[t] = Some(Transition::HoldEntry(e));
            }
            _ => {
                let mut fx = match self.trans[t].take() {
                    Some(Transition::FixedRadius(f)) => f,
                    _ => FixedRadius::new(),
                };
                recompute_fixed_radius(&mut fx, &prev, prev_overshot, &next, next_distance, is_active, tas, gs);
                self.trans[t] = Some(Transition::FixedRadius(fx));
            }
        }
    }

    fn scratch_transition(&mut self, p: usize, n: usize, old: Option<Transition>, is_active: bool, tas: f64, gs: f64, ppos: Coord) -> Option<Transition> {
        let (pl, nl) = (&self.legs[p].as_ref()?.input, &self.legs[n].as_ref()?.input);
        let kind = pick(pl.spec.kind(), nl.spec.kind(), pl.waypoint_id == nl.waypoint_id)?;
        let scratch = self.legs.len();
        self.kinds[scratch] = Some(kind);
        self.slots[scratch] = true;
        self.ends[scratch] = (p, n);
        self.trans[scratch] = old;
        self.recompute_transition(scratch, is_active, tas, gs, ppos);
        self.slots[scratch] = false;
        self.kinds[scratch] = None;
        self.trans[scratch].take()
    }

    fn compute_null_bridge(&mut self, i: usize, active: usize, ppos: Coord, tas: f64, gs: f64) {
        let n = i + 1;
        if n >= self.legs.len() || self.legs[n].is_none() || self.prev_of[n] != Some(i) {
            return;
        }
        self.bridge[n] = match self.prev_of[i] {
            Some(p) => {
                let old = self.bridge[n].take();
                let (prev_tas, prev_gs) = (self.leg_tas(p, tas), self.leg_gs(p, gs));
                self.scratch_transition(p, n, old, active == i, prev_tas, prev_gs, ppos)
            }
            None => None,
        };
        let (next_tas, next_gs) = (self.leg_tas(n, tas), self.leg_gs(n, gs));
        self.recompute_leg_path(n, active == i, ppos, next_tas, next_gs);
        let nn = n + 1;
        if let Some(t) = self.out_slot(n).filter(|&t| t < self.legs.len() && self.slots[t]) {
            self.bridge_out[n] = None;
            if self.next_of[n].is_some() {
                self.recompute_transition(t, active == i + 1, next_tas, next_gs, ppos);
                self.recompute_leg_path(n, active == i, ppos, next_tas, next_gs);
            }
        } else if nn < self.legs.len() && self.legs[nn].is_some() && self.prev_of[nn] == Some(n) {
            let old = self.bridge_out[n].take();
            self.bridge_out[n] = self.scratch_transition(n, nn, old, active == i + 1, next_tas, next_gs, ppos);
            if self.bridge_out[n].is_some() {
                self.recompute_leg_path(n, active == i, ppos, next_tas, next_gs);
            }
        } else {
            self.bridge_out[n] = None;
        }
    }

    fn compute_leg(&mut self, i: usize, active: usize, ppos: Coord, tas: f64, gs: f64) {
        let leg_tas = self.leg_tas(i, tas);
        let leg_gs = self.leg_gs(i, gs);
        if self.is_null(i) {
            self.compute_null_bridge(i, active, ppos, tas, gs);
        } else if i + 1 < self.bridge.len() {
            self.bridge[i + 1] = None;
            self.bridge_out[i + 1] = None;
        }
        let prev = self.prev_of[i];
        if let Some(p) = prev {
            if i > 0 && self.slots[i - 1] {
                let prev_tas = self.leg_tas(p, tas);
                let prev_gs = self.leg_gs(p, gs);
                self.recompute_transition(i - 1, active == i, prev_tas, prev_gs, ppos);
            }
        }
        if prev.is_some_and(|p| self.is_null(p)) {
            return;
        }
        self.recompute_leg_path(i, active == i, ppos, leg_tas, leg_gs);
        if let Some(t) = self.out_slot(i).filter(|&t| t < self.slots.len() && self.slots[t]) {
            if self.next_of[i].is_some() {
                self.recompute_transition(t, active == i + 1, leg_tas, leg_gs, ppos);
                self.recompute_leg_path(i, active == i, ppos, leg_tas, leg_gs);
            }
        }
    }

    pub fn recompute(&mut self, active: usize, ppos: Coord, _true_track: f64, tas: f64, gs: f64) {
        self.version += 1;
        for _ in 0..3 {
            self.compute_pass(active, ppos, tas, gs);
        }
    }

    pub fn recompute_once(&mut self, active: usize, ppos: Coord, _true_track: f64, tas: f64, gs: f64) {
        self.version += 1;
        self.compute_pass(active, ppos, tas, gs);
    }

    fn compute_pass(&mut self, active: usize, ppos: Coord, tas: f64, gs: f64) {
        let from = active as isize - 1;
        let mut i = from.max(0) as usize;
        while i < self.legs.len() {
            if self.legs[i].is_some() {
                let skip = from >= 0 && i as isize == from && self.legs[i].as_ref().unwrap().computed;
                if !skip {
                    let was_null = self.is_null(i);
                    self.compute_leg(i, active, ppos, tas, gs);
                    if was_null != self.is_null(i) {
                        self.compute_leg(i, active, ppos, tas, gs);
                    }
                }
            }
            i += 1;
        }
    }

    pub fn recompute_engine_out(&mut self, ppos: Coord, true_track: f64) {
        self.version += 1;
        for i in 0..self.legs.len() {
            if self.legs[i].is_some() {
                self.compute_leg(i, 0, ppos, 0., 0.);
            }
        }
        let _ = true_track;
    }

    pub fn leg_path(&self, i: usize) -> &[PathVector] {
        self.legs.get(i).and_then(|l| l.as_ref()).map(|l| l.path.as_slice()).unwrap_or(&[])
    }

    fn transition_path(&self, t: usize) -> Vec<PathVector> {
        match self.trans[t].as_ref() {
            Some(Transition::HoldEntry(e)) => match self.legs[self.ends[t].1].as_ref().and_then(|l| l.hold.as_ref()) {
                Some(h) => e.predicted_path(h),
                None => Vec::new(),
            },
            Some(tr) => tr.path(),
            None => Vec::new(),
        }
    }

    pub fn all_path_vectors(&self, active: Option<usize>, missed_approach: bool) -> Vec<PathVector> {
        let mut out = Vec::new();
        for (i, leg) in self.legs.iter().enumerate() {
            let Some(leg) = leg else { continue };
            if leg.input.missed_approach != missed_approach {
                continue;
            }
            if self.is_null(i) {
                continue;
            }
            if active.is_some_and(|a| i < a) {
                continue;
            }
            let transmit_reversal = active.is_some_and(|a| i == a || i == a + 1);
            if is_course_reversal(&leg.input.spec) && !transmit_reversal {
                continue;
            }
            let is_hold = matches!(leg.input.spec, LegSpec::Hold { .. });
            if i > 0 {
                if let Some(t) = self.trans[i - 1].as_ref() {
                    if !t.is_null() && (!is_hold || transmit_reversal) {
                        out.extend(self.transition_path(i - 1));
                    }
                }
            }
            out.extend(leg.path.iter().copied());
        }
        out
    }

    pub fn efis_path_vectors(&self, active: usize, missed_approach: bool) -> Vec<PathVector> {
        let mut out = Vec::new();
        for i in active..self.legs.len() {
            let Some(leg) = self.legs[i].as_ref() else { continue };
            if leg.input.missed_approach != missed_approach || (is_course_reversal(&leg.input.spec) && i > active + 1) {
                continue;
            }
            if i > 0 && self.trans[i - 1].is_some() {
                out.extend(self.transition_path(i - 1));
            }
            out.extend(leg.path.iter().copied());
        }
        out.retain(|v| v.is_valid() && v.length() <= 5000.);
        out
    }

    fn transition_lengths(&self, t: Option<&Transition>, inbound: bool) -> f64 {
        match t {
            Some(Transition::FixedRadius(f)) if !f.is_reverted() => f.distance() / 2.,
            Some(t) if inbound => t.distance(),
            _ => 0.,
        }
    }

    pub fn leg_lengths(&self, i: usize) -> [f64; 3] {
        let outbound = self.transition_lengths(self.out_slot(i).and_then(|t| self.trans[t].as_ref()), false);
        let inbound = if i == 0 { 0. } else { self.transition_lengths(self.trans[i - 1].as_ref(), true) };
        [inbound, self.leg_distance(i), outbound]
    }

    pub fn update_calculated(&self, from: usize, to: usize) -> Vec<Option<LegCalc>> {
        let mut out: Vec<Option<LegCalc>> = vec![None; self.legs.len()];
        let mut cumulative = 0.;
        let mut cumulative_t = 0.;
        for i in from..to.min(self.legs.len()) {
            if i == from {
                out[i] = Some(LegCalc::default());
                continue;
            }
            let Some(_) = self.legs[i] else {
                let prev = self.leg(i as isize - 1);
                let next = self.leg(i as isize + 1);
                if let (Some(p), Some(n)) = (prev, next) {
                    if matches!(n.input.spec, LegSpec::If) {
                        let reference = if matches!(p.input.spec, LegSpec::Vm { .. } | LegSpec::Fm { .. }) { self.leg_start(i - 1) } else { Some(self.leg_end(i - 1)) };
                        if let Some(r) = reference.filter(|r| r.is_finite()) {
                            let d = distance_to(r, n.input.fix);
                            cumulative += d;
                            cumulative_t += d;
                        }
                    }
                }
                continue;
            };
            let [inb, dist, outb] = if self.is_null(i) { [0., 0., 0.] } else { self.leg_lengths(i) };
            cumulative += dist;
            cumulative_t += dist + inb + outb;
            out[i] = Some(LegCalc {
                distance: dist,
                distance_with_transitions: dist + inb + outb,
                cumulative_distance: cumulative,
                cumulative_distance_with_transitions: cumulative_t,
                ..Default::default()
            });
        }
        for c in out.iter_mut().flatten() {
            c.cumulative_distance_to_end = cumulative - c.cumulative_distance;
            c.cumulative_distance_to_end_with_transitions = cumulative_t - c.cumulative_distance_with_transitions;
        }
        out
    }

    fn pwp_location(vectors: &[PathVector], distance_before_terminator: f64) -> Option<Coord> {
        let mut accumulator = 0.;
        for v in vectors.iter().rev() {
            let length = v.length();
            if accumulator + length > distance_before_terminator {
                return Some(v.point(distance_before_terminator - accumulator));
            }
            accumulator += length;
        }
        None
    }

    fn leg_pwp_location(&self, i: usize, distance_before_terminator: f64) -> Option<Coord> {
        let l = self.legs[i].as_ref()?;
        match l.input.spec {
            LegSpec::Vm { .. } | LegSpec::Fm { .. } | LegSpec::Cr { .. } => None,
            LegSpec::If => Some(l.input.fix),
            _ => Self::pwp_location(&l.path, distance_before_terminator),
        }
    }

    pub fn point_from_end_of_path(&self, active: usize, calculated: &[Option<LegCalc>], wpt_count: usize, distance_from_end: f64) -> Option<(Coord, f64, usize)> {
        if !(distance_from_end > 0.) {
            return None;
        }
        for i in active.saturating_sub(1)..wpt_count.min(self.legs.len()) {
            if self.legs[i].is_none() || self.is_null(i) {
                continue;
            }
            let Some(calc) = calculated.get(i).and_then(|c| c.as_ref()) else { continue };
            let accumulator = calc.cumulative_distance_to_end_with_transitions;
            if accumulator < distance_from_end {
                let [inbound_len, leg_len, outbound_len] = self.leg_lengths(i);
                let total = inbound_len + leg_len + outbound_len;
                let from_end_of_leg = (distance_from_end - accumulator).min(total);
                let lla = if from_end_of_leg < outbound_len {
                    self.out_slot(i).and_then(|t| Self::pwp_location(&self.transition_path(t), outbound_len + from_end_of_leg))
                } else if from_end_of_leg <= outbound_len + leg_len {
                    self.leg_pwp_location(i, from_end_of_leg - outbound_len)
                } else if i > 0 {
                    Self::pwp_location(&self.transition_path(i - 1), from_end_of_leg - outbound_len - leg_len)
                } else {
                    None
                };
                return lla.map(|c| (c, from_end_of_leg, i));
            }
        }
        None
    }

    fn guidable_abeam(&mut self, g: Guidable, ppos: Coord) -> bool {
        match g {
            Guidable::Leg(i) => {
                let Some(l) = self.legs[i].as_ref() else { return false };
                match &l.input.spec {
                    LegSpec::If | LegSpec::Df | LegSpec::Ca { .. } | LegSpec::Hold { .. } => false,
                    LegSpec::Fm { .. } | LegSpec::Vm { .. } | LegSpec::Pi { .. } => true,
                    LegSpec::Tf { from } => {
                        let bearing_ac = bearing_to(*from, ppos);
                        let heading_ac = diff_angle(self.inbound_course(i).unwrap_or(0.), bearing_ac).abs();
                        if heading_ac > 90. {
                            return false;
                        }
                        heading_ac.to_radians().cos() * distance_to(*from, ppos) <= self.leg_distance(i)
                    }
                    LegSpec::Cf { course, .. } | LegSpec::Ci { course } | LegSpec::Fa { course, .. } | LegSpec::Fc { course, .. } | LegSpec::Cr { course, .. } => {
                        let dtg = course_to_fix_distance_to_go(ppos, *course, self.leg_end(i));
                        dtg >= 0. && dtg <= self.leg_distance(i)
                    }
                    LegSpec::Cd { .. } | LegSpec::Fd { .. } => {
                        let dtg = self.leg_distance_to_go(i, ppos).unwrap_or(-1.);
                        dtg >= 0. && dtg <= self.leg_distance(i)
                    }
                    LegSpec::Af { centre, .. } => {
                        let Some(start) = self.leg_start(i) else { return false };
                        let bp = bearing_to(*centre, ppos);
                        let bf = bearing_to(*centre, start);
                        let tae = if l.af_sweep > 0. { diff_angle(bf, bp) } else { diff_angle(bp, bf) };
                        tae >= 0.
                    }
                    LegSpec::Rf { from, center } => {
                        let Some((_, _, cw, _)) = self.rf_params(i) else { return false };
                        let bp = bearing_to(*center, ppos);
                        let bf = bearing_to(*center, *from);
                        let tae = if cw { diff_angle(bf, bp) } else { diff_angle(bp, bf) };
                        tae >= 0.
                    }
                }
            }
            Guidable::Transition(t) => {
                let (p, n) = self.ends[t];
                let prev_dtg = self.leg_distance_to_go(p, ppos).unwrap_or(f64::INFINITY);
                let prev_outbound = self.outbound_course(p).unwrap_or(0.);
                let next_inbound = self.inbound_course(n).unwrap_or(0.);
                match self.trans[t].as_mut() {
                    Some(Transition::PathCapture(pc)) => pc_abeam(pc, prev_dtg),
                    Some(Transition::CourseCapture(c)) => !c.is_null && c.computed_turn != TurnDirection::Either && !c.forced_turn_complete && prev_dtg <= 0.,
                    Some(Transition::FixedRadius(f)) => {
                        if let Some(pc) = &f.revert {
                            return pc_abeam(pc, prev_dtg);
                        }
                        let [inb, outb] = f.turning_points;
                        let in_h = diff_angle(prev_outbound, bearing_to(inb, ppos)).abs();
                        let out_h = diff_angle(next_inbound, bearing_to(outb, ppos)).abs();
                        in_h <= 90. && out_h >= 90.
                    }
                    Some(Transition::DirectToFix(d)) => d.is_abeam(ppos),
                    Some(Transition::DmeArc(d)) => d.is_abeam(ppos, prev_outbound, next_inbound),
                    Some(Transition::HoldEntry(e)) => e.is_abeam(prev_dtg),
                    None => false,
                }
            }
        }
    }

    pub fn freeze_transition(&mut self, t: usize) {
        match self.trans[t].as_mut() {
            Some(Transition::FixedRadius(f)) => f.frozen = true,
            Some(Transition::DirectToFix(d)) => d.frozen = true,
            Some(Transition::DmeArc(d)) => d.frozen = true,
            Some(Transition::PathCapture(p)) => p.frozen = true,
            Some(Transition::CourseCapture(_)) | Some(Transition::HoldEntry(_)) => {}
            None => {}
        }
    }

    fn guidable_guidance(&mut self, g: Guidable, ppos: Coord, true_track: f64, tas: f64, gs: f64) -> Option<LateralPath> {
        let gs_ms = gs * 463. / 900.;
        let env = self.env;
        match g {
            Guidable::Leg(i) => {
                let l = self.legs[i].as_ref()?;
                match &l.input.spec {
                    LegSpec::If => {
                        if self.out_slot(i).is_some_and(|t| self.trans[t].is_some()) {
                            self.guidable_guidance(Guidable::Transition(self.out_slot(i).unwrap()), ppos, true_track, tas, gs)
                        } else if let Some(n) = self.next_of[i] {
                            self.guidable_guidance(Guidable::Leg(n), ppos, true_track, tas, gs)
                        } else {
                            None
                        }
                    }
                    LegSpec::Tf { from } => Some(fix_to_fix_guidance(ppos, true_track, *from, l.input.fix)),
                    LegSpec::Df => Some(fix_to_fix_guidance(ppos, true_track, l.df_start?, l.input.fix)),
                    LegSpec::Cf { course, .. } => Some(course_to_fix_guidance(ppos, true_track, *course, self.leg_end(i))),
                    LegSpec::Fa { .. } | LegSpec::Fm { .. } => Some(fix_to_fix_guidance(ppos, true_track, l.input.fix, self.leg_end(i))),
                    LegSpec::Fc { .. } => Some(fix_to_fix_guidance(ppos, true_track, self.leg_start(i)?, self.leg_end(i))),
                    LegSpec::Ca { course, .. } => Some(course_to_fix_guidance(ppos, true_track, *course, l.ca_end?)),
                    LegSpec::Cr { course, .. } => Some(course_to_fix_guidance(ppos, true_track, *course, l.intercept?)),
                    LegSpec::Ci { course } => Some(LateralPath { cross_track_error: 0., track_angle_error: diff_angle(true_track, *course), phi_command: 0., phi_limit: 0., heading: None }),
                    LegSpec::Rf { from, .. } => {
                        let (center, _, cw, angle) = self.rf_params(i)?;
                        Some(arc_guidance(ppos, true_track, *from, center, if cw { angle } else { -angle }, gs_ms))
                    }
                    LegSpec::Vm { heading } => Some(LateralPath { cross_track_error: 0., track_angle_error: 0., phi_command: 0., phi_limit: 0., heading: Some(*heading) }),
                    LegSpec::Cd { .. } | LegSpec::Fd { .. } => Some(fix_to_fix_guidance(ppos, true_track, self.leg_start(i)?, l.intercept?)),
                    LegSpec::Af { centre, .. } => Some(arc_guidance(ppos, true_track, self.leg_start(i)?, *centre, l.af_sweep, gs_ms)),
                    LegSpec::Pi { .. } => {
                        let next = self.next_of[i];
                        let pi = self.legs[i].as_mut()?.pi.as_mut()?;
                        let dtg = pi.advance(ppos);
                        if pi.phase == PiPhase::Intercept {
                            return self.guidable_guidance(Guidable::Leg(next?), ppos, true_track, tas, gs);
                        }
                        pi.guidance(dtg, ppos, true_track, tas, gs)
                    }
                    LegSpec::Hold { kind, .. } => {
                        let entry = if *kind == HoldKind::Hf { self.hold_entry_info(i) } else { None };
                        let h = self.legs[i].as_mut()?.hold.as_mut()?;
                        if let Some((is_null, state)) = entry {
                            h.term_met = is_null || matches!(state, EntryState::Capture | EntryState::Done);
                        }
                        Some(h.guidance(ppos, true_track, tas, gs, &env))
                    }
                }
            }
            Guidable::Transition(t) => {
                let tr = self.trans[t].clone()?;
                match tr {
                    Transition::FixedRadius(f) => match f.revert {
                        None => Some(arc_guidance(ppos, true_track, f.turning_points[0], f.centre, f.sweep, gs_ms)),
                        Some(pc) => self.pc_guidance(t, pc, ppos, true_track, tas, gs),
                    },
                    Transition::PathCapture(pc) => self.pc_guidance(t, pc, ppos, true_track, tas, gs),
                    Transition::CourseCapture(cc) => self.cc_guidance(t, cc, ppos, true_track, tas, gs),
                    Transition::DmeArc(d) => Some(arc_guidance(ppos, true_track, d.itp, d.centre, d.sweep, gs_ms)),
                    Transition::DirectToFix(_) => match self.trans[t].as_mut() {
                        Some(Transition::DirectToFix(d)) => Some(d.guidance(ppos, true_track, tas, gs)),
                        _ => None,
                    },
                    Transition::HoldEntry(_) => {
                        let n = self.ends[t].1;
                        let mut e = match self.trans[t].take() {
                            Some(Transition::HoldEntry(e)) => e,
                            _ => return None,
                        };
                        let out = self.legs[n].as_mut().and_then(|l| l.hold.as_mut()).and_then(|h| e.guidance(h, ppos, true_track, tas, gs, &env));
                        self.trans[t] = Some(Transition::HoldEntry(e));
                        out
                    }
                }
            }
        }
    }

    fn cc_guidance(&mut self, t: usize, cc: CourseCapture, ppos: Coord, true_track: f64, tas: f64, gs: f64) -> Option<LateralPath> {
        let n = self.ends[t].1;
        if cc.computed_turn != TurnDirection::Either {
            let turn_sign = if cc.computed_turn == TurnDirection::Left { -1. } else { 1. };
            let mut tae = self.inbound_course(n).unwrap_or(0.) - true_track;
            if turn_sign != sign(tae) {
                tae += turn_sign * 360.;
            }
            if tae.abs() > 130. {
                return Some(LateralPath { cross_track_error: 0., track_angle_error: 0., phi_command: turn_sign * max_bank(tas, false), phi_limit: 0., heading: None });
            }
            if let Some(Transition::CourseCapture(c)) = self.trans[t].as_mut() {
                c.forced_turn_complete = true;
            }
        }
        self.guidable_guidance(Guidable::Leg(n), ppos, true_track, tas, gs)
    }

    fn pc_guidance(&mut self, t: usize, pc: PathCapture, ppos: Coord, true_track: f64, tas: f64, gs: f64) -> Option<LateralPath> {
        let n = self.ends[t].1;
        if pc.computed_turn != TurnDirection::Either {
            let turn_sign = if pc.computed_turn == TurnDirection::Left { -1. } else { 1. };
            let mut tae = pc.target_track - true_track;
            if turn_sign != sign(tae) {
                tae += turn_sign * 360.;
            }
            if tae.abs() > 130. {
                return Some(LateralPath { cross_track_error: 0., track_angle_error: 0., phi_command: turn_sign * max_bank(tas, true), phi_limit: 0., heading: None });
            }
            match self.trans[t].as_mut() {
                Some(Transition::PathCapture(p)) => p.forced_turn_complete = true,
                Some(Transition::FixedRadius(f)) => {
                    if let Some(p) = f.revert.as_mut() {
                        p.forced_turn_complete = true
                    }
                }
                _ => {}
            }
        }
        self.guidable_guidance(Guidable::Leg(n), ppos, true_track, tas, gs)
    }

    fn guidable_dtg(&self, g: Guidable, ppos: Coord) -> Option<f64> {
        match g {
            Guidable::Leg(i) => self.leg_distance_to_go(i, ppos),
            Guidable::Transition(t) => match self.trans[t].as_ref()? {
                Transition::FixedRadius(f) if f.revert.is_none() => Some(arc_distance_to_go(ppos, f.turning_points[0], f.centre, f.sweep)),
                Transition::CourseCapture(c) => Some(arc_distance_to_go(ppos, c.start, c.centre, c.sweep)),
                Transition::DirectToFix(d) => Some(d.distance_to_go(ppos)),
                Transition::DmeArc(d) => Some(arc_distance_to_go(ppos, d.itp, d.centre, d.sweep)),
                Transition::HoldEntry(e) => Some(e.distance_to_go()),
                _ => Some(1.),
            },
        }
    }

    fn guidable_nominal_roll(&self, g: Guidable, gs: f64) -> (bool, bool, f64) {
        let phi_for = |cw: bool, r: f64| (if cw { 1. } else { -1. }) * (((gs * 463. / 900.).powi(2)) / (r * 1852. * G)).atan().to_degrees();
        match g {
            Guidable::Leg(i) => {
                if let Some((_, r, cw, _)) = self.rf_params(i) {
                    return (true, true, phi_for(cw, r));
                }
                let Some(l) = self.legs[i].as_ref() else { return (false, false, 0.) };
                match (&l.input.spec, &l.hold) {
                    (LegSpec::Af { centre, .. }, _) => (true, true, phi_for(l.af_sweep > 0., distance_to(*centre, l.input.fix))),
                    (_, Some(h)) => (h.ends_in_circular_arc(), h.starts_in_circular_arc(), h.nominal_roll(gs)),
                    _ => (false, false, 0.),
                }
            }
            Guidable::Transition(t) => match self.trans[t].as_ref() {
                Some(Transition::FixedRadius(f)) => {
                    if f.revert.is_some() {
                        (true, true, 0.)
                    } else {
                        (true, true, phi_for(f.clockwise, f.radius))
                    }
                }
                Some(Transition::CourseCapture(c)) if !c.is_null => (true, true, phi_for(c.clockwise, c.radius)),
                Some(Transition::DirectToFix(d)) => (d.has_arc, false, d.nominal_roll(gs)),
                Some(Transition::DmeArc(d)) => (true, true, d.nominal_roll(gs)),
                Some(Transition::HoldEntry(e)) => match self.legs[self.ends[t].1].as_ref().and_then(|l| l.hold.as_ref()) {
                    Some(h) => (true, true, e.nominal_roll(h, gs)),
                    None => (true, true, 0.),
                },
                _ => (false, false, 0.),
            },
        }
    }

    pub fn guidance_parameters(&mut self, active: usize, ppos: Coord, true_track: f64, gs: f64, tas: f64) -> Option<LateralPath> {
        let active_is_leg = self.legs.get(active).is_some_and(|l| l.is_some());
        let next_is_leg = self.legs.get(active + 1).is_some_and(|l| l.is_some());
        let auto_sequencing = !self.disable_automatic_sequencing(active);
        let mut active_g = None;
        let mut next_g = None;

        let from_idx = active.checked_sub(1);
        let from_t = from_idx.filter(|&f| self.slots.get(f).copied().unwrap_or(false) && self.trans[f].is_some());
        let to_t = if self.slots.get(active).copied().unwrap_or(false) && self.trans[active].is_some() { Some(active) } else { None };

        let from_abeam = match from_t.filter(|&f| !self.trans[f].as_ref().unwrap().is_null()) {
            Some(f) => self.guidable_abeam(Guidable::Transition(f), ppos),
            None => false,
        };
        if let (Some(f), true) = (from_t, from_abeam) {
            self.freeze_transition(f);
            let ca_active = matches!(self.legs[active].as_ref().map(|l| &l.input.spec), Some(LegSpec::Ca { .. }));
            if ca_active && matches!(self.trans[f], Some(Transition::CourseCapture(_))) {
                active_g = Some(Guidable::Leg(active));
                next_g = to_t.map(Guidable::Transition);
            } else {
                active_g = Some(Guidable::Transition(f));
                next_g = Some(Guidable::Leg(active));
            }
        } else if let Some(t) = to_t.filter(|&t| !self.trans[t].as_ref().unwrap().is_null() && auto_sequencing) {
            if self.guidable_abeam(Guidable::Transition(t), ppos) {
                if matches!(self.trans[t], Some(Transition::FixedRadius(_))) {
                    self.freeze_transition(t);
                }
                active_g = Some(Guidable::Transition(t));
                next_g = next_is_leg.then_some(Guidable::Leg(active + 1));
            } else if active_is_leg {
                active_g = Some(Guidable::Leg(active));
                next_g = Some(Guidable::Transition(t));
            }
        } else if active_is_leg {
            active_g = Some(Guidable::Leg(active));
            if next_is_leg && auto_sequencing {
                next_g = Some(Guidable::Leg(active + 1));
            }
        }

        let active_g = active_g?;
        let phi_limit = max_bank(tas, self.capturing_path(active_g));
        let mut params = self.guidable_guidance(active_g, ppos, true_track, tas, gs)?;
        params.phi_limit = phi_limit;
        let dtg = self.guidable_dtg(active_g, ppos);
        if let Some(next_g) = next_g {
            let (from_end_arc, _, phi_from) = self.guidable_nominal_roll(active_g, gs);
            let (_, to_start_arc, phi_to) = self.guidable_nominal_roll(next_g, gs);
            if from_end_arc || to_start_arc {
                let rad = roll_anticipation_distance(gs, if from_end_arc { phi_from } else { 0. }, if to_start_arc { phi_to } else { 0. });
                if rad > 0. && dtg.is_some_and(|d| d <= rad) {
                    if let Some(next) = self.guidable_guidance(next_g, ppos, true_track, tas, gs) {
                        if next.heading.is_none() {
                            params.phi_command = next.phi_command;
                        }
                    }
                }
            }
        }
        Some(params)
    }

    fn capturing_path(&self, g: Guidable) -> bool {
        match g {
            Guidable::Leg(i) => !matches!(self.legs[i].as_ref().map(|l| &l.input.spec), Some(LegSpec::Ci { .. } | LegSpec::Ca { .. } | LegSpec::Cr { .. } | LegSpec::Vm { .. })),
            Guidable::Transition(t) => !matches!(self.trans[t], Some(Transition::CourseCapture(_))),
        }
    }

    pub fn distance_to_go(&self, active: usize, ppos: Coord) -> Option<f64> {
        self.leg_distance_to_go(active, ppos)
    }

    pub fn should_sequence_leg(&mut self, active: usize, ppos: Coord) -> bool {
        if self.legs.get(active).and_then(|l| l.as_ref()).is_none() {
            return false;
        }
        if matches!(self.spec(active), Some(LegSpec::Fm { .. })) {
            return false;
        }
        if active > 0 && self.slots[active - 1] && self.trans[active - 1].as_ref().is_some_and(|t| !t.is_null()) {
            let abeam = self.guidable_abeam(Guidable::Transition(active - 1), ppos);
            if abeam && (self.leg_distance(active) > 0.01 || self.overshot(active)) {
                return false;
            }
        }
        if self.is_null(active) {
            return true;
        }
        self.leg_distance_to_go(active, ppos).is_some_and(|d| d < 0.001)
    }

    pub fn leg_kind(&self, i: usize) -> Option<LegKind> {
        self.spec(i).map(|s| s.kind())
    }
}

fn pc_abeam(pc: &PathCapture, prev_dtg: f64) -> bool {
    !pc.is_null && pc.computed_turn != TurnDirection::Either && !pc.forced_turn_complete && prev_dtg <= 0.
}

pub fn string_missed_approach(elements: &[PlanElement], missed_approach_point: usize, first_approach_leg: usize) -> Vec<PlanElement> {
    let mut out: Vec<PlanElement> = Vec::with_capacity(elements.len() * 2);
    for e in elements {
        out.push(match e {
            PlanElement::Leg(l) => {
                let mut l = l.clone();
                l.missed_approach = false;
                PlanElement::Leg(l)
            }
            PlanElement::Discontinuity => PlanElement::Discontinuity,
        });
    }
    out.push(PlanElement::Discontinuity);
    for e in &elements[first_approach_leg..] {
        out.push(e.clone());
    }
    let _ = missed_approach_point;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(lat: f64, long: f64) -> Coord {
        Coord::new(lat, long)
    }

    fn straight() -> Geometry {
        let a = c(50., 0.);
        let b = c(50., 1.);
        let d = c(50., 2.);
        let els = vec![
            PlanElement::Leg(LegInput::initial_fix("A", a)),
            PlanElement::Leg(LegInput::track_to_fix("B", a, b)),
            PlanElement::Leg(LegInput::track_to_fix("C", b, d)),
        ];
        Geometry::new(&els)
    }

    #[test]
    fn a_track_to_fix_after_a_direct_to_fix_starts_at_that_fix() {
        let a = c(-33.80, 150.62);
        let kadom = c(-33.710806, 150.299611);
        let pks = c(-33.131278, 148.237306);
        let els = vec![
            PlanElement::Leg(LegInput::initial_fix("A", a)),
            PlanElement::Leg(LegInput::new("KADOM", LegSpec::Df, kadom)),
            PlanElement::Leg(LegInput::track_to_fix("PKS", kadom, pks)),
        ];
        let mut g = Geometry::new(&els);
        let near = c(-33.7135, 150.3110);
        g.recompute(1, near, 287., 380., 380.);
        let before = g.guidance_parameters(1, near, 287., 380., 380.).unwrap();
        let past = c(-33.7090, 150.2930);
        g.recompute(2, past, 287., 380., 380.);
        let after = g.guidance_parameters(2, past, 287., 380., 380.).unwrap();
        assert!(after.cross_track_error.abs() < 1., "before {before:?} after {after:?}");
    }

    #[test]
    fn straight_legs_have_a_null_transition_and_no_gap() {
        let mut g = straight();
        g.recompute(1, c(50., 0.), 90., 250., 250.);
        let v = g.all_path_vectors(Some(1), false);
        assert!(!v.is_empty());
        assert!(v.iter().all(|x| x.is_valid()));
        let total: f64 = v.iter().map(|x| x.length()).sum();
        assert!((total - distance_to(c(50., 0.), c(50., 2.))).abs() < 0.2);
    }

    #[test]
    fn guidance_on_track_has_no_cross_track_error() {
        let mut g = straight();
        g.recompute(1, c(50., 0.5), 90., 250., 250.);
        let p = g.guidance_parameters(1, c(50., 0.5), 90., 250., 250.).unwrap();
        assert!(p.cross_track_error.abs() < 0.5);
        assert!(p.track_angle_error.abs() < 1.);
        assert_eq!(p.phi_limit, 25.);
    }

    #[test]
    fn sequencing_waits_for_the_fix() {
        let mut g = straight();
        g.recompute(1, c(50., 0.5), 90., 250., 250.);
        assert!(!g.should_sequence_leg(1, c(50., 0.5)));
        assert!(g.should_sequence_leg(1, c(50., 1.01)));
    }

    #[test]
    fn right_angle_turn_uses_a_fixed_radius_arc() {
        let a = c(50., 0.);
        let b = c(50., 1.);
        let d = c(51., 1.);
        let els = vec![
            PlanElement::Leg(LegInput::initial_fix("A", a)),
            PlanElement::Leg(LegInput::track_to_fix("B", a, b)),
            PlanElement::Leg(LegInput::track_to_fix("C", b, d)),
        ];
        let mut g = Geometry::new(&els);
        g.recompute(1, a, 90., 250., 250.);
        let arcs: Vec<_> = g.all_path_vectors(Some(1), false).into_iter().filter(|v| matches!(v, PathVector::Arc { .. })).collect();
        assert_eq!(arcs.len(), 1);
        let PathVector::Arc { sweep, .. } = arcs[0] else { unreachable!() };
        assert!((sweep.abs() - 90.).abs() < 1.5);
    }
    #[test]
    fn course_to_intercept_captures_with_a_course_capture_arc_then_a_fixed_radius_arc() {
        let a = c(50., 0.);
        let b = c(50., 0.0583);
        let d = c(50.3, 0.2);
        let els = vec![
            PlanElement::Leg(LegInput::initial_fix("A", a)),
            PlanElement::Leg(LegInput::new("B", LegSpec::Cf { course: 90., length: 3. }, b)),
            PlanElement::Leg(LegInput::new("INTCPT", LegSpec::Ci { course: 45. }, c(0., 0.))),
            PlanElement::Leg(LegInput::new("D", LegSpec::Cf { course: 0., length: 10. }, d)),
        ];
        let mut g = Geometry::new(&els);
        g.recompute(1, a, 90., 250., 250.);
        let v = g.all_path_vectors(Some(1), false);
        assert!(v.iter().all(|x| x.is_valid()), "{v:?}");
        let arcs: Vec<f64> = v.iter().filter_map(|x| if let PathVector::Arc { sweep, .. } = x { Some(*sweep) } else { None }).collect();
        assert_eq!(arcs.len(), 2, "{v:?}");
        assert!((arcs[0] + 45.).abs() < 0.5, "{arcs:?}");
        assert!(matches!(g.transition(1), Some(Transition::CourseCapture(_))));
        let end = v.last().unwrap().end();
        assert!((end.long - 0.2).abs() < 0.01 && end.lat > 50.1, "{end:?}");
    }

    #[test]
    fn fixed_angle_legs_extend_from_their_fix() {
        let a = c(50., 0.);
        let b = c(50., 1.);
        let els = vec![
            PlanElement::Leg(LegInput::initial_fix("A", a)),
            PlanElement::Leg(LegInput::new("B", LegSpec::Fa { course: 90., altitude: 3000., start_altitude: 0. }, b)),
            PlanElement::Leg(LegInput::new("C", LegSpec::Fm { course: 90. }, b)),
        ];
        let mut g = Geometry::new(&els);
        g.recompute(1, b, 90., 200., 200.);
        let fa = g.leg_path(1);
        assert!((distance_to(b, fa[0].end()) - 6.).abs() < 0.05, "{fa:?}");
        assert!(!g.should_sequence_leg(2, b));
        assert!(g.guidance_parameters(2, b, 90., 200., 200.).is_some());
    }

    #[test]
    fn course_to_radial_ends_on_the_radial() {
        let a = c(50., 0.);
        let navaid = c(50.5, 0.5);
        let els = vec![
            PlanElement::Leg(LegInput::initial_fix("RW", a)),
            PlanElement::Leg(LegInput::new("R", LegSpec::Cr { course: 90., navaid, theta: 180. }, c(0., 0.))),
        ];
        let mut g = Geometry::new(&els);
        g.recompute(1, a, 90., 160., 160.);
        let end = g.leg_path(1)[0].end();
        assert!((end.long - 0.5).abs() < 0.01 && (end.lat - 50.).abs() < 0.01, "{end:?}");
        assert!(g.guidance_parameters(1, a, 90., 160., 160.).is_some());
    }

    #[test]
    fn course_to_altitude_ends_at_the_estimated_climb_distance_and_follows_ppos_once_active() {
        let a = c(50., 0.);
        let els = vec![
            PlanElement::Leg(LegInput::initial_fix("RW", a)),
            PlanElement::Leg(LegInput::new("1000", LegSpec::Ca { course: 90., altitude: 2000., after_runway: false }, c(0., 0.))),
        ];
        let mut g = Geometry::new(&els);
        g.recompute(0, a, 90., 160., 160.);
        let p = g.leg_path(1)[0];
        assert!((distance_to(a, p.end()) - 175. / 60.).abs() < 0.01, "{p:?}");
        let ppos = c(50.01, 0.01);
        g.recompute(1, ppos, 90., 160., 160.);
        assert!(distance_to(g.leg_path(1)[0].start(), ppos) < 1e-6);
        assert!((distance_to(a, g.leg_path(1)[0].end()) - 175. / 60.).abs() < 0.01);
        assert_eq!(g.guidance_parameters(1, ppos, 90., 160., 160.).unwrap().phi_limit, 30.);
    }

    #[test]
    fn heading_to_manual_flies_the_heading_law_and_never_sequences() {
        let a = c(50., 0.);
        let els = vec![
            PlanElement::Leg(LegInput::initial_fix("RW", a)),
            PlanElement::Leg(LegInput::new("1000", LegSpec::Ca { course: 90., altitude: 2000., after_runway: true }, c(0., 0.))),
            PlanElement::Leg(LegInput::new("MANUAL", LegSpec::Vm { heading: 180. }, c(0., 0.))),
        ];
        let mut g = Geometry::new(&els);
        g.recompute(2, a, 90., 160., 160.);
        let p = g.guidance_parameters(2, c(49.9, 0.05), 180., 160., 160.).unwrap();
        assert_eq!(p.heading, Some(180.));
        assert!(g.disable_automatic_sequencing(2));
        assert!((g.leg_path(2)[0].length() - VM_LEG_SIZE).abs() < 0.5);
    }

    #[test]
    fn course_reversal_after_initial_intercept_makes_the_if_null() {
        let a = c(50., 0.);
        let d = c(50.3, 0.2);
        let els = vec![
            PlanElement::Leg(LegInput::initial_fix("A", a)),
            PlanElement::Leg(LegInput::new("B", LegSpec::Cf { course: 90., length: 3. }, c(50., 0.0583))),
            PlanElement::Leg(LegInput::new("INTCPT", LegSpec::Ci { course: 45. }, c(0., 0.))),
            PlanElement::Leg(LegInput::initial_fix("X", c(50.2, 0.2))),
            PlanElement::Leg(LegInput::new("D", LegSpec::Cf { course: 0., length: 10. }, d)),
        ];
        let mut g = Geometry::new(&els);
        g.recompute(1, a, 90., 250., 250.);
        assert!(g.is_null(3));
        assert!(matches!(g.transition(3), Some(Transition::FixedRadius(_))));
        assert!(g.leg_path(2).first().is_some_and(|v| v.end().lat > 50.));
    }

    #[test]
    fn manual_hold_draws_a_hippodrome_and_a_teardrop_entry() {
        let a = c(39.0955, -74.8003);
        let b = c(39.4145, -74.4547);
        let els = vec![
            PlanElement::Leg(LegInput::initial_fix("A", a)),
            PlanElement::Leg(LegInput::track_to_fix("B", a, b)),
            PlanElement::Leg(LegInput { turn: TurnDirection::Right, ..LegInput::new("B", LegSpec::Hold { kind: HoldKind::Hm, course: 270., length: None, length_time: Some(1.), altitude: None }, b) }),
        ];
        let mut g = Geometry::new(&els);
        g.recompute(1, a, 40., 220., 220.);
        let v = g.all_path_vectors(Some(1), false);
        assert_eq!(v.len(), 1 + 6 + 4, "{v:?}");
        let PathVector::Arc { centre, .. } = v[7] else { panic!("{v:?}") };
        assert!((distance_to(centre, b) - 1.012).abs() < 0.01);
    }
}

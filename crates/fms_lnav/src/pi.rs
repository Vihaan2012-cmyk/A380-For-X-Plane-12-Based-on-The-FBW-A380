use crate::common::*;
use crate::geo::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PiPhase {
    Straight,
    Turn1,
    Outbound,
    Turn2,
    Intercept,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Segment {
    pub itp: Coord,
    pub ftp: Coord,
    pub centre: Coord,
    pub sweep: f64,
    pub course: f64,
    pub length: f64,
}

#[derive(Clone, Debug)]
pub struct Pi {
    pub fix: Coord,
    pub outbound_def_course: f64,
    pub turn: TurnDirection,
    pub radius: f64,
    pub straight: Segment,
    pub turn1: Segment,
    pub outbound: Segment,
    pub turn2: Segment,
    pub intercept: Segment,
    pub phase: PiPhase,
    pub computed: bool,
}

impl Pi {
    pub fn new(fix: Coord, outbound_def_course: f64, turn: TurnDirection, next_course: f64, next_fix: Coord) -> Self {
        let mut p = Self {
            fix,
            outbound_def_course,
            turn,
            radius: 1.,
            straight: Segment::default(),
            turn1: Segment::default(),
            outbound: Segment::default(),
            turn2: Segment::default(),
            intercept: Segment::default(),
            phase: PiPhase::Straight,
            computed: false,
        };
        p.recompute(false, 220., 220., next_course, next_fix);
        p
    }

    fn turn1_sign(&self) -> f64 {
        if self.turn == TurnDirection::Left {
            1.
        } else {
            -1.
        }
    }

    pub fn recompute(&mut self, is_active: bool, tas: f64, gs: f64, next_course: f64, next_fix: Coord) {
        if is_active {
            return;
        }
        let turn1_sign = self.turn1_sign();
        let turn2_sign = -turn1_sign;
        let gs_ms = gs / 1.94384;
        self.radius = gs_ms * gs_ms / (9.81 * max_bank(tas, true).to_radians().tan()) / 1852.;
        let min_straight = self.radius * 2.;
        let brg_to_cf = bearing_to(self.fix, next_fix);
        let dist_to_cf = distance_to(self.fix, next_fix);
        let cf_inverse = (next_course + 180.) % 360.;
        self.outbound.course = self.outbound_def_course;
        self.straight.itp = self.fix;
        self.straight.course = cf_inverse;

        let tp = if diff_angle(cf_inverse, brg_to_cf).abs() < 90. && dist_to_cf > min_straight {
            next_fix
        } else {
            let candidates = small_circle_great_circle_intersection(self.fix, min_straight, next_fix, cf_inverse).map(|c| c.to_vec()).unwrap_or_default();
            let tp = candidates.into_iter().find(|p| diff_angle(cf_inverse, bearing_to(next_fix, *p)).abs() < 90.).unwrap_or(Coord::new(f64::NAN, f64::NAN));
            self.straight.course = bearing_to(self.fix, tp);
            tp
        };

        self.turn1.sweep = turn1_sign * diff_angle(self.straight.course, self.outbound.course).abs();
        let tp_t1_ftp = self.radius * (self.turn1.sweep.abs().to_radians() / 2.).tan();
        self.turn1.ftp = place_bearing_distance(tp, self.outbound.course, tp_t1_ftp);
        self.turn1.centre = place_bearing_distance(self.turn1.ftp, (360. + self.outbound.course + turn1_sign * 90.) % 360., self.radius);
        self.turn1.itp = place_bearing_distance(tp, (self.straight.course + 180.) % 360., self.radius * (1. - self.turn1.sweep.to_radians().cos()));
        self.turn1.length = (self.turn1.sweep / 180. * self.radius).abs();
        self.straight.ftp = self.turn1.itp;
        self.straight.length = distance_to(self.fix, self.turn1.itp);

        let theta = diff_angle(self.outbound.course, (next_course + 180.) % 360.).abs().to_radians();
        self.outbound.length = self.radius * (1. / (theta / 2.).tan());
        self.outbound.itp = self.turn1.ftp;
        self.turn2.itp = place_bearing_distance(tp, self.outbound.course, self.outbound.length + tp_t1_ftp);
        self.turn2.centre = place_bearing_distance(self.turn2.itp, (360. + self.outbound.course + turn2_sign * 90.) % 360., self.radius);
        self.turn2.sweep = turn2_sign * 180.;
        self.turn2.ftp = place_bearing_distance(self.turn2.centre, (360. + self.outbound.course + turn2_sign * 90.) % 360., self.radius);
        self.turn2.length = (self.turn2.sweep / 180. * self.radius).abs();
        self.outbound.ftp = self.turn2.itp;

        self.intercept.itp = self.turn2.ftp;
        self.intercept.ftp = place_bearing_intersection(self.turn2.ftp, (self.outbound.course + 180.) % 360., tp, cf_inverse)[0];
        self.intercept.length = distance_to(self.intercept.itp, self.intercept.ftp);
        self.intercept.course = bearing_to(self.intercept.itp, self.intercept.ftp);
        self.computed = true;
    }

    pub fn distance(&self) -> f64 {
        self.intercept.length + self.turn2.length + self.outbound.length + self.turn1.length + self.straight.length
    }

    pub fn distance_to_go(&self, ppos: Coord) -> f64 {
        let (i, t2, o, t1) = (self.intercept.length, self.turn2.length, self.outbound.length, self.turn1.length);
        match self.phase {
            PiPhase::Intercept => course_to_fix_distance_to_go(ppos, self.intercept.course, self.intercept.ftp),
            PiPhase::Turn2 => i + arc_distance_to_go(ppos, self.turn2.itp, self.turn2.centre, self.turn2.sweep),
            PiPhase::Outbound => i + t2 + course_to_fix_distance_to_go(ppos, self.outbound.course, self.outbound.ftp),
            PiPhase::Turn1 => i + t2 + o + arc_distance_to_go(ppos, self.turn1.itp, self.turn1.centre, self.turn1.sweep),
            PiPhase::Straight => i + t2 + o + t1 + course_to_fix_distance_to_go(ppos, self.straight.course, self.straight.ftp),
        }
    }

    fn dtg_current(&self, ppos: Coord) -> f64 {
        match self.phase {
            PiPhase::Intercept => course_to_fix_distance_to_go(ppos, self.intercept.course, self.intercept.ftp),
            PiPhase::Turn2 => arc_distance_to_go(ppos, self.turn2.itp, self.turn2.centre, self.turn2.sweep),
            PiPhase::Outbound => course_to_fix_distance_to_go(ppos, self.outbound.course, self.outbound.ftp),
            PiPhase::Turn1 => arc_distance_to_go(ppos, self.turn1.itp, self.turn1.centre, self.turn1.sweep),
            PiPhase::Straight => course_to_fix_distance_to_go(ppos, self.straight.course, self.straight.ftp),
        }
    }

    fn rad_current(&self, tas: f64, gs: f64) -> (f64, f64) {
        let turn1_sign = self.turn1_sign();
        let turn2_sign = -turn1_sign;
        let (current, next) = match self.phase {
            PiPhase::Turn1 => (turn1_sign * max_bank(tas, true), 0.),
            PiPhase::Turn2 => (turn2_sign * max_bank(tas, true), 0.),
            PiPhase::Straight => (0., turn1_sign * max_bank(tas, true)),
            PiPhase::Outbound => (0., turn2_sign * max_bank(tas, true)),
            PiPhase::Intercept => return (0., 0.),
        };
        (roll_anticipation_distance(gs, current, next), next)
    }

    pub fn advance(&mut self, ppos: Coord) -> f64 {
        let mut dtg = self.dtg_current(ppos);
        if dtg <= 0. && self.phase < PiPhase::Intercept {
            self.phase = match self.phase {
                PiPhase::Straight => PiPhase::Turn1,
                PiPhase::Turn1 => PiPhase::Outbound,
                PiPhase::Outbound => PiPhase::Turn2,
                _ => PiPhase::Intercept,
            };
            dtg = self.dtg_current(ppos);
        }
        dtg
    }

    pub fn guidance(&self, dtg: f64, ppos: Coord, true_track: f64, tas: f64, gs: f64) -> Option<LateralPath> {
        let gs_ms = gs * 463. / 900.;
        let mut params = match self.phase {
            PiPhase::Intercept => return None,
            PiPhase::Turn2 => return Some(arc_guidance(ppos, true_track, self.turn2.itp, self.turn2.centre, self.turn2.sweep, gs_ms)),
            PiPhase::Outbound => course_to_fix_guidance(ppos, true_track, self.outbound.course, self.outbound.ftp),
            PiPhase::Turn1 => arc_guidance(ppos, true_track, self.turn1.itp, self.turn1.centre, self.turn1.sweep, gs_ms),
            PiPhase::Straight => course_to_fix_guidance(ppos, true_track, self.straight.course, self.straight.ftp),
        };
        let (rad, next) = self.rad_current(tas, gs);
        if rad > 0. && dtg <= rad {
            params.phi_command = next;
        }
        Some(params)
    }

    pub fn path(&self, start: Coord) -> Vec<PathVector> {
        vec![
            PathVector::Line { start, end: self.turn1.itp },
            PathVector::Arc { start: self.turn1.itp, centre: self.turn1.centre, end: self.turn1.ftp, sweep: self.turn1.sweep },
            PathVector::Line { start: self.turn1.ftp, end: self.turn2.itp },
            PathVector::Arc { start: self.turn2.itp, centre: self.turn2.centre, end: self.turn2.ftp, sweep: self.turn2.sweep },
            PathVector::Line { start: self.turn2.ftp, end: self.intercept.ftp },
        ]
    }
}

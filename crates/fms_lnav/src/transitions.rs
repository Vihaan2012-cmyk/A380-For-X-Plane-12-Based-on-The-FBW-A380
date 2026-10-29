use crate::common::*;
use crate::geo::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DtfState {
    Straight,
    Turn,
}

#[derive(Clone, Debug)]
pub struct DirectToFix {
    pub state: DtfState,
    pub straight_course: f64,
    pub has_arc: bool,
    pub radius: f64,
    pub clockwise: bool,
    pub line_start: Coord,
    pub line_end: Coord,
    pub arc_start: Coord,
    pub arc_centre: Coord,
    pub arc_end: Coord,
    pub arc_sweep: f64,
    pub path: Vec<PathVector>,
    pub is_null: bool,
    pub computed: bool,
    pub frozen: bool,
}

impl DirectToFix {
    pub fn new() -> Self {
        let z = Coord::new(0., 0.);
        Self {
            state: DtfState::Straight,
            straight_course: 0.,
            has_arc: false,
            radius: 0.,
            clockwise: false,
            line_start: z,
            line_end: z,
            arc_start: z,
            arc_centre: z,
            arc_end: z,
            arc_sweep: 0.,
            path: Vec::new(),
            is_null: false,
            computed: false,
            frozen: false,
        }
    }

    pub fn recompute(&mut self, is_active: bool, tas: f64, gs: f64, prev_end: Coord, prev_outbound: f64, next_fix: Coord, next_turn: TurnDirection, bank: f64) {
        if self.frozen {
            return;
        }
        let term_fix = prev_end;
        self.radius = gs * gs / (G * max_bank(tas, true).to_radians().tan()) / 6997.84 * 1.;
        let mut track_change = diff_angle_dir(prev_outbound, bearing_to(prev_end, next_fix), next_turn);
        if track_change.abs() < 3. || !track_change.is_finite() {
            self.is_null = true;
            self.computed = true;
            return;
        }
        let turn_sign = if track_change > 0. { 1. } else { -1. };
        let turn_direction = if turn_sign > 0. { TurnDirection::Right } else { TurnDirection::Left };
        let current_roll = if is_active { -bank } else { 0. };
        let roll_change = (turn_sign * max_bank(tas, true) - current_roll).abs();
        let rad = roll_anticipation_distance(gs, 0., roll_change);
        let mut itp = if rad >= 0.05 { place_bearing_distance(term_fix, prev_outbound, rad) } else { term_fix };
        let mut centre = place_bearing_distance(itp, prev_outbound + turn_sign * 90., self.radius);
        let mut distance_to_fix = distance_to(centre, next_fix);

        if distance_to_fix < self.radius {
            if diff_angle_dir(prev_outbound, bearing_to(term_fix, next_fix), next_turn).abs() < 60. {
                self.has_arc = false;
                self.line_start = term_fix;
                self.line_end = term_fix;
                self.path = vec![PathVector::Line { start: term_fix, end: term_fix }];
                self.straight_course = bearing_to(self.line_start, self.line_end);
                self.is_null = true;
                self.computed = true;
                return;
            }
            let tc_fix = bearing_to(centre, next_fix);
            let a = (tc_fix - prev_outbound).to_radians();
            let extend = (self.radius * self.radius - distance_to_fix * distance_to_fix * a.sin().powi(2)).sqrt() + distance_to_fix * a.cos() + 0.3;
            itp = place_bearing_distance(itp, prev_outbound, extend);
            centre = place_bearing_distance(centre, prev_outbound, extend);
            distance_to_fix = distance_to(centre, next_fix);
        }

        let bearing_tc_itp = bearing_to(centre, itp);
        let bearing_tc_fix = bearing_to(centre, next_fix);
        let angle_ftp_fix = (self.radius / distance_to_fix).acos().to_degrees();
        track_change = diff_angle_dir(bearing_tc_itp, diff_angle(turn_sign * angle_ftp_fix, bearing_tc_fix), turn_direction);
        let ftp = place_bearing_distance(centre, prev_outbound + track_change - 90. * turn_sign, self.radius);

        self.line_start = prev_end;
        self.line_end = itp;
        self.has_arc = true;
        self.arc_start = itp;
        self.arc_centre = centre;
        self.arc_end = ftp;
        self.arc_sweep = track_change;
        self.path = vec![
            PathVector::Line { start: self.line_start, end: self.line_end },
            PathVector::Arc { start: itp, centre, end: ftp, sweep: track_change },
        ];
        self.straight_course = bearing_to(self.line_start, self.line_end);
        self.is_null = false;
        self.computed = true;
    }

    pub fn end(&self, prev_end: Coord) -> Coord {
        if self.is_null {
            return prev_end;
        }
        self.path.last().map(|v| v.end()).unwrap_or(prev_end)
    }

    pub fn is_abeam(&self, ppos: Coord) -> bool {
        if self.is_null {
            return false;
        }
        let mut dtg = 0.;
        if self.state == DtfState::Straight {
            let straight = distance_to(self.line_start, self.line_end);
            dtg += course_to_fix_distance_to_go(ppos, self.straight_course, self.line_end);
            if dtg - straight > 0.01 {
                return false;
            }
        }
        if self.has_arc {
            if self.state == DtfState::Turn {
                dtg += arc_distance_to_go(ppos, self.arc_start, self.arc_centre, self.arc_sweep);
            } else {
                dtg += arc_length(self.radius, self.arc_sweep);
            }
        }
        dtg > 0.
    }

    pub fn distance(&self) -> f64 {
        if self.is_null {
            return 0.;
        }
        let straight = distance_to(self.line_start, self.line_end);
        if self.has_arc {
            straight + arc_length(self.radius, self.arc_sweep)
        } else {
            straight
        }
    }

    pub fn distance_to_go(&self, ppos: Coord) -> f64 {
        let straight = if self.state == DtfState::Straight { course_to_fix_distance_to_go(ppos, self.straight_course, self.line_end) } else { 0. };
        if !self.has_arc {
            return straight;
        }
        straight + arc_distance_to_go(ppos, self.arc_start, self.arc_centre, self.arc_sweep)
    }

    pub fn guidance(&mut self, ppos: Coord, true_track: f64, tas: f64, gs: f64) -> LateralPath {
        let dtg = match self.state {
            DtfState::Straight => {
                let d = course_to_fix_distance_to_go(ppos, self.straight_course, self.line_end);
                if d <= 0. && self.has_arc {
                    self.state = DtfState::Turn;
                }
                d
            }
            DtfState::Turn => arc_distance_to_go(ppos, self.arc_start, self.arc_centre, self.arc_sweep),
        };
        match self.state {
            DtfState::Straight => {
                let mut params = course_to_fix_guidance(ppos, true_track, self.straight_course, self.line_end);
                let bank_next = if self.has_arc { if self.arc_sweep > 0. { max_bank(tas, true) } else { -max_bank(tas, false) } } else { 0. };
                if dtg <= roll_anticipation_distance(tas, 0., bank_next) {
                    params.phi_command = bank_next;
                }
                params
            }
            DtfState::Turn => arc_guidance(ppos, true_track, self.arc_start, self.arc_centre, self.arc_sweep, gs * 463. / 900.),
        }
    }

    pub fn nominal_roll(&self, gs: f64) -> f64 {
        let gs_ms = gs * 463. / 900.;
        (if self.clockwise { 1. } else { -1. }) * (gs_ms * gs_ms / (self.radius * 1852. * 9.81)).atan().to_degrees()
    }
}

#[derive(Clone, Debug)]
pub struct DmeArc {
    pub radius: f64,
    pub itp: Coord,
    pub centre: Coord,
    pub ftp: Coord,
    pub sweep: f64,
    pub clockwise: bool,
    pub computed: bool,
    pub frozen: bool,
}

pub struct ArcLeg {
    pub centre: Coord,
    pub radius: f64,
    pub turn_sign: f64,
    pub boundary_radial_coded: f64,
}

pub fn closest_small_circle_intersection(centre: Coord, radius: f64, reference: Coord, bearing: f64) -> Option<Coord> {
    let [a, b] = small_circle_great_circle_intersection(centre, radius, reference, bearing)?;
    Some(if distance_to(reference, a) < distance_to(reference, b) { a } else { b })
}

impl DmeArc {
    pub fn new() -> Self {
        let z = Coord::new(0., 0.);
        Self { radius: 0., itp: z, centre: z, ftp: z, sweep: 0., clockwise: false, computed: false, frozen: false }
    }

    pub fn from_arc(&mut self, tas: f64, gs: f64, prev: &ArcLeg, prev_outbound: f64, next_inbound: f64, next_start: Coord) {
        if self.frozen {
            return;
        }
        self.radius = gs * gs / (9.81 * max_bank(tas, true).to_radians().tan()) / 6080.2;
        let turn_direction = sign(diff_angle(prev_outbound, next_inbound));
        let reference = place_bearing_distance(next_start, next_inbound + 90. * turn_direction, self.radius);
        let Some(centre) = closest_small_circle_intersection(prev.centre, prev.radius + self.radius * turn_direction * -prev.turn_sign, reference, next_inbound - 180.) else {
            self.computed = false;
            return;
        };
        self.centre = centre;
        self.itp = place_bearing_distance(centre, if turn_direction * -prev.turn_sign == 1. { bearing_to(centre, prev.centre) } else { bearing_to(prev.centre, centre) }, self.radius);
        self.ftp = place_bearing_distance(centre, next_inbound - 90. * turn_direction, self.radius);
        self.finish();
    }

    pub fn to_arc(&mut self, tas: f64, gs: f64, prev_end: Coord, prev_outbound: f64, prev_is_xf: bool, next: &ArcLeg, next_inbound: f64) {
        if self.frozen {
            return;
        }
        self.radius = gs * gs / (9.81 * max_bank(tas, true).to_radians().tan()) / 6080.2;
        let turn_direction = sign(diff_angle(prev_outbound, next_inbound));
        let reference = place_bearing_distance(prev_end, prev_outbound + 90. * turn_direction, self.radius);
        let dme = next.centre;
        let intersection = closest_small_circle_intersection(dme, next.radius + self.radius * turn_direction * -next.turn_sign, reference, prev_outbound);
        let centre;
        if prev_is_xf {
            if let Some(i) = intersection {
                centre = i;
                self.itp = place_bearing_distance(centre, prev_outbound - 90. * turn_direction, self.radius);
                self.ftp = place_bearing_distance(centre, if turn_direction * -next.turn_sign == 1. { bearing_to(centre, dme) } else { bearing_to(dme, centre) }, self.radius);
            } else {
                self.ftp = place_bearing_distance(dme, next.boundary_radial_coded, next.radius);
                let turn_sign = if turn_direction > 0. { 1. } else { -1. };
                centre = place_bearing_distance(self.ftp, normalise360(next.boundary_radial_coded + if turn_sign > 0. { 180. } else { 0. }), self.radius);
                self.itp = place_bearing_distance(centre, normalise360(prev_outbound - turn_sign * 90.), self.radius);
            }
        } else {
            let Some(i) = intersection else {
                self.computed = false;
                return;
            };
            centre = i;
            self.itp = place_bearing_distance(centre, prev_outbound - 90. * turn_direction, self.radius);
            self.ftp = place_bearing_distance(centre, if turn_direction * -next.turn_sign == 1. { bearing_to(centre, dme) } else { bearing_to(dme, centre) }, self.radius);
        }
        self.centre = centre;
        self.finish();
    }

    fn finish(&mut self) {
        self.sweep = diff_angle(bearing_to(self.centre, self.itp), bearing_to(self.centre, self.ftp));
        self.clockwise = self.sweep > 0.;
        self.computed = true;
    }

    pub fn path(&self) -> Vec<PathVector> {
        vec![PathVector::Arc { start: self.itp, centre: self.centre, end: self.ftp, sweep: self.sweep }]
    }

    pub fn distance(&self) -> f64 {
        arc_length(distance_to(self.itp, self.centre), self.sweep)
    }

    pub fn nominal_roll(&self, gs: f64) -> f64 {
        let gs_ms = gs * 463. / 900.;
        (if self.clockwise { 1. } else { -1. }) * (gs_ms * gs_ms / (self.radius * 1852. * 9.81)).atan().to_degrees()
    }

    pub fn is_abeam(&self, ppos: Coord, prev_outbound: f64, next_inbound: f64) -> bool {
        let in_h = diff_angle(prev_outbound, bearing_to(self.itp, ppos)).abs();
        let out_h = diff_angle(next_inbound, bearing_to(self.ftp, ppos)).abs();
        in_h <= 90. && out_h >= 90.
    }
}

pub fn sign(x: f64) -> f64 {
    if x > 0. {
        1.
    } else if x < 0. {
        -1.
    } else {
        0.
    }
}

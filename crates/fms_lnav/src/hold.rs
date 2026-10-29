use crate::common::*;
use crate::geo::*;
use crate::input::HoldKind;

pub const DEFAULT_HOLD_TAS: f64 = 180.;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Environment {
    pub indicated_altitude: f64,
    pub wind_direction: f64,
    pub wind_speed: f64,
    pub bank: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HxPhase {
    Inbound,
    Arc1,
    Outbound,
    Arc2,
}

impl HxPhase {
    fn next(self) -> Self {
        match self {
            HxPhase::Inbound => HxPhase::Arc1,
            HxPhase::Arc1 => HxPhase::Outbound,
            HxPhase::Outbound => HxPhase::Arc2,
            HxPhase::Arc2 => HxPhase::Inbound,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HxGeometry {
    pub fix_a: Coord,
    pub fix_b: Coord,
    pub fix_c: Coord,
    pub centre1: Coord,
    pub centre2: Coord,
    pub sweep: f64,
    pub leg_length: f64,
    pub radius: f64,
}

#[derive(Clone, Debug)]
pub struct Hold {
    pub kind: HoldKind,
    pub fix: Coord,
    pub course: f64,
    pub turn: TurnDirection,
    pub length: Option<f64>,
    pub length_time: Option<f64>,
    pub altitude: Option<f64>,
    pub phase: HxPhase,
    pub initial_phase: HxPhase,
    pub term_met: bool,
    pub next_tas: f64,
    pub cur_tas: f64,
    pub cur_gs: f64,
    pub inbound_wind: Option<f64>,
    pub geometry: HxGeometry,
    pub imm_exit_length: f64,
    pub is_null: bool,
}

impl Hold {
    pub fn new(kind: HoldKind, fix: Coord, course: f64, turn: TurnDirection, length: Option<f64>, length_time: Option<f64>, altitude: Option<f64>, env: &Environment) -> Self {
        let z = Coord::new(0., 0.);
        let mut h = Self {
            kind,
            fix,
            course,
            turn,
            length,
            length_time,
            altitude,
            phase: HxPhase::Inbound,
            initial_phase: HxPhase::Inbound,
            term_met: false,
            next_tas: DEFAULT_HOLD_TAS,
            cur_tas: DEFAULT_HOLD_TAS,
            cur_gs: DEFAULT_HOLD_TAS,
            inbound_wind: None,
            geometry: HxGeometry { fix_a: z, fix_b: z, fix_c: z, centre1: z, centre2: z, sweep: 0., leg_length: 0., radius: 0. },
            imm_exit_length: 0.,
            is_null: false,
        };
        h.geometry = h.compute_geometry(env);
        h
    }

    pub fn outbound_leg_course(&self) -> f64 {
        (self.course + 180.) % 360.
    }

    pub fn set_initial_phase(&mut self, phase: HxPhase) {
        self.phase = phase;
        self.initial_phase = phase;
    }

    pub fn compute_leg_distance(&self, env: &Environment) -> f64 {
        if self.kind == HoldKind::Hm && self.term_met {
            return self.imm_exit_length;
        }
        if let Some(l) = self.length {
            return l;
        }
        let alt = self.altitude.unwrap_or(env.indicated_altitude);
        let default_minutes = if alt < 14000. { 1. } else { 1.5 };
        let inbound_gs = self.cur_tas + self.inbound_wind.unwrap_or(0.);
        self.length_time.unwrap_or(default_minutes) * inbound_gs / 60.
    }

    pub fn radius(&self) -> f64 {
        let gs_ms = self.cur_gs / 1.94384;
        gs_ms * gs_ms / (9.81 * max_bank(self.cur_tas, true).to_radians().tan()) / 1852.
    }

    fn compute_geometry(&self, env: &Environment) -> HxGeometry {
        let leg_length = self.compute_leg_distance(env);
        let radius = self.radius();
        let turn_sign = if self.turn == TurnDirection::Left { -1. } else { 1. };
        let fix_a = place_bearing_distance(self.fix, self.course + turn_sign * 90., radius * 2.);
        let fix_b = place_bearing_distance(fix_a, self.outbound_leg_course(), leg_length);
        let fix_c = place_bearing_distance(self.fix, self.outbound_leg_course(), leg_length);
        let centre1 = place_bearing_distance(self.fix, self.course + turn_sign * 90., radius);
        let centre2 = place_bearing_distance(fix_c, self.course + turn_sign * 90., radius);
        HxGeometry { fix_a, fix_b, fix_c, centre1, centre2, sweep: turn_sign * 180., leg_length, radius }
    }

    pub fn update_prediction(&mut self, env: &Environment) {
        let angle = diff_angle(reciprocal(env.wind_direction), self.course).abs();
        self.inbound_wind = Some(angle.to_radians().cos() * env.wind_speed);
        self.cur_tas = self.next_tas;
        self.cur_gs = self.cur_tas + env.wind_speed;
        self.geometry = self.compute_geometry(env);
    }

    pub fn set_immediate_exit(&mut self, exit: bool, ppos: Coord, tas: f64, env: &Environment) {
        let g = self.geometry;
        if exit {
            self.imm_exit_length = match self.phase {
                HxPhase::Arc1 => 0.,
                HxPhase::Outbound => {
                    let next_phi = if g.sweep > 0. { max_bank(tas, true) } else { -max_bank(tas, true) };
                    let rad = roll_anticipation_distance(tas, 0., next_phi);
                    g.leg_length.min(rad + course_to_fix_distance_to_go(ppos, self.course, g.fix_a))
                }
                HxPhase::Arc2 | HxPhase::Inbound => g.leg_length,
            };
        }
        self.term_met = exit;
        if exit {
            self.geometry = self.compute_geometry(env);
        }
    }

    pub fn hippodrome_path(&self) -> Vec<PathVector> {
        let g = &self.geometry;
        vec![
            PathVector::Arc { start: self.fix, centre: g.centre1, end: g.fix_a, sweep: g.sweep },
            PathVector::Line { start: g.fix_a, end: g.fix_b },
            PathVector::Arc { start: g.fix_b, centre: g.centre2, end: g.fix_c, sweep: g.sweep },
            PathVector::Line { start: g.fix_c, end: self.fix },
        ]
    }

    pub fn predicted_path(&self) -> Vec<PathVector> {
        match self.kind {
            HoldKind::Hm => self.hippodrome_path(),
            HoldKind::Ha if !self.is_null => self.hippodrome_path(),
            _ => Vec::new(),
        }
    }

    pub fn distance_to_go_this_orbit(&self, ppos: Coord, env: &Environment) -> f64 {
        let g = &self.geometry;
        match self.phase {
            HxPhase::Inbound => course_to_fix_distance_to_go(ppos, self.course, self.fix),
            HxPhase::Arc1 => arc_distance_to_go(ppos, self.fix, g.centre1, g.sweep) + self.compute_leg_distance(env) * 2. + self.radius() * std::f64::consts::PI,
            HxPhase::Outbound => course_to_fix_distance_to_go(ppos, self.outbound_leg_course(), g.fix_b) + self.compute_leg_distance(env) + self.radius() * std::f64::consts::PI,
            HxPhase::Arc2 => arc_distance_to_go(ppos, g.fix_b, g.centre2, g.sweep) + self.compute_leg_distance(env),
        }
    }

    fn update_state(&mut self, ppos: Coord, env: &Environment) {
        let g = self.geometry;
        let dtg = match self.phase {
            HxPhase::Inbound => course_to_fix_distance_to_go(ppos, self.course, self.fix),
            HxPhase::Arc1 => arc_distance_to_go(ppos, self.fix, g.centre1, g.sweep),
            HxPhase::Outbound => course_to_fix_distance_to_go(ppos, self.outbound_leg_course(), g.fix_b),
            HxPhase::Arc2 => arc_distance_to_go(ppos, g.fix_b, g.centre2, g.sweep),
        };
        if dtg <= 0. {
            if self.phase == HxPhase::Inbound {
                if self.term_met {
                    return;
                }
                self.update_prediction(env);
            }
            self.phase = self.phase.next();
        }
    }

    pub fn guidance(&mut self, ppos: Coord, true_track: f64, tas: f64, gs: f64, env: &Environment) -> LateralPath {
        if self.kind == HoldKind::Ha && self.altitude.is_some_and(|a| env.indicated_altitude >= a) {
            self.term_met = true;
        }
        let g = self.geometry;
        self.update_state(ppos, env);
        let gs_ms = gs * 463. / 900.;
        let mut next_phi = 0.;
        let (mut params, dtg, rad) = match self.phase {
            HxPhase::Inbound => {
                let p = course_to_fix_guidance(ppos, true_track, self.course, self.fix);
                next_phi = if g.sweep > 0. { max_bank(tas, true) } else { -max_bank(tas, true) };
                let rad = roll_anticipation_distance(gs, p.phi_command, next_phi);
                (p, course_to_fix_distance_to_go(ppos, self.course, self.fix), rad)
            }
            HxPhase::Arc1 => {
                let p = arc_guidance(ppos, true_track, self.fix, g.centre1, g.sweep, gs_ms);
                let rad = roll_anticipation_distance(gs, p.phi_command, next_phi);
                if g.leg_length <= rad {
                    next_phi = p.phi_command;
                }
                (p, arc_distance_to_go(ppos, self.fix, g.centre1, g.sweep), rad)
            }
            HxPhase::Outbound => {
                let p = course_to_fix_guidance(ppos, true_track, self.outbound_leg_course(), g.fix_b);
                next_phi = if g.sweep > 0. { max_bank(tas, true) } else { -max_bank(tas, true) };
                let rad = roll_anticipation_distance(gs, p.phi_command, next_phi);
                (p, course_to_fix_distance_to_go(ppos, self.outbound_leg_course(), g.fix_b), rad)
            }
            HxPhase::Arc2 => {
                let p = arc_guidance(ppos, true_track, g.fix_b, g.centre2, g.sweep, gs_ms);
                let rad = roll_anticipation_distance(gs, p.phi_command, next_phi);
                if g.leg_length <= rad {
                    next_phi = p.phi_command;
                }
                (p, arc_distance_to_go(ppos, g.fix_b, g.centre2, g.sweep), rad)
            }
        };
        if dtg <= rad && !(self.phase == HxPhase::Inbound && self.term_met) {
            params.phi_command = next_phi;
        }
        params
    }

    pub fn recompute(&mut self, is_active: bool, tas: f64, entry: Option<(bool, EntryState)>, env: &Environment) {
        match self.kind {
            HoldKind::Ha => {
                if self.altitude.is_some_and(|a| env.indicated_altitude >= a) {
                    self.term_met = true;
                }
                if !is_active && self.term_met {
                    self.is_null = true;
                }
                self.next_tas = tas;
            }
            HoldKind::Hf => {
                if let Some((is_null, state)) = entry {
                    self.term_met = is_null || matches!(state, EntryState::Capture | EntryState::Done);
                }
                self.next_tas = tas;
            }
            HoldKind::Hm => {}
        }
        if !is_active {
            self.update_prediction(env);
        }
    }

    pub fn disable_automatic_sequencing(&self) -> bool {
        match self.kind {
            HoldKind::Hm | HoldKind::Ha => !self.term_met,
            HoldKind::Hf => false,
        }
    }

    pub fn ends_in_circular_arc(&self) -> bool {
        false
    }

    pub fn starts_in_circular_arc(&self) -> bool {
        matches!(self.phase, HxPhase::Arc1 | HxPhase::Arc2)
    }

    pub fn nominal_roll(&self, gs: f64) -> f64 {
        if self.ends_in_circular_arc() {
            max_bank(gs, true)
        } else {
            0.
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryType {
    Null,
    Teardrop,
    Parallel,
    DirectOutbound,
    DirectTurn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryState {
    Turn1,
    Straight1,
    Turn2,
    Capture,
    Done,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct EntryTurn {
    pub itp: Coord,
    pub centre: Coord,
    pub ftp: Coord,
    pub sweep: f64,
}

#[derive(Clone, Debug)]
pub struct HoldEntry {
    pub entry: EntryType,
    pub path: Vec<PathVector>,
    pub turn1: EntryTurn,
    pub turn2: EntryTurn,
    pub turn3: EntryTurn,
    pub straight_course: f64,
    pub state: EntryState,
    pub was_abeam: bool,
    pub guidance_active: bool,
    pub frozen: bool,
    pub is_null: bool,
    pub computed: bool,
    pub inbound_course: f64,
}

impl HoldEntry {
    pub fn new() -> Self {
        Self {
            entry: EntryType::Null,
            path: Vec::new(),
            turn1: EntryTurn::default(),
            turn2: EntryTurn::default(),
            turn3: EntryTurn::default(),
            straight_course: 0.,
            state: EntryState::Turn1,
            was_abeam: false,
            guidance_active: false,
            frozen: false,
            is_null: false,
            computed: false,
            inbound_course: 0.,
        }
    }

    fn set_hx_entry(&self, hold: &mut Hold) {
        match self.entry {
            EntryType::DirectOutbound => hold.set_initial_phase(HxPhase::Outbound),
            _ => hold.set_initial_phase(HxPhase::Arc1),
        }
    }

    pub fn predicted_path(&self, hold: &Hold) -> Vec<PathVector> {
        match self.entry {
            EntryType::Null => Vec::new(),
            EntryType::DirectOutbound => {
                if hold.kind == HoldKind::Hf {
                    hold.hippodrome_path()
                } else {
                    Vec::new()
                }
            }
            _ => self.path.clone(),
        }
    }

    pub fn turning_points(&self, hold: &Hold) -> [Coord; 2] {
        match self.entry {
            EntryType::Parallel | EntryType::Teardrop => [hold.fix, self.turn3.ftp],
            EntryType::DirectTurn | EntryType::DirectOutbound => [hold.fix, self.turn1.ftp],
            EntryType::Null => [hold.fix, hold.fix],
        }
    }

    pub fn distance_to_go(&self) -> f64 {
        if self.entry == EntryType::Null || self.state == EntryState::Done {
            0.
        } else {
            1.
        }
    }

    pub fn is_abeam(&mut self, prev_dtg: f64) -> bool {
        if !self.was_abeam && prev_dtg <= 0. {
            self.was_abeam = true;
            return true;
        }
        self.was_abeam && self.state != EntryState::Done
    }

    pub fn nominal_roll(&self, hold: &Hold, gs: f64) -> f64 {
        if self.entry == EntryType::Null {
            return hold.nominal_roll(gs);
        }
        if self.turn1.sweep.abs() <= 3. {
            return 0.;
        }
        if self.turn1.sweep > 0. {
            max_bank(gs, true)
        } else {
            -max_bank(gs, true)
        }
    }

    fn compute_direct_outbound(&mut self, hold: &Hold) {
        self.entry = EntryType::DirectOutbound;
        let max_radius = hold.geometry.radius;
        let turn_sign = if hold.turn == TurnDirection::Right { 1. } else { -1. };
        let track_change = diff_angle(self.inbound_course, hold.course);
        let radius = 2. * max_radius / (1. + track_change.to_radians().cos());
        self.turn1.itp = hold.fix;
        self.turn1.centre = place_bearing_distance(self.turn1.itp, self.inbound_course + turn_sign * 90., radius);
        self.turn1.sweep = turn_sign * 180. + track_change;
        self.turn1.ftp = place_bearing_distance(self.turn1.centre, normalise360(hold.course + turn_sign * 90.), radius);
        self.path.clear();
        self.path.push(PathVector::Arc { start: self.turn1.itp, end: self.turn1.ftp, centre: self.turn1.centre, sweep: self.turn1.sweep });
    }

    fn compute_direct_turn(&mut self, hold: &Hold) {
        self.entry = EntryType::DirectTurn;
        let g = hold.geometry;
        let turn_sign = if hold.turn == TurnDirection::Right { 1. } else { -1. };
        let track_change = diff_angle(self.inbound_course, hold.course);
        let radius = 2. * g.radius / (1. + track_change.to_radians().cos());
        self.turn1.itp = hold.fix;
        self.turn1.centre = place_bearing_distance(self.turn1.itp, self.inbound_course + turn_sign * 90., radius);
        self.turn1.sweep = turn_sign * 180. + track_change;
        self.turn1.ftp = place_bearing_distance(self.turn1.centre, normalise360(hold.course + turn_sign * 90.), radius);
        self.path.clear();
        self.path.push(PathVector::Arc { start: self.turn1.itp, end: self.turn1.ftp, centre: self.turn1.centre, sweep: self.turn1.sweep });
        self.straight_course = reciprocal(hold.course) % 360.;
        self.path.push(PathVector::Line { start: self.turn1.ftp, end: g.fix_b });
        self.turn2 = EntryTurn { itp: g.fix_b, ftp: g.fix_c, sweep: g.sweep, centre: g.centre2 };
        self.path.push(PathVector::Arc { start: g.fix_b, centre: g.centre2, end: g.fix_c, sweep: g.sweep });
        self.path.push(PathVector::Line { start: g.fix_c, end: hold.fix });
    }

    fn compute_teardrop(&mut self, hold: &Hold) {
        self.entry = EntryType::Teardrop;
        let (radius, leg_length) = (hold.geometry.radius, hold.geometry.leg_length);
        let turn_sign = if hold.turn == TurnDirection::Right { 1. } else { -1. };
        let outbound_course = hold.course;
        let sqrt3 = 3f64.sqrt();
        let sqrt2 = std::f64::consts::SQRT_2;
        self.straight_course = normalise360(outbound_course + 150. * turn_sign);
        self.turn1.sweep = diff_angle(self.inbound_course, self.straight_course);
        let cw = self.turn1.sweep >= 0.;
        self.turn1.itp = hold.fix;
        self.turn1.centre = place_bearing_distance(self.turn1.itp, self.inbound_course + if cw { 90. } else { -90. }, radius);
        let bearing1 = normalise360(self.inbound_course + self.turn1.sweep + if cw { -90. } else { 90. });
        self.turn1.ftp = place_bearing_distance(self.turn1.centre, bearing1, radius);
        self.path.clear();
        self.path.push(PathVector::Arc { start: self.turn1.itp, end: self.turn1.ftp, centre: self.turn1.centre, sweep: self.turn1.sweep });

        let kek = diff_angle(self.inbound_course, reciprocal(outbound_course)).abs().to_radians();
        let mut min_straight = radius * 2. / sqrt3 * (0.1 + sqrt2 - 0.5 - (kek.sin() - 0.5).abs());
        let nominal_straight = 1.15 * leg_length;
        let mut straight = min_straight.max(nominal_straight);
        let mut radii2_inbound = (kek.cos() - sqrt3 / 2.).abs() + straight / radius / 2. + (1. - sqrt3 / 2.);
        if sqrt2 - radii2_inbound > 0. {
            min_straight += radius * 2. / sqrt3 * (sqrt2 - radii2_inbound);
            straight = min_straight.max(nominal_straight);
            radii2_inbound = (kek.cos() - sqrt3 / 2.).abs() + straight / radius / 2. + (1. - sqrt3 / 2.);
        }

        self.turn2.itp = place_bearing_distance(self.turn1.ftp, self.straight_course, straight);
        self.path.push(PathVector::Line { start: self.turn1.ftp, end: self.turn2.itp });
        self.turn2.centre = place_bearing_distance(self.turn2.itp, outbound_course - turn_sign * 120., radius);

        if radii2_inbound >= 2. {
            self.turn2.ftp = place_bearing_distance(self.turn2.centre, self.straight_course + turn_sign * 75., radius);
            self.turn2.sweep = turn_sign * 165.;
            let straight_dist = (radii2_inbound - 2.) * sqrt2 * radius;
            self.turn3.itp = place_bearing_distance(self.turn2.ftp, self.straight_course + turn_sign * 165., straight_dist);
            self.turn3.sweep = turn_sign * 45.;
            self.turn3.centre = place_bearing_distance(self.turn3.itp, self.straight_course - turn_sign * 105., radius);
            self.turn3.ftp = place_bearing_distance(self.turn3.centre, outbound_course - turn_sign * 90., radius);
            self.path.push(PathVector::Line { start: self.turn2.ftp, end: self.turn3.itp });
        } else if sqrt2 - radii2_inbound < 0. {
            let intercept_angle = (radii2_inbound / 2.).acos().to_degrees();
            self.turn2.ftp = place_bearing_distance(self.turn2.centre, self.straight_course + turn_sign * (120. + intercept_angle), radius);
            self.turn2.sweep = turn_sign * (210. + intercept_angle);
            self.turn3.itp = self.turn2.ftp;
            self.turn3.sweep = -turn_sign * intercept_angle;
            self.turn3.centre = place_bearing_distance(self.turn3.itp, self.straight_course + turn_sign * (120. + intercept_angle), radius);
            self.turn3.ftp = place_bearing_distance(self.turn3.centre, outbound_course + turn_sign * 90., radius);
        } else {
            self.turn2.ftp = place_bearing_distance(self.turn2.centre, outbound_course - turn_sign * 45., radius);
            self.turn2.sweep = turn_sign * 255.;
            let straight_dist = (2. * (sqrt2 - radii2_inbound).powi(2)).sqrt() * radius;
            self.turn3.itp = place_bearing_distance(self.turn2.ftp, self.straight_course + turn_sign * 255., straight_dist);
            self.turn3.sweep = -turn_sign * 45.;
            self.turn3.centre = place_bearing_distance(self.turn3.itp, outbound_course - turn_sign * 45., radius);
            self.turn3.ftp = place_bearing_distance(self.turn3.centre, outbound_course + turn_sign * 90., radius);
            self.path.push(PathVector::Line { start: self.turn2.ftp, end: self.turn3.itp });
        }

        self.path.push(PathVector::Arc { start: self.turn2.itp, end: self.turn2.ftp, centre: self.turn2.centre, sweep: self.turn2.sweep });
        self.path.push(PathVector::Arc { start: self.turn3.itp, end: self.turn3.ftp, centre: self.turn3.centre, sweep: self.turn3.sweep });
        self.path.push(PathVector::Line { start: self.turn3.ftp, end: hold.fix });
    }

    fn compute_parallel(&mut self, hold: &Hold) {
        self.entry = EntryType::Parallel;
        let (radius, leg_length) = (hold.geometry.radius, hold.geometry.leg_length);
        let turn_sign = if hold.turn == TurnDirection::Right { 1. } else { -1. };
        let outbound_course = hold.course;
        let sqrt2 = std::f64::consts::SQRT_2;
        let right = hold.turn == TurnDirection::Right;
        self.turn1.itp = hold.fix;
        self.turn1.centre = place_bearing_distance(self.turn1.itp, self.inbound_course + if right { -90. } else { 90. }, radius);
        self.turn1.sweep = diff_angle(self.inbound_course, reciprocal(outbound_course));
        let bearing1 = normalise360(self.inbound_course + self.turn1.sweep + if right { 90. } else { -90. });
        self.turn1.ftp = place_bearing_distance(self.turn1.centre, bearing1, radius);
        self.path.clear();
        self.path.push(PathVector::Arc { start: self.turn1.itp, end: self.turn1.ftp, centre: self.turn1.centre, sweep: self.turn1.sweep });

        let t1 = self.turn1.sweep.abs().to_radians();
        let min_straight = 0.1 + 2. * (1. - sqrt2 / 2. - t1.sin()).cos() * radius;
        let nominal_straight = 1.15 * leg_length - radius * t1.sin();
        let straight = min_straight.max(nominal_straight);
        self.turn2.itp = place_bearing_distance(self.turn1.ftp, reciprocal(outbound_course), straight);
        self.path.push(PathVector::Line { start: self.turn1.ftp, end: self.turn2.itp });
        self.straight_course = bearing_to(self.turn1.ftp, self.turn2.itp);

        self.turn2.centre = place_bearing_distance(self.turn2.itp, outbound_course + turn_sign * 90., radius);
        self.turn2.ftp = place_bearing_distance(self.turn2.centre, outbound_course + turn_sign * 45., radius);
        self.turn2.sweep = turn_sign * -225.;
        self.path.push(PathVector::Arc { start: self.turn2.itp, end: self.turn2.ftp, centre: self.turn2.centre, sweep: self.turn2.sweep });

        let ftp2_abeam = t1.cos() + sqrt2 / 2.;
        let straight_dist = (2. * (ftp2_abeam - (1. - sqrt2 / 2.)).powi(2)).sqrt() * radius;
        self.turn3.itp = place_bearing_distance(self.turn2.ftp, outbound_course - turn_sign * 45., straight_dist);
        self.path.push(PathVector::Line { start: self.turn2.ftp, end: self.turn3.itp });
        self.turn3.sweep = turn_sign * 45.;
        self.turn3.centre = place_bearing_distance(self.turn3.itp, outbound_course + turn_sign * 45., radius);
        self.turn3.ftp = place_bearing_distance(self.turn3.centre, outbound_course - turn_sign * 90., radius);
        self.path.push(PathVector::Arc { start: self.turn3.itp, end: self.turn3.ftp, centre: self.turn3.centre, sweep: self.turn3.sweep });
        self.path.push(PathVector::Line { start: self.turn3.ftp, end: hold.fix });
    }

    pub fn recompute(&mut self, is_active: bool, has_previous: bool, inbound_course: f64, hold: &mut Hold) {
        self.inbound_course = inbound_course;
        let entry_angle = diff_angle(inbound_course, hold.course);
        self.computed = true;
        if self.frozen {
            if self.state == EntryState::Done {
                self.path.clear();
            }
            return;
        }
        if is_active {
            self.frozen = true;
        }
        if !has_previous || (-3. ..=3.).contains(&entry_angle) {
            self.entry = EntryType::Null;
            self.path.clear();
            self.set_hx_entry(hold);
            self.is_null = true;
            return;
        }
        self.is_null = false;
        match hold.turn {
            TurnDirection::Left => {
                if entry_angle > 110. && entry_angle < 177. {
                    self.compute_teardrop(hold);
                } else if (177. ..=180.).contains(&entry_angle) || (entry_angle > -180. && entry_angle < -70.) {
                    self.compute_parallel(hold);
                } else if (-70. ..-3.).contains(&entry_angle) {
                    self.compute_direct_turn(hold);
                } else {
                    self.compute_direct_outbound(hold);
                }
            }
            TurnDirection::Right => {
                if entry_angle > -177. && entry_angle < -110. {
                    self.compute_teardrop(hold);
                } else if (entry_angle > 70. && entry_angle <= 180.) || (entry_angle > -180. && entry_angle <= -177.) {
                    self.compute_parallel(hold);
                } else if entry_angle > 3. && entry_angle <= 70. {
                    self.compute_direct_turn(hold);
                } else {
                    self.compute_direct_outbound(hold);
                }
            }
            TurnDirection::Either => {}
        }
        self.set_hx_entry(hold);
    }

    pub fn guidance(&mut self, hold: &mut Hold, ppos: Coord, true_track: f64, tas: f64, gs: f64, env: &Environment) -> Option<LateralPath> {
        if !self.guidance_active {
            hold.update_prediction(env);
            self.guidance_active = true;
        }
        match self.entry {
            EntryType::Parallel | EntryType::Teardrop => Some(self.parallel_teardrop_guidance(hold, ppos, true_track, tas, gs, env)),
            EntryType::DirectOutbound => Some(hold.guidance(ppos, true_track, tas, gs, env)),
            EntryType::DirectTurn => self.direct_turn_guidance(hold, ppos, true_track, tas, gs),
            EntryType::Null => None,
        }
    }

    fn parallel_teardrop_guidance(&mut self, hold: &mut Hold, ppos: Coord, true_track: f64, tas: f64, gs: f64, env: &Environment) -> LateralPath {
        let gs_ms = gs * 463. / 900.;
        let mut dtg = f64::NAN;
        match self.state {
            EntryState::Turn1 => {
                dtg = arc_distance_to_go(ppos, self.turn1.itp, self.turn1.centre, self.turn1.sweep);
                if dtg <= 0. {
                    self.state = EntryState::Straight1;
                }
            }
            EntryState::Straight1 => {
                dtg = course_to_fix_distance_to_go(ppos, self.straight_course, self.turn2.itp);
                if dtg <= 0. {
                    self.state = EntryState::Turn2;
                }
            }
            EntryState::Turn2 => {
                dtg = arc_distance_to_go(ppos, self.turn2.itp, self.turn2.centre, self.turn2.sweep);
                let offset = diff_angle(0., hold.course);
                let tae = if self.turn2.sweep < 0. { normalise360(offset - true_track) } else { normalise360(true_track - offset) };
                if tae < 130. {
                    self.state = EntryState::Capture;
                }
            }
            EntryState::Capture => {
                dtg = course_to_fix_distance_to_go(ppos, hold.course, hold.fix);
                if dtg < 0.1 {
                    hold.update_prediction(env);
                    self.state = EntryState::Done;
                }
            }
            EntryState::Done => {}
        }
        let mut bank_next = 0.;
        let mut params = match self.state {
            EntryState::Turn1 => arc_guidance(ppos, true_track, self.turn1.itp, self.turn1.centre, self.turn1.sweep, gs_ms),
            EntryState::Straight1 => {
                bank_next = if self.turn2.sweep > 0. { max_bank(tas, true) } else { -max_bank(tas, true) };
                course_to_fix_guidance(ppos, true_track, self.straight_course, self.turn2.itp)
            }
            EntryState::Turn2 => {
                let phi = if self.turn2.sweep > 0. { max_bank(tas, true) } else { -max_bank(tas, true) };
                bank_next = phi;
                LateralPath { cross_track_error: 0., track_angle_error: 0., phi_command: phi, phi_limit: 0., heading: None }
            }
            EntryState::Capture => {
                bank_next = if hold.geometry.sweep > 0. { max_bank(tas, true) } else { -max_bank(tas, true) };
                course_to_fix_guidance(ppos, true_track, hold.course, hold.fix)
            }
            EntryState::Done => {
                let p = hold.guidance(ppos, true_track, tas, gs, env);
                bank_next = p.phi_command;
                p
            }
        };
        let rad = roll_anticipation_distance(tas, params.phi_command, bank_next);
        if rad > 0. && dtg <= rad {
            params.phi_command = bank_next;
        }
        params
    }

    fn direct_turn_guidance(&mut self, hold: &Hold, ppos: Coord, true_track: f64, tas: f64, gs: f64) -> Option<LateralPath> {
        let gs_ms = gs * 463. / 900.;
        let mut dtg = f64::NAN;
        match self.state {
            EntryState::Turn1 => {
                dtg = arc_distance_to_go(ppos, self.turn1.itp, self.turn1.centre, self.turn1.sweep);
                if dtg <= 0. {
                    self.state = EntryState::Straight1;
                }
            }
            EntryState::Straight1 => {
                dtg = course_to_fix_distance_to_go(ppos, self.straight_course, self.turn2.itp);
                if dtg <= 0. {
                    self.state = EntryState::Turn2;
                }
            }
            EntryState::Turn2 => {
                dtg = arc_distance_to_go(ppos, self.turn2.itp, self.turn2.centre, self.turn2.sweep);
                if dtg <= 0. {
                    self.state = EntryState::Capture;
                }
            }
            EntryState::Capture => {
                dtg = course_to_fix_distance_to_go(ppos, hold.course, hold.fix);
                if dtg < 0.1 {
                    self.state = EntryState::Done;
                }
            }
            EntryState::Done => {}
        }
        let (mut params, bank_next) = match self.state {
            EntryState::Turn1 => (arc_guidance(ppos, true_track, self.turn1.itp, self.turn1.centre, self.turn1.sweep, gs_ms), 0.),
            EntryState::Straight1 => (
                course_to_fix_guidance(ppos, true_track, self.straight_course, self.turn2.itp),
                if self.turn2.sweep > 0. { max_bank(tas, true) } else { -max_bank(tas, true) },
            ),
            EntryState::Turn2 => (arc_guidance(ppos, true_track, self.turn2.itp, self.turn2.centre, self.turn2.sweep, gs_ms), 0.),
            EntryState::Capture => (course_to_fix_guidance(ppos, true_track, hold.course, hold.fix), 0.),
            EntryState::Done => return None,
        };
        let rad = roll_anticipation_distance(tas, params.phi_command, bank_next);
        if rad > 0. && dtg <= rad {
            params.phi_command = bank_next;
        }
        Some(params)
    }
}

use crate::geo::*;

pub const G: f64 = 9.81;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathVector {
    Line { start: Coord, end: Coord },
    Arc { start: Coord, end: Coord, centre: Coord, sweep: f64 },
}

impl PathVector {
    pub fn start(&self) -> Coord {
        match self {
            PathVector::Line { start, .. } | PathVector::Arc { start, .. } => *start,
        }
    }

    pub fn end(&self) -> Coord {
        match self {
            PathVector::Line { end, .. } | PathVector::Arc { end, .. } => *end,
        }
    }

    pub fn length(&self) -> f64 {
        match self {
            PathVector::Line { start, end } => distance_to(*start, *end),
            PathVector::Arc { start, centre, sweep, .. } => arc_length(distance_to(*start, *centre), *sweep),
        }
    }

    pub fn is_valid(&self) -> bool {
        match self {
            PathVector::Line { start, end } => start.is_finite() && end.is_finite(),
            PathVector::Arc { end, centre, sweep, .. } => end.is_finite() && centre.is_finite() && sweep.is_finite(),
        }
    }

    pub fn point(&self, distance_from_end: f64) -> Coord {
        match self {
            PathVector::Line { start, end } => get_intermediate_point(*end, *start, distance_from_end / self.length()),
            PathVector::Arc { end, centre, sweep, .. } => point_on_arc(distance_from_end, *end, *centre, *sweep),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        let c = |p: Coord| serde_json::json!({ "lat": p.lat, "long": p.long });
        match self {
            PathVector::Line { start, end } => serde_json::json!({ "type": 0, "startPoint": c(*start), "endPoint": c(*end) }),
            PathVector::Arc { start, end, centre, sweep } => {
                serde_json::json!({ "type": 1, "startPoint": c(*start), "centrePoint": c(*centre), "endPoint": c(*end), "sweepAngle": sweep })
            }
        }
    }
}

pub fn vectors_to_json(vectors: &[PathVector]) -> serde_json::Value {
    serde_json::Value::Array(vectors.iter().map(PathVector::to_json).collect())
}

pub fn arc_length(radius: f64, sweep: f64) -> f64 {
    2. * std::f64::consts::PI * radius / 360. * sweep.abs()
}

pub fn reciprocal(course: f64) -> f64 {
    normalise360(course + 180.)
}

pub fn point_on_arc(distance_from_ftp: f64, ftp: Coord, centre: Coord, sweep: f64) -> Coord {
    let radius = distance_to(centre, ftp);
    let ratio = distance_from_ftp / arc_length(radius, sweep);
    place_bearing_distance(centre, normalise360(bearing_to(centre, ftp) - ratio * sweep), radius)
}

pub fn arc_distance_to_go(ppos: Coord, itp: Coord, centre: Coord, sweep: f64) -> f64 {
    let itp_bearing = bearing_to(centre, itp);
    let ppos_bearing = bearing_to(centre, ppos);
    let radius = distance_to(centre, itp);
    let offset = diff_angle(0., itp_bearing);
    let ppos_angle = if sweep < 0. { normalise360(offset - ppos_bearing) } else { normalise360(ppos_bearing - offset) };
    if ppos_angle >= 340. {
        return radius * std::f64::consts::PI * sweep.abs() / 180.;
    }
    if ppos_angle >= sweep.abs() {
        return 0.;
    }
    radius * std::f64::consts::PI * (sweep.abs() - ppos_angle) / 180.
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LateralPath {
    pub cross_track_error: f64,
    pub track_angle_error: f64,
    pub phi_command: f64,
    pub phi_limit: f64,
    pub heading: Option<f64>,
}

pub fn arc_guidance(ppos: Coord, true_track: f64, itp: Coord, centre: Coord, sweep: f64, gs_ms: f64) -> LateralPath {
    let bearing_ppos = bearing_to(centre, ppos);
    let desired = if sweep > 0. { normalise360(bearing_ppos + 90.) } else { normalise360(bearing_ppos - 90.) };
    let tae = diff_angle(true_track, desired);
    let radius = distance_to(centre, itp);
    let from_centre = distance_to(centre, ppos);
    let xte = if sweep > 0. { from_centre - radius } else { radius - from_centre };
    let phi = (if sweep > 0. { 1. } else { -1. }) * (gs_ms * gs_ms / (radius * 1852. * 9.81)).atan().to_degrees();
    LateralPath { cross_track_error: xte, track_angle_error: tae, phi_command: phi, phi_limit: 0., heading: None }
}

pub fn course_to_fix_distance_to_go(ppos: Coord, course: f64, fix: Coord) -> f64 {
    let bearing = bearing_to(ppos, fix);
    let dist = distance_to(ppos, fix);
    let angle = diff_angle(bearing, course);
    (dist * angle.to_radians().cos()).max(0.)
}

pub fn course_to_fix_guidance(ppos: Coord, true_track: f64, course: f64, fix: Coord) -> LateralPath {
    let bearing = bearing_to(ppos, fix);
    let dist = distance_to(ppos, fix);
    let angle = diff_angle(course, bearing);
    LateralPath {
        cross_track_error: dist * angle.to_radians().sin(),
        track_angle_error: diff_angle(true_track, course),
        phi_command: 0.,
        phi_limit: 0.,
        heading: None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointSide {
    Before,
    After,
}

pub fn side_of_point_on_course_to_fix(fix: Coord, course: f64, point: Coord) -> PointSide {
    let bearing = bearing_to(fix, point);
    if diff_angle(bearing, course).abs() < 3. {
        PointSide::After
    } else {
        PointSide::Before
    }
}

pub fn fix_to_fix_guidance(ppos: Coord, true_track: f64, from: Coord, to: Coord) -> LateralPath {
    let total = distance_to(from, to);
    let along = along_track_distance(from, to, ppos);
    let mid = get_intermediate_point(from, to, (along / total).clamp(0.05, 0.95));
    let desired = bearing_to(mid, to);
    let tae = ((desired - true_track + 180.) % 360. + 360.) % 360. - 180.;
    let bearing_ac = bearing_to(from, ppos);
    let bearing_ab = bearing_to(from, to);
    let distance_ac = distance_to(from, ppos);
    let actual = ((distance_ac / EARTH_RADIUS_NM).to_radians().sin() * (bearing_ac - bearing_ab).to_radians().sin()).asin().to_degrees()
        * EARTH_RADIUS_NM;
    LateralPath { cross_track_error: -actual, track_angle_error: tae, phi_command: 0., phi_limit: 0., heading: None }
}

pub fn min_bank(enroute: bool) -> f64 {
    if enroute {
        5.
    } else {
        10.
    }
}

pub fn max_bank(tas: f64, path_capture: bool) -> f64 {
    if path_capture {
        if tas < 100. {
            return 15. + tas / 10.;
        }
        if tas > 350. {
            return 19. + ((450. - tas) * 6. / 100.).max(0.);
        }
        return 25.;
    }
    if tas < 150. {
        return 15. + tas / 10.;
    }
    if tas > 300. {
        return 19. + ((450. - tas) * 11. / 150.).max(0.);
    }
    30.
}

pub fn max_tad(tas: Option<f64>) -> f64 {
    match tas {
        None => 10.,
        Some(t) if t <= 100. => 4.,
        Some(t) if t <= 400. => t / 100. * 4.,
        _ => 16.,
    }
}

pub fn roll_anticipation_distance(gs: f64, bank_a: f64, bank_b: f64) -> f64 {
    let delta = (bank_a - bank_b).abs();
    let max_roll_rate = 5.;
    let k2 = 0.0038;
    (gs / 3600.) * ((1. + 2. * k2 * 9.81 * delta / max_roll_rate).sqrt() - 1.) / (k2 * 9.81)
}

pub mod course_change {
    pub fn normal(turn_direction: f64, turn_center_distance: f64, track_change: f64, radius: f64) -> f64 {
        if turn_direction > 0. {
            if turn_center_distance >= radius {
                return track_change - 45.;
            }
            return track_change + 45.;
        }
        if -turn_center_distance >= radius {
            return track_change + 45.;
        }
        track_change - 45.
    }

    pub fn reverse(_turn_direction: f64, turn_center_distance: f64, track_change: f64, radius: f64) -> f64 {
        if track_change > 0. {
            if turn_center_distance > 0. {
                if turn_center_distance > radius {
                    return track_change - 45.;
                }
                return track_change + 45.;
            }
            return track_change + 45.;
        }
        if turn_center_distance > 0. {
            return track_change - 45.;
        }
        if -turn_center_distance > radius {
            return track_change + 45.;
        }
        track_change - 45.
    }

    pub fn acute_far(turn_direction: f64, _turn_center_distance: f64, track_change: f64) -> f64 {
        turn_direction * (45. - track_change.abs())
    }

    pub fn acute_near(turn_direction: f64, _turn_center_distance: f64, track_change: f64) -> f64 {
        track_change + if turn_direction > 0. { 45. } else { -45. }
    }
}

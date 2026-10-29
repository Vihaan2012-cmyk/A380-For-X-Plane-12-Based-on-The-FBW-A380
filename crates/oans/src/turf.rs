use crate::geo::Position;

pub const EARTH_RADIUS_M: f64 = 6371008.8;
pub const DEGREES_FACTOR: f64 = 360.0 / (2.0 * std::f64::consts::PI);

pub fn degrees_to_radians(degrees: f64) -> f64 {
    (degrees % 360.0) * std::f64::consts::PI / 180.0
}

pub fn radians_to_degrees(radians: f64) -> f64 {
    (radians % (2.0 * std::f64::consts::PI)) * 180.0 / std::f64::consts::PI
}

pub fn metres_to_degrees(metres: f64) -> f64 {
    radians_to_degrees(metres / EARTH_RADIUS_M)
}

pub fn distance_degrees(from: Position, to: Position) -> f64 {
    let d_lat = degrees_to_radians(to[1] - from[1]);
    let d_lon = degrees_to_radians(to[0] - from[0]);
    let lat1 = degrees_to_radians(from[1]);
    let lat2 = degrees_to_radians(to[1]);
    let a = (d_lat / 2.0).sin().powi(2) + (d_lon / 2.0).sin().powi(2) * lat1.cos() * lat2.cos();
    2.0 * a.sqrt().atan2((1.0 - a).sqrt()) * DEGREES_FACTOR
}

pub fn length_degrees(line: &[Position]) -> f64 {
    line.windows(2).map(|w| distance_degrees(w[0], w[1])).sum()
}

pub fn bearing(start: Position, end: Position) -> f64 {
    let lon1 = degrees_to_radians(start[0]);
    let lon2 = degrees_to_radians(end[0]);
    let lat1 = degrees_to_radians(start[1]);
    let lat2 = degrees_to_radians(end[1]);
    let a = (lon2 - lon1).sin() * lat2.cos();
    let b = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * (lon2 - lon1).cos();
    radians_to_degrees(a.atan2(b))
}

pub fn destination_degrees(origin: Position, distance: f64, bearing_deg: f64) -> Position {
    let lon1 = degrees_to_radians(origin[0]);
    let lat1 = degrees_to_radians(origin[1]);
    let brg = degrees_to_radians(bearing_deg);
    let r = distance / DEGREES_FACTOR;
    let lat2 = (lat1.sin() * r.cos() + lat1.cos() * r.sin() * brg.cos()).asin();
    let lon2 = lon1 + (brg.sin() * r.sin() * lat1.cos()).atan2(r.cos() - lat1.sin() * lat2.sin());
    [radians_to_degrees(lon2), radians_to_degrees(lat2)]
}

pub fn along_degrees(line: &[Position], distance: f64) -> Position {
    let mut travelled = 0.0;
    for i in 0..line.len() {
        if distance >= travelled && i == line.len() - 1 {
            break;
        } else if travelled >= distance {
            let overshot = distance - travelled;
            if overshot == 0.0 {
                return line[i];
            }
            let direction = bearing(line[i], line[i - 1]) - 180.0;
            return destination_degrees(line[i], overshot, direction);
        } else {
            travelled += distance_degrees(line[i], line[i + 1]);
        }
    }
    line[line.len() - 1]
}

pub fn line_offset_segment(p1: Position, p2: Position, offset_degrees: f64) -> [Position; 2] {
    let l = ((p1[0] - p2[0]) * (p1[0] - p2[0]) + (p1[1] - p2[1]) * (p1[1] - p2[1])).sqrt();
    let out1x = p1[0] + offset_degrees * (p2[1] - p1[1]) / l;
    let out2x = p2[0] + offset_degrees * (p2[1] - p1[1]) / l;
    let out1y = p1[1] + offset_degrees * (p1[0] - p2[0]) / l;
    let out2y = p2[1] + offset_degrees * (p1[0] - p2[0]) / l;
    [[out1x, out1y], [out2x, out2y]]
}

fn orient(u1: f64, u2: f64, v1: f64, v2: f64) -> f64 {
    u2 * v1 - u1 * v2
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pip {
    Outside,
    Boundary,
    Inside,
}

pub fn point_in_rings(p: Position, rings: &[Vec<Position>]) -> Pip {
    let mut k = 0usize;
    let (x, y) = (p[0], p[1]);
    for contour in rings {
        if contour.len() < 2 {
            continue;
        }
        let len = contour.len() - 1;
        let mut u1 = contour[0][0] - x;
        let mut v1 = contour[0][1] - y;
        for next in &contour[1..=len] {
            let u2 = next[0] - x;
            let v2 = next[1] - y;
            if v1 == 0.0 && v2 == 0.0 {
                if (u2 <= 0.0 && u1 >= 0.0) || (u1 <= 0.0 && u2 >= 0.0) {
                    return Pip::Boundary;
                }
            } else if (v2 >= 0.0 && v1 <= 0.0) || (v2 <= 0.0 && v1 >= 0.0) {
                let f = orient(u1, u2, v1, v2);
                if f == 0.0 {
                    return Pip::Boundary;
                }
                if (f > 0.0 && v2 > 0.0 && v1 <= 0.0) || (f < 0.0 && v2 <= 0.0 && v1 > 0.0) {
                    k += 1;
                }
            }
            v1 = v2;
            u1 = u2;
        }
    }
    if k % 2 == 0 {
        Pip::Outside
    } else {
        Pip::Inside
    }
}

pub fn boolean_point_in_polygon(p: Position, rings: &[Vec<Position>], ignore_boundary: bool) -> bool {
    match point_in_rings(p, rings) {
        Pip::Boundary => !ignore_boundary,
        Pip::Inside => true,
        Pip::Outside => false,
    }
}

fn segments_intersect(a1: Position, a2: Position, b1: Position, b2: Position) -> bool {
    let denom = (b2[1] - b1[1]) * (a2[0] - a1[0]) - (b2[0] - b1[0]) * (a2[1] - a1[1]);
    if denom == 0.0 {
        return false;
    }
    let ua = ((b2[0] - b1[0]) * (a1[1] - b1[1]) - (b2[1] - b1[1]) * (a1[0] - b1[0])) / denom;
    let ub = ((a2[0] - a1[0]) * (a1[1] - b1[1]) - (a2[1] - a1[1]) * (a1[0] - b1[0])) / denom;
    (0.0..=1.0).contains(&ua) && (0.0..=1.0).contains(&ub)
}

fn rings_cross(a: &[Vec<Position>], b: &[Vec<Position>]) -> bool {
    for ra in a {
        for sa in ra.windows(2) {
            for rb in b {
                for sb in rb.windows(2) {
                    if segments_intersect(sa[0], sa[1], sb[0], sb[1]) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

pub fn polygons_disjoint(a: &[Vec<Position>], b: &[Vec<Position>]) -> bool {
    if a.is_empty() || b.is_empty() {
        return true;
    }
    if b[0].iter().any(|p| boolean_point_in_polygon(*p, a, false)) {
        return false;
    }
    if a[0].iter().any(|p| boolean_point_in_polygon(*p, b, false)) {
        return false;
    }
    !rings_cross(b, a)
}

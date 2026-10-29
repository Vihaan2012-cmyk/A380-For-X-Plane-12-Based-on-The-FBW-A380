use crate::geo::{clamp_angle, Position};

pub fn fractional_point_along_line(x1: f64, y1: f64, x2: f64, y2: f64, fraction: f64) -> [f64; 2] {
    [x1 + (x2 - x1) * fraction, y1 + (y2 - y1) * fraction]
}

pub fn mid_point(x1: f64, y1: f64, x2: f64, y2: f64) -> [f64; 2] {
    fractional_point_along_line(x1, y1, x2, y2, 0.5)
}

pub fn point_angle(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    clamp_angle((y2 - y1).atan2(x2 - x1).to_degrees())
}

#[allow(clippy::too_many_arguments)]
fn line_line(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64) -> Option<[f64; 2]> {
    let den = (y4 - y3) * (x2 - x1) - (x4 - x3) * (y2 - y1);
    let ua = ((x4 - x3) * (y1 - y3) - (y4 - y3) * (x1 - x3)) / den;
    let ub = ((x2 - x1) * (y1 - y3) - (y2 - y1) * (x1 - x3)) / den;
    if (0.0..=1.0).contains(&ua) && (0.0..=1.0).contains(&ub) {
        Some([x1 + ua * (x2 - x1), y1 + ua * (y2 - y1)])
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
pub fn intersect_line_with_rectangle(x1: f64, y1: f64, x2: f64, y2: f64, rx: f64, ry: f64, rw: f64, rh: f64) -> Vec<[f64; 2]> {
    let left = line_line(x1, y1, x2, y2, rx, ry, rx, ry + rh);
    let right = line_line(x1, y1, x2, y2, rx + rw, ry, rx + rw, ry + rh);
    let top = line_line(x1, y1, x2, y2, rx, ry, rx + rw, ry);
    let bottom = line_line(x1, y1, x2, y2, rx, ry + rh, rx + rw, ry + rh);
    [left, right, top, bottom].into_iter().flatten().collect()
}

pub fn is_point_in_rectangle(px: f64, py: f64, rx: f64, ry: f64, rw: f64, rh: f64) -> bool {
    px >= rx && px <= rx + rw && py >= ry && ry <= ry + rh
}

pub fn point_to_line_distance(point: Position, line_start: Position, line_end: Position) -> f64 {
    ((line_end[1] - line_start[1]) * point[0] - (line_end[0] - line_start[0]) * point[1] + line_end[0] * line_start[1]
        - line_end[1] * line_start[0])
        .abs()
        / ((line_end[1] - line_start[1]).powi(2) + (line_end[0] - line_start[0]).powi(2)).powf(0.5)
}

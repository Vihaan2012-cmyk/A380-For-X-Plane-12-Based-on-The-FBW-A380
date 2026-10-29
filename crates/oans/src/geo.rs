#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Coordinates {
    pub lat: f64,
    pub long: f64,
}

pub type Position = [f64; 2];

pub const EARTH_RADIUS_NM: f64 = 3443.91846652;
pub const METRES_TO_NAUTICAL_MILES: f64 = 1852.0;
pub const FEET_TO_NAUTICAL_MILES: f64 = 6076.12;

fn sind(a: f64) -> f64 {
    a.to_radians().sin()
}

fn cosd(a: f64) -> f64 {
    a.to_radians().cos()
}

fn robust_acos(v: f64) -> f64 {
    v.clamp(-1.0, 1.0).acos()
}

pub fn bearing_to(from: Coordinates, to: Coordinates) -> f64 {
    let y = sind(to.long - from.long) * cosd(to.lat);
    let x = cosd(from.lat) * sind(to.lat) - sind(from.lat) * cosd(to.lat) * cosd(to.long - from.long);
    (y.atan2(x).to_degrees() + 360.0) % 360.0
}

pub fn distance_to(from: Coordinates, to: Coordinates) -> f64 {
    robust_acos(sind(to.lat) * sind(from.lat) + cosd(to.lat) * cosd(from.lat) * cosd(from.long - to.long)) * EARTH_RADIUS_NM
}

pub fn place_bearing_distance(place: Coordinates, bearing: f64, distance: f64) -> Coordinates {
    let delta = distance / EARTH_RADIUS_NM;
    let lat = (sind(place.lat) * delta.cos() + cosd(place.lat) * delta.sin() * cosd(bearing)).asin().to_degrees();
    let mut long =
        place.long + (sind(bearing) * delta.sin() * cosd(place.lat)).atan2(delta.cos() - sind(place.lat) * sind(lat)).to_degrees();
    if !(-180.0..=180.0).contains(&long) {
        long = ((long + 540.0) % 360.0) - 180.0;
    }
    Coordinates { lat, long }
}

pub fn clamp_angle(mut a: f64) -> f64 {
    while a >= 360.0 {
        a -= 360.0;
    }
    while a < 0.0 {
        a += 360.0;
    }
    a
}

pub fn diff_angle(a: f64, b: f64) -> f64 {
    let mut diff = b - a;
    while diff > 180.0 {
        diff -= 360.0;
    }
    while diff <= -180.0 {
        diff += 360.0;
    }
    diff
}

pub fn normalise_360(angle: f64) -> f64 {
    ((angle % 360.0) + 360.0) % 360.0
}

pub fn reciprocal(course: f64) -> f64 {
    normalise_360(course + 180.0)
}

pub fn point_distance(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    (x2 - x1).hypot(y2 - y1)
}

pub fn round_to(value: f64, quantum: f64) -> f64 {
    js_round(value / quantum) * quantum
}

pub fn js_round(v: f64) -> f64 {
    (v + 0.5).floor()
}

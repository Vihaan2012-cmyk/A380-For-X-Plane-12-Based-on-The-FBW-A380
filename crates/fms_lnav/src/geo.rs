#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Coord {
    pub lat: f64,
    pub long: f64,
}

impl Coord {
    pub fn new(lat: f64, long: f64) -> Self {
        Self { lat, long }
    }

    pub fn is_finite(&self) -> bool {
        self.lat.is_finite() && self.long.is_finite()
    }
}

pub const EARTH_RADIUS_NM: f64 = 6_378_137.0 / 1852.0;

type V3 = [f64; 3];

fn to_vec(c: Coord) -> V3 {
    let (la, lo) = (c.lat.to_radians(), c.long.to_radians());
    [la.cos() * lo.cos(), la.cos() * lo.sin(), la.sin()]
}

fn from_vec(v: V3) -> Coord {
    Coord { lat: v[2].atan2((v[0] * v[0] + v[1] * v[1]).sqrt()).to_degrees(), long: v[1].atan2(v[0]).to_degrees() }
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn scale(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn unit(a: V3) -> V3 {
    let n = dot(a, a).sqrt();
    scale(a, 1. / n)
}

pub fn normalise360(a: f64) -> f64 {
    ((a % 360.) + 360.) % 360.
}

pub fn diff_angle(a: f64, b: f64) -> f64 {
    let mut d = b - a;
    while d > 180. {
        d -= 360.;
    }
    while d <= -180. {
        d += 360.;
    }
    d
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnDirection {
    Either,
    Left,
    Right,
}

pub fn diff_angle_dir(a: f64, b: f64, dir: TurnDirection) -> f64 {
    let mut d = diff_angle(a, b);
    if d < 0. && dir == TurnDirection::Right {
        d += 360.;
    }
    if d > 0. && dir == TurnDirection::Left {
        d -= 360.;
    }
    d
}

pub fn distance_to(a: Coord, b: Coord) -> f64 {
    let (p1, p2) = (a.lat.to_radians(), b.lat.to_radians());
    let dl = (b.long - a.long).to_radians();
    let h = ((p2 - p1) / 2.).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.).sin().powi(2);
    2. * EARTH_RADIUS_NM * h.sqrt().min(1.).asin()
}

pub fn bearing_to(a: Coord, b: Coord) -> f64 {
    let (p1, p2) = (a.lat.to_radians(), b.lat.to_radians());
    let dl = (b.long - a.long).to_radians();
    let y = dl.sin() * p2.cos();
    let x = p1.cos() * p2.sin() - p1.sin() * p2.cos() * dl.cos();
    normalise360(y.atan2(x).to_degrees())
}

pub fn place_bearing_distance(start: Coord, bearing: f64, dist_nm: f64) -> Coord {
    let d = dist_nm / EARTH_RADIUS_NM;
    let b = bearing.to_radians();
    let p1 = start.lat.to_radians();
    let p2 = (p1.sin() * d.cos() + p1.cos() * d.sin() * b.cos()).asin();
    let l2 = start.long.to_radians() + (b.sin() * d.sin() * p1.cos()).atan2(d.cos() - p1.sin() * p2.sin());
    Coord { lat: p2.to_degrees(), long: ((l2.to_degrees() + 540.) % 360.) - 180. }
}

pub fn get_intermediate_point(start: Coord, end: Coord, fraction: f64) -> Coord {
    let (p1, l1) = (start.lat.to_radians(), start.long.to_radians());
    let (p2, l2) = (end.lat.to_radians(), end.long.to_radians());
    let dp = p2 - p1;
    let dl = l2 - l1;
    let a = (dp / 2.).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.).sin().powi(2);
    let delta = 2. * a.sqrt().atan2((1. - a).sqrt());
    let ka = ((1. - fraction) * delta).sin() / delta.sin();
    let kb = (fraction * delta).sin() / delta.sin();
    let x = ka * p1.cos() * l1.cos() + kb * p2.cos() * l2.cos();
    let y = ka * p1.cos() * l1.sin() + kb * p2.cos() * l2.sin();
    let z = ka * p1.sin() + kb * p2.sin();
    Coord { lat: z.atan2((x * x + y * y).sqrt()).to_degrees(), long: y.atan2(x).to_degrees() }
}

fn bearing_normal(p: V3, bearing: f64) -> V3 {
    let north = unit([-p[0] * p[2], -p[1] * p[2], p[0] * p[0] + p[1] * p[1]]);
    let east = unit(cross([0., 0., 1.], p));
    let b = bearing.to_radians();
    let dir = add(scale(north, b.cos()), scale(east, b.sin()));
    unit(cross(p, dir))
}

pub fn place_bearing_intersection(p1: Coord, b1: f64, p2: Coord, b2: f64) -> [Coord; 2] {
    let v1 = to_vec(p1);
    let v2 = to_vec(p2);
    let x = unit(cross(bearing_normal(v1, b1), bearing_normal(v2, b2)));
    let (s1, s2) = (from_vec(x), from_vec(scale(x, -1.)));
    let b1 = normalise360(b1);
    if (b1 - bearing_to(p1, s1)).abs() < (b1 - bearing_to(p1, s2)).abs() {
        [s1, s2]
    } else {
        [s2, s1]
    }
}

pub fn small_circle_great_circle_intersection(centre: Coord, radius_nm: f64, point: Coord, bearing: f64) -> Option<[Coord; 2]> {
    let c = to_vec(centre);
    let n = bearing_normal(to_vec(point), bearing);
    let k = dot(n, c);
    let cos_r = (radius_nm / EARTH_RADIUS_NM).cos();
    let beta = cos_r / (1. - k * k);
    let alpha = -k * beta;
    let base = add(scale(n, alpha), scale(c, beta));
    let rem = 1. - dot(base, base);
    if rem < 0. {
        return None;
    }
    let u = cross(n, c);
    let g = (rem / dot(u, u)).sqrt();
    Some([from_vec(add(base, scale(u, g))), from_vec(add(base, scale(u, -g)))])
}

pub fn first_small_circle_intersection(centre: Coord, radius_nm: f64, point: Coord, bearing: f64) -> Option<Coord> {
    let [a, b] = small_circle_great_circle_intersection(centre, radius_nm, point, bearing)?;
    let ahead = |x: Coord| diff_angle(bearing_to(point, x), bearing).abs() < 90.;
    match (ahead(a), ahead(b)) {
        (true, false) => Some(a),
        (false, true) => Some(b),
        _ => Some(if distance_to(point, a) <= distance_to(point, b) { a } else { b }),
    }
}

pub fn along_track_distance(start: Coord, end: Coord, ppos: Coord) -> f64 {
    let d13 = distance_to(start, ppos) / EARTH_RADIUS_NM;
    let t13 = bearing_to(start, ppos).to_radians();
    let t12 = bearing_to(start, end).to_radians();
    let dxt = (d13.sin() * (t13 - t12).sin()).asin();
    let dat = (d13.cos() / dxt.cos().abs()).min(1.).acos();
    dat * (t12 - t13).cos().signum() * EARTH_RADIUS_NM
}

pub fn cross_track_distance(start: Coord, end: Coord, ppos: Coord) -> f64 {
    let d = distance_to(start, ppos) / EARTH_RADIUS_NM;
    (d.sin() * (bearing_to(start, ppos) - bearing_to(start, end)).to_radians().sin()).asin() * EARTH_RADIUS_NM
}

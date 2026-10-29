const DEG: f64 = std::f64::consts::PI / 180.0;
const RAD: f64 = 180.0 / std::f64::consts::PI;

pub fn great_circle_distance(lat0: f64, lon0: f64, lat1: f64, lon1: f64) -> f64 {
    let lat0 = lat0 * DEG;
    let lon0 = lon0 * DEG;
    let lat1 = lat1 * DEG;
    let lon1 = lon1 * DEG;
    let dlon = lon1 - lon0;
    let a1 = ((lat1 - lat0) / 2.0).sin();
    let a2 = (dlon / 2.0).sin();
    (a1 * a1 + lat0.cos() * lat1.cos() * a2 * a2).sqrt().asin() * 6880.126
}

pub fn distance_3d(lat0: f64, lon0: f64, alt0: f64, lat1: f64, lon1: f64, alt1: f64) -> f64 {
    let earth_radius = 3440.065;
    let r1 = alt0 / 6076.0 + earth_radius;
    let r2 = alt1 / 6076.0 + earth_radius;
    let x1 = r1 * (DEG * (lat0 + 90.0)).sin() * (DEG * (lon0 + 180.0)).cos();
    let y1 = r1 * (DEG * (lat0 + 90.0)).sin() * (DEG * (lon0 + 180.0)).sin();
    let z1 = r1 * (DEG * (lat0 + 90.0)).cos();
    let x2 = r2 * (DEG * (lat1 + 90.0)).sin() * (DEG * (lon1 + 180.0)).cos();
    let y2 = r2 * (DEG * (lat1 + 90.0)).sin() * (DEG * (lon1 + 180.0)).sin();
    let z2 = r2 * (DEG * (lat1 + 90.0)).cos();
    ((x1 - x2).powi(2) + (y1 - y2).powi(2) + (z1 - z2).powi(2)).sqrt()
}

pub fn great_circle_heading(lat0: f64, lon0: f64, lat1: f64, lon1: f64) -> f64 {
    let lat0 = lat0 * DEG;
    let lon0 = lon0 * DEG;
    let lat1 = lat1 * DEG;
    let lon1 = lon1 * DEG;
    let dlon = lon1 - lon0;
    let cos_lat1 = lat1.cos();
    let mut x = (lat1 - lat0).sin();
    let sin_lon2 = (dlon / 2.0).sin();
    x += sin_lon2 * sin_lon2 * 2.0 * lat0.sin() * cos_lat1;
    let mut heading = (cos_lat1 * dlon.sin()).atan2(x);
    if heading < 0.0 {
        heading += 2.0 * std::f64::consts::PI;
    }
    heading * RAD
}

pub fn point_in_ellipse(x: f64, y: f64, x_pos: f64, y_pos: f64, x_neg: f64, y_neg: f64) -> bool {
    x.powi(2) / (if x >= 0.0 { x_pos } else { x_neg }).powi(2) + y.powi(2) / (if y >= 0.0 { y_pos } else { y_neg }).powi(2)
        <= 1.0
}

pub fn falsy(x: f64) -> bool {
    x == 0.0 || x.is_nan()
}

pub fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a < b {
        a
    } else {
        b
    }
}

pub fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a > b {
        a
    } else {
        b
    }
}

pub fn js_round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let f = x.floor();
    if x - f >= 0.5 {
        f + 1.0
    } else {
        f
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ssm {
    FailureWarning,
    NoComputedData,
    FunctionalTest,
    NormalOperation,
}

#[derive(Clone, Copy, Debug)]
pub struct Arinc429 {
    pub ssm: Ssm,
    pub value: f64,
}

impl Arinc429 {
    pub fn from_raw(raw: f64) -> Self {
        let bits = (raw as i64 as u64 & 0xffff_ffff) as u32;
        let ssm = match ((raw / 4_294_967_296.0).trunc() as i64) & 0b11 {
            0 => Ssm::FailureWarning,
            1 => Ssm::NoComputedData,
            2 => Ssm::FunctionalTest,
            _ => Ssm::NormalOperation,
        };
        Self { ssm, value: f32::from_bits(bits) as f64 }
    }

    pub fn to_raw(value: f32, ssm: Ssm) -> f64 {
        let s = match ssm {
            Ssm::FailureWarning => 0.0,
            Ssm::NoComputedData => 1.0,
            Ssm::FunctionalTest => 2.0,
            Ssm::NormalOperation => 3.0,
        };
        value.to_bits() as f64 + s * 4_294_967_296.0
    }

    pub fn is_failure_warning(&self) -> bool {
        self.ssm == Ssm::FailureWarning
    }

    pub fn is_no_computed_data(&self) -> bool {
        self.ssm == Ssm::NoComputedData
    }

    pub fn is_functional_test(&self) -> bool {
        self.ssm == Ssm::FunctionalTest
    }

    pub fn is_normal_operation(&self) -> bool {
        self.ssm == Ssm::NormalOperation
    }
}

/// `Array.prototype.sort` as V8's TimSort runs it on arrays shorter than 64
/// elements (run detection followed by binary insertion), with the comparator
/// result kept as a raw number so a NaN result behaves as it does in V8.
pub fn js_sort<T: Clone>(items: &mut Vec<T>, cmp: impl Fn(&T, &T) -> f64) {
    let n = items.len();
    if n < 2 {
        return;
    }
    let mut run = 2;
    let order = cmp(&items[1], &items[0]);
    let descending = order < 0.0;
    let mut idx = 2;
    while idx < n {
        let order = cmp(&items[idx], &items[idx - 1]);
        if descending {
            if order >= 0.0 {
                break;
            }
        } else if order < 0.0 {
            break;
        }
        run += 1;
        idx += 1;
    }
    if descending {
        items[..run].reverse();
    }
    for start in run..n {
        let pivot = items[start].clone();
        let mut left = 0;
        let mut right = start;
        while left < right {
            let mid = left + ((right - left) >> 1);
            if cmp(&pivot, &items[mid]) < 0.0 {
                right = mid;
            } else {
                left = mid + 1;
            }
        }
        let mut p = start;
        while p > left {
            items[p] = items[p - 1].clone();
            p -= 1;
        }
        items[left] = pivot;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_matches_js() {
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(-2.6), -3.0);
        assert_eq!(js_round(0.49), 0.0);
    }

    #[test]
    fn arinc_round_trip() {
        let raw = Arinc429::to_raw(12345.5, Ssm::NormalOperation);
        let w = Arinc429::from_raw(raw);
        assert!(w.is_normal_operation());
        assert_eq!(w.value, 12345.5);
        let w = Arinc429::from_raw(Arinc429::to_raw(-3.25, Ssm::NoComputedData));
        assert!(w.is_no_computed_data());
        assert_eq!(w.value, -3.25);
    }

    #[test]
    fn sort_is_stable_and_ordered() {
        let mut v = vec![(3.0, 'a'), (1.0, 'b'), (3.0, 'c'), (2.0, 'd'), (1.0, 'e')];
        js_sort(&mut v, |a, b| a.0 - b.0);
        let order: String = v.iter().map(|x| x.1).collect();
        assert_eq!(order, "bedac");
    }
}

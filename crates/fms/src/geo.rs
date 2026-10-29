use serde::{Deserialize, Serialize};

pub const EARTH_RADIUS_NM: f64 = 3440.065;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Coordinates {
    pub lat: f64,
    #[serde(alias = "lon")]
    pub long: f64,
}

impl Coordinates {
    pub fn new(lat: f64, long: f64) -> Self {
        Self { lat, long }
    }

    pub fn distance_to(&self, other: &Coordinates) -> f64 {
        let (la1, la2) = (self.lat.to_radians(), other.lat.to_radians());
        let dlat = la2 - la1;
        let dlon = (other.long - self.long).to_radians();
        let h = (dlat / 2.).sin().powi(2) + la1.cos() * la2.cos() * (dlon / 2.).sin().powi(2);
        2. * EARTH_RADIUS_NM * h.sqrt().min(1.).asin()
    }

    pub fn place_bearing_distance(&self, bearing: f64, distance_nm: f64) -> Coordinates {
        let d = distance_nm / EARTH_RADIUS_NM;
        let b = bearing.to_radians();
        let (la1, lo1) = (self.lat.to_radians(), self.long.to_radians());
        let la2 = (la1.sin() * d.cos() + la1.cos() * d.sin() * b.cos()).asin();
        let lo2 = lo1 + (b.sin() * d.sin() * la1.cos()).atan2(d.cos() - la1.sin() * la2.sin());
        Coordinates::new(la2.to_degrees(), (lo2.to_degrees() + 540.).rem_euclid(360.) - 180.)
    }

    pub fn bearing_to(&self, other: &Coordinates) -> f64 {
        let (la1, la2) = (self.lat.to_radians(), other.lat.to_radians());
        let dlon = (other.long - self.long).to_radians();
        let y = dlon.sin() * la2.cos();
        let x = la1.cos() * la2.sin() - la1.sin() * la2.cos() * dlon.cos();
        y.atan2(x).to_degrees().rem_euclid(360.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearing_matches_the_recorded_first_leg_course() {
        let egll = Coordinates::new(51.4775, -0.461388889);
        let cpt = Coordinates::new(51.491572222, -1.219691667);
        assert!((egll.bearing_to(&cpt) - 272.0035761458742).abs() < 1e-9);
    }
}

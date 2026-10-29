use crate::sample::Point3;

pub const EARTH_RADIUS_M: f64 = 6_371_000.0;
pub const NM_M: f64 = 1852.0;
pub const FT_M: f64 = 0.3048;
pub const G: f64 = 9.806_65;
pub const MS_TO_KT: f64 = 1.943_844;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ownship {
    pub east_m: f64,
    pub north_m: f64,
    pub alt_msl_m: f64,
    pub ra_ft: f64,
    pub heading_true_deg: f64,
    pub tas_ms: f64,
    pub gs_ms: f64,
    pub vs_ms: f64,
    pub on_ground: bool,
}

impl Ownship {
    pub fn ground_msl_m(&self) -> f64 {
        self.alt_msl_m - self.ra_ft * FT_M
    }
}

pub fn bearing_unit(bearing_deg: f64) -> (f64, f64) {
    let b = bearing_deg.to_radians();
    (b.sin(), b.cos())
}

pub fn beam_point(own: &Ownship, azimuth_deg: f64, range_nm: f64, tilt_deg: f64) -> Point3 {
    let range_m = range_nm * NM_M;
    let (se, sn) = bearing_unit(own.heading_true_deg + azimuth_deg);
    let drop = range_m * range_m / (2. * EARTH_RADIUS_M);
    Point3 { east_m: own.east_m + se * range_m, north_m: own.north_m + sn * range_m, alt_m: own.alt_msl_m + range_m * tilt_deg.to_radians().tan() - drop }
}

pub fn path_point(own: &Ownship, azimuth_deg: f64, range_nm: f64, floor_agl_m: f64) -> Point3 {
    let range_m = range_nm * NM_M;
    let (se, sn) = bearing_unit(own.heading_true_deg + azimuth_deg);
    let fpa = if own.gs_ms > 20. { (own.vs_ms / own.gs_ms).atan() } else { 0. };
    let alt = (own.alt_msl_m + range_m * fpa.tan()).max(own.ground_msl_m() + floor_agl_m);
    Point3 { east_m: own.east_m + se * range_m, north_m: own.north_m + sn * range_m, alt_m: alt }
}

pub fn radial_wind_ms(bearing_deg: f64, wind_east_ms: f32, wind_north_ms: f32) -> f64 {
    let (se, sn) = bearing_unit(bearing_deg);
    se * wind_east_ms as f64 + sn * wind_north_ms as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beam_points_along_the_heading_and_curves_away() {
        let own = Ownship { alt_msl_m: 1000., heading_true_deg: 90., ..Default::default() };
        let p = beam_point(&own, 0., 10., 0.);
        assert!((p.east_m - 18_520.).abs() < 1e-6 && p.north_m.abs() < 1e-6);
        assert!(p.alt_m < 1000.);
        let up = beam_point(&own, 0., 10., 3.);
        assert!(up.alt_m > p.alt_m + 900.);
    }

    #[test]
    fn path_point_follows_the_flight_path_and_respects_the_floor() {
        let own = Ownship { alt_msl_m: 600., ra_ft: 1000., gs_ms: 70., vs_ms: -3.7, ..Default::default() };
        let near = path_point(&own, 0., 1., 15.);
        assert!(near.alt_m < 600.);
        let far = path_point(&own, 0., 5., 15.);
        assert!((far.alt_m - (own.ground_msl_m() + 15.)).abs() < 1e-9 || far.alt_m > own.ground_msl_m() + 15.);
    }

    #[test]
    fn radial_wind_is_positive_when_blowing_away_along_the_beam() {
        assert!((radial_wind_ms(90., 10., 0.) - 10.).abs() < 1e-9);
        assert!((radial_wind_ms(0., 10., 0.)).abs() < 1e-9);
        assert!((radial_wind_ms(180., 0., 5.) + 5.).abs() < 1e-9);
    }
}

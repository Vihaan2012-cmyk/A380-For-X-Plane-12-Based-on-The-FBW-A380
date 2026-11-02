//! Glide slope false paths above the protected sector (1.75 x the angle, ICAO
//! Annex 10 3.1.5.3): null-reference array, carrier nulls at 2x and 4x, so a
//! reversed-sense crossing near 2x and a normal-sense false path at 3x.

const PROTECTED: f64 = 1.75;
const NULL_HALF_WIDTH: f64 = 0.08;
const LAST_LOBE: f64 = 3.9;
const SATURATION_DEG: f64 = 1.0;
const EARTH_RADIUS_M: f64 = 6_371_000.;
/// ICAO Annex 10 3.1.5.3.3: 10 NM, 8 degrees either side of the course, down to 0.45 x the angle.
const COVERAGE_M: f64 = 18_520.;
const COVERAGE_AZIMUTH_DEG: f64 = 8.;
const LOWEST: f64 = 0.45;

static LAST: std::sync::Mutex<Option<(i32, Option<f64>)>> = std::sync::Mutex::new(None);
static OWN: std::sync::Mutex<Option<(i32, Option<f64>)>> = std::sync::Mutex::new(None);

/// What the ILS receiver took from the nav database this tick when X-Plane showed no glide
/// slope, for the other receiver on the same frequency.
pub fn publish_own(frequency: i32, shown: Option<Option<f64>>) {
    if let Ok(mut l) = OWN.lock() {
        *l = shown.map(|s| (frequency, s));
    }
}

pub fn own_for_frequency(frequency: i32) -> Option<Option<f64>> {
    OWN.lock().ok().and_then(|l| *l).filter(|(f, _)| *f == frequency).map(|(_, s)| s)
}

/// A glide slope received from its antenna's position alone: deviation in degrees (positive
/// above the path) inside its coverage, the false paths above it, `None` outside.
pub fn received(elevation_deg: f64, path_deg: f64, distance_m: f64, lateral_deg: f64) -> Option<f64> {
    if !(path_deg > 0.) || !(distance_m <= COVERAGE_M) || !(lateral_deg.abs() <= COVERAGE_AZIMUTH_DEG) || !(elevation_deg / path_deg >= LOWEST) {
        return None;
    }
    match indicated(elevation_deg, path_deg) {
        Err(()) => Some((elevation_deg - path_deg).clamp(-SATURATION_DEG, SATURATION_DEG)),
        Ok(shown) => shown,
    }
}

/// What the ILS receiver showed this tick above the protected sector, for the
/// other receivers on the same frequency; cleared inside it.
pub fn publish(frequency: i32, shown: Option<Option<f64>>) {
    if let Ok(mut l) = LAST.lock() {
        *l = shown.map(|s| (frequency, s));
    }
}

pub fn for_frequency(frequency: i32) -> Option<Option<f64>> {
    LAST.lock().ok().and_then(|l| *l).filter(|(f, _)| *f == frequency).map(|(_, s)| s)
}

/// Deviation in degrees (positive above the path), `Ok(None)` with the flag in
/// view, `Err` inside the protected sector where X-Plane's receiver is used.
pub fn indicated(elevation_deg: f64, path_deg: f64) -> Result<Option<f64>, ()> {
    if !(path_deg > 0.) || !elevation_deg.is_finite() {
        return Err(());
    }
    let x = elevation_deg / path_deg;
    if x <= PROTECTED {
        return Err(());
    }
    if (x - 2.).abs() < NULL_HALF_WIDTH || x > LAST_LOBE {
        return Ok(None);
    }
    let half = std::f64::consts::FRAC_PI_2 * x;
    let ddm = 2. * half.cos() * half.sin().signum();
    Ok(Some((-(path_deg / std::f64::consts::PI) * ddm).clamp(-SATURATION_DEG, SATURATION_DEG)))
}

/// Elevation angle (degrees) of a point `height_m` above the antenna, seen
/// from the antenna at (`lat`, `lon`).
pub fn elevation_deg(lat: f64, lon: f64, antenna_lat: f64, antenna_lon: f64, height_m: f64) -> f64 {
    height_m.atan2(distance_m(lat, lon, antenna_lat, antenna_lon).max(1.)).to_degrees()
}

/// Great-circle distance in metres from the antenna at (`antenna_lat`, `antenna_lon`).
pub fn distance_m(lat: f64, lon: f64, antenna_lat: f64, antenna_lon: f64) -> f64 {
    let (p1, p2) = (antenna_lat.to_radians(), lat.to_radians());
    let dp = p2 - p1;
    let dl = (lon - antenna_lon).to_radians();
    let a = (dp / 2.).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.).sin().powi(2);
    2. * EARTH_RADIUS_M * a.sqrt().asin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_protected_sector_is_left_to_x_plane() {
        assert!(indicated(3., 3.).is_err());
        assert!(indicated(5.2, 3.).is_err());
        assert!(indicated(1., 3.).is_err());
    }

    #[test]
    fn twice_the_angle_drops_the_flag_and_reverses_the_sense() {
        assert_eq!(indicated(6., 3.), Ok(None));
        let below = indicated(5.6, 3.).unwrap().unwrap();
        let above = indicated(6.4, 3.).unwrap().unwrap();
        assert!(below > 0. && above < 0., "fly down below 6, fly up above it: {below} {above}");
    }

    #[test]
    fn three_times_the_angle_is_a_false_path_with_normal_sense() {
        assert!(indicated(9., 3.).unwrap().unwrap().abs() < 1e-9);
        assert!(indicated(8.7, 3.).unwrap().unwrap() < 0.);
        assert!(indicated(9.3, 3.).unwrap().unwrap() > 0.);
    }

    #[test]
    fn far_above_the_last_lobe_the_flag_shows() {
        assert_eq!(indicated(12.5, 3.), Ok(None));
    }

    #[test]
    fn the_needle_never_passes_full_scale() {
        for i in 0..400 {
            if let Ok(Some(d)) = indicated(5. + i as f64 * 0.02, 3.) {
                assert!(d.abs() <= SATURATION_DEG + 1e-12);
            }
        }
    }

    #[test]
    fn a_glide_slope_is_received_inside_its_coverage_only() {
        // VOBL 09L, 4.9 NM out, slightly above the 3 degree path.
        assert!((received(3.16, 3., 9_100., 0.1).unwrap() - 0.16).abs() < 1e-9);
        assert!(received(2.7, 3., 18_000., -1.).unwrap() < 0., "below the path reads negative");
        assert_eq!(received(3., 3., 19_000., 0.), None, "beyond 10 NM");
        assert_eq!(received(3., 3., 9_000., 8.5), None, "more than 8 degrees off the course");
        assert_eq!(received(1.2, 3., 9_000., 0.), None, "below 0.45 x the angle");
        assert_eq!(received(6., 3., 9_000., 0.), None, "the null at twice the angle");
        assert!(received(5.6, 3., 9_000., 0.).unwrap() > 0., "the reversed lobe above the protected sector");
        assert_eq!(received(3., 3., 9_000., f64::INFINITY), None);
    }

    #[test]
    fn elevation_follows_height_over_distance() {
        let e = elevation_deg(0., 0.01, 0., 0., 58.);
        assert!((e - (58f64).atan2(1111.95).to_degrees()).abs() < 0.01, "{e}");
    }
}

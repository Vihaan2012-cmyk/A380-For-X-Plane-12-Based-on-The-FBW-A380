//! The radar physics between X-Plane's weather and a painted return:
//! rain rate to reflectivity (Marshall-Palmer, Z = 200 R^1.6), the ARINC 708
//! colour bands on reflectivity (green from 20 dBZ, amber 30, red 40,
//! magenta 50), X-band two-way path attenuation through rain (k = 0.0076
//! R^1.25 dB/km one way at 9.3 GHz) with limited compensation, a 3.5 deg
//! pencil beam that only partly fills, or overshoots, a storm below its
//! top, the weaker return from ice above the freezing level and the bright
//! band in the melting layer, turbulence detection only inside 40 nm and
//! only in precipitation, and ground returns for clutter and MAP mode.
//! X-Plane's `precip_rate_alt` is a 0..1 ratio; [`rain_rate_mm_h`] is this
//! module's own mapping of it to a rain rate.

use super::levels::Level;

pub const BEAM_HALF_WIDTH_DEG: f64 = 1.75;
pub const TURBULENCE_RANGE_NM: f64 = 40.0;
pub const COMPENSATION_LIMIT_DB: f64 = 6.0;
const NM_KM: f64 = 1.852;

pub fn rain_rate_mm_h(precip_ratio: f32) -> f64 {
    60.0 * (precip_ratio.max(0.0) as f64).powf(1.3)
}

pub fn dbz(rain_mm_h: f64) -> f64 {
    if rain_mm_h <= 0.0 {
        f64::NEG_INFINITY
    } else {
        10.0 * (200.0 * rain_mm_h.powf(1.6)).log10()
    }
}

pub fn level(dbz: f64) -> Level {
    if dbz >= 50.0 {
        Level::Magenta
    } else if dbz >= 40.0 {
        Level::Red
    } else if dbz >= 30.0 {
        Level::Amber
    } else if dbz >= 20.0 {
        Level::Green
    } else {
        Level::None
    }
}

pub fn two_way_attenuation_db(rain_mm_h: f64, length_nm: f64) -> f64 {
    if rain_mm_h <= 0.0 {
        0.0
    } else {
        2.0 * 0.0076 * rain_mm_h.powf(1.25) * length_nm * NM_KM
    }
}

pub fn phase_db(temp_c: Option<f32>) -> f64 {
    match temp_c {
        Some(t) if (0.0..=3.0).contains(&t) => 5.0,
        Some(t) if t < -15.0 => -9.0,
        Some(t) if t < 0.0 => -4.0,
        _ => 0.0,
    }
}

pub fn beam_edges_m(centre_alt_m: f64, range_nm: f64) -> (f64, f64) {
    let half = range_nm * NM_KM * 1000.0 * BEAM_HALF_WIDTH_DEG.to_radians().tan();
    (centre_alt_m - half, centre_alt_m + half)
}

pub fn fill_db(storm_top_m: Option<f64>, ground_m: f64, beam: (f64, f64)) -> f64 {
    let Some(top) = storm_top_m else { return 0.0 };
    let (bottom, upper) = (beam.0.max(ground_m), beam.1);
    if upper <= bottom {
        return f64::NEG_INFINITY;
    }
    let filled = ((top.min(upper) - bottom) / (upper - bottom)).clamp(0.0, 1.0);
    if filled <= 0.0 {
        f64::NEG_INFINITY
    } else {
        10.0 * filled.log10()
    }
}

pub fn gain_db(manual_percent: Option<f32>) -> f64 {
    manual_percent.map_or(0.0, |p| ((p as f64 - 50.0) / 50.0 * 12.0).clamp(-12.0, 12.0))
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Radial {
    pub attenuation_db: f64,
}

impl Radial {
    pub fn observe(&mut self, true_dbz: f64, rain_mm_h: f64, bin_nm: f64) -> f64 {
        let lost = (self.attenuation_db - COMPENSATION_LIMIT_DB).max(0.0);
        let seen = true_dbz - lost;
        self.attenuation_db += two_way_attenuation_db(rain_mm_h, bin_nm);
        seen
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub precip_ratio: f32,
    pub turbulence: f32,
    pub temp_c: Option<f32>,
    pub storm_top_m: Option<f64>,
    pub ground_m: f64,
    pub wet_ground: bool,
    pub beam_centre_m: f64,
    pub range_nm: f64,
}

pub struct Settings {
    pub gain_db: f64,
    pub turbulence: bool,
    pub ground_clutter_suppression: bool,
    pub map_mode: bool,
}

pub fn classify(cell: &Cell, radial: &mut Radial, bin_nm: f64, s: &Settings) -> Level {
    let beam = beam_edges_m(cell.beam_centre_m, cell.range_nm);
    let hits_ground = beam.0 <= cell.ground_m;
    let ground_level = if hits_ground && !cell.wet_ground {
        let grazing = ((cell.ground_m - beam.0) / (beam.1 - beam.0).max(1.0)).clamp(0.0, 1.0);
        if grazing > 0.6 {
            Level::Amber
        } else {
            Level::Green
        }
    } else {
        Level::None
    };
    if s.map_mode {
        return ground_level;
    }
    let rain = rain_rate_mm_h(cell.precip_ratio);
    let true_dbz = dbz(rain) + phase_db(cell.temp_c) + fill_db(cell.storm_top_m, cell.ground_m, beam) + s.gain_db;
    let seen = radial.observe(true_dbz, rain, bin_nm);
    let mut weather = level(seen);
    if s.turbulence && cell.range_nm <= TURBULENCE_RANGE_NM && weather >= Level::Green && cell.turbulence >= 0.5 {
        weather = Level::Magenta;
    }
    let clutter = if s.ground_clutter_suppression { Level::None } else { ground_level };
    weather.max(clutter)
}

pub fn storm_top_m(clouds: &[crate::deep::weather::WeatherCloudLayer]) -> Option<f64> {
    clouds.iter().filter(|c| c.coverage >= 0.3 && c.alt_top_m > c.alt_base_m).map(|c| c.alt_top_m as f64).reduce(f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings { gain_db: 0.0, turbulence: true, ground_clutter_suppression: true, map_mode: false }
    }

    fn cell(precip: f32) -> Cell {
        Cell { precip_ratio: precip, turbulence: 0.0, temp_c: Some(10.0), storm_top_m: None, ground_m: 0.0, wet_ground: true, beam_centre_m: 3000.0, range_nm: 20.0 }
    }

    #[test]
    fn reflectivity_follows_marshall_palmer_and_the_arinc_bands() {
        assert!((dbz(1.0) - 23.0).abs() < 0.1);
        assert!((dbz(10.0) - 39.0).abs() < 0.1);
        assert_eq!(level(19.9), Level::None);
        assert_eq!(level(25.0), Level::Green);
        assert_eq!(level(35.0), Level::Amber);
        assert_eq!(level(45.0), Level::Red);
        assert_eq!(level(55.0), Level::Magenta);
        assert!(rain_rate_mm_h(0.05) > 1.0 && rain_rate_mm_h(0.05) < 1.5);
    }

    #[test]
    fn heavy_rain_shadows_what_lies_behind_it() {
        let s = settings();
        let mut clear = Radial::default();
        let behind_alone = classify(&cell(0.4), &mut clear, 2.0, &s);
        let mut through = Radial::default();
        for _ in 0..6 {
            classify(&cell(0.95), &mut through, 2.0, &s);
        }
        let behind_storm = classify(&cell(0.4), &mut through, 2.0, &s);
        assert!(behind_storm < behind_alone, "{behind_storm:?} should be weaker than {behind_alone:?}");
    }

    #[test]
    fn a_beam_above_the_storm_top_sees_nothing_and_ice_is_weaker() {
        let s = settings();
        let mut r = Radial::default();
        let over = Cell { storm_top_m: Some(2000.0), beam_centre_m: 9000.0, range_nm: 30.0, ..cell(0.8) };
        assert_eq!(classify(&over, &mut r, 2.0, &s), Level::None);
        let mut r1 = Radial::default();
        let mut r2 = Radial::default();
        let rain = classify(&Cell { temp_c: Some(10.0), ..cell(0.3) }, &mut r1, 2.0, &s);
        let ice = classify(&Cell { temp_c: Some(-30.0), ..cell(0.3) }, &mut r2, 2.0, &s);
        assert!(ice < rain);
    }

    #[test]
    fn turbulence_only_inside_forty_miles_and_only_in_precipitation() {
        let s = settings();
        let near = Cell { turbulence: 0.8, range_nm: 30.0, ..cell(0.08) };
        let far = Cell { range_nm: 60.0, ..near };
        let dry = Cell { precip_ratio: 0.0, ..near };
        assert_eq!(classify(&near, &mut Radial::default(), 2.0, &s), Level::Magenta);
        assert_ne!(classify(&far, &mut Radial::default(), 2.0, &s), Level::Magenta);
        assert_eq!(classify(&dry, &mut Radial::default(), 2.0, &s), Level::None);
    }

    #[test]
    fn ground_returns_show_in_map_mode_and_as_clutter_without_gcs() {
        let low = Cell { beam_centre_m: 300.0, ground_m: 400.0, wet_ground: false, range_nm: 15.0, ..cell(0.0) };
        let map = Settings { map_mode: true, ..settings() };
        assert_ne!(classify(&low, &mut Radial::default(), 2.0, &map), Level::None);
        let water = Cell { wet_ground: true, ..low };
        assert_eq!(classify(&water, &mut Radial::default(), 2.0, &map), Level::None);
        let gcs = settings();
        assert_eq!(classify(&low, &mut Radial::default(), 2.0, &gcs), Level::None);
        let no_gcs = Settings { ground_clutter_suppression: false, ..settings() };
        assert_ne!(classify(&low, &mut Radial::default(), 2.0, &no_gcs), Level::None);
    }

    #[test]
    fn manual_gain_moves_the_picture_twelve_db_each_way() {
        assert_eq!(gain_db(None), 0.0);
        assert_eq!(gain_db(Some(50.0)), 0.0);
        assert_eq!(gain_db(Some(100.0)), 12.0);
        assert_eq!(gain_db(Some(0.0)), -12.0);
    }
}

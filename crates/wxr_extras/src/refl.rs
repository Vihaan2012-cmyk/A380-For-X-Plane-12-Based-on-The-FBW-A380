#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Level {
    #[default]
    None,
    Green,
    Amber,
    Red,
    Magenta,
}

impl Level {
    pub fn rgba(self) -> [u8; 4] {
        match self {
            Level::None => [0, 0, 0, 0],
            Level::Green => [0, 255, 0, 255],
            Level::Amber => [255, 255, 0, 255],
            Level::Red => [255, 0, 0, 255],
            Level::Magenta => [255, 0, 255, 255],
        }
    }
}

pub const COMPENSATION_LIMIT_DB: f64 = 6.0;
const NM_KM: f64 = 1.852;

pub fn rain_rate_mm_h(precip: f32) -> f64 {
    60.0 * (precip.max(0.0) as f64).powf(1.3)
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

pub fn phase_db(temp_c: Option<f32>) -> f64 {
    match temp_c {
        Some(t) if (0.0..=3.0).contains(&t) => 5.0,
        Some(t) if t < -15.0 => -9.0,
        Some(t) if t < 0.0 => -4.0,
        _ => 0.0,
    }
}

pub fn two_way_attenuation_db(rain_mm_h: f64, length_nm: f64) -> f64 {
    if rain_mm_h <= 0.0 {
        0.0
    } else {
        2.0 * 0.0076 * rain_mm_h.powf(1.25) * length_nm * NM_KM
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Path {
    pub attenuation_db: f64,
}

impl Path {
    pub fn observe(&mut self, true_dbz: f64, rain_mm_h: f64, bin_nm: f64) -> f64 {
        let lost = (self.attenuation_db - COMPENSATION_LIMIT_DB).max(0.0);
        self.attenuation_db += two_way_attenuation_db(rain_mm_h, bin_nm);
        true_dbz - lost
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_match_the_arinc_colours() {
        assert_eq!(level(19.9), Level::None);
        assert_eq!(level(25.0), Level::Green);
        assert_eq!(level(35.0), Level::Amber);
        assert_eq!(level(45.0), Level::Red);
        assert_eq!(level(55.0), Level::Magenta);
        assert!((dbz(1.0) - 23.0).abs() < 0.1);
    }

    #[test]
    fn heavy_rain_attenuates_what_follows_beyond_the_compensation() {
        let mut p = Path::default();
        for _ in 0..8 {
            p.observe(50., rain_rate_mm_h(0.95), 2.0);
        }
        assert!(p.observe(45., 10., 2.0) < 45.);
    }
}

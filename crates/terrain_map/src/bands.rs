//! ND terrain colouring: FlyByWire's normal and peaks modes (SimBridge `navigationdisplay.ts`, as
//! ported in src/mapdata/terrain/navigation_display.rs) on the probe grid, plus the solid caution
//! and warning areas of FCOM DSC-34-20-20-10.

use crate::alert::{AlertArea, AlertLevel};
use crate::grid::{Cell, GeoGrid};

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    Clear = 0,
    GreenLow = 1,
    GreenHigh = 2,
    GreenSolid = 3,
    AmberLow = 4,
    AmberHigh = 5,
    RedHigh = 6,
    Water = 7,
    Unknown = 8,
    CautionSolid = 9,
    WarningSolid = 10,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayMode {
    Normal,
    Peaks,
}

const BIN_FT: f64 = 100.;
const HIST_MIN_FT: f64 = -500.;
const HIST_MAX_FT: f64 = 29040.;
const BIN_COUNT: usize = ((HIST_MAX_FT - HIST_MIN_FT + 1.) / BIN_FT) as usize + 1;
const LOWER_PERCENTILE: f64 = 0.85;
const UPPER_PERCENTILE: f64 = 0.95;
const FLAT_EARTH_FT: f64 = 100.;
const LOW_GREEN_OFFSET: f64 = 2000.;
const HIGH_GREEN_OFFSET: f64 = 1000.;
const HIGH_YELLOW_OFFSET: f64 = 1000.;
const HIGH_RED_OFFSET: f64 = 2000.;
const GEAR_DOWN_OFFSET: f64 = 250.;
const GEAR_UP_OFFSET: f64 = 500.;
pub const CUT_OFF_MIN_FT: f64 = 200.;
pub const CUT_OFF_MAX_FT: f64 = 400.;

#[derive(Clone, Copy, Debug)]
pub struct ClassifyInputs {
    pub lat: f64,
    pub lon: f64,
    pub alt_ft: f64,
    pub vs_fpm: f64,
    pub gear_down: bool,
    pub range_nm: f64,
    /// ARC mode only looks ahead of this bearing; ROSE uses the whole circle.
    pub arc_up_deg: Option<f64>,
    pub cut_off_ft: f64,
}

#[derive(Clone, Debug)]
pub struct BandImage {
    pub n: usize,
    pub lat0: f64,
    pub lon0: f64,
    pub dlat: f64,
    pub dlon: f64,
    pub cell_nm: f64,
    pub bands: Vec<u8>,
    pub mode: DisplayMode,
    /// What FlyByWire writes to L:A32NX_EGPWC_ND_<side>_TERRAIN_MIN/MAX_ELEVATION(_MODE).
    pub min_ft: f64,
    pub min_mode: u8,
    pub max_ft: f64,
    pub max_mode: u8,
}

pub const PEAKS_MODE: u8 = 0;
pub const WARNING: u8 = 1;
pub const CAUTION: u8 = 2;

/// `calculateAbsoluteCutOffAltitude` for a destination `distance_nm` away at `dest_elev_ft`.
pub fn cut_off_altitude(distance_nm: Option<f64>, alt_ft: f64, dest_elev_ft: f64) -> f64 {
    let Some(d) = distance_nm else { return HIST_MIN_FT };
    if d > 4. {
        return CUT_OFF_MAX_FT;
    }
    let dist_ft = d * crate::NM_FT;
    let opposite = alt_ft - dest_elev_ft;
    let glide = if opposite > 0. && d > 0. { (opposite / dist_ft).atan() } else { 0. };
    if glide >= 0.0523599 {
        return CUT_OFF_MAX_FT;
    }
    if d <= 1. || glide == 0. {
        return CUT_OFF_MIN_FT;
    }
    let slope = (CUT_OFF_MIN_FT - CUT_OFF_MAX_FT) / (3. * crate::NM_FT);
    (slope * (dist_ft - crate::NM_FT) + CUT_OFF_MAX_FT).round().clamp(CUT_OFF_MIN_FT, CUT_OFF_MAX_FT)
}

struct Stats {
    reference: f64,
    min: f64,
    max: f64,
    lower: f64,
    upper: f64,
    flat_earth: f64,
    half: f64,
}

fn stats(elevations: &[f64], alt_ft: f64, vs_fpm: f64, cut_off: f64) -> Stats {
    let mut hist = [0u32; BIN_COUNT];
    for &e in elevations {
        let b = ((e - HIST_MIN_FT) / BIN_FT).floor().clamp(0., (BIN_COUNT - 1) as f64) as usize;
        hist[b] += 1;
    }
    let first = (((cut_off - HIST_MIN_FT) / BIN_FT).floor().max(0.) as usize).min(BIN_COUNT);
    let total: f64 = hist[first..].iter().map(|&c| c as f64).sum();
    let (mut min_b, mut max_b, mut lo_b, mut up_b) = (-1i64, -1i64, -1i64, -1i64);
    let mut acc = 0.;
    for (b, &count) in hist.iter().enumerate().skip(first) {
        if total > 0. {
            acc += count as f64 / total;
            if lo_b < 0 && acc >= LOWER_PERCENTILE {
                lo_b = b as i64;
            }
            if up_b < 0 && acc >= UPPER_PERCENTILE {
                up_b = b as i64;
            }
        }
        if count > 0 {
            if min_b < 0 {
                min_b = b as i64;
            }
            max_b = b as i64;
        }
    }
    if up_b < 0 {
        up_b = BIN_COUNT as i64 - 1;
    }
    let min = if min_b >= 0 { min_b as f64 * BIN_FT + HIST_MIN_FT } else { -1. };
    let max = if max_b >= 0 { (max_b + 1) as f64 * BIN_FT + HIST_MIN_FT } else { 0. };
    Stats {
        reference: alt_ft + if vs_fpm <= -1000. { vs_fpm * 0.5 } else { 0. },
        min,
        max,
        lower: lo_b as f64 * BIN_FT + HIST_MIN_FT,
        upper: up_b as f64 * BIN_FT + HIST_MIN_FT,
        flat_earth: FLAT_EARTH_FT - (max - min),
        half: max * 0.5,
    }
}

/// Classifies every cell of `grid` within the display range around the aircraft.
pub fn classify(grid: &GeoGrid, inp: &ClassifyInputs, alert: &AlertArea) -> BandImage {
    let n = grid.n;
    let mut offsets = Vec::with_capacity(n * n);
    let mut sample = Vec::new();
    let up = inp.arc_up_deg.map(|d| d.to_radians().sin_cos());
    for j in 0..n {
        for i in 0..n {
            let (la, lo) = grid.cell_centre(i, j);
            let (nn, ee) = GeoGrid::ne_nm(inp.lat, inp.lon, la, lo);
            let inside = nn.hypot(ee) <= inp.range_nm + grid.cell_nm && up.is_none_or(|(s, c)| nn * c + ee * s >= -grid.cell_nm);
            offsets.push((nn, ee, inside));
            if inside {
                if let Cell::Land(e) = grid.cell(i, j) {
                    sample.push(e as f64);
                }
            }
        }
    }
    let s = stats(&sample, inp.alt_ft, inp.vs_fpm, inp.cut_off_ft);
    let gear = if inp.gear_down { GEAR_DOWN_OFFSET } else { GEAR_UP_OFFSET };
    let normal = s.max >= s.reference - gear;

    let mut low_yellow = s.reference - gear;
    if low_yellow <= s.min {
        low_yellow = s.min + 200.;
    }
    let high_yellow = s.reference + HIGH_YELLOW_OFFSET;
    let high_red = s.reference + HIGH_RED_OFFSET;
    let mut low_green = if s.reference - LOW_GREEN_OFFSET <= s.min { s.min + 200. } else { s.reference - LOW_GREEN_OFFSET };
    let high_green = if s.reference - HIGH_GREEN_OFFSET <= s.min { s.min + 200. } else { s.reference - HIGH_GREEN_OFFSET };
    if s.flat_earth >= 0. {
        if s.half <= s.lower && low_green > s.half {
            low_green = s.half;
        } else if s.half > s.lower && low_green > s.lower {
            low_green = s.lower;
        }
    }
    let peak_low = s.lower.min(s.half);
    let mut peak_high = s.upper.min((s.max - s.min) * 0.65 + s.min);
    let mut peak_solid = (s.max - s.min) * 0.95 + s.min;
    if peak_low >= peak_high || peak_low >= peak_solid || peak_high >= peak_solid || s.lower >= s.upper || s.lower >= peak_solid || s.upper >= peak_solid {
        peak_high = s.max + 100.;
        peak_solid = s.max + 100.;
    }

    let mut bands = vec![Band::Clear as u8; n * n];
    for (k, &(nn, ee, inside)) in offsets.iter().enumerate() {
        if !inside {
            continue;
        }
        let cell = grid.cells()[k];
        let band = match cell {
            Cell::Unknown | Cell::Miss { .. } => Band::Unknown,
            Cell::Water(_) => Band::Water,
            Cell::Land(e) => {
                let e = e as f64;
                match alert.level_at(nn, ee, e) {
                    AlertLevel::Warning => Band::WarningSolid,
                    AlertLevel::Caution => Band::CautionSolid,
                    AlertLevel::None if normal => {
                        if e < inp.cut_off_ft {
                            Band::Clear
                        } else if e >= high_red {
                            Band::RedHigh
                        } else if e >= high_yellow {
                            Band::AmberHigh
                        } else if e >= high_green && e < low_yellow {
                            Band::GreenHigh
                        } else if e >= low_yellow {
                            Band::AmberLow
                        } else if e >= low_green {
                            Band::GreenLow
                        } else {
                            Band::Clear
                        }
                    }
                    AlertLevel::None => {
                        if e >= peak_solid {
                            Band::GreenSolid
                        } else if e >= peak_high {
                            Band::GreenHigh
                        } else if e >= peak_low {
                            Band::GreenLow
                        } else {
                            Band::Clear
                        }
                    }
                }
            }
        };
        bands[k] = band as u8;
    }

    let (min_ft, min_mode, max_ft, max_mode) = if normal {
        (if inp.cut_off_ft > low_green { inp.cut_off_ft } else { low_green }, if low_yellow <= high_green { WARNING } else { PEAKS_MODE }, s.max, if s.max >= high_red { CAUTION } else { WARNING })
    } else if s.max < 0. {
        (-1., PEAKS_MODE, 0., PEAKS_MODE)
    } else {
        (if peak_low > s.min { peak_low } else { s.min }, PEAKS_MODE, s.max, PEAKS_MODE)
    };
    let (lat0, lon0) = grid.cell_centre(0, 0);
    BandImage {
        n,
        lat0,
        lon0,
        dlat: grid.dlat(),
        dlon: grid.dlon(),
        cell_nm: grid.cell_nm,
        bands,
        mode: if normal { DisplayMode::Normal } else { DisplayMode::Peaks },
        min_ft,
        min_mode,
        max_ft,
        max_mode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Probe;

    fn grid_with(f: impl Fn(f64, f64) -> Probe) -> GeoGrid {
        let mut g = GeoGrid::new(64, 0.5);
        g.recentre(0., 0.);
        for j in 0..g.n {
            for i in 0..g.n {
                let (la, lo) = g.cell_centre(i, j);
                let (nn, ee) = GeoGrid::ne_nm(0., 0., la, lo);
                g.store(i, j, f(nn, ee));
            }
        }
        g
    }

    fn inputs(alt: f64) -> ClassifyInputs {
        ClassifyInputs { lat: 0., lon: 0., alt_ft: alt, vs_fpm: 0., gear_down: false, range_nm: 15., arc_up_deg: None, cut_off_ft: -500. }
    }

    fn band_at(img: &BandImage, g: &GeoGrid, nn: f64, ee: f64) -> Band {
        let (la, lo) = GeoGrid::offset(0., 0., nn, ee);
        let (i, j) = g.index_of(la, lo).unwrap();
        let b = img.bands[j * img.n + i];
        [Band::Clear, Band::GreenLow, Band::GreenHigh, Band::GreenSolid, Band::AmberLow, Band::AmberHigh, Band::RedHigh, Band::Water, Band::Unknown, Band::CautionSolid, Band::WarningSolid][b as usize]
    }

    fn ramp(nn: f64, _ee: f64) -> Probe {
        Probe::Hit { elevation_ft: 5000. + nn * 400., water: false }
    }

    #[test]
    fn normal_mode_bands_follow_height_relative_to_the_aircraft() {
        let g = grid_with(ramp);
        let img = classify(&g, &inputs(5000.), &AlertArea::default());
        assert_eq!(img.mode, DisplayMode::Normal);
        assert_eq!(band_at(&img, &g, 7., 0.), Band::RedHigh);
        assert_eq!(band_at(&img, &g, 3.5, 0.), Band::AmberHigh);
        assert_eq!(band_at(&img, &g, 0., 0.), Band::AmberLow);
        assert_eq!(band_at(&img, &g, -2., 0.), Band::GreenHigh);
        assert_eq!(band_at(&img, &g, -4., 0.), Band::GreenLow);
        assert_eq!(band_at(&img, &g, -6., 0.), Band::Clear);
        assert_eq!(img.max_mode, CAUTION);
    }

    #[test]
    fn gear_down_starts_the_amber_band_250_ft_below_instead_of_500() {
        let g = grid_with(ramp);
        let mut i = inputs(5000.);
        assert_eq!(band_at(&classify(&g, &i, &AlertArea::default()), &g, -1.0, 0.), Band::AmberLow);
        i.gear_down = true;
        assert_eq!(band_at(&classify(&g, &i, &AlertArea::default()), &g, -1.0, 0.), Band::GreenHigh);
    }

    #[test]
    fn peaks_mode_when_the_aircraft_is_well_above_everything() {
        let g = grid_with(ramp);
        let img = classify(&g, &inputs(30000.), &AlertArea::default());
        assert_eq!(img.mode, DisplayMode::Peaks);
        assert_eq!(band_at(&img, &g, 14.5, 0.), Band::GreenSolid);
        assert_eq!(band_at(&img, &g, -14.5, 0.), Band::Clear);
        assert!(img.bands.contains(&(Band::GreenLow as u8)));
        assert!(!img.bands.iter().any(|&b| b == Band::AmberLow as u8 || b == Band::RedHigh as u8));
    }

    #[test]
    fn water_and_unprobed_cells_keep_their_own_bands() {
        let mut g = grid_with(|nn, _| if nn < 0. { Probe::Hit { elevation_ft: 0., water: true } } else { Probe::Hit { elevation_ft: 4000., water: false } });
        let (la, lo) = GeoGrid::offset(0., 0., 5., 5.);
        let (i, j) = g.index_of(la, lo).unwrap();
        g.forget_misses();
        g.store(i, j, Probe::Miss);
        let img = classify(&g, &inputs(5000.), &AlertArea::default());
        assert_eq!(band_at(&img, &g, -5., 0.), Band::Water);
        assert_eq!(band_at(&img, &g, 5., 5.), Band::Unknown);
        assert_eq!(band_at(&img, &g, 14.9, 14.9), Band::Clear);
    }

    #[test]
    fn arc_mode_leaves_the_area_behind_clear() {
        let g = grid_with(ramp);
        let mut i = inputs(5000.);
        i.arc_up_deg = Some(0.);
        let img = classify(&g, &i, &AlertArea::default());
        assert_eq!(band_at(&img, &g, -4., 0.), Band::Clear);
        assert_eq!(band_at(&img, &g, 3.5, 0.), Band::AmberHigh);
    }

    #[test]
    fn an_active_alert_paints_the_threat_solid() {
        let g = grid_with(ramp);
        let alert = AlertArea {
            level: AlertLevel::Warning,
            alt_ft: 5000.,
            gs_kt: 300.,
            caution_time_s: 60.,
            warning_time_s: 30.,
            half_angle_deg: 4.,
            start_width_nm: 0.2,
            clearance_ft: 500.,
            max_lookahead_nm: 12.,
            ..Default::default()
        };
        let img = classify(&g, &inputs(5000.), &alert);
        assert_eq!(band_at(&img, &g, 2., 0.), Band::WarningSolid);
        assert_eq!(band_at(&img, &g, 4., 0.), Band::CautionSolid);
        assert_eq!(band_at(&img, &g, 7., 0.), Band::RedHigh);
        assert_eq!(band_at(&img, &g, 2., 4.), Band::AmberLow);
    }

    #[test]
    fn cut_off_follows_flybywire_near_the_destination() {
        assert_eq!(cut_off_altitude(None, 3000., 0.), -500.);
        assert_eq!(cut_off_altitude(Some(10.), 3000., 0.), 400.);
        assert_eq!(cut_off_altitude(Some(0.5), 100., 0.), 200.);
        let mid = cut_off_altitude(Some(2.5), 400., 0.);
        assert!(mid > 200. && mid < 400.);
    }
}

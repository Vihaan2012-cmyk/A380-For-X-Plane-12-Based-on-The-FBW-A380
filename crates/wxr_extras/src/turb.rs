use crate::geom::{beam_point, radial_wind_ms, Ownship};
use crate::refl;
use crate::sample::WeatherSampler;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WxrMode {
    #[default]
    Wx,
    WxTurb,
    Turb,
    Map,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurbConfig {
    pub range_max_nm: f64,
    pub sigma_magenta_ms: f64,
    pub min_dbz: f64,
    pub eddy_per_ratio_ms: f64,
    pub shear_weight: f64,
    pub auto_below_ra_ft: f64,
    pub max_age_s: f64,
}

impl Default for TurbConfig {
    fn default() -> Self {
        Self { range_max_nm: 40., sigma_magenta_ms: 5., min_dbz: 20., eddy_per_ratio_ms: 10., shear_weight: 0.5, auto_below_ra_ft: 2300., max_age_s: 20. }
    }
}

pub fn enabled(cfg: &TurbConfig, mode: WxrMode, turb_auto: bool, ra_ft: f64) -> bool {
    match mode {
        WxrMode::WxTurb | WxrMode::Turb => true,
        WxrMode::Wx => turb_auto && ra_ft <= cfg.auto_below_ra_ft,
        WxrMode::Map => false,
    }
}

pub fn sigma_v(cfg: &TurbConfig, turbulence: f32, radial_ms: f64, neighbours_radial_ms: &[f64]) -> f64 {
    let eddy = turbulence.clamp(0., 1.) as f64 * cfg.eddy_per_ratio_ms;
    let shear = if neighbours_radial_ms.is_empty() {
        0.
    } else {
        let n = neighbours_radial_ms.len() as f64 + 1.;
        let mean = (radial_ms + neighbours_radial_ms.iter().sum::<f64>()) / n;
        let var = ((radial_ms - mean).powi(2) + neighbours_radial_ms.iter().map(|v| (v - mean).powi(2)).sum::<f64>()) / n;
        var.sqrt() * cfg.shear_weight
    };
    eddy.hypot(shear)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurbScanConfig {
    pub half_sector_deg: f64,
    pub az_bins: usize,
    pub range_bins: usize,
    pub max_range_nm: f64,
    pub tilt_deg: f64,
    pub fixed_alt_m: Option<f64>,
}

impl TurbScanConfig {
    pub fn azimuth_deg(&self, a: usize) -> f64 {
        if self.az_bins < 2 {
            return 0.;
        }
        -self.half_sector_deg + a as f64 * 2. * self.half_sector_deg / (self.az_bins - 1) as f64
    }

    pub fn range_nm(&self, r: usize) -> f64 {
        (r + 1) as f64 / self.range_bins as f64 * self.max_range_nm
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurbReturn {
    pub azimuth_deg: f64,
    pub range_nm: f64,
    pub sigma_ms: f64,
    pub east_m: f64,
    pub north_m: f64,
}

#[derive(Clone, Copy, Default)]
struct Cell {
    valid: bool,
    t: f64,
    radial_ms: f64,
    turbulence: f32,
    dbz: f64,
    east_m: f64,
    north_m: f64,
}

pub struct TurbulenceScanner {
    cfg: TurbConfig,
    scan: Option<TurbScanConfig>,
    cells: Vec<Cell>,
    cursor: usize,
}

impl TurbulenceScanner {
    pub fn new(cfg: TurbConfig) -> Self {
        Self { cfg, scan: None, cells: Vec::new(), cursor: 0 }
    }

    pub fn config(&self) -> &TurbConfig {
        &self.cfg
    }

    fn ensure(&mut self, scan: &TurbScanConfig) {
        if self.scan.as_ref() != Some(scan) {
            self.scan = Some(*scan);
            self.cells = vec![Cell::default(); scan.az_bins * scan.range_bins];
            self.cursor = 0;
        }
    }

    pub fn step(&mut self, own: &Ownship, scan: &TurbScanConfig, sampler: &mut dyn WeatherSampler, budget: usize, time_s: f64) {
        self.ensure(scan);
        let total = scan.az_bins * scan.range_bins;
        if total == 0 {
            return;
        }
        let mut sampled = 0;
        let mut visited = 0;
        while sampled < budget && visited < total {
            let idx = self.cursor;
            self.cursor = (idx + 1) % total;
            visited += 1;
            let (a, r) = (idx / scan.range_bins, idx % scan.range_bins);
            let range_nm = scan.range_nm(r);
            if range_nm > self.cfg.range_max_nm {
                self.cells[idx].valid = false;
                continue;
            }
            sampled += 1;
            let az = scan.azimuth_deg(a);
            let mut p = beam_point(own, az, range_nm, scan.tilt_deg);
            if let Some(alt) = scan.fixed_alt_m {
                p.alt_m = alt;
            }
            self.cells[idx] = match sampler.sample(p) {
                Some(w) => {
                    let rain = refl::rain_rate_mm_h(w.precip);
                    let top_cut = w.cloud_top_m.is_some_and(|t| p.alt_m > t as f64);
                    let dbz = if top_cut { f64::NEG_INFINITY } else { refl::dbz(rain) + refl::phase_db(w.temp_c) };
                    Cell { valid: true, t: time_s, radial_ms: radial_wind_ms(own.heading_true_deg + az, w.wind_east_ms, w.wind_north_ms), turbulence: w.turbulence, dbz, east_m: p.east_m, north_m: p.north_m }
                }
                None => Cell::default(),
            };
        }
    }

    pub fn returns(&self, ra_ft: f64, mode: WxrMode, turb_auto: bool, time_s: f64) -> Vec<TurbReturn> {
        let Some(scan) = self.scan else { return Vec::new() };
        if !enabled(&self.cfg, mode, turb_auto, ra_ft) {
            return Vec::new();
        }
        let fresh = |a: i64, r: i64| -> Option<&Cell> {
            if a < 0 || r < 0 || a >= scan.az_bins as i64 || r >= scan.range_bins as i64 {
                return None;
            }
            let c = &self.cells[a as usize * scan.range_bins + r as usize];
            (c.valid && time_s - c.t <= self.cfg.max_age_s).then_some(c)
        };
        let mut out = Vec::new();
        for a in 0..scan.az_bins as i64 {
            for r in 0..scan.range_bins as i64 {
                let Some(c) = fresh(a, r) else { continue };
                let range_nm = scan.range_nm(r as usize);
                if range_nm > self.cfg.range_max_nm || c.dbz < self.cfg.min_dbz {
                    continue;
                }
                let neighbours: Vec<f64> = [(a - 1, r), (a + 1, r), (a, r - 1), (a, r + 1)].iter().filter_map(|(na, nr)| fresh(*na, *nr)).map(|n| n.radial_ms).collect();
                let sigma = sigma_v(&self.cfg, c.turbulence, c.radial_ms, &neighbours);
                if sigma >= self.cfg.sigma_magenta_ms {
                    out.push(TurbReturn { azimuth_deg: scan.azimuth_deg(a as usize), range_nm, sigma_ms: sigma, east_m: c.east_m, north_m: c.north_m });
                }
            }
        }
        out
    }
}

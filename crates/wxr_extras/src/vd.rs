use crate::geom::{bearing_unit, Ownship, FT_M, NM_M};
use crate::refl::{self, Level};
use crate::sample::{Point3, WeatherSampler};
use crate::turb::{self, TurbConfig};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VdConfig {
    pub cols: usize,
    pub rows: usize,
    pub range_nm: f64,
    pub alt_min_ft: f64,
    pub alt_max_ft: f64,
    pub gain_db: f64,
    pub turbulence: bool,
    pub turb: TurbConfig,
}

impl Default for VdConfig {
    fn default() -> Self {
        Self { cols: 96, rows: 48, range_nm: 40., alt_min_ft: -1000., alt_max_ft: 41_000., gain_db: 0., turbulence: true, turb: TurbConfig::default() }
    }
}

impl VdConfig {
    pub fn cell_range_nm(&self, col: usize) -> f64 {
        (col as f64 + 0.5) * self.range_nm / self.cols as f64
    }

    pub fn cell_alt_ft(&self, row: usize) -> f64 {
        self.alt_max_ft - (row as f64 + 0.5) * (self.alt_max_ft - self.alt_min_ft) / self.rows as f64
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VdImage {
    pub cols: usize,
    pub rows: usize,
    pub generation: u64,
    pub bearing_true_deg: f64,
    pub levels: Vec<Level>,
}

impl VdImage {
    pub fn level(&self, col: usize, row: usize) -> Level {
        self.levels[row * self.cols + col]
    }

    pub fn rgba(&self) -> Vec<u8> {
        self.levels.iter().flat_map(|l| l.rgba()).collect()
    }
}

#[derive(Clone, Copy)]
struct Pinned {
    east_m: f64,
    north_m: f64,
    bearing_true_deg: f64,
}

pub struct VdScanner {
    cfg: Option<VdConfig>,
    cursor: usize,
    building: Vec<Level>,
    pinned: Option<Pinned>,
    path: refl::Path,
    image: Option<VdImage>,
    generation: u64,
}

impl Default for VdScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl VdScanner {
    pub fn new() -> Self {
        Self { cfg: None, cursor: 0, building: Vec::new(), pinned: None, path: refl::Path::default(), image: None, generation: 0 }
    }

    pub fn image(&self) -> Option<&VdImage> {
        self.image.as_ref()
    }

    pub fn clear(&mut self) {
        self.cfg = None;
        self.cursor = 0;
        self.building.clear();
        self.pinned = None;
        self.image = None;
    }

    pub fn step(&mut self, own: &Ownship, bearing_true_deg: f64, cfg: &VdConfig, sampler: &mut dyn WeatherSampler, budget: usize) {
        let total = cfg.cols * cfg.rows;
        if total == 0 {
            return;
        }
        if self.cfg.as_ref() != Some(cfg) {
            self.cfg = Some(*cfg);
            self.cursor = 0;
            self.pinned = None;
            self.building = vec![Level::None; total];
        }
        for _ in 0..budget.min(total) {
            let idx = self.cursor;
            let (row, col) = (idx / cfg.cols, idx % cfg.cols);
            if col == 0 {
                self.path = refl::Path::default();
            }
            let pin = *self.pinned.get_or_insert(Pinned { east_m: own.east_m, north_m: own.north_m, bearing_true_deg });
            let range_nm = cfg.cell_range_nm(col);
            let (se, sn) = bearing_unit(pin.bearing_true_deg);
            let p = Point3 { east_m: pin.east_m + se * range_nm * NM_M, north_m: pin.north_m + sn * range_nm * NM_M, alt_m: cfg.cell_alt_ft(row) * FT_M };
            self.building[idx] = match sampler.sample(p) {
                Some(w) => {
                    let top_cut = w.cloud_top_m.is_some_and(|t| p.alt_m > t as f64);
                    let rain = if top_cut { 0. } else { refl::rain_rate_mm_h(w.precip) };
                    let true_dbz = refl::dbz(rain) + refl::phase_db(w.temp_c) + cfg.gain_db;
                    let seen = self.path.observe(true_dbz, rain, cfg.range_nm / cfg.cols as f64);
                    let level = refl::level(seen);
                    let rough = !top_cut && turb::sigma_v(&cfg.turb, w.turbulence, 0., &[]) >= cfg.turb.sigma_magenta_ms;
                    if cfg.turbulence && rough && range_nm <= cfg.turb.range_max_nm && level >= Level::Green {
                        Level::Magenta
                    } else {
                        level
                    }
                }
                None => Level::None,
            };
            self.cursor = (idx + 1) % total;
            if self.cursor == 0 {
                self.generation += 1;
                self.image = Some(VdImage { cols: cfg.cols, rows: cfg.rows, generation: self.generation, bearing_true_deg: pin.bearing_true_deg, levels: self.building.clone() });
                self.pinned = None;
            }
        }
    }
}

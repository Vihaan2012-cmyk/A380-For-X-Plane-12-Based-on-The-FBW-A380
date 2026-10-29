use crate::geom::{path_point, radial_wind_ms, Ownship, G, MS_TO_KT, NM_M};
use crate::sample::WeatherSampler;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Severity {
    #[default]
    None,
    Caution,
    Warning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aural {
    MonitorRadarDisplay,
    WindshearAhead,
    GoAroundWindshearAhead,
}

impl Aural {
    pub fn text(self) -> &'static str {
        match self {
            Aural::MonitorRadarDisplay => "MONITOR RADAR DISPLAY",
            Aural::WindshearAhead => "WINDSHEAR AHEAD",
            Aural::GoAroundWindshearAhead => "GO AROUND, WINDSHEAR AHEAD",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PwsPhase {
    Takeoff,
    Approach,
    #[default]
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PwsStatus {
    Off,
    Failed,
    #[default]
    Standby,
    Active,
}

#[derive(Clone, Copy, Debug)]
pub struct PwsConfig {
    pub scan_half_deg: f64,
    pub alert_half_deg: f64,
    pub az_step_deg: f64,
    pub range_step_nm: f64,
    pub range_bins: usize,
    pub warning_max_nm: f64,
    pub caution_max_nm: f64,
    pub f_threshold: f64,
    pub ceiling_ft: f64,
    pub path_floor_agl_m: f64,
    pub min_airspeed_ms: f64,
    pub max_age_s: f64,
    pub confirm_s: f64,
    pub hold_s: f64,
    pub repeat_s: f64,
    pub max_warning_repeats: u32,
    pub inhibit_roll_gs_kt: f64,
    pub inhibit_ra_ft: f64,
}

impl Default for PwsConfig {
    fn default() -> Self {
        Self {
            scan_half_deg: 40.,
            alert_half_deg: 25.,
            az_step_deg: 5.,
            range_step_nm: 0.25,
            range_bins: 20,
            warning_max_nm: 1.5,
            caution_max_nm: 3.,
            f_threshold: 0.105,
            ceiling_ft: 1500.,
            path_floor_agl_m: 15.,
            min_airspeed_ms: 60.,
            max_age_s: 12.,
            confirm_s: 0.5,
            hold_s: 4.,
            repeat_s: 5.,
            max_warning_repeats: 3,
            inhibit_roll_gs_kt: 100.,
            inhibit_ra_ft: 50.,
        }
    }
}

impl PwsConfig {
    pub fn az_bins(&self) -> usize {
        (2. * self.scan_half_deg / self.az_step_deg).round() as usize + 1
    }

    pub fn azimuth_deg(&self, a: usize) -> f64 {
        -self.scan_half_deg + a as f64 * self.az_step_deg
    }

    pub fn range_nm(&self, r: usize) -> f64 {
        (r + 1) as f64 * self.range_step_nm
    }

    pub fn total_cells(&self) -> usize {
        self.az_bins() * self.range_bins
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PwsInputs {
    pub own: Ownship,
    pub phase: PwsPhase,
    pub pred_ws_auto: bool,
    pub radar_powered: bool,
    pub radar_failed: bool,
    pub data_valid: bool,
    pub takeoff_power: bool,
    pub higher_priority_aural: bool,
    pub time_s: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShearIcon {
    pub azimuth_deg: f64,
    pub range_nm: f64,
    pub half_width_deg: f64,
    pub severity: Severity,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PwsOutput {
    pub status: PwsStatus,
    pub hazard: Severity,
    pub severity: Severity,
    pub alerts_inhibited: bool,
    pub aural: Option<Aural>,
    pub pfd_ws_ahead: Severity,
    pub icon: Option<ShearIcon>,
    pub hazard_cells: usize,
}

#[derive(Clone, Copy, Default)]
struct Cell {
    valid: bool,
    t: f64,
    tail_ms: f64,
    vertical_ms: f64,
}

pub struct PwsScanner {
    cfg: PwsConfig,
    cells: Vec<Cell>,
    cursor: usize,
    first_seen: Option<f64>,
    last_hazard: f64,
    held: Severity,
    held_icon: Option<ShearIcon>,
    announced: Severity,
    last_aural_t: f64,
    warning_repeats: u32,
}

impl PwsScanner {
    pub fn new(cfg: PwsConfig) -> Self {
        let n = cfg.total_cells();
        Self { cfg, cells: vec![Cell::default(); n], cursor: 0, first_seen: None, last_hazard: f64::NEG_INFINITY, held: Severity::None, held_icon: None, announced: Severity::None, last_aural_t: f64::NEG_INFINITY, warning_repeats: 0 }
    }

    pub fn config(&self) -> &PwsConfig {
        &self.cfg
    }

    fn reset(&mut self) {
        for c in &mut self.cells {
            c.valid = false;
        }
        self.cursor = 0;
        self.first_seen = None;
        self.last_hazard = f64::NEG_INFINITY;
        self.held = Severity::None;
        self.held_icon = None;
        self.announced = Severity::None;
        self.warning_repeats = 0;
    }

    fn envelope_open(&self, i: &PwsInputs) -> bool {
        if i.own.on_ground {
            i.phase == PwsPhase::Takeoff && i.takeoff_power
        } else {
            i.phase != PwsPhase::Other && i.own.ra_ft <= self.cfg.ceiling_ft
        }
    }

    pub fn inhibited(&self, i: &PwsInputs) -> bool {
        let gs_kt = i.own.gs_ms * MS_TO_KT;
        let roll = i.own.on_ground && gs_kt > self.cfg.inhibit_roll_gs_kt;
        let low = !i.own.on_ground && i.own.ra_ft < self.cfg.inhibit_ra_ft;
        roll || low || i.higher_priority_aural
    }

    pub fn step(&mut self, i: &PwsInputs, sampler: &mut dyn WeatherSampler, budget: usize) -> PwsOutput {
        let status = if !i.pred_ws_auto {
            PwsStatus::Off
        } else if i.radar_failed || !i.radar_powered || !i.data_valid {
            PwsStatus::Failed
        } else if self.envelope_open(i) {
            PwsStatus::Active
        } else {
            PwsStatus::Standby
        };
        if status != PwsStatus::Active {
            self.reset();
            return PwsOutput { status, ..Default::default() };
        }

        let n_r = self.cfg.range_bins;
        let total = self.cfg.total_cells();
        for _ in 0..budget.min(total) {
            let idx = self.cursor;
            self.cursor = (idx + 1) % total;
            let (a, r) = (idx / n_r, idx % n_r);
            let az = self.cfg.azimuth_deg(a);
            let p = path_point(&i.own, az, self.cfg.range_nm(r), self.cfg.path_floor_agl_m);
            self.cells[idx] = match sampler.sample(p) {
                Some(w) => Cell { valid: true, t: i.time_s, tail_ms: radial_wind_ms(i.own.heading_true_deg + az, w.wind_east_ms, w.wind_north_ms), vertical_ms: w.vertical_ms as f64 },
                None => Cell::default(),
            };
        }

        let (hazard_raw, icon_raw, count) = self.evaluate(i);
        let now = i.time_s;
        if icon_raw.is_some() {
            let since = *self.first_seen.get_or_insert(now);
            self.last_hazard = now;
            if now - since >= self.cfg.confirm_s {
                self.held = hazard_raw;
                self.held_icon = icon_raw;
            }
        } else if now - self.last_hazard > self.cfg.hold_s {
            self.first_seen = None;
            self.held = Severity::None;
            self.held_icon = None;
        }

        let inhibited = self.inhibited(i);
        let severity = if inhibited { Severity::None } else { self.held };
        let aural = self.aural(i, severity);
        PwsOutput { status, hazard: self.held, severity, alerts_inhibited: inhibited, aural, pfd_ws_ahead: severity, icon: self.held_icon, hazard_cells: count }
    }

    fn aural(&mut self, i: &PwsInputs, severity: Severity) -> Option<Aural> {
        if severity == Severity::None {
            self.announced = Severity::None;
            self.warning_repeats = 0;
            return None;
        }
        let now = i.time_s;
        let escalated = severity > self.announced;
        let repeat = severity == Severity::Warning && !escalated && self.warning_repeats < self.cfg.max_warning_repeats && now - self.last_aural_t >= self.cfg.repeat_s;
        if !escalated && !repeat {
            return None;
        }
        self.announced = severity;
        self.last_aural_t = now;
        if severity == Severity::Warning {
            self.warning_repeats += 1;
        }
        Some(match (severity, i.phase) {
            (Severity::Caution, _) => Aural::MonitorRadarDisplay,
            (_, PwsPhase::Takeoff) => Aural::WindshearAhead,
            _ => Aural::GoAroundWindshearAhead,
        })
    }

    fn evaluate(&self, i: &PwsInputs) -> (Severity, Option<ShearIcon>, usize) {
        let (n_az, n_r) = (self.cfg.az_bins(), self.cfg.range_bins);
        let v = i.own.tas_ms.max(self.cfg.min_airspeed_ms);
        let fresh = |idx: usize| self.cells[idx].valid && i.time_s - self.cells[idx].t <= self.cfg.max_age_s;
        let mut haz = vec![false; n_az * n_r];
        for a in 0..n_az {
            for r in 0..n_r {
                let idx = a * n_r + r;
                if !fresh(idx) {
                    continue;
                }
                let lo = if r > 0 && fresh(idx - 1) { r - 1 } else { r };
                let hi = if r + 1 < n_r && fresh(idx + 1) { r + 1 } else { r };
                if lo == hi {
                    continue;
                }
                let dx = (hi - lo) as f64 * self.cfg.range_step_nm * NM_M;
                let dw_dx = (self.cells[a * n_r + hi].tail_ms - self.cells[a * n_r + lo].tail_ms) / dx;
                let f = v / G * dw_dx - self.cells[idx].vertical_ms / v;
                haz[idx] = f >= self.cfg.f_threshold;
            }
        }
        let mut confirmed: Vec<(usize, usize)> = Vec::new();
        for a in 0..n_az {
            for r in 0..n_r {
                if !haz[a * n_r + r] {
                    continue;
                }
                let mut neighbour = false;
                for da in -1i32..=1 {
                    for dr in -1i32..=1 {
                        if da == 0 && dr == 0 {
                            continue;
                        }
                        let (na, nr) = (a as i32 + da, r as i32 + dr);
                        if na >= 0 && nr >= 0 && (na as usize) < n_az && (nr as usize) < n_r && haz[na as usize * n_r + nr as usize] {
                            neighbour = true;
                        }
                    }
                }
                if neighbour {
                    confirmed.push((a, r));
                }
            }
        }
        if confirmed.is_empty() {
            return (Severity::None, None, 0);
        }
        let region = |range_nm: f64| {
            if range_nm <= self.cfg.warning_max_nm {
                Severity::Warning
            } else if range_nm <= self.cfg.caution_max_nm {
                Severity::Caution
            } else {
                Severity::None
            }
        };
        let in_alert = |a: usize| self.cfg.azimuth_deg(a).abs() <= self.cfg.alert_half_deg + 1e-9;
        let nearest_alert = confirmed.iter().filter(|(a, _)| in_alert(*a)).map(|(_, r)| self.cfg.range_nm(*r)).fold(f64::INFINITY, f64::min);
        let hazard = if nearest_alert.is_finite() { region(nearest_alert) } else { Severity::None };
        let nearest = confirmed.iter().map(|(_, r)| self.cfg.range_nm(*r)).fold(f64::INFINITY, f64::min);
        let near: Vec<f64> = confirmed.iter().filter(|(_, r)| self.cfg.range_nm(*r) <= nearest + 1.).map(|(a, _)| self.cfg.azimuth_deg(*a)).collect();
        let (lo, hi) = near.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), a| (lo.min(*a), hi.max(*a)));
        let icon = ShearIcon {
            azimuth_deg: (lo + hi) / 2.,
            range_nm: nearest,
            half_width_deg: ((hi - lo) / 2. + self.cfg.az_step_deg / 2.).max(self.cfg.az_step_deg),
            severity: if region(nearest) == Severity::Warning { Severity::Warning } else { Severity::Caution },
        };
        (hazard, Some(icon), confirmed.len())
    }
}

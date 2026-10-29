use crate::grid::{GeoGrid, Probe};

/// GENERIC: cell sizes and grid size chosen so the finest level covers ARC 20 / ROSE 40 and the
/// coarsest the 640 nm ring.
pub const LEVEL_CELL_NM: [f64; 4] = [0.25, 1.0, 4.0, 8.0];
pub const GRID_N: usize = 176;
pub const MAX_MISS_TRIES: u8 = 3;
pub const MISS_RETRY_FRAMES: u32 = 120;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbeRequest {
    pub level: usize,
    pub i: usize,
    pub j: usize,
    pub lat: f64,
    pub lon: f64,
    pub generation: u64,
}

#[derive(Clone, Debug)]
pub struct Scheduler {
    order: Vec<(i32, i32)>,
    n: usize,
}

impl Scheduler {
    pub fn new(n: usize) -> Self {
        let h = (n / 2) as i32;
        let mut order: Vec<(i32, i32)> = (0..n as i32).flat_map(|j| (0..n as i32).map(move |i| (i - h, j - h))).collect();
        order.sort_by_key(|&(x, y)| (x * x + y * y, y, x));
        Scheduler { order, n }
    }

    /// Up to `budget` cells of `grid` to probe: unknown cells (and misses due a retry) nearest first,
    /// those ahead of the track (or inside a quarter of the grid) before those behind.
    pub fn pick(&self, grid: &GeoGrid, level: usize, track_deg: f64, retry_misses: bool, budget: usize, out: &mut Vec<ProbeRequest>) {
        if budget == 0 || grid.n != self.n {
            return;
        }
        let (s, c) = track_deg.to_radians().sin_cos();
        let near = (self.n as i32 / 4).pow(2);
        let tries = if retry_misses { MAX_MISS_TRIES } else { 1 };
        let h = (self.n / 2) as i32;
        let start = out.len();
        for pass in 0..2 {
            for &(x, y) in &self.order {
                if out.len() - start >= budget {
                    return;
                }
                let ahead = x as f64 * s + y as f64 * c >= 0. || x * x + y * y <= near;
                if ahead != (pass == 0) {
                    continue;
                }
                let (i, j) = ((x + h) as usize, (y + h) as usize);
                if grid.cell(i, j).needs_probe(tries) {
                    let (lat, lon) = grid.cell_centre(i, j);
                    out.push(ProbeRequest { level, i, j, lat, lon, generation: grid.generation });
                }
            }
        }
    }
}

/// Every level of the probe grid around the aircraft and the scheduler that fills them.
#[derive(Clone, Debug)]
pub struct TerrainMap {
    pub levels: Vec<GeoGrid>,
    scheduler: Scheduler,
    frame: u32,
}

impl Default for TerrainMap {
    fn default() -> Self {
        Self::new(GRID_N, &LEVEL_CELL_NM)
    }
}

impl TerrainMap {
    pub fn new(n: usize, cells_nm: &[f64]) -> Self {
        let levels: Vec<GeoGrid> = cells_nm.iter().map(|&c| GeoGrid::new(n, c)).collect();
        let n = levels.first().map_or(n, |g| g.n);
        TerrainMap { levels, scheduler: Scheduler::new(n), frame: 0 }
    }

    /// The finest level whose grid still covers `radius_nm` around the aircraft.
    pub fn level_for(&self, radius_nm: f64) -> usize {
        self.levels.iter().position(|g| g.half_width_nm() * 0.95 >= radius_nm).unwrap_or(self.levels.len().saturating_sub(1))
    }

    /// Recentres every level and returns this frame's probes: most of the budget on the level the
    /// displays need, the rest on the others, finest first.
    pub fn plan(&mut self, lat: f64, lon: f64, track_deg: f64, needed_radius_nm: f64, budget: usize) -> Vec<ProbeRequest> {
        self.frame = self.frame.wrapping_add(1);
        for g in self.levels.iter_mut() {
            g.recentre(lat, lon);
        }
        let retry = self.frame % MISS_RETRY_FRAMES == 0;
        let main = self.level_for(needed_radius_nm);
        let mut out = Vec::with_capacity(budget);
        self.scheduler.pick(&self.levels[main], main, track_deg, retry, budget * 3 / 4, &mut out);
        for (k, g) in self.levels.iter().enumerate() {
            if k != main {
                let left = budget.saturating_sub(out.len());
                self.scheduler.pick(g, k, track_deg, retry, left, &mut out);
            }
        }
        let left = budget.saturating_sub(out.len());
        if left > 0 {
            let taken = out.iter().filter(|r| r.level == main).count();
            let mut more = Vec::new();
            self.scheduler.pick(&self.levels[main], main, track_deg, retry, taken + left, &mut more);
            out.extend(more.into_iter().skip(taken));
        }
        out
    }

    /// Stores a probe made for `req`; dropped when its grid moved since it was planned.
    pub fn store(&mut self, req: &ProbeRequest, probe: Probe) -> bool {
        let Some(g) = self.levels.get_mut(req.level) else { return false };
        if g.generation != req.generation && g.index_of(req.lat, req.lon) != Some((req.i, req.j)) {
            return false;
        }
        g.store(req.i, req.j, probe);
        true
    }

    /// Elevation in feet from the finest level that has the point.
    pub fn elevation_ft(&self, lat: f64, lon: f64) -> Option<f64> {
        self.levels.iter().find_map(|g| g.elevation_at(lat, lon).and_then(|c| c.elevation_ft()))
    }

    pub fn filled(&self, level: usize) -> f64 {
        let g = &self.levels[level];
        g.cells().iter().filter(|c| c.elevation_ft().is_some()).count() as f64 / g.cells().len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(lat: f64, lon: f64) -> Probe {
        Probe::Hit { elevation_ft: 1000. + 100. * lat + 10. * lon, water: false }
    }

    #[test]
    fn the_budget_is_respected_and_nearest_cells_come_first() {
        let mut m = TerrainMap::new(32, &[1.0, 4.0]);
        let reqs = m.plan(45., 7., 0., 10., 50);
        assert!(reqs.len() <= 50 && !reqs.is_empty());
        let first = reqs[0];
        assert_eq!(first.level, 0);
        let (n, e) = GeoGrid::ne_nm(45., 7., first.lat, first.lon);
        assert!(n.hypot(e) <= 1.5);
    }

    #[test]
    fn cells_ahead_of_the_track_are_probed_before_those_behind() {
        let mut m = TerrainMap::new(64, &[1.0]);
        let reqs = m.plan(0., 0., 90., 10., 4096);
        let far: Vec<_> = reqs.iter().map(|r| GeoGrid::ne_nm(0., 0., r.lat, r.lon)).filter(|(n, e)| n.hypot(*e) > 20.).collect();
        let first_behind = far.iter().position(|(_, e)| *e < -1.).unwrap();
        assert!(far[..first_behind].iter().all(|(_, e)| *e >= -1.));
    }

    #[test]
    fn the_grid_fills_up_and_then_asks_for_nothing() {
        let mut m = TerrainMap::new(16, &[1.0]);
        for _ in 0..10 {
            for r in m.plan(30., 30., 0., 5., 64) {
                assert!(m.store(&r, world(r.lat, r.lon)));
            }
        }
        assert_eq!(m.filled(0), 1.0);
        assert!(m.plan(30., 30., 0., 5., 64).is_empty());
        let e = m.elevation_ft(30., 30.).unwrap();
        assert!((e - (1000. + 3000. + 300.)).abs() < 20.);
    }

    #[test]
    fn misses_are_retried_later_but_not_every_frame() {
        let mut m = TerrainMap::new(8, &[1.0]);
        for r in m.plan(0., 0., 0., 2., 64) {
            m.store(&r, Probe::Miss);
        }
        assert!(m.plan(0., 0., 0., 2., 64).is_empty());
        let mut retried = false;
        for _ in 0..MISS_RETRY_FRAMES {
            if !m.plan(0., 0., 0., 2., 64).is_empty() {
                retried = true;
            }
        }
        assert!(retried);
    }

    #[test]
    fn the_level_follows_the_range() {
        let m = TerrainMap::default();
        assert_eq!(m.level_for(10.), 0);
        assert_eq!(m.level_for(40.), 1);
        assert_eq!(m.level_for(160.), 2);
        assert_eq!(m.level_for(640.), 3);
    }
}

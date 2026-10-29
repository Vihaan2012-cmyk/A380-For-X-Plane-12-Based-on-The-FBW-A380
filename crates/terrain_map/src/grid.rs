#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Cell {
    #[default]
    Unknown,
    Miss { tries: u8 },
    Land(f32),
    Water(f32),
}

impl Cell {
    pub fn elevation_ft(self) -> Option<f64> {
        match self {
            Cell::Land(e) | Cell::Water(e) => Some(e as f64),
            _ => None,
        }
    }

    pub fn needs_probe(self, max_tries: u8) -> bool {
        match self {
            Cell::Unknown => true,
            Cell::Miss { tries } => tries < max_tries,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Probe {
    Hit { elevation_ft: f64, water: bool },
    Miss,
}

#[derive(Clone, Debug)]
pub struct GeoGrid {
    pub n: usize,
    pub cell_nm: f64,
    lat_ref: f64,
    dlat: f64,
    dlon: f64,
    i0: i64,
    j0: i64,
    cells: Vec<Cell>,
    pub generation: u64,
    anchored: bool,
}

const REANCHOR_DEG: f64 = 2.0;

impl GeoGrid {
    pub fn new(n: usize, cell_nm: f64) -> Self {
        let n = n.max(2) & !1;
        GeoGrid { n, cell_nm, lat_ref: 0., dlat: cell_nm / 60., dlon: cell_nm / 60., i0: 0, j0: 0, cells: vec![Cell::Unknown; n * n], generation: 0, anchored: false }
    }

    pub fn half_width_nm(&self) -> f64 {
        self.n as f64 * 0.5 * self.cell_nm
    }

    pub fn dlat(&self) -> f64 {
        self.dlat
    }

    pub fn dlon(&self) -> f64 {
        self.dlon
    }

    pub fn origin(&self) -> (f64, f64) {
        (self.j0 as f64 * self.dlat, self.i0 as f64 * self.dlon)
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn cell(&self, i: usize, j: usize) -> Cell {
        self.cells[j * self.n + i]
    }

    pub fn cell_centre(&self, i: usize, j: usize) -> (f64, f64) {
        ((self.j0 + j as i64) as f64 * self.dlat, wrap_lon((self.i0 + i as i64) as f64 * self.dlon))
    }

    pub fn index_of(&self, lat: f64, lon: f64) -> Option<(usize, usize)> {
        if !self.anchored {
            return None;
        }
        let j = (lat / self.dlat).round() as i64 - self.j0;
        let mut lon_cells = (lon / self.dlon).round() as i64 - self.i0;
        let wrap = (360. / self.dlon).round() as i64;
        if lon_cells < 0 {
            lon_cells += wrap;
        } else if lon_cells >= wrap {
            lon_cells -= wrap;
        }
        let n = self.n as i64;
        if (0..n).contains(&lon_cells) && (0..n).contains(&j) {
            Some((lon_cells as usize, j as usize))
        } else {
            None
        }
    }

    pub fn elevation_at(&self, lat: f64, lon: f64) -> Option<Cell> {
        self.index_of(lat, lon).map(|(i, j)| self.cell(i, j))
    }

    pub fn store(&mut self, i: usize, j: usize, probe: Probe) {
        let c = &mut self.cells[j * self.n + i];
        *c = match probe {
            Probe::Hit { elevation_ft, water: true } => Cell::Water(elevation_ft as f32),
            Probe::Hit { elevation_ft, water: false } => Cell::Land(elevation_ft as f32),
            Probe::Miss => match *c {
                Cell::Miss { tries } => Cell::Miss { tries: tries.saturating_add(1) },
                _ => Cell::Miss { tries: 1 },
            },
        };
        self.generation += 1;
    }

    pub fn forget_misses(&mut self) {
        for c in self.cells.iter_mut() {
            if matches!(c, Cell::Miss { .. }) {
                *c = Cell::Unknown;
            }
        }
        self.generation += 1;
    }

    /// Recentres on the aircraft, keeping every cell that stays inside. Returns true when the grid moved.
    pub fn recentre(&mut self, lat: f64, lon: f64) -> bool {
        let half = (self.n / 2) as i64;
        if !self.anchored || (lat - self.lat_ref).abs() > REANCHOR_DEG {
            self.lat_ref = lat;
            self.dlat = self.cell_nm / 60.;
            self.dlon = self.cell_nm / (60. * lat.to_radians().cos().max(0.05));
            self.i0 = (lon / self.dlon).round() as i64 - half;
            self.j0 = (lat / self.dlat).round() as i64 - half;
            self.cells.iter_mut().for_each(|c| *c = Cell::Unknown);
            self.anchored = true;
            self.generation += 1;
            return true;
        }
        let ci = (lon / self.dlon).round() as i64 - half;
        let cj = (lat / self.dlat).round() as i64 - half;
        let slack = (self.n / 8).max(1) as i64;
        let mut di = ci - self.i0;
        let wrap = (360. / self.dlon).round() as i64;
        if di > wrap / 2 {
            di -= wrap;
        } else if di < -wrap / 2 {
            di += wrap;
        }
        let dj = cj - self.j0;
        if di.abs() < slack && dj.abs() < slack {
            return false;
        }
        let n = self.n as i64;
        let mut moved = vec![Cell::Unknown; self.cells.len()];
        for j in 0..n {
            let sj = j + dj;
            if !(0..n).contains(&sj) {
                continue;
            }
            for i in 0..n {
                let si = i + di;
                if (0..n).contains(&si) {
                    moved[(j * n + i) as usize] = self.cells[(sj * n + si) as usize];
                }
            }
        }
        self.cells = moved;
        self.i0 += di;
        self.j0 = cj;
        self.generation += 1;
        true
    }

    /// North and east offset of a point from the reference, nm (local flat earth).
    pub fn ne_nm(ref_lat: f64, ref_lon: f64, lat: f64, lon: f64) -> (f64, f64) {
        let mut dlon = lon - ref_lon;
        if dlon > 180. {
            dlon -= 360.;
        } else if dlon < -180. {
            dlon += 360.;
        }
        ((lat - ref_lat) * 60., dlon * 60. * ref_lat.to_radians().cos())
    }

    pub fn offset(lat: f64, lon: f64, north_nm: f64, east_nm: f64) -> (f64, f64) {
        let c = lat.to_radians().cos().max(1e-6);
        (lat + north_nm / 60., wrap_lon(lon + east_nm / (60. * c)))
    }
}

pub fn wrap_lon(lon: f64) -> f64 {
    let mut l = lon;
    while l > 180. {
        l -= 360.;
    }
    while l < -180. {
        l += 360.;
    }
    l
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_probe_lands_in_the_cell_that_contains_its_point() {
        let mut g = GeoGrid::new(64, 1.0);
        g.recentre(47.26, 11.35);
        let (i, j) = g.index_of(47.30, 11.40).unwrap();
        let (la, lo) = g.cell_centre(i, j);
        assert!((la - 47.30).abs() <= g.dlat() * 0.5 + 1e-9);
        assert!((lo - 11.40).abs() <= g.dlon() * 0.5 + 1e-9);
        g.store(i, j, Probe::Hit { elevation_ft: 6000., water: false });
        assert_eq!(g.elevation_at(47.30, 11.40), Some(Cell::Land(6000.)));
    }

    #[test]
    fn recentring_keeps_the_cells_that_overlap() {
        let mut g = GeoGrid::new(64, 1.0);
        g.recentre(10., 20.);
        let (i, j) = g.index_of(10.1, 20.1).unwrap();
        g.store(i, j, Probe::Hit { elevation_ft: 1234., water: false });
        assert!(!g.recentre(10.02, 20.0));
        assert!(g.recentre(10.25, 20.25));
        assert_eq!(g.elevation_at(10.1, 20.1), Some(Cell::Land(1234.)));
        let (ni, nj) = g.index_of(10.25, 20.25).unwrap();
        assert!((ni as i64 - 32).abs() <= 1 && (nj as i64 - 32).abs() <= 1);
    }

    #[test]
    fn crossing_the_antimeridian_keeps_cells() {
        let mut g = GeoGrid::new(64, 1.0);
        g.recentre(0., 179.9);
        let (i, j) = g.index_of(0., -179.8).unwrap();
        g.store(i, j, Probe::Hit { elevation_ft: 0., water: true });
        g.recentre(0., -179.6);
        assert_eq!(g.elevation_at(0., -179.8), Some(Cell::Water(0.)));
    }

    #[test]
    fn misses_count_up_and_can_be_forgotten() {
        let mut g = GeoGrid::new(8, 1.0);
        g.recentre(0., 0.);
        g.store(1, 1, Probe::Miss);
        g.store(1, 1, Probe::Miss);
        assert_eq!(g.cell(1, 1), Cell::Miss { tries: 2 });
        assert!(!g.cell(1, 1).needs_probe(2));
        g.forget_misses();
        assert_eq!(g.cell(1, 1), Cell::Unknown);
    }
}

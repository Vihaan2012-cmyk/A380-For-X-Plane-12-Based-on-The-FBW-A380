use crate::grid::{Cell, GeoGrid};
use crate::scheduler::TerrainMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProfilePoint {
    Unknown,
    Water(f64),
    Land(f64),
}

/// FCOM DSC-31-20-40-10: the terrain cut is twice the TAWS RNP wide and never wider than 10 nm.
pub const MAX_CUT_WIDTH_NM: f64 = 10.;

pub fn cut_width_nm(taws_rnp_nm: f64) -> f64 {
    (2. * taws_rnp_nm).min(MAX_CUT_WIDTH_NM)
}

fn cell_of(map: &TerrainMap, lat: f64, lon: f64, level: usize) -> Option<Cell> {
    map.levels[level..].iter().find_map(|g| g.elevation_at(lat, lon).filter(|c| c.elevation_ft().is_some()))
}

/// Highest terrain across the cut at every `step_nm` from 0 to `range_nm` along `path`
/// (the aircraft first, then the vertical cut's points: a track line, an azimuth or the flight plan).
pub fn profile_along(map: &TerrainMap, path: &[(f64, f64)], range_nm: f64, step_nm: f64, width_nm: f64) -> Vec<ProfilePoint> {
    let count = (range_nm / step_nm).ceil() as usize + 1;
    let mut out = Vec::with_capacity(count);
    if path.len() < 2 || step_nm <= 0. {
        out.resize(count, ProfilePoint::Unknown);
        return out;
    }
    let level = map.level_for(range_nm);
    let cell = map.levels[level].cell_nm;
    let lanes = ((width_nm * 0.5 / cell).floor() as i32).max(0);
    let mut leg = 0;
    let mut leg_start = 0.;
    let leg_len = |k: usize| {
        let (n, e) = GeoGrid::ne_nm(path[k].0, path[k].1, path[k + 1].0, path[k + 1].1);
        n.hypot(e)
    };
    for s in 0..count {
        let d = s as f64 * step_nm;
        while leg + 2 < path.len() && d > leg_start + leg_len(leg) {
            leg_start += leg_len(leg);
            leg += 1;
        }
        let (a, b) = (path[leg], path[leg + 1]);
        let (n, e) = GeoGrid::ne_nm(a.0, a.1, b.0, b.1);
        let len = n.hypot(e).max(1e-9);
        let (un, ue) = (n / len, e / len);
        let t = d - leg_start;
        let mut best: Option<(f64, bool)> = None;
        let mut any_land = false;
        for lane in -lanes..=lanes {
            let off = lane as f64 * cell;
            let (la, lo) = GeoGrid::offset(a.0, a.1, un * t - ue * off, ue * t + un * off);
            match cell_of(map, la, lo, level) {
                Some(Cell::Land(h)) => {
                    any_land = true;
                    best = Some(best.map_or((h as f64, false), |(m, w)| if h as f64 > m { (h as f64, false) } else { (m, w) }));
                }
                Some(Cell::Water(h)) => {
                    if best.is_none() {
                        best = Some((h as f64, true));
                    }
                }
                _ => {}
            }
        }
        out.push(match best {
            None => ProfilePoint::Unknown,
            Some((h, true)) if !any_land => ProfilePoint::Water(h),
            Some((h, _)) => ProfilePoint::Land(h),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Probe;

    fn filled(f: impl Fn(f64, f64) -> Probe) -> TerrainMap {
        let mut m = TerrainMap::new(96, &[0.5, 2.0]);
        for _ in 0..200 {
            let reqs = m.plan(0., 0., 0., 20., 4096);
            if reqs.is_empty() {
                break;
            }
            for r in reqs {
                let (n, e) = GeoGrid::ne_nm(0., 0., r.lat, r.lon);
                m.store(&r, f(n, e));
            }
        }
        m
    }

    #[test]
    fn a_ridge_across_the_track_shows_at_its_distance() {
        let m = filled(|n, _| Probe::Hit { elevation_ft: if (n - 8.).abs() < 0.6 { 9000. } else { 1000. }, water: false });
        let path = [(0., 0.), GeoGrid::offset(0., 0., 20., 0.)];
        let p = profile_along(&m, &path, 20., 0.5, 2.);
        assert_eq!(p.len(), 41);
        assert_eq!(p[16], ProfilePoint::Land(9000.));
        assert_eq!(p[4], ProfilePoint::Land(1000.));
    }

    #[test]
    fn the_cut_takes_the_highest_terrain_across_its_width() {
        let m = filled(|_, e| Probe::Hit { elevation_ft: if e > 0.9 && e < 1.6 { 7000. } else { 500. }, water: false });
        let path = [(0., 0.), GeoGrid::offset(0., 0., 10., 0.)];
        assert!(profile_along(&m, &path, 10., 1., 4.).iter().all(|p| *p == ProfilePoint::Land(7000.)));
        assert!(profile_along(&m, &path, 10., 1., 0.).iter().all(|p| *p == ProfilePoint::Land(500.)));
        assert_eq!(cut_width_nm(2.), 4.);
        assert_eq!(cut_width_nm(8.), 10.);
    }

    #[test]
    fn water_only_where_no_land_is_in_the_cut() {
        let m = filled(|n, _| if n > 5. { Probe::Hit { elevation_ft: 0., water: true } } else { Probe::Hit { elevation_ft: 200., water: false } });
        let path = [(0., 0.), GeoGrid::offset(0., 0., 10., 0.)];
        let p = profile_along(&m, &path, 10., 1., 2.);
        assert_eq!(p[2], ProfilePoint::Land(200.));
        assert_eq!(p[9], ProfilePoint::Water(0.));
    }

    #[test]
    fn the_cut_follows_a_turning_flight_plan() {
        let m = filled(|n, e| Probe::Hit { elevation_ft: if e > 4. && n > 4. { 8000. } else { 300. }, water: false });
        let corner = GeoGrid::offset(0., 0., 5., 0.);
        let path = [(0., 0.), corner, GeoGrid::offset(corner.0, corner.1, 0., 10.)];
        let p = profile_along(&m, &path, 14., 1., 0.);
        assert_eq!(p[3], ProfilePoint::Land(300.));
        assert_eq!(p[12], ProfilePoint::Land(8000.));
    }
}

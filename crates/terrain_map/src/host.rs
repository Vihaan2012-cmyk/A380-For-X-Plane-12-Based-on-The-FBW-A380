//! What FlyByWire's EfisTawsBridge posts to SimBridge (aircraft status, EFIS data per side, the VD
//! path), turned into probe plans, ND band images and VD profiles for the native displays.

use crate::alert::{AlertArea, AlertLevel};
use crate::bands::{classify, cut_off_altitude, ClassifyInputs};
use crate::grid::{GeoGrid, Probe};
use crate::profile::{cut_width_nm, profile_along};
use crate::publish::{nd_json, vd_json};
use crate::scheduler::{ProbeRequest, TerrainMap};

/// FBW `a380EfisRangeSettings`.
pub const ND_RANGES_NM: [f64; 8] = [-1., 10., 20., 40., 80., 160., 320., 640.];
/// FBW `EfisNdMode`.
pub const MODE_ROSE_NAV: i32 = 2;
pub const MODE_ARC: i32 = 3;
pub const MODE_PLAN: i32 = 4;
/// `L:A380X_EFIS_<side>_ACTIVE_OVERLAY` value for TERR (FBW EfisTawsBridge).
pub const OVERLAY_TERR: i32 = 2;
/// FBW EfisTawsBridge: straight segment appended after the last FMS waypoint.
pub const FMS_TAIL_NM: f64 = 160.;
/// FBW EfisTawsBridge `pathWidth: 1`, as a TAWS RNP.
pub const DEFAULT_TAWS_RNP_NM: f64 = 0.5;
/// GENERIC.
pub const CLASSIFY_PERIOD_MS: f64 = 500.;
/// GENERIC.
pub const PROBE_BUDGET: usize = 256;
/// GENERIC: VD profile samples across the range.
pub const VD_SAMPLES: f64 = 270.;
/// GENERIC: covers the TAWS look-ahead while no display shows terrain.
pub const MIN_PROBE_RADIUS_NM: f64 = 15.;
pub const OFF: &str = "{\"off\":true}";
/// What FBW's terrain gauge writes to the TERR legend L:vars when nothing is drawn.
pub const LEGEND_OFF: [f64; 4] = [-1., 0., -1., 0.];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Efis {
    pub range_index: usize,
    pub mode: i32,
    pub overlay: i32,
    pub vd_lower_ft: f64,
    pub vd_upper_ft: f64,
    pub nd_avail: bool,
    pub vd_avail: bool,
}

impl Efis {
    pub fn range_nm(&self) -> f64 {
        ND_RANGES_NM.get(self.range_index).copied().unwrap_or(0.).max(0.)
    }

    pub fn arc(&self) -> bool {
        self.mode == MODE_ARC
    }

    pub fn terr_on_nd(&self, terr_failed: bool) -> bool {
        self.mode != MODE_PLAN && self.overlay == OVERLAY_TERR && !terr_failed && self.nd_avail
    }

    pub fn terr_on_vd(&self, terr_failed: bool) -> bool {
        (self.mode == MODE_ARC || self.mode == MODE_ROSE_NAV) && !terr_failed && self.vd_avail
    }

    pub fn nd_radius_nm(&self) -> f64 {
        if self.arc() {
            self.range_nm()
        } else {
            self.range_nm() / 2.
        }
    }

    /// SimBridge `VerticalDisplay.aircraftStatusUpdate`.
    pub fn vd_range_nm(&self) -> f64 {
        if self.arc() {
            self.range_nm().min(160.).max(10.)
        } else {
            (self.range_nm() / 2.).min(160.).max(5.)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Status {
    pub valid: bool,
    pub lat: f64,
    pub lon: f64,
    pub alt_ft: f64,
    pub heading_deg: f64,
    pub track_deg: f64,
    pub vs_fpm: f64,
    pub gear_down: bool,
    pub dest: Option<(f64, f64)>,
    pub terr_failed: bool,
    /// FBW `shouldShowTrackLine`: the VD follows the heading instead of the flight plan.
    pub track_line: bool,
    pub efis: [Efis; 2],
    pub taws_rnp_nm: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FmsPath {
    pub waypoints: Vec<(f64, f64)>,
    pub track_change_nm: Option<f64>,
}

impl FmsPath {
    /// FBW `terrVdPathData`: each vector's start, then 160 nm on along the last vector.
    pub fn from_vectors(vectors: &[((f64, f64), (f64, f64))], track_change_nm: Option<f64>) -> Self {
        let mut waypoints: Vec<(f64, f64)> = vectors.iter().map(|v| v.0).collect();
        if let (Some(&(start, end)), Some(&last)) = (vectors.last(), waypoints.last()) {
            let (n, e) = GeoGrid::ne_nm(start.0, start.1, end.0, end.1);
            let b = e.atan2(n);
            waypoints.push(GeoGrid::offset(last.0, last.1, FMS_TAIL_NM * b.cos(), FMS_TAIL_NM * b.sin()));
        }
        FmsPath { waypoints, track_change_nm: track_change_nm.filter(|d| *d >= 0.) }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SideOutput {
    pub nd: String,
    pub vd: String,
    pub legend: [f64; 4],
    pub nd_on: bool,
    pub vd_on: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SideKey {
    nd_on: bool,
    vd_on: bool,
    mode: i32,
    range_index: usize,
}

#[derive(Clone, Debug, Default)]
pub struct TerrainHost {
    pub map: TerrainMap,
    fms: FmsPath,
    next_ms: [f64; 2],
    keys: [Option<SideKey>; 2],
    generation: u64,
}

impl TerrainHost {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn nd_on(s: &Status, side: usize, alert: &AlertArea) -> bool {
        let e = &s.efis[side];
        s.valid && (e.terr_on_nd(s.terr_failed) || (alert.level != AlertLevel::None && e.mode != MODE_PLAN && !s.terr_failed && e.nd_avail))
    }

    pub fn vd_on(s: &Status, side: usize) -> bool {
        s.valid && s.efis[side].terr_on_vd(s.terr_failed)
    }

    pub fn probe_radius_nm(s: &Status) -> f64 {
        let mut r = MIN_PROBE_RADIUS_NM;
        for e in &s.efis {
            if e.terr_on_nd(s.terr_failed) {
                r = r.max(e.nd_radius_nm());
            }
            if e.terr_on_vd(s.terr_failed) {
                r = r.max(e.vd_range_nm());
            }
        }
        r
    }

    pub fn plan(&mut self, s: &Status, budget: usize) -> Vec<ProbeRequest> {
        if !s.valid || !s.lat.is_finite() || !s.lon.is_finite() {
            return Vec::new();
        }
        self.map.plan(s.lat, s.lon, s.track_deg, Self::probe_radius_nm(s), budget)
    }

    pub fn store(&mut self, req: &ProbeRequest, probe: Probe) -> bool {
        self.map.store(req, probe)
    }

    pub fn elevation_ft(&self, lat: f64, lon: f64) -> Option<f64> {
        self.map.elevation_ft(lat, lon)
    }

    pub fn set_fms_path(&mut self, path: FmsPath) {
        self.fms = path;
    }

    pub fn fms_path(&self) -> &FmsPath {
        &self.fms
    }

    /// The vertical cut from the aircraft and where its grey area starts.
    pub fn vd_path(&self, s: &Status) -> (Vec<(f64, f64)>, Option<f64>) {
        let mut path = vec![(s.lat, s.lon)];
        if s.track_line || self.fms.waypoints.is_empty() {
            let b = s.heading_deg.to_radians();
            path.push(GeoGrid::offset(s.lat, s.lon, FMS_TAIL_NM * b.cos(), FMS_TAIL_NM * b.sin()));
            return (path, None);
        }
        path.extend_from_slice(&self.fms.waypoints);
        (path, self.fms.track_change_nm)
    }

    fn cut_off_ft(&self, s: &Status) -> f64 {
        let Some((la, lo)) = s.dest else { return cut_off_altitude(None, s.alt_ft, 0.) };
        let (n, e) = GeoGrid::ne_nm(s.lat, s.lon, la, lo);
        cut_off_altitude(Some(n.hypot(e)), s.alt_ft, self.map.elevation_ft(la, lo).unwrap_or(0.))
    }

    /// Both payloads and the legend for one side, now.
    pub fn side(&mut self, side: usize, s: &Status, alert: &AlertArea) -> SideOutput {
        let e = s.efis[side];
        let nd_on = Self::nd_on(s, side, alert);
        let vd_on = Self::vd_on(s, side);
        let generation = self.generation;
        let (nd, legend) = if nd_on {
            let radius = e.nd_radius_nm();
            let inp = ClassifyInputs {
                lat: s.lat,
                lon: s.lon,
                alt_ft: s.alt_ft,
                vs_fpm: s.vs_fpm,
                gear_down: s.gear_down,
                range_nm: radius,
                arc_up_deg: e.arc().then_some(s.heading_deg),
                cut_off_ft: self.cut_off_ft(s),
            };
            let level = self.map.level_for(radius);
            let img = classify(&self.map.levels[level], &inp, alert);
            (nd_json(&img, generation), [img.min_ft, img.min_mode as f64, img.max_ft, img.max_mode as f64])
        } else {
            (OFF.to_string(), LEGEND_OFF)
        };
        let vd = if vd_on {
            let range = e.vd_range_nm();
            let step = range / VD_SAMPLES;
            let (path, grey) = self.vd_path(s);
            let rnp = if s.taws_rnp_nm > 0. { s.taws_rnp_nm } else { DEFAULT_TAWS_RNP_NM };
            let points = profile_along(&self.map, &path, range, step, cut_width_nm(rnp));
            vd_json(&points, step, grey, generation)
        } else {
            OFF.to_string()
        };
        SideOutput { nd, vd, legend, nd_on, vd_on }
    }

    /// Each side at most every `CLASSIFY_PERIOD_MS`, or at once when what it shows changed.
    pub fn update(&mut self, now_ms: f64, s: &Status, alert: &AlertArea) -> Vec<(usize, SideOutput)> {
        let mut out = Vec::new();
        for side in 0..2 {
            let e = s.efis[side];
            let key = SideKey { nd_on: Self::nd_on(s, side, alert), vd_on: Self::vd_on(s, side), mode: e.mode, range_index: e.range_index };
            let changed = self.keys[side] != Some(key);
            if !changed && now_ms < self.next_ms[side] {
                continue;
            }
            self.keys[side] = Some(key);
            self.next_ms[side] = now_ms + CLASSIFY_PERIOD_MS;
            self.generation += 1;
            out.push((side, self.side(side, s, alert)));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn efis(mode: i32, overlay: i32, range_index: usize) -> Efis {
        Efis { range_index, mode, overlay, vd_lower_ft: -500., vd_upper_ft: 20000., nd_avail: true, vd_avail: true }
    }

    fn status(e: Efis) -> Status {
        Status {
            valid: true,
            lat: 47.,
            lon: 11.,
            alt_ft: 9000.,
            heading_deg: 0.,
            track_deg: 0.,
            vs_fpm: 0.,
            gear_down: false,
            dest: None,
            terr_failed: false,
            track_line: false,
            efis: [e, e],
            taws_rnp_nm: 0.,
        }
    }

    fn filled(h: &mut TerrainHost, s: &Status) {
        for _ in 0..400 {
            let reqs = h.plan(s, 20000);
            if reqs.is_empty() {
                break;
            }
            for r in reqs {
                let (n, _) = GeoGrid::ne_nm(s.lat, s.lon, r.lat, r.lon);
                let elevation_ft = if n > 8. { 9500. } else { 1000. };
                h.store(&r, Probe::Hit { elevation_ft, water: false });
            }
        }
    }

    #[test]
    fn terr_follows_the_overlay_mode_and_failures() {
        assert!(efis(MODE_ARC, OVERLAY_TERR, 2).terr_on_nd(false));
        assert!(!efis(MODE_PLAN, OVERLAY_TERR, 2).terr_on_nd(false));
        assert!(!efis(MODE_ARC, 1, 2).terr_on_nd(false));
        assert!(!efis(MODE_ARC, OVERLAY_TERR, 2).terr_on_nd(true));
        assert!(efis(MODE_ROSE_NAV, 0, 2).terr_on_vd(false));
        assert!(!efis(0, 0, 2).terr_on_vd(false));
        assert!(!efis(MODE_PLAN, 0, 2).terr_on_vd(false));
    }

    #[test]
    fn ranges_follow_fbw() {
        assert_eq!(efis(MODE_ARC, 0, 3).range_nm(), 40.);
        assert_eq!(efis(MODE_ARC, 0, 0).range_nm(), 0.);
        assert_eq!(efis(MODE_ARC, 0, 1).vd_range_nm(), 10.);
        assert_eq!(efis(MODE_ARC, 0, 7).vd_range_nm(), 160.);
        assert_eq!(efis(MODE_ROSE_NAV, 0, 1).vd_range_nm(), 5.);
        assert_eq!(efis(MODE_ROSE_NAV, 0, 3).nd_radius_nm(), 20.);
    }

    #[test]
    fn fms_path_gets_a_160_nm_tail_along_the_last_vector() {
        let p = FmsPath::from_vectors(&[((47., 11.), (47.5, 11.)), ((47.5, 11.), (47.5, 11.5))], Some(12.));
        assert_eq!(p.waypoints.len(), 3);
        assert_eq!(p.waypoints[1], (47.5, 11.));
        let (n, e) = GeoGrid::ne_nm(47.5, 11., p.waypoints[2].0, p.waypoints[2].1);
        assert!(n.abs() < 1. && (e - 160.).abs() < 1., "{n} {e}");
        assert_eq!(p.track_change_nm, Some(12.));
        assert_eq!(FmsPath::from_vectors(&[], Some(-1.)), FmsPath::default());
    }

    #[test]
    fn vd_path_uses_the_track_line_or_the_flight_plan() {
        let mut h = TerrainHost::new();
        let mut s = status(efis(MODE_ARC, 0, 3));
        s.heading_deg = 90.;
        let (p, grey) = h.vd_path(&s);
        assert_eq!(p.len(), 2);
        assert!(grey.is_none());
        let (n, e) = GeoGrid::ne_nm(s.lat, s.lon, p[1].0, p[1].1);
        assert!(n.abs() < 0.5 && (e - 160.).abs() < 1.);
        h.set_fms_path(FmsPath::from_vectors(&[((47.1, 11.), (47.2, 11.))], Some(7.)));
        let (p, grey) = h.vd_path(&s);
        assert_eq!(p.len(), 3);
        assert_eq!(grey, Some(7.));
        s.track_line = true;
        assert_eq!(h.vd_path(&s).0.len(), 2);
    }

    #[test]
    fn publishes_bands_and_profile_with_terr_selected() {
        let mut h = TerrainHost::new();
        let s = status(efis(MODE_ARC, OVERLAY_TERR, 2));
        filled(&mut h, &s);
        let out = h.update(0., &s, &AlertArea::default());
        assert_eq!(out.len(), 2);
        let o = &out[0].1;
        assert!(o.nd_on && o.vd_on);
        assert!(o.nd.contains("\"bands\":\""), "{}", &o.nd[..80]);
        assert!(o.vd.contains("\"kind\":\"L"));
        assert!(o.vd.contains("9500"));
        assert!(o.legend[2] >= 9500., "{:?}", o.legend);
        assert!(h.update(100., &s, &AlertArea::default()).is_empty());
        assert_eq!(h.update(500., &s, &AlertArea::default()).len(), 2);
    }

    #[test]
    fn nd_is_off_without_terr_but_the_vd_stays() {
        let mut h = TerrainHost::new();
        let s = status(efis(MODE_ARC, 0, 2));
        filled(&mut h, &s);
        let out = h.update(0., &s, &AlertArea::default());
        let o = &out[1].1;
        assert_eq!(o.nd, OFF);
        assert_eq!(o.legend, LEGEND_OFF);
        assert_ne!(o.vd, OFF);
    }

    #[test]
    fn an_alert_shows_the_nd_terrain_and_a_mode_change_republishes_at_once() {
        let mut h = TerrainHost::new();
        let mut s = status(efis(MODE_ARC, 0, 2));
        filled(&mut h, &s);
        h.update(0., &s, &AlertArea::default());
        let alert = AlertArea { level: AlertLevel::Warning, lat: s.lat, lon: s.lon, alt_ft: 9000., gs_kt: 300., caution_time_s: 60., warning_time_s: 30., half_angle_deg: 4., start_width_nm: 0.2, clearance_ft: 500., max_lookahead_nm: 12., ..Default::default() };
        let out = h.update(10., &s, &alert);
        assert_eq!(out.len(), 2);
        assert!(out[0].1.nd_on);
        s.efis[0].mode = MODE_PLAN;
        let out = h.update(20., &s, &alert);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].1.nd, OFF);
        assert_eq!(out[0].1.vd, OFF);
    }

    #[test]
    fn invalid_adiru_data_blanks_both() {
        let mut h = TerrainHost::new();
        let mut s = status(efis(MODE_ARC, OVERLAY_TERR, 2));
        s.valid = false;
        assert!(h.plan(&s, 10).is_empty());
        let out = h.update(0., &s, &AlertArea::default());
        assert!(out.iter().all(|(_, o)| o.nd == OFF && o.vd == OFF));
    }

    #[test]
    fn probe_radius_covers_the_widest_display() {
        let mut s = status(efis(MODE_ARC, OVERLAY_TERR, 5));
        assert_eq!(TerrainHost::probe_radius_nm(&s), 160.);
        s.efis = [efis(MODE_PLAN, 0, 7), efis(0, 0, 1)];
        assert_eq!(TerrainHost::probe_radius_nm(&s), MIN_PROBE_RADIUS_NM);
    }
}

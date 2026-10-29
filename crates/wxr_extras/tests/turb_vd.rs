use wxr_extras::synthetic::{Microburst, StormCell, SyntheticField};
use wxr_extras::turb::TurbScanConfig;
use wxr_extras::{Level, Ownship, TurbConfig, TurbulenceScanner, VdConfig, VdScanner, WxrMode};

const NM: f64 = 1852.0;

fn own() -> Ownship {
    Ownship { alt_msl_m: 3000., ra_ft: 9000., heading_true_deg: 90., tas_ms: 150., gs_ms: 150., ..Default::default() }
}

fn storm(range_nm: f64, turbulence: f32, precip: f32) -> SyntheticField {
    SyntheticField {
        storms: vec![StormCell { east_m: range_nm * NM, north_m: 0., radius_m: 8. * NM, peak_precip: precip, peak_turbulence: turbulence, base_m: 0., top_m: 12_000. }],
        ..Default::default()
    }
}

fn scan_cfg(max_range_nm: f64) -> TurbScanConfig {
    TurbScanConfig { half_sector_deg: 60., az_bins: 61, range_bins: 24, max_range_nm, tilt_deg: 0., fixed_alt_m: Some(3000.) }
}

fn sweep(field: &mut SyntheticField, cfg: &TurbScanConfig, ra_ft: f64) -> TurbulenceScanner {
    let mut s = TurbulenceScanner::new(TurbConfig::default());
    let mut o = own();
    o.ra_ft = ra_ft;
    s.step(&o, cfg, field, 100_000, 0.);
    s
}

#[test]
fn turbulent_cell_inside_forty_miles_paints_magenta_in_wx_plus_t() {
    let mut f = storm(25., 0.8, 0.5);
    let s = sweep(&mut f, &scan_cfg(80.), 9000.);
    let r = s.returns(9000., WxrMode::WxTurb, false, 1.);
    assert!(!r.is_empty());
    assert!(r.iter().all(|t| t.range_nm <= 40. && t.sigma_ms >= 5.));
    assert!(r.iter().any(|t| t.azimuth_deg.abs() < 6. && (t.range_nm - 25.).abs() < 6.));
}

#[test]
fn turbulence_beyond_forty_miles_is_not_detected() {
    let mut f = storm(60., 0.9, 0.5);
    let s = sweep(&mut f, &scan_cfg(80.), 9000.);
    assert!(s.returns(9000., WxrMode::WxTurb, false, 1.).is_empty());
}

#[test]
fn mild_cells_and_dry_air_do_not_paint_magenta() {
    let mut f = storm(25., 0.3, 0.5);
    let s = sweep(&mut f, &scan_cfg(80.), 9000.);
    assert!(s.returns(9000., WxrMode::WxTurb, false, 1.).is_empty());
    let mut f = storm(25., 0.9, 0.0);
    let s = sweep(&mut f, &scan_cfg(80.), 9000.);
    assert!(s.returns(9000., WxrMode::WxTurb, false, 1.).is_empty());
}

#[test]
fn mode_and_auto_rules() {
    let mut f = storm(25., 0.8, 0.5);
    let s = sweep(&mut f, &scan_cfg(80.), 9000.);
    assert!(s.returns(9000., WxrMode::Wx, false, 1.).is_empty());
    assert!(s.returns(9000., WxrMode::Wx, true, 1.).is_empty());
    assert!(!s.returns(9000., WxrMode::Turb, false, 1.).is_empty());
    assert!(s.returns(9000., WxrMode::Map, true, 1.).is_empty());

    let mut f = storm(25., 0.8, 0.5);
    let s = sweep(&mut f, &scan_cfg(80.), 1500.);
    assert!(!s.returns(1500., WxrMode::Wx, true, 1.).is_empty());
    assert!(s.returns(1500., WxrMode::Wx, false, 1.).is_empty());
}

#[test]
fn returns_are_dropped_when_old() {
    let mut f = storm(25., 0.8, 0.5);
    let s = sweep(&mut f, &scan_cfg(80.), 9000.);
    assert!(s.returns(9000., WxrMode::WxTurb, false, 60.).is_empty());
}

#[test]
fn shear_across_the_beam_adds_spectrum_width_in_rain() {
    let mut f = SyntheticField { bursts: vec![Microburst::new(12. * NM, 0., 3000., 30., 0.).wet(0.6)], ..Default::default() };
    let mut cfg = scan_cfg(20.);
    cfg.fixed_alt_m = Some(200.);
    let s = sweep(&mut f, &cfg, 650.);
    assert!(!s.returns(650., WxrMode::WxTurb, false, 1.).is_empty());
}

#[test]
fn the_scan_config_change_restarts_the_grid() {
    let mut f = storm(25., 0.8, 0.5);
    let mut s = sweep(&mut f, &scan_cfg(80.), 9000.);
    assert!(!s.returns(9000., WxrMode::WxTurb, false, 1.).is_empty());
    s.step(&own(), &scan_cfg(40.), &mut f, 0, 1.);
    assert!(s.returns(9000., WxrMode::WxTurb, false, 1.).is_empty());
}

fn vd_cfg() -> VdConfig {
    VdConfig { cols: 80, rows: 42, range_nm: 40., alt_min_ft: -1000., alt_max_ft: 41_000., ..Default::default() }
}

fn vd_sweep(field: &mut SyntheticField, cfg: &VdConfig) -> VdScanner {
    let mut v = VdScanner::new();
    v.step(&own(), 90., cfg, field, 100_000);
    v
}

#[test]
fn vertical_slice_shows_the_storm_between_base_and_top_only() {
    let mut f = storm(20., 0.2, 0.7);
    f.storms[0].top_m = 8000.;
    let cfg = vd_cfg();
    let v = vd_sweep(&mut f, &cfg);
    let img = v.image().expect("sweep complete");
    assert_eq!((img.cols, img.rows), (80, 42));
    let col = (20. / 40. * 80.) as usize;
    let row_of = |ft: f64| ((cfg.alt_max_ft - ft) / (cfg.alt_max_ft - cfg.alt_min_ft) * cfg.rows as f64) as usize;
    assert!(img.level(col, row_of(20_000.)) >= Level::Green);
    assert!(img.level(col, row_of(5_000.)) >= Level::Green);
    assert_eq!(img.level(col, row_of(30_000.)), Level::None);
    assert_eq!(img.level(5, row_of(20_000.)), Level::None);
    assert_eq!(img.level(70, row_of(5_000.)), Level::None);
}

#[test]
fn vertical_slice_paints_turbulence_magenta_inside_forty_miles_only() {
    let cfg = VdConfig { range_nm: 80., ..vd_cfg() };
    let mut f = storm(20., 0.9, 0.5);
    let v = vd_sweep(&mut f, &cfg);
    let img = v.image().unwrap();
    assert!(img.levels.contains(&Level::Magenta));
    let mut far = storm(60., 0.9, 0.5);
    let v = vd_sweep(&mut far, &cfg);
    assert!(!v.image().unwrap().levels.contains(&Level::Magenta));
    assert!(v.image().unwrap().levels.iter().any(|l| *l >= Level::Green));
}

#[test]
fn vertical_slice_follows_the_requested_bearing_and_publishes_per_sweep() {
    let mut f = storm(20., 0.2, 0.7);
    let cfg = vd_cfg();
    let mut v = VdScanner::new();
    v.step(&own(), 0., &cfg, &mut f, 1000);
    assert!(v.image().is_none());
    v.step(&own(), 0., &cfg, &mut f, 100_000);
    let img = v.image().unwrap();
    assert!(img.levels.iter().all(|l| *l == Level::None));
    assert_eq!(img.generation, 1);
    assert_eq!(img.rgba().len(), img.cols * img.rows * 4);
}

#[test]
fn heavy_rain_in_the_foreground_attenuates_the_slice_behind_it() {
    let mut f = SyntheticField {
        storms: vec![
            StormCell { east_m: 10. * NM, north_m: 0., radius_m: 10. * NM, peak_precip: 1.0, peak_turbulence: 0., base_m: 0., top_m: 12_000. },
            StormCell { east_m: 30. * NM, north_m: 0., radius_m: 2. * NM, peak_precip: 0.45, peak_turbulence: 0., base_m: 0., top_m: 12_000. },
        ],
        ..Default::default()
    };
    let cfg = vd_cfg();
    let shadowed = vd_sweep(&mut f, &cfg);
    let mut alone = SyntheticField { storms: vec![f.storms[1]], ..Default::default() };
    let clear = vd_sweep(&mut alone, &cfg);
    let (col, row) = (60, 21);
    assert!(shadowed.image().unwrap().level(col, row) < clear.image().unwrap().level(col, row));
}

use gpws::sound::SoundManager;
use gpws::taws::{
    angle_diff, ne_nm, offset_nm, AlertLevel, Obstacle, ObstacleSource, Runway, Taws, TawsInputs, TerrainSampler,
};

const LAT0: f64 = 46.;
const LON0: f64 = 8.;

fn terrain_fn<'a>(f: impl Fn(f64, f64) -> f64 + 'a) -> impl TerrainSampler + 'a {
    move |lat: f64, lon: f64| {
        let (n, e) = ne_nm(LAT0, LON0, lat, lon);
        Some(f(n, e))
    }
}

struct Sim {
    n: f64,
    e: f64,
    alt: f64,
    gs: f64,
    hdg: f64,
    vs: f64,
    rate: f64,
    landing_config: bool,
    glideslope: bool,
    ra: Option<f64>,
    terr_off: bool,
    on_ground: bool,
}

impl Sim {
    fn new(n: f64, e: f64, alt: f64, gs: f64, hdg: f64, vs: f64) -> Sim {
        Sim { n, e, alt, gs, hdg, vs, rate: 0., landing_config: false, glideslope: false, ra: None, terr_off: false, on_ground: false }
    }

    fn inputs<'a>(&self, runways: &'a [Runway]) -> TawsInputs<'a> {
        let (lat, lon) = offset_nm(LAT0, LON0, self.n, self.e);
        TawsInputs {
            lat,
            lon,
            alt_ft: self.alt,
            radio_alt_ft: self.ra,
            gs_kt: self.gs,
            track_deg: self.hdg,
            track_rate_dps: self.rate,
            vs_fpm: self.vs,
            on_ground: self.on_ground,
            landing_config: self.landing_config,
            glideslope_tracking: self.glideslope,
            terr_off: self.terr_off,
            runways,
            ..TawsInputs::default()
        }
    }

    fn advance(&mut self, dt_s: f64) {
        let h = (self.hdg + self.rate * dt_s * 0.5).to_radians();
        let d = self.gs / 3600. * dt_s;
        self.n += d * h.cos();
        self.e += d * h.sin();
        self.hdg += self.rate * dt_s;
        self.alt += self.vs / 60. * dt_s;
    }
}

#[derive(Default)]
struct Log {
    first_caution: Option<f64>,
    first_warning: Option<f64>,
    any_alert: bool,
    max_level: AlertLevel,
    pda: bool,
    tcf: bool,
    obstacle_caution: bool,
    obstacle_warning: bool,
    unavailable: bool,
    impact: Option<f64>,
    sounds: Vec<&'static str>,
}

fn run(
    sim: &mut Sim,
    runways: &[Runway],
    terrain: &dyn TerrainSampler,
    obstacles: Option<&dyn ObstacleSource>,
    seconds: f64,
    stop_at_warning_plus: Option<f64>,
) -> Log {
    let mut taws = Taws::new();
    let mut sm = SoundManager::new();
    let mut log = Log::default();
    let dt = 0.1;
    let mut t = 0.;
    while t < seconds {
        taws.update(dt * 1000., &sim.inputs(runways), terrain, obstacles);
        taws.apply_to(&mut sm);
        sm.update(dt * 1000.);
        log.sounds.extend(sm.take_played());
        let o = taws.outputs();
        if o.flta != AlertLevel::None || o.obstacle != AlertLevel::None || o.premature_descent || o.clearance_floor {
            log.any_alert = true;
        }
        log.max_level = log.max_level.max(o.flta);
        log.pda |= o.premature_descent;
        log.tcf |= o.clearance_floor;
        log.obstacle_caution |= o.obstacle == AlertLevel::Caution;
        log.obstacle_warning |= o.obstacle == AlertLevel::Warning;
        log.unavailable |= o.nd.terrain_unavailable;
        if o.flta >= AlertLevel::Caution && log.first_caution.is_none() {
            log.first_caution = Some(t);
        }
        if o.flta == AlertLevel::Warning && log.first_warning.is_none() {
            log.first_warning = Some(t);
        }
        let (lat, lon) = offset_nm(LAT0, LON0, sim.n, sim.e);
        if let Some(g) = terrain.elevation_ft(lat, lon) {
            if sim.alt <= g && log.impact.is_none() {
                log.impact = Some(t);
            }
        }
        if let (Some(w), Some(extra)) = (log.first_warning, stop_at_warning_plus) {
            if t >= w + extra {
                break;
            }
        }
        sim.advance(dt);
        t += dt;
    }
    log
}

fn runway_at_origin() -> [Runway; 1] {
    let (lat, lon) = offset_nm(LAT0, LON0, 0., 0.);
    [Runway { lat, lon, heading_deg: 0., length_ft: 10000., elevation_ft: 500. }]
}

#[test]
fn geometry_helpers_agree_with_each_other() {
    let (lat, lon) = offset_nm(LAT0, LON0, 3., -4.);
    let (n, e) = ne_nm(LAT0, LON0, lat, lon);
    assert!((n - 3.).abs() < 1e-9 && (e + 4.).abs() < 1e-9);
    assert_eq!(angle_diff(350., 10.), -20.);
    assert_eq!(angle_diff(10., 350.), 20.);
}

#[test]
fn flat_terrain_at_cruise_stays_silent() {
    let terrain = terrain_fn(|_, _| 500.);
    let mut sim = Sim::new(-100., 0., 35000., 480., 0., 0.);
    let log = run(&mut sim, &[], &terrain, None, 300., None);
    assert!(!log.any_alert);
    assert!(log.sounds.is_empty());
}

#[test]
fn ridge_ahead_gives_caution_then_pull_up_warning_with_ground_speed_scaled_envelope() {
    let terrain = terrain_fn(|n, e| if (0. ..=2.).contains(&n) && e.abs() < 20. { 8300. } else { 500. });
    let mut sim = Sim::new(-8., 0., 8000., 250., 0., 0.);
    let log = run(&mut sim, &[], &terrain, None, 120., Some(3.));
    let caution = log.first_caution.expect("caution");
    let warning = log.first_warning.expect("warning");
    let caution_range = 8. - 250. / 3600. * caution;
    let warning_range = 8. - 250. / 3600. * warning;
    assert!((3.3..=4.5).contains(&caution_range), "caution at {caution_range} nm");
    assert!((1.4..=2.3).contains(&warning_range), "warning at {warning_range} nm");
    assert!(warning > caution);
    assert!(log.sounds.contains(&"aural_terrain_ahead"));
    assert!(log.sounds.contains(&"aural_terrain_ahead_pull_up"));

    let mut fast = Sim::new(-12., 0., 8000., 500., 0., 0.);
    let fast_log = run(&mut fast, &[], &terrain, None, 120., None);
    let fast_range = 12. - 500. / 3600. * fast_log.first_caution.expect("fast caution");
    assert!(fast_range > caution_range + 3., "fast envelope {fast_range} vs slow {caution_range}");
}

#[test]
fn ridge_well_below_the_aircraft_or_beside_the_track_is_silent() {
    let low_ridge = terrain_fn(|n, _| if (0. ..=2.).contains(&n) { 6900. } else { 500. });
    let mut sim = Sim::new(-8., 0., 8000., 250., 0., 0.);
    assert!(!run(&mut sim, &[], &low_ridge, None, 120., None).any_alert);

    let side_ridge = terrain_fn(|n, e| if (-3. ..=6.).contains(&n) && (3. ..=6.).contains(&e) { 9000. } else { 500. });
    let mut sim = Sim::new(-8., 0., 8000., 250., 0., 0.);
    assert!(!run(&mut sim, &[], &side_ridge, None, 120., None).any_alert);
}

#[test]
fn marginal_clearance_gives_caution_without_warning() {
    let terrain = terrain_fn(|n, _| if (0. ..=2.).contains(&n) { 7500. } else { 500. });
    let mut sim = Sim::new(-8., 0., 8000., 250., 0., 0.);
    let log = run(&mut sim, &[], &terrain, None, 120., None);
    assert_eq!(log.max_level, AlertLevel::Caution);
    assert!(log.sounds.contains(&"aural_terrain_ahead"));
    assert!(!log.sounds.contains(&"aural_terrain_ahead_pull_up"));
}

#[test]
fn rising_slope_is_flagged_well_before_impact() {
    let terrain = terrain_fn(|n, _| if n > -10. { 500. + 500. * (n + 10.) } else { 500. });
    let mut sim = Sim::new(-12., 0., 3000., 160., 0., -600.);
    let log = run(&mut sim, &[], &terrain, None, 400., None);
    let caution = log.first_caution.expect("caution");
    let warning = log.first_warning.expect("warning");
    let impact = log.impact.expect("the slope is hit when nobody reacts");
    assert!(caution < warning);
    assert!(impact - caution >= 35., "caution {caution} impact {impact}");
    assert!(impact - warning >= 12., "warning {warning} impact {impact}");
}

#[test]
fn turning_into_a_ridge_alerts_where_straight_flight_does_not() {
    let radius = 250. / 3600. / (3f64.to_radians());
    let terrain = terrain_fn(move |n, e| {
        let (cn, ce) = (radius, radius);
        if ((n - cn).powi(2) + (e - ce).powi(2)).sqrt() < 0.4 { 9000. } else { 500. }
    });
    let mut straight = Sim::new(0., 0., 8000., 250., 0., 0.);
    assert!(!run(&mut straight, &[], &terrain, None, 60., None).any_alert);

    let mut turning = Sim::new(0., 0., 8000., 250., 0., 0.);
    turning.rate = 3.;
    assert!(run(&mut turning, &[], &terrain, None, 20., None).first_caution.is_some());
}

#[test]
fn final_approach_to_the_runway_is_inhibited_over_a_hill_under_the_path() {
    let runways = runway_at_origin();
    let terrain = terrain_fn(|n, e| if (-3.5..=-2.).contains(&n) && e.abs() < 1. { 1000. } else { 500. });
    let slope = 3f64.to_radians().tan();
    let start_n = -8.;
    let mut sim = Sim::new(start_n, 0., 500. + 8. * 6076.12 * slope, 140., 0., -(140. * 101.27 * slope));
    sim.landing_config = true;
    let log = run(&mut sim, &runways, &terrain, None, 8. * 3600. / 140. - 20., None);
    assert!(!log.any_alert, "alerts on a normal approach: {:?}", log.sounds);

    let mut control = Sim::new(start_n, 0., 500. + 8. * 6076.12 * slope, 140., 0., -(140. * 101.27 * slope));
    let control_log = run(&mut control, &[], &terrain, None, 8. * 3600. / 140. - 20., None);
    assert!(control_log.first_caution.is_some(), "the same hill must alert without runway and landing config");
}

#[test]
fn over_the_runway_nothing_alerts() {
    let runways = runway_at_origin();
    let terrain = terrain_fn(|n, _| if n > 1.5 { 900. } else { 500. });
    let mut sim = Sim::new(-0.2, 0., 700., 130., 0., 0.);
    sim.ra = Some(200.);
    let log = run(&mut sim, &runways, &terrain, None, 10., None);
    assert!(!log.any_alert);
}

#[test]
fn premature_descent_alerts_below_the_floor_and_not_on_the_glideslope_or_normal_path() {
    let runways = runway_at_origin();
    let terrain = terrain_fn(|_, _| 500.);
    let mut low = Sim::new(-4., 0., 900., 140., 0., -700.);
    low.landing_config = true;
    low.ra = Some(400.);
    let log = run(&mut low, &runways, &terrain, None, 8., None);
    assert!(log.pda);
    assert!(log.sounds.contains(&"aural_too_low_terrain"));
    assert!(!log.any_alert || log.first_caution.is_none());

    let mut on_gs = Sim::new(-4., 0., 900., 140., 0., -700.);
    on_gs.landing_config = true;
    on_gs.glideslope = true;
    on_gs.ra = Some(400.);
    assert!(!run(&mut on_gs, &runways, &terrain, None, 8., None).pda);

    let slope = 3f64.to_radians().tan();
    let mut normal = Sim::new(-4., 0., 500. + 4. * 6076.12 * slope, 140., 0., -743.);
    normal.landing_config = true;
    normal.ra = Some(4. * 6076.12 * slope);
    assert!(!run(&mut normal, &runways, &terrain, None, 8., None).any_alert);

    let mut go_around = Sim::new(-4., 0., 900., 140., 0., 1500.);
    go_around.ra = Some(400.);
    assert!(!run(&mut go_around, &runways, &terrain, None, 8., None).pda);
}

#[test]
fn terrain_clearance_floor_depends_on_distance_from_the_runway() {
    let runways = runway_at_origin();
    let terrain = terrain_fn(|_, _| 500.);
    let mut low = Sim::new(0., 8., 900., 200., 90., 0.);
    low.ra = Some(150.);
    assert!(run(&mut low, &runways, &terrain, None, 5., None).tcf);

    let mut high = Sim::new(0., 8., 1400., 200., 90., 0.);
    high.ra = Some(600.);
    assert!(!run(&mut high, &runways, &terrain, None, 5., None).any_alert);

    let mut near = Sim::new(0., 0.8, 650., 200., 90., 0.);
    near.ra = Some(150.);
    let log = run(&mut near, &runways, &terrain, None, 2., None);
    assert!(!log.tcf);
}

#[test]
fn terr_off_and_ground_inhibit_everything_and_report_the_switch() {
    let terrain = terrain_fn(|n, _| if (0. ..=2.).contains(&n) { 8300. } else { 500. });
    let mut off = Sim::new(-8., 0., 8000., 250., 0., 0.);
    off.terr_off = true;
    let mut taws = Taws::new();
    let mut flagged = false;
    for _ in 0..600 {
        taws.update(100., &off.inputs(&[]), &terrain, None);
        flagged |= taws.outputs().nd.terr_off;
        assert_eq!(taws.outputs().flta, AlertLevel::None);
        off.advance(0.1);
    }
    assert!(flagged);
    assert!(!run(&mut off, &[], &terrain, None, 60., None).any_alert);

    let mut ground = Sim::new(-0.5, 0., 8000., 250., 0., 0.);
    ground.on_ground = true;
    assert!(!run(&mut ground, &[], &terrain, None, 60., None).any_alert);
}

#[test]
fn alerts_clear_after_the_threat_is_gone_and_the_aural_stops() {
    let terrain = terrain_fn(|n, _| if (0. ..=2.).contains(&n) { 8300. } else { 500. });
    let mut sim = Sim::new(-3., 0., 8000., 250., 0., 0.);
    let mut taws = Taws::new();
    let mut sm = SoundManager::new();
    for _ in 0..200 {
        taws.update(100., &sim.inputs(&[]), &terrain, None);
        taws.apply_to(&mut sm);
        sim.advance(0.1);
    }
    assert_eq!(taws.outputs().flta, AlertLevel::Warning);
    assert!(taws.outputs().nd.pull_up && taws.outputs().pull_up_light);
    assert!(taws.outputs().threat.is_some());
    sim.vs = 4000.;
    sim.alt = 12000.;
    for _ in 0..150 {
        taws.update(100., &sim.inputs(&[]), &terrain, None);
        taws.apply_to(&mut sm);
        sim.advance(0.1);
    }
    assert_eq!(taws.outputs().flta, AlertLevel::None);
    assert!(!taws.outputs().nd.terrain_popup);
    assert!(taws.desired_sound().is_none());
    assert!(sm.periodic_names().is_empty());
}

#[test]
fn missing_terrain_data_is_reported_not_alerted() {
    let nothing = |_: f64, _: f64| -> Option<f64> { None };
    let mut sim = Sim::new(-8., 0., 3000., 250., 0., -500.);
    let log = run(&mut sim, &[], &nothing, None, 20., None);
    assert!(!log.any_alert);
    assert!(log.unavailable);
}

struct Towers(Vec<Obstacle>);

impl ObstacleSource for Towers {
    fn obstacles_near(&self, _lat: f64, _lon: f64, _radius_nm: f64, out: &mut Vec<Obstacle>) {
        out.extend_from_slice(&self.0);
    }
}

#[test]
fn obstacles_alert_only_when_a_database_is_attached() {
    let terrain = terrain_fn(|_, _| 500.);
    let (lat, lon) = offset_nm(LAT0, LON0, 0., 0.2);
    let towers = Towers(vec![Obstacle { lat, lon, top_ft: 2150. }]);

    let mut with = Sim::new(-6., 0., 2000., 150., 0., 0.);
    let log = run(&mut with, &[], &terrain, Some(&towers), 120., None);
    assert!(log.obstacle_caution && log.obstacle_warning);
    assert!(log.sounds.contains(&"aural_obstacle_ahead"));
    assert!(log.sounds.contains(&"aural_obstacle_ahead_pull_up"));

    let mut without = Sim::new(-6., 0., 2000., 150., 0., 0.);
    let log = run(&mut without, &[], &terrain, None, 120., None);
    assert!(!log.any_alert);
    let mut taws = Taws::new();
    taws.update(600., &without.inputs(&[]), &terrain, None);
    assert!(!taws.outputs().obstacle_data_available);
    taws.update(600., &without.inputs(&[]), &terrain, Some(&towers));
    assert!(taws.outputs().obstacle_data_available);

    let mut far_off = Sim::new(-6., 3., 2000., 150., 0., 0.);
    assert!(!run(&mut far_off, &[], &terrain, Some(&towers), 120., None).any_alert);
}

#[test]
fn probe_count_per_cycle_is_bounded() {
    let terrain = terrain_fn(|_, _| 500.);
    let mut taws = Taws::new();
    for gs in [60., 150., 300., 500.] {
        let sim = Sim::new(0., 0., 20000., gs, 0., 0.);
        taws.update(600., &sim.inputs(&[]), &terrain, None);
        assert!(taws.probes_last_cycle() <= 200, "{gs} kt: {}", taws.probes_last_cycle());
        assert!(taws.probes_last_cycle() > 0);
    }
}

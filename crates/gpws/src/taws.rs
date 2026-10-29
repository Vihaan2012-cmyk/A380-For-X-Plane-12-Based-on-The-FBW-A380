use crate::sound::{self, Sound, SoundManager};
use crate::throttle::Throttler;

pub const NM_FT: f64 = 6076.12;

pub mod params {
    pub const CYCLE_MS: f64 = 500.;
    pub const MIN_GS_KT: f64 = 40.;
    pub const CAUTION_TIME_S: f64 = 60.;
    pub const WARNING_TIME_S: f64 = 30.;
    pub const TERMINAL_CAUTION_TIME_S: f64 = 45.;
    pub const TERMINAL_WARNING_TIME_S: f64 = 20.;
    pub const TERMINAL_RADIUS_NM: f64 = 15.;
    pub const MAX_LOOKAHEAD_NM: f64 = 12.;
    pub const STEP_NM: f64 = 0.2;
    pub const STEP_MIN_S: f64 = 1.;
    pub const STEP_MAX_S: f64 = 3.;
    pub const CLIMB_CREDIT: f64 = 0.5;
    pub const WARNING_CLEARANCE_FRACTION: f64 = 0.3;
    pub const CONE_HALF_ANGLE_DEG: f64 = 4.;
    pub const CONE_TURN_EXTRA_DEG_PER_DPS: f64 = 2.5;
    pub const CONE_TURN_EXTRA_MAX_DEG: f64 = 8.;
    pub const CONE_START_WIDTH_NM: f64 = 0.2;
    pub const MAX_TURN_RATE_DPS: f64 = 3.;
    pub const CLEARANCE_PROFILE: [(f64, f64); 3] = [(1., 100.), (8., 500.), (15., 700.)];
    pub const OBSTACLE_CAUTION_CLEARANCE_FT: f64 = 300.;
    pub const OBSTACLE_WARNING_CLEARANCE_FT: f64 = 100.;
    pub const OBSTACLE_CORRIDOR_NM: f64 = 0.3;
    pub const RUNWAY_ALIGN_TRACK_DEG: f64 = 30.;
    pub const RUNWAY_ALIGN_CONE_DEG: f64 = 12.;
    pub const RUNWAY_ALIGN_WIDTH_NM: f64 = 0.3;
    pub const APPROACH_RANGE_NM: f64 = 15.;
    pub const FINAL_INHIBIT_NM: f64 = 3.;
    pub const FINAL_DESENSITISE_NM: f64 = 7.5;
    pub const OVER_RUNWAY_BEFORE_NM: f64 = 0.3;
    pub const OVER_RUNWAY_AFTER_NM: f64 = 1.;
    pub const OVER_RUNWAY_HALF_WIDTH_NM: f64 = 0.5;
    pub const OVER_RUNWAY_MAX_AGL_FT: f64 = 2500.;
    pub const PDA_MIN_NM: f64 = 0.5;
    pub const PDA_MAX_NM: f64 = 5.;
    pub const PDA_FLOOR_PATH_DEG: f64 = 1.7;
    pub const PDA_MIN_HEIGHT_FT: f64 = 30.;
    pub const PDA_MAX_RA_FT: f64 = 1000.;
    pub const PDA_MAX_CLIMB_FPM: f64 = 500.;
    pub const TCF_MIN_NM: f64 = 1.;
    pub const TCF_MAX_NM: f64 = 15.;
    pub const TCF_FLOOR_PROFILE: [(f64, f64); 2] = [(1., 100.), (15., 700.)];
    pub const HOLD_CAUTION_S: f64 = 2.;
    pub const HOLD_WARNING_S: f64 = 3.;
    pub const CAUTION_AURAL_PERIOD_S: f64 = 7.;
    pub const WARNING_AURAL_PERIOD_S: f64 = 2.;
    pub const TOO_LOW_AURAL_PERIOD_S: f64 = 3.;
    pub const MAX_PROBES_PER_CYCLE: usize = 200;
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub enum AlertLevel {
    #[default]
    None,
    Caution,
    Warning,
}

pub trait TerrainSampler {
    fn elevation_ft(&self, lat_deg: f64, lon_deg: f64) -> Option<f64>;
}

impl<F> TerrainSampler for F
where
    F: Fn(f64, f64) -> Option<f64>,
{
    fn elevation_ft(&self, lat_deg: f64, lon_deg: f64) -> Option<f64> {
        self(lat_deg, lon_deg)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Obstacle {
    pub lat: f64,
    pub lon: f64,
    pub top_ft: f64,
}

pub trait ObstacleSource {
    fn obstacles_near(&self, lat_deg: f64, lon_deg: f64, radius_nm: f64, out: &mut Vec<Obstacle>);
}

#[derive(Clone, Copy, Debug)]
pub struct Runway {
    pub lat: f64,
    pub lon: f64,
    pub heading_deg: f64,
    pub length_ft: f64,
    pub elevation_ft: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TawsInputs<'a> {
    pub lat: f64,
    pub lon: f64,
    pub alt_ft: f64,
    pub radio_alt_ft: Option<f64>,
    pub gs_kt: f64,
    pub track_deg: f64,
    pub track_rate_dps: f64,
    pub vs_fpm: f64,
    pub on_ground: bool,
    pub landing_config: bool,
    pub glideslope_tracking: bool,
    pub terr_off: bool,
    pub system_off: bool,
    pub failed: bool,
    pub runways: &'a [Runway],
}

#[derive(Clone, Copy, Debug)]
pub struct Threat {
    pub lat: f64,
    pub lon: f64,
    pub distance_nm: f64,
    pub level: AlertLevel,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NdFlags {
    pub terrain_popup: bool,
    pub terrain_caution: bool,
    pub terrain_warning: bool,
    pub obstacle_caution: bool,
    pub obstacle_warning: bool,
    pub too_low_terrain: bool,
    pub pull_up: bool,
    pub terr_off: bool,
    pub terrain_unavailable: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TawsOutputs {
    pub flta: AlertLevel,
    pub obstacle: AlertLevel,
    pub premature_descent: bool,
    pub clearance_floor: bool,
    pub pull_up_light: bool,
    pub caution_light: bool,
    pub obstacle_data_available: bool,
    pub threat: Option<Threat>,
    pub nd: NdFlags,
}

#[derive(Clone, Copy, Debug, Default)]
struct Hold {
    level: AlertLevel,
    remaining_s: f64,
}

impl Hold {
    fn update(&mut self, raw: AlertLevel, dt_s: f64) -> AlertLevel {
        if raw >= self.level {
            self.level = raw;
            self.remaining_s = if raw == AlertLevel::Warning { params::HOLD_WARNING_S } else { params::HOLD_CAUTION_S };
        } else {
            self.remaining_s -= dt_s;
            if self.remaining_s <= 0. {
                self.level = raw;
                self.remaining_s = if raw == AlertLevel::None { 0. } else { params::HOLD_CAUTION_S };
            }
        }
        self.level
    }

    fn clear(&mut self) {
        *self = Hold::default();
    }
}

pub fn ne_nm(ref_lat: f64, ref_lon: f64, lat: f64, lon: f64) -> (f64, f64) {
    let mut dlon = lon - ref_lon;
    if dlon > 180. {
        dlon -= 360.;
    } else if dlon < -180. {
        dlon += 360.;
    }
    ((lat - ref_lat) * 60., dlon * 60. * ref_lat.to_radians().cos())
}

pub fn offset_nm(lat: f64, lon: f64, north_nm: f64, east_nm: f64) -> (f64, f64) {
    let new_lat = lat + north_nm / 60.;
    let cos = lat.to_radians().cos().max(1e-6);
    (new_lat, lon + east_nm / (60. * cos))
}

pub fn angle_diff(a: f64, b: f64) -> f64 {
    let mut d = (a - b) % 360.;
    if d > 180. {
        d -= 360.;
    } else if d < -180. {
        d += 360.;
    }
    d
}

fn interpolate(profile: &[(f64, f64)], x: f64) -> f64 {
    if x <= profile[0].0 {
        return profile[0].1;
    }
    for pair in profile.windows(2) {
        if x <= pair[1].0 {
            let f = (x - pair[0].0) / (pair[1].0 - pair[0].0);
            return pair[0].1 + f * (pair[1].1 - pair[0].1);
        }
    }
    profile[profile.len() - 1].1
}

struct RunwayRel {
    along_nm: f64,
    cross_nm: f64,
    segment_nm: f64,
}

fn runway_rel(rw: &Runway, lat: f64, lon: f64) -> RunwayRel {
    let (n, e) = ne_nm(rw.lat, rw.lon, lat, lon);
    let h = rw.heading_deg.to_radians();
    let along = n * h.cos() + e * h.sin();
    let cross = e * h.cos() - n * h.sin();
    let len = rw.length_ft / NM_FT;
    let clamped = along.clamp(0., len);
    RunwayRel { along_nm: along, cross_nm: cross, segment_nm: ((along - clamped).powi(2) + cross * cross).sqrt() }
}

fn nearest_runway_nm(runways: &[Runway], lat: f64, lon: f64) -> f64 {
    runways.iter().map(|r| runway_rel(r, lat, lon).segment_nm).fold(f64::INFINITY, f64::min)
}

fn terrain_clearance_ft(distance_to_runway_nm: f64) -> f64 {
    if distance_to_runway_nm.is_finite() {
        interpolate(&params::CLEARANCE_PROFILE, distance_to_runway_nm)
    } else {
        params::CLEARANCE_PROFILE[params::CLEARANCE_PROFILE.len() - 1].1
    }
}

#[derive(Clone, Copy)]
struct Approach {
    runway: usize,
    distance_nm: f64,
}

struct Context {
    over_runway: bool,
    approach: Option<Approach>,
    nearest_runway_nm: f64,
}

fn context(input: &TawsInputs) -> Context {
    let mut over_runway = false;
    let mut approach: Option<Approach> = None;
    for (i, rw) in input.runways.iter().enumerate() {
        let rel = runway_rel(rw, input.lat, input.lon);
        let len = rw.length_ft / NM_FT;
        let agl = input.alt_ft - rw.elevation_ft;
        if rel.along_nm >= -params::OVER_RUNWAY_BEFORE_NM
            && rel.along_nm <= len + params::OVER_RUNWAY_AFTER_NM
            && rel.cross_nm.abs() <= params::OVER_RUNWAY_HALF_WIDTH_NM
            && agl <= params::OVER_RUNWAY_MAX_AGL_FT
        {
            over_runway = true;
        }
        let d = -rel.along_nm;
        let width = params::RUNWAY_ALIGN_WIDTH_NM + d * params::RUNWAY_ALIGN_CONE_DEG.to_radians().tan();
        if d > 0.
            && d <= params::APPROACH_RANGE_NM
            && rel.cross_nm.abs() <= width
            && angle_diff(input.track_deg, rw.heading_deg).abs() <= params::RUNWAY_ALIGN_TRACK_DEG
            && approach.map_or(true, |a| d < a.distance_nm)
        {
            approach = Some(Approach { runway: i, distance_nm: d });
        }
    }
    Context { over_runway, approach, nearest_runway_nm: nearest_runway_nm(input.runways, input.lat, input.lon) }
}

struct Scan {
    terrain: AlertLevel,
    obstacle: AlertLevel,
    threat: Option<Threat>,
    samples: usize,
    missing: usize,
}

#[derive(Clone, Debug)]
pub struct Taws {
    throttler: Throttler,
    flta_hold: Hold,
    obstacle_hold: Hold,
    pda_hold: Hold,
    tcf_hold: Hold,
    outputs: TawsOutputs,
    applied: Option<Sound>,
    probes_last_cycle: usize,
    obstacle_scratch: Vec<Obstacle>,
}

impl Default for Taws {
    fn default() -> Self {
        Taws::new()
    }
}

impl Taws {
    pub fn new() -> Taws {
        Taws {
            throttler: Throttler::new(params::CYCLE_MS, 0.),
            flta_hold: Hold::default(),
            obstacle_hold: Hold::default(),
            pda_hold: Hold::default(),
            tcf_hold: Hold::default(),
            outputs: TawsOutputs::default(),
            applied: None,
            probes_last_cycle: 0,
            obstacle_scratch: Vec::new(),
        }
    }

    pub fn outputs(&self) -> TawsOutputs {
        self.outputs
    }

    pub fn probes_last_cycle(&self) -> usize {
        self.probes_last_cycle
    }

    pub fn update(
        &mut self,
        delta_ms: f64,
        input: &TawsInputs,
        terrain: &dyn TerrainSampler,
        obstacles: Option<&dyn ObstacleSource>,
    ) {
        let elapsed = self.throttler.can_update(delta_ms);
        if elapsed > 0. {
            self.evaluate(elapsed / 1000., input, terrain, obstacles);
        }
    }

    pub fn desired_sound(&self) -> Option<(Sound, f64)> {
        let o = &self.outputs;
        if o.flta == AlertLevel::Warning {
            Some((sound::TERRAIN_AHEAD_PULL_UP, params::WARNING_AURAL_PERIOD_S))
        } else if o.obstacle == AlertLevel::Warning {
            Some((sound::OBSTACLE_AHEAD_PULL_UP, params::WARNING_AURAL_PERIOD_S))
        } else if o.flta == AlertLevel::Caution {
            Some((sound::TERRAIN_AHEAD, params::CAUTION_AURAL_PERIOD_S))
        } else if o.obstacle == AlertLevel::Caution {
            Some((sound::OBSTACLE_AHEAD, params::CAUTION_AURAL_PERIOD_S))
        } else if o.premature_descent || o.clearance_floor {
            Some((sound::TOO_LOW_TERRAIN, params::TOO_LOW_AURAL_PERIOD_S))
        } else {
            None
        }
    }

    pub fn apply_to(&mut self, manager: &mut SoundManager) {
        let wanted = self.desired_sound();
        let wanted_sound = wanted.map(|(s, _)| s);
        if wanted_sound != self.applied {
            manager.remove_periodic_sound(self.applied);
            if let Some((s, period)) = wanted {
                manager.add_periodic_sound(Some(s), period);
            }
            self.applied = wanted_sound;
        }
    }

    fn reset(&mut self, input: &TawsInputs, obstacles_available: bool) {
        self.flta_hold.clear();
        self.obstacle_hold.clear();
        self.pda_hold.clear();
        self.tcf_hold.clear();
        self.probes_last_cycle = 0;
        self.outputs = TawsOutputs {
            obstacle_data_available: obstacles_available,
            nd: NdFlags { terr_off: input.terr_off, ..NdFlags::default() },
            ..TawsOutputs::default()
        };
    }

    pub fn evaluate(
        &mut self,
        dt_s: f64,
        input: &TawsInputs,
        terrain: &dyn TerrainSampler,
        obstacles: Option<&dyn ObstacleSource>,
    ) {
        let obstacles_available = obstacles.is_some();
        let data_ok = input.lat.is_finite()
            && input.lon.is_finite()
            && input.alt_ft.is_finite()
            && input.gs_kt.is_finite()
            && input.track_deg.is_finite();
        if input.terr_off || input.system_off || input.failed || input.on_ground || !data_ok {
            self.reset(input, obstacles_available);
            return;
        }

        let ctx = context(input);
        let airborne_moving = input.gs_kt >= params::MIN_GS_KT;

        let (mut flta_raw, mut obstacle_raw, mut threat, mut unavailable) = (AlertLevel::None, AlertLevel::None, None, false);
        let mut probes = 0;
        if airborne_moving && !ctx.over_runway {
            let scale = self.look_ahead_scale(input, &ctx);
            if scale > 0. {
                let scan = self.scan(input, &ctx, scale, terrain, obstacles);
                flta_raw = scan.terrain;
                obstacle_raw = scan.obstacle;
                threat = scan.threat;
                probes = scan.samples;
                unavailable = scan.samples > 0 && scan.missing == scan.samples;
            }
        }
        self.probes_last_cycle = probes;

        let pda_raw = airborne_moving && !ctx.over_runway && self.premature_descent(input, &ctx);
        let tcf_raw = airborne_moving && !ctx.over_runway && self.clearance_floor(input, &ctx);

        let flta = self.flta_hold.update(flta_raw, dt_s);
        let obstacle = self.obstacle_hold.update(obstacle_raw, dt_s);
        let pda = self.pda_hold.update(if pda_raw { AlertLevel::Caution } else { AlertLevel::None }, dt_s) != AlertLevel::None;
        let tcf = self.tcf_hold.update(if tcf_raw { AlertLevel::Caution } else { AlertLevel::None }, dt_s) != AlertLevel::None;

        let pull_up = flta == AlertLevel::Warning || obstacle == AlertLevel::Warning;
        let caution = !pull_up
            && (flta == AlertLevel::Caution || obstacle == AlertLevel::Caution || pda || tcf);
        self.outputs = TawsOutputs {
            flta,
            obstacle,
            premature_descent: pda,
            clearance_floor: tcf,
            pull_up_light: pull_up,
            caution_light: caution,
            obstacle_data_available: obstacles_available,
            threat: if flta != AlertLevel::None || obstacle != AlertLevel::None { threat } else { None },
            nd: NdFlags {
                terrain_popup: flta != AlertLevel::None || obstacle != AlertLevel::None || pda || tcf,
                terrain_caution: flta == AlertLevel::Caution,
                terrain_warning: flta == AlertLevel::Warning,
                obstacle_caution: obstacle == AlertLevel::Caution,
                obstacle_warning: obstacle == AlertLevel::Warning,
                too_low_terrain: pda || tcf,
                pull_up,
                terr_off: false,
                terrain_unavailable: unavailable,
            },
        };
    }

    fn look_ahead_scale(&self, input: &TawsInputs, ctx: &Context) -> f64 {
        match ctx.approach {
            Some(a) if input.landing_config => {
                if a.distance_nm <= params::FINAL_INHIBIT_NM {
                    0.
                } else {
                    ((a.distance_nm - params::FINAL_INHIBIT_NM)
                        / (params::FINAL_DESENSITISE_NM - params::FINAL_INHIBIT_NM))
                        .clamp(0., 1.)
                }
            }
            _ => 1.,
        }
    }

    fn scan(
        &mut self,
        input: &TawsInputs,
        ctx: &Context,
        clearance_scale: f64,
        terrain: &dyn TerrainSampler,
        obstacles: Option<&dyn ObstacleSource>,
    ) -> Scan {
        let terminal = ctx.nearest_runway_nm <= params::TERMINAL_RADIUS_NM;
        let (t_caution, t_warning) = if terminal {
            (params::TERMINAL_CAUTION_TIME_S, params::TERMINAL_WARNING_TIME_S)
        } else {
            (params::CAUTION_TIME_S, params::WARNING_TIME_S)
        };
        let gs_nm_s = input.gs_kt / 3600.;
        let t_limit = t_caution.min(params::MAX_LOOKAHEAD_NM / gs_nm_s);
        let step_s = (params::STEP_NM / gs_nm_s).clamp(params::STEP_MIN_S, params::STEP_MAX_S);
        let rate = input.track_rate_dps.clamp(-params::MAX_TURN_RATE_DPS, params::MAX_TURN_RATE_DPS);
        let vs_eff = if input.vs_fpm < 0. { input.vs_fpm * clearance_scale } else { input.vs_fpm * params::CLIMB_CREDIT };
        let extra = (rate.abs() * params::CONE_TURN_EXTRA_DEG_PER_DPS).min(params::CONE_TURN_EXTRA_MAX_DEG);
        let tan_left = (params::CONE_HALF_ANGLE_DEG + if rate < 0. { extra } else { 0. }).to_radians().tan();
        let tan_right = (params::CONE_HALF_ANGLE_DEG + if rate > 0. { extra } else { 0. }).to_radians().tan();

        let mut scan = Scan { terrain: AlertLevel::None, obstacle: AlertLevel::None, threat: None, samples: 0, missing: 0 };
        let (mut lat, mut lon, mut heading) = (input.lat, input.lon, input.track_deg);
        let mut travelled = 0.;
        let mut t = 0.;
        while t + step_s <= t_limit + 1e-9 && scan.samples < params::MAX_PROBES_PER_CYCLE {
            let mid_heading = (heading + rate * step_s * 0.5).to_radians();
            let ds = gs_nm_s * step_s;
            let (nl, nlo) = offset_nm(lat, lon, ds * mid_heading.cos(), ds * mid_heading.sin());
            lat = nl;
            lon = nlo;
            heading += rate * step_s;
            t += step_s;
            travelled += ds;
            let h = heading.to_radians();
            let predicted = input.alt_ft + vs_eff / 60. * t;
            for side in [0, -1, 1] {
                let (plat, plon) = if side == 0 {
                    (lat, lon)
                } else {
                    let tan = if side < 0 { tan_left } else { tan_right };
                    let w = params::CONE_START_WIDTH_NM + travelled * tan;
                    let (pn, pe) = (-h.sin() * side as f64, h.cos() * side as f64);
                    offset_nm(lat, lon, w * pn, w * pe)
                };
                scan.samples += 1;
                let Some(elev) = terrain.elevation_ft(plat, plon) else {
                    scan.missing += 1;
                    continue;
                };
                let c_caution = terrain_clearance_ft(nearest_runway_nm(input.runways, plat, plon)) * clearance_scale;
                let c_warning = c_caution * params::WARNING_CLEARANCE_FRACTION;
                let level = if t <= t_warning && elev + c_warning > predicted {
                    AlertLevel::Warning
                } else if elev + c_caution > predicted {
                    AlertLevel::Caution
                } else {
                    AlertLevel::None
                };
                if level > scan.terrain {
                    scan.terrain = level;
                    scan.threat = Some(Threat { lat: plat, lon: plon, distance_nm: travelled, level });
                }
            }
        }

        if let Some(source) = obstacles {
            self.scan_obstacles(input, source, clearance_scale, t_caution.min(t_limit), t_warning, vs_eff, &mut scan);
        }
        scan
    }

    #[allow(clippy::too_many_arguments)]
    fn scan_obstacles(
        &mut self,
        input: &TawsInputs,
        source: &dyn ObstacleSource,
        clearance_scale: f64,
        t_caution: f64,
        t_warning: f64,
        vs_eff: f64,
        scan: &mut Scan,
    ) {
        let gs_nm_s = input.gs_kt / 3600.;
        let reach = gs_nm_s * t_caution + params::OBSTACLE_CORRIDOR_NM;
        self.obstacle_scratch.clear();
        source.obstacles_near(input.lat, input.lon, reach, &mut self.obstacle_scratch);
        let h = input.track_deg.to_radians();
        let (fwd_n, fwd_e) = (h.cos(), h.sin());
        for ob in &self.obstacle_scratch {
            let (n, e) = ne_nm(input.lat, input.lon, ob.lat, ob.lon);
            let along = n * fwd_n + e * fwd_e;
            let cross = e * fwd_n - n * fwd_e;
            if along <= 0. {
                continue;
            }
            let width = params::OBSTACLE_CORRIDOR_NM + along * params::CONE_HALF_ANGLE_DEG.to_radians().tan();
            let t = along / gs_nm_s;
            if cross.abs() > width || t > t_caution {
                continue;
            }
            let predicted = input.alt_ft + vs_eff / 60. * t;
            let level = if t <= t_warning && ob.top_ft + params::OBSTACLE_WARNING_CLEARANCE_FT * clearance_scale > predicted {
                AlertLevel::Warning
            } else if ob.top_ft + params::OBSTACLE_CAUTION_CLEARANCE_FT * clearance_scale > predicted {
                AlertLevel::Caution
            } else {
                AlertLevel::None
            };
            if level > scan.obstacle {
                scan.obstacle = level;
                if scan.threat.map_or(true, |t0| level > t0.level) {
                    scan.threat = Some(Threat { lat: ob.lat, lon: ob.lon, distance_nm: along, level });
                }
            }
        }
    }

    fn premature_descent(&self, input: &TawsInputs, ctx: &Context) -> bool {
        let Some(a) = ctx.approach else { return false };
        if a.distance_nm < params::PDA_MIN_NM || a.distance_nm > params::PDA_MAX_NM {
            return false;
        }
        if input.glideslope_tracking || input.vs_fpm > params::PDA_MAX_CLIMB_FPM {
            return false;
        }
        if input.radio_alt_ft.map_or(false, |ra| ra > params::PDA_MAX_RA_FT) {
            return false;
        }
        let height = input.alt_ft - input.runways[a.runway].elevation_ft;
        let floor = a.distance_nm * NM_FT * params::PDA_FLOOR_PATH_DEG.to_radians().tan();
        height > params::PDA_MIN_HEIGHT_FT && height < floor
    }

    fn clearance_floor(&self, input: &TawsInputs, ctx: &Context) -> bool {
        let Some(ra) = input.radio_alt_ft else { return false };
        let d = ctx.nearest_runway_nm;
        if !d.is_finite() || d < params::TCF_MIN_NM || d > params::TCF_MAX_NM {
            return false;
        }
        ra < interpolate(&params::TCF_FLOOR_PROFILE, d)
    }
}

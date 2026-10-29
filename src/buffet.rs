use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::published::{self, Published, Value};
use crate::xp::{DataRef, Xplm};
use crate::Vars;

const ALPHA_MAX_BY_FLAP_INDEX: [f64; 6] = [11.0, 15.0, 15.0, 14.0, 13.4, 13.4];
const STALL_BUFFET_M: f64 = 0.006;
const MACH_BUFFET_M: f64 = 0.004;
const SPEEDBRAKE_BUFFET_M: f64 = 0.0025;
const GEAR_BUFFET_M: f64 = 0.002;
const FLAP_OVERSPEED_BUFFET_M: f64 = 0.003;
const RUNWAY_RUMBLE_M: f64 = 0.0015;
const REVERSE_RUMBLE_M: f64 = 0.0028;
const GROUND_SPOILER_RUMBLE_M: f64 = 0.0012;
const BRAKE_JUDDER_M_PER_G: f64 = 0.006;
const SCRUB_M: f64 = 0.0016;
const TURBULENCE_BUMP_M_PER_G: f64 = 0.012;
const MAX_SHAKE_M: f64 = 0.012;
const JOINT_SPACING_M: f64 = 22.0;
const JOINT_KICK_MS: f64 = 0.05;
const TOUCHDOWN_KICK_MS_PER_FPM: f64 = 0.0006;
const GEAR_THUMP_MS: f64 = 0.09;
const THUMP_HZ: f64 = 5.5;
const THUMP_DAMPING: f64 = 0.28;
const WOBBLE_HZ: f64 = 1.8;
const WOBBLE_DAMPING: f64 = 0.06;
const WOBBLE_DEG_PER_G: f64 = 1.2;
const WOBBLE_LIMIT_DEG: f64 = 4.5;
const SUBSTEP_S: f64 = 0.01;
const FELT_SCALE: f64 = 0.2;
const COCKPIT_SHAKE_SCALE: f64 = 0.25;
const VFE_KT: [f64; 6] = [400.0, 263.0, 222.0, 220.0, 196.0, 182.0];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Conditions {
    pub alpha_deg: f64,
    pub flap_index: usize,
    pub mach: f64,
    pub ias_kt: f64,
    pub speedbrake: f64,
    pub gear_deployed: f64,
    pub on_ground: bool,
    pub ground_speed_ms: f64,
    pub nz_g: f64,
    pub ny_g: f64,
    pub reverse: f64,
    pub decel_g: f64,
    pub yaw_rate_deg_s: f64,
}

pub fn ramp(x: f64, start: f64, full: f64) -> f64 {
    ((x - start) / (full - start)).clamp(0., 1.)
}

pub fn buffet_amplitude_m(c: &Conditions) -> f64 {
    let alpha_max = ALPHA_MAX_BY_FLAP_INDEX[c.flap_index.min(5)];
    let stall = if c.on_ground { 0. } else { ramp(c.alpha_deg, alpha_max - 3.0, alpha_max) * STALL_BUFFET_M };
    let mach = ramp(c.mach, 0.88, 0.92) * MACH_BUFFET_M;
    let speedbrake = c.speedbrake.clamp(0., 1.) * ramp(c.ias_kt, 150.0, 300.0) * SPEEDBRAKE_BUFFET_M;
    let gear = c.gear_deployed.clamp(0., 1.) * ramp(c.ias_kt, 160.0, 250.0) * GEAR_BUFFET_M;
    let flaps = if c.flap_index == 0 { 0. } else { ramp(c.ias_kt, VFE_KT[c.flap_index.min(5)] - 10.0, VFE_KT[c.flap_index.min(5)] + 15.0) * FLAP_OVERSPEED_BUFFET_M };
    let (runway, reverse, spoilers, judder, scrub) = if c.on_ground {
        (
            ramp(c.ground_speed_ms, 2.0, 80.0) * RUNWAY_RUMBLE_M,
            c.reverse.clamp(0., 1.) * REVERSE_RUMBLE_M,
            c.speedbrake.clamp(0., 1.) * ramp(c.ground_speed_ms, 15.0, 60.0) * GROUND_SPOILER_RUMBLE_M,
            ramp(c.decel_g, 0.12, 0.45) * ramp(c.ground_speed_ms, 3.0, 25.0) * BRAKE_JUDDER_M_PER_G * 0.45,
            ramp(c.yaw_rate_deg_s.abs() * c.ground_speed_ms, 8.0, 40.0) * SCRUB_M,
        )
    } else {
        (0., 0., 0., 0., 0.)
    };
    [stall, mach, speedbrake, gear, flaps, runway, reverse, spoilers, judder, scrub].iter().map(|x| x * x).sum::<f64>().sqrt()
}

fn noise(t: f64, seed: f64) -> f64 {
    let f = [7.3, 9.7, 12.1, 15.9];
    let a = [0.45, 0.3, 0.17, 0.08];
    (0..4).map(|k| a[k] * (std::f64::consts::TAU * f[k] * t + seed * (k as f64 + 1.3)).sin()).sum()
}

fn road(distance_m: f64, seed: f64) -> f64 {
    let f = [0.9, 1.7, 2.9, 4.3];
    let a = [0.4, 0.3, 0.2, 0.1];
    (0..4).map(|k| a[k] * (std::f64::consts::TAU * f[k] * distance_m + seed * (k as f64 + 0.7)).sin()).sum()
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spring {
    pub angle: f64,
    pub rate: f64,
}

impl Spring {
    pub fn step(&mut self, drive_deg: f64, dt: f64) {
        let w = std::f64::consts::TAU * WOBBLE_HZ;
        let mut left = dt.min(0.25);
        while left > 1e-9 {
            let h = left.min(SUBSTEP_S);
            let accel = w * w * (drive_deg - self.angle) - 2.0 * WOBBLE_DAMPING * w * self.rate;
            self.rate += accel * h;
            self.angle = (self.angle + self.rate * h).clamp(-WOBBLE_LIMIT_DEG, WOBBLE_LIMIT_DEG);
            left -= h;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Thump {
    pub pos: f64,
    pub vel: f64,
}

impl Thump {
    pub fn kick(&mut self, velocity_ms: f64) {
        self.vel += velocity_ms;
    }

    pub fn step(&mut self, dt: f64) {
        let w = std::f64::consts::TAU * THUMP_HZ;
        let mut left = dt.min(0.25);
        while left > 1e-9 {
            let h = left.min(SUBSTEP_S);
            let accel = -w * w * self.pos - 2.0 * THUMP_DAMPING * w * self.vel;
            self.vel += accel * h;
            self.pos = (self.pos + self.vel * h).clamp(-MAX_SHAKE_M, MAX_SHAKE_M);
            left -= h;
        }
    }
}

struct Ids {
    flap_index: VariableIdentifier,
    speedbrake: VariableIdentifier,
}

struct Refs {
    alpha: Option<DataRef>,
    mach: Option<DataRef>,
    ias: Option<DataRef>,
    gear: Option<DataRef>,
    on_ground: Option<DataRef>,
    ground_speed: Option<DataRef>,
    nz: Option<DataRef>,
    ny: Option<DataRef>,
    yaw_rate: Option<DataRef>,
    vertical_fpm: Option<DataRef>,
    reverser: Option<DataRef>,
    n1: Option<DataRef>,
}

pub struct Buffet {
    _published: Published,
    shake: [Value; 3],
    wobble: [[Value; 2]; 4],
    springs: [[Spring; 2]; 4],
    vertical: Thump,
    lateral: Thump,
    ids: Ids,
    refs: Refs,
    time: f64,
    rolled_m: f64,
    nz_filtered: f64,
    ny_filtered: f64,
    was_on_ground: bool,
    last_fpm: f64,
    last_gear: f64,
    last_gs: f64,
    decel_g: f64,
}

impl Buffet {
    pub fn new(vars: &mut Vars, xplm: &Xplm) -> Self {
        let mut p = Published::default();
        let shake = ["x", "y", "z"].map(|a| p.number(&format!("fbw/anim/buffet_{a}"), 0., false));
        let wobble = [1, 2, 3, 4].map(|n| ["pitch", "yaw"].map(|a| p.number(&format!("fbw/anim/engine_wobble/{n}/{a}"), 0., false)));
        let f = |n: &str| xplm.find(n);
        Self {
            _published: p,
            shake,
            wobble,
            springs: Default::default(),
            vertical: Thump::default(),
            lateral: Thump::default(),
            ids: Ids { flap_index: vars.get("FLAPS_HANDLE_INDEX".into()), speedbrake: vars.get("SPOILERS_HANDLE_POSITION".into()) },
            refs: Refs {
                alpha: f("sim/flightmodel2/misc/AoA_angle_degrees"),
                mach: f("sim/flightmodel/misc/machno"),
                ias: f("sim/flightmodel/position/indicated_airspeed"),
                gear: f("sim/flightmodel2/gear/deploy_ratio"),
                on_ground: f("sim/flightmodel/failures/onground_any"),
                ground_speed: f("sim/flightmodel/position/groundspeed"),
                nz: f("sim/flightmodel2/misc/gforce_normal"),
                ny: f("sim/flightmodel2/misc/gforce_side"),
                yaw_rate: f("sim/flightmodel/position/R"),
                vertical_fpm: f("sim/flightmodel/position/vh_ind_fpm"),
                reverser: f("sim/flightmodel2/engines/thrust_reverser_deploy_ratio"),
                n1: f("sim/cockpit2/engine/indicators/N1_percent"),
            },
            time: 0.,
            rolled_m: 0.,
            nz_filtered: 1.,
            ny_filtered: 0.,
            was_on_ground: true,
            last_fpm: 0.,
            last_gear: 1.,
            last_gs: 0.,
            decel_g: 0.,
        }
    }

    fn reverse_level(&self, xplm: &Xplm) -> f64 {
        let (Some(rev), Some(n1)) = (self.refs.reverser, self.refs.n1) else { return 0. };
        let mut r = [0f32; 8];
        let mut n = [0f32; 8];
        xplm.get_vf(rev, &mut r);
        xplm.get_vf(n1, &mut n);
        (0..4).map(|e| r[e] as f64 * ramp(n[e] as f64, 25.0, 75.0)).fold(0., f64::max)
    }

    pub fn update(&mut self, vars: &mut Vars, xplm: &Xplm, delta: f64) {
        if delta <= 0. {
            return;
        }
        let r = |d: Option<DataRef>| d.map_or(0., |d| xplm.get_f(d) as f64);
        let mut gear = [0f32; 1];
        if let Some(d) = self.refs.gear {
            xplm.get_vf(d, &mut gear);
        }
        let on_ground = self.refs.on_ground.is_some_and(|d| xplm.get_i(d) != 0);
        let gs = r(self.refs.ground_speed);
        let decel = ((self.last_gs - gs) / delta / 9.80665).clamp(-1., 1.);
        self.decel_g += (decel - self.decel_g) * (1. - (-delta / 0.3).exp());
        self.last_gs = gs;
        let c = Conditions {
            alpha_deg: r(self.refs.alpha),
            flap_index: vars.read(&self.ids.flap_index).round().clamp(0., 5.) as usize,
            mach: r(self.refs.mach),
            ias_kt: r(self.refs.ias),
            speedbrake: vars.read(&self.ids.speedbrake),
            gear_deployed: gear[0] as f64,
            on_ground,
            ground_speed_ms: r(self.refs.ground_speed),
            nz_g: self.refs.nz.map_or(1., |d| xplm.get_f(d) as f64),
            ny_g: r(self.refs.ny),
            reverse: if on_ground { self.reverse_level(xplm) } else { 0. },
            decel_g: self.decel_g,
            yaw_rate_deg_s: r(self.refs.yaw_rate),
        };
        self.time += delta;
        let fpm = r(self.refs.vertical_fpm);
        if on_ground && !self.was_on_ground {
            let sink = (-self.last_fpm).clamp(0., 1500.);
            self.vertical.kick(-sink * TOUCHDOWN_KICK_MS_PER_FPM);
            self.lateral.kick(c.ny_g.signum() * sink * TOUCHDOWN_KICK_MS_PER_FPM * 0.25);
        }
        self.was_on_ground = on_ground;
        self.last_fpm = fpm;
        let g = c.gear_deployed;
        if (self.last_gear < 0.98 && g >= 0.98) || (self.last_gear > 0.02 && g <= 0.02) {
            self.vertical.kick(-GEAR_THUMP_MS);
        }
        self.last_gear = g;
        if on_ground {
            let before = (self.rolled_m / JOINT_SPACING_M).floor();
            self.rolled_m += c.ground_speed_ms * delta;
            if (self.rolled_m / JOINT_SPACING_M).floor() > before {
                self.vertical.kick(-JOINT_KICK_MS * ramp(c.ground_speed_ms, 2.0, 50.0));
            }
        }
        self.vertical.step(delta);
        self.lateral.step(delta);
        let follow = 1. - (-delta / 0.6).exp();
        self.nz_filtered += (c.nz_g - self.nz_filtered) * follow;
        self.ny_filtered += (c.ny_g - self.ny_filtered) * follow;
        let bump_z = turbulence_bump(on_ground, c.nz_g, self.nz_filtered);
        let bump_y = turbulence_bump(on_ground, c.ny_g, self.ny_filtered);
        let a = buffet_amplitude_m(&c);
        let t = self.time;
        let ground = if on_ground { ramp(c.ground_speed_ms, 1.0, 60.0) * RUNWAY_RUMBLE_M * road(self.rolled_m, 3.3) } else { 0. };
        let shake = [
            (a * 0.5 * noise(t, 1.7) + bump_y * TURBULENCE_BUMP_M_PER_G * 0.5 + self.lateral.pos).clamp(-MAX_SHAKE_M, MAX_SHAKE_M),
            (a * noise(t, 4.1) - bump_z * TURBULENCE_BUMP_M_PER_G + self.vertical.pos + ground).clamp(-MAX_SHAKE_M, MAX_SHAKE_M),
            (a * 0.35 * noise(t, 8.9) + ground * 0.3).clamp(-MAX_SHAKE_M, MAX_SHAKE_M),
        ];
        for k in 0..3 {
            published::set(self.shake[k], shake[k] * FELT_SCALE * COCKPIT_SHAKE_SCALE);
        }
        for e in 0..4 {
            let vibration = crate::failures::magnitude(72_000 + e as u64).max(crate::failures::magnitude(72_016 + e as u64));
            let buzz = vibration * 0.6 * (std::f64::consts::TAU * 9.0 * t + e as f64).sin();
            let pitch_drive = -bump_z * WOBBLE_DEG_PER_G + buzz + a * 40.0 * noise(t, 2.0 + e as f64) + self.vertical.pos * 120.0;
            let yaw_drive = bump_y * WOBBLE_DEG_PER_G * 0.6 + buzz * 0.5;
            self.springs[e][0].step(pitch_drive, delta);
            self.springs[e][1].step(yaw_drive, delta);
            published::set(self.wobble[e][0], self.springs[e][0].angle * FELT_SCALE);
            published::set(self.wobble[e][1], self.springs[e][1].angle * FELT_SCALE);
        }
    }
}

/// The g a gust adds over the filtered load. On the runway X-Plane's gear
/// contact makes the load factor jitter frame to frame, which is no gust:
/// the ground roll gets its rumble and joint thumps instead.
pub fn turbulence_bump(on_ground: bool, g: f64, filtered: f64) -> f64 {
    if on_ground {
        0.0
    } else {
        g - filtered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gear_contact_jitter_on_the_runway_is_not_a_gust() {
        assert_eq!(turbulence_bump(true, 1.4, 1.0), 0.0);
        assert!((turbulence_bump(false, 1.4, 1.0) - 0.4).abs() < 1e-9);
    }

    #[test]
    fn calm_cruise_has_no_buffet() {
        let c = Conditions { alpha_deg: 2.5, mach: 0.85, ias_kt: 280.0, nz_g: 1.0, ..Default::default() };
        assert_eq!(buffet_amplitude_m(&c), 0.0);
    }

    #[test]
    fn buffet_grows_toward_the_stall_and_with_speedbrakes() {
        let near = Conditions { alpha_deg: 10.5, ias_kt: 160.0, ..Default::default() };
        let far = Conditions { alpha_deg: 6.0, ias_kt: 160.0, ..Default::default() };
        assert!(buffet_amplitude_m(&near) > buffet_amplitude_m(&far));
        let brakes = Conditions { speedbrake: 1.0, ias_kt: 300.0, ..Default::default() };
        assert!(buffet_amplitude_m(&brakes) > 0.002);
        let rolling = Conditions { on_ground: true, ground_speed_ms: 70.0, ..Default::default() };
        assert!(buffet_amplitude_m(&rolling) > 0.001);
        assert!(buffet_amplitude_m(&Conditions { alpha_deg: 30.0, mach: 0.95, speedbrake: 1.0, gear_deployed: 1.0, ias_kt: 400.0, flap_index: 5, ..Default::default() }) <= MAX_SHAKE_M + 1e-9);
    }

    #[test]
    fn rollout_rumbles_more_with_reversers_and_braking() {
        let base = Conditions { on_ground: true, ground_speed_ms: 50.0, ..Default::default() };
        let rev = Conditions { reverse: 1.0, ..base };
        let braking = Conditions { decel_g: 0.4, ..base };
        assert!(buffet_amplitude_m(&rev) > buffet_amplitude_m(&base));
        assert!(buffet_amplitude_m(&braking) > buffet_amplitude_m(&base));
        let airborne_rev = Conditions { on_ground: false, reverse: 1.0, ground_speed_ms: 50.0, ..Default::default() };
        assert_eq!(buffet_amplitude_m(&airborne_rev), 0.0);
    }

    #[test]
    fn a_thump_rings_down() {
        let mut t = Thump::default();
        t.kick(-0.5);
        t.step(0.03);
        assert!(t.pos < 0.0);
        for _ in 0..200 {
            t.step(0.02);
        }
        assert!(t.pos.abs() < 1e-4, "settled, got {}", t.pos);
    }

    #[test]
    fn an_engine_wobble_settles_after_a_gust() {
        let mut s = Spring::default();
        s.step(2.0, 0.2);
        assert!(s.angle > 0.0);
        for _ in 0..600 {
            s.step(0.0, 0.05);
        }
        assert!(s.angle.abs() < 0.05, "damped back to rest, got {}", s.angle);
        let mut big = Spring::default();
        for _ in 0..100 {
            big.step(50.0, 0.05);
        }
        assert!(big.angle <= WOBBLE_LIMIT_DEG);
    }
}

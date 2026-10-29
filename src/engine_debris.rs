use std::path::{Path, PathBuf};

use crate::engine_detach::Pose;
use crate::xp;

const TICKET_BASE: usize = 7_500_000;
const SHAPES: [&str; 3] = ["a380_debris_0.obj", "a380_debris_1.obj", "a380_debris_2.obj"];
const FRAGMENT_DRAG_PER_KG: f64 = 0.02;
const GRAVITY: f64 = 9.81;
const BURST_S: f64 = 2.5;
const KEEP_S: f64 = 900.0;
const FORGET_DISTANCE_M: f64 = 20_000.0;
const HP_STAGE_AFT_M: f64 = 1.2;
const CORE_CASE_RADIUS_M: f64 = 1.0;
const WING_HIT_UPWARD: f64 = 0.45;
const FUEL_HIT_CHANCE: f64 = 0.3;
const HYDRAULIC_HIT_CHANCE: f64 = 0.2;
const GREEN_RESERVOIR_LEAK: u64 = 29_000;
const YELLOW_RESERVOIR_LEAK: u64 = 29_001;

#[derive(Clone, Copy, Debug)]
pub struct Rotor {
    pub base: u64,
    pub uncontained: f64,
    pub fragments: usize,
    pub speed: (f64, f64),
}

pub const ROTORS: [Rotor; 3] = [
    Rotor { base: 72_012, uncontained: 0.25, fragments: 16, speed: (90.0, 220.0) },
    Rotor { base: 72_016, uncontained: 0.35, fragments: 12, speed: (120.0, 260.0) },
    Rotor { base: 72_020, uncontained: 0.15, fragments: 8, speed: (60.0, 150.0) },
];

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng((seed ^ 0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fragment {
    pub pos: [f64; 3],
    pub vel: [f64; 3],
    pub angles: [f64; 3],
    pub rates: [f64; 3],
    pub resting: bool,
    pub rest_time: f64,
    pub shape: usize,
}

impl Fragment {
    pub fn step(&mut self, dt: f64, ground: Option<f64>) {
        if self.resting {
            self.rest_time += dt;
            return;
        }
        let speed = (self.vel[0].powi(2) + self.vel[1].powi(2) + self.vel[2].powi(2)).sqrt();
        for k in 0..3 {
            self.vel[k] -= FRAGMENT_DRAG_PER_KG * speed * self.vel[k] * dt;
        }
        self.vel[1] -= GRAVITY * dt;
        for k in 0..3 {
            self.pos[k] += self.vel[k] * dt;
            self.angles[k] = (self.angles[k] + self.rates[k] * dt).rem_euclid(360.);
        }
        if let Some(g) = ground {
            if self.pos[1] <= g + 0.05 {
                self.pos[1] = g + 0.05;
                self.resting = true;
                self.angles[1] = 0.;
                self.angles[2] = if self.angles[2] < 180. { 0. } else { 180. };
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hits {
    pub fuel: bool,
    pub hydraulic: bool,
}

pub fn wing_hits(directions: &[[f64; 3]], rng: &mut Rng) -> Hits {
    let mut h = Hits::default();
    for d in directions.iter().filter(|d| d[1] > WING_HIT_UPWARD) {
        h.fuel |= rng.next() < FUEL_HIT_CHANCE;
        h.hydraulic |= rng.next() < HYDRAULIC_HIT_CHANCE;
    }
    h
}

pub fn burst(rotor: &Rotor, rng: &mut Rng) -> Vec<([f64; 3], f64)> {
    (0..rotor.fragments)
        .map(|_| {
            let a = rng.range(0., std::f64::consts::TAU);
            let z = rng.range(-0.15, 0.15);
            let n = (1. + z * z).sqrt();
            ([a.cos() / n, a.sin() / n, z / n], rng.range(rotor.speed.0, rotor.speed.1))
        })
        .collect()
}

struct Model {
    ticket: usize,
    object: Option<usize>,
    failed: bool,
}

impl Model {
    fn load(path: &Path, ticket: usize) -> Model {
        Model { ticket, object: None, failed: !xp::load_object_async(&path.to_string_lossy(), ticket) }
    }

    fn poll(&mut self) -> Option<usize> {
        if self.object.is_none() && !self.failed {
            match xp::take_loaded_object(self.ticket) {
                Some(Some(o)) => self.object = Some(o),
                Some(None) => self.failed = true,
                None => {}
            }
        }
        self.object
    }
}

struct Piece {
    fragment: Fragment,
    instance: Option<usize>,
}

struct Burst {
    engine: usize,
    age: f64,
    instance: Option<usize>,
}

pub struct EngineDebris {
    dir: Option<PathBuf>,
    shapes: Option<[Model; 3]>,
    fire: Option<Model>,
    pieces: Vec<Piece>,
    bursts: Vec<Burst>,
    armed: [[bool; 4]; 3],
}

impl EngineDebris {
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self { dir, shapes: None, fire: None, pieces: Vec::new(), bursts: Vec::new(), armed: [[false; 4]; 3] }
    }

    fn ensure_models(&mut self, fire_name: Option<&str>) {
        let Some(dir) = self.dir.clone() else { return };
        if self.shapes.is_none() {
            self.shapes = Some([0, 1, 2].map(|k| Model::load(&dir.join(SHAPES[k]), TICKET_BASE + k)));
        }
        if self.fire.is_none() {
            if let Some(name) = fire_name {
                self.fire = Some(Model::load(&dir.join(name), TICKET_BASE + 3));
            }
        }
    }

    pub fn update(&mut self, delta: f64, pose: &Pose, cg_reference: [f64; 3], aircraft_vel: [f64; 3], centres: &[[f64; 3]; 4], fire_name: Option<&str>) {
        for (r, rotor) in ROTORS.iter().enumerate() {
            for engine in 0..4 {
                let active = crate::failures::is_active(rotor.base + engine as u64);
                if active && !self.armed[r][engine] {
                    self.event(rotor, engine, pose, cg_reference, aircraft_vel, centres[engine], fire_name);
                }
                self.armed[r][engine] = active;
            }
        }
        let shapes: [Option<usize>; 3] = match self.shapes.as_mut() {
            Some(m) => [m[0].poll(), m[1].poll(), m[2].poll()],
            None => [None; 3],
        };
        let fire = self.fire.as_mut().and_then(Model::poll);
        for p in self.pieces.iter_mut() {
            let f = &mut p.fragment;
            let ground = if f.resting { None } else { xp::probe_terrain_y(f.pos[0], f.pos[1], f.pos[2]) };
            f.step(delta, ground);
            if p.instance.is_none() {
                p.instance = shapes[f.shape].and_then(|o| xp::create_instance(o, &[]));
            }
            if let Some(i) = p.instance {
                xp::instance_set_position(i, f.pos, f.angles[0], f.angles[1], f.angles[2], &[]);
            }
        }
        let here = pose.position;
        self.pieces.retain_mut(|p| {
            let f = &p.fragment;
            let far = (0..3).map(|k| (f.pos[k] - here[k]).powi(2)).sum::<f64>().sqrt() > FORGET_DISTANCE_M;
            let keep = !far && f.rest_time < KEEP_S;
            if !keep {
                if let Some(i) = p.instance.take() {
                    xp::destroy_instance(i);
                }
            }
            keep
        });
        for b in self.bursts.iter_mut() {
            b.age += delta;
            if b.instance.is_none() {
                b.instance = fire.and_then(|o| xp::create_instance(o, &[]));
            }
            if let Some(i) = b.instance {
                let at = pose.to_world(cg_reference, centres[b.engine]);
                xp::instance_set_position(i, at, 0., 0., 0., &[]);
            }
        }
        self.bursts.retain_mut(|b| {
            let keep = b.age < BURST_S;
            if !keep {
                if let Some(i) = b.instance.take() {
                    xp::destroy_instance(i);
                }
            }
            keep
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn event(&mut self, rotor: &Rotor, engine: usize, pose: &Pose, cg_reference: [f64; 3], aircraft_vel: [f64; 3], centre: [f64; 3], fire_name: Option<&str>) {
        let seed = (pose.position[0] * 7.0) as i64 as u64 ^ ((pose.position[2] * 13.0) as i64 as u64).rotate_left(21) ^ rotor.base.wrapping_mul(31) ^ engine as u64;
        let mut rng = Rng::new(seed);
        if rng.next() >= rotor.uncontained {
            crate::log(&format!("engine debris: engine {} failure {} contained", engine + 1, rotor.base));
            return;
        }
        self.ensure_models(fire_name);
        let shards = burst(rotor, &mut rng);
        for (d, speed) in &shards {
            let local = [centre[0] + d[0] * CORE_CASE_RADIUS_M, centre[1] + d[1] * CORE_CASE_RADIUS_M, centre[2] + HP_STAGE_AFT_M];
            let w = pose.rotate(*d);
            let fragment = Fragment {
                pos: pose.to_world(cg_reference, local),
                vel: [aircraft_vel[0] + w[0] * speed, aircraft_vel[1] + w[1] * speed, aircraft_vel[2] + w[2] * speed],
                angles: [rng.range(0., 360.), rng.range(0., 360.), rng.range(0., 360.)],
                rates: [rng.range(-900., 900.), rng.range(-900., 900.), rng.range(-900., 900.)],
                shape: (rng.next() * 3.0) as usize % 3,
                ..Default::default()
            };
            self.pieces.push(Piece { fragment, instance: None });
        }
        self.bursts.push(Burst { engine, age: 0., instance: None });
        let dirs: Vec<[f64; 3]> = shards.iter().map(|(d, _)| *d).collect();
        let hits = wing_hits(&dirs, &mut rng);
        if hits.fuel {
            crate::failures::set_active(crate::failures::extra::FEED_TANK_PUNCTURE + engine as u64, true);
        }
        if hits.hydraulic {
            crate::failures::set_active(if engine < 2 { GREEN_RESERVOIR_LEAK } else { YELLOW_RESERVOIR_LEAK }, true);
        }
        crate::log(&format!(
            "engine debris: engine {} failure {} uncontained, {} fragments, fuel hit {}, hydraulic hit {}",
            engine + 1,
            rotor.base,
            shards.len(),
            hits.fuel,
            hits.hydraulic
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragments_leave_in_the_rotor_plane_at_rotor_speeds() {
        let mut rng = Rng::new(42);
        let r = ROTORS[1];
        let shards = burst(&r, &mut rng);
        assert_eq!(shards.len(), r.fragments);
        for (d, v) in shards {
            assert!((d[0].powi(2) + d[1].powi(2) + d[2].powi(2) - 1.).abs() < 1e-9);
            assert!(d[2].abs() < 0.16, "close to the plane of the disc");
            assert!((r.speed.0..=r.speed.1).contains(&v));
        }
    }

    #[test]
    fn a_fragment_flies_slows_falls_and_lands() {
        let mut f = Fragment { pos: [0., 100., 0.], vel: [150., 50., 0.], ..Default::default() };
        let mut t = 0.;
        while !f.resting && t < 120. {
            f.step(0.02, Some(0.));
            t += 0.02;
        }
        assert!(f.resting, "it lands");
        assert!(f.pos[0] > 50. && f.pos[0] < 2_000., "drag stops it within a couple of km: {}", f.pos[0]);
        assert!((f.pos[1] - 0.05).abs() < 1e-9);
    }

    #[test]
    fn only_upward_fragments_can_hit_the_wing() {
        let mut rng = Rng::new(1);
        let down = vec![[0., -1., 0.]; 50];
        assert_eq!(wing_hits(&down, &mut rng), Hits::default());
        let up = vec![[0., 1., 0.]; 50];
        let h = wing_hits(&up, &mut rng);
        assert!(h.fuel && h.hydraulic);
    }

    #[test]
    fn the_containment_roll_is_a_fair_coin_of_the_right_weight() {
        let mut rng = Rng::new(9);
        let n = 20_000;
        let hits = (0..n).filter(|_| rng.next() < 0.25).count() as f64 / n as f64;
        assert!((hits - 0.25).abs() < 0.02, "{hits}");
    }
}

use std::path::PathBuf;

use crate::deep::integration::xp_consequences::{ForceSink, PlugForceAxis, A380_WING_REFERENCE_AREA_M2};
use crate::detach_collision::{parse_spheres, Sphere};
use crate::xp::{self, DataRef, Xplm};

pub const ENGINE_AND_PYLON_KG: f64 = 9_500.0;
const PYLON_STUB_DRAG_AREA_M2: f64 = 1.8;
const TORN_LEADING_EDGE_DELTA_CL: f64 = 0.004;
const NACELLE_RADIUS_M: f64 = 1.5;
const AIR_DRAG_PER_KG: f64 = 0.5 * 1.2 * 1.0 * 10.0 / ENGINE_AND_PYLON_KG;
const COWL_DRAG_PER_KG: f64 = 0.5 * 1.2 * 1.2 * 6.0 / 700.0;
const COWL_DRAG_AREA_M2: f64 = 2.5;
const PANEL_DRAG_PER_KG: f64 = 0.03;
const CRUSH_IMPACT_MS: f64 = 12.0;
const SCORCH_AFTER_S: f64 = 45.0;
const GROUND_FRICTION: f64 = 0.35;
const BURN_S: f64 = 1_200.0;
const FORGET_DISTANCE_M: f64 = 20_000.0;
const TICKET_BASE: usize = 7_000_000;
const GRAVITY: f64 = 9.81;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub position: [f64; 3],
    pub psi: f64,
    pub theta: f64,
    pub phi: f64,
}

impl Pose {
    pub fn rotate(&self, b: [f64; 3]) -> [f64; 3] {
        let [x, y, z] = b;
        let c = -self.phi.to_radians();
        let (x, y) = (x * c.cos() - y * c.sin(), x * c.sin() + y * c.cos());
        let t = self.theta.to_radians();
        let (y, z) = (y * t.cos() - z * t.sin(), y * t.sin() + z * t.cos());
        let a = -self.psi.to_radians();
        let (x, z) = (x * a.cos() + z * a.sin(), -x * a.sin() + z * a.cos());
        [x, y, z]
    }

    pub fn unrotate(&self, b: [f64; 3]) -> [f64; 3] {
        let [x, z] = [b[0], b[2]];
        let a = self.psi.to_radians();
        let (x, z) = (x * a.cos() + z * a.sin(), -x * a.sin() + z * a.cos());
        let y = b[1];
        let t = -self.theta.to_radians();
        let (y, z) = (y * t.cos() - z * t.sin(), y * t.sin() + z * t.cos());
        let c = self.phi.to_radians();
        let (x, y) = (x * c.cos() - y * c.sin(), x * c.sin() + y * c.cos());
        [x, y, z]
    }

    pub fn to_world(&self, cg_reference: [f64; 3], acf: [f64; 3]) -> [f64; 3] {
        let r = self.rotate([acf[0] - cg_reference[0], acf[1] - cg_reference[1], acf[2] - cg_reference[2]]);
        [self.position[0] + r[0], self.position[1] + r[1], self.position[2] + r[2]]
    }
}

#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub centre: [[f64; 3]; 4],
    pub bottom: [f64; 4],
    pub half_width: [f64; 4],
    pub objects: [Vec<String>; 4],
    pub crushed: [Vec<String>; 4],
    pub burnt: [Vec<String>; 4],
    pub cowl_centre: [[f64; 3]; 4],
    pub cowl_bottom: [f64; 4],
    pub cowl_half_width: [f64; 4],
    pub cowl_objects: [Vec<String>; 4],
    pub cowl_parts: [Vec<CowlPart>; 4],
    pub spheres: [Vec<Sphere>; 4],
    pub spheres_crushed: [Vec<Sphere>; 4],
    pub fire: Option<String>,
    pub gp: Option<Box<Layout>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CowlKind {
    Ring,
    Inlet,
    DoorIn,
    DoorOut,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CowlPart {
    pub kind: CowlKind,
    pub centre: [f64; 3],
    pub bottom: f64,
    pub half_width: f64,
    pub objects: Vec<String>,
    pub spheres: Vec<Sphere>,
}

impl CowlKind {
    fn parse(name: &str) -> CowlKind {
        match name {
            "inlet" => CowlKind::Inlet,
            "door_in" => CowlKind::DoorIn,
            "door_out" => CowlKind::DoorOut,
            _ => CowlKind::Ring,
        }
    }

    /// Velocity relative to the aircraft (its own axes, z aft) and tumble
    /// rates a released piece leaves with: the inlet comes off forward, the
    /// fan cowl doors swing out to either side, so none passes through the
    /// engine core even when it drops straight onto the ramp.
    pub fn release(self, outward: f64) -> ([f64; 3], [f64; 3]) {
        match self {
            CowlKind::Ring => ([0., 3.0, 0.], [60. * outward, -90., 140. * outward]),
            CowlKind::Inlet => ([0.3 * outward, -1.0, -6.5], [6. * outward, 8., 10. * outward]),
            CowlKind::DoorOut => ([4.0 * outward, -0.5, -1.5], [4. * outward, 0., 30. * outward]),
            CowlKind::DoorIn => ([-4.0 * outward, -0.5, -1.5], [-4. * outward, 0., -30. * outward]),
        }
    }
}

fn names(v: Option<&serde_json::Value>) -> Vec<String> {
    v.and_then(|o| o.as_array()).map(|a| a.iter().filter_map(|o| o.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

fn point(v: Option<&serde_json::Value>) -> Option<[f64; 3]> {
    let c = v?.as_array()?;
    Some([c.first()?.as_f64()?, c.get(1)?.as_f64()?, c.get(2)?.as_f64()?])
}

impl Layout {
    pub fn parse(text: &str) -> Option<Layout> {
        Layout::from_value(&serde_json::from_str(text).ok()?)
    }

    fn from_value(root: &serde_json::Value) -> Option<Layout> {
        let mut layout = Layout { fire: root.get("fire").and_then(|f| f.as_str()).map(str::to_string), ..Default::default() };
        for (k, e) in root.get("engines")?.as_object()? {
            let i: usize = k.parse().ok()?;
            if i >= 4 {
                continue;
            }
            layout.centre[i] = point(e.get("centre"))?;
            layout.bottom[i] = e.get("bottom").and_then(|b| b.as_f64()).map_or(NACELLE_RADIUS_M, |b| b.abs());
            layout.half_width[i] = e.get("half_extent").and_then(|h| h.get(0)).and_then(|h| h.as_f64()).unwrap_or(NACELLE_RADIUS_M);
            layout.objects[i] = e.get("objects")?.as_array()?.iter().filter_map(|o| o.as_str().map(str::to_string)).collect();
            layout.crushed[i] = names(e.get("crushed"));
            layout.burnt[i] = names(e.get("burnt"));
            layout.spheres[i] = parse_spheres(e.get("spheres"));
            layout.spheres_crushed[i] = parse_spheres(e.get("spheres_crushed"));
        }
        for (k, c) in root.get("cowls").and_then(|c| c.as_object()).into_iter().flatten() {
            let Some(i) = k.parse::<usize>().ok().filter(|i| *i < 4) else { continue };
            let Some(centre) = point(c.get("centre")) else { continue };
            layout.cowl_centre[i] = centre;
            layout.cowl_bottom[i] = c.get("bottom").and_then(|b| b.as_f64()).map_or(1.0, |b| b.abs());
            layout.cowl_half_width[i] = c.get("half_extent").and_then(|h| h.get(0)).and_then(|h| h.as_f64()).unwrap_or(1.0);
            layout.cowl_objects[i] = names(c.get("objects"));
            let pieces: Vec<CowlPart> = c
                .get("pieces")
                .and_then(|p| p.as_array())
                .into_iter()
                .flatten()
                .filter_map(|p| {
                    Some(CowlPart {
                        kind: CowlKind::parse(p.get("kind")?.as_str()?),
                        centre: point(p.get("centre"))?,
                        bottom: p.get("bottom").and_then(|b| b.as_f64()).map_or(1.0, |b| b.abs()),
                        half_width: p.get("half_extent").and_then(|h| h.get(0)).and_then(|h| h.as_f64()).unwrap_or(1.0),
                        objects: names(p.get("objects")),
                        spheres: parse_spheres(p.get("spheres")),
                    })
                })
                .filter(|p| !p.objects.is_empty())
                .collect();
            layout.cowl_parts[i] = if pieces.is_empty() && !layout.cowl_objects[i].is_empty() {
                vec![CowlPart { kind: CowlKind::Ring, centre, bottom: layout.cowl_bottom[i], half_width: layout.cowl_half_width[i], objects: layout.cowl_objects[i].clone(), spheres: Vec::new() }]
            } else {
                pieces
            };
        }
        let fire = layout.fire.clone();
        layout.gp = root.get("gp").and_then(Layout::from_value).map(|g| Box::new(Layout { fire: g.fire.or(fire), ..g }));
        Some(layout)
    }

    pub fn for_engine_type(&self, gp: bool) -> &Layout {
        match &self.gp {
            Some(g) if gp => g,
            _ => self,
        }
    }

    pub fn fallback() -> Layout {
        Layout {
            centre: [[-25.77, -0.51, -1.14], [-14.87, -1.04, -9.01], [14.87, -1.04, -9.01], [25.76, -0.51, -1.14]],
            bottom: [2.2; 4],
            half_width: [1.98; 4],
            ..Default::default()
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    pub pos: [f64; 3],
    pub vel: [f64; 3],
    pub heading: f64,
    pub pitch: f64,
    pub roll: f64,
    pub rates: [f64; 3],
    pub grounded: bool,
    pub resting: bool,
    pub burning: bool,
    pub burn_time: f64,
    pub luck: f64,
    pub bottom: f64,
    pub half_width: f64,
    pub drag: f64,
    pub peak_impact: f64,
    pub sphere_contact: f64,
}

pub fn ignition_chance(impact_ms: f64, ground_speed_ms: f64) -> f64 {
    (((impact_ms - 3.0) / 8.0).max(0.) + ((ground_speed_ms - 10.0) / 40.0).max(0.)).min(0.95)
}

impl Body {
    pub fn contact(&self) -> f64 {
        if self.sphere_contact > 0. {
            return self.sphere_contact;
        }
        let r = self.roll.to_radians();
        let bottom = if self.bottom > 0. { self.bottom } else { NACELLE_RADIUS_M };
        let side = if self.half_width > 0. { self.half_width } else { NACELLE_RADIUS_M };
        bottom * r.cos().abs() + side * r.sin().abs()
    }

    pub fn released(pose: &Pose, cg_reference: [f64; 3], centre: [f64; 3], aircraft_vel: [f64; 3], luck: f64) -> Body {
        let outward = pose.rotate([centre[0].signum(), 0., 0.]);
        Body {
            pos: pose.to_world(cg_reference, centre),
            vel: [aircraft_vel[0] + outward[0] * 1.5, aircraft_vel[1] - 1.0, aircraft_vel[2] + outward[2] * 1.5],
            heading: pose.psi,
            pitch: pose.theta,
            roll: pose.phi,
            rates: [8.0 * centre[0].signum(), 10.0, 25.0 * centre[0].signum()],
            luck,
            ..Default::default()
        }
    }

    pub fn step(&mut self, dt: f64, ground_y: Option<f64>) {
        if dt <= 0. {
            return;
        }
        if self.burning {
            self.burn_time += dt;
            if self.burn_time > BURN_S {
                self.burning = false;
            }
        }
        if self.resting {
            return;
        }
        let speed = (self.vel[0].powi(2) + self.vel[1].powi(2) + self.vel[2].powi(2)).sqrt();
        let drag = if self.drag > 0. { self.drag } else { AIR_DRAG_PER_KG };
        for k in 0..3 {
            self.vel[k] -= drag * speed * self.vel[k] * dt;
        }
        self.vel[1] -= GRAVITY * dt;
        for k in 0..3 {
            self.pos[k] += self.vel[k] * dt;
        }
        self.heading = (self.heading + self.rates[0] * dt).rem_euclid(360.);
        self.pitch += self.rates[1] * dt;
        self.roll = (self.roll + self.rates[2] * dt + 180.).rem_euclid(360.) - 180.;
        let Some(ground) = ground_y else { return };
        let contact = self.contact();
        if self.pos[1] - contact > ground {
            self.grounded = false;
            return;
        }
        self.pos[1] = ground + contact;
        let impact = -self.vel[1];
        if !self.grounded {
            self.peak_impact = self.peak_impact.max(impact);
        }
        let horizontal = (self.vel[0].powi(2) + self.vel[2].powi(2)).sqrt();
        if !self.grounded && !self.burning && self.luck < ignition_chance(impact, horizontal) {
            self.burning = true;
        }
        self.grounded = true;
        self.vel[1] = if impact > 1.0 { impact * 0.3 } else { 0. };
        if horizontal > 0. {
            let slowed = (horizontal - GROUND_FRICTION * GRAVITY * dt).max(0.);
            self.vel[0] *= slowed / horizontal;
            self.vel[2] *= slowed / horizontal;
        }
        let h = self.heading.to_radians();
        let across = self.vel[0] * h.cos() + self.vel[2] * h.sin();
        let side = if self.half_width > 0. { self.half_width } else { NACELLE_RADIUS_M };
        self.rates[2] = (across / side).to_degrees();
        self.rates[0] *= (-2.0 * dt).exp();
        self.rates[1] = 0.;
        self.pitch *= (-3.0 * dt).exp();
        if horizontal < 0.3 && self.vel[1] == 0. {
            self.resting = true;
            self.rates = [0.; 3];
        }
    }
}

pub fn cowl_forces(sink: &dyn ForceSink, centre: [f64; 3], dynamic_pressure_pa: f64) {
    let drag = dynamic_pressure_pa * COWL_DRAG_AREA_M2;
    sink.add_plug_force(PlugForceAxis::Faxil, drag);
    sink.add_plug_force(PlugForceAxis::Yaw, drag * centre[0]);
}

pub fn separation_forces(sink: &dyn ForceSink, centre: [f64; 3], cg_reference: [f64; 3], dynamic_pressure_pa: f64) {
    let lateral = centre[0];
    let longitudinal = centre[2] - cg_reference[2];
    let relief = ENGINE_AND_PYLON_KG * GRAVITY;
    sink.add_plug_force(PlugForceAxis::Fnrml, relief);
    sink.add_plug_force(PlugForceAxis::Roll, -relief * lateral);
    sink.add_plug_force(PlugForceAxis::Pitch, -relief * longitudinal);
    let drag = dynamic_pressure_pa * PYLON_STUB_DRAG_AREA_M2;
    sink.add_plug_force(PlugForceAxis::Faxil, drag);
    sink.add_plug_force(PlugForceAxis::Yaw, drag * lateral);
    let lift_lost = dynamic_pressure_pa * A380_WING_REFERENCE_AREA_M2 * TORN_LEADING_EDGE_DELTA_CL;
    sink.add_plug_force(PlugForceAxis::Fnrml, -lift_lost);
    sink.add_plug_force(PlugForceAxis::Roll, lift_lost * lateral);
}

pub(crate) struct Slot {
    ticket: usize,
    object: Option<usize>,
    instance: Option<usize>,
    failed: bool,
    /// Where this object's origin sits from the body's, in the body's frame.
    offset: [f64; 3],
}

impl Slot {
    fn load(path: &std::path::Path, ticket: usize) -> Slot {
        let failed = !xp::load_object_async(&path.to_string_lossy(), ticket);
        Slot { ticket, object: None, instance: None, failed, offset: [0.; 3] }
    }

    fn poll(&mut self) {
        if self.failed || self.object.is_some() {
            return;
        }
        if let Some(loaded) = xp::take_loaded_object(self.ticket) {
            match loaded {
                Some(o) => {
                    self.object = Some(o);
                    self.instance = xp::create_instance(o, &[]);
                }
                None => self.failed = true,
            }
        }
    }

    fn place(&self, at: [f64; 3], heading: f64, pitch: f64, roll: f64) {
        if let Some(i) = self.instance {
            xp::instance_set_position(i, at, heading, pitch, roll, &[]);
        }
    }

    fn release(&mut self) {
        if let Some(i) = self.instance.take() {
            xp::destroy_instance(i);
        }
        if let Some(o) = self.object.take() {
            xp::unload_object(o);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub enum Look {
    Intact,
    Crushed,
    Burnt,
}

pub fn look_after(body: &Body, now: Look) -> Look {
    let wanted = if body.burning && body.burn_time > SCORCH_AFTER_S {
        Look::Burnt
    } else if body.peak_impact > CRUSH_IMPACT_MS {
        Look::Crushed
    } else {
        Look::Intact
    };
    if wanted > now {
        wanted
    } else {
        now
    }
}

struct Falling {
    body: Body,
    pieces: Vec<Slot>,
    fire: Option<Slot>,
    look: Look,
    gp: bool,
    spheres: Vec<Sphere>,
    reach: f64,
    before: Option<Body>,
    allowed: Vec<f64>,
    airframe: bool,
}

impl Falling {
    fn release(&mut self) {
        self.pieces.iter_mut().for_each(Slot::release);
        if let Some(mut s) = self.fire.take() {
            s.release();
        }
    }

    fn new(body: Body, pieces: Vec<Slot>, gp: bool, spheres: Vec<Sphere>) -> Falling {
        let reach = crate::detach_collision::bounding_radius(&spheres);
        Falling { body, pieces, fire: None, look: Look::Intact, gp, spheres, reach, before: None, allowed: Vec::new(), airframe: true }
    }

    fn set_spheres(&mut self, spheres: Vec<Sphere>) {
        if !spheres.is_empty() {
            self.reach = crate::detach_collision::bounding_radius(&spheres);
            self.spheres = spheres;
            self.allowed.clear();
        }
    }

    /// Moves the body one step; false once it is too far away to keep.
    fn simulate(&mut self, delta: f64, here: [f64; 3]) -> bool {
        let far = (0..3).map(|k| (self.body.pos[k] - here[k]).powi(2)).sum::<f64>().sqrt() > FORGET_DISTANCE_M;
        if far {
            self.release();
            return false;
        }
        self.before = Some(self.body);
        if !self.spheres.is_empty() {
            self.body.sphere_contact = crate::detach_collision::ground_contact(&self.body, &self.spheres);
        }
        let ground = if self.body.resting { None } else { xp::probe_terrain_y(self.body.pos[0], self.body.pos[1], self.body.pos[2]) };
        self.body.step(delta, ground);
        if let Some(g) = ground {
            keep_above(&mut self.body, &self.spheres, g);
        }
        true
    }

    fn present(&mut self, fire: Option<&std::path::Path>, ticket: &mut usize, can_burn: bool, was_burning: bool) -> bool {
        let frame = Pose { position: self.body.pos, psi: self.body.heading, theta: self.body.pitch, phi: self.body.roll };
        for p in self.pieces.iter_mut() {
            p.poll();
            let r = frame.rotate(p.offset);
            p.place([0, 1, 2].map(|k| self.body.pos[k] + r[k]), self.body.heading, self.body.pitch, self.body.roll);
        }
        let burning = self.body.burning && can_burn;
        match (&mut self.fire, burning, fire) {
            (None, true, Some(path)) => {
                *ticket += 1;
                self.fire = Some(Slot::load(path, *ticket));
            }
            (Some(s), false, _) => {
                s.release();
                self.fire = None;
            }
            _ => {}
        }
        if let Some(s) = self.fire.as_mut() {
            s.poll();
            s.place([self.body.pos[0], self.body.pos[1] - self.body.contact(), self.body.pos[2]], 0., 0., 0.);
        }
        burning && !was_burning
    }
}

fn engine_body(set: &Layout, engine: usize, pose: &Pose, cg_reference: [f64; 3], vel: [f64; 3]) -> Body {
    let mut body = Body::released(pose, cg_reference, set.centre[engine], vel, luck(pose, engine as f64));
    body.bottom = set.bottom[engine];
    body.half_width = set.half_width[engine];
    body
}

fn part_body(piece: &PartPiece, name: &str, pose: &Pose, cg_reference: [f64; 3], vel: [f64; 3]) -> Body {
    let salt = name.bytes().map(f64::from).sum::<f64>();
    let mut body = Body::released(pose, cg_reference, piece.centre, vel, luck(pose, salt));
    let spin = if luck(pose, salt + 1.) > 0.5 { 1. } else { -1. };
    body.drag = PANEL_DRAG_PER_KG;
    body.rates = [90. * spin, -60. * spin, 120. * piece.centre[0].signum()];
    if piece.centre[0].abs() < 1.0 {
        let side = pose.rotate([spin, 0., 0.]);
        body.vel = [body.vel[0] + side[0] * FIN_TOPPLE_MS, body.vel[1], body.vel[2] + side[2] * FIN_TOPPLE_MS];
        body.rates[2] = 60. * spin;
    }
    body.bottom = piece.bottom;
    body.half_width = piece.half_width;
    body
}

fn cowl_body(part: &CowlPart, engine: usize, pose: &Pose, cg_reference: [f64; 3], vel: [f64; 3]) -> Body {
    let mut body = Body::released(pose, cg_reference, part.centre, vel, luck(pose, 10. + engine as f64 + part.centre[2]));
    let (rel, rates) = part.kind.release(part.centre[0].signum());
    let rel = pose.rotate(rel);
    body.vel = [vel[0] + rel[0], vel[1] + rel[1], vel[2] + rel[2]];
    body.rates = rates;
    body.drag = COWL_DRAG_PER_KG;
    body.bottom = part.bottom;
    body.half_width = part.half_width;
    body
}

/// Lifts a body whose step turned part of it below the ground back onto it.
fn keep_above(body: &mut Body, spheres: &[Sphere], ground: f64) {
    if spheres.is_empty() {
        return;
    }
    let contact = crate::detach_collision::ground_contact(body, spheres);
    if body.pos[1] - contact < ground {
        body.pos[1] = ground + contact;
    }
}

struct Refs {
    local: [Option<DataRef>; 3],
    velocity: [Option<DataRef>; 3],
    angles: [Option<DataRef>; 3],
    cg: [Option<DataRef>; 2],
    livery: Option<DataRef>,
    on_ground: Option<DataRef>,
    gear: Option<DataRef>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartPiece {
    pub centre: [f64; 3],
    pub bottom: f64,
    pub half_width: f64,
    pub objects: Vec<String>,
    pub sub: Vec<(String, Vec<String>)>,
    /// Whole parts outboard of this one that go with it, drawn from their own objects.
    pub carry: Vec<String>,
    pub spheres: Vec<Sphere>,
}

pub fn parse_parts(text: &str) -> std::collections::BTreeMap<String, PartPiece> {
    let root: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
    let mut out = std::collections::BTreeMap::new();
    for (name, p) in root.get("parts").and_then(|p| p.as_object()).into_iter().flatten() {
        let Some(centre) = point(p.get("centre")) else { continue };
        let sub = p.get("sub").and_then(|s| s.as_object()).map(|s| s.iter().map(|(k, v)| (k.clone(), names(Some(v)))).collect()).unwrap_or_default();
        out.insert(
            name.clone(),
            PartPiece {
                centre,
                bottom: p.get("bottom").and_then(|b| b.as_f64()).map_or(0.3, f64::abs),
                half_width: p.get("half_extent").and_then(|h| h.get(0)).and_then(|h| h.as_f64()).unwrap_or(1.0),
                objects: names(p.get("objects")),
                sub,
                carry: names(p.get("carry")),
                spheres: parse_spheres(p.get("spheres")),
            },
        );
    }
    out
}

pub fn release_plan(flags: &[(String, bool)], layout: &std::collections::BTreeMap<String, PartPiece>, fallen: &std::collections::HashSet<String>) -> Vec<(String, Vec<String>)> {
    let on = |n: &str| flags.iter().any(|(k, v)| k == n && *v);
    let mut plan: Vec<(String, Vec<String>)> = Vec::new();
    let mut taken: std::collections::HashSet<String> = fallen.clone();
    for (name, piece) in layout.iter().filter(|(_, p)| !p.sub.is_empty() || !p.carry.is_empty()) {
        if on(name) && !taken.contains(name) {
            let outboard = piece.carry.iter().flat_map(|c| std::iter::once(c.clone()).chain(layout.get(c).into_iter().flat_map(|cp| cp.sub.iter().map(|(s, _)| s.clone()))));
            let carried: Vec<String> = piece.sub.iter().map(|(s, _)| s.clone()).chain(outboard).filter(|s| !taken.contains(s)).collect();
            taken.insert(name.clone());
            taken.extend(carried.iter().cloned());
            plan.push((name.clone(), carried));
        }
    }
    for (name, _) in layout.iter().filter(|(_, p)| p.sub.is_empty()) {
        if on(name) && !taken.contains(name) {
            taken.insert(name.clone());
            plan.push((name.clone(), Vec::new()));
        }
    }
    plan
}

const NEAR_AIRCRAFT_M: f64 = 90.0;
const FIN_TOPPLE_MS: f64 = 4.0;
/// Contact a sphere may make without counting: the spheres already stand
/// proud of the surface they wrap, so this never shows as one piece inside
/// another.
const CONTACT_M: f64 = 0.05;

/// Undoes whatever part of each body's last step drove it deeper into the
/// airframe or another body: the full step if that is clear, else only its
/// vertical or only its horizontal part (so it slides along or settles on
/// what it met), else no move at all. A sphere already touching what it was
/// mounted to when the piece let go is free to work loose, and is held like
/// the rest once it has come clear. Motion is judged relative to the
/// aircraft while the body is near it, so a piece pressed against a flying
/// wing travels with it instead of stopping dead in the air.
fn resolve(bodies: &mut [&mut Falling], map: &crate::detach_collision::AirframeMap, frame: &crate::detach_collision::Frame, active: &[bool], aircraft_vel: [f64; 3], delta: f64) {
    use crate::detach_collision::{sphere_overlaps, world_spheres};
    let mut world: Vec<Vec<Sphere>> = bodies.iter().map(|f| world_spheres(&f.body, &f.spheres)).collect();
    let still = aircraft_vel.iter().map(|v| v * v).sum::<f64>().sqrt() < 1.0;
    for i in 0..bodies.len() {
        let Some(before) = bodies[i].before else { continue };
        if bodies[i].spheres.is_empty() || before.resting {
            continue;
        }
        let near = bodies[i].airframe && (0..3).map(|k| (bodies[i].body.pos[k] - frame.pose.position[k]).powi(2)).sum::<f64>().sqrt() < NEAR_AIRCRAFT_M;
        let reach = bodies[i].reach;
        let spheres = bodies[i].spheres.clone();
        let others: Vec<(usize, [f64; 3], f64)> = (0..bodies.len()).filter(|&j| j != i && !bodies[j].spheres.is_empty()).map(|j| (j, bodies[j].body.pos, bodies[j].reach)).collect();
        let overlaps = |b: &Body, world: &[Vec<Sphere>]| -> Vec<f64> {
            let mine = world_spheres(b, &spheres);
            let near_others: Vec<&[Sphere]> = others
                .iter()
                .filter(|(_, at, r)| (0..3).map(|k| (b.pos[k] - at[k]).powi(2)).sum::<f64>().sqrt() < reach + r)
                .map(|(j, _, _)| world[*j].as_slice())
                .collect();
            sphere_overlaps(&mine, near.then_some((map, frame, active)), &near_others)
        };
        let shift = if near { aircraft_vel.map(|v| v * delta) } else { [0.; 3] };
        let base = [before.pos[0] + shift[0], before.pos[1] + shift[1], before.pos[2] + shift[2]];
        let start = overlaps(&Body { pos: base, ..before }, &world);
        if bodies[i].allowed.len() != spheres.len() {
            bodies[i].allowed = start.iter().map(|o| if *o > 0. { f64::INFINITY } else { CONTACT_M }).collect();
        }
        let allowed = bodies[i].allowed.clone();
        let depth = |o: &[f64]| -> f64 { o.iter().zip(&allowed).map(|(v, a)| (v - a).max(0.)).sum() };
        let was = depth(&start);
        let stepped = bodies[i].body;
        let now = overlaps(&stepped, &world);
        let (mut chosen, kind, after) = if depth(&now) <= was + 1e-3 {
            (stepped, 0, now)
        } else {
            let unturned = Body { heading: before.heading, pitch: before.pitch, roll: before.roll, ..stepped };
            let candidates = [
                (Body { pos: [stepped.pos[0], stepped.pos[1], base[2]], ..stepped }, 8),
                (Body { pos: [base[0], stepped.pos[1], stepped.pos[2]], ..stepped }, 9),
                (Body { pos: [base[0], stepped.pos[1], base[2]], ..stepped }, 1),
                (Body { pos: [stepped.pos[0], base[1], stepped.pos[2]], ..stepped }, 2),
                (unturned, 5),
                (Body { pos: [base[0], stepped.pos[1], base[2]], ..unturned }, 6),
                (Body { pos: [stepped.pos[0], base[1], stepped.pos[2]], ..unturned }, 7),
                (Body { pos: base, ..stepped }, 3),
                (Body { pos: base, ..unturned }, 4),
            ];
            let mut pick = None;
            for (b, k) in candidates {
                let o = overlaps(&b, &world);
                if depth(&o) <= was + 1e-3 || k == 4 {
                    pick = Some((b, k, o));
                    break;
                }
            }
            pick.expect("the last candidate is always taken")
        };
        if kind != 0 {
            let d1 = stepped.pos[1] - base[1];
            let carry = if near { aircraft_vel } else { [0.; 3] };
            let mut rel = [chosen.vel[0] - carry[0], chosen.vel[1] - carry[1], chosen.vel[2] - carry[2]];
            if kind >= 4 {
                chosen.rates = chosen.rates.map(|r| r * -0.3);
            }
            match kind {
                8 => rel[2] *= 0.9,
                9 => rel[0] *= 0.9,
                1 | 6 => {
                    rel[0] *= 0.9;
                    rel[2] *= 0.9;
                }
                2 | 7 => {
                    if d1 < 0. && rel[1] < 0. {
                        rel[1] = 0.;
                        rel[0] *= 0.95;
                        rel[2] *= 0.95;
                        if still && rel[0].hypot(rel[2]) < 0.2 && depth(&after) < 1e-6 {
                            chosen.resting = true;
                            chosen.rates = [0.; 3];
                        }
                    } else {
                        rel[1] *= -0.2;
                    }
                }
                5 => {}
                _ => {
                    rel = rel.map(|v| v * -0.2);
                }
            }
            chosen.vel = [rel[0] + carry[0], rel[1] + carry[1], rel[2] + carry[2]];
        }
        for (a, o) in bodies[i].allowed.iter_mut().zip(&after) {
            if a.is_infinite() && *o <= 0. {
                *a = CONTACT_M;
            }
        }
        bodies[i].body = chosen;
        world[i] = world_spheres(&chosen, &spheres);
    }
}

pub struct EngineDetach {
    layout: Layout,
    part_layout: std::collections::BTreeMap<String, PartPiece>,
    parts: Vec<(String, Vec<String>, Falling)>,
    fallen: std::collections::HashSet<String>,
    dir: Option<PathBuf>,
    refs: Refs,
    engines: [Option<Falling>; 4],
    cowls: [Vec<Falling>; 4],
    left_behind: [bool; 4],
    cowls_left_behind: [bool; 4],
    next_ticket: usize,
    cg: [f64; 3],
    airborne: bool,
    debris: crate::engine_debris::EngineDebris,
    map: Option<crate::detach_collision::AirframeMap>,
}

pub fn livery_texture_line(line: &str, livery_objects: &std::path::Path) -> Option<(String, bool)> {
    let mut parts = line.split_whitespace();
    let key = parts.next()?;
    if !key.starts_with("TEXTURE") {
        return None;
    }
    let rest: Vec<&str> = parts.collect();
    let file = *rest.last()?;
    if file.contains('/') {
        return None;
    }
    let in_livery = livery_objects.join(file).is_file();
    let mut out: Vec<String> = std::iter::once(key.to_string()).chain(rest[..rest.len() - 1].iter().map(|s| s.to_string())).collect();
    out.push(if in_livery { file.to_string() } else { format!("../../../objects/{file}") });
    Some((out.join(" "), in_livery))
}

fn livery_variant(dir: &std::path::Path, name: &str, livery: Option<&str>) -> PathBuf {
    let plain = dir.join(name);
    let Some(livery) = livery else { return plain };
    let livery_objects = dir.parent().map(|a| a.join("liveries").join(livery).join("objects"));
    let Some(livery_objects) = livery_objects.filter(|p| p.is_dir()) else { return plain };
    let Ok(text) = std::fs::read_to_string(&plain) else { return plain };
    let mut changed = false;
    let lines: Vec<String> = text
        .lines()
        .map(|l| match livery_texture_line(l, &livery_objects) {
            Some(n) => {
                changed |= n.1;
                n.0
            }
            None => l.to_string(),
        })
        .collect();
    if !changed {
        return plain;
    }
    let variant = livery_objects.join(name);
    match std::fs::write(&variant, lines.join("\n")) {
        Ok(()) => variant,
        Err(_) => plain,
    }
}

fn objects_dir() -> Option<PathBuf> {
    crate::sound_pack::plugin_dir().and_then(|d| Some(d.parent()?.parent()?.join("objects")))
}

fn luck(pose: &Pose, salt: f64) -> f64 {
    ((pose.position[0] * 12.9898 + pose.position[2] * 78.233 + salt * 37.719).sin() * 43_758.545).fract().abs()
}

impl EngineDetach {
    pub fn new(xplm: &Xplm) -> Self {
        let dir = objects_dir();
        let layout = dir
            .as_ref()
            .and_then(|d| std::fs::read_to_string(d.join("a380_engdetach.json")).ok())
            .and_then(|t| Layout::parse(&t))
            .unwrap_or_else(Layout::fallback);
        crate::log(&format!(
            "engine detach: {} detached-engine objects, {} cowl objects",
            layout.objects.iter().map(Vec::len).sum::<usize>(),
            layout.cowl_parts.iter().flatten().map(|p| p.objects.len()).sum::<usize>()
        ));
        let f = |n: &str| xplm.find(n);
        let refs = Refs {
            local: ["sim/flightmodel/position/local_x", "sim/flightmodel/position/local_y", "sim/flightmodel/position/local_z"].map(f),
            velocity: ["sim/flightmodel/position/local_vx", "sim/flightmodel/position/local_vy", "sim/flightmodel/position/local_vz"].map(f),
            angles: ["sim/flightmodel/position/psi", "sim/flightmodel/position/theta", "sim/flightmodel/position/phi"].map(f),
            cg: ["sim/aircraft/weight/acf_cgY_original", "sim/aircraft/weight/acf_cgZ_original"].map(f),
            livery: f("sim/aircraft/view/acf_livery_path"),
            on_ground: f("sim/flightmodel/failures/onground_any"),
            gear: f("sim/flightmodel2/gear/deploy_ratio"),
        };
        let debris = crate::engine_debris::EngineDebris::new(dir.clone());
        let part_layout = dir.as_ref().and_then(|d| std::fs::read_to_string(d.join("a380_partdetach.json")).ok()).map(|t| parse_parts(&t)).unwrap_or_default();
        crate::log(&format!("engine detach: {} separable parts", part_layout.len()));
        let map = dir.as_ref().and_then(|d| std::fs::read_to_string(d.join("a380_airframe_map.json")).ok()).and_then(|t| crate::detach_collision::AirframeMap::parse(&t));
        crate::log(&format!("engine detach: airframe collision map {}", if map.is_some() { "loaded" } else { "missing" }));
        Self {
            layout,
            part_layout,
            parts: Vec::new(),
            fallen: Default::default(),
            dir,
            refs,
            engines: Default::default(),
            cowls: Default::default(),
            left_behind: [false; 4],
            cowls_left_behind: [false; 4],
            next_ticket: TICKET_BASE,
            cg: [0., 0.85, -2.44],
            airborne: false,
            debris,
            map,
        }
    }

    pub fn airborne(&self) -> bool {
        self.airborne
    }

    /// Which airframe map layers are still there: a layer drops out once
    /// any part it belongs to has separated, and the gear's once it is up.
    fn solid_layers(&self, xplm: &Xplm) -> Vec<bool> {
        let Some(map) = self.map.as_ref() else { return Vec::new() };
        let separation = crate::separation::Separation::current();
        let parts = separation.parts();
        let mut ratio = [1.0f32; 1];
        let gear_down = self.refs.gear.map_or(true, |d| xplm.get_vf(d, &mut ratio) == 0 || ratio[0] > 0.5);
        let failed = |id: u64| crate::failures::is_active(id);
        let gp = crate::aircraft_options::engine_rating().gp;
        let gone = |gate: &str| -> bool {
            if let Some(variant) = gate.strip_prefix("variant:") {
                return (variant == "gp") != gp;
            }
            use crate::failures::extra::{COWL_SEPARATION, ENGINE_SEPARATION, FIN_SEPARATION, HSTAB_SEPARATION};
            if gate == "gear" {
                return !gear_down;
            }
            if let Some(n) = gate.strip_prefix("sim/operation/failures/rel_engsep") {
                return n.parse::<u64>().is_ok_and(|n| failed(ENGINE_SEPARATION + n));
            }
            if let Some(n) = gate.strip_prefix("fbw/fail/sep/cowl_") {
                return n.parse::<u64>().is_ok_and(|n| n >= 1 && failed(COWL_SEPARATION + n - 1));
            }
            if let Some(name) = gate.strip_prefix("fbw/fail/sep/") {
                return parts.iter().any(|(k, on)| k == name && *on);
            }
            if let Some(piece) = gate.strip_prefix("sim/operation/failures/rel_wing") {
                let side = usize::from(piece.ends_with('R'));
                return piece[..piece.len().saturating_sub(1)].parse::<usize>().is_ok_and(|n| (1..=4).contains(&n) && separation.wing[side].is_some_and(|w| w < n));
            }
            match gate {
                "sim/operation/failures/rel_hstbL" => failed(HSTAB_SEPARATION),
                "sim/operation/failures/rel_hstbR" => failed(HSTAB_SEPARATION + 1),
                "sim/operation/failures/rel_vstb1" => failed(FIN_SEPARATION),
                _ => false,
            }
        };
        map.gates().map(|gates| !gates.iter().any(|g| gone(g))).collect()
    }

    fn update_parts(&mut self, xplm: &Xplm, pose: &Pose, cg_reference: [f64; 3], vel: [f64; 3]) {
        if self.part_layout.is_empty() {
            return;
        }
        let flags = crate::separation::Separation::current().parts();
        let on = |n: &str| flags.iter().any(|(k, v)| k == n && *v);
        let mut kept = Vec::new();
        for (name, carried, mut f) in std::mem::take(&mut self.parts) {
            if on(&name) {
                kept.push((name, carried, f));
                continue;
            }
            f.release();
            self.fallen.remove(&name);
            for c in carried.iter().filter(|c| !on(c)) {
                self.fallen.remove(c);
            }
        }
        self.parts = kept;
        for (name, carried) in release_plan(&flags, &self.part_layout, &self.fallen) {
            let Some(piece) = self.part_layout.get(&name).cloned() else { continue };
            let body = part_body(&piece, &name, pose, cg_reference, vel);
            let mut names: Vec<(String, [f64; 3])> = piece.objects.iter().map(|o| (o.clone(), [0.; 3])).collect();
            let mut owners = vec![(&piece, [0.; 3])];
            for c in &piece.carry {
                if let Some(cp) = self.part_layout.get(c) {
                    let offset = [0, 1, 2].map(|k| cp.centre[k] - piece.centre[k]);
                    if carried.contains(c) {
                        names.extend(cp.objects.iter().map(|o| (o.clone(), offset)));
                    }
                    owners.push((cp, offset));
                }
            }
            for (owner, offset) in owners {
                for (sub, objects) in &owner.sub {
                    if carried.contains(sub) {
                        names.extend(objects.iter().map(|o| (o.clone(), offset)));
                    }
                }
            }
            let mut pieces = self.load_pieces(xplm, &names.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>());
            for (slot, (_, offset)) in pieces.iter_mut().zip(&names) {
                slot.offset = *offset;
            }
            crate::log(&format!("engine detach: {name} separated{}, {} pieces", if carried.is_empty() { String::new() } else { format!(" with {}", carried.join(", ")) }, pieces.len()));
            self.fallen.insert(name.clone());
            self.fallen.extend(carried.iter().cloned());
            let mut falling = Falling::new(body, pieces, false, piece.spheres.clone());
            falling.airframe = false;
            self.parts.push((name, carried, falling));
        }
    }

    pub fn centre(&self, engine: usize) -> [f64; 3] {
        self.layout.centre[engine]
    }

    pub fn cg_reference(&self) -> [f64; 3] {
        self.cg
    }

    fn read_cg_reference(&self, xplm: &Xplm) -> [f64; 3] {
        let ft = |d: Option<DataRef>, fallback: f64| d.map_or(fallback, |d| xplm.get_f(d) as f64) * 0.3048;
        [0., ft(self.refs.cg[0], 2.8), ft(self.refs.cg[1], -8.0)]
    }

    fn load_pieces(&mut self, xplm: &Xplm, names: &[String]) -> Vec<Slot> {
        let livery = self.refs.livery.and_then(|d| crate::cabin_mood::livery_name(&xplm.get_text(d, 512)));
        let paths: Vec<PathBuf> = match &self.dir {
            Some(dir) => names.iter().map(|n| livery_variant(dir, n, livery.as_deref())).collect(),
            None => Vec::new(),
        };
        let first = self.next_ticket + 1;
        self.next_ticket += paths.len();
        paths.iter().enumerate().map(|(k, p)| Slot::load(p, first + k)).collect()
    }

    pub fn update(&mut self, xplm: &Xplm, delta: f64) {
        let pose = Pose {
            position: self.refs.local.map(|d| d.map_or(0., |d| xplm.get_d(d))),
            psi: self.refs.angles[0].map_or(0., |d| xplm.get_f(d) as f64),
            theta: self.refs.angles[1].map_or(0., |d| xplm.get_f(d) as f64),
            phi: self.refs.angles[2].map_or(0., |d| xplm.get_f(d) as f64),
        };
        let cg_reference = self.read_cg_reference(xplm);
        self.cg = cg_reference;
        self.airborne = self.refs.on_ground.is_some_and(|d| xplm.get_i(d) == 0);
        let vel = self.refs.velocity.map(|d| d.map_or(0., |d| xplm.get_f(d) as f64));
        let gp = crate::aircraft_options::engine_rating().gp && self.layout.gp.is_some();
        for engine in 0..4 {
            let separated = crate::failures::is_active(crate::failures::extra::ENGINE_SEPARATION + engine as u64);
            if !separated {
                self.left_behind[engine] = false;
                if let Some(mut f) = self.engines[engine].take() {
                    f.release();
                    crate::log(&format!("engine detach: engine {} cleared", engine + 1));
                }
            } else if self.engines[engine].is_none() && !self.left_behind[engine] {
                let set = self.layout.for_engine_type(gp);
                let body = engine_body(set, engine, &pose, cg_reference, vel);
                let names = set.objects[engine].clone();
                let spheres = set.spheres[engine].clone();
                let pieces = self.load_pieces(xplm, &names);
                crate::log(&format!("engine detach: engine {} separated, {} pieces{}", engine + 1, pieces.len(), if gp { " (GP7200)" } else { "" }));
                self.engines[engine] = Some(Falling::new(body, pieces, gp, spheres));
            }
            let cowl_lost = crate::failures::is_active(crate::failures::extra::COWL_SEPARATION + engine as u64);
            if !cowl_lost {
                self.cowls_left_behind[engine] = false;
                for mut f in self.cowls[engine].drain(..) {
                    f.release();
                }
            } else if self.cowls[engine].is_empty() && !self.cowls_left_behind[engine] && !separated && !self.layout.for_engine_type(gp).cowl_parts[engine].is_empty() {
                let parts = self.layout.for_engine_type(gp).cowl_parts[engine].clone();
                for part in &parts {
                    let body = cowl_body(part, engine, &pose, cg_reference, vel);
                    let pieces = self.load_pieces(xplm, &part.objects);
                    self.cowls[engine].push(Falling::new(body, pieces, gp, part.spheres.clone()));
                }
                crate::log(&format!("engine detach: engine {} inlet and fan cowls separated, {:?}", engine + 1, parts.iter().map(|p| p.kind).collect::<Vec<_>>()));
            }
        }
        self.update_parts(xplm, &pose, cg_reference, vel);
        let fire = self.dir.as_ref().zip(self.layout.fire.as_ref()).map(|(d, n)| d.join(n));
        let mut ticket = self.next_ticket;
        let mut swaps = Vec::new();
        for (engine, slot) in self.engines.iter_mut().enumerate() {
            if slot.as_mut().is_some_and(|f| !f.simulate(delta, pose.position)) {
                *slot = None;
                self.left_behind[engine] = true;
            }
        }
        for (engine, list) in self.cowls.iter_mut().enumerate() {
            let had = !list.is_empty();
            list.retain_mut(|f| f.simulate(delta, pose.position));
            if had && list.is_empty() {
                self.cowls_left_behind[engine] = true;
            }
        }
        self.parts.retain_mut(|(_, _, f)| f.simulate(delta, pose.position));
        let active = self.solid_layers(xplm);
        let frame = crate::detach_collision::Frame { pose, cg_reference };
        let mut bodies: Vec<&mut Falling> = self.engines.iter_mut().flatten().chain(self.cowls.iter_mut().flatten()).chain(self.parts.iter_mut().map(|(_, _, f)| f)).collect();
        if let Some(map) = self.map.as_ref() {
            resolve(&mut bodies, map, &frame, &active, vel, delta);
        }
        for (engine, slot) in self.engines.iter_mut().enumerate() {
            let Some(f) = slot.as_mut() else { continue };
            let was_burning = f.before.is_some_and(|b| b.burning);
            if f.present(fire.as_deref(), &mut ticket, true, was_burning) {
                crate::log(&format!("engine detach: engine {} caught fire on impact", engine + 1));
            }
            let look = look_after(&f.body, f.look);
            if look != f.look {
                swaps.push((engine, look, f.gp));
            }
        }
        for f in self.cowls.iter_mut().flatten() {
            f.present(None, &mut ticket, false, false);
        }
        for (_, _, f) in self.parts.iter_mut() {
            f.present(None, &mut ticket, false, false);
        }
        self.next_ticket = ticket;
        for (engine, look, gp) in swaps {
            let set = self.layout.for_engine_type(gp);
            let names = match look {
                Look::Burnt => set.burnt[engine].clone(),
                Look::Crushed => set.crushed[engine].clone(),
                Look::Intact => continue,
            };
            let crushed = if look == Look::Crushed { set.spheres_crushed[engine].clone() } else { Vec::new() };
            let pieces = if names.is_empty() { Vec::new() } else { self.load_pieces(xplm, &names) };
            let Some(f) = self.engines[engine].as_mut() else { continue };
            if !pieces.is_empty() {
                f.pieces.iter_mut().for_each(Slot::release);
                f.pieces = pieces;
                crate::log(&format!("engine detach: engine {} now {:?}", engine + 1, look));
            }
            f.set_spheres(crushed);
            f.look = look;
        }
        self.debris.update(delta, &pose, cg_reference, vel, &self.layout.centre, self.layout.fire.as_deref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detach_collision::{ground_contact, mutual_penetration, world_spheres, AirframeMap, Frame};

    fn block(w: f64, h: f64, l: f64) -> Vec<Sphere> {
        let mut v = Vec::new();
        let step = 0.4;
        let n = |e: f64| (e / step).round().max(1.) as i32;
        for i in 0..=n(w) {
            for j in 0..=n(h) {
                for k in 0..=n(l) {
                    v.push([-w / 2. + i as f64 * w / n(w) as f64, -h / 2. + j as f64 * h / n(h) as f64, -l / 2. + k as f64 * l / n(l) as f64, 0.25]);
                }
            }
        }
        v
    }

    fn still_frame() -> Frame {
        Frame { pose: Pose::default(), cg_reference: [0.; 3] }
    }

    fn empty_map() -> AirframeMap {
        AirframeMap::parse(r#"{"cell":0.5,"x0":-20,"z0":-20,"nx":80,"nz":80,"layers":{}}"#).unwrap()
    }

    fn fall(f: &mut Falling, others: &mut Vec<Falling>, map: &AirframeMap, ground: f64) {
        f.before = Some(f.body);
        f.body.sphere_contact = ground_contact(&f.body, &f.spheres);
        f.body.step(0.02, Some(ground));
        keep_above(&mut f.body, &f.spheres, ground);
        let mut all: Vec<&mut Falling> = others.iter_mut().collect();
        all.push(f);
        let active = vec![true; map.gates().count()];
        resolve(&mut all, map, &still_frame(), &active, [0.; 3], 0.02);
    }

    #[test]
    fn a_piece_dropped_onto_another_settles_on_top_without_sinking_in() {
        let map = empty_map();
        let mut below = vec![Falling::new(Body { pos: [0., 0.75, 0.], resting: true, ..Default::default() }, Vec::new(), false, block(3., 1., 3.))];
        let mut piece = Falling::new(Body { pos: [0.3, 4.0, 0.2], ..Default::default() }, Vec::new(), false, block(2., 0.6, 2.));
        for _ in 0..400 {
            fall(&mut piece, &mut below, &map, 0.);
            let crossing = mutual_penetration(&world_spheres(&below[0].body, &below[0].spheres), &world_spheres(&piece.body, &piece.spheres));
            assert!(crossing < 1e-6, "sank into the piece below by {crossing}");
        }
        assert!(piece.body.pos[1] > 1.5, "came to rest on top, at {}", piece.body.pos[1]);
    }

    #[test]
    fn a_cowl_door_thrown_at_the_engine_core_cannot_pass_into_it() {
        let cells: Vec<String> = (36..44).flat_map(|i| (36..44).map(move |j| format!("[{i},{j},0.5,2.5]"))).collect();
        let map = AirframeMap::parse(&format!(r#"{{"cell":0.5,"x0":-20,"z0":-20,"nx":80,"nz":80,"layers":{{"sim/operation/failures/rel_engsep0":[{}]}}}}"#, cells.join(","))).unwrap();
        let mut door = Falling::new(Body { pos: [3.0, 1.5, 0.], vel: [-4.0, 0., 0.], ..Default::default() }, Vec::new(), false, block(0.4, 2.0, 2.5));
        let frame = still_frame();
        let core_active = vec![true];
        for _ in 0..300 {
            fall(&mut door, &mut Vec::new(), &map, 0.);
            let inside = crate::detach_collision::airframe_penetration(&world_spheres(&door.body, &door.spheres), &map, &frame, &core_active);
            if inside > 1e-6 {
                eprintln!("step pos {:?} vel {:?} mounted {} inside {inside}", door.body.pos, door.body.vel, door.allowed.iter().filter(|a| **a > CONTACT_M).count());
            }
            assert!(inside < 1e-6, "door entered the core by {inside}");
        }
        assert!(door.body.pos[0] > 2.0, "stopped against the core at x {}", door.body.pos[0]);
        assert!(door.body.pos[1] < 1.5, "and dropped to the ground beside it");
    }

    #[test]
    fn a_tumbling_piece_never_dips_below_the_ground() {
        let map = empty_map();
        let shape = block(4., 0.5, 1.);
        let mut piece = Falling::new(Body { pos: [0., 3.0, 0.], vel: [5., 0., 2.], rates: [40., 90., 200.], ..Default::default() }, Vec::new(), false, shape.clone());
        for _ in 0..600 {
            fall(&mut piece, &mut Vec::new(), &map, 0.);
            let lowest = world_spheres(&piece.body, &shape).iter().map(|s| s[1] - s[3]).fold(f64::INFINITY, f64::min);
            assert!(lowest > -1e-6, "dipped {lowest} m into the ground");
        }
    }

    /// The ramp case from the sim, on the installed aircraft's own shapes:
    /// all four cowls let go at the gate, then all four engines. Nothing may
    /// end up inside the airframe or inside another piece.
    /// `OBJ_DIR=<aircraft objects> cargo test ... -- --ignored`
    #[test]
    #[ignore]
    fn on_the_installed_aircraft_every_separated_surface_comes_to_rest_on_the_ground() {
        let dir = std::path::PathBuf::from(std::env::var("OBJ_DIR").expect("OBJ_DIR"));
        let parts = parse_parts(&std::fs::read_to_string(dir.join("a380_partdetach.json")).unwrap());
        let map = AirframeMap::parse(&std::fs::read_to_string(dir.join("a380_airframe_map.json")).unwrap()).unwrap();
        let cg = [0., 0.85, -2.44];
        let pose = Pose::default();
        let frame = Frame { pose, cg_reference: cg };
        let ground = map.ground.expect("the map records the ramp height") - cg[1];
        let mut worst: Vec<(f64, String)> = Vec::new();
        for (name, piece) in &parts {
            let mine = format!("fbw/fail/sep/{name}");
            let subs: Vec<String> = piece.sub.iter().map(|(s, _)| format!("fbw/fail/sep/{s}")).collect();
            let active: Vec<bool> = map.gates().map(|g| !g.iter().any(|x| *x == mine || subs.contains(x))).collect();
            let mut f = Falling::new(part_body(piece, name, &pose, cg, [0.; 3]), Vec::new(), false, piece.spheres.clone());
            f.airframe = false;
            for _ in 0..(20.0 / 0.02) as usize {
                f.before = Some(f.body);
                f.body.sphere_contact = ground_contact(&f.body, &f.spheres);
                f.body.step(0.02, Some(ground));
                keep_above(&mut f.body, &f.spheres, ground);
                let mut all: Vec<&mut Falling> = vec![&mut f];
                resolve(&mut all, &map, &frame, &active, [0.; 3], 0.02);
            }
            let lowest = world_spheres(&f.body, &f.spheres).iter().map(|s| s[1] - s[3]).fold(f64::INFINITY, f64::min);
            let height = lowest - ground;
            eprintln!("{name:20} resting {} lowest point {height:6.2} m above the ground", f.body.resting);
            worst.push((height, name.clone()));
        }
        worst.sort_by(|a, b| b.0.total_cmp(&a.0));
        let floating: Vec<&(f64, String)> = worst.iter().filter(|(h, _)| *h > 0.15).collect();
        assert!(floating.is_empty(), "parts left above the ground: {floating:?}");
    }

    #[test]
    #[ignore]
    fn on_the_installed_aircraft_nothing_ends_up_inside_anything() {
        let dir = std::path::PathBuf::from(std::env::var("OBJ_DIR").expect("OBJ_DIR"));
        let layout = Layout::parse(&std::fs::read_to_string(dir.join("a380_engdetach.json")).unwrap()).unwrap();
        let map = AirframeMap::parse(&std::fs::read_to_string(dir.join("a380_airframe_map.json")).unwrap()).unwrap();
        for gp in [false, true] {
            let set = layout.for_engine_type(gp);
            let cg = [0., 0.85, -2.44];
            let pose = Pose::default();
            let frame = Frame { pose, cg_reference: cg };
            let ground = map.ground.expect("the map records the ramp height") - cg[1];
            let mut bodies: Vec<Falling> = Vec::new();
            let variant = if gp { "variant:gp" } else { "variant:trent" };
            let gone = |stage: u8, gates: &[String]| -> bool {
                gates.iter().any(|g| (g.starts_with("variant:") && g != variant) || (stage >= 1 && g.starts_with("fbw/fail/sep/cowl_")) || (stage >= 2 && g.starts_with("sim/operation/failures/rel_engsep")))
            };
            let run = |bodies: &mut Vec<Falling>, active: &[bool], seconds: f64| {
                for _ in 0..(seconds / 0.02) as usize {
                    for f in bodies.iter_mut() {
                        f.before = Some(f.body);
                        f.body.sphere_contact = ground_contact(&f.body, &f.spheres);
                        f.body.step(0.02, Some(ground));
                        keep_above(&mut f.body, &f.spheres, ground);
                    }
                    let mut all: Vec<&mut Falling> = bodies.iter_mut().collect();
                    resolve(&mut all, &map, &frame, active, [0.; 3], 0.02);
                }
            };
            let report = |bodies: &[Falling], active: &[bool]| -> (f64, f64) {
                let world: Vec<Vec<Sphere>> = bodies.iter().map(|f| world_spheres(&f.body, &f.spheres)).collect();
                let mut airframe: f64 = 0.;
                let mut mutual: f64 = 0.;
                for i in 0..world.len() {
                    let others: Vec<&[Sphere]> = (0..world.len()).filter(|&j| j != i).map(|j| world[j].as_slice()).collect();
                    let into_airframe = crate::detach_collision::sphere_overlaps(&world[i], Some((&map, &frame, active)), &[]);
                    airframe = into_airframe.into_iter().fold(airframe, f64::max);
                    for other in &others {
                        for s in &world[i] {
                            for t in other.iter() {
                                let d = ((s[0] - t[0]).powi(2) + (s[1] - t[1]).powi(2) + (s[2] - t[2]).powi(2)).sqrt();
                                mutual = mutual.max(s[3] + t[3] - d);
                            }
                        }
                    }
                }
                (airframe, mutual)
            };
            let cowl_active: Vec<bool> = map.gates().map(|g| !gone(1, g)).collect();
            for engine in 0..4 {
                for part in &set.cowl_parts[engine] {
                    assert!(!part.spheres.is_empty(), "cowl {engine} {:?} has no collision shape", part.kind);
                    bodies.push(Falling::new(cowl_body(part, engine, &pose, cg, [0.; 3]), Vec::new(), gp, part.spheres.clone()));
                }
            }
            for f in &bodies {
                eprintln!("spawn {:?} spheres {} pos {:?} vel {:?}", f.body.pos, f.spheres.len(), f.body.pos, f.body.vel);
            }
            run(&mut bodies, &cowl_active, 12.);
            for (n, f) in bodies.iter().enumerate() {
                let w = world_spheres(&f.body, &f.spheres);
                let o = crate::detach_collision::sphere_overlaps(&w, Some((&map, &frame, &cowl_active)), &[]);
                let (k, deepest) = o.iter().enumerate().fold((0, 0.), |acc, (k, v)| if *v > acc.1 { (k, *v) } else { acc });
                if deepest > CONTACT_M {
                    let s = w[k];
                    let acf = frame.to_acf([s[0], s[1], s[2]]);
                    eprintln!("cowl body {n} deepest {deepest:.3} allowed {:.3} at acf {:?} r {} touching {:?}", f.allowed[k], acf.map(|v| (v * 100.).round() / 100.), s[3], map.touching(acf, s[3], &cowl_active));
                }
            }
            for f in &bodies {
                let w = world_spheres(&f.body, &f.spheres);
                eprintln!("rest {:?} resting {} airframe {:.3} mounted {}", f.body.pos.map(|v| (v * 100.).round() / 100.), f.body.resting, crate::detach_collision::airframe_penetration(&w, &map, &frame, &cowl_active), f.allowed.iter().filter(|a| **a > CONTACT_M).count());
                let mut hits: std::collections::BTreeMap<String, usize> = Default::default();
                for (k, s) in w.iter().enumerate() {
                    if f.allowed.get(k).is_some_and(|a| *a > CONTACT_M) {
                        continue;
                    }
                    for h in map.touching(frame.to_acf([s[0], s[1], s[2]]), s[3], &cowl_active) {
                        *hits.entry(h.split(' ').next().unwrap().to_string()).or_default() += 1;
                    }
                }
                eprintln!("   free spheres touching {hits:?}");
                let free: Vec<Sphere> = w.iter().enumerate().filter(|(k, _)| !f.allowed.get(*k).is_some_and(|a| *a > CONTACT_M)).map(|(_, s)| *s).collect();
                let crossing: Vec<String> = bodies.iter().enumerate().map(|(j, g)| format!("{j}:{:.2}", mutual_penetration(&free, &world_spheres(&g.body, &g.spheres)))).filter(|t| !t.ends_with(":0.00")).collect();
                eprintln!("   free spheres crossing {crossing:?}  rates {:?} hpr {:.0} {:.0} {:.0}", f.body.rates, f.body.heading, f.body.pitch, f.body.roll);
            }
            let (a, m) = report(&bodies, &cowl_active);
            assert!(a <= CONTACT_M + 1e-3 && m <= 2. * CONTACT_M + 1e-3, "gp {gp}: a cowl piece at rest is {a:.3} m into the airframe / {m:.3} m into another piece");
            let engine_active: Vec<bool> = map.gates().map(|g| !gone(2, g)).collect();
            for engine in 0..4 {
                bodies.push(Falling::new(engine_body(set, engine, &pose, cg, [0.; 3]), Vec::new(), gp, set.spheres[engine].clone()));
            }
            run(&mut bodies, &engine_active, 20.);
            for (n, f) in bodies.iter().enumerate() {
                let w = world_spheres(&f.body, &f.spheres);
                let o = crate::detach_collision::sphere_overlaps(&w, Some((&map, &frame, &engine_active)), &[]);
                let (k, deepest) = o.iter().enumerate().fold((0, 0.), |acc, (k, v)| if *v > acc.1 { (k, *v) } else { acc });
                if deepest > CONTACT_M {
                    let s = w[k];
                    let acf = frame.to_acf([s[0], s[1], s[2]]);
                    eprintln!("body {n} at {:?} resting {} mounted {} deepest {deepest:.3} sphere acf {:?} r {} touching {:?}", f.body.pos.map(|v| (v * 100.).round() / 100.), f.body.resting, f.allowed.iter().filter(|a| **a > CONTACT_M).count(), acf.map(|v| (v * 100.).round() / 100.), s[3], map.touching(acf, s[3], &engine_active));
                }
            }
            let (a, m) = report(&bodies, &engine_active);
            assert!(a <= CONTACT_M + 1e-3 && m <= 2. * CONTACT_M + 1e-3, "gp {gp}: a piece at rest is {a:.3} m into the airframe / {m:.3} m into another piece");
            for f in &bodies {
                let lowest = world_spheres(&f.body, &f.spheres).iter().map(|s| s[1] - s[3]).fold(f64::INFINITY, f64::min);
                assert!(lowest > ground - 1e-6 && lowest < ground + 3.0, "a piece is under the ramp or floating ({lowest} vs ground {ground})");
            }
        }
    }

    #[test]
    fn the_inlet_leaves_forward_and_the_doors_to_either_side() {
        let left = -1.0;
        assert!(CowlKind::Inlet.release(left).0[2] < -5.0);
        assert!(CowlKind::DoorOut.release(left).0[0] < -3.0);
        assert!(CowlKind::DoorIn.release(left).0[0] > 3.0);
        for k in [CowlKind::Inlet, CowlKind::DoorIn, CowlKind::DoorOut] {
            assert!(k.release(left).0[1] < 0., "{k:?} must drop away from the pylon, not up into it");
        }
    }
    use crate::deep::integration::xp_consequences::RecordingForceSink;

    #[test]
    fn a_dropped_engine_falls_bounces_rolls_and_comes_to_rest() {
        let mut b = Body { pos: [0., 3.0, 0.], vel: [6.0, 0., 0.], heading: 0., luck: 0.5, ..Default::default() };
        let mut landed = false;
        for _ in 0..4000 {
            b.step(0.02, Some(0.0));
            landed |= b.grounded;
            assert!(b.pos[1] >= b.contact() - 1e-9, "never sinks into the ground");
        }
        assert!(landed);
        assert!(b.resting, "friction stops it");
        assert!(b.pos[0] > 0.5, "it carried on along its momentum");
        assert!(!b.burning, "a short drop at taxi speed only ignites on bad luck");
        assert!(ignition_chance(5.4, 6.0) < 0.5 && ignition_chance(20.0, 70.0) > 0.9);
    }

    #[test]
    fn a_fast_impact_sets_the_engine_on_fire_and_it_burns_out() {
        let mut b = Body { pos: [0., 200.0, 0.], vel: [70.0, 0., 0.], luck: 0.5, ..Default::default() };
        for _ in 0..3000 {
            b.step(0.02, Some(0.0));
        }
        assert!(b.burning);
        let mut t = 0.;
        while t < BURN_S + 5. {
            b.step(1.0, Some(0.0));
            t += 1.0;
        }
        assert!(!b.burning, "the fire burns out");
    }

    #[test]
    fn losing_a_left_engine_rolls_right_yaws_left_and_lightens() {
        let sink = RecordingForceSink::new();
        separation_forces(&sink, [-25.77, -0.51, -1.14], [0., 0.85, -2.44], 0.0);
        assert!(sink.total_on(PlugForceAxis::Roll) > 0.0, "the lighter left wing rises: roll right");
        assert!(sink.total_on(PlugForceAxis::Fnrml) > 90_000.0);
        let sink = RecordingForceSink::new();
        separation_forces(&sink, [-25.77, -0.51, -1.14], [0., 0.85, -2.44], 10_000.0);
        assert!(sink.total_on(PlugForceAxis::Yaw) < 0.0, "pylon stub drag on the left yaws left");
        assert!(sink.total_on(PlugForceAxis::Faxil) > 0.0);
    }

    #[test]
    fn engine_textures_point_at_the_livery_when_it_has_them() {
        let dir = std::env::temp_dir().join("engdetach_livery_test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("A380_EXTERIOR_ENG_LH_ALBEDO.dds"), b"x").unwrap();
        let got = livery_texture_line("TEXTURE A380_EXTERIOR_ENG_LH_ALBEDO.dds", &dir).unwrap();
        assert_eq!(got, ("TEXTURE A380_EXTERIOR_ENG_LH_ALBEDO.dds".to_string(), true));
        let base = livery_texture_line("TEXTURE_MAP normal A380_NOT_IN_LIVERY.png", &dir).unwrap();
        assert_eq!(base, ("TEXTURE_MAP normal ../../../objects/A380_NOT_IN_LIVERY.png".to_string(), false));
        assert!(!base.0.contains(' ') || !base.0.split_whitespace().last().unwrap().contains(' '));
        assert!(livery_texture_line("POINT_COUNTS 1 0 0 3", &dir).is_none());
    }

    #[test]
    fn layout_reads_the_generated_manifest() {
        let l = Layout::parse(r#"{"engines":{"0":{"centre":[-25.77,-0.507,-1.136],"half_extent":[2,2.4,5.9],"objects":["a.obj","b.obj"]}},"fire":"f.obj"}"#).unwrap();
        assert_eq!(l.objects[0].len(), 2);
        assert_eq!(l.fire.as_deref(), Some("f.obj"));
        assert!((l.centre[0][0] + 25.77).abs() < 1e-9);
    }

    #[test]
    fn layout_reads_cowls_and_damaged_variants() {
        let l = Layout::parse(r#"{"engines":{"1":{"centre":[-14.8,-1,-9],"objects":["a.obj"],"crushed":["a_crushed.obj"],"burnt":["a_burnt.obj"]}},"cowls":{"1":{"centre":[-14.8,-1,-11.5],"half_extent":[2,2,1.7],"bottom":-1.9,"objects":["c.obj"]}}}"#).unwrap();
        assert_eq!(l.crushed[1], vec!["a_crushed.obj".to_string()]);
        assert_eq!(l.burnt[1].len(), 1);
        assert_eq!(l.cowl_objects[1], vec!["c.obj".to_string()]);
        assert!((l.cowl_bottom[1] - 1.9).abs() < 1e-9);
        assert!(l.cowl_objects[0].is_empty());
    }

    fn tail() -> std::collections::BTreeMap<String, PartPiece> {
        parse_parts(r#"{"parts":{"hstab_l":{"centre":[-8,5,40],"bottom":-1.4,"half_extent":[7,1.4,7],"objects":["s.obj"],"sub":{"l_elevator_inner":["si.obj"],"l_elevator_outer":["so.obj"]}},"l_elevator_inner":{"centre":[-5,5,44],"objects":["i.obj"],"sub":{}},"l_elevator_outer":{"centre":[-10,5,45],"objects":["o.obj"],"sub":{}}}}"#)
    }

    fn flags(on: &[&str]) -> Vec<(String, bool)> {
        ["hstab_l", "l_elevator_inner", "l_elevator_outer"].iter().map(|n| (n.to_string(), on.contains(n))).collect()
    }

    #[test]
    fn a_stabiliser_half_falls_with_both_its_elevators() {
        let plan = release_plan(&flags(&["hstab_l", "l_elevator_inner", "l_elevator_outer"]), &tail(), &Default::default());
        assert_eq!(plan, vec![("hstab_l".to_string(), vec!["l_elevator_inner".to_string(), "l_elevator_outer".to_string()])]);
        assert!((tail()["hstab_l"].bottom - 1.4).abs() < 1e-9);
    }

    #[test]
    fn a_wing_broken_inboard_falls_with_every_section_outboard_that_is_still_on() {
        let wing = parse_parts(
            r#"{"parts":{
            "wing2_l":{"centre":[-15,1,5],"objects":["w2.obj"],"sub":{"l_spoiler3":["w2s3.obj"]},"carry":["wing3_l","wingtip_l"]},
            "wing3_l":{"centre":[-25,2,10],"objects":["w3.obj"],"sub":{"l_spoiler7":["w3s7.obj"]},"carry":["wingtip_l"]},
            "wingtip_l":{"centre":[-35,2,14],"objects":["t.obj"],"sub":{"l_aileron2":["ta2.obj"]}},
            "l_spoiler3":{"centre":[-12,1,4],"objects":["s3.obj"],"sub":{}}}}"#,
        );
        assert_eq!(wing["wing2_l"].carry, vec!["wing3_l".to_string(), "wingtip_l".to_string()]);
        let on = |names: &[&str]| names.iter().map(|n| (n.to_string(), true)).collect::<Vec<_>>();
        let all = on(&["wing2_l", "wing3_l", "wingtip_l", "l_spoiler3", "l_spoiler7", "l_aileron2"]);
        let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(release_plan(&all, &wing, &Default::default()), vec![("wing2_l".to_string(), names(&["l_spoiler3", "wing3_l", "l_spoiler7", "wingtip_l", "l_aileron2"]))]);
        let tip_gone: std::collections::HashSet<String> = ["wingtip_l".to_string(), "l_aileron2".to_string()].into();
        assert_eq!(release_plan(&all, &wing, &tip_gone), vec![("wing2_l".to_string(), names(&["l_spoiler3", "wing3_l", "l_spoiler7"]))]);
    }

    #[test]
    fn an_elevator_that_already_fell_does_not_fall_again_with_its_stabiliser() {
        let first = release_plan(&flags(&["l_elevator_inner"]), &tail(), &Default::default());
        assert_eq!(first, vec![("l_elevator_inner".to_string(), vec![])]);
        let fallen: std::collections::HashSet<String> = ["l_elevator_inner".to_string()].into();
        let later = release_plan(&flags(&["hstab_l", "l_elevator_inner", "l_elevator_outer"]), &tail(), &fallen);
        assert_eq!(later, vec![("hstab_l".to_string(), vec!["l_elevator_outer".to_string()])]);
        assert!(release_plan(&flags(&["l_elevator_inner"]), &tail(), &fallen).is_empty());
    }

    #[test]
    fn a_gp7200_drops_its_own_engine_and_cowl() {
        let l = Layout::parse(r#"{"engines":{"1":{"centre":[-14.8,-1,-9],"bottom":-2.15,"objects":["t.obj"],"crushed":["t_crushed.obj"]}},"fire":"f.obj","gp":{"engines":{"1":{"centre":[-14.8,-1,-9],"bottom":-2.02,"objects":["t_gp.obj"],"crushed":["t_gp_crushed.obj"]}},"cowls":{"1":{"centre":[-14.8,-1.4,-11.7],"objects":["c_gp.obj"]}}}}"#).unwrap();
        assert_eq!(l.for_engine_type(false).objects[1], vec!["t.obj".to_string()]);
        let gp = l.for_engine_type(true);
        assert_eq!(gp.objects[1], vec!["t_gp.obj".to_string()]);
        assert_eq!(gp.crushed[1], vec!["t_gp_crushed.obj".to_string()]);
        assert_eq!(gp.cowl_objects[1], vec!["c_gp.obj".to_string()]);
        assert!((gp.bottom[1] - 2.02).abs() < 1e-9);
        assert_eq!(gp.fire.as_deref(), Some("f.obj"));
        let trent_only = Layout::parse(r#"{"engines":{"1":{"centre":[-14.8,-1,-9],"objects":["t.obj"]}}}"#).unwrap();
        assert_eq!(trent_only.for_engine_type(true).objects[1], vec!["t.obj".to_string()]);
    }

    #[test]
    fn a_hard_landing_crushes_the_engine_and_a_long_fire_scorches_it_for_good() {
        let mut b = Body { pos: [0., 300.0, 0.], vel: [80.0, 0., 0.], luck: 0.0, ..Default::default() };
        for _ in 0..2000 {
            b.step(0.02, Some(0.0));
        }
        assert!(b.peak_impact > CRUSH_IMPACT_MS, "{}", b.peak_impact);
        let crushed = look_after(&b, Look::Intact);
        assert_eq!(crushed, Look::Crushed);
        let mut t = 0.;
        while t < SCORCH_AFTER_S + 2. {
            b.step(1.0, Some(0.0));
            t += 1.;
        }
        assert_eq!(look_after(&b, crushed), Look::Burnt);
        b.burning = false;
        assert_eq!(look_after(&b, Look::Burnt), Look::Burnt, "it never un-burns");
        let gentle = Body { peak_impact: 5.0, ..Default::default() };
        assert_eq!(look_after(&gentle, Look::Intact), Look::Intact);
    }

    #[test]
    fn a_torn_cowl_falls_slower_and_drifts_less_than_an_engine() {
        let start = Body { pos: [0., 500.0, 0.], vel: [120.0, 0., 0.], ..Default::default() };
        let mut engine = start;
        let mut cowl = Body { drag: COWL_DRAG_PER_KG, ..start };
        for _ in 0..500 {
            engine.step(0.02, None);
            cowl.step(0.02, None);
        }
        assert!(cowl.pos[1] > engine.pos[1]);
        assert!(cowl.pos[0] < engine.pos[0]);
    }

    #[test]
    fn a_lost_left_cowl_drags_and_yaws_left() {
        let sink = RecordingForceSink::new();
        cowl_forces(&sink, [-25.77, -0.51, -1.14], 10_000.0);
        assert!(sink.total_on(PlugForceAxis::Faxil) > 0.0);
        assert!(sink.total_on(PlugForceAxis::Yaw) < 0.0);
    }
}

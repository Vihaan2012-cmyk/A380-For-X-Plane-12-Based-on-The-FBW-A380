//! Keeps everything the airframe sheds from passing through anything solid:
//! each falling body is a cloud of spheres fitted to its own surface
//! (tools/optimize/collision_data.py), the airframe is a per-cell map of its
//! solid height interval in aircraft axes, split into layers that drop out
//! once their part has separated (or, for the gear, retracted).

use crate::engine_detach::{Body, Pose};

pub type Sphere = [f64; 4];

pub fn parse_spheres(v: Option<&serde_json::Value>) -> Vec<Sphere> {
    v.and_then(|a| a.as_array())
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let s = s.as_array()?;
            Some([s.first()?.as_f64()?, s.get(1)?.as_f64()?, s.get(2)?.as_f64()?, s.get(3)?.as_f64()?])
        })
        .collect()
}

pub fn bounding_radius(spheres: &[Sphere]) -> f64 {
    spheres.iter().map(|s| (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt() + s[3]).fold(0., f64::max)
}

fn orientation(body: &Body) -> Pose {
    Pose { position: [0.; 3], psi: body.heading, theta: body.pitch, phi: body.roll }
}

pub fn world_spheres(body: &Body, spheres: &[Sphere]) -> Vec<Sphere> {
    let o = orientation(body);
    spheres
        .iter()
        .map(|s| {
            let r = o.rotate([s[0], s[1], s[2]]);
            [body.pos[0] + r[0], body.pos[1] + r[1], body.pos[2] + r[2], s[3]]
        })
        .collect()
}

/// How far below the body's origin its lowest point lies, as drawn now.
pub fn ground_contact(body: &Body, spheres: &[Sphere]) -> f64 {
    let o = orientation(body);
    spheres.iter().map(|s| s[3] - o.rotate([s[0], s[1], s[2]])[1]).fold(0., f64::max)
}

#[derive(Clone, Debug, Default)]
pub struct AirframeMap {
    cell: f64,
    x0: f64,
    z0: f64,
    nx: usize,
    nz: usize,
    pub ground: Option<f64>,
    gates: Vec<Vec<String>>,
    cells: std::collections::HashMap<usize, Vec<(usize, f64, f64)>>,
    obstacle_gates: Vec<Vec<String>>,
    obstacles: std::collections::HashMap<(i64, i64, i64), Vec<(usize, Sphere)>>,
}

const OBSTACLE_CELL: f64 = 1.0;

fn obstacle_key(p: [f64; 3]) -> (i64, i64, i64) {
    ((p[0] / OBSTACLE_CELL).floor() as i64, (p[1] / OBSTACLE_CELL).floor() as i64, (p[2] / OBSTACLE_CELL).floor() as i64)
}

impl AirframeMap {
    pub fn parse(text: &str) -> Option<AirframeMap> {
        let root: serde_json::Value = serde_json::from_str(text).ok()?;
        let mut map = AirframeMap {
            cell: root.get("cell")?.as_f64()?,
            x0: root.get("x0")?.as_f64()?,
            z0: root.get("z0")?.as_f64()?,
            nx: root.get("nx")?.as_u64()? as usize,
            nz: root.get("nz")?.as_u64()? as usize,
            ground: root.get("ground").and_then(|g| g.as_f64()),
            gates: Vec::new(),
            cells: Default::default(),
            obstacle_gates: Vec::new(),
            obstacles: Default::default(),
        };
        for (key, cells) in root.get("layers")?.as_object()? {
            let layer = map.gates.len();
            map.gates.push(if key == "base" { Vec::new() } else { key.split('|').map(str::to_string).collect() });
            for c in cells.as_array()? {
                let c = c.as_array()?;
                let (i, j) = (c.first()?.as_u64()? as usize, c.get(1)?.as_u64()? as usize);
                map.cells.entry(i * map.nz + j).or_default().push((layer, c.get(2)?.as_f64()?, c.get(3)?.as_f64()?));
            }
        }
        for o in root.get("obstacles").and_then(|o| o.as_array()).into_iter().flatten() {
            let index = map.obstacle_gates.len();
            map.obstacle_gates.push(o.get("gates").and_then(|g| g.as_array()).into_iter().flatten().filter_map(|g| g.as_str().map(str::to_string)).collect());
            for sphere in parse_spheres(o.get("spheres")) {
                map.obstacles.entry(obstacle_key([sphere[0], sphere[1], sphere[2]])).or_default().push((index, sphere));
            }
        }
        Some(map)
    }

    pub fn touching(&self, p: [f64; 3], r: f64, active: &[bool]) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(cell) = self.cell_of(p[0], p[2]) {
            for &(layer, lo, hi) in self.cells.get(&cell).into_iter().flatten() {
                if active.get(layer).copied().unwrap_or(true) && hi > p[1] - r && p[1] + r > lo {
                    out.push(format!("{} [{lo:.2},{hi:.2}]", if self.gates[layer].is_empty() { "base".to_string() } else { self.gates[layer].join("|") }));
                }
            }
        }
        out
    }

    /// The map's layers then its obstacles, in the order `active` follows.
    pub fn gates(&self) -> impl Iterator<Item = &[String]> {
        self.gates.iter().chain(&self.obstacle_gates).map(Vec::as_slice)
    }

    fn obstacle_overlap(&self, p: [f64; 3], r: f64, active: &[bool]) -> f64 {
        let (ki, kj, kk) = obstacle_key(p);
        let mut total = 0.;
        for di in -1..=1 {
            for dj in -1..=1 {
                for dk in -1..=1 {
                    for (index, t) in self.obstacles.get(&(ki + di, kj + dj, kk + dk)).into_iter().flatten() {
                        if !active.get(self.gates.len() + index).copied().unwrap_or(true) {
                            continue;
                        }
                        let d = ((p[0] - t[0]).powi(2) + (p[1] - t[1]).powi(2) + (p[2] - t[2]).powi(2)).sqrt();
                        let o = r + t[3] - d;
                        if o > 0. {
                            total += o;
                        }
                    }
                }
            }
        }
        total
    }

    fn cell_of(&self, x: f64, z: f64) -> Option<usize> {
        let i = ((x - self.x0) / self.cell).floor();
        let j = ((z - self.z0) / self.cell).floor();
        (i >= 0. && j >= 0. && (i as usize) < self.nx && (j as usize) < self.nz).then(|| i as usize * self.nz + j as usize)
    }

    fn solid(&self, cell: usize, lo_y: f64, hi_y: f64, active: &[bool]) -> Option<(f64, f64)> {
        let mut found: Option<(f64, f64)> = None;
        for &(layer, lo, hi) in self.cells.get(&cell).into_iter().flatten() {
            if active.get(layer).copied().unwrap_or(true) && hi > lo_y && hi_y > lo {
                found = Some(found.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
            }
        }
        found
    }

    /// Depth one sphere (aircraft axes) reaches into the solid airframe:
    /// the shorter way out, up or down, over the cells it covers.
    pub fn overlap(&self, p: [f64; 3], r: f64, active: &[bool]) -> f64 {
        self.column_overlap(p, r, active) + self.obstacle_overlap(p, r, active)
    }

    fn column_overlap(&self, p: [f64; 3], r: f64, active: &[bool]) -> f64 {
        let mut vertical: f64 = 0.;
        let reach = r * 0.7;
        for (dx, dz) in [(0., 0.), (reach, reach), (reach, -reach), (-reach, reach), (-reach, -reach)] {
            let Some(cell) = self.cell_of(p[0] + dx, p[2] + dz) else { continue };
            if let Some((lo, hi)) = self.solid(cell, p[1] - r, p[1] + r, active) {
                vertical = vertical.max((hi - (p[1] - r)).min((p[1] + r) - lo));
            }
        }
        vertical
    }
}

/// The aircraft's own frame for the airframe test.
pub struct Frame {
    pub pose: Pose,
    pub cg_reference: [f64; 3],
}

impl Frame {
    pub fn to_acf(&self, w: [f64; 3]) -> [f64; 3] {
        let d = [w[0] - self.pose.position[0], w[1] - self.pose.position[1], w[2] - self.pose.position[2]];
        let a = self.pose.unrotate(d);
        [a[0] + self.cg_reference[0], a[1] + self.cg_reference[1], a[2] + self.cg_reference[2]]
    }
}

pub fn airframe_penetration(world: &[Sphere], map: &AirframeMap, frame: &Frame, active: &[bool]) -> f64 {
    world.iter().map(|s| map.overlap(frame.to_acf([s[0], s[1], s[2]]), s[3], active)).sum()
}

/// Per sphere of `mine`: how deep it sits in the airframe (when given) plus
/// how far it crosses into each of the other bodies' spheres.
pub fn sphere_overlaps(mine: &[Sphere], airframe: Option<(&AirframeMap, &Frame, &[bool])>, others: &[&[Sphere]]) -> Vec<f64> {
    mine.iter()
        .map(|s| {
            let mut o = airframe.map_or(0., |(map, frame, active)| map.overlap(frame.to_acf([s[0], s[1], s[2]]), s[3], active));
            for other in others {
                for t in other.iter() {
                    let d = ((s[0] - t[0]).powi(2) + (s[1] - t[1]).powi(2) + (s[2] - t[2]).powi(2)).sqrt();
                    let x = s[3] + t[3] - d;
                    if x > 0. {
                        o += x;
                    }
                }
            }
            o
        })
        .collect()
}

pub fn mutual_penetration(a: &[Sphere], b: &[Sphere]) -> f64 {
    let mut total = 0.;
    for s in a {
        for t in b {
            let d = ((s[0] - t[0]).powi(2) + (s[1] - t[1]).powi(2) + (s[2] - t[2]).powi(2)).sqrt();
            let o = s[3] + t[3] - d;
            if o > 0. {
                total += o;
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slab_map() -> AirframeMap {
        let wing: Vec<String> = (12..30).flat_map(|i| (12..30).map(move |j| format!("[{i},{j},1.0,1.5]"))).collect();
        AirframeMap::parse(&format!(r#"{{"cell":0.5,"x0":-10,"z0":-10,"nx":40,"nz":40,"layers":{{"base":[{}],"gear":[[5,5,-5,1]]}}}}"#, wing.join(","))).unwrap()
    }

    #[test]
    fn a_sphere_inside_a_wing_cell_overlaps_by_the_shorter_way_out() {
        let map = slab_map();
        let on = vec![true; 2];
        assert!((map.overlap([0.1, 1.4, 0.1], 0.2, &on) - 0.3).abs() < 1e-9);
        assert_eq!(map.overlap([0.1, 2.0, 0.1], 0.2, &on), 0.);
        assert_eq!(map.overlap([0.1, 0.5, 0.1], 0.2, &on), 0.);
    }

    #[test]
    fn a_layer_that_is_gone_is_no_longer_solid() {
        let map = slab_map();
        let gear = map.gates().position(|g| g == ["gear"]).unwrap();
        let mut on = vec![true; 2];
        assert!(map.overlap([-7.4, 0., -7.4], 0.2, &on) > 0.);
        on[gear] = false;
        assert_eq!(map.overlap([-7.4, 0., -7.4], 0.2, &on), 0.);
    }

    #[test]
    fn a_tilted_body_rests_on_its_lowest_sphere() {
        let mut b = Body { roll: 90., ..Default::default() };
        let spheres = [[2.0, 0., 0., 0.1], [-2.0, 0., 0., 0.1]];
        assert!((ground_contact(&b, &spheres) - 2.1).abs() < 1e-9);
        b.roll = 0.;
        assert!((ground_contact(&b, &spheres) - 0.1).abs() < 1e-9);
    }

    #[test]
    fn the_aircraft_frame_round_trips_through_world_axes() {
        let frame = Frame { pose: Pose { position: [100., 5., -40.], psi: 37., theta: 4., phi: -9. }, cg_reference: [0., 0.8, -2.4] };
        let acf = [12.0, -1.5, 3.0];
        let world = frame.pose.to_world(frame.cg_reference, acf);
        let back = frame.to_acf(world);
        for k in 0..3 {
            assert!((back[k] - acf[k]).abs() < 1e-9, "{back:?}");
        }
    }

    #[test]
    fn touching_bodies_overlap_by_how_far_their_spheres_cross() {
        let a = [[0., 0., 0., 1.0]];
        let b = [[1.5, 0., 0., 1.0]];
        assert!((mutual_penetration(&a, &b) - 0.5).abs() < 1e-9);
        assert_eq!(mutual_penetration(&a, &[[3., 0., 0., 1.]]), 0.);
    }
}

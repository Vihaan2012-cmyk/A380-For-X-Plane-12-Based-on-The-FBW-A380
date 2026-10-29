//! Cuts the triangle count of an X-Plane OBJ8 batch by batch with
//! meshoptimizer, keeping every command, animation and attribute, and drops
//! the vertices nothing uses any more. Each level becomes its own
//! `ATTR_LOD near far` block.
//!
//! objdecimate <in.obj> <out.obj> <near:far:max_error_m[:mode[:ratio[:rel]]]>...
//! mode: `keep` (seams and normals kept, the default), `prune` (also drops
//! small disconnected parts), `sloppy` (positions only, down to `ratio`).
//! rel: when set, each connected part is simplified on its own to at most
//! rel x its size (capped at max_error), so small parts keep their shape.
//!
//! objdecimate <in.obj> <out.obj> weld
//! merges identical vertices and drops unused ones; triangles and commands
//! stay as they are.

use meshopt::{SimplifyOptions, VertexDataAdapter};
use std::collections::HashMap;

struct Obj {
    header: Vec<String>,
    vt_lines: Vec<String>,
    vt: Vec<[f32; 8]>,
    indices: Vec<u32>,
    commands: Vec<String>,
}

struct Level {
    near: f64,
    far: f64,
    max_error: f32,
    mode: String,
    ratio: f64,
    rel: f32,
}

fn parse(text: &str) -> Obj {
    let mut obj = Obj { header: Vec::new(), vt_lines: Vec::new(), vt: Vec::new(), indices: Vec::new(), commands: Vec::new() };
    let mut seen_points = false;
    for line in text.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("VT ").or_else(|| t.strip_prefix("VT\t")) {
            let v: Vec<f32> = rest.split_whitespace().map(|x| x.parse().expect("VT value")).collect();
            assert_eq!(v.len(), 8, "VT line with {} values: {line}", v.len());
            obj.vt.push(std::array::from_fn(|i| v[i]));
            obj.vt_lines.push(line.to_string());
            seen_points = true;
        } else if let Some(rest) = t.strip_prefix("IDX10").or_else(|| t.strip_prefix("IDX")) {
            obj.indices.extend(rest.split_whitespace().map(|x| x.parse::<u32>().expect("IDX value")));
            seen_points = true;
        } else if t.starts_with("VLINE") || t.starts_with("VLIGHT") {
            panic!("VLINE/VLIGHT points are not supported");
        } else if seen_points {
            obj.commands.push(line.to_string());
        } else {
            obj.header.push(line.to_string());
        }
    }
    obj
}

fn level(spec: &str) -> Level {
    let p: Vec<&str> = spec.split(':').collect();
    assert!(p.len() >= 3, "level {spec}: near:far:max_error_m[:mode[:ratio]]");
    Level {
        near: p[0].parse().expect("near"),
        far: p[1].parse().expect("far"),
        max_error: p[2].parse().expect("max_error_m"),
        mode: p.get(3).unwrap_or(&"keep").to_string(),
        ratio: p.get(4).map_or(0.02, |r| r.parse().expect("ratio")),
        rel: p.get(5).map_or(0., |r| r.parse().expect("rel")),
    }
}

/// The batch's connected parts (triangles sharing a vertex position), each simplified on its own so
/// a small part's error limit follows its own size.
fn parts(batch: &[u32], vt: &[[f32; 8]]) -> Vec<Vec<u32>> {
    let key = |i: u32| {
        let v = vt[i as usize];
        ((v[0] * 1e5).round() as i64, (v[1] * 1e5).round() as i64, (v[2] * 1e5).round() as i64)
    };
    let tris = batch.len() / 3;
    let mut parent: Vec<usize> = (0..tris).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    let mut owner: HashMap<(i64, i64, i64), usize> = HashMap::new();
    for t in 0..tris {
        for k in 0..3 {
            let kk = key(batch[t * 3 + k]);
            if let Some(&o) = owner.get(&kk) {
                let (a, b) = (find(&mut parent, o), find(&mut parent, t));
                if a != b {
                    parent[a] = b;
                }
            } else {
                owner.insert(kk, t);
            }
        }
    }
    let mut groups: HashMap<usize, Vec<u32>> = HashMap::new();
    for t in 0..tris {
        let r = find(&mut parent, t);
        groups.entry(r).or_default().extend_from_slice(&batch[t * 3..t * 3 + 3]);
    }
    let mut out: Vec<Vec<u32>> = groups.into_values().collect();
    out.sort_by_key(|g| g[0]);
    out
}

fn extent(part: &[u32], vt: &[[f32; 8]]) -> f32 {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for &i in part {
        for k in 0..3 {
            lo[k] = lo[k].min(vt[i as usize][k]);
            hi[k] = hi[k].max(vt[i as usize][k]);
        }
    }
    ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2)).sqrt()
}

fn decimate(obj: &Obj, levels: &[Level]) -> (Vec<u32>, Vec<String>) {
    let lods = obj.commands.iter().filter(|l| l.trim_start().starts_with("ATTR_LOD")).count();
    assert!(lods <= 1, "the OBJ already has {lods} LODs");

    let positions: Vec<f32> = obj.vt.iter().flat_map(|v| [v[0], v[1], v[2]]).collect();
    let bytes: &[u8] = unsafe { std::slice::from_raw_parts(positions.as_ptr().cast(), positions.len() * 4) };
    let adapter = VertexDataAdapter::new(bytes, 12, 0).expect("vertex adapter");
    let attributes: Vec<f32> = obj.vt.iter().flat_map(|v| [v[3], v[4], v[5], v[6], v[7]]).collect();
    let weights = [0.5f32, 0.5, 0.5, 1.0, 1.0];
    let locks = vec![false; obj.vt.len()];

    let mut new_indices: Vec<u32> = Vec::new();
    let mut commands = Vec::new();
    for lv in levels {
        commands.push(format!("ATTR_LOD {} {}", lv.near, lv.far));
        let (mut before, mut after) = (0usize, 0usize);
        let (mut worst, mut weighted) = (0f32, 0f64);
        for line in &obj.commands {
            let t = line.trim_start();
            if t.starts_with("ATTR_LOD") {
                continue;
            }
            let Some(rest) = t.strip_prefix("TRIS") else {
                commands.push(line.clone());
                continue;
            };
            let mut it = rest.split_whitespace();
            let offset: usize = it.next().expect("TRIS offset").parse().expect("TRIS offset");
            let count: usize = it.next().expect("TRIS count").parse().expect("TRIS count");
            let batch = &obj.indices[offset..offset + count];
            let target = ((count as f64 * lv.ratio) as usize / 3 * 3).max(3);
            let mut error = 0f32;
            let simplified = if count <= 3 * 64 {
                batch.to_vec()
            } else if lv.rel > 0. && lv.mode != "sloppy" {
                let mut out = Vec::with_capacity(batch.len());
                for part in parts(batch, &obj.vt) {
                    let extent = extent(&part, &obj.vt);
                    let limit = lv.max_error.min(lv.rel * extent);
                    if part.len() <= 3 * 16 || limit <= 0. {
                        out.extend_from_slice(&part);
                        continue;
                    }
                    let mut e = 0f32;
                    let t = ((part.len() as f64 * lv.ratio) as usize / 3 * 3).max(3);
                    let s = meshopt::simplify_with_attributes_and_locks(&part, &adapter, &attributes, &weights, 5 * 4, &locks, t, limit, SimplifyOptions::Sparse | SimplifyOptions::ErrorAbsolute, Some(&mut e));
                    error = error.max(e);
                    out.extend_from_slice(&s);
                }
                out
            } else if lv.mode == "sloppy" {
                meshopt::simplify_sloppy(batch, &adapter, target, lv.max_error, Some(&mut error))
            } else {
                let mut options = SimplifyOptions::Sparse | SimplifyOptions::ErrorAbsolute;
                if lv.mode == "prune" {
                    options |= SimplifyOptions::Prune;
                }
                meshopt::simplify_with_attributes_and_locks(batch, &adapter, &attributes, &weights, 5 * 4, &locks, target, lv.max_error, options, Some(&mut error))
            };
            before += count / 3;
            after += simplified.len() / 3;
            worst = worst.max(error);
            weighted += error as f64 * (count / 3) as f64;
            let indent = &line[..line.len() - t.len()];
            commands.push(format!("{indent}TRIS {} {}", new_indices.len(), simplified.len()));
            new_indices.extend_from_slice(&simplified);
        }
        eprintln!(
            "LOD {}-{} m, {} {} m: {before} -> {after} triangles ({:.1}%), error max {:.4} m, mean {:.4} m",
            lv.near,
            lv.far,
            lv.mode,
            lv.max_error,
            100. * after as f64 / before.max(1) as f64,
            worst,
            weighted / before.max(1) as f64
        );
    }
    (new_indices, commands)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: objdecimate <in.obj> <out.obj> <near:far:max_error_m[:mode[:ratio]]>...");
        std::process::exit(2);
    }
    let text = std::fs::read_to_string(&args[1]).expect("read the OBJ");
    let obj = parse(&text);
    let (mut new_indices, commands) = if args[3] == "weld" {
        let mut first: HashMap<[u32; 8], u32> = HashMap::new();
        let canonical: Vec<u32> = obj.vt.iter().enumerate().map(|(i, v)| *first.entry(v.map(f32::to_bits)).or_insert(i as u32)).collect();
        (obj.indices.iter().map(|&i| canonical[i as usize]).collect(), obj.commands.clone())
    } else {
        let levels: Vec<Level> = args[3..].iter().map(|s| level(s)).collect();
        decimate(&obj, &levels)
    };
    let mut remap = vec![u32::MAX; obj.vt.len()];
    let mut kept: Vec<usize> = Vec::new();
    for i in new_indices.iter_mut() {
        let old = *i as usize;
        if remap[old] == u32::MAX {
            remap[old] = kept.len() as u32;
            kept.push(old);
        }
        *i = remap[old];
    }

    let mut out = String::with_capacity(text.len() / 2);
    for line in &obj.header {
        if line.trim_start().starts_with("POINT_COUNTS") {
            out.push_str(&format!("POINT_COUNTS {} 0 0 {}\n", kept.len(), new_indices.len()));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    for &k in &kept {
        out.push_str(&obj.vt_lines[k]);
        out.push('\n');
    }
    let mut chunks = new_indices.chunks_exact(10);
    for c in &mut chunks {
        out.push_str("IDX10");
        for i in c {
            out.push_str(&format!(" {i}"));
        }
        out.push('\n');
    }
    for i in chunks.remainder() {
        out.push_str(&format!("IDX {i}\n"));
    }
    for line in &commands {
        out.push_str(line);
        out.push('\n');
    }
    std::fs::write(&args[2], out).expect("write the OBJ");
    eprintln!("{} -> {} vertices", obj.vt.len(), kept.len());
}

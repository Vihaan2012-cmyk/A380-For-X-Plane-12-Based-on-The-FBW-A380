use crate::palette;
use crate::projection::{LatLon, Viewport};
use mfd_tree::Shape;
use serde::{Deserialize, Serialize};

pub const CELL_PX: f64 = 24.0;
pub const MAX_SAMPLES: usize = 2_400;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WxSample {
    pub pos: LatLon,
    pub level: u8,
}

pub fn level_from_precip(rate: f64) -> u8 {
    match rate {
        r if r >= 0.75 => 3,
        r if r >= 0.4 => 2,
        r if r >= 0.1 => 1,
        _ => 0,
    }
}

pub fn colour(level: u8) -> Option<&'static str> {
    match level {
        1 => Some(palette::OK),
        2 => Some(palette::WARN),
        3.. => Some(palette::BAD),
        _ => None,
    }
}

pub struct SampleGrid {
    pub cols: usize,
    pub rows: usize,
    pub cell_px: f64,
    pub points: Vec<LatLon>,
}

pub fn sample_grid(v: &Viewport) -> SampleGrid {
    let mut cell = CELL_PX;
    while ((v.width / cell).ceil() * (v.height / cell).ceil()) as usize > MAX_SAMPLES {
        cell *= 1.25;
    }
    let cols = (v.width / cell).ceil() as usize;
    let rows = (v.height / cell).ceil() as usize;
    let mut points = Vec::with_capacity(cols * rows);
    for r in 0..rows {
        for c in 0..cols {
            points.push(v.unproject((c as f64 + 0.5) * cell, (r as f64 + 0.5) * cell));
        }
    }
    SampleGrid { cols, rows, cell_px: cell, points }
}

pub fn grid_shapes(g: &SampleGrid, levels: &[u8]) -> Vec<Shape> {
    let mut out = Vec::new();
    for r in 0..g.rows {
        let mut c = 0;
        while c < g.cols {
            let lv = levels.get(r * g.cols + c).copied().unwrap_or(0);
            let mut end = c + 1;
            while end < g.cols && levels.get(r * g.cols + end).copied().unwrap_or(0) == lv {
                end += 1;
            }
            if let Some(fill) = colour(lv) {
                out.push(Shape {
                    t: "rect".into(),
                    a: [
                        ("x", format!("{:.1}", c as f64 * g.cell_px)),
                        ("y", format!("{:.1}", r as f64 * g.cell_px)),
                        ("width", format!("{:.1}", (end - c) as f64 * g.cell_px)),
                        ("height", format!("{:.1}", g.cell_px)),
                        ("fill", fill.to_string()),
                        ("fill-opacity", "0.45".to_string()),
                    ]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect(),
                });
            }
            c = end;
        }
    }
    out
}

pub fn levels_from_samples(g: &SampleGrid, v: &Viewport, samples: &[WxSample]) -> Vec<u8> {
    let mut levels = vec![0u8; g.cols * g.rows];
    for s in samples {
        let (x, y) = v.project(s.pos);
        if x < 0.0 || y < 0.0 {
            continue;
        }
        let (c, r) = ((x / g.cell_px) as usize, (y / g.cell_px) as usize);
        if c < g.cols && r < g.rows {
            let i = r * g.cols + c;
            levels[i] = levels[i].max(s.level);
        }
    }
    levels
}

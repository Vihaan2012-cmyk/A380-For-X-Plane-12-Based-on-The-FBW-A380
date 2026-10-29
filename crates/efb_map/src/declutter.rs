use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const CHAR_W_PX: f64 = 7.2;
pub const LINE_H_PX: f64 = 14.0;
pub const GAP_PX: f64 = 5.0;
const CELL: f64 = 64.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LabelRequest {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub class: String,
    pub priority: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Rect {
    pub fn around(x: f64, y: f64, half: f64) -> Self {
        Self { x0: x - half, y0: y - half, x1: x + half, y1: y + half }
    }

    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x0 < o.x1 && o.x0 < self.x1 && self.y0 < o.y1 && o.y0 < self.y1
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlacedLabel {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub class: String,
}

#[derive(Default)]
struct Grid {
    cells: HashMap<(i32, i32), Vec<Rect>>,
}

impl Grid {
    fn keys(r: &Rect) -> impl Iterator<Item = (i32, i32)> {
        let (cx0, cy0) = ((r.x0 / CELL).floor() as i32, (r.y0 / CELL).floor() as i32);
        let (cx1, cy1) = ((r.x1 / CELL).floor() as i32, (r.y1 / CELL).floor() as i32);
        (cy0..=cy1).flat_map(move |y| (cx0..=cx1).map(move |x| (x, y)))
    }

    fn hits(&self, r: &Rect) -> bool {
        Self::keys(r).any(|k| self.cells.get(&k).is_some_and(|v| v.iter().any(|o| o.overlaps(r))))
    }

    fn add(&mut self, r: Rect) {
        for k in Self::keys(&r) {
            self.cells.entry(k).or_default().push(r);
        }
    }
}

pub fn text_size(text: &str) -> (f64, f64) {
    (text.chars().count() as f64 * CHAR_W_PX, LINE_H_PX)
}

pub fn declutter(mut reqs: Vec<LabelRequest>, obstacles: &[Rect], width: f64, height: f64) -> (Vec<PlacedLabel>, usize) {
    reqs.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.text.cmp(&b.text)));
    let mut grid = Grid::default();
    for o in obstacles {
        grid.add(*o);
    }
    let bounds = Rect { x0: 0.0, y0: 0.0, x1: width, y1: height };
    let mut placed = Vec::new();
    let mut dropped = 0;
    for r in reqs {
        let (w, h) = text_size(&r.text);
        let candidates = [
            (r.x + GAP_PX, r.y - h - GAP_PX * 0.4),
            (r.x + GAP_PX, r.y + GAP_PX * 0.4),
            (r.x - GAP_PX - w, r.y - h - GAP_PX * 0.4),
            (r.x - GAP_PX - w, r.y + GAP_PX * 0.4),
            (r.x - w / 2.0, r.y - h - GAP_PX * 1.6),
            (r.x - w / 2.0, r.y + GAP_PX * 1.6),
        ];
        let spot = candidates.into_iter().map(|(x, y)| Rect { x0: x, y0: y, x1: x + w, y1: y + h }).find(|b| {
            b.x0 >= bounds.x0 && b.y0 >= bounds.y0 && b.x1 <= bounds.x1 && b.y1 <= bounds.y1 && !grid.hits(b)
        });
        match spot {
            Some(b) => {
                grid.add(b);
                placed.push(PlacedLabel { x: (b.x0 * 10.0).round() / 10.0, y: (b.y0 * 10.0).round() / 10.0, text: r.text, class: r.class });
            }
            None => dropped += 1,
        }
    }
    (placed, dropped)
}

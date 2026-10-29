use crate::style::NodeInfo;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.w > 0.0 && self.h > 0.0 && x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }

    fn intersect(&self, o: &Rect) -> Rect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = (self.x + self.w).min(o.x + o.w);
        let y1 = (self.y + self.h).min(o.y + o.h);
        Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
    }
}

/// The `data-act` of the topmost visible element under (x, y) or of its
/// nearest ancestor that has one.
pub fn hit(nodes: &[NodeInfo], boxes: &[Rect], x: f64, y: f64) -> Option<String> {
    let n = nodes.len().min(boxes.len());
    let screen = Rect::new(-1e7, -1e7, 2e7, 2e7);
    let mut clip: Vec<Rect> = Vec::with_capacity(n);
    let mut best: Option<(i32, usize)> = None;
    for i in 0..n {
        let c = match nodes[i].parent {
            Some(p) if p < i => {
                if nodes[p].clips {
                    clip[p].intersect(&boxes[p])
                } else {
                    clip[p]
                }
            }
            _ => screen,
        };
        clip.push(c);
        if nodes[i].hidden || !boxes[i].contains(x, y) || !c.contains(x, y) {
            continue;
        }
        let key = (nodes[i].layer, i);
        if best.map_or(true, |b| key >= b) {
            best = Some(key);
        }
    }
    let mut cur = best.map(|b| b.1);
    while let Some(i) = cur {
        if let Some(a) = &nodes[i].act {
            return Some(a.clone());
        }
        cur = nodes[i].parent;
    }
    None
}

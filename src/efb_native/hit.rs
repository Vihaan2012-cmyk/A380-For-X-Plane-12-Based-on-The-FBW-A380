use super::doc::NodeInfo;

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

    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }

    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    pub fn shifted(&self, dy: f64) -> Rect {
        Rect { y: self.y - dy, ..*self }
    }

    fn intersect(&self, o: &Rect) -> Rect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = self.right().min(o.right());
        let y1 = self.bottom().min(o.bottom());
        Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub node: usize,
    pub act: Option<(usize, String)>,
    pub scroller: Option<(usize, String)>,
}

/// Where each element is on screen now: its laid-out box moved by the
/// scroll offsets of the containers it sits in, and the clip those
/// containers apply (`None` when it is clipped away entirely).
pub struct Placed {
    pub rect: Vec<Rect>,
    pub clip: Vec<Rect>,
}

pub fn place(nodes: &[NodeInfo], boxes: &[Rect], scroll_top: &dyn Fn(&str) -> f64) -> Placed {
    let screen = Rect::new(-1e7, -1e7, 2e7, 2e7);
    let n = nodes.len().min(boxes.len());
    let mut shift = vec![0.0; n];
    let mut rect: Vec<Rect> = Vec::with_capacity(n);
    let mut clip: Vec<Rect> = Vec::with_capacity(n);
    for i in 0..n {
        let info = &nodes[i];
        let (pshift, pclip) = match info.parent {
            Some(p) if p < i && !info.fixed => {
                let own = nodes[p].scroller.as_deref().map(scroll_top).unwrap_or(0.0);
                let c = if nodes[p].clips { clip[p].intersect(&rect[p]) } else { clip[p] };
                (shift[p] + own, c)
            }
            _ => (0.0, screen),
        };
        shift[i] = pshift;
        rect.push(boxes[i].shifted(pshift));
        clip.push(pclip);
    }
    Placed { rect, clip }
}

pub fn hit(nodes: &[NodeInfo], boxes: &[Rect], scroll_top: &dyn Fn(&str) -> f64, x: f64, y: f64) -> Option<Hit> {
    let placed = place(nodes, boxes, scroll_top);
    let mut best: Option<(i32, usize)> = None;
    for i in 0..placed.rect.len() {
        let info = &nodes[i];
        if info.hidden || !placed.rect[i].contains(x, y) || !placed.clip[i].contains(x, y) {
            continue;
        }
        let key = (info.layer, i);
        if best.map_or(true, |b| key >= b) {
            best = Some(key);
        }
    }
    let node = best?.1;
    let mut act = None;
    let mut scroller = None;
    let mut cur = Some(node);
    while let Some(i) = cur {
        if act.is_none() {
            if let Some(a) = &nodes[i].act {
                act = Some((i, a.clone()));
            }
        }
        if scroller.is_none() {
            if let Some(s) = &nodes[i].scroller {
                scroller = Some((i, s.clone()));
            }
        }
        cur = nodes[i].parent;
    }
    Some(Hit { node, act, scroller })
}

/// How far `idx` can scroll: the bottom of what it contains (nested
/// clipping containers count with their own box) past its own bottom.
pub fn max_scroll(nodes: &[NodeInfo], boxes: &[Rect], idx: usize) -> f64 {
    let Some(own) = boxes.get(idx) else { return 0.0 };
    let mut bottom = own.y;
    for j in idx + 1..nodes.len().min(boxes.len()) {
        if nodes[j].hidden || nodes[j].fixed {
            continue;
        }
        let mut p = nodes[j].parent;
        let mut inside = false;
        while let Some(pi) = p {
            if pi == idx {
                inside = true;
                break;
            }
            if nodes[pi].clips || nodes[pi].fixed {
                break;
            }
            p = nodes[pi].parent;
        }
        if inside && boxes[j].h > 0.0 {
            bottom = bottom.max(boxes[j].bottom());
        }
    }
    (bottom + nodes[idx].padding_bottom - own.bottom()).max(0.0)
}

pub fn find_id(nodes: &[NodeInfo], id: &str) -> Option<usize> {
    nodes.iter().position(|n| n.id.as_deref() == Some(id) || n.scroller.as_deref() == Some(id))
}

pub fn find_act(nodes: &[NodeInfo], act: &str) -> Option<usize> {
    nodes.iter().position(|n| n.act.as_deref() == Some(act))
}

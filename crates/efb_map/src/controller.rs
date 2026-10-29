use crate::layers::LayerToggles;
use crate::projection::{LatLon, Viewport};
use serde::{Deserialize, Serialize};

pub const TAP_DEAD_ZONE_PX: f64 = 6.0;
pub const WHEEL_LEVELS_PER_NOTCH: f64 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct OwnShip {
    pub pos: LatLon,
    pub heading_deg: f64,
    pub track_deg: f64,
    pub gs_kt: f64,
    pub alt_ft: f64,
    pub on_ground: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pointer {
    Tap(f64, f64),
    Dragged,
    None,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapController {
    pub view: Viewport,
    pub follow: bool,
    pub track_up: bool,
    pub toggles: LayerToggles,
    #[serde(skip)]
    drag: Option<Drag>,
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    start: (f64, f64),
    last: (f64, f64),
    moved: bool,
}

impl MapController {
    pub fn new(view: Viewport) -> Self {
        Self { view, follow: true, track_up: false, toggles: LayerToggles::default(), drag: None }
    }

    pub fn update_own(&mut self, own: &OwnShip) {
        self.view.rotation_deg = if self.track_up { own.track_deg.rem_euclid(360.0) } else { 0.0 };
        if self.follow {
            self.view.center = own.pos;
        }
    }

    pub fn pointer_down(&mut self, x: f64, y: f64) {
        self.drag = Some(Drag { start: (x, y), last: (x, y), moved: false });
    }

    pub fn pointer_move(&mut self, x: f64, y: f64) {
        let Some(d) = self.drag.as_mut() else { return };
        if !d.moved && (x - d.start.0).hypot(y - d.start.1) < TAP_DEAD_ZONE_PX {
            return;
        }
        if !d.moved {
            d.moved = true;
            d.last = d.start;
        }
        let (dx, dy) = (x - d.last.0, y - d.last.1);
        d.last = (x, y);
        self.view.pan_pixels(dx, dy);
        self.follow = false;
    }

    pub fn pointer_up(&mut self, x: f64, y: f64) -> Pointer {
        match self.drag.take() {
            Some(d) if d.moved => {
                let (dx, dy) = (x - d.last.0, y - d.last.1);
                self.view.pan_pixels(dx, dy);
                Pointer::Dragged
            }
            Some(_) => Pointer::Tap(x, y),
            None => Pointer::None,
        }
    }

    pub fn wheel(&mut self, x: f64, y: f64, notches: f64) {
        let (ax, ay) = if self.follow { (self.view.width / 2.0, self.view.height / 2.0) } else { (x, y) };
        self.view.zoom_about(ax, ay, notches * WHEEL_LEVELS_PER_NOTCH);
    }

    pub fn pinch(&mut self, cx: f64, cy: f64, scale: f64) {
        if scale > 0.0 {
            let (ax, ay) = if self.follow { (self.view.width / 2.0, self.view.height / 2.0) } else { (cx, cy) };
            self.view.zoom_about(ax, ay, scale.log2());
        }
    }

    pub fn zoom_step(&mut self, levels: f64) {
        self.view.zoom_about(self.view.width / 2.0, self.view.height / 2.0, levels);
    }

    pub fn set_follow(&mut self, on: bool, own: Option<&OwnShip>) {
        self.follow = on;
        if let (true, Some(o)) = (on, own) {
            self.update_own(o);
        }
    }

    pub fn set_track_up(&mut self, on: bool, own: Option<&OwnShip>) {
        self.track_up = on;
        match own {
            Some(o) => self.update_own(o),
            None if !on => self.view.rotation_deg = 0.0,
            None => {}
        }
    }

    pub fn act(&mut self, action: &str, own: Option<&OwnShip>) -> bool {
        match action {
            "map:zoom-in" => self.zoom_step(1.0),
            "map:zoom-out" => self.zoom_step(-1.0),
            "map:follow" => self.set_follow(!self.follow, own),
            "map:orient" => self.set_track_up(!self.track_up, own),
            a => match a.strip_prefix("map:layer:").and_then(crate::layers::LayerKind::from_id) {
                Some(k) => self.toggles.flip(k),
                None => return false,
            },
        }
        true
    }
}

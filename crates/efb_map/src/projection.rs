use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

pub const TILE_PX: f64 = 256.0;
pub const MAX_LAT: f64 = 85.051_128_78;
pub const MIN_ZOOM: f64 = 2.0;
pub const MAX_ZOOM: f64 = 17.0;
const EARTH_CIRCUMFERENCE_NM: f64 = 21_600.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

impl LatLon {
    pub fn new(lat: f64, lon: f64) -> Self {
        Self { lat, lon }
    }
}

pub fn wrap_lon(lon: f64) -> f64 {
    let l = (lon + 180.0).rem_euclid(360.0) - 180.0;
    if l == -180.0 && lon > 0.0 {
        180.0
    } else {
        l
    }
}

pub fn world_px(zoom: f64) -> f64 {
    TILE_PX * zoom.exp2()
}

pub fn to_world(ll: LatLon, zoom: f64) -> (f64, f64) {
    let w = world_px(zoom);
    let lat = ll.lat.clamp(-MAX_LAT, MAX_LAT).to_radians();
    let x = (wrap_lon(ll.lon) + 180.0) / 360.0 * w;
    let y = (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / PI) / 2.0 * w;
    (x, y)
}

pub fn from_world(x: f64, y: f64, zoom: f64) -> LatLon {
    let w = world_px(zoom);
    let lon = wrap_lon(x / w * 360.0 - 180.0);
    let n = PI * (1.0 - 2.0 * y / w);
    let lat = n.sinh().atan().to_degrees();
    LatLon { lat, lon }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TileKey {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

impl TileKey {
    pub fn of(ll: LatLon, z: u8) -> Self {
        let (x, y) = to_world(ll, z as f64);
        let n = 1u32 << z;
        Self { z, x: ((x / TILE_PX) as u32).min(n - 1), y: ((y / TILE_PX).max(0.0) as u32).min(n - 1) }
    }

    pub fn bounds(self) -> (LatLon, LatLon) {
        let z = self.z as f64;
        let nw = from_world(self.x as f64 * TILE_PX, self.y as f64 * TILE_PX, z);
        let se = from_world((self.x + 1) as f64 * TILE_PX - 1e-9, (self.y + 1) as f64 * TILE_PX, z);
        (nw, se)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    pub center: LatLon,
    pub zoom: f64,
    pub rotation_deg: f64,
    pub width: f64,
    pub height: f64,
}

impl Viewport {
    pub fn new(center: LatLon, zoom: f64, width: f64, height: f64) -> Self {
        Self { center, zoom: zoom.clamp(MIN_ZOOM, MAX_ZOOM), rotation_deg: 0.0, width, height }
    }

    fn rot(&self) -> (f64, f64) {
        let r = self.rotation_deg.to_radians();
        (r.sin(), r.cos())
    }

    pub fn project(&self, ll: LatLon) -> (f64, f64) {
        let w = world_px(self.zoom);
        let (cx, cy) = to_world(self.center, self.zoom);
        let (px, py) = to_world(ll, self.zoom);
        let mut dx = px - cx;
        if dx > w / 2.0 {
            dx -= w;
        } else if dx < -w / 2.0 {
            dx += w;
        }
        let dy = py - cy;
        let (s, c) = self.rot();
        (self.width / 2.0 + dx * c + dy * s, self.height / 2.0 - dx * s + dy * c)
    }

    pub fn unproject(&self, sx: f64, sy: f64) -> LatLon {
        let (cx, cy) = to_world(self.center, self.zoom);
        let (x, y) = (sx - self.width / 2.0, sy - self.height / 2.0);
        let (s, c) = self.rot();
        let w = world_px(self.zoom);
        from_world((cx + x * c - y * s).rem_euclid(w), cy + x * s + y * c, self.zoom)
    }

    pub fn pan_pixels(&mut self, dx: f64, dy: f64) {
        let target = self.unproject(self.width / 2.0 - dx, self.height / 2.0 - dy);
        self.center = LatLon { lat: target.lat.clamp(-MAX_LAT, MAX_LAT), lon: target.lon };
    }

    pub fn zoom_about(&mut self, sx: f64, sy: f64, delta_levels: f64) {
        let anchor = self.unproject(sx, sy);
        self.zoom = (self.zoom + delta_levels).clamp(MIN_ZOOM, MAX_ZOOM);
        let (ax, ay) = self.project(anchor);
        self.pan_pixels(sx - ax, sy - ay);
    }

    pub fn nm_per_px(&self) -> f64 {
        EARTH_CIRCUMFERENCE_NM * self.center.lat.to_radians().cos() / world_px(self.zoom)
    }

    pub fn contains(&self, sx: f64, sy: f64, margin: f64) -> bool {
        sx >= -margin && sy >= -margin && sx <= self.width + margin && sy <= self.height + margin
    }

    pub fn corners(&self) -> [LatLon; 4] {
        [
            self.unproject(0.0, 0.0),
            self.unproject(self.width, 0.0),
            self.unproject(self.width, self.height),
            self.unproject(0.0, self.height),
        ]
    }

    pub fn bounds(&self) -> (LatLon, LatLon) {
        let c = self.corners();
        let lat_min = c.iter().map(|p| p.lat).fold(f64::MAX, f64::min);
        let lat_max = c.iter().map(|p| p.lat).fold(f64::MIN, f64::max);
        let rel: Vec<f64> = c.iter().map(|p| wrap_lon(p.lon - self.center.lon)).collect();
        let lo = rel.iter().copied().fold(f64::MAX, f64::min);
        let hi = rel.iter().copied().fold(f64::MIN, f64::max);
        (LatLon::new(lat_min, wrap_lon(self.center.lon + lo)), LatLon::new(lat_max, wrap_lon(self.center.lon + hi)))
    }

    pub fn tiles(&self, z: u8) -> Vec<TileKey> {
        let n = 1i64 << z;
        let (cx, _) = to_world(self.center, z as f64);
        let pts: Vec<(f64, f64)> = self
            .corners()
            .iter()
            .map(|p| {
                let (mut x, y) = to_world(*p, z as f64);
                let w = world_px(z as f64);
                if x - cx > w / 2.0 {
                    x -= w;
                } else if cx - x > w / 2.0 {
                    x += w;
                }
                (x / TILE_PX, y / TILE_PX)
            })
            .collect();
        let x0 = pts.iter().map(|p| p.0).fold(f64::MAX, f64::min).floor() as i64;
        let x1 = pts.iter().map(|p| p.0).fold(f64::MIN, f64::max).floor() as i64;
        let y0 = (pts.iter().map(|p| p.1).fold(f64::MAX, f64::min).floor() as i64).max(0);
        let y1 = (pts.iter().map(|p| p.1).fold(f64::MIN, f64::max).floor() as i64).min(n - 1);
        let mut out = Vec::new();
        for y in y0..=y1 {
            for x in x0..=x1.min(x0 + n - 1) {
                out.push(TileKey { z, x: x.rem_euclid(n) as u32, y: y as u32 });
            }
        }
        out.sort();
        out.dedup();
        out
    }

    pub fn describe(&self) -> String {
        format!(
            "merc;{:.6};{:.6};{:.4};{:.2};{:.0};{:.0}",
            self.center.lat, self.center.lon, self.zoom, self.rotation_deg, self.width, self.height
        )
    }
}

pub fn scale_bar(v: &Viewport, max_px: f64) -> (f64, String) {
    let nm_px = v.nm_per_px();
    let budget = nm_px * max_px;
    let steps = [0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0];
    let nm = steps.iter().copied().filter(|s| *s <= budget).fold(steps[0], f64::max);
    let label = if nm < 1.0 { format!("{:.0} M", nm * 1852.0) } else { format!("{nm:.0} NM") };
    (nm / nm_px, label)
}

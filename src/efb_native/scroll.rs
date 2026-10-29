use std::collections::BTreeMap;

pub const DEAD_ZONE_PX: f64 = 6.0;
pub const FLING_TAU_MS: f64 = 325.0;
pub const FLING_STOP: f64 = 0.02;
pub const FLING_MAX: f64 = 8.0;
pub const WHEEL_STEP_PX: f64 = 48.0;
const VELOCITY_WINDOW_MS: f64 = 100.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scroll {
    pub top: f64,
    pub max: f64,
    pub vel: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Scrolls {
    map: BTreeMap<String, Scroll>,
}

impl Scrolls {
    pub fn top(&self, id: &str) -> f64 {
        self.map.get(id).map_or(0.0, |s| s.top)
    }

    pub fn set_max(&mut self, id: &str, max: f64) {
        let s = self.map.entry(id.to_string()).or_default();
        s.max = max.max(0.0);
        s.top = s.top.clamp(0.0, s.max);
    }

    pub fn set(&mut self, id: &str, top: f64) -> bool {
        let s = self.map.entry(id.to_string()).or_default();
        let t = top.clamp(0.0, s.max);
        let moved = t != s.top;
        s.top = t;
        moved
    }

    pub fn by(&mut self, id: &str, dy: f64) -> bool {
        let t = self.top(id) + dy;
        self.set(id, t)
    }

    pub fn stop(&mut self, id: &str) {
        if let Some(s) = self.map.get_mut(id) {
            s.vel = 0.0;
        }
    }

    pub fn reset(&mut self, id: &str) {
        if let Some(s) = self.map.get_mut(id) {
            s.top = 0.0;
            s.vel = 0.0;
        }
    }

    pub fn fling(&mut self, id: &str, vel_px_per_ms: f64) {
        if let Some(s) = self.map.get_mut(id) {
            s.vel = vel_px_per_ms.clamp(-FLING_MAX, FLING_MAX);
            if s.vel.abs() < FLING_STOP {
                s.vel = 0.0;
            }
        }
    }

    pub fn moving(&self) -> bool {
        self.map.values().any(|s| s.vel != 0.0)
    }

    /// Advances every fling by `dt_ms`; true when an offset changed.
    pub fn tick(&mut self, dt_ms: f64) -> bool {
        let mut moved = false;
        for s in self.map.values_mut() {
            if s.vel == 0.0 || dt_ms <= 0.0 {
                continue;
            }
            let decay = (-dt_ms / FLING_TAU_MS).exp();
            let travel = s.vel * FLING_TAU_MS * (1.0 - decay);
            let t = (s.top + travel).clamp(0.0, s.max);
            moved |= t != s.top;
            let hit_end = t != s.top + travel;
            s.top = t;
            s.vel *= decay;
            if hit_end || s.vel.abs() < FLING_STOP {
                s.vel = 0.0;
            }
        }
        moved
    }

    pub fn to_json(&self) -> String {
        let m: serde_json::Map<String, serde_json::Value> = self.map.iter().map(|(k, s)| (k.clone(), serde_json::json!((s.top * 100.0).round() / 100.0))).collect();
        serde_json::Value::Object(m).to_string()
    }
}

/// Recent pointer samples of a drag, for the release velocity.
#[derive(Clone, Debug, Default)]
pub struct Tracker {
    samples: Vec<(f64, f64)>,
}

impl Tracker {
    pub fn push(&mut self, t_ms: f64, y: f64) {
        self.samples.push((t_ms, y));
        let cutoff = t_ms - VELOCITY_WINDOW_MS;
        while self.samples.len() > 2 && self.samples[0].0 < cutoff {
            self.samples.remove(0);
        }
    }

    /// Pixels per millisecond the content should keep moving (positive
    /// scrolls down, i.e. the finger moved up).
    pub fn velocity(&self) -> f64 {
        let (Some(a), Some(b)) = (self.samples.first(), self.samples.last()) else { return 0.0 };
        let dt = b.0 - a.0;
        if dt <= 0.0 {
            return 0.0;
        }
        -(b.1 - a.1) / dt
    }
}

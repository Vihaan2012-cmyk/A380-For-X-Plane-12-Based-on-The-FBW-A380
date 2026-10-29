use crate::projection::LatLon;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrafficSource {
    Ai,
    Vatsim,
    Ivao,
}

impl TrafficSource {
    pub fn limits_s(self) -> (f64, f64) {
        match self {
            TrafficSource::Ai => (3.0, 10.0),
            TrafficSource::Vatsim | TrafficSource::Ivao => (45.0, 120.0),
        }
    }

    pub fn class(self) -> &'static str {
        match self {
            TrafficSource::Ai => "ai",
            TrafficSource::Vatsim => "vatsim",
            TrafficSource::Ivao => "ivao",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrafficReport {
    pub source: TrafficSource,
    pub id: String,
    pub callsign: String,
    pub pos: LatLon,
    pub alt_ft: f64,
    pub track_deg: f64,
    pub gs_kt: f64,
    pub on_ground: bool,
    pub t_s: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Age {
    Fresh,
    Stale,
    Expired,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrafficView {
    pub report: TrafficReport,
    pub pos: LatLon,
    pub age: Age,
    pub age_s: f64,
}

#[derive(Clone, Debug, Default)]
pub struct TrafficStore {
    items: HashMap<(TrafficSource, String), TrafficReport>,
}

pub fn dead_reckon(p: LatLon, track_deg: f64, gs_kt: f64, dt_s: f64) -> LatLon {
    let d_nm = gs_kt.max(0.0) * dt_s.max(0.0) / 3600.0;
    let t = track_deg.to_radians();
    let dlat = d_nm * t.cos() / 60.0;
    let coslat = p.lat.to_radians().cos().max(1e-6);
    let dlon = d_nm * t.sin() / (60.0 * coslat);
    LatLon::new(p.lat + dlat, crate::projection::wrap_lon(p.lon + dlon))
}

impl TrafficStore {
    pub fn upsert(&mut self, r: TrafficReport) {
        let key = (r.source, r.id.clone());
        match self.items.get(&key) {
            Some(old) if old.t_s > r.t_s => {}
            _ => {
                self.items.insert(key, r);
            }
        }
    }

    pub fn ingest(&mut self, source: TrafficSource, reports: impl IntoIterator<Item = TrafficReport>) {
        for r in reports {
            if r.source == source {
                self.upsert(r);
            }
        }
    }

    pub fn age(r: &TrafficReport, now_s: f64) -> (Age, f64) {
        let dt = (now_s - r.t_s).max(0.0);
        let (stale, drop) = r.source.limits_s();
        let a = if dt >= drop {
            Age::Expired
        } else if dt >= stale {
            Age::Stale
        } else {
            Age::Fresh
        };
        (a, dt)
    }

    pub fn sync(&mut self, reports: impl IntoIterator<Item = TrafficReport>) {
        let mut keep: Vec<(TrafficSource, String)> = Vec::new();
        for r in reports {
            keep.push((r.source, r.id.clone()));
            self.upsert(r);
        }
        self.items.retain(|k, _| keep.contains(k));
    }

    pub fn prune(&mut self, now_s: f64) -> usize {
        let before = self.items.len();
        self.items.retain(|_, r| Self::age(r, now_s).0 != Age::Expired);
        before - self.items.len()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn views(&self, now_s: f64) -> Vec<TrafficView> {
        let mut out: Vec<TrafficView> = self
            .items
            .values()
            .filter_map(|r| {
                let (age, dt) = Self::age(r, now_s);
                if age == Age::Expired {
                    return None;
                }
                let horizon = dt.min(r.source.limits_s().0);
                let pos = if r.on_ground { r.pos } else { dead_reckon(r.pos, r.track_deg, r.gs_kt, horizon) };
                Some(TrafficView { report: r.clone(), pos, age, age_s: dt })
            })
            .collect();
        out.sort_by(|a, b| a.report.source.cmp(&b.report.source).then(a.report.id.cmp(&b.report.id)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(source: TrafficSource, id: &str, t_s: f64) -> TrafficReport {
        TrafficReport { source, id: id.into(), callsign: id.into(), pos: LatLon::new(50.0, 8.0), alt_ft: 5000.0, track_deg: 0.0, gs_kt: 200.0, on_ground: false, t_s }
    }

    #[test]
    fn sync_replaces_the_set_and_keeps_newer_reports() {
        let mut s = TrafficStore::default();
        s.sync(vec![r(TrafficSource::Ai, "1", 10.0), r(TrafficSource::Vatsim, "16777217", 5.0)]);
        s.sync(vec![r(TrafficSource::Ai, "1", 9.0), r(TrafficSource::Ivao, "16777218", 11.0)]);
        let v = s.views(11.0);
        let ids: Vec<(&str, f64)> = v.iter().map(|v| (v.report.id.as_str(), v.report.t_s)).collect();
        assert_eq!(ids, vec![("1", 10.0), ("16777218", 11.0)]);
    }
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayerKind {
    Weather,
    Amdb,
    Airways,
    Airports,
    Vors,
    Ndbs,
    Route,
    AiTraffic,
    OnlineTraffic,
    OwnShip,
}

impl LayerKind {
    pub const ALL: [LayerKind; 10] = [
        LayerKind::Weather,
        LayerKind::Amdb,
        LayerKind::Airways,
        LayerKind::Airports,
        LayerKind::Vors,
        LayerKind::Ndbs,
        LayerKind::Route,
        LayerKind::AiTraffic,
        LayerKind::OnlineTraffic,
        LayerKind::OwnShip,
    ];

    pub fn id(self) -> &'static str {
        match self {
            LayerKind::Weather => "wx",
            LayerKind::Amdb => "amdb",
            LayerKind::Airways => "awy",
            LayerKind::Airports => "apt",
            LayerKind::Vors => "vor",
            LayerKind::Ndbs => "ndb",
            LayerKind::Route => "route",
            LayerKind::AiTraffic => "ai",
            LayerKind::OnlineTraffic => "online",
            LayerKind::OwnShip => "own",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            LayerKind::Weather => "Weather",
            LayerKind::Amdb => "Airport map",
            LayerKind::Airways => "Airways",
            LayerKind::Airports => "Airports",
            LayerKind::Vors => "VOR",
            LayerKind::Ndbs => "NDB",
            LayerKind::Route => "Route",
            LayerKind::AiTraffic => "AI traffic",
            LayerKind::OnlineTraffic => "Online traffic",
            LayerKind::OwnShip => "Own ship",
        }
    }

    pub fn from_id(id: &str) -> Option<LayerKind> {
        LayerKind::ALL.into_iter().find(|k| k.id() == id)
    }

    pub fn zoom_range(self) -> (f64, f64) {
        match self {
            LayerKind::Weather => (3.0, 13.0),
            LayerKind::Amdb => (13.0, 99.0),
            LayerKind::Airways => (8.0, 13.0),
            LayerKind::Airports => (4.0, 13.5),
            LayerKind::Vors => (6.0, 13.0),
            LayerKind::Ndbs => (8.0, 13.0),
            LayerKind::Route => (0.0, 99.0),
            LayerKind::AiTraffic => (6.0, 99.0),
            LayerKind::OnlineTraffic => (4.0, 99.0),
            LayerKind::OwnShip => (0.0, 99.0),
        }
    }

    pub fn label_min_zoom(self) -> f64 {
        match self {
            LayerKind::Airports => 6.0,
            LayerKind::Vors => 7.0,
            LayerKind::Ndbs => 9.0,
            LayerKind::Airways => 9.0,
            LayerKind::AiTraffic | LayerKind::OnlineTraffic => 7.0,
            LayerKind::Amdb => 14.5,
            _ => 0.0,
        }
    }

    pub fn user_toggle(self) -> bool {
        self != LayerKind::OwnShip
    }
}

pub fn airport_min_zoom(size: u8) -> f64 {
    match size {
        2.. => 4.0,
        1 => 6.0,
        0 => 8.0,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LayerToggles {
    pub off: Vec<LayerKind>,
}

impl LayerToggles {
    pub fn is_on(&self, k: LayerKind) -> bool {
        !k.user_toggle() || !self.off.contains(&k)
    }

    pub fn flip(&mut self, k: LayerKind) {
        if !k.user_toggle() {
            return;
        }
        if let Some(i) = self.off.iter().position(|o| *o == k) {
            self.off.remove(i);
        } else {
            self.off.push(k);
            self.off.sort();
        }
    }

    pub fn visible(&self, k: LayerKind, zoom: f64) -> bool {
        let (lo, hi) = k.zoom_range();
        self.is_on(k) && zoom >= lo && zoom < hi
    }

    pub fn labels(&self, k: LayerKind, zoom: f64) -> bool {
        self.visible(k, zoom) && zoom >= k.label_min_zoom()
    }
}

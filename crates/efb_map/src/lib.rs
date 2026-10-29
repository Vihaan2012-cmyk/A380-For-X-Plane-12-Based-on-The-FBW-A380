pub mod amdb;
pub mod controller;
pub mod declutter;
pub mod layers;
pub mod projection;
pub mod scene;
pub mod traffic;
pub mod weather;

pub use controller::MapController;
pub use layers::{LayerKind, LayerToggles};
pub use mfd_tree::Shape;
pub use projection::{LatLon, TileKey, Viewport};
pub use scene::{build_scene, Scene, SceneInput};
pub use traffic::{TrafficReport, TrafficSource, TrafficStore};

pub mod palette {
    pub const OK: &str = "#3ccf6e";
    pub const WARN: &str = "#f0b429";
    pub const BAD: &str = "#e5484d";
    pub const GREY: &str = "#9a9a9a";
    pub const DIM: &str = "#5a5a5a";
    pub const TEXT: &str = "#f2f2f2";
    pub const BG: &str = "#242424";
    pub const FIELD: &str = "#1c1c1c";
    pub const CARD: &str = "#2a2c30";
    pub const CTL: &str = "#3a3c40";
    pub const ACCENT: &str = "#3fb0ff";
}

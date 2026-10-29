pub use crate::btv::{NavDb, Runway};
pub use crate::control_panel::{AmdbAirport, AmdbSource, MapDataFeature};

pub trait OansNavDb: NavDb + AmdbSource {}

impl<T: NavDb + AmdbSource> OansNavDb for T {}

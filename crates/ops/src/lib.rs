//! Ground and airline operations for the A380: full-zone weight and balance,
//! CG in % MAC and the THS for it, boarding and refuel schedulers, and a
//! simulated airline ops centre for flights off-network. Pure logic, no
//! simulator dependency.

pub mod aircraft;
pub mod aoc;
pub mod boarding;
pub mod loadsheet;
pub mod refuel;
pub mod settings;
pub mod trim;

pub use aircraft::{Deck, Door, Hold, PaxZone, Tank};
pub use loadsheet::{Load, Loadsheet};
pub use settings::{OpsSettings, Units};

pub mod command;
pub mod convert;
pub mod core;
pub mod efis;
pub mod facilities;
pub mod guidance;
pub mod host;
pub mod inputs;
pub mod navaids;
pub mod outputs;
pub mod pwp;
pub mod runway;
pub mod symbols;
pub mod vnav;

pub use command::{Envelope, HostCommand};
pub use core::{FmsCore, Snapshot};
pub use host::FmsHost;
pub use inputs::Inputs;
pub use outputs::{Outputs, Reply};

pub mod modes {
    pub const LAT_NONE: i32 = 0;
    pub const LAT_HDG: i32 = 10;
    pub const LAT_TRACK: i32 = 11;
    pub const LAT_NAV: i32 = 20;
    pub const LAT_RWY_TRACK: i32 = 41;
    pub const LAT_GA_TRACK: i32 = 50;
    pub const ARMED_LAT_NAV: u32 = 0;

    pub fn is_armed(bits: u32, mode: u32) -> bool {
        (bits >> mode) & 1 == 1
    }
}

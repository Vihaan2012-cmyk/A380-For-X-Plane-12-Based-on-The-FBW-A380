use fms::navdb::Location;
use fms_lnav::Coord;

pub mod vhf_type {
    pub const UNKNOWN: u32 = 1 << 0;
    pub const VOR: u32 = 1 << 1;
    pub const VOR_DME: u32 = 1 << 2;
    pub const DME: u32 = 1 << 3;
    pub const TACAN: u32 = 1 << 4;
    pub const VORTAC: u32 = 1 << 5;
    pub const VOT: u32 = 1 << 6;
    pub const ILS_DME: u32 = 1 << 7;
    pub const ILS_TACAN: u32 = 1 << 8;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NearbyKind {
    Airport,
    NdbNavaid,
    VhfNavaid,
    Waypoint,
}

/// FBW `NearbyFacility`: what a nearby-facility monitor reports.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NearbyFacility {
    pub database_id: String,
    pub ident: String,
    pub location: Location,
    pub section_code: i32,
    pub vhf_type: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VhfNavaid {
    pub database_id: String,
    pub ident: String,
    pub location: Location,
    pub dme_location: Option<Location>,
    pub frequency: f64,
    pub figure_of_merit: i32,
    pub vhf_type: u32,
}

impl VhfNavaid {
    pub fn dme_or_location(&self) -> Coord {
        let l = self.dme_location.unwrap_or(self.location);
        Coord::new(l.lat, l.long)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NdbNavaid {
    pub database_id: String,
    pub ident: String,
    pub location: Location,
    pub frequency: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IlsNavaid {
    pub database_id: String,
    pub ident: String,
    pub frequency: f64,
    pub loc_bearing: f64,
    pub gs_slope: Option<f64>,
    pub is_localizer_glide_slope: bool,
}

/// Runway fields the plan's runway record does not carry (msfs-navdata `Runway`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunwayDetails {
    pub gradient: f64,
    pub threshold_crossing_height: f64,
    pub width: f64,
    pub ls_ident: Option<String>,
}

/// The facility searches FBW does through the MSFS facility loader. The plan database
/// (`fms::navdb::NavDatabase`) has no spatial queries, so the host takes them separately.
pub trait Facilities: Send + Sync {
    fn nearby(&self, _kind: NearbyKind, _centre: Coord, _radius_nm: f64, _max: usize) -> Vec<NearbyFacility> {
        Vec::new()
    }
    fn vhf_navaid(&self, _database_id: &str) -> Option<VhfNavaid> {
        None
    }
    fn ndb_navaid(&self, _database_id: &str) -> Option<NdbNavaid> {
        None
    }
    fn ils_at_airport(&self, _airport_ident: &str, _ils_ident: &str) -> Option<IlsNavaid> {
        None
    }
    fn ils(&self, _database_id: &str) -> Option<IlsNavaid> {
        None
    }
    fn runway_details(&self, _airport_ident: &str, _runway_ident: &str) -> Option<RunwayDetails> {
        None
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoFacilities;

impl Facilities for NoFacilities {}

/// FBW `UpdateThrottler` without the random refresh offset.
#[derive(Clone, Debug)]
pub struct UpdateThrottler {
    interval_ms: f64,
    current: f64,
    last_update: f64,
    refresh_number: i64,
}

impl UpdateThrottler {
    pub fn new(interval_ms: f64) -> Self {
        UpdateThrottler { interval_ms, current: 0., last_update: 0., refresh_number: 0 }
    }

    pub fn can_update(&mut self, dt_ms: f64, force: bool) -> Option<f64> {
        self.current += dt_ms;
        let number = (self.current / self.interval_ms).floor() as i64;
        let update = number > self.refresh_number;
        self.refresh_number = number;
        if update || force {
            let acc = self.current - self.last_update;
            self.last_update = self.current;
            Some(acc)
        } else {
            None
        }
    }
}

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ffi::CString;
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::Value as Json;
use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::doors::{Doors, Request};
use crate::published::{self, Command, Published, Value};
use crate::xp::{self, DataRef, Xplm};
use crate::Vars;

pub const VEHICLES: [&str; 19] = [
    "gpu_tld409",
    "asu_unit",
    "pca_unit",
    "tow_tpx500",
    "fuel_hydrant",
    "fuel_tanker",
    "water_truck",
    "lav_truck",
    "clean_van",
    "deice_truck",
    "bag_tractor",
    "bulk_cart",
    "belt_loader",
    "ldcl_loader",
    "uld_set",
    "stairs_main",
    "stairs_upper",
    "cat_highloader",
    "udcat_highloader",
];

pub const CREW: &str = "ground_crew";
const CREW_DATAREFS: [&str; 4] = ["opensam/dgs/status", "opensam/dgs/track", "opensam/dgs/distance", "opensam/dgs/lr"];
const WINGTIP: [f64; 3] = [39.93, 2.19, 14.88];

pub const OPERATORS: [&str; 3] = ["dnata", "swissport", "menzies"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Service {
    Gpu,
    Pca,
    Asu,
    Fuel,
    Catering,
    Baggage,
    Water,
    Lav,
    Clean,
    Deice,
    Stairs,
    Tug,
}

impl Service {
    pub const ALL: [Service; 12] = [
        Service::Gpu,
        Service::Pca,
        Service::Asu,
        Service::Fuel,
        Service::Catering,
        Service::Baggage,
        Service::Water,
        Service::Lav,
        Service::Clean,
        Service::Deice,
        Service::Stairs,
        Service::Tug,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Service::Gpu => "gpu",
            Service::Pca => "pca",
            Service::Asu => "asu",
            Service::Fuel => "fuel",
            Service::Catering => "catering",
            Service::Baggage => "baggage",
            Service::Water => "water",
            Service::Lav => "lav",
            Service::Clean => "clean",
            Service::Deice => "deice",
            Service::Stairs => "stairs",
            Service::Tug => "tug",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Service::Gpu => "Ground power",
            Service::Pca => "Preconditioned air",
            Service::Asu => "Air start unit",
            Service::Fuel => "Refuelling",
            Service::Catering => "Catering",
            Service::Baggage => "Baggage",
            Service::Water => "Potable water",
            Service::Lav => "Lavatory service",
            Service::Clean => "Cabin cleaning",
            Service::Deice => "De-icing",
            Service::Stairs => "Stairs",
            Service::Tug => "Pushback tug",
        }
    }

    fn toggled_by_crew(self) -> bool {
        !matches!(self, Service::Gpu | Service::Tug)
    }
}

static REQUESTS: Mutex<Vec<Service>> = Mutex::new(Vec::new());

pub fn request(service: Service) {
    if let Ok(mut r) = REQUESTS.lock() {
        r.push(service);
    }
}

fn take_requests() -> Vec<Service> {
    REQUESTS.lock().map(|mut r| std::mem::take(&mut *r)).unwrap_or_default()
}

pub const NOSE_Z: f64 = -35.19;
pub const NOSE_GEAR_Z: f64 = -30.22;
pub const GPU_RECEPTACLE: [f64; 3] = [0.45, 3.15, NOSE_Z + 5.99];
pub const LP_CONNECTION: [f64; 3] = [-4.0, 2.07, -8.0];
pub const HP_CONNECTION: [f64; 3] = [4.0, 1.8, -5.0];
pub const REFUEL_COUPLING_LEFT: [f64; 3] = [-19.69, 6.10, -6.10];
pub const POTABLE_WATER_PANEL: [f64; 3] = [1.5, 2.8, 24.0];
pub const WASTE_PANEL: [f64; 3] = [-1.2, 2.8, 26.5];
pub const DEICE_SPOT: [f64; 3] = [-16.0, 0.0, -24.0];
pub const BULK_DOOR: [f64; 3] = [3.25, -1.33, 15.92];

pub const M1L: usize = 0;
pub const M1R: usize = 1;
pub const M2L: usize = 2;
pub const M2R: usize = 3;
pub const M4R: usize = 7;
pub const U1L: usize = 10;
pub const U1R: usize = 11;
pub const CARGO_FWD: usize = 16;
pub const CARGO_AFT: usize = 17;
pub const FUEL_HOSE: usize = 18;

pub const SILLS: [[f64; 3]; 18] = [
    [-3.052, 1.0005, -28.853],
    [3.052, 1.0005, -28.853],
    [-3.579, 1.0005, -18.650],
    [3.579, 1.0005, -18.650],
    [-3.71, 0.92, -2.435],
    [3.71, 0.92, -2.435],
    [-3.57, 1.0, 9.66],
    [3.57, 1.0, 9.66],
    [-3.35, 1.0, 18.60],
    [3.35, 1.0, 18.60],
    [-3.377, 3.74, -14.198],
    [3.377, 3.74, -14.198],
    [-3.363, 3.74, 5.205],
    [3.363, 3.74, 5.205],
    [-3.365, 3.74, 14.101],
    [3.365, 3.74, 14.101],
    [3.40, -1.29, -23.45],
    [3.37, -1.30, 13.03],
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    Door(usize),
    Body([f64; 3]),
    Ground([f64; 3]),
    NoseGear,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Heading {
    Fixed(f64),
    Towards,
    Steer,
}

#[derive(Debug)]
pub struct Placement {
    pub id: &'static str,
    pub vehicle: &'static str,
    pub service: Service,
    pub anchor: Anchor,
    pub offset: [f64; 2],
    pub heading: Heading,
    pub attach: &'static [&'static str],
    pub gap: f64,
    pub lift: &'static [&'static str],
    pub lift_offset: f64,
    pub door: Option<usize>,
}

const DOCK_STAIRS: &[&str] = &["platform_front_edge", "platform_front", "platform_edge", "platform", "bumper"];
const DOCK_BOX: &[&str] = &["platform_front_edge", "box_front", "bridge_front", "bridge_edge", "bridge", "box"];
const DOCK_LOADER: &[&str] = &["platform_front_edge", "bridge_lip_front_min", "bridge_deck_front_edge_min", "platform_front", "bridge_front", "front_platform", "platform_edge", "platform"];
const DOCK_BELT: &[&str] = &["belt_tip_top_min", "tip_bumper_front_min", "belt_top_edge", "belt_top", "belt_end", "boom_end", "top"];
const HOLD: &[&str] = &["rear_platform", "curtain", "cradle", "plug_dz"];
const DOCK_TUG: &[&str] = &["cradle_centre", "cradle", "nosewheel", "nose_wheel", "wheel_cradle"];

const fn park(id: &'static str, vehicle: &'static str, service: Service, anchor: Anchor, offset: [f64; 2], heading: Heading) -> Placement {
    Placement { id, vehicle, service, anchor, offset, heading, attach: &[], gap: 0., lift: &[], lift_offset: 0., door: None }
}

const fn dock(
    id: &'static str,
    vehicle: &'static str,
    service: Service,
    door: usize,
    heading: f64,
    attach: &'static [&'static str],
    lift: &'static [&'static str],
    opens: bool,
) -> Placement {
    Placement {
        id,
        vehicle,
        service,
        anchor: Anchor::Door(door),
        offset: [0., 0.],
        heading: Heading::Fixed(heading),
        attach,
        gap: 0.05,
        lift,
        lift_offset: 0.,
        door: if opens { Some(door) } else { None },
    }
}

pub static PLACEMENTS: [Placement; 30] = [
    Placement {
        id: "gpu",
        vehicle: "gpu_tld409",
        service: Service::Gpu,
        anchor: Anchor::Ground(GPU_RECEPTACLE),
        offset: [0., 0.],
        heading: Heading::Fixed(180.),
        attach: &["plug_tip_0", "plug_tip", "plug"],
        gap: 0.,
        lift: &[],
        lift_offset: 0.,
        door: None,
    },
    Placement {
        attach: &["hose_nozzle_tip_left", "hose_nozzle_tip"],
        ..park("pca", "pca_unit", Service::Pca, Anchor::Ground(LP_CONNECTION), [0., 0.], Heading::Fixed(90.))
    },
    Placement {
        attach: &["aircraft_connector_target", "hose_nozzle_tip"],
        ..park("asu", "asu_unit", Service::Asu, Anchor::Ground(HP_CONNECTION), [0., 0.], Heading::Fixed(-90.))
    },
    Placement {
        id: "fuel_hydrant",
        vehicle: "fuel_hydrant",
        service: Service::Fuel,
        anchor: Anchor::Ground(REFUEL_COUPLING_LEFT),
        offset: [0.0, -3.5],
        heading: Heading::Towards,
        attach: &[],
        gap: 0.,
        lift: &["platform", "lift", "deck"],
        lift_offset: -1.2,
        door: None,
    },
    Placement {
        id: "fuel_tanker",
        vehicle: "fuel_tanker",
        service: Service::Fuel,
        anchor: Anchor::Ground(REFUEL_COUPLING_LEFT),
        offset: [-3.0, -5.0],
        heading: Heading::Towards,
        attach: &[],
        gap: 0.,
        lift: &["deck", "platform", "lift"],
        lift_offset: -1.2,
        door: None,
    },
    dock("cat_m2r", "cat_highloader", Service::Catering, M2R, -90., DOCK_BOX, &["lift"], true),
    dock("udcat_u1r", "udcat_highloader", Service::Catering, U1R, -90., DOCK_BOX, &["lift"], true),
    dock("ldcl_fwd", "ldcl_loader", Service::Baggage, CARGO_FWD, -90., DOCK_LOADER, &["lift", "platform_lift", "bridge_lift", "bridge_platform"], true),
    dock("ldcl_aft", "ldcl_loader", Service::Baggage, CARGO_AFT, -90., DOCK_LOADER, &["lift", "platform_lift", "bridge_lift", "bridge_platform"], true),
    park("uld_fwd", "uld_set", Service::Baggage, Anchor::Door(CARGO_FWD), [11.5, 2.0], Heading::Fixed(0.)),
    park("uld_aft", "uld_set", Service::Baggage, Anchor::Door(CARGO_AFT), [11.5, -2.0], Heading::Fixed(0.)),
    Placement {
        id: "belt_bulk",
        vehicle: "belt_loader",
        service: Service::Baggage,
        anchor: Anchor::Body(BULK_DOOR),
        offset: [0., 0.],
        heading: Heading::Fixed(-90.),
        attach: DOCK_BELT,
        gap: 0.1,
        lift: &["boom", "lift"],
        lift_offset: 0.,
        door: None,
    },
    park("bag_tractor", "bag_tractor", Service::Baggage, Anchor::Body(BULK_DOOR), [9.0, 3.5], Heading::Fixed(0.)),
    park("bulk_cart", "bulk_cart", Service::Baggage, Anchor::Body(BULK_DOOR), [9.0, 7.5], Heading::Fixed(0.)),
    park("water", "water_truck", Service::Water, Anchor::Ground(POTABLE_WATER_PANEL), [5.0, 0.0], Heading::Fixed(-90.)),
    park("lav", "lav_truck", Service::Lav, Anchor::Ground(WASTE_PANEL), [-5.0, 0.0], Heading::Fixed(90.)),
    park("clean", "clean_van", Service::Clean, Anchor::Door(M1R), [7.0, 1.0], Heading::Fixed(0.)),
    park("deice", "deice_truck", Service::Deice, Anchor::Ground(DEICE_SPOT), [0.0, 0.0], Heading::Fixed(0.)),
    dock("stairs_m1l", "stairs_main", Service::Stairs, M1L, 90., DOCK_STAIRS, &["lift"], true),
    dock("stairs_m2l", "stairs_main", Service::Stairs, M2L, 90., DOCK_STAIRS, &["lift"], true),
    dock("stairs_u1l", "stairs_upper", Service::Stairs, U1L, 120., DOCK_STAIRS, &["lift"], true),
    Placement {
        id: "tug",
        vehicle: "tow_tpx500",
        service: Service::Tug,
        anchor: Anchor::NoseGear,
        offset: [0., 0.],
        heading: Heading::Steer,
        attach: DOCK_TUG,
        gap: 0.,
        lift: &[],
        lift_offset: 0.,
        door: None,
    },
    park("tug_standby", "tow_tpx500", Service::Tug, Anchor::NoseGear, [0.0, -9.0], Heading::Fixed(180.)),
    park("crew_headset", CREW, Service::Tug, Anchor::NoseGear, [-3.5, 0.5], Heading::Fixed(-90.)),
    park("crew_wing_l", CREW, Service::Tug, Anchor::Ground([-WINGTIP[0], 0., WINGTIP[2]]), [-2.0, -1.0], Heading::Fixed(-90.)),
    park("crew_wing_r", CREW, Service::Tug, Anchor::Ground(WINGTIP), [2.0, -1.0], Heading::Fixed(90.)),
    park("crew_bag_fwd", CREW, Service::Baggage, Anchor::Door(CARGO_FWD), [6.0, 4.0], Heading::Fixed(180.)),
    park("crew_bag_aft", CREW, Service::Baggage, Anchor::Door(CARGO_AFT), [6.0, -4.0], Heading::Fixed(0.)),
    park("crew_bulk", CREW, Service::Baggage, Anchor::Body(BULK_DOOR), [4.0, 3.0], Heading::Fixed(180.)),
    park("crew_fuel", CREW, Service::Fuel, Anchor::Ground(REFUEL_COUPLING_LEFT), [-1.5, 2.5], Heading::Fixed(180.)),
];

/// Every vehicle position the crew can call or send away on its own: not
/// the tug (its position follows the pushback) and not the ground crew,
/// who come with their vehicles.
pub fn per_place(p: &Placement) -> bool {
    p.service != Service::Tug && p.vehicle != CREW
}

pub fn placement(id: &str) -> Option<&'static Placement> {
    PLACEMENTS.iter().find(|p| p.id == id)
}

struct Fallback {
    vehicle: &'static str,
    attach: [f64; 3],
    lift_travel: Option<f64>,
    reach: f64,
}

const FALLBACKS: [Fallback; 8] = [
    Fallback { vehicle: "gpu_tld409", attach: [3.55, 3.15, 1.168], lift_travel: None, reach: 0. },
    Fallback { vehicle: "stairs_main", attach: [0., 4.9, 0.], lift_travel: Some(0.7), reach: 0.35 },
    Fallback { vehicle: "stairs_upper", attach: [0., 7.7, 0.], lift_travel: Some(0.6), reach: 0.45 },
    Fallback { vehicle: "cat_highloader", attach: [0., 1.8, 0.], lift_travel: Some(3.8), reach: 1.3 },
    Fallback { vehicle: "udcat_highloader", attach: [0., 1.9, 0.], lift_travel: Some(6.4), reach: 0.9 },
    Fallback { vehicle: "tow_tpx500", attach: [0., 0., 2.5], lift_travel: None, reach: 0. },
    Fallback { vehicle: "ldcl_loader", attach: [0., 0.6, -4.0], lift_travel: None, reach: 0. },
    Fallback { vehicle: "belt_loader", attach: [0., 1.0, -4.0], lift_travel: None, reach: 0. },
];

fn fallback(vehicle: &str) -> Option<&'static Fallback> {
    FALLBACKS.iter().find(|f| f.vehicle == vehicle)
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimSpec {
    pub dataref: String,
    pub min: f64,
    pub max: f64,
    pub travel: Option<[f64; 3]>,
    pub role: Option<String>,
    pub period: Option<f64>,
    pub radius: Option<f64>,
}

impl AnimSpec {
    pub fn part(&self) -> &str {
        self.dataref.rsplit('/').next().unwrap_or(&self.dataref)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Manifest {
    pub length: Option<f64>,
    pub anims: Vec<AnimSpec>,
    pub attach: BTreeMap<String, [f64; 3]>,
    pub objects: BTreeMap<String, String>,
}

fn number(v: &Json) -> Option<f64> {
    match v {
        Json::Number(n) => n.as_f64(),
        Json::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn first_number(o: &serde_json::Map<String, Json>, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|k| o.get(*k).and_then(number))
}

fn vector(v: &Json) -> Option<[f64; 3]> {
    match v {
        Json::Array(a) if a.len() >= 3 => Some([number(&a[0])?, number(&a[1])?, number(&a[2])?]),
        Json::Object(o) => {
            if let Some(inner) = ["position", "pos", "xyz", "point", "at"].iter().find_map(|k| o.get(*k)) {
                return vector(inner);
            }
            Some([first_number(o, &["x"])?, first_number(o, &["y"])?, first_number(o, &["z"])?])
        }
        _ => None,
    }
}

fn metres(v: [f64; 3], mm: bool) -> [f64; 3] {
    if mm {
        v.map(|c| c / 1000.)
    } else {
        v
    }
}

fn blender_to_obj(v: [f64; 3]) -> [f64; 3] {
    [v[0], v[2], -v[1]]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn height_range(o: &serde_json::Map<String, Json>) -> Option<f64> {
    o.iter().find_map(|(k, v)| {
        let stem = k.strip_suffix("_at_min_m")?;
        let lo = number(v)?;
        let hi = number(o.get(&format!("{stem}_at_max_m"))?)?;
        Some(hi - lo)
    })
}

fn travel_from_text(text: &str) -> Option<[f64; 3]> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let k = words.iter().position(|w| *w == "m" || *w == "m," || *w == "m.")?;
    let n: f64 = words.get(k.checked_sub(1)?)?.parse().ok()?;
    if text.contains("-Z") {
        Some([0., 0., -n])
    } else if text.contains("towards the rear") || text.contains("+Z") {
        Some([0., 0., n])
    } else if text.contains(" up") {
        Some([0., n, 0.])
    } else {
        None
    }
}

fn anim_from(vehicle: &str, key: Option<&str>, v: &Json, mm: bool) -> Option<AnimSpec> {
    let o = v.as_object();
    let first_step = o.and_then(|o| o.get("steps")).and_then(Json::as_array).and_then(|s| s.first()).and_then(Json::as_object);
    let name = [o, first_step]
        .into_iter()
        .flatten()
        .find_map(|m| ["dataref", "name", "part"].iter().find_map(|k| m.get(*k).and_then(Json::as_str)))
        .or(key)
        .or_else(|| v.as_str())?
        .to_string();
    let dataref = if name.contains('/') { name } else { format!("fbw/gse/{vehicle}/{name}") };
    let (min, max, travel) = match o {
        Some(o) => {
            let src = first_step.unwrap_or(o);
            let min = first_number(o, &["min", "v0", "from"]).or_else(|| first_number(src, &["v0"])).unwrap_or(0.);
            let max = first_number(o, &["max", "v1", "to"]).or_else(|| first_number(src, &["v1"])).unwrap_or(1.);
            let travel = if let Some(t) = o.get("travel").or_else(|| o.get("travel_m")).or_else(|| o.get("offset")) {
                match t {
                    Json::Number(_) | Json::String(_) => number(t).map(|n| [0., n, 0.]),
                    _ => vector(t),
                }
                .map(|t| metres(t, mm))
            } else if let (Some(a), Some(b)) = (src.get("offset0").and_then(vector), src.get("offset1").and_then(vector)) {
                Some(blender_to_obj(sub(b, a)))
            } else if let Some(h) = height_range(o) {
                Some([0., h, 0.])
            } else {
                ["meaning", "motion", "how"].iter().find_map(|k| o.get(*k).and_then(Json::as_str)).and_then(travel_from_text)
            };
            (min, max, travel)
        }
        None => (0., 1., None),
    };
    let role = o.and_then(|o| o.get("role")).and_then(Json::as_str).map(str::to_string);
    let period = o.and_then(|o| first_number(o, &["period_s", "period"]));
    let radius = o.and_then(|o| first_number(o, &["radius_m", "radius"]));
    Some(AnimSpec { dataref, min, max, travel, role, period, radius })
}

pub fn parse_manifest(vehicle: &str, text: &str) -> Option<Manifest> {
    let root: Json = serde_json::from_str(text).ok()?;
    let o = root.as_object()?;
    let mm = o.get("units").and_then(Json::as_str).is_some_and(|u| u.starts_with("mm") || u.starts_with("milli"));
    let mut m = Manifest::default();
    m.length = first_number(o, &["length_m", "length"]);
    if let Some(objects) = o.get("objects").and_then(Json::as_object) {
        for (k, v) in objects {
            if let Some(name) = v.as_str() {
                m.objects.insert(k.to_ascii_lowercase(), name.to_string());
            }
        }
    }
    if let Some(d) = ["dimensions_m", "dimensions", "size", "bbox"].iter().find_map(|k| o.get(*k)) {
        let l = match d {
            Json::Object(d) => first_number(d, &["length", "length_m", "l"]),
            Json::Array(a) => a.first().and_then(number),
            _ => None,
        };
        m.length = m.length.or(l);
    }
    if let Some(l) = m.length {
        if mm || l > 100. {
            m.length = Some(l / 1000.);
        }
    }
    if let Some(a) = ["anims", "anim", "animations", "datarefs"].iter().find_map(|k| o.get(*k)) {
        match a {
            Json::Array(list) => m.anims.extend(list.iter().filter_map(|v| anim_from(vehicle, None, v, mm))),
            Json::Object(map) => m.anims.extend(map.iter().filter_map(|(k, v)| anim_from(vehicle, Some(k.as_str()), v, mm))),
            _ => {}
        }
    }
    let mut seen = HashSet::new();
    m.anims.retain(|a| seen.insert(a.dataref.clone()));
    if let Some(a) = ["attach", "attach_points", "attachments", "points"].iter().find_map(|k| o.get(*k)) {
        match a {
            Json::Object(map) => {
                for (k, v) in map {
                    if let Some(p) = vector(v) {
                        m.attach.insert(k.to_ascii_lowercase(), p);
                    }
                }
            }
            Json::Array(list) => {
                for v in list {
                    let name = v.get("name").and_then(Json::as_str);
                    if let (Some(name), Some(p)) = (name, vector(v)) {
                        m.attach.insert(name.to_ascii_lowercase(), p);
                    }
                }
            }
            _ => {}
        }
    }
    let large = m.attach.values().any(|p| p.iter().any(|c| c.abs() > 100.));
    if mm || large {
        for p in m.attach.values_mut() {
            *p = metres(*p, true);
        }
    }
    Some(m)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Lift,
    Extend,
    Hold,
    Loop,
    Progress,
    Wheel,
    Blink,
}

fn declared_role(a: &AnimSpec) -> Option<Role> {
    match a.role.as_deref()? {
        "loop" => Some(Role::Loop),
        "progress" => Some(Role::Progress),
        "wheel" => Some(Role::Wheel),
        "blink" => Some(Role::Blink),
        "lift" => Some(Role::Lift),
        "extend" => Some(Role::Extend),
        "hold" => Some(Role::Hold),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    pub attach: [f64; 3],
    pub anims: Vec<(AnimSpec, Role)>,
    pub lift_travel: Option<f64>,
    pub reach: f64,
}

pub fn geometry(p: &Placement, manifest: Option<&Manifest>) -> Geometry {
    let fb = fallback(p.vehicle);
    let anims: Vec<(AnimSpec, Role)> = manifest
        .map(|m| {
            m.anims
                .iter()
                .map(|a| {
                    let role = if let Some(r) = declared_role(a) {
                        r
                    } else if p.lift.contains(&a.part()) {
                        Role::Lift
                    } else if HOLD.contains(&a.part()) {
                        Role::Hold
                    } else {
                        Role::Extend
                    };
                    (a.clone(), role)
                })
                .collect()
        })
        .unwrap_or_default();
    let named = p.attach.iter().find_map(|n| manifest.and_then(|m| m.attach.get(*n)).map(|a| (*n, *a)));
    let raised = named.and_then(|(n, _)| n.strip_suffix("_min")).and_then(|stem| manifest.and_then(|m| m.attach.get(&format!("{stem}_max"))).copied());
    let attach = if p.attach.is_empty() {
        [0., 0., 0.]
    } else if let Some((_, a)) = named {
        match raised {
            Some(r) => [r[0], a[1], r[2]],
            None => a,
        }
    } else if let Some(f) = fb {
        f.attach
    } else {
        let length = manifest.and_then(|m| m.length).unwrap_or(6.0);
        [0., 0., -length / 2.]
    };
    let lift = anims.iter().find(|(_, r)| *r == Role::Lift);
    let lift_travel = match lift {
        Some((a, _)) => a
            .travel
            .map(|t| t[1])
            .filter(|t| t.abs() > 1e-6)
            .or_else(|| raised.zip(named).map(|(r, (_, a))| r[1] - a[1]).filter(|t| t.abs() > 1e-6))
            .or_else(|| fb.and_then(|f| f.lift_travel)),
        None => None,
    };
    let manifest_reach: Option<f64> = {
        let known: Vec<f64> = anims.iter().filter(|(_, r)| *r == Role::Extend).filter_map(|(a, _)| a.travel.map(|t| -t[2])).collect();
        (!known.is_empty()).then(|| known.iter().filter(|r| **r > 0.).sum())
    };
    let has_extend = anims.iter().any(|(_, r)| *r == Role::Extend);
    let reach = if p.attach.is_empty() {
        0.
    } else {
        manifest_reach.unwrap_or(if has_extend || manifest.is_none() { fb.map_or(0., |f| f.reach) } else { 0. })
    };
    Geometry { attach, anims, lift_travel, reach }
}

pub fn lift_value(min: f64, max: f64, attach_height: f64, travel: Option<f64>, target_height: f64) -> f64 {
    match travel {
        Some(t) if t.abs() > 1e-6 => min + ((target_height - attach_height) / t).clamp(0., 1.) * (max - min),
        _ => max,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub position: [f64; 3],
    pub psi: f64,
    pub theta: f64,
    pub phi: f64,
}

impl Pose {
    pub fn rotate(&self, b: [f64; 3]) -> [f64; 3] {
        let [x, y, z] = b;
        let c = -self.phi.to_radians();
        let (x, y) = (x * c.cos() - y * c.sin(), x * c.sin() + y * c.cos());
        let t = self.theta.to_radians();
        let (y, z) = (y * t.cos() - z * t.sin(), y * t.sin() + z * t.cos());
        let a = -self.psi.to_radians();
        let (x, z) = (x * a.cos() + z * a.sin(), -x * a.sin() + z * a.cos());
        [x, y, z]
    }

    pub fn to_world(&self, cg_reference: [f64; 3], acf: [f64; 3]) -> [f64; 3] {
        let r = self.rotate(sub(acf, cg_reference));
        [self.position[0] + r[0], self.position[1] + r[1], self.position[2] + r[2]]
    }

    fn moved_from(&self, other: &Pose) -> bool {
        (0..3).any(|k| (self.position[k] - other.position[k]).abs() > 0.02)
            || angle_diff(self.psi, other.psi).abs() > 0.05
            || (self.theta - other.theta).abs() > 0.05
    }
}

pub fn yaw(heading_deg: f64, v: [f64; 2]) -> [f64; 2] {
    let a = -heading_deg.to_radians();
    [v[0] * a.cos() + v[1] * a.sin(), -v[0] * a.sin() + v[1] * a.cos()]
}

pub fn heading_of(dx: f64, dz: f64) -> f64 {
    dx.atan2(-dz).to_degrees()
}

fn angle_diff(a: f64, b: f64) -> f64 {
    (a - b + 540.).rem_euclid(360.) - 180.
}

pub fn wrap360(h: f64) -> f64 {
    h.rem_euclid(360.)
}

fn anchor_body(anchor: Anchor) -> ([f64; 3], bool) {
    match anchor {
        Anchor::Door(d) => (SILLS.get(d).copied().unwrap_or([0.; 3]), true),
        Anchor::Body(p) => (p, true),
        Anchor::Ground(p) => ([p[0], 0., p[2]], false),
        Anchor::NoseGear => ([0., 0., NOSE_GEAR_Z], false),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub origin: [f64; 3],
    pub heading: f64,
    pub target_height: f64,
}

pub fn place(p: &Placement, g: &Geometry, pose: &Pose, cg_reference: [f64; 3], steer_deg: f64, ground: &mut dyn FnMut(f64, f64, f64) -> Option<f64>) -> Placed {
    let (body, on_body) = anchor_body(p.anchor);
    let anchor_world = pose.to_world(cg_reference, body);
    let shifted = {
        let o = yaw(pose.psi, [p.offset[0], p.offset[1]]);
        [anchor_world[0] + o[0], anchor_world[2] + o[1]]
    };
    let heading = match p.heading {
        Heading::Fixed(h) => wrap360(pose.psi + h),
        Heading::Steer => wrap360(pose.psi + steer_deg),
        Heading::Towards => {
            if p.offset == [0., 0.] {
                wrap360(pose.psi)
            } else {
                let rel = heading_of(-p.offset[0], -p.offset[1]);
                wrap360(pose.psi + rel)
            }
        }
    };
    let origin_xz = if p.attach.is_empty() {
        shifted
    } else {
        let fwd = yaw(heading, [0., -1.]);
        let dock = [shifted[0] - fwd[0] * p.gap, shifted[1] - fwd[1] * p.gap];
        let a = yaw(heading, [g.attach[0], g.attach[2] - g.reach]);
        [dock[0] - a[0], dock[1] - a[1]]
    };
    let ground_y = ground(origin_xz[0], pose.position[1], origin_xz[1]).unwrap_or(pose.position[1] - 5.0);
    let anchor_height = match p.anchor {
        Anchor::Ground(h) => h[1],
        _ if on_body => anchor_world[1] - ground_y,
        _ => 0.,
    };
    Placed { origin: [origin_xz[0], ground_y, origin_xz[1]], heading, target_height: anchor_height + p.lift_offset }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Inputs {
    pub on_ground: bool,
    pub stationary: bool,
    pub park_brake: bool,
    pub chocks: bool,
    pub engines_running: bool,
    pub apu_running: bool,
    pub beacon: bool,
    pub ext_power: bool,
    pub refuelling: bool,
    pub boarding: bool,
    pub tug_attached: bool,
    pub tug_called: bool,
    pub jetway: [bool; 3],
    pub door_open: [bool; 3],
    pub hydrant: bool,
    pub on_runway: bool,
    pub runways_unknown: bool,
}

pub const STAIR_DOORS: [usize; 3] = [M1L, M2L, U1L];

pub fn may_enter(service: Service, i: &Inputs) -> bool {
    if i.on_runway || i.runways_unknown {
        return false;
    }
    let parked = i.on_ground && i.stationary && (i.park_brake || i.chocks);
    match service {
        Service::Tug => i.on_ground && i.stationary,
        Service::Gpu => parked && !i.engines_running,
        _ => parked && !i.engines_running && !i.beacon,
    }
}

pub fn refusal(service: Service, i: &Inputs) -> Option<&'static str> {
    if may_enter(service, i) {
        return None;
    }
    Some(if i.runways_unknown {
        "not yet: the airport's runways are still loading"
    } else if i.on_runway {
        "vehicles never drive onto a runway; taxi to a stand first"
    } else if !i.on_ground || !i.stationary {
        "the aircraft must be stopped on the ground"
    } else if service != Service::Tug && !(i.park_brake || i.chocks) {
        "set the parking brake or place chocks first"
    } else if service != Service::Tug && i.engines_running {
        "shut the engines down first"
    } else {
        "switch the beacon off first"
    })
}

pub fn may_stay(service: Service, i: &Inputs) -> bool {
    match service {
        Service::Tug => i.on_ground,
        Service::Gpu => i.on_ground && i.stationary && !i.on_runway,
        _ => i.on_ground && i.stationary && !i.beacon && !i.on_runway,
    }
}

fn automatic(service: Service, i: &Inputs) -> bool {
    match service {
        Service::Gpu => i.ext_power,
        Service::Pca | Service::Asu => i.on_ground && !i.engines_running && !i.apu_running,
        Service::Fuel => i.refuelling,
        Service::Baggage | Service::Water | Service::Lav | Service::Clean => i.boarding,
        Service::Tug => i.tug_attached || i.tug_called,
        _ => false,
    }
}

#[derive(Clone, Debug, Default)]
pub struct Wants {
    manual: BTreeMap<Service, bool>,
    auto_seen: BTreeMap<Service, bool>,
    place: BTreeMap<&'static str, bool>,
}

impl Wants {
    pub fn toggle(&mut self, service: Service, i: &Inputs) {
        let now = self.wanted(service, i);
        self.manual.insert(service, !now);
    }

    pub fn toggle_place(&mut self, id: &'static str, present: bool) {
        self.place.insert(id, !present);
    }

    pub fn wanted(&self, service: Service, i: &Inputs) -> bool {
        self.manual.get(&service).copied().unwrap_or_else(|| automatic(service, i))
    }

    pub fn observe(&mut self, i: &Inputs) {
        for s in Service::ALL {
            let auto = automatic(s, i);
            if self.auto_seen.insert(s, auto).is_some_and(|was| was != auto) && s.toggled_by_crew() && s != Service::Stairs && s != Service::Catering && s != Service::Deice {
                self.manual.remove(&s);
            }
        }
        if !i.on_ground {
            self.manual.clear();
            self.place.clear();
        }
    }

}

pub fn wanted_placements(w: &Wants, i: &Inputs, present: &BTreeSet<&'static str>) -> Vec<&'static Placement> {
    let mut ids: Vec<&'static str> = Vec::new();
    for s in Service::ALL {
        let on = w.wanted(s, i);
        match s {
            Service::Stairs => {
                for (k, door) in STAIR_DOORS.iter().enumerate() {
                    if i.jetway[k] {
                        continue;
                    }
                    let by_service = on && (*door != M2L || i.boarding);
                    if by_service || i.door_open[k] {
                        ids.push(match k {
                            0 => "stairs_m1l",
                            1 => "stairs_m2l",
                            _ => "stairs_u1l",
                        });
                    }
                }
            }
            Service::Tug if on => {
                ids.extend(if i.tug_attached { ["tug", "crew_wing_l", "crew_wing_r"].as_slice() } else { ["tug_standby"].as_slice() });
                ids.push("crew_headset");
            }
            Service::Fuel if on => ids.extend([if i.hydrant { "fuel_hydrant" } else { "fuel_tanker" }, "crew_fuel"]),
            _ if on => ids.extend(PLACEMENTS.iter().filter(|p| p.service == s).map(|p| p.id)),
            _ => {}
        }
    }
    for (id, on) in &w.place {
        if *on && !ids.contains(id) {
            ids.push(id);
        } else if !*on {
            ids.retain(|x| x != id);
        }
    }
    ids.into_iter()
        .filter_map(placement)
        .filter(|p| if present.contains(p.id) { may_stay(p.service, i) } else { may_enter(p.service, i) })
        .collect()
}

fn fnv1a(text: &str) -> u32 {
    text.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}

pub fn operator_key(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    if n.starts_with("dnata") {
        Some("dnata")
    } else if n.starts_with("swissport") {
        Some("swissport")
    } else if n.starts_with("menzies") {
        Some("menzies")
    } else {
        None
    }
}

#[derive(Clone, Debug, Default)]
pub struct OperatorTable {
    airports: HashMap<String, Vec<&'static str>>,
}

impl OperatorTable {
    pub fn parse(text: &str) -> Self {
        let mut airports = HashMap::new();
        if let Some(map) = serde_json::from_str::<Json>(text).ok().and_then(|v| v.get("airports").and_then(Json::as_object).cloned()) {
            for (icao, entry) in map {
                let mut ops: Vec<&'static str> = Vec::new();
                for kind in ["handler", "caterer", "fueller"] {
                    for name in entry.get(kind).and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str) {
                        if let Some(k) = operator_key(name) {
                            if !ops.contains(&k) {
                                ops.push(k);
                            }
                        }
                    }
                }
                airports.insert(icao.to_ascii_uppercase(), ops);
            }
        }
        Self { airports }
    }

    pub fn embedded() -> Self {
        Self::parse(include_str!("gse_operators.json"))
    }

    pub fn has_airport(&self, icao: &str) -> bool {
        self.airports.contains_key(&icao.trim().to_ascii_uppercase())
    }

    pub fn pick(&self, icao: Option<&str>, forced: u8) -> Option<&'static str> {
        if (1..=3).contains(&forced) {
            return Some(OPERATORS[forced as usize - 1]);
        }
        let icao = icao?.trim().to_ascii_uppercase();
        if icao.is_empty() {
            return None;
        }
        let h = fnv1a(&icao) as usize;
        match self.airports.get(&icao) {
            Some(ops) if !ops.is_empty() => Some(ops[h % ops.len()]),
            _ => Some(OPERATORS[h % OPERATORS.len()]),
        }
    }
}

pub fn operator_index(op: Option<&str>) -> f64 {
    op.and_then(|o| OPERATORS.iter().position(|k| *k == o)).map_or(0., |i| i as f64 + 1.)
}

pub trait Library {
    fn manifest(&mut self, vehicle: &str) -> Option<Manifest>;
    fn object(&mut self, vehicle: &str, operator: Option<&str>) -> Option<String>;
}

pub struct FolderLibrary {
    pub root: Option<PathBuf>,
    cache: HashMap<String, Manifest>,
}

impl FolderLibrary {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self { root, cache: HashMap::new() }
    }
}

fn crew_manifest() -> Manifest {
    let anims = CREW_DATAREFS
        .iter()
        .map(|d| AnimSpec { dataref: d.to_string(), min: 0., max: 0., travel: None, role: Some("hold".into()), period: None, radius: None })
        .collect();
    Manifest { anims, ..Manifest::default() }
}

fn crew_object() -> Option<PathBuf> {
    let root = xp::system_path()?;
    [
        root.join("Resources").join("plugins").join("openSAM").join("resources").join("Marshaller.obj"),
        root.join("Custom Scenery").join("openSAM_Library").join("dgs").join("Marshaller.obj"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

impl Library for FolderLibrary {
    fn manifest(&mut self, vehicle: &str) -> Option<Manifest> {
        if let Some(m) = self.cache.get(vehicle) {
            return Some(m.clone());
        }
        let path = self.root.as_ref()?.join(vehicle).join("manifest.json");
        let m = parse_manifest(vehicle, &std::fs::read_to_string(path).ok()?)?;
        self.cache.insert(vehicle.to_string(), m.clone());
        Some(m)
    }

    fn object(&mut self, vehicle: &str, operator: Option<&str>) -> Option<String> {
        if vehicle == CREW {
            return crew_object().map(|p| p.to_string_lossy().into_owned());
        }
        let dir = self.root.as_ref()?.join(vehicle);
        let listed = self.manifest(vehicle).map(|m| m.objects).unwrap_or_default();
        let mut names = Vec::new();
        if let Some(op) = operator {
            names.extend(listed.get(op).cloned());
            names.push(format!("{vehicle}_{op}.obj"));
        }
        names.extend(listed.get("default").cloned());
        names.push(format!("{vehicle}.obj"));
        names.into_iter().map(|n| dir.join(n)).find(|p| p.is_file()).map(|p| p.to_string_lossy().into_owned())
    }
}

pub trait Scene {
    fn load(&mut self, path: &str, ticket: usize) -> bool;
    fn poll(&mut self, ticket: usize) -> Option<Option<usize>>;
    fn unload(&mut self, object: usize);
    fn create(&mut self, object: usize, datarefs: &[CString]) -> Option<usize>;
    fn place(&mut self, instance: usize, at: [f64; 3], heading: f64, data: &[f32]);
    fn destroy(&mut self, instance: usize);
    fn ground(&mut self, x: f64, y: f64, z: f64) -> Option<f64>;
}

pub struct XpScene;

impl Scene for XpScene {
    fn load(&mut self, path: &str, ticket: usize) -> bool {
        xp::load_object_async(path, ticket)
    }
    fn poll(&mut self, ticket: usize) -> Option<Option<usize>> {
        xp::take_loaded_object(ticket)
    }
    fn unload(&mut self, object: usize) {
        xp::unload_object(object)
    }
    fn create(&mut self, object: usize, datarefs: &[CString]) -> Option<usize> {
        xp::create_instance(object, datarefs)
    }
    fn place(&mut self, instance: usize, at: [f64; 3], heading: f64, data: &[f32]) {
        xp::instance_set_position(instance, at, heading, 0., 0., data)
    }
    fn destroy(&mut self, instance: usize) {
        xp::destroy_instance(instance)
    }
    fn ground(&mut self, x: f64, y: f64, z: f64) -> Option<f64> {
        xp::probe_terrain_y(x, y, z)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObjectState {
    Loading(usize),
    Ready(usize),
    Failed,
}

#[derive(Default)]
struct Objects {
    map: HashMap<String, ObjectState>,
    next_ticket: usize,
}

impl Objects {
    fn get(&mut self, scene: &mut dyn Scene, path: &str) -> Option<usize> {
        match self.map.get(path).copied() {
            Some(ObjectState::Ready(o)) => Some(o),
            Some(ObjectState::Failed) => None,
            Some(ObjectState::Loading(ticket)) => match scene.poll(ticket) {
                Some(Some(o)) => {
                    self.map.insert(path.to_string(), ObjectState::Ready(o));
                    Some(o)
                }
                Some(None) => {
                    crate::log(&format!("gse: could not load {path}"));
                    self.map.insert(path.to_string(), ObjectState::Failed);
                    None
                }
                None => None,
            },
            None => {
                self.next_ticket += 1;
                let ticket = self.next_ticket;
                let state = if scene.load(path, ticket) { ObjectState::Loading(ticket) } else { ObjectState::Failed };
                self.map.insert(path.to_string(), state);
                None
            }
        }
    }

    fn release_unused(&mut self, scene: &mut dyn Scene, used: &HashSet<String>) {
        let mut drop_keys = Vec::new();
        for (path, state) in self.map.iter_mut() {
            if used.contains(path) {
                continue;
            }
            match *state {
                ObjectState::Ready(o) => {
                    scene.unload(o);
                    drop_keys.push(path.clone());
                }
                ObjectState::Loading(ticket) => match scene.poll(ticket) {
                    Some(Some(o)) => {
                        scene.unload(o);
                        drop_keys.push(path.clone());
                    }
                    Some(None) => drop_keys.push(path.clone()),
                    None => {}
                },
                ObjectState::Failed => {}
            }
        }
        for k in drop_keys {
            self.map.remove(&k);
        }
    }


    fn unload_all(&mut self, scene: &mut dyn Scene) {
        self.release_unused(scene, &HashSet::new());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Arriving,
    Connected,
    Departing,
}

struct Unit {
    placement: &'static Placement,
    path: Option<String>,
    geometry: Geometry,
    names: Vec<CString>,
    values: Vec<f32>,
    instance: Option<usize>,
    phase: Phase,
    placed: Option<(Pose, Placed)>,
    drive: f64,
    clock: f64,
    odometer: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DoorAction {
    Open(usize),
    Close(usize),
    Set(usize, f64),
}

const LIFT_SPEED_MS: f64 = 0.35;
const LIFT_UNKNOWN_S: f64 = 6.0;
const EXTEND_S: f64 = 3.0;
const DRIVE_IN_M: f64 = 25.0;

fn drive_speed(remaining: f64) -> f64 {
    (remaining * 0.6).clamp(0.8, 5.0)
}

fn role_value(a: &AnimSpec, role: Role, current: f32, connected: bool, clock: f64, odometer: f64, delta: f64) -> f32 {
    let span = a.max - a.min;
    let period = a.period.unwrap_or(4.0).max(0.1);
    match role {
        Role::Loop if connected => (a.min + (clock / period).fract() * span) as f32,
        Role::Progress if connected => approach(current, a.max, span.abs() / period * delta),
        Role::Wheel => {
            let turns = odometer / (std::f64::consts::TAU * a.radius.unwrap_or(0.4).max(0.05));
            (a.min + turns.fract() * span) as f32
        }
        Role::Blink => if (clock / period).fract() < 0.5 { a.max as f32 } else { a.min as f32 },
        _ => current,
    }
}

#[derive(Default)]
pub struct Fleet {
    units: Vec<Unit>,
    objects: Objects,
    pub wants: Wants,
    gse_opened: BTreeSet<usize>,
    fuel_hose: Option<bool>,
    moving_s: f64,
    airborne_s: f64,
}

const MOVING_S: f64 = 2.0;
const AIRBORNE_S: f64 = 1.0;

pub struct Context<'a> {
    pub inputs: Inputs,
    pub pose: Pose,
    pub cg_reference: [f64; 3],
    pub steer_deg: f64,
    pub operator: Option<&'a str>,
    pub door_opening: [bool; 20],
    pub delta: f64,
}

fn approach(value: f32, target: f64, step: f64) -> f32 {
    let v = value as f64;
    let next = if (target - v).abs() <= step { target } else { v + step * (target - v).signum() };
    next as f32
}

impl Fleet {
    pub fn present(&self) -> BTreeSet<&'static str> {
        self.units.iter().filter(|u| u.phase != Phase::Departing).map(|u| u.placement.id).collect()
    }

    #[cfg(test)]
    pub fn phase(&self, id: &str) -> Option<Phase> {
        self.units.iter().find(|u| u.placement.id == id).map(|u| u.phase)
    }

    #[cfg(test)]
    pub fn instance_count(&self) -> usize {
        self.units.iter().filter(|u| u.instance.is_some()).count()
    }

    #[cfg(test)]
    pub fn values(&self, id: &str) -> Option<Vec<(String, f32)>> {
        let u = self.units.iter().find(|u| u.placement.id == id)?;
        Some(u.geometry.anims.iter().map(|(a, _)| a.dataref.clone()).zip(u.values.iter().copied()).collect())
    }

    #[cfg(test)]
    pub fn placed(&self, id: &str) -> Option<Placed> {
        self.units.iter().find(|u| u.placement.id == id).and_then(|u| u.placed.map(|p| p.1))
    }

    pub fn service_state(&self, s: Service) -> f64 {
        self.units
            .iter()
            .filter(|u| u.placement.service == s)
            .map(|u| match u.phase {
                Phase::Arriving => 1.,
                Phase::Connected => 2.,
                Phase::Departing => 3.,
            })
            .fold(0., |a: f64, b| if b == 2. || a == 2. { 2. } else { a.max(b) })
    }

    pub fn connected(&self, s: Service) -> bool {
        self.units.iter().any(|u| u.placement.service == s && u.phase == Phase::Connected)
    }

    pub fn placement_state(&self, id: &str) -> f64 {
        self.units.iter().find(|u| u.placement.id == id).map_or(0., |u| match u.phase {
            Phase::Arriving => 1.,
            Phase::Connected => 2.,
            Phase::Departing => 3.,
        })
    }

    fn spawn(&mut self, p: &'static Placement, lib: &mut dyn Library, operator: Option<&str>) {
        let manifest = if p.vehicle == CREW { Some(crew_manifest()) } else { lib.manifest(p.vehicle) };
        let path = lib.object(p.vehicle, operator);
        if path.is_none() {
            crate::log(&format!("gse: no object for {} ({}), not placed", p.vehicle, p.id));
        }
        let geometry = geometry(p, manifest.as_ref());
        let names = geometry.anims.iter().filter_map(|(a, _)| CString::new(a.dataref.clone()).ok()).collect::<Vec<_>>();
        let values = geometry.anims.iter().map(|(a, r)| if *r == Role::Hold { 0f64.clamp(a.min.min(a.max), a.min.max(a.max)) as f32 } else { a.min as f32 }).collect();
        crate::log(&format!("gse: {} ({}) called{}", p.id, p.vehicle, operator.map_or(String::new(), |o| format!(", {o}"))));
        let drive = if p.anchor == Anchor::NoseGear || p.vehicle == CREW { 0. } else { DRIVE_IN_M };
        self.units.push(Unit { placement: p, path, geometry, names, values, instance: None, phase: Phase::Arriving, placed: None, drive, clock: 0., odometer: 0. });
    }

    pub fn step(&mut self, ctx: &Context, lib: &mut dyn Library, scene: &mut dyn Scene) -> Vec<DoorAction> {
        self.moving_s = if ctx.inputs.stationary { 0. } else { self.moving_s + ctx.delta };
        self.airborne_s = if ctx.inputs.on_ground { 0. } else { self.airborne_s + ctx.delta };
        let settled = Inputs { stationary: self.moving_s <= MOVING_S, on_ground: self.airborne_s <= AIRBORNE_S, ..ctx.inputs };
        let i = &settled;
        self.wants.observe(i);
        let wanted = wanted_placements(&self.wants, i, &self.present());
        let wanted_ids: BTreeSet<&str> = wanted.iter().map(|p| p.id).collect();
        let mut actions = Vec::new();

        for p in &wanted {
            match self.units.iter().position(|u| u.placement.id == p.id) {
                Some(k) if self.units[k].phase == Phase::Departing => self.units[k].phase = Phase::Arriving,
                Some(_) => {}
                None => self.spawn(p, lib, ctx.operator),
            }
        }

        let mut gone = Vec::new();
        for (k, u) in self.units.iter_mut().enumerate() {
            let vanish = !i.on_ground || (!i.stationary && u.placement.service != Service::Tug);
            if vanish {
                gone.push(k);
                continue;
            }
            if !wanted_ids.contains(u.placement.id) && u.phase != Phase::Departing {
                u.phase = Phase::Departing;
                crate::log(&format!("gse: {} leaving", u.placement.id));
            }
        }
        for k in gone.into_iter().rev() {
            let u = self.units.remove(k);
            if let Some(inst) = u.instance {
                scene.destroy(inst);
            }
            crate::log(&format!("gse: {} removed (aircraft moving or airborne)", u.placement.id));
        }

        let mut finished = Vec::new();
        for (k, u) in self.units.iter_mut().enumerate() {
            if u.instance.is_none() {
                if let Some(path) = u.path.clone() {
                    if let Some(object) = self.objects.get(scene, &path) {
                        u.instance = scene.create(object, &u.names);
                        if u.instance.is_none() {
                            crate::log(&format!("gse: could not create an instance of {path}"));
                            u.path = None;
                        }
                    }
                }
            }
            let follow = u.placement.anchor == Anchor::NoseGear;
            let stale = match u.placed {
                None => true,
                Some((at, _)) => follow || ctx.pose.moved_from(&at),
            };
            if stale {
                let mut ground = |x: f64, y: f64, z: f64| scene.ground(x, y, z);
                let placed = place(u.placement, &u.geometry, &ctx.pose, ctx.cg_reference, ctx.steer_deg, &mut ground);
                u.placed = Some((ctx.pose, placed));
            }
            let Some((_, placed)) = u.placed else { continue };
            u.clock += ctx.delta;
            let can_drive = u.placement.anchor != Anchor::NoseGear;
            if u.phase == Phase::Arriving && u.drive > 0. {
                let step = (drive_speed(u.drive) * ctx.delta).min(u.drive);
                u.drive -= step;
                u.odometer += step;
            }
            let connected = u.phase == Phase::Connected;
            let (clock, odometer) = (u.clock, u.odometer);
            for ((a, role), v) in u.geometry.anims.iter().zip(u.values.iter_mut()) {
                *v = role_value(a, *role, *v, connected, clock, odometer, ctx.delta);
            }
            let driving_in = u.phase == Phase::Arriving && u.drive > 0.;

            let lifts_done;
            {
                let departing = u.phase == Phase::Departing;
                let mut done = true;
                let extends_in = u
                    .geometry
                    .anims
                    .iter()
                    .zip(&u.values)
                    .filter(|((_, r), _)| *r == Role::Extend)
                    .all(|((a, _), v)| (*v as f64 - a.min).abs() < 1e-4);
                for ((a, role), v) in u.geometry.anims.iter().zip(u.values.iter_mut()) {
                    if *role != Role::Lift || driving_in {
                        continue;
                    }
                    let target = if departing {
                        if !extends_in {
                            *v as f64
                        } else {
                            a.min
                        }
                    } else {
                        lift_value(a.min, a.max, u.geometry.attach[1], u.geometry.lift_travel, placed.target_height)
                    };
                    let rate = match u.geometry.lift_travel {
                        Some(t) if t.abs() > 1e-6 => (LIFT_SPEED_MS / t.abs()) * (a.max - a.min).abs(),
                        _ => (a.max - a.min).abs() / LIFT_UNKNOWN_S,
                    };
                    *v = approach(*v, target, rate * ctx.delta);
                    if ((*v as f64) - target).abs() > 1e-4 {
                        done = false;
                    }
                }
                lifts_done = done;
            }
            let mut extends_done = true;
            for ((a, role), v) in u.geometry.anims.iter().zip(u.values.iter_mut()) {
                if *role != Role::Extend || driving_in {
                    continue;
                }
                let target = match u.phase {
                    Phase::Departing => a.min,
                    _ if lifts_done => a.max,
                    _ => a.min,
                };
                *v = approach(*v, target, (a.max - a.min).abs() / EXTEND_S * ctx.delta);
                if ((*v as f64) - target).abs() > 1e-4 {
                    extends_done = false;
                }
            }
            match u.phase {
                Phase::Arriving if !driving_in && lifts_done && extends_done && (u.instance.is_some() || u.path.is_none()) => {
                    u.phase = Phase::Connected;
                    crate::log(&format!("gse: {} in position", u.placement.id));
                    if let Some(d) = u.placement.door {
                        if !ctx.door_opening.get(d).copied().unwrap_or(false) {
                            actions.push(DoorAction::Open(d));
                            self.gse_opened.insert(d);
                        }
                    }
                }
                Phase::Departing => {
                    if let Some(d) = u.placement.door {
                        if self.gse_opened.remove(&d) {
                            actions.push(DoorAction::Close(d));
                        }
                    }
                    if lifts_done && extends_done {
                        if can_drive && u.drive < DRIVE_IN_M {
                            let step = drive_speed(DRIVE_IN_M - u.drive + 1.) * ctx.delta;
                            u.drive += step;
                            u.odometer += step;
                        } else {
                            finished.push(k);
                        }
                    }
                }
                _ => {}
            }
            if let Some(inst) = u.instance {
                let mut at = placed.origin;
                if u.drive > 0. {
                    let back = yaw(placed.heading, [0., 1.]);
                    at[0] += back[0] * u.drive;
                    at[2] += back[1] * u.drive;
                    at[1] = scene.ground(at[0], ctx.pose.position[1], at[2]).unwrap_or(at[1]);
                }
                scene.place(inst, at, placed.heading, &u.values);
            }
        }
        for k in finished.into_iter().rev() {
            let u = self.units.remove(k);
            if let Some(inst) = u.instance {
                scene.destroy(inst);
            }
            crate::log(&format!("gse: {} gone", u.placement.id));
        }

        let still: BTreeSet<usize> = self.units.iter().filter_map(|u| u.placement.door).collect();
        let orphaned: Vec<usize> = self.gse_opened.iter().copied().filter(|d| !still.contains(d)).collect();
        for d in orphaned {
            self.gse_opened.remove(&d);
            actions.push(DoorAction::Close(d));
        }

        let hose = self.connected(Service::Fuel);
        if self.fuel_hose != Some(hose) {
            if hose || self.fuel_hose.is_some() {
                actions.push(DoorAction::Set(FUEL_HOSE, if hose { 100. } else { 0. }));
            }
            self.fuel_hose = Some(hose);
        }

        let used: HashSet<String> = self.units.iter().filter_map(|u| u.path.clone()).collect();
        self.objects.release_unused(scene, &used);
        actions
    }

    pub fn opened_by_gse(&self, door: usize) -> bool {
        self.gse_opened.contains(&door)
    }

    pub fn clear(&mut self, scene: &mut dyn Scene) {
        for u in self.units.drain(..) {
            if let Some(inst) = u.instance {
                scene.destroy(inst);
            }
        }
        self.objects.unload_all(scene);
    }
}

pub fn jetway_door_actions(was: [bool; 3], now: [bool; 3], door_opening: [bool; 3], opened: &mut [bool; 3]) -> Vec<(usize, Request)> {
    let points = [M1L, M2L, U1L];
    let mut out = Vec::new();
    for k in 0..3 {
        if now[k] && !was[k] && !door_opening[k] {
            out.push((points[k], Request::Open));
            opened[k] = true;
        }
        if !now[k] && was[k] && opened[k] {
            out.push((points[k], Request::Close));
            opened[k] = false;
        }
    }
    out
}

struct Ids {
    on_ground: VariableIdentifier,
    stationary: VariableIdentifier,
    park_brake: VariableIdentifier,
    chocks: VariableIdentifier,
    ext_power: [VariableIdentifier; 4],
    on_runway: VariableIdentifier,
    refuel: VariableIdentifier,
    boarding: VariableIdentifier,
    tug_attached: VariableIdentifier,
    tug_wait: VariableIdentifier,
    apu_n: VariableIdentifier,
}

struct Refs {
    local: [Option<DataRef>; 3],
    angles: [Option<DataRef>; 3],
    lat_lon: [Option<DataRef>; 2],
    engines: Option<DataRef>,
    beacon: Option<DataRef>,
    xplane_tug: Option<DataRef>,
    steer: Option<DataRef>,
    cg: [Option<DataRef>; 2],
}

pub struct Gse {
    fleet: Fleet,
    library: FolderLibrary,
    scene: XpScene,
    operators: OperatorTable,
    ids: Ids,
    refs: Refs,
    _published: Published,
    toggles: Vec<(Service, Command)>,
    places: Vec<(&'static str, Command, Value)>,
    jetway_states: [Value; 3],
    states: Vec<(Service, Value)>,
    connected_gpu: Value,
    connected_pca: Value,
    operator_value: Value,
    operator_override: Value,
    airport: Option<String>,
    airport_at: Option<(f64, f64)>,
    opensam: Option<DataRef>,
    opensam_number: Option<DataRef>,
    opensam_lookup_s: f64,
    jetway_was: [bool; 3],
    jetway_opened: [bool; 3],
    logged_operator: Option<Option<&'static str>>,
}

fn objects_root() -> Option<PathBuf> {
    crate::sound_pack::plugin_dir().and_then(|d| Some(d.parent()?.parent()?.join("objects").join("gse")))
}

impl Gse {
    pub fn connected(&self, s: Service) -> bool {
        self.fleet.connected(s)
    }

    pub fn opened_by_service(&self, door: usize) -> bool {
        self.fleet.opened_by_gse(door) || [M1L, M2L, U1L].iter().zip(self.jetway_opened).any(|(d, o)| *d == door && o)
    }

    pub fn new(vars: &mut Vars, xplm: &Xplm) -> Self {
        let mut p = Published::default();
        let toggles = Service::ALL
            .iter()
            .filter(|s| s.toggled_by_crew())
            .map(|s| (*s, p.command(&format!("fbw/gse/{}/toggle", s.name()), &format!("Ground equipment: call or send away {}", s.name()))))
            .collect();
        let states = Service::ALL.iter().map(|s| (*s, p.number(&format!("fbw/gse/{}/state", s.name()), 0., false))).collect();
        let places = PLACEMENTS
            .iter()
            .filter(|pl| per_place(pl))
            .map(|pl| (pl.id, p.command(&format!("fbw/gse/place/{}/toggle", pl.id), &format!("Ground equipment: call or send away {}", pl.id)), p.number(&format!("fbw/gse/place/{}/state", pl.id), 0., false)))
            .collect();
        let jetway_states = ["m1l", "m2l", "u1l"].map(|d| p.number(&format!("fbw/gse/jetway_{d}/state"), 0., false));
        let connected_gpu = p.number("fbw/gse/connected/gpu", 0., false);
        let connected_pca = p.number("fbw/gse/connected/pca", 0., false);
        let operator_value = p.number("fbw/gse/operator", 0., false);
        let operator_override = p.number("fbw/gse/operator_override", 0., true);
        let on_runway = vars.get_unprefixed("ON ANY RUNWAY".to_string());
        let mut get = |n: &str| vars.get(n.to_string());
        let ids = Ids {
            on_runway,
            on_ground: get("SIM ON GROUND"),
            stationary: get("IS_STATIONARY"),
            park_brake: get("PARK_BRAKE_LEVER_POS"),
            chocks: get("MODEL_WHEELCHOCKS_ENABLED"),
            ext_power: [1, 2, 3, 4].map(|n| get(&format!("EXT_PWR_AVAIL:{n}"))),
            refuel: get("REFUEL_STARTED_BY_USR"),
            boarding: get("BOARDING_STARTED_BY_USR"),
            tug_attached: get("PUSHBACK ATTACHED"),
            tug_wait: get("PUSHBACK WAIT"),
            apu_n: get("APU_N_RAW"),
        };
        let f = |n: &str| xplm.find(n);
        let refs = Refs {
            local: ["sim/flightmodel/position/local_x", "sim/flightmodel/position/local_y", "sim/flightmodel/position/local_z"].map(f),
            angles: ["sim/flightmodel/position/psi", "sim/flightmodel/position/theta", "sim/flightmodel/position/phi"].map(f),
            lat_lon: ["sim/flightmodel/position/latitude", "sim/flightmodel/position/longitude"].map(f),
            engines: f("sim/flightmodel/engine/ENGN_running"),
            beacon: f("sim/cockpit2/switches/beacon_on"),
            xplane_tug: f("sim/aircraft/overflow/pushback_attached"),
            steer: f("sim/flightmodel2/gear/tire_steer_actual_deg"),
            cg: ["sim/aircraft/weight/acf_cgY_original", "sim/aircraft/weight/acf_cgZ_original"].map(f),
        };
        let root = objects_root();
        let operators = root
            .as_ref()
            .and_then(|r| std::fs::read_to_string(r.join("operators.json")).ok())
            .map(|t| OperatorTable::parse(&t))
            .filter(|t| !t.airports.is_empty())
            .unwrap_or_else(OperatorTable::embedded);
        let ready = root.as_ref().map_or(0, |r| VEHICLES.iter().filter(|v| r.join(v).join("manifest.json").is_file()).count());
        let mut library = FolderLibrary::new(root.clone());
        let mut anim_refs = BTreeSet::new();
        for v in VEHICLES {
            if let Some(m) = library.manifest(v) {
                for a in m.anims {
                    if anim_refs.insert(a.dataref.clone()) {
                        p.number(&a.dataref, 0., false);
                    }
                }
            }
        }
        crate::log(&format!(
            "gse: vehicles from {} ({ready} of {} with a manifest)",
            root.as_ref().map_or("(no aircraft folder)".to_string(), |r| r.display().to_string()),
            VEHICLES.len()
        ));
        crate::log(&format!("gse: {} animation datarefs registered for the vehicle objects", anim_refs.len()));
        Self {
            fleet: Fleet::default(),
            library,
            scene: XpScene,
            operators,
            ids,
            refs,
            _published: p,
            toggles,
            places,
            jetway_states,
            states,
            connected_gpu,
            connected_pca,
            operator_value,
            operator_override,
            airport: None,
            airport_at: None,
            opensam: None,
            opensam_number: None,
            opensam_lookup_s: 0.,
            jetway_was: [false; 3],
            jetway_opened: [false; 3],
            logged_operator: None,
        }
    }

    fn jetways(&mut self, xplm: &Xplm, delta: f64) -> [bool; 3] {
        if self.opensam.is_none() {
            self.opensam_lookup_s -= delta;
            if self.opensam_lookup_s <= 0. {
                self.opensam_lookup_s = 5.;
                self.opensam = xplm.find("opensam/jetway/door/status");
                self.opensam_number = xplm.find("opensam/jetway/number");
            }
        }
        let Some(d) = self.opensam else { return [false; 3] };
        let types = xp::data_ref_types(d);
        if types & 16 != 0 {
            let mut v = [0; 4];
            let n = xplm.get_vi(d, &mut v);
            [n > 0 && v[0] != 0, n > 1 && v[1] != 0, n > 2 && v[2] != 0]
        } else if types & 8 != 0 {
            let mut v = [0f32; 4];
            let n = xplm.get_vf(d, &mut v);
            [n > 0 && v[0] > 0.5, n > 1 && v[1] > 0.5, n > 2 && v[2] > 0.5]
        } else {
            [false; 3]
        }
    }

    fn update_airport(&mut self, xplm: &Xplm, on_ground: bool) {
        if !on_ground {
            return;
        }
        let (Some(la), Some(lo)) = (self.refs.lat_lon[0], self.refs.lat_lon[1]) else { return };
        let (lat, lon) = (xplm.get_d(la), xplm.get_d(lo));
        let far = self.airport_at.map_or(true, |(a, b)| (a - lat).abs() > 0.03 || (b - lon).abs() > 0.03);
        if far {
            self.airport_at = Some((lat, lon));
            let id = xp::nearest_airport_id(lat, lon);
            if id != self.airport {
                crate::log(&format!("gse: nearest airport {}", id.as_deref().unwrap_or("(none)")));
            }
            self.airport = id;
        }
    }

    pub fn update(&mut self, vars: &mut Vars, xplm: &Xplm, doors: &mut Doors, delta: f64) {
        let settings = crate::app_settings::current();
        let read = |vars: &mut Vars, id: &VariableIdentifier| vars.read(id);
        let mut running = [0; 8];
        if let Some(d) = self.refs.engines {
            xplm.get_vi(d, &mut running);
        }
        let on_ground = read(vars, &self.ids.on_ground) != 0.;
        self.update_airport(xplm, on_ground);
        let jetway3 = self.jetways(xplm, delta);
        let bridges = self.opensam_number.map_or(0, |d| xplm.get_i(d).max(0) as usize);
        let bridge_at: [bool; 3] = std::array::from_fn(|k| jetway3[k] || k < bridges);
        let model = doors.model();
        let door_opening: [bool; 20] = std::array::from_fn(|k| model.doors.get(k).is_some_and(|d| d.opening));
        let external = |k: usize, door: usize| door_opening[door] && !self.fleet.opened_by_gse(door) && !self.jetway_opened[k];
        let inputs = Inputs {
            on_ground,
            stationary: read(vars, &self.ids.stationary) != 0.,
            on_runway: read(vars, &self.ids.on_runway) != 0.,
            runways_unknown: !crate::extra_backend_fbw::RUNWAYS_READY.load(std::sync::atomic::Ordering::Relaxed),
            park_brake: read(vars, &self.ids.park_brake) > 0.5,
            chocks: read(vars, &self.ids.chocks) != 0.,
            engines_running: running[..4].iter().any(|&r| r != 0),
            apu_running: read(vars, &self.ids.apu_n) > 50.,
            beacon: self.refs.beacon.is_some_and(|d| xplm.get_i(d) != 0),
            ext_power: self.ids.ext_power.iter().any(|id| vars.read(id) != 0.),
            refuelling: read(vars, &self.ids.refuel) != 0.,
            boarding: read(vars, &self.ids.boarding) != 0.,
            tug_attached: read(vars, &self.ids.tug_attached) != 0. || self.refs.xplane_tug.is_some_and(|d| xplm.get_i(d) != 0),
            tug_called: read(vars, &self.ids.tug_wait) != 0.,
            jetway: bridge_at,
            door_open: [external(0, M1L), external(1, M2L), external(2, U1L)],
            hydrant: self.airport.as_deref().is_some_and(|a| self.operators.has_airport(a)),
        };

        for k in 0..3 {
            if jetway3[k] != self.jetway_was[k] {
                crate::log(&format!("gse: jet bridge {} at {}", if jetway3[k] { "docked" } else { "undocked" }, crate::doors::NAMES[[M1L, M2L, U1L][k]]));
            }
        }
        self.jetway_was = jetway3;
        for (v, docked) in self.jetway_states.iter().zip(jetway3) {
            published::set(*v, if docked { 2. } else { 0. });
        }

        let explain = |s: Service, i: &Inputs| {
            if let Some(why) = refusal(s, i) {
                crate::notices::post(format!("{}: {why}", s.label()));
            }
        };
        for s in take_requests() {
            self.fleet.wants.toggle(s, &inputs);
            if self.fleet.wants.wanted(s, &inputs) {
                explain(s, &inputs);
            }
        }
        for (s, c) in &self.toggles {
            for _ in 0..published::presses(*c) {
                self.fleet.wants.toggle(*s, &inputs);
                if self.fleet.wants.wanted(*s, &inputs) {
                    explain(*s, &inputs);
                }
            }
        }
        for (id, c, _) in &self.places {
            for _ in 0..published::presses(*c) {
                let present = self.fleet.present().contains(id);
                self.fleet.wants.toggle_place(id, present);
                if !present {
                    if let Some(p) = placement(id) {
                        explain(p.service, &inputs);
                    }
                }
            }
        }

        let forced_dataref = published::get(self.operator_override).round().clamp(0., 3.) as u8;
        let forced = if forced_dataref != 0 { forced_dataref } else { settings.gse_operator };
        let operator = self.operators.pick(self.airport.as_deref(), forced);
        if self.logged_operator != Some(operator) {
            crate::log(&format!("gse: operator {} for {}", operator.unwrap_or("generic"), self.airport.as_deref().unwrap_or("(no airport)")));
            self.logged_operator = Some(operator);
        }
        published::set(self.operator_value, operator_index(operator));

        let pose = Pose {
            position: self.refs.local.map(|d| d.map_or(0., |d| xplm.get_d(d))),
            psi: self.refs.angles[0].map_or(0., |d| xplm.get_f(d) as f64),
            theta: self.refs.angles[1].map_or(0., |d| xplm.get_f(d) as f64),
            phi: self.refs.angles[2].map_or(0., |d| xplm.get_f(d) as f64),
        };
        let ft = |d: Option<DataRef>, fallback: f64| d.map_or(fallback, |d| xplm.get_f(d) as f64) * 0.3048;
        let cg_reference = [0., ft(self.refs.cg[0], 2.8), ft(self.refs.cg[1], -8.0)];
        let steer_deg = self.refs.steer.map_or(0., |d| {
            let mut v = [0f32; 1];
            xplm.get_vf(d, &mut v);
            v[0] as f64
        });

        let inputs = if settings.ground_equipment { inputs } else { Inputs { on_ground: false, ..inputs } };
        crate::physics::tyre::ground_crew_check(inputs.on_ground && inputs.chocks && !inputs.engines_running);
        let ctx = Context { inputs, pose, cg_reference, steer_deg, operator, door_opening, delta };
        for action in self.fleet.step(&ctx, &mut self.library, &mut self.scene) {
            match action {
                DoorAction::Open(_) | DoorAction::Close(_) => {}
                DoorAction::Set(d, pct) => doors.set_open(d, pct),
            }
        }
        for (s, v) in &self.states {
            published::set(*v, self.fleet.service_state(*s));
        }
        for (id, _, v) in &self.places {
            published::set(*v, self.fleet.placement_state(id));
        }
        crate::deep::cabin::live::set_service_trucks(self.fleet.connected(Service::Water), self.fleet.connected(Service::Lav));
        published::set(self.connected_gpu, self.fleet.connected(Service::Gpu) as i32 as f64);
        published::set(self.connected_pca, self.fleet.connected(Service::Pca) as i32 as f64);
    }
}

impl Drop for Gse {
    fn drop(&mut self) {
        self.fleet.clear(&mut self.scene);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_roles_drive_loops_progress_wheels_and_beacons() {
        let m = parse_manifest(
            "belt_loader",
            r#"{"vehicle":"belt_loader","anims":[
                {"dataref":"fbw/gse/belt_loader/belt","min":0,"max":1,"role":"loop","period_s":2},
                {"dataref":"fbw/gse/belt_loader/bags","min":0,"max":1,"role":"progress","period_s":10},
                {"dataref":"fbw/gse/belt_loader/wheel","min":0,"max":360,"role":"wheel","radius_m":0.5},
                {"dataref":"fbw/gse/belt_loader/beacon","min":0,"max":1,"role":"blink","period_s":1}]}"#,
        )
        .unwrap();
        let roles: Vec<Option<Role>> = m.anims.iter().map(declared_role).collect();
        assert_eq!(roles, [Some(Role::Loop), Some(Role::Progress), Some(Role::Wheel), Some(Role::Blink)]);
        let [belt, bags, wheel, beacon] = [&m.anims[0], &m.anims[1], &m.anims[2], &m.anims[3]];
        assert!((role_value(belt, Role::Loop, 0., true, 3.0, 0., 0.1) - 0.5).abs() < 1e-6, "loop wraps over its period");
        assert_eq!(role_value(belt, Role::Loop, 0.25, false, 3.0, 0., 0.1), 0.25, "loop holds while not connected");
        assert!((role_value(bags, Role::Progress, 0., true, 0., 0., 1.0) - 0.1).abs() < 1e-6, "progress fills over period_s");
        let quarter_turn = std::f64::consts::TAU * 0.5 / 4.;
        assert!((role_value(wheel, Role::Wheel, 0., false, 0., quarter_turn, 0.) - 90.).abs() < 1e-3, "wheel turns with distance");
        assert_eq!(role_value(beacon, Role::Blink, 0., false, 0.2, 0., 0.), 1.);
        assert_eq!(role_value(beacon, Role::Blink, 0., false, 0.7, 0., 0.), 0.);
    }

    #[test]
    fn drive_in_slows_down_near_the_aircraft() {
        assert!(drive_speed(DRIVE_IN_M) > drive_speed(2.));
        assert!(drive_speed(0.) > 0.);
    }

    #[derive(Default)]
    struct FakeScene {
        loaded: BTreeMap<usize, String>,
        unloaded: Vec<String>,
        tickets: HashMap<usize, String>,
        next_object: usize,
        instances: BTreeMap<usize, (usize, usize)>,
        destroyed: usize,
        next_instance: usize,
        ground: f64,
        last: HashMap<usize, ([f64; 3], f64, Vec<f32>)>,
    }

    impl Scene for FakeScene {
        fn load(&mut self, path: &str, ticket: usize) -> bool {
            self.tickets.insert(ticket, path.to_string());
            true
        }
        fn poll(&mut self, ticket: usize) -> Option<Option<usize>> {
            let path = self.tickets.remove(&ticket)?;
            self.next_object += 1;
            self.loaded.insert(self.next_object, path);
            Some(Some(self.next_object))
        }
        fn unload(&mut self, object: usize) {
            if let Some(p) = self.loaded.remove(&object) {
                self.unloaded.push(p);
            }
        }
        fn create(&mut self, object: usize, datarefs: &[CString]) -> Option<usize> {
            self.next_instance += 1;
            self.instances.insert(self.next_instance, (object, datarefs.len()));
            Some(self.next_instance)
        }
        fn place(&mut self, instance: usize, at: [f64; 3], heading: f64, data: &[f32]) {
            assert_eq!(self.instances[&instance].1, data.len(), "one value per instance dataref");
            self.last.insert(instance, (at, heading, data.to_vec()));
        }
        fn destroy(&mut self, instance: usize) {
            self.instances.remove(&instance);
            self.destroyed += 1;
        }
        fn ground(&mut self, _: f64, _: f64, _: f64) -> Option<f64> {
            Some(self.ground)
        }
    }

    #[derive(Default)]
    struct FakeLibrary {
        manifests: HashMap<String, Manifest>,
    }

    impl Library for FakeLibrary {
        fn manifest(&mut self, vehicle: &str) -> Option<Manifest> {
            self.manifests.get(vehicle).cloned()
        }
        fn object(&mut self, vehicle: &str, operator: Option<&str>) -> Option<String> {
            Some(match operator {
                Some(o) => format!("{vehicle}/{vehicle}_{o}.obj"),
                None => format!("{vehicle}/{vehicle}.obj"),
            })
        }
    }

    #[test]
    fn ground_crew_walk_the_wingtips_during_pushback() {
        let mut fleet = Fleet::default();
        let mut lib = FakeLibrary::default();
        let mut scene = FakeScene::default();
        let mut i = parked();
        i.tug_called = true;
        run(&mut fleet, &ctx(i), &mut lib, &mut scene, 2);
        assert!(fleet.phase("crew_headset").is_some() && fleet.phase("crew_wing_l").is_none());
        i.tug_attached = true;
        run(&mut fleet, &ctx(i), &mut lib, &mut scene, 2);
        let l = fleet.placed("crew_wing_l").unwrap();
        let r = fleet.placed("crew_wing_r").unwrap();
        assert!(l.origin[0] < -WINGTIP[0] && r.origin[0] > WINGTIP[0], "outboard of the tips: {l:?} {r:?}");
        assert_eq!(fleet.values("crew_headset").unwrap().len(), 4, "every marshaller dataref per instance");
        assert!(fleet.values("crew_headset").unwrap().iter().all(|(_, v)| *v == 0.), "standing, arms down");
        i.stationary = false;
        let mut c = ctx(i);
        c.pose.position = [0., 0., 3.];
        run(&mut fleet, &c, &mut lib, &mut scene, 6);
        assert!(fleet.phase("crew_wing_r").is_some(), "wing walkers stay with the aircraft");
        assert!(close(fleet.placed("crew_wing_r").unwrap().origin[2], WINGTIP[2] - 1. + 3., 1e-6));
    }

    #[test]
    fn baggage_and_fuel_bring_their_crews() {
        let mut fleet = Fleet::default();
        let mut lib = FakeLibrary::default();
        let mut scene = FakeScene::default();
        let mut i = parked();
        i.boarding = true;
        i.refuelling = true;
        run(&mut fleet, &ctx(i), &mut lib, &mut scene, 2);
        for id in ["crew_bag_fwd", "crew_bag_aft", "crew_bulk", "crew_fuel"] {
            assert!(fleet.phase(id).is_some(), "{id}");
        }
        let f = fleet.placed("crew_fuel").unwrap();
        assert!(f.origin[0] < -16.9 && f.origin[0] > -23.8, "between the left engines: {f:?}");
    }

    #[test]
    fn one_catering_truck_can_be_called_without_the_other() {
        let mut fleet = Fleet::default();
        let mut lib = FakeLibrary::default();
        let mut scene = FakeScene::default();
        let i = parked();
        fleet.wants.toggle_place("udcat_u1r", false);
        run(&mut fleet, &ctx(i), &mut lib, &mut scene, 2);
        assert!(fleet.phase("udcat_u1r").is_some() && fleet.phase("cat_m2r").is_none(), "only the upper deck highloader");
        fleet.wants.toggle(Service::Catering, &i);
        run(&mut fleet, &ctx(i), &mut lib, &mut scene, 2);
        assert!(fleet.phase("cat_m2r").is_some(), "the service brings the rest");
        fleet.wants.toggle_place("cat_m2r", true);
        run(&mut fleet, &ctx(i), &mut lib, &mut scene, 2);
        assert_eq!(fleet.phase("cat_m2r"), Some(Phase::Departing), "sent away on its own");
        assert!(fleet.phase("udcat_u1r").is_some_and(|p| p != Phase::Departing));
        assert!(PLACEMENTS.iter().filter(|p| per_place(p)).all(|p| p.service != Service::Tug && p.vehicle != CREW));
    }

    fn parked() -> Inputs {
        Inputs { on_ground: true, stationary: true, park_brake: true, apu_running: true, ..Default::default() }
    }

    fn ctx(inputs: Inputs) -> Context<'static> {
        Context { inputs, pose: Pose::default(), cg_reference: [0.; 3], steer_deg: 0., operator: Some("dnata"), door_opening: [false; 20], delta: 0.5 }
    }

    fn run(fleet: &mut Fleet, c: &Context, lib: &mut FakeLibrary, scene: &mut FakeScene, ticks: usize) -> Vec<DoorAction> {
        let mut out = Vec::new();
        for _ in 0..ticks {
            out.extend(fleet.step(c, lib, scene));
        }
        out
    }

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    const STAIRS_MANIFEST: &str = r#"{
        "vehicle": "stairs_main",
        "dimensions": {"length": 7.0, "width": 2.5, "height": 3.2},
        "origin": "ground under the rear axle",
        "front": "-Z",
        "anims": [
            {"dataref": "fbw/gse/stairs_main/lift", "min": 0, "max": 1, "travel": [0, 0.4, 0]},
            {"dataref": "fbw/gse/stairs_main/platform", "min": 0, "max": 1, "travel": [0, 0, -0.35]}
        ],
        "attach": {"platform_front_edge": [0, 5.0, -3.5]}
    }"#;

    #[test]
    fn nothing_is_called_before_the_runways_are_known() {
        let mut i = parked();
        i.ext_power = true;
        i.runways_unknown = true;
        assert!(Service::ALL.iter().all(|s| !may_enter(*s, &i)));
        assert!(wanted_placements(&Wants::default(), &i, &BTreeSet::new()).is_empty());
    }

    #[test]
    fn a_refused_vehicle_says_why_and_an_allowed_one_says_nothing() {
        let i = parked();
        assert_eq!(refusal(Service::Water, &i), None);
        let mut runway = parked();
        runway.on_runway = true;
        assert!(refusal(Service::Water, &runway).unwrap().contains("runway"));
        let mut running = parked();
        running.engines_running = true;
        assert!(refusal(Service::Gpu, &running).unwrap().contains("engines"));
        let mut beacon = parked();
        beacon.beacon = true;
        assert!(refusal(Service::Catering, &beacon).unwrap().contains("beacon"));
        let mut rolling = parked();
        rolling.park_brake = false;
        assert!(refusal(Service::Baggage, &rolling).unwrap().contains("parking brake"));
        let mut loading = parked();
        loading.runways_unknown = true;
        assert!(refusal(Service::Lav, &loading).unwrap().contains("loading"));
    }

    #[test]
    fn vehicles_found_on_a_runway_leave() {
        let mut i = parked();
        i.ext_power = true;
        i.on_runway = true;
        assert!(Service::ALL.iter().filter(|s| **s != Service::Tug).all(|s| !may_stay(*s, &i)));
    }

    #[test]
    fn nothing_drives_onto_a_runway() {
        let mut i = parked();
        i.ext_power = true;
        i.on_runway = true;
        assert!(Service::ALL.iter().all(|s| !may_enter(*s, &i)));
    }

    #[test]
    fn gpu_wanted_only_with_ground_power_on_the_ground() {
        let w = Wants::default();
        let none = BTreeSet::new();
        let mut i = parked();
        assert!(!wanted_placements(&w, &i, &none).iter().any(|p| p.id == "gpu"), "no ground power, no GPU");
        i.ext_power = true;
        assert!(wanted_placements(&w, &i, &none).iter().any(|p| p.id == "gpu"));
        i.on_ground = false;
        assert!(!wanted_placements(&w, &i, &none).iter().any(|p| p.id == "gpu"), "never in the air");
        i.on_ground = true;
        i.stationary = false;
        assert!(!wanted_placements(&w, &i, &none).iter().any(|p| p.id == "gpu"), "never while rolling");
    }

    #[test]
    fn gpu_position_is_ahead_of_the_right_receptacle() {
        let p = placement("gpu").unwrap();
        let g = geometry(p, None);
        let mut ground = |_: f64, _: f64, _: f64| Some(0.);
        for psi in [0., 90.] {
            let pose = Pose { psi, ..Default::default() };
            let at = place(p, &g, &pose, [0.; 3], 0., &mut ground);
            let tip = yaw(at.heading, [g.attach[0], g.attach[2]]);
            let receptacle = pose.to_world([0.; 3], [GPU_RECEPTACLE[0], 0., GPU_RECEPTACLE[2]]);
            assert!(close(at.origin[0] + tip[0], receptacle[0], 1e-9) && close(at.origin[2] + tip[1], receptacle[2], 1e-9), "plug under the receptacle at psi {psi}");
            assert!(close(wrap360(at.heading - psi), 180., 1e-9), "parallel to the fuselage, towbar aft");
            let right = yaw(psi, [1., 0.]);
            let side = (at.origin[0] - receptacle[0]) * right[0] + (at.origin[2] - receptacle[2]) * right[1];
            assert!(side > 3.0, "the unit stands to the right of the receptacle: {side}");
        }
        assert!(close(g.attach[1], GPU_RECEPTACLE[1], 1e-9), "the plug tip is at the receptacle height");
    }

    #[test]
    fn the_exported_stairs_manifest_drives_the_platform_to_the_sill() {
        let text = r#"{"vehicle":"stairs_main","objects":{"default":"stairs_main.obj","dnata":"stairs_main_dnata.obj"},"length_m":9.996,
            "anims":[{"dataref":"fbw/gse/stairs_main/lift","min":0.0,"max":1.0,"platform_height_at_min_m":4.9,"platform_height_at_max_m":5.6},
                     {"dataref":"fbw/gse/stairs_main/bumper","meaning":"platform bumper slides out towards the aircraft (-Z) by 0.35 m","min":0.0,"max":1.0,"platform_height_at_min_m":null,"platform_height_at_max_m":null}],
            "attach":{"platform_front_edge":[0.0,4.9,0.0],"how":"text","measured_in_obj":{"bridge_group_front_z_m":0.0}}}"#;
        let m = parse_manifest("stairs_main", text).unwrap();
        assert_eq!(m.length, Some(9.996));
        assert_eq!(m.objects["dnata"], "stairs_main_dnata.obj");
        let p = placement("stairs_m1l").unwrap();
        let g = geometry(p, Some(&m));
        assert_eq!(g.attach, [0., 4.9, 0.]);
        assert!(close(g.lift_travel.unwrap(), 0.7, 1e-9));
        assert!(close(g.reach, 0.35, 1e-9));
        let sill = 5.11;
        assert!(close(4.9 + lift_value(0., 1., g.attach[1], g.lift_travel, sill) * 0.7, sill, 1e-9), "FCOM main deck sill inside the 4.9-5.6 m range");
    }

    #[test]
    fn air_units_put_their_hose_nozzle_under_the_connector() {
        let pca = r#"{"vehicle":"pca_unit","length_m":5.336,"anims":[{"dataref":"fbw/gse/pca/hose","min":0.0,"max":1.0}],
            "attach":{"hose_nozzle_tip_left":{"x":-0.26,"y":2.08,"z":-4.3},"hose_nozzle_tip_right":{"x":0.26,"y":2.08,"z":-4.81}}}"#;
        let asu = r#"{"vehicle":"asu_unit","length_m":4.067,"anims":[{"dataref":"fbw/gse/asu/hose","min":0.0,"max":1.0}],
            "attach":{"hose_nozzle_tip":{"x":0.2,"y":1.74,"z":-3.42},"aircraft_connector_target":{"x":0.2,"y":1.78,"z":-3.42}}}"#;
        let mut ground = |_: f64, _: f64, _: f64| Some(0.);
        for (id, vehicle, text, connector, outboard) in [("pca", "pca_unit", pca, LP_CONNECTION, -1.), ("asu", "asu_unit", asu, HP_CONNECTION, 1.)] {
            let m = parse_manifest(vehicle, text).unwrap();
            let p = placement(id).unwrap();
            let g = geometry(p, Some(&m));
            assert_eq!(g.reach, 0.);
            assert!(g.anims.iter().all(|(_, r)| *r == Role::Extend), "the hose runs out once parked");
            for psi in [0., 90.] {
                let pose = Pose { psi, ..Default::default() };
                let at = place(p, &g, &pose, [0.; 3], 0., &mut ground);
                let tip = yaw(at.heading, [g.attach[0], g.attach[2]]);
                let target = pose.to_world([0.; 3], [connector[0], 0., connector[2]]);
                assert!(close(at.origin[0] + tip[0], target[0], 1e-9) && close(at.origin[2] + tip[1], target[2], 1e-9), "{id} nozzle under the connector at psi {psi}");
                let right = yaw(psi, [1., 0.]);
                let side = (at.origin[0] - target[0]) * right[0] + (at.origin[2] - target[2]) * right[1];
                assert!(side * outboard > 3.0, "{id} body stands outboard of the connector: {side}");
            }
        }
    }

    #[test]
    fn object_unloads_when_no_longer_wanted() {
        let mut fleet = Fleet::default();
        let mut lib = FakeLibrary::default();
        let mut scene = FakeScene::default();
        let mut i = parked();
        i.ext_power = true;
        run(&mut fleet, &ctx(i), &mut lib, &mut scene, 3);
        assert_eq!(fleet.instance_count(), 1);
        assert_eq!(scene.loaded.len(), 1);
        assert!(scene.loaded.values().any(|p| p.ends_with("gpu_tld409_dnata.obj")), "the operator's livery");
        i.ext_power = false;
        run(&mut fleet, &ctx(i), &mut lib, &mut scene, 20);
        assert_eq!(fleet.instance_count(), 0);
        assert!(scene.loaded.is_empty(), "unloaded once nothing uses it");
        assert_eq!(scene.unloaded.len(), 1);
    }

    #[test]
    fn one_object_serves_two_units_of_the_same_vehicle() {
        let mut fleet = Fleet::default();
        let mut lib = FakeLibrary::default();
        let mut scene = FakeScene::default();
        fleet.wants.toggle(Service::Baggage, &parked());
        run(&mut fleet, &ctx(parked()), &mut lib, &mut scene, 3);
        let ldcl = scene.loaded.values().filter(|p| p.contains("ldcl_loader")).count();
        assert_eq!(ldcl, 1, "both cargo doors share one loaded loader");
        assert!(fleet.phase("ldcl_fwd").is_some() && fleet.phase("ldcl_aft").is_some());
        assert!(fleet.phase("belt_bulk").is_some() && fleet.phase("uld_fwd").is_some() && fleet.phase("bulk_cart").is_some());
    }

    #[test]
    fn nothing_enters_without_parking_brake_or_chocks() {
        let w = Wants::default();
        let mut i = parked();
        i.park_brake = false;
        i.ext_power = true;
        assert!(wanted_placements(&w, &i, &BTreeSet::new()).is_empty());
        i.chocks = true;
        assert!(!wanted_placements(&w, &i, &BTreeSet::new()).is_empty(), "chocks are enough");
    }

    #[test]
    fn a_present_vehicle_stays_when_the_brake_is_released() {
        let w = Wants::default();
        let mut i = parked();
        i.ext_power = true;
        let present: BTreeSet<&'static str> = ["gpu"].into_iter().collect();
        i.park_brake = false;
        assert!(wanted_placements(&w, &i, &present).iter().any(|p| p.id == "gpu"));
    }

    #[test]
    fn the_beacon_sends_everything_away_but_the_tug_and_the_gpu() {
        let mut w = Wants::default();
        let mut i = parked();
        i.ext_power = true;
        i.tug_attached = true;
        w.toggle(Service::Catering, &i);
        let present: BTreeSet<&'static str> = PLACEMENTS.iter().map(|p| p.id).collect();
        let before: Vec<_> = wanted_placements(&w, &i, &present).iter().map(|p| p.id).collect();
        assert!(before.contains(&"cat_m2r"));
        i.beacon = true;
        let after: Vec<_> = wanted_placements(&w, &i, &present).iter().map(|p| p.id).collect();
        assert!(after.contains(&"gpu") && after.contains(&"tug"));
        assert!(!after.contains(&"cat_m2r") && !after.contains(&"pca"));
    }

    #[test]
    fn vehicles_vanish_when_the_aircraft_moves_but_the_tug_stays() {
        let mut fleet = Fleet::default();
        let mut lib = FakeLibrary::default();
        let mut scene = FakeScene::default();
        let mut i = parked();
        i.ext_power = true;
        i.tug_attached = true;
        run(&mut fleet, &ctx(i), &mut lib, &mut scene, 3);
        assert!(fleet.phase("gpu").is_some() && fleet.phase("tug").is_some());
        i.stationary = false;
        let mut c = ctx(i);
        c.pose.position = [0., 0., 3.];
        run(&mut fleet, &c, &mut lib, &mut scene, 2);
        assert!(fleet.phase("gpu").is_some(), "a jolt does not send the GPU away");
        run(&mut fleet, &c, &mut lib, &mut scene, 4);
        assert!(fleet.phase("gpu").is_none(), "gone once really rolling");
        assert!(fleet.phase("pca").is_none());
        let tug = fleet.placed("tug").unwrap();
        assert!(close(tug.origin[2], NOSE_GEAR_Z + 3. - 2.5, 1e-6), "the tug follows the nose gear: {tug:?}");
    }

    #[test]
    fn pca_and_asu_come_with_engines_and_apu_off_and_can_be_sent_away() {
        let mut i = parked();
        i.apu_running = false;
        let mut w = Wants::default();
        w.observe(&i);
        let ids = |w: &Wants, i: &Inputs| wanted_placements(w, i, &BTreeSet::new()).iter().map(|p| p.id).collect::<Vec<_>>();
        assert!(ids(&w, &i).contains(&"pca") && ids(&w, &i).contains(&"asu"));
        w.toggle(Service::Pca, &i);
        assert!(!ids(&w, &i).contains(&"pca"), "the crew sent it away");
        i.apu_running = true;
        w.observe(&i);
        assert!(!ids(&w, &i).contains(&"pca") && !ids(&w, &i).contains(&"asu"));
        w.toggle(Service::Asu, &i);
        assert!(ids(&w, &i).contains(&"asu"), "called with the APU running");
    }

    #[test]
    fn stairs_go_to_doors_without_a_jet_bridge() {
        let mut w = Wants::default();
        let mut i = parked();
        w.toggle(Service::Stairs, &i);
        let ids = |w: &Wants, i: &Inputs| wanted_placements(w, i, &BTreeSet::new()).iter().map(|p| p.id).filter(|id| id.starts_with("stairs")).collect::<Vec<_>>();
        assert_eq!(ids(&w, &i), ["stairs_m1l", "stairs_u1l"]);
        i.boarding = true;
        assert_eq!(ids(&w, &i), ["stairs_m1l", "stairs_m2l", "stairs_u1l"], "M2L too while boarding");
        i.jetway = [true, true, false];
        assert_eq!(ids(&w, &i), ["stairs_u1l"], "none at a docked jet bridge");
        let mut j = parked();
        j.door_open = [false, true, false];
        assert_eq!(ids(&Wants::default(), &j), ["stairs_m2l"], "an open door gets stairs");
    }

    #[test]
    fn stairs_platform_meets_the_sill_and_then_the_door_opens() {
        let mut fleet = Fleet::default();
        let mut lib = FakeLibrary::default();
        lib.manifests.insert("stairs_main".into(), parse_manifest("stairs_main", STAIRS_MANIFEST).unwrap());
        let mut scene = FakeScene::default();
        scene.ground = -4.2;
        fleet.wants.toggle(Service::Stairs, &parked());
        let mut c = ctx(parked());
        c.pose.position = [0., 0., 0.];
        let actions = run(&mut fleet, &c, &mut lib, &mut scene, 40);
        assert_eq!(fleet.phase("stairs_m1l"), Some(Phase::Connected));
        let sill_above_ground = SILLS[M1L][1] - scene.ground;
        let values = fleet.values("stairs_m1l").unwrap();
        let lift = values.iter().find(|(n, _)| n.ends_with("/lift")).unwrap().1 as f64;
        assert!(close(5.0 + lift * 0.4, sill_above_ground, 1e-3), "platform {} vs sill {sill_above_ground}", 5.0 + lift * 0.4);
        assert_eq!(values.iter().find(|(n, _)| n.ends_with("/platform")).unwrap().1, 1.0);
        let placed = fleet.placed("stairs_m1l").unwrap();
        let front = yaw(placed.heading, [0., -3.5 - 0.35]);
        let edge_x = placed.origin[0] + front[0];
        assert!(close(edge_x, SILLS[M1L][0] - 0.05, 1e-6), "extended edge {edge_x} stops 5 cm short of the skin");
        assert!(actions.contains(&DoorAction::Open(M1L)), "{actions:?}");
        assert!(actions.contains(&DoorAction::Open(U1L)));
        fleet.wants.toggle(Service::Stairs, &parked());
        let actions = run(&mut fleet, &c, &mut lib, &mut scene, 40);
        assert!(actions.contains(&DoorAction::Close(M1L)), "the door it opened is closed as it leaves");
        assert!(fleet.phase("stairs_m1l").is_none());
    }

    #[test]
    fn the_sill_follows_the_aircraft_s_pitch_and_height() {
        let p = placement("stairs_m1l").unwrap();
        let g = geometry(p, parse_manifest("stairs_main", STAIRS_MANIFEST).as_ref());
        let mut ground = |_: f64, _: f64, _: f64| Some(0.);
        let level = place(p, &g, &Pose { position: [0., 4.2, 0.], ..Default::default() }, [0., 0.853, -2.438], 0., &mut ground);
        let nose_up = place(p, &g, &Pose { position: [0., 4.2, 0.], theta: 1.0, ..Default::default() }, [0., 0.853, -2.438], 0., &mut ground);
        let lower = place(p, &g, &Pose { position: [0., 4.0, 0.], ..Default::default() }, [0., 0.853, -2.438], 0., &mut ground);
        assert!(close(level.target_height, 4.2 + SILLS[M1L][1] - 0.853, 1e-9));
        assert!(nose_up.target_height > level.target_height + 0.4, "the forward door rises nose up");
        assert!(close(level.target_height - lower.target_height, 0.2, 1e-9), "gear compression lowers the sill");
    }

    #[test]
    fn tug_docks_by_its_cradle_and_the_gpu_plug_rests_centred() {
        let tug = r#"{"vehicle":"tow_tpx500","anims":[{"dataref":"fbw/gse/tow/cradle","min":0,"max":1}],"attach":{"cradle_centre":[0,0.76,-2.0],"cradle_roller_top":[0,0.82,-2.0]}}"#;
        let m = parse_manifest("tow_tpx500", tug).unwrap();
        let g = geometry(placement("tug").unwrap(), Some(&m));
        assert!(close(g.attach[2], -2.0, 1e-9));
        assert_eq!(g.anims[0].1, Role::Hold);
        let gpu = r#"{"vehicle":"gpu_tld409","anims":[{"dataref":"fbw/gse/gpu/plug_dz","min":-0.15,"max":0.15}],"attach":{"plug_tip_0":[3.55,3.15,1.168]}}"#;
        let m = parse_manifest("gpu_tld409", gpu).unwrap();
        let g = geometry(placement("gpu").unwrap(), Some(&m));
        assert_eq!(g.anims[0].1, Role::Hold);
        assert_eq!(g.anims[0].0.dataref, "fbw/gse/gpu/plug_dz");
    }

    #[test]
    fn loaders_dock_by_their_raised_front_and_hold_the_feeder_platform() {
        let ldcl = r#"{"vehicle":"ldcl_loader","anims":[{"dataref":"fbw/gse/ldcl_loader/rear_platform","min":"0.0","max":"2.3","motion":"rises 0 to 2.3 m"},{"dataref":"fbw/gse/ldcl_loader/bridge_platform","min":"0.0","max":"2.3","motion":"rises 0 to 2.3 m"}],"attach":{"bridge_lip_front_min":[0,1.17,-7.48],"bridge_lip_front_max":[0,3.47,-7.48]}}"#;
        let m = parse_manifest("ldcl_loader", ldcl).unwrap();
        let p = PLACEMENTS.iter().find(|p| p.id == "ldcl_fwd").unwrap();
        let g = geometry(p, Some(&m));
        assert!(close(g.attach[2], -7.48, 1e-9) && close(g.attach[1], 1.17, 1e-9));
        assert!(close(g.lift_travel.unwrap(), 2.3, 1e-9));
        assert_eq!(g.anims.iter().find(|(a, _)| a.part() == "rear_platform").unwrap().1, Role::Hold);
        assert_eq!(g.anims.iter().find(|(a, _)| a.part() == "bridge_platform").unwrap().1, Role::Lift);
        assert!(close(lift_value(0., 2.3, g.attach[1], g.lift_travel, 3.3), 2.13, 1e-9));
        let belt = r#"{"vehicle":"belt_loader","anims":[{"dataref":"fbw/gse/belt_loader/boom","min":"0.0","max":"20.0","motion":"rotates the boom up 0 to 20 degrees"}],"attach":{"belt_tip_top_min":[-0.25,0.984,-8.04],"belt_tip_top_max":[-0.25,3.485,-7.541]}}"#;
        let m = parse_manifest("belt_loader", belt).unwrap();
        let p = PLACEMENTS.iter().find(|p| p.id == "belt_bulk").unwrap();
        let g = geometry(p, Some(&m));
        assert!(close(g.attach[2], -7.541, 1e-9) && close(g.attach[1], 0.984, 1e-9));
        assert!(close(g.lift_travel.unwrap(), 3.485 - 0.984, 1e-9));
    }

    #[test]
    fn lift_value_is_clamped_to_the_anim_range() {
        assert!(close(lift_value(0., 1., 5.0, Some(0.4), 5.2), 0.5, 1e-12));
        assert_eq!(lift_value(0., 1., 5.0, Some(0.4), 9.0), 1.);
        assert_eq!(lift_value(0., 1., 5.0, Some(0.4), 1.0), 0.);
        assert_eq!(lift_value(0., 1., 5.0, None, 1.0), 1., "unknown travel: fully raised");
        assert!(close(lift_value(10., 20., 1.45, Some(3.65), 5.10), 20., 1e-9));
    }

    #[test]
    fn manifests_parse_in_every_supported_shape() {
        let m = parse_manifest("stairs_main", STAIRS_MANIFEST).unwrap();
        assert_eq!(m.length, Some(7.0));
        assert_eq!(m.anims.len(), 2);
        assert_eq!(m.anims[0].travel, Some([0., 0.4, 0.]));
        assert_eq!(m.attach["platform_front_edge"], [0., 5.0, -3.5]);

        let keyed = r#"{"units":"mm","anims":{"lift":{"min":0,"max":1,"travel":3650}},
            "attach_points":[{"name":"Box_Front","position":{"x":0,"y":1450,"z":-4100}}]}"#;
        let m = parse_manifest("cat_highloader", keyed).unwrap();
        assert_eq!(m.anims[0].dataref, "fbw/gse/cat_highloader/lift");
        assert_eq!(m.anims[0].travel, Some([0., 3.65, 0.]));
        assert_eq!(m.attach["box_front"], [0., 1.45, -4.1]);

        let blender = r#"{"anims":[{"steps":[{"kind":"trans","offset0":[0,0,0],"offset1":[0,0,0.4],"v0":0,"v1":1,"dataref":"fbw/gse/stairs_main/lift"}]}]}"#;
        let m = parse_manifest("stairs_main", blender).unwrap();
        assert_eq!(m.anims[0].dataref, "fbw/gse/stairs_main/lift");
        assert_eq!(m.anims[0].travel, Some([0., 0.4, 0.]), "Blender z up is OBJ y up");
        assert!(parse_manifest("x", "not json").is_none());
    }

    #[test]
    fn geometry_falls_back_to_the_reference_sheet() {
        let p = placement("udcat_u1r").unwrap();
        let g = geometry(p, None);
        assert_eq!(g.attach, [0., 1.9, 0.]);
        assert!(g.anims.is_empty());
        let m = parse_manifest("udcat_highloader", r#"{"anims":["fbw/gse/udcat/lift","fbw/gse/udcat/bridge"]}"#).unwrap();
        let g = geometry(p, Some(&m));
        assert_eq!(g.anims.iter().map(|(_, r)| *r).collect::<Vec<_>>(), [Role::Lift, Role::Extend]);
        assert_eq!(g.lift_travel, Some(6.4));
        assert_eq!(g.reach, 0.9);
    }

    #[test]
    fn the_exported_highloader_docks_its_bridge_edge() {
        let text = r#"{"anims":[{"dataref":"fbw/gse/cat_highloader/lift","min":0.0,"max":1.0,"platform_height_at_min_m":1.8,"platform_height_at_max_m":5.6},
            {"dataref":"fbw/gse/cat_highloader/bridge","meaning":"bridge deck and seal extend towards the aircraft (-Z) by 1.30 m","min":0.0,"max":1.0}],
            "attach":{"platform_front_edge":[0.0,1.8,0.0]}}"#;
        let m = parse_manifest("cat_highloader", text).unwrap();
        let g = geometry(placement("cat_m2r").unwrap(), Some(&m));
        assert_eq!(g.attach, [0., 1.8, 0.]);
        assert!(close(g.lift_travel.unwrap(), 3.8, 1e-9) && close(g.reach, 1.3, 1e-9));
    }

    #[test]
    fn operators_come_from_the_airport_table() {
        let t = OperatorTable::embedded();
        assert_eq!(t.pick(Some("OMDB"), 0), Some("dnata"));
        assert_eq!(t.pick(Some("LSZH"), 0), Some("swissport"));
        assert_eq!(t.pick(Some("EHAM"), 0), Some("menzies"));
        assert!(["swissport", "menzies", "dnata"].contains(&t.pick(Some("egll"), 0).unwrap()));
        let unknown = t.pick(Some("KXYZ"), 0);
        assert_eq!(unknown, t.pick(Some("KXYZ"), 0), "deterministic");
        assert!(OPERATORS.contains(&unknown.unwrap()));
        assert_eq!(t.pick(None, 0), None, "no airport: generic");
        assert_eq!(t.pick(Some("OMDB"), 3), Some("menzies"), "forced");
        assert!(t.has_airport("OMDB") && !t.has_airport("KXYZ"));
    }

    #[test]
    fn the_fuel_hose_point_follows_the_fuel_vehicle() {
        let mut fleet = Fleet::default();
        let mut lib = FakeLibrary::default();
        let mut scene = FakeScene::default();
        let mut i = parked();
        i.refuelling = true;
        i.hydrant = true;
        let actions = run(&mut fleet, &ctx(i), &mut lib, &mut scene, 30);
        assert!(fleet.phase("fuel_hydrant").is_some() && fleet.phase("fuel_tanker").is_none());
        assert!(actions.contains(&DoorAction::Set(FUEL_HOSE, 100.)));
        i.refuelling = false;
        let actions = run(&mut fleet, &ctx(i), &mut lib, &mut scene, 30);
        assert!(actions.contains(&DoorAction::Set(FUEL_HOSE, 0.)));
        i.refuelling = true;
        i.hydrant = false;
        run(&mut fleet, &ctx(i), &mut lib, &mut scene, 2);
        assert!(fleet.phase("fuel_tanker").is_some(), "a tanker where there is no hydrant");
    }

    #[test]
    fn jet_bridges_open_and_close_their_doors() {
        let mut opened = [false; 3];
        let a = jetway_door_actions([false, false, false], [true, true, true], [false, true, false], &mut opened);
        assert_eq!(a, vec![(M1L, Request::Open), (U1L, Request::Open)], "M2L was already open");
        assert_eq!(opened, [true, false, true]);
        let a = jetway_door_actions([true, true, true], [false, false, false], [true, true, true], &mut opened);
        assert_eq!(a, vec![(M1L, Request::Close), (U1L, Request::Close)], "only the doors the bridges opened are closed");
    }

    #[test]
    fn a_body_point_turns_with_the_aircraft() {
        let p = Pose { position: [100., 5., -50.], psi: 90., ..Default::default() };
        let w = p.to_world([0., 1., -2.], [0., 1., -12.]);
        assert!(close(w[0], 110., 1e-9) && close(w[1], 5., 1e-9) && close(w[2], -50., 1e-9), "{w:?}");
    }

    #[test]
    fn every_placement_has_a_known_vehicle_and_door() {
        for p in &PLACEMENTS {
            assert!(VEHICLES.contains(&p.vehicle) || p.vehicle == CREW, "{}", p.vehicle);
            if let Anchor::Door(d) = p.anchor {
                assert!(d < SILLS.len());
            }
        }
        let main = 16.75 * 0.3048;
        let upper = 25.75 * 0.3048;
        assert!(close(SILLS[U1L][1] - SILLS[M1L][1], upper - main, 0.01), "FCOM DSC-52-10 sill difference");
    }
}

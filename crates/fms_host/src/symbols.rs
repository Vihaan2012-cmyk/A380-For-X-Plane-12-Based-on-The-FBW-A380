pub mod flags {
    pub const VOR: u32 = 1 << 0;
    pub const VOR_DME: u32 = 1 << 1;
    pub const NDB: u32 = 1 << 2;
    pub const WAYPOINT: u32 = 1 << 3;
    pub const AIRPORT: u32 = 1 << 4;
    pub const RUNWAY: u32 = 1 << 5;
    pub const TUNED: u32 = 1 << 6;
    pub const ACTIVE_LEG_TERMINATION: u32 = 1 << 7;
    pub const EFIS_OPTION: u32 = 1 << 8;
    pub const DME: u32 = 1 << 9;
    pub const CONSTRAINT: u32 = 1 << 10;
    pub const FIX_INFO: u32 = 1 << 11;
    pub const FLIGHT_PLAN: u32 = 1 << 12;
    pub const COURSE_REVERSAL_LEFT: u32 = 1 << 13;
    pub const COURSE_REVERSAL_RIGHT: u32 = 1 << 14;
    pub const CYAN: u32 = 1 << 15;
    pub const AMBER: u32 = 1 << 16;
    pub const MAGENTA: u32 = 1 << 17;
    pub const LEFT_SIDE_ONLY: u32 = 1 << 18;
    pub const RIGHT_SIDE_ONLY: u32 = 1 << 19;
    pub const SECONDARY_FLIGHT_PLAN: u32 = 1 << 20;
}

pub mod pwp_flags {
    pub const END_OF_VD_MARKER: u32 = 1 << 0;
    pub const DECEL: u32 = 1 << 1;
    pub const TOP_OF_DESCENT: u32 = 1 << 2;
    pub const SPEED_CHANGE: u32 = 1 << 3;
    pub const CLIMB_LEVEL_OFF: u32 = 1 << 4;
    pub const DESCENT_LEVEL_OFF: u32 = 1 << 5;
    pub const START_OF_CLIMB: u32 = 1 << 6;
    pub const INTERCEPT_PROFILE: u32 = 1 << 7;
    pub const TIME_MARKER: u32 = 1 << 8;
    pub const CDA_FLAP1: u32 = 1 << 9;
    pub const CDA_FLAP2: u32 = 1 << 10;
}

use std::collections::BTreeMap;

use fms::navdb::Location;
use fms::plan::{Element, FlightPlan, LegType, SegmentClass, SegmentKind};
use fms_data::PerformanceData;
use fms_lnav::geo::{bearing_to, diff_angle, distance_to, normalise360, place_bearing_distance};
use fms_lnav::geometry::LegCalc;
use fms_lnav::{Coord, Geometry};
use fms_perf::FlightPhase;
use serde_json::{json, Map, Value};

use crate::efis::{departure_procedure_active, eosid_in_plans, EfisInterface, FIRST_SECONDARY, NUM_SECONDARY, SIDES};
use crate::facilities::{vhf_type, Facilities, NearbyFacility, NearbyKind};
use crate::inputs::Inputs;
use crate::navaids::NavaidTuner;
use crate::outputs::Sink;
use crate::pwp::{complete_leg_orientation, PseudoWaypoint};
use crate::vnav::{constraint_json, LegPrediction, VnavHost};

pub mod efis_option {
    pub const CONSTRAINTS: u32 = 1 << 0;
    pub const VOR_DMES: u32 = 1 << 1;
    pub const WAYPOINTS: u32 = 1 << 2;
    pub const NDBS: u32 = 1 << 3;
    pub const AIRPORTS: u32 = 1 << 4;
}

pub const A380_RANGE_VALUES: [f64; 8] = [-1., 10., 20., 40., 80., 160., 320., 640.];
const ND_MODE_ROSE_NAV: i32 = 2;
const ND_MODE_ARC: i32 = 3;
const ND_MODE_PLAN: i32 = 4;
const UPDATE_BLOCK_MS: f64 = 200.;
const WAYPOINT_DESCRIPTOR_AIRPORT: i32 = 1;
const WAYPOINT_DESCRIPTOR_RUNWAY: i32 = 4;
const SECTION_CODE_AIRPORT: i32 = 4;
const CONSTRAINT_TYPE_CLB: i32 = 1;
const CONSTRAINT_TYPE_DES: i32 = 2;
const MAX_SYMBOLS: f64 = 640. / 6.;

/// FBW `InternalFmsSymbol`.
#[derive(Clone, Debug, Default)]
pub struct FmsSymbol {
    pub database_id: String,
    pub ident: String,
    pub location: Option<Location>,
    pub direction: Option<f64>,
    pub length: Option<f64>,
    pub kind: u32,
    pub type_pwp: Option<u32>,
    pub constraints: Option<Vec<String>>,
    pub radials: Option<Vec<f64>>,
    pub radii: Option<Vec<f64>>,
    pub distance_from_airplane: Option<f64>,
    pub predicted_altitude: Option<f64>,
    pub alt_constraint: Option<Value>,
    pub is_altitude_constraint_met: Option<bool>,
}

fn location_json(l: &Location) -> Value {
    let mut v = json!({ "lat": l.lat, "long": l.long });
    if let Some(a) = l.alt {
        v["alt"] = json!(a);
    }
    v
}

fn loc(c: Coord) -> Location {
    Location { lat: c.lat, long: c.long, alt: None }
}

fn coord(l: &Location) -> Coord {
    Coord::new(l.lat, l.long)
}

fn put(m: &mut Map<String, Value>, k: &str, v: Option<Value>) {
    if let Some(v) = v {
        m.insert(k.into(), v);
    }
}

impl FmsSymbol {
    fn base_json(&self) -> Map<String, Value> {
        let mut m = Map::new();
        m.insert("databaseId".into(), json!(self.database_id));
        m.insert("ident".into(), json!(self.ident));
        put(&mut m, "location", self.location.as_ref().map(location_json));
        m
    }

    pub fn nd_json(&self) -> Value {
        let mut m = self.base_json();
        put(&mut m, "direction", self.direction.map(|v| json!(v)));
        put(&mut m, "length", self.length.map(|v| json!(v)));
        put(&mut m, "type", Some(json!(self.kind)));
        put(&mut m, "typePwp", self.type_pwp.map(|v| json!(v)));
        put(&mut m, "constraints", self.constraints.as_ref().map(|v| json!(v)));
        put(&mut m, "radials", self.radials.as_ref().map(|v| json!(v)));
        put(&mut m, "radii", self.radii.as_ref().map(|v| json!(v)));
        put(&mut m, "distanceFromAirplane", self.distance_from_airplane.map(|v| json!(v)));
        Value::Object(m)
    }

    pub fn vd_json(&self) -> Value {
        let mut m = self.base_json();
        put(&mut m, "predictedAltitude", self.predicted_altitude.map(|v| json!(v)));
        put(&mut m, "direction", self.direction.map(|v| json!(v)));
        put(&mut m, "length", self.length.map(|v| json!(v)));
        put(&mut m, "type", Some(json!(self.kind)));
        put(&mut m, "typePwp", self.type_pwp.map(|v| json!(v)));
        put(&mut m, "constraints", self.constraints.as_ref().map(|v| json!(v)));
        put(&mut m, "altConstraint", self.alt_constraint.clone());
        put(&mut m, "isAltitudeConstraintMet", self.is_altitude_constraint_met.map(|v| json!(v)));
        put(&mut m, "distanceFromAirplane", self.distance_from_airplane.map(|v| json!(v)));
        Value::Object(m)
    }
}

fn upsert(symbols: &mut Vec<FmsSymbol>, mut symbol: FmsSymbol) {
    let idx = if symbol.database_id.is_empty() { None } else { symbols.iter().position(|s| s.database_id == symbol.database_id) };
    if let Some(idx) = idx {
        let old = symbols.remove(idx);
        symbol.constraints = symbol.constraints.or(old.constraints);
        symbol.direction = symbol.direction.or(old.direction);
        symbol.length = symbol.length.or(old.length);
        symbol.location = symbol.location.or(old.location);
        symbol.kind |= old.kind;
        if let Some(p) = old.type_pwp.filter(|p| *p != 0) {
            symbol.type_pwp = Some(symbol.type_pwp.unwrap_or(0) | p);
        }
        if let Some(r) = old.radials {
            symbol.radials.get_or_insert_with(Vec::new).extend(r);
        }
        if let Some(r) = old.radii {
            symbol.radii.get_or_insert_with(Vec::new).extend(r);
        }
    }
    symbols.push(symbol);
}

fn vor_dme_type_flag(t: u32) -> u32 {
    match t {
        vhf_type::VOR_DME | vhf_type::VORTAC => flags::VOR_DME,
        vhf_type::VOR => flags::VOR,
        vhf_type::DME | vhf_type::TACAN => flags::DME,
        _ => 0,
    }
}

/// FBW `calculateEditArea`: `[ahead, behind, beside]` in NM.
pub fn edit_area(range: f64, mode: i32) -> [f64; 3] {
    let tiers = |t: [[f64; 3]; 6]| {
        for (i, r) in [10., 20., 40., 80., 160.].iter().enumerate() {
            if range <= *r {
                return t[i];
            }
        }
        t[5]
    };
    match mode {
        ND_MODE_ARC => tiers([[10.5, 3.5, 8.3], [20.5, 7., 16.6], [40.5, 14., 33.2], [80.5, 28., 66.4], [160.5, 56., 132.8], [320.5, 112., 265.6]]),
        ND_MODE_ROSE_NAV => tiers([[7.6, 7.1, 7.1], [14.7, 14.2, 14.2], [28.9, 28.4, 28.4], [57.3, 56.8, 56.8], [114.1, 113.6, 113.6], [227.7, 227.2, 227.2]]),
        ND_MODE_PLAN => tiers([[7., 7., 7.], [14., 14., 14.], [28., 28., 28.], [56., 56., 56.], [112., 112., 112.], [224., 224., 224.]]),
        _ => [0., 0., 0.],
    }
}

fn within_edit_area(ll: Coord, mrp: Coord, orientation: f64, area: [f64; 3]) -> bool {
    let dist = distance_to(mrp, ll);
    let bearing = normalise360(bearing_to(mrp, ll) - orientation).to_radians();
    let (dx, dy) = (dist * bearing.sin(), dist * bearing.cos());
    dx.abs() < area[2] && dy > -area[1] && dy < area[0]
}

fn nearby_params(area: [f64; 3], mrp: Coord, orientation: f64) -> (f64, Coord) {
    let half_height = (area[0] + area[1]) / 2.;
    (area[2].hypot(half_height), place_bearing_distance(mrp, orientation, half_height - area[1]))
}

fn true_course(def: &fms::plan::LegDefinition) -> f64 {
    let c = def.course.unwrap_or(0.);
    def.mag_var.map_or(c, |mv| normalise360(c + mv))
}

fn is_course_reversal(t: LegType) -> bool {
    matches!(t, LegType::HA | LegType::HF | LegType::HM | LegType::PI)
}

/// FBW `Arinc429LocalVarOutputWord.setBnrValue` as the local var stores it.
pub fn bnr_word(value: f64, ssm: u32, bits: i32, range_max: f64, range_min: f64) -> f64 {
    let quantum = range_min.abs().max(range_max) / 2f64.powi(bits);
    let data = ((value / quantum).round() * quantum).clamp(range_min, range_max);
    ssm as f64 * 4_294_967_296. + (data as f32).to_bits() as f64
}

pub struct SymbolsContext<'a> {
    pub plans: &'a [Option<FlightPlan>; 6],
    pub geometries: &'a BTreeMap<(usize, bool), Geometry>,
    pub calculated: &'a BTreeMap<(usize, bool), Vec<Option<LegCalc>>>,
    pub inputs: &'a Inputs,
    pub phase: FlightPhase,
    pub perf: Option<&'a PerformanceData>,
    pub vnav: &'a VnavHost,
    pub pwps: &'a [PseudoWaypoint],
    pub navaids: &'a NavaidTuner,
    pub facilities: &'a dyn Facilities,
}

struct PlanArgs<'a> {
    alternate: bool,
    index: usize,
    plan: &'a FlightPlan,
    geometry: &'a Geometry,
    calc: Option<&'a [Option<LegCalc>]>,
    predictions: Option<&'a BTreeMap<usize, LegPrediction>>,
}

struct View {
    mrp: Coord,
    orientation: f64,
    area: [f64; 3],
    range: f64,
    option: u32,
    plan_mode: bool,
}

/// FBW `EfisSymbols`, one per side, with `transmitVd` on as the A380 builds it.
pub struct EfisSymbols {
    side: usize,
    block_ms: f64,
    last_mode: i32,
    last_range: f64,
    last_option: u32,
    last_ppos: Coord,
    last_mrp: Coord,
    last_true_heading: f64,
    nearby: [Vec<NearbyFacility>; 4],
    nearby_changed: bool,
    last_fp_versions: BTreeMap<usize, i64>,
    last_navaid_version: i64,
    last_vnav_version: Option<u64>,
    last_iface_version: i64,
    pub data_limit_reached: bool,
}

impl EfisSymbols {
    pub fn new(side: usize) -> Self {
        EfisSymbols {
            side,
            block_ms: 0.,
            last_mode: -1,
            last_range: 0.,
            last_option: 0,
            last_ppos: Coord::new(0., 0.),
            last_mrp: Coord::new(0., 0.),
            last_true_heading: -1.,
            nearby: Default::default(),
            nearby_changed: false,
            last_fp_versions: BTreeMap::new(),
            last_navaid_version: -1,
            last_vnav_version: None,
            last_iface_version: -1,
            data_limit_reached: false,
        }
    }

    fn plan_centre(&self, c: &SymbolsContext, iface: &EfisInterface) -> Option<Coord> {
        let (fp_index, index, in_altn) = iface.plan_centre;
        let main = c.plans.get(fp_index)?.as_ref()?;
        let plan = if in_altn { main.alternate.as_deref()? } else { main };
        let leg = plan.leg_at(index)?;
        let geometry = c.geometries.get(&(fp_index, in_altn))?;
        let gi = if leg.is_vectors() { index.checked_sub(1)? } else { index };
        Self::geometry_termination(plan, geometry, gi).map(|(l, _)| coord(&l))
    }

    /// The geometry leg's termination: its fix, or the computed end point for legs that end elsewhere.
    fn geometry_termination(plan: &FlightPlan, g: &Geometry, i: usize) -> Option<(Location, Option<String>)> {
        if i >= g.leg_count() {
            return None;
        }
        let leg = plan.leg_at(i)?;
        if leg.is_xf() || leg.is_hx() || leg.leg_type == LegType::PI {
            let w = leg.termination_waypoint()?;
            return Some((w.location, Some(w.database_id.clone())));
        }
        g.leg_path(i).last().map(|v| (loc(v.end()), None))
    }

    fn nearby_query(&mut self, c: &SymbolsContext, option: u32, centre: Coord, radius: f64) {
        let kinds = [
            (efis_option::AIRPORTS, NearbyKind::Airport, 100),
            (efis_option::NDBS, NearbyKind::NdbNavaid, 100),
            (efis_option::VOR_DMES, NearbyKind::VhfNavaid, 100),
            (efis_option::WAYPOINTS, NearbyKind::Waypoint, 200),
        ];
        for (k, (bit, kind, max)) in kinds.into_iter().enumerate() {
            if option & bit != 0 {
                let found = c.facilities.nearby(kind, centre, radius, max);
                if found != self.nearby[k] {
                    self.nearby[k] = found;
                    self.nearby_changed = true;
                }
            }
        }
    }

    pub fn update(&mut self, dt_ms: f64, c: &SymbolsContext, iface: &EfisInterface, sink: &mut Sink) {
        self.block_ms -= dt_ms;
        if self.block_ms > 0. {
            return;
        }
        let side = SIDES[self.side];
        let i = c.inputs;
        let ppos = Coord::new(i.lat, i.lon);
        let true_heading = i.true_heading;
        let mode = i.nd_mode[self.side];
        let mrp = if mode == ND_MODE_PLAN { self.plan_centre(c, iface).unwrap_or(ppos) } else { ppos };
        let mrp_changed = distance_to(self.last_mrp, mrp) > 2.;
        if mrp_changed {
            self.last_mrp = mrp;
            self.last_ppos = ppos;
        }
        let heading_changed = diff_angle(true_heading, self.last_true_heading).abs() > 2.;
        if heading_changed {
            self.last_true_heading = true_heading;
        }
        let version = |k: usize| c.plans.get(k).and_then(|p| p.as_ref()).map_or(-1, |p| p.version as i64);
        let mut fp_changed = false;
        for k in [0, 1].into_iter().chain(FIRST_SECONDARY..FIRST_SECONDARY + NUM_SECONDARY) {
            let v = version(k);
            fp_changed |= self.last_fp_versions.get(&k) != Some(&v);
            self.last_fp_versions.insert(k, v);
        }
        let navaid_version = c.navaids.navaid_version as i64;
        let navaids_changed = self.last_navaid_version != navaid_version;
        self.last_navaid_version = navaid_version;
        let vnav_changed = self.last_vnav_version != Some(c.vnav.version());
        self.last_vnav_version = Some(c.vnav.version());
        let range = i.nd_range.get(self.side).and_then(|r| A380_RANGE_VALUES.get(*r)).copied().unwrap_or(f64::NAN);
        let option = i.efis_option[self.side];
        let range_changed = !(self.last_range == range || (self.last_range.is_nan() && range.is_nan()));
        self.last_range = range;
        let mode_changed = self.last_mode != mode;
        self.last_mode = mode;
        let option_changed = self.last_option != option;
        self.last_option = option;
        let orientation = if mode == ND_MODE_PLAN { 0. } else { true_heading };
        let area = edit_area(range, mode);
        let (radius, centre) = nearby_params(area, mrp, orientation);
        self.nearby_query(c, option, centre, radius);
        self.nearby_changed &= option & !efis_option::CONSTRAINTS > 0;
        let iface_changed = self.last_iface_version != iface.version as i64;
        self.last_iface_version = iface.version as i64;

        if !mrp_changed && !heading_changed && !range_changed && !mode_changed && !option_changed && !self.nearby_changed && !fp_changed && !navaids_changed && !vnav_changed && !iface_changed {
            return;
        }
        self.nearby_changed = false;
        sink.set(&format!("A32NX_EFIS_{side}_MRP_LAT"), bnr_word(mrp.lat, 3, 20, 90., -90.));
        sink.set(&format!("A32NX_EFIS_{side}_MRP_LONG"), bnr_word(mrp.long, 3, 20, 180., -180.));

        let mut symbols: Vec<FmsSymbol> = Vec::new();
        let within = |l: &Location| within_edit_area(coord(l), mrp, orientation, area);
        let facility = |f: &NearbyFacility, kind: u32| FmsSymbol { database_id: f.database_id.clone(), ident: f.ident.clone(), location: Some(f.location), kind, ..Default::default() };
        if option & efis_option::VOR_DMES != 0 {
            for vor in &self.nearby[2] {
                let t = vor_dme_type_flag(vor.vhf_type);
                if t != 0 && within(&vor.location) {
                    upsert(&mut symbols, facility(vor, t | flags::EFIS_OPTION));
                }
            }
        }
        if option & efis_option::NDBS != 0 {
            for ndb in self.nearby[1].iter().filter(|f| within(&f.location)) {
                upsert(&mut symbols, facility(ndb, flags::NDB | flags::EFIS_OPTION));
            }
        }
        if option & efis_option::AIRPORTS != 0 {
            for ap in self.nearby[0].iter().filter(|f| within(&f.location)) {
                upsert(&mut symbols, facility(ap, flags::AIRPORT | flags::EFIS_OPTION));
            }
        }
        if option & efis_option::WAYPOINTS != 0 {
            for wp in self.nearby[3].iter().filter(|f| (range < 160. || f.section_code != SECTION_CODE_AIRPORT) && within(&f.location)) {
                upsert(&mut symbols, facility(wp, flags::WAYPOINT | flags::EFIS_OPTION));
            }
        }

        let plan_mode = mode == ND_MODE_PLAN;
        let view = View { mrp, orientation, area, range, option, plan_mode };
        let calc = |k: (usize, bool)| c.calculated.get(&k).map(|v| v.as_slice());
        let add_plan = |symbols: &mut Vec<FmsSymbol>, args: PlanArgs| {
            for s in self.plan_symbols(c, iface, &view, &args) {
                upsert(symbols, s);
            }
        };
        if let (Some(active), Some(g)) = (c.plans[0].as_ref(), c.geometries.get(&(0, false))) {
            add_plan(&mut symbols, PlanArgs { alternate: false, index: 0, plan: active, geometry: g, calc: calc((0, false)), predictions: c.vnav.mcdu_profile().map(|_| &c.vnav.predictions) });
            if let (Some(altn), Some(ag)) = (active.alternate.as_deref().filter(|a| a.leg_count() > 0), c.geometries.get(&(0, true))) {
                if iface.should_transmit_alternate(0, active, plan_mode) {
                    add_plan(&mut symbols, PlanArgs { alternate: true, index: 0, plan: altn, geometry: ag, calc: calc((0, true)), predictions: None });
                }
            }
        }
        if let (Some(tmpy), Some(g)) = (c.plans[1].as_ref(), c.geometries.get(&(1, false))) {
            add_plan(&mut symbols, PlanArgs { alternate: false, index: 1, plan: tmpy, geometry: g, calc: calc((1, false)), predictions: None });
        }
        for sec in 1..=NUM_SECONDARY {
            let index = FIRST_SECONDARY + sec - 1;
            let (Some(plan), Some(g)) = (c.plans[index].as_ref(), c.geometries.get(&(index, false))) else { continue };
            if !iface.should_transmit_secondary(sec) {
                continue;
            }
            add_plan(&mut symbols, PlanArgs { alternate: false, index, plan, geometry: g, calc: calc((index, false)), predictions: None });
            if let (Some(altn), Some(ag)) = (plan.alternate.as_deref().filter(|a| a.leg_count() > 0), c.geometries.get(&(index, true))) {
                if iface.should_transmit_alternate(index, plan, plan_mode) {
                    add_plan(&mut symbols, PlanArgs { alternate: true, index, plan: altn, geometry: ag, calc: calc((index, true)), predictions: None });
                }
            }
        }

        if let Some(active) = c.plans[0].as_ref() {
            let eosid = c.phase < FlightPhase::Climb
                && c.geometries.contains_key(&(0, false))
                && range < 160.
                && iface.should_transmit_eosid(0, plan_mode, c.inputs.engine_out, departure_procedure_active(active), eosid_in_plans(c.plans));
            if eosid {
                for leg in active.engine_out_legs().iter().filter_map(Element::leg) {
                    let Some(term) = leg.termination_waypoint() else { continue };
                    if !within(&term.location) {
                        continue;
                    }
                    let mut s = FmsSymbol { database_id: term.database_id.clone(), ident: term.ident.clone(), location: Some(term.location), kind: flags::FLIGHT_PLAN, ..Default::default() };
                    if is_course_reversal(leg.leg_type) {
                        s.kind |= if leg.definition.turn_direction == Some('L') { flags::COURSE_REVERSAL_LEFT } else { flags::COURSE_REVERSAL_RIGHT };
                        s.direction = Some(true_course(&leg.definition));
                    }
                    upsert(&mut symbols, s);
                }
            }
        }

        let active_geometry = c.geometries.get(&(0, false));
        for p in c.pwps.iter().filter(|p| p.displayed_on_nd) {
            if (self.side == 0 && p.efis_symbol_flag & flags::RIGHT_SIDE_ONLY != 0) || (self.side == 1 && p.efis_symbol_flag & flags::LEFT_SIDE_ONLY != 0) {
                continue;
            }
            let mut direction = None;
            if let (Some(f), Some(g), Some(along)) = (p.efis_pwp_symbol_flag, active_geometry, p.along_leg_index) {
                if f & pwp_flags::END_OF_VD_MARKER != 0 && along < g.leg_count() && !g.leg_path(along).is_empty() {
                    direction = complete_leg_orientation(g, along, p.distance_from_leg_termination.unwrap_or(f64::NAN));
                }
            }
            let s = FmsSymbol {
                database_id: format!("W      {}", p.ident),
                ident: p.ident.clone(),
                location: p.efis_symbol_lla.map(loc),
                direction,
                kind: p.efis_symbol_flag,
                type_pwp: p.efis_pwp_symbol_flag,
                distance_from_airplane: Some(p.distance_from_start),
                predicted_altitude: p.predicted_altitude,
                ..Default::default()
            };
            upsert(&mut symbols, s);
        }
        for ndb in c.navaids.tuned_ndbs() {
            upsert(&mut symbols, FmsSymbol { database_id: ndb.database_id.clone(), ident: ndb.ident.clone(), location: Some(ndb.location), kind: flags::NDB | flags::TUNED, ..Default::default() });
        }
        for vor in c.navaids.tuned_vors() {
            upsert(&mut symbols, FmsSymbol { database_id: vor.database_id.clone(), ident: vor.ident.clone(), location: Some(vor.location), kind: vor_dme_type_flag(vor.vhf_type) | flags::TUNED, ..Default::default() });
        }

        let len = symbols.len() as f64;
        self.data_limit_reached = len > MAX_SYMBOLS;
        if self.data_limit_reached {
            symbols.drain(0..(len - MAX_SYMBOLS).floor() as usize);
        }
        sink.gd(&format!("A32NX_EFIS_{side}_SYMBOLS"), Value::Array(symbols.iter().map(FmsSymbol::nd_json).collect()));
        sink.bus(&format!("vdSymbols_{side}"), Value::Array(symbols.iter().map(FmsSymbol::vd_json).collect()));
        self.block_ms = UPDATE_BLOCK_MS;
    }

    fn plan_symbols(&self, c: &SymbolsContext, iface: &EfisInterface, v: &View, a: &PlanArgs) -> Vec<FmsSymbol> {
        let plan = a.plan;
        let lat_auto = c.vnav.is_lat_auto_control_active();
        let lat_armed = c.vnav.is_lat_auto_control_armed_with_intercept();
        let selected_vertical = c.vnav.is_selected_vertical_mode_active();
        let waypoint_predictions = c.vnav.mcdu_profile().map(|_| &c.vnav.predictions);
        let main = c.plans[a.index].as_ref().unwrap_or(plan);
        let transmit_missed = if a.alternate { iface.should_transmit_alternate_missed(a.index, main, v.plan_mode) } else { iface.should_transmit_missed(a.index, plan, v.plan_mode) };
        let all = plan.all_legs();
        let active = plan.active_leg_index;
        let first_missed = plan.first_missed_approach_leg_index();
        let class_of = |i: usize| plan.segment_position(i).map(|(s, _)| plan.segments[s].kind.class());
        let trans_alt = c.perf.and_then(|p| p.transition_altitude()).filter(|t| *t != 0.);
        let trans_fl = c.perf.and_then(|p| p.transition_level()).filter(|t| *t != 0.);
        let fmt_alt = |alt: f64, descent: bool, prefix: &str| {
            if descent {
                let fl = (alt / 100.).round();
                if trans_fl.is_some_and(|t| fl >= t) {
                    return format!("{prefix}FL{fl}");
                }
            } else if trans_alt.is_some_and(|t| alt >= t) {
                return format!("{prefix}FL{}", (alt / 100.).round());
            }
            format!("{prefix}{}", alt.round())
        };
        let mut ret = Vec::new();
        let mut i = all.len() as i64 - 1;
        while i >= active - 1 && i >= 0 {
            let idx = i as usize;
            i -= 1;
            let before_active = (idx as i64) < active;
            let is_active = idx as i64 == active;
            let Some(leg) = all[idx].leg() else { continue };
            let prev = idx.checked_sub(1).and_then(|p| all.get(p));
            let next = all.get(idx + 1);
            if matches!(leg.definition.waypoint_descriptor, Some(WAYPOINT_DESCRIPTOR_AIRPORT | WAYPOINT_DESCRIPTOR_RUNWAY)) {
                continue;
            }
            if leg.definition.leg_type == LegType::VM || (leg.definition.leg_type == LegType::FM && !prev.is_some_and(Element::is_disco)) {
                continue;
            }
            if !is_active && v.range >= 160. {
                if let (Some(Element::Leg(_)), Some(Element::Leg(_))) = (prev, next) {
                    let cls = class_of(idx);
                    if class_of(idx - 1) == cls && matches!(cls, Some(SegmentClass::Departure | SegmentClass::Arrival)) {
                        continue;
                    }
                }
            }
            let (location, database_id) = match Self::geometry_termination(plan, a.geometry, idx) {
                Some((l, Some(id))) => (Some(l), Some(id)),
                Some((l, None)) => (Some(l), Some(format!("X{:06}{}", idx % 1000, leg.ident.chars().take(5).collect::<String>()))),
                None => (leg.termination_waypoint().map(|w| w.location), leg.termination_waypoint().map(|w| w.database_id.clone())),
            };
            let (Some(location), Some(database_id)) = (location, database_id) else { continue };
            if !within_edit_area(coord(&location), v.mrp, v.orientation, v.area) {
                continue;
            }
            let mut kind = flags::FLIGHT_PLAN;
            let mut direction = None;
            if is_active && !a.alternate && a.index == 0 {
                kind |= flags::ACTIVE_LEG_TERMINATION;
            } else if is_course_reversal(leg.leg_type) && idx as i64 > active + 1 && v.range <= 80. {
                kind |= if leg.definition.turn_direction == Some('L') { flags::COURSE_REVERSAL_LEFT } else { flags::COURSE_REVERSAL_RIGHT };
                direction = Some(true_course(&leg.definition));
            } else if a.index >= FIRST_SECONDARY {
                kind |= flags::SECONDARY_FLIGHT_PLAN;
            }
            if idx >= first_missed && !transmit_missed {
                continue;
            }
            let alt_constraint = leg.altitude_constraint();
            let is_xa = matches!(leg.leg_type, LegType::CA | LegType::FA | LegType::VA);
            let circle_phase = (c.phase <= FlightPhase::Climb && leg.constraint_type == CONSTRAINT_TYPE_CLB)
                || (matches!(c.phase, FlightPhase::Cruise | FlightPhase::Descent | FlightPhase::Approach) && leg.constraint_type == CONSTRAINT_TYPE_DES);
            if (lat_auto || lat_armed) && !before_active && alt_constraint.is_some() && circle_phase && !a.alternate && !is_xa && a.index == 0 {
                if !selected_vertical {
                    kind |= flags::CONSTRAINT;
                    match waypoint_predictions.and_then(|w| w.get(&idx)) {
                        Some(p) if p.is_altitude_constraint_met == Some(true) => kind |= flags::MAGENTA,
                        Some(_) => kind |= flags::AMBER,
                        None => {}
                    }
                } else if is_active {
                    kind |= flags::CONSTRAINT;
                }
            }
            let mut constraints = Vec::new();
            if v.option & efis_option::CONSTRAINTS != 0 && !before_active {
                if let (false, Some(ac)) = (is_xa, &alt_constraint) {
                    let descent = leg.constraint_type == CONSTRAINT_TYPE_DES;
                    match (ac.descriptor.as_str(), ac.altitude2) {
                        ("@" | "I" | "X", _) => constraints.push(fmt_alt(ac.altitude1, descent, "")),
                        ("+" | "J" | "V", _) => constraints.push(fmt_alt(ac.altitude1, descent, "+")),
                        ("-" | "Y", _) => constraints.push(fmt_alt(ac.altitude1, descent, "-")),
                        ("B", Some(a2)) => {
                            constraints.push(fmt_alt(ac.altitude1, descent, "-"));
                            constraints.push(fmt_alt(a2, descent, "+"));
                        }
                        ("C", Some(a2)) => constraints.push(fmt_alt(a2, descent, "+")),
                        _ => {}
                    }
                }
                if let Some(sc) = leg.speed_constraint() {
                    constraints.push(format!("{}KT", sc.speed.floor()));
                }
            }
            let pred = a.predictions.and_then(|p| p.get(&idx));
            ret.push(FmsSymbol {
                database_id,
                ident: leg.ident.clone(),
                location: Some(location),
                kind,
                constraints: (!constraints.is_empty()).then_some(constraints),
                alt_constraint: alt_constraint.as_ref().map(constraint_json),
                is_altitude_constraint_met: match (a.predictions, pred) {
                    (Some(_), Some(p)) => p.is_altitude_constraint_met,
                    _ => Some(true),
                },
                direction,
                distance_from_airplane: pred.map(|p| p.distance_from_aircraft),
                predicted_altitude: pred.map(|p| p.altitude),
                ..Default::default()
            });
        }

        let constraint_flags = flags::CONSTRAINT | flags::MAGENTA | flags::AMBER;
        let mut count = 0;
        for s in ret.iter_mut().rev() {
            if s.kind & constraint_flags == 0 {
                continue;
            }
            if count >= 2 {
                s.kind &= !constraint_flags;
            } else {
                count += 1;
            }
        }

        let dtpp = c.vnav.mcdu_profile().map_or(0., |m| m.distance_to_present_position);
        let segment_last = |kind: SegmentKind| -> Option<usize> {
            let mut start = 0;
            for s in &plan.segments {
                if s.kind == kind {
                    return s.legs.iter().rposition(|e| e.leg().is_some()).map(|k| start + k);
                }
                start += s.legs.len();
            }
            None
        };
        let ends = [(plan.origin_airport.as_ref(), plan.origin_runway.as_ref(), SegmentKind::Origin), (plan.destination_airport.as_ref(), plan.destination_runway.as_ref(), SegmentKind::Destination)];
        for (airport, runway, seg) in ends {
            let Some(airport) = airport else { continue };
            let rwy_ident = runway.map_or_else(|| " ".repeat(8), |r| format!("{:<8}", r.fix.ident));
            let database_id = format!("A{}{}{}{}", airport.ident, if a.alternate { "A" } else { " " }, a.index, rwy_ident);
            let along = segment_last(seg).and_then(|k| a.calc.and_then(|cs| cs.get(k)).and_then(|c| c.as_ref())).map(|lc| lc.cumulative_distance_with_transitions);
            let distance = along.unwrap_or_else(|| distance_to(self.last_ppos, coord(&airport.location))) - dtpp;
            match runway {
                Some(r) if within_edit_area(coord(&r.fix.location), v.mrp, v.orientation, v.area) => ret.push(FmsSymbol {
                    database_id,
                    ident: r.fix.ident.clone(),
                    location: Some(r.fix.location),
                    direction: Some(r.bearing),
                    length: Some(r.length * 1852.),
                    kind: flags::RUNWAY,
                    distance_from_airplane: Some(distance),
                    predicted_altitude: r.threshold_location.alt,
                    ..Default::default()
                }),
                None if within_edit_area(coord(&airport.location), v.mrp, v.orientation, v.area) => ret.push(FmsSymbol {
                    database_id,
                    ident: airport.ident.clone(),
                    location: Some(airport.location),
                    kind: flags::AIRPORT | flags::FLIGHT_PLAN,
                    distance_from_airplane: Some(distance),
                    predicted_altitude: airport.location.alt,
                    ..Default::default()
                }),
                _ => {}
            }
        }
        ret
    }
}

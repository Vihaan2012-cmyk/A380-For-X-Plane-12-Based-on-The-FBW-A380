//! The native FMS host (crates/fms_host) running beside FlyByWire's JS FMS,
//! log only. Each frame feeds it `Inputs::from_vars` from the variable store
//! and compares the L: variables it produced with what the JS wrote to them.
//! It starts on an empty navigation database and switches to X-Plane's
//! (src/fms_navdb.rs) once that has loaded on a background thread; the native
//! MFD (src/mfd_native_host.rs) sends it the flight plan commands.
//!
//! Log lines: `native shadow: fms <var> native=<x> fbw=<y>` on the first
//! sustained difference, and a `native shadow: summary fms ...` line every
//! 10 s with the GENERIC_DATA topic count.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};

use fms::navdb::{Airway, Fix, MemoryNavDb, NavDatabase, Procedure, Runway};
use fms_host::{FmsHost, Inputs, Outputs};

use crate::fms_navdb::FmsNavDb;

use datalink::xp_weather::{RoutePoint, RouteStage, WindRoute};

use crate::js_bridge::{EnvRefs, HostState, Resolved, VarsHost};
use crate::native_shadow::{Compared, Comparator};
use crate::Vars;

const MAX_STEP_MS: f64 = 250.;

type Db = Arc<dyn NavDatabase + Send + Sync>;

struct SharedNavDb(Mutex<FmsNavDb>);

impl SharedNavDb {
    fn with<R: Default>(&self, f: impl FnOnce(&FmsNavDb) -> R) -> R {
        self.0.lock().map(|db| f(&db)).unwrap_or_default()
    }
}

impl NavDatabase for SharedNavDb {
    fn airport(&self, icao: &str) -> Option<Fix> {
        self.with(|d| d.airport(icao))
    }
    fn runways(&self, airport_icao: &str) -> Vec<Runway> {
        self.with(|d| d.runways(airport_icao))
    }
    fn departures(&self, airport_icao: &str) -> Vec<Procedure> {
        self.with(|d| d.departures(airport_icao))
    }
    fn arrivals(&self, airport_icao: &str) -> Vec<Procedure> {
        self.with(|d| d.arrivals(airport_icao))
    }
    fn approaches(&self, airport_icao: &str) -> Vec<Procedure> {
        self.with(|d| d.approaches(airport_icao))
    }
    fn search_fix(&self, ident: &str) -> Vec<Fix> {
        self.with(|d| d.search_fix(ident))
    }
    fn search_airway(&self, ident: &str, via: &Fix) -> Vec<Airway> {
        self.with(|d| d.search_airway(ident, via))
    }
    fn navaids(&self, ident: &str) -> Vec<Fix> {
        self.with(|d| d.navaids(ident))
    }
    fn engine_out_departure(&self, airport_icao: &str, departure_database_id: &str, runway_ident: &str) -> Option<Procedure> {
        self.with(|d| d.engine_out_departure(airport_icao, departure_database_id, runway_ident))
    }
}

impl fms_host::facilities::Facilities for SharedNavDb {
    fn nearby(&self, kind: fms_host::facilities::NearbyKind, centre: fms_lnav::Coord, radius_nm: f64, max: usize) -> Vec<fms_host::facilities::NearbyFacility> {
        self.with(|d| d.nearby(kind, centre, radius_nm, max))
    }
    fn vhf_navaid(&self, database_id: &str) -> Option<fms_host::facilities::VhfNavaid> {
        self.with(|d| d.vhf_navaid(database_id))
    }
    fn ndb_navaid(&self, database_id: &str) -> Option<fms_host::facilities::NdbNavaid> {
        self.with(|d| d.ndb_navaid(database_id))
    }
    fn ils_at_airport(&self, airport_ident: &str, ils_ident: &str) -> Option<fms_host::facilities::IlsNavaid> {
        self.with(|d| d.ils_at_airport(airport_ident, ils_ident))
    }
    fn ils(&self, database_id: &str) -> Option<fms_host::facilities::IlsNavaid> {
        self.with(|d| d.ils(database_id))
    }
    fn runway_details(&self, airport_ident: &str, runway_ident: &str) -> Option<fms_host::facilities::RunwayDetails> {
        self.with(|d| d.runway_details(airport_ident, runway_ident))
    }
}

static NAVDB: OnceLock<Db> = OnceLock::new();
static FACILITIES: OnceLock<Arc<SharedNavDb>> = OnceLock::new();
static NAVDB_STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn start_navdb() {
    if NAVDB_STARTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let Some(root) = crate::xp::system_path() else { return };
    let ils_from_scenery = crate::app_settings::current().ils_from_scenery;
    let spawned = std::thread::Builder::new().name("fms-navdb".into()).spawn(move || {
        let loaded = crate::navdata::NavData::load_with(&root, ils_from_scenery).and_then(|mut nav| {
            nav.set_magvar_source(Box::new(|lat, lon| crate::xp::magnetic_variation(lat, lon).map_or(0., f64::from)));
            FmsNavDb::new(nav)
        });
        match loaded {
            Ok(db) => {
                let shared = Arc::new(SharedNavDb(Mutex::new(db)));
                let _ = FACILITIES.set(shared.clone());
                let _ = NAVDB.set(shared);
            }
            Err(e) => crate::log(&format!("native fms: navigation database not loaded: {e}")),
        }
    });
    if let Err(e) = spawned {
        crate::log(&format!("native fms: navigation database thread: {e}"));
    }
}

/// Runs the SimBrief uplink against the loaded database and returns its log (offline harness).
#[cfg(feature = "test-support")]
pub fn uplink_dry_run(json: &str) -> Result<(Vec<String>, usize), String> {
    let db = NAVDB.get().ok_or("navigation database not loaded")?.clone();
    let ofp = fms::uplink::Ofp::from_simbrief_json(json)?;
    let mut fms = fms::service::Fms::new();
    let mut log = vec![format!("{} navlog fixes", ofp.navlog.len())];
    for c in fms::uplink::route_chunks(&ofp) {
        log.push(format!("chunk {c:?}"));
    }
    let r = fms::uplink::uplink_flight_plan_from_simbrief(&mut fms, &*db, &ofp, &mut log);
    log.push(format!("result {r:?}"));
    let legs = fms.plans.get(2).and_then(|p| p.as_ref()).map_or(0, |p| p.all_legs().len());
    Ok((log, legs))
}

/// The ILS the navaid tuner would select for each ILS approach at `airport` (offline harness).
#[cfg(feature = "test-support")]
pub fn ils_dry_run(airport: &str) -> Result<Vec<String>, String> {
    use fms_host::facilities::Facilities;
    let db = NAVDB.get().ok_or("navigation database not loaded")?.clone();
    let fac = FACILITIES.get().ok_or("facilities not loaded")?.clone();
    Ok(db
        .approaches(airport)
        .iter()
        .map(|a| {
            let rec = a.common_legs.last().and_then(|l| l.recommended_navaid.as_ref());
            format!("{} type {} navaid {:?} -> {:?}", a.ident, a.approach_type, rec.map(|r| r.database_id.clone()), rec.and_then(|r| fac.ils(&r.database_id)))
        })
        .chain(db.airport(airport).into_iter().flat_map(|ap| {
            let centre = fms_lnav::Coord::new(ap.location.lat, ap.location.long);
            let vhf = fac.nearby(fms_host::facilities::NearbyKind::VhfNavaid, centre, 60., 5);
            let first = vhf.first().and_then(|f| fac.vhf_navaid(&f.database_id));
            let ndb = fac.nearby(fms_host::facilities::NearbyKind::NdbNavaid, centre, 60., 3);
            let ndb_first = ndb.first().and_then(|f| fac.ndb_navaid(&f.database_id));
            let runways: Vec<String> = db.runways(airport).iter().map(|r| format!("{} {:?}", r.fix.ident, fac.runway_details(airport, &r.fix.ident))).collect();
            let mut lines = vec![format!("nearby vhf {vhf:?}"), format!("vhf_navaid {first:?}"), format!("nearby ndb {ndb:?}"), format!("ndb_navaid {ndb_first:?}")];
            lines.extend(runways);
            lines
        }))
        .collect())
}

/// X-Plane's navigation database once loaded.
#[allow(dead_code)]
pub fn navdb() -> Option<Db> {
    NAVDB.get().cloned()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum System {
    Fms,
}

impl System {
    pub fn name(self) -> &'static str {
        match self {
            System::Fms => "fms",
        }
    }
}

impl Compared for System {
    fn label(self) -> &'static str {
        self.name()
    }

    fn debounce(self) -> f64 {
        // GENERIC: the worker answers a frame late.
        2.
    }
}

pub fn unit_of(name: &str) -> &'static str {
    match name {
        "PLANE LATITUDE" | "PLANE LONGITUDE" | "GPS GROUND TRUE TRACK" | "PLANE HEADING DEGREES TRUE" | "MAGVAR" => "degrees",
        "GPS GROUND SPEED" | "AIRSPEED TRUE" => "knots",
        "PLANE ALT ABOVE GROUND" | "INDICATED ALTITUDE" | "AUTOPILOT ALTITUDE LOCK VAR:3" => "feet",
        "ZULU TIME" => "seconds",
        "FUEL TOTAL QUANTITY WEIGHT" => "pounds",
        "SIM ON GROUND" | "IS SLEW ACTIVE" => "bool",
        _ => "number",
    }
}

fn sim_name(name: &str) -> &str {
    name.strip_prefix("A:").or_else(|| name.strip_prefix("E:")).unwrap_or(name)
}

#[derive(Default)]
pub struct Shadow {
    comparator: Comparator<System>,
    gd_topics: BTreeMap<String, u64>,
}

impl Shadow {
    pub fn compare(&mut self, out: &Outputs, mut fbw: impl FnMut(&str) -> f64, now: f64, lines: &mut Vec<String>) {
        let mut latest: BTreeMap<&str, f64> = BTreeMap::new();
        for (name, value) in &out.vars {
            latest.insert(name, *value);
        }
        for (name, native) in latest {
            let js = fbw(&format!("L:{name}"));
            if let Some(line) = self.comparator.observe(System::Fms, name, native, js, now) {
                lines.push(line);
            }
        }
        for (topic, _) in &out.gd {
            *self.gd_topics.entry(topic.clone()).or_default() += 1;
        }
        if let Some(r) = &out.restarted {
            lines.push(format!("native shadow: fms restarted x{} ({})", r.count, r.reason));
        }
        let summary = self.comparator.summary(now);
        if !summary.is_empty() {
            lines.extend(summary);
            let gd: u64 = std::mem::take(&mut self.gd_topics).values().sum();
            lines.push(format!("native shadow: summary fms {gd} GENERIC_DATA topics"));
        }
    }
}

struct Running {
    host: FmsHost,
    shadow: Shadow,
    state: HostState,
    env: EnvRefs,
    resolved: HashMap<(String, String), Resolved>,
    last_time: Option<f64>,
    real_navdb: bool,
}

thread_local! {
    static RUNNING: std::cell::RefCell<Option<Running>> = const { std::cell::RefCell::new(None) };
}

static WIND_ROUTE: Mutex<(Option<f64>, Option<WindRoute>)> = Mutex::new((None, None));

/// The active plan as the wind uplink samples it: each terminating fix with
/// its climb/cruise/descent stage (by segment class), lat/lon fixes as FBW's
/// `formatWindRequest` names them, and the alternate's destination at the
/// alternate plan's cruise level.
pub fn wind_route(plan: &fms::plan::FlightPlan) -> WindRoute {
    use datalink::winds::WindFix;
    use fms::plan::SegmentClass;
    let mut points = Vec::new();
    for seg in &plan.segments {
        let stage = match seg.kind.class() {
            SegmentClass::Departure => RouteStage::Climb,
            SegmentClass::Enroute => RouteStage::Cruise,
            SegmentClass::Arrival => RouteStage::Descent,
        };
        for leg in seg.legs.iter().filter_map(|e| e.leg()) {
            let Some(w) = leg.termination_waypoint() else { continue };
            let fix = if w.icao_code.trim().is_empty() && w.database_id.starts_with("W      ") {
                WindFix::LatLon { lat: w.location.lat, long: w.location.long }
            } else {
                WindFix::Ident(leg.ident.clone())
            };
            points.push(RoutePoint { fix, latitude: w.location.lat, longitude: w.location.long, stage });
        }
    }
    let alternate = plan.alternate.as_deref().and_then(|a| {
        let dest = a.destination_airport.as_ref()?;
        Some((dest.location.lat, dest.location.long, a.performance.cruise_flight_level?))
    });
    WindRoute { points, alternate }
}

fn current_wind_route() -> Option<WindRoute> {
    WIND_ROUTE.lock().unwrap_or_else(|e| e.into_inner()).1.clone()
}

/// Whether the native FMS's outputs drive the aircraft: authoritative, and no
/// FlyByWire FMS in JS writing the same variables.
pub fn applies() -> bool {
    static APPLIES: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *APPLIES.get_or_init(|| {
        let m = crate::app_settings::native_mode::current();
        m.authoritative("fms") && m.native_all && std::env::var("FBW_JS_INSTRUMENTS").as_deref() != Ok("1")
    })
}

fn publish(out: &Outputs) {
    for (topic, data) in &out.gd {
        let text = data.to_string();
        crate::tod_pause::on_generic_data(topic, &text);
        crate::terrain_probe::tap_generic_data(topic, &text);
        #[cfg(feature = "native-displays")]
        crate::native_displays::inputs::tap_generic_data(topic, &text);
    }
    if !out.bus.is_empty() {
        let events: Vec<serde_json::Value> = out.bus.iter().map(|(t, d)| serde_json::json!({ "topic": t, "data": d })).collect();
        let batch = serde_json::json!({ "events": events }).to_string();
        crate::terrain_probe::tap_generic_data("xp-ui.bus", &batch);
        #[cfg(feature = "native-displays")]
        crate::native_displays::inputs::tap_generic_data("xp-ui.bus", &batch);
    }
}

/// Once a sim frame. Returns the K: events the native FMS sent, for the
/// caller's event loop, when it drives the aircraft ([`applies`]).
pub fn tick(vars: &mut Vars, time: f64) -> Vec<(String, f64)> {
    let mut lines = Vec::new();
    let mut writes: Vec<(String, f64)> = Vec::new();
    let mut keys: Vec<(String, f64)> = Vec::new();
    let applies = applies();
    start_navdb();
    static WIND_PROVIDER: std::sync::Once = std::sync::Once::new();
    WIND_PROVIDER.call_once(|| crate::datalink_native::set_wind_route_provider(Box::new(current_wind_route)));
    #[cfg(feature = "native-displays")]
    let mut mfd_writes = Vec::new();
    RUNNING.with(|cell| {
        let mut guard = cell.borrow_mut();
        let running = guard.get_or_insert_with(|| Running {
            host: FmsHost::new(Arc::new(MemoryNavDb::default())),
            shadow: Shadow::default(),
            state: HostState::default(),
            env: EnvRefs::new(vars.xplm),
            resolved: HashMap::new(),
            last_time: None,
            real_navdb: false,
        });
        if !running.real_navdb {
            if let Some(db) = NAVDB.get() {
                running.host = match FACILITIES.get() {
                    Some(f) => FmsHost::with_facilities(db.clone(), running.host.snapshot(), f.clone()),
                    None => FmsHost::with_snapshot(db.clone(), running.host.snapshot()),
                };
                running.real_navdb = true;
                lines.push("native fms: X-Plane navigation database in use".to_string());
            }
        }
        let dt_ms = running.last_time.map_or(0., |t| ((time - t) * 1000.).clamp(0., MAX_STEP_MS));
        running.last_time = Some(time);
        let mut host = VarsHost::new(vars, &mut running.state, &running.env, time);
        let resolved = &mut running.resolved;
        let mut get = |name: &str| {
            let name = sim_name(name);
            let unit = unit_of(name);
            let r = *resolved.entry((name.to_string(), unit.to_string())).or_insert_with(|| host.resolve(name, unit));
            host.read(name, r)
        };
        let out = running.host.take_outputs();
        if applies {
            publish(&out);
            writes.extend(out.vars.iter().cloned());
            keys.extend(out.keys.iter().cloned());
        } else {
            running.shadow.compare(&out, &mut get, time, &mut lines);
        }
        if let Some(v) = out.last_gd("A32NX_EFIS_VECTORS_L_ACTIVE") {
            crate::terrain_probe::fms_vectors(v);
        }
        if let Some(v) = out.last_bus("a32nx_fms_vd_track_change_distance") {
            crate::terrain_probe::fms_track_change(v.as_f64());
        }
        running.host.tick(dt_ms, Inputs::from_vars(&mut get));
        {
            let mut wind = WIND_ROUTE.lock().unwrap_or_else(|e| e.into_inner());
            if wind.0.map_or(true, |t| time < t || time - t >= 1.0) {
                wind.0 = Some(time);
                wind.1 = running.host.snapshot().and_then(|s| s.plans.into_iter().next().flatten()).map(|p| wind_route(&p));
            }
        }
        #[cfg(feature = "native-displays")]
        if crate::oit_native_host::fms_due(time) {
            if let Some(s) = running.host.snapshot() {
                crate::oit_native_host::set_fms(crate::oit_native_host::fms_info(s.plans.first().and_then(Option::as_ref)));
            }
        }
        #[cfg(feature = "native-displays")]
        if crate::native_displays::input::is_native("SCREEN_DU_MFD") || crate::mfd_native_host::offline() {
            let get = std::cell::RefCell::new(&mut get);
            let read = |name: &str| (get.borrow_mut())(name);
            mfd_writes = crate::mfd_native_host::tick(&read, Some(&mut running.host), &out.messages);
        }
    });
    #[cfg(feature = "native-displays")]
    for (name, value) in mfd_writes {
        let id = vars.add(name.trim_start_matches("L:").to_string(), crate::NAMED);
        systems::simulation::SimulatorReaderWriter::write(vars, &id, value);
    }
    for (name, value) in writes {
        let id = vars.add(name.trim_start_matches("L:").to_string(), crate::NAMED);
        systems::simulation::SimulatorReaderWriter::write(vars, &id, value);
    }
    for line in lines {
        crate::log(&line);
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sustained_difference_is_logged_once_and_summarised() {
        let mut s = Shadow::default();
        let out = Outputs { vars: vec![("A32NX_FM_TEST".into(), 2.)], gd: vec![("A32NX_EFIS_L_TEST".into(), serde_json::json!(1))], ..Default::default() };
        let mut lines = Vec::new();
        s.compare(&out, |_| 1., 0., &mut lines);
        assert!(lines.is_empty());
        s.compare(&out, |_| 1., 3., &mut lines);
        assert_eq!(lines, vec!["native shadow: fms A32NX_FM_TEST native=2 fbw=1".to_string()]);
        lines.clear();
        s.compare(&out, |_| 1., 11., &mut lines);
        assert!(lines.iter().any(|l| l.starts_with("native shadow: summary fms 1 variables differ")));
        assert!(lines.iter().any(|l| l == "native shadow: summary fms 3 GENERIC_DATA topics"));
    }

    #[test]
    fn matching_values_stay_quiet() {
        let mut s = Shadow::default();
        let out = Outputs { vars: vec![("A32NX_FM_TEST".into(), 1.)], ..Default::default() };
        let mut lines = Vec::new();
        for t in 0..5 {
            s.compare(&out, |name| if name == "L:A32NX_FM_TEST" { 1. } else { 0. }, t as f64, &mut lines);
        }
        assert!(lines.is_empty());
    }

    #[test]
    fn wind_route_stages_fixes_by_segment_and_names_lat_lon_fixes() {
        use datalink::winds::WindFix;
        use fms::plan::{Element, FlightPlan, Leg, LegType, SegmentKind};
        let mut plan = FlightPlan::empty(0);
        let fix = |id: &str, lat: f64, long: f64| Fix { ident: id.into(), icao_code: "EG".into(), location: fms::navdb::Location { lat, long, alt: None }, ..Fix::default() };
        let put = |plan: &mut FlightPlan, kind: SegmentKind, f: &Fix| {
            plan.segments[kind.slot()].legs.push(Element::Leg(Box::new(Leg::from_enroute_fix(f, None, LegType::TF))));
        };
        put(&mut plan, SegmentKind::Departure, &fix("CPT", 51.5, -1.2));
        put(&mut plan, SegmentKind::Enroute, &Fix::lat_long("N52W010", 52.0, -10.0));
        put(&mut plan, SegmentKind::Arrival, &fix("BIG", 51.3, 0.03));
        let mut alt = FlightPlan::empty(1);
        alt.destination_airport = Some(fix("EGCC", 53.35, -2.27));
        alt.performance.cruise_flight_level = Some(200.);
        plan.alternate = Some(Box::new(alt));
        let r = wind_route(&plan);
        let stages: Vec<_> = r.points.iter().map(|p| p.stage).collect();
        assert_eq!(stages, vec![RouteStage::Climb, RouteStage::Cruise, RouteStage::Descent]);
        assert_eq!(r.points[0].fix, WindFix::Ident("CPT".into()));
        assert_eq!(r.points[1].fix, WindFix::LatLon { lat: 52.0, long: -10.0 });
        assert_eq!(r.alternate, Some((53.35, -2.27, 200.)));
    }

    #[test]
    fn sim_variables_get_their_units() {
        assert_eq!(sim_name("A:PLANE LATITUDE"), "PLANE LATITUDE");
        assert_eq!(sim_name("E:ZULU TIME"), "ZULU TIME");
        assert_eq!(unit_of("PLANE LATITUDE"), "degrees");
        assert_eq!(unit_of("L:A32NX_FMA_LATERAL_MODE"), "number");
    }
}

//! Native owner of FlyByWire's OANS control panels (one per ND side) and the
//! brake-to-vacate logic, for native NDs (`crates/oans`). The ND's input
//! handler sends `A380D_OANS_<L|R>_<ACTION>` H events; the panel model the
//! native ND draws goes out on `a380.oans.panel.<1|2>`.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use oans::arinc::Arinc;
use oans::btv::{BtvInputs, NavDb, Oans, Runway};
use oans::control_panel::{AirportSearchMode, AmdbAirport, AmdbSource, ControlPanel, FmsAirport, MapDataFeature, MapDataSearchMode};
use oans::geo::{distance_to, Coordinates};
use oans::markers::{MarkerManager, Symbol};
use oans::oanc::{Feature, Geometry, Props};
use serde_json::{json, Value};

const PANEL_PERIOD_MS: f64 = 2000.0;
const TAB_TITLES: [&str; 3] = ["MAP DATA", "ARPT SEL", "STATUS"];
const ND_MAP_QUERIES: [&str; 2] = [
    "?projection=NAVIGRAPH:ARP_AZEQ&format=geojson&exclude=&include=taxiwayelement,taxiwayshoulder,serviceroad,runwayelement,runwayintersection,blastpad,runwaydisplacedarea,stopway,runwaymarking,runwayshoulder,taxiwayguidanceline,runwayexitline,taxiwayholdingposition,apronelement,parkingstandarea,verticalpolygonalstructure,verticalpolygonalstructure,taxiwayguidanceline,runwayexitline,runwayelement,runwayintersection,runwaydisplacedarea,stopway,standguidanceline,aerodromereferencepoint,parkingstandlocation,paintedcenterline,runwaythreshold",
    "?projection=EPSG:4326&format=geojson&exclude=&include=aerodromereferencepoint",
];
const ND_MAP_REFEED_MS: f64 = 10_000.0;
const FEATURE_TYPES: [&str; 4] = ["runwaythreshold", "taxiwayguidanceline", "parkingstandlocation", "deicingarea"];

pub enum Fetch {
    Pending,
    Ready(String),
    Failed,
}

pub trait HostIo {
    fn var(&mut self, name: &str) -> f64;
    fn set_var(&mut self, name: &str, value: f64);
    fn bus(&mut self, topic: &str) -> Option<Value>;
    fn publish(&mut self, events: &[(String, Value)]);
    /// `path_and_query` after the AMDB host, e.g. `/v1/search?q=`.
    fn amdb(&mut self, path_and_query: &str) -> Fetch;
    /// Hands an AMDB answer to the native NDs, which draw the airport from it.
    fn to_displays(&mut self, _path_and_query: &str, _body: &str) {}
}

#[derive(Default)]
pub struct AmdbCache {
    airports: Option<Vec<AmdbAirport>>,
    features: BTreeMap<(String, String), Vec<MapDataFeature>>,
    runways: BTreeMap<String, Vec<Runway>>,
}

#[derive(Clone)]
pub struct Shared(Rc<RefCell<AmdbCache>>);

impl AmdbSource for Shared {
    fn search_airports(&self) -> Result<Vec<AmdbAirport>, String> {
        self.0.borrow().airports.clone().ok_or_else(|| "not loaded".to_string())
    }

    fn map_data(&self, icao: &str, feature_type: &str) -> Option<Vec<MapDataFeature>> {
        self.0.borrow().features.get(&(icao.to_string(), feature_type.to_string())).cloned()
    }
}

impl NavDb for Shared {
    fn airport_location(&self, icao: &str) -> Option<Coordinates> {
        self.0.borrow().airports.as_ref()?.iter().find(|a| a.idarpt == icao).map(|a| a.coordinates)
    }

    fn runways(&self, icao: &str) -> Vec<Runway> {
        self.0.borrow().runways.get(icao).cloned().unwrap_or_default()
    }

    fn nearby_airports(&self, ppos: Coordinates, radius_nm: f64, max_results: usize) -> Vec<String> {
        let c = self.0.borrow();
        let mut near: Vec<(f64, String)> =
            c.airports.iter().flatten().map(|a| (distance_to(ppos, a.coordinates), a.idarpt.clone())).filter(|(d, _)| *d <= radius_nm).collect();
        near.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        near.into_iter().take(max_results).map(|(_, i)| i).collect()
    }
}

pub fn parse_airports(text: &str) -> Option<Vec<AmdbAirport>> {
    let list: Vec<Value> = serde_json::from_str(text).ok()?;
    Some(
        list.iter()
            .filter_map(|a| {
                Some(AmdbAirport {
                    idarpt: a.get("idarpt")?.as_str()?.to_string(),
                    iata: a.get("iata").and_then(Value::as_str).map(str::to_string),
                    name: a.get("name").and_then(Value::as_str).map(str::to_string),
                    coordinates: Coordinates {
                        lat: a.pointer("/coordinates/lat").and_then(Value::as_f64)?,
                        long: a.pointer("/coordinates/lon").and_then(Value::as_f64)?,
                    },
                })
            })
            .collect(),
    )
}

fn point(v: Option<&Value>) -> Option<[f64; 2]> {
    let c = v?.get("coordinates")?.as_array()?;
    Some([c.first()?.as_f64()?, c.get(1)?.as_f64()?])
}

pub fn parse_features(text: &str, layer: &str) -> Vec<MapDataFeature> {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return Vec::new() };
    let s = |p: &Value, k: &str| p.get(k).and_then(|x| x.as_str().map(str::to_string).or_else(|| x.as_f64().map(|n| n.to_string())));
    v.pointer(&format!("/{layer}/features"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|f| {
            let p = f.get("properties").cloned().unwrap_or(Value::Null);
            MapDataFeature {
                id: p.get("id").and_then(Value::as_i64),
                feattype: p.get("feattype").and_then(Value::as_u64).map(|t| t as u8),
                idthr: s(&p, "idthr"),
                idlin: s(&p, "idlin"),
                idstd: s(&p, "idstd"),
                ident: s(&p, "ident"),
                lda: p.get("lda").and_then(Value::as_f64),
                tora: p.get("tora").and_then(Value::as_f64),
                point: point(f.get("geometry")),
                midpoint: point(p.get("midpoint")),
            }
        })
        .collect()
}

pub fn parse_runways(icao: &str, text: &str) -> Vec<Runway> {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return Vec::new() };
    v.pointer("/runwaythreshold/features")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let p = f.get("properties")?;
            let [lon, lat] = point(f.get("geometry"))?;
            Some(Runway {
                ident: format!("{icao}{}", p.get("idthr")?.as_str()?),
                threshold_location: Coordinates { lat, long: lon },
                bearing: p.get("brngtrue").or_else(|| p.get("brngmag")).and_then(Value::as_f64).unwrap_or(0.0),
                length: p.get("lda").and_then(Value::as_f64).unwrap_or(0.0),
            })
        })
        .collect()
}

pub fn parse_marker_features(text: &str) -> Vec<Feature> {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return Vec::new() };
    let pos = |c: &Value| Some([c.get(0)?.as_f64()?, c.get(1)?.as_f64()?]);
    let mut out = Vec::new();
    for layer in ["runwaythreshold", "parkingstandlocation", "taxiwayguidanceline", "runwayexitline"] {
        for f in v.pointer(&format!("/{layer}/features")).and_then(Value::as_array).into_iter().flatten() {
            let p = f.get("properties").cloned().unwrap_or(Value::Null);
            let Some(feattype) = p.get("feattype").and_then(Value::as_u64).and_then(|t| u8::try_from(t).ok()) else { continue };
            let Some(g) = f.get("geometry") else { continue };
            let c = g.get("coordinates").cloned().unwrap_or(Value::Null);
            let geometry = match g.get("type").and_then(Value::as_str) {
                Some("Point") => match pos(&c) {
                    Some(p) => Geometry::Point(p),
                    None => continue,
                },
                Some("LineString") => {
                    let line: Vec<[f64; 2]> = c.as_array().into_iter().flatten().filter_map(pos).collect();
                    if line.is_empty() {
                        continue;
                    }
                    Geometry::LineString(line)
                }
                _ => continue,
            };
            let s = |k: &str| p.get(k).and_then(|x| x.as_str().map(str::to_string).or_else(|| x.as_f64().map(|n| n.to_string())));
            let props = Props { id: p.get("id").and_then(Value::as_i64), idlin: s("idlin"), idstd: s("idstd"), ident: s("ident"), idthr: s("idthr"), ..Props::default() };
            out.push(Feature { feattype, props, geometry });
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Menu {
    Entity,
    Airport,
}

const MENU_ROWS: usize = 4;
const MENU_ROW_H: f64 = 100.0 / 3.0;
const SCREEN_H: f64 = 1024.0;
const PANEL_Y: f64 = 768.0;

struct Side {
    letter: &'static str,
    cp: ControlPanel,
    open: bool,
    menu: Option<(Menu, usize)>,
    last_model: String,
    markers: MarkerManager,
    pan: Option<(u32, [f64; 2])>,
    nd_mode: f64,
}

impl Side {
    fn new(letter: &'static str) -> Self {
        Side { letter, cp: ControlPanel::new(letter), open: false, menu: None, last_model: String::new(), markers: MarkerManager::new(), pan: None, nd_mode: -1.0 }
    }

    fn menu_len(&self, m: Menu) -> usize {
        match m {
            Menu::Entity => self.cp.available_entity_list.len(),
            Menu::Airport => self.cp.airport_search_data.len(),
        }
    }

    fn menu_selected(&self, m: Menu) -> Option<usize> {
        match m {
            Menu::Entity => self.cp.selected_entity_index(),
            Menu::Airport => self.cp.selected_airport_index.and_then(|i| usize::try_from(i).ok()),
        }
    }

    fn open_menu(&mut self, m: Menu) {
        if self.menu.is_some_and(|(k, _)| k == m) {
            self.menu = None;
            return;
        }
        let len = self.menu_len(m);
        let first = self.menu_selected(m).unwrap_or(0).min(len.saturating_sub(MENU_ROWS));
        self.menu = Some((m, first));
    }

    fn scroll_menu(&mut self, down: bool) {
        if let Some((m, first)) = self.menu {
            let last = self.menu_len(m).saturating_sub(MENU_ROWS);
            let next = if down { (first + 1).min(last) } else { first.saturating_sub(1) };
            self.menu = Some((m, next));
        }
    }
}

fn degrees_minutes(v: f64, width: usize, pos: char, neg: char) -> String {
    if !v.is_finite() {
        return String::new();
    }
    let tenths = (v.abs() * 600.0).round() as i64;
    let (deg, min) = (tenths / 600, tenths % 600);
    format!("{deg:0width$}\u{b0}{:02}.{}{}", min / 10, min % 10, if v < 0.0 { neg } else { pos })
}

pub struct OansHost {
    cache: Shared,
    sides: [Side; 2],
    btv: Oans<'static>,
    airports_loaded: bool,
    airports_retry_ms: f64,
    shown: [Option<String>; 2],
    ppos_logged: bool,
    pending_map_data: Option<String>,
    nd_maps: std::collections::HashMap<String, Vec<(String, String)>>,
    nd_maps_ms: f64,
    since_panel_ms: f64,
    last_fms_landing: Option<Option<String>>,
    pan_seq: u32,
    marker_features: std::collections::HashMap<String, Rc<Vec<Feature>>>,
}

fn arinc(io: &mut dyn HostIo, name: &str) -> Arinc {
    Arinc::from_raw(io.var(name))
}

fn bus_str(io: &mut dyn HostIo, topic: &str) -> Option<String> {
    io.bus(topic).and_then(|v| v.as_str().map(str::to_string)).filter(|s| !s.is_empty())
}

impl Default for OansHost {
    fn default() -> Self {
        Self::new()
    }
}

impl OansHost {
    pub fn new() -> Self {
        let cache = Shared(Rc::new(RefCell::new(AmdbCache::default())));
        let nav: &'static Shared = Box::leak(Box::new(cache.clone()));
        Self {
            cache,
            sides: [
                Side::new("L"),
                Side::new("R"),
            ],
            btv: Oans::new(nav),
            airports_loaded: false,
            airports_retry_ms: 0.0,
            shown: [None, None],
            ppos_logged: false,
            pending_map_data: None,
            nd_maps: Default::default(),
            nd_maps_ms: 0.0,
            since_panel_ms: PANEL_PERIOD_MS,
            last_fms_landing: None,
            pan_seq: 0,
            marker_features: Default::default(),
        }
    }

    fn features_of(&mut self, i: usize) -> Option<Rc<Vec<Feature>>> {
        let icao = self.shown[i].clone()?;
        if let Some(f) = self.marker_features.get(&icao) {
            return Some(f.clone());
        }
        let text = self.nd_maps.get(&icao)?.first()?.1.clone();
        let f = Rc::new(parse_marker_features(&text));
        self.marker_features.insert(icao, f.clone());
        Some(f)
    }

    fn apply_map_event(&mut self, topic: &str, value: &Value) {
        let at = |v: &Value| Some((v.get("id")?.as_i64()?, u8::try_from(v.get("feattype")?.as_u64()?).ok()?));
        for i in 0..2 {
            match topic {
                "oans_center_map_on" => {
                    if let Some(p) = value.as_array().and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])) {
                        self.pan_seq += 1;
                        self.sides[i].pan = Some((self.pan_seq, p));
                    }
                }
                "oans_add_cross_at_feature" | "oans_add_flag_at_feature" => {
                    let (Some((id, ft)), Some(features)) = (at(value), self.features_of(i)) else { continue };
                    let kind = if topic.contains("cross") { Symbol::Cross } else { Symbol::Flag };
                    self.sides[i].markers.add_symbol_at_feature(kind, &features, id, ft);
                }
                "oans_remove_cross_at_feature" | "oans_remove_flag_at_feature" => {
                    let Some((id, ft)) = at(value) else { continue };
                    let kind = if topic.contains("cross") { Symbol::Cross } else { Symbol::Flag };
                    self.sides[i].markers.remove_symbol_at_feature(kind, id, ft);
                }
                _ => return,
            }
        }
    }

    pub fn side(&self, i: usize) -> &ControlPanel {
        &self.sides[i].cp
    }

    pub fn panel_open(&self, i: usize) -> bool {
        self.sides[i].open
    }

    fn load_airports(&mut self, io: &mut dyn HostIo, delta_ms: f64) {
        if self.airports_loaded {
            return;
        }
        if self.airports_retry_ms > 0.0 {
            self.airports_retry_ms -= delta_ms;
            return;
        }
        let list = match io.amdb("/v1/search?q=") {
            Fetch::Pending => return,
            Fetch::Ready(t) => parse_airports(&t),
            Fetch::Failed => None,
        };
        match &list {
            Some(l) if !l.is_empty() => crate::log(&format!("oans: {} airports in the AMDB list", l.len())),
            _ => {
                crate::log("oans: the AMDB airport list did not load; trying again in 10 s");
                self.airports_retry_ms = 10_000.0;
                return;
            }
        }
        self.airports_loaded = true;
        self.cache.0.borrow_mut().airports = list;
        let src = self.cache.clone();
        for s in &mut self.sides {
            s.cp.load_oans_db(&src);
        }
    }

    fn load_map_data(&mut self, io: &mut dyn HostIo) {
        let Some(icao) = self.pending_map_data.clone() else { return };
        let mut pending = false;
        for layer in FEATURE_TYPES {
            if self.cache.0.borrow().features.contains_key(&(icao.clone(), layer.to_string())) {
                continue;
            }
            match io.amdb(&format!("/v1/{icao}?projection=NAVIGRAPH:ARP_AZEQ&format=geojson&exclude=&include={layer}")) {
                Fetch::Pending => pending = true,
                Fetch::Ready(t) => {
                    self.cache.0.borrow_mut().features.insert((icao.clone(), layer.to_string()), parse_features(&t, layer));
                }
                Fetch::Failed => {
                    self.cache.0.borrow_mut().features.insert((icao.clone(), layer.to_string()), Vec::new());
                }
            }
        }
        if !self.cache.0.borrow().runways.contains_key(&icao) {
            match io.amdb(&format!("/v1/{icao}?projection=EPSG:4326&format=geojson&exclude=&include=runwaythreshold")) {
                Fetch::Pending => pending = true,
                Fetch::Ready(t) => {
                    self.cache.0.borrow_mut().runways.insert(icao.clone(), parse_runways(&icao, &t));
                }
                Fetch::Failed => {
                    self.cache.0.borrow_mut().runways.insert(icao.clone(), Vec::new());
                }
            }
        }
        if pending {
            return;
        }
        self.pending_map_data = None;
        if let Some(Some(rwy)) = self.last_fms_landing.clone().filter(|r| r.as_deref().is_some_and(|r| r.starts_with(icao.as_str()))) {
            self.btv.set_fms_landing_runway(None);
            self.btv.set_fms_landing_runway(Some(&rwy));
        }
        let src = self.cache.clone();
        for s in &mut self.sides {
            if s.cp.selected_airport.as_ref().is_some_and(|a| a.idarpt == icao) {
                let mode = s.cp.selected_entity_type;
                s.cp.handle_select_map_data_search_mode(&src, mode);
            }
        }
    }

    fn feed_nd_maps(&mut self, io: &mut dyn HostIo, delta_ms: f64) {
        let wanted: Vec<String> = self.shown.iter().flatten().filter(|icao| !self.nd_maps.contains_key(*icao)).cloned().collect();
        for icao in wanted {
            let mut got = Vec::new();
            for q in ND_MAP_QUERIES {
                let pq = format!("/v1/{icao}{q}");
                match io.amdb(&pq) {
                    Fetch::Ready(t) => got.push((pq, t)),
                    Fetch::Pending => {}
                    Fetch::Failed => crate::log(&format!("oans: {icao} map fetch failed ({pq})")),
                }
            }
            if got.len() == ND_MAP_QUERIES.len() {
                crate::log(&format!("oans: {icao} map handed to the NDs ({} bytes)", got.iter().map(|(_, t)| t.len()).sum::<usize>()));
                for (pq, t) in &got {
                    io.to_displays(pq, t);
                }
                self.nd_maps.insert(icao, got);
            }
        }
        self.nd_maps_ms += delta_ms;
        if self.nd_maps_ms >= ND_MAP_REFEED_MS {
            self.nd_maps_ms = 0.0;
            for got in self.nd_maps.values() {
                for (pq, t) in got {
                    io.to_displays(pq, t);
                }
            }
        }
    }

    fn want_map_data(&mut self, i: usize) {
        if let Some(a) = self.sides[i].cp.selected_airport.as_ref() {
            let c = self.cache.0.borrow();
            if !FEATURE_TYPES.iter().all(|l| c.features.contains_key(&(a.idarpt.clone(), l.to_string()))) || !c.runways.contains_key(&a.idarpt) {
                self.pending_map_data = Some(a.idarpt.clone());
            }
        }
    }

    /// One ND event (`A380D_OANS_L_TAB_1`, ...). Returns whether it was ours.
    pub fn handle_event(&mut self, name: &str, sim_time_s: f64) -> bool {
        let Some(rest) = name.strip_prefix("A380D_OANS_") else { return false };
        let (i, action) = match rest.split_once('_') {
            Some(("L", a)) => (0, a),
            Some(("R", a)) => (1, a),
            _ => return false,
        };
        let src = self.cache.clone();
        let s = &mut self.sides[i];
        let num = |p: &str| action.strip_prefix(p).and_then(|n| n.parse::<usize>().ok());
        match action {
            "MENU_ENTITY" if !s.cp.fms_landing_runway_not_selected_in_fallback() => return { s.open_menu(Menu::Entity); true },
            "MENU_AIRPORT" => return { s.open_menu(Menu::Airport); true },
            "MENU_UP" => return { s.scroll_menu(false); true },
            "MENU_DOWN" => return { s.scroll_menu(true); true },
            "MENU_CLOSE" => return { s.menu = None; true },
            "TOGGLE" => {
                s.open = !s.open;
                s.menu = None;
            }
            "SWAP" if !s.cp.oans_available() => {
                self.airports_loaded = false;
            }
            "DISPLAY_AIRPORT" => {
                let _ = s.cp.handle_display_airport(&src, sim_time_s);
            }
            "ORIGIN" => {
                let _ = s.cp.select_fms_airport(&src, FmsAirport::Origin);
            }
            "DEST" => {
                let _ = s.cp.select_fms_airport(&src, FmsAirport::Destination);
            }
            "ALTN" => {
                let _ = s.cp.select_fms_airport(&src, FmsAirport::Alternate);
            }
            "CROSS" if !s.cp.entity_is_not_selected() => s.cp.handle_cross_button(),
            "FLAG" if !s.cp.entity_is_not_selected() => s.cp.handle_flag_button(),
            "CENTER" if !s.cp.entity_is_not_selected() => s.cp.center_map_on_selected(),
            _ => {
                if let Some(n) = num("TAB_").filter(|n| *n < 3) {
                    s.cp.active_tab_index = n;
                    s.menu = None;
                } else if let Some(n) = num("SEARCH_MODE_") {
                    s.cp.handle_select_airport_search_mode(match n {
                        0 => AirportSearchMode::Icao,
                        1 => AirportSearchMode::Iata,
                        _ => AirportSearchMode::City,
                    });
                } else if let Some(n) = num("AIRPORT_") {
                    s.menu = None;
                    let _ = s.cp.select_airport_from_dropdown(&src, Some(n));
                } else if let Some(n) = num("ENTITY_TYPE_") {
                    let mode = match n {
                        0 => MapDataSearchMode::Runway,
                        1 => MapDataSearchMode::Taxiway,
                        2 => MapDataSearchMode::Stand,
                        _ => MapDataSearchMode::Other,
                    };
                    s.cp.set_selected_entity_type(&src, mode);
                } else if let Some(n) = num("ENTITY_") {
                    s.menu = None;
                    if !s.cp.fms_landing_runway_not_selected_in_fallback() && n < s.cp.available_entity_list.len() {
                        s.cp.set_selected_entity_index(Some(n));
                    }
                } else {
                    return true;
                }
            }
        }
        self.want_map_data(i);
        true
    }

    fn model(&self, i: usize) -> Value {
        let s = &self.sides[i];
        let cp = &s.cp;
        let ev = |a: &str| format!("A380D_OANS_{}_{a}", s.letter);
        let mut hits = vec![json!({"r": [717, 768, 49, 45], "ev": ev("TOGGLE")})];
        if s.open {
            for (t, _) in TAB_TITLES.iter().enumerate() {
                hits.push(json!({"r": [9.0 + t as f64 * 230.67, 770, 245.67, 45], "ev": ev(&format!("TAB_{t}"))}));
            }
            if cp.active_tab_index == 2 && !cp.oans_available() {
                hits.push(json!({"r": [328, 825.333, 119.333, 63.333], "ev": ev("SWAP")}));
            }
            let y = |v: f64| PANEL_Y + v;
            if cp.active_tab_index == 0 {
                if !cp.fms_landing_runway_not_selected_in_fallback() {
                    hits.push(json!({"r": [21.33, y(52.0), 168.0, 44.0], "ev": ev("MENU_ENTITY")}));
                }
                for k in 0..4 {
                    hits.push(json!({"r": [21.33, y(106.0 + 35.0 * k as f64), 160.0, 32.0], "ev": ev(&format!("ENTITY_TYPE_{k}"))}));
                }
                if cp.oans_available() && !cp.entity_is_not_selected() {
                    hits.push(json!({"r": [209.5, y(52.0), 171.5, 44.0], "ev": ev("CROSS")}));
                    hits.push(json!({"r": [391.0, y(52.0), 171.5, 44.0], "ev": ev("FLAG")}));
                    hits.push(json!({"r": [326.75, y(106.0), 300.0, 44.0], "ev": ev("CENTER")}));
                }
            } else if cp.active_tab_index == 1 {
                hits.push(json!({"r": [21.33, y(52.0), 215.8, 44.0], "ev": ev("MENU_AIRPORT")}));
                for k in 0..3 {
                    hits.push(json!({"r": [21.33, y(116.0 + 35.0 * k as f64), 200.0, 32.0], "ev": ev(&format!("SEARCH_MODE_{k}"))}));
                }
                hits.push(json!({"r": [267.1, y(193.0), 316.4, 44.0], "ev": ev("DISPLAY_AIRPORT")}));
                for (k, which) in ["ORIGIN", "DEST", "ALTN"].into_iter().enumerate() {
                    hits.push(json!({"r": [628.75, y(67.0 + 60.5 * k as f64), 120.0, 44.0], "ev": ev(which)}));
                }
            }
        }
        let menu = s.menu.filter(|_| s.open).map(|(m, first)| {
            let (items, x, w, ev_prefix): (Vec<String>, f64, f64, &str) = match m {
                Menu::Entity => (cp.available_entity_list.clone(), 21.33, 168.0, "ENTITY_"),
                Menu::Airport => (cp.airport_search_data.clone(), 21.33, 215.8, "AIRPORT_"),
            };
            let total = items.len();
            let row = MENU_ROW_H;
            let top = PANEL_Y + 52.0 + 44.0;
            let h = (row * total as f64 + 8.0 / 3.0).min(275.0);
            let bottom = (top + h).min(SCREEN_H);
            let scroll = total > MENU_ROWS;
            let item_w = if scroll { w - 24.0 } else { w - 4.0 };
            let shown: Vec<String> = items.into_iter().skip(first).take(MENU_ROWS + 1).collect();
            hits.push(json!({"r": [0, 0, 768, 1024], "ev": ev("MENU_CLOSE")}));
            hits.push(json!({"r": [x, top, w, bottom - top], "ev": "", "wu": ev("MENU_UP"), "wd": ev("MENU_DOWN")}));
            for (k, _) in shown.iter().enumerate() {
                let y0 = top + 2.0 + row * k as f64;
                if y0 >= bottom - 2.0 {
                    break;
                }
                hits.push(json!({"r": [x + 2.0, y0, item_w, row.min(bottom - 2.0 - y0)], "ev": ev(&format!("{ev_prefix}{}", first + k)), "wu": ev("MENU_UP"), "wd": ev("MENU_DOWN")}));
            }
            if scroll {
                let track = bottom - 2.0 - (top + 2.0);
                hits.push(json!({"r": [x + w - 22.0, top + 2.0, 20.0, track / 2.0], "ev": ev("MENU_UP")}));
                hits.push(json!({"r": [x + w - 22.0, top + 2.0 + track / 2.0, 20.0, track / 2.0], "ev": ev("MENU_DOWN")}));
            }
            json!({
                "kind": if m == Menu::Entity { "entity" } else { "airport" },
                "x": x, "y": top, "w": w, "h": h, "row": row, "rows": MENU_ROWS, "scroll": scroll,
                "first": first, "total": total, "items": shown,
                "selected": s.menu_selected(m),
            })
        });
        let db = cp.active_database.clone();
        let mut st = cp.state_json();
        if let Some(o) = st.as_object_mut() {
            o.remove("searchData");
        }
        st["airportValue"] = json!(cp.selected_airport_index.and_then(|i| usize::try_from(i).ok()).and_then(|i| cp.airport_search_data.get(i)).cloned().unwrap_or_default());
        st["mode"] = json!(cp.airport_search_mode as u8);
        st["name"] = json!(cp.selected_airport_name());
        st["idents"] = json!(cp.selected_airport_idents());
        st["origin"] = json!(cp.fms_button_label(FmsAirport::Origin));
        st["dest"] = json!(cp.fms_button_label(FmsAirport::Destination));
        st["altn"] = json!(cp.fms_button_label(FmsAirport::Alternate));
        st["coords"] = json!(cp
            .selected_airport
            .as_ref()
            .map(|a| format!("{}/{}", degrees_minutes(a.coordinates.lat, 2, 'N', 'S'), degrees_minutes(a.coordinates.long, 3, 'E', 'W')))
            .unwrap_or_default());
        json!({
            "open": s.open,
            "tabs": TAB_TITLES,
            "active": cp.active_tab_index,
            "status": {"active": db, "second": db, "swapDisabled": cp.oans_available(), "database": cp.airport_database()},
            "arptSel": st,
            "mapData": {
                "entityType": cp.selected_entity_type as u8,
                "entities": cp.available_entity_list,
                "entityIndex": cp.selected_entity_index(),
                "entity": cp.selected_entity_string,
                "cross": if cp.cross_exists_for_entity() { "DEL CROSS" } else { "ADD CROSS" },
                "flag": if cp.flag_exists_for_entity() { "DEL FLAG" } else { "ADD FLAG" },
                "buttonsDisabled": cp.entity_is_not_selected(),
                "lda": ControlPanel::runway_length_text(cp.runway_lda, true),
                "tora": ControlPanel::runway_length_text(cp.runway_tora, true),
                "fallback": !cp.oans_available(),
                "selectLandingRunway": cp.fms_landing_runway_not_selected_in_fallback(),
                "reqStoppingDistance": cp.req_stopping_distance(),
            },
            "menu": menu,
            "markers": s.markers.crosses().iter().map(|m| json!({"k": "c", "p": m.position})).chain(s.markers.flags().iter().map(|m| json!({"k": "f", "p": m.position}))).collect::<Vec<_>>(),
            "pan": s.pan.map(|(seq, p)| json!({"seq": seq, "p": p})),
            "hits": hits,
        })
    }

    pub fn tick(&mut self, io: &mut dyn HostIo, delta_ms: f64, time_ms: f64, sim_time_s: f64, h_events: &[String]) {
        self.load_airports(io, delta_ms);
        self.load_map_data(io);
        for e in h_events {
            self.handle_event(e, sim_time_s);
        }

        let phase = io.var("L:A32NX_FWC_FLIGHT_PHASE");
        let reset = io.var("L:A380X_RESET_PANEL_ARPT_NAV") != 0.0;
        let (lat, long) = (arinc(io, "L:A32NX_ADIRS_IR_1_LATITUDE"), arinc(io, "L:A32NX_ADIRS_IR_1_LONGITUDE"));
        let fms = (bus_str(io, "fmsOrigin"), bus_str(io, "fmsDestination"), bus_str(io, "fmsAlternate"), bus_str(io, "fmsDepartureRunway"), bus_str(io, "fmsLandingRunway"));
        let req = arinc(io, "L:A32NX_OANS_BTV_REQ_STOPPING_DISTANCE");
        let perf = io.bus("CONFIG_A380X_OANS_PERFORMANCE_MODE").and_then(|v| v.as_str().map(|s| s == "1")).unwrap_or(false);
        let mut any_show = false;
        for s in &mut self.sides {
            let range = io.var(&format!("L:A32NX_EFIS_{}_ND_RANGE", s.letter));
            let mode = io.var(&format!("L:A32NX_EFIS_{}_ND_MODE", s.letter));
            let show = range == 0.0 && [2.0, 3.0, 4.0].contains(&mode);
            any_show |= show;
            if mode != s.nd_mode {
                s.nd_mode = mode;
                s.pan = None;
            }
            let side = s.letter;
            s.cp.set_show_oans(side, show);
            s.cp.set_reset_pulled(reset);
            s.cp.set_ppos(lat, long);
            s.cp.fms.origin = fms.0.clone();
            s.cp.fms.destination = fms.1.clone();
            s.cp.fms.alternate = fms.2.clone();
            s.cp.fms.departure_runway = fms.3.clone();
            s.cp.fms.landing_runway = fms.4.clone();
            s.cp.set_requested_stopping_distance(req);
            s.cp.performance_mode = perf;
        }

        self.since_panel_ms += delta_ms;
        if self.since_panel_ms >= PANEL_PERIOD_MS {
            self.since_panel_ms = 0.0;
            let src = self.cache.clone();
            for s in &mut self.sides {
                s.cp.on_real_time(&src, time_ms, phase, sim_time_s, false);
            }
        }

        for i in 0..2 {
            let now = self.sides[i].cp.loaded_airport.as_ref().map(|a| a.idarpt.clone());
            if now != self.shown[i] {
                crate::log(&format!("oans: side {} shows {}", self.sides[i].letter, now.as_deref().unwrap_or("no airport")));
                self.shown[i] = now;
                self.sides[i].markers.erase_all(Symbol::Cross);
                self.sides[i].markers.erase_all(Symbol::Flag);
                self.sides[i].pan = None;
            }
        }
        self.feed_nd_maps(io, delta_ms);
        let no_ppos = !lat.value.is_normal() || !long.value.is_normal();
        if no_ppos && any_show && !self.ppos_logged {
            self.ppos_logged = true;
            crate::log("oans: no present position from IR 1 yet (align the IRs); no airport is picked until there is one");
        } else if !no_ppos {
            self.ppos_logged = false;
        }
        if self.last_fms_landing.as_ref() != Some(&fms.4) {
            self.btv.set_fms_landing_runway(fms.4.as_deref());
            self.last_fms_landing = Some(fms.4.clone());
            if let Some(icao) = fms.4.as_ref().map(|r| r.chars().take(4).collect::<String>()) {
                if !self.cache.0.borrow().runways.contains_key(&icao) && self.pending_map_data.is_none() {
                    self.pending_map_data = Some(icao);
                }
            }
        }
        let ra = |io: &mut dyn HostIo, n: u8| arinc(io, &format!("L:A32NX_RA_{n}_RADIO_ALTITUDE"));
        let ir = |io: &mut dyn HostIo, n: u8, what: &str| arinc(io, &format!("L:A32NX_ADIRS_IR_{n}_{what}"));
        self.btv.inputs = BtvInputs {
            radio_altitude: [ra(io, 1), ra(io, 2), ra(io, 3)],
            vertical_speed: [ir(io, 1, "VERTICAL_SPEED"), ir(io, 2, "VERTICAL_SPEED"), ir(io, 3, "VERTICAL_SPEED")],
            latitude: [ir(io, 1, "LATITUDE"), ir(io, 2, "LATITUDE"), ir(io, 3, "LATITUDE")],
            longitude: [ir(io, 1, "LONGITUDE"), ir(io, 2, "LONGITUDE"), ir(io, 3, "LONGITUDE")],
            true_heading: [ir(io, 1, "TRUE_HEADING"), ir(io, 2, "TRUE_HEADING"), ir(io, 3, "TRUE_HEADING")],
            lgciu_discrete_word_2: [arinc(io, "L:A32NX_LGCIU_1_DISCRETE_WORD_2"), arinc(io, "L:A32NX_LGCIU_2_DISCRETE_WORD_2")],
            fws_flight_phase: phase,
            auto_brake_mode: io.var("L:A32NX_AUTOBRAKES_ARMED_MODE"),
            auto_brake_active: io.var("L:A32NX_AUTOBRAKES_ACTIVE") != 0.0,
        };

        let mut out: Vec<(String, Value)> = Vec::new();
        for i in 0..2 {
            for (topic, value) in std::mem::take(&mut self.sides[i].cp.out_bus) {
                match topic.as_str() {
                    "oans_display_airport" => {
                        if let Some(icao) = value.as_str().filter(|s| !s.is_empty()) {
                            let src = self.cache.clone();
                            self.sides[1 - i].cp.on_display_airport_event(&src, icao);
                            self.want_map_data(0);
                            self.want_map_data(1);
                        }
                    }
                    "oansManualStoppingDistance" => self.btv.set_manual_stopping_distance(value.as_f64()),
                    _ => self.apply_map_event(&topic, &value),
                }
                out.push((topic, value));
            }
            for (k, v) in std::mem::take(&mut self.sides[i].cp.vars) {
                io.set_var(&k, v);
            }
        }

        self.btv.on_update(delta_ms);
        for (k, v) in std::mem::take(&mut self.btv.out.vars) {
            io.set_var(&k, v);
        }
        out.extend(std::mem::take(&mut self.btv.out.log));
        let b = &self.btv.out.bus;
        let rops = (
            b.get("ropsDetectedAirport").and_then(Value::as_str).map(str::to_string),
            b.get("ropsDetectedRunway").and_then(Value::as_str).map(str::to_string),
            b.get("ropsDetectedRunwayLda").and_then(Value::as_f64),
            b.get("oansSelectedLandingRunway").and_then(Value::as_str).map(str::to_string),
        );
        for s in &mut self.sides {
            s.cp.set_runway_info_inputs(rops.0.clone(), rops.1.clone(), rops.2, rops.3.clone());
        }

        for s in &mut self.sides {
            s.markers.published.clear();
            let ids = s.markers.symbols_for_feature_ids();
            s.cp.set_symbols_for_feature_ids(ids.feature_ids_with_crosses, ids.feature_ids_with_flags);
        }
        for i in 0..2 {
            let model = self.model(i);
            let text = model.to_string();
            if text != self.sides[i].last_model {
                self.sides[i].last_model = text;
                out.push((format!("a380.oans.panel.{}", i + 1), model));
            }
        }
        if !out.is_empty() {
            io.publish(&out);
        }
    }
}

/// The plugin side: variables, the native displays' bus log and the AMDB folder/bridge.
pub mod live {
    use super::{Fetch, HostIo};
    use crate::native_displays::inputs::{InputSink, LiveInputs, ReadSet, VarSource};
    use serde_json::Value;
    use std::collections::HashMap;
    use systems::simulation::SimulatorReaderWriter;

    const AMDB_HOST: &str = "https://amdb.api.navigraph.com";

    #[derive(Default)]
    struct BusSink(HashMap<String, Value>);

    impl InputSink for BusSink {
        fn set_var(&mut self, _: &str, _: &str, _: f64) {}
        fn set_string(&mut self, _: &str, _: &str) {}
        fn set_bus(&mut self, topic: &str, json: &str) {
            if let Ok(v) = serde_json::from_str(json) {
                self.0.insert(topic.to_string(), v);
            }
        }
        fn set_bus_string(&mut self, topic: &str, text: &str) {
            self.0.insert(topic.to_string(), Value::String(text.to_string()));
        }
        fn set_generic(&mut self, _: &str, _: &str) {}
        fn set_fetched(&mut self, _: &str, _: &[u8]) {}
        fn set_env_number(&mut self, _: &str, _: f64) {}
        fn set_now(&mut self, _: f64) {}
        fn set_dpr(&mut self, _: f64) {}
    }

    struct NoVars;

    impl VarSource for NoVars {
        fn read_var(&mut self, _: &str, _: &str) -> Option<f64> {
            None
        }
        fn read_string(&mut self, _: &str) -> Option<String> {
            None
        }
    }

    pub struct Live {
        host: super::OansHost,
        feed: LiveInputs,
        bus: BusSink,
    }

    impl Default for Live {
        fn default() -> Self {
            let mut feed = LiveInputs::with_read_set(ReadSet::default());
            feed.skip_mfd_dom(true);
            Self { host: super::OansHost::new(), feed, bus: BusSink::default() }
        }
    }

    struct Io<'a> {
        vars: &'a mut crate::Vars,
        bus: &'a HashMap<String, Value>,
        key: u64,
    }

    impl HostIo for Io<'_> {
        fn var(&mut self, name: &str) -> f64 {
            let id = self.vars.add(name.trim_start_matches("L:").to_string(), crate::NAMED);
            self.vars.read(&id)
        }
        fn set_var(&mut self, name: &str, value: f64) {
            let id = self.vars.add(name.trim_start_matches("L:").to_string(), crate::NAMED);
            self.vars.write(&id, value);
        }
        fn bus(&mut self, topic: &str) -> Option<Value> {
            self.bus.get(topic).cloned()
        }
        fn publish(&mut self, events: &[(String, Value)]) {
            let events: Vec<Value> = events.iter().map(|(t, d)| serde_json::json!({"topic": t, "data": d})).collect();
            crate::native_displays::inputs::tap_generic_data("xp-ui.bus", &serde_json::json!({ "events": events }).to_string());
        }
        fn amdb(&mut self, path_and_query: &str) -> Fetch {
            self.key += 1;
            match crate::oans::plugin::amdb_fetch("GET", &format!("{AMDB_HOST}{path_and_query}"), &format!("oans-host {path_and_query}")) {
                Some(Some((status, body))) if (200..300).contains(&status) => Fetch::Ready(body),
                Some(None) => Fetch::Pending,
                _ => Fetch::Failed,
            }
        }
        fn to_displays(&mut self, path_and_query: &str, body: &str) {
            crate::native_displays::inputs::tap_fetched(&format!("coui://html_ui/__plugin/amdb{path_and_query}"), body.as_bytes());
        }
    }

    impl Live {
        /// Call once per tick after `native_displays::input::apply`, only while the NDs are native.
        pub fn tick(&mut self, vars: &mut crate::Vars, delta_s: f64, sim_time_s: f64, h_events: &[String]) {
            self.feed.update(&mut NoVars, &mut self.bus, sim_time_s * 1000.0, 1.0);
            let mut io = Io { vars, bus: &self.bus.0, key: 0 };
            self.host.tick(&mut io, delta_s * 1000.0, sim_time_s * 1000.0, sim_time_s, h_events);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeIo {
        vars: BTreeMap<String, f64>,
        bus: BTreeMap<String, Value>,
        published: Vec<(String, Value)>,
        pending: bool,
    }

    const SEARCH: &str = r#"[{"idarpt":"YSSY","iata":"SYD","name":"Sydney","coordinates":{"lat":-33.9461,"lon":151.1772}},{"idarpt":"OMDB","iata":"DXB","name":"Dubai Intl","coordinates":{"lat":25.2528,"lon":55.3644}}]"#;

    impl HostIo for FakeIo {
        fn var(&mut self, name: &str) -> f64 {
            self.vars.get(name).copied().unwrap_or(0.0)
        }
        fn set_var(&mut self, name: &str, value: f64) {
            self.vars.insert(name.to_string(), value);
        }
        fn bus(&mut self, topic: &str) -> Option<Value> {
            self.bus.get(topic).cloned()
        }
        fn publish(&mut self, events: &[(String, Value)]) {
            self.published.extend(events.iter().cloned());
        }
        fn amdb(&mut self, pq: &str) -> Fetch {
            if self.pending {
                return Fetch::Pending;
            }
            if pq.starts_with("/v1/search") {
                return Fetch::Ready(SEARCH.into());
            }
            if pq.contains("runwaythreshold") {
                return Fetch::Ready(
                    r#"{"runwaythreshold":{"features":[{"geometry":{"type":"Point","coordinates":[10.0,20.0]},"properties":{"id":7,"feattype":2,"idthr":"16R","lda":3962,"tora":3962}},{"geometry":{"type":"Point","coordinates":[30.0,40.0]},"properties":{"id":8,"feattype":2,"idthr":"34L","lda":3900,"tora":3962}}]}}"#
                        .into(),
                );
            }
            Fetch::Ready("{}".into())
        }
    }

    fn last(io: &FakeIo, topic: &str) -> Option<Value> {
        io.published.iter().rev().find(|(t, _)| t == topic).map(|(_, v)| v.clone())
    }

    fn zoom(io: &mut FakeIo) {
        io.vars.insert("L:A32NX_EFIS_L_ND_RANGE".into(), 0.0);
        io.vars.insert("L:A32NX_EFIS_L_ND_MODE".into(), 3.0);
    }

    fn click(host: &mut OansHost, io: &mut FakeIo, ev: &str) {
        host.tick(io, 50.0, 1000.0, 10.0, &[ev.to_string()]);
    }

    fn hit(model: &Value, ev: &str) -> bool {
        model["hits"].as_array().unwrap().iter().any(|h| h["ev"] == ev)
    }

    #[test]
    fn availability_waits_for_the_airport_list_then_publishes() {
        let mut host = OansHost::new();
        let mut io = FakeIo { pending: true, ..Default::default() };
        host.tick(&mut io, 50.0, 0.0, 0.0, &[]);
        assert_eq!(last(&io, "a380.oans.panel.1").unwrap()["status"]["database"], "N/A");
        io.pending = false;
        host.tick(&mut io, 50.0, 50.0, 0.05, &[]);
        assert_eq!(last(&io, "oans_not_avail"), Some(json!(false)));
        assert_eq!(io.vars.get("L:A32NX_OANS_AVAILABLE"), Some(&1.0));
        let m = last(&io, "a380.oans.panel.1").unwrap();
        assert_eq!(m["status"]["swapDisabled"], true);
        assert_eq!(m["status"]["database"], oans::control_panel::AIRPORT_DATABASE_IDENT);
    }

    #[test]
    fn toggle_and_tab_clicks_change_the_published_model() {
        let mut host = OansHost::new();
        let mut io = FakeIo::default();
        zoom(&mut io);
        host.tick(&mut io, 50.0, 0.0, 0.0, &[]);
        let m = last(&io, "a380.oans.panel.1").unwrap();
        assert_eq!(m["open"], false);
        assert!(hit(&m, "A380D_OANS_L_TOGGLE") && !hit(&m, "A380D_OANS_L_TAB_0"));
        click(&mut host, &mut io, "A380D_OANS_L_TOGGLE");
        let m = last(&io, "a380.oans.panel.1").unwrap();
        assert_eq!((m["open"].clone(), m["active"].clone()), (json!(true), json!(2)));
        assert!(hit(&m, "A380D_OANS_L_TAB_1"));
        click(&mut host, &mut io, "A380D_OANS_L_TAB_1");
        assert_eq!(last(&io, "a380.oans.panel.1").unwrap()["active"], 1);
        assert_eq!(last(&io, "a380.oans.panel.2").unwrap()["open"], false);
        assert!(!host.handle_event("A380D_OANS_X_TOGGLE", 0.0) && !host.handle_event("A32NX_KCCU_L_UP", 0.0));
    }

    #[test]
    fn airport_selection_display_and_map_data_round_trip() {
        let mut host = OansHost::new();
        let mut io = FakeIo::default();
        zoom(&mut io);
        io.bus.insert("fmsOrigin".into(), json!("YSSY"));
        host.tick(&mut io, 50.0, 0.0, 0.0, &[]);
        click(&mut host, &mut io, "A380D_OANS_L_ORIGIN");
        assert_eq!(last(&io, "a380.oans.panel.1").unwrap()["arptSel"]["selectedAirport"], "YSSY");
        click(&mut host, &mut io, "A380D_OANS_L_DISPLAY_AIRPORT");
        assert_eq!(last(&io, "oans_display_airport"), Some(json!("YSSY")));
        assert_eq!(host.side(1).selected_airport.as_ref().map(|a| a.idarpt.as_str()), Some("YSSY"));
        let m = last(&io, "a380.oans.panel.1").unwrap();
        assert_eq!(m["mapData"]["entities"], json!(["16R", "34L"]));
        assert_eq!(m["mapData"]["buttonsDisabled"], true);
        click(&mut host, &mut io, "A380D_OANS_L_ENTITY_1");
        let m = last(&io, "a380.oans.panel.1").unwrap();
        assert_eq!((m["mapData"]["entity"].clone(), m["mapData"]["lda"].clone()), (json!("34L"), json!("3900")));
        click(&mut host, &mut io, "A380D_OANS_L_CROSS");
        assert_eq!(last(&io, "oans_add_cross_at_feature"), Some(json!({"id": 8, "feattype": 2})));
        click(&mut host, &mut io, "A380D_OANS_L_CENTER");
        assert_eq!(last(&io, "oans_center_map_on"), Some(json!([30.0, 40.0])));
        click(&mut host, &mut io, "A380D_OANS_L_SEARCH_MODE_1");
        if !last(&io, "a380.oans.panel.1").unwrap()["open"].as_bool().unwrap_or(false) {
            click(&mut host, &mut io, "A380D_OANS_L_TOGGLE");
        }
        click(&mut host, &mut io, "A380D_OANS_L_MENU_AIRPORT");
        assert_eq!(last(&io, "a380.oans.panel.1").unwrap()["menu"]["items"], json!(["DXB", "SYD"]));
        click(&mut host, &mut io, "A380D_OANS_L_AIRPORT_0");
        assert_eq!(last(&io, "a380.oans.panel.1").unwrap()["arptSel"]["selectedAirport"], "OMDB");
    }

    #[test]
    fn cross_flag_and_centre_reach_the_nd_model() {
        let mut host = OansHost::new();
        let mut io = FakeIo::default();
        zoom(&mut io);
        io.bus.insert("fmsOrigin".into(), json!("YSSY"));
        host.tick(&mut io, 50.0, 0.0, 0.0, &[]);
        click(&mut host, &mut io, "A380D_OANS_L_ORIGIN");
        click(&mut host, &mut io, "A380D_OANS_L_DISPLAY_AIRPORT");
        for k in 0..3 {
            host.tick(&mut io, 2000.0, 2000.0 * (k + 2) as f64, 2.0 * (k + 2) as f64, &[]);
        }
        click(&mut host, &mut io, "A380D_OANS_L_ENTITY_1");
        click(&mut host, &mut io, "A380D_OANS_L_CROSS");
        let m = last(&io, "a380.oans.panel.1").unwrap();
        assert_eq!(m["markers"], json!([{"k": "c", "p": [30.0, 40.0]}]));
        assert_eq!(m["mapData"]["cross"], "DEL CROSS");
        click(&mut host, &mut io, "A380D_OANS_L_FLAG");
        assert_eq!(last(&io, "a380.oans.panel.1").unwrap()["markers"].as_array().unwrap().len(), 2);
        click(&mut host, &mut io, "A380D_OANS_L_CROSS");
        let m = last(&io, "a380.oans.panel.1").unwrap();
        assert_eq!((m["markers"].clone(), m["mapData"]["cross"].clone()), (json!([{"k": "f", "p": [30.0, 40.0]}]), json!("ADD CROSS")));
        click(&mut host, &mut io, "A380D_OANS_L_CENTER");
        assert_eq!(last(&io, "a380.oans.panel.1").unwrap()["pan"]["p"], json!([30.0, 40.0]));
        io.vars.insert("L:A32NX_EFIS_L_ND_MODE".into(), 4.0);
        host.tick(&mut io, 50.0, 9000.0, 9.0, &[]);
        assert_eq!(last(&io, "a380.oans.panel.1").unwrap()["pan"], Value::Null);
    }

    #[test]
    fn btv_runway_from_navdb_reaches_the_bus() {
        let mut host = OansHost::new();
        let mut io = FakeIo::default();
        io.bus.insert("fmsLandingRunway".into(), json!("YSSY16R"));
        host.tick(&mut io, 50.0, 0.0, 0.0, &[]);
        host.tick(&mut io, 250.0, 250.0, 0.25, &[]);
        assert!(io.published.iter().any(|(t, v)| t == "oansSelectedLandingRunway" && v == "YSSY16R"), "{:?}", io.published);
        assert!(io.vars.contains_key("L:A32NX_OANS_RWY_LENGTH"));
    }
}

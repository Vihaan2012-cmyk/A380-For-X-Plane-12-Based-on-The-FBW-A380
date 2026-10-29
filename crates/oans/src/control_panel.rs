use std::collections::BTreeMap;
use std::rc::Rc;

use serde_json::{json, Value};

use crate::arinc::Arinc;
use crate::btv::BTV_MIN_TOUCHDOWN_ZONE_DISTANCE;
use crate::geo::{distance_to, js_round, Coordinates, Position};

pub const LETTERS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const AIRPORT_DATABASE_IDENT: &str = "FBW9027250BB04";
const MONTHS: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];

#[derive(Clone, Debug, PartialEq)]
pub struct AmdbAirport {
    pub idarpt: String,
    pub iata: Option<String>,
    pub name: Option<String>,
    pub coordinates: Coordinates,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AirportSearchMode {
    Icao = 0,
    Iata = 1,
    City = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapDataSearchMode {
    Runway = 0,
    Taxiway = 1,
    Stand = 2,
    Other = 3,
}

impl MapDataSearchMode {
    pub fn feature_type_string(self) -> &'static str {
        match self {
            MapDataSearchMode::Runway => "runwaythreshold",
            MapDataSearchMode::Taxiway => "taxiwayguidanceline",
            MapDataSearchMode::Stand => "parkingstandlocation",
            MapDataSearchMode::Other => "deicingarea",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapDataFeature {
    pub id: Option<i64>,
    pub feattype: Option<u8>,
    pub idthr: Option<String>,
    pub idlin: Option<String>,
    pub idstd: Option<String>,
    pub ident: Option<String>,
    pub lda: Option<f64>,
    pub tora: Option<f64>,
    pub point: Option<Position>,
    pub midpoint: Option<Position>,
}

impl MapDataFeature {
    fn prop(&self, mode: MapDataSearchMode) -> Option<&str> {
        match mode {
            MapDataSearchMode::Runway => self.idthr.as_deref(),
            MapDataSearchMode::Taxiway => self.idlin.as_deref(),
            MapDataSearchMode::Stand => self.idstd.as_deref(),
            MapDataSearchMode::Other => self.ident.as_deref(),
        }
    }
}

pub trait AmdbSource {
    fn search_airports(&self) -> Result<Vec<AmdbAirport>, String>;
    fn map_data(&self, icao: &str, feature_type: &str) -> Option<Vec<MapDataFeature>>;
}

pub fn airport_prop(a: &AmdbAirport, mode: AirportSearchMode) -> Option<&str> {
    match mode {
        AirportSearchMode::Icao => Some(a.idarpt.as_str()),
        AirportSearchMode::Iata => a.iata.as_deref(),
        AirportSearchMode::City => a.name.as_deref(),
    }
}

#[derive(Clone, Debug)]
pub struct ConfirmNode {
    t: f64,
    rising_edge: bool,
    timer: f64,
    previous_input: Option<bool>,
    previous_output: Option<bool>,
}

impl ConfirmNode {
    pub fn new(t: f64, rising_edge: bool) -> Self {
        Self { t, rising_edge, timer: 0.0, previous_input: None, previous_output: None }
    }

    pub fn write(&mut self, value: bool, delta_ms: f64, skip_startup: bool) -> bool {
        if self.previous_input.is_none() && skip_startup {
            self.previous_input = Some(value);
            self.previous_output = Some(value);
        }
        let prev = self.previous_input.unwrap_or(false);
        let held = if self.rising_edge { value } else { !value };
        if !held {
            self.timer = 0.0;
        } else if self.timer > 0.0 {
            self.timer = (self.timer - delta_ms / 1000.0).max(0.0);
            self.previous_input = Some(value);
            self.previous_output = Some(!value);
            return !value;
        } else if (self.rising_edge && !prev) || (!self.rising_edge && prev) {
            self.timer = self.t;
            self.previous_input = Some(value);
            self.previous_output = Some(!value);
            return !value;
        }
        self.previous_input = Some(value);
        self.previous_output = Some(value);
        value
    }

    pub fn read(&self) -> bool {
        self.previous_output.unwrap_or(false)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FmsAirport {
    Origin,
    Destination,
    Alternate,
}

#[derive(Clone, Debug, Default)]
pub struct FmsData {
    pub origin: Option<String>,
    pub destination: Option<String>,
    pub alternate: Option<String>,
    pub departure_runway: Option<String>,
    pub landing_runway: Option<String>,
}

pub struct ControlPanel {
    pub side: String,
    pub out_bus: Vec<(String, Value)>,
    pub vars: BTreeMap<String, f64>,

    pub airports: Vec<Rc<AmdbAirport>>,
    pub sorted_airports: Vec<Rc<AmdbAirport>>,
    pub airport_search_mode: AirportSearchMode,
    pub airport_search_data: Vec<String>,
    pub selected_letter_index: Option<i64>,
    pub selected_airport_index: Option<i64>,
    pub selected_airport: Option<Rc<AmdbAirport>>,
    pub loaded_airport: Option<Rc<AmdbAirport>>,
    pub is_airport_selection_pending: bool,

    navigraph_available: bool,
    reset_pulled: bool,
    oans_available: bool,
    oans_failed: bool,
    pub performance_mode: bool,
    show_oans: bool,
    last_update_time: Option<f64>,
    perf_confirm: ConfirmNode,
    manual_airport_selection: bool,
    manual_airport_selection_time: f64,

    ppos_lat: Arinc,
    ppos_long: Arinc,
    pub fms: FmsData,

    rops_airport: Option<String>,
    rops_runway: Option<String>,
    rops_lda: Option<f64>,
    oans_selected_landing_runway: Option<String>,

    pub active_tab_index: usize,
    pub runway_lda: f64,
    pub runway_tora: f64,
    map_data_features: Option<Vec<MapDataFeature>>,
    pub selected_entity_type: MapDataSearchMode,
    pub available_entity_list: Vec<String>,
    selected_entity_index: Option<usize>,
    pub selected_entity_string: Option<String>,
    pub selected_entity_position: Position,
    pub selected_feature_id: Option<i64>,
    pub selected_feature_type: Option<u8>,
    pub symbols_crosses: Vec<i64>,
    pub symbols_flags: Vec<i64>,
    pub requested_stopping_distance: Arinc,
    pub active_database: String,
    pub threshold_shift: Option<f64>,
    pub end_shift: Option<f64>,
    pub ldg_shift_panel_visible: bool,
}

fn truthy(s: &Option<String>) -> bool {
    s.as_deref().is_some_and(|v| !v.is_empty())
}

impl ControlPanel {
    pub fn new(side: &str) -> Self {
        let mut s = Self {
            side: side.to_string(),
            out_bus: Vec::new(),
            vars: BTreeMap::new(),
            airports: Vec::new(),
            sorted_airports: Vec::new(),
            airport_search_mode: AirportSearchMode::Icao,
            airport_search_data: Vec::new(),
            selected_letter_index: None,
            selected_airport_index: None,
            selected_airport: None,
            loaded_airport: None,
            is_airport_selection_pending: false,
            navigraph_available: false,
            reset_pulled: false,
            oans_available: false,
            oans_failed: false,
            performance_mode: false,
            show_oans: false,
            last_update_time: None,
            perf_confirm: ConfirmNode::new(60.0, true),
            manual_airport_selection: false,
            manual_airport_selection_time: 0.0,
            ppos_lat: Arinc::default(),
            ppos_long: Arinc::default(),
            fms: FmsData::default(),
            rops_airport: None,
            rops_runway: None,
            rops_lda: None,
            oans_selected_landing_runway: None,
            active_tab_index: 2,
            runway_lda: f64::NAN,
            runway_tora: f64::NAN,
            map_data_features: None,
            selected_entity_type: MapDataSearchMode::Runway,
            available_entity_list: vec![String::new()],
            selected_entity_index: Some(0),
            selected_entity_string: None,
            selected_entity_position: [0.0, 0.0],
            selected_feature_id: None,
            selected_feature_type: None,
            symbols_crosses: Vec::new(),
            symbols_flags: Vec::new(),
            requested_stopping_distance: Arinc::default(),
            active_database: "30DEC-27JAN".to_string(),
            threshold_shift: None,
            end_shift: None,
            ldg_shift_panel_visible: false,
        };
        s.update_airport_search_data();
        s.on_selected_entity_index();
        s.write_availability(true);
        s
    }

    fn publish(&mut self, topic: &str, value: Value) {
        self.out_bus.push((topic.to_string(), value));
    }

    fn write_availability(&mut self, force: bool) {
        let avail = self.navigraph_available && !self.reset_pulled;
        let failed = self.reset_pulled;
        if force || avail != self.oans_available {
            self.oans_available = avail;
            self.vars.insert("L:A32NX_OANS_AVAILABLE".into(), if avail { 1.0 } else { 0.0 });
            self.publish("oans_not_avail", json!(!avail));
        }
        if force || failed != self.oans_failed {
            self.oans_failed = failed;
            self.vars.insert("L:A32NX_OANS_FAILED".into(), if failed { 1.0 } else { 0.0 });
        }
    }

    pub fn oans_available(&self) -> bool {
        self.oans_available
    }

    pub fn airport_database(&self) -> &'static str {
        if self.navigraph_available {
            AIRPORT_DATABASE_IDENT
        } else {
            "N/A"
        }
    }

    pub fn set_database_ident(&mut self, from_day: u32, from_month0: usize, to_day: u32, to_month0: usize) {
        self.active_database = format!("{}{}-{}{}", from_day, MONTHS[from_month0 % 12], to_day, MONTHS[to_month0 % 12]);
    }

    pub fn load_oans_db(&mut self, source: &dyn AmdbSource) {
        match source.search_airports() {
            Ok(list) => {
                self.airports = list.into_iter().map(Rc::new).collect();
                self.sort_airports(self.airport_search_mode);
                self.navigraph_available = true;
            }
            Err(_) => {
                self.airports = Vec::new();
                self.sort_airports(self.airport_search_mode);
                self.navigraph_available = false;
            }
        }
        self.write_availability(false);
    }

    pub fn set_reset_pulled(&mut self, pulled: bool) {
        if pulled == self.reset_pulled {
            return;
        }
        self.reset_pulled = pulled;
        self.write_availability(false);
        if pulled {
            self.unload_current_airport();
        }
    }

    pub fn set_ppos(&mut self, lat: Arinc, long: Arinc) {
        self.ppos_lat = lat;
        self.ppos_long = long;
    }

    fn present_pos(&self) -> Coordinates {
        Coordinates { lat: self.ppos_lat.value, long: self.ppos_long.value }
    }

    fn present_pos_not_available(&self) -> bool {
        !self.ppos_lat.is_normal() || !self.ppos_long.is_normal()
    }

    pub fn set_show_oans(&mut self, side: &str, show: bool) {
        if side == self.side {
            self.show_oans = show;
        }
    }

    pub fn set_symbols_for_feature_ids(&mut self, crosses: Vec<i64>, flags: Vec<i64>) {
        self.symbols_crosses = crosses;
        self.symbols_flags = flags;
    }

    pub fn set_requested_stopping_distance(&mut self, word: Arinc) {
        self.requested_stopping_distance = word;
    }

    pub fn req_stopping_distance(&self) -> Option<f64> {
        let w = self.requested_stopping_distance;
        w.is_normal().then(|| js_round(w.value + BTV_MIN_TOUCHDOWN_ZONE_DISTANCE))
    }

    pub fn fms_landing_runway_not_selected_in_fallback(&self) -> bool {
        !self.oans_available && self.fms.landing_runway.is_none()
    }

    pub fn set_runway_info_inputs(&mut self, rops_airport: Option<String>, rops_runway: Option<String>, rops_lda: Option<f64>, oans_runway: Option<String>) {
        let changed = rops_airport != self.rops_airport || rops_runway != self.rops_runway || rops_lda != self.rops_lda || oans_runway != self.oans_selected_landing_runway;
        self.rops_airport = rops_airport;
        self.rops_runway = rops_runway;
        self.rops_lda = rops_lda;
        self.oans_selected_landing_runway = oans_runway;
        if !changed {
            return;
        }
        if truthy(&self.oans_selected_landing_runway) || (truthy(&self.rops_airport) && truthy(&self.rops_runway)) {
            self.runway_lda = self.rops_lda.unwrap_or(f64::NAN);
            self.runway_tora = self.rops_lda.unwrap_or(f64::NAN);
        } else {
            self.clear_runway_info();
        }
    }

    fn clear_runway_info(&mut self) {
        self.set_selected_entity_index(None);
        self.selected_entity_string = None;
        self.runway_lda = f64::NAN;
        self.runway_tora = f64::NAN;
    }

    pub fn runway_length_text(value_m: f64, metric: bool) -> String {
        if value_m.is_nan() {
            return String::new();
        }
        let v = if metric { value_m } else { value_m / 0.3048 };
        format!("{}", js_round(v))
    }

    fn update_airport_search_data(&mut self) {
        let mode = self.airport_search_mode;
        self.airport_search_data = self.sorted_airports.iter().map(|a| airport_prop(a, mode).unwrap_or("").to_uppercase()).collect();
    }

    fn sort_airports(&mut self, mode: AirportSearchMode) {
        let mut array = self.airports.clone();
        array.sort_by(|a, b| match (airport_prop(a, mode), airport_prop(b, mode)) {
            (Some(x), Some(y)) => x.encode_utf16().cmp(y.encode_utf16()),
            _ => std::cmp::Ordering::Equal,
        });
        self.sorted_airports = array.into_iter().filter(|a| airport_prop(a, mode).is_some()).collect();
        self.update_airport_search_data();
    }

    pub fn handle_select_airport(&mut self, source: &dyn AmdbSource, icao: &str, index_in_search_data: Option<i64>) -> Result<(), String> {
        let airport = self.airports.iter().find(|a| a.idarpt == icao).cloned();
        let Some(airport) = airport else { return Err(format!("airport {icao} not found")) };
        let Some(prop) = airport_prop(&airport, self.airport_search_mode) else { return Err(String::new()) };
        let first: Option<char> = prop.chars().next();
        self.selected_letter_index = Some(first.and_then(|c| LETTERS.find(c)).map_or(-1, |i| i as i64));
        let idx = index_in_search_data.unwrap_or_else(|| self.sorted_airports.iter().position(|a| a.idarpt == icao).map_or(-1, |i| i as i64));
        self.selected_airport_index = Some(idx);
        self.selected_airport = Some(airport);
        self.handle_select_map_data_search_mode(source, MapDataSearchMode::Runway);
        self.is_airport_selection_pending = true;
        Ok(())
    }

    pub fn select_airport_from_dropdown(&mut self, source: &dyn AmdbSource, new_index: Option<usize>) -> Result<(), String> {
        let i = new_index.unwrap_or(0);
        let icao = self.sorted_airports.get(i).map(|a| a.idarpt.clone()).ok_or("index")?;
        self.handle_select_airport(source, &icao, new_index.map(|v| v as i64))
    }

    pub fn select_fms_airport(&mut self, source: &dyn AmdbSource, which: FmsAirport) -> Result<(), String> {
        let a = match which {
            FmsAirport::Origin => self.fms.origin.clone(),
            FmsAirport::Destination => self.fms.destination.clone(),
            FmsAirport::Alternate => self.fms.alternate.clone(),
        };
        match a.filter(|v| !v.is_empty()) {
            Some(icao) => self.handle_select_airport(source, &icao, None),
            None => Ok(()),
        }
    }

    pub fn fms_button_label(&self, which: FmsAirport) -> String {
        let (v, d) = match which {
            FmsAirport::Origin => (&self.fms.origin, "ORIGIN"),
            FmsAirport::Destination => (&self.fms.destination, "DEST"),
            FmsAirport::Alternate => (&self.fms.alternate, "ALTN"),
        };
        v.clone().unwrap_or_else(|| d.to_string())
    }

    pub fn handle_select_airport_search_mode(&mut self, mode: AirportSearchMode) {
        let changed = mode != self.airport_search_mode;
        self.airport_search_mode = mode;
        if changed {
            self.sort_airports(mode);
        }
        if let Some(sel) = self.selected_airport.clone() {
            if let Some(prop) = airport_prop(&sel, mode) {
                let first = prop.chars().next();
                let idx = self.sorted_airports.iter().position(|a| a.idarpt == sel.idarpt).map_or(-1, |i| i as i64);
                self.selected_letter_index = Some(first.and_then(|c| LETTERS.find(c)).map_or(-1, |i| i as i64));
                self.selected_airport_index = Some(idx);
            }
        }
    }

    pub fn handle_select_map_data_search_mode(&mut self, source: &dyn AmdbSource, mode: MapDataSearchMode) {
        self.set_selected_entity_index(None);
        let Some(sel) = self.selected_airport.clone() else { return };
        self.map_data_features = source.map_data(&sel.idarpt, mode.feature_type_string());
        if let Some(features) = &self.map_data_features {
            let mut list: Vec<String> = Vec::new();
            for f in features {
                let v: String = f.prop(mode).unwrap_or("").trim().encode_utf16().take(6).map(|u| char::from_u32(u as u32).unwrap_or('?')).collect();
                if !v.is_empty() && !list.contains(&v) {
                    list.push(v);
                }
            }
            list.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            self.available_entity_list = list;
        }
    }

    pub fn set_selected_entity_type(&mut self, source: &dyn AmdbSource, mode: MapDataSearchMode) {
        if mode != self.selected_entity_type {
            self.selected_entity_type = mode;
            self.handle_select_map_data_search_mode(source, mode);
        }
    }

    pub fn selected_entity_index(&self) -> Option<usize> {
        self.selected_entity_index
    }

    pub fn set_selected_entity_index(&mut self, index: Option<usize>) {
        if index != self.selected_entity_index {
            self.selected_entity_index = index;
            self.on_selected_entity_index();
        }
    }

    fn on_selected_entity_index(&mut self) {
        let mode = self.selected_entity_type;
        match (&self.map_data_features, self.selected_entity_index) {
            (Some(features), Some(val)) => {
                let wanted = self.available_entity_list.get(val).cloned();
                let idx = features.iter().position(|f| f.prop(mode).map(str::to_string) == wanted && wanted.is_some());
                let feature = idx.map(|i| features[i].clone());
                self.selected_entity_string = Some(feature.as_ref().and_then(|f| f.prop(mode).map(str::to_string)).unwrap_or_default());
                if (idx.is_some() && mode == MapDataSearchMode::Runway) || mode == MapDataSearchMode::Stand {
                    let Some(f) = feature.as_ref() else { return };
                    let Some(p) = f.point else { return };
                    self.selected_entity_position = p;
                    self.selected_feature_id = f.id;
                    self.selected_feature_type = f.feattype;
                } else if let Some(f) = feature.as_ref().filter(|_| matches!(mode, MapDataSearchMode::Taxiway | MapDataSearchMode::Other)) {
                    self.selected_entity_position = f.midpoint.unwrap_or([0.0, 0.0]);
                    self.selected_feature_id = f.id;
                    self.selected_feature_type = f.feattype;
                }
                if let Some(f) = feature.as_ref().filter(|_| mode == MapDataSearchMode::Runway) {
                    self.runway_lda = f.lda.unwrap_or(f64::NAN);
                    self.runway_tora = f.tora.unwrap_or(f64::NAN);
                }
            }
            _ => {
                if self.oans_available {
                    self.selected_entity_string = Some(String::new());
                    self.runway_lda = f64::NAN;
                    self.runway_tora = f64::NAN;
                }
            }
        }
    }

    pub fn entity_is_not_selected(&self) -> bool {
        self.selected_entity_index.is_none()
    }

    pub fn cross_exists_for_entity(&self) -> bool {
        self.selected_feature_id.is_some_and(|id| self.symbols_crosses.contains(&id))
    }

    pub fn flag_exists_for_entity(&self) -> bool {
        self.selected_feature_id.is_some_and(|id| self.symbols_flags.contains(&id))
    }

    pub fn handle_cross_button(&mut self) {
        if let (Some(id), Some(ft)) = (self.selected_feature_id, self.selected_feature_type) {
            let topic = if self.cross_exists_for_entity() { "oans_remove_cross_at_feature" } else { "oans_add_cross_at_feature" };
            self.publish(topic, json!({"id": id, "feattype": ft}));
        }
    }

    pub fn handle_flag_button(&mut self) {
        if let (Some(id), Some(ft)) = (self.selected_feature_id, self.selected_feature_type) {
            let topic = if self.flag_exists_for_entity() { "oans_remove_flag_at_feature" } else { "oans_add_flag_at_feature" };
            self.publish(topic, json!({"id": id, "feattype": ft}));
        }
    }

    pub fn center_map_on_selected(&mut self) {
        let p = self.selected_entity_position;
        self.publish("oans_center_map_on", json!(p));
    }

    pub fn enter_manual_stopping_distance(&mut self, metres: Option<f64>) {
        self.publish("oansManualStoppingDistance", json!(metres));
    }

    fn display_airport(&mut self, source: &dyn AmdbSource, icao: &str) {
        self.publish("oans_display_airport", json!(icao));
        let _ = self.handle_select_airport(source, icao, None);
    }

    pub fn on_display_airport_event(&mut self, source: &dyn AmdbSource, icao: &str) {
        let _ = self.handle_select_airport(source, icao, None);
    }

    pub fn handle_display_airport(&mut self, source: &dyn AmdbSource, sim_time_s: f64) -> Result<(), String> {
        let Some(sel) = self.selected_airport.clone().filter(|a| !a.idarpt.is_empty()) else {
            return Err("[OANS] Empty airport selected for display.".into());
        };
        self.manual_airport_selection = true;
        self.manual_airport_selection_time = sim_time_s;
        self.display_airport(source, &sel.idarpt);
        self.loaded_airport = Some(sel);
        self.is_airport_selection_pending = false;
        Ok(())
    }

    fn unload_current_airport(&mut self) {
        if self.loaded_airport.is_some() {
            self.publish("oans_display_airport", json!(""));
            self.loaded_airport = None;
            self.is_airport_selection_pending = false;
        }
    }

    fn loaded_is_selected(&self) -> bool {
        match (&self.loaded_airport, &self.selected_airport) {
            (None, None) => true,
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }

    pub fn on_real_time(&mut self, source: &dyn AmdbSource, time_ms: f64, fwc_flight_phase: f64, sim_time_s: f64, skip_startup: bool) {
        let dt = self.last_update_time.map_or(0.0, |l| time_ms - l);
        self.perf_confirm.write(self.performance_mode && !self.show_oans, dt, skip_startup);
        let hide = self.perf_confirm.read();
        let side = self.side.clone();
        self.publish("oans_performance_mode_hide", json!({"side": side, "hide": hide}));
        self.auto_load_airport(source, fwc_flight_phase, sim_time_s);
        self.last_update_time = Some(time_ms);
    }

    pub fn auto_load_airport(&mut self, source: &dyn AmdbSource, fwc_flight_phase: f64, sim_time_s: f64) {
        if self.present_pos_not_available()
            || self.perf_confirm.read()
            || (self.manual_airport_selection && sim_time_s - self.manual_airport_selection_time < 600.0)
            || !self.loaded_is_selected()
            || self.airports.is_empty()
            || self.reset_pulled
        {
            return;
        }
        let ppos = self.present_pos();
        let dist = |a: &AmdbAirport| distance_to(ppos, a.coordinates);
        if ![6.0, 7.0, 8.0, 9.0].contains(&fwc_flight_phase) {
            let mut nearest: Vec<Rc<AmdbAirport>> = self.airports.iter().filter(|a| dist(a) < 20.0).cloned().collect();
            nearest.sort_by(|a, b| (dist(a) - dist(b)).partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal));
            if let Some(ap) = nearest.first().cloned() {
                if Some(ap.idarpt.as_str()) != self.loaded_airport.as_ref().map(|l| l.idarpt.as_str()) {
                    self.display_airport(source, &ap.idarpt);
                    self.loaded_airport = Some(ap);
                    self.is_airport_selection_pending = false;
                }
            }
        } else {
            let dest = self.fms.destination.clone();
            let dest_arpt = self.airports.iter().find(|a| Some(a.idarpt.as_str()) == dest.as_deref()).cloned();
            if let Some(d) = dest_arpt {
                if Some(d.idarpt.as_str()) != self.loaded_airport.as_ref().map(|l| l.idarpt.as_str()) && dist(&d) < 50.0 {
                    self.display_airport(source, &d.idarpt);
                    self.loaded_airport = Some(d);
                    self.is_airport_selection_pending = false;
                }
            }
        }
    }

    pub fn show_ldg_shift_panel(&mut self, show: bool) {
        self.ldg_shift_panel_visible = show;
    }

    pub fn selected_airport_name(&self) -> String {
        self.selected_airport.as_ref().and_then(|a| a.name.as_deref()).map(|n| n.chars().take(18).collect::<String>().to_uppercase()).unwrap_or_default()
    }

    pub fn selected_airport_idents(&self) -> String {
        self.selected_airport.as_ref().map(|a| format!("{}       {}", a.idarpt, a.iata.as_deref().unwrap_or("null"))).unwrap_or_default()
    }

    pub fn state_json(&self) -> Value {
        json!({
            "loadedAirport": self.loaded_airport.as_ref().map(|a| a.idarpt.clone()),
            "selectedAirport": self.selected_airport.as_ref().map(|a| a.idarpt.clone()),
            "selectedAirportIndex": self.selected_airport_index,
            "selectedLetterIndex": self.selected_letter_index,
            "pending": self.is_airport_selection_pending,
            "searchData": self.airport_search_data,
            "entities": self.available_entity_list,
            "entityIndex": self.selected_entity_index,
            "entityString": self.selected_entity_string,
            "oansAvailable": self.oans_available,
        })
    }
}

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::arinc::{Arinc, Ssm};
use crate::geo::{
    distance_to, place_bearing_distance, point_distance, round_to, Coordinates, Position, FEET_TO_NAUTICAL_MILES,
    METRES_TO_NAUTICAL_MILES,
};
use crate::map_utils::{fractional_point_along_line, point_angle, point_to_line_distance};
use crate::projection::{airport_to_global_coordinates, global_to_airport_coordinates};
use crate::advisory::{RunwayArea, RwyAhead, RwyAheadInputs, OANS_WORD_1};

pub const BTV_MIN_TOUCHDOWN_ZONE_DISTANCE: f64 = 400.0;
const CLAMP_DRY_STOPBAR_DISTANCE: f64 = 100.0;
const CLAMP_WET_STOPBAR_DISTANCE: f64 = 200.0;
const UPDATE_PERIOD_MS: f64 = 200.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Runway {
    pub ident: String,
    pub threshold_location: Coordinates,
    pub bearing: f64,
    pub length: f64,
}

pub trait NavDb {
    fn airport_location(&self, icao: &str) -> Option<Coordinates>;
    fn runways(&self, icao: &str) -> Vec<Runway>;
    fn nearby_airports(&self, ppos: Coordinates, radius_nm: f64, max_results: usize) -> Vec<String>;
}

#[derive(Clone, Debug)]
pub struct BtvInputs {
    pub radio_altitude: [Arinc; 3],
    pub vertical_speed: [Arinc; 3],
    pub latitude: [Arinc; 3],
    pub longitude: [Arinc; 3],
    pub true_heading: [Arinc; 3],
    pub lgciu_discrete_word_2: [Arinc; 2],
    pub fws_flight_phase: f64,
    pub auto_brake_mode: f64,
    pub auto_brake_active: bool,
}

impl Default for BtvInputs {
    fn default() -> Self {
        Self {
            radio_altitude: [Arinc::default(); 3],
            vertical_speed: [Arinc::default(); 3],
            latitude: [Arinc::default(); 3],
            longitude: [Arinc::default(); 3],
            true_heading: [Arinc::default(); 3],
            lgciu_discrete_word_2: [Arinc::default(); 2],
            fws_flight_phase: 0.0,
            auto_brake_mode: 0.0,
            auto_brake_active: false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Outputs {
    pub vars: BTreeMap<String, f64>,
    pub bus: BTreeMap<String, Value>,
    pub log: Vec<(String, Value)>,
}

#[derive(Clone, Debug)]
pub struct StopLine {
    pub kind: &'static str,
    pub points: [Position; 3],
    pub overrun: bool,
    pub colour: &'static str,
}

#[derive(Default)]
struct Selection {
    runway: Option<String>,
    lda: Option<f64>,
    bearing_true: Option<f64>,
    threshold: Option<Position>,
    opposite_threshold: Option<Position>,
    exit: Option<String>,
    exit_distance: Option<f64>,
    exit_position: Option<Position>,
    path: Vec<Position>,
}

pub struct FeatureLine {
    pub coordinates: Vec<Position>,
}

pub struct Oans<'a> {
    nav: &'a dyn NavDb,
    pub inputs: BtvInputs,
    pub out: Outputs,
    last_remove_btv_data: Option<Value>,
    in_set_runway: bool,
    pending_runways: Vec<String>,

    arp: Option<Coordinates>,
    landing_runway: Option<Runway>,
    threshold: Option<Position>,
    opposite_threshold: Option<Position>,
    exit_position: Option<Position>,
    local_ppos: Position,
    ppos: Coordinates,

    oans_runway: Option<String>,
    fms_landing_runway: Option<String>,
    mapped_runway: Option<String>,
    oans_exit: Option<String>,
    oans_exit_coords: Coordinates,

    remaining_dist_to_exit: Arinc,
    remaining_dist_to_rwy_end: Arinc,
    requested_stopping_dist: Arinc,
    runway_length: Arinc,
    runway_bearing: Arinc,
    appr_different_runway: bool,
    btv_active_on_ground_last: bool,
    accumulated_ms: f64,

    sel: Selection,
    rwy_ahead: RwyAhead,
    pub dry_stopping_distance: f64,
    pub wet_stopping_distance: f64,
    pub live_stopping_distance: f64,
    pub ground_speed: Arinc,
    pub aircraft_on_ground: bool,
    pub aircraft_ppos_local: Position,
    pub zoom_inverse_scale: f64,
}

impl<'a> Oans<'a> {
    pub fn new(nav: &'a dyn NavDb) -> Self {
        let mut s = Self {
            nav,
            inputs: BtvInputs::default(),
            out: Outputs::default(),
            last_remove_btv_data: None,
            in_set_runway: false,
            pending_runways: Vec::new(),
            arp: None,
            landing_runway: None,
            threshold: None,
            opposite_threshold: None,
            exit_position: None,
            local_ppos: [0.0, 0.0],
            ppos: Coordinates::default(),
            oans_runway: None,
            fms_landing_runway: None,
            mapped_runway: None,
            oans_exit: None,
            oans_exit_coords: Coordinates::default(),
            remaining_dist_to_exit: Arinc::default(),
            remaining_dist_to_rwy_end: Arinc::default(),
            requested_stopping_dist: Arinc::default(),
            runway_length: Arinc::default(),
            runway_bearing: Arinc::default(),
            appr_different_runway: false,
            btv_active_on_ground_last: false,
            accumulated_ms: 0.0,
            sel: Selection::default(),
            rwy_ahead: RwyAhead::default(),
            dry_stopping_distance: 0.0,
            wet_stopping_distance: 0.0,
            live_stopping_distance: 0.0,
            ground_speed: Arinc::default(),
            aircraft_on_ground: true,
            aircraft_ppos_local: [0.0, 0.0],
            zoom_inverse_scale: 1.0,
        };
        s.write_arinc("L:A32NX_OANS_BTV_REMAINING_DIST_TO_EXIT", s.remaining_dist_to_exit);
        s.write_arinc("L:A32NX_OANS_BTV_REMAINING_DIST_TO_RWY_END", s.remaining_dist_to_rwy_end);
        s.write_arinc("L:A32NX_OANS_BTV_REQ_STOPPING_DISTANCE", s.requested_stopping_dist);
        s.write_arinc("L:A32NX_OANS_RWY_LENGTH", s.runway_length);
        s.write_arinc("L:A32NX_OANS_RWY_BEARING", s.runway_bearing);
        s.clear_selection();
        s.clear_btv_data();
        s.drain();
        s
    }

    fn write_var(&mut self, name: &str, value: f64) {
        self.out.vars.insert(name.to_string(), value);
    }

    fn write_arinc(&mut self, name: &str, w: Arinc) {
        self.out.vars.insert(name.to_string(), w.raw());
    }

    fn publish(&mut self, topic: &str, value: Value) {
        self.out.log.push((topic.to_string(), value.clone()));
        self.out.bus.insert(topic.to_string(), value.clone());
        self.dispatch(topic, value);
    }

    fn drain(&mut self) {
        while !self.pending_runways.is_empty() {
            let r = self.pending_runways.remove(0);
            self.set_btv_runway_from_navdata(&r);
        }
    }

    fn dispatch(&mut self, topic: &str, value: Value) {
        {
            match topic {
                "oansSelectedLandingRunway" => {
                    self.oans_runway = value.as_str().map(str::to_string);
                    self.reevaluate_runway();
                }
                "oansSelectedExit" => {
                    let exit = value.as_str().map(str::to_string);
                    if exit != self.oans_exit {
                        self.oans_exit = exit;
                        self.reevaluate_exit();
                    }
                }
                "oansExitCoordinates" => {
                    self.oans_exit_coords = Coordinates {
                        lat: value["lat"].as_f64().unwrap_or(0.0),
                        long: value["long"].as_f64().unwrap_or(0.0),
                    };
                    self.reevaluate_exit();
                }
                "oans_remove_btv_data" => {
                    let changed = self.last_remove_btv_data.as_ref() != Some(&value);
                    self.last_remove_btv_data = Some(value.clone());
                    if changed && value.as_bool() == Some(true) {
                        self.clear_selection();
                    }
                }
                _ => {}
            }
        }
    }

    pub fn bus_event(&mut self, topic: &str, value: Value) {
        self.publish(topic, value);
        self.drain();
    }

    pub fn set_fms_landing_runway(&mut self, runway: Option<&str>) {
        self.fms_landing_runway = runway.map(str::to_string);
        self.reevaluate_runway();
        self.drain();
    }

    pub fn set_manual_stopping_distance(&mut self, distance: Option<f64>) {
        self.set_btv_fallback_stopping_distance(distance);
        self.drain();
    }

    fn reevaluate_runway(&mut self) {
        let mapped = match &self.oans_runway {
            Some(r) if !r.is_empty() => Some(r.clone()),
            _ => self.fms_landing_runway.clone(),
        };
        if mapped != self.mapped_runway {
            self.mapped_runway = mapped.clone();
            match mapped {
                Some(r) if !r.is_empty() => {
                    if self.in_set_runway {
                        self.pending_runways.push(r);
                    } else {
                        self.set_btv_runway_from_navdata(&r);
                    }
                }
                _ => self.clear_btv_data(),
            }
        }
    }

    fn reevaluate_exit(&mut self) {
        let c = self.oans_exit_coords;
        if let Some(name) = self.oans_exit.clone() {
            if !name.is_empty() && c.lat != 0.0 && c.long != 0.0 {
                self.set_exit_from_oans(&name, c);
            }
        }
    }

    fn run_is_set(&self) -> bool {
        self.threshold.is_some() && self.opposite_threshold.is_some()
    }

    fn set_btv_runway_from_navdata(&mut self, rwy_ident: &str) {
        let was = self.in_set_runway;
        self.in_set_runway = true;
        self.set_btv_runway_inner(rwy_ident);
        self.in_set_runway = was;
    }

    fn set_btv_runway_inner(&mut self, rwy_ident: &str) {
        let icao: String = rwy_ident.chars().take(4).collect();
        let Some(arp) = self.nav.airport_location(&icao) else {
            self.clear_btv_data();
            return;
        };
        self.arp = Some(arp);
        let runway = self.nav.runways(&icao).into_iter().find(|r| r.ident == rwy_ident);
        let Some(runway) = runway else {
            self.clear_btv_data();
            return;
        };
        let opposite = place_bearing_distance(runway.threshold_location, runway.bearing, runway.length / METRES_TO_NAUTICAL_MILES);
        self.threshold = Some(global_to_airport_coordinates(arp, runway.threshold_location));
        self.opposite_threshold = Some(global_to_airport_coordinates(arp, opposite));

        self.publish("ropsDetectedRunwayLda", json!(runway.length));
        self.publish("oansSelectedLandingRunway", json!(rwy_ident));

        self.runway_length = Arinc { value: runway.length, ssm: Ssm::NormalOperation };
        self.runway_bearing = Arinc { value: runway.bearing, ssm: Ssm::NormalOperation };
        self.write_arinc("L:A32NX_OANS_RWY_LENGTH", self.runway_length);
        self.write_arinc("L:A32NX_OANS_RWY_BEARING", self.runway_bearing);

        let rem = Arinc { value: runway.length - BTV_MIN_TOUCHDOWN_ZONE_DISTANCE, ssm: Ssm::NormalOperation };
        self.remaining_dist_to_rwy_end = rem;
        self.remaining_dist_to_exit = rem;
        self.write_arinc("L:A32NX_OANS_BTV_REMAINING_DIST_TO_RWY_END", rem);
        self.write_arinc("L:A32NX_OANS_BTV_REMAINING_DIST_TO_EXIT", rem);

        self.landing_runway = Some(runway);
        self.appr_different_runway = false;
        self.write_var("L:A32NX_BTV_APPR_DIFFERENT_RUNWAY", 0.0);
    }

    fn set_btv_fallback_stopping_distance(&mut self, distance: Option<f64>) {
        let (Some(distance), Some(rwy), Some(arp)) = (distance, self.landing_runway.clone(), self.arp) else { return };
        if distance == 0.0 || distance.is_nan() || distance <= BTV_MIN_TOUCHDOWN_ZONE_DISTANCE {
            return;
        }
        let exit_location = place_bearing_distance(rwy.threshold_location, rwy.bearing, distance / METRES_TO_NAUTICAL_MILES);
        self.exit_position = Some(global_to_airport_coordinates(arp, exit_location));
        self.publish("oansSelectedExit", json!("N/A"));
        self.publish("oansExitCoordinates", json!({"lat": exit_location.lat, "long": exit_location.long}));
        self.publish("ndBtvMessage", json!(format!("BTV {}/MANUAL", ident_tail(&rwy.ident))));
    }

    fn set_exit_from_oans(&mut self, exit_name: &str, exit_oans_reference: Coordinates) {
        let (Some(rwy), Some(arp)) = (self.landing_runway.clone(), self.arp) else { return };
        let exit_pos = global_to_airport_coordinates(arp, exit_oans_reference);
        self.exit_position = Some(exit_pos);
        let t = self.threshold.unwrap_or([f64::NAN, f64::NAN]);
        let exit_distance = point_distance(t[0], t[1], exit_pos[0], exit_pos[1]) - BTV_MIN_TOUCHDOWN_ZONE_DISTANCE;
        self.requested_stopping_dist.value = exit_distance;
        self.requested_stopping_dist.ssm = Ssm::NormalOperation;
        self.write_arinc("L:A32NX_OANS_BTV_REQ_STOPPING_DISTANCE", self.requested_stopping_dist);
        self.publish("ndBtvMessage", json!(format!("BTV {}/{}", ident_tail(&rwy.ident), exit_name)));
    }

    fn radio_altitude(&self) -> f64 {
        let ra = &self.inputs.radio_altitude;
        let valid: Vec<bool> = ra.iter().map(|r| !r.is_invalid()).collect();
        let n = valid.iter().filter(|v| **v).count();
        match n {
            1 => {
                if valid[0] {
                    ra[0].value
                } else if valid[1] {
                    ra[1].value
                } else {
                    ra[2].value
                }
            }
            2 => {
                let mut sum = 0.0;
                for i in 0..3 {
                    if valid[i] {
                        sum += ra[i].value;
                    }
                }
                sum / 2.0
            }
            3 => {
                let mut v = [ra[0].value, ra[1].value, ra[2].value];
                v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                v[1]
            }
            _ => 1000.0,
        }
    }

    fn on_ground(&self) -> bool {
        let [g1, g2] = self.inputs.lgciu_discrete_word_2;
        if g1.is_normal() {
            g1.bit_value(11)
        } else {
            g2.bit_value_or(11, false)
        }
    }

    fn vs_high(&self) -> bool {
        self.inputs.vertical_speed.iter().any(|v| v.value_or(0.0) > 400.0)
    }

    fn update_ppos(&mut self) {
        let i = &self.inputs;
        let mut pick = None;
        for k in 0..3 {
            if !i.latitude[k].is_invalid() && !i.longitude[k].is_invalid() {
                pick = Some((i.latitude[k].value, i.longitude[k].value));
                break;
            }
        }
        let (lat, long) = pick.unwrap_or((0.0, 0.0));
        self.ppos = Coordinates { lat, long };
    }

    fn detect_landing_runway(&self) -> (bool, Option<(String, String)>) {
        let ra = self.radio_altitude();
        if ra < 300.0 || ra > 500.0 || self.inputs.fws_flight_phase < 9.0 || self.inputs.fws_flight_phase > 11.0 || self.vs_high() || (self.ppos.lat == 0.0 && self.ppos.long == 0.0) {
            return (false, None);
        }
        let th = &self.inputs.true_heading;
        let heading = if !th[0].is_invalid() {
            th[0].value
        } else if !th[1].is_invalid() {
            th[1].value
        } else if !th[2].is_invalid() {
            th[2].value
        } else {
            0.0
        };
        let dist_to_td = ra / FEET_TO_NAUTICAL_MILES / (3.0f64).to_radians().tan();
        let td_point = place_bearing_distance(self.ppos, heading, dist_to_td);
        let mut best: Option<(String, String)> = None;
        let mut best_dist = 10.0;
        for airport in self.nav.nearby_airports(self.ppos, 5.0, 5) {
            for runway in self.nav.runways(&airport) {
                let dist = distance_to(td_point, runway.threshold_location);
                if dist < best_dist && dist < 2.0 && (runway.bearing - heading).abs() < 30.0 {
                    best = Some((airport.clone(), runway.ident.clone()));
                    best_dist = dist;
                }
            }
        }
        (true, best)
    }

    fn clear_btv_data(&mut self) {
        self.remaining_dist_to_exit = Arinc { value: 0.0, ssm: Ssm::NoComputedData };
        self.remaining_dist_to_rwy_end = Arinc { value: 0.0, ssm: Ssm::NoComputedData };
        self.write_arinc("L:A32NX_OANS_BTV_REMAINING_DIST_TO_EXIT", self.remaining_dist_to_exit);
        self.write_arinc("L:A32NX_OANS_BTV_REMAINING_DIST_TO_RWY_END", self.remaining_dist_to_rwy_end);
        self.publish("oans_remove_btv_data", json!(true));
        self.publish("ropsDetectedAirport", Value::Null);
        self.publish("ropsDetectedRunway", Value::Null);
        self.publish("oansSelectedLandingRunway", Value::Null);
        self.publish("ndBtvMessage", json!(""));
        self.appr_different_runway = false;
        self.write_var("L:A32NX_BTV_APPR_DIFFERENT_RUNWAY", 0.0);
    }

    fn update_remaining_distances(&mut self) {
        let ra = self.radio_altitude();
        if ra > 600.0
            || self.inputs.fws_flight_phase < 9.0
            || self.inputs.fws_flight_phase > 11.0
            || self.vs_high()
            || self.arp.is_none()
            || (self.ppos.lat == 0.0 && self.ppos.long == 0.0)
        {
            self.remaining_dist_to_exit = Arinc { value: 0.0, ssm: Ssm::NoComputedData };
            self.remaining_dist_to_rwy_end = Arinc { value: 0.0, ssm: Ssm::NoComputedData };
            self.write_arinc("L:A32NX_OANS_BTV_REMAINING_DIST_TO_EXIT", self.remaining_dist_to_exit);
            self.write_arinc("L:A32NX_OANS_BTV_REMAINING_DIST_TO_RWY_END", self.remaining_dist_to_rwy_end);
            return;
        }
        let arp = self.arp.unwrap();
        self.local_ppos = global_to_airport_coordinates(arp, self.ppos);
        if let Some(t) = self.threshold {
            if let Some(o) = self.opposite_threshold {
                let from_tdz = point_distance(t[0], t[1], o[0], o[1]) - BTV_MIN_TOUCHDOWN_ZONE_DISTANCE;
                let dist = point_distance(self.local_ppos[0], self.local_ppos[1], o[0], o[1]);
                self.remaining_dist_to_rwy_end = Arinc::normal(round_to(from_tdz.min(dist), 0.1));
                self.write_arinc("L:A32NX_OANS_BTV_REMAINING_DIST_TO_RWY_END", self.remaining_dist_to_rwy_end);
            }
            if let Some(e) = self.exit_position {
                let from_tdz = point_distance(t[0], t[1], e[0], e[1]) - BTV_MIN_TOUCHDOWN_ZONE_DISTANCE;
                let dist = point_distance(self.local_ppos[0], self.local_ppos[1], e[0], e[1]);
                self.remaining_dist_to_exit = Arinc::normal(round_to(from_tdz.min(dist), 0.1));
                self.write_arinc("L:A32NX_OANS_BTV_REMAINING_DIST_TO_EXIT", self.remaining_dist_to_exit);
            }
        }
    }

    pub fn on_update(&mut self, delta_ms: f64) {
        self.accumulated_ms += delta_ms;
        if self.accumulated_ms < UPDATE_PERIOD_MS {
            return;
        }
        self.accumulated_ms = 0.0;

        self.update_ppos();
        let (armed, detected) = self.detect_landing_runway();
        if !armed {
            self.appr_different_runway = false;
            self.write_var("L:A32NX_BTV_APPR_DIFFERENT_RUNWAY", 0.0);
        }
        self.update_remaining_distances();

        let btv_active_on_ground = self.inputs.auto_brake_active && self.on_ground() && self.inputs.auto_brake_mode == 1.0;
        if self.btv_active_on_ground_last && !btv_active_on_ground {
            self.clear_btv_data();
        }
        self.btv_active_on_ground_last = btv_active_on_ground;

        if let Some((airport, runway)) = detected {
            let ra = self.radio_altitude();
            if !self.run_is_set() || (self.run_is_set() && ra < 350.0 && self.oans_runway.as_deref() != Some(runway.as_str())) {
                self.set_btv_runway_from_navdata(&runway);
                self.appr_different_runway = true;
                self.write_var("L:A32NX_BTV_APPR_DIFFERENT_RUNWAY", 1.0);
                self.publish("ropsDetectedAirport", json!(airport));
                self.publish("ropsDetectedRunway", json!(runway));
                self.publish("ndBtvMessage", json!(format!("RWY{}", ident_tail(&runway))));
            }
        }
        self.drain();
    }

    pub fn select_runway_from_oans(&mut self, runway: &str, centerline: &[Position], threshold_location: Position, threshold_brngmag: Option<f64>) {
        self.clear_selection();
        let thr = threshold_location;
        self.sel.threshold = Some(thr);
        let first = centerline[0];
        let last = centerline[centerline.len() - 1];
        let dist1 = point_distance(thr[0], thr[1], first[0], first[1]);
        let dist2 = point_distance(thr[0], thr[1], last[0], last[1]);
        self.sel.opposite_threshold = Some(if dist1 > dist2 { first } else { last });
        let lda = if dist1 > dist2 { dist1 } else { dist2 };
        self.sel.lda = Some(lda);
        self.sel.bearing_true = Some(threshold_brngmag.unwrap_or(0.0));
        self.sel.runway = Some(runway.to_string());
        self.publish("oansSelectedLandingRunway", json!(runway));
        self.drain();
    }

    pub fn select_exit_from_oans(&mut self, exit: &str, exit_line: &[Position], arp: Option<Coordinates>) {
        let (Some(_), Some(thr), Some(opp)) = (self.sel.runway.clone(), self.sel.threshold, self.sel.opposite_threshold) else { return };
        let last_index = exit_line.len() - 1;
        let loc1 = exit_line[0];
        let loc2 = exit_line[last_index];
        let d1 = point_to_line_distance(loc1, thr, opp);
        let d2 = point_to_line_distance(loc2, thr, opp);
        let start_dist = if d1 < d2 { point_distance(thr[0], thr[1], loc1[0], loc1[1]) } else { point_distance(thr[0], thr[1], loc2[0], loc2[1]) };
        let angle = if d1 < d2 {
            point_angle(thr[0], thr[1], loc1[0], loc1[1]) - point_angle(loc1[0], loc1[1], exit_line[1][0], exit_line[1][1])
        } else {
            point_angle(thr[0], thr[1], loc2[0], loc2[1])
                - point_angle(loc2[0], loc2[1], exit_line[last_index - 1][0], exit_line[last_index - 1][1])
        };
        if angle.abs() > 120.0 || d1.min(d2) > 50.0 || start_dist < BTV_MIN_TOUCHDOWN_ZONE_DISTANCE {
            return;
        }
        let exit_pos = if d1 < d2 { loc1 } else { loc2 };
        self.sel.exit_position = Some(exit_pos);
        let Some(arp) = arp else { return };
        let global = airport_to_global_coordinates(arp, exit_pos);
        self.publish("oansExitCoordinates", json!({"lat": global.lat, "long": global.long}));
        let exit_distance = point_distance(thr[0], thr[1], exit_pos[0], exit_pos[1]) - BTV_MIN_TOUCHDOWN_ZONE_DISTANCE;
        self.publish("oansSelectedExit", json!(exit));
        let mut path: Vec<Position> = exit_line.to_vec();
        if d1 < d2 {
            path.insert(0, thr);
        } else {
            path.push(thr);
        }
        self.sel.path = path;
        self.sel.exit_distance = Some(exit_distance);
        self.sel.exit = Some(exit.to_string());
        self.drain();
    }

    pub fn clear_selection(&mut self) {
        self.sel = Selection::default();
        self.publish("oansSelectedLandingRunway", Value::Null);
        self.publish("oansSelectedExit", Value::Null);
    }

    pub fn selected_runway(&self) -> Option<&str> {
        self.sel.runway.as_deref()
    }

    pub fn selected_exit(&self) -> Option<&str> {
        self.sel.exit.as_deref()
    }

    pub fn selected_exit_distance(&self) -> Option<f64> {
        self.sel.exit_distance
    }

    pub fn selected_lda(&self) -> Option<f64> {
        self.sel.lda
    }

    pub fn btv_path(&self) -> &[Position] {
        &self.sel.path
    }

    pub fn remaining_dist_to_rwy_end(&self) -> Arinc {
        self.remaining_dist_to_rwy_end
    }

    pub fn update_rwy_ahead_advisory(&mut self, global_pos: Coordinates, airport_ref: Coordinates, aircraft_bearing: f64, runways: &[RunwayArea], now_ms: f64) {
        let i = RwyAheadInputs {
            on_ground: self.on_ground(),
            ground_speed: self.ground_speed,
            ppos_local: Some(self.aircraft_ppos_local),
            global_pos,
            airport_ref,
            aircraft_bearing,
            now_ms,
        };
        if let Some(t) = self.rwy_ahead.update(&i, runways) {
            self.write_arinc(OANS_WORD_1, t.word);
            self.publish("ndRwyAheadQfu", json!(t.qfu));
            self.drain();
        }
    }

    pub fn rwy_ahead_volume(&self) -> [Position; 5] {
        self.rwy_ahead.volume
    }

    pub fn stop_line_radio_altitude(&self) -> Arinc {
        let ra = &self.inputs.radio_altitude;
        if !ra[0].is_invalid() {
            ra[0]
        } else if !ra[1].is_invalid() {
            ra[1]
        } else {
            ra[2]
        }
    }

    pub fn stop_lines(&self, radio_altitude: Arinc) -> Vec<StopLine> {
        let (Some(thr), Some(opp), Some(bearing), Some(lda)) =
            (self.sel.threshold, self.sel.opposite_threshold, self.sel.bearing_true, self.sel.lda)
        else {
            return Vec::new();
        };
        let ppos = self.aircraft_ppos_local;
        let dry_wet_updating = radio_altitude.value_or(1000.0) <= 600.0;
        let from_threshold = point_distance(thr[0], thr[1], ppos[0], ppos[1]);
        let from_end = point_distance(opp[0], opp[1], ppos[0], ppos[1]);
        let past_threshold = from_end < lda;
        let touchdown = if dry_wet_updating && past_threshold && from_threshold > BTV_MIN_TOUCHDOWN_ZONE_DISTANCE {
            from_threshold
        } else {
            BTV_MIN_TOUCHDOWN_ZONE_DISTANCE
        };
        let dry_over = touchdown + self.dry_stopping_distance > lda;
        let wet_over = touchdown + self.wet_stopping_distance > lda;
        let lat_distance = 27.5 / self.zoom_inverse_scale;
        let mk = |p: [f64; 2]| -> [Position; 3] {
            let p1 = [lat_distance * (180.0 - bearing).to_radians().cos() + p[0], lat_distance * (180.0 - bearing).to_radians().sin() + p[1]];
            let p2 = [lat_distance * (-bearing).to_radians().cos() + p[0], lat_distance * (-bearing).to_radians().sin() + p[1]];
            [p1, p, p2]
        };
        let mut lines = Vec::new();
        if self.dry_stopping_distance > 0.0 && !self.aircraft_on_ground {
            let d = self.dry_stopping_distance.min(lda - touchdown + CLAMP_DRY_STOPBAR_DISTANCE);
            let p = fractional_point_along_line(thr[0], thr[1], opp[0], opp[1], (touchdown + d) / lda);
            lines.push(StopLine { kind: "DRY", points: mk(p), overrun: dry_over, colour: if dry_over { "red" } else { "magenta" } });
        }
        if self.wet_stopping_distance > 0.0 && !self.aircraft_on_ground {
            let d = self.wet_stopping_distance.min(lda - touchdown + CLAMP_WET_STOPBAR_DISTANCE);
            let p = fractional_point_along_line(thr[0], thr[1], opp[0], opp[1], (touchdown + d) / lda);
            let colour = if !dry_over && !wet_over {
                "magenta"
            } else if !dry_over && wet_over {
                "amber"
            } else {
                "red"
            };
            lines.push(StopLine { kind: "WET", points: mk(p), overrun: wet_over, colour });
        }
        let dist_end = Arinc::from_raw(self.remaining_dist_to_rwy_end.raw());
        if self.live_stopping_distance > 0.0 && self.aircraft_on_ground && self.ground_speed.value > 25.0 && dist_end.ssm == Ssm::NormalOperation {
            let over = dist_end.value - self.live_stopping_distance <= 0.0;
            let d = if over && self.live_stopping_distance - dist_end.value > CLAMP_DRY_STOPBAR_DISTANCE {
                dist_end.value + 100.0
            } else {
                self.live_stopping_distance
            };
            let p = fractional_point_along_line(thr[0], thr[1], opp[0], opp[1], (from_threshold + d) / lda);
            lines.push(StopLine { kind: "STOP", points: mk(p), overrun: over, colour: if over { "red" } else { "green" } });
        }
        lines
    }
}

pub fn ident_tail(ident: &str) -> String {
    ident.chars().skip(4).collect()
}

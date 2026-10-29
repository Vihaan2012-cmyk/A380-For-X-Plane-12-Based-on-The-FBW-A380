use serde::{Deserialize, Serialize};

use crate::geo::Coordinates;
use crate::hold::{parse_hold_from_procedure_leg, HoldData};
use crate::winds::{DraftWinds, PendingWindUplink, WindData, WindEntry};
use crate::naming::{js_round, procedure_leg_ident_and_annotation};
use crate::navdb::{are_database_items_equal, Airway, Fix, NavDatabase, Procedure, ProcedureLeg, Runway, Transition};

pub const FLAG_DIRECT_TO_TURNING_POINT: u32 = 1;
pub const FLAG_PENDING_DIRECT_TO_TURNING_POINT: u32 = 2;
pub const FLAG_ORIGIN: u32 = 4;
pub const FLAG_COPIED_WITH_PREDICTIONS: u32 = 8;

pub const DESCRIPTOR_AIRPORT: i32 = 1;
pub const DESCRIPTOR_RUNWAY: i32 = 4;
pub const APPROACH_DESCRIPTOR_MISSED_APPROACH_POINT: i32 = 8;

pub const CONSTRAINT_UNKNOWN: i32 = 0;
pub const CONSTRAINT_CLB: i32 = 1;
pub const CONSTRAINT_DES: i32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegType {
    AF,
    CA,
    CD,
    CF,
    CI,
    CR,
    DF,
    FA,
    FC,
    FD,
    FM,
    HA,
    HF,
    HM,
    IF,
    PI,
    RF,
    TF,
    VA,
    VD,
    VI,
    VM,
    VR,
}

impl LegType {
    pub fn parse(s: &str) -> Option<LegType> {
        use LegType::*;
        Some(match s {
            "AF" => AF,
            "CA" => CA,
            "CD" => CD,
            "CF" => CF,
            "CI" => CI,
            "CR" => CR,
            "DF" => DF,
            "FA" => FA,
            "FC" => FC,
            "FD" => FD,
            "FM" => FM,
            "HA" => HA,
            "HF" => HF,
            "HM" => HM,
            "IF" => IF,
            "PI" => PI,
            "RF" => RF,
            "TF" => TF,
            "VA" => VA,
            "VD" => VD,
            "VI" => VI,
            "VM" => VM,
            "VR" => VR,
            _ => return None,
        })
    }

    pub fn as_str(&self) -> &'static str {
        use LegType::*;
        match self {
            AF => "AF",
            CA => "CA",
            CD => "CD",
            CF => "CF",
            CI => "CI",
            CR => "CR",
            DF => "DF",
            FA => "FA",
            FC => "FC",
            FD => "FD",
            FM => "FM",
            HA => "HA",
            HF => "HF",
            HM => "HM",
            IF => "IF",
            PI => "PI",
            RF => "RF",
            TF => "TF",
            VA => "VA",
            VD => "VD",
            VI => "VI",
            VM => "VM",
            VR => "VR",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AltitudeConstraint {
    pub descriptor: String,
    pub altitude1: f64,
    pub altitude2: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpeedConstraint {
    pub descriptor: String,
    pub speed: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CruiseStep {
    pub distance_before_termination: f64,
    pub to_altitude: f64,
    pub waypoint_index: usize,
    pub is_ignored: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegDefinition {
    pub procedure_ident: String,
    pub leg_type: LegType,
    pub overfly: bool,
    pub waypoint: Option<Fix>,
    pub waypoint_descriptor: Option<i32>,
    pub approach_waypoint_descriptor: Option<i32>,
    pub turn_direction: Option<char>,
    pub altitude_descriptor: Option<String>,
    pub altitude1: Option<f64>,
    pub altitude2: Option<f64>,
    pub speed_descriptor: Option<String>,
    pub speed: Option<f64>,
    pub course: Option<f64>,
    pub mag_var: Option<f64>,
    pub rho: Option<f64>,
    pub theta: Option<f64>,
    pub length: Option<f64>,
    pub length_time: Option<f64>,
    pub recommended_navaid: Option<Fix>,
    pub vertical_angle: Option<f64>,
    #[serde(default)]
    pub is_engine_out_branch: bool,
}

impl LegDefinition {
    pub fn blank(leg_type: LegType) -> Self {
        LegDefinition {
            procedure_ident: String::new(),
            leg_type,
            overfly: false,
            waypoint: None,
            waypoint_descriptor: None,
            approach_waypoint_descriptor: None,
            turn_direction: None,
            altitude_descriptor: None,
            altitude1: None,
            altitude2: None,
            speed_descriptor: None,
            speed: None,
            course: None,
            mag_var: None,
            rho: None,
            theta: None,
            length: None,
            length_time: None,
            recommended_navaid: None,
            vertical_angle: None,
            is_engine_out_branch: false,
        }
    }

    fn enroute(waypoint: &Fix, leg_type: LegType) -> Self {
        LegDefinition { waypoint: Some(waypoint.clone()), mag_var: Some(waypoint.mag_var.unwrap_or(0.)), ..LegDefinition::blank(leg_type) }
    }

    pub fn from_procedure_leg(leg: &ProcedureLeg) -> Option<Self> {
        Some(LegDefinition {
            procedure_ident: leg.procedure_ident.clone(),
            leg_type: LegType::parse(&leg.leg_type)?,
            overfly: leg.overfly,
            waypoint: leg.waypoint.clone(),
            waypoint_descriptor: leg.waypoint_descriptor,
            approach_waypoint_descriptor: leg.approach_waypoint_descriptor,
            turn_direction: leg.turn_direction,
            altitude_descriptor: leg.altitude_descriptor.clone(),
            altitude1: leg.altitude1,
            altitude2: leg.altitude2,
            speed_descriptor: leg.speed_descriptor.clone(),
            speed: leg.speed,
            course: leg.course,
            mag_var: leg.mag_var,
            rho: leg.rho,
            theta: leg.theta,
            length: leg.length,
            length_time: leg.length_time,
            recommended_navaid: leg.recommended_navaid.clone(),
            vertical_angle: leg.vertical_angle,
            is_engine_out_branch: leg.is_engine_out_branch,
        })
    }

    pub fn assign_from(&mut self, src: &LegDefinition) {
        self.procedure_ident = src.procedure_ident.clone();
        self.leg_type = src.leg_type;
        self.overfly = src.overfly;
        macro_rules! take {
            ($($f:ident),*) => { $(if src.$f.is_some() { self.$f = src.$f.clone(); })* };
        }
        take!(
            waypoint,
            waypoint_descriptor,
            approach_waypoint_descriptor,
            turn_direction,
            altitude_descriptor,
            altitude1,
            altitude2,
            speed_descriptor,
            speed,
            course,
            mag_var,
            rho,
            theta,
            length,
            length_time,
            recommended_navaid,
            vertical_angle
        );
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Leg {
    pub definition: LegDefinition,
    pub ident: String,
    pub annotation: String,
    pub leg_type: LegType,
    pub flags: u32,
    pub constraint_type: i32,
    pub pilot_altitude: Option<AltitudeConstraint>,
    pub pilot_speed: Option<SpeedConstraint>,
    pub cruise_step: Option<CruiseStep>,
    #[serde(default)]
    pub default_hold: Option<HoldData>,
    #[serde(default)]
    pub modified_hold: Option<HoldData>,
    #[serde(default)]
    pub cruise_wind_entries: Vec<WindEntry>,
}

impl Leg {
    pub fn from_enroute_fix(waypoint: &Fix, airway_ident: Option<&str>, leg_type: LegType) -> Leg {
        Leg {
            definition: LegDefinition::enroute(waypoint, leg_type),
            ident: waypoint.ident.clone(),
            annotation: airway_ident.unwrap_or("").to_string(),
            leg_type,
            flags: 0,
            constraint_type: CONSTRAINT_UNKNOWN,
            pilot_altitude: None,
            pilot_speed: None,
            cruise_step: None,
            default_hold: None,
            modified_hold: None,
            cruise_wind_entries: Vec::new(),
        }
    }

    pub fn from_procedure_leg(procedure_leg: &ProcedureLeg, procedure_ident: &str, constraint_type: i32) -> Option<Leg> {
        let definition = LegDefinition::from_procedure_leg(procedure_leg)?;
        let (ident, annotation) = procedure_leg_ident_and_annotation(&definition, procedure_ident);
        let default_hold = parse_hold_from_procedure_leg(&definition);
        Some(Leg {
            leg_type: definition.leg_type,
            definition,
            ident,
            annotation,
            flags: 0,
            constraint_type,
            pilot_altitude: None,
            pilot_speed: None,
            cruise_step: None,
            default_hold,
            modified_hold: None,
            cruise_wind_entries: Vec::new(),
        })
    }

    pub fn from_airport_and_runway(procedure_ident: &str, airport: &Fix, runway: Option<&Runway>) -> Leg {
        let mut definition = LegDefinition::blank(LegType::IF);
        definition.mag_var = airport.mag_var;
        let ident;
        if let Some(runway) = runway {
            definition.waypoint = Some(runway.fix.clone());
            definition.waypoint_descriptor = Some(DESCRIPTOR_RUNWAY);
            definition.course = Some(runway.magnetic_bearing);
            ident = runway.fix.ident.clone();
        } else {
            definition.waypoint = Some(airport.clone());
            definition.waypoint_descriptor = Some(DESCRIPTOR_AIRPORT);
            ident = airport.ident.clone();
        }
        Leg {
            leg_type: LegType::IF,
            definition,
            ident,
            annotation: procedure_ident.to_string(),
            flags: 0,
            constraint_type: CONSTRAINT_UNKNOWN,
            pilot_altitude: None,
            pilot_speed: None,
            cruise_step: None,
            default_hold: None,
            modified_hold: None,
            cruise_wind_entries: Vec::new(),
        }
    }

    pub fn from_airport(airport: &Fix) -> Leg {
        Leg::from_airport_and_runway("", airport, None)
    }

    pub fn origin_extended_centerline(runway: &Runway, runway_leg: &Leg) -> Leg {
        let alt = runway.threshold_location.alt;
        let altitude = match alt {
            Some(a) if a.is_finite() => 10. * js_round(a / 10.) as f64 + 1500.,
            _ => 1500.,
        };
        let bearing = runway.magnetic_bearing;
        let prefix: String = runway_leg.ident.chars().take(3).collect();
        let annotation = format!("{prefix}{:03}", js_round(bearing));
        let mut definition = LegDefinition::blank(LegType::FA);
        definition.waypoint = runway_leg.termination_waypoint().cloned();
        definition.course = Some(bearing);
        definition.altitude1 = Some(altitude);
        definition.mag_var = runway.fix.mag_var;
        Leg {
            leg_type: LegType::FA,
            definition,
            ident: js_round(altitude).to_string(),
            annotation,
            flags: 0,
            constraint_type: CONSTRAINT_UNKNOWN,
            pilot_altitude: None,
            pilot_speed: None,
            cruise_step: None,
            default_hold: None,
            modified_hold: None,
            cruise_wind_entries: Vec::new(),
        }
    }

    pub fn destination_extended_centerline(runway: &Runway) -> Leg {
        let bearing = (runway.bearing + 180.).rem_euclid(360.);
        let origin = runway.threshold_location.coordinates();
        let point = origin.place_bearing_distance(bearing, 5.);
        let waypoint = Fix::lat_long("CF", point.lat, point.long);
        let mut definition = LegDefinition::blank(LegType::IF);
        definition.waypoint = Some(waypoint.clone());
        definition.mag_var = runway.fix.mag_var;
        Leg {
            leg_type: LegType::IF,
            definition,
            ident: waypoint.ident,
            annotation: String::new(),
            flags: 0,
            constraint_type: CONSTRAINT_UNKNOWN,
            pilot_altitude: None,
            pilot_speed: None,
            cruise_step: None,
            default_hold: None,
            modified_hold: None,
            cruise_wind_entries: Vec::new(),
        }
    }

    pub fn is_runway(&self) -> bool {
        self.definition.waypoint_descriptor == Some(DESCRIPTOR_RUNWAY)
    }

    pub fn with_definition_from(&mut self, from: &Leg) {
        self.definition.vertical_angle = from.definition.vertical_angle;
        self.definition.altitude_descriptor = from.definition.altitude_descriptor.clone();
        self.definition.altitude1 = from.definition.altitude1;
        self.definition.altitude2 = from.definition.altitude2;
        self.definition.speed_descriptor = from.definition.speed_descriptor.clone();
        self.definition.speed = from.definition.speed;
        self.definition.approach_waypoint_descriptor = from.definition.approach_waypoint_descriptor;
        self.definition.waypoint_descriptor = from.definition.waypoint_descriptor;
        self.definition.is_engine_out_branch = from.definition.is_engine_out_branch;
    }

    pub fn is_xf(&self) -> bool {
        use LegType::*;
        matches!(self.definition.leg_type, AF | CF | IF | DF | RF | TF | HF)
    }

    pub fn is_fx(&self) -> bool {
        use LegType::*;
        matches!(self.definition.leg_type, FA | FC | FD | FM)
    }

    pub fn is_hx(&self) -> bool {
        use LegType::*;
        matches!(self.definition.leg_type, HA | HF | HM)
    }

    pub fn is_xi(&self) -> bool {
        use LegType::*;
        matches!(self.definition.leg_type, PI | CI | VI)
    }

    pub fn is_vectors(&self) -> bool {
        use LegType::*;
        matches!(self.definition.leg_type, FM | VM)
    }

    pub fn termination_waypoint(&self) -> Option<&Fix> {
        if self.is_xf() || self.is_fx() || self.is_hx() {
            self.definition.waypoint.as_ref()
        } else {
            None
        }
    }

    pub fn terminates_with_waypoint(&self, waypoint: &Fix) -> bool {
        use LegType::*;
        if !self.is_xf() && !matches!(self.definition.leg_type, HF | HM) {
            return false;
        }
        self.definition.waypoint.as_ref().is_some_and(|w| w.database_id == waypoint.database_id)
    }

    pub fn location(&self) -> Option<Coordinates> {
        self.termination_waypoint().map(|w| w.location.coordinates())
    }

    pub fn has_database_altitude_constraint(&self) -> bool {
        self.definition.altitude_descriptor.as_deref().is_some_and(|d| !d.is_empty() && d != "G" && d != "H")
    }

    pub fn altitude_constraint(&self) -> Option<AltitudeConstraint> {
        if let Some(p) = &self.pilot_altitude {
            return Some(p.clone());
        }
        if self.has_database_altitude_constraint() {
            return Some(AltitudeConstraint {
                descriptor: self.definition.altitude_descriptor.clone().unwrap_or_default(),
                altitude1: self.definition.altitude1.unwrap_or(0.),
                altitude2: self.definition.altitude2,
            });
        }
        None
    }

    pub fn speed_constraint(&self) -> Option<SpeedConstraint> {
        if let Some(p) = &self.pilot_speed {
            return Some(p.clone());
        }
        match (&self.definition.speed_descriptor, self.definition.speed) {
            (Some(d), Some(s)) => Some(SpeedConstraint { descriptor: d.clone(), speed: s }),
            _ => None,
        }
    }

    pub fn clear_altitude_constraints(&mut self) {
        self.pilot_altitude = None;
        self.definition.altitude_descriptor = None;
        self.definition.altitude1 = None;
        self.definition.altitude2 = None;
    }

    pub fn clear_speed_constraints(&mut self) {
        self.pilot_speed = None;
        self.definition.speed_descriptor = None;
        self.definition.speed = None;
    }

    pub fn clear_constraints(&mut self) {
        self.definition.vertical_angle = None;
        self.clear_altitude_constraints();
        self.clear_speed_constraints();
    }

    pub fn requires_turn_direction(&self) -> bool {
        self.is_hx() || matches!(self.leg_type, LegType::PI | LegType::RF | LegType::AF)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Element {
    Leg(Box<Leg>),
    Disco,
}

impl Element {
    pub fn is_disco(&self) -> bool {
        matches!(self, Element::Disco)
    }

    pub fn leg(&self) -> Option<&Leg> {
        match self {
            Element::Leg(l) => Some(l),
            Element::Disco => None,
        }
    }

    pub fn leg_mut(&mut self) -> Option<&mut Leg> {
        match self {
            Element::Leg(l) => Some(l),
            Element::Disco => None,
        }
    }

    pub fn new_leg(leg: Leg) -> Element {
        Element::Leg(Box::new(leg))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SegmentClass {
    Departure,
    Enroute,
    Arrival,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SegmentKind {
    Origin,
    DepartureRunwayTransition,
    Departure,
    DepartureEnrouteTransition,
    Enroute,
    ArrivalEnrouteTransition,
    Arrival,
    ArrivalRunwayTransition,
    ApproachVia,
    Approach,
    Destination,
    MissedApproach,
}

pub const SEGMENT_KINDS: [SegmentKind; 12] = [
    SegmentKind::Origin,
    SegmentKind::DepartureRunwayTransition,
    SegmentKind::Departure,
    SegmentKind::DepartureEnrouteTransition,
    SegmentKind::Enroute,
    SegmentKind::ArrivalEnrouteTransition,
    SegmentKind::Arrival,
    SegmentKind::ArrivalRunwayTransition,
    SegmentKind::ApproachVia,
    SegmentKind::Approach,
    SegmentKind::Destination,
    SegmentKind::MissedApproach,
];

impl SegmentKind {
    pub fn class(self) -> SegmentClass {
        use SegmentKind::*;
        match self {
            Origin | DepartureRunwayTransition | Departure | DepartureEnrouteTransition => SegmentClass::Departure,
            Enroute => SegmentClass::Enroute,
            _ => SegmentClass::Arrival,
        }
    }

    pub fn slot(self) -> usize {
        SEGMENT_KINDS.iter().position(|k| *k == self).unwrap_or(0)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub kind: SegmentKind,
    pub legs: Vec<Element>,
    pub strung: bool,
    pub is_sequenced_missed_approach: bool,
}

impl Segment {
    fn new(kind: SegmentKind) -> Self {
        Segment { kind, legs: Vec::new(), strung: false, is_sequenced_missed_approach: false }
    }

    pub fn last_leg_index(&self) -> Option<usize> {
        self.legs.iter().rposition(|e| !e.is_disco())
    }

    pub fn find_index_of_waypoint(&self, waypoint: &Fix, after_index: i64) -> Option<usize> {
        self.legs.iter().enumerate().find(|(i, e)| (*i as i64) > after_index && e.leg().is_some_and(|l| l.terminates_with_waypoint(waypoint))).map(|(i, _)| i)
    }

    pub fn find_last_index_of_waypoint(&self, waypoint: &Fix, before_index: i64) -> Option<usize> {
        self.legs.iter().enumerate().rev().find(|(i, e)| (*i as i64) < before_index && e.leg().is_some_and(|l| l.terminates_with_waypoint(waypoint))).map(|(i, _)| i)
    }

    pub fn insert_necessary_discontinuities(&mut self) {
        let mut i = 0;
        while i < self.legs.len() {
            let vectors = self.legs[i].leg().is_some_and(|l| l.is_vectors());
            let next_is_disco = self.legs.get(i + 1).is_some_and(|e| e.is_disco());
            if vectors && !next_is_disco {
                self.legs.insert(i + 1, Element::Disco);
                i += 1;
            }
            i += 1;
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PendingAirwayEntry {
    pub from_index: Option<usize>,
    pub airway: Option<Airway>,
    pub to: Option<Fix>,
    pub is_dct: bool,
    pub is_auto_connected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PendingAirways {
    pub revised_leg_index: usize,
    pub revised_waypoint: Fix,
    pub elements: Vec<PendingAirwayEntry>,
    legs: Vec<Leg>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceData {
    pub cost_index: Option<f64>,
    pub cruise_flight_level: Option<f64>,
    pub database_transition_altitude: Option<f64>,
    pub database_transition_level: Option<f64>,
    pub pilot_tropopause: Option<f64>,
    pub zero_fuel_weight: Option<f64>,
    pub block_fuel: Option<f64>,
    pub default_thrust_reduction_altitude: Option<f64>,
    pub default_acceleration_altitude: Option<f64>,
    pub default_engine_out_acceleration_altitude: Option<f64>,
    pub pilot_thrust_reduction_altitude: Option<f64>,
    pub pilot_acceleration_altitude: Option<f64>,
    pub pilot_engine_out_acceleration_altitude: Option<f64>,
    pub default_missed_thrust_reduction_altitude: Option<f64>,
    pub default_missed_acceleration_altitude: Option<f64>,
    pub default_missed_engine_out_acceleration_altitude: Option<f64>,
    pub pilot_missed_thrust_reduction_altitude: Option<f64>,
    pub pilot_missed_acceleration_altitude: Option<f64>,
    pub pilot_missed_engine_out_acceleration_altitude: Option<f64>,
    pub climb_speed_limit_speed: Option<f64>,
    pub climb_speed_limit_altitude: Option<f64>,
    pub descent_speed_limit_speed: Option<f64>,
    pub descent_speed_limit_altitude: Option<f64>,
    pub alternate_climb_speed_limit_speed: Option<f64>,
    pub alternate_climb_speed_limit_altitude: Option<f64>,
    pub alternate_descent_speed_limit_speed: Option<f64>,
    pub alternate_descent_speed_limit_altitude: Option<f64>,
    pub is_climb_speed_limit_pilot_entered: bool,
    pub is_descent_speed_limit_pilot_entered: bool,
    pub is_alternate_climb_speed_limit_pilot_entered: bool,
    pub is_alternate_descent_speed_limit_pilot_entered: bool,
    pub pilot_alternate_fuel: Option<f64>,
}

#[derive(Debug)]
pub struct FmsError(pub String);

impl std::fmt::Display for FmsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for FmsError {
    fn from(s: String) -> Self {
        FmsError(s)
    }
}

pub type FmsResult<T> = Result<T, FmsError>;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Sel<T> {
    #[default]
    Unset,
    NoneSelected,
    Set(T),
}

impl<T> Sel<T> {
    pub fn get(&self) -> Option<&T> {
        match self {
            Sel::Set(t) => Some(t),
            _ => None,
        }
    }
}

pub const RESTRING_DEFAULT: u32 = 0;
pub const RESTRING_DEPARTURE: u32 = 1;
pub const RESTRING_ARRIVAL: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueuedOp {
    Restring(u32),
    RebuildArrivalAndApproach,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlightPlan {
    pub index: usize,
    pub origin_runway: Option<Runway>,
    pub destination_runway: Option<Runway>,
    pub departure: Sel<Procedure>,
    pub departure_runway_transition: Option<Transition>,
    pub departure_enroute_transition: Sel<Transition>,
    pub arrival: Sel<Procedure>,
    pub arrival_enroute_transition: Sel<Transition>,
    pub arrival_runway_transition: Option<Transition>,
    pub approach_via: Sel<Transition>,
    pub approach: Option<Procedure>,
    #[serde(skip)]
    pub available_origin_runways: Vec<Runway>,
    #[serde(skip)]
    pub available_destination_runways: Vec<Runway>,
    #[serde(skip)]
    pub available_departures: Vec<Procedure>,
    #[serde(skip)]
    pub available_arrivals: Vec<Procedure>,
    #[serde(skip)]
    pub available_approaches: Vec<Procedure>,
    #[serde(skip)]
    pub available_approach_vias: Vec<Transition>,
    #[serde(skip)]
    pub queued_operations: Vec<QueuedOp>,
    pub segments: Vec<Segment>,
    pub active_leg_index: i64,
    pub origin_airport: Option<Fix>,
    pub destination_airport: Option<Fix>,
    pub alternate: Option<Box<FlightPlan>>,
    pub flags: u32,
    pub flight_number: String,
    pub performance: PerformanceData,
    #[serde(skip)]
    pub pending_airways: Option<PendingAirways>,
    pub version: u32,
    pub was_modified: bool,
    #[serde(default)]
    pub winds: WindData,
    #[serde(default)]
    pub engine_out_departure: EngineOutDeparture,
    #[serde(skip)]
    pub draft_winds: DraftWinds,
    #[serde(skip)]
    pub pending_wind_uplink: PendingWindUplink,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EngineOutDeparture {
    pub procedure_ident: Option<String>,
    pub legs: Vec<Element>,
    pub version: u32,
}

impl FlightPlan {
    pub fn empty(index: usize) -> FlightPlan {
        FlightPlan {
            index,
            origin_runway: None,
            destination_runway: None,
            departure: Sel::Unset,
            departure_runway_transition: None,
            departure_enroute_transition: Sel::Unset,
            arrival: Sel::Unset,
            arrival_enroute_transition: Sel::Unset,
            arrival_runway_transition: None,
            approach_via: Sel::Unset,
            approach: None,
            available_origin_runways: Vec::new(),
            available_destination_runways: Vec::new(),
            available_departures: Vec::new(),
            available_arrivals: Vec::new(),
            available_approaches: Vec::new(),
            available_approach_vias: Vec::new(),
            queued_operations: Vec::new(),
            segments: SEGMENT_KINDS.iter().map(|k| Segment::new(*k)).collect(),
            active_leg_index: 1,
            origin_airport: None,
            destination_airport: None,
            alternate: None,
            flags: 0,
            flight_number: String::new(),
            performance: PerformanceData::default(),
            pending_airways: None,
            version: 0,
            was_modified: false,
            winds: WindData::default(),
            engine_out_departure: EngineOutDeparture::default(),
            draft_winds: DraftWinds::default(),
            pending_wind_uplink: PendingWindUplink::default(),
        }
    }

    pub fn seg(&self, kind: SegmentKind) -> &Segment {
        &self.segments[kind.slot()]
    }

    pub fn seg_mut(&mut self, kind: SegmentKind) -> &mut Segment {
        &mut self.segments[kind.slot()]
    }

    pub fn increment_version(&mut self) {
        self.version += 1;
    }

    pub fn all_legs(&self) -> Vec<Element> {
        self.segments.iter().flat_map(|s| s.legs.iter().cloned()).collect()
    }

    pub fn leg_count(&self) -> usize {
        self.segments.iter().map(|s| s.legs.len()).sum()
    }

    pub fn element_at(&self, index: usize) -> Option<&Element> {
        let (s, i) = self.segment_position(index)?;
        self.segments[s].legs.get(i)
    }

    pub fn leg_at(&self, index: usize) -> Option<&Leg> {
        self.element_at(index).and_then(|e| e.leg())
    }

    pub fn leg_at_mut(&mut self, index: usize) -> Option<&mut Leg> {
        let (s, i) = self.segment_position(index)?;
        self.segments[s].legs.get_mut(i).and_then(|e| e.leg_mut())
    }

    pub fn segment_position(&self, index: usize) -> Option<(usize, usize)> {
        let mut acc = 0;
        for (s, seg) in self.segments.iter().enumerate() {
            acc += seg.legs.len();
            if acc > index {
                return Some((s, index - (acc - seg.legs.len())));
            }
        }
        None
    }

    fn index_for_segment_position(&self, segment: usize, index_in_segment: usize) -> usize {
        self.segments[..segment].iter().map(|s| s.legs.len()).sum::<usize>() + index_in_segment
    }

    pub fn first_missed_approach_leg_index(&self) -> usize {
        self.leg_count() - self.seg(SegmentKind::MissedApproach).legs.len()
    }

    pub fn origin_leg_index(&self) -> Option<usize> {
        self.all_legs().iter().position(|e| e.leg().is_some_and(|l| l.flags & FLAG_ORIGIN != 0))
    }

    pub fn destination_leg_index(&self) -> Option<usize> {
        let fm = self.first_missed_approach_leg_index();
        (0..fm).rev().find(|i| {
            self.leg_at(*i).is_some_and(|l| {
                l.definition.approach_waypoint_descriptor == Some(APPROACH_DESCRIPTOR_MISSED_APPROACH_POINT)
                    || are_database_items_equal(l.termination_waypoint(), self.destination_airport.as_ref())
                    || are_database_items_equal(l.termination_waypoint(), self.destination_runway.as_ref().map(|r| &r.fix))
            })
        })
    }

    fn previous_segment(&self, before: usize) -> Option<usize> {
        let mut i = before as i64 - 1;
        while i > 0 && self.segments[i as usize].legs.is_empty() {
            i -= 1;
        }
        (i >= 0 && !self.segments[i as usize].legs.is_empty()).then_some(i as usize)
    }

    fn next_segment(&self, after: usize) -> Option<usize> {
        let mut i = after + 1;
        while i < self.segments.len() && self.segments[i].legs.is_empty() {
            i += 1;
        }
        (i < self.segments.len()).then_some(i)
    }

    pub fn find_last_departure_leg(&self) -> Option<(usize, usize, usize)> {
        let mut s = self.previous_segment(SegmentKind::Enroute.slot());
        while let Some(seg) = s {
            if let Some(last) = self.segments[seg].last_leg_index() {
                return Some((seg, last, self.index_for_segment_position(seg, last)));
            }
            s = self.previous_segment(seg);
        }
        None
    }

    pub fn find_first_arrival_leg(&self) -> Option<(usize, usize, usize)> {
        let seg = self.next_segment(SegmentKind::Enroute.slot())?;
        let first = if self.segments[seg].legs[0].is_disco() { 1 } else { 0 };
        Some((seg, first, self.index_for_segment_position(seg, first)))
    }

    fn find_duplicate(&self, waypoint: &Fix, after_index: i64) -> Option<(usize, usize, usize)> {
        let mut acc = 0i64;
        for (s, seg) in self.segments.iter().enumerate() {
            acc += seg.legs.len() as i64;
            if acc > after_index {
                let local_after = after_index - (acc - seg.legs.len() as i64);
                if let Some(d) = seg.find_index_of_waypoint(waypoint, local_after) {
                    let plan_index = acc - seg.legs.len() as i64 + d as i64;
                    if plan_index <= after_index {
                        continue;
                    }
                    return Some((s, d, plan_index as usize));
                }
            }
        }
        None
    }

    fn find_duplicate_reverse(&self, waypoint: &Fix, before_index: i64) -> Option<(usize, usize, usize)> {
        let mut acc = 0i64;
        let mut result = None;
        for (s, seg) in self.segments.iter().enumerate() {
            if acc >= before_index {
                break;
            }
            acc += seg.legs.len() as i64;
            let local_before = before_index - (acc - seg.legs.len() as i64);
            if let Some(d) = seg.find_last_index_of_waypoint(waypoint, local_before) {
                result = Some((s, d, (acc - seg.legs.len() as i64 + d as i64) as usize));
            }
        }
        result
    }

    pub fn remove_range(&mut self, start: usize, end: usize) {
        if end <= start {
            return;
        }
        let (Some((ss, si)), Some((es, ei))) = (self.segment_position(start), self.segment_position(end - 1)) else { return };
        let mut removed: Vec<Element> = Vec::new();
        if ss == es {
            removed.extend(self.segments[ss].legs.drain(si..=ei));
        } else {
            removed.extend(self.segments[ss].legs.drain(si..));
            for s in ss + 1..es {
                removed.append(&mut self.segments[s].legs);
            }
            removed.extend(self.segments[es].legs.drain(..=ei));
        }
        self.clear_engine_out_if_branch_removed(&removed);
    }

    pub(crate) fn clear_engine_out_if_branch_removed(&mut self, removed: &[Element]) {
        if removed.iter().any(|e| e.leg().is_some_and(|l| l.definition.is_engine_out_branch)) {
            let segment = &mut self.engine_out_departure;
            segment.legs.clear();
            segment.procedure_ident = None;
            segment.version += 1;
        }
    }

    pub fn redistribute_legs_at(&mut self, index: usize) -> i32 {
        let Some((seg, in_seg)) = self.segment_position(index) else { return 0 };
        self.segments[seg].strung = true;
        let enroute = SegmentKind::Enroute.slot();
        match self.segments[seg].kind.class() {
            SegmentClass::Departure => {
                let mut to_insert: Vec<Element> = Vec::new();
                let mut empty_all_next = false;
                for s in 0..enroute {
                    if s == seg {
                        empty_all_next = true;
                        to_insert.extend(self.segments[s].legs.drain(in_seg..));
                    } else if empty_all_next {
                        to_insert.extend(self.segments[s].legs.drain(..));
                    }
                }
                let enroute_legs = &mut self.segments[enroute].legs;
                for (i, e) in to_insert.into_iter().enumerate() {
                    enroute_legs.insert(i, e);
                }
                1
            }
            SegmentClass::Arrival => {
                let mut to_insert: Vec<Element> = Vec::new();
                let mut empty_all_next = false;
                for s in (enroute + 1..self.segments.len()).rev() {
                    if s == seg {
                        empty_all_next = true;
                        let removed: Vec<Element> = self.segments[s].legs.drain(..=in_seg).collect();
                        to_insert.splice(0..0, removed);
                    } else if empty_all_next {
                        let removed: Vec<Element> = self.segments[s].legs.drain(..).collect();
                        to_insert.splice(0..0, removed);
                    }
                }
                self.segments[enroute].legs.extend(to_insert);
                -1
            }
            SegmentClass::Enroute => 0,
        }
    }

    fn insert_element_after(&mut self, index: usize, element: Element, insert_discontinuity: bool) -> FmsResult<()> {
        if index > self.leg_count() {
            return Err(FmsError(format!("[FMS/FPM] Tried to insert waypoint out of bounds (index={index})")));
        }
        let enroute = SegmentKind::Enroute.slot();
        let (mut seg, mut in_seg) = if self.leg_count() > 0 { self.segment_position(index).unwrap_or((enroute, 0)) } else { (enroute, 0) };
        let last_dep = self.find_last_departure_leg().map(|t| t.2 as i64).unwrap_or(-1);
        let first_arr = self.find_first_arrival_leg().map(|t| t.2 as i64).unwrap_or(-1);

        if self.segments[seg].kind.class() == SegmentClass::Departure && index as i64 >= last_dep {
            if in_seg < self.segments[seg].legs.len() {
                self.redistribute_legs_at(index);
            }
            seg = enroute;
            in_seg = 0;
        } else if self.segments[seg].kind.class() == SegmentClass::Arrival && (index as i64) < first_arr {
            self.redistribute_legs_at(index.saturating_sub(1));
            seg = enroute;
            in_seg = self.segments[enroute].legs.len();
        }

        let approach = SegmentKind::Approach.slot();
        let prepend_in_missed = seg == approach && in_seg == self.segments[approach].legs.len().saturating_sub(1);
        if prepend_in_missed {
            seg = SegmentKind::MissedApproach.slot();
            in_seg = 0;
            self.segments[seg].legs.insert(in_seg, element);
        } else {
            let at = (in_seg + 1).min(self.segments[seg].legs.len());
            self.segments[seg].legs.insert(at, element);
        }

        if insert_discontinuity {
            self.increment_version();
            let after_inserted = self.all_legs().get(index + 2).cloned();
            if after_inserted.is_some_and(|e| !e.is_disco()) {
                let at = if prepend_in_missed { in_seg + 1 } else { in_seg + 2 };
                let at = at.min(self.segments[seg].legs.len());
                self.segments[seg].legs.insert(at, Element::Disco);
                self.increment_version();
            }
        }
        self.restring();
        self.increment_version();
        Ok(())
    }

    fn insert_element_before(&mut self, index: usize, element: Element, insert_discontinuity: bool) -> FmsResult<()> {
        if index < 1 || index >= self.leg_count() {
            return Err(FmsError(format!("[FMS/FPM] Tried to insert waypoint out of bounds (index={index})")));
        }
        let enroute = SegmentKind::Enroute.slot();
        let (mut seg, mut in_seg) = self.segment_position(index).unwrap_or((enroute, 0));
        let last_dep = self.find_last_departure_leg().map(|t| t.2 as i64).unwrap_or(-1);
        let first_arr = self.find_first_arrival_leg().map(|t| t.2 as i64).unwrap_or(-1);

        if self.segments[seg].kind.class() == SegmentClass::Departure && index as i64 > last_dep {
            self.redistribute_legs_at(index);
            seg = enroute;
            in_seg = 0;
        } else if self.segments[seg].kind.class() == SegmentClass::Arrival && index as i64 <= first_arr {
            if in_seg > 0 {
                self.redistribute_legs_at(index - 1);
            }
            seg = enroute;
            in_seg = self.segments[enroute].legs.len();
        }
        let at = in_seg.min(self.segments[seg].legs.len());
        self.segments[seg].legs.insert(at, element);

        if insert_discontinuity {
            self.increment_version();
            let after = self.all_legs().get(index + 1).cloned();
            if after.is_some_and(|e| !e.is_disco()) {
                let at = (in_seg + 1).min(self.segments[seg].legs.len());
                self.segments[seg].legs.insert(at, Element::Disco);
                self.increment_version();
            }
        }
        self.restring();
        self.increment_version();
        Ok(())
    }

    fn set_overfly_at(&mut self, index: usize, overfly: bool) {
        if let Some(l) = self.leg_at_mut(index) {
            l.definition.overfly = overfly;
        }
        self.increment_version();
    }

    pub(crate) fn requires_turn_direction_at(&self, index: usize) -> bool {
        self.leg_at(index).is_some_and(|l| l.requires_turn_direction())
    }

    pub(crate) fn remove_forced_turn_at(&mut self, index: usize) {
        if let Some(l) = self.leg_at_mut(index) {
            l.definition.turn_direction = None;
        }
    }

    pub fn next_waypoint(&mut self, index: usize, waypoint: &Fix) -> FmsResult<()> {
        if let Some((dup_seg, _, dup_plan_index)) = self.find_duplicate(waypoint, index as i64) {
            if dup_plan_index < self.first_missed_approach_leg_index() {
                let dup = self.leg_at(dup_plan_index).cloned().ok_or_else(|| FmsError("duplicate leg vanished".into()))?;
                let _ = dup_seg;
                let mut leg = Leg::from_enroute_fix(waypoint, None, LegType::TF);
                leg.definition = LegDefinition { leg_type: LegType::TF, ..dup.definition.clone() };
                leg.pilot_altitude = dup.pilot_altitude.clone();
                leg.pilot_speed = dup.pilot_speed.clone();
                leg.ident = dup.ident.clone();
                leg.annotation = dup.annotation.clone();
                if self.leg_at(index).is_some() && !self.requires_turn_direction_at(dup_plan_index + 1) {
                    self.set_overfly_at(index, false);
                    self.remove_forced_turn_at(dup_plan_index + 1);
                }
                self.remove_range(index + 1, dup_plan_index + 1);
                return self.insert_element_after(index, Element::new_leg(leg), false);
            }
        }
        let after_is_disco = self.element_at(index).ok_or_else(|| FmsError(format!("[FMS/FPM] no element at {index}")))?.is_disco();
        let leg = Leg::from_enroute_fix(waypoint, None, if after_is_disco { LegType::IF } else { LegType::TF });
        self.insert_element_after(index, Element::new_leg(leg), true)
    }

    pub fn insert_waypoint_before(&mut self, index: usize, waypoint: &Fix) -> FmsResult<()> {
        let mut index = index;
        if let Some((_, _, dup_plan_index)) = self.find_duplicate(waypoint, index as i64) {
            if dup_plan_index < self.first_missed_approach_leg_index() {
                let dup = self.leg_at(dup_plan_index).cloned().ok_or_else(|| FmsError("duplicate leg vanished".into()))?;
                let mut leg = Leg::from_enroute_fix(waypoint, None, LegType::TF);
                leg.definition = LegDefinition { leg_type: LegType::TF, ..dup.definition.clone() };
                leg.pilot_altitude = dup.pilot_altitude.clone();
                leg.pilot_speed = dup.pilot_speed.clone();
                leg.ident = dup.ident.clone();
                leg.annotation = dup.annotation.clone();
                if !self.requires_turn_direction_at(dup_plan_index + 1) {
                    if index >= 1 && self.leg_at(index - 1).is_some() {
                        self.set_overfly_at(index - 1, false);
                    }
                    self.remove_forced_turn_at(dup_plan_index + 1);
                }
                self.remove_range(index, dup_plan_index + 1);
                return self.insert_element_before(index, Element::new_leg(leg), false);
            }
        }
        if index >= 1 {
            if let Some(prev) = self.leg_at(index - 1).cloned() {
                self.set_overfly_at(index - 1, false);
                if prev.is_xi() {
                    self.remove_element_at(index - 1, false);
                    index -= 1;
                }
            }
        }
        let leg = Leg::from_enroute_fix(waypoint, None, LegType::TF);
        self.insert_element_before(index, Element::new_leg(leg), true)
    }

    fn merge_constraints(first: &mut Leg, second: &Leg) {
        let only_first_pilot_speed = first.pilot_speed.is_some() && second.pilot_speed.is_none();
        if !only_first_pilot_speed {
            first.pilot_speed = second.pilot_speed.clone();
        }
        let second_has_speed = second.definition.speed_descriptor.is_some() || second.pilot_speed.is_some();
        let only_first_db_speed = first.definition.speed_descriptor.is_some() && !second_has_speed;
        if !only_first_db_speed {
            first.definition.speed_descriptor = second.definition.speed_descriptor.clone();
            first.definition.speed = second.definition.speed;
        }
        let only_first_pilot_alt = first.pilot_altitude.is_some() && second.pilot_altitude.is_none();
        if !only_first_pilot_alt {
            first.pilot_altitude = second.pilot_altitude.clone();
        }
        let second_has_alt = second.has_database_altitude_constraint() || second.pilot_altitude.is_some();
        let only_first_db_alt = first.has_database_altitude_constraint() && !second_has_alt;
        if !only_first_db_alt {
            first.definition.altitude_descriptor = second.definition.altitude_descriptor.clone();
            first.definition.altitude1 = second.definition.altitude1;
            first.definition.altitude2 = second.definition.altitude2;
        }
    }

    pub fn remove_element_at(&mut self, index: usize, insert_discontinuity: bool) -> bool {
        let Some((seg, in_seg)) = self.segment_position(index) else { return false };
        if index > 0 {
            let prev = self.element_at(index - 1).cloned().unwrap_or(Element::Disco);
            let element = self.element_at(index).cloned().unwrap_or(Element::Disco);
            let (prev_seg, prev_in_seg) = self.segment_position(index - 1).unwrap_or((seg, 0));
            let next = self.element_at(index + 1).cloned();
            let n_delete = if next.as_ref().and_then(|e| e.leg()).is_some_and(|l| l.is_hx()) { 2 } else { 1 };

            if let Element::Leg(prev_leg) = &prev {
                if !element.is_disco() {
                    self.remove_range(index, index + n_delete);
                    if insert_discontinuity {
                        let at = in_seg.min(self.segments[seg].legs.len());
                        self.segments[seg].legs.insert(at, Element::Disco);
                    }
                    if prev_leg.is_xi() {
                        self.segments[prev_seg].legs.remove(prev_in_seg);
                    }
                } else if let Some(Element::Leg(next_leg)) = &next {
                    let same = next_leg.termination_waypoint().is_some_and(|w| prev_leg.terminates_with_waypoint(w));
                    if same {
                        let next_leg = (**next_leg).clone();
                        if let Some(p) = self.leg_at_mut(index - 1) {
                            Self::merge_constraints(p, &next_leg);
                        }
                        self.remove_range(index, index + 2);
                    } else {
                        if next_leg.is_xf() {
                            let (ns, ni) = self.segment_position(index + 1).unwrap_or((seg, 0));
                            if let Some(wp) = next_leg.termination_waypoint() {
                                let mut replacement = Leg::from_enroute_fix(wp, Some(&next_leg.annotation), LegType::TF);
                                replacement.definition = LegDefinition { leg_type: next_leg.definition.leg_type, ..next_leg.definition.clone() };
                                replacement.definition.leg_type = next_leg.definition.leg_type;
                                replacement.ident = next_leg.ident.clone();
                                replacement.pilot_altitude = next_leg.pilot_altitude.clone();
                                replacement.pilot_speed = next_leg.pilot_speed.clone();
                                self.segments[ns].legs[ni] = Element::new_leg(replacement);
                            }
                        }
                        self.segments[seg].legs.remove(in_seg);
                    }
                    if !self.requires_turn_direction_at(index + 2) {
                        self.remove_forced_turn_at(index + 2);
                    }
                }
            } else {
                self.remove_range(index, index + n_delete);
            }
        } else {
            self.segments[seg].legs.remove(in_seg);
        }
        self.increment_version();
        self.adjust_if_legs();
        self.adjust_tf_legs();
        self.ensure_no_duplicate_discontinuities();
        self.increment_version();
        true
    }

    pub fn insert_discontinuity_after(&mut self, index: usize) {
        let Some((seg, in_seg)) = self.segment_position(index) else { return };
        if self.segments[seg].legs[in_seg].is_disco() {
            return;
        }
        self.segments[seg].legs.insert(in_seg + 1, Element::Disco);
        self.increment_version();
        self.adjust_if_legs();
        self.increment_version();
    }

    pub fn set_pilot_altitude_constraint_at(&mut self, index: usize, is_descent: bool, constraint: Option<AltitudeConstraint>) {
        let Some(leg) = self.leg_at_mut(index) else { return };
        leg.pilot_altitude = constraint.clone();
        if constraint.is_none() {
            leg.clear_altitude_constraints();
        }
        if is_descent {
            self.set_first_des_constraint_waypoint(index);
        } else {
            self.set_last_clb_constraint_waypoint(index);
        }
        self.increment_version();
    }

    pub fn set_pilot_speed_constraint_at(&mut self, index: usize, is_descent: bool, speed: Option<f64>) {
        let Some(leg) = self.leg_at_mut(index) else { return };
        match speed {
            Some(s) if s != 0. => leg.pilot_speed = Some(SpeedConstraint { descriptor: "Maximum".into(), speed: s }),
            _ => leg.clear_speed_constraints(),
        }
        if is_descent {
            self.set_first_des_constraint_waypoint(index);
        } else {
            self.set_last_clb_constraint_waypoint(index);
        }
        self.increment_version();
    }

    fn set_last_clb_constraint_waypoint(&mut self, index: usize) {
        let floor = if index >= self.first_missed_approach_leg_index() { self.first_missed_approach_leg_index() } else { 0 };
        for i in (floor..=index).rev() {
            if let Some(l) = self.leg_at_mut(i) {
                l.constraint_type = CONSTRAINT_CLB;
            }
        }
    }

    fn set_first_des_constraint_waypoint(&mut self, index: usize) {
        for i in index..self.first_missed_approach_leg_index() {
            if let Some(l) = self.leg_at_mut(i) {
                l.constraint_type = CONSTRAINT_DES;
            }
        }
    }

    pub fn add_or_update_cruise_step(&mut self, index: usize, to_altitude: f64) {
        if let Some(l) = self.leg_at_mut(index) {
            l.cruise_step = Some(CruiseStep { distance_before_termination: 0., to_altitude, waypoint_index: index, is_ignored: false });
        }
        for e in self.segments.iter_mut().flat_map(|s| s.legs.iter_mut()) {
            if let Some(step) = e.leg_mut().and_then(|l| l.cruise_step.as_mut()) {
                step.is_ignored = false;
            }
        }
        self.increment_version();
    }

    pub fn remove_cruise_step(&mut self, index: usize) {
        if let Some(l) = self.leg_at_mut(index) {
            l.cruise_step = None;
        }
        self.increment_version();
    }

    fn adjust_if_legs(&mut self) {
        let elements = self.all_legs();
        let mut changes: Vec<(usize, LegType)> = Vec::new();
        for i in 0..elements.len() {
            let Some(leg) = elements[i].leg() else { continue };
            let prev = if i > 0 { elements.get(i - 1) } else { None };
            let mut current = leg.leg_type;
            if i != 0 && current == LegType::IF {
                let prev_blocks = match prev {
                    Some(Element::Disco) => true,
                    Some(Element::Leg(p)) => p.is_xi(),
                    None => false,
                };
                if !prev_blocks {
                    current = if leg.definition.leg_type == LegType::IF && leg.ident != "T-P" { LegType::TF } else { leg.definition.leg_type };
                    changes.push((i, current));
                }
            }
            if current != LegType::IF {
                let turning_point = leg.flags & FLAG_DIRECT_TO_TURNING_POINT != 0;
                if !turning_point && (prev.is_none_or(|p| p.is_disco()) || i == 0) {
                    changes.push((i, LegType::IF));
                }
            }
        }
        for (i, t) in changes {
            if let Some(l) = self.leg_at_mut(i) {
                l.leg_type = t;
            }
        }
    }

    fn adjust_tf_legs(&mut self) {
        let elements = self.all_legs();
        for i in 1..elements.len() {
            let (Some(leg), Some(prev)) = (elements[i].leg(), elements[i - 1].leg()) else { continue };
            if leg.leg_type == LegType::TF && !(prev.is_xf() || prev.is_hx()) {
                if let Some(l) = self.leg_at_mut(i) {
                    l.leg_type = LegType::DF;
                }
            }
        }
    }

    fn ensure_no_duplicate_discontinuities(&mut self) {
        let mut consecutive = 1usize;
        for seg in self.segments.iter_mut() {
            let mut to_delete: Vec<(usize, usize)> = Vec::new();
            for j in 0..seg.legs.len() {
                if seg.legs[j].is_disco() {
                    consecutive += 1;
                } else {
                    if consecutive > 1 {
                        to_delete.push((j + 1 - consecutive, consecutive - 1));
                    }
                    consecutive = 0;
                }
            }
            if consecutive > 1 {
                to_delete.push((seg.legs.len() + 1 - consecutive, consecutive - 1));
                consecutive = 1;
            }
            for (start, length) in to_delete.into_iter().rev() {
                seg.legs.drain(start..start + length);
            }
        }
    }

    fn ensure_no_discontinuity_as_final_element(&mut self) {
        for seg in self.segments.iter_mut().rev() {
            if seg.legs.is_empty() {
                continue;
            }
            let mut n = 0;
            while seg.legs.last().is_some_and(|e| e.is_disco()) && n < 10 {
                seg.legs.pop();
                n += 1;
            }
            break;
        }
    }

    fn string_departure_to_downstream(&mut self) {
        let Some((seg, last_idx, last_plan_idx)) = self.find_last_departure_leg() else { return };
        if self.segments[seg].strung {
            return;
        }
        let Some(last_leg) = self.segments[seg].legs[last_idx].leg().cloned() else { return };
        if last_leg.is_xf() {
            if let Some(wp) = last_leg.termination_waypoint() {
                if let Some((_, _, dup_plan_index)) = self.find_duplicate(wp, last_plan_idx as i64) {
                    let origin_and_destination = Some(last_plan_idx) == self.origin_leg_index() && Some(dup_plan_index) == self.destination_leg_index();
                    if !origin_and_destination {
                        self.remove_range(last_plan_idx + 1, dup_plan_index + 1);
                        self.segments[seg].strung = true;
                        return;
                    }
                }
            }
        }
        self.increment_version();
        if self.all_legs().get(last_plan_idx + 1).is_some_and(|e| !e.is_disco()) {
            self.seg_mut(SegmentKind::Enroute).legs.insert(0, Element::Disco);
        }
    }

    fn string_arrival_to_upstream(&mut self) {
        let Some((_, first_idx, first_plan_idx)) = self.find_first_arrival_leg() else { return };
        let (seg, _, _) = self.find_first_arrival_leg().unwrap_or((0, 0, 0));
        let Some(first_leg) = self.segments[seg].legs.get(first_idx).and_then(|e| e.leg()).cloned() else { return };
        if first_leg.is_xf() {
            if let Some(wp) = first_leg.termination_waypoint() {
                if let Some((dup_seg, dup_in_seg, dup_plan_index)) = self.find_duplicate_reverse(wp, first_plan_idx as i64) {
                    let before_active = (dup_plan_index as i64) < self.active_leg_index;
                    let origin_and_destination = Some(dup_plan_index) == self.origin_leg_index() && Some(first_plan_idx) == self.destination_leg_index();
                    if !before_active && !origin_and_destination {
                        if let Some(dup) = self.segments[dup_seg].legs[dup_in_seg].leg_mut() {
                            dup.definition = first_leg.definition.clone();
                        }
                        self.remove_range(dup_plan_index + 1, first_plan_idx + 1);
                        self.segments[dup_seg].strung = true;
                        return;
                    }
                }
            }
        }
        if first_plan_idx == 0 {
            return;
        }
        let upstream_is_leg = self.element_at(first_plan_idx - 1).is_some_and(|e| !e.is_disco());
        if upstream_is_leg {
            if let Some(l) = self.leg_at_mut(first_plan_idx - 1) {
                l.clear_constraints();
            }
            self.seg_mut(SegmentKind::Enroute).legs.push(Element::Disco);
        }
    }

    pub fn restring(&mut self) {
        self.restring_with(RESTRING_DEFAULT);
    }

    pub fn enqueue_operation(&mut self, op: QueuedOp) {
        if self.queued_operations.contains(&op) {
            if matches!(op, QueuedOp::Restring(_)) {
                self.queued_operations.retain(|o| *o != op);
            } else {
                return;
            }
        }
        self.queued_operations.push(op);
    }

    pub fn flush_operation_queue(&mut self, db: &dyn NavDatabase) -> FmsResult<()> {
        let mut i = 0;
        while i < self.queued_operations.len() {
            match self.queued_operations[i] {
                QueuedOp::Restring(options) => self.restring_with(options),
                QueuedOp::RebuildArrivalAndApproach => self.rebuild_arrival_and_approach_segments(db)?,
            }
            i += 1;
        }
        self.queued_operations.clear();
        Ok(())
    }

    fn next_segment_after(&self, after: usize) -> Option<usize> {
        let mut i = after + 1;
        if i >= self.segments.len() {
            return None;
        }
        while i < self.segments.len() && self.segments[i].legs.is_empty() {
            i += 1;
        }
        (i < self.segments.len()).then_some(i)
    }

    fn string_segments_forwards(&mut self, first: usize, second: usize) {
        if self.segments[first].strung
            || self.segments[first].legs.is_empty()
            || self.segments[second].legs.is_empty()
            || (self.segments[second].kind == SegmentKind::Enroute && self.segments[second].is_sequenced_missed_approach)
        {
            return;
        }
        let first_kind = self.segments[first].kind;
        let second_kind = self.segments[second].kind;

        let mut last_in_first_index = self.segments[first].legs.len() - 1;
        let last_in_first_is_disco = self.segments[first].legs[last_in_first_index].is_disco();
        if last_in_first_is_disco {
            if self.segments[first].legs.len() < 2 {
                return;
            }
            last_in_first_index = self.segments[first].legs.len() - 2;
        }
        let Some(last_leg_in_first) = self.segments[first].legs[last_in_first_index].leg().cloned() else { return };

        if first_kind == SegmentKind::Origin {
            let last = self.segments[first].last_leg_index().and_then(|i| self.segments[first].legs[i].leg());
            if last.is_some_and(|l| l.definition.waypoint_descriptor == Some(DESCRIPTOR_RUNWAY)) {
                self.segments[first].strung = true;
                return;
            }
        }

        let first_is_arrival = matches!(first_kind, SegmentKind::Arrival | SegmentKind::ArrivalEnrouteTransition | SegmentKind::ArrivalRunwayTransition);
        let second_is_approach = matches!(second_kind, SegmentKind::Approach | SegmentKind::ApproachVia);
        if first_is_arrival && second_is_approach {
            return;
        }
        if first_kind == SegmentKind::Approach && second_kind == SegmentKind::Destination {
            self.segments[first].strung = true;
            return;
        }
        if matches!(first_kind, SegmentKind::Destination | SegmentKind::Approach) && second_kind == SegmentKind::MissedApproach {
            self.segments[first].strung = true;
            return;
        }

        let origin_and_destination = first_kind == SegmentKind::Origin && second_kind == SegmentKind::Destination;
        let total_index = self.index_for_segment_position(first, last_in_first_index) as i64;
        let first_is_before_active_leg = self.active_leg_index > -1 && total_index < self.active_leg_index;

        let mut cut_before: i64 = -1;
        if !origin_and_destination && !first_is_before_active_leg {
            let mut i = 0;
            while i < self.segments[second].legs.len() {
                let Some(element) = self.segments[second].legs[i].leg().cloned() else {
                    i += 1;
                    continue;
                };
                if last_leg_in_first.is_xf() && element.is_xf() && last_leg_in_first.termination_waypoint().is_some_and(|w| element.terminates_with_waypoint(w)) {
                    let mut replaced = element.clone();
                    replaced.leg_type = last_leg_in_first.definition.leg_type;
                    replaced.definition.assign_from(&last_leg_in_first.definition);
                    let (ident, annotation) = procedure_leg_ident_and_annotation(&replaced.definition, &last_leg_in_first.annotation);
                    replaced.ident = ident;
                    replaced.annotation = annotation;
                    Self::merge_constraints(&mut replaced, &last_leg_in_first);
                    self.segments[second].legs[i] = Element::new_leg(replaced);
                    self.segments[first].legs.pop();
                    cut_before = i as i64;
                    break;
                }
                if last_leg_in_first.is_xf() && element.is_fx() && element.termination_waypoint().is_some_and(|w| last_leg_in_first.terminates_with_waypoint(w)) {
                    cut_before = i as i64;
                    break;
                }
                if last_leg_in_first.is_xi() && element.is_xf() {
                    let same_waypoint = are_database_items_equal(last_leg_in_first.definition.waypoint.as_ref(), element.definition.waypoint.as_ref());
                    if last_leg_in_first.definition.leg_type != LegType::PI || same_waypoint {
                        if element.leg_type == LegType::TF {
                            let prev_wp = if i > 0 { self.segments[second].legs[i - 1].leg().and_then(|l| l.termination_waypoint().cloned()) } else { None };
                            if let (Some(prev), Some(own)) = (prev_wp, element.termination_waypoint().cloned()) {
                                let track = prev.location.coordinates().bearing_to(&own.location.coordinates());
                                if let Some(l) = self.segments[second].legs[i].leg_mut() {
                                    l.leg_type = LegType::CF;
                                    l.definition.course = Some(match l.definition.mag_var {
                                        Some(m) => (track - m).rem_euclid(360.),
                                        None => track,
                                    });
                                    let (ident, annotation) = procedure_leg_ident_and_annotation(&l.definition, &l.annotation);
                                    l.ident = ident;
                                    l.annotation = annotation;
                                }
                            }
                        }
                        cut_before = i as i64;
                        break;
                    }
                }
                i += 1;
            }
        }

        if cut_before == -1 || origin_and_destination {
            let last_is_leg = !self.segments[first].legs.last().is_some_and(|e| e.is_disco());
            if last_is_leg && self.segments[second].legs.first().is_some_and(|e| !e.is_disco()) {
                self.segments[first].legs.push(Element::Disco);
            }
            self.segments[first].strung = false;
            return;
        }

        if last_in_first_is_disco {
            self.segments[first].legs.pop();
        }
        for _ in 0..cut_before {
            self.segments[second].legs.remove(0);
        }
        self.segments[first].strung = true;
    }

    fn string_segments_backwards(&mut self, first: usize, second: usize) {
        if self.segments[first].legs.is_empty() || self.segments[second].legs.is_empty() {
            return;
        }
        let first_is_arrival = matches!(self.segments[first].kind, SegmentKind::Arrival | SegmentKind::ArrivalEnrouteTransition | SegmentKind::ArrivalRunwayTransition);
        let second_is_approach = matches!(self.segments[second].kind, SegmentKind::Approach | SegmentKind::ApproachVia);
        if !first_is_arrival || !second_is_approach {
            return;
        }
        let first_element_index = 0usize;
        let Some(first_leg_in_second) = self.segments[second].legs[first_element_index].leg().cloned() else { return };

        let mut cut_before: i64 = -1;
        let mut i = self.segments[first].legs.len() as i64 - 1;
        while i >= 0 {
            let total_index = self.index_for_segment_position(first, first_element_index) as i64;
            if self.active_leg_index > -1 && total_index < self.active_leg_index {
                break;
            }
            let Some(element) = self.segments[first].legs[i as usize].leg().cloned() else {
                i -= 1;
                continue;
            };
            if first_leg_in_second.is_xf() && element.is_xf() && first_leg_in_second.termination_waypoint().is_some_and(|w| element.terminates_with_waypoint(w)) {
                if let Some(e) = self.segments[first].legs[i as usize].leg_mut() {
                    Self::merge_constraints(e, &first_leg_in_second);
                }
                self.segments[second].legs.remove(0);
                cut_before = i;
                break;
            }
            if element.is_xf() && first_leg_in_second.is_fx() && first_leg_in_second.termination_waypoint().is_some_and(|w| element.terminates_with_waypoint(w)) {
                cut_before = i;
                break;
            }
            i -= 1;
        }

        if cut_before == -1 {
            let last_is_leg = self.segments[first].legs.last().is_some_and(|e| !e.is_disco());
            if last_is_leg && self.segments[second].legs.first().is_some_and(|e| !e.is_disco()) {
                self.segments[first].legs.push(Element::Disco);
            }
            return;
        }
        while self.segments[first].legs.len() as i64 - 1 > cut_before {
            self.segments[first].legs.pop();
        }
    }

    pub fn restring_with(&mut self, options: u32) {
        let departure_segments: Vec<usize> = (0..self.segments.len()).filter(|i| self.segments[*i].kind.class() == SegmentClass::Departure).collect();
        let arrival_segments: Vec<usize> = (0..self.segments.len()).filter(|i| self.segments[*i].kind.class() == SegmentClass::Arrival).collect();

        if options & RESTRING_DEPARTURE != 0 {
            for &s in &departure_segments {
                if let Some(next) = self.next_segment_after(s) {
                    if self.segments[next].kind.class() == SegmentClass::Departure {
                        self.string_segments_forwards(s, next);
                    }
                }
                self.segments[s].insert_necessary_discontinuities();
            }
            self.string_departure_to_downstream();
        }

        if options & RESTRING_ARRIVAL != 0 {
            for &s in &arrival_segments {
                if let Some(next) = self.next_segment_after(s) {
                    if self.segments[next].kind.class() == SegmentClass::Arrival {
                        self.string_segments_forwards(s, next);
                    }
                }
                self.segments[s].insert_necessary_discontinuities();
            }
            for &s in &arrival_segments {
                if let Some(next) = self.next_segment_after(s) {
                    if self.segments[next].kind.class() == SegmentClass::Arrival {
                        self.string_segments_backwards(s, next);
                    }
                }
                self.segments[s].insert_necessary_discontinuities();
            }
            self.string_arrival_to_upstream();
        }
        self.ensure_no_duplicate_discontinuities();
        self.ensure_no_discontinuity_as_final_element();
        self.increment_version();
        self.adjust_if_legs();
        self.adjust_tf_legs();
        self.increment_version();
        self.select_active_leg();
    }

    fn select_active_leg(&mut self) {
        if self.active_leg_index == -1 {
            return;
        }
        if let Some((_, _, idx)) = self.find_first_arrival_leg() {
            if self.active_leg_index > idx as i64 {
                self.active_leg_index = idx as i64;
            }
        }
    }

    pub fn deduplicate_downstream_at(&mut self, at_index: usize, keep_upstream: bool) {
        let Some(leg) = self.leg_at(at_index).cloned() else { return };
        let Some(wp) = leg.termination_waypoint().cloned() else { return };
        if let Some((_, _, plan_index)) = self.find_duplicate(&wp, at_index as i64) {
            if keep_upstream {
                self.remove_range(at_index + 1, plan_index + 1);
            } else {
                self.remove_range(at_index, plan_index);
            }
            self.increment_version();
        }
    }

    pub fn start_airway_entry(&mut self, revised_leg_index: usize) -> FmsResult<()> {
        let leg = self.leg_at(revised_leg_index).ok_or_else(|| FmsError("Cannot start airway entry at a discontinuity".into()))?;
        if !leg.is_xf() && !leg.is_hx() {
            return Err(FmsError("Cannot create a pending airways entry from a non XF or HX leg".into()));
        }
        let wp = leg.termination_waypoint().cloned().ok_or_else(|| FmsError("revised leg has no waypoint".into()))?;
        self.pending_airways = Some(PendingAirways { revised_leg_index, revised_waypoint: wp, elements: Vec::new(), legs: Vec::new() });
        Ok(())
    }

    fn fix_index_along(airway: &Airway, fix: &Fix) -> Option<usize> {
        airway.fixes.iter().position(|f| f.ident == fix.ident && f.icao_code == fix.icao_code)
    }

    fn slice_airway(airway: &Airway, from: usize, to: usize) -> Vec<Fix> {
        let reversed = from > to;
        let fixes: Vec<Fix> = if reversed { airway.fixes.iter().rev().cloned().collect() } else { airway.fixes.clone() };
        let (start, end) = if reversed { (fixes.len() - from, fixes.len() - to) } else { (from + 1, to + 1) };
        fixes[start.min(fixes.len())..end.min(fixes.len())].to_vec()
    }

    pub fn continue_airway_entry_via_airway(&mut self, airway: Airway) -> FmsResult<bool> {
        let pending = self.pending_airways.as_mut().ok_or_else(|| FmsError("[BaseFlightPlan](continueAirwayEntryViaAirway) No airway entry is pending".into()))?;
        let tail = pending.elements.last().cloned();
        let start_index;
        match &tail {
            Some(t) if t.to.is_none() => {
                let tail_airway = t.airway.clone().ok_or_else(|| FmsError("tail without airway".into()))?;
                let mut found = None;
                for (i, f) in tail_airway.fixes.iter().enumerate() {
                    if let Some(m) = Self::fix_index_along(&airway, f) {
                        found = Some((i, m));
                        break;
                    }
                }
                let Some((idx_tail, idx_entered)) = found else { return Ok(false) };
                let entered_fix = airway.fixes[idx_entered].clone();
                let from_index = t.from_index.unwrap_or(0);
                let n = pending.elements.len();
                pending.elements[n - 1].to = Some(entered_fix);
                pending.elements[n - 1].is_auto_connected = true;
                for f in Self::slice_airway(&tail_airway, from_index, idx_tail) {
                    pending.legs.push(Leg::from_enroute_fix(&f, Some(&tail_airway.ident), LegType::TF));
                }
                start_index = idx_entered;
            }
            _ => {
                let from = tail.as_ref().and_then(|t| t.to.clone()).unwrap_or_else(|| pending.revised_waypoint.clone());
                match Self::fix_index_along(&airway, &from) {
                    Some(i) => start_index = i,
                    None => return Ok(false),
                }
            }
        }
        pending.elements.push(PendingAirwayEntry { from_index: Some(start_index), airway: Some(airway), to: None, is_dct: false, is_auto_connected: false });
        self.increment_version();
        Ok(true)
    }

    pub fn continue_airway_entry_to_fix(&mut self, fix: &Fix, is_dct: bool) -> FmsResult<bool> {
        let pending = self.pending_airways.as_mut().ok_or_else(|| FmsError("[BaseFlightPlan](continueAirwayEntryDirectToFix) No airway entry is pending".into()))?;
        let tail = pending.elements.last().cloned();
        if is_dct || tail.as_ref().is_some_and(|t| t.to.is_some()) {
            pending.elements.push(PendingAirwayEntry { from_index: None, airway: None, to: Some(fix.clone()), is_dct: true, is_auto_connected: false });
            pending.legs.push(Leg::from_enroute_fix(fix, None, LegType::TF));
            return Ok(true);
        }
        let tail = tail.ok_or_else(|| FmsError("no tail element".into()))?;
        let tail_airway = tail.airway.clone().ok_or_else(|| FmsError("tail without airway".into()))?;
        let Some(end) = Self::fix_index_along(&tail_airway, fix) else { return Ok(false) };
        for f in Self::slice_airway(&tail_airway, tail.from_index.unwrap_or(0), end) {
            pending.legs.push(Leg::from_enroute_fix(&f, Some(&tail_airway.ident), LegType::TF));
        }
        let n = pending.elements.len();
        pending.elements[n - 1].to = Some(fix.clone());
        self.increment_version();
        Ok(true)
    }

    pub fn finalise_airway_entry(&mut self) -> FmsResult<()> {
        let pending = self.pending_airways.take().ok_or_else(|| FmsError("[BaseFlightPlan](finaliseAirwayEntry) No airway entry is pending".into()))?;
        self.redistribute_legs_at(pending.revised_leg_index);
        let (seg, in_seg) = self.segment_position(pending.revised_leg_index).ok_or_else(|| FmsError("revised leg out of range".into()))?;
        if self.segments[seg].kind != SegmentKind::Enroute {
            return Err(FmsError("Finalizing pending airways into a segment that isn't enroute is not yet supported".into()));
        }
        let count = pending.legs.len();
        let at = (in_seg + 1).min(self.segments[seg].legs.len());
        for (i, leg) in pending.legs.into_iter().enumerate() {
            self.segments[seg].legs.insert(at + i, Element::new_leg(leg));
        }
        self.increment_version();
        self.deduplicate_downstream_at(pending.revised_leg_index + count, false);
        self.restring();
        Ok(())
    }

    pub fn clear_pending_airways(&mut self) {
        self.pending_airways = None;
    }

    pub fn enroute_leg_count(&self) -> usize {
        self.seg(SegmentKind::Enroute).legs.len()
    }

    pub fn insert_head_at_end_of_enroute(&self) -> i64 {
        let n: usize = [SegmentKind::Origin, SegmentKind::DepartureRunwayTransition, SegmentKind::Departure, SegmentKind::DepartureEnrouteTransition, SegmentKind::Enroute]
            .iter()
            .map(|k| self.seg(*k).legs.len())
            .sum();
        let mut head = n as i64 - 1;
        if self.enroute_leg_count() > 1 && head >= 0 && self.element_at(head as usize).is_some_and(|e| e.is_disco()) {
            head -= 1;
        }
        head
    }

    pub fn true_track_at(&self, index: usize) -> Option<f64> {
        let leg = self.leg_at(index)?;
        let here = leg.location()?;
        let prev = (0..index).rev().find_map(|i| self.leg_at(i).and_then(|l| l.location()))?;
        Some(prev.bearing_to(&here))
    }
}

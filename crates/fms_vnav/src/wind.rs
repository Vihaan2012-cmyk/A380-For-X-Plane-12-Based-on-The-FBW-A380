use crate::atmosphere::interpolate;
use crate::profile::Winds;
use std::cell::Cell;
use std::f64::consts::PI;

const DEG: f64 = PI / 180.0;
const MAX_WIND_BLEND_ALTITUDE: f64 = 10_000.0;
const MAX_CRUISE_WIND_BLEND_DISTANCE: f64 = 200.0;
const TRIP_WIND_MAX_ALTITUDE: f64 = 20_000.0;

fn clamp(v: f64, lower: f64, upper: f64) -> f64 {
    v.max(lower).min(upper)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WindVector {
    pub magnitude: Option<f64>,
    pub direction: Option<f64>,
}

impl WindVector {
    pub const EMPTY: WindVector = WindVector { magnitude: Some(0.0), direction: Some(0.0) };

    pub fn new(magnitude: f64, direction_rad: f64) -> Self {
        WindVector { magnitude: Some(magnitude), direction: Some(direction_rad) }
    }

    pub fn from_degrees(magnitude: f64, direction_deg: f64) -> Self {
        Self::new(magnitude, direction_deg * DEG)
    }

    pub fn is_complete(&self) -> bool {
        self.magnitude.is_some() && self.direction.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindEntry {
    pub vector: WindVector,
    pub altitude: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
pub enum Forecast {
    Vector(WindVector),
    Tailwind(f64),
}

/// Deliberate differences from FBW, all off in `WindFixes::FBW`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindFixes {
    /// A380 climb winds are stored highest first; FBW takes entry 0 as the lowest.
    pub climb_lowest_entry: bool,
    /// `scaleWindVector` scales the direction as well as the magnitude.
    pub scale_magnitude_only: bool,
    /// Wind directions are interpolated linearly in radians, the long way round across north.
    pub shortest_arc_direction: bool,
}

impl WindFixes {
    pub const FBW: WindFixes = WindFixes { climb_lowest_entry: false, scale_magnitude_only: false, shortest_arc_direction: false };
    pub const ALL: WindFixes = WindFixes { climb_lowest_entry: true, scale_magnitude_only: true, shortest_arc_direction: true };
}

pub fn scale_wind_vector(v: WindVector, scale: f64, fixes: WindFixes) -> WindVector {
    WindVector {
        direction: if fixes.scale_magnitude_only { v.direction } else { v.direction.map(|d| d * scale) },
        magnitude: v.magnitude.map(|m| m * scale),
    }
}

pub fn interpolate_wind_vector(x: f64, x0: f64, x1: f64, v0: WindVector, v1: WindVector, fixes: WindFixes) -> WindVector {
    let direction = match (v0.direction, v1.direction) {
        (Some(a), Some(mut b)) => {
            if fixes.shortest_arc_direction {
                while b - a > PI {
                    b -= 2.0 * PI;
                }
                while a - b > PI {
                    b += 2.0 * PI;
                }
                Some(interpolate(x, x0, x1, a, b).rem_euclid(2.0 * PI))
            } else {
                Some(interpolate(x, x0, x1, a, b))
            }
        }
        _ => None,
    };
    let magnitude = match (v0.magnitude, v1.magnitude) {
        (Some(a), Some(b)) => Some(interpolate(x, x0, x1, a, b)),
        _ => None,
    };
    WindVector { direction, magnitude }
}

pub fn interpolate_wind_entries(entries: &[WindEntry], altitude: f64, fixes: WindFixes) -> WindVector {
    match entries.len() {
        0 => return WindVector::EMPTY,
        1 => return entries[0].vector,
        _ => {}
    }
    let descending = entries[1].altitude.unwrap_or(0.0) < entries[0].altitude.unwrap_or(0.0);
    let (lowest, highest) = if descending { (entries[entries.len() - 1], entries[0]) } else { (entries[0], entries[entries.len() - 1]) };
    if lowest.altitude.is_some_and(|a| altitude <= a) {
        return lowest.vector;
    }
    if highest.altitude.is_some_and(|a| altitude >= a) {
        return highest.vector;
    }
    for i in 0..entries.len() - 1 {
        let (lower, upper) = if descending { (entries[i + 1], entries[i]) } else { (entries[i], entries[i + 1]) };
        if let (Some(la), Some(ua)) = (lower.altitude, upper.altitude) {
            if la <= altitude && altitude <= ua {
                return interpolate_wind_vector(altitude, la, ua, lower.vector, upper.vector, fixes);
            }
        }
    }
    WindVector::EMPTY
}

pub fn dot(a: WindVector, b: WindVector) -> f64 {
    match (a.magnitude, a.direction, b.magnitude, b.direction) {
        (Some(m0), Some(d0), Some(m1), Some(d1)) => m0 * m1 * (d0 - d1).cos(),
        _ => 0.0,
    }
}

pub fn tailwind_component(wind: WindVector, true_course_deg: f64) -> f64 {
    -dot(wind, WindVector::new(1.0, true_course_deg * DEG))
}

#[derive(Clone, Debug, Default)]
pub struct WindLeg {
    pub cumulative_distance_with_transitions: f64,
    pub distance_with_transitions: f64,
    pub true_track: Option<f64>,
    pub calculated: bool,
    pub cruise_wind_entries: Vec<WindEntry>,
}

/// One element per flight plan index; `None` is a discontinuity.
#[derive(Clone, Debug, Default)]
pub struct WindPlan {
    pub elements: Vec<Option<WindLeg>>,
    pub first_missed_approach_leg_index: usize,
    pub max_cruise_wind_levels: usize,
    pub climb_wind_entries: Vec<WindEntry>,
    pub descent_wind_entries: Vec<WindEntry>,
    pub pilot_trip_wind: Option<f64>,
    pub origin_altitude: f64,
    pub destination_altitude: f64,
}

impl WindPlan {
    fn leg(&self, i: usize) -> Option<&WindLeg> {
        self.elements.get(i).and_then(Option::as_ref)
    }

    /// `BaseFlightPlan.propagateWindsAt`.
    pub fn propagate_winds_at(&self, at_index: usize) -> Vec<WindEntry> {
        #[derive(Clone, Copy, PartialEq)]
        enum Kind {
            Forward,
            Entry,
            Backward,
        }
        let mut result: Vec<(WindEntry, Kind, usize)> = Vec::new();
        for i in 0..self.first_missed_approach_leg_index.min(self.elements.len()) {
            let Some(leg) = self.leg(i) else { continue };
            for entry in &leg.cruise_wind_entries {
                let kind = if i == at_index || entry.altitude.is_none() {
                    Kind::Entry
                } else if i < at_index {
                    Kind::Forward
                } else {
                    Kind::Backward
                };
                let round = |a: Option<f64>| a.map(|a| (a / 100.0 + 0.5).floor());
                let existing = result.iter().position(|(e, _, src)| {
                    (entry.altitude.is_none() && e.altitude.is_none() && *src == i)
                        || (round(e.altitude).is_some() && round(e.altitude) == round(entry.altitude))
                });
                if let Some(k) = existing {
                    if kind != Kind::Backward {
                        result[k] = (*entry, kind, i);
                    }
                } else if result.len() < self.max_cruise_wind_levels {
                    result.push((*entry, kind, i));
                }
            }
        }
        let entries: Vec<WindEntry> = result.into_iter().map(|(e, _, _)| e).collect();
        let mut sorted: Vec<WindEntry> = entries.iter().copied().filter(|e| e.altitude.is_some()).collect();
        sorted.sort_by(|a, b| b.altitude.unwrap().partial_cmp(&a.altitude.unwrap()).unwrap());
        let mut k = 0;
        entries
            .iter()
            .map(|e| {
                if e.altitude.is_none() {
                    *e
                } else {
                    k += 1;
                    sorted[k - 1]
                }
            })
            .collect()
    }

    fn has_cruise_wind_entry(&self) -> bool {
        (0..self.first_missed_approach_leg_index.min(self.elements.len())).any(|i| self.leg(i).is_some_and(|l| !l.cruise_wind_entries.is_empty()))
    }
}

/// `WindProfile.ts` with `FlightPlanTrackProfile.ts`, `LegWinds` and `WindObserver.ts` (the measurement).
pub struct WindProfile {
    pub plan: WindPlan,
    pub measurement: Option<WindEntry>,
    pub fixes: WindFixes,
    cached_index: Cell<i64>,
}

impl WindProfile {
    pub fn new(plan: WindPlan, measurement: Option<WindEntry>, fixes: WindFixes) -> Self {
        WindProfile { plan, measurement, fixes, cached_index: Cell::new(-1) }
    }

    pub fn leg_index(&self, distance_from_start: f64) -> i64 {
        let cached = self.cached_index.get();
        if cached > 0 {
            match self.plan.leg(cached as usize).filter(|l| l.calculated) {
                Some(leg) => {
                    if distance_from_start > leg.cumulative_distance_with_transitions - leg.distance_with_transitions
                        && distance_from_start <= leg.cumulative_distance_with_transitions
                    {
                        return cached;
                    }
                }
                None => self.cached_index.set(-1),
            }
        }
        for i in 0..self.plan.first_missed_approach_leg_index.min(self.plan.elements.len()) {
            let Some(leg) = self.plan.leg(i).filter(|l| l.calculated) else { continue };
            if leg.cumulative_distance_with_transitions >= distance_from_start {
                self.cached_index.set(i as i64);
                break;
            }
        }
        self.cached_index.get()
    }

    fn track_at(&self, distance_from_start: f64) -> Option<f64> {
        let i = self.leg_index(distance_from_start);
        if i < 0 {
            return None;
        }
        self.plan.leg(i as usize).and_then(|l| l.true_track)
    }

    fn make_vector_at(&self, distance_from_start: f64, tailwind: f64) -> WindVector {
        match self.track_at(distance_from_start) {
            Some(track) => WindVector::new(-tailwind, track * DEG),
            None => WindVector::EMPTY,
        }
    }

    fn vector_forecast(&self, distance_from_start: f64, forecast: Forecast) -> WindVector {
        match forecast {
            Forecast::Vector(v) => v,
            Forecast::Tailwind(t) => self.make_vector_at(distance_from_start, t),
        }
    }

    fn lowest_climb_entry(&self) -> Option<WindEntry> {
        let entries = &self.plan.climb_wind_entries;
        if self.fixes.climb_lowest_entry {
            entries.iter().copied().filter(|e| e.altitude.is_some()).min_by(|a, b| a.altitude.unwrap().partial_cmp(&b.altitude.unwrap()).unwrap())
        } else {
            entries.first().copied()
        }
    }

    pub fn climb_wind_forecast(&self, altitude: f64) -> Forecast {
        let origin = self.plan.origin_altitude;
        if let Some(first) = self.lowest_climb_entry().filter(|e| e.altitude.is_some()) {
            let lowest = first.altitude.unwrap();
            if altitude < lowest {
                let scaling = f64::EPSILON.max(interpolate(clamp(altitude, origin, lowest), origin, lowest, 0.0, 1.0));
                return Forecast::Vector(scale_wind_vector(first.vector, scaling, self.fixes));
            }
            return Forecast::Vector(interpolate_wind_entries(&self.plan.climb_wind_entries, altitude, self.fixes));
        }
        if let Some(trip) = self.plan.pilot_trip_wind {
            return Forecast::Tailwind(trip * clamp(interpolate(altitude, origin, TRIP_WIND_MAX_ALTITUDE, 0.0, 1.0), 0.0, 1.0));
        }
        Forecast::Vector(WindVector::EMPTY)
    }

    pub fn descent_wind_forecast(&self, altitude: f64) -> Forecast {
        let dest = self.plan.destination_altitude;
        if let Some(lowest_entry) = self.plan.descent_wind_entries.last().copied().filter(|e| e.altitude.is_some()) {
            let lowest = lowest_entry.altitude.unwrap();
            if altitude < lowest {
                let scaling = f64::EPSILON.max(interpolate(clamp(altitude, dest, lowest), dest, lowest, 0.0, 1.0));
                return Forecast::Vector(scale_wind_vector(lowest_entry.vector, scaling, self.fixes));
            }
            return Forecast::Vector(interpolate_wind_entries(&self.plan.descent_wind_entries, altitude, self.fixes));
        }
        if let Some(trip) = self.plan.pilot_trip_wind {
            return Forecast::Tailwind(trip * clamp(interpolate(altitude, dest, TRIP_WIND_MAX_ALTITUDE, 0.0, 1.0), 0.0, 1.0));
        }
        Forecast::Vector(WindVector::EMPTY)
    }

    fn leg_cruise_wind_forecast(&self, leg_index: usize, altitude: f64) -> Forecast {
        if self.plan.has_cruise_wind_entry() {
            return Forecast::Vector(interpolate_wind_entries(&self.plan.propagate_winds_at(leg_index), altitude, self.fixes));
        }
        if let Some(trip) = self.plan.pilot_trip_wind {
            return Forecast::Tailwind(trip);
        }
        Forecast::Vector(WindVector::EMPTY)
    }

    fn leg_cruise_wind_vector(&self, leg_index: usize, altitude: f64) -> WindVector {
        match self.leg_cruise_wind_forecast(leg_index, altitude) {
            Forecast::Vector(v) => v,
            Forecast::Tailwind(t) => match self.plan.leg(leg_index).and_then(|l| l.true_track) {
                Some(track) => WindVector::new(-t, track * DEG),
                None => WindVector::EMPTY,
            },
        }
    }

    pub fn cruise_wind_forecast(&self, distance_from_start: f64, altitude: f64) -> Forecast {
        let leg_index = self.leg_index(distance_from_start);
        let prev = if leg_index < 0 { -1 } else { (0..leg_index).rev().find(|&i| self.plan.leg(i as usize).is_some()).unwrap_or(-1) };
        if leg_index < 0 || prev < 0 {
            return Forecast::Tailwind(0.0);
        }
        let (li, pi) = (leg_index as usize, prev as usize);
        if self.plan.has_cruise_wind_entry() {
            let leg_wind = self.leg_cruise_wind_vector(li, altitude);
            let prev_wind = self.leg_cruise_wind_vector(pi, altitude);
            let (leg, prev_leg) = (self.plan.leg(li).unwrap(), self.plan.leg(pi).unwrap());
            if !leg.calculated || !prev_leg.calculated {
                return Forecast::Vector(WindVector::EMPTY);
            }
            return Forecast::Vector(interpolate_wind_vector(
                distance_from_start,
                prev_leg.cumulative_distance_with_transitions,
                leg.cumulative_distance_with_transitions,
                prev_wind,
                leg_wind,
                self.fixes,
            ));
        }
        if let Some(trip) = self.plan.pilot_trip_wind {
            return Forecast::Tailwind(trip);
        }
        Forecast::Vector(WindVector::EMPTY)
    }

    fn blend_with_measurement(&self, forecast: WindVector, altitude: f64) -> WindVector {
        match self.measurement {
            Some(WindEntry { vector, altitude: Some(m_alt) }) => interpolate_wind_vector(
                (altitude - m_alt).abs().min(MAX_WIND_BLEND_ALTITUDE),
                0.0,
                MAX_WIND_BLEND_ALTITUDE,
                vector,
                forecast,
                self.fixes,
            ),
            _ => forecast,
        }
    }

    pub fn climb_wind(&self, distance_from_start: f64, altitude: f64) -> WindVector {
        let f = self.vector_forecast(distance_from_start, self.climb_wind_forecast(altitude));
        self.blend_with_measurement(f, altitude)
    }

    pub fn descent_wind(&self, distance_from_start: f64, altitude: f64) -> WindVector {
        let f = self.vector_forecast(distance_from_start, self.descent_wind_forecast(altitude));
        self.blend_with_measurement(f, altitude)
    }

    pub fn cruise_wind(&self, distance_from_start: f64, distance_from_aircraft: f64, altitude: f64) -> WindVector {
        let forecast = self.vector_forecast(distance_from_start, self.cruise_wind_forecast(distance_from_start, altitude));
        let Some(WindEntry { vector, altitude: Some(m_alt) }) = self.measurement else { return forecast };
        let at_current = self.vector_forecast(distance_from_start, self.cruise_wind_forecast(distance_from_start, m_alt));
        let prediction = interpolate_wind_vector(
            distance_from_aircraft.abs().min(MAX_CRUISE_WIND_BLEND_DISTANCE),
            0.0,
            MAX_CRUISE_WIND_BLEND_DISTANCE,
            vector,
            at_current,
            self.fixes,
        );
        interpolate_wind_vector((altitude - m_alt).abs().min(MAX_WIND_BLEND_ALTITUDE), 0.0, MAX_WIND_BLEND_ALTITUDE, prediction, forecast, self.fixes)
    }

    fn tailwind(&self, distance_from_start: f64, v: WindVector) -> f64 {
        match self.track_at(distance_from_start) {
            Some(track) => tailwind_component(v, track),
            None => 0.0,
        }
    }
}

impl Winds for WindProfile {
    fn climb_tailwind(&self, distance_from_start: f64, altitude: f64) -> f64 {
        self.tailwind(distance_from_start, self.climb_wind(distance_from_start, altitude))
    }
    fn cruise_tailwind(&self, distance_from_start: f64, distance_from_present: f64, altitude: f64) -> f64 {
        self.tailwind(distance_from_start, self.cruise_wind(distance_from_start, distance_from_present, altitude))
    }
    fn descent_tailwind(&self, distance_from_start: f64, altitude: f64) -> f64 {
        self.tailwind(distance_from_start, self.descent_wind(distance_from_start, altitude))
    }
}

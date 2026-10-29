// Translated from FlyByWire's fmgc guidance/vnav/climb/SpeedProfile.ts: McduSpeedProfile, ExpediteSpeedProfile, NdSpeedProfile
// (GPL-3.0-or-later).

use crate::profile::{ComputationParameters, FlightPhase, MaxSpeedConstraint, SpeedLimit};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ManagedSpeedType {
    Climb,
    Cruise,
    Descent,
}

pub struct McduSpeedProfile<'a> {
    pub params: &'a ComputationParameters,
    pub aircraft_distance_along_track: f64,
    pub climb_speed_constraints: Vec<MaxSpeedConstraint>,
    pub descent_speed_constraints: Vec<MaxSpeedConstraint>,
}

fn valid_limit(l: &Option<SpeedLimit>) -> bool {
    matches!(l, Some(l) if l.speed.is_finite() && l.under_altitude.is_finite())
}

impl<'a> McduSpeedProfile<'a> {
    pub fn new(
        params: &'a ComputationParameters,
        aircraft_distance_along_track: f64,
        climb_speed_constraints: Vec<MaxSpeedConstraint>,
        descent_speed_constraints: Vec<MaxSpeedConstraint>,
    ) -> Self {
        Self { params, aircraft_distance_along_track, climb_speed_constraints, descent_speed_constraints }
    }

    pub fn get_target(&self, distance_from_start: f64, altitude: f64, ty: ManagedSpeedType) -> f64 {
        let p = self.params;
        let phase = p.flight_phase;
        let mut preselected = -1.0;
        if phase < FlightPhase::Climb && p.preselected_clb_speed > 100.0 {
            preselected = p.preselected_clb_speed;
        } else if phase < FlightPhase::Cruise && p.preselected_cruise_speed > 100.0 {
            preselected = p.preselected_cruise_speed;
        }
        let has_preselected = preselected > 0.0;
        let predicting_current = (ty == ManagedSpeedType::Climb && phase == FlightPhase::Climb)
            || (ty == ManagedSpeedType::Cruise && phase == FlightPhase::Cruise)
            || (ty == ManagedSpeedType::Descent && (phase == FlightPhase::Descent || phase == FlightPhase::Approach));
        let has_selected = p.fcu_speed > 100.0 && predicting_current;

        if !has_preselected && !has_selected {
            return self.managed_target(distance_from_start, altitude, ty);
        }
        let next_change = self.next_speed_change(self.aircraft_distance_along_track);
        if distance_from_start > next_change {
            return self.managed_target(distance_from_start, altitude, ty);
        }
        if has_preselected {
            preselected
        } else {
            p.fcu_speed
        }
    }

    pub fn target_without_constraints(&self, altitude: f64, ty: ManagedSpeedType) -> f64 {
        let p = self.params;
        let mut managed = match ty {
            ManagedSpeedType::Climb => p.managed_climb_speed,
            ManagedSpeedType::Cruise => p.managed_cruise_speed,
            ManagedSpeedType::Descent => p.managed_descent_speed,
        };
        if ty == ManagedSpeedType::Climb || ty == ManagedSpeedType::Cruise {
            if let (true, Some(l)) = (self.should_take_climb_speed_limit(), p.climb_speed_limit) {
                if altitude < l.under_altitude {
                    managed = managed.min(l.speed);
                }
            }
        } else if let (true, Some(l)) = (self.should_take_descent_speed_limit(), p.descent_speed_limit) {
            if altitude < l.under_altitude {
                managed = managed.min(l.speed);
            }
        }
        managed
    }

    pub fn managed_target(&self, distance_from_start: f64, altitude: f64, ty: ManagedSpeedType) -> f64 {
        let c = if ty == ManagedSpeedType::Climb || ty == ManagedSpeedType::Cruise {
            self.max_climb_speed_constraint(distance_from_start)
        } else {
            self.max_descent_speed_constraint(distance_from_start)
        };
        let constraint_speed = c.map(|c| c.max_speed).unwrap_or(f64::INFINITY);
        self.target_without_constraints(altitude, ty).min(constraint_speed)
    }

    pub fn max_climb_speed_constraint(&self, distance_along_track: f64) -> Option<MaxSpeedConstraint> {
        let mut active: Option<MaxSpeedConstraint> = None;
        for c in &self.climb_speed_constraints {
            if distance_along_track < c.distance_from_start
                && c.max_speed < active.map(|a| a.max_speed).unwrap_or(f64::INFINITY)
            {
                active = Some(*c);
            }
        }
        active
    }

    pub fn max_descent_speed_constraint(&self, distance_along_track: f64) -> Option<MaxSpeedConstraint> {
        let mut active: Option<MaxSpeedConstraint> = None;
        for c in &self.descent_speed_constraints {
            if c.max_speed >= active.map(|a| a.max_speed).unwrap_or(f64::INFINITY) {
                continue;
            }
            if distance_along_track < c.distance_from_start {
                return active;
            }
            active = Some(*c);
        }
        active
    }

    fn next_speed_change(&self, distance_along_track: f64) -> f64 {
        let mut d = f64::INFINITY;
        for c in &self.climb_speed_constraints {
            if distance_along_track <= c.distance_from_start && c.distance_from_start < d {
                d = c.distance_from_start;
            }
        }
        d
    }

    pub fn should_take_climb_speed_limit(&self) -> bool {
        valid_limit(&self.params.climb_speed_limit)
    }

    pub fn should_take_descent_speed_limit(&self) -> bool {
        valid_limit(&self.params.descent_speed_limit)
    }
}

pub trait SpeedProfile {
    fn get_target(&self, distance_from_start: f64, altitude: f64, ty: ManagedSpeedType) -> f64;
    fn target_without_constraints(&self, altitude: f64, ty: ManagedSpeedType) -> f64;
    fn should_take_climb_speed_limit(&self) -> bool;
    fn should_take_descent_speed_limit(&self) -> bool;
    fn max_climb_speed_constraint(&self, distance_along_track: f64) -> Option<MaxSpeedConstraint>;
    fn max_descent_speed_constraint(&self, distance_along_track: f64) -> Option<MaxSpeedConstraint>;
}

impl SpeedProfile for McduSpeedProfile<'_> {
    fn get_target(&self, d: f64, altitude: f64, ty: ManagedSpeedType) -> f64 {
        McduSpeedProfile::get_target(self, d, altitude, ty)
    }
    fn target_without_constraints(&self, altitude: f64, ty: ManagedSpeedType) -> f64 {
        McduSpeedProfile::target_without_constraints(self, altitude, ty)
    }
    fn should_take_climb_speed_limit(&self) -> bool {
        McduSpeedProfile::should_take_climb_speed_limit(self)
    }
    fn should_take_descent_speed_limit(&self) -> bool {
        McduSpeedProfile::should_take_descent_speed_limit(self)
    }
    fn max_climb_speed_constraint(&self, d: f64) -> Option<MaxSpeedConstraint> {
        McduSpeedProfile::max_climb_speed_constraint(self, d)
    }
    fn max_descent_speed_constraint(&self, d: f64) -> Option<MaxSpeedConstraint> {
        McduSpeedProfile::max_descent_speed_constraint(self, d)
    }
}

pub struct ExpediteSpeedProfile {
    pub green_dot_speed: f64,
}

impl SpeedProfile for ExpediteSpeedProfile {
    fn get_target(&self, _: f64, _: f64, _: ManagedSpeedType) -> f64 {
        self.green_dot_speed
    }
    fn target_without_constraints(&self, _: f64, _: ManagedSpeedType) -> f64 {
        self.green_dot_speed
    }
    fn should_take_climb_speed_limit(&self) -> bool {
        false
    }
    fn should_take_descent_speed_limit(&self) -> bool {
        false
    }
    fn max_climb_speed_constraint(&self, _: f64) -> Option<MaxSpeedConstraint> {
        None
    }
    fn max_descent_speed_constraint(&self, _: f64) -> Option<MaxSpeedConstraint> {
        None
    }
}

/// Holds a selected speed until the end of the flight phase instead of only until the next speed constraint.
pub struct NdSpeedProfile<'a> {
    pub params: &'a ComputationParameters,
    pub aircraft_distance_along_track: f64,
    pub max_speed_constraints: Vec<MaxSpeedConstraint>,
    pub descent_speed_constraints: Vec<MaxSpeedConstraint>,
}

fn constraint_to_speed(c: Option<MaxSpeedConstraint>) -> f64 {
    c.map_or(f64::INFINITY, |c| c.max_speed)
}

impl NdSpeedProfile<'_> {
    pub fn is_selected_speed(&self) -> bool {
        let p = self.params;
        let preselected = p.flight_phase < FlightPhase::Climb && p.preselected_clb_speed > 1.0;
        let selected = p.fcu_speed > 100.0 && p.flight_phase > FlightPhase::Takeoff;
        selected || preselected
    }

    pub fn current_speed_target(&self) -> f64 {
        self.find_max_speed_at_distance_along_track(self.aircraft_distance_along_track)
    }

    fn managed_for_type(&self, ty: ManagedSpeedType) -> f64 {
        match ty {
            ManagedSpeedType::Climb => self.params.managed_climb_speed,
            ManagedSpeedType::Cruise => self.params.managed_cruise_speed,
            ManagedSpeedType::Descent => self.params.managed_descent_speed,
        }
    }

    fn find_max_speed_at_distance_along_track(&self, d: f64) -> f64 {
        constraint_to_speed(self.max_climb_speed_constraint(d)).min(constraint_to_speed(self.max_descent_speed_constraint(d)))
    }
}

impl SpeedProfile for NdSpeedProfile<'_> {
    fn get_target(&self, d: f64, altitude: f64, ty: ManagedSpeedType) -> f64 {
        let p = self.params;
        if p.flight_phase < FlightPhase::Climb && p.preselected_clb_speed > 1.0 {
            return p.preselected_clb_speed;
        }
        if p.fcu_speed > 100.0 && p.flight_phase > FlightPhase::Takeoff {
            return p.fcu_speed;
        }
        self.target_without_constraints(altitude, ty).min(self.find_max_speed_at_distance_along_track(d))
    }

    fn target_without_constraints(&self, altitude: f64, ty: ManagedSpeedType) -> f64 {
        let mut managed = self.managed_for_type(ty);
        if ty == ManagedSpeedType::Climb || ty == ManagedSpeedType::Cruise {
            if let (true, Some(l)) = (self.should_take_climb_speed_limit(), self.params.climb_speed_limit) {
                if altitude < l.under_altitude {
                    managed = l.speed.min(managed);
                }
            }
        } else if let (true, Some(l)) = (self.should_take_descent_speed_limit(), self.params.descent_speed_limit) {
            if altitude < l.under_altitude {
                managed = l.speed.min(managed);
            }
        }
        managed
    }

    fn should_take_climb_speed_limit(&self) -> bool {
        valid_limit(&self.params.climb_speed_limit) && !self.is_selected_speed()
    }

    fn should_take_descent_speed_limit(&self) -> bool {
        valid_limit(&self.params.descent_speed_limit) && !self.is_selected_speed()
    }

    fn max_climb_speed_constraint(&self, d: f64) -> Option<MaxSpeedConstraint> {
        let mut active: Option<MaxSpeedConstraint> = None;
        for c in &self.max_speed_constraints {
            if d < c.distance_from_start && c.max_speed < constraint_to_speed(active) {
                active = Some(*c);
            }
        }
        active
    }

    fn max_descent_speed_constraint(&self, d: f64) -> Option<MaxSpeedConstraint> {
        let mut active = None;
        for c in &self.descent_speed_constraints {
            if d < c.distance_from_start {
                return active;
            }
            active = Some(*c);
        }
        active
    }
}

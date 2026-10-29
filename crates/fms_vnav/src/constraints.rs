// Translated from FlyByWire's fmgc guidance/vnav/ConstraintReader.ts (GPL-3.0-or-later).

use crate::profile::{
    default_faf_distance_to_end, AltitudeConstraint, AltitudeDescriptor, DescentAltitudeConstraint, GeographicCruiseStep, MaxAltitudeConstraint,
    MaxSpeedConstraint, Profile,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConstraintType {
    Unknown,
    Climb,
    Descent,
}

#[derive(Clone, Copy, Debug)]
pub struct CruiseStepEntry {
    pub waypoint_index: usize,
    pub to_altitude: f64,
    pub distance_before_termination: f64,
    pub is_ignored: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ConstraintLeg {
    pub cumulative_distance_with_transitions: Option<f64>,
    pub cumulative_distance_to_end_with_transitions: Option<f64>,
    pub constraint_type: ConstraintType,
    pub altitude_constraint: Option<AltitudeConstraint>,
    pub speed_constraint: Option<f64>,
    pub cruise_step: Option<CruiseStepEntry>,
    pub vertical_angle: Option<f64>,
    pub is_final_approach_fix: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ConstraintPlan {
    pub elements: Vec<Option<ConstraintLeg>>,
    pub active_leg_index: usize,
    pub first_missed_approach_leg_index: usize,
    pub destination_leg_index: Option<usize>,
    pub missed_approach_point_altitude: Option<f64>,
    pub destination_runway_threshold_altitude: Option<f64>,
    pub destination_airport_altitude: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct ConstraintReader {
    pub climb_altitude_constraints: Vec<MaxAltitudeConstraint>,
    pub descent_altitude_constraints: Vec<DescentAltitudeConstraint>,
    pub climb_speed_constraints: Vec<MaxSpeedConstraint>,
    pub descent_speed_constraints: Vec<MaxSpeedConstraint>,
    pub cruise_steps: Vec<GeographicCruiseStep>,
    pub total_flight_plan_distance: f64,
    pub final_descent_angle: f64,
    pub faf_distance_to_end: f64,
    pub final_altitude: f64,
    pub distance_to_end: f64,
}

impl Default for ConstraintReader {
    fn default() -> Self {
        Self {
            climb_altitude_constraints: Vec::new(),
            descent_altitude_constraints: Vec::new(),
            climb_speed_constraints: Vec::new(),
            descent_speed_constraints: Vec::new(),
            cruise_steps: Vec::new(),
            total_flight_plan_distance: 0.0,
            final_descent_angle: -3.0,
            faf_distance_to_end: default_faf_distance_to_end(-3.0),
            final_altitude: 50.0,
            distance_to_end: 0.0,
        }
    }
}

impl ConstraintReader {
    pub fn distance_to_present_position(&self) -> f64 {
        self.total_flight_plan_distance - self.distance_to_end
    }

    pub fn update_flight_plan(&mut self, plan: &ConstraintPlan, distance_to_end: f64) {
        *self = Self { distance_to_end, ..Self::default() };
        self.update_distances_to_end(plan);

        let mut max_speed = f64::INFINITY;
        for i in 0..plan.first_missed_approach_leg_index.min(plan.elements.len()) {
            let Some(leg) = plan.elements[i] else { continue };
            let leg_distance_to_end = leg.cumulative_distance_to_end_with_transitions.unwrap_or(self.total_flight_plan_distance);
            let leg_distance_from_start = leg.cumulative_distance_with_transitions.unwrap_or(0.0);

            if let Some(step) = leg.cruise_step {
                if !step.is_ignored && i >= plan.active_leg_index {
                    self.cruise_steps.push(GeographicCruiseStep {
                        distance_from_start: leg_distance_from_start - step.distance_before_termination,
                        to_altitude: step.to_altitude,
                        waypoint_index: step.waypoint_index,
                        is_ignored: false,
                    });
                }
            }

            use AltitudeDescriptor::*;
            match leg.constraint_type {
                ConstraintType::Climb => {
                    if let Some(c) = leg.altitude_constraint {
                        if matches!(c.descriptor, AtAlt1 | AtOrBelowAlt1 | BetweenAlt1Alt2) {
                            self.climb_altitude_constraints.push(MaxAltitudeConstraint { distance_from_start: leg_distance_from_start, max_altitude: c.altitude1 });
                        }
                    }
                    if let Some(speed) = leg.speed_constraint.filter(|s| *s > 100.0) {
                        self.climb_speed_constraints.push(MaxSpeedConstraint { distance_from_start: leg_distance_from_start, max_speed: speed });
                    }
                }
                ConstraintType::Descent => {
                    if let Some(c) = leg.altitude_constraint {
                        if matches!(
                            c.descriptor,
                            AtAlt1 | AtOrAboveAlt1 | AtAlt1GsIntcptAlt2 | AtOrAboveAlt1GsIntcptAlt2 | AtOrAboveAlt1AngleAlt2 | AtAlt1AngleAlt2 | AtOrBelowAlt1 | BetweenAlt1Alt2
                        ) {
                            self.descent_altitude_constraints.push(DescentAltitudeConstraint { distance_from_start: leg_distance_from_start, constraint: c, leg: i });
                        }
                    }
                    if let Some(speed) = leg.speed_constraint.filter(|s| *s > 100.0) {
                        max_speed = max_speed.min(speed);
                        self.descent_speed_constraints.push(MaxSpeedConstraint { distance_from_start: leg_distance_from_start, max_speed });
                    }
                }
                ConstraintType::Unknown => {}
            }

            if Some(i) == plan.destination_leg_index {
                if let Some(angle) = leg.vertical_angle.filter(|a| *a != 0.0) {
                    self.final_descent_angle = angle;
                }
            }
            if leg.is_final_approach_fix {
                self.faf_distance_to_end = leg_distance_to_end;
            }
        }

        self.final_altitude = if let Some(alt) = plan.missed_approach_point_altitude.filter(|a| a.is_finite()) {
            alt
        } else if let Some(alt) = plan.destination_runway_threshold_altitude.filter(|a| a.is_finite()) {
            alt + 50.0
        } else if let Some(alt) = plan.destination_airport_altitude.filter(|a| a.is_finite()) {
            alt + 50.0
        } else {
            50.0
        };
    }

    fn update_distances_to_end(&mut self, plan: &ConstraintPlan) {
        self.total_flight_plan_distance = 0.0;
        for i in plan.active_leg_index.saturating_sub(1)..plan.first_missed_approach_leg_index.min(plan.elements.len()) {
            if let Some(Some(d)) = plan.elements[i].map(|l| l.cumulative_distance_to_end_with_transitions) {
                self.total_flight_plan_distance = d;
                return;
            }
        }
    }

    /// Marks the plan leg's cruise step as ignored, as `ConstraintReader.ignoreCruiseStep` does on the flight plan.
    pub fn ignore_cruise_step(plan: &mut ConstraintPlan, waypoint_index: usize) {
        if let Some(Some(leg)) = plan.elements.get_mut(waypoint_index) {
            if let Some(step) = leg.cruise_step.as_mut() {
                step.is_ignored = true;
            }
        }
    }

    pub fn apply_to_profile(&self, profile: &mut Profile) {
        profile.max_altitude_constraints = self.climb_altitude_constraints.clone();
        profile.max_climb_speed_constraints = self.climb_speed_constraints.clone();
        profile.descent_speed_constraints = self.descent_speed_constraints.clone();
        profile.descent_altitude_constraints = self.descent_altitude_constraints.clone();
        profile.cruise_steps = self.cruise_steps.clone();
        profile.total_flight_plan_distance = self.total_flight_plan_distance;
        profile.final_descent_angle = self.final_descent_angle;
        profile.faf_distance_to_end = self.faf_distance_to_end;
        profile.final_altitude = self.final_altitude;
        profile.distance_to_present_position = self.distance_to_present_position();
    }
}

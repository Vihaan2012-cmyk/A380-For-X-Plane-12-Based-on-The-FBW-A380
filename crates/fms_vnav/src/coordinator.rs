// Translated from FlyByWire's fmgc guidance/vnav/CruiseToDescentCoordinator.ts and descent/ProfileInterceptCalculator.ts
// (GPL-3.0-or-later).

use crate::approach::ApproachPathBuilder;
use crate::config::AircraftConfig;
use crate::cruise::CruisePathBuilder;
use crate::descent::{DescentPathBuilder, TemporaryCheckpointSequence};
use crate::profile::{Checkpoint, ComputationParameters, Profile, Reason};
use crate::speed_profile::McduSpeedProfile;
use crate::strategy::ClimbStrategy;

#[allow(clippy::too_many_arguments)]
pub fn intersect(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64) -> Option<(f64, f64)> {
    if (x1 == x2 && y1 == y2) || (x3 == x4 && y3 == y4) {
        return None;
    }
    let denominator = (y4 - y3) * (x2 - x1) - (x4 - x3) * (y2 - y1);
    if denominator == 0.0 {
        return None;
    }
    let ua = ((x4 - x3) * (y1 - y3) - (y4 - y3) * (x1 - x3)) / denominator;
    let ub = ((x2 - x1) * (y1 - y3) - (y2 - y1) * (x1 - x3)) / denominator;
    if !(0.0..=1.0).contains(&ua) || !(0.0..=1.0).contains(&ub) {
        return None;
    }
    Some((x1 + ua * (x2 - x1), y1 + ua * (y2 - y1)))
}

pub fn calculate_intercept(checkpoints1: &[Checkpoint], checkpoints2: &[Checkpoint], offset: f64) -> Option<(usize, f64)> {
    for i in 0..checkpoints1.len().saturating_sub(1) {
        let (a, b) = (&checkpoints1[i], &checkpoints1[i + 1]);
        for j in 0..checkpoints2.len().saturating_sub(1) {
            let (c, d) = (&checkpoints2[j], &checkpoints2[j + 1]);
            if let Some((x, _)) = intersect(
                a.distance_from_start,
                a.altitude,
                b.distance_from_start,
                b.altitude,
                c.distance_from_start + offset,
                c.altitude,
                d.distance_from_start + offset,
                d.altitude,
            ) {
                return Some((i, x));
            }
        }
    }
    None
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CoordinatorError {
    ApproachDidNotEndInDecel,
    DescentDidNotEndInTopOfDescent,
}

#[derive(Clone, Copy, Debug)]
pub struct CruiseToDescentCoordinator {
    pub last_estimated_fuel_at_destination: f64,
    pub last_estimated_time_at_destination: f64,
}

impl Default for CruiseToDescentCoordinator {
    fn default() -> Self {
        Self { last_estimated_fuel_at_destination: 4000.0, last_estimated_time_at_destination: 0.0 }
    }
}

impl CruiseToDescentCoordinator {
    pub fn reset_estimations(&mut self, estimated_destination_fuel: Option<f64>, config: &AircraftConfig) {
        self.last_estimated_fuel_at_destination = match estimated_destination_fuel.filter(|f| f.is_finite()) {
            Some(f) => f.max(config.vnav.lowest_fuel_estimate).min(config.vnav.maximum_fuel_estimate),
            None => 4000.0,
        };
        self.last_estimated_time_at_destination = 0.0;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_cruise_and_descent_path(
        &mut self,
        params: &ComputationParameters,
        profile: &mut Profile,
        speed_profile: &McduSpeedProfile,
        step_climb: &dyn ClimbStrategy,
        step_descent: &dyn ClimbStrategy,
        estimated_destination_fuel: Option<f64>,
        too_steep_legs: &mut Vec<usize>,
    ) -> Result<(), CoordinatorError> {
        let Some(starting_index) = profile.find_last_checkpoint_index(&[Reason::TopOfClimb, Reason::PresentPosition]) else {
            return Ok(());
        };
        let mut starting_point = profile.checkpoints[starting_index];
        let (mut iterations, mut fuel_error, mut time_error) = (0, f64::INFINITY, f64::INFINITY);
        if self.last_estimated_fuel_at_destination.is_nan() || self.last_estimated_time_at_destination.is_nan() {
            self.reset_estimations(estimated_destination_fuel, profile.config);
        }

        let config = profile.config;
        let approach = ApproachPathBuilder::new(params, config);
        let descent_builder = DescentPathBuilder::new(params, config);
        let cruise_builder = CruisePathBuilder::new(params);
        let mut descent_path = TemporaryCheckpointSequence { checkpoints: Vec::new() };
        let mut cruise_path: Vec<Checkpoint> = Vec::new();

        while iterations < 4 && (fuel_error.abs() > 100.0 || time_error.abs() > 1.0) {
            iterations += 1;
            descent_path = approach.compute_approach_path(profile, speed_profile, self.last_estimated_fuel_at_destination, self.last_estimated_time_at_destination, too_steep_legs);
            if descent_path.last().reason != Reason::Decel {
                return Err(CoordinatorError::ApproachDidNotEndInDecel);
            }
            let final_cruise_altitude = cruise_builder.final_cruise_altitude(&profile.cruise_steps);
            descent_builder.compute_managed_descent_path(&mut descent_path, profile, speed_profile, final_cruise_altitude, f64::NEG_INFINITY, too_steep_legs);
            if descent_path.last().reason != Reason::TopOfDescent {
                return Err(CoordinatorError::DescentDidNotEndInTopOfDescent);
            }

            if descent_path.last().distance_from_start < starting_point.distance_from_start {
                if starting_point.reason == Reason::PresentPosition {
                    profile.checkpoints.extend(descent_path.checkpoints.iter().rev().copied());
                    return Ok(());
                }
                if starting_point.reason == Reason::TopOfClimb {
                    match calculate_intercept(&profile.checkpoints, &descent_path.checkpoints, 0.0) {
                        None => {
                            let cruise = cruise_builder.compute_cruise_path_with_steps(profile, starting_point, descent_path.at(0).distance_from_start, step_climb, step_descent, speed_profile);
                            profile.checkpoints.extend(cruise.iter().skip(1).copied());
                            return Ok(());
                        }
                        Some((_, intercept_distance)) => {
                            let index = profile.add_interpolated_checkpoint(intercept_distance, Reason::TopOfClimb);
                            let combined = profile.checkpoints[index];
                            let saved_tod = descent_path.last();
                            descent_path.checkpoints.retain(|c| c.distance_from_start >= combined.distance_from_start);
                            descent_path.checkpoints.push(Checkpoint { distance_from_start: combined.distance_from_start, altitude: combined.altitude, ..saved_tod });
                            starting_point = combined;
                        }
                    }
                }
            }

            cruise_path = cruise_builder.compute_cruise_path_with_steps(profile, starting_point, descent_path.last().distance_from_start, step_climb, step_descent, speed_profile);
            let cruise_end = *cruise_path.last().unwrap();
            fuel_error = cruise_end.remaining_fuel_on_board - descent_path.last().remaining_fuel_on_board;
            time_error = cruise_end.seconds_from_present - descent_path.last().seconds_from_present;
            self.last_estimated_fuel_at_destination += fuel_error;
            self.last_estimated_time_at_destination += time_error;
        }

        profile.checkpoints.retain(|c| c.distance_from_start <= starting_point.distance_from_start);
        profile.checkpoints.extend(cruise_path.iter().skip(1).copied());
        profile.checkpoints.extend(descent_path.checkpoints.iter().rev().copied());
        Ok(())
    }
}

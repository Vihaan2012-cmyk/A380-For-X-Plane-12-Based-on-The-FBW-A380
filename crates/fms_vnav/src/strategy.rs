// Translated from FlyByWire's fmgc guidance/vnav/climb/ClimbStrategy.ts (GPL-3.0-or-later).

use crate::atmosphere::{Atmosphere, FT_PER_NM};
use crate::config::{AircraftConfig, FlapConf};
use crate::engine as eng;
use crate::predictions::{self as pred, StepEnv, StepResults};
use crate::profile::ComputationParameters;

#[derive(Clone, Copy, Debug)]
pub struct SurfaceConfig {
    pub flap_conf: FlapConf,
    pub gear_extended: bool,
    pub speedbrakes_extended: bool,
}

impl Default for SurfaceConfig {
    fn default() -> Self {
        Self { flap_conf: FlapConf::Clean, gear_extended: false, speedbrakes_extended: false }
    }
}

pub trait ClimbStrategy {
    fn predict_to_altitude(&self, initial_altitude: f64, final_altitude: f64, speed: f64, mach: f64, fuel_on_board: f64, headwind: f64, cfg: SurfaceConfig) -> StepResults;
    fn predict_to_distance(&self, initial_altitude: f64, distance: f64, speed: f64, mach: f64, fuel_on_board: f64, headwind: f64, cfg: SurfaceConfig) -> StepResults;
    fn predict_to_speed(&self, initial_altitude: f64, final_speed: f64, initial_speed: f64, mach: f64, fuel_on_board: f64, headwind: f64, cfg: SurfaceConfig) -> StepResults;
}

fn env(p: &ComputationParameters, atm: &Atmosphere, c: SurfaceConfig) -> StepEnv {
    StepEnv {
        zero_fuel_weight: p.zero_fuel_weight,
        isa_dev: atm.isa_deviation,
        tropo_altitude: p.tropopause,
        perf_factor_percent: p.perf_factor,
        speedbrakes_extended: c.speedbrakes_extended,
        flaps: c.flap_conf,
        gear_extended: c.gear_extended,
    }
}

pub fn climb_thrust_corrected_n1_limit(cfg: &AircraftConfig, atm: &Atmosphere, altitude: f64) -> f64 {
    eng::climb_thrust_corrected_n1(&cfg.engine, altitude, atm.predict_sat(altitude))
}

pub struct ClimbThrustClimbStrategy<'a> {
    pub params: &'a ComputationParameters,
    pub atm: Atmosphere,
    pub config: &'a AircraftConfig,
}

impl ClimbStrategy for ClimbThrustClimbStrategy<'_> {
    fn predict_to_altitude(&self, ia: f64, fa: f64, speed: f64, mach: f64, fob: f64, hw: f64, c: SurfaceConfig) -> StepResults {
        let n1 = climb_thrust_corrected_n1_limit(self.config, &self.atm, (ia + fa) / 2.0);
        pred::altitude_step(self.config, ia, fa - ia, speed, mach, n1, fob, hw, &env(self.params, &self.atm, c))
    }

    fn predict_to_distance(&self, ia: f64, distance: f64, speed: f64, mach: f64, fob: f64, hw: f64, c: SurfaceConfig) -> StepResults {
        let n1 = climb_thrust_corrected_n1_limit(self.config, &self.atm, ia);
        pred::distance_step(self.config, ia, distance, speed, mach, n1, fob, hw, &env(self.params, &self.atm, c))
    }

    fn predict_to_speed(&self, ia: f64, final_speed: f64, initial_speed: f64, mach: f64, fob: f64, hw: f64, c: SurfaceConfig) -> StepResults {
        let n1 = climb_thrust_corrected_n1_limit(self.config, &self.atm, ia);
        pred::altitude_step_with_speed_change(self.config, ia, initial_speed, final_speed, mach, n1, fob, hw, &env(self.params, &self.atm, c))
    }
}

pub struct VerticalSpeedStrategy<'a> {
    pub params: &'a ComputationParameters,
    pub atm: Atmosphere,
    pub vertical_speed: f64,
    pub config: &'a AircraftConfig,
}

impl ClimbStrategy for VerticalSpeedStrategy<'_> {
    fn predict_to_altitude(&self, ia: f64, fa: f64, speed: f64, mach: f64, fob: f64, hw: f64, c: SurfaceConfig) -> StepResults {
        let mut e = env(self.params, &self.atm, c);
        e.speedbrakes_extended = c.gear_extended;
        pred::vertical_speed_step(self.config, ia, fa, self.vertical_speed, speed, mach, fob, hw, &e)
    }

    fn predict_to_distance(&self, ia: f64, distance: f64, speed: f64, mach: f64, fob: f64, hw: f64, c: SurfaceConfig) -> StepResults {
        pred::vertical_speed_distance_step(self.config, ia, distance, self.vertical_speed, speed, mach, fob, hw, &env(self.params, &self.atm, c))
    }

    fn predict_to_speed(&self, ia: f64, final_speed: f64, initial_speed: f64, mach: f64, fob: f64, hw: f64, c: SurfaceConfig) -> StepResults {
        let computed_mach = self.atm.mach_from_cas(ia, initial_speed).min(mach);
        let n1 = if self.vertical_speed > 0.0 {
            climb_thrust_corrected_n1_limit(self.config, &self.atm, ia)
        } else {
            eng::idle_corrected_n1(&self.config.engine, ia, computed_mach, self.params.tropopause) + self.config.vnav.idle_n1_margin
        };
        pred::vertical_speed_step_with_speed_change(self.config, ia, initial_speed, final_speed, self.vertical_speed, mach, n1, fob, hw, &env(self.params, &self.atm, c))
    }
}

pub struct FlightPathAngleStrategy<'a> {
    pub params: &'a ComputationParameters,
    pub atm: Atmosphere,
    pub flight_path_angle_deg: f64,
    pub config: &'a AircraftConfig,
}

impl ClimbStrategy for FlightPathAngleStrategy<'_> {
    fn predict_to_altitude(&self, ia: f64, fa: f64, speed: f64, mach: f64, fob: f64, hw: f64, c: SurfaceConfig) -> StepResults {
        let distance = (fa - ia) / FT_PER_NM / self.flight_path_angle_deg.to_radians().tan();
        pred::geometric_step(self.config, ia, fa, distance, speed, mach, fob, hw, &env(self.params, &self.atm, c))
    }

    fn predict_to_distance(&self, ia: f64, distance: f64, speed: f64, mach: f64, fob: f64, hw: f64, c: SurfaceConfig) -> StepResults {
        let fa = ia + distance * FT_PER_NM * self.flight_path_angle_deg.to_radians().tan();
        pred::geometric_step(self.config, ia, fa, distance, speed, mach, fob, hw, &env(self.params, &self.atm, c))
    }

    fn predict_to_speed(&self, ia: f64, final_speed: f64, initial_speed: f64, mach: f64, fob: f64, hw: f64, c: SurfaceConfig) -> StepResults {
        let computed_mach = self.atm.mach_from_cas(ia, initial_speed).min(mach);
        let n1 = if self.flight_path_angle_deg > 0.0 {
            climb_thrust_corrected_n1_limit(self.config, &self.atm, ia)
        } else {
            eng::idle_corrected_n1(&self.config.engine, ia, computed_mach, self.params.tropopause) + self.config.vnav.idle_n1_margin
        };
        pred::speed_change_step(self.config, self.flight_path_angle_deg, ia, initial_speed, final_speed, mach, mach, n1, fob, hw, &env(self.params, &self.atm, c), None)
    }
}

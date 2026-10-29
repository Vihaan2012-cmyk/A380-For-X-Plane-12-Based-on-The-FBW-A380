//! Per-type engine data: one struct both engine types fill, read by the
//! start sequencer (`deep::engine_accessories::start_sequencer`), the start
//! protections in `physics::engine`, `physics::damage` and the EWD/SD
//! patches in `aircraft_options`.
//!
//! Sources:
//! - Trent 900: EASA.E.012 TCDS, the constants in `physics::damage::trent900`
//!   and `physics::engine::params`.
//! - GP7200: EASA.IM.E.026 TCDS (take-off 1002 C, max continuous 970 C, the
//!   figures already used by the EWD patches), FCOM LIM-70 (start EGT on
//!   ground / in flight, starter duty, N1/N2 max, oil) and the FCOM idle
//!   check in PRO-NOR-SOP-80.
//! - Start schedule (both types): FCOM DSC-70-80-20, DSC-70-80-30-20,
//!   DSC-70-80-30-30, LIM-70 (starter). The FCOM is a GP7200 fleet manual;
//!   no public Trent 900 schedule exists, so the Trent uses the same
//!   schedule (GENERIC for the Trent).
//!
//! "HP" is the HP spool for both types: Trent N3, GP7200 N2.

use crate::physics::damage::trent900;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineType {
    Trent900,
    Gp7200,
}

impl EngineType {
    pub fn current() -> Self {
        if crate::aircraft_options::engine_rating().gp { Self::Gp7200 } else { Self::Trent900 }
    }

    pub fn data(self) -> &'static EngineData {
        match self {
            Self::Trent900 => &TRENT_900,
            Self::Gp7200 => &GP7200,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineData {
    pub engine_type: EngineType,
    pub spools: u8,
    /// Displayed EGT/TGT limits, C.
    pub egt_takeoff_c: f64,
    pub egt_max_continuous_c: f64,
    pub egt_ground_start_c: f64,
    pub egt_relight_c: f64,
    /// HP speed below which the ground start limit applies, %.
    pub start_limit_hp_pct: f64,
    /// Displayed EGT = 15 + (physical - 15) * scale. The physics is the
    /// Trent gas path, so the GP7200 maps the Trent take-off TGT onto its own.
    pub egt_display_scale: f64,
    pub lp_max_pct: f64,
    pub hp_max_pct: f64,
    pub oil_min_start_c: f64,
    pub oil_min_takeoff_c: f64,
    pub idle_lp_pct: f64,
    pub idle_hp_pct: f64,
    pub idle_egt_c: f64,
    pub start: StartSchedule,
}

/// Start schedule, HP % and seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StartSchedule {
    /// Fuel and ignition on the ground at this HP speed, after `rotor_bow_crank_s`.
    pub fuel_on_hp_pct: f64,
    pub rotor_bow_crank_s: f64,
    pub auto_ignition_off_hp_pct: f64,
    pub auto_valve_close_hp_pct: f64,
    pub manual_ignition_off_hp_pct: f64,
    pub manual_valve_close_hp_pct: f64,
    /// No automatic ground abort above this HP speed.
    pub no_abort_above_hp_pct: f64,
    pub starter_engage_max_hp_pct: f64,
    pub starter_assist_below_hp_pct: f64,
    pub windmill_min_cas_kt: f64,
    pub quick_relight_min_hp_pct: f64,
    pub quick_relight_window_s: f64,
    pub relight_ignition_hold_s: f64,
    pub starter_max_continuous_s: f64,
    pub dry_crank_s: f64,
    pub attempts: u8,
    /// No light-up: no EGT rise of `light_off_rise_c` within this time of fuel on.
    pub no_light_up_s: f64,
    pub light_off_rise_c: f64,
    /// Hung start: HP gains less than `hung_min_gain_pct` over `hung_window_s`.
    pub hung_window_s: f64,
    pub hung_min_gain_pct: f64,
    /// N1 jam: LP below `n1_jam_lp_pct` with HP above `n1_jam_hp_pct`.
    pub n1_jam_hp_pct: f64,
    pub n1_jam_lp_pct: f64,
    /// Start valve abnormal: valve not where commanded after this time.
    pub valve_abnormal_s: f64,
}

const SCHEDULE: StartSchedule = StartSchedule {
    fuel_on_hp_pct: 20.0,
    rotor_bow_crank_s: 20.0,
    auto_ignition_off_hp_pct: 58.0,
    auto_valve_close_hp_pct: 58.4,
    manual_ignition_off_hp_pct: 56.0,
    manual_valve_close_hp_pct: 58.0,
    no_abort_above_hp_pct: 58.4,
    starter_engage_max_hp_pct: 25.0,
    starter_assist_below_hp_pct: 11.0,
    windmill_min_cas_kt: 260.0,
    quick_relight_min_hp_pct: 45.0,
    quick_relight_window_s: 30.0,
    relight_ignition_hold_s: 60.0,
    starter_max_continuous_s: 300.0,
    dry_crank_s: 30.0,
    attempts: 3,
    no_light_up_s: 30.0,
    light_off_rise_c: 50.0,
    hung_window_s: 15.0,
    hung_min_gain_pct: 1.0,
    n1_jam_hp_pct: 35.0,
    n1_jam_lp_pct: 2.0,
    valve_abnormal_s: 12.0,
};

pub const TRENT_900: EngineData = EngineData {
    engine_type: EngineType::Trent900,
    spools: 3,
    egt_takeoff_c: trent900::TGT_TAKEOFF_C,
    egt_max_continuous_c: trent900::TGT_MAX_CONTINUOUS_C,
    egt_ground_start_c: trent900::TGT_GROUND_START_C,
    egt_relight_c: trent900::TGT_RELIGHT_C,
    start_limit_hp_pct: trent900::GROUND_START_HP_PCT,
    egt_display_scale: 1.0,
    lp_max_pct: trent900::LP_TAKEOFF_MAX_PCT,
    hp_max_pct: trent900::HP_TAKEOFF_MAX_PCT,
    oil_min_start_c: trent900::OIL_TEMP_MIN_START_C,
    oil_min_takeoff_c: trent900::OIL_TEMP_MIN_TAKEOFF_C,
    idle_lp_pct: super::params::IDLE_N1_PCT,
    idle_hp_pct: super::params::IDLE_N3_PCT,
    idle_egt_c: 455.0,
    start: SCHEDULE,
};

pub const GP7200: EngineData = EngineData {
    engine_type: EngineType::Gp7200,
    spools: 2,
    egt_takeoff_c: 1002.0,
    egt_max_continuous_c: 970.0,
    egt_ground_start_c: 745.0,
    egt_relight_c: 865.0,
    start_limit_hp_pct: trent900::GROUND_START_HP_PCT,
    egt_display_scale: (1002.0 - 15.0) / (trent900::TGT_TAKEOFF_C - 15.0),
    lp_max_pct: 111.0,
    hp_max_pct: 118.7,
    oil_min_start_c: -40.0,
    oil_min_takeoff_c: 50.0,
    idle_lp_pct: 20.0,
    idle_hp_pct: 66.0,
    idle_egt_c: 455.0,
    start: SCHEDULE,
};

impl EngineData {
    pub fn current() -> &'static Self {
        EngineType::current().data()
    }

    pub fn display_egt_c(&self, physical_c: f64) -> f64 {
        15.0 + (physical_c - 15.0) * self.egt_display_scale
    }

    pub fn physical_egt_c(&self, display_c: f64) -> f64 {
        15.0 + (display_c - 15.0) / self.egt_display_scale
    }

    pub fn ground_start_limit_physical_c(&self) -> f64 {
        self.physical_egt_c(self.egt_ground_start_c)
    }

    pub fn relight_limit_physical_c(&self) -> f64 {
        self.physical_egt_c(self.egt_relight_c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trent_display_is_the_physical_tgt() {
        assert_eq!(TRENT_900.display_egt_c(712.0), 712.0);
        assert_eq!(TRENT_900.ground_start_limit_physical_c(), trent900::TGT_GROUND_START_C);
    }

    #[test]
    fn gp_maps_the_trent_takeoff_tgt_onto_its_red_line_and_back() {
        assert!((GP7200.display_egt_c(trent900::TGT_TAKEOFF_C) - 1002.0).abs() < 1e-9);
        assert!((GP7200.display_egt_c(GP7200.ground_start_limit_physical_c()) - 745.0).abs() < 1e-9);
        assert!(GP7200.ground_start_limit_physical_c() < trent900::TGT_GROUND_START_C);
    }
}

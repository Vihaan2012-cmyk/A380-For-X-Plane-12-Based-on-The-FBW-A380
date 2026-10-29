use crate::constants::*;
use crate::cost_index::*;
use crate::isa::{mach_to_kcas, mach_to_kcas_at_altitude};
use crate::speeds::FlightPhase;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpeedLimit {
    pub speed: f64,
    pub under_altitude: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LegSpeedConstraints {
    pub climb_speed: f64,
    pub descent_speed: f64,
    pub previous_descent_speed: f64,
}

pub fn calculate_decel_dist(from_speed: f64, to_speed: f64) -> f64 {
    20.0_f64.min(3.0_f64.max((from_speed - to_speed) * 0.15))
}

pub fn nav_mode_speed_constraint(
    phase: FlightPhase,
    constraints: Option<LegSpeedConstraints>,
    active_leg_dtg: Option<f64>,
    managed_descent_speed: f64,
) -> f64 {
    if let Some(c) = constraints {
        if phase < FlightPhase::Cruise || phase == FlightPhase::GoAround {
            return c.climb_speed;
        }
        if phase > FlightPhase::Cruise && phase < FlightPhase::GoAround {
            if let Some(dtg) = active_leg_dtg.filter(|d| *d != 0.0) {
                if dtg < calculate_decel_dist(c.previous_descent_speed.min(managed_descent_speed), c.descent_speed) {
                    return c.descent_speed;
                }
            }
            return c.previous_descent_speed;
        }
    }
    f64::INFINITY
}

pub fn speed_constraint(
    lateral_mode_managed: bool,
    phase: FlightPhase,
    constraints: Option<LegSpeedConstraints>,
    active_leg_dtg: Option<f64>,
    managed_descent_speed: f64,
) -> f64 {
    if !lateral_mode_managed {
        return f64::INFINITY;
    }
    nav_mode_speed_constraint(phase, constraints, active_leg_dtg, managed_descent_speed)
}

pub fn managed_targets(v: f64, mach: f64, static_pressure_hpa: Option<f64>) -> (f64, bool) {
    match static_pressure_hpa {
        Some(p) => {
            let vm = mach_to_kcas(mach, p);
            if v > vm {
                (vm, true)
            } else {
                (v, false)
            }
        }
        None => (v, false),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HoldingSpeedInputs {
    pub fcu_altitude_ft: f64,
    pub altitude_ft: Option<f64>,
    pub green_dot_kt: f64,
    pub all_engines_on: bool,
    pub phase: FlightPhase,
    pub approach_managed_speed_kt: Option<f64>,
    pub speed_constraint_kt: Option<f64>,
    pub climb_speed_limit: Option<SpeedLimit>,
    pub descent_speed_limit: Option<SpeedLimit>,
    pub vls_kt: f64,
}

pub fn holding_speed(i: &HoldingSpeedInputs) -> f64 {
    let alt = i.fcu_altitude_ft.max(i.altitude_ft.filter(|a| *a != 0.0).unwrap_or(0.0));
    let mut kcas = i.green_dot_kt;
    if !i.all_engines_on {
        kcas += 25.0;
    }
    if i.phase == FlightPhase::Approach {
        kcas = i.approach_managed_speed_kt.unwrap_or(0.0);
    }
    if let Some(c) = i.speed_constraint_kt {
        if c > 100.0 {
            kcas = kcas.min(c);
        }
    }
    if alt < 14_000.0 {
        kcas = kcas.min(230.0);
    } else if alt < 20_000.0 {
        kcas = kcas.min(240.0);
    } else if alt < 34_000.0 {
        kcas = kcas.min(265.0);
    } else {
        kcas = mach_to_kcas_at_altitude(0.83, alt);
    }
    if i.phase <= FlightPhase::Cruise {
        if let Some(l) = i.climb_speed_limit {
            if alt <= l.under_altitude {
                kcas = kcas.min(l.speed);
            }
        }
    } else if i.phase < FlightPhase::GoAround {
        if let Some(l) = i.descent_speed_limit {
            if alt <= l.under_altitude {
                kcas = kcas.min(l.speed);
            }
        }
    }
    kcas = kcas.max(i.vls_kt);
    kcas.ceil()
}

pub fn approach_managed_speed(
    flap_lever: i32,
    approach_flaps_three_selected: bool,
    green_dot: Option<f64>,
    slat_retraction: Option<f64>,
    flap_retraction: Option<f64>,
    approach_vapp: Option<f64>,
) -> Option<f64> {
    match flap_lever {
        0 => green_dot,
        1 => slat_retraction,
        3 => {
            if approach_flaps_three_selected {
                approach_vapp
            } else {
                flap_retraction
            }
        }
        4 => approach_vapp,
        _ => flap_retraction,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ManagedSpeedInputs {
    pub phase: FlightPhase,
    pub hold_decel_reached: bool,
    pub hold_speed_target: Option<f64>,
    pub v2_kt: Option<f64>,
    pub cost_index: Option<f64>,
    pub pilot_descent_speed_kt: Option<f64>,
    pub pilot_descent_mach: Option<f64>,
    pub climb_speed_limit: Option<SpeedLimit>,
    pub indicated_altitude_ft: f64,
    pub speed_constraint_kt: f64,
    pub engine_out: bool,
    pub green_dot_kt: Option<f64>,
    pub open_or_managed_climb_active: bool,
    pub cas_kt: Option<f64>,
    pub vnav_managed_pfd_kt: f64,
    pub approach_vapp_kt: Option<f64>,
    pub static_pressure_hpa: Option<f64>,
    pub fms_cycle_time_ms: f64,
}

impl Default for ManagedSpeedInputs {
    fn default() -> Self {
        ManagedSpeedInputs {
            phase: FlightPhase::Preflight,
            hold_decel_reached: false,
            hold_speed_target: None,
            v2_kt: None,
            cost_index: None,
            pilot_descent_speed_kt: None,
            pilot_descent_mach: None,
            climb_speed_limit: None,
            indicated_altitude_ft: 0.0,
            speed_constraint_kt: f64::INFINITY,
            engine_out: false,
            green_dot_kt: None,
            open_or_managed_climb_active: false,
            cas_kt: None,
            vnav_managed_pfd_kt: 0.0,
            approach_vapp_kt: None,
            static_pressure_hpa: None,
            fms_cycle_time_ms: FMS_CYCLE_TIME_MS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ManagedSpeed {
    pub target_kt: Option<f64>,
    pub is_mach: bool,
}

pub fn update_managed_speed(i: &ManagedSpeedInputs) -> ManagedSpeed {
    let mut v_pfd: Option<f64> = None;
    let mut is_mach = false;
    if i.hold_decel_reached {
        return ManagedSpeed { target_kt: i.hold_speed_target, is_mach: false };
    }
    match i.phase {
        FlightPhase::Preflight | FlightPhase::Takeoff => {
            v_pfd = i.v2_kt;
        }
        FlightPhase::Climb => {
            let mut speed = managed_climb_speed(i.cost_index);
            if let Some(l) = i.climb_speed_limit {
                if i.indicated_altitude_ft < l.under_altitude {
                    speed = speed.min(l.speed);
                }
            }
            speed = speed.min(i.speed_constraint_kt);
            if i.engine_out {
                if let Some(gd) = i.green_dot_kt.filter(|g| *g != 0.0) {
                    if i.open_or_managed_climb_active {
                        speed = match i.cas_kt.filter(|c| *c != 0.0) {
                            Some(cas) => cas - (cas - gd) * (i.fms_cycle_time_ms / 1000.0),
                            None => gd,
                        };
                    }
                }
            }
            let (v, m) = managed_targets(speed, managed_climb_speed_mach(), i.static_pressure_hpa);
            v_pfd = Some(v);
            is_mach = m;
        }
        FlightPhase::Cruise => {
            let mut speed = managed_cruise_speed();
            if let Some(l) = i.climb_speed_limit {
                if i.indicated_altitude_ft < l.under_altitude {
                    speed = speed.min(l.speed);
                }
            }
            let (v, m) = managed_targets(speed, managed_cruise_speed_mach(), i.static_pressure_hpa);
            v_pfd = Some(v);
            is_mach = m;
        }
        FlightPhase::Descent => {
            v_pfd = Some(i.vnav_managed_pfd_kt);
            is_mach = managed_targets(
                managed_descent_speed(i.cost_index, i.pilot_descent_speed_kt),
                managed_descent_speed_mach(i.pilot_descent_mach),
                i.static_pressure_hpa,
            )
            .1;
        }
        FlightPhase::Approach => {
            v_pfd = i.approach_vapp_kt;
        }
        FlightPhase::GoAround => {
            let gd = i.green_dot_kt.unwrap_or(f64::INFINITY);
            v_pfd = Some(gd.min(i.speed_constraint_kt));
        }
        FlightPhase::Done => {}
    }
    ManagedSpeed { target_kt: v_pfd, is_mach }
}

#[derive(Clone, Copy, Debug)]
pub struct SpeedMarginInputs {
    pub managed_descent_speed_kt: f64,
    pub managed_descent_speed_mach: f64,
    pub approach_speed_kt: f64,
    pub vmax_kt: f64,
    pub vls_kt: f64,
    pub green_dot_kt: f64,
    pub slat_retraction_kt: f64,
    pub flap_retraction_kt: f64,
    pub flaps_handle_index: i32,
    pub static_pressure_hpa: f64,
}

impl SpeedMarginInputs {
    fn mach_to_kias(&self, mach: f64) -> f64 {
        mach_to_kcas(mach, self.static_pressure_hpa)
    }

    fn vman(&self) -> f64 {
        match self.flaps_handle_index {
            0 => self.green_dot_kt,
            1 => self.slat_retraction_kt,
            2 => self.flap_retraction_kt,
            3 | 4 => self.approach_speed_kt,
            _ => self.vls_kt,
        }
    }

    pub fn margins(&self, current_target: f64) -> (f64, f64) {
        let max_mach_as_ias = self.mach_to_kias(MMO - 0.006);
        let is_mach_target = self.managed_descent_speed_kt - self.mach_to_kias(self.managed_descent_speed_mach) > 1.0;
        let dist_lower = 20.0;
        let dist_upper = if !is_mach_target && self.managed_descent_speed_kt - current_target > 1.0 {
            5.0
        } else {
            20.0
        };
        let vman = self.vman();
        let cap = |t: f64| self.vmax_kt.min(VMO - 3.0).min(max_mach_as_ias).min(t);
        (
            self.vls_kt.max(vman).max(cap(current_target - dist_lower)),
            self.vls_kt.max(vman).max(cap(current_target + dist_upper)),
        )
    }

    pub fn target(&self, indicated_airspeed: f64, target_speed: f64) -> f64 {
        let (lower, upper) = self.margins(target_speed);
        indicated_airspeed.min(upper).max(lower)
    }
}

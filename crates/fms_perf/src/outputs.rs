use crate::managed::ManagedSpeed;
use crate::perf::PerfSpeeds;
use crate::phase::FlightPhaseManager;
use crate::takeoff::{flex_temp_lvar_for_setting, v1_or_vr_lvar, v2_lvar, TakeoffPowerSetting, TakeoffSpeeds};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LVar {
    pub name: &'static str,
    pub unit: &'static str,
    pub value: f64,
}

pub const PERF_LVARS: &[(&str, &str)] = &[
    ("A32NX_SPEEDS_GD", "knots"),
    ("A32NX_SPEEDS_S", "knots"),
    ("A32NX_SPEEDS_F", "knots"),
    ("A32NX_SPEEDS_VS", "knots"),
    ("A32NX_SPEEDS_VLS", "knots"),
    ("A32NX_SPEEDS_VMAX", "knots"),
    ("A32NX_SPEEDS_VFEN", "knots"),
    ("A32NX_SPEEDS_VAPP", "knots"),
    ("A32NX_SPEEDS_MANAGED_PFD", "knots"),
    ("A380X_FM_LANDING_CONF3", "bool"),
    ("A32NX_FMGC_FLIGHT_PHASE", "enum"),
    ("AIRLINER_FLIGHT_PHASE", "enum"),
    ("AIRLINER_V1_SPEED", "knots"),
    ("AIRLINER_V2_SPEED", "knots"),
    ("AIRLINER_VR_SPEED", "knots"),
    ("A32NX_AIRLINER_TO_FLEX_TEMP", "number"),
    ("A32NX_FM_GROSS_WEIGHT", "number"),
    ("A32NX_PFD_SHOW_SPEED_MARGINS", "boolean"),
    ("A32NX_PFD_LOWER_SPEED_MARGIN", "knots"),
    ("A32NX_PFD_UPPER_SPEED_MARGIN", "knots"),
];

pub fn perf_speed_lvars(s: &PerfSpeeds) -> Vec<LVar> {
    vec![
        LVar { name: "A32NX_SPEEDS_GD", unit: "knots", value: s.green_dot.unwrap_or(0.0) },
        LVar { name: "A32NX_SPEEDS_S", unit: "knots", value: s.slat_retraction.unwrap_or(0.0) },
        LVar { name: "A32NX_SPEEDS_F", unit: "knots", value: s.flap_retraction.unwrap_or(0.0) },
        LVar { name: "A32NX_SPEEDS_VS", unit: "knots", value: s.vs1g },
        LVar { name: "A32NX_SPEEDS_VLS", unit: "knots", value: s.vls },
        LVar { name: "A32NX_SPEEDS_VMAX", unit: "knots", value: s.vmax },
        LVar { name: "A32NX_SPEEDS_VFEN", unit: "knots", value: s.vfe_next },
        LVar { name: "A32NX_SPEEDS_VAPP", unit: "knots", value: s.approach_vapp.unwrap_or(0.0) },
    ]
}

pub fn managed_speed_lvar(m: &ManagedSpeed) -> LVar {
    LVar { name: "A32NX_SPEEDS_MANAGED_PFD", unit: "knots", value: m.target_kt.unwrap_or(0.0) }
}

pub fn managed_speed_reset_lvar() -> LVar {
    LVar { name: "A32NX_SPEEDS_MANAGED_PFD", unit: "knots", value: -1.0 }
}

pub fn phase_lvars(m: &FlightPhaseManager) -> Vec<LVar> {
    vec![
        LVar { name: "A32NX_FMGC_FLIGHT_PHASE", unit: "enum", value: m.fmgc_flight_phase_lvar() },
        LVar { name: "AIRLINER_FLIGHT_PHASE", unit: "enum", value: m.airliner_flight_phase_lvar() },
    ]
}

pub fn takeoff_lvars(
    speeds: &TakeoffSpeeds,
    setting: TakeoffPowerSetting,
    flex: Option<f64>,
    gross_weight_kg: Option<f64>,
    landing_conf3: bool,
) -> Vec<LVar> {
    vec![
        LVar { name: "AIRLINER_V1_SPEED", unit: "knots", value: v1_or_vr_lvar(speeds.v1) },
        LVar { name: "AIRLINER_V2_SPEED", unit: "knots", value: v2_lvar(speeds.v2) },
        LVar { name: "AIRLINER_VR_SPEED", unit: "knots", value: v1_or_vr_lvar(speeds.vr) },
        LVar { name: "A32NX_AIRLINER_TO_FLEX_TEMP", unit: "number", value: flex_temp_lvar_for_setting(setting, flex) },
        LVar { name: "A32NX_FM_GROSS_WEIGHT", unit: "number", value: gross_weight_kg.unwrap_or(0.0) },
        LVar { name: "A380X_FM_LANDING_CONF3", unit: "bool", value: if landing_conf3 { 1.0 } else { 0.0 } },
    ]
}

pub fn speed_margin_lvars(show: bool, lower: f64, upper: f64) -> Vec<LVar> {
    if show {
        vec![
            LVar { name: "A32NX_PFD_SHOW_SPEED_MARGINS", unit: "boolean", value: 1.0 },
            LVar { name: "A32NX_PFD_LOWER_SPEED_MARGIN", unit: "knots", value: lower },
            LVar { name: "A32NX_PFD_UPPER_SPEED_MARGIN", unit: "knots", value: upper },
        ]
    } else {
        vec![
            LVar { name: "A32NX_PFD_SHOW_SPEED_MARGINS", unit: "boolean", value: 0.0 },
            LVar { name: "A32NX_PFD_LOWER_SPEED_MARGIN", unit: "knots", value: 0.0 },
            LVar { name: "A32NX_PFD_UPPER_SPEED_MARGIN", unit: "knots", value: 0.0 },
        ]
    }
}

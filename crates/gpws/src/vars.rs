use crate::arinc::Word;
use crate::gpws::Inputs;

pub const INPUT_VARS: [&str; 27] = [
    "L:A32NX_ADIRS_ADR_1_BARO_CORRECTED_ALTITUDE_1",
    "L:A32NX_RA_1_RADIO_ALTITUDE",
    "L:A32NX_RA_2_RADIO_ALTITUDE",
    "SIM ON GROUND",
    "L:AIRLINER_MINIMUM_DESCENT_ALTITUDE",
    "L:AIRLINER_DECISION_HEIGHT",
    "L:A32NX_FMGC_FLIGHT_PHASE",
    "L:A32NX_WXR_TAWS_SYS_SELECTED",
    "L:A32NX_GPWS_1_FAILED",
    "L:A32NX_GPWS_2_FAILED",
    "L:A32NX_GPWS_SYS_OFF",
    "L:A380X_FM_LANDING_CONF3",
    "L:A32NX_FLAPS_HANDLE_INDEX",
    "VERTICAL SPEED",
    "AIRSPEED INDICATED",
    "GEAR TOTAL PCT EXTENDED",
    "PLANE ALTITUDE",
    "L:A32NX_GPWS_FLAP_OFF",
    "L:A32NX_GPWS_FLAPS_OFF",
    "L:A32NX_GPWS_GS_OFF",
    "L:A32NX_RADIO_RECEIVER_GS_IS_VALID",
    "L:A32NX_RADIO_RECEIVER_GS_DEVIATION",
    "L:A32NX_AUTOPILOT_ACTIVE",
    "L:A32NX_AUTOTHRUST_TLA:1",
    "L:A32NX_AUTOTHRUST_TLA:2",
    "L:A32NX_AUTOTHRUST_TLA:3",
    "L:A32NX_AUTOTHRUST_TLA:4",
];

pub const F_FACTOR_VAR: &str = "L:A32NX_ENV_F_FACTOR";

pub const OUTPUT_GPWS_WARNING: &str = "L:A32NX_GPWS_Warning_Active";
pub const OUTPUT_GS_WARNING: &str = "L:A32NX_GPWS_GS_Warning_Active";
pub const OUTPUT_ALERT_1_WORD_1: &str = "L:A32NX_EGPWS_ALERT_1_DISCRETE_WORD_1";
pub const OUTPUT_ALERT_2_WORD_1: &str = "L:A32NX_EGPWS_ALERT_2_DISCRETE_WORD_1";
pub const OUTPUT_ALERT_1_WORD_2: &str = "L:A32NX_EGPWS_ALERT_1_DISCRETE_WORD_2";
pub const OUTPUT_ALERT_2_WORD_2: &str = "L:A32NX_EGPWS_ALERT_2_DISCRETE_WORD_2";

impl Inputs {
    pub fn set_var(&mut self, name: &str, v: f64) -> bool {
        let b = v != 0.;
        match name {
            "L:A32NX_ADIRS_ADR_1_BARO_CORRECTED_ALTITUDE_1" => self.baro_alt = Word::from_raw(v),
            "L:A32NX_RA_1_RADIO_ALTITUDE" => self.radio_alt_1 = Word::from_raw(v),
            "L:A32NX_RA_2_RADIO_ALTITUDE" => self.radio_alt_2 = Word::from_raw(v),
            "SIM ON GROUND" => self.on_ground = b,
            "L:AIRLINER_MINIMUM_DESCENT_ALTITUDE" => self.minimum_descent_altitude = v,
            "L:AIRLINER_DECISION_HEIGHT" => self.decision_height = v,
            "L:A32NX_FMGC_FLIGHT_PHASE" => self.flight_phase = v,
            "L:A32NX_WXR_TAWS_SYS_SELECTED" => self.taws_selected = v,
            "L:A32NX_GPWS_1_FAILED" => self.gpws_failed[0] = b,
            "L:A32NX_GPWS_2_FAILED" => self.gpws_failed[1] = b,
            "L:A32NX_GPWS_SYS_OFF" => self.gpws_sys_off = b,
            "L:A380X_FM_LANDING_CONF3" => self.landing_conf3 = b,
            "L:A32NX_FLAPS_HANDLE_INDEX" => self.flaps_handle_index = v,
            "VERTICAL SPEED" => self.vertical_speed_fpm = v,
            "AIRSPEED INDICATED" => self.airspeed_kt = v,
            "GEAR TOTAL PCT EXTENDED" => self.gear_pct_extended = v,
            "PLANE ALTITUDE" => self.plane_altitude_ft = v,
            "L:A32NX_GPWS_FLAP_OFF" => self.gpws_flap_off = b,
            "L:A32NX_GPWS_FLAPS_OFF" => self.gpws_flaps_off = b,
            "L:A32NX_GPWS_GS_OFF" => self.gpws_gs_off = b,
            "L:A32NX_RADIO_RECEIVER_GS_IS_VALID" => self.gs_valid = b,
            "L:A32NX_RADIO_RECEIVER_GS_DEVIATION" => self.gs_deviation = v,
            "L:A32NX_AUTOPILOT_ACTIVE" => self.autopilot_active = b,
            "L:A32NX_AUTOTHRUST_TLA:1" => self.tla[0] = v,
            "L:A32NX_AUTOTHRUST_TLA:2" => self.tla[1] = v,
            "L:A32NX_AUTOTHRUST_TLA:3" => self.tla[2] = v,
            "L:A32NX_AUTOTHRUST_TLA:4" => self.tla[3] = v,
            "L:A32NX_ENV_F_FACTOR" => self.f_factor = v,
            _ => return false,
        }
        true
    }
}

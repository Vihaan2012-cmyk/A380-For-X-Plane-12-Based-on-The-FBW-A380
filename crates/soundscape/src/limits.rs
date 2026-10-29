// Sources: "FBW" = FlyByWire A380X sound.xml or sound_pack.rs; everything else is GENERIC.

pub const SET_GAIN_EPS: f32 = 0.01;
pub const SET_PITCH_EPS: f32 = 0.005;

pub const END_LOW_IN: f64 = 0.005;
pub const END_LOW_OUT: f64 = 0.02;
pub const END_HIGH_OUT: f64 = 0.98;
pub const END_HIGH_IN: f64 = 0.995;

pub const CONTACTOR_HOLD_S: f64 = 0.03;
pub const CONTACTOR_GAP_S: f64 = 0.25;
pub const CONTACTOR_GAIN: f64 = 0.5;
pub const CONTACTOR_OPEN_PITCH: f64 = 0.94;
pub const AC_BUS_GAP_S: f64 = 1.0;
pub const AC_BUS_GAIN: f64 = 0.6;
pub const TR_HUM_GAIN: f64 = 0.2;

pub const EMP_ON_RPM: f64 = 3500.; // FBW epump1..4 LowerBound
pub const EMP_OFF_RPM: f64 = 3000.;
pub const EMP_NOMINAL_RPM: f64 = 7600.;
pub const EMP_GAIN_MIN: f64 = 0.25;
pub const EMP_GAIN_SPAN: f64 = 0.5;
pub const EMP_PITCH_MIN: f64 = 0.5;
pub const EMP_PITCH_MAX: f64 = 1.15;
pub const EMP_FADE_S: f32 = 0.5; // FBW FadeOutTime
pub const EDP_GAIN: f64 = 0.15;

pub const FUEL_GAIN_MIN: f64 = 0.1;
pub const FUEL_GAIN_SPAN: f64 = 0.3;

pub const FAN_UP_S: f64 = 1.5;
pub const FAN_DOWN_S: f64 = 4.0;
pub const FAN_AUDIBLE: f64 = 0.05;
pub const FAN_GAIN: f64 = 0.35;
pub const FAN_PITCH_MIN: f64 = 0.5;
pub const RECIRC_GAIN: f64 = 0.25;

pub const COCKPIT_AIR_ON_KG_S: f64 = 0.001; // FBW cockpitpacks380 LowerBound
pub const COCKPIT_AIR_OFF_KG_S: f64 = 0.0005;
pub const COCKPIT_AIR_GAIN: f64 = 0.45;
pub const COCKPIT_AIR_FADE_S: f32 = 3.0; // FBW FadeOutTime
pub const PACK_ACM_ON_KG_S: f64 = 0.005; // FBW a380pack1ext LowerBound
pub const PACK_ACM_OFF_KG_S: f64 = 0.003;
pub const PACK_ACM_GAIN: f64 = 0.6;
pub const PACK_NOMINAL_KG_S: f64 = 1.0;

pub const VALVE_GAP_S: f64 = 0.5;
pub const VALVE_GAIN: f64 = 0.4;
pub const VALVE_CLOSE_PITCH: f64 = 0.9;

pub const APU_AUDIBLE_ON_N: f64 = 1.0;
pub const APU_AUDIBLE_OFF_N: f64 = 0.5;
pub const APU_RUNNING_ON_N: f64 = 95.0;
pub const APU_RUNNING_OFF_N: f64 = 94.0;
pub const APU_START_GAIN: f64 = 0.6;
pub const APU_SHUTDOWN_GAIN: f64 = 0.6;
pub const APU_GAIN_MIN: f64 = 0.1;
pub const APU_GAIN_SPAN: f64 = 0.5;
pub const APU_PITCH_MIN: f64 = 0.25;
pub const APU_FADE_S: f32 = 2.0;
pub const APU_FLAP_RATE_EPS: f64 = 1.0;
pub const APU_FLAP_GAIN: f64 = 0.3;

pub const MOTOR_STILL_S: f64 = 0.2;
pub const GEAR_RATE_EPS: f64 = 0.01;
pub const GEAR_MOTION_GAIN: f64 = 0.7;
pub const GEAR_LOCK_GAIN: f64 = 0.8;
pub const GEAR_DOOR_GAIN: f64 = 0.5;

pub const CARGO_RATE_EPS: f64 = 0.005;
pub const CARGO_GAIN: f64 = 0.4;
pub const CARGO_LATCH_GAIN: f64 = 0.5;
pub const DOOR_GAP_S: f64 = 0.5;
pub const DOOR_GAIN: f64 = 0.7;

pub const SQUEAL_MIN_KT: f64 = 0.5;
pub const SQUEAL_MAX_KT: f64 = 12.0;
pub const SQUEAL_ON_DEMAND: f64 = 0.3;
pub const SQUEAL_OFF_DEMAND: f64 = 0.2;
pub const SQUEAL_HOLD_S: f64 = 0.3;
pub const SQUEAL_GAIN: f64 = 0.5;

pub const SURFACE_RATE_EPS: f64 = 0.002;
pub const FLAPS_STILL_S: f64 = 0.3; // FBW sound_pack.rs FLAPS_STILL_S
pub const FLAP_MOTOR_GAIN: f64 = 0.5;
pub const FLAP_STOP_GAIN: f64 = 0.5;
pub const SPOILER_RATE_EPS: f64 = 0.02;
pub const SPOILER_FULL_RATE: f64 = 1.0;
pub const SPOILER_GAIN: f64 = 0.5;

pub const WIPER_GAIN: f64 = 0.6;
pub const RAIN_ON_MM: f64 = 0.1; // FBW rain_sound LowerBound
pub const RAIN_OFF_MM: f64 = 0.05;
pub const RAIN_FULL_MM: f64 = 10.0;
pub const RAIN_FULL_TAS_KT: f64 = 250.0;
pub const RAIN_GAIN: f64 = 0.6;
pub const HAIL_ON: f64 = 0.02;
pub const HAIL_OFF: f64 = 0.01;
pub const HAIL_GAIN: f64 = 0.8;

pub const SEAT_RATE_EPS: f64 = 0.01;
pub const SEAT_STILL_S: f64 = 0.1; // FBW seatmotor FadeOutTime
pub const SEAT_GAIN: f64 = 0.4;
pub const OXY_GAIN: f64 = 0.5;
pub const CVR_GAIN: f64 = 0.4;

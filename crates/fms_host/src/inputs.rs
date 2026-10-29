use serde::{Deserialize, Serialize};

pub fn arinc_value(raw: f64) -> Option<f64> {
    let ssm = (raw / 4_294_967_296.0).floor() as u64;
    let low = (raw.rem_euclid(4_294_967_296.0)) as u32;
    (ssm == 3).then(|| f32::from_bits(low) as f64)
}

pub fn arinc_discrete(raw: f64) -> (u32, u32) {
    let ssm = (raw / 4_294_967_296.0).floor() as u32;
    let value = f32::from_bits(raw.rem_euclid(4_294_967_296.0) as u32);
    (ssm, if value.is_finite() { value as i64 as u32 } else { 0 })
}

/// One frame of aircraft state, read from the same variables FlyByWire's FMS host reads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Inputs {
    pub ready: bool,
    pub lat: f64,
    pub lon: f64,
    pub true_track: f64,
    pub true_heading: f64,
    pub mag_var_deg: Option<f64>,
    pub ground_speed_kt: f64,
    pub true_airspeed_kt: f64,
    pub zulu_time_s: f64,
    pub on_ground: bool,
    pub slew_active: bool,
    pub enable_approach_phase: bool,
    pub altitude_above_ground_ft: f64,
    pub indicated_altitude_ft: f64,
    pub pressure_altitude_ft: Option<f64>,
    pub baro_altitude_ft: Option<f64>,
    pub cas_kt: Option<f64>,
    pub static_pressure_hpa: Option<f64>,
    pub n1: [f64; 4],
    pub n2: [f64; 4],
    pub flaps_handle: i32,
    pub left_spoiler_1: f64,
    pub engine_out: bool,
    pub fuel_on_board_kg: Option<f64>,
    pub ir_discrete_words: [f64; 3],
    pub lateral_mode: i32,
    pub lateral_armed: u32,
    pub vertical_mode: i32,
    pub vertical_armed: u32,
    pub fcu_vertical_speed: f64,
    pub fcu_flight_path_angle: f64,
    pub fcu_speed: f64,
    pub fcu_speed_managed: bool,
    pub autopilot_active: bool,
    pub inertial_vertical_speed: Option<f64>,
    pub barometric_vertical_speed: Option<f64>,
    pub radio_height_ft: Option<f64>,
    pub rmp_nav_selected: bool,
    pub expedite: bool,
    pub fcu_selected_altitude_ft: f64,
    pub nd_mode: [i32; 2],
    pub nd_range: [usize; 2],
    pub efis_option: [u32; 2],
    pub fmgc_discrete_word_2: [f64; 2],
    pub thrust_limit_toga: f64,
    /// The master PRIM's FCU selected altitude, V/S, FPA and FG discrete word 3 (FBW `PrimChoiceProvider`).
    pub master_prim_fcu: [f64; 4],
    pub plane_altitude_ft: f64,
    pub ambient_temperature_c: f64,
    pub ambient_wind_direction: f64,
    pub ambient_wind_speed_kt: f64,
    pub fqms_gross_weight_kg: Option<f64>,
    pub fqms_fuel_on_board_kg: Option<f64>,
    pub fqms_cg_percent: Option<f64>,
    pub autothrust_disabled: bool,
    pub loc_receiver_valid: bool,
    pub nav_localizer_course: f64,
    pub engine_state: [f64; 2],
    pub airframe_zfw_kg: f64,
    pub airframe_zfw_cg_percent: f64,
    pub cold_and_dark_spawn: bool,
}

impl Default for Inputs {
    fn default() -> Self {
        Inputs {
            ready: true,
            lat: 0.,
            lon: 0.,
            true_track: 0.,
            true_heading: 0.,
            mag_var_deg: None,
            ground_speed_kt: 0.,
            true_airspeed_kt: 0.,
            zulu_time_s: 0.,
            on_ground: true,
            slew_active: false,
            enable_approach_phase: false,
            altitude_above_ground_ft: 0.,
            indicated_altitude_ft: 0.,
            pressure_altitude_ft: None,
            baro_altitude_ft: None,
            cas_kt: None,
            static_pressure_hpa: None,
            n1: [0.; 4],
            n2: [0.; 4],
            flaps_handle: 0,
            left_spoiler_1: 0.,
            engine_out: false,
            fuel_on_board_kg: None,
            ir_discrete_words: [0.; 3],
            lateral_mode: 0,
            lateral_armed: 0,
            vertical_mode: 0,
            vertical_armed: 0,
            fcu_vertical_speed: 0.,
            fcu_flight_path_angle: 0.,
            fcu_speed: 0.,
            fcu_speed_managed: false,
            autopilot_active: false,
            inertial_vertical_speed: None,
            barometric_vertical_speed: None,
            radio_height_ft: None,
            rmp_nav_selected: false,
            expedite: false,
            fcu_selected_altitude_ft: 0.,
            nd_mode: [3, 3],
            nd_range: [1, 1],
            efis_option: [0, 0],
            fmgc_discrete_word_2: [0., 0.],
            thrust_limit_toga: 0.,
            master_prim_fcu: [0.; 4],
            plane_altitude_ft: 0.,
            ambient_temperature_c: 15.,
            ambient_wind_direction: 0.,
            ambient_wind_speed_kt: 0.,
            fqms_gross_weight_kg: None,
            fqms_fuel_on_board_kg: None,
            fqms_cg_percent: None,
            autothrust_disabled: false,
            loc_receiver_valid: false,
            nav_localizer_course: 0.,
            engine_state: [0.; 2],
            airframe_zfw_kg: 0.,
            airframe_zfw_cg_percent: 0.,
            cold_and_dark_spawn: false,
        }
    }
}

fn first_valid(get: &mut dyn FnMut(&str) -> f64, names: &[String]) -> Option<f64> {
    names.iter().find_map(|n| arinc_value(get(n)))
}

impl Inputs {
    /// Builds the frame from a variable getter; `get` takes FBW's names ("L:..", "A:..", "E:..").
    pub fn from_vars(mut get: impl FnMut(&str) -> f64) -> Inputs {
        let g = &mut get as &mut dyn FnMut(&str) -> f64;
        let adr = |what: &str| (1..=3).map(|i| format!("L:A32NX_ADIRS_ADR_{i}_{what}")).collect::<Vec<_>>();
        let n = |g: &mut dyn FnMut(&str) -> f64, name: &str| g(name);
        let mut n1 = [0.; 4];
        let mut n2 = [0.; 4];
        for i in 0..4 {
            n1[i] = n(g, &format!("L:A32NX_ENGINE_N1:{}", i + 1));
            n2[i] = n(g, &format!("L:A32NX_ENGINE_N2:{}", i + 1));
        }
        let fob_lb = n(g, "A:FUEL TOTAL QUANTITY WEIGHT");
        Inputs {
            ready: true,
            lat: n(g, "A:PLANE LATITUDE"),
            lon: n(g, "A:PLANE LONGITUDE"),
            true_track: n(g, "A:GPS GROUND TRUE TRACK"),
            true_heading: n(g, "A:PLANE HEADING DEGREES TRUE"),
            mag_var_deg: Some(n(g, "A:MAGVAR")),
            ground_speed_kt: n(g, "A:GPS GROUND SPEED"),
            true_airspeed_kt: n(g, "A:AIRSPEED TRUE"),
            zulu_time_s: n(g, "E:ZULU TIME"),
            on_ground: n(g, "A:SIM ON GROUND") != 0.,
            slew_active: n(g, "A:IS SLEW ACTIVE") != 0.,
            enable_approach_phase: n(g, "L:A32NX_FM_ENABLE_APPROACH_PHASE") != 0.,
            altitude_above_ground_ft: n(g, "A:PLANE ALT ABOVE GROUND"),
            indicated_altitude_ft: n(g, "A:INDICATED ALTITUDE"),
            pressure_altitude_ft: first_valid(g, &adr("ALTITUDE")),
            baro_altitude_ft: first_valid(g, &adr("BAROMETRIC_CORRECTED_ALTITUDE_1")),
            cas_kt: first_valid(g, &adr("COMPUTED_AIRSPEED")),
            static_pressure_hpa: first_valid(g, &adr("CORRECTED_AVERAGE_STATIC_PRESSURE")),
            n1,
            n2,
            flaps_handle: n(g, "L:A32NX_FLAPS_HANDLE_INDEX") as i32,
            left_spoiler_1: n(g, "L:A32NX_LEFT_SPOILER_1_COMMANDED_POSITION"),
            engine_out: false,
            fuel_on_board_kg: (fob_lb > 0.).then_some(fob_lb * 0.453592),
            ir_discrete_words: [1, 2, 3].map(|i| n(g, &format!("L:A32NX_ADIRS_IR_{i}_MAINT_WORD"))),
            lateral_mode: n(g, "L:A32NX_FMA_LATERAL_MODE") as i32,
            lateral_armed: n(g, "L:A32NX_FMA_LATERAL_ARMED") as u32,
            vertical_mode: n(g, "L:A32NX_FMA_VERTICAL_MODE") as i32,
            vertical_armed: n(g, "L:A32NX_FMA_VERTICAL_ARMED") as u32,
            fcu_vertical_speed: n(g, "L:A32NX_AUTOPILOT_VS_SELECTED"),
            fcu_flight_path_angle: n(g, "L:A32NX_AUTOPILOT_FPA_SELECTED"),
            fcu_speed: n(g, "L:A32NX_AUTOPILOT_SPEED_SELECTED"),
            thrust_limit_toga: n(g, "L:A32NX_AUTOTHRUST_THRUST_LIMIT_TOGA"),
            fcu_speed_managed: n(g, "L:A32NX_FCU_SPD_MANAGED_DOT") != 0.,
            autopilot_active: n(g, "L:A32NX_AUTOPILOT_ACTIVE") != 0.,
            inertial_vertical_speed: arinc_value(n(g, "L:A32NX_ADIRS_IR_1_VERTICAL_SPEED")),
            barometric_vertical_speed: arinc_value(n(g, "L:A32NX_ADIRS_ADR_1_BAROMETRIC_VERTICAL_SPEED")),
            radio_height_ft: first_valid(g, &["L:A32NX_RA_1_RADIO_ALTITUDE".to_string(), "L:A32NX_RA_2_RADIO_ALTITUDE".to_string(), "L:A32NX_RA_3_RADIO_ALTITUDE".to_string()]),
            rmp_nav_selected: n(g, "L:A32NX_RMP_L_NAV_BUTTON_SELECTED") != 0. || n(g, "L:A32NX_RMP_R_NAV_BUTTON_SELECTED") != 0.,
            expedite: n(g, "L:A32NX_FMA_EXPEDITE_MODE") != 0.,
            fcu_selected_altitude_ft: n(g, "A:AUTOPILOT ALTITUDE LOCK VAR:3"),
            nd_mode: ["L", "R"].map(|s| n(g, &format!("L:A32NX_EFIS_{s}_ND_MODE")) as i32),
            nd_range: ["L", "R"].map(|s| n(g, &format!("L:A32NX_EFIS_{s}_ND_RANGE")).max(0.) as usize),
            efis_option: ["L", "R"].map(|s| n(g, &format!("L:A32NX_EFIS_{s}_OPTION")).max(0.) as u32),
            fmgc_discrete_word_2: [n(g, "L:A32NX_FMGC_1_DISCRETE_WORD_2"), n(g, "L:A32NX_FMGC_1_DISCRETE_WORD_2")],
            master_prim_fcu: {
                let master = (1..=3).find(|i| {
                    let (ssm, bits) = arinc_discrete(n(g, &format!("L:A32NX_PRIM_{i}_FCTL_LAW_STATUS_WORD")));
                    ssm == 3 && bits & (1 << 20) != 0
                }).unwrap_or(1);
                ["SELECTED_ALTITUDE", "SELECTED_VERTICAL_SPEED", "SELECTED_FPA", "FG_DISCRETE_WORD_3"].map(|w| n(g, &format!("L:A32NX_PRIM_{master}_{w}")))
            },
            plane_altitude_ft: n(g, "A:PLANE ALTITUDE"),
            ambient_temperature_c: n(g, "A:AMBIENT TEMPERATURE"),
            ambient_wind_direction: n(g, "A:AMBIENT WIND DIRECTION"),
            ambient_wind_speed_kt: n(g, "A:AMBIENT WIND VELOCITY"),
            fqms_gross_weight_kg: arinc_value(n(g, "L:A32NX_FQMS_GROSS_WEIGHT")),
            fqms_fuel_on_board_kg: arinc_value(n(g, "L:A32NX_FQMS_TOTAL_FUEL_ON_BOARD")),
            fqms_cg_percent: arinc_value(n(g, "L:A32NX_FQMS_CENTER_OF_GRAVITY_MAC")),
            autothrust_disabled: n(g, "L:A32NX_AUTOTHRUST_DISABLED") == 1.,
            loc_receiver_valid: n(g, "L:A32NX_RADIO_RECEIVER_LOC_IS_VALID") == 1.,
            nav_localizer_course: n(g, "A:NAV LOCALIZER:3"),
            engine_state: [n(g, "L:A32NX_ENGINE_STATE:2"), n(g, "L:A32NX_ENGINE_STATE:3")],
            airframe_zfw_kg: n(g, "L:A32NX_AIRFRAME_ZFW"),
            airframe_zfw_cg_percent: n(g, "L:A32NX_AIRFRAME_ZFW_CG_PERCENT_MAC"),
            cold_and_dark_spawn: n(g, "L:A32NX_COLD_AND_DARK_SPAWN") != 0.,
        }
    }

    /// A bit (1-based) of the master PRIM's FG discrete word 3, false when the word is not normal.
    pub fn word3_bit(&self, bit: u32) -> bool {
        let (ssm, bits) = arinc_discrete(self.master_prim_fcu[3]);
        ssm == 3 && bits & (1 << (bit - 1)) != 0
    }

    pub fn prim_selected_altitude(&self) -> Option<f64> {
        arinc_value(self.master_prim_fcu[0])
    }

    /// FBW `FmgcDataService.getFOB` source when an engine runs: the FQMS, falling back to the fuel gauge.
    pub fn fuel_on_board_kg_fqms(&self) -> Option<f64> {
        self.fqms_fuel_on_board_kg.or(self.fuel_on_board_kg)
    }

    pub fn isa_deviation(&self) -> f64 {
        self.ambient_temperature_c - (15. - 0.0019812 * self.indicated_altitude_ft)
    }

    pub fn gps_irs_available(&self) -> bool {
        self.ir_discrete_words.iter().any(|w| {
            let (ssm, bits) = arinc_discrete(*w);
            ssm >= 2 && bits & 4 != 0 && bits & 1 == 0
        })
    }

    pub fn nav_mode_engaged(&self) -> bool {
        let bit12 = |w: f64| {
            let (ssm, bits) = arinc_discrete(w);
            (ssm == 3).then_some(bits & (1 << 11) != 0)
        };
        bit12(self.fmgc_discrete_word_2[0]).or_else(|| bit12(self.fmgc_discrete_word_2[1])).unwrap_or(self.lateral_mode == crate::modes::LAT_NAV)
    }
}

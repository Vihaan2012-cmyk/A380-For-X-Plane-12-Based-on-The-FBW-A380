use crate::arinc::Word;

pub const ND_RANGE_NM: [i32; 8] = [-1, 10, 20, 40, 80, 160, 320, 640];

pub mod nd_mode {
    pub const ROSE_ILS: i32 = 0;
    pub const ROSE_VOR: i32 = 1;
    pub const ROSE_NAV: i32 = 2;
    pub const ARC: i32 = 3;
    pub const PLAN: i32 = 4;
}

pub mod lateral_mode {
    pub const NONE: i32 = 0;
    pub const HDG: i32 = 10;
    pub const TRACK: i32 = 11;
    pub const RWY: i32 = 40;
    pub const RWY_TRACK: i32 = 41;
    pub const GA_TRACK: i32 = 50;
}

const ARMED_LATERAL_NAV_BIT: u32 = 0;

#[derive(Clone, Copy, Debug)]
pub struct EfisSideInputs {
    pub nd_range_index: usize,
    pub nd_mode: i32,
    pub overlay: i32,
    pub vd_range_lower: f64,
    pub vd_range_upper: f64,
}

impl Default for EfisSideInputs {
    fn default() -> Self {
        EfisSideInputs { nd_range_index: 0, nd_mode: 0, overlay: 0, vd_range_lower: 0., vd_range_upper: 0. }
    }
}

#[derive(Clone, Debug)]
pub struct BridgeInputs {
    pub terr_failure: [bool; 2],
    pub gpws_failure: [bool; 2],
    pub aesu_reset_pulled: [bool; 2],
    pub ac_ess_powered: bool,
    pub ac_4_powered: bool,
    pub dc_ess_in_flight_powered: bool,
    pub dc_1_powered: bool,
    pub dc_2_powered: bool,
    pub taws_wxr_selected: f64,
    pub att_hdg_switching: f64,
    pub ir_heading: [Word; 3],
    pub ir_track: [Word; 3],
    pub ir_latitude: [Word; 3],
    pub ir_maint: [Word; 3],
    pub efis: [EfisSideInputs; 2],
    pub latitude: Word,
    pub longitude: Word,
    pub altitude: Word,
    pub heading: Word,
    pub vertical_speed: Word,
    pub destination_latitude: Word,
    pub destination_longitude: Word,
    pub active_lateral_mode: i32,
    pub armed_lateral_bitmask: i32,
}

impl Default for BridgeInputs {
    fn default() -> Self {
        BridgeInputs {
            terr_failure: [false; 2],
            gpws_failure: [false; 2],
            aesu_reset_pulled: [false; 2],
            ac_ess_powered: false,
            ac_4_powered: false,
            dc_ess_in_flight_powered: false,
            dc_1_powered: false,
            dc_2_powered: false,
            taws_wxr_selected: 0.,
            att_hdg_switching: 1.,
            ir_heading: [Word::failed(); 3],
            ir_track: [Word::failed(); 3],
            ir_latitude: [Word::failed(); 3],
            ir_maint: [Word::failed(); 3],
            efis: [EfisSideInputs::default(); 2],
            latitude: Word::failed(),
            longitude: Word::failed(),
            altitude: Word::failed(),
            heading: Word::failed(),
            vertical_speed: Word::failed(),
            destination_latitude: Word::failed(),
            destination_longitude: Word::failed(),
            active_lateral_mode: lateral_mode::NONE,
            armed_lateral_bitmask: 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TawsEfisData {
    pub nd_range: i32,
    pub arc_mode: bool,
    pub terr_on_nd: bool,
    pub terr_on_vd: bool,
    pub efis_mode: i32,
    pub vd_range_lower: f64,
    pub vd_range_upper: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BridgeOutputs {
    pub terr_failed: [bool; 2],
    pub gpws_failed: [bool; 2],
    pub wxr_failed: [bool; 2],
    pub terr_failed_selected: bool,
    pub efis: [TawsEfisData; 2],
    pub adiru_data_valid: bool,
    pub runway_data_valid: bool,
    pub manual_azim_enabled: bool,
}

pub fn update(inputs: &BridgeInputs) -> BridgeOutputs {
    let valid_ir_maint = inputs.ir_maint.iter().find(|w| w.is_normal_operation());
    let extreme_latitude = valid_ir_maint.map_or(false, |w| w.bit_value_or(15, false));

    let terr_failed = [
        inputs.terr_failure[0] || inputs.aesu_reset_pulled[0] || !inputs.ac_ess_powered || extreme_latitude,
        inputs.terr_failure[1] || inputs.aesu_reset_pulled[1] || !inputs.ac_4_powered || extreme_latitude,
    ];
    let gpws_failed = [
        inputs.gpws_failure[0] || inputs.aesu_reset_pulled[0] || !inputs.ac_ess_powered,
        inputs.gpws_failure[1] || inputs.aesu_reset_pulled[1] || !inputs.ac_4_powered,
    ];

    let terr_failed_selected = if inputs.taws_wxr_selected == 1. {
        terr_failed[0]
    } else if inputs.taws_wxr_selected == 2. {
        terr_failed[1]
    } else {
        true
    };

    let own = inputs.att_hdg_switching == 1.;
    let pick = |a: usize, b: usize, words: &[Word; 3]| if own { words[a] } else { words[b] };

    let capt_vd_avail = pick(0, 2, &inputs.ir_heading).is_normal_operation()
        && pick(0, 2, &inputs.ir_track).is_normal_operation()
        && (inputs.dc_ess_in_flight_powered || inputs.dc_1_powered);
    let capt_nd_avail = pick(0, 2, &inputs.ir_heading).is_normal_operation()
        && pick(0, 2, &inputs.ir_latitude).is_normal_operation()
        && (inputs.dc_ess_in_flight_powered || inputs.dc_1_powered);
    let fo_vd_avail = pick(1, 2, &inputs.ir_heading).is_normal_operation()
        && pick(1, 2, &inputs.ir_track).is_normal_operation()
        && (inputs.dc_1_powered || inputs.dc_2_powered);
    let fo_nd_avail = pick(1, 2, &inputs.ir_heading).is_normal_operation()
        && pick(1, 2, &inputs.ir_latitude).is_normal_operation()
        && (inputs.dc_1_powered || inputs.dc_2_powered);

    let side = |e: &EfisSideInputs, nd_avail: bool, vd_avail: bool| TawsEfisData {
        nd_range: ND_RANGE_NM.get(e.nd_range_index).copied().unwrap_or(0).max(0),
        arc_mode: e.nd_mode == nd_mode::ARC,
        terr_on_nd: e.nd_mode != nd_mode::PLAN && e.overlay == 2 && !terr_failed_selected && nd_avail,
        terr_on_vd: (e.nd_mode == nd_mode::ARC || e.nd_mode == nd_mode::ROSE_NAV) && !terr_failed_selected && vd_avail,
        efis_mode: e.nd_mode,
        vd_range_lower: e.vd_range_lower,
        vd_range_upper: e.vd_range_upper,
    };

    let active = inputs.active_lateral_mode;
    let manual_azim_enabled = (active == lateral_mode::NONE
        || active == lateral_mode::HDG
        || active == lateral_mode::TRACK
        || active == lateral_mode::RWY
        || active == lateral_mode::RWY_TRACK
        || active == lateral_mode::GA_TRACK)
        && (inputs.armed_lateral_bitmask >> ARMED_LATERAL_NAV_BIT) & 1 != 1;

    BridgeOutputs {
        terr_failed,
        gpws_failed,
        wxr_failed: [true, true],
        terr_failed_selected,
        efis: [side(&inputs.efis[0], capt_nd_avail, capt_vd_avail), side(&inputs.efis[1], fo_nd_avail, fo_vd_avail)],
        adiru_data_valid: inputs.longitude.is_normal_operation()
            && inputs.latitude.is_normal_operation()
            && inputs.altitude.is_normal_operation()
            && inputs.heading.is_normal_operation()
            && inputs.vertical_speed.is_normal_operation(),
        runway_data_valid: inputs.destination_latitude.is_normal_operation()
            && inputs.destination_longitude.is_normal_operation(),
        manual_azim_enabled,
    }
}

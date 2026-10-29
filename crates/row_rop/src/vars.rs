pub const ROW_ROP_WORD_1: &str = "L:A32NX_ROW_ROP_WORD_1";
pub const ROW_ROP_LOST: &str = "L:A32NX_ROW_ROP_LOST";
pub const BTV_REMAINING_DIST_TO_RWY_END: &str = "L:A32NX_OANS_BTV_REMAINING_DIST_TO_RWY_END";
pub const WET_DISTANCE_ESTIMATED: &str = "L:A32NX_OANS_BTV_WET_DISTANCE_ESTIMATED";
pub const DRY_DISTANCE_ESTIMATED: &str = "L:A32NX_OANS_BTV_DRY_DISTANCE_ESTIMATED";
pub const STOP_BAR_DISTANCE_ESTIMATED: &str = "L:A32NX_OANS_BTV_STOP_BAR_DISTANCE_ESTIMATED";
pub const AUDIO_ROP_MAX_BRAKING: &str = "L:A32NX_AUDIO_ROP_MAX_BRAKING";
pub const AUDIO_ROW_SET_MAX_REVERSE: &str = "L:A32NX_AUDIO_ROW_SET_MAX_REVERSE";
pub const AUDIO_ROP_KEEP_MAX_REVERSE: &str = "L:A32NX_AUDIO_ROP_KEEP_MAX_REVERSE";
pub const AUDIO_ROW_RWY_TOO_SHORT: &str = "L:A32NX_AUDIO_ROW_RWY_TOO_SHORT";

pub const BIT_MAX_BRAKING: u8 = 11;
pub const BIT_SET_MAX_REVERSE: u8 = 12;
pub const BIT_KEEP_MAX_REVERSE: u8 = 13;
pub const BIT_IF_WET_TOO_SHORT: u8 = 14;
pub const BIT_TOO_SHORT: u8 = 15;

pub const SSM_FAILURE_WARNING: u64 = 0;
pub const SSM_NORMAL_OPERATION: u64 = 3;

pub fn bit(n: u8) -> u32 {
    1u32 << (n - 1)
}

pub fn word_raw(word: u32, valid: bool) -> f64 {
    let ssm = if valid { SSM_NORMAL_OPERATION } else { SSM_FAILURE_WARNING };
    f64::from(word) + (ssm << 32) as f64
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Availability {
    pub ir_available: u8,
    pub adr_available: u8,
    pub ra_available: u8,
    pub fws_audio_available: u8,
    pub abn_proc_impacting_ldg_perf: bool,
    pub yellow_hyd_lost: bool,
    pub green_hyd_lost: bool,
    pub dc_ess_failed: bool,
    pub dc2_failed: bool,
    pub ac2_failed: bool,
    pub oans_failed: bool,
    pub oans_ppos_lost: bool,
}

pub fn row_rop_lost(a: &Availability) -> (bool, bool) {
    let common = a.ir_available < 2 || a.adr_available < 2 || a.ra_available < 1 || a.fws_audio_available == 0;
    let perf = a.abn_proc_impacting_ldg_perf;
    let dist = a.yellow_hyd_lost || a.green_hyd_lost || a.dc_ess_failed || a.dc2_failed || a.ac2_failed;
    let row = common || perf || dist || a.oans_failed;
    let rop = row || a.oans_ppos_lost;
    (row, rop)
}

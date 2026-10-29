pub const VMO: f64 = 340.0;
pub const MMO: f64 = 0.89;

pub const VFE_F1: f64 = 263.0;
pub const VFE_F1F: f64 = 222.0;
pub const VFE_F2: f64 = 220.0;
pub const VFE_F3: f64 = 196.0;
pub const VFE_FF: f64 = 182.0;

// FCOM LIM-13; FBW upstream 123 / 147, see docs/fbw-differences.md
pub const VMCL: f64 = 120.0;
pub const VMCL2: f64 = 145.0;

pub const VLE: f64 = 250.0;
pub const MLE: f64 = 0.55;
pub const VLO: f64 = 250.0;
pub const MLO: f64 = 0.55;

pub const MAX_CERTIFIED_ALT: f64 = 43_100.0;
pub const DEFAULT_TROPOPAUSE_ALT: f64 = 36_090.0;

pub const MIN_ZFW_CG: f64 = 23.0;
pub const MAX_ZFW_CG: f64 = 48.0;
pub const MIN_ZFW: f64 = 270_000.0;
pub const MIN_GW: f64 = 270_000.0;

pub const DEFAULT_MAX_ZFW: f64 = 373_000.0;
pub const DEFAULT_MAX_GW: f64 = 512_000.0;
pub const DEFAULT_MAX_TOW: f64 = 510_000.0;
pub const DEFAULT_MAX_LW: f64 = 395_000.0;

pub const FMS_CYCLE_TIME_MS: f64 = 250.0;

pub const LOWEST_FUEL_ESTIMATE_LB: f64 = -77_161.8;
pub const POUND_TO_KG: f64 = 0.453_592_37;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightLimits {
    pub max_zfw: f64,
    pub max_gw: f64,
    pub max_tow: f64,
    pub max_lw: f64,
}

impl Default for WeightLimits {
    fn default() -> Self {
        WeightLimits {
            max_zfw: DEFAULT_MAX_ZFW,
            max_gw: DEFAULT_MAX_GW,
            max_tow: DEFAULT_MAX_TOW,
            max_lw: DEFAULT_MAX_LW,
        }
    }
}

impl WeightLimits {
    pub fn with_variant(mzfw: f64, mrw: f64, mtow: f64, mlw: f64) -> Self {
        let d = WeightLimits::default();
        let pick = |v: f64, f: f64| if v > 0.0 { v } else { f };
        WeightLimits {
            max_zfw: pick(mzfw, d.max_zfw),
            max_gw: pick(mrw, d.max_gw),
            max_tow: pick(mtow, d.max_tow),
            max_lw: pick(mlw, d.max_lw),
        }
    }
}

use crate::constants::VMO;
use crate::speeds::SpeedsUtils;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EntryError {
    NotAllowed,
    OutOfRange { min: f64, max: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TakeoffPowerSetting {
    Toga = 0,
    Flex = 1,
    Derated = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TakeoffDerated {
    D01 = 0,
    D02 = 1,
    D03 = 2,
    D04 = 3,
    D05 = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CostIndexMode {
    Lrc = 0,
    Econ = 1,
}

pub const V_SPEED_MIN: f64 = 90.0;
pub const FLEX_TEMP_MIN: f64 = 0.0;
pub const FLEX_TEMP_MAX: f64 = 99.0;
pub const COST_INDEX_MAX: f64 = 999.0;

fn in_range(v: f64, min: f64, max: f64) -> Result<f64, EntryError> {
    if v.is_nan() {
        return Err(EntryError::NotAllowed);
    }
    if v <= max && v >= min {
        Ok(v)
    } else {
        Err(EntryError::OutOfRange { min, max })
    }
}

pub fn parse_v_speed(v: f64) -> Result<f64, EntryError> {
    in_range(v, V_SPEED_MIN, VMO)
}

pub fn parse_flex_temperature(v: f64) -> Result<f64, EntryError> {
    in_range(v, FLEX_TEMP_MIN, FLEX_TEMP_MAX)
}

pub fn parse_cost_index(v: f64) -> Result<f64, EntryError> {
    in_range(v, 0.0, COST_INDEX_MAX)
}

pub fn flex_temp_lvar(flex: Option<f64>) -> f64 {
    match flex {
        Some(0.0) => 0.1,
        Some(f) => f,
        None => 0.0,
    }
}

pub fn flex_temp_lvar_for_setting(setting: TakeoffPowerSetting, flex: Option<f64>) -> f64 {
    match setting {
        TakeoffPowerSetting::Flex => flex_temp_lvar(flex),
        _ => 0.0,
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TakeoffSpeeds {
    pub v1: Option<f64>,
    pub vr: Option<f64>,
    pub v2: Option<f64>,
}

impl TakeoffSpeeds {
    pub fn not_inserted(&self) -> bool {
        !self.v1.map_or(false, |v| v != 0.0) || !self.vr.map_or(false, |v| v != 0.0) || !self.v2.map_or(false, |v| v != 0.0)
    }

    pub fn valid(&self) -> bool {
        let truthy = |v: Option<f64>| v.filter(|x| *x != 0.0);
        let (v1, vr, v2) = (truthy(self.v1), truthy(self.vr), truthy(self.v2));
        (match (v1, vr) {
            (Some(a), Some(b)) => a <= b,
            _ => true,
        }) && (match (vr, v2) {
            (Some(a), Some(b)) => a <= b,
            _ => true,
        }) && (match (v1, v2) {
            (Some(a), Some(b)) => a <= b,
            _ => true,
        })
    }

    pub fn too_low(&self, flaps_set: bool, gross_weight_known: bool, pressure_altitude_ft: Option<f64>) -> bool {
        if !flaps_set || !gross_weight_known {
            return false;
        }
        let Some(zp) = pressure_altitude_ft else { return false };
        self.v1.unwrap_or(f64::INFINITY) < SpeedsUtils::vmcg(zp).trunc()
            || self.vr.unwrap_or(f64::INFINITY) < (1.05 * SpeedsUtils::vmca(zp)).trunc()
            || self.v2.unwrap_or(f64::INFINITY) < (1.1 * SpeedsUtils::vmca(zp)).trunc()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToSpeedsChecks {
    pub v_speed_disagree: bool,
    pub to_speeds_too_low: bool,
    pub to_speeds_not_inserted: bool,
}

impl ToSpeedsChecks {
    pub fn evaluate(speeds: &TakeoffSpeeds) -> ToSpeedsChecks {
        ToSpeedsChecks {
            v_speed_disagree: !speeds.valid(),
            to_speeds_too_low: false,
            to_speeds_not_inserted: speeds.not_inserted(),
        }
    }

    pub fn discrete_word_3_bits(&self, word: u32) -> u32 {
        let mut w = set_bit(word, 16, self.v_speed_disagree);
        w = set_bit(w, 17, self.to_speeds_too_low);
        set_bit(w, 18, self.to_speeds_not_inserted)
    }
}

pub fn set_bit(word: u32, bit: u32, value: bool) -> u32 {
    let mask = 1u32 << (bit - 1);
    if value {
        word | mask
    } else {
        word & !mask
    }
}

pub fn takeoff_flaps_discrete_word_2(word: u32, flaps: Option<u8>) -> u32 {
    let mut w = word;
    for (bit, conf) in [(13, 0u8), (14, 1), (15, 2), (16, 3)] {
        w = set_bit(w, bit, flaps == Some(conf));
    }
    w
}

pub fn v2_lvar(v2: Option<f64>) -> f64 {
    v2.unwrap_or(0.0)
}

pub fn v1_or_vr_lvar(v: Option<f64>) -> f64 {
    v.unwrap_or(-1.0)
}

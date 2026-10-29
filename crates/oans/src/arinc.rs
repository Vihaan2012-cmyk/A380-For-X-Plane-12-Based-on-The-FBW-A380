#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ssm {
    FailureWarning = 0,
    NoComputedData = 1,
    FunctionalTest = 2,
    NormalOperation = 3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arinc {
    pub value: f64,
    pub ssm: Ssm,
}

impl Default for Arinc {
    fn default() -> Self {
        Self { value: 0.0, ssm: Ssm::FailureWarning }
    }
}

impl Arinc {
    pub fn normal(value: f64) -> Self {
        Self { value, ssm: Ssm::NormalOperation }
    }

    pub fn from_raw(raw: f64) -> Self {
        let bits = (raw % 4294967296.0) as u32;
        let ssm = match ((raw / 4294967296.0).trunc() as i64) & 3 {
            0 => Ssm::FailureWarning,
            1 => Ssm::NoComputedData,
            2 => Ssm::FunctionalTest,
            _ => Ssm::NormalOperation,
        };
        Self { value: f32::from_bits(bits) as f64, ssm }
    }

    pub fn raw(&self) -> f64 {
        (self.value as f32).to_bits() as f64 + (self.ssm as u8 as f64) * 4294967296.0
    }

    pub fn is_normal(&self) -> bool {
        self.ssm == Ssm::NormalOperation
    }

    pub fn is_invalid(&self) -> bool {
        self.ssm != Ssm::NormalOperation && self.ssm != Ssm::FunctionalTest
    }

    pub fn value_or(&self, default: f64) -> f64 {
        if self.is_invalid() {
            default
        } else {
            self.value
        }
    }

    pub fn bit_value(&self, bit: u32) -> bool {
        (((self.value as i64 as i32) >> (bit - 1)) & 1) != 0
    }

    pub fn bit_value_or(&self, bit: u32, default: bool) -> bool {
        if self.is_invalid() {
            default
        } else {
            self.bit_value(bit)
        }
    }

    pub fn set_bit(&mut self, bit: u32, on: bool) {
        let mut v = self.value as i64 as i32;
        if on {
            v |= 1 << (bit - 1);
        } else {
            v &= !(1 << (bit - 1));
        }
        self.value = v as f64;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ssm {
    FailureWarning = 0,
    NoComputedData = 1,
    FunctionalTest = 2,
    NormalOperation = 3,
}

impl Ssm {
    pub fn from_bits(bits: u8) -> Ssm {
        match bits & 3 {
            0 => Ssm::FailureWarning,
            1 => Ssm::NoComputedData,
            2 => Ssm::FunctionalTest,
            _ => Ssm::NormalOperation,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Word {
    pub value: f32,
    pub ssm: Ssm,
}

impl Word {
    pub const fn new(value: f32, ssm: Ssm) -> Word {
        Word { value, ssm }
    }

    pub const fn failed() -> Word {
        Word { value: 0., ssm: Ssm::FailureWarning }
    }

    pub fn normal(value: f32) -> Word {
        Word { value, ssm: Ssm::NormalOperation }
    }

    pub fn from_raw(raw: f64) -> Word {
        let bits = (raw % 4_294_967_296.0) as u64 as u32;
        let ssm = Ssm::from_bits(((raw / 4_294_967_296.0).trunc() as i64 & 3) as u8);
        Word { value: f32::from_bits(bits), ssm }
    }

    pub fn to_raw(self) -> f64 {
        f64::from(self.value.to_bits()) + f64::from(self.ssm as u8) * 4_294_967_296.0
    }

    pub fn is_failure_warning(&self) -> bool {
        self.ssm == Ssm::FailureWarning
    }

    pub fn is_no_computed_data(&self) -> bool {
        self.ssm == Ssm::NoComputedData
    }

    pub fn is_functional_test(&self) -> bool {
        self.ssm == Ssm::FunctionalTest
    }

    pub fn is_normal_operation(&self) -> bool {
        self.ssm == Ssm::NormalOperation
    }

    pub fn bit_value(&self, bit: u32) -> bool {
        ((self.value as i32) >> (bit - 1)) & 1 != 0
    }

    pub fn bit_value_or(&self, bit: u32, default: bool) -> bool {
        if self.is_normal_operation() || self.is_functional_test() {
            self.bit_value(bit)
        } else {
            default
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DiscreteWord {
    bits: i32,
    pub ssm: Ssm,
}

impl DiscreteWord {
    pub const fn empty() -> DiscreteWord {
        DiscreteWord { bits: 0, ssm: Ssm::FailureWarning }
    }

    pub fn set_bit(&mut self, bit: u32, value: bool) {
        if value {
            self.bits |= 1 << (bit - 1);
        } else {
            self.bits &= !(1 << (bit - 1));
        }
    }

    pub fn bits(&self) -> i32 {
        self.bits
    }

    pub fn raw(&self) -> f64 {
        Word::new(self.bits as f32, self.ssm).to_raw()
    }
}

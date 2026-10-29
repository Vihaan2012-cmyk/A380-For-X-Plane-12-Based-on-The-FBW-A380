#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ssm {
    FailureWarning,
    NoComputedData,
    FunctionalTest,
    NormalOperation,
}

pub fn to_int32(v: f64) -> i32 {
    if !v.is_finite() {
        return 0;
    }
    (v.trunc().rem_euclid(4_294_967_296.0) as u32) as i32
}

#[derive(Debug, Clone, Copy)]
pub struct Word {
    pub ssm: Ssm,
    pub raw: u32,
}

impl Word {
    pub fn empty() -> Self {
        Self { ssm: Ssm::FailureWarning, raw: 0 }
    }

    pub fn from_simvar(v: f64) -> Self {
        let raw = (v.trunc() as i64 & 0xffff_ffff) as u32;
        let ssm = match ((v / 4_294_967_296.0).trunc() as i64) & 0b11 {
            0 => Ssm::FailureWarning,
            1 => Ssm::NoComputedData,
            2 => Ssm::FunctionalTest,
            _ => Ssm::NormalOperation,
        };
        Self { ssm, raw }
    }

    pub fn value(&self) -> f64 {
        f32::from_bits(self.raw) as f64
    }

    pub fn is_failure_warning(&self) -> bool {
        self.ssm == Ssm::FailureWarning
    }

    pub fn is_no_computed_data(&self) -> bool {
        self.ssm == Ssm::NoComputedData
    }

    pub fn is_normal(&self) -> bool {
        self.ssm == Ssm::NormalOperation
    }

    pub fn is_functional_test(&self) -> bool {
        self.ssm == Ssm::FunctionalTest
    }

    pub fn value_or(&self, default: f64) -> f64 {
        if self.is_normal() || self.is_functional_test() {
            self.value()
        } else {
            default
        }
    }

    pub fn bit_value(&self, bit: u32) -> bool {
        (to_int32(self.value()) >> ((bit - 1) & 31)) & 1 != 0
    }

    pub fn is_invalid(&self) -> bool {
        self.is_failure_warning() || self.is_no_computed_data()
    }

    pub fn set_value(&mut self, v: f64) {
        self.raw = (v as f32).to_bits();
    }

    pub fn set_bit_value(&mut self, bit: u32, on: bool) {
        let mask = 1i64 << ((bit - 1) & 31);
        let cur = to_int32(self.value()) as i64;
        let next = if on { cur | mask } else { cur & !mask };
        self.set_value(((next as u32) as i32) as f64);
    }

    pub fn to_simvar_string(&self) -> String {
        let ssm = match self.ssm {
            Ssm::FailureWarning => 0u64,
            Ssm::NoComputedData => 1,
            Ssm::FunctionalTest => 2,
            Ssm::NormalOperation => 3,
        };
        format!("{}", self.raw as u64 + ssm * 4_294_967_296)
    }

    pub fn bit_value_or(&self, bit: u32, default: bool) -> bool {
        if self.is_normal() || self.is_functional_test() {
            (to_int32(self.value()) >> ((bit - 1) & 31)) & 1 != 0
        } else {
            default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_ssm_and_value() {
        let w = Word::from_simvar(5_469_372_416.0);
        assert_eq!(w.ssm, Ssm::NoComputedData);
        let raw = (1500f32).to_bits() as f64 + 3. * 4_294_967_296.0;
        let w = Word::from_simvar(raw);
        assert!(w.is_normal());
        assert_eq!(w.value(), 1500.);
        assert_eq!(Word::from_simvar(0.).ssm, Ssm::FailureWarning);
        let bits = (1u32 << 26) as f32;
        let w = Word::from_simvar(bits.to_bits() as f64 + 3. * 4_294_967_296.0);
        assert!(w.bit_value_or(27, false));
        assert!(!w.bit_value_or(26, false));
    }
}

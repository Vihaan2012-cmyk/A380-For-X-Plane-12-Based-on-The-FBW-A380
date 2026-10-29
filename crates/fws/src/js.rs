use serde_json::Value;

/// A JavaScript value as FBW's FWS tables produce it: booleans, numbers read straight from simvars, ids, null/undefined.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum J {
    B(bool),
    N(f64),
    S(&'static str),
    Null,
    Undef,
}

impl J {
    pub fn truthy(&self) -> bool {
        match *self {
            J::B(b) => b,
            J::N(n) => n != 0.0 && !n.is_nan(),
            J::S(s) => !s.is_empty(),
            J::Null | J::Undef => false,
        }
    }

    pub fn num(&self) -> f64 {
        match *self {
            J::B(b) => b as u8 as f64,
            J::N(n) => n,
            J::S(s) => s.trim().parse().unwrap_or(f64::NAN),
            J::Null => 0.0,
            J::Undef => f64::NAN,
        }
    }

    pub fn opt(v: Option<f64>) -> J {
        v.map_or(J::Null, J::N)
    }

    pub fn opt_s(v: Option<&'static str>) -> J {
        v.map_or(J::Null, J::S)
    }

    pub fn strict_eq(&self, o: &J) -> bool {
        match (self, o) {
            (J::N(a), J::N(b)) => a == b,
            _ => self == o,
        }
    }

    pub fn to_json(&self) -> Value {
        match *self {
            J::B(b) => Value::Bool(b),
            J::N(n) => num_json(n),
            J::S(s) => Value::String(s.to_string()),
            J::Null | J::Undef => Value::Null,
        }
    }

    pub fn as_str(&self) -> Option<&'static str> {
        match *self {
            J::S(s) => Some(s),
            _ => None,
        }
    }
}

pub fn num_json(n: f64) -> Value {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 9.0e15 {
        Value::from(n as i64)
    } else if n.is_finite() {
        Value::from(n)
    } else {
        Value::Null
    }
}

/// `Math.round`: halves round towards +infinity.
pub fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aural {
    None,
    SingleChime,
    Crc,
    CavalryCharge,
}

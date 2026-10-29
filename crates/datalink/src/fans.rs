use crate::tables::FANS_B_AREAS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FansMode {
    FansNone,
    FansA,
    FansB,
}

impl FansMode {
    pub fn code(self) -> u8 {
        self as u8
    }

    pub fn from_code(code: u8) -> FansMode {
        match code {
            1 => FansMode::FansA,
            2 => FansMode::FansB,
            _ => FansMode::FansNone,
        }
    }
}

pub fn current_fans_mode(identifier: &str) -> FansMode {
    let valid = identifier.len() == 4
        && identifier
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_uppercase());
    if !valid {
        return FansMode::FansNone;
    }
    if FANS_B_AREAS.contains(&identifier) {
        FansMode::FansB
    } else {
        FansMode::FansA
    }
}

//! Flight values move in digits no screen can draw, and a page repaints for each of them.
//! A change smaller than these bands stays unsent, about a tenth of a monitor pixel at most, so
//! parked noise stops repainting while real motion still goes out every frame.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Band {
    /// An ARINC word: the float in the low 32 bits, its status above them.
    Word(f32),
    Plain(f64),
}

fn word(value: f64) -> (u32, f32) {
    let bits = value as u64;
    ((bits >> 32) as u32, f32::from_bits(bits as u32))
}

impl Band {
    pub fn same_on_screen(self, was: f64, now: f64) -> bool {
        if was.is_nan() {
            return false;
        }
        match self {
            Band::Word(band) => {
                let ((was_status, was), (now_status, now)) = (word(was), word(now));
                was_status == now_status && (was - now).abs() < band
            }
            Band::Plain(band) => (was - now).abs() < band,
        }
    }
}

fn numbered<'a>(name: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = name.strip_prefix(prefix)?;
    let (number, rest) = rest.split_once('_')?;
    number.parse::<u8>().ok()?;
    Some(rest)
}

pub fn of(name: &str) -> Option<Band> {
    let name = name.strip_prefix("L:").unwrap_or(name);
    if let Some(rest) = numbered(name, "A32NX_ADIRS_IR_") {
        return Some(Band::Word(match rest {
            "PITCH" | "ROLL" | "HEADING" | "TRACK" | "TRUE_HEADING" | "TRUE_TRACK" | "DRIFT_ANGLE" | "FLIGHT_PATH_ANGLE" => 0.01,
            "GROUND_SPEED" => 0.02,
            "VERTICAL_SPEED" => 2.,
            "BODY_LATERAL_ACC" | "BODY_LONGITUDINAL_ACC" | "BODY_NORMAL_ACC" => 0.002,
            "BODY_PITCH_RATE" | "BODY_ROLL_RATE" | "BODY_YAW_RATE" | "HEADING_RATE" | "PITCH_ATT_RATE" | "ROLL_ATT_RATE" => 0.01,
            _ => return None,
        }));
    }
    if let Some(rest) = numbered(name, "A32NX_ADIRS_ADR_") {
        return Some(Band::Word(match rest {
            "ALTITUDE" | "BARO_CORRECTED_ALTITUDE_1" | "BARO_CORRECTED_ALTITUDE_2" => 0.2,
            "COMPUTED_AIRSPEED" => 0.02,
            "TRUE_AIRSPEED" => 0.05,
            "MACH" => 0.0002,
            "BAROMETRIC_VERTICAL_SPEED" => 2.,
            "CORRECTED_AVERAGE_STATIC_PRESSURE" => 0.01,
            "STATIC_AIR_TEMPERATURE" | "TOTAL_AIR_TEMPERATURE" => 0.05,
            "ANGLE_OF_ATTACK" => 0.01,
            _ => return None,
        }));
    }
    if let Some(rest) = numbered(name, "A32NX_PRIM_") {
        if rest.starts_with("PITCH_FD_COMMAND_") || rest.starts_with("ROLL_FD_COMMAND_") {
            return Some(Band::Word(0.02));
        }
        if rest == "SPEED_TREND" || rest.starts_with("SPEED_MARGIN_") || rest == "PFD_SHORT_TERM_MANAGED_SPEED" {
            return Some(Band::Word(0.05));
        }
        return None;
    }
    if numbered(name, "A32NX_RA_").is_some_and(|rest| rest == "RADIO_ALTITUDE") {
        return Some(Band::Word(0.2));
    }
    if name.starts_with("A32NX_AUTOTHRUST_TLA:") {
        return Some(Band::Plain(0.05));
    }
    Some(Band::Plain(match name {
        "ACCELERATION BODY X" | "ACCELERATION BODY Y" | "ACCELERATION BODY Z" => 0.02,
        "INCIDENCE ALPHA" | "INCIDENCE BETA" => 0.0005,
        "A32NX_BETA_TARGET" => 0.01,
        "A32NX_SIDESTICK_POSITION_X" | "A32NX_SIDESTICK_POSITION_Y" => 0.002,
        "A32NX_FG_CROSS_TRACK_ERROR" => 0.002,
        "A32NX_RADIO_RECEIVER_GS_DEVIATION" | "A32NX_RADIO_RECEIVER_LOC_DEVIATION" => 0.001,
        _ => return None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packed(status: u64, value: f32) -> f64 {
        ((status << 32) | u64::from(value.to_bits())) as f64
    }

    #[test]
    fn noise_in_a_word_stays_unsent_and_real_motion_goes_out() {
        let pitch = of("L:A32NX_ADIRS_IR_1_PITCH").unwrap();
        assert!(pitch.same_on_screen(packed(3, 2.5), packed(3, 2.504)));
        assert!(!pitch.same_on_screen(packed(3, 2.5), packed(3, 2.52)));
    }

    #[test]
    fn a_word_changing_status_always_goes_out() {
        let pitch = of("A32NX_ADIRS_IR_3_PITCH").unwrap();
        assert!(!pitch.same_on_screen(packed(3, 2.5), packed(1, 2.5)));
        assert!(!pitch.same_on_screen(packed(3, 2.5), 0.));
    }

    #[test]
    fn a_negative_word_keeps_its_sign() {
        let pitch = of("A32NX_ADIRS_IR_1_PITCH").unwrap();
        assert!(!pitch.same_on_screen(packed(3, -0.5), packed(3, 0.5)));
        assert!(pitch.same_on_screen(packed(3, -0.5), packed(3, -0.505)));
    }

    #[test]
    fn a_first_send_is_never_held_back() {
        let pitch = of("A32NX_ADIRS_IR_1_PITCH").unwrap();
        let unsent = f64::from_bits(u64::MAX);
        assert!(!pitch.same_on_screen(unsent, 0.));
        assert!(!Band::Plain(0.1).same_on_screen(unsent, 0.));
    }

    #[test]
    fn only_the_named_values_have_a_band() {
        assert_eq!(of("A32NX_ADIRS_IR_1_LATITUDE"), None);
        assert_eq!(of("A32NX_ADIRS_ADR_1_DISCRETE_WORD_1"), None);
        assert_eq!(of("A32NX_PRIM_1_SELECTED_HEADING"), None);
        assert_eq!(of("E:SIMULATION TIME"), None);
        assert_eq!(of("A32NX_AUTOTHRUST_TLA:2"), Some(Band::Plain(0.05)));
        assert_eq!(of("ACCELERATION BODY X"), Some(Band::Plain(0.02)));
    }
}

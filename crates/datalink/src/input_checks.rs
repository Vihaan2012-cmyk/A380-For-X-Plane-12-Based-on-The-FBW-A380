use crate::status::AtsuStatusCodes;
use crate::validation::{digits_between, first_digit_run};
use AtsuStatusCodes::{EntryOutOfRange, FormatError, Ok as StatusOk};

fn strip_suffix_any<'a>(s: &'a str, suffixes: &[&str]) -> Option<&'a str> {
    for suffix in suffixes {
        if let Some(rest) = s.strip_suffix(suffix) {
            return Some(rest);
        }
    }
    None
}

fn distance_part(s: &str) -> Option<(i64, bool)> {
    let (number, nm) = if let Some(rest) = s.strip_suffix("KM") {
        (rest, false)
    } else if let Some(rest) = s.strip_suffix("NM") {
        (rest, true)
    } else {
        (s, true)
    };
    if digits_between(number, 1, 3) {
        Some((number.parse().ok()?, nm))
    } else {
        None
    }
}

fn parse_offset(offset: &str) -> Option<(i64, bool, bool)> {
    let bytes = offset.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    if matches!(bytes[0], b'L' | b'R') {
        let (distance, nm) = distance_part(&offset[1..])?;
        return Some((distance, nm, bytes[0] == b'L'));
    }
    let last = *bytes.last()?;
    if matches!(last, b'L' | b'R') {
        let (distance, nm) = distance_part(&offset[..offset.len() - 1])?;
        return Some((distance, nm, last == b'L'));
    }
    None
}

pub fn validate_scratchpad_offset(offset: &str) -> AtsuStatusCodes {
    let (distance, nm, _) = match parse_offset(offset) {
        Some(v) => v,
        None => return FormatError,
    };
    let limit = if nm { 128 } else { 256 };
    if distance >= 1 && distance <= limit {
        StatusOk
    } else {
        EntryOutOfRange
    }
}

pub fn format_scratchpad_offset(value: &str) -> String {
    match parse_offset(value) {
        Some((d, nm, left)) => format!("{}{}{}", d, if nm { "NM" } else { "KM" }, if left { 'L' } else { 'R' }),
        None => String::new(),
    }
}

pub fn expand_lateral_offset(offset: &str) -> String {
    match parse_offset(offset) {
        Some((d, nm, left)) => format!("{}{} {}", d, if nm { "NM" } else { "KM" }, if left { "LEFT" } else { "RIGHT" }),
        None => String::new(),
    }
}

fn vertical_speed_parts(value: &str) -> Option<(i64, bool)> {
    let (negative, rest) = match value.as_bytes().first() {
        Some(b'+') => (false, &value[1..]),
        Some(b'-') | Some(b'M') => (true, &value[1..]),
        _ => (false, value),
    };
    let number_end = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
    if !(1..=4).contains(&number_end) {
        return None;
    }
    let unit = &rest[number_end..];
    if !matches!(unit, "FT/MIN" | "FT" | "FTM" | "M/MIN" | "MM" | "M") {
        return None;
    }
    let magnitude: i64 = rest[..number_end].parse().ok()?;
    Some((if negative { -magnitude } else { magnitude }, unit.contains("FT")))
}

pub fn validate_scratchpad_vertical_speed(value: &str) -> AtsuStatusCodes {
    match vertical_speed_parts(value) {
        None => FormatError,
        Some((speed, feet)) => {
            let limit = if feet { 6000 } else { 2000 };
            if speed >= -limit && speed <= limit {
                StatusOk
            } else {
                EntryOutOfRange
            }
        }
    }
}

pub fn format_scratchpad_vertical_speed(value: &str) -> String {
    let magnitude = first_digit_run(value).unwrap_or(0);
    let speed = if value.starts_with('-') || value.starts_with('M') { -magnitude } else { magnitude };
    if value.contains("FT") {
        format!("{}FTM", speed)
    } else {
        format!("{}MM", speed)
    }
}

pub fn validate_scratchpad_persons_on_board(value: &str) -> AtsuStatusCodes {
    if digits_between(value, 1, 4) {
        let pob: i64 = value.parse().unwrap_or(0);
        if (1..=1024).contains(&pob) {
            StatusOk
        } else {
            EntryOutOfRange
        }
    } else {
        FormatError
    }
}

fn endurance_parts(value: &str) -> Option<(i64, i64)> {
    let body = strip_suffix_any(value, &["MIN", "MN", "M"]).unwrap_or(value);
    let (hours, minutes) = match body.find('H') {
        Some(i) => (&body[..i], &body[i + 1..]),
        None => {
            if body.len() != 4 || !body.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            (&body[..2], &body[2..])
        }
    };
    if !(digits_between(hours, 1, 2) && digits_between(minutes, 2, 2)) {
        return None;
    }
    Some((hours.parse().ok()?, minutes.parse().ok()?))
}

pub fn validate_scratchpad_endurance(value: &str) -> AtsuStatusCodes {
    match endurance_parts(value) {
        None => FormatError,
        Some((hours, minutes)) => {
            if !(0..24).contains(&hours) || !(0..60).contains(&minutes) {
                EntryOutOfRange
            } else {
                StatusOk
            }
        }
    }
}

pub fn format_scratchpad_endurance(value: &str) -> String {
    match endurance_parts(value) {
        Some((h, m)) => format!("{}H{}", h, m),
        None => String::new(),
    }
}

fn temperature_value(value: &str) -> Option<(i64, bool)> {
    let negative = value.starts_with('-') || value.starts_with('M');
    let fahrenheit = value.ends_with('F');
    let mut rest = value;
    if matches!(rest.as_bytes().first(), Some(b'-') | Some(b'+') | Some(b'M')) {
        rest = &rest[1..];
    }
    if matches!(rest.as_bytes().last(), Some(b'C') | Some(b'F')) {
        rest = &rest[..rest.len() - 1];
    }
    if !digits_between(rest, 1, 3) {
        return None;
    }
    let magnitude: i64 = rest.parse().ok()?;
    Some((if negative { -magnitude } else { magnitude }, fahrenheit))
}

pub fn validate_scratchpad_temperature(value: &str) -> AtsuStatusCodes {
    match temperature_value(value) {
        None => FormatError,
        Some((temperature, fahrenheit)) => {
            if fahrenheit && (-105..=150).contains(&temperature) {
                StatusOk
            } else if !fahrenheit && (temperature >= 80 || temperature < 47) {
                StatusOk
            } else {
                EntryOutOfRange
            }
        }
    }
}

pub fn format_scratchpad_temperature(value: &str) -> String {
    match temperature_value(value) {
        Some((t, f)) => format!("{}{}", t, if f { 'F' } else { 'C' }),
        None => String::new(),
    }
}

fn wind_parts(value: &str) -> Option<(i64, i64)> {
    let body = strip_suffix_any(value, &["KT", "KM"]).unwrap_or(value);
    let (direction, speed) = body.split_once('/')?;
    if !(digits_between(direction, 1, 3) && digits_between(speed, 1, 3)) {
        return None;
    }
    Some((direction.parse().ok()?, speed.parse().ok()?))
}

pub fn validate_scratchpad_wind(value: &str) -> AtsuStatusCodes {
    match wind_parts(value) {
        None => FormatError,
        Some((direction, speed)) => {
            if !(1..=360).contains(&direction) || !(0..=255).contains(&speed) {
                EntryOutOfRange
            } else {
                StatusOk
            }
        }
    }
}

pub fn format_scratchpad_wind(value: &str) -> String {
    match wind_parts(value) {
        Some((d, s)) => format!("{:03}/{:03}{}", d, s, if value.ends_with('M') { "KM" } else { "KT" }),
        None => String::new(),
    }
}

pub fn validate_scratchpad_speed_ranges(
    value: &str,
    validate: impl Fn(&str) -> AtsuStatusCodes,
    format: impl Fn(&str) -> String,
) -> (AtsuStatusCodes, Vec<String>) {
    let entries: Vec<&str> = value.split('/').collect();
    if entries.len() != 2 {
        return (FormatError, Vec::new());
    }
    let first = validate(entries[0]);
    if first != StatusOk {
        return (first, Vec::new());
    }
    let second = validate(entries[1]);
    if second != StatusOk {
        return (second, Vec::new());
    }
    let lower = format(entries[0]);
    let higher = format(entries[1]);
    if lower.starts_with('M') != higher.starts_with('M') {
        return (FormatError, Vec::new());
    }
    if first_digit_run(&lower).unwrap_or(0) >= first_digit_run(&higher).unwrap_or(0) {
        return (EntryOutOfRange, Vec::new());
    }
    (StatusOk, vec![lower, higher])
}

fn convert_to_feet(value: &str) -> f64 {
    let height = first_digit_run(value).unwrap_or(0) as f64;
    if value.starts_with("FL") {
        height * 100.0
    } else if value.ends_with('M') {
        height * 3.28
    } else if value.ends_with("FT") {
        height
    } else if height < 1000.0 {
        height * 100.0
    } else {
        height
    }
}

pub fn validate_altitude_range(
    lower: &str,
    higher: &str,
    validate: impl Fn(&str) -> AtsuStatusCodes,
) -> AtsuStatusCodes {
    let same_type = if lower.starts_with("FL") && higher.starts_with("FL") {
        true
    } else if lower.starts_with("FL") || higher.starts_with("FL") {
        false
    } else {
        lower.ends_with('M') == higher.ends_with('M')
    };
    if !same_type {
        return FormatError;
    }
    let error = validate(lower);
    if error != StatusOk {
        return error;
    }
    let error = validate(higher);
    if error != StatusOk {
        return error;
    }
    if convert_to_feet(lower) >= convert_to_feet(higher) {
        return EntryOutOfRange;
    }
    StatusOk
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fans::FansMode;
    use crate::validation::{format_scratchpad_speed, validate_scratchpad_altitude, validate_scratchpad_speed};

    #[test]
    fn offsets() {
        assert_eq!(validate_scratchpad_offset("L10"), StatusOk);
        assert_eq!(validate_scratchpad_offset("10NMR"), StatusOk);
        assert_eq!(validate_scratchpad_offset("L129NM"), EntryOutOfRange);
        assert_eq!(validate_scratchpad_offset("L200KM"), StatusOk);
        assert_eq!(validate_scratchpad_offset("X10"), FormatError);
        assert_eq!(format_scratchpad_offset("L10"), "10NML");
        assert_eq!(expand_lateral_offset("12KMR"), "12KM RIGHT");
    }

    #[test]
    fn vertical_speed() {
        assert_eq!(validate_scratchpad_vertical_speed("-1500FTM"), StatusOk);
        assert_eq!(validate_scratchpad_vertical_speed("7000FT"), EntryOutOfRange);
        assert_eq!(validate_scratchpad_vertical_speed("3000MM"), EntryOutOfRange);
        assert_eq!(validate_scratchpad_vertical_speed("12X"), FormatError);
        assert_eq!(format_scratchpad_vertical_speed("-1500FTM"), "-1500FTM");
        assert_eq!(format_scratchpad_vertical_speed("M20MM"), "-20MM");
    }

    #[test]
    fn people_endurance_temperature_wind() {
        assert_eq!(validate_scratchpad_persons_on_board("1025"), EntryOutOfRange);
        assert_eq!(validate_scratchpad_persons_on_board("300"), StatusOk);
        assert_eq!(validate_scratchpad_endurance("0430"), StatusOk);
        assert_eq!(validate_scratchpad_endurance("4H30"), StatusOk);
        assert_eq!(validate_scratchpad_endurance("2560"), EntryOutOfRange);
        assert_eq!(format_scratchpad_endurance("0430"), "4H30");
        assert_eq!(validate_scratchpad_temperature("M50"), StatusOk);
        assert_eq!(validate_scratchpad_temperature("60"), EntryOutOfRange);
        assert_eq!(validate_scratchpad_temperature("100F"), StatusOk);
        assert_eq!(format_scratchpad_temperature("M50"), "-50C");
        assert_eq!(validate_scratchpad_wind("270/45KT"), StatusOk);
        assert_eq!(validate_scratchpad_wind("0/10"), EntryOutOfRange);
        assert_eq!(format_scratchpad_wind("270/45"), "270/045KT");
    }

    #[test]
    fn ranges() {
        let fans = FansMode::FansA;
        let v = |s: &str| validate_scratchpad_altitude(s, fans);
        assert_eq!(validate_altitude_range("FL200", "FL300", &v), StatusOk);
        assert_eq!(validate_altitude_range("FL300", "FL200", &v), EntryOutOfRange);
        assert_eq!(validate_altitude_range("FL200", "10000", &v), FormatError);
        let sv = |s: &str| validate_scratchpad_speed(s, fans);
        let sf = |s: &str| format_scratchpad_speed(s);
        let (code, values) = validate_scratchpad_speed_ranges("250/300", &sv, &sf);
        assert_eq!(code, StatusOk);
        assert_eq!(values.len(), 2);
        assert_eq!(validate_scratchpad_speed_ranges("300/250", &sv, &sf).0, EntryOutOfRange);
        assert_eq!(validate_scratchpad_speed_ranges("250", &sv, &sf).0, FormatError);
    }
}

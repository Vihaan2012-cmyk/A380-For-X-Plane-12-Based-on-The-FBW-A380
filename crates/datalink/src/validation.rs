use crate::fans::FansMode;
use crate::status::AtsuStatusCodes;

use AtsuStatusCodes::{EntryOutOfRange, FormatError, Ok as StatusOk};

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

pub fn digits_between(s: &str, min: usize, max: usize) -> bool {
    all_digits(s) && s.len() >= min && s.len() <= max
}

fn upper_alnum(s: &str, min: usize, max: usize) -> bool {
    s.len() >= min && s.len() <= max && s.bytes().all(|b| b.is_ascii_digit() || b.is_ascii_uppercase())
}

pub fn parse_int_prefix(s: &str) -> Option<i64> {
    let trimmed = s.trim_start();
    let (negative, rest) = match trimmed.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let value: i64 = digits.parse().ok()?;
    Some(if negative { -value } else { value })
}

pub fn first_digit_run(s: &str) -> Option<i64> {
    let start = s.find(|c: char| c.is_ascii_digit())?;
    let digits: String = s[start..].chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn latlon_part(part: &str, hemispheres: [char; 2], int_min: usize, int_max: usize) -> bool {
    let mut s = part;
    if let Some(c) = s.chars().next() {
        if hemispheres.contains(&c) {
            s = &s[1..];
        }
    }
    if let Some(c) = s.chars().last() {
        if hemispheres.contains(&c) {
            s = &s[..s.len() - 1];
        }
    }
    match s.split_once('.') {
        Some((whole, frac)) => digits_between(whole, int_min, int_max) && digits_between(frac, 1, 1),
        None => false,
    }
}

pub fn is_lat_lon_format(waypoint: &str) -> bool {
    match waypoint.split_once('/') {
        Some((lat, lon)) => {
            !lon.contains('/') && latlon_part(lat, ['N', 'S'], 2, 4) && latlon_part(lon, ['E', 'W'], 2, 5)
        }
        None => false,
    }
}

pub fn validate_scratchpad_waypoint(value: &str) -> AtsuStatusCodes {
    if is_lat_lon_format(value) || upper_alnum(value, 1, 5) {
        return StatusOk;
    }
    FormatError
}

pub fn validate_scratchpad_position(value: &str) -> AtsuStatusCodes {
    if upper_alnum(value, 1, 10) {
        StatusOk
    } else {
        FormatError
    }
}

pub fn validate_scratchpad_procedure(value: &str) -> AtsuStatusCodes {
    if upper_alnum(value, 1, 7) {
        StatusOk
    } else {
        FormatError
    }
}

pub fn validate_scratchpad_time(value: &str, expect_zulu: bool) -> AtsuStatusCodes {
    if expect_zulu {
        if let Some(core) = value.strip_suffix('Z') {
            if digits_between(core, 4, 4) {
                return StatusOk;
            }
        }
    } else if digits_between(value, 4, 4) {
        return StatusOk;
    }
    FormatError
}

pub fn validate_scratchpad_atis(value: &str) -> AtsuStatusCodes {
    if value.len() == 1 && value.as_bytes()[0].is_ascii_uppercase() {
        StatusOk
    } else {
        FormatError
    }
}

pub fn validate_scratchpad_degree(value: &str) -> AtsuStatusCodes {
    if digits_between(value, 1, 3) {
        let heading: i64 = value.parse().unwrap_or(0);
        if (0..=360).contains(&heading) {
            return StatusOk;
        }
        return EntryOutOfRange;
    }
    FormatError
}

pub fn validate_scratchpad_squawk(value: &str) -> AtsuStatusCodes {
    if digits_between(value, 4, 4) {
        let squawk: i64 = value.parse().unwrap_or(0);
        if (0..7777).contains(&squawk) {
            return StatusOk;
        }
        return EntryOutOfRange;
    }
    FormatError
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputWaypointType {
    Invalid,
    GeoCoordinate,
    Timepoint,
    Place,
}

pub fn classify_scratchpad_waypoint_type(waypoint: &str, allow_time: bool) -> (InputWaypointType, AtsuStatusCodes) {
    if is_lat_lon_format(waypoint) {
        return (InputWaypointType::GeoCoordinate, StatusOk);
    }

    if allow_time {
        let core = waypoint.strip_suffix('Z').unwrap_or(waypoint);
        let b = core.as_bytes();
        if b.len() == 4
            && b.iter().all(|c| c.is_ascii_digit())
            && (b'0'..=b'2').contains(&b[0])
            && (b'0'..=b'4').contains(&b[1])
            && (b'0'..=b'5').contains(&b[2])
        {
            return (InputWaypointType::Timepoint, StatusOk);
        }
    }

    let b = waypoint.as_bytes();
    if b.len() >= 2 && b[..2].iter().all(|c| c.is_ascii_digit() || c.is_ascii_uppercase()) {
        let run = b.iter().take_while(|c| c.is_ascii_digit() || c.is_ascii_uppercase()).count();
        if run >= 2 {
            return (InputWaypointType::Place, StatusOk);
        }
    }

    (InputWaypointType::Invalid, FormatError)
}

const SPACING_OTHER: [&str; 16] = [
    "00", "05", "10", "15", "25", "30", "35", "40", "50", "55", "60", "65", "75", "80", "85", "90",
];
const SPACING_END: [&str; 13] = [
    "00", "05", "10", "15", "25", "30", "35", "40", "50", "55", "60", "65", "75",
];

pub fn validate_vhf_frequency(value: &str) -> AtsuStatusCodes {
    let b = value.as_bytes();
    let shape_ok = value.is_ascii()
        && b.len() == 7
        && b[0] == b'1'
        && (b'1'..=b'3').contains(&b[1])
        && b[2].is_ascii_digit()
        && b[4].is_ascii_digit()
        && b[5].is_ascii_digit()
        && matches!(b[6], b'0' | b'|' | b'5');
    if !shape_ok {
        return FormatError;
    }
    let (before_s, after_s) = match value.split_once('.') {
        Some(parts) => parts,
        None => return FormatError,
    };
    let before: i64 = before_s.parse().unwrap_or(0);
    if !(118..=136).contains(&before) {
        return EntryOutOfRange;
    }
    let fraction = &after_s[1..];
    let list: &[&str] = if before == 136 { &SPACING_END } else { &SPACING_OTHER };
    if list.contains(&fraction) {
        StatusOk
    } else {
        EntryOutOfRange
    }
}

fn units_ok(mut u: &str) -> bool {
    while !u.is_empty() {
        if let Some(r) = u.strip_prefix("FT") {
            u = r;
        } else if let Some(r) = u.strip_prefix('M') {
            u = r;
        } else {
            return false;
        }
    }
    true
}

fn split_altitude(s: &str, allow_dashes: bool) -> Option<(usize, &str)> {
    let dashes = if allow_dashes { s.bytes().take_while(|b| *b == b'-').count() } else { 0 };
    let rest = &s[dashes..];
    let digits_len = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
    let (digits, units) = rest.split_at(digits_len);
    if digits_between(digits, 1, 5) && units_ok(units) {
        Some((dashes, digits))
    } else {
        None
    }
}

fn altitude_fans_a(value: &str) -> AtsuStatusCodes {
    let digits = match split_altitude(value, false) {
        Some((_, d)) => d,
        None => return FormatError,
    };
    let feet = !value.ends_with('M');
    let altitude: i64 = digits.parse().unwrap_or(0);

    if feet {
        if (0..=1000).contains(&altitude) && !value.ends_with("FT") {
            return FormatError;
        }
        if (0..=25000).contains(&altitude) {
            return StatusOk;
        }
        return EntryOutOfRange;
    }
    if (0..=12500).contains(&altitude) {
        return StatusOk;
    }
    EntryOutOfRange
}

fn altitude_fans_b(value: &str) -> AtsuStatusCodes {
    let (dashes, digits) = match split_altitude(value, true) {
        Some(parts) => parts,
        None => return FormatError,
    };
    let feet = !value.ends_with('M');
    if dashes > 1 {
        return EntryOutOfRange;
    }
    let mut altitude: i64 = digits.parse().unwrap_or(0);
    if dashes == 1 {
        altitude = -altitude;
    }

    if feet {
        if (0..=410).contains(&altitude) && !value.ends_with("FT") {
            return FormatError;
        }
        if (-600..=41000).contains(&altitude) {
            return StatusOk;
        }
        return EntryOutOfRange;
    }
    if (-30..=12500).contains(&altitude) {
        return StatusOk;
    }
    EntryOutOfRange
}

fn mach_digits(value: &str) -> Option<i64> {
    let rest = value.trim_start_matches('M');
    let frac = rest.strip_prefix('.')?;
    if digits_between(frac, 1, 2) {
        first_digit_run(value)
    } else {
        None
    }
}

fn knots_digits(value: &str) -> Option<i64> {
    let mut core = value;
    while let Some(c) = core.strip_suffix("KT") {
        core = c;
    }
    if digits_between(core, 1, 3) {
        first_digit_run(core)
    } else {
        None
    }
}

fn speed_with(value: &str, mach_min: i64, knots_min: i64) -> AtsuStatusCodes {
    if let Some(mut mach) = mach_digits(value) {
        if mach < 10 {
            mach *= 10;
        }
        if (mach_min..=92).contains(&mach) {
            return StatusOk;
        }
        return EntryOutOfRange;
    }
    if let Some(knots) = knots_digits(value) {
        if (knots_min..=350).contains(&knots) {
            return StatusOk;
        }
        return EntryOutOfRange;
    }
    FormatError
}

pub fn validate_scratchpad_altitude(value: &str, fans: FansMode) -> AtsuStatusCodes {
    let mut core = value;
    while let Some(c) = core.strip_prefix("FL") {
        core = c;
    }
    if digits_between(core, 1, 3) {
        let level: i64 = first_digit_run(value).unwrap_or(0);
        if (30..=410).contains(&level) {
            return StatusOk;
        }
        return EntryOutOfRange;
    }

    if fans == FansMode::FansB {
        altitude_fans_b(value)
    } else {
        altitude_fans_a(value)
    }
}

pub fn validate_scratchpad_speed(value: &str, fans: FansMode) -> AtsuStatusCodes {
    if fans == FansMode::FansB {
        speed_with(value, 50, 0)
    } else {
        speed_with(value, 61, 70)
    }
}

pub fn validate_scratchpad_distance(distance: &str) -> AtsuStatusCodes {
    let core = distance
        .strip_suffix("NM")
        .or_else(|| distance.strip_suffix("KM"))
        .unwrap_or(distance);
    if digits_between(core, 1, 3) {
        StatusOk
    } else {
        FormatError
    }
}

pub fn format_scratchpad_altitude(value: &str) -> String {
    if value.starts_with("FL") || value.ends_with('M') || value.ends_with("FT") {
        return value.to_string();
    }
    let altitude = parse_int_prefix(value).unwrap_or(-1);
    if (30..=410).contains(&altitude) {
        return format!("FL{}", value);
    }
    format!("{}FT", value)
}

pub fn format_scratchpad_speed(value: &str) -> String {
    if value.starts_with('M') || value.starts_with('.') {
        return format!("M.{}", first_digit_run_text(value));
    }
    value.replacen("KT", "", 1)
}

fn first_digit_run_text(s: &str) -> String {
    match s.find(|c: char| c.is_ascii_digit()) {
        Some(start) => s[start..].chars().take_while(|c| c.is_ascii_digit()).collect(),
        None => String::new(),
    }
}

pub fn format_scratchpad_distance(distance: &str) -> String {
    if distance.ends_with("NM") || distance.ends_with("KM") {
        return distance.to_string();
    }
    format!("{}NM", distance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use FansMode::{FansA, FansB, FansNone};

    #[test]
    fn waypoints() {
        assert_eq!(validate_scratchpad_waypoint("LAMBO"), StatusOk);
        assert_eq!(validate_scratchpad_waypoint("N5130.0/W00020.0"), StatusOk);
        assert_eq!(validate_scratchpad_waypoint("5130.0N/00020.0W"), StatusOk);
        assert_eq!(validate_scratchpad_waypoint("TOOLONG"), FormatError);
        assert_eq!(validate_scratchpad_waypoint("lambo"), FormatError);
    }

    #[test]
    fn times() {
        assert_eq!(validate_scratchpad_time("1234Z", true), StatusOk);
        assert_eq!(validate_scratchpad_time("1234", true), FormatError);
        assert_eq!(validate_scratchpad_time("1234", false), StatusOk);
        assert_eq!(validate_scratchpad_time("1234Z", false), FormatError);
    }

    #[test]
    fn degrees_and_squawk() {
        assert_eq!(validate_scratchpad_degree("360"), StatusOk);
        assert_eq!(validate_scratchpad_degree("361"), EntryOutOfRange);
        assert_eq!(validate_scratchpad_degree("1A"), FormatError);
        assert_eq!(validate_scratchpad_squawk("7000"), StatusOk);
        assert_eq!(validate_scratchpad_squawk("7777"), EntryOutOfRange);
        assert_eq!(validate_scratchpad_squawk("700"), FormatError);
    }

    #[test]
    fn frequencies() {
        assert_eq!(validate_vhf_frequency("118.000"), StatusOk);
        assert_eq!(validate_vhf_frequency("121.500"), StatusOk);
        assert_eq!(validate_vhf_frequency("136.975"), StatusOk);
        assert_eq!(validate_vhf_frequency("136.980"), EntryOutOfRange);
        assert_eq!(validate_vhf_frequency("136.900"), StatusOk);
        assert_eq!(validate_vhf_frequency("136.775"), StatusOk);
        assert_eq!(validate_vhf_frequency("117.900"), EntryOutOfRange);
        assert_eq!(validate_vhf_frequency("118.020"), EntryOutOfRange);
        assert_eq!(validate_vhf_frequency("118x000"), FormatError);
    }

    #[test]
    fn altitudes() {
        assert_eq!(validate_scratchpad_altitude("FL350", FansA), StatusOk);
        assert_eq!(validate_scratchpad_altitude("350", FansA), StatusOk);
        assert_eq!(validate_scratchpad_altitude("420", FansA), EntryOutOfRange);
        assert_eq!(validate_scratchpad_altitude("5000FT", FansA), StatusOk);
        assert_eq!(validate_scratchpad_altitude("5000", FansA), StatusOk);
        assert_eq!(validate_scratchpad_altitude("26000FT", FansA), EntryOutOfRange);
        assert_eq!(validate_scratchpad_altitude("900", FansA), EntryOutOfRange);
        assert_eq!(validate_scratchpad_altitude("10000M", FansA), StatusOk);
        assert_eq!(validate_scratchpad_altitude("13000M", FansA), EntryOutOfRange);
        assert_eq!(validate_scratchpad_altitude("-500FT", FansB), StatusOk);
        assert_eq!(validate_scratchpad_altitude("-500FT", FansA), FormatError);
        assert_eq!(validate_scratchpad_altitude("41000FT", FansB), StatusOk);
        assert_eq!(validate_scratchpad_altitude("41001FT", FansB), EntryOutOfRange);
        assert_eq!(validate_scratchpad_altitude("FLX", FansNone), FormatError);
    }

    #[test]
    fn speeds() {
        assert_eq!(validate_scratchpad_speed(".78", FansA), StatusOk);
        assert_eq!(validate_scratchpad_speed("M.78", FansA), StatusOk);
        assert_eq!(validate_scratchpad_speed(".5", FansA), EntryOutOfRange);
        assert_eq!(validate_scratchpad_speed(".5", FansB), StatusOk);
        assert_eq!(validate_scratchpad_speed("250", FansA), StatusOk);
        assert_eq!(validate_scratchpad_speed("250KT", FansA), StatusOk);
        assert_eq!(validate_scratchpad_speed("60", FansA), EntryOutOfRange);
        assert_eq!(validate_scratchpad_speed("60", FansB), StatusOk);
        assert_eq!(validate_scratchpad_speed("400", FansB), EntryOutOfRange);
        assert_eq!(validate_scratchpad_speed("FAST", FansA), FormatError);
    }

    #[test]
    fn distances_and_formats() {
        assert_eq!(validate_scratchpad_distance("25NM"), StatusOk);
        assert_eq!(validate_scratchpad_distance("25"), StatusOk);
        assert_eq!(validate_scratchpad_distance("2500"), FormatError);
        assert_eq!(format_scratchpad_distance("25"), "25NM");
        assert_eq!(format_scratchpad_altitude("350"), "FL350");
        assert_eq!(format_scratchpad_altitude("5000"), "5000FT");
        assert_eq!(format_scratchpad_altitude("FL350"), "FL350");
        assert_eq!(format_scratchpad_speed(".78"), "M.78");
        assert_eq!(format_scratchpad_speed("250KT"), "250");
    }

    #[test]
    fn waypoint_classes() {
        assert_eq!(
            classify_scratchpad_waypoint_type("N5130.0/W00020.0", false).0,
            InputWaypointType::GeoCoordinate
        );
        assert_eq!(classify_scratchpad_waypoint_type("1234Z", true).0, InputWaypointType::Timepoint);
        assert_eq!(classify_scratchpad_waypoint_type("LAMBO", true).0, InputWaypointType::Place);
        assert_eq!(classify_scratchpad_waypoint_type("-", true).0, InputWaypointType::Invalid);
    }
}

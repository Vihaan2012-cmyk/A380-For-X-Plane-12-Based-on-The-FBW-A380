use crate::navdb::Fix;
use crate::plan::{LegDefinition, LegType};

pub const DEGREE: char = '\u{b0}';

pub fn js_round(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

fn pad(n: i64, width: usize, fill: char) -> String {
    let s = n.to_string();
    if s.len() >= width {
        s
    } else {
        format!("{}{s}", fill.to_string().repeat(width - s.len()))
    }
}

fn course_annotation(prefix: char, def: &LegDefinition) -> String {
    format!("{prefix}{}{}", pad(js_round(def.course.unwrap_or(0.)), 3, '0'), if def.mag_var.is_some() { DEGREE } else { 'T' })
}

fn ident3(fix: &Option<Fix>) -> String {
    fix.as_ref().map(|f| f.ident.chars().take(3).collect()).unwrap_or_default()
}

fn hold_direction(def: &LegDefinition) -> char {
    if def.turn_direction == Some('L') {
        'L'
    } else {
        'R'
    }
}

pub fn procedure_leg_ident_and_annotation(def: &LegDefinition, procedure_ident: &str) -> (String, String) {
    use LegType::*;
    let wp_ident = def.waypoint.as_ref().map(|w| w.ident.clone()).unwrap_or_default();
    match def.leg_type {
        AF => (wp_ident, format!("{} {}", pad(js_round(def.rho.unwrap_or(0.)), 2, ' '), ident3(&def.recommended_navaid))),
        CF => (wp_ident, course_annotation('C', def)),
        IF | DF | TF => (wp_ident, procedure_ident.to_string()),
        RF => (wp_ident, format!("{} ARC", pad(js_round(def.length.unwrap_or(0.)), 2, ' '))),
        CA | FA | VA => {
            let alt: String = js_round(def.altitude1.unwrap_or(0.)).to_string().chars().take(9).collect();
            (alt, course_annotation(if def.leg_type == VA { 'H' } else { 'C' }, def))
        }
        CD | FC | FD | VD => {
            let target = if def.leg_type == FC { &def.waypoint } else { &def.recommended_navaid };
            (format!("{}/{}", ident3(target), pad(js_round(def.length.unwrap_or(0.)), 2, '0')), course_annotation(if def.leg_type == VD { 'H' } else { 'C' }, def))
        }
        CI | VI => ("INTCPT".to_string(), course_annotation(if def.leg_type == CI { 'C' } else { 'H' }, def)),
        CR | VR => (format!("{}{}", ident3(&def.recommended_navaid), pad(js_round(def.theta.unwrap_or(0.)), 3, '0')), course_annotation(if def.leg_type == VR { 'H' } else { 'C' }, def)),
        HA => (js_round(def.altitude1.unwrap_or(0.)).to_string(), format!("HOLD {}", hold_direction(def))),
        HF => (wp_ident, format!("HOLD {}", hold_direction(def))),
        HM => (wp_ident, course_annotation('C', def)),
        PI => ("INTCPT".to_string(), format!("PROC {}", hold_direction(def))),
        FM | VM => ("MANUAL".to_string(), course_annotation(if def.leg_type == FM { 'C' } else { 'H' }, def)),
    }
}

pub fn short_approach_name(approach_type: i32, runway_ident: &str, multiple_indicator: &str) -> String {
    let kind = match approach_type {
        10 => "GPS",
        5 => "ILS",
        18 => "LDA",
        7 => "LOC",
        1 => "BAC",
        9 | 11 => "NDB",
        12 => "RNV",
        15 => "SDF",
        16 | 2 => "VOR",
        _ => "",
    };
    let suffix = if multiple_indicator.is_empty() { String::new() } else { format!("-{multiple_indicator}") };
    format!("{kind}{}{suffix}", runway_string(runway_ident))
}

pub fn runway_string(runway_ident: &str) -> String {
    let chars: Vec<char> = runway_ident.chars().collect();
    for start in 0..chars.len() {
        let rest = &chars[start..];
        if rest.len() >= 6 && rest[..4].iter().all(|c| c.is_alphanumeric() || *c == '_') && rest[4].is_ascii_digit() && rest[5].is_ascii_digit() {
            let number: String = rest[4..6].iter().collect();
            if number == "00" {
                return String::new();
            }
            let designator = rest.get(6).filter(|c| "LCRTABW".contains(**c)).map(|c| c.to_string()).unwrap_or_default();
            return format!("{number}{designator}");
        }
        if rest.len() >= 6 {
            continue;
        }
        break;
    }
    String::new()
}

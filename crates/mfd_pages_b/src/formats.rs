use crate::widgets::Fmt;

const DEG_C: &str = "\u{00B0}C";

fn js_fixed(v: f64, d: usize) -> String {
    let s = format!("{:.*}", d, v);
    if s.starts_with("-0") && s.trim_start_matches('-').chars().all(|c| c == '0' || c == '.') {
        s.trim_start_matches('-').to_string()
    } else {
        s
    }
}

pub fn fixed0(v: f64) -> String {
    js_fixed(v, 0)
}

pub fn fixed1(v: f64) -> String {
    js_fixed(v, 1)
}

pub fn text_format(placeholder: &str, digits: usize, v: Option<&str>) -> Fmt {
    match v {
        Some(s) if !s.is_empty() => Fmt::new(s, "", "", digits, false),
        _ => Fmt::new(placeholder, "", "", digits, true),
    }
}

pub fn long_alphanumeric(v: Option<&str>) -> Fmt {
    text_format("----------", 10, v)
}

pub fn airport(v: Option<&str>) -> Fmt {
    text_format("----", 4, v)
}

pub fn flight_level(v: Option<f64>) -> Fmt {
    match v {
        None => Fmt::new("---", "FL", "", 3, true).overflow(2),
        Some(v) => Fmt::new(&format!("{:0>3}", fixed0(v.round())), "FL", "", 3, false).overflow(2),
    }
}

pub fn tropo(v: Option<f64>) -> Fmt {
    match v {
        None => Fmt::new("-----", "", "FT", 5, true),
        Some(v) => Fmt::new(&fixed0(v), "", "FT", 5, false),
    }
}

pub fn signed_temp(v: Option<f64>, digits: usize) -> Fmt {
    match v {
        None => Fmt::new("---", "", DEG_C, digits, true),
        Some(v) if v >= 0.0 => Fmt::new(&format!("+{}", fixed0(v)), "", DEG_C, digits, false),
        Some(v) => Fmt::new(&fixed0(v), "", DEG_C, digits, false),
    }
}

pub fn crz_temp(v: Option<f64>) -> Fmt {
    signed_temp(v, 3)
}

pub fn temperature(v: Option<f64>) -> Fmt {
    signed_temp(v, 3)
}

pub fn trip_wind(v: Option<f64>) -> Fmt {
    match v {
        None => Fmt::new("-----", "", "", 5, true),
        Some(v) if v >= 0.0 => Fmt::new(&format!("{:0>3}", fixed0(v.abs())), "TL", "", 5, false),
        Some(v) => Fmt::new(&format!("{:0>3}", fixed0(v.abs())), "HD", "", 5, false),
    }
}

pub fn cost_index(v: Option<f64>) -> Fmt {
    match v {
        None => Fmt::new("---", "", "", 3, true),
        Some(v) => Fmt::new(&js_num(v), "", "", 3, false),
    }
}

pub fn js_num(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

pub fn weight(v_kg: Option<f64>, metric: bool) -> Fmt {
    let unit = if metric { "T" } else { "KLB" };
    match v_kg {
        None => Fmt::new("---.-", "", unit, 5, true),
        Some(v) => {
            let v = if metric { v } else { v * 2.20462262185 };
            Fmt::new(&fixed1(v / 1000.0), "", unit, 5, false)
        }
    }
}

pub fn weight_text(v_kg: Option<f64>, metric: bool) -> String {
    match v_kg {
        Some(v) if !v.is_nan() => {
            let v = if metric { v } else { v * 2.20462262185 };
            fixed1(v / 1000.0)
        }
        _ => "---.-".to_string(),
    }
}

pub fn weight_unit_text(metric: bool) -> &'static str {
    if metric {
        "T"
    } else {
        "KLB"
    }
}

pub fn percentage(v: Option<f64>) -> Fmt {
    match v {
        None => Fmt::new("--.-", "", "%", 4, true),
        Some(v) => Fmt::new(&fixed1(v), "", "%", 4, false),
    }
}

pub fn pax_nbr(v: Option<f64>) -> Fmt {
    match v {
        None => Fmt::new("---", "", "", 3, true),
        Some(v) => Fmt::new(&fixed0(v), "", "", 3, false),
    }
}

pub fn time_hhmm(v: Option<f64>) -> Fmt {
    match v {
        Some(v) if v != 0.0 => {
            let h = (v / 60.0).floor().abs();
            let m = (v % 60.0).abs();
            Fmt::new(&format!("{:0>2}:{:0>2}", fixed0(h), fixed0(m)), "", "", 4, false)
        }
        _ => Fmt::new("--:--", "", "", 4, true),
    }
}

pub fn speed_knots(v: Option<f64>) -> Fmt {
    match v {
        None => Fmt::new("---", "", "KT", 3, true),
        Some(v) => Fmt::new(&js_num(v), "", "KT", 3, false),
    }
}

pub fn speed_mach(v: Option<f64>) -> Fmt {
    match v {
        Some(v) if v != 0.0 => {
            let s = js_fixed(v, 2);
            let frac = s.split('.').nth(1).unwrap_or("");
            Fmt::new(&format!(".{}", frac), "", "", 3, false)
        }
        _ => Fmt::new(".--", "", "", 3, true),
    }
}

pub fn altitude(v: Option<f64>) -> Fmt {
    match v {
        None => Fmt::new("-----", "", "FT", 5, true),
        Some(v) => Fmt::new(&fixed0(v), "", "FT", 5, false),
    }
}

pub fn radio_altitude(v: Option<f64>) -> Fmt {
    match v {
        None => Fmt::new("-----", "", "FT", 5, true),
        Some(v) if v == 0.0 => Fmt::new("NO DH", "", "", 5, false),
        Some(v) => Fmt::new(&fixed0(v), "", "FT", 5, false),
    }
}

pub struct TransAlt {
    pub value_ft: Option<f64>,
    pub is_fl: bool,
}

pub fn altitude_or_fl(v: Option<f64>, trans: Option<&TransAlt>) -> Fmt {
    match v {
        None => Fmt::new("-----", "", "FT", 5, true),
        Some(v) => {
            if let Some(t) = trans {
                if let Some(ta) = t.value_ft {
                    let ta = if t.is_fl { ta * 100.0 } else { ta };
                    if (!t.is_fl && v > ta) || (t.is_fl && v >= ta) {
                        return Fmt::new(&format!("{:0>3}", fixed0(v / 100.0)), "FL", "", 5, false);
                    }
                }
            }
            Fmt::new(&fixed0(v), "", "FT", 5, false)
        }
    }
}

pub fn length(v_m: Option<f64>, metric: bool) -> Fmt {
    let unit = if metric { "M" } else { "FT" };
    match v_m {
        None => Fmt::new("----", "", unit, 4, true),
        Some(v) => {
            let v = if metric { v } else { v * 3.280839895 };
            Fmt::new(&fixed0(v), "", unit, 4, false)
        }
    }
}

pub fn wind_direction(v: Option<f64>) -> Fmt {
    match v {
        None => Fmt::new("---", "", "\u{00B0}", 3, true),
        Some(v) => Fmt::new(&format!("{:0>3}", fixed0(v)), "", "\u{00B0}", 3, false),
    }
}

pub fn wind_speed(v: Option<f64>) -> Fmt {
    match v {
        None => Fmt::new("---", "", "KT", 3, true),
        Some(v) => Fmt::new(&format!("{:0>3}", fixed0(v)), "", "KT", 3, false),
    }
}

pub fn qnh(v: Option<f64>, hpa: bool) -> Fmt {
    match v {
        None => Fmt::new(if hpa { "----" } else { "--.--" }, "", "", 5, true),
        Some(v) => Fmt::new(&if v < 745.0 { js_fixed(v, 2) } else { fixed0(v) }, "", "", 5, false),
    }
}

pub fn descent_rate(v: Option<f64>) -> Fmt {
    match v {
        None => Fmt::new("----", "", "FT/MN", 4, true),
        Some(v) => Fmt::new(&js_num(v), "", "FT/MN", 4, false),
    }
}

pub fn fmc_format_value(v: Option<f64>) -> String {
    match v {
        Some(v) if !v.is_nan() => fixed0(v),
        _ => "---".to_string(),
    }
}

pub fn group_thousands(digits: &str) -> String {
    let mut out = String::new();
    let n = digits.len();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (n - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn num(v: Option<f64>, decimals: usize) -> String {
    match v {
        Some(x) if x.is_finite() => {
            let s = format!("{:.*}", decimals, x.abs());
            let (int, frac) = match s.split_once('.') {
                Some((i, f)) => (i.to_string(), Some(f.to_string())),
                None => (s, None),
            };
            let mut out = String::new();
            if x < 0.0 && s_nonzero(&int, &frac) {
                out.push('-');
            }
            out.push_str(&group_thousands(&int));
            if let Some(f) = frac {
                out.push('.');
                out.push_str(&f);
            }
            out
        }
        _ => "--".to_string(),
    }
}

fn s_nonzero(int: &str, frac: &Option<String>) -> bool {
    int.chars().any(|c| c != '0') || frac.as_deref().map_or(false, |f| f.chars().any(|c| c != '0'))
}

pub fn hhmm(sec: Option<f64>) -> String {
    match sec {
        Some(s) if s.is_finite() => {
            let m = (s / 60.0).round() as i64;
            format!("{:02}:{:02}", m.div_euclid(60), m.rem_euclid(60))
        }
        _ => "--".to_string(),
    }
}

pub fn utc_hm(sec: Option<f64>) -> String {
    match sec {
        Some(n) if n.is_finite() && n > 0.0 => {
            let s = n as i64;
            format!("{:02}:{:02}", (s / 3600) % 24, (s / 60) % 60)
        }
        _ => "--:--".to_string(),
    }
}

pub fn zulu_hm(zulu: f64) -> String {
    let s = zulu.floor() as i64;
    format!("{:02}:{:02}Z", (s / 3600) % 24, (s / 60) % 60)
}

pub fn js_round(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_formats_like_to_locale_string() {
        assert_eq!(num(Some(1234567.0), 0), "1,234,567");
        assert_eq!(num(Some(12.5), 1), "12.5");
        assert_eq!(num(None, 0), "--");
    }

    #[test]
    fn times() {
        assert_eq!(hhmm(Some(3600.0 * 7.0 + 1800.0)), "07:30");
        assert_eq!(utc_hm(Some(1_700_000_000.0)), "22:13");
        assert_eq!(zulu_hm(45296.0), "12:34Z");
    }
}

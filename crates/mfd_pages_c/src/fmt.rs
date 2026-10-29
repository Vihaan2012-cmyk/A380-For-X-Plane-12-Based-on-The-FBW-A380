pub fn to_fixed(v: f64, d: usize) -> String {
    let s = format!("{:.*}", d, v);
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        s[1..].to_string()
    } else {
        s
    }
}

pub fn pad_start(s: &str, n: usize, c: char) -> String {
    let len = s.chars().count();
    if len >= n {
        s.to_string()
    } else {
        std::iter::repeat(c).take(n - len).collect::<String>() + s
    }
}

pub fn mandatory(s: &str) -> String {
    s.replace('-', "\u{25AF}")
}

pub fn month_abbr(mm: &str) -> &'static str {
    match mm {
        "01" => "JAN",
        "02" => "FEB",
        "03" => "MAR",
        "04" => "APR",
        "05" => "MAY",
        "06" => "JUN",
        "07" => "JUL",
        "08" => "AUG",
        "09" => "SEP",
        "10" => "OCT",
        "11" => "NOV",
        "12" => "DEC",
        _ => "undefined",
    }
}

pub fn signed_factor(v: f64) -> String {
    format!("{}{}", if v >= 0.0 { '+' } else { '-' }, to_fixed(v, 1))
}

pub const NO_POSITION: &str = "--\u{b0}--.--/---\u{b0}--.--";

fn dms(deg: f64, lng: bool) -> (char, i64, f64, f64) {
    let m = ((deg % 1.0) * 60e7).trunc();
    let d = (if deg < 0.0 { -deg } else { deg }).trunc() as i64;
    let dir = if deg < 0.0 {
        if lng { 'W' } else { 'S' }
    } else if lng {
        'E'
    } else {
        'N'
    };
    let min = (m / 1e7).trunc().abs();
    let sec = ((((m / 1e6) % 1.0) * 6e4).trunc() / 100.0).abs();
    (dir, d, min, sec)
}

pub fn coordinate_to_string(lat: f64, lon: f64, short: bool) -> String {
    let (ld, ldeg, lmin, lsec) = dms(lat, false);
    let (od, odeg, omin, osec) = dms(lon, true);
    let lsec = (lsec / 100.0).ceil() as i64;
    let osec = (osec / 100.0).ceil() as i64;
    if short {
        return if ld == 'N' {
            if od == 'E' { format!("{}N{}", ldeg, odeg) } else { format!("{}{}N", ldeg, odeg) }
        } else if od == 'E' {
            format!("{}{}S", ldeg, odeg)
        } else {
            format!("{}W{}", ldeg, odeg)
        };
    }
    format!(
        "{}\u{b0}{}.{}{}/{}\u{b0}{}.{}{}",
        pad_start(&ldeg.to_string(), 2, '0'),
        pad_start(&(lmin as i64).to_string(), 2, '0'),
        lsec,
        ld,
        pad_start(&odeg.to_string(), 3, '0'),
        pad_start(&(omin as i64).to_string(), 2, '0'),
        osec,
        od
    )
}

pub fn pad_end(s: &str, n: usize, c: char) -> String {
    let len = s.chars().count();
    if len >= n {
        s.to_string()
    } else {
        s.to_string() + &std::iter::repeat(c).take(n - len).collect::<String>()
    }
}

pub fn position_text(p: Option<(f64, f64)>) -> String {
    match p {
        Some((a, b)) => coordinate_to_string(a, b, false),
        None => NO_POSITION.to_string(),
    }
}

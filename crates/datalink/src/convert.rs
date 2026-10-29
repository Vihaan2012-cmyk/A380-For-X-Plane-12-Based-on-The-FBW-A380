fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | ':' | '*' | '@' | '_')
}

fn tokenize(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if is_word_char(c) {
            let start = i;
            while i < chars.len() && is_word_char(chars[i]) {
                i += 1;
            }
            tokens.push(chars[start..i].iter().collect());
            continue;
        }
        if c == '[' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j > i + 1 && j < chars.len() && chars[j] == ']' {
                tokens.push(chars[i..=j].iter().collect());
                i = j + 1;
                continue;
            }
        }
        if c == '\n' {
            tokens.push("\n".to_string());
        }
        i += 1;
    }
    tokens
}

pub fn word_wrap(text: &str, max_length: usize) -> Vec<String> {
    let mut result = Vec::new();
    let mut line: Vec<String> = Vec::new();
    let mut length = 0usize;

    for word in tokenize(text) {
        if word == "\n" {
            continue;
        }
        let word_len = word.chars().count();
        if length + word_len >= max_length {
            result.push(line.join(" ").to_uppercase());
            line.clear();
            length = 0;
        }
        length += word_len + 1;
        line.push(word);
    }

    if !line.is_empty() {
        result.push(line.join(" ").to_uppercase());
    }
    result
}

pub fn timestamp_to_string(seconds: f64) -> String {
    let hours = (seconds / 3600.0).floor() as i64;
    let minutes = ((seconds / 60.0).floor() as i64).rem_euclid(60);
    format!("{:02}{:02}", hours, minutes)
}

struct Dms {
    dir: char,
    deg: i64,
    min: i64,
    sec: f64,
}

fn trunc_i32(v: f64) -> f64 {
    (v as i32) as f64
}

fn decimal_to_dms(deg: f64, lng: bool) -> Dms {
    let m = trunc_i32((deg % 1.0) * 60e7);
    let whole = trunc_i32(if deg < 0.0 { -deg } else { deg }) as i64;
    let dir = if deg < 0.0 {
        if lng {
            'W'
        } else {
            'S'
        }
    } else if lng {
        'E'
    } else {
        'N'
    };
    Dms {
        dir,
        deg: whole,
        min: trunc_i32(m / 1e7).abs() as i64,
        sec: (trunc_i32(((m / 1e6) % 1.0) * 6e4) / 100.0).abs(),
    }
}

pub fn coordinate_to_string(lat: f64, lon: f64, short_version: bool) -> String {
    let mut dms_lat = decimal_to_dms(lat, false);
    let mut dms_lon = decimal_to_dms(lon, true);
    dms_lat.sec = (dms_lat.sec / 100.0).ceil();
    dms_lon.sec = (dms_lon.sec / 100.0).ceil();

    if short_version {
        if dms_lat.dir == 'N' {
            if dms_lon.dir == 'E' {
                return format!("{}N{}", dms_lat.deg, dms_lon.deg);
            }
            return format!("{}{}N", dms_lat.deg, dms_lon.deg);
        }
        if dms_lon.dir == 'E' {
            return format!("{}{}S", dms_lat.deg, dms_lon.deg);
        }
        return format!("{}W{}", dms_lat.deg, dms_lon.deg);
    }

    let lat_s = format!(
        "{}\u{b0}{}.{}{}",
        dms_lat.deg, dms_lat.min, dms_lat.sec as i64, dms_lat.dir
    );
    let lon_s = format!(
        "{}\u{b0}{}.{}{}",
        dms_lon.deg, dms_lon.min, dms_lon.sec as i64, dms_lon.dir
    );
    format!("{}/{}", lat_s, lon_s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_is_hhmm() {
        assert_eq!(timestamp_to_string(0.0), "0000");
        assert_eq!(timestamp_to_string(45296.0), "1234");
        assert_eq!(timestamp_to_string(86399.0), "2359");
    }

    #[test]
    fn wrap_breaks_at_the_limit_and_uppercases() {
        let lines = word_wrap("request climb to fl350 due to turbulence", 25);
        assert_eq!(lines, vec!["REQUEST CLIMB TO FL350", "DUE TO TURBULENCE"]);
    }

    #[test]
    fn wrap_keeps_blank_placeholders_and_marks() {
        let lines = word_wrap("AT @LAMBO@ REQUEST [      ]", 40);
        assert_eq!(lines, vec!["AT @LAMBO@ REQUEST [      ]"]);
    }

    #[test]
    fn wrap_ignores_newlines_and_unknown_characters() {
        assert_eq!(word_wrap("A\nB, C", 30), vec!["A B C"]);
    }

    #[test]
    fn wrap_pushes_an_empty_line_for_an_overlong_first_word() {
        let lines = word_wrap("ABCDEFGHIJKL", 5);
        assert_eq!(lines, vec!["", "ABCDEFGHIJKL"]);
    }

    #[test]
    fn coordinates_follow_the_original_rounding() {
        assert_eq!(coordinate_to_string(51.4775, -0.4614, true), "510N");
        assert_eq!(coordinate_to_string(40.6398, -73.7789, true), "4073N");
        assert_eq!(coordinate_to_string(-33.9461, 151.1772, true), "33151S");
        assert_eq!(
            coordinate_to_string(51.4775, -0.4614, false),
            "51\u{b0}28.3N/0\u{b0}27.6W"
        );
        assert_eq!(
            coordinate_to_string(40.6398, -73.7789, false),
            "40\u{b0}38.6N/73\u{b0}46.3W"
        );
        assert_eq!(
            coordinate_to_string(-33.9461, 151.1772, false),
            "33\u{b0}56.4S/151\u{b0}10.2E"
        );
    }
}

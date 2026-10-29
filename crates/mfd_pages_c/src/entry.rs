pub const MAX_CERTIFIED_ALT: f64 = 43_100.0;

#[derive(Clone, Debug, PartialEq)]
pub enum EntryError {
    FormatError(Option<String>),
    EntryOutOfRange(Option<String>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Formatted {
    pub value: String,
    pub leading: Option<String>,
    pub trailing: Option<String>,
}

fn out(value: &str, leading: Option<&str>, trailing: Option<&str>) -> Formatted {
    Formatted { value: value.to_string(), leading: leading.map(String::from), trailing: trailing.map(String::from) }
}

fn unit_suffix(unit: Option<&str>) -> String {
    unit.map(|u| format!(" {}", u)).unwrap_or_default()
}

fn range_error(min: &str, max: &str, unit: Option<&str>) -> EntryError {
    EntryError::EntryOutOfRange(Some(format!("RNG: {} TO {}{}", min, max, unit_suffix(unit))))
}

fn format_error(fmt: &str, unit: Option<&str>) -> EntryError {
    EntryError::FormatError(Some(format!("FORMAT: {}{}", fmt, unit_suffix(unit))))
}

pub fn js_number(input: &str) -> f64 {
    let s = input.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    if s.is_empty() {
        return 0.0;
    }
    for (p, radix) in [("0x", 16), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
        if let Some(rest) = s.strip_prefix(p) {
            return u64::from_str_radix(rest, radix).map(|v| v as f64).unwrap_or(f64::NAN);
        }
    }
    let unsigned = s.strip_prefix(['+', '-']).unwrap_or(s);
    if unsigned == "Infinity" {
        return if s.starts_with('-') { f64::NEG_INFINITY } else { f64::INFINITY };
    }
    if !unsigned.chars().all(|c| c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' || c == '+' || c == '-') {
        return f64::NAN;
    }
    s.parse::<f64>().unwrap_or(f64::NAN)
}

pub fn js_to_fixed(v: f64, digits: usize) -> String {
    if !v.is_finite() {
        return if v.is_nan() { "NaN".into() } else if v > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if v.abs() >= 1e21 {
        return format!("{}", v);
    }
    let exact = format!("{:.1100}", v.abs());
    let (int_part, frac) = exact.split_once('.').unwrap();
    let mut d: Vec<u8> = int_part.bytes().chain(frac.bytes().take(digits)).map(|b| b - b'0').collect();
    let rest = &frac[digits..];
    if rest.as_bytes()[0] >= b'5' {
        let mut i = d.len();
        loop {
            if i == 0 {
                d.insert(0, 1);
                break;
            }
            i -= 1;
            if d[i] == 9 {
                d[i] = 0;
            } else {
                d[i] += 1;
                break;
            }
        }
    }
    let n_int = d.len() - digits;
    let mut s: String = d[..n_int].iter().map(|x| (b'0' + x) as char).collect();
    if digits > 0 {
        s.push('.');
        s.extend(d[n_int..].iter().map(|x| (b'0' + x) as char));
    }
    if v < 0.0 && s.bytes().any(|b| b != b'0' && b != b'.') {
        s.insert(0, '-');
    }
    s
}

fn pad3(s: String) -> String {
    format!("{:0>3}", s)
}

fn parse_range(input: &str, min: f64, max: f64, on_range: impl Fn() -> EntryError, on_format: impl Fn() -> EntryError) -> Result<Option<f64>, EntryError> {
    if input.is_empty() {
        return Ok(None);
    }
    let n = js_number(input);
    if !n.is_nan() && n <= max && n >= min {
        return Ok(Some(n));
    }
    if n > max || n < min {
        Err(on_range())
    } else {
        Err(on_format())
    }
}

fn js_num_str(v: f64) -> String {
    if v == f64::INFINITY {
        "Infinity".into()
    } else if v.fract() == 0.0 && v.abs() < 1e21 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SpeedKnotsFormat {
    pub min: f64,
    pub max: f64,
}

impl Default for SpeedKnotsFormat {
    fn default() -> Self {
        SpeedKnotsFormat { min: 0.0, max: f64::INFINITY }
    }
}

impl SpeedKnotsFormat {
    pub fn format(&self, v: Option<f64>) -> Formatted {
        match v {
            None => out("---", None, Some("KT")),
            Some(v) => out(&js_num_str(v), None, Some("KT")),
        }
    }
    pub fn parse(&self, input: &str) -> Result<Option<f64>, EntryError> {
        if input.is_empty() {
            return Ok(None);
        }
        let n = js_number(input);
        if n.is_nan() {
            return Err(format_error("XXX", Some("KT")));
        }
        if n <= self.max && n >= self.min {
            Ok(Some(n))
        } else {
            Err(range_error(&js_num_str(self.min), &js_num_str(self.max), Some("KT")))
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AltitudeOrFlightLevelFormat {
    pub trans_alt: Option<f64>,
    pub trans_alt_is_fl: bool,
    pub min: f64,
    pub max: f64,
}

impl Default for AltitudeOrFlightLevelFormat {
    fn default() -> Self {
        AltitudeOrFlightLevelFormat { trans_alt: None, trans_alt_is_fl: false, min: 0.0, max: MAX_CERTIFIED_ALT }
    }
}

impl AltitudeOrFlightLevelFormat {
    pub fn new(trans_alt: Option<f64>, trans_alt_is_fl: bool) -> Self {
        AltitudeOrFlightLevelFormat { trans_alt, trans_alt_is_fl, ..Default::default() }
    }
    fn trans_alt_feet(&self) -> Option<f64> {
        self.trans_alt.map(|t| if self.trans_alt_is_fl { t * 100.0 } else { t })
    }
    pub fn format(&self, v: Option<f64>) -> Formatted {
        let Some(v) = v else { return out("-----", None, Some("FT")) };
        if let Some(t) = self.trans_alt_feet() {
            if (!self.trans_alt_is_fl && v > t) || (self.trans_alt_is_fl && v >= t) {
                return out(&pad3(js_to_fixed(v / 100.0, 0)), Some("FL"), None);
            }
        }
        out(&js_to_fixed(v, 0), None, Some("FT"))
    }
    pub fn parse(&self, input: &str) -> Result<Option<f64>, EntryError> {
        if input.is_empty() {
            return Ok(None);
        }
        let mut n = js_number(input);
        if input.chars().count() <= 3 {
            n = js_number(input) * 100.0;
        }
        if n.is_nan() {
            Err(format_error("FOR ALT XXXXX FOR FL XXX", None))
        } else if n > self.max || n < self.min {
            Err(EntryError::EntryOutOfRange(None))
        } else {
            Ok(Some(n))
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AltitudeFormat {
    pub min: f64,
    pub max: f64,
}

impl Default for AltitudeFormat {
    fn default() -> Self {
        AltitudeFormat { min: 0.0, max: MAX_CERTIFIED_ALT }
    }
}

impl AltitudeFormat {
    pub fn format(&self, v: Option<f64>) -> Formatted {
        match v {
            None => out("-----", None, Some("FT")),
            Some(v) => out(&js_to_fixed(v, 0), None, Some("FT")),
        }
    }
    pub fn parse(&self, input: &str) -> Result<Option<f64>, EntryError> {
        parse_range(input, self.min, self.max, || EntryError::EntryOutOfRange(None), || format_error("XXXXX", Some("FT")))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FlightLevelFormat {
    pub min: f64,
    pub max: f64,
}

impl Default for FlightLevelFormat {
    fn default() -> Self {
        FlightLevelFormat { min: 0.0, max: MAX_CERTIFIED_ALT / 100.0 }
    }
}

impl FlightLevelFormat {
    pub fn format(&self, v: Option<f64>) -> Formatted {
        match v {
            None => out("---", Some("FL"), None),
            Some(v) => out(&pad3(js_to_fixed(js_round(v), 0)), Some("FL"), None),
        }
    }
    pub fn parse(&self, input: &str) -> Result<Option<f64>, EntryError> {
        parse_flight_level(input, self.min, self.max)
    }
}

fn js_round(v: f64) -> f64 {
    (v + 0.5).floor()
}

pub fn parse_flight_level(input: &str, min: f64, max: f64) -> Result<Option<f64>, EntryError> {
    if input.is_empty() {
        return Ok(None);
    }
    let mut n = js_number(input);
    if n.is_nan() {
        let up = input.to_ascii_uppercase();
        if let Some(d) = up.strip_prefix("FL") {
            if (1..=3).contains(&d.len()) && d.bytes().all(|b| b.is_ascii_digit()) {
                n = js_number(d);
            }
        }
    }
    if n > max || n < min {
        Err(EntryError::EntryOutOfRange(None))
    } else if n.is_nan() {
        Err(format_error("FL XXX", None))
    } else {
        Ok(Some(n))
    }
}

pub struct TimeHhmmssFormat;

impl TimeHhmmssFormat {
    pub fn format(v: Option<f64>) -> Formatted {
        let v = match v {
            Some(v) if v != 0.0 && !v.is_nan() => v,
            _ => return out("--:--:--", None, None),
        };
        let h = pad2(js_to_fixed((v / 3600.0).floor().abs(), 0));
        let m = pad2(js_to_fixed(((v / 60.0).floor() % 60.0).abs(), 0));
        let s = pad2(js_to_fixed((v % 60.0).abs(), 0));
        out(&format!("{}:{}:{}", h, m, s), None, None)
    }
    pub fn parse(input: &str) -> Result<Option<f64>, EntryError> {
        if input.is_empty() {
            return Ok(None);
        }
        let r = input.replacen(':', "", 1);
        let chars: Vec<char> = r.chars().collect();
        if chars.len() < 4 {
            return Ok(None);
        }
        let sub = |a: usize, b: usize| js_number(&chars[a.min(chars.len())..b.min(chars.len())].iter().collect::<String>());
        let minutes = sub(2, 4);
        let hours = sub(0, 2);
        let seconds = if chars.len() > 4 { sub(chars.len() - 2, chars.len()) } else { 0.0 };
        if seconds < 0.0 || seconds > 59.0 || minutes < 0.0 || minutes > 59.0 || hours < 0.0 || hours > 23.0 {
            return Ok(None);
        }
        let n = seconds + minutes * 60.0 + hours * 3600.0;
        if !n.is_nan() && (0.0..=86400.0).contains(&n) {
            return Ok(Some(n));
        }
        if n > 86400.0 || n < 0.0 {
            Err(EntryError::EntryOutOfRange(None))
        } else {
            Err(format_error("HHMMSS", None))
        }
    }
}

fn pad2(s: String) -> String {
    format!("{:0>2}", s)
}

pub struct LatitudeFormat {
    pub placeholder: String,
}

impl Default for LatitudeFormat {
    fn default() -> Self {
        LatitudeFormat { placeholder: "----.--".into() }
    }
}

impl LatitudeFormat {
    pub fn format(&self, v: Option<f64>) -> Formatted {
        match v {
            None => out(&self.placeholder, None, None),
            Some(v) => out(&js_to_fixed(v, 0), None, None),
        }
    }
    pub fn parse(&self, input: &str) -> Result<Option<f64>, EntryError> {
        parse_range(input, -90.0, 90.0, || EntryError::EntryOutOfRange(None), || EntryError::FormatError(None))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frequency {
    Ils,
    VorDme,
    Adf,
}

impl Frequency {
    fn spec(self) -> (&'static str, &'static str, f64, f64, usize) {
        match self {
            Frequency::Ils => ("---.--", "XXX.XX", 108.0, 111.95, 2),
            Frequency::VorDme => ("---.--", "XXX.XX", 108.0, 117.95, 2),
            Frequency::Adf => ("----.-", "XXXX.X", 190.0, 1750.0, 1),
        }
    }
    pub fn format(self, v: Option<f64>) -> Formatted {
        let (ph, _, _, _, d) = self.spec();
        match v {
            None => out(ph, None, None),
            Some(v) => out(&js_to_fixed(v, d), None, None),
        }
    }
    pub fn parse(self, input: &str) -> Result<Option<f64>, EntryError> {
        let (_, fmt, min, max, d) = self.spec();
        parse_range(input, min, max, || range_error(&js_to_fixed(min, d), &js_to_fixed(max, d), None), || format_error(fmt, None))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldEntry {
    Heading,
    InboundCourse,
    HoldDist,
    HoldTime,
}

impl HoldEntry {
    fn spec(self) -> (&'static str, &'static str, &'static str, usize) {
        match self {
            HoldEntry::Heading => ("---.-", "XXX.X", "\u{b0}", 1),
            HoldEntry::InboundCourse => ("---", "XXX", "\u{b0}", 0),
            HoldEntry::HoldDist => ("--.-", "XX.X", "NM", 1),
            HoldEntry::HoldTime => ("-.-", "X.X", "MN", 1),
        }
    }
    pub fn format(self, v: Option<f64>) -> Formatted {
        let (ph, _, unit, d) = self.spec();
        let trailing = if self == HoldEntry::Heading { "\u{b0}T" } else { unit };
        match v {
            None => out(ph, None, Some(trailing)),
            Some(v) => out(&js_to_fixed(v, d), None, Some(trailing)),
        }
    }
    pub fn parse(self, input: &str, min: f64, max: f64) -> Result<Option<f64>, EntryError> {
        let (_, fmt, unit, d) = self.spec();
        let lo = if self == HoldEntry::Heading { format!("{:0>4}", js_to_fixed(min, d)) } else { js_to_fixed(min, d) };
        parse_range(input, min, max, || range_error(&lo, &js_to_fixed(max, d), Some(unit)), || format_error(fmt, Some(unit)))
    }
}

pub fn text_entry_format(v: &str, placeholder: &str) -> Formatted {
    out(if v.is_empty() { placeholder } else { v }, None, None)
}

pub fn text_entry_parse(input: &str, placeholder: &str) -> Option<String> {
    if input.is_empty() || input == placeholder {
        None
    } else {
        Some(input.to_string())
    }
}

pub fn airport_parse(input: &str) -> Result<Option<String>, EntryError> {
    if input.is_empty() || input == "----" {
        return Ok(None);
    }
    if input.chars().count() != 4 {
        return Err(format_error("AAAA", None));
    }
    Ok(Some(input.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_number_rules() {
        assert_eq!(js_number(" 12 "), 12.0);
        assert_eq!(js_number(".5"), 0.5);
        assert_eq!(js_number("0x10"), 16.0);
        assert!(js_number("inf").is_nan());
        assert!(js_number("FL350").is_nan());
        assert_eq!(js_number(""), 0.0);
    }

    #[test]
    fn to_fixed_rounds_like_js() {
        assert_eq!(js_to_fixed(2.5, 0), "3");
        assert_eq!(js_to_fixed(1.005, 2), "1.00");
        assert_eq!(js_to_fixed(117.95, 2), "117.95");
        assert_eq!(js_to_fixed(-0.4, 0), "0");
        assert_eq!(js_to_fixed(-1.5, 0), "-2");
        assert_eq!(js_to_fixed(9.96, 1), "10.0");
    }

    #[test]
    fn altitude_or_fl() {
        let f = AltitudeOrFlightLevelFormat::new(Some(10000.0), false);
        assert_eq!(f.format(None), out("-----", None, Some("FT")));
        assert_eq!(f.format(Some(10000.0)), out("10000", None, Some("FT")));
        assert_eq!(f.format(Some(10100.0)), out("101", Some("FL"), None));
        assert_eq!(f.format(Some(35000.0)), out("350", Some("FL"), None));
        let fl = AltitudeOrFlightLevelFormat::new(Some(100.0), true);
        assert_eq!(fl.format(Some(10000.0)), out("100", Some("FL"), None));
        assert_eq!(f.parse("350"), Ok(Some(35000.0)));
        assert_eq!(f.parse("5000"), Ok(Some(5000.0)));
        assert_eq!(f.parse("50000"), Err(EntryError::EntryOutOfRange(None)));
        assert_eq!(f.parse("ABC"), Err(EntryError::FormatError(Some("FORMAT: FOR ALT XXXXX FOR FL XXX".into()))));
        assert_eq!(f.parse(""), Ok(None));
    }

    #[test]
    fn flight_level() {
        let f = FlightLevelFormat::default();
        assert_eq!(f.format(Some(50.0)), out("050", Some("FL"), None));
        assert_eq!(f.parse("FL350"), Ok(Some(350.0)));
        assert_eq!(f.parse("fl35"), Ok(Some(35.0)));
        assert_eq!(f.parse("FL4500"), Err(EntryError::FormatError(Some("FORMAT: FL XXX".into()))));
        assert_eq!(f.parse("432"), Err(EntryError::EntryOutOfRange(None)));
    }

    #[test]
    fn speed_knots() {
        let f = SpeedKnotsFormat { min: 90.0, max: 340.0 };
        assert_eq!(f.format(Some(250.0)), out("250", None, Some("KT")));
        assert_eq!(f.format(None), out("---", None, Some("KT")));
        assert_eq!(f.parse("80"), Err(EntryError::EntryOutOfRange(Some("RNG: 90 TO 340 KT".into()))));
        assert_eq!(f.parse("X"), Err(EntryError::FormatError(Some("FORMAT: XXX KT".into()))));
    }

    #[test]
    fn frequencies() {
        assert_eq!(Frequency::VorDme.format(Some(116.7)), out("116.70", None, None));
        assert_eq!(Frequency::Adf.format(None), out("----.-", None, None));
        assert_eq!(Frequency::Ils.parse("109.9"), Ok(Some(109.9)));
        assert_eq!(Frequency::Ils.parse("115"), Err(EntryError::EntryOutOfRange(Some("RNG: 108.00 TO 111.95".into()))));
        assert_eq!(Frequency::Adf.parse("A"), Err(EntryError::FormatError(Some("FORMAT: XXXX.X".into()))));
    }

    #[test]
    fn time_and_latitude() {
        assert_eq!(TimeHhmmssFormat::format(None).value, "--:--:--");
        assert_eq!(TimeHhmmssFormat::format(Some(3725.0)).value, "01:02:05");
        assert_eq!(TimeHhmmssFormat::parse("0102"), Ok(Some(3720.0)));
        assert_eq!(TimeHhmmssFormat::parse("01:02:05"), Ok(Some(3725.0)));
        assert_eq!(TimeHhmmssFormat::parse("2460"), Ok(None));
        let l = LatitudeFormat::default();
        assert_eq!(l.format(None).value, "----.--");
        assert_eq!(l.parse("-91"), Err(EntryError::EntryOutOfRange(None)));
        assert_eq!(l.parse("N"), Err(EntryError::FormatError(None)));
    }

    #[test]
    fn hold_entries() {
        assert_eq!(HoldEntry::Heading.format(Some(90.0)), out("90.0", None, Some("\u{b0}T")));
        assert_eq!(HoldEntry::InboundCourse.format(None), out("---", None, Some("\u{b0}")));
        assert_eq!(HoldEntry::HoldTime.format(Some(1.0)), out("1.0", None, Some("MN")));
        assert_eq!(
            HoldEntry::HoldDist.parse("120", 0.0, 99.9),
            Err(EntryError::EntryOutOfRange(Some("RNG: 0.0 TO 99.9 NM".into())))
        );
        assert_eq!(
            HoldEntry::Heading.parse("400", 0.0, 360.0),
            Err(EntryError::EntryOutOfRange(Some("RNG: 00.0 TO 360.0 \u{b0}".into())))
        );
    }

    #[test]
    fn text_entries() {
        assert_eq!(text_entry_format("", "-------").value, "-------");
        assert_eq!(text_entry_parse("-------", "-------"), None);
        assert_eq!(airport_parse("YSY"), Err(EntryError::FormatError(Some("FORMAT: AAAA".into()))));
        assert_eq!(airport_parse("YSSY"), Ok(Some("YSSY".into())));
    }
}

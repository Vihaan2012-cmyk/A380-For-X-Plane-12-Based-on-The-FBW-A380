use crate::selector::split_top;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f64,
}

pub fn fmt_num(x: f64) -> String {
    if !x.is_finite() {
        return "0".into();
    }
    if (x - x.round()).abs() < 1e-9 {
        let r = x.round();
        return if r == 0.0 { "0".into() } else { format!("{}", r as i64) };
    }
    let mag = x.abs().log10().floor() as i32;
    let decimals = (5 - mag).clamp(0, 12) as usize;
    let s = format!("{:.*}", decimals, x);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

pub fn fmt_px(x: f64) -> String {
    format!("{}px", fmt_num(x))
}

pub fn fmt_color(c: Rgba) -> String {
    if c.a >= 1.0 {
        format!("rgb({}, {}, {})", c.r, c.g, c.b)
    } else {
        let a8 = (c.a * 255.0).round();
        let two = (a8 / 255.0 * 100.0).round() / 100.0;
        let a = if (two * 255.0).round() == a8 { two } else { (a8 / 255.0 * 1000.0).round() / 1000.0 };
        format!("rgba({}, {}, {}, {})", c.r, c.g, c.b, fmt_num(a))
    }
}

const NAMED: &[(&str, u32)] = &[
    ("black", 0x000000),
    ("white", 0xffffff),
    ("red", 0xff0000),
    ("green", 0x008000),
    ("blue", 0x0000ff),
    ("yellow", 0xffff00),
    ("cyan", 0x00ffff),
    ("aqua", 0x00ffff),
    ("magenta", 0xff00ff),
    ("fuchsia", 0xff00ff),
    ("lime", 0x00ff00),
    ("gray", 0x808080),
    ("grey", 0x808080),
    ("silver", 0xc0c0c0),
    ("maroon", 0x800000),
    ("olive", 0x808000),
    ("navy", 0x000080),
    ("purple", 0x800080),
    ("teal", 0x008080),
    ("orange", 0xffa500),
    ("pink", 0xffc0cb),
    ("brown", 0xa52a2a),
    ("darkgray", 0xa9a9a9),
    ("darkgrey", 0xa9a9a9),
    ("lightgray", 0xd3d3d3),
    ("lightgrey", 0xd3d3d3),
    ("dimgray", 0x696969),
    ("gold", 0xffd700),
    ("darkred", 0x8b0000),
    ("darkgreen", 0x006400),
    ("darkblue", 0x00008b),
    ("lightblue", 0xadd8e6),
    ("lightgreen", 0x90ee90),
    ("whitesmoke", 0xf5f5f5),
    ("gainsboro", 0xdcdcdc),
    ("orangered", 0xff4500),
    ("limegreen", 0x32cd32),
    ("royalblue", 0x4169e1),
    ("dodgerblue", 0x1e90ff),
    ("deepskyblue", 0x00bfff),
    ("crimson", 0xdc143c),
    ("tomato", 0xff6347),
    ("coral", 0xff7f50),
    ("indigo", 0x4b0082),
    ("violet", 0xee82ee),
    ("transparent_", 0),
];

fn hue(h: f64, m1: f64, m2: f64) -> f64 {
    let mut h = h;
    if h < 0.0 {
        h += 1.0;
    }
    if h > 1.0 {
        h -= 1.0;
    }
    if h * 6.0 < 1.0 {
        m1 + (m2 - m1) * h * 6.0
    } else if h * 2.0 < 1.0 {
        m2
    } else if h * 3.0 < 2.0 {
        m1 + (m2 - m1) * (2.0 / 3.0 - h) * 6.0
    } else {
        m1
    }
}

fn comp(s: &str, scale: f64) -> Option<f64> {
    let s = s.trim();
    if let Some(p) = s.strip_suffix('%') {
        p.trim().parse::<f64>().ok().map(|v| v / 100.0 * scale)
    } else {
        s.parse::<f64>().ok()
    }
}

pub fn parse_color(s: &str, current: Rgba) -> Option<Rgba> {
    let t = s.trim().to_ascii_lowercase();
    if t == "transparent" {
        return Some(Rgba { r: 0, g: 0, b: 0, a: 0.0 });
    }
    if t == "currentcolor" {
        return Some(current);
    }
    if let Some(h) = t.strip_prefix('#') {
        let d: Vec<u32> = h.chars().map(|c| c.to_digit(16)).collect::<Option<Vec<_>>>()?;
        return match d.len() {
            3 => Some(Rgba { r: (d[0] * 17) as u8, g: (d[1] * 17) as u8, b: (d[2] * 17) as u8, a: 1.0 }),
            4 => Some(Rgba { r: (d[0] * 17) as u8, g: (d[1] * 17) as u8, b: (d[2] * 17) as u8, a: (d[3] * 17) as f64 / 255.0 }),
            6 => Some(Rgba { r: (d[0] * 16 + d[1]) as u8, g: (d[2] * 16 + d[3]) as u8, b: (d[4] * 16 + d[5]) as u8, a: 1.0 }),
            8 => Some(Rgba { r: (d[0] * 16 + d[1]) as u8, g: (d[2] * 16 + d[3]) as u8, b: (d[4] * 16 + d[5]) as u8, a: (d[6] * 16 + d[7]) as f64 / 255.0 }),
            _ => None,
        };
    }
    for (n, v) in NAMED {
        if *n == t {
            return Some(Rgba { r: (v >> 16) as u8, g: (v >> 8) as u8, b: *v as u8, a: 1.0 });
        }
    }
    let open = t.find('(')?;
    let name = &t[..open];
    let close = t.rfind(')')?;
    let inner = &t[open + 1..close];
    let normalized = inner.replace(',', " ").replace('/', " / ");
    let toks: Vec<&str> = normalized.split_whitespace().collect();
    let mut args: Vec<&str> = Vec::new();
    let mut alpha: Option<&str> = None;
    let mut seen_slash = false;
    for tk in &toks {
        if *tk == "/" {
            seen_slash = true;
        } else if seen_slash {
            alpha = Some(tk);
        } else {
            args.push(tk);
        }
    }
    if alpha.is_none() && args.len() == 4 {
        alpha = Some(args.pop().unwrap());
    }
    let a = match alpha {
        Some(x) => comp(x, 1.0)?.clamp(0.0, 1.0),
        None => 1.0,
    };
    match name {
        "rgb" | "rgba" if args.len() == 3 => {
            let c = |x: &str| comp(x, 255.0).map(|v| v.round().clamp(0.0, 255.0) as u8);
            Some(Rgba { r: c(args[0])?, g: c(args[1])?, b: c(args[2])?, a })
        }
        "hsl" | "hsla" if args.len() == 3 => {
            let h = args[0].trim_end_matches("deg").parse::<f64>().ok()?;
            let sat = comp(args[1], 1.0)?.clamp(0.0, 1.0);
            let l = comp(args[2], 1.0)?.clamp(0.0, 1.0);
            let h = ((h % 360.0) + 360.0) % 360.0 / 360.0;
            let m2 = if l <= 0.5 { l * (sat + 1.0) } else { l + sat - l * sat };
            let m1 = l * 2.0 - m2;
            let f = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;
            Some(Rgba { r: f(hue(h + 1.0 / 3.0, m1, m2)), g: f(hue(h, m1, m2)), b: f(hue(h - 1.0 / 3.0, m1, m2)), a })
        }
        _ => None,
    }
}

pub struct Ctx {
    pub fs: f64,
    pub root_fs: f64,
    pub vw: f64,
    pub vh: f64,
}

pub fn split_unit(tok: &str) -> Option<(f64, String)> {
    let t = tok.trim();
    let bytes = t.as_bytes();
    let mut i = 0;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let ds = i;
    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
        i += 1;
    }
    if i == ds {
        return None;
    }
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') && i + 1 < bytes.len() && (bytes[i + 1].is_ascii_digit() || bytes[i + 1] == b'-' || bytes[i + 1] == b'+') {
        let mut j = i + 1;
        if bytes[j] == b'-' || bytes[j] == b'+' {
            j += 1;
        }
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        i = j;
    }
    let n: f64 = t[..i].parse().ok()?;
    Some((n, t[i..].to_ascii_lowercase()))
}

pub fn unit_px(n: f64, unit: &str, c: &Ctx) -> Option<f64> {
    Some(match unit {
        "px" | "" if unit == "px" || n == 0.0 => n,
        "em" => n * c.fs,
        "rem" => n * c.root_fs,
        "ex" => n * c.fs * 0.5,
        "ch" => n * c.fs * 0.5,
        "vw" | "svw" | "lvw" | "dvw" => n * c.vw / 100.0,
        "vh" | "svh" | "lvh" | "dvh" => n * c.vh / 100.0,
        "vmin" => n * c.vw.min(c.vh) / 100.0,
        "vmax" => n * c.vw.max(c.vh) / 100.0,
        "pt" => n * 96.0 / 72.0,
        "pc" => n * 16.0,
        "in" => n * 96.0,
        "cm" => n * 96.0 / 2.54,
        "mm" => n * 96.0 / 25.4,
        "q" => n * 96.0 / 101.6,
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug)]
struct Cv {
    v: f64,
    len: bool,
}

struct Calc<'a> {
    s: &'a [u8],
    i: usize,
    c: &'a Ctx,
}

impl<'a> Calc<'a> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn expr(&mut self) -> Option<Cv> {
        let mut a = self.term()?;
        loop {
            self.ws();
            match self.s.get(self.i) {
                Some(b'+') => {
                    self.i += 1;
                    let b = self.term()?;
                    a.v += b.v;
                    a.len |= b.len;
                }
                Some(b'-') => {
                    self.i += 1;
                    let b = self.term()?;
                    a.v -= b.v;
                    a.len |= b.len;
                }
                _ => return Some(a),
            }
        }
    }

    fn term(&mut self) -> Option<Cv> {
        let mut a = self.factor()?;
        loop {
            self.ws();
            match self.s.get(self.i) {
                Some(b'*') => {
                    self.i += 1;
                    let b = self.factor()?;
                    a.v *= b.v;
                    a.len |= b.len;
                }
                Some(b'/') => {
                    self.i += 1;
                    let b = self.factor()?;
                    if b.v == 0.0 {
                        return None;
                    }
                    a.v /= b.v;
                }
                _ => return Some(a),
            }
        }
    }

    fn factor(&mut self) -> Option<Cv> {
        self.ws();
        let c = *self.s.get(self.i)?;
        if c == b'(' {
            self.i += 1;
            let v = self.expr()?;
            self.ws();
            if self.s.get(self.i) == Some(&b')') {
                self.i += 1;
            }
            return Some(v);
        }
        let start = self.i;
        if c.is_ascii_alphabetic() {
            while self.i < self.s.len() && (self.s[self.i].is_ascii_alphabetic() || self.s[self.i] == b'-') {
                self.i += 1;
            }
            let name = std::str::from_utf8(&self.s[start..self.i]).ok()?.to_ascii_lowercase();
            self.ws();
            if self.s.get(self.i) != Some(&b'(') {
                return None;
            }
            self.i += 1;
            let mut args = vec![self.expr()?];
            loop {
                self.ws();
                match self.s.get(self.i) {
                    Some(b',') => {
                        self.i += 1;
                        args.push(self.expr()?);
                    }
                    Some(b')') => {
                        self.i += 1;
                        break;
                    }
                    _ => return None,
                }
            }
            let len = args.iter().any(|a| a.len);
            let v = match name.as_str() {
                "calc" => args[0].v,
                "min" => args.iter().map(|a| a.v).fold(f64::INFINITY, f64::min),
                "max" => args.iter().map(|a| a.v).fold(f64::NEG_INFINITY, f64::max),
                "clamp" if args.len() == 3 => args[1].v.max(args[0].v).min(args[2].v),
                _ => return None,
            };
            return Some(Cv { v, len });
        }
        while self.i < self.s.len() && (self.s[self.i].is_ascii_alphanumeric() || matches!(self.s[self.i], b'.' | b'%' | b'-' | b'+')) {
            if self.i > start && matches!(self.s[self.i], b'-' | b'+') && !matches!(self.s[self.i - 1], b'e' | b'E') {
                break;
            }
            self.i += 1;
        }
        let tok = std::str::from_utf8(&self.s[start..self.i]).ok()?;
        let (n, u) = split_unit(tok)?;
        if u.is_empty() {
            Some(Cv { v: n, len: false })
        } else {
            Some(Cv { v: unit_px(n, &u, self.c)?, len: true })
        }
    }
}

pub fn eval_calc(s: &str, c: &Ctx) -> Option<f64> {
    if s.contains('%') {
        return None;
    }
    let mut p = Calc { s: s.as_bytes(), i: 0, c };
    let v = p.expr()?;
    Some(v.v)
}

pub fn parse_length(tok: &str, c: &Ctx) -> Option<f64> {
    let t = tok.trim();
    let low = t.to_ascii_lowercase();
    if low.starts_with("calc(") || low.starts_with("min(") || low.starts_with("max(") || low.starts_with("clamp(") {
        return eval_calc(&low, c);
    }
    let (n, u) = split_unit(t)?;
    unit_px(n, &u, c)
}

pub fn tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0;
    let mut quote: Option<char> = None;
    for c in s.chars() {
        if let Some(q) = quote {
            cur.push(c);
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                quote = Some(c);
                cur.push(c);
            }
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            _ if c.is_whitespace() && depth == 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn commas(s: &str) -> Vec<String> {
    split_top(s, ',').into_iter().map(|x| x.trim().to_string()).collect()
}

pub fn subst_vars(value: &str, custom: &HashMap<String, String>, depth: u32) -> Option<String> {
    if depth > 16 {
        return None;
    }
    let lower = value.to_ascii_lowercase();
    let mut out = String::new();
    let mut pos = 0;
    while let Some(p) = lower[pos..].find("var(") {
        let start = pos + p;
        out.push_str(&value[pos..start]);
        let mut d = 1;
        let mut j = start + 4;
        let b = value.as_bytes();
        while j < b.len() && d > 0 {
            match b[j] {
                b'(' => d += 1,
                b')' => d -= 1,
                _ => {}
            }
            j += 1;
        }
        let inner = &value[start + 4..j - 1];
        let parts = split_top(inner, ',');
        let name = parts[0].trim();
        let fallback = if parts.len() > 1 { Some(inner[parts[0].len() + 1..].trim().to_string()) } else { None };
        let rep = match custom.get(name) {
            Some(v) => v.clone(),
            None => subst_vars(&fallback?, custom, depth + 1)?,
        };
        out.push_str(&rep);
        pos = j;
    }
    out.push_str(&value[pos..]);
    Some(out)
}

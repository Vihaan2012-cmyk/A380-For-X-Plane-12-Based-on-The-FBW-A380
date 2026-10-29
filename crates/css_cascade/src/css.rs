use crate::selector::{parse_selector_list, split_top, Selector};
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct Decl {
    pub name: String,
    pub value: String,
    pub important: bool,
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub sel: Selector,
    pub decls: Rc<Vec<Decl>>,
    pub order: u32,
    pub origin: u8,
}

pub fn strip_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut quote: Option<char> = None;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            out.push(c);
            if c == '\\' && i + 1 < b.len() {
                out.push(b[i + 1]);
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
            out.push(c);
            i += 1;
            continue;
        }
        if c == '/' && i + 1 < b.len() && b[i + 1] == '*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == '*' && b[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

pub fn parse_decls(block: &str) -> Vec<Decl> {
    let mut out = Vec::new();
    for part in split_top(block, ';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let colon = match part.find(':') {
            Some(c) => c,
            None => continue,
        };
        let name = part[..colon].trim();
        let mut value = part[colon + 1..].trim().to_string();
        let mut important = false;
        let lower = value.to_ascii_lowercase();
        if let Some(p) = lower.rfind("!important") {
            if lower[p + 10..].trim().is_empty() {
                important = true;
                value = value[..p].trim().to_string();
            }
        }
        let name = if name.starts_with("--") { name.to_string() } else { name.to_ascii_lowercase() };
        if name.is_empty() {
            continue;
        }
        out.push(Decl { name, value, important });
    }
    out
}

struct Scan<'a> {
    s: &'a [char],
    i: usize,
}

impl<'a> Scan<'a> {
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_whitespace() {
            self.i += 1;
        }
    }

    fn until(&mut self, stops: &[char]) -> String {
        let mut out = String::new();
        let mut depth = 0i32;
        let mut quote: Option<char> = None;
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if let Some(q) = quote {
                out.push(c);
                self.i += 1;
                if c == '\\' && self.i < self.s.len() {
                    out.push(self.s[self.i]);
                    self.i += 1;
                    continue;
                }
                if c == q {
                    quote = None;
                }
                continue;
            }
            if c == '\\' {
                out.push(c);
                self.i += 1;
                if self.i < self.s.len() {
                    out.push(self.s[self.i]);
                    self.i += 1;
                }
                continue;
            }
            if c == '"' || c == '\'' {
                quote = Some(c);
            } else if c == '(' || c == '[' {
                depth += 1;
            } else if c == ')' || c == ']' {
                depth -= 1;
            } else if depth <= 0 && stops.contains(&c) {
                break;
            }
            out.push(c);
            self.i += 1;
        }
        out
    }

    fn block(&mut self) -> String {
        let mut depth = 1;
        let mut out = String::new();
        let mut quote: Option<char> = None;
        while self.i < self.s.len() {
            let c = self.s[self.i];
            self.i += 1;
            if let Some(q) = quote {
                out.push(c);
                if c == '\\' && self.i < self.s.len() {
                    out.push(self.s[self.i]);
                    self.i += 1;
                    continue;
                }
                if c == q {
                    quote = None;
                }
                continue;
            }
            match c {
                '"' | '\'' => {
                    quote = Some(c);
                    out.push(c);
                }
                '{' => {
                    depth += 1;
                    out.push(c);
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return out;
                    }
                    out.push(c);
                }
                _ => out.push(c),
            }
        }
        out
    }
}

pub struct Viewport {
    pub w: f64,
    pub h: f64,
}

fn media_len(s: &str) -> Option<f64> {
    let s = s.trim();
    if let Some(v) = s.strip_suffix("px") {
        return v.trim().parse().ok();
    }
    if let Some(v) = s.strip_suffix("em") {
        return v.trim().parse::<f64>().ok().map(|x| x * 16.0);
    }
    s.parse().ok()
}

fn media_feature(f: &str, vp: &Viewport) -> bool {
    let f = f.trim().trim_start_matches('(').trim_end_matches(')').trim();
    let (name, val) = match f.find(':') {
        Some(c) => (f[..c].trim().to_ascii_lowercase(), f[c + 1..].trim().to_string()),
        None => return matches!(f.to_ascii_lowercase().as_str(), "color" | "hover"),
    };
    let v = media_len(&val);
    match name.as_str() {
        "min-width" => v.map(|x| vp.w >= x).unwrap_or(false),
        "max-width" => v.map(|x| vp.w <= x).unwrap_or(false),
        "min-height" => v.map(|x| vp.h >= x).unwrap_or(false),
        "max-height" => v.map(|x| vp.h <= x).unwrap_or(false),
        "orientation" => (val == "landscape") == (vp.w >= vp.h),
        "prefers-color-scheme" => val == "dark",
        "prefers-reduced-motion" => val == "no-preference",
        "hover" => val == "hover",
        _ => false,
    }
}

pub fn media_matches(q: &str, vp: &Viewport) -> bool {
    for one in q.split(',') {
        let mut t = one.trim().to_ascii_lowercase();
        let mut neg = false;
        if let Some(r) = t.strip_prefix("not ") {
            neg = true;
            t = r.to_string();
        }
        if let Some(r) = t.strip_prefix("only ") {
            t = r.to_string();
        }
        let mut ok = true;
        for part in t.split(" and ") {
            let part = part.trim();
            if part.starts_with('(') {
                ok &= media_feature(part, vp);
            } else {
                ok &= matches!(part, "all" | "screen" | "");
            }
        }
        if ok != neg {
            return true;
        }
    }
    false
}

pub fn parse_sheet(text: &str, origin: u8, vp: &Viewport, order: &mut u32, out: &mut Vec<Rule>) {
    let clean = strip_comments(text);
    let chars: Vec<char> = clean.chars().collect();
    let mut sc = Scan { s: &chars, i: 0 };
    parse_block(&mut sc, origin, vp, order, out);
}

fn parse_block(sc: &mut Scan, origin: u8, vp: &Viewport, order: &mut u32, out: &mut Vec<Rule>) {
    loop {
        sc.skip_ws();
        if sc.i >= sc.s.len() {
            return;
        }
        if sc.s[sc.i] == '@' {
            sc.i += 1;
            let mut name = String::new();
            while sc.i < sc.s.len() && (sc.s[sc.i].is_alphanumeric() || sc.s[sc.i] == '-') {
                name.push(sc.s[sc.i]);
                sc.i += 1;
            }
            let prelude = sc.until(&['{', ';']);
            if sc.i >= sc.s.len() {
                return;
            }
            if sc.s[sc.i] == ';' {
                sc.i += 1;
                continue;
            }
            sc.i += 1;
            let body = sc.block();
            let name = name.to_ascii_lowercase();
            let inner = |out: &mut Vec<Rule>, order: &mut u32| {
                let chars: Vec<char> = body.chars().collect();
                let mut s2 = Scan { s: &chars, i: 0 };
                parse_block(&mut s2, origin, vp, order, out);
            };
            match name.as_str() {
                "media" => {
                    if media_matches(prelude.trim(), vp) {
                        inner(out, order);
                    }
                }
                "layer" | "supports" | "document" | "container" => {
                    if name != "container" {
                        inner(out, order);
                    }
                }
                _ => {}
            }
            continue;
        }
        let prelude = sc.until(&['{']);
        if sc.i >= sc.s.len() {
            return;
        }
        sc.i += 1;
        let body = sc.block();
        let decls = Rc::new(parse_decls(&body));
        if let Some(list) = parse_selector_list(prelude.trim()) {
            for sel in list {
                *order += 1;
                out.push(Rule { sel, decls: decls.clone(), order: *order, origin });
            }
        } else if prelude.contains(',') {
            for part in split_top(prelude.trim(), ',') {
                if let Some(mut l) = parse_selector_list(part.trim()) {
                    if let Some(sel) = l.pop() {
                        *order += 1;
                        out.push(Rule { sel, decls: decls.clone(), order: *order, origin });
                    }
                }
            }
        }
    }
}

use crate::dom::Doc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Comb {
    Descendant,
    Child,
    Adjacent,
    Sibling,
}

#[derive(Clone, Debug)]
pub enum AttrOp {
    Exists,
    Eq,
    Includes,
    Dash,
    Prefix,
    Suffix,
    Substr,
}

#[derive(Clone, Debug)]
pub struct AttrSel {
    pub name: String,
    pub op: AttrOp,
    pub value: String,
    pub ci: bool,
}

#[derive(Clone, Debug)]
pub enum Pseudo {
    Not(Vec<Selector>),
    Is(Vec<Selector>),
    Nth { a: i32, b: i32, last: bool, of_type: bool },
    OnlyChild,
    OnlyOfType,
    Empty,
    Root,
    Checked,
    Disabled,
    Enabled,
    Hover,
    Never,
}

#[derive(Clone, Debug, Default)]
pub struct Compound {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attrs: Vec<AttrSel>,
    pub pseudos: Vec<Pseudo>,
}

#[derive(Clone, Debug, Default)]
pub struct Selector {
    pub compounds: Vec<Compound>,
    pub combs: Vec<Comb>,
    pub spec: u32,
}

pub fn pack(a: u32, b: u32, c: u32) -> u32 {
    (a.min(255) << 20) | (b.min(1023) << 10) | c.min(1023)
}

fn unpack(s: u32) -> (u32, u32, u32) {
    (s >> 20, (s >> 10) & 1023, s & 1023)
}

pub fn split_top(s: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut esc = false;
    for c in s.chars() {
        if esc {
            cur.push(c);
            esc = false;
            continue;
        }
        if c == '\\' {
            cur.push(c);
            esc = true;
            continue;
        }
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
            '(' | '[' => {
                depth += 1;
                cur.push(c);
            }
            ')' | ']' => {
                depth -= 1;
                cur.push(c);
            }
            _ if c == sep && depth == 0 => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

struct P<'a> {
    s: &'a [char],
    i: usize,
}

impl<'a> P<'a> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn ident(&mut self) -> String {
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '-' || c == '_' || !c.is_ascii() {
                out.push(c);
                self.i += 1;
            } else if c == '\\' {
                self.i += 1;
                let mut hex = String::new();
                while let Some(h) = self.peek() {
                    if h.is_ascii_hexdigit() && hex.len() < 6 {
                        hex.push(h);
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                if hex.is_empty() {
                    if let Some(c) = self.peek() {
                        out.push(c);
                        self.i += 1;
                    }
                } else {
                    if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                        out.push(ch);
                    }
                    if self.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
                        self.i += 1;
                    }
                }
            } else {
                break;
            }
        }
        out
    }

    fn balanced(&mut self) -> String {
        let mut depth = 1;
        let mut out = String::new();
        let mut quote: Option<char> = None;
        while let Some(c) = self.peek() {
            self.i += 1;
            if let Some(q) = quote {
                out.push(c);
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
                '(' => {
                    depth += 1;
                    out.push(c);
                }
                ')' => {
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

fn parse_an_b(s: &str) -> Option<(i32, i32)> {
    let t: String = s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
    match t.as_str() {
        "odd" => return Some((2, 1)),
        "even" => return Some((2, 0)),
        _ => {}
    }
    if let Some(n) = t.find('n') {
        let a = &t[..n];
        let a = match a {
            "" | "+" => 1,
            "-" => -1,
            x => x.parse().ok()?,
        };
        let rest = &t[n + 1..];
        let b = if rest.is_empty() { 0 } else { rest.trim_start_matches('+').parse().ok()? };
        Some((a, b))
    } else {
        Some((0, t.trim_start_matches('+').parse().ok()?))
    }
}

pub fn parse_selector_list(s: &str) -> Option<Vec<Selector>> {
    let mut out = Vec::new();
    for part in split_top(s, ',') {
        let part = part.trim();
        if part.is_empty() {
            return None;
        }
        out.push(parse_selector(part)?);
    }
    Some(out)
}

fn parse_forgiving(s: &str) -> Vec<Selector> {
    split_top(s, ',').iter().filter_map(|p| parse_selector(p.trim())).collect()
}

pub fn parse_selector(s: &str) -> Option<Selector> {
    let chars: Vec<char> = s.chars().collect();
    let mut p = P { s: &chars, i: 0 };
    let mut sel = Selector::default();
    let mut cur = Compound::default();
    let mut any = false;
    let (mut sa, mut sb, mut sc) = (0u32, 0u32, 0u32);
    let mut pending: Option<Comb> = None;
    loop {
        let mut ws = false;
        while p.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
            p.i += 1;
            ws = true;
        }
        let c = match p.peek() {
            Some(c) => c,
            None => break,
        };
        if matches!(c, '>' | '+' | '~') {
            if !any {
                return None;
            }
            pending = Some(match c {
                '>' => Comb::Child,
                '+' => Comb::Adjacent,
                _ => Comb::Sibling,
            });
            p.i += 1;
            sel.compounds.push(std::mem::take(&mut cur));
            sel.combs.push(pending.unwrap());
            any = false;
            continue;
        }
        if ws && any {
            sel.compounds.push(std::mem::take(&mut cur));
            sel.combs.push(Comb::Descendant);
            any = false;
        }
        match c {
            '*' => {
                p.i += 1;
                any = true;
            }
            '#' => {
                p.i += 1;
                cur.id = Some(p.ident());
                sa += 1;
                any = true;
            }
            '.' => {
                p.i += 1;
                cur.classes.push(p.ident());
                sb += 1;
                any = true;
            }
            '[' => {
                p.i += 1;
                let body = p.balanced_bracket();
                cur.attrs.push(parse_attr(&body)?);
                sb += 1;
                any = true;
            }
            ':' => {
                p.i += 1;
                if p.peek() == Some(':') {
                    return None;
                }
                let name = p.ident().to_ascii_lowercase();
                let arg = if p.peek() == Some('(') {
                    p.i += 1;
                    Some(p.balanced())
                } else {
                    None
                };
                any = true;
                match (name.as_str(), arg) {
                    ("before" | "after" | "first-line" | "first-letter", _) => return None,
                    ("not", Some(a)) => {
                        let list = parse_forgiving(&a);
                        let mx = list.iter().map(|x| x.spec).max().unwrap_or(0);
                        let (a1, b1, c1) = unpack(mx);
                        sa += a1;
                        sb += b1;
                        sc += c1;
                        cur.pseudos.push(Pseudo::Not(list));
                    }
                    ("is" | "matches" | "-webkit-any" | "any", Some(a)) => {
                        let list = parse_forgiving(&a);
                        let mx = list.iter().map(|x| x.spec).max().unwrap_or(0);
                        let (a1, b1, c1) = unpack(mx);
                        sa += a1;
                        sb += b1;
                        sc += c1;
                        cur.pseudos.push(Pseudo::Is(list));
                    }
                    ("where", Some(a)) => {
                        cur.pseudos.push(Pseudo::Is(parse_forgiving(&a)));
                    }
                    ("first-child", _) => {
                        sb += 1;
                        cur.pseudos.push(Pseudo::Nth { a: 0, b: 1, last: false, of_type: false });
                    }
                    ("last-child", _) => {
                        sb += 1;
                        cur.pseudos.push(Pseudo::Nth { a: 0, b: 1, last: true, of_type: false });
                    }
                    ("first-of-type", _) => {
                        sb += 1;
                        cur.pseudos.push(Pseudo::Nth { a: 0, b: 1, last: false, of_type: true });
                    }
                    ("last-of-type", _) => {
                        sb += 1;
                        cur.pseudos.push(Pseudo::Nth { a: 0, b: 1, last: true, of_type: true });
                    }
                    ("nth-child" | "nth-last-child" | "nth-of-type" | "nth-last-of-type", Some(a)) => {
                        let a = a.split(" of ").next().unwrap_or("").to_string();
                        let (x, y) = parse_an_b(&a)?;
                        sb += 1;
                        cur.pseudos.push(Pseudo::Nth { a: x, b: y, last: name.contains("last"), of_type: name.ends_with("of-type") });
                    }
                    ("only-child", _) => {
                        sb += 1;
                        cur.pseudos.push(Pseudo::OnlyChild);
                    }
                    ("only-of-type", _) => {
                        sb += 1;
                        cur.pseudos.push(Pseudo::OnlyOfType);
                    }
                    ("empty", _) => {
                        sb += 1;
                        cur.pseudos.push(Pseudo::Empty);
                    }
                    ("root", _) => {
                        sb += 1;
                        cur.pseudos.push(Pseudo::Root);
                    }
                    ("checked", _) => {
                        sb += 1;
                        cur.pseudos.push(Pseudo::Checked);
                    }
                    ("disabled", _) => {
                        sb += 1;
                        cur.pseudos.push(Pseudo::Disabled);
                    }
                    ("enabled", _) => {
                        sb += 1;
                        cur.pseudos.push(Pseudo::Enabled);
                    }
                    ("hover", _) => {
                        sb += 1;
                        cur.pseudos.push(Pseudo::Hover);
                    }
                    _ => {
                        sb += 1;
                        cur.pseudos.push(Pseudo::Never);
                    }
                }
            }
            _ => {
                let id = p.ident();
                if id.is_empty() {
                    return None;
                }
                cur.tag = Some(id.to_ascii_lowercase());
                sc += 1;
                any = true;
            }
        }
    }
    if !any || pending.is_some() && !any {
        return None;
    }
    sel.compounds.push(cur);
    sel.spec = pack(sa, sb, sc);
    Some(sel)
}

impl<'a> P<'a> {
    fn balanced_bracket(&mut self) -> String {
        let mut out = String::new();
        let mut quote: Option<char> = None;
        while let Some(c) = self.peek() {
            self.i += 1;
            if let Some(q) = quote {
                out.push(c);
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
                ']' => return out,
                _ => out.push(c),
            }
        }
        out
    }
}

fn parse_attr(body: &str) -> Option<AttrSel> {
    let body = body.trim();
    let ops = ["~=", "|=", "^=", "$=", "*=", "="];
    for op in ops {
        if let Some(p) = body.find(op) {
            let name = body[..p].trim().to_string();
            let mut v = body[p + op.len()..].trim().to_string();
            let mut ci = false;
            if v.ends_with(" i") || v.ends_with(" I") {
                ci = true;
                v.truncate(v.len() - 2);
                v = v.trim().to_string();
            }
            let v = v.trim_matches(|c| c == '"' || c == '\'').to_string();
            let op = match op {
                "~=" => AttrOp::Includes,
                "|=" => AttrOp::Dash,
                "^=" => AttrOp::Prefix,
                "$=" => AttrOp::Suffix,
                "*=" => AttrOp::Substr,
                _ => AttrOp::Eq,
            };
            return Some(AttrSel { name, op, value: v, ci });
        }
    }
    Some(AttrSel { name: body.to_string(), op: AttrOp::Exists, value: String::new(), ci: false })
}

fn attr_match(doc: &Doc, el: usize, a: &AttrSel) -> bool {
    let v = match doc.attr(el, &a.name) {
        Some(v) => v,
        None => return false,
    };
    let (v, w) = if a.ci { (v.to_ascii_lowercase(), a.value.to_ascii_lowercase()) } else { (v.to_string(), a.value.clone()) };
    match a.op {
        AttrOp::Exists => true,
        AttrOp::Eq => v == w,
        AttrOp::Includes => v.split_whitespace().any(|x| x == w),
        AttrOp::Dash => v == w || v.starts_with(&format!("{}-", w)),
        AttrOp::Prefix => !w.is_empty() && v.starts_with(&w),
        AttrOp::Suffix => !w.is_empty() && v.ends_with(&w),
        AttrOp::Substr => !w.is_empty() && v.contains(&w),
    }
}

fn nth_ok(a: i32, b: i32, pos: i32) -> bool {
    if a == 0 {
        return pos == b;
    }
    let d = pos - b;
    d % a == 0 && d / a >= 0
}

fn sib_pos(doc: &Doc, el: usize, last: bool, of_type: bool) -> i32 {
    let e = &doc.els[el];
    let par = match e.parent {
        Some(p) => p,
        None => return 1,
    };
    let kids = &doc.els[par].kids;
    let mut n = 0;
    let range: Vec<usize> = if last { kids.iter().rev().copied().collect() } else { kids.clone() };
    for k in range {
        if of_type && !doc.els[k].tag.eq_ignore_ascii_case(&e.tag) {
            continue;
        }
        n += 1;
        if k == el {
            return n;
        }
    }
    n
}

fn compound_match(doc: &Doc, el: usize, c: &Compound) -> bool {
    let e = &doc.els[el];
    if let Some(t) = &c.tag {
        if !e.tag.eq_ignore_ascii_case(t) {
            return false;
        }
    }
    if let Some(id) = &c.id {
        if e.id.as_deref() != Some(id.as_str()) {
            return false;
        }
    }
    for cl in &c.classes {
        if !e.classes.iter().any(|x| x == cl) {
            return false;
        }
    }
    for a in &c.attrs {
        if !attr_match(doc, el, a) {
            return false;
        }
    }
    for ps in &c.pseudos {
        let ok = match ps {
            Pseudo::Not(list) => !list.iter().any(|s| matches(doc, el, s)),
            Pseudo::Is(list) => list.iter().any(|s| matches(doc, el, s)),
            Pseudo::Nth { a, b, last, of_type } => nth_ok(*a, *b, sib_pos(doc, el, *last, *of_type)),
            Pseudo::OnlyChild => e.parent.map(|p| doc.els[p].kids.len() == 1).unwrap_or(true),
            Pseudo::OnlyOfType => e.parent.map(|p| doc.els[p].kids.iter().filter(|&&k| doc.els[k].tag.eq_ignore_ascii_case(&e.tag)).count() == 1).unwrap_or(true),
            Pseudo::Empty => !e.has_text,
            Pseudo::Root => e.parent.is_none(),
            Pseudo::Checked => doc.has_attr(el, "checked") || doc.has_attr(el, "selected"),
            Pseudo::Disabled => doc.has_attr(el, "disabled"),
            Pseudo::Enabled => !doc.has_attr(el, "disabled") && matches!(e.tag.as_str(), "input" | "button" | "select" | "textarea"),
            Pseudo::Hover => doc.has_attr(el, "data-hover"),
            Pseudo::Never => false,
        };
        if !ok {
            return false;
        }
    }
    true
}

fn match_from(doc: &Doc, el: usize, sel: &Selector, ci: usize) -> bool {
    if !compound_match(doc, el, &sel.compounds[ci]) {
        return false;
    }
    if ci == 0 {
        return true;
    }
    let comb = sel.combs[ci - 1];
    let e = &doc.els[el];
    match comb {
        Comb::Child => e.parent.map(|p| match_from(doc, p, sel, ci - 1)).unwrap_or(false),
        Comb::Descendant => {
            let mut a = e.parent;
            while let Some(p) = a {
                if match_from(doc, p, sel, ci - 1) {
                    return true;
                }
                a = doc.els[p].parent;
            }
            false
        }
        Comb::Adjacent => doc.prev_sibling(el).map(|s| match_from(doc, s, sel, ci - 1)).unwrap_or(false),
        Comb::Sibling => {
            let mut s = doc.prev_sibling(el);
            while let Some(x) = s {
                if match_from(doc, x, sel, ci - 1) {
                    return true;
                }
                s = doc.prev_sibling(x);
            }
            false
        }
    }
}

pub fn matches(doc: &Doc, el: usize, sel: &Selector) -> bool {
    match_from(doc, el, sel, sel.compounds.len() - 1)
}

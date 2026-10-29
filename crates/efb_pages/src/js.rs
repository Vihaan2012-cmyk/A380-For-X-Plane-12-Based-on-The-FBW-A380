use crate::Node;
use mfd_tree::Shape;
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub use crate::ground::{js_fixed, squircle};

pub fn el(tag: &str, classes: &str) -> Node {
    Node::el(tag, classes)
}

/// `String(x)` for a JS number.
pub fn num(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if v == 0.0 {
        return "0".into();
    }
    let a = v.abs();
    if (1e-7..1e21).contains(&a) {
        return format!("{v}");
    }
    let s = format!("{v:e}");
    let (m, e) = s.split_once('e').unwrap();
    let e: i32 = e.parse().unwrap();
    format!("{m}e{}{}", if e >= 0 { "+" } else { "" }, e)
}

pub fn round(x: f64) -> f64 {
    (x + 0.5).floor()
}

pub fn pad2(n: f64) -> String {
    format!("{:02}", n.floor() as i64)
}

/// `toLocaleString('en-US')` with no fraction digits.
pub fn locale_int(v: f64) -> String {
    let r = round(v);
    let s = format!("{}", r.abs() as i64);
    let g = crate::fmt::group_thousands(&s);
    if r < 0.0 { format!("-{g}") } else { g }
}

const PUNCT: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";

fn primary(c: char) -> (u8, u32) {
    if c.is_whitespace() {
        return (0, c as u32);
    }
    if let Some(i) = PUNCT.find(c) {
        return (1, i as u32);
    }
    if c.is_ascii_digit() {
        return (3, c as u32);
    }
    if c.is_alphabetic() {
        let base = c.to_lowercase().next().unwrap_or(c);
        let base = match base {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            x => x,
        };
        return (4, base as u32);
    }
    (2, c as u32)
}

/// `a.localeCompare(b)` under the root collation, for the ASCII titles the EFB sorts.
pub fn locale_cmp(a: &str, b: &str) -> Ordering {
    let pa: Vec<(u8, u32)> = a.chars().map(primary).collect();
    let pb: Vec<(u8, u32)> = b.chars().map(primary).collect();
    pa.cmp(&pb).then_with(|| {
        let ta: Vec<u8> = a.chars().map(|c| u8::from(c.is_uppercase())).collect();
        let tb: Vec<u8> = b.chars().map(|c| u8::from(c.is_uppercase())).collect();
        ta.cmp(&tb)
    })
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;")
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&")
}

fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i] as char).is_ascii_whitespace() {
            i += 1;
        }
        let k0 = i;
        while i < b.len() && b[i] != b'=' && !(b[i] as char).is_ascii_whitespace() {
            i += 1;
        }
        if k0 == i {
            break;
        }
        let key = s[k0..i].to_string();
        if i < b.len() && b[i] == b'=' {
            i += 1;
            let q = b[i];
            if q == b'"' || q == b'\'' {
                let v0 = i + 1;
                let end = s[v0..].find(q as char).map(|p| v0 + p).unwrap_or(b.len());
                out.push((key, unescape(&s[v0..end])));
                i = end + 1;
            } else {
                let v0 = i;
                while i < b.len() && !(b[i] as char).is_ascii_whitespace() {
                    i += 1;
                }
                out.push((key, unescape(&s[v0..i])));
            }
        } else {
            out.push((key, String::new()));
        }
    }
    out
}

const VOID: [&str; 2] = ["br", "img"];

/// Parses SVG markup (as the page builds it with strings) into an `svg` node with flat shapes, as dom2tree reads it.
pub fn svg(markup: &str) -> Node {
    let mut root = Node::new("svg");
    let mut depth: i64 = -1;
    let mut rest = markup;
    while let Some(lt) = rest.find('<') {
        rest = &rest[lt + 1..];
        let gt = rest.find('>').unwrap_or(rest.len());
        let tag_src = &rest[..gt];
        rest = &rest[(gt + 1).min(rest.len())..];
        if let Some(_close) = tag_src.strip_prefix('/') {
            depth -= 1;
            continue;
        }
        let self_close = tag_src.ends_with('/');
        let body = tag_src.trim_end_matches('/');
        let (name, attrs) = match body.find(|c: char| c.is_ascii_whitespace()) {
            Some(p) => (&body[..p], &body[p..]),
            None => (body, ""),
        };
        if name == "style" && !self_close {
            rest = rest.find("</style>").map(|p| &rest[p + 8..]).unwrap_or("");
            continue;
        }
        let name = name.to_string();
        let attrs = parse_attrs(attrs);
        if depth < 0 {
            for (k, v) in attrs {
                match k.as_str() {
                    "class" => root.cls = v.split_whitespace().map(str::to_string).collect(),
                    "xmlns" => {}
                    _ => {
                        root.svgattr.insert(k, v);
                    }
                }
            }
            depth = 0;
            continue;
        }
        let mut a: BTreeMap<String, String> = attrs.into_iter().collect();
        a.insert("_d".into(), depth.to_string());
        if name == "text" && !self_close {
            let t = &rest[..rest.find('<').unwrap_or(rest.len())];
            if !t.is_empty() {
                a.insert("_text".into(), unescape(t));
            }
        }
        root.shapes.push(Shape { t: name.to_ascii_lowercase(), a });
        if !self_close && !VOID.contains(&name.as_str()) {
            depth += 1;
        }
    }
    root
}

/// A `select`: the options travel as data (dom2tree keeps no option children), the shown label as `data-label`.
pub fn select(options: &[(&str, &str)], act: &str, value: &str) -> Node {
    let label = match options.iter().find(|(k, _)| *k == value) {
        Some((_, l)) => *l,
        None if !value.is_empty() => value,
        None => options.first().map(|(_, l)| *l).unwrap_or(""),
    };
    let opts = options.iter().map(|(k, l)| format!("{k}={l}")).collect::<Vec<_>>().join("|");
    Node::new("select").act(act).data("value", value).data("label", label).data("options", &opts)
}

/// A `div` whose innerHTML is the given SVG markup.
pub fn svg_div(classes: &str, markup: &str) -> Node {
    el("div", classes).child(svg(markup))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_print_like_js() {
        assert_eq!(num(1.0), "1");
        assert_eq!(num(-0.0), "0");
        assert_eq!(num(26.000000000000004), "26.000000000000004");
        assert_eq!(num(1e21), "1e+21");
        assert_eq!(js_fixed(0.25, 1), "0.3");
        assert_eq!(locale_int(1234567.4), "1,234,567");
    }

    #[test]
    fn collation_puts_punctuation_and_digits_first() {
        assert_eq!(locale_cmp("ADIRU probe", "Adiru probe"), Ordering::Greater);
        assert_eq!(locale_cmp("Air pack", "air pack 2"), Ordering::Less);
        assert_eq!(locale_cmp("A-B", "AB"), Ordering::Less);
        assert_eq!(locale_cmp("B 2", "B1"), Ordering::Less);
    }

    #[test]
    fn svg_markup_becomes_shapes() {
        let n = svg(r#"<svg viewBox="0 0 10 10" class="qd-phases"><g transform="x"><rect x="1" y="2"/><text x="3">A &amp; B</text></g><path d="M0,0"/></svg>"#);
        assert_eq!(n.cls, vec!["qd-phases"]);
        assert_eq!(n.svgattr["viewBox"], "0 0 10 10");
        let t: Vec<(&str, &str)> = n.shapes.iter().map(|s| (s.t.as_str(), s.a["_d"].as_str())).collect();
        assert_eq!(t, vec![("g", "0"), ("rect", "1"), ("text", "1"), ("path", "0")]);
    }
}

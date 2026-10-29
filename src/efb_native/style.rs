use css_cascade::selector::{matches, parse_selector, split_top, Selector};
use css_cascade::Doc;

pub const EFB_CSS: &str = include_str!("efb.css");
pub const NATIVE_CSS: &str = include_str!("native.css");

pub const BEFORE_TAG: &str = "efbn-before";
pub const AFTER_TAG: &str = "efbn-after";

/// Every stylesheet the live EFB page uses: its `<style>` blocks and the CSS
/// its scripts inject (template literals assigned to `textContent` or to a
/// `css` constant, and single-quoted `textContent` strings).
pub fn extract_css(index_html: &str) -> String {
    let mut out = String::new();
    let mut rest = index_html;
    while let Some(at) = rest.find("<style") {
        let after = &rest[at..];
        let Some(open) = after.find('>') else { break };
        let body = &after[open + 1..];
        let Some(close) = body.find("</style>") else { break };
        out.push_str(&body[..close]);
        out.push('\n');
        rest = &body[close..];
    }
    for marker in ["textContent = `", "const css = `"] {
        let mut rest = index_html;
        while let Some(at) = rest.find(marker) {
            let body = &rest[at + marker.len()..];
            let end = literal_end(body, '`');
            let text = &body[..end];
            if !text.contains("${") {
                out.push_str(text);
                out.push('\n');
            }
            rest = &body[end.min(body.len())..];
        }
    }
    let marker = "textContent = '";
    let mut rest = index_html;
    while let Some(at) = rest.find(marker) {
        let body = &rest[at + marker.len()..];
        let end = literal_end(body, '\'');
        let text = &body[..end];
        if text.contains('{') && text.contains('}') {
            out.push_str(text);
            out.push('\n');
        }
        rest = &body[end.min(body.len())..];
    }
    out
}

fn literal_end(s: &str, quote: char) -> usize {
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == quote {
            return i;
        }
    }
    s.len()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Before,
    After,
}

#[derive(Clone, Debug)]
pub struct PseudoRule {
    pub base: Selector,
    pub which: Which,
    pub content: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Sheet {
    pub css: String,
    pub pseudo: Vec<PseudoRule>,
}

/// `X::after { .. }` becomes `X > efbn-after { .. }`, and the elements that
/// match `X` get a real `efbn-after` child when the document is built, so
/// css_cascade (which has no pseudo-elements) styles them like Chromium does.
pub fn prepare(css: &str) -> Sheet {
    let clean = css_cascade::css::strip_comments(css);
    let mut sheet = Sheet::default();
    rewrite_block(&clean, &mut sheet);
    sheet
}

fn rewrite_block(text: &str, sheet: &mut Sheet) {
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let start = i;
        let mut depth_paren = 0i32;
        let mut quote: Option<u8> = None;
        while i < b.len() {
            let c = b[i];
            match quote {
                Some(q) if c == q => quote = None,
                Some(_) => {}
                None => match c {
                    b'"' | b'\'' => quote = Some(c),
                    b'(' => depth_paren += 1,
                    b')' => depth_paren -= 1,
                    b'{' | b';' if depth_paren <= 0 => break,
                    _ => {}
                },
            }
            i += 1;
        }
        if i >= b.len() {
            sheet.css.push_str(&text[start..]);
            return;
        }
        let prelude = &text[start..i];
        if b[i] == b';' {
            sheet.css.push_str(&text[start..=i]);
            i += 1;
            continue;
        }
        let body_start = i + 1;
        let mut depth = 1;
        let mut j = body_start;
        let mut quote: Option<u8> = None;
        while j < b.len() && depth > 0 {
            let c = b[j];
            match quote {
                Some(q) if c == q => quote = None,
                Some(_) => {}
                None => match c {
                    b'"' | b'\'' => quote = Some(c),
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                },
            }
            j += 1;
        }
        let body = &text[body_start..j.saturating_sub(1).max(body_start)];
        if prelude.trim_start().starts_with('@') {
            let name = prelude.trim_start().split(|c: char| c.is_whitespace() || c == '(').next().unwrap_or("");
            if matches!(name, "@media" | "@supports" | "@layer") {
                sheet.css.push_str(prelude);
                sheet.css.push('{');
                rewrite_block(body, sheet);
                sheet.css.push('}');
            } else {
                sheet.css.push_str(&text[start..j]);
            }
        } else {
            let content = content_of(body);
            let mut sels = Vec::new();
            for sel in split_top(prelude.trim(), ',') {
                let sel = sel.trim();
                match split_pseudo(sel) {
                    Some((base, which)) => {
                        let tag = if which == Which::Before { BEFORE_TAG } else { AFTER_TAG };
                        let base_text = if base.is_empty() { "*" } else { base };
                        if let Some(parsed) = parse_selector(base_text) {
                            sheet.pseudo.push(PseudoRule { base: parsed, which, content: content.clone() });
                        }
                        sels.push(format!("{base_text} > {tag}"));
                    }
                    None => sels.push(sel.to_string()),
                }
            }
            sheet.css.push_str(&sels.join(","));
            sheet.css.push('{');
            sheet.css.push_str(body);
            sheet.css.push('}');
        }
        i = j;
    }
}

fn split_pseudo(sel: &str) -> Option<(&str, Which)> {
    for (suffix, which) in [("::after", Which::After), (":after", Which::After), ("::before", Which::Before), (":before", Which::Before)] {
        if let Some(base) = sel.strip_suffix(suffix) {
            return Some((base.trim_end(), which));
        }
    }
    None
}

fn content_of(body: &str) -> Option<String> {
    let mut found = None;
    for decl in body.split(';') {
        let Some((name, value)) = decl.split_once(':') else { continue };
        if name.trim().eq_ignore_ascii_case("content") {
            let v = value.trim().trim_end_matches("!important").trim();
            found = Some(match v {
                "none" | "normal" => None,
                _ => Some(css_string(v)),
            });
        }
    }
    found.flatten()
}

fn css_string(v: &str) -> String {
    let inner = v.trim();
    let inner = if (inner.starts_with('"') && inner.ends_with('"') || inner.starts_with('\'') && inner.ends_with('\'')) && inner.len() >= 2 {
        &inner[1..inner.len() - 1]
    } else {
        return String::new();
    };
    let mut out = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let mut hex = String::new();
        while hex.len() < 6 && chars.peek().is_some_and(|h| h.is_ascii_hexdigit()) {
            hex.push(chars.next().unwrap());
        }
        if hex.is_empty() {
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else {
            if chars.peek() == Some(&' ') {
                chars.next();
            }
            if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                out.push(ch);
            }
        }
    }
    out
}

/// Which pseudo-elements `el` gets: (before text, after text).
pub fn pseudo_for(sheet: &Sheet, doc: &Doc, el: usize) -> (Option<String>, Option<String>) {
    let mut before = None;
    let mut after = None;
    for r in &sheet.pseudo {
        if r.content.is_none() || !matches(doc, el, &r.base) {
            continue;
        }
        match r.which {
            Which::Before => before = r.content.clone(),
            Which::After => after = r.content.clone(),
        }
    }
    (before, after)
}

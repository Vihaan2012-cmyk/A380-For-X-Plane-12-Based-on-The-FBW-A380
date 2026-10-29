#[derive(Clone, Debug, Default)]
pub struct Element {
    pub tag: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attrs: Vec<(String, String)>,
    pub inline: Option<String>,
    pub parent: Option<usize>,
    pub kids: Vec<usize>,
    pub index: usize,
    pub has_text: bool,
    pub golden: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub els: Vec<Element>,
    pub styles: Vec<String>,
    pub links: Vec<String>,
}

impl Doc {
    pub fn new() -> Self {
        Doc::default()
    }

    pub fn add(&mut self, parent: Option<usize>, tag: &str) -> usize {
        let idx = self.els.len();
        let index = match parent {
            Some(p) => self.els[p].kids.len(),
            None => 0,
        };
        self.els.push(Element { tag: tag.to_string(), parent, index, ..Default::default() });
        if let Some(p) = parent {
            self.els[p].kids.push(idx);
        }
        idx
    }

    pub fn set_attr(&mut self, i: usize, name: &str, value: &str) {
        let e = &mut self.els[i];
        match name {
            "id" => e.id = Some(value.to_string()),
            "class" => e.classes = value.split_whitespace().map(str::to_string).collect(),
            "style" => e.inline = Some(value.to_string()),
            "data-cs" => e.golden = Some(value.to_string()),
            _ => {}
        }
        if let Some(slot) = e.attrs.iter_mut().find(|(n, _)| n == name) {
            slot.1 = value.to_string();
        } else {
            e.attrs.push((name.to_string(), value.to_string()));
        }
    }

    pub fn attr(&self, i: usize, name: &str) -> Option<&str> {
        self.els[i].attrs.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    pub fn has_attr(&self, i: usize, name: &str) -> bool {
        self.attr(i, name).is_some()
    }

    pub fn mark_text(&mut self, i: usize) {
        self.els[i].has_text = true;
    }

    pub fn prev_sibling(&self, i: usize) -> Option<usize> {
        let e = &self.els[i];
        let p = e.parent?;
        if e.index == 0 {
            None
        } else {
            Some(self.els[p].kids[e.index - 1])
        }
    }
}

const VOID: &[&str] = &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"];
const RAW: &[&str] = &["script", "style", "textarea", "title"];

pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        rest = &rest[p..];
        if let Some(semi) = rest[..rest.len().min(12)].find(';') {
            let ent = &rest[1..semi];
            let rep = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
                _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32),
                _ => None,
            };
            if let Some(c) = rep {
                out.push(c);
                rest = &rest[semi + 1..];
                continue;
            }
        }
        out.push('&');
        rest = &rest[1..];
    }
    out.push_str(rest);
    out
}

pub fn parse_html(src: &str) -> Doc {
    let mut doc = Doc::new();
    let b = src.as_bytes();
    let mut pos = 0;
    let mut stack: Vec<usize> = Vec::new();
    while pos < b.len() {
        if b[pos] != b'<' {
            let end = src[pos..].find('<').map(|x| pos + x).unwrap_or(b.len());
            if let Some(&top) = stack.last() {
                doc.mark_text(top);
            }
            pos = end;
            continue;
        }
        if src[pos..].starts_with("<!--") {
            pos = src[pos..].find("-->").map(|x| pos + x + 3).unwrap_or(b.len());
            continue;
        }
        if src[pos..].starts_with("<!") || src[pos..].starts_with("<?") {
            pos = src[pos..].find('>').map(|x| pos + x + 1).unwrap_or(b.len());
            continue;
        }
        if src[pos..].starts_with("</") {
            let end = src[pos..].find('>').map(|x| pos + x).unwrap_or(b.len());
            let name = src[pos + 2..end].trim().to_ascii_lowercase();
            if let Some(at) = stack.iter().rposition(|&i| doc.els[i].tag.to_ascii_lowercase() == name) {
                stack.truncate(at);
            }
            pos = (end + 1).min(b.len());
            continue;
        }
        let mut p = pos + 1;
        let name_start = p;
        while p < b.len() && !b[p].is_ascii_whitespace() && b[p] != b'>' && b[p] != b'/' {
            p += 1;
        }
        let tag = &src[name_start..p];
        if tag.is_empty() {
            pos += 1;
            continue;
        }
        let mut attrs: Vec<(String, String)> = Vec::new();
        let mut self_close = false;
        loop {
            while p < b.len() && (b[p].is_ascii_whitespace()) {
                p += 1;
            }
            if p >= b.len() {
                break;
            }
            if b[p] == b'>' {
                p += 1;
                break;
            }
            if b[p] == b'/' {
                self_close = true;
                p += 1;
                continue;
            }
            let ns = p;
            while p < b.len() && !b[p].is_ascii_whitespace() && b[p] != b'=' && b[p] != b'>' && b[p] != b'/' {
                p += 1;
            }
            let an = src[ns..p].to_string();
            while p < b.len() && b[p].is_ascii_whitespace() {
                p += 1;
            }
            let mut val = String::new();
            if p < b.len() && b[p] == b'=' {
                p += 1;
                while p < b.len() && b[p].is_ascii_whitespace() {
                    p += 1;
                }
                if p < b.len() && (b[p] == b'"' || b[p] == b'\'') {
                    let q = b[p];
                    let vs = p + 1;
                    let ve = src[vs..].bytes().position(|c| c == q).map(|x| vs + x).unwrap_or(b.len());
                    val = decode_entities(&src[vs..ve]);
                    p = (ve + 1).min(b.len());
                } else {
                    let vs = p;
                    while p < b.len() && !b[p].is_ascii_whitespace() && b[p] != b'>' {
                        p += 1;
                    }
                    val = decode_entities(&src[vs..p]);
                }
            }
            attrs.push((an, val));
        }
        pos = p;
        let lname = tag.to_ascii_lowercase();
        let parent = stack.last().copied();
        if let Some(par) = parent {
            doc.mark_text(par);
            doc.els[par].has_text = doc.els[par].has_text;
        }
        let idx = doc.add(parent, tag);
        doc.els[idx].has_text = false;
        for (n, v) in &attrs {
            doc.set_attr(idx, n, v);
        }
        if lname == "link" {
            if let Some(h) = doc.attr(idx, "href") {
                if doc.attr(idx, "rel").map(|r| r.eq_ignore_ascii_case("stylesheet")).unwrap_or(false) {
                    let h = h.to_string();
                    doc.links.push(h);
                }
            }
        }
        if RAW.contains(&lname.as_str()) && !self_close {
            let close = format!("</{}", lname);
            let lower = src[pos..].to_ascii_lowercase();
            let end = lower.find(&close).map(|x| pos + x).unwrap_or(b.len());
            if lname == "style" {
                doc.styles.push(src[pos..end].to_string());
            } else if lname == "textarea" {
                doc.mark_text(idx);
            }
            pos = src[end..].find('>').map(|x| end + x + 1).unwrap_or(b.len());
            continue;
        }
        if !(VOID.contains(&lname.as_str()) || self_close) {
            stack.push(idx);
        }
    }
    for e in doc.els.iter_mut() {
        e.has_text = e.has_text || !e.kids.is_empty();
    }
    doc
}

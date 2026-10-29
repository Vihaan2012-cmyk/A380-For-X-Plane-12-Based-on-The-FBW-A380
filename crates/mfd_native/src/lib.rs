use css_cascade::{Cascade, Doc};
use mfd_tree::{Kid, Node, Shape};
use std::fmt::Write as _;

pub const MFD_CSS: &str = "/Pages/VCockpit/Instruments/A380X/MFD/mfd.css";
pub const MFD_W: f64 = 1646.0;
pub const MFD_H: f64 = 1024.0;

const VOID: &[&str] = &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"];

pub struct Native {
    cascade: Cascade,
}

impl Native {
    pub fn from_css(sheets: &[&str], dpr: f64) -> Native {
        let mut c = Cascade::with_defaults(MFD_W, MFD_H);
        c.add_host_sheets();
        for s in sheets {
            c.add_sheet(s, 1);
        }
        c.dpr = dpr;
        Native { cascade: c }
    }

    pub fn load(html_ui: &str, dpr: f64) -> std::io::Result<Native> {
        let css = std::fs::read_to_string(format!("{}{}", html_ui.trim_end_matches('/'), MFD_CSS))?;
        Ok(Native::from_css(&[&css], dpr))
    }

    pub fn document(&self, body: &Node) -> String {
        let mut b = DocBuilder::default();
        let html = b.doc.add(None, "html");
        b.doc.set_attr(html, "class", "animationsEnabled");
        b.doc.mark_text(html);
        b.add_node(html, body);
        let styles = self.cascade.compute(&b.doc);
        let mut out = String::with_capacity(body_size_hint(body));
        let mut cur = 1usize;
        write_node(body, &styles, &mut cur, &mut out);
        out
    }
}

fn body_size_hint(n: &Node) -> usize {
    count(n) * 1400
}

fn count(n: &Node) -> usize {
    1 + n.shapes.len() + n.kids.iter().map(|k| if let Kid::Node(k) = k { count(k) } else { 0 }).sum::<usize>()
}

#[derive(Default)]
struct DocBuilder {
    doc: Doc,
}

impl DocBuilder {
    fn add_node(&mut self, parent: usize, n: &Node) -> usize {
        let i = self.doc.add(Some(parent), &n.tag);
        self.doc.mark_text(parent);
        if let Some(id) = &n.id {
            self.doc.set_attr(i, "id", id);
        }
        if !n.cls.is_empty() {
            self.doc.set_attr(i, "class", &n.cls.join(" "));
        }
        if let Some(s) = &n.inline {
            self.doc.set_attr(i, "style", s);
        }
        if let Some(v) = &n.value {
            self.doc.set_attr(i, "value", v);
        }
        for (k, v) in &n.attrs {
            self.doc.set_attr(i, k, v);
        }
        for (k, v) in &n.svgattr {
            if !self.doc.has_attr(i, k) {
                self.doc.set_attr(i, k, v);
            }
        }
        let mut stack: Vec<usize> = vec![i];
        for s in &n.shapes {
            let d = shape_depth(s);
            stack.truncate(d + 1);
            let p = *stack.last().unwrap();
            let si = self.doc.add(Some(p), &s.t);
            self.doc.mark_text(p);
            for (k, v) in &s.a {
                if !k.starts_with('_') {
                    self.doc.set_attr(si, k, v);
                }
            }
            stack.push(si);
        }
        for k in &n.kids {
            match k {
                Kid::Node(c) => {
                    self.add_node(i, c);
                }
                Kid::Text { text } => {
                    if !text.is_empty() {
                        self.doc.mark_text(i);
                    }
                }
            }
        }
        i
    }
}

fn shape_depth(s: &Shape) -> usize {
    s.a.get("_d").and_then(|d| d.parse().ok()).unwrap_or(0)
}

fn esc_text(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}

fn esc_attr(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            _ => out.push(c),
        }
    }
}

fn attr(out: &mut String, k: &str, v: &str) {
    let _ = write!(out, " {}=\"", k);
    esc_attr(v, out);
    out.push('"');
}

fn svg_size_from_attrs(cs: &str, n: &Node) -> String {
    cs.split(';')
        .map(|decl| {
            let Some((k, v)) = decl.split_once(':') else { return decl.to_string() };
            let k = k.trim();
            if (k == "width" || k == "height") && v.trim() == "auto" {
                if let Some(px) = n.svgattr.get(k).or_else(|| n.attrs.get(k)).and_then(|a| a.trim().trim_end_matches("px").parse::<f64>().ok()) {
                    return format!("{k}:{px}px");
                }
            }
            decl.to_string()
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// The properties mfdx's `fill_cs_defaults` (dom.cpp) puts back when they are missing.
const CS_DEFAULTS: &[(&str, &str)] = &[
    ("position", "static"), ("top", "auto"), ("left", "auto"), ("right", "auto"), ("bottom", "auto"),
    ("min-width", "0px"), ("min-height", "0px"), ("max-width", "none"), ("max-height", "none"),
    ("margin-top", "0px"), ("margin-right", "0px"), ("margin-bottom", "0px"), ("margin-left", "0px"),
    ("padding-top", "0px"), ("padding-right", "0px"), ("padding-bottom", "0px"), ("padding-left", "0px"),
    ("border-top-width", "0px"), ("border-right-width", "0px"), ("border-bottom-width", "0px"), ("border-left-width", "0px"),
    ("border-top-style", "none"), ("flex-direction", "row"), ("flex-wrap", "nowrap"), ("justify-content", "normal"),
    ("align-items", "normal"), ("align-self", "auto"), ("align-content", "normal"), ("flex-grow", "0"), ("flex-shrink", "1"),
    ("flex-basis", "auto"), ("gap", "normal"), ("row-gap", "normal"), ("column-gap", "normal"),
    ("grid-template-columns", "none"), ("grid-template-rows", "none"), ("grid-column", "auto"), ("grid-row", "auto"),
    ("order", "0"), ("vertical-align", "baseline"), ("text-decoration-line", "none"), ("text-transform", "none"),
    ("background-color", "rgba(0, 0, 0, 0)"), ("opacity", "1"), ("visibility", "visible"), ("overflow", "visible"),
    ("transform", "none"), ("z-index", "auto"),
];

fn without_defaults(cs: &str) -> String {
    let pairs: Vec<(&str, &str)> = cs.split(';').filter_map(|p| p.split_once(':')).collect();
    let color = pairs.iter().find(|(k, _)| *k == "color").map(|(_, v)| *v);
    let mut out = String::with_capacity(cs.len() / 2);
    for (k, v) in pairs {
        if CS_DEFAULTS.iter().any(|(dk, dv)| *dk == k && *dv == v) {
            continue;
        }
        if k.starts_with("border-") && k.ends_with("-color") && Some(v) == color {
            continue;
        }
        if !out.is_empty() {
            out.push(';');
        }
        out.push_str(k);
        out.push(':');
        out.push_str(v);
    }
    out
}

fn style_count(n: &Node) -> usize {
    1 + n.shapes.len() + n.kids.iter().map(|k| if let Kid::Node(c) = k { style_count(c) } else { 0 }).sum::<usize>()
}

fn write_hidden(n: &Node, out: &mut String) {
    out.push_str("<i data-cs=\"display:none\">");
    for k in &n.kids {
        if let Kid::Node(c) = k {
            write_hidden(c, out);
        }
    }
    out.push_str("</i>");
}

fn write_node(n: &Node, styles: &[css_cascade::Style], cur: &mut usize, out: &mut String) {
    let idx = *cur;
    if styles[idx].get("display") == Some("none") {
        *cur += style_count(n);
        write_hidden(n, out);
        return;
    }
    *cur += 1;
    let svg = n.tag == "svg";
    out.push('<');
    out.push_str(&n.tag);
    if let Some(id) = &n.id {
        attr(out, "id", id);
    }
    if !n.cls.is_empty() {
        attr(out, "class", &n.cls.join(" "));
    }
    if let Some(st) = &n.inline {
        attr(out, "style", st);
    }
    for (k, v) in &n.attrs {
        if k != "value" && k != "style" {
            attr(out, k, v);
        }
    }
    if svg {
        for (k, v) in &n.svgattr {
            if !n.attrs.contains_key(k) && !(k == "style" && n.inline.is_some()) {
                attr(out, k, v);
            }
        }
    }
    if let Some(v) = &n.value {
        attr(out, "value", v);
    } else if let Some(v) = n.attrs.get("value") {
        attr(out, "value", v);
    }
    let mut cs = styles[idx].to_data_cs();
    if svg {
        cs = svg_size_from_attrs(&cs, n);
    }
    attr(out, "data-cs", &without_defaults(&cs));
    out.push('>');
    if VOID.contains(&n.tag.as_str()) {
        return;
    }
    let mut open: Vec<(usize, &str)> = Vec::new();
    for s in &n.shapes {
        let d = shape_depth(s);
        while open.last().map(|(od, _)| *od >= d).unwrap_or(false) {
            let (_, t) = open.pop().unwrap();
            let _ = write!(out, "</{}>", t);
        }
        let si = *cur;
        *cur += 1;
        out.push('<');
        out.push_str(&s.t);
        for (k, v) in &s.a {
            if !k.starts_with('_') {
                attr(out, k, v);
            }
        }
        attr(out, "data-cs", &styles[si].to_data_cs());
        out.push('>');
        open.push((d, s.t.as_str()));
    }
    while let Some((_, t)) = open.pop() {
        let _ = write!(out, "</{}>", t);
    }
    for k in &n.kids {
        match k {
            Kid::Node(c) => write_node(c, styles, cur, out),
            Kid::Text { text } => esc_text(text, out),
        }
    }
    let _ = write!(out, "</{}>", n.tag);
}

pub fn mfd_screen(mut left: Node, mut right: Node) -> Node {
    left.inline.get_or_insert_with(|| "width: 768px; position: relative; margin-right: 110px;".into());
    right.inline.get_or_insert_with(|| "width: 768px; position: relative;".into());
    Node::new("body").attr("quality", "high").attr("gamestate", "ingame").child(
        Node::new("vcockpit-panel").style("position: absolute; inset: 0px;").child(
            Node::new("a380x-mfd")
                .attr("url", "/Pages/VCockpit/Instruments/A380X/MFD/mfd.html?duID=2")
                .style("position: absolute; left: 0px; top: 0px; width: 100%; height: 100%;")
                .child(Node::new("div").id("MFD_CONTENT").style("display: flex; flex-direction: row; height: 1024px;").child(left).child(right)),
        ),
    )
}

pub fn node_from_json(text: &str) -> Result<Node, serde_json::Error> {
    serde_json::from_str(text)
}

pub fn find_mut<'a>(n: &'a mut Node, pred: &dyn Fn(&Node) -> bool) -> Option<&'a mut Node> {
    if pred(n) {
        return Some(n);
    }
    for k in n.kids.iter_mut() {
        if let Kid::Node(c) = k {
            if let Some(f) = find_mut(c, pred) {
                return Some(f);
            }
        }
    }
    None
}

fn has_cls(n: &Node, c: &str) -> bool {
    n.cls.iter().any(|x| x == c)
}

pub fn splice_left(screen: &mut Node, part: Node) -> bool {
    let left = match find_mut(screen, &|n| n.id.as_deref() == Some("MFD_LEFT_PARENT_DIV")) {
        Some(l) => l,
        None => return false,
    };
    if part.id.as_deref() == Some("MFD_LEFT_PARENT_DIV") {
        let mut part = part;
        if part.inline.is_none() {
            part.inline = left.inline.take();
        }
        *left = part;
        return true;
    }
    let nav = match find_mut(left, &|n| has_cls(n, "mfd-navigator-container")) {
        Some(n) => n,
        None => return false,
    };
    if has_cls(&part, "mfd-navigator-container") {
        *nav = part;
        return true;
    }
    if has_cls(&part, "mfd-page-container") {
        for k in nav.kids.iter_mut() {
            if let Kid::Node(c) = k {
                if has_cls(c, "mfd-page-container") {
                    *c = part;
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_body_with_data_cs() {
        let css = ".a { color: #0f0; display: flex; } .b input[type=radio]:checked { margin-left: 5px; }";
        let n = Native::from_css(&[css], 1.0);
        let left = Node::new("div").id("MFD_LEFT_PARENT_DIV").child(Node::el("div", "a b").text("X<").child(Node::new("input").attr("type", "radio").attr_if("checked", true)));
        let doc = n.document(&mfd_screen(left, Node::new("div").id("MFD_RIGHT_PARENT_DIV")));
        assert!(doc.starts_with("<body gamestate=\"ingame\" quality=\"high\""));
        assert!(doc.ends_with("</body>"));
        assert!(doc.contains("class=\"a b\" data-cs=\"display:flex;"));
        assert!(doc.contains("color:rgb(0, 255, 0)"));
        assert!(doc.contains("X&lt;<input checked=\"\" type=\"radio\" data-cs=\""));
        assert!(doc.contains("margin-left:5px"));
        assert!(!doc.contains("</input>"));
    }

    #[test]
    fn hover_matches_the_marked_element() {
        let css = ".m { border: 3px solid transparent; } .m:hover { border: 3px solid #0ff; }";
        let n = Native::from_css(&[css], 1.0);
        let left = Node::new("div").child(Node::el("span", "m").attr("data-hover", "").text("A")).child(Node::el("span", "m").text("B"));
        let doc = n.document(&mfd_screen(left, Node::new("div")));
        let a = doc.find(">A<").unwrap();
        let b = doc.find(">B<").unwrap();
        assert!(doc[..a].rfind("rgb(0, 255, 255)").is_some_and(|i| i > doc[..a].rfind("<span").unwrap()));
        assert!(!doc[a..b].contains("rgb(0, 255, 255)"));
    }

    #[test]
    fn svg_shapes_nest_by_depth() {
        let mut s = Node::new("svg");
        s.svgattr.insert("viewBox".into(), "0 0 10 10".into());
        let sh = |t: &str, d: &str| {
            let mut a = std::collections::BTreeMap::new();
            a.insert("_d".to_string(), d.to_string());
            Shape { t: t.into(), a }
        };
        s.shapes = vec![sh("g", "0"), sh("line", "1"), sh("path", "1"), sh("rect", "0")];
        let doc = Native::from_css(&[], 1.0).document(&Node::new("body").child(s));
        let tags: Vec<&str> = doc.split('<').skip(1).filter_map(|p| p.split(|c: char| c == ' ' || c == '>').next()).collect();
        assert_eq!(tags, vec!["body", "svg", "g", "line", "/line", "path", "/path", "/g", "rect", "/rect", "/svg", "/body"]);
    }
}

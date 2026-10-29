use std::fmt::Write as _;

use css_cascade::{Cascade, Doc, Style};
use mfd_tree::{Kid, Node, Shape};

pub const VIEW_W: f64 = 1333.0;
pub const VIEW_H: f64 = 1000.0;
pub const OIT_CSS: &str = include_str!("oit.css");

const VOID: &[&str] = &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"];
const LEAF: &[&str] = &["svg", "canvas", "img", "input", "textarea", "select", "iframe"];

/// One element of the published document as input sees it, indexed like the
/// boxes the display reports: document order, `body` first, svg shapes and
/// anything inside a replaced element left out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeInfo {
    pub parent: Option<usize>,
    pub tag: String,
    pub act: Option<String>,
    pub clips: bool,
    pub hidden: bool,
    pub layer: i32,
}

pub struct Built {
    pub html: String,
    pub nodes: Vec<NodeInfo>,
}

pub struct Styler {
    cascade: Cascade,
}

impl Default for Styler {
    fn default() -> Self {
        Styler::new(&[OIT_CSS])
    }
}

/// The page around `#OIT_CONTENT` as the CEF view has it.
pub fn wrap(content: Node, index: u8) -> Node {
    Node::new("body")
        .attr("quality", "high")
        .attr("gamestate", "ingame")
        .child(
            Node::new("vcockpit-panel").style("position: absolute; inset: 0px;").child(
                Node::new("a380x-oit")
                    .attr("url", &format!("/Pages/VCockpit/Instruments/A380X/OIT/oit.html?Index={index}"))
                    .attr("created", "true")
                    .style("position: absolute; left: 0px; top: 0px; width: 100%; height: 100%;")
                    .child(content),
            ),
        )
}

impl Styler {
    pub fn new(sheets: &[&str]) -> Styler {
        let mut c = Cascade::with_defaults(VIEW_W, VIEW_H);
        c.add_host_sheets();
        for s in sheets {
            c.add_sheet(s, 1);
        }
        Styler { cascade: c }
    }

    pub fn build(&self, body: &Node) -> Built {
        let (doc, kinds) = doc_of(body);
        let styles = self.cascade.compute(&doc);
        let mut nodes: Vec<NodeInfo> = Vec::new();
        let mut box_of: Vec<Option<usize>> = vec![None; doc.els.len()];
        for (i, kind) in kinds.iter().enumerate() {
            if *kind != Kind::Box {
                continue;
            }
            let e = &doc.els[i];
            let st = &styles[i];
            let get = |n: &str| st.get(n).unwrap_or("");
            let parent = e.parent.and_then(|p| box_of[p]);
            let parent_layer = parent.map(|p: usize| nodes[p].layer).unwrap_or(0);
            let position = get("position");
            let z = get("z-index").trim().parse::<i32>().ok();
            let layer = if position != "static" { z.unwrap_or(parent_layer) } else { parent_layer };
            let hidden_parent = parent.map(|p| nodes[p].hidden).unwrap_or(false);
            box_of[i] = Some(nodes.len());
            nodes.push(NodeInfo {
                parent,
                tag: e.tag.clone(),
                act: doc.attr(i, "data-act").map(str::to_string),
                clips: get("overflow") != "visible",
                hidden: hidden_parent || get("display") == "none" || get("visibility") == "hidden",
                layer,
            });
        }
        let mut out = String::with_capacity(count(body) * 1200);
        let mut cur = 1usize;
        write_node(body, &styles, &mut cur, &mut out);
        Built { html: out, nodes }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Root,
    Box,
    Inner,
}

fn doc_of(body: &Node) -> (Doc, Vec<Kind>) {
    let mut doc = Doc::new();
    let mut kinds = vec![Kind::Root];
    let html = doc.add(None, "html");
    doc.set_attr(html, "class", "animationsEnabled");
    doc.mark_text(html);
    add_node(&mut doc, &mut kinds, html, body, false);
    (doc, kinds)
}

fn add_node(doc: &mut Doc, kinds: &mut Vec<Kind>, parent: usize, n: &Node, inner: bool) -> usize {
    let i = doc.add(Some(parent), &n.tag);
    kinds.push(if inner { Kind::Inner } else { Kind::Box });
    doc.mark_text(parent);
    if let Some(id) = &n.id {
        doc.set_attr(i, "id", id);
    }
    if !n.cls.is_empty() {
        doc.set_attr(i, "class", &n.cls.join(" "));
    }
    if let Some(s) = &n.inline {
        doc.set_attr(i, "style", s);
    }
    if let Some(v) = &n.value {
        doc.set_attr(i, "value", v);
    }
    for (k, v) in &n.attrs {
        doc.set_attr(i, k, v);
    }
    for (k, v) in &n.data {
        doc.set_attr(i, &format!("data-{k}"), v);
    }
    for (k, v) in &n.svgattr {
        if !doc.has_attr(i, k) {
            doc.set_attr(i, k, v);
        }
    }
    let mut stack: Vec<usize> = vec![i];
    for s in &n.shapes {
        let d = shape_depth(s);
        stack.truncate(d + 1);
        let p = *stack.last().unwrap_or(&i);
        let si = doc.add(Some(p), &s.t);
        kinds.push(Kind::Inner);
        doc.mark_text(p);
        for (k, v) in &s.a {
            if !k.starts_with('_') {
                doc.set_attr(si, k, v);
            }
        }
        stack.push(si);
    }
    let leaf = inner || LEAF.contains(&n.tag.as_str());
    for k in &n.kids {
        match k {
            Kid::Node(c) => {
                add_node(doc, kinds, i, c, leaf);
            }
            Kid::Text { text } => {
                if !text.is_empty() {
                    doc.mark_text(i);
                }
            }
        }
    }
    i
}

fn shape_depth(s: &Shape) -> usize {
    s.a.get("_d").and_then(|d| d.parse().ok()).unwrap_or(0)
}

fn count(n: &Node) -> usize {
    1 + n.shapes.len() + n.kids.iter().map(|k| if let Kid::Node(k) = k { count(k) } else { 0 }).sum::<usize>()
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

fn attr(out: &mut String, k: &str, v: &str) {
    let _ = write!(out, " {k}=\"");
    for c in v.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            _ => out.push(c),
        }
    }
    out.push('"');
}

fn write_node(n: &Node, styles: &[Style], cur: &mut usize, out: &mut String) {
    let idx = *cur;
    *cur += 1;
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
    for (k, v) in &n.data {
        attr(out, &format!("data-{k}"), v);
    }
    if n.tag == "svg" {
        for (k, v) in &n.svgattr {
            if !n.attrs.contains_key(k) && !(k == "style" && n.inline.is_some()) {
                attr(out, k, v);
            }
        }
    }
    if let Some(v) = n.value.as_ref().or_else(|| n.attrs.get("value")) {
        attr(out, "value", v);
    }
    if let Some(s) = styles.get(idx) {
        attr(out, "data-cs", &s.to_data_cs());
    }
    out.push('>');
    if VOID.contains(&n.tag.as_str()) {
        return;
    }
    let mut open: Vec<(usize, &str)> = Vec::new();
    for s in &n.shapes {
        let d = shape_depth(s);
        while open.last().is_some_and(|(od, _)| *od >= d) {
            if let Some((_, t)) = open.pop() {
                let _ = write!(out, "</{t}>");
            }
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
        if let Some(st) = styles.get(si) {
            attr(out, "data-cs", &st.to_data_cs());
        }
        out.push('>');
        open.push((d, s.t.as_str()));
    }
    while let Some((_, t)) = open.pop() {
        let _ = write!(out, "</{t}>");
    }
    for k in &n.kids {
        match k {
            Kid::Node(c) => write_node(c, styles, cur, out),
            Kid::Text { text } => esc_text(text, out),
        }
    }
    let _ = write!(out, "</{}>", n.tag);
}

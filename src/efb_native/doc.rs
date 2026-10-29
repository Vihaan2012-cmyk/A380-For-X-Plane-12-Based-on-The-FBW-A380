use std::fmt::Write as _;

use css_cascade::{Cascade, Doc, Style};
use mfd_tree::{Kid, Node, Shape};

use super::style::{pseudo_for, Sheet, AFTER_TAG, BEFORE_TAG, EFB_CSS, NATIVE_CSS};

pub const VIEW_W: f64 = 1430.0;
pub const VIEW_H: f64 = 1000.0;

const VOID: &[&str] = &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"];
const LEAF: &[&str] = &["svg", "canvas", "img", "input", "textarea", "select"];

/// What input needs to know about one element of the published document,
/// indexed like the boxes the display reports: elements in document order,
/// `body` first, svg shapes and anything inside a replaced element left out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeInfo {
    pub parent: Option<usize>,
    pub tag: String,
    pub id: Option<String>,
    pub act: Option<String>,
    pub scroller: Option<String>,
    pub clips: bool,
    pub hidden: bool,
    pub fixed: bool,
    pub layer: i32,
    pub padding_bottom: f64,
}

pub struct Built {
    pub html: String,
    pub nodes: Vec<NodeInfo>,
}

pub struct Styler {
    cascade: Cascade,
    sheet: Sheet,
}

impl Styler {
    pub fn new(extra_css: &[&str]) -> Styler {
        let mut css = String::from(EFB_CSS);
        css.push('\n');
        css.push_str(NATIVE_CSS);
        for e in extra_css {
            css.push('\n');
            css.push_str(e);
        }
        let sheet = super::style::prepare(&css);
        let mut c = Cascade::with_defaults(VIEW_W, VIEW_H);
        c.add_sheet(&sheet.css, 1);
        Styler { cascade: c, sheet }
    }

    /// `body` is the whole page; `scroll_prefix` names the generated ids of
    /// scroll containers that have none (stable while the page keeps its shape).
    pub fn build(&self, body: &Node, scroll_prefix: &str) -> Built {
        let mut body = body.clone();
        native_selects(&mut body);
        let (doc, _) = doc_of(&body);
        let mut want: Vec<(Option<String>, Option<String>)> = Vec::new();
        let mut any = false;
        for (i, e) in doc.els.iter().enumerate() {
            if i == 0 || is_shape_or_inside_leaf(&doc, i) || VOID.contains(&e.tag.as_str()) || LEAF.contains(&e.tag.as_str()) {
                want.push((None, None));
                continue;
            }
            let p = pseudo_for(&self.sheet, &doc, i);
            any |= p.0.is_some() || p.1.is_some();
            want.push(p);
        }
        if any {
            let mut cur = 1usize;
            add_pseudo(&mut body, &want, &mut cur);
        }
        let (doc, kinds) = doc_of(&body);
        let styles = self.cascade.compute(&doc);
        let mut nodes: Vec<NodeInfo> = Vec::new();
        let mut box_of: Vec<Option<usize>> = vec![None; doc.els.len()];
        let mut scrollers = 0usize;
        for (i, kind) in kinds.iter().enumerate() {
            if *kind != Kind::Box {
                continue;
            }
            let e = &doc.els[i];
            let st = &styles[i];
            let parent = e.parent.and_then(|p| box_of[p]);
            let get = |n: &str| st.get(n).unwrap_or("");
            let overflow = get("overflow");
            let oy = overflow.split_whitespace().last().unwrap_or("visible");
            let position = get("position");
            let parent_layer = parent.map(|p: usize| nodes[p].layer).unwrap_or(0);
            let z = get("z-index").trim().parse::<i32>().ok();
            let layer = if position != "static" { z.unwrap_or(parent_layer) } else { parent_layer };
            let hidden_parent = parent.map(|p| nodes[p].hidden).unwrap_or(false);
            let mut info = NodeInfo {
                parent,
                tag: e.tag.clone(),
                id: e.id.clone(),
                act: doc.attr(i, "data-act").map(str::to_string),
                scroller: None,
                clips: overflow != "visible",
                hidden: hidden_parent || get("display") == "none" || get("visibility") == "hidden",
                fixed: position == "fixed",
                layer,
                padding_bottom: px(get("padding-bottom")),
            };
            if matches!(oy, "auto" | "scroll") {
                info.scroller = Some(e.id.clone().unwrap_or_else(|| {
                    scrollers += 1;
                    format!("efbn-s-{scroll_prefix}-{scrollers}")
                }));
            }
            box_of[i] = Some(nodes.len());
            nodes.push(info);
        }
        let mut out = String::with_capacity(count(&body) * 900);
        let mut cur = 1usize;
        let mut bi = 0usize;
        write_node(&body, &styles, &nodes, &mut cur, &mut bi, &mut out);
        Built { html: out, nodes }
    }
}

fn px(v: &str) -> f64 {
    v.trim().trim_end_matches("px").parse().unwrap_or(0.0)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Root,
    Box,
    Inner,
}

const SEL: &str = "efbn-sel";

fn is_leaf(n: &Node) -> bool {
    LEAF.contains(&n.tag.as_str()) && !n.cls.iter().any(|c| c == SEL)
}

fn native_selects(n: &mut Node) {
    if n.tag == "select" && !n.cls.iter().any(|c| c == SEL) {
        let label = n.data.get("label").cloned().unwrap_or_default();
        let options = n.data.get("options").cloned().unwrap_or_default();
        let mut v = Node::el("span", "efbn-sel-v").child(Node::el("span", "efbn-sel-t").text(&label));
        for o in options.split('|').filter(|o| !o.is_empty()) {
            v = v.child(Node::el("span", "efbn-sel-o").text(o.split_once('=').map_or(o, |p| p.1)));
        }
        let mut arrow = Node::el("svg", "efbn-sel-a");
        for (k, val) in [("viewBox", "0 0 8 4"), ("fill", "none"), ("stroke", "currentColor"), ("stroke-width", "1")] {
            arrow.svgattr.insert(k.into(), val.into());
        }
        let mut a = std::collections::BTreeMap::new();
        a.insert("d".to_string(), "M0 0L4 4L8 0".to_string());
        a.insert("_d".to_string(), "0".to_string());
        arrow.shapes.push(Shape { t: "path".into(), a });
        n.cls.push(SEL.into());
        n.kids = vec![Kid::Node(v), Kid::Node(arrow)];
        return;
    }
    for k in n.kids.iter_mut() {
        if let Kid::Node(c) = k {
            native_selects(c);
        }
    }
}

fn is_shape_or_inside_leaf(doc: &Doc, i: usize) -> bool {
    let mut p = doc.els[i].parent;
    while let Some(pi) = p {
        if LEAF.contains(&doc.els[pi].tag.as_str()) && !doc.attr(pi, "class").is_some_and(|c| c.split_whitespace().any(|c| c == SEL)) {
            return true;
        }
        p = doc.els[pi].parent;
    }
    false
}

fn doc_of(body: &Node) -> (Doc, Vec<Kind>) {
    let mut doc = Doc::new();
    let mut kinds = vec![Kind::Root];
    let html = doc.add(None, "html");
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
        let p = *stack.last().unwrap();
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
    let leaf = inner || is_leaf(n);
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

fn add_pseudo(n: &mut Node, want: &[(Option<String>, Option<String>)], cur: &mut usize) {
    let me = *cur;
    *cur += 1 + n.shapes.len();
    for k in n.kids.iter_mut() {
        if let Kid::Node(c) = k {
            add_pseudo(c, want, cur);
        }
    }
    if let Some((before, after)) = want.get(me) {
        if let Some(t) = after {
            n.kids.push(Kid::Node(Node::new(AFTER_TAG).text(t)));
        }
        if let Some(t) = before {
            n.kids.insert(0, Kid::Node(Node::new(BEFORE_TAG).text(t)));
        }
    }
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

fn write_node(n: &Node, styles: &[Style], nodes: &[NodeInfo], cur: &mut usize, bi: &mut usize, out: &mut String) {
    let idx = *cur;
    *cur += 1;
    let info = nodes.get(*bi);
    *bi += 1;
    let tag = if n.tag == "select" && !is_leaf(n) { "div" } else { n.tag.as_str() };
    out.push('<');
    out.push_str(tag);
    match (&n.id, info.and_then(|i| i.scroller.as_ref())) {
        (Some(id), _) => attr(out, "id", id),
        (None, Some(s)) => attr(out, "id", s),
        _ => {}
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
            if !n.attrs.contains_key(k) {
                attr(out, k, v);
            }
        }
    }
    if let Some(v) = n.value.as_ref().or_else(|| n.attrs.get("value")) {
        attr(out, "value", v);
    }
    attr(out, "data-cs", &styles[idx].to_data_cs());
    out.push('>');
    if VOID.contains(&n.tag.as_str()) {
        return;
    }
    let mut open: Vec<(usize, &str)> = Vec::new();
    for s in &n.shapes {
        let d = shape_depth(s);
        while open.last().is_some_and(|(od, _)| *od >= d) {
            let (_, t) = open.pop().unwrap();
            let _ = write!(out, "</{t}>");
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
        let _ = write!(out, "</{t}>");
    }
    let leaf = is_leaf(n);
    for k in &n.kids {
        match k {
            Kid::Node(c) if leaf => skip_inner(c, cur),
            Kid::Node(c) => write_node(c, styles, nodes, cur, bi, out),
            Kid::Text { text } => esc_text(text, out),
        }
    }
    let _ = write!(out, "</{tag}>");
}

fn skip_inner(n: &Node, cur: &mut usize) {
    *cur += 1 + n.shapes.len();
    for k in &n.kids {
        if let Kid::Node(c) = k {
            skip_inner(c, cur);
        }
    }
}

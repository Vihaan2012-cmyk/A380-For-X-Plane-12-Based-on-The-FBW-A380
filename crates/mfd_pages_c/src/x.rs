use crate::w::*;
use mfd_pages_a::widgets as aw;
use mfd_pages_a::widgets::{ButtonSpec, MenuItem};
use mfd_tree::{Kid, Node};

pub fn br() -> Node {
    Node::new("br")
}

pub fn lines_label(lines: &[&str]) -> Node {
    let mut s = Node::new("span");
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            s = s.child(br());
        }
        s = s.text(l);
    }
    s
}

pub fn star_label(lines: &[&str], span_style: &str) -> Node {
    let mut s = Node::new("span").style(span_style);
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            s = s.child(br());
        }
        s = s.text(l);
    }
    Node::new("div")
        .style("display: flex; flex-direction: row; justify-content: space-between;")
        .child(s)
        .child(Node::new("span").style("display: flex; align-items: center; justify-content: center;").text("*"))
}

pub fn btn(text: &str, disabled: bool, style: &str) -> Node {
    aw::button(ButtonSpec::new(text).disabled(disabled).style(style))
}

pub fn btn_lines(lines: &[&str], disabled: bool, style: &str) -> Node {
    let mut n = btn("", disabled, style);
    if let Some(Kid::Node(face)) = n.kids.get_mut(0) {
        if let Some(Kid::Node(span)) = face.kids.get_mut(0) {
            for (i, l) in lines.iter().enumerate() {
                if i > 0 {
                    span.kids.push(Kid::Node(br()));
                }
                span.kids.push(Kid::Text { text: l.to_string() });
            }
        }
    }
    n
}

pub fn set_opened(mut n: Node, opened: bool) -> Node {
    if opened {
        if let Some(Kid::Node(face)) = n.kids.get_mut(0) {
            face.cls.push("opened".into());
        }
        if let Some(Kid::Node(menu)) = n.kids.get_mut(1) {
            menu.inline = menu.inline.as_ref().map(|s| s.replacen("display: none", "display: block", 1));
        }
    }
    n
}

pub fn btn_sel(text: &str, selected: bool, style: &str) -> Node {
    let mut s = ButtonSpec::new(text).style(style);
    s.selected = selected;
    aw::button(s)
}

pub fn btn_node(label: Node, disabled: bool, selected: bool, style: &str) -> Node {
    let mut s = ButtonSpec::node(label).disabled(disabled).style(style);
    s.selected = selected;
    aw::button(s)
}

pub fn btn_menu(text: &str, disabled: bool, style: &str, prefix: &str, items: &[&str], arrow: bool) -> Node {
    let mut spec = ButtonSpec::new(text).disabled(disabled).style(style);
    let mi: Vec<MenuItem> = items.iter().map(|l| MenuItem::new(l, false)).collect();
    if arrow {
        spec = spec.menu(prefix, mi, false);
        return aw::button(spec);
    }
    let mut n = aw::button(spec);
    if let Some(Kid::Node(menu)) = n.kids.get_mut(1) {
        let mut inner = Node::new("div");
        for (i, l) in items.iter().enumerate() {
            inner = inner.child(Node::el("span", "mfd-dropdown-menu-element").id(&format!("{}_{}", prefix, i)).text(l));
        }
        menu.kids.push(Kid::Node(inner));
    }
    n
}

pub fn icon(name: &str, disabled: bool, style: &str) -> Node {
    icon_button(name, disabled, style)
}

pub fn lv(kids: Vec<Node>) -> Node {
    Node::el("div", "mfd-label-value-container").children(kids)
}

pub fn footer(message: &str) -> Node {
    footer_bg(message, "")
}

pub fn footer_bg(message: &str, background: &str) -> Node {
    aw::footer(&aw::FooterModel { message: if message.is_empty() { None } else { Some(message.to_string()) }, message_background: background.to_string() })
}

pub fn unit(text: &str, big: bool) -> Node {
    Node::el("span", &format!("mfd-label-unit {}mfd-unit-trailing", if big { "bigger " } else { "" })).text(text)
}

use mfd_tree::Node;
use serde::Deserialize;

pub fn trim_style(s: &str) -> String {
    s.trim().to_string()
}

pub fn label_nodes(mut host: Node, label: &str) -> Node {
    let mut first = true;
    for part in label.split("<br />").flat_map(|p| p.split("<br/>")) {
        if !first {
            host = host.child(Node::new("br"));
        }
        host = host.text(part);
        first = false;
    }
    host
}

pub fn visibility_style(container_style: &str, visible: bool) -> String {
    let base = container_style.trim().trim_end_matches(';').trim();
    let vis = if visible { "inherit" } else { "hidden" };
    if base.is_empty() {
        format!("visibility: {vis};")
    } else {
        format!("{base}; visibility: {vis};")
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MenuItem {
    pub label: String,
    pub disabled: bool,
}

impl MenuItem {
    pub fn new(label: &str) -> Self {
        MenuItem { label: label.to_string(), disabled: false }
    }

    pub fn disabled(label: &str) -> Self {
        MenuItem { label: label.to_string(), disabled: true }
    }
}

#[derive(Clone, Debug)]
pub struct ButtonOpts {
    pub label: String,
    pub button_style: String,
    pub container_style: String,
    pub disabled: bool,
    pub visible: bool,
    pub selected: bool,
    pub highlighted: bool,
    pub id_prefix: String,
    pub menu: Option<Vec<MenuItem>>,
    pub menu_open: bool,
    pub menu_right_aligned: bool,
    pub menu_opens_upwards: bool,
    pub undefined_style: bool,
}

impl ButtonOpts {
    pub fn new(label: &str) -> Self {
        ButtonOpts {
            label: label.to_string(),
            button_style: String::new(),
            container_style: String::new(),
            disabled: false,
            visible: true,
            selected: false,
            highlighted: false,
            id_prefix: String::new(),
            menu: None,
            menu_open: false,
            menu_right_aligned: false,
            menu_opens_upwards: false,
            undefined_style: false,
        }
    }

    pub fn style(mut self, s: &str) -> Self {
        self.button_style = s.to_string();
        self
    }

    pub fn container(mut self, s: &str) -> Self {
        self.container_style = s.to_string();
        self
    }

    pub fn disabled(mut self, on: bool) -> Self {
        self.disabled = on;
        self
    }

    pub fn visible(mut self, on: bool) -> Self {
        self.visible = on;
        self
    }

    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    pub fn highlighted(mut self, on: bool) -> Self {
        self.highlighted = on;
        self
    }
}

fn triangle(points: &str, fill: &str, hidden: bool) -> Node {
    let mut n = Node::new("svg");
    n.svgattr.insert("height".into(), "15".into());
    n.svgattr.insert("width".into(), "15".into());
    if hidden {
        n.cls.push("hidden".into());
    }
    let mut a = std::collections::BTreeMap::new();
    a.insert("points".to_string(), points.to_string());
    a.insert("style".to_string(), format!("fill: {fill};"));
    a.insert("_d".to_string(), "0".to_string());
    a.insert("_display".to_string(), "inline".to_string());
    n.shapes.push(mfd_tree::Shape { t: "polygon".into(), a });
    n
}

pub fn button(o: &ButtonOpts) -> Node {
    let has_menu = o.menu.as_ref().map(|m| !m.is_empty()).unwrap_or(false);
    let bs = if o.undefined_style { "undefined".to_string() } else { o.button_style.clone() };
    let btn_style = if has_menu { trim_style(&format!("{bs} padding-right: 5px;")) } else { format!("{bs} ") };
    let mut btn = Node::el("span", "mfd-button")
        .class_if("disabled", o.disabled)
        .class_if("selected", o.selected)
        .class_if("highlighted", o.highlighted)
        .class_if("opened", o.menu_open && has_menu)
        .style(&btn_style);
    match &o.menu {
        Some(_) => {
            let arrow_fill = if o.disabled { "grey" } else { "white" };
            let lab = label_nodes(Node::el("span", "mfd-fms-fpln-button-dropdown-label"), &o.label);
            let arrow = Node::el("span", "mfd-fms-fpln-button-dropdown-arrow")
                .child(triangle("7.5,0 15,15 0,15", arrow_fill, !o.menu_opens_upwards))
                .child(triangle("0,0 15,0 7.5,15", arrow_fill, o.menu_opens_upwards));
            btn = btn.child(Node::el("div", "mfd-fms-fpln-button-dropdown").child(lab).child(arrow));
        }
        None => {
            btn = btn.child(label_nodes(Node::new("span"), &o.label));
        }
    }
    let mut menu = Node::el("div", "mfd-dropdown-menu");
    let display = if o.menu_open && has_menu { "block" } else { "none" };
    match &o.menu {
        Some(items) if !items.is_empty() => {
            let mut wrap = Node::new("div");
            for (i, it) in items.iter().enumerate() {
                wrap = wrap.child(
                    Node::el("span", "mfd-dropdown-menu-element")
                        .id(&format!("{}_{}", o.id_prefix, i))
                        .class_if("disabled", it.disabled)
                        .text(&it.label),
                );
            }
            let right = if o.menu_right_aligned { " right: 0px;" } else { "" };
            menu = menu.style(&format!("display: {display};{right}")).child(wrap);
        }
        Some(_) => {
            menu = menu.style(&format!("display: {display};"));
        }
        None => {
            menu = menu.style(&format!("display: {display}"));
        }
    }
    Node::el("div", "mfd-dropdown-container")
        .style(&visibility_style(&o.container_style, o.visible))
        .child(btn)
        .child(menu)
}

pub fn input_field_text(
    text: &str,
    container_style: &str,
    align: &str,
    digits: usize,
    leading: &str,
    trailing: &str,
    text_classes: &str,
) -> Node {
    let unit = |pos: &str, t: &str| {
        Node::el("span", &format!("mfd-label-unit mfd-unit-{pos} mfd-input-field-unit"))
            .style("visibility: inherit;")
            .text(if t.is_empty() { " " } else { t })
    };
    let ti = Node::el("span", &format!("mfd-input-field-text-input {text_classes}")).text(text);
    let caret = Node::el("span", "mfd-input-field-caret").style("display: none;");
    let tc = Node::el("div", "mfd-input-field-text-input-container")
        .style(&format!("justify-content: {align}; min-width: {}px;", digits * 18))
        .child(ti)
        .child(caret);
    let mut container = Node::el("div", "mfd-input-field-container")
        .child(unit("leading", leading))
        .child(tc)
        .child(unit("trailing", trailing));
    if !container_style.is_empty() {
        container = container.style(container_style);
    }
    Node::el("div", "mfd-input-field-root").child(container)
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DropdownModel {
    pub values: Vec<String>,
    pub selected: Option<usize>,
    pub open: bool,
    pub disabled: bool,
}

pub struct DropdownOpts<'a> {
    pub id_prefix: &'a str,
    pub container_style: &'a str,
    pub align: &'a str,
    pub digits: usize,
}

pub fn dropdown(o: &DropdownOpts, m: &DropdownModel) -> Node {
    let text = m.selected.and_then(|i| m.values.get(i)).cloned().unwrap_or_else(|| "-".repeat(o.digits.max(1)));
    let classes = if m.open { "editing valueSelected" } else { "" };
    let input = input_field_text(&text, "border: 2px inset transparent", o.align, o.digits, "", "", classes);
    let inner = Node::el("div", "mfd-dropdown-inner")
        .class_if("disabled", m.disabled)
        .style(&format!("justify-content: {};", o.align))
        .child(input);
    let arrow = Node::el("div", "mfd-dropdown-arrow")
        .class_if("disabled", m.disabled)
        .child(triangle("0,0 15,0 7.5,15", if m.disabled { "gray" } else { "white" }, false));
    let outer = Node::el("div", "mfd-dropdown-outer").class_if("disabled", m.disabled).child(inner).child(arrow);
    let mut menu = Node::el("div", "mfd-dropdown-menu")
        .style(&format!("display: {};", if m.open { "block" } else { "none" }));
    for (i, v) in m.values.iter().enumerate() {
        menu = menu.child(
            Node::el("span", "mfd-dropdown-menu-element")
                .id(&format!("{}_{}", o.id_prefix, i))
                .style(&format!("text-align: {};", o.align))
                .text(v),
        );
    }
    let mut c = Node::el("div", "mfd-dropdown-container");
    if !o.container_style.is_empty() {
        c = c.style(o.container_style);
    }
    c.child(outer).child(menu)
}

pub fn toggle_button(top_cls: &str, state: bool, disabled: bool, label_true: &str, label_false: &str, style: &str) -> Node {
    let upper_green = state && !disabled;
    let upper_hidden = !state && disabled;
    let lower_green = !state && !disabled;
    let lower_hidden = state && disabled;
    let vis = |hidden: bool| if hidden { "visibility: hidden;" } else { "visibility: visible;" };
    let (up_cls, low_cls) = if top_cls == "mfd-adsc-button" {
        (
            if upper_green { "mfd-value bigger" } else { "mfd-adsc-label-off" },
            if lower_green { "mfd-label bigger" } else { "mfd-adsc-label-off" },
        )
    } else {
        (
            if upper_green { "mfd-value bigger" } else { "mfd-label" },
            if lower_green { "mfd-value bigger" } else { "mfd-label" },
        )
    };
    let mut top = Node::el("div", top_cls).class_if("disabled", disabled);
    if !style.is_empty() {
        top = top.style(style);
    }
    top.child(Node::el("div", up_cls).style(vis(upper_hidden)).text(label_true))
        .child(Node::el("div", low_cls).style(vis(lower_hidden)).text(label_false))
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RadioModel {
    pub selected: Option<usize>,
    pub color: String,
    pub disabled: bool,
}

pub fn radio_group(id_prefix: &str, values: &[&str], spacing: u32, color: &str, disabled: bool) -> Node {
    let mut form = Node::new("form");
    for (i, v) in values.iter().enumerate() {
        let mut label = Node::el("label", &format!("mfd-radio-button {color}"))
            .id(&format!("{id_prefix}_label_{i}"))
            .class_if("disabled", disabled);
        if spacing > 0 {
            label = label.style(&format!("margin-top: {spacing}px;"));
        }
        label = label
            .child(Node::new("input").id(&format!("{id_prefix}_{i}")))
            .child(Node::new("span").text(v));
        form = form.child(label);
    }
    form
}

pub fn title_bar(title: &str, penalty: bool, eo: bool, tmpy: bool) -> Node {
    let vis = |on: bool| if on { "visibility: visible;" } else { "visibility: hidden;" };
    Node::el("div", "mfd-title-bar-container")
        .child(
            Node::el("div", "mfd-title-bar-title fr space-between")
                .child(Node::el("span", "mfd-label mfd-title-bar-text").text(&format!("{title} ")))
                .child(Node::el("span", "mfd-title-bar-text label penalty").style(vis(penalty)).text("PENALTY")),
        )
        .child(
            Node::el("div", "mfd-title-bar-section eo")
                .style("margin-left: 0px;")
                .child(Node::el("span", "mfd-label mfd-title-bar-text label eo").style(vis(eo)).text("EO")),
        )
        .child(
            Node::el("div", "mfd-title-bar-section tmpy")
                .style("margin-left: 0px;")
                .child(Node::el("span", "mfd-label mfd-title-bar-text label tmpy").style(vis(tmpy)).text("TMPY")),
        )
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct FooterModel {
    pub message: Option<String>,
    pub button_text: String,
}

fn footer_message_area(message: &Option<String>) -> Node {
    let mut span = Node::new("span");
    if let Some(m) = message {
        span = span.style("background-color: rgb(255, 255, 255);").text(m);
    }
    Node::el("div", "mfd-footer-message-area").child(span)
}

pub fn footer(m: &FooterModel) -> Node {
    let text = if m.button_text.is_empty() { "CLEAR\nINFO" } else { m.button_text.as_str() };
    let lab = Node::new("span").style("white-space: pre").text(text);
    let btn = Node::el("span", "mfd-button").style("width: 100px; ").child(Node::new("span").child(lab));
    let c = Node::el("div", "mfd-dropdown-container")
        .style("visibility: inherit;")
        .child(btn)
        .child(Node::el("div", "mfd-dropdown-menu").style("display: none"));
    Node::el("div", "mfd-footer").child(c).child(footer_message_area(&m.message))
}

pub fn atccom_footer(m: &FooterModel) -> Node {
    let btn = button(&ButtonOpts::new("CLEAR<br />INFO").style("width: 100px;"));
    let btn = wrap_span_label(btn);
    Node::el("div", "mfd-footer").child(btn).child(footer_message_area(&m.message))
}

fn wrap_span_label(mut container: Node) -> Node {
    use mfd_tree::Kid;
    if let Some(Kid::Node(btn)) = container.kids.get_mut(0) {
        let inner = std::mem::take(&mut btn.kids);
        let mut wrapper = Node::new("span");
        wrapper.kids = inner;
        btn.kids = vec![Kid::Node(wrapper)];
    }
    container
}

pub fn navigator(title: &str, body: Node, extras: Vec<Node>, footer: Node) -> Node {
    Node::el("div", "mfd-navigator-container")
        .child(title_bar(title, false, false, false))
        .child(body)
        .children(extras)
        .child(footer)
}

pub fn inop_overlay() -> Node {
    Node::new("div")
        .id("atccom-inop")
        .style("\n    position: absolute;\n    top: 132px;\n    width: 768px;\n    height: 818px;\n    display: flex;\n    justify-content: center;\n    align-items: center;\n    font-size: 35px;\n    background-color: rgba(0, 0, 0, 0.7);\n    color: #e68000")
        .child(Node::new("span").text("NOT YET IMPLEMENTED"))
}

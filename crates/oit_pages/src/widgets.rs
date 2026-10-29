use mfd_tree::{Kid, Node, Shape};
use serde::Deserialize;
use std::collections::BTreeMap;

fn shape(t: &str, pairs: &[(&str, &str)], depth: usize) -> Shape {
    let mut a: BTreeMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    a.insert("_d".into(), depth.to_string());
    a.insert("_display".into(), "inline".into());
    Shape { t: t.to_string(), a }
}

fn svg(attrs: &[(&str, &str)], shapes: Vec<Shape>) -> Node {
    let mut n = Node::new("svg");
    n.svgattr = attrs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    n.shapes = shapes;
    n
}

pub fn triangle_down() -> Node {
    svg(
        &[("height", "15"), ("width", "15")],
        vec![shape("polygon", &[("points", "0,0 15,0 7.5,15"), ("style", "fill: white;")], 0)],
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Icon {
    FlatSingleDown,
    SingleLeft,
    SingleRight,
}

pub fn icon_button(icon: Icon, disabled: bool, container_style: &str) -> Node {
    let fill = if disabled { "grey" } else { "white" };
    let (view, transform, points, width) = match icon {
        Icon::FlatSingleDown => ("0 0 35 35", "rotate(180 17.5 17.5)", "0,25 17.5,7.5 35,25", "35"),
        Icon::SingleLeft => ("0 0 15 15", "rotate(90 7.5 7.5)", "0,1 15,1 7.5,14", "35"),
        Icon::SingleRight => ("0 0 15 15", "rotate(270 7.5 7.5)", "0,1 15,1 7.5,14", "35"),
    };
    let mark = svg(
        &[("width", width), ("viewbox", view)],
        vec![
            shape("g", &[("fill", fill), ("transform", transform)], 0),
            shape("polygon", &[("points", points)], 1),
        ],
    );
    let mut n = Node::el("span", "mfd-icon-button").class_if("disabled", disabled).child(mark);
    n = n.style(container_style);
    n
}

pub fn label_into(mut n: Node, label: &str) -> Node {
    let mut first = true;
    for part in label.split("<br />") {
        if !first {
            n = n.child(Node::new("br"));
        }
        first = false;
        n = n.text(part);
    }
    n
}

pub fn button(label: &str, disabled: bool, container_style: &str, button_style: &str) -> Node {
    button_sel(label, disabled, false, container_style, button_style)
}

pub fn button_sel(label: &str, disabled: bool, selected: bool, container_style: &str, button_style: &str) -> Node {
    let mut inner = Node::el("span", "mfd-button").class_if("disabled", disabled).class_if("selected", selected);
    if !button_style.is_empty() {
        inner = inner.style(button_style);
    }
    inner = inner.child(label_into(Node::new("span"), label));
    let outer = Node::el("div", "mfd-dropdown-container").style(&inherit_visibility(container_style));
    outer.child(inner).child(Node::el("div", "mfd-dropdown-menu").style("display: none"))
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct PageItem {
    pub label: String,
    pub disabled: bool,
    pub separator: bool,
}

pub fn page_selector(label: &str, items: &[PageItem], id_prefix: &str, opened: bool, menu_style: &str) -> Node {
    let mut menu = Node::el("div", "mfd-dropdown-menu").style(&format!(
        "display: {};{}",
        if opened { "block" } else { "none" },
        menu_style
    ));
    for (i, it) in items.iter().enumerate() {
        let n = Node::el("span", "mfd-dropdown-menu-element")
            .class_if("disabled", it.disabled)
            .class_if("separator", it.separator)
            .id(&format!("{id_prefix}_{i}"))
            .text(&it.label);
        menu = menu.child(n);
    }
    let mut triangle = Node::el("div", "mfd-page-selector-label-triangle");
    if items.len() > 1 {
        triangle = triangle.child(Node::new("span").child(triangle_down()));
    }
    Node::el("div", "mfd-dropdown-container")
        .child(
            Node::el("div", "mfd-page-selector-outer")
                .child(
                    Node::el("div", "mfd-page-selector-label-container")
                        .child(Node::el("span", "mfd-page-selector-label").class_if("opened", opened).text(label)),
                )
                .child(triangle),
        )
        .child(menu)
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Field {
    pub text: String,
    pub caret: String,
    pub mandatory: bool,
    pub editing: bool,
    pub value_selected: bool,
    pub inactive: bool,
    pub disabled: bool,
    pub white: bool,
    pub validating: bool,
    pub computed_by_fms: bool,
    pub tmpy: bool,
    pub leading: String,
    pub trailing: String,
    pub container_style: String,
    pub align: String,
    pub max_digits: u32,
}

impl Field {
    pub fn text(t: &str) -> Field {
        Field { text: t.to_string(), ..Default::default() }
    }
}

pub fn input_field(f: &Field) -> Node {
    let text_input = Node::el("span", "mfd-input-field-text-input")
        .class_if("validating", f.validating)
        .class_if("computedByFms", f.computed_by_fms)
        .class_if("valueSelected", f.value_selected)
        .class_if("tmpy", f.tmpy)
        .class_if("editing", f.editing)
        .class_if("inactive", f.inactive)
        .class_if("white", f.white)
        .class_if("mandatory", f.mandatory)
        .class_if("disabled", f.disabled)
        .text(&f.text);
    let mut container = Node::el("div", "mfd-input-field-container")
        .class_if("inactive", f.inactive)
        .class_if("disabled", f.disabled);
    if !f.container_style.is_empty() {
        container = container.style(&f.container_style);
    }
    let value_null = f.text.chars().all(|c| c == '-' || c == ' ');
    let unit_vis = format!("visibility: {};", if f.inactive && value_null { "hidden" } else { "inherit" });
    let caret_display = format!("display: {};", if f.caret.is_empty() { "none" } else { "inline" });
    let container = container
        .child(Node::el("span", "mfd-label-unit mfd-unit-leading mfd-input-field-unit").style(&unit_vis).text(&f.leading))
        .child(
            Node::el("div", "mfd-input-field-text-input-container")
                .style(&format!(
                    "justify-content: {};{}",
                    if f.align.is_empty() { "center" } else { &f.align },
                    if f.max_digits > 0 { format!(" min-width: {}px;", (f.max_digits as f64 * 27.0 / 1.5).round()) } else { String::new() }
                ))
                .child(text_input)
                .child(Node::el("span", "mfd-input-field-caret").style(&caret_display).text(&f.caret)),
        )
        .child(Node::el("span", "mfd-label-unit mfd-unit-trailing mfd-input-field-unit").style(&unit_vis).text(&f.trailing));
    Node::el("div", "mfd-input-field-root").child(container)
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Dropdown {
    pub values: Vec<String>,
    pub text: String,
    pub editing: bool,
    pub value_selected: bool,
    pub inactive: bool,
    pub disabled: bool,
    pub container_style: String,
    pub digits: u32,
    pub opened: bool,
}

pub fn dropdown(id_prefix: &str, d: &Dropdown) -> Node {
    let field = Field {
        text: d.text.clone(),
        editing: d.editing,
        value_selected: d.value_selected,
        inactive: d.inactive,
        disabled: d.disabled,
        container_style: "border: 2px inset transparent".into(),
        max_digits: if d.digits > 0 { d.digits } else { 6 },
        ..Default::default()
    };
    let mut menu = Node::el("div", "mfd-dropdown-menu").style(if d.opened { "display: block" } else { "display: none" });
    for (i, v) in d.values.iter().enumerate() {
        menu = menu.child(
            Node::el("span", "mfd-dropdown-menu-element").id(&format!("{id_prefix}_{i}")).style("text-align: center;").text(v),
        );
    }
    let mut root = Node::el("div", "mfd-dropdown-container");
    if !d.container_style.is_empty() {
        root = root.style(&d.container_style);
    }
    root.child(
        Node::el("div", "mfd-dropdown-outer")
            .class_if("inactive", d.inactive)
            .class_if("disabled", d.disabled)
            .child(
                Node::el("div", "mfd-dropdown-inner")
                    .class_if("inactive", d.inactive)
                    .class_if("disabled", d.disabled)
                    .style("justify-content: center;")
                    .child(input_field(&field)),
            )
            .child(
                Node::el("div", "mfd-dropdown-arrow")
                    .class_if("inactive", d.inactive)
                    .class_if("disabled", d.disabled)
                    .child(triangle_down()),
            ),
    )
    .child(menu)
}

pub fn radio_group(id_prefix: &str, values: &[&str]) -> Node {
    let mut form = Node::new("form");
    for (i, v) in values.iter().enumerate() {
        form = form.child(
            Node::el("label", "mfd-radio-button cyan")
                .id(&format!("{id_prefix}_label_{i}"))
                .child(Node::new("input").id(&format!("{id_prefix}_{i}")))
                .child(Node::new("span").text(v)),
        );
    }
    form
}

pub fn text_node(t: &str) -> Kid {
    Kid::Text { text: t.to_string() }
}

fn inherit_visibility(style: &str) -> String {
    if style.is_empty() {
        "visibility: inherit;".to_string()
    } else {
        format!("{}; visibility: inherit;", style.trim_end().trim_end_matches(';'))
    }
}

pub fn div(classes: &str) -> Node {
    Node::el("div", classes)
}

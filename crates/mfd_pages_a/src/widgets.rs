use mfd_tree::{Node, Shape};
use serde::Deserialize;
use std::collections::BTreeMap;

pub const NBSP: &str = "\u{a0}";
const MENU_ITEM_PX: usize = 33;
const MENU_PAD_PX: usize = 6;

pub fn shape(t: &str, depth: usize, attrs: &[(&str, &str)]) -> Shape {
    let mut a: BTreeMap<String, String> = attrs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    a.insert("_d".into(), depth.to_string());
    a.insert("_display".into(), "inline".into());
    Shape { t: t.into(), a }
}

pub fn svg(cls: &str, attrs: &[(&str, &str)], shapes: Vec<Shape>) -> Node {
    let mut n = Node::el("svg", cls);
    n.svgattr = attrs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    n.shapes = shapes;
    n
}

pub fn triangle_down(cls: &str, color: &str) -> Node {
    let style = format!("fill: {};", color);
    svg(cls, &[("height", "15"), ("width", "15")], vec![shape("polygon", 0, &[("points", "0,0 15,0 7.5,15"), ("style", &style)])])
}

pub fn triangle_up(cls: &str, color: &str) -> Node {
    let style = format!("fill: {};", color);
    svg(cls, &[("height", "15"), ("width", "15")], vec![shape("polygon", 0, &[("points", "7.5,0 15,15 0,15"), ("style", &style)])])
}

#[derive(Clone, Debug)]
pub enum Label {
    Text(String),
    Node(Node),
}

impl Label {
    pub fn text(s: &str) -> Self {
        Label::Text(s.to_string())
    }

    fn put(&self, parent: Node) -> Node {
        match self {
            Label::Text(t) => parent.text(t),
            Label::Node(n) => parent.child(n.clone()),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MenuItem {
    pub label: String,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub separator: bool,
}

impl MenuItem {
    pub fn new(label: &str, disabled: bool) -> Self {
        MenuItem { label: label.to_string(), disabled, separator: false }
    }
}

#[derive(Clone, Debug)]
pub struct ButtonSpec {
    pub label: Label,
    pub disabled: bool,
    pub selected: bool,
    pub highlighted: bool,
    pub opened: bool,
    pub style: String,
    pub container_style: String,
    pub id_prefix: String,
    pub menu: Option<Vec<MenuItem>>,
    pub menu_opens_upwards: bool,
    pub menu_display_block: bool,
}

impl ButtonSpec {
    pub fn new(label: &str) -> Self {
        ButtonSpec {
            label: Label::text(label),
            disabled: false,
            selected: false,
            highlighted: false,
            opened: false,
            style: String::new(),
            container_style: String::new(),
            id_prefix: String::new(),
            menu: None,
            menu_opens_upwards: false,
            menu_display_block: false,
        }
    }

    pub fn node(label: Node) -> Self {
        let mut s = ButtonSpec::new("");
        s.label = Label::Node(label);
        s
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }

    pub fn style(mut self, s: &str) -> Self {
        self.style = s.to_string();
        self
    }

    pub fn menu(mut self, prefix: &str, items: Vec<MenuItem>, upwards: bool) -> Self {
        self.id_prefix = prefix.to_string();
        self.menu = Some(items);
        self.menu_opens_upwards = upwards;
        self
    }

    pub fn opened(mut self, v: bool) -> Self {
        self.opened = v;
        self
    }
}

pub fn button(spec: ButtonSpec) -> Node {
    let has_menu = spec.menu.is_some();
    let mut face = Node::el("span", "mfd-button")
        .class_if("disabled", spec.disabled)
        .class_if("selected", spec.selected)
        .class_if("highlighted", spec.highlighted)
        .class_if("opened", spec.opened)
        .style(&format!(
            "{} {}",
            if spec.style.is_empty() { "undefined" } else { spec.style.as_str() },
            if spec.menu.as_ref().is_some_and(|m| !m.is_empty()) { "padding-right: 5px;" } else { "" }
        ));
    if has_menu {
        let label = spec.label.put(Node::el("span", "mfd-fms-fpln-button-dropdown-label"));
        let up_hidden = !spec.menu_opens_upwards;
        let arrow = Node::el("span", "mfd-fms-fpln-button-dropdown-arrow")
            .child(triangle_up(if up_hidden { "hidden" } else { "" }, "white"))
            .child(triangle_down(if up_hidden { "" } else { "hidden" }, "white"));
        face = face.child(Node::el("div", "mfd-fms-fpln-button-dropdown").child(label).child(arrow));
    } else {
        face = face.child(spec.label.put(Node::new("span")));
    }
    let shown = spec.menu_display_block || (has_menu && spec.opened);
    let menu_top = match &spec.menu {
        Some(items) if spec.menu_opens_upwards && !items.is_empty() => format!("; top: -{}px;", MENU_ITEM_PX * items.len() + MENU_PAD_PX),
        _ => String::new(),
    };
    let mut menu = Node::el("div", "mfd-dropdown-menu")
        .style(&format!("display: {}{}", if shown { "block" } else { "none" }, menu_top));
    if let Some(items) = &spec.menu {
        if !items.is_empty() {
            let mut inner = Node::new("div");
            for (i, it) in items.iter().enumerate() {
                inner = inner.child(
                    Node::el("span", "mfd-dropdown-menu-element")
                        .id(&format!("{}_{}", spec.id_prefix, i))
                        .class_if("disabled", it.disabled)
                        .text(&it.label),
                );
            }
            menu = menu.child(inner);
        }
    }
    let container = if spec.container_style.is_empty() {
        "visibility: inherit;".to_string()
    } else {
        format!("{} visibility: inherit;", spec.container_style)
    };
    Node::el("div", "mfd-dropdown-container").style(&container).child(face).child(menu)
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct InputState {
    pub value_selected: bool,
    pub editing: bool,
    pub validating: bool,
    pub computed_by_fms: bool,
    pub tmpy: bool,
    pub inactive: bool,
    pub disabled: bool,
    pub mandatory: bool,
    pub white: bool,
    pub overflow: bool,
}

impl InputState {
    pub fn focused() -> Self {
        InputState { value_selected: true, editing: true, ..Default::default() }
    }
}

#[derive(Clone, Debug, Default)]
pub struct InputSpec {
    pub text: String,
    pub caret: String,
    pub leading_unit: String,
    pub trailing_unit: String,
    pub big_unit: bool,
    pub root_class: String,
    pub container_style: String,
    pub align: String,
    pub max_digits: usize,
    pub state: InputState,
}

impl InputSpec {
    pub fn new(text: &str) -> Self {
        InputSpec { text: text.to_string(), align: "center".into(), ..Default::default() }
    }

    pub fn state(mut self, s: InputState) -> Self {
        self.state = s;
        self
    }

    pub fn trailing(mut self, u: &str) -> Self {
        self.trailing_unit = u.to_string();
        self
    }

    pub fn leading(mut self, u: &str) -> Self {
        self.leading_unit = u.to_string();
        self
    }

    pub fn align(mut self, a: &str) -> Self {
        self.align = a.to_string();
        self
    }
}

pub fn input_field(spec: InputSpec) -> Node {
    let st = &spec.state;
    let unit_cls = |pos: &str| format!("mfd-label-unit {} mfd-unit-{} mfd-input-field-unit", if spec.big_unit { "bigger" } else { "" }, pos);
    let value_null = spec.text.chars().all(|c| c == '-' || c == ' ');
    let unit_vis = if st.inactive && value_null { "hidden" } else { "inherit" };
    let min_width = if spec.max_digits > 0 { format!(" min-width: {}px;", (spec.max_digits as f64 * 27.0 / 1.5).round()) } else { String::new() };
    let caret_display = if spec.caret.is_empty() { "none" } else { "inline" };
    let text = Node::el("span", "mfd-input-field-text-input")
        .class_if("validating", st.validating)
        .class_if("computedByFms", st.computed_by_fms)
        .class_if("valueSelected", st.value_selected)
        .class_if("tmpy", st.tmpy)
        .class_if("editing", st.editing)
        .class_if("inactive", st.inactive)
        .class_if("white", st.white)
        .class_if("mandatory", st.mandatory)
        .class_if("disabled", st.disabled)
        .text(&spec.text);
    let container = Node::el("div", "mfd-input-field-container")
        .class_if("inactive", st.inactive)
        .class_if("disabled", st.disabled)
        .class_if("overflow", st.overflow)
        .style(&spec.container_style)
        .child(Node::el("span", &unit_cls("leading")).style(&format!("visibility: {};", unit_vis)).text(&spec.leading_unit))
        .child(
            Node::el("div", "mfd-input-field-text-input-container")
                .style(&format!("justify-content: {};{}", spec.align, min_width))
                .child(text)
                .child(Node::el("span", "mfd-input-field-caret").style(&format!("display: {};", caret_display)).text(&spec.caret)),
        )
        .child(Node::el("span", &unit_cls("trailing")).style(&format!("visibility: {};", unit_vis)).text(&spec.trailing_unit));
    Node::el("div", &format!("mfd-input-field-root {}", spec.root_class)).class_if("overflow", st.overflow).child(container)
}

#[derive(Clone, Debug, Default)]
pub struct DropdownSpec {
    pub id_prefix: String,
    pub input: InputSpec,
    pub options: Vec<String>,
    pub inactive: bool,
    pub disabled: bool,
    pub container_style: String,
    pub align: String,
    pub open: bool,
    pub digits: usize,
}

pub fn dropdown_menu(spec: DropdownSpec) -> Node {
    let disabled_when_active = !spec.inactive && spec.disabled;
    let mut inner = Node::el("div", "mfd-dropdown-inner")
        .class_if("inactive", spec.inactive)
        .class_if("disabled", disabled_when_active)
        .style(&format!("justify-content: {};", spec.align));
    let mut input = spec.input.clone();
    input.container_style = "border: 2px inset transparent".into();
    input.align = spec.align.clone();
    input.max_digits = if spec.digits > 0 { spec.digits } else { 6 };
    input.state.inactive = spec.inactive;
    input.state.disabled = spec.disabled;
    inner = inner.child(input_field(input));
    let arrow = Node::el("div", "mfd-dropdown-arrow")
        .class_if("inactive", spec.inactive)
        .class_if("disabled", disabled_when_active)
        .child(triangle_down("", if spec.disabled { "gray" } else { "white" }));
    let outer = Node::el("div", "mfd-dropdown-outer")
        .class_if("inactive", spec.inactive)
        .class_if("disabled", disabled_when_active)
        .child(inner)
        .child(arrow);
    let mut menu = Node::el("div", "mfd-dropdown-menu").style(&format!("display: {}", if spec.open { "block" } else { "none" }));
    for (i, o) in spec.options.iter().enumerate() {
        menu = menu.child(
            Node::el("span", "mfd-dropdown-menu-element")
                .id(&format!("{}_{}", spec.id_prefix, i))
                .style(&format!("text-align: {};", spec.align))
                .text(o),
        );
    }
    Node::el("div", "mfd-dropdown-container").style(&spec.container_style).child(outer).child(menu)
}

pub fn page_selector(label: &str, active: bool, id_prefix: &str, items: &[MenuItem], open: bool) -> Node {
    let mut menu = Node::el("div", "mfd-dropdown-menu").style(&format!("display: {};", if open { "block" } else { "none" }));
    for (i, it) in items.iter().enumerate() {
        menu = menu.child(
            Node::el("span", "mfd-dropdown-menu-element")
                .id(&format!("{}_{}", id_prefix, i))
                .class_if("disabled", it.disabled)
                .class_if("separator", it.separator)
                .text(&it.label),
        );
    }
    let mut tri = Node::el("div", "mfd-page-selector-label-triangle");
    if items.len() > 1 {
        tri = tri.child(Node::new("span").child(triangle_down("", "white")));
    }
    Node::el("div", "mfd-dropdown-container")
        .style("flex: 1")
        .child(
            Node::el("div", "mfd-page-selector-outer")
                .child(
                    Node::el("div", "mfd-page-selector-label-container")
                        .child(Node::el("span", "mfd-page-selector-label").class_if("active", active).text(label)),
                )
                .child(tri),
        )
        .child(menu)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Icon {
    DoubleUp,
    DoubleDown,
    SingleUp,
    SingleDown,
}

pub fn icon_button(icon: Icon, disabled: bool, container_style: &str) -> Node {
    let fill = if disabled { "grey" } else { "white" };
    let two = |transform: Option<&str>| {
        let mut g: Vec<(&str, &str)> = vec![("fill", fill)];
        if let Some(t) = transform {
            g.push(("transform", t));
        }
        svg(
            "",
            &[("width", "35"), ("viewbox", "0 0 35 35")],
            vec![
                shape("g", 0, &g),
                shape("polygon", 1, &[("points", "0,17.5 17.5,0 35,17.5")]),
                shape("polygon", 1, &[("points", "0,35 17.5,17.5 35,35")]),
            ],
        )
    };
    let one = |points: &str| {
        svg(
            "",
            &[("width", "35"), ("viewbox", "0 0 15 15")],
            vec![shape("g", 0, &[("fill", fill)]), shape("polygon", 1, &[("points", points)])],
        )
    };
    let svg_node = match icon {
        Icon::DoubleUp => two(None),
        Icon::DoubleDown => two(Some("rotate(180 17.5 17.5)")),
        Icon::SingleUp => one("7.5,1 15,14 0,14"),
        Icon::SingleDown => one("0,1 15,1 7.5,14"),
    };
    Node::el("span", "mfd-icon-button").class_if("disabled", disabled).style(container_style).child(svg_node)
}

pub fn context_menu(id_prefix: &str, items: &[MenuItem], open: bool) -> Node {
    let mut n = Node::el("div", "mfd-context-menu").style(&format!("display: {}", if open { "block" } else { "none" }));
    let mut inner = Node::new("div");
    for (i, it) in items.iter().enumerate() {
        inner = inner.child(
            Node::el("span", "mfd-context-menu-element")
                .class_if("disabled", it.disabled)
                .id(&format!("{}_{}", id_prefix, i))
                .text(&it.label),
        );
    }
    n = n.child(inner);
    n
}

/// Names the FMS entry an input field edits (`data-key`), for fields no label precedes.
pub fn keyed(mut n: Node, key: &str) -> Node {
    n.attrs.insert("data-key".into(), key.to_string());
    n
}

pub fn conditional(condition: bool, if_true: Option<Node>, if_false: Option<Node>, width: Option<u32>) -> Node {
    let mut style = "display: flex; justify-content: center; align-items: center;".to_string();
    if let Some(w) = width {
        style.push_str(&format!(" width: {}px;", w));
    }
    let mut t = Node::new("div").style(&format!("display: {};", if condition { "block" } else { "none" }));
    if let Some(n) = if_true {
        t = t.child(n);
    }
    let mut f = Node::new("div").style(&format!("display: {};", if condition { "none" } else { "block" }));
    if let Some(n) = if_false {
        f = f.child(n);
    }
    Node::new("div").style(&style).child(t).child(f)
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct TitleBarModel {
    pub title: String,
    pub offset: String,
    pub eo: bool,
    pub tmpy: bool,
    pub penalty: bool,
    pub is_fms_page: bool,
}

pub fn title_bar(m: &TitleBarModel) -> Node {
    let vis = |on: bool| format!("visibility: {};", if on && m.is_fms_page { "visible" } else { "hidden" });
    let margin = if m.is_fms_page { "margin-left: 2px;" } else { "margin-left: 0px;" };
    let offset = if m.offset.is_empty() { String::new() } else { format!("{}OFFSET{}", "     ", m.offset) };
    Node::el("div", "mfd-title-bar-container")
        .child(
            Node::el("div", "mfd-title-bar-title fr space-between")
                .child(Node::el("span", "mfd-label mfd-title-bar-text").text(&m.title).text(&offset))
                .child(Node::el("span", "mfd-title-bar-text label penalty").style(&vis(m.penalty)).text("PENALTY")),
        )
        .child(
            Node::el("div", "mfd-title-bar-section eo")
                .style(margin)
                .child(Node::el("span", "mfd-label mfd-title-bar-text label eo").style(&vis(m.eo)).text("EO")),
        )
        .child(
            Node::el("div", "mfd-title-bar-section tmpy")
                .style(margin)
                .child(Node::el("span", "mfd-label mfd-title-bar-text label tmpy").style(&vis(m.tmpy)).text("TMPY")),
        )
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct FooterModel {
    pub message: Option<String>,
    pub message_background: String,
}

pub fn footer(m: &FooterModel) -> Node {
    let button_text = if m.message.is_some() { "CLEAR\nINFO" } else { "MSG\nLIST" };
    let mut msg = Node::new("span");
    if let Some(t) = &m.message {
        msg = msg.text(t);
        let bg = match m.message_background.as_str() {
            "white" => "rgb(255, 255, 255)",
            "cyan" => "rgb(0, 255, 255)",
            "amber" => "rgb(230, 128, 0)",
            _ => "",
        };
        if !bg.is_empty() {
            msg = msg.style(&format!("background-color: {};", bg));
        }
    }
    Node::el("div", "mfd-footer")
        .child(button(
            ButtonSpec::node(Node::new("span").style("white-space: pre").text(button_text)).style("width: 100px;"),
        ))
        .child(Node::el("div", "mfd-footer-message-area").child(msg))
}

pub fn mouse_cursor() -> Node {
    let stroke = "fill: none; stroke: rgb(255, 255, 0); stroke-width: 3;";
    let line = "stroke: rgb(255, 255, 0); stroke-width: 2;";
    Node::el("div", "mfd-mouse-cursor").style("display: none").child(svg(
        "",
        &[("width", "80"), ("height", "80")],
        vec![
            shape("g", 0, &[("transform", "rotate(0,40,40)")]),
            shape("polyline", 1, &[("points", "0,0 40,35 80,0"), ("style", stroke)]),
            shape("line", 1, &[("x1", "40"), ("y1", "39"), ("x2", "40"), ("y2", "41"), ("style", line)]),
            shape("line", 1, &[("x1", "39"), ("y1", "40"), ("x2", "41"), ("y2", "40"), ("style", line)]),
            shape("polyline", 1, &[("points", "0,80 40,45 80,80"), ("style", stroke)]),
        ],
    ))
}

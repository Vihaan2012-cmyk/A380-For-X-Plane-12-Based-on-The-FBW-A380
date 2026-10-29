use mfd_tree::{Kid, Node, Shape};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct Fmt {
    pub text: String,
    pub lead: String,
    pub trail: String,
    pub digits: usize,
    pub overflow_digits: Option<usize>,
    pub null: bool,
}

impl Fmt {
    pub fn new(text: &str, lead: &str, trail: &str, digits: usize, null: bool) -> Fmt {
        Fmt { text: text.to_string(), lead: lead.to_string(), trail: trail.to_string(), digits, overflow_digits: None, null }
    }

    pub fn overflow(mut self, n: usize) -> Fmt {
        self.overflow_digits = Some(n);
        self
    }
}

pub struct Field {
    fmt: Fmt,
    mandatory: bool,
    disabled: bool,
    inactive: bool,
    pilot: Option<bool>,
    class: String,
    container_style: String,
    align: String,
    tmpy: bool,
    focused: bool,
    white: bool,
    big_unit: bool,
    text_override: Option<String>,
    key: Option<String>,
}

pub fn field(fmt: Fmt) -> Field {
    Field {
        fmt,
        mandatory: false,
        disabled: false,
        inactive: false,
        pilot: None,
        class: String::new(),
        container_style: String::new(),
        align: "flex-end".to_string(),
        tmpy: false,
        focused: false,
        white: false,
        big_unit: false,
        text_override: None,
        key: None,
    }
}

impl Field {
    pub fn mandatory(mut self, v: bool) -> Self {
        self.mandatory = v;
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn inactive(mut self, v: bool) -> Self {
        self.inactive = v;
        self
    }
    pub fn pilot(mut self, v: bool) -> Self {
        self.pilot = Some(v);
        self
    }
    pub fn class(mut self, v: &str) -> Self {
        self.class = v.to_string();
        self
    }
    pub fn cstyle(mut self, v: &str) -> Self {
        self.container_style = v.to_string();
        self
    }
    pub fn align(mut self, v: &str) -> Self {
        self.align = v.to_string();
        self
    }
    pub fn tmpy(mut self, v: bool) -> Self {
        self.tmpy = v;
        self
    }
    pub fn focused(mut self, v: bool) -> Self {
        self.focused = v;
        self
    }
    pub fn white(mut self, v: bool) -> Self {
        self.white = v;
        self
    }
    pub fn big_unit(mut self, v: bool) -> Self {
        self.big_unit = v;
        self
    }
    pub fn typed(mut self, v: Option<&str>) -> Self {
        self.text_override = v.map(str::to_string);
        self
    }
    /// Names the FMS entry the field edits (`data-key`), for fields no label precedes.
    pub fn key(mut self, v: &str) -> Self {
        self.key = Some(v.to_string());
        self
    }

    pub fn node(self) -> Node {
        let f = &self.fmt;
        let mandatory_class = self.mandatory && f.null && !(self.disabled && !self.inactive);
        let text = match &self.text_override {
            Some(t) => t.clone(),
            None if f.null && self.mandatory && !self.inactive && !self.disabled => f.text.replace('-', "\u{25AF}"),
            None => f.text.clone(),
        };
        let overflow = match f.overflow_digits {
            Some(_) => f.text.len() > f.digits && !f.null,
            None => false,
        };
        let unit_vis = if f.null && self.inactive { "visibility: hidden;" } else { "visibility: inherit;" };
        let eff_disabled = self.disabled && !self.inactive;
        let min_width = format!("{}px", ((f.digits as f64 * 27.0) / 1.5).round() as i64);
        let bigger = if self.big_unit { " bigger" } else { "" };
        let root_cls = format!("mfd-input-field-root {}{}", self.class, if overflow { " overflow" } else { "" });
        let mut container = Node::el("div", "mfd-input-field-container")
            .class_if("inactive", self.inactive)
            .class_if("disabled", eff_disabled)
            .class_if("overflow", overflow);
        if !self.container_style.is_empty() {
            container = container.style(&self.container_style);
        }
        let mut text_cls = Node::el("span", "mfd-input-field-text-input")
            .class_if("computedByFms", self.pilot == Some(false))
            .class_if("valueSelected", self.focused)
            .class_if("tmpy", self.tmpy && !self.focused)
            .class_if("editing", self.focused)
            .class_if("inactive", self.inactive)
            .class_if("white", self.white)
            .class_if("mandatory", mandatory_class)
            .class_if("disabled", eff_disabled);
        text_cls = text_cls.text(&text);
        let mut root = Node::el("div", root_cls.trim());
        if let Some(k) = &self.key {
            root.attrs.insert("data-key".into(), k.clone());
        }
        root
            .child(
                container
                    .child(
                        Node::el("span", &format!("mfd-label-unit{} mfd-unit-leading mfd-input-field-unit", bigger))
                            .style(unit_vis)
                            .text(&f.lead),
                    )
                    .child(
                        Node::el("div", "mfd-input-field-text-input-container")
                            .style(&format!("justify-content: {}; min-width: {};", self.align, min_width))
                            .child(text_cls)
                            .child(Node::el("span", "mfd-input-field-caret").style("display: none;")),
                    )
                    .child(
                        Node::el("span", &format!("mfd-label-unit{} mfd-unit-trailing mfd-input-field-unit", bigger))
                            .style(unit_vis)
                            .text(&f.trail),
                    ),
            )
    }
}

pub enum Label<'a> {
    Text(&'a str),
    Node(Node),
}

pub fn text_with_br(mut n: Node, s: &str) -> Node {
    let mut first = true;
    for part in s.split("<br />") {
        if !first {
            n = n.child(Node::new("br"));
        }
        n = n.text(part);
        first = false;
    }
    n
}

pub struct Btn<'a> {
    label: Label<'a>,
    disabled: bool,
    visible: bool,
    selected: bool,
    highlighted: bool,
    opened: bool,
    style: String,
    container_style: String,
    menu: Option<(Vec<(String, bool)>, String)>,
    show_arrow: bool,
    menu_up: bool,
}

pub fn button(label: &str) -> Btn<'_> {
    Btn::with(Label::Text(label))
}

pub fn button_node(label: Node) -> Btn<'static> {
    Btn::with(Label::Node(label))
}

impl<'a> Btn<'a> {
    fn with(label: Label<'a>) -> Btn<'a> {
        Btn {
            label,
            disabled: false,
            visible: true,
            selected: false,
            highlighted: false,
            opened: false,
            style: String::new(),
            container_style: String::new(),
            menu: None,
            show_arrow: true,
            menu_up: false,
        }
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn visible(mut self, v: bool) -> Self {
        self.visible = v;
        self
    }
    pub fn selected(mut self, v: bool) -> Self {
        self.selected = v;
        self
    }
    pub fn highlighted(mut self, v: bool) -> Self {
        self.highlighted = v;
        self
    }
    pub fn opened(mut self, v: bool) -> Self {
        self.opened = v;
        self
    }
    pub fn style(mut self, v: &str) -> Self {
        self.style = v.to_string();
        self
    }
    pub fn cstyle(mut self, v: &str) -> Self {
        self.container_style = v.to_string();
        self
    }
    pub fn menu(mut self, items: Vec<(String, bool)>, id_prefix: &str) -> Self {
        self.menu = Some((items, id_prefix.to_string()));
        self
    }
    pub fn show_arrow(mut self, v: bool) -> Self {
        self.show_arrow = v;
        self
    }

    pub fn node(self) -> Node {
        let label = match self.label {
            Label::Text(t) => text_with_br(Node::new("span"), t),
            Label::Node(n) => Node::new("span").child(n),
        };
        let has_items = self.menu.as_ref().map(|m| !m.0.is_empty()).unwrap_or(false);
        let inner = if self.menu.is_some() && self.show_arrow {
            let arrow_color = if self.disabled { "grey" } else { "white" };
            let lbl = match label.kids.into_iter().next() {
                Some(Kid::Node(n)) => Node::el("span", "mfd-fms-fpln-button-dropdown-label").child(n),
                Some(Kid::Text { text }) => Node::el("span", "mfd-fms-fpln-button-dropdown-label").text(&text),
                None => Node::el("span", "mfd-fms-fpln-button-dropdown-label"),
            };
            Node::el("div", "mfd-fms-fpln-button-dropdown").child(lbl).child(
                Node::el("span", "mfd-fms-fpln-button-dropdown-arrow")
                    .child(triangle(true, !self.menu_up, arrow_color))
                    .child(triangle(false, self.menu_up, arrow_color)),
            )
        } else {
            label
        };
        let style = format!("{} {}", self.style, if has_items { "padding-right: 5px;" } else { "" });
        let mut menu_div = Node::el("div", "mfd-dropdown-menu")
            .style(&format!("display: {}", if self.opened { "block" } else { "none" }));
        if let Some((items, prefix)) = &self.menu {
            let mut wrapper = Node::new("div");
            for (i, (label, dis)) in items.iter().enumerate() {
                wrapper = wrapper.child(
                    Node::el("span", "mfd-dropdown-menu-element")
                        .id(&format!("{}_{}", prefix, i))
                        .class_if("disabled", *dis)
                        .text(label),
                );
            }
            menu_div = menu_div.child(wrapper);
        }
        let mut cstyle = self.container_style.clone();
        if !cstyle.is_empty() && !cstyle.ends_with(';') {
            cstyle.push(';');
        }
        cstyle.push_str(if self.visible { "visibility: inherit;" } else { "visibility: hidden;" });
        Node::el("div", "mfd-dropdown-container").style(&cstyle).child(
            Node::el("span", "mfd-button")
                .class_if("disabled", self.disabled)
                .class_if("selected", self.selected)
                .class_if("highlighted", self.highlighted)
                .class_if("opened", self.opened)
                .style(&style)
                .child(inner),
        ).child(menu_div)
    }
}

pub fn svg_node(w: &str, h: &str, shapes: Vec<Shape>) -> Node {
    let mut n = Node::new("svg");
    n.svgattr = BTreeMap::from([("height".to_string(), h.to_string()), ("width".to_string(), w.to_string())]);
    n.shapes = shapes;
    n
}

pub fn shape(t: &str, attrs: &[(&str, &str)]) -> Shape {
    let mut a: BTreeMap<String, String> = attrs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    a.insert("_d".to_string(), "0".to_string());
    a.insert("_display".to_string(), "inline".to_string());
    Shape { t: t.to_string(), a }
}

pub fn triangle(up: bool, hidden: bool, color: &str) -> Node {
    let points = if up { "0,15 15,15 7.5,0" } else { "0,0 15,0 7.5,15" };
    let mut n = svg_node("15", "15", vec![shape("polygon", &[("points", points), ("style", &format!("fill: {};", color))])]);
    if hidden {
        n.cls.push("hidden".to_string());
    }
    n
}

pub struct Dropdown {
    pub id_prefix: String,
    pub values: Vec<String>,
    pub selected: Option<usize>,
    pub disabled: bool,
    pub inactive: bool,
    pub container_style: String,
    pub align: String,
    pub digits: usize,
    pub open: bool,
}

impl Dropdown {
    pub fn new(id_prefix: &str, values: &[&str], selected: Option<usize>) -> Dropdown {
        Dropdown {
            id_prefix: id_prefix.to_string(),
            values: values.iter().map(|s| s.to_string()).collect(),
            selected,
            disabled: false,
            inactive: false,
            container_style: String::new(),
            align: "center".to_string(),
            digits: 6,
            open: false,
        }
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn inactive(mut self, v: bool) -> Self {
        self.inactive = v;
        self
    }
    pub fn cstyle(mut self, v: &str) -> Self {
        self.container_style = v.to_string();
        self
    }
    pub fn align(mut self, v: &str) -> Self {
        self.align = v.to_string();
        self
    }
    pub fn digits(mut self, v: usize) -> Self {
        self.digits = v;
        self
    }
    pub fn open(mut self, v: bool) -> Self {
        self.open = v;
        self
    }

    pub fn node(self) -> Node {
        let label = self.selected.and_then(|i| self.values.get(i)).cloned().unwrap_or_default();
        let fmt = if label.is_empty() {
            Fmt::new(&"-".repeat(self.digits), "", "", self.digits, false)
        } else {
            Fmt::new(&label, "", "", self.digits, false)
        };
        let dis_active = self.disabled && !self.inactive;
        let input = field(fmt)
            .cstyle("border: 2px inset transparent")
            .align(&self.align)
            .inactive(self.inactive)
            .disabled(self.disabled)
            .focused(self.open && !self.inactive && !self.disabled)
            .node();
        let mut menu = Node::el("div", "mfd-dropdown-menu").style(&format!("display: {};", if self.open { "block" } else { "none" }));
        for (i, v) in self.values.iter().enumerate() {
            menu = menu.child(
                Node::el("span", "mfd-dropdown-menu-element")
                    .id(&format!("{}_{}", self.id_prefix, i))
                    .style(&format!("text-align: {};", self.align))
                    .text(v),
            );
        }
        let fill = if self.disabled { "gray" } else { "white" };
        let mut container = Node::el("div", "mfd-dropdown-container");
        if !self.container_style.is_empty() {
            container = container.style(&self.container_style);
        }
        container
            .child(
                Node::el("div", "mfd-dropdown-outer")
                    .class_if("inactive", self.inactive)
                    .class_if("disabled", dis_active)
                    .child(
                        Node::el("div", "mfd-dropdown-inner")
                            .class_if("inactive", self.inactive)
                            .class_if("disabled", dis_active)
                            .style(&format!("justify-content: {};", self.align))
                            .child(input),
                    )
                    .child(
                        Node::el("div", "mfd-dropdown-arrow")
                            .class_if("inactive", self.inactive)
                            .class_if("disabled", dis_active)
                            .child(svg_node(
                                "15",
                                "15",
                                vec![shape("polygon", &[("points", "0,0 15,0 7.5,15"), ("style", &format!("fill: {};", fill))])],
                            )),
                    ),
            )
            .child(menu)
    }
}

pub fn radio_group(values: &[&str], id_prefix: &str, vspace: i32, selected: Option<usize>, disabled: &[bool]) -> Node {
    let mut form = Node::new("form");
    for (i, v) in values.iter().enumerate() {
        let mut label = Node::el("label", "mfd-radio-button cyan").id(&format!("{}_label_{}", id_prefix, i));
        if vspace != 0 {
            label = label.style(&format!("margin-top: {}px;", vspace));
        }
        label.attrs.insert("for".into(), format!("{}_{}", id_prefix, i));
        let mut input = Node::new("input").id(&format!("{}_{}", id_prefix, i));
        input.attrs.insert("type".into(), "radio".into());
        input.attrs.insert("name".into(), id_prefix.into());
        if selected == Some(i) {
            input.attrs.insert("checked".into(), String::new());
        }
        if disabled.get(i).copied().unwrap_or(false) {
            input.attrs.insert("disabled".into(), String::new());
        }
        form = form.child(label.child(input).child(Node::new("span").text(v)));
    }
    form
}

pub fn conditional(width: Option<i32>, height: Option<i32>, cond: bool, if_true: Vec<Node>, if_false: Vec<Node>) -> Node {
    let mut style = String::from("display: flex; justify-content: center; align-items: center;");
    if let Some(w) = width {
        style.push_str(&format!(" width: {}px;", w));
    }
    if let Some(h) = height {
        style.push_str(&format!(" height: {}px;", h));
    }
    Node::new("div").style(&style)
        .child(Node::new("div").style(&format!("display: {};", if cond { "block" } else { "none" })).children(if_true))
        .child(Node::new("div").style(&format!("display: {};", if cond { "none" } else { "block" })).children(if_false))
}

pub fn conditional_plain(cond: bool, if_true: Vec<Node>, if_false: Vec<Node>) -> Node {
    conditional(None, None, cond, if_true, if_false)
}

pub fn confirmation_dialog(visible: bool, style: &str, amber: bool, content: Vec<Node>, title_text: &str) -> Node {
    let mut title = Node::el("span", "mfd-label").class_if("amber", amber).text(title_text);
    title = title.children(content);
    Node::new("div")
        .style(&format!("position: relative; display: {};", if visible { "block" } else { "none" }))
        .child(
            Node::el("div", "mfd-dialog")
                .style(style)
                .child(Node::el("div", "mfd-dialog-title").child(title))
                .child(
                    Node::el("div", "mfd-dialog-buttons")
                        .child(button("CANCEL").node())
                        .child(button("CONFIRM *").style("padding-right: 6px;").node()),
                ),
        )
}

pub fn label_value(style: &str) -> Node {
    let n = Node::el("div", "mfd-label-value-container");
    if style.is_empty() {
        n
    } else {
        n.style(style)
    }
}

pub fn div(style: &str) -> Node {
    let n = Node::new("div");
    if style.is_empty() {
        n
    } else {
        n.style(style)
    }
}

pub fn div_c(class: &str, style: &str) -> Node {
    let n = Node::el("div", class);
    if style.is_empty() {
        n
    } else {
        n.style(style)
    }
}

pub fn span_c(class: &str, text: &str) -> Node {
    Node::el("span", class).text(text)
}

pub fn circle_svg() -> Node {
    let mut n = Node::new("svg");
    n.svgattr = BTreeMap::from([
        ("height".to_string(), "13".to_string()),
        ("viewBox".to_string(), "0 0 13 13".to_string()),
        ("width".to_string(), "13".to_string()),
    ]);
    n.shapes = vec![shape("circle", &[("cx", "6"), ("cy", "6"), ("r", "5"), ("stroke", "#00ff00"), ("stroke-width", "2")])];
    n
}

pub fn arrow_svg() -> Node {
    let mut n = Node::new("svg");
    n.svgattr = BTreeMap::from([
        ("fill".to_string(), "#ffffff".to_string()),
        ("height".to_string(), "35px".to_string()),
        ("viewBox".to_string(), "0 0 60 60".to_string()),
        ("width".to_string(), "35px".to_string()),
    ]);
    n.shapes = vec![shape("polygon", &[("points", "0,28 50,28 50,20 60,30 50,40 50,32 0,32")])];
    n
}

pub fn two_line_star_label(l1: &str, l2: &str) -> Node {
    star_label("display: flex; flex-direction: row;", l1, l2)
}

pub fn two_line_star_label_spaced(l1: &str, l2: &str) -> Node {
    star_label("display: flex; flex-direction: row; justify-content: space-between;", l1, l2)
}

fn star_label(style: &str, l1: &str, l2: &str) -> Node {
    Node::new("div")
        .style(style)
        .child(Node::new("span").style("text-align: center; vertical-align: center; margin-right: 10px;").text(l1).child(Node::new("br")).text(l2))
        .child(Node::new("span").style("display: flex; align-items: center; justify-content: center;").text("*"))
}

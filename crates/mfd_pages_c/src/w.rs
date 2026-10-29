use mfd_tree::{Kid, Node, Shape};
use std::collections::BTreeMap;

pub const NBSP: &str = "\u{a0}";

pub fn shape(t: &str, depth: u32, attrs: &[(&str, &str)]) -> Shape {
    let mut a: BTreeMap<String, String> = attrs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    a.insert("_d".to_string(), depth.to_string());
    Shape { t: t.to_string(), a }
}

pub fn svg(attrs: &[(&str, &str)], shapes: Vec<Shape>) -> Node {
    let mut n = Node::new("svg");
    n.svgattr = attrs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    n.shapes = shapes;
    n
}

pub fn label(text: &str) -> Node {
    Node::el("span", "mfd-label").text(text)
}

pub fn label_big(text: &str) -> Node {
    Node::el("span", "mfd-label bigger").text(text)
}

pub fn value(text: &str) -> Node {
    Node::el("span", "mfd-value").text(text)
}

pub fn value_big(text: &str) -> Node {
    Node::el("span", "mfd-value bigger").text(text)
}

pub fn spacer() -> Node {
    Node::new("div").style("flex-grow: 1;")
}

#[derive(Clone, Debug, Default)]
pub struct Btn {
    pub disabled: bool,
    pub selected: bool,
    pub highlighted: bool,
    pub button_style: Option<String>,
    pub container_style: Option<String>,
    pub dropdown_arrow: Option<bool>,
}

impl Btn {
    pub fn new() -> Self {
        Btn::default()
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn selected(mut self, d: bool) -> Self {
        self.selected = d;
        self
    }
    pub fn style(mut self, s: &str) -> Self {
        self.button_style = Some(s.to_string());
        self
    }
    pub fn container(mut self, s: &str) -> Self {
        self.container_style = Some(s.to_string());
        self
    }
}

pub fn button_inner(inner: Node, b: &Btn) -> Node {
    let btn = Node::el("span", "mfd-button")
        .class_if("disabled", b.disabled)
        .class_if("selected", b.selected)
        .class_if("highlighted", b.highlighted)
        .style(&format!("{} ", b.button_style.as_deref().unwrap_or("undefined")))
        .child(inner);
    let cs = match &b.container_style {
        Some(c) => format!("{} visibility: inherit;", c),
        None => "visibility: inherit;".to_string(),
    };
    Node::el("div", "mfd-dropdown-container")
        .style(&cs)
        .child(btn)
        .child(Node::el("div", "mfd-dropdown-menu").style("display: none"))
}

pub fn button(text: &str, b: &Btn) -> Node {
    button_inner(Node::new("span").text(text), b)
}

pub fn button_label_node(label: Node, b: &Btn) -> Node {
    button_inner(Node::new("span").child(label), b)
}

pub fn menu_button(text: &str, b: &Btn) -> Node {
    let btn = Node::el("span", "mfd-button")
        .class_if("disabled", b.disabled)
        .class_if("selected", b.selected)
        .style(&format!("{} padding-right: 5px;", b.button_style.as_deref().unwrap_or("undefined")))
        .child(
            Node::el("div", "mfd-fms-fpln-button-dropdown")
                .child(Node::el("span", "mfd-fms-fpln-button-dropdown-label").text(text))
                .child(
                    Node::el("span", "mfd-fms-fpln-button-dropdown-arrow")
                        .child(triangle("up", true))
                        .child(triangle("down", false)),
                ),
        );
    let cs = match &b.container_style {
        Some(c) => format!("{} visibility: inherit;", c),
        None => "visibility: inherit;".to_string(),
    };
    Node::el("div", "mfd-dropdown-container")
        .style(&cs)
        .child(btn)
        .child(Node::el("div", "mfd-dropdown-menu").style("display: none"))
}

fn triangle(dir: &str, hidden: bool) -> Node {
    let pts = if dir == "up" { "0,15 7.5,0 15,15" } else { "0,0 15,0 7.5,15" };
    let n = svg(
        &[("height", "15"), ("width", "15")],
        vec![shape("polygon", 0, &[("points", pts), ("style", "fill: #ffffff")])],
    );
    let mut n = n;
    if hidden {
        n.cls.push("hidden".to_string());
    }
    n
}

pub fn icon_button(icon: &str, disabled: bool, container_style: &str) -> Node {
    let poly = |pts: &'static str| shape("polygon", 1, &[("points", pts)]);
    let fill = if disabled { "grey" } else { "#ffffff" };
    let g = |rot: Option<&'static str>, kids: Vec<Shape>| {
        let mut v = vec![match rot {
            Some(r) => shape("g", 0, &[("fill", fill), ("transform", r)]),
            None => shape("g", 0, &[("fill", fill)]),
        }];
        v.extend(kids);
        v
    };
    let dbl = || vec![poly("0,17.5 17.5,0 35,17.5"), poly("0,35 17.5,17.5 35,35")];
    let sv = match icon {
        "double-left" => svg(&[("width", "35"), ("height", "35")], g(Some("rotate(270 17.5 17.5)"), dbl())),
        "double-right" => svg(&[("width", "35"), ("height", "35")], g(Some("rotate(90 17.5 17.5)"), dbl())),
        "double-up" => svg(&[("width", "35"), ("viewBox", "0 0 35 35")], g(None, dbl())),
        "double-down" => svg(&[("width", "35"), ("viewBox", "0 0 35 35")], g(Some("rotate(180 17.5 17.5)"), dbl())),
        "single-up" => svg(&[("width", "35"), ("viewBox", "0 0 15 15")], g(None, vec![poly("7.5,1 15,14 0,14")])),
        "single-down" => svg(&[("width", "35"), ("viewBox", "0 0 15 15")], g(None, vec![poly("0,1 15,1 7.5,14")])),
        "single-left" => svg(&[("width", "35"), ("height", "35")], g(Some("rotate(90 7.5 7.5)"), vec![poly("0,1 15,1 7.5,14")])),
        "single-right" => svg(&[("width", "35"), ("height", "35")], g(Some("rotate(270 7.5 7.5)"), vec![poly("0,1 15,1 7.5,14")])),
        _ => svg(&[("width", "35")], vec![]),
    };
    Node::el("span", "mfd-icon-button").class_if("disabled", disabled).style(container_style).child(sv)
}

#[derive(Clone, Debug, Default)]
pub struct Field {
    pub text: String,
    pub leading: String,
    pub trailing: String,
    pub text_classes: Vec<&'static str>,
    pub root_class: String,
    pub container_style: String,
    pub container_inactive: bool,
    pub align: String,
    pub min_width: Option<u32>,
    pub big_unit: bool,
}

impl Field {
    pub fn new(text: &str) -> Self {
        Field { text: text.to_string(), align: "flex-end".to_string(), ..Default::default() }
    }
    pub fn units(mut self, leading: &str, trailing: &str) -> Self {
        self.leading = leading.to_string();
        self.trailing = trailing.to_string();
        self
    }
    pub fn cls(mut self, c: &[&'static str]) -> Self {
        self.text_classes.extend_from_slice(c);
        self
    }
    pub fn style(mut self, s: &str) -> Self {
        self.container_style = s.to_string();
        self
    }
    pub fn align(mut self, a: &str) -> Self {
        self.align = a.to_string();
        self
    }
    pub fn min_width(mut self, w: u32) -> Self {
        self.min_width = Some(w);
        self
    }
    pub fn root(mut self, c: &str) -> Self {
        self.root_class = c.to_string();
        self
    }
    pub fn big_unit(mut self, b: bool) -> Self {
        self.big_unit = b;
        self
    }
}

pub fn input_field(f: &Field) -> Node {
    let unit_cls = |pos: &str| {
        format!("mfd-label-unit {} mfd-unit-{} mfd-input-field-unit", if f.big_unit { "bigger" } else { "" }, pos)
    };
    let mut txt = Node::el("span", "mfd-input-field-text-input");
    for c in &f.text_classes {
        txt.cls.push(c.to_string());
    }
    let txt = txt.text(&f.text);
    let mut tc_style = format!("justify-content: {};", f.align);
    if let Some(w) = f.min_width {
        tc_style.push_str(&format!(" min-width: {}px;", w));
    }
    let mut container = Node::el("div", "mfd-input-field-container").class_if("inactive", f.container_inactive);
    if !f.container_style.is_empty() {
        container = container.style(&f.container_style);
    }
    let container = container
        .child(Node::el("span", &unit_cls("leading")).style("visibility: inherit;").text(&f.leading))
        .child(
            Node::el("div", "mfd-input-field-text-input-container")
                .style(&tc_style)
                .child(txt)
                .child(Node::el("span", "mfd-input-field-caret").style("display: none;")),
        )
        .child(Node::el("span", &unit_cls("trailing")).style("visibility: inherit;").text(&f.trailing));
    let mut root = Node::el("div", "mfd-input-field-root");
    for c in f.root_class.split_whitespace() {
        root.cls.push(c.to_string());
    }
    root.child(container)
}

pub struct Dropdown {
    pub text: String,
    pub container_style: Option<String>,
    pub inactive: bool,
    pub disabled: bool,
    pub align: String,
    pub text_classes: Vec<&'static str>,
    pub items: Vec<String>,
    pub id_prefix: String,
}

impl Dropdown {
    pub fn new(text: &str) -> Self {
        Dropdown {
            text: text.to_string(),
            container_style: None,
            inactive: false,
            disabled: false,
            align: "center".to_string(),
            text_classes: vec![],
            items: vec![],
            id_prefix: String::new(),
        }
    }
}

pub fn dropdown(d: &Dropdown) -> Node {
    let field = Field {
        text: if d.text.is_empty() { "------".to_string() } else { d.text.clone() },
        container_style: "border: 2px inset transparent".to_string(),
        align: d.align.clone(),
        text_classes: d.text_classes.clone(),
        container_inactive: d.inactive,
        ..Field::new("")
    };
    let mut c = Node::el("div", "mfd-dropdown-container");
    if let Some(s) = &d.container_style {
        c = c.style(s);
    }
    c.child(
        Node::el("div", "mfd-dropdown-outer")
            .class_if("inactive", d.inactive)
            .class_if("disabled", d.disabled)
            .child(
                Node::el("div", "mfd-dropdown-inner")
                    .class_if("inactive", d.inactive)
                    .class_if("disabled", d.disabled)
                    .style(&format!("justify-content: {};", d.align))
                    .child(input_field(&field)),
            )
            .child(
                Node::el("div", "mfd-dropdown-arrow")
                    .class_if("inactive", d.inactive)
                    .class_if("disabled", d.disabled)
                    .child(svg(
                        &[("height", "15"), ("width", "15")],
                        vec![shape("polygon", 0, &[("points", "0,0 15,0 7.5,15"), ("style", "fill: #ffffff")])],
                    )),
            ),
    )
    .child(
        Node::el("div", "mfd-dropdown-menu").style("display: none").children(
            d.items
                .iter()
                .enumerate()
                .map(|(i, t)| Node::el("span", "mfd-dropdown-menu-element").id(&format!("{}_{}", d.id_prefix, i)).text(t)),
        ),
    )
}

pub fn empty() -> Node {
    Node::new("")
}

pub fn conditional(cond: bool, if_true: Node, if_false: Node, style: Option<&str>) -> Node {
    let st = style.unwrap_or("display: flex; justify-content: center; align-items: center;");
    let mut outer = Node::new("div");
    if !st.is_empty() {
        outer = outer.style(st);
    }
    outer
        .child(Node::new("div").style(if cond { "display: block;" } else { "display: none;" }).child(if_true))
        .child({
            let f = Node::new("div").style(if cond { "display: none;" } else { "display: block;" });
            if if_false.tag.is_empty() {
                f
            } else {
                f.child(if_false)
            }
        })
}

pub fn confirmation_dialog(visible: bool, title: &str, content_style: &str) -> Node {
    Node::new("div")
        .style(if visible { "position: relative; display: block;" } else { "position: relative; display: none;" })
        .child(
            Node::el("div", "mfd-dialog")
                .style(content_style)
                .child(Node::el("div", "mfd-dialog-title").child(Node::el("span", "mfd-label").text(title)))
                .child(
                    Node::el("div", "mfd-dialog-buttons")
                        .child(button("CANCEL", &Btn::new()))
                        .child(button("CONFIRM *", &Btn::new().style("padding-right: 6px;"))),
                ),
        )
}

pub struct Tabs<'a> {
    pub titles: &'a [&'a str],
    pub selected: usize,
    pub selected_color: &'a str,
    pub angle: f64,
    pub height: f64,
    pub highlighted: Option<usize>,
    pub additional_right_space: f64,
}

impl<'a> Tabs<'a> {
    pub fn new(titles: &'a [&'a str], selected: usize) -> Self {
        Tabs { titles, selected, selected_color: "white", angle: 30.0, height: 36.0, highlighted: None, additional_right_space: 0.0 }
    }
    pub fn angle(mut self, a: f64) -> Self {
        self.angle = a;
        self
    }
    pub fn color(mut self, c: &'a str) -> Self {
        self.selected_color = c;
        self
    }
}

fn js_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

fn tab_element(title: &str, sel: bool, last: bool, hl: bool, t: &Tabs, small: bool) -> Node {
    let h = t.height;
    let tw = h * (t.angle * std::f64::consts::PI / 180.0).tan();
    let hs = js_num(h);
    let tws = js_num(tw);
    let fill = if sel { "fill:#000000;" } else { "fill:#272525;" };
    let line = "stroke: lightgrey; stroke-width:2";
    let mut left = vec![
        shape("polygon", 0, &[("points", &format!("0,{} {},0 {},{}", hs, tws, tws, hs)), ("style", fill)]),
        shape("line", 0, &[("x1", "0"), ("y1", &hs), ("x2", &tws), ("y2", "0"), ("style", line)]),
    ];
    let mut right = vec![
        shape("polygon", 0, &[("points", &format!("0,0 {},{} 0,{}", tws, hs, hs)), ("style", fill)]),
        shape("line", 0, &[("x1", "0"), ("y1", "0"), ("x2", &tws), ("y2", &hs), ("style", line)]),
    ];
    if !sel {
        let y = js_num(h - 1.0);
        left.push(shape("line", 0, &[("x1", "0"), ("y1", &y), ("x2", &tws), ("y2", &y), ("style", line)]));
        right.push(shape("line", 0, &[("x1", "0"), ("y1", &y), ("x2", &tws), ("y2", &y), ("style", line)]));
    }
    let fs = if small { (h * 0.58).round() } else { (h * 0.66).round() };
    let color = if hl {
        " color: rgb(0, 255, 0);".to_string()
    } else if sel {
        format!(" color: {};", t.selected_color)
    } else {
        String::new()
    };
    Node::el("div", "mfd-top-tab-navigator-bar-element-outer")
        .class_if("active", sel)
        .style(&format!("margin-right: {}px", if last { 0 } else { -15 }))
        .child(svg(&[("height", &hs), ("width", &tws)], left))
        .child(
            Node::el("span", "mfd-top-tab-navigator-bar-element-label")
                .class_if("active", sel)
                .style(&format!("font-size: {}px;{}", js_num(fs), color))
                .text(title),
        )
        .child(svg(&[("height", &hs), ("width", &tws)], right))
}

pub fn tab_page(visible: bool, container_style: Option<&str>, kids: Vec<Node>) -> Node {
    let st = format!("{}display: {};", container_style.map(|s| format!("{} ", s)).unwrap_or_default(), if visible { "flex" } else { "none" });
    Node::el("div", "mfd-top-tab-navigator-tab-content").style(&st).children(kids)
}

pub fn top_tab_navigator(t: &Tabs, pages: Vec<Node>) -> Node {
    let shown: Vec<(usize, &&str)> = t.titles.iter().enumerate().filter(|(_, s)| !s.is_empty()).collect();
    let small = shown.len() > 2;
    let mut bar = Node::el("div", "mfd-top-tab-navigator-bar").style(&format!("height: {}px", js_num(t.height)));
    for (i, title) in &shown {
        bar = bar.child(tab_element(title, *i == t.selected, *i == t.titles.len() - 1, t.highlighted == Some(*i), t, small));
    }
    if t.additional_right_space > 0.0 {
        bar = bar.child(Node::new("div").style(&format!("width: {}px; border-bottom: 2px solid lightgray;", js_num(t.additional_right_space))));
    }
    Node::el("div", "mfd-top-tab-navigator-container").child(bar).children(pages)
}

pub fn page_container(kids: Vec<Node>) -> Node {
    Node::el("div", "mfd-page-container").children(kids)
}

pub fn text_of(n: &Node) -> String {
    let mut s = String::new();
    for k in &n.kids {
        match k {
            Kid::Text { text } => s.push_str(text),
            Kid::Node(c) => s.push_str(&text_of(c)),
        }
    }
    s
}

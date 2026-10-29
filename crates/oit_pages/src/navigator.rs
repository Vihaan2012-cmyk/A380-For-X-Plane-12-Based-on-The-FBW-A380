use crate::widgets::button;
use mfd_tree::{Node, Shape};
use std::collections::BTreeMap;

fn shape(t: &str, pairs: &[(&str, &str)], depth: usize) -> Shape {
    let mut a: BTreeMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    a.insert("_d".into(), depth.to_string());
    a.insert("_display".into(), "inline".into());
    Shape { t: t.to_string(), a }
}

fn svg_with(attrs: &[(&str, &str)], inline: &str, shapes: Vec<Shape>) -> Node {
    let mut n = Node::new("svg").style(inline);
    n.svgattr = attrs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    n.svgattr.insert("style".into(), inline.to_string());
    n.shapes = shapes;
    n
}

fn folder_icon(visible: bool) -> Node {
    let style = format!("display: {}; margin-right: 10px;", if visible { "inline-block" } else { "none" });
    svg_with(
        &[("width", "40"), ("viewbox", "0 0 60 50"), ("version", "1.1")],
        &style,
        vec![
            shape("g", &[], 0),
            shape(
                "path",
                &[
                    ("d", "m55.76271,48.81356c0,0 2.54237,-1.94915 2.54237,-1.94915c0,0 0,-33.38994 0,-33.47458c0,0.08464 -50.84746,-12.79661 -50.84746,-12.79661c0,0 -4.40678,0.76271 -4.40678,0.76271c0,0 52.71186,47.45763 52.71186,47.45763z"),
                    ("stroke", "#dbdbdb"),
                    ("fill", "#a3aeaf"),
                ],
                1,
            ),
            shape(
                "path",
                &[
                    ("d", "m2.88453,1.43962l52.75989,14.37994l0.08475,33.24294l-53.09039,-12.93785l0.23808,-29.5185l0.00768,-5.16653z"),
                    ("stroke", "#dbdbdb"),
                    ("fill", "#baf4ff"),
                ],
                1,
            ),
            shape(
                "path",
                &[
                    ("d", "m2.6875,14.6875c0,0 -1.9375,2.125 -1.9375,1.9375c0,-0.1875 0,5.37645 0,5.375c0,0.00145 55.5,13.75 55.5,13.75c0,0 2.375,-1.1875 2.375,-1.1875c0,0 -0.0625,-9.8125 -0.0625,-9.8125c0,0 -2.75,1.5 -2.75,1.5c0,0 -53.0625,-12.375 -53.0625,-12.375c0,0 -0.0625,0.8125 -0.0625,0.8125z"),
                    ("stroke", "#dbdbdb"),
                    ("fill", "#afd6e0"),
                ],
                1,
            ),
            shape(
                "ellipse",
                &[("stroke", "#333333"), ("ry", "4.09375"), ("rx", "4.09375"), ("id", "svg_4"), ("cy", "24.03125"), ("cx", "27.21875"), ("fill", "none")],
                1,
            ),
            shape(
                "line",
                &[("y2", "21.56311"), ("x2", "27.375"), ("y1", "18.375"), ("x1", "27.375"), ("stroke", "#333333"), ("fill", "none")],
                1,
            ),
        ],
    )
}

fn file_icon(visible: bool) -> Node {
    let style = format!("display: {}; margin-right: 10px;", if visible { "inline-block" } else { "none" });
    svg_with(
        &[("width", "18"), ("viewbox", "0 0 36 50"), ("version", "1.1")],
        &style,
        vec![
            shape("g", &[], 0),
            shape("path", &[("d", "m1.375,49l0,-47.875l26,0l6.3125,15l0.3125,32.5l-32.625,0.375z"), ("stroke", "#fff"), ("stroke-width", "2"), ("fill", "none")], 1),
            shape("path", &[("d", "m32.4375,10.8125"), ("stroke", "#fff"), ("fill", "none"), ("stroke-width", "2")], 1),
            shape("path", &[("d", "m33.75,16.125l-11,-9.625l4.25,-5.125"), ("stroke", "#fff"), ("fill", "none"), ("stroke-width", "2")], 1),
        ],
    )
}

pub struct FolderOpts {
    pub expanded: bool,
    pub hide_icon: bool,
    pub hide_opener: bool,
    pub hidden: bool,
}

pub fn folder(name: &str, o: &FolderOpts, children: Vec<Node>) -> Node {
    Node::el("div", "oit-avncs-navigator-folder-container")
        .class_if("hidden", o.hidden)
        .child(
            Node::new("div")
                .style("display: flex; flex-direction: row; align-items: center;")
                .child(
                    Node::el("span", "oit-avncs-icon-folder-opener")
                        .class_if("hidden", o.hide_opener)
                        .text(if o.expanded { "-" } else { "+" }),
                )
                .child(folder_icon(!o.hide_icon))
                .child(Node::el("span", "oit-avncs-navigator-folder").class_if("expanded", o.expanded).text(name)),
        )
        .child(
            Node::el("div", "oit-avncs-navigator-folder-children")
                .class_if("hidden", !o.expanded)
                .children(children),
        )
}

pub fn file(name: &str, selected: bool, disabled: bool, with_symbol_node: bool) -> Node {
    let mut n = Node::el("div", "oit-avncs-navigator-file");
    if with_symbol_node {
        n = n.child(file_icon(false));
    }
    n.child(
        Node::el("span", "oit-avncs-navigator-file-name")
            .class_if("selected", selected)
            .class_if("disabled", disabled)
            .text(name),
    )
}

pub fn nav_button(label: &str, disabled: bool, container_style: &str) -> Node {
    button(label, disabled, container_style, "font-size: 28px; height: 40px;")
}

pub fn oit_file(name: &str, selected: bool, disabled: bool, show_symbol: bool) -> Node {
    Node::el("div", "oit-avncs-navigator-file").child(file_icon(show_symbol)).child(
        Node::el("span", "oit-avncs-navigator-file-name")
            .class_if("selected", selected)
            .class_if("disabled", disabled)
            .text(name),
    )
}

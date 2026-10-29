pub mod bg;
pub mod diagrams;
pub mod docs;
pub mod failures;
pub mod fap;
pub mod flight;
pub mod fmt;
pub mod ground;
pub mod home;
pub mod js;
pub mod jv;
pub mod maint;
pub mod map;
pub mod md;
pub mod misc;
pub mod model;
pub mod ops;
pub mod payload;
pub mod perf;
pub mod pushback;
pub mod settings;
pub mod shell;
pub mod study;

pub use mfd_tree::{Kid, Node};
pub use model::EfbModel;

pub fn svg_icon() -> Node {
    Node::new("svg")
}

pub fn label(classes: &str, text: &str) -> Node {
    Node::el("div", classes).text(text)
}

pub fn span(classes: &str, text: &str) -> Node {
    Node::el("span", classes).text(text)
}

pub fn button(classes: &str, text: &str) -> Node {
    Node::el("button", classes).text(text)
}

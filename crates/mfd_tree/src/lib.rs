use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Kid {
    Node(Node),
    Text { text: String },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub t: String,
    #[serde(default)]
    pub a: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cls: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attrs: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub svgattr: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shapes: Vec<Shape>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub data: BTreeMap<String, String>,
    #[serde(default)]
    pub kids: Vec<Kid>,
}

impl Node {
    pub fn new(tag: &str) -> Self {
        Node { tag: tag.to_string(), ..Default::default() }
    }

    pub fn el(tag: &str, classes: &str) -> Self {
        let mut n = Node::new(tag);
        n.cls = classes.split_whitespace().map(str::to_string).collect();
        n
    }

    pub fn id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }

    pub fn class_if(mut self, class: &str, on: bool) -> Self {
        if on {
            self.cls.push(class.to_string());
        }
        self
    }

    pub fn style(mut self, inline: &str) -> Self {
        self.inline = Some(inline.to_string());
        self
    }

    pub fn value(mut self, v: &str) -> Self {
        self.value = Some(v.to_string());
        self
    }

    pub fn attr(mut self, name: &str, v: &str) -> Self {
        self.attrs.insert(name.to_string(), v.to_string());
        self
    }

    pub fn data(mut self, name: &str, v: &str) -> Self {
        self.data.insert(name.to_string(), v.to_string());
        self
    }

    pub fn act(self, v: &str) -> Self {
        self.data("act", v)
    }

    pub fn attr_if(self, name: &str, on: bool) -> Self {
        if on {
            self.attr(name, "")
        } else {
            self
        }
    }

    pub fn text(mut self, t: &str) -> Self {
        if !t.is_empty() {
            match self.kids.last_mut() {
                Some(Kid::Text { text }) => text.push_str(t),
                _ => self.kids.push(Kid::Text { text: t.to_string() }),
            }
        }
        self
    }

    pub fn child(mut self, n: Node) -> Self {
        self.kids.push(Kid::Node(n));
        self
    }

    pub fn children(mut self, ns: impl IntoIterator<Item = Node>) -> Self {
        self.kids.extend(ns.into_iter().map(Kid::Node));
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialises_like_dom2tree() {
        let n = Node::el("div", "mfd-label green").id("x").text("A").text("B").child(Node::new("span"));
        assert_eq!(n.to_json(), r#"{"tag":"div","id":"x","cls":["mfd-label","green"],"kids":[{"text":"AB"},{"tag":"span","kids":[]}]}"#);
    }

    #[test]
    fn attrs_serialise_only_when_set() {
        let n = Node::new("input").attr("type", "radio").attr_if("checked", true).attr_if("disabled", false);
        assert_eq!(n.to_json(), r#"{"tag":"input","attrs":{"checked":"","type":"radio"},"kids":[]}"#);
        let back: Node = serde_json::from_str(&n.to_json()).unwrap();
        assert_eq!(back, n);
        let k: Kid = serde_json::from_str(r#"{"text":"A"}"#).unwrap();
        assert_eq!(k, Kid::Text { text: "A".into() });
    }

    #[test]
    fn data_serialises_only_when_set() {
        let n = Node::el("button", "x").act("tab:home").data("st", "12");
        assert_eq!(n.to_json(), r#"{"tag":"button","cls":["x"],"data":{"act":"tab:home","st":"12"},"kids":[]}"#);
        let back: Node = serde_json::from_str(&n.to_json()).unwrap();
        assert_eq!(back, n);
        let k: Kid = serde_json::from_str(r#"{"text":"A"}"#).unwrap();
        assert_eq!(k, Kid::Text { text: "A".into() });
    }
}

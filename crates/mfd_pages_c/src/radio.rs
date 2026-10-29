use mfd_tree::Node;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadioColor {
    Yellow,
    Green,
    Amber,
    Cyan,
    White,
}

impl RadioColor {
    pub fn class(self) -> &'static str {
        match self {
            RadioColor::Yellow => "yellow",
            RadioColor::Green => "green",
            RadioColor::Amber => "amber",
            RadioColor::Cyan => "cyan",
            RadioColor::White => "white",
        }
    }
}

pub fn radio_button_group(id_prefix: &str, values: &[&str], selected: Option<usize>, color: RadioColor, disabled: &[bool]) -> Node {
    radio_button_group_spaced(id_prefix, values, selected, color, disabled, None)
}

pub fn radio_button_group_spaced(
    id_prefix: &str,
    values: &[&str],
    selected: Option<usize>,
    color: RadioColor,
    disabled: &[bool],
    vertical_spacing: Option<u32>,
) -> Node {
    let mut form = Node::new("form");
    for (i, v) in values.iter().enumerate() {
        let mut input = Node::new("input").id(&format!("{}_{}", id_prefix, i));
        input.attrs.insert("type".into(), "radio".into());
        input.attrs.insert("name".into(), id_prefix.into());
        if selected == Some(i) {
            input.attrs.insert("checked".into(), String::new());
        }
        if disabled.get(i).copied().unwrap_or(false) {
            input.attrs.insert("disabled".into(), String::new());
        }
        let mut label = Node::el("label", &format!("mfd-radio-button {}", color.class())).id(&format!("{}_label_{}", id_prefix, i));
        if let Some(s) = vertical_spacing {
            label = label.style(&format!("margin-top: {}px;", s));
        }
        label.attrs.insert("for".into(), format!("{}_{}", id_prefix, i));
        form = form.child(label.child(input).child(Node::new("span").text(v)));
    }
    form
}

#[cfg(test)]
mod tests {
    use super::*;
    use mfd_tree::Kid;

    #[test]
    fn ids_classes_and_checked() {
        let f = radio_button_group("CAPT_MFD_x", &["A", "B"], Some(1), RadioColor::Amber, &[false, true]);
        assert_eq!(f.kids.len(), 2);
        let Kid::Node(l1) = &f.kids[1] else { panic!() };
        assert_eq!(l1.id.as_deref(), Some("CAPT_MFD_x_label_1"));
        assert_eq!(l1.cls, vec!["mfd-radio-button", "amber"]);
        let Kid::Node(i1) = &l1.kids[0] else { panic!() };
        assert_eq!(i1.id.as_deref(), Some("CAPT_MFD_x_1"));
        assert_eq!(i1.attrs.get("checked").map(String::as_str), Some(""));
        assert_eq!(i1.attrs.get("disabled").map(String::as_str), Some(""));
        let Kid::Node(l0) = &f.kids[0] else { panic!() };
        let Kid::Node(i0) = &l0.kids[0] else { panic!() };
        assert!(!i0.attrs.contains_key("checked"));
    }
}

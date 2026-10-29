use crate::w::*;
use mfd_tree::Node;

pub fn fms_page_not_avail() -> Node {
    page_container(vec![Node::el("div", "mfd-amber-error-message").text("FMS PAGE NOT AVAIL")])
}

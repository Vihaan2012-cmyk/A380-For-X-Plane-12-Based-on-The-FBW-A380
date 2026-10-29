use mfd_tree::{Kid, Node};

use crate::tree::ACT;

/// One element of the published document, in document order (the `<body>` first, an
/// element's SVG shapes right after it, then its child elements), as the display
/// reports its boxes.
#[derive(Clone, Debug, PartialEq)]
pub struct Flat {
    pub parent: Option<usize>,
    pub act: Option<String>,
    pub hidden: bool,
    pub overlay: bool,
}

fn style_hides(n: &Node) -> bool {
    n.inline.as_deref().is_some_and(|s| {
        s.split(';').any(|d| {
            let mut kv = d.splitn(2, ':');
            let k = kv.next().unwrap_or("").trim();
            let v = kv.next().unwrap_or("").trim();
            (k == "display" && v == "none") || (k == "visibility" && v == "hidden")
        })
    })
}

fn push(n: &Node, parent: Option<usize>, hidden: bool, overlay: bool, out: &mut Vec<Flat>) {
    let hidden = hidden || style_hides(n) || n.cls.iter().any(|c| c == "disabled" || c == "invisible");
    let overlay = overlay || n.cls.iter().any(|c| c == "mfd-dropdown-menu" || c == "mfd-context-menu");
    let i = out.len();
    out.push(Flat { parent, act: n.attrs.get(ACT).cloned(), hidden, overlay });
    for k in &n.kids {
        if let Kid::Node(c) = k {
            push(c, Some(i), hidden, overlay, out);
        }
    }
}

pub fn flatten(body: &Node) -> Vec<Flat> {
    let mut out = Vec::new();
    push(body, None, false, false, &mut out);
    out
}

/// The `data-act` of the topmost visible element under (x, y), from the boxes the
/// display laid out (x, y, w, h per element, document order). Open menus paint above
/// the page, so an element inside one wins over later page content.
pub fn hit(flat: &[Flat], boxes: &[[f32; 4]], x: f64, y: f64) -> Option<String> {
    let n = flat.len().min(boxes.len());
    let mut best = None;
    let mut best_overlay = None;
    for i in 0..n {
        let b = boxes[i];
        let (bx, by, bw, bh) = (b[0] as f64, b[1] as f64, b[2] as f64, b[3] as f64);
        if flat[i].hidden || bw <= 0. || bh <= 0. || x < bx || y < by || x >= bx + bw || y >= by + bh {
            continue;
        }
        best = Some(i);
        if flat[i].overlay {
            best_overlay = Some(i);
        }
    }
    let best = best_overlay.or(best);
    let mut cur = best;
    while let Some(i) = cur {
        if let Some(a) = &flat[i].act {
            return Some(a.clone());
        }
        cur = flat[i].parent;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_open_menu_item_wins_over_page_content_laid_out_after_it() {
        let mut menu = Node::el("div", "mfd-dropdown-menu");
        let mut item = Node::el("span", "mfd-dropdown-menu-element");
        item.attrs.insert(ACT.into(), "c|item:x_1".into());
        menu.kids.push(Kid::Node(item));
        let mut field = Node::el("div", "mfd-input-field-root");
        field.attrs.insert(ACT.into(), "c|field:FROM".into());
        let mut body = Node::el("body", "");
        body.kids.push(Kid::Node(menu));
        body.kids.push(Kid::Node(field));
        let flat = flatten(&body);
        let boxes = [[0., 0., 800., 600.], [0., 40., 200., 300.], [0., 80., 200., 40.], [0., 90., 300., 40.]];
        assert_eq!(hit(&flat, &boxes, 50., 100.).as_deref(), Some("c|item:x_1"));
        assert_eq!(hit(&flat, &boxes, 250., 100.).as_deref(), Some("c|field:FROM"));
    }
}

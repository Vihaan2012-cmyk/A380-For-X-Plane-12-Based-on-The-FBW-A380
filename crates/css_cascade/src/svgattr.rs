use crate::dom::Doc;
use crate::shorthand::is_wide;

pub fn in_svg(doc: &Doc, i: usize) -> bool {
    let mut cur = Some(i);
    while let Some(c) = cur {
        if doc.els[c].tag.eq_ignore_ascii_case("svg") {
            return true;
        }
        cur = doc.els[c].parent;
    }
    false
}

pub fn svg_transform(v: &str) -> String {
    let mut out = Vec::new();
    let mut rest = v.trim();
    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim().trim_start_matches(',').trim().to_ascii_lowercase();
        let close = match rest.find(')') {
            Some(c) => c,
            None => break,
        };
        let nums: Vec<String> = rest[open + 1..close].replace(',', " ").split_whitespace().map(str::to_string).collect();
        rest = &rest[close + 1..];
        if nums.is_empty() {
            return "none".into();
        }
        let px = |s: &str| format!("{}px", s);
        match name.as_str() {
            "translate" => out.push(format!("translate({}, {})", px(&nums[0]), px(nums.get(1).map(|s| s.as_str()).unwrap_or("0")))),
            "scale" => out.push(format!("scale({}, {})", nums[0], nums.get(1).unwrap_or(&nums[0]))),
            "rotate" => {
                if nums.len() == 3 {
                    out.push(format!("translate({}, {}) rotate({}deg) translate(-{}, -{})", px(&nums[1]), px(&nums[2]), nums[0], px(&nums[1]), px(&nums[2])));
                } else {
                    out.push(format!("rotate({}deg)", nums[0]));
                }
            }
            "skewx" => out.push(format!("skewX({}deg)", nums[0])),
            "skewy" => out.push(format!("skewY({}deg)", nums[0])),
            "matrix" => out.push(format!("matrix({})", nums.join(", "))),
            _ => return "none".into(),
        }
    }
    if out.is_empty() {
        "none".into()
    } else {
        out.join(" ")
    }
}

pub fn valid(name: &str, v: &str) -> bool {
    let l = v.trim().to_ascii_lowercase();
    if is_wide(&l) {
        return true;
    }
    let set: &[&str] = match name {
        "display" => &[
            "none", "block", "inline", "inline-block", "flex", "inline-flex", "grid", "inline-grid", "contents", "list-item", "table", "inline-table", "table-row", "table-cell", "table-row-group",
            "table-header-group", "table-footer-group", "table-column", "table-column-group", "table-caption", "flow-root", "block flex", "inline flex", "block grid", "inline grid", "block flow-root",
            "inline flow-root", "ruby",
        ],
        "position" => &["static", "relative", "absolute", "fixed", "sticky"],
        "text-align" => &["start", "end", "left", "right", "center", "justify", "match-parent", "-webkit-center", "-webkit-left", "-webkit-right"],
        "white-space" => &["normal", "nowrap", "pre", "pre-wrap", "pre-line", "break-spaces"],
        "text-transform" => &["none", "capitalize", "uppercase", "lowercase", "full-width"],
        "visibility" => &["visible", "hidden", "collapse"],
        "overflow-x" | "overflow-y" => &["visible", "hidden", "scroll", "auto", "clip"],
        "box-sizing" => &["content-box", "border-box"],
        "text-overflow" => &["clip", "ellipsis"],
        "pointer-events" => &["auto", "none", "visiblepainted", "visiblefill", "visiblestroke", "visible", "painted", "fill", "stroke", "all", "bounding-box"],
        "flex-direction" => &["row", "row-reverse", "column", "column-reverse"],
        "flex-wrap" => &["nowrap", "wrap", "wrap-reverse"],
        "font-style" => &["normal", "italic", "oblique"],
        "border-top-style" | "border-right-style" | "border-bottom-style" | "border-left-style" => &["none", "hidden", "dotted", "dashed", "solid", "double", "groove", "ridge", "inset", "outset"],
        "justify-content" | "align-content" => {
            const ALL: &[&str] = &["normal", "stretch", "center", "start", "end", "flex-start", "flex-end", "left", "right", "baseline", "space-between", "space-around", "space-evenly", "safe", "unsafe", "first", "last"];
            return l.split_whitespace().all(|w| ALL.contains(&w));
        }
        "align-items" | "align-self" => {
            const ALL: &[&str] = &["normal", "auto", "stretch", "center", "start", "end", "flex-start", "flex-end", "self-start", "self-end", "baseline", "first", "last", "safe", "unsafe"];
            return l.split_whitespace().all(|w| ALL.contains(&w));
        }
        "vertical-align" => {
            return matches!(l.as_str(), "baseline" | "sub" | "super" | "text-top" | "text-bottom" | "middle" | "top" | "bottom") || l.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '+' || c == '.') || l.starts_with("calc(");
        }
        "font-weight" => {
            return matches!(l.as_str(), "normal" | "bold" | "bolder" | "lighter") || l.parse::<f64>().is_ok();
        }
        _ => return true,
    };
    set.contains(&l.as_str())
}

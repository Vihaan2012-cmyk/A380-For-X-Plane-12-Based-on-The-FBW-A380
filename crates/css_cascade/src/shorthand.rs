use crate::values::{commas, parse_color, tokens, Rgba};

const SIDES: [&str; 4] = ["top", "right", "bottom", "left"];
const CORNERS: [&str; 4] = ["top-left", "top-right", "bottom-right", "bottom-left"];
const WIDE: [&str; 5] = ["inherit", "initial", "unset", "revert", "revert-layer"];

pub fn is_wide(v: &str) -> bool {
    WIDE.contains(&v.trim().to_ascii_lowercase().as_str())
}

fn box4(toks: &[String]) -> Option<[String; 4]> {
    match toks.len() {
        1 => Some([toks[0].clone(), toks[0].clone(), toks[0].clone(), toks[0].clone()]),
        2 => Some([toks[0].clone(), toks[1].clone(), toks[0].clone(), toks[1].clone()]),
        3 => Some([toks[0].clone(), toks[1].clone(), toks[2].clone(), toks[1].clone()]),
        4 => Some([toks[0].clone(), toks[1].clone(), toks[2].clone(), toks[3].clone()]),
        _ => None,
    }
}

fn is_style(t: &str) -> bool {
    matches!(t, "none" | "hidden" | "dotted" | "dashed" | "solid" | "double" | "groove" | "ridge" | "inset" | "outset")
}

fn is_width(t: &str) -> bool {
    matches!(t, "thin" | "medium" | "thick") || t.starts_with(|c: char| c.is_ascii_digit() || c == '.' || c == '+' || c == '-') || t.starts_with("calc(") || t.starts_with("var(")
}

fn wide_all(names: &[String], v: &str) -> Vec<(String, String)> {
    names.iter().map(|n| (n.clone(), v.trim().to_ascii_lowercase())).collect()
}

fn border_side_names(side: &str) -> [String; 3] {
    [format!("border-{}-width", side), format!("border-{}-style", side), format!("border-{}-color", side)]
}

fn parse_border(value: &str) -> (String, String, String) {
    let mut w = "initial".to_string();
    let mut st = "initial".to_string();
    let mut c = "initial".to_string();
    for t in tokens(value) {
        let l = t.to_ascii_lowercase();
        if is_style(&l) {
            st = l;
        } else if is_width(&l) && parse_color(&l, Rgba { r: 0, g: 0, b: 0, a: 1.0 }).is_none() {
            w = l;
        } else {
            c = t;
        }
    }
    (w, st, c)
}

fn logical(name: &str, v: &str) -> Option<Vec<(String, String)>> {
    let phys = |s: &str| -> Option<&'static str> {
        Some(match s {
            "inline-size" => "width",
            "block-size" => "height",
            "min-inline-size" => "min-width",
            "min-block-size" => "min-height",
            "max-inline-size" => "max-width",
            "max-block-size" => "max-height",
            _ => return None,
        })
    };
    if let Some(p) = phys(name) {
        return Some(vec![(p.to_string(), v.to_string())]);
    }
    for base in ["margin", "padding", "inset"] {
        let rest = match name.strip_prefix(base) {
            Some(r) if r.starts_with("-block") || r.starts_with("-inline") => r,
            _ => continue,
        };
        let name_of = |side: &str| if base == "inset" { side.to_string() } else { format!("{}-{}", base, side) };
        let (a, b) = if rest.starts_with("-block") { ("top", "bottom") } else { ("left", "right") };
        let suffix = rest.trim_start_matches("-block").trim_start_matches("-inline");
        if is_wide(v) {
            return Some(vec![(name_of(a), v.to_string()), (name_of(b), v.to_string())]);
        }
        return match suffix {
            "" => {
                let t = tokens(v);
                let x = t.first()?.clone();
                let y = t.get(1).cloned().unwrap_or_else(|| x.clone());
                Some(vec![(name_of(a), x), (name_of(b), y)])
            }
            "-start" => Some(vec![(name_of(a), v.to_string())]),
            "-end" => Some(vec![(name_of(b), v.to_string())]),
            _ => None,
        };
    }
    None
}

pub fn expand(name: &str, value: &str) -> Option<Vec<(String, String)>> {
    let v = value.trim();
    if let Some(r) = logical(name, v) {
        return Some(r);
    }
    let all = |list: Vec<String>| -> Option<Vec<(String, String)>> { Some(wide_all(&list, v)) };
    match name {
        "margin" | "padding" | "inset" => {
            let names: Vec<String> = SIDES.iter().map(|s| if name == "inset" { s.to_string() } else { format!("{}-{}", name, s) }).collect();
            if is_wide(v) {
                return all(names);
            }
            let b = box4(&tokens(v))?;
            Some(names.into_iter().zip(b).collect())
        }
        "border-width" | "border-style" | "border-color" => {
            let kind = &name[7..];
            let names: Vec<String> = SIDES.iter().map(|s| format!("border-{}-{}", s, kind)).collect();
            if is_wide(v) {
                return all(names);
            }
            let b = box4(&tokens(v))?;
            Some(names.into_iter().zip(b).collect())
        }
        "border" => {
            let mut names = Vec::new();
            for s in SIDES {
                names.extend(border_side_names(s));
            }
            if is_wide(v) {
                return all(names);
            }
            let (w, st, c) = parse_border(v);
            let mut out = Vec::new();
            for s in SIDES {
                let n = border_side_names(s);
                out.push((n[0].clone(), w.clone()));
                out.push((n[1].clone(), st.clone()));
                out.push((n[2].clone(), c.clone()));
            }
            Some(out)
        }
        "border-top" | "border-right" | "border-bottom" | "border-left" => {
            let n = border_side_names(&name[7..]);
            if is_wide(v) {
                return all(n.to_vec());
            }
            let (w, st, c) = parse_border(v);
            Some(vec![(n[0].clone(), w), (n[1].clone(), st), (n[2].clone(), c)])
        }
        "border-radius" => {
            let names: Vec<String> = CORNERS.iter().map(|c| format!("border-{}-radius", c)).collect();
            if is_wide(v) {
                return all(names);
            }
            let mut parts = v.split('/');
            let h = box4(&tokens(parts.next()?))?;
            let vv = match parts.next() {
                Some(p) => box4(&tokens(p))?,
                None => h.clone(),
            };
            Some(names.into_iter().enumerate().map(|(i, n)| (n, if h[i] == vv[i] { h[i].clone() } else { format!("{} {}", h[i], vv[i]) })).collect())
        }
        "background" => {
            let names: Vec<String> = ["background-color", "background-image", "background-size", "background-position", "background-repeat"].iter().map(|s| s.to_string()).collect();
            if is_wide(v) {
                return all(names);
            }
            let layers = commas(v);
            let first = layers.first()?.clone();
            let last = layers.last()?.clone();
            let mut color = "initial".to_string();
            for t in tokens(&last) {
                let l = t.to_ascii_lowercase();
                if parse_color(&l, Rgba { r: 0, g: 0, b: 0, a: 1.0 }).is_some() {
                    color = t;
                }
            }
            let images: Vec<String> = layers
                .iter()
                .map(|l| tokens(l).into_iter().find(|t| t.contains("url(") || t.contains("gradient(") || t.eq_ignore_ascii_case("none")).unwrap_or_else(|| "none".into()))
                .collect();
            let mut size = "initial".to_string();
            let ft = tokens(&first);
            if let Some(p) = ft.iter().position(|t| t == "/") {
                let rest: Vec<String> = ft[p + 1..].iter().take_while(|t| !is_style(t) && !matches!(t.as_str(), "repeat" | "no-repeat" | "repeat-x" | "repeat-y")).take(2).cloned().collect();
                if !rest.is_empty() {
                    size = rest.join(" ");
                }
            }
            Some(vec![
                ("background-color".into(), color),
                ("background-image".into(), images.join(", ")),
                ("background-size".into(), size),
                ("background-position".into(), "initial".into()),
                ("background-repeat".into(), "initial".into()),
            ])
        }
        "flex" => {
            let names: Vec<String> = ["flex-grow", "flex-shrink", "flex-basis"].iter().map(|s| s.to_string()).collect();
            if is_wide(v) {
                return all(names);
            }
            let t = tokens(&v.to_ascii_lowercase());
            let (mut g, mut s, mut b) = ("1".to_string(), "1".to_string(), "0%".to_string());
            match t.as_slice() {
                [x] if x == "none" => {
                    g = "0".into();
                    s = "0".into();
                    b = "auto".into();
                }
                [x] if x == "auto" => {
                    b = "auto".into();
                }
                _ => {
                    let mut nums = Vec::new();
                    for x in &t {
                        if x.parse::<f64>().is_ok() {
                            nums.push(x.clone());
                        } else {
                            b = x.clone();
                        }
                    }
                    if let Some(n) = nums.first() {
                        g = n.clone();
                    }
                    if let Some(n) = nums.get(1) {
                        s = n.clone();
                    }
                    if t.len() == 3 && nums.len() == 3 {
                        b = nums[2].clone();
                    }
                    if t.len() == 1 && nums.len() == 1 {
                        b = "0%".into();
                        s = "1".into();
                    }
                }
            }
            Some(vec![("flex-grow".into(), g), ("flex-shrink".into(), s), ("flex-basis".into(), b)])
        }
        "flex-flow" => {
            if is_wide(v) {
                return all(vec!["flex-direction".into(), "flex-wrap".into()]);
            }
            let mut d = "initial".to_string();
            let mut w = "initial".to_string();
            for t in tokens(&v.to_ascii_lowercase()) {
                if t.contains("wrap") {
                    w = t;
                } else {
                    d = t;
                }
            }
            Some(vec![("flex-direction".into(), d), ("flex-wrap".into(), w)])
        }
        "gap" | "grid-gap" => {
            if is_wide(v) {
                return all(vec!["row-gap".into(), "column-gap".into()]);
            }
            let t = tokens(v);
            let r = t.first()?.clone();
            let c = t.get(1).cloned().unwrap_or_else(|| r.clone());
            Some(vec![("row-gap".into(), r), ("column-gap".into(), c)])
        }
        "grid-row-gap" => Some(vec![("row-gap".into(), v.into())]),
        "grid-column-gap" => Some(vec![("column-gap".into(), v.into())]),
        "overflow" => {
            if is_wide(v) {
                return all(vec!["overflow-x".into(), "overflow-y".into()]);
            }
            let t = tokens(&v.to_ascii_lowercase());
            let x = t.first()?.clone();
            let y = t.get(1).cloned().unwrap_or_else(|| x.clone());
            Some(vec![("overflow-x".into(), x), ("overflow-y".into(), y)])
        }
        "text-decoration" => {
            if is_wide(v) {
                return all(vec!["text-decoration-line".into()]);
            }
            let lines: Vec<String> = tokens(&v.to_ascii_lowercase()).into_iter().filter(|t| matches!(t.as_str(), "none" | "underline" | "overline" | "line-through" | "blink")).collect();
            Some(vec![("text-decoration-line".into(), if lines.is_empty() { "none".into() } else { lines.join(" ") })])
        }
        "place-items" => Some(vec![("align-items".into(), tokens(v).first()?.clone())]),
        "place-self" => Some(vec![("align-self".into(), tokens(v).first()?.clone())]),
        "place-content" => {
            let t = tokens(v);
            Some(vec![("align-content".into(), t.first()?.clone()), ("justify-content".into(), t.get(1).unwrap_or(&t[0]).clone())])
        }
        "font" => {
            let names: Vec<String> = ["font-style", "font-weight", "font-size", "line-height", "font-family"].iter().map(|s| s.to_string()).collect();
            if is_wide(v) {
                return all(names);
            }
            let t = tokens(v);
            let mut style = "normal".to_string();
            let mut weight = "normal".to_string();
            let mut i = 0;
            while i < t.len() {
                let l = t[i].to_ascii_lowercase();
                if matches!(l.as_str(), "italic" | "oblique") {
                    style = l;
                } else if matches!(l.as_str(), "bold" | "bolder" | "lighter") || l.parse::<f64>().map(|x| x >= 1.0 && x <= 1000.0).unwrap_or(false) {
                    weight = l;
                } else if matches!(l.as_str(), "normal" | "small-caps") || l.contains("condensed") || l.contains("expanded") {
                } else {
                    break;
                }
                i += 1;
            }
            let sz = t.get(i)?.clone();
            let (size, lh) = match sz.find('/') {
                Some(p) => (sz[..p].to_string(), sz[p + 1..].to_string()),
                None => (sz, "normal".to_string()),
            };
            let family = t[i + 1..].join(" ");
            Some(vec![("font-style".into(), style), ("font-weight".into(), weight), ("font-size".into(), size), ("line-height".into(), lh), ("font-family".into(), family)])
        }
        "outline" | "list-style" | "transition" | "animation" | "grid-area" | "grid" | "grid-template" | "columns" | "column-rule" | "mask" | "-webkit-mask" | "scroll-margin" | "scroll-padding" | "will-change" | "touch-action" | "all" | "container" | "place-items-" => Some(vec![]),
        _ => None,
    }
}

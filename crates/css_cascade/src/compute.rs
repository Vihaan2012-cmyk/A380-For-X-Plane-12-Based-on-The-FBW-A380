use crate::css::{parse_decls, parse_sheet, Decl, Rule, Viewport};
use crate::dom::Doc;
use crate::selector::matches;
use crate::shorthand::{expand, is_wide};
use crate::svgattr::{in_svg, svg_transform, valid};
use crate::values::*;
use std::collections::HashMap;
use std::rc::Rc;

pub const PROPS: &[&str] = &[
    "display", "position", "top", "left", "right", "bottom", "width", "height", "min-width", "min-height", "max-width", "max-height",
    "margin-top", "margin-right", "margin-bottom", "margin-left", "padding-top", "padding-right", "padding-bottom", "padding-left",
    "border-top-width", "border-right-width", "border-bottom-width", "border-left-width",
    "border-top-style", "border-right-style", "border-bottom-style", "border-left-style",
    "border-top-color", "border-left-color", "border-right-color", "border-bottom-color",
    "border-top-left-radius", "border-top-right-radius", "border-bottom-right-radius", "border-bottom-left-radius",
    "box-sizing", "flex-direction", "flex-wrap", "justify-content", "align-items", "align-self", "align-content",
    "flex-grow", "flex-shrink", "flex-basis", "gap", "row-gap", "column-gap", "grid-template-columns", "grid-template-rows",
    "grid-column", "grid-row", "order", "font-family", "font-size", "font-weight", "font-style", "line-height", "letter-spacing",
    "text-align", "vertical-align", "white-space", "text-transform", "text-decoration-line", "color", "background-color",
    "background-image", "background-size", "box-shadow", "opacity", "visibility", "overflow", "transform", "z-index",
    "filter", "backdrop-filter", "text-overflow", "background-position", "pointer-events", "transform-origin",
    "aspect-ratio",
];

/// SVG paint properties a stylesheet or inline style can set on a shape; the
/// shape's own presentation attributes and inheritance stay with the painter.
const SVG_PAINT: &[&str] = &["fill", "fill-opacity", "stroke", "stroke-width", "stroke-opacity", "stroke-dasharray", "stroke-linecap", "stroke-linejoin"];

const INHERITED: &[&str] = &[
    "color", "font-family", "font-size", "font-weight", "font-style", "line-height", "letter-spacing", "text-align", "text-transform", "white-space", "visibility", "pointer-events",
];

fn pidx(name: &str) -> usize {
    PROPS.iter().position(|p| *p == name).unwrap_or(usize::MAX)
}

fn is_inherited(name: &str) -> bool {
    INHERITED.contains(&name)
}

fn initial_raw(name: &str) -> &'static str {
    match name {
        "display" => "inline",
        "position" => "static",
        "top" | "left" | "right" | "bottom" | "width" | "height" | "min-width" | "min-height" | "align-self" | "flex-basis" | "grid-column" | "grid-row" | "z-index" | "background-size" | "aspect-ratio" => "auto",
        "max-width" | "max-height" | "grid-template-columns" | "grid-template-rows" | "transform" | "box-shadow" | "background-image" | "text-decoration-line" | "text-transform" | "filter" | "backdrop-filter" => "none",
        "border-top-width" | "border-right-width" | "border-bottom-width" | "border-left-width" => "medium",
        "border-top-style" | "border-right-style" | "border-bottom-style" | "border-left-style" => "none",
        "border-top-color" | "border-left-color" | "border-right-color" | "border-bottom-color" => "currentcolor",
        "margin-top" | "margin-right" | "margin-bottom" | "margin-left" | "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => "0px",
        "border-top-left-radius" | "border-top-right-radius" | "border-bottom-right-radius" | "border-bottom-left-radius" => "0px",
        "box-sizing" => "content-box",
        "flex-direction" => "row",
        "flex-wrap" => "nowrap",
        "justify-content" | "align-items" | "align-content" | "gap" | "row-gap" | "column-gap" | "letter-spacing" | "line-height" => "normal",
        "flex-grow" | "order" => "0",
        "flex-shrink" => "1",
        "font-family" => "\"Times New Roman\"",
        "font-size" => "medium",
        "font-weight" => "normal",
        "font-style" => "normal",
        "text-align" => "start",
        "vertical-align" => "baseline",
        "white-space" => "normal",
        "color" => "rgb(0, 0, 0)",
        "background-color" => "transparent",
        "opacity" => "1",
        "visibility" => "visible",
        "overflow" => "visible",
        "text-overflow" => "clip",
        "background-position" => "0% 0%",
        "pointer-events" => "auto",
        "transform-origin" => "50% 50%",
        _ => "initial",
    }
}

#[derive(Clone, Debug)]
pub struct Style {
    pub vals: Vec<String>,
    pub custom: Rc<HashMap<String, String>>,
    pub fs: f64,
    pub lh_num: Option<f64>,
    pub hidden: bool,
    pub paint: Vec<(&'static str, String)>,
}

impl Style {
    pub fn get(&self, name: &str) -> Option<&str> {
        let i = pidx(name);
        if i == usize::MAX {
            None
        } else {
            Some(self.vals[i].as_str())
        }
    }

    pub fn to_data_cs(&self) -> String {
        let mut out = String::new();
        for (i, p) in PROPS.iter().enumerate() {
            if !out.is_empty() {
                out.push(';');
            }
            out.push_str(p);
            out.push(':');
            out.push_str(&self.vals[i]);
        }
        for (p, v) in &self.paint {
            out.push(';');
            out.push_str(p);
            out.push(':');
            out.push_str(v);
        }
        out
    }

    pub fn to_data_cs_attr(&self) -> String {
        self.to_data_cs().replace('&', "&amp;").replace('"', "&quot;")
    }
}

pub struct Cascade {
    rules: Vec<Rule>,
    by_id: HashMap<String, Vec<usize>>,
    by_class: HashMap<String, Vec<usize>>,
    by_tag: HashMap<String, Vec<usize>>,
    universal: Vec<usize>,
    vp: Viewport,
    order: u32,
    pub dpr: f64,
}

struct Matched<'a> {
    key: (bool, u8, u32, u32),
    decl: &'a Decl,
}

impl Cascade {
    pub fn new(vw: f64, vh: f64) -> Self {
        Cascade { rules: Vec::new(), by_id: HashMap::new(), by_class: HashMap::new(), by_tag: HashMap::new(), universal: Vec::new(), vp: Viewport { w: vw, h: vh }, order: 0, dpr: 1.0 }
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    pub fn add_sheet(&mut self, text: &str, origin: u8) {
        let mut out = Vec::new();
        parse_sheet(text, origin, &self.vp, &mut self.order, &mut out);
        self.rules.extend(out);
        self.reindex();
    }

    fn reindex(&mut self) {
        self.by_id.clear();
        self.by_class.clear();
        self.by_tag.clear();
        self.universal.clear();
        for (i, r) in self.rules.iter().enumerate() {
            let c = r.sel.compounds.last().unwrap();
            if let Some(id) = &c.id {
                self.by_id.entry(id.clone()).or_default().push(i);
            } else if let Some(cl) = c.classes.first() {
                self.by_class.entry(cl.clone()).or_default().push(i);
            } else if let Some(t) = &c.tag {
                self.by_tag.entry(t.clone()).or_default().push(i);
            } else {
                self.universal.push(i);
            }
        }
    }

    pub fn compute(&self, doc: &Doc) -> Vec<Style> {
        let mut out: Vec<Style> = Vec::with_capacity(doc.els.len());
        let mut root_fs = 16.0;
        for i in 0..doc.els.len() {
            let parent = doc.els[i].parent.map(|p| &out[p]);
            let st = self.compute_el(doc, i, parent, root_fs);
            if doc.els[i].parent.is_none() {
                root_fs = st.fs;
            }
            out.push(st);
        }
        out
    }

    fn compute_el(&self, doc: &Doc, i: usize, parent: Option<&Style>, root_fs: f64) -> Style {
        let el = &doc.els[i];
        let mut cand: Vec<usize> = self.universal.clone();
        if let Some(id) = &el.id {
            if let Some(v) = self.by_id.get(id) {
                cand.extend(v);
            }
        }
        for c in &el.classes {
            if let Some(v) = self.by_class.get(c) {
                cand.extend(v);
            }
        }
        if let Some(v) = self.by_tag.get(&el.tag.to_ascii_lowercase()) {
            cand.extend(v);
        }
        cand.sort_unstable();
        cand.dedup();
        let inline_decls: Vec<Decl> = el.inline.as_deref().map(parse_decls).unwrap_or_default();
        let mut pres: Vec<Decl> = Vec::new();
        if in_svg(doc, i) {
            for (n, v) in &el.attrs {
                let n = n.to_ascii_lowercase();
                match n.as_str() {
                    "transform" => pres.push(Decl { name: "transform".into(), value: svg_transform(v), important: false }),
                    "display" | "visibility" | "opacity" | "font-family" | "font-size" | "font-weight" | "font-style" | "overflow" | "letter-spacing" | "color" => pres.push(Decl { name: n.clone(), value: v.clone(), important: false }),
                    _ => {}
                }
            }
        }
        let mut ms: Vec<Matched> = Vec::new();
        for d in &pres {
            ms.push(Matched { key: (false, 0, 0, 0), decl: d });
        }
        for ci in cand {
            let r = &self.rules[ci];
            if matches(doc, i, &r.sel) {
                for d in r.decls.iter() {
                    ms.push(Matched { key: (d.important, r.origin, r.sel.spec, r.order), decl: d });
                }
            }
        }
        for d in &inline_decls {
            ms.push(Matched { key: (d.important, 1, u32::MAX, 0), decl: d });
        }
        ms.sort_by_key(|m| m.key);

        let mut own: Vec<(String, String)> = Vec::new();
        for m in &ms {
            if m.decl.name.starts_with("--") {
                own.push((m.decl.name.clone(), m.decl.value.clone()));
            }
        }
        let custom: Rc<HashMap<String, String>> = if own.is_empty() {
            parent.map(|p| p.custom.clone()).unwrap_or_default()
        } else {
            let mut merged: HashMap<String, String> = parent.map(|p| (*p.custom).clone()).unwrap_or_default();
            let mut keys = Vec::new();
            for (k, v) in own {
                let lv = v.trim().to_ascii_lowercase();
                if lv == "inherit" {
                    continue;
                }
                if lv == "initial" || lv == "unset" {
                    merged.remove(&k);
                    continue;
                }
                keys.push(k.clone());
                merged.insert(k, v);
            }
            for _ in 0..4 {
                let mut changed = false;
                for k in &keys {
                    let v = match merged.get(k) {
                        Some(v) if v.contains("var(") => v.clone(),
                        _ => continue,
                    };
                    match subst_vars(&v, &merged, 0) {
                        Some(r) => {
                            merged.insert(k.clone(), r);
                            changed = true;
                        }
                        None => {
                            merged.remove(k);
                        }
                    }
                }
                if !changed {
                    break;
                }
            }
            Rc::new(merged)
        };

        let mut spec: HashMap<String, String> = HashMap::new();
        for m in &ms {
            let d = m.decl;
            if d.name.starts_with("--") {
                continue;
            }
            let mut val = d.value.clone();
            if val.to_ascii_lowercase().contains("var(") {
                match subst_vars(&val, &custom, 0) {
                    Some(s) => val = s,
                    None => val = "unset".into(),
                }
            }
            match expand(&d.name, &val) {
                Some(list) => {
                    for (k, v) in list {
                        if valid(&k, &v) {
                            spec.insert(k, v);
                        }
                    }
                }
                None => {
                    let name = if d.name.eq_ignore_ascii_case("-webkit-backdrop-filter") { "backdrop-filter".to_string() } else { d.name.clone() };
                    if valid(&name, &val) {
                        spec.insert(name, val);
                    }
                }
            }
        }
        let mut st = self.finish(doc, i, &spec, parent, custom, root_fs);
        if in_svg(doc, i) {
            st.paint = SVG_PAINT.iter().filter_map(|p| spec.get(*p).map(|v| (*p, v.trim().to_string()))).filter(|(_, v)| !is_wide(&v.to_ascii_lowercase())).collect();
        }
        st
    }

    fn finish(&self, doc: &Doc, i: usize, spec: &HashMap<String, String>, parent: Option<&Style>, custom: Rc<HashMap<String, String>>, root_fs: f64) -> Style {
        let n = PROPS.len();
        let mut vals: Vec<String> = vec![String::new(); n];
        let mut fs = parent.map(|p| p.fs).unwrap_or(16.0);
        let mut lh_num: Option<f64> = None;

        enum Pick {
            Final(String),
            Raw(String),
        }

        let pick = |name: &str| -> Pick {
            let inh = is_inherited(name);
            let from_parent = || -> Pick {
                match parent {
                    Some(p) => Pick::Final(p.vals[pidx(name)].clone()),
                    None => Pick::Raw(initial_raw(name).to_string()),
                }
            };
            match spec.get(name) {
                Some(v) => {
                    let l = v.trim().to_ascii_lowercase();
                    match l.as_str() {
                        "inherit" => from_parent(),
                        "initial" => Pick::Raw(initial_raw(name).to_string()),
                        "unset" | "revert" | "revert-layer" => {
                            if inh {
                                from_parent()
                            } else {
                                Pick::Raw(initial_raw(name).to_string())
                            }
                        }
                        _ => Pick::Raw(v.trim().to_string()),
                    }
                }
                None => {
                    if inh {
                        from_parent()
                    } else {
                        Pick::Raw(initial_raw(name).to_string())
                    }
                }
            }
        };

        let ctx0 = |fs: f64| Ctx { fs, root_fs, vw: self.vp.w, vh: self.vp.h };

        // font-size
        {
            let pfs = parent.map(|p| p.fs).unwrap_or(16.0);
            let sz = match pick("font-size") {
                Pick::Final(_) => pfs,
                Pick::Raw(r) => {
                    let l = r.to_ascii_lowercase();
                    match l.as_str() {
                        "xx-small" => 9.0,
                        "x-small" => 10.0,
                        "small" => 13.0,
                        "medium" => 16.0,
                        "large" => 18.0,
                        "x-large" => 24.0,
                        "xx-large" => 32.0,
                        "xxx-large" => 48.0,
                        "smaller" => pfs / 1.2,
                        "larger" => pfs * 1.2,
                        _ => {
                            if let Some(p) = l.strip_suffix('%') {
                                p.trim().parse::<f64>().map(|x| x / 100.0 * pfs).unwrap_or(pfs)
                            } else {
                                parse_length(&l, &ctx0(pfs)).unwrap_or(pfs)
                            }
                        }
                    }
                }
            };
            fs = sz.max(0.0);
            vals[pidx("font-size")] = fmt_px(fs);
        }
        let cx = ctx0(fs);

        // color
        {
            let pc = parent.map(|p| p.vals[pidx("color")].clone()).unwrap_or_else(|| "rgb(0, 0, 0)".into());
            let pcol = parse_color(&pc, Rgba { r: 0, g: 0, b: 0, a: 1.0 }).unwrap();
            let v = match pick("color") {
                Pick::Final(s) => s,
                Pick::Raw(r) => match parse_color(&r, pcol) {
                    Some(c) => fmt_color(c),
                    None => pc,
                },
            };
            vals[pidx("color")] = v;
        }
        let cur = parse_color(&vals[pidx("color")], Rgba { r: 0, g: 0, b: 0, a: 1.0 }).unwrap();

        let set = |vals: &mut Vec<String>, name: &str, v: String| {
            vals[pidx(name)] = v;
        };

        // font-weight
        {
            let pw: f64 = parent.map(|p| p.vals[pidx("font-weight")].parse().unwrap_or(400.0)).unwrap_or(400.0);
            let v = match pick("font-weight") {
                Pick::Final(s) => s,
                Pick::Raw(r) => {
                    let l = r.to_ascii_lowercase();
                    let w = match l.as_str() {
                        "normal" => 400.0,
                        "bold" => 700.0,
                        "bolder" => {
                            if pw < 400.0 {
                                400.0
                            } else if pw < 600.0 {
                                700.0
                            } else {
                                900.0
                            }
                        }
                        "lighter" => {
                            if pw < 600.0 {
                                100.0
                            } else if pw < 800.0 {
                                400.0
                            } else {
                                700.0
                            }
                        }
                        x => x.parse::<f64>().unwrap_or(pw).clamp(1.0, 1000.0),
                    };
                    fmt_num(w)
                }
            };
            set(&mut vals, "font-weight", v);
        }

        // font-family, font-style
        {
            let v = match pick("font-family") {
                Pick::Final(s) => s,
                Pick::Raw(r) => norm_family(&r),
            };
            set(&mut vals, "font-family", v);
            let v = match pick("font-style") {
                Pick::Final(s) => s,
                Pick::Raw(r) => r.to_ascii_lowercase(),
            };
            set(&mut vals, "font-style", v);
        }

        // line-height
        {
            let v = match pick("line-height") {
                Pick::Final(s) => match parent.and_then(|p| p.lh_num) {
                    Some(nm) => {
                        lh_num = Some(nm);
                        fmt_px(nm * fs)
                    }
                    None => s,
                },
                Pick::Raw(r) => {
                    let l = r.to_ascii_lowercase();
                    if l == "normal" {
                        "normal".to_string()
                    } else if let Ok(nm) = l.parse::<f64>() {
                        lh_num = Some(nm);
                        fmt_px(nm * fs)
                    } else if let Some(p) = l.strip_suffix('%') {
                        p.trim().parse::<f64>().map(|x| fmt_px(x / 100.0 * fs)).unwrap_or_else(|_| "normal".into())
                    } else {
                        parse_length(&l, &cx).map(fmt_px).unwrap_or_else(|| "normal".into())
                    }
                }
            };
            set(&mut vals, "line-height", v);
        }

        // letter-spacing
        {
            let v = match pick("letter-spacing") {
                Pick::Final(s) => s,
                Pick::Raw(r) => {
                    let l = r.to_ascii_lowercase();
                    if l == "normal" {
                        l
                    } else {
                        parse_length(&l, &cx).map(fmt_px).unwrap_or_else(|| "normal".into())
                    }
                }
            };
            set(&mut vals, "letter-spacing", v);
        }

        for name in ["text-align", "white-space", "text-transform", "visibility", "pointer-events"] {
            let v = match pick(name) {
                Pick::Final(s) => s,
                Pick::Raw(r) => r.to_ascii_lowercase(),
            };
            set(&mut vals, name, v);
        }

        let parent_display = parent.map(|p| p.vals[pidx("display")].clone()).unwrap_or_default();
        let flexish_parent = matches!(parent_display.as_str(), "flex" | "inline-flex" | "grid" | "inline-grid");

        let rawv = |name: &str| -> String {
            match pick(name) {
                Pick::Final(s) | Pick::Raw(s) => s,
            }
        };

        let lenpct = |r: &str, kw: &[&str]| -> String {
            let l = r.trim().to_ascii_lowercase();
            if kw.contains(&l.as_str()) {
                return l;
            }
            if let Some(p) = l.strip_suffix('%') {
                if let Ok(x) = p.trim().parse::<f64>() {
                    return format!("{}%", fmt_num(x));
                }
            }
            if let Some(px) = parse_length(&l, &cx) {
                return fmt_px(px);
            }
            if l.contains("fit-content") || l.contains("calc(") || l.contains("min(") || l.contains("max(") || l.contains("clamp(") {
                return l;
            }
            kw.first().map(|s| s.to_string()).unwrap_or(l)
        };

        // styles first (border widths depend on them)
        for side in ["top", "right", "bottom", "left"] {
            let nm = format!("border-{}-style", side);
            let v = rawv(&nm).to_ascii_lowercase();
            set(&mut vals, &nm, v);
        }

        for &name in PROPS {
            let idx = pidx(name);
            if !vals[idx].is_empty() {
                continue;
            }
            let v: String = match name {
                "display" => {
                    let r = rawv(name).to_ascii_lowercase();
                    let t = tokens(&r);
                    if t.len() == 2 {
                        match (t[0].as_str(), t[1].as_str()) {
                            ("block", "flex") => "flex".into(),
                            ("inline", "flex") => "inline-flex".into(),
                            ("block", "grid") => "grid".into(),
                            ("inline", "grid") => "inline-grid".into(),
                            ("block", "flow-root") => "flow-root".into(),
                            (a, _) => a.into(),
                        }
                    } else {
                        r
                    }
                }
                "position" | "box-sizing" | "flex-direction" | "flex-wrap" | "justify-content" | "align-items" | "align-self" | "align-content" | "text-decoration-line" | "background-image" | "background-size" | "box-shadow" => {
                    let r = rawv(name);
                    let l = r.split_whitespace().collect::<Vec<_>>().join(" ");
                    if name == "background-image" || name == "box-shadow" || name == "background-size" {
                        l
                    } else {
                        l.to_ascii_lowercase()
                    }
                }
                "top" | "left" | "right" | "bottom" | "width" | "height" => lenpct(&rawv(name), &["auto"]),
                "min-width" | "min-height" => {
                    let r = lenpct(&rawv(name), &["auto"]);
                    let oof = matches!(vals[pidx("position")].as_str(), "absolute" | "fixed");
                    let hidden = vals[pidx("display")] == "none" || parent.map(|p| p.hidden).unwrap_or(false);
                    if r == "auto" && (!flexish_parent || oof || hidden) {
                        "0px".to_string()
                    } else {
                        r
                    }
                }
                "max-width" | "max-height" => lenpct(&rawv(name), &["none"]),
                "margin-top" | "margin-right" | "margin-bottom" | "margin-left" => lenpct(&rawv(name), &["0px", "auto"]),
                "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => lenpct(&rawv(name), &["0px"]),
                "border-top-width" | "border-right-width" | "border-bottom-width" | "border-left-width" => {
                    let side = &name[7..name.len() - 6];
                    let st = vals[pidx(&format!("border-{}-style", side))].clone();
                    if st == "none" || st == "hidden" {
                        "0px".to_string()
                    } else {
                        let r = rawv(name).to_ascii_lowercase();
                        let w = match r.as_str() {
                            "thin" => 1.0,
                            "medium" => 3.0,
                            "thick" => 5.0,
                            _ => parse_length(&r, &cx).unwrap_or(3.0),
                        };
                        let w = if w > 0.0 { ((w * self.dpr).floor().max(1.0)) / self.dpr } else { 0.0 };
                        fmt_px(w)
                    }
                }
                "border-top-color" | "border-left-color" | "border-right-color" | "border-bottom-color" | "background-color" => {
                    let r = rawv(name);
                    if name == "background-color" && r.to_ascii_lowercase() == "transparent" {
                        "rgba(0, 0, 0, 0)".to_string()
                    } else {
                        match parse_color(&r, cur) {
                            Some(c) => fmt_color(c),
                            None => {
                                if name == "background-color" {
                                    "rgba(0, 0, 0, 0)".to_string()
                                } else {
                                    fmt_color(cur)
                                }
                            }
                        }
                    }
                }
                "border-top-left-radius" | "border-top-right-radius" | "border-bottom-right-radius" | "border-bottom-left-radius" => {
                    let r = rawv(name);
                    let parts: Vec<String> = tokens(&r)
                        .iter()
                        .take(2)
                        .map(|t| {
                            let v = lenpct(t, &["0px"]);
                            if v.starts_with('-') {
                                "0px".to_string()
                            } else {
                                v
                            }
                        })
                        .collect();
                    if parts.len() == 2 && parts[0] != parts[1] {
                        parts.join(" ")
                    } else {
                        parts.first().cloned().unwrap_or_else(|| "0px".into())
                    }
                }
                "flex-grow" | "flex-shrink" | "order" => {
                    let r = rawv(name);
                    match r.trim().parse::<f64>() {
                        Ok(x) => fmt_num(if name == "order" { x.trunc() } else { x.max(0.0) }),
                        Err(_) => initial_raw(name).to_string(),
                    }
                }
                "flex-basis" => lenpct(&rawv(name), &["auto", "content"]),
                "row-gap" | "column-gap" => lenpct(&rawv(name), &["normal"]),
                "gap" => {
                    let r = lenpct(&rawv("row-gap"), &["normal"]);
                    let c = lenpct(&rawv("column-gap"), &["normal"]);
                    if r == c {
                        r
                    } else {
                        format!("{} {}", r, c)
                    }
                }
                "grid-template-columns" | "grid-template-rows" => {
                    let r = rawv(name);
                    let l = r.to_ascii_lowercase();
                    if l == "none" {
                        l
                    } else {
                        tokens(&l).iter().map(|t| if t.contains('(') || t.ends_with("fr") || t.ends_with('%') || t.starts_with('[') { t.clone() } else { parse_length(t, &cx).map(fmt_px).unwrap_or_else(|| t.clone()) }).collect::<Vec<_>>().join(" ")
                    }
                }
                "grid-column" | "grid-row" => {
                    let axis = &name[5..];
                    let norm = |s: &str| s.split('/').map(|x| x.split_whitespace().collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join(" / ").to_ascii_lowercase();
                    if let Some(v) = spec.get(name) {
                        norm(v)
                    } else {
                        let s = spec.get(&format!("grid-{}-start", axis)).map(|x| norm(x)).unwrap_or_else(|| "auto".into());
                        let e = spec.get(&format!("grid-{}-end", axis)).map(|x| norm(x)).unwrap_or_else(|| "auto".into());
                        if e == "auto" {
                            s
                        } else {
                            format!("{} / {}", s, e)
                        }
                    }
                }
                "vertical-align" => {
                    let r = rawv(name).to_ascii_lowercase();
                    if r.ends_with('%') {
                        r
                    } else {
                        parse_length(&r, &cx).map(fmt_px).unwrap_or(r)
                    }
                }
                "opacity" => {
                    let r = rawv(name);
                    let x = if let Some(p) = r.trim().strip_suffix('%') { p.trim().parse::<f64>().map(|v| v / 100.0) } else { r.trim().parse::<f64>() };
                    fmt_num(x.unwrap_or(1.0).clamp(0.0, 1.0))
                }
                "overflow" => {
                    let ox = spec.get("overflow-x").map(|s| s.to_ascii_lowercase()).unwrap_or_else(|| "visible".into());
                    let oy = spec.get("overflow-y").map(|s| s.to_ascii_lowercase()).unwrap_or_else(|| "visible".into());
                    let fix = |a: &str, b: &str| if a == "visible" && b != "visible" && b != "clip" { "auto".to_string() } else { a.to_string() };
                    let (x, y) = (fix(&ox, &oy), fix(&oy, &ox));
                    if x == y {
                        x
                    } else {
                        format!("{} {}", x, y)
                    }
                }
                "transform" => norm_transform(&rawv(name), &cx),
                "filter" | "backdrop-filter" => norm_filter(&rawv(name), &cx),
                "text-overflow" => rawv(name).trim().to_ascii_lowercase(),
                "background-position" => commas(&rawv(name)).iter().map(|l| norm_position(l, &cx, false)).collect::<Vec<_>>().join(", "),
                "transform-origin" => norm_position(&rawv(name), &cx, true),
                "z-index" => {
                    let r = rawv(name);
                    match r.trim().parse::<f64>() {
                        Ok(x) => fmt_num(x.trunc()),
                        Err(_) => "auto".into(),
                    }
                }
                _ => rawv(name),
            };
            vals[idx] = v;
        }

        // blockification
        {
            let pos = vals[pidx("position")].clone();
            let float = spec.get("float").map(|s| s.to_ascii_lowercase()).unwrap_or_else(|| "none".into());
            let oof = pos == "absolute" || pos == "fixed" || (float != "none" && float != "initial");
            if oof || flexish_parent {
                let d = vals[pidx("display")].clone();
                let nd = match d.as_str() {
                    "inline" | "inline-block" | "run-in" => "block",
                    "inline-flex" => "flex",
                    "inline-grid" => "grid",
                    "inline-table" => "table",
                    x => x,
                };
                vals[pidx("display")] = nd.to_string();
            }
        }
        let _ = (doc, i);
        let hidden = vals[pidx("display")] == "none" || parent.map(|p| p.hidden).unwrap_or(false);
        Style { vals, custom, fs, lh_num, hidden, paint: Vec::new() }
    }
}

fn is_ident(s: &str) -> bool {
    let mut ch = s.chars();
    match ch.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || (c == '-' && !s.starts_with("--")) => {}
        _ => return false,
    }
    s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') && s != "-"
}

fn norm_family(r: &str) -> String {
    let mut out = Vec::new();
    for f in commas(r) {
        let f = f.trim();
        if f.is_empty() {
            continue;
        }
        if (f.starts_with('"') && f.ends_with('"')) || (f.starts_with('\'') && f.ends_with('\'')) {
            let inner = &f[1..f.len() - 1];
            if is_ident(inner) {
                out.push(inner.to_string());
            } else {
                out.push(format!("\"{}\"", inner));
            }
        } else {
            let c = f.split_whitespace().collect::<Vec<_>>().join(" ");
            if c.contains(' ') {
                out.push(format!("\"{}\"", c));
            } else {
                out.push(c);
            }
        }
    }
    out.join(", ")
}

type M = [f64; 6];

fn mul(a: M, b: M) -> M {
    [a[0] * b[0] + a[2] * b[1], a[1] * b[0] + a[3] * b[1], a[0] * b[2] + a[2] * b[3], a[1] * b[2] + a[3] * b[3], a[0] * b[4] + a[2] * b[5] + a[4], a[1] * b[4] + a[3] * b[5] + a[5]]
}

fn angle(s: &str) -> Option<f64> {
    let (n, u) = split_unit(s)?;
    Some(match u.as_str() {
        "deg" | "" => n.to_radians(),
        "rad" => n,
        "grad" => n * std::f64::consts::PI / 200.0,
        "turn" => n * std::f64::consts::TAU,
        _ => return None,
    })
}

fn norm_filter(raw: &str, cx: &Ctx) -> String {
    let l = raw.trim().to_ascii_lowercase();
    if l == "none" || l.is_empty() {
        return "none".into();
    }
    let mut out = Vec::new();
    for t in tokens(&l) {
        let Some(open) = t.find('(') else { return l };
        let name = &t[..open];
        let arg = t[open + 1..].trim_end_matches(')').trim();
        let v = match name {
            "blur" => parse_length(if arg.is_empty() { "0px" } else { arg }, cx).map(fmt_px),
            "brightness" | "contrast" | "grayscale" | "invert" | "opacity" | "saturate" | "sepia" => {
                if arg.is_empty() {
                    Some("1".to_string())
                } else if let Some(p) = arg.strip_suffix('%') {
                    p.trim().parse::<f64>().ok().map(|x| fmt_num(x / 100.0))
                } else {
                    arg.parse::<f64>().ok().map(fmt_num)
                }
            }
            "hue-rotate" => angle(if arg.is_empty() { "0deg" } else { arg }).map(|a| format!("{}deg", fmt_num(a.to_degrees()))),
            _ => Some(arg.split_whitespace().collect::<Vec<_>>().join(" ")),
        };
        match v {
            Some(v) => out.push(format!("{name}({v})")),
            None => return l,
        }
    }
    out.join(" ")
}

fn norm_position(raw: &str, cx: &Ctx, with_z: bool) -> String {
    let l = raw.trim().to_ascii_lowercase();
    let mut t = tokens(&l);
    if t.is_empty() || t.len() > 3 || (t.len() == 3 && !with_z) {
        return l;
    }
    let z = if t.len() == 3 { t.pop() } else { None };
    if t.len() == 1 {
        if t[0] == "top" || t[0] == "bottom" {
            t.insert(0, "center".into());
        } else {
            t.push("center".into());
        }
    }
    if t[0] == "top" || t[0] == "bottom" || t[1] == "left" || t[1] == "right" {
        t.swap(0, 1);
    }
    let one = |s: &str| -> Option<String> {
        match s {
            "left" | "top" => Some("0%".into()),
            "center" => Some("50%".into()),
            "right" | "bottom" => Some("100%".into()),
            _ => {
                if let Some(p) = s.strip_suffix('%') {
                    p.trim().parse::<f64>().ok().map(|x| format!("{}%", fmt_num(x)))
                } else {
                    parse_length(s, cx).map(fmt_px)
                }
            }
        }
    };
    let (Some(x), Some(y)) = (one(&t[0]), one(&t[1])) else { return l };
    match z.and_then(|z| parse_length(&z, cx)) {
        Some(z) if z != 0.0 => format!("{x} {y} {}", fmt_px(z)),
        _ => format!("{x} {y}"),
    }
}

fn norm_transform(raw: &str, cx: &Ctx) -> String {
    let l = raw.trim().to_ascii_lowercase();
    if l == "none" || l.is_empty() {
        return "none".into();
    }
    let mut m: M = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut rest = l.as_str();
    while !rest.trim().is_empty() {
        rest = rest.trim_start();
        let open = match rest.find('(') {
            Some(o) => o,
            None => return l,
        };
        let name = &rest[..open];
        let mut d = 1;
        let mut j = open + 1;
        let b = rest.as_bytes();
        while j < b.len() && d > 0 {
            match b[j] {
                b'(' => d += 1,
                b')' => d -= 1,
                _ => {}
            }
            j += 1;
        }
        let args: Vec<String> = rest[open + 1..j - 1].split(',').map(|s| s.trim().to_string()).collect();
        rest = &rest[j..];
        let len = |s: &str| parse_length(s, cx);
        let num = |s: &str| s.trim().parse::<f64>().ok();
        let t: Option<M> = match name {
            "translate" | "translate3d" => (|| Some([1.0, 0.0, 0.0, 1.0, len(&args[0])?, args.get(1).map(|a| len(a)).unwrap_or(Some(0.0))?]))(),
            "translatex" => (|| Some([1.0, 0.0, 0.0, 1.0, len(&args[0])?, 0.0]))(),
            "translatey" => (|| Some([1.0, 0.0, 0.0, 1.0, 0.0, len(&args[0])?]))(),
            "scale" | "scale3d" => (|| {
                let x = num(&args[0])?;
                let y = args.get(1).map(|a| num(a)).unwrap_or(Some(x))?;
                Some([x, 0.0, 0.0, y, 0.0, 0.0])
            })(),
            "scalex" => num(&args[0]).map(|x| [x, 0.0, 0.0, 1.0, 0.0, 0.0]),
            "scaley" => num(&args[0]).map(|y| [1.0, 0.0, 0.0, y, 0.0, 0.0]),
            "rotate" | "rotatez" => angle(&args[0]).map(|a| [a.cos(), a.sin(), -a.sin(), a.cos(), 0.0, 0.0]),
            "skewx" => angle(&args[0]).map(|a| [1.0, 0.0, a.tan(), 1.0, 0.0, 0.0]),
            "skewy" => angle(&args[0]).map(|a| [1.0, a.tan(), 0.0, 1.0, 0.0, 0.0]),
            "skew" => (|| {
                let x = angle(&args[0])?;
                let y = args.get(1).map(|a| angle(a)).unwrap_or(Some(0.0))?;
                Some([1.0, y.tan(), x.tan(), 1.0, 0.0, 0.0])
            })(),
            "matrix" if args.len() == 6 => {
                let v: Vec<f64> = args.iter().filter_map(|a| num(a)).collect();
                if v.len() == 6 {
                    Some([v[0], v[1], v[2], v[3], v[4], v[5]])
                } else {
                    None
                }
            }
            _ => None,
        };
        match t {
            Some(t) => m = mul(m, t),
            None => return l,
        }
    }
    let f = |x: f64| fmt_num(if x.abs() < 1e-9 { 0.0 } else { x });
    format!("matrix({}, {}, {}, {}, {}, {})", f(m[0]), f(m[1]), f(m[2]), f(m[3]), f(m[4]), f(m[5]))
}

pub fn _unused(_: &str) -> bool {
    is_wide("")
}

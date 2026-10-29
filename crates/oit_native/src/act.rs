//! Marks every element a tap can act on with `data-act`, worked out from the
//! page trees' own classes and ids:
//! `<scope>|<kind>:<key>#<n>`, scope `F` (FLT OPS) or `A` (NSS AVNCS), `n`
//! the element's ordinal among same kind and key in that scope.

use std::collections::{BTreeMap, HashMap};

use mfd_tree::{Kid, Node};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Act {
    pub scope: char,
    pub kind: String,
    pub key: String,
    pub n: usize,
}

impl Act {
    pub fn parse(s: &str) -> Option<Act> {
        let (scope, rest) = s.split_once('|')?;
        let (body, n) = rest.rsplit_once('#')?;
        let (kind, key) = body.split_once(':').unwrap_or((body, ""));
        Some(Act { scope: scope.chars().next()?, kind: kind.into(), key: key.into(), n: n.parse().ok()? })
    }
}

pub struct Marks<'a> {
    /// Folder path (`A/Pre-flight/Requests`) to expanded state, set by taps.
    pub folders: &'a BTreeMap<String, bool>,
    /// Values for plain `input`/`textarea` elements by scope and ordinal.
    pub texts: &'a HashMap<(char, usize), String>,
}

struct Cx<'a> {
    marks: &'a Marks<'a>,
    scope: char,
    mains: usize,
    counts: HashMap<(char, String), usize>,
}

impl Cx<'_> {
    fn mark(&mut self, n: &mut Node, kind: &str, key: &str) {
        let base = format!("{kind}:{key}");
        let c = self.counts.entry((self.scope, base.clone())).or_insert(0);
        let v = format!("{}|{}#{}", self.scope, base, *c);
        *c += 1;
        n.data.insert("act".into(), v);
    }

    fn ordinal(&mut self, kind: &str) -> usize {
        let c = self.counts.entry((self.scope, format!("{kind}:"))).or_insert(0);
        let v = *c;
        *c += 1;
        v
    }
}

fn has(n: &Node, c: &str) -> bool {
    n.cls.iter().any(|x| x == c)
}

pub fn text_of(n: &Node) -> String {
    let mut out = String::new();
    collect(n, &mut out);
    out.trim().to_string()
}

fn collect(n: &Node, out: &mut String) {
    for k in &n.kids {
        match k {
            Kid::Text { text } => out.push_str(text),
            Kid::Node(c) if c.tag == "br" => out.push(' '),
            Kid::Node(c) => collect(c, out),
        }
    }
}

fn kids_mut(n: &mut Node) -> impl Iterator<Item = &mut Node> {
    n.kids.iter_mut().filter_map(|k| if let Kid::Node(c) = k { Some(c) } else { None })
}

fn kids(n: &Node) -> impl Iterator<Item = &Node> {
    n.kids.iter().filter_map(|k| if let Kid::Node(c) = k { Some(c) } else { None })
}

/// The id prefix of a dropdown's menu (`FO_OIT_toPerfFlaps`).
fn menu_prefix(container: &Node) -> Option<String> {
    let menu = kids(container).find(|k| has(k, "mfd-dropdown-menu"))?;
    let first = kids(menu).find_map(|e| e.id.clone())?;
    first.rsplit_once('_').map(|(p, _)| p.to_string())
}

pub fn annotate(root: &mut Node, marks: &Marks<'_>) {
    let mut cx = Cx { marks, scope: 'F', mains: 0, counts: HashMap::new() };
    walk(root, &mut cx, false, "");
}

fn walk(n: &mut Node, cx: &mut Cx<'_>, in_dd: bool, folder: &str) {
    if has(n, "oit-main") {
        cx.scope = if cx.mains == 0 { 'F' } else { 'A' };
        cx.mains += 1;
    }
    let disabled = has(n, "disabled") || has(n, "inactive");
    let mut in_dd = in_dd;
    let mut folder = folder.to_string();
    if n.tag == "div" && has(n, "mfd-dropdown-container") {
        if let Some(p) = menu_prefix(n) {
            for k in kids_mut(n) {
                if has(k, "mfd-page-selector-outer") {
                    cx.mark(k, "ps", &p);
                } else if has(k, "mfd-dropdown-outer") && !has(k, "disabled") && !has(k, "inactive") {
                    cx.mark(k, "dd", &p);
                }
            }
        }
        if kids(n).any(|k| has(k, "mfd-dropdown-outer")) {
            in_dd = true;
        }
    }
    if n.tag == "span" && has(n, "mfd-button") && !disabled {
        let label = text_of(n);
        cx.mark(n, "b", &label);
    } else if has(n, "mfd-dropdown-menu-element") && !disabled {
        if let Some(id) = n.id.clone() {
            cx.mark(n, "mi", &id);
        }
    } else if has(n, "mfd-input-field-container") && !in_dd && !disabled {
        cx.mark(n, "f", "");
    } else if n.tag == "label" && has(n, "mfd-radio-button") {
        if let Some(id) = n.id.clone() {
            cx.mark(n, "r", &id.replacen("_label_", "_", 1));
        }
    } else if has(n, "oit-avncs-navigator-file-name") && !disabled {
        let t = text_of(n);
        cx.mark(n, "nf", &t);
    } else if has(n, "oit-avncs-navigator-folder-container") {
        let name = kids(n)
            .next()
            .and_then(|row| kids(row).find(|s| has(s, "oit-avncs-navigator-folder")))
            .map(text_of)
            .unwrap_or_default();
        folder = if folder.is_empty() { format!("{}/{}", cx.scope, name) } else { format!("{folder}/{name}") };
        if let Some(&open) = cx.marks.folders.get(&folder) {
            set_folder(n, open);
        }
        if let Some(row) = kids_mut(n).next() {
            cx.mark(row, "fo", &folder);
        }
    } else if has(n, "oefb-item") {
        cx.mark(n, "oi", "");
    } else if has(n, "oefb-btn") {
        let t = text_of(n);
        cx.mark(n, "ob", &t);
    } else if has(n, "oefb-chip") {
        cx.mark(n, "oc", "");
    } else if has(n, "oefb-seg") {
        let seg = cx.ordinal("seg");
        for (i, k) in kids_mut(n).enumerate() {
            k.data.insert("act".into(), format!("{}|os:{seg}:{i}#0", cx.scope));
        }
    } else if has(n, "oit-cb-row") && !has(n, "oit-cb-header") {
        cx.mark(n, "cb", "");
    } else if has(n, "oit-ccom-inbox-msg-table") {
        if let Some(rows) = kids_mut(n).nth(1) {
            if rows.cls.is_empty() {
                for r in kids_mut(rows) {
                    cx.mark(r, "ib", "");
                }
            }
        }
    } else if has(n, "fr") && has(n, "ass") && n.inline.as_deref().is_some_and(|s| s.contains("cursor: pointer")) {
        cx.mark(n, "row", "");
    } else if has(n, "oit-avncs-navigator-file") && n.inline.as_deref().is_some_and(|s| s.contains("cursor: pointer")) && !kids(n).any(|k| has(k, "oit-avncs-navigator-file-name")) {
        cx.mark(n, "lib", "");
    } else if matches!(n.tag.as_str(), "input" | "textarea" | "select") && !has(n, "oefb-input") {
        let i = cx.ordinal("t");
        if let Some(v) = cx.marks.texts.get(&(cx.scope, i)) {
            n.value = Some(v.clone());
        }
        n.data.insert("act".into(), format!("{}|t:#{i}", cx.scope));
    } else if n.tag == "input" && has(n, "oefb-input") {
        cx.mark(n, "icao", "");
    }
    for k in kids_mut(n) {
        walk(k, cx, in_dd, &folder);
    }
}

fn set_folder(container: &mut Node, open: bool) {
    let mut it = kids_mut(container);
    if let Some(row) = it.next() {
        for k in kids_mut(row) {
            if has(k, "oit-avncs-icon-folder-opener") {
                k.kids = vec![Kid::Text { text: if open { "-".into() } else { "+".into() } }];
            }
            if has(k, "oit-avncs-navigator-folder") {
                k.cls.retain(|c| c != "expanded");
                if open {
                    k.cls.push("expanded".into());
                }
            }
        }
    }
    if let Some(children) = it.next() {
        children.cls.retain(|c| c != "hidden");
        if !open {
            children.cls.push("hidden".into());
        }
    }
}

/// The current expanded state of the folder rows in a built tree, by path.
pub fn folder_state(root: &Node) -> BTreeMap<String, bool> {
    let mut out = BTreeMap::new();
    find_folders(root, &mut out);
    out
}

fn find_folders(n: &Node, out: &mut BTreeMap<String, bool>) {
    if has(n, "oit-avncs-navigator-folder-container") {
        if let Some(row) = kids(n).next() {
            if let Some(path) = row.data.get("act").and_then(|a| Act::parse(a)).map(|a| a.key) {
                let open = kids(row).any(|s| has(s, "oit-avncs-navigator-folder") && has(s, "expanded"));
                out.insert(path, open);
            }
        }
    }
    for k in kids(n) {
        find_folders(k, out);
    }
}

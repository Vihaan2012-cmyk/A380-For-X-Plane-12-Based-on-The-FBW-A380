use crate::js::el;
use crate::Node;
use regex::{Captures, Regex};
use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

macro_rules! re {
    ($s:expr) => {{
        static R: OnceLock<Regex> = OnceLock::new();
        R.get_or_init(|| Regex::new($s).unwrap())
    }};
}

pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    re!(r"&(#x[0-9a-fA-F]+|#[0-9]+|[a-zA-Z]+);")
        .replace_all(s, |c: &Captures| {
            let e = &c[1];
            let ch = if let Some(h) = e.strip_prefix("#x").or_else(|| e.strip_prefix("#X")) {
                u32::from_str_radix(h, 16).ok().and_then(char::from_u32)
            } else if let Some(d) = e.strip_prefix('#') {
                d.parse().ok().and_then(char::from_u32)
            } else {
                match e {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "nbsp" => Some('\u{a0}'),
                    "deg" => Some('\u{b0}'),
                    _ => None,
                }
            };
            ch.map(String::from).unwrap_or_else(|| c[0].to_string())
        })
        .into_owned()
}

const VOID: [&str; 5] = ["br", "hr", "img", "input", "meta"];

/// Parses the HTML the page builds with strings into nodes.
pub fn html_nodes(markup: &str) -> Vec<Node> {
    let mut stack: Vec<Node> = vec![Node::new("#root")];
    let mut rest = markup;
    loop {
        let lt = rest.find('<');
        let text = &rest[..lt.unwrap_or(rest.len())];
        if !text.is_empty() {
            let t = decode_entities(text);
            let top = stack.last_mut().unwrap();
            *top = std::mem::take(top).text(&t);
        }
        let Some(lt) = lt else { break };
        rest = &rest[lt + 1..];
        let gt = rest.find('>').unwrap_or(rest.len());
        let tag_src = &rest[..gt];
        rest = &rest[(gt + 1).min(rest.len())..];
        if let Some(name) = tag_src.strip_prefix('/') {
            let name = name.trim().to_ascii_lowercase();
            if let Some(pos) = stack.iter().rposition(|n| n.tag == name) {
                while stack.len() > pos {
                    let n = stack.pop().unwrap();
                    let top = stack.last_mut().unwrap();
                    *top = std::mem::take(top).child(n);
                }
            }
            continue;
        }
        let body = tag_src.trim_end_matches('/');
        let (name, attrs) = match body.find(|c: char| c.is_ascii_whitespace()) {
            Some(p) => (&body[..p], &body[p..]),
            None => (body, ""),
        };
        let name = name.to_ascii_lowercase();
        if name == "svg" {
            let end = rest.find("</svg>").map(|p| p + 6).unwrap_or(rest.len());
            let n = crate::js::svg(&format!("<{tag_src}>{}", &rest[..end]));
            rest = &rest[end..];
            let top = stack.last_mut().unwrap();
            *top = std::mem::take(top).child(n);
            continue;
        }
        let mut n = Node::new(&name);
        for (k, v) in parse_attrs(attrs) {
            match k.as_str() {
                "class" => n.cls = v.split_whitespace().map(str::to_string).collect(),
                "id" => n.id = Some(v),
                "style" => n.inline = Some(v),
                "value" if matches!(name.as_str(), "input" | "textarea") => n.value = Some(v),
                _ => {
                    n.attrs.insert(k, v);
                }
            }
        }
        if VOID.contains(&name.as_str()) || tag_src.ends_with('/') {
            let top = stack.last_mut().unwrap();
            *top = std::mem::take(top).child(n);
        } else {
            if name == "tr" && stack.last().is_some_and(|t| t.tag == "table") {
                stack.push(Node::new("tbody"));
            }
            stack.push(n);
        }
    }
    while stack.len() > 1 {
        let n = stack.pop().unwrap();
        let top = stack.last_mut().unwrap();
        *top = std::mem::take(top).child(n);
    }
    stack
        .pop()
        .unwrap()
        .kids
        .into_iter()
        .filter_map(|k| match k {
            mfd_tree::Kid::Node(n) => Some(n),
            mfd_tree::Kid::Text { .. } => None,
        })
        .collect()
}

fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for c in re!(r#"([^\s=]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?"#).captures_iter(s) {
        let v = c.get(2).or(c.get(3)).or(c.get(4)).map(|m| decode_entities(m.as_str())).unwrap_or_default();
        out.push((c[1].to_ascii_lowercase(), v));
    }
    out
}

/// An element whose innerHTML is `markup`.
pub fn with_html(mut n: Node, markup: &str) -> Node {
    for k in html_nodes_with_text(markup) {
        n.kids.push(k);
    }
    n
}

fn html_nodes_with_text(markup: &str) -> Vec<mfd_tree::Kid> {
    let wrapped = format!("<x-root>{markup}</x-root>");
    let mut root = html_nodes(&wrapped);
    root.pop().map(|r| r.kids).unwrap_or_default()
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn dir(path: &str) -> String {
    match path.rfind('/') {
        Some(i) => path[..=i].to_string(),
        None => String::new(),
    }
}

pub fn slug(t: &str) -> String {
    let s = t.to_lowercase();
    let s = re!(r"<[^>]+>").replace_all(&s, "");
    let s = re!(r"[^a-z0-9]+").replace_all(&s, "-");
    let s = s.strip_prefix('-').unwrap_or(&s);
    s.strip_suffix('-').unwrap_or(s).to_string()
}

pub fn encode_uri_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn inline(s: &str, base: &str) -> String {
    let mut codes: Vec<String> = Vec::new();
    let h = esc(s);
    let h = re!(r"`([^`]+)`").replace_all(&h, |c: &Captures| {
        codes.push(c[1].to_string());
        format!("\u{0}{}\u{0}", codes.len() - 1)
    });
    let h = re!(r"!\[([^\]]*)\]\(([^)\s]+)\)").replace_all(&h, |c: &Captures| {
        let src = &c[2];
        let s = if re!(r"^https?:").is_match(src) { src.to_string() } else { format!("/api/docfile?path={}", encode_uri_component(&format!("{base}{src}"))) };
        format!(r#"<img class="md-img" alt="{}" src="{s}">"#, &c[1])
    });
    let h = re!(r"\*\*([^*]+)\*\*").replace_all(&h, "<b>${1}</b>");
    let h = re!(r"__([^_]+)__").replace_all(&h, "<b>${1}</b>");
    let h = re!(r"(^|[^*])\*([^*]+)\*").replace_all(&h, "${1}<i>${2}</i>");
    let h = re!(r"(^|[^_0-9A-Za-z])_([^_]+)_").replace_all(&h, "${1}<i>${2}</i>");
    let h = re!(r"~~([^~]+)~~").replace_all(&h, "<s>${1}</s>");
    let h = re!(r"\[([^\]]+)\]\(([^)\s]+)\)").replace_all(&h, |c: &Captures| {
        let href = &c[2];
        let target = if href.starts_with('#') || re!(r"^https?:").is_match(href) { href.to_string() } else { format!("{base}{href}") };
        format!(r#"<span class="md-link" data-href="{target}">{}</span>"#, &c[1])
    });
    re!("\u{0}(\\d+)\u{0}").replace_all(&h, |c: &Captures| format!("<code>{}</code>", codes[c[1].parse::<usize>().unwrap()])).into_owned()
}

fn check_line(text: &str, base: &str) -> String {
    let parts: Vec<&str> = re!(r"\s*\.{3,}\s*|\s+\|\s+").split(text).collect();
    let name = parts[0];
    let value = parts[1..].join(" ");
    let v = if value.is_empty() { String::new() } else { format!(r#"<span class="qd-dots"></span><span class="qd-v">{}</span>"#, inline(&value, base)) };
    format!(r#"<div class="qd-line md-check"><span class="qd-n">{}</span>{v}</div>"#, inline(name, base))
}

#[derive(Clone, Debug)]
pub struct OutlineItem {
    pub level: usize,
    pub id: String,
    pub text: String,
    pub fold: bool,
}

pub struct Rendered {
    pub html: String,
    pub outline: Vec<OutlineItem>,
}

/// `mdRender`: the document's markdown as the page's HTML.
pub fn render(text: &str, path: &str) -> Rendered {
    let base = dir(path);
    let text = text.replace("\r\n", "\n");
    let lines: Vec<&str> = text.split('\n').collect();
    let mut outline = Vec::new();
    let mut used: HashMap<String, usize> = HashMap::new();
    let mut html: Vec<String> = Vec::new();
    let mut i = 0;
    let list_re = re!(r"^\s*([-*+]|\d+\.)\s+");
    let check_re = re!(r"^\s*[-*+]\s+\[[ xX]\]\s+");
    let row_re = re!(r"^\|.*\|\s*$");
    while i < lines.len() {
        let line = lines[i];
        if let Some(open) = re!(r"(?i)^(?::::\s*(?:details|collapse)\s+(.*)|<details>\s*(?:<summary>(.*?)</summary>)?\s*)$").captures(line) {
            let t = open.get(1).or(open.get(2)).map(|m| m.as_str()).filter(|s| !s.is_empty()).unwrap_or("Details").trim().to_string();
            html.push(format!(r#"<details class="md-fold"><summary class="qd-card"><span>{}</span><span class="qd-chev">{}</span></summary><div class="qd-cardbody">"#, inline(&t, &base), '\u{25be}'));
            outline.push(OutlineItem { level: 3, id: String::new(), text: t, fold: true });
            i += 1;
            continue;
        }
        if re!(r"(?i)^(:::|</details>)\s*$").is_match(line) {
            html.push("</div></details>".into());
            i += 1;
            continue;
        }
        if line.starts_with("```") {
            let mut buf = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].starts_with("```") {
                buf.push(esc(lines[i]));
                i += 1;
            }
            i += 1;
            html.push(format!(r#"<pre class="md-pre">{}</pre>"#, buf.join("\n")));
            continue;
        }
        if let Some(h) = re!(r"^(#{1,4})\s+(.*)$").captures(line) {
            let level = h[1].len();
            let raw = &h[2];
            let inner = inline(raw, &base);
            let mut id = slug(raw);
            if id.is_empty() {
                id = "section".into();
            }
            let n = *used.get(&id).unwrap_or(&0);
            used.insert(id.clone(), n + 1);
            if n > 0 {
                id = format!("{id}-{n}");
            }
            outline.push(OutlineItem { level, id: id.clone(), text: re!(r"[*_`~]").replace_all(raw, "").into_owned(), fold: false });
            html.push(format!(r#"<div class="md-h{level}" id="mdh-{id}">{inner}</div>"#));
            i += 1;
            continue;
        }
        if re!(r"^\s*(-{3,}|\*{3,})\s*$").is_match(line) {
            html.push(r#"<hr class="md-hr">"#.into());
            i += 1;
            continue;
        }
        if re!(r"^\s*>").is_match(line) {
            let mut buf = Vec::new();
            while i < lines.len() && re!(r"^\s*>").is_match(lines[i]) {
                buf.push(inline(&re!(r"^\s*>\s?").replace(lines[i], ""), &base));
                i += 1;
            }
            html.push(format!(r#"<div class="md-quote">{}</div>"#, buf.join("<br>")));
            continue;
        }
        if row_re.is_match(line) && i + 1 < lines.len() && re!(r"^\|?\s*:?-+").is_match(lines[i + 1]) {
            let row = |l: &str| -> Vec<String> { re!(r"^\||\|$").replace_all(l.trim(), "").split('|').map(|c| inline(c.trim(), &base)).collect() };
            let head = row(line);
            i += 2;
            let mut body = Vec::new();
            while i < lines.len() && row_re.is_match(lines[i]) {
                body.push(row(lines[i]));
                i += 1;
            }
            html.push(format!(
                r#"<table class="md-table"><tr>{}</tr>{}</table>"#,
                head.iter().map(|c| format!("<th>{c}</th>")).collect::<String>(),
                body.iter().map(|r| format!("<tr>{}</tr>", r.iter().map(|c| format!("<td>{c}</td>")).collect::<String>())).collect::<String>()
            ));
            continue;
        }
        if check_re.is_match(line) {
            while i < lines.len() && check_re.is_match(lines[i]) {
                html.push(check_line(&check_re.replace(lines[i], ""), &base));
                i += 1;
            }
            continue;
        }
        if list_re.is_match(line) {
            let mut items = Vec::new();
            let ordered = re!(r"^\s*\d+\.").is_match(line);
            while i < lines.len() && list_re.is_match(lines[i]) && !check_re.is_match(lines[i]) {
                let m = re!(r"^(\s*)([-*+]|\d+\.)\s+(.*)$").captures(lines[i]).unwrap();
                items.push(format!(r#"<li class="md-l{}">{}</li>"#, (m[1].len() / 2).min(3), inline(&m[3], &base)));
                i += 1;
            }
            let tag = if ordered { "ol" } else { "ul" };
            html.push(format!(r#"<{tag} class="md-list">{}</{tag}>"#, items.join("")));
            continue;
        }
        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        let mut buf = Vec::new();
        while i < lines.len() && !lines[i].trim().is_empty() && !re!(r"(?i)^(#{1,4}\s|```|\s*>|\||:::|</?details)").is_match(lines[i]) && !list_re.is_match(lines[i]) {
            buf.push(inline(lines[i], &base));
            i += 1;
        }
        html.push(format!(r#"<p class="md-p">{}</p>"#, buf.join("<br>")));
    }
    Rendered { html: html.join(""), outline }
}

fn mark_checks(n: &mut Node, ticks: &BTreeSet<usize>, idx: &mut usize, current: &mut Option<usize>, total: usize) {
    if n.cls.iter().any(|c| c == "md-check") {
        let k = *idx;
        *idx += 1;
        if ticks.contains(&k) {
            n.cls.push("done".into());
        }
        if current.is_none() && !ticks.contains(&k) {
            *current = Some(k);
            n.cls.push("current".into());
        }
        let _ = total;
    }
    for k in n.kids.iter_mut() {
        if let mfd_tree::Kid::Node(c) = k {
            mark_checks(c, ticks, idx, current, total);
        }
    }
}

/// `mdShow`: the rendered document, with its check lines ticked and an outline pane when it has two or more entries.
pub fn show(r: &Rendered, ticks: &BTreeSet<usize>) -> Node {
    let mut root = with_html(el("div", "md"), &r.html);
    let (mut idx, mut cur) = (0, None);
    mark_checks(&mut root, ticks, &mut idx, &mut cur, 0);
    let items: Vec<&OutlineItem> = r.outline.iter().filter(|o| !o.id.is_empty() || o.fold).collect();
    if items.len() < 2 {
        return root;
    }
    let min = items.iter().map(|o| o.level).min().unwrap_or(1);
    let mut pane = el("div", "md-toc").child(el("div", "md-toc-h").text("ON THIS PAGE"));
    for o in items {
        pane = pane.child(el("button", "md-toc-i").attr("type", "button").style(&format!("padding-left:{}px", 12 + (o.level - min) * 16)).text(&o.text));
    }
    el("div", "md-wrap").child(el("div", "md-col").child(root)).child(pane)
}

/// `textContent` of SimBrief's `plan_html` with line breaks kept.
pub fn html_text(html: &str) -> String {
    let s = re!(r"(?is)<script.*?</script>").replace_all(html, "");
    let s = re!(r"(?i)<br\s*/?>").replace_all(&s, "\n");
    let s = re!(r"<[^>]*>").replace_all(&s, "");
    decode_entities(&s).replace('\r', "")
}

pub fn ofp_plan_markdown(text: &str, org: &str, dst: &str) -> String {
    let head = re!(r"^[ \t]*(?:-{3,}|={3,})[ \t]*([A-Z][A-Z0-9 /&()-]{2,}?)[ \t]*(?:-{3,}|={3,})?[ \t]*$");
    let lines: Vec<&str> = text.split('\n').collect();
    let mut chunks: Vec<Vec<&str>> = vec![vec![]];
    for (k, l) in lines.iter().enumerate() {
        if k > 0 && k + 1 < lines.len() && head.is_match(l) {
            chunks.push(vec![]);
        }
        chunks.last_mut().unwrap().push(l);
    }
    let mut md = vec![format!("# OFP {org} - {dst}")];
    for c in chunks {
        let joined = c.join("\n");
        let trimmed = joined.trim_end_matches('\n').to_string();
        if c.len() > 1 {
            if let Some(m) = head.captures(c[0]) {
                md.push(format!("## {}", m[1].trim()));
                md.push("```".into());
                md.push(c[1..].join("\n").trim_end_matches('\n').to_string());
                md.push("```".into());
                continue;
            }
        }
        md.push("```".into());
        md.push(trimmed);
        md.push("```".into());
    }
    md.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_blocks() {
        let r = render("# Title\n\nSome **bold** and _it_ text\n- a\n- b\n- [x] Check ... SET\n| A | B |\n|---|---|\n| 1 | 2 |", "dir/x.md");
        assert!(r.html.starts_with(r#"<div class="md-h1" id="mdh-title">Title</div><p class="md-p">Some <b>bold</b> and <i>it</i> text</p><ul class="md-list">"#), "{}", r.html);
        assert!(r.html.contains(r#"<div class="qd-line md-check"><span class="qd-n">Check</span><span class="qd-dots"></span><span class="qd-v">SET</span></div>"#));
        let n = show(&r, &BTreeSet::new());
        assert_eq!(n.tag, "div");
    }

    #[test]
    fn ofp_sections() {
        let t = "HEADER\n--- FUEL PLAN ---\nTRIP 100\n=== ROUTE ===\nA B\n";
        let md = ofp_plan_markdown(t, "EGLL", "KJFK");
        assert_eq!(md, "# OFP EGLL - KJFK\n```\nHEADER\n```\n## FUEL PLAN\n```\nTRIP 100\n```\n## ROUTE\n```\nA B\n```");
    }
}

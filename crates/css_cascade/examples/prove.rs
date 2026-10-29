use css_cascade::{parse_html, Cascade, PROPS};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::fs;

const SKIP: &[&str] = &["width", "height", "top", "left", "right", "bottom"];

#[derive(Default)]
struct Tally {
    ok: u32,
    bad: u32,
    layout: u32,
}

fn parse_golden(s: &str) -> Vec<(String, String)> {
    s.split(';').filter_map(|p| p.find(':').map(|c| (p[..c].trim().to_string(), p[c + 1..].trim().to_string()))).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let root = args.get(1).cloned().unwrap_or_else(|| "E:/golden-tour/yssy-omdb-v15/captures".into());
    let html_ui = args.get(2).cloned().unwrap_or_else(|| "D:/Steam Games/steamapps/common/X-Plane 12/Aircraft/A380 for X-Plane 12/html_ui".into());
    let out_path = args.get(3).cloned().unwrap_or_else(|| "E:/fbw-build/css-cascade/report.txt".into());
    let filter = args.get(4).cloned().unwrap_or_default();

    let mut dirs: Vec<_> = fs::read_dir(&root).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).collect();
    dirs.sort();
    let mut report = String::new();
    let mut per_prop: BTreeMap<String, Tally> = BTreeMap::new();
    let mut per_kind: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    let mut mism: HashMap<(String, String, String), (u32, String)> = HashMap::new();
    let mut css_cache: HashMap<String, String> = HashMap::new();
    let mut dumped = 0;
    let mut per_cap: Vec<(String, u32, u32)> = Vec::new();

    for d in dirs {
        let name = d.file_name().unwrap().to_string_lossy().to_string();
        if !(name.contains("_mfd_") || name.contains("_oit_")) || !name.contains(&filter) {
            continue;
        }
        let file = fs::read_dir(&d).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.to_string_lossy().ends_with(".dom.html"));
        let file = match file {
            Some(f) => f,
            None => continue,
        };
        let kind = if name.contains("_mfd_") { "mfd" } else { "oit" };
        let text = fs::read_to_string(&file).unwrap();
        let doc = parse_html(&text);
        let mut vw = 1646.0;
        let mut vh = 1024.0;
        if let Some(g) = doc.els.iter().find(|e| e.tag == "vcockpit-panel").and_then(|e| e.golden.clone()) {
            for (k, v) in parse_golden(&g) {
                if k == "width" {
                    vw = v.trim_end_matches("px").parse().unwrap_or(vw);
                }
                if k == "height" {
                    vh = v.trim_end_matches("px").parse().unwrap_or(vh);
                }
            }
        }
        let mut c = Cascade::with_defaults(vw, vh);
        c.dpr = std::env::var("DPR").ok().and_then(|x| x.parse().ok()).unwrap_or(if kind == "mfd" { 1.5 } else { 1.0 });
        for s in &doc.styles {
            c.add_sheet(s, 1);
        }
        for l in &doc.links {
            let css = css_cache.entry(l.clone()).or_insert_with(|| fs::read_to_string(format!("{}{}", html_ui, l)).unwrap_or_default()).clone();
            c.add_sheet(&css, 1);
        }
        let styles = c.compute(&doc);
        let (mut ok, mut bad) = (0u32, 0u32);
        for (i, e) in doc.els.iter().enumerate() {
            let g = match &e.golden {
                Some(g) => g,
                None => continue,
            };
            let st = &styles[i];
            for (k, gv) in parse_golden(g) {
                if SKIP.contains(&k.as_str()) {
                    continue;
                }
                let mine = match st.get(&k) {
                    Some(m) => m.to_string(),
                    None => continue,
                };
                let t = per_prop.entry(k.clone()).or_default();
                if mine == gv {
                    t.ok += 1;
                    ok += 1;
                    continue;
                }
                let layoutish = (k.starts_with("margin") || k.starts_with("padding")) && (mine == "auto" || mine.ends_with('%'))
                    || (k.starts_with("grid-template") && mine != "none")
                    || (k == "transform" && mine.contains('%'))
                    || (k == "line-height" && false);
                if layoutish {
                    t.layout += 1;
                    continue;
                }
                t.bad += 1;
                bad += 1;
                if std::env::var("DUMP").ok().as_deref() == Some(k.as_str()) && dumped < 25 {
                    dumped += 1;
                    let gp = e.parent.and_then(|p| doc.els[p].golden.clone()).map(|g| parse_golden(&g)).unwrap_or_default();
                    let pg = |n: &str| gp.iter().find(|(a, _)| a == n).map(|(_, v)| v.clone()).unwrap_or_default();
                    let og = parse_golden(g);
                    let og = |n: &str| og.iter().find(|(a, _)| a == n).map(|(_, v)| v.clone()).unwrap_or_default();
                    println!("DUMP {} <{} .{}> exp={} got={} | parent<{}> disp={} | disp={} pos={} ov={}", k, e.tag, e.classes.join("."), gv, mine, e.parent.map(|p| doc.els[p].tag.clone()).unwrap_or_default(), pg("display"), og("display"), og("position"), og("overflow"));
                }
                let desc = format!("{}{}{} [{}]", e.tag, e.id.as_ref().map(|i| format!("#{}", i)).unwrap_or_default(), if e.classes.is_empty() { String::new() } else { format!(".{}", e.classes.join(".")) }, name);
                let m = mism.entry((k.clone(), gv.clone(), mine.clone())).or_insert((0, desc));
                m.0 += 1;
            }
        }
        let tot = ok + bad;
        let k = per_kind.entry(kind.into()).or_default();
        k.0 += ok;
        k.1 += bad;
        per_cap.push((name, ok, bad));
        let _ = tot;
    }

    writeln!(report, "== per property ==").unwrap();
    let (mut tok, mut tbad, mut tlay) = (0, 0, 0);
    for p in PROPS {
        if let Some(t) = per_prop.get(*p) {
            let n = t.ok + t.bad;
            let pct = if n > 0 { 100.0 * t.ok as f64 / n as f64 } else { 100.0 };
            writeln!(report, "{:28} {:7.3}%  ok {:7} bad {:6} layout-resolved {:6}", p, pct, t.ok, t.bad, t.layout).unwrap();
            tok += t.ok;
            tbad += t.bad;
            tlay += t.layout;
        }
    }
    writeln!(report, "TOTAL {:.4}%  ok {} bad {} layout-resolved(skipped) {}", 100.0 * tok as f64 / (tok + tbad).max(1) as f64, tok, tbad, tlay).unwrap();
    writeln!(report, "\n== per kind ==").unwrap();
    for (k, (o, b)) in &per_kind {
        writeln!(report, "{} {:.4}%  ({} ok, {} bad)", k, 100.0 * *o as f64 / (*o + *b).max(1) as f64, o, b).unwrap();
    }
    writeln!(report, "\n== per capture ==").unwrap();
    for (n, o, b) in &per_cap {
        writeln!(report, "{:100} {:8.4}%  ({} bad)", n, 100.0 * *o as f64 / (*o + *b).max(1) as f64, b).unwrap();
    }
    let mut ms: Vec<_> = mism.into_iter().collect();
    ms.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    writeln!(report, "\n== mismatches (prop | expected | got | count | sample) ==").unwrap();
    for ((p, e, g), (n, s)) in ms.iter().take(400) {
        writeln!(report, "{} | {} | {} | {} | {}", p, e, g, n, s).unwrap();
    }
    fs::write(&out_path, &report).unwrap();
    let head: String = report.lines().take(75).collect::<Vec<_>>().join("\n");
    println!("{}", head);
    let _ = ();
}

use mfd_native::{find_mut, mfd_screen, node_from_json, splice_left, Native};
use mfd_tree::Node;
use std::fs;

fn take(tree: &mut Node, id: &str) -> Node {
    find_mut(tree, &|n| n.id.as_deref() == Some(id)).map(std::mem::take).unwrap_or_else(|| Node::new("div").id(id))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: assemble <html_ui> <jobs.txt> <out_dir>   (jobs: <name> <capture_tree.json> <builder.json|->)");
        std::process::exit(2);
    }
    let native = Native::load(&args[1], 1.5).expect("mfd.css");
    let jobs = fs::read_to_string(&args[2]).expect("jobs");
    for line in jobs.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let mut cap = node_from_json(&fs::read_to_string(f[1]).expect("capture tree")).expect("capture json");
        let left = take(&mut cap, "MFD_LEFT_PARENT_DIV");
        let right = take(&mut cap, "MFD_RIGHT_PARENT_DIV");
        let mut screen = mfd_screen(left, right);
        let mut tag = "stripped";
        if f.len() > 2 && f[2] != "-" {
            let part = node_from_json(&fs::read_to_string(f[2]).expect("builder tree")).expect("builder json");
            if !splice_left(&mut screen, part) {
                eprintln!("{}: splice failed", f[0]);
                continue;
            }
            tag = "native";
        }
        let t0 = std::time::Instant::now();
        let html = native.document(&screen);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let out = format!("{}/{}.{}.dom.html", args[3], f[0], tag);
        fs::write(&out, html).expect("write");
        println!("{} {} {:.1} ms", f[0], tag, ms);
    }
}

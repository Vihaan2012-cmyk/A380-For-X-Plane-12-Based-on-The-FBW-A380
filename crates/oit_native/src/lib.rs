pub mod act;
pub mod data;
pub mod hit;
pub mod ofp;
pub mod style;

use std::collections::{BTreeMap, HashMap};

use oit_pages::OitModel;

pub struct Page {
    pub html: String,
    pub nodes: Vec<style::NodeInfo>,
}

/// The oitx document for one side: page tree, tap marks, cascade.
pub fn document(styler: &style::Styler, model: &OitModel, folders: &BTreeMap<String, bool>, texts: &HashMap<(char, usize), String>) -> Page {
    let mut content = oit_pages::build(model);
    act::annotate(&mut content, &act::Marks { folders, texts });
    let built = styler.build(&style::wrap(content, if model.capt { 1 } else { 2 }));
    Page { html: built.html, nodes: built.nodes }
}

/// Element-by-element agreement of two snapshots from `<body>` on: tag and
/// class sequence, then computed style. (matching, total, first difference)
pub fn compare(native: &str, cef: &str) -> (usize, usize, Option<String>) {
    let body = |s: &str| s.find("<body").map(|i| s[i..].to_string()).unwrap_or_default();
    let a = css_cascade::parse_html(&body(native));
    let b = css_cascade::parse_html(&body(cef));
    let total = a.els.len().max(b.els.len());
    let mut same = 0;
    let mut first = None;
    for i in 0..total {
        let (x, y) = (a.els.get(i), b.els.get(i));
        let ok = match (x, y) {
            (Some(x), Some(y)) => x.tag == y.tag && x.classes == y.classes && x.golden == y.golden,
            _ => false,
        };
        if ok {
            same += 1;
        } else if first.is_none() {
            first = Some(format!(
                "#{i} native={} cef={}",
                x.map_or("-".into(), |e| format!("{}.{}", e.tag, e.classes.join("."))),
                y.map_or("-".into(), |e| format!("{}.{}", e.tag, e.classes.join(".")))
            ));
        }
    }
    (same, total, first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hit::Rect;

    fn page(model: &OitModel) -> Page {
        document(&style::Styler::default(), model, &BTreeMap::new(), &HashMap::new())
    }

    #[test]
    fn document_is_wrapped_and_styled() {
        let p = page(&OitModel { capt: true, oit_visible: true, ..Default::default() });
        assert!(p.html.starts_with("<body"));
        assert!(p.html.contains("id=\"OIT_CONTENT\""));
        assert!(p.html.contains("data-cs=\"display:"));
        assert_eq!(p.nodes[0].tag, "body");
    }

    #[test]
    fn buttons_menus_and_folders_are_marked() {
        let p = page(&OitModel { capt: false, oit_visible: true, ..Default::default() });
        let acts: Vec<&str> = p.nodes.iter().filter_map(|n| n.act.as_deref()).collect();
        assert!(acts.contains(&"F|b:FLT FOLDER#0"));
        assert!(acts.contains(&"F|ps:FO_OIT_menu_menu#0"));
        assert!(acts.contains(&"F|mi:FO_OIT_menu_menu_4#0"));
        assert!(acts.contains(&"A|b:COMPANY COM#0"));
        assert!(acts.iter().any(|a| a.starts_with("A|fo:A/OIS MENU")));
        assert!(!acts.iter().any(|a| a.contains("UNDO")));
        let a = act::Act::parse("A|fo:A/OIS MENU/UTILITIES#0").unwrap();
        assert_eq!((a.scope, a.kind.as_str(), a.key.as_str(), a.n), ('A', "fo", "A/OIS MENU/UTILITIES", 0));
    }

    #[test]
    fn a_folder_override_collapses_it() {
        let mut f = BTreeMap::new();
        f.insert("A/OIS MENU/UTILITIES".to_string(), false);
        let mut content = oit_pages::build(&OitModel::default());
        act::annotate(&mut content, &act::Marks { folders: &f, texts: &HashMap::new() });
        assert_eq!(act::folder_state(&content).get("A/OIS MENU/UTILITIES"), Some(&false));
        assert_eq!(act::folder_state(&content).get("A/OIS MENU"), Some(&true));
    }

    #[test]
    fn hits_go_to_the_topmost_marked_ancestor() {
        let p = page(&OitModel::default());
        let i = p.nodes.iter().position(|n| n.act.as_deref() == Some("F|b:FLT FOLDER#0")).unwrap();
        let mut boxes = vec![Rect::default(); p.nodes.len()];
        let mut up = p.nodes[i].parent;
        while let Some(a) = up {
            boxes[a] = Rect::new(0.0, 0.0, 1333.0, 1000.0);
            up = p.nodes[a].parent;
        }
        boxes[i] = Rect::new(10.0, 10.0, 100.0, 40.0);
        if let Some(k) = p.nodes.iter().position(|n| n.parent == Some(i)) {
            boxes[k] = Rect::new(12.0, 12.0, 50.0, 20.0);
        }
        let mut nodes = p.nodes.clone();
        for n in nodes.iter_mut() {
            n.hidden = false;
        }
        assert_eq!(hit::hit(&nodes, &boxes, 20.0, 20.0).as_deref(), Some("F|b:FLT FOLDER#0"));
    }

    #[test]
    fn identical_snapshots_compare_equal() {
        let p = page(&OitModel::default());
        let (same, total, first) = compare(&p.html, &p.html);
        assert_eq!(same, total);
        assert!(first.is_none());
    }

    #[test]
    fn ofp_plan_sections_follow_bookmarks() {
        let html = "<div x>HEAD<!--BKMK///Fuel///0-->A&amp;B<br/>C<!--BKMK///Route///1-->R</div>";
        let s = ofp::plan_sections(html);
        assert_eq!(s.iter().map(|x| x.title.as_str()).collect::<Vec<_>>(), ["HEADER", "FUEL", "ROUTE"]);
        assert_eq!(s[1].text.as_deref(), Some("A&B\nC"));
        assert_eq!(s[2].level, 1);
        let e = ofp::parse(r#"{"fetch":{"status":"Error: no flight"}}"#).unwrap_err();
        assert_eq!(e, "SIMBRIEF: ERROR: NO FLIGHT");
    }

    #[test]
    fn runway_ends_and_dates() {
        let apt: serde_json::Value = serde_json::json!({"runways":[{"designation":"16-34","designatorCharPrimary":1,"designatorCharSecondary":2,"length":3000.0,"direction":160.0,"width":45.0,"primaryElevation":10.0,"secondaryElevation":13.0}]});
        let r = data::runway_ends(&apt);
        assert_eq!(r.iter().map(|x| x.ident.as_str()).collect::<Vec<_>>(), ["16L", "34R"]);
        assert_eq!(r[1].bearing_true, 340.0);
        assert!((r[0].slope_percent - 0.1).abs() < 1e-9);
        assert_eq!(ofp::civil_from_days(ofp::days_from_civil(2026, 10, 22)), (2026, 10, 22));
    }

    #[test]
    fn oooi_out_then_off() {
        let mut o = data::Oooi::default();
        let mut i = data::OooiInputs { zulu_ms: 1000.0, park_brake: true, on_ground: true, fob_kg: Some(9.0), door_open: 0.0, airspeed: 0.0, hyd_pressurised: true };
        o.update(&i);
        i.park_brake = false;
        i.zulu_ms = 2000.0;
        o.update(&i);
        assert_eq!(o.out.0, Some(2000.0));
        i.on_ground = false;
        for k in 0..30 {
            i.zulu_ms = 3000.0 + k as f64 * 1000.0;
            o.update(&i);
        }
        assert_eq!(o.off.0, Some(3000.0));
    }
}

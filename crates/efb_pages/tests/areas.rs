use efb_pages::docs::{docs_page, with_ecl, DocsData, DocsState};
use efb_pages::failures::{window, Window};
use efb_pages::ground::format_value;
use efb_pages::{EfbModel, Kid, Node};
use fws_ecl::mirror::{ChecklistMirror, EclMirror, ItemMirror};

fn count(n: &Node, cls: &str) -> usize {
    let own = usize::from(n.cls.iter().any(|c| c == cls));
    own + n.kids.iter().map(|k| if let Kid::Node(c) = k { count(c, cls) } else { 0 }).sum::<usize>()
}

fn item(name: &str, headline: bool, checked: bool) -> ItemMirror {
    ItemMirror { name: name.into(), value: if headline { String::new() } else { "SET".into() }, level: 0, headline, sensed: false, checked, sensed_now: None }
}

#[test]
fn checklist_rows_follow_the_ecl_mirror() {
    let ecl = EclMirror {
        ecl_shown: true,
        open_checklist_id: 1,
        open_efb_page_id: Some("n:0".into()),
        selected_item: None,
        selected_special: None,
        checklists: vec![ChecklistMirror {
            id: 1,
            efb_page_id: "n:0".into(),
            title: "COCKPIT PREPARATION".into(),
            in_menu: true,
            completed: false,
            deferred: false,
            items: vec![item("HEAD", true, false), item("A", false, true), item("B", false, false), item("C", false, false)],
        }],
    };
    let data = with_ecl(&DocsData::bundled(), &ecl);
    let mut st = DocsState { sel: "n:0".into(), ..Default::default() };
    st.apply_ecl(&ecl);
    let t = docs_page(&data, &EfbModel::default(), &st);
    assert_eq!(count(&t, "qd-sub"), 1);
    assert_eq!(count(&t, "qd-line"), 3);
    assert_eq!(count(&t, "done"), 1);
    assert_eq!(count(&t, "current"), 1);
}

#[test]
fn window_clamps_and_covers_the_viewport() {
    assert_eq!(window(923, 0.0, None, 39.25), Window { first: 0, end: 923, n: 923 });
    assert_eq!(window(923, 3819.0, Some(813.0), 39.25), Window { first: 97, end: 120, n: 923 });
    assert_eq!(window(10, 5000.0, Some(813.0), 39.25), Window { first: 0, end: 10, n: 10 });
    let w = window(923, 1e9, Some(813.0), 39.25);
    assert_eq!(w.end, 923);
    assert!(w.first as f64 * 39.25 <= 923.0 * 39.25 - 813.0);
}

#[test]
fn study_values_format_like_the_page() {
    assert_eq!(format_value(0.0, "num", 3, ""), "0.000");
    assert_eq!(format_value(1.0, "lamp", 0, ""), "ON");
    assert_eq!(format_value(f64::from(1.5f32.to_bits()) + 3.0 * 4294967296.0, "num", 1, "kt"), "1.5 kt");
    assert_eq!(format_value(f64::from(1.5f32.to_bits()), "arinc", 1, ""), "1.5 FW");
}

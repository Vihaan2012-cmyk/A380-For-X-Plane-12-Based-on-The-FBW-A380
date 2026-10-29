use efb_pages::home::home_page;
use efb_pages::model::{ChecklistRow, EfbModel};
use efb_pages::settings::{settings_page, SettingsTable};
use efb_pages::{Kid, Node};

fn count(n: &Node, cls: &str) -> usize {
    let own = usize::from(n.cls.iter().any(|c| c == cls));
    own + n.kids.iter().map(|k| if let Kid::Node(c) = k { count(c, cls) } else { 0 }).sum::<usize>()
}

#[test]
fn home_without_ofp_has_the_empty_card_and_the_checklist_rows() {
    let m = EfbModel {
        offline: true,
        checklists: vec![ChecklistRow { title: "A".into(), done: 1, total: 1 }, ChecklistRow { title: "B".into(), done: 0, total: 3 }, ChecklistRow { title: "C".into(), done: 0, total: 2 }],
        ..Default::default()
    };
    let t = home_page(&m);
    assert_eq!(t.id.as_deref(), Some("home-content"));
    assert_eq!(count(&t, "home-empty"), 1);
    assert_eq!(count(&t, "home-rem-row"), 3);
    assert_eq!(count(&t, "done"), 1);
    assert_eq!(count(&t, "next"), 1);
}

#[test]
fn settings_marks_only_the_selected_category_visible() {
    let table = SettingsTable::bundled();
    let m = EfbModel { category: 1, ..Default::default() };
    let t = settings_page(&table, &m);
    let rows = count(&t, "st-row");
    let off = count(&t, "st-off");
    assert_eq!(rows, 121);
    assert_eq!(rows - off, table.categories[1].items.len());
    assert_eq!(count(&t, "st-cat"), 8);
}

#[test]
fn search_overrides_the_category() {
    let table = SettingsTable::bundled();
    let m = EfbModel { search: "weight unit".into(), ..Default::default() };
    let t = settings_page(&table, &m);
    assert_eq!(count(&t, "st-row") - count(&t, "st-off"), 1);
    assert_eq!(count(&t, "active"), 0);
}

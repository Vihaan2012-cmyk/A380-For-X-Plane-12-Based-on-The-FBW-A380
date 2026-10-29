use efb_pages::settings::SettingsTable;

#[test]
fn bundled_table_carries_ids() {
    let t = SettingsTable::bundled();
    let item = t.categories.iter().flat_map(|c| c.items.iter()).find(|i| i.key.as_deref() == Some("xphfbw.cabinVisible")).unwrap();
    assert_eq!(item.id.as_deref(), Some("cabin-visible"));
    assert_eq!(t.categories.len(), 8);
    assert_eq!(t.categories.iter().map(|c| c.items.len()).sum::<usize>(), 121);
}

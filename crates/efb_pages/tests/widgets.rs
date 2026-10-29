use efb_pages::failures::window;
use efb_pages::fap::{fap_content, FapView};
use efb_pages::js::select;
use efb_pages::settings::{clamp_step, settings_page, SettingsTable};
use efb_pages::{EfbModel, Kid, Node};

fn count(n: &Node, cls: &str) -> usize {
    let own = usize::from(n.cls.iter().any(|c| c == cls));
    own + n.kids.iter().map(|k| if let Kid::Node(c) = k { count(c, cls) } else { 0 }).sum::<usize>()
}

fn collect<'a>(n: &'a Node, cls: &str, out: &mut Vec<&'a Node>) {
    if n.cls.iter().any(|c| c == cls) {
        out.push(n);
    }
    for k in &n.kids {
        if let Kid::Node(c) = k {
            collect(c, cls, out);
        }
    }
}

fn text_of(n: &Node) -> String {
    n.kids.iter().map(|k| match k {
        Kid::Text { text } => text.clone(),
        Kid::Node(c) => text_of(c),
    }).collect()
}

fn row_of<'a>(page: &'a Node, label: &str) -> &'a Node {
    let mut rows = Vec::new();
    collect(page, "st-row", &mut rows);
    rows.into_iter().find(|r| {
        let mut names = Vec::new();
        collect(r, "st-name", &mut names);
        names.iter().any(|n| text_of(n) == label)
    }).unwrap()
}

fn input_value(page: &Node, label: &str, tag: &str) -> Option<String> {
    fn find(n: &Node, tag: &str) -> Option<Option<String>> {
        if n.tag == tag {
            return Some(n.value.clone());
        }
        n.kids.iter().find_map(|k| if let Kid::Node(c) = k { find(c, tag) } else { None })
    }
    find(row_of(page, label), tag).flatten()
}

fn model(pairs: &[(&str, &str)]) -> EfbModel {
    let mut m = EfbModel::default();
    for (k, v) in pairs {
        m.settings.insert(k.to_string(), v.to_string());
    }
    m
}

#[test]
fn clamp_step_snaps_without_float_noise() {
    assert_eq!(clamp_step(0.35, 0.0, 1.0, 0.05), 0.35);
    assert_eq!(clamp_step(0.7, 0.0, 1.0, 0.05), 0.7);
    assert_eq!(clamp_step(42.0, 0.0, 100.0, 5.0), 40.0);
    assert_eq!(clamp_step(-5.0, 0.0, 100.0, 5.0), 0.0);
    assert_eq!(clamp_step(500.0, 0.0, 100.0, 5.0), 100.0);
    assert_eq!(clamp_step(11.0, 0.0, 11.0, 3.0), 9.0);
    assert_eq!(clamp_step(0.34, 0.1, 1000.0, 0.1), 0.3);
    assert_eq!(clamp_step(7.3, 0.0, 10.0, 0.0), 7.3);
}

#[test]
fn number_fields_normalise_to_min_max_step() {
    let t = SettingsTable::bundled();
    let item = |k: &str| t.categories.iter().flat_map(|c| c.items.iter()).find(|i| i.key.as_deref() == Some(k)).unwrap();
    let pax = item("xphfbw.stdPaxWeightKg");
    assert_eq!(pax.normalize("200").as_deref(), Some("150"));
    assert_eq!(pax.normalize(" 49 ").as_deref(), Some("50"));
    assert_eq!(pax.normalize("83.6").as_deref(), Some("84"));
    assert_eq!(pax.normalize("abc"), None);
    assert_eq!(pax.normalize(""), None);
    assert_eq!(pax.normalize("NaN"), None);
    assert_eq!(item("xphfbw.failureRate").normalize("0").as_deref(), Some("0.1"));
    assert_eq!(item("xphfbw.cabinBrightness").normalize("0.35").as_deref(), Some("0.35"));
    assert_eq!(item("CONFIG_THR_RED_ALT").normalize("1504").as_deref(), Some("1500"));
    assert_eq!(item("xphfbw.nadp").normalize("5"), None);
}

#[test]
fn number_and_range_widgets_show_the_clamped_value() {
    let t = SettingsTable::bundled();
    let p = settings_page(&t, &model(&[("xphfbw.stdPaxWeightKg", "500"), ("xphfbw.stdBagWeightKg", "junk"), ("xphfbw.cabinBrightness", "0.35000000000000003"), ("SOUND_LEVEL_MASTER", "130")]));
    assert_eq!(input_value(&p, "Standard passenger weight", "input").as_deref(), Some("150"));
    assert_eq!(input_value(&p, "Standard bag weight", "input").as_deref(), Some(""));
    assert_eq!(input_value(&p, "Cabin brightness", "input").as_deref(), Some("0.35"));
    assert_eq!(input_value(&p, "Master", "input").as_deref(), Some("100"));
}

#[test]
fn show_rules_follow_the_controlling_setting() {
    let t = SettingsTable::bundled();
    let hidden = |p: &Node, l: &str| row_of(p, l).inline.as_deref() == Some("display: none;");
    let off = settings_page(&t, &model(&[("xphfbw.deratesEnabled", "false")]));
    let on = settings_page(&t, &model(&[("xphfbw.deratesEnabled", "true")]));
    assert!(hidden(&off, "Takeoff derate levels"));
    assert!(!hidden(&on, "Takeoff derate levels"));
    let co = settings_page(&t, &model(&[("xphfbw.copilotMode", "true")]));
    assert!(hidden(&co, "First officer callouts"));
    assert!(!hidden(&settings_page(&t, &model(&[("xphfbw.copilotMode", "false")])), "First officer callouts"));
    let chained = settings_page(&t, &model(&[("xphfbw.copilotMode", "false"), ("xphfbw.copilotCallouts", "false")]));
    assert!(hidden(&chained, "First officer SOP callouts"));
    let light = settings_page(&t, &model(&[("LIGHT_PRESET_AUTOLOAD", "1")]));
    assert!(!hidden(&light, "Day preset"));
    assert!(hidden(&settings_page(&t, &model(&[("LIGHT_PRESET_AUTOLOAD", "0")])), "Day preset"));
}

#[test]
fn out_of_range_category_falls_back_to_the_last_one() {
    let t = SettingsTable::bundled();
    let last = t.categories.len() - 1;
    let m = EfbModel { category: 99, ..Default::default() };
    let p = settings_page(&t, &m);
    assert_eq!(count(&p, "st-row") - count(&p, "st-off"), t.categories[last].items.len());
    let mut cats = Vec::new();
    collect(&p, "st-cat", &mut cats);
    assert!(cats[last].cls.iter().any(|c| c == "active"));
    assert_eq!(count(&p, "active"), 1);
}

#[test]
fn virtual_window_survives_a_zero_row_height() {
    assert_eq!(window(50, 100.0, Some(300.0), 0.0).end, 50);
    assert_eq!(window(50, f64::NAN, Some(300.0), 39.25).first, 0);
}

#[test]
fn select_shows_the_current_value_even_when_it_is_not_listed() {
    let opts = [("0", "Off"), ("5", "Every 5 min")];
    assert_eq!(select(&opts, "x", "5").data["label"], "Every 5 min");
    assert_eq!(select(&opts, "x", "").data["label"], "Off");
    assert_eq!(select(&opts, "x", "7").data["label"], "7");
}

#[test]
fn fap_sliders_stay_inside_their_range() {
    let t = SettingsTable::bundled();
    let v = FapView { page: "audio".into(), ok: true, ..Default::default() };
    let p = fap_content(&v, &model(&[("SOUND_LEVEL_CABIN", "junk")]), &t);
    let mut sl = Vec::new();
    collect(&p, "fap-range", &mut sl);
    assert_eq!(sl[0].value.as_deref(), Some("100"));
    let p = fap_content(&v, &model(&[("SOUND_LEVEL_CABIN", "250")]), &t);
    let mut sl = Vec::new();
    collect(&p, "fap-range", &mut sl);
    assert_eq!(sl[0].value.as_deref(), Some("100"));
    let v = FapView { page: "lights".into(), ok: true, ..Default::default() };
    let p = fap_content(&v, &model(&[("xphfbw.cabinBrightness", "-2")]), &t);
    let mut sl = Vec::new();
    collect(&p, "fap-range", &mut sl);
    assert_eq!(sl[0].value.as_deref(), Some("0"));
}

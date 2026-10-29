use std::path::PathBuf;

use efb_pages::model::StatusModel;
use serde_json::{Map, Value};

use super::doc::NodeInfo;
use super::engine::{Backend, Efb};
use super::hit::{self, Rect};
use super::scroll::Scrolls;
use super::text::{key_of, vk, Key, Outcome, TextEdit};

struct FileBackend {
    path: PathBuf,
    captured: bool,
}

impl FileBackend {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("efb-native-{name}-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        FileBackend { path, captured: false }
    }
}

impl Backend for FileBackend {
    fn load_settings(&mut self) -> Map<String, Value> {
        std::fs::read_to_string(&self.path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }
    fn save_settings(&mut self, values: &Map<String, Value>) -> Result<(), String> {
        let mut all = self.load_settings();
        all.extend(values.clone());
        std::fs::write(&self.path, Value::Object(all).to_string()).map_err(|e| e.to_string())
    }
    fn route(&mut self, _m: &str, _p: &str, _b: &str) -> Option<(u16, String)> {
        None
    }
    fn app_action(&mut self, _n: &str) -> Option<String> {
        None
    }
    fn fetch(&mut self, _u: &str) -> Option<(u16, String)> {
        None
    }
    fn status(&mut self) -> StatusModel {
        StatusModel::default()
    }
    fn clipboard_get(&mut self) -> String {
        "pasted".into()
    }
    fn clipboard_set(&mut self, _t: &str) {}
    fn keyboard_capture(&mut self, on: bool) {
        self.captured = on;
    }
}

fn info(parent: Option<usize>, act: Option<&str>) -> NodeInfo {
    NodeInfo { parent, tag: "div".into(), act: act.map(str::to_string), ..Default::default() }
}

#[test]
fn hit_finds_the_topmost_element_and_its_act() {
    let mut nodes = vec![info(None, None), info(Some(0), Some("tab:home")), info(Some(1), None), info(Some(0), Some("overlay"))];
    let boxes = vec![Rect::new(0., 0., 100., 100.), Rect::new(10., 10., 50., 50.), Rect::new(20., 20., 10., 10.), Rect::new(0., 0., 100., 100.)];
    nodes[3].hidden = true;
    let h = hit::hit(&nodes, &boxes, &|_| 0.0, 25., 25.).unwrap();
    assert_eq!(h.node, 2);
    assert_eq!(h.act, Some((1, "tab:home".into())));
    nodes[3].hidden = false;
    nodes[3].layer = 50;
    assert_eq!(hit::hit(&nodes, &boxes, &|_| 0.0, 25., 25.).unwrap().act, Some((3, "overlay".into())));
}

#[test]
fn hit_honours_clips_and_scroll_offsets() {
    let mut nodes = vec![info(None, None), info(Some(0), None), info(Some(1), Some("row:a")), info(Some(1), Some("row:b"))];
    nodes[1].clips = true;
    nodes[1].scroller = Some("list".into());
    let boxes = vec![Rect::new(0., 0., 500., 500.), Rect::new(0., 0., 100., 100.), Rect::new(0., 0., 100., 80.), Rect::new(0., 80., 100., 80.)];
    let at = |top: f64, y: f64| hit::hit(&nodes, &boxes, &|_| top, 50., y).and_then(|h| h.act).map(|a| a.1);
    assert_eq!(at(0., 90.), Some("row:b".into()));
    assert_eq!(at(0., 150.), None, "row b below the clip is not hit");
    assert_eq!(at(70., 50.), Some("row:b".into()), "scrolled up by 70 px");
    assert_eq!(hit::max_scroll(&nodes, &boxes, 1), 60.);
    assert_eq!(hit::hit(&nodes, &boxes, &|_| 0., 50., 50.).unwrap().scroller, Some((1, "list".into())));
}

#[test]
fn scroll_clamps_and_flings_decay() {
    let mut s = Scrolls::default();
    s.set_max("a", 300.);
    assert!(s.by("a", 500.));
    assert_eq!(s.top("a"), 300.);
    s.set("a", 0.);
    s.fling("a", 2.0);
    let mut last = 0.;
    for _ in 0..200 {
        s.tick(16.);
        assert!(s.top("a") >= last);
        last = s.top("a");
    }
    assert!(!s.moving());
    assert!(last > 100. && last <= 300., "{last}");
    assert!(s.to_json().starts_with("{\"a\":"));
}

#[test]
fn text_editing() {
    let mut t = TextEdit::new("abc", false, Some(5));
    assert_eq!(t.apply(Key::Left), Outcome::Moved);
    assert_eq!(t.apply(Key::Char('X')), Outcome::Edited);
    assert_eq!(t.display(true), "abX|c");
    assert_eq!(t.apply(Key::Backspace), Outcome::Edited);
    assert_eq!(t.apply(Key::Delete), Outcome::Edited);
    assert_eq!(t.text, "ab");
    assert_eq!(t.apply(Key::Paste("12345".into())), Outcome::Edited);
    assert_eq!(t.text, "ab123", "max length 5");
    t.apply(Key::SelectAll);
    t.apply(Key::Char('z'));
    assert_eq!((t.text.as_str(), t.caret), ("z", 1));
    let mut n = TextEdit::new("", true, None);
    n.apply(Key::Paste("1a2.5".into()));
    assert_eq!(n.text, "12.5");
    assert_eq!(n.apply(Key::Enter), Outcome::Commit);
    assert_eq!(n.apply(Key::Escape), Outcome::Release);
    let clip = || "c".to_string();
    assert_eq!(key_of(0, vk::LEFT, vk::DOWN_FLAG, &clip), Some(Key::Left));
    assert_eq!(key_of('%' as u32, vk::LEFT, vk::DOWN_FLAG, &clip), Some(Key::Left));
    assert_eq!(key_of('a' as u32, vk::A, vk::DOWN_FLAG, &clip), Some(Key::Char('a')));
    assert_eq!(key_of('v' as u32, vk::V, vk::DOWN_FLAG | vk::CTRL_FLAG, &clip), Some(Key::Paste("c".into())));
    assert_eq!(key_of('a' as u32, vk::A, vk::UP_FLAG, &clip), None);
}

fn page(efb: &mut Efb<FileBackend>) -> String {
    efb.tick(1.0);
    efb.document().map(str::to_string).unwrap_or_default()
}

fn click(efb: &mut Efb<FileBackend>, act: &str) {
    let nodes = efb.nodes().to_vec();
    let idx = hit::find_act(&nodes, act).unwrap_or_else(|| panic!("no element with act {act}"));
    let mut boxes = vec![Rect::default(); nodes.len()];
    boxes[0] = Rect::new(0., 0., 1430., 1000.);
    let mut p = Some(idx);
    while let Some(i) = p {
        boxes[i] = Rect::new(100., 100., 40., 20.);
        p = nodes[i].parent.filter(|&q| q != 0);
    }
    efb.set_boxes(boxes);
    assert!(efb.pointer(110., 105., 0, 0));
    assert!(efb.pointer(110., 105., 2, 0));
}

#[test]
fn settings_toggle_round_trips_through_the_settings_file() {
    let mut efb = Efb::new(FileBackend::new("toggle"));
    efb.locked = false;
    efb.activate("tab:settings");
    let html = page(&mut efb);
    assert!(html.contains("data-act=\"toggle:xphfbw.encoderAccel\""));
    assert!(html.contains("data-cs=\""));
    click(&mut efb, "toggle:xphfbw.encoderAccel");
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(&efb.backend.path).unwrap()).unwrap();
    assert_eq!(saved["xphfbw.encoderAccel"], Value::Bool(true));
    assert!(saved["xphfbw.userKeys"].as_str().unwrap().contains("xphfbw.encoderAccel"));
    let path = efb.backend.path.clone();
    let mut again = Efb::new(FileBackend { path: path.clone(), captured: false });
    again.locked = false;
    again.activate("tab:settings");
    let html = page(&mut again);
    let at = html.find("data-act=\"toggle:xphfbw.encoderAccel\"").unwrap();
    let input = &html[at..at + html[at..].find("</label>").unwrap()];
    assert!(input.contains(" checked=\"\""), "the reloaded page shows the switch on");
    let _ = std::fs::remove_file(path);
}

#[test]
fn text_field_focus_keyboard_and_commit() {
    let mut efb = Efb::new(FileBackend::new("text"));
    efb.locked = false;
    efb.activate("tab:settings");
    page(&mut efb);
    efb.activate("focus:set:xphfbw.registration");
    assert!(efb.backend.captured);
    let html = page(&mut efb);
    assert!(html.contains("id=\"efbn-kb\""), "on-screen keyboard shown");
    efb.activate("kb:char:70");
    assert!(efb.key('-' as u32, 0xBD, vk::DOWN_FLAG));
    efb.activate("kb:char:88");
    assert!(efb.key(27, vk::ESCAPE, vk::DOWN_FLAG), "Esc releases");
    assert!(!efb.backend.captured);
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(&efb.backend.path).unwrap()).unwrap();
    assert_eq!(saved["xphfbw.registration"], Value::String("F-X".into()));
    assert!(!efb.key('a' as u32, vk::A, vk::DOWN_FLAG), "keys go back to X-Plane");
    let _ = std::fs::remove_file(&efb.backend.path);
}

#[test]
fn drag_scrolls_instead_of_tapping_and_wheel_scrolls() {
    let mut efb = Efb::new(FileBackend::new("drag"));
    efb.locked = false;
    efb.activate("tab:settings");
    page(&mut efb);
    let nodes = efb.nodes().to_vec();
    let inside = |i: usize, list: usize| {
        let mut p = nodes[i].parent;
        while let Some(q) = p {
            if q == list {
                return true;
            }
            p = nodes[q].parent;
        }
        false
    };
    let (list, row) = (0..nodes.len())
        .filter(|&l| nodes[l].scroller.is_some() && nodes[l].id.is_none())
        .find_map(|l| (l + 1..nodes.len()).find(|&i| nodes[i].act.as_deref().is_some_and(|a| a.starts_with("toggle:")) && inside(i, l)).map(|r| (l, r)))
        .expect("a settings scroller holding a switch");
    let id = nodes[list].scroller.clone().unwrap();
    let mut boxes = vec![Rect::default(); nodes.len()];
    let mut p = nodes[list].parent;
    while let Some(i) = p {
        boxes[i] = Rect::new(0., 0., 1430., 1000.);
        p = nodes[i].parent;
    }
    boxes[list] = Rect::new(200., 100., 800., 500.);
    let mut p = nodes[row].parent;
    while let Some(i) = p.filter(|&i| i != list) {
        boxes[i] = Rect::new(200., 100., 800., 1800.);
        p = nodes[i].parent;
    }
    boxes[row] = Rect::new(250., 150., 100., 30.);
    efb.set_boxes(boxes);
    let before = std::fs::read_to_string(&efb.backend.path).unwrap_or_default();
    efb.tick(100.);
    efb.pointer(260., 160., 0, 0);
    efb.tick(116.);
    efb.pointer(260., 120., 1, 0);
    efb.tick(132.);
    efb.pointer(260., 80., 1, 0);
    efb.pointer(260., 80., 2, 0);
    assert_eq!(efb.scrolls.top(&id), 80.);
    assert_eq!(std::fs::read_to_string(&efb.backend.path).unwrap_or_default(), before, "a drag is not a tap");
    efb.tick(400.);
    assert!(efb.scrolls.top(&id) > 80., "momentum after release");
    let t = efb.scrolls.top(&id);
    efb.pointer(300., 300., 4, 1);
    assert_eq!(efb.scrolls.top(&id), (t - 48.).max(0.));
    assert!(efb.scroll_json().contains(&id));
}

#[test]
fn pseudo_elements_become_children() {
    let sheet = super::style::prepare(".st-switch span::after{content:'';position:absolute;} .x:before{content:'\\2713'}");
    assert!(sheet.css.contains(".st-switch span > efbn-after"));
    assert_eq!(sheet.pseudo.len(), 2);
    assert_eq!(sheet.pseudo[1].content.as_deref(), Some("\u{2713}"));
}

#[test]
fn selects_get_the_menulist_look_and_the_app_list() {
    let mut efb = Efb::new(FileBackend::new("select"));
    efb.locked = false;
    efb.activate("tab:settings");
    let html = page(&mut efb);
    assert!(html.contains("class=\"st-sel efbn-sel\"") || html.contains("efbn-sel\""), "closed select rebuilt");
    assert!(html.contains("<div") && html.contains("class=\"efbn-sel-a\""), "arrow drawn");
    efb.activate("select:zoom");
    let html = page(&mut efb);
    assert!(html.contains("class=\"select-list no-drag-scroll\""));
    assert!(html.contains("class=\"select-item on\""));
    assert_eq!(super::engine::select_list_style(Some(Rect::new(100., 900., 200., 40.)), 3), "left:100px;width:200px;max-height:107px;top:793px");
}

#[test]
#[ignore]
fn page_build_times() {
    let _g = crate::failures::tests::serial();
    let mut efb = Efb::new(FileBackend::new("times"));
    for tab in ["home", "study", "failures", "breakers", "components", "maintenance", "wear", "ground", "pushback", "loadsheet", "checklists", "settings", "performance", "fap", "atc"] {
        efb.activate(&format!("tab:{tab}"));
        let t = std::time::Instant::now();
        efb.document();
        let first = t.elapsed().as_secs_f64() * 1e3;
        efb.activate("dismiss");
        let t = std::time::Instant::now();
        efb.document();
        eprintln!("{tab:12} first {first:7.1} ms, unchanged rebuild {:6.1} ms, {} elements", t.elapsed().as_secs_f64() * 1e3, efb.nodes().len());
    }
}

#[test]
#[ignore]
fn data_source_times() {
    let _g = crate::failures::tests::serial();
    use crate::study::web;
    let time = |name: &str, f: &dyn Fn() -> String| {
        let t = std::time::Instant::now();
        let s = f();
        eprintln!("{name:18} {:7.1} ms {:8} bytes", t.elapsed().as_secs_f64() * 1e3, s.len());
    };
    for _ in 0..2 {
        time("pages_json", &web::pages_json);
        time("failures_json", &web::failures_json);
        time("breakers_json", &web::breakers_json);
        time("components_json", &web::components_json);
        time("maintenance_json", &web::maintenance_json);
        time("wear_json", &web::wear_json);
    }
}

#[test]
#[ignore]
fn fap_first_button_style() {
    let _g = crate::failures::tests::serial();
    let mut efb = Efb::new(FileBackend::new("fap"));
    efb.activate("tab:fap");
    let html = efb.document().unwrap().to_string();
    std::fs::write(std::env::var("FAP_DUMP").unwrap_or("fap.html".into()), &html).unwrap();
    let i = html.find("CABIN STATUS").unwrap();
    let start = html[..i].rfind("<button").unwrap();
    eprintln!("{}", &html[start..i + 20]);
}

#[test]
#[ignore]
fn failures_diagram_dump() {
    let _g = crate::failures::tests::serial();
    let mut efb = Efb::new(FileBackend::new("failures"));
    efb.activate("tab:failures");
    efb.activate("fail:open:part-separation");
    let html = efb.document().unwrap().to_string();
    std::fs::write(std::env::var("FAIL_DUMP").unwrap_or("failures.html".into()), &html).unwrap();
    eprintln!("ff-family {} ff-diagram {} svg {}", html.matches("ff-family").count(), html.matches("ff-diagram").count(), html.matches("<svg").count());
}

use efb_map::amdb;
use efb_map::controller::{OwnShip, Pointer};
use efb_map::declutter::{declutter, LabelRequest, Rect};
use efb_map::scene::{Airport, Navaid, NavaidKind, Route, RoutePoint, SceneInput};
use efb_map::traffic::Age;
use efb_map::weather::{grid_shapes, levels_from_samples, sample_grid, WxSample};
use efb_map::*;

fn close(a: LatLon, b: LatLon, tol_deg: f64) -> bool {
    (a.lat - b.lat).abs() < tol_deg && (projection::wrap_lon(a.lon - b.lon)).abs() < tol_deg
}

fn own(lat: f64, lon: f64, trk: f64) -> OwnShip {
    OwnShip { pos: LatLon::new(lat, lon), heading_deg: trk, track_deg: trk, gs_kt: 250.0, alt_ft: 10_000.0, on_ground: false }
}

#[test]
fn projection_round_trips_at_every_rotation_and_zoom() {
    let centres = [LatLon::new(51.47, -0.45), LatLon::new(-33.94, 151.17), LatLon::new(25.25, 55.36), LatLon::new(64.0, 179.8), LatLon::new(0.0, 0.0)];
    for c in centres {
        for zoom in [3.0, 7.5, 12.0, 16.0] {
            for rot in [0.0, 37.0, 90.0, 211.0] {
                let mut v = Viewport::new(c, zoom, 1100.0, 860.0);
                v.rotation_deg = rot;
                let (cx, cy) = v.project(c);
                assert!((cx - 550.0).abs() < 1e-6 && (cy - 430.0).abs() < 1e-6);
                for (sx, sy) in [(0.0, 0.0), (1100.0, 0.0), (300.0, 700.0), (1100.0, 860.0)] {
                    let ll = v.unproject(sx, sy);
                    if ll.lat.abs() > 85.0 {
                        continue;
                    }
                    let (bx, by) = v.project(ll);
                    assert!((bx - sx).abs() < 1e-4 && (by - sy).abs() < 1e-4, "{c:?} z{zoom} r{rot}: ({sx},{sy}) -> ({bx},{by})");
                }
            }
        }
    }
}

#[test]
fn track_up_puts_the_track_direction_at_the_top() {
    let c = LatLon::new(40.0, -74.0);
    let mut v = Viewport::new(c, 9.0, 800.0, 800.0);
    v.rotation_deg = 90.0;
    let (x, y) = v.project(LatLon::new(40.0, -73.8));
    assert!((x - 400.0).abs() < 1.0 && y < 400.0);
    v.rotation_deg = 0.0;
    let (x, y) = v.project(LatLon::new(40.2, -74.0));
    assert!((x - 400.0).abs() < 1e-6 && y < 400.0);
}

#[test]
fn antimeridian_projects_continuously() {
    let v = Viewport::new(LatLon::new(0.0, 179.9), 8.0, 800.0, 800.0);
    let (x, _) = v.project(LatLon::new(0.0, -179.9));
    assert!(x > 400.0 && x < 800.0, "{x}");
    assert!(close(v.unproject(x, 400.0), LatLon::new(0.0, -179.9), 1e-6));
    assert!(!v.tiles(8).is_empty());
}

#[test]
fn zoom_about_keeps_the_anchor_fixed_and_tiles_cover_the_view() {
    let mut v = Viewport::new(LatLon::new(48.35, 11.78), 10.0, 1000.0, 700.0);
    v.rotation_deg = 30.0;
    let anchor = v.unproject(800.0, 150.0);
    v.zoom_about(800.0, 150.0, 1.5);
    assert!((v.zoom - 11.5).abs() < 1e-9);
    let (x, y) = v.project(anchor);
    assert!((x - 800.0).abs() < 1e-3 && (y - 150.0).abs() < 1e-3);
    let tiles = v.tiles(11);
    for corner in v.corners() {
        assert!(tiles.contains(&TileKey::of(corner, 11)));
    }
    let (nw, se) = TileKey::of(LatLon::new(48.35, 11.78), 11).bounds();
    assert!(nw.lat > 48.35 && se.lat < 48.35 && nw.lon < 11.78 && se.lon > 11.78);
}

#[test]
fn scale_bar_picks_a_round_distance() {
    let v = Viewport::new(LatLon::new(0.0, 0.0), 8.0, 1000.0, 800.0);
    let (px, lbl) = projection::scale_bar(&v, 180.0);
    assert!(px <= 180.0 && px > 60.0, "{px}");
    assert!(lbl.ends_with("NM"));
}

#[test]
fn declutter_drops_overlaps_by_priority_and_keeps_in_bounds() {
    let reqs = vec![
        LabelRequest { x: 100.0, y: 100.0, text: "LOW".into(), class: "a".into(), priority: 1 },
        LabelRequest { x: 101.0, y: 100.0, text: "HIGH".into(), class: "b".into(), priority: 9 },
        LabelRequest { x: 100.0, y: 101.0, text: "MID".into(), class: "c".into(), priority: 5 },
        LabelRequest { x: 102.0, y: 102.0, text: "AAAAAAAAAA".into(), class: "c".into(), priority: 4 },
        LabelRequest { x: 101.0, y: 101.0, text: "BBBBBBBBBB".into(), class: "c".into(), priority: 3 },
        LabelRequest { x: 99.0, y: 99.0, text: "CCCCCCCCCC".into(), class: "c".into(), priority: 2 },
        LabelRequest { x: 798.0, y: 2.0, text: "EDGE".into(), class: "d".into(), priority: 0 },
    ];
    let (placed, dropped) = declutter(reqs, &[Rect::around(100.0, 100.0, 4.0)], 800.0, 600.0);
    assert_eq!(placed.len() + dropped, 7);
    assert!(dropped >= 1);
    assert_eq!(placed[0].text, "HIGH");
    assert!(!placed.iter().any(|p| p.text == "LOW") || dropped > 1);
    for (i, a) in placed.iter().enumerate() {
        let ra = Rect { x0: a.x, y0: a.y, x1: a.x + declutter::text_size(&a.text).0, y1: a.y + declutter::LINE_H_PX };
        assert!(ra.x0 >= 0.0 && ra.y0 >= 0.0 && ra.x1 <= 800.0 && ra.y1 <= 600.0);
        assert!(!ra.overlaps(&Rect::around(100.0, 100.0, 4.0)));
        for b in &placed[i + 1..] {
            let rb = Rect { x0: b.x, y0: b.y, x1: b.x + declutter::text_size(&b.text).0, y1: b.y + declutter::LINE_H_PX };
            assert!(!ra.overlaps(&rb), "{} overlaps {}", a.text, b.text);
        }
    }
}

#[test]
fn layer_visibility_follows_zoom_and_toggles() {
    let mut t = LayerToggles::default();
    assert!(t.visible(LayerKind::Route, 3.0));
    assert!(t.visible(LayerKind::Airports, 5.0) && !t.visible(LayerKind::Airports, 14.0));
    assert!(!t.visible(LayerKind::Airways, 6.0) && t.visible(LayerKind::Airways, 9.0));
    assert!(!t.visible(LayerKind::Amdb, 12.0) && t.visible(LayerKind::Amdb, 15.0));
    assert!(!t.visible(LayerKind::Ndbs, 7.0) && t.visible(LayerKind::Ndbs, 8.5));
    assert!(t.visible(LayerKind::Vors, 6.5) && !t.labels(LayerKind::Vors, 6.5) && t.labels(LayerKind::Vors, 7.5));
    t.flip(LayerKind::Weather);
    assert!(!t.visible(LayerKind::Weather, 6.0));
    t.flip(LayerKind::Weather);
    assert!(t.visible(LayerKind::Weather, 6.0));
    t.flip(LayerKind::OwnShip);
    assert!(t.visible(LayerKind::OwnShip, 10.0));
    for k in LayerKind::ALL {
        assert_eq!(LayerKind::from_id(k.id()), Some(k));
    }
}

fn report(src: TrafficSource, id: &str, t: f64) -> TrafficReport {
    TrafficReport { source: src, id: id.into(), callsign: id.to_uppercase(), pos: LatLon::new(50.0, 8.0), alt_ft: 10_000.0, track_deg: 90.0, gs_kt: 360.0, on_ground: false, t_s: t }
}

#[test]
fn traffic_ages_extrapolates_and_expires_per_source() {
    let mut s = TrafficStore::default();
    s.upsert(report(TrafficSource::Ai, "ai1", 100.0));
    s.ingest(TrafficSource::Vatsim, vec![report(TrafficSource::Vatsim, "dlh1", 100.0), report(TrafficSource::Ivao, "x", 100.0)]);
    assert_eq!(s.len(), 2);
    s.upsert(report(TrafficSource::Vatsim, "dlh1", 90.0));
    let v = s.views(101.0);
    assert!(v.iter().all(|t| t.age == Age::Fresh));
    let dlh = v.iter().find(|t| t.report.id == "dlh1").unwrap();
    assert!(dlh.pos.lon > 8.0 && (dlh.pos.lat - 50.0).abs() < 1e-6);
    let v = s.views(105.0);
    let ai = v.iter().find(|t| t.report.id == "ai1").unwrap();
    assert_eq!(ai.age, Age::Stale);
    let at_limit = traffic::dead_reckon(LatLon::new(50.0, 8.0), 90.0, 360.0, 3.0);
    assert!(close(ai.pos, at_limit, 1e-9));
    assert_eq!(s.views(111.0).len(), 1);
    assert_eq!(s.prune(111.0), 1);
    assert_eq!(s.views(150.0)[0].age, Age::Stale);
    assert_eq!(s.prune(221.0), 1);
    assert!(s.is_empty());
}

#[test]
fn controller_taps_drags_wheels_and_toggles() {
    let mut c = MapController::new(Viewport::new(LatLon::new(51.0, 0.0), 8.0, 800.0, 600.0));
    let o = own(51.0, 0.0, 120.0);
    c.update_own(&o);
    c.pointer_down(100.0, 100.0);
    c.pointer_move(103.0, 102.0);
    assert_eq!(c.pointer_up(103.0, 102.0), Pointer::Tap(103.0, 102.0));
    assert!(c.follow);
    let grabbed = c.view.unproject(100.0, 100.0);
    c.pointer_down(100.0, 100.0);
    c.pointer_move(140.0, 130.0);
    assert_eq!(c.pointer_up(160.0, 160.0), Pointer::Dragged);
    assert!(!c.follow);
    let (x, y) = c.view.project(grabbed);
    assert!((x - 160.0).abs() < 1e-3 && (y - 160.0).abs() < 1e-3);
    c.wheel(400.0, 300.0, 4.0);
    assert!((c.view.zoom - 9.0).abs() < 1e-9);
    c.pinch(400.0, 300.0, 0.5);
    assert!((c.view.zoom - 8.0).abs() < 1e-9);
    assert!(c.act("map:orient", Some(&o)));
    assert!((c.view.rotation_deg - 120.0).abs() < 1e-9);
    assert!(c.act("map:follow", Some(&o)));
    assert!(c.follow && close(c.view.center, o.pos, 1e-12));
    assert!(c.act("map:layer:wx", None));
    assert!(!c.toggles.is_on(LayerKind::Weather));
    assert!(!c.act("map:nothing", None));
}

#[test]
fn weather_grid_merges_runs_and_colours_levels() {
    let v = Viewport::new(LatLon::new(10.0, 10.0), 8.0, 480.0, 240.0);
    let g = sample_grid(&v);
    assert_eq!(g.points.len(), g.cols * g.rows);
    let samples: Vec<WxSample> = g.points.iter().take(g.cols).map(|p| WxSample { pos: *p, level: 2 }).chain(std::iter::once(WxSample { pos: g.points[g.cols + 3], level: weather::level_from_precip(0.9) })).collect();
    let levels = levels_from_samples(&g, &v, &samples);
    let shapes = grid_shapes(&g, &levels);
    assert_eq!(shapes.len(), 2);
    assert_eq!(shapes[0].a["fill"], palette::WARN);
    assert_eq!(shapes[1].a["fill"], palette::BAD);
}

#[test]
fn scene_builds_layers_labels_and_amdb() {
    let mut v = Viewport::new(LatLon::new(25.25, 55.36), 15.0, 1000.0, 760.0);
    let toggles = LayerToggles::default();
    let data = serde_json::json!({
        "aerodromereferencepoint": {"type": "FeatureCollection", "features": [{"type": "Feature", "properties": {}, "geometry": {"type": "Point", "coordinates": [55.364, 25.253]}}]},
        "runwayelement": {"type": "FeatureCollection", "features": [{"type": "Feature", "properties": {"idrwy": "12L/30R"}, "geometry": {"type": "Polygon", "coordinates": [[[55.35, 25.25], [55.37, 25.24], [55.371, 25.241], [55.351, 25.251], [55.35, 25.25]]]}}]},
        "taxiwayguidanceline": {"type": "FeatureCollection", "features": [{"type": "Feature", "properties": {"idlin": "M1"}, "geometry": {"type": "LineString", "coordinates": [[55.355, 25.252], [55.365, 25.248]]}}]},
        "unknownlayer": {"type": "FeatureCollection", "features": []}
    });
    let apt = amdb::parse("OMDB", &data);
    assert_eq!(apt.features.len(), 2);
    assert!(apt.arp.is_some_and(|p| (p.lat - 25.253).abs() < 1e-9));
    let route = Route { points: vec![RoutePoint { ident: "OMDB".into(), pos: LatLon::new(25.25, 55.36) }, RoutePoint { ident: "DESDI".into(), pos: LatLon::new(25.5, 55.0) }], active_to: Some(1) };
    let tv = vec![traffic::TrafficView { report: report(TrafficSource::Vatsim, "uae1", 0.0), pos: LatLon::new(25.251, 55.361), age: Age::Fresh, age_s: 0.0 }];
    let own_ship = own(25.25, 55.36, 300.0);
    let input = SceneInput { own: Some(own_ship), route: Some(&route), traffic: &tv, amdb: Some(&apt), ..Default::default() };
    let s = build_scene(&v, &toggles, &input);
    let kinds: Vec<LayerKind> = s.layers.iter().map(|l| l.kind).collect();
    assert_eq!(kinds, vec![LayerKind::Amdb, LayerKind::Route, LayerKind::AiTraffic, LayerKind::OnlineTraffic, LayerKind::OwnShip]);
    assert_eq!(s.layers[0].shapes.len(), 2);
    assert!(s.layers[0].shapes[0].a["d"].ends_with('Z'));
    assert_eq!(s.counts.vatsim, 1);
    assert!(s.labels.iter().any(|l| l.text == "12L/30R"));
    assert!(s.projection.starts_with("merc;25.250000;55.360000;15.0000"));
    v.zoom = 7.0;
    let apts = vec![Airport { ident: "OMDB".into(), pos: LatLon::new(25.25, 55.36), size: 2 }, Airport { ident: "OMSJ".into(), pos: LatLon::new(25.33, 55.52), size: 1 }];
    let navs = vec![Navaid { kind: NavaidKind::Vor, ident: "DXB".into(), pos: LatLon::new(25.24, 55.40) }, Navaid { kind: NavaidKind::Ndb, ident: "XX".into(), pos: LatLon::new(25.1, 55.1) }];
    let input = SceneInput { own: Some(own_ship), route: Some(&route), airports: &apts, navaids: &navs, ..Default::default() };
    let s = build_scene(&v, &toggles, &input);
    assert!(!s.layers.iter().any(|l| l.kind == LayerKind::Amdb || l.kind == LayerKind::Ndbs));
    assert_eq!(s.counts.airports, 2);
    assert_eq!(s.counts.navaids, 1);
    let json = serde_json::to_string(&s).unwrap();
    let back: Scene = serde_json::from_str(&json).unwrap();
    assert_eq!(serde_json::to_string(&back).unwrap(), json);
}

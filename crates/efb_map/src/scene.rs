use crate::amdb::{AmdbAirport, AmdbClass};
use crate::controller::OwnShip;
use crate::declutter::{declutter, LabelRequest, PlacedLabel, Rect};
use crate::layers::{airport_min_zoom, LayerKind, LayerToggles};
use crate::palette;
use crate::projection::{scale_bar, LatLon, Viewport};
use crate::traffic::{Age, TrafficSource, TrafficView};
use crate::weather::{grid_shapes, levels_from_samples, sample_grid, WxSample};
use mfd_tree::Shape;
use serde::{Deserialize, Serialize};

pub const PROXIMATE_NM: f64 = 6.0;
pub const PROXIMATE_FT: f64 = 1_200.0;
const OFFSCREEN_MARGIN: f64 = 40.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutePoint {
    pub ident: String,
    pub pos: LatLon,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Route {
    pub points: Vec<RoutePoint>,
    pub active_to: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Airport {
    pub ident: String,
    pub pos: LatLon,
    pub size: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NavaidKind {
    Vor,
    Ndb,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Navaid {
    pub kind: NavaidKind,
    pub ident: String,
    pub pos: LatLon,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AirwaySegment {
    pub name: String,
    pub a: LatLon,
    pub b: LatLon,
}

#[derive(Clone, Debug, Default)]
pub struct SceneInput<'a> {
    pub own: Option<OwnShip>,
    pub route: Option<&'a Route>,
    pub airports: &'a [Airport],
    pub navaids: &'a [Navaid],
    pub airways: &'a [AirwaySegment],
    pub traffic: &'a [TrafficView],
    pub weather: &'a [WxSample],
    pub amdb: Option<&'a AmdbAirport>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneLayer {
    pub kind: LayerKind,
    pub shapes: Vec<Shape>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Counts {
    pub ai: usize,
    pub vatsim: usize,
    pub ivao: usize,
    pub stale: usize,
    pub wx_cells: usize,
    pub airports: usize,
    pub navaids: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub projection: String,
    pub width: f64,
    pub height: f64,
    pub layers: Vec<SceneLayer>,
    pub labels: Vec<PlacedLabel>,
    pub dropped_labels: usize,
    pub scale_px: f64,
    pub scale_label: String,
    pub counts: Counts,
}

fn shape(t: &str, attrs: &[(&str, String)]) -> Shape {
    Shape { t: t.to_string(), a: attrs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect() }
}

fn pts(p: &[(f64, f64)]) -> String {
    p.iter().map(|(x, y)| format!("{x:.1},{y:.1}")).collect::<Vec<_>>().join(" ")
}

fn rotated(cx: f64, cy: f64, deg: f64, local: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let (s, c) = deg.to_radians().sin_cos();
    local.iter().map(|(x, y)| (cx + x * c - y * s, cy + x * s + y * c)).collect()
}

fn line(a: (f64, f64), b: (f64, f64), stroke: &str, width: &str) -> Shape {
    shape(
        "line",
        &[
            ("x1", format!("{:.1}", a.0)),
            ("y1", format!("{:.1}", a.1)),
            ("x2", format!("{:.1}", b.0)),
            ("y2", format!("{:.1}", b.1)),
            ("stroke", stroke.into()),
            ("stroke-width", width.into()),
            ("stroke-linecap", "round".into()),
        ],
    )
}

fn segment_visible(v: &Viewport, a: (f64, f64), b: (f64, f64)) -> bool {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    x1 >= -OFFSCREEN_MARGIN && y1 >= -OFFSCREEN_MARGIN && x0 <= v.width + OFFSCREEN_MARGIN && y0 <= v.height + OFFSCREEN_MARGIN
}

fn traffic_colour(t: &TrafficView, own: Option<&OwnShip>) -> &'static str {
    if t.age == Age::Stale {
        return palette::DIM;
    }
    if let Some(o) = own {
        let d = o.pos;
        let dlat = (t.pos.lat - d.lat) * 60.0;
        let dlon = (t.pos.lon - d.lon) * 60.0 * d.lat.to_radians().cos();
        if !t.report.on_ground && !o.on_ground && dlat.hypot(dlon) < PROXIMATE_NM && (t.report.alt_ft - o.alt_ft).abs() < PROXIMATE_FT {
            return palette::WARN;
        }
    }
    match t.report.source {
        TrafficSource::Ai => palette::GREY,
        TrafficSource::Vatsim | TrafficSource::Ivao => palette::OK,
    }
}

pub fn build_scene(v: &Viewport, toggles: &LayerToggles, input: &SceneInput) -> Scene {
    let z = v.zoom;
    let mut layers = Vec::new();
    let mut labels: Vec<LabelRequest> = Vec::new();
    let mut obstacles: Vec<Rect> = Vec::new();
    let mut counts = Counts::default();
    let mut label = |x: f64, y: f64, text: &str, class: &str, priority: i32| {
        labels.push(LabelRequest { x, y, text: text.to_string(), class: class.to_string(), priority });
    };

    if toggles.visible(LayerKind::Weather, z) && !input.weather.is_empty() {
        let g = sample_grid(v);
        let shapes = grid_shapes(&g, &levels_from_samples(&g, v, input.weather));
        counts.wx_cells = shapes.len();
        layers.push(SceneLayer { kind: LayerKind::Weather, shapes });
    }

    if let (true, Some(apt)) = (toggles.visible(LayerKind::Amdb, z), input.amdb) {
        let mut shapes = Vec::new();
        for f in &apt.features {
            let (fill, stroke, sw) = f.class.style();
            let mut d = String::new();
            let mut any = false;
            for r in &f.rings {
                let p: Vec<(f64, f64)> = r.iter().map(|ll| v.project(*ll)).collect();
                if p.windows(2).any(|w| segment_visible(v, w[0], w[1])) {
                    any = true;
                }
                for (i, (x, y)) in p.iter().enumerate() {
                    d.push_str(&format!("{}{x:.1},{y:.1} ", if i == 0 { "M" } else { "L" }));
                }
                if f.closed {
                    d.push_str("Z ");
                }
            }
            if !any {
                continue;
            }
            shapes.push(shape(
                "path",
                &[
                    ("d", d.trim_end().to_string()),
                    ("fill", fill.into()),
                    ("fill-rule", "evenodd".into()),
                    ("stroke", stroke.into()),
                    ("stroke-width", sw.into()),
                ],
            ));
            if let (Some(t), true) = (&f.label, toggles.labels(LayerKind::Amdb, z)) {
                if let Some(mid) = f.rings.first().and_then(|r| r.get(r.len() / 2)) {
                    let (x, y) = v.project(*mid);
                    let pri = if f.class == AmdbClass::Runway { 60 } else { 10 };
                    label(x, y, t, &format!("map-lbl amdb {}", format!("{:?}", f.class).to_lowercase()), pri);
                }
            }
        }
        layers.push(SceneLayer { kind: LayerKind::Amdb, shapes });
    }

    if toggles.visible(LayerKind::Airways, z) {
        let mut shapes = Vec::new();
        for s in input.airways {
            let (a, b) = (v.project(s.a), v.project(s.b));
            if segment_visible(v, a, b) {
                shapes.push(line(a, b, palette::DIM, "1"));
                if toggles.labels(LayerKind::Airways, z) {
                    label((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0, &s.name, "map-lbl awy", 5);
                }
            }
        }
        layers.push(SceneLayer { kind: LayerKind::Airways, shapes });
    }

    if toggles.visible(LayerKind::Airports, z) {
        let mut shapes = Vec::new();
        for a in input.airports.iter().filter(|a| z >= airport_min_zoom(a.size)) {
            let (x, y) = v.project(a.pos);
            if !v.contains(x, y, 8.0) {
                continue;
            }
            counts.airports += 1;
            let r = 3.0 + a.size.min(2) as f64;
            shapes.push(shape(
                "circle",
                &[("cx", format!("{x:.1}")), ("cy", format!("{y:.1}")), ("r", format!("{r:.1}")), ("fill", palette::BG.into()), ("stroke", palette::ACCENT.into()), ("stroke-width", "2".into())],
            ));
            obstacles.push(Rect::around(x, y, r + 1.0));
            if toggles.labels(LayerKind::Airports, z) {
                label(x, y, &a.ident, "map-lbl apt", 40 + 10 * a.size as i32);
            }
        }
        layers.push(SceneLayer { kind: LayerKind::Airports, shapes });
    }

    for (kind, nk) in [(LayerKind::Vors, NavaidKind::Vor), (LayerKind::Ndbs, NavaidKind::Ndb)] {
        if !toggles.visible(kind, z) {
            continue;
        }
        let mut shapes = Vec::new();
        for n in input.navaids.iter().filter(|n| n.kind == nk) {
            let (x, y) = v.project(n.pos);
            if !v.contains(x, y, 8.0) {
                continue;
            }
            counts.navaids += 1;
            shapes.push(match nk {
                NavaidKind::Vor => shape(
                    "polygon",
                    &[("points", pts(&rotated(x, y, 0.0, &[(-5.0, 0.0), (-2.5, -4.3), (2.5, -4.3), (5.0, 0.0), (2.5, 4.3), (-2.5, 4.3)]))), ("fill", "none".into()), ("stroke", palette::GREY.into()), ("stroke-width", "1.5".into())],
                ),
                NavaidKind::Ndb => shape("circle", &[("cx", format!("{x:.1}")), ("cy", format!("{y:.1}")), ("r", "3".into()), ("fill", palette::GREY.into())]),
            });
            obstacles.push(Rect::around(x, y, 5.0));
            if toggles.labels(kind, z) {
                label(x, y, &n.ident, if nk == NavaidKind::Vor { "map-lbl vor" } else { "map-lbl ndb" }, if nk == NavaidKind::Vor { 30 } else { 20 });
            }
        }
        layers.push(SceneLayer { kind, shapes });
    }

    if let (true, Some(route)) = (toggles.visible(LayerKind::Route, z), input.route) {
        let mut shapes = Vec::new();
        let p: Vec<(f64, f64)> = route.points.iter().map(|r| v.project(r.pos)).collect();
        for i in 1..p.len() {
            if !segment_visible(v, p[i - 1], p[i]) {
                continue;
            }
            let (stroke, w) = match route.active_to {
                Some(a) if i < a => (palette::DIM, "2"),
                Some(a) if i == a => (palette::OK, "4"),
                _ => (palette::OK, "2.5"),
            };
            shapes.push(line(p[i - 1], p[i], stroke, w));
        }
        for (i, (rp, (x, y))) in route.points.iter().zip(p.iter()).enumerate() {
            if !v.contains(*x, *y, 8.0) {
                continue;
            }
            let flown = route.active_to.is_some_and(|a| i < a);
            shapes.push(shape(
                "polygon",
                &[("points", pts(&rotated(*x, *y, 0.0, &[(0.0, -5.0), (5.0, 0.0), (0.0, 5.0), (-5.0, 0.0)]))), ("fill", (if flown { palette::DIM } else { palette::TEXT }).into())],
            ));
            obstacles.push(Rect::around(*x, *y, 5.0));
            label(*x, *y, &rp.ident, if flown { "map-lbl wpt flown" } else { "map-lbl wpt" }, if route.active_to == Some(i) { 95 } else { 80 });
        }
        layers.push(SceneLayer { kind: LayerKind::Route, shapes });
    }

    for kind in [LayerKind::AiTraffic, LayerKind::OnlineTraffic] {
        if !toggles.visible(kind, z) {
            continue;
        }
        let mut shapes = Vec::new();
        for t in input.traffic.iter().filter(|t| (t.report.source == TrafficSource::Ai) == (kind == LayerKind::AiTraffic)) {
            let (x, y) = v.project(t.pos);
            if !v.contains(x, y, 10.0) {
                continue;
            }
            match t.report.source {
                TrafficSource::Ai => counts.ai += 1,
                TrafficSource::Vatsim => counts.vatsim += 1,
                TrafficSource::Ivao => counts.ivao += 1,
            }
            if t.age == Age::Stale {
                counts.stale += 1;
            }
            let fill = traffic_colour(t, input.own.as_ref());
            let rot = t.report.track_deg - v.rotation_deg;
            shapes.push(shape(
                "polygon",
                &[("points", pts(&rotated(x, y, rot, &[(0.0, -7.0), (5.0, 5.0), (0.0, 2.5), (-5.0, 5.0)]))), ("fill", fill.into()), ("stroke", palette::BG.into()), ("stroke-width", "1".into())],
            ));
            obstacles.push(Rect::around(x, y, 6.0));
            if toggles.labels(kind, z) && !t.report.callsign.is_empty() {
                label(x, y, &t.report.callsign, &format!("map-lbl tfc {}", t.report.source.class()), if t.age == Age::Fresh { 50 } else { 15 });
            }
        }
        layers.push(SceneLayer { kind, shapes });
    }

    if let Some(o) = input.own {
        let (x, y) = v.project(o.pos);
        let rot = o.heading_deg - v.rotation_deg;
        let body = [(0.0, -16.0), (2.0, -11.0), (2.0, -4.0), (14.0, 3.0), (14.0, 6.0), (2.0, 2.0), (1.5, 10.0), (6.0, 14.0), (6.0, 16.0), (0.0, 14.5), (-6.0, 16.0), (-6.0, 14.0), (-1.5, 10.0), (-2.0, 2.0), (-14.0, 6.0), (-14.0, 3.0), (-2.0, -4.0), (-2.0, -11.0)];
        let mut shapes = vec![shape("polygon", &[("points", pts(&rotated(x, y, rot, &body))), ("fill", palette::WARN.into()), ("stroke", palette::BG.into()), ("stroke-width", "1.5".into())])];
        if o.gs_kt > 30.0 {
            let lead = (o.gs_kt / 60.0 * 2.0) / v.nm_per_px();
            let t = (o.track_deg - v.rotation_deg).to_radians();
            shapes.push(line((x, y), (x + lead * t.sin(), y - lead * t.cos()), palette::WARN, "1.5"));
        }
        obstacles.push(Rect::around(x, y, 16.0));
        layers.push(SceneLayer { kind: LayerKind::OwnShip, shapes });
    }

    let (placed, dropped) = declutter(labels, &obstacles, v.width, v.height);
    let (scale_px, scale_label) = scale_bar(v, v.width * 0.18);
    Scene { projection: v.describe(), width: v.width, height: v.height, layers, labels: placed, dropped_labels: dropped, scale_px, scale_label, counts }
}

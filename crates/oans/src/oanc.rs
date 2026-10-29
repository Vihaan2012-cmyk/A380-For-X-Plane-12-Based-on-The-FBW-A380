use serde_json::{json, Value};

use crate::geo::{clamp_angle, reciprocal, Position};
use crate::labels::{feature_type as ft, Label, LabelStyle, OancLabelFilter};

pub const POLYGON_STRUCTURE_TERMINAL_BUILDING: i32 = 1;

pub const A380_ZOOM_VALUES: [f64; 5] = [0.2, 0.5, 1.0, 2.0, 5.0];
pub const A320_ZOOM_VALUES: [f64; 4] = [0.2, 0.5, 1.0, 2.5];
const DEFAULT_SCALE_NM: f64 = 0.539957;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NdMode {
    RoseIls = 0,
    RoseVor = 1,
    RoseNav = 2,
    Arc = 3,
    Plan = 4,
}

#[derive(Clone, Debug)]
pub enum Geometry {
    Point(Position),
    LineString(Vec<Position>),
    Polygon(Vec<Vec<Position>>),
}

#[derive(Clone, Debug, Default)]
pub struct Props {
    pub id: Option<i64>,
    pub idlin: Option<String>,
    pub idstd: Option<String>,
    pub ident: Option<String>,
    pub idrwy: Option<String>,
    pub idthr: Option<String>,
    pub termref: Option<String>,
    pub plysttyp: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct Feature {
    pub feattype: u8,
    pub props: Props,
    pub geometry: Geometry,
}

pub fn oans_available(navigraph_available: bool, reset_pulled: bool) -> bool {
    navigraph_available && !reset_pulled
}

pub fn oans_failed(reset_pulled: bool) -> bool {
    reset_pulled
}

pub fn aircraft_on_ground(fwc_flight_phase: f64) -> bool {
    ![6.0, 7.0, 8.0, 9.0].contains(&fwc_flight_phase)
}

pub fn ppos_not_available(lat_normal: bool, long_normal: bool, true_heading_normal: bool) -> bool {
    !lat_normal || !long_normal || !true_heading_normal
}

pub fn using_ppos_as_reference(overlay_mode: NdMode, on_ground: bool, within_airport: bool) -> bool {
    (on_ground && within_airport) || overlay_mode == NdMode::Arc
}

pub fn airport_too_far_away_and_in_arc_nav_mode(ppos_unavailable: bool, using_ppos: bool, dist_to_arpt_nm: f64) -> bool {
    !ppos_unavailable && using_ppos && dist_to_arpt_nm > 30.0
}

pub fn airport_within_range(zoom_value: f64, dist_to_arpt_nm: f64) -> bool {
    dist_to_arpt_nm < zoom_value + 3.0
}

pub fn zoom_level_scales(zoom_values: &[f64]) -> Vec<f64> {
    zoom_values.iter().map(|z| 1.0 / ((z * 2.0) / DEFAULT_SCALE_NM)).collect()
}

pub fn zoom_level_inverse_scale(zoom_values: &[f64], index: usize, rose_nav: bool) -> f64 {
    zoom_level_scales(zoom_values)[index] * if rose_nav { 0.5 } else { 1.0 }
}

pub fn label_filter_for(mode: NdMode, zoom_level_index: usize) -> OancLabelFilter {
    if mode == NdMode::Arc {
        match zoom_level_index {
            4 | 3 => OancLabelFilter::None,
            2 => OancLabelFilter::Major,
            _ => OancLabelFilter::Null,
        }
    } else {
        match zoom_level_index {
            0 => OancLabelFilter::RunwayBtvSelection { runway_ident: None, show_adjacent: true },
            _ => OancLabelFilter::RunwayBtvSelection { runway_ident: None, show_adjacent: false },
        }
    }
}

pub fn mode_animation_offset(mode: NdMode) -> [f64; 2] {
    match mode {
        NdMode::Arc => [0.0, 620.0 - 768.0 / 2.0],
        _ => [0.0, 0.0],
    }
}

pub fn pad_designators(f: &mut Props) {
    if let Some(thr) = f.idthr.as_mut() {
        if !thr.is_empty() && thr.chars().filter(|c| c.is_ascii_digit()).count() < 2 {
            *thr = format!("0{thr}");
        }
    }
    if let Some(rwy) = f.idrwy.as_mut().filter(|r| !r.is_empty()) {
        *rwy = rwy
            .split('.')
            .map(|q| if q.chars().filter(|c| c.is_ascii_digit()).count() < 2 { format!("0{q}") } else { q.to_string() })
            .collect::<Vec<_>>()
            .join(".");
    }
}

fn centroid(rings: &[&[Position]], wrap: bool) -> Position {
    let mut x = 0.0;
    let mut y = 0.0;
    let mut count = 0usize;
    for points in rings {
        let n = if wrap && points.len() > 1 { points.len() - 1 } else { points.len() };
        for p in &points[..n] {
            x += p[0];
            y += p[1];
        }
        count += n;
    }
    [x / count as f64, y / count as f64]
}

fn digits_only(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).collect()
}

fn js_to_number(s: &str) -> f64 {
    if s.is_empty() {
        0.0
    } else {
        s.parse::<f64>().unwrap_or(f64::NAN)
    }
}

pub struct LabelGenInput<'a> {
    pub airport_icao: &'a str,
    pub fms_origin: Option<&'a str>,
    pub fms_departure_runway: Option<&'a str>,
    pub fms_landing_runway: Option<&'a str>,
    pub btv_runway: Option<&'a str>,
}

fn tail4(s: Option<&str>) -> Option<String> {
    s.map(|v| v.chars().skip(4).collect())
}

pub fn generate_all_labels(features: &[Feature], input: &LabelGenInput) -> Vec<Label> {
    let mut labels: Vec<Label> = Vec::new();
    let label_types = [ft::TAXIWAY_ELEMENT, ft::VERTICAL_POLYGONAL_STRUCTURE, ft::PAINTED_CENTERLINE, ft::PARKING_STAND_LOCATION, ft::RUNWAY_EXIT_LINE];
    for feature in features {
        if !label_types.contains(&feature.feattype) {
            continue;
        }
        if feature.feattype == ft::TAXIWAY_ELEMENT && feature.props.idlin.as_deref().unwrap_or("").is_empty() {
            continue;
        }
        if feature.feattype == ft::VERTICAL_POLYGONAL_STRUCTURE {
            if let Some(t) = feature.props.plysttyp {
                if t != 0 && t != POLYGON_STRUCTURE_TERMINAL_BUILDING {
                    continue;
                }
            }
        }
        let label_position: Position = match &feature.geometry {
            Geometry::Point(p) => *p,
            Geometry::Polygon(rings) => centroid(&rings.iter().map(Vec::as_slice).collect::<Vec<_>>(), true),
            Geometry::LineString(l) => centroid(&[l.as_slice()], false),
        };
        if feature.feattype == ft::PAINTED_CENTERLINE {
            let mut designators: Vec<String> = Vec::new();
            if let Some(r) = feature.props.idrwy.as_deref() {
                if !r.is_empty() {
                    designators.extend(r.split('.').map(str::to_string));
                }
            }
            if designators.is_empty() {
                continue;
            }
            let Geometry::LineString(line) = &feature.geometry else { continue };
            let start = line[0];
            let end = line[line.len() - 1];
            let bearing = clamp_angle(-(start[1] - end[1]).atan2(start[0] - end[0]).to_degrees() + 90.0);
            if (js_to_number(&digits_only(&designators[0])) * 10.0 - reciprocal(bearing)).abs() > 90.0 && designators.len() == 2 {
                designators.swap(0, 1);
            }
            let is_fms_origin = input.airport_icao == input.fms_origin.unwrap_or("\u{0}");
            let is_fms_destination = input.airport_icao == input.fms_origin.unwrap_or("\u{0}");
            let dep = tail4(input.fms_departure_runway);
            let ldg = tail4(input.fms_landing_runway);
            let is_selected = (is_fms_origin && dep.as_ref().is_some_and(|d| !d.is_empty() && designators.contains(d)))
                || (is_fms_destination && ldg.as_ref().is_some_and(|d| !d.is_empty() && designators.contains(d)));
            let axis_style = if is_selected { LabelStyle::FmsSelectedRunwayAxis } else { LabelStyle::RunwayAxis };
            let end_style = |d: &str| if input.btv_runway == Some(d) { LabelStyle::BtvSelectedRunwayEnd } else { LabelStyle::RunwayEnd };
            let ft_c = Some(ft::PAINTED_CENTERLINE);
            let ends = Some([start, end]);
            let termref = feature.props.termref.clone();
            let mk = |text: &str, style: LabelStyle, pos: Position, rot: f64, axis: bool| Label {
                text: text.to_string(),
                style,
                position: pos,
                rotation: Some(rot),
                feattype: ft_c,
                axis_ends: if axis { ends } else { None },
                termref: termref.clone(),
            };
            labels.push(mk(&designators[0], end_style(&designators[0]), start, reciprocal(bearing), false));
            if designators.len() > 1 {
                labels.push(mk(&designators[1], end_style(&designators[1]), end, bearing, false));
                labels.push(mk(&format!("{}-{}", designators[0], designators[1]), axis_style, end, bearing, true));
            } else {
                labels.push(mk(&designators[0], axis_style, end, bearing, true));
            }
            labels.push(mk(&designators[0], LabelStyle::FmsSelectedRunwayEnd, start, reciprocal(bearing), false));
            labels.push(mk(designators.get(1).map(String::as_str).unwrap_or(""), LabelStyle::FmsSelectedRunwayEnd, end, bearing, false));
            labels.push(mk(&designators[0], LabelStyle::BtvSelectedRunwayArrow, start, reciprocal(bearing), false));
            labels.push(mk(designators.get(1).map(String::as_str).unwrap_or(""), LabelStyle::BtvSelectedRunwayArrow, end, bearing, false));
        } else {
            let text = feature.props.idlin.clone().or_else(|| feature.props.idstd.clone()).or_else(|| feature.props.ident.clone());
            let Some(text) = text else { continue };
            if feature.feattype == ft::PARKING_STAND_LOCATION && text.contains('_') {
                continue;
            }
            let style = match feature.feattype {
                ft::VERTICAL_POLYGONAL_STRUCTURE | ft::PARKING_STAND_LOCATION => LabelStyle::TerminalBuilding,
                ft::RUNWAY_EXIT_LINE => LabelStyle::ExitLine,
                _ => LabelStyle::Taxiway,
            };
            let mut shortest = 9007199254740991.0_f64;
            for l in labels.iter().filter(|l| l.text == text) {
                let d = (l.position[0] - label_position[0]).hypot(l.position[1] - label_position[1]);
                shortest = if d > shortest { d } else { shortest };
            }
            let _ = &mut shortest;
            let existing_same_terminal = feature.feattype == ft::PARKING_STAND_LOCATION
                && labels.iter().filter(|l| l.text == text).any(|l| l.termref == feature.props.termref);
            if existing_same_terminal || shortest < 50.0 {
                continue;
            }
            labels.push(Label {
                text: text.to_uppercase(),
                style,
                position: label_position,
                rotation: None,
                feattype: Some(feature.feattype),
                axis_ends: None,
                termref: feature.props.termref.clone(),
            });
        }
    }
    labels
}

fn in_ring(pt: Position, ring: &[Position], ignore_boundary: bool) -> bool {
    let mut inside = false;
    let len = ring.len();
    let mut j = len.wrapping_sub(1);
    for i in 0..len {
        let (xi, yi) = (ring[i][0], ring[i][1]);
        let (xj, yj) = (ring[j][0], ring[j][1]);
        let on_boundary = pt[1] * (xi - xj) + yi * (xj - pt[0]) + yj * (pt[0] - xi) == 0.0
            && (xi - pt[0]) * (xj - pt[0]) <= 0.0
            && (yi - pt[1]) * (yj - pt[1]) <= 0.0;
        if on_boundary {
            return !ignore_boundary;
        }
        let intersect = ((yi > pt[1]) != (yj > pt[1])) && (pt[0] < ((xj - xi) * (pt[1] - yi)) / (yj - yi) + xi);
        if intersect {
            inside = !inside;
        }
        j = i;
    }
    inside
}

pub fn point_in_polygon(pt: Position, rings: &[Vec<Position>]) -> bool {
    if rings.is_empty() {
        return false;
    }
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in &rings[0] {
        minx = minx.min(p[0]);
        miny = miny.min(p[1]);
        maxx = maxx.max(p[0]);
        maxy = maxy.max(p[1]);
    }
    if pt[0] < minx || pt[0] > maxx || pt[1] < miny || pt[1] > maxy {
        return false;
    }
    if in_ring(pt, &rings[0], false) {
        for hole in &rings[1..] {
            if in_ring(pt, hole, true) {
                return false;
            }
        }
        return true;
    }
    false
}

pub fn compute_position(features: &[Feature], projected_ppos: Position) -> Option<String> {
    let valid = [ft::PARKING_STAND_AREA, ft::TAXIWAY_ELEMENT, ft::RUNWAY_ELEMENT, ft::STOPWAY, ft::RUNWAY_DISPLACED_AREA, ft::BLAST_PAD];
    for f in features {
        if !valid.contains(&f.feattype) {
            continue;
        }
        let Geometry::Polygon(rings) = &f.geometry else { continue };
        if point_in_polygon(projected_ppos, rings) {
            return match f.feattype {
                ft::PARKING_STAND_AREA => f.props.idstd.clone(),
                ft::TAXIWAY_ELEMENT => f.props.idlin.clone(),
                ft::RUNWAY_ELEMENT | ft::STOPWAY => f.props.idrwy.as_ref().map(|r| r.replacen('.', "-", 1)),
                ft::BLAST_PAD | ft::RUNWAY_DISPLACED_AREA => f.props.idthr.clone(),
                _ => f.props.ident.clone(),
            };
        }
    }
    None
}

pub fn label_json(l: &Label) -> Value {
    json!({
        "text": l.text,
        "style": l.style.as_str(),
        "position": l.position,
        "rotation": l.rotation,
    })
}

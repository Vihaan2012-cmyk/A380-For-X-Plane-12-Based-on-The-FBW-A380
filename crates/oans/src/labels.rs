use crate::geo::{clamp_angle, Position};
use crate::map_utils::{intersect_line_with_rectangle, is_point_in_rectangle, mid_point, point_angle};

pub const OANC_RENDER_WIDTH: f64 = 768.0;
pub const OANC_RENDER_HEIGHT: f64 = 768.0;

pub const LABEL_VISIBILITY_RULES: [bool; 5] = [true, true, true, true, true];

pub mod feature_type {
    pub const RUNWAY_ELEMENT: u8 = 0;
    pub const RUNWAY_THRESHOLD: u8 = 2;
    pub const PAINTED_CENTERLINE: u8 = 4;
    pub const STOPWAY: u8 = 8;
    pub const RUNWAY_DISPLACED_AREA: u8 = 9;
    pub const TAXIWAY_ELEMENT: u8 = 14;
    pub const RUNWAY_EXIT_LINE: u8 = 19;
    pub const PARKING_STAND_LOCATION: u8 = 23;
    pub const PARKING_STAND_AREA: u8 = 24;
    pub const VERTICAL_POLYGONAL_STRUCTURE: u8 = 27;
    pub const BLAST_PAD: u8 = 33;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelStyle {
    Taxiway,
    ExitLine,
    TerminalBuilding,
    RunwayAxis,
    RunwayEnd,
    FmsSelectedRunwayEnd,
    FmsSelectedRunwayAxis,
    BtvSelectedRunwayEnd,
    BtvSelectedRunwayArrow,
    BtvSelectedExit,
    BtvStopLineMagenta,
    BtvStopLineAmber,
    BtvStopLineRed,
    BtvStopLineGreen,
    CrossSymbol,
    FlagSymbol,
}

impl LabelStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            LabelStyle::Taxiway => "taxiway",
            LabelStyle::ExitLine => "exit-line",
            LabelStyle::TerminalBuilding => "terminal-building",
            LabelStyle::RunwayAxis => "runway-axis",
            LabelStyle::RunwayEnd => "runway-end",
            LabelStyle::FmsSelectedRunwayEnd => "runway-end-fms-selected",
            LabelStyle::FmsSelectedRunwayAxis => "runway-axis-fms-selected",
            LabelStyle::BtvSelectedRunwayEnd => "runway-end-btv-selected",
            LabelStyle::BtvSelectedRunwayArrow => "runway-arrow-btv-selected",
            LabelStyle::BtvSelectedExit => "exit-line-btv-selected",
            LabelStyle::BtvStopLineMagenta => "btv-stop-line-magenta",
            LabelStyle::BtvStopLineAmber => "btv-stop-line-amber",
            LabelStyle::BtvStopLineRed => "btv-stop-line-red",
            LabelStyle::BtvStopLineGreen => "btv-stop-line-green",
            LabelStyle::CrossSymbol => "cross-symbol",
            LabelStyle::FlagSymbol => "flag-symbol",
        }
    }

    pub fn is_stop_line(self) -> bool {
        matches!(
            self,
            LabelStyle::BtvStopLineAmber | LabelStyle::BtvStopLineGreen | LabelStyle::BtvStopLineMagenta | LabelStyle::BtvStopLineRed
        )
    }
}

#[derive(Clone, Debug)]
pub struct Label {
    pub text: String,
    pub style: LabelStyle,
    pub position: Position,
    pub rotation: Option<f64>,
    pub feattype: Option<u8>,
    pub axis_ends: Option<[Position; 2]>,
    pub termref: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OancLabelFilter {
    RunwayBtvSelection { runway_ident: Option<String>, show_adjacent: bool },
    None,
    Null,
    Major,
}

fn tail4(s: Option<&str>) -> Option<&str> {
    s.map(|v| v.get(4..).unwrap_or(""))
}

pub fn filter_label(
    label: &Label,
    filter: &OancLabelFilter,
    fms_dep_runway: Option<&str>,
    fms_ldg_runway: Option<&str>,
    btv_selected_runway: Option<&str>,
    btv_selected_exit: Option<&str>,
) -> bool {
    if label.style == LabelStyle::FmsSelectedRunwayEnd && !label.text.is_empty() {
        return Some(label.text.as_str()) == tail4(fms_dep_runway) || Some(label.text.as_str()) == tail4(fms_ldg_runway);
    }
    if label.style == LabelStyle::BtvSelectedRunwayArrow && !label.text.is_empty() {
        return Some(label.text.as_str()) == tail4(btv_selected_runway);
    }
    if btv_selected_runway.is_some_and(|r| !r.is_empty())
        && label.feattype == Some(feature_type::PAINTED_CENTERLINE)
        && Some(label.text.as_str()) == tail4(btv_selected_runway)
    {
        return true;
    }
    if btv_selected_exit.is_some_and(|e| !e.is_empty()) && label.style == LabelStyle::BtvSelectedExit {
        return true;
    }
    if label.style.is_stop_line() || matches!(label.style, LabelStyle::CrossSymbol | LabelStyle::FlagSymbol) {
        return true;
    }
    match filter {
        OancLabelFilter::None => false,
        OancLabelFilter::Null => label.feattype != Some(feature_type::RUNWAY_EXIT_LINE),
        OancLabelFilter::Major => {
            label.feattype == Some(feature_type::PAINTED_CENTERLINE)
                || (label.text.encode_utf16().count() < 2 && label.feattype != Some(feature_type::RUNWAY_EXIT_LINE))
        }
        OancLabelFilter::RunwayBtvSelection { show_adjacent, .. } => {
            label.feattype == Some(feature_type::PAINTED_CENTERLINE)
                || label.feattype == Some(feature_type::RUNWAY_EXIT_LINE)
                || *show_adjacent
        }
    }
}

pub struct FmsRunways<'a> {
    pub departure: Option<&'a str>,
    pub landing: Option<&'a str>,
}

pub fn label_style(
    label: &Label,
    fms: &FmsRunways,
    is_fms_origin: bool,
    is_fms_destination: bool,
    btv_selected_runway: Option<&str>,
    btv_selected_exit: Option<&str>,
) -> LabelStyle {
    if label.style == LabelStyle::RunwayEnd || label.style == LabelStyle::BtvSelectedRunwayEnd {
        return if tail4(btv_selected_runway) == Some(label.text.as_str()) { LabelStyle::BtvSelectedRunwayEnd } else { LabelStyle::RunwayEnd };
    }
    if label.style == LabelStyle::RunwayAxis || label.style == LabelStyle::FmsSelectedRunwayAxis {
        let selected = (is_fms_origin && Some(label.text.as_str()) == tail4(fms.departure))
            || (is_fms_destination && Some(label.text.as_str()) == tail4(fms.landing));
        return if selected { LabelStyle::FmsSelectedRunwayAxis } else { LabelStyle::RunwayAxis };
    }
    if label.style == LabelStyle::ExitLine || label.style == LabelStyle::BtvSelectedExit {
        return if Some(label.text.as_str()) == btv_selected_exit { LabelStyle::BtvSelectedExit } else { LabelStyle::ExitLine };
    }
    label.style
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ScreenParams {
    pub offset_x: f64,
    pub offset_y: f64,
    pub map_heading: f64,
    pub zoom_inverse_scale: f64,
    pub pan_x: f64,
    pub pan_y: f64,
    pub mode_x: f64,
    pub mode_y: f64,
}

pub fn project_point(sp: &ScreenParams, coordinates: Position) -> [f64; 2] {
    let (label_x, label_y) = (coordinates[0], coordinates[1]);
    let rotate = -sp.map_heading;
    let hypotenuse = label_x.hypot(label_y) * sp.zoom_inverse_scale;
    let angle = clamp_angle(label_y.atan2(label_x).to_degrees());
    let rot_x = hypotenuse * (angle - rotate).to_radians().cos();
    let rot_y = hypotenuse * (angle - rotate).to_radians().sin();
    let scaled_x = sp.offset_x * sp.zoom_inverse_scale;
    let scaled_y = sp.offset_y * sp.zoom_inverse_scale;
    let sx = OANC_RENDER_WIDTH / 2.0 + rot_x + -scaled_x + sp.pan_x + sp.mode_x;
    let sy = OANC_RENDER_HEIGHT / 2.0 + -rot_y + scaled_y + sp.pan_y + sp.mode_y;
    [sx, sy]
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlacedLabel {
    pub visible: bool,
    pub left: f64,
    pub top: f64,
    pub transform: String,
}

fn js_num(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

pub fn place_label(
    label: &Label,
    sp: &ScreenParams,
    filter: &OancLabelFilter,
    fms: &FmsRunways,
    btv_runway: Option<&str>,
    btv_exit: Option<&str>,
) -> PlacedLabel {
    let hidden = PlacedLabel { visible: false, left: 0.0, top: 0.0, transform: String::new() };
    if !filter_label(label, filter, fms.departure, fms.landing, btv_runway, btv_exit) {
        return hidden;
    }
    if label.style != LabelStyle::RunwayAxis && label.style != LabelStyle::FmsSelectedRunwayAxis {
        let p = project_point(sp, label.position);
        if p[0] < 0.0 || p[0] > OANC_RENDER_WIDTH || p[1] < 0.0 || p[1] > OANC_RENDER_HEIGHT {
            return hidden;
        }
        let rot = label.rotation.unwrap_or(0.0);
        let transform = match label.style {
            LabelStyle::RunwayEnd | LabelStyle::BtvSelectedRunwayEnd => {
                format!("translate(-50%, -50%) rotate({}deg) translate(0px, 50px)", js_num(rot - sp.map_heading))
            }
            LabelStyle::BtvSelectedRunwayArrow => format!(
                "translate(-50%, -50%) rotate({}deg) translate(0px, -100px) rotate(-180deg)",
                js_num(rot - sp.map_heading)
            ),
            LabelStyle::FmsSelectedRunwayEnd => {
                format!("translate(-50%, -50%) rotate({}deg) translate(0px, 82.5px)", js_num(rot - sp.map_heading))
            }
            _ => "translate(-50%, -50%)".to_string(),
        };
        return PlacedLabel { visible: true, left: p[0], top: p[1], transform };
    }
    let Some(ends) = label.axis_ends else { return hidden };
    let sp_nopan = *sp;
    let [sx1, sy1] = project_point(&sp_nopan, ends[0]);
    let [sx2, sy2] = project_point(&sp_nopan, ends[1]);
    let [lcx, lcy] = mid_point(sx1, sy1, sx2, sy2);
    let (rx, ry, rw, rh) = (0.0, 0.0, OANC_RENDER_WIDTH, OANC_RENDER_HEIGHT);
    let rwy_angle = clamp_angle(point_angle(sx1, sy1, sx2, sy2) - 90.0);
    let intersections = intersect_line_with_rectangle(sx1, sy1, sx2, sy2, rx, ry, rw, rh);
    let (mut x, mut y);
    if intersections.len() > 1 {
        let lscx = intersections[0][0] + (intersections[1][0] - intersections[0][0]) / 2.0;
        let lscy = intersections[0][1] + (intersections[1][1] - intersections[0][1]) / 2.0;
        let lsci = intersect_line_with_rectangle(lscx, lscy, lcx, lcy, rx, ry, rw, rh);
        if !lsci.is_empty() {
            x = intersections[0][0];
            y = intersections[0][1];
        } else {
            x = lcx;
            y = lcy;
        }
    } else if !intersections.is_empty() {
        let [px, py] = if is_point_in_rectangle(sx1, sy1, rx, ry, rw, rh) { [sx1, sy1] } else { [sx2, sy2] };
        let x_length = intersections[0][0] - px;
        let y_length = intersections[0][1] - py;
        let hyp = x_length.hypot(y_length);
        let new_hyp = hyp - 50.0;
        let angle = y_length.atan2(x_length);
        x = px + angle.cos() * new_hyp;
        y = py + angle.sin() * new_hyp;
    } else {
        x = -1000.0;
        y = -1000.0;
    }
    x += sp.mode_x;
    y += sp.mode_y;
    let _ = (&mut x, &mut y);
    PlacedLabel {
        visible: true,
        left: x,
        top: y,
        transform: format!("translate(-50%, -50%) rotate({}deg)", js_num(rwy_angle + if rwy_angle > 180.0 { 90.0 } else { -90.0 })),
    }
}

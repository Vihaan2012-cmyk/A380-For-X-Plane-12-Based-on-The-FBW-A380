use fms::navdb::{Fix, Procedure, Runway, Transition};
use fms::plan::{Element, FlightPlan, Leg, Sel};
use fms::service::FLAG_COPIED_FROM_ACTIVE;
use fms_data::{CostIndexMode, PerformanceData};
use mfd_pages_a::widgets::MenuItem;
use mfd_pages_a::{FplnModel, ShellModel};
use mfd_pages_a::fpln::{DestRow, LineData};
use mfd_pages_b::fuel::FuelModel;
use mfd_pages_b::init::InitModel;
use mfd_pages_b::perf::PerfModel;
use mfd_pages_c::fpln_rev::{AirwayLine, AirwaysModel, ArrivalModel, DepartureModel, DirectToModel, HoldModel, ProcMenu};
use mfd_pages_c::position::{PositionIrsModel, PositionMonitorModel, PositionNavaidsModel};
use mfd_pages_c::sec_index::{created_label, import_items, SecIndexModel, SecTab};
use mfd_pages_c::vert_rev::{StepLine, VertRevModel};
use mfd_pages_c::wind::{WindModel, WindRow};
use mfd_pages_d::fcubkup::{FcuBkupAfsModel, FcuBkupEfisModel};
use mfd_pages_d::surv::{SurvControlsModel, SurvStatusModel};

use crate::page::Page;
use crate::sources::Sources;
use crate::ui::UiState;

pub const PHASE_PREFLIGHT: u8 = 0;
pub const PHASE_TAKEOFF: u8 = 1;
pub const PHASE_CRUISE: u8 = 3;
pub const PHASE_DESCENT: u8 = 4;
pub const PHASE_APPROACH: u8 = 5;
pub const PHASE_GOAROUND: u8 = 6;

/// FBW `loadedFlightPlanIndex`: the temporary plan while one exists on ACTIVE pages, 3.. for SEC.
pub fn plan_index(ui: &UiState, src: &Sources) -> usize {
    match ui.uri.sec() {
        Some(n) => 2 + n,
        None if src.tmpy() => 1,
        None => 0,
    }
}

pub fn active_or_copy(src: &Sources, idx: usize) -> bool {
    idx <= 1 || src.plan(idx).is_some_and(|p| p.flags & FLAG_COPIED_FROM_ACTIVE != 0)
}

pub fn page_title(ui: &UiState, title: &str) -> String {
    let cat = match ui.uri.category.as_str() {
        "sec" => "SEC INDEX".to_string(),
        c => c.to_uppercase(),
    };
    format!("{cat}/{title}")
}

fn weight_unit_kg(metric: bool) -> f64 {
    if metric {
        1000.
    } else {
        453.59237
    }
}

fn t_to_kg(v: Option<f64>) -> Option<f64> {
    v.map(|t| t * 1000.)
}

pub fn shell(ui: &UiState, src: &Sources) -> ShellModel {
    let callsign = src.plan(0).map(|p| p.flight_number.clone()).filter(|s| !s.is_empty());
    let message = ui.entry_message.clone().or_else(|| src.messages.iter().find(|m| !ui.cleared_messages.contains(m)).cloned());
    ShellModel {
        side: ui.side().into(),
        fms_source: if ui.captain { "FMS 1".into() } else { "FMS 2".into() },
        callsign,
        system: ui.uri.sys.clone(),
        category: ui.uri.category.clone(),
        sys_dropdown_open: ui.open.as_deref().is_some_and(|o| o.ends_with("sysSelectorDropdown")),
        sys_dropdown_typed: String::new(),
        open_page_selector: ui.page_selector,
        eo: false,
        tmpy: src.tmpy() && ui.uri.category == "active",
        penalty: false,
        message,
        message_background: String::new(),
        message_list_open: ui.message_list_open,
        message_list: src.messages.to_vec(),
        duplicates_open: false,
        page_not_avail: ui.page() == Page::NotFound,
    }
}

fn leg_location(l: &Leg) -> Option<fms::geo::Coordinates> {
    l.location()
}

fn constraint_kind(descriptor: &str) -> &'static str {
    match descriptor {
        "+" => "above",
        "-" => "below",
        "B" => "range",
        _ => "at",
    }
}

fn mag_track(true_track: f64, fix: Option<&Fix>) -> f64 {
    let mv = fix.and_then(|f| f.mag_var).unwrap_or(0.);
    let t = (true_track - mv).rem_euclid(360.);
    if t < 0.5 {
        360.
    } else {
        t
    }
}

fn special(label: &str) -> LineData {
    LineData { kind: "special".into(), label: label.into(), ..Default::default() }
}

fn legs_lines(plan: &FlightPlan, start: usize, altn: bool, out: &mut Vec<LineData>) {
    let legs = plan.all_legs();
    let missed_from = plan.first_missed_approach_leg_index();
    let mut prev_loc: Option<fms::geo::Coordinates> = (0..start).rev().find_map(|i| legs.get(i).and_then(|e| e.leg()).and_then(leg_location));
    let trans_alt = plan.performance.database_transition_altitude;
    for (i, e) in legs.iter().enumerate().skip(start) {
        match e {
            Element::Disco => {
                let mut d = special("DISCONTINUITY");
                d.original_leg_index = Some(i);
                d.is_altn = altn;
                out.push(d);
            }
            Element::Leg(l) => {
                let loc = leg_location(l);
                let alt = l.altitude_constraint();
                let spd = l.speed_constraint();
                let (track, dist) = match (prev_loc, loc) {
                    (Some(a), Some(b)) => (plan.true_track_at(i).map(|t| mag_track(t, l.termination_waypoint())), Some(a.distance_to(&b))),
                    _ => (None, None),
                };
                if loc.is_some() {
                    prev_loc = loc;
                }
                out.push(LineData {
                    kind: if l.is_hx() { "hold".into() } else { "waypoint".into() },
                    original_leg_index: Some(i),
                    ident: l.ident.clone(),
                    overfly: l.definition.overfly,
                    annotation: l.annotation.clone(),
                    is_altn: altn,
                    is_missed: i >= missed_from,
                    transition_altitude: trans_alt,
                    has_altitude_constraint: alt.is_some(),
                    altitude_constraint_altitude1: alt.as_ref().map(|a| a.altitude1),
                    altitude_constraint_altitude2: alt.as_ref().and_then(|a| a.altitude2),
                    altitude_constraint_kind: alt.as_ref().map(|a| constraint_kind(&a.descriptor)).unwrap_or("").into(),
                    altitude_constraint_respected: true,
                    has_speed_constraint: spd.is_some(),
                    speed_constraint: spd.as_ref().map(|s| s.speed),
                    speed_constraint_respected: true,
                    track,
                    dist,
                    ..Default::default()
                });
            }
        }
    }
}

/// FBW `MfdFmsFpln.update`: the plan's legs from the FROM leg, DISCONTINUITY and END OF F-PLN lines, then the alternate.
pub fn fpln_line_data(plan: Option<&FlightPlan>) -> Vec<LineData> {
    let mut out = Vec::new();
    let Some(plan) = plan.filter(|p| p.leg_count() > 0) else {
        out.push(LineData { kind: "waypoint".into(), ident: "P.POS".into(), ..Default::default() });
        out.push(special("DISCONTINUITY"));
        out.push(special("END OF F-PLN"));
        out.push(special("NO ALTN F-PLN"));
        return out;
    };
    let start = (plan.active_leg_index - 1).max(0) as usize;
    legs_lines(plan, start, false, &mut out);
    out.push(special("END OF F-PLN"));
    match plan.alternate.as_deref().filter(|a| a.leg_count() > 0) {
        Some(a) => {
            legs_lines(a, 0, true, &mut out);
            out.push(special("END OF ALTN F-PLN"));
        }
        None => out.push(special("NO ALTN F-PLN")),
    }
    out
}

pub fn fpln_visible_lines(ui: &UiState, src: &Sources) -> (Vec<LineData>, usize, usize) {
    let idx = plan_index(ui, src);
    let plan = src.plan(idx);
    let all = fpln_line_data(plan);
    let count = if src.tmpy() && ui.uri.sec().is_none() { 8 } else { 9 };
    let start = ui.fpln_scroll.unwrap_or(0).min(all.len().saturating_sub(1));
    let active_leg = plan.map(|p| p.active_leg_index).unwrap_or(-1);
    let mut shown = Vec::new();
    for (k, d) in all.iter().enumerate().skip(start).take(count) {
        let mut d = d.clone();
        let line = k - start;
        d.first_line = line == 0;
        d.last_line = line == count - 1;
        d.after_special = k > 0 && all[k - 1].kind == "special";
        d.before_special = all.get(k + 1).is_some_and(|n| n.kind == "special");
        let draw_index = k as i64;
        d.is_active_leg = draw_index == active_leg || d.is_active_hold_leg;
        d.before_active_leg = !d.is_active_leg && draw_index == active_leg - 1;
        d.selected = ui.open.as_deref() == Some("revisions") && d.original_leg_index == ui.revised_leg && d.is_altn == ui.revised_altn;
        shown.push(d);
    }
    (shown, start, all.len())
}

fn route_distance_to_dest(plan: &FlightPlan, src: &Sources) -> Option<f64> {
    let dest = plan.destination_leg_index()?;
    let legs = plan.all_legs();
    let from = plan.active_leg_index.max(0) as usize;
    let lat = src.var("PLANE LATITUDE");
    let lon = src.var("PLANE LONGITUDE");
    let mut prev = if lat != 0. || lon != 0. { Some(fms::geo::Coordinates::new(lat, lon)) } else { None };
    let mut total = 0.;
    for e in legs.iter().take(dest + 1).skip(from) {
        if let Some(loc) = e.leg().and_then(leg_location) {
            if let Some(p) = prev {
                total += p.distance_to(&loc);
            }
            prev = Some(loc);
        }
    }
    Some(total)
}

pub const FPLN_PAGE_LINES: usize = 9;

/// The plan the revised leg belongs to: the alternate when an alternate leg was revised.
pub fn revised_plan<'a>(ui: &UiState, plan: Option<&'a FlightPlan>) -> Option<&'a FlightPlan> {
    let p = plan?;
    if ui.revised_altn {
        if let Some(a) = p.alternate.as_deref() {
            return Some(a);
        }
    }
    Some(p)
}

/// FBW `openInsertNextWptFromWindow`: the XF legs after the revised leg, as (leg index, ident).
pub fn next_wpt_options(ui: &UiState, plan: Option<&FlightPlan>) -> Vec<(usize, String)> {
    let (Some(fp), Some(rev)) = (revised_plan(ui, plan), ui.revised_leg) else { return Vec::new() };
    fp.all_legs().iter().enumerate().filter(|(i, _)| *i > rev).filter_map(|(i, e)| e.leg().filter(|l| l.is_xf()).map(|l| (i, l.ident.clone()))).collect()
}

/// FBW `getRevisionsMenu` items and their disabled rules for the revised leg.
pub fn revisions_menu(ui: &UiState, src: &Sources) -> Vec<MenuItem> {
    let idx = plan_index(ui, src);
    let Some(plan) = src.plan(idx) else { return Vec::new() };
    let Some(li) = ui.revised_leg else { return Vec::new() };
    let altn = ui.revised_altn;
    let fp = if altn { plan.alternate.as_deref().unwrap_or(plan) } else { plan };
    let revised = fp.element_at(li);
    let prev = if li > 0 { fp.element_at(li - 1) } else { None };
    let disco = revised.is_some_and(|e| e.is_disco());
    let leg = revised.and_then(|e| e.leg());
    let is_from = !altn && li as i64 == plan.active_leg_index - 1;
    let xf_db = leg.is_some_and(|l| l.is_xf() && l.flags & fms::plan::FLAG_DIRECT_TO_TURNING_POINT == 0);
    let runway = leg.is_some_and(|l| l.is_runway());
    let first_missed = fp.first_missed_approach_leg_index();
    let sec = idx >= 3;
    let overfly = leg.is_some_and(|l| l.definition.overfly) && !altn && !disco;
    let prev_vectors = prev.and_then(|e| e.leg()).is_some_and(|l| l.is_vectors());
    let pos = fp.segment_position(li).map(|(s, _)| fp.segments.get(s).map(|seg| seg.kind));
    let in_dep = matches!(pos, Some(Some(fms::plan::SegmentKind::Origin | fms::plan::SegmentKind::Departure | fms::plan::SegmentKind::DepartureRunwayTransition)));
    let in_arr = matches!(
        pos,
        Some(Some(
            fms::plan::SegmentKind::Destination
                | fms::plan::SegmentKind::Arrival
                | fms::plan::SegmentKind::Approach
                | fms::plan::SegmentKind::ApproachVia
                | fms::plan::SegmentKind::ArrivalRunwayTransition
                | fms::plan::SegmentKind::ArrivalEnrouteTransition
        ))
    );
    let has_altn_dest = plan.alternate.as_ref().is_some_and(|a| a.destination_airport.is_some());
    vec![
        MenuItem::new(if sec { "\u{a0}" } else { "FROM P.POS DIR TO" }, altn || li >= first_missed || idx == 1 || sec || disco || is_from || !xf_db),
        MenuItem::new("INSERT NEXT WPT", leg.is_some_and(|l| l.is_vectors())),
        MenuItem::new("DELETE *", runway || (disco && prev_vectors) || is_from),
        MenuItem::new("DEPARTURE", !(in_dep || runway)),
        MenuItem::new("ARRIVAL", !(in_arr || runway)),
        MenuItem::new("(N/A) OFFSET", true),
        MenuItem::new("HOLD", disco || is_from || !xf_db),
        MenuItem::new("AIRWAYS", runway || disco || is_from || !xf_db),
        MenuItem::new(if overfly { "DELETE OVERFLY *" } else { "OVERFLY *" }, true),
        MenuItem::new("ENABLE ALTN *", altn || leg.is_none() || !has_altn_dest),
        MenuItem::new("NEW DEST", is_from || !xf_db),
        MenuItem::new("CONSTRAINTS", leg.is_none()),
        MenuItem::new("(N/A) CMS", true),
        MenuItem::new("STEP ALTs", altn || !xf_db || disco),
        MenuItem::new("WIND", altn || !xf_db || disco),
    ]
}

pub fn fpln(ui: &UiState, src: &Sources) -> FplnModel {
    let idx = plan_index(ui, src);
    let plan = src.plan(idx);
    let (lines, start, total) = fpln_visible_lines(ui, src);
    let tmpy = idx == 1;
    let sec = idx >= 3;
    let dest = plan.and_then(|p| p.destination_airport.as_ref()).map(|a| {
        let rwy = plan.and_then(|p| p.destination_runway.as_ref()).map(|r| r.ident().trim_start_matches(a.ident.as_str()).trim_start_matches("RW").to_string()).unwrap_or_default();
        DestRow {
            loaded: true,
            button_label: format!("{}{}", a.ident, rwy),
            time_ms: None,
            efob_kg: None,
            efob_amber: false,
            distance: plan.and_then(|p| route_distance_to_dest(p, src)).map(|d| d.round()),
        }
    });
    let mut m = FplnModel {
        title: page_title(ui, "F-PLN"),
        line_color: if tmpy { "temporary" } else if sec { "secondary" } else { "active" }.into(),
        preflight: src.phase == PHASE_PREFLIGHT,
        display_efob_and_wind: ui.efob_and_wind,
        true_track: false,
        weight_kg: ui.metric,
        lines,
        dest: dest.unwrap_or_default(),
        tmpy_active: tmpy,
        active_plan: idx == 0,
        scroll_down_disabled: start >= total.saturating_sub(FPLN_PAGE_LINES),
        scroll_up_disabled: start == 0,
        spd_alt_open: ui.open.as_deref().is_some_and(|o| o.ends_with("_MFD_windbtn")),
        info_open: ui.open.as_deref().is_some_and(|o| o.ends_with("_MFD_f-pln-infoBtn")),
        sec_active: sec,
        show_init: !sec && src.phase == PHASE_PREFLIGHT,
        sec_index: ui.uri.sec().unwrap_or(1),
        revisions_menu: revisions_menu(ui, src),
        revisions_menu_open: ui.open.as_deref() == Some("revisions"),
        revisions_menu_positioned: false,
        tmpy_line: None,
        ..Default::default()
    };
    if ui.new_dest_open || ui.insert_next_open {
        let fp = revised_plan(ui, plan);
        let from_leg = ui.revised_leg.and_then(|i| fp.and_then(|p| p.leg_at(i)));
        let from = from_leg.map(|l| l.ident.clone()).unwrap_or_default();
        m.new_dest.open = ui.new_dest_open;
        m.new_dest.from_ident = from.clone();
        m.insert_next.open = ui.insert_next_open;
        m.insert_next.from_ident = from;
        if ui.insert_next_open {
            m.insert_next.coordinates = from_leg.and_then(|l| l.definition.waypoint.as_ref()).map(|w| position_text(w.location.lat, w.location.long)).unwrap_or_default();
            m.insert_next.options = next_wpt_options(ui, plan).into_iter().map(|(_, ident)| ident).collect();
        }
    }
    m
}

fn cost_index_mode(pd: Option<&PerformanceData>) -> Option<usize> {
    pd.and_then(|d| d.cost_index_mode).map(|m| match m {
        CostIndexMode::Lrc => 0,
        CostIndexMode::Econ => 1,
    })
}

/// 0 none, 1 request pending, 2 received: the company flight plan request the host runs.
pub const CPNY_FPLN_STATE: &str = "L:A380X_FMS_CPNY_FPLN_STATE";
pub const CPNY_FPLN_PLAN: &str = "L:A380X_FMS_CPNY_FPLN_PLAN";

pub fn init(ui: &UiState, src: &Sources) -> InitModel {
    let idx = plan_index(ui, src);
    let plan = src.plan(idx);
    let pd = src.perf(idx);
    let ident = |f: Option<&Fix>| f.map(|a| a.ident.clone());
    InitModel {
        captain: ui.captain,
        loaded_fp_index: idx as i32,
        flight_phase: src.phase,
        tmpy_active: idx == 1,
        eo_active: false,
        has_flight_plan: plan.is_some(),
        fp_is_active_or_copy_of_active: active_or_copy(src, idx),
        flight_number: plan.map(|p| p.flight_number.clone()).filter(|s| !s.is_empty()),
        from: ident(plan.and_then(|p| p.origin_airport.as_ref())).or_else(|| ui.pending_from.clone()),
        to: ident(plan.and_then(|p| p.destination_airport.as_ref())).or_else(|| ui.pending_to.clone()),
        altn: ident(plan.and_then(|p| p.alternate.as_deref()).and_then(|a| a.destination_airport.as_ref())),
        cpny_rte: None,
        altn_rte: None,
        crz_fl: pd.and_then(|d| d.cruise_flight_level),
        crz_temp: pd.and_then(|d| d.cruise_temperature()),
        crz_temp_pilot_entered: pd.is_some_and(|d| d.cruise_temperature_pilot_entry.is_some()),
        cost_index_mode: cost_index_mode(pd).unwrap_or(1),
        cost_index: pd.and_then(|d| d.cost_index),
        tropo: pd.map(|d| d.tropopause()),
        tropo_pilot_entered: pd.is_some_and(|d| d.pilot_tropopause.is_some()),
        trip_wind: pd.and_then(|d| d.pilot_trip_wind),
        trip_wind_pilot_entered: pd.is_some_and(|d| d.pilot_trip_wind.is_some()),
        fp_has_wind_entries: plan.is_some_and(|p| !p.winds.climb.is_empty() || !p.winds.descent.is_empty()),
        cpny_fpln_avail: src.var(CPNY_FPLN_STATE) == 2.,
        cpny_fpln_uplink_in_progress: src.var(CPNY_FPLN_STATE) == 1.,
        uplink_in_progress: src.var(CPNY_FPLN_STATE) == 1.,
        cpny_fpln_requested_for_plan: (src.var(CPNY_FPLN_STATE) > 0.).then(|| src.var(CPNY_FPLN_PLAN) as i32),
        engines_were_started: src.phase > PHASE_PREFLIGHT,
        has_active_fp_with_city_pair: src.plan(0).is_some_and(|p| p.origin_airport.is_some() && p.destination_airport.is_some()),
        open_dropdown: None,
        ..InitModel::default()
    }
}

pub fn fuel(ui: &UiState, src: &Sources) -> FuelModel {
    let idx = plan_index(ui, src);
    let plan = src.plan(idx);
    let pd = src.perf(idx);
    let fob = src.var("FUEL TOTAL QUANTITY WEIGHT") * 0.45359237;
    let zfw = pd.and_then(|d| t_to_kg(d.zero_fuel_weight));
    let block = pd.and_then(|d| t_to_kg(d.block_fuel));
    let taxi = pd.map(|d| d.taxi_fuel() * 1000.);
    FuelModel {
        captain: ui.captain,
        loaded_fp_index: idx as i32,
        flight_phase: src.phase,
        fp_is_active_or_copy_of_active: active_or_copy(src, idx),
        sec_active: idx >= 3,
        eo_active: false,
        metric: ui.metric,
        engines_were_started: src.phase > PHASE_PREFLIGHT,
        gross_weight_kg: zfw.map(|z| z + if fob > 0. { fob } else { block.unwrap_or(0.) }),
        fuel_on_board_kg: if fob > 0. { Some(fob) } else { None },
        zero_fuel_weight_kg: zfw,
        zero_fuel_weight_cg: pd.and_then(|d| d.zero_fuel_weight_center_of_gravity),
        block_fuel_kg: block,
        taxi_fuel_kg: taxi,
        taxi_fuel_pilot_entered: pd.is_some_and(|d| d.pilot_taxi_fuel.is_some()),
        route_reserve_kg: pd.and_then(|d| t_to_kg(d.pilot_route_reserve_fuel)),
        route_reserve_pilot_entered: pd.is_some_and(|d| d.pilot_route_reserve_fuel.is_some()),
        route_reserve_pct: pd.map(|d| d.route_reserve_fuel_percentage()),
        route_reserve_pct_pilot_entered: pd.is_some_and(|d| d.pilot_route_reserve_fuel_percentage.is_some()),
        alternate_fuel_kg: pd.and_then(|d| t_to_kg(d.alternate_fuel())),
        alternate_fuel_pilot_entered: pd.is_some_and(|d| d.pilot_alternate_fuel.is_some()),
        alternate_exists: plan.is_some_and(|p| p.alternate.as_ref().is_some_and(|a| a.destination_airport.is_some())),
        final_fuel_kg: pd.and_then(|d| t_to_kg(d.final_holding_fuel())),
        final_fuel_pilot_entered: pd.is_some_and(|d| d.pilot_final_holding_fuel.is_some()),
        final_time_min: pd.map(|d| d.final_holding_time()),
        final_time_pilot_entered: pd.is_some_and(|d| d.pilot_final_holding_time.is_some()),
        pax: pd.and_then(|d| d.pax_number),
        cost_index_mode: cost_index_mode(pd),
        cost_index: pd.and_then(|d| d.cost_index),
        takeoff_weight_kg: pd.and_then(|d| t_to_kg(d.takeoff_weight)),
        dest_icao: plan.and_then(|p| p.destination_airport.as_ref()).map(|a| a.ident.clone()),
        altn_icao: plan.and_then(|p| p.alternate.as_deref()).and_then(|a| a.destination_airport.as_ref()).map(|a| a.ident.clone()).unwrap_or_else(|| "NONE".into()),
        min_dest_fuel_kg: pd.and_then(|d| t_to_kg(d.minimum_destination_fuel_on_board())),
        min_dest_pilot_entered: pd.is_some_and(|d| d.pilot_minimum_destination_fuel_on_board.is_some()),
        open_dropdown: None,
        ..FuelModel::default()
    }
}

/// FBW `MfdFmsPerf`: the tab the page opens on follows the flight phase.
pub fn perf_default_tab(phase: u8) -> usize {
    match phase {
        0 | 1 => 0,
        2 => 1,
        3 => 2,
        4 => 3,
        5 => 4,
        _ => 5,
    }
}

/// FBW opens PERF on the phase tab for the active plan and its copies, else on T.O.
pub fn perf_tab_default(src: &Sources, idx: usize) -> usize {
    if active_or_copy(src, idx) {
        perf_default_tab(src.phase)
    } else {
        0
    }
}

pub fn perf(ui: &UiState, src: &Sources) -> PerfModel {
    let idx = plan_index(ui, src);
    let plan = src.plan(idx);
    let pd = src.perf(idx);
    let d = |f: fn(&PerformanceData) -> Option<f64>| pd.and_then(f);
    let b = |f: fn(&PerformanceData) -> bool| pd.is_some_and(f);
    PerfModel {
        captain: ui.captain,
        loaded_fp_index: idx as i32,
        active_flight_phase: src.phase,
        is_active_or_copy: active_or_copy(src, idx),
        eo_active: false,
        sec_active: idx >= 3,
        metric: ui.metric,
        selected_tab: ui.tab(Page::Perf, perf_tab_default(src, idx)),
        open_dropdown: None,
        crz_fl: d(|p| p.cruise_flight_level),
        crz_fl_mandatory: src.phase < PHASE_DESCENT,
        cost_index: d(|p| p.cost_index),
        cost_index_mode: cost_index_mode(pd),
        trans_alt: d(|p| p.transition_altitude()),
        trans_alt_from_database: b(|p| p.transition_altitude_is_from_database()),
        trans_fl: d(|p| p.transition_level()),
        trans_fl_from_database: b(|p| p.transition_level_is_from_database()),
        thr_red_alt: d(|p| p.thrust_reduction_altitude()),
        thr_red_alt_pilot: b(|p| p.pilot_thrust_reduction_altitude.is_some()),
        accel_alt: d(|p| p.acceleration_altitude()),
        accel_alt_pilot: b(|p| p.pilot_acceleration_altitude.is_some()),
        eo_accel_alt: d(|p| p.engine_out_acceleration_altitude()),
        eo_accel_alt_pilot: b(|p| p.pilot_engine_out_acceleration_altitude.is_some()),
        noise_enabled: b(|p| p.noise_enabled),
        noise_n1: d(|p| p.noise_n1),
        noise_speed: d(|p| p.noise_speed),
        noise_end_altitude: d(|p| p.noise_end_altitude),
        origin_runway_ident: plan.and_then(|p| p.origin_runway.as_ref()).map(|r| format!("{:<4}", runway_short(r))).unwrap_or_else(|| "---".into()),
        takeoff_shift_disabled: plan.is_none_or(|p| p.origin_runway.is_none()),
        takeoff_shift_m: d(|p| p.takeoff_shift),
        v1: d(|p| p.v1),
        vr: d(|p| p.vr),
        v2: d(|p| p.v2),
        takeoff_flaps_index: pd.and_then(|p| p.takeoff_flaps).map(|f| f as usize),
        takeoff_power_setting: pd.map(|p| p.takeoff_power_setting as usize),
        takeoff_derated: pd.map(|p| p.takeoff_derated_setting as usize),
        takeoff_ths_for: d(|p| p.takeoff_ths_for),
        takeoff_packs: pd.and_then(|p| p.takeoff_packs).map(|v| v as usize),
        takeoff_anti_ice: pd.and_then(|p| p.takeoff_anti_ice).map(|v| v as usize),
        flex_temp: d(|p| p.flex_takeoff_temperature),
        climb_derated: pd.and_then(|p| p.climb_derated).map(|v| v as usize),
        climb_presel_speed: d(|p| p.preselected_climb_speed),
        speed_constraint_speed: d(|p| p.climb_speed_limit_speed),
        speed_constraint_altitude: d(|p| p.climb_speed_limit_altitude),
        cruise_presel_speed: d(|p| p.preselected_cruise_speed),
        dest_ident: plan.and_then(|p| p.destination_airport.as_ref()).map(|a| a.ident.clone()).unwrap_or_default(),
        descent_cabin_rate: d(|p| p.descent_cabin_rate),
        des_managed_spd: d(|p| p.pilot_managed_descent_speed),
        des_managed_mach: d(|p| p.pilot_managed_descent_mach),
        appr_ident: plan.and_then(|p| p.approach.as_ref()).map(|a| a.ident.clone()).unwrap_or_default(),
        dest_airport_missing: plan.is_none_or(|p| p.destination_airport.is_none()),
        appr_wind_direction: d(|p| p.approach_wind_direction),
        appr_wind_magnitude: d(|p| p.approach_wind_magnitude),
        appr_temperature: d(|p| p.approach_temperature),
        appr_qnh: d(|p| p.approach_qnh),
        appr_qnh_hpa: pd.and_then(|p| p.approach_qnh).is_none_or(|q| q >= 100.),
        appr_params_mandatory: src.phase >= PHASE_DESCENT,
        appr_baro_min: d(|p| p.approach_baro_minimum),
        appr_radio_min: d(|p| p.approach_radio_minimum),
        appr_vapp: d(|p| p.pilot_vapp),
        appr_vapp_pilot: b(|p| p.pilot_vapp.is_some()),
        appr_flaps3: b(|p| p.approach_flaps_three_selected),
        missed_thr_red_alt: d(|p| p.missed_thrust_reduction_altitude()),
        missed_thr_red_pilot: b(|p| p.pilot_missed_thrust_reduction_altitude.is_some()),
        missed_accel_alt: d(|p| p.missed_acceleration_altitude()),
        missed_accel_pilot: b(|p| p.pilot_missed_acceleration_altitude.is_some()),
        missed_eo_accel_alt: d(|p| p.missed_engine_out_acceleration_altitude()),
        missed_eo_accel_pilot: b(|p| p.pilot_missed_engine_out_acceleration_altitude.is_some()),
        ..PerfModel::default()
    }
}

fn wind_alt(alt: Option<f64>, fl: bool) -> String {
    match alt {
        Some(a) if fl => format!("FL{:03.0}", a / 100.),
        Some(a) => format!("{a:.0}"),
        None => "-----".into(),
    }
}

fn wind_rows(entries: &[fms::winds::WindEntry], n: usize, blank: &str, trans: f64) -> Vec<WindRow> {
    let mut rows: Vec<WindRow> = entries
        .iter()
        .take(n)
        .map(|e| WindRow {
            alt: wind_alt(e.altitude, e.altitude.is_some_and(|a| a >= trans)),
            dir: e.vector.direction.map(|d| format!("{d:03.0}")).unwrap_or_else(|| "---".into()),
            spd: e.vector.magnitude.map(|s| format!("{s:03.0}")).unwrap_or_else(|| "---".into()),
            in_fp: true,
        })
        .collect();
    while rows.len() < n {
        rows.push(WindRow::blank(blank));
    }
    rows
}

pub fn wind(ui: &UiState, src: &Sources) -> WindModel {
    let idx = plan_index(ui, src);
    let plan = src.plan(idx);
    let trans = src.perf(idx).and_then(|d| d.transition_altitude()).unwrap_or(18000.);
    let mut m = WindModel { cap_or_fo: ui.side().into(), secondary: idx >= 3, ..WindModel::default() };
    m.selected_tab = ui.tab(Page::Wind, 1);
    if let Some(p) = plan {
        m.climb = wind_rows(&p.winds.climb, 5, "-----", trans);
        m.descent = wind_rows(&p.winds.descent, 5, "-----", trans);
        m.alternate_dir = p.winds.alternate.direction.map(|d| format!("{d:03.0}")).unwrap_or_else(|| "---".into());
        m.alternate_spd = p.winds.alternate.magnitude.map(|s| format!("{s:03.0}")).unwrap_or_else(|| "---".into());
        let legs = p.all_legs();
        let from = p.active_leg_index.max(0) as usize;
        m.waypoints = legs.iter().skip(from).filter_map(|e| e.leg()).map(|l| l.ident.clone()).collect();
        if let Some(i) = ui.revised_leg.and_then(|i| p.leg_at(i)) {
            m.selected_waypoint = i.ident.clone();
            let cw = wind_rows(&i.cruise_wind_entries, 4, "---", 0.);
            m.cruise = cw;
        }
        if let Some(fl) = src.perf(idx).and_then(|d| d.cruise_flight_level) {
            m.cruise_temperature_fl = format!("FL{fl:03.0}");
        }
    }
    m.disabled_due_to_phase = src.phase >= PHASE_DESCENT && idx <= 1;
    m.return_button_visible = ui.uri.extra.starts_with("withReturn");
    m.history_button_visible = !m.secondary;
    m
}

fn dm(v: f64, deg_w: usize, pos: char, neg: char) -> String {
    let a = v.abs();
    let deg = a.floor();
    let min = (a - deg) * 60.;
    format!("{:0w$}\u{b0}{:05.2}{}", deg as i64, min, if v >= 0. { pos } else { neg }, w = deg_w)
}

/// FBW `coordinateToString` for the POSITION pages: `DD°MM.MMN/DDD°MM.MMW`.
pub fn position_text(lat: f64, lon: f64) -> String {
    format!("{}/{}", dm(lat, 2, 'N', 'S'), dm(lon, 3, 'E', 'W'))
}

pub fn position_monitor(ui: &UiState, src: &Sources) -> PositionMonitorModel {
    let lat = src.var("PLANE LATITUDE");
    let lon = src.var("PLANE LONGITUDE");
    let mut m = PositionMonitorModel::default();
    m.return_visible = ui.uri.extra == "withReturn";
    if lat != 0. || lon != 0. {
        let p = position_text(lat, lon);
        m.position1 = p.clone();
        m.position2 = p.clone();
        m.gnss1 = p.clone();
        m.gnss2 = p.clone();
        m.ir = [p.clone(), p.clone(), p.clone()];
        m.ir_deviation = ["0.0".into(), "0.0".into(), "0.0".into()];
        m.mix_irs = p;
        // GENERIC: GPS primary accuracy, FBW's navigation provider value is not on a variable.
        m.epe = 0.05;
        m.accuracy_high = true;
        m.position1_mode = "GPS".into();
        m.position2_mode = "GPS".into();
    }
    m
}

pub fn position_irs(_ui: &UiState, _src: &Sources) -> PositionIrsModel {
    PositionIrsModel::default()
}

pub fn position_navaids(ui: &UiState, _src: &Sources) -> PositionNavaidsModel {
    PositionNavaidsModel { return_visible: matches!(ui.uri.extra.as_str(), "withReturn" | "nav"), ..PositionNavaidsModel::default() }
}

pub fn sec_index(ui: &UiState, src: &Sources) -> SecIndexModel {
    let mut m = SecIndexModel { selected_tab: ui.tab(Page::SecIndex, 0), cap_or_fo: ui.side().into(), ..Default::default() };
    for i in 0..3 {
        let plan = src.plan(3 + i);
        let mut t = SecTab { import_items: import_items(i + 1), cpny_label: "CPNY F-PLN".into(), cpny_disabled: true, ..Default::default() };
        if let Some(p) = plan {
            t.exists = true;
            let f = p.origin_airport.as_ref().map(|a| a.ident.as_str()).unwrap_or("");
            let to = p.destination_airport.as_ref().map(|a| a.ident.as_str()).unwrap_or("");
            t.city_pair = format!("{f}/{to}");
            let source = if p.flags & FLAG_COPIED_FROM_ACTIVE != 0 { "ACTIVE" } else { "PILOT" };
            t.created = created_label(true, None, source);
            t.swap_visible = true;
            let enroute = p.seg(fms::plan::SegmentKind::Enroute);
            t.route = enroute.legs.iter().filter_map(|e| e.leg()).map(|l| (if l.annotation.is_empty() { "DCT".to_string() } else { l.annotation.clone() }, l.ident.clone())).collect();
        } else {
            t.created = created_label(false, None, "");
        }
        t.import_open = ui.open.as_deref() == Some(&format!("{}_MFD_sec{}index_import", ui.side(), 3 + i));
        m.tabs[i] = t;
    }
    m
}

fn alt_text(v: f64, trans: Option<f64>) -> String {
    if trans.is_some_and(|t| v >= t) {
        format!("FL{:03.0}", v / 100.)
    } else {
        format!("{v:.0}")
    }
}

/// FBW `MfdFmsFplnVertRev.onNewData`: the XF legs of the revised plan from its active leg.
pub fn vert_rev_legs(ui: &UiState, src: &Sources) -> Vec<usize> {
    let idx = plan_index(ui, src);
    let Some(p) = revised_plan(ui, src.plan(idx)) else { return Vec::new() };
    let from = p.active_leg_index.max(0) as usize;
    p.all_legs().iter().enumerate().skip(from).filter(|(_, e)| e.leg().is_some_and(|l| l.is_xf())).map(|(k, _)| k).collect()
}

/// The revised leg when the page can show it, else the first leg of the list.
pub fn vert_rev_selected(ui: &UiState, src: &Sources) -> Option<usize> {
    let legs = vert_rev_legs(ui, src);
    ui.revised_leg.filter(|l| legs.contains(l)).or_else(|| legs.first().copied())
}

pub fn vert_rev(ui: &UiState, src: &Sources) -> VertRevModel {
    let idx = plan_index(ui, src);
    let plan = src.plan(idx);
    let pd = src.perf(idx);
    let trans = pd.and_then(|d| d.transition_altitude());
    let tab = match ui.uri.extra.as_str() {
        "rta" => 0,
        "spd" => 1,
        "cms" => 2,
        "alt" => 3,
        "step-alts" => 4,
        _ => 1,
    };
    let mut m = VertRevModel { cap_or_fo: ui.side().into(), tmpy_active: idx == 1, sec_active: idx >= 3, preflight: src.phase == PHASE_PREFLIGHT, ..Default::default() };
    m.selected_tab = ui.tab(Page::VertRev, tab);
    let Some(p) = plan else { return m };
    let p = revised_plan(ui, plan).unwrap_or(p);
    m.waypoints = vert_rev_legs(ui, src).into_iter().filter_map(|i| p.leg_at(i)).map(|l| l.ident.clone()).collect();
    if let Some(fl) = pd.and_then(|d| d.cruise_flight_level) {
        m.crz_fl = format!("FL{fl:03.0}");
    }
    if let Some(l) = vert_rev_selected(ui, src).and_then(|i| p.leg_at(i)) {
        m.selected_waypoint = l.ident.clone();
        m.spd_cstr_disabled = false;
        m.alt_cstr_disabled = false;
        if let Some(s) = l.speed_constraint() {
            m.spd_cstr = format!("{:.0}", s.speed);
            m.delete_spd_cstr_disabled = false;
        }
        if let Some(a) = l.altitude_constraint() {
            m.alt_cstr = alt_text(a.altitude1, trans);
            m.delete_alt_cstr_disabled = false;
            m.alt_option = Some(match a.descriptor.as_str() {
                "+" => 1,
                "-" => 2,
                "B" => 3,
                _ => 0,
            });
        }
    }
    m.speed_limit_speed = pd.and_then(|d| d.climb_speed_limit_speed).map(|v| format!("{v:.0}")).unwrap_or_else(|| "---".into());
    m.speed_limit_alt = pd.and_then(|d| d.climb_speed_limit_altitude).map(|v| alt_text(v, trans)).unwrap_or_else(|| "-----".into());
    m.speed_limit_pilot = pd.is_some_and(|d| d.is_climb_speed_limit_pilot_entered);
    let steps: Vec<StepLine> = p
        .all_legs()
        .iter()
        .filter_map(|e| e.leg())
        .filter_map(|l| l.cruise_step.as_ref().map(|s| (l, s)))
        .take(5)
        .map(|(l, s)| StepLine { visible: true, wpt: l.ident.clone(), alt: format!("FL{:03.0}", s.to_altitude / 100.), ..Default::default() })
        .collect();
    for (i, s) in steps.into_iter().enumerate() {
        m.step_lines[i] = s;
    }
    m
}

pub fn hold(ui: &UiState, src: &Sources) -> HoldModel {
    let idx = plan_index(ui, src);
    let mut m = HoldModel { cap_or_fo: ui.side().into(), tmpy_active: idx == 1, active_or_tmpy: idx <= 1, metric: ui.metric, ..Default::default() };
    let Some(l) = ui.revised_leg.and_then(|i| src.plan(idx).and_then(|p| p.leg_at(i))) else { return m };
    m.waypoint_ident = l.ident.clone();
    let (h, computed) = match (&l.modified_hold, &l.default_hold) {
        (Some(h), _) => (Some(h), false),
        (None, Some(h)) => (Some(h), true),
        _ => (None, true),
    };
    m.hold_type = mfd_pages_c::fpln_rev::hold_type_label(computed, !computed).to_string();
    if let Some(h) = h {
        m.inbound_course = h.inbound_magnetic_course;
        m.turn = h.turn_direction.map(|c| if c == 'L' { 0 } else { 1 });
        m.leg_parameter = if h.distance.is_some() { Some(1) } else { Some(0) };
        m.leg_time = h.time;
        m.leg_distance = h.distance;
    }
    m
}

/// FBW `MfdFmsFplnDirectTo.onNewData`: the XF legs of the active plan from the active leg to the missed approach, by leg index.
pub fn dir_to_legs(src: &Sources) -> Vec<usize> {
    let Some(p) = src.plan(0) else { return Vec::new() };
    let from = p.active_leg_index.max(0) as usize;
    let missed = p.first_missed_approach_leg_index();
    p.all_legs().iter().enumerate().skip(from).take(missed.saturating_sub(from)).filter(|(_, e)| e.leg().is_some_and(|l| l.is_xf())).map(|(k, _)| k).collect()
}

pub fn dir_to(ui: &UiState, src: &Sources) -> DirectToModel {
    let mut m = DirectToModel { cap_or_fo: ui.side().into(), tmpy_active: src.tmpy(), option: Some(ui.dir_to_option), ..Default::default() };
    if let Some(p) = src.plan(0) {
        m.waypoints = dir_to_legs(src).into_iter().filter_map(|i| p.leg_at(i)).map(|l| l.ident.clone()).collect();
    }
    m.selected = ui.dir_to_selected;
    m.free_text = ui.dir_to_text.clone();
    m.editing = ui.open.as_deref().is_some_and(|o| o.ends_with("_MFD_directToDropdown"));
    m
}

pub fn airways(ui: &UiState, src: &Sources) -> AirwaysModel {
    let idx = plan_index(ui, src);
    let mut m = AirwaysModel { tmpy_active: idx == 1, ..Default::default() };
    if let Some(pa) = src.plan(idx).and_then(|p| p.pending_airways.as_ref()) {
        m.revised_fix = pa.revised_waypoint.ident.clone();
        for e in &pa.elements {
            m.lines.push(AirwayLine {
                via: Some(if e.is_dct { "DCT".to_string() } else { e.airway.as_ref().map(|a| a.ident.clone()).unwrap_or_default() }),
                via_disabled: true,
                to: e.to.as_ref().map(|f| f.ident.clone()),
                to_disabled: true,
            });
        }
    } else if let Some(l) = ui.revised_leg.and_then(|i| src.plan(idx).and_then(|p| p.leg_at(i))) {
        m.revised_fix = l.ident.clone();
    }
    m.lines.push(AirwayLine::default());
    m.scroll_down_disabled = true;
    m.scroll_up_disabled = true;
    m
}

fn proc_menu(labels: Vec<String>, disabled: bool) -> ProcMenu {
    ProcMenu { disabled, items: labels.into_iter().map(|l| (l, false)).collect() }
}

fn with_none(labels: Vec<String>) -> Vec<String> {
    let mut v = vec!["NONE".to_string()];
    v.extend(labels);
    v
}

/// FBW `ident.substring(4)`: the runway designation without the airport.
pub fn runway_short(r: &Runway) -> String {
    let id = r.ident();
    if let Some(a) = r.fix.airport_ident.as_deref().filter(|a| id.starts_with(*a)) {
        return id[a.len()..].to_string();
    }
    if let Some(rest) = id.strip_prefix("RW") {
        return rest.to_string();
    }
    id.get(4..).unwrap_or(id).to_string()
}

/// FBW sorts `availableOriginRunways` / `availableDestinationRunways` by ident.
pub fn sorted_runways(list: &[Runway]) -> Vec<&Runway> {
    let mut v: Vec<&Runway> = list.iter().collect();
    v.sort_by(|a, b| a.ident().cmp(b.ident()));
    v
}

pub fn sorted_departures(p: &FlightPlan) -> Vec<&Procedure> {
    let mut v: Vec<&Procedure> = p.available_departures.iter().collect();
    v.sort_by(|a, b| a.ident.cmp(&b.ident));
    v
}

fn approach_rank(t: i32) -> u8 {
    match t {
        5 => 3,
        7 => 6,
        1 => 7,
        18 => 8,
        15 => 9,
        10 => 11,
        12 => 12,
        2 | 16 => 14,
        9 | 11 => 16,
        _ => 17,
    }
}

/// FBW `MfdFmsFplnArr`: approaches with a runway, by runway then approach type.
pub fn approach_options(p: &FlightPlan) -> Vec<&Procedure> {
    let mut v: Vec<&Procedure> = p.available_approaches.iter().filter(|a| a.runway_ident.as_deref().is_some_and(|r| !r.is_empty())).collect();
    v.sort_by(|a, b| a.runway_ident.cmp(&b.runway_ident).then(approach_rank(a.approach_type).cmp(&approach_rank(b.approach_type))));
    v
}

/// FBW `MfdFmsFplnArr`: only STARs whose runway transitions fit the approach runway.
pub fn star_options(p: &FlightPlan) -> Vec<&Procedure> {
    let rw = p.approach.as_ref().and_then(|a| a.runway_ident.as_deref());
    p.available_arrivals
        .iter()
        .filter(|a| {
            a.runway_transitions.is_empty()
                || a.runway_transitions.iter().any(|t| {
                    Some(t.ident.as_str()) == rw || (t.ident.chars().nth(4) == Some('B') && rw.is_some_and(|r| t.ident.get(..4) == r.get(..4)))
                })
        })
        .collect()
}

/// FBW `MfdFmsFplnArr`: with an arrival that has runway transitions, only VIAs that join its last fix.
pub fn via_options(p: &FlightPlan) -> Vec<&Transition> {
    let arrival_ends: Vec<Option<&str>> = p
        .arrival
        .get()
        .map(|a| {
            a.runway_transitions
                .iter()
                .map(|t| t.legs.last().or_else(|| a.common_legs.last()).and_then(|l| l.waypoint.as_ref()).map(|w| w.database_id.as_str()))
                .collect()
        })
        .unwrap_or_default();
    p.available_approach_vias
        .iter()
        .filter(|v| arrival_ends.is_empty() || arrival_ends.iter().any(|e| e.is_some() && *e == v.legs.first().and_then(|l| l.waypoint.as_ref()).map(|w| w.database_id.as_str())))
        .collect()
}

pub fn proc_target_is_alternate(ui: &UiState, src: &Sources, idx: usize) -> bool {
    ui.revised_altn && src.plan(idx).is_some_and(|p| p.alternate.is_some())
}

/// FBW `isAltn && loadedAlternateFlightPlan ? alternate : loaded`: the plan the DEPARTURE and ARRIVAL pages edit.
pub fn proc_plan<'a>(ui: &UiState, src: &Sources<'a>, idx: usize) -> Option<&'a FlightPlan> {
    let p = src.plan(idx)?;
    if ui.revised_altn {
        if let Some(a) = p.alternate.as_deref() {
            return Some(a);
        }
    }
    Some(p)
}

fn nbsp_pad(s: &str, width: usize, left: bool) -> String {
    let n = width.saturating_sub(s.chars().count());
    let pad = "\u{a0}".repeat(n);
    if left {
        format!("{pad}{s}")
    } else {
        format!("{s}{pad}")
    }
}

fn runway_label(r: &Runway, metric: bool, nbsp: bool) -> String {
    let (len, unit) = if metric { (r.length, "M") } else { (r.length / 0.3048, "FT") };
    let num = format!("{len:.0}");
    let short = runway_short(r);
    if nbsp {
        format!("{}\u{a0}{}{} ", nbsp_pad(&short, 3, false), nbsp_pad(&num, 5, true), unit)
    } else {
        let pad = |s: &str, w: usize, left: bool| nbsp_pad(s, w, left).replace('\u{a0}', " ");
        format!("{} {}{}", pad(&short, 3, false), pad(&num, 5, true), unit)
    }
}

pub fn departure(ui: &UiState, src: &Sources) -> DepartureModel {
    let idx = plan_index(ui, src);
    let mut m = DepartureModel { cap_or_fo: ui.side().into(), tmpy_active: idx == 1, sec_active: idx >= 3, metric: ui.metric, ..Default::default() };
    let Some(p) = proc_plan(ui, src, idx) else { return m };
    m.from_icao = p.origin_airport.as_ref().map(|a| a.ident.clone()).unwrap_or_else(|| "----".into());
    if p.origin_airport.is_none() {
        return m;
    }
    m.freq = "---.--".into();
    match &p.origin_runway {
        Some(r) => {
            m.rwy_ident = runway_short(r);
            m.rwy_length_m = r.length;
            m.rwy_crs = format!("{:03.0}", r.magnetic_bearing);
            m.eosid = "NONE".into();
        }
        None => {
            m.rwy_ident = "---".into();
            m.rwy_crs = "---".into();
            m.eosid = "------".into();
        }
    }
    m.sid = match &p.departure {
        Sel::Set(d) => d.ident.clone(),
        _ if p.available_departures.is_empty() => "NONE".into(),
        _ => "------".into(),
    };
    let dep_trans = p.departure.get().map(|d| d.enroute_transitions.as_slice()).unwrap_or(&[]);
    m.trans = match &p.departure_enroute_transition {
        Sel::Set(t) => t.ident.clone(),
        _ if p.departure.get().is_some() && dep_trans.is_empty() => "NONE".into(),
        _ => "------".into(),
    };
    m.rwy = proc_menu(sorted_runways(&p.available_origin_runways).into_iter().map(|r| runway_label(r, ui.metric, true)).collect(), false);
    let sids: Vec<String> = if p.origin_runway.is_some() && !p.available_departures.is_empty() {
        with_none(sorted_departures(p).into_iter().map(|d| d.ident.clone()).collect())
    } else {
        Vec::new()
    };
    m.sid_menu = proc_menu(sids, p.origin_runway.is_none());
    let trans: Vec<String> = if dep_trans.is_empty() { Vec::new() } else { with_none(dep_trans.iter().map(|t| t.ident.clone()).collect()) };
    m.trans_menu = proc_menu(trans, p.departure.get().is_none());
    m
}

pub fn arrival(ui: &UiState, src: &Sources) -> ArrivalModel {
    let idx = plan_index(ui, src);
    let mut m = ArrivalModel { cap_or_fo: ui.side().into(), tmpy_active: idx == 1, sec_active: idx >= 3, metric: ui.metric, ..Default::default() };
    let Some(p) = proc_plan(ui, src, idx) else { return m };
    m.to_icao = p.destination_airport.as_ref().map(|a| a.ident.clone()).unwrap_or_else(|| "----".into());
    if p.destination_airport.is_none() {
        return m;
    }
    match &p.destination_runway {
        Some(r) => {
            m.rwy_ident = runway_short(r);
            m.rwy_length_m = r.length;
            m.rwy_crs = format!("{:03.0}", r.magnetic_bearing);
        }
        None => {
            m.rwy_ident = "---".into();
            m.rwy_crs = "---".into();
        }
    }
    m.approach = match &p.approach {
        Some(a) => a.ident.clone(),
        None if !p.available_approaches.is_empty() => "------".into(),
        None => "NONE".into(),
    };
    m.via = match &p.approach_via {
        Sel::Set(t) => t.ident.clone(),
        _ if p.approach.as_ref().is_none_or(|a| !a.enroute_transitions.is_empty()) => "------".into(),
        _ => "NONE".into(),
    };
    m.star = match &p.arrival {
        Sel::Set(a) => a.ident.clone(),
        _ if p.available_arrivals.is_empty() => "NONE".into(),
        _ => "------".into(),
    };
    let arr_trans = p.arrival.get().map(|a| a.enroute_transitions.as_slice()).unwrap_or(&[]);
    m.trans = match &p.arrival_enroute_transition {
        Sel::Set(t) => t.ident.clone(),
        _ if p.arrival.get().is_some() && arr_trans.is_empty() => "NONE".into(),
        _ => "------".into(),
    };
    m.rwy = proc_menu(sorted_runways(&p.available_destination_runways).into_iter().map(|r| runway_label(r, ui.metric, false)).collect(), false);
    let apprs: Vec<String> = if p.available_approaches.is_empty() { Vec::new() } else { with_none(approach_options(p).into_iter().map(|a| a.ident.clone()).collect()) };
    m.appr_menu = proc_menu(apprs, p.available_approaches.is_empty());
    let vias: Vec<String> = if p.approach.is_some() && !p.available_approach_vias.is_empty() { with_none(via_options(p).into_iter().map(|t| t.ident.clone()).collect()) } else { Vec::new() };
    m.via_menu = proc_menu(vias, p.approach.is_none() || p.available_approach_vias.is_empty());
    let stars_ok = !p.available_arrivals.is_empty() && p.approach.is_some();
    let stars: Vec<String> = if stars_ok { with_none(star_options(p).into_iter().map(|a| a.ident.clone()).collect()) } else { Vec::new() };
    m.star_menu = proc_menu(stars, !stars_ok);
    let trans: Vec<String> = if arr_trans.is_empty() { Vec::new() } else { with_none(arr_trans.iter().map(|t| t.ident.clone()).collect()) };
    m.trans_menu = proc_menu(trans, p.arrival.get().is_none());
    m
}

pub fn surv_controls(_ui: &UiState, src: &Sources) -> SurvControlsModel {
    let code = src.var("TRANSPONDER CODE:1");
    SurvControlsModel {
        squawk_text: format!("{:04.0}", code.max(0.)),
        alt_rptg_on: true,
        xpdr_radio_color: "cyan".into(),
        tcas_radio_color: "cyan".into(),
        elevn_tilt_color: "cyan".into(),
        elevn_unit: "FT".into(),
        elevn_ft_text: "+0".into(),
        elevn_fl_text: "---".into(),
        tilt_text: "0.0".into(),
        gain_text: "50".into(),
        wxr_auto: true,
        pred_ws_auto: true,
        turb_auto: true,
        gain_auto: true,
        mode_wx: true,
        vd_auto: true,
        terr_on: !src.flag("L:A32NX_GPWS_TERR_OFF"),
        gpws_on: !src.flag("L:A32NX_GPWS_SYS_OFF"),
        gs_on: !src.flag("L:A32NX_GPWS_GS_OFF"),
        flap_on: !src.flag("L:A32NX_GPWS_FLAPS_OFF"),
        ..Default::default()
    }
}

pub fn surv_status(_ui: &UiState, _src: &Sources) -> SurvStatusModel {
    let mut m = SurvStatusModel::default();
    m.wxr_taws[0].active = true;
    m.xpdr_tcas[0].active = true;
    m
}

fn fcu_number(v: f64, dashes: bool, digits: usize, decimals: usize) -> String {
    if dashes {
        "-".repeat(digits)
    } else {
        format!("{:0w$.p$}", v, w = digits, p = decimals)
    }
}

pub fn fcu_afs(_ui: &UiState, src: &Sources) -> FcuBkupAfsModel {
    let mach = src.flag("L:A32NX_FCU_AFS_DISPLAY_MACH_MODE");
    let trk = src.flag("L:A32NX_FCU_AFS_DISPLAY_TRK_FPA_MODE");
    let spd = src.var("L:A32NX_FCU_AFS_DISPLAY_SPD_MACH_VALUE");
    let vs = src.var("L:A32NX_FCU_AFS_DISPLAY_VS_FPA_VALUE");
    FcuBkupAfsModel {
        backup_on: src.flag("L:A380X_FCU_BKUP_AFS_ACTIVE"),
        trk_fpa_mode: trk,
        metric: src.flag("L:A32NX_METRIC_ALT_TOGGLE"),
        mach_mode: mach,
        true_mode: src.flag("L:A32NX_FCU_AFS_DISPLAY_TRUE_MODE"),
        spd_text: if src.flag("L:A32NX_FCU_AFS_DISPLAY_SPD_MACH_DASHES") { "---".into() } else if mach { format!("{spd:.2}") } else { format!("{spd:03.0}") },
        hdg_text: fcu_number(src.var("L:A32NX_FCU_AFS_DISPLAY_HDG_TRK_VALUE"), src.flag("L:A32NX_FCU_AFS_DISPLAY_HDG_TRK_DASHES"), 3, 0),
        alt_text: format!("{:05.0}", src.var("L:A32NX_FCU_AFS_DISPLAY_ALT_VALUE")),
        vs_text: if src.flag("L:A32NX_FCU_AFS_DISPLAY_VS_FPA_DASHES") {
            "-----".into()
        } else if trk {
            format!("{vs:+.1}")
        } else {
            format!("{:+05.0}", vs)
        },
        loc_light: src.flag("L:A32NX_FCU_LOC_LIGHT_ON"),
        alt_light: src.flag("L:A32NX_FCU_ALT_LIGHT_ON"),
        appr_light: src.flag("L:A32NX_FCU_APPR_LIGHT_ON"),
        fd_light: src.flag("L:A32NX_FCU_FD_LIGHT_ON"),
        ap1_light: src.flag("L:A32NX_FCU_AP_1_LIGHT_ON"),
        ap2_light: src.flag("L:A32NX_FCU_AP_2_LIGHT_ON"),
        athr_light: src.flag("L:A32NX_FCU_ATHR_LIGHT_ON"),
        ..Default::default()
    }
}

pub fn fcu_efis(ui: &UiState, src: &Sources) -> FcuBkupEfisModel {
    let s = if ui.captain { "L" } else { "R" };
    let v = |n: &str| src.var(&format!("L:A32NX_FCU_EFIS_{s}_{n}"));
    let lit = |n: &str| v(&format!("{n}_LIGHT_ON")) != 0.;
    let inhg = v("DISPLAY_BARO_IS_INHG") != 0.;
    let baro = v("DISPLAY_BARO_VALUE");
    FcuBkupEfisModel {
        captain_side: ui.captain,
        backup_on: src.flag(&format!("L:A380X_FCU_BKUP_EFIS_{s}_ACTIVE")),
        baro_text: if inhg { format!("{baro:.2}") } else { format!("{baro:.0}") },
        baro_unit: if inhg { "IN.HG".into() } else { "HPA".into() },
        baro_std: v("DISPLAY_BARO_IS_STD") != 0.,
        ls: lit("LS"),
        vv: lit("VV"),
        cstr: lit("CSTR"),
        arpt: lit("ARPT"),
        traf: lit("TRAF"),
        filter: if lit("WPT") {
            1
        } else if lit("VORD") {
            2
        } else if lit("NDB") {
            3
        } else {
            0
        },
        overlay: if lit("WX") {
            1
        } else if lit("TERR") {
            2
        } else {
            0
        },
        ..Default::default()
    }
}

pub fn weight_to_kg(v: f64, metric: bool) -> f64 {
    v * weight_unit_kg(metric)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fms::navdb::ProcedureLeg;

    fn leg_at(ident: &str) -> ProcedureLeg {
        ProcedureLeg { waypoint: Some(Fix::lat_long(ident, 25., 55.)), ..Default::default() }
    }

    #[test]
    fn a_star_whose_runway_transitions_only_name_runways_offers_the_via_at_its_last_fix() {
        let mut p = FlightPlan::empty(0);
        p.arrival = Sel::Set(Procedure {
            ident: "IMPE3C".into(),
            common_legs: vec![leg_at("DB504"), leg_at("ULDOT")],
            runway_transitions: vec![Transition { ident: "OMDB30L".into(), ..Default::default() }, Transition { ident: "OMDB30R".into(), ..Default::default() }],
            ..Default::default()
        });
        p.available_approach_vias = vec![
            Transition { ident: "ULDOT".into(), legs: vec![leg_at("ULDOT")], ..Default::default() },
            Transition { ident: "RIBNA".into(), legs: vec![leg_at("RIBNA")], ..Default::default() },
        ];
        let idents: Vec<&str> = via_options(&p).iter().map(|v| v.ident.as_str()).collect();
        assert_eq!(idents, ["ULDOT"]);
    }
}

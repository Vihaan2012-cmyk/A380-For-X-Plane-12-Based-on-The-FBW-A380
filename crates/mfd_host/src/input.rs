use fms::geo::Coordinates;
use fms::navdb::{Fix, NavDatabase};
use fms::plan::AltitudeConstraint;
use fms::service::{Command as Plan, PlanRef};
use fms_data::{ClimbDerated, CostIndexMode, Entry, TakeoffAntiIce, TakeoffDerated, TakeoffPacks, TakeoffPowerSetting};
use fms_host::HostCommand;

use crate::model::{self, plan_index};
use crate::page::{fms_uri, header_menu_uri, sub_header_uri, sys_uri, Page};
use crate::sources::Sources;
use crate::ui::UiState;

#[derive(Clone, Debug)]
pub enum Out {
    Fms(HostCommand),
    Var(String, f64),
    CpnyFplnRequest(usize),
    CpnyFplnInsert(usize),
    CpnyFplnClear,
    KeyEvent(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Escape,
    Delete,
}

pub const FORMAT_ERROR: &str = "FORMAT ERROR";
pub const NOT_ALLOWED: &str = "NOT ALLOWED";
pub const NOT_IN_DATABASE: &str = "NOT IN DATABASE";
pub const OUT_OF_RANGE: &str = "ENTRY OUT OF RANGE";

const SCRATCHPAD_MAX: usize = 24;

fn perf(plan: usize, entry: Entry) -> Out {
    Out::Fms(HostCommand::Perf(fms_data::Command::Enter { plan, entry }))
}

fn plan_cmd(c: Plan) -> Out {
    Out::Fms(HostCommand::Plan(c))
}

fn strip_dup(key: &str) -> &str {
    key.split_once('#').map(|(k, _)| k).unwrap_or(key)
}

fn item_index(id: &str) -> Option<(&str, usize)> {
    let (p, i) = id.rsplit_once('_')?;
    Some((p, i.parse().ok()?))
}

fn ppos(src: &Sources) -> Coordinates {
    Coordinates::new(src.var("PLANE LATITUDE"), src.var("PLANE LONGITUDE"))
}

fn mag_var(src: &Sources) -> Option<f64> {
    let v = src.var("MAGVAR");
    if v == 0. {
        None
    } else {
        Some(v)
    }
}

fn category(ui: &UiState) -> String {
    match ui.uri.category.as_str() {
        c @ ("active" | "sec1" | "sec2" | "sec3") => c.to_string(),
        _ => "active".into(),
    }
}

/// The labelled page buttons that only navigate.
fn nav_button(ui: &UiState, label: &str) -> Option<String> {
    let cat = category(ui);
    let page = match label {
        "F-PLN" => "f-pln",
        "FUEL&LOAD" | "FUEL & LOAD" => "fuel-load",
        "PERF" => "perf",
        "INIT" => "init",
        "WIND" => "wind",
        "DIR TO" => return Some("fms/active/f-pln-direct-to".into()),
        "POSITION MONITOR" | "POS MONITOR" => return Some("fms/position/monitor".into()),
        _ => return None,
    };
    Some(fms_uri(&cat, page))
}

/// A press on the element with `data-act` = `act` (side prefix removed).
pub fn click(ui: &mut UiState, src: &Sources, act: &str) -> Vec<Out> {
    let (kind, key) = act.split_once(':').unwrap_or((act, ""));
    let mut out = Vec::new();
    let had_entry = ui.entry_message.take().is_some();
    ui.cleared_messages.retain(|m| src.messages.contains(m));
    if ui.focus.is_some() && !(kind == "field" && ui.focus.as_deref() == Some(act)) {
        out = blur(ui, src);
    }
    match kind {
        "psel" => {
            let i: usize = key.parse().unwrap_or(0);
            if ui.uri.sys == "fms" {
                ui.page_selector = if ui.page_selector == Some(i) { None } else { Some(i) };
                ui.open = None;
            } else if let Some(u) = sub_header_uri(&ui.uri.sys, i) {
                ui.navigate(u);
            }
        }
        "dd" | "bmenu" => {
            ui.page_selector = None;
            ui.menu_scroll = 0.;
            let opening = ui.open.as_deref() != Some(key);
            ui.open = if opening { Some(key.to_string()) } else { None };
            if opening && kind == "dd" && free_text_dropdown(key) {
                ui.focus = Some(act.to_string());
                ui.scratchpad.clear();
                ui.keyboard = true;
            }
            if opening && key.ends_with("_MFD_f-pln-arr-appr-btn") {
                ui.menu_scroll = appr_menu_scroll(ui, src);
            }
        }
        "item" => out.extend(item(ui, src, key)),
        "field" => ui.focus_field(act),
        "tab" => {
            let i = key.parse().unwrap_or(0);
            ui.tabs.insert(ui.page(), i);
            ui.close_popups();
        }
        "icon" => {
            ui.close_popups();
            if ui.page() == Page::Fpln {
                let (_, start, total) = model::fpln_visible_lines(ui, src);
                let down = strip_dup(key) == "down";
                let page = model::FPLN_PAGE_LINES;
                let next = if down { (start + page - 1).min(total.saturating_sub(page)) } else { (start + 1).saturating_sub(page) };
                ui.fpln_scroll = Some(next);
            }
        }
        "fpln" => out.extend(fpln_click(ui, src, key)),
        "radio" => out.extend(radio(ui, src, key)),
        "btn" if had_entry && strip_dup(key) == "CLEAR INFO" => ui.close_popups(),
        "btn" => out.extend(button(ui, src, strip_dup(key))),
        _ => ui.close_popups(),
    }
    out
}

/// FBW `apprButtonScrollTo`: the approach menu opens at the first approach of the destination runway, 36 px per item.
fn appr_menu_scroll(ui: &UiState, src: &Sources) -> f64 {
    let idx = plan_index(ui, src);
    let Some(p) = model::proc_plan(ui, src, idx) else { return 0. };
    let rw = p.destination_runway.as_ref().map(|r| r.ident());
    model::approach_options(p).iter().position(|a| a.runway_ident.as_deref() == rw).map_or(0., |i| (i + 1) as f64 * 36.)
}

fn free_text_dropdown(key: &str) -> bool {
    key.ends_with("_MFD_directToDropdown") || key.ends_with("_MFD_insertNextWptDropdown")
}

fn fpln_click(ui: &mut UiState, src: &Sources, key: &str) -> Vec<Out> {
    let mut parts = key.split(':');
    let col = parts.next().unwrap_or("");
    let line: usize = parts.next().and_then(|s| strip_dup(s).parse().ok()).unwrap_or(0);
    let (lines, _, _) = model::fpln_visible_lines(ui, src);
    let Some(d) = lines.get(line) else { return Vec::new() };
    let Some(leg) = d.original_leg_index else { return Vec::new() };
    ui.revised_leg = Some(leg);
    ui.revised_altn = d.is_altn;
    match col {
        "ident" => {
            ui.page_selector = None;
            ui.open = if ui.open.as_deref() == Some("revisions") { None } else { Some("revisions".into()) };
        }
        "time" | "spd" | "alt" if col != "time" || !d.is_altn => {
            let cat = category(ui);
            ui.navigate(&format!("fms/{cat}/f-pln-vert-rev/{}", match col {
                "time" => "rta",
                "spd" => "spd",
                _ => "alt",
            }));
        }
        _ => ui.close_popups(),
    }
    Vec::new()
}

fn revision(ui: &mut UiState, src: &Sources, i: usize) -> Vec<Out> {
    let idx = plan_index(ui, src);
    let items = model::revisions_menu(ui, src);
    ui.open = None;
    if items.get(i).is_none_or(|m| m.disabled) {
        return Vec::new();
    }
    let Some(leg) = ui.revised_leg else { return Vec::new() };
    let r = PlanRef::from_index(idx);
    let altn = ui.revised_altn;
    let cat = category(ui);
    match i {
        0 => {
            ui.navigate("fms/active/f-pln-direct-to");
            ui.dir_to_selected = model::dir_to_legs(src).iter().position(|&l| l == leg);
            vec![plan_cmd(Plan::DirectToLeg { ppos: ppos(src), true_track: src.var("GPS GROUND TRUE TRACK"), index: leg, mag_var: mag_var(src), with_abeam: false, plan: r })]
        }
        1 => {
            ui.insert_next_open = true;
            Vec::new()
        }
        2 => {
            let _ = altn;
            vec![plan_cmd(Plan::DeleteElementAt { index: leg, insert_discontinuity: false, plan: r })]
        }
        3 => {
            ui.navigate(&fms_uri(&cat, "f-pln-departure"));
            Vec::new()
        }
        4 => {
            ui.navigate(&fms_uri(&cat, "f-pln-arrival"));
            Vec::new()
        }
        6 => {
            ui.navigate(&fms_uri(&cat, "f-pln-hold"));
            Vec::new()
        }
        7 => {
            ui.navigate(&fms_uri(&cat, "f-pln-airways"));
            Vec::new()
        }
        9 => vec![plan_cmd(Plan::EnableAltn { index: leg, cruise_level: 100., plan: r })],
        10 => {
            ui.new_dest_open = true;
            Vec::new()
        }
        11 => {
            ui.navigate(&format!("fms/{cat}/f-pln-vert-rev/alt"));
            Vec::new()
        }
        13 => {
            ui.navigate(&format!("fms/{cat}/f-pln-vert-rev/step-alts"));
            Vec::new()
        }
        14 => {
            ui.navigate(&format!("fms/{cat}/wind/withReturn/{leg}"));
            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn item(ui: &mut UiState, src: &Sources, id: &str) -> Vec<Out> {
    let Some((prefix, i)) = item_index(id) else { return Vec::new() };
    let side = format!("{}_MFD_", ui.side());
    let name = prefix.strip_prefix(&side).unwrap_or(prefix);
    let idx = plan_index(ui, src);
    let r = PlanRef::from_index(idx);
    let pp = model::proc_plan(ui, src, idx);
    let alt = model::proc_target_is_alternate(ui, src, idx);
    let mut out = Vec::new();
    let set_dep = |id: Option<String>| plan_cmd(Plan::SetDeparture { id, plan: r, alternate: alt });
    let set_dep_trans = |id: Option<String>| plan_cmd(Plan::SetDepartureEnrouteTransition { id, plan: r, alternate: alt });
    let set_appr = |id: Option<String>| plan_cmd(Plan::SetApproach { id, plan: r, alternate: alt });
    let set_via = |id: Option<String>| plan_cmd(Plan::SetApproachVia { id, plan: r, alternate: alt });
    let set_star = |id: Option<String>| plan_cmd(Plan::SetArrival { id, plan: r, alternate: alt });
    let set_arr_trans = |id: Option<String>| plan_cmd(Plan::SetArrivalEnrouteTransition { id, plan: r, alternate: alt });
    match name {
        "fplnreq" => {
            ui.close_popups();
            return vec![if i == 0 { Out::CpnyFplnInsert(idx) } else { Out::CpnyFplnClear }];
        }
        "sysSelectorDropdown" => {
            ui.close_popups();
            ui.navigate(sys_uri(i));
            return out;
        }
        "pageSelectorActive" | "pageSelectorPosition" | "pageSelectorSecIndex" | "pageSelectorData" => {
            let sel = match name {
                "pageSelectorActive" => 0,
                "pageSelectorPosition" => 1,
                "pageSelectorSecIndex" => 2,
                _ => 3,
            };
            ui.close_popups();
            if let Some(u) = header_menu_uri(sel, i) {
                ui.navigate(u);
            }
            return out;
        }
        "revisionsMenu" => return revision(ui, src, i),
        "windbtn" => ui.efob_and_wind = !ui.efob_and_wind,
        "f-pln-dep-rwy-btn" => {
            if let Some(rw) = pp.and_then(|p| model::sorted_runways(&p.available_origin_runways).get(i).copied()) {
                out.push(plan_cmd(Plan::SetOriginRunway { runway: rw.ident().to_string(), plan: r, alternate: alt }));
                out.push(set_dep(None));
                out.push(set_dep_trans(None));
            }
        }
        "f-pln-dep-sid-btn" => {
            let id = if i == 0 { None } else { pp.and_then(|p| model::sorted_departures(p).get(i - 1).map(|d| d.database_id.clone())) };
            if i == 0 || id.is_some() {
                out.push(set_dep(id));
                out.push(set_dep_trans(None));
            }
        }
        "f-pln-dep-trans-btn" => {
            let id = if i == 0 { None } else { pp.and_then(|p| p.departure.get()).and_then(|d| d.enroute_transitions.get(i - 1)).map(|t| t.database_id.clone()) };
            if i == 0 || id.is_some() {
                out.push(set_dep_trans(id));
            }
        }
        "f-pln-arr-rwy-btn" => {
            if let Some(rw) = pp.and_then(|p| model::sorted_runways(&p.available_destination_runways).get(i).copied()) {
                out.push(plan_cmd(Plan::SetDestinationRunway { runway: rw.ident().to_string(), plan: r, alternate: alt }));
                out.push(set_appr(None));
                out.push(set_via(None));
            }
        }
        "f-pln-arr-appr-btn" => {
            let appr = if i == 0 { None } else { pp.and_then(|p| model::approach_options(p).get(i - 1).copied()) };
            if i == 0 || appr.is_some() {
                if let Some(a) = appr {
                    out.push(plan_cmd(Plan::SetDestinationRunway { runway: a.runway_ident.clone().unwrap_or_default(), plan: r, alternate: alt }));
                }
                out.push(set_appr(appr.map(|a| a.database_id.clone())));
                out.push(set_via(None));
            }
        }
        "f-pln-arr-via-btn" => {
            let id = if i == 0 { None } else { pp.and_then(|p| model::via_options(p).get(i - 1).map(|t| t.database_id.clone())) };
            if i == 0 || id.is_some() {
                out.push(set_via(id));
            }
        }
        "f-pln-arr-star-btn" => {
            let id = if i == 0 { None } else { pp.and_then(|p| model::star_options(p).get(i - 1).map(|a| a.database_id.clone())) };
            if i == 0 || id.is_some() {
                out.push(set_star(id));
                out.push(set_arr_trans(None));
            }
        }
        "f-pln-arr-trans-btn" => {
            let id = if i == 0 { None } else { pp.and_then(|p| p.arrival.get()).and_then(|a| a.enroute_transitions.get(i - 1)).map(|t| t.database_id.clone()) };
            if i == 0 || id.is_some() {
                out.push(set_arr_trans(id));
            }
        }
        "directToDropdown" => {
            if src.tmpy() {
                out.push(plan_cmd(Plan::TemporaryDelete));
            }
            let legs = model::dir_to_legs(src);
            if let Some(&index) = legs.get(i) {
                ui.dir_to_selected = Some(i);
                out.push(plan_cmd(Plan::DirectToLeg { ppos: ppos(src), true_track: src.var("GPS GROUND TRUE TRACK"), index, mag_var: mag_var(src), with_abeam: ui.dir_to_option == 1, plan: PlanRef::Active }));
            }
        }
        "insertNextWptDropdown" => {
            let target = model::next_wpt_options(ui, src.plan(idx)).get(i).map(|(leg, _)| *leg);
            let waypoint = target.and_then(|t| model::revised_plan(ui, src.plan(idx)).and_then(|p| p.leg_at(t))).and_then(|l| l.definition.waypoint.clone());
            if let (Some(index), Some(waypoint)) = (ui.revised_leg, waypoint) {
                out.push(plan_cmd(Plan::NextWaypoint { index, waypoint, plan: r }));
                ui.insert_next_open = false;
                ui.revised_leg = None;
                ui.revised_altn = false;
            }
        }
        "clbConstraintWptDropdown" | "altConstraintWptDropdown" => {
            if let Some(&leg) = model::vert_rev_legs(ui, src).get(i) {
                ui.revised_leg = Some(leg);
            }
        }
        n if n.starts_with("sec") && n.ends_with("index_import") => {
            let sec = n.trim_start_matches("sec").trim_end_matches("index_import").parse::<usize>().unwrap_or(3);
            if i == 0 {
                out.push(plan_cmd(Plan::SecondaryCopyFromActive { index: sec.saturating_sub(2).max(1) }));
            }
        }
        _ => out.extend(perf_dropdown(ui, src, name, i)),
    }
    ui.close_popups();
    out
}

/// PERF and INIT dropdowns, by the id prefix FBW gives them.
fn perf_dropdown(ui: &UiState, src: &Sources, name: &str, i: usize) -> Vec<Out> {
    let idx = plan_index(ui, src);
    let e = match name {
        "initCostIndexModeDropdown" | "clbCostIndexModeDropdown" | "crzCostIndexModeDropdown" | "desCostIndexModeDropdown" => {
            Entry::CostIndexMode(if i == 0 { CostIndexMode::Lrc } else { CostIndexMode::Econ })
        }
        "flapDropdown" if i < 3 => Entry::TakeoffFlaps(Some(i as u8 + 1)),
        "deratedDropdown" => Entry::TakeoffDerated(match i {
            1 => TakeoffDerated::D02,
            2 => TakeoffDerated::D03,
            3 => TakeoffDerated::D04,
            4 => TakeoffDerated::D05,
            _ => TakeoffDerated::D01,
        }),
        "deratedClbDropdown" => Entry::ClimbDerated(match i {
            1 => ClimbDerated::D01,
            2 => ClimbDerated::D02,
            3 => ClimbDerated::D03,
            4 => ClimbDerated::D04,
            5 => ClimbDerated::D05,
            _ => ClimbDerated::None,
        }),
        "packsDropdown" => Entry::TakeoffPacks(if i == 0 { TakeoffPacks::OffApu } else { TakeoffPacks::On }),
        "antiIceDropdown" => Entry::TakeoffAntiIce(match i {
            1 => TakeoffAntiIce::EngOnly,
            2 => TakeoffAntiIce::EngWings,
            _ => TakeoffAntiIce::Off,
        }),
        _ => return Vec::new(),
    };
    vec![perf(idx, e)]
}

fn radio(ui: &mut UiState, src: &Sources, key: &str) -> Vec<Out> {
    let Some((prefix, i)) = item_index(key) else { return Vec::new() };
    let side = format!("{}_MFD_", ui.side());
    let name = prefix.strip_prefix(&side).unwrap_or(prefix);
    let idx = plan_index(ui, src);
    match name {
        "directToOptionsRadio" => {
            if i == 0 {
                ui.dir_to_option = i;
            }
            Vec::new()
        }
        "toThrustSettingRadio" if i < 2 && src.phase < model::PHASE_TAKEOFF => {
            vec![perf(idx, Entry::TakeoffPowerSetting(if i == 0 { TakeoffPowerSetting::Toga } else { TakeoffPowerSetting::Flex }))]
        }
        "apprFlapsSettingsRadio" => vec![perf(idx, Entry::ApproachFlapsThree(i == 0))],
        "altCstrRadioButtons" => {
            let (Some(leg), Some(p)) = (ui.revised_leg, src.plan(idx)) else { return Vec::new() };
            let Some(c) = p.leg_at(leg).and_then(|l| l.altitude_constraint()) else { return Vec::new() };
            let descriptor = match i {
                1 => "+",
                2 => "-",
                _ => "@",
            };
            let constraint = AltitudeConstraint { descriptor: descriptor.into(), altitude1: c.altitude1, altitude2: None };
            vec![plan_cmd(Plan::SetPilotAltitudeConstraint { index: leg, descent: src.phase >= model::PHASE_CRUISE, constraint: Some(constraint), plan: PlanRef::from_index(idx) })]
        }
        _ => Vec::new(),
    }
}

fn reset_revised(ui: &mut UiState) {
    ui.revised_leg = None;
    ui.revised_altn = false;
}

/// Buttons whose target depends on the page they are on (FBW `onClick` handlers).
fn page_button(ui: &mut UiState, label: &str) -> bool {
    let cat = category(ui);
    let page = ui.page();
    let sec = ui.uri.sec();
    let tab = ui.tab(Page::SecIndex, 0) + 1;
    let uri: String = match (page, label) {
        (Page::SecIndex, "F-PLN") => format!("fms/sec{tab}/f-pln"),
        (Page::SecIndex, "PERF") => format!("fms/sec{tab}/perf"),
        (Page::SecIndex, "WIND") => format!("fms/sec{tab}/wind/withReturn"),
        (Page::SecIndex, "FUEL&LOAD") => format!("fms/sec{tab}/fuel-load"),
        (Page::SecIndex, "INIT") => format!("fms/sec{tab}/init"),
        (Page::Init, "ACFT STATUS") => "fms/data/status".into(),
        (Page::Init, "WIND") => format!("fms/{cat}/wind/withReturn"),
        (Page::Init, "IRS") | (Page::PosMonitor, "IRS") => "fms/position/irs".into(),
        (Page::Init, "NAVAIDS") => "fms/position/navaids/withReturn".into(),
        (Page::PosMonitor, "NAVAIDS") => "fms/position/navaids/nav".into(),
        (Page::Init, "FUEL&LOAD") => format!("fms/{cat}/fuel-load"),
        (Page::Init, "T.O. PERF") => format!("fms/{cat}/perf/to"),
        (Page::Init | Page::Fpln, "RETURN") if sec.is_some() => format!("fms/sec/index/{}", sec.unwrap_or(1)),
        (Page::Perf, "SPD CSTR") => format!("fms/{cat}/f-pln-vert-rev/spd"),
        (Page::Perf, "STEP ALTs") => format!("fms/{cat}/f-pln-vert-rev/step-alts"),
        (Page::Perf, "POS MONITOR") => "fms/position/monitor/withReturn".into(),
        (Page::DirTo | Page::Airways, "RETURN" | "TMPY F-PLN") => fms_uri(&cat, "f-pln"),
        (Page::Hold | Page::Departure | Page::Arrival, "RETURN") => {
            reset_revised(ui);
            "back".into()
        }
        (Page::Hold | Page::Departure | Page::Arrival, "TMPY F-PLN") => {
            reset_revised(ui);
            fms_uri(&cat, "f-pln")
        }
        _ => return false,
    };
    ui.navigate(&uri);
    true
}

fn button(ui: &mut UiState, src: &Sources, label: &str) -> Vec<Out> {
    ui.close_popups();
    let idx = plan_index(ui, src);
    let label = label.trim_end_matches('*').trim_end();
    if page_button(ui, label) {
        return Vec::new();
    }
    if let Some(u) = nav_button(ui, label) {
        ui.navigate(&u);
        return Vec::new();
    }
    if label.contains("CPNY F-PLN") {
        return if label.starts_with("RECEIVED") {
            vec![Out::CpnyFplnInsert(idx)]
        } else if label.contains("PENDING") {
            Vec::new()
        } else {
            vec![Out::CpnyFplnRequest(idx)]
        };
    }
    match label {
        "RETURN" => ui.navigate("back"),
        "CLOSE" => ui.message_list_open = false,
        "MSG LIST" => ui.message_list_open = true,
        "CLEAR INFO" => {
            if let Some(m) = src.messages.iter().find(|m| !ui.cleared_messages.contains(m)) {
                ui.cleared_messages.push(m.clone());
            }
        }
        "CANCEL" => {
            ui.new_dest_open = false;
            ui.insert_next_open = false;
            reset_revised(ui);
        }
        "CANCEL NOISE" if ui.page() == Page::Perf => return vec![perf(idx, Entry::NoiseEnabled(false))],
        "DELETE SPD CSTR" | "DELETE ALT CSTR" if ui.page() == Page::VertRev => {
            let Some(leg) = model::vert_rev_selected(ui, src) else { return Vec::new() };
            let r = PlanRef::from_index(idx);
            let descent = src.phase >= model::PHASE_CRUISE;
            return vec![if label == "DELETE SPD CSTR" {
                plan_cmd(Plan::SetPilotSpeedConstraint { index: leg, descent, speed: None, plan: r })
            } else {
                plan_cmd(Plan::SetPilotAltitudeConstraint { index: leg, descent, constraint: None, plan: r })
            }];
        }
        "ERASE TMPY" | "ERASE" | "ERASE DIR TO" => {
            ui.dir_to_selected = None;
            if label == "ERASE DIR TO" {
                ui.navigate(&fms_uri(&category(ui), "f-pln"));
            }
            return vec![plan_cmd(Plan::TemporaryDelete)];
        }
        "INSERT TMPY" | "INSERT" | "INSERT DIR TO" | "TMPY F-PLN" => {
            if label == "TMPY F-PLN" {
                ui.navigate(&fms_uri(&category(ui), "f-pln"));
                return Vec::new();
            }
            if ui.page() == Page::DirTo {
                ui.navigate(&fms_uri(&category(ui), "f-pln"));
            }
            if label == "INSERT DIR TO" {
                return vec![Out::KeyEvent("A32NX.FMGC_DIR_TO_TRIGGER".into()), plan_cmd(Plan::TemporaryInsert)];
            }
            return vec![plan_cmd(Plan::TemporaryInsert)];
        }
        _ if ui.page() == Page::Fpln
            && src.plan(idx).and_then(|p| p.destination_airport.as_ref()).is_some_and(|a| !a.ident.is_empty() && label.starts_with(a.ident.as_str())) =>
        {
            reset_revised(ui);
            ui.navigate(&fms_uri(&category(ui), "f-pln-arrival"));
        }
        "DEPARTURE" if ui.page() == Page::Init => {
            let m = model::init(ui, src);
            if m.from.is_some() && m.to.is_some() && m.flight_phase == 0 {
                ui.navigate(&fms_uri(&category(ui), "f-pln-departure"));
            }
        }
        "DEST" => {
            let plan = src.plan(idx);
            let dest = plan.and_then(|p| p.destination_leg_index());
            let lines = model::fpln_line_data(plan);
            let count = if src.tmpy() && ui.uri.sec().is_none() { 8 } else { 9 };
            let line = dest.and_then(|d| lines.iter().position(|l| !l.is_altn && l.original_leg_index == Some(d)));
            ui.fpln_scroll = Some(line.map_or(0, |k| (k + 1).saturating_sub(count)));
        }
        "ACTIVATE SEC" | "SWAP ACTIVE" => {
            if let Some(n) = ui.uri.sec().or(Some(ui.tab(Page::SecIndex, 0) + 1)) {
                return vec![plan_cmd(Plan::ActiveAndSecondarySwap { index: n })];
            }
        }
        _ => {}
    }
    Vec::new()
}

/// FBW's MFD keys: typed characters go to the scratchpad of the focused field,
/// ENTER validates, ESC (and a click elsewhere) releases the keyboard.
pub fn key(ui: &mut UiState, src: &Sources, db: Option<&dyn NavDatabase>, k: Key) -> (bool, Vec<Out>) {
    let Some(field) = ui.focus.clone() else { return (false, Vec::new()) };
    match k {
        Key::Escape => {
            ui.release_keyboard();
            ui.close_popups();
            (true, Vec::new())
        }
        Key::Backspace => {
            ui.scratchpad.pop();
            (true, Vec::new())
        }
        Key::Char(c) => {
            let c = c.to_ascii_uppercase();
            if (c.is_ascii_alphanumeric() || "./+-/ ".contains(c)) && ui.scratchpad.len() < SCRATCHPAD_MAX {
                ui.scratchpad.push(c);
            }
            (true, Vec::new())
        }
        Key::Enter | Key::Delete => {
            let text = if k == Key::Delete { "CLR".to_string() } else { ui.scratchpad.trim().to_string() };
            ui.release_keyboard();
            ui.close_popups();
            if text.is_empty() {
                return (true, Vec::new());
            }
            (true, commit_field(ui, src, db, &field, &text))
        }
    }
}

fn commit_field(ui: &mut UiState, src: &Sources, db: Option<&dyn NavDatabase>, field: &str, text: &str) -> Vec<Out> {
    let f = field.split_once('|').map(|(_, a)| a).unwrap_or(field);
    let label = strip_dup(f.strip_prefix("field:").unwrap_or(f));
    let nth = f.split_once('#').and_then(|(_, n)| n.parse::<usize>().ok()).unwrap_or(1);
    match commit(ui, src, db, label, nth, text) {
        Ok(out) => out,
        Err(m) => {
            ui.entry_message = Some(m.to_string());
            Vec::new()
        }
    }
}

/// FBW `InputField` blur: leaving a field validates what was typed.
pub fn blur(ui: &mut UiState, src: &Sources) -> Vec<Out> {
    let Some(field) = ui.focus.clone() else { return Vec::new() };
    let text = ui.scratchpad.trim().to_string();
    ui.release_keyboard();
    if text.is_empty() {
        return Vec::new();
    }
    commit_field(ui, src, None, &field, &text)
}

const MMO: f64 = 0.89;
const VMO: f64 = 340.;
const MAX_CERTIFIED_ALT: f64 = 43100.;

fn parse_num(text: &str) -> Result<f64, &'static str> {
    text.trim().parse::<f64>().ok().filter(|v| v.is_finite()).ok_or(FORMAT_ERROR)
}

fn in_range(v: f64, lo: f64, hi: f64) -> Result<f64, &'static str> {
    if v < lo || v > hi {
        Err(OUT_OF_RANGE)
    } else {
        Ok(v)
    }
}

fn num(text: &str) -> Result<Option<f64>, &'static str> {
    if text == "CLR" {
        return Ok(None);
    }
    parse_num(text).map(Some)
}

fn num_in(text: &str, lo: f64, hi: f64) -> Result<Option<f64>, &'static str> {
    match num(text)? {
        Some(v) => in_range(v, lo, hi).map(Some),
        None => Ok(None),
    }
}

/// FBW `SpeedKnotsFormat(90, Vmo)`.
fn knots(text: &str) -> Result<Option<f64>, &'static str> {
    num_in(text, 90., VMO)
}

/// FBW `SpeedMachFormat(0.1, Mmo)`: `82` is .82.
fn mach(text: &str) -> Result<Option<f64>, &'static str> {
    if text == "CLR" {
        return Ok(None);
    }
    let mut v = parse_num(text)?;
    if v > MMO {
        v = parse_num(&format!("0.{text}"))?;
    }
    in_range(v, 0.1, MMO).map(Some)
}

/// FBW `FlightLevelFormat`: `FL350` or `350`.
fn flight_level(text: &str) -> Result<Option<f64>, &'static str> {
    if text == "CLR" {
        return Ok(None);
    }
    let t = text.strip_prefix("FL").filter(|r| !r.is_empty() && r.len() <= 3 && r.chars().all(|c| c.is_ascii_digit())).unwrap_or(text);
    in_range(parse_num(t)?, 0., MAX_CERTIFIED_ALT / 100.).map(Some)
}

/// FBW `AltitudeOrFlightLevelFormat`: up to three digits are hundreds of feet.
fn alt_or_fl(text: &str) -> Result<Option<f64>, &'static str> {
    if text == "CLR" {
        return Ok(None);
    }
    let t = text.strip_prefix("FL").unwrap_or(text);
    let v = parse_num(t)?;
    in_range(if t.len() <= 3 { v * 100. } else { v }, 0., MAX_CERTIFIED_ALT).map(Some)
}

/// FBW `AltitudeFormat(min, maxCertifiedAlt)`: feet as typed.
fn altitude(text: &str, min: f64) -> Result<Option<f64>, &'static str> {
    num_in(text, min, MAX_CERTIFIED_ALT)
}

/// FBW `TropoFormat`.
fn tropo(text: &str) -> Result<Option<f64>, &'static str> {
    if text == "CLR" {
        return Ok(None);
    }
    let v = parse_num(text)?;
    in_range(if text.len() <= 3 { v * 100. } else { v }, 1000., 60000.).map(Some)
}

/// FBW `CrzTempFormat`: a positive number without `+` is below zero.
fn crz_temp(text: &str) -> Result<Option<f64>, &'static str> {
    if text == "CLR" {
        return Ok(None);
    }
    let v = parse_num(text)?;
    in_range(if v > 0. && !text.starts_with('+') { -v } else { v }, -99., 99.).map(Some)
}

/// FBW `TripWindFormat`: `HD`/`-`/`H` is a headwind, `TL`/`+`/`T` a tailwind.
fn trip_wind(text: &str) -> Result<Option<f64>, &'static str> {
    if text == "CLR" {
        return Ok(None);
    }
    let (sign, rest) = if let Some(r) = text.strip_prefix("HD") {
        (-1., r)
    } else if let Some(r) = text.strip_prefix('-').or_else(|| text.strip_prefix('H')) {
        (-1., r)
    } else if let Some(r) = text.strip_prefix("TL") {
        (1., r)
    } else if let Some(r) = text.strip_prefix('+').or_else(|| text.strip_prefix('T')) {
        (1., r)
    } else {
        (1., text)
    };
    in_range(sign * parse_num(rest)?, -250., 250.).map(Some)
}

/// FBW `DescentRateFormat(-999, -100)`: a positive number is a descent.
fn descent_rate(text: &str) -> Result<Option<f64>, &'static str> {
    if text == "CLR" {
        return Ok(None);
    }
    let v = parse_num(text)?;
    in_range(if v > 0. { -v } else { v }, -999., -100.).map(Some)
}

/// FBW `QnhFormat`: `1013`, or `2992` / `29.92` inches.
fn qnh(text: &str) -> Result<Option<f64>, &'static str> {
    if text == "CLR" {
        return Ok(None);
    }
    let parsed = parse_num(text)?;
    let digits = |s: &str| s.chars().all(|c| c.is_ascii_digit());
    let inches_shape = (text.len() == 4 && digits(text)) || (text.len() == 5 && text.as_bytes()[2] == b'.' && digits(&text[..2]) && digits(&text[3..]));
    if inches_shape {
        let decimal = text.contains('.');
        let inches = if decimal { parsed } else { parsed / 100. };
        if (22.0..=32.48).contains(&inches) {
            return Ok(Some(inches));
        }
        if decimal {
            return Err(OUT_OF_RANGE);
        }
    }
    if !text.is_empty() && text.len() <= 4 && digits(text) {
        in_range(parsed, 745., 1100.).map(Some)
    } else {
        Err(FORMAT_ERROR)
    }
}

fn weight(text: &str, metric: bool) -> Result<Option<f64>, &'static str> {
    Ok(num(text)?.map(|v| model::weight_to_kg(v, metric)))
}

fn icao(text: &str) -> Result<String, &'static str> {
    if text.len() == 4 && text.chars().all(|c| c.is_ascii_alphanumeric()) {
        Ok(text.to_string())
    } else {
        Err(FORMAT_ERROR)
    }
}

fn fix(db: Option<&dyn NavDatabase>, ident: &str) -> Result<Fix, &'static str> {
    db.and_then(|d| d.search_fix(ident).into_iter().next()).ok_or(NOT_IN_DATABASE)
}

/// What validating a field does: the FMS command for its label (or `data-key`) on this page.
fn commit(ui: &mut UiState, src: &Sources, db: Option<&dyn NavDatabase>, label: &str, nth: usize, text: &str) -> Result<Vec<Out>, &'static str> {
    let idx = plan_index(ui, src);
    let r = PlanRef::from_index(idx);
    let p = |e: Entry| Ok(vec![perf(idx, e)]);
    let metric = ui.metric;
    let page = ui.page();
    match (page, label) {
        (Page::Init, "FLT NBR") => Ok(vec![Out::Fms(HostCommand::SetFlightNumber { plan: idx, number: text.to_string() })]),
        (Page::Init, "FROM") | (Page::Init, "TO") => {
            let (from, to) = match text.split_once('/') {
                Some((f, t)) => (Some(icao(f)?), Some(icao(t)?)),
                None if label == "FROM" => (Some(icao(text)?), ui.pending_to.clone().or_else(|| src.plan(idx).and_then(|p| p.destination_airport.as_ref()).map(|a| a.ident.clone()))),
                None => (ui.pending_from.clone().or_else(|| src.plan(idx).and_then(|p| p.origin_airport.as_ref()).map(|a| a.ident.clone())), Some(icao(text)?)),
            };
            ui.pending_from = from.clone();
            ui.pending_to = to.clone();
            match (from, to) {
                (Some(from), Some(to)) => {
                    ui.pending_from = None;
                    ui.pending_to = None;
                    Ok(vec![plan_cmd(Plan::NewCityPair { from, to, alternate: None, plan: r })])
                }
                _ => Ok(Vec::new()),
            }
        }
        (Page::Init, "ALTN") => {
            let a = if text == "CLR" || text == "NONE" { None } else { Some(icao(text)?) };
            Ok(vec![plan_cmd(Plan::SetAlternate { icao: a, plan: r })])
        }
        (Page::Init | Page::Perf, "CRZ FL" | "CRZ") => p(Entry::CruiseFlightLevel(flight_level(text)?.ok_or(NOT_ALLOWED)?)),
        (Page::Init, "CRZ TEMP") => p(Entry::CruiseTemperature(crz_temp(text)?)),
        (Page::Init | Page::FuelLoad | Page::Perf, "CI") => p(Entry::CostIndex(num_in(text, 0., 999.)?)),
        (Page::Init, "TROPO") => p(Entry::Tropopause(tropo(text)?)),
        (Page::Init, "TRIP WIND") => p(Entry::TripWind(trip_wind(text)?)),
        (Page::FuelLoad, "ZFW") => p(Entry::ZeroFuelWeightKg(weight(text, metric)?)),
        (Page::FuelLoad, "ZFWCG" | "ZFW CG" | "CG") => p(Entry::ZeroFuelWeightCg(num(text)?)),
        (Page::FuelLoad, "BLOCK") => p(Entry::BlockFuelKg(weight(text, metric)?)),
        (Page::FuelLoad, "TAXI") => p(Entry::TaxiFuelKg(weight(text, metric)?)),
        (Page::FuelLoad, "RTE RSV") if nth == 1 => p(Entry::RouteReserveFuelKg(weight(text, metric)?)),
        (Page::FuelLoad, "RTE RSV") => p(Entry::RouteReservePercent(num(text)?)),
        (Page::FuelLoad, "ALTN") => p(Entry::AlternateFuelKg(weight(text, metric)?)),
        (Page::FuelLoad, "FINAL") if nth == 1 => p(Entry::FinalHoldingFuelKg(weight(text, metric)?)),
        (Page::FuelLoad, "FINAL") => p(Entry::FinalHoldingTimeMin(time_min(text)?)),
        (Page::FuelLoad, "MIN FUEL AT DEST") => p(Entry::MinDestFobKg(weight(text, metric)?)),
        (Page::FuelLoad, "PAX" | "PAX NBR") => p(Entry::PaxNumber(num_in(text, 0., 999.)?)),
        (Page::Perf, "V1") => p(Entry::V1(knots(text)?)),
        (Page::Perf, "VR") => p(Entry::Vr(knots(text)?)),
        (Page::Perf, "V2") => p(Entry::V2(knots(text)?)),
        (Page::Perf, "FLEX" | "FLEX TO TEMP") => p(Entry::FlexTemperature(num_in(text, 0., 99.)?)),
        (Page::Perf, "THS FOR") => p(Entry::ThsFor(num_in(text.trim_start_matches("UP").trim_start_matches("DN"), 0., 99.9)?)),
        (Page::Perf, "T.O SHIFT") => p(Entry::TakeoffShift(num(text)?)),
        (Page::Perf, "THR RED") if perf_tab(ui, src) == 5 => p(Entry::MissedThrustReductionAltitude(alt_or_fl(text)?)),
        (Page::Perf, "ACCEL") if perf_tab(ui, src) == 5 => p(Entry::MissedAccelerationAltitude(alt_or_fl(text)?)),
        (Page::Perf, "EO ACCEL") if perf_tab(ui, src) == 5 => p(Entry::MissedEngineOutAccelerationAltitude(alt_or_fl(text)?)),
        (Page::Perf, "THR RED") => p(Entry::ThrustReductionAltitude(alt_or_fl(text)?)),
        (Page::Perf, "ACCEL") => p(Entry::AccelerationAltitude(alt_or_fl(text)?)),
        (Page::Perf, "EO ACCEL") => p(Entry::EngineOutAccelerationAltitude(alt_or_fl(text)?)),
        (Page::Perf, "TRANS") if perf_tab(ui, src) >= 3 => p(Entry::TransitionLevel(flight_level(text)?)),
        (Page::Perf, "TRANS") => p(Entry::TransitionAltitude(altitude(text, 1.)?)),
        (Page::Perf, "N1") => p(Entry::NoiseN1(num_in(text, 40., 110.)?)),
        (Page::Perf, "SPD") => p(Entry::NoiseSpeed(knots(text)?)),
        (Page::Perf, "END" | "NOISE END") => p(Entry::NoiseEndAltitude(alt_or_fl(text)?)),
        (Page::Perf, "PRESEL") => p(Entry::PreselectedClimbSpeed(knots(text)?)),
        (Page::Perf, "PRESEL MACH") => p(Entry::PreselectedCruiseSpeed(mach(text)?)),
        (Page::Perf, "PRESEL SPD") => p(Entry::PreselectedCruiseSpeed(knots(text)?)),
        (Page::Perf, "DES MACH") => p(Entry::ManagedDescentMach(mach(text)?)),
        (Page::Perf, "DES SPD") => p(Entry::ManagedDescentSpeed(knots(text)?)),
        (Page::Perf, "DES CABIN RATE" | "CABIN RATE") => p(Entry::DescentCabinRate(descent_rate(text)?)),
        (Page::Perf, "QNH") => p(Entry::ApproachQnh(qnh(text)?)),
        (Page::Perf, "OAT" | "TEMP") => p(Entry::ApproachTemperature(num_in(text, -99., 99.)?)),
        (Page::Perf, "WIND DIR") => p(Entry::ApproachWindDirection(num_in(text, 0., 359.)?)),
        (Page::Perf, "WIND SPD") => p(Entry::ApproachWindMagnitude(num_in(text, 0., 250.)?)),
        (Page::Perf, "MAG WIND") => {
            let (d, s) = text.split_once('/').ok_or(FORMAT_ERROR)?;
            let mut out = Vec::new();
            if !d.is_empty() {
                out.push(perf(idx, Entry::ApproachWindDirection(num_in(d, 0., 359.)?)));
            }
            if !s.is_empty() {
                out.push(perf(idx, Entry::ApproachWindMagnitude(num_in(s, 0., 250.)?)));
            }
            Ok(out)
        }
        (Page::Perf, "BARO") => p(Entry::ApproachBaroMinimum(altitude(text, 0.)?)),
        (Page::Perf, "RADIO") => p(Entry::ApproachRadioMinimum(if matches!(text, "NO DH" | "NODH" | "NONE" | "NO") { Some(0.) } else { altitude(text, 1.)? })),
        (Page::Perf, "VAPP") => p(Entry::Vapp(knots(text)?)),
        (Page::VertRev, "SPD CSTR" | "SPD") => {
            let leg = model::vert_rev_selected(ui, src).ok_or(NOT_ALLOWED)?;
            Ok(vec![plan_cmd(Plan::SetPilotSpeedConstraint { index: leg, descent: src.phase >= model::PHASE_CRUISE, speed: knots(text)?, plan: r })])
        }
        (Page::VertRev, "ALT CSTR" | "ALT") => {
            let leg = model::vert_rev_selected(ui, src).ok_or(NOT_ALLOWED)?;
            let descriptor = src.plan(idx).and_then(|p| p.leg_at(leg)).and_then(|l| l.altitude_constraint()).map(|c| if matches!(c.descriptor.as_str(), "+" | "-") { c.descriptor } else { "@".to_string() }).unwrap_or_else(|| "@".into());
            let c = alt_or_fl(text)?.map(|a| AltitudeConstraint { descriptor, altitude1: a, altitude2: None });
            Ok(vec![plan_cmd(Plan::SetPilotAltitudeConstraint { index: leg, descent: src.phase >= model::PHASE_CRUISE, constraint: c, plan: r })])
        }
        (Page::Fpln, _) if ui.insert_next_open || ui.new_dest_open => {
            let leg = ui.revised_leg.ok_or(NOT_ALLOWED)?;
            let out = if ui.new_dest_open {
                plan_cmd(Plan::NewDest { index: leg, airport: icao(text)?, plan: r, alternate: model::proc_target_is_alternate(ui, src, idx) })
            } else {
                plan_cmd(Plan::NextWaypoint { index: leg, waypoint: fix(db, text)?, plan: r })
            };
            ui.new_dest_open = false;
            ui.insert_next_open = false;
            reset_revised(ui);
            Ok(vec![out])
        }
        (Page::DirTo, _) => {
            let f = fix(db, text)?;
            let mut out = Vec::new();
            if src.tmpy() {
                out.push(plan_cmd(Plan::TemporaryDelete));
            }
            ui.dir_to_selected = None;
            ui.dir_to_text = f.ident.clone();
            out.push(plan_cmd(Plan::DirectToWaypoint { ppos: ppos(src), true_track: src.var("GPS GROUND TRUE TRACK"), waypoint: f, mag_var: mag_var(src), with_abeam: false, plan: PlanRef::Active }));
            Ok(out)
        }
        _ => Err(NOT_ALLOWED),
    }
}

fn perf_tab(ui: &UiState, src: &Sources) -> usize {
    ui.tab(Page::Perf, model::perf_tab_default(src, plan_index(ui, src)))
}

fn time_min(text: &str) -> Result<Option<f64>, &'static str> {
    if text == "CLR" {
        return Ok(None);
    }
    if text.len() == 4 && text.chars().all(|c| c.is_ascii_digit()) {
        let h: f64 = text[..2].parse().map_err(|_| FORMAT_ERROR)?;
        let m: f64 = text[2..].parse().map_err(|_| FORMAT_ERROR)?;
        return Ok(Some(h * 60. + m));
    }
    num(text)
}

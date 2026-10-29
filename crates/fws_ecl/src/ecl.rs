use crate::deferred::{
    self, SPECIAL_INDEX_ACTIVATE, SPECIAL_INDEX_DEFERRED_PROC_COMPLETE, SPECIAL_INDEX_DEFERRED_PROC_RECALL,
};
use crate::normal_data::{DEFERRED_PROCEDURE_IDS, NORMAL_PROCEDURES, WD_NUM_LINES};
use crate::types::{LineStyle, NormalProcedure};

pub const SPECIAL_INDEX_NORMAL_CL_COMPLETE: i32 = -5;
pub const SPECIAL_INDEX_NORMAL_RESET: i32 = -6;
pub const SPECIAL_INDEX_DEFERRED_PAGE_CLEAR: i32 = -99;
const HIGHEST_SPECIAL_INDEX: i32 = -2;
const DEPARTURE_CHANGE_ID: u32 = 1000006;

const NON_SELECTABLE_STYLES: [LineStyle; 6] = [
    LineStyle::Headline,
    LineStyle::OmissionDots,
    LineStyle::SeparationLine,
    LineStyle::SubHeadline,
    LineStyle::CenteredSubHeadline,
    LineStyle::Red,
];

/// FwsCore values the sensed items read. Raw sources are listed in docs/ecl.md.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SensedInputs {
    pub seat_belt_switch: bool,
    pub beacon_on: bool,
    pub rudder_trim_deg: f64,
    pub spoilers_armed: bool,
    pub flaps_handle: i32,
    pub auto_brake_mode: i32,
    pub to_config_normal: bool,
    pub all_gear_downlocked: bool,
    pub landing_conf3: bool,
    pub engine_master: [bool; 4],
    pub all_fuel_pumps_off: bool,
    pub crew_oxygen_pb: bool,
    pub emer_exit_light_position: i32,
}

/// Whether RUDDER TRIM compares |trim| (this port's ecl SourcePatch) or the signed value (FBW as shipped).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RudderTrimSensing {
    Absolute,
    SignedAsFbw,
}

pub fn sensed_items(id: u32, i: &SensedInputs, trim: RudderTrimSensing) -> Vec<Option<bool>> {
    let trim_neutral = match trim {
        RudderTrimSensing::Absolute => i.rudder_trim_deg.abs() < 0.35,
        RudderTrimSensing::SignedAsFbw => i.rudder_trim_deg < 0.35,
    };
    let flaps_to = matches!(i.flaps_handle, 1..=3);
    let flaps_ldg = (!i.landing_conf3 && i.flaps_handle == 4) || (i.landing_conf3 && i.flaps_handle == 3);
    match id {
        1000001 => vec![None, None, Some(i.seat_belt_switch), None],
        1000002 => vec![None, None, Some(i.beacon_on)],
        1000003 => vec![None, None, Some(trim_neutral)],
        1000004 => vec![
            None,
            None,
            None,
            Some(false),
            Some(i.seat_belt_switch),
            Some(i.spoilers_armed),
            Some(flaps_to),
            Some(i.auto_brake_mode == 6),
            Some(i.to_config_normal),
        ],
        1000005 => vec![None, None],
        1000006 => vec![None, None, None, None],
        1000010 => vec![None, Some(i.seat_belt_switch), None, None],
        1000012 => vec![
            Some(false),
            Some(i.seat_belt_switch),
            Some(i.all_gear_downlocked),
            Some(i.spoilers_armed),
            Some(flaps_ldg),
        ],
        1000013 => vec![None, Some(i.engine_master.iter().all(|m| !m)), None, Some(i.all_fuel_pumps_off)],
        1000014 => vec![Some(i.crew_oxygen_pb), Some(i.emer_exit_light_position == 2), None, None],
        _ => vec![None],
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChecklistState {
    pub id: u32,
    pub procedure_completed: bool,
    pub procedure_activated: bool,
    pub items_checked: Vec<bool>,
    pub items_active: Vec<bool>,
    pub items_to_show: Vec<bool>,
}

/// ALL PHASES, AT TOP OF DESCENT, FOR APPROACH, FOR LANDING.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeferredStatus {
    pub has: [bool; 4],
    pub completed: [bool; 4],
}

impl DeferredStatus {
    pub fn none() -> Self {
        Self { has: [false; 4], completed: [true; 4] }
    }
}

/// Rising edges of the ECP keys (FwsCore pulse nodes) for one FWS cycle.
#[derive(Clone, Copy, Debug, Default)]
pub struct EcpPulses {
    pub cl: bool,
    pub check: bool,
    pub up: bool,
    pub down: bool,
    pub clr: bool,
}

/// An entry of FwsCore's `activeDeferredProceduresList`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeferredEntry {
    pub state: ChecklistState,
    /// Last `whichItemsActive()` of the procedure, None when FBW defines none.
    pub items_active_source: Option<Vec<bool>>,
}

#[derive(Clone, Debug)]
struct DeferredGen {
    state: ChecklistState,
    sii: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct CycleInputs {
    pub sensed: SensedInputs,
    pub pulses: EcpPulses,
    pub flight_phase: u8,
    pub takeoff_power: bool,
    /// presented abnormal list empty, phase 8, pressure altitude < 20000 ft, and a QNH-set or slats-extended pulse.
    pub approach_auto_display: bool,
    /// abnormal sensed page shown: CLR goes to it instead of the ECL.
    pub abnormal_sensed_shown: bool,
    /// ABN PROC page requested: it takes the EWD over the ECL.
    pub abn_proc_requested: bool,
}

impl Default for CycleInputs {
    fn default() -> Self {
        Self {
            sensed: SensedInputs::default(),
            pulses: EcpPulses::default(),
            flight_phase: 1,
            takeoff_power: false,
            approach_auto_display: false,
            abnormal_sensed_shown: false,
            abn_proc_requested: false,
        }
    }
}

pub struct Ecl {
    pub states: Vec<ChecklistState>,
    pub overview: ChecklistState,
    pub checklist_id: u32,
    pub selected_line: i32,
    pub show_from_line: i32,
    pub shown: bool,
    pub show_requested: bool,
    pub manual_reset: bool,
    pub rudder_trim: RudderTrimSensing,
    /// Copy the open checklist's procedure works on; FBW only refreshes it on open and on CHECK.
    gen_state: Option<ChecklistState>,
    pub deferred: DeferredStatus,
    prev_takeoff_power: bool,
    /// FwsCore `activeDeferredProceduresList`, insertion order.
    pub deferred_procs: Vec<DeferredEntry>,
    pub active_deferred_id: Option<u32>,
    visible_deferred: Vec<u32>,
    deferred_gens: Vec<DeferredGen>,
    active_gen: Option<usize>,
}

fn proc_at(index: usize) -> &'static NormalProcedure {
    &NORMAL_PROCEDURES[index]
}

fn index_of(id: u32) -> Option<usize> {
    NORMAL_PROCEDURES.iter().position(|p| p.id == id)
}

fn initial_state(p: &NormalProcedure) -> ChecklistState {
    let n = p.items.len();
    ChecklistState {
        id: p.id,
        procedure_completed: p.only_activated_by_request,
        procedure_activated: true,
        items_checked: vec![false; n],
        items_active: vec![true; n],
        items_to_show: vec![true; n],
    }
}

impl Default for Ecl {
    fn default() -> Self {
        Self::new()
    }
}

impl Ecl {
    pub fn new() -> Self {
        let n = NORMAL_PROCEDURES.len();
        Self {
            states: NORMAL_PROCEDURES.iter().map(initial_state).collect(),
            overview: ChecklistState {
                id: 0,
                procedure_completed: false,
                procedure_activated: true,
                items_checked: vec![false; n],
                items_active: vec![true; n],
                items_to_show: vec![true; n],
            },
            checklist_id: 0,
            selected_line: 0,
            show_from_line: 0,
            shown: false,
            show_requested: false,
            manual_reset: false,
            rudder_trim: RudderTrimSensing::Absolute,
            gen_state: None,
            deferred: DeferredStatus::none(),
            prev_takeoff_power: false,
            deferred_procs: Vec::new(),
            active_deferred_id: None,
            visible_deferred: Vec::new(),
            deferred_gens: Vec::new(),
            active_gen: None,
        }
    }

    pub fn state(&self, id: u32) -> Option<&ChecklistState> {
        if id == 0 {
            return Some(&self.overview);
        }
        self.states.iter().find(|s| s.id == id)
    }

    fn state_mut(&mut self, id: u32) -> Option<&mut ChecklistState> {
        self.states.iter_mut().find(|s| s.id == id)
    }

    fn set_selected(&mut self, line: i32) {
        self.selected_line = line;
        self.show_from_line = (line - WD_NUM_LINES as i32 + 2).max(0);
    }

    fn open_procedure(&self) -> Option<&'static NormalProcedure> {
        if self.checklist_id == 0 || DEFERRED_PROCEDURE_IDS.contains(&self.checklist_id) {
            return None;
        }
        crate::normal_procedure(self.checklist_id)
    }

    /// Power-up (FWS startup completed): FBW resets every checklist.
    pub fn on_startup_completed(&mut self) {
        self.reset(Some(None));
        self.core_clear_deferred();
    }

    /// FWS shut down for 50 minutes: reset unless the crew reset manually in this phase.
    pub fn on_shutdown_50_minutes(&mut self) {
        if !self.manual_reset {
            self.reset(Some(None));
        }
        self.core_clear_deferred();
    }

    /// `from`: None = not found (no-op), Some(None) = everything, Some(Some(i)) = after index i.
    fn reset(&mut self, from: Option<Option<usize>>) {
        let Some(from) = from else { return };
        self.manual_reset = from.is_some();
        let start = from.map_or(0, |i| i + 1);
        for idx in start..NORMAL_PROCEDURES.len() {
            let p = proc_at(idx);
            let s = &mut self.states[idx];
            s.procedure_completed = p.only_activated_by_request;
            s.procedure_activated = true;
            for (k, c) in s.items_checked.iter_mut().enumerate() {
                if !p.items[k].sensed {
                    *c = false;
                }
            }
        }
    }

    fn navigate_to(&mut self, id: u32) {
        if id != self.checklist_id {
            self.checklist_id = id;
            self.build_page();
        }
        self.select_first();
    }

    fn build_page(&mut self) {
        let id = self.checklist_id;
        if self.open_procedure().is_some() {
            self.gen_state = self.state(id).cloned();
            self.active_gen = None;
            self.set_selected(0);
        } else if let Some(slot) = DEFERRED_PROCEDURE_IDS.iter().position(|d| *d == id) {
            self.deferred_gens.clear();
            self.visible_deferred = self
                .deferred_procs
                .iter()
                .map(|e| e.state.id)
                .filter(|k| deferred::procedure(*k).is_some_and(|p| deferred::slot(p.kind) == slot))
                .collect();
            let Some(&first) = self.visible_deferred.first() else {
                if self.active_deferred_id.take().is_some() {
                    self.set_selected(SPECIAL_INDEX_DEFERRED_PAGE_CLEAR);
                }
                self.active_gen = None;
                return;
            };
            self.active_deferred_id = Some(first);
            for k in self.visible_deferred.clone() {
                if let Some(e) = self.deferred_procs.iter().find(|e| e.state.id == k) {
                    self.deferred_gens.push(DeferredGen { state: e.state.clone(), sii: 0 });
                }
            }
            self.active_gen = Some(0);
            self.set_selected(self.deferred_gens[0].sii);
        }
    }

    fn set_active_deferred(&mut self, id: Option<u32>) {
        if id == self.active_deferred_id {
            return;
        }
        self.active_deferred_id = id;
        match id {
            Some(id) => {
                if let Some(k) = self.deferred_gens.iter().position(|g| g.state.id == id) {
                    self.active_gen = Some(k);
                    self.set_selected(self.deferred_gens[k].sii);
                    self.dgen_select_first(k);
                }
            }
            None => self.set_selected(SPECIAL_INDEX_DEFERRED_PAGE_CLEAR),
        }
    }

    fn dgen_set(&mut self, k: usize, v: i32) {
        if self.deferred_gens[k].sii != v {
            self.deferred_gens[k].sii = v;
            self.set_selected(v);
        }
    }

    fn dgen_selectable(&self, k: usize, skip_completed_sensed: bool) -> Vec<i32> {
        let s = &self.deferred_gens[k].state;
        let Some(p) = deferred::procedure(s.id) else { return Vec::new() };
        p.items
            .iter()
            .enumerate()
            .filter(|(i, item)| {
                s.items_active[*i]
                    && s.items_to_show[*i]
                    && deferred::selectable(item)
                    && (!item.sensed || !skip_completed_sensed || !s.items_checked[*i])
            })
            .map(|(i, _)| i as i32)
            .collect()
    }

    fn dgen_select_first(&mut self, k: usize) {
        let s = &self.deferred_gens[k].state;
        if !s.procedure_activated {
            self.dgen_set(k, SPECIAL_INDEX_ACTIVATE);
            return;
        }
        if s.procedure_completed {
            self.dgen_set(k, SPECIAL_INDEX_DEFERRED_PROC_RECALL);
            return;
        }
        let n = s.items_checked.len() as i32;
        let sel = self.dgen_selectable(k, true);
        self.dgen_set(k, sel.first().map_or(n - 1, |f| f - 1));
        self.dgen_move_down(k, true);
    }

    fn dgen_first_line_selected(&self, k: usize) -> bool {
        let g = &self.deferred_gens[k];
        !g.state.procedure_activated && g.sii == SPECIAL_INDEX_ACTIVATE
    }

    fn dgen_last_line_selected(&self, k: usize) -> bool {
        let g = &self.deferred_gens[k];
        if g.state.procedure_activated {
            g.sii == SPECIAL_INDEX_DEFERRED_PROC_COMPLETE || g.sii == SPECIAL_INDEX_DEFERRED_PROC_RECALL
        } else {
            g.sii == SPECIAL_INDEX_ACTIVATE
        }
    }

    fn dgen_move_up(&mut self, k: usize) {
        if !self.deferred_gens[k].state.procedure_activated {
            self.dgen_set(k, SPECIAL_INDEX_ACTIVATE);
            return;
        }
        let sel = self.dgen_selectable(k, true);
        if sel.is_empty() {
            self.dgen_select_first(k);
            return;
        }
        let sii = self.deferred_gens[k].sii;
        if sii == SPECIAL_INDEX_DEFERRED_PROC_COMPLETE {
            self.dgen_set(k, *sel.last().unwrap());
        } else if sii >= 0 {
            if let Some(prev) = sel.iter().rev().copied().find(|v| *v < sii) {
                self.dgen_set(k, prev.max(0));
            }
        }
    }

    fn dgen_move_down(&mut self, k: usize, skip_completed_sensed: bool) {
        let g = &self.deferred_gens[k];
        if !g.state.procedure_activated {
            self.dgen_set(k, SPECIAL_INDEX_ACTIVATE);
            return;
        }
        let (completed, sii) = (g.state.procedure_completed, g.sii);
        let sel = self.dgen_selectable(k, skip_completed_sensed);
        if sel.is_empty() || sii >= *sel.last().unwrap() {
            let v = if completed { SPECIAL_INDEX_DEFERRED_PROC_RECALL } else { SPECIAL_INDEX_DEFERRED_PROC_COMPLETE };
            self.dgen_set(k, v);
        } else if sii == SPECIAL_INDEX_ACTIVATE {
            self.dgen_set(k, sel[0]);
        } else if sii > HIGHEST_SPECIAL_INDEX {
            let last = *sel.last().unwrap();
            let next = sel.iter().copied().find(|v| *v > sii).unwrap_or(last);
            self.dgen_set(k, next.min(last));
        }
    }

    fn store_deferred(&mut self, st: ChecklistState) {
        match self.deferred_procs.iter_mut().find(|e| e.state.id == st.id) {
            Some(e) => e.state = st,
            None => self.deferred_procs.push(DeferredEntry { state: st, items_active_source: None }),
        }
        self.after_deferred_list_change();
    }

    /// `ProcedureLinesGenerator.checkSelected` for a deferred procedure; moves use the page's previous copy, as in FBW.
    fn dgen_check_selected(&mut self, k: usize) {
        let mut cl = self.deferred_gens[k].state.clone();
        let Some(p) = deferred::procedure(cl.id) else { return };
        let sii = self.deferred_gens[k].sii;
        let last_item = if cl.procedure_completed { None } else { p.items.len().checked_sub(1).map(|v| v as i32) };
        if sii >= 0 && last_item.is_some_and(|l| sii <= l) && !p.items[sii as usize].sensed {
            let i = sii as usize;
            cl.items_checked[i] = !cl.items_checked[i];
            if cl.items_checked[i] && p.items[i].kind == fws_data::ItemKind::Condition {
                let to_show = cl.items_to_show.clone();
                deferred::conditional_active_items(p.items, &mut cl.items_checked, &mut cl.items_active, &to_show);
            }
            self.store_deferred(cl.clone());
            if cl.items_checked[i] {
                self.dgen_move_down(k, true);
            }
        } else if sii == SPECIAL_INDEX_ACTIVATE {
            cl.procedure_activated = !cl.procedure_activated;
            let source = self.deferred_procs.iter().find(|e| e.state.id == cl.id).and_then(|e| e.items_active_source.clone());
            cl.items_active = source.unwrap_or_else(|| vec![cl.procedure_activated; cl.items_checked.len()]);
            self.store_deferred(cl.clone());
        } else if sii == SPECIAL_INDEX_DEFERRED_PROC_COMPLETE {
            cl.procedure_completed = true;
            self.dgen_set(k, SPECIAL_INDEX_DEFERRED_PROC_RECALL);
            self.store_deferred(cl.clone());
        } else if sii == SPECIAL_INDEX_DEFERRED_PROC_RECALL {
            cl.procedure_completed = false;
            self.dgen_set(k, SPECIAL_INDEX_DEFERRED_PROC_COMPLETE);
            self.store_deferred(cl.clone());
        }
        if let Some(g) = self.deferred_gens.get_mut(k) {
            g.state = cl;
        }
        if self.deferred_gens.get(k).is_some_and(|g| g.sii == SPECIAL_INDEX_ACTIVATE) {
            self.dgen_select_first(k);
        }
    }

    /// FBW re-runs the page subscription when the active deferred procedure leaves the list.
    fn after_deferred_list_change(&mut self) {
        if let Some(a) = self.active_deferred_id {
            if !self.deferred_procs.iter().any(|e| e.state.id == a) {
                self.build_page();
            }
        }
    }

    /// FwsCore: an abnormal procedure was presented while this deferred procedure is active.
    pub fn core_add_deferred(&mut self, id: u32, which_checked: &[bool], which_to_show: Option<&[bool]>) {
        let Some(p) = deferred::procedure(id) else { return };
        let n = which_checked.len();
        let mut active = vec![false; n];
        let mut checked = which_checked.to_vec();
        let to_show = which_to_show.map_or_else(|| vec![true; n], |v| v.to_vec());
        deferred::conditional_active_items(p.items, &mut checked, &mut active, &to_show);
        self.store_deferred(ChecklistState {
            id,
            procedure_completed: false,
            procedure_activated: false,
            items_checked: checked,
            items_active: active,
            items_to_show: to_show,
        });
    }

    /// FwsCore's per-cycle refresh of one active deferred procedure.
    pub fn core_update_deferred(&mut self, id: u32, which_checked: &[bool], which_to_show: &[bool], which_active: Option<&[bool]>) {
        let Some(p) = deferred::procedure(id) else { return };
        let Some(pos) = self.deferred_procs.iter().position(|e| e.state.id == id) else { return };
        self.deferred_procs[pos].items_active_source = which_active.map(|v| v.to_vec());
        let value = &self.deferred_procs[pos].state;
        let n = p.items.len();
        let mut active = match which_active {
            Some(w) if value.procedure_activated => w.to_vec(),
            Some(_) => vec![false; n],
            None => vec![value.procedure_activated; n],
        };
        let mut fused: Vec<bool> = (0..n)
            .map(|i| if p.items[i].sensed { which_checked.get(i).copied().unwrap_or(false) } else { value.items_checked[i] })
            .collect();
        deferred::conditional_active_items(p.items, &mut fused, &mut active, which_to_show);
        let changed = (0..n).any(|i| {
            value.items_to_show[i] != which_to_show[i]
                || value.items_active[i] != active[i]
                || (value.items_checked[i] != fused[i] && p.items[i].sensed)
        });
        if changed {
            let st = ChecklistState {
                id,
                procedure_activated: value.procedure_activated,
                procedure_completed: value.procedure_completed,
                items_checked: fused,
                items_active: active,
                items_to_show: which_to_show.to_vec(),
            };
            self.store_deferred(st);
        }
    }

    /// FwsCore: deferred procedures whose abnormal procedure is no longer active leave the list.
    pub fn core_retain_deferred(&mut self, keep: &[u32]) {
        let before = self.deferred_procs.len();
        self.deferred_procs.retain(|e| keep.contains(&e.state.id));
        if self.deferred_procs.len() != before {
            self.after_deferred_list_change();
        }
    }

    pub fn core_clear_deferred(&mut self) {
        self.core_retain_deferred(&[]);
    }

    fn update_deferred_status(&mut self) {
        let mut st = DeferredStatus { has: [false; 4], completed: [true; 4] };
        for e in &self.deferred_procs {
            if let Some(p) = deferred::procedure(e.state.id) {
                let s = deferred::slot(p.kind);
                st.has[s] = true;
                if !e.state.procedure_completed {
                    st.completed[s] = false;
                }
            }
        }
        self.deferred = st;
    }

    fn navigate_to_parent(&mut self) {
        if self.checklist_id == 0 {
            self.show_requested = false;
        } else {
            self.navigate_to(0);
        }
    }

    fn gen_selectable(&self, p: &NormalProcedure, skip_completed_sensed: bool) -> Vec<i32> {
        let Some(s) = self.gen_state.as_ref() else { return Vec::new() };
        p.items
            .iter()
            .enumerate()
            .filter(|(k, item)| {
                s.items_active[*k]
                    && s.items_to_show[*k]
                    && item.style.is_none_or(|st| !NON_SELECTABLE_STYLES.contains(&st))
                    && (!item.sensed || !skip_completed_sensed || !s.items_checked[*k])
            })
            .map(|(k, _)| k as i32)
            .collect()
    }

    fn gen_select_first(&mut self) {
        let Some(p) = self.open_procedure() else { return };
        if self.gen_state.as_ref().is_some_and(|s| s.procedure_completed) {
            self.set_selected(SPECIAL_INDEX_NORMAL_RESET);
            return;
        }
        let sel = self.gen_selectable(p, true);
        let n = self.gen_state.as_ref().map_or(0, |s| s.items_checked.len()) as i32;
        self.set_selected(sel.first().map_or(n - 1, |f| f - 1));
        self.gen_move_down(true);
    }

    fn gen_move_down(&mut self, skip_completed_sensed: bool) {
        let Some(p) = self.open_procedure() else { return };
        let sel = self.gen_selectable(p, skip_completed_sensed);
        let sii = self.selected_line;
        if sel.is_empty() || sii >= *sel.last().unwrap() {
            self.set_selected(SPECIAL_INDEX_NORMAL_CL_COMPLETE);
        } else if sii == SPECIAL_INDEX_NORMAL_CL_COMPLETE {
            self.set_selected(SPECIAL_INDEX_NORMAL_RESET);
        } else if sii > HIGHEST_SPECIAL_INDEX {
            let last = *sel.last().unwrap();
            let next = sel.iter().copied().find(|v| *v > sii).unwrap_or(last);
            self.set_selected(next.min(last));
        }
    }

    fn gen_move_up(&mut self) {
        let Some(p) = self.open_procedure() else { return };
        let sel = self.gen_selectable(p, true);
        if sel.is_empty() {
            self.gen_select_first();
            return;
        }
        let sii = self.selected_line;
        if sii == SPECIAL_INDEX_NORMAL_RESET {
            self.set_selected(SPECIAL_INDEX_NORMAL_CL_COMPLETE);
        } else if sii == SPECIAL_INDEX_NORMAL_CL_COMPLETE {
            self.set_selected(*sel.last().unwrap());
        } else if sii >= 0 {
            if let Some(prev) = sel.iter().rev().copied().find(|v| *v < sii) {
                self.set_selected(prev.max(0));
            }
        }
    }

    fn gen_check_selected(&mut self) {
        let Some(p) = self.open_procedure() else { return };
        let Some(mut cl) = self.gen_state.clone() else { return };
        let sii = self.selected_line;
        let last_item = p.items.len() as i32 - 1;
        let id = self.checklist_id;
        if sii >= 0 && sii <= last_item && !p.items[sii as usize].sensed {
            let k = sii as usize;
            cl.items_checked[k] = !cl.items_checked[k];
            *self.state_mut(id).unwrap() = cl.clone();
            self.gen_state = Some(cl.clone());
            if cl.items_checked[k] {
                self.gen_move_down(true);
            }
        } else if sii == SPECIAL_INDEX_NORMAL_CL_COMPLETE {
            cl.procedure_completed = true;
            for (k, c) in cl.items_checked.iter_mut().enumerate() {
                if !p.items[k].sensed {
                    *c = true;
                }
            }
            *self.state_mut(id).unwrap() = cl.clone();
            self.show_requested = false;
        } else if sii == SPECIAL_INDEX_NORMAL_RESET {
            cl.procedure_completed = false;
            for (k, c) in cl.items_checked.iter_mut().enumerate() {
                if !p.items[k].sensed {
                    *c = false;
                }
            }
            *self.state_mut(id).unwrap() = cl.clone();
            self.reset(index_of(id).map(Some));
        }
        self.gen_state = Some(cl);
        if sii == SPECIAL_INDEX_NORMAL_RESET {
            self.gen_select_first();
        }
    }

    fn overview_shown_indices(&self) -> Vec<i32> {
        (0..NORMAL_PROCEDURES.len()).filter(|i| self.overview.items_to_show[*i]).map(|i| i as i32).collect()
    }

    fn select_first(&mut self) {
        if self.checklist_id == 0 {
            let first = NORMAL_PROCEDURES
                .iter()
                .enumerate()
                .position(|(i, p)| self.overview.items_to_show[i] && !self.state(p.id).unwrap().procedure_completed);
            self.set_selected(first.map_or(0, |v| v as i32));
        } else if DEFERRED_PROCEDURE_IDS.contains(&self.checklist_id) {
            if let Some(k) = self.active_gen {
                self.dgen_select_first(k);
            }
        } else {
            self.gen_select_first();
        }
    }

    fn move_up(&mut self) {
        if self.checklist_id == 0 {
            let shown = self.overview_shown_indices();
            let pos = shown.iter().position(|v| *v == self.selected_line);
            let prev = match pos {
                Some(p) if p > 0 => shown[p - 1],
                _ => 0,
            };
            self.set_selected(prev.max(0));
        } else if DEFERRED_PROCEDURE_IDS.contains(&self.checklist_id) {
            match self.active_deferred_id {
                Some(aid) => {
                    let Some(k) = self.active_gen else { return };
                    if self.dgen_first_line_selected(k) {
                        let pos = self.visible_deferred.iter().position(|v| *v == aid);
                        if let Some(p) = pos.filter(|p| *p > 0) {
                            self.set_active_deferred(Some(self.visible_deferred[p - 1]));
                        }
                    } else {
                        self.dgen_move_up(k);
                    }
                }
                None => self.set_active_deferred(self.visible_deferred.last().copied()),
            }
        } else {
            self.gen_move_up();
        }
    }

    fn move_down(&mut self) {
        if self.checklist_id == 0 {
            let shown = self.overview_shown_indices();
            let Some(&last) = shown.last() else { return };
            let pos = shown.iter().position(|v| *v == self.selected_line);
            let next = match pos {
                Some(p) => shown.get(p + 1).copied().unwrap_or(last),
                None => shown.first().copied().unwrap_or(last),
            };
            self.set_selected(next.min(last));
        } else if DEFERRED_PROCEDURE_IDS.contains(&self.checklist_id) {
            let Some(k) = self.active_gen else { return };
            match self.active_deferred_id {
                Some(aid) if self.dgen_last_line_selected(k) => {
                    if let Some(p) = self.visible_deferred.iter().position(|v| *v == aid) {
                        self.set_active_deferred(self.visible_deferred.get(p + 1).copied());
                    }
                }
                _ => self.dgen_move_down(k, true),
            }
        } else {
            self.gen_move_down(true);
        }
    }

    /// One FWS cycle, in FwsCore's order: CLR, takeoff-power reset, FwsNormalChecklists.update, then display arbitration.
    pub fn update(&mut self, c: &CycleInputs) {
        if c.flight_phase != 1 {
            self.manual_reset = false;
        }
        if c.takeoff_power && !self.prev_takeoff_power {
            self.reset(index_of(DEPARTURE_CHANGE_ID).map(Some));
        }
        self.prev_takeoff_power = c.takeoff_power;
        if c.pulses.clr && !c.abnormal_sensed_shown && self.shown {
            self.navigate_to_parent();
        }

        if c.pulses.cl {
            self.navigate_to(0);
            self.show_requested = !self.show_requested;
        }
        self.update_deferred_status();
        let deferred_open = self.deferred.has.iter().any(|v| *v) && self.deferred.completed.iter().any(|v| !v);
        if c.approach_auto_display && deferred_open && !self.show_requested {
            self.show_requested = true;
            self.navigate_to(0);
        }

        if self.shown {
            self.update_shown(c);
            for (slot, id) in DEFERRED_PROCEDURE_IDS.iter().enumerate() {
                let i = index_of(*id).unwrap();
                self.overview.items_checked[i] = self.deferred.completed[slot];
                self.overview.items_to_show[i] = self.deferred.has[slot];
            }
        }

        self.shown = !c.abn_proc_requested && self.show_requested;
    }

    fn update_shown(&mut self, c: &CycleInputs) {
        if c.pulses.down {
            self.move_down();
        }
        if c.pulses.up {
            self.move_up();
        }
        if c.pulses.check {
            if self.checklist_id == 0 {
                if let Some(p) = usize::try_from(self.selected_line).ok().and_then(|i| NORMAL_PROCEDURES.get(i)) {
                    self.navigate_to(p.id);
                }
            } else if DEFERRED_PROCEDURE_IDS.contains(&self.checklist_id) {
                if self.active_deferred_id.is_none() {
                    self.navigate_to(0);
                } else if let Some(k) = self.active_gen {
                    self.dgen_check_selected(k);
                }
            } else {
                self.gen_check_selected();
            }
        }

        for idx in 0..NORMAL_PROCEDURES.len() {
            let p = proc_at(idx);
            let deferred_slot = DEFERRED_PROCEDURE_IDS.iter().position(|d| *d == p.id);
            let cl = &self.states[idx];
            let completed = deferred_slot.map_or(cl.procedure_completed, |s| self.deferred.completed[s]);
            let sensed = sensed_items(p.id, &c.sensed, self.rudder_trim);
            let changed_entries: Vec<usize> = sensed
                .iter()
                .enumerate()
                .filter(|(k, v)| v.is_some_and(|v| Some(&v) != cl.items_checked.get(*k)))
                .map(|(k, _)| k)
                .collect();
            if changed_entries.is_empty() && completed == cl.procedure_completed {
                continue;
            }
            let sel = self.selected_line;
            let move_down = sel >= 0
                && changed_entries.contains(&(sel as usize))
                && sensed.get(sel as usize).copied().flatten() == Some(true);
            let cl = &mut self.states[idx];
            cl.procedure_completed = completed;
            for (k, c) in cl.items_checked.iter_mut().enumerate() {
                if p.items[k].sensed {
                    if let Some(Some(v)) = sensed.get(k) {
                        *c = *v;
                    }
                }
            }
            if move_down {
                self.move_down();
            }
        }
    }

    pub fn checklist_index(id: u32) -> Option<usize> {
        index_of(id)
    }
}

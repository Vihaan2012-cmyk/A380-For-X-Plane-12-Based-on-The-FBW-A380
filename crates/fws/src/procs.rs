//! The second half of `FwsCore.update` (clear/recall, the abnormal procedure loop, deferred procedures, memos,
//! INOP SYS/INFO/LIMITATIONS, master lights, display arbitration), FwsAbnormalSensed with its
//! ProcedureLinesGenerator navigation, FwsAbnormalNonSensed and FwsAutoCallouts.

use std::collections::HashMap;

use fws_data::{Item, ItemKind, Level, Style};
use serde_json::Value;

use crate::arinc::Word;
use crate::fwc::{b, Core};
use crate::gen::{abnormal as ab, memos, sts};
use crate::js::{Aural, J};
use crate::jsort;
use crate::nodes::{Memory, Monostable, Pulse};
use crate::vars::Vars;

pub const AURAL_SC_INHIBIT_MS: f64 = 2000.;
const EWD_LINES: usize = 10;
const PFD_MEMO_LINES: usize = 3;
const PFD_LIMITATIONS_LINES: usize = 8;
const SD_STATUS: f64 = 14.;

const SPECIAL_ACTIVATE: i32 = -2;
const SPECIAL_NORMAL_CL_COMPLETE: i32 = -5;
const SPECIAL_NORMAL_RESET: i32 = -6;
const SPECIAL_CLEAR: i32 = -7;
const HIGHEST_SPECIAL: i32 = SPECIAL_ACTIVATE;

const NON_SELECTABLE: [Style; 6] = [Style::Headline, Style::OmissionDots, Style::SeparationLine, Style::SubHeadline, Style::CenteredSubHeadline, Style::Red];

/// `ChecklistState` of an abnormal procedure; item vectors hold what FBW stores (raw callback values included).
#[derive(Clone, Debug, PartialEq)]
pub struct ProcState {
    pub id: u32,
    pub activated: bool,
    pub completed: bool,
    pub checked: Vec<J>,
    pub active: Vec<J>,
    pub to_show: Vec<J>,
    pub timestamps: Option<Vec<J>>,
}

impl ProcState {
    fn json(&self) -> Value {
        let mut v = serde_json::json!({
            "id": self.id.to_string(),
            "procedureActivated": self.activated,
            "procedureCompleted": self.completed,
            "itemsChecked": self.checked.iter().map(J::to_json).collect::<Vec<_>>(),
            "itemsActive": self.active.iter().map(J::to_json).collect::<Vec<_>>(),
            "itemsToShow": self.to_show.iter().map(J::to_json).collect::<Vec<_>>(),
        });
        if let Some(ts) = &self.timestamps {
            v["itemsTimeStamp"] = Value::Array(ts.iter().map(J::to_json).collect());
        }
        v
    }
}

fn at(v: &[J], i: usize) -> J {
    v.get(i).copied().unwrap_or(J::Undef)
}

fn put(v: &mut Vec<J>, i: usize, x: J) {
    while v.len() <= i {
        v.push(J::Undef);
    }
    v[i] = x;
}

fn items(id: u32) -> &'static [Item] {
    fws_data::procedure(id).map_or(&[], |p| p.items)
}

fn is_condition(item: &Item) -> bool {
    item.kind == ItemKind::Condition
}

/// `ProcedureLinesGenerator.conditionalActiveItems`.
pub fn conditional_active_items(items: &[Item], checked: &mut Vec<J>, active: &mut Vec<J>, timestamps: Option<&[J]>, to_show: Option<&[J]>, now_ms: f64) {
    let mut levels: HashMap<usize, J> = HashMap::new();
    let ok = |levels: &HashMap<usize, J>, l: usize| levels.get(&l).is_none_or(|v| v.truthy());
    let mut previous: Option<usize> = None;
    for (i, item) in items.iter().enumerate() {
        let level = item.level as usize;
        if let Some(p) = previous {
            if p > level && ok(&levels, level) {
                for l in (level + 1..=p).rev() {
                    levels.insert(l, J::B(true));
                }
            }
        }
        let mut item_active = true;
        if level != 0 {
            for l in 1..=level {
                if !ok(&levels, l) {
                    item_active = false;
                    break;
                }
            }
        }
        let parent = is_condition(item) && to_show.is_some_and(|t| match at(t, i) {
            J::Undef | J::Null => true,
            x => x.truthy(),
        });
        if parent {
            let timed = item.time.is_some();
            let ts = timestamps.map_or(J::Undef, |t| at(t, i));
            let elapsed = match ts {
                J::N(t) => (now_ms - t) / 1000. > item.time.unwrap_or(0) as f64,
                _ => false,
            };
            let c = at(checked, i);
            let parent_active = if c.truthy() { c } else { J::B(timed && elapsed) };
            levels.insert(level + 1, parent_active);
            if timed {
                put(checked, i, parent_active);
            }
        }
        put(active, i, J::B(item_active));
        previous = Some(level);
    }
}

/// ProcedureLinesGenerator of type Abnormal.
#[derive(Clone, Debug)]
pub struct Gen {
    pub id: u32,
    pub state: ProcState,
    pub sii: i32,
    recommendation: bool,
    piped: bool,
}

impl Gen {
    fn new(state: ProcState, recommendation: bool) -> Self {
        Self { id: state.id, state, sii: 0, recommendation, piped: false }
    }

    fn sensed_proc(&self) -> bool {
        fws_data::procedure(self.id).is_none_or(|p| p.sensed)
    }

    fn selectable(&self, skip_completed_sensed: bool) -> Vec<i32> {
        items(self.id)
            .iter()
            .enumerate()
            .filter(|(i, item)| {
                at(&self.state.active, *i).truthy()
                    && at(&self.state.to_show, *i).truthy()
                    && !NON_SELECTABLE.contains(&item.style)
                    && (!item.sensed || !skip_completed_sensed || !at(&self.state.checked, *i).truthy())
            })
            .map(|(i, _)| i as i32)
            .collect()
    }

    /// `getActualShownItems()` as `toLineData` lays the procedure out (`None` for lines without an item index).
    fn shown_items(&self, is_active: bool) -> Vec<Option<i32>> {
        let mut out = vec![None];
        let its = items(self.id);
        if is_active {
            if self.recommendation && fws_data::procedure(self.id).and_then(|p| p.recommendation).is_some() {
                out.push(None);
            }
            if !self.sensed_proc() {
                out.push(Some(SPECIAL_ACTIVATE));
            }
            for (i, item) in its.iter().enumerate() {
                if !at(&self.state.to_show, i).truthy() {
                    continue;
                }
                let cond = is_condition(item);
                out.push(if !cond || item.sensed { Some(i as i32) } else { None });
                if cond && !item.sensed {
                    out.push(Some(i as i32));
                }
            }
            out.push(Some(SPECIAL_CLEAR));
        } else if !its.is_empty() {
            out.push(None);
        }
        out.push(None);
        out
    }

    fn num_lines_until_selected(&self, is_active: bool) -> i32 {
        self.shown_items(is_active).iter().position(|x| *x == Some(self.sii)).map_or(-1, |p| p as i32)
    }

    fn move_up(&mut self) {
        if !self.state.activated {
            self.sii = SPECIAL_ACTIVATE;
            return;
        }
        let sel = self.selectable(true);
        if sel.is_empty() {
            self.select_first();
            return;
        }
        let sii = self.sii;
        if sii == SPECIAL_NORMAL_RESET {
            self.sii = SPECIAL_NORMAL_CL_COMPLETE;
        } else if sii == -3 || sii == SPECIAL_NORMAL_CL_COMPLETE || sii == SPECIAL_CLEAR {
            self.sii = *sel.last().unwrap();
        } else if sii >= 0 {
            if sii == sel[0] && !self.sensed_proc() {
                self.sii = SPECIAL_ACTIVATE;
            } else if let Some(p) = sel.iter().rev().find(|x| **x < sii) {
                self.sii = (*p).max(0);
            }
        }
    }

    fn move_down(&mut self, skip_completed_sensed: bool) {
        if !self.state.activated {
            self.sii = SPECIAL_ACTIVATE;
            return;
        }
        let sel = self.selectable(skip_completed_sensed);
        if sel.is_empty() || self.sii >= *sel.last().unwrap() {
            self.sii = SPECIAL_CLEAR;
        } else {
            let sii = self.sii;
            let last = *sel.last().unwrap();
            if sii == SPECIAL_NORMAL_CL_COMPLETE {
                self.sii = SPECIAL_NORMAL_RESET;
            } else if sii == SPECIAL_ACTIVATE {
                self.sii = sel[0];
            } else if sii > HIGHEST_SPECIAL {
                self.sii = sel.iter().copied().find(|v| *v > sii).unwrap_or(last).min(last);
            }
        }
    }

    fn select_first(&mut self) {
        if !self.state.activated {
            self.sii = SPECIAL_ACTIVATE;
            return;
        }
        let sel = self.selectable(true);
        self.sii = match sel.first() {
            Some(f) => f - 1,
            None => self.state.checked.len() as i32 - 1,
        };
        self.move_down(true);
    }
}

#[derive(Default)]
pub struct AbnSensed {
    pub shown: bool,
    pub requested: bool,
    pub active_id: Option<u32>,
    pub selected: i32,
    pub show_from_line: i32,
    gens: Vec<Gen>,
    active_gen: Option<u32>,
}

#[derive(Default)]
pub struct NonSensed {
    pub shown: bool,
    pub requested: bool,
    pub checklist_id: u32,
    pub selected: i32,
    pub show_from_line: i32,
    published_id: Option<u32>,
}

#[derive(Clone)]
pub struct AutoCallouts {
    phase10_mtrig: Monostable,
    keep_memory: Memory,
    keep_down_pulse: Pulse,
    keep_pulse: Pulse,
    pub brake_max_braking: bool,
    pub set_max_reverse: bool,
    pub keep_max_reverse: bool,
    pub runway_too_short: bool,
}

impl Default for AutoCallouts {
    fn default() -> Self {
        Self {
            phase10_mtrig: Monostable::new(4.5, false, true),
            keep_memory: Memory::new(true),
            keep_down_pulse: Pulse::new(false),
            keep_pulse: Pulse::new(true),
            brake_max_braking: false,
            set_max_reverse: false,
            keep_max_reverse: false,
            runway_too_short: false,
        }
    }
}

impl AutoCallouts {
    fn update(&mut self, v: &dyn Vars, phase: f64, dt: f64, skip: bool, max_reverse_played: bool) {
        let word = Word::from_simvar(v.get("L:A32NX_ROW_ROP_WORD_1"));
        let mtrig = self.phase10_mtrig.write(phase == 10., dt, skip);
        let rollout = phase == 11. || phase == 10. || (mtrig && (phase == 8. || phase == 9.));
        let requested = word.bit_value_or(11, false);
        let set = v.get("L:A32NX_LEFT_BRAKE_PEDAL_INPUT") > 90. || v.get("L:A32NX_RIGHT_BRAKE_PEDAL_INPUT") > 90.;
        self.brake_max_braking = requested && !set && rollout;
        self.set_max_reverse = word.bit_value_or(12, false) && rollout && !self.brake_max_braking;
        let keep = word.bit_value_or(13, false) && !self.brake_max_braking && rollout;
        let p = self.keep_pulse.write(keep);
        let d = self.keep_down_pulse.write(keep) || max_reverse_played;
        self.keep_max_reverse = self.keep_memory.write(p, d);
        self.runway_too_short = (8. ..=10.).contains(&phase) && word.bit_value_or(15, false);
    }
}

pub struct Loop {
    act: HashMap<String, f64>,
    on: HashMap<String, bool>,
    ident: HashMap<String, J>,
    pub presented: Vec<ProcState>,
    pub cleared: Vec<ProcState>,
    abn_updated: HashMap<u32, Vec<usize>>,
    aural_crc_keys: Vec<u32>,
    aural_sc_keys: Vec<u32>,
    pub left_lines: Vec<String>,
    pub right_lines: Vec<String>,
    pub pfd_memo_lines: Vec<String>,
    pub pfd_limitations_lines: Vec<String>,
    pub information_keys: Vec<Value>,
    pub inop_sys_all_phases_keys: Vec<Value>,
    pub inop_sys_appr_ldg_keys: Vec<Value>,
    pub inop_sys_redund_loss_keys: Vec<Value>,
    pub limitations_all_phases_keys: Vec<Value>,
    pub limitations_appr_ldg_keys: Vec<Value>,
    pub sc_inhibit_until: Option<f64>,
    pub ecl_published: (Value, Value, u32),
    pub abn: AbnSensed,
    pub ns: NonSensed,
    published_abn: Option<Value>,
}

impl Default for Loop {
    fn default() -> Self {
        Self {
            act: HashMap::new(),
            on: HashMap::new(),
            ident: HashMap::new(),
            presented: Vec::new(),
            cleared: Vec::new(),
            abn_updated: HashMap::new(),
            aural_crc_keys: Vec::new(),
            aural_sc_keys: Vec::new(),
            left_lines: vec![String::new(); EWD_LINES],
            right_lines: vec![String::new(); EWD_LINES],
            pfd_memo_lines: vec![String::new(); PFD_MEMO_LINES],
            pfd_limitations_lines: vec![String::new(); PFD_LIMITATIONS_LINES],
            information_keys: Vec::new(),
            inop_sys_all_phases_keys: Vec::new(),
            inop_sys_appr_ldg_keys: Vec::new(),
            inop_sys_redund_loss_keys: Vec::new(),
            limitations_all_phases_keys: Vec::new(),
            limitations_appr_ldg_keys: Vec::new(),
            sc_inhibit_until: None,
            ecl_published: (Value::Null, Value::Null, 0),
            abn: AbnSensed { selected: 1, ..Default::default() },
            ns: NonSensed::default(),
            published_abn: None,
        }
    }
}

/// Keys of `ewdAbnormal` in `Object.entries` order (integer keys ascending).
fn abnormal_keys() -> &'static [u32] {
    static KEYS: std::sync::OnceLock<Vec<u32>> = std::sync::OnceLock::new();
    KEYS.get_or_init(|| {
        let mut k = ab::ABNORMAL_KEYS.to_vec();
        k.sort();
        k.dedup();
        k
    })
}

#[derive(Clone, Copy)]
enum Item_ {
    Abn(u32),
    Sts(&'static sts::StsItem),
}

fn lookup(key: &str) -> Option<Item_> {
    if let Some(i) = sts::LIMITATIONS.iter().find(|i| i.id == key) {
        return Some(Item_::Sts(i));
    }
    if let Some(i) = sts::INFORMATION.iter().find(|i| i.id == key) {
        return Some(Item_::Sts(i));
    }
    if let Some(i) = sts::INOP_SYS.iter().find(|i| i.id == key) {
        return Some(Item_::Sts(i));
    }
    let id: u32 = key.parse().ok()?;
    abnormal_keys().binary_search(&id).ok().map(|_| Item_::Abn(id))
}

fn sts_active(i: &sts::StsItem, table: u8, c: &Core, v: &dyn Vars) -> bool {
    match table {
        0 => sts::inop_sys_active(i.id, c, v),
        1 => sts::information_active(i.id, c, v),
        _ => sts::limitations_active(i.id, c, v),
    }
}

fn mct(item: Item_) -> Option<f64> {
    match item {
        Item_::Abn(id) => fws_data::alert(id).and_then(|a| a.monitor_confirm_time).map(|t| t as f64),
        Item_::Sts(i) => i.monitor_confirm_time,
    }
}

fn suppressors(item: Item_) -> Vec<String> {
    match item {
        Item_::Abn(id) => fws_data::alert(id).map_or(Vec::new(), |a| a.suppressed_by.iter().map(|x| x.to_string()).collect()),
        Item_::Sts(i) => i.not_active_when_item_active.iter().map(|x| x.to_string()).collect(),
    }
}

impl Loop {
    /// The `simVarIsActive.sub` FwsCore's constructor attaches to every suppressable item.
    pub fn observe_activation(&mut self, c: &Core, v: &dyn Vars, sim_time: f64) {
        let mut all: Vec<(String, bool, Option<&'static str>)> = Vec::new();
        for id in abnormal_keys() {
            all.push((id.to_string(), ab::abnormal_active(*id, c, v), ab::abnormal_active_field(*id)));
        }
        for (table, list) in [(0u8, sts::INOP_SYS), (1, sts::INFORMATION), (2, sts::LIMITATIONS)] {
            for i in list {
                all.push((i.id.to_string(), sts_active(i, table, c, v), sts::sts_active_field(i.id)));
            }
        }
        for (k, on, field) in all {
            let ident = field.and_then(|f| c.raw.get(f).copied()).unwrap_or(J::B(on));
            self.on.insert(k.clone(), on);
            if !self.ident.insert(k.clone(), ident).is_some_and(|p| p.strict_eq(&ident)) {
                if on {
                    self.act.insert(k, sim_time);
                } else {
                    self.act.remove(&k);
                }
            }
        }
    }

    fn is_on(&self, key: &str) -> bool {
        self.on.get(key).copied().unwrap_or(false)
    }

    /// `itemIsActiveConsideringFaultSuppression`.
    fn active_supp(&self, key: &str, item: Item_, default_mct: f64, sim: f64) -> bool {
        let since = self.act.get(key).copied().unwrap_or(0.);
        if !self.is_on(key) || sim < since + mct(item).unwrap_or(default_mct) {
            return false;
        }
        !suppressors(item).iter().any(|o| match lookup(o) {
            Some(other) => self.is_on(o) && sim >= since + mct(other).unwrap_or(default_mct),
            None => false,
        })
    }

    fn get(list: &[ProcState], id: u32) -> Option<&ProcState> {
        list.iter().find(|p| p.id == id)
    }

    fn set(list: &mut Vec<ProcState>, st: ProcState) -> bool {
        match list.iter_mut().find(|p| p.id == st.id) {
            Some(p) => {
                *p = st;
                false
            }
            None => {
                list.push(st);
                true
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Change {
    Added,
    Changed,
    Deleted,
}

pub fn failure_number(level: Level) -> i8 {
    match level {
        Level::Hidden => -1,
        Level::None => 0,
        Level::Advisory => 1,
        Level::Caution => 2,
        Level::Warning => 3,
    }
}

fn priority(id: u32) -> i32 {
    if id / 100_000 == 9998 {
        0
    } else if fws_data::procedure(id).is_some_and(|p| !p.sensed) {
        4
    } else {
        fws_data::alert(id).map_or(0, |a| failure_number(a.level) as i32)
    }
}

fn sorted_presented(c: &Core) -> Vec<ProcState> {
    let mut v = c.l.presented.clone();
    v.sort_by_key(|p| -priority(p.id));
    v
}

fn presented_set(c: &mut Core, st: ProcState) {
    let id = st.id;
    let added = Loop::set(&mut c.l.presented, st.clone());
    on_presented_change(c, if added { Change::Added } else { Change::Changed }, id, Some(st));
}

fn presented_delete(c: &mut Core, id: u32) {
    let before = c.l.presented.len();
    c.l.presented.retain(|p| p.id != id);
    if c.l.presented.len() != before {
        on_presented_change(c, Change::Deleted, id, None);
    }
}

fn set_gen_sii(c: &mut Core, id: u32, f: impl FnOnce(&mut Gen)) {
    let Some(g) = c.l.abn.gens.iter_mut().find(|g| g.id == id) else { return };
    let before = g.sii;
    f(g);
    let (after, piped) = (g.sii, g.piped);
    if after != before && piped {
        set_selected(c, after);
    }
}

fn set_selected(c: &mut Core, sii: i32) {
    if c.l.abn.selected == sii {
        return;
    }
    c.l.abn.selected = sii;
    let Some(active) = c.l.abn.active_gen else { return };
    let is_active = c.l.abn.active_id == Some(active);
    if let Some(g) = c.l.abn.gens.iter().find(|g| g.id == active) {
        let n = g.num_lines_until_selected(is_active);
        c.l.abn.show_from_line = (n - fws_ecl::WD_NUM_LINES as i32 + 2).max(0);
    }
}

fn pipe(c: &mut Core, id: u32) {
    let Some(g) = c.l.abn.gens.iter_mut().find(|g| g.id == id) else { return };
    g.piped = true;
    let sii = g.sii;
    set_selected(c, sii);
}

/// The `presentedAbnormalProceduresList.sub` handler of FwsAbnormalSensed.
fn on_presented_change(c: &mut Core, change: Change, key: u32, value: Option<ProcState>) {
    let sorted = sorted_presented(c);
    match change {
        Change::Added => {
            let st = value.expect("value");
            let rec = !c.s.aircraft_on_ground;
            c.l.abn.gens.push(Gen::new(st, rec));
        }
        Change::Changed => {
            let st = value.expect("value");
            if let Some(g) = c.l.abn.gens.iter_mut().find(|g| g.id == key) {
                g.state = st;
            }
        }
        Change::Deleted => match c.l.abn.gens.iter().position(|g| g.id == key) {
            Some(i) => {
                c.l.abn.gens.remove(i);
            }
            None => {
                c.l.abn.gens.pop();
            }
        },
    }
    for val in &sorted {
        if Some(val.id) == c.l.abn.active_id && c.l.abn.gens.iter().any(|g| g.id == val.id) {
            c.l.abn.active_gen = Some(val.id);
            pipe(c, val.id);
        }
    }
    let new_active = sorted.first().map(|p| p.id);
    if new_active != c.l.abn.active_id {
        c.l.abn.active_id = new_active;
        if let Some(id) = new_active {
            if c.l.abn.gens.iter().any(|g| g.id == id) {
                c.l.abn.active_gen = Some(id);
                pipe(c, id);
            }
            if let Some(a) = c.l.abn.active_gen {
                set_gen_sii(c, a, |g| g.select_first());
            }
        }
    }
    let json = Value::Array(sorted.iter().map(ProcState::json).collect());
    c.l.published_abn = Some(json.clone());
    c.publish("fws_abn_sensed_procedures", json);
}

pub fn publish_abnormal_initial(c: &mut Core) {
    c.publish("fws_abn_sensed_procedures", Value::Array(Vec::new()));
    publish_non_sensed_id(c);
}

fn publish_non_sensed_id(c: &mut Core) {
    let id = c.l.ns.checklist_id;
    if c.l.ns.published_id == Some(id) {
        return;
    }
    c.l.ns.published_id = Some(id);
    c.publish("fws_abn_non_sensed_id", Value::from(id));
    if id > 10 {
        let on = c.active_abnormal_non_sensed_keys.contains(&id);
        c.publish("fws_abn_non_sensed_current_active", Value::Bool(on));
    }
}

pub fn reset_non_sensed(c: &mut Core) {
    c.active_abnormal_non_sensed_keys.clear();
    c.l.ns.requested = false;
}

/// Startup / 50 minute shutdown: the abnormal and deferred lists are emptied.
pub fn clear_lists(c: &mut Core) {
    c.l.abn.requested = false;
    let ids: Vec<u32> = c.l.presented.iter().map(|p| p.id).collect();
    for id in ids {
        presented_delete(c, id);
    }
    c.l.cleared.clear();
    c.ecl.core_clear_deferred();
    c.all_current_failures.clear();
    c.presented_failures.clear();
    c.recall_failures.clear();
}

/// `FwsAbnormalSensed.clearActiveProcedure`.
fn clear_active_procedure(c: &mut Core, new_state: Option<&ProcState>) {
    let Some(active) = c.l.abn.active_id else { return };
    if c.presented_failures.len() == 1 {
        if !c.s.ecam_status_normal {
            c.write_var("L:A32NX_ECAM_SD_CURRENT_PAGE_INDEX", SD_STATUS);
        }
        let d = c.ecl.deferred;
        if d.has.iter().any(|v| *v) && d.completed.iter().any(|v| !v) && !c.ecl.show_requested {
            c.ecl.show_requested = true;
            c.ecl.checklist_id = 0;
            c.ecl.selected_line = 0;
            c.ecl.show_from_line = 0;
        }
    }
    js_splice_one(&mut c.presented_failures, active);
    if new_state.is_some_and(|s| !s.activated) && fws_data::procedure(active).is_some_and(|p| !p.sensed) {
        c.active_abnormal_non_sensed_keys.remove(&active);
        js_splice_one(&mut c.all_current_failures, active);
    }
    c.recall_failures = c.all_current_failures.iter().copied().filter(|k| !c.presented_failures.contains(k)).collect();
}

/// `array.splice(array.indexOf(x), 1)`: removes the last element when `x` is missing.
fn js_splice_one(v: &mut Vec<u32>, x: u32) {
    match v.iter().position(|k| *k == x) {
        Some(i) => {
            v.remove(i);
        }
        None => {
            v.pop();
        }
    }
}

fn check_selected(c: &mut Core) {
    let Some(id) = c.l.abn.active_gen else { return };
    let Some(g) = c.l.abn.gens.iter().find(|g| g.id == id).cloned() else { return };
    let is_active = c.l.abn.active_id == Some(id);
    let its = items(id);
    let mut cl = g.state.clone();
    let shown: Vec<i32> = g.shown_items(is_active).into_iter().flatten().filter(|v| *v > HIGHEST_SPECIAL).collect();
    let last = shown.last().copied();
    let sii = g.sii;
    let selected_item = usize::try_from(sii).ok().and_then(|i| its.get(i));
    if sii >= 0 && last.is_some_and(|l| sii <= l) && !selected_item.is_some_and(|i| i.sensed) {
        let i = sii as usize;
        let now = !at(&cl.checked, i).truthy();
        put(&mut cl.checked, i, J::B(now));
        presented_set(c, cl.clone());
        if now {
            if selected_item.is_some_and(is_condition) {
                let (mut ch, mut ac) = (cl.checked.clone(), cl.active.clone());
                conditional_active_items(its, &mut ch, &mut ac, cl.timestamps.as_deref(), Some(&cl.to_show), c.now_ms);
                cl.checked = ch;
                cl.active = ac;
                if let Some(p) = c.l.presented.iter_mut().find(|p| p.id == id) {
                    *p = cl.clone();
                }
            }
            set_gen_sii(c, id, |g| {
                g.state = cl.clone();
                g.move_down(true);
            });
        }
    } else if sii == SPECIAL_CLEAR {
        clear_active_procedure(c, Some(&cl));
    } else if sii == SPECIAL_ACTIVATE {
        cl.activated = !cl.activated;
        clear_active_procedure(c, Some(&cl));
    }
    if let Some(g) = c.l.abn.gens.iter_mut().find(|g| g.id == id) {
        g.state = cl;
    }
    if sii == SPECIAL_NORMAL_RESET || sii == SPECIAL_ACTIVATE {
        set_gen_sii(c, id, |g| g.select_first());
    }
}

fn message_priority(raw: Option<&str>) -> f64 {
    let Some(m) = raw.filter(|m| !m.is_empty()) else { return 10. };
    let t = m.trim();
    let head: String = t.chars().take(3).collect();
    match head.as_str() {
        "\u{1b}<6" => 0.,
        "\u{1b}<2" => 1.,
        "\u{1b}<4" => 2.,
        "\u{1b}<3" => 3.,
        _ => 10.,
    }
}

fn memo_priority(k: &str) -> f64 {
    message_priority(fws_ecl::memo_message(k).map(|m| m.raw))
}

fn limitation_priority(k: &J) -> f64 {
    message_priority(k.as_str().and_then(fws_ecl::limitation_message).map(|m| m.raw))
}

fn map_order(arr: &mut Vec<Option<&'static str>>, order: &[&'static str]) {
    let idx = |x: &&str| order.iter().position(|o| o == x).map_or(-1, |p| p as i64);
    jsort::sort_by(arr, |a, b| if idx(a) > idx(b) { 1. } else { -1. });
}

fn push_key_unique(val: Option<Vec<J>>, push_to: &mut Vec<J>) {
    let Some(val) = val else { return };
    for key in val {
        if key.truthy() && !push_to.iter().any(|k| k.strict_eq(&key)) {
            push_to.push(key);
        }
    }
}

fn lines(v: &[Option<&'static str>], n: usize) -> Vec<String> {
    (0..n).map(|i| v.get(i).copied().flatten().unwrap_or("").to_string()).collect()
}

fn memo_codes(codes: Vec<J>, table: &'static [&'static str]) -> Vec<Option<&'static str>> {
    codes
        .into_iter()
        .filter(|e| *e != J::Null)
        .map(|e| match e {
            J::N(n) if n >= 0. && n.fract() == 0. => table.get(n as usize).copied(),
            _ => None,
        })
        .collect()
}

fn keys_json(v: &[J]) -> Vec<Value> {
    v.iter().map(J::to_json).collect()
}

/// `FwsCore.update` from CLEAR AND RECALL to the end, then the per-cycle updates of the helper classes.
pub fn post(c: &mut Core, v: &dyn Vars, dt: f64) {
    let sim = c.s.sim_time;
    let flight_phase = c.s.flight_phase;
    let phase_u8 = flight_phase as u8;
    if c.s.clr_pulse_node.read() {
        if c.l.abn.shown {
            clear_active_procedure(c, None);
        } else if c.ecl.shown {
        } else if c.l.ns.shown {
            if c.l.ns.checklist_id == 0 {
                c.l.ns.requested = false;
            } else {
                c.l.ns.checklist_id = 0;
                c.l.ns.selected = 0;
                publish_non_sensed_id(c);
            }
        }
    }

    if c.s.rcl_up_pulse_node.read() && !c.recall_failures.is_empty() {
        let key = c.recall_failures.remove(0);
        c.presented_failures.push(key);
        if let Some(st) = Loop::get(&c.l.cleared, key).cloned() {
            presented_set(c, st);
            c.l.cleared.retain(|p| p.id != key);
        }
    }

    c.s.land_asap = !c.s.aircraft_on_ground
        && (c.s.apu_fire_detected
            || c.s.eng1_fire_detected
            || c.s.eng2_fire_detected
            || c.s.eng3_fire_detected
            || c.s.eng4_fire_detected
            || c.s.mlg_fire_detected
            || c.smoke_fumes_activated()
            || c.s.all_engines_failure);
    observe(c, v);

    let mut temp_left: Vec<Option<&'static str>> = Vec::new();
    let mut temp_right: Vec<Option<&'static str>> = Vec::new();
    let mut all_failure_keys: Vec<u32> = Vec::new();
    let mut sts_info: Vec<J> = Vec::new();
    let mut sts_inop_all: Vec<J> = Vec::new();
    let mut sts_inop_appr: Vec<J> = Vec::new();
    let mut sts_redund: Vec<J> = Vec::new();
    let mut lim_all: Vec<J> = Vec::new();
    let mut lim_appr: Vec<J> = Vec::new();
    let mut pfd_lim: Vec<J> = Vec::new();
    let mut failure_keys: Vec<u32> = c.presented_failures.clone();
    let mut recall_keys: Vec<u32> = c.recall_failures.clone();
    let mut failure_system_count = 0;
    let mut crc_keys: Vec<u32> = Vec::new();
    let mut sc_keys: Vec<u32> = Vec::new();

    for id in abnormal_keys() {
        if !c.l.active_supp(&id.to_string(), Item_::Abn(*id), 0.6, sim) {
            failure_keys.retain(|k| k != id);
            recall_keys.retain(|k| k != id);
        }
    }
    c.recall_failures = recall_keys.clone();
    c.s.non_cancellable_warning_count = 0.;

    c.l.abn_updated.clear();
    for &key in abnormal_keys() {
        let Some(alert) = fws_data::alert(key) else { continue };
        let proc = fws_data::procedure(key);
        let is_procedure = !alert.non_procedure_key;
        let new_warning = if is_procedure {
            !c.presented_failures.contains(&key) && !recall_keys.contains(&key)
        } else {
            !c.all_current_failures.contains(&key)
        };
        if proc.is_none() && is_procedure {
            continue;
        }
        if new_warning && alert.phases_inhibited.contains(phase_u8) {
            continue;
        }
        let failure = failure_number(alert.level);
        if c.l.active_supp(&key.to_string(), Item_::Abn(key), 0.6, sim) {
            if new_warning {
                if is_procedure {
                    failure_keys.push(key);
                }
                if failure == 3 {
                    c.s.request_master_warning_from_faults = true;
                }
                if failure == 2 {
                    c.s.request_master_caution_from_faults = true;
                }
            }
            if is_procedure {
                let its = items(key);
                let raw_checked = ab::abnormal_checked(key, c, v).unwrap_or_default();
                let mut checked: Vec<J> = raw_checked.iter().enumerate().map(|(i, v)| J::B(its.get(i).is_some_and(|it| it.sensed) && v.truthy())).collect();
                let to_show = ab::abnormal_to_show(key, c, v).unwrap_or_else(|| vec![J::B(true); checked.len()]);
                let active_src = ab::abnormal_active_items(key, c, v);
                let mut active = active_src.clone().unwrap_or_else(|| vec![J::B(true); checked.len()]);
                let timer = ab::abnormal_timer(key, c, v);
                conditional_active_items(its, &mut checked, &mut active, timer.as_deref(), Some(&to_show), c.now_ms);
                let prev_presented = Loop::get(&c.l.presented, key).cloned();
                let prev_cleared = Loop::get(&c.l.cleared, key).is_some();
                if prev_presented.is_none() && !prev_cleared {
                    presented_set(
                        c,
                        ProcState { id: key, activated: true, completed: false, checked, active, to_show, timestamps: timer },
                    );
                    for &dkey in ab::DEFERRED_KEYS {
                        let Some(d) = fws_data::deferred(dkey) else { continue };
                        if d.from_abnormal.contains(&key) && ab::deferred_active(dkey, c, v) {
                            let dchecked: Vec<bool> = ab::deferred_checked(dkey, c, v).unwrap_or_default().iter().map(J::truthy).collect();
                            let dshow: Option<Vec<bool>> = ab::deferred_to_show(dkey, c, v).map(|s| s.iter().map(J::truthy).collect());
                            c.ecl.core_add_deferred(dkey, &dchecked, dshow.as_deref());
                        }
                    }
                } else if let Some(prev) = prev_presented {
                    let mut fused: Vec<J> = prev.checked.iter().enumerate().map(|(i, val)| if its.get(i).is_some_and(|it| it.sensed) { at(&checked, i) } else { J::B(val.truthy()) }).collect();
                    let mut active2 = active.clone();
                    conditional_active_items(its, &mut fused, &mut active2, timer.as_deref(), Some(&to_show), c.now_ms);
                    let mut changed = Vec::new();
                    for (idx, item) in its.iter().enumerate() {
                        let timed = item.time.is_some();
                        if !at(&prev.to_show, idx).strict_eq(&at(&to_show, idx))
                            || !at(&prev.active, idx).strict_eq(&at(&active2, idx))
                            || (!at(&prev.checked, idx).strict_eq(&at(&fused, idx)) && item.sensed)
                            || (timed
                                && timer.is_some()
                                && prev.timestamps.is_some()
                                && !at(prev.timestamps.as_ref().unwrap(), idx).strict_eq(&at(timer.as_ref().unwrap(), idx)))
                        {
                            changed.push(idx);
                        }
                    }
                    let any = !changed.is_empty();
                    c.l.abn_updated.insert(key, changed);
                    if any {
                        let st = ProcState {
                            id: key,
                            activated: prev.activated,
                            completed: prev.completed,
                            checked: fused,
                            active: (0..prev.active.len()).map(|i| at(&active2, i)).collect(),
                            to_show: (0..prev.to_show.len()).map(|i| at(&to_show, i)).collect(),
                            timestamps: prev.timestamps.as_ref().map(|t| (0..t.len()).map(|i| timer.as_ref().map_or(J::Undef, |x| at(x, i))).collect()),
                        };
                        presented_set(c, st);
                    }
                }
            }
            if !alert.cancel && failure == 3 {
                c.s.non_cancellable_warning_count += 1.;
            }
            let default_aural = matches!(alert.aural, fws_data::Aural::Default);
            if default_aural && failure == 3 {
                if new_warning {
                    c.set_aural_crc(true);
                }
                crc_keys.push(key);
            }
            if default_aural && failure == 2 {
                if new_warning {
                    c.s.aural_single_chime_pending = true;
                }
                sc_keys.push(key);
            }
            if is_procedure {
                all_failure_keys.push(key);
                let checked_state: Option<Vec<J>> = Loop::get(&c.l.presented, key).or_else(|| Loop::get(&c.l.cleared, key)).map(|p| p.checked.clone());
                if let Some(cs) = checked_state {
                    push_key_unique(ab::abnormal_info(key, c, v, &cs), &mut sts_info);
                    push_key_unique(ab::abnormal_inop_sys_all_phases(key, c, v, &cs), &mut sts_inop_all);
                    push_key_unique(ab::abnormal_inop_sys_appr_ldg(key, c, v, &cs), &mut sts_inop_appr);
                    push_key_unique(ab::abnormal_limitations_all_phases(key, c, v, &cs), &mut lim_all);
                    push_key_unique(ab::abnormal_limitations_appr_ldg(key, c, v, &cs), &mut lim_appr);
                    push_key_unique(ab::abnormal_limitations_pfd(key, c, v, &cs), &mut pfd_lim);
                }
                if !recall_keys.contains(&key) && alert.sd_page.value() > -1 {
                    failure_system_count += 1;
                }
            } else {
                all_failure_keys.push(key);
            }
        }
        match ab::abnormal_aural(key, c, v) {
            Some(Aural::Crc) => {
                if !c.l.aural_crc_keys.contains(&key) {
                    c.set_aural_crc(true);
                }
                crc_keys.push(key);
            }
            Some(Aural::SingleChime) => {
                if !c.l.aural_sc_keys.contains(&key) {
                    c.s.aural_single_chime_pending = true;
                }
                sc_keys.push(key);
            }
            Some(Aural::CavalryCharge) => c.sound.enqueue_sound("cavalryChargeCont"),
            _ => {}
        }
    }

    let deferred_ids: Vec<u32> = c.ecl.deferred_procs.iter().map(|e| e.state.id).collect();
    for dkey in deferred_ids {
        let dchecked: Vec<bool> = ab::deferred_checked(dkey, c, v).unwrap_or_default().iter().map(J::truthy).collect();
        let dshow: Vec<bool> = ab::deferred_to_show(dkey, c, v).unwrap_or_default().iter().map(J::truthy).collect();
        c.ecl.core_update_deferred(dkey, &dchecked, &dshow, None);
    }
    let mut deferred_keys: Vec<u32> = Vec::new();
    for fk in &all_failure_keys {
        for &dkey in ab::DEFERRED_KEYS {
            if fws_data::deferred(dkey).is_some_and(|d| d.from_abnormal.contains(fk)) && ab::deferred_active(dkey, c, v) {
                deferred_keys.push(dkey);
            }
        }
    }
    c.ecl.core_retain_deferred(&deferred_keys);

    let presented_ids: Vec<u32> = c.l.presented.iter().map(|p| p.id).collect();
    for key in presented_ids {
        let in_recall = c.recall_failures.contains(&key);
        if !all_failure_keys.contains(&key) || in_recall {
            if in_recall {
                if let Some(st) = Loop::get(&c.l.presented, key).cloned() {
                    Loop::set(&mut c.l.cleared, st);
                }
            } else {
                c.l.cleared.retain(|p| p.id != key);
            }
            presented_delete(c, key);
        }
    }
    c.l.cleared.retain(|p| all_failure_keys.contains(&p.id));

    c.l.aural_crc_keys = crc_keys;
    c.l.aural_sc_keys = sc_keys;
    if c.l.aural_crc_keys.is_empty() {
        c.set_aural_crc(false);
    }
    if c.l.aural_sc_keys.is_empty() {
        c.s.aural_single_chime_pending = false;
    }
    c.all_current_failures = all_failure_keys.clone();
    c.presented_failures = failure_keys;

    for m in memos::EWD_MEMOS {
        if memos::ewd_memos_active(m.key, c, v) && !memos::ewd_memos_inhibit(m.key, c, v) && !m.flight_phase_inhib.contains(&phase_u8) {
            let new_code = memo_codes(memos::ewd_memos_codes(m.key, c, v), m.codes);
            temp_right.retain(|e| !e.is_some_and(|e| m.codes.contains(&e)));
            temp_right.extend(new_code);
        }
    }
    for m in memos::TO_LDG_MEMOS {
        if memos::to_ldg_memos_active(m.key, c, v) && !memos::to_ldg_memos_inhibit(m.key, c, v) && !m.flight_phase_inhib.contains(&phase_u8) {
            temp_left.extend(memo_codes(memos::to_ldg_memos_codes(m.key, c, v), m.codes));
        }
    }
    let mut order_left: Vec<&'static str> = Vec::new();
    let mut order_right: Vec<&'static str> = Vec::new();
    for m in memos::TO_LDG_MEMOS {
        if m.left_side {
            order_left.extend(m.codes);
        } else {
            order_right.extend(m.codes);
        }
    }
    map_order(&mut temp_left, &order_left);
    map_order(&mut temp_right, &order_right);
    jsort::sort_by(&mut temp_right, |a, b| memo_priority(a) - memo_priority(b));

    for i in sts::INOP_SYS {
        let item = Item_::Sts(i);
        if c.l.active_supp(i.id, item, 1.0, sim) {
            let k = J::S(i.id);
            let has = |v: &Vec<J>| v.iter().any(|x| x.strict_eq(&k));
            if !i.redundancy_loss && i.phase == sts::Phase::AllPhases && !has(&sts_inop_all) {
                sts_inop_all.push(k);
            } else if !i.redundancy_loss && i.phase == sts::Phase::ApprLdg && !has(&sts_inop_appr) {
                sts_inop_appr.push(k);
            } else if i.redundancy_loss && !has(&sts_redund) {
                sts_redund.push(k);
            }
        }
    }
    for i in sts::INFORMATION {
        let k = J::S(i.id);
        if c.l.active_supp(i.id, Item_::Sts(i), 1.0, sim) && !sts_info.iter().any(|x| x.strict_eq(&k)) {
            sts_info.push(k);
        }
    }
    for i in sts::LIMITATIONS {
        if c.l.active_supp(i.id, Item_::Sts(i), 1.0, sim) {
            let k = J::S(i.id);
            let has = |v: &Vec<J>| v.iter().any(|x| x.strict_eq(&k));
            if i.phase == sts::Phase::AllPhases && !has(&lim_all) {
                lim_all.push(k);
            } else if i.phase == sts::Phase::ApprLdg && !has(&lim_appr) {
                lim_appr.push(k);
            }
            if i.pfd && !has(&pfd_lim) {
                pfd_lim.push(k);
            }
        }
    }

    let level = |k: &u32| fws_data::alert(*k).map_or(-1, |a| failure_number(a.level));
    if !all_failure_keys.iter().any(|k| level(k) == 2) {
        c.s.request_master_caution_from_faults = false;
    }
    if !all_failure_keys.iter().any(|k| level(k) == 3) && c.s.non_cancellable_warning_count == 0. {
        c.s.request_master_warning_from_faults = false;
    }
    c.s.master_caution = c.s.request_master_caution_from_faults || c.s.request_master_caution_from_a_thr_off;
    c.s.master_warning = c.s.request_master_warning_from_faults || c.s.request_master_warning_from_ap_off;
    c.write_var("L:A32NX_ECAM_FAILURE_ACTIVE", (!c.presented_failures.is_empty()) as u8 as f64);
    if failure_system_count == 0 {
        c.write_var("L:A32NX_ECAM_SFAIL", -1.);
    } else {
        c.presented_failures.sort_by_key(|id| -priority(*id));
        if let Some(page) = c.presented_failures.first().and_then(|id| fws_data::alert(*id)).map(|a| a.sd_page.value()).filter(|p| *p > -1) {
            c.write_var("L:A32NX_ECAM_SFAIL", page as f64);
        }
    }

    c.l.left_lines = lines(&temp_left, EWD_LINES);
    c.l.right_lines = lines(&temp_right, EWD_LINES);
    let mut pfd_memos: Vec<Option<&'static str>> = temp_right.iter().copied().filter(|x| x.is_some_and(|x| fws_ecl::PFD_MEMO_DISPLAY.contains(&x))).collect();
    if c.s.auto_thrust_involuntary_pfd_memo_memory_node.read() && !pfd_memos.contains(&Some(fws_ecl::A_THR_OFF_MEMO_KEY)) {
        pfd_memos.push(Some(fws_ecl::A_THR_OFF_MEMO_KEY));
    }
    jsort::sort_by(&mut pfd_memos, |a, b| memo_priority(a) - memo_priority(b));
    c.l.pfd_memo_lines = lines(&pfd_memos, PFD_MEMO_LINES);

    c.l.information_keys = keys_json(&sts_info);
    c.l.inop_sys_all_phases_keys = keys_json(&sts_inop_all);
    c.l.inop_sys_appr_ldg_keys = keys_json(&sts_inop_appr);
    c.l.inop_sys_redund_loss_keys = keys_json(&sts_redund);

    let one = J::S("1");
    let two = J::S("2");
    let has = |v: &Vec<J>, k: &J| v.iter().any(|x| x.strict_eq(k));
    if has(&lim_all, &one) && has(&lim_all, &two) {
        if let Some(p) = lim_all.iter().position(|x| x.strict_eq(&two)) {
            lim_all.remove(p);
        }
    }
    let mut lim_all_o: Vec<Option<J>> = lim_all.iter().copied().map(Some).collect();
    jsort::sort_by(&mut lim_all_o, |a, b| limitation_priority(a) - limitation_priority(b));
    let mut lim_appr_o: Vec<Option<J>> = lim_appr.iter().copied().map(Some).collect();
    jsort::sort_by(&mut lim_appr_o, |a, b| limitation_priority(a) - limitation_priority(b));
    let lim_all_sorted: Vec<J> = lim_all_o.into_iter().map(|x| x.unwrap_or(J::Undef)).collect();
    let lim_appr_sorted: Vec<J> = lim_appr_o.into_iter().map(|x| x.unwrap_or(J::Undef)).collect();
    c.l.limitations_all_phases_keys = keys_json(&lim_all_sorted);
    c.l.limitations_appr_ldg_keys = keys_json(&lim_appr_sorted);

    let mut uniq: Vec<J> = Vec::new();
    for k in &pfd_lim {
        if !uniq.iter().any(|x| x.strict_eq(k)) {
            uniq.push(*k);
        }
    }
    let mut pfd_o: Vec<Option<J>> = uniq.into_iter().map(Some).collect();
    jsort::sort_by(&mut pfd_o, |a, b| limitation_priority(a) - limitation_priority(b));
    let mut pfd_combined: Vec<J> = pfd_o.into_iter().map(|x| x.unwrap_or(J::Undef)).collect();
    if has(&pfd_combined, &one) && has(&pfd_combined, &two) {
        if let Some(p) = pfd_combined.iter().position(|x| x.strict_eq(&two)) {
            pfd_combined.remove(p);
        }
    }
    c.l.pfd_limitations_lines = (0..PFD_LIMITATIONS_LINES)
        .map(|i| match pfd_combined.get(i) {
            Some(J::S(s)) => s.to_string(),
            Some(J::N(n)) => format!("{n}"),
            _ => String::new(),
        })
        .collect();

    c.s.ecam_status_normal = sts_info.is_empty() && sts_inop_all.is_empty() && sts_inop_appr.is_empty() && lim_all_sorted.is_empty() && lim_appr_sorted.is_empty();
    let sd_sts_shown = v.get("L:A32NX_ECAM_SD_CURRENT_PAGE_INDEX") == SD_STATUS;
    c.s.ecam_ewd_show_sts_indication = !c.s.ecam_status_normal && !sd_sts_shown;

    let baro_mode = v.get("L:XMLVAR_Baro1_Mode");
    c.s.approach_auto_display_qnh_set_pulse_node.write(baro_mode == 0. || baro_mode == 1.);
    let slats = c.s.flaps_handle > 0.;
    c.s.approach_auto_display_slats_extended_pulse_node.write(slats);

    let chime = (c.s.aural_single_chime_pending || c.s.request_single_chime_from_a_thr_off) && !c.s.aural_crc_active;
    if chime && c.l.sc_inhibit_until.is_none() {
        c.s.aural_single_chime_pending = false;
        c.s.request_single_chime_from_a_thr_off = false;
        c.sound.enqueue_sound("singleChime");
        c.l.sc_inhibit_until = Some(c.now_ms + AURAL_SC_INHIBIT_MS);
    }

    let pulses = fws_ecl::ecl::EcpPulses {
        cl: c.s.cl_pulse_node.read(),
        check: c.s.cl_check_pulse_node.read(),
        up: c.s.cl_up_pulse_node.read(),
        down: c.s.cl_down_pulse_node.read(),
        clr: c.s.clr_pulse_node.read(),
    };
    let lgciu = |n: u8| Word::from_simvar(v.get(&format!("L:A32NX_LGCIU_{n}_DISCRETE_WORD_1")));
    let _ = lgciu;
    let sensed = fws_ecl::ecl::SensedInputs {
        seat_belt_switch: v.get("A:CABIN SEATBELTS ALERT SWITCH") == 1.,
        beacon_on: b(v.get("A:LIGHT BEACON")),
        rudder_trim_deg: c.s.rudder_trim_position,
        spoilers_armed: c.s.spoilers_armed,
        flaps_handle: v.get("L:A32NX_FLAPS_HANDLE_INDEX") as i32,
        auto_brake_mode: c.s.auto_brake as i32,
        to_config_normal: c.s.to_config_normal,
        all_gear_downlocked: c.s.is_all_gear_downlocked,
        landing_conf3: b(v.get("L:A380X_FM_LANDING_CONF3")),
        engine_master: [c.s.engine1_master, c.s.engine2_master, c.s.engine3_master, c.s.engine4_master],
        all_fuel_pumps_off: c.s.all_fuel_pumps_off,
        crew_oxygen_pb: b(v.get("L:PUSH_OVHD_OXYGEN_CREW")),
        emer_exit_light_position: v.get("L:XMLVAR_SWITCH_OVHD_INTLT_EMEREXIT_Position") as i32,
    };
    let approach = c.l.presented.is_empty()
        && phase_u8 == 8
        && c.s.adr_pressure_altitude.unwrap_or(0.) < 20_000.
        && (c.s.approach_auto_display_qnh_set_pulse_node.read() || c.s.approach_auto_display_slats_extended_pulse_node.read());
    let cycle = fws_ecl::ecl::CycleInputs {
        sensed,
        pulses,
        flight_phase: phase_u8,
        takeoff_power: c.s.eng1_or2_takeoff_power || c.s.eng3_or4_takeoff_power,
        approach_auto_display: approach,
        abnormal_sensed_shown: c.l.abn.shown,
        abn_proc_requested: c.l.ns.requested,
    };
    c.ecl.update(&cycle);
    c.publish_normal_checklists(false);

    abnormal_sensed_update(c);
    non_sensed_update(c, v);
    crate::sd::update(c, v, dt);
    let played = c.sound.keep_max_reverse_played();
    let skip = c.skip;
    c.callouts.update(v, flight_phase, dt, skip, played);
    c.callout_sounds();

    if c.l.ns.requested {
        c.l.ns.shown = true;
        c.l.abn.shown = false;
        let (sel, from) = (c.l.ns.selected, c.l.ns.show_from_line);
        c.publish("fws_active_item", Value::from(sel));
        c.publish("fws_show_from_line", Value::from(from));
        c.s.ecam_ewd_show_failure_pending_indication = false;
    } else if c.ecl.show_requested {
        c.l.ns.shown = false;
        c.l.abn.shown = false;
        c.publish("fws_active_item", Value::from(c.ecl.selected_line));
        if let Some(id) = c.ecl.active_deferred_id {
            c.publish("fws_active_procedure", Value::from(id.to_string()));
        }
        c.publish("fws_show_from_line", Value::from(c.ecl.show_from_line));
        c.s.ecam_ewd_show_failure_pending_indication = c.l.abn.requested;
    } else if c.l.abn.requested {
        c.l.ns.shown = false;
        let was = c.l.abn.shown;
        c.l.abn.shown = true;
        if !was {
            if let Some(a) = c.l.abn.active_gen {
                set_gen_sii(c, a, |g| g.select_first());
            }
        }
        if let Some(id) = c.l.abn.active_id {
            c.publish("fws_active_item", Value::from(c.l.abn.selected));
            c.publish("fws_active_procedure", Value::from(id.to_string()));
        }
        c.publish("fws_show_from_line", Value::from(c.l.abn.show_from_line));
        c.s.ecam_ewd_show_failure_pending_indication = false;
    } else {
        c.l.ns.shown = false;
        c.l.abn.shown = false;
        c.s.ecam_ewd_show_failure_pending_indication = false;
    }
    let (ns, abn, ecl) = (c.l.ns.shown, c.l.abn.shown, c.ecl.shown);
    c.publish("fws_show_abn_non_sensed", Value::Bool(ns));
    c.publish("fws_show_abn_sensed", Value::Bool(abn));
    c.publish("fws_show_normal_checklists", Value::Bool(ecl));

    for n in [
        &mut c.s.to_config_input_buffer,
        &mut c.s.clear_button_input_buffer,
        &mut c.s.recall_button_input_buffer,
        &mut c.s.cl_input_buffer,
        &mut c.s.cl_check_input_buffer,
        &mut c.s.cl_up_input_buffer,
        &mut c.s.cl_down_input_buffer,
        &mut c.s.abn_proc_input_buffer,
        &mut c.s.a_thr_disc_input_buffer,
        &mut c.s.ap_disc_input_buffer,
    ] {
        n.write(false, true);
    }
    c.s.auto_pilot_instinctive_disc_count_since_last_fws_cycle = 0.;
    observe(c, v);
    c.subscribed_outputs();
}

fn observe(c: &mut Core, v: &dyn Vars) {
    let mut l = std::mem::take(&mut c.l);
    l.observe_activation(c, v, c.s.sim_time);
    c.l = l;
}

/// `FwsAbnormalSensed.update`.
fn abnormal_sensed_update(c: &mut Core) {
    c.l.abn.requested = !c.l.presented.is_empty();
    let approach = c.l.presented.is_empty()
        && c.s.flight_phase == 8.
        && c.s.adr_pressure_altitude.unwrap_or(0.) < 20_000.
        && !c.s.ecam_status_normal;
    if approach && (c.s.approach_auto_display_qnh_set_pulse_node.read() || c.s.approach_auto_display_slats_extended_pulse_node.read()) {
        c.write_var("L:A32NX_ECAM_SD_CURRENT_PAGE_INDEX", SD_STATUS);
    }
    if !c.l.abn.shown {
        return;
    }
    let Some(active) = c.l.abn.active_gen else { return };
    if c.s.cl_down_pulse_node.read() {
        set_gen_sii(c, active, |g| g.move_down(true));
    }
    if c.s.cl_up_pulse_node.read() {
        set_gen_sii(c, active, |g| g.move_up());
    }
    if c.s.cl_check_pulse_node.read() {
        check_selected(c);
    }
    let ids: Vec<u32> = c.l.presented.iter().map(|p| p.id).collect();
    for id in ids {
        let Some(changed) = c.l.abn_updated.get(&id).cloned() else { continue };
        if Some(id) != c.l.abn.active_id {
            continue;
        }
        let Some(g) = c.l.abn.gens.iter().find(|g| g.id == active) else { continue };
        let sii = g.sii;
        if sii >= 0 && changed.contains(&(sii as usize)) && at(&g.state.checked, sii as usize).truthy() {
            set_gen_sii(c, active, |g| g.move_down(true));
        }
    }
}

fn non_sensed_menu_size(id: u32) -> i32 {
    let top = fws_data::NON_SENSED.iter().filter(|p| p.category == fws_data::Category::None).count() as i32;
    let sub = fws_data::SUB_MENU.len() as u32;
    if id == 0 {
        top + sub as i32
    } else if id <= sub {
        let cat = fws_data::SUB_MENU[id as usize - 1];
        fws_data::NON_SENSED.iter().filter(|p| p.category == cat).count() as i32
    } else {
        0
    }
}

/// `FwsAbnormalNonSensed.update`.
fn non_sensed_update(c: &mut Core, v: &dyn Vars) {
    if c.s.abn_proc_pulse_node.read() {
        c.l.ns.checklist_id = 0;
        c.l.ns.selected = 0;
        c.l.ns.requested = !c.l.ns.requested;
        publish_non_sensed_id(c);
    }
    if !c.l.ns.shown {
        return;
    }
    let id = c.l.ns.checklist_id;
    if c.s.cl_down_pulse_node.read() && id <= 10 {
        c.l.ns.selected = (c.l.ns.selected + 1).min(non_sensed_menu_size(id) - 1);
    }
    if c.s.cl_up_pulse_node.read() && id <= 10 {
        c.l.ns.selected = (c.l.ns.selected - 1).max(0);
    }
    if c.s.cl_check_pulse_node.read() {
        let top = fws_data::NON_SENSED.iter().filter(|p| p.category == fws_data::Category::None).count() as i32;
        let sel = c.l.ns.selected;
        if id == 0 {
            if sel >= top && sel < non_sensed_menu_size(0) {
                c.l.ns.checklist_id = (sel - top + 1) as u32;
                c.l.ns.selected = 0;
            } else if sel < top {
                c.l.ns.checklist_id = usize::try_from(sel).ok().and_then(|i| fws_data::NON_SENSED.get(i)).map_or(0, |p| p.id);
                c.l.ns.selected = -1;
            }
        } else if id <= 10 {
            let cat = fws_data::SUB_MENU.get(id as usize - 1).copied();
            let start = fws_data::NON_SENSED.iter().position(|p| Some(p.category) == cat).unwrap_or(0) as i32;
            c.l.ns.checklist_id = usize::try_from(start + sel).ok().and_then(|i| fws_data::NON_SENSED.get(i)).map_or(0, |p| p.id);
            c.l.ns.selected = -1;
        } else {
            if !c.active_abnormal_non_sensed_keys.insert(id) {
                c.active_abnormal_non_sensed_keys.remove(&id);
            }
            c.l.ns.selected = 0;
            c.l.ns.requested = false;
            observe(c, v);
        }
        publish_non_sensed_id(c);
    }
}

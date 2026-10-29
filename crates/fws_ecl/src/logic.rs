use crate::types::{MemoRule, MemoSelect};

pub fn message_priority(raw: Option<&str>) -> u8 {
    let Some(raw) = raw else { return 10 };
    let head: String = raw.trim().chars().take(3).collect();
    match head.as_str() {
        "\u{1b}<6" => 0,
        "\u{1b}<2" => 1,
        "\u{1b}<4" => 2,
        "\u{1b}<3" => 3,
        _ => 10,
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ToMemoInputs {
    pub no_smoking_memo_and_seat_belt_switch_on: bool,
    pub spoilers_armed: bool,
    pub slat_flap_takeoff_selection: bool,
    pub auto_brake_rto: bool,
    pub to_config_normal: bool,
}

pub fn to_memo_indices(i: &ToMemoInputs) -> [usize; 6] {
    [
        0,
        if i.no_smoking_memo_and_seat_belt_switch_on { 2 } else { 1 },
        if i.spoilers_armed { 4 } else { 3 },
        if i.slat_flap_takeoff_selection { 6 } else { 5 },
        if i.auto_brake_rto { 8 } else { 7 },
        if i.to_config_normal { 10 } else { 9 },
    ]
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LdgMemoInputs {
    pub no_smoking_memo_and_seat_belt_switch_on: bool,
    pub all_gear_downlocked: bool,
    pub spoilers_armed: bool,
    pub landing_conf3: bool,
    pub flaps_handle_index: i32,
}

pub fn ldg_flaps_set(i: &LdgMemoInputs) -> bool {
    (!i.landing_conf3 && i.flaps_handle_index == 4) || (i.landing_conf3 && i.flaps_handle_index == 3)
}

pub fn ldg_memo_indices(i: &LdgMemoInputs) -> [usize; 5] {
    [
        0,
        if i.no_smoking_memo_and_seat_belt_switch_on { 2 } else { 1 },
        if i.all_gear_downlocked { 4 } else { 3 },
        if i.spoilers_armed { 6 } else { 5 },
        if ldg_flaps_set(i) { 8 } else { 7 },
    ]
}

fn align_minutes(seconds: f64) -> i64 {
    (seconds / 60.0).ceil() as i64
}

pub fn adirs_message_over_3_min(remaining_s: f64, engine_running: bool) -> usize {
    let row = match align_minutes(remaining_s) {
        m if m >= 7 => 0,
        6 => 2,
        5 => 4,
        4 => 6,
        _ => return 0,
    };
    row + usize::from(engine_running)
}

pub fn adirs_message_under_3_min(remaining_s: f64, engine_running: bool) -> usize {
    let row = match align_minutes(remaining_s) {
        3 => 0,
        2 => 2,
        1 => 4,
        _ => return 0,
    };
    row + usize::from(engine_running)
}

pub fn oxy_pax_sys_code(aircraft_on_ground: bool, excess_cabin_altitude: bool) -> &'static str {
    if !aircraft_on_ground && excess_cabin_altitude {
        "350000001"
    } else {
        "350000002"
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SelectInputs {
    pub named_flag: bool,
    pub engines_running: u8,
    pub adirs_remaining_s: f64,
    pub engine_starting: bool,
}

pub fn pure_selection(rule: &MemoRule, i: &SelectInputs) -> Option<Vec<usize>> {
    match rule.select {
        MemoSelect::Fixed0 => Some(vec![0]),
        MemoSelect::Ternary { if_true, if_false, .. } => {
            Some(vec![usize::from(if i.named_flag { if_true } else { if_false })])
        }
        MemoSelect::MoreThanOneEngineRunning { if_true, if_false } => {
            Some(vec![usize::from(if i.engines_running > 1 { if_true } else { if_false })])
        }
        MemoSelect::AdirsAlignOver3Min => {
            Some(vec![adirs_message_over_3_min(i.adirs_remaining_s, i.engine_starting)])
        }
        MemoSelect::AdirsAlignUnder3Min => {
            Some(vec![adirs_message_under_3_min(i.adirs_remaining_s, i.engine_starting)])
        }
        MemoSelect::ToMemo | MemoSelect::LdgMemo | MemoSelect::OxyPaxSys | MemoSelect::Custom => None,
    }
}

pub fn order_limitations(ids: &mut Vec<&'static str>) {
    if ids.contains(&"1") && ids.contains(&"2") {
        ids.retain(|id| *id != "2");
    }
    ids.sort_by_key(|id| crate::limitation_message(id).map_or(10, |m| message_priority(Some(m.raw))));
}

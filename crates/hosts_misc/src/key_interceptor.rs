use crate::pilot_seat::PilotSeat;
use crate::Command;

pub const MASTER_PUSH_HIGH_TIME_MS: u64 = 500;
pub const PRESET_READY_FOR_TAXI: u32 = 4;
pub const PRESET_ENGINES_OFF: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RmpState {
    OffFailed = 0,
    OffStandby = 1,
    On = 2,
    OnFailed = 3,
}

impl RmpState {
    pub fn from_value(v: f64) -> Option<Self> {
        match v as i64 {
            0 => Some(RmpState::OffFailed),
            1 => Some(RmpState::OffStandby),
            2 => Some(RmpState::On),
            3 => Some(RmpState::OnFailed),
            _ => None,
        }
    }

    fn is_on(self) -> bool {
        self == RmpState::On || self == RmpState::OnFailed
    }

    fn is_off(self) -> bool {
        self == RmpState::OffFailed || self == RmpState::OffStandby
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RmpEnv {
    pub states: [RmpState; 3],
    pub fo_sync_efis_enabled: bool,
    pub pilot_seat: PilotSeat,
    pub vhf_tx: [[bool; 3]; 3],
    pub vhf_vol: [[f64; 3]; 3],
    pub vhf_rx_switch: [[bool; 3]; 3],
}

impl Default for RmpEnv {
    fn default() -> Self {
        Self {
            states: [RmpState::On, RmpState::On, RmpState::OffStandby],
            fo_sync_efis_enabled: false,
            pilot_seat: PilotSeat::Left,
            vhf_tx: [[false; 3]; 3],
            vhf_vol: [[0.; 3]; 3],
            vhf_rx_switch: [[false; 3]; 3],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresetDialog {
    EngineAutoStart { preset_id: u32 },
    EngineAutoShutdown { preset_id: u32 },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Notice {
    PresetAlreadyLoading { preset_id: u32 },
}

pub struct KeyInterceptor {
    dialog_visible: bool,
}

impl KeyInterceptor {
    pub fn new() -> Self {
        Self { dialog_visible: false }
    }

    pub fn dialog_visible(&self) -> bool {
        self.dialog_visible
    }

    pub fn dialog_closed(&mut self) {
        self.dialog_visible = false;
    }

    pub fn load_preset(&mut self, preset_id: u32) -> Command {
        self.dialog_visible = false;
        Command::lvar("A32NX_AIRCRAFT_PRESET_LOAD", preset_id as f64)
    }

    pub fn engine_auto_start(&mut self, preset_load_in_progress: u32) -> (Option<PresetDialog>, Option<Notice>) {
        if self.dialog_visible {
            return (None, None);
        }
        if preset_load_in_progress > 0 {
            return (None, Some(Notice::PresetAlreadyLoading { preset_id: preset_load_in_progress }));
        }
        self.dialog_visible = true;
        (Some(PresetDialog::EngineAutoStart { preset_id: PRESET_READY_FOR_TAXI }), None)
    }

    pub fn engine_auto_shutdown(
        &mut self,
        preset_load_in_progress: u32,
        engine1_n1: f64,
        engine2_n1: f64,
    ) -> (Option<PresetDialog>, Option<Notice>) {
        if preset_load_in_progress > 0 {
            return (None, Some(Notice::PresetAlreadyLoading { preset_id: preset_load_in_progress }));
        }
        if !self.dialog_visible && (engine1_n1 > 0.1 || engine2_n1 > 0.1) {
            self.dialog_visible = true;
            return (Some(PresetDialog::EngineAutoShutdown { preset_id: PRESET_ENGINES_OFF }), None);
        }
        (None, None)
    }

    pub fn master_caution_acknowledge(&self, seat: PilotSeat) -> Vec<Command> {
        pilot_pulse("PUSH_AUTOPILOT_MASTERCAUT", seat)
    }

    pub fn master_warning_acknowledge(&self, seat: PilotSeat) -> Vec<Command> {
        pilot_pulse("PUSH_AUTOPILOT_MASTERAWARN", seat)
    }
}

impl Default for KeyInterceptor {
    fn default() -> Self {
        Self::new()
    }
}

fn pilot_pulse(name: &str, seat: PilotSeat) -> Vec<Command> {
    let var = format!("{}_{}", name, if seat == PilotSeat::Right { 'R' } else { 'L' });
    vec![
        Command::SetLvar { name: var.clone(), value: 1. },
        Command::SetLvarAfter { name: var, value: 0., delay_ms: MASTER_PUSH_HIGH_TIME_MS },
    ]
}

pub fn should_control_rmp(env: &RmpEnv, rmp: usize) -> bool {
    let [s1, s2, s3] = env.states;
    match rmp {
        1 => env.fo_sync_efis_enabled || s3.is_off() || (env.pilot_seat == PilotSeat::Left && s1.is_on()),
        2 => env.fo_sync_efis_enabled || s3.is_off() || (env.pilot_seat == PilotSeat::Right && s2.is_on()),
        3 => {
            env.fo_sync_efis_enabled
                || (env.pilot_seat == PilotSeat::Left && s1.is_off())
                || (env.pilot_seat == PilotSeat::Right && s2.is_off())
        }
        _ => false,
    }
}

fn controlled(env: &RmpEnv) -> impl Iterator<Item = usize> + '_ {
    (1..=3).filter(move |r| should_control_rmp(env, *r))
}

pub fn com_rx_select(env: &RmpEnv, index: usize, value0: f64) -> Vec<Command> {
    let rx_on = value0 == 1.;
    controlled(env)
        .map(|rmp| Command::flag(format!("A380X_RMP_{rmp}_VHF_VOL_RX_SWITCH_{index}"), rx_on))
        .collect()
}

pub fn com_tx_select(env: &RmpEnv, value0: Option<f64>) -> Vec<Command> {
    let vhf_index = (value0.unwrap_or(0.) + 1.) as usize;
    let mut out = Vec::new();
    for rmp in controlled(env) {
        let tx_active = vhf_index
            .checked_sub(1)
            .and_then(|v| env.vhf_tx[rmp - 1].get(v).copied())
            .unwrap_or(false);
        if !tx_active {
            out.push(Command::HEvent { name: format!("RMP_{rmp}_VHF_CALL_{vhf_index}_PRESSED") });
            out.push(Command::HEventNextFrame { name: format!("RMP_{rmp}_VHF_CALL_{vhf_index}_RELEASED") });
        }
    }
    out
}

pub fn com_volume_set(env: &RmpEnv, index: usize, value0: Option<f64>) -> Vec<Command> {
    let volume = value0.unwrap_or(0.).clamp(0., 100.);
    controlled(env).map(|rmp| Command::lvar(format!("A380X_RMP_{rmp}_VHF_VOL_{index}"), volume)).collect()
}

pub fn com_volume_inc(env: &RmpEnv, index: usize, sign: i32) -> Vec<Command> {
    let increment = 2. * sign as f64;
    controlled(env)
        .map(|rmp| {
            let current = env.vhf_vol[rmp - 1][index - 1];
            Command::lvar(format!("A380X_RMP_{rmp}_VHF_VOL_{index}"), (current + increment).clamp(0., 100.))
        })
        .collect()
}

pub fn com_receive_all(env: &RmpEnv, toggle: bool, value0: Option<f64>) -> Vec<Command> {
    let mut out = Vec::new();
    for rmp in controlled(env) {
        for index in 1..=3 {
            let on = if toggle { !env.vhf_rx_switch[rmp - 1][index - 1] } else { value0.map_or(false, |v| v > 0.) };
            out.push(Command::flag(format!("A380X_RMP_{rmp}_VHF_VOL_RX_SWITCH_{index}"), on));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(states: [RmpState; 3], seat: PilotSeat, sync: bool) -> RmpEnv {
        RmpEnv { states, pilot_seat: seat, fo_sync_efis_enabled: sync, ..Default::default() }
    }

    use RmpState::*;

    #[test]
    fn left_seat_controls_rmp1_when_it_is_on_and_rmp3_when_rmp1_is_off() {
        let e = env([On, On, OffStandby], PilotSeat::Left, false);
        assert!(should_control_rmp(&e, 1));
        assert!(should_control_rmp(&e, 2), "rmp3 off hands the key events to both");
        assert!(!should_control_rmp(&e, 3));
        let e = env([OffFailed, On, On], PilotSeat::Left, false);
        assert!(!should_control_rmp(&e, 1));
        assert!(!should_control_rmp(&e, 2));
        assert!(should_control_rmp(&e, 3));
    }

    #[test]
    fn right_seat_mirrors_it() {
        let e = env([On, OnFailed, On], PilotSeat::Right, false);
        assert!(!should_control_rmp(&e, 1));
        assert!(should_control_rmp(&e, 2));
        assert!(!should_control_rmp(&e, 3));
        let e = env([On, OffStandby, On], PilotSeat::Right, false);
        assert!(should_control_rmp(&e, 3));
    }

    #[test]
    fn sync_enabled_controls_everything() {
        let e = env([OffFailed, OffFailed, On], PilotSeat::Left, true);
        assert!((1..=3).all(|r| should_control_rmp(&e, r)));
    }

    #[test]
    fn volume_set_clamps_and_writes_each_controlled_panel() {
        let e = env([On, On, On], PilotSeat::Left, false);
        assert_eq!(com_volume_set(&e, 2, Some(150.)), vec![Command::lvar("A380X_RMP_1_VHF_VOL_2", 100.)]);
        assert_eq!(com_volume_set(&e, 2, None), vec![Command::lvar("A380X_RMP_1_VHF_VOL_2", 0.)]);
        let all = env([On, On, On], PilotSeat::Left, true);
        assert_eq!(com_volume_set(&all, 1, Some(40.)).len(), 3);
    }

    #[test]
    fn volume_inc_steps_by_two_from_the_current_value() {
        let mut e = env([On, On, On], PilotSeat::Left, false);
        e.vhf_vol[0][0] = 99.;
        assert_eq!(com_volume_inc(&e, 1, 1), vec![Command::lvar("A380X_RMP_1_VHF_VOL_1", 100.)]);
        e.vhf_vol[0][0] = 1.;
        assert_eq!(com_volume_inc(&e, 1, -1), vec![Command::lvar("A380X_RMP_1_VHF_VOL_1", 0.)]);
        e.vhf_vol[0][0] = 50.;
        assert_eq!(com_volume_inc(&e, 1, 1), vec![Command::lvar("A380X_RMP_1_VHF_VOL_1", 52.)]);
    }

    #[test]
    fn rx_select_sets_the_switch_on_for_value_one_only() {
        let e = env([On, On, On], PilotSeat::Left, false);
        assert_eq!(com_rx_select(&e, 3, 1.), vec![Command::flag("A380X_RMP_1_VHF_VOL_RX_SWITCH_3", true)]);
        assert_eq!(com_rx_select(&e, 3, 0.), vec![Command::flag("A380X_RMP_1_VHF_VOL_RX_SWITCH_3", false)]);
        assert_eq!(com_rx_select(&e, 3, 2.), vec![Command::flag("A380X_RMP_1_VHF_VOL_RX_SWITCH_3", false)]);
    }

    #[test]
    fn tx_select_presses_then_releases_next_frame_unless_already_transmitting() {
        let mut e = env([On, On, On], PilotSeat::Left, false);
        assert_eq!(
            com_tx_select(&e, Some(1.)),
            vec![
                Command::HEvent { name: "RMP_1_VHF_CALL_2_PRESSED".to_string() },
                Command::HEventNextFrame { name: "RMP_1_VHF_CALL_2_RELEASED".to_string() },
            ]
        );
        e.vhf_tx[0][1] = true;
        assert!(com_tx_select(&e, Some(1.)).is_empty());
    }

    #[test]
    fn legacy_transmit_select_keys_carry_no_value0_and_select_vhf1() {
        let e = env([On, On, On], PilotSeat::Left, false);
        let cmds = com_tx_select(&e, None);
        assert_eq!(cmds[0], Command::HEvent { name: "RMP_1_VHF_CALL_1_PRESSED".to_string() });
    }

    #[test]
    fn receive_all_sets_or_toggles_all_three_switches() {
        let mut e = env([On, On, On], PilotSeat::Left, false);
        e.vhf_rx_switch[0] = [true, false, true];
        let toggled = com_receive_all(&e, true, None);
        assert_eq!(
            toggled,
            vec![
                Command::flag("A380X_RMP_1_VHF_VOL_RX_SWITCH_1", false),
                Command::flag("A380X_RMP_1_VHF_VOL_RX_SWITCH_2", true),
                Command::flag("A380X_RMP_1_VHF_VOL_RX_SWITCH_3", false),
            ]
        );
        let set = com_receive_all(&e, false, Some(1.));
        assert!(set.iter().all(|c| matches!(c, Command::SetLvar { value, .. } if *value == 1.)));
        let none = com_receive_all(&e, false, None);
        assert!(none.iter().all(|c| matches!(c, Command::SetLvar { value, .. } if *value == 0.)));
    }

    #[test]
    fn engine_auto_start_asks_once_and_defers_to_a_running_preset() {
        let mut k = KeyInterceptor::new();
        assert_eq!(k.engine_auto_start(3), (None, Some(Notice::PresetAlreadyLoading { preset_id: 3 })));
        assert!(!k.dialog_visible());
        let (d, n) = k.engine_auto_start(0);
        assert_eq!(d, Some(PresetDialog::EngineAutoStart { preset_id: 4 }));
        assert!(n.is_none() && k.dialog_visible());
        assert_eq!(k.engine_auto_start(0), (None, None));
        assert_eq!(k.load_preset(4), Command::lvar("A32NX_AIRCRAFT_PRESET_LOAD", 4.));
        assert!(!k.dialog_visible());
    }

    #[test]
    fn engine_auto_shutdown_needs_a_running_engine() {
        let mut k = KeyInterceptor::new();
        assert_eq!(k.engine_auto_shutdown(0, 0.05, 0.1), (None, None));
        assert_eq!(k.engine_auto_shutdown(0, 0.05, 0.2).0, Some(PresetDialog::EngineAutoShutdown { preset_id: 2 }));
        assert_eq!(k.engine_auto_shutdown(0, 20., 20.), (None, None));
        k.dialog_closed();
        assert_eq!(k.engine_auto_shutdown(1, 20., 20.).1, Some(Notice::PresetAlreadyLoading { preset_id: 1 }));
    }

    #[test]
    fn master_acknowledge_pulses_the_pilots_own_variable_for_500_ms() {
        let k = KeyInterceptor::new();
        assert_eq!(
            k.master_warning_acknowledge(PilotSeat::Right),
            vec![
                Command::lvar("PUSH_AUTOPILOT_MASTERAWARN_R", 1.),
                Command::SetLvarAfter { name: "PUSH_AUTOPILOT_MASTERAWARN_R".to_string(), value: 0., delay_ms: 500 },
            ]
        );
        assert_eq!(k.master_caution_acknowledge(PilotSeat::Left)[0], Command::lvar("PUSH_AUTOPILOT_MASTERCAUT_L", 1.));
    }

    #[test]
    fn rmp_state_from_value() {
        assert_eq!(RmpState::from_value(2.), Some(RmpState::On));
        assert_eq!(RmpState::from_value(9.), None);
    }
}

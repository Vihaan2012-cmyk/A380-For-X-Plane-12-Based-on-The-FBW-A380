use crate::radio::*;
use crate::vars::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RmpState {
    OffFailed = 0,
    OffStandby = 1,
    On = 2,
    OnFailed = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Vhf,
    Transponder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reception {
    Off,
    Receive,
    TransmitOnly,
}

const CLR_HELD_MS: f64 = 1000.;
const IDENT_MS: f64 = 5000.;
const AUTO_VALIDATE_MS: f64 = 1000.;
const RX_DEBOUNCE_MS: f64 = 380.;
const TX_ONLY_FLASH_MS: f64 = 2000.;
const NO_ENTRY_FREQ: &str = "------";

#[derive(Default, Clone)]
struct Timer {
    due: Option<f64>,
}

impl Timer {
    fn schedule(&mut self, now: f64, ms: f64) {
        self.due = Some(now + ms);
    }
    fn clear(&mut self) {
        self.due = None;
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    entered: String,
    offset: usize,
    displayed: String,
    frequency: u32,
}

impl Entry {
    fn empty() -> Self {
        Entry { entered: String::new(), offset: 0, displayed: "---.---".to_string(), frequency: 0 }
    }
}

#[derive(Clone, Copy, Default)]
struct BusVhf {
    active: Option<u32>,
    standby: Option<u32>,
    active_mode: Option<f64>,
    standby_mode: Option<f64>,
}

struct Mgr {
    index: usize,
    tuning_var: String,
    relay_var: Option<String>,
    relay: f64,
    relay_active: bool,
    subs_active: bool,
    active_freq: Option<f64>,
    standby_freq: Option<f64>,
    active_mode: f64,
    standby_mode: f64,
}

struct Row {
    index: usize,
    entry: Entry,
    invalid: bool,
    in_progress: bool,
    digits: [char; 6],
    pilot: [bool; 6],
    radio_raw: f64,
    rx: bool,
    tx: bool,
    reception: Reception,
    tx_flashing: bool,
    flash_timer: Timer,
}

struct Xpdr {
    entered: Option<String>,
    invalid: bool,
    auto_timer: Timer,
    ident_timer: Timer,
    ident_active: bool,
    ident_msg_active: bool,
    squawk_sub_active: bool,
    active_code: Option<f64>,
    is_auto: bool,
}

struct KeyDef {
    key: &'static str,
    var: String,
    state: bool,
}

struct RxDef {
    switch_var: String,
    rx_var: String,
    last: Option<f64>,
    timer: Timer,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RowView {
    pub active_text: String,
    pub active_amber: bool,
    pub standby_digits: String,
    pub standby_pilot_entered: [bool; 6],
    pub standby_amber: bool,
    pub standby_mode_title: &'static str,
    pub standby_entry_in_progress: bool,
    pub loudspeaker_hidden: bool,
    pub no_reception_cross: bool,
    pub transmitting: bool,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct XpdrView {
    pub auto: bool,
    pub code: String,
    pub entry_in_progress: bool,
    pub entry_invalid: bool,
    pub ident: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub screen_on: bool,
    pub state: RmpState,
    pub page: Page,
    pub rows: [RowView; 3],
    pub xpdr: XpdrView,
    pub message: String,
}

pub struct Rmp {
    idx: usize,
    now: f64,
    pub fo_sync_enabled: bool,
    state: RmpState,
    keypad_state: RmpState,
    rmp3_bus_a: bool,
    bus_a_out: bool,
    screen_brightness: f64,
    pot: f64,
    mgrs: [Mgr; 3],
    bus: [BusVhf; 3],
    cached_set_standby: [Option<u32>; 3],
    rows: [Row; 3],
    xpdr: Xpdr,
    page: Page,
    selected_row: usize,
    vhf_clr_timer: Timer,
    xpdr_clr_timer: Timer,
    msg_displayed: String,
    msg_background: bool,
    squawk_enabled: bool,
    squawk_code: f64,
    squawk_auto: bool,
    squawk_msg: String,
    key_defs: Vec<KeyDef>,
    rx_defs: Vec<RxDef>,
    tx_last: Vec<Option<f64>>,
    flash_last: Option<f64>,
    flash_1hz: bool,
    xpdr_code_last: Option<f64>,
    xpdr_auto_last: Option<bool>,
    radio_last: [Option<f64>; 3],
}

const TX_MONITOR: [(&str, &str, usize); 7] =
    [("VHF", "VHF", 1), ("VHF", "VHF", 2), ("VHF", "VHF", 3), ("HF", "HF", 1), ("HF", "HF", 2), ("TEL", "TEL", 1), ("TEL", "TEL", 2)];

fn failure_key(idx: usize) -> &'static str {
    match idx {
        1 => "RadioManagementPanel1",
        2 => "RadioManagementPanel2",
        _ => "RadioManagementPanel3",
    }
}

impl Rmp {
    pub fn new(idx: usize) -> Self {
        assert!((1..=3).contains(&idx));
        let mgr = |i: usize| {
            let primary = match i {
                1 => idx == 1,
                2 => idx == 2,
                _ => idx == 1,
            };
            let mode = if i == 3 { 1.0 } else { 0.0 };
            Mgr {
                index: i,
                tuning_var: format!("L:FBW_RMP{idx}_{}_VHF{i}_FREQUENCY", if primary { "PRIMARY" } else { "BACKUP" }),
                relay_var: primary.then(|| format!("L:FBW_RMP3_BACKUP_VHF{i}_FREQUENCY")),
                relay: 0.,
                relay_active: false,
                subs_active: false,
                active_freq: None,
                standby_freq: None,
                active_mode: mode,
                standby_mode: mode,
            }
        };
        let row = |i: usize| Row {
            index: i,
            entry: Entry::empty(),
            invalid: false,
            in_progress: false,
            digits: ['-'; 6],
            pilot: [false; 6],
            radio_raw: 0.,
            rx: false,
            tx: false,
            reception: Reception::Off,
            tx_flashing: false,
            flash_timer: Timer::default(),
        };
        let key_defs = [
            ("VHF_CALL_1", "VHF_TX_1"),
            ("VHF_CALL_2", "VHF_TX_2"),
            ("VHF_CALL_3", "VHF_TX_3"),
            ("HF_CALL_1", "HF_TX_1"),
            ("HF_CALL_2", "HF_TX_2"),
            ("TEL_CALL_1", "TEL_TX_1"),
            ("TEL_CALL_2", "TEL_TX_2"),
            ("MECH_CALL", "INT_TX"),
            ("CAB_CALL", "CAB_TX"),
            ("PA_CALL", "PA_TX"),
            ("VOICE", "NAV_FILTER"),
        ]
        .iter()
        .map(|(k, v)| KeyDef { key: k, var: format!("L:A380X_RMP_{idx}_{v}"), state: false })
        .collect();
        let rx_pairs = [
            ("VHF_VOL_RX_SWITCH_1", "VHF_VOL_RX_1"),
            ("VHF_VOL_RX_SWITCH_2", "VHF_VOL_RX_2"),
            ("VHF_VOL_RX_SWITCH_3", "VHF_VOL_RX_3"),
            ("HF_VOL_RX_SWITCH_1", "HF_VOL_RX_1"),
            ("HF_VOL_RX_SWITCH_2", "HF_VOL_RX_2"),
            ("TEL_VOL_RX_SWITCH_1", "TEL_VOL_RX_1"),
            ("TEL_VOL_RX_SWITCH_2", "TEL_VOL_RX_2"),
            ("INT_VOL_RX_SWITCH", "INT_VOL_RX"),
            ("CAB_VOL_RX_SWITCH", "CAB_VOL_RX"),
            ("PA_VOL_RX_SWITCH", "PA_VOL_RX"),
            ("NAV_VOL_RX_SWITCH", "NAV_VOL_RX"),
        ];
        let rx_defs = rx_pairs
            .iter()
            .map(|(s, r)| RxDef { switch_var: format!("L:A380X_RMP_{idx}_{s}"), rx_var: format!("L:A380X_RMP_{idx}_{r}"), last: None, timer: Timer::default() })
            .collect();
        Rmp {
            idx,
            now: 0.,
            fo_sync_enabled: false,
            state: RmpState::OffStandby,
            keypad_state: RmpState::OffStandby,
            rmp3_bus_a: false,
            bus_a_out: false,
            screen_brightness: 0.,
            pot: 79. + idx as f64,
            mgrs: [mgr(1), mgr(2), mgr(3)],
            bus: [BusVhf::default(); 3],
            cached_set_standby: [None; 3],
            rows: [row(1), row(2), row(3)],
            xpdr: Xpdr {
                entered: None,
                invalid: false,
                auto_timer: Timer::default(),
                ident_timer: Timer::default(),
                ident_active: false,
                ident_msg_active: false,
                squawk_sub_active: false,
                active_code: None,
                is_auto: false,
            },
            page: Page::Vhf,
            selected_row: 0,
            vhf_clr_timer: Timer::default(),
            xpdr_clr_timer: Timer::default(),
            msg_displayed: String::new(),
            msg_background: true,
            squawk_enabled: false,
            squawk_code: 0.,
            squawk_auto: false,
            squawk_msg: String::new(),
            key_defs,
            rx_defs,
            tx_last: vec![None; 7],
            flash_last: None,
            flash_1hz: false,
            xpdr_code_last: None,
            xpdr_auto_last: None,
            radio_last: [None; 3],
        }
    }

    pub fn index(&self) -> usize {
        self.idx
    }

    pub fn state(&self) -> RmpState {
        self.state
    }

    fn dc_power_var(&self) -> &'static str {
        if self.idx == 3 {
            "L:A32NX_ELEC_DC_1_BUS_IS_POWERED"
        } else {
            "L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED"
        }
    }

    fn select_bus_a_vars(&self) -> Vec<&'static str> {
        match self.idx {
            1 => vec!["L:FBW_VHF1_BUS_A", "L:FBW_VHF3_BUS_A", "L:FBW_HF1_BUS_A"],
            2 => vec!["L:FBW_VHF2_BUS_A", "L:FBW_HF2_BUS_A"],
            _ => vec!["L:FBW_RMP3_BUS_A"],
        }
    }

    pub fn init(&mut self, vars: &mut Vars) {
        self.publish_state(self.state, vars);
        let (pot, b) = (self.pot, self.screen_brightness);
        vars.key("LIGHT_POTENTIOMETER_SET", &[pot, b]);
        self.activate_page(Page::Vhf, vars);
    }

    pub fn flight_start(&mut self, vars: &mut Vars) {
        if self.idx != 1 {
            return;
        }
        for i in 1..=3usize {
            if i == 3 {
                vars.set(&format!("L:FBW_RMP_FREQUENCY_ACTIVE_{i}"), DATA_FREQUENCY as f64);
                vars.set(&format!("L:FBW_RMP_MODE_ACTIVE_{i}"), FrequencyMode::Data as i32 as f64);
            } else {
                let f = vars.get(&format!("COM ACTIVE FREQUENCY:{i}"));
                vars.set(&format!("L:FBW_RMP_FREQUENCY_ACTIVE_{i}"), f);
            }
            let s = vars.get(&format!("COM STANDBY FREQUENCY:{i}"));
            vars.set(&format!("L:FBW_RMP_FREQUENCY_STANDBY_{i}"), s);
        }
    }

    fn publish_state(&mut self, state: RmpState, vars: &mut Vars) {
        self.keypad_state = state;
        for i in 0..3 {
            self.mgr_on_rmp_state(i, vars);
        }
        vars.set(&format!("L:A380X_RMP_{}_RED_LED", self.idx), matches!(state, RmpState::OffFailed | RmpState::OnFailed) as i32 as f64);
        vars.set(&format!("L:A380X_RMP_{}_GREEN_LED", self.idx), (state == RmpState::OffStandby) as i32 as f64);
        vars.set(&format!("L:A380X_RMP_{}_STATE", self.idx), state as i32 as f64);
    }

    fn set_state(&mut self, new: RmpState, vars: &mut Vars) {
        if new == self.state {
            return;
        }
        self.state = new;
        self.publish_state(new, vars);
        self.update_bus_a(vars);
    }

    fn update_bus_a(&mut self, vars: &mut Vars) {
        let out = match self.state {
            RmpState::On | RmpState::OnFailed => true,
            _ => self.rmp3_bus_a,
        };
        if out != self.bus_a_out {
            self.bus_a_out = out;
            for v in self.select_bus_a_vars() {
                vars.set(v, out as i32 as f64);
            }
        }
    }

    fn update_state_controller(&mut self, vars: &mut Vars) {
        let power_on = vars.get(self.dc_power_var()) > 0.;
        let brightness = vars.get(&format!("L:A380X_RMP_{}_BRIGHTNESS_KNOB", self.idx));
        let bright_on = brightness > 0.;
        let failed = vars.failed(failure_key(self.idx));
        let new = if power_on {
            if failed {
                if bright_on { RmpState::OnFailed } else { RmpState::OffFailed }
            } else if bright_on {
                RmpState::On
            } else {
                RmpState::OffStandby
            }
        } else if bright_on {
            RmpState::OnFailed
        } else {
            RmpState::OffFailed
        };
        self.set_state(new, vars);
        if self.idx != 3 {
            let v = vars.get("L:FBW_RMP3_BUS_A") > 0.;
            if v != self.rmp3_bus_a {
                self.rmp3_bus_a = v;
                self.update_bus_a(vars);
            }
        }
        let sb = if self.state == RmpState::On { brightness.max(5.) } else { 0. };
        if sb != self.screen_brightness {
            self.screen_brightness = sb;
            let pot = self.pot;
            vars.key("LIGHT_POTENTIOMETER_SET", &[pot, sb]);
        }
    }

    fn freq_var(kind: &str, which: &str, i: usize) -> String {
        format!("L:FBW_RMP_{kind}_{which}_{i}")
    }

    fn mgr_on_rmp_state(&mut self, i: usize, vars: &mut Vars) {
        let on = self.state == RmpState::On;
        if on {
            self.mgrs[i].relay_active = false;
        }
        self.mgrs[i].subs_active = on;
        if on {
            self.mgr_active_freq_changed(i, vars);
            self.mgr_standby_freq_changed(i);
            self.bus[i].active_mode = Some(self.mgrs[i].active_mode);
            self.bus[i].standby_mode = Some(self.mgrs[i].standby_mode);
            if let Some(f) = self.cached_set_standby[i] {
                self.mgr_set_standby_frequency(i, f, vars);
            }
        } else {
            vars.set(&self.mgrs[i].tuning_var.clone(), arinc_raw(0, SSM_FAILURE_WARNING));
            let m = &mut self.mgrs[i];
            if m.relay_var.is_some() {
                if m.relay != 0. && m.relay_active {
                    let (name, r) = (m.tuning_var.clone(), 0.);
                    m.relay = r;
                    vars.set(&name, r);
                }
                m.relay = 0.;
                m.relay_active = true;
            }
        }
    }

    fn mgr_active_freq_changed(&mut self, i: usize, vars: &mut Vars) {
        let f = self.mgrs[i].active_freq.unwrap_or(0.);
        if f < 1. {
            vars.set(&self.mgrs[i].tuning_var.clone(), arinc_raw(0, SSM_NO_COMPUTED_DATA));
        } else {
            vars.set(&self.mgrs[i].tuning_var.clone(), arinc_raw(pack_bcd32_vhf_to_arinc(f as u32), SSM_NORMAL_OPERATION));
        }
        self.bus[i].active = if f > 0. { Some(f as u32) } else { None };
    }

    fn mgr_standby_freq_changed(&mut self, i: usize) {
        let f = self.mgrs[i].standby_freq.unwrap_or(0.);
        let new = if f > 0. { Some(f as u32) } else { None };
        self.bus[i].standby = new;
        self.row_refresh(i);
    }

    fn mgr_update(&mut self, i: usize, vars: &mut Vars) {
        if self.state != RmpState::On {
            if let Some(rv) = self.mgrs[i].relay_var.clone() {
                let v = vars.get(&rv);
                let m = &mut self.mgrs[i];
                if v != m.relay {
                    m.relay = v;
                    if m.relay_active {
                        let name = m.tuning_var.clone();
                        vars.set(&name, v);
                    }
                }
            }
        }
        let n = i + 1;
        let af = vars.get(&Self::freq_var("FREQUENCY", "ACTIVE", n));
        if self.mgrs[i].active_freq != Some(af) {
            self.mgrs[i].active_freq = Some(af);
            if self.mgrs[i].subs_active {
                self.mgr_active_freq_changed(i, vars);
            }
        }
        let am = vars.get(&Self::freq_var("MODE", "ACTIVE", n));
        if self.mgrs[i].active_mode != am {
            self.mgrs[i].active_mode = am;
            if self.mgrs[i].subs_active {
                self.bus[i].active_mode = Some(am);
            }
        }
        let sf = vars.get(&Self::freq_var("FREQUENCY", "STANDBY", n));
        if self.mgrs[i].standby_freq != Some(sf) {
            self.mgrs[i].standby_freq = Some(sf);
            if self.mgrs[i].subs_active {
                self.mgr_standby_freq_changed(i);
            }
        }
        let sm = vars.get(&Self::freq_var("MODE", "STANDBY", n));
        if self.mgrs[i].standby_mode != sm {
            self.mgrs[i].standby_mode = sm;
            if self.mgrs[i].subs_active {
                self.bus[i].standby_mode = Some(sm);
            }
        }
    }

    pub fn set_active_frequency(&mut self, vhf: usize, freq: u32, vars: &mut Vars) {
        if self.state == RmpState::On {
            vars.set(&Self::freq_var("FREQUENCY", "ACTIVE", vhf), freq as f64);
        }
    }

    pub fn set_active_mode(&mut self, vhf: usize, mode: FrequencyMode, vars: &mut Vars) {
        if self.state == RmpState::On {
            vars.set(&Self::freq_var("MODE", "ACTIVE", vhf), mode as i32 as f64);
            match mode {
                FrequencyMode::Data => vars.set(&Self::freq_var("FREQUENCY", "ACTIVE", vhf), DATA_FREQUENCY as f64),
                FrequencyMode::Emergency => vars.set(&Self::freq_var("FREQUENCY", "ACTIVE", vhf), EMERGENCY_FREQUENCY as f64),
                _ => {}
            }
        }
    }

    fn mgr_set_standby_frequency(&mut self, i: usize, freq: u32, vars: &mut Vars) {
        if self.state == RmpState::On {
            vars.set(&Self::freq_var("FREQUENCY", "STANDBY", i + 1), freq as f64);
        }
    }

    fn mgr_set_standby_mode(&mut self, i: usize, mode: FrequencyMode, vars: &mut Vars) {
        if self.state == RmpState::On {
            vars.set(&Self::freq_var("MODE", "STANDBY", i + 1), mode as i32 as f64);
            match mode {
                FrequencyMode::Data => vars.set(&Self::freq_var("FREQUENCY", "STANDBY", i + 1), DATA_FREQUENCY as f64),
                FrequencyMode::Emergency => vars.set(&Self::freq_var("FREQUENCY", "STANDBY", i + 1), EMERGENCY_FREQUENCY as f64),
                _ => {}
            }
        }
    }

    fn mgr_swap(&mut self, i: usize, vars: &mut Vars) {
        if self.state == RmpState::On {
            swap_vhf(i + 1, vars);
        }
    }

    fn message_set(&mut self, msg: &str) {
        self.msg_background = false;
        self.msg_displayed = msg.to_string();
    }

    fn message_clear_if(&mut self, msg: &str) {
        if self.msg_displayed == msg {
            self.message_clear();
        }
    }

    fn message_clear(&mut self) {
        self.msg_background = true;
        self.msg_displayed = self.squawk_msg.clone();
    }

    fn recompute_squawk(&mut self) {
        let msg = if self.squawk_enabled { format!("SQUAWK : {:04x}{}", self.squawk_code as u32, if self.squawk_auto { "" } else { " - STBY" }) } else { String::new() };
        if msg != self.squawk_msg {
            self.squawk_msg = msg.clone();
            if self.msg_background {
                self.msg_displayed = msg;
            }
        }
    }

    fn set_squawk_enabled(&mut self, v: bool) {
        if v != self.squawk_enabled {
            self.squawk_enabled = v;
            self.recompute_squawk();
        }
    }

    fn activate_page(&mut self, page: Page, _vars: &mut Vars) {
        match page {
            Page::Vhf => self.set_squawk_enabled(true),
            Page::Transponder => {
                self.xpdr.ident_msg_active = true;
                self.xpdr.squawk_sub_active = true;
                let auto = self.xpdr.is_auto;
                self.set_squawk_enabled(!auto);
            }
        }
    }

    fn deactivate_page(&mut self, page: Page) {
        if page == Page::Transponder {
            self.xpdr_set_entered(None);
            self.xpdr.ident_msg_active = false;
            self.xpdr.squawk_sub_active = false;
        }
    }

    fn navigate(&mut self, page: Page, vars: &mut Vars) {
        if page == self.page {
            return;
        }
        let prev = self.page;
        self.page = page;
        self.deactivate_page(prev);
        self.activate_page(page, vars);
    }

    pub fn hevent(&mut self, name: &str, vars: &mut Vars) {
        if self.keypad_state != RmpState::On {
            return;
        }
        let prefix = format!("RMP_{}_", self.idx);
        let Some(rest) = name.strip_prefix(&prefix) else { return };
        let (key, pressed) = if let Some(k) = rest.strip_suffix("_PRESSED") {
            (k, true)
        } else if let Some(k) = rest.strip_suffix("_RELEASED") {
            (k, false)
        } else {
            return;
        };
        const SYSTEM: [&str; 19] = [
            "VHF", "HF", "TEL", "SQWK", "MENU", "NAV", "MSG_CLR", "RST", "VHF_CALL_1", "VHF_CALL_2", "VHF_CALL_3", "HF_CALL_1", "HF_CALL_2", "TEL_CALL_1", "TEL_CALL_2", "MECH_CALL", "CAB_CALL", "PA_CALL", "VOICE",
        ];
        const PAGE: [&str; 20] = [
            "ADK_1", "ADK_2", "ADK_3", "LSK_1", "LSK_2", "LSK_3", "UP", "DOWN", "DIGIT_0", "DIGIT_1", "DIGIT_2", "DIGIT_3", "DIGIT_4", "DIGIT_5", "DIGIT_6", "DIGIT_7", "DIGIT_8", "DIGIT_9", "DIGIT_DOT", "DIGIT_CLR",
        ];
        if SYSTEM.contains(&key) {
            if pressed {
                self.system_key(key, vars);
            }
        } else if PAGE.contains(&key) {
            if pressed {
                self.page_key_pressed(key, vars);
            } else {
                self.page_key_released(key);
            }
        }
    }

    fn system_key(&mut self, key: &str, vars: &mut Vars) {
        if key == "MSG_CLR" {
            self.message_clear();
        }
        self.audio_key_pressed(key, vars);
        match key {
            "VHF" => self.navigate(Page::Vhf, vars),
            "SQWK" => self.navigate(Page::Transponder, vars),
            _ => {}
        }
    }

    fn audio_key_pressed(&mut self, key: &str, vars: &mut Vars) {
        let Some(pos) = self.key_defs.iter().position(|k| k.key == key) else { return };
        let new_state = !self.key_defs[pos].state;
        if new_state && key != "VOICE" {
            for j in 0..self.key_defs.len() {
                if j != pos {
                    self.audio_set_key_state(j, false, vars);
                }
            }
        }
        self.audio_set_key_state(pos, new_state, vars);
    }

    fn audio_set_key_state(&mut self, j: usize, v: bool, vars: &mut Vars) {
        if self.key_defs[j].state == v {
            return;
        }
        self.key_defs[j].state = v;
        let var = self.key_defs[j].var.clone();
        vars.set(&var, v as i32 as f64);
        if self.fo_sync_enabled {
            let base = var.splitn(2, &format!("RMP_{}_", self.idx)).nth(1).unwrap().to_string();
            for o in [if self.idx == 1 { 2 } else { 1 }, if self.idx == 3 { 1 } else { 3 }] {
                vars.set(&format!("L:A380X_RMP_{o}_{base}"), v as i32 as f64);
            }
        }
    }

    fn page_key_pressed(&mut self, key: &str, vars: &mut Vars) {
        match self.page {
            Page::Vhf => self.vhf_key_pressed(key, vars),
            Page::Transponder => self.xpdr_key_pressed(key, vars),
        }
    }

    fn page_key_released(&mut self, key: &str) {
        if key == "DIGIT_CLR" {
            match self.page {
                Page::Vhf => self.vhf_clr_timer.clear(),
                Page::Transponder => self.xpdr_clr_timer.clear(),
            }
        }
    }

    fn vhf_key_pressed(&mut self, key: &str, vars: &mut Vars) {
        let sel = self.selected_row;
        match key {
            "ADK_1" | "ADK_2" | "ADK_3" => {
                let r = (key.as_bytes()[4] - b'1') as usize;
                if sel == r {
                    self.mgr_swap(r, vars);
                } else {
                    self.select_row(r);
                }
            }
            "LSK_1" | "LSK_2" | "LSK_3" => {
                let r = (key.as_bytes()[4] - b'1') as usize;
                if sel == r {
                    self.row_lsk(r, vars);
                } else {
                    self.select_row(r);
                }
            }
            "DIGIT_CLR" => {
                self.row_clear(sel, true);
                self.vhf_clr_timer.schedule(self.now, CLR_HELD_MS);
            }
            "UP" => self.row_up(sel, vars),
            "DOWN" => self.row_down(sel, vars),
            k if k.starts_with("DIGIT_") && k.len() == 7 && k.as_bytes()[6].is_ascii_digit() => {
                self.row_digit(sel, (k.as_bytes()[6] - b'0') as u32);
            }
            _ => {}
        }
    }

    fn select_row(&mut self, r: usize) {
        let prev = self.selected_row;
        if prev == r {
            return;
        }
        self.selected_row = r;
        self.rows[prev].entry = Entry::empty();
        self.row_refresh(prev);
    }

    fn row_set_entry(&mut self, r: usize, e: Entry) {
        self.rows[r].entry = e;
        self.row_refresh(r);
    }

    fn row_refresh(&mut self, r: usize) {
        let e = self.rows[r].entry.clone();
        let invalid = e.frequency != 0 && e.frequency != 0x1100000 && !is_valid_frequency(e.frequency);
        if invalid != self.rows[r].invalid {
            self.rows[r].invalid = invalid;
            if invalid {
                self.message_set(&format!("VHF{} FREQ NOT VALID", r + 1));
            }
        }
        let in_progress = !e.entered.is_empty();
        self.rows[r].in_progress = in_progress;
        let display: String = if in_progress {
            e.displayed.clone()
        } else {
            match self.bus[r].standby {
                None => NO_ENTRY_FREQ.to_string(),
                Some(f) => format_bcd32(f, ""),
            }
        };
        let chars: Vec<char> = display.chars().collect();
        for i in 0..6 {
            self.rows[r].digits[i] = chars.get(i).copied().unwrap_or('_');
            self.rows[r].pilot[i] = in_progress && i >= e.offset && i < e.offset + e.entered.len();
        }
    }

    fn row_standby_mode(&self, r: usize) -> FrequencyMode {
        match self.bus[r].standby_mode {
            Some(m) => FrequencyMode::from_f64(m),
            None => {
                if r == 2 {
                    FrequencyMode::Data
                } else {
                    FrequencyMode::Frequency
                }
            }
        }
    }

    fn row_lsk(&mut self, r: usize, vars: &mut Vars) {
        let name = format!("VHF{}", r + 1);
        if self.rows[r].entry.entered.is_empty() {
            self.message_set(&format!("{name} FREQ NOT COMPLETED"));
            return;
        }
        let f = self.rows[r].entry.frequency;
        if is_valid_frequency(f) {
            self.cached_set_standby[r] = Some(f);
            if self.mgrs[r].subs_active {
                self.mgr_set_standby_frequency(r, f, vars);
            }
        } else {
            self.message_set(&format!("{name} STBY REVERTED TO PREV ENTRY"));
        }
        self.row_set_entry(r, Entry::empty());
    }

    fn row_digit(&mut self, r: usize, digit: u32) {
        if self.row_standby_mode(r) != FrequencyMode::Frequency {
            return;
        }
        let cur = self.rows[r].entry.clone();
        if cur.entered.is_empty() && digit == 1 {
            self.row_set_entry(r, Entry { entered: "1".into(), offset: 0, displayed: "1_____".into(), frequency: 0 });
            return;
        }
        if cur.entered.len() >= 6 || self.rows[r].invalid {
            return;
        }
        let entered = format!("{}{}", cur.entered, digit);
        let shift = 4 + 4 * (6 - entered.len() as u32);
        let mut frequency = u32::from_str_radix(&entered, 16).unwrap() << shift;
        if is_valid_frequency(frequency) || entered.len() == 6 {
            self.row_set_entry(r, Entry { entered: entered.clone(), offset: 0, displayed: format!("{:0<6}", entered), frequency });
            return;
        }
        if entered.starts_with('3') {
            frequency = 0x1000000 | (frequency >> 4);
            if entered.len() < 5 && frequency < VHF_COM_8_33_25.max {
                frequency = 0;
            }
            self.row_set_entry(r, Entry { entered: entered.clone(), offset: 1, displayed: format!("1{:_<5}", entered), frequency });
            return;
        }
        for i in 0..3 {
            if i > 0 {
                frequency >>= 4;
                frequency |= 0x1000000;
                if is_valid_frequency(frequency) {
                    self.row_set_entry(r, Entry { entered: entered.clone(), offset: i, displayed: format_bcd32(frequency, ""), frequency });
                    return;
                }
            }
            if cur.offset + entered.len() == 5 {
                for &chan in VHF_COM_8_33_25.channels {
                    if (chan & 0xf00) != (frequency & 0xf00) {
                        continue;
                    }
                    frequency &= !0xffu32;
                    frequency |= chan;
                    if is_valid_frequency(frequency) {
                        self.row_set_entry(r, Entry { entered: entered.clone(), offset: i, displayed: format_bcd32(frequency, ""), frequency });
                        return;
                    }
                }
            }
        }
        frequency = u32::from_str_radix(&entered, 16).unwrap() << shift;
        let shown: String = format_bcd32(frequency, "").chars().take(entered.len()).collect();
        self.row_set_entry(r, Entry { entered: entered.clone(), offset: 0, displayed: format!("{:_<6}", shown), frequency });
    }

    fn row_clear(&mut self, r: usize, single: bool) {
        let entered = self.rows[r].entry.entered.clone();
        if entered.is_empty() {
            return;
        }
        self.row_set_entry(r, Entry::empty());
        if single && entered.len() > 1 {
            let chars: Vec<char> = entered.chars().collect();
            for c in chars.iter().take(chars.len() - 1) {
                self.row_digit(r, c.to_digit(10).unwrap());
            }
            let ne = self.rows[r].entry.clone();
            let take = ne.offset + ne.entered.len();
            let shown: String = ne.displayed.chars().take(take).collect();
            let mut e = ne;
            e.displayed = format!("{:_<6}", shown);
            self.row_set_entry(r, e);
        } else if entered.len() == 1 {
            self.message_set(&format!("VHF{} STBY REVERTED TO PREV ENTRY", r + 1));
        }
    }

    fn row_up(&mut self, r: usize, vars: &mut Vars) {
        let old = self.row_standby_mode(r) as i32;
        let max = if r == 2 { 2 } else { 0 };
        if old >= max {
            return;
        }
        self.row_set_entry(r, Entry::empty());
        self.mgr_set_standby_mode_if_subs(r, FrequencyMode::from_f64((old + 1) as f64), vars);
    }

    fn row_down(&mut self, r: usize, vars: &mut Vars) {
        let old = self.row_standby_mode(r) as i32;
        if old <= 0 {
            return;
        }
        self.row_set_entry(r, Entry::empty());
        self.mgr_set_standby_mode_if_subs(r, FrequencyMode::from_f64((old - 1) as f64), vars);
    }

    fn mgr_set_standby_mode_if_subs(&mut self, r: usize, m: FrequencyMode, vars: &mut Vars) {
        if self.mgrs[r].subs_active {
            self.mgr_set_standby_mode(r, m, vars);
        }
    }

    fn xpdr_set_entered(&mut self, v: Option<String>) {
        if self.xpdr.entered != v {
            self.xpdr.entered = v;
            self.xpdr.invalid = false;
        }
    }

    fn xpdr_set_invalid(&mut self, v: bool) {
        if self.xpdr.invalid != v {
            self.xpdr.invalid = v;
            if v {
                self.message_set("SQUAWK CODE NOT VALID");
            }
        }
    }

    fn xpdr_key_pressed(&mut self, key: &str, vars: &mut Vars) {
        match key {
            "ADK_3" => self.xpdr_ident(vars),
            "LSK_1" | "LSK_2" | "LSK_3" => self.xpdr_validate(vars),
            "DIGIT_CLR" => {
                self.xpdr_clear(true);
                self.xpdr_clr_timer.schedule(self.now, CLR_HELD_MS);
            }
            k if k.starts_with("DIGIT_") && k.len() == 7 && k.as_bytes()[6].is_ascii_digit() => {
                self.xpdr_digit((k.as_bytes()[6] - b'0') as u32);
            }
            _ => {}
        }
    }

    fn xpdr_set_ident(&mut self, v: bool) {
        if self.xpdr.ident_active != v {
            self.xpdr.ident_active = v;
            if self.xpdr.ident_msg_active {
                if v {
                    self.message_set("TRANSMITTING IDENT");
                } else {
                    self.message_clear_if("TRANSMITTING IDENT");
                }
            }
        }
    }

    fn xpdr_ident(&mut self, vars: &mut Vars) {
        if self.xpdr.ident_active {
            return;
        }
        vars.key("XPNDR_IDENT_ON", &[1.]);
        self.xpdr.ident_timer.clear();
        self.xpdr_set_ident(true);
        self.xpdr.ident_timer.schedule(self.now, IDENT_MS);
    }

    fn xpdr_clear(&mut self, single: bool) {
        self.xpdr.auto_timer.clear();
        let Some(old) = self.xpdr.entered.clone() else { return };
        if !single {
            self.xpdr_set_entered(None);
        } else if old.find('_').map_or(false, |p| p > 1) {
            let cut: String = old.chars().take(old.len() - 2).collect();
            self.xpdr_set_entered(Some(format!("{:_<4}", cut)));
        } else {
            self.xpdr_set_entered(None);
            self.message_set("SQUAWK CODE REVERTED TO PREV ENTRY");
        }
    }

    fn xpdr_digit(&mut self, digit: u32) {
        self.xpdr.auto_timer.clear();
        let entered = self.xpdr.entered.clone();
        if self.xpdr.invalid || entered.as_ref().map_or(false, |e| !e.contains('_')) {
            return;
        }
        match entered {
            None => self.xpdr_set_entered(Some(format!("{digit}___"))),
            Some(e) => {
                let upto = e.find('_').unwrap();
                let digits = format!("{}{}", &e[..upto], digit);
                self.xpdr_set_entered(Some(format!("{:_<4}", digits)));
                if digits.len() == 4 {
                    self.xpdr.auto_timer.schedule(self.now, AUTO_VALIDATE_MS);
                }
            }
        }
        if digit == 8 || digit == 9 {
            self.xpdr_set_invalid(true);
        }
    }

    fn xpdr_validate(&mut self, vars: &mut Vars) {
        self.xpdr.auto_timer.clear();
        let Some(code) = self.xpdr.entered.clone() else { return };
        if self.xpdr.invalid || code.contains('_') {
            self.message_set("SQUAWK CODE REVERTED TO PREV ENTRY");
            self.xpdr_set_entered(None);
            return;
        }
        let bcd = u32::from_str_radix(&code, 16).unwrap_or(0);
        vars.key("XPNDR_SET", &[bcd as f64]);
        self.xpdr_set_entered(None);
    }

    fn run_timers(&mut self, upto: f64, vars: &mut Vars) {
        loop {
            let mut next: Option<(f64, u8, usize)> = None;
            let consider = |d: Option<f64>, kind: u8, i: usize, next: &mut Option<(f64, u8, usize)>| {
                if let Some(d) = d {
                    if d <= upto && next.map_or(true, |n| d < n.0) {
                        *next = Some((d, kind, i));
                    }
                }
            };
            consider(self.vhf_clr_timer.due, 0, 0, &mut next);
            consider(self.xpdr_clr_timer.due, 1, 0, &mut next);
            consider(self.xpdr.auto_timer.due, 2, 0, &mut next);
            consider(self.xpdr.ident_timer.due, 3, 0, &mut next);
            for i in 0..3 {
                consider(self.rows[i].flash_timer.due, 4, i, &mut next);
            }
            for i in 0..self.rx_defs.len() {
                consider(self.rx_defs[i].timer.due, 5, i, &mut next);
            }
            let Some((t, kind, i)) = next else { break };
            let saved = self.now;
            self.now = t;
            match kind {
                0 => {
                    self.vhf_clr_timer.clear();
                    let sel = self.selected_row;
                    self.row_clear(sel, false);
                }
                1 => {
                    self.xpdr_clr_timer.clear();
                    self.xpdr_clear(false);
                }
                2 => {
                    self.xpdr.auto_timer.clear();
                    self.xpdr_validate(vars);
                }
                3 => {
                    self.xpdr.ident_timer.clear();
                    self.xpdr_set_ident(false);
                }
                4 => {
                    self.rows[i].flash_timer.clear();
                    self.rows[i].tx_flashing = false;
                }
                _ => {
                    self.rx_defs[i].timer.clear();
                    let v = self.rx_defs[i].rx_var.clone();
                    vars.set(&v, 1.);
                }
            }
            self.now = saved.max(t);
        }
    }

    fn rows_reception(&mut self, r: usize) {
        let row = &mut self.rows[r];
        let mode = if row.rx {
            Reception::Receive
        } else if row.tx {
            Reception::TransmitOnly
        } else {
            Reception::Off
        };
        if mode != row.reception {
            row.reception = mode;
            if mode == Reception::TransmitOnly {
                row.tx_flashing = true;
                row.flash_timer.schedule(self.now, TX_ONLY_FLASH_MS);
            }
        }
    }

    fn publish_audio(&mut self, vars: &mut Vars) {
        let mut rx_changed: Vec<(usize, f64)> = Vec::new();
        for k in 0..self.rx_defs.len() {
            let v = vars.get(&self.rx_defs[k].switch_var);
            if self.rx_defs[k].last != Some(v) {
                self.rx_defs[k].last = Some(v);
                rx_changed.push((k, v));
            }
        }
        let mut tx_changed: Vec<(usize, f64)> = Vec::new();
        for (m, (kind, _, i)) in TX_MONITOR.iter().enumerate() {
            let v = vars.get(&format!("L:A380X_RMP_{}_{}_TX_{}", self.idx, kind, i));
            if self.tx_last[m] != Some(v) {
                self.tx_last[m] = Some(v);
                tx_changed.push((m, v));
            }
        }
        for (k, v) in rx_changed.iter().filter(|(k, _)| *k < 3) {
            self.rows[*k].rx = *v != 0.;
            self.rows_reception(*k);
        }
        for (m, v) in tx_changed.iter().filter(|(m, _)| *m < 3) {
            self.rows[*m].tx = *v != 0.;
            self.rows_reception(*m);
        }
        for &(m, v) in &tx_changed {
            let (kind, _, i) = TX_MONITOR[m];
            let rx = self.rx_defs[m].last.map_or(false, |x| x != 0.);
            if v != 0. && !rx {
                self.message_set(&format!("{kind}{i} RECEPTION NOT SELECTED"));
            }
        }
        for (k, v) in rx_changed {
            if v != 0. {
                self.rx_defs[k].timer.schedule(self.now, RX_DEBOUNCE_MS);
            } else {
                self.rx_defs[k].timer.clear();
                let var = self.rx_defs[k].rx_var.clone();
                vars.set(&var, 0.);
            }
        }
    }

    fn publish_radio_and_xpdr(&mut self, vars: &mut Vars) {
        for i in 0..3 {
            let v = vars.get(&format!("L:FBW_VHF{}_FREQUENCY", i + 1));
            if self.radio_last[i] != Some(v) {
                self.radio_last[i] = Some(v);
                self.rows[i].radio_raw = v;
            }
        }
        let code = vars.get("TRANSPONDER CODE:1");
        if self.xpdr_code_last != Some(code) {
            self.xpdr_code_last = Some(code);
            self.squawk_code = code;
            self.recompute_squawk();
            self.xpdr.active_code = Some(code);
        }
        let st = vars.get("TRANSPONDER STATE:1") as i64;
        let auto = matches!(st, 3 | 4 | 5);
        if self.xpdr_auto_last != Some(auto) {
            self.xpdr_auto_last = Some(auto);
            self.squawk_auto = auto;
            self.recompute_squawk();
            self.xpdr.is_auto = auto;
            if self.xpdr.squawk_sub_active {
                self.set_squawk_enabled(!auto);
            }
        }
    }

    pub fn update(&mut self, dt_ms: f64, vars: &mut Vars) {
        let target = self.now + dt_ms;
        self.run_timers(target, vars);
        self.now = target;
        if self.flash_last.map_or(true, |l| self.now - l >= 500.) {
            self.flash_last = Some(self.now);
            self.flash_1hz = !self.flash_1hz;
        }
        self.update_state_controller(vars);
        self.publish_audio(vars);
        self.publish_radio_and_xpdr(vars);
        for i in 0..3 {
            self.mgr_update(i, vars);
        }
    }

    pub fn view(&self) -> View {
        let rows = std::array::from_fn(|r| {
            let row = &self.rows[r];
            let (_, ssm) = arinc_split(row.radio_raw);
            let failed = ssm == SSM_FAILURE_WARNING;
            let mode_raw = self.bus[r].active_mode.unwrap_or(if r == 2 { 1. } else { 0. });
            let mode = FrequencyMode::from_f64(mode_raw);
            let active_text = if mode == FrequencyMode::Data {
                if self.bus[r].active.is_none() || failed { "----".to_string() } else { "DATA".to_string() }
            } else {
                match self.bus[r].active {
                    Some(f) if !failed => format_bcd32(f, "."),
                    _ => "---.---".to_string(),
                }
            };
            let smode = self.row_standby_mode(r);
            let hidden = row.reception == Reception::Off || (row.reception == Reception::TransmitOnly && row.tx_flashing && !self.flash_1hz);
            RowView {
                active_text,
                active_amber: failed,
                standby_digits: row.digits.iter().collect(),
                standby_pilot_entered: row.pilot,
                standby_amber: row.invalid || self.bus[r].standby.is_none(),
                standby_mode_title: if smode == FrequencyMode::Emergency { "EMER" } else { "STBY" },
                standby_entry_in_progress: row.in_progress,
                loudspeaker_hidden: hidden,
                no_reception_cross: row.reception == Reception::TransmitOnly,
                transmitting: row.tx,
                selected: self.selected_row == r,
            }
        });
        let code = match &self.xpdr.entered {
            Some(e) => e.clone(),
            None => match self.xpdr.active_code {
                Some(c) => format!("{:04x}", c as u32),
                None => "----".to_string(),
            },
        };
        View {
            screen_on: self.state == RmpState::On,
            state: self.state,
            page: self.page,
            rows,
            xpdr: XpdrView { auto: self.xpdr.is_auto, code, entry_in_progress: self.xpdr.entered.is_some(), entry_invalid: self.xpdr.invalid, ident: self.xpdr.ident_active },
            message: self.msg_displayed.clone(),
        }
    }
}

pub fn swap_vhf(vhf: usize, vars: &mut Vars) {
    let af = format!("L:FBW_RMP_FREQUENCY_ACTIVE_{vhf}");
    let am = format!("L:FBW_RMP_MODE_ACTIVE_{vhf}");
    let sf = format!("L:FBW_RMP_FREQUENCY_STANDBY_{vhf}");
    let sm = format!("L:FBW_RMP_MODE_STANDBY_{vhf}");
    let (nsf, nsm) = (vars.get(&af), vars.get(&am));
    let (a, b) = (vars.get(&sf), vars.get(&sm));
    vars.set(&af, a);
    vars.set(&am, b);
    vars.set(&sf, nsf);
    vars.set(&sm, nsm);
}

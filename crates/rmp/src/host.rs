use crate::radio::*;
use crate::vars::*;

pub struct VhfRadioUnit {
    index: usize,
    circuit: u32,
    failure_key: &'static str,
    bus_a_rmp: usize,
    bus_b_rmp: usize,
    msfs_hz: Option<u32>,
    radio_on: bool,
    tuned: (u32, u32),
    started: bool,
}

impl VhfRadioUnit {
    pub fn new(index: usize) -> Self {
        let a = if index == 2 { 2 } else { 1 };
        let b = if index == 2 { 1 } else { 2 };
        VhfRadioUnit {
            index,
            circuit: match index {
                1 => 36,
                2 => 38,
                _ => 40,
            },
            failure_key: match index {
                1 => "Vhf1",
                2 => "Vhf2",
                _ => "Vhf3",
            },
            bus_a_rmp: a,
            bus_b_rmp: b,
            msfs_hz: None,
            radio_on: false,
            tuned: (0, SSM_FAILURE_WARNING),
            started: false,
        }
    }

    fn out_var(&self) -> String {
        format!("L:FBW_VHF{}_FREQUENCY", self.index)
    }

    fn set_msfs_frequency(&mut self, hz: u32, vars: &mut Vars) {
        if self.msfs_hz == Some(hz) {
            return;
        }
        self.msfs_hz = Some(hz);
        if hz > 0 {
            let name = if self.index == 1 { "COM_RADIO_SET_HZ".to_string() } else { format!("COM{}_RADIO_SET_HZ", self.index) };
            vars.key(&name, &[hz as f64]);
        } else {
            self.radio_on = false;
        }
        let is_on = vars.get(&format!("CIRCUIT SWITCH ON:{}", self.circuit)) > 0.;
        if (hz > 0) != is_on {
            vars.key("ELECTRICAL_CIRCUIT_TOGGLE", &[self.circuit as f64]);
        }
    }

    fn set_tuned(&mut self, value: u32, ssm: u32, vars: &mut Vars) {
        if !self.started || self.tuned != (value, ssm) {
            self.started = true;
            self.tuned = (value, ssm);
            vars.set(&self.out_var(), arinc_raw(value, ssm));
        }
    }

    pub fn update(&mut self, powered: bool, vars: &mut Vars) {
        let failed = vars.failed(self.failure_key);
        if powered && !failed {
            let use_a = vars.get(&format!("L:FBW_VHF{}_BUS_A", self.index)) > 0.;
            let var = if use_a {
                format!("L:FBW_RMP{}_PRIMARY_VHF{}_FREQUENCY", self.bus_a_rmp, self.index)
            } else {
                format!("L:FBW_RMP{}_BACKUP_VHF{}_FREQUENCY", self.bus_b_rmp, self.index)
            };
            let (value, ssm) = arinc_split(vars.get(&var));
            if ssm == SSM_NORMAL_OPERATION || ssm == SSM_FUNCTIONAL_TEST {
                self.set_msfs_frequency(unpack_vhf_arinc_to_hz(value), vars);
            } else {
                self.set_msfs_frequency(0, vars);
            }
            if self.radio_on {
                let f = vars.get(&format!("COM ACTIVE FREQUENCY:{}", self.index)) as u32;
                self.set_tuned(pack_bcd32_vhf_to_arinc(f), SSM_NORMAL_OPERATION, vars);
            } else {
                self.set_tuned(0, SSM_NO_COMPUTED_DATA, vars);
            }
        } else {
            self.set_msfs_frequency(0, vars);
            self.set_tuned(0, SSM_FAILURE_WARNING, vars);
        }
    }
}

pub struct TransponderUnit {
    circuit: u32,
    state: u32,
    pub auto: bool,
    pub alt_reporting: bool,
    started: bool,
}

impl TransponderUnit {
    pub fn new() -> Self {
        TransponderUnit { circuit: 41, state: 0, auto: false, alt_reporting: true, started: false }
    }

    pub fn flight_start(&mut self, vars: &mut Vars) {
        vars.key("XPNDR_SET", &[0x2000 as f64]);
        if vars.get("L:A32NX_COLD_AND_DARK_SPAWN") == 0. {
            self.auto = true;
        }
    }

    pub fn update(&mut self, powered: bool, vars: &mut Vars) {
        let failed = vars.failed("Transponder1");
        let on = vars.get(&format!("CIRCUIT SWITCH ON:{}", self.circuit)) > 0.;
        let should_be_on = powered && !failed;
        if on != should_be_on {
            vars.key("ELECTRICAL_CIRCUIT_TOGGLE", &[self.circuit as f64]);
        }
        let on_ground = vars.get("L:A32NX_LGCIU_1_LEFT_GEAR_COMPRESSED") > 0.;
        let new = if !should_be_on {
            0
        } else if !self.auto {
            1
        } else if !on_ground && self.alt_reporting {
            4
        } else if !on_ground {
            3
        } else {
            5
        };
        if !self.started || new != self.state {
            self.started = true;
            self.state = new;
            vars.set("TRANSPONDER STATE:1", new as f64);
        }
    }
}

impl Default for TransponderUnit {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComIndex {
    Vhf1 = 1,
    Vhf2 = 2,
    Vhf3 = 3,
}

pub struct Amu {
    index: usize,
    pub healthy: bool,
    onside_on: bool,
    rmp3_on: bool,
}

impl Amu {
    pub fn new(index: usize) -> Self {
        Amu { index, healthy: false, onside_on: false, rmp3_on: false }
    }

    pub fn update(&mut self, vars: &Vars) {
        let failed = vars.failed(if self.index == 2 { "AudioManagementUnit2" } else { "AudioManagementUnit1" });
        let powered = vars.get("L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED") > 0.;
        self.healthy = !failed && powered;
        let on = |v: f64| v as i64 == 2 || v as i64 == 3;
        self.onside_on = on(vars.get(&format!("L:A380X_RMP_{}_STATE", self.index)));
        self.rmp3_on = on(vars.get("L:A380X_RMP_3_STATE"));
    }

    pub fn active_rmp(&self) -> usize {
        let offside = if self.index == 2 { 1 } else { 2 };
        if self.onside_on {
            self.index
        } else if self.rmp3_on {
            3
        } else {
            offside
        }
    }

    fn v(&self, vars: &Vars, suffix: &str) -> f64 {
        vars.get(&format!("L:A380X_RMP_{}_{}", self.active_rmp(), suffix))
    }

    pub fn selected_com_transmitter(&self, vars: &Vars) -> Option<ComIndex> {
        if self.v(vars, "VHF_TX_1") != 0. {
            Some(ComIndex::Vhf1)
        } else if self.v(vars, "VHF_TX_2") != 0. {
            Some(ComIndex::Vhf2)
        } else if self.v(vars, "VHF_TX_3") != 0. {
            Some(ComIndex::Vhf3)
        } else {
            None
        }
    }

    pub fn com_receive_on(&self, vars: &Vars, i: ComIndex) -> bool {
        self.v(vars, &format!("VHF_VOL_RX_{}", i as i32)) != 0.
    }

    pub fn com_volume(&self, vars: &Vars, i: ComIndex) -> f64 {
        self.v(vars, &format!("VHF_VOL_{}", i as i32))
    }

    pub fn nav_selected(&self, vars: &Vars) -> i32 {
        self.v(vars, "NAV_SEL") as i32
    }

    pub fn nav_output_on(&self, vars: &Vars) -> bool {
        self.v(vars, "NAV_VOL_RX") != 0.
    }

    pub fn nav_voice_filtered(&self, vars: &Vars) -> bool {
        self.v(vars, "NAV_FILTER") != 0.
    }

    pub fn nav_volume(&self, vars: &Vars) -> f64 {
        self.v(vars, "NAV_VOL")
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SimComTarget {
    pub receive: bool,
    pub transmit: bool,
    pub volume: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SimAudioTarget {
    pub active_amu: usize,
    pub com: [SimComTarget; 3],
    pub nav_selected: Option<i32>,
    pub nav_ident_on: bool,
    pub nav_volume: f64,
}

pub fn sim_audio_target(amu1: &Amu, amu2: &Amu, pilot_seat_right: bool, vars: &Vars) -> SimAudioTarget {
    let active_index = if !amu1.healthy || pilot_seat_right { 2 } else { 1 };
    let amu = if active_index == 2 { amu2 } else { amu1 };
    let mut t = SimAudioTarget { active_amu: active_index, ..Default::default() };
    if !amu.healthy {
        return t;
    }
    t.nav_selected = Some(amu.nav_selected(vars));
    t.nav_ident_on = amu.nav_output_on(vars) && !amu.nav_voice_filtered(vars);
    t.nav_volume = amu.nav_volume(vars);
    let tx = amu.selected_com_transmitter(vars);
    for (n, i) in [ComIndex::Vhf1, ComIndex::Vhf2, ComIndex::Vhf3].into_iter().enumerate() {
        t.com[n] = SimComTarget { receive: amu.com_receive_on(vars, i), transmit: tx == Some(i), volume: amu.com_volume(vars, i) };
    }
    t
}

pub struct KeyInterceptorRmp {
    pub sync_enabled: bool,
    pub pilot_seat_right: bool,
}

fn off(state: f64) -> bool {
    matches!(state as i64, 0 | 1)
}

fn on(state: f64) -> bool {
    matches!(state as i64, 2 | 3)
}

impl KeyInterceptorRmp {
    pub fn controls(&self, rmp: usize, vars: &Vars) -> bool {
        let s = |n: usize| vars.get(&format!("L:A380X_RMP_{n}_STATE"));
        match rmp {
            1 => self.sync_enabled || off(s(3)) || (!self.pilot_seat_right && on(s(1))),
            2 => self.sync_enabled || off(s(3)) || (self.pilot_seat_right && on(s(2))),
            _ => self.sync_enabled || (!self.pilot_seat_right && off(s(1))) || (self.pilot_seat_right && off(s(2))),
        }
    }

    pub fn fract_increment(&self, vhf: usize, sign: i32, carry: bool, vars: &mut Vars) {
        let sf = format!("L:FBW_RMP_FREQUENCY_STANDBY_{vhf}");
        let next = get_next_valid_channel(vars.get(&sf) as u32, sign, carry);
        vars.set(&sf, next as f64);
        vars.set(&format!("L:FBW_RMP_MODE_STANDBY_{vhf}"), FrequencyMode::Frequency as i32 as f64);
    }

    pub fn whole_increment(&self, vhf: usize, sign: i32, vars: &mut Vars) {
        let sf = format!("L:FBW_RMP_FREQUENCY_STANDBY_{vhf}");
        let next = get_closest_valid_frequency(increment_bcd32(vars.get(&sf) as u32, 4, sign));
        vars.set(&sf, next as f64);
        vars.set(&format!("L:FBW_RMP_MODE_STANDBY_{vhf}"), FrequencyMode::Frequency as i32 as f64);
    }

    pub fn active_set(&self, vhf: usize, hertz: bool, value0: f64, vars: &mut Vars) {
        let f = if hertz { pack_bcd32_hz(value0) } else { bcd16_to_bcd32(value0 as u32) };
        vars.set(&format!("L:FBW_RMP_FREQUENCY_ACTIVE_{vhf}"), get_closest_valid_frequency(f) as f64);
        vars.set(&format!("L:FBW_RMP_MODE_ACTIVE_{vhf}"), FrequencyMode::Frequency as i32 as f64);
    }

    pub fn standby_set(&self, vhf: usize, hertz: bool, value0: f64, vars: &mut Vars) {
        let f = if hertz { pack_bcd32_hz(value0) } else { bcd16_to_bcd32(value0 as u32) };
        vars.set(&format!("L:FBW_RMP_FREQUENCY_STANDBY_{vhf}"), get_closest_valid_frequency(f) as f64);
        vars.set(&format!("L:FBW_RMP_MODE_STANDBY_{vhf}"), FrequencyMode::Frequency as i32 as f64);
    }

    pub fn volume_set(&self, vhf: usize, value0: f64, vars: &mut Vars) {
        let v = value0.clamp(0., 100.);
        for rmp in 1..=3usize {
            if self.controls(rmp, vars) {
                vars.set(&format!("L:A380X_RMP_{rmp}_VHF_VOL_{vhf}"), v);
            }
        }
    }

    pub fn volume_increment(&self, vhf: usize, sign: i32, vars: &mut Vars) {
        for rmp in 1..=3usize {
            if self.controls(rmp, vars) {
                let name = format!("L:A380X_RMP_{rmp}_VHF_VOL_{vhf}");
                let v = (vars.get(&name) + 2. * sign as f64).clamp(0., 100.);
                vars.set(&name, v);
            }
        }
    }

    pub fn swap(&self, vhf: usize, vars: &mut Vars) {
        crate::panel::swap_vhf(vhf, vars);
    }

    pub fn receive_switch(&self, vhf: usize, rx_on: bool, vars: &mut Vars) {
        for rmp in 1..=3usize {
            if self.controls(rmp, vars) {
                vars.set(&format!("L:A380X_RMP_{rmp}_VHF_VOL_RX_SWITCH_{vhf}"), rx_on as i32 as f64);
            }
        }
    }
}

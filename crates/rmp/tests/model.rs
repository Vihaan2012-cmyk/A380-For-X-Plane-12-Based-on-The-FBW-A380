use rmp::*;
use rmp::radio::*;

fn powered(idx: usize) -> (Rmp, Vars) {
    let mut vars = Vars::new();
    vars.set("L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED", 1.);
    vars.set("L:A32NX_ELEC_DC_1_BUS_IS_POWERED", 1.);
    vars.set(&format!("L:A380X_RMP_{idx}_BRIGHTNESS_KNOB"), 50.);
    let mut rmp = Rmp::new(idx);
    rmp.init(&mut vars);
    rmp.flight_start(&mut vars);
    for _ in 0..4 {
        rmp.update(50., &mut vars);
    }
    (rmp, vars)
}

fn press(rmp: &mut Rmp, vars: &mut Vars, key: &str) {
    let idx = rmp.index();
    rmp.hevent(&format!("RMP_{idx}_{key}_PRESSED"), vars);
    rmp.hevent(&format!("RMP_{idx}_{key}_RELEASED"), vars);
    rmp.update(50., vars);
}

#[test]
fn power_and_leds() {
    let (rmp, vars) = powered(1);
    assert_eq!(rmp.state(), RmpState::On);
    assert_eq!(vars.get("L:A380X_RMP_1_STATE"), 2.);
    assert_eq!(vars.get("L:A380X_RMP_1_GREEN_LED"), 0.);
    assert_eq!(vars.get("L:A380X_RMP_1_RED_LED"), 0.);
    assert_eq!(vars.get("L:FBW_VHF1_BUS_A"), 1.);
}

#[test]
fn off_standby_green_led_and_failed_red_led() {
    let mut vars = Vars::new();
    vars.set("L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED", 1.);
    let mut rmp = Rmp::new(2);
    rmp.init(&mut vars);
    rmp.update(50., &mut vars);
    assert_eq!(vars.get("L:A380X_RMP_2_GREEN_LED"), 1.);
    vars.set_failure("RadioManagementPanel2", true);
    vars.set("L:A380X_RMP_2_BRIGHTNESS_KNOB", 10.);
    rmp.update(50., &mut vars);
    assert_eq!(rmp.state(), RmpState::OnFailed);
    assert_eq!(vars.get("L:A380X_RMP_2_RED_LED"), 1.);
}

#[test]
fn standby_entry_and_transfer() {
    let (mut rmp, mut vars) = powered(1);
    for d in ["1", "1", "8", "9", "5", "0"] {
        press(&mut rmp, &mut vars, &format!("DIGIT_{d}"));
    }
    assert_eq!(rmp.view().rows[0].standby_digits, "118950");
    press(&mut rmp, &mut vars, "LSK_1");
    assert_eq!(vars.get("L:FBW_RMP_FREQUENCY_STANDBY_1"), 0x1189500 as f64);
    assert_eq!(rmp.view().rows[0].standby_digits, "118950");
    press(&mut rmp, &mut vars, "ADK_1");
    assert_eq!(vars.get("L:FBW_RMP_FREQUENCY_ACTIVE_1"), 0x1189500 as f64);
    assert_eq!(vars.get("L:FBW_RMP_FREQUENCY_STANDBY_1"), 0.);
}

#[test]
fn incomplete_entry_message() {
    let (mut rmp, mut vars) = powered(1);
    press(&mut rmp, &mut vars, "LSK_1");
    assert_eq!(rmp.view().message, "VHF1 FREQ NOT COMPLETED");
    press(&mut rmp, &mut vars, "MSG_CLR");
    assert_ne!(rmp.view().message, "VHF1 FREQ NOT COMPLETED");
}

#[test]
fn vhf3_data_mode() {
    let (mut rmp, mut vars) = powered(1);
    assert_eq!(vars.get("L:FBW_RMP_FREQUENCY_ACTIVE_3"), DATA_FREQUENCY as f64);
    assert_eq!(rmp.view().rows[2].active_text, "---.---".replace("---.---", "----"));
    press(&mut rmp, &mut vars, "LSK_3");
    press(&mut rmp, &mut vars, "UP");
    press(&mut rmp, &mut vars, "UP");
    assert_eq!(vars.get("L:FBW_RMP_MODE_STANDBY_3"), 2.);
    assert_eq!(vars.get("L:FBW_RMP_FREQUENCY_STANDBY_3"), EMERGENCY_FREQUENCY as f64);
    assert_eq!(rmp.view().rows[2].standby_mode_title, "EMER");
}

#[test]
fn squawk_entry_auto_validates() {
    let (mut rmp, mut vars) = powered(1);
    vars.set_quiet("TRANSPONDER CODE:1", 0x2000 as f64);
    press(&mut rmp, &mut vars, "SQWK");
    for d in ["7", "7", "0", "0"] {
        press(&mut rmp, &mut vars, &format!("DIGIT_{d}"));
    }
    assert_eq!(rmp.view().xpdr.code, "7700");
    vars.take_keys();
    for _ in 0..25 {
        rmp.update(50., &mut vars);
    }
    let keys = vars.take_keys();
    assert!(keys.iter().any(|k| k.name == "XPNDR_SET" && k.args == vec![0x7700 as f64]));
}

#[test]
fn squawk_rejects_8_and_9() {
    let (mut rmp, mut vars) = powered(1);
    press(&mut rmp, &mut vars, "SQWK");
    press(&mut rmp, &mut vars, "DIGIT_8");
    assert!(rmp.view().xpdr.entry_invalid);
    assert_eq!(rmp.view().message, "SQUAWK CODE NOT VALID");
}

#[test]
fn audio_tx_keys_are_exclusive() {
    let (mut rmp, mut vars) = powered(1);
    press(&mut rmp, &mut vars, "VHF_CALL_1");
    assert_eq!(vars.get("L:A380X_RMP_1_VHF_TX_1"), 1.);
    press(&mut rmp, &mut vars, "HF_CALL_1");
    assert_eq!(vars.get("L:A380X_RMP_1_VHF_TX_1"), 0.);
    assert_eq!(vars.get("L:A380X_RMP_1_HF_TX_1"), 1.);
}

#[test]
fn rx_switch_debounce_and_tx_message() {
    let (mut rmp, mut vars) = powered(1);
    vars.set("L:A380X_RMP_1_VHF_VOL_RX_SWITCH_1", 1.);
    rmp.update(50., &mut vars);
    assert_eq!(vars.get("L:A380X_RMP_1_VHF_VOL_RX_1"), 0.);
    for _ in 0..8 {
        rmp.update(50., &mut vars);
    }
    assert_eq!(vars.get("L:A380X_RMP_1_VHF_VOL_RX_1"), 1.);
    press(&mut rmp, &mut vars, "VHF_CALL_2");
    assert_eq!(rmp.view().message, "VHF2 RECEPTION NOT SELECTED");
}

#[test]
fn vhf_radio_unit_failure_word() {
    let mut vars = Vars::new();
    let mut unit = VhfRadioUnit::new(1);
    unit.update(false, &mut vars);
    assert_eq!(vars.get("L:FBW_VHF1_FREQUENCY"), 0.);
    unit.update(true, &mut vars);
    assert_eq!(vars.get("L:FBW_VHF1_FREQUENCY") as u64 >> 32, 1);
}

#[test]
fn radio_channel_helpers() {

    assert!(is_valid_frequency(0x1189500));
    assert!(!is_valid_frequency(0x1189200));
    assert_eq!(format_bcd32(0x1189500, "."), "118.950");
    assert_eq!(get_next_valid_channel(0x1189000, 1, false), 0x1189050);
    assert_eq!(get_next_valid_channel(0x1189900, 1, true), 0x1190000);
    assert_eq!(pack_bcd32_hz(118_950_000.), 0x1189500);
}

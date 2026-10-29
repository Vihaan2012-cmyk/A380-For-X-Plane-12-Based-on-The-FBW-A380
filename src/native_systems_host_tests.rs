use super::*;

#[derive(Default)]
struct Fake {
    vars: HashMap<String, f64>,
    strings: HashMap<String, String>,
    written: Vec<(String, f64)>,
    failures: BTreeSet<u64>,
    keys: Vec<(String, Vec<f64>)>,
    stored: HashMap<String, String>,
}

impl Io for Fake {
    fn get(&mut self, name: &str, _unit: &str) -> f64 {
        self.vars.get(name).copied().unwrap_or(0.)
    }

    fn set(&mut self, name: &str, value: f64) {
        self.written.push((name.to_string(), value));
        self.vars.insert(name.to_string(), value);
    }

    fn get_string(&mut self, name: &str) -> String {
        self.strings.get(name).cloned().unwrap_or_default()
    }

    fn set_string(&mut self, name: &str, value: &str) {
        self.strings.insert(name.to_string(), value.to_string());
    }

    fn failure(&mut self, id: u64) -> bool {
        self.failures.contains(&id)
    }

    fn key(&mut self, name: &str, args: &[f64]) {
        self.keys.push((name.to_string(), args.to_vec()));
    }

    fn stored(&mut self, key: &str) -> Option<String> {
        self.stored.get(key).cloned()
    }

    fn nearest_airport(&mut self) -> Option<String> {
        Some("A      EGLL ".to_string())
    }
}

fn natives(raw: &str) -> Natives {
    let mut n = Natives::new();
    n.set_config(Config::parse(raw));
    n
}

fn run(n: &mut Natives, f: &mut Fake, frames: usize, t0: f64) -> (f64, Vec<String>) {
    let mut lines = Vec::new();
    let mut t = t0;
    for _ in 0..frames {
        t += 0.05;
        n.frame(f, t, Vec::new(), &mut lines);
    }
    (t, lines)
}

fn reported(lines: &[String], module: &str) -> Vec<String> {
    lines.iter().filter(|l| l.starts_with(&format!("native shadow: {module} "))).cloned().collect()
}

fn powered() -> Fake {
    let mut f = Fake::default();
    for k in VHF_POWER.iter().chain(&["L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED", "L:A32NX_ELEC_AC_2_BUS_IS_POWERED", "L:A32NX_ELEC_AC_4_BUS_IS_POWERED"]) {
        f.vars.insert(k.to_string(), 1.);
    }
    f
}

#[test]
fn config_selects_modules_and_all_leaves_out_the_incomplete_ones() {
    let c = Config::parse("vhf, XPDR,gpws");
    assert!(c.authoritative(Module::Vhf) && c.authoritative(Module::Xpdr) && !c.authoritative(Module::Amu));
    assert_eq!(c.mask(), 3.);
    let all = Config::parse("all");
    assert!(!all.authoritative(Module::Fws) && !all.authoritative(Module::Btv) && all.authoritative(Module::LightSync));
    assert!(!all.authoritative(Module::TerrainFeed));
    assert_eq!(Config::parse("terrain_feed").mask(), BIT_TERRAIN_FEED as f64);
    assert!(Config::parse("fws").authoritative(Module::Fws));
    assert!(Config::parse("off,vhf").off);
}

#[test]
fn vhf_in_shadow_matches_a_twin_and_reports_a_disagreement() {
    let mut n = natives("");
    let mut f = powered();
    f.vars.insert("L:FBW_VHF1_BUS_A".into(), 1.);
    f.vars.insert("L:FBW_RMP1_PRIMARY_VHF1_FREQUENCY".into(), rmp::vars::arinc_raw(0x18_0000, 3));
    let mut twin = Vhf::new();
    let mut lines = Vec::new();
    for i in 0..60 {
        let s = twin.step(&mut f);
        for (k, v) in s.compare {
            f.vars.insert(k, v);
        }
        n.frame(&mut f, 0.05 * (i + 1) as f64, Vec::new(), &mut lines);
    }
    assert!(reported(&lines, "vhf").is_empty(), "{lines:?}");
    assert!(f.written.iter().all(|(k, _)| k == MASK_VAR), "{:?}", f.written);
    f.vars.insert("L:FBW_VHF2_FREQUENCY".into(), 123.);
    let (_, lines) = run(&mut n, &mut f, 40, 3.);
    assert_eq!(reported(&lines, "vhf").len(), 1, "{lines:?}");
}

#[test]
fn authoritative_vhf_writes_the_frequency_and_sends_the_tuning_keys() {
    let mut n = natives("vhf");
    let mut f = powered();
    f.vars.insert("L:FBW_VHF1_BUS_A".into(), 1.);
    f.vars.insert("L:FBW_RMP1_PRIMARY_VHF1_FREQUENCY".into(), rmp::vars::arinc_raw(0x18_0000, 3));
    run(&mut n, &mut f, 3, 0.);
    assert!(f.written.contains(&(MASK_VAR.to_string(), BIT_VHF as f64)));
    assert!(f.written.iter().any(|(k, _)| k == "L:FBW_VHF1_FREQUENCY"));
    assert!(f.keys.iter().any(|(k, _)| k == "COM_RADIO_SET_HZ"), "{:?}", f.keys);
}

#[test]
fn xpdr_follows_the_mirrored_mfd_selection_like_a_twin() {
    let mut n = natives("xpdr");
    let mut f = powered();
    f.vars.insert(XPDR_MIRROR_VAR.into(), 1.);
    f.vars.insert(XPDR_AUTO_VAR.into(), 1.);
    f.vars.insert(XPDR_ALT_RPTG_VAR.into(), 1.);
    f.vars.insert("CIRCUIT SWITCH ON:41".into(), 1.);
    run(&mut n, &mut f, 3, 0.);
    assert_eq!(f.vars["TRANSPONDER STATE:1"], 4.);
    f.vars.insert("L:A32NX_LGCIU_1_LEFT_GEAR_COMPRESSED".into(), 1.);
    run(&mut n, &mut f, 2, 1.);
    assert_eq!(f.vars["TRANSPONDER STATE:1"], 5.);
    f.vars.insert(XPDR_AUTO_VAR.into(), 0.);
    run(&mut n, &mut f, 2, 2.);
    assert_eq!(f.vars["TRANSPONDER STATE:1"], 1.);
    assert!(f.keys.iter().any(|(k, a)| k == "XPNDR_SET" && a == &[8192.]));
}

#[test]
fn amu_sends_the_sim_audio_keys_once_per_change() {
    let mut n = natives("amu");
    let mut f = powered();
    f.vars.insert("L:A380X_RMP_1_STATE".into(), 2.);
    f.vars.insert("L:A380X_RMP_1_VHF_VOL_RX_1".into(), 1.);
    f.vars.insert("L:A380X_RMP_1_VHF_TX_1".into(), 1.);
    f.vars.insert("L:A380X_RMP_1_VHF_VOL_1".into(), 60.);
    run(&mut n, &mut f, 5, 0.);
    let count = |f: &Fake, k: &str| f.keys.iter().filter(|(n, _)| n == k).count();
    assert_eq!(count(&f, "COM1_RECEIVE_SELECT"), 1);
    assert!(f.keys.contains(&("COM1_VOLUME_SET".to_string(), vec![60.])));
    assert!(f.keys.contains(&("PILOT_TRANSMITTER_SET".to_string(), vec![0.])));
    f.vars.insert("L:A380X_RMP_1_VHF_VOL_RX_1".into(), 0.);
    run(&mut n, &mut f, 5, 1.);
    assert_eq!(count(&f, "COM1_RECEIVE_SELECT"), 2);
    assert_eq!(f.keys.iter().rev().find(|(n, _)| n == "COM1_RECEIVE_SELECT").unwrap().1, vec![0.]);
}

#[test]
fn amu_in_shadow_reports_a_receive_mismatch() {
    let mut n = natives("");
    let mut f = powered();
    f.vars.insert("L:A380X_RMP_1_STATE".into(), 2.);
    f.vars.insert("L:A380X_RMP_1_VHF_VOL_RX_2".into(), 1.);
    let (_, lines) = run(&mut n, &mut f, 40, 0.);
    assert!(reported(&lines, "amu").iter().any(|l| l.contains("COM RECEIVE:2 native=1 fbw=0")), "{lines:?}");
    assert!(f.keys.is_empty());
}

#[test]
fn btv_in_shadow_matches_a_twin_fed_the_same_inputs() {
    let mut n = natives("");
    let mut f = Fake::default();
    let mut twin = Oans::new(&NO_NAV);
    let (mut lines, mut t) = (Vec::new(), 0.);
    for _ in 0..80 {
        t += 0.05;
        twin.on_update(50.);
        for (k, v) in &twin.out.vars {
            f.vars.insert(k.clone(), *v);
        }
        n.frame(&mut f, t, Vec::new(), &mut lines);
    }
    assert!(reported(&lines, "btv").is_empty(), "{lines:?}");
    f.vars.insert("L:A32NX_OANS_RWY_LENGTH".into(), 3000.);
    let (_, lines) = run(&mut n, &mut f, 60, t);
    assert!(reported(&lines, "btv").iter().any(|l| l.contains("L:A32NX_OANS_RWY_LENGTH")), "{lines:?}");
}

#[test]
fn efis_taws_flags_follow_failures_power_and_reset_like_the_bridge() {
    let mut n = natives("efis_taws");
    let mut f = powered();
    run(&mut n, &mut f, 2, 0.);
    let flags = |f: &Fake| ["TERR_1", "TERR_2", "GPWS_1", "GPWS_2", "WXR_1", "WXR_2"].map(|s| f.vars[&format!("L:A32NX_{s}_FAILED")]);
    assert_eq!(flags(&f), [0.; 6]);
    f.failures.insert(34005);
    f.vars.insert("L:A32NX_RESET_PANEL_AESU2".into(), 1.);
    run(&mut n, &mut f, 2, 1.);
    assert_eq!(flags(&f), [1., 1., 0., 1., 0., 1.]);
    f.failures.clear();
    f.vars.insert("L:A32NX_RESET_PANEL_AESU2".into(), 0.);
    f.vars.insert("L:A32NX_ADIRS_IR_1_MAINT_WORD".into(), oans_native::arinc::Arinc::normal((1 << 14) as f64).raw());
    run(&mut n, &mut f, 2, 2.);
    assert_eq!(flags(&f), [1., 1., 0., 0., 0., 0.]);
}

#[test]
fn efis_taws_in_shadow_reports_when_fbw_disagrees() {
    let mut n = natives("");
    let mut f = powered();
    f.failures.insert(34007);
    let (_, lines) = run(&mut n, &mut f, 40, 0.);
    assert_eq!(reported(&lines, "efis_taws"), vec!["native shadow: efis_taws L:A32NX_GPWS_1_FAILED native=1 fbw=0".to_string()]);
}

#[test]
fn fws_in_shadow_matches_a_twin_fed_the_same_variables() {
    let mut n = natives("");
    let mut f = powered();
    for k in ["L:A32NX_CPIOM_C1_AVAIL", "L:A32NX_CPIOM_C2_AVAIL"] {
        f.vars.insert(k.into(), 1.);
    }
    let mut twin = fws::Fws::new(0.);
    let (mut lines, mut t) = (Vec::new(), 0.);
    for _ in 0..200 {
        t += 0.05;
        let events = twin.tick(&FwsVars(RefCell::new(&mut f as &mut dyn Io)), t * 1000.);
        for e in events {
            match e.out {
                fws::Out::Var(k, v) => {
                    f.vars.insert(k, v);
                }
                fws::Out::Str(k, v) => {
                    f.strings.insert(k, v);
                }
            }
        }
        n.frame(&mut f, t, Vec::new(), &mut lines);
    }
    assert!(!n.fws.latest.is_empty());
    assert!(reported(&lines, "fws").is_empty(), "{:?}", reported(&lines, "fws"));
    let (name, _) = n.fws.latest.iter().find(|(_, o)| matches!(o, fws::Out::Var(..))).unwrap();
    let name = name.clone();
    f.vars.insert(name.clone(), -42.);
    let (_, lines) = run(&mut n, &mut f, 60, t);
    assert!(reported(&lines, "fws").iter().any(|l| l.contains(&name)), "{lines:?}");
}

#[test]
fn light_sync_initialises_once_the_brightness_holds_then_drives_the_cabin() {
    let mut n = natives("light_sync");
    let mut f = Fake::default();
    f.vars.insert("GLASSCOCKPIT AUTOMATIC BRIGHTNESS".into(), 60.);
    f.vars.insert("A:CIRCUIT SWITCH ON:151".into(), 1.);
    run(&mut n, &mut f, 80, 0.);
    assert!(f.keys.is_empty());
    let (t, _) = run(&mut n, &mut f, 40, 4.);
    let twin = hosts_misc::light_sync::init_lighting(
        &LightSyncInputs { glasscockpit_automatic_brightness_pct: 60., cabin_circuit_switch_on: true, ..Default::default() },
        LightSyncConfig { integral_follows_glareshield: true },
    );
    let pots: Vec<_> = twin.iter().filter_map(|c| if let Command::TriggerKey { key, args, .. } = c { Some((key.clone(), args.clone())) } else { None }).collect();
    assert_eq!(f.keys, pots);
    assert_eq!(f.vars["L:A380X_PED_LIGHTING_AMBIENT_LT_KNOB"], 0.6);
    f.vars.insert("L:A32NX_CABIN_USING_AUTOBRIGHTNESS".into(), 1.);
    f.vars.insert("E:TIME OF DAY".into(), 3.);
    f.vars.insert("L:A32NX_FMGC_FLIGHT_PHASE".into(), 2.);
    run(&mut n, &mut f, 60, t);
    assert_eq!(f.vars["L:A32NX_CABIN_AUTOBRIGHTNESS"], 0.);
    assert_eq!(f.keys.last().unwrap(), &("LIGHT_POTENTIOMETER_SET".to_string(), vec![60., 0.]));
}

#[test]
fn intercepted_keys_reach_the_native_interceptor_and_match_the_twin() {
    let mut n = natives("key_interceptor");
    let mut f = Fake::default();
    f.vars.insert("L:A380X_RMP_1_STATE".into(), 2.);
    f.vars.insert("L:A380X_RMP_3_STATE".into(), 1.);
    f.vars.insert("L:FBW_RMP_FREQUENCY_STANDBY_1".into(), 0x0121_8000 as f64);
    assert_eq!(coherent_call(KEY_CALL, r#"["COM1_VOLUME_SET",40,null,null]"#), Some("true".into()));
    let keys = take(&KEYS_IN);
    assert_eq!(keys, vec![("COM1_VOLUME_SET".to_string(), vec![40.])]);
    let mut lines = Vec::new();
    n.frame(&mut f, 0.05, Vec::new(), &mut lines);
    n.frame(&mut f, 0.1, keys, &mut lines);
    let twin = ki::com_volume_set(&rmp_env(&mut f), 1, Some(40.));
    assert!(!twin.is_empty());
    for c in twin {
        let Command::SetLvar { name, value } = c else { panic!() };
        assert_eq!(f.vars[&format!("L:{name}")], value);
    }
    n.frame(&mut f, 0.15, vec![("COM_STBY_RADIO_SWAP".into(), vec![]), ("MASTER_CAUTION_ACKNOWLEDGE".into(), vec![])], &mut lines);
    assert_eq!(f.vars["L:FBW_RMP_FREQUENCY_ACTIVE_1"], 0x0121_8000 as f64);
    assert_eq!(f.vars["L:PUSH_AUTOPILOT_MASTERCAUT_L"], 1.);
    run(&mut n, &mut f, 12, 0.2);
    assert_eq!(f.vars["L:PUSH_AUTOPILOT_MASTERCAUT_L"], 0.);
    assert!(lines.iter().all(|l| !l.contains("not handled")), "{lines:?}");
}

#[test]
fn pilot_seat_in_shadow_matches_a_twin_manager() {
    let mut n = natives("");
    let mut f = Fake::default();
    f.stored.insert("CONFIG_PILOT_SEAT".into(), "Auto".into());
    f.vars.insert("GAME:CAMERA POS IN PLANE:X".into(), -0.5);
    f.vars.insert("GAME:CAMERA POS IN PLANE:Y".into(), 2.5);
    f.vars.insert("GAME:CAMERA POS IN PLANE:Z".into(), 31.5);
    f.vars.insert("CAMERA STATE".into(), 2.);
    let mut twin = PilotSeatManager::new(A380X_FLIGHT_DECK);
    twin.init(PilotSeatConfig::Auto);
    for c in twin.update((-0.5, 2.5, 31.5), 2) {
        if let Command::SetLvar { name, value } = c {
            f.vars.insert(format!("L:{name}"), value);
        }
    }
    let (t, lines) = run(&mut n, &mut f, 40, 0.);
    assert_eq!(f.vars["L:FBW_PILOT_SEAT"], 1.);
    assert!(reported(&lines, "pilot_seat").is_empty(), "{lines:?}");
    f.vars.insert("L:FBW_PILOT_SEAT".into(), 0.);
    let (_, lines) = run(&mut n, &mut f, 40, t);
    assert_eq!(reported(&lines, "pilot_seat"), vec!["native shadow: pilot_seat L:FBW_PILOT_SEAT native=1 fbw=0".to_string()]);
}

#[test]
fn pushbutton_held_in_cruise_raises_one_notification_for_the_extras_host() {
    let mut n = natives("pushbutton_check");
    let mut f = Fake::default();
    f.vars.insert("L:A32NX_BTN_TOCONFIG".into(), 1.);
    f.vars.insert("L:A32NX_FWC_FLIGHT_PHASE".into(), 8.);
    let mut lines = Vec::new();
    for i in 0..70 {
        n.frame(&mut f, i as f64 * 0.5, Vec::new(), &mut lines);
    }
    let out = n.take_outbox();
    assert_eq!(out.notifications.len(), 1);
    assert_eq!(out.notifications[0].0, "ECP Pushbutton Held");
    push_capped(&EXTRAS_OUT, out.notifications);
    let json: Value = serde_json::from_str(&coherent_call(TAKE_EXTRAS_CALL, "[]").unwrap()).unwrap();
    assert_eq!(json["notifications"][0]["title"], "ECP Pushbutton Held");
}

#[test]
fn baro_unit_and_ready_are_written_once() {
    let mut n = natives("baro_unit,ready");
    let mut f = Fake::default();
    run(&mut n, &mut f, 5, 0.);
    assert_eq!(f.vars["L:XMLVAR_Baro_Selector_HPA_1"], 1.);
    assert_eq!(f.vars["L:A32NX_IS_READY"], 1.);
    let mut n = natives("baro_unit");
    let mut f = Fake::default();
    f.stored.insert("CONFIG_INIT_BARO_UNIT".into(), "IN HG".into());
    run(&mut n, &mut f, 5, 0.);
    assert_eq!(f.vars["L:XMLVAR_Baro_Selector_HPA_2"], 0.);
    assert_eq!(f.written.iter().filter(|(k, _)| k == "L:XMLVAR_Baro_Selector_HPA_2").count(), 1);
}

#[test]
fn ready_in_shadow_reports_when_the_js_never_set_it() {
    let mut n = natives("");
    let mut f = Fake::default();
    let (_, lines) = run(&mut n, &mut f, 260, 0.);
    assert_eq!(reported(&lines, "ready"), vec!["native shadow: ready L:A32NX_IS_READY native=1 fbw=0".to_string()]);
}

#[test]
fn btv_bus_events_go_out_through_the_systems_take_call() {
    let mut n = natives("btv");
    let mut f = Fake::default();
    run(&mut n, &mut f, 3, 0.);
    let out = n.take_outbox();
    assert!(out.bus.iter().any(|(t, _)| t == "ndBtvMessage"), "{:?}", out.bus);
    push_capped(&SYSTEMS_OUT, out.bus);
    let json: Value = serde_json::from_str(&coherent_call(TAKE_SYSTEMS_CALL, "[]").unwrap()).unwrap();
    assert!(json["bus"].as_array().unwrap().iter().any(|e| e[0] == "ndBtvMessage"));
    assert_eq!(coherent_call("OTHER", "[]"), None);
}

fn rmp_powered() -> Fake {
    let mut f = Fake::default();
    f.vars.insert("L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED".into(), 1.);
    f.vars.insert("L:A380X_RMP_1_BRIGHTNESS_KNOB".into(), 50.);
    f
}

fn rmp_frames(p: &mut RmpPanels, f: &mut Fake, frames: usize, h: &[&str]) -> Option<Value> {
    let events: Vec<String> = h.iter().map(|s| s.to_string()).collect();
    let mut last = None;
    for k in 0..frames {
        let ev = if k == 0 { events.clone() } else { Vec::new() };
        if let Some((_, v)) = p.step(f, 50., &ev).into_iter().last() {
            last = Some(v);
        }
    }
    last
}

#[test]
fn a_native_rmp_comes_on_with_the_first_row_selected() {
    let mut p = RmpPanels::new([true, false, false]);
    let mut f = rmp_powered();
    let view = rmp_frames(&mut p, &mut f, 10, &[]).expect("a view");
    assert_eq!(f.vars.get("L:A380X_RMP_1_STATE"), Some(&2.));
    assert_eq!(view["screen_on"], true);
    assert_eq!(view["rows"][0]["selected"], true);
    assert_eq!(view["rows"][1]["selected"], false);
}

#[test]
fn rmp_digits_start_an_entry_and_a_line_key_selects_its_row() {
    let mut p = RmpPanels::new([true, false, false]);
    let mut f = rmp_powered();
    rmp_frames(&mut p, &mut f, 10, &[]);
    let view = rmp_frames(&mut p, &mut f, 4, &["RMP_1_LSK_2_PRESSED"]).expect("a view");
    rmp_frames(&mut p, &mut f, 2, &["RMP_1_LSK_2_RELEASED"]);
    assert_eq!(view["rows"][1]["selected"], true);
    let view = rmp_frames(&mut p, &mut f, 4, &["RMP_1_DIGIT_2_PRESSED", "RMP_1_DIGIT_2_RELEASED"]).expect("a view");
    assert_eq!(view["rows"][1]["standby_entry_in_progress"], true);
}

#[test]
fn an_rmp_failure_turns_the_panel_to_failed() {
    let mut p = RmpPanels::new([true, false, false]);
    let mut f = rmp_powered();
    rmp_frames(&mut p, &mut f, 10, &[]);
    f.failures.insert(23002);
    rmp_frames(&mut p, &mut f, 10, &[]);
    assert_eq!(f.vars.get("L:A380X_RMP_1_STATE"), Some(&3.));
}

#[test]
fn the_fws_reads_the_cockpits_own_names_for_three_ecp_buttons() {
    use fws::Vars;
    let mut f = Fake::default();
    f.vars.insert("L:A32NX_BTN_TOCONF".into(), 1.);
    f.vars.insert("L:A32NX_BTN_CLR_RH".into(), 1.);
    let v = FwsVars(RefCell::new(&mut f));
    assert_eq!(v.get("L:A32NX_BTN_TOCONFIG"), 1.);
    assert_eq!(v.get("L:A32NX_BTN_CLR2"), 1.);
    assert_eq!(v.get("L:A32NX_BTN_CLR"), 0.);
}

#[test]
fn a_pressed_ecp_button_is_released_once_the_fws_has_read_it() {
    let pressed = ["L:A32NX_BTN_CL", "L:A32NX_BTN_CHECK_LH", "L:A32NX_BTN_TOCONF"];
    let mut n = natives("fws");
    let mut f = powered();
    run(&mut n, &mut f, 4, 0.);
    for name in pressed {
        f.vars.insert(name.into(), 1.);
    }
    run(&mut n, &mut f, 1, 1.);
    for name in pressed {
        assert_eq!(f.vars.get(name), Some(&0.), "{name} must return to 0 so the next press is a new rising edge");
    }
    let mut shadow = natives("");
    let mut g = powered();
    g.vars.insert("L:A32NX_BTN_CL".into(), 1.);
    run(&mut shadow, &mut g, 2, 0.);
    assert_eq!(g.vars.get("L:A32NX_BTN_CL"), Some(&1.), "a shadow FWS leaves the JS one's input alone");
}

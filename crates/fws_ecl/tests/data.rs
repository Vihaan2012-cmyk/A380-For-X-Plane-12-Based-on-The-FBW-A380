use fws_ecl::logic::*;
use fws_ecl::*;
use std::path::PathBuf;

const FBW_SYSTEMS: &str = "../../../fbw-aircraft/fbw-a380x/src/systems";

fn fbw_file(rel: &str) -> Option<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FBW_SYSTEMS).join(rel);
    std::fs::read_to_string(path).ok()
}

fn count_entries(text: &str, indent: usize) -> usize {
    let pad = " ".repeat(indent);
    text.lines()
        .filter(|l| {
            let Some(rest) = l.strip_prefix(pad.as_str()) else { return false };
            let rest = rest.trim_start_matches('\'');
            let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
            if digits == 0 || rest.starts_with(' ') {
                return false;
            }
            let after = rest[digits..].trim_start_matches('\'');
            after.starts_with(": {")
        })
        .count()
}

fn fws(rel: &str) -> Option<String> {
    fbw_file(&format!("systems-host/CpiomC/FlightWarningSystem/{rel}"))
}

#[test]
fn table_sizes() {
    assert_eq!(NORMAL_PROCEDURES.len(), 14);
    assert_eq!(NORMAL_PROCEDURES.iter().map(|p| p.items.len()).sum::<usize>(), 42);
    assert_eq!(DEFERRED_PROCEDURE_IDS, &[1000007, 1000008, 1000009, 1000011]);
    assert_eq!(MEMO_MESSAGES.len(), 132);
    assert_eq!(INFO_MESSAGES.len(), 51);
    assert_eq!(LIMITATION_MESSAGES.len(), 34);
    assert_eq!(INOP_SYS_MESSAGES.len(), 337);
    assert_eq!(PFD_MEMO_DISPLAY.len(), 6);
    assert_eq!(EWD_MEMOS.len(), 52);
    assert_eq!(TO_LDG_MEMOS.len(), 2);
    assert_eq!(LIMITATION_RULES.len(), 10);
    assert_eq!(INFO_RULES.len(), 9);
    assert_eq!(INOP_SYS_RULES.len(), 119);
    assert_eq!((WD_NUM_LINES, WD_LINE_CHARACTERS), (18, 41));
}

#[test]
fn counts_match_fbw_source_when_present() {
    let mut checked = 0;
    if let Some(t) = fbw_file("instruments/src/MsfsAvionicsCommon/EcamMessages/NormalProcedures.ts") {
        assert_eq!(count_entries(&t, 2), NORMAL_PROCEDURES.len());
        checked += 1;
    }
    if let Some(t) = fws("FwsMemos.ts") {
        assert_eq!(count_entries(&t, 4), EWD_MEMOS.len() + TO_LDG_MEMOS.len());
        checked += 1;
    }
    if let Some(t) = fws("FwsLimitations.ts") {
        assert_eq!(count_entries(&t, 4), LIMITATION_RULES.len());
        checked += 1;
    }
    if let Some(t) = fws("FwsInformation.ts") {
        assert_eq!(count_entries(&t, 4), INFO_RULES.len());
        checked += 1;
    }
    if let Some(t) = fws("FwsInopSys.ts") {
        assert_eq!(count_entries(&t, 4), INOP_SYS_RULES.len());
        checked += 1;
    }
    if let Some(t) = fws("FwsNormalChecklists.ts") {
        assert_eq!(count_entries(&t, 4), NORMAL_PROCEDURES.len());
        checked += 1;
    }
    if let Some(t) = fbw_file("instruments/src/MsfsAvionicsCommon/EcamMessages/index.ts") {
        let section = |name: &str| {
            let start = t.find(&format!("export const {name}")).unwrap();
            let rest = &t[start..];
            let end = rest.find("\n};").unwrap();
            rest[..end].lines().filter(|l| l.starts_with("  ") && l.contains(": '")).count()
        };
        assert_eq!(section("EcamMemos"), MEMO_MESSAGES.len());
        assert_eq!(section("EcamInfos"), INFO_MESSAGES.len());
        assert_eq!(section("EcamLimitations"), LIMITATION_MESSAGES.len());
        assert_eq!(section("EcamInopSys"), INOP_SYS_MESSAGES.len());
        checked += 1;
    }
    eprintln!("compared {checked} FBW source files");
}

#[test]
fn every_procedure_has_one_check_per_item() {
    for p in NORMAL_PROCEDURES {
        if p.deferred {
            assert!(p.items.is_empty(), "{}", p.id);
            assert_eq!(p.checks.len(), 1);
        } else {
            assert_eq!(p.items.len(), p.checks.len(), "{}", p.id);
        }
        for (item, check) in p.items.iter().zip(p.checks) {
            if item.sensed {
                assert!(check.is_some(), "{} {}", p.id, item.name);
            }
        }
    }
}

#[test]
fn rule_ids_have_texts() {
    for r in LIMITATION_RULES {
        assert!(limitation_message(r.id).is_some(), "{}", r.id);
    }
    for r in INFO_RULES {
        assert!(info_message(r.id).is_some(), "{}", r.id);
    }
    for r in INOP_SYS_RULES {
        assert!(inop_sys_message(r.id).is_some(), "{}", r.id);
        for other in r.not_active_when_item_active {
            assert!(INOP_SYS_RULES.iter().any(|o| o.id == *other), "{} -> {}", r.id, other);
        }
    }
    for r in EWD_MEMOS.iter().chain(TO_LDG_MEMOS) {
        assert!(!r.codes.is_empty());
        for c in r.codes {
            assert!(memo_message(c).is_some(), "{} {}", r.key, c);
        }
    }
    let orphans: Vec<_> = PFD_MEMO_DISPLAY.iter().filter(|id| memo_message(id).is_none()).collect();
    assert_eq!(orphans, [&"000006002"]);
    assert!(memo_message(A_THR_OFF_MEMO_KEY).is_some());
}

#[test]
fn no_unrecognised_selection_or_inhibit() {
    for r in EWD_MEMOS.iter().chain(TO_LDG_MEMOS) {
        assert_ne!(r.select, MemoSelect::Custom, "{}", r.key);
        assert_ne!(r.inhibit, MemoInhibit::Custom, "{}", r.key);
    }
}

#[test]
fn takeoff_memo_items() {
    let r = TO_LDG_MEMOS.iter().find(|r| r.key == "0000010").unwrap();
    assert!(r.left_side);
    assert_eq!(r.flight_phase_inhib, &[1, 3, 8, 12]);
    assert_eq!(r.select, MemoSelect::ToMemo);
    assert_eq!(r.active_inputs, &["fws.toMemo"]);
    let plain: Vec<String> = r.codes.iter().map(|c| memo_message(c).unwrap().plain()).collect();
    assert_eq!(
        plain,
        [
            "T.O",
            "-SEAT BELTS ....ON",
            "-SEAT BELTS ON",
            "-GND SPLRs ....ARM",
            "-GND SPLRs ARM",
            "-FLAPS ........T.O",
            "-FLAPS : T.O",
            "-AUTO BRK .....RTO",
            "-AUTO BRK RTO",
            "-T.O CONFIG ..TEST",
            "-T.O CONFIG NORM",
        ]
    );
    assert_eq!(memo_message("000001002").unwrap().color(), Some(5));
    assert_eq!(memo_message("000001003").unwrap().color(), Some(3));
    let none = to_memo_indices(&ToMemoInputs::default());
    assert_eq!(none, [0, 1, 3, 5, 7, 9]);
    let all = to_memo_indices(&ToMemoInputs {
        no_smoking_memo_and_seat_belt_switch_on: true,
        spoilers_armed: true,
        slat_flap_takeoff_selection: true,
        auto_brake_rto: true,
        to_config_normal: true,
    });
    assert_eq!(all, [0, 2, 4, 6, 8, 10]);
    assert_eq!(r.code_for(all[5]), Some("000001013"));
}

#[test]
fn landing_memo_items() {
    let r = TO_LDG_MEMOS.iter().find(|r| r.key == "0000020").unwrap();
    assert_eq!(r.flight_phase_inhib, &[1, 2, 3, 4, 5, 6, 7, 11, 12]);
    assert_eq!(r.select, MemoSelect::LdgMemo);
    let plain: Vec<String> = r.codes.iter().map(|c| memo_message(c).unwrap().plain()).collect();
    assert_eq!(
        plain,
        [
            "LDG",
            "-SEAT BELTS ....ON",
            "-SEAT BELTS ON",
            "-LDG GEAR ....DOWN",
            "-LDG GEAR DOWN",
            "-GND SPLRs ....ARM",
            "-GND SPLRs ARM",
            "-FLAPS ........LDG",
            "-FLAPS : LDG",
        ]
    );
    let base = LdgMemoInputs { flaps_handle_index: 4, ..Default::default() };
    assert_eq!(ldg_memo_indices(&base), [0, 1, 3, 5, 8]);
    let conf3 = LdgMemoInputs { landing_conf3: true, flaps_handle_index: 4, ..Default::default() };
    assert_eq!(ldg_memo_indices(&conf3)[4], 7);
    let conf3_set = LdgMemoInputs { landing_conf3: true, flaps_handle_index: 3, ..Default::default() };
    assert_eq!(ldg_memo_indices(&conf3_set)[4], 8);
}

#[test]
fn takeoff_and_landing_checklist_items() {
    let taxi = normal_procedure(1000004).unwrap();
    assert_eq!(taxi.title, "TAXI");
    let names: Vec<_> = taxi.items.iter().map(|i| i.name).collect();
    assert_eq!(
        names,
        ["FLIGHT CONTROLS", "FLAPS SETTING", "RADAR", "T.O", "SEAT BELTS", "GND SPLRs", "FLAPS", "AUTO BRK", "T.O CONFIG"]
    );
    assert_eq!(taxi.items[3].style, Some(LineStyle::SubHeadline));
    assert_eq!(taxi.items[8].level, 1);
    assert_eq!(
        taxi.items[8].kind,
        ItemKind::Action { label_not_completed: "TEST", label_completed: Some("NORM"), colon_if_completed: false }
    );
    assert_eq!(taxi.checks[8].unwrap().inputs, &["fws.toConfigNormal"]);
    assert_eq!(taxi.checks[3].unwrap().expr, "false");
    let landing = normal_procedure(1000012).unwrap();
    assert_eq!(landing.items.len(), 5);
    assert_eq!(landing.checks[2].unwrap().inputs, &["fws.isAllGearDownlocked"]);
    assert!(normal_procedure(1000006).unwrap().only_activated_by_request);
    assert_eq!(normal_procedure(1000006).unwrap().title, "<<DEPARTURE CHANGE>>");
    assert!(normal_procedure(1000007).unwrap().deferred);
    assert!(normal_procedure(1999999).is_none());
}

#[test]
fn limitations_and_status() {
    let r = LIMITATION_RULES.iter().find(|r| r.id == "240400002").unwrap();
    assert_eq!(r.phase, RulePhase::AllPhases);
    assert!(r.pfd);
    assert_eq!(r.active_inputs, &["fws.directLawCondition"]);
    assert_eq!(limitation_message("240400002").unwrap().plain(), "MAX SPEED: 310/.86");
    assert_eq!(limitation_message("240400002").unwrap().color(), Some(5));
    let l = LIMITATION_RULES.iter().find(|r| r.id == "290400001").unwrap();
    assert_eq!(l.phase, RulePhase::ApprLdg);
    assert_eq!(limitation_message("290400001").unwrap().plain(), "SLATS SLOW");
    let alt = LIMITATION_RULES.iter().find(|r| r.id == "270400002").unwrap();
    assert_eq!(alt.active_inputs, &["fws.altnLawCondition", "fws.altn1ALawCondition"]);

    let info = INFO_RULES.iter().find(|r| r.id == "340200004").unwrap();
    assert_eq!(info_message(info.id).unwrap().plain(), "DIRECT LAW : PROT LOST");
    let bkup = INFO_RULES.iter().find(|r| r.id == "270200003").unwrap();
    assert_eq!(bkup.active_inputs.len(), 3);

    let fms1 = INOP_SYS_RULES.iter().find(|r| r.id == "221300004").unwrap();
    assert_eq!(fms1.not_active_when_item_active, &["221300006"]);
    assert_eq!(inop_sys_message("221300004").unwrap().plain(), "FMS 1");
    let fmc = INOP_SYS_RULES.iter().find(|r| r.id == "221300001").unwrap();
    assert!(fmc.redundancy_loss);
    let autoland = INOP_SYS_RULES.iter().find(|r| r.id == "220300026").unwrap();
    assert_eq!(autoland.phase, RulePhase::ApprLdg);
    assert_eq!(autoland.active_inputs, &["fws.land2Inop"]);
}

#[test]
fn memo_rules_spot_checks() {
    let rmp = EWD_MEMOS.iter().find(|r| r.key == "230000001").unwrap();
    assert_eq!(rmp.active_inputs, &["fws.rmp1Off", "fws.rmp2Off", "fws.rmp3Off"]);
    let spl = EWD_MEMOS.iter().find(|r| r.key == "271000001").unwrap();
    assert_eq!(spl.inhibit, MemoInhibit::ToOrLdgMemoActive);
    let ext = EWD_MEMOS.iter().find(|r| r.key == "241000001").unwrap();
    assert_eq!(ext.select, MemoSelect::MoreThanOneEngineRunning { if_true: 0, if_false: 1 });
    assert_eq!(ext.codes, &["241000001", "241000002"]);
    assert_eq!(ext.flight_phase_inhib, &[3, 4, 5, 6, 7, 8]);
    let rat = EWD_MEMOS.iter().find(|r| r.key == "242000001").unwrap();
    assert_eq!(rat.select, MemoSelect::Ternary { input: "fws.flightPhase1211", if_true: 0, if_false: 1 });
    let refuel = EWD_MEMOS.iter().find(|r| r.key == "280000010").unwrap();
    assert_eq!(refuel.select, MemoSelect::Ternary { input: "fws.oneEngineRunning", if_true: 1, if_false: 0 });
    let oxy = EWD_MEMOS.iter().find(|r| r.key == "350000001").unwrap();
    assert_eq!(oxy.select, MemoSelect::OxyPaxSys);
    assert_eq!(oxy.codes, &["350000001", "350000002"]);
    assert_eq!(oxy_pax_sys_code(false, true), "350000001");
    assert_eq!(oxy_pax_sys_code(true, true), "350000002");
    let ir = EWD_MEMOS.iter().find(|r| r.key == "340003001").unwrap();
    assert_eq!(ir.select, MemoSelect::AdirsAlignOver3Min);
    assert_eq!(ir.codes.len(), 8);
    assert!(ir.phase_inhibited(3) && !ir.phase_inhibited(1));
}

#[test]
fn selection_helpers() {
    assert_eq!(adirs_message_over_3_min(500.0, false), 0);
    assert_eq!(adirs_message_over_3_min(500.0, true), 1);
    assert_eq!(adirs_message_over_3_min(360.0, false), 2);
    assert_eq!(adirs_message_over_3_min(240.0, true), 7);
    assert_eq!(adirs_message_under_3_min(180.0, false), 0);
    assert_eq!(adirs_message_under_3_min(61.0, true), 3);
    assert_eq!(adirs_message_under_3_min(1.0, true), 5);
    assert_eq!(adirs_message_under_3_min(0.0, true), 0);
    assert_eq!(message_priority(None), 10);
    assert_eq!(message_priority(Some(" \u{1b}<6mT.O INHIBIT")), 0);
    assert_eq!(message_priority(Some("\u{1b}<3mX")), 3);
    assert_eq!(memo_message("314000001").unwrap().priority(), 0);
    assert_eq!(memo_message("242000001").unwrap().priority(), 2);

    let mut limits = vec!["240400002", "2", "1", "270400001"];
    order_limitations(&mut limits);
    assert_eq!(limits, ["1", "240400002", "270400001"]);
}

#[test]
fn message_parsing() {
    let m = memo_message("000001001").unwrap();
    assert_eq!(m.color(), Some(7));
    assert_eq!(m.plain(), "T.O");
    assert_eq!(info_message("340200011").unwrap().plain(), "\u{a0}\u{a0}PITCH TRGT : 3\u{b0}");
    assert_eq!(pfd_shows_memo("220000001"), true);
    assert_eq!(pfd_shows_memo("210000001"), false);
}

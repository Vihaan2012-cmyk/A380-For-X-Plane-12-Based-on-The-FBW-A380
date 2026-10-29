use super::*;
use std::collections::HashSet;

fn count_in(ata: AtaFile) -> usize {
    procedures_in(ata).count()
}

fn deferred_in(ata: AtaFile) -> usize {
    DEFERRED.iter().filter(|d| d.ata == ata).count()
}

#[test]
fn counts_per_source_file_match_fbw_tables() {
    assert_eq!(EXPECTED_COUNTS.len(), AtaFile::ALL.len());
    for expected in EXPECTED_COUNTS {
        assert_eq!(count_in(expected.ata), expected.procedures, "{}", expected.ata.file_name());
        assert_eq!(deferred_in(expected.ata), expected.deferred, "{}", expected.ata.file_name());
    }
    assert_eq!(PROCEDURES.len(), MERGED_PROCEDURES);
    assert_eq!(DEFERRED.len(), MERGED_DEFERRED);
    assert_eq!(ALERTS.len(), FWS_ALERTS);
    assert_eq!(PROCEDURES.len(), EXPECTED_COUNTS.iter().map(|e| e.procedures).sum::<usize>());
}

#[test]
fn counts_per_ata_chapter() {
    let chapter = |c: u32| procedures_in_chapter(c).count();
    assert_eq!(chapter(21) + chapter(22) + chapter(23), count_in(AtaFile::Ata212223));
    assert_eq!(chapter(24), count_in(AtaFile::Ata24));
    assert_eq!(chapter(26), count_in(AtaFile::Ata26));
    assert_eq!(chapter(27), count_in(AtaFile::Ata27));
    assert_eq!(chapter(28), count_in(AtaFile::Ata28));
    assert_eq!(chapter(29) + chapter(30), count_in(AtaFile::Ata2930));
    assert_eq!(chapter(31) + chapter(32) + chapter(33), count_in(AtaFile::Ata313233));
    assert_eq!(chapter(34), count_in(AtaFile::Ata34));
    assert_eq!(chapter(35) + chapter(36) + chapter(42), 0);
    assert_eq!(chapter(46) + chapter(49) + chapter(52) + chapter(56), count_in(AtaFile::Ata46495256));
    assert_eq!(chapter(70), count_in(AtaFile::Ata70));
    assert_eq!(chapter(99), count_in(AtaFile::Ata80Rest) + count_in(AtaFile::SecondaryFailures));
}

#[test]
fn tables_are_sorted_and_unique() {
    assert!(PROCEDURES.windows(2).all(|w| w[0].id < w[1].id));
    assert!(ALERTS.windows(2).all(|w| w[0].id < w[1].id));
    assert!(DEFERRED.windows(2).all(|w| w[0].id < w[1].id));
}

#[test]
fn non_sensed_overview_matches_procedures() {
    assert_eq!(NON_SENSED.len(), 31);
    let ids: HashSet<u32> = NON_SENSED.iter().map(|n| n.id).collect();
    assert_eq!(ids.len(), NON_SENSED.len());
    for n in NON_SENSED {
        let p = procedure(n.id).unwrap_or_else(|| panic!("non-sensed {} has no procedure", n.id));
        assert_eq!(p.kind, Kind::NonSensed, "{}", n.id);
    }
    assert_eq!(PROCEDURES.iter().filter(|p| p.kind == Kind::NonSensed).count(), NON_SENSED.len());
    assert_eq!(PROCEDURES.iter().filter(|p| p.kind == Kind::Secondary).count(), 7);
    assert_eq!(SUB_MENU.len(), 6);
}

#[test]
fn alerts_refer_to_procedures_and_suppressors() {
    let mut dangling = Vec::new();
    for a in ALERTS {
        match a.source {
            AlertSource::Deferred => assert!(deferred(a.id).is_some(), "deferred alert {} has no deferred procedure", a.id),
            _ => assert!(a.non_procedure_key || procedure(a.id).is_some(), "alert {} has no procedure", a.id),
        }
        for s in a.suppressed_by {
            if alert(*s).is_none() {
                dangling.push((a.id, *s));
            }
        }
        for list in [a.inop_sys_all_phases, a.inop_sys_appr_ldg, a.redund_loss, a.limitations_all_phases, a.limitations_appr_ldg, a.limitations_pfd, a.info] {
            assert!(list.ids.iter().all(|i| *i > 100_000_000 && *i < 1_000_000_000));
        }
    }
    println!("dangling suppressors: {:?}", dangling);
    assert_eq!(dangling, DANGLING_SUPPRESSORS);
}

const DANGLING_SUPPRESSORS: &[(u32, u32)] = &[
    (314800006, 314800004),
    (314800008, 314800003),
    (314800008, 314800004),
    (314800009, 314800003),
    (314800009, 314800004),
    (340800008, 340800010),
    (340800008, 340800071),
];
const LENGTH_MISMATCHES: &[u32] = &[990900011];

#[test]
fn item_function_lengths_match_procedure_items() {
    let mut mismatches = Vec::new();
    for a in ALERTS.iter().filter(|a| a.source != AlertSource::Deferred && !a.non_procedure_key) {
        let p = procedure(a.id).unwrap();
        for f in [a.items_to_show, a.items_checked, a.items_active] {
            if let ItemsFn::Array { len: Some(n), .. } = f {
                if n as usize != p.items.len() && !mismatches.contains(&a.id) {
                    mismatches.push(a.id);
                }
            }
        }
    }
    println!("length mismatches: {:?}", mismatches);
    assert_eq!(mismatches, LENGTH_MISMATCHES);
}

#[test]
fn deferred_procedures_reference_abnormal_ones() {
    assert_eq!(DEFERRED.len(), 8);
    for d in DEFERRED {
        assert!(!d.from_abnormal.is_empty());
        for id in d.from_abnormal {
            assert!(procedure(*id).is_some(), "deferred {} from unknown {}", d.id, id);
        }
    }
    let rudder = deferred(270700001).unwrap();
    assert_eq!(rudder.kind, DeferredType::ForApproach);
    assert_eq!(rudder.from_abnormal, &[270900002]);
    assert_eq!(rudder.title, Some("RUDDER TRIM RUNAWAY"));
    assert_eq!(rudder.items.len(), 5);
    assert_eq!(rudder.items[1].kind, ItemKind::Condition);
    assert_eq!(rudder.items[2].level, 1);
}

#[test]
fn spot_check_engine_1_fire() {
    let p = procedure(260800005).unwrap();
    assert_eq!(p.plain_title, "ENG 1 FIRE");
    assert_eq!(p.system, "ENG 1 FIRE");
    assert_eq!(p.colour, Colour::Red);
    assert_eq!(p.kind, Kind::Sensed);
    assert_eq!(p.recommendation, Some(Recommendation::LandAsap));
    assert_eq!(p.chapter(), 26);
    let names: Vec<&str> = p.items.iter().take(4).map(|i| i.name).collect();
    assert_eq!(names, ["THR LEVER 1", "ENG 1 MASTER", "ENG 1 FIRE P/B", "APU BLEED"]);
    assert_eq!(p.items[0].label, "IDLE");
    assert_eq!(p.items[0].kind, ItemKind::Action);
    assert!(p.items[0].sensed);

    let a = alert(260800005).unwrap();
    assert_eq!(a.level, Level::Warning);
    assert_eq!(a.sd_page, SdPage::Eng);
    assert_eq!(a.comment, "ENG 1 FIRE (IN FLIGHT)");
    assert_eq!(a.phases_inhibited.list(), vec![5, 6]);
    assert!(a.phases_inhibited.contains(5) && !a.phases_inhibited.contains(4));
    assert_eq!(a.aural, Aural::Signal { name: "eng1FireDetectedAural", on: AuralKind::Crc, off: AuralKind::None });
    assert_eq!(a.active_refs, &["aircraftOnGround", "eng1FireDetected"]);
    assert_eq!(a.items_active, ItemsFn::Absent);
    assert!(matches!(a.items_to_show, ItemsFn::Array { len: Some(n), .. } if n as usize == p.items.len()));
}

#[test]
fn spot_check_hydraulic_green_low_pressure() {
    let p = procedure(290800035).unwrap();
    assert_eq!(p.plain_title, "HYD G SYS PRESS LO");
    assert_eq!(p.system, "HYD");
    assert_eq!(p.colour, Colour::Amber);
    assert_eq!(p.recommendation, None);
    assert_eq!(p.items.len(), 8);
    assert_eq!(p.items[0].name, "FOR TAXI : FLAP LVR 1");
    let fms = p.items.iter().find(|i| i.name == "FMS PRED UNRELIABLE").unwrap();
    assert_eq!(fms.style, Style::ChecklistCondition);
    assert!(!fms.sensed);

    let a = alert(290800035).unwrap();
    assert_eq!(a.level, Level::Caution);
    assert_eq!(a.sd_page, SdPage::Hyd);
    assert_eq!(a.aural, Aural::Default);
    assert_eq!(a.active_expr, "this.fws.greenAbnormLoPressure");
    assert_eq!(a.active_refs, &["greenAbnormLoPressure"]);
    assert_eq!(a.suppressed_by, &[290800039]);
    assert_eq!(a.phases_inhibited.list(), vec![4, 5, 6, 7, 9, 10]);
    assert_eq!(a.inop_sys_all_phases.ids, &[290100001, 320300023, 290300021]);
    assert!(a.limitations_all_phases.conditional);
    assert!(!a.limitations_pfd.conditional);
    assert_eq!(a.limitations_pfd.ids, &[320400001]);
    assert!(alert(290800039).is_some());
}

#[test]
fn spot_check_gear_not_downlocked() {
    let p = procedure(320800039).unwrap();
    assert_eq!(p.plain_title, "L/G GEAR NOT LOCKED DOWN");
    assert_eq!(p.colour, Colour::Red);
    assert_eq!(p.items[0].name, "L/G (AFTER 60 S)");
    assert!(procedure(320800038).is_some());

    let no_cancel = alert(320800037).unwrap();
    assert_eq!(no_cancel.level, Level::Warning);
    assert_eq!(no_cancel.sd_page, SdPage::Wheel);
    assert!(!no_cancel.cancel);
    assert_eq!(no_cancel.active_refs, &["lgNotDownNoCancel"]);
    assert_eq!(no_cancel.phases_inhibited.list(), vec![1, 2, 3, 4, 5, 6, 7, 10, 11, 12]);
    let cancel = alert(320800038).unwrap();
    assert!(cancel.cancel);
    assert_eq!(cancel.active_refs, &["lgNotDown", "lgNotDownNoCancel"]);
}

#[test]
fn alert_levels_and_pages() {
    assert_eq!(Level::from_failure(-1), Level::Hidden);
    assert!(Level::Warning > Level::Caution && Level::Caution > Level::Advisory);
    assert_eq!(alert(290800040).unwrap().level, Level::Hidden);
    assert_eq!(alert(701800029).unwrap().phases_inhibited.list(), vec![5, 6]);
    assert_eq!(SdPage::None.value(), -1);
    assert_eq!(SdPage::Wheel.value(), 9);
    let warnings = ALERTS.iter().filter(|a| a.level == Level::Warning).count();
    assert!(warnings > 20);
}

#[test]
fn titles_carry_fbw_escape_codes() {
    for p in PROCEDURES {
        assert!(p.title.contains('\u{1b}'), "{}", p.id);
        assert!(!p.plain_title.contains('\u{1b}'), "{}", p.id);
    }
    assert_eq!(procedure(260800001).unwrap().title, "\u{1b}<2m\u{1b}4mAPU FIRE");
}

#[test]
fn timed_items_keep_their_seconds() {
    let apu_agent = procedure(260800001).unwrap().items.iter().find(|i| i.name == "APU AGENT").unwrap();
    assert_eq!(apu_agent.time, Some(30));
    assert_eq!(apu_agent.label, "DISCH");
}

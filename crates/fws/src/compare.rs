use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use crate::core::Fws;
use crate::vars::{Out, Vars};

pub const STEP_MS: f64 = 50.;

pub const FWS_OWNED_PREFIXES: [&str; 10] = [
    "A32NX_FWC_",
    "A32NX_FWS",
    "A32NX_MASTER_",
    "A32NX_ECAM_SFAIL",
    "A32NX_ECAM_FAILURE_ACTIVE",
    "A32NX_ECAM_SD_PAGE_TO_SHOW",
    "A32NX_STATUS_",
    "A32NX_TO_CONFIG",
    "A32NX_NO_SMOKING",
    "A32NX_CABIN_READY",
];

/// Vars written earlier in the frame than the FWS update (Xpndr1, and the GPWS, which SystemsHost updates just
/// before the FWS); every other recorded write lands after it and is seen one frame later.
pub const SAME_FRAME_INPUTS: [&str; 5] = ["TRANSPONDER STATE:1", "L:A32NX_GPWS_", "L:A32NX_TERR_", "L:A32NX_EGPWS_", "L:A32NX_TAWS_"];

fn same_frame(name: &str) -> bool {
    SAME_FRAME_INPUTS.iter().any(|p| name.starts_with(p))
}

pub const COVERED: [&str; 32] = [
    "L:A32NX_FWS1_IS_HEALTHY",
    "L:A32NX_FWS2_IS_HEALTHY",
    "L:A32NX_FWS_ECP_FAILED",
    "L:A32NX_FWC_FLIGHT_PHASE",
    "L:A32NX_FWC_TOMEMO",
    "L:A32NX_FWC_LDGMEMO",
    "L:A32NX_NO_SMOKING_MEMO",
    "L:A32NX_CABIN_READY",
    "L:A32NX_FWS_AUDIO_VOLUME",
    "L:A32NX_EWD_LOWER_LEFT_LINE_",
    "L:A32NX_EWD_LOWER_RIGHT_LINE_",
    "L:A32NX_PFD_MEMO_LINE_",
    "L:A32NX_PFD_LIMITATIONS_LINE_",
    "L:A32NX_MASTER_CAUTION",
    "L:A32NX_MASTER_WARNING",
    "L:A32NX_ECAM_SFAIL",
    "L:A32NX_ECAM_FAILURE_ACTIVE",
    "L:A32NX_STATUS_NORMAL",
    "L:A32NX_ECAM_SD_PAGE_TO_SHOW",
    "L:A32NX_ECAM_SD_CURRENT_PAGE_INDEX",
    "L:A32NX_FWC_CRC",
    "L:A32NX_FWC_SC",
    "L:A32NX_FWC_CAVALRY_CHARGE",
    "L:A32NX_FMA_TRIPLE_CLICK",
    "L:A32NX_AUDIO_",
    "L:A32NX_ALT_DEVIATION",
    "L:A32NX_FWC_1_LG_RED_ARROW",
    "L:A32NX_FWC_2_LG_RED_ARROW",
    "L:A32NX_FWC_1_DISCRETE_WORD_126",
    "L:A32NX_FWC_2_DISCRETE_WORD_126",
    "L:A32NX_FWC_1_ABN_PROC_IMPACT_",
    "L:A32NX_TO_CONFIG_NORMAL",
];

/// Bus topics FwsCore publishes that the oracle records.
pub const COVERED_TOPICS: [&str; 22] = [
    "fws_abn_non_sensed_id",
    "fws_abn_non_sensed_current_active",
    "fws_abn_sensed_procedures",
    "fws_active_item",
    "fws_active_procedure",
    "fws_alerts_impacting_ldg_perf",
    "fws_cancelled_caution",
    "fws_deferred_procedures",
    "fws_information",
    "fws_inop_sys_all_phases",
    "fws_inop_sys_appr_ldg",
    "fws_inop_sys_redundancy_loss",
    "fws_limitations_all_phases",
    "fws_limitations_appr_ldg",
    "fws_normal_checklists",
    "fws_normal_checklists_id",
    "fws_show_abn_non_sensed",
    "fws_show_abn_sensed",
    "fws_show_failure_pending",
    "fws_show_from_line",
    "fws_show_normal_checklists",
    "fws_show_sts_indication",
];

#[derive(Default)]
pub struct Store {
    vars: HashMap<String, f64>,
}

impl Store {
    pub fn put(&mut self, name: &str, value: f64) {
        if name.starts_with("L:") {
            self.vars.insert(name.to_string(), value);
        } else {
            let bare = name.strip_prefix("A:").unwrap_or(name);
            self.vars.insert(bare.to_string(), value);
            self.vars.insert(format!("A:{bare}"), value);
        }
    }

    pub fn apply_state(&mut self, state: &serde_json::Value) -> usize {
        let mut applied = 0;
        for item in state["vars"].as_array().into_iter().flatten() {
            let (Some(is_l), Some(name), Some(value)) = (item[0].as_bool(), item[1].as_str(), item[2].as_f64()) else {
                continue;
            };
            if is_l {
                if FWS_OWNED_PREFIXES.iter().any(|p| name.starts_with(p)) {
                    continue;
                }
                self.put(&format!("L:{name}"), value);
            } else {
                self.put(name, value);
            }
            applied += 1;
        }
        applied
    }
}

impl Vars for Store {
    fn get(&self, name: &str) -> f64 {
        self.vars.get(name).copied().unwrap_or(0.)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rec {
    pub t: f64,
    pub name: String,
    pub value: String,
}

pub fn replay(scenario: &serde_json::Value, states_dir: &Path, refresh_offset_ms: f64) -> Result<Vec<Rec>, String> {
    replay_full(scenario, states_dir, refresh_offset_ms, &[]).map(|r| r.0)
}

pub type Input = (f64, String, f64);

pub fn golden_inputs(golden: &serde_json::Value) -> Vec<Input> {
    let mut out = Vec::new();
    for e in golden["events"].as_array().into_iter().flatten() {
        let Some(name) = e.get("var").and_then(|v| v.as_str()) else { continue };
        let bare = name.strip_prefix("L:").unwrap_or(name);
        if FWS_OWNED_PREFIXES.iter().any(|p| bare.starts_with(p)) {
            continue;
        }
        out.push((e["t"].as_f64().unwrap_or(0.), name.to_string(), e["value"].as_f64().unwrap_or(0.)));
    }
    out
}

pub type ProcChange = (i64, u32, bool);

pub fn replay_full(scenario: &serde_json::Value, states_dir: &Path, refresh_offset_ms: f64, inputs: &[Input]) -> Result<(Vec<Rec>, Vec<ProcChange>), String> {
    replay_all(scenario, states_dir, refresh_offset_ms, inputs, &[]).map(|r| (r.0, r.1))
}

/// Sounds other SystemsHost computers (the GPWS) enqueue: the oracle does not record that channel, so each callout
/// the golden plays is enqueued 150 ms (at least one FWS cycle) before it plays.
pub fn golden_sound_inputs(golden: &serde_json::Value) -> Vec<(f64, String)> {
    let mut out = Vec::new();
    for e in golden["events"].as_array().into_iter().flatten() {
        if e.get("call").and_then(|v| v.as_str()) != Some("PLAY_INSTRUMENT_SOUND") {
            continue;
        }
        let args: serde_json::Value = serde_json::from_str(e["args"].as_str().unwrap_or("[]")).unwrap_or_default();
        if let Some(key) = args[0].as_str().and_then(crate::sound::key_for_event) {
            out.push((e["t"].as_f64().unwrap_or(0.) - 150., key.to_string()));
        }
    }
    out
}

pub fn replay_all(
    scenario: &serde_json::Value,
    states_dir: &Path,
    refresh_offset_ms: f64,
    inputs: &[Input],
    sounds: &[(f64, String)],
) -> Result<(Vec<Rec>, Vec<ProcChange>, Vec<Rec>), String> {
    let mut next_sound = 0;
    let mut topics: Vec<Rec> = Vec::new();
    let mut next_input = 0;
    let mut procs: Vec<ProcChange> = Vec::new();
    let mut current: Vec<u32> = Vec::new();
    let mut store = Store::default();
    let mut fws = Fws::new(refresh_offset_ms);
    let mut now = 0.;
    let mut out = Vec::new();
    for step in scenario["steps"].as_array().ok_or("steps")? {
        if let Some(name) = step.get("state").and_then(|v| v.as_str()) {
            let text = std::fs::read_to_string(states_dir.join(format!("{name}.json"))).map_err(|e| format!("{name}: {e}"))?;
            let state: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{name}: {e}"))?;
            store.apply_state(&state);
        } else if let Some(set) = step.get("set").and_then(|v| v.as_object()) {
            for (k, v) in set {
                store.put(k, v.as_f64().ok_or("set value")?);
            }
        } else if let Some(ms) = step.get("run_ms").and_then(|v| v.as_f64()) {
            for _ in 0..(ms / STEP_MS).ceil() as usize {
                while next_input < inputs.len() && inputs[next_input].0 < now {
                    if !(inputs[next_input].0 == now - STEP_MS && same_frame(&inputs[next_input].1)) {
                        store.put(&inputs[next_input].1, inputs[next_input].2);
                    }
                    next_input += 1;
                }
                for i in &inputs[next_input..] {
                    if i.0 > now {
                        break;
                    }
                    if same_frame(&i.1) {
                        store.put(&i.1, i.2);
                    }
                }
                while next_sound < sounds.len() && sounds[next_sound].0 <= now {
                    fws.enqueue_sound(&sounds[next_sound].1);
                    next_sound += 1;
                }
                for e in fws.tick(&store, now) {
                    let (name, value) = match e.out {
                        Out::Var(n, v) => (n, format!("{v}")),
                        Out::Str(n, s) => (n, s),
                    };
                    out.push(Rec { t: e.t, name, value });
                }
                for (t, e) in fws.take_extra() {
                    match e {
                        crate::fwc::Emit::Topic(name, value) => topics.push(Rec { t, name, value: value.to_string() }),
                        crate::fwc::Emit::Call(name, args) => topics.push(Rec { t, name: format!("call:{name}"), value: args }),
                        _ => {}
                    }
                }
                let next = fws.current_procedures();
                proc_changes(&current, &next, now as i64, &mut procs);
                current = next;
                now += STEP_MS;
            }
        }
    }
    Ok((out, procs, topics))
}

/// The value each covered topic holds at the end of every tick it was published in, minus repeats of the previous
/// tick's value (the recorder and the FWS publish every intermediate value of a tick; only the end state is compared).
pub fn tick_final(recs: &[Rec]) -> Vec<Rec> {
    let mut by_tick: BTreeMap<(i64, String), String> = BTreeMap::new();
    for r in recs.iter().filter(|r| COVERED_TOPICS.contains(&r.name.as_str()) || r.name.starts_with("call:")) {
        by_tick.insert((r.t as i64, r.name.clone()), r.value.clone());
    }
    let mut last: BTreeMap<String, String> = BTreeMap::new();
    let mut out = Vec::new();
    for ((t, name), value) in by_tick {
        if !name.starts_with("call:") && last.get(&name) == Some(&value) {
            continue;
        }
        last.insert(name.clone(), value.clone());
        out.push(Rec { t: t as f64, name, value });
    }
    out
}

pub fn golden_topics(golden: &serde_json::Value) -> Vec<Rec> {
    let mut out = Vec::new();
    for e in golden["events"].as_array().into_iter().flatten() {
        let t = e["t"].as_f64().unwrap_or(0.);
        if let Some(n) = e.get("topic").and_then(|v| v.as_str()) {
            out.push(Rec { t, name: n.to_string(), value: e["data"].to_string() });
        } else if e.get("call").and_then(|v| v.as_str()) == Some("PLAY_INSTRUMENT_SOUND") {
            out.push(Rec { t, name: "call:PLAY_INSTRUMENT_SOUND".into(), value: e["args"].as_str().unwrap_or("").to_string() });
        }
    }
    out
}

pub fn topic_diff(golden: &[Rec], ours: &[Rec]) -> Report {
    let g = tick_final(golden);
    let o = tick_final(ours);
    let mut report = Report::default();
    for r in &g {
        if o.contains(r) {
            report.matching += 1;
        } else {
            report.golden_only.push(r.clone());
        }
    }
    report.ours_only = o.into_iter().filter(|r| !g.contains(r)).collect();
    report
}

fn proc_changes(before: &[u32], after: &[u32], t: i64, out: &mut Vec<ProcChange>) {
    for id in after.iter().filter(|id| !before.contains(id)) {
        out.push((t, *id, true));
    }
    for id in before.iter().filter(|id| !after.contains(id)) {
        out.push((t, *id, false));
    }
}

pub fn golden_procedures(golden: &serde_json::Value) -> Vec<ProcChange> {
    let mut by_time: BTreeMap<i64, Vec<u32>> = BTreeMap::new();
    for e in golden["events"].as_array().into_iter().flatten() {
        if e.get("topic").and_then(|v| v.as_str()) != Some("fws_abn_sensed_procedures") {
            continue;
        }
        let data = match &e["data"] {
            serde_json::Value::String(s) => serde_json::from_str(s).unwrap_or(serde_json::Value::Null),
            v => v.clone(),
        };
        let ids = data.as_array().into_iter().flatten().filter_map(|p| p["id"].as_str()?.parse().ok()).collect();
        by_time.insert(e["t"].as_f64().unwrap_or(0.) as i64, ids);
    }
    let mut out = Vec::new();
    let mut current: Vec<u32> = Vec::new();
    for (t, ids) in by_time {
        proc_changes(&current, &ids, t, &mut out);
        current = ids;
    }
    out
}

pub fn procedure_diff(golden: &[ProcChange], ours: &[ProcChange], covered: impl Fn(u32) -> bool) -> (usize, Vec<ProcChange>, Vec<ProcChange>) {
    let g: Vec<ProcChange> = golden.iter().copied().filter(|c| covered(c.1)).collect();
    let o: Vec<ProcChange> = ours.iter().copied().filter(|c| covered(c.1)).collect();
    let matching = g.iter().filter(|c| o.contains(c)).count();
    (matching, g.iter().copied().filter(|c| !o.contains(c)).collect(), o.iter().copied().filter(|c| !g.contains(c)).collect())
}

pub fn golden_records(golden: &serde_json::Value) -> Vec<Rec> {
    let mut out = Vec::new();
    for e in golden["events"].as_array().into_iter().flatten() {
        let t = e["t"].as_f64().unwrap_or(0.);
        if let Some(n) = e.get("var").and_then(|v| v.as_str()) {
            out.push(Rec { t, name: n.to_string(), value: format!("{}", e["value"].as_f64().unwrap_or(0.)) });
        } else if let Some(n) = e.get("str").and_then(|v| v.as_str()) {
            out.push(Rec { t, name: n.to_string(), value: e["value"].as_str().unwrap_or("").to_string() });
        }
    }
    out
}

pub fn is_covered(name: &str) -> bool {
    COVERED.iter().any(|c| name.starts_with(c))
}

#[derive(Debug, Default)]
pub struct Report {
    pub matching: usize,
    pub golden_only: Vec<Rec>,
    pub ours_only: Vec<Rec>,
}

pub fn diff(golden: &[Rec], ours: &[Rec]) -> Report {
    let mut report = Report::default();
    let mut remaining: BTreeMap<(i64, String, String), usize> = BTreeMap::new();
    for r in ours {
        *remaining.entry((r.t as i64, r.name.clone(), r.value.clone())).or_default() += 1;
    }
    for g in golden.iter().filter(|g| is_covered(&g.name)) {
        let key = (g.t as i64, g.name.clone(), g.value.clone());
        match remaining.get_mut(&key) {
            Some(n) if *n > 0 => {
                *n -= 1;
                report.matching += 1;
            }
            _ => report.golden_only.push(g.clone()),
        }
    }
    for ((t, name, value), n) in remaining.into_iter().filter(|((_, name, _), _)| is_covered(name)) {
        for _ in 0..n {
            report.ours_only.push(Rec { t: t as f64, name: name.clone(), value: value.clone() });
        }
    }
    report
}

pub fn scenarios_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/fws_oracle/scenarios")
}

pub fn golden_dir() -> PathBuf {
    PathBuf::from(std::env::var("FWS_ORACLE_OUT").unwrap_or_else(|_| r"E:\fbw-build\fws-oracle".into()))
}

pub fn states_dir() -> PathBuf {
    PathBuf::from(std::env::var("FWS_ORACLE_STATES").unwrap_or_else(|_| r"E:\fbw-build\emu-snapshots".into()))
}

pub fn run_all(refresh_offset_ms: f64) -> Vec<(String, Report)> {
    run_all_full(refresh_offset_ms).into_iter().map(|(n, r, _)| (n, r)).collect()
}

pub type ProcReport = (usize, Vec<ProcChange>, Vec<ProcChange>);

pub fn run_all_full(refresh_offset_ms: f64) -> Vec<(String, Report, ProcReport)> {
    run_everything(refresh_offset_ms).into_iter().map(|(n, r, p, _)| (n, r, p)).collect()
}

pub fn run_everything(refresh_offset_ms: f64) -> Vec<(String, Report, ProcReport, Report)> {
    let mut names: Vec<_> = std::fs::read_dir(scenarios_dir())
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect())
        .unwrap_or_default();
    names.sort();
    let mut reports = Vec::new();
    for path in names {
        let scenario: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let name = scenario["name"].as_str().unwrap().to_string();
        let Ok(golden_text) = std::fs::read_to_string(golden_dir().join(format!("{name}.json"))) else { continue };
        let golden: serde_json::Value = serde_json::from_str(&golden_text).unwrap();
        let (ours, procs, topics) = replay_all(&scenario, &states_dir(), refresh_offset_ms, &golden_inputs(&golden), &golden_sound_inputs(&golden)).unwrap();
        let proc_report = procedure_diff(&golden_procedures(&golden), &procs, |_| true);
        if let Ok(dir) = std::env::var("FWS_DUMP") {
            let _ = std::fs::create_dir_all(&dir);
            let lines: Vec<String> = tick_final(&topics).iter().chain(ours.iter()).map(|r| serde_json::json!({"t": r.t, "name": r.name, "value": r.value}).to_string()).collect();
            let _ = std::fs::write(format!("{dir}/{name}.ours.jsonl"), lines.join("
"));
        }
        reports.push((name, diff(&golden_records(&golden), &ours), proc_report, topic_diff(&golden_topics(&golden), &topics)));
    }
    reports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_application_skips_the_fws_owned_variables() {
        let mut s = Store::default();
        let state = serde_json::json!({ "vars": [[true, "A32NX_FWC_FLIGHT_PHASE", 6], [true, "A32NX_FLAPS_HANDLE_INDEX", 2], [false, "GEAR HANDLE POSITION", 1]] });
        assert_eq!(s.apply_state(&state), 2);
        assert_eq!(s.get("L:A32NX_FWC_FLIGHT_PHASE"), 0.);
        assert_eq!(s.get("L:A32NX_FLAPS_HANDLE_INDEX"), 2.);
        assert_eq!(s.get("A:GEAR HANDLE POSITION"), 1.);
    }

    #[test]
    fn diff_counts_matches_and_misses() {
        let rec = |t: f64, n: &str, v: &str| Rec { t, name: n.into(), value: v.into() };
        let golden = vec![rec(1.0, "L:A32NX_FWC_TOMEMO", "0"), rec(2.0, "L:A32NX_FWC_TOMEMO", "1"), rec(2.0, "L:A32NX_TCAS_MODE", "1")];
        let ours = vec![rec(1.0, "L:A32NX_FWC_TOMEMO", "0"), rec(3.0, "L:A32NX_FWC_TOMEMO", "1")];
        let r = diff(&golden, &ours);
        assert_eq!((r.matching, r.golden_only.len(), r.ours_only.len()), (1, 1, 1));
    }

    #[test]
    fn covered_outputs_match_the_recorded_goldens() {
        if !golden_dir().exists() || !states_dir().exists() {
            return;
        }
        for (name, r) in run_all(80.) {
            assert!(r.golden_only.is_empty() && r.ours_only.is_empty(), "{name}: {:?} / {:?}", r.golden_only.first(), r.ours_only.first());
        }
    }

    #[test]
    fn ported_alert_procedures_match_the_recorded_goldens() {
        if !golden_dir().exists() || !states_dir().exists() {
            return;
        }
        for (name, _, (matching, golden_only, ours_only)) in run_all_full(80.) {
            assert!(matching > 0 && golden_only.is_empty() && ours_only.is_empty(), "{name}: {golden_only:?} / {ours_only:?}");
        }
    }

    #[test]
    fn covered_topics_match_the_recorded_goldens() {
        if !golden_dir().exists() || !states_dir().exists() {
            return;
        }
        for (name, _, _, t) in run_everything(80.) {
            assert!(t.golden_only.is_empty() && t.ours_only.is_empty(), "{name}: {:?} / {:?}", t.golden_only.first(), t.ours_only.first());
        }
    }

    #[test]
    fn clear_sends_the_active_procedure_to_recall_and_recall_brings_it_back() {
        if !states_dir().exists() {
            return;
        }
        let text = std::fs::read_to_string(scenarios_dir().join("engine_fire.json")).unwrap();
        let mut scenario: serde_json::Value = serde_json::from_str(&text).unwrap();
        let end: f64 = scenario["steps"].as_array().unwrap().iter().filter_map(|s| s["run_ms"].as_f64()).sum();
        let extra = serde_json::json!([
            {"set": {"L:A32NX_BTN_CLR": 1}}, {"run_ms": 300}, {"set": {"L:A32NX_BTN_CLR": 0}}, {"run_ms": 500},
            {"set": {"L:A32NX_BTN_RCL": 1}}, {"run_ms": 300}, {"set": {"L:A32NX_BTN_RCL": 0}}, {"run_ms": 500}
        ]);
        scenario["steps"].as_array_mut().unwrap().extend(extra.as_array().unwrap().iter().cloned());
        let (_, procs) = replay_full(&scenario, &states_dir(), 80., &[]).unwrap();
        let after: Vec<ProcChange> = procs.into_iter().filter(|c| c.0 as f64 >= end).collect();
        assert_eq!(after.len(), 2, "{after:?}");
        assert!(!after[0].2 && after[1].2 && after[0].1 == after[1].1, "{after:?}");
    }

    #[test]
    fn sts_next_page_h_event_steps_through_the_pages() {
        let mut s = Store::default();
        s.put("L:A32NX_ECAM_SD_STS_NUMBER_OF_PAGES", 3.);
        let mut fws = Fws::new(80.);
        fws.tick(&s, 1000.);
        fws.h_event("A32NX_SD_STS_NEXT_PAGE");
        let out = fws.tick(&s, 1050.);
        assert!(out.iter().any(|e| e.out == Out::Var("L:A32NX_ECAM_SD_STS_PAGE_TO_SHOW".into(), 1.)));
    }

    #[test]
    #[ignore]
    fn replay_goldens_report() {
        for (name, r, (pm, pg, po), t) in run_everything(80.) {
            println!("{name}: topics matching {} golden_only {} ours_only {}", t.matching, t.golden_only.len(), t.ours_only.len());
            for g in t.golden_only.iter().take(4) {
                println!("  topic golden only: {} {} {}", g.t, g.name, &g.value[..g.value.len().min(300)]);
            }
            for g in t.ours_only.iter().take(4) {
                println!("  topic ours only:   {} {} {}", g.t, g.name, &g.value[..g.value.len().min(300)]);
            }
            println!("{name}: ported procedures matching {pm} golden_only {:?} ours_only {:?}", &pg[..pg.len().min(6)], &po[..po.len().min(6)]);
            println!("{name}: matching {} golden_only {} ours_only {}", r.matching, r.golden_only.len(), r.ours_only.len());
            for g in r.golden_only.iter().take(6) {
                println!("  golden only: {} {} {}", g.t, g.name, g.value);
            }
            for g in r.ours_only.iter().take(6) {
                println!("  ours only:   {} {} {}", g.t, g.name, g.value);
            }
        }
    }
}

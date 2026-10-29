use fws_ecl::ecl::*;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

const STEP_MS: f64 = 50.;
const FWS_CYCLE_MS: f64 = 125.;
const SETTLE_MS: f64 = 450.;

fn env_path(var: &str, default: &str) -> PathBuf {
    PathBuf::from(std::env::var(var).unwrap_or_else(|_| default.to_string()))
}

fn load(path: &PathBuf) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

struct Vars(HashMap<String, f64>);

impl Vars {
    fn put(&mut self, name: &str, v: f64) {
        let key = if name.starts_with("L:") || name.starts_with("A:") { name.to_string() } else { format!("A:{name}") };
        self.0.insert(key, v);
    }
    fn get(&self, name: &str) -> f64 {
        self.0.get(name).copied().unwrap_or(0.)
    }
    fn word(&self, name: &str) -> (u64, f32) {
        let raw = self.get(name);
        let ssm = ((raw / 4294967296.).trunc() as u64) & 3;
        let low = (raw as u64 & 0xffff_ffff) as u32;
        (ssm, f32::from_bits(low))
    }
    fn bit(&self, name: &str, bit: u32) -> bool {
        let (ssm, v) = self.word(name);
        (ssm == 3 || ssm == 1) && ((v as i32 >> (bit - 1)) & 1) != 0
    }
    fn value_or0(&self, name: &str) -> f64 {
        let (ssm, v) = self.word(name);
        if ssm == 3 { v as f64 } else { 0. }
    }
}

fn sensed(v: &Vars, to_config_normal: bool) -> SensedInputs {
    let lg = |bit| v.bit("L:A32NX_LGCIU_1_DISCRETE_WORD_1", bit) || v.bit("L:A32NX_LGCIU_2_DISCRETE_WORD_1", bit);
    let trim = if v.get("L:A32NX_SEC_1_HEALTHY") > 0. {
        v.value_or0("L:A32NX_SEC_1_RUDDER_ACTUAL_POSITION")
    } else {
        v.value_or0("L:A32NX_SEC_3_RUDDER_ACTUAL_POSITION")
    };
    let masters = [1, 2, 3, 4].map(|n| v.get(&format!("A:FUELSYSTEM VALVE SWITCH:{n}")) != 0.);
    let pumps_off = (1..=8).all(|n| v.get(&format!("A:FUELSYSTEM PUMP ACTIVE:{n}")) == 0.);
    SensedInputs {
        seat_belt_switch: v.get("A:CABIN SEATBELTS ALERT SWITCH") != 0.,
        beacon_on: v.get("A:LIGHT BEACON") != 0.,
        rudder_trim_deg: trim,
        spoilers_armed: v.bit("L:A32NX_FCDC_1_DISCRETE_WORD_4", 27) || v.bit("L:A32NX_FCDC_2_DISCRETE_WORD_4", 27),
        flaps_handle: v.get("L:A32NX_FLAPS_HANDLE_INDEX") as i32,
        auto_brake_mode: v.get("L:A32NX_AUTOBRAKES_ARMED_MODE") as i32,
        to_config_normal,
        all_gear_downlocked: lg(23) && lg(24) && lg(25),
        landing_conf3: v.get("L:A380X_FM_LANDING_CONF3") != 0.,
        engine_master: masters,
        all_fuel_pumps_off: pumps_off && masters.iter().all(|m| !m),
        crew_oxygen_pb: v.get("L:PUSH_OVHD_OXYGEN_CREW") != 0.,
        emer_exit_light_position: v.get("L:XMLVAR_SWITCH_OVHD_INTLT_EMEREXIT_Position") as i32,
    }
}

#[derive(Default, Clone)]
struct Golden {
    lists: Option<Value>,
    deferred: Vec<Value>,
    shown: bool,
    id: u32,
    active_item: i64,
    abn_sensed: bool,
    to_config_normal: bool,
}

fn truthy(v: &Value) -> bool {
    v.as_bool().unwrap_or_else(|| v.as_f64().is_some_and(|n| n != 0.))
}

fn compare(ecl: &Ecl, g: &Golden) -> Vec<String> {
    let mut out = Vec::new();
    if ecl.shown != g.shown {
        out.push(format!("shown rust {} fbw {}", ecl.shown, g.shown));
    }
    if ecl.checklist_id != g.id {
        out.push(format!("checklist id rust {} fbw {}", ecl.checklist_id, g.id));
    }
    if g.shown && ecl.shown && ecl.selected_line as i64 != g.active_item {
        out.push(format!("selected rust {} fbw {}", ecl.selected_line, g.active_item));
    }
    let ids: Vec<u32> = ecl.deferred_procs.iter().map(|e| e.state.id).collect();
    let gids: Vec<u32> = g.deferred.iter().map(|d| d["id"].as_str().unwrap().parse().unwrap()).collect();
    if ids != gids {
        out.push(format!("deferred list rust {ids:?} fbw {gids:?}"));
    }
    for d in &g.deferred {
        let id: u32 = d["id"].as_str().unwrap().parse().unwrap();
        let Some(e) = ecl.deferred_procs.iter().find(|e| e.state.id == id) else { continue };
        let b = |k: &str| d[k].as_array().unwrap().iter().map(truthy).collect::<Vec<bool>>();
        let st = &e.state;
        if st.procedure_activated != truthy(&d["procedureActivated"])
            || st.procedure_completed != truthy(&d["procedureCompleted"])
            || st.items_checked != b("itemsChecked")
            || st.items_active != b("itemsActive")
        {
            out.push(format!("deferred {id}: rust {st:?} fbw {d}"));
        }
    }
    let Some(lists) = g.lists.as_ref().and_then(|l| l.as_array()) else { return out };
    for cl in lists {
        let id: u32 = cl["id"].as_str().unwrap().parse().unwrap();
        let st = ecl.state(id).unwrap();
        let checked: Vec<bool> = cl["itemsChecked"].as_array().unwrap().iter().map(truthy).collect();
        let completed = cl["procedureCompleted"].as_bool().unwrap_or(false);
        if checked != st.items_checked || (id != 0 && completed != st.procedure_completed) {
            out.push(format!(
                "{id}: rust {}{:?} fbw {}{:?}",
                st.procedure_completed, st.items_checked, completed, checked
            ));
        }
    }
    out
}

#[test]
fn ecl_matches_fbw_fws_oracle() {
    run("ecl_normal_walk", "ECL_ORACLE_GOLDEN", "E:/fbw-build/ecl39/oracle/ecl_normal_walk.json", 80);
}

#[test]
fn ecl_complete_and_reset_paths_match_fbw() {
    run("ecl_complete_reset", "ECL_ORACLE_GOLDEN_RESET", "E:/fbw-build/ecl49/oracle/ecl_complete_reset.json", 80);
}

fn run(name: &str, golden_var: &str, golden_default: &str, min_compared: usize) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scenario_path = root.join(format!("tools/fws_oracle/scenarios/{name}.json"));
    let golden_path = env_path(golden_var, golden_default);
    let states = env_path("FWS_ORACLE_STATES", "E:/fbw-build/emu-snapshots");
    let (Some(scenario), Some(golden)) = (load(&scenario_path), load(&golden_path)) else {
        eprintln!("skipped: no scenario or golden");
        return;
    };
    let mut events: Vec<Value> = golden["events"].as_array().unwrap().clone();
    events.sort_by(|a, b| a["t"].as_f64().partial_cmp(&b["t"].as_f64()).unwrap());
    let mut ev = 0;

    let mut vars = Vars(HashMap::new());
    let mut ecl = Ecl::new();
    ecl.rudder_trim = RudderTrimSensing::SignedAsFbw;
    let mut g = Golden::default();
    let mut prev_buttons = [false; 5];
    let mut abn_at_cycle = false;
    let mut buffered = [false; 5];
    let (mut since_cycle, mut last_set) = (0., 0.);
    let mut now = 0.;
    let (mut ticks, mut compared, mut bad) = (0usize, 0usize, Vec::new());

    for step in scenario["steps"].as_array().unwrap() {
        if let Some(name) = step.get("state").and_then(|v| v.as_str()) {
            let Some(state) = load(&states.join(format!("{name}.json"))) else {
                eprintln!("skipped: no state {name}");
                return;
            };
            for item in state["vars"].as_array().unwrap() {
                let (is_l, n, v) = (item[0].as_bool().unwrap(), item[1].as_str().unwrap(), item[2].as_f64().unwrap_or(0.));
                vars.put(&if is_l { format!("L:{n}") } else { n.to_string() }, v);
            }
        } else if let Some(set) = step.get("set").and_then(|v| v.as_object()) {
            for (n, v) in set {
                vars.put(n, v.as_f64().unwrap());
            }
            last_set = now;
        } else if let Some(ms) = step.get("run_ms").and_then(|v| v.as_f64()) {
            for _ in 0..(ms / STEP_MS).ceil() as usize {
                let buttons = ["L:A32NX_BTN_CL", "L:A32NX_BTN_CHECK_LH", "L:A32NX_BTN_UP", "L:A32NX_BTN_DOWN", "L:A32NX_BTN_CLR"]
                    .map(|n| vars.get(n) != 0.);
                for i in 0..5 {
                    buffered[i] |= buttons[i];
                }
                since_cycle += STEP_MS;
                let cycle = since_cycle >= FWS_CYCLE_MS;
                if cycle {
                    since_cycle -= FWS_CYCLE_MS;
                }
                let edge = |i: usize| cycle && buffered[i] && !prev_buttons[i];
                let pulses = EcpPulses { cl: edge(0), check: edge(1), up: edge(2), down: edge(3), clr: edge(4) };
                if cycle {
                    prev_buttons = buffered;
                    buffered = [false; 5];
                }

                let inputs = CycleInputs {
                    sensed: sensed(&vars, g.to_config_normal),
                    pulses,
                    abnormal_sensed_shown: abn_at_cycle,
                    ..CycleInputs::default()
                };
                if cycle {
                    for d in &g.deferred {
                        let id: u32 = d["id"].as_str().unwrap().parse().unwrap();
                        let b = |k: &str| d[k].as_array().unwrap().iter().map(truthy).collect::<Vec<bool>>();
                        ecl.core_update_deferred(id, &b("itemsChecked"), &b("itemsToShow"), None);
                    }
                    ecl.update(&inputs);
                }

                while ev < events.len() && events[ev]["t"].as_f64().unwrap() <= now {
                    let e = &events[ev];
                    match (e.get("topic").and_then(|t| t.as_str()), e.get("var").and_then(|t| t.as_str())) {
                        (Some("fws_normal_checklists"), _) if e["data"].as_array().is_some_and(|a| !a.is_empty()) => {
                            g.lists = Some(e["data"].clone())
                        }
                        (Some("fws_show_normal_checklists"), _) => g.shown = e["data"].as_bool().unwrap_or(false),
                        (Some("fws_deferred_procedures"), _) => {
                            g.deferred = e["data"].as_array().cloned().unwrap_or_default();
                            let keep: Vec<u32> = g.deferred.iter().map(|d| d["id"].as_str().unwrap().parse().unwrap()).collect();
                            ecl.core_retain_deferred(&keep);
                            for d in &g.deferred {
                                let id: u32 = d["id"].as_str().unwrap().parse().unwrap();
                                if !ecl.deferred_procs.iter().any(|x| x.state.id == id) {
                                    let b = |k: &str| d[k].as_array().unwrap().iter().map(truthy).collect::<Vec<bool>>();
                                    ecl.core_add_deferred(id, &b("itemsChecked"), Some(&b("itemsToShow")));
                                }
                            }
                        }
                        (Some("fws_normal_checklists_id"), _) => g.id = e["data"].as_u64().unwrap_or(0) as u32,
                        (Some("fws_active_item"), _) => g.active_item = e["data"].as_i64().unwrap_or(0),
                        (Some("fws_show_abn_sensed"), _) => g.abn_sensed = e["data"].as_bool().unwrap_or(false),
                        (_, Some("L:A32NX_TO_CONFIG_NORMAL")) => g.to_config_normal = e["value"].as_f64().unwrap_or(0.) != 0.,
                        _ => {}
                    }
                    ev += 1;
                }
                if cycle {
                    abn_at_cycle = g.abn_sensed;
                }
                ticks += 1;
                if g.lists.is_some() && now - last_set >= SETTLE_MS {
                    compared += 1;
                    let diffs = compare(&ecl, &g);
                    if !diffs.is_empty() {
                        bad.push(format!("t={now}: {}", diffs.join("; ")));
                    }
                }
                now += STEP_MS;
            }
        }
    }
    println!("{name}: {ticks} ticks, {compared} compared, {} mismatched", bad.len());
    for b in bad.iter().take(25) {
        println!("  {b}");
    }
    assert!(compared > min_compared, "too few compared ticks: {compared}");
    assert!(bad.is_empty(), "{} of {compared} ticks differ from FBW", bad.len());
}

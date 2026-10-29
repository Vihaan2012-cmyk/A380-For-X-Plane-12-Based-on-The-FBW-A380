use std::collections::BTreeMap;
use std::path::PathBuf;

use rmp::*;
use serde_json::Value;

const STEP_MS: f64 = 50.;
const RELEVANT: [&str; 6] = ["RMP", "VHF", "TRANSPONDER", "FBW_HF", "FBW_TEL", "FBW_AMU"];
const KEYS: [&str; 7] = ["XPNDR_SET", "XPNDR_IDENT_ON", "COM_RADIO_SET_HZ", "COM2_RADIO_SET_HZ", "COM3_RADIO_SET_HZ", "ELECTRICAL_CIRCUIT_TOGGLE", "LIGHT_POTENTIOMETER_SET"];

fn golden_dir() -> PathBuf {
    PathBuf::from(std::env::var("RMP_ORACLE_GOLDEN").unwrap_or_else(|_| "E:/fbw-build/rmp-oracle/golden".into()))
}

struct World {
    vars: Vars,
    rmps: Vec<Rmp>,
    vhf: Vec<VhfRadioUnit>,
    xpdr: TransponderUnit,
    keys: Vec<(String, Vec<f64>)>,
    pending: Vec<String>,
}

fn bcd_from_hz(hz: f64) -> f64 {
    u64::from_str_radix(&format!("{}", (hz / 100.).round() as u64), 16).unwrap_or(0) as f64
}

impl World {
    fn new() -> Self {
        let mut vars = Vars::new();
        for (name, value) in [
            ("L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED", 1.),
            ("L:A32NX_ELEC_DC_1_BUS_IS_POWERED", 1.),
            ("L:A32NX_ELEC_DC_2_BUS_IS_POWERED", 1.),
            ("L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED", 1.),
            ("L:A32NX_ELEC_AC_2_BUS_IS_POWERED", 1.),
            ("L:A32NX_COLD_AND_DARK_SPAWN", 0.),
            ("L:A32NX_LGCIU_1_LEFT_GEAR_COMPRESSED", 1.),
            ("L:A380X_RMP_1_BRIGHTNESS_KNOB", 50.),
            ("L:A380X_RMP_2_BRIGHTNESS_KNOB", 50.),
            ("L:A380X_RMP_3_BRIGHTNESS_KNOB", 50.),
            ("COM ACTIVE FREQUENCY:1", 0x1189500 as f64),
            ("COM STANDBY FREQUENCY:1", 0x1219000 as f64),
            ("COM ACTIVE FREQUENCY:2", 0x1262000 as f64),
            ("COM STANDBY FREQUENCY:2", 0x1215000 as f64),
            ("TRANSPONDER CODE:1", 0x2000 as f64),
        ] {
            vars.set_quiet(name, value);
        }
        let mut w = World { vars, rmps: (1..=3).map(Rmp::new).collect(), vhf: (1..=3).map(VhfRadioUnit::new).collect(), xpdr: TransponderUnit::new(), keys: Vec::new(), pending: Vec::new() };
        for i in 0..3 {
            w.rmps[i].init(&mut w.vars);
        }
        w.rmps[0].flight_start(&mut w.vars);
        w.xpdr.flight_start(&mut w.vars);
        w.absorb_keys();
        w
    }

    fn absorb_keys(&mut self) {
        for k in self.vars.take_keys() {
            let arg = k.args.first().copied().unwrap_or(0.);
            if k.name == "ELECTRICAL_CIRCUIT_TOGGLE" {
                let var = format!("CIRCUIT SWITCH ON:{}", arg as i64);
                let on = self.vars.get(&var) > 0.;
                self.vars.set_quiet(&var, if on { 0. } else { 1. });
            } else if let Some(n) = k.name.strip_prefix("COM").and_then(|s| s.strip_suffix("_RADIO_SET_HZ")) {
                let index: u32 = n.parse().unwrap_or(1);
                self.vars.set_quiet(&format!("COM ACTIVE FREQUENCY:{index}"), bcd_from_hz(arg));
            } else if k.name == "COM_RADIO_SET_HZ" {
                self.vars.set_quiet("COM ACTIVE FREQUENCY:1", bcd_from_hz(arg));
            }
            self.keys.push((k.name, k.args));
        }
    }

    fn apply(&mut self, input: &Value) {
        if let Some(set) = input.get("set").and_then(Value::as_object) {
            for (name, value) in set {
                self.vars.set_quiet(name, value.as_f64().unwrap());
            }
        }
        if let Some(h) = input.get("h").and_then(Value::as_str) {
            self.pending.push(h.to_string());
        }
        if let Some(ids) = input.get("failures").and_then(Value::as_array) {
            let active: Vec<i64> = ids.iter().filter_map(Value::as_i64).collect();
            for (id, key) in [(23002, "RadioManagementPanel1"), (23003, "RadioManagementPanel2"), (23004, "RadioManagementPanel3"), (23005, "Vhf1"), (23006, "Vhf2"), (23007, "Vhf3"), (34003, "Transponder1")] {
                self.vars.set_failure(key, active.contains(&id));
            }
        }
    }

    fn tick(&mut self) {
        for i in 0..3 {
            for h in &self.pending {
                self.rmps[i].hevent(h, &mut self.vars);
            }
            self.rmps[i].update(STEP_MS, &mut self.vars);
            self.absorb_keys();
        }
        self.pending.clear();
        let powered = [self.vars.get("L:A32NX_ELEC_DC_ESS_BUS_IS_POWERED") > 0., self.vars.get("L:A32NX_ELEC_DC_2_BUS_IS_POWERED") > 0., self.vars.get("L:A32NX_ELEC_DC_1_BUS_IS_POWERED") > 0.];
        for i in 0..3 {
            self.vhf[i].update(powered[i], &mut self.vars);
            self.absorb_keys();
        }
        let xp = self.vars.get("L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED") > 0. || self.vars.get("L:A32NX_ELEC_AC_2_BUS_IS_POWERED") > 0.;
        self.xpdr.update(xp, &mut self.vars);
        self.absorb_keys();
    }

    fn take_writes(&mut self) -> Vec<(String, f64)> {
        self.vars
            .take_writes()
            .into_iter()
            .filter(|w| RELEVANT.iter().any(|r| w.name.contains(r)))
            .map(|w| (w.name.strip_prefix("A:").unwrap_or(&w.name).to_string(), w.value))
            .collect()
    }

    fn take_relevant_keys(&mut self) -> Vec<(String, Vec<f64>)> {
        std::mem::take(&mut self.keys).into_iter().filter(|(n, a)| KEYS.contains(&n.as_str()) && (n != "LIGHT_POTENTIOMETER_SET" || (80.0..=82.0).contains(&a[0]))).collect()
    }
}

fn golden_writes(v: &Value) -> Vec<(String, f64)> {
    v.as_array().unwrap().iter().filter(|w| !w[0].as_str().unwrap().contains('#')).map(|w| (w[0].as_str().unwrap().to_string(), w[1].as_f64().unwrap())).collect()
}

fn golden_keys(v: &Value) -> Vec<(String, Vec<f64>)> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|k| (k[0].as_str().unwrap().to_string(), k[1].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect::<Vec<_>>()))
        .filter(|(n, a)| KEYS.contains(&n.as_str()) && (n != "LIGHT_POTENTIOMETER_SET" || (80.0..=82.0).contains(&a[0])))
        .map(|(n, mut a)| {
            a.truncate(if n == "LIGHT_POTENTIOMETER_SET" { 2 } else { 1 });
            (n, a)
        })
        .collect()
}

fn trim_args(keys: Vec<(String, Vec<f64>)>) -> Vec<(String, Vec<f64>)> {
    keys.into_iter()
        .map(|(n, mut a)| {
            a.truncate(if n == "LIGHT_POTENTIOMETER_SET" { 2 } else { 1 });
            (n, a)
        })
        .collect()
}

#[derive(Default, Debug, PartialEq)]
struct Screen {
    message: String,
    actives: Vec<(String, bool)>,
    standby: Vec<(String, Vec<bool>)>,
}

fn screen_from_dom(leaves: &Value) -> Screen {
    let mut s = Screen::default();
    let mut rows: BTreeMap<i64, Vec<(char, bool)>> = BTreeMap::new();
    for l in leaves.as_array().unwrap() {
        let (text, x, y, class) = (l[0].as_str().unwrap(), l[1].as_i64().unwrap(), l[2].as_i64().unwrap(), l[3].as_str().unwrap());
        if class.contains("rmp-messages") {
            s.message = text.to_string();
        } else if class.contains("active-frequency") {
            s.actives.push((text.to_string(), class.contains("amber")));
        } else if x == 1381 && text.chars().count() == 1 && text != "." && !class.contains("stdby-mode") {
            rows.entry(y).or_default().push((text.chars().next().unwrap(), class.contains("font-19")));
        }
    }
    for (_, digits) in rows {
        s.standby.push((digits.iter().map(|d| d.0).collect(), digits.iter().map(|d| d.1).collect()));
    }
    s
}

fn screen_from_model(rmp: &Rmp) -> Screen {
    let v = rmp.view();
    let mut s = Screen { message: v.message.clone(), ..Default::default() };
    if !v.screen_on {
        return Screen::default();
    }
    if v.page == Page::Vhf {
        for r in &v.rows {
            s.actives.push((r.active_text.clone(), r.active_amber));
            s.standby.push((r.standby_digits.clone(), r.standby_pilot_entered.to_vec()));
        }
    }
    s
}

fn run(name: &str) -> (usize, usize, usize, Vec<String>) {
    let path = golden_dir().join(format!("{name}.json"));
    let golden: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap();
    let mut w = World::new();
    let boot_ticks = (golden["boot_ms"].as_f64().unwrap() / STEP_MS) as usize;
    for _ in 0..boot_ticks {
        w.tick();
    }
    w.take_writes();
    w.take_relevant_keys();
    let mut diffs = Vec::new();
    let mut last: BTreeMap<String, f64> = BTreeMap::new();
    for (n, v) in golden_writes(&golden["boot_writes"]) {
        last.insert(n, v);
    }
    for (n, v) in &last {
        let got = w.vars.get(n);
        if got.to_bits() != v.to_bits() {
            diffs.push(format!("boot {n}: oracle {v}, model {got}"));
        }
    }
    let (mut exact, mut order_only, mut bad) = (0, 0, 0);
    let ticks = golden["ticks"].as_array().unwrap();
    for t in ticks {
        for input in t["inputs"].as_array().unwrap() {
            w.apply(input);
        }
        w.tick();
        let mut want = golden_writes(&t["writes"]);
        let mut got = w.take_writes();
        let want_keys = golden_keys(&t["keys"]);
        let got_keys = trim_args(w.take_relevant_keys());
        let at = t["t"].as_f64().unwrap();
        if want == got && want_keys == got_keys {
            exact += 1;
        } else {
            let (mut a, mut b) = (want.clone(), got.clone());
            a.sort_by(|x, y| x.partial_cmp(y).unwrap());
            b.sort_by(|x, y| x.partial_cmp(y).unwrap());
            if a == b && want_keys == got_keys {
                order_only += 1;
                if diffs.len() < 3 {
                    diffs.push(format!("order only t={at}: oracle {want:?}; model {got:?}"));
                }
            } else {
                bad += 1;
                diffs.push(format!("t={at} inputs {}: oracle writes {want:?} keys {want_keys:?}; model writes {got:?} keys {got_keys:?}", t["inputs"]));
            }
        }
        want.clear();
        got.clear();
    }
    (exact, order_only, bad, diffs)
}

fn screens(name: &str) -> (usize, usize, Vec<String>) {
    let path = golden_dir().join(format!("{name}.json"));
    let golden: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut w = World::new();
    for _ in 0..(golden["boot_ms"].as_f64().unwrap() / STEP_MS) as usize {
        w.tick();
    }
    let mut current: Vec<Option<Screen>> = (0..3).map(|_| None).collect();
    let (mut ok, mut bad) = (0, 0);
    let mut diffs = Vec::new();
    for t in golden["ticks"].as_array().unwrap() {
        for input in t["inputs"].as_array().unwrap() {
            w.apply(input);
        }
        w.tick();
        for (i, v) in t["view"].as_array().unwrap().iter().enumerate() {
            if v.as_str() != Some("same") && !v.is_null() {
                current[i] = Some(screen_from_dom(v));
            }
            let Some(want) = &current[i] else { continue };
            let got = screen_from_model(&w.rmps[i]);
            let got = Screen { message: got.message, ..got };
            if want.actives == got.actives && want.standby == got.standby && want.message == got.message {
                ok += 1;
            } else {
                bad += 1;
                if diffs.len() < 6 {
                    diffs.push(format!("t={} rmp{}: oracle {want:?}; model {got:?}", t["t"], i + 1));
                }
            }
        }
    }
    (ok, bad, diffs)
}

const SCENARIOS: [&str; 8] = ["power_up", "vhf_entry_transfer", "incomplete_freq_message", "vhf3_data_emer", "squawk", "transmit_receive", "rmp_failure", "rmp3_backup"];

#[test]
fn model_matches_oracle_writes_and_keys() {
    let mut failed = Vec::new();
    for name in SCENARIOS {
        let (exact, order_only, bad, diffs) = run(name);
        println!("{name}: {exact} exact ticks, {order_only} order-only, {bad} differing");
        for d in diffs.iter().take(8) {
            println!("   {d}");
        }
        if bad > 0 || diffs.iter().any(|d| d.starts_with("boot")) {
            failed.push(name);
        }
    }
    assert!(failed.is_empty(), "differences in {failed:?}");
}

#[test]
fn model_screens_match_oracle_dom() {
    let mut failed = Vec::new();
    for name in SCENARIOS {
        let (ok, bad, diffs) = screens(name);
        println!("{name}: screens {ok} match, {bad} differ");
        for d in &diffs {
            println!("   {d}");
        }
        if bad > 0 {
            failed.push(name);
        }
    }
    assert!(failed.is_empty(), "screen differences in {failed:?}");
}

use super::*;
use datalink::system::{arinc_raw, ATC_MSG_ACK_VAR};

#[derive(Default)]
struct Fake {
    vars: HashMap<String, f64>,
    written: Vec<(String, f64)>,
    sounds: Vec<String>,
}

impl Io for Fake {
    fn get(&mut self, name: &str, _unit: &str) -> f64 {
        self.vars.get(name).copied().unwrap_or(0.)
    }

    fn set(&mut self, name: &str, value: f64) {
        self.written.push((name.to_string(), value));
        self.vars.insert(name.to_string(), value);
    }

    fn play_sound(&mut self, name: &str) {
        self.sounds.push(name.to_string());
    }
}

fn native(authoritative: bool) -> Native {
    let mut n = Native::new();
    n.set_config(Config { off: false, authoritative });
    n
}

fn worker_out() -> StepOut {
    StepOut {
        bus: vec![("routerDatalinkStatus".into(), json!({ "vhf": 2, "satellite": 0, "hf": 0 }))],
        mfd: vec![("atcAtis_0".into(), json!({ "icao": "EGLL" }))],
        vars: vec![(ATC_MSG_WAITING_VAR.into(), 1.), (ACARS_ACTIVE_VAR.into(), 1.)],
        sounds: vec!["cpdlc_ring".into()],
        winds: vec![(9, WindRequestMessage::default())],
    }
}

#[test]
fn config_names_datalink_and_off_wins() {
    assert!(Config::parse("vhf, DATALINK", false).authoritative);
    assert!(!Config::parse("vhf,xpdr", false).authoritative);
    assert!(Config::parse("", true).authoritative);
    assert!(!Config::parse("datalink off", true).authoritative);
    assert_eq!(Config::parse("datalink", false).mask(), 1.);
    assert_eq!(Config::parse("", false).mask(), 0.);
}

#[test]
fn inputs_are_read_from_simvarhandlings_sources() {
    let mut io = Fake::default();
    io.vars.insert("L:A32NX_ADIRS_IR_1_LATITUDE".into(), arinc_raw(51.5, 3));
    io.vars.insert("L:A32NX_CPIOM_D1_AVAIL".into(), 1.);
    io.vars.insert("L:A32NX_CPIOM_D3_AVAIL".into(), 1.);
    io.vars.insert("L:A32NX_ELEC_AC_1_BUS_IS_POWERED".into(), 1.);
    io.vars.insert("L:FBW_RMP_MODE_ACTIVE_3".into(), 1.);
    io.vars.insert(ATC_MSG_ACK_VAR.into(), 1.);
    io.vars.insert("E:ZULU TIME".into(), 45_000.);
    let s = read_inputs(&mut io);
    assert_eq!(s.latitude, arinc_raw(51.5, 3));
    assert!(s.cpiom_d1_available && s.cpiom_d3_available && s.ac_bus_1 && s.atc_button_pressed);
    assert_eq!(s.rmp_mode_active_3, 1.);
    assert_eq!(s.utc_seconds, 45_000.);
}

#[test]
fn authoritative_writes_vars_mask_sounds_and_queues_topics() {
    let mut n = native(true);
    let mut io = Fake::default();
    let mut lines = Vec::new();
    let f = n.frame(&mut io, 1.0, worker_out(), &mut lines);
    assert!(io.written.contains(&(MASK_VAR.to_string(), 1.)));
    assert!(io.written.contains(&(ATC_MSG_WAITING_VAR.to_string(), 1.)));
    assert_eq!(io.sounds, vec!["cpdlc_ring".to_string()]);
    assert_eq!(f.to_js.len(), 1, "MFD-local topics stay native");
    assert_eq!(f.winds.len(), 1);
    assert!(f.snapshot.as_ref().is_some_and(|s| s.authoritative && s.now_ms == 1000.));
    assert!(datalink_host::string_var("atcAtis_0").is_some());
    let again = n.frame(&mut io, 1.1, StepOut::default(), &mut lines);
    assert_eq!(io.written.iter().filter(|(v, _)| v == MASK_VAR).count(), 1);
    assert!(again.to_js.is_empty());
}

#[test]
fn shadow_writes_nothing_and_compares_after_the_debounce() {
    let mut n = native(false);
    let mut io = Fake::default();
    let mut lines = Vec::new();
    let f = n.frame(&mut io, 0.0, worker_out(), &mut lines);
    assert_eq!(io.written, vec![(MASK_VAR.to_string(), 0.)]);
    assert!(io.sounds.is_empty() && f.to_js.is_empty() && f.winds.is_empty());
    assert_eq!(f.unanswered_winds, vec![9]);
    assert!(f.snapshot.is_some_and(|s| !s.authoritative));
    for i in 1..40 {
        n.frame(&mut io, i as f64 * 0.1, StepOut::default(), &mut lines);
    }
    assert!(lines.iter().any(|l| l.contains("datalink") && l.contains("A32NX_DCDU_ATC_MSG_WAITING") && l.contains("native=1") && l.contains("fbw=0")), "{lines:?}");

    let mut n = native(false);
    let mut io = Fake::default();
    io.vars.insert(ACARS_ACTIVE_VAR.into(), 1.);
    let mut lines = Vec::new();
    for i in 0..40 {
        n.frame(&mut io, i as f64 * 0.1, if i == 0 { worker_out() } else { StepOut::default() }, &mut lines);
    }
    assert!(!lines.iter().any(|l| l.contains("A32NX_DCDU_ATC_MSG_WAITING")), "JS online: nothing comparable {lines:?}");
}

#[test]
fn off_does_nothing() {
    let mut n = Native::new();
    n.set_config(Config { off: true, authoritative: false });
    let mut io = Fake::default();
    let f = n.frame(&mut io, 0.0, worker_out(), &mut Vec::new());
    assert!(io.written.is_empty() && f.snapshot.is_none());
}

#[test]
fn coherent_calls_forward_inputs_and_hand_back_topics() {
    assert_eq!(coherent_call(INPUT_CALL, r#"["atcLogon","{\"station\":\"EDDF\",\"requestId\":3}"]"#).as_deref(), Some("true"));
    push_to_js(vec![("atcStationStatus".into(), json!({ "current": "EDDF" }))]);
    let taken: Value = serde_json::from_str(&coherent_call(TAKE_CALL, "[]").unwrap()).unwrap();
    assert_eq!(taken["bus"][0][0], "atcStationStatus");
    assert_eq!(taken["bus"][0][1]["current"], "EDDF");
    let empty: Value = serde_json::from_str(&coherent_call(TAKE_CALL, "[]").unwrap()).unwrap();
    assert_eq!(empty["bus"].as_array().map(Vec::len), Some(0));
    assert!(coherent_call("SOMETHING_ELSE", "[]").is_none());
}

#[test]
fn dispatch_answers_winds_and_submits_the_snapshot() {
    let frame = Frame {
        snapshot: Some(Snapshot { now_ms: 42., sim: SimInputs::default(), authoritative: true }),
        to_js: vec![],
        winds: vec![(5, WindRequestMessage::default())],
        unanswered_winds: vec![6],
    };
    let asked = std::cell::Cell::new(0);
    dispatch(frame, |_| {
        asked.set(asked.get() + 1);
        None
    });
    assert_eq!(asked.get(), 1);
}

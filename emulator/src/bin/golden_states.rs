//! Writes one snapshot of every variable per scenario, for the screen
//! goldens (tools/reference, `states/emu-*.json` via
//! tools/reference/src/emu-states.mjs).
//!
//!     golden_states <out dir>

use std::fmt::Write as _;
use std::path::Path;

use fbw_a380_emulator::{presets, Emulator};

const DT: f64 = 0.1;

#[derive(Clone, Copy)]
struct Flight {
    alt_ft: f64,
    ias_kt: f64,
    gs_kt: f64,
    pitch_deg: f64,
    vs_fpm: f64,
}

thread_local! {
    static FLIGHT: std::cell::Cell<Option<Flight>> = const { std::cell::Cell::new(None) };
    static DEAD_ADIRU: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
}

fn arinc(value: f64, ssm: u64) -> f64 {
    (((ssm << 32) | (value as f32).to_bits() as u64) as f64).round()
}

fn adirs_words(e: &mut Emulator, f: Flight) {
    let sat = (15.0 - 0.001_98 * f.alt_ft).max(-56.5);
    let tas = f.ias_kt * (1.0 + f.alt_ft * 0.000_02);
    let mach = tas / (661.47 * ((sat + 273.15) / 288.15).sqrt());
    let tat = sat + (sat + 273.15) * 0.2 * mach * mach;
    let pressure_hpa = 1013.25 * (1.0 - 6.875_6e-6 * f.alt_ft).max(0.2).powf(5.255_9);
    let fpa = if f.gs_kt > 1.0 { (f.vs_fpm / (f.gs_kt * 101.27)).atan().to_degrees() } else { 0.0 };
    let normal = 3u64;
    for n in 1..=3u32 {
        if DEAD_ADIRU.with(|d| d.get()) == Some(n) {
            continue;
        }
        let adr = |name: &str| format!("A32NX_ADIRS_ADR_{n}_{name}");
        let ir = |name: &str| format!("A32NX_ADIRS_IR_{n}_{name}");
        for (name, v) in [
            ("ALTITUDE", f.alt_ft),
            ("BARO_CORRECTED_ALTITUDE_1", f.alt_ft),
            ("BARO_CORRECTED_ALTITUDE_2", f.alt_ft),
            ("BARO_CORRECTION_1_HPA", 1013.25),
            ("BARO_CORRECTION_2_HPA", 1013.25),
            ("BARO_CORRECTION_1_INHG", 29.92),
            ("BARO_CORRECTION_2_INHG", 29.92),
            ("COMPUTED_AIRSPEED", f.ias_kt),
            ("TRUE_AIRSPEED", tas),
            ("MACH", mach),
            ("BAROMETRIC_VERTICAL_SPEED", f.vs_fpm),
            ("STATIC_AIR_TEMPERATURE", sat),
            ("TOTAL_AIR_TEMPERATURE", tat),
            ("ANGLE_OF_ATTACK", f.pitch_deg - fpa),
            ("CORRECTED_AVERAGE_STATIC_PRESSURE", pressure_hpa),
            ("MAX_AIRSPEED", 340.0),
        ] {
            e.set_var(&adr(name), arinc(v, normal));
        }
        for (name, v) in [
            ("PITCH", f.pitch_deg),
            ("ROLL", 0.0),
            ("HEADING", 90.0),
            ("TRUE_HEADING", 90.0),
            ("TRACK", 90.0),
            ("TRUE_TRACK", 90.0),
            ("GROUND_SPEED", f.gs_kt),
            ("VERTICAL_SPEED", f.vs_fpm),
            ("FLIGHT_PATH_ANGLE", fpa),
            ("DRIFT_ANGLE", 0.0),
            ("BODY_NORMAL_ACC", 1.0),
            ("BODY_LATERAL_ACC", 0.0),
            ("BODY_LONGITUDINAL_ACC", 0.0),
            ("BODY_PITCH_RATE", 0.0),
            ("BODY_ROLL_RATE", 0.0),
            ("BODY_YAW_RATE", 0.0),
            ("PITCH_ATT_RATE", 0.0),
            ("ROLL_ATT_RATE", 0.0),
            ("HEADING_RATE", 0.0),
            ("LATITUDE", 51.47),
            ("LONGITUDE", -0.46),
            ("WIND_DIRECTION", 270.0),
            ("WIND_SPEED", 20.0),
            ("WIND_DIRECTION_BNR", 270.0),
            ("WIND_SPEED_BNR", 20.0),
        ] {
            e.set_var(&ir(name), arinc(v, normal));
        }
    }
}

fn settle(e: &mut Emulator, seconds: f64) {
    e.run(DT, (seconds / DT).round() as u32);
}

fn fail(e: &mut Emulator, words: &[&str]) -> Vec<String> {
    let ids: Vec<u64> = e
        .list_failures()
        .into_iter()
        .filter(|&id| {
            let name = e.failure_name(id).to_ascii_lowercase();
            words.iter().all(|w| name.contains(&w.to_ascii_lowercase()))
        })
        .collect();
    let Some(&id) = ids.first() else {
        return vec![format!("no failure matches {words:?}")];
    };
    e.set_failure_magnitude(id, 1.0);
    vec![format!("{id} {}", e.failure_name(id))]
}

fn adirs_nav(e: &mut Emulator) {
    FLIGHT.with(|f| {
        if f.get().is_none() {
            f.set(Some(Flight { alt_ft: 80.0, ias_kt: 0.0, gs_kt: 0.0, pitch_deg: 0.0, vs_fpm: 0.0 }));
        }
    });
    e.set_var("A32NX_CONFIG_ADIRS_IR_ALIGN_TIME", 1.0);
    for n in 1..=3 {
        e.set_var(&format!("A32NX_OVHD_ADIRS_IR_{n}_MODE_SELECTOR_KNOB"), 1.0);
        e.set_var(&format!("A32NX_OVHD_ADIRS_IR_{n}_PB_IS_ON"), 1.0);
        e.set_var(&format!("A32NX_OVHD_ADIRS_ADR_{n}_PB_IS_ON"), 1.0);
    }
}

fn levers(e: &mut Emulator, ratio: f64) {
    for n in 1..=4 {
        e.set_thrust_lever(n, ratio);
    }
}

fn airborne(e: &mut Emulator, alt_ft: f64, agl_ft: f64, ias_kt: f64, pitch_deg: f64, gear_down: bool, flaps: f64) {
    let gs = ias_kt * (1.0 + alt_ft / 1000.0 * 0.02);
    let vs = gs * 101.27 * (pitch_deg - 2.5).to_radians().tan();
    FLIGHT.with(|f| f.set(Some(Flight { alt_ft, ias_kt, gs_kt: gs, pitch_deg, vs_fpm: vs })));
    adirs_nav(e);
    e.set_on_ground(false);
    e.set_pressure_altitude_ft(alt_ft);
    e.set_agl_ft(agl_ft);
    e.set_indicated_airspeed_kt(ias_kt);
    e.set_true_airspeed_kt(ias_kt * (1.0 + alt_ft / 1000.0 * 0.02));
    e.set_groundspeed_kt(ias_kt * (1.0 + alt_ft / 1000.0 * 0.02));
    e.set_pitch_deg(pitch_deg);
    e.set_bank_deg(0.0);
    e.set_heading_true_deg(90.0);
    e.set_gear_handle_down(gear_down);
    e.set_var("A32NX_FLAPS_HANDLE_INDEX", flaps);
}

fn on_ground(e: &mut Emulator, gs_kt: f64, flaps: f64) {
    FLIGHT.with(|f| f.set(Some(Flight { alt_ft: 80.0, ias_kt: gs_kt, gs_kt, pitch_deg: 0.0, vs_fpm: 0.0 })));
    adirs_nav(e);
    e.set_on_ground(true);
    e.set_agl_ft(0.0);
    e.set_groundspeed_kt(gs_kt);
    e.set_indicated_airspeed_kt(gs_kt);
    e.set_true_airspeed_kt(gs_kt);
    e.set_pitch_deg(0.0);
    e.set_heading_true_deg(90.0);
    e.set_gear_handle_down(true);
    e.set_var("A32NX_FLAPS_HANDLE_INDEX", flaps);
}

fn flying(alt_ft: f64, agl_ft: f64, ias_kt: f64, pitch_deg: f64, gear_down: bool, flaps: f64, lever: f64) -> Emulator {
    let mut e = presets::engines_running();
    levers(&mut e, lever);
    airborne(&mut e, alt_ft, agl_ft, ias_kt, pitch_deg, gear_down, flaps);
    settle(&mut e, 15.0);
    e
}

fn cruise() -> Emulator {
    flying(37_000.0, 36_000.0, 270.0, 2.5, false, 0.0, 0.85)
}

type Build = fn() -> (Emulator, Vec<String>);

fn scenarios() -> Vec<(&'static str, &'static str, Build)> {
    vec![
        ("emu-01-cold-dark", "Unpowered at the gate", || (presets::cold_and_dark(), vec![])),
        ("emu-02-batteries", "Batteries on, nothing else", || {
            let mut e = presets::cold_and_dark();
            for bat in ["BAT_1", "BAT_2", "BAT_ESS", "BAT_APU"] {
                e.set_var(&format!("A32NX_OVHD_ELEC_{bat}_PB_IS_AUTO"), 1.0);
            }
            settle(&mut e, 20.0);
            (e, vec![])
        }),
        ("emu-03-ground-power", "External power on, systems powered, engines off", || {
            let mut e = presets::ground_power();
            adirs_nav(&mut e);
            settle(&mut e, 20.0);
            (e, vec![])
        }),
        ("emu-04-apu", "APU running with its generators and bleed", || {
            let mut e = presets::powered();
            adirs_nav(&mut e);
            settle(&mut e, 20.0);
            (e, vec![])
        }),
        ("emu-05-engines-idle", "All four engines at ground idle, parked", || {
            let mut e = presets::engines_running();
            on_ground(&mut e, 0.0, 0.0);
            settle(&mut e, 15.0);
            (e, vec![])
        }),
        ("emu-06-taxi", "Taxiing at 15 kt, flaps 1", || {
            let mut e = presets::engines_running();
            on_ground(&mut e, 15.0, 1.0);
            settle(&mut e, 15.0);
            (e, vec![])
        }),
        ("emu-07-takeoff-roll", "Take-off roll at 100 kt, levers at TOGA, flaps 2", || {
            let mut e = presets::engines_running();
            on_ground(&mut e, 100.0, 2.0);
            levers(&mut e, 1.0);
            settle(&mut e, 10.0);
            (e, vec![])
        }),
        ("emu-08-initial-climb", "Just airborne, 400 ft, 170 kt, gear down, flaps 2", || {
            let mut e = presets::engines_running();
            levers(&mut e, 1.0);
            airborne(&mut e, 400.0, 400.0, 170.0, 12.0, true, 2.0);
            settle(&mut e, 10.0);
            (e, vec![])
        }),
        ("emu-09-climb", "Clean climb through FL150 at 300 kt", || {
            (flying(15_000.0, 14_000.0, 300.0, 6.0, false, 0.0, 0.9), vec![])
        }),
        ("emu-10-cruise", "Cruise FL370, M0.85", || (cruise(), vec![])),
        ("emu-11-descent", "Idle descent through FL200", || {
            (flying(20_000.0, 19_000.0, 300.0, -3.0, false, 0.0, 0.0), vec![])
        }),
        ("emu-12-approach", "Approach at 1500 ft, 145 kt, CONF 3, gear down", || {
            (flying(1_500.0, 1_500.0, 145.0, 3.0, true, 3.0, 0.45), vec![])
        }),
        ("emu-13-rollout", "Landing rollout at 60 kt, CONF FULL, idle", || {
            let mut e = presets::engines_running();
            on_ground(&mut e, 60.0, 4.0);
            levers(&mut e, 0.0);
            settle(&mut e, 8.0);
            (e, vec![])
        }),
        ("emu-14-eng1-shutdown", "Cruise with engine 1 master off", || {
            let mut e = cruise();
            e.set_var("GENERAL ENG STARTER:1", 0.0);
            settle(&mut e, 30.0);
            (e, vec!["engine 1 master off".to_string()])
        }),
        ("emu-15-eng2-fire", "Cruise with an engine 2 fire", || {
            let mut e = cruise();
            let notes = fail(&mut e, &["engine 2", "fire"]);
            settle(&mut e, 10.0);
            (e, notes)
        }),
        ("emu-16-green-hyd", "Cruise with a green hydraulic system leak", || {
            let mut e = cruise();
            let notes = fail(&mut e, &["green", "leak"]);
            settle(&mut e, 60.0);
            (e, notes)
        }),
        ("emu-17-elec-emer", "Cruise with every engine and APU generator failed", || {
            let mut e = cruise();
            for id in [24_020, 24_021, 24_022, 24_023, 24_030, 24_031] {
                e.set_failure_magnitude(id, 1.0);
            }
            settle(&mut e, 15.0);
            (e, vec!["generators 1-4 and APU generators 1-2 failed".to_string()])
        }),
        ("emu-18-adiru", "Cruise with the ADIRS 1 mode selector OFF", || {
            let mut e = cruise();
            e.set_var("A32NX_OVHD_ADIRS_IR_1_MODE_SELECTOR_KNOB", 0.0);
            DEAD_ADIRU.with(|d| d.set(Some(1)));
            settle(&mut e, 15.0);
            (e, vec!["ADIRS 1 selector OFF".to_string()])
        }),
        ("emu-19-decompression", "Cruise with a rapid decompression", || {
            let mut e = cruise();
            let notes = fail(&mut e, &["rapid decompression"]);
            settle(&mut e, 30.0);
            (e, notes)
        }),
        ("emu-20-apu-fire-ground", "APU running on the ground with an APU fire", || {
            let mut e = presets::powered();
            adirs_nav(&mut e);
            let notes = fail(&mut e, &["apu", "fire"]);
            settle(&mut e, 10.0);
            (e, notes)
        }),
    ]
}

fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn main() {
    std::thread::Builder::new().stack_size(512 << 20).spawn(generate).unwrap().join().unwrap();
}

fn generate() {
    let out = std::env::args().nth(1).expect("usage: golden_states <out dir>");
    std::fs::create_dir_all(&out).unwrap();
    let only = std::env::args().nth(2);
    for (name, description, build) in scenarios() {
        if only.as_deref().is_some_and(|o| !name.contains(o)) {
            continue;
        }
        FLIGHT.with(|f| f.set(None));
        DEAD_ADIRU.with(|d| d.set(None));
        let (mut e, notes) = build();
        if let Some(f) = FLIGHT.with(|f| f.get()) {
            adirs_words(&mut e, f);
        }
        let mut text = String::new();
        let _ = write!(text, "{{\"name\":{},\"description\":{},\"notes\":[", json_string(name), json_string(description));
        let _ = write!(text, "{}", notes.iter().map(|n| json_string(n)).collect::<Vec<_>>().join(","));
        text.push_str("],\"vars\":[");
        let vars: Vec<String> = e
            .snapshot_kinds()
            .into_iter()
            .filter(|(_, _, v)| v.is_finite())
            .map(|(named, n, v)| format!("[{},{},{}]", named, json_string(&n), v))
            .collect();
        text.push_str(&vars.join(","));
        text.push_str("]}");
        std::fs::write(Path::new(&out).join(format!("{name}.json")), text).unwrap();
        println!("{name}: {} vars {}", vars.len(), notes.join("; "));
    }
}

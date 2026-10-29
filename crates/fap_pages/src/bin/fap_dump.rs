#![allow(clippy::needless_range_loop)]

use cids::doors::{DoorState, DoorView, SlideState};
use cids::view::{FapView, HardKey, PageBody, SwitchView, ViewMode};
use cids::{Caution, Level, Page};
use fap_pages::pages::AREAS;
use fap_pages::sim::{DoorMark, DOORS, ZONES};
use fap_pages::{fap_root, page_from_id, FapSim, Ui, LEGACY_NAV};

fn legacy_cautions(s: &FapSim) -> Vec<Caution> {
    let mut out = Vec::new();
    let c = |level, text: String| Caution { level, text };
    if s.cargo_smoke.iter().any(|v| FapSim::smoke_on(*v)) {
        out.push(c(Level::Red, "CARGO SMOKE".into()));
    }
    for i in 0..3 {
        if FapSim::smoke_on(s.ife_smoke[i]) || FapSim::smoke_on(s.galley_smoke[i]) {
            out.push(c(Level::Red, format!("SMOKE {}", ZONES[i])));
        }
        if s.waste_full[i] {
            out.push(c(Level::Amber, format!("WASTE {} FULL", ZONES[i])));
        }
    }
    if s.slide_low {
        out.push(c(Level::Amber, "SLIDE PRESS LO".into()));
    }
    if s.calls > 0. {
        out.push(c(Level::Cyan, format!("CALL {}", s.calls)));
    }
    out.truncate(3);
    out
}

fn legacy_keys(s: &FapSim) -> Vec<HardKey> {
    let k = |id, label, lit| HardKey { id, label, lit, alert: false, guard: false };
    vec![
        k("ped", "PED POWER", !s.seat_power_off && !s.pax_sys_off),
        k("lights", "LIGHTS MAIN ON/OFF", s.level_raw != 1.),
        k("lav", "LAV MAINT", false),
        k("lock", "SCREEN 30 sec LOCK", false),
        k("evac", "EVAC CMD", false),
        k("evacreset", "EVAC RESET", false),
        k("smoke", "SMOKE RESET", false),
        k("pcreset", "FAP - PC RESET", false),
    ]
}

fn body(page: Page, s: &FapSim, st: &serde_json::Value) -> PageBody {
    let pa = st["paCmd"].as_u64().unwrap_or(0) as u8;
    match page {
        Page::Music => PageBody::Music { playing: pa == 1, track: 0, volume_pct: 100 },
        Page::PreAnn => PageBody::PreAnn { selected: if pa > 1 { Some(pa) } else { None }, left_s: 0., auto_enabled: s.ann_enabled, deck_ok: [true; 3] },
        Page::Lights => PageBody::Lights {
            scene: if s.scene_raw > 0. { Some(s.scene_raw as u8) } else { None },
            level_pct: if s.level_raw > 0. { Some(s.level_raw as u8 - 1) } else { None },
            seatbelt: false,
            no_smoking: false,
            emer_lights: s.emer_light,
            exit_signs: false,
        },
        Page::Doors => PageBody::Doors {
            doors: DOORS
                .iter()
                .zip(s.doors.iter())
                .map(|(d, m)| DoorView {
                    name: d.0,
                    state: match m {
                        DoorMark::Open => DoorState::Open,
                        DoorMark::Moving => DoorState::Moving,
                        _ => DoorState::Closed,
                    },
                    slide: if *m == DoorMark::Armed { SlideState::Armed } else { SlideState::Disarmed },
                    fault: false,
                })
                .collect(),
            slides_armed: s.doors.iter().filter(|m| **m == DoorMark::Armed).count(),
            pax_closed: true,
        },
        Page::Seats => PageBody::Seats { seat_power: !s.seat_power_off, pax_sys: !s.pax_sys_off, heaters: !s.heaters_off, galley: [!s.galley_off[0], !s.galley_off[1], !s.galley_off[2]] },
        Page::Ready => {
            let mut areas = [false; 6];
            for (i, (id, _)) in AREAS.iter().enumerate() {
                areas[i] = st["areas"][*id].as_bool().unwrap_or(false);
            }
            PageBody::Ready { areas, checks: Vec::new(), ready: s.cabin_ready, incomplete: false }
        }
        Page::SysInfo => PageBody::SysInfo { directors: [!s.cids_fault[0], !s.cids_fault[1], !s.cids_fault[2]], active: None, channels: [0.; 4], lines: Vec::new() },
        _ => PageBody::Status(Vec::new()),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: fap_dump <fixture.json> <page> <out.json>");
        std::process::exit(2);
    }
    let fx: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&args[1]).expect("fixture")).expect("fixture json");
    let page = page_from_id(&args[2]).expect("page id");
    let sim = FapSim::read(|n| fx["vars"][n].as_f64());
    let st = &fx["pages"][&args[2]];
    let view = FapView {
        mode: ViewMode::Live,
        boot_remaining_s: 0.,
        page,
        title: page.title(),
        cautions: legacy_cautions(&sim),
        message: None,
        keys: legacy_keys(&sim),
        switches: vec![SwitchView { id: "emer", label: "EMER LIGHT", up: sim.emer_light }, SwitchView { id: "paxsys", label: "PAX SYS", up: !sim.pax_sys_off }],
        locked: false,
        body: body(page, &sim, st),
    };
    let ui = Ui { nav: LEGACY_NAV.to_vec(), ..Ui::default() };
    std::fs::write(&args[3], fap_root(&view, &sim, &ui).to_json()).expect("write");
}

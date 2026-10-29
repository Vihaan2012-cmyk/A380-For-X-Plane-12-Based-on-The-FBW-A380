use crate::doors::DoorView;
use crate::maint::TestResult;
use crate::ready::Check;
use crate::types::*;
use crate::Cids;

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub label: String,
    pub value: String,
    pub level: Level,
}

impl Row {
    fn new(label: &str, value: impl Into<String>, level: Level) -> Self {
        Self { label: label.to_string(), value: value.into(), level }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HardKey {
    pub id: &'static str,
    pub label: &'static str,
    pub lit: bool,
    pub alert: bool,
    pub guard: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SwitchView {
    pub id: &'static str,
    pub label: &'static str,
    pub up: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TempRow {
    pub area: &'static str,
    pub set_c: f64,
    pub actual_c: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CallLine {
    pub text: String,
    pub age_s: f64,
    pub answered: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PageBody {
    Status(Vec<Row>),
    Music { playing: bool, track: u8, volume_pct: u8 },
    PreAnn { selected: Option<u8>, left_s: f64, auto_enabled: bool, deck_ok: [bool; 3] },
    Lights { scene: Option<u8>, level_pct: Option<u8>, seatbelt: bool, no_smoking: bool, emer_lights: bool, exit_signs: bool },
    Doors { doors: Vec<DoorView>, slides_armed: usize, pax_closed: bool },
    Temp { rows: Vec<TempRow>, ckpt_c: f64, purser_active: bool },
    Water { water_pct: f64, waste_pct: [f64; ZONES], waste_full: [bool; ZONES], lav_inop: [bool; ZONES], fault: bool },
    Smoke { rows: Vec<(String, bool)>, horn: bool },
    Seats { seat_power: bool, pax_sys: bool, heaters: bool, galley: [bool; ZONES] },
    Ready { areas: [bool; AREAS], checks: Vec<Check>, ready: bool, incomplete: bool },
    SysInfo { directors: [bool; 3], active: Option<usize>, channels: [f64; 4], lines: Vec<Row> },
    Calls { log: Vec<CallLine>, pending: usize, emer: bool },
    Evac { active: bool, cockpit_cmd: bool, cabin_cmd: bool, capt_only: bool, guard_open: bool, horn_cockpit: bool },
    Maintenance { result: TestResult, test_allowed: bool, lamp_test: bool },
    Fault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Dark,
    Booting,
    ScreenOff,
    Live,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FapView {
    pub mode: ViewMode,
    pub boot_remaining_s: f64,
    pub page: Page,
    pub title: &'static str,
    pub cautions: Vec<Caution>,
    pub message: Option<String>,
    pub keys: Vec<HardKey>,
    pub switches: Vec<SwitchView>,
    pub locked: bool,
    pub body: PageBody,
}

fn on_off(x: bool) -> &'static str {
    if x {
        "ON"
    } else {
        "OFF"
    }
}

impl Cids {
    pub fn view(&self) -> FapView {
        let o = &self.last;
        let i = &self.last_in;
        let powered = i.power.ac_cabin || i.power.dc_ess;
        let mode = if !powered {
            ViewMode::Dark
        } else if self.fap.boot_s > 0. {
            ViewMode::Booting
        } else if self.fap.off {
            ViewMode::ScreenOff
        } else {
            ViewMode::Live
        };
        let keys = if mode == ViewMode::Dark || mode == ViewMode::Booting {
            Vec::new()
        } else {
            vec![
                HardKey { id: "ped", label: "PED POWER", lit: o.seat_power, alert: false, guard: false },
                HardKey { id: "lights", label: "LIGHTS MAIN ON/OFF", lit: self.fap.level_pct != Some(0), alert: false, guard: false },
                HardKey { id: "lav", label: "LAV MAINT", lit: self.fap.lav_maint, alert: false, guard: false },
                HardKey { id: "lock", label: "SCREEN 30 sec LOCK", lit: self.fap.lock_s > 0., alert: false, guard: false },
                HardKey { id: "evac", label: "EVAC CMD", lit: false, alert: o.evac.fap_flash, guard: o.evac.guard_open },
                HardKey { id: "evacreset", label: "EVAC RESET", lit: false, alert: false, guard: false },
                HardKey { id: "smoke", label: "SMOKE RESET", lit: o.smoke_horn, alert: self.smoke_out.any, guard: false },
                HardKey { id: "callreset", label: "CALL RESET", lit: o.calls.pending > 0, alert: o.calls.cockpit_emer_light, guard: false },
                HardKey { id: "pcreset", label: "FAP - PC RESET", lit: false, alert: false, guard: false },
            ]
        };
        let switches = vec![
            SwitchView { id: "emer", label: "EMER LIGHT", up: self.fap.emer_light },
            SwitchView { id: "paxsys", label: "PAX SYS", up: !self.fap.pax_sys_off },
        ];
        let body = match mode {
            ViewMode::Live if !o.cids_ok => PageBody::Fault,
            ViewMode::Live => self.body(self.fap.page),
            _ => PageBody::Fault,
        };
        FapView {
            mode,
            boot_remaining_s: self.fap.boot_s,
            page: self.fap.page,
            title: self.fap.page.title(),
            cautions: if mode == ViewMode::Live { o.cautions.iter().take(3).cloned().collect() } else { Vec::new() },
            message: self.fap.msg.as_ref().map(|m| m.0.clone()),
            keys,
            switches,
            locked: self.fap.lock_s > 0.,
            body,
        }
    }

    fn body(&self, page: Page) -> PageBody {
        let o = &self.last;
        let i = &self.last_in;
        match page {
            Page::Status => {
                let open = self.doors_out.views.iter().filter(|d| d.state != crate::doors::DoorState::Closed).count();
                let mut rows = vec![
                    Row::new("DOORS", if open == 0 { "ALL CLOSED".to_string() } else { format!("{open} NOT CLOSED") }, if open == 0 { Level::White } else { Level::Amber }),
                    Row::new("SLIDES", format!("{} ARMED", self.doors_out.slides_armed), Level::White),
                    Row::new("SEAT BELTS", on_off(o.seatbelt_sign), Level::White),
                    Row::new("NO SMOKING", on_off(o.no_smoking_sign), Level::White),
                    Row::new("EMER LIGHTS", on_off(o.emer_lights), if o.emer_lights { Level::Amber } else { Level::White }),
                    Row::new("CABIN READY", if o.cabin_ready { "READY" } else { "NOT READY" }, Level::White),
                ];
                if o.evac.active {
                    rows.push(Row::new("EVAC", "ACTIVE", Level::Red));
                }
                if self.smoke_out.any {
                    rows.push(Row::new("SMOKE", "DETECTED", Level::Red));
                }
                if o.calls.pending > 0 {
                    rows.push(Row::new("CALLS", o.calls.pending.to_string(), Level::Cyan));
                }
                rows.push(Row::new("DIRECTORS", format!("{} OF 3", self.directors.healthy_count()), if self.directors.degraded() { Level::Amber } else { Level::White }));
                PageBody::Status(rows)
            }
            Page::Music => PageBody::Music { playing: o.pa.music_on, track: o.pa.music_track, volume_pct: (self.pa_volume_pct()) },
            Page::PreAnn => PageBody::PreAnn { selected: o.pa.announcement, left_s: o.pa.announcement_left_s, auto_enabled: true, deck_ok: o.pa.deck_ok },
            Page::Lights => PageBody::Lights {
                scene: self.fap.scene,
                level_pct: self.fap.level_pct,
                seatbelt: o.seatbelt_sign,
                no_smoking: o.no_smoking_sign,
                emer_lights: o.emer_lights,
                exit_signs: o.exit_signs,
            },
            Page::Doors => PageBody::Doors { doors: self.doors_out.views.clone(), slides_armed: self.doors_out.slides_armed, pax_closed: self.doors_out.pax_closed },
            Page::Temp => PageBody::Temp {
                rows: (0..AREAS).map(|a| TempRow { area: AREA_NAMES[a], set_c: o.temp.setpoints[a], actual_c: i.cabin.zone_temp_c[a] }).collect(),
                ckpt_c: i.cabin.ckpt_temp_c,
                purser_active: o.temp.purser_active,
            },
            Page::Water => PageBody::Water {
                water_pct: i.cabin.water_qty_pct,
                waste_pct: i.cabin.waste_level_pct,
                waste_full: i.cabin.waste_full,
                lav_inop: i.cabin.lav_inop,
                fault: i.cabin.water_fault,
            },
            Page::Smoke => PageBody::Smoke {
                rows: (0..SMOKE_LOCS).map(|n| (SMOKE_NAMES[n].to_string(), self.smoke_out.active[n])).collect(),
                horn: o.smoke_horn,
            },
            Page::Seats => PageBody::Seats { seat_power: o.seat_power, pax_sys: o.pax_sys, heaters: o.water_heaters, galley: o.galley_on },
            Page::Ready => PageBody::Ready { areas: self.ready_out.areas, checks: self.ready_out.checks.clone(), ready: o.cabin_ready, incomplete: o.cabin_ready_incomplete },
            Page::SysInfo => {
                let mut lines = Vec::new();
                for (n, name) in ["PA UPPER", "PA MAIN", "PA LOWER", "INTERPHONE"].iter().enumerate() {
                    let f = i.cabin.channel_fault[n];
                    let (v, l) = if f >= 1. {
                        ("FAULT", Level::Amber)
                    } else if f > 0. {
                        ("DEGRADED", Level::Amber)
                    } else {
                        ("OK", Level::White)
                    };
                    lines.push(Row::new(name, v, l));
                }
                PageBody::SysInfo { directors: self.directors.healthy, active: self.directors.active, channels: i.cabin.channel_fault, lines }
            }
            Page::Calls => PageBody::Calls {
                log: self
                    .calls
                    .log
                    .iter()
                    .map(|r| CallLine { text: call_text(r.kind), age_s: r.age_s, answered: r.answered })
                    .collect(),
                pending: o.calls.pending,
                emer: o.calls.cockpit_emer_light || o.calls.emer_to_cabin,
            },
            Page::Evac => PageBody::Evac {
                active: o.evac.active,
                cockpit_cmd: i.cockpit.evac_cmd,
                cabin_cmd: o.evac.from_cabin,
                capt_only: i.cockpit.evac_capt_only,
                guard_open: o.evac.guard_open,
                horn_cockpit: o.evac.horn_cockpit,
            },
            Page::Maintenance => PageBody::Maintenance {
                result: self.test_result(),
                test_allowed: o.phase == Phase::Parked && i.flight.parking_brake && i.flight.engines_off(),
                lamp_test: o.lamp_test,
            },
        }
    }

    fn pa_volume_pct(&self) -> u8 {
        self.pa.volume_pct()
    }
}

fn call_text(k: crate::calls::CallKind) -> String {
    use crate::calls::CallKind::*;
    match k {
        Attendant(z) => format!("ATTENDANT CALL {}", ["FWD", "MID", "AFT"][z.min(2)]),
        Purser => "PURSER CALL".to_string(),
        Emergency => "EMERGENCY CALL".to_string(),
        Pilot(0) => "COCKPIT CALL ALL".to_string(),
        Pilot(1) => "COCKPIT CALL FWD".to_string(),
        Pilot(2) => "COCKPIT CALL AFT".to_string(),
        Pilot(_) => "COCKPIT CALL PURSER".to_string(),
        Mech => "GROUND MECH CALL".to_string(),
    }
}

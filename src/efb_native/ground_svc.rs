//! The Ground Services page's own work: what each request sends, the automation switches, and the status column's
//! services in progress and activity log.

use std::collections::{BTreeMap, HashMap};

use efb_pages::ground::{gs_setting, Running, ServicesView, GS_NOTIFY};

use crate::navdata::Stand;

/// What the engine carries out for the services.
#[derive(Debug, PartialEq)]
pub enum Do {
    Command(String),
    Press(&'static str),
    Set(&'static str, f64),
    Toast(String),
    CallGround,
}

enum Kind {
    Places(&'static [&'static str]),
    Svc(&'static str),
}

const TRACKED: [(&str, &str, Kind); 12] = [
    ("catering", "Catering", Kind::Places(&["cat_m2r", "udcat_u1r"])),
    ("fuel", "Fuel truck", Kind::Places(&["fuel_hydrant", "fuel_tanker"])),
    ("cargo", "Cargo loaders", Kind::Places(&["ldcl_fwd", "uld_fwd", "ldcl_aft", "uld_aft", "belt_bulk", "bag_tractor", "bulk_cart"])),
    ("stairs", "Stairs", Kind::Places(&["stairs_m1l", "stairs_m2l", "stairs_u1l"])),
    ("gpu", "Ground power", Kind::Svc("gpu")),
    ("asu", "Air start unit", Kind::Svc("asu")),
    ("pca", "Preconditioned air", Kind::Svc("pca")),
    ("water", "Potable water", Kind::Svc("water")),
    ("lav", "Lavatory service", Kind::Svc("lav")),
    ("clean", "Cabin cleaning", Kind::Svc("clean")),
    ("deice", "De-icing", Kind::Svc("deice")),
    ("tug", "Tug", Kind::Svc("tug")),
];

/// Sent away when the aircraft is readied for departure; power, air and the tug stay for the start and push.
const LEAVE_FOR_DEPARTURE: [&str; 7] = ["catering", "fuel", "cargo", "stairs", "water", "lav", "clean"];
const STAIRS: [&str; 3] = ["stairs_m1l", "stairs_m2l", "stairs_u1l"];
const JETWAY_DOORS: [&str; 3] = ["m1l", "m2l", "u1l"];
const DOORS: usize = 18;
const LOG_LINES: usize = 7;

#[derive(Clone, Copy, Default, PartialEq)]
enum Depart {
    #[default]
    Idle,
    Preparing,
    Ready,
}

/// The last second's readings, kept so a press can act on them at once.
#[derive(Default)]
pub struct Reading {
    pub vars: HashMap<String, f64>,
    pub gse: HashMap<String, f64>,
    pub efb: HashMap<&'static str, f64>,
    pub settings: BTreeMap<String, String>,
}

pub const EFB_VALUES: [&str; 6] = ["payload/pax", "payload/pax_desired", "payload/boarding_started", "payload/boarding_eta_s", "refuel/started", "refuel/total_kg"];
pub const EFB_TARGET: &str = "refuel/target_kg";

#[derive(Default)]
pub struct GroundServices {
    pub view: String,
    pub more: bool,
    stand: Option<Stand>,
    looked_at: Option<(f64, f64)>,
    next_look_ms: f64,
    seen: HashMap<&'static str, (u32, f64)>,
    progress: HashMap<&'static str, f64>,
    jetways: Option<usize>,
    boarding: bool,
    deboarding: bool,
    refuelling: bool,
    boarded_here: bool,
    depart: Depart,
    depart_since: f64,
    log: Vec<(String, String)>,
    r: Reading,
    now: f64,
    clock: String,
    out: Vec<Do>,
}

impl GroundServices {
    fn var(&self, k: &str) -> f64 {
        self.r.vars.get(k).copied().unwrap_or(0.0)
    }

    fn efb(&self, k: &str) -> f64 {
        self.r.efb.get(k).copied().unwrap_or(0.0)
    }

    fn gse(&self, k: &str) -> f64 {
        self.r.gse.get(k).copied().unwrap_or(0.0)
    }

    fn setting(&self, key: &str, default: bool) -> bool {
        gs_setting(&self.r.settings, key, default)
    }

    fn state_of(&self, kind: &Kind) -> u32 {
        match kind {
            Kind::Places(ps) => ps.iter().map(|p| self.gse(&format!("place/{p}/state"))).fold(0.0, f64::max) as u32,
            Kind::Svc("gpu") if self.gse("connected/gpu") > 0.5 || self.var("A32NX_EFB_GROUND_GPU_CONNECTED") > 0.5 => 2,
            Kind::Svc(s) => self.gse(&format!("{s}/state")) as u32,
        }
    }

    fn state(&self, key: &str) -> u32 {
        TRACKED.iter().find(|t| t.0 == key).map_or(0, |t| self.state_of(&t.2))
    }

    fn present(&self, key: &str) -> bool {
        matches!(self.state(key), 1 | 2)
    }

    fn jetways(&self) -> usize {
        JETWAY_DOORS.iter().filter(|d| self.gse(&format!("jetway_{d}/state")) > 1.0).count()
    }

    fn on_ground(&self) -> bool {
        self.var("SIM ON GROUND") > 0.5
    }

    fn parked(&self) -> bool {
        self.on_ground() && self.var("GROUND VELOCITY").abs() < 1.0
    }

    fn pax(&self) -> (f64, f64) {
        (self.efb("payload/pax"), self.efb("payload/pax_desired"))
    }

    fn line(&mut self, text: String) {
        if self.setting(GS_NOTIFY, true) {
            self.out.push(Do::Toast(text.clone()));
        }
        self.log.push((self.clock.clone(), text));
        if self.log.len() > LOG_LINES {
            self.log.remove(0);
        }
    }

    fn cmd(&mut self, c: String) {
        self.out.push(Do::Command(c));
    }

    /// Calls a service that is not there, or sends away one that is.
    fn toggle(&mut self, key: &str) {
        let Some((_, _, kind)) = TRACKED.iter().find(|t| t.0 == key) else { return };
        match kind {
            Kind::Svc("gpu") => self.cmd("fbw/efb/ground/gpu".into()),
            Kind::Svc("tug") => self.cmd("fbw/pushback/toggle".into()),
            Kind::Svc(s) => self.cmd(format!("fbw/gse/{s}/toggle")),
            Kind::Places(ps) => {
                let here: Vec<&str> = ps.iter().copied().filter(|p| matches!(self.gse(&format!("place/{p}/state")) as i64, 1 | 2)).collect();
                for p in if here.is_empty() { ps.to_vec() } else { here } {
                    self.cmd(format!("fbw/gse/place/{p}/toggle"));
                }
            }
        }
    }

    fn call(&mut self, key: &str) {
        if !self.present(key) {
            self.toggle(key);
        }
    }

    fn send_away(&mut self, key: &str) {
        if self.present(key) {
            self.toggle(key);
        }
    }

    fn call_fuel_truck(&mut self) {
        if self.present("fuel") {
            return;
        }
        let at_gate = self.stand.as_ref().is_some_and(|s| s.kind == "gate");
        self.cmd(format!("fbw/gse/place/{}/toggle", if at_gate { "fuel_hydrant" } else { "fuel_tanker" }));
    }

    fn bring_catering(&mut self) {
        if !self.present("catering") && self.state("catering") != 3 {
            self.toggle("catering");
        }
    }

    /// A press of one of the page's requests.
    pub fn request(&mut self, what: &str) -> Vec<Do> {
        if !self.parked() && what != "jetway" {
            self.out.push(Do::Toast("Only while the aircraft is stopped on the ground".into()));
            return std::mem::take(&mut self.out);
        }
        if what != "depart" && what != "jetway" {
            self.depart = Depart::Idle;
        }
        let (pax, desired) = self.pax();
        let started = self.efb("payload/boarding_started") > 0.5;
        match what {
            "deboard" => {
                if started && desired < pax {
                    self.out.push(Do::Set("payload/boarding_started", 0.0));
                    self.line("Deboarding stopped".into());
                } else {
                    self.out.push(Do::Press("payload/deboard"));
                    self.call("cargo");
                    self.line("Deboarding requested".into());
                }
            }
            "catering" => {
                let here = self.present("catering");
                self.toggle("catering");
                self.line(if here { "Catering sent away" } else { "Catering requested" }.into());
            }
            "refuel" => {
                let refuelling = self.efb("refuel/started") > 0.5;
                self.out.push(Do::Press("refuel/start_stop"));
                if refuelling {
                    self.line("Refuelling stopped".into());
                } else {
                    self.call_fuel_truck();
                    self.line("Refuelling requested".into());
                }
            }
            "board" => {
                if started && desired > pax {
                    self.out.push(Do::Set("payload/boarding_started", 0.0));
                    self.line("Boarding stopped".into());
                } else {
                    self.out.push(Do::Set("payload/boarding_started", 1.0));
                    self.call("cargo");
                    if self.setting("xphfbw.gsAutoCatering", false) {
                        self.bring_catering();
                    }
                    self.line("Boarding requested".into());
                }
            }
            "depart" => self.prepare_departure(),
            "jetway" => {
                self.cmd("fbw/efb/ground/jetway".into());
                self.line(if self.jetways() > 0 { "Jetway retracting" } else { "Jetway requested" }.into());
            }
            "stairs" => {
                if self.present("stairs") {
                    self.toggle("stairs");
                    self.line("Stairs sent away".into());
                } else {
                    let bridged = self.jetways();
                    for p in &STAIRS[bridged.min(STAIRS.len())..] {
                        self.cmd(format!("fbw/gse/place/{p}/toggle"));
                    }
                    self.line("Stairs requested".into());
                }
            }
            _ => {}
        }
        std::mem::take(&mut self.out)
    }

    fn prepare_departure(&mut self) {
        let (pax, desired) = self.pax();
        if self.efb("payload/boarding_started") > 0.5 && (desired - pax).abs() >= 0.5 {
            self.line("Boarding is still in progress".into());
            return;
        }
        if self.efb("refuel/started") > 0.5 {
            self.line("Refuelling is still in progress".into());
            return;
        }
        for key in LEAVE_FOR_DEPARTURE {
            self.send_away(key);
        }
        for i in 0..DOORS {
            if self.var(&format!("INTERACTIVE POINT OPEN:{i}")) > 2.0 {
                self.cmd(format!("fbw/door/{}/toggle", crate::doors::NAMES[i]));
            }
        }
        if self.jetways() > 0 {
            self.cmd("fbw/efb/ground/jetway".into());
        }
        self.depart = Depart::Preparing;
        self.depart_since = self.now;
        self.line("Preparing for departure: services leaving, doors closing".into());
    }

    fn departure_clear(&self) -> bool {
        LEAVE_FOR_DEPARTURE.iter().all(|k| self.state(k) == 0) && (0..DOORS).all(|i| self.var(&format!("INTERACTIVE POINT OPEN:{i}")) <= 2.0) && self.jetways() == 0
    }

    fn look_up_stand(&mut self) {
        let (lat, lon) = (self.var("PLANE LATITUDE"), self.var("PLANE LONGITUDE"));
        if !self.on_ground() {
            self.stand = None;
            self.looked_at = None;
            return;
        }
        let moved = self.looked_at.is_none_or(|(a, b)| crate::navdata::distance_m(a, b, lat, lon) > 40.0);
        if !self.parked() || !moved || self.now < self.next_look_ms || (lat == 0.0 && lon == 0.0) {
            return;
        }
        self.next_look_ms = self.now + 5000.0;
        if let Some(s) = crate::native_fms_shadow::stand_at(lat, lon) {
            self.stand = Some(s);
            self.looked_at = Some((lat, lon));
        }
    }

    /// Once a second, after the ground engineer: the readings, and whether it just saw the aircraft arrive.
    pub fn tick(&mut self, now: f64, clock: &str, r: Reading, arrived: bool) -> Vec<Do> {
        self.now = now;
        self.clock = clock.trim_end_matches('Z').to_string();
        self.r = r;
        self.look_up_stand();
        if !self.on_ground() {
            self.depart = Depart::Idle;
            self.boarded_here = false;
        }
        for (key, label, kind) in &TRACKED {
            let st = self.state_of(kind);
            let was = self.seen.get(key).map(|s| s.0);
            if was != Some(st) {
                self.seen.insert(*key, (st, now));
                if let Some(was) = was {
                    let text = match st {
                        1 => format!("{label} on the way"),
                        2 => format!("{label} in place"),
                        3 => format!("{label} leaving"),
                        _ if was == 3 => format!("{label} gone"),
                        _ => format!("{label} removed"),
                    };
                    self.line(text);
                }
            }
        }
        let jet = self.jetways();
        if let Some(was) = self.jetways.replace(jet) {
            if jet > was {
                self.line("Jetway docked".into());
            } else if jet < was {
                self.line("Jetway retracted".into());
            }
        }
        let (pax, desired) = self.pax();
        let started = self.efb("payload/boarding_started") > 0.5;
        let boarding = started && desired > pax + 0.5;
        let deboarding = started && desired + 0.5 < pax;
        let refuelling = self.efb("refuel/started") > 0.5;
        let auto_catering = self.setting("xphfbw.gsAutoCatering", false);
        if boarding && !self.boarding {
            self.progress.insert("boarding", now);
            self.line("Boarding started".into());
        }
        if !boarding && self.boarding {
            if (desired - pax).abs() < 0.5 {
                self.boarded_here = true;
                self.line(format!("Boarding complete, {} aboard", pax.round()));
            }
        }
        if deboarding && !self.deboarding {
            self.progress.insert("deboarding", now);
            self.line("Deboarding started".into());
        }
        if !deboarding && self.deboarding && pax < 0.5 {
            self.line("Deboarding complete".into());
            if auto_catering {
                self.bring_catering();
            }
        }
        if refuelling && !self.refuelling {
            self.progress.insert("refuel", now);
            self.line("Refuelling started".into());
        }
        if !refuelling && self.refuelling {
            self.line(format!("Refuelling complete, {:.1} t aboard", self.efb("refuel/total_kg") / 1000.0));
            self.send_away("fuel");
        }
        self.boarding = boarding;
        self.deboarding = deboarding;
        self.refuelling = refuelling;
        if arrived {
            self.depart = Depart::Idle;
            self.boarded_here = false;
            if self.setting("xphfbw.gsAutoGpu", false) && !self.present("gpu") {
                self.toggle("gpu");
            }
            if self.setting("xphfbw.gsAutoJetway", false) && jet == 0 {
                self.cmd("fbw/efb/ground/jetway".into());
            }
            if self.setting("xphfbw.gsAutoDeboard", false) && pax >= 0.5 {
                self.out.push(Do::Press("payload/deboard"));
                self.call("cargo");
                self.line("Deboarding on arrival".into());
            }
        }
        if self.setting("xphfbw.gsAutoPushback", false) && self.boarded_here && !refuelling && self.depart == Depart::Idle && self.parked() {
            self.boarded_here = false;
            self.prepare_departure();
        }
        if self.depart == Depart::Preparing && self.departure_clear() {
            self.depart = Depart::Ready;
            self.line("Ready for pushback".into());
            if self.setting("xphfbw.gsAutoPushback", false) {
                self.out.push(Do::CallGround);
            }
        }
        if self.depart != Depart::Idle && self.var("GROUND VELOCITY").abs() > 3.0 && self.var("PUSHBACK ATTACHED") < 0.5 {
            self.depart = Depart::Idle;
        }
        std::mem::take(&mut self.out)
    }

    /// The page's model of the services.
    pub fn model(&self, route: String) -> ServicesView {
        let (pax, desired) = self.pax();
        let mut running: Vec<Running> = Vec::new();
        for (key, label, kind) in &TRACKED {
            let st = self.state_of(kind);
            if st == 0 {
                continue;
            }
            let since = self.seen.get(key).map_or(self.now, |s| s.1);
            let state = ["", "On the way", "In place", "Leaving"].get(st as usize).copied().unwrap_or("");
            running.push(Running { label: label.to_string(), state: state.into(), secs: (self.now - since) / 1000.0, done: st == 2 });
        }
        let since = |k: &str| (self.now - self.progress.get(k).copied().unwrap_or(self.now)) / 1000.0;
        if self.boarding {
            running.insert(0, Running { label: "Boarding".into(), state: format!("{} of {}", pax.round(), desired.round()), secs: since("boarding"), done: false });
        }
        if self.deboarding {
            running.insert(0, Running { label: "Deboarding".into(), state: format!("{} aboard", pax.round()), secs: since("deboarding"), done: false });
        }
        if self.refuelling {
            running.insert(0, Running { label: "Refuelling".into(), state: format!("{:.1} t", self.efb("refuel/total_kg") / 1000.0), secs: since("refuel"), done: false });
        }
        let situation = if !self.on_ground() {
            "Airborne"
        } else if self.var("PUSHBACK ATTACHED") > 0.5 && self.var("GROUND VELOCITY").abs() > 0.3 {
            "Being pushed back"
        } else if self.var("GROUND VELOCITY").abs() > 1.0 {
            "Taxiing"
        } else {
            "Parked"
        };
        let departure = match self.depart {
            Depart::Idle => String::new(),
            Depart::Preparing => format!("Preparing, {}", mins_text((self.now - self.depart_since) / 1000.0)),
            Depart::Ready => "Ready for pushback".into(),
        };
        let s = self.stand.clone().unwrap_or_default();
        ServicesView {
            icao: s.icao,
            airport: s.airport,
            gate: s.gate.unwrap_or_default(),
            gate_kind: s.kind,
            situation: situation.into(),
            route,
            pax,
            pax_planned: desired,
            boarding: self.boarding,
            deboarding: self.deboarding,
            board_eta_s: self.efb("payload/boarding_eta_s"),
            fuel_kg: self.efb("refuel/total_kg"),
            fuel_target_kg: self.efb(EFB_TARGET),
            refuelling: self.refuelling,
            departure,
            running,
            log: self.log.clone(),
        }
    }
}

fn mins_text(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(vars: &[(&str, f64)], gse: &[(&str, f64)], efb: &[(&'static str, f64)]) -> Reading {
        Reading {
            vars: vars.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            gse: gse.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            efb: efb.iter().copied().collect(),
            settings: BTreeMap::new(),
        }
    }

    const PARKED: [(&str, f64); 2] = [("SIM ON GROUND", 1.0), ("GROUND VELOCITY", 0.0)];

    #[test]
    fn boarding_brings_the_cargo_loaders_and_starts_the_flypad_boarding() {
        let mut g = GroundServices::default();
        g.tick(0.0, "14:01Z", reading(&PARKED, &[], &[("payload/pax_desired", 450.0)]), false);
        let out = g.request("board");
        assert!(out.contains(&Do::Set("payload/boarding_started", 1.0)));
        for p in ["ldcl_fwd", "uld_fwd", "ldcl_aft", "uld_aft", "belt_bulk", "bag_tractor", "bulk_cart"] {
            assert!(out.contains(&Do::Command(format!("fbw/gse/place/{p}/toggle"))), "{p}");
        }
    }

    #[test]
    fn departure_waits_for_the_services_and_doors_then_reports_ready() {
        let mut g = GroundServices::default();
        let mut vars = PARKED.to_vec();
        vars.push(("INTERACTIVE POINT OPEN:0", 100.0));
        g.tick(0.0, "14:01Z", reading(&vars, &[("place/cat_m2r/state", 2.0)], &[]), false);
        let out = g.request("depart");
        assert!(out.contains(&Do::Command("fbw/gse/place/cat_m2r/toggle".into())));
        assert!(out.contains(&Do::Command(format!("fbw/door/{}/toggle", crate::doors::NAMES[0]))));
        g.tick(1000.0, "14:01Z", reading(&vars, &[("place/cat_m2r/state", 3.0)], &[]), false);
        assert_eq!(g.model(String::new()).departure, "Preparing, 0:01");
        g.tick(2000.0, "14:02Z", reading(&PARKED, &[], &[]), false);
        assert_eq!(g.model(String::new()).departure, "Ready for pushback");
        assert!(g.log.iter().any(|l| l.1 == "Catering gone"));
    }

    #[test]
    fn departure_is_refused_while_boarding() {
        let mut g = GroundServices::default();
        let efb = [("payload/boarding_started", 1.0), ("payload/pax", 100.0), ("payload/pax_desired", 450.0)];
        g.tick(0.0, "14:01Z", reading(&PARKED, &[], &efb), false);
        let out = g.request("depart");
        assert!(!out.iter().any(|d| matches!(d, Do::Command(_))));
        assert_eq!(g.log.last().unwrap().1, "Boarding is still in progress");
    }

    #[test]
    fn stairs_skip_the_doors_with_a_jetway() {
        let mut g = GroundServices::default();
        g.tick(0.0, "14:01Z", reading(&PARKED, &[("jetway_m1l/state", 2.0)], &[]), false);
        let out = g.request("stairs");
        assert!(!out.contains(&Do::Command("fbw/gse/place/stairs_m1l/toggle".into())));
        assert!(out.contains(&Do::Command("fbw/gse/place/stairs_m2l/toggle".into())));
        assert!(out.contains(&Do::Command("fbw/gse/place/stairs_u1l/toggle".into())));
    }

    #[test]
    fn arrival_runs_only_the_automations_that_are_on() {
        let mut g = GroundServices::default();
        let mut r = reading(&PARKED, &[], &[("payload/pax", 300.0)]);
        r.settings.insert("xphfbw.gsAutoGpu".into(), "1".into());
        let out = g.tick(0.0, "14:01Z", r, true);
        assert!(out.contains(&Do::Command("fbw/efb/ground/gpu".into())));
        assert!(!out.contains(&Do::Press("payload/deboard")));
        assert!(!out.contains(&Do::Command("fbw/efb/ground/jetway".into())));
    }
}

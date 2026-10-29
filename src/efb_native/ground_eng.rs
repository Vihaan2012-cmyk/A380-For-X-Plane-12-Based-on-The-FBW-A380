//! The ground engineer on the interphone: answers the cockpit's requests,
//! runs the pushback sequence with the tug and services the aircraft
//! (the EFB's ground_comms.js, natively).

use std::collections::HashMap;

pub const CHOCKS: [&str; 5] = ["nose", "lwing", "rwing", "lbody", "rbody"];
pub const COVERS: [&str; 20] = [
    "pitot_cover_1", "pitot_cover_2", "pitot_cover_3", "static_covers", "aoa_cover_1", "aoa_cover_2", "aoa_cover_3", "eng_inlet_cover_1", "eng_inlet_cover_2", "eng_inlet_cover_3",
    "eng_inlet_cover_4", "eng_exhaust_cover_1", "eng_exhaust_cover_2", "eng_exhaust_cover_3", "eng_exhaust_cover_4", "gear_pin_nose", "gear_pin_lwing", "gear_pin_rwing", "gear_pin_lbody",
    "gear_pin_rbody",
];

const ARRIVE_MS: f64 = 12000.0;
const ARRIVAL_MS: f64 = 20000.0;
const DISCONNECT_MS: f64 = 6000.0;
const BYE_MS: f64 = 4000.0;
const ANSWER_MS: f64 = 1500.0;

pub fn var_names() -> Vec<String> {
    let mut v: Vec<String> = ["SIM ON GROUND", "GROUND VELOCITY", "PUSHBACK ATTACHED", "A32NX_PARK_BRAKE_LEVER_POS", "LIGHT BEACON", "PUSH_OVHD_CALLS_MECH", "A32NX_EFB_GROUND_GPU_CONNECTED", "A32NX_REFUEL_STARTED_BY_USR", "A32NX_BOARDING_STARTED_BY_USR"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    v.extend((1..=4).map(|n| format!("A32NX_ENGINE_STATE:{n}")));
    v.extend((0..18).map(|i| format!("INTERACTIVE POINT OPEN:{i}")));
    v.extend(CHOCKS.iter().map(|c| format!("A32NX_WALKAROUND_CHOCKS_{}", c.to_uppercase())));
    v.extend(COVERS.iter().map(|c| format!("A32NX_WALKAROUND_{}", c.to_uppercase())));
    v
}

fn text(key: &str) -> &'static str {
    match key {
        "go_ahead" => "Cockpit, ground. Go ahead.",
        "on_headset" => "Cockpit, ground. I'm on the headset.",
        "release_brakes" => "Roger. Release the parking brake, please.",
        "push_start" => "Brakes released. Commencing pushback.",
        "tug_coming" => "Standby, cockpit. The tug is on its way.",
        "tug_wait" => "Standby. Connecting the tug.",
        "engines_clear" => "Area is clear. You are cleared to start engines.",
        "push_complete" => "Pushback complete. Set the parking brake, please.",
        "disconnect" => "Parking brake set. Disconnecting. Steering pin removed. Hand signals on your left.",
        "bye" => "Disconnected. Have a nice flight.",
        "stop" => "Stopping the pushback.",
        "arrival" => "Cockpit, ground. Chocks are in place. You can release the parking brake.",
        "chocks_in" => "Chocks in place.",
        "chocks_out" => "Chocks removed.",
        "gpu_on" => "Ground power connected and available.",
        "gpu_off" => "Ground power disconnected.",
        "asu_on" => "Air start unit connected.",
        "asu_off" => "Air start unit removed.",
        "pca_on" => "Preconditioned air connected.",
        "pca_off" => "Preconditioned air disconnected.",
        "covers_off" => "All covers and pins removed. Walkaround complete.",
        "refuelling" => "Negative, cockpit. Refuelling is still in progress.",
        "doors_open" => "Negative, cockpit. A door is still open.",
        "boarding" => "Negative, cockpit. Loading is still in progress.",
        "beacon" => "Beacon is on. Clearing the area.",
        "roger" => "Roger.",
        _ => "Unable, cockpit. Not at this time.",
    }
}

fn capt_text(what: &str) -> &'static str {
    match what {
        "call" => "Ground, cockpit.",
        "push" => "Ready for pushback.",
        "start" => "Request engine start.",
        "stop" => "Stop the pushback.",
        "disconnect" => "Parking brake set, disconnect.",
        "chocks" => "Chocks, please.",
        "covers" => "Remove covers and pins, please.",
        "gpu" => "Ground power, please.",
        "asu" => "Air start unit, please.",
        _ => "Preconditioned air, please.",
    }
}

fn service_command(svc: &str) -> Option<&'static str> {
    match svc {
        "gpu" => Some("fbw/efb/ground/gpu"),
        "asu" => Some("fbw/gse/asu/toggle"),
        "pca" => Some("fbw/gse/pca/toggle"),
        _ => None,
    }
}

#[derive(Debug, PartialEq)]
pub enum Event {
    Command(String),
    StartAuto,
    StopAuto,
    Say(String),
}

enum Later {
    Answer(String),
    GoAhead,
    ReleaseTug,
    Bye,
}

#[derive(Default)]
pub struct GroundEngineer {
    pub present: bool,
    coming: Option<f64>,
    pub phase: String,
    pub log: Vec<(String, String)>,
    flown: bool,
    airborne_ms: f64,
    arrival: Option<f64>,
    mech: f64,
    beacon_said: bool,
    moved_at: Option<f64>,
    still_since: Option<f64>,
    pending: Vec<(String, bool, f64)>,
    presses: Option<HashMap<String, f64>>,
    later: Vec<(f64, Later)>,
    v: HashMap<String, f64>,
    gse: HashMap<String, f64>,
    now: f64,
    out: Vec<Event>,
}

impl GroundEngineer {
    pub fn coming(&self) -> bool {
        self.coming.is_some()
    }

    fn on(&self, name: &str) -> bool {
        self.v.get(name).copied().unwrap_or(0.0) > 0.5
    }

    fn gse(&self, name: &str) -> f64 {
        self.gse.get(&format!("{name}/state")).copied().unwrap_or(0.0)
    }

    fn busy(&self, name: &str) -> bool {
        matches!(self.gse(name) as i64, 1 | 2)
    }

    fn at(&self, svc: &str) -> bool {
        if svc == "gpu" {
            self.on("A32NX_EFB_GROUND_GPU_CONNECTED") || self.gse("gpu") == 2.0
        } else {
            self.gse(svc) == 2.0
        }
    }

    fn line(&mut self, who: &str, t: &str) {
        self.log.push((who.to_string(), t.to_string()));
        if self.log.len() > 8 {
            self.log.remove(0);
        }
    }

    fn say(&mut self, key: &str) {
        self.out.push(Event::Say(format!("gnd_{key}")));
        let t = text(key);
        self.line("GND", t);
    }

    fn command(&mut self, c: &str) {
        self.out.push(Event::Command(c.to_string()));
    }

    fn parked(&self) -> bool {
        self.on("SIM ON GROUND") && self.v.get("GROUND VELOCITY").copied().unwrap_or(0.0).abs() < 1.0
    }

    fn chocks_in(&self) -> bool {
        CHOCKS.iter().any(|c| self.on(&format!("A32NX_WALKAROUND_CHOCKS_{}", c.to_uppercase())))
    }

    fn set_chocks(&mut self, want: bool) {
        for c in CHOCKS {
            if self.on(&format!("A32NX_WALKAROUND_CHOCKS_{}", c.to_uppercase())) != want {
                self.command(&format!("fbw/walkaround/chocks_{c}_toggle"));
            }
        }
    }

    fn blocker(&self) -> Option<&'static str> {
        if self.on("A32NX_REFUEL_STARTED_BY_USR") || self.busy("fuel") {
            return Some("refuelling");
        }
        if (0..18).any(|i| self.v.get(&format!("INTERACTIVE POINT OPEN:{i}")).copied().unwrap_or(0.0) > 0.5) {
            return Some("doors_open");
        }
        if self.on("A32NX_BOARDING_STARTED_BY_USR") || self.busy("baggage") || self.busy("catering") {
            return Some("boarding");
        }
        None
    }

    fn push_step(&mut self) {
        if self.phase == "await_tug" {
            if !self.on("PUSHBACK ATTACHED") {
                return;
            }
            self.phase = "await_brake".into();
            if self.on("A32NX_PARK_BRAKE_LEVER_POS") {
                self.say("release_brakes");
                return;
            }
        }
        if self.phase == "await_brake" && !self.on("A32NX_PARK_BRAKE_LEVER_POS") {
            self.say("push_start");
            self.phase = "pushing".into();
            self.moved_at = None;
            self.still_since = None;
            self.out.push(Event::StartAuto);
        }
    }

    /// A press of an interphone button (EFB or X-Plane command).
    pub fn request(&mut self, what: &str) {
        let parked = self.parked();
        self.line("CAPT", capt_text(what));
        if what == "call" {
            if self.present {
                self.later.push((self.now + ANSWER_MS, Later::GoAhead));
                return;
            }
            if !parked && !self.on("PUSHBACK ATTACHED") {
                self.line("", "No answer.");
                return;
            }
            if self.coming.is_none() {
                self.coming = Some(self.now + ARRIVE_MS);
            }
            self.line("", "Calling the ground engineer\u{2026}");
            return;
        }
        if !self.present {
            self.line("", "Nobody on the headset. Call ground first.");
            return;
        }
        self.later.push((self.now + ANSWER_MS, Later::Answer(what.to_string())));
    }

    fn answer(&mut self, what: &str) {
        let pushing = matches!(self.phase.as_str(), "pushing" | "await_brake" | "await_tug");
        match what {
            "push" => {
                if pushing {
                    self.say("roger");
                    return;
                }
                if let Some(b) = self.blocker() {
                    self.say(b);
                    return;
                }
                if self.chocks_in() {
                    self.set_chocks(false);
                    self.say("chocks_out");
                }
                if !self.on("PUSHBACK ATTACHED") {
                    let idle = self.gse("tug") == 0.0;
                    if idle {
                        self.command("fbw/pushback/toggle");
                    }
                    self.say(if idle { "tug_coming" } else { "tug_wait" });
                    self.phase = "await_tug".into();
                    return;
                }
                self.phase = "await_brake".into();
                if self.on("A32NX_PARK_BRAKE_LEVER_POS") {
                    self.say("release_brakes");
                } else {
                    self.push_step();
                }
            }
            "start" => {
                let k = if self.on("A32NX_REFUEL_STARTED_BY_USR") || self.busy("fuel") { "refuelling" } else { "engines_clear" };
                self.say(k);
            }
            "stop" => {
                self.out.push(Event::StopAuto);
                self.say("stop");
                if pushing {
                    self.phase = "push_stopped".into();
                }
            }
            "disconnect" => {
                if !self.on("PUSHBACK ATTACHED") {
                    self.say("not_now");
                } else if !self.on("A32NX_PARK_BRAKE_LEVER_POS") {
                    self.say("push_complete");
                    self.phase = "push_done".into();
                } else {
                    self.disconnect();
                }
            }
            "chocks" => {
                let want = !self.chocks_in();
                if want && !self.parked() {
                    self.say("not_now");
                    return;
                }
                self.set_chocks(want);
                self.say(if want { "chocks_in" } else { "chocks_out" });
            }
            "covers" => {
                for c in COVERS {
                    if self.on(&format!("A32NX_WALKAROUND_{}", c.to_uppercase())) {
                        self.command(&format!("fbw/walkaround/{c}_toggle"));
                    }
                }
                self.say("covers_off");
            }
            svc => {
                let Some(cmd) = service_command(svc) else { return };
                if !self.parked() || self.on("LIGHT BEACON") {
                    self.say("not_now");
                    return;
                }
                let want = !self.at(svc);
                self.command(cmd);
                self.pending.retain(|p| p.0 != svc);
                self.pending.push((svc.to_string(), want, self.now + 120_000.0));
                self.say("roger");
            }
        }
    }

    fn disconnect(&mut self) {
        self.say("disconnect");
        self.phase = "releasing".into();
        self.later.push((self.now + DISCONNECT_MS, Later::ReleaseTug));
    }

    /// Once a second: the variables, the GSE states (`fbw/gse/*` without the
    /// prefix) and the interphone press counts. Returns what to do.
    pub fn tick(&mut self, now: f64, v: HashMap<String, f64>, gse: HashMap<String, f64>, presses: HashMap<String, f64>, auto_running: bool) -> Vec<Event> {
        self.now = now;
        self.v = v;
        self.gse = gse;
        let first = self.presses.is_none();
        let was = self.presses.replace(presses.clone()).unwrap_or_default();
        if !first {
            for (what, count) in &presses {
                if was.get(what).is_some_and(|w| count > w) {
                    self.request(what);
                }
            }
        }
        let due: Vec<Later> = {
            let (ready, wait): (Vec<_>, Vec<_>) = std::mem::take(&mut self.later).into_iter().partition(|(t, _)| *t <= now);
            self.later = wait;
            ready.into_iter().map(|(_, l)| l).collect()
        };
        for l in due {
            match l {
                Later::Answer(w) => self.answer(&w),
                Later::GoAhead => self.say("go_ahead"),
                Later::ReleaseTug => {
                    if self.on("PUSHBACK ATTACHED") {
                        self.command("fbw/pushback/toggle");
                    }
                }
                Later::Bye => {
                    self.say("bye");
                    self.present = false;
                    self.phase = "idle".into();
                }
            }
        }
        if !self.v.contains_key("SIM ON GROUND") {
            return std::mem::take(&mut self.out);
        }
        let on_ground = self.on("SIM ON GROUND");
        let gs = self.v.get("GROUND VELOCITY").copied().unwrap_or(0.0).abs();
        self.airborne_ms = if on_ground { 0.0 } else { self.airborne_ms + 1000.0 };
        if self.airborne_ms >= 15000.0 {
            self.flown = true;
        }
        if !on_ground {
            self.present = false;
            self.coming = None;
            self.phase = "idle".into();
            self.pending.clear();
        }
        let mech = self.v.get("PUSH_OVHD_CALLS_MECH").copied().unwrap_or(0.0);
        if mech > 0.5 && self.mech <= 0.5 && on_ground {
            self.request("call");
        }
        self.mech = mech;
        if self.coming.is_some_and(|t| now >= t) {
            self.coming = None;
            if self.parked() || self.on("PUSHBACK ATTACHED") {
                self.present = true;
                self.say("on_headset");
            } else {
                self.line("", "No answer.");
            }
        }
        if !self.present && on_ground && self.on("PUSHBACK ATTACHED") {
            self.present = true;
        }
        let engines_off = (1..=4).all(|n| self.v.get(&format!("A32NX_ENGINE_STATE:{n}")).copied().unwrap_or(0.0) == 0.0);
        if self.flown && on_ground && gs < 1.0 && engines_off && self.on("A32NX_PARK_BRAKE_LEVER_POS") {
            match self.arrival {
                None => self.arrival = Some(now + ARRIVAL_MS),
                Some(t) if now >= t => {
                    self.flown = false;
                    self.arrival = None;
                    self.present = true;
                    self.set_chocks(true);
                    self.say("arrival");
                }
                _ => {}
            }
        } else {
            self.arrival = None;
        }
        if self.present && gs > 3.0 && !self.on("PUSHBACK ATTACHED") && self.phase != "releasing" {
            self.present = false;
            self.phase = "idle".into();
        }
        self.push_step();
        if self.phase == "pushing" {
            if gs > 0.5 {
                self.moved_at = Some(now);
                self.still_since = None;
            } else if self.moved_at.is_some() && self.still_since.is_none() {
                self.still_since = Some(now);
            }
            if !auto_running && self.moved_at.is_some() && self.still_since.is_some_and(|t| now - t > 3000.0) {
                self.phase = "push_done".into();
                self.say("push_complete");
            }
            if !self.on("PUSHBACK ATTACHED") {
                self.phase = "idle".into();
            }
        }
        if matches!(self.phase.as_str(), "push_done" | "push_stopped") && self.on("A32NX_PARK_BRAKE_LEVER_POS") && self.on("PUSHBACK ATTACHED") {
            self.disconnect();
        }
        if self.phase == "releasing" && !self.on("PUSHBACK ATTACHED") {
            self.phase = "bye".into();
            self.later.push((now + BYE_MS, Later::Bye));
        }
        let pending = std::mem::take(&mut self.pending);
        for (svc, want, until) in pending {
            if self.at(&svc) == want {
                if self.present {
                    self.say(&format!("{svc}_{}", if want { "on" } else { "off" }));
                }
            } else if now < until {
                self.pending.push((svc, want, until));
            }
        }
        let services = ["gpu", "asu", "pca", "fuel", "baggage", "catering", "water", "lav", "clean"].iter().any(|s| self.busy(s));
        if self.on("LIGHT BEACON") && self.present && services && !self.beacon_said {
            self.beacon_said = true;
            self.say("beacon");
        }
        if !self.on("LIGHT BEACON") {
            self.beacon_said = false;
        }
        std::mem::take(&mut self.out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn a_call_on_the_stand_brings_the_engineer_and_push_calls_the_tug() {
        let mut g = GroundEngineer::default();
        let parked = vars(&[("SIM ON GROUND", 1.0), ("A32NX_PARK_BRAKE_LEVER_POS", 1.0)]);
        g.tick(0.0, parked.clone(), HashMap::new(), HashMap::new(), false);
        g.request("call");
        g.tick(13000.0, parked.clone(), HashMap::new(), HashMap::new(), false);
        assert!(g.present);
        g.request("push");
        let out = g.tick(15000.0, parked.clone(), HashMap::new(), HashMap::new(), false);
        assert!(out.contains(&Event::Command("fbw/pushback/toggle".into())));
        assert_eq!(g.phase, "await_tug");
        let mut attached = parked.clone();
        attached.insert("PUSHBACK ATTACHED".into(), 1.0);
        g.tick(16000.0, attached.clone(), HashMap::new(), HashMap::new(), false);
        assert_eq!(g.phase, "await_brake");
        attached.insert("A32NX_PARK_BRAKE_LEVER_POS".into(), 0.0);
        let out = g.tick(17000.0, attached, HashMap::new(), HashMap::new(), false);
        assert!(out.contains(&Event::StartAuto));
        assert_eq!(g.phase, "pushing");
    }
}

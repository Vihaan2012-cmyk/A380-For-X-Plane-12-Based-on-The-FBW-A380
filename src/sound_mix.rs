//! A tester's start-up and cabin balance on the pack's own events: start events lifted while an engine starts, outside sounds lowered in cabin views, the APU quiet on the flight deck.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Start,
    Comb,
    Engine,
    Apu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Role {
    pub kind: Kind,
    pub engine: usize,
    pub outside: bool,
}

/// Starter whine while the engine starts, about +6.6 dB.
const START_GAIN: f32 = 2.15;
/// Combustion while starting and just after light-off, about +4.9 dB.
const COMB_GAIN: f32 = 1.75;
const ENGINE_START_GAIN: f32 = 1.2;
const LIGHTOFF_HOLD_S: f64 = 10.;
/// The outside engine events heard from a cabin view.
const OUTSIDE_IN_CABIN: f32 = 0.28;
/// X-Plane's exterior aircraft, environment and unprocessed buses (XPLMAudioBus 4-6) in cabin views.
pub const INSIDE_BUS_GAIN: [(i32, f32); 3] = [(4, 0.42), (5, 0.20), (6, 0.18)];
pub const SMOOTH_S: f64 = 0.35;

/// The event's part in the mix, from its GUIDs.txt path; None for events left alone.
pub fn role(path: &str) -> Option<Role> {
    let p = path.strip_prefix("event:/")?;
    if let Some(rest) = p.strip_prefix("Dynamics/Apu") {
        return match rest {
            "" => Some(Role { kind: Kind::Apu, engine: 0, outside: false }),
            "X" => Some(Role { kind: Kind::Apu, engine: 0, outside: true }),
            _ => None,
        };
    }
    let rest = p.strip_prefix("Engines/RR/").or_else(|| p.strip_prefix("Engines/GP/"))?;
    let (kind, rest) = [("Start", Kind::Start), ("Comb", Kind::Comb), ("Engine", Kind::Engine)].iter().find_map(|(n, k)| rest.strip_prefix(n).map(|r| (*k, r)))?;
    let mut c = rest.chars();
    let engine = "ABCD".find(c.next()?)?;
    let outside = match c.as_str() {
        "" => false,
        "X" => true,
        _ => return None,
    };
    Some(Role { kind, engine, outside })
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Inputs {
    pub now_s: f64,
    pub outside_view: bool,
    /// X-Plane's pilots_head_z, m, aft positive: the flight deck sits near -32.
    pub head_z_m: f64,
    pub starting: [bool; 4],
    pub burning: [bool; 4],
}

#[derive(Default)]
pub struct Mix {
    lightoff_s: [Option<f64>; 4],
    was_burning: [bool; 4],
}

impl Mix {
    /// Once a frame before `target`: notes each engine lighting off during its start.
    pub fn observe(&mut self, i: &Inputs) {
        for k in 0..4 {
            if i.burning[k] && !self.was_burning[k] && i.starting[k] {
                self.lightoff_s[k] = Some(i.now_s);
            }
            self.was_burning[k] = i.burning[k];
        }
    }

    /// The event's volume multiple this frame.
    pub fn target(&self, r: Role, i: &Inputs) -> f32 {
        let starting = i.starting[r.engine];
        let mut gain = match r.kind {
            Kind::Start if starting => START_GAIN,
            Kind::Comb if starting || self.lightoff_s[r.engine].is_some_and(|t| i.now_s - t < LIGHTOFF_HOLD_S) => COMB_GAIN,
            Kind::Engine if starting => ENGINE_START_GAIN,
            Kind::Apu if !r.outside && !i.outside_view => ((i.head_z_m + 24.) / 4.).clamp(0., 1.) as f32,
            _ => 1.,
        };
        if r.outside && r.kind != Kind::Apu && !i.outside_view {
            gain *= OUTSIDE_IN_CABIN;
        }
        gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pack_paths_name_their_engine_and_side() {
        assert_eq!(role("event:/Engines/RR/StartC"), Some(Role { kind: Kind::Start, engine: 2, outside: false }));
        assert_eq!(role("event:/Engines/GP/EngineDX"), Some(Role { kind: Kind::Engine, engine: 3, outside: true }));
        assert_eq!(role("event:/Engines/RR/CombAX"), Some(Role { kind: Kind::Comb, engine: 0, outside: true }));
        assert_eq!(role("event:/Dynamics/ApuX"), Some(Role { kind: Kind::Apu, engine: 0, outside: true }));
        assert_eq!(role("event:/Engines/RR/RevB"), None);
        assert_eq!(role("event:/Dynamics/ApuStart"), None);
        assert_eq!(role("event:/Engines/RR/StartE"), None);
    }

    #[test]
    fn a_starting_engine_is_lifted_and_its_combustion_holds_ten_seconds_past_light_off() {
        let mut m = Mix::default();
        let comb = Role { kind: Kind::Comb, engine: 1, outside: false };
        let start = Role { kind: Kind::Start, engine: 1, outside: false };
        let mut i = Inputs { now_s: 0., outside_view: true, ..Default::default() };
        i.starting[1] = true;
        m.observe(&i);
        assert_eq!(m.target(start, &i), START_GAIN);
        i.now_s = 30.;
        i.burning[1] = true;
        m.observe(&i);
        i.starting[1] = false;
        i.now_s = 39.;
        assert_eq!(m.target(comb, &i), COMB_GAIN);
        assert_eq!(m.target(start, &i), 1.);
        i.now_s = 41.;
        assert_eq!(m.target(comb, &i), 1.);
        assert_eq!(m.target(Role { engine: 0, ..start }, &i), 1., "other engines untouched");
    }

    #[test]
    fn cabin_views_lower_the_outside_engines_and_the_flight_deck_hears_no_apu() {
        let m = Mix::default();
        let inside = Inputs { head_z_m: -32., ..Default::default() };
        let engine_x = Role { kind: Kind::Engine, engine: 0, outside: true };
        assert_eq!(m.target(engine_x, &inside), OUTSIDE_IN_CABIN);
        assert_eq!(m.target(engine_x, &Inputs { outside_view: true, ..inside }), 1.);
        let apu = Role { kind: Kind::Apu, engine: 0, outside: false };
        assert_eq!(m.target(apu, &inside), 0.);
        assert_eq!(m.target(apu, &Inputs { head_z_m: 0., ..inside }), 1.);
        assert_eq!(m.target(Role { outside: true, ..apu }, &inside), 1.);
    }
}

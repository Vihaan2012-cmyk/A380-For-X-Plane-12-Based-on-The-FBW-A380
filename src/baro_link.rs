//! `xphfbw.baroLink`: the captain's and first officer's EFIS baro
//! references (and with `LRS` the standby altimeter) follow each other.
//!
//! The side the crew last touched is the source. Each tick the other side is
//! compared with it, as FlyByWire's own `updateEfisSync` does, and brought
//! level with an absolute set (value), a push/pull (STD/QNH) or a copy of the
//! unit variable. Nothing is sent once the sides agree, and after
//! [`MAX_ATTEMPTS`] unanswered ticks it stops until the crew touches a knob
//! again, so a side the FCU will not follow is not fought. The standby is
//! written only when the source's value or mode changes, never held.

use crate::user_prefs::BaroLink;

pub const MAX_ATTEMPTS: u8 = 3;
pub const INHG_TO_HPA: f64 = 33.8638687;

const INHG_TOLERANCE: f64 = 0.005;
const HPA_TOLERANCE: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapshot {
    pub inhg: f64,
    pub hpa: f64,
    pub is_inhg: bool,
    pub std: bool,
}

impl Snapshot {
    fn plausible(&self) -> bool {
        (745.0..=1100.0).contains(&self.hpa) && (22.0..=32.5).contains(&self.inhg)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Standby {
    pub inhg: f64,
    pub std: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mirror {
    pub set_hpa: [Option<f64>; 2],
    pub push: [bool; 2],
    pub pull: [bool; 2],
    pub unit_inhg: [Option<bool>; 2],
    pub standby: Option<Standby>,
}

impl Mirror {
    fn acts(&self) -> bool {
        self.set_hpa.iter().any(Option::is_some) || self.push.contains(&true) || self.pull.contains(&true) || self.unit_inhg.iter().any(Option::is_some)
    }
}

#[derive(Debug, Default)]
pub struct State {
    source: Option<usize>,
    last_unit: [Option<bool>; 2],
    pushed_standby: Option<(i64, bool)>,
    attempts: u8,
}

impl State {
    pub fn step(&mut self, link: BaroLink, touched: [bool; 2], sides: [Snapshot; 2]) -> Mirror {
        let mut out = Mirror::default();
        if link == BaroLink::Independent {
            *self = Self::default();
            return out;
        }
        let unit_changed = [0, 1].map(|i| self.last_unit[i].is_some_and(|was| was != sides[i].is_inhg));
        self.last_unit = [Some(sides[0].is_inhg), Some(sides[1].is_inhg)];
        let acted = [0, 1].map(|i| touched[i] || unit_changed[i]);
        if acted[0] || acted[1] {
            self.source = Some(match (acted[0], acted[1]) {
                (true, false) => 0,
                (false, true) => 1,
                _ => self.source.unwrap_or(0),
            });
            self.attempts = 0;
        }
        let Some(src) = self.source else { return out };
        let tgt = 1 - src;
        let (s, t) = (sides[src], sides[tgt]);
        if self.attempts < MAX_ATTEMPTS {
            if t.is_inhg != s.is_inhg {
                out.unit_inhg[tgt] = Some(s.is_inhg);
                self.last_unit[tgt] = Some(s.is_inhg);
            }
            if s.plausible() && t.plausible() {
                let differs = if s.is_inhg { (s.inhg - t.inhg).abs() >= INHG_TOLERANCE } else { (s.hpa - t.hpa).abs() >= HPA_TOLERANCE };
                if differs {
                    out.set_hpa[tgt] = Some(if s.is_inhg { s.inhg * INHG_TO_HPA } else { s.hpa });
                }
            }
            if s.std != t.std {
                if s.std {
                    out.push[tgt] = true;
                } else {
                    out.pull[tgt] = true;
                }
            }
            self.attempts = if out.acts() { self.attempts + 1 } else { 0 };
        }
        if link == BaroLink::LeftRightStandby && s.plausible() {
            let key = ((s.inhg * 100.0).round() as i64, s.std);
            if self.pushed_standby != Some(key) {
                self.pushed_standby = Some(key);
                out.standby = Some(Standby { inhg: s.inhg, std: s.std });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(inhg: f64, is_inhg: bool, std: bool) -> Snapshot {
        Snapshot { inhg, hpa: (inhg * INHG_TO_HPA).round(), is_inhg, std }
    }

    const QUIET: [bool; 2] = [false, false];

    #[test]
    fn independent_never_mirrors_and_forgets_the_source() {
        let mut s = State::default();
        let sides = [snap(29.92, true, false), snap(30.10, true, true)];
        assert_eq!(s.step(BaroLink::Independent, [true, false], sides), Mirror::default());
        assert_eq!(s.step(BaroLink::LeftRight, QUIET, sides), Mirror::default(), "no source until a knob is touched");
    }

    #[test]
    fn a_left_change_brings_the_right_level_once_in_value_and_mode() {
        let mut s = State::default();
        let sides = [snap(30.05, true, false), snap(29.92, true, false)];
        let m = s.step(BaroLink::LeftRight, [true, false], sides);
        assert_eq!(m.set_hpa[1], Some(30.05 * INHG_TO_HPA));
        assert_eq!(m.set_hpa[0], None);
        assert!(!m.push[1] && !m.pull[1]);
        assert_eq!(m.standby, None, "LR leaves the standby alone");
        let level = [snap(30.05, true, false), snap(30.05, true, false)];
        assert_eq!(s.step(BaroLink::LeftRight, QUIET, level), Mirror::default(), "in step: nothing more is sent");
    }

    #[test]
    fn the_right_side_can_be_the_source_and_std_and_qnh_follow() {
        let mut s = State::default();
        let sides = [snap(29.92, true, false), snap(29.92, true, true)];
        let m = s.step(BaroLink::LeftRight, [false, true], sides);
        assert_eq!(m.push, [true, false]);
        assert_eq!(m.set_hpa, [None, None]);
        let m = s.step(BaroLink::LeftRight, [false, true], [snap(29.92, true, true), snap(29.92, true, false)]);
        assert_eq!(m.pull, [true, false]);
    }

    #[test]
    fn hpa_sides_compare_in_whole_hectopascals() {
        let mut s = State::default();
        let left = Snapshot { inhg: 29.92, hpa: 1013.0, is_inhg: false, std: false };
        let right = Snapshot { inhg: 29.95, hpa: 1014.0, is_inhg: false, std: false };
        let m = s.step(BaroLink::LeftRight, [true, false], [left, right]);
        assert_eq!(m.set_hpa[1], Some(1013.0));
        let near = Snapshot { hpa: 1013.3, ..right };
        assert_eq!(State::default().step(BaroLink::LeftRight, [true, false], [left, near]).set_hpa, [None, None]);
    }

    #[test]
    fn a_unit_change_on_one_side_is_copied_and_not_echoed_back() {
        let mut s = State::default();
        let both_inhg = [snap(29.92, true, false), snap(29.92, true, false)];
        s.step(BaroLink::LeftRight, QUIET, both_inhg);
        let m = s.step(BaroLink::LeftRight, QUIET, [snap(29.92, false, false), snap(29.92, true, false)]);
        assert_eq!(m.unit_inhg, [None, Some(false)]);
        let after = s.step(BaroLink::LeftRight, QUIET, [snap(29.92, false, false), snap(29.92, false, false)]);
        assert_eq!(after, Mirror::default(), "the copied unit does not read as a second change");
    }

    #[test]
    fn a_side_that_never_follows_is_given_up_on_until_the_next_touch() {
        let mut s = State::default();
        let stuck = [snap(30.00, true, false), snap(29.00, true, false)];
        assert!(s.step(BaroLink::LeftRight, [true, false], stuck).acts());
        assert!(s.step(BaroLink::LeftRight, QUIET, stuck).acts());
        assert!(s.step(BaroLink::LeftRight, QUIET, stuck).acts());
        for _ in 0..5 {
            assert!(!s.step(BaroLink::LeftRight, QUIET, stuck).acts());
        }
        assert!(s.step(BaroLink::LeftRight, [true, false], stuck).acts(), "a new touch tries again");
    }

    #[test]
    fn the_standby_is_written_when_the_source_changes_and_not_held() {
        let mut s = State::default();
        let sides = [snap(30.05, true, false), snap(30.05, true, false)];
        let m = s.step(BaroLink::LeftRightStandby, [true, false], sides);
        assert_eq!(m.standby, Some(Standby { inhg: 30.05, std: false }));
        assert_eq!(s.step(BaroLink::LeftRightStandby, QUIET, sides).standby, None, "the crew's own standby setting is not overwritten each tick");
        let std = [snap(30.05, true, true), snap(30.05, true, true)];
        assert_eq!(s.step(BaroLink::LeftRightStandby, [true, false], std).standby, Some(Standby { inhg: 30.05, std: true }));
    }

    #[test]
    fn implausible_readings_are_ignored() {
        let mut s = State::default();
        let blank = Snapshot { inhg: 0.0, hpa: 0.0, is_inhg: true, std: false };
        let m = s.step(BaroLink::LeftRightStandby, [true, false], [blank, snap(29.92, true, false)]);
        assert_eq!(m, Mirror::default());
    }
}

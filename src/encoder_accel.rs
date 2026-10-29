//! `xphfbw.encoderAccel`: a knob turned in quick succession steps by more
//! than one detent. Every FCU knob turn (SPD/MACH, HDG/TRK, ALT, V/S FPA and
//! the two baro knobs) reaches the FCU through `afs_events::send`, whether it
//! came from a `fbw/event/*` command, an X-Plane barometer command or a
//! converted key event; the accelerated count replaces the one-detent turn
//! the tick would otherwise carry.

pub const WINDOW_MS: u64 = 150;
pub const FAST_DETENTS: usize = 3;
pub const FASTER_DETENTS: usize = 5;
pub const FAST_FACTOR: i32 = 5;
pub const FASTER_FACTOR: i32 = 10;

const HISTORY: usize = FASTER_DETENTS;
const KNOBS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Knob {
    Spd,
    Hdg,
    Alt,
    Vs,
    BaroLeft,
    BaroRight,
}

impl Knob {
    fn index(self) -> usize {
        self as usize
    }

    pub fn max_turns(self) -> i32 {
        match self {
            Knob::BaroLeft | Knob::BaroRight => 10,
            _ => 7,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Track {
    direction: i8,
    stamps: [u64; HISTORY],
    len: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Accelerator {
    tracks: [Track; KNOBS],
    sums: [i32; KNOBS],
    touched: [bool; KNOBS],
}

impl Accelerator {
    pub fn factor_for(detents_in_window: usize) -> i32 {
        if detents_in_window >= FASTER_DETENTS {
            FASTER_FACTOR
        } else if detents_in_window >= FAST_DETENTS {
            FAST_FACTOR
        } else {
            1
        }
    }

    /// One detent of `knob` in direction `direction` (+1/-1) at `now_ms`;
    /// returns the signed step it counts for, and adds it to the pending
    /// total for the tick.
    pub fn detent(&mut self, knob: Knob, direction: i8, now_ms: u64) -> i32 {
        let direction = direction.signum();
        if direction == 0 {
            return 0;
        }
        let i = knob.index();
        let track = &mut self.tracks[i];
        if track.direction != direction {
            *track = Track { direction, ..Track::default() };
        }
        let mut kept = [0u64; HISTORY];
        let mut n = 0;
        for &t in &track.stamps[..track.len] {
            if now_ms >= t && now_ms - t <= WINDOW_MS && n < HISTORY - 1 {
                kept[n] = t;
                n += 1;
            }
        }
        kept[n] = now_ms;
        track.stamps = kept;
        track.len = n + 1;
        let step = i32::from(direction) * Self::factor_for(track.len);
        self.sums[i] += step;
        self.touched[i] = true;
        step
    }

    /// The turns this tick should carry per knob (in `Knob` order), clamped
    /// to what the FCU accepts, for knobs that turned; clears the totals.
    pub fn take(&mut self) -> [Option<i8>; KNOBS] {
        let mut out = [None; KNOBS];
        for (i, slot) in out.iter_mut().enumerate() {
            if self.touched[i] {
                let max = if i >= 4 { Knob::BaroLeft.max_turns() } else { Knob::Spd.max_turns() };
                *slot = Some(self.sums[i].clamp(-max, max) as i8);
            }
        }
        self.sums = [0; KNOBS];
        self.touched = [false; KNOBS];
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_turns_stay_single_detents() {
        let mut a = Accelerator::default();
        for k in 0..6 {
            assert_eq!(a.detent(Knob::Hdg, 1, k * 400), 1);
        }
    }

    #[test]
    fn three_detents_within_the_window_step_by_five_and_five_step_by_ten() {
        let mut a = Accelerator::default();
        assert_eq!(a.detent(Knob::Spd, 1, 1000), 1);
        assert_eq!(a.detent(Knob::Spd, 1, 1040), 1);
        assert_eq!(a.detent(Knob::Spd, 1, 1080), 5);
        assert_eq!(a.detent(Knob::Spd, 1, 1110), 5);
        assert_eq!(a.detent(Knob::Spd, 1, 1140), 10);
        assert_eq!(a.detent(Knob::Spd, 1, 1170), 10);
    }

    #[test]
    fn the_window_is_150_ms_inclusive_and_old_detents_drop_out() {
        let mut a = Accelerator::default();
        a.detent(Knob::Alt, 1, 0);
        a.detent(Knob::Alt, 1, 75);
        assert_eq!(a.detent(Knob::Alt, 1, 150), 5);
        let mut b = Accelerator::default();
        b.detent(Knob::Alt, 1, 0);
        b.detent(Knob::Alt, 1, 75);
        assert_eq!(b.detent(Knob::Alt, 1, 151), 1);
    }

    #[test]
    fn a_reversal_or_another_knob_starts_over() {
        let mut a = Accelerator::default();
        for t in [0, 30, 60] {
            a.detent(Knob::Vs, 1, t);
        }
        assert_eq!(a.detent(Knob::Vs, -1, 90), -1);
        assert_eq!(a.detent(Knob::Hdg, 1, 95), 1);
        assert_eq!(a.detent(Knob::BaroLeft, 1, 100), 1);
        assert_eq!(a.detent(Knob::BaroRight, 1, 100), 1);
    }

    #[test]
    fn the_same_inputs_always_give_the_same_steps() {
        let run = || {
            let mut a = Accelerator::default();
            (0..12).map(|k| a.detent(Knob::Hdg, 1, k * 30)).collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
        assert_eq!(run(), vec![1, 1, 5, 5, 10, 10, 10, 10, 10, 10, 10, 10]);
    }

    #[test]
    fn take_clamps_to_the_fcu_limit_reports_only_turned_knobs_and_clears() {
        let mut a = Accelerator::default();
        for t in [0, 20, 40, 60, 80, 100] {
            a.detent(Knob::Hdg, 1, t);
        }
        a.detent(Knob::BaroLeft, -1, 0);
        let turns = a.take();
        assert_eq!(turns, [None, Some(7), None, None, Some(-1), None]);
        assert_eq!(a.take(), [None; 6]);
    }

    #[test]
    fn baro_may_exceed_the_afs_limit_up_to_ten() {
        let mut a = Accelerator::default();
        for t in [0, 10, 20, 30, 40, 50] {
            a.detent(Knob::BaroRight, 1, t);
        }
        assert_eq!(a.take()[5], Some(10));
    }
}

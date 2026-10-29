#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    PauseSim,
    PlaceAircraft,
    WaitForScenery,
    SetSituation,
    RebuildSystems,
    WriteVariables,
    RestoreModules,
    RestoreDeepAreas,
    RestoreFbwInternals,
    RestoreFailures,
    RestoreFms,
    RestoreUi,
    Settle,
    ReapplySituation,
    Resume,
    Done,
}

pub const ORDER: [Stage; 16] = [
    Stage::PauseSim,
    Stage::PlaceAircraft,
    Stage::WaitForScenery,
    Stage::SetSituation,
    Stage::RebuildSystems,
    Stage::WriteVariables,
    Stage::RestoreModules,
    Stage::RestoreDeepAreas,
    Stage::RestoreFbwInternals,
    Stage::RestoreFailures,
    Stage::RestoreFms,
    Stage::RestoreUi,
    Stage::Settle,
    Stage::ReapplySituation,
    Stage::Resume,
    Stage::Done,
];

/// GENERIC: settle at least this many system frames and this much system
/// time with the sim paused, and give up waiting after `max_frames`.
#[derive(Clone, Copy, Debug)]
pub struct SettleLimits {
    pub min_frames: u32,
    pub min_system_s: f64,
    pub max_frames: u32,
    pub quiet_frames: u32,
    pub scenery_timeout_s: f64,
}

impl Default for SettleLimits {
    fn default() -> Self {
        Self { min_frames: 60, min_system_s: 2.0, max_frames: 900, quiet_frames: 30, scenery_timeout_s: 120.0 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Do(Stage),
    Wait(Stage),
    Finished { settle_frames: u32, converged: bool },
}

/// The frame-by-frame load driver. The plugin performs each `Do` stage on
/// the frame it is returned, then calls `advance`; `Wait` stages are polled
/// with the facts they wait on.
#[derive(Clone, Debug)]
pub struct Sequence {
    index: usize,
    limits: SettleLimits,
    waited_s: f64,
    settle_frames: u32,
    settle_system_s: f64,
    quiet: u32,
    converged: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Poll {
    pub scenery_loading: bool,
    pub frame_s: f64,
    /// True when no restored variable moved by more than its tolerance this
    /// settle frame.
    pub quiet: bool,
}

impl Sequence {
    pub fn new(limits: SettleLimits) -> Self {
        Self { index: 0, limits, waited_s: 0.0, settle_frames: 0, settle_system_s: 0.0, quiet: 0, converged: false }
    }

    pub fn stage(&self) -> Stage {
        ORDER[self.index.min(ORDER.len() - 1)]
    }

    pub fn step(&mut self, poll: Poll) -> Step {
        match self.stage() {
            Stage::WaitForScenery => {
                self.waited_s += poll.frame_s.max(0.0);
                if poll.scenery_loading && self.waited_s < self.limits.scenery_timeout_s {
                    return Step::Wait(Stage::WaitForScenery);
                }
                self.index += 1;
                self.step(Poll { frame_s: 0.0, ..poll })
            }
            Stage::Settle => {
                if poll.frame_s > 0.0 {
                    self.settle_frames += 1;
                    self.settle_system_s += poll.frame_s;
                    self.quiet = if poll.quiet { self.quiet + 1 } else { 0 };
                }
                let enough = self.settle_frames >= self.limits.min_frames && self.settle_system_s >= self.limits.min_system_s;
                self.converged = self.quiet >= self.limits.quiet_frames;
                if (enough && self.converged) || self.settle_frames >= self.limits.max_frames {
                    self.index += 1;
                    return Step::Do(self.stage());
                }
                Step::Wait(Stage::Settle)
            }
            Stage::Done => Step::Finished { settle_frames: self.settle_frames, converged: self.converged },
            s => Step::Do(s),
        }
    }

    pub fn advance(&mut self) {
        if self.stage() != Stage::Done {
            self.index += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sim_is_paused_before_anything_moves_and_resumed_last() {
        assert_eq!(ORDER[0], Stage::PauseSim);
        let pos = |s| ORDER.iter().position(|x| *x == s).unwrap();
        assert!(pos(Stage::SetSituation) < pos(Stage::RebuildSystems));
        assert!(pos(Stage::RebuildSystems) < pos(Stage::WriteVariables));
        assert!(pos(Stage::RestoreFbwInternals) < pos(Stage::RestoreFailures));
        assert!(pos(Stage::Settle) < pos(Stage::Resume));
        assert_eq!(ORDER[ORDER.len() - 2], Stage::Resume);
    }

    #[test]
    fn a_full_load_walks_every_stage_waiting_on_scenery_and_settling() {
        let limits = SettleLimits { min_frames: 3, min_system_s: 0.05, max_frames: 100, quiet_frames: 2, scenery_timeout_s: 10.0 };
        let mut seq = Sequence::new(limits);
        let mut done = Vec::new();
        let mut waits = 0;
        let mut loading_frames = 3;
        for _ in 0..200 {
            let poll = Poll { scenery_loading: loading_frames > 0, frame_s: 1.0 / 30.0, quiet: true };
            match seq.step(poll) {
                Step::Do(s) => {
                    done.push(s);
                    seq.advance();
                }
                Step::Wait(_) => {
                    waits += 1;
                    loading_frames -= 1;
                }
                Step::Finished { settle_frames, converged } => {
                    assert!(converged);
                    assert!(settle_frames >= 3);
                    break;
                }
            }
        }
        let expected: Vec<Stage> = ORDER.iter().copied().filter(|s| !matches!(s, Stage::WaitForScenery | Stage::Settle | Stage::Done)).collect();
        assert_eq!(done, expected);
        assert!(waits >= 3);
    }

    #[test]
    fn settling_gives_up_at_the_frame_limit_and_says_so() {
        let limits = SettleLimits { min_frames: 1, min_system_s: 0.0, max_frames: 5, quiet_frames: 10, scenery_timeout_s: 0.0 };
        let mut seq = Sequence::new(limits);
        while seq.stage() != Stage::Settle {
            seq.advance();
        }
        let mut frames = 0;
        while let Step::Wait(_) = seq.step(Poll { frame_s: 0.02, quiet: false, ..Default::default() }) {
            frames += 1;
        }
        assert_eq!(frames, 4);
        seq.advance();
        seq.advance();
        assert_eq!(seq.step(Poll::default()), Step::Finished { settle_frames: 5, converged: false });
    }
}

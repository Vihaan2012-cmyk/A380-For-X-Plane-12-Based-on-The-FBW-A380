use crate::{Action, EcamLive};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Action,
    Condition,
    Plain,
}

/// One procedure line; the plugin fills it field for field from `fws_data::Item`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EcamItem {
    pub name: &'static str,
    pub kind: ItemKind,
    pub sensed: bool,
    pub level: u8,
    pub label: &'static str,
    pub time: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EcamProcedure {
    pub id: u32,
    pub title: &'static str,
    pub items: Vec<EcamItem>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch {
    EngMaster { engine: u8, on: bool },
    EngFirePb { engine: u8 },
    EngAgent { engine: u8, agent: u8 },
    ApuBleed { on: bool },
    ApuMaster { on: bool },
    ApuFirePb,
    ApuAgent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Doer {
    Pm(Switch),
    Pf,
    Atc,
    Tick,
    Unmapped,
}

fn engine_of(rest: &str) -> Option<(u8, &str)> {
    let (n, tail) = rest.split_once(' ')?;
    let n: u8 = n.parse().ok()?;
    (1..=4).contains(&n).then_some((n, tail))
}

pub fn doer(item: &EcamItem) -> Doer {
    if item.kind != ItemKind::Action {
        return Doer::Tick;
    }
    let name = item.name.trim();
    let label = item.label.trim();
    if name.starts_with("THR LEVER") || name.starts_with("SIDESTICK") || name.starts_with("RUDDER") {
        return Doer::Pf;
    }
    if name == "ATC" {
        return Doer::Atc;
    }
    let sw = if let Some((engine, tail)) = name.strip_prefix("ENG ").and_then(engine_of) {
        match (tail, label) {
            ("MASTER", "OFF") => Some(Switch::EngMaster { engine, on: false }),
            ("MASTER", "ON") => Some(Switch::EngMaster { engine, on: true }),
            ("FIRE P/B", "PUSH") => Some(Switch::EngFirePb { engine }),
            ("AGENT 1", "DISCH") => Some(Switch::EngAgent { engine, agent: 1 }),
            ("AGENT 2", "DISCH") => Some(Switch::EngAgent { engine, agent: 2 }),
            _ => None,
        }
    } else {
        match (name, label) {
            ("APU BLEED", "OFF") => Some(Switch::ApuBleed { on: false }),
            ("APU BLEED", "ON") => Some(Switch::ApuBleed { on: true }),
            ("APU MASTER SW", "OFF") => Some(Switch::ApuMaster { on: false }),
            ("APU FIRE P/B", "PUSH") => Some(Switch::ApuFirePb),
            ("APU AGENT", "DISCH") => Some(Switch::ApuAgent),
            _ => None,
        }
    };
    match sw {
        Some(sw) => Doer::Pm(sw),
        None if !item.sensed => Doer::Tick,
        None => Doer::Unmapped,
    }
}

/// Irreversible items the PM only moves after the PF confirms.
pub fn needs_confirmation(sw: Switch) -> bool {
    matches!(sw, Switch::EngMaster { on: false, .. } | Switch::EngFirePb { .. })
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    out.trim_end_matches('_').to_owned()
}

/// The voice key the plugin plays for a read item, `e_<name>[_<label>]`.
pub fn ecam_voice_key(item: &EcamItem) -> String {
    let mut key = format!("e_{}", slug(item.name));
    if !item.label.is_empty() {
        key.push('_');
        key.push_str(&slug(item.label));
    }
    key.truncate(64);
    key.trim_end_matches('_').to_owned()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    Arrive,
    AwaitConfirm(Switch),
    AwaitSensed,
}

#[derive(Clone, Debug)]
pub struct EcamRunner {
    proc: EcamProcedure,
    idx: usize,
    stage: Stage,
    last_done_t: f64,
    skip_above: Option<u8>,
    done: bool,
}

impl EcamRunner {
    pub fn new(proc: EcamProcedure, t: f64) -> Self {
        EcamRunner { proc, idx: 0, stage: Stage::Arrive, last_done_t: t, skip_above: None, done: false }
    }

    pub fn procedure(&self) -> u32 {
        self.proc.id
    }

    pub fn finished(&self) -> bool {
        self.done
    }

    pub fn awaiting_confirmation(&self) -> bool {
        matches!(self.stage, Stage::AwaitConfirm(_))
    }

    fn complete(&mut self, t: f64) {
        self.idx += 1;
        self.stage = Stage::Arrive;
        self.last_done_t = t;
    }

    pub fn step(&mut self, t: f64, live: Option<&EcamLive>, confirmed: bool, out: &mut Vec<Action>) {
        let id = self.proc.id;
        let sensed_done = |i: usize| live.filter(|l| l.procedure == id).and_then(|l| l.done.get(i).copied()).unwrap_or(false);
        for _ in 0..self.proc.items.len() + 1 {
            if self.done {
                return;
            }
            let Some(item) = self.proc.items.get(self.idx).copied() else {
                self.done = true;
                out.push(Action::Say("ecam_actions_complete"));
                return;
            };
            if let Some(level) = self.skip_above {
                if item.level > level {
                    self.idx += 1;
                    continue;
                }
                self.skip_above = None;
            }
            let due = item.time.map_or(0., |s| self.last_done_t + s as f64);
            match (item.kind, self.stage) {
                (ItemKind::Plain, _) => {
                    out.push(Action::ReadEcam { procedure: id, item: self.idx });
                    self.idx += 1;
                }
                (ItemKind::Condition, _) => {
                    if t < due {
                        return;
                    }
                    if sensed_done(self.idx) {
                        out.push(Action::ReadEcam { procedure: id, item: self.idx });
                    } else {
                        self.skip_above = Some(item.level);
                    }
                    self.idx += 1;
                }
                (ItemKind::Action, Stage::Arrive) => {
                    if item.sensed && sensed_done(self.idx) {
                        self.complete(self.last_done_t);
                        continue;
                    }
                    if t < due {
                        return;
                    }
                    out.push(Action::ReadEcam { procedure: id, item: self.idx });
                    match doer(&item) {
                        Doer::Pm(sw) if needs_confirmation(sw) => {
                            out.push(Action::Say("confirm"));
                            self.stage = Stage::AwaitConfirm(sw);
                            return;
                        }
                        Doer::Pm(sw) => {
                            out.push(Action::Switch(sw));
                            if !item.sensed {
                                out.push(Action::TickEcam { procedure: id, item: self.idx });
                                self.complete(t);
                                continue;
                            }
                            self.stage = Stage::AwaitSensed;
                            return;
                        }
                        Doer::Atc => {
                            out.push(Action::AtcNotify);
                            out.push(Action::TickEcam { procedure: id, item: self.idx });
                            self.complete(t);
                        }
                        Doer::Tick => {
                            out.push(Action::TickEcam { procedure: id, item: self.idx });
                            self.complete(t);
                        }
                        Doer::Pf | Doer::Unmapped => {
                            self.stage = Stage::AwaitSensed;
                            return;
                        }
                    }
                }
                (ItemKind::Action, Stage::AwaitConfirm(sw)) => {
                    if !confirmed {
                        return;
                    }
                    out.push(Action::Switch(sw));
                    self.stage = Stage::AwaitSensed;
                    return;
                }
                (ItemKind::Action, Stage::AwaitSensed) => {
                    if !sensed_done(self.idx) {
                        return;
                    }
                    self.complete(t);
                }
            }
        }
    }
}

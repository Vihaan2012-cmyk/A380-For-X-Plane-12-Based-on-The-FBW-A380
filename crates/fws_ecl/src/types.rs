#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineStyle {
    Standard,
    Cyan,
    Green,
    GreenTable,
    Amber,
    White,
    Red,
    Headline,
    SubHeadline,
    CenteredSubHeadline,
    SeparationLine,
    Empty,
    ChecklistItem,
    CompletedChecklist,
    CompletedDeferredProcedure,
    DeferredProcedure,
    OmissionDots,
    LandAsap,
    LandAnsa,
    ChecklistCondition,
    ChecklistItemInactive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Special,
    Condition,
    Action {
        label_not_completed: &'static str,
        label_completed: Option<&'static str>,
        colon_if_completed: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub name: &'static str,
    pub sensed: bool,
    pub level: u8,
    pub style: Option<LineStyle>,
    pub kind: ItemKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Check {
    pub expr: &'static str,
    pub inputs: &'static [&'static str],
}

#[derive(Clone, Copy, Debug)]
pub struct NormalProcedure {
    pub id: u32,
    pub title: &'static str,
    pub only_activated_by_request: bool,
    pub deferred: bool,
    pub items: &'static [Item],
    pub checks: &'static [Option<Check>],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message {
    pub id: &'static str,
    pub raw: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RulePhase {
    AllPhases,
    ApprLdg,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoSelect {
    Fixed0,
    Ternary {
        input: &'static str,
        if_true: u8,
        if_false: u8,
    },
    MoreThanOneEngineRunning {
        if_true: u8,
        if_false: u8,
    },
    ToMemo,
    LdgMemo,
    AdirsAlignOver3Min,
    AdirsAlignUnder3Min,
    OxyPaxSys,
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoInhibit {
    Never,
    ToOrLdgMemoActive,
    Custom,
}

#[derive(Clone, Copy, Debug)]
pub struct MemoRule {
    pub key: &'static str,
    pub flight_phase_inhib: &'static [u8],
    pub active_expr: &'static str,
    pub active_inputs: &'static [&'static str],
    pub select: MemoSelect,
    pub select_expr: &'static str,
    pub select_inputs: &'static [&'static str],
    pub codes: &'static [&'static str],
    pub inhibit: MemoInhibit,
    pub inhibit_inputs: &'static [&'static str],
    pub left_side: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct LimitationRule {
    pub id: &'static str,
    pub phase: RulePhase,
    pub pfd: bool,
    pub active_expr: &'static str,
    pub active_inputs: &'static [&'static str],
    pub not_active_when_item_active: &'static [&'static str],
    pub monitor_confirm_time: Option<f32>,
}

#[derive(Clone, Copy, Debug)]
pub struct InfoRule {
    pub id: &'static str,
    pub active_expr: &'static str,
    pub active_inputs: &'static [&'static str],
    pub not_active_when_item_active: &'static [&'static str],
    pub monitor_confirm_time: Option<f32>,
}

#[derive(Clone, Copy, Debug)]
pub struct InopRule {
    pub id: &'static str,
    pub phase: RulePhase,
    pub redundancy_loss: bool,
    pub active_expr: &'static str,
    pub active_inputs: &'static [&'static str],
    pub not_active_when_item_active: &'static [&'static str],
    pub monitor_confirm_time: Option<f32>,
}

impl MemoRule {
    pub fn code_for(&self, index: usize) -> Option<&'static str> {
        self.codes.get(index).copied()
    }

    pub fn phase_inhibited(&self, flight_phase: u8) -> bool {
        self.flight_phase_inhib.contains(&flight_phase)
    }
}

impl Message {
    pub fn color(&self) -> Option<u8> {
        let b = self.raw.trim_start().as_bytes();
        if b.len() >= 4 && b[0] == 0x1b && b[1] == b'<' && b[3] == b'm' && b[2].is_ascii_digit() {
            Some(b[2] - b'0')
        } else {
            None
        }
    }

    pub fn plain(&self) -> String {
        let mut out = String::new();
        let mut chars = self.raw.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '\u{1b}' {
                out.push(c);
                continue;
            }
            while let Some(&n) = chars.peek() {
                chars.next();
                if n == 'm' {
                    break;
                }
            }
        }
        out.trim_matches(|c: char| c.is_ascii_whitespace()).to_string()
    }

    pub fn priority(&self) -> u8 {
        crate::logic::message_priority(Some(self.raw))
    }
}

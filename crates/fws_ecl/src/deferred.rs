use fws_data::{Deferred, DeferredType, Item, ItemKind, Style};

pub const SPECIAL_INDEX_ACTIVATE: i32 = -2;
pub const SPECIAL_INDEX_DEFERRED_PROC_COMPLETE: i32 = -3;
pub const SPECIAL_INDEX_DEFERRED_PROC_RECALL: i32 = -4;

const NON_SELECTABLE: [Style; 6] = [
    Style::Headline,
    Style::OmissionDots,
    Style::SeparationLine,
    Style::SubHeadline,
    Style::CenteredSubHeadline,
    Style::Red,
];

pub fn procedure(id: u32) -> Option<&'static Deferred> {
    fws_data::deferred(id)
}

/// Slot in ALL PHASES, AT TOP OF DESCENT, FOR APPROACH, FOR LANDING.
pub fn slot(kind: DeferredType) -> usize {
    match kind {
        DeferredType::AllPhases => 0,
        DeferredType::AtTopOfDescent => 1,
        DeferredType::ForApproach => 2,
        DeferredType::ForLanding => 3,
    }
}

pub fn selectable(item: &Item) -> bool {
    !NON_SELECTABLE.contains(&item.style)
}

/// `ProcedureLinesGenerator.conditionalActiveItems` without timers (deferred procedures carry none).
pub fn conditional_active_items(items: &[Item], checked: &mut [bool], active: &mut [bool], to_show: &[bool]) {
    let mut level_ok = [true; 8];
    let mut previous: Option<usize> = None;
    for (i, item) in items.iter().enumerate() {
        let level = item.level as usize;
        if let Some(p) = previous {
            if p > level && level_ok[level] {
                for l in level + 1..=p {
                    level_ok[l] = true;
                }
            }
        }
        let is_active = (1..=level).all(|l| level_ok[l]);
        if item.kind == ItemKind::Condition && to_show.get(i).copied().unwrap_or(true) {
            level_ok[level + 1] = checked[i];
        }
        active[i] = is_active;
        previous = Some(level);
    }
}

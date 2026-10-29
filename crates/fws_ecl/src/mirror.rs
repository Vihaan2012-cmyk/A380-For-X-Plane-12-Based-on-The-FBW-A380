use crate::ecl::{sensed_items, Ecl, SensedInputs, SPECIAL_INDEX_NORMAL_CL_COMPLETE, SPECIAL_INDEX_NORMAL_RESET};
use crate::normal_data::NORMAL_PROCEDURES;
use crate::types::{ItemKind, LineStyle};

/// What the EFB checklist page renders. `efb_page_id` is the existing app/ui page id ("n:<index>").
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EclMirror {
    pub ecl_shown: bool,
    pub open_checklist_id: u32,
    pub open_efb_page_id: Option<String>,
    pub selected_item: Option<usize>,
    pub selected_special: Option<SpecialLine>,
    pub checklists: Vec<ChecklistMirror>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SpecialLine {
    ClComplete,
    Reset,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChecklistMirror {
    pub id: u32,
    pub efb_page_id: String,
    pub title: String,
    pub in_menu: bool,
    pub completed: bool,
    pub deferred: bool,
    pub items: Vec<ItemMirror>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ItemMirror {
    pub name: String,
    pub value: String,
    pub level: u8,
    pub headline: bool,
    pub sensed: bool,
    pub checked: bool,
    /// Live sensed value, also while the ECL is closed (FBW only copies it into `checked` while shown).
    pub sensed_now: Option<bool>,
}

pub fn efb_page_id(index: usize) -> String {
    format!("n:{index}")
}

impl EclMirror {
    pub fn from_ecl(ecl: &Ecl, live: &SensedInputs) -> Self {
        let checklists = NORMAL_PROCEDURES
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let st = ecl.state(p.id).unwrap();
                let now = sensed_items(p.id, live, ecl.rudder_trim);
                let items = p
                    .items
                    .iter()
                    .enumerate()
                    .map(|(k, it)| {
                        let checked = st.items_checked[k];
                        let value = match it.kind {
                            ItemKind::Action { label_not_completed, label_completed, .. } => {
                                if checked { label_completed.unwrap_or(label_not_completed) } else { label_not_completed }
                            }
                            _ => "",
                        };
                        let headline = matches!(it.style, Some(LineStyle::SubHeadline | LineStyle::Headline | LineStyle::CenteredSubHeadline));
                        ItemMirror {
                            name: it.name.to_string(),
                            value: value.to_string(),
                            level: it.level,
                            headline,
                            sensed: it.sensed && !headline,
                            checked,
                            sensed_now: if it.sensed && !headline { now.get(k).copied().flatten() } else { None },
                        }
                    })
                    .collect();
                ChecklistMirror {
                    id: p.id,
                    efb_page_id: efb_page_id(i),
                    title: p.title.to_string(),
                    in_menu: ecl.overview.items_to_show[i],
                    completed: if p.deferred { ecl.overview.items_checked[i] } else { st.procedure_completed },
                    deferred: p.deferred,
                    items,
                }
            })
            .collect();
        let open = ecl.checklist_id;
        let sel = ecl.selected_line;
        Self {
            ecl_shown: ecl.shown,
            open_checklist_id: open,
            open_efb_page_id: Ecl::checklist_index(open).map(efb_page_id),
            selected_item: usize::try_from(sel).ok(),
            selected_special: match sel {
                SPECIAL_INDEX_NORMAL_CL_COMPLETE => Some(SpecialLine::ClComplete),
                SPECIAL_INDEX_NORMAL_RESET => Some(SpecialLine::Reset),
                _ => None,
            },
            checklists,
        }
    }

    pub fn efb_ticks(&self, efb_page_id: &str) -> Vec<usize> {
        self.checklists
            .iter()
            .find(|c| c.efb_page_id == efb_page_id)
            .map(|c| c.items.iter().enumerate().filter(|(_, it)| it.checked).map(|(k, _)| k).collect())
            .unwrap_or_default()
    }
}

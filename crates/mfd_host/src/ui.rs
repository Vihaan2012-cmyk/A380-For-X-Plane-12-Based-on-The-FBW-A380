use std::collections::BTreeMap;

use crate::page::{Page, Uri};

/// What the MFD shows and what the pilot is doing with it, beside the FMS data.
#[derive(Clone, Debug)]
pub struct UiState {
    pub captain: bool,
    pub uri: Uri,
    stack: Vec<String>,
    /// Act of the dropdown, button menu or context menu that is open.
    pub open: Option<String>,
    /// scrollTop in px of the open dropdown menu.
    pub menu_scroll: f64,
    pub page_selector: Option<usize>,
    /// Act of the input field being typed into.
    pub focus: Option<String>,
    pub scratchpad: String,
    pub cursor: (f64, f64),
    pub keyboard: bool,
    pub tabs: BTreeMap<Page, usize>,
    pub fpln_scroll: Option<usize>,
    pub revised_leg: Option<usize>,
    pub revised_altn: bool,
    pub efob_and_wind: bool,
    pub message_list_open: bool,
    pub metric: bool,
    pub dir_to_selected: Option<usize>,
    pub dir_to_option: usize,
    /// A waypoint typed into the DIR TO field instead of picked from the list.
    pub dir_to_text: String,
    pub new_dest_open: bool,
    pub insert_next_open: bool,
    pub pending_from: Option<String>,
    pub pending_to: Option<String>,
    /// A message FBW puts in the footer when an entry is refused.
    pub entry_message: Option<String>,
    /// FMS messages the pilot cleared with CLEAR INFO while the host still lists them.
    pub cleared_messages: Vec<String>,
}

impl Ord for Page {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

impl PartialOrd for Page {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl UiState {
    pub fn new(captain: bool) -> UiState {
        let start = "fms/active/init";
        UiState {
            captain,
            uri: Uri::parse(start),
            stack: vec![start.to_string()],
            open: None,
            menu_scroll: 0.,
            page_selector: None,
            focus: None,
            scratchpad: String::new(),
            cursor: (0., 0.),
            keyboard: false,
            tabs: BTreeMap::new(),
            fpln_scroll: None,
            revised_leg: None,
            revised_altn: false,
            efob_and_wind: false,
            message_list_open: false,
            metric: true,
            dir_to_selected: None,
            dir_to_option: 0,
            dir_to_text: String::new(),
            new_dest_open: false,
            insert_next_open: false,
            pending_from: None,
            pending_to: None,
            entry_message: None,
            cleared_messages: Vec::new(),
        }
    }

    pub fn side(&self) -> &'static str {
        if self.captain {
            "CAPT"
        } else {
            "FO"
        }
    }

    pub fn page(&self) -> Page {
        self.uri.page()
    }

    pub fn tab(&self, page: Page, default: usize) -> usize {
        self.tabs.get(&page).copied().unwrap_or(default)
    }

    pub fn close_popups(&mut self) {
        self.open = None;
        self.menu_scroll = 0.;
        self.page_selector = None;
    }

    /// FBW `MfdUiService.navigateTo`: same URI ignored except F-PLN top and PERF, `back` pops.
    pub fn navigate(&mut self, uri: &str) {
        if uri == self.uri.uri && !matches!(uri, "fms/active/f-pln/top" | "fms/active/perf") {
            return;
        }
        self.release_keyboard();
        self.close_popups();
        self.new_dest_open = false;
        self.insert_next_open = false;
        let next = if uri == "back" {
            if self.stack.len() < 2 {
                return;
            }
            self.stack.pop();
            self.stack.last().cloned().unwrap_or_default()
        } else {
            self.stack.push(uri.to_string());
            if self.stack.len() > 32 {
                self.stack.remove(0);
            }
            uri.to_string()
        };
        let before = self.uri.page();
        self.uri = Uri::parse(&next);
        if self.uri.page() != before || self.uri.page == "f-pln" {
            self.fpln_scroll = None;
        }
        self.tabs.remove(&self.uri.page());
        if self.uri.page() == crate::page::Page::DirTo {
            self.dir_to_selected = None;
            self.dir_to_option = 0;
            self.dir_to_text.clear();
        }
        if self.uri.page() == crate::page::Page::Perf {
            let tab = match self.uri.extra.as_str() {
                "to" => Some(0),
                "clb" => Some(1),
                "crz" => Some(2),
                "des" => Some(3),
                "appr" => Some(4),
                "ga" => Some(5),
                _ => None,
            };
            if let Some(t) = tab {
                self.tabs.insert(crate::page::Page::Perf, t);
            }
        }
        if self.uri.page() == crate::page::Page::SecIndex {
            if let Ok(n) = self.uri.extra.parse::<usize>() {
                self.tabs.insert(crate::page::Page::SecIndex, n.saturating_sub(1).min(2));
            }
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.stack.len() > 1
    }

    pub fn focus_field(&mut self, act: &str) {
        self.close_popups();
        self.focus = Some(act.to_string());
        self.scratchpad.clear();
        self.keyboard = true;
    }

    pub fn release_keyboard(&mut self) {
        self.focus = None;
        self.scratchpad.clear();
        self.keyboard = false;
    }
}

pub mod hit;
pub mod input;
pub mod model;
pub mod page;
pub mod sources;
pub mod tree;
pub mod ui;

pub use input::{Key, Out};
pub use sources::Sources;
pub use ui::UiState;

use fms::navdb::NavDatabase;
use mfd_tree::Node;

/// Both MFD sides: UI state, the last built tree and its flattened elements for hit tests.
pub struct Mfd {
    pub capt: UiState,
    pub fo: UiState,
    pub tree: Node,
    pub flat: Vec<hit::Flat>,
    pub boxes: Vec<[f32; 4]>,
}

impl Default for Mfd {
    fn default() -> Self {
        Mfd { capt: UiState::new(true), fo: UiState::new(false), tree: Node::new("body"), flat: Vec::new(), boxes: Vec::new() }
    }
}

impl Mfd {
    pub fn build(&mut self, src: &Sources) -> &Node {
        self.tree = tree::screen(&self.capt, &self.fo, src);
        self.flat = hit::flatten(&self.tree);
        &self.tree
    }

    pub fn act(&mut self, src: &Sources, act: &str) -> Vec<Out> {
        let (side, a) = act.split_once('|').unwrap_or(("c", act));
        let ui = if side == "f" { &mut self.fo } else { &mut self.capt };
        input::click(ui, src, a)
    }

    /// The `data-act` of what the pointer is over.
    pub fn act_at(&self, x: f64, y: f64) -> Option<String> {
        hit::hit(&self.flat, &self.boxes, x, y)
    }

    pub fn pointer(&mut self, src: &Sources, x: f64, y: f64) -> Vec<Out> {
        match hit::hit(&self.flat, &self.boxes, x, y) {
            Some(a) => self.act(src, &a),
            None => {
                let ui = if x >= 768. + 110. { &mut self.fo } else { &mut self.capt };
                ui.close_popups();
                input::blur(ui, src)
            }
        }
    }

    /// The wheel over an open dropdown menu: FBW menus are `overflow: auto`, 275 px high.
    /// `dy` is in CSS pixels, positive down. Returns whether a menu took it.
    pub fn wheel(&mut self, x: f64, dy: f64) -> bool {
        let ui = if x >= 768. + 110. { &mut self.fo } else { &mut self.capt };
        let Some(open) = ui.open.clone() else { return false };
        let items = ["dd", "bmenu"].iter().find_map(|k| tree::menu_items(&self.tree, &format!("{k}:{open}"))).unwrap_or(0);
        let max = (items as f64 * 33. - 270.).max(0.);
        if max <= 0. {
            return false;
        }
        ui.menu_scroll = (ui.menu_scroll + dy).clamp(0., max);
        true
    }

    pub fn keyboard(&self) -> bool {
        self.capt.keyboard || self.fo.keyboard
    }

    pub fn key(&mut self, src: &Sources, db: Option<&dyn NavDatabase>, k: Key) -> (bool, Vec<Out>) {
        let ui = if self.fo.keyboard { &mut self.fo } else { &mut self.capt };
        input::key(ui, src, db, k)
    }
}

#[cfg(test)]
mod tests;

use mfd_tree::Node;

pub const CARET: char = '|';
pub const BLINK_MS: f64 = 530.0;

#[derive(Clone, Debug, PartialEq)]
pub enum Key {
    Char(char),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    Enter,
    Escape,
    Tab,
    SelectAll,
    Copy,
    Cut,
    Paste(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Edited,
    Moved,
    Commit,
    Release,
    Copy(String),
    Nothing,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextEdit {
    pub text: String,
    pub caret: usize,
    pub all: bool,
    pub numeric: bool,
    pub max_len: Option<usize>,
}

impl TextEdit {
    pub fn new(text: &str, numeric: bool, max_len: Option<usize>) -> TextEdit {
        TextEdit { caret: text.chars().count(), text: text.to_string(), all: false, numeric, max_len }
    }

    fn len(&self) -> usize {
        self.text.chars().count()
    }

    fn byte(&self, ci: usize) -> usize {
        self.text.char_indices().nth(ci).map_or(self.text.len(), |(b, _)| b)
    }

    fn accepts(&self, c: char) -> bool {
        if c.is_control() {
            return false;
        }
        !self.numeric || c.is_ascii_digit() || c == '.' || c == '-'
    }

    fn take_all(&mut self) -> bool {
        if self.all {
            self.text.clear();
            self.caret = 0;
            self.all = false;
            return true;
        }
        false
    }

    fn insert(&mut self, s: &str) -> Outcome {
        let s: String = s.chars().filter(|c| self.accepts(*c)).collect();
        if s.is_empty() {
            return Outcome::Nothing;
        }
        self.take_all();
        let room = self.max_len.map_or(usize::MAX, |m| m.saturating_sub(self.len()));
        let s: String = s.chars().take(room).collect();
        if s.is_empty() {
            return Outcome::Nothing;
        }
        let at = self.byte(self.caret);
        self.text.insert_str(at, &s);
        self.caret += s.chars().count();
        Outcome::Edited
    }

    pub fn apply(&mut self, k: Key) -> Outcome {
        match k {
            Key::Char(c) => self.insert(&c.to_string()),
            Key::Paste(s) => self.insert(&s),
            Key::Backspace => {
                if self.take_all() {
                    return Outcome::Edited;
                }
                if self.caret == 0 {
                    return Outcome::Nothing;
                }
                let a = self.byte(self.caret - 1);
                let b = self.byte(self.caret);
                self.text.replace_range(a..b, "");
                self.caret -= 1;
                Outcome::Edited
            }
            Key::Delete => {
                if self.take_all() {
                    return Outcome::Edited;
                }
                if self.caret >= self.len() {
                    return Outcome::Nothing;
                }
                let a = self.byte(self.caret);
                let b = self.byte(self.caret + 1);
                self.text.replace_range(a..b, "");
                Outcome::Edited
            }
            Key::Left | Key::Right | Key::Home | Key::End => {
                let n = self.len();
                let was = (self.caret, self.all);
                self.caret = match k {
                    Key::Left if self.all => 0,
                    Key::Right if self.all => n,
                    Key::Left => self.caret.saturating_sub(1),
                    Key::Right => (self.caret + 1).min(n),
                    Key::Home => 0,
                    _ => n,
                };
                self.all = false;
                if was == (self.caret, self.all) {
                    Outcome::Nothing
                } else {
                    Outcome::Moved
                }
            }
            Key::SelectAll => {
                self.all = !self.text.is_empty();
                self.caret = self.len();
                Outcome::Moved
            }
            Key::Copy => Outcome::Copy(self.text.clone()),
            Key::Cut => {
                let t = self.text.clone();
                self.text.clear();
                self.caret = 0;
                self.all = false;
                Outcome::Copy(t)
            }
            Key::Enter | Key::Tab => Outcome::Commit,
            Key::Escape => Outcome::Release,
        }
    }

    /// What the field shows: the text with the caret drawn into it.
    pub fn display(&self, caret_on: bool) -> String {
        if !caret_on {
            return self.text.clone();
        }
        let at = self.byte(self.caret);
        let mut s = String::with_capacity(self.text.len() + 1);
        s.push_str(&self.text[..at]);
        s.push(CARET);
        s.push_str(&self.text[at..]);
        s
    }
}

/// XPLM virtual keys and flags (XPLMDefs.h).
pub mod vk {
    pub const BACK: u32 = 0x08;
    pub const TAB: u32 = 0x09;
    pub const RETURN: u32 = 0x0D;
    pub const ESCAPE: u32 = 0x1B;
    pub const END: u32 = 0x23;
    pub const HOME: u32 = 0x24;
    pub const LEFT: u32 = 0x25;
    pub const RIGHT: u32 = 0x27;
    pub const DELETE: u32 = 0x2E;
    pub const A: u32 = 0x41;
    pub const C: u32 = 0x43;
    pub const V: u32 = 0x56;
    pub const X: u32 = 0x58;
    pub const ENTER: u32 = 0x9C;
    pub const NUMPAD_ENT: u32 = 0xBB;
    pub const SHIFT_FLAG: u32 = 1;
    pub const CTRL_FLAG: u32 = 4;
    pub const DOWN_FLAG: u32 = 8;
    pub const UP_FLAG: u32 = 16;
}

/// One X-Plane key callback as an edit, `None` for key-ups and keys a text
/// field does not use. `clipboard` is read only for Ctrl+V.
pub fn key_of(ch: u32, vkey: u32, flags: u32, clipboard: &dyn Fn() -> String) -> Option<Key> {
    if flags & vk::UP_FLAG != 0 {
        return None;
    }
    if flags & vk::CTRL_FLAG != 0 {
        return match vkey {
            vk::A => Some(Key::SelectAll),
            vk::C => Some(Key::Copy),
            vk::X => Some(Key::Cut),
            vk::V => Some(Key::Paste(clipboard())),
            _ => None,
        };
    }
    match vkey {
        vk::BACK => return Some(Key::Backspace),
        vk::DELETE => return Some(Key::Delete),
        vk::LEFT => return Some(Key::Left),
        vk::RIGHT => return Some(Key::Right),
        vk::HOME => return Some(Key::Home),
        vk::END => return Some(Key::End),
        vk::RETURN | vk::ENTER | vk::NUMPAD_ENT => return Some(Key::Enter),
        vk::ESCAPE => return Some(Key::Escape),
        vk::TAB => return Some(Key::Tab),
        _ => {}
    }
    match ch {
        0 => None,
        8 => Some(Key::Backspace),
        13 => Some(Key::Enter),
        27 => Some(Key::Escape),
        9 => Some(Key::Tab),
        c => char::from_u32(c).filter(|c| !c.is_control()).map(Key::Char),
    }
}

const LETTERS: [&str; 3] = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];
const SYMBOLS: [&str; 3] = ["1234567890", "-/:;()$&@\"", ".,?!'#%*+"];

/// The on-screen keyboard, docked at the bottom of the tablet.
pub fn keyboard(shift: bool, symbols: bool, numeric: bool) -> Node {
    let key = |label: &str, act: &str, extra: &str| Node::el("button", &format!("efbn-key {extra}")).act(act).text(label);
    let mut kb = Node::el("div", "efbn-kb").id("efbn-kb");
    let rows: [&str; 3] = if symbols || numeric { SYMBOLS } else { LETTERS };
    for (ri, row) in rows.iter().enumerate() {
        let mut r = Node::el("div", "efbn-kb-row");
        if ri == 2 {
            r = r.child(if symbols || numeric { key("ABC", "kb:abc", "wide") } else { key("shift", "kb:shift", if shift { "wide on" } else { "wide" }) });
        }
        for c in row.chars() {
            let c = if shift && !symbols && !numeric { c.to_ascii_uppercase() } else { c };
            let label = c.to_string();
            r = r.child(key(&label, &format!("kb:char:{}", c as u32), ""));
        }
        if ri == 2 {
            r = r.child(key("del", "kb:bksp", "wide"));
        }
        kb = kb.child(r);
    }
    let last = Node::el("div", "efbn-kb-row")
        .child(if symbols || numeric { key("ABC", "kb:abc", "wide") } else { key("123", "kb:sym", "wide") })
        .child(key("space", "kb:char:32", "space"))
        .child(key("done", "kb:enter", "wide go"))
        .child(key("hide", "kb:hide", "wide"));
    kb.child(last)
}

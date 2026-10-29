pub mod deferred;
pub mod ecl;
pub mod logic;
pub mod mirror;
pub mod types;

mod message_data;
mod normal_data;
mod rule_data;

pub use message_data::{
    A_THR_OFF_MEMO_KEY, INFO_MESSAGES, INOP_SYS_MESSAGES, LIMITATION_MESSAGES, MEMO_MESSAGES, PFD_MEMO_DISPLAY,
};
pub use normal_data::{DEFERRED_PROCEDURE_IDS, NORMAL_PROCEDURES, WD_LINE_CHARACTERS, WD_NUM_LINES};
pub use rule_data::{EWD_MEMOS, INFO_RULES, INOP_SYS_RULES, LIMITATION_RULES, TO_LDG_MEMOS};
pub use types::*;

fn find<'a>(table: &'a [Message], id: &str) -> Option<&'a Message> {
    table.iter().find(|m| m.id == id)
}

pub fn memo_message(id: &str) -> Option<&'static Message> {
    find(MEMO_MESSAGES, id)
}

pub fn info_message(id: &str) -> Option<&'static Message> {
    find(INFO_MESSAGES, id)
}

pub fn limitation_message(id: &str) -> Option<&'static Message> {
    find(LIMITATION_MESSAGES, id)
}

pub fn inop_sys_message(id: &str) -> Option<&'static Message> {
    find(INOP_SYS_MESSAGES, id)
}

pub fn normal_procedure(id: u32) -> Option<&'static NormalProcedure> {
    NORMAL_PROCEDURES.iter().find(|p| p.id == id)
}

pub fn pfd_shows_memo(id: &str) -> bool {
    PFD_MEMO_DISPLAY.contains(&id)
}

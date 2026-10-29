mod generated;
mod types;

pub use generated::*;
pub use types::*;

pub fn procedure(id: u32) -> Option<&'static Procedure> {
    PROCEDURES.binary_search_by_key(&id, |p| p.id).ok().map(|i| &PROCEDURES[i])
}

pub fn alert(id: u32) -> Option<&'static Alert> {
    ALERTS.binary_search_by_key(&id, |a| a.id).ok().map(|i| &ALERTS[i])
}

pub fn deferred(id: u32) -> Option<&'static Deferred> {
    DEFERRED.binary_search_by_key(&id, |d| d.id).ok().map(|i| &DEFERRED[i])
}

pub fn procedures_in(ata: AtaFile) -> impl Iterator<Item = &'static Procedure> {
    PROCEDURES.iter().filter(move |p| p.ata == ata)
}

pub fn procedures_in_chapter(chapter: u32) -> impl Iterator<Item = &'static Procedure> {
    PROCEDURES.iter().filter(move |p| p.chapter() == chapter)
}

#[cfg(test)]
mod tests;

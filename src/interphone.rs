//! Cockpit-to-ground interphone requests as X-Plane commands
//! (`fbw/interphone/<request>`), so they can be bound to keys or VR
//! buttons. Each press counts up `fbw/efb/interphone/<request>`; the EFB's
//! ground engineer (`ground_comms.js`) answers on the next count it sees.

use crate::published::{self, Command, Published, Value};

pub const REQUESTS: [(&str, &str); 10] = [
    ("call", "Interphone: ground, cockpit (call the ground engineer)"),
    ("push", "Interphone: ready for pushback"),
    ("start", "Interphone: request engine start"),
    ("stop", "Interphone: stop the pushback"),
    ("disconnect", "Interphone: parking brake set, disconnect the tug"),
    ("chocks", "Interphone: chocks in or out"),
    ("covers", "Interphone: remove covers and pins"),
    ("gpu", "Interphone: connect or remove ground power"),
    ("asu", "Interphone: connect or remove the air start unit"),
    ("pca", "Interphone: connect or remove preconditioned air"),
];

pub struct Interphone {
    _published: Published,
    requests: Vec<(Command, Value)>,
}

impl Interphone {
    pub fn new() -> Self {
        let mut p = Published::default();
        let requests = REQUESTS
            .iter()
            .map(|(name, description)| (p.command(&format!("fbw/interphone/{name}"), description), p.number(&format!("fbw/efb/interphone/{name}"), 0., false)))
            .collect();
        Self { _published: p, requests }
    }

    pub fn update(&mut self) {
        for (command, count) in &self.requests {
            let n = published::presses(*command);
            if n > 0 {
                published::set(*count, published::get(*count) + n as f64);
            }
        }
    }
}

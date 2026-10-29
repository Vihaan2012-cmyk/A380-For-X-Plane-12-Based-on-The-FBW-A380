//! The EFB's heavy data sources (Study pages, failures, components, breakers),
//! made on a background thread about once a second while a tab asks for them,
//! so the flight loop only ever takes a finished copy.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    Pages,
    Failures,
    Components,
    Breakers,
}

const SOURCES: [Source; 4] = [Source::Pages, Source::Failures, Source::Components, Source::Breakers];
const PERIOD: Duration = Duration::from_millis(1000);
const WANTED_FOR: Duration = Duration::from_secs(5);

#[derive(Default)]
struct Slot {
    value: Option<Arc<Value>>,
    text_hash: u64,
    version: u64,
    wanted_at: Option<Instant>,
    made_at: Option<Instant>,
    made_for: u64,
}

/// How often a source is remade at most, and whether it only changes with
/// the failure state.
fn cadence(s: Source) -> (Duration, bool) {
    match s {
        Source::Pages => (Duration::from_secs(10), true),
        Source::Failures => (Duration::from_secs(30), true),
        Source::Components => (Duration::from_secs(5), false),
        Source::Breakers => (PERIOD, false),
    }
}

fn failure_signature() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut ids = crate::failures::active_ids();
    ids.sort_unstable();
    for id in ids {
        id.hash(&mut h);
        crate::failures::magnitude(id).to_bits().hash(&mut h);
    }
    crate::mel::deferred_remaining().len().hash(&mut h);
    h.finish()
}

#[derive(Default)]
struct Cache {
    slots: [Slot; 4],
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
static STARTED: OnceLock<()> = OnceLock::new();
static WAKES: AtomicU64 = AtomicU64::new(0);

fn index(s: Source) -> usize {
    SOURCES.iter().position(|x| *x == s).unwrap_or(0)
}

fn make(s: Source) -> String {
    use crate::study::web;
    match s {
        Source::Pages => web::pages_json(),
        Source::Failures => web::failures_json(),
        Source::Components => web::components_json(),
        Source::Breakers => web::breakers_json(),
    }
}

fn hash(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

fn due(s: Source, signature: u64) -> bool {
    let (every, on_failures) = cadence(s);
    CACHE
        .lock()
        .ok()
        .and_then(|g| {
            g.as_ref().map(|c| {
                let slot = &c.slots[index(s)];
                slot.value.is_none() || slot.made_at.is_none_or(|t| t.elapsed() >= every) || (on_failures && slot.made_for != signature)
            })
        })
        .unwrap_or(true)
}

fn refresh(s: Source) {
    let signature = failure_signature();
    if !due(s, signature) {
        return;
    }
    if let Ok(mut g) = CACHE.lock() {
        let slot = &mut g.get_or_insert_with(Cache::default).slots[index(s)];
        slot.made_at = Some(Instant::now());
        slot.made_for = signature;
    }
    let text = make(s);
    let h = hash(&text);
    let same = CACHE.lock().ok().and_then(|g| g.as_ref().map(|c| c.slots[index(s)].text_hash == h && c.slots[index(s)].value.is_some())).unwrap_or(false);
    if same {
        return;
    }
    let value = serde_json::from_str::<Value>(&text).unwrap_or(Value::Null);
    if let Ok(mut g) = CACHE.lock() {
        let slot = &mut g.get_or_insert_with(Cache::default).slots[index(s)];
        slot.value = Some(Arc::new(value));
        slot.text_hash = h;
        slot.version += 1;
    }
}

fn start() {
    STARTED.get_or_init(|| {
        let _ = std::thread::Builder::new().name("fbw-efb-data".into()).spawn(|| loop {
            let wanted: Vec<Source> = CACHE
                .lock()
                .ok()
                .and_then(|g| g.as_ref().map(|c| SOURCES.iter().copied().filter(|s| c.slots[index(*s)].wanted_at.is_some_and(|t| t.elapsed() < WANTED_FOR)).collect()))
                .unwrap_or_default();
            for s in wanted {
                refresh(s);
            }
            let seen = WAKES.load(Ordering::Relaxed);
            let until = Instant::now() + PERIOD;
            while Instant::now() < until && WAKES.load(Ordering::Relaxed) == seen {
                std::thread::sleep(Duration::from_millis(20));
            }
        });
    });
}

/// The latest `source` and its version (bumped whenever its content changed),
/// asking the background thread to keep it fresh. `None` until the first one
/// is made.
pub fn get(source: Source) -> (Option<Arc<Value>>, u64) {
    #[cfg(test)]
    refresh(source);
    #[cfg(not(test))]
    start();
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let slot = &mut g.get_or_insert_with(Cache::default).slots[index(source)];
    let first = slot.wanted_at.is_none_or(|t| t.elapsed() >= WANTED_FOR);
    slot.wanted_at = Some(Instant::now());
    let out = (slot.value.clone(), slot.version);
    drop(g);
    if first {
        WAKES.fetch_add(1, Ordering::Relaxed);
    }
    out
}

/// Remake `source` soon (after an action changed it).
pub fn poke() {
    if let Ok(mut g) = CACHE.lock() {
        if let Some(c) = g.as_mut() {
            for slot in c.slots.iter_mut() {
                slot.made_at = None;
            }
        }
    }
    WAKES.fetch_add(1, Ordering::Relaxed);
}

/// The versions of every source, for change detection.
pub fn versions() -> [u64; 4] {
    CACHE.lock().ok().and_then(|g| g.as_ref().map(|c| [0, 1, 2, 3].map(|i| c.slots[i].version))).unwrap_or_default()
}

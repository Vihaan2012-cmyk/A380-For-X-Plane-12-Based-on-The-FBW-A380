use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use fms::navdb::NavDatabase;

use crate::command::{Envelope, HostCommand};
use crate::core::{FmsCore, Snapshot};
use crate::facilities::{Facilities, NoFacilities};
use crate::inputs::Inputs;
use crate::outputs::{Outputs, Restarted};

const SNAPSHOT_PERIOD_MS: f64 = 5_000.;

enum Msg {
    Tick(f64, Box<Inputs>),
    Command(Envelope),
    Stop,
}

type Db = Arc<dyn NavDatabase + Send + Sync>;
type Fac = Arc<dyn Facilities>;

struct Shared {
    outputs: Mutex<Outputs>,
    snapshot: Mutex<Option<Snapshot>>,
    restarts: Mutex<u32>,
}

/// Runs `FmsCore` on a worker thread. A panic in the core restores it from the last
/// snapshot (plans, performance data, phase) and reports `Outputs::restarted`; a worker
/// that died is respawned on the next call.
pub struct FmsHost {
    db: Db,
    facilities: Fac,
    tx: Sender<Msg>,
    worker: Option<JoinHandle<()>>,
    shared: Arc<Shared>,
    next_id: u64,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn run(db: Db, facilities: Fac, rx: Receiver<Msg>, shared: Arc<Shared>) {
    let mut core = match lock(&shared.snapshot).as_ref() {
        Some(s) => FmsCore::restore_with(s, facilities.clone()),
        None => FmsCore::with_facilities(facilities.clone()),
    };
    let mut since_snapshot = 0.;
    while let Ok(msg) = rx.recv() {
        if matches!(msg, Msg::Stop) {
            break;
        }
        let mut changed = false;
        let result = catch_unwind(AssertUnwindSafe(|| match msg {
            Msg::Tick(dt, inputs) => {
                since_snapshot += dt;
                Some(core.tick(db.as_ref(), dt, &inputs))
            }
            Msg::Command(env) => {
                core.command(db.as_ref(), env);
                changed = true;
                None
            }
            Msg::Stop => None,
        }));
        match result {
            Ok(Some(out)) => lock(&shared.outputs).merge(out),
            Ok(None) => {}
            Err(e) => {
                let reason = e.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| e.downcast_ref::<String>().cloned()).unwrap_or_else(|| "panic".into());
                let count = {
                    let mut r = lock(&shared.restarts);
                    *r += 1;
                    *r
                };
                core = match lock(&shared.snapshot).as_ref() {
                    Some(s) => FmsCore::restore(s),
                    None => FmsCore::new(),
                };
                lock(&shared.outputs).restarted = Some(Restarted { count, reason });
                continue;
            }
        }
        if changed || since_snapshot >= SNAPSHOT_PERIOD_MS {
            since_snapshot = 0.;
            *lock(&shared.snapshot) = Some(core.snapshot());
        }
    }
}

impl FmsHost {
    pub fn new(db: Db) -> Self {
        Self::with_snapshot(db, None)
    }

    pub fn with_snapshot(db: Db, snapshot: Option<Snapshot>) -> Self {
        Self::with_facilities(db, snapshot, Arc::new(NoFacilities))
    }

    pub fn with_facilities(db: Db, snapshot: Option<Snapshot>, facilities: Fac) -> Self {
        let shared = Arc::new(Shared { outputs: Mutex::new(Outputs::default()), snapshot: Mutex::new(snapshot), restarts: Mutex::new(0) });
        let (tx, worker) = Self::spawn(db.clone(), facilities.clone(), shared.clone());
        FmsHost { db, facilities, tx, worker: Some(worker), shared, next_id: 1 }
    }

    fn spawn(db: Db, facilities: Fac, shared: Arc<Shared>) -> (Sender<Msg>, JoinHandle<()>) {
        let (tx, rx) = channel();
        let worker = std::thread::Builder::new().name("fms_host".into()).spawn(move || run(db, facilities, rx, shared)).expect("fms_host worker");
        (tx, worker)
    }

    fn ensure_alive(&mut self) {
        if self.worker.as_ref().is_none_or(|w| w.is_finished()) {
            let (tx, worker) = Self::spawn(self.db.clone(), self.facilities.clone(), self.shared.clone());
            self.tx = tx;
            self.worker = Some(worker);
            let count = {
                let mut r = lock(&self.shared.restarts);
                *r += 1;
                *r
            };
            lock(&self.shared.outputs).restarted = Some(Restarted { count, reason: "worker exited".into() });
        }
    }

    pub fn send(&mut self, command: HostCommand) -> u64 {
        self.ensure_alive();
        let id = self.next_id;
        self.next_id += 1;
        let _ = self.tx.send(Msg::Command(Envelope { id, command }));
        id
    }

    pub fn tick(&mut self, dt_ms: f64, inputs: Inputs) {
        self.ensure_alive();
        let _ = self.tx.send(Msg::Tick(dt_ms, Box::new(inputs)));
    }

    pub fn take_outputs(&self) -> Outputs {
        std::mem::take(&mut *lock(&self.shared.outputs))
    }

    pub fn snapshot(&self) -> Option<Snapshot> {
        lock(&self.shared.snapshot).clone()
    }

    pub fn restarts(&self) -> u32 {
        *lock(&self.shared.restarts)
    }
}

impl Drop for FmsHost {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Stop);
        drop(std::mem::replace(&mut self.tx, channel().0));
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

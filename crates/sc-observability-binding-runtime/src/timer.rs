//! One fallible process timer, with physically removable observer entries.
use crate::{
    error,
    sync::{Signal, lock},
};
use sc_observability_dto::Failure;
use std::cmp::Ordering as CmpOrdering;
use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak};
use std::time::Instant;

// Resource-only singleton prescribed by the binding contract; no logger/policy.
static TIMER: OnceLock<Mutex<Option<Arc<TimerService>>>> = OnceLock::new();

pub(crate) struct TimerService {
    // MUTEX: heap insertion, physical cancellation and timer sleep are one short
    // atomic update; notification callbacks execute after releasing the lock.
    heap: Mutex<BinaryHeap<Entry>>,
    changed: Condvar,
    next_id: AtomicU64,
}
struct Entry {
    at: Instant,
    id: u64,
    signal: Weak<Signal>,
}
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        (self.at, self.id) == (other.at, other.id)
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}
impl Ord for Entry {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        (other.at, other.id).cmp(&(self.at, self.id))
    }
}

pub(crate) fn shared() -> Result<Arc<TimerService>, Failure> {
    let mut cell = TIMER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| error::internal("timer initialization state poisoned"))?;
    if let Some(timer) = &*cell {
        return Ok(timer.clone());
    }
    let timer = Arc::new(TimerService {
        heap: Mutex::new(BinaryHeap::new()),
        changed: Condvar::new(),
        next_id: AtomicU64::new(1),
    });
    let worker = timer.clone();
    crate::spawn::spawn("binding-timer", move || worker.run())
        .map_err(|e| error::start_failed(e.to_string()))?;
    *cell = Some(timer.clone());
    Ok(timer)
}
impl TimerService {
    pub(crate) fn register(&self, at: Instant, signal: &Arc<Signal>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        lock(&self.heap).push(Entry {
            at,
            id,
            signal: Arc::downgrade(signal),
        });
        self.changed.notify_one();
        id
    }
    pub(crate) fn remove(&self, id: u64) {
        lock(&self.heap).retain(|entry| entry.id != id);
        self.changed.notify_one();
    }
    fn run(&self) {
        let mut heap = lock(&self.heap);
        loop {
            if let Some(entry) = heap.peek() {
                let now = Instant::now();
                if entry.at <= now {
                    let signal = heap.pop().and_then(|entry| entry.signal.upgrade());
                    drop(heap);
                    if let Some(signal) = signal {
                        signal.notify();
                    }
                    heap = lock(&self.heap);
                } else {
                    let delay = entry.at - now;
                    heap = self
                        .changed
                        .wait_timeout(heap, delay)
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .0;
                }
            } else {
                heap = self
                    .changed
                    .wait(heap)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        }
    }
    #[cfg(test)]
    pub(crate) fn entries(&self) -> usize {
        lock(&self.heap).len()
    }
}

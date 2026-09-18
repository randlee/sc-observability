//! Internal locks protect bounded bookkeeping, never foreign callbacks or I/O.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::task::Waker;

// Bookkeeping contains no user code. Recovery keeps cancellation/teardown viable
// if an unexpected worker unwind crossed a bookkeeping update.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
#[derive(Default)]
pub(crate) struct Signal {
    pub(crate) notified: AtomicBool,
    // MUTEX: short notification critical section prevents lost synchronous wakes.
    pub(crate) gate: Mutex<()>,
    pub(crate) changed: Condvar,
    // MUTEX: a Future replaces its executor waker; never invoke it under this lock.
    pub(crate) waker: Mutex<Option<Waker>>,
}
impl Signal {
    pub(crate) fn notify(&self) {
        {
            let _gate = lock(&self.gate);
            self.notified.store(true, Ordering::SeqCst);
            self.changed.notify_all();
        }
        let waker = lock(&self.waker).take();
        if let Some(waker) = waker {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| waker.wake()));
        }
    }
}

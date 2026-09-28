//! Bounded callback reservations outlive native operation slots.
use crate::{error, sync::lock};
use sc_observability_types::v2::SubscriberError;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};

pub(crate) struct Dispatcher {
    // MUTEX: queue operations and the completion worker sleep are coordinated;
    // no callback executes while the queue or callback-payload lock is held.
    queue: Mutex<VecDeque<Arc<Job>>>,
    changed: Condvar,
    reserved: AtomicUsize,
    closed: AtomicBool,
    next: AtomicU64,
    pub(crate) panics: AtomicU64,
}
struct BackendPermit(Arc<Dispatcher>);
impl Drop for BackendPermit {
    fn drop(&mut self) {
        let _queue = lock(&self.0.queue);
        self.0.reserved.fetch_sub(1, Ordering::SeqCst);
        self.0.changed.notify_all();
    }
}
pub(crate) struct ObserverPermit(pub(crate) Arc<AtomicUsize>);
impl Drop for ObserverPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
struct Payload {
    callback: Box<dyn FnOnce() + Send>,
    _backend: BackendPermit,
    _observer: ObserverPermit,
}
pub(crate) struct Job {
    id: u64,
    dispatcher: Weak<Dispatcher>,
    // MUTEX: cancellation and claiming move this one FnOnce payload exactly once.
    payload: Mutex<Option<Payload>>,
}
impl Dispatcher {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            queue: Mutex::new(VecDeque::with_capacity(128)),
            changed: Condvar::new(),
            reserved: AtomicUsize::new(0),
            closed: AtomicBool::new(false),
            next: AtomicU64::new(1),
            panics: AtomicU64::new(0),
        })
    }
    pub(crate) fn reserve(
        self: &Arc<Self>,
        observer: ObserverPermit,
        callback: Box<dyn FnOnce() + Send>,
    ) -> Result<Arc<Job>, SubscriberError> {
        let _queue = lock(&self.queue);
        if self.closed.load(Ordering::SeqCst) {
            return Err(error::subscriber(
                sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_CLOSED,
                "callback registration is closed",
            ));
        }
        self.reserved
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                (count < 128).then_some(count + 1)
            })
            .map_err(|_| {
                error::subscriber(
                    sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_WAITERS_FULL,
                    "callback registration capacity is occupied",
                )
            })?;
        Ok(Arc::new(Job {
            id: self.next.fetch_add(1, Ordering::SeqCst),
            dispatcher: Arc::downgrade(self),
            payload: Mutex::new(Some(Payload {
                callback,
                _backend: BackendPermit(self.clone()),
                _observer: observer,
            })),
        }))
    }
    pub(crate) fn close(&self) {
        let _queue = lock(&self.queue);
        self.closed.store(true, Ordering::SeqCst);
        self.changed.notify_all();
    }
    pub(crate) fn run(&self) {
        loop {
            let job = {
                let mut queue = lock(&self.queue);
                while queue.is_empty() {
                    if self.closed.load(Ordering::SeqCst)
                        && self.reserved.load(Ordering::SeqCst) == 0
                    {
                        return;
                    }
                    queue = self
                        .changed
                        .wait(queue)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
                queue.pop_front()
            };
            if let Some(job) = job {
                let payload = lock(&job.payload).take();
                if let Some(Payload {
                    callback,
                    _backend,
                    _observer,
                }) = payload
                    && std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback)).is_err()
                {
                    let _ = self
                        .panics
                        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                            Some(n.saturating_add(1))
                        });
                }
            }
        }
    }
    #[cfg(test)]
    pub(crate) fn reserved(&self) -> usize {
        self.reserved.load(Ordering::SeqCst)
    }
}
impl Job {
    pub(crate) fn schedule(self: &Arc<Self>) {
        if let Some(dispatcher) = self.dispatcher.upgrade() {
            let mut queue = lock(&dispatcher.queue);
            if lock(&self.payload).is_some() {
                queue.push_back(self.clone());
                dispatcher.changed.notify_one();
            }
        }
    }
    pub(crate) fn cancel(&self) {
        // Match queue->payload ordering with schedule; drop permits outside queue.
        let payload = if let Some(dispatcher) = self.dispatcher.upgrade() {
            let mut queue = lock(&dispatcher.queue);
            queue.retain(|job| job.id != self.id);
            lock(&self.payload).take()
        } else {
            lock(&self.payload).take()
        };
        drop(payload);
    }
}

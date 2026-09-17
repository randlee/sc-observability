//! Saved completion is independent of observer and callback ownership.
use crate::{
    callback::{Dispatcher, Job, ObserverPermit},
    error,
    sync::{Signal, lock},
    timer::TimerService,
};
use sc_observability_dto::Failure;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

/// Immutable observation of one native operation.
#[derive(Debug, Clone, PartialEq)]
pub enum OperationState<T> {
    Pending,
    Completed { result: Result<T, Failure> },
}
struct Published<T> {
    at: Instant,
    result: Result<T, Failure>,
}
enum Observer {
    Wait(Weak<Signal>),
    Callback(Arc<Job>),
}
struct Inner<T> {
    published: Arc<OnceLock<Published<T>>>,
    // MUTEX: registration and completion exchange bounded observer entries only.
    observers: Mutex<BTreeMap<u64, Observer>>,
    count: Arc<AtomicUsize>,
    next: AtomicU64,
    dispatcher: Weak<Dispatcher>,
    timer: Arc<TimerService>,
}
/// Caller-owned saved result; completed values do not own backend helpers.
pub struct Operation<T> {
    inner: Arc<Inner<T>>,
}
impl<T> Clone for Operation<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
impl<T: Clone + Send + Sync + 'static> std::fmt::Debug for Operation<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Operation")
            .field("completed", &self.inner.published.get().is_some())
            .finish_non_exhaustive()
    }
}
/// Cancels an unclaimed callback registration when dropped.
pub struct CompletionSubscription {
    cancel: Option<Box<dyn FnOnce() + Send + Sync>>,
}
impl std::fmt::Debug for CompletionSubscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompletionSubscription")
            .finish_non_exhaustive()
    }
}
impl Drop for CompletionSubscription {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel();
        }
    }
}

impl<T: Clone + Send + Sync + 'static> Operation<T> {
    pub(crate) fn new(dispatcher: &Arc<Dispatcher>, timer: &Arc<TimerService>) -> Self {
        Self {
            inner: Arc::new(Inner {
                published: Arc::new(OnceLock::new()),
                observers: Mutex::new(BTreeMap::new()),
                count: Arc::new(AtomicUsize::new(0)),
                next: AtomicU64::new(1),
                dispatcher: Arc::downgrade(dispatcher),
                timer: timer.clone(),
            }),
        }
    }
    #[cfg(test)]
    pub(crate) fn observer_count(&self) -> usize {
        self.inner.count.load(Ordering::SeqCst)
    }
    /// Reads saved completion without a lock, waiter registration or blocking.
    pub fn state(&self) -> OperationState<T> {
        self.inner
            .published
            .get()
            .map_or(OperationState::Pending, |value| OperationState::Completed {
                result: value.result.clone(),
            })
    }
    fn observed(&self, deadline: Instant) -> Option<Result<T, Failure>> {
        if let Some(value) = self.inner.published.get() {
            // Already-completed operations remain inspectable even with zero wait.
            return Some(if value.at <= deadline {
                value.result.clone()
            } else {
                Err(error::timeout())
            });
        }
        (Instant::now() >= deadline).then(|| Err(error::timeout()))
    }
    fn permit(&self) -> Result<ObserverPermit, Failure> {
        self.inner
            .count
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                (n < 64).then_some(n + 1)
            })
            .map_err(|_| error::waiters_full())?;
        Ok(ObserverPermit(self.inner.count.clone()))
    }
    fn register(&self, deadline: Instant) -> Result<Registration<T>, Failure> {
        let permit = self.permit()?;
        let id = self.inner.next.fetch_add(1, Ordering::SeqCst);
        let signal = Arc::new(Signal::default());
        let mut observers = lock(&self.inner.observers);
        if self.inner.published.get().is_none() {
            observers.insert(id, Observer::Wait(Arc::downgrade(&signal)));
        } else {
            signal.notify();
        }
        let timer_id = self.inner.timer.register(deadline, &signal);
        drop(observers);
        Ok(Registration {
            operation: self.clone(),
            id,
            signal,
            timer_id,
            _permit: permit,
        })
    }
    /// Waits for this observer only; expiry never cancels native work.
    ///
    /// # Errors
    /// Invalid duration, observer saturation or the saved native failure.
    pub fn wait(&self, timeout: Duration) -> Result<T, Failure> {
        error::duration(timeout)?;
        let deadline = Instant::now() + timeout;
        if let Some(result) = self.observed(deadline) {
            return result;
        }
        let registration = self.register(deadline)?;
        let mut gate = lock(&registration.signal.gate);
        loop {
            if let Some(result) = self.observed(deadline) {
                return result;
            }
            gate = registration
                .signal
                .changed
                .wait_timeout(gate, deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }
    /// Returns a nonblocking Future whose absolute deadline begins now.
    pub fn completion(
        &self,
        timeout: Duration,
    ) -> impl Future<Output = Result<T, Failure>> + Send + 'static {
        let deadline = Instant::now() + timeout.min(Duration::from_secs(60));
        let registration = error::duration(timeout).and_then(|()| {
            if self.observed(deadline).is_some() {
                Ok(None)
            } else {
                self.register(deadline).map(Some)
            }
        });
        CompletionFuture {
            operation: self.clone(),
            deadline,
            registration,
        }
    }
    /// Registers one callback on the backend's fixed completion worker.
    ///
    /// # Errors
    /// Returns bounded observer/callback capacity or closed-backend failures.
    pub fn subscribe(
        &self,
        callback: Box<dyn FnOnce(Result<T, Failure>) + Send>,
    ) -> Result<CompletionSubscription, Failure> {
        let permit = self.permit()?;
        let dispatcher = self.inner.dispatcher.upgrade().ok_or_else(error::closed)?;
        let published = self.inner.published.clone();
        let job = dispatcher.reserve(
            permit,
            Box::new(move || {
                if let Some(value) = published.get() {
                    callback(value.result.clone());
                }
            }),
        )?;
        let id = self.inner.next.fetch_add(1, Ordering::SeqCst);
        {
            let mut observers = lock(&self.inner.observers);
            if self.inner.published.get().is_some() {
                job.schedule();
            } else {
                observers.insert(id, Observer::Callback(job.clone()));
            }
        }
        let inner = Arc::downgrade(&self.inner);
        Ok(CompletionSubscription {
            cancel: Some(Box::new(move || {
                if let Some(inner) = inner.upgrade() {
                    lock(&inner.observers).remove(&id);
                }
                job.cancel();
            })),
        })
    }
    pub(crate) fn complete(&self, result: Result<T, Failure>, release: impl FnOnce()) {
        let mut observers = lock(&self.inner.observers);
        if self
            .inner
            .published
            .set(Published {
                at: Instant::now(),
                result,
            })
            .is_err()
        {
            return;
        }
        release();
        let ready = std::mem::take(&mut *observers);
        drop(observers);
        for observer in ready.into_values() {
            match observer {
                Observer::Wait(signal) => {
                    if let Some(signal) = signal.upgrade() {
                        signal.notify();
                    }
                }
                Observer::Callback(job) => job.schedule(),
            }
        }
    }
}
struct Registration<T: Clone + Send + Sync + 'static> {
    operation: Operation<T>,
    id: u64,
    signal: Arc<Signal>,
    timer_id: u64,
    _permit: ObserverPermit,
}
impl<T: Clone + Send + Sync + 'static> Drop for Registration<T> {
    fn drop(&mut self) {
        lock(&self.operation.inner.observers).remove(&self.id);
        self.operation.inner.timer.remove(self.timer_id);
    }
}
struct CompletionFuture<T: Clone + Send + Sync + 'static> {
    operation: Operation<T>,
    deadline: Instant,
    registration: Result<Option<Registration<T>>, Failure>,
}
impl<T: Clone + Send + Sync + 'static> Future for CompletionFuture<T> {
    type Output = Result<T, Failure>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Err(error) = &this.registration {
            return Poll::Ready(Err(error.clone()));
        }
        if let Some(result) = this.operation.observed(this.deadline) {
            this.registration = Ok(None);
            return Poll::Ready(result);
        }
        if let Ok(Some(registration)) = &this.registration {
            *lock(&registration.signal.waker) = Some(cx.waker().clone());
        }
        // Publication/deadline may race waker installation; recheck afterward.
        if let Some(result) = this.operation.observed(this.deadline) {
            this.registration = Ok(None);
            Poll::Ready(result)
        } else {
            Poll::Pending
        }
    }
}

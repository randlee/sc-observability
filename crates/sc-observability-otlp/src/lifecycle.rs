//! Backend-neutral OTLP admission and lifecycle coordination.
//!
//! The core deliberately knows about neither an SDK runtime nor a wire
//! protocol.  Exporters provide the two asynchronous lifecycle futures while
//! this module owns the short admission critical section, ordered barriers,
//! bounded admission, and terminal accounting.
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::ValidatedTransportBounds;
use crate::constants::MAX_OTLP_RECORD_BYTES;
use crate::contracts::{ExporterSet, LifecycleFuture};
use crate::error_codes;
use sc_observability_types::error_codes::otlp::OTLP_WORKER_TERMINATED;
use sc_observability_types::v2::{ExportError, TelemetryError};
use sc_observability_types::{DiagnosticSummary, ErrorContext, Remediation};

/// Signal family used for per-signal dropped accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SignalKind {
    /// Log records.
    Logs,
    /// Completed spans.
    Traces,
    /// Metric records.
    Metrics,
}

impl SignalKind {
    const fn index(self) -> usize {
        match self {
            Self::Logs => 0,
            Self::Traces => 1,
            Self::Metrics => 2,
        }
    }
}

/// Snapshot of lifecycle state and fail-open accounting for health surfaces.
#[allow(
    dead_code,
    reason = "D.18 exposes lifecycle health through the public facade"
)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LifecycleHealth {
    /// Current lifecycle phase.
    pub(crate) phase: LifecycleState,
    /// Number of admitted records not yet terminal.
    pub(crate) admitted_records: usize,
    /// Aggregate bytes not yet terminal.
    pub(crate) admitted_bytes: usize,
    /// Dropped records by signal: logs, traces, metrics.
    pub(crate) dropped_by_signal: [u64; 3],
    /// Whether any admission or lifecycle operation degraded health.
    pub(crate) degraded: bool,
    /// Last diagnostic recorded by the lifecycle core.
    pub(crate) last_error: Option<DiagnosticSummary>,
}

/// Terminal state of the shared lifecycle machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifecycleState {
    /// Admission and lifecycle operations are open.
    Open,
    /// New admission is rejected while prior work drains.
    Closing,
    /// The provider has reached its terminal state.
    Shutdown,
}

/// A record admitted without copying or interpreting its payload.
pub(crate) struct Admitted<T> {
    value: Option<T>,
    permit: Option<AdmissionPermit>,
}

impl<T> Admitted<T> {
    /// Borrows the original payload for backend projection.
    pub(crate) fn get(&self) -> &T {
        self.value.as_ref().expect("admitted value is present")
    }

    /// Completes the admission and returns the original payload.
    pub(crate) fn complete(mut self, result: Result<(), ExportError>) -> T {
        if let Some(permit) = self.permit.take() {
            permit.finish(result);
        }
        self.value.take().expect("admitted value is present")
    }
}

struct AdmissionPermit {
    inner: Weak<LifecycleInner>,
    sequence: u64,
    signal: SignalKind,
    bytes: usize,
    finished: AtomicBool,
}

impl AdmissionPermit {
    fn finish(&self, result: Result<(), ExportError>) {
        if self.finished.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Some(inner) = self.inner.upgrade() {
            inner.finish_admission(self.sequence, self.signal, self.bytes, result);
        }
    }
}

impl Drop for AdmissionPermit {
    fn drop(&mut self) {
        self.finish(Err(terminal_drop_error()));
    }
}

struct AdmissionMeta {
    signal: SignalKind,
    bytes: usize,
}

struct CoreState {
    phase: LifecycleState,
    next_sequence: u64,
    admitted_records: usize,
    admitted_bytes: usize,
    active: BTreeMap<u64, AdmissionMeta>,
    dropped_by_signal: [u64; 3],
    degraded: bool,
    last_error: Option<DiagnosticSummary>,
    // Failures completed outside an active barrier belong to the next window.
    // Each window needs only one representative error, not a history of admissions.
    pending_failure: Option<ErrorSnapshot>,
    flush: Option<Arc<Operation>>,
    shutdown: Option<Arc<Operation>>,
    barrier_wakers: Vec<Waker>,
}

struct LifecycleInner {
    /// The terminal backend runs only after the core's admission barrier.
    ///
    /// Signal adapters deliberately are not retained here: they themselves
    /// hold this core in order to admit work. Retaining the full exporter set
    /// would create a lifecycle recursion during flush/shutdown.
    terminal_backend: Arc<dyn crate::contracts::ExporterLifecycle>,
    queue_capacity: usize,
    queue_byte_capacity: usize,
    flush_timeout: Duration,
    shutdown_timeout: Duration,
    state: Mutex<CoreState>,
    #[cfg(test)]
    barrier_registration_hook: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

/// Shared lifecycle core consumed by backend adapters and the future facade.
#[derive(Clone)]
pub(crate) struct LifecycleCore {
    inner: Arc<LifecycleInner>,
}

impl LifecycleCore {
    /// Constructs the state machine from D.21's validated, backend-neutral
    /// bounds.  Exporter preflight is intentionally performed before any
    /// state becomes visible to callers.
    #[allow(dead_code, reason = "D.18 constructs the staged lifecycle core")]
    pub(crate) fn new(
        exporters: ExporterSet,
        bounds: &ValidatedTransportBounds,
    ) -> Result<Self, ExportError> {
        Self::from_backend(exporters.lifecycle, bounds)
    }

    /// Constructs the admission/barrier core around the terminal backend.
    ///
    /// Backend adapters are intentionally constructed *after* this method
    /// returns and retain a clone of the resulting core. This preserves one
    /// canonical admission/accounting domain without requiring a second core
    /// or making a terminal lifecycle call recursively re-enter its own
    /// barrier.
    pub(crate) fn from_backend(
        terminal_backend: Arc<dyn crate::contracts::ExporterLifecycle>,
        bounds: &ValidatedTransportBounds,
    ) -> Result<Self, ExportError> {
        terminal_backend.blocking_preflight()?;
        Ok(Self {
            inner: Arc::new(LifecycleInner {
                terminal_backend,
                queue_capacity: bounds.queue_capacity().get(),
                queue_byte_capacity: bounds.queue_byte_capacity().get(),
                flush_timeout: bounds.lifecycle().flush().get(),
                shutdown_timeout: bounds.lifecycle().shutdown().get(),
                state: Mutex::new(CoreState {
                    phase: LifecycleState::Open,
                    next_sequence: 0,
                    admitted_records: 0,
                    admitted_bytes: 0,
                    active: BTreeMap::new(),
                    dropped_by_signal: [0; 3],
                    degraded: false,
                    last_error: None,
                    pending_failure: None,
                    flush: None,
                    shutdown: None,
                    barrier_wakers: Vec::new(),
                }),
                #[cfg(test)]
                barrier_registration_hook: Mutex::new(None),
            }),
        })
    }

    #[cfg(test)]
    pub(crate) fn set_barrier_registration_hook(&self, hook: impl Fn() + Send + Sync + 'static) {
        *self
            .inner
            .barrier_registration_hook
            .lock()
            .expect("barrier registration hook lock") = Some(Arc::new(hook));
    }

    /// Admits a payload while holding the admission lock only long enough to
    /// assign its sequence and reserve its record/byte budget.
    pub(crate) fn admit<T>(
        &self,
        signal: SignalKind,
        value: T,
        bytes: usize,
    ) -> Result<Admitted<T>, TelemetryError> {
        let mut state = self.inner.state.lock().expect("lifecycle state lock");
        if state.phase != LifecycleState::Open {
            LifecycleInner::record_drop(&mut state, signal, None);
            return Err(TelemetryError::Shutdown {
                context: Box::new(ErrorContext::new(
                    error_codes::OTLP_TELEMETRY_SHUTDOWN,
                    "telemetry runtime is shut down",
                    Remediation::recoverable("Construct a new telemetry instance", [] as [&str; 0]),
                )),
            });
        }
        if bytes > MAX_OTLP_RECORD_BYTES {
            let error = queue_full_error();
            LifecycleInner::record_drop(&mut state, signal, Some(&error));
            return Err(TelemetryError::ExportFailure(error));
        }
        if state.admitted_records >= self.inner.queue_capacity
            || state
                .admitted_bytes
                .checked_add(bytes)
                .is_none_or(|total| total > self.inner.queue_byte_capacity)
        {
            let error = queue_full_error();
            LifecycleInner::record_drop(&mut state, signal, Some(&error));
            return Err(TelemetryError::ExportFailure(error));
        }

        let sequence = state.next_sequence;
        state.next_sequence = state.next_sequence.saturating_add(1);
        state.admitted_records += 1;
        state.admitted_bytes += bytes;
        state
            .active
            .insert(sequence, AdmissionMeta { signal, bytes });
        drop(state);

        Ok(Admitted {
            value: Some(value),
            permit: Some(AdmissionPermit {
                inner: Arc::downgrade(&self.inner),
                sequence,
                signal,
                bytes,
                finished: AtomicBool::new(false),
            }),
        })
    }

    /// Starts or joins the ordered flush barrier.
    ///
    /// A new barrier consumes previously unreported admission failures. All
    /// waiters on that barrier receive the same result; later successful
    /// windows can succeed without erasing cumulative health diagnostics.
    pub(crate) fn flush_async(&self) -> LifecycleWaiter {
        let mut state = self.inner.state.lock().expect("lifecycle state lock");
        if let Some(operation) = state.flush.as_ref().filter(|op| !op.is_complete()) {
            return LifecycleWaiter::new(Arc::clone(operation));
        }
        let cutoff = state.next_sequence;
        let operation = Arc::new(Operation::new(
            Arc::clone(&self.inner),
            OperationKind::Flush,
            cutoff,
            self.inner.flush_timeout,
            None,
            state.pending_failure.take(),
        ));
        state.flush = Some(Arc::clone(&operation));
        LifecycleWaiter::new(operation)
    }

    /// Atomically closes admission and starts or joins the one shutdown.
    pub(crate) fn shutdown_async(&self) -> LifecycleWaiter {
        let mut state = self.inner.state.lock().expect("lifecycle state lock");
        if state.phase == LifecycleState::Shutdown {
            let operation = Arc::new(Operation::completed(Arc::clone(&self.inner), Ok(())));
            state.shutdown = Some(Arc::clone(&operation));
            return LifecycleWaiter::new(operation);
        }
        if let Some(operation) = state.shutdown.as_ref() {
            return LifecycleWaiter::new(Arc::clone(operation));
        }
        state.phase = LifecycleState::Closing;
        let cutoff = state.next_sequence;
        let precondition = state.flush.clone().filter(|op| !op.is_complete());
        let operation = Arc::new(Operation::new(
            Arc::clone(&self.inner),
            OperationKind::Shutdown,
            cutoff,
            self.inner.shutdown_timeout,
            precondition,
            state.pending_failure.take(),
        ));
        state.shutdown = Some(Arc::clone(&operation));
        LifecycleWaiter::new(operation)
    }

    /// Abandons all admitted work when the final backend handle is dropped.
    ///
    /// This is deliberately separate from explicit shutdown: dropping a
    /// handle cannot await the worker, but it must still close admission and
    /// publish the same terminal accounting to retained health/barrier
    /// observers. Admission permits that complete later see their sequence
    /// removed and therefore cannot count the record twice.
    pub(crate) fn abandon(&self) {
        let worker_error = worker_terminated_error();
        let mut state = self.inner.state.lock().expect("lifecycle state lock");
        if state.phase == LifecycleState::Shutdown && state.active.is_empty() {
            return;
        }
        state.phase = LifecycleState::Shutdown;
        let active = std::mem::take(&mut state.active);
        for (sequence, metadata) in active {
            state.admitted_records = state.admitted_records.saturating_sub(1);
            state.admitted_bytes = state.admitted_bytes.saturating_sub(metadata.bytes);
            LifecycleInner::record_drop(&mut state, metadata.signal, Some(&worker_error));
            let snapshot = ErrorSnapshot::from_error(&worker_error);
            let mut captured = false;
            for operation in [&state.flush, &state.shutdown].into_iter().flatten() {
                if sequence < operation.cutoff {
                    captured |= operation.record_failure(&snapshot);
                }
            }
            if !captured {
                state.pending_failure.get_or_insert(snapshot);
            }
        }
        let waiters = std::mem::take(&mut state.barrier_wakers);
        drop(state);
        for waiter in waiters {
            waiter.wake();
        }
    }

    /// Returns a point-in-time health/accounting snapshot.
    #[allow(
        dead_code,
        reason = "D.18 exposes lifecycle health through the public facade"
    )]
    pub(crate) fn health(&self) -> LifecycleHealth {
        let state = self.inner.state.lock().expect("lifecycle state lock");
        LifecycleHealth {
            phase: state.phase,
            admitted_records: state.admitted_records,
            admitted_bytes: state.admitted_bytes,
            dropped_by_signal: state.dropped_by_signal,
            degraded: state.degraded,
            last_error: state.last_error.clone(),
        }
    }
}

impl LifecycleInner {
    fn record_drop(state: &mut CoreState, signal: SignalKind, error: Option<&ExportError>) {
        state.dropped_by_signal[signal.index()] += 1;
        state.degraded = true;
        if let Some(error) = error {
            state.last_error = Some(DiagnosticSummary::from(error.diagnostic()));
        }
    }

    fn finish_admission(
        &self,
        sequence: u64,
        signal: SignalKind,
        bytes: usize,
        result: Result<(), ExportError>,
    ) {
        let mut state = self.state.lock().expect("lifecycle state lock");
        if state.active.remove(&sequence).is_none() {
            return;
        }
        state.admitted_records = state.admitted_records.saturating_sub(1);
        state.admitted_bytes = state.admitted_bytes.saturating_sub(bytes);
        if let Err(error) = result {
            Self::record_drop(&mut state, signal, Some(&error));
            let snapshot = ErrorSnapshot::from_error(&error);
            // A shutdown can overlap the current flush; both must retain an
            // in-scope failure. An out-of-scope completion cannot overwrite it.
            let mut captured = false;
            for operation in [&state.flush, &state.shutdown].into_iter().flatten() {
                if sequence < operation.cutoff {
                    captured |= operation.record_failure(&snapshot);
                }
            }
            if !captured {
                state.pending_failure.get_or_insert(snapshot);
            }
        }
        let waiters = std::mem::take(&mut state.barrier_wakers);
        drop(state);
        for waiter in waiters {
            waiter.wake();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationKind {
    Flush,
    Shutdown,
}

struct Operation {
    inner: Arc<LifecycleInner>,
    kind: OperationKind,
    // Exclusive admission bound: an empty window must exclude sequence zero.
    cutoff: u64,
    deadline: Instant,
    precondition: Option<Arc<Operation>>,
    state: Mutex<OperationState>,
    // Shared by admission completion and all waiters, retained across backend polls.
    failure: Mutex<Option<ErrorSnapshot>>,
    waiters: Mutex<Vec<Waker>>,
    timer_started: AtomicBool,
    timer_signal: Arc<TimerSignal>,
    polling: AtomicBool,
}

struct TimerSignal {
    completed: Mutex<bool>,
    wake: Condvar,
}

enum OperationState {
    Pending,
    Running(LifecycleFuture),
    Complete(Arc<Completion>),
}

struct Completion {
    snapshot: Option<ErrorSnapshot>,
    // The first observer receives the original typed error (including its
    // source and construction backtrace); later observers reconstruct from
    // the stable diagnostic snapshot because ExportError is not Clone.
    original: Mutex<Option<ExportError>>,
}

/// Future returned by the internal flush/shutdown operations.
pub(crate) struct LifecycleWaiter {
    operation: Arc<Operation>,
}

impl LifecycleWaiter {
    fn new(operation: Arc<Operation>) -> Self {
        Self { operation }
    }
}

impl Future for LifecycleWaiter {
    type Output = Result<(), ExportError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        self.operation.poll(context)
    }
}

impl Operation {
    fn new(
        inner: Arc<LifecycleInner>,
        kind: OperationKind,
        cutoff: u64,
        timeout: Duration,
        precondition: Option<Arc<Operation>>,
        failure: Option<ErrorSnapshot>,
    ) -> Self {
        Self {
            inner,
            kind,
            cutoff,
            deadline: Instant::now() + timeout,
            precondition,
            state: Mutex::new(OperationState::Pending),
            failure: Mutex::new(failure),
            waiters: Mutex::new(Vec::new()),
            timer_started: AtomicBool::new(false),
            timer_signal: Arc::new(TimerSignal {
                completed: Mutex::new(false),
                wake: Condvar::new(),
            }),
            polling: AtomicBool::new(false),
        }
    }

    fn completed(inner: Arc<LifecycleInner>, result: Result<(), ExportError>) -> Self {
        let completion = Completion::from_result(result);
        Self {
            inner,
            kind: OperationKind::Shutdown,
            cutoff: 0,
            deadline: Instant::now(),
            precondition: None,
            state: Mutex::new(OperationState::Complete(Arc::new(completion))),
            failure: Mutex::new(None),
            waiters: Mutex::new(Vec::new()),
            timer_started: AtomicBool::new(true),
            timer_signal: Arc::new(TimerSignal {
                completed: Mutex::new(true),
                wake: Condvar::new(),
            }),
            polling: AtomicBool::new(false),
        }
    }

    /// Records an error only while the operation can still report it.
    ///
    /// Lock order is core state, operation state, then failure. No operation
    /// state guard is held while acquiring the core lock in `finish`.
    fn record_failure(&self, error: &ErrorSnapshot) -> bool {
        let state = self.state.lock().expect("operation state lock");
        if matches!(*state, OperationState::Complete(_)) {
            return false;
        }
        self.failure
            .lock()
            .expect("operation failure lock")
            .get_or_insert_with(|| error.clone());
        true
    }

    fn is_complete(&self) -> bool {
        matches!(
            *self.state.lock().expect("operation state lock"),
            OperationState::Complete(_)
        )
    }

    fn poll(self: &Arc<Self>, context: &mut Context<'_>) -> Poll<Result<(), ExportError>> {
        if self.polling.swap(true, Ordering::Acquire) {
            self.register_waiter(context.waker());
            if self.is_complete() {
                context.waker().wake_by_ref();
            }
            return Poll::Pending;
        }
        let result = self.poll_inner(context);
        self.polling.store(false, Ordering::Release);
        result
    }

    fn poll_inner(self: &Arc<Self>, context: &mut Context<'_>) -> Poll<Result<(), ExportError>> {
        self.start_timer(context.waker());

        if let Some(result) = self.completed_result() {
            return Poll::Ready(result);
        }
        if Instant::now() >= self.deadline {
            self.finish(Err(lifecycle_timeout_error()));
            return Poll::Ready(self.completed_result().expect("operation completed"));
        }

        if let Some(precondition) = &self.precondition {
            match precondition.poll(context) {
                Poll::Pending => {
                    self.register_waiter(context.waker());
                    return Poll::Pending;
                }
                Poll::Ready(Err(error)) => {
                    self.record_failure(&ErrorSnapshot::from_error(&error));
                }
                Poll::Ready(Ok(())) => {}
            }
        }

        let waiting_for_admissions = {
            let mut state = self.inner.state.lock().expect("lifecycle state lock");
            if state.active.range(..self.cutoff).next().is_some() {
                #[cfg(test)]
                if let Some(hook) = self
                    .inner
                    .barrier_registration_hook
                    .lock()
                    .expect("barrier registration hook lock")
                    .take()
                {
                    hook();
                }
                // Register while holding the same mutex used to observe active
                // admissions. Otherwise the last admission can complete after
                // the observation but before registration, leaving this barrier
                // asleep until its deadline.
                state.barrier_wakers.push(context.waker().clone());
                true
            } else {
                false
            }
        };
        if waiting_for_admissions {
            self.register_waiter(context.waker());
            return Poll::Pending;
        }

        let mut future = {
            let mut operation_state = self.state.lock().expect("operation state lock");
            match std::mem::replace(&mut *operation_state, OperationState::Pending) {
                OperationState::Pending => {
                    let future = match self.kind {
                        OperationKind::Flush => self.inner.terminal_backend.flush_async(),
                        OperationKind::Shutdown => self.inner.terminal_backend.shutdown_async(),
                    };
                    *operation_state = OperationState::Running(future);
                    match std::mem::replace(&mut *operation_state, OperationState::Pending) {
                        OperationState::Running(future) => future,
                        _ => unreachable!("operation just entered running state"),
                    }
                }
                OperationState::Running(future) => {
                    *operation_state = OperationState::Running(future);
                    match std::mem::replace(&mut *operation_state, OperationState::Pending) {
                        OperationState::Running(future) => future,
                        _ => unreachable!("running operation state preserved"),
                    }
                }
                OperationState::Complete(completion) => {
                    *operation_state = OperationState::Complete(completion);
                    return Poll::Ready(
                        self.completed_result().expect("completed operation result"),
                    );
                }
            }
        };

        match future.as_mut().poll(context) {
            Poll::Pending => {
                *self.state.lock().expect("operation state lock") = OperationState::Running(future);
                self.register_waiter(context.waker());
                Poll::Pending
            }
            Poll::Ready(result) => {
                let result = match result {
                    Ok(()) => self
                        .failure
                        .lock()
                        .expect("operation failure lock")
                        .as_ref()
                        .map_or(Ok(()), |error| Err(error.to_error())),
                    Err(error) => Err(error),
                };
                self.finish(result);
                Poll::Ready(self.completed_result().expect("operation completed"))
            }
        }
    }

    fn start_timer(self: &Arc<Self>, waker: &Waker) {
        if self.timer_started.swap(true, Ordering::AcqRel) {
            return;
        }
        let weak = Arc::downgrade(self);
        let deadline = self.deadline;
        let timer_waker = waker.clone();
        let timer_signal = Arc::clone(&self.timer_signal);
        thread::spawn(move || {
            let mut completed = timer_signal.completed.lock().expect("operation timer lock");
            while !*completed {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    drop(completed);
                    timer_waker.wake_by_ref();
                    if let Some(operation) = weak.upgrade() {
                        let waiters = std::mem::take(
                            &mut *operation.waiters.lock().expect("operation waiters lock"),
                        );
                        for waiter in waiters {
                            waiter.wake();
                        }
                    }
                    return;
                }
                let (next, timeout) = timer_signal
                    .wake
                    .wait_timeout(completed, remaining)
                    .expect("operation timer wait");
                completed = next;
                if timeout.timed_out() && !*completed {
                    drop(completed);
                    timer_waker.wake_by_ref();
                    if let Some(operation) = weak.upgrade() {
                        let waiters = std::mem::take(
                            &mut *operation.waiters.lock().expect("operation waiters lock"),
                        );
                        for waiter in waiters {
                            waiter.wake();
                        }
                    }
                    return;
                }
            }
        });
    }

    fn register_waiter(&self, waker: &Waker) {
        self.waiters
            .lock()
            .expect("operation waiters lock")
            .push(waker.clone());
    }

    fn completed_result(&self) -> Option<Result<(), ExportError>> {
        let state = self.state.lock().expect("operation state lock");
        let OperationState::Complete(completion) = &*state else {
            return None;
        };
        // This is intentionally a take-once accessor: preserving the original
        // error for one waiter retains its source/backtrace; all later waiters
        // receive a typed reconstruction from the shared diagnostic snapshot.
        Some(
            match completion.original.lock().expect("completion lock").take() {
                Some(error) => Err(error),
                None => completion
                    .snapshot
                    .as_ref()
                    .map_or(Ok(()), |snapshot| Err(snapshot.to_error())),
            },
        )
    }

    fn finish(&self, result: Result<(), ExportError>) {
        let completion = Arc::new(Completion::from_result(result));
        let mut state = self.state.lock().expect("operation state lock");
        if matches!(*state, OperationState::Complete(_)) {
            return;
        }
        *state = OperationState::Complete(Arc::clone(&completion));
        drop(state);
        *self
            .timer_signal
            .completed
            .lock()
            .expect("operation timer lock") = true;
        self.timer_signal.wake.notify_one();
        if self.kind == OperationKind::Shutdown {
            self.inner.state.lock().expect("lifecycle state lock").phase = LifecycleState::Shutdown;
        }
        if let Some(snapshot) = &completion.snapshot {
            let mut lifecycle_state = self.inner.state.lock().expect("lifecycle state lock");
            lifecycle_state.degraded = true;
            lifecycle_state.last_error = Some(DiagnosticSummary::from(&snapshot.diagnostic));
        }
        let waiters = std::mem::take(&mut *self.waiters.lock().expect("operation waiters lock"));
        for waiter in waiters {
            waiter.wake();
        }
    }
}

impl Completion {
    fn from_result(result: Result<(), ExportError>) -> Self {
        match result {
            Ok(()) => Self {
                snapshot: None,
                original: Mutex::new(None),
            },
            Err(error) => Self {
                snapshot: Some(ErrorSnapshot::from_error(&error)),
                original: Mutex::new(Some(error)),
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ErrorKind {
    Transport,
    BlockingBackendInAsyncContext,
    AsyncLifecycleRequired,
    RuntimeTerminated,
    LifecycleTimeout,
    QueueFull,
    WorkerTerminated,
    ShutdownCancelledRetry,
    RetryDeadlineExhausted,
    NonRetryableHttpStatus,
    RetryAttemptsExhausted,
    TerminalExportFailure,
}

#[derive(Clone)]
struct ErrorSnapshot {
    kind: ErrorKind,
    diagnostic: sc_observability_types::Diagnostic,
}

impl ErrorSnapshot {
    fn from_error(error: &ExportError) -> Self {
        let kind = match error {
            ExportError::Transport { .. } => ErrorKind::Transport,
            ExportError::BlockingBackendInAsyncContext { .. } => {
                ErrorKind::BlockingBackendInAsyncContext
            }
            ExportError::AsyncLifecycleRequired { .. } => ErrorKind::AsyncLifecycleRequired,
            ExportError::RuntimeTerminated { .. } => ErrorKind::RuntimeTerminated,
            ExportError::LifecycleTimeout { .. } => ErrorKind::LifecycleTimeout,
            ExportError::QueueFull { .. } => ErrorKind::QueueFull,
            ExportError::WorkerTerminated { .. } => ErrorKind::WorkerTerminated,
            ExportError::ShutdownCancelledRetry { .. } => ErrorKind::ShutdownCancelledRetry,
            ExportError::RetryDeadlineExhausted { .. } => ErrorKind::RetryDeadlineExhausted,
            ExportError::NonRetryableHttpStatus { .. } => ErrorKind::NonRetryableHttpStatus,
            ExportError::RetryAttemptsExhausted { .. } => ErrorKind::RetryAttemptsExhausted,
            _ => ErrorKind::TerminalExportFailure,
        };
        Self {
            kind,
            diagnostic: error.diagnostic().clone(),
        }
    }

    fn to_error(&self) -> ExportError {
        let context = Box::new(context_from_diagnostic(&self.diagnostic));
        match self.kind {
            ErrorKind::Transport => ExportError::Transport { context },
            ErrorKind::BlockingBackendInAsyncContext => {
                ExportError::BlockingBackendInAsyncContext { context }
            }
            ErrorKind::AsyncLifecycleRequired => ExportError::AsyncLifecycleRequired { context },
            ErrorKind::RuntimeTerminated => ExportError::RuntimeTerminated { context },
            ErrorKind::LifecycleTimeout => ExportError::LifecycleTimeout { context },
            ErrorKind::QueueFull => ExportError::QueueFull { context },
            ErrorKind::WorkerTerminated => ExportError::WorkerTerminated { context },
            ErrorKind::ShutdownCancelledRetry => ExportError::ShutdownCancelledRetry { context },
            ErrorKind::RetryDeadlineExhausted => ExportError::RetryDeadlineExhausted { context },
            ErrorKind::NonRetryableHttpStatus => ExportError::NonRetryableHttpStatus { context },
            ErrorKind::RetryAttemptsExhausted => ExportError::RetryAttemptsExhausted { context },
            ErrorKind::TerminalExportFailure => ExportError::TerminalExportFailure { context },
        }
    }
}

fn context_from_diagnostic(diagnostic: &sc_observability_types::Diagnostic) -> ErrorContext {
    let mut context = ErrorContext::new(
        diagnostic.code.clone(),
        diagnostic.message.clone(),
        diagnostic.remediation.clone(),
    );
    if let Some(cause) = &diagnostic.cause {
        context = context.cause(cause.clone());
    }
    if let Some(docs) = &diagnostic.docs {
        context = context.docs(docs.clone());
    }
    for (key, value) in &diagnostic.details {
        context = context.detail(key.clone(), value.clone());
    }
    context
}

fn queue_full_error() -> ExportError {
    ExportError::QueueFull {
        context: Box::new(ErrorContext::new(
            error_codes::OTLP_QUEUE_FULL,
            "OTLP admission queue is full",
            Remediation::recoverable(
                "allow the exporter to drain before retrying admission",
                ["inspect telemetry health and dropped-record accounting"],
            ),
        )),
    }
}

fn terminal_drop_error() -> ExportError {
    ExportError::TerminalExportFailure {
        context: Box::new(ErrorContext::new(
            error_codes::OTLP_EXPORT_TERMINAL,
            "admitted telemetry reached a terminal outcome without completion",
            Remediation::recoverable(
                "inspect telemetry health and exporter terminal state",
                std::iter::empty::<String>(),
            ),
        )),
    }
}

fn worker_terminated_error() -> ExportError {
    ExportError::WorkerTerminated {
        context: Box::new(ErrorContext::new(
            OTLP_WORKER_TERMINATED,
            "OTLP worker terminated while abandoning admitted telemetry",
            Remediation::recoverable(
                "inspect telemetry health and construct a new exporter",
                std::iter::empty::<String>(),
            ),
        )),
    }
}

fn lifecycle_timeout_error() -> ExportError {
    ExportError::LifecycleTimeout {
        context: Box::new(ErrorContext::new(
            error_codes::OTLP_LIFECYCLE_TIMEOUT,
            "OTLP lifecycle operation exceeded its monotonic deadline",
            Remediation::recoverable(
                "inspect exporter health and keep the runtime alive through lifecycle completion",
                std::iter::empty::<String>(),
            ),
        )),
    }
}

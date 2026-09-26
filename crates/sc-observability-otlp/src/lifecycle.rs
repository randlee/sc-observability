//! Backend-neutral OTLP admission and lifecycle coordination.
//!
//! The core deliberately knows about neither an SDK runtime nor a wire
//! protocol.  Exporters provide the two asynchronous lifecycle futures while
//! this module owns the short admission critical section, ordered barriers,
//! bounded admission, and terminal accounting.
#![allow(
    dead_code,
    reason = "D.18 integrates this staged lifecycle core into the public facade"
)]

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::ValidatedTransportBounds;
use crate::contracts::{ExporterSet, LifecycleFuture};
use crate::error_codes;
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
    _signal: SignalKind,
    _bytes: usize,
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
    flush: Option<Arc<Operation>>,
    shutdown: Option<Arc<Operation>>,
    barrier_wakers: Vec<Waker>,
}

struct LifecycleInner {
    exporters: ExporterSet,
    queue_capacity: usize,
    queue_byte_capacity: usize,
    flush_timeout: Duration,
    shutdown_timeout: Duration,
    state: Mutex<CoreState>,
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
    pub(crate) fn new(
        exporters: ExporterSet,
        bounds: &ValidatedTransportBounds,
    ) -> Result<Self, ExportError> {
        exporters.lifecycle.blocking_preflight()?;
        Ok(Self {
            inner: Arc::new(LifecycleInner {
                exporters,
                queue_capacity: bounds.queue_capacity,
                queue_byte_capacity: bounds.queue_byte_capacity,
                flush_timeout: bounds.lifecycle_flush_timeout,
                shutdown_timeout: bounds.lifecycle_shutdown_timeout,
                state: Mutex::new(CoreState {
                    phase: LifecycleState::Open,
                    next_sequence: 0,
                    admitted_records: 0,
                    admitted_bytes: 0,
                    active: BTreeMap::new(),
                    dropped_by_signal: [0; 3],
                    degraded: false,
                    last_error: None,
                    flush: None,
                    shutdown: None,
                    barrier_wakers: Vec::new(),
                }),
            }),
        })
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
            self.inner.record_drop(&mut state, signal, None);
            return Err(TelemetryError::Shutdown);
        }
        if state.admitted_records >= self.inner.queue_capacity
            || state
                .admitted_bytes
                .checked_add(bytes)
                .is_none_or(|total| total > self.inner.queue_byte_capacity)
        {
            let error = queue_full_error();
            self.inner.record_drop(&mut state, signal, Some(&error));
            return Err(TelemetryError::ExportFailure(error));
        }

        let sequence = state.next_sequence;
        state.next_sequence = state.next_sequence.saturating_add(1);
        state.admitted_records += 1;
        state.admitted_bytes += bytes;
        state.active.insert(
            sequence,
            AdmissionMeta {
                _signal: signal,
                _bytes: bytes,
            },
        );
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
    pub(crate) fn flush_async(&self) -> LifecycleWaiter {
        let mut state = self.inner.state.lock().expect("lifecycle state lock");
        if let Some(operation) = state.flush.as_ref().filter(|op| !op.is_complete()) {
            return LifecycleWaiter::new(Arc::clone(operation));
        }
        let cutoff = state.next_sequence.saturating_sub(1);
        let operation = Arc::new(Operation::new(
            Arc::clone(&self.inner),
            OperationKind::Flush,
            cutoff,
            self.inner.flush_timeout,
            None,
        ));
        state.flush = Some(Arc::clone(&operation));
        LifecycleWaiter::new(operation)
    }

    /// Atomically closes admission and starts or joins the one shutdown.
    pub(crate) fn shutdown_async(&self) -> LifecycleWaiter {
        let mut state = self.inner.state.lock().expect("lifecycle state lock");
        if let Some(operation) = state.shutdown.as_ref() {
            return LifecycleWaiter::new(Arc::clone(operation));
        }
        if state.phase == LifecycleState::Shutdown {
            let operation = Arc::new(Operation::completed(Arc::clone(&self.inner), Ok(())));
            state.shutdown = Some(Arc::clone(&operation));
            return LifecycleWaiter::new(operation);
        }
        state.phase = LifecycleState::Closing;
        let cutoff = state.next_sequence.saturating_sub(1);
        let precondition = state.flush.clone().filter(|op| !op.is_complete());
        let operation = Arc::new(Operation::new(
            Arc::clone(&self.inner),
            OperationKind::Shutdown,
            cutoff,
            self.inner.shutdown_timeout,
            precondition,
        ));
        state.shutdown = Some(Arc::clone(&operation));
        LifecycleWaiter::new(operation)
    }

    /// Returns a point-in-time health/accounting snapshot.
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
    fn record_drop(&self, state: &mut CoreState, signal: SignalKind, error: Option<&ExportError>) {
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
            self.record_drop(&mut state, signal, Some(&error));
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
    cutoff: u64,
    deadline: Instant,
    precondition: Option<Arc<Operation>>,
    state: Mutex<OperationState>,
    waiters: Mutex<Vec<Waker>>,
    timer_started: AtomicBool,
}

enum OperationState {
    Pending,
    Running(LifecycleFuture),
    Complete(Arc<Completion>),
}

struct Completion {
    snapshot: Option<ErrorSnapshot>,
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
    ) -> Self {
        Self {
            inner,
            kind,
            cutoff,
            deadline: Instant::now() + timeout,
            precondition,
            state: Mutex::new(OperationState::Pending),
            waiters: Mutex::new(Vec::new()),
            timer_started: AtomicBool::new(false),
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
            waiters: Mutex::new(Vec::new()),
            timer_started: AtomicBool::new(true),
        }
    }

    fn is_complete(&self) -> bool {
        matches!(
            *self.state.lock().expect("operation state lock"),
            OperationState::Complete(_)
        )
    }

    fn poll(self: &Arc<Self>, context: &mut Context<'_>) -> Poll<Result<(), ExportError>> {
        self.start_timer(context.waker());

        if let Some(result) = self.completed_result() {
            return Poll::Ready(result);
        }
        if Instant::now() >= self.deadline {
            self.finish(Err(lifecycle_timeout_error()));
            return Poll::Ready(self.completed_result().expect("operation completed"));
        }

        if let Some(precondition) = &self.precondition {
            if precondition.poll(context).is_pending() {
                self.register_waiter(context.waker());
                return Poll::Pending;
            }
        }

        {
            let state = self.inner.state.lock().expect("lifecycle state lock");
            if state.active.range(..=self.cutoff).next().is_some() {
                drop(state);
                self.inner
                    .state
                    .lock()
                    .expect("lifecycle state lock")
                    .barrier_wakers
                    .push(context.waker().clone());
                self.register_waiter(context.waker());
                return Poll::Pending;
            }
        }

        let mut future = {
            let mut operation_state = self.state.lock().expect("operation state lock");
            match std::mem::replace(&mut *operation_state, OperationState::Pending) {
                OperationState::Pending => {
                    let future = match self.kind {
                        OperationKind::Flush => self.inner.exporters.lifecycle.flush_async(),
                        OperationKind::Shutdown => self.inner.exporters.lifecycle.shutdown_async(),
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
        thread::spawn(move || {
            let now = Instant::now();
            if deadline > now {
                thread::sleep(deadline - now);
            }
            timer_waker.wake_by_ref();
            if let Some(operation) = weak.upgrade() {
                let waiters =
                    std::mem::take(&mut *operation.waiters.lock().expect("operation waiters lock"));
                for waiter in waiters {
                    waiter.wake();
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
        if self.kind == OperationKind::Shutdown {
            self.inner.state.lock().expect("lifecycle state lock").phase = LifecycleState::Shutdown;
        }
        if let Some(error) = completion
            .original
            .lock()
            .expect("completion lock")
            .as_ref()
        {
            let mut lifecycle_state = self.inner.state.lock().expect("lifecycle state lock");
            lifecycle_state.degraded = true;
            lifecycle_state.last_error = Some(DiagnosticSummary::from(error.diagnostic()));
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
            ExportError::TerminalExportFailure { .. } => ErrorKind::TerminalExportFailure,
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

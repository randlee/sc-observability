//! Process-global logger slot, drop accounting, the guarded submission core, and
//! the bounded flush / shutdown helpers.
//!
//! Every lock acquisition recovers from poisoning with `PoisonError::into_inner`:
//! the slot holds only an `Option<Arc<Installed>>`, which has no invariant a
//! panic can break.

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, TryRecvError};
use std::sync::{Arc, Condvar, Mutex, OnceLock, PoisonError, RwLock};
use std::time::{Duration, Instant};

use crate::__private::EventParts;
use crate::health::BridgeLifecycle;
use crate::{
    DropCause, DroppedEvents, FlushError, ShutdownError, ShutdownOutcome, ShutdownReport,
    UnconfirmedShutdown, health,
};
use sc_observability::TryLogError;
use sc_observability_types::DiagnosticInfo;

/// A rejected submission: every failure of the guarded core maps to exactly one [`DropCause`].
///
/// Implemented by `DropCause` (the facade and the macros, which discard the
/// result) and by [`crate::EmitError`] (`LogControl::try_log`, which returns it).
pub(crate) trait Rejection: Sized {
    /// The single counter this rejection increments.
    fn drop_cause(&self) -> DropCause;
    /// The rejection for a call made while this thread is already inside the guard.
    fn reentrant() -> Self;
    /// The rejection for a panic caught inside the guard.
    fn panicked() -> Self;
}

impl Rejection for DropCause {
    fn drop_cause(&self) -> DropCause {
        *self
    }

    fn reentrant() -> Self {
        DropCause::ReentrantEmit
    }

    fn panicked() -> Self {
        DropCause::LoggerPanicked
    }
}

impl Rejection for crate::EmitError {
    fn drop_cause(&self) -> DropCause {
        match self {
            Self::InvalidField { .. } | Self::InvalidEvent { .. } => DropCause::InvalidEvent,
            Self::QueueFull { .. } => DropCause::QueueFull,
            Self::WriterDegraded { .. } => DropCause::WriterDegraded,
            Self::ShutdownTimedOut { .. } => DropCause::ShutdownTimedOut,
            Self::NotRunning { .. } => DropCause::NotInstalled,
            Self::Reentrant => DropCause::ReentrantEmit,
            Self::Panicked => DropCause::LoggerPanicked,
        }
    }

    fn reentrant() -> Self {
        Self::Reentrant
    }
    fn panicked() -> Self {
        Self::Panicked
    }
}

/// Everything the emit path needs, shared behind one `Arc`.
pub(crate) struct Installed {
    pub(crate) logger: sc_observability::Logger,
    pub(crate) service: sc_observability_types::ServiceName,
    pub(crate) identity: sc_observability_types::ProcessIdentity,
    pub(crate) options: crate::BridgeOptions,
}

pub(crate) static SLOT: RwLock<Option<Arc<Installed>>> = RwLock::new(None);
pub(crate) static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Encoded [`BridgeLifecycle`]; `Stopped` until `init` succeeds (no guard exists before).
static LIFECYCLE: AtomicU8 = AtomicU8::new(LIFECYCLE_STOPPED);
const LIFECYCLE_RUNNING: u8 = 0;
const LIFECYCLE_SHUTTING_DOWN: u8 = 1;
const LIFECYCLE_STOPPED: u8 = 2;
const LIFECYCLE_FAILED: u8 = 3;

/// Publishes a lifecycle transition; read lock-free by health snapshots.
pub(crate) fn set_lifecycle(lifecycle: BridgeLifecycle) {
    let encoded = match lifecycle {
        BridgeLifecycle::Running => LIFECYCLE_RUNNING,
        BridgeLifecycle::ShuttingDown => LIFECYCLE_SHUTTING_DOWN,
        BridgeLifecycle::Failed => LIFECYCLE_FAILED,
        BridgeLifecycle::Stopped => LIFECYCLE_STOPPED,
    };
    LIFECYCLE.store(encoded, Ordering::SeqCst);
}

/// Current lifecycle phase.
pub(crate) fn lifecycle() -> BridgeLifecycle {
    match LIFECYCLE.load(Ordering::SeqCst) {
        LIFECYCLE_RUNNING => BridgeLifecycle::Running,
        LIFECYCLE_SHUTTING_DOWN => BridgeLifecycle::ShuttingDown,
        LIFECYCLE_FAILED => BridgeLifecycle::Failed,
        _ => BridgeLifecycle::Stopped,
    }
}

/// Target lifecycle projection shared by the owner and non-owning controls.
pub(crate) fn lifecycle_phase() -> crate::LifecyclePhase {
    match lifecycle() {
        BridgeLifecycle::Running => crate::LifecyclePhase::Running,
        BridgeLifecycle::ShuttingDown => crate::LifecyclePhase::Stopping,
        BridgeLifecycle::Stopped => crate::LifecyclePhase::Stopped,
        BridgeLifecycle::Failed => crate::LifecyclePhase::Failed,
    }
}

/// Completion state belongs to one bridge-wide coordinator, independent of the
/// owner and every control. A timeout ends only one caller's wait, never this
/// operation.
enum ShutdownState {
    NotStarted,
    InProgress,
    Complete(Box<Result<ShutdownReport, crate::WaitError>>),
}

struct ShutdownCoordinator {
    state: Mutex<ShutdownState>,
    complete: Condvar,
    work: mpsc::Sender<ShutdownCommand>,
}

static SHUTDOWN_COORDINATOR: OnceLock<ShutdownCoordinator> = OnceLock::new();

#[cfg(feature = "test_hooks")]
static FAIL_NEXT_COORDINATOR_RESERVATION: AtomicBool = AtomicBool::new(false);

type ShutdownCommand = Box<dyn FnOnce() + Send + 'static>;

#[cfg(feature = "test_hooks")]
struct SaveShutdownHook {
    entered: mpsc::SyncSender<()>,
    release: mpsc::Receiver<()>,
}

#[cfg(feature = "test_hooks")]
static SAVE_SHUTDOWN_HOOK: OnceLock<Mutex<Option<SaveShutdownHook>>> = OnceLock::new();

#[cfg(feature = "test_hooks")]
static WAIT_STOPPED_HOOK: OnceLock<Mutex<Option<mpsc::SyncSender<()>>>> = OnceLock::new();

#[cfg(test)]
static SHUTDOWN_WORK_HOOK: OnceLock<Mutex<Option<ShutdownCommand>>> = OnceLock::new();

#[cfg(test)]
fn run_shutdown_work_hook() {
    let hook = SHUTDOWN_WORK_HOOK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    if let Some(hook) = hook {
        hook();
    }
}

/// Reserves the sole shutdown worker before the global facade is installed.
/// A failed reservation is therefore an initialization failure rather than a
/// partially usable bridge that discovers it cannot complete its lifecycle.
pub(crate) fn reserve_shutdown_coordinator() -> Result<(), std::io::Error> {
    #[cfg(feature = "test_hooks")]
    if FAIL_NEXT_COORDINATOR_RESERVATION.swap(false, Ordering::SeqCst) {
        return Err(std::io::Error::other(
            "injected coordinator reservation failure",
        ));
    }
    if SHUTDOWN_COORDINATOR.get().is_some() {
        return Ok(());
    }
    let (work, receiver) = mpsc::channel::<ShutdownCommand>();
    std::thread::Builder::new()
        .name("sc-observability-log-shutdown".to_owned())
        .spawn(move || {
            // Exactly one lifecycle owner exists, so this worker handles one
            // final operation and then exits. It is reserved at init rather
            // than spawned by a timeout caller.
            if let Ok(command) = receiver.recv() {
                command();
            }
        })?;
    let _ = SHUTDOWN_COORDINATOR.set(ShutdownCoordinator {
        state: Mutex::new(ShutdownState::NotStarted),
        complete: Condvar::new(),
        work,
    });
    Ok(())
}

#[cfg(feature = "test_hooks")]
#[doc(hidden)]
pub fn fail_next_shutdown_coordinator_reservation() {
    FAIL_NEXT_COORDINATOR_RESERVATION.store(true, Ordering::SeqCst);
}

/// Pauses the next terminal shutdown save before it locks the coordinator.
#[cfg(feature = "test_hooks")]
#[doc(hidden)]
pub fn block_next_shutdown_save(entered: mpsc::SyncSender<()>, release: mpsc::Receiver<()>) {
    *SAVE_SHUTDOWN_HOOK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(SaveShutdownHook { entered, release });
}

/// Signals immediately before the next in-progress waiter sleeps on completion.
#[cfg(feature = "test_hooks")]
#[doc(hidden)]
pub fn notify_next_wait_stopped(entered: mpsc::SyncSender<()>) {
    *WAIT_STOPPED_HOOK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(entered);
}

#[cfg(feature = "test_hooks")]
fn run_save_shutdown_hook() {
    let hook = SAVE_SHUTDOWN_HOOK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    if let Some(hook) = hook {
        let _ = hook.entered.send(());
        let _ = hook.release.recv();
    }
}

#[cfg(feature = "test_hooks")]
fn notify_wait_stopped_hook() {
    if let Some(entered) = WAIT_STOPPED_HOOK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
    {
        let _ = entered.send(());
    }
}

fn shutdown_coordinator() -> Option<&'static ShutdownCoordinator> {
    SHUTDOWN_COORDINATOR.get()
}

pub(crate) fn begin_shutdown() -> bool {
    let Some(coordinator) = shutdown_coordinator() else {
        return false;
    };
    let mut state = coordinator
        .state
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    match &*state {
        ShutdownState::NotStarted => {
            *state = ShutdownState::InProgress;
            true
        }
        ShutdownState::InProgress | ShutdownState::Complete(_) => false,
    }
}

fn save_shutdown(outcome: ShutdownOutcome, lifecycle: BridgeLifecycle) {
    set_lifecycle(lifecycle);
    let Some(coordinator) = shutdown_coordinator() else {
        return;
    };
    #[cfg(feature = "test_hooks")]
    run_save_shutdown_hook();
    let mut state = coordinator
        .state
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if !matches!(&*state, ShutdownState::Complete(_)) {
        let result = match health::snapshot() {
            Ok(health) => Ok(ShutdownReport { outcome, health }),
            Err(error) => Err(wait_error_from_snapshot(error)),
        };
        *state = ShutdownState::Complete(Box::new(result));
        coordinator.complete.notify_all();
    }
}

fn wait_error_from_snapshot(error: crate::ControlError) -> crate::WaitError {
    let diagnostic = match error {
        crate::ControlError::Query { diagnostic }
        | crate::ControlError::Unavailable { diagnostic } => diagnostic,
        crate::ControlError::NotRunning { phase } => diagnostic(
            crate::error_codes::SC_OBSERVABILITY_LOG_STATUS_UNAVAILABLE,
            format!("shutdown health became unavailable in {phase:?}"),
        ),
    };
    crate::WaitError::Unavailable { diagnostic }
}

fn diagnostic(
    code: sc_observability_types::ErrorCode,
    message: String,
) -> sc_observability_types::OperationDiagnostic {
    sc_observability_types::OperationDiagnostic {
        code,
        message,
        remediation: sc_observability_types::Remediation::not_recoverable(
            "inspect retained lifecycle health; writer completion is unconfirmed",
        ),
        at: sc_observability_types::Timestamp::now_utc(),
    }
}

fn save_unconfirmed(cause: UnconfirmedShutdown) {
    save_shutdown(
        ShutdownOutcome::Unconfirmed { cause },
        BridgeLifecycle::Failed,
    );
}

/// Waits only for an already-started shutdown and returns its exact retained
/// result. It never reconstructs a report from the current lifecycle.
pub(crate) fn wait_stopped(timeout: Duration) -> Result<ShutdownReport, crate::WaitError> {
    let Some(coordinator) = shutdown_coordinator() else {
        return Err(crate::WaitError::NotStarted);
    };
    let deadline = Instant::now() + timeout;
    let mut state = coordinator
        .state
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    loop {
        match &*state {
            ShutdownState::NotStarted => return Err(crate::WaitError::NotStarted),
            ShutdownState::Complete(result) => return *result.clone(),
            ShutdownState::InProgress => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(crate::WaitError::TimedOut { timeout });
                }
                #[cfg(feature = "test_hooks")]
                notify_wait_stopped_hook();
                let (next, result) = coordinator
                    .complete
                    .wait_timeout(state, remaining)
                    .unwrap_or_else(PoisonError::into_inner);
                state = next;
                if result.timed_out() && matches!(&*state, ShutdownState::InProgress) {
                    return Err(crate::WaitError::TimedOut { timeout });
                }
            }
        }
    }
}

static QUEUE_FULL: AtomicU64 = AtomicU64::new(0);
static INVALID_EVENT: AtomicU64 = AtomicU64::new(0);
static WRITER_DEGRADED: AtomicU64 = AtomicU64::new(0);
static SHUTDOWN_TIMED_OUT: AtomicU64 = AtomicU64::new(0);
static NOT_INSTALLED: AtomicU64 = AtomicU64::new(0);
static LOGGER_PANICKED: AtomicU64 = AtomicU64::new(0);
static REENTRANT_EMIT: AtomicU64 = AtomicU64::new(0);

fn counter(cause: DropCause) -> &'static AtomicU64 {
    match cause {
        DropCause::QueueFull => &QUEUE_FULL,
        DropCause::InvalidEvent => &INVALID_EVENT,
        DropCause::WriterDegraded => &WRITER_DEGRADED,
        DropCause::ShutdownTimedOut => &SHUTDOWN_TIMED_OUT,
        DropCause::NotInstalled => &NOT_INSTALLED,
        DropCause::LoggerPanicked => &LOGGER_PANICKED,
        DropCause::ReentrantEmit => &REENTRANT_EMIT,
    }
}

/// Counts one dropped event. Wrapped by `__private::record_drop`.
pub(crate) fn record_drop(cause: DropCause) {
    counter(cause).fetch_add(1, Ordering::Relaxed);
}

/// Reads the current value of one drop counter.
pub(crate) fn drop_count(cause: DropCause) -> u64 {
    counter(cause).load(Ordering::Relaxed)
}

/// Snapshot of every drop counter.
pub(crate) fn dropped_events() -> DroppedEvents {
    DroppedEvents::from_counter(drop_count)
}

/// Reads the staged core's coherent effective level for lazy facade and macro
/// evaluation. This is an optimization only: `try_log_with_outcome` remains the
/// authoritative admission decision and no bridge-owned threshold is retained.
pub(crate) fn core_enabled(level: sc_observability_types::Level) -> bool {
    let Some(installed) = current_installed() else {
        return false;
    };
    let effective = installed.logger.level_state().effective_level;
    level_rank(level) >= filter_rank(effective)
}

fn level_rank(level: sc_observability_types::Level) -> u8 {
    match level {
        sc_observability_types::Level::Trace => 0,
        sc_observability_types::Level::Debug => 1,
        sc_observability_types::Level::Info => 2,
        sc_observability_types::Level::Warn => 3,
        sc_observability_types::Level::Error => 4,
    }
}

fn filter_rank(level: sc_observability_types::LevelFilter) -> u8 {
    match level {
        sc_observability_types::LevelFilter::Trace => 0,
        sc_observability_types::LevelFilter::Debug => 1,
        sc_observability_types::LevelFilter::Info => 2,
        sc_observability_types::LevelFilter::Warn => 3,
        sc_observability_types::LevelFilter::Error => 4,
        sc_observability_types::LevelFilter::Off => 5,
    }
}

thread_local! {
    static IN_EMIT: Cell<bool> = const { Cell::new(false) };
}

/// Per-thread reentrancy guard. Only `try_with` is used: `with` can panic.
pub(crate) struct EmitScope(());

impl EmitScope {
    /// Enters the scope, or returns `None` when this thread is already inside `emit`.
    pub(crate) fn enter() -> Option<Self> {
        match IN_EMIT.try_with(|flag| flag.replace(true)) {
            Ok(false) => Some(Self(())),
            // Reentrant, or TLS unavailable (impossible for a const Cell<bool>): treated as reentrant.
            Ok(true) | Err(_) => None,
        }
    }
}

impl Drop for EmitScope {
    fn drop(&mut self) {
        // Also runs while a panic unwinds, so the flag is always released.
        let _ = IN_EMIT.try_with(|flag| flag.set(false));
    }
}

/// The guarded submission core: reentrancy guard plus panic containment.
///
/// This is the single outermost boundary of every submission, shared by the
/// `log` facade, the event macros / `#[instrument]` and `LogControl::try_log`:
/// exactly one call per record. A rejection is counted under its one
/// [`DropCause`] *before* it is returned, so a caller that discards the result
/// (the facade and the macros) still leaves exactly-once drop accounting.
///
/// The closure must reach the logger only through the unguarded cores
/// [`submit_installed`] / [`submit_to`]; calling a guarded entry point from
/// inside the closure would classify the record as `DropCause::ReentrantEmit`.
pub(crate) fn submit_guarded<T, E: Rejection>(
    submit: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    let Some(_scope) = EmitScope::enter() else {
        let rejection = E::reentrant();
        record_drop(rejection.drop_cause());
        return Err(rejection);
    };
    // AssertUnwindSafe: the closure reaches Logger, which holds dyn LogSink / dyn Redactor /
    // dyn ProcessIdentityResolver; none is RefUnwindSafe (E0277 without the wrapper). After a
    // caught panic nothing reads logger state except try_log itself, which reports its
    // poisoned mutexes by panicking again.
    let result =
        catch_unwind(AssertUnwindSafe(submit)).unwrap_or_else(|_payload| Err(E::panicked()));
    if let Err(rejection) = &result {
        record_drop(rejection.drop_cause());
    }
    result
}

/// Unguarded submit core: event assembly, ambient trace context and `Logger::try_log`.
///
/// Never call it outside a [`submit_guarded`] closure: it neither contains panics
/// nor detects reentrancy.
pub(crate) fn submit_to(
    installed: &Installed,
    parts: EventParts,
) -> Result<sc_observability_types::AdmissionOutcome, DropCause> {
    let mut event = crate::mapping::assemble_event(
        parts,
        &installed.service,
        &installed.identity,
        &installed.options.default_action,
    );
    // Ambient `#[instrument]` context of the emitting thread; bridge records included.
    event.trace = crate::context::current_trace();
    installed
        .logger
        .try_log_with_outcome(event)
        .map_err(|error| match error {
            TryLogError::QueueFull(_) => DropCause::QueueFull,
            TryLogError::InvalidEvent(_) => DropCause::InvalidEvent,
            TryLogError::WriterDegraded(_) => DropCause::WriterDegraded,
            TryLogError::ShutdownTimedOut(_) => DropCause::ShutdownTimedOut,
        })
}

/// Unguarded submit core for pre-built parts: slot read, then [`submit_to`].
///
/// Same contract as [`submit_to`]: call it only inside a [`submit_guarded`] closure.
pub(crate) fn submit_installed(
    parts: EventParts,
) -> Result<sc_observability_types::AdmissionOutcome, DropCause> {
    let installed = current_installed().ok_or(DropCause::NotInstalled)?;
    submit_to(&installed, parts)
}

/// Crate-private failure of [`run_bounded`], mapped 1:1 into `FlushError` / `ShutdownError`.
#[derive(Debug)]
pub(crate) enum BoundedError {
    /// The work did not finish within the timeout; the helper is detached.
    TimedOut,
    /// The helper thread could not be started.
    Spawn { source: std::io::Error },
    /// The channel disconnected without a value: the work closure panicked.
    WorkerLost,
}

/// Helpers whose caller timed out and whose work has not finished (flush and shutdown).
///
/// This remains private accounting for bounded helper cleanup; native health
/// deliberately exposes the staged core report plus bridge lifecycle only.
static DETACHED_HELPERS: AtomicU32 = AtomicU32::new(0);

/// Set while a flush helper runs: at most one flush helper per installed bridge.
static FLUSH_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// Per-helper state shared by the caller and the helper of one [`run_bounded`] call.
const HELPER_RUNNING: u8 = 0;
/// The caller timed out and counted the helper in its detached counter.
const HELPER_DETACHED: u8 = 1;
/// The helper's work returned or unwound.
const HELPER_DONE: u8 = 2;

/// Marks the helper done when its work returns or unwinds, and uncounts it if detached.
struct HelperExit {
    state: Arc<AtomicU8>,
    detached: &'static AtomicU32,
}

impl Drop for HelperExit {
    fn drop(&mut self) {
        // Also runs while a panic in the work unwinds.
        if self.state.swap(HELPER_DONE, Ordering::SeqCst) == HELPER_DETACHED {
            self.detached.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

/// Exclusive claim on a single-flight flag; released on drop, including during unwinding.
struct Flight {
    flag: &'static AtomicBool,
}

impl Flight {
    /// Claims `flag` without blocking, or returns `None` while another claim is alive.
    fn claim(flag: &'static AtomicBool) -> Option<Self> {
        flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| Self { flag })
    }
}

impl Drop for Flight {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

/// Runs `work` on a named helper thread and waits at most `timeout` for its result.
///
/// A helper left running past `timeout` is counted in the process-wide detached
/// counter until its work finishes.
pub(crate) fn run_bounded<T: Send + 'static>(
    timeout: Duration,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, BoundedError> {
    run_bounded_in(&DETACHED_HELPERS, timeout, work, |_| {})
}

/// [`run_bounded`] against an explicit detached counter (unit tests use their own).
///
/// `after_timeout` runs once, only after `recv_timeout` has timed out and before the
/// caller's RUNNING→DETACHED transition, with the helper's state. Production passes a
/// no-op; unit tests use it to order the helper's completion against that transition.
fn run_bounded_in<T: Send + 'static>(
    detached: &'static AtomicU32,
    timeout: Duration,
    work: impl FnOnce() -> T + Send + 'static,
    after_timeout: impl FnOnce(&AtomicU8),
) -> Result<T, BoundedError> {
    let (tx, rx) = mpsc::sync_channel(1);
    let state = Arc::new(AtomicU8::new(HELPER_RUNNING));
    let exit = HelperExit {
        state: Arc::clone(&state),
        detached,
    };
    std::thread::Builder::new()
        .name("sc-observability-log-helper".to_owned())
        .spawn(move || {
            // Keep `exit` alive until the result is in the channel.  A caller
            // that loses the timeout race can then observe a completed helper
            // with `try_recv`, never with an unbounded second receive.
            let _exit = exit;
            let value = work();
            let _ = tx.send(value);
        })
        .map_err(|source| BoundedError::Spawn { source })?;
    match rx.recv_timeout(timeout) {
        Ok(value) => Ok(value),
        Err(RecvTimeoutError::Disconnected) => Err(BoundedError::WorkerLost),
        Err(RecvTimeoutError::Timeout) => {
            after_timeout(&state);
            // Count first, then publish: the helper's decrement can never precede this increment.
            detached.fetch_add(1, Ordering::SeqCst);
            if state
                .compare_exchange(
                    HELPER_RUNNING,
                    HELPER_DETACHED,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                )
                .is_ok()
            {
                return Err(BoundedError::TimedOut);
            }
            // A failed CAS means the helper has already published its result
            // before changing state to done. Do not make the stated timeout
            // conditional on scheduler progress after this point.
            detached.fetch_sub(1, Ordering::SeqCst);
            match rx.try_recv() {
                Ok(value) => Ok(value),
                Err(TryRecvError::Empty) => Err(BoundedError::TimedOut),
                Err(TryRecvError::Disconnected) => Err(BoundedError::WorkerLost),
            }
        }
    }
}

const UNWRAP_BACKOFF_START: Duration = Duration::from_millis(1);
const UNWRAP_BACKOFF_MAX: Duration = Duration::from_millis(50);

/// Retries `Arc::try_unwrap` with a sleep backoff until every other clone is released.
///
/// Has no deadline: it returns only once this call holds the sole reference.
pub(crate) fn take_sole<T>(mut shared: Arc<T>) -> T {
    let mut backoff = UNWRAP_BACKOFF_START;
    loop {
        match Arc::try_unwrap(shared) {
            Ok(value) => return value,
            Err(still_shared) => {
                shared = still_shared;
                std::thread::sleep(backoff);
                backoff = backoff.saturating_mul(2).min(UNWRAP_BACKOFF_MAX);
            }
        }
    }
}

/// Shutdown helper. `Logger::shutdown(self)` consumes the logger (runtime.rs:224), so the
/// helper first gains sole ownership.
///
/// The caller waits at most `timeout`. The helper itself has no deadline: an
/// `Arc<Installed>` clone held past `timeout` (a detached `flush` helper, or a
/// submission blocked in a sink or redactor) makes the caller return
/// `ShutdownError::TimedOut` while the detached helper keeps waiting. When it
/// completes late it stores the final health report and publishes
/// `BridgeLifecycle::Stopped`, so the late completion is observable.
pub(crate) fn shutdown_installed(
    installed: Arc<Installed>,
    timeout: Duration,
) -> Result<(), ShutdownError> {
    enum WorkerOutcome {
        Completed(Result<(), sc_observability_types::FlushError>),
        Panicked,
    }

    let Some(coordinator) = shutdown_coordinator() else {
        let source =
            std::io::Error::other("shutdown coordinator was not reserved at initialization");
        save_unconfirmed(UnconfirmedShutdown::HelperSpawn {
            diagnostic: diagnostic(
                crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
                source.to_string(),
            ),
        });
        return Err(ShutdownError::HelperSpawn {
            diagnostic: diagnostic(
                crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
                source.to_string(),
            ),
        });
    };
    let work = coordinator.work.clone();
    let (result_tx, result_rx) = mpsc::sync_channel(1);
    let command: ShutdownCommand = Box::new(move || {
        let result = catch_unwind(AssertUnwindSafe(|| {
            #[cfg(test)]
            run_shutdown_work_hook();
            let sole = take_sole(installed);
            let flushed = sole.logger.flush();
            let stopped = sole.logger.shutdown();
            health::store_level_state(stopped.level_state());
            if let Some(report) = health::read_report(&stopped) {
                health::store_report(report);
            }
            let outcome = match &flushed {
                Ok(()) => ShutdownOutcome::Stopped,
                Err(source) => ShutdownOutcome::StoppedWithFlushError {
                    diagnostic: diagnostic(source.diagnostic().code.clone(), source.to_string()),
                },
            };
            save_shutdown(outcome, BridgeLifecycle::Stopped);
            flushed
        }));
        let outcome = if let Ok(flushed) = result {
            WorkerOutcome::Completed(flushed)
        } else {
            save_unconfirmed(UnconfirmedShutdown::HelperLost {
                diagnostic: diagnostic(
                    crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST,
                    "the reserved shutdown worker panicked before completion".to_owned(),
                ),
            });
            WorkerOutcome::Panicked
        };
        let _ = result_tx.send(outcome);
    });
    if let Err(error) = work.send(command) {
        let source = std::io::Error::other(error.to_string());
        save_unconfirmed(UnconfirmedShutdown::HelperSpawn {
            diagnostic: diagnostic(
                crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
                source.to_string(),
            ),
        });
        return Err(ShutdownError::HelperSpawn {
            diagnostic: diagnostic(
                crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
                source.to_string(),
            ),
        });
    }
    match result_rx.recv_timeout(timeout) {
        Ok(WorkerOutcome::Completed(Ok(()))) => Ok(()),
        Ok(WorkerOutcome::Completed(Err(source))) => Err(ShutdownError::FinalFlush {
            diagnostic: crate::error::diagnostic_from_info(&source),
        }),
        Ok(WorkerOutcome::Panicked) | Err(RecvTimeoutError::Disconnected) => {
            save_unconfirmed(UnconfirmedShutdown::HelperLost {
                diagnostic: diagnostic(
                    crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST,
                    "the reserved shutdown worker ended without a result".to_owned(),
                ),
            });
            Err(ShutdownError::HelperLost {
                diagnostic: diagnostic(
                    crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST,
                    "the reserved shutdown worker ended without a result".to_owned(),
                ),
            })
        }
        Err(RecvTimeoutError::Timeout) => Err(ShutdownError::TimedOut { timeout }),
    }
}

/// `LogGuard::shutdown` and `Drop for LogGuard` both call this once.
///
/// The lifecycle is `ShuttingDown` from the first statement. When this returns
/// it is `Stopped` for every result except `ShutdownError::TimedOut`, which
/// leaves `ShutdownTimedOut` until the detached helper completes and publishes
/// `Stopped` (see [`shutdown_installed`]).
pub(crate) fn shutdown_sequence(timeout: Duration) -> Result<(), ShutdownError> {
    if !begin_shutdown() {
        return Ok(());
    }
    set_lifecycle(BridgeLifecycle::ShuttingDown);
    // Runtime admission belongs exclusively to the staged core logger.  The
    // facade is disabled only because shutdown has started; it is never a
    // second threshold policy.
    log::set_max_level(log::LevelFilter::Off);
    let taken = SLOT.write().unwrap_or_else(PoisonError::into_inner).take();
    if let Some(installed) = &taken {
        health::store_level_state(installed.logger.level_state());
        if let Some(report) = health::read_report(&installed.logger) {
            health::store_report(report);
        }
    }
    let no_logger = taken.is_none();
    let result = match taken {
        Some(installed) => shutdown_installed(installed, timeout),
        None => Ok(()),
    };
    if no_logger {
        save_shutdown(ShutdownOutcome::Stopped, BridgeLifecycle::Stopped);
    }
    result
}

/// Clones the installed `Arc` out of the slot; the read lock is held only for the clone.
pub(crate) fn current_installed() -> Option<Arc<Installed>> {
    SLOT.read().unwrap_or_else(PoisonError::into_inner).clone()
}

/// `LogGuard::flush` / `LogControl::flush`: the helper owns an `Arc` clone until
/// sc-observability's flush returns.
///
/// An empty slot means shutdown has taken the logger: `FlushError::NotRunning`. A
/// flush whose helper already holds its clone when shutdown starts is awaited by
/// the shutdown's `take_sole`, within the shutdown's own timeout.
///
/// At most one flush helper runs at a time. While one is running (also after its
/// caller timed out and detached it), a new flush spawns nothing and returns
/// `FlushError::InProgress`, so a stuck sink cannot accumulate helper threads.
pub(crate) fn flush_installed(timeout: Duration) -> Result<(), FlushError> {
    if lifecycle() != BridgeLifecycle::Running {
        return Err(FlushError::NotRunning {
            phase: lifecycle_phase(),
        });
    }
    let Some(installed) = current_installed() else {
        return Err(FlushError::NotRunning {
            phase: lifecycle_phase(),
        });
    };
    let Some(flight) = Flight::claim(&FLUSH_IN_FLIGHT) else {
        return Err(FlushError::InProgress);
    };
    let flush = move || {
        // Released when the flush returns or unwinds, before the result is sent.
        let _flight = flight;
        installed.logger.flush()
    };
    match run_bounded(timeout, flush) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(source)) => Err(FlushError::Logger {
            diagnostic: crate::error::diagnostic_from_info(&source),
        }),
        Err(BoundedError::TimedOut) => Err(FlushError::TimedOut { timeout }),
        Err(BoundedError::Spawn { source }) => Err(FlushError::HelperSpawn {
            diagnostic: diagnostic(
                crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
                source.to_string(),
            ),
        }),
        Err(BoundedError::WorkerLost) => Err(FlushError::HelperLost {
            diagnostic: diagnostic(
                crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST,
                "the flush helper ended without a result".to_owned(),
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn run_bounded_times_out_on_blocked_work() {
        let started = Instant::now();
        let result = run_bounded(Duration::from_millis(100), || {
            std::thread::sleep(Duration::from_secs(2));
        });
        assert!(matches!(result, Err(BoundedError::TimedOut)));
        assert!(started.elapsed() < Duration::from_millis(600));
    }

    #[test]
    fn run_bounded_reports_worker_lost_on_panic() {
        let result: Result<(), BoundedError> = run_bounded(Duration::from_secs(5), || {
            panic!("worker panic (expected by this test)");
        });
        assert!(matches!(result, Err(BoundedError::WorkerLost)));
    }

    #[test]
    fn reserved_shutdown_worker_publishes_failure_after_a_waiter_times_out() {
        let root = std::env::temp_dir().join(format!("bp3-worker-{}", std::process::id()));
        let service = sc_observability_types::ServiceName::new("bp3-worker");
        assert!(service.is_ok());
        let Some(service) = service.ok() else {
            return;
        };
        let mut config = sc_observability::LoggerConfig::default_for(service.clone(), root);
        config.enable_console_sink = false;
        let logger = sc_observability::Logger::new(config);
        assert!(logger.is_ok());
        let Some(logger) = logger.ok() else {
            return;
        };
        let initial_report = logger.health();
        health::set_snapshot_config(health::SinkConfig {
            active_log_path: Some(initial_report.active_log_path.clone()),
            level_state: logger.level_state(),
        });
        health::store_report(initial_report);
        let action = sc_observability_types::ActionName::new("log.record");
        assert!(action.is_ok());
        let Some(action) = action.ok() else {
            return;
        };
        let installed = Arc::new(Installed {
            logger,
            service,
            identity: sc_observability_types::ProcessIdentity::default(),
            options: crate::BridgeOptions {
                default_action: action,
                parse_bracket_action: false,
            },
        });
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        *SHUTDOWN_WORK_HOOK
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(Box::new(move || {
            let _ = entered_tx.send(());
            let _ = release_rx.recv();
            panic!("test worker failure");
        }));
        assert!(reserve_shutdown_coordinator().is_ok());
        assert!(begin_shutdown());
        set_lifecycle(BridgeLifecycle::ShuttingDown);
        let owner =
            std::thread::spawn(move || shutdown_installed(installed, Duration::from_millis(1)));
        assert!(entered_rx.recv().is_ok());
        assert!(matches!(
            wait_stopped(Duration::from_millis(1)),
            Err(crate::WaitError::TimedOut { .. })
        ));
        assert!(matches!(
            owner.join(),
            Ok(Err(ShutdownError::TimedOut { .. }))
        ));
        assert!(release_tx.send(()).is_ok());
        assert!(matches!(
            wait_stopped(Duration::from_secs(1)),
            Ok(ShutdownReport {
                outcome: ShutdownOutcome::Unconfirmed { .. },
                ..
            })
        ));
        assert_eq!(lifecycle(), BridgeLifecycle::Failed);
    }

    /// Signals its channel when dropped: the work closure has returned.
    struct SignalOnDrop(mpsc::SyncSender<()>);

    impl Drop for SignalOnDrop {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }

    /// A helper is counted as detached from its caller's timeout until its work finishes.
    #[test]
    fn run_bounded_counts_a_detached_helper_until_it_finishes() {
        static DETACHED: AtomicU32 = AtomicU32::new(0);
        let (release_tx, release_rx) = mpsc::sync_channel::<()>(0);
        let (done_tx, done_rx) = mpsc::sync_channel::<()>(1);
        let result = run_bounded_in(
            &DETACHED,
            Duration::from_millis(20),
            move || {
                let _done = SignalOnDrop(done_tx);
                release_rx.recv().unwrap();
            },
            |_| {},
        );
        assert!(matches!(result, Err(BoundedError::TimedOut)));
        assert_eq!(DETACHED.load(Ordering::SeqCst), 1);
        release_tx.send(()).unwrap();
        done_rx.recv().unwrap();
        // The work has returned; the helper's exit guard runs right after the closure.
        let deadline = Instant::now() + Duration::from_secs(10);
        while DETACHED.load(Ordering::SeqCst) != 0 {
            assert!(Instant::now() < deadline, "detached helper never uncounted");
            std::thread::yield_now();
        }
        assert_eq!(
            run_bounded_in(&DETACHED, Duration::from_secs(5), || 3, |_| {}).ok(),
            Some(3)
        );
        assert_eq!(DETACHED.load(Ordering::SeqCst), 0);
    }

    /// A panicking detached helper is uncounted by its unwinding exit guard.
    #[test]
    fn run_bounded_uncounts_a_detached_helper_that_panics() {
        static DETACHED: AtomicU32 = AtomicU32::new(0);
        let (release_tx, release_rx) = mpsc::sync_channel::<()>(0);
        let result: Result<(), BoundedError> = run_bounded_in(
            &DETACHED,
            Duration::from_millis(20),
            move || {
                release_rx.recv().unwrap();
                panic!("detached helper panic (expected by this test)");
            },
            |_| {},
        );
        assert!(matches!(result, Err(BoundedError::TimedOut)));
        assert_eq!(DETACHED.load(Ordering::SeqCst), 1);
        release_tx.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while DETACHED.load(Ordering::SeqCst) != 0 {
            assert!(
                Instant::now() < deadline,
                "panicking helper never uncounted"
            );
            std::thread::yield_now();
        }
    }

    /// R-A5-006 losing race: the helper publishes its value and completes after the
    /// caller's `recv_timeout` timed out but before the caller's RUNNING→DETACHED CAS.
    ///
    /// The `after_timeout` seam releases the work and waits for `HELPER_DONE`, so the
    /// CAS deterministically fails and the caller must take the value with `try_recv`.
    #[test]
    fn run_bounded_losing_race_returns_the_published_value() {
        static DETACHED: AtomicU32 = AtomicU32::new(0);
        let (release_tx, release_rx) = mpsc::sync_channel::<()>(0);
        let mut seam_ran = false;
        let result = run_bounded_in(
            &DETACHED,
            Duration::from_millis(20),
            move || {
                release_rx.recv().unwrap();
                11_u32
            },
            |state| {
                seam_ran = true;
                release_tx.send(()).unwrap();
                // The helper sends its value before its exit guard publishes DONE.
                let deadline = Instant::now() + Duration::from_secs(10);
                while state.load(Ordering::SeqCst) != HELPER_DONE {
                    assert!(Instant::now() < deadline, "helper never completed");
                    std::thread::yield_now();
                }
            },
        );
        assert!(seam_ran, "the caller's recv_timeout must have timed out");
        assert!(matches!(result, Ok(11)), "{result:?}");
        assert_eq!(DETACHED.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn flight_claim_is_exclusive_and_released_on_drop_or_panic() {
        static FLAG: AtomicBool = AtomicBool::new(false);
        let first = Flight::claim(&FLAG).unwrap();
        assert!(Flight::claim(&FLAG).is_none());
        drop(first);
        let second = Flight::claim(&FLAG).unwrap();
        let unwound = std::thread::spawn(move || {
            let _held = second;
            panic!("flight holder panic (expected by this test)");
        })
        .join();
        assert!(unwound.is_err());
        assert!(!FLAG.load(Ordering::SeqCst));
        assert!(Flight::claim(&FLAG).is_some());
    }

    #[test]
    fn run_bounded_returns_value() {
        assert!(matches!(run_bounded(Duration::from_secs(5), || 7), Ok(7)));
    }

    #[test]
    fn take_sole_succeeds_once_clone_is_released() {
        let shared = Arc::new(5_u32);
        let clone = Arc::clone(&shared);
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            drop(clone);
        });
        assert_eq!(take_sole(shared), 5);
        releaser.join().unwrap();
    }

    #[test]
    fn bridge_has_no_independent_level_policy() {
        // Admission filtering is delegated to sc-observability's LevelOwner.
        // The bridge keeps no mutable level threshold to test or synchronize.
        assert_eq!(log::STATIC_MAX_LEVEL, log::LevelFilter::Trace);
    }

    /// The only unit test that calls `submit_guarded` or `record_drop`, so its counter deltas cannot race.
    #[test]
    fn emit_core_counts_panics_and_reentry() {
        // 1. A nested call inside the closure is counted once as ReentrantEmit.
        let reentrant_before = drop_count(DropCause::ReentrantEmit);
        let panicked_before = drop_count(DropCause::LoggerPanicked);
        let _ = submit_guarded(|| {
            assert_eq!(
                submit_guarded(|| Ok::<(), DropCause>(())),
                Err(DropCause::ReentrantEmit)
            );
            Ok::<(), DropCause>(())
        });
        assert_eq!(drop_count(DropCause::ReentrantEmit), reentrant_before + 1);
        assert_eq!(drop_count(DropCause::LoggerPanicked), panicked_before);

        // 2. A panicking closure is counted once as LoggerPanicked, and the panic hook's
        //    own submit_guarded call (same thread, still inside emit) once as ReentrantEmit.
        std::panic::set_hook(Box::new(|_| {
            let _ = submit_guarded(|| Ok::<(), DropCause>(()));
        }));
        let panicked: Result<(), DropCause> =
            submit_guarded(|| panic!("simulated sc-observability panic"));
        assert_eq!(panicked, Err(DropCause::LoggerPanicked));
        // 3. Remove the hook.
        let _ = std::panic::take_hook();
        assert_eq!(drop_count(DropCause::LoggerPanicked), panicked_before + 1);
        assert_eq!(drop_count(DropCause::ReentrantEmit), reentrant_before + 2);

        // 4. The EmitScope was released during unwinding: a fresh call is not counted.
        assert_eq!(submit_guarded(|| Ok::<(), DropCause>(())), Ok(()));
        assert_eq!(drop_count(DropCause::ReentrantEmit), reentrant_before + 2);
        assert_eq!(drop_count(DropCause::LoggerPanicked), panicked_before + 1);

        // A closure error is counted under its own cause.
        let invalid_before = drop_count(DropCause::InvalidEvent);
        assert_eq!(
            submit_guarded(|| Err::<(), DropCause>(DropCause::InvalidEvent)),
            Err(DropCause::InvalidEvent)
        );
        assert_eq!(drop_count(DropCause::InvalidEvent), invalid_before + 1);
        record_drop(DropCause::QueueFull);
        let snapshot = dropped_events();
        assert_eq!(
            snapshot.total(),
            DropCause::ALL
                .iter()
                .map(|cause| snapshot.get(*cause))
                .sum::<u64>()
        );
    }
}

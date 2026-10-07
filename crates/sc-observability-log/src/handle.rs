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
#[cfg(test)]
use crate::constants::{
    BOUNDED_HELPER_BLOCK, BOUNDED_HELPER_PANIC, BOUNDED_HELPER_SPAWN_FAILURE,
    ISOLATED_TEST_CHILD_DEADLINE, ISOLATED_TEST_CHILD_POLL_INTERVAL,
};
use crate::constants::{
    HELPER_DETACHED, HELPER_DONE, HELPER_RUNNING, LIFECYCLE_FAILED, LIFECYCLE_RUNNING,
    LIFECYCLE_SHUTTING_DOWN, LIFECYCLE_STOPPED, UNWRAP_BACKOFF_MAX, UNWRAP_BACKOFF_START,
};
use crate::health::BridgeLifecycle;
use crate::{
    DropCause, DroppedEvents, ShutdownOutcome, ShutdownReport, UnconfirmedShutdown, health,
};
use sc_observability_types::FailureClassification;
use sc_observability_types::v2::EventError;
use sc_observability_types::v2::{FlushError, ShutdownError};
use sc_observability_types::{Level, LevelFilter};

/// A rejected submission: every failure of the guarded core maps to exactly one [`DropCause`].
///
/// Implemented by `DropCause` (the facade and the macros, which discard the
/// result) and by [`crate::v2::EmitError`] (`LogControl::try_log`, which returns it).
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

impl Rejection for crate::error::EmitError {
    fn drop_cause(&self) -> DropCause {
        match self {
            Self::InvalidField { .. } | Self::InvalidEvent { .. } => DropCause::InvalidEvent,
            Self::QueueFull { .. } => DropCause::QueueFull,
            Self::WriterDegraded { .. } => DropCause::WriterDegraded,
            Self::ShutdownTimedOut { .. } => DropCause::ShutdownTimedOut,
            Self::NotRunning { .. } | Self::NotInstalled => DropCause::NotInstalled,
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
    pub(crate) logger: Arc<sc_observability::v2::Logger>,
    pub(crate) service: sc_observability_types::ServiceName,
    pub(crate) identity: sc_observability_types::ProcessIdentity,
    pub(crate) options: crate::BridgeOptions,
}

// MUTEX: coordinates install, short Arc-clone reads, and shutdown take; all slot accesses recover poison.
pub(crate) static SLOT: RwLock<Option<Arc<Installed>>> = RwLock::new(None);
pub(crate) static INSTALLED: AtomicBool = AtomicBool::new(false);

// Test-only observability for the actual native flush boundary.  This remains
// private to the crate: the public facade must not expose a test counter.
#[cfg(test)]
thread_local! {
    static NATIVE_FLUSH_CALLS: Cell<u64> = const { Cell::new(0) };
}

// A bounded, crate-private fault seam for the actual helper spawn point. It
// is compiled only for unit tests; non-test execution always calls
// `Builder::spawn` unchanged.
#[cfg(test)]
static NEXT_BOUNDED_HELPER_FAULT: AtomicU8 = AtomicU8::new(0);
#[cfg(test)]
static NEXT_BOUNDED_HELPER_BLOCK: OnceLock<Mutex<Option<mpsc::Receiver<()>>>> = OnceLock::new();

#[cfg(test)]
fn fail_next_bounded_helper_spawn() {
    NEXT_BOUNDED_HELPER_FAULT.store(BOUNDED_HELPER_SPAWN_FAILURE, Ordering::SeqCst);
}

#[cfg(test)]
fn panic_next_bounded_helper() {
    NEXT_BOUNDED_HELPER_FAULT.store(BOUNDED_HELPER_PANIC, Ordering::SeqCst);
}

#[cfg(test)]
fn block_next_bounded_helper() -> mpsc::SyncSender<()> {
    let (release, blocked) = mpsc::sync_channel(0);
    *NEXT_BOUNDED_HELPER_BLOCK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(blocked);
    NEXT_BOUNDED_HELPER_FAULT.store(BOUNDED_HELPER_BLOCK, Ordering::SeqCst);
    release
}

#[cfg(test)]
pub(crate) fn reset_native_flush_calls() {
    NATIVE_FLUSH_CALLS.with(|calls| calls.set(0));
}

#[cfg(test)]
pub(crate) fn native_flush_calls() -> u64 {
    NATIVE_FLUSH_CALLS.with(Cell::get)
}

#[cfg(test)]
fn record_native_flush_call() {
    NATIVE_FLUSH_CALLS.with(|calls| calls.set(calls.get() + 1));
}

/// Encoded [`BridgeLifecycle`]; `Stopped` until `init` succeeds (no guard exists before).
static LIFECYCLE: AtomicU8 = AtomicU8::new(LIFECYCLE_STOPPED);

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
    // MUTEX: shares the single shutdown transition/result with waiters; state accesses recover poison.
    state: Mutex<ShutdownState>,
    complete: Condvar,
    work: mpsc::Sender<ShutdownCommand>,
}

static SHUTDOWN_COORDINATOR: OnceLock<ShutdownCoordinator> = OnceLock::new();

#[cfg(test)]
static FAIL_NEXT_COORDINATOR_RESERVATION: AtomicBool = AtomicBool::new(false);

type ShutdownCommand = Box<dyn FnOnce() + Send + 'static>;

enum ShutdownWorkerOutcome {
    Completed(Result<(), sc_observability_types::v2::FlushError>),
    Panicked,
}

fn shutdown_command(
    installed: Arc<Installed>,
    result_tx: mpsc::SyncSender<ShutdownWorkerOutcome>,
) -> ShutdownCommand {
    Box::new(move || {
        let result = catch_unwind(AssertUnwindSafe(|| {
            #[cfg(test)]
            run_shutdown_work_hook();
            let sole = take_sole(installed);
            let logger = take_sole(sole.logger);
            let flushed = logger.flush();
            let _ = logger.shutdown();
            health::store_level_state(logger.level_state());
            if let Some(report) = health::read_report(&logger) {
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
            ShutdownWorkerOutcome::Completed(flushed)
        } else {
            save_unconfirmed(UnconfirmedShutdown::HelperLost {
                diagnostic: diagnostic(
                    crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST,
                    "the reserved shutdown worker panicked before completion".to_owned(),
                ),
            });
            ShutdownWorkerOutcome::Panicked
        };
        let _ = result_tx.send(outcome);
    })
}

#[cfg(test)]
struct SaveShutdownHook {
    entered: mpsc::SyncSender<()>,
    release: mpsc::Receiver<()>,
}

#[cfg(test)]
static SAVE_SHUTDOWN_HOOK: OnceLock<Mutex<Option<SaveShutdownHook>>> = OnceLock::new();

#[cfg(test)]
static WAIT_STOPPED_HOOK: OnceLock<Mutex<Option<mpsc::SyncSender<()>>>> = OnceLock::new();

#[cfg(test)]
static SHUTDOWN_WORK_HOOK: OnceLock<Mutex<Option<ShutdownCommand>>> = OnceLock::new();

#[cfg(test)]
struct ReapIsolatedTestChildOnDrop {
    child: Option<std::process::Child>,
}

#[cfg(test)]
impl ReapIsolatedTestChildOnDrop {
    fn new(child: std::process::Child) -> Self {
        Self { child: Some(child) }
    }

    fn child_mut(&mut self) -> &mut std::process::Child {
        self.child
            .as_mut()
            .expect("isolated test child remains guarded until it is reaped")
    }

    fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.child_mut().try_wait()
    }

    fn kill(&mut self) -> std::io::Result<()> {
        self.child_mut().kill()
    }

    fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        let status = self.child_mut().wait()?;
        self.child = None;
        Ok(status)
    }
}

#[cfg(test)]
impl Drop for ReapIsolatedTestChildOnDrop {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(test)]
#[derive(Debug)]
enum IsolatedTestChildRun {
    Completed(std::process::Output),
    TimedOut {
        elapsed: Duration,
        status: std::process::ExitStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        kill_error: Option<std::io::Error>,
    },
}

#[cfg(test)]
fn read_isolated_test_child_output(
    reader: impl std::io::Read + Send + 'static,
) -> std::thread::JoinHandle<std::io::Result<Vec<u8>>> {
    std::thread::spawn(move || {
        let mut reader = reader;
        let mut output = Vec::new();
        std::io::Read::read_to_end(&mut reader, &mut output)?;
        Ok(output)
    })
}

#[cfg(test)]
fn collect_isolated_test_child_output(
    reader: std::thread::JoinHandle<std::io::Result<Vec<u8>>>,
) -> Vec<u8> {
    match reader.join() {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => format!("<capture failed: {error}>").into_bytes(),
        Err(_) => b"<capture thread panicked>".to_vec(),
    }
}

#[cfg(test)]
fn run_isolated_test_child(
    command: &mut std::process::Command,
    deadline: Duration,
) -> IsolatedTestChildRun {
    let mut child = ReapIsolatedTestChildOnDrop::new(
        command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn isolated library test"),
    );
    let stdout = read_isolated_test_child_output(
        child
            .child_mut()
            .stdout
            .take()
            .expect("piped isolated test child stdout"),
    );
    let stderr = read_isolated_test_child_output(
        child
            .child_mut()
            .stderr
            .take()
            .expect("piped isolated test child stderr"),
    );
    let started = Instant::now();
    loop {
        if child
            .try_wait()
            .expect("poll isolated library test child")
            .is_some()
        {
            let status = child.wait().expect("reap isolated library test child");
            return IsolatedTestChildRun::Completed(std::process::Output {
                status,
                stdout: collect_isolated_test_child_output(stdout),
                stderr: collect_isolated_test_child_output(stderr),
            });
        }
        if started.elapsed() >= deadline {
            let kill_error = child.kill().err();
            let status = child
                .wait()
                .expect("reap timed-out isolated library test child");
            return IsolatedTestChildRun::TimedOut {
                elapsed: started.elapsed(),
                status,
                stdout: collect_isolated_test_child_output(stdout),
                stderr: collect_isolated_test_child_output(stderr),
                kill_error,
            };
        }
        std::thread::sleep(ISOLATED_TEST_CHILD_POLL_INTERVAL);
    }
}

#[cfg(test)]
pub(crate) fn is_isolated_test_child(test_name: &str) -> bool {
    const CHILD_ENV: &str = "SC_OBSERVABILITY_LOG_ISOLATED_LIB_TEST";
    if std::env::var(CHILD_ENV).as_deref() == Ok(test_name) {
        return true;
    }
    let mut command = std::process::Command::new(std::env::current_exe().expect("test executable"));
    command
        .args(["--exact", test_name, "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, test_name);
    let output = match run_isolated_test_child(&mut command, ISOLATED_TEST_CHILD_DEADLINE) {
        IsolatedTestChildRun::Completed(output) => output,
        IsolatedTestChildRun::TimedOut {
            elapsed,
            status,
            stdout,
            stderr,
            kill_error,
        } => panic!(
            "isolated {test_name} exceeded {ISOLATED_TEST_CHILD_DEADLINE:?} and was killed after {elapsed:?}: status={status}; kill_error={kill_error:?}; stdout={}\\nstderr={}",
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr),
        ),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success()
            && stdout.contains("running 1 test")
            && stdout.contains("test result: ok. 1 passed;"),
        "isolated {test_name} must execute and pass exactly one test: {}\n{stdout}\n{stderr}",
        output.status,
    );
    false
}

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
    #[cfg(test)]
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

#[cfg(all(test, feature = "v1"))]
fn fail_next_shutdown_coordinator_reservation() {
    FAIL_NEXT_COORDINATOR_RESERVATION.store(true, Ordering::SeqCst);
}

/// Pauses the next terminal shutdown save before it locks the coordinator.
#[cfg(test)]
fn block_next_shutdown_save(entered: mpsc::SyncSender<()>, release: mpsc::Receiver<()>) {
    *SAVE_SHUTDOWN_HOOK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(SaveShutdownHook { entered, release });
}

/// Signals immediately before the next in-progress waiter sleeps on completion.
#[cfg(test)]
fn notify_next_wait_stopped(entered: mpsc::SyncSender<()>) {
    *WAIT_STOPPED_HOOK
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(entered);
}

#[cfg(test)]
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

#[cfg(test)]
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
    #[cfg(test)]
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
                #[cfg(test)]
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
    if let Some(enabled) = crate::bridge::attached_enabled(level) {
        return enabled;
    }
    let Some(installed) = current_installed() else {
        return false;
    };
    let effective = installed.logger.level_state().effective_level;
    level_enabled(level, effective)
}

pub(crate) fn level_enabled(level: Level, effective: LevelFilter) -> bool {
    log_level_rank(level) >= log_level_rank(effective)
}

#[derive(Clone, Copy)]
pub(crate) enum RankedLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Off,
}

impl From<Level> for RankedLevel {
    fn from(level: Level) -> Self {
        match level {
            Level::Trace => Self::Trace,
            Level::Debug => Self::Debug,
            Level::Info => Self::Info,
            Level::Warn => Self::Warn,
            Level::Error => Self::Error,
        }
    }
}

impl From<LevelFilter> for RankedLevel {
    fn from(filter: LevelFilter) -> Self {
        match filter {
            LevelFilter::Trace => Self::Trace,
            LevelFilter::Debug => Self::Debug,
            LevelFilter::Info => Self::Info,
            LevelFilter::Warn => Self::Warn,
            LevelFilter::Error => Self::Error,
            LevelFilter::Off => Self::Off,
        }
    }
}

pub(crate) fn log_level_rank(level: impl Into<RankedLevel>) -> u8 {
    match level.into() {
        RankedLevel::Trace => 0,
        RankedLevel::Debug => 1,
        RankedLevel::Info => 2,
        RankedLevel::Warn => 3,
        RankedLevel::Error => 4,
        RankedLevel::Off => 5,
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
        .map_err(|error| event_drop_cause(&error))
}

/// Classifies a canonical admission failure into the drop cause it counts as.
///
/// Validation is an invalid event; routing failures carrying the core logger's
/// queue-full or shutdown-timeout code keep those causes. Every other failure is a
/// degraded writer.
pub(crate) fn event_drop_cause(error: &EventError) -> DropCause {
    match error {
        EventError::Validation { .. } => DropCause::InvalidEvent,
        EventError::Routing { context } => {
            let code = &context.diagnostic().code;
            if *code == sc_observability::error_codes::LOGGER_QUEUE_FULL {
                DropCause::QueueFull
            } else if *code == sc_observability::error_codes::LOGGER_SHUTDOWN_TIMED_OUT {
                DropCause::ShutdownTimedOut
            } else {
                DropCause::WriterDegraded
            }
        }
        _ => DropCause::WriterDegraded,
    }
}

/// Unguarded submit core for pre-built parts: slot read, then [`submit_to`].
///
/// Same contract as [`submit_to`]: call it only inside a [`submit_guarded`] closure.
pub(crate) fn submit_installed(
    parts: EventParts,
) -> Result<sc_observability_types::AdmissionOutcome, DropCause> {
    let parts = match crate::bridge::submit_parts_if_attached(parts) {
        Ok(result) => return result,
        Err(parts) => *parts,
    };
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

/// Snapshot of helper state for the canonical bridge health report.
pub(crate) fn helper_health() -> crate::HelperHealth {
    crate::HelperHealth {
        flush_in_flight: FLUSH_IN_FLIGHT.load(Ordering::SeqCst),
        detached: u64::from(DETACHED_HELPERS.load(Ordering::SeqCst)),
    }
}

// MUTEX: transfers the one-shot observer into the next flush claim; no callback runs under it.
#[cfg(test)]
static FLUSH_COMPLETION: OnceLock<Mutex<Option<mpsc::SyncSender<bool>>>> = OnceLock::new();

/// Observes whether the next claimed flush released its flag before notifying.
///
/// Register before starting the flush. The isolated test must not start another
/// flush until it receives the witness; `true` means the flag was clear at send.
#[cfg(test)]
fn notify_next_flush_complete(completed: mpsc::SyncSender<bool>) {
    *FLUSH_COMPLETION
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(completed);
}

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
pub(crate) struct Flight {
    flag: &'static AtomicBool,
    #[cfg(test)]
    completion: Option<mpsc::SyncSender<bool>>,
}

/// Claims the process-wide flush single-flight slot for an attached logger.
///
/// Attachments share the same bounded-helper accounting as an owned bridge, so
/// a timed-out attachment flush cannot accumulate helpers through retries.
pub(crate) fn claim_flush_flight() -> Option<Flight> {
    Flight::claim(&FLUSH_IN_FLIGHT)
}

impl Flight {
    /// Claims `flag` without blocking, or returns `None` while another claim is alive.
    fn claim(flag: &'static AtomicBool) -> Option<Self> {
        flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| Self {
                flag,
                #[cfg(test)]
                completion: if std::ptr::eq(flag, &raw const FLUSH_IN_FLIGHT) {
                    FLUSH_COMPLETION
                        .get_or_init(|| Mutex::new(None))
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .take()
                } else {
                    None
                },
            })
    }
}

impl Drop for Flight {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
        #[cfg(test)]
        if let Some(completed) = self.completion.take() {
            // Capture at the notification site: an early notification must not
            // pass just because the receiver happens to run after the clear.
            let _ = completed.send(!self.flag.load(Ordering::SeqCst));
        }
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
    #[cfg(test)]
    let fault = NEXT_BOUNDED_HELPER_FAULT.swap(0, Ordering::SeqCst);
    let spawn = move || {
        std::thread::Builder::new()
            .name("sc-observability-log-helper".to_owned())
            .spawn(move || {
                // Keep `exit` alive until the result is in the channel.  A caller
                // that loses the timeout race can then observe a completed helper
                // with `try_recv`, never with an unbounded second receive.
                let _exit = exit;
                #[cfg(test)]
                assert!(
                    fault != BOUNDED_HELPER_PANIC,
                    "injected bounded helper loss"
                );
                #[cfg(test)]
                if fault == BOUNDED_HELPER_BLOCK {
                    NEXT_BOUNDED_HELPER_BLOCK
                        .get_or_init(|| Mutex::new(None))
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .take()
                        .expect("bounded helper block receiver")
                        .recv()
                        .expect("bounded helper block release");
                }
                let value = work();
                let _ = tx.send(value);
            })
    };
    #[cfg(test)]
    let spawn_result = if fault == BOUNDED_HELPER_SPAWN_FAILURE {
        Err(std::io::Error::other(
            "injected bounded helper spawn failure",
        ))
    } else {
        spawn()
    };
    #[cfg(not(test))]
    let spawn_result = spawn();
    spawn_result.map_err(|source| BoundedError::Spawn { source })?;
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
/// `ShutdownError::Timeout` while the detached helper keeps waiting. When it
/// completes late it stores the final health report and publishes
/// `BridgeLifecycle::Stopped`, so the late completion is observable.
pub(crate) fn shutdown_installed(
    installed: Arc<Installed>,
    timeout: Duration,
) -> Result<(), ShutdownError> {
    let Some(coordinator) = shutdown_coordinator() else {
        let source =
            std::io::Error::other("shutdown coordinator was not reserved at initialization");
        save_unconfirmed(UnconfirmedShutdown::HelperSpawn {
            diagnostic: diagnostic(
                crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
                source.to_string(),
            ),
        });
        return Err(crate::error::shutdown_drain_as(
            crate::error::operation_context_with_source(
                crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
                source.to_string(),
                crate::Remediation::not_recoverable(
                    "inspect resource availability and retained shutdown health",
                ),
                source,
            ),
            FailureClassification::Unavailable,
        ));
    };
    let work = coordinator.work.clone();
    let (result_tx, result_rx) = mpsc::sync_channel(1);
    let command = shutdown_command(installed, result_tx);
    if let Err(error) = work.send(command) {
        let source = std::io::Error::other(error.to_string());
        save_unconfirmed(UnconfirmedShutdown::HelperSpawn {
            diagnostic: diagnostic(
                crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
                source.to_string(),
            ),
        });
        return Err(crate::error::shutdown_drain_as(
            crate::error::operation_context_with_source(
                crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
                source.to_string(),
                crate::Remediation::not_recoverable(
                    "inspect resource availability and retained shutdown health",
                ),
                source,
            ),
            FailureClassification::Unavailable,
        ));
    }
    match result_rx.recv_timeout(timeout) {
        Ok(ShutdownWorkerOutcome::Completed(Ok(()))) => Ok(()),
        Ok(ShutdownWorkerOutcome::Completed(Err(source))) => {
            Err(crate::error::shutdown_drain(source.into_context()))
        }
        Ok(ShutdownWorkerOutcome::Panicked) | Err(RecvTimeoutError::Disconnected) => {
            save_unconfirmed(UnconfirmedShutdown::HelperLost {
                diagnostic: diagnostic(
                    crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST,
                    "the reserved shutdown worker ended without a result".to_owned(),
                ),
            });
            Err(crate::error::shutdown_drain_as(
                crate::error::operation_context(
                    crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST,
                    "the reserved shutdown worker ended without a result",
                    crate::Remediation::not_recoverable(
                        "inspect retained lifecycle and health; completion is unconfirmed",
                    ),
                ),
                FailureClassification::Internal,
            ))
        }
        Err(RecvTimeoutError::Timeout) => Err(crate::error::shutdown_timeout(
            crate::error::operation_context(
                crate::error_codes::SC_OBSERVABILITY_LOG_SHUTDOWN_TIMED_OUT,
                format!("shutdown did not complete within {timeout:?}"),
                crate::Remediation::recoverable(
                    "observe the existing shutdown",
                    ["wait for completion"],
                ),
            ),
        )),
    }
}

/// `LogGuard::shutdown` and `Drop for LogGuard` both call this once.
///
/// The lifecycle is `ShuttingDown` from the first statement. When this returns
/// it is `Stopped` for every result except `ShutdownError::Timeout`, which
/// leaves `ShutdownTimedOut` until the detached helper completes and publishes
/// `Stopped` (see [`shutdown_installed`]).
pub(crate) fn shutdown_sequence(timeout: Duration) -> Result<(), ShutdownError> {
    if !begin_shutdown() {
        return Ok(());
    }
    crate::bridge::mark_owned_stopped();
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
/// An empty slot means shutdown has taken the logger and returns `FlushError::Drain`
/// with a stable not-running diagnostic. A
/// flush whose helper already holds its clone when shutdown starts is awaited by
/// the shutdown's `take_sole`, within the shutdown's own timeout.
///
/// At most one flush helper runs at a time. While one is running (also after its
/// caller timed out and detached it), a new flush spawns nothing and returns
/// `FlushError::Drain` with the stable in-progress diagnostic, so a stuck sink cannot
/// accumulate helper threads.
pub(crate) fn flush_installed(timeout: Duration) -> Result<(), FlushError> {
    #[cfg(test)]
    record_native_flush_call();
    if lifecycle() != BridgeLifecycle::Running {
        return Err(crate::error::flush_drain_as(
            crate::error::operation_context(
                crate::error_codes::SC_OBSERVABILITY_LOG_NOT_RUNNING,
                format!("logger is not running: {:?}", lifecycle_phase()),
                crate::Remediation::not_recoverable("the lifecycle owner has stopped the logger"),
            ),
            FailureClassification::Closed,
        ));
    }
    let Some(installed) = current_installed() else {
        return Err(crate::error::flush_drain_as(
            crate::error::operation_context(
                crate::error_codes::SC_OBSERVABILITY_LOG_NOT_RUNNING,
                format!("logger is not running: {:?}", lifecycle_phase()),
                crate::Remediation::not_recoverable("the lifecycle owner has stopped the logger"),
            ),
            FailureClassification::Closed,
        ));
    };
    let Some(flight) = Flight::claim(&FLUSH_IN_FLIGHT) else {
        return Err(crate::error::flush_drain_as(
            crate::error::operation_context(
                crate::error_codes::SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS,
                "a previous flush is still running; no new flush was started",
                crate::Remediation::recoverable(
                    "wait for the existing flush before retrying",
                    ["observe writer health"],
                ),
            ),
            FailureClassification::QueueFull,
        ));
    };
    let flush = move || {
        // Released when the flush returns or unwinds, before the result is sent.
        let _flight = flight;
        installed.logger.flush()
    };
    match run_bounded(timeout, flush) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(source)) => Err(crate::error::flush_drain(source.into_context())),
        Err(BoundedError::TimedOut) => Err(crate::error::flush_drain_as(
            crate::error::operation_context(
                crate::error_codes::SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT,
                format!("flush did not complete within {timeout:?}"),
                crate::Remediation::recoverable(
                    "retry flush later or raise its bounded timeout",
                    ["inspect writer health"],
                ),
            ),
            FailureClassification::timeout("flush"),
        )),
        Err(BoundedError::Spawn { source }) => Err(crate::error::flush_drain_as(
            crate::error::operation_context_with_source(
                crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED,
                source.to_string(),
                crate::Remediation::not_recoverable("inspect resource availability"),
                source,
            ),
            FailureClassification::Unavailable,
        )),
        Err(BoundedError::WorkerLost) => Err(crate::error::flush_drain_as(
            crate::error::operation_context(
                crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST,
                "the flush helper ended without a result",
                crate::Remediation::not_recoverable("inspect retained lifecycle and health"),
            ),
            FailureClassification::Internal,
        )),
    }
}

#[cfg(test)]
#[allow(
    deprecated,
    reason = "unit tests retain coverage of the released v1 facade"
)]
mod tests {
    use super::*;
    use std::sync::mpsc::sync_channel;
    use std::time::Instant;

    #[test]
    fn isolated_test_child_timeout_kills_and_reaps() {
        const CHILD_ENV: &str = "SC_OBSERVABILITY_LOG_ISOLATED_TEST_TIMEOUT_CHILD";
        const TEST_NAME: &str = "handle::tests::isolated_test_child_timeout_kills_and_reaps";
        if std::env::var_os(CHILD_ENV).is_some() {
            std::thread::sleep(Duration::from_secs(60));
            return;
        }

        let mut command =
            std::process::Command::new(std::env::current_exe().expect("test executable"));
        command
            .args(["--exact", TEST_NAME, "--nocapture", "--test-threads=1"])
            .env(CHILD_ENV, "1");
        let started = Instant::now();
        let run = run_isolated_test_child(&mut command, Duration::from_millis(100));
        let (elapsed, kill_error) = match run {
            IsolatedTestChildRun::TimedOut {
                elapsed,
                kill_error,
                ..
            } => (elapsed, kill_error),
            other @ IsolatedTestChildRun::Completed(_) => {
                panic!("hung isolated test child must time out: {other:?}")
            }
        };
        assert!(
            kill_error.is_none(),
            "timed-out child must be killed: {kill_error:?}"
        );
        assert!(
            elapsed < Duration::from_secs(1),
            "deadline was not bounded: {elapsed:?}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "parent must return after killing and reaping the timed-out child"
        );
    }

    #[test]
    #[cfg(feature = "v1")]
    fn coordinator_failure_returns_runtime_start_before_global_install() {
        if !is_isolated_test_child(
            "handle::tests::coordinator_failure_returns_runtime_start_before_global_install",
        ) {
            return;
        }
        let root = tempfile::tempdir().expect("temporary log root");
        let options = crate::BridgeOptions {
            default_action: crate::ActionName::new("log.record").expect("action"),
            parse_bracket_action: false,
        };
        let mut failed = crate::LoggerConfig::default_for(
            crate::ServiceName::new("runtime-start").expect("service"),
            root.path().to_path_buf(),
        );
        failed.level = crate::LevelFilter::Info;
        failed.enable_console_sink = false;
        fail_next_shutdown_coordinator_reservation();
        let error = crate::init(failed, options.clone()).expect_err("injected startup failure");
        assert_eq!(
            error.code().as_str(),
            "SC_OBSERVABILITY_LOG_RUNTIME_START_FAILED"
        );

        let mut retry = crate::LoggerConfig::default_for(
            crate::ServiceName::new("runtime-start").expect("service"),
            root.path().to_path_buf(),
        );
        retry.level = crate::LevelFilter::Info;
        retry.enable_console_sink = false;
        crate::v2::init(retry, options)
            .expect("retry starts")
            .shutdown_with_timeout(Duration::from_secs(5))
            .expect("retry shuts down");
    }

    #[test]
    fn health_snapshot_failure_notifies_and_retains_unavailable_for_all_controls() {
        if !is_isolated_test_child(
            "handle::tests::health_snapshot_failure_notifies_and_retains_unavailable_for_all_controls",
        ) {
            return;
        }
        let root = tempfile::tempdir().expect("temporary log root");
        let mut config = crate::LoggerConfig::default_for(
            crate::ServiceName::new("shutdown-snapshot-unavailable").expect("service"),
            root.path().to_path_buf(),
        );
        config.level = crate::LevelFilter::Info;
        config.enable_console_sink = false;
        let guard = crate::v2::init(
            config,
            crate::BridgeOptions {
                default_action: crate::ActionName::new("log.record").expect("action"),
                parse_bracket_action: false,
            },
        )
        .expect("starts");
        let first_control = guard.control();
        let second_control = first_control.clone();

        let (save_entered_tx, save_entered_rx) = sync_channel(1);
        let (save_release_tx, save_release_rx) = sync_channel(1);
        block_next_shutdown_save(save_entered_tx, save_release_rx);
        health::fail_next_health_snapshot();
        let (shutdown_tx, shutdown_rx) = sync_channel(1);
        let _shutdown = std::thread::spawn(move || {
            let _ = shutdown_tx.send(guard.shutdown_with_timeout(Duration::from_secs(5)));
        });
        save_entered_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("shutdown reaches terminal save gate");

        let (wait_entered_tx, wait_entered_rx) = sync_channel(1);
        notify_next_wait_stopped(wait_entered_tx);
        let (waiter_tx, waiter_rx) = sync_channel(1);
        let _waiter = std::thread::spawn(move || {
            let _ = waiter_tx.send(first_control.wait_stopped(Duration::from_secs(5)));
        });
        wait_entered_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("control begins waiting");

        save_release_tx.send(()).expect("release terminal save");
        let first = waiter_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("waiting control notified promptly");
        shutdown_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("shutdown completes")
            .expect("terminal shutdown result");

        let second = second_control.wait_stopped(Duration::ZERO);
        let first_diagnostic = match first {
            Err(crate::WaitError::Unavailable { diagnostic }) => diagnostic,
            other => panic!("expected retained unavailable result, got {other:?}"),
        };
        let second_diagnostic = match second {
            Err(crate::WaitError::Unavailable { diagnostic }) => diagnostic,
            other => panic!("expected repeated unavailable result, got {other:?}"),
        };
        assert_eq!(
            first_diagnostic.code.as_str(),
            "SC_OBSERVABILITY_LOG_STATUS_UNAVAILABLE"
        );
        assert_eq!(
            serde_json::to_value(first_diagnostic).expect("serialize first diagnostic"),
            serde_json::to_value(second_diagnostic).expect("serialize second diagnostic"),
            "repeated wait_stopped retains the terminal diagnostic"
        );
    }

    #[test]
    fn flush_completion_observer_confirms_release_before_notification() {
        FLUSH_IN_FLIGHT.store(false, Ordering::SeqCst);
        let (completed_tx, completed_rx) = sync_channel(1);
        notify_next_flush_complete(completed_tx);
        let flight = Flight::claim(&FLUSH_IN_FLIGHT).expect("claim flush flight");
        assert!(FLUSH_IN_FLIGHT.load(Ordering::SeqCst));
        drop(flight);
        assert!(
            completed_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("completion witness"),
            "the flight flag is clear before observer notification"
        );
        assert!(!FLUSH_IN_FLIGHT.load(Ordering::SeqCst));
    }

    #[test]
    fn shared_level_gate_covers_every_level_and_filter() {
        let levels = [
            Level::Trace,
            Level::Debug,
            Level::Info,
            Level::Warn,
            Level::Error,
        ];
        for (filter, admitted) in [
            (LevelFilter::Off, [false, false, false, false, false]),
            (LevelFilter::Error, [false, false, false, false, true]),
            (LevelFilter::Warn, [false, false, false, true, true]),
            (LevelFilter::Info, [false, false, true, true, true]),
            (LevelFilter::Debug, [false, true, true, true, true]),
            (LevelFilter::Trace, [true, true, true, true, true]),
        ] {
            for (level, expected) in levels.into_iter().zip(admitted) {
                assert_eq!(
                    level_enabled(level, filter),
                    expected,
                    "{level:?} / {filter:?}"
                );
            }
        }
    }

    #[test]
    fn log_level_rank_ordering_covers_every_filter_pair() {
        let filters = [
            LevelFilter::Trace,
            LevelFilter::Debug,
            LevelFilter::Info,
            LevelFilter::Warn,
            LevelFilter::Error,
            LevelFilter::Off,
        ];
        for (requested, available) in [
            (
                LevelFilter::Trace,
                [true, false, false, false, false, false],
            ),
            (LevelFilter::Debug, [true, true, false, false, false, false]),
            (LevelFilter::Info, [true, true, true, false, false, false]),
            (LevelFilter::Warn, [true, true, true, true, false, false]),
            (LevelFilter::Error, [true, true, true, true, true, false]),
            (LevelFilter::Off, [true, true, true, true, true, true]),
        ] {
            for (available_filter, expected) in filters.into_iter().zip(available) {
                assert_eq!(
                    log_level_rank(requested) >= log_level_rank(available_filter),
                    expected,
                    "requested {requested:?} / available {available_filter:?}"
                );
            }
        }
    }

    #[test]
    fn native_flush_counter_positive_control_and_facade_zero_proof() {
        reset_native_flush_calls();
        let result = flush_installed(Duration::from_millis(1));
        assert!(matches!(result, Err(FlushError::Drain { .. })));
        assert_eq!(
            result.unwrap_err().diagnostic().code,
            crate::error_codes::SC_OBSERVABILITY_LOG_NOT_RUNNING
        );
        assert_eq!(native_flush_calls(), 1);

        reset_native_flush_calls();
        log::Log::flush(&crate::bridge::Bridge);
        assert_eq!(
            native_flush_calls(),
            0,
            "facade flush must not invoke the native flush boundary"
        );
    }

    /// The real native flush producer owns helper spawn/loss classification.
    /// Run alone because it installs the process-global slot.
    #[test]
    fn native_flush_classifies_real_helper_spawn_and_loss() {
        const CHILD_ENV: &str = "SC_OBSERVABILITY_LOG_NATIVE_HELPER_CLASSIFICATION_CHILD";
        if std::env::var_os(CHILD_ENV).is_some() {
            native_flush_classifies_real_helper_spawn_and_loss_in_child();
            return;
        }
        let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "handle::tests::native_flush_classifies_real_helper_spawn_and_loss",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD_ENV, "1")
            .output()
            .expect("spawn isolated native helper classification regression");
        assert!(
            output.status.success(),
            "isolated native helper regression failed: {output:?}"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout)
                .matches("running 1 test")
                .count(),
            1,
            "isolated native helper regression must execute exactly one test: {output:?}"
        );
    }

    fn native_flush_classifies_real_helper_spawn_and_loss_in_child() {
        let root = tempfile::tempdir().expect("temporary log root");
        let service = sc_observability_types::ServiceName::new("native-helper-classification")
            .expect("service name");
        let logger =
            sc_observability::v2::Logger::new(sc_observability::v2::LoggerConfig::default_for(
                service.clone(),
                root.path().to_path_buf(),
            ))
            .expect("host logger");
        let installed = Arc::new(Installed {
            logger: Arc::new(logger),
            service,
            identity: sc_observability_types::ProcessIdentity::default(),
            options: crate::BridgeOptions {
                default_action: sc_observability_types::ActionName::new("log.record")
                    .expect("action name"),
                parse_bracket_action: false,
            },
        });
        *SLOT.write().unwrap_or_else(PoisonError::into_inner) = Some(Arc::clone(&installed));
        set_lifecycle(BridgeLifecycle::Running);

        fail_next_bounded_helper_spawn();
        let spawn = flush_installed(Duration::from_secs(1)).expect_err("injected spawn failure");
        assert_eq!(
            spawn.diagnostic().code,
            crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_SPAWN_FAILED
        );
        assert_eq!(
            spawn.failure_classification(),
            FailureClassification::Unavailable
        );
        assert!(
            std::error::Error::source(&spawn).is_some(),
            "io source is retained"
        );

        panic_next_bounded_helper();
        let lost = flush_installed(Duration::from_secs(1)).expect_err("injected helper loss");
        assert_eq!(
            lost.diagnostic().code,
            crate::error_codes::SC_OBSERVABILITY_LOG_HELPER_LOST
        );
        assert_eq!(
            lost.failure_classification(),
            FailureClassification::Internal
        );

        let release = block_next_bounded_helper();
        let timed_out = flush_installed(Duration::from_millis(10)).expect_err("blocked helper");
        assert_eq!(
            timed_out.diagnostic().code,
            crate::error_codes::SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT
        );
        assert_eq!(
            timed_out.failure_classification(),
            FailureClassification::timeout("flush")
        );
        let in_progress = flush_installed(Duration::from_secs(1)).expect_err("one flight only");
        assert_eq!(
            in_progress.diagnostic().code,
            crate::error_codes::SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS
        );
        assert_eq!(
            in_progress.failure_classification(),
            FailureClassification::QueueFull
        );
        release.send(()).expect("release blocked helper");
        let deadline = Instant::now() + Duration::from_secs(1);
        while FLUSH_IN_FLIGHT.load(Ordering::SeqCst) {
            assert!(
                Instant::now() < deadline,
                "blocked helper did not release flight"
            );
            std::thread::yield_now();
        }

        set_lifecycle(BridgeLifecycle::Stopped);
        let stopped = flush_installed(Duration::from_secs(1)).expect_err("stopped lifecycle");
        assert_eq!(
            stopped.diagnostic().code,
            crate::error_codes::SC_OBSERVABILITY_LOG_NOT_RUNNING
        );
        assert_eq!(
            stopped.failure_classification(),
            FailureClassification::Closed
        );

        let _ = SLOT.write().unwrap_or_else(PoisonError::into_inner).take();
        set_lifecycle(BridgeLifecycle::Stopped);
        drop(installed);
    }

    struct ReleaseOnDrop(Option<mpsc::SyncSender<()>>);

    impl ReleaseOnDrop {
        fn release(&mut self) -> Result<(), mpsc::TrySendError<()>> {
            self.0.take().map_or(Ok(()), |sender| sender.try_send(()))
        }
    }

    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            let _ = self.release();
        }
    }

    fn await_shutdown_hook(entered: &mpsc::Receiver<()>, timeout: Duration) {
        let result = entered.recv_timeout(timeout);
        assert!(
            result.is_ok(),
            "shutdown worker did not enter its test hook within {timeout:?}: {result:?}"
        );
    }

    #[test]
    fn missing_shutdown_hook_times_out_and_releases_worker() {
        let (_entered_tx, entered_rx) = mpsc::sync_channel::<()>(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || release_rx.recv_timeout(Duration::from_secs(5)));
        let started = Instant::now();
        let unwound = std::panic::catch_unwind(move || {
            let _release = ReleaseOnDrop(Some(release_tx));
            await_shutdown_hook(&entered_rx, Duration::from_millis(10));
        });
        assert!(unwound.is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(matches!(worker.join(), Ok(Ok(()))));
    }

    #[test]
    fn shutdown_test_release_survives_assertion_unwind() {
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || release_rx.recv_timeout(Duration::from_secs(5)));
        let unwound = std::panic::catch_unwind(move || {
            let _release = ReleaseOnDrop(Some(release_tx));
            panic!("injected assertion before normal release");
        });
        assert!(unwound.is_err());
        assert!(matches!(worker.join(), Ok(Ok(()))));
    }

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
        if !is_isolated_test_child(
            "handle::tests::reserved_shutdown_worker_publishes_failure_after_a_waiter_times_out",
        ) {
            return;
        }
        let root = std::env::temp_dir().join(format!("bp3-worker-{}", std::process::id()));
        let service = sc_observability_types::ServiceName::new("bp3-worker");
        assert!(service.is_ok());
        let Some(service) = service.ok() else {
            return;
        };
        let mut config = sc_observability::v2::LoggerConfig::default_for(service.clone(), root);
        config.enable_console_sink = false;
        let logger = sc_observability::v2::Logger::new(config);
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
            logger: Arc::new(logger),
            service,
            identity: sc_observability_types::ProcessIdentity::default(),
            options: crate::BridgeOptions {
                default_action: action,
                parse_bracket_action: false,
            },
        });
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let mut release = ReleaseOnDrop(Some(release_tx));
        *SHUTDOWN_WORK_HOOK
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(Box::new(move || {
            let _ = entered_tx.send(());
            let _ = release_rx.recv_timeout(Duration::from_secs(5));
            panic!("test worker failure");
        }));
        assert!(reserve_shutdown_coordinator().is_ok());
        assert!(begin_shutdown());
        set_lifecycle(BridgeLifecycle::ShuttingDown);
        let owner =
            std::thread::spawn(move || shutdown_installed(installed, Duration::from_millis(1)));
        await_shutdown_hook(&entered_rx, Duration::from_secs(1));
        assert!(matches!(
            wait_stopped(Duration::from_millis(1)),
            Err(crate::WaitError::TimedOut { .. })
        ));
        assert!(matches!(
            owner.join(),
            Ok(Err(ShutdownError::Timeout { .. }))
        ));
        assert!(release.release().is_ok());
        assert!(matches!(
            wait_stopped(Duration::from_secs(1)),
            Ok(ShutdownReport {
                outcome: ShutdownOutcome::Unconfirmed { .. },
                ..
            })
        ));
        assert_eq!(lifecycle(), BridgeLifecycle::Failed);
        let error = crate::v2::LogControl::new()
            .flush_with_timeout(Duration::ZERO)
            .expect_err("failed lifecycle rejects flush");
        assert_eq!(
            error.diagnostic().code.as_str(),
            crate::error_codes::SC_OBSERVABILITY_LOG_NOT_RUNNING.as_str()
        );
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
        if !is_isolated_test_child("handle::tests::emit_core_counts_panics_and_reentry") {
            return;
        }
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

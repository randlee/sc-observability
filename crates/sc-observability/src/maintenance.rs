use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use sc_observability_types::{
    DiagnosticSummary, ErrorContext, MaintenanceHealthReport, MaintenanceWorkerState, Remediation,
    Timestamp,
};

use crate::sinks::JsonlFileSink;
use crate::{RetainedLogPolicy, error_codes};

/// Per-pass retained-log maintenance counters recorded by the worker.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct MaintenancePassStats {
    pub(crate) rotated_files: u64,
    pub(crate) pruned_files: u64,
}

/// Background retained-log maintenance runtime owned by one `Logger`.
pub(crate) struct MaintenanceRuntime {
    tracker: Arc<MaintenanceTracker>,
    signal: Arc<MaintenanceSignal>,
    join_handle: Mutex<Option<JoinHandle<()>>>,
    done_rx: Mutex<Option<mpsc::Receiver<()>>>,
    join_timeout: Duration,
}

impl MaintenanceRuntime {
    /// Spawns the retained-log maintenance worker for one file sink.
    ///
    /// # Panics
    ///
    /// Panics if the worker thread cannot be spawned.
    pub(crate) fn new(sink: Arc<JsonlFileSink>, policy: RetainedLogPolicy) -> Self {
        let tracker = Arc::new(MaintenanceTracker::new());
        let signal = Arc::new(MaintenanceSignal::default());
        let (done_tx, done_rx) = mpsc::channel();

        let worker_tracker = tracker.clone();
        let worker_signal = signal.clone();
        #[cfg(test)]
        let test_pass_delay = policy.test_pass_delay;
        let join_handle = thread::Builder::new()
            .name("sc-observability-retained-log".to_string())
            .spawn(move || {
                retained_log_worker(
                    worker_tracker,
                    worker_signal,
                    sink,
                    policy,
                    done_tx,
                    #[cfg(test)]
                    test_pass_delay,
                );
            })
            .expect("retained-log worker thread should spawn");

        Self {
            tracker,
            signal,
            join_handle: Mutex::new(Some(join_handle)),
            done_rx: Mutex::new(Some(done_rx)),
            join_timeout: policy.maintenance_join_timeout,
        }
    }

    /// Returns the current retained-log maintenance health snapshot.
    ///
    /// # Panics
    ///
    /// Panics if internal maintenance state has been poisoned.
    pub(crate) fn snapshot(&self) -> MaintenanceHealthReport {
        self.tracker.snapshot()
    }

    /// Requests worker shutdown and waits up to the configured join timeout.
    ///
    /// Returns a diagnostic summary when shutdown degrades because the worker
    /// panicked, timed out, or disconnected before the join completed.
    ///
    /// # Panics
    ///
    /// Panics if the worker coordination mutexes have been poisoned.
    pub(crate) fn shutdown(&self) -> Option<DiagnosticSummary> {
        self.signal.request_stop();

        let done = self
            .done_rx
            .lock()
            .expect("maintenance done receiver poisoned")
            .take();
        let handle = self
            .join_handle
            .lock()
            .expect("maintenance join handle poisoned")
            .take();

        let done_rx = done?;
        let join_handle = handle?;

        match done_rx.recv_timeout(self.join_timeout) {
            Ok(()) => match join_handle.join() {
                Ok(()) => None,
                Err(_) => Some(self.tracker.record_worker_failure(
                    error_codes::LOGGER_MAINTENANCE_WORKER_FAILED,
                    "retained-log maintenance worker panicked during shutdown",
                )),
            },
            Err(mpsc::RecvTimeoutError::Timeout) => Some(self.tracker.record_join_timeout(
                self.join_timeout,
                error_codes::LOGGER_MAINTENANCE_JOIN_TIMEOUT,
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => Some(self.tracker.record_worker_failure(
                error_codes::LOGGER_MAINTENANCE_WORKER_FAILED,
                "retained-log maintenance worker completion channel disconnected",
            )),
        }
    }
}

#[derive(Default)]
struct MaintenanceSignal {
    state: Mutex<SignalState>,
    condvar: Condvar,
}

#[derive(Default)]
struct SignalState {
    stop_requested: bool,
    pass_requested: bool,
}

impl MaintenanceSignal {
    fn request_stop(&self) {
        let mut state = self.state.lock().expect("maintenance signal poisoned");
        state.stop_requested = true;
        state.pass_requested = true;
        self.condvar.notify_one();
    }

    fn wait_for_work(&self, cadence: Duration) -> bool {
        let mut state = self.state.lock().expect("maintenance signal poisoned");
        if !state.stop_requested && !state.pass_requested {
            let (next_state, _) = self
                .condvar
                .wait_timeout(state, cadence)
                .expect("maintenance signal wait poisoned");
            state = next_state;
        }

        let should_stop = state.stop_requested;
        state.pass_requested = false;
        should_stop
    }
}

#[derive(Debug)]
struct MaintenanceTracker {
    last_pass_at: Mutex<Option<Timestamp>>,
    last_error: Mutex<Option<DiagnosticSummary>>,
    state: Mutex<MaintenanceWorkerState>,
    rotated_files_total: AtomicU64,
    pruned_files_total: AtomicU64,
    join_timeout_recorded: AtomicBool,
}

impl MaintenanceTracker {
    fn new() -> Self {
        Self {
            last_pass_at: Mutex::new(None),
            last_error: Mutex::new(None),
            state: Mutex::new(MaintenanceWorkerState::Running),
            rotated_files_total: AtomicU64::new(0),
            pruned_files_total: AtomicU64::new(0),
            join_timeout_recorded: AtomicBool::new(false),
        }
    }

    fn snapshot(&self) -> MaintenanceHealthReport {
        MaintenanceHealthReport {
            state: *self.state.lock().expect("maintenance state poisoned"),
            last_pass_at: *self
                .last_pass_at
                .lock()
                .expect("maintenance last_pass_at poisoned"),
            rotated_files_total: self.rotated_files_total.load(Ordering::SeqCst),
            pruned_files_total: self.pruned_files_total.load(Ordering::SeqCst),
            last_error: self
                .last_error
                .lock()
                .expect("maintenance last_error poisoned")
                .clone(),
        }
    }

    fn record_pass(&self, stats: MaintenancePassStats) {
        self.rotated_files_total
            .fetch_add(stats.rotated_files, Ordering::SeqCst);
        self.pruned_files_total
            .fetch_add(stats.pruned_files, Ordering::SeqCst);
        *self
            .last_pass_at
            .lock()
            .expect("maintenance last_pass_at poisoned") = Some(Timestamp::now_utc());
        if !self.join_timeout_recorded.load(Ordering::SeqCst) {
            *self.state.lock().expect("maintenance state poisoned") =
                MaintenanceWorkerState::Running;
        }
    }

    fn record_failure(&self, error: &ErrorContext) -> DiagnosticSummary {
        let summary = DiagnosticSummary::from(error.diagnostic());
        *self.state.lock().expect("maintenance state poisoned") = MaintenanceWorkerState::Degraded;
        *self
            .last_error
            .lock()
            .expect("maintenance last_error poisoned") = Some(summary.clone());
        summary
    }

    fn record_join_timeout(
        &self,
        timeout: Duration,
        code: sc_observability_types::ErrorCode,
    ) -> DiagnosticSummary {
        self.join_timeout_recorded.store(true, Ordering::SeqCst);
        let summary = DiagnosticSummary::from(
            ErrorContext::new(
                code,
                format!(
                    "retained-log maintenance worker did not stop within {}ms",
                    timeout.as_millis()
                ),
                Remediation::recoverable(
                    "inspect logger shutdown sequencing",
                    [
                        "increase maintenance_join_timeout",
                        "review retained-log maintenance load",
                    ],
                ),
            )
            .diagnostic(),
        );
        *self.state.lock().expect("maintenance state poisoned") = MaintenanceWorkerState::Degraded;
        *self
            .last_error
            .lock()
            .expect("maintenance last_error poisoned") = Some(summary.clone());
        summary
    }

    fn record_worker_failure(
        &self,
        code: sc_observability_types::ErrorCode,
        message: &str,
    ) -> DiagnosticSummary {
        let summary = DiagnosticSummary::from(
            ErrorContext::new(
                code,
                message,
                Remediation::recoverable(
                    "restart the logger runtime",
                    [
                        "inspect retained-log worker failures",
                        "collect worker panic context",
                    ],
                ),
            )
            .diagnostic(),
        );
        *self.state.lock().expect("maintenance state poisoned") = MaintenanceWorkerState::Degraded;
        *self
            .last_error
            .lock()
            .expect("maintenance last_error poisoned") = Some(summary.clone());
        summary
    }

    fn mark_stopped(&self) {
        if !self.join_timeout_recorded.load(Ordering::SeqCst) {
            *self.state.lock().expect("maintenance state poisoned") =
                MaintenanceWorkerState::Stopped;
        }
    }
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "the worker thread takes ownership of its handles and Arcs for the full spawned lifetime"
)]
fn retained_log_worker(
    tracker: Arc<MaintenanceTracker>,
    signal: Arc<MaintenanceSignal>,
    sink: Arc<JsonlFileSink>,
    policy: RetainedLogPolicy,
    done_tx: mpsc::Sender<()>,
    #[cfg(test)] test_pass_delay: Option<Duration>,
) {
    loop {
        let should_stop = signal.wait_for_work(policy.maintenance_cadence);
        maybe_run_test_delay(
            #[cfg(test)]
            test_pass_delay,
        );
        match sink.perform_maintenance(&policy) {
            Ok(stats) => tracker.record_pass(stats),
            Err(error) => {
                tracker.record_failure(error.0.as_ref());
            }
        }

        if should_stop {
            break;
        }
    }

    tracker.mark_stopped();
    let _ = done_tx.send(());
}

#[cfg(test)]
static TEST_PASS_DELAY_ACTIVE: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
/// Clears the shared test-only delay activity flag after maintenance timing assertions.
pub(crate) fn clear_test_pass_delay() {
    TEST_PASS_DELAY_ACTIVE.store(false, Ordering::SeqCst);
}

#[cfg(test)]
/// Returns whether any test-configured maintenance worker is currently inside its injected delay.
pub(crate) fn test_pass_delay_active() -> bool {
    TEST_PASS_DELAY_ACTIVE.load(Ordering::SeqCst)
}

#[cfg(not(test))]
fn maybe_run_test_delay() {}

#[cfg(test)]
fn maybe_run_test_delay(test_pass_delay: Option<Duration>) {
    let Some(delay) = test_pass_delay else {
        return;
    };

    TEST_PASS_DELAY_ACTIVE.store(true, Ordering::SeqCst);
    thread::sleep(delay);
    TEST_PASS_DELAY_ACTIVE.store(false, Ordering::SeqCst);
}

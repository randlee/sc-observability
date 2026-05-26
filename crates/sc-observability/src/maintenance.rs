use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use sc_observability_types::{
    DiagnosticInfo, DiagnosticSummary, ErrorContext, FileCount, FlushError,
    MaintenanceHealthReport, MaintenanceWorkerState, Remediation, Timestamp, WriterState,
};

use crate::sinks::JsonlFileSink;
use crate::{LogEvent, RetainedLogPolicy, SinkRegistration, constants, error_codes};

/// Per-pass retained-log maintenance counters recorded by the writer thread.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct MaintenancePassStats {
    pub(crate) rotated_files: u64,
    pub(crate) pruned_files: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct WriterHealthSnapshot {
    pub(crate) queue_depth: u64,
    pub(crate) queue_capacity: u64,
    pub(crate) queue_high_water_mark: u64,
    pub(crate) queue_full_drops_total: u64,
    pub(crate) writer_state: WriterState,
    pub(crate) last_writer_error: Option<DiagnosticSummary>,
    pub(crate) maintenance: Option<MaintenanceHealthReport>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TryEnqueueError {
    Full,
    Disconnected,
}

pub(crate) struct WriterRuntime {
    sender: mpsc::SyncSender<WriterCommand>,
    done_rx: Mutex<mpsc::Receiver<()>>,
    join_handle: JoinHandle<()>,
    join_timeout: Duration,
    writer_tracker: Arc<WriterTracker>,
    maintenance_tracker: Option<Arc<MaintenanceTracker>>,
}

impl WriterRuntime {
    pub(crate) fn new(
        sinks: Vec<SinkRegistration>,
        file_sink: Option<Arc<JsonlFileSink>>,
        policy: RetainedLogPolicy,
        queue_capacity: usize,
        dropped_events_total: Arc<AtomicU64>,
        last_error: Arc<Mutex<Option<DiagnosticSummary>>>,
        #[cfg(test)] test_pass_delay: Option<Duration>,
    ) -> Self {
        let writer_tracker = Arc::new(WriterTracker::new(queue_capacity));
        let maintenance_tracker = file_sink
            .as_ref()
            .map(|_| Arc::new(MaintenanceTracker::new()));
        let (sender, receiver) = mpsc::sync_channel(queue_capacity);
        let (done_tx, done_rx) = mpsc::channel();

        let worker_writer_tracker = writer_tracker.clone();
        let worker_maintenance_tracker = maintenance_tracker.clone();
        let join_handle = thread::Builder::new()
            .name("sc-observability-writer".to_string())
            .spawn(move || {
                writer_worker(
                    receiver,
                    done_tx,
                    sinks,
                    file_sink,
                    policy,
                    worker_writer_tracker,
                    worker_maintenance_tracker,
                    dropped_events_total,
                    last_error,
                    #[cfg(test)]
                    test_pass_delay,
                );
            })
            .expect("writer thread should spawn");

        Self {
            sender,
            done_rx: Mutex::new(done_rx),
            join_handle,
            join_timeout: policy.maintenance_join_timeout.as_duration(),
            writer_tracker,
            maintenance_tracker,
        }
    }

    pub(crate) fn enqueue_blocking(&self, event: LogEvent) -> Result<(), ()> {
        self.sender
            .send(WriterCommand::Log(event))
            .map_err(|_| ())?;
        self.writer_tracker.record_enqueue();
        Ok(())
    }

    pub(crate) fn enqueue_nonblocking(&self, event: LogEvent) -> Result<(), TryEnqueueError> {
        self.sender
            .try_send(WriterCommand::Log(event))
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => TryEnqueueError::Full,
                mpsc::TrySendError::Disconnected(_) => TryEnqueueError::Disconnected,
            })?;
        self.writer_tracker.record_enqueue();
        Ok(())
    }

    pub(crate) fn record_queue_full_drop(&self) -> DiagnosticSummary {
        self.writer_tracker.record_queue_full_drop()
    }

    pub(crate) fn flush(&self) -> Result<(), FlushError> {
        let (tx, rx) = mpsc::channel();
        self.sender.send(WriterCommand::Flush(tx)).map_err(|_| {
            FlushError(Box::new(crate::writer_degraded_error_context(
                "writer thread is not available for flush",
            )))
        })?;
        match rx.recv() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(summary)) => Err(FlushError(Box::new(
                ErrorContext::new(
                    error_codes::LOGGER_FLUSH_FAILED,
                    "writer flush failed",
                    Remediation::recoverable(
                        "inspect the writer-thread flush failure",
                        [
                            "inspect logger.health().last_writer_error",
                            "retry the flush after the writer recovers",
                        ],
                    ),
                )
                .cause(summary.message.clone()),
            ))),
            Err(_) => Err(FlushError(Box::new(crate::writer_degraded_error_context(
                "writer thread disconnected during flush",
            )))),
        }
    }

    pub(crate) fn shutdown(self) -> WriterHealthSnapshot {
        drop(self.sender);

        match self
            .done_rx
            .lock()
            .expect("writer done receiver poisoned")
            .recv_timeout(self.join_timeout)
        {
            Ok(()) => {
                if self.join_handle.join().is_err() {
                    self.writer_tracker
                        .record_writer_failure(&ErrorContext::new(
                            error_codes::LOGGER_WRITER_DEGRADED,
                            "writer thread panicked during shutdown",
                            Remediation::recoverable(
                                "restart the logger runtime",
                                [
                                    "inspect writer-thread panic context",
                                    "recreate the logger instance",
                                ],
                            ),
                        ));
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.writer_tracker
                    .record_shutdown_timeout(self.join_timeout);
                if let Some(tracker) = self.maintenance_tracker.as_ref() {
                    tracker.record_failure(&ErrorContext::new(
                        error_codes::LOGGER_SHUTDOWN_TIMED_OUT,
                        format!(
                            "maintenance did not stop within {}ms",
                            self.join_timeout.as_millis()
                        ),
                        Remediation::recoverable(
                            "inspect maintenance shutdown timing",
                            [
                                "inspect logger.health().maintenance",
                                "inspect logger.health().last_writer_error",
                            ],
                        ),
                    ));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.writer_tracker
                    .record_writer_failure(&ErrorContext::new(
                        error_codes::LOGGER_WRITER_DEGRADED,
                        "writer thread completion channel disconnected during shutdown",
                        Remediation::recoverable(
                            "restart the logger runtime",
                            [
                                "inspect writer-thread shutdown state",
                                "recreate the logger instance",
                            ],
                        ),
                    ));
            }
        }

        snapshot_from_trackers(&self.writer_tracker, self.maintenance_tracker.as_ref())
    }

    pub(crate) fn snapshot(&self) -> WriterHealthSnapshot {
        snapshot_from_trackers(&self.writer_tracker, self.maintenance_tracker.as_ref())
    }

    pub(crate) fn maintenance_active(&self) -> bool {
        self.maintenance_tracker
            .as_ref()
            .is_some_and(|tracker| tracker.pass_active())
    }
}

fn snapshot_from_trackers(
    writer_tracker: &WriterTracker,
    maintenance_tracker: Option<&Arc<MaintenanceTracker>>,
) -> WriterHealthSnapshot {
    WriterHealthSnapshot {
        queue_depth: writer_tracker.queue_depth(),
        queue_capacity: writer_tracker.queue_capacity(),
        queue_high_water_mark: writer_tracker.queue_high_water_mark(),
        queue_full_drops_total: writer_tracker.queue_full_drops_total(),
        writer_state: writer_tracker.state(),
        last_writer_error: writer_tracker.last_error(),
        maintenance: maintenance_tracker.map(|tracker| tracker.snapshot()),
    }
}

pub(crate) struct WriterTracker {
    queue_depth: AtomicU64,
    queue_capacity: u64,
    queue_high_water_mark: AtomicU64,
    queue_full_drops_total: AtomicU64,
    state: RwLock<WriterState>,
    last_error: RwLock<Option<DiagnosticSummary>>,
    shutdown_timeout_recorded: AtomicBool,
}

impl WriterTracker {
    fn new(queue_capacity: usize) -> Self {
        Self {
            queue_depth: AtomicU64::new(0),
            queue_capacity: queue_capacity as u64,
            queue_high_water_mark: AtomicU64::new(0),
            queue_full_drops_total: AtomicU64::new(0),
            state: RwLock::new(WriterState::Running),
            last_error: RwLock::new(None),
            shutdown_timeout_recorded: AtomicBool::new(false),
        }
    }

    fn record_enqueue(&self) {
        let depth = self.queue_depth.fetch_add(1, Ordering::SeqCst) + 1;
        self.queue_high_water_mark
            .fetch_max(depth, Ordering::SeqCst);
    }

    fn record_write_completion(&self, completed: usize) {
        self.queue_depth
            .fetch_sub(completed as u64, Ordering::SeqCst);
    }

    fn record_queue_full_drop(&self) -> DiagnosticSummary {
        self.queue_full_drops_total.fetch_add(1, Ordering::SeqCst);
        DiagnosticSummary::from(
            ErrorContext::new(
                error_codes::LOGGER_QUEUE_FULL,
                "writer queue is full",
                Remediation::recoverable(
                    "reduce logging pressure or increase queue capacity",
                    [
                        "inspect logger.health().queue_depth",
                        "inspect logger.health().queue_high_water_mark",
                    ],
                ),
            )
            .diagnostic(),
        )
    }

    fn record_writer_failure(&self, error: &ErrorContext) -> DiagnosticSummary {
        let summary = DiagnosticSummary::from(error.diagnostic());
        *self.state.write().expect("writer state poisoned") = WriterState::Degraded;
        *self.last_error.write().expect("writer last_error poisoned") = Some(summary.clone());
        summary
    }

    fn record_shutdown_timeout(&self, timeout: Duration) -> DiagnosticSummary {
        self.shutdown_timeout_recorded.store(true, Ordering::SeqCst);
        let summary = DiagnosticSummary::from(
            ErrorContext::new(
                error_codes::LOGGER_SHUTDOWN_TIMED_OUT,
                format!(
                    "writer thread did not stop within {}ms",
                    timeout.as_millis()
                ),
                Remediation::recoverable(
                    "inspect writer-thread shutdown timing",
                    [
                        "inspect logger.health().queue_depth",
                        "inspect logger.health().last_writer_error",
                    ],
                ),
            )
            .diagnostic(),
        );
        *self.state.write().expect("writer state poisoned") = WriterState::Degraded;
        *self.last_error.write().expect("writer last_error poisoned") = Some(summary.clone());
        summary
    }

    fn mark_stopped(&self) {
        if !self.shutdown_timeout_recorded.load(Ordering::SeqCst) {
            let mut state = self.state.write().expect("writer state poisoned");
            if *state == WriterState::Running {
                *state = WriterState::Stopped;
            }
        }
    }

    fn queue_depth(&self) -> u64 {
        self.queue_depth.load(Ordering::SeqCst)
    }

    fn queue_capacity(&self) -> u64 {
        self.queue_capacity
    }

    fn queue_high_water_mark(&self) -> u64 {
        self.queue_high_water_mark.load(Ordering::SeqCst)
    }

    fn queue_full_drops_total(&self) -> u64 {
        self.queue_full_drops_total.load(Ordering::SeqCst)
    }

    fn state(&self) -> WriterState {
        *self.state.read().expect("writer state poisoned")
    }

    fn last_error(&self) -> Option<DiagnosticSummary> {
        self.last_error
            .read()
            .expect("writer last_error poisoned")
            .clone()
    }
}

pub(crate) struct MaintenanceTracker {
    pass_active: AtomicBool,
    last_pass_at: RwLock<Option<Timestamp>>,
    last_error: RwLock<Option<DiagnosticSummary>>,
    state: RwLock<MaintenanceWorkerState>,
    rotated_files_total: AtomicU64,
    pruned_files_total: AtomicU64,
}

impl MaintenanceTracker {
    fn new() -> Self {
        Self {
            pass_active: AtomicBool::new(false),
            last_pass_at: RwLock::new(None),
            last_error: RwLock::new(None),
            state: RwLock::new(MaintenanceWorkerState::Running),
            rotated_files_total: AtomicU64::new(0),
            pruned_files_total: AtomicU64::new(0),
        }
    }

    pub(crate) fn snapshot(&self) -> MaintenanceHealthReport {
        MaintenanceHealthReport {
            state: *self.state.read().expect("maintenance state poisoned"),
            last_pass_at: *self
                .last_pass_at
                .read()
                .expect("maintenance last_pass_at poisoned"),
            rotated_files_total: FileCount::try_from_u64(
                self.rotated_files_total.load(Ordering::SeqCst),
            )
            .expect("maintenance rotated-files count should fit usize"),
            pruned_files_total: FileCount::try_from_u64(
                self.pruned_files_total.load(Ordering::SeqCst),
            )
            .expect("maintenance pruned-files count should fit usize"),
            last_error: self
                .last_error
                .read()
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
            .write()
            .expect("maintenance last_pass_at poisoned") = Some(Timestamp::now_utc());
        *self.state.write().expect("maintenance state poisoned") = MaintenanceWorkerState::Running;
    }

    fn mark_pass_active(&self, active: bool) {
        self.pass_active.store(active, Ordering::SeqCst);
    }

    fn pass_active(&self) -> bool {
        self.pass_active.load(Ordering::SeqCst)
    }

    fn record_failure(&self, error: &ErrorContext) {
        let summary = DiagnosticSummary::from(error.diagnostic());
        *self.state.write().expect("maintenance state poisoned") = MaintenanceWorkerState::Degraded;
        *self
            .last_error
            .write()
            .expect("maintenance last_error poisoned") = Some(summary);
    }

    fn mark_stopped(&self) {
        *self.state.write().expect("maintenance state poisoned") = MaintenanceWorkerState::Stopped;
    }
}

#[expect(
    clippy::large_enum_variant,
    reason = "the queue intentionally carries owned log events so producers can hand off complete records to the writer thread"
)]
enum WriterCommand {
    Log(LogEvent),
    Flush(mpsc::Sender<Result<(), DiagnosticSummary>>),
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "the writer thread takes ownership of its handles for the full spawned lifetime"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "the worker entry point wires queue state, sink ownership, and health trackers into one spawned runtime boundary"
)]
#[expect(
    clippy::too_many_lines,
    reason = "the worker loop intentionally keeps queue admission, flush barriers, and maintenance scheduling together to preserve shutdown ordering"
)]
fn writer_worker(
    receiver: mpsc::Receiver<WriterCommand>,
    done_tx: mpsc::Sender<()>,
    sinks: Vec<SinkRegistration>,
    file_sink: Option<Arc<JsonlFileSink>>,
    policy: RetainedLogPolicy,
    writer_tracker: Arc<WriterTracker>,
    maintenance_tracker: Option<Arc<MaintenanceTracker>>,
    dropped_events_total: Arc<AtomicU64>,
    last_error: Arc<Mutex<Option<DiagnosticSummary>>>,
    #[cfg(test)] test_pass_delay: Option<Duration>,
) {
    let mut batch = Vec::with_capacity(constants::DEFAULT_LOG_BATCH_SIZE);
    let mut pending_flush = Vec::new();
    let mut next_maintenance_at = Instant::now() + policy.maintenance_cadence.as_duration();

    loop {
        let timeout = if batch.is_empty() {
            file_sink
                .as_ref()
                .map(|_| next_maintenance_at.saturating_duration_since(Instant::now()))
        } else {
            Some(constants::DEFAULT_WRITER_BATCH_TIMEOUT)
        };

        let message_result = match timeout {
            Some(duration) => receiver.recv_timeout(duration),
            None => receiver
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
        };

        match message_result {
            Ok(command) => {
                handle_command(command, &mut batch, &mut pending_flush);
                while batch.len() < constants::DEFAULT_LOG_BATCH_SIZE {
                    match receiver.try_recv() {
                        Ok(command) => handle_command(command, &mut batch, &mut pending_flush),
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            flush_batch(
                                &mut batch,
                                &sinks,
                                &writer_tracker,
                                dropped_events_total.as_ref(),
                                last_error.as_ref(),
                                &mut pending_flush,
                            );
                            flush_sinks(
                                &sinks,
                                &writer_tracker,
                                last_error.as_ref(),
                                &mut pending_flush,
                            );
                            run_maintenance_if_due(
                                file_sink.as_ref(),
                                maintenance_tracker.as_ref(),
                                &policy,
                                &mut next_maintenance_at,
                                #[cfg(test)]
                                test_pass_delay,
                            );
                            writer_tracker.mark_stopped();
                            if let Some(tracker) = maintenance_tracker.as_ref() {
                                tracker.mark_stopped();
                            }
                            let _ = done_tx.send(());
                            return;
                        }
                    }
                }

                if batch.len() >= constants::DEFAULT_LOG_BATCH_SIZE || !pending_flush.is_empty() {
                    flush_batch(
                        &mut batch,
                        &sinks,
                        &writer_tracker,
                        dropped_events_total.as_ref(),
                        last_error.as_ref(),
                        &mut pending_flush,
                    );
                    flush_sinks(
                        &sinks,
                        &writer_tracker,
                        last_error.as_ref(),
                        &mut pending_flush,
                    );
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                flush_batch(
                    &mut batch,
                    &sinks,
                    &writer_tracker,
                    dropped_events_total.as_ref(),
                    last_error.as_ref(),
                    &mut pending_flush,
                );
                flush_sinks(
                    &sinks,
                    &writer_tracker,
                    last_error.as_ref(),
                    &mut pending_flush,
                );
                run_maintenance_if_due(
                    file_sink.as_ref(),
                    maintenance_tracker.as_ref(),
                    &policy,
                    &mut next_maintenance_at,
                    #[cfg(test)]
                    test_pass_delay,
                );
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                flush_batch(
                    &mut batch,
                    &sinks,
                    &writer_tracker,
                    dropped_events_total.as_ref(),
                    last_error.as_ref(),
                    &mut pending_flush,
                );
                flush_sinks(
                    &sinks,
                    &writer_tracker,
                    last_error.as_ref(),
                    &mut pending_flush,
                );
                run_maintenance_if_due(
                    file_sink.as_ref(),
                    maintenance_tracker.as_ref(),
                    &policy,
                    &mut next_maintenance_at,
                    #[cfg(test)]
                    test_pass_delay,
                );
                writer_tracker.mark_stopped();
                if let Some(tracker) = maintenance_tracker.as_ref() {
                    tracker.mark_stopped();
                }
                let _ = done_tx.send(());
                return;
            }
        }
    }
}

fn handle_command(
    command: WriterCommand,
    batch: &mut Vec<LogEvent>,
    pending_flush: &mut Vec<mpsc::Sender<Result<(), DiagnosticSummary>>>,
) {
    match command {
        WriterCommand::Log(event) => batch.push(event),
        WriterCommand::Flush(sender) => pending_flush.push(sender),
    }
}

fn flush_batch(
    batch: &mut Vec<LogEvent>,
    sinks: &[SinkRegistration],
    writer_tracker: &WriterTracker,
    dropped_events_total: &AtomicU64,
    last_error: &Mutex<Option<DiagnosticSummary>>,
    pending_flush: &mut Vec<mpsc::Sender<Result<(), DiagnosticSummary>>>,
) {
    if batch.is_empty() {
        if !pending_flush.is_empty() {
            flush_sinks(sinks, writer_tracker, last_error, pending_flush);
        }
        return;
    }

    for event in batch.drain(..) {
        for registration in sinks {
            if registration
                .filter
                .as_ref()
                .is_some_and(|filter| !filter.accepts(&event))
            {
                continue;
            }

            if let Err(error) = registration.sink.write(&event) {
                dropped_events_total.fetch_add(1, Ordering::SeqCst);
                let summary = writer_tracker.record_writer_failure(&ErrorContext::new(
                    error_codes::LOGGER_WRITER_DEGRADED,
                    "writer-thread sink write failed",
                    Remediation::recoverable(
                        "inspect sink health and writer runtime state",
                        [
                            "inspect logger.health().sink_statuses",
                            "inspect logger.health().last_writer_error",
                        ],
                    ),
                ));
                *last_error.lock().expect("logger last_error poisoned") =
                    Some(DiagnosticSummary::from(error.diagnostic()));
                let _ = summary;
            }
        }
        writer_tracker.record_write_completion(1);
    }

    if !pending_flush.is_empty() {
        flush_sinks(sinks, writer_tracker, last_error, pending_flush);
    }
}

fn flush_sinks(
    sinks: &[SinkRegistration],
    writer_tracker: &WriterTracker,
    last_error: &Mutex<Option<DiagnosticSummary>>,
    pending_flush: &mut Vec<mpsc::Sender<Result<(), DiagnosticSummary>>>,
) {
    let mut first_error = None;

    for registration in sinks {
        if let Err(error) = registration.sink.flush() {
            if first_error.is_none() {
                first_error = Some(DiagnosticSummary::from(error.diagnostic()));
            }
            let _ = writer_tracker.record_writer_failure(&ErrorContext::new(
                error_codes::LOGGER_WRITER_DEGRADED,
                "writer-thread sink flush failed",
                Remediation::recoverable(
                    "inspect sink health and retry the flush after recovery",
                    [
                        "inspect logger.health().sink_statuses",
                        "inspect logger.health().last_writer_error",
                    ],
                ),
            ));
            *last_error.lock().expect("logger last_error poisoned") =
                Some(DiagnosticSummary::from(error.diagnostic()));
        }
    }

    for sender in pending_flush.drain(..) {
        let _ = sender.send(match &first_error {
            Some(summary) => Err(summary.clone()),
            None => Ok(()),
        });
    }
}

fn run_maintenance_if_due(
    file_sink: Option<&Arc<JsonlFileSink>>,
    maintenance_tracker: Option<&Arc<MaintenanceTracker>>,
    policy: &RetainedLogPolicy,
    next_maintenance_at: &mut Instant,
    #[cfg(test)] test_pass_delay: Option<Duration>,
) {
    let Some(file_sink) = file_sink else {
        return;
    };

    if Instant::now() < *next_maintenance_at {
        return;
    }

    if let Some(tracker) = maintenance_tracker {
        tracker.mark_pass_active(true);
        maybe_run_test_delay(
            #[cfg(test)]
            test_pass_delay,
        );
        match file_sink.perform_maintenance(policy) {
            Ok(stats) => tracker.record_pass(stats),
            Err(error) => tracker.record_failure(error.0.as_ref()),
        }
        tracker.mark_pass_active(false);
    } else {
        maybe_run_test_delay(
            #[cfg(test)]
            test_pass_delay,
        );
    }

    *next_maintenance_at = Instant::now() + policy.maintenance_cadence.as_duration();
}

#[cfg(test)]
static TEST_PASS_DELAY_ACTIVE: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
pub(crate) fn clear_test_pass_delay() {
    TEST_PASS_DELAY_ACTIVE.store(false, Ordering::SeqCst);
}

#[cfg(test)]
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

//! Durable, at-least-once telemetry admission and draining.
//!
//! Receipts follow a FULL-synchronous `SQLite` commit. After a crash or lease loss,
//! at most one in-flight batch per signal per takeover may be sent again; this is
//! not an exactly-once transport. Disk accounting measures retained envelope bytes.
//! Eviction clears payloads but retains delivery tombstones, and keyed receipts
//! survive payload retention until the record-key retention period ends.
#![allow(
    clippy::result_large_err,
    reason = "approved errors preserve the inline four-signal FlushReport"
)]
mod adapter;
mod config_file;
mod database;
mod query;
mod row;
mod store;
#[cfg(test)]
mod tests;
mod worker;
use crate::contracts::{credits::AdmissionCredits, submission::SubmissionExporter};
pub use config_file::load_telemetry_file;
use sc_lint_attributes::sc_lint;
use sc_observability_types::otlp::submission::{
    AdmissionError, AdmissionReceipt, FlushReport, StatusQuery, StoreStatus, SubmissionEnvelope,
    SubmissionId, TelemetryClient, TelemetryClientConfig, TelemetryClientError, error_codes,
};
use sc_observability_types::{ErrorCode, ErrorContext, Remediation};
use std::{
    sync::{
        Arc, Condvar, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// Durable client. Dropping a handle stops its workers; explicit shutdown also flushes.
/// Operation deadlines bound coordination and `SQLite` busy waits; filesystem calls
/// are subject to the operating system and cannot be forcibly interrupted here.
/// Unsupported backends are rejected by `open` before any submission exists;
/// the frozen capability error uses Logs/Log as representative fields.
#[derive(Debug)]
pub struct DurableTelemetryClient {
    owner: Owner,
}
struct Owner {
    shared: Arc<Shared>,
    exporter: Option<Arc<dyn SubmissionExporter>>,
}
impl std::fmt::Debug for Owner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Owner")
            .field("shared", &self.shared)
            .field(
                "exporter",
                &self.exporter.as_ref().map(|_| "submission exporter"),
            )
            .finish()
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        if let Some(exporter) = &self.exporter {
            exporter.cancel();
        }
        self.shared.stop.store(true, Ordering::Release);
        self.shared.notify();
    }
}
// A mutex serializes short SQLite transactions. Export and waits never hold it.
#[derive(Debug)]
struct Shared {
    db: database::Database,
    config: TelemetryClientConfig,
    holder: row::LeaseHolder,
    // Acquire reads reject operations; the AcqRel swap linearizes concurrent shutdown.
    closed: AtomicBool,
    // Shutdown/drop stores Release; worker wait predicates observe it with Acquire.
    stop: AtomicBool,
    // AdmissionCredits protects its counters with its internal mutex and condition variable.
    credits: AdmissionCredits,
    // The generation is mutex-guarded so a waiter cannot miss a notification.
    wake: Mutex<u64>,
    // Always paired with `wake`; waiters recheck the generation and `stop` predicate.
    changed: Condvar,
    // Last background failure is retained and emitted as a code-only diagnostic.
    last_error: Mutex<Option<ErrorCode>>,
    // Handles are owned by the client; completion count and wake share one mutex
    // so shutdown cannot miss the last worker notification.
    workers: Mutex<Vec<std::thread::JoinHandle<()>>>,
    live_workers: std::sync::atomic::AtomicUsize,
    #[cfg(test)]
    waiting_for_credits:
        Mutex<std::collections::HashSet<sc_observability_types::otlp::submission::Signal>>,
    #[cfg(test)]
    drain_on_flush_only: AtomicBool,
    #[cfg(test)]
    active_flushes: std::sync::atomic::AtomicUsize,
    #[cfg(test)]
    stalled_signals:
        Mutex<std::collections::HashSet<sc_observability_types::otlp::submission::Signal>>,
}
impl Shared {
    fn record_error(&self, error: &TelemetryClientError) {
        use std::io::Write;
        let code = error.code().clone();
        let mut last = self
            .last_error
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if last.as_ref() != Some(&code) {
            let _ = writeln!(
                std::io::stderr().lock(),
                "durable telemetry worker error: {code}"
            );
        }
        *last = Some(code);
        drop(last);
        self.notify();
    }
    fn notify(&self) {
        let mut generation = self.wake.lock().unwrap_or_else(PoisonError::into_inner);
        *generation = generation.wrapping_add(1);
        self.changed.notify_all();
    }
    fn generation(&self) -> u64 {
        *self.wake.lock().unwrap_or_else(PoisonError::into_inner)
    }
    fn wait(&self, duration: Duration) {
        // Heartbeat/error pacing ignores admission notifications, but stops promptly.
        let wake = self.wake.lock().unwrap_or_else(PoisonError::into_inner);
        let _guard = self
            .changed
            .wait_timeout_while(wake, duration, |_| !self.stop.load(Ordering::Acquire))
            .unwrap_or_else(PoisonError::into_inner);
    }
    fn wait_since(&self, before: u64, duration: Duration) {
        let generation = self.wake.lock().unwrap_or_else(PoisonError::into_inner);
        let _guard = self
            .changed
            .wait_timeout_while(generation, duration, |g| {
                *g == before && !self.stop.load(Ordering::Acquire)
            })
            .unwrap_or_else(PoisonError::into_inner);
    }
    fn poll_interval() -> Duration {
        Duration::from_millis(crate::constants::DRAIN_POLL_INTERVAL_MS)
    }
}
fn context(code: ErrorCode, message: &str) -> Box<ErrorContext> {
    let (action, step) = match code.as_str() {
        "SC_OBSERVABILITY_ADMIT_CLOSED" => (
            "open a new client",
            "reuse the same store to resume retained deliveries",
        ),
        "SC_OBSERVABILITY_DURABLE_RECORD_TOO_LARGE" => (
            "split the submission",
            "keep each canonical envelope within the drain byte budget",
        ),
        "SC_OBSERVABILITY_DURABLE_CORRUPT_ENVELOPE" => (
            "inspect the failed submission",
            "restore it from the original source; later valid submissions continue draining",
        ),
        "SC_OBSERVABILITY_DURABLE_UNSUPPORTED_QUERY" => (
            "use a supported status query",
            "select Summary or Submissions with typed submission identifiers",
        ),
        "SC_OBSERVABILITY_DURABLE_LOCK_TIMEOUT" => (
            "retry after the active store transaction completes",
            "use a deadline appropriate for concurrent store access",
        ),
        "SC_OBSERVABILITY_TELEMETRY_UNSUPPORTED" => (
            "select the sync_http backend",
            "the durable client rejects unsupported backends before opening the store",
        ),
        "SC_OBSERVABILITY_TELEMETRY_CONFIG_FILE" => (
            "correct the telemetry file",
            "check its path, permissions, UTF-8 YAML syntax and size limit",
        ),
        "SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID" => (
            "correct the named configuration field",
            "use a value within the documented field bounds",
        ),
        "SC_OBSERVABILITY_ADMIT_SCHEMA_TOO_NEW" => (
            "use a client supporting this store version",
            "retain the database; do not downgrade or delete pending records",
        ),
        "SC_OBSERVABILITY_ADMIT_DISK_BOUND" => (
            "drain retained submissions or increase max_store_bytes",
            "retry admission after sufficient payload capacity becomes available",
        ),
        _ => (
            "check store accessibility and the reported database failure",
            "retain the store for recovery; do not delete pending records",
        ),
    };
    let remediation = match code.as_str() {
        "SC_OBSERVABILITY_DURABLE_RECORD_TOO_LARGE"
        | "SC_OBSERVABILITY_DURABLE_CORRUPT_ENVELOPE"
        | "SC_OBSERVABILITY_DURABLE_UNSUPPORTED_QUERY"
        | "SC_OBSERVABILITY_ADMIT_SCHEMA_TOO_NEW"
        | "SC_OBSERVABILITY_TELEMETRY_CONFIG_FILE"
        | "SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID"
        | "SC_OBSERVABILITY_TELEMETRY_UNSUPPORTED" => {
            Remediation::not_recoverable(format!("{action}; {step}."))
        }
        _ => Remediation::recoverable(action, [step]),
    };
    Box::new(ErrorContext::new(code, message, remediation))
}

fn persistence(error: impl std::fmt::Display) -> TelemetryClientError {
    AdmissionError::Persistence {
        context: context(
            error_codes::SC_OBSERVABILITY_ADMIT_PERSISTENCE,
            &format!("durable store operation failed: {error}"),
        ),
    }
    .into()
}
fn closed() -> TelemetryClientError {
    AdmissionError::Closed {
        context: context(
            error_codes::SC_OBSERVABILITY_ADMIT_CLOSED,
            "client has shut down; open a new client to resume",
        ),
    }
    .into()
}
impl DurableTelemetryClient {
    #[cfg(test)]
    pub(crate) fn open_with_exporter(
        config: TelemetryClientConfig,
        exporter: Arc<dyn SubmissionExporter>,
    ) -> Result<Self, TelemetryClientError> {
        Self::open_with_exporter_factory(config, |_, _| Ok(exporter))
    }
    fn open_with_exporter_factory(
        config: TelemetryClientConfig,
        build_exporter: impl FnOnce(
            crate::sync_http::submission::SyncHttpConfig,
            crate::config::ValidatedTransportBounds,
        ) -> Result<
            Arc<dyn SubmissionExporter>,
            sc_observability_types::v2::ExportError,
        >,
    ) -> Result<Self, TelemetryClientError> {
        let otel = adapter::otel_config_from(&config)?;
        let (worker_config, bounds) =
            crate::sync_http::submission::SyncHttpConfig::from_otel(&otel)
                .map_err(|error| adapter::invalid_reason("otlp", error.code().as_str()))?;
        let mut client = Self::prepare_validated(config, &bounds)?;
        let exporter = build_exporter(worker_config, bounds)
            .map_err(|error| adapter::invalid_reason("otlp", error.code().as_str()))?;
        client.owner.exporter = Some(Arc::clone(&exporter));
        worker::start(&client.owner.shared, exporter)?;
        Ok(client)
    }
    fn prepare_validated(
        config: TelemetryClientConfig,
        bounds: &crate::config::ValidatedTransportBounds,
    ) -> Result<Self, TelemetryClientError> {
        let shared = Arc::new(Shared {
            db: database::Database::new(store::open(&config.store_path)?)?,
            config,
            holder: row::LeaseHolder(format!("{}:{}", std::process::id(), uuid::Uuid::now_v7())),
            closed: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            credits: AdmissionCredits::new(bounds),
            wake: Mutex::new(0),
            changed: Condvar::new(),
            last_error: Mutex::new(None),
            workers: Mutex::new(Vec::new()),
            live_workers: std::sync::atomic::AtomicUsize::new(0),
            #[cfg(test)]
            waiting_for_credits: Mutex::new(std::collections::HashSet::new()),
            #[cfg(test)]
            drain_on_flush_only: AtomicBool::new(tests::conformance::enabled()),
            #[cfg(test)]
            active_flushes: std::sync::atomic::AtomicUsize::new(0),
            #[cfg(test)]
            stalled_signals: Mutex::new(std::collections::HashSet::new()),
        });
        Ok(Self {
            owner: Owner {
                shared,
                exporter: None,
            },
        })
    }
    fn flush_scope(
        &self,
        id: Option<&SubmissionId>,
        timeout: Duration,
    ) -> Result<FlushReport, TelemetryClientError> {
        let shared = &self.owner.shared;
        let start = Instant::now();
        // A WAL reader must not queue behind an admission waiting for a write lock.
        let reader = store::reader(&shared.config.store_path)?;
        reader
            .busy_timeout(timeout.saturating_sub(start.elapsed()))
            .map_err(persistence)?;
        let scope = query::snapshot(&reader, id)?;
        #[cfg(test)]
        let _flush_activity = FlushActivity::new(shared);
        shared.notify();
        // Unit-test scheduling settles scripted outcomes only after the real snapshot.
        // Production includes snapshot/drain time in the original deadline.
        #[cfg(test)]
        let start = if shared.drain_on_flush_only.load(Ordering::Acquire) {
            tests::conformance::await_scripted_outcomes(shared, &reader, &scope);
            Instant::now()
        } else {
            start
        };
        loop {
            let generation = shared.generation();
            reader
                .busy_timeout(timeout.saturating_sub(start.elapsed()))
                .map_err(persistence)?;
            let report = query::report(&reader, &scope)?;
            if report.still_pending.total() == 0 || start.elapsed() >= timeout {
                return report.into_result();
            }
            shared.wait_since(
                generation,
                timeout
                    .saturating_sub(start.elapsed())
                    .min(Shared::poll_interval()),
            );
        }
    }
}
impl TelemetryClient for DurableTelemetryClient {
    #[sc_lint(boundary.allow("cycle.type_method_self_loop"))]
    fn open(config: TelemetryClientConfig) -> Result<Self, TelemetryClientError> {
        Self::open_with_exporter_factory(config, crate::sync_http::submission::exporter_for)
    }
    fn emit(&self, envelope: SubmissionEnvelope) -> Result<AdmissionReceipt, TelemetryClientError> {
        let shared = &self.owner.shared;
        let receipt = {
            let mut db = shared.db.lock()?;
            if shared.closed.load(Ordering::Acquire) {
                return Err(closed());
            }
            store::admit(&mut db, &shared.config, &envelope)?
        };
        shared.notify();
        Ok(receipt)
    }
    fn flush(&self, deadline: Duration) -> Result<FlushReport, TelemetryClientError> {
        if self.owner.shared.closed.load(Ordering::Acquire) {
            return Err(closed());
        }
        self.flush_scope(None, deadline)
    }
    fn flush_submission(
        &self,
        id: &SubmissionId,
        deadline: Duration,
    ) -> Result<FlushReport, TelemetryClientError> {
        if self.owner.shared.closed.load(Ordering::Acquire) {
            return Err(closed());
        }
        self.flush_scope(Some(id), deadline)
    }
    fn shutdown(&self, deadline: Duration) -> Result<FlushReport, TelemetryClientError> {
        let shared = &self.owner.shared;
        let start = Instant::now();
        if shared.closed.swap(true, Ordering::AcqRel) {
            return Ok(FlushReport::default());
        }
        // Keep workers alive for the bounded final flush.  Taking the single
        // write connection first lets a worker repeatedly contend with the
        // shutdown path after delivery has already completed.
        let result = self.flush_scope(None, deadline.saturating_sub(start.elapsed()));
        shared.stop.store(true, Ordering::Release);
        if let Some(exporter) = &self.owner.exporter {
            exporter.cancel();
        }
        shared.notify();
        worker::join(shared, deadline.saturating_sub(start.elapsed()));
        // This waits only for an admission that was already in progress when
        // `closed` was set.  Workers are gone, so a successful final flush
        // cannot be turned into an in-process lock timeout during shutdown.
        let remaining = deadline.saturating_sub(start.elapsed());
        if !remaining.is_zero() {
            let _db = shared.db.lock_for(remaining)?;
        }
        result
    }

    fn status(&self, query: StatusQuery) -> Result<StoreStatus, TelemetryClientError> {
        let shared = &self.owner.shared;
        query::status(&*shared.db.lock()?, &shared.config, query)
    }
}

// The synchronous conformance harness needs a deterministic snapshot boundary.
// Normal tests and production use autonomous workers.
#[cfg(test)]
struct FlushActivity<'a>(&'a Shared);
#[cfg(test)]
impl<'a> FlushActivity<'a> {
    fn new(shared: &'a Shared) -> Self {
        shared.active_flushes.fetch_add(1, Ordering::AcqRel);
        Self(shared)
    }
}
#[cfg(test)]
impl Drop for FlushActivity<'_> {
    fn drop(&mut self) {
        self.0.active_flushes.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod diagnostic_classification_tests {
    use super::context;
    use crate::error_codes::{DURABLE_CORRUPT, DURABLE_OVERSIZE, DURABLE_QUERY};
    use sc_observability_types::{Remediation, error_codes};

    #[test]
    fn permanent_durable_and_configuration_failures_are_not_recoverable() {
        let cases = [
            (DURABLE_OVERSIZE, "split the submission"),
            (DURABLE_CORRUPT, "restore it from the original source"),
            (DURABLE_QUERY, "select Summary or Submissions"),
            (
                error_codes::SC_OBSERVABILITY_ADMIT_SCHEMA_TOO_NEW,
                "use a client supporting this store version",
            ),
            (
                error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_FILE,
                "correct the telemetry file",
            ),
            (
                error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID,
                "correct the named configuration field",
            ),
            (
                error_codes::SC_OBSERVABILITY_TELEMETRY_UNSUPPORTED,
                "select the sync_http backend",
            ),
        ];

        for (code, expected_guidance) in cases {
            let diagnostic = context(code.clone(), "test diagnostic")
                .diagnostic()
                .clone();
            assert_eq!(diagnostic.code, code);
            let Remediation::NotRecoverable { justification } = diagnostic.remediation else {
                panic!("{code} is a permanent failure and must not be recoverable");
            };
            assert!(
                justification.contains(expected_guidance),
                "{code} remediation is not actionable: {justification}"
            );
        }
    }

    #[test]
    fn transient_store_failures_remain_recoverable() {
        let diagnostic = context(
            error_codes::SC_OBSERVABILITY_ADMIT_PERSISTENCE,
            "temporary store failure",
        )
        .diagnostic()
        .clone();

        assert_eq!(
            diagnostic.code,
            error_codes::SC_OBSERVABILITY_ADMIT_PERSISTENCE
        );
        let Remediation::Recoverable { steps } = diagnostic.remediation else {
            panic!("transient persistence failures should remain recoverable");
        };
        assert!(
            steps
                .first_step()
                .is_some_and(|step| !step.trim().is_empty())
        );
    }
}

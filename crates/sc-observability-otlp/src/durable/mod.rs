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
mod query;
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
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// Durable client. Dropping a handle stops its workers; explicit shutdown also flushes.
#[derive(Debug)]
pub struct DurableTelemetryClient {
    owner: Owner,
}
#[derive(Debug)]
struct Owner {
    shared: Arc<Shared>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        self.shared.notify();
    }
}
// A mutex serializes short SQLite transactions. Export and waits never hold it.
#[derive(Debug)]
struct Shared {
    db: Mutex<rusqlite::Connection>,
    config: TelemetryClientConfig,
    holder: String,
    closed: AtomicBool,
    stop: AtomicBool,
    credits: AdmissionCredits,
    wake: Mutex<u64>,
    changed: Condvar,
    #[cfg(test)]
    drain_on_flush_only: AtomicBool,
    #[cfg(test)]
    active_flushes: std::sync::atomic::AtomicUsize,
}
impl Shared {
    fn notify(&self) {
        let mut generation = self.wake.lock().expect("notification lock poisoned");
        *generation = generation.wrapping_add(1);
        self.changed.notify_all();
    }
    fn generation(&self) -> u64 {
        *self.wake.lock().expect("notification lock poisoned")
    }
    fn wait(&self, duration: Duration) {
        self.wait_since(self.generation(), duration);
    }
    fn wait_since(&self, before: u64, duration: Duration) {
        let generation = self.wake.lock().expect("notification lock poisoned");
        let _guard = self
            .changed
            .wait_timeout_while(generation, duration, |g| {
                *g == before && !self.stop.load(Ordering::Acquire)
            })
            .expect("notification lock poisoned");
    }
    fn poll_interval(&self) -> Duration {
        (self.config.lease_duration / crate::constants::LEASE_RENEWAL_DIVISOR).min(
            Duration::from_millis(
                crate::constants::STORE_BUSY_TIMEOUT_MS / crate::constants::DRAIN_BATCH_SIZE as u64,
            ),
        )
    }
}
fn context(code: ErrorCode, message: &str) -> Box<ErrorContext> {
    Box::new(ErrorContext::new(
        code,
        message,
        Remediation::recoverable(
            "inspect the typed failure and correct the store or configuration before retrying",
            ["retain the store for recovery; do not delete pending records"],
        ),
    ))
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
    pub(crate) fn open_with_exporter(
        config: TelemetryClientConfig,
        exporter: Arc<dyn SubmissionExporter>,
    ) -> Result<Self, TelemetryClientError> {
        let otel = adapter::otel_config_from(&config)?;
        let bounds = crate::config::validated_transport_bounds(&otel)
            .map_err(|_| adapter::invalid("otlp"))?;
        let shared = Arc::new(Shared {
            db: Mutex::new(store::open(&config.store_path)?),
            config,
            holder: format!("{}:{}", std::process::id(), uuid::Uuid::now_v7()),
            closed: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            credits: AdmissionCredits::new(&bounds),
            wake: Mutex::new(0),
            changed: Condvar::new(),
            #[cfg(test)]
            drain_on_flush_only: AtomicBool::new(false),
            #[cfg(test)]
            active_flushes: std::sync::atomic::AtomicUsize::new(0),
        });
        let client = Self {
            owner: Owner { shared },
        };
        worker::start(&client.owner.shared, exporter)?;
        Ok(client)
    }
    fn flush_scope(
        &self,
        id: Option<&SubmissionId>,
        timeout: Duration,
    ) -> Result<FlushReport, TelemetryClientError> {
        let shared = &self.owner.shared;
        let start = Instant::now();
        let scope = query::snapshot(&*shared.db.lock().map_err(persistence)?, id)?;
        #[cfg(test)]
        let _flush_activity = FlushActivity::new(shared);
        shared.notify();
        loop {
            let generation = shared.generation();
            let report = query::report(&*shared.db.lock().map_err(persistence)?, &scope)?;
            if report.still_pending.total() == 0 || start.elapsed() >= timeout {
                return report.into_result();
            }
            shared.wait_since(
                generation,
                timeout
                    .saturating_sub(start.elapsed())
                    .min(shared.poll_interval()),
            );
        }
    }
}
impl TelemetryClient for DurableTelemetryClient {
    #[sc_lint(boundary.allow("cycle.type_method_self_loop"))]
    fn open(config: TelemetryClientConfig) -> Result<Self, TelemetryClientError> {
        let otel = adapter::otel_config_from(&config)?;
        let (worker, bounds) = crate::sync_http::submission::SyncHttpConfig::from_otel(&otel)
            .map_err(|_| adapter::invalid("otlp"))?;
        Self::open_with_exporter(
            config,
            crate::sync_http::submission::exporter_for(worker, bounds),
        )
    }
    fn emit(&self, envelope: SubmissionEnvelope) -> Result<AdmissionReceipt, TelemetryClientError> {
        let shared = &self.owner.shared;
        let receipt = {
            let mut db = shared.db.lock().map_err(persistence)?;
            if shared.closed.load(Ordering::Acquire) {
                return Err(closed());
            }
            store::admit(&mut db, &shared.config, &envelope)?
        };
        shared.notify();
        Ok(receipt)
    }
    fn flush(&self, deadline: Duration) -> Result<FlushReport, TelemetryClientError> {
        self.flush_scope(None, deadline)
    }
    fn flush_submission(
        &self,
        id: &SubmissionId,
        deadline: Duration,
    ) -> Result<FlushReport, TelemetryClientError> {
        self.flush_scope(Some(id), deadline)
    }
    fn shutdown(&self, deadline: Duration) -> Result<FlushReport, TelemetryClientError> {
        let shared = &self.owner.shared;
        {
            let _db = shared.db.lock().map_err(persistence)?;
            if shared.closed.swap(true, Ordering::AcqRel) {
                return Ok(FlushReport::default());
            }
        }
        let result = self.flush_scope(None, deadline);
        shared.stop.store(true, Ordering::Release);
        shared.notify();
        // No thread join here: a slow exporter must not extend the caller's deadline.
        worker::release(shared);
        result
    }
    fn status(&self, query: StatusQuery) -> Result<StoreStatus, TelemetryClientError> {
        let shared = &self.owner.shared;
        query::status(
            &*shared.db.lock().map_err(persistence)?,
            &shared.config,
            query,
        )
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

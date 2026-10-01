//! Scripted in-memory client for front-end tests; it performs no transport or durable I/O.
#![allow(
    clippy::result_large_err,
    reason = "approved client errors retain an inline four-signal FlushReport"
)]
use super::{
    AdmissionError, AdmissionReceipt, DeliveryState, DeliveryStatus, FlushReport, Signal,
    StatusQuery, StoreStatus, SubmissionEnvelope, SubmissionId, TelemetryClient,
    TelemetryClientConfig, TelemetryClientError, TelemetryConfigError, error_codes,
    errors::{context, context_with_source},
};
use crate::{ErrorCode, Timestamp};
use sc_lint_attributes::sc_lint;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};
/// Reusable trait-level lifecycle tests, also run against the real durable client.
pub mod conformance;

/// Script file shared by CLI and Python test hooks.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoubleScript {
    /// Admission outcomes consumed in input order.
    #[serde(default)]
    pub admissions: Vec<ScriptedAdmission>,
    /// Delivery outcomes consumed in order for each signal.
    #[serde(default)]
    pub deliveries: Vec<ScriptedDelivery>,
    /// Minimum delay on every flush or shutdown, for GIL-release tests.
    #[serde(default)]
    pub flush_delay_ms: u64,
}
impl DoubleScript {
    /// Parses the common test-hook script schema.
    /// # Errors
    /// Rejects invalid or unknown script fields.
    pub fn from_json(json: &str) -> Result<Self, TelemetryConfigError> {
        serde_json::from_str(json).map_err(|error| TelemetryConfigError::InvalidField {
            field: "test_double_script",
            context: context_with_source(
                error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID,
                format!(
                    "invalid test double script at line {} column {}",
                    error.line(),
                    error.column()
                ),
                error,
            ),
        })
    }
}
/// One admission decision.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", from = "AdmissionRaw")]
pub enum ScriptedAdmission {
    /// Admit normally.
    Admit,
    /// Reject before committing.
    Reject {
        /// Stable admission error category.
        kind: AdmissionErrorKind,
    },
}
#[derive(Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
enum AdmissionRaw {
    Admit {},
    Reject { kind: AdmissionErrorKind },
}
impl From<AdmissionRaw> for ScriptedAdmission {
    fn from(raw: AdmissionRaw) -> Self {
        match raw {
            AdmissionRaw::Admit {} => Self::Admit,
            AdmissionRaw::Reject { kind } => Self::Reject { kind },
        }
    }
}
/// Admission categories available to front-end failure tests.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionErrorKind {
    /// Store could not be opened.
    StoreUnavailable,
    /// Store capacity reached.
    DiskBoundExceeded,
    /// Commit failed.
    Persistence,
    /// Store belongs to a newer schema.
    SchemaTooNew,
    /// Client already closed.
    Closed,
}
/// One signal's next export outcome.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptedDelivery {
    /// Target signal.
    pub signal: Signal,
    /// Export behavior.
    pub outcome: DeliveryOutcome,
}
impl ScriptedDelivery {
    /// Creates a per-signal outcome.
    #[must_use]
    pub const fn new(signal: Signal, outcome: DeliveryOutcome) -> Self {
        Self { signal, outcome }
    }
}
/// Scripted exporter behavior.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryOutcome {
    /// Collector accepts the row.
    Deliver,
    /// Terminal collector failure.
    Fail,
    /// Remains pending through the deadline.
    Stall,
}
#[derive(Debug, Clone)]
struct Entry {
    envelope: SubmissionEnvelope,
    receipt: AdmissionReceipt,
    status: DeliveryStatus,
}
#[derive(Debug, Default)]
struct State {
    entries: Vec<Entry>,
    admissions: VecDeque<ScriptedAdmission>,
    deliveries: VecDeque<ScriptedDelivery>,
    failures: VecDeque<(Signal, ErrorCode)>,
    flush_delay: Duration,
    closed: bool,
}
/// Cloneable scripted implementation of the shared client contract.
///
/// A poisoned state lock means the store is unavailable: client operations
/// return `AdmissionError::StoreUnavailable`, the setup helpers leave the
/// script unchanged and `envelopes` returns no envelopes. Nothing panics.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct InMemoryTelemetryClient {
    config: TelemetryClientConfig,
    /// Shared and mutex-guarded because every `TelemetryClient` method takes
    /// `&self` while admitting and delivering, and clones (such as a test
    /// harness handle) must observe the same store.
    state: Arc<Mutex<State>>,
}
fn admission(kind: AdmissionErrorKind) -> TelemetryClientError {
    let error = match kind {
        AdmissionErrorKind::StoreUnavailable => AdmissionError::StoreUnavailable {
            context: context(
                error_codes::SC_OBSERVABILITY_ADMIT_STORE_UNAVAILABLE,
                "scripted store unavailable",
            ),
        },
        AdmissionErrorKind::DiskBoundExceeded => AdmissionError::DiskBoundExceeded {
            context: context(
                error_codes::SC_OBSERVABILITY_ADMIT_DISK_BOUND,
                "scripted disk bound",
            ),
        },
        AdmissionErrorKind::Persistence => AdmissionError::Persistence {
            context: context(
                error_codes::SC_OBSERVABILITY_ADMIT_PERSISTENCE,
                "scripted persistence failure",
            ),
        },
        AdmissionErrorKind::SchemaTooNew => AdmissionError::SchemaTooNew {
            found: 2,
            supported: 1,
            context: context(
                error_codes::SC_OBSERVABILITY_ADMIT_SCHEMA_TOO_NEW,
                "scripted newer store schema",
            ),
        },
        AdmissionErrorKind::Closed => AdmissionError::Closed {
            context: context(
                error_codes::SC_OBSERVABILITY_ADMIT_CLOSED,
                "client is closed",
            ),
        },
    };
    error.into()
}
impl InMemoryTelemetryClient {
    /// Creates a test client with prescribed admission and delivery outcomes.
    #[must_use]
    #[sc_lint(boundary.allow("cycle.type_method_self_loop"))]
    pub fn with_script(config: TelemetryClientConfig, script: DoubleScript) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(State {
                admissions: script.admissions.into(),
                deliveries: script.deliveries.into(),
                flush_delay: Duration::from_millis(script.flush_delay_ms),
                ..State::default()
            })),
        }
    }
    /// Appends outcomes and replaces the per-call delay; no effect once the
    /// store is unavailable.
    pub fn push_script(&self, script: DoubleScript) {
        if let Ok(mut state) = self.lock() {
            state.admissions.extend(script.admissions);
            state.deliveries.extend(script.deliveries);
            state.flush_delay = Duration::from_millis(script.flush_delay_ms);
        }
    }
    /// Delivers every nonterminal row and removes pending export scripts; no
    /// effect once the store is unavailable.
    pub fn deliver_all(&self) {
        let Ok(mut state) = self.lock() else {
            return;
        };
        state.deliveries.clear();
        state.failures.clear();
        for entry in &mut state.entries {
            for (_, row) in &mut entry.status.signals {
                if !row.is_terminal() {
                    *row = DeliveryState::Delivered {
                        at: Timestamp::now_utc(),
                        attempts: 1,
                    };
                }
            }
        }
    }
    /// Makes the next attempt for a signal fail with a caller-selected code;
    /// no effect once the store is unavailable.
    pub fn fail_next(&self, signal: Signal, code: ErrorCode) {
        if let Ok(mut state) = self.lock() {
            state.failures.push_back((signal, code));
        }
    }
    /// Returns admitted envelopes in admission order; empty once the store is
    /// unavailable.
    #[must_use]
    pub fn envelopes(&self) -> Vec<SubmissionEnvelope> {
        self.lock()
            .map(|state| state.entries.iter().map(|e| e.envelope.clone()).collect())
            .unwrap_or_default()
    }
    /// The only access path to the state, applying the poison policy above.
    fn lock(&self) -> Result<MutexGuard<'_, State>, TelemetryClientError> {
        self.state.lock().map_err(|_| {
            AdmissionError::StoreUnavailable {
                context: context(
                    error_codes::SC_OBSERVABILITY_ADMIT_STORE_UNAVAILABLE,
                    "test double state lock is poisoned by a panicking caller",
                ),
            }
            .into()
        })
    }
    fn complete(
        &self,
        id: Option<&SubmissionId>,
        deadline: Duration,
        shutdown: bool,
    ) -> Result<FlushReport, TelemetryClientError> {
        let start = Instant::now();
        let (scope, delay, repeated) = {
            let mut state = self.lock()?;
            let repeated = shutdown && state.closed;
            let scope = if repeated {
                vec![]
            } else {
                state
                    .entries
                    .iter()
                    .enumerate()
                    .flat_map(|(i, e)| {
                        e.status
                            .signals
                            .iter()
                            .enumerate()
                            .filter_map(move |(j, (_, s))| {
                                if id.map_or_else(
                                    || !s.is_terminal(),
                                    |id| id == &e.receipt.submission_id,
                                ) {
                                    Some((i, j))
                                } else {
                                    None
                                }
                            })
                    })
                    .collect::<Vec<_>>()
            };
            if shutdown {
                state.closed = true;
            }
            (scope, state.flush_delay, repeated)
        };
        std::thread::sleep(delay);
        if repeated {
            return Ok(FlushReport::default());
        }
        let mut state = self.lock()?;
        for &(i, j) in &scope {
            if state.entries[i].status.signals[j].1.is_terminal() {
                continue;
            }
            let signal = state.entries[i].status.signals[j].0;
            let failure = state
                .failures
                .iter()
                .position(|(s, _)| *s == signal)
                .and_then(|p| state.failures.remove(p))
                .map(|(_, code)| code);
            let outcome = state
                .deliveries
                .iter()
                .position(|d| d.signal == signal)
                .and_then(|p| state.deliveries.remove(p))
                .map_or(DeliveryOutcome::Deliver, |d| d.outcome);
            state.entries[i].status.signals[j].1 = if let Some(error) = failure {
                DeliveryState::Failed { attempts: 1, error }
            } else {
                match outcome {
                    DeliveryOutcome::Deliver => DeliveryState::Delivered {
                        at: Timestamp::now_utc(),
                        attempts: 1,
                    },
                    DeliveryOutcome::Fail => DeliveryState::Failed {
                        attempts: 1,
                        error: error_codes::SC_OBSERVABILITY_TEST_DOUBLE_SCRIPTED_FAILURE,
                    },
                    DeliveryOutcome::Stall => DeliveryState::Pending,
                }
            };
        }
        let pending = scope
            .iter()
            .any(|&(i, j)| !state.entries[i].status.signals[j].1.is_terminal());
        drop(state);
        if pending {
            std::thread::sleep(deadline.saturating_sub(start.elapsed()));
        }
        let state = self.lock()?;
        let mut report = FlushReport::default();
        for (i, j) in scope {
            let (signal, row) = &state.entries[i].status.signals[j];
            match row {
                DeliveryState::Delivered { .. } => report.delivered.increment(*signal),
                DeliveryState::Failed { .. } => report.failed.increment(*signal),
                DeliveryState::EvictedByDiskBound { .. } => report.evicted.increment(*signal),
                _ => report.still_pending.increment(*signal),
            }
        }
        report.into_result()
    }
}
impl TelemetryClient for InMemoryTelemetryClient {
    #[sc_lint(boundary.allow("cycle.type_method_self_loop"))]
    fn open(config: TelemetryClientConfig) -> Result<Self, TelemetryClientError> {
        config.validate()?;
        Ok(Self::with_script(config, DoubleScript::default()))
    }
    fn emit(&self, envelope: SubmissionEnvelope) -> Result<AdmissionReceipt, TelemetryClientError> {
        envelope.validate()?;
        let mut state = self.lock()?;
        if state.closed {
            return Err(admission(AdmissionErrorKind::Closed));
        }
        if let Some(key) = &envelope.record_key
            && let Some(entry) = state
                .entries
                .iter()
                .find(|e| e.receipt.record_key.as_ref() == Some(key))
        {
            let mut receipt = entry.receipt.clone();
            receipt.duplicate = true;
            return Ok(receipt);
        }
        if let Some(ScriptedAdmission::Reject { kind }) = state.admissions.pop_front() {
            return Err(admission(kind));
        }
        let submission_id: SubmissionId = uuid::Uuid::now_v7().to_string().parse()?;
        let receipt = AdmissionReceipt {
            submission_id: submission_id.clone(),
            record_key: envelope.record_key.clone(),
            admitted_at: Timestamp::now_utc(),
            signals: envelope.signals().iter().collect(),
            duplicate: false,
        };
        let status = DeliveryStatus {
            submission_id,
            signals: receipt
                .signals
                .iter()
                .map(|s| (*s, DeliveryState::Pending))
                .collect(),
        };
        state.entries.push(Entry {
            envelope,
            receipt: receipt.clone(),
            status,
        });
        Ok(receipt)
    }
    fn flush(&self, deadline: Duration) -> Result<FlushReport, TelemetryClientError> {
        self.complete(None, deadline, false)
    }
    fn flush_submission(
        &self,
        id: &SubmissionId,
        deadline: Duration,
    ) -> Result<FlushReport, TelemetryClientError> {
        self.complete(Some(id), deadline, false)
    }
    fn shutdown(&self, deadline: Duration) -> Result<FlushReport, TelemetryClientError> {
        self.complete(None, deadline, true)
    }
    fn status(&self, query: StatusQuery) -> Result<StoreStatus, TelemetryClientError> {
        let state = self.lock()?;
        let mut status = StoreStatus {
            schema_version: 1,
            max_store_bytes: self.config.max_store_bytes,
            ..StoreStatus::default()
        };
        for entry in &state.entries {
            status.store_bytes +=
                u64::try_from(entry.envelope.to_canonical_json().len()).unwrap_or(u64::MAX);
            for (signal, row) in &entry.status.signals {
                match row {
                    DeliveryState::Delivered { .. } => status.delivered_retained.increment(*signal),
                    DeliveryState::Failed { .. } => status.failed.increment(*signal),
                    DeliveryState::RetryScheduled { .. } => {
                        status.retry_scheduled.increment(*signal);
                    }
                    DeliveryState::EvictedByDiskBound { .. } => status.evicted_by_disk_bound += 1,
                    _ => status.pending.increment(*signal),
                }
            }
            let include = match &query {
                StatusQuery::Summary => false,
                StatusQuery::Submissions(ids) => ids.contains(&entry.receipt.submission_id),
                StatusQuery::RecordKeys(keys) => entry
                    .receipt
                    .record_key
                    .as_ref()
                    .is_some_and(|k| keys.contains(k)),
            };
            if include {
                status.submissions.push(entry.status.clone());
            }
        }
        Ok(status)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AdmissionError, DoubleScript, InMemoryTelemetryClient, Signal, StatusQuery,
        SubmissionEnvelope, TelemetryClient, TelemetryClientError,
    };
    use crate::otlp::submission::{
        ConfigOverrides, ConfigSources, LogInput, SubmissionInput, SystemIds, resolve_config,
    };
    use std::time::Duration;

    fn client() -> InMemoryTelemetryClient {
        let overrides = ConfigOverrides {
            store_path: Some("unused.db".into()),
            ..ConfigOverrides::default()
        };
        let config = resolve_config(ConfigSources::new(&overrides, None, &|_| None)).unwrap();
        InMemoryTelemetryClient::with_script(config, DoubleScript::default())
    }
    fn envelope() -> SubmissionEnvelope {
        let mut input = SubmissionInput::new();
        input.logs.push(LogInput::new());
        SubmissionEnvelope::from_input(input, &mut SystemIds::new()).unwrap()
    }
    fn unavailable(result: &Result<impl std::fmt::Debug, TelemetryClientError>) {
        assert!(
            matches!(
                result,
                Err(TelemetryClientError::Admission(
                    AdmissionError::StoreUnavailable { .. }
                ))
            ),
            "{result:?}"
        );
    }

    #[test]
    fn poisoned_state_is_one_store_unavailable_policy() {
        let client = client();
        client.emit(envelope()).unwrap();
        let poisoner = client.clone();
        let panicked = std::thread::spawn(move || {
            let _guard = poisoner.state.lock();
            panic!("poison the double state");
        })
        .join();
        assert!(panicked.is_err());
        assert!(client.state.is_poisoned());

        unavailable(&client.emit(envelope()));
        unavailable(&client.flush(Duration::ZERO));
        unavailable(&client.shutdown(Duration::ZERO));
        unavailable(&client.status(StatusQuery::Summary));
        client.push_script(DoubleScript::default());
        client.deliver_all();
        client.fail_next(
            Signal::Logs,
            crate::error_codes::SC_OBSERVABILITY_DELIVERY_FAILED,
        );
        assert!(client.envelopes().is_empty());
    }
}

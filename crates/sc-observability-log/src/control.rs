//! [`LogControl`]: the cloneable, non-owning direct bridge control surface.

use std::path::PathBuf;
use std::time::Duration;

use crate::health::BridgeLifecycle;
use crate::{
    BridgeHealthReport, ControlError, EmitError, FieldKeyError, FlushError, LifecyclePhase, handle,
    health, mapping,
};

/// Typed direct producer input. Bridge-owned envelope identity and timestamps
/// remain absent, so a caller cannot replace host provenance.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BridgeEvent {
    /// Canonical core event level.
    pub level: sc_observability_types::Level,
    /// Pre-validated event target.
    pub target: sc_observability_types::TargetCategory,
    /// Optional action; the bridge default is used when omitted.
    pub action: Option<sc_observability_types::ActionName>,
    /// Optional producer message.
    pub message: Option<String>,
    /// Optional operation outcome.
    pub outcome: Option<sc_observability_types::OutcomeLabel>,
    /// Producer fields, validated by the same key policy as macros.
    pub fields: serde_json::Map<String, serde_json::Value>,
    /// Optional request identifier.
    pub request_id: Option<sc_observability_types::CorrelationId>,
    /// Optional correlation identifier.
    pub correlation_id: Option<sc_observability_types::CorrelationId>,
    /// Optional explicit trace context.
    pub trace: Option<sc_observability_types::TraceContext>,
}

/// Compatibility spelling for the core admission result, not an independent enum.
pub type EmitOutcome = sc_observability_types::AdmissionOutcome;

/// Cloneable, non-owning control of the installed bridge.
#[derive(Debug, Clone)]
pub struct LogControl {
    _private: (),
}

impl LogControl {
    pub(crate) const fn new() -> Self {
        Self { _private: () }
    }

    /// Flushes on a helper thread, bounded by `timeout`.
    ///
    /// # Errors
    ///
    /// Returns [`FlushError`] when the lifecycle or bounded flush rejects the request.
    pub fn flush(&self, timeout: Duration) -> Result<(), FlushError> {
        handle::flush_installed(timeout)
    }

    /// Read-only native bridge-health snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`ControlError::Unavailable`] when no core report is retained.
    pub fn health(&self) -> Result<BridgeHealthReport, ControlError> {
        health::snapshot()
    }

    /// The active JSONL file captured at `init`, as an owned value.
    ///
    /// # Errors
    ///
    /// Returns [`ControlError::Unavailable`] before initialization.
    pub fn active_log_path(&self) -> Result<Option<PathBuf>, ControlError> {
        health::active_log_path()
    }

    /// Snapshot of exact-once bridge rejection counters.
    #[must_use]
    pub fn dropped_events(&self) -> crate::DroppedEvents {
        handle::dropped_events()
    }

    /// Waits only for the already-started owner shutdown.
    ///
    /// # Errors
    ///
    /// Returns [`crate::WaitError`] when completion is absent or the deadline expires.
    pub fn wait_stopped(
        &self,
        timeout: Duration,
    ) -> Result<crate::ShutdownReport, crate::WaitError> {
        handle::wait_stopped(timeout)
    }

    /// Admits a typed direct event through the same staged-core writer path as
    /// facade and macro producers.
    ///
    /// # Errors
    ///
    /// Returns [`EmitError`] after exact-once accounting for a rejected event.
    pub fn try_log(&self, event: BridgeEvent) -> Result<EmitOutcome, EmitError> {
        handle::submit_guarded(|| {
            if handle::lifecycle() != BridgeLifecycle::Running {
                return Err(not_running());
            }
            let installed = handle::current_installed().ok_or_else(not_running)?;
            let event = direct_event(event, &installed)?;
            installed
                .logger
                .try_log_with_outcome(event)
                .map_err(|error| core_emit_error(&error))
        })
    }

    /// Executes a typed core query without yielding owner or shutdown authority.
    ///
    /// # Errors
    ///
    /// Returns [`ControlError`] when the lifecycle or staged core rejects the query.
    pub fn query(
        &self,
        query: &sc_observability_types::LogQuery,
    ) -> Result<sc_observability_types::LogSnapshot, ControlError> {
        let installed = handle::current_installed().ok_or_else(|| ControlError::NotRunning {
            phase: lifecycle_phase(),
        })?;
        installed.logger.query(query).map_err(|error| {
            let diagnostic = error.diagnostic();
            ControlError::Query {
                diagnostic: operation_diagnostic(
                    error.code(),
                    error.to_string(),
                    diagnostic.remediation.clone(),
                ),
            }
        })
    }
}

fn direct_event(
    event: BridgeEvent,
    installed: &handle::Installed,
) -> Result<sc_observability_types::LogEvent, EmitError> {
    let mut fields = serde_json::Map::new();
    let mut raw_keys = std::collections::BTreeMap::new();
    for (raw, value) in event.fields {
        let key = mapping::field_key_label(&raw)
            .map_err(|error| EmitError::InvalidField {
                raw_key: raw.clone(),
                reason: match error {
                    mapping::LabelError::Empty { .. } | mapping::LabelError::Rejected { .. } => {
                        FieldKeyError::Empty
                    }
                    mapping::LabelError::ReservedPrefix { .. } => FieldKeyError::ReservedPrefix,
                },
            })?
            .into_owned();
        if let Some(other_raw_key) = raw_keys.insert(key.clone(), raw.clone()) {
            return Err(EmitError::InvalidField {
                raw_key: raw,
                reason: FieldKeyError::Collision { other_raw_key },
            });
        }
        fields.insert(key, value);
    }
    let observation = sc_observability_types::Observation::new(installed.service.clone(), ());
    Ok(sc_observability_types::LogEvent {
        version: observation.version,
        timestamp: observation.timestamp,
        level: event.level,
        service: installed.service.clone(),
        target: event.target,
        action: event
            .action
            .unwrap_or_else(|| installed.options.default_action.clone()),
        message: event.message,
        identity: installed.identity.clone(),
        trace: event.trace.or_else(crate::context::current_trace),
        request_id: event.request_id,
        correlation_id: event.correlation_id,
        outcome: event.outcome,
        diagnostic: None,
        state_transition: None,
        fields,
    })
}

fn lifecycle_phase() -> LifecyclePhase {
    handle::lifecycle_phase()
}

fn not_running() -> EmitError {
    EmitError::NotRunning {
        phase: lifecycle_phase(),
    }
}

fn operation_diagnostic(
    code: sc_observability_types::ErrorCode,
    message: String,
    remediation: sc_observability_types::Remediation,
) -> sc_observability_types::OperationDiagnostic {
    sc_observability_types::OperationDiagnostic {
        code,
        message,
        remediation,
        at: sc_observability_types::Timestamp::now_utc(),
    }
}

fn diagnostic_from_context(
    source: &sc_observability_types::ErrorContext,
) -> sc_observability_types::OperationDiagnostic {
    let diagnostic = source.diagnostic();
    sc_observability_types::OperationDiagnostic {
        code: diagnostic.code.clone(),
        message: diagnostic.message.clone(),
        remediation: diagnostic.remediation.clone(),
        at: diagnostic.timestamp,
    }
}

fn core_emit_error(error: &sc_observability::TryLogError) -> EmitError {
    match error {
        sc_observability::TryLogError::InvalidEvent(source) => EmitError::InvalidEvent {
            diagnostic: crate::error::diagnostic_from_info(source),
        },
        sc_observability::TryLogError::QueueFull(source) => EmitError::QueueFull {
            diagnostic: diagnostic_from_context(source),
        },
        sc_observability::TryLogError::WriterDegraded(source) => EmitError::WriterDegraded {
            diagnostic: diagnostic_from_context(source),
        },
        sc_observability::TryLogError::ShutdownTimedOut(source) => EmitError::ShutdownTimedOut {
            diagnostic: diagnostic_from_context(source),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn control_is_send_and_sync() {
        assert_send_sync::<LogControl>();
    }
}

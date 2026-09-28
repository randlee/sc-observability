//! The only core/bridge-to-wire runtime conversion boundary.
use crate::ProducerOrigin;
use dto::{Diagnostic, Failure};
use native::DiagnosticInfo;
use native::typed::ClassifiedError;
use sc_observability_dto as dto;
use sc_observability_log as bridge;
use sc_observability_types as native;

/// Native-owned failure classification reused at the sole binding-to-wire boundary.
pub(crate) type Kind = native::v2::FailureClassification;
pub(crate) fn diagnostic(value: &native::Diagnostic) -> Diagnostic {
    Diagnostic {
        at: value.timestamp.to_string(),
        code: value.code.as_str().into(),
        message: value.message.clone(),
        remediation: value.remediation.clone().into(),
    }
}
fn failure(diagnostic: Diagnostic, kind: Kind) -> Failure {
    dto::failure_from_diagnostic(diagnostic, kind)
}
pub(crate) fn context(value: &native::Diagnostic, kind: Kind) -> Failure {
    failure(diagnostic(value), kind)
}
pub(crate) fn canonical<T: DiagnosticInfo>(value: &T, kind: Kind) -> Failure {
    failure(diagnostic(value.diagnostic()), kind)
}
fn operation(value: native::OperationDiagnostic, kind: Kind) -> Failure {
    failure(value.into(), kind)
}
fn boundary_native(
    code: native::ErrorCode,
    message: String,
    remediation: native::Remediation,
    kind: Kind,
) -> Failure {
    operation(
        native::OperationDiagnostic {
            code,
            message,
            remediation,
            at: native::Timestamp::now_utc(),
        },
        kind,
    )
}
pub(crate) fn admission(value: native::AdmissionOutcome) -> dto::AdmissionDto {
    match value {
        native::AdmissionOutcome::Accepted => dto::AdmissionDto::Accepted,
        native::AdmissionOutcome::Filtered => dto::AdmissionDto::Filtered,
    }
}
pub(crate) fn core_admission(value: sc_observability::TryLogFailure) -> Failure {
    use sc_observability::TryLogFailure as E;
    match value {
        E::InvalidEvent(error) => {
            let kind = if error.kind() == native::typed::EventFailureKind::Closed {
                Kind::Closed
            } else {
                Kind::validation("event")
            };
            let typed = native::v2::EventError::Validation {
                context: error.into_context(),
            };
            canonical(&typed, kind)
        }
        E::QueueFull(error) => context(error.diagnostic(), Kind::QueueFull),
        E::WriterDegraded(error) => context(error.diagnostic(), Kind::Unavailable),
        E::ShutdownTimedOut(error) => context(error.diagnostic(), Kind::timeout("shutdown")),
        _ => crate::error::internal("unrecognized native admission variant"),
    }
}
pub(crate) fn core_flush(error: native::typed::FlushFailure) -> (native::v2::FlushError, Kind) {
    use native::typed::FlushFailureKind as K;
    let kind = match error.kind() {
        K::Closed => Kind::Closed,
        K::WriterDegraded => Kind::Unavailable,
        _ => Kind::Io,
    };
    (crate::error::flush_drain(error.into_context()), kind)
}
pub(crate) fn query(error: &native::QueryError) -> Failure {
    let kind = match &error {
        native::QueryError::InvalidQuery(_) => Kind::validation("query"),
        native::QueryError::Shutdown => Kind::Closed,
        native::QueryError::Unavailable(_) => Kind::Unavailable,
        _ => Kind::Io,
    };
    context(error.diagnostic(), kind)
}
pub(crate) fn bridge_flush(error: bridge::FlushError) -> Failure {
    use bridge::FlushError as E;
    match error {
        E::Logger { diagnostic } => operation(diagnostic, Kind::Io),
        E::HelperSpawn { diagnostic } => operation(diagnostic, Kind::Unavailable),
        E::HelperLost { diagnostic } => operation(diagnostic, Kind::Internal),
        other => {
            let kind = match &other {
                E::TimedOut { .. } => Kind::timeout("flush"),
                E::InProgress => Kind::QueueFull,
                _ => Kind::Closed,
            };
            boundary_native(other.code(), other.to_string(), other.remediation(), kind)
        }
    }
}
pub(crate) fn bridge_control(error: bridge::ControlError) -> Failure {
    match error {
        bridge::ControlError::Query { diagnostic } => operation(diagnostic, Kind::Io),
        bridge::ControlError::Unavailable { diagnostic } => {
            operation(diagnostic, Kind::Unavailable)
        }
        other @ bridge::ControlError::NotRunning { .. } => boundary_native(
            other.code(),
            other.to_string(),
            other.remediation(),
            Kind::Closed,
        ),
    }
}
pub(crate) fn bridge_admission(error: bridge::EmitError) -> Failure {
    use bridge::EmitError as E;
    match error {
        E::InvalidEvent { diagnostic } => operation(diagnostic, Kind::validation("event")),
        E::QueueFull { diagnostic } => operation(diagnostic, Kind::QueueFull),
        E::WriterDegraded { diagnostic } => operation(diagnostic, Kind::Unavailable),
        E::ShutdownTimedOut { diagnostic } => operation(diagnostic, Kind::timeout("shutdown")),
        other => {
            let kind = match &other {
                E::InvalidField { .. } => Kind::validation("event"),
                E::NotRunning { .. } => Kind::Closed,
                _ => Kind::Internal,
            };
            boundary_native(other.code(), other.to_string(), other.remediation(), kind)
        }
    }
}
pub(crate) fn bridge_health(value: bridge::BridgeHealthReport) -> dto::LogHealthDto {
    let level_state = dto::LevelStateDto {
        configured_level: value.configured_level.into(),
        effective_level: value.effective_level.into(),
        level_revision: value.level_revision.into(),
    };
    let checked = dto::from_core_health(
        value.logging,
        native::LevelState {
            configured_level: value.configured_level,
            effective_level: value.effective_level,
            revision: value.level_revision,
        },
    );
    let logging = checked.logging;
    let dropped = value.dropped;
    let bridge = dto::BridgeHealthDto {
        schema_version: value.schema_version,
        logging: logging.clone(),
        dropped: dto::DropCountsDto {
            queue_full: dropped.get(bridge::DropCause::QueueFull).into(),
            invalid_event: dropped.get(bridge::DropCause::InvalidEvent).into(),
            writer_degraded: dropped.get(bridge::DropCause::WriterDegraded).into(),
            shutdown_timed_out: dropped.get(bridge::DropCause::ShutdownTimedOut).into(),
            not_installed: dropped.get(bridge::DropCause::NotInstalled).into(),
            logger_panicked: dropped.get(bridge::DropCause::LoggerPanicked).into(),
            reentrant_emit: dropped.get(bridge::DropCause::ReentrantEmit).into(),
        },
        lifecycle: match value.lifecycle {
            bridge::LifecyclePhase::Running => dto::LifecycleDto::Running,
            bridge::LifecyclePhase::Stopping => dto::LifecycleDto::Stopping,
            bridge::LifecyclePhase::Stopped => dto::LifecycleDto::Stopped,
            bridge::LifecyclePhase::Failed => dto::LifecycleDto::Failed,
        },
        active_log_path: dto::from_path(value.active_log_path.as_deref()),
        configured_level: level_state.configured_level.clone(),
        effective_level: level_state.effective_level.clone(),
        level_revision: level_state.level_revision.clone(),
    };
    dto::LogHealthDto {
        schema_version: 1,
        logging,
        bridge: Some(bridge),
        level_state,
    }
}
pub(crate) fn event(
    value: dto::LogEventDto,
    stamp: dto::EventStamp,
    origin: ProducerOrigin,
) -> Result<native::LogEvent, Failure> {
    let mut event = dto::to_core_event(value, stamp)?;
    let (language, channel) = match origin {
        ProducerOrigin::TauriFrontend => ("typescript", "tauri"),
        ProducerOrigin::Python => ("python", "pyo3"),
        ProducerOrigin::RustHost => ("rust", "native"),
    };
    event
        .fields
        .insert("sc_observability.binding.language".into(), language.into());
    event
        .fields
        .insert("sc_observability.binding.channel".into(), channel.into());
    Ok(event)
}
pub(crate) fn bridge_event(event: native::LogEvent) -> bridge::BridgeEvent {
    bridge::BridgeEvent {
        level: event.level,
        target: event.target,
        action: Some(event.action),
        message: event.message,
        outcome: event.outcome,
        fields: event.fields,
        request_id: event.request_id,
        correlation_id: event.correlation_id,
        trace: event.trace,
    }
}

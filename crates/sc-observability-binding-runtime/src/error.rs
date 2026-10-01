use crate::constants::{MAX_OBSERVATION_TIMEOUT, MAX_OBSERVATION_TIMEOUT_MS};
use sc_observability_dto::{Failure, boundary_diagnostic, error_codes as codes};
use sc_observability_types::{self as native, ErrorCode, ErrorContext, Remediation};
use std::time::Duration;

pub(crate) fn internal(message: impl Into<String>) -> Failure {
    Failure::Internal {
        diagnostic: boundary_diagnostic(codes::SC_OBSERVABILITY_BINDING_INTERNAL, message).into(),
    }
}
pub(crate) fn closed() -> Failure {
    Failure::Closed {
        diagnostic: boundary_diagnostic(
            codes::SC_OBSERVABILITY_BINDING_CLOSED,
            "backend admission is closed",
        )
        .into(),
    }
}
pub(crate) fn full(code: &str) -> Failure {
    Failure::QueueFull {
        diagnostic: boundary_diagnostic(code, "bounded native operation capacity is occupied")
            .into(),
    }
}
pub(crate) fn waiters_full() -> Failure {
    full(codes::SC_OBSERVABILITY_BINDING_WAITERS_FULL)
}
pub(crate) fn timeout() -> Failure {
    Failure::Timeout {
        diagnostic: boundary_diagnostic(
            codes::SC_OBSERVABILITY_BINDING_TIMEOUT,
            "operation observation deadline elapsed",
        )
        .into(),
        operation: "native_operation".into(),
    }
}

/// The native operation represented by an observer.  Observer deadlines are
/// local to the binding runtime, but their diagnostics retain the canonical
/// operation family so a flush timeout cannot be mistaken for shutdown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OperationKind {
    Query,
    Flush,
    Shutdown,
}

fn context(
    code: ErrorCode,
    message: impl Into<String>,
    remediation: Remediation,
) -> Box<ErrorContext> {
    Box::new(ErrorContext::new(code, message, remediation))
}

pub(crate) fn init_runtime(
    message: impl Into<String>,
    source: Box<dyn std::error::Error + Send + Sync + 'static>,
) -> native::v2::InitError {
    let context = context(
        ErrorCode::new_static(codes::SC_OBSERVABILITY_BINDING_COORDINATOR_START_FAILED),
        message,
        Remediation::recoverable(
            "restore thread or runtime resources and retry initialization",
            std::iter::empty::<String>(),
        ),
    );
    let context = Box::new((*context).source(source));
    native::v2::InitError::Runtime { context }
}

pub(crate) fn init_runtime_internal(message: impl Into<String>) -> native::v2::InitError {
    native::v2::InitError::Runtime {
        context: context(
            ErrorCode::new_static(codes::SC_OBSERVABILITY_BINDING_INTERNAL),
            message,
            Remediation::not_recoverable(
                "restart the binding runtime after resolving the poisoned state",
            ),
        ),
    }
}

fn registry_remediation(code: &'static str) -> Remediation {
    let entry = codes::REGISTRY
        .iter()
        .find(|entry| entry.code == code)
        .expect("binding runtime codes are registered by the DTO boundary");
    Remediation::recoverable(entry.remediation, std::iter::empty::<String>())
}

pub(crate) fn subscriber_closed(message: impl Into<String>) -> native::v2::SubscriberError {
    native::v2::SubscriberError::classified_subscriber(
        context(
            ErrorCode::new_static(codes::SC_OBSERVABILITY_BINDING_CLOSED),
            message,
            registry_remediation(codes::SC_OBSERVABILITY_BINDING_CLOSED),
        ),
        native::v2::FailureClassification::Closed,
    )
}

pub(crate) fn subscriber_waiters_full(message: impl Into<String>) -> native::v2::SubscriberError {
    native::v2::SubscriberError::classified_subscriber(
        context(
            ErrorCode::new_static(codes::SC_OBSERVABILITY_BINDING_WAITERS_FULL),
            message,
            registry_remediation(codes::SC_OBSERVABILITY_BINDING_WAITERS_FULL),
        ),
        native::v2::FailureClassification::QueueFull,
    )
}

pub(crate) fn shutdown_drain(message: impl Into<String>) -> native::v2::ShutdownError {
    native::v2::ShutdownError::classified_drain(
        context(
            ErrorCode::new_static(codes::SC_OBSERVABILITY_BINDING_INTERNAL),
            message,
            Remediation::not_recoverable("inspect the retained shutdown diagnostic"),
        ),
        native::v2::FailureClassification::Internal,
    )
}

pub(crate) fn observer_timeout(kind: OperationKind) -> Failure {
    let _ = kind;
    timeout()
}

pub(crate) fn duration(value: Duration) -> Result<(), Failure> {
    if value > MAX_OBSERVATION_TIMEOUT || !value.subsec_nanos().is_multiple_of(1_000_000) {
        return Err(sc_observability_dto::invalid_input(
            "timeout_ms",
            format!("timeout must be integral milliseconds in 0..{MAX_OBSERVATION_TIMEOUT_MS}"),
        ));
    }
    Ok(())
}

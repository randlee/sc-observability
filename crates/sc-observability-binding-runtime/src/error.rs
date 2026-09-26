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

pub(crate) fn event_validation(context: Box<ErrorContext>) -> native::v2::EventError {
    native::v2::EventError::Validation { context }
}

pub(crate) fn init_configuration_code(
    code: ErrorCode,
    message: impl Into<String>,
) -> native::v2::InitError {
    native::v2::InitError::Configuration {
        context: context(
            code,
            message,
            Remediation::recoverable(
                "correct the binding runtime configuration",
                std::iter::empty::<String>(),
            ),
        ),
    }
}

pub(crate) fn init_runtime(
    message: impl Into<String>,
    source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
) -> native::v2::InitError {
    let context = context(
        ErrorCode::new_static(codes::SC_OBSERVABILITY_BINDING_COORDINATOR_START_FAILED),
        message,
        Remediation::recoverable(
            "restore thread or runtime resources and retry initialization",
            std::iter::empty::<String>(),
        ),
    );
    let context = if let Some(source) = source {
        Box::new((*context).source(source))
    } else {
        context
    };
    native::v2::InitError::Runtime { context }
}

pub(crate) fn flush_drain(source: Box<native::ErrorContext>) -> native::v2::FlushError {
    let (code, message, remediation) = {
        let diagnostic = source.diagnostic();
        (
            diagnostic.code.clone(),
            diagnostic.message.clone(),
            diagnostic.remediation.clone(),
        )
    };
    let sink = native::v2::LogSinkError::Flush { context: source };
    let context = Box::new(ErrorContext::new(code, message, remediation).source(Box::new(sink)));
    native::v2::FlushError::Drain { context }
}

pub(crate) fn subscriber(code: &str, message: impl Into<String>) -> native::v2::SubscriberError {
    native::v2::SubscriberError::Subscriber {
        context: context(
            ErrorCode::new_owned(code),
            message,
            Remediation::recoverable(
                "retry the subscription after the backend is available",
                std::iter::empty::<String>(),
            ),
        ),
    }
}

pub(crate) fn shutdown_timeout(message: impl Into<String>) -> native::v2::ShutdownError {
    native::v2::ShutdownError::Timeout {
        context: context(
            ErrorCode::new_static(codes::SC_OBSERVABILITY_BINDING_TIMEOUT),
            message,
            Remediation::recoverable(
                "wait for the existing shutdown operation",
                std::iter::empty::<String>(),
            ),
        ),
    }
}

pub(crate) fn shutdown_drain(message: impl Into<String>) -> native::v2::ShutdownError {
    native::v2::ShutdownError::Drain {
        context: context(
            ErrorCode::new_static(codes::SC_OBSERVABILITY_BINDING_INTERNAL),
            message,
            Remediation::not_recoverable("inspect the retained shutdown diagnostic"),
        ),
    }
}

pub(crate) fn observer_timeout(kind: OperationKind) -> Failure {
    match kind {
        OperationKind::Flush => {
            let error = native::v2::FlushError::Drain {
                context: context(
                    ErrorCode::new_static(codes::SC_OBSERVABILITY_BINDING_TIMEOUT),
                    "flush observation deadline elapsed",
                    Remediation::recoverable(
                        "wait for the existing flush operation",
                        std::iter::empty::<String>(),
                    ),
                ),
            };
            Failure::Timeout {
                diagnostic: Box::new(crate::conversion::diagnostic(error.diagnostic())),
                operation: "flush".into(),
            }
        }
        OperationKind::Shutdown => {
            let error = shutdown_timeout("shutdown observation deadline elapsed");
            Failure::Timeout {
                diagnostic: Box::new(crate::conversion::diagnostic(error.diagnostic())),
                operation: "shutdown".into(),
            }
        }
        OperationKind::Query => timeout(),
    }
}

pub(crate) fn duration(value: Duration) -> Result<(), Failure> {
    if value > Duration::from_secs(60) || !value.subsec_nanos().is_multiple_of(1_000_000) {
        return Err(sc_observability_dto::invalid_input(
            "timeout_ms",
            "timeout must be integral milliseconds in 0..60000",
        ));
    }
    Ok(())
}

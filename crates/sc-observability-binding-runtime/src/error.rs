use sc_observability_dto::{Failure, boundary_diagnostic, error_codes as codes};
use std::time::Duration;

pub(crate) fn internal(message: impl Into<String>) -> Failure {
    Failure::Internal {
        diagnostic: boundary_diagnostic(codes::SC_OBSERVABILITY_BINDING_INTERNAL, message),
    }
}
pub(crate) fn closed() -> Failure {
    Failure::Closed {
        diagnostic: boundary_diagnostic(
            codes::SC_OBSERVABILITY_BINDING_CLOSED,
            "backend admission is closed",
        ),
    }
}
pub(crate) fn full(code: &str) -> Failure {
    Failure::QueueFull {
        diagnostic: boundary_diagnostic(code, "bounded native operation capacity is occupied"),
    }
}
pub(crate) fn waiters_full() -> Failure {
    full(codes::SC_OBSERVABILITY_BINDING_WAITERS_FULL)
}
pub(crate) fn start_failed(message: impl Into<String>) -> Failure {
    Failure::Unavailable {
        diagnostic: boundary_diagnostic(
            codes::SC_OBSERVABILITY_BINDING_COORDINATOR_START_FAILED,
            message,
        ),
    }
}
pub(crate) fn timeout() -> Failure {
    Failure::Timeout {
        diagnostic: boundary_diagnostic(
            codes::SC_OBSERVABILITY_BINDING_TIMEOUT,
            "operation observation deadline elapsed",
        ),
        operation: "native_operation".into(),
    }
}
pub(crate) fn duration(value: Duration) -> Result<(), Failure> {
    if value > Duration::from_secs(60) || value.subsec_nanos() % 1_000_000 != 0 {
        return Err(sc_observability_dto::invalid_input(
            "timeout_ms",
            "timeout must be integral milliseconds in 0..60000",
        ));
    }
    Ok(())
}

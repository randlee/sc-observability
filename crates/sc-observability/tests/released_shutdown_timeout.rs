//! Released shutdown-timeout error-surface compatibility coverage.

use sc_observability::{ErrorContext, LogError, TryLogError, error_codes};
use sc_observability_types::Remediation;
use sc_observability_types::typed::{LogFailure, TryLogFailure};

fn timeout_context() -> Box<ErrorContext> {
    Box::new(ErrorContext::new(
        error_codes::LOGGER_SHUTDOWN_TIMED_OUT,
        "writer thread did not stop within 10ms",
        Remediation::recoverable("wait for writer shutdown", ["retry after shutdown"]),
    ))
}

#[test]
fn released_shutdown_timeout_variants_preserve_the_typed_contract() {
    assert!(matches!(
        LogFailure::from(LogError::ShutdownTimedOut(timeout_context())),
        LogFailure::ShutdownTimedOut(_)
    ));
    assert!(matches!(
        TryLogFailure::from(TryLogError::ShutdownTimedOut(timeout_context())),
        TryLogFailure::ShutdownTimedOut(_)
    ));
}

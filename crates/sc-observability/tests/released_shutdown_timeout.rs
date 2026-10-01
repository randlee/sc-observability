//! Defensive released shutdown-timeout variant-preservation coverage.
//!
//! A real writer shutdown timeout is retained by `Logger<Stopped>::health()`;
//! this fixture covers only the released error-wrapper conversions retained for
//! defensive compatibility.

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
fn defensive_shutdown_timeout_variants_preserve_typed_conversions() {
    assert!(matches!(
        LogFailure::from(LogError::ShutdownTimedOut(timeout_context())),
        LogFailure::ShutdownTimedOut(_)
    ));
    assert!(matches!(
        TryLogFailure::from(TryLogError::ShutdownTimedOut(timeout_context())),
        TryLogFailure::ShutdownTimedOut(_)
    ));
}

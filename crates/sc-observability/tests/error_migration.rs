use std::error::Error;
use std::fmt;

use sc_observability::error_codes;
use sc_observability::v2::{EventError, LogSinkError, ShutdownError};
use sc_observability::{ErrorContext, Remediation};
use sc_observability_types::DiagnosticInfo;

#[derive(Debug)]
struct NativeCause(&'static str);

impl fmt::Display for NativeCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for NativeCause {}

fn context(code: sc_observability::ErrorCode, message: &'static str) -> Box<ErrorContext> {
    Box::new(
        ErrorContext::new(
            code,
            message,
            Remediation::recoverable("retry", ["inspect the diagnostic source"]),
        )
        .source(Box::new(NativeCause("native cause"))),
    )
}

fn assert_diagnostic_and_source(error: &(impl DiagnosticInfo + Error), code: &str) {
    let diagnostic = error.diagnostic();
    assert_eq!(diagnostic.code.as_str(), code);
    assert!(matches!(
        diagnostic.remediation,
        Remediation::Recoverable { .. }
    ));

    let context = Error::source(error).expect("canonical error exposes its context");
    let cause = context
        .source()
        .and_then(|source| source.downcast_ref::<NativeCause>())
        .expect("canonical context preserves the native source type");
    assert_eq!(cause.0, "native cause");
}

#[test]
fn event_variants_preserve_the_canonical_cause_mapping() {
    let validation = EventError::Validation {
        context: context(error_codes::LOGGER_INVALID_EVENT, "event validation failed"),
    };
    assert_diagnostic_and_source(&validation, "SC_OBSERVABILITY_LOGGER_INVALID_EVENT");

    let routing = EventError::Routing {
        context: context(error_codes::LOGGER_WRITER_DEGRADED, "writer routing failed"),
    };
    assert_diagnostic_and_source(&routing, "SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED");
}

#[test]
fn shutdown_timeout_and_drain_remain_distinct_named_variants() {
    let timeout = ShutdownError::Timeout {
        context: context(
            error_codes::LOGGER_SHUTDOWN_TIMED_OUT,
            "shutdown deadline exceeded",
        ),
    };
    assert_diagnostic_and_source(&timeout, "SC_OBSERVABILITY_LOGGER_SHUTDOWN_TIMED_OUT");

    let drain = ShutdownError::Drain {
        context: context(error_codes::LOGGER_WRITER_DEGRADED, "shutdown drain failed"),
    };
    assert_diagnostic_and_source(&drain, "SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED");
    assert!(matches!(timeout, ShutdownError::Timeout { .. }));
    assert!(matches!(drain, ShutdownError::Drain { .. }));
}

#[test]
fn sink_write_variant_preserves_remediation_and_source() {
    let error = LogSinkError::Write {
        context: context(error_codes::LOGGER_SINK_WRITE_FAILED, "sink write failed"),
    };
    assert_diagnostic_and_source(&error, "SC_OBSERVABILITY_LOGGER_SINK_WRITE_FAILED");
}

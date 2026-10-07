//! Typed result envelope and single panic containment boundary.
use super::error_projection::{
    admission_variant, config_variant, delivery_report, delivery_variant, submission_path,
    submission_variant,
};
use sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL;
use sc_observability_types::otlp::submission::{FlushReport, TelemetryClientError};
use sc_observability_types::{ErrorContext, Remediation};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum FailureKind {
    Submission,
    Admission,
    Delivery,
    Config,
    Internal,
    Unknown,
}
#[derive(Serialize)]
struct Failure {
    kind: FailureKind,
    variant: &'static str,
    code: String,
    message: String,
    path: Option<String>,
    report: Option<FlushReport>,
    remediation: Option<Remediation>,
    cause: Option<String>,
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WireResult<T> {
    Ok { value: T },
    Error { error: Box<Failure> },
}

pub(super) fn ok(value: impl Serialize) -> String {
    serde_json::to_string(&WireResult::Ok { value }).unwrap_or_else(|error| {
        internal(
            "serialization",
            "failed to serialize telemetry result",
            Some(error.to_string()),
        )
    })
}
fn encode_failure(error: Failure) -> String {
    // All fields have infallible JSON serializers (strings, counts, enums).
    serde_json::to_string(&WireResult::<()>::Error {
        error: Box::new(error),
    })
    .expect("telemetry failure is JSON serializable")
}
pub(super) fn internal(variant: &'static str, message: &str, cause: Option<String>) -> String {
    encode_failure(Failure {
        kind: FailureKind::Internal,
        variant,
        code: SC_OBSERVABILITY_BINDING_INTERNAL.into(),
        message: message.into(),
        path: None,
        report: None,
        remediation: Some(Remediation::not_recoverable(
            "Report the native binding failure with its cause; retry using a new handle.",
        )),
        cause,
    })
}
pub(super) fn failure(error: &TelemetryClientError) -> String {
    let (kind, variant, context, path, report) = match error {
        TelemetryClientError::Submission(e) => (
            FailureKind::Submission,
            submission_variant(e),
            Some(e.context()),
            submission_path(e),
            None,
        ),
        TelemetryClientError::Admission(e) => (
            FailureKind::Admission,
            admission_variant(e),
            Some(e.context()),
            None,
            None,
        ),
        TelemetryClientError::Delivery(e) => (
            FailureKind::Delivery,
            delivery_variant(e),
            Some(e.context()),
            None,
            delivery_report(e),
        ),
        TelemetryClientError::Config(e) => (
            FailureKind::Config,
            config_variant(e),
            Some(e.context()),
            None,
            None,
        ),
        _ => (FailureKind::Unknown, "unknown", None, None, None),
    };
    let cause = if variant == "unknown" {
        Some(format!("{error:?}"))
    } else {
        context.and_then(context_cause)
    };
    encode_failure(Failure {
        kind,
        variant,
        code: error.code().as_str().into(),
        message: error.to_string(),
        path: path.map(str::to_owned),
        report: report.cloned(),
        remediation: context.map(|c| c.diagnostic().remediation.clone()),
        cause,
    })
}
fn context_cause(context: &ErrorContext) -> Option<String> {
    let mut causes = context
        .diagnostic()
        .cause
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let mut source = std::error::Error::source(context);
    while let Some(error) = source {
        causes.push(error.to_string());
        source = error.source();
    }
    (!causes.is_empty()).then(|| causes.join(": "))
}
pub(super) fn contained<T>(
    action: impl FnOnce() -> Result<T, TelemetryClientError>,
) -> Result<T, String> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(action)) {
        Ok(result) => result.map_err(|error| failure(&error)),
        Err(payload) => {
            let cause = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).into()))
                .unwrap_or_else(|| "non-string panic payload".into());
            Err(internal(
                "panic",
                "native telemetry operation panicked",
                Some(cause),
            ))
        }
    }
}
pub(super) fn result<T: Serialize>(
    action: impl FnOnce() -> Result<T, TelemetryClientError>,
) -> String {
    contained(|| action().map(ok)).unwrap_or_else(|error| error)
}

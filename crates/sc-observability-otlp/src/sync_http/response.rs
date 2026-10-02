//! Bounded OTLP/JSON export acknowledgements for the pinned signal protocols.

use std::io::Read;

use sc_observability_types::{ErrorContext, Remediation, v2::ExportError};
use serde_json::Value;

use super::implementation::SubmissionRoute;
use crate::{
    constants::MAX_OTLP_RESPONSE_BYTES, error_codes::OTLP_EXPORT_TERMINAL, lifecycle::Signal,
};

pub(super) fn check(
    response: reqwest::blocking::Response,
    route: SubmissionRoute,
) -> Result<(), ExportError> {
    // Read one extra byte to distinguish a complete bounded body from truncation.
    // The request timeout continues to bound reads, including chunked responses.
    let mut bytes = Vec::new();
    response
        .take(MAX_OTLP_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| failure("could not read OTLP export response", Some(Box::new(error))))?;
    if bytes.len() as u64 > MAX_OTLP_RESPONSE_BYTES {
        return Err(failure(
            "OTLP export response exceeds the 64 KiB limit",
            None,
        ));
    }
    parse(&bytes, route)
}

fn parse(bytes: &[u8], route: SubmissionRoute) -> Result<(), ExportError> {
    // Existing collectors also acknowledge successful exports with an empty body.
    if bytes.is_empty() {
        return Ok(());
    }
    let response: Value = serde_json::from_slice(bytes)
        .map_err(|error| failure("invalid OTLP export response JSON", Some(Box::new(error))))?;
    let response = response
        .as_object()
        .ok_or_else(|| failure("OTLP export response must be an object", None))?;
    let Some(partial) = response.get("partialSuccess") else {
        return Ok(());
    };
    // Proto-JSON null is the same as an absent message/field.
    if partial.is_null() {
        return Ok(());
    }
    let partial = partial
        .as_object()
        .ok_or_else(|| failure("OTLP partialSuccess must be an object", None))?;
    let field = match route {
        SubmissionRoute::Signal(Signal::Logs) => "rejectedLogRecords",
        SubmissionRoute::Signal(Signal::Traces) => "rejectedSpans",
        SubmissionRoute::Signal(Signal::Metrics) => "rejectedDataPoints",
        SubmissionRoute::Signal(Signal::Profiles) | SubmissionRoute::Profiles => "rejectedProfiles",
        SubmissionRoute::Signal(_) => {
            return Err(failure("unsupported OTLP response signal", None));
        }
    };
    let count = match partial.get(field) {
        None | Some(Value::Null) => Some(0),
        Some(Value::String(count)) => count.parse::<i64>().ok(),
        Some(count) => count.as_i64(),
    };
    let count = count.ok_or_else(|| failure("invalid OTLP rejected count", None))?;
    if count < 0 {
        return Err(failure("negative OTLP rejected count", None));
    }
    if count > 0 {
        // Do not copy arbitrary collector text (potential secrets) into diagnostics.
        return Err(failure(
            &format!("collector rejected telemetry: {field}={count}"),
            None,
        ));
    }
    Ok(())
}

fn failure(message: &str, source: Option<Box<dyn std::error::Error + Send + Sync>>) -> ExportError {
    let mut context = ErrorContext::new(
        OTLP_EXPORT_TERMINAL,
        message,
        Remediation::recoverable(
            "inspect collector diagnostics and rejected telemetry; do not replay an entire partially accepted batch",
            [] as [&str; 0],
        ),
    );
    if let Some(source) = source {
        context = context.source(source);
    }
    ExportError::TerminalExportFailure {
        context: Box::new(context),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acknowledgements_obey_each_signal_count() {
        for (signal, field) in [
            (Signal::Logs, "rejectedLogRecords"),
            (Signal::Traces, "rejectedSpans"),
            (Signal::Metrics, "rejectedDataPoints"),
            (Signal::Profiles, "rejectedProfiles"),
        ] {
            let route = SubmissionRoute::Signal(signal);
            for count in [serde_json::json!(1), serde_json::json!("1")] {
                let response = serde_json::json!({"partialSuccess": {field: count}});
                assert!(matches!(
                    parse(response.to_string().as_bytes(), route),
                    Err(ExportError::TerminalExportFailure { .. })
                ));
            }
            for response in [
                "",
                "{}",
                "{\"partialSuccess\":null}",
                "{\"partialSuccess\":{\"errorMessage\":\"warning\"}}",
            ] {
                assert!(parse(response.as_bytes(), route).is_ok());
            }
            let warning =
                serde_json::json!({"partialSuccess": {field: "0", "errorMessage":"warning"}});
            assert!(parse(warning.to_string().as_bytes(), route).is_ok());
        }
    }

    #[test]
    fn malformed_acknowledgements_are_terminal_not_retryable() {
        for response in [
            "oops",
            "[]",
            "{\"partialSuccess\":1}",
            "{\"partialSuccess\":{\"rejectedLogRecords\":-1}}",
            "{\"partialSuccess\":{\"rejectedLogRecords\":\"invalid\"}}",
        ] {
            assert!(matches!(
                parse(response.as_bytes(), SubmissionRoute::Signal(Signal::Logs)),
                Err(ExportError::TerminalExportFailure { .. })
            ));
        }
    }
}

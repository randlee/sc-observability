//! Conformance coverage for the existing non-exhaustive error surface.
use super::{config, dto};
use sc_observability_types::otlp::submission::*;
use sc_observability_types::{ErrorContext, Remediation, error_codes};
fn context() -> Box<ErrorContext> {
    Box::new(
        ErrorContext::new(
            error_codes::SC_OBSERVABILITY_SUBMIT_VALIDATION,
            "detail",
            Remediation::not_recoverable("correct input"),
        )
        .cause("original cause"),
    )
}
fn assert_projection(error: &TelemetryClientError, kind: &str, variant: &str) {
    let value: serde_json::Value = serde_json::from_str(&dto::failure(error)).unwrap();
    assert_eq!(value["error"]["kind"], kind);
    assert_eq!(value["error"]["variant"], variant);
    assert_eq!(value["error"]["code"], error.code().as_str());
    assert_eq!(value["error"]["cause"], "original cause");
    assert_eq!(
        value["error"]["remediation"]["justification"],
        "correct input"
    );
}
#[test]
fn every_current_error_variant_has_a_specific_projection() {
    assert_projection(
        &SubmissionError::InvalidJson { context: context() }.into(),
        "submission",
        "invalid_json",
    );
    assert_projection(
        &SubmissionError::UnsupportedVersion {
            found: EnvelopeVersion::new(2),
            context: context(),
        }
        .into(),
        "submission",
        "unsupported_version",
    );
    assert_projection(
        &SubmissionError::EmptySubmission { context: context() }.into(),
        "submission",
        "empty_submission",
    );
    assert_projection(
        &SubmissionError::Validation {
            path: "logs".into(),
            context: context(),
        }
        .into(),
        "submission",
        "validation",
    );
    assert_projection(
        &SubmissionError::ValueOutOfRange {
            path: "logs".into(),
            context: context(),
        }
        .into(),
        "submission",
        "value_out_of_range",
    );
    assert_projection(
        &SubmissionError::CorrelationConflict { context: context() }.into(),
        "submission",
        "correlation_conflict",
    );
    assert_projection(
        &SubmissionError::TimingConflict { context: context() }.into(),
        "submission",
        "timing_conflict",
    );
    assert_projection(
        &SubmissionError::DictionaryReference {
            path: "profiles".into(),
            context: context(),
        }
        .into(),
        "submission",
        "dictionary_reference",
    );
}
#[test]
fn every_admission_variant_has_a_specific_projection() {
    assert_projection(
        &AdmissionError::StoreUnavailable { context: context() }.into(),
        "admission",
        "store_unavailable",
    );
    assert_projection(
        &AdmissionError::DiskBoundExceeded { context: context() }.into(),
        "admission",
        "disk_bound_exceeded",
    );
    assert_projection(
        &AdmissionError::Persistence { context: context() }.into(),
        "admission",
        "persistence",
    );
    assert_projection(
        &AdmissionError::SchemaTooNew {
            found: 2,
            supported: 1,
            context: context(),
        }
        .into(),
        "admission",
        "schema_too_new",
    );
    assert_projection(
        &AdmissionError::Closed { context: context() }.into(),
        "admission",
        "closed",
    );
}
#[test]
fn every_delivery_and_config_variant_has_a_specific_projection() {
    assert_projection(
        &DeliveryError::DeadlineExceeded {
            report: FlushReport::default(),
            context: context(),
        }
        .into(),
        "delivery",
        "deadline_exceeded",
    );
    assert_projection(
        &DeliveryError::TerminalFailure {
            report: FlushReport::default(),
            context: context(),
        }
        .into(),
        "delivery",
        "terminal_failure",
    );
    assert_projection(
        &TelemetryConfigError::ConfigFile {
            path: "bad.yaml".into(),
            context: context(),
        }
        .into(),
        "config",
        "config_file",
    );
    assert_projection(
        &TelemetryConfigError::MissingField {
            field: "store_path",
            context: context(),
        }
        .into(),
        "config",
        "missing_field",
    );
    assert_projection(
        &TelemetryConfigError::InvalidField {
            field: "endpoint",
            context: context(),
        }
        .into(),
        "config",
        "invalid_field",
    );
    assert_projection(
        &TelemetryConfigError::UnsupportedCombination {
            backend: ExporterBackendId::SyncHttp,
            signal: Signal::Logs,
            representation: Representation::Log,
            context: context(),
        }
        .into(),
        "config",
        "unsupported_combination",
    );
}
#[test]
fn invalid_arguments_keep_registered_code_and_parse_cause() {
    for error in [
        config::config("{").unwrap_err(),
        config::status_query("{").unwrap_err(),
        config::status_query("\"unsupported\"").unwrap_err(),
    ] {
        let value: serde_json::Value = serde_json::from_str(&dto::failure(&error)).unwrap();
        assert_eq!(
            value["error"]["code"],
            error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID.as_str()
        );
        assert!(!value["error"]["cause"].as_str().unwrap().is_empty());
    }
}
struct Bad;
impl serde::Serialize for Bad {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("serialize detail"))
    }
}

#[test]
fn panic_payload_and_serialization_failure_are_distinct() {
    let raw = dto::result::<()>(|| panic!("panic detail"));
    let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(value["error"]["variant"], "panic");
    assert_eq!(value["error"]["cause"], "panic detail");
    let value: serde_json::Value = serde_json::from_str(&dto::ok(Bad)).unwrap();
    assert_eq!(value["error"]["variant"], "serialization");
    assert_eq!(value["error"]["cause"], "serialize detail");
    assert_eq!(
        value["error"]["code"],
        sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL
    );
}

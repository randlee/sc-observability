//! Canonical diagnostic and native failure-classification conversions.
use super::*;

/// Projects a canonical diagnostic without formatting or serializing native source objects.
///
/// # Errors
/// Rejects overlarge diagnostics/details and invalid timestamps using existing boundary codes.
pub fn from_canonical_diagnostic(
    value: &core::Diagnostic,
) -> Result<CanonicalDiagnosticDto, Failure> {
    let diagnostic = Diagnostic {
        at: value.timestamp.to_string(),
        code: value.code.as_str().into(),
        message: value.message.clone(),
        remediation: value.remediation.clone().into(),
    };
    validate_diagnostic(&diagnostic, "diagnostic")?;
    if value
        .cause
        .as_ref()
        .is_some_and(|v| v.len() > MAX_DIAGNOSTIC_FIELD_BYTES)
        || value
            .docs
            .as_ref()
            .is_some_and(|v| v.len() > MAX_DIAGNOSTIC_FIELD_BYTES)
    {
        return Err(invalid_input(
            "diagnostic",
            format!("diagnostic metadata exceeds {MAX_DIAGNOSTIC_FIELD_BYTES} bytes"),
        ));
    }
    let details = from_fields(value.details.clone())?;
    let result = CanonicalDiagnosticDto {
        diagnostic,
        cause: value.cause.clone(),
        docs: value.docs.clone(),
        details,
    };
    let wire = checked(serde_json::to_value(&result), "diagnostic")?;
    measure(&wire, 0)?;
    if checked(serde_json::to_vec(&wire), "diagnostic")?.len() > MAX_WIRE_PAYLOAD_BYTES {
        return Err(invalid_input(
            "diagnostic",
            format!("diagnostic exceeds {MAX_WIRE_PAYLOAD_BYTES} bytes"),
        ));
    }
    Ok(result)
}

/// Builds either wire failure representation from the single native classification authority.
///
/// The generic diagnostic keeps the retained and canonical wire shapes distinct while
/// ensuring their native classification mapping cannot drift.
fn classified_failure<D>(
    diagnostic: Box<D>,
    classification: core::v2::FailureClassification,
) -> Failure<D> {
    match classification {
        core::v2::FailureClassification::Validation { field } => Failure::Validation {
            diagnostic,
            field: field.into(),
        },
        core::v2::FailureClassification::QueueFull => Failure::QueueFull { diagnostic },
        core::v2::FailureClassification::Closed => Failure::Closed { diagnostic },
        core::v2::FailureClassification::Unavailable => Failure::Unavailable { diagnostic },
        core::v2::FailureClassification::Io => Failure::Io { diagnostic },
        core::v2::FailureClassification::Timeout { operation } => Failure::Timeout {
            diagnostic,
            operation: operation.into(),
        },
        core::v2::FailureClassification::Cancelled { operation } => Failure::Cancelled {
            diagnostic,
            operation: operation.into(),
        },
        core::v2::FailureClassification::Internal => Failure::Internal { diagnostic },
    }
}

/// Builds the canonical wire failure from the single native classification authority.
fn canonical_failure(
    diagnostic: Box<CanonicalDiagnosticDto>,
    classification: core::v2::FailureClassification,
) -> CanonicalFailureDto {
    classified_failure(diagnostic, classification)
}

/// Builds the legacy wire failure from the single native classification authority.
///
/// The legacy and canonical representations intentionally remain distinct so
/// their JSON shapes stay lossless and backwards compatible.
/// Returns `validate_diagnostic`'s validation failure when the diagnostic is invalid or oversized.
pub fn failure_from_diagnostic(
    diagnostic: Diagnostic,
    classification: core::v2::FailureClassification,
) -> Failure {
    if let Err(error) = validate_diagnostic(&diagnostic, "response.error") {
        return error;
    }
    classified_failure(Box::new(diagnostic), classification)
}

/// Projects a native diagnostic through its native-owned wire classification.
#[must_use]
pub fn failure_from_classification(
    value: &core::Diagnostic,
    classification: core::v2::FailureClassification,
) -> Failure {
    failure_from_diagnostic(
        Diagnostic {
            at: value.timestamp.to_string(),
            code: value.code.as_str().into(),
            message: value.message.clone(),
            remediation: value.remediation.clone().into(),
        },
        classification,
    )
}

macro_rules! canonical_projection {
    ($ty:ident) => {
        impl TryFrom<&core::v2::$ty> for CanonicalFailureDto {
            type Error = Failure;
            fn try_from(value: &core::v2::$ty) -> Result<Self, Self::Error> {
                Ok(canonical_failure(
                    Box::new(from_canonical_diagnostic(value.diagnostic())?),
                    value.failure_classification(),
                ))
            }
        }
    };
}
canonical_projection!(IdentityError);
canonical_projection!(InitError);
canonical_projection!(EventError);
canonical_projection!(FlushError);
canonical_projection!(ShutdownError);
canonical_projection!(ProjectionError);
canonical_projection!(SubscriberError);
canonical_projection!(LogSinkError);
canonical_projection!(MetricModelError);
canonical_projection!(ConfigFailure);
canonical_projection!(ExportError);

impl TryFrom<&core::v2::TelemetryError> for CanonicalFailureDto {
    type Error = Failure;
    fn try_from(value: &core::v2::TelemetryError) -> Result<Self, Self::Error> {
        match value {
            core::v2::TelemetryError::ExportFailure(error) => Self::try_from(error),
            core::v2::TelemetryError::Event(error) => Self::try_from(error),
            core::v2::TelemetryError::Shutdown { context } => Ok(Self::Closed {
                diagnostic: Box::new(from_canonical_diagnostic(context.diagnostic())?),
            }),
            _ => Ok(canonical_failure(
                Box::new(from_canonical_diagnostic(value.diagnostic())?),
                value.failure_classification(),
            )),
        }
    }
}

#[cfg(test)]
mod export_projection_tests {
    use super::*;

    fn diagnostic() -> Diagnostic {
        Diagnostic {
            at: "2024-01-01T00:00:00Z".into(),
            code: "TEST_FAILURE".into(),
            message: "test failure".into(),
            remediation: RemediationDto::Recoverable {
                steps: vec!["retry".into()],
            },
        }
    }

    #[test]
    fn classification_constructor_preserves_all_native_categories_and_wire_shapes() {
        for (classification, kind, field, operation) in [
            (
                core::v2::FailureClassification::validation("request.payload"),
                "validation",
                Some("request.payload"),
                None,
            ),
            (
                core::v2::FailureClassification::QueueFull,
                "queue_full",
                None,
                None,
            ),
            (
                core::v2::FailureClassification::Closed,
                "closed",
                None,
                None,
            ),
            (
                core::v2::FailureClassification::Unavailable,
                "unavailable",
                None,
                None,
            ),
            (core::v2::FailureClassification::Io, "io", None, None),
            (
                core::v2::FailureClassification::timeout("flush"),
                "timeout",
                None,
                Some("flush"),
            ),
            (
                core::v2::FailureClassification::Cancelled {
                    operation: "shutdown",
                },
                "cancelled",
                None,
                Some("shutdown"),
            ),
            (
                core::v2::FailureClassification::Internal,
                "internal",
                None,
                None,
            ),
        ] {
            let legacy = failure_from_diagnostic(diagnostic(), classification);
            let canonical = canonical_failure(
                Box::new(CanonicalDiagnosticDto {
                    diagnostic: diagnostic(),
                    cause: Some("canonical-only cause".into()),
                    docs: None,
                    details: std::collections::BTreeMap::new(),
                }),
                classification,
            );
            let legacy_wire = serde_json::to_value(legacy).expect("legacy serializes");
            let canonical_wire = serde_json::to_value(canonical).expect("canonical serializes");

            assert_eq!(legacy_wire["kind"], kind);
            assert_eq!(canonical_wire["kind"], kind);
            assert_eq!(legacy_wire["field"].as_str(), field);
            assert_eq!(canonical_wire["field"].as_str(), field);
            assert_eq!(legacy_wire["operation"].as_str(), operation);
            assert_eq!(canonical_wire["operation"].as_str(), operation);
            assert_eq!(legacy_wire["code"], canonical_wire["code"]);
            assert_eq!(legacy_wire.get("cause"), None);
            assert_eq!(canonical_wire["cause"], "canonical-only cause");
        }
    }

    #[test]
    fn failure_from_diagnostic_rejects_oversized_diagnostic() {
        let mut oversized = diagnostic();
        oversized.message = "x".repeat(MAX_DIAGNOSTIC_FIELD_BYTES + 1);

        let failure = failure_from_diagnostic(oversized, core::v2::FailureClassification::Internal);

        let Failure::Validation { diagnostic, field } = failure else {
            panic!("oversized diagnostic should return a validation failure");
        };
        assert_eq!(
            diagnostic.code,
            error_codes::SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE
        );
        assert_eq!(field, "response.error");
        assert_eq!(
            diagnostic.remediation,
            RemediationDto::Recoverable {
                steps: vec![
                    "Reduce remote diagnostic text or remediation steps to the documented bounds"
                        .into()
                ]
            }
        );
    }

    #[test]
    fn queue_full_projects_to_queue_full_with_its_diagnostic() {
        let error = core::v2::ExportError::QueueFull {
            context: Box::new(core::ErrorContext::new(
                core::error_codes::otlp::OTLP_QUEUE_FULL,
                "queue full in unit regression",
                core::Remediation::recoverable("retry later", ["inspect health"]),
            )),
        };

        let projected = CanonicalFailureDto::try_from(&error).expect("conversion succeeds");
        assert!(matches!(projected, CanonicalFailureDto::QueueFull { .. }));
        let diagnostic = projected.diagnostic();
        assert_eq!(diagnostic.diagnostic.code, "OTLP_QUEUE_FULL");
        assert_eq!(
            diagnostic.diagnostic.message,
            "queue full in unit regression"
        );
        assert!(matches!(
            &diagnostic.diagnostic.remediation,
            RemediationDto::Recoverable { steps }
                if steps.iter().any(|step| step == "inspect health")
        ));
    }
}

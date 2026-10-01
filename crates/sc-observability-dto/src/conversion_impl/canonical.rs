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

/// Builds the canonical wire failure from the single native classification authority.
fn canonical_failure(
    diagnostic: Box<CanonicalDiagnosticDto>,
    classification: core::v2::FailureClassification,
) -> CanonicalFailureDto {
    match classification {
        core::v2::FailureClassification::Validation { field } => CanonicalFailureDto::Validation {
            diagnostic,
            field: field.into(),
        },
        core::v2::FailureClassification::QueueFull => CanonicalFailureDto::QueueFull { diagnostic },
        core::v2::FailureClassification::Closed => CanonicalFailureDto::Closed { diagnostic },
        core::v2::FailureClassification::Unavailable => {
            CanonicalFailureDto::Unavailable { diagnostic }
        }
        core::v2::FailureClassification::Io => CanonicalFailureDto::Io { diagnostic },
        core::v2::FailureClassification::Timeout { operation } => CanonicalFailureDto::Timeout {
            diagnostic,
            operation: operation.into(),
        },
        core::v2::FailureClassification::Cancelled { operation } => {
            CanonicalFailureDto::Cancelled {
                diagnostic,
                operation: operation.into(),
            }
        }
        core::v2::FailureClassification::Internal => CanonicalFailureDto::Internal { diagnostic },
    }
}

/// Builds the legacy wire failure from the single native classification authority.
///
/// The legacy and canonical representations intentionally remain distinct so
/// their JSON shapes stay lossless and backwards compatible.
pub fn failure_from_diagnostic(
    diagnostic: Diagnostic,
    classification: core::v2::FailureClassification,
) -> Failure {
    if let Err(error) = validate_diagnostic(&diagnostic, "response.error") {
        return error;
    }
    let diagnostic = Box::new(diagnostic);
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
                core::v2::FailureClassification::Internal,
            )),
        }
    }
}

#[cfg(test)]
mod export_projection_tests {
    use super::*;

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

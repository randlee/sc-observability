//! Diagnostic and canonical-envelope conversions.
use super::*;

/// Preserves the canonical diagnostic payload in the retained stored-diagnostic shape.
fn stored_diagnostic_dto(v: CanonicalDiagnosticDto) -> StoredDiagnosticDto {
    StoredDiagnosticDto {
        timestamp: v.diagnostic.at,
        code: v.diagnostic.code,
        message: v.diagnostic.message,
        remediation: v.diagnostic.remediation,
        cause: v.cause,
        docs: v.docs,
        details: v.details,
    }
}
fn native_diagnostic(value: StoredDiagnosticDto) -> Result<core::Diagnostic, Failure> {
    let remediation = match value.remediation {
        RemediationDto::Recoverable { steps } => core::Remediation::Recoverable {
            steps: core::RecoverableSteps::all(steps),
        },
        RemediationDto::NotRecoverable { justification } => {
            core::Remediation::not_recoverable(justification)
        }
    };
    let result = core::Diagnostic {
        timestamp: timestamp(value.timestamp, "diagnostic.timestamp")?,
        code: core::ErrorCode::new_owned(value.code),
        message: value.message,
        remediation,
        cause: value.cause,
        docs: value.docs,
        details: value
            .details
            .into_iter()
            .map(|(key, value)| Ok((key, to_value(value, "details", false, 0)?)))
            .collect::<Result<_, Failure>>()?,
    };
    from_canonical_diagnostic(&result)?;
    Ok(result)
}
/// Decodes the compatible operational envelope while retaining additive canonical metadata.
///
/// # Errors
/// Rejects malformed envelopes, overlarge metadata and invalid tagged payloads.
/// Unknown error kinds remain `UnknownRemote`, never a successful result.
pub fn decode_canonical_envelope<T: DeserializeOwned>(
    value: Value,
) -> Result<CanonicalWireEnvelope<T>, Failure> {
    decode_envelope_shell(
        value,
        |value| checked(serde_json::from_value(value), "response"),
        |diagnostic: &Diagnostic| validate_diagnostic(diagnostic, "response.error"),
        |value| decode(value, "response"),
        |value, _, tag| {
            Ok(CanonicalFailureDto::UnknownRemote {
                diagnostic: Box::new(decode(value, "response")?),
                remote_kind: tag.into(),
            })
        },
        |failure| {
            native_diagnostic(stored_diagnostic_dto(failure.diagnostic().clone())).map(|_| ())
        },
    )
}

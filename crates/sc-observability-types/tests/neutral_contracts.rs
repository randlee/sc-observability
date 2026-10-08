//! Contract tests for consumers of the staged canonical surface.
use sc_observability_types::v2::*;
use sc_observability_types::{DiagnosticInfo, ErrorContext, Remediation, error_codes};
use serde_json::json;
use std::error::Error;

#[derive(Debug, thiserror::Error)]
#[error("sentinel source")]
struct Sentinel(u64);

fn context(code: sc_observability_types::ErrorCode) -> Box<ErrorContext> {
    Box::new(
        ErrorContext::new(
            code,
            "failure",
            Remediation::recoverable("correct input", ["retry"]),
        )
        .docs("https://example.test/recovery")
        .cause("bounded cause")
        .detail("config_field", json!("queue_capacity"))
        .source(Box::new(Sentinel(42))),
    )
}

#[test]
fn canonical_error_variants_preserve_context() {
    macro_rules! check {
        ($name:ident::$variant:ident, $code:expr) => {{ check!($name::$variant, error_codes::VALUE_VALIDATION_FAILED, $code) }};
        ($name:ident::$variant:ident, $context_code:expr, $code:expr) => {{
            let original = context($context_code);
            let diagnostic = original.diagnostic().clone();
            let pointer = std::ptr::from_ref(&*original);
            let error = $name::$variant { context: original };
            assert_eq!(std::ptr::from_ref(error.context()), pointer);
            assert_eq!(DiagnosticInfo::diagnostic(&error), &diagnostic);
            assert_eq!(error.code(), $code);
            assert_eq!(
                error.to_string(),
                "failure: bounded cause; caused by: sentinel source"
            );
            let original_source = error.source().unwrap().downcast_ref::<Sentinel>().unwrap();
            assert_eq!(original_source.0, 42);
            let saved = serde_json::to_value(&error).unwrap();
            assert!(saved.get("kind").is_some());
            assert_eq!(
                saved["context"]["diagnostic"]["remediation"]["kind"],
                "recoverable"
            );
            assert_eq!(std::ptr::from_ref(&*error.into_context()), pointer);
        }};
    }
    check!(
        IdentityError::Process,
        error_codes::IDENTITY_RESOLUTION_FAILED
    );
    check!(InitError::Configuration, error_codes::DIAGNOSTIC_INVALID);
    check!(InitError::Runtime, error_codes::DIAGNOSTIC_INVALID);
    check!(EventError::Validation, error_codes::DIAGNOSTIC_INVALID);
    check!(EventError::Routing, error_codes::DIAGNOSTIC_INVALID);
    check!(FlushError::Drain, error_codes::DIAGNOSTIC_INVALID);
    check!(ShutdownError::Timeout, error_codes::DIAGNOSTIC_INVALID);
    check!(ShutdownError::Drain, error_codes::DIAGNOSTIC_INVALID);
    check!(ProjectionError::Projection, error_codes::DIAGNOSTIC_INVALID);
    check!(SubscriberError::Subscriber, error_codes::DIAGNOSTIC_INVALID);
    check!(LogSinkError::Write, error_codes::DIAGNOSTIC_INVALID);
    check!(LogSinkError::Flush, error_codes::DIAGNOSTIC_INVALID);
}

#[test]
fn cloned_canonical_error_preserves_source_identity() {
    let original = FlushError::Drain {
        context: context(error_codes::VALUE_VALIDATION_FAILED),
    };
    let cloned = original.clone();
    let original_source = std::error::Error::source(original.context())
        .expect("original context must retain its source");
    let cloned_source =
        std::error::Error::source(cloned.context()).expect("cloned context must retain its source");

    assert!(std::ptr::eq(original_source, cloned_source));
    assert_eq!(original_source.to_string(), "sentinel source");
    assert_eq!(cloned_source.to_string(), "sentinel source");
}

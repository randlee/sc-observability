//! Canonical 2.0 errors staged without changing the retained 1.x surface.
use crate::{Diagnostic, DiagnosticInfo, ErrorContext, sealed};
use serde::{Deserialize, Serialize};

/// Wire failure category and its native-owned, operation-specific context.
///
/// This deliberately records only stable contract terms.  Boundary crates use
/// it to select their legacy or canonical wire representation without
/// re-deciding a native error's meaning or inventing a field name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClassification {
    /// Invalid caller-controlled input identified by its real input field.
    Validation {
        /// Exact native input field that failed validation.
        field: &'static str,
    },
    /// A bounded queue cannot accept more work.
    QueueFull,
    /// The requested lifecycle operation is no longer available.
    Closed,
    /// A dependency or runtime is unavailable.
    Unavailable,
    /// An I/O or transport operation failed.
    Io,
    /// An operation exceeded its deadline.
    Timeout {
        /// Exact native operation that exceeded its deadline.
        operation: &'static str,
    },
    /// An operation was cancelled during controlled shutdown.
    Cancelled {
        /// Exact native operation cancelled during shutdown.
        operation: &'static str,
    },
    /// A local invariant or unexpected implementation failure occurred.
    Internal,
}

impl FailureClassification {
    /// Creates a validation classification for the exact native input field.
    #[must_use]
    pub const fn validation(field: &'static str) -> Self {
        Self::Validation { field }
    }

    /// Creates a timeout classification for the exact native operation.
    #[must_use]
    pub const fn timeout(operation: &'static str) -> Self {
        Self::Timeout { operation }
    }
}

// Every case owns the original context, including its typed source and backtrace.
macro_rules! context_error {
    ($name:ident, $($variant:ident => $code:expr),+ $(,)?) => {
        #[doc = concat!("Canonical ", stringify!($name), " with preserved diagnostic context.")]
        #[non_exhaustive]
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub enum $name {
            $(
                #[doc = concat!(stringify!($variant), " failure; see the canonical cause mapping.")]
                    #[error(transparent)]
                    $variant {
                        /// Diagnostic, remediation, source and construction backtrace.
                        context: Box<ErrorContext>,
                    },
            )+
        }
        impl $name {
            /// Returns the original error context without reconstruction.
            #[must_use]
            pub fn context(&self) -> &ErrorContext {
                match self { $(Self::$variant { context } => context,)+ }
            }
            /// Returns the preserved diagnostic.
            #[must_use]
            pub fn diagnostic(&self) -> &Diagnostic { self.context().diagnostic() }
            /// Returns the stable machine-readable code fixed for this variant.
            #[must_use]
            pub fn code(&self) -> crate::ErrorCode {
                match self { $(Self::$variant { .. } => $code,)+ }
            }
            /// Takes the original boxed context, preserving source identity and backtrace.
            #[must_use]
            pub fn into_context(self) -> Box<ErrorContext> {
                match self { $(Self::$variant { context } => context,)+ }
            }
        }
        impl sealed::Sealed for $name {}
        impl DiagnosticInfo for $name {
            fn diagnostic(&self) -> &Diagnostic { self.diagnostic() }
        }
    };
}

context_error!(IdentityError, Process => crate::error_codes::IDENTITY_RESOLUTION_FAILED);

impl IdentityError {
    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub const fn failure_classification(&self) -> FailureClassification {
        FailureClassification::validation("process")
    }
}

context_error!(
    InitError,
    Configuration => crate::error_codes::DIAGNOSTIC_INVALID,
    Runtime => crate::error_codes::DIAGNOSTIC_INVALID
);

impl InitError {
    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub const fn failure_classification(&self) -> FailureClassification {
        match self {
            Self::Configuration { .. } => FailureClassification::validation("configuration"),
            Self::Runtime { .. } => FailureClassification::Unavailable,
        }
    }
}

context_error!(
    EventError,
    Validation => crate::error_codes::DIAGNOSTIC_INVALID,
    Routing => crate::error_codes::DIAGNOSTIC_INVALID
);

impl EventError {
    /// Creates a routing failure with its native-owned wire classification.
    #[must_use]
    pub fn classified_routing(
        mut context: Box<ErrorContext>,
        classification: FailureClassification,
    ) -> Self {
        context.set_failure_classification(classification);
        Self::Routing { context }
    }

    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub fn failure_classification(&self) -> FailureClassification {
        match self {
            Self::Validation { .. } => FailureClassification::validation("event"),
            Self::Routing { context } => context
                .failure_classification()
                .unwrap_or(FailureClassification::Unavailable),
        }
    }
}

context_error!(FlushError, Drain => crate::error_codes::DIAGNOSTIC_INVALID);

impl FlushError {
    /// Creates a drain failure with its native-owned wire classification.
    #[must_use]
    pub fn classified_drain(
        mut context: Box<ErrorContext>,
        classification: FailureClassification,
    ) -> Self {
        context.set_failure_classification(classification);
        Self::Drain { context }
    }

    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub fn failure_classification(&self) -> FailureClassification {
        self.context()
            .failure_classification()
            .unwrap_or(FailureClassification::Io)
    }
}

context_error!(
    ShutdownError,
    Timeout => crate::error_codes::DIAGNOSTIC_INVALID,
    Drain => crate::error_codes::DIAGNOSTIC_INVALID
);

impl ShutdownError {
    /// Creates a drain failure with its native-owned wire classification.
    #[must_use]
    pub fn classified_drain(
        mut context: Box<ErrorContext>,
        classification: FailureClassification,
    ) -> Self {
        context.set_failure_classification(classification);
        Self::Drain { context }
    }

    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub fn failure_classification(&self) -> FailureClassification {
        match self {
            Self::Timeout { .. } => FailureClassification::timeout("shutdown"),
            Self::Drain { context } => context
                .failure_classification()
                .unwrap_or(FailureClassification::Io),
        }
    }
}

context_error!(ProjectionError, Projection => crate::error_codes::DIAGNOSTIC_INVALID);

impl ProjectionError {
    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub const fn failure_classification(&self) -> FailureClassification {
        FailureClassification::validation("projection")
    }
}

context_error!(SubscriberError, Subscriber => crate::error_codes::DIAGNOSTIC_INVALID);

impl SubscriberError {
    /// Creates a subscriber failure with its native-owned wire classification.
    #[must_use]
    pub fn classified_subscriber(
        mut context: Box<ErrorContext>,
        classification: FailureClassification,
    ) -> Self {
        context.set_failure_classification(classification);
        Self::Subscriber { context }
    }

    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub const fn failure_classification(&self) -> FailureClassification {
        match self {
            Self::Subscriber { context } => match context.failure_classification() {
                Some(classification) => classification,
                None => FailureClassification::Unavailable,
            },
        }
    }
}

context_error!(
    LogSinkError,
    Write => crate::error_codes::DIAGNOSTIC_INVALID,
    Flush => crate::error_codes::DIAGNOSTIC_INVALID
);

impl LogSinkError {
    /// Returns the native-owned wire failure classification.
    #[must_use]
    pub const fn failure_classification(&self) -> FailureClassification {
        FailureClassification::Io
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(message: &str) -> ErrorContext {
        ErrorContext::new(
            crate::error_codes::DIAGNOSTIC_INVALID,
            message,
            crate::Remediation::not_recoverable("inspect the native failure"),
        )
    }

    fn helper_lost_context(message: &str) -> ErrorContext {
        ErrorContext::new(
            crate::ErrorCode::new_static("SC_OBSERVABILITY_LOG_HELPER_LOST"),
            message,
            crate::Remediation::not_recoverable("inspect the lost helper"),
        )
        .source(Box::new(std::io::Error::other("retained helper cause")))
    }

    #[test]
    fn drain_classification_prefers_explicit_native_value_and_keeps_its_source() {
        let drain = ErrorContext::new(
            crate::error_codes::DIAGNOSTIC_INVALID,
            "flush helper failed",
            crate::Remediation::not_recoverable("inspect the helper"),
        )
        .source(Box::new(std::io::Error::other("helper transport failed")));
        let error =
            FlushError::classified_drain(Box::new(drain), FailureClassification::Unavailable);

        let source = std::error::Error::source(error.context())
            .and_then(|source| source.downcast_ref::<std::io::Error>())
            .expect("drain retains its source");
        assert_eq!(source.to_string(), "helper transport failed");
        assert_eq!(
            error.failure_classification(),
            FailureClassification::Unavailable
        );
    }

    #[test]
    fn routing_classification_retains_the_explicit_native_value() {
        let error = EventError::classified_routing(
            Box::new(context("writer shutdown timed out")),
            FailureClassification::timeout("shutdown"),
        );

        assert_eq!(
            error.failure_classification(),
            FailureClassification::timeout("shutdown")
        );
    }

    #[test]
    fn routing_classification_preserves_context_and_released_wire_contract() {
        for classification in [
            FailureClassification::QueueFull,
            FailureClassification::timeout("shutdown"),
        ] {
            let context = Box::new(
                ErrorContext::new(
                    crate::error_codes::DIAGNOSTIC_INVALID,
                    "routing failure",
                    crate::Remediation::recoverable("inspect the writer", ["retry the operation"]),
                )
                .source(Box::new(std::io::Error::other("retained routing source"))),
            );
            let context_pointer = std::ptr::from_ref(context.as_ref());
            let plain = EventError::Routing {
                context: context.clone(),
            };
            let error = EventError::classified_routing(context, classification);

            assert_eq!(error.failure_classification(), classification);
            assert_eq!(
                plain.failure_classification(),
                FailureClassification::Unavailable
            );
            assert!(std::ptr::eq(error.context(), context_pointer));
            assert_eq!(error.diagnostic(), plain.diagnostic());
            assert!(std::ptr::eq(
                std::error::Error::source(error.context()).expect("classified source retained"),
                std::error::Error::source(plain.context()).expect("plain source retained"),
            ));
            assert_eq!(
                std::error::Error::source(error.context())
                    .expect("classified source retained")
                    .to_string(),
                "retained routing source"
            );
            assert_eq!(
                error, plain,
                "classification is not released equality state"
            );
            let wire = serde_json::to_vec(&error).expect("classified event serializes");
            assert_eq!(
                wire,
                serde_json::to_vec(&plain).expect("plain event serializes")
            );
            let decoded: EventError = serde_json::from_slice(&wire).expect("event deserializes");
            assert_eq!(
                decoded.failure_classification(),
                FailureClassification::Unavailable
            );
            assert_eq!(decoded.diagnostic(), error.diagnostic());
        }
    }

    #[test]
    fn subscriber_classification_preserves_context_and_released_wire_contract() {
        const fn subscriber_classification(error: &SubscriberError) -> FailureClassification {
            error.failure_classification()
        }

        for classification in [
            FailureClassification::Closed,
            FailureClassification::QueueFull,
        ] {
            let context = Box::new(
                ErrorContext::new(
                    crate::error_codes::DIAGNOSTIC_INVALID,
                    "subscriber registration failed",
                    crate::Remediation::recoverable("retry registration", ["wait for capacity"]),
                )
                .source(Box::new(std::io::Error::other(
                    "retained subscriber source",
                ))),
            );
            let context_pointer = std::ptr::from_ref(context.as_ref());
            let plain = SubscriberError::Subscriber {
                context: context.clone(),
            };
            let error = SubscriberError::classified_subscriber(context, classification);

            assert_eq!(subscriber_classification(&error), classification);
            assert_eq!(
                plain.failure_classification(),
                FailureClassification::Unavailable
            );
            assert!(std::ptr::eq(error.context(), context_pointer));
            assert_eq!(error.diagnostic(), plain.diagnostic());
            assert!(std::ptr::eq(
                std::error::Error::source(error.context()).expect("classified source retained"),
                std::error::Error::source(plain.context()).expect("plain source retained"),
            ));
            assert_eq!(
                std::error::Error::source(error.context())
                    .expect("classified source retained")
                    .to_string(),
                "retained subscriber source"
            );
            assert_eq!(
                error, plain,
                "classification is not released equality state"
            );
            assert_eq!(error.clone().failure_classification(), classification);
            let wire = serde_json::to_vec(&error).expect("classified subscriber serializes");
            assert_eq!(
                wire,
                serde_json::to_vec(&plain).expect("plain subscriber serializes")
            );
            let decoded: SubscriberError =
                serde_json::from_slice(&wire).expect("subscriber deserializes");
            assert_eq!(
                decoded.failure_classification(),
                FailureClassification::Unavailable
            );
            assert_eq!(decoded.diagnostic(), error.diagnostic());
        }
    }

    #[test]
    fn helper_lost_drain_retains_internal_classification_and_source() {
        let flush = FlushError::classified_drain(
            Box::new(helper_lost_context("flush helper lost")),
            FailureClassification::Internal,
        );
        let shutdown = ShutdownError::classified_drain(
            Box::new(helper_lost_context("shutdown helper lost")),
            FailureClassification::Internal,
        );

        macro_rules! assert_helper_lost {
            ($error:expr, $message:literal) => {
                assert_eq!(
                    $error.diagnostic().code,
                    crate::ErrorCode::new_static("SC_OBSERVABILITY_LOG_HELPER_LOST")
                );
                assert_eq!($error.diagnostic().message, $message);
                assert_eq!(
                    $error.failure_classification(),
                    FailureClassification::Internal
                );
                assert_eq!(
                    std::error::Error::source($error.context()).map(ToString::to_string),
                    Some("retained helper cause".to_owned())
                );
            };
        }

        assert_helper_lost!(flush, "flush helper lost");
        assert_helper_lost!(shutdown, "shutdown helper lost");
    }

    #[test]
    fn unclassified_drain_falls_back_to_io() {
        let flush = FlushError::Drain {
            context: Box::new(context("drain").source(Box::new(std::io::Error::other("io")))),
        };
        let shutdown = ShutdownError::Drain {
            context: Box::new(context("drain without source")),
        };

        assert_eq!(flush.failure_classification(), FailureClassification::Io);
        assert_eq!(shutdown.failure_classification(), FailureClassification::Io);
    }
}

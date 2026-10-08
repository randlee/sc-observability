//! Selective re-exports of the unmodified official OpenTelemetry 0.33.0 types
//! named by the synchronous client and log-sink contracts (ADR-023).
//!
//! Frontends reach these types through this crate so they declare no direct
//! OpenTelemetry dependency. Native Tokio hosts depend on the official crates
//! directly and need none of these paths.

/// OpenTelemetry API types used by the native contracts.
pub mod api {
    #[doc(inline)]
    pub use opentelemetry::{InstrumentationScope, Key, KeyValue, Value};

    /// Native log data model types.
    pub mod logs {
        #[doc(inline)]
        pub use opentelemetry::logs::{AnyValue, LogRecord, Severity};
    }

    /// Native trace identity and completed-span component types.
    pub mod trace {
        #[doc(inline)]
        pub use opentelemetry::trace::{
            Event, Link, SpanContext, SpanId, SpanKind, Status, TraceFlags, TraceId, TraceState,
        };
    }

    /// Native metric instrument types.
    pub mod metrics {
        #[doc(inline)]
        pub use opentelemetry::metrics::{Meter, MeterProvider};
    }
}

/// OpenTelemetry SDK types used by the native contracts.
pub mod sdk {
    #[doc(inline)]
    pub use opentelemetry_sdk::Resource;

    /// Native SDK completed-span types.
    pub mod trace {
        #[doc(inline)]
        pub use opentelemetry_sdk::trace::{
            IdGenerator, RandomIdGenerator, SpanData, SpanEvents, SpanLinks,
        };
    }

    /// Native SDK logger provider and record types.
    pub mod logs {
        #[doc(inline)]
        pub use opentelemetry_sdk::logs::{SdkLogRecord, SdkLoggerProvider};
    }

    /// Native SDK operation result types.
    pub mod error {
        #[doc(inline)]
        pub use opentelemetry_sdk::error::{OTelSdkError, OTelSdkResult};
    }
}

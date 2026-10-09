//! Selective re-exports of the unmodified official OpenTelemetry 0.33.0 types
//! named by the synchronous client, log-sink and Tokio exporter contracts
//! (ADR-023).
//!
//! Hosts and frontends reach these types through this crate so they declare no
//! direct OpenTelemetry dependency. The `otlp` module (feature
//! `tokio-exporter`) adds the official OTLP/HTTP exporters.

/// OpenTelemetry API types used by the native contracts.
pub mod api {
    #[doc(inline)]
    pub use opentelemetry::{Context, InstrumentationScope, Key, KeyValue, Value};

    /// Native log data model types.
    pub mod logs {
        #[doc(inline)]
        pub use opentelemetry::logs::{AnyValue, LogRecord, Logger, LoggerProvider, Severity};
    }

    /// Native trace identity and completed-span component types.
    pub mod trace {
        #[doc(inline)]
        pub use opentelemetry::trace::{
            Event, Link, Span, SpanContext, SpanId, SpanKind, Status, TraceContextExt, TraceFlags,
            TraceId, TraceState, Tracer, TracerProvider,
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
            BatchSpanProcessor, IdGenerator, RandomIdGenerator, SdkTracerProvider, SpanData,
            SpanEvents, SpanLinks,
        };
    }

    /// Native SDK logger provider and record types.
    pub mod logs {
        #[doc(inline)]
        pub use opentelemetry_sdk::logs::{BatchLogProcessor, SdkLogRecord, SdkLoggerProvider};
    }

    /// Native SDK metric reader and provider types.
    pub mod metrics {
        #[doc(inline)]
        pub use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
    }

    /// Native SDK operation result types.
    pub mod error {
        #[doc(inline)]
        pub use opentelemetry_sdk::error::{OTelSdkError, OTelSdkResult};
    }
}

/// Official OTLP/HTTP exporters for the default SDK batch processors.
#[cfg(feature = "tokio-exporter")]
pub mod otlp {
    #[doc(inline)]
    pub use opentelemetry_otlp::{
        LogExporter, MetricExporter, Protocol, SpanExporter, WithExportConfig,
    };
}

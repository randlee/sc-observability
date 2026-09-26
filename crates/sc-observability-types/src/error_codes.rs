//! Stable `ErrorCode` registry for `sc-observability-types`.

use crate::ErrorCode;

/// Generic validation error code used for shared value-type failures.
pub const VALUE_VALIDATION_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_TYPES_VALUE_VALIDATION_FAILED");
/// Error code for invalid W3C trace identifier values.
pub const TRACE_ID_INVALID: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_TYPES_TRACE_ID_INVALID");
/// Error code for invalid W3C span identifier values.
pub const SPAN_ID_INVALID: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_TYPES_SPAN_ID_INVALID");
/// Error code for process identity resolution failures.
pub const IDENTITY_RESOLUTION_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED");
/// Error code for generic diagnostic construction or validation failures.
pub const DIAGNOSTIC_INVALID: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_TYPES_DIAGNOSTIC_INVALID");
/// Error code for invalid historical/follow query inputs.
pub const SC_LOG_QUERY_INVALID_QUERY: ErrorCode =
    ErrorCode::new_static("SC_LOG_QUERY_INVALID_QUERY");
/// Error code for query I/O failures.
pub const SC_LOG_QUERY_IO: ErrorCode = ErrorCode::new_static("SC_LOG_QUERY_IO");
/// Error code for query decode failures.
pub const SC_LOG_QUERY_DECODE: ErrorCode = ErrorCode::new_static("SC_LOG_QUERY_DECODE");
/// Error code for query unavailability failures.
pub const SC_LOG_QUERY_UNAVAILABLE: ErrorCode = ErrorCode::new_static("SC_LOG_QUERY_UNAVAILABLE");
/// Error code for query shutdown failures.
pub const SC_LOG_QUERY_SHUTDOWN: ErrorCode = ErrorCode::new_static("SC_LOG_QUERY_SHUTDOWN");
/// Error code for a mutation attempted while logger shutdown is in progress.
pub const LEVEL_STOPPING: ErrorCode = ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_STOPPING");
/// Error code for a mutation attempted after the logger lifetime ended.
pub const LEVEL_STOPPED: ErrorCode = ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_STOPPED");
/// Error code for a requested level below the configured baseline.
pub const LEVEL_BELOW_BASELINE: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_BELOW_BASELINE");
/// Error code for a requested level unavailable in the current build.
pub const LEVEL_UNSUPPORTED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_UNSUPPORTED");
/// Error code for unavailable or poisoned runtime level state.
pub const LEVEL_STATE_UNAVAILABLE: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_STATE_UNAVAILABLE");
/// Error code for a runtime-level revision that cannot be incremented.
pub const LEVEL_REVISION_EXHAUSTED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LEVEL_REVISION_EXHAUSTED");

/// Enumerable registry of all public `sc-observability-types` error codes.
pub const ALL: &[ErrorCode] = &[
    SC_METRIC_INVALID_HISTOGRAM,
    SC_METRIC_INVALID_TEMPORALITY,
    SC_METRIC_INVALID_INTERVAL,
    SC_METRIC_NON_FINITE,
    otlp::OTLP_CONFIG_ZERO_DURATION,
    otlp::OTLP_CONFIG_DURATION_OVERFLOW,
    otlp::OTLP_CONFIG_BOUND_ORDER,
    otlp::OTLP_CONFIG_JITTER_PERCENT,
    otlp::OTLP_CONFIG_QUEUE_CAPACITY,
    otlp::OTLP_CONFIG_QUEUE_BYTE_CAPACITY,
    otlp::OTLP_CONFIG_FIELD_NOT_APPLICABLE,
    otlp::OTLP_CONFIG_INSECURE_TRANSPORT_REJECTED,
    otlp::OTLP_CONFIG_INVALID_ENDPOINT,
    otlp::OTLP_CONFIG_INVALID_HEADER,
    otlp::OTLP_TRANSPORT_CONSTRUCTION_FAILED,
    otlp::OTLP_UNSUPPORTED_BACKEND,
    otlp::OTLP_UNSUPPORTED_PROTOCOL,
    otlp::OTLP_TOKIO_RUNTIME_REQUIRED,
    otlp::OTLP_BLOCKING_BACKEND_IN_ASYNC_CONTEXT,
    otlp::OTLP_ASYNC_LIFECYCLE_REQUIRED,
    otlp::OTLP_RUNTIME_TERMINATED,
    otlp::OTLP_LIFECYCLE_TIMEOUT,
    otlp::OTLP_QUEUE_FULL,
    otlp::OTLP_WORKER_TERMINATED,
    otlp::OTLP_SHUTDOWN_CANCELLED_RETRY,
    otlp::OTLP_RETRY_DEADLINE_EXHAUSTED,
    otlp::OTLP_HTTP_STATUS_TERMINAL,
    otlp::OTLP_RETRY_ATTEMPTS_EXHAUSTED,
    otlp::OTLP_EXPORT_TERMINAL,
    otlp::OTLP_TELEMETRY_SHUTDOWN,
    VALUE_VALIDATION_FAILED,
    TRACE_ID_INVALID,
    SPAN_ID_INVALID,
    IDENTITY_RESOLUTION_FAILED,
    DIAGNOSTIC_INVALID,
    SC_LOG_QUERY_INVALID_QUERY,
    SC_LOG_QUERY_IO,
    SC_LOG_QUERY_DECODE,
    SC_LOG_QUERY_UNAVAILABLE,
    SC_LOG_QUERY_SHUTDOWN,
    LEVEL_STOPPING,
    LEVEL_STOPPED,
    LEVEL_BELOW_BASELINE,
    LEVEL_UNSUPPORTED,
    LEVEL_STATE_UNAVAILABLE,
    LEVEL_REVISION_EXHAUSTED,
];

/// Stable code for sc metric invalid histogram.
pub const SC_METRIC_INVALID_HISTOGRAM: ErrorCode =
    ErrorCode::new_static("SC_METRIC_INVALID_HISTOGRAM");
/// Stable code for sc metric invalid temporality.
pub const SC_METRIC_INVALID_TEMPORALITY: ErrorCode =
    ErrorCode::new_static("SC_METRIC_INVALID_TEMPORALITY");
/// Stable code for sc metric invalid interval.
pub const SC_METRIC_INVALID_INTERVAL: ErrorCode =
    ErrorCode::new_static("SC_METRIC_INVALID_INTERVAL");
/// Stable code for sc metric non finite.
pub const SC_METRIC_NON_FINITE: ErrorCode = ErrorCode::new_static("SC_METRIC_NON_FINITE");

/// Types-owned transport diagnostic codes; transport crates re-export this registry.
pub mod otlp {
    use crate::ErrorCode;
    /// Required duration is zero. Recovery: provide a positive value.
    pub const OTLP_CONFIG_ZERO_DURATION: ErrorCode =
        ErrorCode::new_static("OTLP_CONFIG_ZERO_DURATION");
    /// Milliseconds cannot convert safely. Recovery: reduce the field.
    pub const OTLP_CONFIG_DURATION_OVERFLOW: ErrorCode =
        ErrorCode::new_static("OTLP_CONFIG_DURATION_OVERFLOW");
    /// Resolved ordering rule fails. Recovery: correct the named explicit/defaulted fields.
    pub const OTLP_CONFIG_BOUND_ORDER: ErrorCode = ErrorCode::new_static("OTLP_CONFIG_BOUND_ORDER");
    /// Jitter exceeds 100. Recovery: use `0..=100`.
    pub const OTLP_CONFIG_JITTER_PERCENT: ErrorCode =
        ErrorCode::new_static("OTLP_CONFIG_JITTER_PERCENT");
    /// Queue capacity is outside `1..=65_536`. Recovery: choose a bounded capacity.
    pub const OTLP_CONFIG_QUEUE_CAPACITY: ErrorCode =
        ErrorCode::new_static("OTLP_CONFIG_QUEUE_CAPACITY");
    /// Zero, overflow or aggregate byte bound above 64 mib. Recovery: choose 1..=64 MiB (default 16 MiB).
    pub const OTLP_CONFIG_QUEUE_BYTE_CAPACITY: ErrorCode =
        ErrorCode::new_static("OTLP_CONFIG_QUEUE_BYTE_CAPACITY");
    /// Field is inapplicable to disabled transport or the selected backend. Recovery: omit it, enable transport, or select its applicable backend.
    pub const OTLP_CONFIG_FIELD_NOT_APPLICABLE: ErrorCode =
        ErrorCode::new_static("OTLP_CONFIG_FIELD_NOT_APPLICABLE");
    /// Selected backend does not implement the requested insecure verification override. Recovery: disable the override or choose an explicitly supporting backend.
    pub const OTLP_CONFIG_INSECURE_TRANSPORT_REJECTED: ErrorCode =
        ErrorCode::new_static("OTLP_CONFIG_INSECURE_TRANSPORT_REJECTED");
    /// Endpoint url syntax is invalid. Recovery: provide a valid endpoint URL.
    pub const OTLP_CONFIG_INVALID_ENDPOINT: ErrorCode =
        ErrorCode::new_static("OTLP_CONFIG_INVALID_ENDPOINT");
    /// Header/auth syntax or credential placement is invalid. Recovery: correct the header/auth configuration.
    pub const OTLP_CONFIG_INVALID_HEADER: ErrorCode =
        ErrorCode::new_static("OTLP_CONFIG_INVALID_HEADER");
    /// Ca/auth/client/provider/legacy-worker initialization failed. Recovery: correct the bounded typed source and reconstruct.
    pub const OTLP_TRANSPORT_CONSTRUCTION_FAILED: ErrorCode =
        ErrorCode::new_static("OTLP_TRANSPORT_CONSTRUCTION_FAILED");
    /// Feature/backend unavailable. Recovery: enable/select a supported backend.
    pub const OTLP_UNSUPPORTED_BACKEND: ErrorCode =
        ErrorCode::new_static("OTLP_UNSUPPORTED_BACKEND");
    /// Protocol invalid for backend. Recovery: select a matrix-supported protocol.
    pub const OTLP_UNSUPPORTED_PROTOCOL: ErrorCode =
        ErrorCode::new_static("OTLP_UNSUPPORTED_PROTOCOL");
    /// Sdk construction lacks an entered tokio runtime. Recovery: construct inside the host runtime.
    pub const OTLP_TOKIO_RUNTIME_REQUIRED: ErrorCode =
        ErrorCode::new_static("OTLP_TOKIO_RUNTIME_REQUIRED");
    /// Legacy synchronous lifecycle entered tokio; construction preserves this condition as the redacted source of `TransportConstructionFailed`. Recovery: use a plain thread or async lifecycle.
    pub const OTLP_BLOCKING_BACKEND_IN_ASYNC_CONTEXT: ErrorCode =
        ErrorCode::new_static("OTLP_BLOCKING_BACKEND_IN_ASYNC_CONTEXT");
    /// Sdk synchronous completion requested. Recovery: await the typed async operation.
    pub const OTLP_ASYNC_LIFECYCLE_REQUIRED: ErrorCode =
        ErrorCode::new_static("OTLP_ASYNC_LIFECYCLE_REQUIRED");
    /// Host runtime ended before completion. Recovery: keep the runtime alive through awaited shutdown.
    pub const OTLP_RUNTIME_TERMINATED: ErrorCode = ErrorCode::new_static("OTLP_RUNTIME_TERMINATED");
    /// Monotonic lifecycle deadline elapsed. Recovery: inspect terminal health and transport/provider.
    pub const OTLP_LIFECYCLE_TIMEOUT: ErrorCode = ErrorCode::new_static("OTLP_LIFECYCLE_TIMEOUT");
    /// Bounded admission queue saturated. Recovery: preserve fail-open behavior and inspect health.
    pub const OTLP_QUEUE_FULL: ErrorCode = ErrorCode::new_static("OTLP_QUEUE_FULL");
    /// Sdk dispatcher or legacy worker terminated unexpectedly. Recovery: correct the terminal cause and construct a new instance.
    pub const OTLP_WORKER_TERMINATED: ErrorCode = ErrorCode::new_static("OTLP_WORKER_TERMINATED");
    /// Shutdown cancelled a retryable pre-barrier legacy sequence. Recovery: inspect terminal health; resend only if duplicates are acceptable.
    pub const OTLP_SHUTDOWN_CANCELLED_RETRY: ErrorCode =
        ErrorCode::new_static("OTLP_SHUTDOWN_CANCELLED_RETRY");
    /// No legacy sequence budget remains. Recovery: increase the validated sequence bound or restore collector health.
    pub const OTLP_RETRY_DEADLINE_EXHAUSTED: ErrorCode =
        ErrorCode::new_static("OTLP_RETRY_DEADLINE_EXHAUSTED");
    /// Collector returned a non-retryable http status. Recovery: correct request/auth/config before retrying.
    pub const OTLP_HTTP_STATUS_TERMINAL: ErrorCode =
        ErrorCode::new_static("OTLP_HTTP_STATUS_TERMINAL");
    /// Legacy maximum attempts ended before success. Recovery: restore collector health or adjust the validated policy.
    pub const OTLP_RETRY_ATTEMPTS_EXHAUSTED: ErrorCode =
        ErrorCode::new_static("OTLP_RETRY_ATTEMPTS_EXHAUSTED");
    /// Sdk or legacy provider returned a terminal export failure. Recovery: inspect the preserved source and collector state.
    pub const OTLP_EXPORT_TERMINAL: ErrorCode = ErrorCode::new_static("OTLP_EXPORT_TERMINAL");
    /// Emit was attempted after shutdown began. Recovery: construct a new telemetry instance.
    pub const OTLP_TELEMETRY_SHUTDOWN: ErrorCode = ErrorCode::new_static("OTLP_TELEMETRY_SHUTDOWN");
    /// Complete types-owned transport code inventory.
    pub const ALL: &[ErrorCode] = &[
        OTLP_CONFIG_ZERO_DURATION,
        OTLP_CONFIG_DURATION_OVERFLOW,
        OTLP_CONFIG_BOUND_ORDER,
        OTLP_CONFIG_JITTER_PERCENT,
        OTLP_CONFIG_QUEUE_CAPACITY,
        OTLP_CONFIG_QUEUE_BYTE_CAPACITY,
        OTLP_CONFIG_FIELD_NOT_APPLICABLE,
        OTLP_CONFIG_INSECURE_TRANSPORT_REJECTED,
        OTLP_CONFIG_INVALID_ENDPOINT,
        OTLP_CONFIG_INVALID_HEADER,
        OTLP_TRANSPORT_CONSTRUCTION_FAILED,
        OTLP_UNSUPPORTED_BACKEND,
        OTLP_UNSUPPORTED_PROTOCOL,
        OTLP_TOKIO_RUNTIME_REQUIRED,
        OTLP_BLOCKING_BACKEND_IN_ASYNC_CONTEXT,
        OTLP_ASYNC_LIFECYCLE_REQUIRED,
        OTLP_RUNTIME_TERMINATED,
        OTLP_LIFECYCLE_TIMEOUT,
        OTLP_QUEUE_FULL,
        OTLP_WORKER_TERMINATED,
        OTLP_SHUTDOWN_CANCELLED_RETRY,
        OTLP_RETRY_DEADLINE_EXHAUSTED,
        OTLP_HTTP_STATUS_TERMINAL,
        OTLP_RETRY_ATTEMPTS_EXHAUSTED,
        OTLP_EXPORT_TERMINAL,
        OTLP_TELEMETRY_SHUTDOWN,
    ];
}

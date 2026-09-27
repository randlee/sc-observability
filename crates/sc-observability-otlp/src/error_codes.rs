//! Types-owned OTLP error-code re-exports.
//!
//! The canonical inventory lives in `sc-observability-types`; this transport
//! crate intentionally has no independent registry or string literals.

pub(crate) use sc_observability_types::error_codes::otlp::{
    OTLP_CONFIG_BOUND_ORDER, OTLP_CONFIG_FIELD_NOT_APPLICABLE,
    OTLP_CONFIG_INSECURE_TRANSPORT_REJECTED, OTLP_CONFIG_INVALID_ENDPOINT,
    OTLP_CONFIG_INVALID_HEADER, OTLP_CONFIG_JITTER_PERCENT, OTLP_CONFIG_QUEUE_BYTE_CAPACITY,
    OTLP_CONFIG_QUEUE_CAPACITY, OTLP_CONFIG_ZERO_DURATION, OTLP_EXPORT_TERMINAL,
    OTLP_TRANSPORT_CONSTRUCTION_FAILED,
};

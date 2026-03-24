//! Stable `ErrorCode` registry for `sc-observe`.

use sc_observability_types::ErrorCode;

/// Error code for routing after shutdown.
pub const OBSERVATION_SHUTDOWN: ErrorCode =
    ErrorCode::new_static("SC_OBSERVE_OBSERVATION_SHUTDOWN");
/// Error code for future queue-capacity exhaustion.
pub const OBSERVATION_QUEUE_FULL: ErrorCode =
    ErrorCode::new_static("SC_OBSERVE_OBSERVATION_QUEUE_FULL");
/// Error code for missing or failed routing paths.
pub const OBSERVATION_ROUTING_FAILURE: ErrorCode =
    ErrorCode::new_static("SC_OBSERVE_OBSERVATION_ROUTING_FAILURE");
/// Error code for builder/runtime initialization failures.
pub const OBSERVABILITY_INIT_FAILED: ErrorCode = ErrorCode::new_static("SC_OBSERVE_INIT_FAILED");
/// Error code for future flush failures.
pub const OBSERVABILITY_FLUSH_FAILED: ErrorCode = ErrorCode::new_static("SC_OBSERVE_FLUSH_FAILED");

/// Enumerable registry of all public `sc-observe` error codes.
pub const ALL: &[ErrorCode] = &[
    OBSERVATION_SHUTDOWN,
    OBSERVATION_QUEUE_FULL,
    OBSERVATION_ROUTING_FAILURE,
    OBSERVABILITY_INIT_FAILED,
    OBSERVABILITY_FLUSH_FAILED,
];

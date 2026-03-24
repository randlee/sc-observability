use sc_observability_types::ErrorCode;

pub const OBSERVATION_SHUTDOWN: ErrorCode =
    ErrorCode::new_static("SC_OBSERVE_OBSERVATION_SHUTDOWN");
pub const OBSERVATION_QUEUE_FULL: ErrorCode =
    ErrorCode::new_static("SC_OBSERVE_OBSERVATION_QUEUE_FULL");
pub const OBSERVATION_ROUTING_FAILURE: ErrorCode =
    ErrorCode::new_static("SC_OBSERVE_OBSERVATION_ROUTING_FAILURE");
pub const OBSERVABILITY_INIT_FAILED: ErrorCode = ErrorCode::new_static("SC_OBSERVE_INIT_FAILED");
pub const OBSERVABILITY_FLUSH_FAILED: ErrorCode = ErrorCode::new_static("SC_OBSERVE_FLUSH_FAILED");

pub const ALL: &[ErrorCode] = &[
    OBSERVATION_SHUTDOWN,
    OBSERVATION_QUEUE_FULL,
    OBSERVATION_ROUTING_FAILURE,
    OBSERVABILITY_INIT_FAILED,
    OBSERVABILITY_FLUSH_FAILED,
];

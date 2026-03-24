use sc_observability_types::ErrorCode;

pub const LOGGER_INVALID_EVENT: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_INVALID_EVENT");
pub const LOGGER_SHUTDOWN: ErrorCode = ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_SHUTDOWN");
pub const LOGGER_SINK_WRITE_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_SINK_WRITE_FAILED");
pub const LOGGER_INIT_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_INIT_FAILED");
pub const LOGGER_FLUSH_FAILED: ErrorCode =
    ErrorCode::new_static("SC_OBSERVABILITY_LOGGER_FLUSH_FAILED");

pub const ALL: &[ErrorCode] = &[
    LOGGER_INVALID_EVENT,
    LOGGER_SHUTDOWN,
    LOGGER_SINK_WRITE_FAILED,
    LOGGER_INIT_FAILED,
    LOGGER_FLUSH_FAILED,
];

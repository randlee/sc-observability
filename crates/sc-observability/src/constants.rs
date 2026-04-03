//! Crate-local constants for `sc-observability`.

pub const DEFAULT_LOG_QUEUE_CAPACITY: usize = 1024;
pub const DEFAULT_ROTATION_MAX_BYTES: u64 = 64 * 1024 * 1024;
pub const DEFAULT_ROTATION_MAX_FILES: u32 = 10;
pub const DEFAULT_RETENTION_MAX_AGE_DAYS: u32 = 7;
pub const DEFAULT_ENABLE_FILE_SINK: bool = true;
pub const DEFAULT_ENABLE_CONSOLE_SINK: bool = false;
pub const REDACTED_VALUE: &str = "[REDACTED]";

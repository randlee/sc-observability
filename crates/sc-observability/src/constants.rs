//! Crate-local constants for `sc-observability`.

use std::time::Duration;

/// Environment variable that overrides the default log root when explicit
/// configuration leaves `LoggerConfig.log_root` empty.
pub const SC_LOG_ROOT_ENV_VAR: &str = "SC_LOG_ROOT";
/// Directory name used by the built-in JSONL file sink beneath the configured
/// log root.
pub const DEFAULT_LOG_DIR_NAME: &str = "logs";
/// File suffix used by the built-in JSONL file sink.
pub const DEFAULT_LOG_FILE_SUFFIX: &str = ".log.jsonl";
/// Stable sink name reported by the built-in JSONL file sink.
pub const JSONL_FILE_SINK_NAME: &str = "jsonl-file";
/// Stable sink name reported by the built-in console sink.
pub const CONSOLE_SINK_NAME: &str = "console";
/// Seconds in one calendar day; used as the retention-age multiplier.
pub(crate) const SECS_PER_DAY: u64 = 86_400;
/// Default synchronous queue-capacity placeholder retained for the v1 config
/// surface.
pub const DEFAULT_LOG_QUEUE_CAPACITY: usize = 1024;
/// Internal maximum number of records one writer-thread batch drains before it
/// flushes or rechecks maintenance work.
pub(crate) const DEFAULT_LOG_BATCH_SIZE: usize = 64;
/// Internal maximum time the writer thread waits to coalesce a partial batch
/// before writing it.
pub(crate) const DEFAULT_WRITER_BATCH_TIMEOUT: Duration = Duration::from_millis(5);
/// Default maximum active-log size before rotation.
pub const DEFAULT_ROTATION_MAX_BYTES: u64 = 64 * 1024 * 1024;
/// Default number of rotated files retained beside the active log.
pub const DEFAULT_ROTATION_MAX_FILES: u32 = 10;
/// `usize` alias for logger-managed retained-log rotation counts.
pub const DEFAULT_ROTATION_MAX_FILES_USIZE: usize = DEFAULT_ROTATION_MAX_FILES as usize;
/// Default retention window for rotated logs in calendar days.
pub const DEFAULT_RETENTION_MAX_AGE_DAYS: u32 = 7;
/// Default retention window for retained logs.
pub const DEFAULT_RETENTION_MAX_AGE: Duration =
    Duration::from_secs(DEFAULT_RETENTION_MAX_AGE_DAYS as u64 * SECS_PER_DAY);
/// Default cadence for retained-log maintenance passes.
pub const DEFAULT_MAINTENANCE_CADENCE: Duration = Duration::from_secs(60);
/// Default writer-thread shutdown timeout used during logger shutdown.
pub const DEFAULT_WRITER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
/// Default work limit per maintenance pass.
pub const DEFAULT_MAINTENANCE_MAX_WORK_PER_PASS: Option<usize> = None;
/// Default enablement for the built-in JSONL file sink.
pub const DEFAULT_ENABLE_FILE_SINK: bool = true;
/// Default enablement for the built-in console sink.
pub const DEFAULT_ENABLE_CONSOLE_SINK: bool = false;
pub(crate) const REDACTED_VALUE: &str = "[REDACTED]";

//! Deprecated released 1.x facade retained for the compatible transition.
#![allow(deprecated)]

mod compat;

#[deprecated(note = "use crate::v2::LogSink; see docs/migration/phase-f.md")]
pub use crate::v2::LogSink;
pub use compat::{LogError, TryLogError};

#[deprecated(note = "use crate::v2::LoggerBuilder; see docs/migration/phase-f.md")]
#[expect(
    missing_debug_implementations,
    reason = "the wrapper deliberately hides the canonical builder's sink trait objects"
)]
pub struct LoggerBuilder {
    pub(crate) inner: crate::builder::CanonicalLoggerBuilder,
}

#[deprecated(note = "use crate::v2::Logger; see docs/migration/phase-f.md")]
#[expect(
    missing_debug_implementations,
    reason = "the wrapper deliberately hides runtime handles and trait-object sinks"
)]
pub struct Logger<State = crate::Running> {
    pub(crate) inner: crate::CanonicalLogger<State>,
    shutdown: std::marker::PhantomData<State>,
}

/// Rotation limits for the built-in JSONL file sink.
#[deprecated(note = "use crate::RetainedLogPolicy; see docs/migration/phase-f.md")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotationPolicy {
    /// Maximum size of the active JSONL file before rotation.
    pub max_bytes: crate::ByteCount,
    /// Maximum number of rotated files to retain.
    pub max_files: sc_observability_types::FileCount,
}

impl Default for RotationPolicy {
    fn default() -> Self {
        Self {
            max_bytes: crate::ByteCount::from_bytes(crate::constants::DEFAULT_ROTATION_MAX_BYTES),
            max_files: sc_observability_types::FileCount::from_usize(
                crate::constants::DEFAULT_ROTATION_MAX_FILES_USIZE,
            ),
        }
    }
}

/// Retention limits for rotated JSONL files owned by the built-in file sink.
#[deprecated(note = "use crate::RetainedLogPolicy; see docs/migration/phase-f.md")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Maximum age in days for rotated JSONL files.
    #[deprecated(note = "use RetainedLogPolicy::retention_max_age; see docs/migration/phase-f.md")]
    pub max_age_days: u32,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_age_days: crate::constants::DEFAULT_RETENTION_MAX_AGE_DAYS,
        }
    }
}

impl crate::JsonlFileSink {
    /// Creates a JSONL sink using the released rotation and retention policies.
    ///
    /// New code should use [`crate::JsonlFileSink::open`] with
    /// [`crate::RetainedLogPolicy`].
    #[deprecated(
        note = "use crate::JsonlFileSink::open with RetainedLogPolicy; see docs/migration/phase-f.md"
    )]
    pub fn new(
        path: std::path::PathBuf,
        rotation: RotationPolicy,
        retention: RetentionPolicy,
    ) -> Self {
        let policy = crate::RetainedLogPolicy {
            rotation_max_bytes: rotation.max_bytes,
            rotation_max_files: rotation.max_files,
            retention_max_age: crate::RetentionMaxAge::from_days(u64::from(retention.max_age_days)),
            ..crate::RetainedLogPolicy::default()
        };
        let mut sink = Self::for_logger(path);
        sink.legacy_policy = Some(policy);
        sink
    }
}

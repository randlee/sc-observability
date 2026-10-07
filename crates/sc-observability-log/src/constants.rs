//! Crate-owned operational constants (ADR-005 / SRC-003–006).
//!
//! Error identifiers deliberately remain in [`crate::error_codes`].

use std::time::Duration;

/// Version of the native bridge-health shape.
pub const BRIDGE_HEALTH_SCHEMA_VERSION: u32 = 1;

/// Timeout used by implicit `LogGuard` and `LogAttachment` drop paths.
pub const DEFAULT_DROP_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

/// Field keys beginning with this prefix are owned by the bridge.
pub const RESERVED_FIELD_PREFIX: &str = "sc_observability_log.";

/// Field holding user values displaced by crate-owned keys.
pub(crate) const SHADOWED_FIELDS_KEY: &str = "sc_observability_log.shadowed_fields";

/// Field holding serialization errors, keyed by field name.
pub(crate) const SERIALIZE_ERRORS_KEY: &str = "sc_observability_log.serialize_errors";

/// Deadline for an isolated unit-test child process.
#[cfg(test)]
pub(crate) const ISOLATED_TEST_CHILD_DEADLINE: Duration = Duration::from_secs(30);

/// Poll interval while reaping an isolated unit-test child process.
#[cfg(test)]
pub(crate) const ISOLATED_TEST_CHILD_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Initial pause while waiting for sole ownership during shutdown.
pub(crate) const UNWRAP_BACKOFF_START: Duration = Duration::from_millis(1);

/// Maximum pause while waiting for sole ownership during shutdown.
pub(crate) const UNWRAP_BACKOFF_MAX: Duration = Duration::from_millis(50);

/// Encoded native bridge lifecycle states used by the lock-free health snapshot.
pub(crate) const LIFECYCLE_RUNNING: u8 = 0;
pub(crate) const LIFECYCLE_SHUTTING_DOWN: u8 = 1;
pub(crate) const LIFECYCLE_STOPPED: u8 = 2;
pub(crate) const LIFECYCLE_FAILED: u8 = 3;

/// Encoded bounded-helper states used to maintain the detached-helper count.
pub(crate) const HELPER_RUNNING: u8 = 0;
pub(crate) const HELPER_DETACHED: u8 = 1;
pub(crate) const HELPER_DONE: u8 = 2;

/// Test-only fault selectors for the bounded native helper seam.
#[cfg(test)]
pub(crate) const BOUNDED_HELPER_SPAWN_FAILURE: u8 = 1;
#[cfg(test)]
pub(crate) const BOUNDED_HELPER_PANIC: u8 = 2;
#[cfg(test)]
pub(crate) const BOUNDED_HELPER_BLOCK: u8 = 3;

/// Test-only fault selectors for the non-owning attachment helper seam.
#[cfg(test)]
pub(crate) const ATTACHMENT_HELPER_SPAWN_FAILURE: u8 = 1;
#[cfg(test)]
pub(crate) const ATTACHMENT_HELPER_PANIC: u8 = 2;

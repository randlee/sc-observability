//! Binding-runtime policy limits shared by validation and operation observers.

use std::time::Duration;

/// Maximum duration an observer may wait for a native operation.
pub(crate) const MAX_OBSERVATION_TIMEOUT: Duration = Duration::from_secs(60);

/// Maximum observer timeout expressed in the DTO's millisecond unit.
pub(crate) const MAX_OBSERVATION_TIMEOUT_MS: u32 = 60_000;

/// Maximum callback registrations retained by one native dispatcher.
pub const CALLBACK_REGISTRATION_CAPACITY: usize = 128;

/// Maximum concurrent observers retained by one native operation.
pub const OPERATION_OBSERVER_CAPACITY: usize = 64;

/// Maximum targets accepted by the Tauri query policy.
pub const TAURI_MAX_QUERY_TARGETS: usize = 64;

/// Default Tauri query-observation deadline in milliseconds.
pub const TAURI_DEFAULT_QUERY_TIMEOUT_MS: u32 = 2_000;

/// Replacement text for Tauri-host redacted values.
pub const TAURI_REDACTED_VALUE: &str = "[REDACTED]";

#[cfg(test)]
mod tests {
    use super::{
        CALLBACK_REGISTRATION_CAPACITY, OPERATION_OBSERVER_CAPACITY,
        TAURI_DEFAULT_QUERY_TIMEOUT_MS, TAURI_MAX_QUERY_TARGETS, TAURI_REDACTED_VALUE,
    };

    #[test]
    fn binding_policy_constants_preserve_the_established_limits() {
        assert_eq!(CALLBACK_REGISTRATION_CAPACITY, 128);
        assert_eq!(OPERATION_OBSERVER_CAPACITY, 64);
        assert_eq!(TAURI_MAX_QUERY_TARGETS, 64);
        assert_eq!(TAURI_DEFAULT_QUERY_TIMEOUT_MS, 2_000);
        assert_eq!(TAURI_REDACTED_VALUE, "[REDACTED]");
    }
}

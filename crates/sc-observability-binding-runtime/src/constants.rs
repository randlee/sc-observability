//! Binding-runtime policy limits shared by validation and operation observers.

use std::time::Duration;

/// Maximum duration an observer may wait for a native operation.
pub(crate) const MAX_OBSERVATION_TIMEOUT: Duration = Duration::from_secs(60);

/// Maximum observer timeout expressed in the DTO's millisecond unit.
pub(crate) const MAX_OBSERVATION_TIMEOUT_MS: u32 = 60_000;

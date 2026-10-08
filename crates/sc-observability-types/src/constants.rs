//! Shared cross-crate constants owned by `sc-observability-types`.

use std::sync::LazyLock;

use crate::SchemaVersion;

/// Current version string for the observation envelope contract.
pub const OBSERVATION_ENVELOPE_VERSION: &str = "v1";
/// Validated schema version used by newly-created observation envelopes.
///
/// This is a static because `SchemaVersion` owns a `String` and validates its
/// input, so it cannot be constructed in a `const` initializer.
pub static OBSERVATION_SCHEMA_VERSION: LazyLock<SchemaVersion> = LazyLock::new(|| {
    SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION)
        .expect("shared schema version constant is valid")
});
/// Required character length for W3C trace identifiers.
pub const TRACE_ID_LEN: usize = 32;
/// Required character length for W3C span identifiers.
pub const SPAN_ID_LEN: usize = 16;
/// Separator used when deriving environment prefixes.
pub const DEFAULT_ENV_PREFIX_SEPARATOR: char = '_';

/// Maximum W3C tracestate byte length.
pub const TRACE_STATE_MAX_BYTES: usize = 512;
/// Maximum W3C tracestate list members.
pub const TRACE_STATE_MAX_MEMBERS: usize = 32;
/// Maximum tracestate key or value byte length.
pub const TRACE_STATE_MEMBER_MAX_BYTES: usize = 256;
/// Maximum multi-tenant tracestate tenant length.
pub const TRACE_STATE_TENANT_MAX_BYTES: usize = 241;
/// Maximum multi-tenant tracestate system length.
pub const TRACE_STATE_SYSTEM_MAX_BYTES: usize = 14;

/// Maximum OTLP log severity number.
pub const OTLP_SEVERITY_MAX: u8 = 24;
/// Minimum exponential histogram scale.
pub const OTLP_EXPONENTIAL_SCALE_MIN: i32 = -10;
/// Maximum exponential histogram scale.
pub const OTLP_EXPONENTIAL_SCALE_MAX: i32 = 20;

#[cfg(test)]
mod tests {
    use super::{OBSERVATION_ENVELOPE_VERSION, OBSERVATION_SCHEMA_VERSION};
    use crate::SchemaVersion;

    #[test]
    fn observation_schema_version_matches_the_envelope_version() {
        assert_eq!(
            *OBSERVATION_SCHEMA_VERSION,
            SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION).expect("valid schema version")
        );
    }
}

//! Shared cross-crate constants owned by `sc-observability-types`.

/// Current version string for the observation envelope contract.
pub const OBSERVATION_ENVELOPE_VERSION: &str = "v1";
/// Required character length for W3C trace identifiers.
pub const TRACE_ID_LEN: usize = 32;
/// Required character length for W3C span identifiers.
pub const SPAN_ID_LEN: usize = 16;
/// Separator used when deriving environment prefixes.
pub const DEFAULT_ENV_PREFIX_SEPARATOR: char = '_';

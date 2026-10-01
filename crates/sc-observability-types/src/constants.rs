//! Shared cross-crate constants owned by `sc-observability-types`.

/// Current version string for the observation envelope contract.
pub const OBSERVATION_ENVELOPE_VERSION: &str = "v1";
/// Required character length for W3C trace identifiers.
pub const TRACE_ID_LEN: usize = 32;
/// Required character length for W3C span identifiers.
pub const SPAN_ID_LEN: usize = 16;
/// Separator used when deriving environment prefixes.
pub const DEFAULT_ENV_PREFIX_SEPARATOR: char = '_';

/// W3C sampled bit in the trace flags byte.
pub(crate) const TRACE_FLAG_SAMPLED: u8 = 0x01;

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

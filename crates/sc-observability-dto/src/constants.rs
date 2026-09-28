//! Shared wire-policy limits used by checked conversion and schema generation.

/// Maximum UTF-8 bytes in a diagnostic field, bounding retained error metadata.
pub const MAX_DIAGNOSTIC_FIELD_BYTES: usize = 4_096;
/// Maximum serialized wire payload bytes, bounding decoding and transfer work.
pub const MAX_WIRE_PAYLOAD_BYTES: usize = 65_536;
/// Maximum container nesting, bounding recursive validation work.
pub const MAX_CONTAINER_DEPTH: usize = 32;
/// Maximum retained remediation steps, bounding diagnostic rendering work.
pub const MAX_REMEDIATION_STEPS: usize = 32;
/// Maximum observer timeout in milliseconds, bounding a caller's wait.
pub const MAX_TIMEOUT_MS: u32 = 60_000;
/// Maximum query result count, bounding snapshot allocation and transfer.
pub const MAX_QUERY_LIMIT: usize = 1_000;
/// Default query result count when omitted, providing a bounded initial page.
pub const DEFAULT_QUERY_LIMIT: usize = 100;
/// Maximum reported client operations, matching bounded client admission.
pub const MAX_CLIENT_IN_FLIGHT: u32 = 256;

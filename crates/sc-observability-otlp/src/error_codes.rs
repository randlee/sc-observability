//! Types-owned OTLP error-code re-exports.
//!
//! The canonical inventory lives in `sc-observability-types`; this transport
//! crate intentionally has no independent registry or string literals.

pub use sc_observability_types::error_codes::otlp::*;

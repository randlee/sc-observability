//! Stable public wire re-exports grouped by DTO concern.
//!
//! The module paths below are implementation organization only. `wire` itself
//! is private; re-exports here surface at the reachable `sc_observability_dto::*`
//! path and therefore preserve the original API and canonical Serde/schema
//! surface.

mod events;
mod health;
mod operations;
mod primitives;

pub use events::*;
pub use health::*;
pub use operations::*;
pub use primitives::*;

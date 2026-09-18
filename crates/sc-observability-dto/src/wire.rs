//! Stable public wire re-exports grouped by DTO concern.
//!
//! The module paths below are implementation organization only. Re-exports
//! preserve the original `sc_observability_dto::wire::*` API and therefore the
//! canonical Serde/schema surface.

mod events;
mod health;
mod operations;
mod primitives;

pub use events::*;
pub use health::*;
pub use operations::*;
pub use primitives::*;

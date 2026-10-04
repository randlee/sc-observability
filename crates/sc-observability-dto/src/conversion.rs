//! Checked semantic conversion; no runtime or transport dependency.
//!
//! The implementation is partitioned by contract boundary while this facade
//! preserves the historical `crate::conversion::*` public surface.
use crate::{Failure, LogHealthDto};
use sc_observability_types as core;

#[path = "conversion_impl/mod.rs"]
mod implementation;

pub use implementation::*;

/// Projects an independent core logger without inventing bridge state.
pub fn from_core_health(
    value: core::LoggingHealthReport,
    level: core::LevelState,
) -> Result<LogHealthDto, Failure> {
    Ok(implementation::from_canonical_core_health(value, level))
}

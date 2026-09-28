//! Checked semantic conversion; no runtime or transport dependency.
//!
//! The implementation is partitioned by contract boundary while this facade
//! preserves the historical `crate::conversion::*` public surface.
#[path = "conversion_impl/mod.rs"]
mod implementation;

pub use implementation::*;

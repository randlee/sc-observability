//! Minimal workspace skeleton for the future observation routing crate.
//!
//! This crate intentionally contains no runtime implementation yet. It exists
//! to establish the workspace boundary and dependency direction:
//! `sc-observability-types <- sc-observability <- sc-observe`.

/// Marker constant proving the crate is linked and available to the workspace.
pub const CRATE_NAME: &str = "sc-observe";

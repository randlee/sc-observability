//! Opt-in typed logger sink interoperability.
//!
//! The retained [`crate::LogSink`] trait remains the released registration
//! boundary. These released adapters let sink implementations use neutral
//! typed failures without changing legacy consumers or introducing root trait
//! ambiguity. Their definitions live in the removable compatibility module;
//! this module keeps them at their released public paths. New code should
//! implement [`crate::v2::LogSink`] instead.

#[doc(inline)]
pub use crate::released_sink_adapters::{TypedLogSink, legacy_sink, typed_sink};

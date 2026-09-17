//! Neutral, versioned wire contracts and checked conversions for language bindings.
//! The crate has no logging runtime, transport, or ownership authority.
mod conversion;
pub mod error_codes;
mod wire;
pub use conversion::*;
pub use wire::*;

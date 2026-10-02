//! Official SDK adapter module root staged by the OTLP contract.

pub(crate) mod implementation;
mod retry;
#[allow(
    unused_imports,
    reason = "D.18 consumes this constructor re-export during facade composition"
)]
pub(crate) use implementation::build_exporter_set;
#[cfg(feature = "sdk-test-support")]
pub mod fixture;
#[cfg(feature = "sdk-test-support")]
#[doc(inline)]
pub use fixture::SdkFixture;
#[cfg(test)]
mod tests;

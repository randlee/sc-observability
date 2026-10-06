//! Official SDK adapter module root staged by the OTLP contract.

pub(crate) mod implementation;
mod retry;
pub(crate) use implementation::build_exporter_set;
#[cfg(all(test, feature = "otlp-sdk"))]
pub(crate) mod fixture;
#[cfg(all(test, feature = "otlp-sdk"))]
mod fixture_tests;
#[cfg(test)]
mod tests;

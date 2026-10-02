//! Synchronous HTTP/JSON adapter module root staged by the OTLP contract.

mod implementation;
pub(crate) mod payload;
mod response;
pub(crate) use implementation::build_exporter_set;
#[cfg(test)]
mod tests;

#[cfg(feature = "durable-store")]
#[cfg(any(test, feature = "durable-store"))]
pub(crate) mod submission;

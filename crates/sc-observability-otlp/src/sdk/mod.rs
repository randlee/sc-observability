//! Official SDK adapter module root staged by the OTLP contract.

pub(crate) mod implementation;
pub(crate) use implementation::build_exporter_set;
#[cfg(test)]
mod tests;

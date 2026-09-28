//! Official SDK adapter module root staged by the OTLP contract.

pub(crate) mod implementation;
#[allow(
    unused_imports,
    reason = "D.18 consumes this constructor re-export during facade composition"
)]
pub(crate) use implementation::build_exporter_set;
#[cfg(test)]
mod tests;

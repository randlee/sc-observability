//! Python transport for the shared telemetry submission contract.
mod client;
mod config;
mod dto;
mod error_projection;
#[cfg(feature = "test-hooks")]
mod test_gate;
#[cfg(test)]
mod tests;
use pyo3::prelude::*;
pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<client::NativeTelemetry>()?;
    module.add_function(wrap_pyfunction!(client::open, module)?)?;
    module.add_function(wrap_pyfunction!(client::build_envelope, module)?)?;
    module.add(
        "TELEMETRY_CONFIG_INVALID",
        sc_observability_types::error_codes::SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID.as_str(),
    )?;
    #[cfg(feature = "test-hooks")]
    module.add_function(wrap_pyfunction!(client::_open_test_double, module)?)?;
    Ok(())
}

//! Python calls into the shared synchronous OTLP client (ADR-023).
mod send;
use pyo3::prelude::*;
pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(send::send_log, module)?)?;
    module.add_function(wrap_pyfunction!(send::send_span, module)?)?;
    module.add_function(wrap_pyfunction!(send::send_metric, module)?)?;
    Ok(())
}

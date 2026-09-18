//! B.5 mixed request proof, linked into the same embedded Rust/Python process.
use pyo3::prelude::*;
use pyo3::types::PyDict;
use sc_observability_binding_runtime::{HostLoggingBackend, ProducerOrigin};
use std::ffi::CString;

pub(crate) fn verify(py: Python<'_>, backend: &impl HostLoggingBackend) -> PyResult<()> {
    let correlation = format!(
        "b5-mixed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))?
            .as_nanos()
    );
    let event = sc_observability_dto::decode_event(serde_json::json!({
        "schema_version": 1, "level": "info", "target": "rust.mixed",
        "action": "rust.request", "request_id": "b5-request",
        "correlation_id": correlation, "fields": {"side": {"kind": "string", "value": "rust"}}
    }))
    .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(format!("{error:?}")))?;
    backend
        .try_log(event, ProducerOrigin::RustHost)
        .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(format!("{error:?}")))?;
    let source = CString::new(include_str!("../b5_context.py"))
        .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
    let globals = PyDict::new(py);
    globals.set_item("B5_CORRELATION", correlation)?;
    py.run(&source, Some(&globals), None)
}

pub(crate) fn after_stop(py: Python<'_>) -> PyResult<()> {
    py.run(
        pyo3::ffi::c_str!(
            "
from sc_observability import Err, get_host_logger
from sc_observability.logging import create_handler, HandlerDropCause
import logging
attached = get_host_logger().value
handler = create_handler(attached).value
handler.emit(logging.LogRecord('python.stopped', 20, 'embedded', 1, 'closed host', (), None))
health = handler.health().value
assert isinstance(health.last_result, Err) and health.last_result.error.kind == 'closed'
assert health.dropped_by_cause[HandlerDropCause.CLOSED] == 1
handler.close()
print('B5_ATTACHED_STOP_OK', flush=True)
"
        ),
        None,
        None,
    )
}

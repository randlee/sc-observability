//! Real Rust-host embedding example for the B.4 binding rlib.
//!
//! This executable links the binding rlib and installs the PyO3 module before
//! interpreter initialization. It never loads a wheel or exchanges a Rust
//! trait object through a dynamic-library boundary.

mod b5_context;

extern crate _native as binding;

use binding::{_native as native_module, install_host_logger};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use sc_observability_binding_runtime::{HostLoggingBackend, create_core_backend};
use sc_observability_types::ServiceName;
use std::path::PathBuf;
use std::sync::Arc;

fn main() -> PyResult<()> {
    pyo3::append_to_inittab!(native_module);
    Python::initialize();
    Python::attach(|py| {
        let service = ServiceName::new("rust-python-logging")
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(error.to_string()))?;
        let mut config = sc_observability::LoggerConfig::default_for(
            service,
            std::env::temp_dir().join("sc-observability-rust-python-example"),
        );
        config.enable_console_sink = false;
        let (owner, backend) = create_core_backend(config)
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(format!("{error:?}")))?;
        let module = PyModule::import(py, "_native")?;
        let shared: Arc<dyn HostLoggingBackend> = Arc::new(backend.clone());
        install_host_logger(&module, shared)
            .map_err(|error| pyo3::exceptions::PyRuntimeError::new_err(format!("{error:?}")))?;

        let sys = PyModule::import(py, "sys")?;
        let modules = sys.getattr("modules")?.cast_into::<PyDict>()?;
        modules.set_item("sc_observability._native", &module)?;
        let source_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../bindings/python/sc-observability-py/python");
        sys.getattr("path")?
            .call_method1("insert", (0, source_root.to_string_lossy().as_ref()))?;

        let api = PyModule::import(py, "sc_observability")?;
        let attached = api.getattr("get_host_logger")?.call0()?;
        let kind: String = attached.getattr("kind")?.extract()?;
        if kind != "ok" {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "host attachment did not return tagged Ok",
            ));
        }
        let attached_logger = attached.getattr("value")?;
        let event = api
            .getattr("LogEvent")?
            .call1(("info", "rust.python", "embedded"))?;
        let logged = attached_logger.call_method1("log", (event,))?;
        let logged_kind: String = logged.getattr("kind")?.extract()?;
        if logged_kind != "ok" {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "attached Python log was not admitted",
            ));
        }
        let health = backend.health();
        if health.is_err() {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "Rust host cannot observe attached backend health",
            ));
        }
        b5_context::verify(py, &backend)?;
        drop(owner);
        b5_context::after_stop(py)?;
        Ok(())
    })
}

//! Real Rust-host embedding example for the B.4 binding rlib.
//!
//! This executable links the binding rlib and installs the PyO3 module before
//! interpreter initialization. It never loads a wheel or exchanges a Rust
//! trait object through a dynamic-library boundary.

mod b5_context;

extern crate _native as binding;

mod async_conformance;

use binding::{_native as native_module, install_host_logger};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use sc_observability_binding_runtime::{HostLoggingBackend, ProducerOrigin, create_core_backend};
use sc_observability_dto::{LevelDto, LogEventDto, LogOrderDto, LogQueryDto, ValueDto};
use sc_observability_types::ServiceName;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const CORRELATION_ID: &str = "rust-python-shared-writer";

fn runtime_error(message: impl Into<String>) -> PyErr {
    pyo3::exceptions::PyRuntimeError::new_err(message.into())
}

fn rust_event() -> LogEventDto {
    LogEventDto {
        schema_version: 1,
        level: LevelDto::Info,
        target: "rust.python".into(),
        action: "rust-embedded".into(),
        message: Some("Bearer rust-example-secret".into()),
        trace: None,
        request_id: None,
        correlation_id: Some(CORRELATION_ID.into()),
        outcome: None,
        fields: BTreeMap::new(),
    }
}

fn correlated_query() -> LogQueryDto {
    LogQueryDto {
        schema_version: 1,
        service: None,
        levels: Vec::new(),
        target: None,
        action: None,
        request_id: None,
        correlation_id: Some(CORRELATION_ID.into()),
        since: None,
        until: None,
        field_matches: Vec::new(),
        limit: 100,
        order: LogOrderDto::OldestFirst,
    }
}

fn has_record(
    snapshot: &sc_observability_dto::LogSnapshotDto,
    action: &str,
    language: &str,
) -> bool {
    snapshot.events.iter().any(|event| {
        event.action == action
            && event.message.as_deref() == Some("Bearer [REDACTED]")
            && matches!(
                event.fields.get("sc_observability.binding.language"),
                Some(ValueDto::String { value }) if value == language
            )
    })
}

fn main() -> PyResult<()> {
    if let Some(mode) =
        std::env::args().find_map(|arg| arg.strip_prefix("--b6-finalize=").map(str::to_owned))
    {
        match async_conformance::finalize(&mode) {
            Ok(()) => std::process::exit(0),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
    }
    pyo3::append_to_inittab!(native_module);
    Python::initialize();
    Python::attach(|py| {
        let service = ServiceName::new("rust-python-logging")
            .map_err(|error| runtime_error(error.to_string()))?;
        let mut config = sc_observability::LoggerConfig::default_for(
            service,
            std::env::temp_dir().join("sc-observability-rust-python-example"),
        );
        config.enable_console_sink = false;
        let (owner, backend) =
            create_core_backend(config).map_err(|error| runtime_error(format!("{error:?}")))?;
        let module = PyModule::import(py, "_native")?;
        let shared: Arc<dyn HostLoggingBackend> = Arc::new(backend.clone());
        install_host_logger(&module, shared)
            .map_err(|error| runtime_error(format!("{error:?}")))?;

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
            return Err(runtime_error("host attachment did not return tagged Ok"));
        }
        let attached_logger = attached.getattr("value")?;
        let event_kwargs = PyDict::new(py);
        event_kwargs.set_item("message", "Bearer python-example-secret")?;
        event_kwargs.set_item("correlation_id", CORRELATION_ID)?;
        let event = api.getattr("LogEvent")?.call(
            ("info", "rust.python", "python-embedded"),
            Some(&event_kwargs),
        )?;
        let logged = attached_logger.call_method1("log", (event,))?;
        let logged_kind: String = logged.getattr("kind")?.extract()?;
        if logged_kind != "ok" {
            return Err(runtime_error("attached Python log was not admitted"));
        }

        backend
            .try_log(rust_event(), ProducerOrigin::RustHost)
            .map_err(|error| runtime_error(format!("Rust record was not admitted: {error:?}")))?;
        backend
            .start_flush(Duration::from_secs(2))
            .and_then(|operation| operation.wait(Duration::from_secs(2)))
            .map_err(|error| runtime_error(format!("shared writer did not flush: {error:?}")))?;

        let query_kwargs = PyDict::new(py);
        query_kwargs.set_item("correlation_id", CORRELATION_ID)?;
        let query = api.getattr("LogQuery")?.call((), Some(&query_kwargs))?;
        let python_snapshot = attached_logger.call_method1("query", (query,))?;
        if python_snapshot.getattr("kind")?.extract::<String>()? != "ok" {
            return Err(runtime_error(
                "attached Python query did not return tagged Ok",
            ));
        }
        let native_snapshot = backend
            .start_query(correlated_query())
            .and_then(|operation| operation.wait(Duration::from_secs(2)))
            .map_err(|error| runtime_error(format!("Rust query failed: {error:?}")))?;
        if !has_record(&native_snapshot, "rust-embedded", "rust")
            || !has_record(&native_snapshot, "python-embedded", "python")
        {
            return Err(runtime_error(
                "Rust and Python records were not correlated through one redacted writer",
            ));
        }
        if backend.health().is_err() {
            return Err(runtime_error(
                "Rust host cannot observe attached backend health",
            ));
        }
        b5_context::verify(py, &backend)?;
        owner
            .shutdown(Duration::from_secs(2))
            .map_err(|error| runtime_error(format!("host shutdown failed: {error:?}")))?;
        if backend
            .try_log(rust_event(), ProducerOrigin::RustHost)
            .is_ok()
        {
            return Err(runtime_error("host admitted a record after shutdown"));
        }
        if attached_logger
            .call_method0("health")?
            .getattr("kind")?
            .extract::<String>()?
            != "ok"
        {
            return Err(runtime_error(
                "attached health was not retained after host shutdown",
            ));
        }
        b5_context::after_stop(py)?;
        async_conformance::run(py)?;
        Ok(())
    })
}

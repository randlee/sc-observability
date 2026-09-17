//! PyO3 transport over the shared binding-runtime backends.
//!
//! Python values are encoded to canonical DTO JSON in the Python facade. This
//! crate deliberately owns no native conversion map: DTO validation and all
//! core/bridge conversion remain in the shared Rust crates.
#![allow(
    clippy::result_large_err,
    reason = "the public binding contract preserves the complete Failure union"
)]

use pyo3::prelude::*;
use sc_observability_binding_runtime::{
    CoreLoggerBackend, CoreLoggerOwner, HostLoggingBackend, Operation, OperationState,
    ProducerOrigin, create_core_backend,
};
use sc_observability_dto::{
    CompletionDto, Failure, LevelChangeDto, LogEventDto, LogHealthDto, LogQueryDto, ResultDto,
};
use sc_observability_types::{LevelChangeSource, LevelFilter, ServiceName};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PythonLoggerConfig {
    service: String,
    log_root: String,
    #[serde(default = "default_level")]
    level: String,
    #[serde(default = "default_file_sink")]
    enable_file_sink: bool,
    #[serde(default)]
    enable_console_sink: bool,
}

fn default_level() -> String {
    "info".into()
}

const fn default_file_sink() -> bool {
    true
}

fn internal_failure(message: impl Into<String>) -> Failure {
    Failure::Internal {
        diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
            sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL,
            message,
        )),
    }
}

fn closed_failure(message: impl Into<String>) -> Failure {
    Failure::Closed {
        diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(
            sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_CLOSED,
            message,
        )),
    }
}

fn unavailable_failure(code: &str, message: impl Into<String>) -> Failure {
    Failure::Unavailable {
        diagnostic: Box::new(sc_observability_dto::boundary_diagnostic(code, message)),
    }
}

fn result_json<T: Serialize>(value: Result<T, Failure>) -> String {
    let envelope = match value {
        Ok(value) => ResultDto::Ok { value },
        Err(error) => ResultDto::Error { error },
    };
    match serde_json::to_string(&envelope) {
        Ok(json) => json,
        Err(_) => {
            r#"{"kind":"error","error":{"kind":"internal","at":"1970-01-01T00:00:00Z","code":"SC_OBSERVABILITY_BINDING_INTERNAL","message":"failed to serialize binding result","remediation":{"kind":"recoverable","steps":["Inspect the retained status and restore the affected host or client"]}}}"#.into()
        }
    }
}

fn parse_value(value: &str, field: &str) -> Result<Value, Failure> {
    serde_json::from_str(value)
        .map_err(|error| sc_observability_dto::invalid_input(field, error.to_string()))
}

fn parse_timeout(value: &str) -> Result<Duration, Failure> {
    let milliseconds = sc_observability_dto::decode_timeout(parse_value(value, "timeout_ms")?)?;
    Ok(Duration::from_millis(u64::from(milliseconds)))
}

fn level(value: &str) -> Result<LevelFilter, Failure> {
    match value {
        "off" => Ok(LevelFilter::Off),
        "error" => Ok(LevelFilter::Error),
        "warn" => Ok(LevelFilter::Warn),
        "info" => Ok(LevelFilter::Info),
        "debug" => Ok(LevelFilter::Debug),
        "trace" => Ok(LevelFilter::Trace),
        _ => Err(sc_observability_dto::invalid_input(
            "level",
            "unsupported level filter",
        )),
    }
}

fn source(value: &str) -> Result<LevelChangeSource, Failure> {
    match value {
        "application" => Ok(LevelChangeSource::Application),
        "user_request" => Ok(LevelChangeSource::UserRequest),
        "diagnostic_session" => Ok(LevelChangeSource::DiagnosticSession),
        _ => Err(sc_observability_dto::invalid_input(
            "source",
            "unsupported level change source",
        )),
    }
}

fn logger_config(value: &str) -> Result<sc_observability::LoggerConfig, Failure> {
    let config: PythonLoggerConfig = serde_json::from_str(value)
        .map_err(|error| sc_observability_dto::invalid_input("config", error.to_string()))?;
    let service = ServiceName::new(config.service)
        .map_err(|error| sc_observability_dto::invalid_input("service", error.to_string()))?;
    let root = sc_observability_dto::to_path(
        &config.log_root,
        &std::env::current_dir().map_err(|error| {
            internal_failure(format!(
                "could not resolve Python startup directory: {error}"
            ))
        })?,
    )?;
    let mut native = sc_observability::LoggerConfig::default_for(service, PathBuf::from(root));
    native.level = level(&config.level)?;
    native.enable_file_sink = config.enable_file_sink;
    native.enable_console_sink = config.enable_console_sink;
    Ok(native)
}

struct OwnedState {
    owner: CoreLoggerOwner,
    shutdown: Option<Operation<LogHealthDto>>,
}

#[pyclass]
struct NativeLogger {
    observer_identity: Py<NativeObserverIdentity>,
    backend: CoreLoggerBackend,
    owned: Mutex<OwnedState>,
}

/// Module-owned host backend. Its `Arc` survives while any attached Python
/// handle exists, but it never grants host shutdown or level ownership.
#[pyclass(frozen)]
struct HostSlot {
    observer_identity: Py<NativeObserverIdentity>,
    backend: Arc<dyn HostLoggingBackend>,
}

#[pyclass]
struct NativeAttachedLogger {
    observer_identity: Py<NativeObserverIdentity>,
    backend: Arc<dyn HostLoggingBackend>,
}

// Opaque Python-local identity; never a pointer/integer handle or capability.
// Stored only by Python wrapper/module state, never native workers.
#[pyclass(frozen, weakref)]
struct NativeObserverIdentity {}

// This transport holds only native shared completion; it never registers a
// callback or retains a Python loop/Future in native coordinator state.
#[pyclass(frozen)]
struct NativeFlushOperation {
    operation: Operation<CompletionDto>,
}

#[pymethods]
impl NativeFlushOperation {
    fn state(&self) -> Option<String> {
        match self.operation.state() {
            OperationState::Pending => None,
            OperationState::Completed { result } => Some(result_json(result)),
        }
    }
}

fn start_flush_backend(
    backend: &dyn HostLoggingBackend,
    py: Python<'_>,
    timeout: &str,
) -> (Option<Py<NativeFlushOperation>>, String) {
    let operation = parse_timeout(timeout).and_then(|timeout| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            backend.start_flush(timeout)
        }))
        .unwrap_or_else(|_| Err(internal_failure("native flush start panicked")))
    });
    match operation {
        Ok(operation) => match Py::new(py, NativeFlushOperation { operation }) {
            Ok(operation) => (Some(operation), result_json::<()>(Ok(()))),
            Err(_) => (
                None,
                result_json::<()>(Err(internal_failure("could not allocate flush observer"))),
            ),
        },
        Err(error) => (None, result_json::<()>(Err(error))),
    }
}

fn log_backend(backend: Arc<dyn HostLoggingBackend>, py: Python<'_>, event: &str) -> String {
    let result = parse_value(event, "event")
        .and_then(sc_observability_dto::decode_event)
        .and_then(|event: LogEventDto| {
            py.detach(move || backend.try_log(event, ProducerOrigin::Python))
        });
    result_json(result)
}

fn query_backend(backend: Arc<dyn HostLoggingBackend>, py: Python<'_>, query: &str) -> String {
    let result = parse_value(query, "query")
        .and_then(sc_observability_dto::decode_query)
        .and_then(|query: LogQueryDto| {
            py.detach(move || {
                let operation = backend.start_query(query)?;
                operation.wait(Duration::from_millis(2_000))
            })
        });
    result_json(result)
}

fn flush_backend(backend: Arc<dyn HostLoggingBackend>, py: Python<'_>, timeout: &str) -> String {
    let result = parse_timeout(timeout).and_then(|timeout| {
        py.detach(move || {
            let operation = backend.start_flush(timeout)?;
            operation.wait(timeout)
        })
    });
    result_json(result)
}

impl NativeLogger {
    fn wait_shutdown(&self, timeout: Duration) -> Result<LogHealthDto, Failure> {
        let operation = {
            let state = self
                .owned
                .lock()
                .map_err(|_| internal_failure("owned logger state lock poisoned"))?;
            state
                .shutdown
                .clone()
                .ok_or_else(|| closed_failure("shutdown has not started"))?
        };
        operation.wait(timeout)
    }

    fn start_shutdown(&self) -> Result<Operation<LogHealthDto>, Failure> {
        let mut state = self
            .owned
            .lock()
            .map_err(|_| internal_failure("owned logger state lock poisoned"))?;
        if let Some(operation) = &state.shutdown {
            return Ok(operation.clone());
        }
        let operation = state.owner.start_shutdown()?;
        state.shutdown = Some(operation.clone());
        Ok(operation)
    }
}

#[pymethods]
impl NativeLogger {
    fn observer_key(&self, py: Python<'_>) -> Py<NativeObserverIdentity> {
        self.observer_identity.clone_ref(py)
    }

    fn start_flush(
        &self,
        py: Python<'_>,
        timeout: &str,
    ) -> (Option<Py<NativeFlushOperation>>, String) {
        start_flush_backend(&self.backend, py, timeout)
    }

    fn log(&self, py: Python<'_>, event: &str) -> String {
        log_backend(Arc::new(self.backend.clone()), py, event)
    }

    fn query(&self, py: Python<'_>, query: &str) -> String {
        query_backend(Arc::new(self.backend.clone()), py, query)
    }

    fn health(&self) -> String {
        result_json(self.backend.health())
    }

    fn flush(&self, py: Python<'_>, timeout: &str) -> String {
        flush_backend(Arc::new(self.backend.clone()), py, timeout)
    }

    fn shutdown(&self, py: Python<'_>, timeout: &str) -> String {
        let result = parse_timeout(timeout).and_then(|timeout| {
            let operation = self.start_shutdown()?;
            py.detach(move || operation.wait(timeout))
        });
        result_json(result)
    }

    fn wait_stopped(&self, py: Python<'_>, timeout: &str) -> String {
        let result = parse_timeout(timeout)
            .and_then(|timeout| py.detach(move || self.wait_shutdown(timeout)));
        result_json(result)
    }

    fn elevate_level(&self, level_value: &str, source_value: &str) -> String {
        let result = level(level_value).and_then(|level_value| {
            source(source_value).and_then(|source_value| {
                let mut state = self
                    .owned
                    .lock()
                    .map_err(|_| internal_failure("owned logger state lock poisoned"))?;
                state.owner.elevate_level(level_value, source_value)
            })
        });
        result_json::<LevelChangeDto>(result)
    }

    fn reset_level(&self, source_value: &str) -> String {
        let result = source(source_value).and_then(|source_value| {
            let mut state = self
                .owned
                .lock()
                .map_err(|_| internal_failure("owned logger state lock poisoned"))?;
            state.owner.reset_level(source_value)
        });
        result_json::<LevelChangeDto>(result)
    }
}

#[pymethods]
impl NativeAttachedLogger {
    fn observer_key(&self, py: Python<'_>) -> Py<NativeObserverIdentity> {
        self.observer_identity.clone_ref(py)
    }

    fn start_flush(
        &self,
        py: Python<'_>,
        timeout: &str,
    ) -> (Option<Py<NativeFlushOperation>>, String) {
        start_flush_backend(self.backend.as_ref(), py, timeout)
    }

    fn log(&self, py: Python<'_>, event: &str) -> String {
        log_backend(self.backend.clone(), py, event)
    }

    fn query(&self, py: Python<'_>, query: &str) -> String {
        query_backend(self.backend.clone(), py, query)
    }

    fn health(&self) -> String {
        result_json(self.backend.health())
    }

    fn flush(&self, py: Python<'_>, timeout: &str) -> String {
        flush_backend(self.backend.clone(), py, timeout)
    }
}

#[pyfunction]
fn create_owned(py: Python<'_>, config: &str) -> PyResult<(Option<Py<NativeLogger>>, String)> {
    match logger_config(config).and_then(create_core_backend) {
        Ok((owner, backend)) => {
            let logger = Py::new(
                py,
                NativeLogger {
                    observer_identity: Py::new(py, NativeObserverIdentity {})?,
                    backend,
                    owned: Mutex::new(OwnedState {
                        owner,
                        shutdown: None,
                    }),
                },
            )?;
            Ok((Some(logger), result_json::<()>(Ok(()))))
        }
        Err(error) => Ok((None, result_json::<()>(Err(error)))),
    }
}

/// Installs a host-owned backend once for this concrete PyO3 module instance.
///
/// The module state owns one `Arc`; each attached handle clones it. Repeated
/// installation cannot replace the first backend and returns tagged data to
/// the Rust embedding caller instead of relying on Python exceptions.
pub fn install_host_logger(
    module: &Bound<'_, PyModule>,
    backend: Arc<dyn HostLoggingBackend>,
) -> Result<(), Failure> {
    let installed = module
        .hasattr("_sc_observability_host_backend")
        .map_err(|error| {
            internal_failure(format!("could not inspect module host state: {error}"))
        })?;
    if installed {
        return Err(unavailable_failure(
            sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_HOST_ALREADY_INSTALLED,
            "a host logger is already installed for this module",
        ));
    }
    let observer_identity = Py::new(module.py(), NativeObserverIdentity {}).map_err(|error| {
        internal_failure(format!("could not allocate observer identity: {error}"))
    })?;
    let slot = Py::new(
        module.py(),
        HostSlot {
            backend,
            observer_identity,
        },
    )
    .map_err(|error| internal_failure(format!("could not allocate module host state: {error}")))?;
    module
        .add("_sc_observability_host_backend", slot)
        .map_err(|error| internal_failure(format!("could not install module host state: {error}")))
}

#[pyfunction]
fn get_installed_host_logger(
    py: Python<'_>,
) -> PyResult<(Option<Py<NativeAttachedLogger>>, String)> {
    let module = match PyModule::import(py, "sc_observability._native") {
        Ok(module) => module,
        Err(error) => {
            return Ok((
                None,
                result_json::<()>(Err(internal_failure(format!(
                    "could not access native module: {error}"
                )))),
            ));
        }
    };
    let slot = match module.getattr("_sc_observability_host_backend") {
        Ok(slot) => slot,
        Err(_) => {
            return Ok((
                None,
                result_json::<()>(Err(unavailable_failure(
                    sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_HOST_NOT_INSTALLED,
                    "host logger has not been installed in this module",
                ))),
            ));
        }
    };
    let slot = match slot.extract::<Py<HostSlot>>() {
        Ok(slot) => slot,
        Err(error) => {
            return Ok((
                None,
                result_json::<()>(Err(internal_failure(format!(
                    "module host state has an invalid type: {error}"
                )))),
            ));
        }
    };
    let state = slot.borrow(py);
    let backend = state.backend.clone();
    let observer_identity = state.observer_identity.clone_ref(py);
    drop(state);
    let logger = Py::new(
        py,
        NativeAttachedLogger {
            backend,
            observer_identity,
        },
    )?;
    Ok((Some(logger), result_json::<()>(Ok(()))))
}

// Validation-only bridge for Python scopes/handler setup. It creates no logger
// and performs no admission; the shared conversion owns every native constraint.
#[pyfunction]
fn _validate_event(payload: &str) -> String {
    let checked = std::panic::catch_unwind(|| {
        let value = sc_observability_dto::decode_event(parse_value(payload, "event")?)?;
        let service = ServiceName::new("python.validation")
            .map_err(|_| internal_failure("invalid private validation service"))?;
        sc_observability_dto::to_core_event(
            value,
            sc_observability_dto::EventStamp {
                service,
                timestamp: sc_observability_types::Timestamp::now_utc(),
                identity: sc_observability_types::ProcessIdentity::default(),
            },
        )
        .map(|_| ())
    })
    .unwrap_or_else(|_| Err(internal_failure("native input validation panicked")));
    result_json(checked)
}

/// Native module used only by the high-level Python facade.
#[pymodule(gil_used = true)]
pub fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeLogger>()?;
    module.add_class::<NativeFlushOperation>()?;
    module.add_class::<NativeAttachedLogger>()?;
    module.add_function(wrap_pyfunction!(_validate_event, module)?)?;
    module.add_function(wrap_pyfunction!(create_owned, module)?)?;
    module.add_function(wrap_pyfunction!(get_installed_host_logger, module)?)?;
    Ok(())
}

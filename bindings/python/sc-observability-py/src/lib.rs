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
use pyo3::sync::MutexExt;
use sc_observability_binding_runtime::{
    CoreLoggerBackend, CoreLoggerOwner, HostLoggingBackend, Operation, ProducerOrigin,
    create_core_backend,
};
use sc_observability_dto::{
    Failure, LevelChangeDto, LogEventDto, LogHealthDto, LogQueryDto, ResultDto,
};
use sc_observability_types::{LevelChangeSource, LevelFilter, ServiceName};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::panic::{AssertUnwindSafe, catch_unwind};
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

/// No Rust panic may cross a public PyO3 call boundary as `PanicException`.
fn contained_json(call: impl FnOnce() -> String) -> String {
    match catch_unwind(AssertUnwindSafe(call)) {
        Ok(result) => result,
        Err(_) => result_json::<()>(Err(internal_failure("native Python entrypoint panicked"))),
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
    backend: CoreLoggerBackend,
    owned: Mutex<OwnedState>,
}

/// Module-owned host backend. Its `Arc` survives while any attached Python
/// handle exists, but it never grants host shutdown or level ownership.
#[pyclass(frozen)]
struct HostSlot {
    backend: Arc<dyn HostLoggingBackend>,
}

#[pyclass]
struct NativeAttachedLogger {
    backend: Arc<dyn HostLoggingBackend>,
}

// This serializes only the check-and-install transition.  The host backend
// itself remains retained exclusively by its concrete Python module slot; no
// process-global backend or ownership capability is introduced.
static HOST_INSTALLATION_LOCK: Mutex<()> = Mutex::new(());

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
    fn log(&self, py: Python<'_>, event: &str) -> String {
        contained_json(|| log_backend(Arc::new(self.backend.clone()), py, event))
    }

    fn query(&self, py: Python<'_>, query: &str) -> String {
        contained_json(|| query_backend(Arc::new(self.backend.clone()), py, query))
    }

    fn health(&self) -> String {
        contained_json(|| result_json(self.backend.health()))
    }

    fn flush(&self, py: Python<'_>, timeout: &str) -> String {
        contained_json(|| flush_backend(Arc::new(self.backend.clone()), py, timeout))
    }

    fn shutdown(&self, py: Python<'_>, timeout: &str) -> String {
        contained_json(|| {
            let result = parse_timeout(timeout).and_then(|timeout| {
                let operation = self.start_shutdown()?;
                py.detach(move || operation.wait(timeout))
            });
            result_json(result)
        })
    }

    fn wait_stopped(&self, py: Python<'_>, timeout: &str) -> String {
        contained_json(|| {
            let result = parse_timeout(timeout)
                .and_then(|timeout| py.detach(move || self.wait_shutdown(timeout)));
            result_json(result)
        })
    }

    fn elevate_level(&self, level_value: &str, source_value: &str) -> String {
        contained_json(|| {
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
        })
    }

    fn reset_level(&self, source_value: &str) -> String {
        contained_json(|| {
            let result = source(source_value).and_then(|source_value| {
                let mut state = self
                    .owned
                    .lock()
                    .map_err(|_| internal_failure("owned logger state lock poisoned"))?;
                state.owner.reset_level(source_value)
            });
            result_json::<LevelChangeDto>(result)
        })
    }
}

#[pymethods]
impl NativeAttachedLogger {
    fn log(&self, py: Python<'_>, event: &str) -> String {
        contained_json(|| log_backend(self.backend.clone(), py, event))
    }

    fn query(&self, py: Python<'_>, query: &str) -> String {
        contained_json(|| query_backend(self.backend.clone(), py, query))
    }

    fn health(&self) -> String {
        contained_json(|| result_json(self.backend.health()))
    }

    fn flush(&self, py: Python<'_>, timeout: &str) -> String {
        contained_json(|| flush_backend(self.backend.clone(), py, timeout))
    }
}

#[pyfunction]
fn create_owned(py: Python<'_>, config: &str) -> PyResult<(Option<Py<NativeLogger>>, String)> {
    match catch_unwind(AssertUnwindSafe(|| {
        match logger_config(config).and_then(create_core_backend) {
            Ok((owner, backend)) => {
                let logger = Py::new(
                    py,
                    NativeLogger {
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
    })) {
        Ok(result) => result,
        Err(_) => Ok((
            None,
            result_json::<()>(Err(internal_failure("native create_owned panicked"))),
        )),
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
    let py = module.py();
    let _installation = HOST_INSTALLATION_LOCK
        .lock_py_attached(py)
        .map_err(|_| internal_failure("host installation lock poisoned"))?;
    // Inspect the module dictionary directly. `hasattr` may invoke a
    // user-defined module `__getattr__`, which is foreign Python code and can
    // detach while the once-only transition is locked.
    let state = module.dict();
    let installed = state
        .contains("_sc_observability_host_backend")
        .map_err(|error| {
            internal_failure(format!("could not inspect module host state: {error}"))
        })?;
    if installed {
        return Err(unavailable_failure(
            sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_HOST_ALREADY_INSTALLED,
            "a host logger is already installed for this module",
        ));
    }
    let slot = Py::new(py, HostSlot { backend }).map_err(|error| {
        internal_failure(format!("could not allocate module host state: {error}"))
    })?;
    state
        .set_item("_sc_observability_host_backend", slot)
        .map_err(|error| internal_failure(format!("could not install module host state: {error}")))
}

#[pyfunction]
fn get_installed_host_logger(
    py: Python<'_>,
) -> PyResult<(Option<Py<NativeAttachedLogger>>, String)> {
    match catch_unwind(AssertUnwindSafe(|| get_installed_host_logger_inner(py))) {
        Ok(result) => result,
        Err(_) => Ok((
            None,
            result_json::<()>(Err(internal_failure("native host factory panicked"))),
        )),
    }
}

fn get_installed_host_logger_inner(
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
    attached_logger_from_module(py, &module)
}

/// Projects a retained module host slot to one non-owning Python handle.
fn attached_logger_from_module(
    py: Python<'_>,
    module: &Bound<'_, PyModule>,
) -> PyResult<(Option<Py<NativeAttachedLogger>>, String)> {
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
    let backend = slot.borrow(py).backend.clone();
    let logger = Py::new(py, NativeAttachedLogger { backend })?;
    Ok((Some(logger), result_json::<()>(Ok(()))))
}

/// Native module used only by the high-level Python facade.
#[pymodule(gil_used = true)]
pub fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeLogger>()?;
    module.add_class::<NativeAttachedLogger>()?;
    module.add_function(wrap_pyfunction!(create_owned, module)?)?;
    module.add_function(wrap_pyfunction!(get_installed_host_logger, module)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_observability_dto::error_codes::{
        SC_OBSERVABILITY_BINDING_HOST_ALREADY_INSTALLED,
        SC_OBSERVABILITY_BINDING_HOST_NOT_INSTALLED,
    };
    use std::sync::{
        Barrier,
        atomic::{AtomicUsize, Ordering},
    };
    use std::thread;
    use std::time::Instant;

    #[test]
    fn host_installation_is_immutable_per_module() {
        Python::initialize();
        let passed = Python::attach(|py| {
            let module = match PyModule::new(py, "_b4_host_install_test") {
                Ok(module) => module,
                Err(_) => return false,
            };
            let service = match ServiceName::new("b4-host-install-test") {
                Ok(service) => service,
                Err(_) => return false,
            };
            let mut config = sc_observability::LoggerConfig::default_for(
                service,
                std::env::temp_dir().join("sc-observability-b4-host-install-test"),
            );
            config.enable_console_sink = false;
            let (owner, backend) = match create_core_backend(config) {
                Ok(pair) => pair,
                Err(_) => return false,
            };
            let first: Arc<dyn HostLoggingBackend> = Arc::new(backend.clone());
            let second: Arc<dyn HostLoggingBackend> = Arc::new(backend);
            let first_install = install_host_logger(&module, first).is_ok();
            let duplicate_is_rejected = matches!(
                install_host_logger(&module, second),
                Err(Failure::Unavailable { diagnostic })
                    if diagnostic.code == SC_OBSERVABILITY_BINDING_HOST_ALREADY_INSTALLED
            );
            let retained_slot = match module.getattr("_sc_observability_host_backend") {
                Ok(slot) => slot.extract::<Py<HostSlot>>().is_ok(),
                Err(_) => false,
            };
            let stopped = owner.shutdown(Duration::from_secs(2)).is_ok();
            first_install && duplicate_is_rejected && retained_slot && stopped
        });
        assert!(passed);
    }

    #[test]
    fn attached_factory_preserves_host_ownership_and_retained_health() {
        Python::initialize();
        let passed = Python::attach(|py| {
            let module = match PyModule::new(py, "_b4_attached_host_test") {
                Ok(module) => module,
                Err(_) => return false,
            };
            let missing_is_tagged = matches!(
                attached_logger_from_module(py, &module),
                Ok((None, result)) if result.contains(SC_OBSERVABILITY_BINDING_HOST_NOT_INSTALLED)
            );
            let service = match ServiceName::new("b4-attached-host-test") {
                Ok(service) => service,
                Err(_) => return false,
            };
            let mut config = sc_observability::LoggerConfig::default_for(
                service,
                std::env::temp_dir().join("sc-observability-b4-attached-host-test"),
            );
            config.enable_console_sink = false;
            let (owner, backend) = match create_core_backend(config) {
                Ok(pair) => pair,
                Err(_) => return false,
            };
            let host: Arc<dyn HostLoggingBackend> = Arc::new(backend);
            if install_host_logger(&module, host).is_err() {
                return false;
            }
            let attached = match attached_logger_from_module(py, &module) {
                Ok((Some(attached), result)) if result.contains("\"kind\":\"ok\"") => attached,
                _ => return false,
            };
            let event = r#"{"schema_version":1,"level":"info","target":"python.attached","action":"host-owned","fields":{}}"#;
            let admitted = attached
                .borrow(py)
                .log(py, event)
                .contains("\"kind\":\"ok\"");
            let stopped = owner.shutdown(Duration::from_secs(2)).is_ok();
            let closed = attached
                .borrow(py)
                .log(py, event)
                .contains(sc_observability_dto::error_codes::SC_OBSERVABILITY_BINDING_CLOSED);
            let retained_health = attached.borrow(py).health().contains("\"kind\":\"ok\"");
            missing_is_tagged && admitted && stopped && closed && retained_health
        });
        assert!(passed);
    }

    #[test]
    fn concurrent_host_installs_have_exactly_one_winner() {
        Python::initialize();
        let setup = Python::attach(|py| {
            let module = match PyModule::new(py, "_b4_concurrent_host_test") {
                Ok(module) => module,
                Err(_) => return None,
            };
            let module_refs = (0..8).map(|_| module.clone().unbind()).collect::<Vec<_>>();
            let service = match ServiceName::new("b4-concurrent-host-test") {
                Ok(service) => service,
                Err(_) => return None,
            };
            let config = sc_observability::LoggerConfig::default_for(
                service,
                std::env::temp_dir().join("sc-observability-b4-concurrent-host-test"),
            );
            match create_core_backend(config) {
                Ok((owner, backend)) => Some((module_refs, owner, Arc::new(backend))),
                Err(_) => None,
            }
        });
        let Some((module_refs, owner, backend)) = setup else {
            assert!(false, "could not create concurrent host-install fixture");
            return;
        };
        let start = Arc::new(Barrier::new(module_refs.len()));
        let winners = Arc::new(AtomicUsize::new(0));
        let rejects = Arc::new(AtomicUsize::new(0));
        let workers = module_refs
            .into_iter()
            .map(|module| {
                let start = start.clone();
                let winners = winners.clone();
                let rejects = rejects.clone();
                let backend: Arc<dyn HostLoggingBackend> = backend.clone();
                thread::spawn(move || {
                    start.wait();
                    Python::attach(|py| match install_host_logger(&module.bind(py), backend) {
                        Ok(()) => winners.fetch_add(1, Ordering::SeqCst),
                        Err(Failure::Unavailable { diagnostic })
                            if diagnostic.code
                                == SC_OBSERVABILITY_BINDING_HOST_ALREADY_INSTALLED =>
                        {
                            rejects.fetch_add(1, Ordering::SeqCst)
                        }
                        Err(_) => 0,
                    });
                })
            })
            .collect::<Vec<_>>();
        let joined = workers.into_iter().all(|worker| worker.join().is_ok());
        let stopped = owner.shutdown(Duration::from_secs(2)).is_ok();
        assert!(joined && stopped);
        assert_eq!(winners.load(Ordering::SeqCst), 1);
        assert_eq!(rejects.load(Ordering::SeqCst), 7);
    }

    #[test]
    fn attached_python_producers_do_not_contend_for_dispatch() {
        Python::initialize();
        let setup = Python::attach(|py| {
            let module = match PyModule::new(py, "_b4_attached_producer_test") {
                Ok(module) => module,
                Err(_) => return None,
            };
            let service = match ServiceName::new("b4-attached-producer-test") {
                Ok(service) => service,
                Err(_) => return None,
            };
            let config = sc_observability::LoggerConfig::default_for(
                service,
                std::env::temp_dir().join("sc-observability-b4-attached-producer-test"),
            );
            let (owner, backend) = match create_core_backend(config) {
                Ok(pair) => pair,
                Err(_) => return None,
            };
            let host: Arc<dyn HostLoggingBackend> = Arc::new(backend);
            if install_host_logger(&module, host).is_err() {
                return None;
            }
            let attached = match attached_logger_from_module(py, &module) {
                Ok((Some(attached), _)) => attached,
                _ => return None,
            };
            Some((
                (0..32).map(|_| attached.clone_ref(py)).collect::<Vec<_>>(),
                owner,
            ))
        });
        let Some((attached, owner)) = setup else {
            assert!(false, "could not create attached producer fixture");
            return;
        };
        let start = Arc::new(Barrier::new(attached.len()));
        let accepted = Arc::new(AtomicUsize::new(0));
        let workers = attached
            .into_iter()
            .map(|attached| {
                let start = start.clone();
                let accepted = accepted.clone();
                thread::spawn(move || {
                    start.wait();
                    Python::attach(|py| {
                        let event = r#"{"schema_version":1,"level":"info","target":"python.attached","action":"parallel","fields":{}}"#;
                        let result = attached.bind(py).borrow().log(py, event);
                        if result.contains("\"kind\":\"ok\"") {
                            accepted.fetch_add(1, Ordering::SeqCst);
                        }
                    });
                })
            })
            .collect::<Vec<_>>();
        let joined = workers.into_iter().all(|worker| worker.join().is_ok());
        let stopped = owner.shutdown(Duration::from_secs(2)).is_ok();
        assert!(joined && stopped);
        assert_eq!(accepted.load(Ordering::SeqCst), 32);
    }

    #[test]
    fn module_collection_keeps_live_attached_handle_nonowning() {
        Python::initialize();
        let setup = Python::attach(|py| {
            let module = match PyModule::new(py, "_b4_module_collection_test") {
                Ok(module) => module,
                Err(_) => return None,
            };
            let service = match ServiceName::new("b4-module-collection-test") {
                Ok(service) => service,
                Err(_) => return None,
            };
            let config = sc_observability::LoggerConfig::default_for(
                service,
                std::env::temp_dir().join("sc-observability-b4-module-collection-test"),
            );
            let (owner, backend) = match create_core_backend(config) {
                Ok(pair) => pair,
                Err(_) => return None,
            };
            if install_host_logger(&module, Arc::new(backend)).is_err() {
                return None;
            }
            let attached = match attached_logger_from_module(py, &module) {
                Ok((Some(attached), _)) => attached,
                _ => return None,
            };
            // The attached handle must keep the backend Arc, not the module or
            // the host owner. Dropping this module therefore cannot stop it.
            drop(module);
            Some((owner, attached))
        });
        let Some((owner, attached)) = setup else {
            assert!(false, "could not create module collection fixture");
            return;
        };
        let active = Python::attach(|py| {
            let event = r#"{"schema_version":1,"level":"info","target":"python.attached","action":"module-collected","fields":{}}"#;
            attached
                .bind(py)
                .borrow()
                .log(py, event)
                .contains("\"kind\":\"ok\"")
        });
        let stopped = owner.shutdown(Duration::from_secs(2)).is_ok();
        let retained = Python::attach(|py| {
            attached
                .bind(py)
                .borrow()
                .health()
                .contains("\"kind\":\"ok\"")
        });
        assert!(active && stopped && retained);
    }

    fn install_during_releasing_module_hook() -> bool {
        Python::initialize();
        let setup = Python::attach(|py| {
            let module = match PyModule::new(py, "_b4_install_hook_test") {
                Ok(module) => module,
                Err(_) => return None,
            };
            let hooks = match PyModule::from_code(
                py,
                pyo3::ffi::c_str!(
                    "import time\ndef release_gil(name):\n    time.sleep(0.02)\n    raise AttributeError(name)\n"
                ),
                pyo3::ffi::c_str!("install_hook.py"),
                pyo3::ffi::c_str!("_b4_install_hook_support"),
            ) {
                Ok(hooks) => hooks,
                Err(_) => return None,
            };
            let hook = match hooks.getattr("release_gil") {
                Ok(hook) => hook,
                Err(_) => return None,
            };
            if module.dict().set_item("__getattr__", hook).is_err() {
                return None;
            }
            let service = match ServiceName::new("b4-install-hook-test") {
                Ok(service) => service,
                Err(_) => return None,
            };
            let config = sc_observability::LoggerConfig::default_for(
                service,
                std::env::temp_dir().join("sc-observability-b4-install-hook-test"),
            );
            let (owner, backend) = match create_core_backend(config) {
                Ok(pair) => pair,
                Err(_) => return None,
            };
            Some((
                vec![module.clone().unbind(), module.clone().unbind()],
                module.unbind(),
                owner,
                Arc::new(backend),
            ))
        });
        let Some((modules, hook_module, owner, backend)) = setup else {
            return false;
        };
        let start = Arc::new(Barrier::new(3));
        let winners = Arc::new(AtomicUsize::new(0));
        let duplicates = Arc::new(AtomicUsize::new(0));
        let installers = modules
            .into_iter()
            .map(|module| {
                let start = start.clone();
                let winners = winners.clone();
                let duplicates = duplicates.clone();
                let backend: Arc<dyn HostLoggingBackend> = backend.clone();
                thread::spawn(move || {
                    start.wait();
                    Python::attach(|py| match install_host_logger(&module.bind(py), backend) {
                        Ok(()) => winners.fetch_add(1, Ordering::SeqCst),
                        Err(Failure::Unavailable { diagnostic })
                            if diagnostic.code
                                == SC_OBSERVABILITY_BINDING_HOST_ALREADY_INSTALLED =>
                        {
                            duplicates.fetch_add(1, Ordering::SeqCst)
                        }
                        Err(_) => 0,
                    });
                })
            })
            .collect::<Vec<_>>();
        let hook_start = start.clone();
        let hook = thread::spawn(move || {
            hook_start.wait();
            Python::attach(|py| hook_module.bind(py).getattr("missing_attribute").is_err())
        });
        let installed = installers.into_iter().all(|worker| worker.join().is_ok());
        let hook_finished = hook.join().is_ok_and(|result| result);
        let stopped = owner.shutdown(Duration::from_secs(2)).is_ok();
        installed
            && hook_finished
            && stopped
            && winners.load(Ordering::SeqCst) == 1
            && duplicates.load(Ordering::SeqCst) == 1
    }

    #[test]
    fn host_install_with_releasing_module_hook_is_bounded() {
        if std::env::var_os("SC_B4_INSTALL_HOOK_CHILD").is_some() {
            assert!(install_during_releasing_module_hook());
            return;
        }
        let executable = match std::env::current_exe() {
            Ok(executable) => executable,
            Err(_) => {
                assert!(false, "could not find the binding test executable");
                return;
            }
        };
        let mut child = match std::process::Command::new(executable)
            .arg("--exact")
            .arg("tests::host_install_with_releasing_module_hook_is_bounded")
            .arg("--nocapture")
            .env("SC_B4_INSTALL_HOOK_CHILD", "1")
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                assert!(false, "could not start host-install hook subprocess");
                return;
            }
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    assert!(status.success(), "host-install hook subprocess failed");
                    return;
                }
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    assert!(false, "host-install hook subprocess exceeded five seconds");
                    return;
                }
                Err(_) => {
                    let _ = child.kill();
                    assert!(false, "could not observe host-install hook subprocess");
                    return;
                }
            }
        }
    }
}

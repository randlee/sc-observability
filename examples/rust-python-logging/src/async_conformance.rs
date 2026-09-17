//! B.6 real embedded writer holds: only this executable owns test controls.
use pyo3::prelude::*;
use pyo3::types::PyDict;
use sc_observability_binding_runtime::{HostLoggingBackend, Operation, ProducerOrigin};
use sc_observability_dto::{
    AdmissionDto, CompletionDto, Failure, LogEventDto, LogHealthDto, LogQueryDto, LogSnapshotDto,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;

struct Counted {
    backend: Arc<dyn HostLoggingBackend>,
    flushes: Arc<AtomicUsize>,
    recorded: Option<Arc<std::sync::OnceLock<Operation<CompletionDto>>>>,
}
impl HostLoggingBackend for Counted {
    fn try_log(&self, event: LogEventDto, origin: ProducerOrigin) -> Result<AdmissionDto, Failure> {
        self.backend.try_log(event, origin)
    }
    fn start_query(&self, query: LogQueryDto) -> Result<Operation<LogSnapshotDto>, Failure> {
        self.backend.start_query(query)
    }
    fn health(&self) -> Result<LogHealthDto, Failure> {
        self.backend.health()
    }
    fn start_flush(&self, timeout: Duration) -> Result<Operation<CompletionDto>, Failure> {
        self.flushes.fetch_add(1, Ordering::SeqCst);
        let operation = self.backend.start_flush(timeout)?;
        if let Some(recorded) = &self.recorded {
            let _ = recorded.set(operation.clone());
        }
        Ok(operation)
    }
}

#[pyclass]
struct Hold {
    release: Option<mpsc::Sender<()>>,
    flushes: Arc<AtomicUsize>,
}
#[pymethods]
impl Hold {
    fn hold(&mut self) -> PyResult<()> {
        self.release();
        let (release, receive) = mpsc::channel();
        let (ready, entered) = mpsc::sync_channel(0);
        std::thread::spawn(move || {
            let stdout = std::io::stdout();
            let lock = stdout.lock();
            let _ = ready.send(());
            let _ = receive.recv_timeout(Duration::from_secs(20));
            drop(lock);
        });
        entered
            .recv_timeout(Duration::from_secs(5))
            .map_err(failure)?;
        self.release = Some(release);
        Ok(())
    }
    fn release(&mut self) {
        if let Some(sender) = self.release.take() {
            let _ = sender.send(());
        }
    }
    fn flush_calls(&self) -> usize {
        self.flushes.load(Ordering::SeqCst)
    }
}
impl Drop for Hold {
    fn drop(&mut self) {
        self.release();
    }
}

fn failure(error: impl std::fmt::Debug) -> PyErr {
    pyo3::exceptions::PyRuntimeError::new_err(format!("{error:?}"))
}

/// Run real owned/core-attached/bridge-attached asyncio tests in this interpreter.
///
/// # Errors
/// Returns an error if native construction, Python checks or a subprocess fail.
pub fn run(py: Python<'_>) -> PyResult<()> {
    for mode in ["owned", "core", "bridge"] {
        let flushes = Arc::new(AtomicUsize::new(0));
        let service =
            sc_observability_types::ServiceName::new(format!("b6-{mode}")).map_err(failure)?;
        let root = std::env::temp_dir().join(format!("b6-async-{}-{mode}", std::process::id()));
        let mut config = sc_observability::LoggerConfig::default_for(service, root.clone());
        config.enable_console_sink = true;
        config.queue_capacity = 128;
        let mut core_owner = None;
        let mut bridge_owner = None;
        let backend: Option<Arc<dyn HostLoggingBackend>> = match mode {
            "core" => {
                let (owner, backend) =
                    sc_observability_binding_runtime::create_core_backend(config)
                        .map_err(failure)?;
                core_owner = Some(owner);
                Some(Arc::new(backend))
            }
            "bridge" => {
                let guard = sc_observability_log::init(
                    config,
                    sc_observability_log::BridgeOptions {
                        default_action: sc_observability_types::ActionName::new("async.host")
                            .map_err(failure)?,
                        parse_bracket_action: false,
                    },
                )
                .map_err(failure)?;
                let backend = sc_observability_binding_runtime::bridge_backend(guard.control())
                    .map_err(failure)?;
                bridge_owner = Some(guard);
                Some(Arc::new(backend))
            }
            _ => None,
        };
        let module = PyModule::new(py, "_native")?;
        binding::_native(&module)?;
        if let Some(backend) = backend {
            binding::install_host_logger(
                &module,
                Arc::new(Counted {
                    backend,
                    flushes: flushes.clone(),
                    recorded: None,
                }),
            )
            .map_err(failure)?;
        }
        let modules = PyModule::import(py, "sys")?
            .getattr("modules")?
            .cast_into::<PyDict>()?;
        modules.set_item("sc_observability._native", &module)?;
        let (release, receive) = mpsc::channel();
        let (ready, entered) = mpsc::sync_channel(0);
        let held = std::thread::spawn(move || {
            let stdout = std::io::stdout();
            let lock = stdout.lock();
            let _ = ready.send(());
            let _ = receive.recv_timeout(Duration::from_secs(20));
            drop(lock);
        });
        entered
            .recv_timeout(Duration::from_secs(5))
            .map_err(failure)?;
        let controls = Py::new(
            py,
            Hold {
                release: Some(release),
                flushes,
            },
        )?;
        let globals = PyDict::new(py);
        globals.set_item("controls", &controls)?;
        globals.set_item("mode", mode)?;
        globals.set_item("root", root.to_string_lossy().as_ref())?;
        let code = std::ffi::CString::new(include_str!("async_conformance.py")).map_err(failure)?;
        let result = py.run(code.as_c_str(), Some(&globals), None);
        controls.borrow_mut(py).release();
        py.detach(move || held.join()).map_err(failure)?;
        if let Some(owner) = core_owner {
            py.detach(move || owner.shutdown(Duration::from_secs(5)))
                .map_err(failure)?;
        }
        if let Some(guard) = bridge_owner {
            py.detach(move || guard.shutdown(Duration::from_secs(5)))
                .map_err(failure)?;
        }
        result?;
    }
    for mode in ["core", "bridge"] {
        py.detach(move || finalization_child(mode))
            .map_err(failure)?;
    }
    println!(
        "B6_EMBEDDED_ASYNC_PASSED: owned/core/bridge held writer, heartbeat, cancellation, timeout and admission"
    );
    Ok(())
}

/// Isolated process: finalize Python while a native flush is held, then let
/// that exact native operation complete and shut down its Rust owner afterward.
///
/// # Errors
/// Returns the failed native/embedding check as owned text, without retaining
/// Python values after interpreter finalization.
pub fn finalize(mode: &str) -> Result<(), String> {
    let service =
        sc_observability_types::ServiceName::new("b6-finalize").map_err(|e| e.to_string())?;
    let root = std::env::temp_dir().join(format!("b6-finalize-{}", std::process::id()));
    let mut config = sc_observability::LoggerConfig::default_for(service, root);
    config.enable_console_sink = true;
    let mut core_owner = None;
    let mut bridge_owner = None;
    let backend: Arc<dyn HostLoggingBackend> = if mode == "bridge" {
        let guard = sc_observability_log::init(
            config,
            sc_observability_log::BridgeOptions {
                default_action: sc_observability_types::ActionName::new("finalize.host")
                    .map_err(|e| e.to_string())?,
                parse_bracket_action: false,
            },
        )
        .map_err(|e| format!("{e:?}"))?;
        let backend = sc_observability_binding_runtime::bridge_backend(guard.control())
            .map_err(|e| format!("{e:?}"))?;
        bridge_owner = Some(guard);
        Arc::new(backend)
    } else {
        let (owner, backend) = sc_observability_binding_runtime::create_core_backend(config)
            .map_err(|e| format!("{e:?}"))?;
        core_owner = Some(owner);
        Arc::new(backend)
    };
    let recorded = Arc::new(std::sync::OnceLock::new());
    let flushes = Arc::new(AtomicUsize::new(0));
    let counted: Arc<dyn HostLoggingBackend> = Arc::new(Counted {
        backend: backend.clone(),
        flushes: flushes.clone(),
        recorded: Some(recorded.clone()),
    });
    let stdout = std::io::stdout();
    let hold = stdout.lock();
    // SAFETY: main selects this isolated subprocess branch before any Python
    // initialization. It executes once; the closure returns only owned Rust
    // Result<(), String>, and all subsequent work uses native Rust values only.
    let python_result = unsafe {
        pyo3::with_embedded_python_interpreter(|py| -> Result<(), String> {
            let action = || -> PyResult<()> {
                let module = PyModule::new(py, "_native")?;
                binding::_native(&module)?;
                binding::install_host_logger(&module, counted).map_err(failure)?;
                let sys = PyModule::import(py, "sys")?;
                sys.getattr("modules")?
                    .cast_into::<PyDict>()?
                    .set_item("sc_observability._native", module)?;
                let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../bindings/python/sc-observability-py/python");
                sys.getattr("path")?
                    .call_method1("insert", (0, source.to_string_lossy().as_ref()))?;
                py.run(c"import asyncio\nfrom sc_observability import Ok,LogEvent,get_host_logger\nfrom sc_observability.async_logging import _pools\nlogger=get_host_logger().value\nreceipt=logger.submit(LogEvent(level='info',target='finalize.host',action='held'))\nassert isinstance(receipt,Ok)\nloop=asyncio.new_event_loop()\nwait=logger.flush_async(60000)\nloop.call_soon(wait.send,None)\nloop.run_until_complete(asyncio.sleep(0))\nassert len(_pools[logger._native.observer_key()].observers)==1\nloop.close()", None, None)
            };
            action().map_err(|error| error.to_string())
        })
    };
    // Python is finalized. The held native operation must still be pending.
    let operation = recorded
        .get()
        .ok_or_else(|| "native flush was never started".to_owned())?;
    if !matches!(
        operation.state(),
        sc_observability_binding_runtime::OperationState::Pending
    ) {
        return Err("native flush unexpectedly finished before writer release".into());
    }
    drop(hold);
    operation
        .wait(Duration::from_secs(5))
        .map_err(|e| format!("{e:?}"))?;
    if flushes.load(Ordering::SeqCst) != 1 {
        return Err("native flush was resubmitted".into());
    }
    backend.health().map_err(|e| format!("{e:?}"))?;
    if let Some(owner) = core_owner {
        owner
            .shutdown(Duration::from_secs(5))
            .map_err(|e| format!("{e:?}"))?;
    }
    if let Some(guard) = bridge_owner {
        guard
            .shutdown(Duration::from_secs(5))
            .map_err(|e| format!("{e:?}"))?;
    }
    python_result?;
    println!("B6_FINALIZATION_PASSED: {mode} native flush completed after Python finalized");
    Ok(())
}

fn finalization_child(mode: &str) -> Result<(), String> {
    let mut child = std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
        .arg(format!("--b6-finalize={mode}"))
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return if status.success() {
                Ok(())
            } else {
                Err(format!("{mode} finalization subprocess failed"))
            };
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{mode} finalization subprocess timed out"));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

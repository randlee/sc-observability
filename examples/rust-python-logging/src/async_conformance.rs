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
        self.backend.start_flush(timeout)
    }
}

#[pyclass]
struct Hold {
    release: Option<mpsc::Sender<()>>,
    flushes: Arc<AtomicUsize>,
}
#[pymethods]
impl Hold {
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
    println!(
        "B6_EMBEDDED_ASYNC_PASSED: owned/core/bridge held writer, heartbeat, cancellation, timeout and admission"
    );
    Ok(())
}

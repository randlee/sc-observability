//! Fixed-resource native backends shared by language bindings.
//!
//! Backend handles admit work without owning shutdown. Only `CoreLoggerOwner`
//! holds core shutdown and level-mutation authority; bridge owners stay with hosts.
mod callback;
mod conversion;
mod coordinator;
mod error;
mod operation;
mod spawn;
mod sync;
#[cfg(test)]
mod tests;
mod timer;
use coordinator::Coordinator;
pub use operation::{CompletionSubscription, Operation, OperationState};
use sc_observability_dto::{
    AdmissionDto, CompletionDto, Failure, LevelChangeDto, LogEventDto, LogHealthDto, LogQueryDto,
    LogSnapshotDto,
};
#[cfg(feature = "test-hooks")]
use sc_observability_types::DiagnosticInfo;
use sc_observability_types::{LevelChangeSource, LevelFilter};
use std::sync::{Arc, atomic::Ordering};
use std::time::Duration;

#[cfg(feature = "test-hooks")]
use std::sync::{Condvar, Mutex as StdMutex};

/// Trusted adapter identity; never selected from producer event fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProducerOrigin {
    /// Frontend event stamped as TypeScript through Tauri.
    TauriFrontend,
    /// Python event stamped as Python through `PyO3`.
    Python,
    /// Trusted Rust host event stamped as native Rust.
    RustHost,
}
/// Read-only lifecycle capability used by host and language clients.
pub trait HostLoggingBackend: Send + Sync {
    /// Validates and admits one event, returning the final native admission.
    ///
    /// # Errors
    /// Validation, bounded capacity, lifecycle or native diagnostic failure.
    fn try_log(&self, event: LogEventDto, origin: ProducerOrigin) -> Result<AdmissionDto, Failure>;
    /// Reserves the single query slot and starts one native query.
    ///
    /// # Errors
    /// Validation, bounded capacity, lifecycle or native diagnostic failure.
    fn start_query(&self, query: LogQueryDto) -> Result<Operation<LogSnapshotDto>, Failure>;
    /// Reads native health, or the retained snapshot after admission closes.
    ///
    /// # Errors
    /// Validation, bounded capacity, lifecycle or native diagnostic failure.
    fn health(&self) -> Result<LogHealthDto, Failure>;
    /// Reserves one flush slot; observer deadlines never cancel native work.
    ///
    /// # Errors
    /// Validation, bounded capacity, lifecycle or native diagnostic failure.
    fn start_flush(&self, native_timeout: Duration) -> Result<Operation<CompletionDto>, Failure>;
}
/// Cloneable core access without shutdown or level-mutation authority.
///
/// ```compile_fail
/// fn cannot_shutdown(backend: sc_observability_binding_runtime::CoreLoggerBackend) {
///     backend.start_shutdown();
/// }
/// ```
/// ```compile_fail
/// fn cannot_mutate(mut backend: sc_observability_binding_runtime::CoreLoggerBackend) {
///     backend.reset_level(sc_observability_types::LevelChangeSource::Application);
/// }
/// ```
#[derive(Clone)]
pub struct CoreLoggerBackend {
    shared: Arc<Coordinator>,
}
/// Unique core ownership. Drop requests shutdown without joining a helper.
///
/// ```compile_fail
/// fn cannot_clone(owner: sc_observability_binding_runtime::CoreLoggerOwner) {
///     let duplicate = owner.clone();
/// }
/// ```
pub struct CoreLoggerOwner {
    shared: Arc<Coordinator>,
}
/// Cloneable bridge access; it never retains the host's `LogGuard`.
///
/// ```compile_fail
/// fn cannot_shutdown(backend: sc_observability_binding_runtime::BridgeControlBackend) {
///     backend.start_shutdown();
/// }
/// ```
pub struct BridgeControlBackend {
    shared: Arc<Coordinator>,
}
macro_rules! backend_impl {
    ($ty:ty) => {
        impl HostLoggingBackend for $ty {
            fn try_log(
                &self,
                event: LogEventDto,
                origin: ProducerOrigin,
            ) -> Result<AdmissionDto, Failure> {
                self.shared.log(event, origin)
            }
            fn start_query(
                &self,
                query: LogQueryDto,
            ) -> Result<Operation<LogSnapshotDto>, Failure> {
                self.shared.query(query)
            }
            fn health(&self) -> Result<LogHealthDto, Failure> {
                self.shared.health()
            }
            fn start_flush(&self, timeout: Duration) -> Result<Operation<CompletionDto>, Failure> {
                self.shared.flush(timeout)
            }
        }
        impl std::fmt::Debug for $ty {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(stringify!($ty)).finish_non_exhaustive()
            }
        }
    };
}
backend_impl!(CoreLoggerBackend);
backend_impl!(BridgeControlBackend);
impl std::fmt::Debug for CoreLoggerOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoreLoggerOwner").finish_non_exhaustive()
    }
}
impl Clone for BridgeControlBackend {
    fn clone(&self) -> Self {
        self.shared.handles.fetch_add(1, Ordering::SeqCst);
        Self {
            shared: self.shared.clone(),
        }
    }
}
impl Drop for BridgeControlBackend {
    fn drop(&mut self) {
        if self.shared.handles.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.shared.close();
        }
    }
}
impl Drop for CoreLoggerOwner {
    fn drop(&mut self) {
        self.shared.close();
    }
}
/// Reserves the fixed helpers before constructing one owned core logger.
///
/// # Errors
/// Returns native initialization, timer, or helper reservation diagnostics.
pub fn create_core_backend(
    config: sc_observability::LoggerConfig,
) -> Result<(CoreLoggerOwner, CoreLoggerBackend), Failure> {
    let shared = coordinator::core(config)?;
    Ok((
        CoreLoggerOwner {
            shared: shared.clone(),
        },
        CoreLoggerBackend { shared },
    ))
}

/// Feature-only gate for proving that a real logger sink can retain its writer.
#[cfg(feature = "test-hooks")]
#[derive(Debug)]
pub struct TestWriterGate {
    state: StdMutex<(bool, bool)>,
    changed: Condvar,
}

#[cfg(feature = "test-hooks")]
impl TestWriterGate {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            state: StdMutex::new((false, false)),
            changed: Condvar::new(),
        })
    }

    /// Returns whether the real sink writer entered its held write.
    pub fn entered(&self) -> bool {
        self.state.lock().map(|state| state.0).unwrap_or(false)
    }

    /// Releases the real sink writer after another binding operation has run.
    pub fn release(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.1 = true;
            self.changed.notify_all();
        }
    }
}

#[cfg(feature = "test-hooks")]
struct TestBlockingSink {
    gate: Arc<TestWriterGate>,
}

#[cfg(feature = "test-hooks")]
#[allow(
    deprecated,
    reason = "the test-only sink implements the published compatibility trait signature"
)]
impl sc_observability::LogSink for TestBlockingSink {
    fn write(&self, _: &sc_observability::LogEvent) -> Result<(), sc_observability::LogSinkError> {
        let mut state = self
            .gate
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.0 = true;
        self.gate.changed.notify_all();
        while !state.1 {
            state = self
                .gate
                .changed
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        Ok(())
    }

    fn health(&self) -> sc_observability::SinkHealth {
        sc_observability::SinkHealth {
            name: sc_observability::SinkName::new("binding-test-held-writer")
                .unwrap_or_else(|_| unreachable!("static sink name is valid")),
            state: sc_observability::SinkHealthState::Healthy,
            last_error: None,
        }
    }
}

/// Creates a feature-only owned core whose actual sink blocks its writer.
///
/// # Errors
/// Returns the same tagged construction failure as a production core backend.
#[cfg(feature = "test-hooks")]
pub fn create_test_blocking_core_backend(
    config: sc_observability::LoggerConfig,
) -> Result<(CoreLoggerOwner, CoreLoggerBackend, Arc<TestWriterGate>), Failure> {
    let gate = TestWriterGate::new();
    let gate_for_sink = gate.clone();
    let shared = coordinator::core_from_test_factory(move || {
        let stamp = sc_observability_dto::EventStamp {
            service: config.service_name.clone(),
            timestamp: sc_observability_types::Timestamp::now_utc(),
            identity: sc_observability_types::ProcessIdentity::default(),
        };
        let mut builder = sc_observability::Logger::builder_typed(config).map_err(|error| {
            conversion::context(error.diagnostic(), conversion::Kind::Unavailable)
        })?;
        builder.register_sink(sc_observability::SinkRegistration::new(Arc::new(
            TestBlockingSink {
                gate: gate_for_sink,
            },
        )));
        let (logger, level) = builder.build_with_level_owner_typed().map_err(|error| {
            conversion::context(error.diagnostic(), conversion::Kind::Unavailable)
        })?;
        Ok((stamp, logger, level))
    })?;
    Ok((
        CoreLoggerOwner {
            shared: shared.clone(),
        },
        CoreLoggerBackend { shared },
        gate,
    ))
}
/// Attaches bounded operations to an existing host-owned bridge control.
///
/// # Errors
/// Returns initialization or unavailable native snapshot diagnostics.
pub fn bridge_backend(
    control: sc_observability_log::LogControl,
) -> Result<BridgeControlBackend, Failure> {
    Ok(BridgeControlBackend {
        shared: coordinator::bridge(control)?,
    })
}
impl CoreLoggerOwner {
    #[cfg(feature = "test-hooks")]
    /// Forces the owned core's real level revision to exhaustion for binding tests.
    ///
    /// # Errors
    ///
    /// Returns a tagged failure when the owner is no longer available.
    pub fn force_revision_exhaustion_for_test(&mut self) -> Result<(), Failure> {
        self.shared.force_revision_exhaustion_for_test()
    }

    /// Closes admission once and returns the same saved shutdown operation.
    ///
    /// # Errors
    /// Native shutdown failures are retained by the returned operation.
    pub fn start_shutdown(&self) -> Result<Operation<LogHealthDto>, Failure> {
        self.shared.close();
        Ok(self.shared.shutdown.clone())
    }
    /// Starts shutdown and waits only for the specified observation deadline.
    ///
    /// # Errors
    /// Invalid duration, observer saturation, timeout or saved native failure.
    pub fn shutdown(&self, timeout: Duration) -> Result<LogHealthDto, Failure> {
        error::duration(timeout)?;
        self.start_shutdown()?.wait(timeout)
    }
    /// Observes the existing shutdown operation without starting shutdown.
    ///
    /// # Errors
    /// Invalid duration, observer saturation, timeout or saved native failure.
    pub fn wait_stopped(&self, timeout: Duration) -> Result<LogHealthDto, Failure> {
        self.shared.shutdown.wait(timeout)
    }
    /// Changes level directly, independently of queued native I/O.
    ///
    /// # Errors
    /// Closed admission, busy owner gate, or the original native level failure.
    pub fn elevate_level(
        &mut self,
        level: LevelFilter,
        source: LevelChangeSource,
    ) -> Result<LevelChangeDto, Failure> {
        self.shared
            .level(|owner| owner.elevate_level(level, source))
    }
    /// Restores the configured baseline without queuing behind I/O.
    ///
    /// # Errors
    /// Closed admission, busy owner gate, or the original native level failure.
    pub fn reset_level(&mut self, source: LevelChangeSource) -> Result<LevelChangeDto, Failure> {
        self.shared.level(|owner| owner.reset_level(source))
    }
}

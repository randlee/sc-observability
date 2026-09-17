//! Fixed workers and shared admission; no client handle owns a logger reference.
use crate::{
    Operation, ProducerOrigin, callback::Dispatcher, conversion, error, sync::lock,
    timer::TimerService,
};
use arc_swap::{ArcSwap, ArcSwapOption};
use sc_observability::{LevelOwner, Logger, Running};
use sc_observability_dto::{self as dto, CompletionDto, Failure, LogHealthDto, LogSnapshotDto};
use sc_observability_types::{self as native, DiagnosticInfo};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

pub(crate) enum Backend {
    Core {
        logger: ArcSwapOption<Logger<Running>>,
        level: Mutex<LevelOwner>,
        stamp: dto::EventStamp,
    },
    Bridge(sc_observability_log::LogControl),
}
enum Work {
    Query(Box<native::LogQuery>, Operation<LogSnapshotDto>),
    Flush(Duration, Operation<CompletionDto>),
}
struct Queue {
    items: VecDeque<Work>,
    query: bool,
    flush: bool,
}
pub(crate) struct Coordinator {
    pub(crate) backend: Backend,
    pub(crate) snapshot: ArcSwap<LogHealthDto>,
    closed: AtomicBool,
    active: AtomicUsize,
    failed: AtomicBool,
    // MUTEX: bounded queue/slot bookkeeping and worker sleep predicates only;
    // native calls and notifications execute outside this critical section.
    queue: Mutex<Queue>,
    changed: Condvar,
    operation_exited: AtomicBool,
    pub(crate) dispatcher: Arc<Dispatcher>,
    timer: Arc<TimerService>,
    pub(crate) shutdown: Operation<LogHealthDto>,
    pub(crate) handles: AtomicUsize,
    #[cfg(test)]
    pub(crate) hooks: TestHooks,
}
struct Admission<'a>(&'a Coordinator);
impl Drop for Admission<'_> {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        if self.0.closed.load(Ordering::SeqCst) {
            let _queue = lock(&self.0.queue);
            self.0.changed.notify_all();
        }
    }
}
enum Start {
    Parked,
    Run(Arc<Coordinator>),
    Abort,
}
struct StartGate {
    state: Mutex<Start>,
    changed: Condvar,
}
impl StartGate {
    fn take(&self) -> Option<Arc<Coordinator>> {
        let mut state = lock(&self.state);
        loop {
            match &*state {
                Start::Run(value) => return Some(value.clone()),
                Start::Abort => return None,
                Start::Parked => {
                    state = self
                        .changed
                        .wait(state)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
            }
        }
    }
    fn set(&self, state: Start) {
        *lock(&self.state) = state;
        self.changed.notify_all();
    }
}
impl Coordinator {
    pub(crate) fn create(
        build: impl FnOnce() -> Result<(Backend, LogHealthDto), Failure>,
    ) -> Result<Arc<Self>, Failure> {
        let timer = crate::timer::shared()?;
        let dispatcher = Dispatcher::new();
        let gate = Arc::new(StartGate {
            state: Mutex::new(Start::Parked),
            changed: Condvar::new(),
        });
        let mut helpers = Vec::with_capacity(3);
        for (index, name) in [
            "binding-operation",
            "binding-lifecycle",
            "binding-completion",
        ]
        .into_iter()
        .enumerate()
        {
            let worker_gate = gate.clone();
            match crate::spawn::spawn(name, move || {
                let state = worker_gate.take();
                drop(worker_gate);
                if let Some(shared) = state {
                    match index {
                        0 => shared.run_operations(),
                        1 => shared.run_lifecycle(),
                        _ => shared.dispatcher.run(),
                    }
                }
            }) {
                Ok(handle) => helpers.push(handle),
                Err(cause) => {
                    gate.set(Start::Abort);
                    for helper in helpers {
                        let _ = helper.join();
                    }
                    return Err(error::start_failed(cause.to_string()));
                }
            }
        }
        let constructed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(build))
            .unwrap_or_else(|_| Err(error::internal("native constructor panicked")));
        let (backend, health) = match constructed {
            Ok(value) => value,
            Err(error) => {
                gate.set(Start::Abort);
                for helper in helpers {
                    let _ = helper.join();
                }
                return Err(error);
            }
        };
        let shutdown = Operation::new(&dispatcher, &timer);
        let shared = Arc::new(Self {
            backend,
            snapshot: ArcSwap::from_pointee(health),
            closed: AtomicBool::new(false),
            active: AtomicUsize::new(0),
            failed: AtomicBool::new(false),
            queue: Mutex::new(Queue {
                items: VecDeque::with_capacity(2),
                query: false,
                flush: false,
            }),
            changed: Condvar::new(),
            operation_exited: AtomicBool::new(false),
            dispatcher,
            timer,
            shutdown,
            handles: AtomicUsize::new(1),
            #[cfg(test)]
            hooks: TestHooks::default(),
        });
        gate.set(Start::Run(shared.clone()));
        Ok(shared)
    }
    fn enter(&self) -> Result<Admission<'_>, Failure> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(error::closed());
        }
        self.active.fetch_add(1, Ordering::SeqCst);
        let admission = Admission(self);
        #[cfg(test)]
        {
            let gate = lock(&self.hooks.admission).clone();
            if let Some(gate) = gate {
                gate.arrive();
            }
        }
        if self.closed.load(Ordering::SeqCst) {
            drop(admission);
            return Err(error::closed());
        }
        Ok(admission)
    }
    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        let _queue = lock(&self.queue);
        self.changed.notify_all();
    }
    fn protected<T>(&self, call: impl FnOnce() -> Result<T, Failure>) -> Result<T, Failure> {
        let _admission = self.enter()?;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(call))
            .unwrap_or_else(|_| Err(error::internal("native operation panicked")))
    }
    pub(crate) fn log(
        &self,
        event: dto::LogEventDto,
        origin: ProducerOrigin,
    ) -> Result<dto::AdmissionDto, Failure> {
        self.protected(|| match &self.backend {
            Backend::Core { logger, stamp, .. } => {
                let mut stamp = stamp.clone();
                stamp.timestamp = native::Timestamp::now_utc();
                let event = conversion::event(event, stamp, origin)?;
                let logger = logger.load_full().ok_or_else(error::closed)?;
                logger
                    .try_log_with_outcome_typed(event)
                    .map(conversion::admission)
                    .map_err(conversion::core_admission)
            }
            Backend::Bridge(control) => {
                // Conversion-only envelope is discarded; the bridge supplies its
                // installed service/identity/time when constructing BridgeEvent.
                let service = native::ServiceName::new("binding-validation")
                    .map_err(|_| error::internal("invalid internal service label"))?;
                let stamp = dto::EventStamp {
                    service,
                    timestamp: native::Timestamp::now_utc(),
                    identity: native::ProcessIdentity::default(),
                };
                let event = conversion::bridge_event(conversion::event(event, stamp, origin)?);
                control
                    .try_log(event)
                    .map(conversion::admission)
                    .map_err(conversion::bridge_admission)
            }
        })
    }
    pub(crate) fn health(&self) -> Result<LogHealthDto, Failure> {
        let Ok(_admission) = self.enter() else {
            return Ok((**self.snapshot.load()).clone());
        };
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match &self.backend {
                Backend::Core { logger, .. } => {
                    let logger = logger.load_full().ok_or_else(error::closed)?;
                    dto::from_core_health(logger.health(), logger.level_state())
                }
                Backend::Bridge(control) => {
                    conversion::bridge_health(control.health().map_err(conversion::bridge_control)?)
                }
            }))
            .unwrap_or_else(|_| Err(error::internal("native health panicked")));
        if let Ok(health) = &result {
            self.snapshot.store(Arc::new(health.clone()));
        }
        result
    }
    pub(crate) fn query(
        &self,
        query: dto::LogQueryDto,
    ) -> Result<Operation<LogSnapshotDto>, Failure> {
        let _admission = self.enter()?;
        let query = dto::to_core_query(query)?;
        let mut queue = lock(&self.queue);
        if queue.query {
            return Err(error::full(
                dto::error_codes::SC_OBSERVABILITY_BINDING_QUERY_IN_PROGRESS,
            ));
        }
        queue.query = true;
        let operation = Operation::new(&self.dispatcher, &self.timer);
        queue
            .items
            .push_back(Work::Query(Box::new(query), operation.clone()));
        self.changed.notify_all();
        Ok(operation)
    }
    pub(crate) fn flush(&self, timeout: Duration) -> Result<Operation<CompletionDto>, Failure> {
        let _admission = self.enter()?;
        error::duration(timeout)?;
        let mut queue = lock(&self.queue);
        if queue.flush {
            return Err(error::full(
                dto::error_codes::SC_OBSERVABILITY_BINDING_FLUSH_IN_PROGRESS,
            ));
        }
        queue.flush = true;
        let operation = Operation::new(&self.dispatcher, &self.timer);
        queue
            .items
            .push_back(Work::Flush(timeout, operation.clone()));
        self.changed.notify_all();
        Ok(operation)
    }
    fn run_operations(&self) {
        loop {
            let work = {
                let mut queue = lock(&self.queue);
                loop {
                    if let Some(work) = queue.items.pop_front() {
                        break Some(work);
                    }
                    if self.closed.load(Ordering::SeqCst) && self.active.load(Ordering::SeqCst) == 0
                    {
                        break None;
                    }
                    queue = self
                        .changed
                        .wait(queue)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
            };
            let Some(work) = work else {
                break;
            };
            match work {
                Work::Query(query, operation) => {
                    #[cfg(test)]
                    {
                        let gate = lock(&self.hooks.query).clone();
                        if let Some(gate) = gate {
                            gate.arrive();
                        }
                    }
                    let result = self.execute(|| match &self.backend {
                        Backend::Core { logger, .. } => {
                            let logger = logger.load_full().ok_or_else(error::closed)?;
                            dto::from_core_snapshot(
                                logger
                                    .query(&query)
                                    .map_err(|error| conversion::query(&error))?,
                            )
                        }
                        Backend::Bridge(control) => dto::from_core_snapshot(
                            control.query(&query).map_err(conversion::bridge_control)?,
                        ),
                    });
                    operation.complete(result, || {
                        let mut queue = lock(&self.queue);
                        queue.query = false;
                        self.changed.notify_all();
                    });
                }
                Work::Flush(timeout, operation) => {
                    #[cfg(test)]
                    {
                        let gate = lock(&self.hooks.flush).clone();
                        if let Some(gate) = gate {
                            gate.arrive();
                        }
                    }
                    let result = self.execute(|| {
                        match &self.backend {
                            Backend::Core { logger, .. } => logger
                                .load_full()
                                .ok_or_else(error::closed)?
                                .flush_typed()
                                .map_err(|error| conversion::core_flush(&error))?,
                            Backend::Bridge(control) => {
                                control.flush(timeout).map_err(conversion::bridge_flush)?;
                            }
                        }
                        Ok(CompletionDto::Completed)
                    });
                    operation.complete(result, || {
                        let mut queue = lock(&self.queue);
                        queue.flush = false;
                        self.changed.notify_all();
                    });
                }
            }
        }
        let _queue = lock(&self.queue);
        self.operation_exited.store(true, Ordering::SeqCst);
        self.changed.notify_all();
    }
    fn execute<T>(&self, call: impl FnOnce() -> Result<T, Failure>) -> Result<T, Failure> {
        if self.failed.load(Ordering::SeqCst) {
            return Err(error::internal("native helper failed"));
        }
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            #[cfg(test)]
            assert!(
                !self.hooks.crash.swap(false, Ordering::SeqCst),
                "injected helper failure"
            );
            call()
        }))
        .unwrap_or_else(|_| {
            self.failed.store(true, Ordering::SeqCst);
            self.close();
            Err(error::internal("native helper panicked"))
        })
    }
    fn run_lifecycle(&self) {
        {
            let mut queue = lock(&self.queue);
            while !self.closed.load(Ordering::SeqCst)
                || self.active.load(Ordering::SeqCst) != 0
                || queue.query
                || queue.flush
                || !self.operation_exited.load(Ordering::SeqCst)
            {
                queue = self
                    .changed
                    .wait(queue)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        }
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match &self.backend {
                Backend::Core { logger, .. } => {
                    let logger = logger
                        .swap(None)
                        .ok_or_else(|| error::internal("core owner already consumed"))?;
                    let logger = Arc::try_unwrap(logger).map_err(|_| {
                        error::internal("active native reference survived admission drain")
                    })?;
                    let stopped = logger.shutdown();
                    dto::from_core_health(stopped.health(), stopped.level_state())
                }
                Backend::Bridge(control) => {
                    conversion::bridge_health(control.health().map_err(conversion::bridge_control)?)
                }
            }))
            .unwrap_or_else(|_| Err(error::internal("native shutdown panicked")));
        let result = if self.failed.load(Ordering::SeqCst) {
            Err(error::internal(
                "helper failure prevents confirmed shutdown",
            ))
        } else {
            result
        };
        if let Ok(health) = &result {
            self.snapshot.store(Arc::new(health.clone()));
        }
        self.shutdown.complete(result, || {});
        self.dispatcher.close();
    }
    pub(crate) fn level(
        &self,
        change: impl FnOnce(&mut LevelOwner) -> Result<native::LevelChange, native::LevelChangeError>,
    ) -> Result<dto::LevelChangeDto, Failure> {
        let _admission = self.enter()?;
        let Backend::Core { level, .. } = &self.backend else {
            return Err(error::internal("bridge control has no owner authority"));
        };
        let mut owner = match level.try_lock() {
            Ok(owner) => owner,
            Err(std::sync::TryLockError::WouldBlock) => {
                return Err(error::full(
                    dto::error_codes::SC_OBSERVABILITY_BINDING_DISPATCH_FULL,
                ));
            }
            Err(_) => return Err(error::internal("level owner state poisoned")),
        };
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dto::from_level_change(change(&mut owner).map_err(dto::from_level_error)?)
        }))
        .unwrap_or_else(|_| Err(error::internal("level owner callback panicked")))
    }
}

pub(crate) fn core(
    mut config: sc_observability::LoggerConfig,
) -> Result<Arc<Coordinator>, Failure> {
    Coordinator::create(|| {
        let stamp = dto::EventStamp {
            service: config.service_name.clone(),
            timestamp: native::Timestamp::now_utc(),
            identity: match &config.process_identity {
                native::ProcessIdentityPolicy::Auto => native::ProcessIdentity::default(),
                native::ProcessIdentityPolicy::Fixed { hostname, pid } => native::ProcessIdentity {
                    hostname: hostname.clone(),
                    pid: *pid,
                },
                native::ProcessIdentityPolicy::Resolver(resolver) => {
                    resolver.resolve().map_err(|e| {
                        conversion::context(e.diagnostic(), conversion::Kind::Unavailable)
                    })?
                }
            },
        };
        // Resolve once: native diagnostic events and producer events share the
        // exact host-selected identity even when a resolver is stateful.
        config.process_identity = native::ProcessIdentityPolicy::Fixed {
            hostname: stamp.identity.hostname.clone(),
            pid: stamp.identity.pid,
        };
        let (logger, level) = Logger::new_with_level_owner_typed(config)
            .map_err(|e| conversion::context(e.diagnostic(), conversion::Kind::Unavailable))?;
        let health = dto::from_core_health(logger.health(), logger.level_state())?;
        Ok((
            Backend::Core {
                logger: ArcSwapOption::from(Some(Arc::new(logger))),
                level: Mutex::new(level),
                stamp,
            },
            health,
        ))
    })
}
pub(crate) fn bridge(
    control: sc_observability_log::LogControl,
) -> Result<Arc<Coordinator>, Failure> {
    Coordinator::create(|| {
        let health =
            conversion::bridge_health(control.health().map_err(conversion::bridge_control)?)?;
        Ok((Backend::Bridge(control), health))
    })
}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct TestHooks {
    pub(crate) admission: Mutex<Option<Arc<crate::tests::Gate>>>,
    pub(crate) query: Mutex<Option<Arc<crate::tests::Gate>>>,
    pub(crate) flush: Mutex<Option<Arc<crate::tests::Gate>>>,
    pub(crate) crash: AtomicBool,
}

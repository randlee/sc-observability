//! Every resource/lifecycle case runs in its own process: never reset TIMER.
use super::*;
use crate::{
    coordinator::{Backend, Coordinator},
    sync::lock,
};
use sc_observability_dto as dto;
use sc_observability_types as native;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Barrier, Condvar, Mutex, mpsc};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Instant;

pub(crate) struct Gate {
    state: Mutex<(usize, bool)>,
    changed: Condvar,
}
impl Gate {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new((0, false)),
            changed: Condvar::new(),
        })
    }
    pub(crate) fn arrive(&self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut state = lock(&self.state);
        state.0 += 1;
        self.changed.notify_all();
        while !state.1 {
            let (next, timeout) = self
                .changed
                .wait_timeout(state, deadline.saturating_duration_since(Instant::now()))
                .unwrap();
            state = next;
            assert!(
                !timeout.timed_out() || state.1,
                "gate was not explicitly released before the absolute deadline"
            );
        }
    }
    fn entered(&self, count: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut state = lock(&self.state);
        while state.0 < count {
            let (next, timeout) = self
                .changed
                .wait_timeout(state, deadline.saturating_duration_since(Instant::now()))
                .unwrap();
            state = next;
            assert!(
                !timeout.timed_out() || state.0 >= count,
                "gate arrivals {} < {count}",
                state.0
            );
        }
    }
    fn release(&self) {
        lock(&self.state).1 = true;
        self.changed.notify_all();
    }
}
struct Release(Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
    }
}
fn config() -> (tempfile::TempDir, sc_observability::LoggerConfig) {
    let root = tempfile::tempdir().unwrap();
    let mut config = sc_observability::LoggerConfig::default_for(
        native::ServiceName::new("native-contract").unwrap(),
        root.path().into(),
    );
    config.enable_console_sink = false;
    config.queue_capacity = 4096;
    config.process_identity = native::ProcessIdentityPolicy::Fixed {
        hostname: Some("host".into()),
        pid: Some(123),
    };
    (root, config)
}
fn event() -> dto::LogEventDto {
    dto::decode_event(serde_json::json!({"schema_version":1,"level":"info","target":"binding.contract","action":"contract.emit"})).unwrap()
}
fn query() -> dto::LogQueryDto {
    dto::decode_query(serde_json::json!({"schema_version":1})).unwrap()
}
fn core() -> (tempfile::TempDir, CoreLoggerOwner, CoreLoggerBackend) {
    let (root, config) = config();
    let (owner, backend) = create_core_backend(config).unwrap();
    (root, owner, backend)
}
fn stop(owner: &CoreLoggerOwner) {
    owner.shutdown(Duration::from_secs(5)).unwrap();
    crate::spawn::wait_live(1);
}
fn code<T>(result: Result<T, Failure>, expected: &str) {
    match result {
        Err(error) => assert_eq!(error.diagnostic().code, expected),
        Ok(_) => panic!("expected {expected}"),
    }
}

const CASES: &[&str] = &[
    "timer_poison",
    "cross_logger_cancellation",
    "sync_and_async_waiters",
    "timer_spawn_retry",
    "worker1_rollback",
    "worker2_rollback",
    "worker3_rollback",
    "core_start_rollback",
    "concurrent_timer",
    "observer_bounds",
    "callback_bounds",
    "callback_panic",
    "slot_ordering",
    "bridge_slot_ordering",
    "callback_race",
    "level_gate_busy",
    "core_sink_and_shutdown",
    "admission32",
    "admission32_close",
    "failed_helper",
    "bridge_native_timeout",
    "bridge_external_overlap",
    "bridge_churn",
    "last_handle_teardown",
    "bridge_observers_callbacks",
    "native_diagnostic_fidelity",
];

#[test]
fn contract_matrix() {
    if let Ok(case) = std::env::var("SC_BINDING_RUNTIME_CASE") {
        match case.as_str() {
            "timer_poison" => {
                crate::timer::poison_initialization();
                let (_root, config) = config();
                code(
                    create_core_backend(config),
                    dto::error_codes::SC_OBSERVABILITY_BINDING_INTERNAL,
                );
                crate::spawn::wait_live(0);
            }
            "cross_logger_cancellation" => cross_logger_cancellation(),
            "sync_and_async_waiters" => sync_and_async_waiters(),
            "timer_spawn_retry" => spawn_rollback(0),
            "worker1_rollback" => spawn_rollback(1),
            "worker2_rollback" => spawn_rollback(2),
            "worker3_rollback" => spawn_rollback(3),
            "core_start_rollback" => core_start_rollback(),
            "concurrent_timer" => concurrent_timer(),
            "observer_bounds" => observer_bounds(),
            "callback_bounds" => callback_bounds(),
            "callback_panic" => callback_panic(),
            "slot_ordering" => slot_ordering(),
            "bridge_slot_ordering" => bridge_slot_ordering(),
            "callback_race" => callback_race(),
            "level_gate_busy" => level_gate_busy(),
            "core_sink_and_shutdown" => core_sink_and_shutdown(),
            "admission32" => admission32(false),
            "admission32_close" => admission32(true),
            "failed_helper" => failed_helper(),
            "bridge_native_timeout" => bridge_timeout(false),
            "bridge_external_overlap" => bridge_timeout(true),
            "bridge_churn" => bridge_churn(),
            "last_handle_teardown" => last_handle_teardown(),
            "bridge_observers_callbacks" => bridge_observers_callbacks(),
            "native_diagnostic_fidelity" => native_diagnostic_fidelity(),
            _ => panic!("unknown contract case {case}"),
        }
        println!("BINDING_CASE_PASS {case}");
        return;
    }
    for case in CASES {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "tests::contract_matrix", "--nocapture"])
            .env("SC_BINDING_RUNTIME_CASE", case)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "case {case}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(&format!("BINDING_CASE_PASS {case}"))
        );
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }
}
fn spawn_rollback(index: usize) {
    crate::spawn::fail_at(index);
    let (root, config) = config();
    code(
        create_core_backend(config),
        dto::error_codes::SC_OBSERVABILITY_BINDING_COORDINATOR_START_FAILED,
    );
    assert_eq!(
        std::fs::read_dir(root.path()).unwrap().count(),
        0,
        "core construction must follow helper reservation"
    );
    crate::spawn::wait_live(usize::from(index != 0));
    crate::spawn::fail_at(usize::MAX);
    let (_root, owner, _backend) = core();
    stop(&owner);
}
fn core_start_rollback() {
    let (_root, mut config) = config();
    config.queue_capacity = 0;
    assert!(create_core_backend(config).is_err());
    crate::spawn::wait_live(1);
    let (_root, owner, _backend) = core();
    stop(&owner);
}
fn concurrent_timer() {
    let barrier = Arc::new(Barrier::new(9));
    let (tx, rx) = mpsc::channel();
    let mut workers = Vec::new();
    for _ in 0..8 {
        let barrier = barrier.clone();
        let tx = tx.clone();
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            tx.send(core()).unwrap();
        }));
    }
    barrier.wait();
    let mut owners = Vec::new();
    for _ in 0..8 {
        owners.push(rx.recv_timeout(Duration::from_secs(5)).unwrap());
    }
    for worker in workers {
        worker.join().unwrap();
    }
    crate::spawn::wait_live(25);
    for (_, owner, _) in &owners {
        owner.start_shutdown().unwrap();
    }
    for (_, owner, _) in &owners {
        owner.wait_stopped(Duration::from_secs(5)).unwrap();
    }
    crate::spawn::wait_live(1);
    assert_eq!(crate::timer::shared().unwrap().entries(), 0);
}
struct Notify {
    state: Mutex<bool>,
    changed: Condvar,
}
impl Wake for Notify {
    fn wake(self: Arc<Self>) {
        *lock(&self.state) = true;
        self.changed.notify_all();
    }
}
fn ready<T>(future: impl Future<Output = T>) -> T {
    let wake = Arc::new(Notify {
        state: Mutex::new(false),
        changed: Condvar::new(),
    });
    let waker = Waker::from(wake.clone());
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(result) = future.as_mut().poll(&mut context) {
            return result;
        }
        let mut notified = lock(&wake.state);
        while !*notified {
            let (next, timeout) = wake
                .changed
                .wait_timeout(notified, Duration::from_secs(5))
                .unwrap();
            notified = next;
            assert!(!timeout.timed_out(), "future not woken");
        }
        *notified = false;
    }
}
fn pending<T: Clone + Send + Sync + 'static>(backend: &CoreLoggerBackend) -> Operation<T> {
    Operation::new(&backend.shared.dispatcher, &crate::timer::shared().unwrap())
}
fn observer_bounds() {
    let (_root, owner, backend) = core();
    let operation: Operation<u32> = pending(&backend);
    let mut futures: Vec<_> = (0..64)
        .map(|_| operation.completion(Duration::from_secs(60)))
        .collect();
    assert_eq!(operation.observer_count(), 64);
    assert_eq!(crate::timer::shared().unwrap().entries(), 64);
    code(
        operation.wait(Duration::from_millis(1)),
        dto::error_codes::SC_OBSERVABILITY_BINDING_WAITERS_FULL,
    );
    code(
        operation.subscribe(Box::new(|_| panic!("overflow callback retained"))),
        dto::error_codes::SC_OBSERVABILITY_BINDING_WAITERS_FULL,
    );
    futures.pop();
    let callback = operation.subscribe(Box::new(|_| {})).unwrap();
    assert_eq!(operation.observer_count(), 64);
    drop(callback);
    drop(futures);
    assert_eq!(operation.observer_count(), 0);
    assert_eq!(crate::timer::shared().unwrap().entries(), 0);
    code(
        operation.wait(Duration::ZERO),
        dto::error_codes::SC_OBSERVABILITY_BINDING_TIMEOUT,
    );
    code(
        ready(operation.completion(Duration::from_nanos(1))),
        dto::error_codes::SC_OBSERVABILITY_BINDING_INVALID_INPUT,
    );
    code(
        operation.wait(Duration::from_millis(60001)),
        dto::error_codes::SC_OBSERVABILITY_BINDING_INVALID_INPUT,
    );
    let other: Operation<u32> = pending(&backend);
    let first = operation.completion(Duration::from_secs(60));
    let second = other.completion(Duration::from_secs(60));
    drop(first);
    assert_eq!(crate::timer::shared().unwrap().entries(), 1);
    other.complete(Ok(8), || {});
    assert_eq!(ready(second).unwrap(), 8);
    assert_eq!(crate::timer::shared().unwrap().entries(), 0);
    let timed = operation.completion(Duration::from_millis(1));
    code(
        ready(timed),
        dto::error_codes::SC_OBSERVABILITY_BINDING_TIMEOUT,
    );
    operation.complete(Ok(7), || {});
    assert_eq!(operation.wait(Duration::ZERO).unwrap(), 7);
    assert_eq!(ready(operation.completion(Duration::ZERO)).unwrap(), 7);
    assert_eq!(operation.observer_count(), 0);
    stop(&owner);
    assert_eq!(operation.wait(Duration::ZERO).unwrap(), 7);
}
fn callback_bounds() {
    let (_root, owner, backend) = core();
    let gate = Gate::new();
    let _release = Release(gate.clone());
    let one: Operation<u32> = pending(&backend);
    let two: Operation<u32> = pending(&backend);
    let three: Operation<u32> = pending(&backend);
    let gate_in = gate.clone();
    let mut subscriptions = vec![one.subscribe(Box::new(move |_| gate_in.arrive())).unwrap()];
    for _ in 1..64 {
        subscriptions.push(one.subscribe(Box::new(|_| {})).unwrap());
    }
    for _ in 0..64 {
        subscriptions.push(two.subscribe(Box::new(|_| {})).unwrap());
    }
    assert_eq!(backend.shared.dispatcher.reserved(), 128);
    code(
        three.subscribe(Box::new(|_| panic!("overflow retained"))),
        dto::error_codes::SC_OBSERVABILITY_BINDING_WAITERS_FULL,
    );
    one.complete(Ok(1), || {});
    two.complete(Ok(2), || {});
    gate.entered(1);
    // Completion worker is blocked; the operation worker and lifecycle progress.
    backend
        .start_query(query())
        .unwrap()
        .wait(Duration::from_secs(2))
        .unwrap();
    owner.shutdown(Duration::from_secs(2)).unwrap();
    drop(subscriptions);
    assert_eq!(
        backend.shared.dispatcher.reserved(),
        1,
        "claimed callback owns its reservation"
    );
    assert_eq!(one.wait(Duration::ZERO).unwrap(), 1);
    gate.release();
    crate::spawn::wait_live(1);
}
fn callback_panic() {
    let (_root, owner, backend) = core();
    let operation: Operation<u32> = pending(&backend);
    let (tx, rx) = mpsc::channel();
    let cancelled = operation
        .subscribe(Box::new(|_| panic!("cancelled callback ran")))
        .unwrap();
    drop(cancelled);
    let panic_callback = operation
        .subscribe(Box::new(|_| panic!("contained callback panic")))
        .unwrap();
    let completed = operation
        .subscribe(Box::new(move |result| tx.send(result).unwrap()))
        .unwrap();
    operation.complete(Ok(4), || {});
    assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap().unwrap(), 4);
    assert_eq!(backend.shared.dispatcher.panics.load(Ordering::SeqCst), 1);
    assert_eq!(operation.wait(Duration::ZERO).unwrap(), 4);
    drop((panic_callback, completed));
    stop(&owner);
}
fn slot_ordering() {
    let (_root, owner, backend) = core();
    slots(&backend, &backend.shared);
    stop(&owner);
}
fn slots(backend: &impl HostLoggingBackend, shared: &Arc<Coordinator>) {
    let query_gate = Gate::new();
    let flush_gate = Gate::new();
    let _release_query = Release(query_gate.clone());
    let _release_flush = Release(flush_gate.clone());
    *lock(&shared.hooks.query) = Some(query_gate.clone());
    *lock(&shared.hooks.flush) = Some(flush_gate.clone());
    let first = backend.start_query(query()).unwrap();
    query_gate.entered(1);
    for invalid in [
        Duration::from_nanos(1),
        Duration::from_millis(60001),
        Duration::MAX,
    ] {
        code(
            backend.start_flush(invalid),
            dto::error_codes::SC_OBSERVABILITY_BINDING_INVALID_INPUT,
        );
    }
    let flush = backend.start_flush(Duration::from_secs(60)).unwrap();
    code(
        backend.start_query(query()),
        dto::error_codes::SC_OBSERVABILITY_BINDING_QUERY_IN_PROGRESS,
    );
    code(
        backend.start_flush(Duration::from_millis(1)),
        dto::error_codes::SC_OBSERVABILITY_BINDING_FLUSH_IN_PROGRESS,
    );
    code(
        first.wait(Duration::from_millis(1)),
        dto::error_codes::SC_OBSERVABILITY_BINDING_TIMEOUT,
    );
    assert_eq!(lock(&flush_gate.state).0, 0, "cross-class FIFO");
    query_gate.release();
    first.wait(Duration::from_secs(2)).unwrap();
    flush_gate.entered(1);
    code(
        flush.wait(Duration::from_millis(1)),
        dto::error_codes::SC_OBSERVABILITY_BINDING_TIMEOUT,
    );
    let later = backend.start_query(query()).unwrap();
    assert_eq!(lock(&query_gate.state).0, 1);
    drop(flush.completion(Duration::from_secs(60)));
    flush_gate.release();
    flush.wait(Duration::from_secs(2)).unwrap();
    later.wait(Duration::from_secs(2)).unwrap();
    // A fresh zero-duration request is valid and reaches native execution.
    let zero = backend.start_flush(Duration::ZERO).unwrap();
    match zero.wait(Duration::from_secs(2)) {
        Ok(_) | Err(Failure::Timeout { .. }) => {}
        Err(other) => panic!("unexpected native zero-timeout result {other:?}"),
    }
}
struct HeldSink {
    gate: Arc<Gate>,
    armed: std::sync::atomic::AtomicBool,
    flushes: AtomicUsize,
}
#[allow(
    deprecated,
    reason = "test sink preserves the public legacy LogSink trait"
)]
impl sc_observability::LogSink for HeldSink {
    fn write(&self, _: &native::LogEvent) -> Result<(), native::v2::LogSinkError> {
        Ok(())
    }
    fn flush(&self) -> Result<(), native::v2::LogSinkError> {
        self.flushes.fetch_add(1, Ordering::SeqCst);
        if self.armed.swap(false, Ordering::SeqCst) {
            self.gate.arrive();
        }
        Ok(())
    }
    fn health(&self) -> native::SinkHealth {
        native::SinkHealth {
            name: native::SinkName::new("held").unwrap(),
            state: native::SinkHealthState::Healthy,
            last_error: None,
        }
    }
}
fn core_sink_and_shutdown() {
    let (_root, config) = config();
    let gate = Gate::new();
    let _release = Release(gate.clone());
    let sink = Arc::new(HeldSink {
        gate: gate.clone(),
        armed: std::sync::atomic::AtomicBool::new(true),
        flushes: AtomicUsize::new(0),
    });
    let stamp = dto::EventStamp {
        service: config.service_name.clone(),
        timestamp: native::Timestamp::now_utc(),
        identity: native::ProcessIdentity::default(),
    };
    let shared = Coordinator::create(|| {
        let mut builder = sc_observability::Logger::builder_typed(config).unwrap();
        builder.register_sink(sc_observability::SinkRegistration::new(sink.clone()));
        let (logger, level) = builder.build_with_level_owner_typed().unwrap();
        let health = dto::from_core_health(logger.health(), logger.level_state()).unwrap();
        Ok((
            Backend::Core {
                logger: arc_swap::ArcSwapOption::from(Some(Arc::new(logger))),
                level: Mutex::new(level),
                stamp,
            },
            health,
        ))
    })
    .unwrap();
    let mut owner = CoreLoggerOwner {
        shared: shared.clone(),
    };
    let backend = CoreLoggerBackend { shared };
    let flush = backend.start_flush(Duration::ZERO).unwrap();
    gate.entered(1);
    assert!(matches!(
        backend.try_log(event(), ProducerOrigin::RustHost),
        Ok(AdmissionDto::Accepted)
    ));
    owner
        .elevate_level(LevelFilter::Debug, LevelChangeSource::Application)
        .unwrap();
    code(
        flush.wait(Duration::ZERO),
        dto::error_codes::SC_OBSERVABILITY_BINDING_TIMEOUT,
    );
    let shutdown = owner.start_shutdown().unwrap();
    code(
        shutdown.wait(Duration::from_millis(1)),
        dto::error_codes::SC_OBSERVABILITY_BINDING_TIMEOUT,
    );
    assert!(backend.health().is_ok());
    drop(owner);
    assert!(matches!(
        backend.try_log(event(), ProducerOrigin::RustHost),
        Err(Failure::Closed { .. })
    ));
    gate.release();
    flush.wait(Duration::from_secs(2)).unwrap();
    shutdown.wait(Duration::from_secs(2)).unwrap();
    crate::spawn::wait_live(1);
    assert!(sink.flushes.load(Ordering::SeqCst) >= 1);
}
fn admission32(close: bool) {
    let (_root, owner, backend) = core();
    let gate = Gate::new();
    let _release = Release(gate.clone());
    *lock(&backend.shared.hooks.admission) = Some(gate.clone());
    let (tx, rx) = mpsc::channel();
    let mut workers = Vec::new();
    for _ in 0..32 {
        let backend = backend.clone();
        let tx = tx.clone();
        workers.push(std::thread::spawn(move || {
            let _ = tx.send(backend.try_log(event(), ProducerOrigin::RustHost));
        }));
    }
    gate.entered(32);
    if close {
        owner.start_shutdown().unwrap();
    }
    gate.release();
    for _ in 0..32 {
        let result = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        if close {
            assert!(matches!(result, Err(Failure::Closed { .. })));
        } else {
            assert!(matches!(
                result,
                Ok(AdmissionDto::Accepted | AdmissionDto::Filtered)
            ));
        }
    }
    for worker in workers {
        worker.join().unwrap();
    }
    stop(&owner);
}
fn failed_helper() {
    let (_root, owner, backend) = core();
    backend.shared.hooks.crash.store(true, Ordering::SeqCst);
    let op = backend.start_query(query()).unwrap();
    assert!(matches!(
        op.wait(Duration::from_secs(2)),
        Err(Failure::Internal { .. })
    ));
    assert!(matches!(
        owner.shutdown(Duration::from_secs(2)),
        Err(Failure::Internal { .. })
    ));
    crate::spawn::wait_live(1);
}
fn bridge_host() -> (tempfile::TempDir, sc_observability_log::LogGuard) {
    let (root, mut config) = config();
    config.enable_console_sink = true;
    let host = sc_observability_log::init(
        config,
        sc_observability_log::BridgeOptions {
            default_action: native::ActionName::new("default").unwrap(),
            parse_bracket_action: false,
        },
    )
    .unwrap();
    (root, host)
}
fn bridge_timeout(external: bool) {
    let (_root, host) = bridge_host();
    let control = host.control();
    let backend = bridge_backend(control.clone()).unwrap();
    let stdout = std::io::stdout();
    let held = stdout.lock();
    backend.try_log(event(), ProducerOrigin::RustHost).unwrap();
    if external {
        assert!(matches!(
            control.flush(Duration::from_millis(1)),
            Err(sc_observability_log::FlushError::TimedOut { .. })
        ));
    } else {
        let timed = backend.start_flush(Duration::from_millis(1)).unwrap();
        code(
            timed.wait(Duration::from_secs(2)),
            sc_observability_log::error_codes::SC_OBSERVABILITY_LOG_FLUSH_TIMED_OUT.as_str(),
        );
    }
    let overlap = backend.start_flush(Duration::from_secs(1)).unwrap();
    code(
        overlap.wait(Duration::from_secs(2)),
        sc_observability_log::error_codes::SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS.as_str(),
    );
    drop(held);
    // These are explicit new host requests; the adapter itself never retries or
    // retrieves the previous native result. Retry only the documented overlap.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let result = backend
            .start_flush(Duration::from_secs(1))
            .unwrap()
            .wait(Duration::from_secs(2));
        match result{Ok(_)=>break,Err(error)if error.diagnostic().code==sc_observability_log::error_codes::SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS.as_str()=>{assert!(Instant::now()<deadline);std::thread::yield_now();},other=>panic!("new bridge barrier: {other:?}")}
    }
    drop(backend);
    crate::spawn::wait_live(1);
    host.shutdown(Duration::from_secs(2)).unwrap();
}
fn bridge_churn() {
    let (_root, host) = bridge_host();
    let control = host.control();
    for _ in 0..12 {
        let backend = bridge_backend(control.clone()).unwrap();
        let saved = backend.start_query(query()).unwrap();
        saved.wait(Duration::from_secs(2)).unwrap();
        drop(backend);
        crate::spawn::wait_live(1);
        assert!(matches!(saved.state(), OperationState::Completed { .. }));
        assert!(control.health().is_ok());
    }
    host.shutdown(Duration::from_secs(2)).unwrap();
}
fn native_diagnostic_fidelity() {
    let diagnostic = native::OperationDiagnostic {
        code: native::ErrorCode::new_static("SC_NATIVE_FIXTURE"),
        message: "exact native message".into(),
        remediation: native::Remediation::recoverable("first", ["second"]),
        at: native::Timestamp::UNIX_EPOCH,
    };
    let failure = crate::conversion::bridge_flush(sc_observability_log::FlushError::Logger {
        diagnostic: diagnostic.clone(),
    });
    assert!(matches!(failure, Failure::Io { .. }));
    assert_eq!(failure.diagnostic(), &dto::Diagnostic::from(diagnostic));
    let golden: serde_json::Value =
        serde_json::from_str(include_str!("../tests/native-diagnostic.json")).unwrap();
    assert_eq!(serde_json::to_value(failure).unwrap(), golden);
}

fn cross_logger_cancellation() {
    let (_root_a, owner_a, backend_a) = core();
    let (_root_b, owner_b, backend_b) = core();
    let a: Operation<u32> = pending(&backend_a);
    let b: Operation<u32> = pending(&backend_b);
    let first = a.completion(Duration::from_secs(60));
    let second = b.completion(Duration::from_secs(60));
    assert_eq!(crate::timer::shared().unwrap().entries(), 2);
    drop(first);
    assert_eq!(crate::timer::shared().unwrap().entries(), 1);
    assert_eq!(b.observer_count(), 1);
    b.complete(Ok(23), || {});
    assert_eq!(ready(second).unwrap(), 23);
    assert_eq!(crate::timer::shared().unwrap().entries(), 0);
    owner_a.shutdown(Duration::from_secs(5)).unwrap();
    stop(&owner_b);
}
fn sync_and_async_waiters() {
    let (_root, owner, backend) = core();
    let operation: Operation<u32> = pending(&backend);
    let sync = operation.clone();
    let waiter = std::thread::spawn(move || sync.wait(Duration::from_secs(5)));
    let deadline = Instant::now() + Duration::from_secs(5);
    while operation.observer_count() != 1 {
        assert!(Instant::now() < deadline, "sync waiter did not register");
        std::thread::yield_now();
    }
    let futures: Vec<_> = (0..63)
        .map(|_| operation.completion(Duration::from_secs(60)))
        .collect();
    assert_eq!(operation.observer_count(), 64);
    code(
        operation.subscribe(Box::new(|_| {})),
        dto::error_codes::SC_OBSERVABILITY_BINDING_WAITERS_FULL,
    );
    operation.complete(Ok(41), || {});
    assert_eq!(waiter.join().unwrap().unwrap(), 41);
    for future in futures {
        assert_eq!(ready(future).unwrap(), 41);
    }
    for _ in 0..128 {
        assert_eq!(operation.wait(Duration::ZERO).unwrap(), 41);
    }
    assert_eq!(operation.observer_count(), 0);
    assert_eq!(crate::timer::shared().unwrap().entries(), 0);
    crate::spawn::wait_live(4);
    stop(&owner);
}

fn bridge_slot_ordering() {
    let (_root, config) = config();
    let host = sc_observability_log::init(
        config,
        sc_observability_log::BridgeOptions {
            default_action: native::ActionName::new("bridge.test").unwrap(),
            parse_bracket_action: false,
        },
    )
    .unwrap();
    let backend = bridge_backend(host.control()).unwrap();
    slots(&backend, &backend.shared);
    drop(backend);
    crate::spawn::wait_live(1);
    host.shutdown(Duration::from_secs(5)).unwrap();
}
fn callback_race() {
    let (_root, owner, backend) = core();
    for _ in 0..64 {
        let operation: Operation<u32> = pending(&backend);
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let subscription = operation
            .subscribe(Box::new(move |_| {
                count.fetch_add(1, Ordering::SeqCst);
            }))
            .unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let race = barrier.clone();
        let complete = operation.clone();
        let worker = std::thread::spawn(move || {
            race.wait();
            complete.complete(Ok(1), || {});
        });
        barrier.wait();
        drop(subscription);
        worker.join().unwrap();
        assert_eq!(operation.wait(Duration::ZERO).unwrap(), 1);
        assert!(calls.load(Ordering::SeqCst) <= 1);
    }
    stop(&owner);
    assert_eq!(backend.shared.dispatcher.reserved(), 0);
}
fn level_gate_busy() {
    let (_root, mut owner, backend) = core();
    let Backend::Core { level, .. } = &backend.shared.backend else {
        unreachable!()
    };
    let before = backend.health().unwrap().level_state;
    let guard = lock(level);
    code(
        owner.elevate_level(
            native::LevelFilter::Debug,
            native::LevelChangeSource::Application,
        ),
        dto::error_codes::SC_OBSERVABILITY_BINDING_DISPATCH_FULL,
    );
    assert_eq!(backend.health().unwrap().level_state, before);
    drop(guard);
    owner
        .elevate_level(
            native::LevelFilter::Debug,
            native::LevelChangeSource::Application,
        )
        .unwrap();
    stop(&owner);
}

fn last_handle_teardown() {
    for running in [false, true] {
        let (_root, owner, backend) = core();
        let weak = Arc::downgrade(&backend.shared);
        let gate = Gate::new();
        let _release = Release(gate.clone());
        if running {
            *lock(&backend.shared.hooks.query) = Some(gate.clone());
            let operation = backend.start_query(query()).unwrap();
            gate.entered(1);
            let observation = operation.completion(Duration::from_secs(60));
            drop((observation, operation));
        }
        drop((owner, backend));
        if running {
            crate::spawn::wait_live(4);
            gate.release();
        }
        crate::spawn::wait_live(1);
        assert!(
            weak.upgrade().is_none(),
            "idle/running teardown retained coordinator"
        );
    }
    let (_root, owner, backend) = core();
    let saved = backend.start_query(query()).unwrap();
    saved.wait(Duration::from_secs(2)).unwrap();
    let weak = Arc::downgrade(&backend.shared);
    drop((owner, backend));
    crate::spawn::wait_live(1);
    assert!(weak.upgrade().is_none());
    assert!(matches!(
        saved.state(),
        OperationState::Completed { result: Ok(_) }
    ));
}
fn bridge_observers_callbacks() {
    let (_root, host) = bridge_host();
    let backend = bridge_backend(host.control()).unwrap();
    let timer = crate::timer::shared().unwrap();
    let operation: Operation<u32> = Operation::new(&backend.shared.dispatcher, &timer);
    let futures: Vec<_> = (0..64)
        .map(|_| operation.completion(Duration::from_secs(60)))
        .collect();
    code(
        operation.wait(Duration::from_millis(1)),
        dto::error_codes::SC_OBSERVABILITY_BINDING_WAITERS_FULL,
    );
    drop(futures);
    let (tx, rx) = mpsc::channel();
    let cancelled = operation
        .subscribe(Box::new(|_| panic!("cancelled callback ran")))
        .unwrap();
    drop(cancelled);
    let panic_callback = operation
        .subscribe(Box::new(|_| panic!("contained bridge callback panic")))
        .unwrap();
    let callback = operation
        .subscribe(Box::new(move |result| tx.send(result).unwrap()))
        .unwrap();
    code(
        operation.wait(Duration::from_millis(1)),
        dto::error_codes::SC_OBSERVABILITY_BINDING_TIMEOUT,
    );
    operation.complete(Ok(19), || {});
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(2)).unwrap().unwrap(),
        19
    );
    assert_eq!(backend.shared.dispatcher.panics.load(Ordering::SeqCst), 1);
    for _ in 0..128 {
        assert_eq!(operation.wait(Duration::ZERO).unwrap(), 19);
    }
    drop((panic_callback, callback));
    assert_eq!(timer.entries(), 0);
    drop(backend);
    crate::spawn::wait_live(1);
    assert_eq!(operation.wait(Duration::ZERO).unwrap(), 19);
    host.shutdown(Duration::from_secs(2)).unwrap();
}

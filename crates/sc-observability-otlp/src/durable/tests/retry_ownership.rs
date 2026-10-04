//! Exercise durable retry ownership across real and scripted exporters, including profiles.
//!
//! Attempt budgets are asserted through the scripted exporter seam, while the
//! real collector test checks only that each signal reaches its intended route.
use super::loopback::ScriptedStatusCollector;
use super::*;
use std::net::TcpListener;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;

fn retry_config(path: &Path) -> TelemetryClientConfig {
    let mut config = config(path);
    let mut retry = SyncHttpRetryPolicyDto::default();
    retry.max_retries = Some(3);
    retry.initial_backoff_ms = Some(1);
    retry.max_backoff_ms = Some(1);
    retry.retry_jitter_percent = Some(0);
    config.sync_http_retry = Some(retry);
    config
}

fn assert_real_http_smoke(collector: &ScriptedStatusCollector, name: &str, path: &str) {
    let paths = collector.paths();
    assert!(!paths.is_empty(), "{name} reached the real collector");
    assert!(
        paths.iter().all(|actual| actual == path),
        "{name} only used the expected OTLP route: {paths:?}"
    );
}

struct RetryableThenOkExporter {
    retryable_before_success: usize,
    calls: AtomicUsize,
}
impl RetryableThenOkExporter {
    fn new(retryable_before_success: usize) -> Self {
        Self {
            retryable_before_success,
            calls: AtomicUsize::new(0),
        }
    }
}
impl SubmissionExporter for RetryableThenOkExporter {
    fn export(&self, _: Signal, _: &[SubmissionEnvelope]) -> Result<(), SubmissionExportFailure> {
        let call = self.calls.fetch_add(1, Ordering::AcqRel);
        if call < self.retryable_before_success {
            return Err(SubmissionExportFailure::Retryable(export_error()));
        }
        Ok(())
    }
}

#[test]
fn scripted_retry_attempt_budgets_are_exact_for_logs_and_profiles() {
    for (name, retryable_before_success, expected_calls) in [
        ("logs", 1, 2),
        ("profiles", 1, 2),
        ("logs", usize::MAX, 4),
        ("profiles", usize::MAX, 4),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let exporter = Arc::new(RetryableThenOkExporter::new(retryable_before_success));
        let client =
            DurableTelemetryClient::open_with_exporter(retry_config(dir.path()), exporter.clone())
                .unwrap();
        let receipt = client.emit(fixture(name)).unwrap();

        let result = client.flush_submission(&receipt.submission_id, DEADLINE);
        if retryable_before_success == usize::MAX {
            assert!(matches!(
                result,
                Err(TelemetryClientError::Delivery(
                    DeliveryError::TerminalFailure { .. }
                ))
            ));
            assert!(matches!(
                client
                    .status(StatusQuery::Submissions(vec![
                        receipt.submission_id.clone()
                    ]))
                    .unwrap()
                    .submissions[0]
                    .signals[0]
                    .1,
                DeliveryState::Failed { attempts: 4, .. }
            ));
            assert!(
                client
                    .flush_submission(&receipt.submission_id, DEADLINE)
                    .is_err(),
                "a terminal scripted retry budget is not spent again"
            );
        } else {
            result.unwrap();
            assert!(matches!(
                client
                    .status(StatusQuery::Submissions(vec![
                        receipt.submission_id.clone()
                    ]))
                    .unwrap()
                    .submissions[0]
                    .signals[0]
                    .1,
                DeliveryState::Delivered { attempts: 2, .. }
            ));
        }
        assert_eq!(
            exporter.calls.load(Ordering::Acquire),
            expected_calls,
            "{name} scripted durable attempt budget"
        );
        client.shutdown(DEADLINE).unwrap();
    }
}

#[test]
fn collector_drop_does_not_depend_on_a_wake_connection() {
    let mut collector = ScriptedStatusCollector::start(vec![], "503 Service Unavailable");
    let closed_listener = TcpListener::bind("127.0.0.1:0").expect("reserve closed wake address");
    collector.set_address_for_drop_regression(
        closed_listener.local_addr().expect("closed wake address"),
    );
    drop(closed_listener);

    let (completed, dropped) = mpsc::channel();
    std::thread::spawn(move || {
        drop(collector);
        completed.send(()).expect("report collector drop");
    });
    dropped
        .recv_timeout(Duration::from_secs(1))
        .expect("collector drops without a wake connection");
}

fn open_real(config: TelemetryClientConfig) -> DurableTelemetryClient {
    conformance::open_manual(|| DurableTelemetryClient::open(config)).unwrap()
}

#[test]
fn real_http_smoke_delivers_logs_and_profiles_without_counting_retries() {
    for name in ["logs", "profiles"] {
        let dir = tempfile::tempdir().unwrap();
        let collector = ScriptedStatusCollector::start(vec!["503 Service Unavailable"], "200 OK");
        let mut config = retry_config(dir.path());
        config.endpoint = collector.endpoint();
        let client = open_real(config);
        let receipt = client.emit(fixture(name)).unwrap();
        client
            .flush_submission(&receipt.submission_id, DEADLINE)
            .unwrap();
        let status = client
            .status(StatusQuery::Submissions(vec![receipt.submission_id]))
            .unwrap();
        assert!(matches!(
            status.submissions[0].signals[0].1,
            DeliveryState::Delivered { attempts: 1, .. }
        ));
        let path = if name == "logs" {
            "/v1/logs"
        } else {
            "/v1development/profiles"
        };
        assert_real_http_smoke(&collector, name, path);
        client.shutdown(DEADLINE).unwrap();
    }
}

struct CancelledExporter(AtomicUsize);
impl SubmissionExporter for CancelledExporter {
    fn export(&self, _: Signal, _: &[SubmissionEnvelope]) -> Result<(), SubmissionExportFailure> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Err(SubmissionExportFailure::Retryable(
            ExportError::ShutdownCancelledRetry {
                context: context(
                    error_codes::SC_OBSERVABILITY_ADMIT_PERSISTENCE,
                    "transport shutdown",
                ),
            },
        ))
    }
}

fn logs_row(shared: &Shared) -> (String, u32, Option<String>) {
    shared
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT state,attempts,claimed_by FROM signal_deliveries",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
}

#[test]
fn interrupted_transport_remains_pending_for_replacement_client() {
    let dir = tempfile::tempdir().unwrap();
    let exporter = Arc::new(CancelledExporter(AtomicUsize::new(0)));
    let config = config(dir.path());
    let client = conformance::open_manual(|| {
        DurableTelemetryClient::open_with_exporter(config.clone(), exporter.clone())
    })
    .unwrap();
    let receipt = client.emit(fixture("logs")).unwrap();
    let shared = &client.owner.shared;
    {
        let _clock = FrozenClock::new();
        assert!(
            !drain_once_bounded(shared, exporter.as_ref(), Signal::Logs),
            "an interrupted export is not delivery progress"
        );
    }
    assert_eq!(exporter.0.load(Ordering::Acquire), 1);
    let status = client
        .status(StatusQuery::Submissions(vec![
            receipt.submission_id.clone(),
        ]))
        .unwrap();
    assert!(matches!(
        status.submissions[0].signals[0].1,
        DeliveryState::Pending
    ));
    assert_eq!(logs_row(shared), ("pending".to_owned(), 0, None));
    // A manual client has no worker to release its lease on exit; perform the
    // release the last worker owns, then stop the client without draining.
    worker::release(shared);
    drop(client);
    let replacement = conformance::open_manual(|| {
        DurableTelemetryClient::open_with_exporter(
            config,
            Arc::new(ScriptedExporter::new(dir.path())),
        )
    })
    .unwrap();
    assert_eq!(
        replacement
            .flush_submission(&receipt.submission_id, DEADLINE)
            .unwrap()
            .delivered
            .logs,
        1
    );
    assert_eq!(exporter.0.load(Ordering::Acquire), 1);
    replacement.shutdown(DEADLINE).unwrap();
}

#[test]
fn interrupted_transport_stops_only_its_signal_worker() {
    let dir = tempfile::tempdir().unwrap();
    let exporter = Arc::new(CancelledExporter(AtomicUsize::new(0)));
    let client =
        DurableTelemetryClient::open_with_exporter(config(dir.path()), exporter.clone()).unwrap();
    let receipt = client.emit(fixture("logs")).unwrap();
    let shared = &client.owner.shared;
    // The logs worker exits after its first owned Interrupted result; the
    // heartbeat and the three other signal workers stay live.
    let deadline = Instant::now() + DEADLINE;
    loop {
        let generation = shared.generation();
        if shared.live_workers.load(Ordering::Acquire) == 4 {
            break;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "logs worker did not stop after the interrupted export"
        );
        shared.wait_since(generation, remaining);
    }
    let calls = exporter.0.load(Ordering::Acquire);
    assert!(calls >= 1);
    let (state, _, claimed_by) = logs_row(shared);
    assert_eq!((state.as_str(), claimed_by), ("pending", None));
    let status = client
        .status(StatusQuery::Submissions(vec![receipt.submission_id]))
        .unwrap();
    assert!(matches!(
        status.submissions[0].signals[0].1,
        DeliveryState::Pending
    ));
    // The pending row cannot be flushed; whether the deadline or the final store
    // lock budget expires first depends on live worker transactions.
    assert!(client.shutdown(Duration::from_millis(10)).is_err());
    worker::join(shared, DEADLINE);
    assert_eq!(
        exporter.0.load(Ordering::Acquire),
        calls,
        "a stopped exporter is not called again"
    );
}

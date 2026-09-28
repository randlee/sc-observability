use super::implementation::{
    OtlpHttpExporter, build_logs_payload, normalize_logs_endpoint, parse_retry_after,
};
use crate::contracts::{ExporterLifecycle, LogExporter};
use sc_observability_types::{
    ActionName, Level, LogEvent, ProcessIdentity, SchemaVersion, ServiceName, TargetCategory,
    Timestamp,
};
use serde_json::Value;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime};

static RETRY_WAIT_TEST_LOCK: Mutex<()> = Mutex::new(());

fn sample_log() -> LogEvent {
    LogEvent {
        version: SchemaVersion::new("v1").expect("schema version"),
        timestamp: Timestamp::UNIX_EPOCH,
        level: Level::Info,
        service: ServiceName::new("legacy-test").expect("service"),
        target: TargetCategory::new("legacy.http").expect("target"),
        action: ActionName::new("export").expect("action"),
        message: Some("hello".to_owned()),
        identity: ProcessIdentity::default(),
        trace: None,
        request_id: None,
        correlation_id: None,
        outcome: None,
        diagnostic: None,
        state_transition: None,
        fields: serde_json::Map::new(),
    }
}

fn read_request(stream: &mut std::net::TcpStream) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let read = stream.read(&mut buffer).expect("read request");
    request.extend_from_slice(&buffer[..read]);
    String::from_utf8_lossy(&request).into_owned()
}

#[test]
fn endpoint_normalization_preserves_existing_suffix() {
    assert_eq!(
        normalize_logs_endpoint("https://collector.example/"),
        "https://collector.example/v1/logs"
    );
    assert_eq!(
        normalize_logs_endpoint("https://collector.example/v1/logs"),
        "https://collector.example/v1/logs"
    );
}

#[test]
fn retry_after_accepts_delta_seconds_and_bounded_dates() {
    assert_eq!(
        parse_retry_after("3", SystemTime::UNIX_EPOCH),
        Some(Duration::from_secs(3))
    );
    assert!(parse_retry_after(&"x".repeat(129), SystemTime::now()).is_none());
}

#[test]
fn safety_delta_retry_classification_is_bounded() {
    assert!(super::implementation::is_retryable_status(
        reqwest::StatusCode::REQUEST_TIMEOUT
    ));
    assert!(super::implementation::is_retryable_status(
        reqwest::StatusCode::TOO_MANY_REQUESTS
    ));
    assert!(super::implementation::is_retryable_status(
        reqwest::StatusCode::INTERNAL_SERVER_ERROR
    ));
    assert!(!super::implementation::is_retryable_status(
        reqwest::StatusCode::BAD_REQUEST
    ));
}

#[test]
fn safety_delta_shutdown_cancels_retry_wait() {
    let _retry_wait_test_guard = RETRY_WAIT_TEST_LOCK.lock().expect("retry wait test lock");
    let cancel = std::sync::atomic::AtomicBool::new(false);
    cancel.store(true, Ordering::Release);
    assert!(!super::implementation::wait_cancelable(
        Duration::from_secs(30),
        &cancel
    ));
}

#[test]
fn provenance_pin_and_destination_disposition_are_present() {
    let manifest = include_str!("../../../../docs/plans/phase-d/legacy-otlp-provenance.json");
    assert!(manifest.contains("7b39f4e7f72b6845edec4eab4cd671611661445f"));
    assert!(manifest.contains("transplant-and-adapt"));
    assert!(manifest.contains("timestamp_export_integration.rs"));
    assert!(manifest.contains("legacy_http_json"));
}

#[test]
fn log_payload_preserves_timestamp_and_severity() {
    let record = super::implementation::log_record(&sample_log());
    let payload = build_logs_payload(&[record]);
    let log = &payload["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0];
    assert_eq!(log["timeUnixNano"], Value::String("0".to_owned()));
    assert_eq!(log["severityNumber"], 9);
    assert_eq!(log["body"]["stringValue"], "hello");
}

#[test]
fn loopback_export_posts_json_to_logs_endpoint() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    let seen = Arc::new(AtomicUsize::new(0));
    let seen_server = Arc::clone(&seen);
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let request = read_request(&mut stream);
        assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
        assert!(request.contains("\"hello\""));
        seen_server.fetch_add(1, Ordering::Relaxed);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("write response");
    });
    let exporter =
        OtlpHttpExporter::for_endpoint(format!("http://{address}")).expect("construct exporter");
    exporter
        .export_logs(&[sample_log()])
        .expect("export succeeds");
    server.join().expect("join server");
    assert_eq!(seen.load(Ordering::Relaxed), 1);
}

#[test]
fn terminal_client_status_is_not_retried() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_server = Arc::clone(&calls);
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let _ = read_request(&mut stream);
        calls_server.fetch_add(1, Ordering::Relaxed);
        stream
            .write_all(
                b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .expect("write response");
    });
    let exporter =
        OtlpHttpExporter::for_endpoint(format!("http://{address}")).expect("construct exporter");
    let error = exporter
        .send_payload_sync(
            "logs",
            &build_logs_payload(&[super::implementation::log_record(&sample_log())]),
        )
        .expect_err("400 is terminal");
    assert!(matches!(
        error,
        sc_observability_types::v2::ExportError::NonRetryableHttpStatus { .. }
    ));
    server.join().expect("join server");
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn response_loss_retries_the_same_batch_without_false_success_drop() {
    let _retry_wait_test_guard = RETRY_WAIT_TEST_LOCK.lock().expect("retry wait test lock");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_server = Arc::clone(&calls);
    let server = thread::spawn(move || {
        for attempt in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept request");
            let request = read_request(&mut stream);
            assert!(request.contains("\"hello\""));
            calls_server.fetch_add(1, Ordering::Relaxed);
            if attempt == 1 {
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .expect("write retry response");
            }
        }
    });
    let exporter =
        OtlpHttpExporter::for_endpoint(format!("http://{address}")).expect("construct exporter");
    exporter
        .send_payload_sync(
            "logs",
            &build_logs_payload(&[super::implementation::log_record(&sample_log())]),
        )
        .expect("response loss is retried");
    server.join().expect("join server");
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

#[test]
fn shutdown_cancels_an_actual_retry_backoff() {
    let _retry_wait_test_guard = RETRY_WAIT_TEST_LOCK.lock().expect("retry wait test lock");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let request = read_request(&mut stream);
        assert!(request.contains("\"hello\""));
        stream
            .write_all(
                b"HTTP/1.1 503 Service Unavailable\r\nRetry-After: 30\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .expect("write retry response");
    });
    let exporter = Arc::new(
        OtlpHttpExporter::for_endpoint(format!("http://{address}")).expect("construct exporter"),
    );
    let (retry_wait_tx, retry_wait_rx) = mpsc::sync_channel(0);
    super::implementation::install_retry_wait_hook(retry_wait_tx);
    let export = Arc::clone(&exporter);
    let export_thread = thread::spawn(move || {
        export.send_payload_sync(
            "logs",
            &build_logs_payload(&[super::implementation::log_record(&sample_log())]),
        )
    });
    retry_wait_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("request entered the retry backoff");
    super::implementation::clear_retry_wait_hook();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let shutdown = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(1), exporter.shutdown_async()).await
    });
    assert!(shutdown.is_ok(), "shutdown did not cancel retry backoff");
    let error = export_thread
        .join()
        .expect("join export thread")
        .expect_err("shutdown must cancel the retry instead of waiting for the Retry-After delay");
    assert!(matches!(
        error,
        sc_observability_types::v2::ExportError::ShutdownCancelledRetry { .. }
    ));
    server.join().expect("join server");
}

#[test]
fn async_shutdown_stays_responsive_during_an_in_flight_request() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    let (request_started_tx, request_started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let request = read_request(&mut stream);
        assert!(request.contains("\"hello\""));
        request_started_tx.send(()).expect("signal request start");
        release_rx.recv().expect("release request");
    });
    let exporter = Arc::new(
        OtlpHttpExporter::for_endpoint(format!("http://{address}")).expect("construct exporter"),
    );
    let export = Arc::clone(&exporter);
    let export_thread = thread::spawn(move || {
        export.send_payload_sync(
            "logs",
            &build_logs_payload(&[super::implementation::log_record(&sample_log())]),
        )
    });
    request_started_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("request entered the server");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let (marker_tx, _marker_rx) = tokio::sync::oneshot::channel();
    let (responsive, _shutdown_result) = runtime.block_on(async {
        let shutdown = exporter.shutdown_async();
        tokio::pin!(shutdown);
        let marker = async {
            tokio::task::yield_now().await;
            let _ = marker_tx.send(());
        };
        tokio::pin!(marker);
        tokio::select! {
            () = &mut marker => {
                release_tx.send(()).expect("release in-flight request");
                (true, shutdown.await)
            }
            result = &mut shutdown => (false, result),
        }
    });
    assert!(
        responsive,
        "async shutdown blocked the executor while the request was in flight"
    );
    let error = export_thread
        .join()
        .expect("join export thread")
        .expect_err("shutdown must cancel the in-flight request retry");
    assert!(matches!(
        error,
        sc_observability_types::v2::ExportError::ShutdownCancelledRetry { .. }
    ));
    server.join().expect("join server");
}

#[test]
fn blocking_lifecycle_is_rejected_from_entered_tokio() {
    let exporter = OtlpHttpExporter::for_endpoint("http://127.0.0.1:4318".to_owned())
        .expect("construct exporter");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let error = runtime.block_on(async { exporter.flush_blocking() });
    assert!(matches!(
        error,
        Err(sc_observability_types::v2::ExportError::BlockingBackendInAsyncContext { .. })
    ));
}

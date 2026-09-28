use super::implementation::{
    OtlpHttpExporter, build_logs_payload, normalize_logs_endpoint, parse_retry_after,
};
use crate::config::LegacyRetryPolicy;
use crate::contracts::{ExporterLifecycle, LogExporter};
use sc_observability_types::{
    ActionName, CorrelationId, Level, LogEvent, ProcessIdentity, SchemaVersion, ServiceName,
    TargetCategory, Timestamp,
};
use serde_json::Value;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

static RETRY_WAIT_TEST_LOCK: Mutex<()> = Mutex::new(());

// This is a test watchdog, not the production handshake deadline (1 ms).
const STARTUP_TEST_WATCHDOG: Duration = Duration::from_secs(10);

struct StartupFixture {
    initialize: mpsc::Sender<()>,
    receive: mpsc::Sender<()>,
    ready: mpsc::Receiver<()>,
    exited: mpsc::Receiver<bool>,
    result: mpsc::Receiver<Result<OtlpHttpExporter, sc_observability_types::v2::ExportError>>,
    constructor: thread::JoinHandle<()>,
}

impl StartupFixture {
    fn start() -> Self {
        let (initialize, initialize_rx) = mpsc::channel();
        let (receive, receive_rx) = mpsc::channel();
        let (ready_tx, ready) = mpsc::channel();
        let (exited_tx, exited) = mpsc::channel();
        let (result_tx, result) = mpsc::channel();
        let hooks = Arc::new(super::implementation::StartupTestHooks {
            initialize: Mutex::new(initialize_rx),
            receive: Mutex::new(receive_rx),
            ready: ready_tx,
            exited: exited_tx,
        });
        let constructor = thread::spawn(move || {
            let _ = result_tx.send(OtlpHttpExporter::for_startup_test(hooks));
        });
        Self {
            initialize,
            receive,
            ready,
            exited,
            result,
            constructor,
        }
    }
}

#[test]
fn construction_handshake_timeout_cancels_startup_and_disposes_client() {
    let fixture = StartupFixture::start();
    // The real worker cannot initialize until after the actual timed receive
    // expires. Holding a channel gate avoids any sleep/scheduler race.
    fixture
        .receive
        .send(())
        .expect("start timed readiness receive");
    let result = fixture
        .result
        .recv_timeout(STARTUP_TEST_WATCHDOG)
        .expect("constructor returns within watchdog while initialization is held");
    fixture
        .initialize
        .send(())
        .expect("release initializer for worker disposal");
    let cancelled = fixture
        .exited
        .recv_timeout(STARTUP_TEST_WATCHDOG)
        .expect("worker exits and drops its actual reqwest client");
    fixture
        .constructor
        .join()
        .expect("constructor thread exits");
    let Err(sc_observability_types::v2::ExportError::Transport { context }) = result else {
        panic!("expired construction handshake must not publish an exporter");
    };
    assert_eq!(
        context.diagnostic().code,
        sc_observability_types::error_codes::otlp::OTLP_TRANSPORT_CONSTRUCTION_FAILED
    );
    assert!(
        context
            .diagnostic()
            .message
            .contains("construction handshake")
    );
    assert!(
        cancelled,
        "expired handshake must cancel startup before returning"
    );
}

#[test]
fn construction_handshake_ready_before_deadline_publishes_usable_handle() {
    let fixture = StartupFixture::start();
    fixture
        .initialize
        .send(())
        .expect("release real client initialization");
    fixture
        .ready
        .recv_timeout(STARTUP_TEST_WATCHDOG)
        .expect("actual worker queues readiness after building its client");
    // Readiness is already queued when recv_timeout starts. Even the 1 ms
    // boundary succeeds regardless of how long either test thread was paused.
    fixture
        .receive
        .send(())
        .expect("start timed readiness receive");
    let exporter = fixture
        .result
        .recv_timeout(STARTUP_TEST_WATCHDOG)
        .expect("constructor completes")
        .expect("ready handshake publishes exporter");
    exporter
        .flush_blocking()
        .expect("published worker handles flush");
    exporter
        .shutdown_blocking()
        .expect("published worker shuts down");
    assert!(
        fixture
            .exited
            .recv_timeout(STARTUP_TEST_WATCHDOG)
            .expect("worker disposes client on its owning thread")
    );
    fixture
        .constructor
        .join()
        .expect("constructor thread exits");
}

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

// Adapted from `otlp_http_exporter_loads_custom_ca_bundle` at immutable
// source 7b39f4e7f72b6845edec4eab4cd671611661445f. This valid root fixture
// verifies retained custom-root input handling; collector/TLS endpoint
// qualification remains D.9 scope.
const CUSTOM_CA_PEM: &str = "-----BEGIN CERTIFICATE-----\nMIIDCTCCAfGgAwIBAgIUPC5ERscjwotMtYG0fdpdfqGOV3owDQYJKoZIhvcNAQEL\nBQAwFDESMBAGA1UEAwwJbG9jYWxob3N0MB4XDTI2MDkyODA3NTk1OFoXDTI2MDky\nOTA3NTk1OFowFDESMBAGA1UEAwwJbG9jYWxob3N0MIIBIjANBgkqhkiG9w0BAQEF\nAAOCAQ8AMIIBCgKCAQEAxaq67o7/VZfr8g5f4pt9q8A8cKDuQqkt4tIl1iDEtNtE\nuCrODPoaOaOCE2YCCWKLhomg2zE8rxS9aLPiYp0bHrtt+Pep7Eiec655+9yAqhc/\nT2GOjB2Os4PVUYs/pBXw/yl8gxFoXblR+TDKxv9uAAqYOovEChwjQ2Ux/LeTFZyD\nrP78de2GKWTreKnok4gx9B1T73KnIdujUYYb1KgMURN303l03HkR3KvvG2a4n3Ut\naAeIEPq06c0S7p0ZzavdNDZzwGL+jaSzK8EGZ/0PbVS5c9bYKzMinaQks62IJ6Ax\n8dt8PqiFSvgo/h+TGmKBDnBu61G+tUw/mU+vFIDSoQIDAQABo1MwUTAdBgNVHQ4E\nFgQUj6Jj0MpubqGr2MgW9cZVC82GX8AwHwYDVR0jBBgwFoAUj6Jj0MpubqGr2MgW\n9cZVC82GX8AwDwYDVR0TAQH/BAUwAwEB/zANBgkqhkiG9w0BAQsFAAOCAQEAC7Lo\ncI4JTe6deieiMSd2OkpTfoXUg6nAgmFgtXvnmbPiCUogjvVmuJURCY4giA4V1e7x\n/EWKmdelNqKi8mtitGww5V/T2tBqGZomrY9D7ihIygIm+jLHyQWYs9Zja+4ANhCi\nU5VIZUoioUanjwSmO9IOCG/bP755EKclTGcYMV3m4o+vb1nNekuEruMikAdXI5Dx\n8hak8/hkhCImuUw/0zxPt8/Yj5B4Rsx415iOO6UtxOVv1n1W89roO2ocv5a1gs1b\nnecEcX2nGabOFi4UAgCchMGzetKFB67oOjknKuFb0rBOjCfvnoDq265kOKMspdsl\nj0O08tFGchg3b1aLiA==\n-----END CERTIFICATE-----\n";

fn custom_ca_file(contents: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "sc-observability-otlp-d8-ca-{}-{nonce}.pem",
        process::id()
    ));
    fs::write(&path, contents).expect("write temporary CA bundle");
    path
}

fn retry_policy(
    max_retries: u32,
    initial_backoff_ms: u64,
    max_backoff_ms: u64,
    retry_sequence_timeout_ms: u64,
    retry_after_cap_ms: u64,
    retry_jitter_percent: u8,
) -> LegacyRetryPolicy {
    LegacyRetryPolicy {
        max_retries: Some(max_retries),
        initial_backoff_ms: Some(initial_backoff_ms.into()),
        max_backoff_ms: Some(max_backoff_ms.into()),
        retry_sequence_timeout_ms: Some(retry_sequence_timeout_ms.into()),
        retry_after_cap_ms: Some(retry_after_cap_ms.into()),
        retry_jitter_percent: Some(retry_jitter_percent),
    }
}

fn logs_payload() -> Value {
    build_logs_payload(&[super::implementation::log_record(&sample_log())])
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
    let manifest: Value = serde_json::from_str(manifest).expect("valid provenance manifest");
    assert_eq!(
        manifest["source_commit"],
        "7b39f4e7f72b6845edec4eab4cd671611661445f"
    );

    let destinations = [
        (
            "crates/sc-observability-otlp/src/lib.rs",
            "crates/sc-observability-otlp/src/legacy_http_json/implementation.rs",
        ),
        (
            "crates/sc-observability-otlp/tests/timestamp_export_integration.rs",
            "crates/sc-observability-otlp/src/legacy_http_json/tests.rs",
        ),
    ];
    for (source, destination) in destinations {
        let entry = manifest["entries"]
            .as_array()
            .expect("manifest entries")
            .iter()
            .find(|entry| entry["source"] == source)
            .unwrap_or_else(|| panic!("missing provenance source {source}"));
        assert_eq!(entry["destination"], destination);
        assert_eq!(entry["disposition"], "transplant-and-adapt");
        assert!(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(destination)
                .is_file(),
            "missing provenance destination {destination}"
        );
    }
}

#[test]
fn log_payload_preserves_service_correlation_timestamp_severity_and_body() {
    let mut event = sample_log();
    event.correlation_id = Some(CorrelationId::new("corr-42").expect("correlation id"));
    let record = super::implementation::log_record(&event);
    let payload = build_logs_payload(&[record]);
    let resource_logs = &payload["resourceLogs"][0];
    let log = &resource_logs["scopeLogs"][0]["logRecords"][0];
    assert_eq!(
        resource_logs["resource"]["attributes"][0]["key"],
        "service.name"
    );
    assert_eq!(
        resource_logs["resource"]["attributes"][0]["value"]["stringValue"],
        "legacy-test"
    );
    assert!(
        log["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|attribute| {
                attribute["key"] == "sc.observability.log.correlation_id"
                    && attribute["value"]["stringValue"] == "corr-42"
            })
    );
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
fn retained_auth_header_fixture_posts_expected_header_and_payload() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let request = read_request(&mut stream);
        assert!(request.starts_with("POST /v1/logs HTTP/1.1"));
        assert!(request.contains("authorization: Bearer d8-fixture-token"));
        assert!(request.contains("\"hello\""));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("write response");
    });
    let exporter = OtlpHttpExporter::for_test_config(
        format!("http://{address}"),
        Some("authorization: Bearer d8-fixture-token"),
        None,
    )
    .expect("construct exporter");
    exporter
        .export_logs(&[sample_log()])
        .expect("authenticated export succeeds");
    server.join().expect("join server");
}

#[test]
fn retained_wrong_auth_fixture_is_rejected_without_credential_diagnostic() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let request = read_request(&mut stream);
        assert!(request.contains("authorization: Bearer wrong-d8-fixture-token"));
        stream
            .write_all(
                b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .expect("write rejection");
    });
    let exporter = OtlpHttpExporter::for_test_config(
        format!("http://{address}"),
        Some("authorization: Bearer wrong-d8-fixture-token"),
        None,
    )
    .expect("construct exporter");
    let error = exporter
        .send_payload_sync(
            "logs",
            &build_logs_payload(&[super::implementation::log_record(&sample_log())]),
        )
        .expect_err("wrong credential is rejected by collector");
    assert!(matches!(
        error,
        sc_observability_types::v2::ExportError::NonRetryableHttpStatus { .. }
    ));
    assert!(
        !format!("{error:?}").contains("wrong-d8-fixture-token"),
        "transport diagnostics must not expose authentication material"
    );
    server.join().expect("join server");
}

#[test]
fn retained_custom_ca_bundle_builds_the_actual_client() {
    let valid_ca = custom_ca_file(CUSTOM_CA_PEM);
    let exporter = OtlpHttpExporter::for_test_config(
        "https://collector.example".to_owned(),
        None,
        Some(valid_ca.clone()),
    );
    fs::remove_file(&valid_ca).expect("remove temporary CA bundle");
    assert!(
        exporter.is_ok(),
        "valid custom CA must build the reqwest client"
    );
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
fn loopback_retry_after_is_capped_before_the_next_request() {
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
            let response = if attempt == 0 {
                b"HTTP/1.1 503 Service Unavailable\r\nRetry-After: 3600\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_slice()
            } else {
                b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_slice()
            };
            stream.write_all(response).expect("write response");
        }
    });
    let (delay_tx, delay_rx) = mpsc::sync_channel(0);
    let exporter = OtlpHttpExporter::for_endpoint_with_retry(
        format!("http://{address}"),
        retry_policy(1, 5, 50, 500, 20, 0),
        1,
        Some(delay_tx),
    )
    .expect("construct exporter");
    let export_thread = thread::spawn(move || exporter.send_payload_sync("logs", &logs_payload()));
    let delay = delay_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("retry entered the capped backoff");
    let result = export_thread.join().expect("join export thread");
    result.expect("capped Retry-After permits the next request");
    server.join().expect("join server");
    assert_eq!(delay, Duration::from_millis(20));
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

#[test]
fn loopback_fallback_jitter_preserves_an_ordered_retry_sequence() {
    let _retry_wait_test_guard = RETRY_WAIT_TEST_LOCK.lock().expect("retry wait test lock");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_server = Arc::clone(&calls);
    let server = thread::spawn(move || {
        for attempt in 0..3 {
            let (mut stream, _) = listener.accept().expect("accept request");
            let request = read_request(&mut stream);
            assert!(request.contains("\"hello\""));
            calls_server.fetch_add(1, Ordering::Relaxed);
            let response = if attempt < 2 {
                b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_slice()
            } else {
                b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_slice()
            };
            stream.write_all(response).expect("write response");
        }
    });
    let (delay_tx, delay_rx) = mpsc::sync_channel(0);
    let exporter = OtlpHttpExporter::for_endpoint_with_retry(
        format!("http://{address}"),
        retry_policy(2, 20, 40, 500, 100, 50),
        1,
        Some(delay_tx),
    )
    .expect("construct exporter");
    let export_thread = thread::spawn(move || exporter.send_payload_sync("logs", &logs_payload()));
    let first_delay = delay_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("first fallback retry entered backoff");
    let second_delay = delay_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("second fallback retry entered backoff");
    let result = export_thread.join().expect("join export thread");
    result.expect("fallback retries eventually succeed");
    server.join().expect("join server");
    assert!(
        (10..=30).contains(&first_delay.as_millis()),
        "unexpected first fallback delay: {first_delay:?}"
    );
    assert!(
        (20..=60).contains(&second_delay.as_millis()),
        "unexpected second fallback delay: {second_delay:?}"
    );
    assert_eq!(calls.load(Ordering::Relaxed), 3);
}

#[test]
fn loopback_retry_sequence_returns_deadline_exhaustion_after_multiple_requests() {
    let _retry_wait_test_guard = RETRY_WAIT_TEST_LOCK.lock().expect("retry wait test lock");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_server = Arc::clone(&calls);
    let done = Arc::new(AtomicBool::new(false));
    let done_server = Arc::clone(&done);
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut handled = 0;
        listener
            .set_nonblocking(true)
            .expect("set nonblocking listener");
        while !done_server.load(Ordering::Acquire) && Instant::now() < deadline {
            let (mut stream, _) = match listener.accept() {
                Ok(accepted) => accepted,
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    thread::yield_now();
                    continue;
                }
                Err(error) => panic!("accept request: {error}"),
            };
            stream
                .set_nonblocking(false)
                .expect("set blocking accepted stream");
            let request = read_request(&mut stream);
            assert!(request.contains("\"hello\""));
            calls_server.fetch_add(1, Ordering::Relaxed);
            stream
                .write_all(
                    b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .expect("write response");
            handled += 1;
        }
        assert!(handled >= 2, "deadline fixture must observe two requests");
    });
    let exporter = OtlpHttpExporter::for_endpoint_with_retry(
        format!("http://{address}"),
        retry_policy(100, 20, 20, 500, 20, 0),
        1,
        None,
    )
    .expect("construct exporter");
    let error = exporter
        .send_payload_sync("logs", &logs_payload())
        .expect_err("retry sequence must exhaust its deadline");
    done.store(true, Ordering::Release);
    server.join().expect("join server");
    assert!(matches!(
        error,
        sc_observability_types::v2::ExportError::RetryDeadlineExhausted { .. }
    ));
    assert!(calls.load(Ordering::Relaxed) >= 2);
}

#[test]
fn loopback_retry_attempt_limit_returns_typed_exhaustion() {
    let _retry_wait_test_guard = RETRY_WAIT_TEST_LOCK.lock().expect("retry wait test lock");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_server = Arc::clone(&calls);
    let server = thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("accept request");
            let request = read_request(&mut stream);
            assert!(request.contains("\"hello\""));
            calls_server.fetch_add(1, Ordering::Relaxed);
            stream
                .write_all(
                    b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .expect("write response");
        }
    });
    let exporter = OtlpHttpExporter::for_endpoint_with_retry(
        format!("http://{address}"),
        retry_policy(1, 5, 5, 500, 20, 0),
        1,
        None,
    )
    .expect("construct exporter");
    let error = exporter
        .send_payload_sync("logs", &logs_payload())
        .expect_err("retry attempt limit must stop the sequence");
    server.join().expect("join server");
    assert!(matches!(
        error,
        sc_observability_types::v2::ExportError::RetryAttemptsExhausted { .. }
    ));
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
    let (retry_wait_tx, retry_wait_rx) = mpsc::channel();
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

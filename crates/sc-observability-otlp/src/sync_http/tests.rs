use super::implementation::{
    OtlpHttpExporter, build_logs_payload, normalize_logs_endpoint, parse_retry_after,
};
use crate::config::SyncHttpRetryPolicy;
use crate::contracts::{ExporterLifecycle, LogExporter};
use crate::lifecycle::LifecycleState;
use sc_observability_types::{
    ActionName, CorrelationId, Level, LogEvent, ProcessIdentity, Remediation, SchemaVersion,
    ServiceName, TargetCategory, Timestamp,
};
use serde_json::Value;
use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{self, Command, Stdio};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

// This is a test watchdog, not the production handshake deadline (1 ms).
const STARTUP_TEST_WATCHDOG: Duration = Duration::from_secs(10);
const REQUEST_FIXTURE_WATCHDOG: Duration = Duration::from_secs(2);
const STALLED_RETRY_SEQUENCE_TIMEOUT_MS: u64 = 4_000;
const STALLED_RETRY_REQUEST_TIMEOUT_MS: u64 = 4_000;
const STALLED_RETRY_BACKOFF_MS: u64 = 2_500;
const STALLED_RETRY_SERVER_WATCHDOG: Duration = Duration::from_secs(20);
const STALLED_RETRY_ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(10);
const STALLED_RETRY_STEP_WATCHDOG: Duration = Duration::from_secs(10);
const STALLED_RETRY_EXPORT_WATCHDOG: Duration = Duration::from_secs(10);
const STALLED_RETRY_CLEANUP_WATCHDOG: Duration = Duration::from_secs(2);
const CONTROL_ORDERING_WATCHDOG: Duration = Duration::from_secs(2);

#[test]
fn sync_http_error_remediations_preserve_diagnostics_and_match_failure_semantics() {
    let cases = [
        (
            super::implementation::worker_terminated_error(),
            sc_observability_types::error_codes::otlp::OTLP_WORKER_TERMINATED,
            "synchronous HTTP worker terminated",
            Remediation::recoverable(
                "restart the synchronous HTTP exporter",
                ["resubmit any batch that was not acknowledged"],
            ),
        ),
        (
            super::implementation::queue_full_error(),
            sc_observability_types::error_codes::otlp::OTLP_QUEUE_FULL,
            "synchronous HTTP worker admission is full",
            Remediation::recoverable(
                "wait for synchronous HTTP worker capacity",
                ["retry the export after capacity is available"],
            ),
        ),
        (
            super::implementation::shutdown_cancelled_error(),
            sc_observability_types::error_codes::otlp::OTLP_SHUTDOWN_CANCELLED_RETRY,
            "synchronous HTTP retry was cancelled by shutdown",
            Remediation::not_recoverable(
                "the synchronous HTTP exporter is shutting down and cannot retry this batch",
            ),
        ),
        (
            super::implementation::retry_deadline_error(),
            sc_observability_types::error_codes::otlp::OTLP_RETRY_DEADLINE_EXHAUSTED,
            "synchronous HTTP retry sequence exceeded its deadline",
            Remediation::recoverable(
                "restore collector availability before retrying the export",
                ["increase the retry sequence deadline only when the delivery budget permits"],
            ),
        ),
        (
            super::implementation::non_retryable_status_error(400),
            sc_observability_types::error_codes::otlp::OTLP_HTTP_STATUS_TERMINAL,
            "synchronous HTTP collector returned terminal HTTP status 400",
            Remediation::not_recoverable(
                "correct the collector request, credentials, or endpoint before submitting a new batch",
            ),
        ),
    ];

    for (error, code, message, remediation) in cases {
        assert_eq!(error.diagnostic().code, code);
        assert_eq!(error.diagnostic().message, message);
        assert_eq!(error.diagnostic().remediation, remediation);
    }
}

#[test]
fn control_reply_timeout_and_disconnect_have_distinct_typed_errors() {
    let (_sender, receiver) = mpsc::channel();
    assert!(matches!(
        super::implementation::wait_for_control_result(&receiver, Duration::ZERO),
        Err(sc_observability_types::v2::ExportError::LifecycleTimeout { .. })
    ));

    let (sender, receiver) = mpsc::channel();
    drop(sender);
    assert!(matches!(
        super::implementation::wait_for_control_result(&receiver, CONTROL_ORDERING_WATCHDOG),
        Err(sc_observability_types::v2::ExportError::WorkerTerminated { .. })
    ));
}

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
        service: ServiceName::new("sync-http-test").expect("service"),
        target: TargetCategory::new("sync-http.http").expect("target"),
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
    read_framed_request(stream, Instant::now() + REQUEST_FIXTURE_WATCHDOG)
        .expect("read complete HTTP request")
}

const RETRY_OBSERVER_SERVER_WATCHDOG: Duration = Duration::from_secs(8);
const RETRY_OBSERVER_ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(10);
const RETRY_OBSERVER_NEGATIVE_WINDOW: Duration = Duration::from_millis(100);
const RETRY_SLEEP_BUDGET_REQUEST_CONSUMPTION: Duration = Duration::from_millis(800);
const RETRY_SLEEP_BUDGET_SEQUENCE_TIMEOUT_MS: u64 = 1_500;
const RETRY_SLEEP_BUDGET_MAX_OBSERVED_DELAY: Duration = Duration::from_millis(700);
const RETRY_SLEEP_BUDGET_BACKOFF_MS: u64 = 2_000;

fn accept_retry_observer_request(listener: &TcpListener, deadline: Instant) -> std::net::TcpStream {
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_nonblocking(false)
                    .expect("restore blocking mode for accepted retry-observer socket");
                return stream;
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "request was not accepted before the server watchdog"
                );
                thread::sleep(RETRY_OBSERVER_ACCEPT_POLL_INTERVAL);
            }
            Err(error) => panic!("accept retry-observer request: {error}"),
        }
    }
}

fn set_retry_observer_io_deadlines(stream: &std::net::TcpStream, deadline: Instant) {
    let remaining = deadline.saturating_duration_since(Instant::now());
    assert!(!remaining.is_zero(), "server watchdog expired before I/O");
    stream
        .set_read_timeout(Some(remaining))
        .expect("set retry-observer server read deadline");
    stream
        .set_write_timeout(Some(remaining))
        .expect("set retry-observer server write deadline");
}

struct DeadlineReader<'a> {
    stream: &'a mut std::net::TcpStream,
    deadline: Instant,
}

impl Read for DeadlineReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(std::io::Error::new(
                ErrorKind::TimedOut,
                "server watchdog expired",
            ));
        }
        self.stream.set_read_timeout(Some(remaining))?;
        self.stream.read(buffer)
    }
}

fn read_framed_request_from_reader(reader: &mut impl Read) -> Result<String, String> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut expected_len = None;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("read complete HTTP request: {error}"))?;
        if read == 0 {
            return Err("client closed before sending the complete request".to_owned());
        }
        request.extend_from_slice(&buffer[..read]);

        if expected_len.is_none()
            && let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
        {
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_len = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .ok_or_else(|| "HTTP request includes a valid Content-Length".to_owned())?;
            expected_len = Some(header_end + 4 + content_len);
        }

        if expected_len.is_some_and(|expected| request.len() >= expected) {
            return Ok(String::from_utf8_lossy(&request).into_owned());
        }
    }
}

fn read_framed_request(
    stream: &mut std::net::TcpStream,
    deadline: Instant,
) -> Result<String, String> {
    read_framed_request_from_reader(&mut DeadlineReader { stream, deadline })
}

fn read_retry_observer_request(stream: &mut std::net::TcpStream, deadline: Instant) -> String {
    read_framed_request(stream, deadline).expect("read complete retry-observer HTTP request")
}

#[test]
fn framed_request_reader_collects_fragmented_headers_and_body() {
    struct ScriptedReader(VecDeque<Vec<u8>>);
    impl Read for ScriptedReader {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let Some(chunk) = self.0.pop_front() else {
                return Ok(0);
            };
            buffer[..chunk.len()].copy_from_slice(&chunk);
            Ok(chunk.len())
        }
    }
    let mut reader = ScriptedReader(VecDeque::from([
        b"POST /v1/logs HTTP/1.1\r\nContent-".to_vec(),
        b"Length: 5\r\n\r\nhe".to_vec(),
        b"llo".to_vec(),
    ]));
    let request = read_framed_request_from_reader(&mut reader).expect("complete request");
    assert!(request.ends_with("\r\n\r\nhello"));
}

#[test]
fn framed_request_reader_rejects_premature_eof() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixture listener");
    let address = listener.local_addr().expect("fixture address");
    let client = thread::spawn(move || {
        let mut stream = std::net::TcpStream::connect(address).expect("connect fixture client");
        stream
            .write_all(b"POST /v1/logs HTTP/1.1\r\nContent-Length: 5\r\n\r\nhe")
            .expect("write incomplete request");
        stream
            .shutdown(std::net::Shutdown::Write)
            .expect("close incomplete request");
    });
    let (mut stream, _) = listener.accept().expect("accept fixture client");
    let error = read_framed_request(&mut stream, Instant::now() + REQUEST_FIXTURE_WATCHDOG)
        .expect_err("premature EOF must not count as a request");
    client.join().expect("join fixture client");
    assert_eq!(error, "client closed before sending the complete request");
}

fn retrying_server(
    listener: TcpListener,
    first_request_tx: mpsc::Sender<()>,
    release_first_response_rx: mpsc::Receiver<()>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        listener
            .set_nonblocking(true)
            .expect("make retry-observer listener nonblocking");
        let deadline = Instant::now() + RETRY_OBSERVER_SERVER_WATCHDOG;
        let mut first = accept_retry_observer_request(&listener, deadline);
        set_retry_observer_io_deadlines(&first, deadline);
        let request = read_retry_observer_request(&mut first, deadline);
        assert!(request.contains("\"hello\""));
        first_request_tx
            .send(())
            .expect("signal target request is gated at the server");
        release_first_response_rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("release target retry response before the server watchdog");
        first
            .write_all(
                b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .expect("write retry response");

        let mut retry = accept_retry_observer_request(&listener, deadline);
        set_retry_observer_io_deadlines(&retry, deadline);
        let request = read_retry_observer_request(&mut retry, deadline);
        assert!(request.contains("\"hello\""));
        retry
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("write success response");
    })
}

fn join_retry_observer_server(server: thread::JoinHandle<()>) {
    let deadline = Instant::now() + RETRY_OBSERVER_SERVER_WATCHDOG;
    while !server.is_finished() {
        assert!(
            Instant::now() < deadline,
            "retry-observer server did not finish before the join watchdog"
        );
        thread::sleep(RETRY_OBSERVER_ACCEPT_POLL_INTERVAL);
    }
    server.join().expect("retry-observer server exits cleanly");
}

fn retry_sleep_budget_server(
    listener: TcpListener,
    send_retryable_response: bool,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        listener
            .set_nonblocking(true)
            .expect("make retry-budget listener nonblocking");
        let deadline = Instant::now() + RETRY_OBSERVER_SERVER_WATCHDOG;
        let mut first = accept_retry_observer_request(&listener, deadline);
        set_retry_observer_io_deadlines(&first, deadline);
        let request = read_retry_observer_request(&mut first, deadline);
        assert!(request.contains("\"hello\""));
        thread::sleep(RETRY_SLEEP_BUDGET_REQUEST_CONSUMPTION);
        if send_retryable_response {
            first
                .write_all(
                    b"HTTP/1.1 503 Service Unavailable\r\nRetry-After: 2\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .expect("write delayed retry response");
        }
    })
}

fn assert_retry_sleep_uses_post_request_remaining(send_retryable_response: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    let server = retry_sleep_budget_server(listener, send_retryable_response);
    let (delay_tx, delay_rx) = mpsc::channel();
    let exporter = OtlpHttpExporter::for_endpoint_with_retry_timeout(
        format!("http://{address}"),
        retry_policy(
            1,
            RETRY_SLEEP_BUDGET_BACKOFF_MS,
            RETRY_SLEEP_BUDGET_BACKOFF_MS,
            RETRY_SLEEP_BUDGET_SEQUENCE_TIMEOUT_MS,
            RETRY_SLEEP_BUDGET_SEQUENCE_TIMEOUT_MS,
            50,
        ),
        1_200,
        1,
        Some(delay_tx),
    )
    .expect("construct exporter");
    let export_thread = thread::spawn(move || exporter.send_payload_sync("logs", &logs_payload()));
    let delay = delay_rx
        .recv_timeout(RETRY_OBSERVER_SERVER_WATCHDOG)
        .expect("retry entered the observed backoff");
    let result = export_thread.join().expect("join export thread");
    join_retry_observer_server(server);

    assert!(
        delay <= RETRY_SLEEP_BUDGET_MAX_OBSERVED_DELAY,
        "retry sleep {delay:?} exceeded the post-request remaining budget"
    );
    assert!(matches!(
        result,
        Err(sc_observability_types::v2::ExportError::RetryDeadlineExhausted { .. })
    ));
}

#[test]
fn retryable_response_caps_jittered_sleep_by_post_request_remaining() {
    assert_retry_sleep_uses_post_request_remaining(true);
}

#[test]
fn transport_error_caps_jittered_sleep_by_post_request_remaining() {
    assert_retry_sleep_uses_post_request_remaining(false);
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

fn start_tls_test_server(
    cert: &PathBuf,
    key: &PathBuf,
    address: std::net::SocketAddr,
) -> process::Child {
    let server = Command::new("python3")
        .args([
            "-u",
            "-c",
            "import http.server, ssl, sys\nclass H(http.server.BaseHTTPRequestHandler):\n def do_POST(self):\n  n=int(self.headers.get('Content-Length','0')); self.rfile.read(n); self.send_response(200); self.send_header('Content-Length','0'); self.end_headers()\n def log_message(self,*args): pass\ns=http.server.HTTPServer(('127.0.0.1',int(sys.argv[1])),H); c=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER); c.load_cert_chain(sys.argv[2],sys.argv[3]); s.socket=c.wrap_socket(s.socket,server_side=True); print('READY',flush=True); s.serve_forever()",
        ])
        .arg(address.port().to_string())
        .arg(cert)
        .arg(key)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start standard-library local TLS server");
    let mut server = server;
    let mut ready = String::new();
    std::io::BufReader::new(server.stdout.take().expect("TLS server ready output"))
        .read_line(&mut ready)
        .expect("read TLS server readiness");
    assert_eq!(ready.trim(), "READY", "local TLS server became ready");
    server
}

fn retry_policy(
    max_retries: u32,
    initial_backoff_ms: u64,
    max_backoff_ms: u64,
    retry_sequence_timeout_ms: u64,
    retry_after_cap_ms: u64,
    retry_jitter_percent: u8,
) -> SyncHttpRetryPolicy {
    SyncHttpRetryPolicy {
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
fn selected_request_timeout_uses_the_shorter_bound() {
    let cases = [
        (
            Duration::from_secs(2),
            Duration::from_secs(3),
            Duration::from_secs(2),
        ),
        (
            Duration::from_secs(3),
            Duration::from_secs(2),
            Duration::from_secs(2),
        ),
        (
            Duration::from_secs(2),
            Duration::from_secs(2),
            Duration::from_secs(2),
        ),
    ];

    for (request_timeout, remaining, expected) in cases {
        assert_eq!(
            super::implementation::selected_request_timeout(request_timeout, remaining),
            expected
        );
    }
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
    let cancel = std::sync::atomic::AtomicBool::new(false);
    cancel.store(true, Ordering::Release);
    assert!(!super::implementation::wait_cancelable(
        Duration::from_secs(30),
        &cancel
    ));
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
        "sync-http-test"
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
fn malformed_ca_bundle_fails_client_construction() {
    let ca_file = custom_ca_file(
        "-----BEGIN CERTIFICATE-----\nnot-base64-certificate-data\n-----END CERTIFICATE-----\n",
    );
    let result = OtlpHttpExporter::for_test_config(
        "http://127.0.0.1:1".to_owned(),
        None,
        Some(ca_file.clone()),
    );
    fs::remove_file(&ca_file).expect("remove malformed CA fixture");

    let Err(sc_observability_types::v2::ExportError::Transport { context }) = result else {
        panic!("malformed CA bundle must fail HTTP client construction");
    };
    assert_eq!(
        context.diagnostic().code,
        sc_observability_types::error_codes::otlp::OTLP_TRANSPORT_CONSTRUCTION_FAILED
    );
    assert_eq!(
        context.diagnostic().message,
        "failed to build HTTP client",
        "malformed CA bytes should reach client construction, not the read or parse branch"
    );
}

#[test]
fn retained_custom_ca_bundle_verifies_real_tls_exports() {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    let cert = std::env::temp_dir().join(format!("sc-otlp-d8-{nonce}.crt"));
    let key = std::env::temp_dir().join(format!("sc-otlp-d8-{nonce}.key"));
    let generated = Command::new("openssl")
        .args(["req", "-x509", "-newkey", "rsa:2048", "-nodes"])
        .arg("-keyout")
        .arg(&key)
        .arg("-out")
        .arg(&cert)
        .args([
            "-days",
            "1",
            "-subj",
            "/CN=127.0.0.1",
            "-addext",
            "subjectAltName=IP:127.0.0.1",
            "-addext",
            "basicConstraints=critical,CA:FALSE",
            "-addext",
            "extendedKeyUsage=serverAuth",
        ])
        .output()
        .expect("openssl is available for local TLS test");
    assert!(generated.status.success(), "generate local TLS certificate");
    let unrelated = custom_ca_file(CUSTOM_CA_PEM);
    let ca = custom_ca_file(&fs::read_to_string(&cert).expect("read server CA"));
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve TLS port");
    let address = listener.local_addr().expect("TLS address");
    drop(listener);
    let mut server = start_tls_test_server(&cert, &key, address);
    let exporter =
        OtlpHttpExporter::for_test_config(format!("https://{address}"), None, Some(ca.clone()))
            .expect("construct exporter with trusted CA");
    let trusted = exporter.send_payload_sync("logs", &logs_payload());
    exporter
        .shutdown_blocking()
        .expect("shut down successful TLS exporter");
    let _ = server.kill();
    let _ = server.wait();
    assert!(
        trusted.is_ok(),
        "trusted CA completes TLS export: {trusted:?}"
    );

    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve TLS port");
    let address = listener.local_addr().expect("TLS address");
    drop(listener);
    let mut server = start_tls_test_server(&cert, &key, address);
    let exporter = OtlpHttpExporter::for_test_config(
        format!("https://{address}"),
        None,
        Some(unrelated.clone()),
    )
    .expect("unrelated CA does not prevent client construction");
    let rejected = exporter.send_payload_sync("logs", &logs_payload());
    exporter
        .shutdown_blocking()
        .expect("shut down certificate-rejected TLS exporter");
    let _ = server.kill();
    let _ = server.wait();
    let error = rejected.expect_err("unrelated CA fails certificate verification");
    assert!(
        format!("{error:?}").contains("InvalidCertificate"),
        "unrelated CA failure must be certificate verification, got {error:?}"
    );
    let _ = fs::remove_file(ca);
    let _ = fs::remove_file(cert);
    let _ = fs::remove_file(key);
    let _ = fs::remove_file(unrelated);
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
    let (delay_tx, delay_rx) = mpsc::channel();
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
    let (delay_tx, delay_rx) = mpsc::channel();
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

fn drop_without_shutdown_abandons_pending_admission(entered_tokio: bool) {
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
    let (retry_wait_tx, retry_wait_rx) = mpsc::channel();
    let exporter = OtlpHttpExporter::for_endpoint_with_retry(
        format!("http://{address}"),
        retry_policy(3, 100, 200, 5_000, 4_000, 0),
        1,
        Some(retry_wait_tx),
    )
    .expect("construct exporter");
    let lifecycle = exporter.lifecycle_for_test();
    exporter
        .export_logs(&[sample_log()])
        .expect("admit log before final-handle drop");
    retry_wait_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("request entered retained retry wait");

    let flush = exporter.flush_async();
    let flush_thread = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("flush runtime");
        runtime.block_on(flush)
    });
    if entered_tokio {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        runtime.block_on(async {
            drop(exporter);
            tokio::task::yield_now().await;
        });
    } else {
        drop(exporter);
    }

    let outcome = flush_thread.join().expect("join flush observer");
    assert!(matches!(
        outcome,
        Err(sc_observability_types::v2::ExportError::WorkerTerminated { .. })
    ));
    let health = lifecycle.health();
    assert_eq!(health.phase, LifecycleState::Shutdown);
    assert_eq!(health.admitted_records, 0);
    assert_eq!(health.admitted_bytes, 0);
    assert_eq!(health.dropped_by_signal, [1, 0, 0]);
    // A second retained observation is stable and does not count again.
    assert_eq!(lifecycle.health().dropped_by_signal, [1, 0, 0]);
    server.join().expect("join server");
}

#[test]
fn drop_without_shutdown_abandons_pending_admission_on_plain_thread() {
    drop_without_shutdown_abandons_pending_admission(false);
}

#[test]
fn drop_without_shutdown_abandons_pending_admission_in_entered_tokio() {
    drop_without_shutdown_abandons_pending_admission(true);
}

#[test]
fn loopback_retry_sequence_returns_deadline_exhaustion_after_multiple_requests() {
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

fn assert_stalled_retry_applied_timeouts(
    first: Result<(u32, Duration, Duration), mpsc::RecvTimeoutError>,
    second: Result<(u32, Duration, Duration), mpsc::TryRecvError>,
) {
    let configured_timeout = Duration::from_millis(STALLED_RETRY_REQUEST_TIMEOUT_MS);
    let first = first.expect("first request timeout is observed");
    assert_eq!(first.0, 0, "first request is attempt zero");
    assert_eq!(
        first.1,
        configured_timeout.min(first.2),
        "first request applies the minimum of configured and remaining timeout"
    );

    let second = second.expect("second request timeout is observed");
    assert_eq!(second.0, 1, "stalled request is retry attempt one");
    assert!(
        second.2 < configured_timeout,
        "retry has less sequence time remaining than the configured request timeout"
    );
    assert_eq!(
        second.1,
        configured_timeout.min(second.2),
        "second request applies its same-attempt remaining timeout"
    );
}

#[test]
fn loopback_stalled_retry_attempt_is_bounded_by_remaining_sequence_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    listener
        .set_nonblocking(true)
        .expect("set listener nonblocking for bounded accept");
    let (stalled_tx, stalled_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let server_deadline = Instant::now() + STALLED_RETRY_SERVER_WATCHDOG;
        for attempt in 0..2 {
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == ErrorKind::WouldBlock
                            && Instant::now() < server_deadline =>
                    {
                        thread::sleep(STALLED_RETRY_ACCEPT_POLL_INTERVAL);
                    }
                    Err(error) => panic!("accept retry request before deadline: {error}"),
                }
            };
            stream
                .set_nonblocking(false)
                .expect("set accepted stream blocking");
            let request = read_request(&mut stream);
            assert!(request.contains("\"hello\""));
            if attempt == 0 {
                stream
                    .write_all(
                        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .expect("write retryable response");
            } else {
                stalled_tx.send(()).expect("signal stalled final request");
                let _ = release_rx.recv_timeout(Duration::from_secs(8));
            }
        }
    });
    let (delay_tx, delay_rx) = mpsc::channel();
    let (request_timeout_tx, request_timeout_rx) = mpsc::channel();
    let exporter = OtlpHttpExporter::for_endpoint_with_retry_timeout_observing_request_timeout(
        format!("http://{address}"),
        retry_policy(
            1,
            STALLED_RETRY_BACKOFF_MS,
            STALLED_RETRY_BACKOFF_MS,
            STALLED_RETRY_SEQUENCE_TIMEOUT_MS,
            100,
            0,
        ),
        STALLED_RETRY_REQUEST_TIMEOUT_MS,
        1,
        Some(delay_tx),
        Some(request_timeout_tx),
    )
    .expect("construct exporter with equal request and sequence bounds");
    let (result_tx, result_rx) = mpsc::channel();
    let export_thread = thread::spawn(move || {
        let result = exporter.send_payload_sync("logs", &logs_payload());
        let _ = result_tx.send(result);
    });

    let first_timeout = request_timeout_rx.recv_timeout(STALLED_RETRY_STEP_WATCHDOG);
    let delay = delay_rx.recv_timeout(STALLED_RETRY_STEP_WATCHDOG);
    let stalled_request = stalled_rx.recv_timeout(STALLED_RETRY_STEP_WATCHDOG);
    // The server cannot observe this request until the actual timeout argument
    // has been evaluated and sent through the exporter-local observer.
    let second_timeout = request_timeout_rx.try_recv();

    // This watchdog only detects a hung export. The timeout policy assertion
    // below observes the value passed at the real request application site.
    let timely_result = result_rx.recv_timeout(STALLED_RETRY_EXPORT_WATCHDOG);
    let _ = release_tx.send(());
    server
        .join()
        .expect("stalled collector exits after release");
    let completed_before_watchdog = timely_result.is_ok();
    let result = match timely_result {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => result_rx
            .recv_timeout(STALLED_RETRY_CLEANUP_WATCHDOG)
            .expect("closing the loopback connection bounds failure cleanup"),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            panic!("export worker exited without reporting its result")
        }
    };
    export_thread.join().expect("join export worker");

    assert_stalled_retry_applied_timeouts(first_timeout, second_timeout);
    assert_eq!(
        delay.expect("first retry enters the configured backoff"),
        Duration::from_millis(STALLED_RETRY_BACKOFF_MS)
    );
    stalled_request.expect("second request reaches the stalled collector");
    assert!(
        completed_before_watchdog,
        "stalled export must complete before the hang watchdog"
    );
    assert!(matches!(
        result,
        Err(sc_observability_types::v2::ExportError::RetryAttemptsExhausted { .. })
    ));
}

#[test]
fn loopback_retry_attempt_limit_returns_typed_exhaustion() {
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
    let (retry_wait_tx, retry_wait_rx) = mpsc::channel();
    let exporter = Arc::new(
        OtlpHttpExporter::for_endpoint_with_retry(
            format!("http://{address}"),
            retry_policy(3, 100, 200, 5_000, 4_000, 0),
            1,
            Some(retry_wait_tx),
        )
        .expect("construct exporter"),
    );
    let export = Arc::clone(&exporter);
    let export_thread = thread::spawn(move || {
        export.send_payload_sync(
            "logs",
            &build_logs_payload(&[super::implementation::log_record(&sample_log())]),
        )
    });
    assert_eq!(
        retry_wait_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("request entered the retry backoff"),
        Duration::from_secs(4)
    );

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
fn retry_wait_notification_is_retained_before_receiver_waits() {
    let (notification_tx, notification_rx) = mpsc::channel();

    let cancel = std::sync::atomic::AtomicBool::new(true);
    assert!(!super::implementation::wait_cancelable_with_observer(
        Duration::ZERO,
        &cancel,
        Some(&notification_tx),
    ));

    assert!(
        notification_rx
            .try_recv()
            .is_ok_and(|delay| delay.is_zero()),
        "retry notification remains queued until the receiver waits"
    );
}

struct RetryObserverUnrelated {
    delay: mpsc::Receiver<Duration>,
    exporter: Arc<OtlpHttpExporter>,
    export: thread::JoinHandle<Result<(), sc_observability_types::v2::ExportError>>,
    server: thread::JoinHandle<()>,
}

fn start_unrelated_retry_observer() -> RetryObserverUnrelated {
    let unrelated_listener = TcpListener::bind("127.0.0.1:0").expect("bind unrelated listener");
    let unrelated_address = unrelated_listener
        .local_addr()
        .expect("unrelated listener address");
    let unrelated_server = thread::spawn(move || {
        unrelated_listener
            .set_nonblocking(true)
            .expect("make unrelated listener nonblocking");
        let deadline = Instant::now() + RETRY_OBSERVER_SERVER_WATCHDOG;
        let mut stream = accept_retry_observer_request(&unrelated_listener, deadline);
        set_retry_observer_io_deadlines(&stream, deadline);
        let request = read_retry_observer_request(&mut stream, deadline);
        assert!(request.contains("\"hello\""));
        stream
            .write_all(
                b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .expect("write unrelated retry response");
    });

    let (unrelated_delay_tx, unrelated_delay_rx) = mpsc::channel();
    let exporter = Arc::new(
        OtlpHttpExporter::for_endpoint_with_retry(
            format!("http://{unrelated_address}"),
            retry_policy(1, 10_000, 10_000, 30_000, 1_000, 0),
            1,
            Some(unrelated_delay_tx),
        )
        .expect("construct unrelated exporter"),
    );
    let for_export = Arc::clone(&exporter);
    let export = thread::spawn(move || for_export.send_payload_sync("logs", &logs_payload()));
    RetryObserverUnrelated {
        delay: unrelated_delay_rx,
        exporter,
        export,
        server: unrelated_server,
    }
}

#[test]
fn retry_wait_observer_is_scoped_to_its_worker() {
    const UNRELATED_DELAY: Duration = Duration::from_secs(10);
    const TARGET_DELAY: Duration = Duration::from_millis(100);
    const OBSERVER_WATCHDOG: Duration = Duration::from_secs(2);

    let target_listener = TcpListener::bind("127.0.0.1:0").expect("bind target listener");
    let target_address = target_listener
        .local_addr()
        .expect("target listener address");
    let (target_request_tx, target_request_rx) = mpsc::channel();
    let (release_target_response_tx, release_target_response_rx) = mpsc::channel();
    let target_server = retrying_server(
        target_listener,
        target_request_tx,
        release_target_response_rx,
    );
    let (target_delay_tx, target_delay_rx) = mpsc::channel();
    let target_exporter = OtlpHttpExporter::for_endpoint_with_retry_timeout(
        format!("http://{target_address}"),
        retry_policy(1, 100, 100, 10_000, 6_000, 0),
        6_000,
        1,
        Some(target_delay_tx),
    )
    .expect("construct target exporter");
    let target_export =
        thread::spawn(move || target_exporter.send_payload_sync("logs", &logs_payload()));
    target_request_rx
        .recv_timeout(OBSERVER_WATCHDOG)
        .expect("target request reaches the server and waits for its response gate");

    let unrelated = start_unrelated_retry_observer();
    assert_eq!(
        unrelated
            .delay
            .recv_timeout(OBSERVER_WATCHDOG)
            .expect("unrelated worker entered its retry wait"),
        UNRELATED_DELAY
    );

    assert!(
        matches!(
            target_delay_rx.recv_timeout(RETRY_OBSERVER_NEGATIVE_WINDOW),
            Err(mpsc::RecvTimeoutError::Timeout)
        ),
        "unrelated retry wait cannot notify the live target observer"
    );
    release_target_response_tx
        .send(())
        .expect("release target request to enter its own retry wait");

    assert_eq!(
        target_delay_rx
            .recv_timeout(OBSERVER_WATCHDOG)
            .expect("target worker entered its own retry wait"),
        TARGET_DELAY
    );
    target_export
        .join()
        .expect("join target export")
        .expect("target retry succeeds");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let shutdown = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(1), unrelated.exporter.shutdown_async()).await
    });
    assert!(
        shutdown.is_ok(),
        "shutdown did not cancel unrelated retry wait"
    );
    assert!(matches!(
        unrelated
            .export
            .join()
            .expect("join unrelated export")
            .expect_err("unrelated retry wait is cancelled"),
        sc_observability_types::v2::ExportError::ShutdownCancelledRetry { .. }
    ));
    join_retry_observer_server(target_server);
    join_retry_observer_server(unrelated.server);
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
fn flush_control_does_not_hold_producer_admission_while_its_reply_is_withheld() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    let (first_request_tx, first_request_rx) = mpsc::channel();
    let (release_first_tx, release_first_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut first, _) = listener.accept().expect("accept first request");
        assert!(read_request(&mut first).contains("\"hello\""));
        first_request_tx
            .send(())
            .expect("signal first request before releasing it");
        release_first_rx
            .recv()
            .expect("release first request after producer progress");
        first
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("complete first response");

        let (mut second, _) = listener.accept().expect("accept producer request");
        assert!(read_request(&mut second).contains("\"hello\""));
        second
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("complete producer response");
    });
    let (control_submitted_tx, control_submitted_rx) = mpsc::channel();
    let exporter = Arc::new(
        OtlpHttpExporter::for_control_ordering_test(
            format!("http://{address}"),
            control_submitted_tx,
        )
        .expect("construct exporter"),
    );

    <OtlpHttpExporter as LogExporter<LogEvent>>::export_logs(&*exporter, &[sample_log()])
        .expect("admit first request");
    first_request_rx
        .recv_timeout(CONTROL_ORDERING_WATCHDOG)
        .expect("worker enters the held first request");

    let (flush_result_rx, flush_thread) = exporter.flush_worker_for_test();
    control_submitted_rx
        .recv_timeout(CONTROL_ORDERING_WATCHDOG)
        .expect("flush command enters the bounded control channel");

    let producer = Arc::clone(&exporter);
    let (producer_result_tx, producer_result_rx) = mpsc::channel();
    let producer_thread = thread::spawn(move || {
        let result =
            <OtlpHttpExporter as LogExporter<LogEvent>>::export_logs(&*producer, &[sample_log()]);
        let _ = producer_result_tx.send(result);
    });
    let producer_result = match producer_result_rx.recv_timeout(CONTROL_ORDERING_WATCHDOG) {
        Ok(result) => result,
        Err(error) => {
            let _ = release_first_tx.send(());
            panic!("producer admission waited for the flush reply: {error:?}");
        }
    };
    producer_result.expect("producer admission progresses while flush reply is withheld");

    release_first_tx
        .send(())
        .expect("release held first response");
    flush_result_rx
        .recv_timeout(CONTROL_ORDERING_WATCHDOG)
        .expect("flush completes after the worker handles its command")
        .expect("flush preserves the successful drain");
    flush_thread.join().expect("join flush worker");
    producer_thread.join().expect("join producer");
    server.join().expect("join server");
    exporter
        .shutdown_blocking()
        .expect("shutdown the live worker");
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

#[test]
fn shutdown_blocking_is_rejected_from_entered_tokio() {
    let exporter = OtlpHttpExporter::for_endpoint("http://127.0.0.1:4318".to_owned())
        .expect("construct exporter");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let error = runtime.block_on(async { exporter.shutdown_blocking() });
    assert!(matches!(
        error,
        Err(sc_observability_types::v2::ExportError::BlockingBackendInAsyncContext { .. })
    ));
}

#[test]
fn prepared_immediate_retry_preserves_zero_and_attempt_limit() {
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
    let transport = crate::config::OtelConfig {
        enabled: true,
        backend: crate::config::ExporterBackend::SyncHttp,
        protocol: crate::config::OtlpProtocol::HttpJson,
        endpoint: Some(
            crate::config::OtlpEndpoint::new_typed(format!("http://{address}")).unwrap(),
        ),
        sync_http_retry: Some(retry_policy(1, 0, 0, 30_000, 20, 0)),
        ..crate::config::OtelConfig::default()
    };
    assert!(crate::config::validated_transport_bounds(&transport).is_err());
    let config = crate::config::TelemetryConfig {
        service_name: ServiceName::new("zero-delay").unwrap(),
        resource: crate::config::ResourceAttributes::default(),
        transport,
        logs: Some(crate::config::LogsConfig::default()),
        traces: None,
        metrics: None,
    };
    let bounds = crate::config::validated_released_telemetry_bounds(&config).unwrap();
    let connection =
        crate::config::prepared_backend_connection(&config.transport, &bounds).unwrap();
    let (delay_tx, delay_rx) = mpsc::channel();
    let exporter = OtlpHttpExporter::for_prepared_test(&connection, &bounds, delay_tx).unwrap();
    let error = exporter
        .send_payload_sync("logs", &logs_payload())
        .expect_err("retry attempt limit must stop the sequence");
    server.join().expect("join server");
    assert!(matches!(
        error,
        sc_observability_types::v2::ExportError::RetryAttemptsExhausted { .. }
    ));
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert_eq!(
        delay_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        Duration::ZERO
    );
}

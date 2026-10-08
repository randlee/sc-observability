//! Runs the built `sc-otel` binary, as a fresh process without a Tokio
//! runtime, against a loopback collector that captures each OTLP request.

use std::{
    io::{BufRead, BufReader, ErrorKind, Read, Write},
    net::{TcpListener, TcpStream},
    process::{Child, ChildStdin, Command, Output, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

/// Watchdog for a fixture that never receives its request; not a pass condition.
const FIXTURE_WATCHDOG: Duration = Duration::from_secs(20);
/// Hard deadline for each CLI child, independent of the collector fixture.
const CHILD_WATCHDOG: Duration = Duration::from_secs(20);
const FIXTURE_WATCHDOG_WAKE: &str = "fixture-watchdog-wake\n";
const TRACE_ID: &str = "4bf92f3577b34da6a3ce929d0e0e4736";
const SPAN_ID: &str = "00f067aa0ba902b7";
const SECRET: &str = "s3cret-token-value";
const PROXY_AND_TRUST_ENV: [&str; 10] = [
    "HTTP_PROXY",
    "http_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "ALL_PROXY",
    "all_proxy",
    "NO_PROXY",
    "no_proxy",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
];

struct Captured {
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Captured {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    fn body_contains(&self, needle: &[u8]) -> bool {
        self.body
            .windows(needle.len())
            .any(|window| window == needle)
    }
}

/// Accepts exactly one request, answers with `status` and returns it.
struct Collector {
    listener: TcpListener,
    endpoint: String,
}

impl Collector {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind collector");
        let endpoint = format!("http://{}", listener.local_addr().expect("address"));
        Self { listener, endpoint }
    }

    /// Serves one request on a thread so the CLI can run on the test thread.
    fn serve(self, status: &'static str) -> thread::JoinHandle<Captured> {
        thread::spawn(move || {
            let address = self.listener.local_addr().expect("collector address");
            let (accepted, watch) = mpsc::channel();
            let watchdog = thread::spawn(move || {
                if let Err(mpsc::RecvTimeoutError::Timeout) = watch.recv_timeout(FIXTURE_WATCHDOG) {
                    let mut wake = TcpStream::connect(address).expect("wake collector watchdog");
                    wake.write_all(FIXTURE_WATCHDOG_WAKE.as_bytes())
                        .expect("write watchdog wake");
                }
            });
            let (stream, _) = self.listener.accept().expect("accept");
            let _ = accepted.send(());
            watchdog.join().expect("collector watchdog");
            handle(stream, status)
        })
    }

    /// After the CLI has exited, proves it never connected.
    fn assert_untouched(&self) {
        self.listener
            .set_nonblocking(true)
            .expect("nonblocking untouched check");
        match self.listener.accept() {
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            other => panic!("the collector received a connection: {other:?}"),
        }
    }
}

fn handle(mut stream: TcpStream, status: &str) -> Captured {
    stream.set_nonblocking(false).expect("blocking stream");
    stream
        .set_read_timeout(Some(FIXTURE_WATCHDOG))
        .expect("read watchdog");
    let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
    let mut line = String::new();
    reader.read_line(&mut line).expect("request line");
    assert_ne!(line, FIXTURE_WATCHDOG_WAKE, "no request arrived");
    let path = line.split_whitespace().nth(1).expect("path").to_owned();
    let mut headers = Vec::new();
    loop {
        line.clear();
        reader.read_line(&mut line).expect("header line");
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').expect("header");
        headers.push((name.to_ascii_lowercase(), value.trim().to_owned()));
    }
    let length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .map_or(0, |(_, value)| value.parse().expect("content length"));
    let mut body = vec![0; length];
    reader.read_exact(&mut body).expect("body");
    write!(
        stream,
        "HTTP/1.1 {status}\r\ncontent-type: application/x-protobuf\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
    )
    .expect("response");
    Captured {
        path,
        headers,
        body,
    }
}

/// The binary with ambient `OTel`, proxy and trust settings removed and an empty
/// working directory, so a test sees only the configuration it sets.
fn sc_otel(directory: &tempfile::TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sc-otel"));
    command.current_dir(directory.path());
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("OTEL_") {
            command.env_remove(key);
        }
    }
    for key in PROXY_AND_TRUST_ENV {
        command.env_remove(key);
    }
    command
}

#[test]
fn sc_otel_removes_ambient_proxy_and_trust_settings() {
    let directory = tempfile::tempdir().expect("tempdir");
    let command = sc_otel(&directory);
    let removed = command.get_envs().collect::<Vec<_>>();
    for key in PROXY_AND_TRUST_ENV {
        assert!(
            removed
                .iter()
                .any(|(name, value)| *name == key && value.is_none()),
            "{key} was not removed from the spawned CLI environment"
        );
    }
}

struct RunningChild {
    child: Child,
    stdout: thread::JoinHandle<Vec<u8>>,
    stderr: thread::JoinHandle<Vec<u8>>,
    deadline: Instant,
}

impl RunningChild {
    fn spawn(command: &mut Command, timeout: Duration) -> Self {
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn sc-otel");
        let stdout = child.stdout.take().expect("stdout");
        let stderr = child.stderr.take().expect("stderr");
        Self {
            child,
            stdout: thread::spawn(move || {
                let mut bytes = Vec::new();
                let mut stdout = stdout;
                stdout.read_to_end(&mut bytes).expect("read child stdout");
                bytes
            }),
            stderr: thread::spawn(move || {
                let mut bytes = Vec::new();
                let mut stderr = stderr;
                stderr.read_to_end(&mut bytes).expect("read child stderr");
                bytes
            }),
            deadline: Instant::now() + timeout,
        }
    }

    fn stdin(&mut self) -> ChildStdin {
        self.child.stdin.take().expect("stdin")
    }

    fn wait(mut self) -> Output {
        let (status, timed_out) = loop {
            match self.child.try_wait().expect("poll sc-otel") {
                Some(status) => break (status, false),
                None if Instant::now() < self.deadline => {
                    thread::sleep(Duration::from_millis(5));
                }
                None => {
                    if let Err(error) = self.child.kill() {
                        assert_eq!(
                            error.kind(),
                            ErrorKind::InvalidInput,
                            "kill sc-otel: {error}"
                        );
                    }
                    break (self.child.wait().expect("reap timed-out sc-otel"), true);
                }
            }
        };
        let stdout = self.stdout.join().expect("stdout reader");
        let stderr = self.stderr.join().expect("stderr reader");
        assert!(
            !timed_out,
            "sc-otel exceeded hard deadline; status: {status}; stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&stdout),
            String::from_utf8_lossy(&stderr),
        );
        Output {
            status,
            stdout,
            stderr,
        }
    }
}

fn run(command: &mut Command) -> Output {
    run_with_deadline(command, CHILD_WATCHDOG)
}

fn run_with_deadline(command: &mut Command, timeout: Duration) -> Output {
    RunningChild::spawn(command, timeout).wait()
}

fn join_stdin_writer(writer: thread::JoinHandle<()>) {
    let deadline = Instant::now() + CHILD_WATCHDOG;
    while !writer.is_finished() {
        assert!(
            Instant::now() < deadline,
            "stdin writer exceeded hard deadline"
        );
        thread::sleep(Duration::from_millis(5));
    }
    writer.join().expect("stdin writer");
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).expect("hex"))
        .collect()
}

fn assert_no_files(directory: &tempfile::TempDir) {
    let entries = std::fs::read_dir(directory.path())
        .expect("read working directory")
        .count();
    assert_eq!(entries, 0, "sc-otel created files in its working directory");
}

#[test]
fn log_exports_native_record_with_cli_and_environment_configuration() {
    let directory = tempfile::tempdir().expect("tempdir");
    let collector = Collector::start();
    let endpoint = collector.endpoint.clone();
    let request = collector.serve("200 OK");
    let output = run(sc_otel(&directory)
        .env(
            "OTEL_EXPORTER_OTLP_HEADERS",
            "x-tenant=from-env,x-env-only=yes",
        )
        .args(["--endpoint", &endpoint, "--service", "cli-service"])
        .args(["--header", "x-tenant=from-cli"])
        .args(["log", "--body", "hello-from-cli", "--severity", "warn"])
        .args(["--trace-id", TRACE_ID, "--span-id", SPAN_ID])
        .args(["--attributes", r#"{"job":"build-42","attempt":2}"#]));
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let request = request.join().expect("collector");
    assert_eq!(request.path, "/v1/logs");
    assert_eq!(
        request.header("content-type"),
        Some("application/x-protobuf")
    );
    assert_eq!(request.header("x-tenant"), Some("from-cli"));
    assert_eq!(request.header("x-env-only"), Some("yes"));
    for needle in [
        &b"hello-from-cli"[..],
        b"WARN",
        b"cli-service",
        b"build-42",
        b"attempt",
        b"sc-otel",
        &hex(TRACE_ID),
        &hex(SPAN_ID),
    ] {
        assert!(
            request.body_contains(needle),
            "{:?} missing from the log request",
            String::from_utf8_lossy(needle)
        );
    }
    assert_no_files(&directory);
}

#[test]
fn span_exports_completed_native_span() {
    let directory = tempfile::tempdir().expect("tempdir");
    let collector = Collector::start();
    let endpoint = collector.endpoint.clone();
    let request = collector.serve("200 OK");
    let mut child = RunningChild::spawn(
        sc_otel(&directory)
            .args(["--endpoint", &endpoint, "span", "--name", "deploy-step"])
            .args(["--trace-id", TRACE_ID, "--parent-span-id", SPAN_ID])
            .args(["--kind", "client", "--error", "rollout-timed-out"])
            .args(["--start-time-unix-nano", "1700000000000000000"])
            .args(["--end-time-unix-nano", "1700000005000000000"])
            .args(["--attributes", "-"])
            .stdin(Stdio::piped()),
        CHILD_WATCHDOG,
    );
    child
        .stdin()
        .write_all(br#"{"region":"eu-west"}"#)
        .expect("write attributes");
    let output = child.wait();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let request = request.join().expect("collector");
    assert_eq!(request.path, "/v1/traces");
    for needle in [
        &b"deploy-step"[..],
        b"rollout-timed-out",
        b"region",
        b"eu-west",
        &hex(TRACE_ID),
        &hex(SPAN_ID),
        // OTLP Span.flags field 16: sampled plus the known-remote parent bits.
        &[0x85, 0x01, 0x01, 0x03, 0x00, 0x00],
        &1_700_000_000_000_000_000_u64.to_le_bytes(),
        &1_700_000_005_000_000_000_u64.to_le_bytes(),
    ] {
        assert!(
            request.body_contains(needle),
            "{:?} missing from the span request",
            String::from_utf8_lossy(needle)
        );
    }
    assert_no_files(&directory);
}

#[test]
fn metric_exports_one_measurement_to_the_environment_endpoint() {
    let directory = tempfile::tempdir().expect("tempdir");
    let collector = Collector::start();
    let endpoint = collector.endpoint.clone();
    let request = collector.serve("200 OK");
    let output = run(sc_otel(&directory)
        .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
        .args(["metric", "--name", "jobs.completed", "--kind", "counter"])
        .args(["--value", "3", "--unit", "{job}"])
        .args(["--attributes", r#"{"queue":"default"}"#]));
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let request = request.join().expect("collector");
    assert_eq!(request.path, "/v1/metrics");
    for needle in [&b"jobs.completed"[..], b"{job}", b"queue", b"default"] {
        assert!(
            request.body_contains(needle),
            "{:?} missing from the metric request",
            String::from_utf8_lossy(needle)
        );
    }
    assert_no_files(&directory);
}

#[test]
fn metric_counter_and_histogram_edge_values_export_without_cli_range_validation() {
    let cases = [
        ("negative-counter", "counter", "-1"),
        ("negative-histogram", "histogram", "-1"),
        ("nan-counter", "counter", "NaN"),
        ("nan-histogram", "histogram", "NaN"),
        ("infinite-counter", "counter", "inf"),
        ("infinite-histogram", "histogram", "inf"),
    ];
    for (name, kind, value) in cases {
        let directory = tempfile::tempdir().expect("tempdir");
        let collector = Collector::start();
        let endpoint = collector.endpoint.clone();
        let request = collector.serve("200 OK");
        let output = run(sc_otel(&directory)
            .args(["--endpoint", &endpoint])
            .args(["metric", "--name", name, "--kind", kind, "--value", value]));
        assert_eq!(output.status.code(), Some(0), "{name}: {output:?}");
        let request = request.join().expect("collector");
        assert_eq!(request.path, "/v1/metrics", "{name}");
        assert!(
            request.body_contains(name.as_bytes()),
            "{name} was not present in the metric request"
        );
        assert_no_files(&directory);
    }
}

#[test]
fn rejected_export_exits_7_without_printing_credentials() {
    let directory = tempfile::tempdir().expect("tempdir");
    let collector = Collector::start();
    let endpoint = collector
        .endpoint
        .replace("http://", "http://user:s3cret-pass@");
    let request = collector.serve("400 Bad Request");
    let output = run(sc_otel(&directory)
        .args(["--endpoint", &endpoint])
        .args(["--header", &format!("authorization=Bearer {SECRET}")])
        .args(["log", "--body", "rejected"]));
    assert_eq!(output.status.code(), Some(7), "{output:?}");
    let request = request.join().expect("collector");
    assert_eq!(
        request.header("authorization"),
        Some(format!("Bearer {SECRET}").as_str())
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("SC_OBSERVABILITY_OTLP_EXPORT_FAILED"),
        "{stderr}"
    );
    assert!(stderr.contains("export failed"), "{stderr}");
    assert!(
        stderr.contains("check the endpoint and --root-certificate; raise --timeout"),
        "{stderr}"
    );
    assert!(!stderr.contains(SECRET), "{stderr}");
    assert!(!stderr.contains("s3cret-pass"), "{stderr}");
    assert!(output.stdout.is_empty());
}

#[test]
fn rejected_export_does_not_print_environment_header_values() {
    const ENV_SECRET: &str = "Bearer environment-secret";

    let directory = tempfile::tempdir().expect("tempdir");
    let collector = Collector::start();
    let endpoint = collector.endpoint.clone();
    let request = collector.serve("400 Bad Request");
    let output = run(sc_otel(&directory)
        .env(
            "OTEL_EXPORTER_OTLP_HEADERS",
            format!("authorization={ENV_SECRET}"),
        )
        .args(["--endpoint", &endpoint, "--timeout", "5"])
        .args(["log", "--body", "rejected"]));
    assert_eq!(output.status.code(), Some(7), "{output:?}");
    let request = request.join().expect("collector");
    assert_eq!(request.header("authorization"), Some(ENV_SECRET));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("export failed"), "{stderr}");
    assert!(!stderr.contains(ENV_SECRET), "{stderr}");
    assert!(output.stdout.is_empty());
}

#[test]
fn malformed_header_exits_3_without_printing_credentials() {
    let directory = tempfile::tempdir().expect("tempdir");
    let output = run(sc_otel(&directory)
        .args(["--header", &format!("Authorization: Bearer {SECRET}")])
        .args(["log", "--body", "rejected"]));
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("header must use NAME=VALUE"), "{stderr}");
    assert!(!stderr.contains(SECRET), "{stderr}");
    assert!(output.stdout.is_empty());
    assert_no_files(&directory);
}

#[test]
fn invalid_input_exits_3_and_sends_nothing() {
    let directory = tempfile::tempdir().expect("tempdir");
    let collector = Collector::start();
    let endpoint = collector.endpoint.clone();
    let cases: [(&[&str], &str); 3] = [
        (
            &[
                "log",
                "--body",
                "x",
                "--attributes",
                r#"{"nested":{"a":1}}"#,
            ],
            "SC_OBSERVABILITY_OTLP_SYNC_INVALID_RECORD",
        ),
        (
            &[
                "span",
                "--name",
                "x",
                "--start-time-unix-nano",
                "2",
                "--end-time-unix-nano",
                "1",
            ],
            "SC_OBSERVABILITY_OTLP_SYNC_INVALID_RECORD",
        ),
        (
            &["--root-certificate", "missing.pem", "log", "--body", "x"],
            "SC_OBSERVABILITY_OTLP_SYNC_INVALID_CONFIG",
        ),
    ];
    for (args, code) in cases {
        let output = run(sc_otel(&directory)
            .args(["--endpoint", &endpoint])
            .args(args));
        assert_eq!(output.status.code(), Some(3), "{args:?}: {output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(code), "{args:?}: {stderr}");
    }
    collector.assert_untouched();
    assert_no_files(&directory);
}

#[cfg(unix)]
#[test]
fn non_utf8_endpoint_environment_exits_3_without_sending() {
    use std::os::unix::ffi::OsStringExt;

    let directory = tempfile::tempdir().expect("tempdir");
    let mut command = sc_otel(&directory);
    command
        .env(
            "OTEL_EXPORTER_OTLP_ENDPOINT",
            std::ffi::OsString::from_vec(vec![0xff]),
        )
        .args(["log", "--body", "x"]);

    let output = run(&mut command);
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("SC_OBSERVABILITY_OTLP_SYNC_INVALID_CONFIG"),
        "{stderr}"
    );
    assert!(
        stderr.contains("OTEL_EXPORTER_OTLP_ENDPOINT must be valid UTF-8"),
        "{stderr}"
    );
    assert_no_files(&directory);
}

#[test]
fn invalid_metric_names_exit_3_and_send_nothing_for_every_instrument_kind() {
    let directory = tempfile::tempdir().expect("tempdir");
    let collector = Collector::start();
    let endpoint = collector.endpoint.clone();
    for kind in ["counter", "up-down-counter", "gauge", "histogram"] {
        let output = run(sc_otel(&directory).args([
            "--endpoint",
            &endpoint,
            "metric",
            "--name",
            "1-not-an-instrument-name",
            "--kind",
            kind,
            "--value",
            "1",
        ]));
        assert_eq!(output.status.code(), Some(3), "{kind}: {output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("SC_OBSERVABILITY_OTLP_SYNC_INVALID_RECORD"),
            "{kind}: {stderr}"
        );
    }
    collector.assert_untouched();
    assert_no_files(&directory);
}

#[test]
fn oversized_standard_input_is_rejected_before_export() {
    let directory = tempfile::tempdir().expect("tempdir");
    let collector = Collector::start();
    let mut child = RunningChild::spawn(
        sc_otel(&directory)
            .args(["--endpoint", &collector.endpoint])
            .args(["log", "--body", "x", "--attributes", "-"])
            .stdin(Stdio::piped()),
        CHILD_WATCHDOG,
    );
    let mut stdin = child.stdin();
    let writer = thread::spawn(move || {
        let chunk = vec![b' '; 64 * 1024];
        // The CLI stops reading at the limit and exits, closing the pipe.
        for _ in 0..64 {
            if stdin.write_all(&chunk).is_err() {
                break;
            }
        }
    });
    let output = child.wait();
    join_stdin_writer(writer);
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("SC_OBSERVABILITY_OTLP_SYNC_INPUT_LIMIT_EXCEEDED"),
        "{stderr}"
    );
    collector.assert_untouched();
}

#[test]
fn child_deadline_kills_and_reaps_when_standard_input_never_reaches_eof() {
    let directory = tempfile::tempdir().expect("tempdir");
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_with_deadline(
            sc_otel(&directory)
                .args(["log", "--body", "blocked", "--attributes", "-"])
                .stdin(Stdio::piped()),
            Duration::from_millis(100),
        )
    }))
    .expect_err("open stdin must exceed the hard deadline");
    let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .expect("panic message");
    assert!(message.contains("exceeded hard deadline"), "{message}");
}

#[test]
fn usage_errors_exit_2_and_help_exits_0() {
    let directory = tempfile::tempdir().expect("tempdir");
    for args in [
        &["--store", "queue.sqlite", "log", "--body", "x"][..],
        &["emit", "--log", "{}"],
        &[
            "log",
            "--body",
            "x",
            "--trace-id",
            "xyz",
            "--span-id",
            SPAN_ID,
        ],
    ] {
        let output = run(sc_otel(&directory).args(args));
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
    }
    let output = run(sc_otel(&directory).arg("--help"));
    assert_eq!(output.status.code(), Some(0));
    let help = String::from_utf8_lossy(&output.stdout);
    for command in ["log", "span", "metric"] {
        assert!(help.contains(command), "{help}");
    }
    assert_no_files(&directory);
}
